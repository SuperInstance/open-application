# Contributing to App Size Guardian

Thanks for your interest! Here's how to contribute.

## Development Setup

```bash
git clone https://github.com/SuperInstance/tauri.git
cd tauri/guardian
cargo build
cargo test
```

## Making Changes

1. **Fork** the repo and create a branch from `guardian`.
2. **Write code** — follow existing style. Run `cargo fmt` before committing.
3. **Test** — `cargo test` must pass. Add tests for new functionality.
4. **Lint** — `cargo clippy -- -D warnings` must be clean.
5. **Commit** — use conventional commits: `feat:`, `fix:`, `docs:`, `chore:`.

## Code Style

- Rust 2021 edition, stable toolchain.
- Use `anyhow` for error handling in the binary, `thiserror` if we add a library later.
- Prefer pure-Rust dependencies over shelling out.
- All public functions need doc comments.
- New commands need tests and README documentation.

## Adding a New Command

1. Add the command variant to the `Commands` enum in `main.rs`.
2. Implement the logic in a dedicated module (e.g., `src/my_feature.rs`).
3. Add `mod my_feature;` to `main.rs`.
4. Write tests in the module.
5. Update README and CHANGELOG.

## Reporting Issues

Open an issue at [github.com/SuperInstance/tauri/issues](https://github.com/SuperInstance/tauri/issues) with:

- Guardian version (`guardian --version`)
- OS and architecture
- Steps to reproduce
- Expected vs actual behavior

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
