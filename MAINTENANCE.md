# Maintenance Policy & Stewardship

**Status:** Minimal Maintenance / Open to Community Governance  
**Original Author / Sponsor:** Hara Horizon Software Systems Inc.  
**Primary License:** Dual MIT OR Apache-2.0

---

## 1. Scope of Author Support

`docversion` was developed by **Hara Horizon Software Systems Inc.** as an internal architectural solution and released to the Rust community under permissive open-source terms.

**Please note the following operational realities:**
* **As-Is Provision:** The original authors do not provide commercial SLA support, guaranteed response times, or roadmaps for feature requests.
* **Feature Completeness:** The core tools—version-synchronized documentation extraction and deterministic schema compatibility verification—are considered feature-complete for our internal use cases.
* **Liability & Warranty:** In accordance with the MIT and Apache-2.0 licenses, this software is provided *"as is"*, without warranty of any kind.

---

## 2. Community Stewardship & Co-Maintainership

We actively welcome developers and teams who rely on this crate to participate in maintenance or step into formal co-maintainer roles.

### How Issues & Pull Requests Are Handled:
1. **Critical Security & Rust Breakage:** PRs fixing compiler breakage across new stable Rust releases or security vulnerabilities will be reviewed and merged promptly.
2. **Feature PRs:** PRs containing clean tests, zero added telemetry, and no unnecessary runtime dependencies will be reviewed and merged.
3. **Issue Triage:** Issues reporting verified bugs with reproduction examples will be tagged for community resolution.

### Stepping Up as a Maintainer:
If you or your organization depend heavily on `docversion` and wish to help steer crate releases, triage PRs, or manage `crates.io` publishes:
* Demonstrate contributions through 2–3 merged PRs (fixes, docs, or feature improvements).
* Open an issue titled `[Maintainer Request] Your Name / Org` or contact `dev@harahorizon.com` to request repository write access or `crates.io` co-ownership.