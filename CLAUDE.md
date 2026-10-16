# Project Brain

## Stack
Rust 2021 terminal music player. Ratatui + crossterm (TUI), rodio + symphonia (audio), rustfft/realfft (FFT), tokio + reqwest (async/HTTP), rusqlite bundled (SQLite), serde/toml (config), thiserror + anyhow (errors), tracing (logs). Tests: built-in `cargo test`, benches with criterion. CI: `.github/workflows/ci.yml` (fmt, clippy `-D warnings`, test, tarpaulin).

## Commands
- Build: `cargo build` | `cargo build --release`
- Run: `cargo run` (TUI; no CLI args are parsed)
- Test: `cargo test`
- Lint: `cargo clippy --all-targets --all-features -- -D warnings`
- Format: `cargo fmt --all`
- Bench: `cargo bench`
- Release: push a `vX.Y.Z` tag (triggers `release.yml`)

## Conventions
- Rust 2021, rustfmt defaults, zero clippy warnings (CI enforces `-D warnings`)
- Typed errors with `thiserror` in modules, `anyhow` at the binary edge
- No `unwrap()`/`expect()` on user input, file, network, or DB paths
- Never modify files outside the task scope
- Commit only when asked

## Real state vs README (read before trusting docs)
README/CHANGELOG overclaim. Verified by reading code and tests:
- Wired and tested: CLI (`src/cli.rs`: init, scan, stream, add, "<prompt>"), local playback with tags/duration/progress/shuffle, AI prompt (`a`, async bridge in `app.rs`, acts on Play/Search/Skip/SetState), local `.lrc` lyrics (`l`), Pomodoro (`w`) and git status in the status bar. Keys live in `src/input.rs`.
- AI was only tested against an unreachable provider; never against a live Ollama/OpenRouter model. Mood/tempo/era from the AI are not matched against local files (only artist/genre/title)
- `streaming/` is only reachable from the `stream` CLI search; `add youtube:`/`spotify:` and playing streams from the TUI are not implemented
- `wasmtime` and `rspotify` are commented out in `Cargo.toml`; `plugins/` is a simulation and is not called from the app
- `lyrics/fetcher.rs` online fetch and `ui/artwork.rs` image rendering are placeholders; lyrics come only from a `.lrc` next to the audio file
- No crossfade, gapless, or loudness normalization in `audio/engine.rs`; the visualizer is not fed audio from `analyzer.rs`; no seeking
- README performance numbers have no benchmark results behind them
Do not describe a feature as working until it is wired from `main.rs`/`app.rs` and tested.

## Project Structure
- `src/main.rs`, `src/app.rs` — entry point, event loop, app state
- `src/audio/` — rodio playback engine, FFT analyzer
- `src/ui/` — ratatui rendering, visualizer, artwork
- `src/config/` — config + themes (`themes/*.toml`, `config/symphony.toml`)
- `src/db/` — SQLite library (inline migrations in `run_migrations`)
- `src/scanner/` — library scan
- `src/input.rs` — key handling (testable without a terminal); `src/cli.rs` — argument parsing
- `src/ai/`, `src/streaming/`, `src/plugins/`, `src/lyrics/`, `src/integrations/` — feature modules (see state above)
- `tests/` integration tests, `benches/` criterion, `docs/` user docs
- `.claude/worktrees/` — harness-created git worktree, ignore
