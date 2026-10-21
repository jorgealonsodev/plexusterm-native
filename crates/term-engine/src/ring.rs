use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};

/// Returned when writing to a ring that was closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Closed;

impl std::fmt::Display for Closed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("the session's byte ring is closed")
    }
}

impl std::error::Error for Closed {}

/// Why [`ByteRing::try_push`] did not take the bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TryPushError {
    /// Not enough room right now; nothing was written.
    Full,
    Closed,
}

struct State {
    bytes: VecDeque<u8>,
    closed: bool,
}

struct Inner {
    capacity: usize,
    state: Mutex<State>,
    readable: Condvar,
    writable: Condvar,
}

/// A bounded byte queue between a session source and its parser (PRD §6.3 ST-3).
///
/// Writers block while the ring is full, so a slow parser holds back the source (a PTY stops
/// being read, an SSH channel stops being consumed) instead of output being dropped or memory
/// growing with output speed. Clones share the same ring.
#[derive(Clone)]
pub struct ByteRing(Arc<Inner>);

impl ByteRing {
    /// Session rings hold 4 MiB (ST-3).
    pub const SESSION_CAPACITY: usize = 4 * 1024 * 1024;

    pub fn new(capacity: usize) -> Self {
        Self(Arc::new(Inner {
            capacity: capacity.max(1),
            state: Mutex::new(State {
                bytes: VecDeque::new(),
                closed: false,
            }),
            readable: Condvar::new(),
            writable: Condvar::new(),
        }))
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        // The state stays consistent at every await point, so a poisoned lock is still usable.
        self.0.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Appends all of `bytes`, waiting for room as often as needed.
    pub fn push(&self, mut bytes: &[u8]) -> Result<(), Closed> {
        let mut state = self.lock();
        while !bytes.is_empty() {
            if state.closed {
                return Err(Closed);
            }
            let room = self.0.capacity.saturating_sub(state.bytes.len());
            if room == 0 {
                state = self.0.writable.wait(state).unwrap_or_else(PoisonError::into_inner);
                continue;
            }
            let (now, rest) = bytes.split_at(room.min(bytes.len()));
            state.bytes.extend(now);
            bytes = rest;
            self.0.readable.notify_one();
        }
        if state.closed { Err(Closed) } else { Ok(()) }
    }

    /// Appends `bytes` only if they all fit right now; never waits.
    pub fn try_push(&self, bytes: &[u8]) -> Result<(), TryPushError> {
        let mut state = self.lock();
        if state.closed {
            return Err(TryPushError::Closed);
        }
        if self.0.capacity.saturating_sub(state.bytes.len()) < bytes.len() {
            return Err(TryPushError::Full);
        }
        state.bytes.extend(bytes);
        self.0.readable.notify_one();
        Ok(())
    }

    /// Takes up to `max` bytes, waiting until some arrive; `None` once closed and drained.
    pub fn pop(&self, max: usize) -> Option<Vec<u8>> {
        let mut state = self.lock();
        while state.bytes.is_empty() {
            if state.closed {
                return None;
            }
            state = self.0.readable.wait(state).unwrap_or_else(PoisonError::into_inner);
        }
        Some(self.take(&mut state, max))
    }

    /// Takes up to `max` bytes without waiting.
    pub fn try_pop(&self, max: usize) -> Option<Vec<u8>> {
        let mut state = self.lock();
        if state.bytes.is_empty() {
            None
        } else {
            Some(self.take(&mut state, max))
        }
    }

    fn take(&self, state: &mut State, max: usize) -> Vec<u8> {
        let count = max.min(state.bytes.len());
        let taken = state.bytes.drain(..count).collect();
        self.0.writable.notify_all();
        taken
    }

    /// Ends the stream: waiting writers fail, the reader drains what is left and then sees the end.
    pub fn close(&self) {
        self.lock().closed = true;
        self.0.readable.notify_all();
        self.0.writable.notify_all();
    }

    /// Whether [`ByteRing::close`] was called (buffered bytes may still be waiting).
    pub fn is_closed(&self) -> bool {
        self.lock().closed
    }

    /// Bytes currently buffered (never more than the capacity).
    pub fn len(&self) -> usize {
        self.lock().bytes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn capacity(&self) -> usize {
        self.0.capacity
    }
}
