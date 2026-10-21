//! Port of v0.2.15 `src/features/panes/paneLayout.test.ts` (PANE-1); the original `it(...)`
//! text is quoted above each test.

use workspace_model::{FocusDirection, MAX_PANES, PaneId, PaneLayout, Rect, Side, SplitDirection, SplitId};

const fn p(n: u32) -> PaneId {
    PaneId(n)
}

/// p1 | (p2 / p3): p1 on the left, p2 above p3 on the right.
fn three_panes() -> PaneLayout {
    let mut layout = PaneLayout::leaf(p(1));
    assert!(layout.split_pane(p(1), SplitDirection::Row, p(2), Side::After));
    assert!(layout.split_pane(p(2), SplitDirection::Column, p(3), Side::After));
    layout
}

fn rect(x: f64, y: f64, w: f64, h: f64) -> Rect {
    Rect { x, y, w, h }
}

// "splits a pane in two halves, the new pane after the old one"
#[test]
fn splits_a_pane_in_two_halves_the_new_pane_after_the_old_one() {
    let mut layout = PaneLayout::leaf(p(1));
    assert!(layout.split_pane(p(1), SplitDirection::Row, p(2), Side::After));
    let split = layout.root_split().unwrap();
    assert_eq!(split.direction, SplitDirection::Row);
    assert_eq!(split.ratio, 0.5);
    assert_eq!(split.id, SplitId(p(2)));
    assert_eq!(layout.pane_ids(), vec![p(1), p(2)]);
}

// "can put the new pane before the old one (left of it, or above)"
#[test]
fn can_put_the_new_pane_before_the_old_one() {
    let mut layout = PaneLayout::leaf(p(1));
    assert!(layout.split_pane(p(1), SplitDirection::Row, p(2), Side::Before));
    let split = layout.root_split().unwrap();
    assert_eq!(split.direction, SplitDirection::Row);
    assert_eq!(split.ratio, 0.5);
    assert_eq!(layout.pane_ids(), vec![p(2), p(1)]);
    assert_eq!(layout.pane_rect(p(2)).unwrap().x, 0.0);
}

// "splits a nested pane in place"
#[test]
fn splits_a_nested_pane_in_place() {
    assert_eq!(three_panes().pane_ids(), vec![p(1), p(2), p(3)]);
}

// "refuses to grow a tab beyond ${MAX_PANES} panes"
#[test]
fn refuses_to_grow_a_tab_beyond_max_panes() {
    let mut four = three_panes();
    assert!(four.split_pane(p(1), SplitDirection::Column, p(4), Side::After));
    assert_eq!(four.pane_count(), MAX_PANES);
    let before = four.clone();
    assert!(!four.split_pane(p(4), SplitDirection::Row, p(5), Side::After));
    assert_eq!(four, before);
}

// "leaves the layout unchanged for an unknown pane"
#[test]
fn split_leaves_the_layout_unchanged_for_an_unknown_pane() {
    let mut layout = PaneLayout::leaf(p(1));
    assert!(!layout.split_pane(p(99), SplitDirection::Row, p(2), Side::After));
    assert_eq!(layout, PaneLayout::leaf(p(1)));
}

// Not in v0.2.15's tests: `splitPane` also refuses an id already in the layout.
#[test]
fn split_refuses_a_pane_id_already_in_the_layout() {
    let mut layout = three_panes();
    assert!(!layout.split_pane(p(1), SplitDirection::Row, p(3), Side::After));
    assert_eq!(layout, three_panes());
}

// "returns null when the last pane closes"
#[test]
fn returns_none_when_the_last_pane_closes() {
    assert_eq!(PaneLayout::leaf(p(1)).closed(p(1)), None);
}

// "lets the sibling take the parent's place"
#[test]
fn lets_the_sibling_take_the_parents_place() {
    let closed = three_panes().closed(p(2)).unwrap();
    assert_eq!(closed.pane_ids(), vec![p(1), p(3)]);
    let split = closed.root_split().unwrap();
    assert_eq!(split.direction, SplitDirection::Row);
    assert_eq!(*split.b, PaneLayout::Pane(p(3)));
    assert_eq!(closed.closed(p(1)).unwrap().closed(p(3)), None);
}

// "leaves the layout unchanged for an unknown pane"
#[test]
fn close_leaves_the_layout_unchanged_for_an_unknown_pane() {
    assert_eq!(three_panes().closed(p(99)), Some(three_panes()));
}

