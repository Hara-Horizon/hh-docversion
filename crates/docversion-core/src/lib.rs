//! # Intent: Core path routing, redirect synthesis, and doc bundler logic for hh-docversion.
//!
//! Handles HTML redirect generation, link resolution, and static site assembly without external cloud dependencies.
pub mod shortcut;
pub mod xref;

pub use shortcut::{highlight_code_universal, markdown_to_clean_html, sanitize_for_shortcut_docs};
pub use xref::{SymbolEntry, SymbolKind, WorkspaceSymbolIndex};

use semver::Version;
use std::fmt;
use thiserror::Error;

/// Core domain errors for version identification and safe redirect generation.
#[derive(Error, Debug, PartialEq, Eq)]
pub enum CoreError {
    #[error("Invalid SemVer string '{0}': {1}")]
    InvalidVersion(String, String),

    #[error("Unsafe redirect target path rejected: '{0}'")]
    UnsafePath(String),
}

impl From<semver::Error> for CoreError {
    fn from(err: semver::Error) -> Self {
        CoreError::InvalidVersion(String::new(), err.to_string())
    }
}

/// Canonical SemVer + optional Git revision identifier for documentation builds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionIdentifier {
    pub declared_version: Version,
    pub revision_suffix: Option<String>,
}

impl VersionIdentifier {
    /// Constructs a new `VersionIdentifier` from a SemVer string and an optional revision suffix.
    ///
    /// ```rust
    /// use docversion_core::VersionIdentifier;
    ///
    /// let ver = VersionIdentifier::new("1.4.2", Some("rev.8a4c1f0")).unwrap();
    /// assert_eq!(ver.display_version(), "v1.4.2 (rev.8a4c1f0)");
    /// assert_eq!(ver.target_directory(), "v1.4.2");
    /// ```
    pub fn new(declared: &str, revision: Option<&str>) -> Result<Self, CoreError> {
        let trimmed = declared.trim().trim_start_matches('v');
        let parsed = Version::parse(trimmed)
            .map_err(|e| CoreError::InvalidVersion(declared.to_string(), e.to_string()))?;

        let clean_rev = revision
            .map(|r| r.trim().to_string())
            .filter(|r| !r.is_empty());

        Ok(Self {
            declared_version: parsed,
            revision_suffix: clean_rev,
        })
    }

    /// Formats the version for display banners.
    pub fn display_version(&self) -> String {
        match &self.revision_suffix {
            Some(rev) => format!("v{} ({})", self.declared_version, rev),
            None => format!("v{}", self.declared_version),
        }
    }

    /// Resolves the filesystem target directory component.
    pub fn target_directory(&self) -> String {
        format!("v{}", self.declared_version)
    }
}

impl fmt::Display for VersionIdentifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display_version())
    }
}

/// Escapes special HTML characters for safe attribute interpolation.
pub fn escape_html_attribute(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Generates a validated, escaped HTML redirect targeting `target_url`.
///
/// ```rust
/// use docversion_core::generate_redirect_html;
///
/// let markup = generate_redirect_html("latest/docversion/index.html").unwrap();
/// assert!(markup.contains("latest/docversion/index.html"));
/// ```
pub fn generate_redirect_html(target_url: &str) -> Result<String, CoreError> {
    let trimmed = target_url.trim();
    if trimmed.is_empty()
        || trimmed.contains("..")
        || trimmed.starts_with("//")
        || trimmed.starts_with('\\')
        || trimmed.contains(':')
        || trimmed.contains('\n')
        || trimmed.contains('\r')
    {
        return Err(CoreError::UnsafePath(target_url.to_string()));
    }

    let escaped = escape_html_attribute(trimmed);
    Ok(format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta http-equiv="refresh" content="0; url={escaped}">
  <link rel="canonical" href="{escaped}">
  <title>Redirecting to {escaped}</title>
</head>
<body>
  <p>Redirecting to <a href="{escaped}">{escaped}</a>...</p>
  <script>window.location.replace("{escaped}");</script>
</body>
</html>
"#
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_version_with_and_without_revision() {
        let v1 = VersionIdentifier::new("v0.2.0", Some("rev.abc1234")).unwrap();
        assert_eq!(v1.display_version(), "v0.2.0 (rev.abc1234)");
        assert_eq!(v1.target_directory(), "v0.2.0");

        let v2 = VersionIdentifier::new("1.0.0", None).unwrap();
        assert_eq!(v2.display_version(), "v1.0.0");
    }

    #[test]
    fn rejects_unsafe_redirect_targets() {
        assert!(generate_redirect_html("../etc/passwd").is_err());
        assert!(generate_redirect_html("javascript:alert(1)").is_err());
        assert!(generate_redirect_html("//evil.example.com/phish").is_err());
        assert!(generate_redirect_html("/latest/my_crate/index.html").is_ok());
        assert!(generate_redirect_html("latest/docversion/index.html").is_ok());
    }
}
