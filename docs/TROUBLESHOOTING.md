# Troubleshooting

First step for almost anything: run `symphony --verbose`, which prints debug logs to stderr, and look at the config file (see [Configuration](CONFIGURATION.md)). The messages below are the ones Symphony actually prints.

## Start-up and audio

**"Failed to initialize audio output stream"**
Symphony could not open an output device. Check that a sound device exists and is not exclusively held by another program. On Linux install the ALSA development package at build time (`libasound2-dev`) and make sure PulseAudio or PipeWire is running. Over SSH or in a container there is usually no device. The automated tests skip their playback checks in that case, but the player itself needs one.

**Nothing in the library**
Run `symphony scan <folder>`. The `music_directory` setting is not read; the folder always comes from the command. Supported files: MP3, FLAC, OGG, WAV, AAC, M4A.

**Start over with a clean library**
Delete `library.db` (path printed by `symphony init`) and scan again. This also forgets saved stream tracks and playlists.

**A key does nothing**
The keys are fixed and the `[keybindings]` section is ignored. Press `h` for the in-app list. In search and AI prompt mode, letters are typed into the box; `Esc` leaves it.

## AI assistant

**"AI is disabled: set enabled = true under [ai] in the config"**
Set `enabled = true` under `[ai]`.

**"OpenRouter API key not set. Get one from https://openrouter.ai/keys"**
`provider = "openrouter"` needs `api_key` under `[ai.openrouter]`.

**"Rate limited"**
OpenRouter free models (ids ending in `:free`) are throttled. Wait, or switch `model` to another `:free` model. A paid model id on a key without credits fails differently.

**Ollama: connection refused or no answer**
Start it (`ollama serve`), check `base_url` under `[ai.ollama]`, and pull the model (`ollama pull gemma3:4b`). "Find tracks by meaning" also uses the embedding model (`ollama pull nomic-embed-text`); without it Symphony falls back to a plain-text search.

**The AI plays the wrong thing**
Mood and tempo are matched against what Symphony measured from your audio, which is analysed in the background after a scan; give it a minute on a big library. Genre and artist come from the file tags, so untagged files can only be found by title or by meaning.

**Small models misread requests**
Small or free models sometimes pick the wrong intent. Rephrase ("play calm music") or try a stronger model.

## Streaming

**"yt-dlp is not installed. Install with: pip install yt-dlp"**
Install it (`pip install yt-dlp`, or your package manager) and make sure the command works in the same terminal, or set `ytdlp_path` under `[streaming.youtube]`. YouTube changes often; if searches or downloads start failing, update it (`pip install -U yt-dlp`).

**"Stream search failed: ..." / "No streams found for ..."**
The text after the colon is yt-dlp's own error. Try the same search in yt-dlp directly.

**A search picked a strange video**
The default pick is the first result that is between 1 second and 20 minutes long. Long mixes and livestreams are skipped, but anything else YouTube ranks first can still win; the results list is not filtered.

**Disk usage**
Streams are cached under your cache folder (`symphony/streams`) in hot, warm and cold tiers with the size limits in `[streaming.cache]`. Deleting that folder is safe; tracks are fetched again when played.

## Lyrics

**"No lyrics for this track"**
Put `<song name>.lrc` next to the audio file (same name, `.lrc` extension), or leave `auto_fetch = true` under `[lyrics]` to look it up on LRCLIB. Not every song is there.

**Translation does nothing**
`x` needs lyrics on screen and a working AI provider (see above). The target language is `translate_to`.

## Last.fm

**"set api_key and api_secret under [plugins.lastfm] ... first"**
Create an API account at last.fm/api/account/create and put both values in `[plugins.lastfm]`.

**"Last.fm: Unauthorized Token - This token has not been authorized"**
`symphony lastfm-login` prints a Last.fm link; open that exact link, click "Yes, allow access", and only then press Enter in the terminal. Each run makes a new link, so a link from an earlier run will not work. You must be signed in to Last.fm in the browser you use.

**Plays do not show up**
Check `enabled = true` and a non-empty `session_key` under `[plugins.lastfm]`. A track is scrobbled only after `scrobble_threshold` percent of it has played. Last.fm can take a minute to show new plays.

## Discord

"Discord: set client_id under [plugins.discord]" means the Application ID is missing. Create an application at discord.com/developers and copy its Application ID. The Discord desktop app must be running on the same machine.

## Status line

`symphony status` prints nothing (zsh) or an idle label (tmux) when the player is not running or its snapshot is more than 4 seconds old. The player writes the snapshot into `status.json` in `cache_directory` once a second; `status_file` under `[integrations.statusline]` changes the location, and `SYMPHONY_STATUS_FILE` overrides it for the command. An unknown format prints the list of valid ones.

## Plugins

Run with `--verbose`: a plugin that fails to load, traps, runs out of its instruction budget or is refused a permission is logged by name. A plugin that failed receives no more events until Symphony restarts. See [Plugin Development](PLUGIN_DEVELOPMENT.md).

## Still stuck

Open an issue at https://github.com/Rcidshacker/Symphony/issues with the output of `symphony --verbose`, your OS, and the config file with every key, secret and token removed.
