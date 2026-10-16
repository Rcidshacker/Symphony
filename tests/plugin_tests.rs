//! The WASM plugin runtime with real (hand-written WAT) plugins: events in, host calls out,
//! permissions, metering and memory limits.

use std::path::Path;
use std::sync::{Arc, Mutex};

use symphony::plugins::{HostFunctions, PluginEvent, PluginRuntime, PluginStatus, TrackInfo};

/// Everything the plugins did to the outside world
#[derive(Clone, Default)]
struct Spy {
    logs: Arc<Mutex<Vec<String>>>,
    notes: Arc<Mutex<Vec<(String, String)>>>,
    discord: Arc<Mutex<Vec<(String, String)>>>,
    scrobbles: Arc<Mutex<Vec<(String, String)>>>,
}

impl Spy {
    fn host(&self) -> HostFunctions {
        let (logs, notes, discord, scrobbles) = (
            self.logs.clone(),
            self.notes.clone(),
            self.discord.clone(),
            self.scrobbles.clone(),
        );
        HostFunctions::new()
            .with_log(move |m| logs.lock().unwrap().push(m.to_string()))
            .with_notification(move |t, b| notes.lock().unwrap().push((t.into(), b.into())))
            .with_discord(move |t, a, _, _| discord.lock().unwrap().push((t.into(), a.into())))
            .with_scrobble(move |t, a, _| scrobbles.lock().unwrap().push((t.into(), a.into())))
            .with_get_track(|| Some(TrackInfo::new("t1", "Hello", "World").with_duration(200)))
            .with_get_volume(|| 0.25)
    }

    fn logs(&self) -> Vec<String> {
        self.logs.lock().unwrap().clone()
    }
}

/// Bump allocator + log/notify/discord/scrobble/get_track on every event
const ECHO: &str = r#"
(module
  (import "symphony" "log" (func $log (param i32 i32)))
  (import "symphony" "notify" (func $notify (param i32 i32 i32 i32) (result i32)))
  (import "symphony" "discord_presence" (func $discord (param i32 i32 i32 i32) (result i32)))
  (import "symphony" "scrobble" (func $scrobble (param i32 i32 i32 i32) (result i32)))
  (import "symphony" "get_track" (func $get_track (param i32 i32) (result i32)))
  (import "symphony" "get_volume" (func $get_volume (result f32)))
  (memory (export "memory") 1)
  (data (i32.const 0) "hi")
  (global $heap (mut i32) (i32.const 4096))
  (func (export "alloc") (param $n i32) (result i32)
    (local $p i32)
    (local.set $p (global.get $heap))
    (global.set $heap (i32.add (local.get $p) (local.get $n)))
    (local.get $p))
  (func (export "plugin_init") (call $log (i32.const 0) (i32.const 2)))
  (func (export "plugin_shutdown") (call $log (i32.const 0) (i32.const 2)))
  (func (export "on_event") (param $p i32) (param $n i32)
    (call $log (local.get $p) (local.get $n))
    (drop (call $notify (i32.const 0) (i32.const 2) (i32.const 0) (i32.const 2)))
    (drop (call $discord (i32.const 0) (i32.const 2) (i32.const 0) (i32.const 2)))
    (drop (call $scrobble (i32.const 0) (i32.const 2) (i32.const 0) (i32.const 2)))
    ;; the current track, as JSON, from the host
    (call $log (i32.const 1024) (call $get_track (i32.const 1024) (i32.const 512)))
    (if (f32.gt (call $get_volume) (f32.const 0.2)) (then (call $log (i32.const 0) (i32.const 1))))))
"#;

/// Spins forever on the first event
const LOOPER: &str = r#"
(module
  (memory (export "memory") 1)
  (func (export "alloc") (param i32) (result i32) (i32.const 1024))
  (func (export "on_event") (param i32 i32) (loop (br 0))))
"#;

