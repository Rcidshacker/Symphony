# Plugin development

A Symphony plugin is a WebAssembly module plus a small manifest. Symphony runs it in a sandbox (the [wasmi](https://github.com/wasmi-labs/wasmi) interpreter), sends it playback events as JSON, and lets it call a handful of host functions. This page walks through a plugin that works today; the full contract is in the [API reference](API.md).

What exists and what does not:

* The runtime, permissions, metering and memory cap are real and tested with hand-written WebAssembly (`tests/plugin_tests.rs`).
* There is **no plugin SDK, template or marketplace**. You bring your own toolchain, and anything that produces a wasm module with the right exports works.
* Only the events and host functions listed in the API reference exist.

## 1. The smallest useful plugin

This plugin logs every event it receives. It is WebAssembly text format; the test suite loads the same shape of module.

```wat
(module
  (import "symphony" "log" (func $log (param i32 i32)))
  (memory (export "memory") 1)

  ;; a bump allocator: Symphony asks for a buffer, writes the event JSON into it, then calls on_event
  (global $heap (mut i32) (i32.const 1024))
  (func (export "alloc") (param $n i32) (result i32)
    (local $p i32)
    (local.set $p (global.get $heap))
    (global.set $heap (i32.add (local.get $p) (local.get $n)))
    (local.get $p))

  (func (export "on_event") (param $ptr i32) (param $len i32)
    (call $log (local.get $ptr) (local.get $len))))
```

Notes:

* The bump allocator never frees. Real plugins should reset `$heap` at the start of `on_event` (or use a proper allocator) or they will eventually hit the 16 MiB memory cap.
* Compile it to a binary module with [`wat2wasm`](https://github.com/WebAssembly/wabt): `wat2wasm plugin.wat -o plugin.wasm`.

## 2. The manifest

Next to `plugin.wasm`, create `plugin.toml`:

```toml
name = "event-logger"
version = "0.1.0"
author = "you"
description = "Logs every playback event"
permissions = ["track_info", "playback_state"]
```

Events are only delivered to plugins that hold the right permission, so without `track_info` this plugin would never see `TrackChanged` (see the table in the [API reference](API.md#events)). `AppStarted` and `AppStopped` need none.

## 3. Install

Put both files in their own folder inside the plugins folder:

```text
<config folder>/symphony/plugins/event-logger/
  plugin.toml
  plugin.wasm
```

`<config folder>` is `~/.config` on Linux, `~/Library/Application Support` on macOS and `%APPDATA%` on Windows; change it with `plugin_dir` under `[plugins]`. Start the player with `symphony --verbose` and play a track; the plugin's `log` lines appear in the debug output.

## 4. Doing something with the player

Ask for permissions only for what the plugin needs, then call host functions:

| Goal | Permission | Host function |
|------|------------|---------------|
| Know what is playing | `track_info` | `get_track` |
| Know the volume | `playback_state` | `get_volume` |
| Show a message | `notifications` | `notify` |
| Set Discord Rich Presence | `discord` | `discord_presence` |
| Scrobble to Last.fm | `lastfm` | `scrobble` |

`get_track` writes JSON into a buffer you provide; keep a buffer of a few KiB and handle the `-2` ("too small") result.

## 5. Limits and failure

* Each call may run about 20 million wasm instructions. A plugin that loops forever is stopped and gets no further events.
* Memory is capped at 16 MiB.
* A trap marks only that plugin as failed.
* Calling a host function without its permission returns `-1` (or `-1.0`); it is logged and counted.

## 6. Testing a plugin

Run Symphony with `--verbose`, or load the module in your own test the way `tests/plugin_tests.rs` does: build a `PluginRuntime`, point it at a folder, call `load_all_plugins`, then `trigger_event` with a `PluginEvent`.

## Compiling from a language

Rust, C or AssemblyScript can all target WebAssembly. The module must export `memory`, `alloc` and (to receive events) `on_event` with the signatures above, and import host functions from the module name `symphony`. This has not been tried with any of those toolchains yet, so expect to verify the exports with a tool like `wasm-objdump` or `wasm-tools print` if a plugin does not load.
