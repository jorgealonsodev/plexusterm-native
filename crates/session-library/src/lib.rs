//! Saved sessions and tunnels, ported from PlexusTerm v0.2.15 `src/features/sessions` and
//! `src/features/tunnels` (PRD SES-1, SES-2, SSH-3, SSH-6a, TUN-1a, DATA-1a).
//!
//! Pure logic: the library model and its actions, search, jump chains, edit forms with the
//! exact v0.2.15 messages, the saved-password rules, `.ini` and `~/.ssh/config` imports, and
//! `sessions.json` / `tunnels.json` read and written byte for byte like v0.2.15.

pub mod file;
pub mod form;
pub mod jump_hosts;
pub mod library;
pub mod model;
pub mod panel;
pub mod password_flow;
pub mod shell;
pub mod target;
mod text;

pub use file::{LibraryError, load_library, parse_library, save_library, serialize_library};
pub use library::{ChainError, LibraryAction, SessionLibrary, TreeNode, build_tree, normalize_folder, resolve_chain};
pub use model::*;
