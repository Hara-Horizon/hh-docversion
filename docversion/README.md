# docversion (`cargo-docversion`)

[![Crates.io](https://img.shields.io/crates/v/cargo-docversion.svg)](https://crates.io/crates/cargo-docversion)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)
[![Maintenance Status](https://img.shields.io/badge/maintenance-community--driven-orange.svg)](MAINTENANCE.md)
[![Zero Telemetry](https://img.shields.io/badge/telemetry-zero-brightgreen.svg)](#zero-telemetry-invariant)

> Zero-telemetry, version-aware documentation automation and deterministic schema compatibility resolution for Rust systems.

Released by **Hara Horizon Software Systems Inc.** Dual-licensed under **MIT OR Apache-2.0**.

---

## Maintenance Status & Expectations

This tool is **feature-complete for its original design** and operated as a **community-driven project**. The original authors do not actively solicit feature requests or provide ongoing maintenance guarantees. 

* Review [MAINTENANCE.md](MAINTENANCE.md) for maintenance boundaries and governance.
* Review [CONTRIBUTING.md](CONTRIBUTING.md) if you wish to contribute improvements or become a co-maintainer.

---

## Key Capabilities

1. **Synchronized Documentation Routing:** Builds versioned `rustdoc` and `mdBook` distributions tied directly to `package.version` in `Cargo.toml`.
2. **Zero-Maintenance Document Subversioning:** Derives subversion identifiers (e.g. `v1.4.0 (rev.8a4c1f0)`) dynamically via Git tree hashes touching documentation paths—eliminating manual manifest edits and SemVer pollution.
3. **Deterministic Schema Contract Enforcement:** Verifies cross-repository database schemas against application versions via content-addressed compatibility matrices (`schema_matrix.json`).
4. **Zero-Telemetry Invariant:** Completely self-contained; no network phone-homes, external tracker tags, or CDN script dependencies.

---

## Workspace Crates

| Crate | Purpose |
| :--- | :--- |
| **`crates/docversion-core`** | Domain logic, non-polluting version formatting, and HTML redirect synthesis. |
| **`crates/docversion-git`** | Git tree inspection and commit-distance subversion calculation. |
| **`crates/docversion-manifest`** | Schema compatibility matrix structs (`schema_matrix.json`) and SemVer validation. |
| **`crates/cargo-docversion`** | Unified CLI tool and `xtask` integration engine. |

---

## Quick Start

### 1. Build Version-Aware Documentation

Run from your workspace root:

```bash
cargo run -p cargo-docversion -- doc --no-deps
```

This derives the crate version and git doc-hash, synthesizes an HTML banner, and runs `cargo doc` with `--html-before-content`.

### 2. Verify Database Schema Compatibility

Assert that your running application version meets the database schema contract before running migrations or queries:

```bash
cargo run -p cargo-docversion -- verify-db \
  --manifest schema_matrix.json \
  --schema core-ledger \
  --db-version 4
```

---

## Multi-Version Hosting Pattern

For hosting multi-version documentation (e.g., GitHub Pages or air-gapped webservers), structure your output directory as follows:

```text
public/
├── index.html                   # Redirects to /latest/
├── versions.json                # Manifest listing active versions
├── v1.0.0/                      # Snapshot build of git tag v1.0.0
│   └── <crate_name>/index.html
├── v1.1.0/                      # Snapshot build of git tag v1.1.0
│   └── <crate_name>/index.html
└── latest/                      # Build of current main branch
    └── <crate_name>/index.html
```

Static switcher and banner assets are provided in [`static/`](static/).