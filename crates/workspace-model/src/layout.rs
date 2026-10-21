//! Split-pane layout of one tab (PANE-1): a binary tree whose leaves are panes.
//!
//! Port of v0.2.15 `src/features/panes/paneLayout.ts`.

use std::fmt;

/// A pane of a tab; unique across every tab of a window. Shown as `pane-<n>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PaneId(pub u32);

impl fmt::Display for PaneId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "pane-{}", self.0)
    }
}

/// A split node, named after the pane its split created (`split-<new pane>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SplitId(pub PaneId);

impl fmt::Display for SplitId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "split-{}", self.0)
    }
}

/// `Row` places the two halves side by side, `Column` stacks them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SplitDirection {
    Row,
    Column,
}

/// The direction of Alt+Arrow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FocusDirection {
    Left,
    Right,
    Up,
    Down,
}

/// Where a split puts the new pane: `After` the old one (right or below), or `Before` it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Side {
    Before,
    #[default]
    After,
}

/// A rectangle inside the unit square.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// One split of the tree: `a` is left of (row) or above (column) `b`, taking `ratio` of the room.
#[derive(Debug, Clone, PartialEq)]
pub struct Split {
    pub id: SplitId,
    pub direction: SplitDirection,
    pub ratio: f64,
    pub a: Box<PaneLayout>,
    pub b: Box<PaneLayout>,
}

/// The layout of one tab.
#[derive(Debug, Clone, PartialEq)]
pub enum PaneLayout {
    Pane(PaneId),
    Split(Split),
}

/// A split with the rectangle it divides (where its draggable divider goes).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SplitBox {
    pub id: SplitId,
    pub direction: SplitDirection,
    pub ratio: f64,
    pub rect: Rect,
}

pub const MAX_PANES: usize = 4;
pub const MIN_RATIO: f64 = 0.15;
pub const MAX_RATIO: f64 = 0.85;

/// A split ratio clamped to `MIN_RATIO..=MAX_RATIO`; a non-finite one is 0.5.
pub fn clamp_ratio(ratio: f64) -> f64 {
    if ratio.is_finite() {
        ratio.clamp(MIN_RATIO, MAX_RATIO)
    } else {
        0.5
    }
}

const UNIT: Rect = Rect {
    x: 0.0,
    y: 0.0,
    w: 1.0,
    h: 1.0,
};

const EPS: f64 = 1e-9;

impl PaneLayout {
    /// A layout of one pane.
    pub fn leaf(id: PaneId) -> Self {
        Self::Pane(id)
    }

    /// The split at the root, if the layout has more than one pane.
    pub fn root_split(&self) -> Option<&Split> {
        match self {
            Self::Pane(_) => None,
            Self::Split(split) => Some(split),
        }
    }

    /// Pane ids in visual order (left to right, top to bottom within each split).
    pub fn pane_ids(&self) -> Vec<PaneId> {
        let mut ids = Vec::new();
        self.collect_ids(&mut ids);
        ids
    }

    fn collect_ids(&self, ids: &mut Vec<PaneId>) {
        match self {
            Self::Pane(id) => ids.push(*id),
            Self::Split(split) => {
                split.a.collect_ids(ids);
                split.b.collect_ids(ids);
            }
        }
    }

    pub fn pane_count(&self) -> usize {
        match self {
            Self::Pane(_) => 1,
            Self::Split(split) => split.a.pane_count() + split.b.pane_count(),
        }
    }

    pub fn contains(&self, pane: PaneId) -> bool {
        match self {
            Self::Pane(id) => *id == pane,
            Self::Split(split) => split.a.contains(pane) || split.b.contains(pane),
        }
    }

    /// Splits `pane` in two halves at ratio 0.5; `new` goes on `side` of it. Refused (false) when
    /// the pane is unknown, `new` already exists or the tab already has `MAX_PANES` panes.
    pub fn split_pane(&mut self, pane: PaneId, direction: SplitDirection, new: PaneId, side: Side) -> bool {
        if self.pane_count() >= MAX_PANES || !self.contains(pane) || self.contains(new) {
            return false;
        }
        self.replace_leaf(pane, direction, new, side)
    }

    fn replace_leaf(&mut self, pane: PaneId, direction: SplitDirection, new: PaneId, side: Side) -> bool {
        match self {
            Self::Pane(id) if *id == pane => {
                let (a, b) = match side {
                    Side::Before => (new, pane),
                    Side::After => (pane, new),
                };
                *self = Self::Split(Split {
                    id: SplitId(new),
                    direction,
                    ratio: 0.5,
                    a: Box::new(Self::Pane(a)),
                    b: Box::new(Self::Pane(b)),
                });
                true
            }
            Self::Pane(_) => false,
            Self::Split(split) => {
                split.a.replace_leaf(pane, direction, new, side) || split.b.replace_leaf(pane, direction, new, side)
            }
        }
    }

