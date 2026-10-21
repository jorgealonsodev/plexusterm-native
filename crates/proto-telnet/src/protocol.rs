//! Telnet option negotiation (RFC 854/855), free of any I/O so it can be tested directly.
//! Ported from PlexusTerm v0.2.15 `src-tauri/src/telnet/protocol.rs`.
//!
//! Policy (PRD TEL-1):
//! - from the server we accept ECHO and SUPPRESS-GO-AHEAD (`WILL` → `DO`) and refuse
//!   every other option (`WILL` → `DONT`);
//! - we offer TERMINAL-TYPE and NAWS ourselves and agree to SUPPRESS-GO-AHEAD
//!   (`DO` → `WILL`); every other `DO` gets `WONT`;
//! - TERMINAL-TYPE answers `SEND` with `IS xterm-256color` (RFC 1091);
//! - NAWS sends the window size once the server agreed and again on every resize (RFC 1073).
//!
//! Replies are only sent when an option actually changes state, so two peers can never
//! loop on acknowledgements (the "Q method" of RFC 1143 in its simplest form).
//!
//! Line endings: the terminal sends Enter as a bare CR. RFC 854 requires a CR that is
//! not part of CR LF to be followed by NUL, so a bare CR is sent as CR NUL (what BSD
//! telnet does by default); CR LF is sent unchanged. On output, the NUL after a CR is dropped.

pub const IAC: u8 = 255;
pub const DONT: u8 = 254;
pub const DO: u8 = 253;
pub const WONT: u8 = 252;
pub const WILL: u8 = 251;
pub const SB: u8 = 250;
pub const SE: u8 = 240;

pub const ECHO: u8 = 1;
pub const SGA: u8 = 3;
pub const TTYPE: u8 = 24;
pub const NAWS: u8 = 31;

const TTYPE_IS: u8 = 0;
const TTYPE_SEND: u8 = 1;

/// Terminal type reported to the server.
pub const TERMINAL_TYPE: &str = "xterm-256color";

const CR: u8 = b'\r';
const LF: u8 = b'\n';
const NUL: u8 = 0;

/// Options the server may enable on its side.
fn server_may(option: u8) -> bool {
    matches!(option, ECHO | SGA)
}

/// Options we agree to enable on our side.
fn we_may(option: u8) -> bool {
    matches!(option, TTYPE | NAWS | SGA)
}

/// One flag per option code.
#[derive(Clone)]
struct Options([bool; 256]);

impl Options {
    fn get(&self, option: u8) -> bool {
        self.0.get(usize::from(option)).copied().unwrap_or(false)
    }

    fn set(&mut self, option: u8, value: bool) {
        if let Some(flag) = self.0.get_mut(usize::from(option)) {
            *flag = value;
        }
    }

    /// Clears the flag and returns what it was.
    fn take(&mut self, option: u8) -> bool {
        let was = self.get(option);
        self.set(option, false);
        was
    }
}

impl Default for Options {
    fn default() -> Self {
        Self([false; 256])
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Data,
    /// After a CR in the data stream (a following NUL is dropped).
    DataCr,
    Iac,
    /// After `IAC <verb>`, waiting for the option byte.
    Option(u8),
    /// Inside `IAC SB`, collecting the sub-negotiation.
    Sub,
    /// `IAC` inside a sub-negotiation.
    SubIac,
}

/// What a chunk of received bytes contained.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Received {
    /// Bytes for the terminal, with every Telnet command removed.
    pub data: Vec<u8>,
    /// Bytes to send back to the server (negotiation answers).
    pub reply: Vec<u8>,
}

/// Longest sub-negotiation kept; real ones (TTYPE, NAWS) are a few bytes.
pub const MAX_SUBNEGOTIATION: usize = 1024;

