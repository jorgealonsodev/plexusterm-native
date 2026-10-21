//! Tabs, split panes, multi-execution and DEV mode as plain state machines.
//!
//! A port of PlexusTerm v0.2.15's pure logic (`src/features/panes/paneLayout.ts`,
//! `multiExec.ts`, `src/features/tabs/tabs.ts`, `src/features/devmode/devMode.ts`,
//! `useDevMode.ts`, `recentFolders.ts`) and of the state rules its React components rely on
//! (PRD §7.2: TAB-1, TAB-2, PANE-1..3b, MEXEC-1, DEV-1..2b, TERM-19). No UI types: the window
//! calls these methods and draws what they hold.

pub mod layout;
pub mod multi_exec;
pub mod tab_drag;
pub mod tabs;

pub use layout::{
    FocusDirection, MAX_PANES, MAX_RATIO, MIN_RATIO, PaneId, PaneLayout, Rect, Side, Split, SplitBox, SplitDirection,
    SplitId, clamp_ratio,
};
pub use multi_exec::{MultiExec, PaneInfo, Registration, WriterRegistry, broadcast_targets, multi_exec_group};
pub use tab_drag::{DRAG_THRESHOLD, DropMarker, TabDrag, landing_index};
pub use tabs::{ChatAction, ChatLink, PathOrigin, Tab, TabId, Tabs, ViewerLink, ViewerPosition};
