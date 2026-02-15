# Symphony v2.0 User Guide

**The AI-Powered Terminal Music Player for Developers**

---

## Table of Contents

1. [Introduction](#introduction)
2. [Installation](#installation)
3. [Quick Start](#quick-start)
4. [Basic Usage](#basic-usage)
5. [AI Features](#ai-features)
6. [Streaming](#streaming)
7. [Visualizations](#visualizations)
8. [Plugins](#plugins)
9. [Lyrics](#lyrics)
10. [Developer Integrations](#developer-integrations)
11. [Keyboard Reference](#keyboard-reference)
12. [Configuration](#configuration)
13. [Troubleshooting](#troubleshooting)

---

## Introduction

Symphony is a revolutionary terminal music player designed specifically for developers who spend their days in the command line. Unlike traditional music players, Symphony lives entirely in your terminal, providing a seamless, keyboard-driven experience that integrates with your development workflow.

### What Makes Symphony Different?

**AI-Powered Intelligence**: Symphony understands natural language commands. Instead of navigating menus, simply tell it what you want: "play something energetic for coding" or "find songs like Radiohead but jazzier." The AI learns your preferences and adapts to your workflow.

**Hybrid Playback**: Your local music library, YouTube, and Spotify all in one interface. Seamlessly switch between sources without leaving your terminal. Smart caching means your favorite streaming tracks are available offline.

**Developer-Focused Features**: Git integration that adapts music to your coding intensity, a built-in Pomodoro timer, and status line integration for tmux and zsh. Symphony understands that developers have unique needs.

**Stunning Visualizations**: Real-time FFT spectrum analyzer with multiple visualization modes, album art rendered as ASCII art or Sixel graphics, and beat-reactive animations that make your terminal come alive.

**Extensible Architecture**: WASM-based plugin system allows you to extend Symphony with custom functionality. Official plugins include Discord Rich Presence, Last.fm scrobbling, and notification support.

---

## Installation

### Prerequisites

Before installing Symphony, ensure your system meets the following requirements:

**Rust (Required for building from source)**:
```bash
# Install Rust via rustup
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

Symphony requires Rust 1.75 or later. Verify your installation:
```bash
rustc --version
```

**System Dependencies**:

Linux (Ubuntu/Debian):
```bash
sudo apt update
sudo apt install -y libasound2-dev libpulse-dev pkg-config build-essential
```

Linux (Arch):
```bash
sudo pacman -S alsa-lib libpulse pkg-config base-devel
```

Linux (Fedora):
```bash
sudo dnf install alsa-lib-devel pulseaudio-libs-devel pkgconfig gcc
```

macOS:
```bash
brew install portaudio pkg-config
```

**Optional Dependencies**:

For YouTube streaming, install yt-dlp:
```bash
# Using pip
pip install yt-dlp

# Using Homebrew
brew install yt-dlp

# Using apt (Ubuntu)
sudo apt install yt-dlp
```

For FFmpeg audio format support:
```bash
# Ubuntu/Debian
sudo apt install ffmpeg

# macOS
brew install ffmpeg

# Arch
sudo pacman -S ffmpeg
```

### Installation Methods

#### Method 1: Build from Source (Recommended)

Building from source gives you the latest features and optimizations:

```bash
# Clone the repository
git clone https://github.com/symphony-player/symphony.git
cd symphony

# Build in release mode
cargo build --release

# The binary will be at target/release/symphony
# Optionally, install system-wide
sudo cp target/release/symphony /usr/local/bin/
```

#### Method 2: Install via Cargo

```bash
cargo install symphony-player
```

#### Method 3: Homebrew (macOS/Linux)

```bash
brew tap symphony-player/tap
brew install symphony
```

#### Method 4: AUR (Arch Linux)

```bash
# Using yay
yay -S symphony-bin

# Using paru
paru -S symphony-bin
```

### Verifying Installation

After installation, verify Symphony is working:

```bash
symphony --version
symphony --help
```

---

## Quick Start

### First Run Setup

When you run Symphony for the first time, it will create the necessary configuration directories and files:

```bash
symphony init
```

This creates:
- `~/.config/symphony/config.toml` - Main configuration file
- `~/.local/share/symphony/` - Database and library data
- `~/.cache/symphony/` - Cached streaming content and lyrics

### Scanning Your Music Library

Point Symphony to your music collection:

```bash
# Scan a single directory
symphony scan ~/Music

# Scan multiple directories
symphony scan ~/Music ~/Downloads/music /mnt/external/music

# Scan with verbose output
symphony scan --verbose ~/Music
```

Symphony supports the following audio formats:
- MP3 (ID3v1, ID3v2 tags)
- FLAC (Vorbis comments)
- AAC/M4A (iTunes tags)
- OGG Vorbis (Vorbis comments)
- WAV (limited metadata support)

The scanner will:
1. Recursively find all audio files
2. Extract metadata (title, artist, album, etc.)
3. Generate audio fingerprints for duplicate detection
4. Create embeddings for AI-powered search
5. Store everything in the local database

### Starting the Player

Launch the interactive terminal interface:

```bash
symphony
```

Or use AI to start playing immediately:

```bash
symphony "play something energetic for coding"
```

### Basic Controls

Once in the interface, use these essential keyboard shortcuts:

| Key | Action |
|-----|--------|
| `Space` | Play/Pause |
| `n` or `]` | Next track |
| `p` or `[` | Previous track |
| `+`/`=` | Volume up |
| `-` | Volume down |
| `m` | Mute toggle |
| `/` | Search |
| `j`/`k` | Navigate down/up |
| `Enter` | Play selected track |
| `q` or `Ctrl+C` | Quit |

---

## Basic Usage

### The User Interface

Symphony's interface is divided into several panels:

```
┌──────────────────────────────────────────────────────────────────────────┐
│  🎵 NOW PLAYING                        │  📊 SPECTRUM ANALYZER          │
│                                         │                                 │
│  Radiohead - Paranoid Android          │  ████████████░░░░░░░░          │
│  Album: OK Computer                     │  ████████████████░░░░          │
│  ━━━━━━━━━━━━●────────── 3:42/6:23     │  ████████████████████          │
│                                         │  ████████████████░░░░          │
│  🔊 Volume: ▓▓▓▓▓▓░░░░ 60%             │  ████████████░░░░░░░░          │
├─────────────────────────────────────────┴────────────────────────────────┤
│  📚 LIBRARY                          📝 PLAYLIST                         │
│  ┌────────────────────────────────────────────────────────────────────┐  │
│  │ > Radiohead - Paranoid Android           6:23  [LOCAL]            │  │
│  │   The Smiths - This Charming Man          2:43  [SPOTIFY]         │  │
│  │   Arcade Fire - Wake Up                   5:39  [YOUTUBE]         │  │
│  │   Pixies - Where Is My Mind?              3:53  [LOCAL]           │  │
│  └────────────────────────────────────────────────────────────────────┘  │
│  🎯 AI: "Try The National - Fake Empire"                                 │
└──────────────────────────────────────────────────────────────────────────┘
│  ⌨️ Space:Play  n:Next  /:Search  a:AI  v:Visualizer  h:Help            │
└──────────────────────────────────────────────────────────────────────────┘
```

### Navigation

Symphony uses vim-style navigation that developers will find familiar:

- **j/k**: Move down/up in lists (standard vim keys)
- **Arrow keys**: Alternative navigation for non-vim users
- **g/G**: Jump to beginning/end of list
- **Ctrl+d/u**: Page down/up
- **Enter**: Select and play the highlighted track

### Playlist Management

Create and manage playlists directly from the interface:

**Creating a Playlist**:
1. Press `:` to enter command mode
2. Type `playlist create <name>` and press Enter
3. Navigate to tracks and press `a` to add to current playlist

**Playlist Commands**:
```
:playlist create <name>     Create a new playlist
:playlist delete <name>     Delete a playlist
:playlist rename <old> <new> Rename a playlist
:playlist add <track_id>    Add track to current playlist
:playlist remove <index>    Remove track at index
:playlist shuffle           Shuffle current playlist
:playlist save              Save current queue as playlist
```

### Search

Press `/` to enter search mode. Symphony supports several search types:

**Text Search**:
```
radiohead          # Search for "radiohead" in titles and artists
album:"OK Computer" # Search specific field
artist:Radiohead   # Search by artist
```

**AI-Powered Semantic Search**:
```
energetic coding music
melancholic songs for rainy days
similar to pink floyd but modern
```

The AI understands context and mood, returning results that match the feeling you describe rather than just matching keywords.

### Queue Management

The play queue shows upcoming tracks:

- `s` - Toggle shuffle mode
- `r` - Cycle repeat modes (none → single → all)
- `x` - Remove current track from queue
- `c` - Clear entire queue

---

## AI Features

Symphony's AI integration is what sets it apart from every other terminal music player. Powered by local LLMs (via Ollama) with optional cloud fallback, your data stays private while you get intelligent music assistance.

### Setting Up AI

**Local AI (Ollama) - Recommended**:

1. Install Ollama:
```bash
# Linux/macOS
curl -fsSL https://ollama.com/install.sh | sh

# Start Ollama service
ollama serve
```

2. Pull a model:
```bash
# Recommended for music tasks (fast, capable)
ollama pull llama3.2:3b

# More capable but slower
ollama pull llama3.1:8b

# Alternative models
ollama pull mistral:7b
ollama pull gemma2:9b
```

3. Configure Symphony to use Ollama (in `~/.config/symphony/config.toml`):
```toml
[ai]
enabled = true
provider = "ollama"

[ai.ollama]
base_url = "http://localhost:11434"
model = "llama3.2:3b"
```

**Cloud AI (OpenRouter)**:

If you prefer cloud AI or need more powerful models:

```toml
[ai]
enabled = true
provider = "openrouter"

[ai.openrouter]
api_key = "your-api-key-here"
model = "anthropic/claude-3.5-sonnet"
```

### Natural Language Commands

Press `a` to open the AI command input, then type naturally:

**Playback Commands**:
```
play something energetic for coding
find sad songs from the 90s
play that song from the coffee shop
skip to something happier
```

**Discovery Commands**:
```
find artists similar to Radiohead
what album is this song from?
recommend music for a dinner party
suggest songs with good bass lines
```

**Playlist Commands**:
```
create a 2-hour focus session starting calm
build a workout playlist that peaks in the middle
make a playlist for winding down after work
```

**Context-Aware Commands**:
```
what should I listen to while debugging?
play music that matches the weather
something for late-night coding
```

### Smart Playlists

AI-generated playlists adapt to your preferences over time:

**Built-in Smart Playlists**:
- **Focus Flow**: Calm, instrumental tracks for deep work
- **Energy Surge**: High-tempo music for active coding
- **Wind Down**: Gradually decreasing tempo toward the end of day
- **Discovery**: New tracks similar to your favorites

**Creating Custom Smart Playlists**:
```
:ai playlist "morning coffee vibes - jazzy, upbeat, under 3 minutes"
:ai playlist "coding deep focus - instrumental, electronic, no vocals"
```

### AI Recommendations

Symphony learns from your listening habits:

1. **Skip Behavior**: Frequently skipped tracks are deprioritized
2. **Replay Patterns**: Tracks played repeatedly get higher scores
3. **Time Context**: Learns what you like at different times of day
4. **Activity Context**: Integrates with Git to match music to coding intensity

The recommendation engine combines:
- Content-based filtering (audio features, genre, tempo)
- Collaborative filtering (what similar users enjoy)
- Contextual signals (time, activity, weather)

---

## Streaming

Symphony unifies local files, YouTube, and Spotify into a single seamless experience.

### YouTube Integration

**Prerequisites**: Install yt-dlp (see Installation section)

**Searching YouTube**:
Press `y` to open YouTube search, or use commands:

```
:youtube search "lofi hip hop radio"
:youtube search "Radiohead live Glastonbury 1997"
```

**Playing from YouTube**:
```
# Direct URL
:youtube play https://youtube.com/watch?v=...

# Search and play first result
:youtube play "daft punk one more time"

# Add to queue
:youtube queue "pink floyd comfortably numb live"
```

**YouTube-Specific Options**:
```toml
[streaming.youtube]
enabled = true
quality = "medium"  # low, medium, high, best
# Auto-download liked YouTube tracks
auto_download_likes = true
```

### Spotify Integration

**Setup**:

1. Create a Spotify Developer App:
   - Go to https://developer.spotify.com/dashboard
   - Create a new app
   - Copy the Client ID and Client Secret

2. Configure Symphony:
```toml
[streaming.spotify]
enabled = true
client_id = "your-client-id"
client_secret = "your-client-secret"
redirect_uri = "http://localhost:8888/callback"
```

3. Authenticate:
```bash
symphony spotify auth
```

This opens a browser for you to authorize Symphony. After authorization, tokens are stored securely.

**Using Spotify**:
```
:spotify search "artist:Radiohead"
:spotify playlist "37i9dQZF1DXcBWIGoYBM5M"  # Play by ID
:spotify liked                            # Your liked songs
:spotify album "OK Computer"              # Play album
```

### Smart Caching

Symphony's 3-tier cache system ensures smooth playback:

```
┌─────────────────────────────────────────────────────────────────────────┐
│                          SMART CACHE ARCHITECTURE                        │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  ┌─────────────┐   ┌─────────────┐   ┌─────────────────────────────┐   │
│  │   HOT TIER  │   │  WARM TIER  │   │         COLD TIER           │   │
│  │   (Memory)  │   │   (SSD)     │   │          (HDD)              │   │
│  │   ~100 MB   │   │   ~2 GB     │   │         ~20 GB              │   │
│  │   < 1 ms    │   │   < 10 ms   │   │         < 100 ms            │   │
│  └─────────────┘   └─────────────┘   └─────────────────────────────┘   │
│                                                                          │
│  Predictive Prefetch: Downloads likely-next tracks in background        │
│  Adaptive Quality: Adjusts stream quality based on bandwidth            │
│  Offline Mode: Automatically switches to cached versions                │
└─────────────────────────────────────────────────────────────────────────┘
```

**Cache Configuration**:
```toml
[streaming.cache]
enabled = true
cache_dir = "~/.cache/symphony/streams"
max_hot_size_mb = 100      # Memory cache
max_warm_size_mb = 2048    # Fast SSD cache
max_cold_size_mb = 20480   # Large HDD cache
prefetch_enabled = true    # Predictive downloading
prefetch_count = 3         # How many tracks to prefetch
```

### Offline Mode

Work without internet:

```bash
# Start in offline mode
symphony --offline

# Or toggle in-app with 'o'
```

In offline mode, Symphony:
- Only shows cached tracks
- Prefers local files over streaming
- Queues downloads for when connectivity returns

---

## Visualizations

Symphony features real-time FFT-based visualizations that transform your terminal into an audio spectrum analyzer.

### Visualization Modes

Press `v` to cycle through modes, or use number keys for direct selection:

**Mode 1: Spectrum Bars (Default)**
```
┌────────────────────────────────────────────────────────────┐
│ ████    ████████    ████████████    ████████    ████       │
│ ████    ████████    ████████████    ████████    ████       │
│ ████    ████████    ████████████    ████████    ████       │
│                                                          │
│ Bass    Mid-Low    Mid         Mid-High    Treble       │
│ 20Hz    200Hz      2kHz        8kHz        20kHz        │
└────────────────────────────────────────────────────────────┘
```

**Mode 2: Waveform**
```
┌────────────────────────────────────────────────────────────┐
│        ╭──────╮                    ╭──────╮               │
│    ╭───╯      ╰──╮            ╭───╯      ╰───╮           │
│ ╭──╯              ╰──╮    ╭──╯              ╰──╮         │
│╯                    ╰────╯                    ╰──        │
└────────────────────────────────────────────────────────────┘
```

**Mode 3: Circular**
```
┌────────────────────────────────────────────────────────────┐
│                    ╭──────╮                                │
│               ╭────╯      ╰────╮                          │
│           ╭───╯    ◉ ♪ ♫     ╰───╮                       │
│          │                        │                       │
│           ╰───╮              ╭───╯                        │
│               ╰────╮    ╭────╯                            │
│                    ╰────╯                                 │
└────────────────────────────────────────────────────────────┘
```

**Mode 4: Minimal**
```
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
 ♪ ♫ ♪ ♫ ♪                    ♫ ♪ ♫ ♪ ♫
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
```

### Visualization Settings

```toml
[visualizer]
enabled = true
mode = "spectrum"      # spectrum, waveform, circular, minimal
fft_size = 4096        # 2048, 4096, 8192 (higher = more detail, more CPU)
smoothing = 0.8        # 0.0 - 1.0 (higher = smoother transitions)
color_scheme = "rainbow"  # rainbow, monochrome, heatmap, custom
beat_detection = true  # Pulse on detected beats
sensitivity = 1.0      # 0.1 - 3.0 (visual amplitude multiplier)
```

### Album Art

Symphony displays album art directly in your terminal:

**ASCII Mode** (works in all terminals):
- Converts album art to ASCII characters
- Adjustable resolution and dithering

**Sixel Mode** (Kitty, iTerm2, WezTerm):
- True-color image display
- Higher quality than ASCII

**Configuration**:
```toml
[visualizer.album_art]
enabled = true
mode = "auto"          # auto, ascii, sixel, none
ascii_width = 40       # Character width for ASCII mode
fetch_missing = true   # Download missing art from MusicBrainz
```

---

## Plugins

Symphony's plugin system lets you extend functionality with WASM-based plugins that run in a secure sandbox.

### Plugin Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         PLUGIN RUNTIME                                   │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  ┌────────────────┐    ┌────────────────┐    ┌────────────────┐        │
│  │ Discord Plugin │    │ Last.fm Plugin │    │ Custom Plugin  │        │
│  │   (WASM)       │    │   (WASM)       │    │   (WASM)       │        │
│  └───────┬────────┘    └───────┬────────┘    └───────┬────────┘        │
│          │                      │                      │                 │
│          └──────────────────────┼──────────────────────┘                 │
│                                 ▼                                        │
│                    ┌────────────────────────┐                           │
│                    │    Host Function API   │                           │
│                    │  • get_current_track() │                           │
│                    │  • scrobble()          │                           │
│                    │  • set_presence()      │                           │
│                    │  • notify()            │                           │
│                    └────────────────────────┘                           │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

### Official Plugins

**Discord Rich Presence**:
Shows what you're listening to on your Discord profile:

```toml
[plugins.discord]
enabled = true
show_timestamp = true     # Show elapsed time
show_album_art = true     # Show album art (as external link)
app_id = ""               # Custom Discord app ID (optional)
```

**Last.fm Scrobbler**:
Automatically scrobbles tracks you listen to:

```toml
[plugins.lastfm]
enabled = true
api_key = "your-api-key"
api_secret = "your-api-secret"
username = "your-username"
# Scrobble after this percentage played
scrobble_threshold = 50
```

**Desktop Notifications**:
System notifications for track changes:

```toml
[plugins.notifications]
enabled = true
on_track_change = true
on_playlist_end = true
show_album_art = true
```

### Managing Plugins

**List installed plugins**:
```
:plugin list
```

**Enable/disable a plugin**:
```
:plugin enable discord
:plugin disable lastfm
```

**Install a plugin**:
```
:plugin install /path/to/plugin.wasm
:plugin install https://example.com/plugins/cool-plugin.wasm
```

**Reload plugins** (after adding new ones):
```
:plugin reload
```

### Creating Custom Plugins

See the [Plugin Development Guide](PLUGIN_DEVELOPMENT.md) for detailed instructions on creating your own plugins.

Quick overview:
1. Create a Rust library project
2. Implement the plugin interface
3. Compile to WASM
4. Place in `~/.config/symphony/plugins/`

---

## Lyrics

Symphony provides synchronized lyrics with karaoke-style display.

### Lyrics Sources

1. **Local LRC Files**: Place `.lrc` files alongside your music files
2. **Embedded Lyrics**: Read from audio file metadata
3. **Online Fetching**: Automatically search for lyrics

### LRC File Format

Create `.lrc` files with timestamp-synced lyrics:

```lrc
[ti:Paranoid Android]
[ar:Radiohead]
[al:OK Computer]
[au:Thom Yorke]
[length:383.42]

[00:00.00]Please could you stop the noise
[00:04.50]I'm trying to get some rest
[00:09.00]From all the unborn chicken
[00:13.50]Voices in my head
[00:18.00]What's that?
[00:20.00]What's that?
```

### Lyrics Display

Press `l` to toggle the lyrics panel. In lyrics mode:

- Current line is highlighted
- Past lines fade out
- Future lines are dimmed
- Beat-synced scrolling

**Karaoke Mode**:
Press `K` to toggle karaoke mode, which highlights words as they're sung (requires word-level timestamps in LRC file).

**Translations**:
Press `T` to fetch translations for foreign language songs:

```toml
[lyrics]
enabled = true
auto_fetch = true           # Auto-fetch missing lyrics
show_translation = true     # Show translations
translation_lang = "en"     # Target language for translations
karaoke_mode = true         # Word-level highlighting
context_lines = 2           # Lines before/after current
```

### Online Lyrics

Configure online sources:

```toml
[lyrics.fetcher]
enabled = true
sources = ["lrclib", "musixmatch", "genius"]  # Priority order
cache_duration_days = 30
```

---

## Developer Integrations

Symphony integrates deeply with developer workflows.

### Git Integration

Symphony monitors your Git activity and can adapt music accordingly:

**Activity Detection**:
```
┌─────────────────────────────────────────────────────────────┐
│                    GIT ACTIVITY MONITOR                      │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  Commit Velocity: ████████░░  80%                           │
│  Branch: feature/awesome-feature                            │
│  Last Commit: 5 minutes ago                                 │
│                                                              │
│  Activity Level: INTENSE 🔥                                  │
│  → Music adapted: Higher tempo, energetic tracks            │
│                                                              │
│  Time in Flow: 2h 34m                                       │
│  Suggest break in: 26 minutes                               │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

**Configuration**:
```toml
[integrations.git]
enabled = true
adapt_music = true          # Auto-adjust music to activity
check_interval = 60         # Seconds between checks
repo_paths = []             # Auto-detect from cwd if empty

# Activity thresholds (commits per hour)
[integrations.git.thresholds]
idle = 0        # Less than this = idle
light = 2       # 0-2 = light activity
focused = 5     # 2-5 = focused
intense = 10    # 5+ = intense
```

### Pomodoro Timer

Built-in focus timer with music integration:

**Controls**:
- `p` - Start/pause Pomodoro
- `b` - Start break
- `P` - Pomodoro settings

**Display**:
```
┌───────────────────────────────────────────┐
│  🍅 POMODORO                              │
│                                           │
│  Session 3/4                              │
│  ████████████████░░░░░░░░ 18:42 / 25:00  │
│                                           │
│  Current: Lo-fi beats for focus           │
│  Next: 5 min break                        │
└───────────────────────────────────────────┘
```

**Configuration**:
```toml
[integrations.pomodoro]
enabled = true
work_minutes = 25
break_minutes = 5
long_break_minutes = 15
sessions_until_long_break = 4
auto_start_break = false
auto_start_work = false
notifications = true

# Music for different phases
[integrations.pomodoro.music]
work_playlist = "Focus Flow"
break_playlist = "Wind Down"
```

### Status Line Integration

Show current track in tmux or zsh status:

**Tmux**:
Add to `~/.tmux.conf`:
```
set -g status-right '#(symphony --status-line) | %H:%M '
```

**Zsh**:
Add to `~/.zshrc`:
```bash
# Right prompt
RPROMPT='$(symphony --status-short)'

# Or in a custom prompt
function prompt_symphony() {
    local status=$(symphony --status-short 2>/dev/null)
    if [[ -n "$status" ]]; then
        echo "🎵 $status"
    fi
}
```

**Output Formats**:
```bash
# Full status
symphony --status-line
# Output: 🎵 Radiohead - Paranoid Android [3:21/6:23]

# Short status
symphony --status-short
# Output: Radiohead - Paranoid Android

# JSON for scripts
symphony --status-json
# Output: {"artist":"Radiohead","title":"Paranoid Android","position":201,"duration":383}
```

---

## Keyboard Reference

### Playback Controls

| Key | Action |
|-----|--------|
| `Space` | Play/Pause |
| `n` or `]` | Next track |
| `p` or `[` | Previous track |
| `+` or `=` | Volume up |
| `-` | Volume down |
| `m` | Mute toggle |
| `s` | Shuffle toggle |
| `r` | Repeat cycle (off → single → all) |

### Navigation

| Key | Action |
|-----|--------|
| `j` or `↓` | Move down |
| `k` or `↑` | Move up |
| `g` | Go to beginning |
| `G` | Go to end |
| `Ctrl+d` | Page down |
| `Ctrl+u` | Page up |
| `Enter` | Play selected track |
| `x` | Remove from queue |

### Search & Filter

| Key | Action |
|-----|--------|
| `/` | Enter search mode |
| `Esc` | Exit search/cancel |
| `Enter` | Execute search |
| `Backspace` | Delete character |

### Visualizer

| Key | Action |
|-----|--------|
| `v` | Cycle visualization mode |
| `1` | Spectrum bars |
| `2` | Waveform |
| `3` | Circular |
| `4` | Minimal |
| `t` | Next theme |

### AI & Advanced

| Key | Action |
|-----|--------|
| `a` | AI command input |
| `y` | YouTube search |
| `l` | Toggle lyrics panel |
| `K` | Toggle karaoke mode |
| `T` | Translate lyrics |
| `p` | Pomodoro timer |
| `P` | Plugin manager |
| `o` | Toggle offline mode |

### General

| Key | Action |
|-----|--------|
| `h` or `?` | Help overlay |
| `q` | Quit |
| `Ctrl+c` | Force quit |
| `:` | Command mode |

### Command Mode

Press `:` to enter command mode:

| Command | Description |
|---------|-------------|
| `quit`, `q` | Exit Symphony |
| `scan <path>` | Scan directory for music |
| `playlist create <name>` | Create playlist |
| `playlist delete <name>` | Delete playlist |
| `playlist shuffle` | Shuffle current playlist |
| `theme <name>` | Switch theme |
| `volume <0-100>` | Set volume |
| `output <device>` | Set audio output |
| `plugin list` | List plugins |
| `plugin enable <name>` | Enable plugin |
| `plugin disable <name>` | Disable plugin |

---

## Configuration

Configuration is stored in `~/.config/symphony/config.toml`.

### Complete Configuration Reference

```toml
# Symphony v2.0 Configuration
# ~/.config/symphony/config.toml

[general]
# Music library directory
music_directory = "~/Music"
# Cache for streaming and lyrics
cache_directory = "~/.cache/symphony"
# Database file location
database_path = "~/.local/share/symphony/library.db"
# UI theme (see themes/ directory)
theme = "monokai"
# Default view on startup: library, playlist, search
default_view = "library"

[audio]
# Audio output device ("default" for system default)
output_device = "default"
# Sample rate for playback
sample_rate = 44100
# Buffer size (lower = lower latency, higher CPU)
buffer_size = 2048
# Default volume (0.0 - 1.0)
volume = 0.6
# Normalize loudness across tracks
normalize_audio = false

[audio.effects]
# Enable equalizer
equalizer = false
# Enable reverb effect
reverb = false
# Bass boost multiplier (1.0 = none)
bass_boost = 1.0

[visualizer]
# Enable visualizations
enabled = true
# Visualization mode: spectrum, waveform, circular, minimal
mode = "spectrum"
# FFT size: 2048, 4096, 8192
fft_size = 4096
# Smoothing factor (0.0 - 1.0)
smoothing = 0.8
# Color scheme: rainbow, monochrome, heatmap, custom
color_scheme = "rainbow"
# Enable beat detection visual pulse
beat_detection = true
# Visual sensitivity (0.1 - 3.0)
sensitivity = 1.0

[ai]
# Enable AI features
enabled = true
# Provider: ollama (local) or openrouter (cloud)
provider = "ollama"
# Enable AI recommendations
recommendations_enabled = true
# Enable natural language commands
natural_language_commands = true
# Cache AI responses
cache_responses = true

# Ollama (Local AI) Configuration
[ai.ollama]
base_url = "http://localhost:11434"
model = "llama3.2:3b"

# OpenRouter (Cloud AI) Configuration
[ai.openrouter]
api_key = ""
model = "anthropic/claude-3.5-sonnet"
site_url = "https://github.com/symphony-player/symphony"
app_name = "Symphony Music Player"

[streaming]
# Enable streaming features
enabled = true

[streaming.youtube]
enabled = true
# Quality: low, medium, high, best
quality = "medium"
ytdlp_path = "yt-dlp"

[streaming.spotify]
enabled = false
client_id = ""
client_secret = ""
redirect_uri = "http://localhost:8888/callback"
quality = "high"

[streaming.cache]
enabled = true
cache_dir = "~/.cache/symphony/streams"
max_hot_size_mb = 100
max_warm_size_mb = 2048
max_cold_size_mb = 20480
prefetch_enabled = true
prefetch_count = 3

[streaming.network]
max_concurrent_downloads = 2
bandwidth_limit_mbps = 0  # 0 = unlimited
prefer_cached = true
timeout_secs = 30

[plugins]
enabled = true
plugin_dir = "~/.config/symphony/plugins"
auto_load = []

[plugins.discord]
enabled = false
show_timestamp = true
show_album_art = true

[plugins.lastfm]
enabled = false
api_key = ""
api_secret = ""
username = ""
scrobble_threshold = 50

[plugins.notifications]
enabled = false
on_track_change = true
on_playlist_end = true

[lyrics]
enabled = true
cache_dir = "~/.cache/symphony/lyrics"
auto_fetch = true
show_translation = false
karaoke_mode = true
context_lines = 2

[integrations.git]
enabled = true
adapt_music = false
check_interval = 60

[integrations.pomodoro]
enabled = false
work_minutes = 25
break_minutes = 5
long_break_minutes = 15
auto_start_break = false
auto_start_work = false
notifications = true

[integrations.statusline]
tmux_enabled = false
zsh_enabled = false
status_file = ""

[keybindings]
play_pause = "Space"
skip_next = "]"
skip_prev = "["
volume_up = "+"
volume_down = "-"
search = "/"
ai_command = "a"
toggle_lyrics = "l"
toggle_karaoke = "K"
translate_lyrics = "T"
pomodoro_start = "p"
pomodoro_break = "b"
```

### Themes

Symphony includes several built-in themes:

- `monokai` - Classic Monokai color scheme
- `gruvbox` - Warm, retro colors
- `nord` - Arctic, bluish colors
- `dracula` - Dark purple theme
- `solarized-dark` - Precision colors

Place custom themes in `~/.config/symphony/themes/`:

```toml
# ~/.config/symphony/themes/my-theme.toml
name = "my-theme"

[colors]
background = "#1a1a2e"
foreground = "#eaeaea"
accent = "#16c79a"
secondary = "#0f3460"
error = "#e94560"
warning = "#f39c12"
success = "#27ae60"

[ui]
border_style = "rounded"
selected_highlight = true
```

---

## Troubleshooting

### Common Issues

#### No Audio Output

**Symptoms**: Player appears to play but no sound is heard.

**Solutions**:
1. Check volume is not muted (`m` key to toggle mute)
2. Verify audio device is not in use by another application
3. On Linux, check ALSA/PulseAudio configuration:
   ```bash
   # List audio devices
   aplay -l        # ALSA
   pactl list sinks short  # PulseAudio
   
   # Test audio system
   speaker-test -t sine -f 440 -c 2
   ```
4. Try specifying output device in config:
   ```toml
   [audio]
   output_device = "hw:0,0"  # Or your specific device
   ```

#### AI Commands Not Working

**Symptoms**: AI features return errors or don't respond.

**Solutions**:
1. Verify Ollama is running:
   ```bash
   curl http://localhost:11434/api/tags
   ```
2. Check model is downloaded:
   ```bash
   ollama list
   ollama pull llama3.2:3b  # If not present
   ```
3. Check OpenRouter API key is valid (if using cloud AI)
4. Check configuration:
   ```toml
   [ai]
   enabled = true
   provider = "ollama"  # or "openrouter"
   ```

#### YouTube Streaming Fails

**Symptoms**: YouTube videos fail to load or play.

**Solutions**:
1. Update yt-dlp:
   ```bash
   pip install --upgrade yt-dlp
   # or
   brew upgrade yt-dlp
   ```
2. Check FFmpeg is installed:
   ```bash
   ffmpeg -version
   ```
3. Test yt-dlp directly:
   ```bash
   yt-dlp --simulate "https://youtube.com/watch?v=..."
   ```
4. Check network connectivity and firewall settings

#### Database Errors

**Symptoms**: Library doesn't load, search fails.

**Solutions**:
1. Rebuild the database:
   ```bash
   symphony scan --rebuild ~/Music
   ```
2. Check database file permissions:
   ```bash
   ls -la ~/.local/share/symphony/library.db
   ```
3. Reset database (WARNING: loses all metadata):
   ```bash
   rm ~/.local/share/symphony/library.db
   symphony scan ~/Music
   ```

#### Plugin Loading Errors

**Symptoms**: Plugins fail to load or crash.

**Solutions**:
1. Check plugin is compatible with your Symphony version
2. Verify WASM file is valid:
   ```bash
   wasm-validate plugin.wasm
   ```
3. Check plugin permissions in manifest
4. Check logs for detailed error:
   ```bash
   symphony --verbose 2>&1 | grep plugin
   ```

#### Slow Performance

**Symptoms**: UI lags, high CPU usage.

**Solutions**:
1. Reduce FFT size:
   ```toml
   [visualizer]
   fft_size = 2048
   ```
2. Disable visualizations temporarily (`v` key)
3. Reduce cache sizes:
   ```toml
   [streaming.cache]
   max_hot_size_mb = 50
   ```
4. Check for memory leaks:
   ```bash
   valgrind --leak-check=full symphony
   ```

### Getting Help

1. **Documentation**: Check this guide and [API.md](API.md)
2. **GitHub Issues**: https://github.com/symphony-player/symphony/issues
3. **Discord**: https://discord.gg/symphony-player
4. **Debug Mode**: Run with verbose logging:
   ```bash
   symphony --verbose
   symphony --debug
   ```

### Reporting Bugs

When reporting bugs, please include:

1. Symphony version (`symphony --version`)
2. Operating system and version
3. Steps to reproduce
4. Expected behavior
5. Actual behavior
6. Logs (use `--verbose` flag)

```bash
symphony --verbose 2>&1 | tee symphony-debug.log
```

---

## Appendix

### Supported Audio Formats

| Format | Extension | Metadata Support |
|--------|-----------|-----------------|
| MP3 | .mp3 | ID3v1, ID3v2 |
| FLAC | .flac | Vorbis Comments |
| AAC/M4A | .m4a, .aac | iTunes Tags |
| OGG Vorbis | .ogg | Vorbis Comments |
| WAV | .wav | Limited |
| Opus | .opus | Vorbis Comments |

### Environment Variables

| Variable | Description |
|----------|-------------|
| `SYMPHONY_CONFIG_DIR` | Override config directory |
| `SYMPHONY_DATA_DIR` | Override data directory |
| `SYMPHONY_CACHE_DIR` | Override cache directory |
| `SYMPHONY_LOG_LEVEL` | Set log level (trace, debug, info, warn, error) |
| `SYMPHONY_NO_COLOR` | Disable colored output |

### File Locations

```
~/.config/symphony/
├── config.toml          # Main configuration
├── themes/              # Custom themes
│   └── custom.toml
└── plugins/             # WASM plugins
    └── my-plugin.wasm

~/.local/share/symphony/
├── library.db           # SQLite database
└── embeddings/          # Audio embeddings

~/.cache/symphony/
├── streams/             # Cached streaming audio
└── lyrics/              # Cached lyrics
```

---

*Symphony v2.0 - Built with ❤️ for developers who love music*
