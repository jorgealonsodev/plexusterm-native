//! Rate limit for AI requests, per pane (PRD AI-2, TERM-12c), ported from PlexusTerm v0.2.15
//! `src/features/ai/aiLimiter.ts`: at most one request in flight and at least
//! [`AI_MIN_INTERVAL_MS`] between two sends of a pane. Only the newest waiting request is sent;
//! one it replaces resolves to `None`.

use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use tokio::time::Instant;

/// Minimum pause between two AI requests of one pane.
pub const AI_MIN_INTERVAL_MS: u64 = 1000;

/// Per-pane request limiter; panes are told apart by any string id.
pub struct Limiter {
    min_interval: Duration,
    panes: Mutex<HashMap<String, Arc<Pane>>>,
}

#[derive(Default)]
struct Pane {
    state: Mutex<PaneState>,
    /// Woken when the pane frees up or a newer request arrives.
    changed: tokio::sync::Notify,
}

#[derive(Default)]
struct PaneState {
    busy: bool,
    /// When the last request was sent.
    last: Option<Instant>,
    /// Ticket of the newest waiting request; older waiting ones give up.
    newest: u64,
}

impl Pane {
    fn lock(&self) -> std::sync::MutexGuard<'_, PaneState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Frees the pane when its request ends, also when the caller drops it (Stop).
struct Sending<'a>(&'a Pane);

impl Drop for Sending<'_> {
    fn drop(&mut self) {
        self.0.lock().busy = false;
        self.0.changed.notify_waiters();
    }
}

impl Default for Limiter {
    fn default() -> Self {
        Self::new(Duration::from_millis(AI_MIN_INTERVAL_MS))
    }
}

impl Limiter {
    pub fn new(min_interval: Duration) -> Self {
        Self {
            min_interval,
            panes: Mutex::default(),
        }
    }

    fn pane(&self, id: &str) -> Arc<Pane> {
        self.panes
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .entry(id.to_owned())
            .or_default()
            .clone()
    }

    /// Runs `request` for `pane` once the pane is free and the interval has passed. `None` when
    /// a newer request of the same pane replaced this one before it was sent.
    pub async fn run<T, F, Fut>(&self, pane: &str, request: F) -> Option<T>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = T>,
    {
        let pane = self.pane(pane);
        let ticket = {
            let mut state = pane.lock();
            state.newest += 1;
            state.newest
        };
        // The request this one replaces is woken to give up.
        pane.changed.notify_waiters();
        loop {
            let changed = pane.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            let wait_until = {
                let mut state = pane.lock();
                if state.newest != ticket {
                    return None;
                }
                let now = Instant::now();
                match state.last.map(|last| last + self.min_interval) {
                    _ if state.busy => None,
                    Some(ready) if ready > now => Some(ready),
                    _ => {
                        state.busy = true;
                        state.last = Some(now);
                        break;
                    }
                }
            };
            match wait_until {
                Some(ready) => {
                    tokio::select! {
                        () = &mut changed => {}
                        () = tokio::time::sleep_until(ready) => {}
                    }
                }
                None => changed.await,
            }
        }
        let _sending = Sending(&pane);
        Some(request().await)
    }

    /// Forgets a closed pane.
    pub fn remove(&self, pane: &str) {
        self.panes.lock().unwrap_or_else(PoisonError::into_inner).remove(pane);
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing, reason = "test assertions")]

    use std::sync::atomic::{AtomicUsize, Ordering};

    use tokio::sync::oneshot;
    use tokio::time::{Duration, advance};

    use super::*;

    /// Lets every spawned task run until it waits again.
    async fn settle() {
        for _ in 0..10 {
            tokio::task::yield_now().await;
        }
    }

    #[tokio::test(start_paused = true)]
    async fn keeps_at_most_one_request_in_flight_per_pane_sending_the_newest_waiting_one_next() {
        let limit = Arc::new(Limiter::new(Duration::from_millis(1000)));
        let calls = Arc::new([AtomicUsize::new(0), AtomicUsize::new(0), AtomicUsize::new(0)]);
        let (finish_first, first_done) = oneshot::channel::<&str>();
        let (finish_third, third_done) = oneshot::channel::<&str>();

        let (l, c) = (limit.clone(), calls.clone());
        let a = tokio::spawn(async move {
            l.run("p1", || async move {
                c[0].fetch_add(1, Ordering::SeqCst);
                first_done.await.unwrap()
            })
            .await
        });
        settle().await;
        let (l, c) = (limit.clone(), calls.clone());
        let b = tokio::spawn(async move {
            l.run("p1", || async move {
                c[1].fetch_add(1, Ordering::SeqCst);
                "two"
            })
            .await
        });
        settle().await;
        let (l, c) = (limit.clone(), calls.clone());
        let c_task = tokio::spawn(async move {
            l.run("p1", || async move {
                c[2].fetch_add(1, Ordering::SeqCst);
                third_done.await.unwrap()
            })
            .await
        });
        settle().await;
        assert_eq!(calls[0].load(Ordering::SeqCst), 1);
        assert_eq!(calls[1].load(Ordering::SeqCst), 0);
        // A newer request replaces one that has not been sent yet.
        assert_eq!(b.await.unwrap(), None);
        finish_first.send("one").unwrap();
        assert_eq!(a.await.unwrap(), Some("one"));
        advance(Duration::from_millis(1000)).await;
        settle().await;
        assert_eq!(calls[1].load(Ordering::SeqCst), 0);
        assert_eq!(calls[2].load(Ordering::SeqCst), 1);
        finish_third.send("three").unwrap();
        assert_eq!(c_task.await.unwrap(), Some("three"));
    }

    #[tokio::test(start_paused = true)]
    async fn waits_the_minimum_interval_between_two_requests_of_a_pane_but_not_across_panes() {
        let limit = Arc::new(Limiter::new(Duration::from_millis(1000)));
        assert_eq!(limit.run("p1", || async { "1" }).await, Some("1"));
        let sent = Arc::new(AtomicUsize::new(0));
        let (l, s) = (limit.clone(), sent.clone());
        let next = tokio::spawn(async move {
            l.run("p1", || async move {
                s.fetch_add(1, Ordering::SeqCst);
                "2"
            })
            .await
        });
        assert_eq!(limit.run("p2", || async { "x" }).await, Some("x"));
        advance(Duration::from_millis(999)).await;
        settle().await;
        assert_eq!(sent.load(Ordering::SeqCst), 0);
        advance(Duration::from_millis(1)).await;
        settle().await;
        assert_eq!(sent.load(Ordering::SeqCst), 1);
        assert_eq!(next.await.unwrap(), Some("2"));
    }

    #[tokio::test(start_paused = true)]
    async fn passes_errors_on_and_keeps_serving_the_pane() {
        let limit = Limiter::new(Duration::ZERO);
        let failed: Option<Result<&str, &str>> = limit.run("p1", || async { Err("down") }).await;
        assert_eq!(failed, Some(Err("down")));
        assert_eq!(limit.run("p1", || async { "ok" }).await, Some("ok"));
    }

    #[tokio::test(start_paused = true)]
    async fn a_cancelled_request_frees_its_pane() {
        let limit = Arc::new(Limiter::new(Duration::ZERO));
        let l = limit.clone();
        let stuck = tokio::spawn(async move { l.run("p1", std::future::pending::<()>).await });
        settle().await;
        stuck.abort();
        let _ = stuck.await;
        let next = tokio::time::timeout(Duration::from_secs(5), limit.run("p1", || async { "ok" })).await;
        assert_eq!(next.unwrap(), Some("ok"));
    }
}
