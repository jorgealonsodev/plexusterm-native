//! Text of local files: attached to AI questions, shown and saved by the file viewer in the
//! encoding it was opened with (PRD VIEW-2, VIEW-5), and images for the Markdown preview.

use std::fs;
use std::path::Path;

use crate::{Entry, EntryKind, failure, list, save};

/// Bytes read from one file attached to an AI question; the rest is left out.
pub const MAX_ATTACH_BYTES: usize = 64 * 1024;
/// Entries of a folder attached to an AI question.
const MAX_ATTACH_ENTRIES: usize = 500;
/// Bytes of a file shown in the file viewer; the rest is left out.
pub const MAX_VIEW_BYTES: usize = 4 * 1024 * 1024;

/// A file's text (or a folder's listing) attached to an AI question or shown in the viewer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextFile {
    pub path: String,
    pub content: String,
    /// The file is longer than what was read.
    pub truncated: bool,
    /// Id of the encoding `content` was decoded with (`utf-8`, `utf-16le`, `windows-1252`...).
    pub encoding: String,
}

/// Replaces the text of the existing file `path` with `content`, but only while it still holds
/// `expected` (the text that was opened), so changes made by someone else are never lost.
/// The new text is written to a new file renamed over the old one, so a crash never leaves half
/// a file; a symlink and the file's mode are kept (see the `save` module for its owner and links).
/// `encoding` is the id to write with (`None`: UTF-8); `expected_encoding` the one `expected` was
/// decoded with (`None`: the same as `encoding`).
pub fn write_text(
    path: &Path,
    expected: &str,
    content: &str,
    encoding: Option<&str>,
    expected_encoding: Option<&str>,
) -> Result<(), String> {
    let label = path.to_string_lossy();
    // Encoded first: characters the encoding cannot hold leave the file untouched.
    let bytes = encode_text(&label, content, encoding)?;
    let (current, truncated) =
        read_prefix(path, MAX_VIEW_BYTES).map_err(|err| err.replacen("cannot read", "cannot save", 1))?;
    check_unchanged(&label, current, truncated, expected, expected_encoding.or(encoding))?;
    save::replace_contents(path, &bytes).map_err(|err| failure("save", path, err))
}

/// The text of the first bytes of a file; refused when they hold a NUL byte or invalid UTF-8
/// (a character cut by the size limit is dropped, not refused).
pub fn text_from(path: &str, bytes: Vec<u8>, truncated: bool) -> Result<TextFile, String> {
    text_for("attach", path, bytes, truncated)
}

/// [`text_from`] for another action (`view`): its errors say `cannot <verb> <path>: ...`.
pub fn text_for(verb: &str, path: &str, bytes: Vec<u8>, truncated: bool) -> Result<TextFile, String> {
    let not_text = || format!("cannot {verb} {path}: it is not a text file");
    if bytes.contains(&0) {
        return Err(not_text());
    }
    let content = utf8(bytes, truncated).ok_or_else(not_text)?;
    Ok(TextFile {
        path: path.to_string(),
        content,
        truncated,
        encoding: "utf-8".to_string(),
    })
}

const UTF8_BOM: &[u8] = b"\xEF\xBB\xBF";
const UTF16LE_BOM: &[u8] = b"\xFF\xFE";
const UTF16BE_BOM: &[u8] = b"\xFE\xFF";
/// Ids of the non-Unicode encodings, as `encoding_rs` labels.
const LEGACY_ENCODINGS: [&str; 11] = [
    "windows-1252",
    "iso-8859-15",
    "iso-8859-2",
    "windows-1250",
    "windows-1251",
    "koi8-r",
    "shift_jis",
    "euc-jp",
    "gbk",
    "big5",
    "euc-kr",
];

