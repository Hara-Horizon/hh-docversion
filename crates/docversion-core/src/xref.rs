//! Workspace-wide cross-crate symbol index and intra-doc link resolver.

use std::collections::BTreeMap;

/// Kind of public Rust item indexed across workspace member crates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    Module,
    Struct,
    Enum,
    Trait,
    Function,
    Method,
    TypeAlias,
    Constant,
}

/// Canonical location of an indexed workspace symbol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolEntry {
    /// Cargo crate identifier using underscores (e.g., `docversion_core`).
    pub crate_ident: String,
    /// Full Rust path (e.g., `docversion_core::shortcut::sanitize_for_shortcut_docs`).
    pub full_path: String,
    /// Short item name (e.g., `sanitize_for_shortcut_docs` or `VersionIdentifier::new`).
    pub short_name: String,
    /// Item classification.
    pub kind: SymbolKind,
    /// Single-file Markdown heading slug (e.g., `#fn-sanitize_for_shortcut_docs`).
    pub markdown_anchor: String,
    /// Multi-file tracked `docs/` relative path (e.g., `docversion_core.md` or `docversion_core/shortcut.md`).
    pub doc_rel_path: String,
    /// Relative Rustdoc HTML path (e.g., `docversion_core/shortcut/fn.sanitize_for_shortcut_docs.html`).
    pub rustdoc_path: String,
}

/// Exact workspace symbol registry for cross-crate intra-doc link resolution.
#[derive(Debug, Default, Clone)]
pub struct WorkspaceSymbolIndex {
    by_full_path: BTreeMap<String, SymbolEntry>,
    by_short_name: BTreeMap<String, Vec<SymbolEntry>>,
}

impl WorkspaceSymbolIndex {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a public symbol in the workspace index.
    pub fn insert(&mut self, entry: SymbolEntry) {
        self.by_full_path
            .insert(entry.full_path.clone(), entry.clone());
        self.by_short_name
            .entry(entry.short_name.clone())
            .or_default()
            .push(entry);
    }

    /// Resolves a Rustdoc intra-doc target relative to `current_crate` and optional `current_type`.
    pub fn resolve(
        &self,
        raw_target: &str,
        current_crate: &str,
        current_type: Option<&str>,
    ) -> Option<&SymbolEntry> {
        let cleaned = raw_target
            .trim()
            .trim_matches('`')
            .trim_end_matches("()")
            .trim();

        if cleaned.is_empty()
            || cleaned.starts_with("http://")
            || cleaned.starts_with("https://")
            || cleaned.starts_with('#')
            || cleaned.starts_with("./")
            || cleaned.starts_with("../")
        {
            return None;
        }

        let normalized_path = normalize_crate_prefix(cleaned);

        // 1. Expand `crate::...` against current crate
        if let Some(rest) = normalized_path.strip_prefix("crate::") {
            let candidate = format!("{}::{}", current_crate, rest);
            if let Some(entry) = self.by_full_path.get(&candidate) {
                return Some(entry);
            }
        }

        // 2. Expand `Self::...` against current impl/struct type
        if let Some(rest) = normalized_path.strip_prefix("Self::") {
            if let Some(ty) = current_type {
                let qualified_method = format!("{}::{}", ty, rest);
                if let Some(entry) = self.resolve_short_or_suffix(&qualified_method, current_crate)
                {
                    return Some(entry);
                }
            }
        }

        // 3. Exact full path match
        if let Some(entry) = self.by_full_path.get(&normalized_path) {
            return Some(entry);
        }

        // 4. Suffix / short-name match
        self.resolve_short_or_suffix(&normalized_path, current_crate)
    }

    fn resolve_short_or_suffix(&self, needle: &str, current_crate: &str) -> Option<&SymbolEntry> {
        if let Some(candidates) = self.by_short_name.get(needle) {
            if let Some(same_crate) = candidates.iter().find(|e| e.crate_ident == current_crate) {
                return Some(same_crate);
            }
            if candidates.len() == 1 {
                return candidates.first();
            }
        }

        let suffix = format!("::{}", needle);
        let mut matches: Vec<&SymbolEntry> = self
            .by_full_path
            .values()
            .filter(|e| e.full_path.ends_with(&suffix))
            .collect();

        matches.sort_by(|a, b| a.full_path.cmp(&b.full_path));
        if let Some(same_crate) = matches.iter().find(|e| e.crate_ident == current_crate) {
            return Some(same_crate);
        }
        if matches.len() == 1 {
            return matches.first().copied();
        }

        None
    }

