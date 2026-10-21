//! Terminal sessions for PlexusTerm Native (PRD §6.3).
//!
//! Every session kind (local PTY, SSH, Mosh, Telnet, serial, shared) feeds a bounded
//! [`ByteRing`]; a [`Session`] parses it on its own thread into an `alacritty_terminal` grid and
//! hands the UI damage-only [`Frame`]s. Input goes the other way through a bounded write queue.

mod frame;
mod guard;
mod ring;
mod session;

pub use alacritty_terminal::term::cell::{Cell, Flags};
pub use alacritty_terminal::vte::ansi::{Color, NamedColor};
pub use frame::{Frame, FrameLine};
pub use guard::{MAX_HYPERLINK, MAX_STRING, SequenceGuard};
pub use ring::{ByteRing, Closed, TryPushError};
pub use session::{
    EndReason, MAX_TITLE, Session, SessionConfig, SessionEvent, SessionHooks, SessionState, WRITE_CHUNK, WriteError,
};
