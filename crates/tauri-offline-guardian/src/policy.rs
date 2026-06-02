// SPDX-License-Identifier: Apache-2.0 OR MIT

/// Conflict resolution strategy when reconnecting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OfflinePolicy {
    /// Last writer wins (timestamp-based).
    LastWriterWins,
    /// First writer wins (oldest mutation kept).
    FirstWriterWins,
}
