//! # Intent: Universal version-aware documentation site bundler and DB schema matrix contract verifier.
//!
//! Exposes cargo plugin subcommands for building versioned HTML docs, exporting tracked Markdown trees,
//! and verifying database schema compatibility offline.
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};
use cargo_metadata::MetadataCommand;
use clap::{Args, Parser, Subcommand, ValueEnum};
use docversion_core::{
    generate_redirect_html, sanitize_for_shortcut_docs, SymbolEntry, SymbolKind, VersionIdentifier,
    WorkspaceSymbolIndex,
};
use docversion_git::{compute_doc_revision, GitContext};
use docversion_manifest::SchemaMatrix;
use serde::{Deserialize, Serialize};

use syn::visit::{self, Visit};
use syn::{
    ExprCall, ExprMacro, Fields, ImplItem, Item, ItemConst, ItemEnum, ItemFn, ItemImpl, ItemMod,
    ItemStruct, ItemTrait, ItemType, Visibility,
};

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct CrateMetadata {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub homepage: Option<String>,
    pub repository: Option<String>,
    pub documentation: Option<String>,
    pub keywords: Vec<String>,
    pub categories: Vec<String>,
    pub intent_summary: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct RepoMetadata {
    pub workspace_name: String,
    pub crates: Vec<CrateMetadata>,
}

#[derive(Args, Debug)]
pub struct RepoMetadataArgs {
    /// Output path for synthesized repo_metadata.json
    #[arg(long, default_value = "repo_metadata.json")]
    pub out: PathBuf,
}

#[derive(Args, Debug)]
pub struct GithubMetaArgs {
    /// Path to synthesized repo_metadata.json manifest
    #[arg(long, default_value = "repo_metadata.json")]
    pub manifest: PathBuf,
}

/// Context carried through Pass 2 Markdown rendering for exact cross-crate link resolution.
struct RenderContext<'a> {
    index: &'a WorkspaceSymbolIndex,
    current_crate: &'a str,
    current_type: Option<&'a str>,
    /// When `Some(rel_path)`, rewrites links as cross-file relative Markdown paths (`./foo.md#bar`).
    /// When `None`, rewrites links as single-file heading anchors (`#bar`).
    current_doc_path: Option<&'a str>,
}

impl<'a> RenderContext<'a> {
    fn resolve_text(&self, text: &str) -> String {
        match self.current_doc_path {
            Some(doc_path) => self.index.resolve_multifile_markdown_links(
                text,
                self.current_crate,
                self.current_type,
                doc_path,
            ),
            None => self
                .index
                .resolve_markdown_links(text, self.current_crate, self.current_type),
        }
    }

    fn with_type(&self, ty: &'a str) -> RenderContext<'a> {
        RenderContext {
            index: self.index,
            current_crate: self.current_crate,
            current_type: Some(ty),
            current_doc_path: self.current_doc_path,
        }
    }
}

/// Generates a GitHub/Shortcut-compatible heading anchor slug matching Markdown heading rules.
fn heading_slug(raw_heading: &str) -> String {
    let mut slug = String::with_capacity(raw_heading.len());
    for ch in raw_heading.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
        } else if ch == ' ' || ch == '-' || ch == '_' {
            slug.push('-');
        }
    }
    while slug.contains("--") {
        slug = slug.replace("--", "-");
    }
    format!("#{}", slug.trim_matches('-'))
}

/// Normalizes `quote!` token strings into clean, idiomatic code formatting.
fn normalize_token_spacing(raw: &str) -> String {
    let mut s = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    for (pat, rep) in [
        (" :: ", "::"),
        (":: ", "::"),
        (" ::", "::"),
        (" < ", "<"),
        ("< ", "<"),
        (" <", "<"),
        (" > ", "> "),
        (" >", ">"),
        (" ( ", "("),
        ("( ", "("),
        (" (", "("),
        (" ) ", ") "),
        (" )", ")"),
        (" [ ", "["),
        ("[ ", "["),
        (" [", "["),
        (" ]", "]"),
        (" ,", ","),
        (" ;", ";"),
        (" :", ":"),
        ("& ", "&"),
        ("->(", "-> ("),
        (":(", ": ("),
        ("fn(", "fn ("),
        ("for(", "for ("),
        ("if(", "if ("),
        ("while(", "while ("),
        ("match(", "match ("),
        ("in(", "in ("),
    ] {
        while s.contains(pat) {
            s = s.replace(pat, rep);
        }
    }
    s
}

/// Extracts doc comments (`///` or `//!`) from attributes.
fn extract_doc_comments(attrs: &[syn::Attribute]) -> String {
    let mut docs = Vec::new();
    for attr in attrs {
        if attr.path().is_ident("doc") {
            if let syn::Meta::NameValue(nv) = &attr.meta {
                if let syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Str(s),
                    ..
                }) = &nv.value
                {
                    docs.push(s.value().trim().to_string());
                }
            }
        }
    }
    docs.join("\n")
}

fn build_doc_rel_path(crate_ident: &str, mod_segments: &[String]) -> String {
    if mod_segments.is_empty() {
        format!("{}.md", crate_ident)
    } else {
        format!("{}/{}.md", crate_ident, mod_segments.join("/"))
    }
}

/// Pass 1: Collects public symbols across a single crate root into `WorkspaceSymbolIndex`.
fn collect_crate_symbols(
    crate_ident: &str,
    crate_root: &Path,
    index: &mut WorkspaceSymbolIndex,
) -> Result<()> {
    let src_dir = crate_root.join("src");
    let lib_rs = src_dir.join("lib.rs");
    let main_rs = src_dir.join("main.rs");

    let entry_file = if lib_rs.exists() {
        lib_rs
    } else if main_rs.exists() {
        main_rs
    } else {
        return Ok(());
    };

    collect_file_symbols(&entry_file, &src_dir, crate_ident, &[], index)
}

fn collect_file_symbols(
    file_path: &Path,
    base_dir: &Path,
    crate_ident: &str,
    mod_segments: &[String],
    index: &mut WorkspaceSymbolIndex,
) -> Result<()> {
    if !file_path.exists() {
        return Ok(());
    }

    let code = fs::read_to_string(file_path)?;
    let syntax = match syn::parse_file(&code) {
        Ok(s) => s,
        Err(_) => return Ok(()),
    };

    for item in &syntax.items {
        collect_item_symbol(item, base_dir, crate_ident, mod_segments, index)?;
    }

    Ok(())
}

fn build_qualified_path(crate_ident: &str, mod_segments: &[String], item_name: &str) -> String {
    if mod_segments.is_empty() {
        format!("{}::{}", crate_ident, item_name)
    } else {
        format!("{}::{}::{}", crate_ident, mod_segments.join("::"), item_name)
    }
}

fn build_rustdoc_prefix(crate_ident: &str, mod_segments: &[String]) -> String {
    if mod_segments.is_empty() {
        crate_ident.to_string()
    } else {
        format!("{}/{}", crate_ident, mod_segments.join("/"))
    }
}

