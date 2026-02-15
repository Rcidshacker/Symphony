# Symphony Configuration Reference

**Complete guide to configuring Symphony v2.0**

---

## Table of Contents

1. [Configuration Overview](#configuration-overview)
2. [Configuration File Location](#configuration-file-location)
3. [General Settings](#general-settings)
4. [Audio Settings](#audio-settings)
5. [Visualizer Settings](#visualizer-settings)
6. [AI Settings](#ai-settings)
7. [Streaming Settings](#streaming-settings)
8. [Plugin Settings](#plugin-settings)
9. [Lyrics Settings](#lyrics-settings)
10. [Integration Settings](#integration-settings)
11. [Keybindings](#keybindings)
12. [Themes](#themes)
13. [Environment Variables](#environment-variables)
14. [Example Configurations](#example-configurations)

---

## Configuration Overview

Symphony uses TOML for configuration, providing a clean, human-readable format. The configuration system supports:

- **Hierarchical settings**: Nested sections for organized configuration
- **Environment variables**: Override settings via environment
- **Runtime changes**: Some settings can be changed without restart
- **Validation**: Invalid settings are reported with helpful error messages

### Configuration Priority

Settings are loaded in this order (later overrides earlier):

1. Default values (built into Symphony)
2. System-wide config (`/etc/symphony/config.toml`)
3. User config (`~/.config/symphony/config.toml`)
4. Environment variables
5. Command-line arguments

---

## Configuration File Location

### Default Location

```
~/.config/symphony/config.toml
```

### Alternative Locations

| Platform | Location |
|----------|----------|
| Linux | `~/.config/symphony/config.toml` |
| macOS | `~/.config/symphony/config.toml` |
| Windows | `%APPDATA%\symphony\config.toml` |

### Override Location

```bash
# Use custom config file
symphony --config /path/to/custom-config.toml

# Or set environment variable
export SYMPHONY_CONFIG_DIR=/custom/config/path
symphony
```

### Initialize Configuration

```bash
# Create default configuration file
symphony init

# Force overwrite existing config
symphony init --force
```

---

## General Settings

The `[general]` section controls basic application behavior.

```toml
[general]
# Music library directory (supports ~ expansion)
music_directory = "~/Music"

# Additional music directories (optional)
music_directories = ["~/Music", "/mnt/external/music"]

# Cache directory for streaming and lyrics
cache_directory = "~/.cache/symphony"

# Database file location
database_path = "~/.local/share/symphony/library.db"

# UI theme name (see themes/ directory for options)
theme = "monokai"

# Default view on startup
# Options: library, playlist, search, queue
default_view = "library"

# Show album art (when available)
show_album_art = true

# Confirm before quitting
confirm_quit = true

# Check for updates on startup
check_updates = true
```

### music_directory

**Type**: String (path)  
**Default**: `"~/Music"`  
**Environment**: `SYMPHONY_MUSIC_DIR`

The primary directory where Symphony looks for music files. Supports:
- `~` expansion for home directory
- Environment variables: `$HOME/Music`
- Relative paths (resolved from current directory)

### music_directories

**Type**: Array of strings  
**Default**: `[]`

Additional directories to scan for music. Use this to include multiple music locations:
```toml
music_directories = [
    "~/Music",
    "/mnt/nas/music",
    "/run/media/user/external/music"
]
```

### theme

**Type**: String  
**Default**: `"monokai"`  
**Options**: Built-in themes or custom theme filename

Built-in themes:
- `monokai` - Classic dark theme with warm colors
- `gruvbox` - Retro warm color palette
- `nord` - Arctic, cool blue tones
- `dracula` - Dark purple theme
- `solarized-dark` - Precision color scheme

### default_view

**Type**: String  
**Default**: `"library"`  
**Options**: `library`, `playlist`, `search`, `queue`

The view shown when Symphony starts.

---

## Audio Settings

The `[audio]` section controls audio playback behavior.

```toml
[audio]
# Audio output device
# Use "default" for system default
# Or specify device name/ID
output_device = "default"

# Sample rate for audio playback
sample_rate = 44100

# Audio buffer size (affects latency)
# Lower = lower latency, higher CPU
# Options: 512, 1024, 2048, 4096
buffer_size = 2048

# Default volume (0.0 to 1.0)
volume = 0.6

# Normalize loudness across tracks
normalize_audio = false

# Crossfade between tracks (seconds)
crossfade = 0

# Gapless playback
gapless = true
```

### output_device

**Type**: String  
**Default**: `"default"`

Specify the audio output device. Use `"default"` for system default, or specify a device name.

To list available devices:
```bash
symphony --list-audio-devices
```

### sample_rate

**Type**: Integer  
**Default**: `44100`  
**Options**: `44100`, `48000`, `96000`

Sample rate in Hz. Higher rates provide better quality but use more CPU.

### buffer_size

**Type**: Integer  
**Default**: `2048`  
**Options**: `512`, `1024`, `2048`, `4096`

Buffer size in samples. Smaller buffers reduce latency but increase CPU usage and may cause audio glitches on slower systems.

### normalize_audio

**Type**: Boolean  
**Default**: `false`

Enable loudness normalization using EBU R128 standard. This makes all tracks play at similar perceived loudness.

### Audio Effects

```toml
[audio.effects]
# Enable equalizer
equalizer = false

# Enable reverb effect
reverb = false

# Bass boost multiplier (1.0 = no boost)
bass_boost = 1.0

# Custom equalizer settings (10-band)
# Bands: 31Hz, 62Hz, 125Hz, 250Hz, 500Hz, 
#        1kHz, 2kHz, 4kHz, 8kHz, 16kHz
equalizer_bands = [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]
```

---

## Visualizer Settings

The `[visualizer]` section controls audio visualization.

```toml
[visualizer]
# Enable/disable visualizations
enabled = true

# Visualization mode
# Options: spectrum, waveform, circular, minimal, off
mode = "spectrum"

# FFT size (frequency resolution)
# Higher = more detail, more CPU
# Options: 2048, 4096, 8192
fft_size = 4096

# Smoothing factor (0.0 to 1.0)
# Higher = smoother transitions
smoothing = 0.8

# Color scheme
# Options: rainbow, monochrome, heatmap, fire, ice, custom
color_scheme = "rainbow"

# Enable beat detection visual pulse
beat_detection = true

# Visual sensitivity multiplier (0.1 to 3.0)
sensitivity = 1.0

# Bars count for spectrum mode
spectrum_bars = 32
```

### mode

**Type**: String  
**Default**: `"spectrum"`  
**Options**: `spectrum`, `waveform`, `circular`, `minimal`, `off`

Visualization display mode:
- **spectrum**: Classic frequency bars
- **waveform**: Audio waveform display
- **circular**: Circular frequency visualization
- **minimal**: Simple visual indicator
- **off**: No visualization

### fft_size

**Type**: Integer  
**Default**: `4096`  
**Options**: `2048`, `4096`, `8192`

FFT window size. Larger values give more frequency detail but use more CPU and may have slightly more latency.

### smoothing

**Type**: Float  
**Default**: `0.8`  
**Range**: `0.0` to `1.0`

Temporal smoothing for visualizations. Higher values make transitions smoother but less responsive.

### Album Art Settings

```toml
[visualizer.album_art]
# Enable album art display
enabled = true

# Display mode
# Options: auto, ascii, sixel, none
mode = "auto"

# ASCII art width in characters
ascii_width = 40

# Automatically fetch missing album art
fetch_missing = true

# Album art source
# Options: local, musicbrainz, lastfm
source = "local"
```

---

## AI Settings

The `[ai]` section configures AI integration.

```toml
[ai]
# Enable AI features
enabled = true

# AI provider: "ollama" (local) or "openrouter" (cloud)
provider = "ollama"

# Enable AI-powered recommendations
recommendations_enabled = true

# Enable natural language commands
natural_language_commands = true

# Cache AI responses
cache_responses = true

# Maximum tokens for AI responses
max_tokens = 500

# Temperature for AI responses (0.0 to 2.0)
temperature = 0.7
```

### Ollama Configuration (Local AI)

```toml
[ai.ollama]
# Ollama server URL
base_url = "http://localhost:11434"

# Model to use
# Recommended: llama3.2:3b (fast, capable)
# Alternatives: llama3.1:8b, mistral:7b, gemma2:9b
model = "llama3.2:3b"

# Request timeout in seconds
timeout = 30
```

### OpenRouter Configuration (Cloud AI)

```toml
[ai.openrouter]
# OpenRouter API key
api_key = ""

# Model to use
# Options: anthropic/claude-3.5-sonnet, openai/gpt-4o, etc.
model = "anthropic/claude-3.5-sonnet"

# Site URL for attribution
site_url = "https://github.com/symphony-player/symphony"

# Application name
app_name = "Symphony Music Player"
```

---

## Streaming Settings

Configure YouTube and Spotify streaming.

```toml
[streaming]
# Enable streaming features
enabled = true
```

### YouTube Configuration

```toml
[streaming.youtube]
# Enable YouTube streaming
enabled = true

# Stream quality
# Options: low, medium, high, best
quality = "medium"

# Path to yt-dlp binary
ytdlp_path = "yt-dlp"

# Download format preference
format = "bestaudio"

# Auto-download liked tracks
auto_download_likes = false
```

### Spotify Configuration

```toml
[streaming.spotify]
# Enable Spotify integration
enabled = false

# Spotify API credentials (from developer.spotify.com)
client_id = ""
client_secret = ""

# OAuth redirect URI
redirect_uri = "http://localhost:8888/callback"

# Stream quality
# Options: low, normal, high
quality = "high"

# Sync liked songs to local library
sync_liked = false
```

### Cache Configuration

```toml
[streaming.cache]
# Enable caching
enabled = true

# Cache directory
cache_dir = "~/.cache/symphony/streams"

# Hot tier (memory) size in MB
max_hot_size_mb = 100

# Warm tier (SSD) size in MB
max_warm_size_mb = 2048

# Cold tier (HDD) size in MB
max_cold_size_mb = 20480

# Enable predictive prefetching
prefetch_enabled = true

# Number of tracks to prefetch
prefetch_count = 3

# Cache expiration in days
expiration_days = 30
```

### Network Configuration

```toml
[streaming.network]
# Maximum concurrent downloads
max_concurrent_downloads = 2

# Bandwidth limit in Mbps (0 = unlimited)
bandwidth_limit_mbps = 0

# Prefer cached content
prefer_cached = true

# Request timeout in seconds
timeout_secs = 30

# Retry attempts
retry_attempts = 3
```

---

## Plugin Settings

Configure the plugin system.

```toml
[plugins]
# Enable plugin system
enabled = true

# Plugin directory
plugin_dir = "~/.config/symphony/plugins"

# Auto-load these plugins on startup
auto_load = []
```

### Discord Plugin

```toml
[plugins.discord]
enabled = false

# Show elapsed timestamp
show_timestamp = true

# Show album art as large image
show_album_art = true

# Custom Discord application ID (optional)
app_id = ""
```

### Last.fm Plugin

```toml
[plugins.lastfm]
enabled = false

# Last.fm API credentials
api_key = ""
api_secret = ""
username = ""

# Scrobble threshold (percentage of track played)
scrobble_threshold = 50

# Show now playing status
show_now_playing = true
```

### Notifications Plugin

```toml
[plugins.notifications]
enabled = false

# Show on track change
on_track_change = true

# Show on playlist end
on_playlist_end = true

# Show album art in notification
show_album_art = true
```

---

## Lyrics Settings

Configure lyrics display.

```toml
[lyrics]
# Enable lyrics features
enabled = true

# Lyrics cache directory
cache_dir = "~/.cache/symphony/lyrics"

# Auto-fetch missing lyrics
auto_fetch = true

# Show translation below lyrics
show_translation = false

# Target language for translations
translation_lang = "en"

# Karaoke-style highlighting
karaoke_mode = true

# Context lines (before/after current)
context_lines = 2

# Font size for lyrics display
font_size = "normal"
```

### Lyrics Fetcher

```toml
[lyrics.fetcher]
# Enable online fetching
enabled = true

# Sources to try (in order)
sources = ["lrclib", "musixmatch", "genius"]

# Cache duration in days
cache_duration_days = 30

# Request timeout in seconds
timeout = 10
```

---

## Integration Settings

### Git Integration

```toml
[integrations.git]
# Enable Git monitoring
enabled = true

# Adapt music to coding activity
adapt_music = false

# How often to check Git status (seconds)
check_interval = 60

# Repositories to monitor (empty = auto-detect)
repo_paths = []

# Activity thresholds (commits per hour)
[integrations.git.thresholds]
idle = 0
light = 2
focused = 5
intense = 10
```

### Pomodoro Timer

```toml
[integrations.pomodoro]
# Enable Pomodoro timer
enabled = false

# Work session duration (minutes)
work_minutes = 25

# Short break duration (minutes)
break_minutes = 5

# Long break duration (minutes)
long_break_minutes = 15

# Sessions until long break
sessions_until_long_break = 4

# Auto-start breaks
auto_start_break = false

# Auto-start work sessions
auto_start_work = false

# Show desktop notifications
notifications = true

# Play sound on session end
sound = true
```

### Status Line

```toml
[integrations.statusline]
# Enable tmux status line
tmux_enabled = false

# Enable zsh status line
zsh_enabled = false

# Status file path (for external tools)
status_file = ""

# Update interval (milliseconds)
update_interval = 1000

# Status format
# Variables: {artist}, {title}, {album}, {position}, {duration}
format = "{artist} - {title}"
```

---

## Keybindings

Customize keyboard shortcuts.

```toml
[keybindings]
# Playback
play_pause = "Space"
skip_next = "]"
skip_prev = "["
volume_up = "+"
volume_down = "-"
mute = "m"
shuffle = "s"
repeat = "r"

# Navigation
up = "k"
down = "j"
page_up = "Ctrl+u"
page_down = "Ctrl+d"
top = "g"
bottom = "G"
select = "Enter"

# Search and AI
search = "/"
ai_command = "a"

# Views
library = "1"
playlist = "2"
queue = "3"
search_view = "4"

# Visualizer
cycle_viz = "v"
viz_spectrum = "1"
viz_waveform = "2"
viz_circular = "3"
viz_minimal = "4"

# Lyrics
toggle_lyrics = "l"
karaoke = "K"
translate = "T"

# Integrations
pomodoro_start = "p"
pomodoro_break = "b"
plugin_manager = "P"

# General
help = "h"
quit = "q"
```

### Key Format

Keys can be specified as:
- Single character: `"a"`, `"1"`, `"/"`
- Named keys: `"Space"`, `"Enter"`, `"Esc"`
- With modifiers: `"Ctrl+c"`, `"Alt+1"`, `"Shift+a"`

---

## Themes

### Using Built-in Themes

```toml
[general]
theme = "gruvbox"
```

### Creating Custom Themes

Create a TOML file in `~/.config/symphony/themes/`:

```toml
# ~/.config/symphony/themes/my-theme.toml
name = "my-theme"
author = "Your Name"

[colors]
# Background color
background = "#1a1a2e"

# Primary foreground/text
foreground = "#eaeaea"

# Accent/highlight color
accent = "#16c79a"

# Secondary accent
secondary = "#0f3460"

# Error messages
error = "#e94560"

# Warning messages
warning = "#f39c12"

# Success indicators
success = "#27ae60"

# Muted/dimmed text
muted = "#666666"

[ui]
# Border style: plain, rounded, double, thick
border_style = "rounded"

# Highlight selected items
selected_highlight = true

# Progress bar style: solid, blocks, dots
progress_style = "blocks"
```

### Color Formats

Colors can be specified as:
- Hex: `"#ff0000"`
- RGB: `"rgb(255, 0, 0)"`
- Named: `"red"`, `"blue"`, `"green"`

---

## Environment Variables

Override configuration via environment:

| Variable | Description | Example |
|----------|-------------|---------|
| `SYMPHONY_CONFIG_DIR` | Config directory | `/custom/config` |
| `SYMPHONY_DATA_DIR` | Data directory | `/custom/data` |
| `SYMPHONY_CACHE_DIR` | Cache directory | `/custom/cache` |
| `SYMPHONY_MUSIC_DIR` | Music library | `/mnt/music` |
| `SYMPHONY_THEME` | Theme name | `gruvbox` |
| `SYMPHONY_VOLUME` | Volume level | `0.8` |
| `SYMPHONY_LOG_LEVEL` | Log level | `debug` |
| `SYMPHONY_NO_COLOR` | Disable colors | `1` |

Example:
```bash
export SYMPHONY_THEME=gruvbox
export SYMPHONY_VOLUME=0.7
symphony
```

---

## Example Configurations

### Minimal Configuration

```toml
# Minimal Symphony config
[general]
music_directory = "~/Music"
theme = "monokai"

[audio]
volume = 0.6

[ai]
enabled = false
```

### Developer Configuration

```toml
# Symphony config for developers
[general]
music_directory = "~/Music"
theme = "dracula"

[audio]
volume = 0.5
normalize_audio = true

[visualizer]
enabled = false

[ai]
enabled = true
provider = "ollama"

[ai.ollama]
model = "llama3.2:3b"

[integrations.git]
enabled = true
adapt_music = true

[integrations.pomodoro]
enabled = true
work_minutes = 25

[plugins.discord]
enabled = true
```

### Streaming Focus

```toml
# Symphony config for streaming
[general]
theme = "nord"

[streaming]
enabled = true

[streaming.youtube]
enabled = true
quality = "high"

[streaming.cache]
max_warm_size_mb = 4096
prefetch_enabled = true
prefetch_count = 5

[ai]
enabled = true
provider = "ollama"
```

### Low Resource

```toml
# Symphony config for low-resource systems
[general]
show_album_art = false

[audio]
buffer_size = 4096

[visualizer]
enabled = false

[streaming.cache]
max_hot_size_mb = 50
max_warm_size_mb = 512

[ai]
enabled = false
```

---

## Configuration Validation

### Check Configuration

```bash
# Validate config file
symphony --check-config

# Show effective configuration
symphony --show-config
```

### Common Errors

| Error | Cause | Solution |
|-------|-------|----------|
| `Invalid TOML` | Syntax error | Check quotes, brackets |
| `Unknown option` | Typo or wrong section | Verify option name |
| `Invalid value` | Wrong type or range | Check expected format |
| `Permission denied` | Can't read/write file | Check file permissions |

---

*Configuration Reference for Symphony v2.0*
