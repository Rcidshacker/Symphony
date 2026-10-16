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
README/CHANGELOG overclaim. Verified by reading code:
- `main.rs` parses no CLI args (`symphony init|scan|stream|add|"play ..."` do not exist)
- `ai/`, `streaming/`, `plugins/`, `lyrics/`, `integrations/` compile but are not called from `app.rs` or `ui/` (only `config` references AI provider types)
- `wasmtime` and `rspotify` are commented out in `Cargo.toml`; `plugins/runtime.rs` is a simulation
- `lyrics/fetcher.rs` online fetch and `ui/artwork.rs` image rendering are placeholders
- No crossfade, gapless, or loudness normalization in `audio/engine.rs`; the visualizer is not fed audio from `analyzer.rs`
- README performance numbers have no benchmark results behind them
Do not describe a feature as working until it is wired from `main.rs`/`app.rs` and tested.

## Project Structure
- `src/main.rs`, `src/app.rs` — entry point, event loop, app state
- `src/audio/` — rodio playback engine, FFT analyzer
- `src/ui/` — ratatui rendering, visualizer, artwork
- `src/config/` — config + themes (`themes/*.toml`, `config/symphony.toml`)
- `src/db/` — SQLite library (inline migrations in `run_migrations`)
- `src/scanner/` — library scan
- `src/ai/`, `src/streaming/`, `src/plugins/`, `src/lyrics/`, `src/integrations/` — feature modules (see state above)
- `tests/` integration tests, `benches/` criterion, `docs/` user docs
- `.claude/worktrees/` — harness-created git worktree, ignore
