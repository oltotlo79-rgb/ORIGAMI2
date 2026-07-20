use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{AssetId, LayerId, Point2};

pub const UNDERLAY_SCHEMA_VERSION_V1: u32 = 1;
pub const MAX_UNDERLAYS_V1: usize = 256;
pub const MIN_UNDERLAY_SCALE_V1: f64 = 0.000_001;
pub const MAX_UNDERLAY_SCALE_V1: f64 = 1_000_000.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct UnderlayId(Uuid);

impl UnderlayId {
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for UnderlayId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnderlayTransformV1 {
    pub position: Point2,
    pub scale_x: f64,
    pub scale_y: f64,
    pub rotation_degrees: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnderlayRecordV1 {
    pub id: UnderlayId,
    pub asset: AssetId,
    pub transform: UnderlayTransformV1,
    pub opacity: f64,
    pub layer: LayerId,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct UnderlayDocumentV1 {
    pub schema_version: u32,
    pub underlays: Vec<UnderlayRecordV1>,
}

impl Default for UnderlayDocumentV1 {
    fn default() -> Self {
        Self { schema_version: UNDERLAY_SCHEMA_VERSION_V1, underlays: Vec::new() }
    }
}

impl UnderlayDocumentV1 {
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.underlays.is_empty()
    }
}

pub fn validate_underlay_document_v1(document: &UnderlayDocumentV1) -> Result<(), &'static str> {
    if document.schema_version != UNDERLAY_SCHEMA_VERSION_V1 {
        return Err("unsupported underlay schema");
    }
    if document.underlays.len() > MAX_UNDERLAYS_V1 {
        return Err("too many underlays");
    }
    let mut ids = HashSet::with_capacity(document.underlays.len());
    for underlay in &document.underlays {
        if !ids.insert(underlay.id) {
            return Err("duplicate underlay id");
        }
        let transform = underlay.transform;
        if !transform.position.x.is_finite()
            || !transform.position.y.is_finite()
            || !transform.scale_x.is_finite()
            || !transform.scale_y.is_finite()
            || !transform.rotation_degrees.is_finite()
            || !(MIN_UNDERLAY_SCALE_V1..=MAX_UNDERLAY_SCALE_V1).contains(&transform.scale_x.abs())
            || !(MIN_UNDERLAY_SCALE_V1..=MAX_UNDERLAY_SCALE_V1).contains(&transform.scale_y.abs())
            || !underlay.opacity.is_finite()
            || !(0.0..=1.0).contains(&underlay.opacity)
        {
            return Err("invalid underlay transform or opacity");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record() -> UnderlayRecordV1 {
        UnderlayRecordV1 {
            id: UnderlayId::new(),
            asset: AssetId::new(),
            transform: UnderlayTransformV1 {
                position: Point2::new(10.0, 20.0),
                scale_x: 1.0,
                scale_y: 1.0,
                rotation_degrees: 15.0,
            },
            opacity: 0.5,
            layer: LayerId::new(),
        }
    }

    #[test]
    fn validates_strict_bounded_document() {
        let document = UnderlayDocumentV1 { schema_version: 1, underlays: vec![record()] };
        assert_eq!(validate_underlay_document_v1(&document), Ok(()));
        let value = serde_json::to_value(document).unwrap();
        assert!(serde_json::from_value::<UnderlayDocumentV1>(value).is_ok());
    }

    #[test]
    fn rejects_duplicate_nonfinite_and_zero_scale() {
        let first = record();
        let mut second = first.clone();
        second.transform.scale_x = 0.0;
        let document = UnderlayDocumentV1 { schema_version: 1, underlays: vec![first, second] };
        assert!(validate_underlay_document_v1(&document).is_err());
    }
}
