use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use docversion_core::{escape_html_attribute, generate_redirect_html, VersionIdentifier};
use docversion_git::compute_doc_revision;
use docversion_manifest::SchemaMatrix;
use serde::Serialize;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Parser)]
#[command(name = "cargo", bin_name = "cargo")]
enum CargoCli {
    Docversion(DocversionCli),
}

#[derive(Parser, Debug)]
#[command(
    name = "docversion",
    bin_name = "docversion",
    author,
    version,
    about = "Universal version-aware documentation site bundler and DB schema contract verifier"
)]
pub struct DocversionCli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Injects an ephemeral version banner into rustdoc builds
    Doc {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        cargo_args: Vec<String>,
    },
    /// Compiles a multi-version static doc distribution (/latest, /vX.Y.Z, versions.json)
    BuildSite {
        #[arg(long)]
        source: Option<PathBuf>,

        #[arg(long, requires = "source")]
        version: Option<String>,

        #[arg(long, default_value = "index.html")]
        entry_page: String,

        #[arg(long, default_value = "target/doc_site")]
        out_dir: PathBuf,
    },
    /// Asserts schema SemVer compatibility and validates artifact SHA-256 digests
    VerifyDb {
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long)]
        schema: String,
        #[arg(long)]
        db_version: u32,
        #[arg(long)]
        app_version: Option<String>,
        #[arg(long)]
        artifact_path: Option<PathBuf>,
    },
}

#[derive(Serialize)]
struct SiteVersionEntry {
    name: String,
    path: String,
}

fn atomic_write<P: AsRef<Path>>(dest: P, contents: &[u8]) -> io::Result<()> {
    let dest = dest.as_ref();
    let parent = dest.parent().unwrap_or_else(|| Path::new("."));
    let temp_name = format!(".tmp_{}", dest.file_name().and_then(|s| s.to_str()).unwrap_or("file"));
    let temp_path = parent.join(temp_name);

    fs::write(&temp_path, contents)?;
    fs::rename(&temp_path, dest)?;
    Ok(())
}

