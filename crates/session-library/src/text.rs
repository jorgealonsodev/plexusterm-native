//! JavaScript string semantics the v0.2.15 parsers rely on.

/// `\s` of a JavaScript regular expression, which is also what `String.prototype.trim` removes:
/// Unicode spaces, line terminators and U+FEFF, but not U+0085 (Rust's `is_whitespace` differs
/// on those two).
pub(crate) fn is_js_space(c: char) -> bool {
    c == '\u{feff}' || (c != '\u{85}' && c.is_whitespace())
}

/// `String.prototype.trim`.
pub(crate) fn js_trim(text: &str) -> &str {
    text.trim_matches(is_js_space)
}

/// `/^\d+$/` (ASCII digits only).
pub(crate) fn all_digits(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit())
}

/// A `/^\d+$/` text read as a port: `None` unless it is within 1..=65535.
pub(crate) fn port_of(text: &str) -> Option<u16> {
    if !all_digits(text) {
        return None;
    }
    // Leading zeros are fine ("0022" is 22); longer texts are out of range anyway.
    let digits = text.trim_start_matches('0');
    if digits.len() > 5 {
        return None;
    }
    match digits.parse::<u32>() {
        Ok(port) if (1..=65535).contains(&port) => u16::try_from(port).ok(),
        _ => None,
    }
}
