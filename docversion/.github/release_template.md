## `docversion` Release Checklist & Notes

**Tag:** `v$VERSION`  
**License:** MIT OR Apache-2.0  
**Status:** Open Source / Community Maintained (Provided As-Is)

---

### What's Changed
- <!-- Summarize the main updates, fixes, or API refinements -->

### Workspace Crates Included
- `docversion-core`: Pure version resolution, target path mapping, and redirect generation.
- `docversion-git`: Zero-maintenance Git tree hashing and revision fallback.
- `docversion-manifest`: Cross-repo database compatibility contract validator.
- `cargo-docversion`: CLI binary and `cargo xtask` runner.

---

### Installation & Quick Start

Install via Cargo:
```bash
cargo install cargo-docversion --locked
Build version-aware documentation:

Bash
cargo docversion doc --no-deps
Validate database schema compatibility:

Bash
cargo docversion verify-db --manifest schema_matrix.json --schema core-ledger --db-version 4
Maintenance & Support Notice
This utility is open-sourced under the MIT and Apache-2.0 licenses without formal commercial SLAs or dedicated triage guarantees. Community contributions, bug fixes, and feature enhancements are welcome via Pull Requests. If you are interested in becoming a co-maintainer, please review MAINTENANCE.md and reach out via the repository discussions.
