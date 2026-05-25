//! `EventBus<T>` — per-subscriber bounded mpsc channels with backpressure.
//!
//! Contrast with `tokio::sync::broadcast` (shared ring buffer, drop-oldest):
//! - Each subscriber has its own channel → slow subscriber can't block others.
//! - Full channel → drop event + increment counter (observable via metrics).
//! - Closed channel → subscriber silently removed on next publish.
//! - Snapshot-then-send pattern → no lock held during sends.
//!
//! Pattern ported from compozy/Go `Bus[T]` (per-subscriber bounded channel +
//! dropped event tracking + RWLock snapshot). See plan section 3.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use tokio::sync::mpsc;
use uuid::Uuid;

pub type SubscriberId = Uuid;

/// Default per-subscriber channel capacity.
pub const DEFAULT_CAP: usize = 256;

struct BusInner<T: Clone + Send + 'static> {
    subs: RwLock<Vec<(SubscriberId, mpsc::Sender<T>)>>,
    drops: AtomicU64,
}

/// Generic event bus with per-subscriber bounded channels.
///
/// Cheaply clonable — all clones share the same `Arc<BusInner>`.
pub struct EventBus<T: Clone + Send + 'static> {
    inner: Arc<BusInner<T>>,
}

impl<T: Clone + Send + 'static> Clone for EventBus<T> {
    fn clone(&self) -> Self {
        Self { inner: Arc::clone(&self.inner) }
    }
}

impl<T: Clone + Send + 'static> Default for EventBus<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Clone + Send + 'static> EventBus<T> {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(BusInner {
                subs: RwLock::new(Vec::new()),
                drops: AtomicU64::new(0),
            }),
        }
    }

    /// Subscribe with a per-subscriber channel of `cap` capacity.
    ///
    /// Returns `(SubscriberId, mpsc::Receiver<T>)`. The ID is stable and can
    /// be passed to [`unsubscribe`] when the subscriber is done.
    pub fn subscribe(&self, cap: usize) -> (SubscriberId, mpsc::Receiver<T>) {
        let id = Uuid::now_v7();
        let (tx, rx) = mpsc::channel(cap);
        self.inner
            .subs
            .write()
            .expect("bus subs poisoned")
            .push((id, tx));
        (id, rx)
    }

    /// Remove a subscriber by ID. No-op if not found.
    pub fn unsubscribe(&self, id: SubscriberId) {
        self.inner
            .subs
            .write()
            .expect("bus subs poisoned")
            .retain(|(sid, _)| *sid != id);
    }

    /// Publish an event to all active subscribers.
    ///
    /// - Snapshot subscribers under a read lock (no lock held during sends).
    /// - `try_send` each: full buffer → increment drop counter; closed channel → mark dead.
    /// - Dead subscribers are removed in a single write-lock pass after sends complete.
    pub fn publish(&self, event: T) {
        let snapshot: Vec<(SubscriberId, mpsc::Sender<T>)> = self
            .inner
            .subs
            .read()
            .expect("bus subs poisoned")
            .clone();

        let mut dead: Vec<SubscriberId> = Vec::new();
        for (id, tx) in &snapshot {
            match tx.try_send(event.clone()) {
                Ok(()) => {}
                Err(mpsc::error::TrySendError::Full(_)) => {
                    self.inner.drops.fetch_add(1, Ordering::Relaxed);
                }
                Err(mpsc::error::TrySendError::Closed(_)) => {
                    dead.push(*id);
                }
            }
        }

        if !dead.is_empty() {
            self.inner
                .subs
                .write()
                .expect("bus subs poisoned")
                .retain(|(id, _)| !dead.contains(id));
        }
    }

    /// Total number of events dropped due to full subscriber buffers.
    pub fn dropped_events(&self) -> u64 {
        self.inner.drops.load(Ordering::Relaxed)
    }

    /// Current number of active subscribers.
    pub fn subscriber_count(&self) -> usize {
        self.inner
            .subs
            .read()
            .expect("bus subs poisoned")
            .len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn single_subscriber_receives() {
        let bus: EventBus<u32> = EventBus::new();
        let (_id, mut rx) = bus.subscribe(8);
        bus.publish(42);
        assert_eq!(rx.recv().await, Some(42));
    }

    #[tokio::test]
    async fn multiple_subscribers_all_receive() {
        let bus: EventBus<&str> = EventBus::new();
        let (_a, mut rx_a) = bus.subscribe(8);
        let (_b, mut rx_b) = bus.subscribe(8);
        bus.publish("hello");
        assert_eq!(rx_a.recv().await, Some("hello"));
        assert_eq!(rx_b.recv().await, Some("hello"));
    }

    #[tokio::test]
    async fn dropped_on_full_channel() {
        let bus: EventBus<u32> = EventBus::new();
        let (_id, _rx) = bus.subscribe(1); // cap=1, don't read
        bus.publish(1); // fills channel
        bus.publish(2); // drops (channel full)
        assert_eq!(bus.dropped_events(), 1);
    }

    #[tokio::test]
    async fn dead_subscriber_removed_on_publish() {
        let bus: EventBus<u32> = EventBus::new();
        let (id, rx) = bus.subscribe(4);
        drop(rx); // close the receiver
        bus.publish(99);
        // subscriber should be cleaned up
        assert_eq!(bus.subscriber_count(), 0);
        let _ = id; // just to show we held it
    }

    #[tokio::test]
    async fn unsubscribe_removes_subscriber() {
        let bus: EventBus<u32> = EventBus::new();
        let (id, _rx) = bus.subscribe(4);
        assert_eq!(bus.subscriber_count(), 1);
        bus.unsubscribe(id);
        assert_eq!(bus.subscriber_count(), 0);
    }

    #[tokio::test]
    async fn clone_shares_state() {
        let bus1: EventBus<u32> = EventBus::new();
        let bus2 = bus1.clone();
        let (_id, mut rx) = bus1.subscribe(4);
        bus2.publish(7); // publish via clone
        assert_eq!(rx.recv().await, Some(7));
    }
}
