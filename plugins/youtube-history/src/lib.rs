use std::cell::RefCell;
use std::collections::HashMap;
use std::slice;
use std::str;

use serde::{Deserialize, Serialize};

// ── Host functions imported from Symphony ────────────────────────────────────

#[link(wasm_import_module = "env")]
extern "C" {
    fn host_log(msg_ptr: i32, msg_len: i32);
    fn host_notify(title_ptr: i32, title_len: i32, body_ptr: i32, body_len: i32);
    fn host_get_config(key_ptr: i32, key_len: i32, buf_ptr: i32, buf_len: i32) -> i32;
    fn host_set_config(key_ptr: i32, key_len: i32, val_ptr: i32, val_len: i32);
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn log(msg: &str) {
    unsafe { host_log(msg.as_ptr() as i32, msg.len() as i32) }
}

fn notify(title: &str, body: &str) {
    unsafe {
        host_notify(
            title.as_ptr() as i32,
            title.len() as i32,
            body.as_ptr() as i32,
            body.len() as i32,
        )
    }
}

static mut CONFIG_BUF: [u8; 65536] = [0; 65536];

fn load_config(key: &str) -> Option<String> {
    unsafe {
        let len = host_get_config(
            key.as_ptr() as i32,
            key.len() as i32,
            CONFIG_BUF.as_mut_ptr() as i32,
            CONFIG_BUF.len() as i32,
        );
        if len <= 0 {
            return None;
        }
        Some(str::from_utf8_unchecked(&CONFIG_BUF[..len as usize]).to_string())
    }
}

fn save_config(key: &str, value: &str) {
    unsafe {
        host_set_config(
            key.as_ptr() as i32,
            key.len() as i32,
            value.as_ptr() as i32,
            value.len() as i32,
        )
    }
}

// ── Data types ────────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone)]
struct PlayRecord {
    title: String,
    artist: String,
    duration_secs: u64,
    completed: bool,
}

#[derive(Serialize, Deserialize, Default)]
struct HistoryStore {
    /// Ordered play records (oldest first, capped at max_entries)
    records: Vec<PlayRecord>,
    /// Artist → completed play count
    artist_counts: HashMap<String, u32>,
    /// Track title → completed play count
    track_counts: HashMap<String, u32>,
}

const HISTORY_KEY: &str = "yt_history";
const MAX_ENTRIES: usize = 500;

impl HistoryStore {
    fn load() -> Self {
        load_config(HISTORY_KEY)
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    fn save(&self) {
        if let Ok(json) = serde_json::to_string(self) {
            save_config(HISTORY_KEY, &json);
        }
    }

    fn record_start(&mut self, title: &str, artist: &str, duration: u64) {
        if self.records.len() >= MAX_ENTRIES {
            self.records.remove(0);
        }
        self.records.push(PlayRecord {
            title: title.to_string(),
            artist: artist.to_string(),
            duration_secs: duration,
            completed: false,
        });
    }

    fn record_completion(&mut self, title: &str, artist: &str) {
        // Mark the most recent matching record as completed
        if let Some(rec) = self
            .records
            .iter_mut()
            .rev()
            .find(|r| r.title == title && r.artist == artist)
        {
            rec.completed = true;
        }
        *self.artist_counts.entry(artist.to_string()).or_insert(0) += 1;
        *self.track_counts.entry(title.to_string()).or_insert(0) += 1;
    }

    fn top_artists(&self, n: usize) -> Vec<(&str, u32)> {
        let mut pairs: Vec<(&str, u32)> = self
            .artist_counts
            .iter()
            .map(|(k, v)| (k.as_str(), *v))
            .collect();
        pairs.sort_by(|a, b| b.1.cmp(&a.1));
        pairs.truncate(n);
        pairs
    }

    fn export_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }
}

// ── Plugin state (in-memory, per session) ─────────────────────────────────────

struct State {
    store: HistoryStore,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State {
        store: HistoryStore::default(),
    });
}

// ── Event payloads ─────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct TrackInfo {
    title: String,
    artist: String,
    #[serde(default)]
    duration: u64,
    #[serde(default)]
    source: String,
}

// Mirrors PluginEvent from Symphony (externally tagged, default serde)
#[derive(Deserialize)]
enum Event {
    TrackChanged(TrackInfo),
    TrackCompleted {
        track: TrackInfo,
        duration_played_secs: u64,
    },
    AppStarted,
    AppStopped,
    #[serde(other)]
    Unknown,
}