fn install(dir: &Path, name: &str, permissions: &[&str], wat_source: &str) {
    let folder = dir.join(name);
    std::fs::create_dir_all(&folder).unwrap();
    let permissions = permissions
        .iter()
        .map(|p| format!("\"{p}\""))
        .collect::<Vec<_>>()
        .join(", ");
    std::fs::write(
        folder.join("plugin.toml"),
        format!(
            "name = \"{name}\"\nversion = \"1.0.0\"\nauthor = \"test\"\ndescription = \"test\"\npermissions = [{permissions}]\n"
        ),
    )
    .unwrap();
    std::fs::write(
        folder.join("plugin.wasm"),
        wat::parse_str(wat_source).unwrap(),
    )
    .unwrap();
}

fn runtime(dir: &Path, spy: &Spy) -> PluginRuntime {
    let mut runtime = PluginRuntime::new(dir.to_path_buf()).unwrap();
    runtime.set_host(spy.host());
    runtime
}

fn track_changed() -> PluginEvent {
    PluginEvent::TrackChanged(TrackInfo::new("t1", "Hello", "World"))
}

const ALL: &[&str] = &[
    "track_info",
    "playback_state",
    "notifications",
    "discord",
    "lastfm",
];

#[tokio::test]
async fn a_plugin_runs_receives_events_and_uses_the_host() {
    let (dir, spy) = (tempfile::tempdir().unwrap(), Spy::default());
    install(dir.path(), "echo", ALL, ECHO);
    let mut runtime = runtime(dir.path(), &spy);

    assert_eq!(runtime.load_all_plugins().await.unwrap(), ["echo"]);
    assert_eq!(spy.logs(), ["hi"], "plugin_init ran");

    runtime.trigger_event(&track_changed());
    let logs = spy.logs();
    assert!(
        logs[1].contains("\"TrackChanged\"") && logs[1].contains("Hello"),
        "{logs:?}"
    );
    assert!(
        logs[2].contains("\"id\":\"t1\"") && logs[2].contains("\"duration\":200"),
        "get_track: {logs:?}"
    );
    assert_eq!(logs[3], "h", "get_volume returned 0.25");
    assert_eq!(
        *spy.notes.lock().unwrap(),
        [("hi".to_string(), "hi".to_string())]
    );
    assert_eq!(
        *spy.discord.lock().unwrap(),
        [("hi".to_string(), "hi".to_string())]
    );
    assert_eq!(
        *spy.scrobbles.lock().unwrap(),
        [("hi".to_string(), "hi".to_string())]
    );
    assert_eq!(runtime.denied_calls("echo"), 0);

    // shutdown hook runs when the plugin is unloaded
    runtime.unload_plugin("echo").unwrap();
    assert_eq!(spy.logs().last().map(String::as_str), Some("hi"));
    assert!(runtime.list_plugins().is_empty());
}

#[tokio::test]
async fn missing_permissions_block_events_and_host_calls() {
    let (dir, spy) = (tempfile::tempdir().unwrap(), Spy::default());
    // may see track events and read the track, but not notify, share on Discord or scrobble
    install(dir.path(), "limited", &["track_info"], ECHO);
    // no permissions at all: gets no track events
    install(dir.path(), "blind", &[], ECHO);
    let mut runtime = runtime(dir.path(), &spy);
    runtime.load_all_plugins().await.unwrap();

    runtime.trigger_event(&track_changed());

    assert!(spy.notes.lock().unwrap().is_empty());
    assert!(spy.discord.lock().unwrap().is_empty());
    assert!(spy.scrobbles.lock().unwrap().is_empty());
    assert_eq!(
        runtime.denied_calls("limited"),
        4,
        "notify, discord, scrobble, get_volume were refused"
    );
    assert_eq!(
        runtime.denied_calls("blind"),
        0,
        "never got the event, so never tried"
    );
    // only plugin_init lines from both, plus limited's event + track
    let events = spy
        .logs()
        .iter()
        .filter(|l| l.contains("TrackChanged"))
        .count();
    assert_eq!(events, 1);
    assert_eq!(
        runtime.get_plugin_status("limited"),
        Some(PluginStatus::Active)
    );
}

