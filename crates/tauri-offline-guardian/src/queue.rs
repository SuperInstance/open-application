// SPDX-License-Identifier: Apache-2.0 OR MIT

use serde::{Deserialize, Serialize};

/// A single mutation queued for replay when connectivity returns.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueuedMutation {
    /// The command or endpoint key.
    pub command: String,
    /// JSON payload.
    pub payload: String,
    /// Monotonic timestamp (ms since guardian creation).
    pub timestamp_ms: u128,
}

impl QueuedMutation {
    /// Create a new queued mutation with the current instant.
    pub fn new(command: String, payload: String) -> Self {
        Self {
            command,
            payload,
            timestamp_ms: now_ms(),
        }
    }
}

fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

/// A bounded FIFO queue of mutations.
#[derive(Debug)]
pub struct MutationQueue {
    inner: Vec<QueuedMutation>,
    capacity: usize,
}

impl MutationQueue {
    /// Create an empty queue with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: Vec::with_capacity(capacity),
            capacity,
        }
    }

    /// Push a mutation onto the queue.
    ///
    /// Returns `Err(mutation)` if the queue is full.
    pub fn push(&mut self, mutation: QueuedMutation) -> Result<(), QueuedMutation> {
        if self.inner.len() >= self.capacity {
            return Err(mutation);
        }
        self.inner.push(mutation);
        Ok(())
    }

    /// Drain all queued mutations (FIFO order).
    pub fn drain(&mut self) -> Vec<QueuedMutation> {
        std::mem::take(&mut self.inner)
    }

    /// Number of mutations currently queued.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Returns `true` if the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_and_drain() {
        let mut q = MutationQueue::new(10);
        q.push(QueuedMutation::new("a".into(), "1".into()))
            .unwrap();
        q.push(QueuedMutation::new("b".into(), "2".into()))
            .unwrap();
        assert_eq!(q.len(), 2);

        let items = q.drain();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].command, "a");
        assert_eq!(items[1].command, "b");
        assert!(q.is_empty());
    }

    #[test]
    fn full_queue_rejects() {
        let mut q = MutationQueue::new(1);
        q.push(QueuedMutation::new("a".into(), "1".into()))
            .unwrap();
        assert!(q.push(QueuedMutation::new("b".into(), "2".into())).is_err());
    }
}
