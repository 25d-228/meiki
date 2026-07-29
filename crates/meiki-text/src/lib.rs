//! Centralized text comparison primitives.

use meiki_domain::ComparisonResult;
use unicode_normalization::UnicodeNormalization;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Comparison {
    pub result: ComparisonResult,
    pub normalized_response: String,
}

pub fn normalize_for_default_comparison(value: &str) -> String {
    value.trim().nfc().collect()
}

pub fn compare_answer(expected: &str, accepted: &[String], response: &str) -> Comparison {
    let normalized_response = normalize_for_default_comparison(response);
    let result = if normalized_response.is_empty() {
        ComparisonResult::Empty
    } else if normalized_response == normalize_for_default_comparison(expected) {
        ComparisonResult::Exact
    } else if accepted
        .iter()
        .any(|candidate| normalized_response == normalize_for_default_comparison(candidate))
    {
        ComparisonResult::AcceptedVariant
    } else {
        ComparisonResult::Incorrect
    };

    Comparison {
        result,
        normalized_response,
    }
}

#[cfg(test)]
mod tests {
    use meiki_domain::ComparisonResult;

    use super::{compare_answer, normalize_for_default_comparison};

    #[test]
    fn default_comparison_uses_nfc_and_outer_trim_only() {
        assert_eq!(normalize_for_default_comparison("  e\u{301}  "), "\u{e9}");
        assert_eq!(
            compare_answer("Café", &[], " cafe ").result,
            ComparisonResult::Incorrect
        );
    }

    #[test]
    fn accepted_variants_are_explicit() {
        let accepted = vec!["ゆきます".to_owned()];
        assert_eq!(
            compare_answer("行きます", &accepted, "ゆきます").result,
            ComparisonResult::AcceptedVariant
        );
    }

    #[test]
    fn empty_response_is_distinct() {
        assert_eq!(
            compare_answer("行きます", &[], " \n").result,
            ComparisonResult::Empty
        );
    }
}
