use unicode_normalization::UnicodeNormalization;

pub fn normalize_for_search(value: &str) -> String {
    let compatibility_folded: String = value.nfkc().flat_map(char::to_lowercase).collect();
    let mut normalized = String::with_capacity(compatibility_folded.len());
    let mut pending_space = false;

    for character in compatibility_folded.trim().chars() {
        if character.is_whitespace() {
            pending_space = !normalized.is_empty();
        } else {
            if pending_space {
                normalized.push(' ');
                pending_space = false;
            }
            normalized.push(character);
        }
    }
    normalized.nfc().collect()
}

pub fn search_contains(value: &str, query: &str) -> bool {
    let query = normalize_for_search(query);
    !query.is_empty() && normalize_for_search(value).contains(&query)
}

#[cfg(test)]
mod tests {
    use super::{normalize_for_search, search_contains};

    #[test]
    fn search_is_script_neutral_and_does_not_require_word_boundaries() {
        assert!(search_contains("日曜日は図書館に行きます", "図書館"));
        assert!(search_contains("Café du matin", "ＣＡＦÉ"));
        assert!(search_contains("در کتابخانه", "کتاب"));
        assert!(search_contains("Meeting الساعة 三時", " الساعة "));
        assert!(!search_contains("नमस्ते", ""));
        assert_eq!(normalize_for_search("  A\t\nＢ  "), "a b");
    }
}
