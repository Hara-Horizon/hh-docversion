use std::process::Command;
use assert_fs::prelude::*; // or use assert_fs::fixture::PathChild;
use assert_fs::TempDir;
use predicates::prelude::*;

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
    let source_dir = temp.child("docs/dist");
    source_dir.create_dir_all().unwrap();
    source_dir.child("index.html").write_str("<html>Custom Docs</html>").unwrap();

    let out_dir = temp.child("site_out");

    let mut cmd = assert_cmd::Command::cargo_bin("docversion").unwrap();
    cmd.arg("build-site")
        .arg("--source")
        .arg(source_dir.path())
        .arg("--version")
        .arg("1.0.0")
        .arg("--entry-page")
        .arg("index.html")
        .arg("--out-dir")
        .arg(out_dir.path());

    cmd.assert().success();
    assert!(out_dir.join("latest/index.html").exists());
}
