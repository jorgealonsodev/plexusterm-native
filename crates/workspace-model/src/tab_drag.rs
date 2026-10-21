//! Tab drag reorder (TAB-2): a tab dragged along the bar, or a row up or down the DEV column,
//! moves to where it is dropped. Port of the pointer rules of v0.2.15 `TabBar.tsx` `startMove`.

use crate::tabs::TabId;

/// Pointer travel, in px, before a press becomes a drag (`DRAG_THRESHOLD`).
pub const DRAG_THRESHOLD: f64 = 5.0;

/// Where the dragged tab would land: before or after the tab `at` (the 2 px marker line).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DropMarker {
    pub at: TabId,
    pub after: bool,
}

/// The landing index among the other tabs: how many have their middle before `at` (after it
/// along an RTL bar). A tab with no box on screen passes `f64::INFINITY`.
pub fn landing_index(middles: &[f64], at: f64, rtl: bool) -> usize {
    middles
        .iter()
        .filter(|middle| if rtl { **middle > at } else { **middle < at })
        .count()
}

/// One press on a tab, until it is released or cancelled (a cancel just drops the value).
#[derive(Debug, Clone, PartialEq)]
pub struct TabDrag {
    key: TabId,
    from: usize,
    start: (f64, f64),
    started: bool,
    to: Option<usize>,
}

impl TabDrag {
    /// A press on `key` (with `order` the tabs' current order). Only the primary button starts
    /// a drag, and never from the tab's × (`on_close`).
    pub fn press(order: &[TabId], key: TabId, primary: bool, on_close: bool, x: f64, y: f64) -> Option<Self> {
        if !primary || on_close {
            return None;
        }
        let from = order.iter().position(|k| *k == key)?;
        Some(Self {
            key,
            from,
            start: (x, y),
            started: false,
            to: None,
        })
    }

    pub fn started(&self) -> bool {
        self.started
    }

    /// The pointer moved to (`x`, `y`). `others` are the other tabs in order with their middles
    /// along the drag axis, `along` the pointer on that axis (x along the bar, y in the column).
    /// Returns the marker to draw, `None` while it is still a click or there is no other tab.
    pub fn pointer_moved(
        &mut self,
        x: f64,
        y: f64,
        others: &[(TabId, f64)],
        along: f64,
        rtl: bool,
    ) -> Option<DropMarker> {
        if !self.started && (x - self.start.0).hypot(y - self.start.1) < DRAG_THRESHOLD {
            return None;
        }
        self.started = true;
        let middles: Vec<f64> = others.iter().map(|(_, middle)| *middle).collect();
        let index = landing_index(&middles, along, rtl);
        self.to = Some(index);
        let after = index == others.len();
        let target = if after { index.checked_sub(1)? } else { index };
        others.get(target).map(|(at, _)| DropMarker { at: *at, after })
    }

    /// Released: the move to make (`Tabs::move_tab`), or `None` for a click or a drop in place.
    pub fn finish(self) -> Option<(TabId, usize)> {
        let to = self.to?;
        (self.started && to != self.from).then_some((self.key, to))
    }
}
