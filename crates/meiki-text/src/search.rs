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

    #[test]
    #[ignore = "release performance budget; run with scripts/performance"]
    fn release_budget_multiscript_search_fixture() {
        let values = [
            "日曜日は図書館に行きます",
            "أنا أقرأ كتابًا في المكتبة",
            "मैं पुस्तक पढ़ता हूँ",
            "Réviser le café",
            "Meetingは الساعة 三時",
            "👨‍👩‍👧‍👦 family",
        ];
        let records = (0..250_000)
            .map(|index| format!("{} {index}", values[index % values.len()]))
            .collect::<Vec<_>>();

        let started = std::time::Instant::now();
        let matches = records
            .iter()
            .filter(|record| search_contains(record, "كتاب"))
            .count();
        let elapsed = started.elapsed();

        assert_eq!(matches, 41_667);
        assert!(
            elapsed <= std::time::Duration::from_secs(5),
            "250,000-record multilingual search exceeded 5 s: {elapsed:?}"
        );
        eprintln!(
            "release-budget multiscript_search_250000 elapsed_ms={}",
            elapsed.as_millis()
        );
    }
}
