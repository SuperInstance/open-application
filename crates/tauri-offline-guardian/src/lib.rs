// Copyright 2025-2026 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Graceful offline degradation for Tauri apps.
//!
//! Instead of crashing or freezing when the network drops, the Offline Guardian
//! absorbs the spike, queues writes, falls back to read-only mode, and replays
//! mutations when connectivity returns.
//!
//! # Timeline
//!
//! | Elapsed | State | Behaviour |
//! |---------|-------|-----------|
//! | ≤ 0 s   | **Online** | Normal operation. |
//! | 0-30 s  | **Deadband** | Network jitter absorbed silently. |
//! | 30 s    | **Queueing** | Mutations buffered locally; reads try cache. |
//! | 5 min   | **Read-only** | Writes rejected; app serves stale data. |
//! | 30 min  | **Offline** | Full offline mode with cached UI. |
//!
//! # Handoff
//!
//! When the network returns, queued mutations replay in submission order.
//! Conflicting writes are resolved with LastWriterWins by default.
//!
//! # Example
//!
//! ```rust
//! use tauri_offline_guardian::{GuardianConfig, OnlineGuardian, OfflinePolicy};
//!
//! let guardian = OnlineGuardian::new(GuardianConfig {
//!     deadband_ms: 30_000,        // absorb 30s of jitter
//!     read_only_after_ms: 300_000,  // 5 min until read-only
//!     offline_after_ms: 1_800_000,  // 30 min until full offline
//!     max_queued_mutations: 1000,
//!     policy: OfflinePolicy::LastWriterWins,
//! });
//!
//! assert!(!guardian.is_offline());
//! ```

#![warn(missing_docs, rust_2018_idioms)]
#![cfg_attr(docsrs, feature(doc_cfg))]

mod policy;
mod queue;
mod state;

pub use policy::OfflinePolicy;
pub use queue::{MutationQueue, QueuedMutation};
pub use state::OnlineState;

use std::time::Duration;

/// Configuration for the Offline Guardian.
///
/// All durations are in milliseconds.
#[derive(Debug, Clone)]
pub struct GuardianConfig {
    /// How long network jitter is absorbed before declaring a problem.
    /// Default: 30 000 (30 s).
    pub deadband_ms: u64,
    /// After this duration, the app enters read-only mode (writes rejected).
    /// Default: 300 000 (5 min).
    pub read_only_after_ms: u64,
    /// After this duration, the app enters full offline mode.
    /// Default: 1 800 000 (30 min).
    pub offline_after_ms: u64,
    /// Maximum number of queued mutations before backpressure kicks in.
    pub max_queued_mutations: usize,
    /// How to resolve conflicts on reconnect.
    pub policy: OfflinePolicy,
}

impl Default for GuardianConfig {
    fn default() -> Self {
        Self {
            deadband_ms: 30_000,
            read_only_after_ms: 300_000,
            offline_after_ms: 1_800_000,
            max_queued_mutations: 1000,
            policy: OfflinePolicy::LastWriterWins,
        }
    }
}

/// The Offline Guardian.
///
/// Tracks connectivity state, manages the mutation queue, and reports the
/// current operating mode so the rest of the app can adapt its behaviour.
#[derive(Debug)]
pub struct OnlineGuardian {
    config: GuardianConfig,
    state: OnlineState,
    queue: MutationQueue,
    offline_since: Option<std::time::Instant>,
}

impl OnlineGuardian {
    /// Create a new guardian with the given configuration.
    pub fn new(config: GuardianConfig) -> Self {
        Self {
            state: OnlineState::Online,
            queue: MutationQueue::new(config.max_queued_mutations),
            config,
            offline_since: None,
        }
    }

    /// Returns `true` if the network is considered lost.
    pub fn is_offline(&self) -> bool {
        self.state != OnlineState::Online
    }

    /// Returns the current connectivity state.
    pub fn state(&self) -> OnlineState {
        self.state
    }

    /// Notify the guardian that the network went down.
    ///
    /// Returns the transitioned state.
    pub fn network_lost(&mut self) -> OnlineState {
        if self.offline_since.is_none() {
            self.offline_since = Some(std::time::Instant::now());
        }
        self.recompute()
    }

    /// Notify the guardian that the network returned.
    ///
    /// Returns any queued mutations that are ready to be replayed.
    pub fn network_restored(&mut self) -> Vec<QueuedMutation> {
        self.offline_since = None;
        self.state = OnlineState::Online;
        self.queue.drain()
    }

    /// Submit a mutation while offline.
    ///
    /// Returns `Ok(())` if queued, `Err(mutation)` if the queue is full or writes are
    /// not allowed (read-only / offline modes may reject depending on policy).
    pub fn submit_mutation(&mut self, mutation: QueuedMutation) -> Result<(), QueuedMutation> {
        if matches!(self.state, OnlineState::ReadOnly | OnlineState::Offline) {
            return Err(mutation);
        }
        self.queue.push(mutation)
    }

