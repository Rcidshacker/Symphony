//! A stand-in for yt-dlp used by the streaming tests: no network, deterministic output.
//!
//! - `--version`                prints a version
//! - `ytsearchN:query ...`      three results (durations 60 / 120 / 180 s); the query "broken" finds one broken video
//! - `... -o <path> <url>`      "downloads" by copying $FAKE_YTDLP_AUDIO to `<path>`
//! - `<url>` with `-j`          info for the video id in the url (id "broken" fails like an unavailable video)
//!
//! Every call's arguments are appended to the file named by $FAKE_YTDLP_LOG.

use std::io::Write;
use std::{env, fs, process::exit};

use serde_json::json;

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if let Ok(log) = env::var("FAKE_YTDLP_LOG") {
        if let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(log) {
            let _ = writeln!(file, "{}", args.join(" "));
        }
    }

    if args.iter().any(|a| a == "--version") {
        println!("2099.01.01");
        return;
    }

    if let Some(search) = args.iter().find_map(|a| a.strip_prefix("ytsearch")) {
        let (count, query) = search.split_once(':').unwrap_or(("3", search));
        let count: usize = count.parse().unwrap_or(3);
        let slug: String = query
            .to_lowercase()
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '_' })
            .collect();
        if query == "broken" {
            println!(
                "{}",
                json!({"id": "broken", "title": "Broken", "channel": "Nobody", "duration": 60})
            );
            return;
        }
        for (i, duration) in [60, 120, 180].iter().take(count).enumerate() {
            println!(
                "{}",
                json!({
                    "id": format!("{slug}-{duration}"),
                    "title": format!("{query} #{}", i + 1),
                    "channel": "Fake Channel",
                    "duration": duration,
                })
            );
        }
        return;
    }

    let url = args.last().cloned().unwrap_or_default();
    let id = url.split("v=").nth(1).unwrap_or("unknown").to_string();
    if id == "broken" {
        eprintln!("ERROR: [youtube] broken: Video unavailable");
        exit(1);
    }

    if let Some(pos) = args.iter().position(|a| a == "-o") {
        let out = &args[pos + 1];
        if let Some(parent) = std::path::Path::new(out).parent() {
            let _ = fs::create_dir_all(parent);
        }
        let source = env::var("FAKE_YTDLP_AUDIO").expect("FAKE_YTDLP_AUDIO");
        fs::copy(source, out).expect("copy audio");
        return;
    }

    if args.iter().any(|a| a == "--get-url") {
        println!("http://127.0.0.1:9/audio.m4a");
        return;
    }

    println!(
        "{}",
        json!({"id": id, "title": format!("Fake Video {id}"), "channel": "Fake Channel", "duration": 2})
    );
}
