//! win_10 line parsing. Regexes are ported verbatim from the Node
//! `parse/parsers.js` `win_10_re` (only `(?<name>)` → `(?P<name>)` for the Rust
//! `regex` crate). Both variants are TSV-shaped; `re_v1` and `re_v2` differ only in
//! whether `host_time` or `host_state` comes first.

use std::sync::OnceLock;

use regex::Regex;

use super::blank_line::is_blank;
use crate::types::Row;

/// re_v1: `<host_state>\t<host_date>\t<host_time>\t<source>\t<type>\t<text>`
fn re_v1() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?P<host_state>\w+)\t(?P<host_date>\d{4}-\d{1,2}-\d{1,2})\t(?P<host_time>\d{2}:\d{2}:\d{2})\t(?P<source_group>(.*?(\d+)?)(\.\d\.\d)?)\t?\s?(?P<type_group>(\d{1,5}))\t(?P<text_group>.*)",
        )
        .expect("re_v1 is a valid regex")
    })
}

/// re_v2: `<host_time>\t<host_state>\t<host_date>\t<source>\t<type>\t<text>`
fn re_v2() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?P<host_time>\d{2}:\d{2}:\d{2})\t(?P<host_state>\w+)\t(?P<host_date>\d{4}-\d{1,2}-\d{1,2})\t(?P<source_group>(.*?(\d+)?)(\.\d\.\d)?)\t?\s?(?P<type_group>(\d{1,5}))\t(?P<text_group>.*)",
        )
        .expect("re_v2 is a valid regex")
    })
}

/// Select the regex variant from `file_config.parsers[0]` (e.g. `"re_v1"`).
/// Unknown names error rather than silently defaulting.
pub fn select_regex(parser_name: &str) -> Result<&'static Regex, String> {
    match parser_name {
        "re_v1" => Ok(re_v1()),
        "re_v2" => Ok(re_v2()),
        other => Err(format!("unknown win_10 parser variant: {other:?}")),
    }
}

/// Outcome of classifying one scanned line against the active regex.
#[derive(Debug, PartialEq, Eq)]
pub enum LineOutcome {
    /// Parsed into a row.
    Matched(Box<Row>),
    /// Blank line (Node `blankLineTest`) — skipped silently.
    Blank,
    /// Non-blank, no regex match — "Bad Match": warn-and-skip (TD-017). Carries the
    /// offending line for the WARN note.
    BadMatch(String),
}

/// Classify a single (already `\r`-stripped) line, building a `Row` on a match.
/// `system_id` is stamped onto the row (Node sets `matches.groups.system_id`).
/// `host_datetime`/`capture_datetime` are left `None` here — Phase 2.
pub fn parse_line(re: &Regex, line: &str, system_id: &str) -> LineOutcome {
    match re.captures(line) {
        Some(caps) => {
            let g = |name: &str| caps.name(name).map(|m| m.as_str().to_string());
            LineOutcome::Matched(Box::new(Row {
                system_id: system_id.to_string(),
                host_state: g("host_state"),
                host_date: g("host_date"),
                host_time: g("host_time"),
                source_group: g("source_group"),
                type_group: g("type_group"),
                text_group: g("text_group"),
                // Not captured by win_10 regex → NULL, matching Node.
                domain_group: None,
                id_group: None,
                month: None,
                day: None,
                year: None,
                // Datetime work is Phase 2.
                host_datetime: None,
                capture_datetime: None,
            }))
        }
        None if is_blank(line) => LineOutcome::Blank,
        None => LineOutcome::BadMatch(line.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_V1: &str =
        "I\t2026-06-01\t08:44:29\tCT_MCU\t3119\tControl info MCU (E 00 03 77 00 4e 00 a4)";

    #[test]
    fn re_v1_matches_real_line() {
        let re = select_regex("re_v1").unwrap();
        match parse_line(re, SAMPLE_V1, "SME21862") {
            LineOutcome::Matched(row) => {
                assert_eq!(row.system_id, "SME21862");
                assert_eq!(row.host_state.as_deref(), Some("I"));
                assert_eq!(row.host_date.as_deref(), Some("2026-06-01"));
                assert_eq!(row.host_time.as_deref(), Some("08:44:29"));
                assert_eq!(row.type_group.as_deref(), Some("3119"));
                assert_eq!(
                    row.text_group.as_deref(),
                    Some("Control info MCU (E 00 03 77 00 4e 00 a4)")
                );
                // Uncaptured fields stay None (→ null), per Node.
                assert_eq!(row.domain_group, None);
                assert_eq!(row.month, None);
                assert_eq!(row.host_datetime, None);
            }
            other => panic!("expected Matched, got {other:?}"),
        }
    }

    #[test]
    fn blank_line_is_blank() {
        let re = select_regex("re_v1").unwrap();
        assert_eq!(parse_line(re, "   ", "SME"), LineOutcome::Blank);
        assert_eq!(parse_line(re, "", "SME"), LineOutcome::Blank);
    }

    #[test]
    fn non_blank_unmatched_is_bad_match() {
        let re = select_regex("re_v1").unwrap();
        match parse_line(re, "this is not a tsv line", "SME") {
            LineOutcome::BadMatch(l) => assert_eq!(l, "this is not a tsv line"),
            other => panic!("expected BadMatch, got {other:?}"),
        }
    }

    #[test]
    fn re_v2_time_first() {
        let re = select_regex("re_v2").unwrap();
        let line = "08:44:29\tI\t2026-06-01\tCT_MCU\t3119\tsome text";
        match parse_line(re, line, "SME") {
            LineOutcome::Matched(row) => {
                assert_eq!(row.host_time.as_deref(), Some("08:44:29"));
                assert_eq!(row.host_state.as_deref(), Some("I"));
                assert_eq!(row.host_date.as_deref(), Some("2026-06-01"));
            }
            other => panic!("expected Matched, got {other:?}"),
        }
    }

    #[test]
    fn unknown_parser_errors() {
        assert!(select_regex("re_v9").is_err());
    }
}
