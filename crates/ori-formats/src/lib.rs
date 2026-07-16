//! Versioned persistence and interchange adapters.

mod ori2;

use ori_domain::{CreasePattern, ProjectId};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use ori2::{
    CURRENT_ORI2_CONTAINER_VERSION, ORI2_CONTAINER_IDENTIFIER, ORI2_MANIFEST_PATH,
    ORI2_PROJECT_PATH, Ori2Limits, Ori2Manifest, Ori2ProjectEntry, read_project_ori2,
    read_project_ori2_with_limits, write_project_ori2, write_project_ori2_with_limits,
};

pub const CURRENT_FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectDocument {
    pub format_version: u32,
    pub project_id: ProjectId,
    pub name: String,
    pub crease_pattern: CreasePattern,
}

impl ProjectDocument {
    #[must_use]
    pub fn new(name: impl Into<String>, crease_pattern: CreasePattern) -> Self {
        Self {
            format_version: CURRENT_FORMAT_VERSION,
            project_id: ProjectId::new(),
            name: name.into(),
            crease_pattern,
        }
    }
}

#[derive(Debug, Error)]
pub enum FormatError {
    #[error("project JSON is invalid: {0}")]
    InvalidJson(#[from] serde_json::Error),
    #[error(".ori2 manifest JSON is invalid: {0}")]
    InvalidManifestJson(#[source] serde_json::Error),
    #[error(".ori2 ZIP data is invalid: {0}")]
    InvalidZip(#[from] zip::result::ZipError),
    #[error(".ori2 ZIP end-of-central-directory record is missing or invalid")]
    InvalidZipFooter,
    #[error("multi-disk .ori2 ZIP archives are not supported")]
    MultiDiskZipNotSupported,
    #[error("ZIP64 .ori2 archives are not supported")]
    Zip64NotSupported,
    #[error(
        ".ori2 ZIP declares {declared} entries, but {parsed} unique entries were parsed; duplicate names are not allowed"
    )]
    ArchiveEntryCountMismatch { declared: usize, parsed: usize },
    #[error("could not read or write .ori2 data: {0}")]
    Io(#[from] std::io::Error),
    #[error(
        "project format version {found} is not supported; latest supported version is {latest}"
    )]
    UnsupportedVersion { found: u32, latest: u32 },
    #[error(".ori2 archive is {actual} bytes; the limit is {limit} bytes")]
    ContainerTooLarge { actual: u64, limit: u64 },
    #[error(".ori2 archive has {actual} entries; the limit is {limit}")]
    TooManyEntries { actual: usize, limit: usize },
    #[error(".ori2 entry path is {actual} bytes; the limit is {limit} bytes")]
    EntryPathTooLong { actual: usize, limit: usize },
    #[error(".ori2 entry path is not safe: {path:?}")]
    UnsafeEntryPath { path: String },
    #[error(".ori2 entry path is not valid UTF-8")]
    NonUtf8EntryPath,
    #[error(".ori2 is missing the required entry: {path}")]
    MissingEntry { path: &'static str },
    #[error("required .ori2 entry is a directory: {path}")]
    RequiredEntryIsDirectory { path: &'static str },
    #[error("encrypted .ori2 entries are not supported: {path}")]
    EncryptedEntry { path: String },
    #[error(".ori2 entry {path} is {actual} bytes; the limit is {limit} bytes")]
    EntryTooLarge {
        path: String,
        actual: u64,
        limit: u64,
    },
    #[error(".ori2 expands to {actual} bytes; the limit is {limit} bytes")]
    ExpandedArchiveTooLarge { actual: u64, limit: u64 },
    #[error("unexpected .ori2 container identifier {found:?}")]
    InvalidContainerIdentifier { found: String },
    #[error(
        ".ori2 container version {found} is not supported; latest supported version is {latest}"
    )]
    UnsupportedContainerVersion { found: u32, latest: u32 },
    #[error(".ori2 requires unsupported features: {features:?}")]
    UnsupportedRequiredFeatures { features: Vec<String> },
    #[error(".ori2 manifest references an invalid project path: {found:?}")]
    InvalidManifestProjectPath { found: String },
    #[error(
        ".ori2 manifest declares project size {declared} bytes, but project.json is {actual} bytes"
    )]
    ProjectSizeMismatch { declared: u64, actual: u64 },
    #[error(".ori2 project checksum differs (expected {expected}, actual {actual})")]
    ProjectHashMismatch { expected: String, actual: String },
    #[error(
        ".ori2 manifest declares project format version {manifest}, but project.json declares {project}"
    )]
    ManifestProjectVersionMismatch { manifest: u32, project: u32 },
}

pub fn write_project_json(document: &ProjectDocument) -> Result<Vec<u8>, FormatError> {
    Ok(serde_json::to_vec_pretty(document)?)
}

pub fn read_project_json(bytes: &[u8]) -> Result<ProjectDocument, FormatError> {
    let document: ProjectDocument = serde_json::from_slice(bytes)?;
    if document.format_version != CURRENT_FORMAT_VERSION {
        return Err(FormatError::UnsupportedVersion {
            found: document.format_version,
            latest: CURRENT_FORMAT_VERSION,
        });
    }
    Ok(document)
}

#[cfg(test)]
mod tests {
    use ori_domain::{Edge, EdgeId, EdgeKind, Point2, Vertex, VertexId};

    use super::*;

    fn sample_document() -> ProjectDocument {
        let start = VertexId::new();
        let end = VertexId::new();
        ProjectDocument::new(
            "Mountain fold",
            CreasePattern {
                vertices: vec![
                    Vertex {
                        id: start,
                        position: Point2::new(0.0, 0.0),
                    },
                    Vertex {
                        id: end,
                        position: Point2::new(10.0, 4.0),
                    },
                ],
                edges: vec![Edge {
                    id: EdgeId::new(),
                    start,
                    end,
                    kind: EdgeKind::Mountain,
                }],
            },
        )
    }

    #[test]
    fn json_round_trip_preserves_ids_geometry_and_kinds() {
        let original = sample_document();
        let bytes = write_project_json(&original).expect("write project");
        let restored = read_project_json(&bytes).expect("read project");
        assert_eq!(restored, original);
    }

    #[test]
    fn rejects_unknown_format_version() {
        let mut document = sample_document();
        document.format_version = CURRENT_FORMAT_VERSION + 1;
        let bytes = serde_json::to_vec(&document).expect("serialize future project");
        let error = read_project_json(&bytes).expect_err("future version must fail");
        assert!(matches!(
            error,
            FormatError::UnsupportedVersion {
                found: 2,
                latest: 1
            }
        ));
    }
}
