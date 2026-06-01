//! Parsing: win_10 regex selection + per-line classification, and the file scan
//! that turns a log file into a `Vec<Row>` with the Node cursor/blank/bad-match
//! semantics.

mod blank_line;
mod win10;

pub use win10::{parse_line, select_regex, LineOutcome};

use crate::types::Row;

/// Result of scanning a whole file.
#[derive(Debug, Default)]
pub struct ScanResult {
    pub rows: Vec<Row>,
    /// The file's first line (the would-be new Redis cursor). `None` for an empty
    /// file.
    pub first_line: Option<String>,
    /// Count of non-blank lines that failed the regex (TD-017 warn-and-skip).
    pub bad_match_count: usize,
    /// True if the scan stopped because it hit the previous cursor line.
    pub cursor_hit: bool,
}

/// Scan already-read file content top→bottom, mirroring the Node loop
/// (`jobs/win_10/siemens_ct.js`):
///
/// - line 1 is captured as `first_line` (new cursor),
/// - a line equal to `redis_line` stops the scan (cursor hit; that line and the
///   rest are old data),
/// - otherwise classify: matched → row; blank → skip; non-blank no-match → warn
///   counter + skip (the caller logs the WARN).
///
/// CRLF handling (TD-016): each line must already have its trailing `\r` stripped
/// by the caller so comparisons and stored cursor values match Node `readline`.
pub fn scan_lines<'a, I>(
    lines: I,
    redis_line: Option<&str>,
    parser_name: &str,
    system_id: &str,
) -> Result<ScanResult, String>
where
    I: IntoIterator<Item = &'a str>,
{
    let re = select_regex(parser_name)?;
    let mut out = ScanResult::default();

    for (idx, line) in lines.into_iter().enumerate() {
        if idx == 0 {
            out.first_line = Some(line.to_string());
        }
        // Cursor hit → stop; everything from here down is already-parsed history.
        if redis_line == Some(line) {
            out.cursor_hit = true;
            break;
        }
        match parse_line(re, line, system_id) {
            LineOutcome::Matched(row) => out.rows.push(*row),
            LineOutcome::Blank => {}
            LineOutcome::BadMatch(_) => out.bad_match_count += 1,
        }
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines() -> Vec<&'static str> {
        vec![
            "I\t2026-06-01\t08:44:29\tCT_MCU\t3119\tnewest",
            "",                     // blank → skip
            "garbage non-tsv line", // bad match → counted, skipped
            "I\t2026-06-01\t08:43:39\tCT_MCU\t3119\tolder",
        ]
    }

    #[test]
    fn scans_rows_first_line_and_bad_count() {
        let r = scan_lines(lines(), None, "re_v1", "SME21862").unwrap();
        assert_eq!(r.rows.len(), 2);
        assert_eq!(
            r.first_line.as_deref(),
            Some("I\t2026-06-01\t08:44:29\tCT_MCU\t3119\tnewest")
        );
        assert_eq!(r.bad_match_count, 1);
        assert!(!r.cursor_hit);
        assert_eq!(r.rows[0].text_group.as_deref(), Some("newest"));
    }

    #[test]
    fn stops_at_cursor_line() {
        // Cursor = the 4th line; scan should stop there and only collect the first row.
        let cursor = "I\t2026-06-01\t08:43:39\tCT_MCU\t3119\tolder";
        let r = scan_lines(lines(), Some(cursor), "re_v1", "SME21862").unwrap();
        assert!(r.cursor_hit);
        assert_eq!(r.rows.len(), 1);
        assert_eq!(r.rows[0].text_group.as_deref(), Some("newest"));
    }

    #[test]
    fn empty_file_yields_no_rows_no_first_line() {
        let empty: Vec<&str> = vec![];
        let r = scan_lines(empty, None, "re_v1", "SME21863").unwrap();
        assert_eq!(r.rows.len(), 0);
        assert_eq!(r.first_line, None);
        assert_eq!(r.bad_match_count, 0);
    }
}
