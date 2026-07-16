//! Versioned persistence and interchange adapters.

use ori_domain::{CreasePattern, ProjectId};
use serde::{Deserialize, Serialize};
use thiserror::Error;

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
    #[error(
        "project format version {found} is not supported; latest supported version is {latest}"
    )]
    UnsupportedVersion { found: u32, latest: u32 },
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
