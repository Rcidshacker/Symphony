# 🎵 Symphony v2.0

<p align="center">
  <strong>The AI-Powered Terminal Music Player for Developers</strong>
</p>

<p align="center">
  <em>"Your AI-Powered Terminal Concert Hall"</em>
</p>

<p align="center">
  <a href="#features">Features</a> •
  <a href="#installation">Installation</a> •
  <a href="#quick-start">Quick Start</a> •
  <a href="#documentation">Documentation</a> •
  <a href="#contributing">Contributing</a>
</p>

<p align="center">
  <a href="https://www.rust-lang.org/">
    <img src="https://img.shields.io/badge/rust-1.75%2B-orange.svg?style=flat-square" alt="Rust Version">
  </a>
  <a href="https://github.com/Rcidshacker/Symphony/blob/main/LICENSE">
    <img src="https://img.shields.io/badge/license-MIT-blue.svg?style=flat-square" alt="License">
  </a>
  <a href="https://github.com/Rcidshacker/Symphony/stargazers">
    <img src="https://img.shields.io/github/stars/Rcidshacker/Symphony?style=flat-square" alt="GitHub Stars">
  </a>
  <a href="https://github.com/Rcidshacker/Symphony/actions">
    <img src="https://img.shields.io/github/actions/workflow/status/Rcidshacker/Symphony/ci.yml?style=flat-square" alt="CI Status">
  </a>
  <a href="https://codecov.io/gh/Rcidshacker/Symphony">
    <img src="https://img.shields.io/codecov/c/github/Rcidshacker/Symphony?style=flat-square" alt="Coverage">
  </a>
</p>

---

## Preview

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
│  📚 LIBRARY                                                               │
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

---

## Why Symphony?

**The Problem**: Existing terminal music players are basic. They play music, but don't understand you or your workflow.

**The Solution**: Symphony is the first terminal music player with native AI integration, hybrid streaming, and developer-focused features—all while being lightning fast and privacy-respecting.

### Key Differentiators

| Feature | Symphony | Spotify CLI | cmus | moc |
|---------|----------|-------------|------|-----|
| AI-Powered Control | ✅ | ❌ | ❌ | ❌ |
| YouTube Streaming | ✅ | ❌ | ❌ | ❌ |
| Spotify Integration | ✅ | ✅ | ❌ | ❌ |
| Real-time Visualizer | ✅ | ❌ | ❌ | ❌ |
| Plugin System | ✅ | ❌ | ❌ | ❌ |
| Synced Lyrics | ✅ | ❌ | ❌ | ❌ |
| Git Integration | ✅ | ❌ | ❌ | ❌ |
| Pomodoro Timer | ✅ | ❌ | ❌ | ❌ |
| Local AI (Private) | ✅ | ❌ | ❌ | ❌ |

---

## Features

### 🤖 AI-Powered Music Intelligence

Natural language control powered by local LLMs:

```bash
# Control with natural language
symphony "play something energetic for coding"
symphony "find songs like Radiohead but jazzier"
symphony "create a 2-hour focus session"

# AI understands context
symphony "what should I listen to while debugging?"
```

**AI Features**:
- Natural language commands
- Smart playlist generation
- Context-aware recommendations
- Semantic search across your library
- Mood-based music selection

### 🌐 Hybrid Playback

All your music sources in one interface:

- **Local Library**: MP3, FLAC, AAC, OGG, WAV support
- **YouTube**: Stream any video as audio
- **Spotify**: Access your playlists and liked songs
- **Smart Caching**: Predictive downloads for offline access

```bash
# Play from YouTube
symphony stream "lofi hip hop radio"

# Mix sources seamlessly
symphony add youtube:https://youtube.com/watch?v=...
symphony add spotify:playlist:37i9dQZF1DXcBWIGoYBM5M
```

### 🎨 Stunning Visualizations

Real-time FFT spectrum analyzer with multiple modes:

- **Spectrum Bars**: Classic frequency visualization
- **Waveform**: Audio waveform display
- **Circular**: Circular frequency visualization
- **Minimal**: Simple visual indicators

All running at 60 FPS with customizable themes.

### 🔌 Extensible Plugin System

WASM-based plugins for unlimited extensibility:

- **Discord Rich Presence**: Show what you're listening to
- **Last.fm Scrobbler**: Track your listening history
- **Custom Plugins**: Create your own in Rust, C, or AssemblyScript

### 🎤 Synced Lyrics

Karaoke-style lyrics display:

- LRC file support with synced timestamps
- Auto-fetch lyrics from online sources
- Translation support
- Word-level highlighting

### 💻 Developer-Focused

Built by developers, for developers:

- **Git Integration**: Music adapts to commit velocity
- **Pomodoro Timer**: Built-in focus sessions
- **Status Line**: Tmux and zsh integration
- **SSH-Friendly**: Works perfectly over remote connections

---

## Installation

### Prerequisites

**System Dependencies**:

Linux (Ubuntu/Debian):
```bash
sudo apt install -y libasound2-dev libpulse-dev pkg-config build-essential
```

macOS:
```bash
brew install portaudio pkg-config
```

**Optional Dependencies**:
```bash
# For YouTube streaming
pip install yt-dlp

# For FFmpeg support
brew install ffmpeg  # macOS
sudo apt install ffmpeg  # Linux
```

### From Source (Recommended)

```bash
# Clone the repository
git clone https://github.com/Rcidshacker/Symphony.git
cd symphony

# Build in release mode
cargo build --release

# Run
./target/release/symphony
```

### Via Cargo

```bash
cargo install symphony-player
```

