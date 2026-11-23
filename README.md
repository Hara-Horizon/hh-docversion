# hh-docversion

[![CI](https://github.com/Hara-Horizon/hh-docversion/actions/workflows/ci.yml/badge.svg)](https://github.com/Hara-Horizon/hh-docversion/actions/workflows/ci.yml)
[![Security Audit](https://github.com/Hara-Horizon/hh-docversion/actions/workflows/audit.yml/badge.svg)](https://github.com/Hara-Horizon/hh-docversion/actions/workflows/audit.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)
[![Telemetry](https://img.shields.io/badge/telemetry-zero-brightgreen.svg)](#zero-telemetry)
[![Rust](https://img.shields.io/badge/rust-1.78%2B-orange.svg)](Cargo.toml)

A zero-telemetry, client-side toolkit and Cargo plugin for automated multi-version documentation bundling, universal polyglot LLM context synthesis (`llms.txt`), Shortcut Docs RTF/Markdown export, and reliable database schema contract verification.

Designed for local-first systems, embedded ledgers, and privacy-focused teams who require reproducible documentation trees, autonomous agent context indexes, and strict migration contracts without external SaaS dependencies.

---

## Why hh-docversion?

1. **Autonomous Agent Context (`llms.txt`)**: Automatically synthesizes single-file Markdown API indexes (`llms.txt` and `api-reference.md`) during site builds across all workspace crates, capturing public interfaces, preserved doctests, AST assertions, and explicit design contracts for LLM agents.
2. **Shortcut Docs Clean Markdown & RTF Clipboard Export**: Supports `--format shortcut` to strip raw HTML tags outside fenced code blocks, rewrite internal Rustdoc/versioned links into clean relative `.md` paths, render GFM tables and uniform-weight syntax-highlighted code blocks, and automatically load rich-text RTF onto your clipboard (`textutil`/`pbcopy` on macOS, with `wl-copy`, `xclip`, and `clip.exe` fallbacks) for instant `Cmd+V` paste into Shortcut Docs.
3. **Universal Multi-Language Source Scanning**: Extracts public interfaces and docstrings across Cargo workspace crates (`.rs`) alongside TypeScript/JavaScript (`.ts`, `.tsx`, `.js`, `.jsx`), Python (`.py`), Go (`.go`), SQL (`.sql`), and authored `docs/**/*.md` guides into one unified index.
4. **Intent & Invariant Extraction**: Parses intentional design contracts (`# Intent`, `# Guarantees`, `# Invariants`, `# Non-Goals`, `# Notes`) from doc comments and automatically extracts deduplicated runtime invariant guards (`assert!`, `ensure!`, `return Err(...)`).
5. **Obsidian-Themed Rustdoc Injection**: Injects a fixed, responsive topbar with active version switching, Git commit hashes, and an Obsidian Ayu aesthetic directly into `cargo doc` HTML without post-build tampering.
6. **Subprocess-Free Git Inspection**: Queries commit history, tags, and revision hashes directly in-process via pure-Rust `gix`.
7. **Versioned Docs Without SaaS**: Compiles version-segregated documentation trees (`/latest`, `/vX.Y.Z`, `versions.json`) with automatic root redirects for GitHub Pages, Cloudflare Pages, or static web servers.
8. **Ecosystem-Agnostic Ingestion**: Works seamlessly with `cargo doc` or static output from `TypeDoc`, `VitePress`, `Sphinx`, `MkDocs`, and `Starlight`.
9. **Reliable DB Contracts**: Validates application SemVer boundaries against SQLite/DB migration matrix schemas (`schema_matrix.json`) and verifies cryptographic SHA-256 artifact checksums before application boot.
10. **Zero Telemetry**: Completely offline, self-contained, and audit-friendly.

---

## Installation

### Prerequisites

Ensure you have a modern Rust toolchain installed (MSRV: **1.78+**):

~~~bash
rustup update stable
~~~

### Option 1: Global Installation from Git (Recommended)

Install the latest tagged release directly from GitHub into `~/.cargo/bin` so it is globally available across all your workspace repositories:

~~~bash
cargo install --git https://github.com/Hara-Horizon/hh-docversion.git --tag v0.3.0 --force
~~~

To pin to a specific commit SHA for reproducible CI pipelines:

~~~bash
# Pin to a specific commit SHA
cargo install --git https://github.com/Hara-Horizon/hh-docversion.git --rev <COMMIT_SHA> cargo-docversion --force
~~~

### Option 2: Build and Install from Local Source

If you have cloned the repository locally inside `~/Projects/hh-docversion`:

~~~bash
cargo install --path crates/cargo-docversion --force
~~~

Both options install the standalone CLI (`docversion`) and the Cargo plugin (`cargo-hh-docversion`) into `~/.cargo/bin`.

### Option 3: Use as a Library Dependency

To integrate `docversion` engines into another Rust project:

~~~toml
[dependencies]
docversion-core = { git = "https://github.com/Hara-Horizon/hh-docversion.git", tag = "v0.3.0" }
docversion-git = { git = "https://github.com/Hara-Horizon/hh-docversion.git", tag = "v0.3.0" }
docversion-manifest = { git = "https://github.com/Hara-Horizon/hh-docversion.git", tag = "v0.3.0" }
~~~

---

## Quick Start

### 1. Document & Bundle Multi-Version Rust Trees

~~~bash
# Document workspace crates with injected Obsidian topbar and theme
cargo hh-docversion doc --no-deps

# Assemble versioned distribution tree into target/doc_site and generate llms.txt
cargo hh-docversion site --out-dir target/doc_site
~~~

### 2. Export Clean Markdown & Paste RTF into Shortcut Docs

Pass `--format shortcut` to `site` to strip raw HTML tags outside code fences, rewrite internal links into relative `.md` paths, write `target/doc_site/api-reference.md`, and automatically copy rich-text RTF (with GFM tables and syntax-highlighted code blocks) to your clipboard:

~~~bash
# Build with Shortcut Docs sanitization + automatic RTF clipboard copy
cargo hh-docversion site --format shortcut --out-dir target/doc_site
~~~

Open your target Shortcut Doc and press `Cmd+V` (`Ctrl+V` on Linux/Windows) to paste formatted headings, tables, blockquotes, and uniform-weight syntax-highlighted code blocks.

### 3. Autonomous Agent Context (`llms.txt`)

Every `site` run generates an agent-ready documentation bundle:

- `target/doc_site/llms.txt`: Primary context file containing workspace crate intent, module contracts, polyglot interfaces, and `docs/**/*.md` guides.
- `target/doc_site/api-reference.md`: Mirrored reference Markdown (sanitized for Shortcut Docs when `--format shortcut` is enabled).
- `target/doc_site/vX.Y.Z/llms.txt`: Version-locked agent manifest.

### 4. Documenting Architectural Intent & Contracts

Document your modules and types using clean markdown or blockquote tags. `hh-docversion` parses them for agents in `llms.txt` while rendering styled callouts for human readers in Rustdoc:

~~~rust
//! # Stage 04: Tuos
//! Double-entry transactional reconciliation engine.
//!
//! > **🎯 INTENT**
//! > Guarantees exact ledger accounting and verification. Tuos acts as
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
~~~

`docversion` automatically extracts both your declared invariants and your code-level runtime assertions (`assert!`, `ensure!`, `Err(...)` returns) into a structured manifest:

~~~markdown
### `struct JournalBatch`

#### **Intent Directive**
> Guarantees exact ledger accounting and verification.

#### **System Invariants**
- 🛡️ `Zero-sum debit and credit balances across every committed journal transaction.`
- 🛡️ `Monotonic sequence increments: rejected if timestamp or index regresses.`

#### **Programmatic Invariants & Rejection Boundaries**
- 🔒 `assert_eq`: `debit_total == credit_total`
- 🚫 Reject: `Err(HaraError::Validation("Zero-amount transaction rejected"))`
~~~

### 5. Ingest External Docs (TypeDoc, VitePress, etc.)

~~~bash
cargo hh-docversion site \
  --source docs/dist \
  --version 1.0.0 \
  --entry-page index.html \
  --out-dir public_site
~~~

### 6. Verify Database Schema Contracts

~~~bash
cargo hh-docversion verify-db \
  --manifest schema_matrix.json \
  --schema core-ledger \
  --db-version 1 \
  --app-version 0.3.0
~~~

---

## Developer Ergonomics: Git Hook Strategy

The repository includes a two-tiered `.githooks/` suite optimized for high-velocity micro-commits:

- **pre-commit (Sub-second)**: Runs `cargo check --workspace --quiet` and JSON structure verification. Keeps micro-commits instant.
- **pre-push (Gatekeeper)**: Runs `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings` prior to branch pushes.

### Activate Hooks

~~~bash
git config core.hooksPath .githooks
chmod +x .githooks/pre-commit .githooks/pre-push
~~~

---

## Zero Telemetry & Privacy Guarantees

`hh-docversion` operates entirely offline with zero telemetry collection, zero analytics beacons, and no external runtime network requests. All AST parsing, git interrogation, and LLM context synthesis happen locally within your trusted workspace boundary.

---

## License & Stewardship

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

`hh-docversion` is open-sourced as community-maintained infrastructure. Contributions, issues, and discussions are welcome.