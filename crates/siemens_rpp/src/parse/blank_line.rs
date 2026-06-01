//! Node's `blankLineTest` (`tooling/regExHelpers.js:30-33`): `/^[ \t\n]*$/`.
//! A line that is empty or only spaces/tabs/newline is "blank" → skipped silently.
//! A non-blank line that fails the parser regex is a "Bad Match" → warn-and-skip
//! (TD-017).

use std::sync::OnceLock;

use regex::Regex;

fn blank_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    // Mirrors JS /^[ \t\n]*$/ exactly. (Lines are already \n/\r-stripped by the
    // scanner, but the \n in the class is kept for byte-faithful behavior.)
    RE.get_or_init(|| Regex::new(r"^[ \t\n]*$").unwrap())
}

/// True if the line is blank per Node's test.
pub fn is_blank(line: &str) -> bool {
    blank_re().is_match(line)
}

#[cfg(test)]
mod tests {
    use super::is_blank;

    #[test]
    fn blank_cases() {
        assert!(is_blank(""));
        assert!(is_blank("   "));
        assert!(is_blank("\t\t"));
        assert!(is_blank(" \t "));
    }

    #[test]
    fn non_blank_cases() {
        assert!(!is_blank("x"));
        assert!(!is_blank("  a  "));
        assert!(!is_blank("I\t2026-06-01"));
    }
}
