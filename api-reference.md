# cargo-docversion

> Autonomous Agent API Context & Single-File Documentation Index

---

## Crate `cargo-docversion`

#### **Intent Directive**

> Universal version-aware documentation site bundler and DB schema matrix contract verifier.

### `struct DocversionCli`

**Fields:**

* `command: Commands`

### `struct CrateMetadata`

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

### `struct RepoMetadata`

**Fields:**

* `workspace_name: String`
* `crates: Vec<CrateMetadata>`

### `struct RepoMetadataArgs`

**Fields:**

* `out: PathBuf` - Output path for synthesized repo_metadata.json

### `struct GithubMetaArgs`

**Fields:**

* `manifest: PathBuf` - Path to synthesized repo_metadata.json manifest

### `fn generate_markdown_summary`

Two-pass workspace scanner: 1. Indexes all public symbols across workspace crates into `WorkspaceSymbolIndex`. 2. Renders public interfaces, doctests, multi-language files, and handwritten `docs/*.md` guides with resolved cross-crate intra-doc links.

~~~rust
pub fn generate_markdown_summary(workspace_title: &str, crate_root: &Path) -> Result<String>
~~~

### `fn export_tracked_docs_tree`

~~~rust
pub fn export_tracked_docs_tree(workspace_root: &Path, out_dir: &Path) -> Result<()>
~~~

### `enum InvariantKind`

**Variants:**

* `Assert`
* `ErrorRejection`

### `struct ExtractedAssertion`

Extracted runtime invariant check, assertion, or error rejection boundary.

**Fields:**

* `kind: InvariantKind`
* `condition: String`

### `struct AssertionVisitor`

**Fields:**

* `assertions: Vec<ExtractedAssertion>`

### `fn default` (impl Default for AssertionVisitor)

~~~rust
fn default() -> Self
~~~

### `fn new` (impl AssertionVisitor)

~~~rust
pub fn new() -> Self
~~~

### `fn visit_expr_macro` (impl Visit<'ast> for AssertionVisitor)

