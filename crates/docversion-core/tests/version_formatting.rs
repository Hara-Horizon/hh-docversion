use docversion_core::{generate_redirect_html, VersionIdentifier};

#[test]
fn test_version_identifier_with_revision() {
    let ver = VersionIdentifier::new("1.4.2", Some("rev.8a4c1f0")).unwrap();
    assert_eq!(ver.display_version(), "v1.4.2 (rev.8a4c1f0)");
    assert_eq!(ver.target_directory(), "v1.4.2");
}

#[test]
fn test_version_identifier_without_revision() {
    let ver = VersionIdentifier::new("2.0.0", None).unwrap();
    assert_eq!(ver.display_version(), "v2.0.0");
    assert_eq!(ver.target_directory(), "v2.0.0");
}

#[test]
fn test_invalid_semver_fails() {
    let ver = VersionIdentifier::new("not-a-semver", None);
    assert!(ver.is_err());
}

#[test]
fn test_redirect_html_output() {
    let html =
        generate_redirect_html("/latest/my_crate/index.html").expect("Failed to generate redirect");
    assert!(html
        .contains(r#"<meta http-equiv="refresh" content="0; url=/latest/my_crate/index.html">"#));
    assert!(html.contains(r#"<a href="/latest/my_crate/index.html">"#));
}
