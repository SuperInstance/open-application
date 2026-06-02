// Copyright 2026 SuperInstance
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! # Tauri Offline Guardian
//!
//! A connectivity-aware plugin for [Tauri](https://tauri.app) apps that
//! **work offline — not break offline — and sync when you're back**.
//!
//! ## Features
//!
//! - **Deadband filtering** — ±5s hysteresis prevents connectivity thrashing.
//! - **Device tiers** — Cloud (full sync), Cortex (local-first, lazy sync),
//!   Limb (offline-only).
//! - **Graceful escalation** — 5 min offline → queue mutations;
//!   30 min → degrade to read-only; 2 hr → full offline mode.
//! - **Sync handoff** — When connection returns, replay queued mutations
//!   with conflict resolution.
//! - **Push-down principle** — Offline mode retains ~80% capability.
//! - **Frontend API** — JavaScript bridge exposes connection status, tier,
//!   and mutation queue.
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use tauri_offline_guardian::OfflineGuardian;
//!
//! fn main() {
//!     tauri::Builder::default()
//!         .plugin(OfflineGuardian::default())
//!         .run(tauri::generate_context!())
//!         .expect("error while running tauri application");
//! }
//! ```

mod deadband;
mod escalation;
mod tier;
mod queue;
mod handoff;

use std::sync::Arc;
use std::time::Duration;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{
    plugin::{Builder, TauriPlugin},
    Emitter, Manager, Runtime,
};
use tokio::sync::watch;

pub use deadband::Deadband;
pub use escalation::Escalation;
pub use tier::{DeviceTier, TierDetector};
pub use queue::{Mutation, MutationQueue};
pub use handoff::{Handoff, HandoffResult, ConflictStrategy};

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for the Offline Guardian.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuardianConfig {
    /// Deadband hysteresis in seconds (default: 5).
    #[serde(default = "default_deadband_secs")]
    pub deadband_secs: u64,
    /// Time in seconds before escalating to mutation queuing (default: 300 / 5 min).
    #[serde(default = "default_escalation_queue_secs")]
    pub escalation_queue_secs: u64,
    /// Time in seconds before escalating to read-only (default: 1800 / 30 min).
    #[serde(default = "default_escalation_readonly_secs")]
    pub escalation_readonly_secs: u64,
    /// Time in seconds before escalating to full offline mode (default: 7200 / 2 hr).
    #[serde(default = "default_escalation_offline_secs")]
    pub escalation_offline_secs: u64,
    /// Tick interval in seconds for the guardian loop (default: 1).
    #[serde(default = "default_tick_secs")]
    pub tick_secs: u64,
    /// Initial device tier (default: Cortex).
    #[serde(default)]
    pub initial_tier: DeviceTier,
}

fn default_deadband_secs() -> u64 { 5 }
fn default_escalation_queue_secs() -> u64 { 300 }
fn default_escalation_readonly_secs() -> u64 { 1800 }
fn default_escalation_offline_secs() -> u64 { 7200 }
fn default_tick_secs() -> u64 { 1 }

impl Default for GuardianConfig {
    fn default() -> Self {
        Self {
            deadband_secs: default_deadband_secs(),
            escalation_queue_secs: default_escalation_queue_secs(),
            escalation_readonly_secs: default_escalation_readonly_secs(),
            escalation_offline_secs: default_escalation_offline_secs(),
            tick_secs: default_tick_secs(),
            initial_tier: DeviceTier::Cortex,
        }
    }
}

// ---------------------------------------------------------------------------
// Public types exposed to the frontend
// ---------------------------------------------------------------------------

/// Connection status sent to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionStatus {
    /// Whether the app is considered online (after deadband filtering).
    pub online: bool,
    /// Current device tier.
    pub tier: DeviceTier,
    /// Escalation state name.
    pub escalation: String,
    /// Number of queued mutations waiting for sync.
    pub queued_mutations: usize,
    /// Timestamp (ISO 8601) of the last connectivity change.
    pub last_change: String,
}

