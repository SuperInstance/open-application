// Copyright 2026 SuperInstance
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! Mutation queue for offline operation.
//!
//! When connectivity is lost, mutations are queued locally with metadata:
//! - Unique ID (UUID v4)
//! - Timestamp (ISO 8601)
//! - Mutation kind (e.g., "create", "update", "delete")
//! - Payload (arbitrary JSON)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A queued mutation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Mutation {
    /// Unique identifier.
    pub id: String,
    /// Timestamp when the mutation was created.
    pub timestamp: DateTime<Utc>,
    /// The kind of mutation (e.g., "create:note", "update:task").
    pub kind: String,
    /// The mutation payload.
    pub payload: serde_json::Value,
    /// Conflict resolution metadata.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}

impl Mutation {
    /// Create a new mutation.
    pub fn new(kind: String, payload: serde_json::Value) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            kind,
            payload,
            retry_count: None,
            last_error: None,
        }
    }

    /// Increment the retry count.
    pub fn increment_retry(&mut self) {
        self.retry_count = Some(self.retry_count.unwrap_or(0) + 1);
    }
}

/// A thread-safe mutation queue for buffering offline operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MutationQueue {
    /// The ordered list of queued mutations.
    mutations: Vec<Mutation>,
    /// Maximum capacity (0 = unlimited).
    #[serde(skip)]
    max_capacity: usize,
}

impl MutationQueue {
    /// Create a new, unbounded mutation queue.
    pub fn new() -> Self {
        Self {
            mutations: Vec::new(),
            max_capacity: 0,
        }
    }

    /// Create a new mutation queue with a maximum capacity.
    /// When the queue is full, the oldest mutation is evicted.
    pub fn with_capacity(max_capacity: usize) -> Self {
        Self {
            mutations: Vec::new(),
            max_capacity,
        }
    }

    /// Push a mutation onto the queue.
    /// If the queue is at capacity, the oldest mutation is evicted.
    pub fn push(&mut self, mutation: Mutation) {
        if self.max_capacity > 0 && self.mutations.len() >= self.max_capacity {
            self.mutations.remove(0);
        }
        self.mutations.push(mutation);
    }

    /// Pop the next mutation for replay (FIFO).
    pub fn pop(&mut self) -> Option<Mutation> {
        if self.mutations.is_empty() {
            None
        } else {
            Some(self.mutations.remove(0))
        }
    }

    /// Peek at the next mutation without removing it.
    pub fn peek(&self) -> Option<&Mutation> {
        self.mutations.first()
    }

    /// Get the number of queued mutations.
    pub fn len(&self) -> usize {
        self.mutations.len()
    }

    /// Check if the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.mutations.is_empty()
    }

    /// Get all queued mutations (for frontend display).
    pub fn all(&self) -> &[Mutation] {
        &self.mutations
    }

    /// Drain all mutations into a Vec.
    pub fn drain(&mut self) -> Vec<Mutation> {
        std::mem::take(&mut self.mutations)
    }

    /// Remove a mutation by ID.
    pub fn remove(&mut self, id: &str) -> Option<Mutation> {
        if let Some(pos) = self.mutations.iter().position(|m| m.id == id) {
            Some(self.mutations.remove(pos))
        } else {
            None
        }
    }
}

impl Default for MutationQueue {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_push_and_pop() {
        let mut queue = MutationQueue::new();
        assert!(queue.is_empty());

        queue.push(Mutation::new("create".into(), serde_json::json!({"text": "hello"})));
        queue.push(Mutation::new("update".into(), serde_json::json!({"id": 1, "text": "world"})));

        assert_eq!(queue.len(), 2);

        let first = queue.pop().unwrap();
        assert_eq!(first.kind, "create");

        let second = queue.pop().unwrap();
        assert_eq!(second.kind, "update");

        assert!(queue.is_empty());
    }

    #[test]
    fn test_peek() {
        let mut queue = MutationQueue::new();
        queue.push(Mutation::new("test".into(), serde_json::json!({"x": 1})));
        let peeked = queue.peek().unwrap();
        assert_eq!(peeked.kind, "test");
        assert_eq!(queue.len(), 1); // still there
    }

    #[test]
    fn test_capacity_eviction() {
        let mut queue = MutationQueue::with_capacity(2);
        queue.push(Mutation::new("a".into(), serde_json::json!({})));
        queue.push(Mutation::new("b".into(), serde_json::json!({})));
        queue.push(Mutation::new("c".into(), serde_json::json!({}))); // should evict "a"

        assert_eq!(queue.len(), 2);
        assert_eq!(queue.pop().unwrap().kind, "b");
        assert_eq!(queue.pop().unwrap().kind, "c");
    }

    #[test]
    fn test_drain() {
        let mut queue = MutationQueue::new();
        queue.push(Mutation::new("a".into(), serde_json::json!({})));
        queue.push(Mutation::new("b".into(), serde_json::json!({})));

        let drained = queue.drain();
        assert_eq!(drained.len(), 2);
        assert!(queue.is_empty());
    }

    #[test]
    fn test_remove_by_id() {
        let mut queue = MutationQueue::new();
        let m = Mutation::new("test".into(), serde_json::json!({}));
        let id = m.id.clone();
        queue.push(m);

        let removed = queue.remove(&id);
        assert!(removed.is_some());
        assert!(queue.is_empty());

        assert!(queue.remove("nonexistent").is_none());
    }

    #[test]
    fn test_retry_increment() {
        let mut m = Mutation::new("test".into(), serde_json::json!({}));
        assert_eq!(m.retry_count, None);
        m.increment_retry();
        assert_eq!(m.retry_count, Some(1));
        m.increment_retry();
        assert_eq!(m.retry_count, Some(2));
    }
}