/// How the text of one encoding id is read and written.
#[derive(Clone, Copy)]
enum Codec {
    Utf8,
    Utf8Bom,
    Utf16 { little_endian: bool },
    Legacy(&'static encoding_rs::Encoding),
}

fn codec_of(verb: &str, path: &str, id: &str) -> Result<Codec, String> {
    let codec = match id {
        "utf-8" => Some(Codec::Utf8),
        "utf-8-bom" => Some(Codec::Utf8Bom),
        "utf-16le" => Some(Codec::Utf16 { little_endian: true }),
        "utf-16be" => Some(Codec::Utf16 { little_endian: false }),
        _ if LEGACY_ENCODINGS.contains(&id) => encoding_rs::Encoding::for_label(id.as_bytes()).map(Codec::Legacy),
        _ => None,
    };
    codec.ok_or_else(|| format!("cannot {verb} {path}: unknown encoding {id}"))
}

/// Strict UTF-8; when `truncated`, a character cut at the end is dropped.
fn utf8(bytes: Vec<u8>, truncated: bool) -> Option<String> {
    match String::from_utf8(bytes) {
        Ok(text) => Some(text),
        Err(err) if truncated && err.utf8_error().error_len().is_none() => {
            let valid = err.utf8_error().valid_up_to();
            let mut bytes = err.into_bytes();
            bytes.truncate(valid);
            String::from_utf8(bytes).ok()
        }
        Err(_) => None,
    }
}

/// UTF-16 without its BOM; when `truncated`, an odd final byte or a cut surrogate pair is dropped.
fn utf16(bytes: &[u8], truncated: bool, little_endian: bool) -> Option<String> {
    if bytes.len() % 2 == 1 && !truncated {
        return None;
    }
    // An odd final byte (only possible when truncated) is left out.
    let (pairs, _) = bytes.as_chunks::<2>();
    let mut units: Vec<u16> = pairs
        .iter()
        .map(|&pair| {
            if little_endian {
                u16::from_le_bytes(pair)
            } else {
                u16::from_be_bytes(pair)
            }
        })
        .collect();
    if truncated && units.last().is_some_and(|unit| (0xD800..0xDC00).contains(unit)) {
        units.pop();
    }
    char::decode_utf16(units).collect::<Result<String, _>>().ok()
}

/// A non-Unicode encoding; when `truncated`, a multibyte character cut at the end is dropped.
fn legacy(encoding: &'static encoding_rs::Encoding, bytes: &[u8], truncated: bool) -> Option<String> {
    let mut decoder = encoding.new_decoder_without_bom_handling();
    let mut text = String::with_capacity(decoder.max_utf8_buffer_length_without_replacement(bytes.len())?);
    // Not the last chunk when truncated: an incomplete final sequence stays buffered, unreported.
    let (result, _) = decoder.decode_to_string_without_replacement(bytes, &mut text, !truncated);
    matches!(result, encoding_rs::DecoderResult::InputEmpty).then_some(text)
}

/// The text of the first bytes of a file decoded with the encoding id `encoding`; `None` detects a
/// UTF-8 or UTF-16 BOM (left out of the text) and otherwise reads UTF-8 like [`text_for`].
/// A character cut by the size limit is dropped, not refused.
pub fn decode_text(
    verb: &str,
    path: &str,
    bytes: Vec<u8>,
    truncated: bool,
    encoding: Option<&str>,
) -> Result<TextFile, String> {
    let not_text = || format!("cannot {verb} {path}: it is not a text file");
    let Some(id) = encoding else {
        let detected = if bytes.starts_with(UTF8_BOM) {
            "utf-8-bom"
        } else if bytes.starts_with(UTF16LE_BOM) {
            "utf-16le"
        } else if bytes.starts_with(UTF16BE_BOM) {
            "utf-16be"
        } else {
            return text_for(verb, path, bytes, truncated);
        };
        return decode_text(verb, path, bytes, truncated, Some(detected)).map_err(|_| not_text());
    };
    let codec = codec_of(verb, path, id)?;
    let malformed = || format!("cannot {verb} {path}: it is not a text file in {id}");
    if !matches!(codec, Codec::Utf16 { .. }) && bytes.contains(&0) {
        return Err(not_text());
    }
    let content = match codec {
        Codec::Utf8 => utf8(bytes, truncated).ok_or_else(not_text)?,
        Codec::Utf8Bom => {
            let body = bytes.strip_prefix(UTF8_BOM).unwrap_or(&bytes).to_vec();
            utf8(body, truncated).ok_or_else(not_text)?
        }
        Codec::Utf16 { little_endian } => {
            let bom = if little_endian { UTF16LE_BOM } else { UTF16BE_BOM };
            let body = bytes.strip_prefix(bom).unwrap_or(&bytes);
            utf16(body, truncated, little_endian).ok_or_else(malformed)?
        }
        Codec::Legacy(encoding) => legacy(encoding, &bytes, truncated).ok_or_else(malformed)?,
    };
    Ok(TextFile {
        path: path.to_string(),
        content,
        truncated,
        encoding: id.to_string(),
    })
}

/// `content` as the bytes of the encoding id `encoding` (`None`: UTF-8), with the BOM of UTF-8 with
/// BOM and UTF-16. Refused when the encoding cannot hold some of its characters.
pub fn encode_text(path: &str, content: &str, encoding: Option<&str>) -> Result<Vec<u8>, String> {
    let id = encoding.unwrap_or("utf-8");
    Ok(match codec_of("save", path, id)? {
        Codec::Utf8 => content.as_bytes().to_vec(),
        Codec::Utf8Bom => [UTF8_BOM, content.as_bytes()].concat(),
        // encoding_rs only decodes UTF-16.
        Codec::Utf16 { little_endian } => {
            let bom = if little_endian { UTF16LE_BOM } else { UTF16BE_BOM };
            let units = content.encode_utf16().flat_map(|unit| {
                if little_endian {
                    unit.to_le_bytes()
                } else {
                    unit.to_be_bytes()
                }
            });
            bom.iter().copied().chain(units).collect()
        }
        Codec::Legacy(encoding) => {
            let (bytes, _, unmappable) = encoding.encode(content);
            if unmappable {
                return Err(format!("cannot save {path}: some characters cannot be written in {id}"));
            }
            bytes.into_owned()
        }
    })
}

/// Whether `current` (the first bytes of a file, read up to [`MAX_VIEW_BYTES`]) still holds the
/// text `expected` once decoded with `encoding`; refused as changed on disk otherwise.
pub fn check_unchanged(
    path: &str,
    current: Vec<u8>,
    truncated: bool,
    expected: &str,
    encoding: Option<&str>,
) -> Result<(), String> {
    let id = encoding.unwrap_or("utf-8");
    codec_of("save", path, id)?;
    let unchanged =
        !truncated && decode_text("save", path, current, false, Some(id)).is_ok_and(|text| text.content == expected);
    if unchanged {
        Ok(())
    } else {
        Err(format!("cannot save {path}: it changed on disk since it was opened"))
    }
}

/// A folder's entries as text, one per line, folders ending in `/`.
pub fn listing_text(path: &str, entries: &[Entry]) -> TextFile {
    let lines: Vec<String> = entries
        .iter()
        .take(MAX_ATTACH_ENTRIES)
        .map(|entry| match entry.kind {
            EntryKind::Dir => format!("{}/", entry.name),
            _ => entry.name.clone(),
        })
        .collect();
    TextFile {
        path: path.to_string(),
        content: lines.join("\n"),
        truncated: entries.len() > MAX_ATTACH_ENTRIES,
        encoding: "utf-8".to_string(),
    }
}

/// The first [`MAX_ATTACH_BYTES`] of a text file, or a folder's listing.
pub fn read_text(path: &Path) -> Result<TextFile, String> {
    let label = path.to_string_lossy().into_owned();
    if fs::metadata(path).map(|m| m.is_dir()).unwrap_or(false) {
        return Ok(listing_text(&label, &list(path)?));
    }
    let (bytes, truncated) = read_prefix(path, MAX_ATTACH_BYTES)?;
    text_from(&label, bytes, truncated)
}

/// The first [`MAX_VIEW_BYTES`] of a text file, for the file viewer, decoded with the encoding id
/// `encoding` (`None`: detected, see [`decode_text`]).
pub fn view_text(path: &Path, encoding: Option<&str>) -> Result<TextFile, String> {
    let label = path.to_string_lossy().into_owned();
    if fs::metadata(path).map(|m| m.is_dir()).unwrap_or(false) {
        return Err(format!("cannot view {label}: it is a folder"));
    }
    let (bytes, truncated) = read_prefix(path, MAX_VIEW_BYTES)?;
    decode_text("view", &label, bytes, truncated, encoding)
}

/// Largest image the Markdown preview shows.
pub const MAX_IMAGE_BYTES: usize = 20 * 1024 * 1024;
const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "svg", "bmp", "ico", "avif"];

