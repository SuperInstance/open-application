# Tauri Offline Guardian — Integration Guide

## 🏆 SuperInstance Enhancement: Offline Guardian

**Tauri apps that work offline. Not broken offline. USEFUL offline. Sync when you're back.**

---

## Overview

The **Offline Guardian** is a first-party Tauri plugin crate (`tauri-offline-guardian`) that gives any Tauri app a rich, configurable offline-first architecture. It was developed as a **SuperInstance enhancement** to the `tauri-apps/tauri` fork — bringing desktop apps the same kind of resilience mobile apps have enjoyed for years.

### Core Concepts

| Concept | Description |
|---------|-------------|
| **Deadband** | ±5s hysteresis filter that prevents connectivity thrashing. A brief blip doesn't flip the state. |
| **Device Tiers** | Cloud (full sync), Cortex (local-first with lazy sync), Limb (offline-only). |
| **Escalation** | Progressive degradation as offline duration grows. |
| **Mutation Queue** | FIFO queue of offline mutations, replayed when connectivity returns. |
| **Sync Handoff** | Replays queued mutations with conflict resolution on reconnect. |

### Escalation Timeline

```
Online ──► Offline ──► 5 min ──► Queue ──► 30 min ──► Read-Only ──► 2 hr ──► Full Offline
                                                                    │
                                                                    ▼
Return Online ◄──────────────── Sync Handoff ◄──────────────────────┘
```

- **0–5 min**: Normal. App assumes it's a brief blip.
- **5–30 min**: Queue mutations locally. Everything still works, writes are deferred.
- **30 min–2 hr**: Degrade to read-only. No new mutations allowed. Cache displayed.
- **>2 hr**: Full offline mode. App is 80% as capable (push-down principle).
- **On reconnect**: All queued mutations are replayed with conflict resolution. Tier upgrades.

---

## Quick Start

### 1. Add the dependency

In your `Cargo.toml`:

```toml
[dependencies]
tauri-offline-guardian = { path = "path/to/tauri-offline-guardian" }
```

### 2. Register the plugin

```rust
use tauri_offline_guardian::OfflineGuardian;

fn main() {
    tauri::Builder::default()
        .plugin(OfflineGuardian::default())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

### 3. Access the connectivity monitor

```rust
use std::sync::Arc;
use tauri_offline_guardian::ConnectivityMonitor;

#[tauri::command]
fn my_command<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Result<String, String> {
    let monitor = app.state::<Arc<ConnectivityMonitor>>();

    if monitor.is_online() {
        Ok("We're online!".into())
    } else {
        Ok(format!(
            "Offline for {:?}. Escalation: {}",
            monitor.escalation_state(),
            monitor.tier()
        ))
    }
}
```

---

## Configuration

You can customize the plugin with a `GuardianConfig`:

```rust
use tauri_offline_guardian::{GuardianConfig, OfflineGuardian, DeviceTier};

let config = GuardianConfig {
    deadband_secs: 3,                // ±3s hysteresis
    escalation_queue_secs: 10,       // 10s → queue mutations
    escalation_readonly_secs: 30,    // 30s → read-only
    escalation_offline_secs: 60,     // 60s → full offline
    tick_secs: 1,                    // check every second
    initial_tier: DeviceTier::Cortex,
};

tauri::Builder::default()
    .plugin(OfflineGuardian::with_config(config))
    .run(tauri::generate_context!());
```

Or via `tauri.conf.json`:

```json
{
  "plugins": {
    "offline-guardian": {
      "deadbandSecs": 5,
      "escalationQueueSecs": 300,
      "escalationReadonlySecs": 1800,
      "escalationOfflineSecs": 7200,
      "tickSecs": 1,
      "initialTier": "cortex"
    }
  }
}
```

---

## Frontend API

The plugin injects `window.__OFFLINE_GUARDIAN__` into the webview:

```javascript
// Connection status
const status = window.__OFFLINE_GUARDIAN__.status;
console.log(status.online, status.tier, status.escalation);

// Subscribe to events
const unsub = window.__OFFLINE_GUARDIAN__.on('connection-changed', (status) => {
    if (status.online) showSnackbar('Back online!');
});