/// Negotiation state of one Telnet connection.
pub struct Telnet {
    state: State,
    /// Options enabled on our side.
    us: Options,
    /// Options enabled on the server's side.
    him: Options,
    /// `WILL` sent by us and not yet answered.
    offered: Options,
    sub: Vec<u8>,
    size: (u16, u16),
}

impl Telnet {
    pub fn new(cols: u16, rows: u16) -> Self {
        Self {
            state: State::Data,
            us: Options::default(),
            him: Options::default(),
            offered: Options::default(),
            sub: Vec::new(),
            size: (cols, rows),
        }
    }

    /// What to send right after connecting: the offers of TERMINAL-TYPE and NAWS.
    pub fn start(&mut self) -> Vec<u8> {
        let mut out = Vec::new();
        for option in [TTYPE, NAWS] {
            self.offered.set(option, true);
            out.extend_from_slice(&[IAC, WILL, option]);
        }
        out
    }

    /// Records the new window size and returns the NAWS update to send, if NAWS is on.
    pub fn resize(&mut self, cols: u16, rows: u16) -> Option<Vec<u8>> {
        self.size = (cols, rows);
        self.us.get(NAWS).then(|| self.naws())
    }

    /// `IAC SB NAWS <cols> <rows> IAC SE`, with IAC bytes of the size doubled.
    fn naws(&self) -> Vec<u8> {
        let (cols, rows) = self.size;
        let mut out = vec![IAC, SB, NAWS];
        for byte in cols.to_be_bytes().into_iter().chain(rows.to_be_bytes()) {
            out.push(byte);
            if byte == IAC {
                out.push(IAC);
            }
        }
        out.extend_from_slice(&[IAC, SE]);
        out
    }

    /// Processes bytes received from the server.
    pub fn receive(&mut self, bytes: &[u8]) -> Received {
        let mut received = Received::default();
        for &byte in bytes {
            self.state = match (self.state, byte) {
                (State::Data | State::DataCr, IAC) => State::Iac,
                (State::DataCr, NUL) => State::Data,
                (State::Data | State::DataCr, CR) => {
                    received.data.push(CR);
                    State::DataCr
                }
                (State::Data | State::DataCr, other) => {
                    received.data.push(other);
                    State::Data
                }
                (State::Iac, IAC) => {
                    received.data.push(IAC);
                    State::Data
                }
                (State::Iac, verb @ (DO | DONT | WILL | WONT)) => State::Option(verb),
                (State::Iac, SB) => {
                    self.sub.clear();
                    State::Sub
                }
                // NOP, GA, DM, AYT and the other commands carry nothing for the terminal.
                (State::Iac, _) => State::Data,
                (State::Option(verb), option) => {
                    self.negotiate(verb, option, &mut received.reply);
                    State::Data
                }
                (State::Sub, IAC) => State::SubIac,
                (State::Sub, other) => {
                    self.push_sub(other);
                    State::Sub
                }
                (State::SubIac, IAC) => {
                    self.push_sub(IAC);
                    State::Sub
                }
                (State::SubIac, SE) => {
                    self.subnegotiation(&mut received.reply);
                    State::Data
                }
                // Malformed: treat it as the end of the sub-negotiation.
                (State::SubIac, _) => State::Data,
            };
        }
        received
    }

    /// Bounded: a server that never sends SE cannot make the client grow without limit.
    fn push_sub(&mut self, byte: u8) {
        if self.sub.len() < MAX_SUBNEGOTIATION {
            self.sub.push(byte);
        }
    }