// ---------------------------------------------------------------------------
// Guardian events (emitted to the frontend)
// ---------------------------------------------------------------------------

/// Events the plugin emits to the webview.
pub mod events {
    use serde::{Deserialize, Serialize};

    /// Fired when online/offline status changes.
    pub const CONNECTION_CHANGED: &str = "offline-guardian:connection-changed";
    /// Fired when the device tier changes.
    pub const TIER_CHANGED: &str = "offline-guardian:tier-changed";
    /// Fired on escalation state transitions.
    pub const ESCALATION_CHANGED: &str = "offline-guardian:escalation-changed";
    /// Fired when mutations are queued or replayed.
    pub const MUTATION_EVENT: &str = "offline-guardian:mutation";
    /// Fired during sync handoff.
    pub const SYNC_HANDOFF: &str = "offline-guardian:sync";

    /// Sync handoff payload.
    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct SyncHandoffPayload {
        pub replayed: usize,
        pub conflicts: usize,
        pub success: bool,
        pub message: String,
    }

    /// Mutation event payload.
    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct MutationEventPayload {
        pub action: String, // "queued" | "replayed" | "conflicted"
        pub id: String,
        pub kind: String,
    }
}

// ---------------------------------------------------------------------------
// Plugin state
// ---------------------------------------------------------------------------

/// Internal plugin state managed by Tauri.
#[derive(Debug)]
pub struct GuardianState<R: Runtime> {
    /// The shared connectivity monitor.
    pub monitor: Arc<ConnectivityMonitor>,
    /// The app handle.
    pub app_handle: tauri::AppHandle<R>,
}

// ---------------------------------------------------------------------------
// Connectivity Monitor (heart of the guardian)
// ---------------------------------------------------------------------------

/// Monitors connectivity with deadband filtering and escalation.
///
/// This is the central orchestrator. It runs a background task that:
/// 1. Periodically checks connectivity.
/// 2. Applies deadband hysteresis (±`deadband_secs`).
/// 3. Drives escalation when offline persists.
/// 4. Manages the mutation queue.
/// 5. Handles sync handoff when connectivity returns.
#[derive(Debug)]
pub struct ConnectivityMonitor {
    /// Current connection state (after deadband).
    pub online: RwLock<bool>,
    /// Raw connectivity signal (before deadband).
    pub raw_connected: AtomicBool,
    /// Current device tier.
    pub tier: RwLock<DeviceTier>,
    /// Escalation state.
    pub escalation: RwLock<Escalation>,
    /// Mutation queue.
    pub queue: RwLock<MutationQueue>,
    /// Handoff handler.
    pub handoff: RwLock<Handoff>,
    /// The time (in ticks) since the last connectivity change.
    last_tick: parking_lot::Mutex<u64>,
    /// Deadband filter.
    deadband: RwLock<Deadband>,
    /// Timestamp of last connectivity change.
    last_change: RwLock<chrono::DateTime<chrono::Utc>>,
    /// Watch channel for status updates (for efficiency).
    status_tx: watch::Sender<bool>,
}

impl ConnectivityMonitor {
    /// Create a new monitor with default settings.
    pub fn new(config: &GuardianConfig) -> Self {
        let now = chrono::Utc::now();
        Self {
            online: RwLock::new(true),
            raw_connected: AtomicBool::new(true),
            tier: RwLock::new(config.initial_tier),
            escalation: RwLock::new(Escalation::Normal),
            queue: RwLock::new(MutationQueue::new()),
            handoff: RwLock::new(Handoff::default()),
            last_tick: parking_lot::Mutex::new(0),
            deadband: RwLock::new(Deadband::new(Duration::from_secs(config.deadband_secs))),
            last_change: RwLock::new(now),
            status_tx: watch::Sender::new(true),
        }
    }

    /// Return a receiver for status updates.
    pub fn subscribe(&self) -> watch::Receiver<bool> {
        self.status_tx.subscribe()
    }

    /// Check if we're online (after deadband).
    pub fn is_online(&self) -> bool {
        *self.online.read()
    }