    /// Number of queued mutations awaiting replay.
    pub fn queued_count(&self) -> usize {
        self.queue.len()
    }

    /// Recompute state based on elapsed offline time.
    fn recompute(&mut self) -> OnlineState {
        let elapsed = self
            .offline_since
            .map(|t| t.elapsed())
            .unwrap_or(Duration::ZERO);

        let elapsed_ms = elapsed.as_millis() as u64;

        self.state = if elapsed_ms < self.config.deadband_ms {
            OnlineState::Online
        } else if elapsed_ms < self.config.read_only_after_ms {
            OnlineState::Deadband
        } else if elapsed_ms < self.config.offline_after_ms {
            OnlineState::ReadOnly
        } else {
            OnlineState::Offline
        };

        self.state
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn starts_online() {
        let g = OnlineGuardian::new(GuardianConfig::default());
        assert_eq!(g.state(), OnlineState::Online);
        assert!(!g.is_offline());
    }

    #[test]
    fn deadband_absorbs_short_offline() {
        let mut g = OnlineGuardian::new(GuardianConfig {
            deadband_ms: 100,
            read_only_after_ms: 200,
            offline_after_ms: 300,
            ..Default::default()
        });

        // Just lost — still inside deadband
        g.network_lost();
        // After deadband but before read-only
        std::thread::sleep(Duration::from_millis(120));
        g.network_lost();
        assert_eq!(g.state(), OnlineState::Deadband);
    }

    #[test]
    fn transitions_to_read_only() {
        let mut g = OnlineGuardian::new(GuardianConfig {
            deadband_ms: 5,
            read_only_after_ms: 50,
            offline_after_ms: 200,
            ..Default::default()
        });

        g.network_lost();
        std::thread::sleep(Duration::from_millis(60));
        g.network_lost();
        assert_eq!(g.state(), OnlineState::ReadOnly);
    }

    #[test]
    fn transitions_to_offline() {
        let mut g = OnlineGuardian::new(GuardianConfig {
            deadband_ms: 5,
            read_only_after_ms: 50,
            offline_after_ms: 100,
            ..Default::default()
        });

        g.network_lost();
        std::thread::sleep(Duration::from_millis(120));
        g.network_lost();
        assert_eq!(g.state(), OnlineState::Offline);
    }

    #[test]
    fn rejects_mutations_in_read_only() {
        let mut g = OnlineGuardian::new(GuardianConfig {
            deadband_ms: 5,
            read_only_after_ms: 50,
            offline_after_ms: 200,
            ..Default::default()
        });

        g.network_lost();
        std::thread::sleep(Duration::from_millis(60));
        g.network_lost();

        let mutation = QueuedMutation::new("set_theme".into(), r#"{"dark":true}"#.into());
        assert!(g.submit_mutation(mutation).is_err());
    }

    #[test]
    fn queues_mutations_in_deadband() {
        let mut g = OnlineGuardian::new(GuardianConfig {
            deadband_ms: 100,
            read_only_after_ms: 200,
            offline_after_ms: 300,
            max_queued_mutations: 100,
            ..Default::default()
        });

        g.network_lost();
        std::thread::sleep(Duration::from_millis(10));
        g.network_lost();

        let m1 = QueuedMutation::new("save_doc".into(), r#"{"id":1}"#.into());
        let m2 = QueuedMutation::new("save_doc".into(), r#"{"id":2}"#.into());
        assert!(g.submit_mutation(m1).is_ok());
        assert!(g.submit_mutation(m2).is_ok());
        assert_eq!(g.queued_count(), 2);
    }

    #[test]
    fn restores_and_drains_queue() {
        let mut g = OnlineGuardian::new(GuardianConfig {
            deadband_ms: 100,
            read_only_after_ms: 200,
            offline_after_ms: 300,
            max_queued_mutations: 100,
            ..Default::default()
        });

        g.network_lost();
        std::thread::sleep(Duration::from_millis(10));
        g.network_lost();
        g.submit_mutation(QueuedMutation::new("ping".into(), "{}".into()))
            .unwrap();
        g.submit_mutation(QueuedMutation::new("pong".into(), "{}".into()))
            .unwrap();

        let drained = g.network_restored();
        assert_eq!(drained.len(), 2);
        assert_eq!(g.queued_count(), 0);
        assert_eq!(g.state(), OnlineState::Online);
    }

    #[test]
    fn respects_queue_capacity() {
        let mut g = OnlineGuardian::new(GuardianConfig {
            max_queued_mutations: 2,
            ..Default::default()
        });

        g.network_lost();
        std::thread::sleep(Duration::from_millis(10));
        g.network_lost();

        assert!(g
            .submit_mutation(QueuedMutation::new("a".into(), "{}".into()))
            .is_ok());
        assert!(g
            .submit_mutation(QueuedMutation::new("b".into(), "{}".into()))
            .is_ok());
        assert!(
            g.submit_mutation(QueuedMutation::new("c".into(), "{}".into()))
                .is_err()
        );
    }
}
