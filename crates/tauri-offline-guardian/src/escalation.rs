// Copyright 2026 SuperInstance
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! Escalation state machine.
//!
//! Tracks how long the app has been offline and escalates through
//! four states:
//!
//! 1. **Normal** — fully connected, all features available.
//! 2. **QueueMutations** — offline for >5 min; mutations are queued locally.
//! 3. **ReadOnly** — offline for >30 min; write operations are blocked,
//!    app enters local-first mode.
//! 4. **FullOffline** — offline for >2 hr; full local snapshot mode.
//!    The app is still 80% useful (push-down principle).

use serde::{Deserialize, Serialize};

/// Escalation state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Escalation {
    /// Everything is normal — fully connected.
    Normal,
    /// Offline >5 min — start queueing mutations for later replay.
    QueueMutations,
    /// Offline >30 min — degrade to read-only. No new writes.
    ReadOnly,
    /// Offline >2 hr — full offline mode. App is local-snapshot-only.
    FullOffline,
}

impl Escalation {
    /// Returns `true` if write operations should be allowed.
    pub fn can_write(&self) -> bool {
        !matches!(self, Self::ReadOnly | Self::FullOffline)
    }

    /// Returns `true` if the app should queue mutations instead of
    /// attempting real-time sync.
    pub fn should_queue(&self) -> bool {
        matches!(self, Self::QueueMutations | Self::ReadOnly | Self::FullOffline)
    }

    /// Returns `true` if this is the normal state.
    pub fn is_normal(&self) -> bool {
        matches!(self, Self::Normal)
    }

    /// Returns the human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Normal => "Normal",
            Self::QueueMutations => "Queueing Mutations",
            Self::ReadOnly => "Read-Only",
            Self::FullOffline => "Full Offline",
        }
    }

    /// Returns the severity level (higher = more restricted).
    pub fn severity(&self) -> u8 {
        match self {
            Self::Normal => 0,
            Self::QueueMutations => 1,
            Self::ReadOnly => 2,
            Self::FullOffline => 3,
        }
    }
}

impl std::fmt::Display for Escalation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normal_allows_writes() {
        assert!(Escalation::Normal.can_write());
    }

    #[test]
    fn test_queue_mutations_allows_writes() {
        assert!(Escalation::QueueMutations.can_write());
    }

    #[test]
    fn test_readonly_blocks_writes() {
        assert!(!Escalation::ReadOnly.can_write());
    }

    #[test]
    fn test_full_offline_blocks_writes() {
        assert!(!Escalation::FullOffline.can_write());
    }

    #[test]
    fn test_should_queue() {
        assert!(Escalation::QueueMutations.should_queue());
        assert!(Escalation::ReadOnly.should_queue());
        assert!(Escalation::FullOffline.should_queue());
        assert!(!Escalation::Normal.should_queue());
    }

    #[test]
    fn test_severity() {
        assert_eq!(Escalation::Normal.severity(), 0);
        assert_eq!(Escalation::QueueMutations.severity(), 1);
        assert_eq!(Escalation::ReadOnly.severity(), 2);
        assert_eq!(Escalation::FullOffline.severity(), 3);
    }

    #[test]
    fn test_labels() {
        assert_eq!(Escalation::Normal.to_string(), "Normal");
        assert_eq!(Escalation::QueueMutations.to_string(), "Queueing Mutations");
        assert_eq!(Escalation::ReadOnly.to_string(), "Read-Only");
        assert_eq!(Escalation::FullOffline.to_string(), "Full Offline");
    }
}
