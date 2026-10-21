//! Port of v0.2.15 `src/features/panes/multiExec.test.ts` (MEXEC-1), plus the on/off and
//! exclusion state `App.tsx` keeps around it.

use std::cell::RefCell;
use std::rc::Rc;

use workspace_model::{MultiExec, PaneId, PaneInfo, WriterRegistry, broadcast_targets, multi_exec_group};

const A: PaneId = PaneId(1);
const B: PaneId = PaneId(2);
const C: PaneId = PaneId(3);
const D: PaneId = PaneId(4);
const E: PaneId = PaneId(5);

fn panes() -> Vec<PaneInfo> {
    vec![
        PaneInfo::new(A, true, false),
        PaneInfo::new(B, true, false),
        PaneInfo::new(C, false, false), // still connecting
        PaneInfo::new(D, true, true),
        PaneInfo::new(E, true, false),
    ]
}

type Log = Rc<RefCell<Vec<String>>>;

fn recorder(log: &Log, name: &'static str) -> Box<dyn FnMut(&str)> {
    let log = Rc::clone(log);
    Box::new(move |data: &str| log.borrow_mut().push(format!("{name}:{data}")))
}

// "contains the connected panes that are not excluded"
#[test]
fn contains_the_connected_panes_that_are_not_excluded() {
    assert_eq!(multi_exec_group(&panes()), vec![A, B, E]);
}

// "sends to every other connected, non-excluded pane"
#[test]
fn sends_to_every_other_connected_non_excluded_pane() {
    assert_eq!(broadcast_targets(&panes(), A), vec![B, E]);
}

// "sends nothing from an excluded or unknown pane"
#[test]
fn sends_nothing_from_an_excluded_or_unknown_pane() {
    assert_eq!(broadcast_targets(&panes(), D), vec![]);
    assert_eq!(broadcast_targets(&panes(), PaneId(99)), vec![]);
    // A pane still connecting reaches only itself too.
    assert_eq!(broadcast_targets(&panes(), C), vec![]);
}

// "writes to the registered panes only"
#[test]
fn writes_to_the_registered_panes_only() {
    let log: Log = Rc::default();
    let mut registry = WriterRegistry::new();
    registry.register(A, recorder(&log, "a"));
    registry.register(B, recorder(&log, "b"));
    registry.write(&[B, PaneId(99)], "ls\r");
    assert_eq!(*log.borrow(), vec!["b:ls\r".to_owned()]);
}

// "unregistering an old writer keeps a newer one for the same pane"
#[test]
fn unregistering_an_old_writer_keeps_a_newer_one_for_the_same_pane() {
    let log: Log = Rc::default();
    let mut registry = WriterRegistry::new();
    let first = registry.register(A, recorder(&log, "first"));
    let second = registry.register(A, recorder(&log, "second"));
    registry.unregister(first);
    registry.write(&[A], "x");
    assert_eq!(*log.borrow(), vec!["second:x".to_owned()]);
    registry.unregister(second);
    registry.write(&[A], "y");
    assert_eq!(*log.borrow(), vec!["second:x".to_owned()]);
}

// MEXEC-1: off at start; typing in the focused pane of the active tab reaches the group; Turn off.
#[test]
fn multi_exec_is_off_at_start_and_broadcasts_only_from_the_focused_pane_while_on() {
    let mut mexec = MultiExec::default();
    assert!(!mexec.is_on());
    assert_eq!(mexec.targets(&panes(), Some(A), A), vec![]);
    mexec.toggle();
    assert!(mexec.is_on());
    assert_eq!(mexec.targets(&panes(), Some(A), A), vec![B, E]);
    // Not the focused pane of the active tab: nothing is mirrored.
    assert_eq!(mexec.targets(&panes(), Some(B), A), vec![]);
    assert_eq!(mexec.targets(&panes(), None, A), vec![]);
    assert_eq!(mexec.count(&panes()), 3);
    mexec.turn_off();
    assert!(!mexec.is_on());
}

// MEXEC-1: the exclude toggle of each pane, forgotten when the pane closes.
#[test]
fn excludes_a_pane_by_its_toggle_and_forgets_closed_panes() {
    let mut mexec = MultiExec::default();
    assert!(!mexec.is_excluded(A));
    assert!(mexec.toggle_excluded(A));
    assert!(mexec.is_excluded(A));
    assert_eq!(mexec.info(A, true), PaneInfo::new(A, true, true));
    assert!(!mexec.toggle_excluded(A));
    mexec.toggle_excluded(B);
    mexec.forget_except(&[A]);
    assert!(!mexec.is_excluded(B));
}
