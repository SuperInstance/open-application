# Tauri CLI Plugin Integration

## Option 1: npm script

Add to your `package.json`:

```json
{
  "scripts": {
    "size:analyze": "guardian analyze ./src-tauri/target/release/my-app --tauri-conf ./src-tauri/tauri.conf.json --cargo-toml ./src-tauri/Cargo.toml --assets-dir ./dist --save",
    "size:check": "guardian check ./src-tauri/target/release/my-app --budget guardian-budget.toml --tauri-conf ./src-tauri/tauri.conf.json",
    "size:trend": "guardian trend --limit 20 --markdown",
    "size:export": "guardian export ./src-tauri/target/release/my-app --format prometheus -o metrics/guardian.txt",
    "size:alerts": "guardian alerts ./src-tauri/target/release/my-app"
  }
}
```

## Option 2: GitHub Actions (full pipeline)

```yaml
- name: Build Tauri app
  run: npm run tauri build

- name: Size Guardian check
  run: |
    npm run size:check

- name: Export size metrics
  if: always()
  run: npm run size:export

- name: Upload metrics
  if: always()
  uses: actions/upload-artifact@v4
  with:
    name: size-metrics
    path: metrics/
```

## Option 3: Prometheus + Grafana

Export metrics on every build and push to your Prometheus pushgateway:

```bash
guardian export ./target/release/my-app --format prometheus | \
  curl --data-binary @- http://pushgateway:9091/metrics/job/tauri-build/commit/$GITHUB_SHA
```

Then in Grafana, create a dashboard tracking `guardian_binary_size_bytes` over time.

## Option 4: Cargo.toml integration

Add a build script that runs guardian automatically:

```toml
[package.metadata.guardian]
budget = "guardian-budget.toml"
history = ".guardian-history.json"
fail-on-violation = true
```

Then use a `build.rs` or xtask to invoke guardian after the build completes.
