# App Size Guardian v0.2.0

> Track binary size across builds. Enforce size budgets. Detect bloat before it ships.

Part of [SuperInstance/tauri](https://github.com/SuperInstance/tauri).

## What's New in v0.2.0

- 🔬 **Native binary analysis** — Uses the `object` crate for cross-platform ELF/Mach-O/PE parsing. No more shelling out to `size`/`nm`/`readelf`.
- 🔌 **Tauri-specific adapter** — Parses `tauri.conf.json` for feature detection, plugin cross-referencing, and app metadata enrichment.
- 💾 **Persistence** — Save/load size history to `.guardian-history.json`. Track binary size across builds with `--save`.
- 📊 **Export formats** — JSON, Prometheus metrics, Markdown tables, CSV.
- 🚨 **Alerting** — Detects growth spikes (>10%), large new dependencies (>500KB), absolute size thresholds, and debug info bloat.
- 📈 **Trend analysis** — `guardian trend` shows size history with per-build deltas and top growth contributors.
- 📋 **Integration examples** — CI workflow, pre-commit hook, Tauri CLI plugin.

## Quick Start

```bash
# Install
cargo install --path .

# Analyze a binary
guardian analyze ./target/release/my-app

# Analyze with Cargo.toml and Tauri config enrichment
guardian analyze ./target/release/my-app \
  --cargo-toml ./Cargo.toml \
  --tauri-conf ./src-tauri/tauri.conf.json \
  --assets-dir ./dist \
  --save

# Check against a budget
guardian init                          # creates guardian-budget.toml
guardian check ./target/release/my-app --budget guardian-budget.toml

# Compare two builds
guardian delta --before ./build-v1 --after ./build-v2

# View size trend
guardian trend --limit 20

# Export as Prometheus metrics
guardian export ./target/release/my-app --format prometheus -o metrics.txt

# Check alerts
guardian alerts ./target/release/my-app
```

## Commands

### `guardian analyze <PATH>`

Analyze a binary or bundle directory and print a conservation report.

| Flag | Description |
|------|-------------|
| `--cargo-toml` | Path to Cargo.toml for dependency analysis |
| `--assets-dir` | Path to assets directory |
| `--tauri-conf` | Path to tauri.conf.json for Tauri-specific analysis |
| `--json` | Output as JSON |
| `--save` | Save to history file |
| `--history` | Path to history file (default: `.guardian-history.json`) |

### `guardian check <PATH> --budget <FILE>`

Check a binary against a size budget. Exits with code 1 on violations.

### `guardian delta --before <PATH> --after <PATH>`

Compare two builds. Shows section and feature deltas.

### `guardian init [OUTPUT]`

Create a default budget file.

### `guardian export <PATH> --format <FORMAT>`

Export analysis in various formats: `json`, `prometheus`, `markdown`, `csv`.

### `guardian trend`

Show size trend across historical builds. Use `--limit` to control depth, `--markdown` for table output.

### `guardian alerts <PATH>`

Check for alert conditions: growth spikes, large new deps, absolute size thresholds, debug info bloat.

## Budget File

```toml
max_total_bytes = 15728640        # 15 MB
max_per_feature_bytes = 2097152   # 2 MB
max_webview_bundle_bytes = 5242880 # 5 MB
max_native_lib_bytes = 8388608    # 8 MB
max_debug_info_ratio = 0.10       # 10%
exempt_features = ["tauri", "webview"]
```

## How It Works

### Binary Analysis (v0.2.0 — `object` crate)

Uses the `object` crate to parse ELF, Mach-O, and PE binaries natively — no external tools required.

- **Sections**: Extracts all sections with sizes (`.text`, `.data`, `.rodata`, `.bss`, `.debug_*`)
- **Symbols**: Lists top 50 symbols by size with section attribution
- **Debug info**: Computes debug section total and ratio
- **Format detection**: Identifies binary format and architecture

### Feature Attribution

Cross-references Cargo.toml dependency names with Rust symbol prefixes (`<crate>::<module>::...`) to estimate per-crate size contribution.

### Tauri Integration

Parses `tauri.conf.json` to extract:
- App name and version
- Plugin list
- Bundle identifier
- Window count
- Cross-references plugins with detected binary symbols

### Persistence

Saves analysis snapshots to `.guardian-history.json` with:
- Timestamp, total size, section/feature counts
- Top 10 features by size
- Build metadata (format, architecture)

### Alerting Rules

| Alert | Trigger | Severity |
|-------|---------|----------|
| Growth spike | >10% since last build | Critical |
| Growth warning | >5% since last build | Warning |
| Large new dep | New dependency >500KB | Warning |
| Absolute size | >50MB binary | Critical |
| Absolute warning | >30MB binary | Warning |
| Debug info critical | >20% of binary | Critical |
| Debug info warning | >10% of binary | Warning |

## Building

```bash
cargo build --release
```

## Testing

```bash
cargo test
cargo clippy -- -D warnings
```

## License

MIT