    /// Get the current device tier.
    pub fn tier(&self) -> DeviceTier {
        *self.tier.read()
    }

    /// Get the escalation state.
    pub fn escalation_state(&self) -> Escalation {
        *self.escalation.read()
    }

    /// Get the queued mutation count.
    pub fn queued_mutations(&self) -> usize {
        self.queue.read().len()
    }

    /// Get connection status payload for frontend.
    pub fn connection_status(&self) -> ConnectionStatus {
        ConnectionStatus {
            online: self.is_online(),
            tier: self.tier(),
            escalation: self.escalation_state().to_string(),
            queued_mutations: self.queued_mutations(),
            last_change: self.last_change.read().to_rfc3339(),
        }
    }

    /// Tick the monitor: check connectivity, apply deadband, escalate,
    /// and handle handoff.
    pub fn tick<R: Runtime>(&self, config: &GuardianConfig, app: &tauri::AppHandle<R>) {
        let mut tick = self.last_tick.lock();
        *tick += 1;

        // --- Phase 1: Check raw connectivity ---
        let raw = self.detect_connectivity();

        // --- Phase 2: Apply deadband ---
        let is_online = self.deadband.write().filter(raw);

        // Handle transition
        let prev_online = *self.online.read();
        if is_online != prev_online {
            *self.online.write() = is_online;
            *self.last_change.write() = chrono::Utc::now();
            *tick = 0;
            self.status_tx.send_replace(is_online);

            let status = self.connection_status();
            let _ = app.emit(events::CONNECTION_CHANGED, &status);
        }

        // --- Phase 3: Escalation ---
        let elapsed = Duration::from_secs(*tick * config.tick_secs);

        let prev_escalation = *self.escalation.read();
        let new_escalation = if is_online {
            Escalation::Normal
        } else {
            self.determine_escalation(elapsed, config)
        };

        if new_escalation != prev_escalation {
            // Tier demotion based on escalation
            if new_escalation == Escalation::FullOffline {
                *self.tier.write() = DeviceTier::Limb;
            } else if prev_escalation == Escalation::Normal && !is_online {
                *self.tier.write() = DeviceTier::Cortex;
            }

            *self.escalation.write() = new_escalation;
            let _ = app.emit(
                events::ESCALATION_CHANGED,
                serde_json::json!({
                    "state": new_escalation.to_string(),
                    "online": is_online,
                }),
            );
        }

        // --- Phase 4: Handoff if we just came back online ---
        if is_online && self.queue.read().len() > 0 {
            self.perform_handoff(app);
        }
    }

    /// Probe connectivity. Override this in tests or provide a custom checker.
    /// Uses platform APIs by default (HTTP HEAD to a known endpoint).
    fn detect_connectivity(&self) -> bool {
        // Check raw connectivity by attempting a lightweight TCP connection
        // or HTTP HEAD. This is a best-effort check; the deadband handles jitter.
        match std::net::TcpStream::connect_timeout(
            &"1.1.1.1:80".parse().unwrap(),
            Duration::from_secs(2),
        ) {
            Ok(_) => true,
            Err(_) => {
                // Fallback: try Google DNS
                match std::net::TcpStream::connect_timeout(
                    &"8.8.8.8:53".parse().unwrap(),
                    Duration::from_secs(2),
                ) {
                    Ok(_) => true,
                    Err(_) => false,
                }
            }
        }
    }

    fn determine_escalation(&self, elapsed: Duration, config: &GuardianConfig) -> Escalation {
        let queue_threshold = Duration::from_secs(config.escalation_queue_secs);
        let readonly_threshold = Duration::from_secs(config.escalation_readonly_secs);
        let offline_threshold = Duration::from_secs(config.escalation_offline_secs);

        if elapsed >= offline_threshold {
            Escalation::FullOffline
        } else if elapsed >= readonly_threshold {
            Escalation::ReadOnly
        } else if elapsed >= queue_threshold {
            Escalation::QueueMutations
        } else {
            Escalation::Normal
        }
    }

