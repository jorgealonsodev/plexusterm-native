//! Bounds escape strings before they reach the parser (PRD §6.3 ST-9).
//!
//! `vte` buffers an OSC payload in a growing `Vec` until its terminator, so a remote end could
//! send `ESC ]` and then gigabytes. The guard holds each OSC, DCS, SOS, PM or APC string itself,
//! up to its limit, and forwards it whole once terminated. A string that grows past the limit is
//! discarded with its terminator: the parser never sees it, and the guard never holds more than
//! the limit.

/// Largest OSC/DCS/SOS/PM/APC payload forwarded (ST-9).
pub const MAX_STRING: usize = 64 * 1024;
/// Largest OSC 8 payload (hyperlink parameters and URI) forwarded (ST-9).
pub const MAX_HYPERLINK: usize = 8 * 1024;

const ESC: u8 = 0x1b;
const BEL: u8 = 0x07;
const CAN: u8 = 0x18;
const SUB: u8 = 0x1a;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    Ground,
    /// ESC seen and held back until the next byte tells whether a string starts.
    Escape,
    /// Inside a string; OSC strings also end with BEL.
    Payload {
        osc: bool,
    },
    /// ESC seen inside a string: `\` ends it, anything else ends it and starts a new escape.
    PayloadEscape,
}

/// Streaming filter; feed it every chunk of output in order.
#[derive(Clone, Debug)]
pub struct SequenceGuard {
    state: State,
    /// The string being held: introducer and payload, never more than `limit` payload bytes.
    held: Vec<u8>,
    payload: usize,
    limit: usize,
    /// Digits of the OSC number while it is being read (`None` once known or not an OSC).
    osc_number: Option<u32>,
    discarding: bool,
}

impl Default for SequenceGuard {
    fn default() -> Self {
        Self {
            state: State::Ground,
            held: Vec::new(),
            payload: 0,
            limit: MAX_STRING,
            osc_number: None,
            discarding: false,
        }
    }
}

impl SequenceGuard {
    /// Appends the bytes of `input` the parser may see to `out`.
    pub fn filter(&mut self, input: &[u8], out: &mut Vec<u8>) {
        out.reserve(input.len());
        for &byte in input {
            self.step(byte, out);
        }
    }

    fn step(&mut self, byte: u8, out: &mut Vec<u8>) {
        match self.state {
            State::Ground => {
                if byte == ESC {
                    self.state = State::Escape;
                } else {
                    out.push(byte);
                }
            }
            State::Escape => self.after_escape(byte, out),
            State::Payload { osc } => match byte {
                BEL if osc => self.finish(&[byte], out),
                ESC => self.state = State::PayloadEscape,
                // CAN and SUB end the string in the parser too.
                CAN | SUB => self.finish(&[byte], out),
                _ => self.hold(byte),
            },
            State::PayloadEscape => {
                if byte == b'\\' {
                    self.finish(&[ESC, byte], out);
                } else {
                    // A new escape ends the string, as in the parser, and is handled normally.
                    self.finish(&[], out);
                    self.after_escape(byte, out);
                }
            }
        }
    }

    fn after_escape(&mut self, byte: u8, out: &mut Vec<u8>) {
        match byte {
            b']' => self.start(byte, true),
            b'P' | b'X' | b'^' | b'_' => self.start(byte, false),
            // ESC ESC: the first one is complete on its own; keep holding the second.
            ESC => {
                out.push(ESC);
                self.state = State::Escape;
            }
            _ => {
                out.extend_from_slice(&[ESC, byte]);
                self.state = State::Ground;
            }
        }
    }

    fn start(&mut self, introducer: u8, osc: bool) {
        self.state = State::Payload { osc };
        self.held.clear();
        self.held.extend_from_slice(&[ESC, introducer]);
        self.payload = 0;
        self.limit = MAX_STRING;
        self.osc_number = osc.then_some(0);
        self.discarding = false;
    }

    fn hold(&mut self, byte: u8) {
        if self.discarding {
            return;
        }
        if let Some(number) = self.osc_number {
            match byte {
                b'0'..=b'9' if number < 10_000 => self.osc_number = Some(number * 10 + u32::from(byte - b'0')),
                _ => {
                    if number == 8 {
                        self.limit = MAX_HYPERLINK;
                    }
                    self.osc_number = None;
                }
            }
        }
        self.payload += 1;
        if self.payload > self.limit {
            self.discarding = true;
            self.held = Vec::new();
            return;
        }
        self.held.push(byte);
    }

    /// Ends the string: forwards it with `terminator` unless it was discarded.
    fn finish(&mut self, terminator: &[u8], out: &mut Vec<u8>) {
        if !self.discarding {
            out.append(&mut self.held);
            out.extend_from_slice(terminator);
        }
        self.held.clear();
        self.discarding = false;
        self.state = State::Ground;
    }
}
