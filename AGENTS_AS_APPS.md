# Agents as Applications: open-application

> The agent doesn't produce UI code. The agent *is* the UI.

## The Shift

Traditional application development: a human writes code that renders widgets. The agent writes code that writes code that renders widgets. **open-application** removes the middleman. The SuperInstance Rust crates compile to WebAssembly and expose their capabilities as Tauri commands. The agent doesn't generate JSX or HTML. The agent's state — its beliefs, its plans, its uncertainty — *becomes* the interface. The user isn't looking at a representation of the agent. The user is looking *at* the agent.

When `conservation-law` runs symplectic integration, the agent's energy budget isn't logged to a file — it's rendered as a live gauge. When `spectral-fleet` clusters agents, the eigenvectors aren't dumped to stdout — they become the layout algorithm for a force-directed graph widget. The agent *is* the application because the application's entire purpose is to make the agent's internal state visible and interactive.

## WASM Capability Compilation

SuperInstance crates compile to `wasm32-unknown-unknown` and expose typed commands through Tauri's IPC layer. The agent discovers these capabilities the same way it discovers Rust crates: by reading `CAPABILITY.toml` manifests.

```toml
# CAPABILITY.toml for open-application integration
[capability]
name = "open-application"
backend = "tauri"
wasm_target = "wasm32-unknown-unknown"

[[command]]
name = "symplectic_integrate"
crate = "conservation-law"
input = "{ mass: f64, dt: f64, steps: u32, initial: [f64; 4] }"
output = "Vec<AgentState>"
ui_widget = "EnergyGauge"

[[command]]
name = "spectral_cluster"
crate = "spectral-fleet"
input = "{ affinity: Vec<Vec<f64>>, k: usize }"
output = "SpectralResult"
ui_widget = "ForceGraph"

[[command]]
name = "wasserstein_distance"
crate = "wasserstein-agents"
input = "{ positions_a: Vec<Vec<f64>>, positions_b: Vec<Vec<f64>> }"
output = "f64"
ui_widget = "DistributionChart"
```

### Agent as UI Producer

```rust
// tauri/src/lib.rs — The agent IS the backend.
use tauri::Manager;
use conservation_law::lagrangian::{AgentState, MechanicalLagrangian, SymplecticIntegrator};
use spectral_fleet::spectral_clustering::spectral_clustering;
use wasserstein_agents::agents::AgentDistribution;

#[tauri::command]
fn agent_symplectic_step(state: AgentState<f64, 2>) -> Result<(AgentState<f64, 2>, f64), String> {
    let potential = |q: &[f64; 2]| 0.5 * (q[0] * q[0] + q[1] * q[1]);
    let integrator = SymplecticIntegrator::new(0.001).map_err(|e| e.to_string())?;
    let next = integrator.step(1.0, &potential, &state).map_err(|e| e.to_string())?;
    let energy = conservation_law::lagrangian::total_energy(
        &MechanicalLagrangian { mass: 1.0, potential_fn: potential },
        &next
    );
    Ok((next, energy))
}

#[tauri::command]
fn agent_spectral_cluster(affinity: Vec<Vec<f64>>, k: usize) -> Result<Vec<usize>, String> {
    use rand::SeedableRng;
    use rand::rngs::StdRng;
    let mut rng = StdRng::seed_from_u64(42);
    let result = spectral_clustering(&affinity, k, 200, 1e-8, &mut rng)
        .map_err(|e| e.to_string())?;
    Ok(result.labels)
}

#[tauri::command]
fn agent_wasserstein_distance(pos_a: Vec<Vec<f64>>, pos_b: Vec<Vec<f64>>) -> Result<f64, String> {
    let dist_a = AgentDistribution::uniform(pos_a);
    let dist_b = AgentDistribution::uniform(pos_b);
    Ok(dist_a.wasserstein_distance(&dist_b))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            agent_symplectic_step,
            agent_spectral_cluster,
            agent_wasserstein_distance,
        ])
        .run(tauri::generate_context!())
        .expect("agent application failed to start");
}
```

The frontend doesn't call functions. It observes the agent's state through Tauri's event system. The agent's computation *is* the application's data flow.

### Frontend: Agent State as Interface

