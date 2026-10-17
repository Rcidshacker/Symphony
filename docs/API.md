# Plugin API reference

The contract between Symphony and a WebAssembly plugin (ABI v1). For a walkthrough see [Plugin Development](PLUGIN_DEVELOPMENT.md). The source of truth is `src/plugins/runtime.rs`; the behaviour below is covered by `tests/plugin_tests.rs`.

## Plugin folder

```text
<plugin_dir>/my-plugin/
  plugin.toml
  plugin.wasm
```

`plugin_dir` is `plugins/` next to the config file (see [Configuration](CONFIGURATION.md#plugins)). Each sub-folder with a `plugin.toml` is loaded at start-up, or only the folders named in `auto_load` when that list is not empty.

### `plugin.toml`

```toml
name = "my-plugin"          # required, unique
version = "1.0.0"           # required
author = "you"              # required
description = "What it does" # required
permissions = ["track_info", "notifications"]
homepage = "https://example.com"   # optional
enabled = true                     # optional, default true
```

## What a plugin exports

| Export | Required | Signature | |
|--------|----------|-----------|--|
| `memory` | yes | linear memory | |
| `alloc` | for events | `(len: i32) -> i32` | Returns a buffer of `len` bytes in `memory` for Symphony to write the event into |
| `on_event` | no | `(ptr: i32, len: i32)` | Receives one event as UTF-8 JSON. A plugin without it still loads but gets no events |
| `plugin_init` | no | `()` | Called once after loading |
| `plugin_shutdown` | no | `()` | Called when the player quits |

## Host functions

Imported from the module name `symphony`. Strings are `(ptr, len)` pairs in the plugin's memory, at most 64 KiB. Functions that return `i32` give `0` on success and `-1` when the plugin lacks the permission.

| Function | Permission | |
|----------|------------|--|
| `log(ptr, len)` | none | Writes a line to Symphony's log |
| `get_track(buf_ptr, buf_cap) -> i32` | `track_info` | Writes the current track as JSON into your buffer and returns the byte count. `0` means nothing is playing, `-1` no permission, `-2` the buffer is too small or invalid |
| `get_volume() -> f32` | `playback_state` | 0.0 to 1.0, or `-1.0` without permission |
| `notify(title_ptr, title_len, body_ptr, body_len) -> i32` | `notifications` | Shows a notification in the player |
| `discord_presence(title_ptr, title_len, artist_ptr, artist_len) -> i32` | `discord` | Sets Discord Rich Presence (needs `[plugins.discord]` set up) |
| `scrobble(title_ptr, title_len, artist_ptr, artist_len) -> i32` | `lastfm` | Scrobbles to Last.fm (needs `symphony lastfm-login` done) |

Other permission names are accepted in `plugin.toml` (`playback_control`, `library`, `library_write`, `playlists`, `playlists_write`, `network`, `config_read`, `config_write`, `custom`) but no host function uses them yet.

A call that is refused for lack of a permission is counted and logged; the plugin keeps running.

## Events

Each event is sent as JSON using serde's default enum encoding: a unit event is a bare string, the others are a one-key object.

| Event | Needs permission | When | JSON |
|-------|------------------|------|------|
| `AppStarted` | none | after the plugin loads | `"AppStarted"` |
| `AppStopped` | none | the player is quitting | `"AppStopped"` |
| `TrackChanged` | `track_info` | a track starts | `{"TrackChanged":{"title":"..","artist":"..","album":null,"duration":214,"position":0,"id":"..","source":"local","album_art_url":null}}` |
| `PlaybackStateChanged` | `playback_state` | play, pause, stop | `{"PlaybackStateChanged":"Playing"}` (also `"Paused"`, `"Stopped"`) |
| `VolumeChanged` | `playback_state` | volume changes | `{"VolumeChanged":0.6}` |
| `TrackCompleted` | `track_info` | a track finishes or is left | `{"TrackCompleted":{"track":{..as above..},"duration_played_secs":120}}` |

`PlaylistChanged`, `SeekChanged` and `Custom` exist in the type but the player does not send them yet.

## Limits

| | |
|--|--|
| CPU | 20,000,000 wasm instructions per call. A plugin that exceeds it (an endless loop) is stopped, marked failed, and gets no more events |
| Memory | 16 MiB of linear memory |
| Traps | A trap marks the plugin failed; other plugins are unaffected |
| Isolation | One wasmi instance per plugin; no file, network or system access except through the host functions above |

Built-in Discord Rich Presence and Last.fm scrobbling do not need a plugin; a plugin only needs `discord_presence` / `scrobble` if it wants to drive them with its own logic.
