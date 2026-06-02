// SPDX-License-Identifier: Apache-2.0 OR MIT

/// The connectivity state of the application.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnlineState {
    /// Connected. Normal operation.
    Online,
    /// Network jitter — short drop absorbed.
    Deadband,
    /// Network down long enough that writes are rejected.
    ReadOnly,
    /// Fully offline — cached data and UI only.
    Offline,
}
