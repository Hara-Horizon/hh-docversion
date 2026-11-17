# Module `docversion_core::xref`

[← Workspace Index](../README.md) · [Crate `docversion_core`](../docversion_core.md)

Workspace-wide cross-crate symbol index and intra-doc link resolver.

## `enum SymbolKind`

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

## `struct SymbolEntry`

Canonical location of an indexed workspace symbol.

**Fields:**

* `crate_ident: String` - Cargo crate identifier using underscores (e.g., `docversion_core`).
* `full_path: String` - Full Rust path (e.g., `docversion_core::shortcut::sanitize_for_shortcut_docs`).
* `short_name: String` - Short item name (e.g., `sanitize_for_shortcut_docs` or `VersionIdentifier::new`).
* `kind: SymbolKind` - Item classification.
* `markdown_anchor: String` - Single-file Markdown heading slug (e.g., `#fn-sanitize_for_shortcut_docs`).
* `doc_rel_path: String` - Multi-file tracked `docs/` relative path (e.g., `docversion_core.md` or `docversion_core/shortcut.md`).
* `rustdoc_path: String` - Relative Rustdoc HTML path (e.g., `docversion_core/shortcut/fn.sanitize_for_shortcut_docs.html`).

## `struct WorkspaceSymbolIndex`

Exact workspace symbol registry for cross-crate intra-doc link resolution.

**Fields:**

## `fn new` (impl WorkspaceSymbolIndex)

~~~rust
pub fn new() -> Self
~~~

## `fn insert` (impl WorkspaceSymbolIndex)

Registers a public symbol in the workspace index.

~~~rust
pub fn insert(&mut self, entry: SymbolEntry)
~~~

## `fn resolve` (impl WorkspaceSymbolIndex)

Resolves a Rustdoc intra-doc target relative to `current_crate` and optional `current_type`.

~~~rust
pub fn resolve(&self, raw_target: &str, current_crate: &str, current_type: Option<&str>,) -> Option<&SymbolEntry>
~~~

## `fn resolve_markdown_links` (impl WorkspaceSymbolIndex)

Rewrites Rustdoc intra-doc links in single-file Markdown prose into `#anchor` links.

~~~rust
pub fn resolve_markdown_links(&self, markdown: &str, current_crate: &str, current_type: Option<&str>,) -> String
~~~

## `fn resolve_multifile_markdown_links` (impl WorkspaceSymbolIndex)

Rewrites Rustdoc intra-doc links in multi-file `docs/` exports into relative `.md#anchor` links calculated from `current_doc_path` (e.g., `"docversion_core/shortcut.md"`).

~~~rust
pub fn resolve_multifile_markdown_links(&self, markdown: &str, current_crate: &str, current_type: Option<&str>, current_doc_path: &str,) -> String
~~~

## `fn relative_doc_path`

Computes a POSIX-style relative Markdown path from `from_file` to `to_file` inside `docs/`.

~~~rust
pub fn relative_doc_path(from_file: &str, to_file: &str) -> String
~~~

