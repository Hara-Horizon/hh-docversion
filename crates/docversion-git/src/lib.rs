//! # Intent: In-process Git tree hash and revision resolver for hh-docversion.
//!
//! Resolves HEAD commit revisions, repository tags, and file-level tree hashes efficiently using in-process gitoxide (`gix`).
use anyhow::{Context, Result};
use std::path::Path;

pub struct GitContext {
    pub tree_hash: String,
    pub is_dirty: bool,
    pub latest_tag: Option<String>,
}

impl GitContext {
    pub fn resolve<P: AsRef<Path>>(repo_path: P) -> Result<Self> {
        let repo = gix::discover(repo_path.as_ref()).context("Failed to locate Git repository")?;

        let mut head = repo.head().context("Failed to resolve HEAD")?;
        let head_commit = head
            .peel_to_commit_in_place()
            .context("Failed to peel HEAD to commit")?;
        let head_id = head_commit.id();
        let tree_hash = head_id.to_hex_with_len(7).to_string();

        let is_dirty = check_is_dirty(&repo);
        let latest_tag = resolve_head_or_latest_tag(&repo, head_id);

        Ok(Self {
            tree_hash,
            is_dirty,
            latest_tag,
        })
    }
}

fn resolve_head_or_latest_tag(repo: &gix::Repository, head_id: gix::Id<'_>) -> Option<String> {
    let refs = repo.references().ok()?;
    let tags = refs.tags().ok()?;

    let mut head_tags = Vec::new();
    let mut all_tags = Vec::new();

    for mut tag_ref in tags.flatten() {
        let short_name = tag_ref.name().shorten().to_string();
        if let Ok(target) = tag_ref.peel_to_id_in_place() {
            if target == head_id {
                head_tags.push(short_name.clone());
            }
        }
        all_tags.push(short_name);
    }

    head_tags.sort();
    if let Some(exact) = head_tags.pop() {
        return Some(exact);
    }

    all_tags.sort();
    all_tags.pop()
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
    let work_dir = match repo.worktree() {
        Some(wt) => wt.base(),
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