//! Local shell arguments and names (v0.2.15 `terminal/localShell.ts`).

use crate::text::is_js_space;

/// Splits typed arguments on spaces like a shell: single or double quotes keep spaces in one
/// argument (and are removed); backslashes are kept, so Windows paths need no escaping.
pub fn split_args(text: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut started = false;
    let mut quote: Option<char> = None;
    for ch in text.chars() {
        if let Some(open) = quote {
            if ch == open {
                quote = None;
            } else {
                current.push(ch);
            }
        } else if ch == '"' || ch == '\'' {
            quote = Some(ch);
            started = true;
        } else if is_js_space(ch) {
            if started {
                args.push(std::mem::take(&mut current));
            }
            current.clear();
            started = false;
        } else {
            current.push(ch);
            started = true;
        }
    }
    if started {
        args.push(current);
    }
    args
}

/// Joins arguments back into text that [`split_args`] reads the same way. An argument with
/// both quote kinds is double-quoted with each `"` written as `"'"'"` (close, quoted ", reopen).
pub fn join_args<S: AsRef<str>>(args: &[S]) -> String {
    let quote = |arg: &str| {
        if !arg.is_empty() && !arg.chars().any(|c| is_js_space(c) || c == '"' || c == '\'') {
            arg.to_string()
        } else if !arg.contains('"') {
            format!("\"{arg}\"")
        } else if !arg.contains('\'') {
            format!("'{arg}'")
        } else {
            format!("\"{}\"", arg.split('"').collect::<Vec<_>>().join("\"'\"'\""))
        }
    };
    args.iter().map(|a| quote(a.as_ref())).collect::<Vec<_>>().join(" ")
}

/// The file name of a program without its directory or a `.exe` extension (any case).
pub fn shell_name_of(program: &str) -> String {
    let file = crate::text::js_trim(program).rsplit(['/', '\\']).next().unwrap_or("");
    match file
        .len()
        .checked_sub(4)
        .and_then(|i| file.get(i..).map(|ext| (i, ext)))
    {
        Some((i, ext)) if ext.eq_ignore_ascii_case(".exe") => file.get(..i).unwrap_or(file).to_string(),
        _ => file.to_string(),
    }
}
