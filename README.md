# hh-docversion

[![CI](https://github.com/harahorizon/hh-docversion/actions/workflows/ci.yml/badge.svg)](https://github.com/harahorizon/hh-docversion/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)
[![Telemetry](https://img.shields.io/badge/telemetry-zero-brightgreen.svg)](#zero-telemetry)
[![Rust](https://img.shields.io/badge/rust-1.78%2B-orange.svg)](Cargo.toml)

A zero-telemetry, client-side toolkit and Cargo plugin for automated multi-version documentation bundling and deterministic database schema contract verification.

Designed for local-first software, embedded ledgers, and privacy-focused teams who require reproducible documentation trees and strict migration contracts without external SaaS dependencies.

---

## Why hh-docversion?

1. **Subprocess-Free Git Inspection**: Queries commit history, tags, and revision hashes directly in-process via pure-Rust `gix`.
2. **Versioned Docs Without SaaS**: Compiles version-segregated documentation trees (`/latest`, `/vX.Y.Z`, `versions.json`) with automatic root redirects for GitHub Pages, Cloudflare Pages, or static web servers.
3. **Ecosystem-Agnostic Ingestion**: Works seamlessly with `cargo doc` or static output from `TypeDoc`, `VitePress`, `Sphinx`, `MkDocs`, and `Starlight`.
4. **Deterministic DB Contracts**: Validates application SemVer boundaries against SQLite/DB migration matrix schemas (`schema_matrix.json`) and verifies cryptographic SHA-256 artifact checksums before application boot.
5. **Zero Telemetry**: Completely offline, self-contained, and audit-friendly.

---

## Installation

### Build and Install from Source
```bash
cargo install --path crates/cargo-docversion --force
This installs both the docversion standalone CLI and the cargo-docversion plugin into ~/.cargo/bin.

Quick Start
1. Multi-Version Rust Documentation
Bash
# Document workspace crates
cargo docversion doc --no-deps

# Assemble versioned distribution tree into target/doc_site
docversion build-site --out-dir target/doc_site
2. Ingest External Docs (TypeDoc, VitePress, etc.)
Bash
docversion build-site \
  --source docs/dist \
  --version 1.0.0 \
  --entry-page index.html \
  --out-dir public_site
3. Verify Database Schema Contracts
Bash
docversion verify-db \
  --manifest schema_matrix.json \
  --schema core-ledger \
  --db-version 1 \
  --app-version 0.1.0
Workspace Orchestration (cargo xtask)
Automated workflows are managed through the internal xtask orchestrator:

Bash
# Fast CI check (check, workspace tests, doc-tests)
cargo xtask ci

# Build documentation distribution bundle
cargo xtask docs --out-dir target/doc_site

# Validate schema contracts against local matrix
cargo xtask verify-schema --schema core-ledger --db-version 1 --app-version 0.1.0

# End-to-end local test run
cargo xtask check-all
Developer Ergonomics: Git Hook Strategy
The repository includes a two-tiered .githooks/ suite optimized for high-velocity micro-commits:

pre-commit (Sub-second): Runs cargo check --workspace --quiet and JSON structure verification. Keeps micro-commits instant.

pre-push (Gatekeeper): Runs full cargo xtask ci test suite prior to branch pushes.

Activate Hooks
Bash
git config core.hooksPath .githooks
chmod +x .githooks/pre-commit .githooks/pre-push
License & Stewardship
Licensed under either of:

Apache License, Version 2.0 (LICENSE-APACHE)

MIT License (LICENSE-MIT)

hh-docversion is open-sourced as community-maintained infrastructure. Contributions, issues, and discussions are welcome.
