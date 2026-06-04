# Integration Guide: Ternary Intelligence in Open Application (Tauri)

> How the TernaryEngine, grid system, and conservation metrics plug into Tauri's polyglot application architecture.

## Overview

Open Application (fork of Tauri) integrates ternary intelligence as a **desktop application example** — the `ternary-app` demonstrates how ternary cells, grids, and conservation laws work as a native desktop app via Tauri's Rust backend + webview frontend.

## Ternary Crates & Modules

| Module | File | Role |
|--------|------|------|
| `TernaryEngine` | `examples/ternary-app/src-tauri/src/ternary_engine.rs` | Core ternary logic: cell values, grid operations, conservation metrics |
| `TernaryGrid` | `examples/ternary-app/src-tauri/src/ternary_engine.rs` | N×M grid of ternary cells with row/column/region queries |
| `ConservationMetrics` | `examples/ternary-app/src-tauri/src/ternary_engine.rs` | Measures whether ternary ratios hold across grid scales |

## Integration Points

### 1. TernaryEngine → Tauri Commands

The engine exposes ternary grid operations as Tauri commands, callable from the webview frontend:

```rust
// In ternary-app/src-tauri/src/main.rs
use tauri::State;
use std::sync::Mutex;

struct AppState {
    engine: Mutex<TernaryEngine>,
}

#[tauri::command]
fn get_cell(state: State<AppState>, row: usize, col: usize) -> CellValue {
    state.engine.lock().unwrap().grid().get(row, col)
        .unwrap_or(CellValue::Unknown)
}

#[tauri::command]
fn set_cell(state: State<AppState>, row: usize, col: usize, value: CellValue) {
    state.engine.lock().unwrap().grid_mut().set(row, col, value);
}

#[tauri::command]
fn conservation_report(state: State<AppState>) -> ConservationMetrics {
    state.engine.lock().unwrap().check_conservation()
}
```

**Where it connects:** Tauri's `tauri::command` macro exposes Rust functions to the webview via message passing. The frontend calls these via `@tauri-apps/api`.

### 2. TernaryGrid → Frontend Rendering

The grid renders in the webview using ternary cell symbols:

```rust
// CellValue symbols for ASCII/Unicode rendering
impl CellValue {
    pub fn symbol(&self) -> char {
        match self {
            CellValue::Avoid   => '░',  // empty — don't fill
            CellValue::Unknown => '▒',  // unexplored
            CellValue::Choose  => '█',  // chosen/filled
        }
    }
}
```

**Where it connects:** The frontend JavaScript fetches the grid state via Tauri commands and renders cells using CSS or canvas. Each cell's symbol drives visual representation.

### 3. Conservation Metrics → JSON Export

Conservation metrics serialize to JSON for export and visualization:

```rust
// Export metrics for frontend charts or external analysis
let metrics = engine.check_conservation();
let json = serde_json::to_string_pretty(&metrics)?;

// Metrics include:
// - row/column/region avoidance ratios
// - cross-scale conservation check (std < threshold)
// - overall grid health score
```

**Where it connects:** The JSON export feeds into the Tauri webview for chart rendering, or into external analysis tools.

## Architecture

```
┌─────────────────────────────────────────────────────┐
│  Webview (HTML/JS)                                  │
│  ┌────────────┐  ┌─────────────┐  ┌──────────────┐ │
│  │ Grid View  │  │ Metrics UI  │  │ Export Panel │ │
│  └─────┬──────┘  └──────┬──────┘  └──────┬───────┘ │
│        │ @tauri-apps/api │                │          │
│  ┌─────▼─────────────────▼────────────────▼───────┐ │
│  │ Tauri Runtime (tauri-runtime-wry)               │ │
│  │  message passing: invoke commands               │ │
│  └─────────────────────┬──────────────────────────┘ │
│                        │                             │
│  ┌─────────────────────▼──────────────────────────┐ │
│  │ Rust Backend (tauri crate)                     │ │
│  │  ┌──────────────────────────────────────────┐  │ │
│  │  │ TernaryEngine                            │  │ │
│  │  │  ├─ TernaryGrid (N×M cells)             │  │ │
│  │  │  ├─ ConservationMetrics (scale checks)  │  │ │
│  │  │  └─ JSON export (serde)                 │  │ │
│  │  └──────────────────────────────────────────┘  │ │
│  └────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────┘
```

## Committed Files

- `e3cc75b` — `examples/ternary-app/src-tauri/src/ternary_engine.rs` — full TernaryEngine implementation with grid, conservation metrics, ASCII rendering, and JSON export

## Extending the Integration

1. **New Tauri commands:** Add `#[tauri::command]` functions that wrap `TernaryEngine` methods
2. **Frontend state:** Use Tauri events (`app.emit()`) to push grid updates reactively
3. **Persistence:** Serialize the grid to disk via `tauri::api::path::app_data_dir()`
4. **WASM target:** The same `TernaryEngine` can compile to WASM for browser-only deployment