    fn negotiate(&mut self, verb: u8, option: u8, reply: &mut Vec<u8>) {
        match verb {
            DO => {
                let was_offered = self.offered.take(option);
                if !we_may(option) {
                    reply.extend_from_slice(&[IAC, WONT, option]);
                } else if !self.us.get(option) {
                    self.us.set(option, true);
                    if !was_offered {
                        reply.extend_from_slice(&[IAC, WILL, option]);
                    }
                    if option == NAWS {
                        reply.extend(self.naws());
                    }
                }
            }
            DONT => {
                self.offered.set(option, false);
                if self.us.take(option) {
                    reply.extend_from_slice(&[IAC, WONT, option]);
                }
            }
            WILL => {
                if !server_may(option) {
                    reply.extend_from_slice(&[IAC, DONT, option]);
                } else if !self.him.get(option) {
                    self.him.set(option, true);
                    reply.extend_from_slice(&[IAC, DO, option]);
                }
            }
            WONT if self.him.take(option) => {
                reply.extend_from_slice(&[IAC, DONT, option]);
            }
            _ => {}
        }
    }

    fn subnegotiation(&mut self, reply: &mut Vec<u8>) {
        if self.sub.as_slice() == [TTYPE, TTYPE_SEND] && self.us.get(TTYPE) {
            reply.extend_from_slice(&[IAC, SB, TTYPE, TTYPE_IS]);
            reply.extend_from_slice(TERMINAL_TYPE.as_bytes());
            reply.extend_from_slice(&[IAC, SE]);
        }
    }
}