### Via Homebrew (macOS/Linux)

```bash
brew tap symphony-player/tap
brew install symphony
```

### Via AUR (Arch Linux)

```bash
yay -S symphony-bin
```

---

## Quick Start

### 1. Initialize

```bash
symphony init
```

Creates configuration files at `~/.config/symphony/`.

### 2. Scan Your Library

```bash
symphony scan ~/Music
```

### 3. Start Playing

```bash
# Interactive mode
symphony

# Or with AI
symphony "play some indie rock"
```

### 4. Essential Keybindings

| Key | Action |
|-----|--------|
| `Space` | Play/Pause |
| `n` / `]` | Next track |
| `p` / `[` | Previous track |
| `+` / `-` | Volume up/down |
| `/` | Search |
| `a` | AI command |
| `v` | Cycle visualizer |
| `l` | Toggle lyrics |
| `h` | Help |
| `q` | Quit |

---

## AI Setup

### Local AI (Ollama) - Recommended

```bash
# Install Ollama
curl -fsSL https://ollama.com/install.sh | sh

# Start Ollama
ollama serve

# Download a model
ollama pull gemma3:4b
```

Configure Symphony (`~/.config/symphony/config.toml`):
```toml
[ai]
enabled = true
provider = "ollama"

[ai.ollama]
model = "gemma3:4b"
```

### Cloud AI (OpenRouter)

```toml
[ai]
enabled = true
provider = "openrouter"

[ai.openrouter]
api_key = "your-api-key"
model = "anthropic/claude-3.5-sonnet"
```

---

## Documentation

| Document | Description |
|----------|-------------|
| [User Guide](docs/USER_GUIDE.md) | Complete user documentation |
| [Configuration](docs/CONFIGURATION.md) | Configuration reference |
| [API Reference](docs/API.md) | Plugin API documentation |
| [Plugin Development](docs/PLUGIN_DEVELOPMENT.md) | Creating plugins |
| [Troubleshooting](docs/TROUBLESHOOTING.md) | Common issues and solutions |

---

## Project Structure

```
symphony/
├── src/
│   ├── main.rs           # Entry point
│   ├── app.rs            # Application state
│   ├── audio/            # Audio playback engine
│   │   ├── engine.rs     # Core playback
│   │   └── analyzer.rs   # FFT analysis
│   ├── ui/               # Terminal UI
│   │   ├── visualizer.rs # Visualizations
│   │   └── artwork.rs    # Album art
│   ├── ai/               # AI integration
│   │   ├── ollama.rs     # Local LLM
│   │   ├── openrouter.rs # Cloud AI
│   │   └── recommendations.rs
│   ├── streaming/        # Streaming sources
│   │   ├── youtube.rs
│   │   ├── spotify.rs
│   │   └── cache.rs
│   ├── plugins/          # Plugin system
│   │   ├── runtime.rs    # WASM runtime
│   │   └── api.rs        # Host functions
│   ├── lyrics/           # Lyrics system
│   ├── integrations/     # Developer features
│   │   ├── git.rs
│   │   └── pomodoro.rs
│   ├── config/           # Configuration
│   └── db/               # Database
├── themes/               # Color themes
├── config/               # Default config
├── tests/                # Test suites
└── benches/              # Benchmarks
```

---

## Performance

Symphony is optimized for performance:

| Metric | Target | Actual |
|--------|--------|--------|
| Startup time | < 1s | ~500ms |
| Memory (idle) | < 50MB | ~35MB |
| Memory (playing) | < 200MB | ~120MB |
| CPU (playback) | < 5% | ~2-3% |
| FFT latency | < 5ms | ~2ms |
| Binary size | < 15MB | ~12MB |

---

## Development Status

| Phase | Status | Description |
|-------|--------|-------------|
| Phase 1: Foundation | ✅ Complete | Core playback + UI |
| Phase 2: Visuals | ✅ Complete | FFT analyzer + Themes |
| Phase 3: AI | ✅ Complete | LLM + Embeddings |
| Phase 4: Streaming | ✅ Complete | YouTube + Spotify |
| Phase 5: Advanced | ✅ Complete | Plugins + Lyrics |
| Phase 6: Polish | ✅ Complete | Docs + Release |

---

## Contributing

We welcome contributions! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for details.

### Development

```bash
# Clone and build
git clone https://github.com/Rcidshacker/Symphony.git
cd symphony
cargo build

# Run tests
cargo test

# Run with debug logging
cargo run -- --verbose

# Format code
cargo fmt

# Lint
cargo clippy
```

### Roadmap

- [ ] Mobile companion app
- [ ] Collaborative listening sessions
- [ ] Music quiz game mode
- [ ] Built-in equalizer
- [ ] MIDI controller support
- [ ] Voice control

---

## License

Symphony is licensed under the [MIT License](LICENSE).

---

## Acknowledgments

Built with amazing open-source projects:

- [Ratatui](https://github.com/ratatui-org/ratatui) - Terminal UI framework
- [Rodio](https://github.com/RustAudio/rodio) - Audio playback
- [RustFFT](https://github.com/ejmahler/RustFFT) - FFT implementation
- [Ollama](https://ollama.ai) - Local LLM runtime
- [yt-dlp](https://github.com/yt-dlp/yt-dlp) - YouTube downloader

---

## Community

- **GitHub**: https://github.com/Rcidshacker/Symphony
- **Discord**: https://discord.gg/symphony-player
- **Twitter**: @symphony_player

---

<p align="center">
  <strong>Built with ❤️ for developers who love music</strong>
</p>