/// Whether a path names an image, by its extension: a Markdown file may point anywhere, so the
/// preview reads nothing else (not a key or a document named in an image link).
pub fn is_image_path(path: &str) -> bool {
    Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| IMAGE_EXTENSIONS.iter().any(|known| ext.eq_ignore_ascii_case(known)))
}

pub fn not_an_image(path: &str) -> String {
    format!("cannot show {path}: it is not an image")
}

pub fn image_too_large(path: &str) -> String {
    format!(
        "cannot show {path}: it is larger than {} MiB",
        MAX_IMAGE_BYTES / (1024 * 1024)
    )
}

/// The bytes of an image file a Markdown file shows, up to [`MAX_IMAGE_BYTES`].
pub fn read_image(path: &Path) -> Result<Vec<u8>, String> {
    let label = path.to_string_lossy().into_owned();
    // A network share or a device is never opened: Windows would sign in to the share's host with
    // the user's credentials. Only a regular file is read (a FIFO would never end).
    let remote = label.starts_with(r"\\") || label.starts_with("//");
    if remote || !is_image_path(&label) || !fs::metadata(path).is_ok_and(|m| m.is_file()) {
        return Err(not_an_image(&label));
    }
    let (bytes, truncated) = read_prefix(path, MAX_IMAGE_BYTES)?;
    if truncated {
        return Err(image_too_large(&label));
    }
    Ok(bytes)
}

/// The first `max` bytes of a file, and whether it is longer.
fn read_prefix(path: &Path, max: usize) -> Result<(Vec<u8>, bool), String> {
    use std::io::Read;
    let file = fs::File::open(path).map_err(|err| failure("read", path, err))?;
    let mut bytes = Vec::new();
    file.take(max as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|err| failure("read", path, err))?;
    let truncated = bytes.len() > max;
    bytes.truncate(max);
    Ok((bytes, truncated))
}
