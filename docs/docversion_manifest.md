# Crate `docversion-manifest` (`v0.2.0`)

[← Back to Workspace Index](./README.md) · [Full API Reference](./api-reference.md)

### **Intent Directive**

> Schema compatibility matrix parser and contract assertions for hh-docversion.

## `enum ContractError`

**Variants:**

* `Semver`
* `Io`
* `RangeMismatch`
* `NotFound`
* `ChecksumMismatch`

## `struct SchemaContract`

**Fields:**

* `schema_id: String`
* `schema_version: u32`
* `supported_app_range: String`
* `artifact_uri: String`
* `sha256: String`

## `struct SchemaMatrix`

**Fields:**

* `generated_at: String`
* `contracts: Vec<SchemaContract>`

## `fn write` (impl Write for Sha256Writer)

~~~rust
fn write(&mut self, buf: &[u8]) -> io::Result<usize>
~~~

## `fn flush` (impl Write for Sha256Writer)

~~~rust
fn flush(&mut self) -> io::Result<()>
~~~

## `fn from_json_slice` (impl SchemaMatrix)

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

## `fn verify_compatibility` (impl SchemaMatrix)

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

### **Programmatic Invariants & Rejection Boundaries**

* 🚫 Reject: `Err(ContractError::RangeMismatch { app_ver, range: contract . supported_app_range . clone(), schema_id: target_schema_id . to_string(), schema_ver: actual_db_version, })`

## `fn verify_file_checksum` (impl SchemaMatrix)

Computes the artifact digest using a zero-allocation buffered streaming pipeline.

~~~rust
pub fn verify_file_checksum<P: AsRef<Path>>(file_path: P, expected_sha256: &str,) -> Result<(), ContractError>
~~~

### **Programmatic Invariants & Rejection Boundaries**

* 🚫 Reject: `Err(ContractError::ChecksumMismatch { path: file_path . as_ref() . display() . to_string(), expected: expected_sha256 . to_string(), actual: actual_hash, })`