fn collect_item_symbol(
    item: &Item,
    base_dir: &Path,
    crate_ident: &str,
    mod_segments: &[String],
    index: &mut WorkspaceSymbolIndex,
) -> Result<()> {
    let html_prefix = build_rustdoc_prefix(crate_ident, mod_segments);
    let doc_rel_path = build_doc_rel_path(crate_ident, mod_segments);

    match item {
        Item::Mod(ItemMod {
            vis: Visibility::Public(_),
            ident,
            content,
            ..
        }) => {
            let name = ident.to_string();
            let mut next_mods = mod_segments.to_vec();
            next_mods.push(name.clone());

            let full_path = build_qualified_path(crate_ident, mod_segments, &name);
            index.insert(SymbolEntry {
                crate_ident: crate_ident.to_string(),
                full_path,
                short_name: name.clone(),
                kind: SymbolKind::Module,
                markdown_anchor: heading_slug(&format!("Module {}", name)),
                doc_rel_path: build_doc_rel_path(crate_ident, &next_mods),
                rustdoc_path: format!("{}/{}/index.html", html_prefix, name),
            });

            if let Some((_, inline_items)) = content {
                for inner in inline_items {
                    collect_item_symbol(inner, base_dir, crate_ident, &next_mods, index)?;
                }
            } else {
                let mod_file_direct = base_dir.join(format!("{}.rs", name));
                let mod_file_dir = base_dir.join(&name).join("mod.rs");

                if mod_file_direct.exists() {
                    collect_file_symbols(
                        &mod_file_direct,
                        &base_dir.join(&name),
                        crate_ident,
                        &next_mods,
                        index,
                    )?;
                } else if mod_file_dir.exists() {
                    collect_file_symbols(
                        &mod_file_dir,
                        &base_dir.join(&name),
                        crate_ident,
                        &next_mods,
                        index,
                    )?;
                }
            }
        }
        Item::Struct(ItemStruct {
            vis: Visibility::Public(_),
            ident,
            ..
        }) => {
            let name = ident.to_string();
            index.insert(SymbolEntry {
                crate_ident: crate_ident.to_string(),
                full_path: build_qualified_path(crate_ident, mod_segments, &name),
                short_name: name.clone(),
                kind: SymbolKind::Struct,
                markdown_anchor: heading_slug(&format!("struct {}", name)),
                doc_rel_path,
                rustdoc_path: format!("{}/struct.{}.html", html_prefix, name),
            });
        }
        Item::Enum(ItemEnum {
            vis: Visibility::Public(_),
            ident,
            ..
        }) => {
            let name = ident.to_string();
            index.insert(SymbolEntry {
                crate_ident: crate_ident.to_string(),
                full_path: build_qualified_path(crate_ident, mod_segments, &name),
                short_name: name.clone(),
                kind: SymbolKind::Enum,
                markdown_anchor: heading_slug(&format!("enum {}", name)),
                doc_rel_path,
                rustdoc_path: format!("{}/enum.{}.html", html_prefix, name),
            });
        }
        Item::Trait(ItemTrait {
            vis: Visibility::Public(_),
            ident,
            ..
        }) => {
            let name = ident.to_string();
            index.insert(SymbolEntry {
                crate_ident: crate_ident.to_string(),
                full_path: build_qualified_path(crate_ident, mod_segments, &name),
                short_name: name.clone(),
                kind: SymbolKind::Trait,
                markdown_anchor: heading_slug(&format!("trait {}", name)),
                doc_rel_path,
                rustdoc_path: format!("{}/trait.{}.html", html_prefix, name),
            });
        }
        Item::Fn(ItemFn {
            vis: Visibility::Public(_),
            sig,
            ..
        }) => {
            let name = sig.ident.to_string();
            index.insert(SymbolEntry {
                crate_ident: crate_ident.to_string(),
                full_path: build_qualified_path(crate_ident, mod_segments, &name),
                short_name: name.clone(),
                kind: SymbolKind::Function,
                markdown_anchor: heading_slug(&format!("fn {}", name)),
                doc_rel_path,
                rustdoc_path: format!("{}/fn.{}.html", html_prefix, name),
            });
        }
        Item::Type(ItemType {
            vis: Visibility::Public(_),
            ident,
            ..
        }) => {
            let name = ident.to_string();
            index.insert(SymbolEntry {
                crate_ident: crate_ident.to_string(),
                full_path: build_qualified_path(crate_ident, mod_segments, &name),
                short_name: name.clone(),
                kind: SymbolKind::TypeAlias,
                markdown_anchor: heading_slug(&format!("type {}", name)),
                doc_rel_path,
                rustdoc_path: format!("{}/type.{}.html", html_prefix, name),
            });
        }
        Item::Const(ItemConst {
            vis: Visibility::Public(_),
            ident,
            ..
        }) => {
            let name = ident.to_string();
            index.insert(SymbolEntry {
                crate_ident: crate_ident.to_string(),
                full_path: build_qualified_path(crate_ident, mod_segments, &name),
                short_name: name.clone(),
                kind: SymbolKind::Constant,
                markdown_anchor: heading_slug(&format!("const {}", name)),
                doc_rel_path,
                rustdoc_path: format!("{}/constant.{}.html", html_prefix, name),
            });
        }
        Item::Impl(ItemImpl {
            items,
            self_ty,
            trait_,
            ..
        }) => {
            let type_name = normalize_token_spacing(&quote::quote!(#self_ty).to_string());
            let is_trait_impl = trait_.is_some();
            let impl_header = if let Some((_, tr, _)) = trait_ {
                let tr_name = normalize_token_spacing(&quote::quote!(#tr).to_string());
                format!("impl {} for {}", tr_name, type_name)
            } else {
                format!("impl {}", type_name)
            };

            for impl_item in items {
                if let ImplItem::Fn(m) = impl_item {
                    if is_trait_impl || matches!(m.vis, Visibility::Public(_)) {
                        let method_name = m.sig.ident.to_string();
                        let scoped_short = format!("{}::{}", type_name, method_name);
                        let full_path =
                            build_qualified_path(crate_ident, mod_segments, &scoped_short);
                        let heading_text = format!("fn {} ({})", method_name, impl_header);

                        index.insert(SymbolEntry {
                            crate_ident: crate_ident.to_string(),
                            full_path,
                            short_name: scoped_short,
                            kind: SymbolKind::Method,
                            markdown_anchor: heading_slug(&heading_text),
                            doc_rel_path: doc_rel_path.clone(),
                            rustdoc_path: format!(
                                "{}/struct.{}.html#method.{}",
                                html_prefix, type_name, method_name
                            ),
                        });
                    }
                }
            }
        }
        _ => {}
    }

    Ok(())
}

/// Scans a single crate root (`src/lib.rs` or `src/main.rs`) and appends its public API markdown.
fn scan_crate_root(
    crate_name: &str,
    crate_root: &Path,
    out: &mut String,
    crate_heading_level: usize,
    ctx: &RenderContext<'_>,
) -> Result<()> {
    let hashes = "#".repeat(crate_heading_level.min(6));
    out.push_str(&format!("---\n\n{} Crate `{}`\n\n", hashes, crate_name));

    let src_dir = crate_root.join("src");
    let lib_rs = src_dir.join("lib.rs");
    let main_rs = src_dir.join("main.rs");

    let entry_file = if lib_rs.exists() {
        lib_rs
    } else if main_rs.exists() {
        main_rs
    } else {
        return Ok(());
    };

    scan_file(&entry_file, &src_dir, out, crate_heading_level + 1, ctx)
}

/// Two-pass workspace scanner:
/// 1. Indexes all public symbols across workspace crates into `WorkspaceSymbolIndex`.
/// 2. Renders public interfaces, doctests, multi-language files, and handwritten `docs/*.md`
///    guides with resolved cross-crate intra-doc links.
pub fn generate_markdown_summary(workspace_title: &str, crate_root: &Path) -> Result<String> {
    let mut symbol_index = WorkspaceSymbolIndex::new();
    let mut workspace_crates: Vec<(String, String, PathBuf)> = Vec::new();

    let manifest_path = crate_root.join("Cargo.toml");
    if manifest_path.exists() {
        if let Ok(metadata) = MetadataCommand::new()
            .manifest_path(&manifest_path)
            .no_deps()
            .exec()
        {
            let mut packages = metadata.workspace_packages();
            packages.sort_by(|a, b| a.name.cmp(&b.name));

            for pkg in packages {
                if let Some(pkg_dir) = pkg.manifest_path.parent() {
                    let crate_ident = pkg.name.replace('-', "_");
                    let pkg_path = pkg_dir.as_std_path().to_path_buf();
                    collect_crate_symbols(&crate_ident, &pkg_path, &mut symbol_index)?;
                    workspace_crates.push((pkg.name.clone(), crate_ident, pkg_path));
                }
            }
        }
    }

    if workspace_crates.is_empty() {
        let fallback_ident = workspace_title.replace('-', "_");
        collect_crate_symbols(&fallback_ident, crate_root, &mut symbol_index)?;
    }

    let mut out = String::new();
    out.push_str(&format!("# {}\n\n", workspace_title));
    out.push_str("> Autonomous Agent API Context & Single-File Documentation Index\n\n");

    if !workspace_crates.is_empty() {
        for (crate_name, crate_ident, pkg_dir) in &workspace_crates {
            let ctx = RenderContext {
                index: &symbol_index,
                current_crate: crate_ident,
                current_type: None,
                current_doc_path: None,
            };
            scan_crate_root(crate_name, pkg_dir, &mut out, 2, &ctx)?;
        }
    } else {
        let fallback_ident = workspace_title.replace('-', "_");
        let ctx = RenderContext {
            index: &symbol_index,
            current_crate: &fallback_ident,
            current_type: None,
            current_doc_path: None,
        };
        let src_dir = crate_root.join("src");
        let lib_rs = src_dir.join("lib.rs");
        let main_rs = src_dir.join("main.rs");

        if lib_rs.exists() {
            scan_file(&lib_rs, &src_dir, &mut out, 2, &ctx)?;
        } else if main_rs.exists() {
            scan_file(&main_rs, &src_dir, &mut out, 2, &ctx)?;
        }
    }

    // Universal multi-language source scanner (.ts, .tsx, .js, .py, .go, .sql)
    scan_universal_sources(crate_root, crate_root, &mut out)?;

    // Handwritten Markdown guides in docs/ (skipping generated export-md artifacts)
    let docs_dir = crate_root.join("docs");
    if docs_dir.is_dir() {
        let default_crate = workspace_crates
            .first()
            .map(|(_, ident, _)| ident.as_str())
            .unwrap_or("docversion_core");
        let ctx = RenderContext {
            index: &symbol_index,
            current_crate: default_crate,
            current_type: None,
            current_doc_path: None,
        };
        scan_markdown_docs(&docs_dir, crate_root, &mut out, &ctx)?;
    }

    Ok(out)
}

pub fn export_tracked_docs_tree(workspace_root: &Path, out_dir: &Path) -> Result<()> {
    let manifest_path = workspace_root.join("Cargo.toml");
    let metadata = MetadataCommand::new()
        .manifest_path(&manifest_path)
        .no_deps()
        .exec()
        .context("Failed to resolve workspace metadata for export-md")?;

    let root_pkg = metadata
        .root_package()
        .or_else(|| metadata.workspace_packages().first().copied())
        .context("No packages found in workspace")?;

    let workspace_title = root_pkg.name.clone();
    let workspace_ver = root_pkg.version.to_string();

    let mut packages = metadata.workspace_packages();
    packages.sort_by(|a, b| a.name.cmp(&b.name));

    let mut symbol_index = WorkspaceSymbolIndex::new();
    let mut crate_specs: Vec<(String, String, String, PathBuf)> = Vec::new();

    for pkg in &packages {
        if let Some(pkg_dir) = pkg.manifest_path.parent() {
            let crate_ident = pkg.name.replace('-', "_");
            let pkg_path = pkg_dir.as_std_path().to_path_buf();
            collect_crate_symbols(&crate_ident, &pkg_path, &mut symbol_index)?;
            crate_specs.push((
                pkg.name.clone(),
                crate_ident,
                pkg.version.to_string(),
                pkg_path,
            ));
        }
    }

    let git_ctx = GitContext::resolve(workspace_root).ok();
    let git_rev = compute_doc_revision(workspace_root, &["src/", "Cargo.toml", "README.md"])
        .unwrap_or_else(|_| "rev.clean".to_string());
    let tag_display = git_ctx
        .as_ref()
        .and_then(|c| c.latest_tag.clone())
        .unwrap_or_else(|| format!("v{}", workspace_ver));

    fs::create_dir_all(out_dir)?;

    let mut readme = String::new();
    readme.push_str(&format!("# {} — Documentation Index\n\n", workspace_title));
    readme.push_str(&format!(
        "> **Version:** `v{}` · **Tag:** `{}` · **Revision:** `{}`\n\n",
        workspace_ver, tag_display, git_rev
    ));
    readme.push_str(
        "Tracked workspace documentation generated by `cargo hh-docversion export-md`.\n\n",
    );
    readme.push_str("## Workspace Crates & Modules\n\n");
    readme.push_str("| Crate | Version | Reference | Submodules |\n");
    readme.push_str("| :--- | :--- | :--- | :--- |\n");

    let mut generated_files = BTreeMap::new();

    for (crate_name, crate_ident, pkg_ver, pkg_dir) in &crate_specs {
        let mut module_links = Vec::new();
        let crate_doc_rel = format!("{}.md", crate_ident);
        let ctx = RenderContext {
            index: &symbol_index,
            current_crate: crate_ident,
            current_type: None,
            current_doc_path: Some(&crate_doc_rel),
        };

        let mut crate_md = String::new();
        crate_md.push_str(&format!("# Crate `{}` (`v{}`)\n\n", crate_name, pkg_ver));
        crate_md.push_str("[← Back to Workspace Index](./README.md) · [Full API Reference](./api-reference.md)\n\n");

        let src_dir = pkg_dir.join("src");
        let lib_rs = src_dir.join("lib.rs");
        let main_rs = src_dir.join("main.rs");
        let entry_file = if lib_rs.exists() {
            Some(lib_rs)
        } else if main_rs.exists() {
            Some(main_rs)
        } else {
            None
        };

        if let Some(entry) = entry_file {
            render_multifile_crate(
                &entry,
                &src_dir,
                crate_ident,
                &[],
                &mut crate_md,
                2,
                &ctx,
                &symbol_index,
                &mut generated_files,
                &mut module_links,
            )?;
        }

        generated_files.insert(crate_doc_rel.clone(), sanitize_for_shortcut_docs(&crate_md));

        let mods_cell = if module_links.is_empty() {
            "—".to_string()
        } else {
            module_links.join(", ")
        };
        readme.push_str(&format!(
            "| `{}` | `v{}` | [`{}`](./{}) | {} |\n",
            crate_name, pkg_ver, crate_doc_rel, crate_doc_rel, mods_cell
        ));
    }

    readme.push_str("\n## Single-File Reference\n\n");
    readme.push_str(
        "- [`api-reference.md`](./api-reference.md) — Complete single-file workspace API reference and invariant manifest.\n",
    );

    let raw_api_ref = generate_markdown_summary(&workspace_title, workspace_root)?;
    let clean_api_ref = sanitize_for_shortcut_docs(&raw_api_ref);
    atomic_write(out_dir.join("api-reference.md"), clean_api_ref.as_bytes())?;

    for (rel_path, content) in generated_files {
        let full_out = out_dir.join(&rel_path);
        if let Some(parent) = full_out.parent() {
            fs::create_dir_all(parent)?;
        }
        atomic_write(full_out, content.as_bytes())?;
    }

    let clean_readme = sanitize_for_shortcut_docs(&readme);
    atomic_write(out_dir.join("README.md"), clean_readme.as_bytes())?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn render_multifile_crate(
    file_path: &Path,
    base_dir: &Path,
    crate_ident: &str,
    mod_segments: &[String],
    out: &mut String,
    heading_level: usize,
    ctx: &RenderContext<'_>,
    symbol_index: &WorkspaceSymbolIndex,
    generated_files: &mut BTreeMap<String, String>,
    module_links: &mut Vec<String>,
) -> Result<()> {
    if !file_path.exists() {
        return Ok(());
    }

    let code = fs::read_to_string(file_path)?;
    let syntax = match syn::parse_file(&code) {
        Ok(s) => s,
        Err(_) => return Ok(()),
    };

    let file_docs = extract_doc_comments(&syntax.attrs);
    if !file_docs.is_empty() {
        let contract = parse_intent_contract(&file_docs);
        format_intent_markdown(out, &contract, heading_level, ctx);
    }

    for item in &syntax.items {
        if let Item::Mod(ItemMod {
            vis: Visibility::Public(_),
            ident,
            content,
            attrs,
            ..
        }) = item
        {
            let mod_name = ident.to_string();
            let mut next_mods = mod_segments.to_vec();
            next_mods.push(mod_name.clone());
            let mod_doc_rel = build_doc_rel_path(crate_ident, &next_mods);

            module_links.push(format!("[`{}`](./{})", next_mods.join("::"), mod_doc_rel));

            // Link to submodule file from parent crate/module page
            let hashes = "#".repeat(heading_level.min(6));
            let rel_from_parent = ctx
                .current_doc_path
                .map(|from| docversion_core::xref::relative_doc_path(from, &mod_doc_rel))
                .unwrap_or_else(|| format!("./{}", mod_doc_rel));
            out.push_str(&format!(
                "{} Module [`{}`]({})\n\n",
                hashes, mod_name, rel_from_parent
            ));

            let mod_docs = extract_doc_comments(attrs);
            if !mod_docs.is_empty() {
                let contract = parse_intent_contract(&mod_docs);
                format_intent_markdown(out, &contract, heading_level, ctx);
            }

            // Render dedicated per-module Markdown file
            let mod_ctx = RenderContext {
                index: symbol_index,
                current_crate: crate_ident,
                current_type: None,
                current_doc_path: Some(&mod_doc_rel),
            };

            let back_to_index = docversion_core::xref::relative_doc_path(&mod_doc_rel, "README.md");
            let back_to_crate = docversion_core::xref::relative_doc_path(
                &mod_doc_rel,
                &format!("{}.md", crate_ident),
            );

            let mut mod_md = String::new();
            mod_md.push_str(&format!(
                "# Module `{}::{}`\n\n",
                crate_ident,
                next_mods.join("::")
            ));
            mod_md.push_str(&format!(
                "[← Workspace Index]({}) · [Crate `{}`]({})\n\n",
                back_to_index, crate_ident, back_to_crate
            ));

            if !mod_docs.is_empty() {
                let contract = parse_intent_contract(&mod_docs);
                format_intent_markdown(&mut mod_md, &contract, 2, &mod_ctx);
            }

            if let Some((_, inline_items)) = content {
                for inner in inline_items {
                    process_item(inner, base_dir, &mut mod_md, 2, &mod_ctx)?;
                }
            } else {
                let mod_file_direct = base_dir.join(format!("{}.rs", mod_name));
                let mod_file_dir = base_dir.join(&mod_name).join("mod.rs");
                if mod_file_direct.exists() {
                    render_multifile_crate(
                        &mod_file_direct,
                        &base_dir.join(&mod_name),
                        crate_ident,
                        &next_mods,
                        &mut mod_md,
                        2,
                        &mod_ctx,
                        symbol_index,
                        generated_files,
                        module_links,
                    )?;
                } else if mod_file_dir.exists() {
                    render_multifile_crate(
                        &mod_file_dir,
                        &base_dir.join(&mod_name),
                        crate_ident,
                        &next_mods,
                        &mut mod_md,
                        2,
                        &mod_ctx,
                        symbol_index,
                        generated_files,
                        module_links,
                    )?;
                }
            }

            generated_files.insert(mod_doc_rel, sanitize_for_shortcut_docs(&mod_md));
        } else {
            process_item(item, base_dir, out, heading_level, ctx)?;
        }
    }

    Ok(())
}

/// Walks `root` for non-Rust source files and extracts public interfaces + docstrings.
fn scan_universal_sources(dir: &Path, workspace_root: &Path, out: &mut String) -> Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    let mut entries: Vec<_> = fs::read_dir(dir)?.filter_map(|e| e.ok()).collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        if path.is_dir() {
            if matches!(
                name_str.as_ref(),
                "target" | "node_modules" | ".git" | ".venv" | "venv" | "__pycache__" | "dist"
            ) {
                continue;
            }
            scan_universal_sources(&path, workspace_root, out)?;
        } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            let lang = match ext {
                "ts" | "tsx" => Some("typescript"),
                "js" | "jsx" => Some("javascript"),
                "py" => Some("python"),
                "go" => Some("go"),
                "sql" => Some("sql"),
                _ => None,
            };
            if let Some(lang_tag) = lang {
                let rel = path.strip_prefix(workspace_root).unwrap_or(&path);
                let content = fs::read_to_string(&path).unwrap_or_default();
                let decls = extract_universal_declarations(&content, lang_tag);
                if !decls.is_empty() {
                    out.push_str(&format!("---\n\n## File `{}`\n\n", rel.display()));
                    for (doc, sig) in decls {
                        if !doc.is_empty() {
                            out.push_str(&format!("{}\n\n", doc));
                        }
                        out.push_str(&format!("~~~{}\n{}\n~~~\n\n", lang_tag, sig));
                    }
                }
            }
        }
    }
    Ok(())
}

/// Extracts top-level exported/public declarations and preceding docstrings across TS/JS, Python, Go, and SQL.
fn extract_universal_declarations(content: &str, lang: &str) -> Vec<(String, String)> {
    let mut results = Vec::new();
    let lines: Vec<&str> = content.lines().collect();

    for (idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        let is_decl = match lang {
            "typescript" | "javascript" => {
                trimmed.starts_with("export function ")
                    || trimmed.starts_with("export async function ")
                    || trimmed.starts_with("export interface ")
                    || trimmed.starts_with("export type ")
                    || trimmed.starts_with("export class ")
                    || trimmed.starts_with("export enum ")
            }
            "python" => {
                (line.starts_with("def ")
                    || line.starts_with("async def ")
                    || line.starts_with("class "))
                    && !trimmed.starts_with("def _")
                    && !trimmed.starts_with("async def _")
            }
            "go" => {
                (trimmed.starts_with("func ") || trimmed.starts_with("type "))
                    && is_go_exported_decl(trimmed)
            }
            "sql" => {
                let upper = trimmed.to_ascii_uppercase();
                upper.starts_with("CREATE TABLE ")
                    || upper.starts_with("CREATE VIEW ")
                    || upper.starts_with("CREATE INDEX ")
            }
            _ => false,
        };

        if is_decl {
            let mut doc_lines = Vec::new();
            let mut back = idx;
            while back > 0 {
                back -= 1;
                let prev = lines[back].trim();
                if let Some(comment) = prev
                    .strip_prefix("///")
                    .or_else(|| prev.strip_prefix("//"))
                    .or_else(|| prev.strip_prefix('#'))
                    .or_else(|| prev.strip_prefix("--"))
                    .or_else(|| prev.strip_prefix('*'))
                {
                    let cleaned = comment.trim_end_matches("*/").trim();
                    if !cleaned.is_empty() {
                        doc_lines.push(cleaned.to_string());
                    }
                } else if prev == "/**" || prev == "/*" {
                    continue;
                } else {
                    break;
                }
            }
            doc_lines.reverse();

            let sig = if lang == "sql" {
                let mut stmt = Vec::new();
                for future_line in &lines[idx..] {
                    stmt.push(future_line.trim_end());
                    if future_line.contains(';') {
                        break;
                    }
                }
                stmt.join("\n")
            } else {
                trimmed.trim_end_matches('{').trim().to_string()
            };

            results.push((doc_lines.join(" "), sig));
        }
    }

    results
}

fn is_go_exported_decl(trimmed: &str) -> bool {
    let after_kw = trimmed
        .strip_prefix("func ")
        .or_else(|| trimmed.strip_prefix("type "))
        .unwrap_or("");
    let ident_part = if let Some(after_recv) = after_kw.strip_prefix('(') {
        after_recv
            .split_once(')')
            .map(|(_, rest)| rest.trim_start())
            .unwrap_or(after_kw)
    } else {
        after_kw
    };
    ident_part.chars().next().is_some_and(|c| c.is_ascii_uppercase())
}

/// Appends authored Markdown guides from `docs/` (ignoring generated `export-md` outputs).
fn scan_markdown_docs(
    dir: &Path,
    workspace_root: &Path,
    out: &mut String,
    ctx: &RenderContext<'_>,
) -> Result<()> {
    let mut entries: Vec<_> = fs::read_dir(dir)?.filter_map(|e| e.ok()).collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            scan_markdown_docs(&path, workspace_root, out, ctx)?;
        } else if path.extension().is_some_and(|ext| ext == "md") {
            let content = fs::read_to_string(&path).unwrap_or_default();
            if content.contains("Tracked workspace documentation generated by `cargo hh-docversion export-md`")
                || content.contains("[← Back to Workspace Index](./README.md)")
                || content.contains("[← Workspace Index](")
                || path.file_name().is_some_and(|n| n == "api-reference.md")
            {
                continue;
            }
            let rel = path.strip_prefix(workspace_root).unwrap_or(&path);
            if !content.trim().is_empty() {
                let resolved = ctx.resolve_text(content.trim());
                out.push_str(&format!("---\n\n## Guide `{}`\n\n", rel.display()));
                out.push_str(&resolved);
                out.push_str("\n\n");
            }
        }
    }
    Ok(())
}

