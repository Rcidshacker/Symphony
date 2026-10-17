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
- Minimum Rust version is 1.88 (verified: the first draft said 1.82, but wasmi and other locked dependencies need 1.88); CI now builds on it
- Key `5` turns the visualizer off, as the in-app help always said
- `symphony stream` skips livestreams and results over 20 minutes when picking the default, so a 12-hour loop is no longer "the song"
- An open question the AI files as a suggestion with nothing to base it on is answered in the popup instead of showing an unrelated mix
- CI: the Security Audit job gets the token permissions it needs; the coverage job builds the fake yt-dlp the stream tests use; the visualizer test no longer counts the panel title and tolerates slow sound cards
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
