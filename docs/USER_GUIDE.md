# Symphony User Guide

This guide describes what Symphony does today. Everything here is wired into the player and covered by automated tests; where a feature depends on an outside service that has only been tried against mock servers, it says so. The [README](../README.md) lists what is not built.

## Install and first run

Build from source (Rust 1.88 or newer; on Linux also `sudo apt install libasound2-dev pkg-config build-essential`):

```bash
git clone https://github.com/Rcidshacker/Symphony.git
cd Symphony
cargo build --release          # binary: target/release/symphony
symphony init                  # writes the config, creates the database and cache folders
symphony scan ~/Music          # adds your music to the library
symphony                       # opens the player
```

`symphony --help` lists every command. `symphony --verbose` logs debug output to stderr.

Where things live (`<config>` is `~/.config` on Linux, `~/Library/Application Support` on macOS, `%APPDATA%` on Windows):

| What | Default location |
|------|------------------|
| Config | `<config>/symphony/config.toml` (created on first run) |
| Library database | your OS data folder, `symphony/library.db` |
| Stream and lyrics caches | your OS cache folder, `symphony/streams` and `symphony/lyrics` |
| Plugins | `<config>/symphony/plugins/` |

`symphony init` prints the exact paths on your machine.

## Commands

| Command | What it does |
|---------|--------------|
| `symphony` | Opens the interactive player |
| `symphony init` | Creates the config file, folders and database |
| `symphony scan <dir>` | Adds the music in a folder to the library |
| `symphony stream <query>` | Searches YouTube (and Spotify, if configured) and plays the best match |
| `symphony add <source>` | Saves a track or playlist to the library without playing: a path, `youtube:<id or url>`, `spotify:track:<id>`, `spotify:playlist:<id>`, or a youtube.com / youtu.be / open.spotify.com link |
| `symphony status [format]` | Prints one line about the running player (see [Status line](#status-line)) |
| `symphony lastfm-login` | Authorizes Last.fm scrobbling |
| `symphony "<prompt>"` | Opens the player and sends the prompt to the AI |

## The player

Supported audio: MP3, FLAC, OGG/Vorbis, WAV, AAC and M4A. Tags (title, artist, album, genre, year) and duration are read from the files. Seeking inside a track is not implemented.

### Keys

| Key | Action |
|-----|--------|
| `Space` | Play / pause |
| `n` or `]`, `p` or `[` | Next / previous track |
| `+` or `=`, `-` | Volume up / down |
| `m` | Mute |
| `s`, `r` | Shuffle, repeat |
| `Up` / `k`, `Down` / `j` | Move in the list |
| `Enter` | Play the selected track |
| `/` | Search (type, `Enter` to accept, `Esc` to cancel) |
| `a` | Ask the AI |
| `l` | Show / hide lyrics |
| `x` | Translate the lyrics |
| `w` | Pomodoro: start, stop, or start the break |
| `v` | Next visualizer mode |
| `1` `2` `3` `4` `5` | Bars, waveform, circular, minimal, off |
| `t` | Next theme |
| `h` or `?` | Help overlay |
| `q` or `Ctrl+C` | Quit |

The keys are fixed. The `[keybindings]` section in the config file is not read.

### Visualizer and themes

The spectrum is computed from the audio that is actually playing (FFT size and smoothing come from `[visualizer]`). Five themes ship with the player: monokai, gruvbox, nord, dracula and solarized-dark. Set the default with `theme` under `[general]`, or press `t`.

Album art is shown when the file has an embedded cover or a `cover.jpg` / `folder.png` style image next to it.

## AI assistant

Press `a`, type a request, press `Enter`. You can also pass the request on the command line. It needs either a running [Ollama](https://ollama.com) or an OpenRouter key; set `provider` under `[ai]` (see [Configuration](CONFIGURATION.md)).

| You say | What happens |
|---------|--------------|
| "play something energetic", "play calm music" | Queues the tracks whose measured loudness and tempo fit the mood. Mood and tempo are matched against what Symphony measured from your audio (analysis runs in the background and is remembered), not against file names |
| "play some jazz", "play Radiohead" | Matches genre and artist tags |
| "create a 2-hour focus session" | Builds a playlist of about that length, calm tracks first, and says if the library is too small |
| "more like Gym Anthem" | Shows the tracks that sound most alike, `Enter` plays |
| "something sad" | Semantic search: finds tracks by meaning using embeddings (Ollama) with a plain-text fallback |
| "what should I listen to while debugging?" | Answers in a popup; any key closes it |
| "volume 30", "shuffle on", "repeat all", "skip" | Changes the player |

If the AI understands a request as a suggestion but there is nothing to base it on (no track named, nothing playing), your question is answered in the popup instead.

During a Pomodoro work session, an unqualified "play something" picks calm focus music.

Providers: Ollama has been run end to end only against a test server; OpenRouter was tried live with a free model (`nvidia/nemotron-3-super-120b-a12b:free`) for intent parsing and answers. Free OpenRouter models are rate limited and sometimes answer "Rate limited"; try another `:free` model or wait.

## Streaming

Needs [`yt-dlp`](https://github.com/yt-dlp/yt-dlp) on your `PATH` (or set `streaming.youtube.ytdlp_path`). Tested live: search, download, caching and playback.

* `symphony stream "lofi hip hop"` searches and plays the best match. Results that are livestreams or longer than 20 minutes are skipped for the default pick.
* Streamed tracks are downloaded to the cache as M4A and played from there, so replays and the prefetched next track start instantly and work offline. The cache has hot, warm and cold tiers sized by `[streaming.cache]`.
* Saved stream tracks stay in your library; `Enter` fetches and plays them.
* **Spotify** (`[streaming.spotify]`, needs a client id and secret; the API itself is free, but Spotify requires the app owner's account to be Premium for a development-mode app to work, and limits it to 5 users) reads track and public-playlist metadata only; the audio comes from the closest-length YouTube match. Liked songs and private playlists are not supported, and the Spotify path has only been tested against a mock server.

## Lyrics

* A `.lrc` file with the same name as the song, next to it (`song.mp3` and `song.lrc`), is shown with the current line highlighted, word by word for enhanced LRC.
* With `auto_fetch = true` (the default) Symphony looks the song up on [LRCLIB](https://lrclib.net) when there is no `.lrc` file, and caches the result.
* `x` translates the current lyrics into `translate_to` (default English) with your configured AI model.

## Status line

`symphony status [format]` prints what the running player is playing. Formats: `tmux` (default), `zsh`, `minimal`, `json`, `i3blocks`, `waybar`, `polybar`. The player writes a snapshot file once a second; the command reads it. When the player is not running or the snapshot is stale, `zsh` prints nothing and `tmux` prints a short idle label.

```bash
# tmux.conf
set -g status-right '#(symphony status tmux)'
# .zshrc
RPROMPT='$(symphony status zsh)'
```

## Developer features

* **Git-adaptive music.** With `adapt_music = true` under `[integrations.git]`, Symphony counts your commits in the last hour (checked every `check_interval` seconds) and, at the next track change, switches the queue to music that suits it: 0 commits calm, 1-2 steady, 3-5 driven, 6 or more energetic. Your queue is untouched while the level stays the same.
* **Pomodoro** (`w`): work and break timers from `[integrations.pomodoro]`, shown in the status bar.
* The status bar also shows the current git branch and activity level.

## Last.fm

Free account and API key from last.fm/api, then put `api_key` and `api_secret` under `[plugins.lastfm]` and run `symphony lastfm-login`. It opens an authorization page, you click "Yes, allow access", press Enter in the terminal, and the session key is saved. After that the player shows "now playing" and scrobbles once `scrobble_threshold` percent of a track has played. Verified live: now playing and a scrobble both reached a real account.

## Discord and plugins

* **Discord Rich Presence** (`[plugins.discord]`): create an application at the Discord developer portal and put its Application ID in `client_id`. The Discord desktop app must be running. Only tested against a fake pipe.
* **WASM plugins** go in the plugins folder; see [Plugin Development](PLUGIN_DEVELOPMENT.md) and the [host API](API.md).

## Troubleshooting

See [Troubleshooting](TROUBLESHOOTING.md).