    /// The layout without `pane`: its sibling takes the parent's place. `None` when the last pane
    /// closes; an equal layout for an unknown pane.
    pub fn closed(&self, pane: PaneId) -> Option<PaneLayout> {
        match self {
            Self::Pane(id) if *id == pane => None,
            Self::Pane(_) => Some(self.clone()),
            Self::Split(split) => match (split.a.closed(pane), split.b.closed(pane)) {
                (None, rest) | (rest, None) => rest,
                (Some(a), Some(b)) => Some(Self::Split(Split {
                    a: Box::new(a),
                    b: Box::new(b),
                    ..split.clone()
                })),
            },
        }
    }

    /// The layout on screen with `hidden` panes out of sight, as if closed; the whole layout
    /// when every pane would be hidden (DEV-2a).
    pub fn shown(&self, hidden: &[PaneId]) -> PaneLayout {
        hidden
            .iter()
            .try_fold(self.clone(), |layout, id| layout.closed(*id))
            .unwrap_or_else(|| self.clone())
    }

    /// Sets the ratio of one split, clamped; false when unknown or unchanged.
    pub fn set_ratio(&mut self, split_id: SplitId, ratio: f64) -> bool {
        let clamped = clamp_ratio(ratio);
        match self {
            Self::Pane(_) => false,
            Self::Split(split) if split.id == split_id => {
                if split.ratio == clamped {
                    return false;
                }
                split.ratio = clamped;
                true
            }
            Self::Split(split) => split.a.set_ratio(split_id, ratio) || split.b.set_ratio(split_id, ratio),
        }
    }

    fn walk(&self, rect: Rect, visit: &mut impl FnMut(&PaneLayout, Rect)) {
        visit(self, rect);
        let Self::Split(split) = self else {
            return;
        };
        let Rect { x, y, w, h } = rect;
        match split.direction {
            SplitDirection::Row => {
                let wa = w * split.ratio;
                split.a.walk(Rect { x, y, w: wa, h }, visit);
                split.b.walk(
                    Rect {
                        x: x + wa,
                        y,
                        w: w - wa,
                        h,
                    },
                    visit,
                );
            }
            SplitDirection::Column => {
                let ha = h * split.ratio;
                split.a.walk(Rect { x, y, w, h: ha }, visit);
                split.b.walk(
                    Rect {
                        x,
                        y: y + ha,
                        w,
                        h: h - ha,
                    },
                    visit,
                );
            }
        }
    }

    /// The rectangle of every pane inside the unit square, in visual order.
    pub fn pane_rects(&self) -> Vec<(PaneId, Rect)> {
        let mut rects = Vec::new();
        self.walk(UNIT, &mut |node, rect| {
            if let Self::Pane(id) = node {
                rects.push((*id, rect));
            }
        });
        rects
    }

    pub fn pane_rect(&self, pane: PaneId) -> Option<Rect> {
        self.pane_rects()
            .into_iter()
            .find_map(|(id, rect)| (id == pane).then_some(rect))
    }

    /// Every split with the rectangle it divides.
    pub fn split_boxes(&self) -> Vec<SplitBox> {
        let mut boxes = Vec::new();
        self.walk(UNIT, &mut |node, rect| {
            if let Self::Split(split) = node {
                boxes.push(SplitBox {
                    id: split.id,
                    direction: split.direction,
                    ratio: split.ratio,
                    rect,
                });
            }
        });
        boxes
    }

    /// The pane Alt+Arrow moves to: the closest pane in that direction that overlaps the current
    /// one on the perpendicular axis; ties go to the larger overlap. `None` at the edge.
    pub fn neighbour(&self, pane: PaneId, direction: FocusDirection) -> Option<PaneId> {
        let rects = self.pane_rects();
        let from = rects.iter().find_map(|(id, rect)| (*id == pane).then_some(*rect))?;
        let horizontal = matches!(direction, FocusDirection::Left | FocusDirection::Right);
        let mut best: Option<(PaneId, f64, f64)> = None;
        for (id, r) in rects {
            if id == pane {
                continue;
            }
            let distance = match direction {
                FocusDirection::Right => r.x - (from.x + from.w),
                FocusDirection::Left => from.x - (r.x + r.w),
                FocusDirection::Down => r.y - (from.y + from.h),
                FocusDirection::Up => from.y - (r.y + r.h),
            };
            if distance < -EPS {
                continue;
            }
            let overlap = if horizontal {
                (from.y + from.h).min(r.y + r.h) - from.y.max(r.y)
            } else {
                (from.x + from.w).min(r.x + r.w) - from.x.max(r.x)
            };
            if overlap <= EPS {
                continue;
            }
            let better = match best {
                None => true,
                Some((_, d, o)) => distance < d - EPS || ((distance - d).abs() <= EPS && overlap > o + EPS),
            };
            if better {
                best = Some((id, distance, overlap));
            }
        }
        best.map(|(id, _, _)| id)
    }
}
