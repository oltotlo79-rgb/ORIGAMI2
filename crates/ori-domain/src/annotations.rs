use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{LayerId, Point2, RgbaColor, VertexId};

pub const ANNOTATION_SCHEMA_VERSION_V1: u32 = 1;
pub const MAX_ANNOTATIONS_V1: usize = 10_000;
pub const MAX_ANNOTATION_TEXT_BYTES_V1: usize = 4_096;
pub const MIN_ANNOTATION_FONT_SIZE_MM_V1: f64 = 0.5;
pub const MAX_ANNOTATION_FONT_SIZE_MM_V1: f64 = 200.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AnnotationId(Uuid);

impl AnnotationId {
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    #[must_use]
    pub const fn canonical_bytes(&self) -> [u8; 16] {
        self.0.into_bytes()
    }
}

impl Default for AnnotationId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnnotationStyleV1 {
    pub color: RgbaColor,
    pub font_size_mm: f64,
    pub bold: bool,
    pub italic: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnnotationRecordV1 {
    pub id: AnnotationId,
    pub text: String,
    pub anchor: AnnotationAnchorV1,
    pub style: AnnotationStyleV1,
    pub layer: LayerId,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum AnnotationAnchorV1 {
    Absolute { position: Point2 },
    Vertex { vertex: VertexId, offset: Point2 },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AnnotationDocumentV1 {
    pub schema_version: u32,
    pub annotations: Vec<AnnotationRecordV1>,
}

impl Default for AnnotationDocumentV1 {
    fn default() -> Self {
        Self {
            schema_version: ANNOTATION_SCHEMA_VERSION_V1,
            annotations: Vec::new(),
        }
    }
}

impl AnnotationDocumentV1 {
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.annotations.is_empty()
    }
}

pub fn validate_annotation_document_v1(
    document: &AnnotationDocumentV1,
) -> Result<(), &'static str> {
    if document.schema_version != ANNOTATION_SCHEMA_VERSION_V1 {
        return Err("unsupported annotation schema");
    }
    if document.annotations.len() > MAX_ANNOTATIONS_V1 {
        return Err("too many annotations");
    }
    let mut ids = HashSet::with_capacity(document.annotations.len());
    for annotation in &document.annotations {
        if !ids.insert(annotation.id) {
            return Err("duplicate annotation id");
        }
        if annotation.text.is_empty()
            || annotation.text.len() > MAX_ANNOTATION_TEXT_BYTES_V1
            || annotation.text.chars().any(char::is_control)
        {
            return Err("invalid annotation text");
        }
        let anchor_point = match annotation.anchor {
            AnnotationAnchorV1::Absolute { position } => position,
            AnnotationAnchorV1::Vertex { offset, .. } => offset,
        };
        if !anchor_point.x.is_finite()
            || !anchor_point.y.is_finite()
            || !annotation.style.font_size_mm.is_finite()
            || !(MIN_ANNOTATION_FONT_SIZE_MM_V1..=MAX_ANNOTATION_FONT_SIZE_MM_V1)
                .contains(&annotation.style.font_size_mm)
        {
            return Err("invalid annotation geometry or style");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strict_annotation_document_accepts_one_bounded_record() {
        let document = AnnotationDocumentV1 {
            schema_version: 1,
            annotations: vec![AnnotationRecordV1 {
                id: AnnotationId::new(),
                text: "Fold here".to_owned(),
                anchor: AnnotationAnchorV1::Absolute {
                    position: Point2::new(1.0, 2.0),
                },
                style: AnnotationStyleV1 {
                    color: RgbaColor {
                        red: 0,
                        green: 0,
                        blue: 0,
                        alpha: 255,
                    },
                    font_size_mm: 4.0,
                    bold: false,
                    italic: false,
                },
                layer: LayerId::new(),
            }],
        };
        assert_eq!(validate_annotation_document_v1(&document), Ok(()));
        let encoded = serde_json::to_value(&document).unwrap();
        assert!(serde_json::from_value::<AnnotationDocumentV1>(encoded).is_ok());
    }

    #[test]
    fn annotation_document_rejects_nonfinite_and_control_text() {
        let mut document = AnnotationDocumentV1 {
            schema_version: 1,
            annotations: vec![AnnotationRecordV1 {
                id: AnnotationId::new(),
                text: "bad\ntext".to_owned(),
                anchor: AnnotationAnchorV1::Absolute {
                    position: Point2::new(0.0, 0.0),
                },
                style: AnnotationStyleV1 {
                    color: RgbaColor {
                        red: 0,
                        green: 0,
                        blue: 0,
                        alpha: 255,
                    },
                    font_size_mm: 4.0,
                    bold: false,
                    italic: false,
                },
                layer: LayerId::new(),
            }],
        };
        assert!(validate_annotation_document_v1(&document).is_err());
        document.annotations[0].text = "ok".to_owned();
        document.annotations[0].anchor = AnnotationAnchorV1::Absolute {
            position: Point2::new(f64::NAN, 0.0),
        };
        assert!(validate_annotation_document_v1(&document).is_err());
    }
}
