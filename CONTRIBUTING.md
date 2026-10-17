# Contributing to Symphony

Thanks for helping. Bug reports with a reproduction, fixes and docs corrections are all welcome. Open an issue first for anything big so the direction can be agreed before you write it.

## Set up

You need Rust 1.88 or newer (the oldest compiler the locked dependencies build on; CI checks it) and Git. On Linux also install `libasound2-dev pkg-config build-essential`.

```bash
git clone https://github.com/Rcidshacker/Symphony.git
cd Symphony
cargo build
cargo test
```

## Before you open a pull request

CI runs exactly these, so run them locally:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

CI runs the tests on Linux, macOS and Windows. The playback tests need an audio output device and return early without one, so a green run on a headless machine does not exercise playback.

## Where things are

| Path | What |
|------|------|
| `src/main.rs`, `src/cli.rs` | Entry point and command-line parsing |
| `src/app.rs`, `src/app_assistant.rs`, `src/app_stream.rs`, `src/app_plugins.rs` | App state and the AI, streaming and plugin hooks |
| `src/input.rs` | Key handling, testable without a terminal |
| `src/audio/` | Playback engine, spectrum analyzer, feature extraction |
| `src/ui/` | Rendering, visualizer, album art |
| `src/ai/`, `src/streaming/`, `src/lyrics/`, `src/plugins/`, `src/integrations/` | Feature modules |
| `src/config/` | Config file and themes (`themes/*.toml`) |
| `tests/` | Integration tests; helpers and mock servers in `tests/common/` |
| `examples/` | `fake_ytdlp` (a stand-in for yt-dlp used by tests) and `perf_probe` |
| `benches/` | Criterion benchmarks |

## Conventions

* rustfmt defaults and zero clippy warnings.
* Typed errors with `thiserror` in modules, `anyhow` only at the binary edge.
* No `unwrap()` / `expect()` on user input, files, network or database paths.
* Keep changes focused; do not reformat or refactor code the change does not need.
* Commit messages: a short imperative summary line ("Fix ...", "Add ..."), details in the body if needed.

## Tests

* Add a test with every fix or feature. Tests must not need the network or a real service: use the mock servers in `tests/common/` and the fake yt-dlp. A feature that talks to an outside service is "tested" only against a mock until someone has run it live; say which it is in the pull request.
* `cargo test` builds the examples the tests need. If you run a single test file (`cargo test --test stream_tests`) after changing `examples/fake_ytdlp.rs`, run `cargo build --example fake_ytdlp` first.
* Do not claim a feature in the README, docs or changelog until it is wired into the app and tested. `docs/` describes only what exists.

## Documentation

User-facing changes update `README.md`, the matching file in `docs/`, and `CHANGELOG.md`. The plugin example in `docs/PLUGIN_DEVELOPMENT.md` is compiled and run by a test, so keep it working.

## Releases

Pushing a tag like `v1.2.3` runs `.github/workflows/release.yml`.

## License

By contributing you agree that your contributions are licensed under the MIT License, the same as the project.
