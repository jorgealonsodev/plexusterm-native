//! OpenSSH client config (`~/.ssh/config`) import (v0.2.15 `sshConfigImport.ts`).

use super::{ImportedSession, ParsedImport};
use crate::model::{MAX_JUMPS, SavedAuth};
use crate::text::{is_js_space, js_trim, port_of};

/// Every host of an OpenSSH config file goes in this folder, under [`super::IMPORT_ROOT`].
pub const SSH_CONFIG_FOLDER: &str = "Imported/SSH config";

/// Options read from the file; every other option is ignored.
const OPTIONS: [&str; 6] = ["hostname", "user", "port", "identityfile", "proxyjump", "proxycommand"];

#[derive(Default)]
struct Options([Option<String>; 6]);

impl Options {
    fn get(&self, option: &str) -> Option<&str> {
        let index = OPTIONS.iter().position(|o| *o == option)?;
        self.0.get(index)?.as_deref()
    }

    /// Sets `option` unless it already has a value: the first one wins, as in ssh.
    fn set_first(&mut self, index: usize, value: Option<&str>) {
        if let (Some(slot @ None), Some(value)) = (self.0.get_mut(index), value) {
            *slot = Some(value.to_string());
        }
    }
}

struct Block {
    /// `None` for a Match block, which this import never applies.
    patterns: Option<Vec<String>>,
    options: Options,
}

/// The file decoded like v0.2.15 (`new TextDecoder().decode`): UTF-8 with invalid bytes
/// replaced and a leading BOM dropped.
pub fn decode_ssh_config(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    text.strip_prefix('\u{feff}').unwrap_or(&text).to_string()
}

/// `Key value`, `Key=value` or `Key = value`, with the key lower-cased and the value trimmed.
fn split_line(line: &str) -> Option<(String, &str)> {
    let mut chars = line.char_indices();
    chars.next()?;
    let (at, _) = chars.find(|(_, c)| *c == '=' || is_js_space(*c))?;
    let key = line.get(..at)?;
    let rest = line.get(at..)?.trim_start_matches(is_js_space);
    let value = rest.strip_prefix('=').unwrap_or(rest);
    Some((key.to_lowercase(), js_trim(value)))
}

/// The first argument of a value, without its quotes.
fn first_argument(value: &str) -> &str {
    if let Some(rest) = value.strip_prefix('"') {
        return rest.find('"').and_then(|end| rest.get(..end)).unwrap_or(rest);
    }
    value.split(is_js_space).next().unwrap_or("")
}

/// Case-insensitive glob with `*` and `?`.
fn glob_matches(pattern: &[char], text: &[char]) -> bool {
    match pattern.split_first() {
        None => text.is_empty(),
        Some(('*', rest)) => (0..=text.len()).any(|skip| text.get(skip..).is_some_and(|t| glob_matches(rest, t))),
        Some((p, rest)) => match text.split_first() {
            Some((t, text_rest)) => {
                (*p == '?' || p.to_lowercase().eq(t.to_lowercase())) && glob_matches(rest, text_rest)
            }
            None => false,
        },
    }
}

fn glob(pattern: &str, alias: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().collect();
    let alias: Vec<char> = alias.chars().collect();
    glob_matches(&pattern, &alias)
}

/// Like ssh: a negated pattern that matches excludes the block, otherwise any positive match applies it.
fn block_matches(patterns: &[String], alias: &str) -> bool {
    let mut matched = false;
    for pattern in patterns {
        if let Some(negated) = pattern.strip_prefix('!') {
            if glob(negated, alias) {
                return false;
            }
        } else if glob(pattern, alias) {
            matched = true;
        }
    }
    matched
}

/// Reads the hosts of an OpenSSH client config. Each concrete `Host` alias becomes a session;
/// wildcard blocks and options before the first `Host` give it defaults, the first value of
/// each option winning as in ssh. `Include` and `Match` are not followed, and `ProxyCommand` is
/// not imported; they are counted in `skipped`. `ProxyJump` hops become jump hosts when they
/// are at most [`MAX_JUMPS`] aliases of the file other than the host itself.
pub fn parse_ssh_config(text: &str) -> ParsedImport {
    let mut blocks = vec![Block {
        patterns: Some(vec!["*".into()]),
        options: Options::default(),
    }];
    let mut aliases: Vec<String> = Vec::new();
    let mut parsed = ParsedImport::default();

    for raw in text.split('\n') {
        let raw = raw.strip_suffix('\r').unwrap_or(raw);
        let line = js_trim(raw);
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = split_line(line) else {
            continue;
        };
        match key.as_str() {
            "host" => {
                let patterns: Vec<String> = value
                    .split(is_js_space)
                    .map(|p| {
                        let p = p.strip_prefix('"').unwrap_or(p);
                        p.strip_suffix('"').unwrap_or(p).to_string()
                    })
                    .filter(|p| !p.is_empty())
                    .collect();
                for pattern in &patterns {
                    let concrete = !pattern.contains(['*', '?']) && !pattern.starts_with('!');
                    if concrete && !aliases.contains(pattern) {
                        aliases.push(pattern.clone());
                    }
                }
                blocks.push(Block {
                    patterns: Some(patterns),
                    options: Options::default(),
                });
            }
            "match" => {
                parsed.skipped.add("match");
                blocks.push(Block {
                    patterns: None,
                    options: Options::default(),
                });
            }
            "include" => parsed.skipped.add("include"),
            option => {
                if let (Some(index), Some(block)) = (OPTIONS.iter().position(|o| *o == option), blocks.last_mut()) {
                    block.options.set_first(index, Some(first_argument(value)));
                }
            }
        }
    }

    let resolve = |alias: &str| {
        let mut resolved = Options::default();
        for block in &blocks {
            if block.patterns.as_deref().is_some_and(|p| block_matches(p, alias)) {
                for (index, option) in OPTIONS.iter().enumerate() {
                    resolved.set_first(index, block.options.get(option));
                }
            }
        }
        resolved
    };

    for alias in &aliases {
        let options = resolve(alias);
        let host = options.get("hostname").unwrap_or(alias).replace("%h", alias);
        let port = options.get("port").and_then(port_of).unwrap_or(22);
        let username = options.get("user").unwrap_or("").to_string();
        let auth = options
            .get("identityfile")
            .filter(|f| !f.is_empty())
            .map(|f| SavedAuth::Key {
                path: f.replace("%d", "~"),
            });
        if options
            .get("proxycommand")
            .is_some_and(|c| !c.is_empty() && !c.eq_ignore_ascii_case("none"))
        {
            parsed.skipped.add("proxyCommand");
        }
        let mut jumps = Vec::new();
        if let Some(jump) = options
            .get("proxyjump")
            .filter(|j| !j.is_empty() && !j.eq_ignore_ascii_case("none"))
        {
            let hops: Vec<String> = jump
                .split(',')
                .map(js_trim)
                .filter(|h| !h.is_empty())
                .map(str::to_string)
                .collect();
            if hops.len() <= MAX_JUMPS && hops.iter().all(|hop| aliases.contains(hop) && hop != alias) {
                jumps = hops;
            } else {
                parsed.skipped.add("jump");
            }
        }
        parsed.sessions.push(ImportedSession::Ssh {
            name: alias.clone(),
            folder: SSH_CONFIG_FOLDER.into(),
            host,
            port,
            username,
            auth,
            jumps,
        });
    }
    parsed
}
