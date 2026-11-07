use std::process::Command;

#[test]
fn test_cli_help_flag() {
    let binary = env!("CARGO_BIN_EXE_cargo-docversion");
    let output = Command::new(binary)
        .arg("--help")
        .output()
        .expect("Failed to execute cargo-docversion binary");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("docversion") || stdout.contains("Usage:"));
}

#[test]
fn test_cli_subcommand_doc_help() {
    let binary = env!("CARGO_BIN_EXE_cargo-docversion");
    let output = Command::new(binary)
        .args(["doc", "--help"])
        .output()
        .expect("Failed to execute cargo-docversion binary");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Injects an ephemeral version banner") || stdout.contains("cargo doc"));
}

#[test]
fn test_cli_verify_db_missing_args_fails() {
    let binary = env!("CARGO_BIN_EXE_cargo-docversion");
    let output = Command::new(binary)
        .arg("verify-db")
        .output()
        .expect("Failed to execute cargo-docversion binary");

    assert!(!output.status.success());
}

#[test]
fn test_cli_build_site_with_custom_source() {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let temp_root = std::env::temp_dir().join(format!("docversion_ext_test_{}", nanos));
    let mock_docs = temp_root.join("vitepress_dist");
    let out_dir = temp_root.join("public_site");

    fs::create_dir_all(&mock_docs).unwrap();
    fs::write(mock_docs.join("index.html"), "<h1>VitePress Documentation</h1>").unwrap();

    let binary = env!("CARGO_BIN_EXE_cargo-docversion");
    let output = Command::new(binary)
        .args([
            "build-site",
            "--source",
            mock_docs.to_str().unwrap(),
            "--version",
            "3.2.1",
            "--entry-page",
            "index.html",
            "--out-dir",
            out_dir.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to execute cargo-docversion binary");

    assert!(output.status.success(), "Command failed: {:?}", String::from_utf8_lossy(&output.stderr));

    // Assert structural directory expectations
    assert!(out_dir.join("latest/index.html").exists());
    assert!(out_dir.join("v3.2.1/index.html").exists());
    assert!(out_dir.join("versions.json").exists());
    assert!(out_dir.join("index.html").exists());

    let redirect_html = fs::read_to_string(out_dir.join("index.html")).unwrap();
    assert!(redirect_html.contains("url=latest/index.html"));

    let _ = fs::remove_dir_all(temp_root);
}
