//! Golden/integration test: scan the committed CRLF fixture and assert the parsed
//! rows, exercising the full Phase 1 line pipeline (CRLF strip → regex → blank skip
//! → bad-match warn-and-skip) without any DB/Redis/live-file dependency.
//!
//! Note: the boot query and runner orchestration need a live DB, so this test
//! drives the pure parsing layer directly via the public `parse` API. The
//! end-to-end runner is exercised manually against the container (see PHASE_LOG).

use std::io::{BufRead, BufReader};

use siemens_rpp::parse::scan_lines;

/// Read the fixture exactly as the runner does: BufRead lines, strip a trailing
/// `\r` (TD-016).
fn read_fixture_lines(path: &str) -> Vec<String> {
    let file = std::fs::File::open(path).expect("fixture exists");
    BufReader::new(file)
        .lines()
        .map(|l| {
            let l = l.expect("readable line");
            l.strip_suffix('\r').unwrap_or(&l).to_string()
        })
        .collect()
}

#[test]
fn sample_ct_fixture_parses_expected_rows() {
    let lines = read_fixture_lines(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/sample_ct.log"
    ));

    let result = scan_lines(lines.iter().map(String::as_str), None, "re_v1", "SME21862")
        .expect("scan succeeds");

    // 3 valid lines → 3 rows; the blank line is skipped; the bad line is counted.
    assert_eq!(result.rows.len(), 3, "expected 3 parsed rows");
    assert_eq!(result.bad_match_count, 1, "one non-blank unmatched line");
    assert!(!result.cursor_hit);

    // CRLF was stripped: no trailing '\r' anywhere in the captured fields.
    for row in &result.rows {
        assert!(
            !row.text_group.as_deref().unwrap_or("").ends_with('\r'),
            "text_group must not retain a trailing CR"
        );
    }

    // Spot-check first + last matched rows.
    let first = &result.rows[0];
    assert_eq!(first.system_id, "SME21862");
    assert_eq!(first.host_state.as_deref(), Some("I"));
    assert_eq!(first.type_group.as_deref(), Some("3119"));
    assert_eq!(
        first.text_group.as_deref(),
        Some("Control info MCU (E 00 03 77 00 4e 00 a4)")
    );
    // Uncaptured-by-regex fields stay null, matching Node mapDataToSchema.
    assert_eq!(first.domain_group, None);
    assert_eq!(first.month, None);
    assert_eq!(first.host_datetime, None);

    let second = &result.rows[1];
    assert_eq!(second.host_state.as_deref(), Some("Wr"));
    assert_eq!(second.text_group.as_deref(), Some("WCS internal error"));
}
