//! Lyrics translation through the configured AI provider.
//!
//! The model gets numbered lines and must answer `N|translation`; parsing tolerates the usual
//! deviations (`N.`/`N:`/`N)` separators, code fences, preambles, or no numbering at all).

use regex::Regex;

use crate::ai::{AIManager, AIProviderConfig};

/// Lines translated per request (the provider caps the length of a reply)
const CHUNK: usize = 20;

/// Prompt asking for a translation of `lines` into `language`
pub fn translation_prompt(language: &str, lines: &[String]) -> String {
    let mut prompt = format!(
        "Translate these song lyric lines into {language}. Reply with exactly one line per input \
         line, in the form `N|translation`, keeping the numbering. Do not add commentary, quotes \
         or blank lines.\n"
    );
    for (i, line) in lines.iter().enumerate() {
        prompt.push_str(&format!("{}|{}\n", i + 1, line));
    }
    prompt
}

/// Map a model reply back to the `n` input lines (`None` where a line is missing)
pub fn parse_translations(response: &str, n: usize) -> Vec<Option<String>> {
    let numbered = Regex::new(r"^\s*(\d+)\s*[|.:)\-]\s*(.+?)\s*$").expect("valid regex");

    let mut out = vec![None; n];
    let mut found = 0;
    for line in response.lines() {
        if let Some(caps) = numbered.captures(line) {
            let index: usize = caps[1].parse().unwrap_or(0);
            if (1..=n).contains(&index) && out[index - 1].is_none() {
                out[index - 1] = Some(caps[2].to_string());
                found += 1;
            }
        }
    }
    if found > 0 {
        return out;
    }

    // The model ignored the numbering: trust the order if the line count matches
    let plain: Vec<&str> = response
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with("```"))
        .collect();
    if plain.len() == n {
        return plain.into_iter().map(|l| Some(l.to_string())).collect();
    }
    out
}

/// Translate all `lines`, a chunk per request
pub async fn translate_lines(
    provider: AIProviderConfig,
    language: &str,
    lines: &[String],
) -> Result<Vec<Option<String>>, String> {
    let manager = AIManager::new(provider).await.map_err(|e| e.to_string())?;

    let mut translated = vec![None; lines.len()];
    for (chunk_index, chunk) in lines.chunks(CHUNK).enumerate() {
        let reply = manager
            .respond(&translation_prompt(language, chunk))
            .await
            .map_err(|e| e.to_string())?;
        for (i, translation) in parse_translations(&reply, chunk.len())
            .into_iter()
            .enumerate()
        {
            translated[chunk_index * CHUNK + i] = translation;
        }
    }
    Ok(translated)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_numbers_the_lines() {
        let prompt = translation_prompt("French", &["Hello".into(), "World".into()]);
        assert!(prompt.contains("into French"));
        assert!(prompt.contains("\n1|Hello\n2|World\n"));
    }

    #[test]
    fn parses_the_requested_format() {
        let got = parse_translations("1|Bonjour\n2|Monde\n3|Encore", 3);
        assert_eq!(
            got,
            vec![
                Some("Bonjour".into()),
                Some("Monde".into()),
                Some("Encore".into())
            ]
        );
    }

    #[test]
    fn tolerates_other_separators_fences_and_preambles() {
        let reply = "Here is the translation:\n```\n1. Bonjour\n2: Monde\n3) Encore\n```";
        let got = parse_translations(reply, 3);
        assert_eq!(
            got,
            vec![
                Some("Bonjour".into()),
                Some("Monde".into()),
                Some("Encore".into())
            ]
        );
    }

    #[test]
    fn missing_and_out_of_range_lines_stay_empty() {
        let got = parse_translations("1|Bonjour\n3|Encore\n9|Ignored", 3);
        assert_eq!(
            got,
            vec![Some("Bonjour".into()), None, Some("Encore".into())]
        );
    }

    #[test]
    fn unnumbered_replies_are_used_by_position_when_the_count_matches() {
        assert_eq!(
            parse_translations("Bonjour\nMonde", 2),
            vec![Some("Bonjour".into()), Some("Monde".into())]
        );
        // wrong count: do not guess
        assert_eq!(
            parse_translations("Bonjour\nMonde\nExtra", 2),
            vec![None, None]
        );
    }
}
