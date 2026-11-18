use std::process::Command;
use assert_fs::fixture::PathChild;

#[test]
fn test_cli_help_flag() {
    let binary = env!("CARGO_BIN_EXE_docversion");
    let output = Command::new(binary)
        .arg("--help")
        .output()
        .expect("Failed to execute docversion binary");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("docversion") || stdout.contains("Usage:"));
}

#[test]
fn test_cli_subcommand_doc_help() {
    let binary = env!("CARGO_BIN_EXE_docversion");
    let output = Command::new(binary)
        .args(["doc", "--help"])
        .output()
        .expect("Failed to execute docversion binary");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Injects an ephemeral version banner")
            || stdout.contains("cargo doc")
            || stdout.contains("doc")
    );
}

#[test]
fn test_cli_verify_db_missing_args_fails() {
    let binary = env!("CARGO_BIN_EXE_docversion");
    let output = Command::new(binary)
        .arg("verify-db")
        .output()
        .expect("Failed to execute docversion binary");

    assert!(!output.status.success());
}

#[test]
fn test_cli_build_site_with_custom_source() {
    let temp = assert_fs::TempDir::new().unwrap();
    let source_dir = temp.child("docs_custom");
    source_dir.create_dir_all().unwrap();
    
    // Production-grade crate manifest
    source_dir.child("Cargo.toml").write_str(r#"
        [package]
        name = "cargo-docversion"
        version = "0.1.0"
        edition = "2021"
    "#).unwrap();

    let target_doc = source_dir.child("target/doc");
    target_doc.create_dir_all().unwrap();
    
    // Production-grade Rustdoc HTML structure
    target_doc.child("index.html").write_str(r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="utf-8">
    <title>cargo_docversion - Rust</title>
    <link rel="stylesheet" type="text/css" href="normalize.css">
    <link rel="stylesheet" type="text/css" href="rustdoc.css" id="mainThemeStyle">
</head>
<body class="rustdoc mod">
    <main>
        <section id="main-content" class="content">
            <h1 class="fqn">Crate <a class="mod" href="#">cargo_docversion</a></h1>
            <block class="docblock">
                <p>Zero-telemetry documentation bundler and LLM context synthesizer.</p>
            </block>
        </section>
    </main>
</body>
</html>"#).unwrap();

    let out_dir = temp.child("site_out");

    let mut cmd = assert_cmd::Command::cargo_bin("docversion").unwrap();
    cmd.arg("build-site")
        .arg("--source")
        .arg(source_dir.path())
        .arg("--version")
        .arg("0.1.0")
        .arg("--entry-page")
        .arg("index.html")
        .arg("--out-dir")
        .arg(out_dir.path());

    cmd.assert().success();
    assert!(out_dir.join("latest/index.html").exists());
}
