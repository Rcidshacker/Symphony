# Configuration

Symphony reads one TOML file. `symphony init` creates it with the defaults below; edit it with any editor and restart the player.

| OS | Path |
|----|------|
| Linux | `~/.config/symphony/config.toml` |
| macOS | `~/Library/Application Support/symphony/config.toml` |
| Windows | `%APPDATA%\symphony\config.toml` |

Every key has a default, so you can keep only the ones you change. **Keys marked "ignored" are accepted in the file but nothing reads them yet.** They are listed so you are not surprised when changing them does nothing.

Secrets (`api_key`, `api_secret`, `client_secret`, `session_key`) are stored in plain text in this file. Keep it private and out of version control.

## `[general]`

| Key | Default | |
|-----|---------|--|
| `theme` | `"monokai"` | One of `monokai`, `gruvbox`, `nord`, `dracula`, `solarized-dark` (also changed with `t`) |
| `cache_directory` | data folder + `symphony/cache` | Holds the status file and analysis caches |
| `database_path` | data folder + `symphony/library.db` | The SQLite library |
| `music_directory` | your Music folder | Ignored: `symphony scan <dir>` takes the folder as an argument |
| `default_view` | `"library"` | Ignored |

## `[audio]`

| Key | Default | |
|-----|---------|--|
| `volume` | `0.6` | Start volume, 0.0 to 1.0 |
| `output_device`, `sample_rate`, `buffer_size`, `normalize_audio` | `"default"`, 44100, 2048, `false` | Ignored: the system default output device is always used, at its own rate |
| `[audio.effects]` `equalizer`, `reverb`, `bass_boost` | off | Ignored |

## `[visualizer]`

| Key | Default | |
|-----|---------|--|
| `enabled` | `true` | `false` turns the spectrum off |
| `mode` | `"spectrum"` | `spectrum` (bars), `waveform`, `circular`, `minimal`, `off` |
| `fft_size` | `4096` | Rounded up to 2048, 4096 or 8192 (anything above 4096 becomes 8192) |
| `smoothing` | `0.8` | 0.0 to 1.0, higher is smoother |
| `color_scheme`, `beat_detection` | `"rainbow"`, `true` | Ignored |

## `[ai]`

| Key | Default | |
|-----|---------|--|
| `enabled` | `true` | `false` makes `a` and `symphony "<prompt>"` report that the AI is off |
| `provider` | `"ollama"` | `"ollama"` or `"openrouter"` |
| `recommendations_enabled`, `natural_language_commands`, `cache_responses` | `true` | Ignored |

### `[ai.ollama]`

| Key | Default | |
|-----|---------|--|
| `base_url` | `"http://localhost:11434"` | |
| `model` | `"gemma3:4b"` | Used to understand requests and answer questions |
| `embedding_model` | `"nomic-embed-text"` | Used for "find tracks by meaning"; without it a plain-text search is used |

### `[ai.openrouter]`

| Key | Default | |
|-----|---------|--|
| `api_key` | `""` | Required; from openrouter.ai/keys |
| `model` | `"anthropic/claude-3.5-sonnet"` | **Paid.** For the free tier use an id ending in `:free` (browse openrouter.ai/models and filter by Free). `nvidia/nemotron-3-super-120b-a12b:free` worked in testing |
| `site_url`, `app_name` | repo URL, app name | Sent as OpenRouter's attribution headers |

Free models are rate limited and the list changes; "Rate limited" in the status bar means try another `:free` model or wait.

## `[streaming]`

`enabled = true` switches streaming on. It needs `yt-dlp`.

