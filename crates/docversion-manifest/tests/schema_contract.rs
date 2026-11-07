use docversion_manifest::{SchemaContract, SchemaMatrix};

fn build_test_matrix() -> SchemaMatrix {
    SchemaMatrix {
        generated_at: "2026-11-07T00:00:00Z".to_string(),
        contracts: vec![
            SchemaContract {
                schema_id: "core-ledger".to_string(),
                schema_version: 4,
                supported_app_range: ">=1.4.0, <2.0.0".to_string(),
                artifact_uri: "https://example.com/core-ledger-v4.sqlite".to_string(),
                sha256: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".to_string(),
            },
            SchemaContract {
                schema_id: "identity-vault".to_string(),
                schema_version: 1,
                supported_app_range: "^0.9.0".to_string(),
                artifact_uri: "https://example.com/identity-v1.sqlite".to_string(),
                sha256: "ca978112ca1bbdcafac231b39a23dc4da786eff8147c4e72b9807785afee48bb".to_string(),
            },
        ],
    }
}

#[test]
fn test_contract_validation_success() {
    let matrix = build_test_matrix();
    let contract = matrix.verify_compatibility("1.5.2", "core-ledger", 4);
    assert!(contract.is_ok());
    assert_eq!(contract.unwrap().schema_version, 4);
}

#[test]
fn test_contract_app_version_lower_bound_rejection() {
    let matrix = build_test_matrix();
    let res = matrix.verify_compatibility("1.3.9", "core-ledger", 4);
    assert!(res.is_err());
}

#[test]
fn test_contract_app_version_upper_bound_rejection() {
    let matrix = build_test_matrix();
    let res = matrix.verify_compatibility("2.0.0", "core-ledger", 4);
    assert!(res.is_err());
}

#[test]
fn test_contract_missing_schema_rejection() {
    let matrix = build_test_matrix();
    let res = matrix.verify_compatibility("1.5.0", "nonexistent-schema", 1);
    assert!(res.is_err());
}

#[test]
fn test_json_manifest_deserialization() {
    let raw = r#"{
      "generated_at": "2026-11-07T00:00:00Z",
      "contracts": [
        {
          "schema_id": "test-db",
          "schema_version": 2,
          "supported_app_range": "^1.0.0",
          "artifact_uri": "https://example.com/test.sqlite",
          "sha256": "abcdef"
        }
      ]
    }"#;
    let matrix = SchemaMatrix::from_json_slice(raw.as_bytes()).unwrap();
    assert!(matrix.verify_compatibility("1.1.0", "test-db", 2).is_ok());
}
