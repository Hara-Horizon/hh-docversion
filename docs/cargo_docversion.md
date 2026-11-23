# Crate `cargo-docversion` (`v0.3.0`)

[← Back to Workspace Index](./README.md) · [Full API Reference](./api-reference.md)

### **Intent Directive**

> Universal version-aware documentation site bundler and DB schema matrix contract verifier.

## `struct DocversionCli`

**Fields:**

* `command: Commands`

## `struct CrateMetadata`

**Fields:**

* `name: String`
* `version: String`
* `description: Option<String>`
* `homepage: Option<String>`
* `repository: Option<String>`
* `documentation: Option<String>`
* `keywords: Vec<String>`
* `categories: Vec<String>`
* `intent_summary: Option<String>`

## `struct RepoMetadata`

**Fields:**

* `workspace_name: String`
* `crates: Vec<CrateMetadata>`

## `struct RepoMetadataArgs`

**Fields:**

* `out: PathBuf` - Output path for synthesized repo_metadata.json

## `struct GithubMetaArgs`

**Fields:**

* `manifest: PathBuf` - Path to synthesized repo_metadata.json manifest

## `struct AuditClaimsArgs`

**Fields:**

* `manifest: PathBuf` - Path to backlog stories JSON manifest
* `audit_log: PathBuf` - Path to audit NDJSON log file
* `workspace_root: PathBuf` - Workspace root directory to inspect
* `tickets: Vec<String>` - Specific ticket IDs to audit (comma-separated or multiple flags)
* `fail_on_gap: bool` - Fail the command if any audit gaps or unverified claims are found

## `fn generate_markdown_summary`

Two-pass workspace scanner: 1. Indexes all public symbols across workspace crates into `WorkspaceSymbolIndex`. 2. Renders public interfaces, doctests, multi-language files, and handwritten `docs/*.md` guides with resolved cross-crate intra-doc links.

~~~rust
pub fn generate_markdown_summary(workspace_title: &str, crate_root: &Path) -> Result<String>
~~~

## `fn export_tracked_docs_tree`

~~~rust
pub fn export_tracked_docs_tree(workspace_root: &Path, out_dir: &Path) -> Result<()>
~~~

## `enum InvariantKind`

**Variants:**

* `Assert`
* `ErrorRejection`

## `struct ExtractedAssertion`

**Fields:**

* `kind: InvariantKind`
* `condition: String`

## `struct AssertionVisitor`

**Fields:**

* `assertions: Vec<ExtractedAssertion>`

## `fn default` (impl Default for AssertionVisitor)

~~~rust
fn default() -> Self
~~~

## `fn new` (impl AssertionVisitor)

~~~rust
pub fn new() -> Self
~~~

## `fn visit_expr_macro` (impl Visit<'ast> for AssertionVisitor)

~~~rust
fn visit_expr_macro(&mut self, node: &'ast ExprMacro)
~~~

## `fn visit_expr_call` (impl Visit<'ast> for AssertionVisitor)

~~~rust
fn visit_expr_call(&mut self, node: &'ast ExprCall)
~~~

## `fn format_assertions_markdown`

~~~rust
pub fn format_assertions_markdown(out: &mut String, assertions: &[ExtractedAssertion], heading_level: usize,)
~~~

## `struct CargoCli`

**Fields:**

* `command: CargoSubcommands`

## `enum CargoSubcommands`

**Variants:**

* `Docversion`

## `enum Commands`

**Variants:**

* `Doc` - Document workspace crates with version-injected banners
* `ExportMd` - Export tracked Markdown docs/ directory (README.md index, per-module .md files, api-reference.md)
* `AuditClaims` - Audit ticket claims and workspace backlog stories against audit logs
* `RepoMetadata` - Synthesize deterministic repo_metadata.json from Cargo.toml and Intent blocks
* `GithubMeta` - Validate and preview repository metadata against GitHub constraints offline
* `VerifyDb` - Verify application compatibility against schema migration matrix
* `Site` - One-shot build: compiles documentation and bundles versioned site

## `enum DocFormat`

**Variants:**

* `Standard` - Standard site bundle (Rustdoc HTML + llms.txt + api-reference.md)
* `Shortcut` - Clean Markdown sanitized for Shortcut Docs import (strips raw HTML, rewrites relative .md links)

## `struct DocArgs`

**Fields:**

* `no_deps: bool` - Skip documenting dependent crates
* `workspace: bool` - Document all packages in the workspace
* `cargo_args: Vec<String>` - Pass additional flags directly to cargo doc

## `struct ExportMdArgs`

**Fields:**

* `out_dir: PathBuf` - Output directory for tracked Markdown documentation tree

## `struct VerifyDbArgs`

**Fields:**

* `manifest: PathBuf` - Path to schema compatibility matrix JSON file
* `schema: Option<String>` - Unique schema key identifier
* `db_version: Option<u32>` - Target database schema version
* `app_version: Option<String>` - Current application SemVer version
* `artifact: Option<PathBuf>` - Optional artifact file to verify against SHA-256 checksum

## `struct SiteArgs`

**Fields:**

* `source: PathBuf` - Path to documentation source directory (defaults to target/doc)
* `version: Option<String>` - Explicit SemVer version for distribution (inferred from workspace if omitted)
* `entry_page: String` - Landing page relative path within docs
* `out_dir: PathBuf` - Output directory for bundled distribution
* `format: DocFormat` - Output format mode

## `fn run_cargo_plugin`

~~~rust
pub fn run_cargo_plugin (cli: CargoCli) -> Result<()>
~~~

## `fn run_cli`

~~~rust
pub fn run_cli(cli: DocversionCli) -> Result<()>
~~~

