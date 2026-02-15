# Symphony Plugin API Reference

**Complete API documentation for plugin developers**

---

## Table of Contents

1. [Overview](#overview)
2. [Plugin Architecture](#plugin-architecture)
3. [Plugin Manifest](#plugin-manifest)
4. [Host Functions](#host-functions)
5. [Events](#events)
6. [Types](#types)
7. [Permissions](#permissions)
8. [Examples](#examples)
9. [Best Practices](#best-practices)
10. [API Versioning](#api-versioning)

---

## Overview

Symphony's plugin system allows developers to extend the music player's functionality using WebAssembly (WASM). Plugins run in a secure sandbox and communicate with the host application through a well-defined API.

### Key Concepts

**WASM Sandbox**: Plugins are compiled to WebAssembly and run in a sandboxed environment. This ensures plugins cannot crash the main application or access memory they shouldn't.

**Host Functions**: The Symphony host exposes functions that plugins can call to interact with the player. These include getting track info, scrobbling, showing notifications, and more.

**Events**: The host sends events to plugins when things happen (track changes, playback state changes, etc.). Plugins can react to these events.

**Permissions**: Plugins must declare what they need to do in their manifest. This protects users from malicious plugins.

### Language Support

Any language that compiles to WASM can be used to write Symphony plugins:

| Language | Support Level | Notes |
|----------|--------------|-------|
| Rust | ✅ Primary | Best support, official examples |
| C/C++ | ✅ Full | Requires manual memory management |
| AssemblyScript | ✅ Full | TypeScript-like syntax |
| Go | ⚠️ Partial | Larger binary sizes |
| Zig | ✅ Full | Growing community support |

---

## Plugin Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         SYMPHONY HOST                                    │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │                    PLUGIN RUNTIME (Wasmtime)                     │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐             │   │
│  │  │   Plugin A  │  │   Plugin B  │  │   Plugin C  │             │   │
│  │  │   (WASM)    │  │   (WASM)    │  │   (WASM)    │             │   │
│  │  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘             │   │
│  │         │                │                │                     │   │
│  │         └────────────────┼────────────────┘                     │   │
│  │                          ▼                                       │   │
│  │              ┌───────────────────────┐                          │   │
│  │              │    Host Functions     │                          │   │
│  │              │  • host_log()         │                          │   │
│  │              │  • host_get_track()   │                          │   │
│  │              │  • host_scrobble()    │                          │   │
│  │              │  • host_notify()      │                          │   │
│  │              └───────────────────────┘                          │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                          │
│  Core Components: Audio Engine, Database, AI, Streaming, etc.          │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

### Plugin Lifecycle

1. **Load**: Plugin WASM file is read and compiled
2. **Validate**: Manifest is parsed, permissions are checked
3. **Initialize**: `plugin_init()` is called
4. **Run**: Plugin receives and processes events
5. **Shutdown**: `plugin_shutdown()` is called when Symphony exits

---

## Plugin Manifest

Every plugin must have a `plugin.toml` manifest file describing its capabilities.

### Required Fields

```toml
# plugin.toml
name = "my-awesome-plugin"
version = "1.0.0"
author = "Your Name <you@example.com>"
description = "Does awesome things with music"
```

### Full Manifest Schema

```toml
# Plugin identification
name = "my-awesome-plugin"          # Required: Unique identifier (alphanumeric, dashes)
version = "1.0.0"                   # Required: Semver version
author = "Your Name"                # Required: Author name/email
description = "Does awesome things" # Required: Brief description

# Optional metadata
homepage = "https://github.com/user/plugin"
repository = "https://github.com/user/plugin"
license = "MIT"
min_symphony_version = "2.0.0"

# Permissions required
permissions = [
    "track_info",
    "playback_state",
    "network",
    "notifications"
]

# Plugin configuration schema (optional)
[config]
enable_feature_x = { type = "bool", default = true, description = "Enable feature X" }
api_key = { type = "string", default = "", description = "API key for service" }

# Default state
enabled = true
```

### Permission Reference

| Permission | Description | Required For |
|------------|-------------|--------------|
| `track_info` | Read current track information | `host_get_track()` |
| `playback_state` | Read playback state | `host_get_playback_state()` |
| `playback_control` | Control playback | `host_play()`, `host_pause()` |
| `library` | Read library contents | `host_search_library()` |
| `library_write` | Modify library | `host_add_track()` |
| `playlists` | Read playlists | `host_get_playlist()` |
| `playlists_write` | Modify playlists | `host_create_playlist()` |
| `discord` | Set Discord presence | `host_set_discord_presence()` |
| `lastfm` | Scrobble to Last.fm | `host_scrobble()` |
| `network` | Make HTTP requests | `host_http_request()` |
| `config_read` | Read user config | `host_get_config()` |
| `config_write` | Modify user config | `host_set_config()` |
| `notifications` | Show notifications | `host_notify()` |

---

## Host Functions

Host functions are provided by Symphony for plugins to call. These are imported from the "env" module.

### Logging Functions

#### `host_log(message_ptr: i32, message_len: i32)`

Log a message to Symphony's log output.

**Parameters**:
- `message_ptr`: Pointer to message string in plugin memory
- `message_len`: Length of message string

**Example (Rust)**:
```rust
#[link(wasm_import_module = "env")]
extern "C" {
    fn host_log(message_ptr: i32, message_len: i32);
}

fn log_message(msg: &str) {
    unsafe {
        let ptr = msg.as_ptr() as i32;
        host_log(ptr, msg.len() as i32);
    }
}
```

**No permissions required**

---

### Track Information Functions

#### `host_get_track(buffer_ptr: i32, buffer_len: i32) -> i32`

Get information about the currently playing track.

**Parameters**:
- `buffer_ptr`: Pointer to buffer where JSON will be written
- `buffer_len`: Size of buffer

**Returns**: Number of bytes written, or -1 if no track

**Output Format** (JSON):
```json
{
    "id": "abc123",
    "title": "Paranoid Android",
    "artist": "Radiohead",
    "album": "OK Computer",
    "duration": 383,
    "position": 201,
    "source": "local",
    "album_art_url": null
}
```

**Permission required**: `track_info`

---

#### `host_get_playback_state() -> i32`

Get current playback state.

**Returns**:
- `0`: Stopped
- `1`: Playing
- `2`: Paused

**Permission required**: `playback_state`

---

### Playback Control Functions

#### `host_play()`

Start or resume playback.

**Permission required**: `playback_control`

---

#### `host_pause()`

Pause playback.

**Permission required**: `playback_control`

---

#### `host_next_track()`

Skip to next track.

**Permission required**: `playback_control`

---

#### `host_previous_track()`

Go to previous track.

**Permission required**: `playback_control`

---

#### `host_seek(position_ms: i64)`

Seek to position in current track.

**Parameters**:
- `position_ms`: Target position in milliseconds

**Permission required**: `playback_control`

---

#### `host_set_volume(volume: f32)`

Set playback volume.

**Parameters**:
- `volume`: Volume level (0.0 to 1.0)

**Permission required**: `playback_control`

---

### Library Functions

#### `host_search_library(query_ptr: i32, query_len: i32, buffer_ptr: i32, buffer_len: i32) -> i32`

Search the music library.

**Parameters**:
- `query_ptr`: Pointer to search query
- `query_len`: Length of query
- `buffer_ptr`: Buffer for results
- `buffer_len`: Buffer size

**Returns**: Number of bytes written

**Output Format** (JSON array):
```json
[
    {
        "id": "track-1",
        "title": "Song Title",
        "artist": "Artist Name",
        "album": "Album Name",
        "duration": 240
    }
]
```

**Permission required**: `library`

---

### Playlist Functions

#### `host_get_playlist(playlist_id_ptr: i32, playlist_id_len: i32, buffer_ptr: i32, buffer_len: i32) -> i32`

Get playlist contents.

**Permission required**: `playlists`

---

#### `host_create_playlist(name_ptr: i32, name_len: i32) -> i32`

Create a new playlist.

**Returns**: Playlist ID or -1 on error

**Permission required**: `playlists_write`

---

#### `host_add_to_playlist(playlist_id: i32, track_id_ptr: i32, track_id_len: i32)`

Add a track to a playlist.

**Permission required**: `playlists_write`

---

### Discord Integration

#### `host_set_discord_presence(state_ptr: i32, state_len: i32, details_ptr: i32, details_len: i32, large_image_ptr: i32, large_image_len: i32)`

Set Discord Rich Presence.

**Parameters**:
- `state`: Short state text (e.g., "Listening")
- `details`: Longer details text (e.g., "Radiohead - Paranoid Android")
- `large_image`: Image key or URL

**Permission required**: `discord`

---

### Last.fm Integration

#### `host_scrobble(artist_ptr: i32, artist_len: i32, track_ptr: i32, track_len: i32, album_ptr: i32, album_len: i32, duration_secs: i32)`

Scrobble a track to Last.fm.

**Permission required**: `lastfm`

---

### Network Functions

#### `host_http_request(method_ptr: i32, method_len: i32, url_ptr: i32, url_len: i32, headers_ptr: i32, headers_len: i32, body_ptr: i32, body_len: i32, response_ptr: i32, response_len: i32) -> i32`

Make an HTTP request.

**Parameters**:
- `method`: HTTP method (GET, POST, etc.)
- `url`: Request URL
- `headers`: JSON object of headers
- `body`: Request body (can be empty)
- `response_ptr`: Buffer for response

**Permission required**: `network`

---

### Notification Functions

#### `host_notify(title_ptr: i32, title_len: i32, body_ptr: i32, body_len: i32)`

Show a desktop notification.

**Permission required**: `notifications`

---

### Configuration Functions

#### `host_get_config(key_ptr: i32, key_len: i32, buffer_ptr: i32, buffer_len: i32) -> i32`

Get a configuration value.

**Permission required**: `config_read`

---

#### `host_set_config(key_ptr: i32, key_len: i32, value_ptr: i32, value_len: i32)`

Set a configuration value.

**Permission required**: `config_write`

---

## Events

Plugins receive events from Symphony through exported functions.

### Event Types

```rust
pub enum PluginEvent {
    /// A new track started playing
    TrackChanged(TrackInfo),
    /// Playback state changed
    PlaybackStateChanged(PlaybackState),
    /// Volume was changed
    VolumeChanged(f32),
    /// Playlist was modified
    PlaylistChanged {
        playlist_id: String,
        action: PlaylistAction,
    },
    /// Application started
    AppStarted,
    /// Application is shutting down
    AppStopped,
    /// A track completed
    TrackCompleted {
        track: TrackInfo,
        duration_played_secs: u64,
    },
    /// Seek position changed
    SeekChanged {
        position_secs: u64,
    },
    /// Custom event
    Custom {
        event_type: String,
        data: HashMap<String, String>,
    },
}
```

### Handling Events

Plugins must export an `on_event` function:

```rust
#[no_mangle]
pub extern "C" fn on_event(event_type_ptr: i32, event_type_len: i32, event_data_ptr: i32, event_data_len: i32) {
    let event_type = unsafe {
        let slice = std::slice::from_raw_parts(event_type_ptr as *const u8, event_type_len as usize);
        std::str::from_utf8_unchecked(slice)
    };
    
    let event_data = unsafe {
        let slice = std::slice::from_raw_parts(event_data_ptr as *const u8, event_data_len as usize);
        std::str::from_utf8_unchecked(slice)
    };
    
    match event_type {
        "track_changed" => handle_track_changed(event_data),
        "playback_state_changed" => handle_playback_changed(event_data),
        _ => {}
    }
}
```

### Event Data Formats

Each event type has a corresponding JSON format:

**TrackChanged**:
```json
{
    "id": "abc123",
    "title": "Paranoid Android",
    "artist": "Radiohead",
    "album": "OK Computer",
    "duration": 383,
    "position": 0,
    "source": "local"
}
```

**PlaybackStateChanged**:
```json
{
    "state": "playing"
}
```

**VolumeChanged**:
```json
{
    "volume": 0.75
}
```

---

## Types

### TrackInfo

```rust
pub struct TrackInfo {
    /// Unique track identifier
    pub id: String,
    /// Track title
    pub title: String,
    /// Artist name
    pub artist: String,
    /// Album name (if available)
    pub album: Option<String>,
    /// Duration in seconds
    pub duration: u64,
    /// Current playback position in seconds
    pub position: u64,
    /// Source type: "local", "youtube", "spotify"
    pub source: String,
    /// Album art URL (if available)
    pub album_art_url: Option<String>,
}
```

### PlaybackState

```rust
pub enum PlaybackState {
    Playing = 0,
    Paused = 1,
    Stopped = 2,
}
```

### PlaylistAction

```rust
pub enum PlaylistAction {
    Added,
    Removed,
    Created,
    Deleted,
    Reordered,
}
```

---

## Permissions

Permissions are declared in the manifest and control what host functions a plugin can call.

### Permission Model

```
┌─────────────────────────────────────────────────────────────────────────┐
│                       PERMISSION CHECK FLOW                              │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  Plugin calls host_function()                                            │
│           │                                                              │
│           ▼                                                              │
│  ┌─────────────────────┐                                                 │
│  │ Check Permission    │                                                 │
│  │ in Manifest         │                                                 │
│  └──────────┬──────────┘                                                 │
│             │                                                            │
│      ┌──────┴──────┐                                                     │
│      ▼             ▼                                                     │
│  [Permission]   [No Permission]                                          │
│  Granted        Denied                                                   │
│      │             │                                                     │
│      ▼             ▼                                                     │
│  Execute       Return Error                                              │
│  Function      (PermissionDenied)                                        │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

### Permission Groups

For convenience, related permissions can be grouped:

```toml
# Read-only playback access
permissions = ["track_info", "playback_state"]

# Full playback control
permissions = ["track_info", "playback_state", "playback_control"]

# Library management
permissions = ["library", "library_write", "playlists", "playlists_write"]

# External integrations
permissions = ["discord", "lastfm", "network"]

# Full access (use sparingly)
permissions = ["track_info", "playback_state", "playback_control", 
               "library", "library_write", "playlists", "playlists_write",
               "discord", "lastfm", "network", "config_read", "notifications"]
```

---

## Examples

### Minimal Plugin (Rust)

```rust
// src/lib.rs
use std::slice;
use std::str;

// Import host functions
#[link(wasm_import_module = "env")]
extern "C" {
    fn host_log(message_ptr: i32, message_len: i32);
}

// Log helper
fn log(msg: &str) {
    unsafe {
        host_log(msg.as_ptr() as i32, msg.len() as i32);
    }
}

// Plugin initialization
#[no_mangle]
pub extern "C" fn plugin_init() {
    log("Hello from my plugin!");
}

// Handle events
#[no_mangle]
pub extern "C" fn on_event(event_type_ptr: i32, event_type_len: i32, 
                          event_data_ptr: i32, event_data_len: i32) {
    let event_type = unsafe {
        str::from_utf8_unchecked(slice::from_raw_parts(
            event_type_ptr as *const u8, 
            event_type_len as usize
        ))
    };
    
    log(&format!("Received event: {}", event_type));
}

// Plugin shutdown
#[no_mangle]
pub extern "C" fn plugin_shutdown() {
    log("Plugin shutting down");
}
```

### Discord Rich Presence Plugin

```rust
use std::slice;
use std::str;
use serde::Deserialize;

#[link(wasm_import_module = "env")]
extern "C" {
    fn host_log(message_ptr: i32, message_len: i32);
    fn host_get_track(buffer_ptr: i32, buffer_len: i32) -> i32;
    fn host_set_discord_presence(
        state_ptr: i32, state_len: i32,
        details_ptr: i32, details_len: i32,
        large_image_ptr: i32, large_image_len: i32
    );
}

#[derive(Deserialize)]
struct TrackInfo {
    title: String,
    artist: String,
    album: Option<String>,
    duration: u64,
}

static mut BUFFER: [u8; 4096] = [0; 4096];

fn log(msg: &str) {
    unsafe { host_log(msg.as_ptr() as i32, msg.len() as i32); }
}

fn get_current_track() -> Option<TrackInfo> {
    unsafe {
        let len = host_get_track(BUFFER.as_mut_ptr() as i32, BUFFER.len() as i32);
        if len <= 0 {
            return None;
        }
        let json = str::from_utf8_unchecked(&BUFFER[..len as usize]);
        serde_json::from_str(json).ok()
    }
}

fn set_presence(state: &str, details: &str) {
    unsafe {
        host_set_discord_presence(
            state.as_ptr() as i32, state.len() as i32,
            details.as_ptr() as i32, details.len() as i32,
            "symphony".as_ptr() as i32, 8
        );
    }
}

#[no_mangle]
pub extern "C" fn plugin_init() {
    log("Discord plugin initialized");
}

#[no_mangle]
pub extern "C" fn on_event(event_type_ptr: i32, event_type_len: i32,
                          _data_ptr: i32, _data_len: i32) {
    let event_type = unsafe {
        str::from_utf8_unchecked(slice::from_raw_parts(
            event_type_ptr as *const u8,
            event_type_len as usize
        ))
    };
    
    if event_type == "track_changed" {
        if let Some(track) = get_current_track() {
            let details = format!("{} - {}", track.artist, track.title);
            set_presence("Listening to Symphony", &details);
        }
    } else if event_type == "playback_state_changed" {
        // Handle pause/play for presence
    }
}

#[no_mangle]
pub extern "C" fn plugin_shutdown() {
    // Clear presence on shutdown
    set_presence("", "");
    log("Discord plugin shutdown");
}
```

### Last.fm Scrobbler Plugin

```rust
use std::slice;
use std::str;
use serde::Deserialize;

#[link(wasm_import_module = "env")]
extern "C" {
    fn host_log(message_ptr: i32, message_len: i32);
    fn host_scrobble(
        artist_ptr: i32, artist_len: i32,
        track_ptr: i32, track_len: i32,
        album_ptr: i32, album_len: i32,
        duration_secs: i32
    );
}

#[derive(Deserialize)]
struct TrackCompletedData {
    track: TrackInfo,
    duration_played_secs: u64,
}

#[derive(Deserialize)]
struct TrackInfo {
    title: String,
    artist: String,
    album: Option<String>,
    duration: u64,
}

static mut BUFFER: [u8; 8192] = [0; 8192];

fn log(msg: &str) {
    unsafe { host_log(msg.as_ptr() as i32, msg.len() as i32); }
}

fn scrobble(track: &TrackInfo) {
    unsafe {
        let album_ptr = track.album.as_ref().map(|a| a.as_ptr() as i32).unwrap_or(0);
        let album_len = track.album.as_ref().map(|a| a.len() as i32).unwrap_or(0);
        
        host_scrobble(
            track.artist.as_ptr() as i32, track.artist.len() as i32,
            track.title.as_ptr() as i32, track.title.len() as i32,
            album_ptr, album_len,
            track.duration as i32
        );
    }
}

#[no_mangle]
pub extern "C" fn plugin_init() {
    log("Last.fm scrobbler initialized");
}

#[no_mangle]
pub extern "C" fn on_event(_type_ptr: i32, _type_len: i32,
                          data_ptr: i32, data_len: i32) {
    let event_data = unsafe {
        str::from_utf8_unchecked(slice::from_raw_parts(
            data_ptr as *const u8,
            data_len as usize
        ))
    };
    
    // Parse track_completed event
    if let Ok(completed) = serde_json::from_str::<TrackCompletedData>(event_data) {
        // Only scrobble if more than 50% played
        let threshold = completed.track.duration / 2;
        if completed.duration_played_secs >= threshold {
            scrobble(&completed.track);
            log(&format!("Scrobbled: {} - {}", 
                completed.track.artist, completed.track.title));
        }
    }
}

#[no_mangle]
pub extern "C" fn plugin_shutdown() {
    log("Last.fm scrobbler shutdown");
}
```

---

## Best Practices

### Memory Management

1. **Use Static Buffers**: Allocate static buffers for data exchange
2. **Check Buffer Sizes**: Always validate buffer sizes before writing
3. **Avoid Allocations**: Minimize dynamic allocations in WASM

```rust
// Good: Static buffer
static mut BUFFER: [u8; 4096] = [0; 4096];

// Avoid: Dynamic allocation in hot paths
let mut buffer = Vec::new(); // Don't do this frequently
```

### Error Handling

1. **Check Return Values**: Host functions return -1 or 0 to indicate errors
2. **Log Errors**: Use `host_log` for debugging
3. **Fail Gracefully**: Don't crash on errors

```rust
let result = unsafe { host_get_track(buffer_ptr, buffer_len) };
if result < 0 {
    log("No track playing");
    return;
}
```

### Performance

1. **Minimize Work in on_event**: Keep event handlers fast
2. **Use Background Processing**: For heavy work, use async patterns
3. **Batch API Calls**: Combine multiple operations

### Security

1. **Request Minimal Permissions**: Only ask for what you need
2. **Validate Input**: Check event data before processing
3. **Don't Store Sensitive Data**: Let Symphony manage credentials

---

## API Versioning

The Plugin API follows semantic versioning.

### Current Version: 2.0.0

### Compatibility Promise

- **Major version**: Breaking changes to API
- **Minor version**: New functions, backward compatible
- **Patch version**: Bug fixes

### Version Checking

Plugins can check API version:

```rust
#[link(wasm_import_module = "env")]
extern "C" {
    fn host_get_api_version() -> i32;
}

fn check_version() -> bool {
    let version = unsafe { host_get_api_version() };
    // Version encoded as: major * 10000 + minor * 100 + patch
    // 2.0.0 = 20000
    version >= 20000
}
```

### Deprecation Policy

- Deprecated functions are marked in documentation
- Deprecated functions work for at least one major version
- Removal is announced in release notes

---

## Appendix

### Complete Host Function Reference

| Function | Parameters | Returns | Permission |
|----------|-----------|---------|------------|
| `host_log` | ptr, len | void | none |
| `host_get_api_version` | - | i32 | none |
| `host_get_track` | buffer_ptr, buffer_len | i32 | track_info |
| `host_get_playback_state` | - | i32 | playback_state |
| `host_play` | - | void | playback_control |
| `host_pause` | - | void | playback_control |
| `host_next_track` | - | void | playback_control |
| `host_previous_track` | - | void | playback_control |
| `host_seek` | position_ms | void | playback_control |
| `host_set_volume` | volume | void | playback_control |
| `host_search_library` | query_ptr, query_len, buffer_ptr, buffer_len | i32 | library |
| `host_get_playlist` | id_ptr, id_len, buffer_ptr, buffer_len | i32 | playlists |
| `host_create_playlist` | name_ptr, name_len | i32 | playlists_write |
| `host_add_to_playlist` | playlist_id, track_ptr, track_len | void | playlists_write |
| `host_set_discord_presence` | state, details, image | void | discord |
| `host_scrobble` | artist, track, album, duration | void | lastfm |
| `host_http_request` | method, url, headers, body, response | i32 | network |
| `host_notify` | title_ptr, title_len, body_ptr, body_len | void | notifications |
| `host_get_config` | key_ptr, key_len, buffer_ptr, buffer_len | i32 | config_read |
| `host_set_config` | key_ptr, key_len, value_ptr, value_len | void | config_write |

---

*Symphony Plugin API v2.0.0 - Build something amazing*