    /// Rewrites Rustdoc intra-doc links in single-file Markdown prose into `#anchor` links.
    pub fn resolve_markdown_links(
        &self,
        markdown: &str,
        current_crate: &str,
        current_type: Option<&str>,
    ) -> String {
        self.resolve_links_with_target(markdown, current_crate, current_type, None)
    }

    /// Rewrites Rustdoc intra-doc links in multi-file `docs/` exports into relative `.md#anchor` links
    /// calculated from `current_doc_path` (e.g., `"docversion_core/shortcut.md"`).
    pub fn resolve_multifile_markdown_links(
        &self,
        markdown: &str,
        current_crate: &str,
        current_type: Option<&str>,
        current_doc_path: &str,
    ) -> String {
        self.resolve_links_with_target(
            markdown,
            current_crate,
            current_type,
            Some(current_doc_path),
        )
    }

    fn resolve_links_with_target(
        &self,
        markdown: &str,
        current_crate: &str,
        current_type: Option<&str>,
        current_doc_path: Option<&str>,
    ) -> String {
        let mut out = String::with_capacity(markdown.len());
        let mut in_fence = false;
        let mut fence_marker: Option<&str> = None;

        for line in markdown.split_inclusive('\n') {
            let trimmed = line.trim_start();
            if let Some(marker) = detect_fence(trimmed) {
                if !in_fence {
                    in_fence = true;
                    fence_marker = Some(marker);
                } else if fence_marker == Some(marker) {
                    in_fence = false;
                    fence_marker = None;
                }
                out.push_str(line);
                continue;
            }

            if in_fence {
                out.push_str(line);
                continue;
            }

            out.push_str(&self.rewrite_line_intra_doc_links(
                line,
                current_crate,
                current_type,
                current_doc_path,
            ));
        }

        out
    }

    fn rewrite_line_intra_doc_links(
        &self,
        line: &str,
        current_crate: &str,
        current_type: Option<&str>,
        current_doc_path: Option<&str>,
    ) -> String {
        let mut out = String::with_capacity(line.len());
        let mut rest = line;

        while let Some(open_idx) = rest.find('[') {
            out.push_str(&rest[..open_idx]);
            let after_open = &rest[open_idx + 1..];

            if let Some(close_idx) = after_open.find(']') {
                let label = &after_open[..close_idx];
                let after_close = &after_open[close_idx + 1..];

                if let Some(after_paren) = after_close.strip_prefix('(') {
                    if let Some(end_paren) = after_paren.find(')') {
                        let target = &after_paren[..end_paren];
                        if let Some(entry) = self.resolve(target, current_crate, current_type) {
                            let href = format_entry_href(entry, current_doc_path);
                            out.push('[');
                            out.push_str(label);
                            out.push_str("](");
                            out.push_str(&href);
                            out.push(')');
                            rest = &after_paren[end_paren + 1..];
                            continue;
                        }
                    }
                }

                if !after_close.starts_with('(') && !after_close.starts_with('[') {
                    if let Some(entry) = self.resolve(label, current_crate, current_type) {
                        let href = format_entry_href(entry, current_doc_path);
                        out.push('[');
                        out.push_str(label);
                        out.push_str("](");
                        out.push_str(&href);
                        out.push(')');
                        rest = after_close;
                        continue;
                    }
                }
            }

            out.push('[');
            rest = after_open;
        }

        out.push_str(rest);
        out
    }
}

fn format_entry_href(entry: &SymbolEntry, current_doc_path: Option<&str>) -> String {
    match current_doc_path {
        None => entry.markdown_anchor.clone(),
        Some(from_path) if from_path == entry.doc_rel_path => {
            if entry.kind == SymbolKind::Module {
                "#".to_string()
            } else {
                entry.markdown_anchor.clone()
            }
        }
        Some(from_path) => {
            let rel_file = relative_doc_path(from_path, &entry.doc_rel_path);
            if entry.kind == SymbolKind::Module {
                rel_file
            } else {
                format!("{}{}", rel_file, entry.markdown_anchor)
            }
        }
    }
}

/// Computes a POSIX-style relative Markdown path from `from_file` to `to_file` inside `docs/`.
pub fn relative_doc_path(from_file: &str, to_file: &str) -> String {
    let from_parts: Vec<&str> = from_file.split('/').filter(|s| !s.is_empty()).collect();
    let to_parts: Vec<&str> = to_file.split('/').filter(|s| !s.is_empty()).collect();

    let from_dirs = if from_parts.len() > 1 {
        &from_parts[..from_parts.len() - 1]
    } else {
        &[][..]
    };
    let to_dirs = if to_parts.len() > 1 {
        &to_parts[..to_parts.len() - 1]
    } else {
        &[][..]
    };

    let mut common = 0;
    while common < from_dirs.len() && common < to_dirs.len() && from_dirs[common] == to_dirs[common]
    {
        common += 1;
    }

    let up_count = from_dirs.len() - common;
    let mut segments = Vec::new();
    if up_count == 0 {
        segments.push(".".to_string());
    } else {
        for _ in 0..up_count {
            segments.push("..".to_string());
        }
    }

    for dir in &to_dirs[common..] {
        segments.push((*dir).to_string());
    }
    if let Some(file_name) = to_parts.last() {
        segments.push((*file_name).to_string());
    }

    segments.join("/")
}

