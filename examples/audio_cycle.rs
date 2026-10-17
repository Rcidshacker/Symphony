//! TEMPORARY diagnostic: does opening and dropping audio engines repeatedly crash on a CI runner?
use std::time::Duration;
use symphony::audio::AudioEngine;

fn main() {
    let wav = std::env::args().nth(1).expect("wav path");
    for i in 1..=8 {
        eprintln!("cycle {i}: creating engine");
        match AudioEngine::new() {
            Err(e) => {
                eprintln!("cycle {i}: no engine: {e}");
                return;
            }
            Ok(mut engine) => {
                engine.set_volume(0.0);
                eprintln!("cycle {i}: playing");
                let r = engine.play_file(std::path::Path::new(&wav));
                eprintln!("cycle {i}: play -> {:?}", r.is_ok());
                std::thread::sleep(Duration::from_millis(300));
                eprintln!("cycle {i}: dropping");
            }
        }
    }
    eprintln!("all cycles done");
}
