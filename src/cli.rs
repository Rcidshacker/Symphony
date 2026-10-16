//! Command-line argument parsing (std only, no extra dependency).
//!
//! `symphony`                      start the TUI
//! `symphony init`                 create config, data dirs and database
//! `symphony scan <dir>`           add a music directory to the library
//! `symphony stream <query>`       search YouTube/Spotify and play the best match
//! `symphony add <path|source:id>` add local music (or a stream source)
//! `symphony lastfm-login`         authorize Last.fm scrobbling
//! `symphony status [format]`      one line about what is playing (tmux, zsh, json, ...)
//! `symphony "<prompt>"`           start the player and run a natural-language command

use std::path::PathBuf;

use thiserror::Error;

/// Help text shown by `--help` and on usage errors.
pub const USAGE: &str = "\
Symphony - AI-powered terminal music player

USAGE:
    symphony [OPTIONS] [COMMAND | \"PROMPT\"]

COMMANDS:
    init                   Create config file, data directories and database
    scan <DIR>             Scan a directory and add its music to the library
    stream <QUERY>         Search YouTube/Spotify and play the best match
    add <SOURCE>           Add a local path, youtube:<id|url> or spotify:<track|playlist>:<id>
    status [FORMAT]        Print what the running player is playing, for tmux/zsh/bars
                           (tmux, zsh, minimal, json, i3blocks, waybar, polybar; default tmux)
    lastfm-login           Authorize Last.fm scrobbling (needs api_key/api_secret in the config)
    \"<PROMPT>\"             Start the player and ask the AI, e.g. symphony \"play something energetic\"

With no command the interactive player starts.

OPTIONS:
    -v, --verbose          Log debug output to stderr
    -h, --help             Print this help
    -V, --version          Print version";

/// What the user asked Symphony to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Start the interactive player
    Tui,
    /// Create config, data directories and database
    Init,
    /// Scan a directory into the library
    Scan(PathBuf),
    /// Search streaming sources
    Stream(String),
    /// Add a local path or `source:id` spec
    Add(String),
    /// Natural-language prompt: start the player and let the AI act on it
    Ask(String),
    /// Authorize Last.fm and store the session key
    LastfmLogin,
    /// Print the status line of the running player in the given format
    Status(String),
    /// Print help
    Help,
    /// Print version
    Version,
}

/// Parsed command line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cli {
    /// `--verbose` was given
    pub verbose: bool,
    /// Selected command
    pub command: Command,
}

/// Command-line errors.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum CliError {
    /// A subcommand needs an argument
    #[error("`{command}` needs {what}")]
    MissingArgument {
        /// Subcommand name
        command: &'static str,
        /// Description of the missing argument
        what: &'static str,
    },
    /// Unrecognized `-x` / `--xxx` option
    #[error("unknown option: {0}")]
    UnknownOption(String),
}

/// Parse arguments (without the program name).
pub fn parse<I>(args: I) -> Result<Cli, CliError>
where
    I: IntoIterator<Item = String>,
{
    let mut verbose = false;
    let mut help = false;
    let mut version = false;
    let mut positional: Vec<String> = Vec::new();

    for arg in args {
        match arg.as_str() {
            "-v" | "--verbose" => verbose = true,
            "-h" | "--help" => help = true,
            "-V" | "--version" => version = true,
            s if s.starts_with('-') && s.len() > 1 && positional.is_empty() => {
                return Err(CliError::UnknownOption(arg));
            }
            _ => positional.push(arg),
        }
    }

    let command = if help {
        Command::Help
    } else if version {
        Command::Version
    } else {
        command_from(positional)?
    };

    Ok(Cli { verbose, command })
}

fn command_from(positional: Vec<String>) -> Result<Command, CliError> {
    let Some(first) = positional.first() else {
        return Ok(Command::Tui);
    };
    let rest = positional[1..].join(" ");

    match first.as_str() {
        "init" => Ok(Command::Init),
        "lastfm-login" => Ok(Command::LastfmLogin),
        "status" if rest.is_empty() => Ok(Command::Status("tmux".to_string())),
        "status" => Ok(Command::Status(rest)),
        "scan" if rest.is_empty() => Err(CliError::MissingArgument {
            command: "scan",
            what: "a directory",
        }),
        "scan" => Ok(Command::Scan(PathBuf::from(rest))),
        "stream" if rest.is_empty() => Err(CliError::MissingArgument {
            command: "stream",
            what: "a search query",
        }),
        "stream" => Ok(Command::Stream(rest)),
        "add" if rest.is_empty() => Err(CliError::MissingArgument {
            command: "add",
            what: "a path or source",
        }),
        "add" => Ok(Command::Add(rest)),
        _ => Ok(Command::Ask(positional.join(" "))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(args: &[&str]) -> Result<Cli, CliError> {
        parse(args.iter().map(|s| s.to_string()))
    }

    #[test]
    fn no_args_starts_tui() {
        assert_eq!(p(&[]).unwrap().command, Command::Tui);
    }

    #[test]
    fn subcommands() {
        assert_eq!(p(&["init"]).unwrap().command, Command::Init);
        assert_eq!(p(&["lastfm-login"]).unwrap().command, Command::LastfmLogin);
        assert_eq!(p(&["status"]).unwrap().command, Command::Status("tmux".into()));
        assert_eq!(p(&["status", "zsh"]).unwrap().command, Command::Status("zsh".into()));
        assert_eq!(
            p(&["scan", "/music"]).unwrap().command,
            Command::Scan(PathBuf::from("/music"))
        );
        assert_eq!(
            p(&["stream", "lofi", "hip", "hop"]).unwrap().command,
            Command::Stream("lofi hip hop".into())
        );
        assert_eq!(
            p(&["add", "youtube:abc"]).unwrap().command,
            Command::Add("youtube:abc".into())
        );
    }

    #[test]
    fn bare_prompt_goes_to_ai() {
        assert_eq!(
            p(&["play something energetic"]).unwrap().command,
            Command::Ask("play something energetic".into())
        );
    }

    #[test]
    fn flags() {
        assert!(p(&["-v"]).unwrap().verbose);
        assert_eq!(p(&["--help"]).unwrap().command, Command::Help);
        assert_eq!(p(&["scan", "x", "-V"]).unwrap().command, Command::Version);
    }

    #[test]
    fn errors() {
        assert!(matches!(
            p(&["scan"]),
            Err(CliError::MissingArgument { command: "scan", .. })
        ));
        assert!(matches!(p(&["stream"]), Err(CliError::MissingArgument { .. })));
        assert_eq!(p(&["--bogus"]), Err(CliError::UnknownOption("--bogus".into())));
    }
}
