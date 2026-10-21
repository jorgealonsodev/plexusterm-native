//! ST-3: the byte ring between a source and the parser is bounded, applies backpressure and
//! never drops or reorders output.

use std::thread;
use std::time::{Duration, Instant};

use term_engine::{ByteRing, Closed};

#[test]
fn delivers_bytes_in_order() {
    let ring = ByteRing::new(16);
    ring.push(b"hello").unwrap();
    ring.push(b" world").unwrap();
    assert_eq!(ring.pop(64), Some(b"hello world".to_vec()));
}

#[test]
fn pop_takes_at_most_the_requested_amount() {
    let ring = ByteRing::new(16);
    ring.push(b"abcdef").unwrap();
    assert_eq!(ring.pop(4), Some(b"abcd".to_vec()));
    assert_eq!(ring.try_pop(4), Some(b"ef".to_vec()));
    assert_eq!(
        ring.try_pop(4),
        None,
        "an empty ring has nothing to take without waiting"
    );
}

#[test]
fn a_full_ring_blocks_the_writer_instead_of_growing_or_dropping() {
    let ring = ByteRing::new(8);
    let writer = {
        let ring = ring.clone();
        thread::spawn(move || ring.push(&(0..32u8).collect::<Vec<_>>()))
    };
    let deadline = Instant::now() + Duration::from_secs(5);
    while ring.len() < 8 {
        assert!(Instant::now() < deadline, "the writer never filled the ring");
        thread::sleep(Duration::from_millis(1));
    }
    thread::sleep(Duration::from_millis(20));
    assert_eq!(ring.len(), 8, "the writer filled the ring and waits for room");
    assert!(!writer.is_finished(), "the writer is held back");

    let mut received = Vec::new();
    while received.len() < 32 {
        assert!(ring.len() <= 8, "never more than the capacity is buffered");
        received.extend(ring.pop(3).unwrap());
    }
    writer.join().unwrap().unwrap();
    assert_eq!(
        received,
        (0..32u8).collect::<Vec<_>>(),
        "nothing dropped, nothing reordered"
    );
}

#[test]
fn closing_drains_what_is_left_then_ends_the_reader() {
    let ring = ByteRing::new(8);
    ring.push(b"tail").unwrap();
    ring.close();
    assert_eq!(ring.pop(8), Some(b"tail".to_vec()));
    assert_eq!(ring.pop(8), None);
    assert_eq!(ring.push(b"late"), Err(Closed));
}

#[test]
fn closing_wakes_a_writer_waiting_for_room() {
    let ring = ByteRing::new(2);
    let writer = {
        let ring = ring.clone();
        thread::spawn(move || ring.push(b"abcd"))
    };
    thread::sleep(Duration::from_millis(30));
    ring.close();
    assert_eq!(writer.join().unwrap(), Err(Closed));
}

#[test]
fn reports_whether_it_was_closed() {
    let ring = ByteRing::new(4);
    assert!(!ring.is_closed());
    ring.close();
    assert!(ring.is_closed());
}
