# temper: Agent Instructions

Instructions for autonomous coding agents working on `Tools/temper`.

## Stack & Conventions

- Language: Pure Rust (2021 edition, MSRV 1.82).
- Safety: #![forbid(unsafe_code)] is required across the crate.
- Dependencies: Zero -sys FFI crates. Pure Rust dependencies only.
- Output: Every CLI subcommand must support --json and emit canonical HarnessEnvelope<T>.
- Exit codes:
  - 0: Evaluation passed, lock verification succeeded.
  - 1: Assertion, reliability score, or hygiene check failed.
  - 2: I/O, parse, lockfile drift, or missing evidence error.
- Typography: Zero em-dashes and zero en-dashes across all files.
- Gates: Run full gates via `q.mjs`:
  ```bash
  node C:/Users/Admin/TechTide/Tools/agent-harness/q.mjs -- cargo fmt --all -- --check
  node C:/Users/Admin/TechTide/Tools/agent-harness/q.mjs -- cargo clippy --all-targets -- -D warnings
  node C:/Users/Admin/TechTide/Tools/agent-harness/q.mjs -- cargo test --all-targets
  ```
