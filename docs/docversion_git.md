# Crate `docversion-git` (`v0.1.0`)

[← Back to Workspace Index](./README.md) · [Full API Reference](./api-reference.md)

## `struct GitContext`

**Fields:**

* `tree_hash: String`
* `is_dirty: bool`
* `latest_tag: Option<String>`

## `fn resolve` (impl GitContext)

~~~rust
pub fn resolve<P: AsRef<Path>>(repo_path: P) -> Result<Self>
~~~

## `fn is_working_tree_dirty`

Checks whether the Git working tree has uncommitted changes.

~~~rust
pub fn is_working_tree_dirty<P: AsRef<Path>>(repo_path: P) -> Result<bool>
~~~

## `fn compute_doc_revision`

Computes the revision string for the specified paths (e.g. "rev.1a2b3c4" or "rev.1a2b3c4+dirty").

~~~rust
pub fn compute_doc_revision<P: AsRef<Path>>(repo_path: P, _paths: &[&str]) -> Result<String>
~~~

