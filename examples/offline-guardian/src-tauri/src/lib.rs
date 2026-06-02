// Copyright 2026 SuperInstance
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! Offline Guardian Example
//!
//! Demonstrates how to integrate the Offline Guardian plugin into a Tauri app.
//!
//! This example shows:
//! - Plugin initialization with custom configuration
//! - Custom commands that respect the escalation state
//! - How to use `can_write()` to block mutations during read-only/offline
//! - How to queue mutations when offline

use std::sync::Arc;

use serde::Serialize;
use tauri::Runtime;
use tauri_offline_guardian::{
    ConnectivityMonitor, DeviceTier, Escalation, GuardianConfig, OfflineGuardian,
};

// ---------------------------------------------------------------------------
// Custom state: a toy data store
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Serialize)]
struct NotesStore {
    notes: Vec<Note>,
}

#[derive(Debug, Clone, Serialize)]
struct Note {
    id: u64,
    title: String,
    body: String,
}

// ---------------------------------------------------------------------------
/// Build the app with the offline guardian plugin.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Custom config with shorter escalation times for demo purposes
    let config = GuardianConfig {
        deadband_secs: 3,
        escalation_queue_secs: 10,   // 10 seconds for demo
        escalation_readonly_secs: 30, // 30 seconds
        escalation_offline_secs: 60,  // 60 seconds
        tick_secs: 1,
        initial_tier: DeviceTier::Cortex,
    };

    tauri::Builder::default()
        .plugin(OfflineGuardian::with_config(config))
        .manage(NotesStore::default())
        .invoke_handler(tauri::generate_handler![
            create_note,
            list_notes,
            get_offline_status,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

/// Create a note. Respects the escalation state: if read-only or full offline,
/// the mutation is queued instead.
#[tauri::command]
fn create_note<R: Runtime>(
    app: tauri::AppHandle<R>,
    title: String,
    body: String,
) -> Result<String, String> {
    let monitor = app.state::<Arc<ConnectivityMonitor>>();
    let escalation = monitor.escalation_state();

    if escalation.can_write() {
        // We can write directly (online, or at least not read-only)
        let mut store = app.state::<NotesStore>();
        let id = store.notes.len() as u64 + 1;
        store.notes.push(Note { id, title, body });
        Ok(format!("Note {} created", id))
    } else {
        // Queue the mutation for later replay
        let id = monitor.queue_mutation(
            "create:note".into(),
            serde_json::json!({ "title": title, "body": body }),
            &app,
        );
        Ok(format!("Note queued for creation (mutation {})", id))
    }
}

/// List notes.
#[tauri::command]
fn list_notes<R: Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Vec<Note>, String> {
    let store = app.state::<NotesStore>();
    Ok(store.notes.clone())
}

/// Get the current offline status for the frontend.
#[tauri::command]
fn get_offline_status<R: Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<serde_json::Value, String> {
    let monitor = app.state::<Arc<ConnectivityMonitor>>();
    let status = monitor.connection_status();

    Ok(serde_json::json!({
        "online": status.online,
        "tier": status.tier,
        "escalation": status.escalation,
        "queuedMutations": status.queued_mutations,
        "lastChange": status.last_change,
        "canWrite": monitor.escalation_state().can_write(),
    }))
}