| Key | Default | |
|-----|---------|--|
| `[streaming.youtube]` `ytdlp_path` | `"yt-dlp"` | Command or full path |
| `[streaming.youtube]` `quality` | `"medium"` | `low`, `medium`, `high`, `best`: picks the audio quality downloaded |
| `[streaming.youtube]` `enabled` | `true` | Ignored |
| `[streaming.spotify]` `enabled` | `false` | Turns Spotify metadata on |
| `[streaming.spotify]` `client_id`, `client_secret` | `""` | From developer.spotify.com. Metadata and public playlists only |
| `[streaming.spotify]` `redirect_uri`, `api_url`, `accounts_url` | Spotify's defaults | Change `api_url` / `accounts_url` only to point at a test server |
| `[streaming.spotify]` `quality` | `"high"` | Ignored |
| `[streaming.cache]` `cache_dir` | cache folder + `symphony/streams` | Where downloaded streams are kept |
| `[streaming.cache]` `max_hot_size_mb`, `max_warm_size_mb`, `max_cold_size_mb` | 100, 2048, 20480 | Size of the three cache tiers |
| `[streaming.cache]` `prefetch_enabled`, `prefetch_count` | `true`, `3` | Download the next tracks in advance |
| `[streaming.cache]` `enabled` | `true` | Ignored |
| `[streaming.network]` `max_concurrent_downloads`, `prefer_cached` | `2`, `true` | |
| `[streaming.network]` `bandwidth_limit_mbps`, `timeout_secs` | `0`, `30` | Ignored |

## `[plugins]`

| Key | Default | |
|-----|---------|--|
| `enabled` | `true` | `false` stops the WASM plugin runtime from starting |
| `plugin_dir` | config folder + `symphony/plugins` | One sub-folder per plugin |
| `auto_load` | `[]` | Load only the named plugins; empty loads all |

### `[plugins.discord]`

| Key | Default | |
|-----|---------|--|
| `enabled` | `false` | |
| `client_id` | `""` | Application ID of an application you create at discord.com/developers. Not a secret |
| `show_timestamp` | `true` | Show the elapsed time |

### `[plugins.lastfm]`

| Key | Default | |
|-----|---------|--|
| `enabled` | `false` | Set to `true` by `symphony lastfm-login` |
| `api_key`, `api_secret` | `""` | From last.fm/api/account/create |
| `username`, `session_key` | `""` | Filled in by `symphony lastfm-login` |
| `scrobble_threshold` | `50` | Percent of a track that must play before it is scrobbled |
| `api_url` | Last.fm's endpoint | Change only to point at a test server |

## `[lyrics]`

| Key | Default | |
|-----|---------|--|
| `enabled` | `true` | |
| `auto_fetch` | `true` | Look songs up on LRCLIB when there is no `.lrc` file |
| `lrclib_url` | `"https://lrclib.net"` | |
| `cache_dir` | cache folder + `symphony/lyrics` | |
| `karaoke_mode` | `true` | Highlight word by word when the file has word timings |
| `context_lines` | `2` | Lines shown around the current one |
| `translate_to` | `"English"` | Target language for `x` |
| `show_translation` | `false` | Show a translation under each line when available |

## `[integrations]`

| Key | Default | |
|-----|---------|--|
| `[integrations.git]` `enabled` | `true` | Show git branch and activity in the status bar |
| `[integrations.git]` `adapt_music` | `false` | Switch the queue to calmer or more energetic music as commits per hour change |
| `[integrations.git]` `check_interval` | `60` | Seconds between checks |
| `[integrations.pomodoro]` `enabled` | `false` | Ignored: `w` always works |
| `[integrations.pomodoro]` `work_minutes`, `break_minutes`, `long_break_minutes` | 25, 5, 15 | |
| `[integrations.pomodoro]` `auto_start_break`, `auto_start_work` | `false` | |
| `[integrations.pomodoro]` `notifications` | `true` | |
| `[integrations.statusline]` `enabled` | `true` | Write the status file `symphony status` reads |
| `[integrations.statusline]` `status_file` | `""` | Empty means `status.json` in `cache_directory` |

The environment variable `SYMPHONY_STATUS_FILE` overrides the status file path for `symphony status` only.

## `[keybindings]`

The whole section is ignored: keys are fixed (see the [User Guide](USER_GUIDE.md#keys)).

## Example

A minimal config that uses OpenRouter's free tier and turns on git-adaptive music:

```toml
[ai]
provider = "openrouter"

[ai.openrouter]
api_key = "..."
model = "nvidia/nemotron-3-super-120b-a12b:free"

[integrations.git]
adapt_music = true
```