    fn perform_handoff<R: Runtime>(&self, app: &tauri::AppHandle<R>) {
        let result = self.handoff.read().sync(&self.queue, &self.tier);
        let payload = events::SyncHandoffPayload {
            replayed: result.replayed,
            conflicts: result.conflicts,
            success: result.conflicts == 0,
            message: if result.conflicts > 0 {
                format!("{} conflict(s) detected, resolved using last-writer-wins", result.conflicts)
            } else {
                format!("Successfully replayed {} mutation(s)", result.replayed)
            },
        };

        if *self.tier.read() == DeviceTier::Limb {
            // Tier was Limb due to full offline; restore to Cortex
            *self.tier.write() = DeviceTier::Cortex;
        }

        let _ = app.emit(events::SYNC_HANDOFF, &payload);

        if self.queue.read().len() == 0 {
            *self.escalation.write() = Escalation::Normal;
        }
    }

    /// Queue a mutation. Returns the mutation ID.
    pub fn queue_mutation<R: Runtime>(
        &self,
        kind: String,
        payload: serde_json::Value,
        app: &tauri::AppHandle<R>,
    ) -> String {
        let mut queue = self.queue.write();
        let kind_clone = kind.clone();
        let mutation = Mutation::new(kind, payload);
        let id = mutation.id.clone();
        queue.push(mutation);
        let _ = app.emit(
            events::MUTATION_EVENT,
            events::MutationEventPayload {
                action: "queued".into(),
                id: id.clone(),
                kind: kind_clone,
            },
        );
        id
    }
}

// ---------------------------------------------------------------------------
// Plugin entry point
// ---------------------------------------------------------------------------

/// Create the Offline Guardian Tauri plugin.
///
/// # Examples
///
/// ```rust,ignore
/// use tauri_offline_guardian::OfflineGuardian;
///
/// fn main() {
///     tauri::Builder::default()
///         .plugin(OfflineGuardian::default())
///         .run(tauri::generate_context!())
///         .expect("error while running tauri application");
/// }
/// ```
pub struct OfflineGuardian;

impl OfflineGuardian {
    /// Build the plugin with default configuration.
    pub fn default<R: Runtime>() -> TauriPlugin<R> {
        Self::with_config(GuardianConfig::default())
    }

    /// Build the plugin with a custom configuration.
    pub fn with_config<R: Runtime>(config: GuardianConfig) -> TauriPlugin<R> {
        let config = Arc::new(config);

        Builder::new("offline-guardian")
            .setup(move |app, _api| {
                let monitor = Arc::new(ConnectivityMonitor::new(&config));
                app.manage(monitor.clone());

                // Spawn guardian background loop
                let app_handle = app.clone();
                let cfg = config.clone();
                std::thread::spawn(move || {
                    let rt = tokio::runtime::Runtime::new().unwrap();
                    rt.block_on(async move {
                        let mut interval = tokio::time::interval(Duration::from_secs(cfg.tick_secs));
                        loop {
                            interval.tick().await;
                            monitor.tick(&cfg, &app_handle);
                        }
                    });
                });

                Ok(())
            })
            .invoke_handler(tauri::generate_handler![
                get_connection_status,
                get_device_tier,
                get_escalation,
                get_queued_mutations,
                queue_mutation_command,
                force_reconnect,
            ])
            .build()
    }
}

// ---------------------------------------------------------------------------
// Tauri IPC Commands
// ---------------------------------------------------------------------------

/// Returns the current connection status.
#[tauri::command]
fn get_connection_status<R: Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<ConnectionStatus, String> {
    let state = app.state::<Arc<ConnectivityMonitor>>();
    Ok(state.connection_status())
}

/// Returns the current device tier.
#[tauri::command]
fn get_device_tier<R: Runtime>(app: tauri::AppHandle<R>) -> Result<DeviceTier, String> {
    let state = app.state::<Arc<ConnectivityMonitor>>();
    Ok(state.tier())
}