/// Encodes user input for the wire: IAC doubled, a bare CR sent as CR NUL.
pub fn encode_input(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() + 4);
    for (i, &byte) in data.iter().enumerate() {
        out.push(byte);
        match byte {
            IAC => out.push(IAC),
            CR if data.get(i + 1) != Some(&LF) => out.push(NUL),
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn telnet() -> Telnet {
        Telnet::new(80, 24)
    }

    #[test]
    fn start_offers_terminal_type_and_naws() {
        assert_eq!(telnet().start(), vec![IAC, WILL, TTYPE, IAC, WILL, NAWS]);
    }

    #[test]
    fn an_unterminated_subnegotiation_is_bounded() {
        let mut t = telnet();
        let mut bytes = vec![IAC, SB, 99];
        bytes.extend(std::iter::repeat_n(b'x', 1 << 20));
        let r = t.receive(&bytes);
        assert!(r.data.is_empty());
        assert!(t.sub.len() <= MAX_SUBNEGOTIATION, "sub grew to {}", t.sub.len());
    }

    #[test]
    fn plain_data_passes_through() {
        let r = telnet().receive(b"hello\r\nworld");
        assert_eq!(r.data, b"hello\r\nworld");
        assert!(r.reply.is_empty());
    }

    #[test]
    fn escaped_iac_in_output_is_one_data_byte() {
        let r = telnet().receive(&[b'x', IAC, IAC, b'y']);
        assert_eq!(r.data, vec![b'x', IAC, b'y']);
    }

    #[test]
    fn commands_are_stripped_from_output() {
        // NOP (241) and GA (249) between data bytes.
        let r = telnet().receive(&[b'a', IAC, 241, b'b', IAC, 249, b'c']);
        assert_eq!(r.data, b"abc");
        assert!(r.reply.is_empty());
    }

    #[test]
    fn nul_after_cr_is_dropped_from_output() {
        let mut t = telnet();
        assert_eq!(t.receive(b"a\r").data, b"a\r");
        // Split across reads.
        assert_eq!(t.receive(&[0, b'b']).data, b"b");
        assert_eq!(t.receive(&[b'\r', 0, b'\r', b'\n']).data, b"\r\r\n");
    }

    #[test]
    fn server_echo_and_sga_are_accepted_once() {
        let mut t = telnet();
        let r = t.receive(&[IAC, WILL, ECHO, IAC, WILL, SGA]);
        assert_eq!(r.reply, vec![IAC, DO, ECHO, IAC, DO, SGA]);
        // A repeated WILL for an enabled option is not answered again.
        assert!(t.receive(&[IAC, WILL, ECHO]).reply.is_empty());
    }

    #[test]
    fn other_server_options_are_refused() {
        let r = telnet().receive(&[IAC, WILL, 42, IAC, WILL, 0]);
        assert_eq!(r.reply, vec![IAC, DONT, 42, IAC, DONT, 0]);
    }

    #[test]
    fn unsupported_do_is_refused() {
        let r = telnet().receive(&[IAC, DO, ECHO, IAC, DO, 39]);
        assert_eq!(r.reply, vec![IAC, WONT, ECHO, IAC, WONT, 39]);
    }

    #[test]
    fn do_naws_after_our_offer_sends_the_size_without_repeating_will() {
        let mut t = telnet();
        t.start();
        let r = t.receive(&[IAC, DO, NAWS]);
        assert_eq!(r.reply, vec![IAC, SB, NAWS, 0, 80, 0, 24, IAC, SE]);
    }

    #[test]
    fn unsolicited_do_naws_is_accepted_with_will_and_the_size() {
        let r = telnet().receive(&[IAC, DO, NAWS]);
        assert_eq!(r.reply, vec![IAC, WILL, NAWS, IAC, SB, NAWS, 0, 80, 0, 24, IAC, SE]);
    }

    #[test]
    fn resize_sends_naws_only_once_it_is_enabled() {
        let mut t = telnet();
        t.start();
        assert_eq!(t.resize(100, 40), None);
        t.receive(&[IAC, DO, NAWS]);
        assert_eq!(
            t.resize(300, 255),
            Some(vec![IAC, SB, NAWS, 1, 44, 0, 255, 255, IAC, SE])
        );
        t.receive(&[IAC, DONT, NAWS]);
        assert_eq!(t.resize(80, 24), None);
    }

    #[test]
    fn dont_for_an_enabled_option_is_acknowledged_once() {
        let mut t = telnet();
        t.start();
        t.receive(&[IAC, DO, TTYPE]);
        assert_eq!(t.receive(&[IAC, DONT, TTYPE]).reply, vec![IAC, WONT, TTYPE]);
        assert!(t.receive(&[IAC, DONT, TTYPE]).reply.is_empty());
    }

    #[test]
    fn terminal_type_send_is_answered_with_xterm_256color() {
        let mut t = telnet();
        t.start();
        let r = t.receive(&[IAC, DO, TTYPE, IAC, SB, TTYPE, 1, IAC, SE]);
        let mut expected = vec![IAC, SB, TTYPE, 0];
        expected.extend_from_slice(b"xterm-256color");
        expected.extend_from_slice(&[IAC, SE]);
        assert_eq!(r.reply, expected);
    }

    #[test]
    fn terminal_type_send_is_ignored_while_ttype_is_off() {
        let r = telnet().receive(&[IAC, SB, TTYPE, 1, IAC, SE]);
        assert!(r.reply.is_empty());
    }

    #[test]
    fn negotiation_split_across_reads_is_handled() {
        let mut t = telnet();
        assert!(t.receive(&[IAC]).reply.is_empty());
        assert!(t.receive(&[WILL]).reply.is_empty());
        assert_eq!(
            t.receive(&[ECHO, b'z']),
            Received {
                data: b"z".to_vec(),
                reply: vec![IAC, DO, ECHO],
            }
        );
    }

    #[test]
    fn subnegotiation_data_is_not_terminal_output() {
        let r = telnet().receive(&[b'a', IAC, SB, 99, b'x', IAC, IAC, IAC, SE, b'b']);
        assert_eq!(r.data, b"ab");
    }

    #[test]
    fn input_iac_is_doubled() {
        assert_eq!(encode_input(&[b'a', IAC, b'b']), vec![b'a', IAC, IAC, b'b']);
    }

    #[test]
    fn input_bare_cr_becomes_cr_nul_and_cr_lf_is_kept() {
        assert_eq!(encode_input(b"ls\r"), b"ls\r\0");
        assert_eq!(encode_input(b"a\r\nb"), b"a\r\nb");
        assert_eq!(encode_input(b"\r\r"), b"\r\0\r\0");
    }
}
