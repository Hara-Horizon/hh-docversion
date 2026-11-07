# Contributing to docversion

Thank you for your interest in contributing. `docversion` is a community-supported utility. We value focused, high-quality contributions that respect the project's architectural invariants.

---

## Core Invariants

1. **Zero External Telemetry:** No tracking, metrics, analytics, or calls to external CDNs.
2. **Minimal Dependencies:** Avoid adding heavy transitive dependencies. Prefer standard library solutions where feasible.
3. **SemVer Safety:** Do not manipulate `package.version` strings in ways that break Cargo's resolver or crates.io idempotency.
4. **Comprehensive Testing:** All PRs must include unit tests, integration tests, or doctests for new functionality.

---

## Local Development Workflow

### 1. Prerequisites
* Rust 1.80.0+ (MSRV)
* Git 2.25+

### 2. Running Quality Checks
Before submitting a pull request, ensure the entire test suite and linter pass cleanly:

```bash
# Check formatting
cargo fmt --all -- --check

# Run linter
cargo clippy --workspace --all-targets -- -D warnings

# Run all unit tests, integration tests, and doctests
cargo test --workspace --doc
cargo test --workspace
```

---

## Co-Maintainership

If you are actively using `docversion` in production and would like merge permissions or co-ownership of the crate to keep it maintained, please refer to [MAINTENANCE.md](MAINTENANCE.md).