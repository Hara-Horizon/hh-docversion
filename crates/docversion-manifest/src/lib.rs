use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{self, BufReader, Write};
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ContractError {
    #[error("SemVer parsing error: {0}")]
    Semver(#[from] semver::Error),
    #[error("Artifact I/O failure: {0}")]
    Io(#[from] io::Error),
    #[error("Contract violation: App v{app_ver} fails requirements '{range}' for schema '{schema_id}' v{schema_ver}")]
    RangeMismatch {
        app_ver: Version,
        range: String,
        schema_id: String,
        schema_ver: u32,
    },
    #[error("Contract absent: Schema '{schema_id}' v{schema_ver} is not declared in manifest")]
    NotFound { schema_id: String, schema_ver: u32 },
    #[error("Integrity violation for '{path}': expected {expected}, calculated {actual}")]
    ChecksumMismatch {
        path: String,
        expected: String,
        actual: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SchemaContract {
    pub schema_id: String,
    pub schema_version: u32,
    pub supported_app_range: String,
    pub artifact_uri: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaMatrix {
    pub generated_at: String,
    pub contracts: Vec<SchemaContract>,
}

struct Sha256Writer<D: Digest> {
    hasher: D,
}

impl<D: Digest> Write for Sha256Writer<D> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.hasher.update(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl SchemaMatrix {
    /// Deserializes a compatibility matrix from JSON bytes.
    ///
    /// ```rust
    /// use docversion_manifest::SchemaMatrix;
    ///
    /// let raw = br#"{"generated_at":"2026-11-07T00:00:00Z","contracts":[]}"#;
    /// let matrix = SchemaMatrix::from_json_slice(raw).unwrap();
    /// assert!(matrix.contracts.is_empty());
    /// ```
    pub fn from_json_slice(data: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(data)
    }

    /// Verifies whether the specified application version satisfies the schema contract requirement.
    ///
    /// ```rust
    /// use docversion_manifest::{SchemaContract, SchemaMatrix};
    ///
    /// let matrix = SchemaMatrix {
    ///     generated_at: "2026-11-07T00:00:00Z".into(),
    ///     contracts: vec![SchemaContract {
    ///         schema_id: "core-ledger".into(),
    ///         schema_version: 4,
    ///         supported_app_range: ">=1.4.0, <2.0.0".into(),
    ///         artifact_uri: "[https://example.com/db.sqlite](https://example.com/db.sqlite)".into(),
    ///         sha256: "abc".into(),
    ///     }],
    /// };
    ///
    /// assert!(matrix.verify_compatibility("1.5.0", "core-ledger", 4).is_ok());
    /// assert!(matrix.verify_compatibility("2.1.0", "core-ledger", 4).is_err());
    /// ```
    pub fn verify_compatibility(
        &self,
        app_version_str: &str,
        target_schema_id: &str,
        actual_db_version: u32,
    ) -> Result<&SchemaContract, ContractError> {
        let app_ver = Version::parse(app_version_str.trim())?;

        let contract = self
            .contracts
            .iter()
            .find(|c| c.schema_id == target_schema_id && c.schema_version == actual_db_version)
            .ok_or_else(|| ContractError::NotFound {
                schema_id: target_schema_id.to_string(),
                schema_ver: actual_db_version,
            })?;

        let req = VersionReq::parse(&contract.supported_app_range)?;
        if !req.matches(&app_ver) {
            return Err(ContractError::RangeMismatch {
                app_ver,
                range: contract.supported_app_range.clone(),
                schema_id: target_schema_id.to_string(),
                schema_ver: actual_db_version,
            });
        }

        Ok(contract)
    }

    /// Computes the artifact digest using a zero-allocation buffered streaming pipeline.
    pub fn verify_file_checksum<P: AsRef<Path>>(
        file_path: P,
        expected_sha256: &str,
    ) -> Result<(), ContractError> {
        let file = File::open(file_path.as_ref())?;
        let mut reader = BufReader::with_capacity(64 * 1024, file);
        let mut writer = Sha256Writer {
            hasher: Sha256::new(),
        };

        io::copy(&mut reader, &mut writer)?;
        let actual_hash = format!("{:x}", writer.hasher.finalize());

        if !actual_hash.eq_ignore_ascii_case(expected_sha256.trim()) {
            return Err(ContractError::ChecksumMismatch {
                path: file_path.as_ref().display().to_string(),
                expected: expected_sha256.to_string(),
                actual: actual_hash,
            });
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn test_matrix_contract_verification() {
        let matrix = SchemaMatrix {
            generated_at: "2026-11-07T00:00:00Z".into(),
            contracts: vec![SchemaContract {
                schema_id: "core-ledger".into(),
                schema_version: 4,
                supported_app_range: ">=1.4.0, <2.0.0".into(),
                artifact_uri: "https://example.com/db.sqlite".into(),
                sha256: "abc".into(),
            }],
        };

        assert!(matrix
            .verify_compatibility("1.5.0", "core-ledger", 4)
            .is_ok());
        assert!(matrix
            .verify_compatibility("2.1.0", "core-ledger", 4)
            .is_err());
        assert!(matrix
            .verify_compatibility("1.5.0", "core-ledger", 5)
            .is_err());
    }

    #[test]
    fn test_streaming_checksum_verification() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let tmp_path = std::env::temp_dir().join(format!("docversion_stream_test_{}.bin", nanos));

        {
            let mut file = fs::File::create(&tmp_path).unwrap();
            file.write_all(b"deterministic-payload").unwrap();
            file.flush().unwrap();
        }

        let expected = "ba93a56a8fe9befface03cc6b575aebef8cb8cde4d259452aeea11cab154fb74";
        assert!(SchemaMatrix::verify_file_checksum(&tmp_path, expected).is_ok());
        assert!(SchemaMatrix::verify_file_checksum(&tmp_path, "deadbeef").is_err());

        let _ = fs::remove_file(tmp_path);
    }
}