#[tokio::test]
async fn a_plugin_stuck_in_a_loop_is_stopped_and_the_others_keep_working() {
    let (dir, spy) = (tempfile::tempdir().unwrap(), Spy::default());
    install(dir.path(), "looper", &["track_info"], LOOPER);
    install(dir.path(), "echo", ALL, ECHO);
    let mut runtime = runtime(dir.path(), &spy);
    runtime.load_all_plugins().await.unwrap();

    let started = std::time::Instant::now();
    runtime.trigger_event(&track_changed());
    assert!(
        started.elapsed() < std::time::Duration::from_secs(10),
        "the loop was cut off"
    );

    assert_eq!(
        runtime.get_plugin_status("looper"),
        Some(PluginStatus::Error)
    );
    let error = runtime.plugin_error("looper").unwrap().to_lowercase();
    assert!(error.contains("fuel"), "{error}");
    assert!(!runtime.is_plugin_enabled("looper"));
    assert!(
        spy.logs().iter().any(|l| l.contains("TrackChanged")),
        "echo still got its event"
    );

    // a failed plugin gets no more events; enabling it again lets it fail again
    runtime.trigger_event(&track_changed());
    assert_eq!(
        runtime.get_plugin_status("looper"),
        Some(PluginStatus::Error)
    );
    runtime.enable_plugin("looper").unwrap();
    assert_eq!(
        runtime.get_plugin_status("looper"),
        Some(PluginStatus::Active)
    );
}

#[tokio::test]
async fn broken_plugins_are_rejected_without_crashing_the_host() {
    let (dir, spy) = (tempfile::tempdir().unwrap(), Spy::default());
    install(
        dir.path(),
        "nomemory",
        &[],
        r#"(module (func (export "on_event") (param i32 i32)))"#,
    );
    install(
        dir.path(),
        "noalloc",
        &[],
        r#"(module (memory (export "memory") 1) (func (export "on_event") (param i32 i32)))"#,
    );
    install(
        dir.path(),
        "hog",
        &[],
        r#"(module (memory (export "memory") 2000))"#,
    ); // 128 MB
    install(
        dir.path(),
        "badimport",
        &[],
        r#"(module (import "symphony" "format_disk" (func)) (memory (export "memory") 1))"#,
    );
    std::fs::create_dir_all(dir.path().join("garbage")).unwrap();
    std::fs::write(
        dir.path().join("garbage/plugin.toml"),
        "name = \"garbage\"\nversion = \"1\"\nauthor = \"x\"\ndescription = \"x\"\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("garbage/plugin.wasm"),
        b"definitely not wasm",
    )
    .unwrap();
    install(dir.path(), "echo", ALL, ECHO);

    let mut runtime = runtime(dir.path(), &spy);
    assert_eq!(
        runtime.load_all_plugins().await.unwrap(),
        ["echo"],
        "only the valid plugin loads"
    );

    // and each one is refused for its own reason
    for (name, reason) in [
        ("nomemory", "export `memory`"),
        ("noalloc", "alloc"),
        ("hog", "memory"),
        ("badimport", "format_disk"),
        ("garbage", ""),
    ] {
        let folder = dir.path().join(name);
        let error = runtime
            .load_plugin(&folder.join("plugin.toml"), &folder.join("plugin.wasm"))
            .await
            .expect_err(name)
            .to_string();
        assert!(error.contains(reason), "{name}: {error}");
    }
}

#[tokio::test]
async fn auto_load_picks_plugins_and_reload_restarts_one() {
    let (dir, spy) = (tempfile::tempdir().unwrap(), Spy::default());
    install(dir.path(), "echo", ALL, ECHO);
    install(dir.path(), "other", ALL, ECHO);
    let mut runtime = runtime(dir.path(), &spy);
    runtime.set_auto_load(vec!["echo".to_string()]);
    assert_eq!(runtime.load_all_plugins().await.unwrap(), ["echo"]);

    let before = spy.logs().len();
    runtime.reload_plugin("echo").await.unwrap();
    assert_eq!(spy.logs().len(), before + 2, "shutdown then init ran again");

    runtime.disable_plugin("echo").unwrap();
    runtime.trigger_event(&track_changed());
    assert!(!spy.logs().iter().any(|l| l.contains("TrackChanged")));
}