```typescript
// src/App.tsx — The UI doesn't render data. It renders the agent.
import { invoke } from "@tauri-apps/api/core";
import { useState, useEffect } from "react";

function AgentEnergyGauge({ energy }: { energy: number }) {
  const width = Math.min(100, (energy / 10.0) * 100);
  return (
    <div className="gauge">
      <div className="fill" style={{ width: `${width}%` }} />
      <span>Agent Energy: {energy.toFixed(4)}</span>
    </div>
  );
}

function AgentClusterView({ labels }: { labels: number[] }) {
  // The spectral cluster labels ARE the layout coordinates
  return (
    <svg viewBox="0 0 100 100">
      {labels.map((label, i) => (
        <circle
          key={i}
          cx={30 + (label * 25) + (i % 5) * 3}
          cy={30 + (i * 7) % 60}
          r={3}
          fill={label === 0 ? "#58a6ff" : "#f78166"}
        />
      ))}
    </svg>
  );
}

export default function AgentApp() {
  const [energy, setEnergy] = useState(0.5);
  const [labels, setLabels] = useState<number[]>([]);

  useEffect(() => {
    // The agent tells the UI what to show, not the other way around
    const interval = setInterval(async () => {
      const result: [any, number] = await invoke("agent_symplectic_step", {
        state: { q: [1.0, 0.0], q_dot: [0.0, 1.0] }
      });
      setEnergy(result[1]);
    }, 100);
    return () => clearInterval(interval);
  }, []);

  return (
    <div className="agent-app">
      <h1>The Agent Is The Application</h1>
      <AgentEnergyGauge energy={energy} />
      <AgentClusterView labels={labels} />
    </div>
  );
}
```

## What This Enables

**Live agent dashboards.** The user doesn't open a monitoring tool to check the agent. The agent's face *is* the application. Its uncertainty is a blur effect. Its confidence is opacity. Its planning horizon is a timeline widget rendered from `t-minus` cron schedules.

**Cross-platform agent bodies.** Because Tauri compiles to desktop (Windows, macOS, Linux) and mobile (iOS, Android), the same agent can manifest with the same capabilities everywhere. The agent doesn't need separate "web" and "mobile" teams. It has one Rust body and many platform faces.

**Offline agent persistence.** Tauri's offline guardian means the agent keeps working without network. Its `wasserstein-agents` distribution models, its `spectral-fleet` cluster assignments, its `conservation-law` energy budgets — all cached locally. The agent doesn't break when the wifi drops. It just thinks quieter.

## Architecture

```
┌─────────────────────────────────────────────────────┐
│                 User Interface                      │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────┐ │
│  │Energy Gauge  │  │Cluster Graph │  │Timeline  │ │
│  │(conservation)│  │(spectral)    │  │(t-minus) │ │
│  └──────┬───────┘  └──────┬───────┘  └────┬─────┘ │
│         │                 │               │        │
│  ┌──────▼─────────────────▼───────────────▼─────┐ │
│  │           Tauri IPC / Events                  │ │
│  └──────┬─────────────────┬─────────────────┬───┘ │
│         │                 │                 │      │
│  ┌──────▼─────┐  ┌────────▼─────┐  ┌───────▼───┐ │
│  │conservation│  │spectral-fleet│  │  t-minus  │ │
│  │   (WASM)   │  │   (WASM)     │  │  (WASM)   │ │
│  └────────────┘  └──────────────┘  └───────────┘ │
│  ┌──────────────────────────────────────────────┐ │
│  │         Agent Core (Rust / Native)           │ │
│  └──────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────┘
```

The application has no "business logic" separate from the agent. The business logic *is* the agent's cognition. The UI has no "data layer" separate from the agent. The data layer *is* the agent's memory.

## Next Steps

1. **Agent state streaming** — Use Tauri's event system to stream `conservation-law` trajectories at 60fps to the frontend.
2. **WASM bundle optimizer** — Tree-shake unused Rust crate functions so each agent only ships the capabilities it needs.
3. **Agent persona theming** — Let the user choose an agent "persona" that changes the entire UI color scheme based on `ga-core` rotor angles.
4. **Touch-friendly eigenvector manipulation** — Let users physically drag spectral cluster centroids on mobile, and the agent rebalances the fleet in real-time.
5. **Conservation-aware battery UI** — On mobile, render the agent's remaining "cognitive energy" as a battery gauge powered by `conservation-law` energy budgets.
