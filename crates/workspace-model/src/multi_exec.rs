//! Multi-execution (MEXEC-1): typing in the focused pane also goes to the other panes of the
//! group. Port of v0.2.15 `src/features/panes/multiExec.ts` and the state `App.tsx` keeps.

use std::collections::{BTreeMap, BTreeSet};

use crate::layout::PaneId;

/// A terminal pane as multi-execution sees it (chat and viewer panes are never listed).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaneInfo {
    pub id: PaneId,
    /// False while the pane is still connecting or after its session ended.
    pub connected: bool,
    pub excluded: bool,
}

impl PaneInfo {
    pub const fn new(id: PaneId, connected: bool, excluded: bool) -> Self {
        Self {
            id,
            connected,
            excluded,
        }
    }
}

/// Panes that take part in multi-execution: connected and not excluded.
pub fn multi_exec_group(panes: &[PaneInfo]) -> Vec<PaneId> {
    panes
        .iter()
        .filter(|p| p.connected && !p.excluded)
        .map(|p| p.id)
        .collect()
}

/// Panes that receive the input typed in `source`; nothing when the source is not in the group.
pub fn broadcast_targets(panes: &[PaneInfo], source: PaneId) -> Vec<PaneId> {
    let group = multi_exec_group(panes);
    if !group.contains(&source) {
        return Vec::new();
    }
    group.into_iter().filter(|id| *id != source).collect()
}

/// The on/off switch (off at start, never persisted) and the panes excluded from it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MultiExec {
    on: bool,
    excluded: BTreeSet<PaneId>,
}

impl MultiExec {
    pub fn is_on(&self) -> bool {
        self.on
    }

    /// Ctrl+Shift+M and the toolbar's **Multi-exec**.
    pub fn toggle(&mut self) {
        self.on = !self.on;
    }

    /// The banner's **Turn off**.
    pub fn turn_off(&mut self) {
        self.on = false;
    }

    pub fn is_excluded(&self, pane: PaneId) -> bool {
        self.excluded.contains(&pane)
    }

    /// The pane's **exclude** toggle; returns whether it is excluded now.
    pub fn toggle_excluded(&mut self, pane: PaneId) -> bool {
        if self.excluded.remove(&pane) {
            false
        } else {
            self.excluded.insert(pane);
            true
        }
    }

    /// Forgets the exclusions of panes that closed.
    pub fn forget_except(&mut self, open: &[PaneId]) {
        self.excluded.retain(|pane| open.contains(pane));
    }

    /// The `PaneInfo` of a terminal pane, with its exclusion.
    pub fn info(&self, pane: PaneId, connected: bool) -> PaneInfo {
        PaneInfo::new(pane, connected, self.is_excluded(pane))
    }

    /// The banner's count: connected, non-excluded terminal panes of every tab.
    pub fn count(&self, panes: &[PaneInfo]) -> usize {
        multi_exec_group(panes).len()
    }

    /// Where input typed in `source` is mirrored: only while on and only from the focused pane of
    /// the active tab (`focused`).
    pub fn targets(&self, panes: &[PaneInfo], focused: Option<PaneId>, source: PaneId) -> Vec<PaneId> {
        if !self.on || focused != Some(source) {
            return Vec::new();
        }
        broadcast_targets(panes, source)
    }
}

/// A registered writer; unregistering it never removes a newer writer for the same pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Registration {
    pane: PaneId,
    generation: u64,
}

/// Where each mounted terminal pane can be written to, as if the user typed it.
pub struct WriterRegistry<W> {
    writers: BTreeMap<PaneId, (u64, W)>,
    next: u64,
}

impl<W> Default for WriterRegistry<W> {
    fn default() -> Self {
        Self {
            writers: BTreeMap::new(),
            next: 0,
        }
    }
}

impl<W: FnMut(&str)> WriterRegistry<W> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, pane: PaneId, writer: W) -> Registration {
        self.next += 1;
        self.writers.insert(pane, (self.next, writer));
        Registration {
            pane,
            generation: self.next,
        }
    }

    pub fn unregister(&mut self, registration: Registration) {
        if self
            .writers
            .get(&registration.pane)
            .is_some_and(|(generation, _)| *generation == registration.generation)
        {
            self.writers.remove(&registration.pane);
        }
    }

    pub fn write(&mut self, panes: &[PaneId], data: &str) {
        for pane in panes {
            if let Some((_, writer)) = self.writers.get_mut(pane) {
                writer(data);
            }
        }
    }
}
