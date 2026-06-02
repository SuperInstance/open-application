# App Size Guardian

Tracks app binary size across builds. Enforces size budgets per feature. Detects bloat before it ships.

Part of [SuperInstance/tauri](https://github.com/SuperInstance/tauri).

## What it does

```
$ guardian analyze ./target/release/my-app --cargo-toml ./Cargo.toml --assets-dir ./dist

📐 App Size Guardian Report ═══════════════════════════════════

  Your app is 12.3MB

  Composition
    WebView assets  4.1MB (33.3%)
    Native code     6.8MB (55.3%)
    Debug info      1.2MB (9.8%)

  Largest Assets
    app.js — 1.4MB (11.4%, javascript)
    icon-512.png — 380.0KB (3.0%, icon)
    vendor.js — 290.0KB (2.3%, javascript)

  Features by Size
    serde_json                800.0KB (120 symbols)
    tokio                     650.0KB (200 symbols)
    tauri                     520.0KB (95 symbols)

  ⚠ Bloat Detected

    🟡 Feature 'serde_json' adds 800.0KB across 120 symbols
       → Consider whether serde_json is worth its size cost, or if a lighter alternative exists.
    🔴 Debug info is 9.8% of binary (1.2MB of 12.3MB)
       → Strip debug symbols from release builds with `strip = true` in Cargo.toml.

  ═════════════════════════════════════════════════════════════
```

## Commands

### `guardian analyze <PATH>`

Analyze a binary or bundle directory and print a conservation report.

```
guardian analyze ./target/release/my-app
guardian analyze ./target/release/bundle/ --json
guardian analyze ./target/release/my-app --cargo-toml ./Cargo.toml --assets-dir ./dist
```

### `guardian check <PATH> --budget <FILE>`

Check a binary against a size budget. Exits with code 1 on violations.

```
guardian check ./target/release/my-app --budget guardian-budget.toml
```

### `guardian delta --before <PATH> --after <PATH>`

Compare two builds. Shows what grew, what shrunk, what appeared, what vanished.

```
guardian delta --before ./build-v1 --after ./build-v2
```

### `guardian init [OUTPUT]`

Create a default budget file.

```
guardian init
guardian init my-budget.toml
```

## Budget file

```toml
max_total_bytes = 15728640        # 15 MB
max_per_feature_bytes = 2097152   # 2 MB
max_webview_bundle_bytes = 5242880 # 5 MB
max_native_lib_bytes = 8388608    # 8 MB
max_debug_info_ratio = 0.10       # 10%
exempt_features = ["tauri", "webview"]
```

## How it works

1. **Binary analysis** — runs `size`, `nm`, and `readelf` on ELF binaries to extract section sizes, symbol tables, and debug info ratios. For directory bundles, walks the tree and categorizes files by extension.

2. **Feature attribution** — cross-references Cargo.toml dependency names with Rust symbol prefixes (`<crate>::<module>::...`) to estimate per-crate size contribution.

3. **Bloat detection** — flags features adding >500KB, unused Cargo dependencies, unstripped debug info, oversized asset collections, and WebView bundle bloat.

4. **Delta tracking** — compares two analyses section-by-section and feature-by-feature, showing what changed between builds.

## Building

```
cd guardian
cargo build --release
```

The binary is `guardian` — put it wherever you want. Run it in CI, run it locally, run it every build.

## In CI

```yaml
- name: Check app size
  run: |
    cargo build --release
    guardian check ./target/release/my-app --budget guardian-budget.toml \
      --cargo-toml ./Cargo.toml --assets-dir ./dist
```

## Tests

```
cargo test
```

## License

MIT
