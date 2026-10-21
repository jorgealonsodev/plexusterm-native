//! Sessions `.ini` files: only `[Bookmarks]` / `[Bookmarks_N]` are read (v0.2.15 `iniImport.ts`).

use super::{IMPORT_ROOT, ImportedSession, ParsedImport};
use crate::library::normalize_folder;
use crate::text::{all_digits, js_trim, port_of};

const SSH_TYPE: u64 = 109;
const TELNET_TYPE: u64 = 98;

fn type_label(kind: u64) -> Option<&'static str> {
    match kind {
        1 => Some("RDP"),
        2 => Some("VNC"),
        131 => Some("Serial"),
        105 => Some("WSL"),
        4 => Some("FTP"),
        _ => None,
    }
}

/// Windows-1252 code points of bytes 0x80..=0x9F (the WHATWG table; undefined bytes map to the
/// C1 control of the same value). Every other byte is its Latin-1 code point.
const CP1252_HIGH: [char; 32] = [
    '\u{20ac}', '\u{81}', '\u{201a}', '\u{192}', '\u{201e}', '\u{2026}', '\u{2020}', '\u{2021}', '\u{2c6}', '\u{2030}',
    '\u{160}', '\u{2039}', '\u{152}', '\u{8d}', '\u{17d}', '\u{8f}', '\u{90}', '\u{2018}', '\u{2019}', '\u{201c}',
    '\u{201d}', '\u{2022}', '\u{2013}', '\u{2014}', '\u{2dc}', '\u{2122}', '\u{161}', '\u{203a}', '\u{153}', '\u{9d}',
    '\u{17e}', '\u{178}',
];

fn windows_1252(byte: u8) -> char {
    match byte {
        0x80..=0x9f => CP1252_HIGH
            .get(usize::from(byte - 0x80))
            .copied()
            .unwrap_or(char::from(byte)),
        _ => char::from(byte),
    }
}

/// These files are often in the Windows ANSI code page; newer ones may be UTF-8 (with or
/// without a BOM): strict UTF-8 first, else Windows-1252, and a leading BOM dropped.
pub fn decode_ini(bytes: &[u8]) -> String {
    let text = match std::str::from_utf8(bytes) {
        Ok(text) => text.to_string(),
        Err(_) => bytes.iter().copied().map(windows_1252).collect(),
    };
    match text.strip_prefix('\u{feff}') {
        Some(rest) => rest.to_string(),
        None => text,
    }
}

/// `[Bookmarks]` or `[Bookmarks_<digits>]`, any case.
fn is_bookmarks_section(line: &str) -> bool {
    let Some(inner) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) else {
        return false;
    };
    let Some(prefix) = inner.get(..9).filter(|p| p.eq_ignore_ascii_case("bookmarks")) else {
        return false;
    };
    match inner.get(prefix.len()..) {
        Some("") => true,
        Some(rest) => rest.strip_prefix('_').is_some_and(all_digits),
        None => false,
    }
}

/// `#<type>#<digits>%<fields>`: the type and the `%`-separated fields.
fn session_value(value: &str) -> Option<(&str, &str)> {
    let rest = value.strip_prefix('#')?;
    let (kind, rest) = rest.split_once('#')?;
    let (digits, fields) = rest.split_once('%')?;
    (all_digits(kind) && (digits.is_empty() || all_digits(digits))).then_some((kind, fields))
}

fn parse_port(value: Option<&str>, fallback: u16) -> u16 {
    port_of(js_trim(value.unwrap_or(""))).unwrap_or(fallback)
}

struct Section {
    sub_rep: String,
    entries: Vec<(String, String)>,
}

/// Reads the bookmark sections; every other section (e.g. `[Passwords]`) is ignored. `SubRep`
/// gives the folder (`\` becomes `/`) under "Imported"; `ImgNum` is ignored; type 109 is SSH
/// (port 22 by default), type 98 Telnet (port 23); other types and bad lines are counted.
pub fn parse_sessions_ini(text: &str) -> ParsedImport {
    let mut sections: Vec<Section> = Vec::new();
    let mut in_bookmarks = false;
    for raw in text.split('\n') {
        let raw = raw.strip_suffix('\r').unwrap_or(raw);
        let line = js_trim(raw);
        if line.is_empty() || line.starts_with(';') {
            continue;
        }
        if line.starts_with('[') {
            in_bookmarks = is_bookmarks_section(line);
            if in_bookmarks {
                sections.push(Section {
                    sub_rep: String::new(),
                    entries: Vec::new(),
                });
            }
            continue;
        }
        let Some(current) = sections.last_mut().filter(|_| in_bookmarks) else {
            continue;
        };
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = js_trim(key);
        if key == "SubRep" {
            current.sub_rep = value.to_string();
        } else if key != "ImgNum" {
            current.entries.push((key.to_string(), value.to_string()));
        }
    }

    let mut parsed = ParsedImport::default();
    for section in sections {
        let folder = normalize_folder(&format!("{IMPORT_ROOT}/{}", section.sub_rep.replace('\\', "/")));
        for (name, value) in section.entries {
            let Some((kind, fields)) = session_value(js_trim(&value)) else {
                parsed.skipped.add("unrecognized");
                continue;
            };
            let kind_digits = kind.trim_start_matches('0');
            let kind_number = kind_digits.parse::<u64>().unwrap_or(0);
            if kind_number != SSH_TYPE && kind_number != TELNET_TYPE {
                let label = type_label(kind_number)
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("type {}", if kind_digits.is_empty() { "0" } else { kind_digits }));
                parsed.skipped.add(&label);
                continue;
            }
            // Telnet lines start like SSH ones: host, then port.
            let fields: Vec<&str> = fields.split('%').collect();
            let host = js_trim(fields.first().copied().unwrap_or(""));
            if host.is_empty() {
                parsed.skipped.add("invalid");
                continue;
            }
            let session = if kind_number == TELNET_TYPE {
                ImportedSession::Telnet {
                    name,
                    folder: folder.clone(),
                    host: host.to_string(),
                    port: parse_port(fields.get(1).copied(), 23),
                }
            } else {
                ImportedSession::Ssh {
                    name,
                    folder: folder.clone(),
                    host: host.to_string(),
                    port: parse_port(fields.get(1).copied(), 22),
                    username: js_trim(fields.get(2).copied().unwrap_or("")).to_string(),
                    auth: None,
                    jumps: Vec::new(),
                }
            };
            parsed.sessions.push(session);
        }
    }
    parsed
}
