# Future Integration: open-application (Tauri Fork)

## Current State
A fork of the Tauri framework for building tiny, fast desktop applications with Rust backends and web frontends. Tauri uses WRY for webview rendering and tao for window management across all platforms.

> **Note:** This is a fork of the Tauri open-source project. We preserve upstream code and add SuperInstance-specific features.

## Integration Opportunities

### With ternary engine desktop app
Tauri provides the perfect desktop shell for the ternary engine. A Tauri app bundles: (1) Rust backend running ternary-cell for local simulation, (2) Web frontend using Spreadsheet-moment's Univer UI for visualization, (3) Native OS integration for notifications, file access, and system tray. The ternary engine runs locally on the desktop.

### With BrowserRoom desktop client
Tauri wraps BrowserRoom in a native desktop application. Instead of running in a browser tab, the room runs in a Tauri window with native menus, file dialogs, and system integration. The Rust backend handles ternary computation; the web frontend handles visualization.

### With construct-core
Tauri's Rust backend naturally integrates with construct-core. The app IS a construct: it loads skills (room configurations), queries state (cell grids), and manages tools (LLM proxy connections). Tauri provides the GUI; construct-core provides the agent runtime.

## Our Integration (Not Upstream Changes)
We do NOT modify Tauri's core framework. Our integration is:
- SuperInstance Tauri app template with ternary engine bundled
- Custom plugins for room management (window per room)
- Native notification integration for room events
- System tray for room status monitoring

## Potential in Mature Systems
The Tauri app becomes the fleet's desktop client. Each room opens in its own Tauri window. Room events trigger native notifications. The system tray shows room status. The ternary engine runs in the Rust backend at full speed. Native, fast, beautiful.

## Cross-Pollination Ideas
- **lever-runner-wasm**: WASM pipeline runs in Tauri's webview for room control
- **Spreadsheet-moment**: Univer UI in Tauri's webview for room visualization
- **open-terminal**: Terminal view embedded in Tauri app for room debugging

## Dependencies for Next Steps
- SuperInstance Tauri app template
- Room window management (one Tauri window per room)
- Native notification bridge from ternary-cell events
