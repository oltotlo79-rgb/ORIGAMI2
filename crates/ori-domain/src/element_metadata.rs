use serde::{Deserialize, Serialize};

use crate::{EdgeId, FaceId, RgbaColor, VertexId};

pub const MAX_ELEMENT_NAME_CHARS: usize = 120;
pub const MAX_ELEMENT_MEMO_CHARS: usize = 4_000;
pub const MAX_ELEMENT_METADATA_RECORDS: usize = 100_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ElementMetadataV1 {
    pub name: String,
    pub color: Option<RgbaColor>,
    pub memo: String,
}

impl ElementMetadataV1 {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.name.is_empty() && self.color.is_none() && self.memo.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VertexMetadataRecordV1 {
    pub vertex: VertexId,
    pub metadata: ElementMetadataV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EdgeMetadataRecordV1 {
    pub edge: EdgeId,
    pub metadata: ElementMetadataV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FaceMetadataRecordV1 {
    pub face: FaceId,
    pub metadata: ElementMetadataV1,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ElementMetadataDocumentV1 {
    pub vertices: Vec<VertexMetadataRecordV1>,
    pub edges: Vec<EdgeMetadataRecordV1>,
    pub faces: Vec<FaceMetadataRecordV1>,
}

impl ElementMetadataDocumentV1 {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.vertices.is_empty() && self.edges.is_empty() && self.faces.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElementMetadataValidationError {
    TooManyRecords,
    DuplicateVertex(VertexId),
    DuplicateEdge(EdgeId),
    DuplicateFace(FaceId),
    EmptyRecord,
    NameTooLong,
    MemoTooLong,
    UnsupportedControlCharacter,
}

pub fn validate_element_metadata_document_v1(
    document: &ElementMetadataDocumentV1,
) -> Result<(), ElementMetadataValidationError> {
    let total = document
        .vertices
        .len()
        .checked_add(document.edges.len())
        .and_then(|value| value.checked_add(document.faces.len()))
        .ok_or(ElementMetadataValidationError::TooManyRecords)?;
    if total > MAX_ELEMENT_METADATA_RECORDS {
        return Err(ElementMetadataValidationError::TooManyRecords);
    }
    let mut vertices = std::collections::HashSet::with_capacity(document.vertices.len());
    for record in &document.vertices {
        if !vertices.insert(record.vertex) {
            return Err(ElementMetadataValidationError::DuplicateVertex(
                record.vertex,
            ));
        }
        validate_metadata(&record.metadata)?;
    }
    let mut edges = std::collections::HashSet::with_capacity(document.edges.len());
    for record in &document.edges {
        if !edges.insert(record.edge) {
            return Err(ElementMetadataValidationError::DuplicateEdge(record.edge));
        }
        validate_metadata(&record.metadata)?;
    }
    let mut faces = std::collections::HashSet::with_capacity(document.faces.len());
    for record in &document.faces {
        if !faces.insert(record.face) {
            return Err(ElementMetadataValidationError::DuplicateFace(record.face));
        }
        validate_metadata(&record.metadata)?;
    }
    Ok(())
}

pub fn validate_element_metadata_v1(
    metadata: &ElementMetadataV1,
) -> Result<(), ElementMetadataValidationError> {
    validate_metadata(metadata)
}

fn validate_metadata(metadata: &ElementMetadataV1) -> Result<(), ElementMetadataValidationError> {
    if metadata.is_empty() {
        return Err(ElementMetadataValidationError::EmptyRecord);
    }
    if metadata.name.chars().count() > MAX_ELEMENT_NAME_CHARS {
        return Err(ElementMetadataValidationError::NameTooLong);
    }
    if metadata.memo.chars().count() > MAX_ELEMENT_MEMO_CHARS {
        return Err(ElementMetadataValidationError::MemoTooLong);
    }
    if metadata.name.chars().any(char::is_control)
        || metadata
            .memo
            .chars()
            .any(|character| character.is_control() && !matches!(character, '\n' | '\t'))
    {
        return Err(ElementMetadataValidationError::UnsupportedControlCharacter);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_element_kinds_round_trip_and_validate() {
        let metadata = ElementMetadataV1 {
            name: "左上".to_owned(),
            color: Some(RgbaColor::opaque(10, 20, 30)),
            memo: "基準要素\n保持".to_owned(),
        };
        let document = ElementMetadataDocumentV1 {
            vertices: vec![VertexMetadataRecordV1 {
                vertex: VertexId::new(),
                metadata: metadata.clone(),
            }],
            edges: vec![EdgeMetadataRecordV1 {
                edge: EdgeId::new(),
                metadata: metadata.clone(),
            }],
            faces: vec![FaceMetadataRecordV1 {
                face: FaceId::new(),
                metadata,
            }],
        };
        validate_element_metadata_document_v1(&document).expect("valid metadata");
        let restored: ElementMetadataDocumentV1 =
            serde_json::from_slice(&serde_json::to_vec(&document).expect("serialize"))
                .expect("deserialize");
        assert_eq!(restored, document);
    }

    #[test]
    fn empty_duplicate_oversized_and_control_data_fail_closed() {
        assert_eq!(
            validate_element_metadata_v1(&ElementMetadataV1 {
                name: String::new(),
                color: None,
                memo: String::new(),
            }),
            Err(ElementMetadataValidationError::EmptyRecord)
        );
        for metadata in [
            ElementMetadataV1 {
                name: "x".repeat(MAX_ELEMENT_NAME_CHARS + 1),
                color: None,
                memo: String::new(),
            },
            ElementMetadataV1 {
                name: "ok".to_owned(),
                color: None,
                memo: "x".repeat(MAX_ELEMENT_MEMO_CHARS + 1),
            },
            ElementMetadataV1 {
                name: "bad\u{7}".to_owned(),
                color: None,
                memo: String::new(),
            },
        ] {
            assert!(validate_element_metadata_v1(&metadata).is_err());
        }
    }
}
