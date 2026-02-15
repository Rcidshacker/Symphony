# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [2.0.0] - 2024-01-15

### Added

#### AI Integration
- Local LLM support via Ollama integration
- Cloud AI support via OpenRouter
- Natural language music control ("play something energetic for coding")
- AI-powered smart playlist generation
- Semantic search across music library
- Context-aware recommendations
- Music intent parsing and entity extraction

#### Streaming
- YouTube integration via yt-dlp
- Spotify Web API integration
- 3-tier smart caching system (Hot/Warm/Cold)
- Predictive prefetching for smooth playback
- Adaptive quality based on bandwidth
- Offline mode with cached content

#### Visualizations
- Real-time FFT spectrum analyzer
- 4 visualization modes (Spectrum, Waveform, Circular, Minimal)
- Beat detection with visual pulse
- Album art rendering (ASCII and Sixel)
- 5 built-in themes (Monokai, Gruvbox, Nord, Dracula, Solarized)
- Custom theme support

#### Plugin System
- WASM-based plugin runtime (Wasmtime)
- Plugin permission system
- Host function API for plugin interaction
- Official plugins:
  - Discord Rich Presence
  - Last.fm Scrobbler
  - Desktop Notifications

#### Lyrics
- LRC file parsing with synced timestamps
- Online lyrics fetching
- Karaoke-style highlighting
- Translation support
- Word-level timing

#### Developer Integrations
- Git activity monitoring
- Activity-based music adaptation
- Pomodoro timer with work/break sessions
- Tmux and zsh status line integration

#### Core Features
- SQLite database for library management
- Multi-format support (MP3, FLAC, AAC, OGG, WAV)
- Audio normalization (EBU R128)
- Gapless playback
- Crossfade support
- Keyboard-driven interface
- Vim-style navigation

### Changed
- Complete rewrite from v1.x
- New UI framework (Ratatui)
- Improved audio engine with lower latency
- Better memory management
- Optimized FFT processing

### Performance
- < 1 second cold startup
- < 50MB idle memory usage
- < 200MB active memory usage
- < 5% CPU during playback
- < 5ms FFT processing time

## [1.0.0] - 2023-06-01

### Added
- Basic terminal music player
- MP3 and FLAC support
- Simple playlist management
- Basic keyboard controls
- Library scanning

---

## Version History

| Version | Date | Highlights |
|---------|------|------------|
| 2.0.0 | 2024-01-15 | AI integration, streaming, plugins, visualizer |
| 1.0.0 | 2023-06-01 | Initial release |

---

## Upgrade Guide

### Upgrading from 1.x to 2.0

Version 2.0 is a complete rewrite with many new features. Here's how to upgrade:

1. **Backup your data**:
   ```bash
   cp -r ~/.local/share/symphony ~/.local/share/symphony-backup
   ```

2. **Install new version**:
   ```bash
   cargo install symphony-player
   ```

3. **Initialize new config**:
   ```bash
   symphony init
   ```

4. **Rescan library**:
   ```bash
   symphony scan ~/Music
   ```

**Note**: The database format has changed. Your play history and playlists will need to be recreated.

### Configuration Changes

The configuration format has changed significantly. Key differences:

- `[ai]` section is new for AI configuration
- `[streaming]` section is new for streaming configuration
- `[plugins]` section is new for plugin configuration
- Keybindings format has changed

Use `symphony init` to generate a new default config.

---

## Roadmap

### Planned for 2.1
- Built-in equalizer with presets
- More streaming sources (SoundCloud, Bandcamp)
- Collaborative playlists
- Music quiz mode

### Planned for 2.2
- Mobile companion app
- Voice control
- MIDI controller support
- LAN sharing mode

### Planned for 3.0
- Web interface
- Multi-user support
- Advanced audio effects
- Plugin marketplace

---

[2.0.0]: https://github.com/symphony-player/symphony/compare/v1.0.0...v2.0.0
[1.0.0]: https://github.com/symphony-player/symphony/releases/tag/v1.0.0
