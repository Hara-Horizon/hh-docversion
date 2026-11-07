use std::fmt;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CoreError {
    #[error("Invalid SemVer string: {0}")]
    InvalidVersion(String),
    #[error("Target path contains forbidden relative traversals: {0}")]
    UnsafePath(String),
}

impl From<semver::Error> for CoreError {
    fn from(err: semver::Error) -> Self {
        CoreError::InvalidVersion(err.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionIdentifier {
    pub declared_version: semver::Version,
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
        let declared_version = semver::Version::parse(declared.trim())?;
        let revision_suffix = revision
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());

        Ok(Self {
            declared_version,
            revision_suffix,
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

pub fn escape_html_attribute(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
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
    if target_url.contains("..") || target_url.starts_with('\\') {
        return Err(CoreError::UnsafePath(target_url.to_string()));
    }

    let safe_url = escape_html_attribute(target_url);
    Ok(format!(
        r#"<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="utf-8">
    <meta http-equiv="refresh" content="0; url={safe_url}">
    <link rel="canonical" href="{safe_url}">
    <title>Redirecting...</title>
  </head>
  <body>
    <p>Redirecting to <a href="{safe_url}">{safe_url}</a>...</p>
  </body>
</html>
"#
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_version_formatting() {
        let v = VersionIdentifier::new("1.4.2", Some("rev.8a4c1f0")).unwrap();
        assert_eq!(v.display_version(), "v1.4.2 (rev.8a4c1f0)");
        assert_eq!(v.target_directory(), "v1.4.2");
    }

    #[test]
    fn test_empty_revision_handling() {
        let v = VersionIdentifier::new("1.4.2", Some("   ")).unwrap();
        assert_eq!(v.display_version(), "v1.4.2");
        assert!(v.revision_suffix.is_none());
    }

    #[test]
    fn test_redirect_html_escaping() {
        let html = generate_redirect_html("latest/my_crate/index.html?param=1&other=2").unwrap();
        assert!(html.contains("param=1&amp;other=2"));
    }

    #[test]
    fn test_redirect_unsafe_path_rejection() {
        assert!(generate_redirect_html("../etc/passwd").is_err());
    }
}
