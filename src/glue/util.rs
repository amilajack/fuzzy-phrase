use regex;

#[inline(always)]
pub fn contains_digit_or_pound(word: &str) -> bool {
    // we can operate on bytes because all the characters we're looking for are ASCII, and the
    // utf8 encoding guarantees that no valid ASCII bytes occur inside non-ASCII characters
    word.as_bytes().iter().any(|b| {
        (*b >= ('0' as u8) && *b <= ('9' as u8)) ||
        *b == ('#' as u8)
    })
}

#[inline(always)]
pub fn can_fuzzy_match(word: &str, script_regex: &regex::Regex) -> bool {
    if contains_digit_or_pound(word) {
        false
    } else if script_regex.is_match(word) {
        true
    } else {
        false
    }
}

#[test]
fn digit_test() {
    assert!(contains_digit_or_pound("1"));
    assert!(contains_digit_or_pound("78348"));
    assert!(contains_digit_or_pound("#"));
    assert!(contains_digit_or_pound("a0"));
    assert!(contains_digit_or_pound("9Г"));

    assert!(!contains_digit_or_pound(""));
    assert!(!contains_digit_or_pound("!"));
    assert!(!contains_digit_or_pound("hello"));
}