// Copyright 2026 SuperInstance
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! Device tier definitions and detection.
//!
//! Three tiers map to progressively reduced network dependency:
//!
//! | Tier    | Sync Mode       | Data Freshness | Fallback Behavior               |
//! |---------|-----------------|----------------|----------------------------------|
//! | Cloud   | Full real-time  | Latest         | Push mutations immediately       |
//! | Cortex  | Lazy background | Stale-while-revalidate | Queue, batch sync later        |
//! | Limb    | None (offline)  | Local snapshot | Read-only, no outbound requests  |

use serde::{Deserialize, Serialize};

/// Device capability tier.
///
/// Every app starts at Cloud or Cortex and demotes as offline duration grows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DeviceTier {
    /// Full network access. Real-time sync. Everything works.
    Cloud,
    /// Local-first with lazy background sync. Stale data is OK.
    Cortex,
    /// Offline-only. No network requests. All data is local snapshots.
    Limb,
}

impl DeviceTier {
    /// Whether this tier can make new network requests.
    pub fn can_sync(&self) -> bool {
        matches!(self, Self::Cloud | Self::Cortex)
    }

    /// Whether this tier syncs immediately (Cloud) or lazily (Cortex/Limb).
    pub fn is_realtime(&self) -> bool {
        matches!(self, Self::Cloud)
    }

    /// Returns the priority (higher = more capable).
    pub fn priority(&self) -> u8 {
        match self {
            Self::Cloud => 3,
            Self::Cortex => 2,
            Self::Limb => 1,
        }
    }

    /// Whether this is the most restricted tier.
    pub fn is_limb(&self) -> bool {
        matches!(self, Self::Limb)
    }

    /// The human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Cloud => "Cloud",
            Self::Cortex => "Cortex",
            Self::Limb => "Limb",
        }
    }
}

impl std::fmt::Display for DeviceTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label())
    }
}

impl Default for DeviceTier {
    fn default() -> Self {
        Self::Cortex
    }
}

/// Heuristic tier detector based on connection quality and history.
#[derive(Debug, Clone)]
pub struct TierDetector {
    /// Number of consecutive online ticks to upgrade to Cloud.
    pub upgrade_threshold: u64,
    /// Number of consecutive offline ticks to downgrade from Cloud.
    pub downgrade_threshold: u64,
    online_count: u64,
    offline_count: u64,
}

impl Default for TierDetector {
    fn default() -> Self {
        Self {
            upgrade_threshold: 10,
            downgrade_threshold: 3,
            online_count: 0,
            offline_count: 0,
        }
    }
}

impl TierDetector {
    /// Create a new detector.
    pub fn new(upgrade_threshold: u64, downgrade_threshold: u64) -> Self {
        Self {
            upgrade_threshold,
            downgrade_threshold,
            online_count: 0,
            offline_count: 0,
        }
    }

    /// Feed a connectivity sample and get the suggested tier.
    pub fn sample(&mut self, online: bool, current_tier: DeviceTier) -> DeviceTier {
        if online {
            self.online_count += 1;
            self.offline_count = 0;

            if self.online_count >= self.upgrade_threshold && current_tier == DeviceTier::Cortex {
                DeviceTier::Cloud
            } else {
                current_tier
            }
        } else {
            self.offline_count += 1;
            self.online_count = 0;

            match current_tier {
                DeviceTier::Cloud if self.offline_count >= self.downgrade_threshold => {
                    DeviceTier::Cortex
                }
                DeviceTier::Cortex if self.offline_count >= self.downgrade_threshold * 2 => {
                    DeviceTier::Limb
                }
                _ => current_tier,
            }
        }
    }

    /// Reset counters.
    pub fn reset(&mut self) {
        self.online_count = 0;
        self.offline_count = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_is_cortex() {
        assert_eq!(DeviceTier::default(), DeviceTier::Cortex);
    }

    #[test]
    fn test_tier_properties() {
        assert!(DeviceTier::Cloud.can_sync());
        assert!(DeviceTier::Cloud.is_realtime());
        assert_eq!(DeviceTier::Cloud.priority(), 3);

        assert!(DeviceTier::Cortex.can_sync());
        assert!(!DeviceTier::Cortex.is_realtime());
        assert_eq!(DeviceTier::Cortex.priority(), 2);

        assert!(!DeviceTier::Limb.can_sync());
        assert!(!DeviceTier::Limb.is_realtime());
        assert!(DeviceTier::Limb.is_limb());
        assert_eq!(DeviceTier::Limb.priority(), 1);
    }

    #[test]
    fn test_tier_detector_upgrade() {
        let mut detector = TierDetector::new(3, 2);
        // Start at Cortex
        let mut tier = DeviceTier::Cortex;

        tier = detector.sample(true, tier);
        assert_eq!(tier, DeviceTier::Cortex);
        tier = detector.sample(true, tier);
        assert_eq!(tier, DeviceTier::Cortex);
        tier = detector.sample(true, tier);
        assert_eq!(tier, DeviceTier::Cloud); // 3 consistent online ticks
    }

    #[test]
    fn test_tier_detector_downgrade() {
        let mut detector = TierDetector::new(3, 2);
        let mut tier = DeviceTier::Cloud;

        tier = detector.sample(false, tier);
        assert_eq!(tier, DeviceTier::Cloud);
        tier = detector.sample(false, tier);
        assert_eq!(tier, DeviceTier::Cortex); // 2 offline ticks → Cortex

        tier = detector.sample(false, tier);
        tier = detector.sample(false, tier);
        tier = detector.sample(false, tier);
        assert_eq!(tier, DeviceTier::Limb); // 4 total offline ticks → Limb
    }

    #[test]
    fn test_reset() {
        let mut detector = TierDetector::new(3, 2);
        let tier = detector.sample(false, DeviceTier::Cortex);
        assert_eq!(tier, DeviceTier::Cortex);
        detector.reset();
        let tier = detector.sample(true, tier);
        assert_eq!(tier, DeviceTier::Cortex); // counter reset
    }
}
