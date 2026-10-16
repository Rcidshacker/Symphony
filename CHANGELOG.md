# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

Everything listed under 2.0.0 in earlier revisions of this file was announced before it was
built. This release is the first where each item below is wired into the player and tested.

### Added
- `symphony` CLI: `init`, `scan`, `add`, `stream`, `status`, `lastfm-login`, and `symphony "<prompt>"`
- AI assistant (`a` key and command line): play by mood and tempo from measured audio features,
  playlists, suggestions, semantic search (Ollama embeddings with lexical fallback), questions
- Streaming: YouTube through `yt-dlp` and Spotify (public playlists and tracks, audio from the
  closest YouTube match), cached as M4A for instant replays and offline playback, with prefetch
- Real WASM plugin runtime (`wasmi`) with permissions, execution metering and a memory cap
- Discord Rich Presence over local IPC; Last.fm now playing and scrobbling
- `symphony status` for tmux, zsh, waybar, i3blocks, polybar and JSON
- Git-adaptive music (`integrations.git.adapt_music`)
- Lyrics: LRCLIB lookup, word-level highlighting, translation through the local model
- Album art from embedded covers or sidecar images; M4A/MP4 decoding
- Visualizer fed by the audio being played (bars, waveform, circular, minimal)
- Integration tests with mock Ollama, LRCLIB, Spotify and Last.fm servers, a fake `yt-dlp`
  and ratatui's `TestBackend`

### Changed
- Minimum Rust version is 1.82
- Status line truncation no longer panics on multi-byte titles
- Fixed the FFT peak search, Windows double key presses (key release events), volume, progress
  and shuffle handling

### Removed (announced earlier, never built)
- Crossfade, gapless playback, EBU R128 normalization, seeking
- Sixel album art, bandwidth-adaptive quality
- "Official" plugin installer and the desktop-notifications plugin
- Homebrew, AUR and crates.io packages, Discord and Twitter community links

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
| Unreleased | - | AI assistant, streaming, WASM plugins, Discord, Last.fm, status line, lyrics |
| 1.0.0 | 2023-06-01 | Initial release |