~~~rust
fn visit_expr_macro(&mut self, node: &'ast ExprMacro)
~~~

### `fn visit_expr_call` (impl Visit<'ast> for AssertionVisitor)

~~~rust
fn visit_expr_call(&mut self, node: &'ast ExprCall)
~~~

### `fn format_assertions_markdown`

~~~rust
pub fn format_assertions_markdown(out: &mut String, assertions: &[ExtractedAssertion], heading_level: usize,)
~~~

### `struct CargoCli`

**Fields:**

* `command: CargoSubcommands`

### `enum CargoSubcommands`

**Variants:**

* `Docversion`

### `enum Commands`

**Variants:**

* `Doc` - Document workspace crates with version-injected banners
* `ExportMd` - Export tracked Markdown docs/ directory (README.md index, per-module .md files, api-reference.md)
* `RepoMetadata` - Synthesize deterministic repo_metadata.json from Cargo.toml and Intent blocks
* `GithubMeta` - Validate and preview repository metadata against GitHub constraints offline
* `VerifyDb` - Verify application compatibility against schema migration matrix
* `Site` - One-shot build: compiles documentation and bundles versioned site

### `enum DocFormat`

**Variants:**

* `Standard` - Standard site bundle (Rustdoc HTML + llms.txt + api-reference.md)
* `Shortcut` - Clean Markdown sanitized for Shortcut Docs import (strips raw HTML, rewrites relative .md links)

### `struct DocArgs`

**Fields:**

* `no_deps: bool` - Skip documenting dependent crates
* `workspace: bool` - Document all packages in the workspace
* `cargo_args: Vec<String>` - Pass additional flags directly to cargo doc

### `struct ExportMdArgs`

**Fields:**

* `out_dir: PathBuf` - Output directory for tracked Markdown documentation tree

### `struct VerifyDbArgs`

**Fields:**

* `manifest: PathBuf` - Path to schema compatibility matrix JSON file
* `schema: Option<String>` - Unique schema key identifier
* `db_version: Option<u32>` - Target database schema version
* `app_version: Option<String>` - Current application SemVer version
* `artifact: Option<PathBuf>` - Optional artifact file to verify against SHA-256 checksum

### `struct SiteArgs`

**Fields:**

* `source: PathBuf` - Path to documentation source directory (defaults to target/doc)
* `version: Option<String>` - Explicit SemVer version for distribution (inferred from workspace if omitted)
* `entry_page: String` - Landing page relative path within docs
* `out_dir: PathBuf` - Output directory for bundled distribution
* `format: DocFormat` - Output format mode

### `fn run_cargo_plugin`

~~~rust
pub fn run_cargo_plugin (cli: CargoCli) -> Result<()>
~~~

### `fn run_cli`

~~~rust
pub fn run_cli(cli: DocversionCli) -> Result<()>
~~~

---

## Crate `docversion-core`

#### **Intent Directive**

> Core path routing, redirect synthesis, and doc bundler logic for hh-docversion.

### Module `shortcut`

Clean Markdown exporter and rich-text HTML converter for Shortcut Docs compatibility. Strips raw HTML tags outside fenced code blocks, normalizes internal Rustdoc/site links to clean relative Markdown paths, and converts sanitized Markdown (including GFM pipe tables and fenced code blocks) to semantic HTML with language-agnostic syntax highlighting for RTF paste.

#### `fn sanitize_for_shortcut_docs`

Sanitizes generated Markdown for zero-friction import into Shortcut Docs. Preserves fenced code blocks (` ``` ` and `~~~`) and inline backtick spans verbatim. Converts `<br>` / `<br/>` into Markdown hard line breaks (`  \n`). Converts `<details>` / `<summary>` blocks into clean Markdown bold headers. Strips all other raw HTML tags (`<div>`, `<span>`, `<a name="...">`, etc.) while keeping inner text. Rewrites `.html` and root-prefixed (`/latest/...`, `/vX.Y.Z/...`) links to relative `.md` paths.

~~~rust
pub fn sanitize_for_shortcut_docs(markdown: &str) -> String
~~~

#### `fn markdown_to_clean_html`

Converts sanitized Shortcut Markdown into compact, well-spaced semantic HTML with GFM table support and language-agnostic syntax highlighting for RTF conversion.

~~~rust
pub fn markdown_to_clean_html(markdown: &str) -> String
~~~

#### `fn highlight_code_universal`

Language-agnostic lexical syntax highlighter emitting uniform styled spans so macOS `textutil` RTF conversion preserves consistent weight and monospace runs.

~~~rust
pub fn highlight_code_universal(line: &str) -> String
~~~

### Module `xref`

Workspace-wide cross-crate symbol index and intra-doc link resolver.

#### `enum SymbolKind`

Kind of public Rust item indexed across workspace member crates.

**Variants:**

* `Module`
* `Struct`
* `Enum`
* `Trait`
* `Function`
* `Method`
* `TypeAlias`
* `Constant`

#### `struct SymbolEntry`

Canonical location of an indexed workspace symbol.

**Fields:**

* `crate_ident: String` - Cargo crate identifier using underscores (e.g., `docversion_core`).
* `full_path: String` - Full Rust path (e.g., `docversion_core::shortcut::sanitize_for_shortcut_docs`).
* `short_name: String` - Short item name (e.g., `sanitize_for_shortcut_docs` or `VersionIdentifier::new`).
* `kind: SymbolKind` - Item classification.
* `markdown_anchor: String` - Single-file Markdown heading slug (e.g., `#fn-sanitize_for_shortcut_docs`).
* `doc_rel_path: String` - Multi-file tracked `docs/` relative path (e.g., `docversion_core.md` or `docversion_core/shortcut.md`).
* `rustdoc_path: String` - Relative Rustdoc HTML path (e.g., `docversion_core/shortcut/fn.sanitize_for_shortcut_docs.html`).

#### `struct WorkspaceSymbolIndex`

Exact workspace symbol registry for cross-crate intra-doc link resolution.

**Fields:**


#### `fn new` (impl WorkspaceSymbolIndex)

~~~rust
pub fn new() -> Self
~~~

#### `fn insert` (impl WorkspaceSymbolIndex)

Registers a public symbol in the workspace index.

~~~rust
pub fn insert(&mut self, entry: SymbolEntry)
~~~

#### `fn resolve` (impl WorkspaceSymbolIndex)

Resolves a Rustdoc intra-doc target relative to `current_crate` and optional `current_type`.

~~~rust
pub fn resolve(&self, raw_target: &str, current_crate: &str, current_type: Option<&str>,) -> Option<&SymbolEntry>
~~~

#### `fn resolve_markdown_links` (impl WorkspaceSymbolIndex)

Rewrites Rustdoc intra-doc links in single-file Markdown prose into `#anchor` links.

~~~rust
pub fn resolve_markdown_links(&self, markdown: &str, current_crate: &str, current_type: Option<&str>,) -> String
~~~

#### `fn resolve_multifile_markdown_links` (impl WorkspaceSymbolIndex)

Rewrites Rustdoc intra-doc links in multi-file `docs/` exports into relative `.md#anchor` links calculated from `current_doc_path` (e.g., `"docversion_core/shortcut.md"`).

~~~rust
pub fn resolve_multifile_markdown_links(&self, markdown: &str, current_crate: &str, current_type: Option<&str>, current_doc_path: &str,) -> String
~~~

#### `fn relative_doc_path`

Computes a POSIX-style relative Markdown path from `from_file` to `to_file` inside `docs/`.

~~~rust
pub fn relative_doc_path(from_file: &str, to_file: &str) -> String
~~~

### `enum CoreError`

Core domain errors for version identification and safe redirect generation.

**Variants:**

* `InvalidVersion`
* `UnsafePath`

### `fn from` (impl From<semver::Error> for CoreError)

~~~rust
fn from(err: semver::Error) -> Self
~~~

### `struct VersionIdentifier`

Canonical SemVer + optional Git revision identifier for documentation builds.

**Fields:**

* `declared_version: Version`
* `revision_suffix: Option<String>`

### `fn new` (impl VersionIdentifier)

Constructs a new `VersionIdentifier` from a SemVer string and an optional revision suffix.

~~~rust
use docversion_core::VersionIdentifier;

let ver = VersionIdentifier::new("1.4.2", Some("rev.8a4c1f0")).unwrap();
assert_eq!(ver.display_version(), "v1.4.2 (rev.8a4c1f0)");
assert_eq!(ver.target_directory(), "v1.4.2");
~~~

~~~rust
pub fn new(declared: &str, revision: Option<&str>) -> Result<Self, CoreError>
~~~

### `fn display_version` (impl VersionIdentifier)

Formats the version for display banners.

~~~rust
pub fn display_version(&self) -> String
~~~

### `fn target_directory` (impl VersionIdentifier)

Resolves the filesystem target directory component.

~~~rust
pub fn target_directory(&self) -> String
~~~

### `fn fmt` (impl fmt::Display for VersionIdentifier)

~~~rust
fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result
~~~

### `fn escape_html_attribute`

Escapes special HTML characters for safe attribute interpolation.

~~~rust
pub fn escape_html_attribute(input: &str) -> String
~~~

### `fn generate_redirect_html`

Generates a validated, escaped HTML redirect targeting `target_url`.

~~~rust
use docversion_core::generate_redirect_html;

let markup = generate_redirect_html("latest/docversion/index.html").unwrap();
assert!(markup.contains("latest/docversion/index.html"));
~~~

~~~rust
pub fn generate_redirect_html(target_url: &str) -> Result<String, CoreError>
~~~

#### **Programmatic Invariants & Rejection Boundaries**

* 🚫 Reject: `Err(CoreError::UnsafePath(target_url . to_string()))`

---

## Crate `docversion-git`

#### **Intent Directive**

> In-process Git tree hash and revision resolver for hh-docversion.

### `struct GitContext`

**Fields:**

* `tree_hash: String`
* `is_dirty: bool`
* `latest_tag: Option<String>`

### `fn resolve` (impl GitContext)

~~~rust
pub fn resolve<P: AsRef<Path>>(repo_path: P) -> Result<Self>
~~~

### `fn is_working_tree_dirty`

Checks whether the Git working tree has uncommitted changes.

~~~rust
pub fn is_working_tree_dirty<P: AsRef<Path>>(repo_path: P) -> Result<bool>
~~~

### `fn compute_doc_revision`

Computes the revision string for the specified paths (e.g. "rev.1a2b3c4" or "rev.1a2b3c4+dirty").

~~~rust
pub fn compute_doc_revision<P: AsRef<Path>>(repo_path: P, _paths: &[&str]) -> Result<String>
~~~

---

## Crate `docversion-manifest`

#### **Intent Directive**

> Schema compatibility matrix parser and contract assertions for hh-docversion.

### `enum ContractError`

**Variants:**

* `Semver`
* `Io`
* `RangeMismatch`
* `NotFound`
* `ChecksumMismatch`

### `struct SchemaContract`

**Fields:**

* `schema_id: String`
* `schema_version: u32`
* `supported_app_range: String`
* `artifact_uri: String`
* `sha256: String`

### `struct SchemaMatrix`

**Fields:**

* `generated_at: String`
* `contracts: Vec<SchemaContract>`

### `fn write` (impl Write for Sha256Writer<D>)

~~~rust
fn write(&mut self, buf: &[u8]) -> io::Result<usize>
~~~

### `fn flush` (impl Write for Sha256Writer<D>)

~~~rust
fn flush(&mut self) -> io::Result<()>
~~~

### `fn from_json_slice` (impl SchemaMatrix)

Deserializes a compatibility matrix from JSON bytes.

~~~rust
use docversion_manifest::SchemaMatrix;

let raw = br#"{"generated_at":"2026-11-07T00:00:00Z","contracts":[]}"#;
let matrix = SchemaMatrix::from_json_slice(raw).unwrap();
assert!(matrix.contracts.is_empty());
~~~

~~~rust
pub fn from_json_slice(data: &[u8]) -> Result<Self, serde_json::Error>
~~~

### `fn verify_compatibility` (impl SchemaMatrix)

Verifies whether the specified application version satisfies the schema contract requirement.

~~~rust
use docversion_manifest::{SchemaContract, SchemaMatrix};

let matrix = SchemaMatrix {
generated_at: "2026-11-07T00:00:00Z".into(),
contracts: vec![SchemaContract {
schema_id: "core-ledger".into(),
schema_version: 4,
supported_app_range: ">=1.4.0, <2.0.0".into(),
artifact_uri: "[https://example.com/db.sqlite](https://example.com/db.sqlite)".into(),
sha256: "abc".into(),
}],
};

assert!(matrix.verify_compatibility("1.5.0", "core-ledger", 4).is_ok());
assert!(matrix.verify_compatibility("2.1.0", "core-ledger", 4).is_err());
~~~

~~~rust
pub fn verify_compatibility(&self, app_version_str: &str, target_schema_id: &str, actual_db_version: u32,) -> Result<&SchemaContract, ContractError>
~~~

#### **Programmatic Invariants & Rejection Boundaries**

* 🚫 Reject: `Err(ContractError::RangeMismatch { app_ver, range: contract . supported_app_range . clone(), schema_id: target_schema_id . to_string(), schema_ver: actual_db_version, })`

### `fn verify_file_checksum` (impl SchemaMatrix)

Computes the artifact digest using a zero-allocation buffered streaming pipeline.

~~~rust
pub fn verify_file_checksum<P: AsRef<Path>>(file_path: P, expected_sha256: &str,) -> Result<(), ContractError>
~~~

#### **Programmatic Invariants & Rejection Boundaries**

* 🚫 Reject: `Err(ContractError::ChecksumMismatch { path: file_path . as_ref() . display() . to_string(), expected: expected_sha256 . to_string(), actual: actual_hash, })`