// Queue a mutation when offline
await window.__OFFLINE_GUARDIAN__.queueMutation('create:note', {
    title: 'Hello', body: 'World'
});

// Force a reconnection check
await window.__OFFLINE_GUARDIAN__.forceReconnect();
```

### Events

| Event | Payload | Fires |
|-------|---------|-------|
| `offline-guardian:connection-changed` | `ConnectionStatus` | Online/offline transition |
| `offline-guardian:tier-changed` | `{ tier: string }` | Tier upgrade/downgrade |
| `offline-guardian:escalation-changed` | `{ state: string, online: bool }` | Escalation level change |
| `offline-guardian:mutation` | `{ action, id, kind }` | Mutation queued/conflicted |
| `offline-guardian:sync` | `{ replayed, conflicts, success, message }` | Sync handoff completed |

### IPC Commands

| Command | Returns | Description |
|---------|---------|-------------|
| `get_connection_status` | `ConnectionStatus` | Current connection state |
| `get_device_tier` | `DeviceTier` | Current tier |
| `get_escalation` | `string` | Escalation state label |
| `get_queued_mutations` | `number` | Queue length |
| `queue_mutation_command` | `string` (ID) | Queue a new mutation |
| `force_reconnect` | `ConnectionStatus` | Force re-evaluation |

---

## Example App

A complete example is available in `examples/offline-guardian/`. It demonstrates:

- Plugin registration with custom config
- Commands that respect escalation state
- Mutation queuing when offline
- Frontend event listeners and status display

Run it with:

```bash
cd examples/offline-guardian
npm install     # or pnpm install
cargo tauri dev
```

---

## Architecture

```
┌──────────────────────────────────────────────────────────────┐
│                        Webview (JS)                         │
│  window.__OFFLINE_GUARDIAN__                                │
│  ┌──────────────────────────────────────────────────────┐   │
│  │ Events: connection-changed, tier-changed, escalation │   │
│  │ IPC: get_connection_status, queue_mutation, etc.    │   │
│  └──────────────────────────────────────────────────────┘   │
└──────────────────────┬───────────────────────────────────────┘
                       │ Tauri IPC (invoke / events)
┌──────────────────────▼───────────────────────────────────────┐
│                    Rust Backend                              │
│  ┌──────────────────────────────────────────────────────┐   │
│  │ ConnectivityMonitor (shared state via Arc)           │   │
│  │  ├─ Deadband filter         ← ±5s hysteresis        │   │
│  │  ├─ Tier detector           ← Cloud / Cortex / Limb │   │
│  │  ├─ Escalation state machine ← progressive degrade  │   │
│  │  ├─ MutationQueue           ← FIFO offline buffer   │   │
│  │  └─ Handoff handler          ← sync + conflict res. │   │
│  └──────────────────────────────────────────────────────┘   │
│                                                              │
│  Background thread ticks every `tick_secs` seconds:         │
│  1. Probe raw connectivity                                   │
│  2. Apply deadband filter                                    │
│  3. Calculate escalation tier                                │
│  4. Handle sync handoff if reconnected                       │
│  5. Emit events to frontend                                  │
└──────────────────────────────────────────────────────────────┘
```

---

## Conflict Resolution

The handoff supports four strategies:

| Strategy | Behavior |
|----------|----------|
| `LastWriterWins` | Mutation with most recent timestamp wins (default) |
| `ServerWins` | Server/cloud state is authoritative |
| `LocalWins` | Local mutations always override server |
| `Manual` | All conflicts flagged for user intervention |

Configure via `Handoff`:

```rust
use tauri_offline_guardian::handoff::{Handoff, ConflictStrategy};

let handoff = Handoff::new(ConflictStrategy::ServerWins, 3);
```

---

## Push-Down Principle

When in **Limb** (full offline) tier, the app should:

✅ **Work offline** — local data is available, navigation works
✅ **Show cached content** — stale data is better than no data
✅ **Queue writes** — mutations are saved for later sync
✅ **Display offline indicators** — user knows their state
❌ **Fail with errors** — no "Cannot connect to server" modals
❌ **Block navigation** — every screen should be usable
❌ **Hide content** — local snapshots are better than empty screens

The goal is **80% capability offline** — the app is useful, not broken.

---

## License

Apache-2.0 OR MIT (same as Tauri)