fn recursive_copy(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> io::Result<()> {
    let src = src.as_ref();
    let dst = dst.as_ref();
    fs::create_dir_all(dst)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ft = entry.file_type()?;
        let target = dst.join(entry.file_name());

        if ft.is_dir() {
            recursive_copy(entry.path(), target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

fn resolve_target_identity(metadata: &cargo_metadata::Metadata) -> Result<(String, String)> {
    if let Some(root) = metadata.root_package() {
        return Ok((root.name.clone(), root.version.to_string()));
    }

    let workspace_members: Vec<_> = metadata
        .packages
        .iter()
        .filter(|p| metadata.workspace_members.contains(&p.id))
        .collect();

    if let Some(pkg) = workspace_members.first() {
        let workspace_name = metadata
            .workspace_root
            .file_name()
            .unwrap_or(&pkg.name);
        Ok((workspace_name.to_string(), pkg.version.to_string()))
    } else {
        bail!("No packages found in workspace");
    }
}

pub fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let cli = if args.len() > 1 && args[1] == "docversion" {
        let CargoCli::Docversion(inner) = CargoCli::parse();
        inner
    } else {
        DocversionCli::parse()
    };

    match cli.command {
        Commands::Doc { cargo_args } => {
            let metadata = cargo_metadata::MetadataCommand::new()
                .exec()
                .context("Failed to resolve workspace metadata")?;

            let (package_name, declared_ver) = resolve_target_identity(&metadata)?;

            let git_rev = compute_doc_revision(".", &["src/", "docs/", "README.md"])
                .unwrap_or_else(|_| "rev.clean".to_string());
            let ver_id = VersionIdentifier::new(&declared_ver, Some(&git_rev))
                .context("Failed to construct version identifier")?;

            let banner_dir = PathBuf::from("target/docversion");
            fs::create_dir_all(&banner_dir)?;
            let banner_path = banner_dir.join("banner.html");

            let safe_crate = escape_html_attribute(&package_name);
            let safe_version = escape_html_attribute(&ver_id.display_version());

            let banner_html = format!(
                r#"<div id="hh-docversion-bar" style="background:#1b1e24;color:#dcdcdc;padding:6px 16px;font-family:ui-monospace,SFMono-Regular,Consolas,monospace;font-size:12px;border-bottom:1px solid #3c3c44;display:flex;justify-content:space-between;align-items:center;">
  <span>Crate / Workspace: <strong style="color:#61afef;">{safe_crate}</strong></span>
  <span>Artifact: <strong style="color:#98c379;">{safe_version}</strong></span>
</div>"#,
            );

            atomic_write(&banner_path, banner_html.as_bytes())?;

            let mut cmd = Command::new("cargo");
            cmd.arg("doc")
                .env("RUSTDOCFLAGS", format!("--html-before-content {}", banner_path.display()))
                .args(&cargo_args);

            let status = cmd.status().context("Failed to invoke cargo doc")?;
            if !status.success() {
                bail!("cargo doc exited with status code {}", status.code().unwrap_or(1));
            }
        }

        Commands::BuildSite {
            source,
            version,
            entry_page,
            out_dir,
        } => {
            let (source_doc, declared_ver, default_entry) = match source {
                Some(src_dir) => {
                    if !src_dir.exists() {
                        bail!("Specified --source directory does not exist: {}", src_dir.display());
                    }

                    let ver = match version {
                        Some(v) => v,
                        None => {
                            let metadata = cargo_metadata::MetadataCommand::new()
                                .exec()
                                .context("Failed to resolve workspace metadata for version inference")?;
                            let (_, v) = resolve_target_identity(&metadata)?;
                            v
                        }
                    };

                    let clean_entry = entry_page.trim_start_matches('/');
                    let redirect_target = format!("latest/{}", clean_entry);
                    (src_dir, ver, redirect_target)
                }
                None => {
                    let metadata = cargo_metadata::MetadataCommand::new()
                        .exec()
                        .context("Failed to resolve workspace metadata")?;

                    let (primary_name, declared_ver) = resolve_target_identity(&metadata)?;
                    let crate_ident = primary_name.replace('-', "_");

                    println!("==> docversion: Compiling Cargo docs for {}", primary_name);
                    let status = Command::new("cargo")
                        .args(["doc", "--no-deps", "--all-features"])
                        .status()
                        .context("Failed to build workspace documentation")?;

                    if !status.success() {
                        bail!("Documentation build failed");
                    }

                    let doc_path = PathBuf::from("target/doc");
                    if !doc_path.exists() {
                        bail!("Target doc output directory not found at target/doc");
                    }

                    let redirect_target = if doc_path.join(&crate_ident).join("index.html").exists() {
                        format!("latest/{}/index.html", crate_ident)
                    } else if doc_path.join("cargo_docversion/index.html").exists() {
                        "latest/cargo_docversion/index.html".to_string()
                    } else if let Some(first_member) = metadata
                        .packages
                        .iter()
                        .find(|p| metadata.workspace_members.contains(&p.id))
                    {
                        format!("latest/{}/index.html", first_member.name.replace('-', "_"))
                    } else {
                        "latest/index.html".to_string()
                    };

                    (doc_path, declared_ver, redirect_target)
                }
            };

            let ver_id = VersionIdentifier::new(&declared_ver, None)?;
            let target_sub = ver_id.target_directory();

            println!(
                "==> docversion: Packaging site for {} (version: {}) into {}",
                source_doc.display(),
                declared_ver,
                out_dir.display()
            );

            fs::create_dir_all(&out_dir)?;
            recursive_copy(&source_doc, out_dir.join(&target_sub))?;
            recursive_copy(&source_doc, out_dir.join("latest"))?;

            let versions_index = vec![
                SiteVersionEntry {
                    name: "latest".into(),
                    path: "latest".into(),
                },
                SiteVersionEntry {
                    name: target_sub.clone(),
                    path: target_sub,
                },
            ];
            let manifest_bytes = serde_json::to_vec_pretty(&versions_index)?;
            atomic_write(out_dir.join("versions.json"), &manifest_bytes)?;

            let redirect_markup = generate_redirect_html(&default_entry)?;
            atomic_write(out_dir.join("index.html"), redirect_markup.as_bytes())?;

            println!("==> docversion: Site distribution compiled successfully at {}", out_dir.display());
        }

        Commands::VerifyDb {
            manifest,
            schema,
            db_version,
            app_version,
            artifact_path,
        } => {
            let raw = fs::read(&manifest).with_context(|| format!("Manifest unreadable: {}", manifest.display()))?;
            let matrix = SchemaMatrix::from_json_slice(&raw).context("Malformed schema matrix JSON")?;

            let target_app_ver = match app_version {
                Some(v) => v,
                None => {
                    let metadata = cargo_metadata::MetadataCommand::new().exec()?;
                    let (_, ver) = resolve_target_identity(&metadata)?;
                    ver
                }
            };

            let contract = matrix
                .verify_compatibility(&target_app_ver, &schema, db_version)
                .map_err(|e| anyhow::anyhow!("{}", e))?;

            println!(
                "==> Contract Verified: App v{} matches schema '{}' v{} (Allowed: {})",
                target_app_ver, schema, db_version, contract.supported_app_range
            );

            if let Some(artifact) = artifact_path {
                SchemaMatrix::verify_file_checksum(&artifact, &contract.sha256)
                    .map_err(|e| anyhow::anyhow!("{}", e))?;
                println!("==> Artifact Verified: SHA-256 digest validated successfully.");
            }
        }
    }

    Ok(())
}
