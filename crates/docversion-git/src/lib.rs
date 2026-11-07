use anyhow::{Context, Result};
use std::path::Path;

pub struct GitContext {
    pub tree_hash: String,
    pub is_dirty: bool,
}

impl GitContext {
    pub fn resolve<P: AsRef<Path>>(repo_path: P) -> Result<Self> {
        let repo = gix::discover(repo_path.as_ref()).context("Failed to locate Git repository")?;

        let mut head = repo.head().context("Failed to resolve HEAD")?;
        let head_commit = head
            .peel_to_commit_in_place()
            .context("Failed to peel HEAD to commit")?;
        let tree_hash = head_commit.id().to_hex_with_len(7).to_string();

        let is_dirty = check_is_dirty(&repo);

        Ok(Self {
            tree_hash,
            is_dirty,
        })
    }
}

/// Checks whether the Git working tree has uncommitted changes.
pub fn is_working_tree_dirty<P: AsRef<Path>>(repo_path: P) -> Result<bool> {
    let repo = gix::discover(repo_path.as_ref()).context("Failed to locate Git repository")?;
    Ok(check_is_dirty(&repo))
}

/// Computes the revision string for the specified paths (e.g. "rev.1a2b3c4" or "rev.1a2b3c4+dirty").
pub fn compute_doc_revision<P: AsRef<Path>>(repo_path: P, _paths: &[&str]) -> Result<String> {
    match GitContext::resolve(repo_path) {
        Ok(ctx) => {
            if ctx.is_dirty {
                Ok(format!("rev.{}+dirty", ctx.tree_hash))
            } else {
                Ok(format!("rev.{}", ctx.tree_hash))
            }
        }
        Err(_) => Ok("rev.clean".to_string()),
    }
}

fn check_is_dirty(repo: &gix::Repository) -> bool {
    let work_dir = match repo.workdir() {
        Some(w) => w,
        None => return false,
    };

    let index = match repo.index_or_empty() {
        Ok(idx) => idx,
        Err(_) => return false,
    };

    for entry in index.entries() {
        let path = match std::str::from_utf8(entry.path(&index)) {
            Ok(p) => work_dir.join(p),
            Err(_) => continue,
        };

        if let Ok(meta) = std::fs::metadata(&path) {
            let stat = entry.stat;
            if stat.size != 0 && stat.size != meta.len() as u32 {
                return true;
            }
        } else {
            return true;
        }
    }

    false
}
