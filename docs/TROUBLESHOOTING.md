# Symphony Troubleshooting Guide

**Solutions to common problems and how to get help**

---

## Table of Contents

1. [Quick Diagnostics](#quick-diagnostics)
2. [Audio Issues](#audio-issues)
3. [AI/LLM Issues](#aillm-issues)
4. [Streaming Issues](#streaming-issues)
5. [Plugin Issues](#plugin-issues)
6. [Performance Issues](#performance-issues)
7. [UI/Display Issues](#uidisplay-issues)
8. [Database Issues](#database-issues)
9. [Installation Issues](#installation-issues)
10. [Platform-Specific Issues](#platform-specific-issues)
11. [Error Messages Reference](#error-messages-reference)
12. [Getting Help](#getting-help)

---

## Quick Diagnostics

Before diving into specific issues, run these diagnostic commands:

### Check Symphony Status

```bash
# Version and build info
symphony --version

# Configuration status
symphony --check-config

# Show effective configuration
symphony --show-config

# List audio devices
symphony --list-audio-devices

# Database status
symphony --db-status
```

### Enable Verbose Logging

```bash
# Run with debug logging
symphony --verbose

# Or with trace logging (very detailed)
symphony --debug

# Save logs to file
symphony --verbose 2>&1 | tee symphony-debug.log
```

### Health Check

```bash
# Run comprehensive health check
symphony --health-check
```

This checks:
- Configuration validity
- Database connectivity
- Audio system
- AI provider connection
- Streaming capabilities
- Plugin status

---

## Audio Issues

### No Sound Output

**Symptoms**: Symphony appears to play music but no audio is heard.

**Diagnosis**:
```bash
# Check if other audio apps work
mpv test.mp3

# Check system volume
pactl get-sink-volume @DEFAULT_SINK@  # Linux PulseAudio
amixer sget Master                     # Linux ALSA
```

**Solutions**:

1. **Check mute status**:
   - Press `m` in Symphony to toggle mute
   - Check system volume isn't muted

2. **Check output device**:
   ```bash
   # List available devices
   symphony --list-audio-devices
   
   # Set specific device in config
   # ~/.config/symphony/config.toml
   [audio]
   output_device = "sysdefault"
   ```

3. **Linux-specific fixes**:
   ```bash
   # Ensure user is in audio group
   sudo usermod -aG audio $USER
   # Log out and back in
   
   # Check ALSA devices
   aplay -l
   
   # Test ALSA directly
   speaker-test -t sine -f 440 -c 2
   
   # Restart PulseAudio (if using)
   pulseaudio -k
   ```

4. **macOS-specific fixes**:
   ```bash
   # Check CoreAudio
   sudo killall coreaudiod
   
   # Verify portaudio
   brew reinstall portaudio
   ```

### Audio Stuttering/Glitches

**Symptoms**: Audio plays but stutters, pops, or has dropouts.

**Solutions**:

1. **Increase buffer size**:
   ```toml
   [audio]
   buffer_size = 4096  # Try 4096 or 8192
   ```

2. **Reduce CPU load**:
   - Disable visualizations: `v` key or config
   - Reduce FFT size if enabled

3. **Check system load**:
   ```bash
   # Check CPU usage
   top
   
   # Check for CPU throttling (Linux)
   cat /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor
   ```

4. **Disable audio processing**:
   ```toml
   [audio]
   normalize_audio = false
   
   [audio.effects]
   equalizer = false
   ```

### Distorted Audio

**Symptoms**: Audio sounds distorted, crackly, or overdriven.

**Solutions**:

1. **Lower volume**:
   - Press `-` key multiple times
   - Or set in config: `volume = 0.5`

2. **Disable normalization**:
   ```toml
   [audio]
   normalize_audio = false
   ```

3. **Check for clipping in source files**:
   - Some poorly mastered tracks have built-in distortion
   - Try different tracks to isolate the issue

### Wrong Sample Rate

**Symptoms**: Audio plays at wrong pitch or speed.

**Solution**:
```toml
[audio]
sample_rate = 44100  # Match your audio files
```

Check your audio files:
```bash
ffprobe input.mp3 2>&1 | grep "Audio:"
```

---

## AI/LLM Issues

### Ollama Not Responding

**Symptoms**: AI commands fail with connection errors.

**Diagnosis**:
```bash
# Check if Ollama is running
curl http://localhost:11434/api/tags

# Check Ollama service
systemctl status ollama  # If running as service
```

**Solutions**:

1. **Start Ollama**:
   ```bash
   ollama serve
   ```

2. **Install model**:
   ```bash
   # List available models
   ollama list
   
   # Pull recommended model
   ollama pull llama3.2:3b
   ```

3. **Check configuration**:
   ```toml
   [ai]
   enabled = true
   provider = "ollama"
   
   [ai.ollama]
   base_url = "http://localhost:11434"
   model = "llama3.2:3b"
   ```

4. **Check port conflicts**:
   ```bash
   lsof -i :11434
   ```

### AI Responses Are Slow

**Symptoms**: AI commands take too long to respond.

**Solutions**:

1. **Use a smaller/faster model**:
   ```bash
   ollama pull llama3.2:1b  # Faster but less capable
   ```
   ```toml
   [ai.ollama]
   model = "llama3.2:1b"
   ```

2. **Increase timeout**:
   ```toml
   [ai.ollama]
   timeout = 60
   ```

3. **Use cloud AI for complex queries**:
   ```toml
   [ai]
   provider = "openrouter"
   
   [ai.openrouter]
   api_key = "your-key"
   model = "anthropic/claude-3.5-sonnet"
   ```

### AI Gives Wrong Results

**Symptoms**: AI misinterprets commands or gives unexpected responses.

**Solutions**:

1. **Try a more capable model**:
   ```bash
   ollama pull llama3.1:8b
   ```

2. **Adjust temperature**:
   ```toml
   [ai]
   temperature = 0.3  # Lower = more deterministic
   ```

3. **Clear AI cache**:
   ```bash
   rm -rf ~/.cache/symphony/ai/*
   ```

---

## Streaming Issues

### YouTube Streaming Fails

**Symptoms**: YouTube videos don't play or show errors.

**Diagnosis**:
```bash
# Check yt-dlp version
yt-dlp --version

# Test yt-dlp directly
yt-dlp --simulate "https://youtube.com/watch?v=dQw4w9WgXcQ"
```

**Solutions**:

1. **Update yt-dlp** (YouTube changes frequently):
   ```bash
   pip install --upgrade yt-dlp
   # or
   brew upgrade yt-dlp
   ```

2. **Install FFmpeg**:
   ```bash
   # Ubuntu/Debian
   sudo apt install ffmpeg
   
   # macOS
   brew install ffmpeg
   ```

3. **Check yt-dlp path**:
   ```toml
   [streaming.youtube]
   ytdlp_path = "/usr/local/bin/yt-dlp"
   ```

4. **Lower quality**:
   ```toml
   [streaming.youtube]
   quality = "low"  # or "medium"
   ```

### Spotify Authentication Fails

**Symptoms**: Can't connect to Spotify account.

**Solutions**:

1. **Verify credentials**:
   - Go to https://developer.spotify.com/dashboard
   - Check Client ID and Secret are correct
   - Ensure redirect URI matches: `http://localhost:8888/callback`

2. **Re-authenticate**:
   ```bash
   symphony spotify auth --force
   ```

3. **Check callback server**:
   ```bash
   # Ensure port 8888 is free
   lsof -i :8888
   ```

### Streaming Buffering

**Symptoms**: Streaming audio pauses to buffer frequently.

**Solutions**:

1. **Increase cache size**:
   ```toml
   [streaming.cache]
   max_warm_size_mb = 4096
   ```

2. **Enable prefetching**:
   ```toml
   [streaming.cache]
   prefetch_enabled = true
   prefetch_count = 5
   ```

3. **Lower quality**:
   ```toml
   [streaming.youtube]
   quality = "low"
   
   [streaming.spotify]
   quality = "normal"
   ```

4. **Check network**:
   ```bash
   # Test bandwidth
   curl -s https://raw.githubusercontent.com/sivel/speedtest-cli/master/speedtest.py | python3 -
   ```

---

## Plugin Issues

### Plugin Won't Load

**Symptoms**: Plugin shows as "Error" or doesn't appear.

**Diagnosis**:
```bash
# Check plugin directory
ls -la ~/.config/symphony/plugins/

# Validate WASM file
wasm-validate ~/.config/symphony/plugins/my-plugin/plugin.wasm
```

**Solutions**:

1. **Check manifest**:
   ```bash
   cat ~/.config/symphony/plugins/my-plugin/plugin.toml
   ```
   - Ensure all required fields exist
   - Check TOML syntax

2. **Check WASM exports**:
   ```bash
   wasm-objdump -x plugin.wasm | grep export
   ```
   - Must export `plugin_init` and `on_event`

3. **Check permissions**:
   - Ensure manifest permissions match required host functions
   - Remove unused permissions

4. **Rebuild plugin**:
   ```bash
   cargo build --target wasm32-unknown-unknown --release
   ```

### Plugin Causes Crashes

**Symptoms**: Symphony crashes when plugin is loaded.

**Solutions**:

1. **Disable plugin**:
   ```toml
   [plugins]
   auto_load = []  # Remove from auto-load
   ```

2. **Check plugin logs**:
   ```bash
   symphony --verbose 2>&1 | grep -i "plugin\|wasm"
   ```

3. **Validate WASM**:
   ```bash
   wasm-validate plugin.wasm
   ```

4. **Report bug**: Include crash logs and plugin code

### Plugin Permission Denied

**Symptoms**: Plugin fails with "Permission denied" errors.

**Solution**:

Add required permissions to `plugin.toml`:
```toml
permissions = ["track_info", "network", "notifications"]
```

---

## Performance Issues

### High CPU Usage

**Symptoms**: Symphony uses excessive CPU.

**Diagnosis**:
```bash
# Check CPU usage
top -p $(pgrep symphony)

# Profile Symphony
perf record -g symphony
perf report
```

**Solutions**:

1. **Reduce FFT size**:
   ```toml
   [visualizer]
   fft_size = 2048
   ```

2. **Disable visualizations**:
   ```toml
   [visualizer]
   enabled = false
   ```

3. **Reduce cache sizes**:
   ```toml
   [streaming.cache]
   max_hot_size_mb = 50
   ```

4. **Disable AI when not needed**:
   ```toml
   [ai]
   enabled = false
   ```

### High Memory Usage

**Symptoms**: Symphony uses too much RAM.

**Solutions**:

1. **Reduce cache**:
   ```toml
   [streaming.cache]
   max_hot_size_mb = 50
   max_warm_size_mb = 1024
   max_cold_size_mb = 10240
   ```

2. **Clear caches**:
   ```bash
   rm -rf ~/.cache/symphony/streams/*
   ```

3. **Reduce library size**:
   ```bash
   # Rescan with specific directories
   symphony scan ~/Music/favorites
   ```

### Slow Startup

**Symptoms**: Symphony takes too long to start.

**Solutions**:

1. **Reduce scanned directories**:
   ```toml
   [general]
   music_directory = "~/Music"
   music_directories = []  # Remove extra directories
   ```

2. **Disable auto-scan**:
   ```bash
   symphony --no-scan
   ```

3. **Optimize database**:
   ```bash
   symphony --db-optimize
   ```

---

## UI/Display Issues

### Terminal Display Issues

**Symptoms**: UI looks broken, characters don't render correctly.

**Solutions**:

1. **Check terminal capabilities**:
   ```bash
   echo $TERM
   infocmp | grep colors
   ```

2. **Set correct TERM**:
   ```bash
   export TERM=xterm-256color
   ```

3. **Disable True Color if unsupported**:
   ```bash
   export NO_AT_BRIDGE=1
   ```

4. **Use appropriate font**:
   - Install Nerd Font for best symbols support
   - Or use `--no-unicode` flag

### Album Art Not Showing

**Symptoms**: Album art area is blank or shows errors.

**Solutions**:

1. **Check terminal support**:
   - Kitty: Native Sixel support
   - iTerm2: Native Sixel support
   - Others: ASCII fallback

2. **Enable fetching**:
   ```toml
   [visualizer.album_art]
   enabled = true
   fetch_missing = true
   ```

3. **Force ASCII mode**:
   ```toml
   [visualizer.album_art]
   mode = "ascii"
   ```

### Colors Look Wrong

**Symptoms**: Colors don't match expected theme.

**Solutions**:

1. **Verify theme**:
   ```bash
   symphony --show-config | grep theme
   ```

2. **Check terminal colors**:
   ```bash
   # Test 256 colors
   for i in {0..255}; do printf "\x1b[38;5;${i}mcolor${i}\n"; done
   ```

3. **Use a compatible theme**:
   ```toml
   [general]
   theme = "monokai"
   ```

---

## Database Issues

### Database Corrupted

**Symptoms**: Errors about database integrity, missing tracks.

**Solutions**:

1. **Check database**:
   ```bash
   symphony --db-check
   ```

2. **Rebuild database**:
   ```bash
   # Backup first
   cp ~/.local/share/symphony/library.db ~/.local/share/symphony/library.db.backup
   
   # Rebuild
   symphony scan --rebuild ~/Music
   ```

3. **Reset completely**:
   ```bash
   rm ~/.local/share/symphony/library.db
   symphony scan ~/Music
   ```

### Library Not Updating

**Symptoms**: New music doesn't appear in library.

**Solutions**:

1. **Force rescan**:
   ```bash
   symphony scan --force ~/Music
   ```

2. **Check file permissions**:
   ```bash
   ls -la ~/Music/
   ```

3. **Check supported formats**:
   - MP3, FLAC, AAC, M4A, OGG, WAV are supported
   - WMA and others may not be

---

## Installation Issues

### Build Fails

**Symptoms**: `cargo build` fails with errors.

**Common fixes**:

1. **Missing dependencies (Linux)**:
   ```bash
   sudo apt install libasound2-dev libpulse-dev pkg-config build-essential
   ```

2. **Outdated Rust**:
   ```bash
   rustup update
   ```

3. **Clean build**:
   ```bash
   cargo clean
   cargo build --release
   ```

### Runtime Linking Errors

**Symptoms**: "library not found" errors when running.

**Solutions**:

1. **Linux**:
   ```bash
   # Find missing library
   ldd $(which symphony)
   
   # Install if missing
   sudo apt install libasound2
   ```

2. **macOS**:
   ```bash
   # Check libraries
   otool -L $(which symphony)
   
   # Reinstall if needed
   brew reinstall portaudio
   ```

---

## Platform-Specific Issues

### Linux: PipeWire Issues

**Symptoms**: Audio doesn't work with PipeWire.

**Solution**:
```bash
# Ensure PipeWire-Pulse is installed
sudo apt install pipewire-pulse

# Restart PipeWire
systemctl --user restart pipewire
```

### macOS: Permission Issues

**Symptoms**: Symphony can't access Music folder.

**Solution**:
1. System Preferences → Privacy & Security → Files and Folders
2. Grant Terminal (or your terminal app) access to Music folder

### Windows (WSL2): No Audio

**Symptoms**: No audio output in WSL2.

**Solution**:
```bash
# Install PulseAudio on Windows
# Then in WSL2:
export PULSE_SERVER=tcp:$(hostname).local
```

---

## Error Messages Reference

### Audio Errors

| Error | Cause | Solution |
|-------|-------|----------|
| `No audio device found` | No output devices available | Check system audio config |
| `Device busy` | Another app using device | Close other audio apps |
| `Underrun detected` | Buffer too small | Increase buffer_size |
| `Unsupported format` | Can't play file type | Check file format |

### AI Errors

| Error | Cause | Solution |
|-------|-------|----------|
| `Ollama connection refused` | Ollama not running | Start Ollama service |
| `Model not found` | Model not downloaded | `ollama pull <model>` |
| `API timeout` | Slow response | Increase timeout setting |
| `Rate limited` | Too many requests | Wait or use different provider |

### Streaming Errors

| Error | Cause | Solution |
|-------|-------|----------|
| `yt-dlp not found` | Not installed | Install yt-dlp |
| `Video unavailable` | Removed or region-locked | Try different video |
| `Authentication failed` | Bad credentials | Re-authenticate |
| `Network timeout` | Connection issue | Check internet |

---

## Getting Help

### Before Asking for Help

1. **Gather information**:
   ```bash
   symphony --version
   symphony --health-check
   uname -a
   ```

2. **Capture debug logs**:
   ```bash
   symphony --debug 2>&1 | tee symphony-debug.log
   ```

3. **Check existing issues**:
   - https://github.com/symphony-player/symphony/issues

### Where to Get Help

1. **Documentation**: Check this guide and other docs
2. **GitHub Issues**: Bug reports and feature requests
3. **Discord**: Real-time community help
4. **Reddit**: r/symphony_player

### Reporting Bugs

When reporting bugs, include:

1. **System Information**:
   - OS and version
   - Symphony version
   - Terminal and version

2. **Steps to Reproduce**: Exact steps to trigger the bug

3. **Expected vs Actual Behavior**: What you expected vs what happened

4. **Logs**: Attach debug logs
   ```bash
   symphony --debug 2>&1 > debug.log
   ```

5. **Configuration**: If relevant, share your config (remove sensitive info)

### Bug Report Template

```
**Description**
A clear description of the bug.

**To Reproduce**
Steps to reproduce:
1. Start Symphony
2. Press '...'
3. See error

**Expected**
What should happen.

**Actual**
What actually happens.

**Environment**
- OS: Ubuntu 22.04
- Symphony: 2.0.0
- Terminal: Alacritty 0.12

**Logs**
```
[paste debug logs]
```

**Additional Context**
Any other relevant information.
```

---

*Troubleshooting Guide for Symphony v2.0*
