# Crate `docversion-core` (`v0.1.0`)

[← Back to Workspace Index](./README.md) · [Full API Reference](./api-reference.md)

## Module [`shortcut`](./docversion_core/shortcut.md)

## Module [`xref`](./docversion_core/xref.md)

## `enum CoreError`

Core domain errors for version identification and safe redirect generation.

**Variants:**

* `InvalidVersion`
* `UnsafePath`

## `fn from` (impl From for CoreError)

~~~rust
fn from(err: semver::Error) -> Self
~~~

## `struct VersionIdentifier`

Canonical SemVer + optional Git revision identifier for documentation builds.

**Fields:**

* `declared_version: Version`
* `revision_suffix: Option<String>`

## `fn new` (impl VersionIdentifier)

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

## `fn display_version` (impl VersionIdentifier)

Formats the version for display banners.

~~~rust
pub fn display_version(&self) -> String
~~~

## `fn target_directory` (impl VersionIdentifier)

Resolves the filesystem target directory component.

~~~rust
pub fn target_directory(&self) -> String
~~~

## `fn fmt` (impl fmt::Display for VersionIdentifier)

~~~rust
fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result
~~~

## `fn escape_html_attribute`

Escapes special HTML characters for safe attribute interpolation.

~~~rust
pub fn escape_html_attribute(input: &str) -> String
~~~

## `fn generate_redirect_html`

Generates a validated, escaped HTML redirect targeting `target_url`.

~~~rust
use docversion_core::generate_redirect_html;

let markup = generate_redirect_html("latest/docversion/index.html").unwrap();
assert!(markup.contains("latest/docversion/index.html"));
~~~

~~~rust
pub fn generate_redirect_html(target_url: &str) -> Result<String, CoreError>
~~~

### **Programmatic Invariants & Rejection Boundaries**

* 🚫 Reject: `Err(CoreError::UnsafePath(target_url . to_string()))`