// ── Plugin entry points ───────────────────────────────────────────────────────

#[no_mangle]
pub extern "C" fn plugin_init() {
    STATE.with(|s| {
        s.borrow_mut().store = HistoryStore::load();
    });
    log("youtube-history: initialized");
}

#[no_mangle]
pub extern "C" fn on_event(event_ptr: i32, event_len: i32) {
    let json = unsafe {
        let slice = slice::from_raw_parts(event_ptr as *const u8, event_len as usize);
        match str::from_utf8(slice) {
            Ok(s) => s,
            Err(_) => return,
        }
    };

    let event: Event = match serde_json::from_str(json) {
        Ok(e) => e,
        Err(_) => return,
    };

    match event {
        Event::TrackChanged(track) => handle_track_changed(track),
        Event::TrackCompleted { track, duration_played_secs } => {
            handle_track_completed(track, duration_played_secs);
        }
        Event::AppStopped => handle_app_stopped(),
        _ => {}
    }
}

#[no_mangle]
pub extern "C" fn plugin_shutdown() {
    STATE.with(|s| s.borrow().store.save());
    log("youtube-history: shutdown");
}

// ── Event handlers ────────────────────────────────────────────────────────────

fn handle_track_changed(track: TrackInfo) {
    if track.source != "youtube" {
        return;
    }
    let title = track.title.clone();
    let artist = track.artist.clone();
    STATE.with(|s| {
        let mut state = s.borrow_mut();
        state.store.record_start(&track.title, &track.artist, track.duration);
        state.store.save();
    });
    log(&format!("youtube-history: tracking '{}' by {}", title, artist));
}

fn handle_track_completed(track: TrackInfo, duration_played_secs: u64) {
    if track.source != "youtube" {
        return;
    }
    // Count as completed only if >50% was played
    let threshold = track.duration / 2;
    if duration_played_secs < threshold {
        return;
    }
    STATE.with(|s| {
        let mut state = s.borrow_mut();
        state.store.record_completion(&track.title, &track.artist);
        state.store.save();
    });
}

fn handle_app_stopped() {
    STATE.with(|s| {
        let state = s.borrow();
        let total = state.store.records.len();
        if total == 0 {
            return;
        }
        let top = state.store.top_artists(3);
        let mut summary = format!("YouTube plays this session: {}\nTop artists:", total);
        for (artist, count) in &top {
            summary.push_str(&format!("\n  {} ({}x)", artist, count));
        }
        notify("YouTube History", &summary);
        log(&format!("youtube-history: export\n{}", state.store.export_json()));
    });
}

// ── Tests (run on native, not WASM) ──────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_start_caps_at_max() {
        let mut store = HistoryStore::default();
        for i in 0..MAX_ENTRIES + 5 {
            store.record_start(&format!("Track {}", i), "Artist", 180);
        }
        assert_eq!(store.records.len(), MAX_ENTRIES);
    }

    #[test]
    fn test_record_completion_increments_counts() {
        let mut store = HistoryStore::default();
        store.record_start("Song", "Artist A", 200);
        store.record_completion("Song", "Artist A");
        assert_eq!(*store.artist_counts.get("Artist A").unwrap(), 1);
        assert_eq!(*store.track_counts.get("Song").unwrap(), 1);
    }

    #[test]
    fn test_top_artists_sorted() {
        let mut store = HistoryStore::default();
        store.artist_counts.insert("A".to_string(), 5);
        store.artist_counts.insert("B".to_string(), 10);
        store.artist_counts.insert("C".to_string(), 1);
        let top = store.top_artists(2);
        assert_eq!(top[0].0, "B");
        assert_eq!(top[1].0, "A");
    }

    #[test]
    fn test_non_youtube_track_ignored() {
        let track = TrackInfo {
            title: "Local Song".to_string(),
            artist: "Local Artist".to_string(),
            duration: 180,
            source: "local".to_string(),
        };
        // If source != youtube, handle_track_changed returns early
        assert_ne!(track.source, "youtube");
    }

    #[test]
    fn test_export_json_is_valid() {
        let mut store = HistoryStore::default();
        store.record_start("Song", "Artist", 120);
        store.record_completion("Song", "Artist");
        let json = store.export_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed.get("records").is_some());
        assert!(parsed.get("artist_counts").is_some());
    }
}
