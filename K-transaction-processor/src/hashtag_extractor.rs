use base64::{Engine as _, engine::general_purpose};
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashSet;
use tracing::warn;

// Compile regex patterns once at startup
static VALID_HASHTAG_PATTERN: Lazy<Regex> = Lazy::new(|| {
    // Match just the hashtag pattern itself - we'll validate boundaries manually
    Regex::new(r"#[\p{L}\p{N}_]{1,30}").unwrap()
});

static ALL_HASH_PATTERNS: Lazy<Regex> = Lazy::new(|| Regex::new(r"#[^\s]+").unwrap());

/// Extract hashtags from a base64-encoded message
/// Returns a vector of unique hashtags (lowercase, without # prefix)
pub fn extract_hashtags_from_base64(base64_message: &str) -> Vec<String> {
    // 1. Decode base64
    let decoded_bytes = match general_purpose::STANDARD.decode(base64_message) {
        Ok(bytes) => bytes,
        Err(e) => {
            warn!("Failed to decode base64 message: {}", e);
            return vec![];
        }
    };

    let decoded_text = match String::from_utf8(decoded_bytes) {
        Ok(text) => text,
        Err(e) => {
            warn!("Failed to convert decoded bytes to UTF-8: {}", e);
            return vec![];
        }
    };

    // 2. Pass 1: Extract valid hashtags (with Unicode support)
    let mut valid_hashtags = HashSet::new();

    // Use find_iter to get all matches and manually validate boundaries
    for mat in VALID_HASHTAG_PATTERN.find_iter(&decoded_text) {
        let start_pos = mat.start();
        let end_pos = mat.end();

        // Check if there's a valid character before the hashtag
        let valid_before = if start_pos == 0 {
            true // Start of string is valid
        } else {
            // Get the character before the #
            let chars_before: Vec<char> = decoded_text[..start_pos].chars().collect();
            if let Some(&prev_char) = chars_before.last() {
                prev_char.is_whitespace() // Must be whitespace before
            } else {
                false
            }
        };

        // Check if there's a valid character after the hashtag
        let valid_after = if end_pos >= decoded_text.len() {
            true // End of string is valid
        } else {
            // Get the character after the hashtag
            let chars_after: Vec<char> = decoded_text[end_pos..].chars().collect();
            if let Some(&next_char) = chars_after.first() {
                next_char.is_whitespace() || ".,;!?".contains(next_char) // Must be whitespace or punctuation
            } else {
                false
            }
        };

        // Only add if both boundaries are valid
        if valid_before && valid_after {
            let hashtag = &mat.as_str()[1..]; // Remove the # prefix
            valid_hashtags.insert(hashtag.to_lowercase());
        }
    }

    // 3. Pass 2: Detect and warn about invalid patterns
    for capture in ALL_HASH_PATTERNS.captures_iter(&decoded_text) {
        let full_match = capture.get(0).unwrap().as_str();
        let tag_part = &full_match[1..]; // Remove the '#'

        // Check if this pattern was already captured as valid
        if !valid_hashtags.contains(&tag_part.to_lowercase()) {
            // This is an invalid pattern - determine why and warn
            if tag_part.is_empty() {
                warn!("Invalid hashtag pattern '{}' - empty hashtag", full_match);
            } else if tag_part.len() > 30 {
                warn!(
                    "Invalid hashtag pattern '{}' - exceeds 30 character limit",
                    full_match
                );
            } else if !tag_part.chars().all(|c| c.is_alphanumeric() || c == '_') {
                warn!(
                    "Invalid hashtag pattern '{}' - contains invalid characters",
                    full_match
                );
            } else {
                // Pattern doesn't match our boundary requirements
                warn!(
                    "Invalid hashtag pattern '{}' - invalid boundaries or format",
                    full_match
                );
            }
        }
    }

    // 4. Return unique valid hashtags as Vec
    valid_hashtags.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_single_hashtag() {
        let message = general_purpose::STANDARD.encode("Hello #world");
        let hashtags = extract_hashtags_from_base64(&message);
        assert_eq!(hashtags, vec!["world"]);
    }

    #[test]
    fn test_extract_multiple_hashtags() {
        let message = general_purpose::STANDARD.encode("Check #rust and #programming");
        let mut hashtags = extract_hashtags_from_base64(&message);
        hashtags.sort();
        assert_eq!(hashtags, vec!["programming", "rust"]);
    }

    #[test]
    fn test_hashtag_case_insensitive() {
        let message = general_purpose::STANDARD.encode("#Rust #RUST #rust");
        let hashtags = extract_hashtags_from_base64(&message);
        assert_eq!(hashtags, vec!["rust"]); // Only one unique
    }

    #[test]
    fn test_hashtag_at_start() {
        let message = general_purpose::STANDARD.encode("#start of message");
        let hashtags = extract_hashtags_from_base64(&message);
        assert_eq!(hashtags, vec!["start"]);
    }

    #[test]
    fn test_hashtag_at_end() {
        let message = general_purpose::STANDARD.encode("end of message #end");
        let hashtags = extract_hashtags_from_base64(&message);
        assert_eq!(hashtags, vec!["end"]);
    }

    #[test]
    fn test_hashtag_with_punctuation() {
        let message = general_purpose::STANDARD.encode("Hello #world! How are you?");
        let hashtags = extract_hashtags_from_base64(&message);
        assert_eq!(hashtags, vec!["world"]);
    }

    #[test]
    fn test_reject_no_space_before() {
        let message = general_purpose::STANDARD.encode("word#tag");
        let hashtags = extract_hashtags_from_base64(&message);
        assert!(hashtags.is_empty());
    }

    #[test]
    fn test_reject_url_with_hash() {
        let message = general_purpose::STANDARD.encode("Visit google.com#section");
        let hashtags = extract_hashtags_from_base64(&message);
        assert!(hashtags.is_empty());
    }

    #[test]
    fn test_reject_too_long() {
        let message =
            general_purpose::STANDARD.encode("#thistagiswaytoolongandshouldbrejected123456");
        let hashtags = extract_hashtags_from_base64(&message);
        assert!(hashtags.is_empty());
    }

    #[test]
    fn test_accept_max_length() {
        let message = general_purpose::STANDARD.encode("#a123456789012345678901234567890"); // 31 chars
        let hashtags = extract_hashtags_from_base64(&message);
        // This should be rejected because it's 31 chars (> 30)
        assert!(hashtags.is_empty());
    }

    #[test]
    fn test_accept_exactly_30_chars() {
        let message = general_purpose::STANDARD.encode("#a12345678901234567890123456789"); // exactly 30 chars
        let hashtags = extract_hashtags_from_base64(&message);
        assert_eq!(hashtags, vec!["a12345678901234567890123456789"]);
    }

    #[test]
    fn test_numeric_hashtags() {
        let message = general_purpose::STANDARD.encode("#2024 and #123");
        let mut hashtags = extract_hashtags_from_base64(&message);
        hashtags.sort();
        assert_eq!(hashtags, vec!["123", "2024"]);
    }

    #[test]
    fn test_empty_message() {
        let message = general_purpose::STANDARD.encode("");
        let hashtags = extract_hashtags_from_base64(&message);
        assert!(hashtags.is_empty());
    }

    #[test]
    fn test_no_hashtags() {
        let message = general_purpose::STANDARD.encode("This message has no hashtags");
        let hashtags = extract_hashtags_from_base64(&message);
        assert!(hashtags.is_empty());
    }

    #[test]
    fn test_just_hash_symbol() {
        let message = general_purpose::STANDARD.encode("Just a # symbol");
        let hashtags = extract_hashtags_from_base64(&message);
        assert!(hashtags.is_empty());
    }

    #[test]
    fn test_mixed_valid_and_invalid_patterns() {
        // Message contains both valid hashtags and invalid patterns
        // Valid: #rust, #programming
        // Invalid: word#tag, #verylongtagthatshouldberejectedbecauseitstoolong
        let message = general_purpose::STANDARD.encode("#rust and word#tag plus #programming and #verylongtagthatshouldberejectedbecauseitstoolong");
        let hashtags = extract_hashtags_from_base64(&message);

        // Should return only the valid hashtags, warnings logged for invalid ones
        assert_eq!(hashtags.len(), 2);
        assert!(hashtags.contains(&"rust".to_string()));
        assert!(hashtags.contains(&"programming".to_string()));
        // Invalid patterns should NOT be in the result
        assert!(!hashtags.contains(&"tag".to_string()));
        assert!(
            !hashtags.contains(&"verylongtagthatshouldberejectedbecauseitstoolong".to_string())
        );
    }

    #[test]
    fn test_all_invalid_patterns() {
        // Message contains only invalid patterns
        let message = general_purpose::STANDARD.encode("word#tag and another#invalid");
        let hashtags = extract_hashtags_from_base64(&message);

        // Should return empty, but warnings logged (not failing)
        assert!(hashtags.is_empty());
    }

    #[test]
    fn test_unicode_hashtags_latin() {
        let message = general_purpose::STANDARD.encode("Bonjour #café et #résumé");
        let mut hashtags = extract_hashtags_from_base64(&message);
        hashtags.sort();
        assert_eq!(hashtags.len(), 2);
        assert!(hashtags.contains(&"café".to_string()));
        assert!(hashtags.contains(&"résumé".to_string()));
    }

    #[test]
    fn test_unicode_hashtags_cyrillic() {
        let message = general_purpose::STANDARD.encode("Привет #москва and #русский");
        let hashtags = extract_hashtags_from_base64(&message);
        assert_eq!(hashtags.len(), 2);
        assert!(hashtags.contains(&"москва".to_string()));
        assert!(hashtags.contains(&"русский".to_string()));
    }

    #[test]
    fn test_unicode_hashtags_japanese() {
        let message = general_purpose::STANDARD.encode("こんにちは #日本語 and #東京");
        let hashtags = extract_hashtags_from_base64(&message);
        assert_eq!(hashtags.len(), 2);
        assert!(hashtags.contains(&"日本語".to_string()));
        assert!(hashtags.contains(&"東京".to_string()));
    }

    #[test]
    fn test_unicode_hashtags_chinese() {
        let message = general_purpose::STANDARD.encode("你好 #中文 and #北京");
        let hashtags = extract_hashtags_from_base64(&message);
        assert_eq!(hashtags.len(), 2);
        assert!(hashtags.contains(&"中文".to_string()));
        assert!(hashtags.contains(&"北京".to_string()));
    }

    #[test]
    fn test_unicode_hashtags_arabic() {
        let message = general_purpose::STANDARD.encode("مرحبا #العربية and #القاهرة");
        let hashtags = extract_hashtags_from_base64(&message);
        assert_eq!(hashtags.len(), 2);
        assert!(hashtags.contains(&"العربية".to_string()));
        assert!(hashtags.contains(&"القاهرة".to_string()));
    }

    #[test]
    fn test_hashtag_with_underscore() {
        let message = general_purpose::STANDARD.encode("Check #rust_lang and #web_dev");
        let hashtags = extract_hashtags_from_base64(&message);
        assert_eq!(hashtags.len(), 2);
        assert!(hashtags.contains(&"rust_lang".to_string()));
        assert!(hashtags.contains(&"web_dev".to_string()));
    }

    #[test]
    fn test_mixed_unicode_hashtags() {
        let message = general_purpose::STANDARD.encode("#rust #café #日本語 #москва");
        let hashtags = extract_hashtags_from_base64(&message);
        assert_eq!(hashtags.len(), 4);
        assert!(hashtags.contains(&"rust".to_string()));
        assert!(hashtags.contains(&"café".to_string()));
        assert!(hashtags.contains(&"日本語".to_string()));
        assert!(hashtags.contains(&"москва".to_string()));
    }
}
