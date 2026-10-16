//! Playback hooks for plugins, Discord Rich Presence and Last.fm scrobbling.
//!
//! Nothing here runs until `App::start_integrations` is called (the binary does it at startup),
//! so building an `App` has no side effects.

use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::task::JoinHandle;

use super::{App, RepeatMode, Track};
use crate::ai::playlists::{PlaylistCriteria, SmartPlaylistGenerator};
use crate::integrations::discord::{Activity, DiscordPresence};
use crate::integrations::git::ActivityLevel;
use crate::integrations::statusline::StatusSnapshot;
use crate::integrations::lastfm::{should_scrobble, LastFm, ScrobbleTrack};
use crate::plugins::{HostFunctions, PlaybackState as PluginPlayback, PluginEvent, PluginRuntime, TrackInfo};

/// How long quitting waits for the last scrobble to go out
const SHUTDOWN_GRACE: Duration = Duration::from_secs(3);

/// What plugins can read, and what they leave for the UI
#[derive(Default)]
struct Shared {
    track: Option<TrackInfo>,
    volume: f32,
    /// Plugin notifications waiting to be shown in the status bar
    notifications: Vec<String>,
}

#[derive(Default)]
pub(super) struct Integrations {
    plugins: Option<PluginRuntime>,
    discord: Option<Arc<Mutex<DiscordPresence>>>,
    lastfm: Option<LastFm>,
    shared: Arc<Mutex<Shared>>,
    /// Unix time the current track started
    started_unix: i64,
    /// The current track was already scrobbled/finished
    counted: bool,
    /// Scrobbles still being sent
    pending: Vec<JoinHandle<()>>,
    /// `start_integrations` ran: the status file may be written
    started: bool,
    last_status_write: Option<std::time::Instant>,
}

/// A mutex that stays usable after a panic elsewhere
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

fn track_info(track: &Track, position: f64) -> TrackInfo {
    TrackInfo {
        id: track.id.clone(),
        title: track.title.clone(),
        artist: track.artist.clone(),
        album: (!track.album.is_empty()).then(|| track.album.clone()),
        duration: track.duration as u64,
        position: position as u64,
        source: format!("{:?}", track.source).to_lowercase(),
        album_art_url: None,
    }
}

fn scrobble_track(track: &Track) -> ScrobbleTrack {
    ScrobbleTrack {
        artist: track.artist.clone(),
        title: track.title.clone(),
        album: track.album.clone(),
        duration_secs: track.duration as u64,
    }
}

impl App {
    /// Start the plugin runtime, Discord presence and Last.fm from the `[plugins]` config
    pub fn start_integrations(&mut self) {
        self.integrations.started = true;
        let config = self.config.plugins.clone();
        lock(&self.integrations.shared).volume = self.playback_state.volume;

        if config.discord.enabled {
            if config.discord.client_id.trim().is_empty() {
                self.ui_state.error_message =
                    Some("Discord: set client_id under [plugins.discord] (see docs)".to_string());
            } else {
                let presence = DiscordPresence::new(config.discord.client_id.trim());
                self.integrations.discord = Some(Arc::new(Mutex::new(presence)));
            }
        }

        if config.lastfm.enabled {
            let lastfm = &config.lastfm;
            if lastfm.api_key.is_empty() || lastfm.api_secret.is_empty() || lastfm.session_key.is_empty() {
                self.ui_state.error_message =
                    Some("Last.fm: run `symphony lastfm-login` first".to_string());
            } else {
                self.integrations.lastfm = Some(LastFm::new(
                    &lastfm.api_key,
                    &lastfm.api_secret,
                    &lastfm.session_key,
                    &lastfm.api_url,
                ));
            }
        }

        if config.enabled {
            match PluginRuntime::new(config.plugin_dir.clone()) {
                Ok(mut plugins) => {
                    plugins.set_host(self.host_functions());
                    plugins.set_auto_load(config.auto_load.clone());
                    if let Err(e) = futures::executor::block_on(plugins.load_all_plugins()) {
                        self.ui_state.error_message = Some(format!("Plugins: {e}"));
                    }
                    plugins.trigger_event(&PluginEvent::AppStarted);
                    self.integrations.plugins = Some(plugins);
                }
                Err(e) => self.ui_state.error_message = Some(format!("Plugins: {e}")),
            }
        }
    }

