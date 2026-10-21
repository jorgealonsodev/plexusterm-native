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
