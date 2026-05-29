//! Event `type` and `tag` enums. The serialized string values MUST stay identical
//! to the Node `utils/logger/enums.js` constants so existing dashboards and log
//! consumers keep working unchanged.

use serde::{Deserialize, Serialize};

/// Event severity. Serializes to the exact uppercase Node strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Type {
    #[serde(rename = "INFO")]
    Info,
    #[serde(rename = "WARN")]
    Warn,
    #[serde(rename = "ERROR")]
    Error,
}

impl Type {
    /// True for the severities included in `warn_error_logs`.
    pub fn is_warn_or_error(self) -> bool {
        matches!(self, Type::Warn | Type::Error)
    }
}

/// Event tag. Serializes to the exact Node strings (note the spaces in the last two).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Tag {
    #[serde(rename = "CALL")]
    Call,
    #[serde(rename = "DETAILS")]
    Details,
    #[serde(rename = "CATCH")]
    Catch,
    #[serde(rename = "SEQUENCE HALTED")]
    SequenceHalted,
    #[serde(rename = "QA FAILURE")]
    QaFailure,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_serializes_to_node_strings() {
        assert_eq!(serde_json::to_string(&Type::Info).unwrap(), "\"INFO\"");
        assert_eq!(serde_json::to_string(&Type::Warn).unwrap(), "\"WARN\"");
        assert_eq!(serde_json::to_string(&Type::Error).unwrap(), "\"ERROR\"");
    }

    #[test]
    fn tag_serializes_to_node_strings() {
        assert_eq!(serde_json::to_string(&Tag::Call).unwrap(), "\"CALL\"");
        assert_eq!(serde_json::to_string(&Tag::Details).unwrap(), "\"DETAILS\"");
        assert_eq!(serde_json::to_string(&Tag::Catch).unwrap(), "\"CATCH\"");
        assert_eq!(
            serde_json::to_string(&Tag::SequenceHalted).unwrap(),
            "\"SEQUENCE HALTED\""
        );
        assert_eq!(
            serde_json::to_string(&Tag::QaFailure).unwrap(),
            "\"QA FAILURE\""
        );
    }

    #[test]
    fn warn_and_error_are_filtered() {
        assert!(!Type::Info.is_warn_or_error());
        assert!(Type::Warn.is_warn_or_error());
        assert!(Type::Error.is_warn_or_error());
    }
}
