# CHANGELOG

## v0.2.0 — 2026-06-02

### Added

- **Native binary analysis adapter** — Parse ELF/Mach-O/PE sections using the `object` crate instead of shelling out to `size`/`nm`/`readelf`. Cross-platform, zero external dependencies.
- **Tauri-specific adapter** — Parse `tauri.conf.json` for feature detection. Cross-reference Cargo.toml dependencies with actual binary symbols. Extract app metadata (name, version, plugins, windows).
- **Persistence** — Save/load size history to `.guardian-history.json` via `--save` flag. Track binary size across builds.
- **Export formats** — `guardian export` with `--format json|prometheus|markdown|csv`. Output to stdout or file with `-o`.
- **Alerting** — `guardian alerts` command. Detects growth spikes (>10%), large new dependencies (>500KB), absolute size thresholds (30/50MB), and debug info bloat (>10/20%).
- **Trend analysis** — `guardian trend` shows size history with per-build deltas, overall trend, and top growth contributors. Markdown export via `--markdown`.
- **`--tauri-conf` flag** — Added to `analyze`, `check`, and `export` commands for Tauri enrichment.
- **Binary metadata** — Analysis now includes `binary_format` (elf/macho/pe/unknown) and `architecture` fields.
- **Integration examples** — GitHub Actions CI workflow, pre-commit hook, Tauri CLI plugin documentation.
- **CI** — cargo test + clippy on stable via GitHub Actions.
- **CONTRIBUTING.md** — Contribution guidelines.

### Changed

- Binary analysis no longer requires `binutils`/`readelf`/`nm` — uses pure Rust `object` crate.
- `BinaryAnalysis` struct now has `binary_format`, `architecture`, and `tauri_meta` fields.
- All structs derive `Clone` for easier manipulation.

### Fixed

- Cross-platform binary analysis now works on macOS and Windows without GNU binutils.
- Correctly handles missing external tools gracefully.

## v0.1.0 — 2026-05-30

### Added

- Initial release.
- `guardian analyze` — Analyze binary or bundle with conservation report.
- `guardian check` — Check against size budget (TOML config).
- `guardian delta` — Compare two builds.
- `guardian init` — Create default budget file.
- Bloat detection: oversized features, unused deps, debug info, WebView bloat.
- Feature attribution from Cargo.toml + symbol prefix matching.
- Asset categorization: icons, images, fonts, JS, CSS, HTML, WASM.
- Pretty terminal output with colored sections.
