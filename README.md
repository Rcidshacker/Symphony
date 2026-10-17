# 🎵 Symphony

<p align="center">
  <strong>A terminal music player with an AI assistant, streaming, synced lyrics and a real spectrum analyzer.</strong>
</p>

<p align="center">
  <a href="#features">Features</a> •
  <a href="#installation">Installation</a> •
  <a href="#quick-start">Quick Start</a> •
  <a href="#what-is-not-done-or-not-verified">Limits</a> •
  <a href="#contributing">Contributing</a>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/rust-1.88%2B-orange.svg?style=flat-square" alt="Rust 1.88+">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg?style=flat-square" alt="License"></a>
  <a href="https://github.com/Rcidshacker/Symphony/actions"><img src="https://img.shields.io/github/actions/workflow/status/Rcidshacker/Symphony/ci.yml?style=flat-square" alt="CI Status"></a>
</p>

---

## Features

Every feature below is wired into the player and covered by automated tests. How each was
verified (mock servers, fake `yt-dlp`, or the real thing) is in [What is not done or not verified](#what-is-not-done-or-not-verified).

### 🎧 Playback

- MP3, FLAC, OGG/Vorbis, WAV, AAC and M4A, with tags, duration, progress bar, volume, mute, shuffle and repeat
- Album art from embedded covers or `cover.jpg`/`folder.png` next to the file (JPEG/PNG, drawn with block characters)
- Library kept in SQLite (`symphony scan <dir>`), plus search (`/`)

### 🤖 AI assistant

Press `a` in the player, or pass a prompt on the command line. It needs a running
[Ollama](https://ollama.com) or an OpenRouter key.

```bash
symphony "play something calm"
symphony "create a 2-hour focus session"
symphony "more like Gym Anthem"
symphony "what should I listen to while debugging?"
```

- **Play by mood**: mood and tempo are matched against what Symphony measured from your audio (loudness and tempo per track, analysed in the background), not against file names
- **Playlists**: time-boxed playlists, saved in the library database
- **Suggestions** based on a track, **semantic search** by meaning (embeddings via Ollama, lexical fallback), and **questions** answered with your library as context

### 🌐 YouTube

Requires [`yt-dlp`](https://github.com/yt-dlp/yt-dlp) on your `PATH` (or set `streaming.youtube.ytdlp_path`).

```bash
symphony stream "lofi hip hop"                 # search and play the best match
symphony add youtube:https://youtube.com/watch?v=...   # save to the library
```

- Stream tracks are downloaded to a local cache (as M4A) and played from there, so replays and the prefetched next track start instantly and work offline

### 🎨 Visualizer

Live FFT spectrum of the audio actually being played: bars, waveform, circular and minimal modes (`v` cycles, `1`–`4` select a mode and `5` turns it off). Five themes (`t`): Monokai, Gruvbox, Nord, Dracula, Solarized.

### 🎤 Lyrics

- Synced `.lrc` files next to the audio, with word-level highlighting for enhanced LRC
- Online lookup on [LRCLIB](https://lrclib.net) when no `.lrc` exists
- Translation of the current lyrics through your local model (`x`, target language in `lyrics.translate_to`)

### 🔌 Plugins, Discord, Last.fm

- **WASM plugins** run in a sandbox ([`wasmi`](https://github.com/wasmi-labs/wasmi)): metered execution (runaway plugins are stopped), 16 MB memory cap, and host functions gated by the permissions in the plugin's `plugin.toml`. See [Plugin Development](docs/PLUGIN_DEVELOPMENT.md)
- **Discord Rich Presence** (`[plugins.discord]`): create an application at the Discord developer portal and set its Application ID as `client_id`
- **Last.fm scrobbling** (`[plugins.lastfm]`): set `api_key` and `api_secret`, then run `symphony lastfm-login`

### 💻 Developer touches

- **Status line**: `symphony status [tmux|zsh|json|waybar|i3blocks|polybar|minimal]` prints what the running player is playing. In tmux: `set -g status-right '#(symphony status tmux)'`
- **Git-adaptive music**: with `integrations.git.adapt_music = true`, the queue switches to calmer or more energetic music at the next track change as your commits per hour change
- **Pomodoro timer** (`w`) and git activity in the status bar

---

## Installation

Symphony is built from source. There is no crates.io package, Homebrew tap or AUR package.

**Requirements**: Rust 1.88+ (the oldest compiler the locked dependencies build on). On Linux: `sudo apt install -y libasound2-dev pkg-config build-essential`.

```bash
git clone https://github.com/Rcidshacker/Symphony.git
cd Symphony
cargo build --release
./target/release/symphony
```

Optional: `yt-dlp` for YouTube playback, [Ollama](https://ollama.com) for the AI assistant.

---

## Quick Start

```bash
symphony init            # create the config and database
symphony scan ~/Music    # add your music
symphony                 # open the player
```

| Key | Action |
|-----|--------|
| `Space` | Play/Pause |
| `n` / `]`, `p` / `[` | Next / previous track |
| `+` / `-`, `m` | Volume up/down, mute |
| `s`, `r` | Shuffle, repeat |
| `j` / `k`, `Enter` | Move, play |
| `/` | Search |
| `a` | AI assistant |
| `l`, `x` | Lyrics, translate lyrics |
| `v`, `1`–`5`, `t` | Visualizer mode, theme |
| `w` | Pomodoro |
| `h` / `?` | Help |
| `q` | Quit |

The config lives at `~/.config/symphony/config.toml` (on Windows: `%APPDATA%\symphony\config.toml`). `symphony --help` lists all commands.

### AI setup

```bash
ollama pull gemma3:4b
```

```toml
[ai]
enabled = true
provider = "ollama"          # or "openrouter"

[ai.ollama]
model = "gemma3:4b"
embedding_model = "nomic-embed-text"   # optional, for semantic search
```

---

## Performance

Measured on the author's Windows 11 laptop with a release build, playing an MP3 and rendering the full UI at 60 fps into an in-memory terminal (`cargo run --release --example perf_probe`). Single runs, not a benchmark suite.

| Metric | Measured |
|--------|----------|
| Binary size | 11.6 MB |
| Start to first rendered frame | 77–93 ms |
| Memory while playing | 16–24 MB |
| CPU while playing | about 3.6% of one core |
| Frame time (update + render) | 0.5 ms average, one 130–155 ms frame at track start |

Not measured: Linux and macOS, long sessions, large libraries.

---

## What is not done or not verified

- **Live services**: the AI assistant, LRCLIB lyrics and YouTube were first tested against mock servers and a fake `yt-dlp`. Since then OpenRouter (one free model), real `yt-dlp` and Last.fm have each been run live; Ollama and Discord have not. Please report what breaks
- **Missing**: seeking inside a track, crossfade, gapless playback, loudness normalization, a plugin marketplace. Spotify was dropped on purpose (its developer API needs the app owner to have Premium)
- **Platforms**: developed and tested on Windows. Linux and macOS should work (CI is set up to build on both) but are not verified by hand
- **Plugin toolchain**: the runtime is tested with hand-written WebAssembly; no Rust/C plugin SDK is provided yet

---

## Documentation

| Document | Description |
|----------|-------------|
| [User Guide](docs/USER_GUIDE.md) | User documentation |
| [Configuration](docs/CONFIGURATION.md) | Configuration reference |
| [Plugin Development](docs/PLUGIN_DEVELOPMENT.md) | Creating plugins |
| [API Reference](docs/API.md) | Plugin API |
| [Troubleshooting](docs/TROUBLESHOOTING.md) | Common issues |

The files in `docs/` were rewritten to match the code: every setting they list is marked as read or ignored, and the plugin example is run by the test suite.

---

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

```bash
cargo test                                                  # unit + integration tests
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
```

## License

MIT, see [LICENSE](LICENSE).

## Built with

[Ratatui](https://github.com/ratatui-org/ratatui), [Rodio](https://github.com/RustAudio/rodio), [Symphonia](https://github.com/pdeljanov/Symphonia), [RustFFT](https://github.com/ejmahler/RustFFT), [wasmi](https://github.com/wasmi-labs/wasmi), [Ollama](https://ollama.com), [yt-dlp](https://github.com/yt-dlp/yt-dlp), [LRCLIB](https://lrclib.net).