    /// Tell plugins we are quitting, clear Discord and wait briefly for the last scrobble
    pub fn stop_integrations(&mut self) {
        self.track_finished();
        self.emit(PluginEvent::AppStopped);
        self.integrations.plugins = None; // runs every plugin's shutdown hook

        // no status file means "not running" to `symphony status`
        if self.integrations.started {
            let _ = std::fs::remove_file(self.config.status_path());
            self.integrations.started = false;
        }

        if let Some(discord) = &self.integrations.discord {
            if let Err(e) = lock(discord).clear() {
                tracing::debug!("Discord: {e}");
            }
        }
        if let Some(runtime) = &self.runtime {
            let pending = std::mem::take(&mut self.integrations.pending);
            runtime.block_on(async {
                let _ = tokio::time::timeout(SHUTDOWN_GRACE, async {
                    for task in pending {
                        let _ = task.await;
                    }
                })
                .await;
            });
        }
    }

    /// What plugins may call back into
    fn host_functions(&self) -> HostFunctions {
        let (track, volume, notes) = (
            self.integrations.shared.clone(),
            self.integrations.shared.clone(),
            self.integrations.shared.clone(),
        );
        let (discord, lastfm, handle) = (
            self.integrations.discord.clone(),
            self.integrations.lastfm.clone(),
            self.runtime.as_ref().map(|r| r.handle().clone()),
        );
        let discord_handle = handle.clone();

        HostFunctions::new()
            .with_log(|message| tracing::info!("[plugin] {message}"))
            .with_get_track(move || lock(&track).track.clone())
            .with_get_volume(move || lock(&volume).volume)
            .with_notification(move |title, body| lock(&notes).notifications.push(format!("{title}: {body}")))
            .with_discord(move |title, artist, _, _| {
                if let (Some(discord), Some(handle)) = (discord.clone(), discord_handle.clone()) {
                    let activity = Activity {
                        details: title.to_string(),
                        state: artist.to_string(),
                        start_unix: Some(unix_now()),
                    };
                    handle.spawn_blocking(move || {
                        if let Err(e) = lock(&discord).set_activity(&activity) {
                            tracing::debug!("Discord: {e}");
                        }
                    });
                }
            })
            .with_scrobble(move |title, artist, album| {
                if let (Some(lastfm), Some(handle)) = (lastfm.clone(), handle.clone()) {
                    let track = ScrobbleTrack {
                        artist: artist.to_string(),
                        title: title.to_string(),
                        album: album.to_string(),
                        duration_secs: 0,
                    };
                    handle.spawn(async move {
                        if let Err(e) = lastfm.scrobble(&track, unix_now()).await {
                            tracing::debug!("Last.fm: {e}");
                        }
                    });
                }
            })
    }

    fn emit(&mut self, event: PluginEvent) {
        if let Some(plugins) = &mut self.integrations.plugins {
            plugins.trigger_event(&event);
        }
    }

    /// `track` just started playing
    pub(super) fn track_started(&mut self, track: &Track) {
        let started = unix_now();
        self.integrations.started_unix = started;
        self.integrations.counted = false;
        let info = track_info(track, 0.0);
        lock(&self.integrations.shared).track = Some(info.clone());
        self.emit(PluginEvent::TrackChanged(info));
        self.emit(PluginEvent::PlaybackStateChanged(PluginPlayback::Playing));

        self.discord_show(track, true, 0.0);
        if let (Some(lastfm), Some(runtime)) = (self.integrations.lastfm.clone(), &self.runtime) {
            let track = scrobble_track(track);
            runtime.spawn(async move {
                if let Err(e) = lastfm.now_playing(&track).await {
                    tracing::debug!("Last.fm: {e}");
                }
            });
        }
    }

    /// The current track ends or is replaced: count it as played if it was heard long enough
    pub(super) fn track_finished(&mut self) {
        if self.integrations.counted {
            return;
        }
        let Some(track) = self.playback_state.current_track.clone() else {
            return;
        };
        self.integrations.counted = true;
        let played = self.playback_state.position;
        self.emit(PluginEvent::TrackCompleted {
            track: track_info(&track, played),
            duration_played_secs: played as u64,
        });

        let threshold = self.config.plugins.lastfm.scrobble_threshold;
        if !should_scrobble(track.duration, played, threshold) {
            return;
        }
        if let (Some(lastfm), Some(runtime)) = (self.integrations.lastfm.clone(), &self.runtime) {
            let (scrobble, started) = (scrobble_track(&track), self.integrations.started_unix);
            let task = runtime.spawn(async move {
                if let Err(e) = lastfm.scrobble(&scrobble, started).await {
                    tracing::debug!("Last.fm: {e}");
                }
            });
            self.integrations.pending.retain(|t| !t.is_finished());
            self.integrations.pending.push(task);
        }
    }

