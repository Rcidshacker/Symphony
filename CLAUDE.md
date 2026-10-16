# Project Brain

## Stack
Rust 2021 (MSRV 1.82) terminal music player. Ratatui + crossterm (TUI), rodio + symphonia (audio), rustfft/realfft (FFT), tokio + reqwest (async/HTTP), rusqlite bundled (SQLite), wasmi (WASM plugins), serde/toml (config), thiserror + anyhow (errors), tracing (logs). Tests: built-in `cargo test`; benches with criterion. CI: `.github/workflows/ci.yml` (fmt, clippy `-D warnings`, test on ubuntu/macos/windows, tarpaulin).

## Commands
- Build: `cargo build` | `cargo build --release`
- Run: `cargo run` (TUI) or `cargo run -- <command>`; `symphony --help` lists commands
- Test: `cargo test`
- Lint: `cargo clippy --all-targets --all-features -- -D warnings`
- Format: `cargo fmt --all`
- Bench: `cargo bench`
- Perf probe: `cargo run --release --example perf_probe -- <audio file> [seconds]`
- Release: push a `vX.Y.Z` tag (triggers `release.yml`)

## Conventions
- Rust 2021, rustfmt defaults, zero clippy warnings (CI enforces `-D warnings`; currently clean)
- Typed errors with `thiserror` in modules, `anyhow` at the binary edge
- No `unwrap()`/`expect()` on user input, file, network, or DB paths
- Tests never need the network or a real service: mock servers and `examples/fake_ytdlp.rs` live in `tests/common/`. Playback tests need an audio device and return early without one; they play muted
- Never modify files outside the task scope
- Commit only when asked

## Real state (verified by tests; keep this section honest)
Wired from `main.rs`/`app.rs` and covered by integration tests:
- CLI (`src/cli.rs`): `init`, `scan`, `add`, `stream`, `status`, `lastfm-login`, `"<prompt>"`
- Playback: tags, duration, progress, volume, mute, shuffle, repeat, MP3/FLAC/OGG/WAV/AAC/M4A (symphonia fallback in `audio/symphonia_source.rs`). No seeking, crossfade, gapless or loudness normalization
- Visualizer fed by the played audio (`audio/tap.rs` -> `analyzer.rs`); album art from tags or sidecar images (`ui/artwork.rs`)
- AI assistant (`app_assistant.rs`, `ai/`): play by mood/tempo from measured audio features (`audio/features.rs`), playlists, suggestions, semantic search (embeddings with lexical fallback), questions. Ollama or OpenRouter
- Streaming (`app_stream.rs`, `streaming/`): YouTube via `yt-dlp`, Spotify metadata + public playlists (audio from a YouTube match), M4A cache, prefetch
- Lyrics (`lyrics/`): `.lrc`, LRCLIB lookup, word highlighting, translation via the AI model
- Plugins (`app_plugins.rs`, `plugins/`): wasmi runtime with permissions, metering and a memory cap; Discord Rich Presence and Last.fm are built in (`integrations/`)
- Developer features: `symphony status` + status file (`integrations/statusline.rs`), git-adaptive queue (`integrations/git.rs`, `App::adapt_to_activity`), Pomodoro

Never exercised against the real service (mock servers only): Ollama, OpenRouter, real `yt-dlp`, Spotify API, Discord, Last.fm. LRCLIB was also run live once. Developed and tested on Windows; Linux/macOS are built by CI only.
Not built: Spotify user login (liked songs, private playlists), plugin SDK/marketplace, seeking, crossfade, gapless, normalization.
`docs/*.md` predate the audit and may describe features that do not exist; README.md and this file win. Do not describe a feature as working until it is wired and tested.

## Project Structure
- `src/main.rs`, `src/app.rs` (+ `app_assistant.rs`, `app_stream.rs`, `app_plugins.rs`) — entry point, event loop, app state
- `src/audio/` — rodio engine, symphonia source, FFT analyzer, audio tap, feature extraction
- `src/ui/` — ratatui rendering, visualizer, artwork
- `src/config/` — config + themes (`themes/*.toml`, `config/symphony.toml`)
- `src/db/` — SQLite library (inline migrations in `run_migrations`)
- `src/scanner/` — library scan
- `src/input.rs` — key handling (testable without a terminal); `src/cli.rs` — argument parsing
- `src/ai/`, `src/streaming/`, `src/plugins/`, `src/lyrics/`, `src/integrations/` — feature modules
- `tests/` integration tests (shared helpers in `tests/common/`), `benches/` criterion, `examples/` fake yt-dlp + perf probe, `docs/` user docs
- `.claude/worktrees/` — harness-created git worktree, ignore
