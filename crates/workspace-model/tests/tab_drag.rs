//! Tab drag reorder (TAB-2): the pointer rules of v0.2.15 `TabBar.tsx` `startMove`, checked
//! there only through rendered components (`TabBar.test.tsx`).

use workspace_model::{DRAG_THRESHOLD, DropMarker, TabDrag, TabId, Tabs, landing_index};

const fn t(n: u32) -> TabId {
    TabId(n)
}

/// tab-1..tab-3 along the bar, each 100 px wide: middles at 50, 150, 250.
fn middles_without(dragged: TabId) -> Vec<(TabId, f64)> {
    [(t(1), 50.0), (t(2), 150.0), (t(3), 250.0)]
        .into_iter()
        .filter(|(key, _)| *key != dragged)
        .collect()
}

fn order() -> Vec<TabId> {
    vec![t(1), t(2), t(3)]
}

// The landing index counts the other tabs whose middle lies before the pointer (RTL: after).
#[test]
fn the_landing_index_counts_the_other_tabs_whose_middle_is_before_the_pointer() {
    let middles = [50.0, 250.0];
    assert_eq!(landing_index(&middles, 10.0, false), 0);
    assert_eq!(landing_index(&middles, 100.0, false), 1);
    assert_eq!(landing_index(&middles, 300.0, false), 2);
    assert_eq!(landing_index(&middles, 300.0, true), 0);
    assert_eq!(landing_index(&middles, 10.0, true), 2);
    // A tab with no box on screen never counts.
    assert_eq!(landing_index(&[f64::INFINITY], 10.0, false), 0);
}

// A drag starts with the primary button, not from the close button.
#[test]
fn a_drag_starts_with_the_primary_button_away_from_the_close_button() {
    assert!(TabDrag::press(&order(), t(1), true, false, 0.0, 0.0).is_some());
    assert!(TabDrag::press(&order(), t(1), false, false, 0.0, 0.0).is_none());
    assert!(TabDrag::press(&order(), t(1), true, true, 0.0, 0.0).is_none());
    assert!(TabDrag::press(&order(), t(9), true, false, 0.0, 0.0).is_none());
}

// A plain click, or a move under 5 px, moves nothing.
#[test]
fn a_plain_click_or_a_short_move_moves_nothing() {
    let drag = TabDrag::press(&order(), t(1), true, false, 50.0, 10.0).unwrap();
    assert_eq!(drag.finish(), None);
    let mut drag = TabDrag::press(&order(), t(1), true, false, 50.0, 10.0).unwrap();
    let short = 50.0 + DRAG_THRESHOLD - 1.0;
    assert_eq!(
        drag.pointer_moved(short, 10.0, &middles_without(t(1)), short, false),
        None
    );
    assert!(!drag.started());
    assert_eq!(drag.finish(), None);
}

// Dragged past the threshold, the marker shows where it lands; the drop moves the tab there.
#[test]
fn a_dragged_tab_shows_its_landing_place_and_moves_there_on_drop() {
    let mut drag = TabDrag::press(&order(), t(1), true, false, 50.0, 10.0).unwrap();
    let marker = drag.pointer_moved(200.0, 10.0, &middles_without(t(1)), 200.0, false);
    assert_eq!(marker, Some(DropMarker { at: t(3), after: false }));
    let marker = drag.pointer_moved(300.0, 10.0, &middles_without(t(1)), 300.0, false);
    assert_eq!(marker, Some(DropMarker { at: t(3), after: true }));
    assert!(drag.started());
    let (key, to) = drag.finish().unwrap();
    let mut tabs: Tabs<()> = Tabs::new();
    for _ in 0..3 {
        tabs.open(());
    }
    tabs.move_tab(key, to);
    let keys: Vec<_> = tabs.tabs().iter().map(|tab| tab.key()).collect();
    assert_eq!(keys, vec![t(2), t(3), t(1)]);
    assert_eq!(tabs.active_key(), Some(t(3)));
}

// A drop back in place moves nothing; a cancelled drag is dropped without a move.
#[test]
fn a_drop_back_in_place_moves_nothing() {
    let mut drag = TabDrag::press(&order(), t(2), true, false, 150.0, 10.0).unwrap();
    drag.pointer_moved(170.0, 10.0, &middles_without(t(2)), 170.0, false);
    assert!(drag.started());
    assert_eq!(drag.finish(), None);
}

// The DEV column drags along y the same way; a single tab shows no marker.
#[test]
fn the_column_drags_along_y_and_a_single_tab_has_no_marker() {
    let mut drag = TabDrag::press(&order(), t(3), true, false, 10.0, 250.0).unwrap();
    let rows = [(t(1), 20.0), (t(2), 60.0)];
    assert_eq!(
        drag.pointer_moved(10.0, 5.0, &rows, 5.0, false),
        Some(DropMarker { at: t(1), after: false })
    );
    assert_eq!(drag.finish(), Some((t(3), 0)));
    let mut alone = TabDrag::press(&[t(1)], t(1), true, false, 0.0, 0.0).unwrap();
    assert_eq!(alone.pointer_moved(40.0, 0.0, &[], 40.0, false), None);
    assert_eq!(alone.finish(), None);
}
