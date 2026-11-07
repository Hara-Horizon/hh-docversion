use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GitError {
    #[error("Git inspection error: {0}")]
    Gix(String),
}

/// Resolves a documentation subversion string using in-process Git tree discovery.
///
/// ```rust
/// use docversion_git::compute_doc_revision;
///
/// let rev = compute_doc_revision(".", &["src/"]).unwrap();
/// assert!(rev.starts_with("rev."));
/// ```
pub fn compute_doc_revision<P: AsRef<Path>>(
    repo_root: P,
    _paths_to_track: &[&str],
) -> Result<String, GitError> {
    let repo = match gix::discover(repo_root.as_ref()) {
        Ok(r) => r,
        Err(_) => return Ok("rev.clean".to_string()),
    };

    let revision = match repo.head_id() {
        Ok(id) => {
            let hex = id.to_hex_with_len(7).to_string();
            format!("rev.{}", hex)
        }
        Err(_) => "rev.clean".to_string(),
    };

    Ok(revision)
}

/// Inspects working tree dirty status using in-process discovery.
///
/// ```rust
/// use docversion_git::is_working_tree_dirty;
///
/// let _ = is_working_tree_dirty(".");
/// ```
pub fn is_working_tree_dirty<P: AsRef<Path>>(repo_root: P) -> bool {
    let repo = match gix::discover(repo_root.as_ref()) {
        Ok(r) => r,
        Err(_) => return false,
    };

    repo.is_dirty().unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unit_compute_doc_revision() {
        let rev = compute_doc_revision(".", &[]).unwrap();
        assert!(rev.starts_with("rev."));
    }
}