fn normalize_crate_prefix(path: &str) -> String {
    match path.split_once("::") {
        Some((head, tail)) => format!("{}::{}", head.replace('-', "_"), tail),
        None => path.to_string(),
    }
}

fn detect_fence(trimmed: &str) -> Option<&'static str> {
    if trimmed.starts_with("```") {
        Some("```")
    } else if trimmed.starts_with("~~~") {
        Some("~~~")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_index() -> WorkspaceSymbolIndex {
        let mut idx = WorkspaceSymbolIndex::new();
        idx.insert(SymbolEntry {
            crate_ident: "docversion_core".into(),
            full_path: "docversion_core::VersionIdentifier".into(),
            short_name: "VersionIdentifier".into(),
            kind: SymbolKind::Struct,
            markdown_anchor: "#struct-versionidentifier".into(),
            doc_rel_path: "docversion_core.md".into(),
            rustdoc_path: "docversion_core/struct.VersionIdentifier.html".into(),
        });
        idx.insert(SymbolEntry {
            crate_ident: "docversion_core".into(),
            full_path: "docversion_core::VersionIdentifier::new".into(),
            short_name: "VersionIdentifier::new".into(),
            kind: SymbolKind::Method,
            markdown_anchor: "#fn-new-impl-versionidentifier".into(),
            doc_rel_path: "docversion_core.md".into(),
            rustdoc_path: "docversion_core/struct.VersionIdentifier.html#method.new".into(),
        });
        idx.insert(SymbolEntry {
            crate_ident: "docversion_core".into(),
            full_path: "docversion_core::shortcut::sanitize_for_shortcut_docs".into(),
            short_name: "sanitize_for_shortcut_docs".into(),
            kind: SymbolKind::Function,
            markdown_anchor: "#fn-sanitize_for_shortcut_docs".into(),
            doc_rel_path: "docversion_core/shortcut.md".into(),
            rustdoc_path: "docversion_core/shortcut/fn.sanitize_for_shortcut_docs.html".into(),
        });
        idx
    }

    #[test]
    fn resolves_cross_crate_and_shorthand_intra_doc_links() {
        let idx = sample_index();
        let input = concat!(
            "Uses [`docversion_core::VersionIdentifier`] and [`VersionIdentifier::new`] ",
            "alongside [shortcut cleaner](docversion-core::shortcut::sanitize_for_shortcut_docs).\n",
            "Unknown [`Result`] remains untouched.\n"
        );

        let resolved = idx.resolve_markdown_links(input, "cargo_docversion", None);

        assert!(
            resolved.contains("[`docversion_core::VersionIdentifier`](#struct-versionidentifier)")
        );
        assert!(resolved.contains("[`VersionIdentifier::new`](#fn-new-impl-versionidentifier)"));
        assert!(resolved.contains("[shortcut cleaner](#fn-sanitize_for_shortcut_docs)"));
        assert!(resolved.contains("Unknown [`Result`] remains untouched."));
    }

    #[test]
    fn resolves_multifile_relative_markdown_paths() {
        let idx = sample_index();
        let text = "Calls [`VersionIdentifier::new`] and [`sanitize_for_shortcut_docs`].";

        let from_other_crate = idx.resolve_multifile_markdown_links(
            text,
            "cargo_docversion",
            None,
            "cargo_docversion.md",
        );
        assert!(from_other_crate.contains(
            "[`VersionIdentifier::new`](./docversion_core.md#fn-new-impl-versionidentifier)"
        ));
        assert!(from_other_crate.contains(
            "[`sanitize_for_shortcut_docs`](./docversion_core/shortcut.md#fn-sanitize_for_shortcut_docs)"
        ));

        let from_nested_mod = idx.resolve_multifile_markdown_links(
            text,
            "docversion_core",
            None,
            "docversion_core/shortcut.md",
        );
        assert!(from_nested_mod.contains(
            "[`VersionIdentifier::new`](../docversion_core.md#fn-new-impl-versionidentifier)"
        ));
        assert!(
            from_nested_mod
                .contains("[`sanitize_for_shortcut_docs`](#fn-sanitize_for_shortcut_docs)")
        );
    }
}
