# hh-docversion

[![CI](https://github.com/harahorizon/hh-docversion/actions/workflows/ci.yml/badge.svg)](https://github.com/harahorizon/hh-docversion/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)
[![Telemetry](https://img.shields.io/badge/telemetry-zero-brightgreen.svg)](#zero-telemetry)
[![Rust](https://img.shields.io/badge/rust-1.78%2B-orange.svg)](Cargo.toml)

A zero-telemetry, client-side toolkit and Cargo plugin for automated multi-version documentation bundling, LLM context synthesis (`llms.txt`), and deterministic database schema contract verification.

Designed for local-first systems, embedded ledgers, and privacy-focused teams who require reproducible documentation trees, autonomous agent context indexes, and strict migration contracts without external SaaS dependencies.

---

## Why hh-docversion?

1. **Autonomous Agent Context (`llms.txt`)**: Automatically synthesizes single-file Markdown API indexes (`llms.txt` and `api-reference.md`) during site builds, capturing public interfaces, AST assertions, and explicit design contracts for LLM agents.
2. **Intent & Invariant Extraction**: Parses intentional design contracts (`# Intent`, `# Guarantees`, `# Invariants`, `# Non-Goals`, `# Notes`) from doc comments and automatically extracts runtime invariant guards (`assert!`, `ensure!`, `return Err(...)`).
3. **Obsidian-Themed Rustdoc Injection**: Injects a fixed, responsive topbar with active version switching, Git commit hashes, and an Obsidian Ayu aesthetic directly into `cargo doc` HTML without post-build tampering.
4. **Subprocess-Free Git Inspection**: Queries commit history, tags, and revision hashes directly in-process via pure-Rust `gix`.
5. **Versioned Docs Without SaaS**: Compiles version-segregated documentation trees (`/latest`, `/vX.Y.Z`, `versions.json`) with automatic root redirects for GitHub Pages, Cloudflare Pages, or static web servers.
6. **Ecosystem-Agnostic Ingestion**: Works seamlessly with `cargo doc` or static output from `TypeDoc`, `VitePress`, `Sphinx`, `MkDocs`, and `Starlight`.
7. **Deterministic DB Contracts**: Validates application SemVer boundaries against SQLite/DB migration matrix schemas (`schema_matrix.json`) and verifies cryptographic SHA-256 artifact checksums before application boot.
8. **Zero Telemetry**: Completely offline, self-contained, and audit-friendly.

---

## Installation

### Build and Install from Source

```bash
cargo install --path crates/cargo-docversion --force
This installs both the docversion standalone CLI and the cargo-docversion Cargo plugin into ~/.cargo/bin.

Quick Start
1. Document & Bundle Multi-Version Rust Trees
Bash
# Document workspace crates with injected Obsidian topbar and theme
cargo docversion doc --no-deps

# Assemble versioned distribution tree into target/doc_site and generate llms.txt
cargo docversion build-site --out-dir target/doc_site
Or run the all-in-one pipeline:

Bash
cargo docversion site --out-dir target/doc_site
2. Autonomous Agent Context (llms.txt)
Every build-site run generates an agent-ready documentation bundle:

target/doc_site/llms.txt: Primary context file containing crate intent, module contracts, and public API interfaces.

target/doc_site/api-reference.md: Mirrored reference markdown.

target/doc_site/vX.Y.Z/llms.txt: Version-locked agent manifest.

3. Documenting Architectural Intent & Contracts
Document your modules and types using clean markdown or blockquote tags. hh-docversion parses them for agents in llms.txt while rendering styled callouts for human readers in Rustdoc:

Rust
//! # Stage 04: Tuos
//! Double-entry transactional reconciliation engine.
//!
//! > **🎯 INTENT**  
//! > Guarantees absolute deterministic ledger accounting and verification. Tuos acts as 
//! > the terminal validator for financial events before persistent journal serialization.
//!
//! > **🛡️ INVARIANT**  
//! > - Zero-sum debit and credit balances across every committed journal transaction.  
//! > - Monotonic sequence increments: rejected if timestamp or index regresses.
//!
//! > **⛔ NON-GOAL**  
//! > External distributed consensus or unsanctioned state recovery.
//!
//! > **📝 NOTE**  
//! > Replay integrity relies on uncorrupted cryptographic hash chains.
docversion automatically extracts both your declared invariants and your code-level runtime assertions (assert!, ensure!, Err(...) returns) into a structured manifest:

Markdown
### `struct JournalBatch`

#### **Intent Directive**
> Guarantees absolute deterministic ledger accounting and verification.

#### **System Invariants**
* 🛡️ `Zero-sum debit and credit balances across every committed journal transaction.`
* 🛡️ `Monotonic sequence increments: rejected if timestamp or index regresses.`

#### **Programmatic Invariants & Rejection Boundaries**
* 🔒 `assert_eq`: `debit_total == credit_total`
* 🚫 Reject: `Err(HaraError::Validation("Zero-amount transaction rejected"))`
4. Ingest External Docs (TypeDoc, VitePress, etc.)
Bash
docversion build-site \
  --source docs/dist \
  --version 1.0.0 \
  --entry-page index.html \
  --out-dir public_site
5. Verify Database Schema Contracts
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

# Build documentation distribution bundle and LLM index
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