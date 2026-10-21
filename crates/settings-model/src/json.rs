//! A JSON tree that keeps the order of object keys, as JavaScript objects do.
//!
//! settings.json depends on it twice: of two clashing shortcut overrides the one written later
//! is dropped (KEY-5), and unknown keys are written back as they were (PRD §4.1 deviation 2).
//! `serde_json::Value` sorts keys unless the `preserve_order` feature is on, and turning that
//! feature on would change the output of every crate in the workspace that uses `Value`.

use std::fmt;

use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use serde::ser::{Serialize, SerializeMap, SerializeSeq, Serializer};
use serde_json::Number;

#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Number(Number),
    String(String),
    Array(Vec<Json>),
    /// Keys in the order they were written; a repeated key keeps its first place and its last
    /// value, like `JSON.parse`.
    Object(Vec<(String, Json)>),
}

impl Json {
    /// Parses like `JSON.parse`, which reads a number too large for a double (`1e999`) as
    /// Infinity where serde_json refuses the whole text. Such a number is read as `null`: v0.2.15
    /// takes the default for a value that is not finite and `JSON.stringify` writes it as `null`,
    /// so the rest of the file is kept exactly as v0.2.15 kept it.
    pub fn parse(text: &str) -> Option<Json> {
        serde_json::from_str(text)
            .ok()
            .or_else(|| serde_json::from_str(&finite_numbers(text)?).ok())
    }

    pub fn to_pretty_string(&self) -> String {
        // Serializing this tree cannot fail: every key is a string and every number is finite.
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Json::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// The value of a number, like JavaScript's `typeof value === "number"`.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Json::Number(n) => n.as_f64(),
            _ => None,
        }
    }

    /// A number written the way JavaScript writes it: whole values without a fraction.
    pub fn number(value: f64) -> Json {
        #[allow(
            clippy::cast_possible_truncation,
            reason = "only whole values well inside i64 are cast"
        )]
        let whole = (value.fract() == 0.0 && value.abs() < 9.0e15).then_some(value as i64);
        match whole {
            Some(int) => Json::Number(Number::from(int)),
            None => Number::from_f64(value).map_or(Json::Null, Json::Number),
        }
    }
}

impl Serialize for Json {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Json::Null => serializer.serialize_unit(),
            Json::Bool(b) => serializer.serialize_bool(*b),
            Json::Number(n) => n.serialize(serializer),
            Json::String(s) => serializer.serialize_str(s),
            Json::Array(items) => {
                let mut seq = serializer.serialize_seq(Some(items.len()))?;
                for item in items {
                    seq.serialize_element(item)?;
                }
                seq.end()
            }
            Json::Object(entries) => {
                let mut map = serializer.serialize_map(Some(entries.len()))?;
                for (key, value) in entries {
                    map.serialize_entry(key, value)?;
                }
                map.end()
            }
        }
    }
}

struct JsonVisitor;

impl<'de> Visitor<'de> for JsonVisitor {
    type Value = Json;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("any JSON value")
    }

    fn visit_bool<E: de::Error>(self, v: bool) -> Result<Json, E> {
        Ok(Json::Bool(v))
    }

    fn visit_i64<E: de::Error>(self, v: i64) -> Result<Json, E> {
        Ok(Json::Number(v.into()))
    }

    fn visit_u64<E: de::Error>(self, v: u64) -> Result<Json, E> {
        Ok(Json::Number(v.into()))
    }

    fn visit_f64<E: de::Error>(self, v: f64) -> Result<Json, E> {
        Ok(Number::from_f64(v).map_or(Json::Null, Json::Number))
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<Json, E> {
        Ok(Json::String(v.to_owned()))
    }

    fn visit_string<E: de::Error>(self, v: String) -> Result<Json, E> {
        Ok(Json::String(v))
    }

    fn visit_unit<E: de::Error>(self) -> Result<Json, E> {
        Ok(Json::Null)
    }

    fn visit_none<E: de::Error>(self) -> Result<Json, E> {
        Ok(Json::Null)
    }

    fn visit_some<D: Deserializer<'de>>(self, deserializer: D) -> Result<Json, D::Error> {
        Json::deserialize(deserializer)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Json, A::Error> {
        let mut items = Vec::new();
        while let Some(item) = seq.next_element()? {
            items.push(item);
        }
        Ok(Json::Array(items))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Json, A::Error> {
        let mut entries: Vec<(String, Json)> = Vec::new();
        while let Some((key, value)) = map.next_entry::<String, Json>()? {
            match entries.iter_mut().find(|(k, _)| *k == key) {
                Some(entry) => entry.1 = value,
                None => entries.push((key, value)),
            }
        }
        Ok(Json::Object(entries))
    }
}

impl<'de> Deserialize<'de> for Json {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Json, D::Error> {
        deserializer.deserialize_any(JsonVisitor)
    }
}

/// `text` with every number literal outside strings that overflows a double replaced by `null`;
/// `None` when there is none.
fn finite_numbers(text: &str) -> Option<String> {
    let mut out = String::with_capacity(text.len());
    let mut replaced = false;
    let mut chars = text.char_indices().peekable();
    let mut in_string = false;
    while let Some((start, c)) = chars.next() {
        if in_string {
            out.push(c);
            match c {
                '\\' => out.extend(chars.next().map(|(_, escaped)| escaped)),
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }
        if c == '-' || c.is_ascii_digit() {
            let mut end = start + c.len_utf8();
            while let Some(&(i, next)) = chars.peek() {
                if !(next.is_ascii_digit() || matches!(next, '.' | 'e' | 'E' | '+' | '-')) {
                    break;
                }
                end = i + next.len_utf8();
                chars.next();
            }
            let literal = text.get(start..end).unwrap_or_default();
            match literal.parse::<f64>() {
                Ok(value) if value.is_infinite() => {
                    replaced = true;
                    out.push_str("null");
                }
                _ => out.push_str(literal),
            }
            continue;
        }
        in_string = c == '"';
        out.push(c);
    }
    replaced.then_some(out)
}
