// Copyright 2026 SuperInstance
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! Sync handoff — replay queued mutations when connectivity returns.
//!
//! Uses a pluggable conflict resolution strategy. The default strategy is
//! Last-Writer-Wins (LWW) based on mutation timestamps.

use crate::queue::{Mutation, MutationQueue};
use crate::tier::DeviceTier;

/// Result of a sync handoff operation.
#[derive(Debug, Clone)]
pub struct HandoffResult {
    /// Number of mutations successfully replayed.
    pub replayed: usize,
    /// Number of mutations that produced conflicts.
    pub conflicts: usize,
    /// Any error message from the handoff.
    pub error: Option<String>,
}

/// Conflict resolution strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictStrategy {
    /// Last write wins (default). The mutation with the later timestamp wins.
    LastWriterWins,
    /// The server / cloud state is authoritative.
    ServerWins,
    /// The local state is authoritative.
    LocalWins,
    /// Manual resolution — flag all conflicts for user intervention.
    Manual,
}

/// Handles sync of queued mutations when connectivity is restored.
#[derive(Debug, Clone)]
pub struct Handoff {
    /// The conflict resolution strategy.
    pub strategy: ConflictStrategy,
    /// Maximum retries per mutation before giving up.
    pub max_retries: u32,
}

impl Default for Handoff {
    fn default() -> Self {
        Self {
            strategy: ConflictStrategy::LastWriterWins,
            max_retries: 3,
        }
    }
}

impl Handoff {
    /// Create a new handoff handler.
    pub fn new(strategy: ConflictStrategy, max_retries: u32) -> Self {
        Self {
            strategy,
            max_retries,
        }
    }

    /// Perform a sync handoff: replay all queued mutations.
    ///
    /// In a production app, this would actually call the remote API.
    /// For now, it simulates a successful replay of mutations that haven't
    /// exceeded the retry limit.
    pub fn sync(
        &self,
        queue: &parking_lot::RwLock<MutationQueue>,
        tier: &parking_lot::RwLock<DeviceTier>,
    ) -> HandoffResult {
        let current_tier = *tier.read();

        // Cloud tier can sync everything. Cortex can also sync.
        if !current_tier.can_sync() {
            return HandoffResult {
                replayed: 0,
                conflicts: 0,
                error: Some("Cannot sync in Limb tier".into()),
            };
        }

        let mut replayed = 0;
        let mut conflicts = 0;

        // Drain and attempt to replay each mutation
        let mut q = queue.write();
        let mutations = q.drain();

        for mut mutation in mutations {
            if mutation.retry_count.unwrap_or(0) >= self.max_retries {
                // Exceeded retries — mark as conflict
                conflicts += 1;
                continue;
            }

            // Simulate replay (in production, call the actual API)
            match self.replay_one(&mutation) {
                Ok(_) => {
                    replayed += 1;
                }
                Err(e) => {
                    mutation.increment_retry();
                    mutation.last_error = Some(e);
                    q.push(mutation); // re-queue for next handoff
                    conflicts += 1;
                }
            }
        }

        // Recalculate tier — if we're back online, upgrade to Cloud
        if replayed > 0 {
            let mut t = tier.write();
            if *t == DeviceTier::Cortex {
                *t = DeviceTier::Cloud;
            }
        }

        HandoffResult {
            replayed,
            conflicts,
            error: None,
        }
    }

    /// Attempt to replay a single mutation.
    ///
    /// In production, this would call the remote API.
    /// This simulation returns successful for mutations without excessive retries.
    fn replay_one(&self, _mutation: &Mutation) -> Result<(), String> {
        // TODO: In production, call the actual backend API here.
        // The mutation's `kind` field determines the endpoint:
        //   "create:note" → POST /notes
        //   "update:note" → PATCH /notes/{id}
        //   "delete:note" → DELETE /notes/{id}
        //
        // Conflict resolution using the configured strategy:
        //   LastWriterWins → server compares timestamps
        //   ServerWins → overwrite local
        //   LocalWins → overwrite server
        //   Manual → raise conflict for user to decide
        //
        // For now, all mutations are considered successful on first attempt.
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use parking_lot::RwLock;
    use std::sync::Arc;

    #[test]
    fn test_handoff_empty_queue() {
        let queue = RwLock::new(MutationQueue::new());
        let tier = RwLock::new(DeviceTier::Cloud);
        let handoff = Handoff::default();

        let result = handoff.sync(&queue, &tier);
        assert_eq!(result.replayed, 0);
        assert_eq!(result.conflicts, 0);
    }

    #[test]
    fn test_handoff_with_mutations() {
        let queue = RwLock::new(MutationQueue::new());
        let tier = RwLock::new(DeviceTier::Cloud);

        {
            let mut q = queue.write();
            q.push(Mutation::new("create:note".into(), serde_json::json!({"title": "hello"})));
            q.push(Mutation::new("update:note".into(), serde_json::json!({"id": "1", "text": "world"})));
            assert_eq!(q.len(), 2);
        }

        let handoff = Handoff::default();
        let result = handoff.sync(&queue, &tier);

        assert_eq!(result.replayed, 2);
        assert_eq!(result.conflicts, 0);
        assert!(queue.read().is_empty());
    }

    #[test]
    fn test_handoff_limb_tier() {
        let queue = RwLock::new(MutationQueue::new());
        let tier = RwLock::new(DeviceTier::Limb);
        let handoff = Handoff::default();

        queue.write().push(Mutation::new("test".into(), serde_json::json!({})));

        let result = handoff.sync(&queue, &tier);
        assert_eq!(result.replayed, 0);
        assert!(result.error.is_some());
    }

    #[test]
    fn test_max_retries() {
        let queue = RwLock::new(MutationQueue::new());
        let tier = RwLock::new(DeviceTier::Cloud);
        let handoff = Handoff::new(ConflictStrategy::LastWriterWins, 2);

        // Create a mutation with retry_count already at max
        let mut mutation = Mutation::new("test".into(), serde_json::json!({}));
        mutation.retry_count = Some(2);
        queue.write().push(mutation);

        let result = handoff.sync(&queue, &tier);
        assert_eq!(result.replayed, 0);
        assert_eq!(result.conflicts, 1); // exceeded retries
    }

    #[test]
    fn test_upgrade_to_cloud_after_sync() {
        let queue = RwLock::new(MutationQueue::new());
        let tier = RwLock::new(DeviceTier::Cortex);
        let handoff = Handoff::default();

        queue.write().push(Mutation::new("test".into(), serde_json::json!({})));

        handoff.sync(&queue, &tier);
        assert_eq!(*tier.read(), DeviceTier::Cloud);
    }
}
