# Contributing to Symphony

First off, thank you for considering contributing to Symphony! It's people like you that make Symphony such a great tool.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Setup](#development-setup)
- [Project Structure](#project-structure)
- [Making Changes](#making-changes)
- [Coding Standards](#coding-standards)
- [Testing](#testing)
- [Documentation](#documentation)
- [Pull Request Process](#pull-request-process)
- [Release Process](#release-process)

---

## Code of Conduct

This project and everyone participating in it is governed by our Code of Conduct. By participating, you are expected to uphold this code. Please report unacceptable behavior to conduct@symphony-player.dev.

### Our Standards

- Be respectful and inclusive
- Welcome newcomers and help them get started
- Focus on what is best for the community
- Show empathy towards other community members
- Gracefully accept constructive criticism

---

## Getting Started

### Ways to Contribute

- **Bug reports**: Submit detailed bug reports
- **Feature requests**: Suggest new features or improvements
- **Documentation**: Improve or translate documentation
- **Code**: Fix bugs or implement new features
- **Plugins**: Create and share plugins
- **Themes**: Design and share themes

### First Time Contributors

Look for issues labeled:
- `good first issue` - Great for newcomers
- `help wanted` - We need assistance
- `documentation` - Documentation improvements
- `bug` - Confirmed bugs that need fixing

---

## Development Setup

### Prerequisites

- **Rust 1.75+**: Install from https://rustup.rs
- **Git**: For version control
- **A good editor**: VS Code with rust-analyzer, Neovim, etc.

### System Dependencies

**Linux (Ubuntu/Debian)**:
```bash
sudo apt install -y libasound2-dev libpulse-dev pkg-config build-essential
```

**macOS**:
```bash
brew install portaudio pkg-config
```

### Clone and Build

```bash
# Clone the repository
git clone https://github.com/symphony-player/symphony.git
cd symphony

# Build in debug mode
cargo build

# Run
cargo run

# Run tests
cargo test
```

### Enable Pre-commit Hooks

```bash
# Install pre-commit
pip install pre-commit

# Install hooks
pre-commit install
```

---

## Project Structure

Understanding the codebase:

```
symphony/
├── src/
│   ├── main.rs           # Entry point, event loop
│   ├── app.rs            # Application state machine
│   │
│   ├── audio/            # Audio subsystem
│   │   ├── mod.rs
│   │   ├── engine.rs     # Core playback engine
│   │   └── analyzer.rs   # FFT analysis
│   │
│   ├── ui/               # Terminal UI
│   │   ├── mod.rs
│   │   ├── visualizer.rs # Visualization rendering
│   │   └── artwork.rs    # Album art handling
│   │
│   ├── ai/               # AI integration
│   │   ├── mod.rs
│   │   ├── provider.rs   # LLMProvider trait
│   │   ├── ollama.rs     # Ollama implementation
│   │   ├── openrouter.rs # OpenRouter implementation
│   │   ├── manager.rs    # AIManager facade
│   │   ├── embeddings.rs # Embedding generation
│   │   ├── recommendations.rs
│   │   ├── context.rs
│   │   └── playlists.rs
│   │
│   ├── streaming/        # Streaming subsystem
│   │   ├── mod.rs
│   │   ├── provider.rs   # StreamProvider trait
│   │   ├── youtube.rs    # YouTube implementation
│   │   ├── spotify.rs    # Spotify implementation
│   │   ├── cache.rs      # SmartCache system
│   │   ├── predictor.rs  # Prefetch prediction
│   │   └── manager.rs    # StreamManager facade
│   │
│   ├── plugins/          # Plugin system
│   │   ├── mod.rs
│   │   ├── runtime.rs    # Wasmtime runtime
│   │   ├── api.rs        # Host functions
│   │   └── registry.rs   # Plugin management
│   │
│   ├── lyrics/           # Lyrics system
│   │   ├── mod.rs
│   │   ├── lrc.rs        # LRC parsing
│   │   ├── fetcher.rs    # Online fetching
│   │   └── display.rs    # Rendering
│   │
│   ├── integrations/     # Developer features
│   │   ├── mod.rs
│   │   ├── git.rs        # Git monitoring
│   │   ├── pomodoro.rs   # Pomodoro timer
│   │   └── statusline.rs # Status line output
│   │
│   ├── config/           # Configuration
│   │   ├── mod.rs
│   │   └── theme.rs      # Theme loading
│   │
│   ├── db/               # Database
│   │   └── mod.rs
│   │
│   └── scanner/          # Library scanning
│       └── mod.rs
│
├── tests/                # Integration tests
│   ├── integration_tests.rs
│   ├── streaming_tests.rs
│   ├── visualizer_tests.rs
│   └── advanced_tests.rs
│
├── benches/              # Benchmarks
│   └── fft_bench.rs
│
├── themes/               # Built-in themes
│   ├── monokai.toml
│   ├── gruvbox.toml
│   ├── nord.toml
│   ├── dracula.toml
│   └── solarized-dark.toml
│
├── config/               # Default config
│   └── symphony.toml
│
└── docs/                 # Documentation
    ├── USER_GUIDE.md
    ├── API.md
    ├── PLUGIN_DEVELOPMENT.md
    ├── CONFIGURATION.md
    └── TROUBLESHOOTING.md
```

### Key Concepts

1. **Modular Architecture**: Each subsystem is self-contained
2. **Trait-based Design**: Interfaces enable extensibility
3. **Async Runtime**: Tokio for concurrent operations
4. **Event-driven**: Events flow through the application

---

## Making Changes

### Branch Naming

Use descriptive branch names:
- `feature/add-equalizer`
- `fix/audio-buffer-underrun`
- `docs/update-user-guide`
- `refactor/simplify-cache-logic`

### Commit Messages

Follow the [Conventional Commits](https://www.conventionalcommits.org/) specification:

```
<type>(<scope>): <description>

[optional body]

[optional footer(s)]
```

Types:
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation only
- `style`: Code style changes (formatting, etc.)
- `refactor`: Code refactoring
- `perf`: Performance improvement
- `test`: Adding or modifying tests
- `chore`: Maintenance tasks

Examples:
```
feat(ai): add support for Claude 3.5 Sonnet

fix(audio): prevent buffer underrun on slow systems

docs(user-guide): add streaming setup instructions

refactor(cache): simplify LRU implementation
```

### Code Organization

1. **Keep modules focused**: Each module should have a single responsibility
2. **Use traits for extensibility**: Define traits for component interfaces
3. **Prefer composition**: Combine small, focused components
4. **Document public APIs**: All public items need documentation

---

## Coding Standards

### Formatting

We use `rustfmt` with default settings:

```bash
# Format code
cargo fmt

# Check formatting
cargo fmt -- --check
```

### Linting

We use Clippy for linting:

```bash
# Run clippy
cargo clippy

# Run with warnings as errors
cargo clippy -- -D warnings
```

### Code Style Guidelines

1. **Use meaningful names**:
   ```rust
   // Good
   let spectrum_analyzer = SpectrumAnalyzer::new(sample_rate);
   
   // Bad
   let sa = SpectrumAnalyzer::new(sr);
   ```

2. **Prefer functional style**:
   ```rust
   // Good
   let spectrum: Vec<f32> = samples
       .iter()
       .zip(window.iter())
       .map(|(s, w)| s * w)
       .collect();
   
   // Bad
   let mut spectrum = Vec::with_capacity(samples.len());
   for i in 0..samples.len() {
       spectrum.push(samples[i] * window[i]);
   }
   ```

3. **Handle errors properly**:
   ```rust
   // Good
   pub fn load_track(&mut self, path: &Path) -> Result<(), AudioError> {
       let track = Track::from_file(path)?;
       self.playlist.add(track);
       Ok(())
   }
   
   // Bad
   pub fn load_track(&mut self, path: &Path) {
       let track = Track::from_file(path).unwrap();
       self.playlist.add(track);
   }
   ```

4. **Document public APIs**:
   ```rust
   /// Analyzes audio samples and returns frequency spectrum.
   ///
   /// # Arguments
   ///
   /// * `samples` - Audio samples to analyze (must match FFT size)
   ///
   /// # Returns
   ///
   /// Vector of magnitude values for each frequency bin
   ///
   /// # Example
   ///
   /// ```
   /// let analyzer = SpectrumAnalyzer::new(44100, 4096);
   /// let spectrum = analyzer.analyze(&samples);
   /// ```
   pub fn analyze(&mut self, samples: &[f32]) -> Vec<f32> {
       // Implementation
   }
   ```

5. **Use appropriate types**:
   ```rust
   // Good
   pub struct TrackId(Uuid);
   pub struct Duration(std::time::Duration);
   
   // Bad
   pub type TrackId = String;
   pub type Duration = u64;
   ```

---

## Testing

### Running Tests

```bash
# Run all tests
cargo test

# Run specific test
cargo test test_fft_analyzer

# Run tests with output
cargo test -- --nocapture

# Run integration tests
cargo test --test integration_tests
```

### Writing Tests

1. **Unit tests** go in the same file:
   ```rust
   // src/audio/analyzer.rs
   
   #[cfg(test)]
   mod tests {
       use super::*;
       
       #[test]
       fn test_sine_wave_detection() {
           let mut analyzer = SpectrumAnalyzer::new(44100, 2048);
           let samples = generate_sine_wave(440.0, 1.0, 44100, 2048);
           let spectrum = analyzer.analyze(&samples);
           
           let peak_freq = find_peak_frequency(&spectrum, 44100);
           assert!((peak_freq - 440.0).abs() < 10.0);
       }
   }
   ```

2. **Integration tests** go in `tests/`:
   ```rust
   // tests/integration_tests.rs
   
   use symphony::audio::AudioEngine;
   
   #[test]
   fn test_playback_workflow() {
       let mut engine = AudioEngine::new().unwrap();
       engine.load_track("test.mp3").unwrap();
       engine.play().unwrap();
       assert!(engine.is_playing());
   }
   ```

3. **Use test helpers**:
   ```rust
   #[cfg(test)]
   mod helpers {
       pub fn generate_sine_wave(freq: f32, amp: f32, sample_rate: u32, samples: usize) -> Vec<f32> {
           (0..samples)
               .map(|i| {
                   let t = i as f32 / sample_rate as f32;
                   amp * (2.0 * std::f32::consts::PI * freq * t).sin()
               })
               .collect()
       }
   }
   ```

### Test Coverage

We aim for >80% code coverage. Check coverage:

```bash
cargo tarpaulin --out Html
```

---

## Documentation

### Code Documentation

- All public items must have doc comments
- Include examples in doc comments
- Document panics and errors

```rust
/// Plays the next track in the queue.
///
/// If shuffle is enabled, selects a random unplayed track.
/// If all tracks have been played, reshuffles and starts over.
///
/// # Errors
///
/// Returns `AudioError::QueueEmpty` if the queue is empty.
///
/// # Panics
///
/// Panics if the audio backend is not initialized.
///
/// # Example
///
/// ```no_run
/// let player = Symphony::new()?;
/// player.next_track()?;
/// ```
pub fn next_track(&mut self) -> Result<(), AudioError> {
    // Implementation
}
```

### User Documentation

When adding features, update the relevant documentation:

- `docs/USER_GUIDE.md` - User-facing documentation
- `docs/CONFIGURATION.md` - Configuration options
- `docs/API.md` - Plugin API changes
- `README.md` - Feature list and examples

---

## Pull Request Process

### Before Submitting

1. **Update from main**:
   ```bash
   git fetch origin
   git rebase origin/main
   ```

2. **Run all checks**:
   ```bash
   cargo fmt -- --check
   cargo clippy -- -D warnings
   cargo test
   cargo tarpaulin
   ```

3. **Update documentation** if needed

4. **Add tests** for new functionality

### PR Template

```markdown
## Description
Brief description of changes

## Type of Change
- [ ] Bug fix
- [ ] New feature
- [ ] Breaking change
- [ ] Documentation update

## Testing
- [ ] Tests pass locally
- [ ] New tests added
- [ ] Coverage maintained

## Checklist
- [ ] Code formatted
- [ ] Clippy passes
- [ ] Documentation updated
- [ ] CHANGELOG updated (if significant)
```

### Review Process

1. PRs require at least one approval
2. CI must pass
3. Coverage must not decrease
4. Documentation must be updated if applicable

### After Merge

- Delete your branch
- Your PR will be included in the next release notes

---

## Release Process

Releases are managed by maintainers:

1. Update version in `Cargo.toml`
2. Update `CHANGELOG.md`
3. Create git tag: `git tag v2.0.0`
4. Push tag: `git push origin v2.0.0`
5. CI builds and publishes to crates.io

---

## Questions?

- Open a [Discussion](https://github.com/symphony-player/symphony/discussions)
- Join our [Discord](https://discord.gg/symphony-player)
- Email: dev@symphony-player.dev

---

Thank you for contributing to Symphony! 🎵
