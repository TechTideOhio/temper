# Contributing to temper

Thank you for contributing to `temper`, part of The Complete TideOS Eval Kit.

## Rules for Contributions

1. Pure Rust: No C-FFI or -sys crate dependencies.
2. Safety: #![forbid(unsafe_code)] is required across the codebase.
3. Errors: Never use .unwrap() in src/. Use thiserror for domain errors and anyhow in CLI handlers.
4. Lints: Code must pass `cargo clippy --all-targets -- -D warnings` with zero warnings.
5. Formatting: Code must pass `cargo fmt --all -- --check`.
6. Testing: All changes must include unit or integration tests.
7. Typography: Zero em-dashes and zero en-dashes across all files.

## Local Development

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo build --release
```
