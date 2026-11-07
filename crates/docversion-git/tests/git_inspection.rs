use docversion_git::{compute_doc_revision, is_working_tree_dirty};

#[test]
fn test_compute_doc_revision_fallback() {
    let res = compute_doc_revision(".", &["src/", "Cargo.toml"]);
    assert!(res.is_ok());
    let rev = res.unwrap();
    assert!(rev.starts_with("rev."));
}

#[test]
fn test_working_tree_status() {
    let _dirty = is_working_tree_dirty(".");
}