fn scan_file(
    file_path: &Path,
    base_dir: &Path,
    out: &mut String,
    heading_level: usize,
    ctx: &RenderContext<'_>,
) -> Result<()> {
    if !file_path.exists() {
        return Ok(());
    }

    let code = fs::read_to_string(file_path)?;
    let syntax = match syn::parse_file(&code) {
        Ok(s) => s,
        Err(_) => return Ok(()),
    };

    let file_docs = extract_doc_comments(&syntax.attrs);
    if !file_docs.is_empty() {
        let contract = parse_intent_contract(&file_docs);
        format_intent_markdown(out, &contract, heading_level, ctx);
    }

    for item in syntax.items {
        process_item(&item, base_dir, out, heading_level, ctx)?;
    }

    Ok(())
}

#[derive(Debug, Clone)]
pub enum InvariantKind {
    Assert(String),
    ErrorRejection,
}

/// Extracted runtime invariant check, assertion, or error rejection boundary.
#[derive(Debug, Clone)]
pub struct ExtractedAssertion {
    pub kind: InvariantKind,
    pub condition: String,
}

pub struct AssertionVisitor {
    pub assertions: Vec<ExtractedAssertion>,
}

impl Default for AssertionVisitor {
    fn default() -> Self {
        Self::new()
    }
}

