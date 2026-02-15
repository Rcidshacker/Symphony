# Symphony Plugin Development Guide

**A comprehensive guide to creating plugins for Symphony v2.0**

---

## Table of Contents

1. [Introduction](#introduction)
2. [Getting Started](#getting-started)
3. [Your First Plugin](#your-first-plugin)
4. [Plugin Structure](#plugin-structure)
5. [Working with Events](#working-with-events)
6. [Calling Host Functions](#calling-host-functions)
7. [Building & Testing](#building--testing)
8. [Distribution](#distribution)
9. [Advanced Topics](#advanced-topics)
10. [Troubleshooting](#troubleshooting)

---

## Introduction

Symphony's plugin system allows you to extend the music player with custom functionality. Whether you want to integrate with a new service, create a custom visualizer, or add quality-of-life features, plugins give you the power to make Symphony your own.

### Why WASM?

We chose WebAssembly (WASM) for the plugin system for several important reasons:

**Security**: Plugins run in a sandboxed environment. They can't access your filesystem, network, or system resources without explicit permission. This protects you from malicious or buggy plugins.

**Performance**: WASM runs at near-native speed. Your plugin code executes efficiently with minimal overhead.

**Language Choice**: You can write plugins in any language that compiles to WASM—Rust, C/C++, AssemblyScript, Go, and more.

**Portability**: WASM plugins work on any platform Symphony supports without recompilation.

### What Can Plugins Do?

- **React to events**: Know when tracks change, playback starts/stops, etc.
- **Access track information**: Get details about what's playing
- **Control playback**: Play, pause, skip, seek, adjust volume
- **Integrate external services**: Discord, Last.fm, Slack, custom APIs
- **Show notifications**: Desktop notifications for track changes
- **Manage playlists**: Create, modify, and organize playlists
- **Search the library**: Find tracks by various criteria

### Prerequisites

Before developing plugins, ensure you have:

1. **Rust 1.75+** installed (recommended for plugin development)
2. **wasm32-unknown-unknown target**:
   ```bash
   rustup target add wasm32-unknown-unknown
   ```
3. **Basic Rust knowledge** (this guide uses Rust for examples)
4. **Symphony v2.0+** installed for testing

---

## Getting Started

### Development Environment Setup

1. **Create a workspace directory**:
   ```bash
   mkdir -p ~/symphony-plugins
   cd ~/symphony-plugins
   ```

2. **Install additional tools** (optional but recommended):
   ```bash
   # WASM validation
   cargo install wasm-validate

   # WASM optimization (smaller binaries)
   cargo install wasm-opt

   # For AssemblyScript development
   npm install -g assemblyscript
   ```

3. **Verify your setup**:
   ```bash
   rustc --version
   rustup target list --installed | grep wasm32
   ```

### Project Templates

For quick starts, we provide templates:

```bash
# Basic plugin template
cargo generate --git https://github.com/symphony-player/plugin-template

# Discord plugin template
cargo generate --git https://github.com/symphony-player/plugin-template-discord

# Last.fm plugin template
cargo generate --git https://github.com/symphony-player/plugin-template-lastfm
```

---

## Your First Plugin

Let's create a simple "Hello World" plugin that logs messages when tracks change.

### Step 1: Create the Project

```bash
cargo new --lib hello-plugin
cd hello-plugin
```

### Step 2: Configure Cargo.toml

Edit `Cargo.toml`:

```toml
[package]
name = "hello-plugin"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]  # Required for WASM output

[dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

[profile.release]
opt-level = "z"      # Optimize for size
lto = true           # Link-time optimization
```

### Step 3: Create the Plugin Manifest

Create `plugin.toml`:

```toml
name = "hello-plugin"
version = "0.1.0"
author = "Your Name <you@example.com>"
description = "A simple hello world plugin"
permissions = ["track_info"]
```

### Step 4: Write the Plugin Code

Edit `src/lib.rs`:

```rust
use std::slice;
use std::str;

// Import host functions from Symphony
#[link(wasm_import_module = "env")]
extern "C" {
    /// Log a message to Symphony's log output
    fn host_log(message_ptr: i32, message_len: i32);
    
    /// Get current track information
    fn host_get_track(buffer_ptr: i32, buffer_len: i32) -> i32;
}

// Static buffer for data exchange
static mut BUFFER: [u8; 4096] = [0; 4096];

/// Helper to log messages
fn log(msg: &str) {
    unsafe {
        host_log(msg.as_ptr() as i32, msg.len() as i32);
    }
}

/// Plugin initialization - called when plugin is loaded
#[no_mangle]
pub extern "C" fn plugin_init() {
    log("👋 Hello Plugin initialized!");
}

/// Handle events from Symphony
#[no_mangle]
pub extern "C" fn on_event(
    event_type_ptr: i32, 
    event_type_len: i32,
    _event_data_ptr: i32, 
    _event_data_len: i32
) {
    // Get the event type as a string
    let event_type = unsafe {
        let slice = slice::from_raw_parts(
            event_type_ptr as *const u8, 
            event_type_len as usize
        );
        str::from_utf8_unchecked(slice)
    };
    
    // Log what event we received
    log(&format!("Received event: {}", event_type));
    
    // Handle specific events
    if event_type == "track_changed" {
        handle_track_changed();
    }
}

/// Handle track change events
fn handle_track_changed() {
    // Get track info from host
    let track_json = unsafe {
        let len = host_get_track(BUFFER.as_mut_ptr() as i32, BUFFER.len() as i32);
        if len <= 0 {
            log("No track playing");
            return;
        }
        str::from_utf8_unchecked(&BUFFER[..len as usize])
    };
    
    // Parse the JSON
    if let Ok(track) = serde_json::from_str::<TrackInfo>(track_json) {
        log(&format!("🎵 Now playing: {} - {}", track.artist, track.title));
    }
}

/// Track information structure
#[derive(serde::Deserialize)]
struct TrackInfo {
    title: String,
    artist: String,
    album: Option<String>,
}

/// Plugin shutdown - called when Symphony exits
#[no_mangle]
pub extern "C" fn plugin_shutdown() {
    log("👋 Hello Plugin shutting down!");
}
```

### Step 5: Build the Plugin

```bash
# Build for WASM target
cargo build --target wasm32-unknown-unknown --release

# The output is at:
# target/wasm32-unknown-unknown/release/hello_plugin.wasm
```

### Step 6: Install the Plugin

```bash
# Create plugin directory
mkdir -p ~/.config/symphony/plugins/hello-plugin

# Copy WASM file and manifest
cp target/wasm32-unknown-unknown/release/hello_plugin.wasm \
   ~/.config/symphony/plugins/hello-plugin/plugin.wasm
cp plugin.toml ~/.config/symphony/plugins/hello-plugin/
```

### Step 7: Test the Plugin

```bash
# Start Symphony with verbose logging
symphony --verbose

# In another terminal, watch the logs
tail -f ~/.local/share/symphony/logs/symphony.log | grep plugin
```

You should see:
```
[INFO] Loading plugin: hello-plugin
[INFO] 👋 Hello Plugin initialized!
[INFO] Received event: track_changed
[INFO] 🎵 Now playing: Radiohead - Paranoid Android
```

---

## Plugin Structure

A complete plugin consists of several components:

### Directory Layout

```
my-plugin/
├── Cargo.toml          # Rust package configuration
├── plugin.toml         # Symphony plugin manifest
├── src/
│   └── lib.rs          # Plugin code
├── tests/              # Unit tests (optional)
│   └── test.rs
└── README.md           # Plugin documentation
```

### Required Files

**plugin.toml** (Manifest):
```toml
# Required fields
name = "my-plugin"           # Unique identifier
version = "1.0.0"            # Semantic version
author = "Your Name"         # Your name/email
description = "What it does" # Brief description

# Permissions (what the plugin can do)
permissions = ["track_info", "playback_state"]

# Optional metadata
homepage = "https://github.com/user/plugin"
license = "MIT"
min_symphony_version = "2.0.0"
```

**plugin.wasm** (Binary):
The compiled WebAssembly module. Must export:
- `plugin_init()` - Called on load
- `on_event(type_ptr, type_len, data_ptr, data_len)` - Event handler
- `plugin_shutdown()` - Called on unload (optional)

### Exported Functions

```rust
/// Initialize the plugin (required)
/// Called once when the plugin is loaded
#[no_mangle]
pub extern "C" fn plugin_init() {
    // Initialize your plugin
}

/// Handle events from Symphony (required)
/// Called for each event that occurs
#[no_mangle]
pub extern "C" fn on_event(
    event_type_ptr: i32,
    event_type_len: i32,
    event_data_ptr: i32,
    event_data_len: i32
) {
    // Process the event
}

/// Clean up resources (optional)
/// Called when Symphony is shutting down
#[no_mangle]
pub extern "C" fn plugin_shutdown() {
    // Clean up
}
```

---

## Working with Events

Events are the primary way plugins react to what's happening in Symphony.

### Event Types

| Event | When Fired | Data |
|-------|-----------|------|
| `app_started` | Symphony launches | `{}` |
| `app_stopped` | Symphony exits | `{}` |
| `track_changed` | New track starts playing | `TrackInfo` JSON |
| `track_completed` | Track finishes | `{track, duration_played_secs}` |
| `playback_state_changed` | Play/pause/stop | `{state: "playing"|"paused"|"stopped"}` |
| `volume_changed` | Volume adjusted | `{volume: 0.75}` |
| `seek_changed` | Seek position changes | `{position_secs: 120}` |
| `playlist_changed` | Playlist modified | `{playlist_id, action}` |

### Parsing Event Data

```rust
use serde::Deserialize;

#[derive(Deserialize)]
struct TrackInfo {
    id: String,
    title: String,
    artist: String,
    album: Option<String>,
    duration: u64,
    position: u64,
}

fn parse_track_event(data: &str) -> Option<TrackInfo> {
    serde_json::from_str(data).ok()
}

#[no_mangle]
pub extern "C" fn on_event(
    event_type_ptr: i32,
    event_type_len: i32,
    event_data_ptr: i32,
    event_data_len: i32
) {
    let (event_type, event_data) = unsafe {
        let type_slice = slice::from_raw_parts(
            event_type_ptr as *const u8, 
            event_type_len as usize
        );
        let data_slice = slice::from_raw_parts(
            event_data_ptr as *const u8, 
            event_data_len as usize
        );
        (
            str::from_utf8_unchecked(type_slice),
            str::from_utf8_unchecked(data_slice)
        )
    };
    
    match event_type {
        "track_changed" => {
            if let Some(track) = parse_track_event(event_data) {
                handle_track(&track);
            }
        }
        "playback_state_changed" => {
            handle_playback_change(event_data);
        }
        _ => {}
    }
}
```

### Event-Driven Example: Auto-Scrobbler

```rust
use serde::Deserialize;

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

#[no_mangle]
pub extern "C" fn on_event(
    event_type_ptr: i32,
    event_type_len: i32,
    event_data_ptr: i32,
    event_data_len: i32
) {
    let event_type = unsafe {
        str::from_utf8_unchecked(slice::from_raw_parts(
            event_type_ptr as *const u8,
            event_type_len as usize
        ))
    };
    
    if event_type == "track_completed" {
        let data = unsafe {
            str::from_utf8_unchecked(slice::from_raw_parts(
                event_data_ptr as *const u8,
                event_data_len as usize
            ))
        };
        
        if let Ok(completed) = serde_json::from_str::<TrackCompletedData>(data) {
            // Only scrobble if >50% played
            let threshold = completed.track.duration / 2;
            if completed.duration_played_secs >= threshold {
                scrobble(&completed.track);
            }
        }
    }
}
```

---

## Calling Host Functions

Host functions let your plugin interact with Symphony.

### Importing Host Functions

```rust
#[link(wasm_import_module = "env")]
extern "C" {
    fn host_log(message_ptr: i32, message_len: i32);
    fn host_get_track(buffer_ptr: i32, buffer_len: i32) -> i32;
    fn host_get_playback_state() -> i32;
    fn host_play();
    fn host_pause();
    fn host_set_volume(volume: f32);
    fn host_notify(title_ptr: i32, title_len: i32, body_ptr: i32, body_len: i32);
    fn host_set_discord_presence(
        state_ptr: i32, state_len: i32,
        details_ptr: i32, details_len: i32,
        image_ptr: i32, image_len: i32
    );
}
```

### Helper Wrappers

Create safe wrappers for common operations:

```rust
// Logging helper
fn log(msg: &str) {
    unsafe {
        host_log(msg.as_ptr() as i32, msg.len() as i32);
    }
}

// Notification helper
fn notify(title: &str, body: &str) {
    unsafe {
        host_notify(
            title.as_ptr() as i32, title.len() as i32,
            body.as_ptr() as i32, body.len() as i32
        );
    }
}

// Track info helper
static mut TRACK_BUFFER: [u8; 4096] = [0; 4096];

fn get_current_track() -> Option<TrackInfo> {
    unsafe {
        let len = host_get_track(
            TRACK_BUFFER.as_mut_ptr() as i32, 
            TRACK_BUFFER.len() as i32
        );
        if len <= 0 {
            return None;
        }
        let json = str::from_utf8_unchecked(&TRACK_BUFFER[..len as usize]);
        serde_json::from_str(json).ok()
    }
}

// Playback control helpers
fn play() {
    unsafe { host_play(); }
}

fn pause() {
    unsafe { host_pause(); }
}

fn set_volume(vol: f32) {
    unsafe { host_set_volume(vol.clamp(0.0, 1.0)); }
}
```

### String Handling

WASM uses pointers and lengths for strings:

```rust
// Pass a string to a host function
fn send_string(s: &str) {
    unsafe {
        some_host_function(s.as_ptr() as i32, s.len() as i32);
    }
}

// Receive a string from host into buffer
fn receive_string(buffer: &mut [u8]) -> Option<&str> {
    unsafe {
        let len = host_get_string(buffer.as_mut_ptr() as i32, buffer.len() as i32);
        if len <= 0 {
            return None;
        }
        Some(str::from_utf8_unchecked(&buffer[..len as usize]))
    }
}
```

---

## Building & Testing

### Build Commands

```bash
# Development build (faster compile, larger binary)
cargo build --target wasm32-unknown-unknown

# Release build (optimized)
cargo build --target wasm32-unknown-unknown --release

# Check without building
cargo check --target wasm32-unknown-unknown
```

### Optimizing Binary Size

Add to `Cargo.toml`:

```toml
[profile.release]
opt-level = "z"      # Optimize for size
lto = true           # Link-time optimization
codegen-units = 1    # Better optimization
strip = true         # Remove symbols
panic = "abort"      # Smaller panic handling
```

Then use `wasm-opt`:

```bash
# Install if needed
cargo install wasm-opt

# Optimize the WASM binary
wasm-opt -Oz \
    target/wasm32-unknown-unknown/release/my_plugin.wasm \
    -o plugin.wasm
```

### Testing Your Plugin

**1. Syntax Validation**:
```bash
wasm-validate plugin.wasm
```

**2. Local Testing**:
```bash
# Install to local Symphony
cp plugin.wasm ~/.config/symphony/plugins/my-plugin/
cp plugin.toml ~/.config/symphony/plugins/my-plugin/

# Run Symphony with debug output
symphony --verbose
```

**3. Check Logs**:
```bash
# View plugin logs
symphony --verbose 2>&1 | grep -i plugin
```

**4. Unit Testing**:

Create `src/lib.rs` with testable functions:

```rust
// Make functions testable
#[cfg(not(target_arch = "wasm32"))]
pub fn parse_track(json: &str) -> Option<TrackInfo> {
    serde_json::from_str(json).ok()
}

#[cfg(target_arch = "wasm32")]
fn parse_track(json: &str) -> Option<TrackInfo> {
    serde_json::from_str(json).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_track() {
        let json = r#"{"title":"Test","artist":"Artist","duration":180}"#;
        let track = parse_track(json).unwrap();
        assert_eq!(track.title, "Test");
        assert_eq!(track.artist, "Artist");
    }
}
```

Run tests:
```bash
cargo test
```

---

## Distribution

### Packaging

Create a distributable package:

```
my-plugin-v1.0.0/
├── plugin.wasm      # Compiled binary
├── plugin.toml      # Manifest
├── README.md        # Documentation
└── LICENSE          # License file
```

### Publishing

**Option 1: GitHub Release**:
```bash
# Create release archive
tar -czvf my-plugin-v1.0.0.tar.gz \
    plugin.wasm plugin.toml README.md LICENSE

# Upload to GitHub releases
gh release create v1.0.0 ./my-plugin-v1.0.0.tar.gz
```

**Option 2: Plugin Registry** (coming soon):
```bash
symphony plugin publish my-plugin/
```

### Installation Methods

Users can install your plugin:

```bash
# From local file
symphony plugin install ./my-plugin-v1.0.0.tar.gz

# From URL
symphony plugin install https://github.com/user/plugin/releases/download/v1.0.0/plugin.tar.gz

# Manual installation
tar -xzf my-plugin-v1.0.0.tar.gz -C ~/.config/symphony/plugins/my-plugin/
```

---

## Advanced Topics

### Managing Plugin State

For plugins that need to maintain state across events:

```rust
use std::cell::RefCell;

// Thread-local storage for state
thread_local! {
    static STATE: RefCell<PluginState> = RefCell::new(PluginState::default());
}

#[derive(Default)]
struct PluginState {
    tracks_played: u32,
    total_duration: u64,
    last_artist: Option<String>,
}

fn with_state<F, R>(f: F) -> R
where
    F: FnOnce(&mut PluginState) -> R,
{
    STATE.with(|s| f(&mut s.borrow_mut()))
}

#[no_mangle]
pub extern "C" fn on_event(
    event_type_ptr: i32,
    event_type_len: i32,
    event_data_ptr: i32,
    event_data_len: i32
) {
    let event_type = unsafe {
        str::from_utf8_unchecked(slice::from_raw_parts(
            event_type_ptr as *const u8,
            event_type_len as usize
        ))
    };
    
    if event_type == "track_changed" {
        with_state(|state| {
            state.tracks_played += 1;
            // Process track...
        });
    }
}
```

### HTTP Requests

Make network requests (requires `network` permission):

```rust
#[link(wasm_import_module = "env")]
extern "C" {
    fn host_http_request(
        method_ptr: i32, method_len: i32,
        url_ptr: i32, url_len: i32,
        headers_ptr: i32, headers_len: i32,
        body_ptr: i32, body_len: i32,
        response_ptr: i32, response_len: i32
    ) -> i32;
}

static mut HTTP_BUFFER: [u8; 65536] = [0; 65536];

fn http_get(url: &str) -> Option<String> {
    let headers = r#"{"User-Agent":"Symphony-Plugin"}"#;
    
    unsafe {
        let len = host_http_request(
            "GET".as_ptr() as i32, 3,
            url.as_ptr() as i32, url.len() as i32,
            headers.as_ptr() as i32, headers.len() as i32,
            0, 0,  // No body
            HTTP_BUFFER.as_mut_ptr() as i32,
            HTTP_BUFFER.len() as i32
        );
        
        if len <= 0 {
            return None;
        }
        Some(str::from_utf8_unchecked(&HTTP_BUFFER[..len as usize]).to_string())
    }
}
```

### Custom Configuration

Let users configure your plugin:

**In plugin.toml**:
```toml
[config]
api_key = { type = "string", default = "", description = "API key for service" }
notify_on_change = { type = "bool", default = true, description = "Show notifications" }
threshold = { type = "number", default = 50, description = "Minimum percentage to scrobble" }
```

**In your plugin**:
```rust
#[link(wasm_import_module = "env")]
extern "C" {
    fn host_get_config(key_ptr: i32, key_len: i32, buffer_ptr: i32, buffer_len: i32) -> i32;
}

fn get_config_string(key: &str) -> Option<String> {
    unsafe {
        let len = host_get_config(
            key.as_ptr() as i32, key.len() as i32,
            CONFIG_BUFFER.as_mut_ptr() as i32,
            CONFIG_BUFFER.len() as i32
        );
        if len <= 0 {
            return None;
        }
        Some(str::from_utf8_unchecked(&CONFIG_BUFFER[..len as usize]).to_string())
    }
}
```

---

## Troubleshooting

### Common Issues

**Plugin not loading**:
- Check `plugin.toml` syntax with a TOML validator
- Verify WASM exports: `wasm-objdump -x plugin.wasm | grep export`
- Check permissions match required host functions

**"Permission denied" errors**:
- Add required permission to `plugin.toml`
- Rebuild and reinstall the plugin

**Plugin crashes**:
- Add defensive error handling
- Use `log()` to debug
- Check for null pointers and invalid UTF-8

**Large binary size**:
- Enable LTO and size optimizations
- Run `wasm-opt -Oz`
- Remove unnecessary dependencies

### Debugging Tips

```rust
// Add extensive logging during development
fn debug_log(context: &str, value: &str) {
    log(&format!("[DEBUG] {}: {}", context, value));
}

// Check all return values
let result = unsafe { host_get_track(...) };
debug_log("get_track", &format!("returned {}", result));
if result < 0 {
    debug_log("get_track", "no track playing");
    return;
}
```

### Getting Help

- **Documentation**: [API Reference](API.md)
- **Examples**: https://github.com/symphony-player/plugin-examples
- **Community**: https://discord.gg/symphony-player
- **Issues**: https://github.com/symphony-player/symphony/issues

---

## Quick Reference

### Essential Cargo.toml

```toml
[package]
name = "my-plugin"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

[profile.release]
opt-level = "z"
lto = true
```

### Essential lib.rs Template

```rust
use std::slice;
use std::str;

#[link(wasm_import_module = "env")]
extern "C" {
    fn host_log(message_ptr: i32, message_len: i32);
}

fn log(msg: &str) {
    unsafe { host_log(msg.as_ptr() as i32, msg.len() as i32); }
}

#[no_mangle]
pub extern "C" fn plugin_init() {
    log("Plugin initialized");
}

#[no_mangle]
pub extern "C" fn on_event(
    event_type_ptr: i32, event_type_len: i32,
    _data_ptr: i32, _data_len: i32
) {
    let event_type = unsafe {
        str::from_utf8_unchecked(slice::from_raw_parts(
            event_type_ptr as *const u8, event_type_len as usize
        ))
    };
    log(&format!("Event: {}", event_type));
}

#[no_mangle]
pub extern "C" fn plugin_shutdown() {
    log("Plugin shutdown");
}
```

### Build & Install Commands

```bash
# Build
cargo build --target wasm32-unknown-unknown --release

# Optimize (optional)
wasm-opt -Oz target/wasm32-unknown-unknown/release/my_plugin.wasm -o plugin.wasm

# Install
mkdir -p ~/.config/symphony/plugins/my-plugin
cp plugin.wasm plugin.toml ~/.config/symphony/plugins/my-plugin/

# Test
symphony --verbose
```

---

*Happy plugin development! 🎵*
