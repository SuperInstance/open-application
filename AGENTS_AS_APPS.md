# Agents as Applications: open-application

## The Agent IS the Application

In traditional app frameworks, code generates UI elements in response to user actions. In open-application, **the agent itself is the application**. It doesn't generate UI code — it directly controls what the UI shows through its state machine, driven by the agent runtime inside Tauri.

### How It Works

1. **Rust Crates → WASM → Capabilities**: Our Rust math crates (spectral, fleet, conservation) compile to WASM. The `capability_loader` reads `CAPABILITY.toml` files from each crate and generates Tauri commands that expose the crate's functionality to the frontend.

2. **Agent Runtime Drives the UI**: The `agent_runtime` runs inside the Tauri app as a state machine. Each `tick()` produces an `AgentAction` — either updating the UI, processing a task, requesting budget, or providing spectral feedback. The agent decides what to show, not the user.

3. **Conservation Budget**: The agent operates under a conservation budget. Each UI update or computation consumes budget; completed work releases it. The agent can't overcommit — it must balance activity against its energy allocation.

4. **Spectral Feedback Loop**: After each tick, the agent computes spectral feedback — an eigenvalue analysis of its task distribution. This feedback drives adaptive behavior: when the spectral concentration is high, the agent focuses on the bottleneck; when it's low, it works in parallel.

### Architecture

```
┌──────────────────────────────────────────┐
│  Tauri Application                       │
│                                          │
│  ┌────────────────┐  ┌───────────────┐   │
│  │ CapabilityLoader│  │ AgentRuntime  │   │
│  │                 │  │               │   │
│  │ CAPABILITY.toml │  │ tick() →      │   │
│  │       ↓         │  │  AgentAction  │   │
│  │ TauriCommand    │  │               │   │
│  └────────────────┘  │ SpectralFeed  │   │
│                      │ ConservationBudget│
│                      └───────┬───────┘   │
│                              │            │
│                     ┌────────▼────────┐   │
│                     │   UI / WebView   │   │
│                     └─────────────────┘   │
└──────────────────────────────────────────┘
```

### CAPABILITY.toml Format

Each Rust crate that wants to expose UI features provides a `CAPABILITY.toml`:

```toml
[capability]
name = "spectral_search"
description = "Search using spectral eigenvalue similarity"
version = "0.1.0"

[capability.inputs]
query = { type = "String", description = "Query vector" }
top_k = { type = "usize", description = "Result count", default = "10" }

[capability.outputs]
results = { type = "Vec<SearchResult>", description = "Ranked results" }
```

The `capability_loader` parses this and generates a `#[tauri::command]` function that the frontend can call.

### The Agent Loop

```rust
let mut runtime = AgentRuntime::new(budget);

loop {
    let action = runtime.tick();

    match action {
        AgentAction::ProcessTask { task_id, name } => {
            // Execute the task, update UI
        }
        AgentAction::UpdateUI { component, payload } => {
            // Push state to the webview
        }
        AgentAction::Idle => {
            // Wait for new tasks
        }
        AgentAction::RequestBudget { requested } => {
            // Negotiate with the system for more energy
        }
        AgentAction::SpectralFeedback { eigenvalue, bottleneck } => {
            // Adapt behavior based on spectral analysis
        }
    }
}
```

### Files

- `crates/tauri-runtime/src/capability_loader.rs` — Loads CAPABILITY.toml, generates Tauri commands
- `crates/tauri-runtime/src/agent_runtime.rs` — Agent state machine with conservation budget and spectral feedback