impl AssertionVisitor {
    pub fn new() -> Self {
        Self {
            assertions: Vec::new(),
        }
    }
}

impl<'ast> Visit<'ast> for AssertionVisitor {
    fn visit_expr_macro(&mut self, node: &'ast ExprMacro) {
        if let Some(ident) = node.mac.path.get_ident() {
            let name = ident.to_string();
            if matches!(
                name.as_str(),
                "assert" | "assert_eq" | "assert_ne" | "ensure" | "debug_assert"
            ) {
                let tokens = node.mac.tokens.to_string();
                let clean = normalize_token_spacing(&tokens);
                if !self
                    .assertions
                    .iter()
                    .any(|a| matches!(&a.kind, InvariantKind::Assert(k) if k == &name) && a.condition == clean)
                {
                    self.assertions.push(ExtractedAssertion {
                        kind: InvariantKind::Assert(name),
                        condition: clean,
                    });
                }
            }
        }
        visit::visit_expr_macro(self, node);
    }

    fn visit_expr_call(&mut self, node: &'ast ExprCall) {
        if let Some(err_desc) = match_err_call(node) {
            if !self
                .assertions
                .iter()
                .any(|a| matches!(a.kind, InvariantKind::ErrorRejection) && a.condition == err_desc)
            {
                self.assertions.push(ExtractedAssertion {
                    kind: InvariantKind::ErrorRejection,
                    condition: err_desc,
                });
            }
        }
        visit::visit_expr_call(self, node);
    }
}