/// Returns the current escalation state.
#[tauri::command]
fn get_escalation<R: Runtime>(app: tauri::AppHandle<R>) -> Result<String, String> {
    let state = app.state::<Arc<ConnectivityMonitor>>();
    Ok(state.escalation_state().to_string())
}

/// Returns the count of queued mutations.
#[tauri::command]
fn get_queued_mutations<R: Runtime>(app: tauri::AppHandle<R>) -> Result<usize, String> {
    let state = app.state::<Arc<ConnectivityMonitor>>();
    Ok(state.queued_mutations())
}

/// Queue a mutation (from frontend).
#[tauri::command]
fn queue_mutation_command<R: Runtime>(
    app: tauri::AppHandle<R>,
    kind: String,
    payload: serde_json::Value,
) -> Result<String, String> {
    let state = app.state::<Arc<ConnectivityMonitor>>();
    Ok(state.queue_mutation(kind, payload, &app))
}

/// Force a reconnect check.
#[tauri::command]
fn force_reconnect<R: Runtime>(app: tauri::AppHandle<R>) -> Result<ConnectionStatus, String> {
    let state = app.state::<Arc<ConnectivityMonitor>>();
    // Force raw to true momentarily to trigger re-evaluation
    state.raw_connected.store(true, Ordering::SeqCst);
    state.deadband.write().reset();
    Ok(state.connection_status())
}

// ---------------------------------------------------------------------------
// Events JavaScript initialization script
// ---------------------------------------------------------------------------

/// Default initialization script injected into the webview.
/// Exposes the `window.__OFFLINE_GUARDIAN__` object with connection
/// state and helper methods.
pub const INIT_SCRIPT: &str = r#"
(function () {
  if (window.__OFFLINE_GUARDIAN__) return;

  const listeners = new Map();
  let currentStatus = { online: true, tier: 'Cortex', escalation: 'Normal', queuedMutations: 0 };

  window.__OFFLINE_GUARDIAN__ = {
    get status() { return currentStatus; },

    on(event, callback) {
      if (!listeners.has(event)) listeners.set(event, new Set());
      listeners.get(event).add(callback);
      return () => listeners.get(event).delete(callback);
    },

    async refresh() {
      try {
        const { invoke } = window.__TAURI__;
        currentStatus = await invoke('plugin:offline-guardian|get_connection_status');
        return currentStatus;
      } catch { return currentStatus; }
    },

    async queueMutation(kind, payload) {
      try {
        const { invoke } = window.__TAURI__;
        return await invoke('plugin:offline-guardian|queue_mutation_command', { kind, payload });
      } catch { return null; }
    },

    async forceReconnect() {
      try {
        const { invoke } = window.__TAURI__;
        currentStatus = await invoke('plugin:offline-guardian|force_reconnect');
        return currentStatus;
      } catch { return currentStatus; }
    }
  };

  // Listen for Tauri events and update local cache
  document.addEventListener('DOMContentLoaded', async () => {
    try {
      const { listen } = window.__TAURI__;
      await listen('offline-guardian:connection-changed', (e) => {
        currentStatus = e.payload;
        (listeners.get('connection-changed') || []).forEach(cb => cb(e.payload));
      });
      await listen('offline-guardian:tier-changed', (e) => {
        currentStatus.tier = e.payload.tier;
        (listeners.get('tier-changed') || []).forEach(cb => cb(e.payload));
      });
      await listen('offline-guardian:escalation-changed', (e) => {
        currentStatus.escalation = e.payload.state;
        (listeners.get('escalation-changed') || []).forEach(cb => cb(e.payload));
      });
      await listen('offline-guardian:mutation', (e) => {
        (listeners.get('mutation') || []).forEach(cb => cb(e.payload));
      });
      await listen('offline-guardian:sync', (e) => {
        (listeners.get('sync') || []).forEach(cb => cb(e.payload));
      });
      // Initial sync
      await window.__OFFLINE_GUARDIAN__.refresh();
    } catch {}
  });
})();
"#;