// "changes the ratio of one split, clamped to 15%-85%"
#[test]
fn changes_the_ratio_of_one_split_clamped_to_15_85_percent() {
    let at = |layout: &PaneLayout| {
        let outer = layout.root_split().unwrap();
        let inner = outer.b.root_split().unwrap();
        (outer.ratio, inner.ratio)
    };
    let mut layout = three_panes();
    assert!(layout.set_ratio(SplitId(p(3)), 0.3));
    assert_eq!(at(&layout), (0.5, 0.3));
    let mut layout = three_panes();
    assert!(layout.set_ratio(SplitId(p(2)), 0.05));
    assert_eq!(at(&layout), (0.15, 0.5));
    let mut layout = three_panes();
    assert!(layout.set_ratio(SplitId(p(2)), 0.99));
    assert_eq!(at(&layout), (0.85, 0.5));
}

// PANE-1: a non-finite ratio is 0.5; an unknown split changes nothing.
#[test]
fn a_non_finite_ratio_is_one_half_and_an_unknown_split_changes_nothing() {
    let mut layout = three_panes();
    assert!(layout.set_ratio(SplitId(p(2)), 0.3));
    assert!(layout.set_ratio(SplitId(p(2)), f64::NAN));
    assert_eq!(layout.root_split().unwrap().ratio, 0.5);
    assert!(!layout.set_ratio(SplitId(p(2)), 0.5));
    assert!(!layout.set_ratio(SplitId(p(9)), 0.3));
}

// "places panes in a unit square following the ratios"
#[test]
fn places_panes_in_a_unit_square_following_the_ratios() {
    let mut layout = three_panes();
    assert!(layout.set_ratio(SplitId(p(2)), 0.25));
    assert_eq!(
        layout.pane_rects(),
        vec![
            (p(1), rect(0.0, 0.0, 0.25, 1.0)),
            (p(2), rect(0.25, 0.0, 0.75, 0.5)),
            (p(3), rect(0.25, 0.5, 0.75, 0.5)),
        ]
    );
}

// Divider placement (`splitBoxes`, used by PaneGrid.tsx).
#[test]
fn lists_every_split_with_the_rectangle_it_divides() {
    let boxes = three_panes().split_boxes();
    assert_eq!(boxes.len(), 2);
    assert_eq!(boxes[0].id, SplitId(p(2)));
    assert_eq!(boxes[0].rect, rect(0.0, 0.0, 1.0, 1.0));
    assert_eq!(boxes[1].id, SplitId(p(3)));
    assert_eq!(boxes[1].direction, SplitDirection::Column);
    assert_eq!(boxes[1].rect, rect(0.5, 0.0, 0.5, 1.0));
}

// "moves focus to the closest overlapping pane in the direction"
#[test]
fn moves_focus_to_the_closest_overlapping_pane_in_the_direction() {
    let layout = three_panes();
    assert_eq!(layout.neighbour(p(1), FocusDirection::Right), Some(p(2)));
    assert_eq!(layout.neighbour(p(3), FocusDirection::Left), Some(p(1)));
    assert_eq!(layout.neighbour(p(2), FocusDirection::Down), Some(p(3)));
    assert_eq!(layout.neighbour(p(3), FocusDirection::Up), Some(p(2)));
}

// "returns null at the edge or for a single pane"
#[test]
fn returns_none_at_the_edge_or_for_a_single_pane() {
    assert_eq!(three_panes().neighbour(p(1), FocusDirection::Left), None);
    assert_eq!(three_panes().neighbour(p(1), FocusDirection::Up), None);
    assert_eq!(PaneLayout::leaf(p(1)).neighbour(p(1), FocusDirection::Right), None);
}

// "ignores panes that do not overlap on the perpendicular axis"
#[test]
fn ignores_panes_that_do_not_overlap_on_the_perpendicular_axis() {
    // (p1 / p2) | (p3 / p4) with the right column split lower than the left one.
    let mut layout = PaneLayout::leaf(p(1));
    assert!(layout.split_pane(p(1), SplitDirection::Row, p(3), Side::After));
    assert!(layout.split_pane(p(1), SplitDirection::Column, p(2), Side::After));
    assert!(layout.split_pane(p(3), SplitDirection::Column, p(4), Side::After));
    assert!(layout.set_ratio(SplitId(p(4)), 0.8));
    assert_eq!(layout.neighbour(p(2), FocusDirection::Right), Some(p(3)));
    assert_eq!(layout.neighbour(p(4), FocusDirection::Left), Some(p(2)));
}

// DEV-2a (`shownLayout`): hidden panes are out of sight; all of them hidden shows everything.
#[test]
fn shown_layout_hides_panes_and_shows_all_when_every_pane_would_be_hidden() {
    let layout = three_panes();
    assert_eq!(layout.shown(&[p(2)]).pane_ids(), vec![p(1), p(3)]);
    assert_eq!(layout.shown(&[p(1), p(2), p(3)]), layout);
    assert_eq!(layout.shown(&[]), layout);
}
