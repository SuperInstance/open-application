// Copyright 2026 SuperInstance
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! Deadband filter for connectivity signals.
//!
//! Prevents thrashing when the connection flickers on/off rapidly.
//! Implements ±hysteresis: once the state flips, it stays flipped for at least
//! `duration` before flipping back.

use std::time::{Duration, Instant};

/// A deadband filter with configurable hysteresis.
///
/// # Example
///
/// ```
/// use tauri_offline_guardian::Deadband;
/// use std::time::Duration;
///
/// let mut db = Deadband::new(Duration::from_secs(5));
///
/// // Initially assumes connected
/// assert!(db.filter(true));
///
/// // A brief blip is absorbed
/// assert!(db.filter(false));
/// assert!(db.filter(true)); // accepted before hysteresis window ends
/// assert_eq!(db.state(), true);
/// ```
#[derive(Debug, Clone)]
pub struct Deadband {
    hysteresis: Duration,
    current_state: bool,
    last_change: Instant,
}

impl Deadband {
    /// Create a new deadband with the given hysteresis duration.
    /// A duration of 0 disables filtering.
    pub fn new(hysteresis: Duration) -> Self {
        Self {
            hysteresis,
            current_state: true, // assume online initially
            last_change: Instant::now(),
        }
    }

    /// Apply the deadband filter to a raw signal.
    ///
    /// Returns the filtered state.
    pub fn filter(&mut self, raw: bool) -> bool {
        if raw == self.current_state {
            // Signal matches current state — no change needed.
            return self.current_state;
        }

        // Signal differs from current state.
        let elapsed = self.last_change.elapsed();

        if elapsed >= self.hysteresis {
            // Hysteresis window expired — accept the change.
            self.current_state = raw;
            self.last_change = Instant::now();
        }
        // else: within hysteresis window — absorb the blip.

        self.current_state
    }

    /// Get the current filtered state.
    pub fn state(&self) -> bool {
        self.current_state
    }

    /// Reset the deadband to the given state.
    pub fn reset_to(&mut self, state: bool) {
        self.current_state = state;
        self.last_change = Instant::now();
    }

    /// Reset the deadband to "online".
    pub fn reset(&mut self) {
        self.reset_to(true);
    }

    /// Change the hysteresis duration.
    pub fn set_hysteresis(&mut self, hysteresis: Duration) {
        self.hysteresis = hysteresis;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_state() {
        let db = Deadband::new(Duration::from_secs(5));
        assert!(db.state());
    }

    #[test]
    fn test_blip_absorbed() {
        let mut db = Deadband::new(Duration::from_secs(5));
        assert!(db.filter(false)); // within hysteresis → still true
        assert!(db.filter(true));
        assert!(db.state());
    }

    #[test]
    fn test_change_accepted_after_hysteresis() {
        let mut db = Deadband::new(Duration::from_millis(10));
        std::thread::sleep(Duration::from_millis(20));
        assert!(!db.filter(false)); // hysteresis expired → change accepted
        assert_eq!(db.state(), false);
    }

    #[test]
    fn test_rapid_thrash_stabilized() {
        let mut db = Deadband::new(Duration::from_millis(50));
        // Rapid flickering
        assert!(db.filter(false)); // absorbed
        assert!(db.filter(true));  // back to match
        assert!(db.filter(false)); // absorbed
        assert!(db.state());       // still online
    }

    #[test]
    fn test_reset() {
        let mut db = Deadband::new(Duration::from_secs(5));
        db.reset_to(false);
        assert!(!db.state());
        db.reset();
        assert!(db.state());
    }

    #[test]
    fn test_zero_hysteresis() {
        let mut db = Deadband::new(Duration::from_secs(0));
        // Zero hysteresis means signal passes through immediately
        assert!(!db.filter(false));
        assert!(db.filter(true));
        assert!(!db.filter(false));
    }
}
