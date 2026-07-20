use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::{AssetId, UnderlayId};
pub const BEGINNER_GENERATION_CONSTRAINTS_SCHEMA_VERSION_V1: u32 = 1;
pub const MIN_BEGINNER_GENERATION_STEPS_V1: u16 = 1;
pub const MAX_BEGINNER_GENERATION_STEPS_V1: u16 = 500;
pub const MAX_BEGINNER_ALLOWED_TECHNIQUES_V1: usize = 8;
pub const MAX_BEGINNER_TARGET_PART_RECORDS_V1: usize = 7;
pub const MAX_BEGINNER_TARGET_PART_COUNT_V1: u8 = 8;
pub const MAX_BEGINNER_TARGET_PARTS_TOTAL_V1: u16 = 32;
pub const MAX_BEGINNER_SKELETON_SEGMENTS_V1: usize = 64;
pub const MAX_BEGINNER_SKELETON_COORDINATE_TENTHS_MM_V1: i32 = 100_000;
pub const MAX_BEGINNER_SKELETON_THICKNESS_TENTHS_MM_V1: u16 = 10_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BeginnerFoldTechniqueV1 {
    ValleyFold,
    MountainFold,
    InsideReverseFold,
    OutsideReverseFold,
    SquashFold,
    PetalFold,
    SinkFold,
    CrimpFold,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BeginnerDetailLevelV1 {
    Simple,
    Standard,
    Detailed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BeginnerTargetCategoryV1 {
    Animal,
    Insect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BeginnerTargetPartKindV1 {
    Head,
    Torso,
    Leg,
    Horn,
    Ear,
    Wing,
    Tail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BeginnerTargetPartRecordV1 {
    pub kind: BeginnerTargetPartKindV1,
    pub count: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BeginnerSkeletonPointV1 {
    pub x_tenths_mm: i32,
    pub y_tenths_mm: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BeginnerSkeletonSegmentV1 {
    pub id: u16,
    pub start: BeginnerSkeletonPointV1,
    pub end: BeginnerSkeletonPointV1,
    pub thickness_tenths_mm: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum BeginnerTargetAssetReferenceV1 {
    ReferenceImage {
        underlay_id: UnderlayId,
        asset_id: AssetId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BeginnerGenerationConstraintsV1 {
    pub schema_version: u32,
    pub maximum_steps: u16,
    pub detail_level: BeginnerDetailLevelV1,
    #[serde(default)]
    pub target_category: Option<BeginnerTargetCategoryV1>,
    #[serde(default)]
    pub target_parts: Vec<BeginnerTargetPartRecordV1>,
    #[serde(default)]
    pub skeleton_segments: Vec<BeginnerSkeletonSegmentV1>,
    #[serde(default)]
    pub target_asset: Option<BeginnerTargetAssetReferenceV1>,
    pub allowed_techniques: Vec<BeginnerFoldTechniqueV1>,
}

impl Default for BeginnerGenerationConstraintsV1 {
    fn default() -> Self {
        Self {
            schema_version: BEGINNER_GENERATION_CONSTRAINTS_SCHEMA_VERSION_V1,
            maximum_steps: 60,
            detail_level: BeginnerDetailLevelV1::Standard,
            target_category: None,
            target_parts: Vec::new(),
            skeleton_segments: Vec::new(),
            target_asset: None,
            allowed_techniques: vec![
                BeginnerFoldTechniqueV1::ValleyFold,
                BeginnerFoldTechniqueV1::MountainFold,
                BeginnerFoldTechniqueV1::InsideReverseFold,
                BeginnerFoldTechniqueV1::OutsideReverseFold,
                BeginnerFoldTechniqueV1::SquashFold,
            ],
        }
    }
}

impl BeginnerGenerationConstraintsV1 {
    #[must_use]
    pub fn is_default(&self) -> bool {
        self == &Self::default()
    }
}

#[must_use]
pub fn validate_beginner_generation_constraints_v1(
    constraints: &BeginnerGenerationConstraintsV1,
) -> bool {
    if constraints.schema_version != BEGINNER_GENERATION_CONSTRAINTS_SCHEMA_VERSION_V1
        || !(MIN_BEGINNER_GENERATION_STEPS_V1..=MAX_BEGINNER_GENERATION_STEPS_V1)
            .contains(&constraints.maximum_steps)
        || constraints.allowed_techniques.is_empty()
        || constraints.allowed_techniques.len() > MAX_BEGINNER_ALLOWED_TECHNIQUES_V1
        || constraints.target_parts.len() > MAX_BEGINNER_TARGET_PART_RECORDS_V1
        || constraints.skeleton_segments.len() > MAX_BEGINNER_SKELETON_SEGMENTS_V1
    {
        return false;
    }
    if !constraints.target_parts.is_empty() && constraints.target_category.is_none() {
        return false;
    }
    let mut unique = HashSet::with_capacity(constraints.allowed_techniques.len());
    if !constraints
        .allowed_techniques
        .iter()
        .all(|technique| unique.insert(*technique))
    {
        return false;
    }
    let mut part_kinds = HashSet::with_capacity(constraints.target_parts.len());
    let mut total = 0_u16;
    if !constraints.target_parts.iter().all(|part| {
        total = total.saturating_add(u16::from(part.count));
        (1..=MAX_BEGINNER_TARGET_PART_COUNT_V1).contains(&part.count)
            && total <= MAX_BEGINNER_TARGET_PARTS_TOTAL_V1
            && part_kinds.insert(part.kind)
    }) {
        return false;
    }
    let mut segment_ids = HashSet::with_capacity(constraints.skeleton_segments.len());
    constraints.skeleton_segments.iter().all(|segment| {
        let coordinates = [
            segment.start.x_tenths_mm,
            segment.start.y_tenths_mm,
            segment.end.x_tenths_mm,
            segment.end.y_tenths_mm,
        ];
        coordinates.into_iter().all(|value| {
            value.unsigned_abs() <= MAX_BEGINNER_SKELETON_COORDINATE_TENTHS_MM_V1 as u32
        }) && segment.start != segment.end
            && (1..=MAX_BEGINNER_SKELETON_THICKNESS_TENTHS_MM_V1)
                .contains(&segment.thickness_tenths_mm)
            && segment_ids.insert(segment.id)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generation_constraints_are_bounded_and_reject_duplicates() {
        let valid = BeginnerGenerationConstraintsV1::default();
        assert!(validate_beginner_generation_constraints_v1(&valid));

        let mut duplicate = valid.clone();
        duplicate
            .allowed_techniques
            .push(BeginnerFoldTechniqueV1::ValleyFold);
        assert!(!validate_beginner_generation_constraints_v1(&duplicate));

        let mut unbounded = valid;
        unbounded.maximum_steps = MAX_BEGINNER_GENERATION_STEPS_V1 + 1;
        assert!(!validate_beginner_generation_constraints_v1(&unbounded));

        let mut parts = BeginnerGenerationConstraintsV1::default();
        parts.target_category = Some(BeginnerTargetCategoryV1::Animal);
        parts.target_parts = vec![
            BeginnerTargetPartRecordV1 {
                kind: BeginnerTargetPartKindV1::Head,
                count: 1,
            },
            BeginnerTargetPartRecordV1 {
                kind: BeginnerTargetPartKindV1::Torso,
                count: 1,
            },
        ];
        assert!(validate_beginner_generation_constraints_v1(&parts));
        parts.target_parts.push(BeginnerTargetPartRecordV1 {
            kind: BeginnerTargetPartKindV1::Head,
            count: 1,
        });
        assert!(!validate_beginner_generation_constraints_v1(&parts));
    }
}