fn match_err_call(call: &ExprCall) -> Option<String> {
    if let syn::Expr::Path(p) = &*call.func {
        if let Some(seg) = p.path.segments.last() {
            if seg.ident == "Err" {
                let tokens = quote::quote!(#call).to_string();
                return Some(normalize_token_spacing(&tokens));
            }
        }
    }
    None
}

/// Scans an AST block for runtime assertions and invariant guards.
fn extract_block_assertions(block: &syn::Block) -> Vec<ExtractedAssertion> {
    let mut visitor = AssertionVisitor::new();
    visitor.visit_block(block);
    visitor.assertions
}

pub fn format_assertions_markdown(
    out: &mut String,
    assertions: &[ExtractedAssertion],
    heading_level: usize,
) {
    if assertions.is_empty() {
        return;
    }

    let subhashes = "#".repeat((heading_level + 1).min(6));
    out.push_str(&format!(
        "{} **Programmatic Invariants & Rejection Boundaries**\n\n",
        subhashes
    ));

    for a in assertions {
        match &a.kind {
            InvariantKind::Assert(name) => {
                out.push_str(&format!("* 🔒 `{}`: `{}`\n", name, a.condition));
            }
            InvariantKind::ErrorRejection => {
                out.push_str(&format!("* 🚫 Reject: `{}`\n", a.condition));
            }
        }
    }
    out.push('\n');
}

fn process_item(
    item: &Item,
    base_dir: &Path,
    out: &mut String,
    level: usize,
    ctx: &RenderContext<'_>,
) -> Result<()> {
    let hashes = "#".repeat(level.min(6));

    match item {
        Item::Mod(ItemMod {
            vis: Visibility::Public(_),
            ident,
            content,
            attrs,
            ..
        }) => {
            out.push_str(&format!("{} Module `{}`\n\n", hashes, ident));
            let docs = extract_doc_comments(attrs);
            if !docs.is_empty() {
                let contract = parse_intent_contract(&docs);
                format_intent_markdown(out, &contract, level, ctx);
            }

            if let Some((_, inline_items)) = content {
                for inner in inline_items {
                    process_item(inner, base_dir, out, level + 1, ctx)?;
                }
            } else {
                let mod_file_direct = base_dir.join(format!("{}.rs", ident));
                let mod_file_dir = base_dir.join(ident.to_string()).join("mod.rs");

                if mod_file_direct.exists() {
                    scan_file(
                        &mod_file_direct,
                        &base_dir.join(ident.to_string()),
                        out,
                        level + 1,
                        ctx,
                    )?;
                } else if mod_file_dir.exists() {
                    scan_file(
                        &mod_file_dir,
                        &base_dir.join(ident.to_string()),
                        out,
                        level + 1,
                        ctx,
                    )?;
                }
            }
        }
        Item::Const(ItemConst {
            vis: Visibility::Public(_),
            ident,
            ty,
            expr,
            attrs,
            ..
        }) => {
            out.push_str(&format!("{} `const {}`\n\n", hashes, ident));
            let docs = extract_doc_comments(attrs);
            if !docs.is_empty() {
                let contract = parse_intent_contract(&docs);
                format_intent_markdown(out, &contract, level, ctx);
            }
            let ty_str = normalize_token_spacing(&quote::quote!(#ty).to_string());
            let expr_str = normalize_token_spacing(&quote::quote!(#expr).to_string());
            out.push_str(&format!(
                "~~~rust\npub const {}: {} = {};\n~~~\n\n",
                ident, ty_str, expr_str
            ));
        }
        Item::Type(ItemType {
            vis: Visibility::Public(_),
            ident,
            generics,
            ty,
            attrs,
            ..
        }) => {
            out.push_str(&format!("{} `type {}`\n\n", hashes, ident));
            let docs = extract_doc_comments(attrs);
            if !docs.is_empty() {
                let contract = parse_intent_contract(&docs);
                format_intent_markdown(out, &contract, level, ctx);
            }
            let gen_str = normalize_token_spacing(&quote::quote!(#generics).to_string());
            let ty_str = normalize_token_spacing(&quote::quote!(#ty).to_string());
            out.push_str(&format!(
                "~~~rust\npub type {}{} = {};\n~~~\n\n",
                ident, gen_str, ty_str
            ));
        }
        Item::Struct(ItemStruct {
            vis: Visibility::Public(_),
            ident,
            fields,
            attrs,
            ..
        }) => {
            let struct_name = ident.to_string();
            let scoped_ctx = ctx.with_type(&struct_name);
            out.push_str(&format!("{} `struct {}`\n\n", hashes, ident));
            let docs = extract_doc_comments(attrs);
            if !docs.is_empty() {
                let contract = parse_intent_contract(&docs);
                format_intent_markdown(out, &contract, level, &scoped_ctx);
            }

            match fields {
                Fields::Named(named) => {
                    out.push_str("**Fields:**\n\n");
                    for f in &named.named {
                        if let Visibility::Public(_) = f.vis {
                            let f_name =
                                f.ident.as_ref().map(|i| i.to_string()).unwrap_or_default();
                            let f_ty_raw = &f.ty;
                            let f_ty = normalize_token_spacing(&quote::quote!(#f_ty_raw).to_string());
                            let f_doc = extract_doc_comments(&f.attrs);
                            if f_doc.is_empty() {
                                out.push_str(&format!("* `{}: {}`\n", f_name, f_ty));
                            } else {
                                let resolved_doc = scoped_ctx.resolve_text(&f_doc);
                                out.push_str(&format!(
                                    "* `{}: {}` - {}\n",
                                    f_name, f_ty, resolved_doc
                                ));
                            }
                        }
                    }
                    out.push('\n');
                }
                Fields::Unnamed(unnamed) => {
                    let tuple_types: Vec<_> = unnamed
                        .unnamed
                        .iter()
                        .map(|f| {
                            let ty = &f.ty;
                            normalize_token_spacing(&quote::quote!(#ty).to_string())
                        })
                        .collect();
                    out.push_str(&format!("Tuple Struct: `({})`\n\n", tuple_types.join(", ")));
                }
                Fields::Unit => {}
            }
        }
        Item::Enum(ItemEnum {
            vis: Visibility::Public(_),
            ident,
            variants,
            attrs,
            ..
        }) => {
            let enum_name = ident.to_string();
            let scoped_ctx = ctx.with_type(&enum_name);
            out.push_str(&format!("{} `enum {}`\n\n", hashes, ident));
            let docs = extract_doc_comments(attrs);
            if !docs.is_empty() {
                let contract = parse_intent_contract(&docs);
                format_intent_markdown(out, &contract, level, &scoped_ctx);
            }

            out.push_str("**Variants:**\n\n");
            for v in variants {
                let v_doc = extract_doc_comments(&v.attrs);
                let v_name = &v.ident;
                if v_doc.is_empty() {
                    out.push_str(&format!("* `{}`\n", v_name));
                } else {
                    let resolved_doc = scoped_ctx.resolve_text(&v_doc);
                    out.push_str(&format!("* `{}` - {}\n", v_name, resolved_doc));
                }
            }
            out.push('\n');
        }
        Item::Trait(ItemTrait {
            vis: Visibility::Public(_),
            ident,
            items,
            attrs,
            ..
        }) => {
            let trait_name = ident.to_string();
            let scoped_ctx = ctx.with_type(&trait_name);
            out.push_str(&format!("{} `trait {}`\n\n", hashes, ident));
            let docs = extract_doc_comments(attrs);
            if !docs.is_empty() {
                let contract = parse_intent_contract(&docs);
                format_intent_markdown(out, &contract, level, &scoped_ctx);
            }

            out.push_str("**Methods:**\n\n");
            for ti in items {
                if let syn::TraitItem::Fn(m) = ti {
                    let m_sig = &m.sig;
                    let sig = normalize_token_spacing(&quote::quote!(#m_sig).to_string());
                    let m_docs = extract_doc_comments(&m.attrs);
                    if m_docs.is_empty() {
                        out.push_str(&format!("* `{}`\n", sig));
                    } else {
                        let resolved_doc = scoped_ctx.resolve_text(&m_docs);
                        out.push_str(&format!("* `{}` - {}\n", sig, resolved_doc));
                    }
                }
            }
            out.push('\n');
        }
        Item::Fn(ItemFn {
            vis: Visibility::Public(_),
            sig,
            attrs,
            block,
            ..
        }) => {
            out.push_str(&format!("{} `fn {}`\n\n", hashes, sig.ident));
            let docs = extract_doc_comments(attrs);
            if !docs.is_empty() {
                let contract = parse_intent_contract(&docs);
                format_intent_markdown(out, &contract, level, ctx);
            }
            let clean_sig = normalize_token_spacing(&quote::quote!(#sig).to_string());
            out.push_str(&format!("~~~rust\npub {}\n~~~\n\n", clean_sig));

            let assertions = extract_block_assertions(block);
            format_assertions_markdown(out, &assertions, level);
        }
        Item::Impl(ItemImpl {
            items,
            self_ty,
            trait_,
            ..
        }) => {
            let type_name = normalize_token_spacing(&quote::quote!(#self_ty).to_string());
            let scoped_ctx = ctx.with_type(&type_name);
            let is_trait_impl = trait_.is_some();
            let impl_header = if let Some((_, tr, _)) = trait_ {
                let tr_name = normalize_token_spacing(&quote::quote!(#tr).to_string());
                format!("impl {} for {}", tr_name, type_name)
            } else {
                format!("impl {}", type_name)
            };

            for impl_item in items {
                if let ImplItem::Fn(m) = impl_item {
                    if is_trait_impl || matches!(m.vis, Visibility::Public(_)) {
                        let m_sig = &m.sig;
                        let sig_str = normalize_token_spacing(&quote::quote!(#m_sig).to_string());
                        out.push_str(&format!(
                            "{} `fn {}` ({})\n\n",
                            hashes, m.sig.ident, impl_header
                        ));

                        let docs = extract_doc_comments(&m.attrs);
                        if !docs.is_empty() {
                            let contract = parse_intent_contract(&docs);
                            format_intent_markdown(out, &contract, level, &scoped_ctx);
                        }

                        let vis_prefix = if is_trait_impl { "" } else { "pub " };
                        out.push_str(&format!("~~~rust\n{}{}\n~~~\n\n", vis_prefix, sig_str));

                        let assertions = extract_block_assertions(&m.block);
                        format_assertions_markdown(out, &assertions, level);
                    }
                }
            }
        }
        _ => {}
    }

    Ok(())
}

#[derive(Parser, Debug)]
#[command(
    name = "cargo-docversion",
    bin_name = "cargo",
    version,
    about = "Cargo plugin for multi-version documentation bundling and schema contract verification"
)]
pub struct CargoCli {
    #[command(subcommand)]
    pub command: CargoSubcommands,
}

#[derive(Subcommand, Debug)]
pub enum CargoSubcommands {
    Docversion(DocversionCli),
}

#[derive(Parser, Debug)]
#[command(
    name = "docversion",
    version,
    about = "Multi-version documentation bundler and schema contract verification engine"
)]
pub struct DocversionCli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Document workspace crates with version-injected banners
    Doc(DocArgs),

    /// Assemble versioned distribution tree into target directory
    BuildSite(BuildSiteArgs),

    /// Export tracked Markdown docs/ directory (README.md index, per-module .md files, api-reference.md)
    ExportMd(ExportMdArgs),

    /// Synthesize deterministic repo_metadata.json from Cargo.toml and Intent blocks
    RepoMetadata(RepoMetadataArgs),

    /// Validate and preview repository metadata against GitHub constraints offline
    GithubMeta(GithubMetaArgs),

    /// Verify application compatibility against schema migration matrix
    #[command(alias = "check")]
    VerifyDb(VerifyDbArgs),

    /// One-shot build: compiles documentation and bundles versioned site
    Site(SiteArgs),
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
pub enum DocFormat {
    /// Standard site bundle (Rustdoc HTML + llms.txt + api-reference.md)
    Standard,
    /// Clean Markdown sanitized for Shortcut Docs import (strips raw HTML, rewrites relative .md links)
    Shortcut,
}

#[derive(Args, Debug)]
pub struct DocArgs {
    /// Skip documenting dependent crates
    #[arg(long, default_value_t = true)]
    pub no_deps: bool,

    /// Document all packages in the workspace
    #[arg(long, default_value_t = true)]
    pub workspace: bool,

    /// Pass additional flags directly to cargo doc
    #[arg(last = true)]
    pub cargo_args: Vec<String>,
}

#[derive(Args, Debug)]
pub struct BuildSiteArgs {
    /// Path to documentation source directory (defaults to target/doc)
    #[arg(long, default_value = "target/doc")]
    pub source: PathBuf,

    /// Explicit SemVer version for distribution (inferred from workspace if omitted)
    #[arg(long)]
    pub version: Option<String>,

    /// Landing page relative path within docs
    #[arg(long, default_value = "index.html")]
    pub entry_page: String,

    /// Output directory for bundled distribution
    #[arg(long, default_value = "target/doc_site")]
    pub out_dir: PathBuf,

    /// Output format mode
    #[arg(long, value_enum, default_value_t = DocFormat::Standard)]
    pub format: DocFormat,
}

#[derive(Args, Debug)]
pub struct ExportMdArgs {
    /// Output directory for tracked Markdown documentation tree
    #[arg(long, default_value = "docs")]
    pub out_dir: PathBuf,
}

#[derive(Args, Debug)]
pub struct VerifyDbArgs {
    /// Path to schema compatibility matrix JSON file
    #[arg(long, default_value = "schema_matrix.json")]
    pub manifest: PathBuf,

    /// Unique schema key identifier
    #[arg(long)]
    pub schema: Option<String>,

    /// Target database schema version
    #[arg(long)]
    pub db_version: Option<u32>,

    /// Current application SemVer version
    #[arg(long)]
    pub app_version: Option<String>,

    /// Optional artifact file to verify against SHA-256 checksum
    #[arg(long)]
    pub artifact: Option<PathBuf>,
}

#[derive(Args, Debug)]
pub struct SiteArgs {
    /// Output directory for bundled distribution
    #[arg(long, default_value = "target/doc_site")]
    pub out_dir: PathBuf,

    /// Output format mode
    #[arg(long, value_enum, default_value_t = DocFormat::Standard)]
    pub format: DocFormat,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct SiteManifestEntry {
    version: String,
    path: String,
    revision: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct SiteManifest {
    latest: String,
    versions: Vec<SiteManifestEntry>,
}

pub fn run_cargo_plugin(cli: CargoCli) -> Result<()> {
    match cli.command {
        CargoSubcommands::Docversion(docversion_cli) => run_cli(docversion_cli),
    }
}

pub fn run_cli(cli: DocversionCli) -> Result<()> {
    match cli.command {
        Commands::Doc(args) => execute_doc(args),
        Commands::BuildSite(args) => execute_build_site(args),
        Commands::ExportMd(args) => execute_export_md(args),
        Commands::RepoMetadata(args) => execute_repo_metadata(&args),
        Commands::GithubMeta(args) => execute_github_meta(&args),
        Commands::VerifyDb(args) => execute_verify_db(args),
        Commands::Site(args) => execute_site(args),
    }
}

fn execute_export_md(args: ExportMdArgs) -> Result<()> {
    println!(
        "==> docversion: Exporting tracked Markdown documentation tree into {}",
        args.out_dir.display()
    );
    export_tracked_docs_tree(Path::new("."), &args.out_dir)?;
    println!(
        "==> docversion: Tracked Markdown tree exported successfully at {}",
        args.out_dir.display()
    );
    Ok(())
}

fn execute_doc(args: DocArgs) -> Result<()> {
    let meta_cmd = MetadataCommand::new();
    let metadata = meta_cmd
        .exec()
        .context("Failed to resolve workspace metadata")?;

    let root_pkg = metadata
        .root_package()
        .or_else(|| metadata.workspace_packages().first().copied())
        .context("No packages found in workspace")?;

    let primary_ver = root_pkg.version.to_string();
    let primary_name = &root_pkg.name;

    println!(
        "==> docversion: Compiling Cargo docs for {} v{}",
        primary_name, primary_ver
    );

    let git_rev = compute_doc_revision(".", &["src/", "docs/", "README.md"])
        .unwrap_or_else(|_| "rev.clean".to_string());

    let _version_id = VersionIdentifier::new(&primary_ver, Some(&git_rev))
        .context("Failed to construct version identifier")?;

    let target_dir = PathBuf::from("target/docversion");
    fs::create_dir_all(&target_dir)?;

    let head_path = target_dir.join("head.html");
    let head_html = r#"<script>
(function() {
  try {
    var theme = localStorage.getItem('rustdoc-theme');
    var useSystem = localStorage.getItem('rustdoc-use-system-theme');
    if (!theme || useSystem === 'true' || theme === 'dark') {
      localStorage.setItem('rustdoc-theme', 'ayu');
      localStorage.setItem('rustdoc-use-system-theme', 'false');
      document.documentElement.setAttribute('data-theme', 'ayu');
    }
  } catch(e) {}

  function syncRustdocControls() {
    var hideRadios = ['dark', 'system preference'];
    hideRadios.forEach(function(val) {
      var r = document.querySelector('input[name="theme"][value="' + val + '"]');
      if (r) {
        var p = r.closest('label') || r.parentElement;
        if (p) p.style.setProperty('display', 'none', 'important');
      }
    });

    var subSelectors = [
      '#preferred-light-theme',
      '#preferred-dark-theme',
      '.theme-sub-choices',
      '#theme-system-preference',
      'label[for="theme-system-preference"]'
    ];
    subSelectors.forEach(function(sel) {
      document.querySelectorAll(sel).forEach(function(el) {
        el.style.setProperty('display', 'none', 'important');
      });
    });

    var ayuRadio = document.querySelector('input[name="theme"][value="ayu"]');
    if (ayuRadio) {
      var label = document.querySelector('label[for="' + ayuRadio.id + '"]') || ayuRadio.closest('label') || ayuRadio.parentElement;
      if (label) {
        for (var i = 0; i < label.childNodes.length; i++) {
          var node = label.childNodes[i];
          if (node.nodeType === Node.TEXT_NODE && node.textContent.indexOf('ayu') !== -1) {
            node.textContent = node.textContent.replace('ayu', 'dark');
          }
        }
      }
    }

    var hasToggles = document.querySelector('details.toggle') !== null;
    var toggleBtn = document.getElementById('toggle-all-docs');
    if (toggleBtn) {
      if (!hasToggles) {
        toggleBtn.style.setProperty('display', 'none', 'important');
      } else {
        toggleBtn.style.removeProperty('display');
      }
    }
  }

  document.addEventListener('DOMContentLoaded', syncRustdocControls);
  var observer = new MutationObserver(syncRustdocControls);
  observer.observe(document.documentElement, { childList: true, subtree: true });
})();
</script>
<style>
  input[name="theme"][value="system preference"],
  label:has(input[value="system preference"]),
  label[for*="system-preference"],
  input[name="theme"][value="dark"],
  label:has(input[name="theme"][value="dark"]),
  label[for="theme-dark"],
  #preferred-light-theme,
  #preferred-dark-theme,
  .theme-sub-choices {
    display: none !important;
  }

  #toggle-all-docs::before {
    display: inline-block !important;
    transition: transform 0.22s cubic-bezier(0.4, 0, 0.2, 1) !important;
    transform-origin: center center !important;
  }

  #toggle-all-docs.will-expand::before {
    transform: rotate(180deg) !important;
  }

  .docblock blockquote {
    border-left: 3px solid #ffb454 !important;
    background: rgba(255, 180, 84, 0.04) !important;
    padding: 8px 14px !important;
    border-radius: 2px !important;
  }
</style>"#;
    fs::write(&head_path, head_html)?;

    let banner_path = target_dir.join("banner.html");
    let banner_html = format!(
        r#"<!-- OBSIDIAN AYU TOPBAR -->
<style>
  :root {{
    --hh-bg: #0f141c;
    --hh-border: #1e2430;
    --hh-fg: #cbccc6;
    --hh-subfg: #707a8c;
    --hh-pill-bg: #141925;
    --hh-pill-border: #283040;
  }}

  :root[data-theme="light"], body.light {{
    --hh-bg: #191e28;
    --hh-border: #2d3648;
    --hh-fg: #e6e8ee;
    --hh-subfg: #8a94a8;
    --hh-pill-bg: #222938;
    --hh-pill-border: #3b465c;
  }}

  html {{
    scroll-padding-top: 40px;
  }}

  body {{
    padding-top: 40px !important;
  }}

  nav.sidebar, .sidebar {{
    top: 40px !important;
    height: calc(100vh - 40px) !important;
  }}

  #hh-obsidian-topbar {{
    position: fixed !important;
    top: 0 !important;
    left: 0 !important;
    width: 100vw !important;
    height: 40px !important;
    background: var(--hh-bg) !important;
    border-bottom: 0.75px solid var(--hh-border) !important;
    color: var(--hh-fg) !important;
    font-family: ui-monospace, "SF Mono", "JetBrains Mono", Menlo, Consolas, monospace !important;
    font-size: 13px !important;
    letter-spacing: 0.03em !important;
    z-index: 2147483647 !important;
    display: flex !important;
    align-items: center !important;
    justify-content: space-between !important;
    padding: 0 16px !important;
    box-sizing: border-box !important;
    user-select: none !important;
    box-shadow: 0 2px 10px rgba(0, 0, 0, 0.45) !important;
  }}

  #hh-obsidian-topbar * {{
    box-sizing: border-box !important;
  }}

  .hh-accent {{
    color: var(--hh-fg);
    font-weight: 600;
  }}

  .hh-tag {{
    color: var(--hh-subfg);
  }}

  .hh-select-wrapper {{
    position: relative;
    display: inline-flex;
    align-items: center;
  }}

  #hh-version-select {{
    appearance: none;
    -webkit-appearance: none;
    background: var(--hh-pill-bg);
    color: var(--hh-fg);
    border: 0.75px solid var(--hh-pill-border);
    border-radius: 3px;
    padding: 4px 26px 4px 10px;
    font-family: inherit;
    font-size: 12.5px;
    cursor: pointer;
    outline: none;
    transition: border-color 0.15s ease, background 0.15s ease;
  }}

  #hh-version-select:hover {{
    border-color: #ffb454;
  }}

  .hh-select-arrow {{
    position: absolute;
    right: 8px;
    pointer-events: none;
    font-size: 8px;
    color: var(--hh-subfg);
  }}

  .hh-pill {{
    display: inline-flex;
    align-items: center;
    background: var(--hh-pill-bg);
    border: 0.75px solid var(--hh-pill-border);
    border-radius: 3px;
    padding: 4px 10px;
    gap: 8px;
  }}

  .hh-status-indicator {{
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: #7fd962;
    box-shadow: 0 0 6px rgba(127, 217, 98, 0.6);
  }}
</style>

<div id="hh-obsidian-topbar">
  <div style="display:flex; align-items:center; gap:16px;">
    <span class="hh-pill">
      <span class="hh-status-indicator"></span>
      <span class="hh-accent">{name}</span>
    </span>
    <div class="hh-select-wrapper">
      <select id="hh-version-select" aria-label="Documentation Version">
        <option value="" selected class="hh-active-ver-label">v{version}</option>
      </select>
      <span class="hh-select-arrow">▼</span>
    </div>
  </div>

  <div style="display:flex; align-items:center;">
    <span class="hh-tag">[REV: <span style="color:var(--hh-fg);">{revision}</span>]</span>
  </div>
</div>

<script>
(function() {{
  const select = document.getElementById('hh-version-select');
  if (!select) return;

  const currentPath = window.location.pathname;
  let rootOffset = '';
  const match = currentPath.match(/(.*?\/(?:v\d+\.\d+\.\d+|latest)\/)/);
  if (match) {{
    const prefix = match[1];
    const rest = currentPath.substring(prefix.length);
    const depth = (rest.match(/\//g) || []).length;
    rootOffset = '../'.repeat(depth + 1);
  }} else {{
    rootOffset = './';
  }}

  fetch(rootOffset + 'versions.json')
    .then(r => {{
      if (!r.ok) throw new Error();
      return r.json();
    }})
    .then(data => {{
      if (!data || !Array.isArray(data.versions)) return;
      select.innerHTML = '';

      const latestOption = document.createElement('option');
      latestOption.value = rootOffset + 'latest/';
      latestOption.textContent = 'latest (v' + data.latest + ')';
      select.appendChild(latestOption);

      data.versions.forEach(v => {{
        const opt = document.createElement('option');
        opt.value = rootOffset + v.path;
        opt.textContent = 'v' + v.version;
        select.appendChild(opt);
      }});

      for (let i = 0; i < select.options.length; i++) {{
        const optVal = select.options[i].value;
        const normalized = optVal.replace(/^(\.\.\/)+/, '');
        if (currentPath.includes('/' + normalized)) {{
          select.selectedIndex = i;
          break;
        }}
      }}
    }})
    .catch(() => {{}});

  select.addEventListener('change', function() {{
    const targetBase = this.value;
    if (!targetBase) return;
    if (match) {{
      const subpath = currentPath.substring(match[1].length);
      window.location.href = targetBase + subpath;
    }} else {{
      window.location.href = targetBase;
    }}
  }});
}})();
</script>"#,
        name = primary_name,
        version = primary_ver,
        revision = git_rev
    );

    fs::write(&banner_path, banner_html)?;

    let mut cmd = Command::new("cargo");
    cmd.arg("doc");

    if args.no_deps {
        cmd.arg("--no-deps");
    }
    if args.workspace {
        cmd.arg("--workspace");
    }

    cmd.env(
        "RUSTDOCFLAGS",
        format!(
            "--html-in-header {} --html-before-content {} --default-theme ayu",
            head_path.canonicalize()?.display(),
            banner_path.canonicalize()?.display()
        ),
    );

    for extra in args.cargo_args {
        cmd.arg(extra);
    }

    let status = cmd.status().context("Failed to invoke cargo doc")?;
    if !status.success() {
        bail!("cargo doc failed with exit code: {:?}", status.code());
    }

    Ok(())
}

fn execute_site(args: SiteArgs) -> Result<()> {
    execute_doc(DocArgs {
        no_deps: true,
        workspace: true,
        cargo_args: vec![],
    })?;

    execute_build_site(BuildSiteArgs {
        source: PathBuf::from("target/doc"),
        version: None,
        entry_page: "index.html".to_string(),
        out_dir: args.out_dir,
        format: args.format,
    })
}

fn execute_build_site(args: BuildSiteArgs) -> Result<()> {
    let resolved_version = match args.version {
        Some(v) => v,
        None => {
            let metadata = MetadataCommand::new()
                .exec()
                .context("Failed to resolve workspace metadata for version inference")?;
            let root_pkg = metadata
                .root_package()
                .or_else(|| metadata.workspace_packages().first().copied())
                .context("No packages found in workspace")?;
            root_pkg.version.to_string()
        }
    };

    println!(
        "==> docversion: Packaging site for {} (version: {}) into {}",
        args.source.display(),
        resolved_version,
        args.out_dir.display()
    );

    let metadata = MetadataCommand::new()
        .exec()
        .context("Failed to resolve workspace metadata")?;

    let primary_name = metadata
        .root_package()
        .map(|p| p.name.clone())
        .unwrap_or_else(|| {
            metadata
                .workspace_packages()
                .first()
                .map(|p| p.name.clone())
                .unwrap_or_else(|| "docversion".to_string())
        });

    let crate_ident = primary_name.replace('-', "_");
    let version_dir = args.out_dir.join(format!("v{}", resolved_version));
    let latest_dir = args.out_dir.join("latest");

    if version_dir.exists() {
        fs::remove_dir_all(&version_dir)?;
    }
    fs::create_dir_all(&version_dir)?;
    copy_dir_recursive(&args.source, &version_dir)?;
    patch_distribution_versions(&version_dir, &resolved_version)?;

    if latest_dir.exists() {
        fs::remove_dir_all(&latest_dir)?;
    }
    copy_dir_recursive(&args.source, &latest_dir)?;
    patch_distribution_versions(&latest_dir, &resolved_version)?;

    let redirect_target = format!("latest/{}/index.html", crate_ident);
    let redirect_markup =
        generate_redirect_html(&redirect_target).context("Failed to generate redirect HTML")?;

    atomic_write(args.out_dir.join("index.html"), redirect_markup.as_bytes())?;

    let git_rev = compute_doc_revision(".", &["src/", "docs/", "README.md"])
        .unwrap_or_else(|_| "rev.clean".to_string());

    println!("==> docversion: Generating llms.txt context index...");
    let raw_markdown_summary = generate_markdown_summary(&primary_name, Path::new("."))?;
    let markdown_summary = match args.format {
        DocFormat::Standard => raw_markdown_summary,
        DocFormat::Shortcut => {
            println!("==> docversion: Sanitizing Markdown for Shortcut Docs export...");
            sanitize_for_shortcut_docs(&raw_markdown_summary)
        }
    };

    atomic_write(args.out_dir.join("llms.txt"), markdown_summary.as_bytes())?;
    atomic_write(
        args.out_dir.join("api-reference.md"),
        markdown_summary.as_bytes(),
    )?;

    atomic_write(version_dir.join("llms.txt"), markdown_summary.as_bytes())?;
    atomic_write(latest_dir.join("llms.txt"), markdown_summary.as_bytes())?;

    if args.format == DocFormat::Shortcut {
        copy_markdown_to_clipboard(&markdown_summary)?;
    }

    let manifest_path = args.out_dir.join("versions.json");
    let mut versions: Vec<SiteManifestEntry> = if manifest_path.exists() {
        let existing_data = fs::read(&manifest_path).unwrap_or_default();
        serde_json::from_slice::<SiteManifest>(&existing_data)
            .map(|m| m.versions)
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    if let Some(existing) = versions.iter_mut().find(|v| v.version == resolved_version) {
        existing.path = format!("v{}/", resolved_version);
        existing.revision = git_rev;
    } else {
        versions.push(SiteManifestEntry {
            version: resolved_version.clone(),
            path: format!("v{}/", resolved_version),
            revision: git_rev,
        });
    }

    versions.sort_by(|a, b| {
        let ver_a = semver::Version::parse(&a.version).ok();
        let ver_b = semver::Version::parse(&b.version).ok();
        ver_b.cmp(&ver_a)
    });

    let latest_version = versions
        .first()
        .map(|v| v.version.clone())
        .unwrap_or_else(|| resolved_version);

    let site_manifest = SiteManifest {
        latest: latest_version,
        versions,
    };

    let manifest_bytes = serde_json::to_vec_pretty(&site_manifest)?;
    atomic_write(manifest_path, &manifest_bytes)?;

    println!(
        "==> docversion: Site distribution compiled successfully at {}",
        args.out_dir.display()
    );

    Ok(())
}

#[cfg(target_os = "macos")]
fn copy_markdown_to_clipboard(markdown: &str) -> Result<()> {
    use std::io::Write;
    use std::process::Stdio;

    let html = docversion_core::markdown_to_clean_html(markdown);

    let mut textutil = Command::new("/usr/bin/textutil")
        .args(["-stdin", "-format", "html", "-convert", "rtf", "-stdout"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .context("Failed to spawn /usr/bin/textutil")?;

    if let Some(mut stdin) = textutil.stdin.take() {
        stdin.write_all(html.as_bytes())?;
    }

    let rtf_output = textutil.wait_with_output()?;
    if !rtf_output.status.success() {
        bail!("textutil failed to convert HTML to RTF");
    }

    let mut pbcopy = Command::new("/usr/bin/pbcopy")
        .stdin(Stdio::piped())
        .spawn()
        .context("Failed to spawn /usr/bin/pbcopy")?;

    if let Some(mut stdin) = pbcopy.stdin.take() {
        stdin.write_all(&rtf_output.stdout)?;
    }

    let copy_status = pbcopy.wait()?;
    if !copy_status.success() {
        bail!("pbcopy failed with exit code: {:?}", copy_status.code());
    }

    println!("==> docversion: Rich-text RTF copied to clipboard — press Cmd+V in Shortcut Docs.");
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn copy_markdown_to_clipboard(markdown: &str) -> Result<()> {
    use std::io::Write;
    use std::process::Stdio;

    let candidates: &[(&str, &[&str])] = &[
        ("wl-copy", &[]),
        ("xclip", &["-selection", "clipboard"]),
        ("clip.exe", &[]),
    ];

    for (bin, args) in candidates {
        if let Ok(mut child) = Command::new(bin)
            .args(*args)
            .stdin(Stdio::piped())
            .spawn()
        {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(markdown.as_bytes());
            }
            if child.wait().is_ok_and(|s| s.success()) {
                println!(
                    "==> docversion: Sanitized Markdown copied via {} — paste into Shortcut Docs.",
                    bin
                );
                return Ok(());
            }
        }
    }

    println!(
        "==> docversion: No system clipboard utility found; read target/doc_site/api-reference.md directly."
    );
    Ok(())
}

fn patch_distribution_versions(dir: &Path, target_version: &str) -> Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let p = entry.path();
        if p.is_dir() {
            patch_distribution_versions(&p, target_version)?;
        } else if p
            .extension()
            .is_some_and(|ext| ext == "html" || ext == "js")
        {
            if let Ok(content) = fs::read_to_string(&p) {
                let mut updated = content.clone();

                updated = replace_delimited(
                    &updated,
                    r#"class="hh-active-ver-label">"#,
                    "</option>",
                    &format!("v{}", target_version),
                );

                updated = replace_delimited(
                    &updated,
                    r#"<span class="version">"#,
                    "</span>",
                    target_version,
                );
                updated = replace_delimited(&updated, r#"class="version">"#, "</", target_version);
                updated = replace_delimited(
                    &updated,
                    r#"<div class="version">"#,
                    "</div>",
                    target_version,
                );

                updated = replace_delimited(
                    &updated,
                    r#"<span>cargo-docversion v"#,
                    "</span>",
                    &format!("{}</span>", target_version),
                );

                if updated != content {
                    let _ = fs::write(&p, updated);
                }
            }
        }
    }
    Ok(())
}

fn replace_delimited(content: &str, start_tag: &str, end_delim: &str, replacement: &str) -> String {
    let mut result = String::new();
    let mut remainder = content;

    while let Some(idx) = remainder.find(start_tag) {
        result.push_str(&remainder[..idx]);
        result.push_str(start_tag);
        let after = &remainder[idx + start_tag.len()..];
        if let Some(end_idx) = after.find(end_delim) {
            result.push_str(replacement);
            result.push_str(end_delim);
            remainder = &after[end_idx + end_delim.len()..];
        } else {
            remainder = after;
            break;
        }
    }
    result.push_str(remainder);
    result
}

fn execute_verify_db(args: VerifyDbArgs) -> Result<()> {
    let manifest_path = &args.manifest;
    if !manifest_path.exists() {
        bail!(
            "Schema matrix manifest not found at: {}",
            manifest_path.display()
        );
    }

    let raw = fs::read(manifest_path)
        .with_context(|| format!("Failed to read manifest file: {}", manifest_path.display()))?;

    let matrix =
        SchemaMatrix::from_json_slice(&raw).context("Malformed schema matrix JSON manifest")?;

    let metadata = MetadataCommand::new().exec().ok();
    let root_pkg = metadata.as_ref().and_then(|m| {
        m.root_package()
            .or_else(|| m.workspace_packages().first().copied())
    });

    let app_ver_str = match args.app_version {
        Some(v) => v,
        None => root_pkg
            .map(|p| p.version.to_string())
            .context("Application version omitted and could not be inferred from Cargo metadata")?,
    };

    let schema_key = match args.schema {
        Some(s) => s,
        None => {
            let mut available: Vec<String> = matrix
                .contracts
                .iter()
                .map(|c| c.schema_id.clone())
                .collect();
            available.sort();
            available.dedup();
            if available.is_empty() {
                bail!("No schema contracts defined in schema matrix manifest");
            } else if available.len() == 1 {
                available[0].clone()
            } else {
                bail!(
                    "Multiple schemas found in matrix {:?}. Specify target with --schema <name>",
                    available
                );
            }
        }
    };

    let db_ver = args.db_version.unwrap_or(1);

    matrix.verify_compatibility(&app_ver_str, &schema_key, db_ver)?;

    println!(
        "==> Contract Verified: App v{} matches schema '{}' v{}",
        app_ver_str, schema_key, db_ver
    );

    if let Some(artifact_path) = args.artifact {
        if !artifact_path.exists() {
            bail!("Artifact file not found: {}", artifact_path.display());
        }
        let matching_contract = matrix
            .contracts
            .iter()
            .find(|c| c.schema_id == schema_key && c.schema_version == db_ver)
            .context("No contract found matching schema and version for artifact validation")?;

        SchemaMatrix::verify_file_checksum(&artifact_path, &matching_contract.sha256)?;
        println!(
            "==> Artifact Verified: SHA-256 digest confirmed for {}",
            artifact_path.display()
        );
    }

    Ok(())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    if !src.exists() {
        return Ok(());
    }
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let entry_path = entry.path();
        let target_path = dst.join(entry.file_name());
        if entry_path.is_dir() {
            copy_dir_recursive(&entry_path, &target_path)?;
        } else {
            fs::copy(&entry_path, &target_path)?;
        }
    }
    Ok(())
}

fn atomic_write<P: AsRef<Path>>(path: P, data: &[u8]) -> Result<()> {
    let path = path.as_ref();
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, data)?;
    fs::rename(tmp, path)?;
    Ok(())
}

/// Parsed intentional sections and preserved doctest blocks from doc comments.
#[derive(Default, Debug)]
struct IntentContract {
    summary: String,
    doctests: Vec<String>,
    intent: Option<String>,
    guarantees: Vec<String>,
    invariants: Vec<String>,
    non_goals: Vec<String>,
    notes: Vec<String>,
}

fn parse_intent_contract(doc_text: &str) -> IntentContract {
    let mut contract = IntentContract::default();
    let mut target: Option<&mut Vec<String>> = None;
    let mut in_fence = false;
    let mut current_fence = String::new();

    for line in doc_text.lines() {
        let trimmed_raw = line.trim();
        if trimmed_raw.starts_with("```") || trimmed_raw.starts_with("~~~") {
            if !in_fence {
                in_fence = true;
                current_fence.clear();
                let lang = trimmed_raw.trim_start_matches(['`', '~']).trim();
                let lang_tag = if lang.is_empty() { "rust" } else { lang };
                current_fence.push_str(&format!("~~~{}\n", lang_tag));
            } else {
                in_fence = false;
                current_fence.push_str("~~~");
                contract.doctests.push(current_fence.clone());
                current_fence.clear();
            }
            continue;
        }

        if in_fence {
            current_fence.push_str(line);
            current_fence.push('\n');
            continue;
        }

        let clean = trimmed_raw
            .trim_start_matches(['>', '#', '*', '-', ' '])
            .trim();
        let lower = clean.to_lowercase();

        if lower.starts_with("intent") {
            let desc = clean["intent".len()..]
                .trim_start_matches([':', '*', ' '])
                .trim();
            if !desc.is_empty() {
                contract.intent = Some(desc.to_string());
            }
            target = None;
        } else if lower.starts_with("guarantee") {
            target = Some(&mut contract.guarantees);
        } else if lower.starts_with("invariant") {
            target = Some(&mut contract.invariants);
        } else if lower.starts_with("non-goal") || lower.starts_with("non goal") {
            target = Some(&mut contract.non_goals);
        } else if lower.starts_with("note") {
            target = Some(&mut contract.notes);
        } else if let Some(ref mut list) = target {
            if !clean.is_empty() {
                list.push(clean.to_string());
            }
        } else if contract.intent.is_none() && !clean.is_empty() {
            if contract.summary.is_empty() {
                contract.summary = clean.to_string();
            } else {
                contract.summary.push_str(&format!(" {}", clean));
            }
        }
    }

    contract
}

fn format_intent_markdown(
    out: &mut String,
    contract: &IntentContract,
    heading_level: usize,
    ctx: &RenderContext<'_>,
) {
    let subhashes = "#".repeat((heading_level + 1).min(6));

    if let Some(intent) = &contract.intent {
        let resolved_intent = ctx.resolve_text(intent);
        out.push_str(&format!(
            "{} **Intent Directive**\n\n> {}\n\n",
            subhashes,
            resolved_intent.replace('\n', "\n> ")
        ));
    } else if !contract.summary.is_empty() {
        let resolved_summary = ctx.resolve_text(&contract.summary);
        out.push_str(&format!("{}\n\n", resolved_summary));
    }

    for dt in &contract.doctests {
        out.push_str(&format!("{}\n\n", dt));
    }

    if !contract.guarantees.is_empty() {
        out.push_str(&format!("{} **Guarantees**\n\n", subhashes));
        for g in &contract.guarantees {
            out.push_str(&format!("* [x] **Guarantee:** {}\n", ctx.resolve_text(g)));
        }
        out.push('\n');
    }

    if !contract.invariants.is_empty() {
        out.push_str(&format!("{} **System Invariants**\n\n", subhashes));
        for inv in &contract.invariants {
            out.push_str(&format!("* 🛡️ `{}`\n", inv));
        }
        out.push('\n');
    }

    if !contract.non_goals.is_empty() {
        out.push_str(&format!("{} **Explicit Non-Goals**\n\n", subhashes));
        for ng in &contract.non_goals {
            out.push_str(&format!("* ⛔ {}\n", ctx.resolve_text(ng)));
        }
        out.push('\n');
    }

    if !contract.notes.is_empty() {
        out.push_str(&format!("{} **Architectural Notes**\n\n", subhashes));
        for note in &contract.notes {
            out.push_str(&format!("* 📝 {}\n", ctx.resolve_text(note)));
        }
        out.push('\n');
    }
}

fn execute_repo_metadata(args: &RepoMetadataArgs) -> Result<()> {
    println!(
        "==> docversion: Synthesizing deterministic workspace repo metadata into {}",
        args.out.display()
    );

    let manifest_path = Path::new("Cargo.toml");
    let metadata = MetadataCommand::new()
        .manifest_path(manifest_path)
        .no_deps()
        .exec()
        .context("Failed to resolve workspace metadata for repo-metadata")?;

    let mut packages = metadata.workspace_packages();
    packages.sort_by(|a, b| a.name.cmp(&b.name));

    let mut crates = Vec::new();

    for pkg in packages {
        let mut keywords = pkg.keywords.clone();
        keywords.sort();
        keywords.dedup();

        let mut categories = pkg.categories.clone();
        categories.sort();
        categories.dedup();

        let mut intent_summary = None;
        if let Some(pkg_dir) = pkg.manifest_path.parent() {
            let src_dir = pkg_dir.as_std_path().join("src");
            let lib_rs = src_dir.join("lib.rs");
            let main_rs = src_dir.join("main.rs");
            let entry = if lib_rs.exists() {
                Some(lib_rs)
            } else if main_rs.exists() {
                Some(main_rs)
            } else {
                None
            };

            if let Some(path) = entry {
                if let Ok(code) = fs::read_to_string(path) {
                    if let Ok(syntax) = syn::parse_file(&code) {
                        let docs = extract_doc_comments(&syntax.attrs);
                        if !docs.is_empty() {
                            let contract = parse_intent_contract(&docs);
                            if let Some(intent) = contract.intent {
                                intent_summary = Some(intent);
                            } else if !contract.summary.is_empty() {
                                intent_summary = Some(contract.summary);
                            }
                        }
                    }
                }
            }
        }

        crates.push(CrateMetadata {
            name: pkg.name.clone(),
            version: pkg.version.to_string(),
            description: pkg.description.clone(),
            homepage: pkg.homepage.clone(),
            repository: pkg.repository.clone(),
            documentation: pkg.documentation.clone(),
            keywords,
            categories,
            intent_summary,
        });
    }

    let root_pkg = metadata
        .root_package()
        .or_else(|| metadata.workspace_packages().first().copied())
        .context("No packages found in workspace")?;

    let repo_meta = RepoMetadata {
        workspace_name: root_pkg.name.clone(),
        crates,
    };

    let json_bytes = serde_json::to_vec_pretty(&repo_meta)
        .context("Failed to serialize repo metadata to JSON")?;

    atomic_write(&args.out, &json_bytes)?;

    println!(
        "==> docversion: Deterministic repo_metadata.json synthesized successfully at {}",
        args.out.display()
    );
    Ok(())
}


fn execute_github_meta(args: &GithubMetaArgs) -> Result<()> {
    if !args.manifest.exists() {
        bail!(
            "Repository metadata manifest not found at: {}. Run `cargo hh-docversion repo-metadata` first.",
            args.manifest.display()
        );
    }

    println!(
        "==> docversion: Validating and previewing GitHub metadata from {}...",
        args.manifest.display()
    );

    let raw = fs::read(&args.manifest)
        .with_context(|| format!("Failed to read manifest file: {}", args.manifest.display()))?;
    
    let repo_meta: RepoMetadata = serde_json::from_slice(&raw)
        .context("Malformed repo_metadata.json manifest JSON")?;

    let mut has_errors = false;

    println!("\n=== WORKSPACE REPOSITORY METADATA ===");
    println!("Workspace: {}\n", repo_meta.workspace_name);

    for (idx, crate_meta) in repo_meta.crates.iter().enumerate() {
        println!("[{}] Crate: {} (v{})", idx + 1, crate_meta.name, crate_meta.version);
        
        // Validate description length
        if let Some(desc) = &crate_meta.description {
            let char_count = desc.chars().count();
            if char_count > 350 {
                println!("  ❌ Description exceeds GitHub limit of 350 characters (current: {})", char_count);
                has_errors = true;
            } else {
                println!("  description: {} ({} chars)", desc, char_count);
            }
        } else {
            println!("  description: [None]");
        }

        println!("  homepage: {}", crate_meta.homepage.as_deref().unwrap_or("[None]"));
        println!("  repository: {}", crate_meta.repository.as_deref().unwrap_or("[None]"));
        println!("  documentation: {}", crate_meta.documentation.as_deref().unwrap_or("[None]"));

        // Validate topics / keywords
        if crate_meta.keywords.len() > 20 {
            println!("  ❌ Topics exceed GitHub maximum limit of 20 (current: {})", crate_meta.keywords.len());
            has_errors = true;
        }

        let mut valid_keywords = Vec::new();
        for kw in &crate_meta.keywords {
            let kw_len = kw.chars().count();
            let is_valid_chars = kw.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
            
            if kw_len > 35 {
                println!("  ❌ Topic '{}' exceeds 35 character limit (length: {})", kw, kw_len);
                has_errors = true;
            } else if !is_valid_chars {
                println!("  ❌ Topic '{}' contains invalid characters (must be lowercase alphanumeric and hyphens)", kw);
                has_errors = true;
            } else {
                valid_keywords.push(kw.clone());
            }
        }
        println!("  topics ({}/{} valid): {:?}", valid_keywords.len(), crate_meta.keywords.len(), valid_keywords);
        println!("  intent_summary: {}\n", crate_meta.intent_summary.as_deref().unwrap_or("[None]"));
    }

    if has_errors {
        bail!("GitHub metadata validation failed due to policy violations.");
    }

    println!("==> docversion: GitHub metadata validation passed successfully.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn cli_parses_shortcut_and_export_md_commands() {
        let site_cli = DocversionCli::try_parse_from([
            "docversion",
            "site",
            "--format",
            "shortcut",
            "--out-dir",
            "target/shortcut_test",
        ])
        .expect("site --format shortcut should parse");

        match site_cli.command {
            Commands::Site(args) => {
                assert_eq!(args.format, DocFormat::Shortcut);
                assert_eq!(args.out_dir, PathBuf::from("target/shortcut_test"));
            }
            other => panic!("Expected Commands::Site, got {:?}", other),
        }

        let export_cli =
            DocversionCli::try_parse_from(["docversion", "export-md", "--out-dir", "docs"])
                .expect("export-md should parse");

        match export_cli.command {
            Commands::ExportMd(args) => {
                assert_eq!(args.out_dir, PathBuf::from("docs"));
            }
            other => panic!("Expected Commands::ExportMd, got {:?}", other),
        }
    }

    #[test]
    fn cli_defaults_to_standard_format() {
        let cli = DocversionCli::try_parse_from(["docversion", "site"])
            .expect("site without --format should parse");

        match cli.command {
            Commands::Site(args) => assert_eq!(args.format, DocFormat::Standard),
            other => panic!("Expected Commands::Site, got {:?}", other),
        }
    }

    #[test]
    fn generated_crate_markdown_sanitizes_html_for_shortcut_docs() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let temp_crate = std::env::temp_dir().join(format!("hh_docversion_shortcut_{nonce}"));
        let src_dir = temp_crate.join("src");
        fs::create_dir_all(&src_dir).expect("create temp src dir");

        let sample_lib = concat!(
            "//! <div class=\"banner\">Internal Header</div>\n",
            "//! See [Ledger](/latest/tuos/index.html) for details.<br/>\n",
            "/// Verifies input items using [`BatchConfig`].\n",
            "/// ```rust\n",
            "/// let ok = verify(vec![1, 2]);\n",
            "/// ```\n",
            "pub fn verify<T: Ord>(items: Vec<T>) -> Result<(), String> {\n",
            "    assert!(!items.is_empty());\n",
            "    if items.len() > 10 {\n",
            "        return Err(\"too many\".into());\n",
            "    }\n",
            "    Ok(())\n",
            "}\n",
            "pub struct BatchConfig;\n"
        );
        fs::write(src_dir.join("lib.rs"), sample_lib).expect("write temp lib.rs");

        let raw_md = generate_markdown_summary("sample_crate", &temp_crate)
            .expect("generate markdown summary");
        let clean_md = sanitize_for_shortcut_docs(&raw_md);

        let _ = fs::remove_dir_all(&temp_crate);

        assert!(!clean_md.contains("<div"));
        assert!(!clean_md.contains("<br/>"));
        assert!(clean_md.contains("[Ledger](./tuos.md)"));
        assert!(clean_md.contains("[`BatchConfig`](#struct-batchconfig)"));
        assert!(
            clean_md.contains("pub fn verify<T: Ord>(items: Vec<T>) -> Result<(), String>"),
            "Actual generated markdown:\n{}",
            clean_md
        );
        assert!(clean_md.contains("~~~rust\nlet ok = verify(vec![1, 2]);\n~~~"));
        assert_eq!(clean_md.matches("🚫 Reject:").count(), 1);
    }

    #[test]
    fn export_tracked_docs_tree_emits_readme_modules_and_api_reference() {
        let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("workspace root");

        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let temp_docs = std::env::temp_dir().join(format!("hh_docversion_export_md_{nonce}"));

        export_tracked_docs_tree(workspace_root, &temp_docs)
            .expect("export_tracked_docs_tree should succeed");

        let readme = fs::read_to_string(temp_docs.join("README.md")).expect("read README.md");
        let api_ref =
            fs::read_to_string(temp_docs.join("api-reference.md")).expect("read api-reference.md");
        let core_md = fs::read_to_string(temp_docs.join("docversion_core.md"))
            .expect("read docversion_core.md");
        let xref_md = fs::read_to_string(temp_docs.join("docversion_core/xref.md"))
            .expect("read docversion_core/xref.md");

        let _ = fs::remove_dir_all(&temp_docs);

        assert!(readme.contains("**Version:** `v"));
        assert!(readme.contains("**Tag:** `"));
        assert!(readme.contains("**Revision:** `rev."));
        assert!(readme.contains("[`docversion_core.md`](./docversion_core.md)"));
        assert!(readme.contains("[`api-reference.md`](./api-reference.md)"));
        assert!(core_md.contains("Module [`xref`](./docversion_core/xref.md)"));
        assert!(xref_md.contains("# Module `docversion_core::xref`"));
        assert!(api_ref.contains("## Crate `docversion-core`"));
    }

    #[test]
    fn universal_source_scanner_extracts_ts_py_go_and_sql() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let temp_dir = std::env::temp_dir().join(format!("hh_docversion_polyglot_{nonce}"));
        fs::create_dir_all(&temp_dir).expect("create temp dir");

        fs::write(
            temp_dir.join("ledger.ts"),
            "// Computes balance\nexport function computeBalance(entries: number[]): number {\n  return 0;\n}\n",
        )
        .unwrap();
        fs::write(
            temp_dir.join("audit.py"),
            "# Verifies hash chain\ndef verify_chain(blocks: list[str]) -> bool:\n    return True\n",
        )
        .unwrap();
        fs::write(
            temp_dir.join("schema.sql"),
            "-- Core ledger entries\nCREATE TABLE journal_entries (\n    id TEXT PRIMARY KEY\n);\n",
        )
        .unwrap();

        let raw_md = generate_markdown_summary("polyglot-workspace", &temp_dir).unwrap();
        let clean_md = sanitize_for_shortcut_docs(&raw_md);
        let _ = fs::remove_dir_all(&temp_dir);

        assert!(clean_md.contains("~~~typescript\nexport function computeBalance(entries: number[]): number\n~~~"));
        assert!(clean_md.contains("~~~python\ndef verify_chain(blocks: list[str]) -> bool:\n~~~"));
        assert!(clean_md.contains("~~~sql\nCREATE TABLE journal_entries (\n    id TEXT PRIMARY KEY\n);\n~~~"));
    }
}
