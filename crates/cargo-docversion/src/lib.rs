use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};
use cargo_metadata::MetadataCommand;
use clap::{Args, Parser, Subcommand};
use docversion_core::{generate_redirect_html, VersionIdentifier};
use docversion_git::compute_doc_revision;
use docversion_manifest::SchemaMatrix;
use serde::{Deserialize, Serialize};

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

    /// Verify application compatibility against schema migration matrix
    #[command(alias = "check")]
    VerifyDb(VerifyDbArgs),

    /// One-shot build: compiles documentation and bundles versioned site
    Site(SiteArgs),
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
        Commands::VerifyDb(args) => execute_verify_db(args),
        Commands::Site(args) => execute_site(args),
    }
}

fn execute_doc(args: DocArgs) -> Result<()> {
    let metadata = MetadataCommand::new()
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
  // Pre-seed theme to ayu
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
    // 1. Hide unwanted theme radio items in settings
    var hideRadios = ['dark', 'system preference'];
    hideRadios.forEach(function(val) {
      var r = document.querySelector('input[name="theme"][value="' + val + '"]');
      if (r) {
        var p = r.closest('label') || r.parentElement;
        if (p) p.style.setProperty('display', 'none', 'important');
      }
    });

    // 2. Hide sub-theme choices
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

    // 3. Re-label ayu as dark in settings text
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

    // 4. Hide toggle-all-docs if page has no collapsible sections
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
  /* Settings styling: Hide redundant themes */
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

  /* FLIP RUSTDOC SUMMARY / SHOW ALL ICON DIRECTLY ON ::before */
  #toggle-all-docs::before {
    display: inline-block !important;
    transition: transform 0.22s cubic-bezier(0.4, 0, 0.2, 1) !important;
    transform-origin: center center !important;
  }

  /* When collapsed (Show all), rustdoc applies .will-expand: rotate 180deg */
  #toggle-all-docs.will-expand::before {
    transform: rotate(180deg) !important;
  }
</style>"#;
    fs::write(&head_path, head_html)?;

    // Obsidian top banner
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
    let redirect_markup = generate_redirect_html(&redirect_target)
        .context("Failed to generate redirect HTML")?;

    atomic_write(
        args.out_dir.join("index.html"),
        redirect_markup.as_bytes(),
    )?;

    let git_rev = compute_doc_revision(".", &["src/", "docs/", "README.md"])
        .unwrap_or_else(|_| "rev.clean".to_string());

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

fn patch_distribution_versions(dir: &Path, target_version: &str) -> Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let p = entry.path();
        if p.is_dir() {
            patch_distribution_versions(&p, target_version)?;
        } else if p.extension().map_or(false, |ext| ext == "html" || ext == "js") {
            if let Ok(content) = fs::read_to_string(&p) {
                let mut updated = content.clone();

                // 1. Topbar dropdown default option label
                updated = replace_delimited(&updated, r#"class="hh-active-ver-label">"#, "</option>", &format!("v{}", target_version));

                // 2. Rustdoc sidebar crate version
                updated = replace_delimited(&updated, r#"<span class="version">"#, "</span>", target_version);
                updated = replace_delimited(&updated, r#"class="version">"#, "</", target_version);
                updated = replace_delimited(&updated, r#"<div class="version">"#, "</div>", target_version);

                // 3. Obsolete badges
                updated = replace_delimited(&updated, r#"<span>cargo-docversion v"#, "</span>", &format!("{}</span>", target_version));

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

    let matrix = SchemaMatrix::from_json_slice(&raw)
        .context("Malformed schema matrix JSON manifest")?;

    let metadata = MetadataCommand::new().exec().ok();
    let root_pkg = metadata.as_ref().and_then(|m| {
        m.root_package().or_else(|| m.workspace_packages().first().copied())
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
            let mut available: Vec<String> = matrix.contracts.iter().map(|c| c.schema_id.clone()).collect();
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