    /// Playback was paused or resumed
    pub(super) fn playback_changed(&mut self) {
        let playing = self.playback_state.is_playing;
        self.emit(PluginEvent::PlaybackStateChanged(if playing {
            PluginPlayback::Playing
        } else {
            PluginPlayback::Paused
        }));
        if let Some(track) = self.playback_state.current_track.clone() {
            self.discord_show(&track, playing, self.playback_state.position);
        }
    }

    /// Playback stopped with nothing playing
    pub(super) fn playback_stopped(&mut self) {
        self.emit(PluginEvent::PlaybackStateChanged(PluginPlayback::Stopped));
        self.discord_send(None);
    }

    pub(super) fn volume_changed(&mut self) {
        let volume = self.playback_state.volume;
        lock(&self.integrations.shared).volume = volume;
        self.emit(PluginEvent::VolumeChanged(volume));
    }

    /// Show plugin notifications in the status bar
    pub(super) fn poll_plugin_notifications(&mut self) {
        let notes = std::mem::take(&mut lock(&self.integrations.shared).notifications);
        if let Some(latest) = notes.into_iter().last() {
            self.ui_state.status_message = Some(latest);
        }
    }

    fn discord_show(&self, track: &Track, playing: bool, position: f64) {
        let show_timer = self.config.plugins.discord.show_timestamp;
        let started = unix_now() - position as i64;
        self.discord_send(Some(Activity {
            details: track.title.clone(),
            state: if playing { format!("by {}", track.artist) } else { "Paused".to_string() },
            start_unix: (playing && show_timer).then_some(started),
        }));
    }

    /// Publish what is playing for `symphony status` (once a second, from `start_integrations` on)
    pub(super) fn publish_status(&mut self) {
        let integrations = &mut self.integrations;
        if !integrations.started
            || !self.config.integrations.statusline.enabled
            || integrations.last_status_write.is_some_and(|t| t.elapsed() < Duration::from_secs(1))
        {
            return;
        }
        integrations.last_status_write = Some(std::time::Instant::now());

        let (title, artist) = self
            .playback_state
            .current_track
            .as_ref()
            .map(|t| (t.title.clone(), t.artist.clone()))
            .unwrap_or_default();
        let snapshot = StatusSnapshot {
            title,
            artist,
            duration: self.playback_state.duration as u64,
            position: self.playback_state.position as u64,
            playing: self.playback_state.is_playing,
            volume: self.playback_state.volume,
            updated_unix: 0,
        };
        if let Err(e) = snapshot.stamped().write(&self.config.status_path()) {
            tracing::debug!("status file: {e}");
        }
    }

    /// At a track change: if the coding activity changed since the queue was built and
    /// `adapt_music` is on, replace the queue with music for the new level.
    /// Returns `true` when a new queue took over.
    pub(super) fn adapt_queue_for_git(&mut self) -> bool {
        if !self.config.integrations.git.adapt_music || self.playback_state.repeat == RepeatMode::One {
            return false;
        }
        let Some(git) = self.git_monitor.as_ref().filter(|g| g.in_repo()) else {
            return false;
        };
        let level = git.get_activity_level();
        match self.adapted_level.replace(level) {
            // the first track change: the queue the user picked stays
            None => false,
            Some(previous) if previous == level => false,
            Some(_) => self.adapt_to_activity(level),
        }
    }

    /// Replace the queue with music for `level` and play it; `false` if the library has nothing for it
    pub fn adapt_to_activity(&mut self, level: ActivityLevel) -> bool {
        let criteria = PlaylistCriteria {
            name: Some(format!("git: {}", level.label())),
            mood: Some(level.recommended_mood().to_string()),
            ..Default::default()
        };
        let generator = SmartPlaylistGenerator::new().with_features(self.features.clone());
        match generator.generate(&self.library.tracks, &criteria) {
            Ok(playlist) if !playlist.tracks.is_empty() => {
                let count = playlist.tracks.len();
                self.adapted_level = Some(level);
                self.start_queue(playlist.tracks, false);
                self.ui_state.status_message = Some(format!(
                    "Git: {} {} - {count} {} tracks queued",
                    level.icon(),
                    level.label(),
                    level.recommended_mood()
                ));
                true
            }
            _ => false,
        }
    }

    /// Update Discord off the UI thread (`None` clears the presence)
    fn discord_send(&self, activity: Option<Activity>) {
        let (Some(discord), Some(runtime)) = (self.integrations.discord.clone(), &self.runtime) else {
            return;
        };
        runtime.spawn_blocking(move || {
            let mut discord = lock(&discord);
            let result = match &activity {
                Some(activity) => discord.set_activity(activity),
                None => discord.clear(),
            };
            if let Err(e) = result {
                tracing::debug!("Discord: {e}");
            }
        });
    }
}
