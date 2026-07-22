use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use unicode_normalization::UnicodeNormalization;

use crate::{AssetId, FaceId, UnderlayId};
pub const BEGINNER_GENERATION_CONSTRAINTS_SCHEMA_VERSION_V1: u32 = 1;
pub const MIN_BEGINNER_GENERATION_STEPS_V1: u16 = 1;
pub const MAX_BEGINNER_GENERATION_STEPS_V1: u16 = 500;
pub const MAX_BEGINNER_ALLOWED_TECHNIQUES_V1: usize = 8;
pub const MAX_BEGINNER_TARGET_PART_RECORDS_V1: usize = 8;
pub const MAX_BEGINNER_TARGET_PART_COUNT_V1: u8 = 8;
pub const MAX_BEGINNER_TARGET_PARTS_TOTAL_V1: u16 = 32;
pub const MAX_BEGINNER_SKELETON_SEGMENTS_V1: usize = 64;
pub const MAX_BEGINNER_SKELETON_COORDINATE_TENTHS_MM_V1: i32 = 100_000;
pub const MAX_BEGINNER_SKELETON_THICKNESS_TENTHS_MM_V1: u16 = 10_000;
pub const MAX_BEGINNER_PROTRUSIONS_V1: usize = 32;
pub const MAX_BEGINNER_BULGE_TARGETS_V1: usize = 32;
pub const MAX_BEGINNER_BULGE_FACES_V1: usize = 32;
pub const MAX_BEGINNER_CUSTOM_OBJECT_DISPLAY_NAME_CHARS_V1: usize = 64;
pub const BEGINNER_CUSTOM_OBJECT_DISPLAY_NAME_V1: &str = "Custom object";

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

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BeginnerBodyOutlineModeV1 {
    #[default]
    Symmetric,
    General,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BeginnerTargetCategoryV1 {
    Animal,
    Insect,
    CustomObject,
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
    Fin,
    Antenna,
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
#[serde(rename_all = "snake_case")]
pub enum BeginnerProtrusionSymmetryV1 {
    None,
    Bilateral,
    Radial,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BeginnerProtrusionJointV1 {
    Fixed,
    Hinge,
    Ball,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BeginnerProtrusionSideV1 {
    Front,
    Back,
    Either,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BeginnerProtrusionTargetV1 {
    pub id: u16,
    pub count: u8,
    pub length_tenths_mm: u32,
    pub thickness_tenths_mm: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_width_tenths_mm: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tip_width_tenths_mm: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_outline_tenths_mm: Option<Vec<[i32; 2]>>,
    pub position_tenths_mm: [i32; 3],
    pub direction_milli: [i16; 3],
    pub symmetry: BeginnerProtrusionSymmetryV1,
    pub curvature_degrees: i16,
    pub joint: BeginnerProtrusionJointV1,
    pub motion_degrees: [i16; 2],
    pub side: BeginnerProtrusionSideV1,
    pub priority: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BeginnerBulgeTargetV1 {
    pub id: u16,
    pub face_ids: Vec<FaceId>,
    pub range_min_tenths_mm: [i32; 3],
    pub range_max_tenths_mm: [i32; 3],
    pub direction_milli: [i16; 3],
    pub amount_tenths_mm: u32,
    pub source_fold_model_fingerprint: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference_surface_binding: Option<BeginnerReferenceSurfaceBindingV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BeginnerReferenceSurfaceBindingV1 {
    pub asset_id: AssetId,
    pub range_id: u16,
    pub protrusion_id: u16,
    pub triangle_indices: Vec<u32>,
    pub range_digest_sha256: [u8; 32],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum BeginnerTargetAssetReferenceV1 {
    ReferenceImage {
        underlay_id: UnderlayId,
        asset_id: AssetId,
    },
    /// A passive visual reference. This grants no recognition or generation authority.
    ReferenceModel { asset_id: AssetId },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BeginnerGenerationConstraintsV1 {
    pub schema_version: u32,
    pub maximum_steps: u16,
    pub detail_level: BeginnerDetailLevelV1,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generic_body_size_tenths_mm: Option<[u32; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generic_body_outline_tenths_mm: Option<Vec<[i32; 2]>>,
    #[serde(default)]
    pub generic_body_outline_mode: BeginnerBodyOutlineModeV1,
    #[serde(default)]
    pub target_category: Option<BeginnerTargetCategoryV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_object_display_name: Option<String>,
    #[serde(default)]
    pub target_parts: Vec<BeginnerTargetPartRecordV1>,
    #[serde(default)]
    pub skeleton_segments: Vec<BeginnerSkeletonSegmentV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub component_bridge_override: Option<BeginnerComponentBridgeOverrideV1>,
    #[serde(default)]
    pub protrusions: Vec<BeginnerProtrusionTargetV1>,
    #[serde(default)]
    pub bulge_targets: Vec<BeginnerBulgeTargetV1>,
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
            generic_body_size_tenths_mm: None,
            generic_body_outline_tenths_mm: None,
            generic_body_outline_mode: BeginnerBodyOutlineModeV1::Symmetric,
            target_category: None,
            custom_object_display_name: None,
            target_parts: Vec::new(),
            skeleton_segments: Vec::new(),
            component_bridge_override: None,
            protrusions: Vec::new(),
            bulge_targets: Vec::new(),
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BeginnerComponentBridgeOverrideV1 {
    pub schema_version: u32,
    pub source_asset_sha256: [u8; 32],
    pub component_count: u8,
    pub reviewed: bool,
    pub bridges: Vec<BeginnerComponentBridgeRecordV1>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BeginnerComponentBridgeRecordV1 {
    pub id: u8,
    pub start_component_id: u8,
    pub end_component_id: u8,
    pub accepted: bool,
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
        || constraints
            .component_bridge_override
            .as_ref()
            .is_some_and(|document| {
                document.schema_version != 1
                    || !(2..=8).contains(&document.component_count)
                    || document.bridges.len() > 7
                    || document.bridges.iter().enumerate().any(|(index, bridge)| {
                        bridge.id as usize != index
                            || bridge.start_component_id >= document.component_count
                            || bridge.end_component_id >= document.component_count
                            || bridge.start_component_id == bridge.end_component_id
                    })
            })
        || constraints.protrusions.len() > MAX_BEGINNER_PROTRUSIONS_V1
        || constraints.bulge_targets.len() > MAX_BEGINNER_BULGE_TARGETS_V1
    {
        return false;
    }
    if !validate_custom_object_display_name_v1(
        constraints.target_category,
        constraints.custom_object_display_name.as_deref(),
    ) {
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
    let mut total = 0_u16;
    if !constraints.target_parts.iter().all(|part| {
        total = total.saturating_add(u16::from(part.count));
        (1..=MAX_BEGINNER_TARGET_PART_COUNT_V1).contains(&part.count)
            && total <= MAX_BEGINNER_TARGET_PARTS_TOTAL_V1
    }) {
        return false;
    }
    let mut segment_ids = HashSet::with_capacity(constraints.skeleton_segments.len());
    let skeletons_valid = constraints.skeleton_segments.iter().all(|segment| {
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
    });
    let mut protrusion_ids = HashSet::with_capacity(constraints.protrusions.len());
    let body_size_valid = constraints
        .generic_body_size_tenths_mm
        .is_none_or(|size| size.into_iter().all(|axis| (1..=1_000_000).contains(&axis)));
    let body_outline_valid = constraints
        .generic_body_outline_tenths_mm
        .as_deref()
        .is_none_or(|points| {
            valid_generic_body_outline_v1(points, constraints.generic_body_outline_mode)
        });
    let protrusions_valid = skeletons_valid
        && body_size_valid
        && body_outline_valid
        && constraints.protrusions.iter().all(|target| {
            (1..=8).contains(&target.count)
                && (1..=1_000_000).contains(&target.length_tenths_mm)
                && (1..=10_000).contains(&target.thickness_tenths_mm)
                && target
                    .root_width_tenths_mm
                    .is_none_or(|width| (1..=10_000).contains(&width))
                && target
                    .tip_width_tenths_mm
                    .is_none_or(|width| (1..=10_000).contains(&width))
                && target
                    .local_outline_tenths_mm
                    .as_deref()
                    .is_none_or(|points| valid_protrusion_local_outline_v1(points, target.symmetry))
                && target
                    .position_tenths_mm
                    .iter()
                    .all(|value| value.unsigned_abs() <= 100_000)
                && target
                    .direction_milli
                    .iter()
                    .all(|value| value.unsigned_abs() <= 1_000)
                && target.direction_milli != [0, 0, 0]
                && (-360..=360).contains(&target.curvature_degrees)
                && (-360..=360).contains(&target.motion_degrees[0])
                && (-360..=360).contains(&target.motion_degrees[1])
                && target.motion_degrees[0] <= target.motion_degrees[1]
                && (1..=100).contains(&target.priority)
                && protrusion_ids.insert(target.id)
        });
    let mut bulge_ids = HashSet::with_capacity(constraints.bulge_targets.len());
    protrusions_valid
        && constraints.bulge_targets.iter().all(|target| {
            !target.face_ids.is_empty()
                && target.face_ids.len() <= MAX_BEGINNER_BULGE_FACES_V1
                && target.face_ids.iter().collect::<HashSet<_>>().len() == target.face_ids.len()
                && target
                    .range_min_tenths_mm
                    .iter()
                    .zip(target.range_max_tenths_mm)
                    .all(|(minimum, maximum)| {
                        minimum <= &maximum
                            && minimum.unsigned_abs() <= 100_000
                            && maximum.unsigned_abs() <= 100_000
                    })
                && target.range_min_tenths_mm != target.range_max_tenths_mm
                && target
                    .direction_milli
                    .iter()
                    .all(|value| value.unsigned_abs() <= 1_000)
                && target.direction_milli != [0, 0, 0]
                && (1..=1_000_000).contains(&target.amount_tenths_mm)
                && target.source_fold_model_fingerprint.len() == 64
                && target
                    .source_fold_model_fingerprint
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit())
                && target
                    .reference_surface_binding
                    .as_ref()
                    .is_none_or(|binding| {
                        binding.range_id > 0
                            && binding.protrusion_id > 0
                            && !binding.triangle_indices.is_empty()
                            && binding.triangle_indices.len() <= 40_000
                            && binding
                                .triangle_indices
                                .iter()
                                .collect::<HashSet<_>>()
                                .len()
                                == binding.triangle_indices.len()
                    })
                && bulge_ids.insert(target.id)
        })
}

#[must_use]
pub fn validate_custom_object_display_name_v1(
    category: Option<BeginnerTargetCategoryV1>,
    name: Option<&str>,
) -> bool {
    match (category, name) {
        (Some(BeginnerTargetCategoryV1::CustomObject), None) => true,
        (Some(BeginnerTargetCategoryV1::CustomObject), Some(value)) => {
            let count = value.chars().count();
            count >= 1
                && count <= MAX_BEGINNER_CUSTOM_OBJECT_DISPLAY_NAME_CHARS_V1
                && value.trim() == value
                && value.nfc().eq(value.chars())
                && !value.chars().any(|character| {
                    character.is_control()
                        || matches!(character, '/' | '\\')
                        || matches!(character as u32, 0x202A..=0x202E | 0x2066..=0x2069)
                })
        }
        (_, None) => true,
        (_, Some(_)) => false,
    }
}

#[must_use]
pub fn custom_object_display_name_v1(
    constraints: &BeginnerGenerationConstraintsV1,
) -> Option<&str> {
    (constraints.target_category == Some(BeginnerTargetCategoryV1::CustomObject)).then(|| {
        constraints
            .custom_object_display_name
            .as_deref()
            .unwrap_or(BEGINNER_CUSTOM_OBJECT_DISPLAY_NAME_V1)
    })
}

fn valid_generic_body_outline_v1(points: &[[i32; 2]], mode: BeginnerBodyOutlineModeV1) -> bool {
    if !(4..=16).contains(&points.len())
        || points
            .iter()
            .any(|point| point.iter().any(|axis| axis.unsigned_abs() > 100_000))
        || points.iter().collect::<HashSet<_>>().len() != points.len()
        || points[0] != *points.iter().min().expect("non-empty bounded outline")
    {
        return false;
    }
    let twice_area = points
        .iter()
        .enumerate()
        .fold(0_i128, |sum, (index, point)| {
            let next = points[(index + 1) % points.len()];
            sum + i128::from(point[0]) * i128::from(next[1])
                - i128::from(next[0]) * i128::from(point[1])
        });
    let winding_valid = match mode {
        BeginnerBodyOutlineModeV1::Symmetric => twice_area < 0,
        BeginnerBodyOutlineModeV1::General => twice_area > 0,
    };
    let symmetry_valid = mode == BeginnerBodyOutlineModeV1::General
        || points
            .iter()
            .all(|point| points.contains(&[-point[0], point[1]]));
    if !winding_valid || !symmetry_valid {
        return false;
    }
    for first in 0..points.len() {
        let first_end = (first + 1) % points.len();
        for second in (first + 1)..points.len() {
            let second_end = (second + 1) % points.len();
            if first == second_end || first_end == second {
                continue;
            }
            if segments_intersect_v1(
                points[first],
                points[first_end],
                points[second],
                points[second_end],
            ) {
                return false;
            }
        }
    }
    true
}

fn segments_intersect_v1(a: [i32; 2], b: [i32; 2], c: [i32; 2], d: [i32; 2]) -> bool {
    fn orient(a: [i32; 2], b: [i32; 2], c: [i32; 2]) -> i128 {
        (i128::from(b[0]) - i128::from(a[0])) * (i128::from(c[1]) - i128::from(a[1]))
            - (i128::from(b[1]) - i128::from(a[1])) * (i128::from(c[0]) - i128::from(a[0]))
    }
    let values = [
        orient(a, b, c),
        orient(a, b, d),
        orient(c, d, a),
        orient(c, d, b),
    ];
    values.contains(&0)
        || (values[0].signum() != values[1].signum() && values[2].signum() != values[3].signum())
}

fn valid_protrusion_local_outline_v1(
    points: &[[i32; 2]],
    symmetry: BeginnerProtrusionSymmetryV1,
) -> bool {
    if !(3..=8).contains(&points.len())
        || points
            .iter()
            .any(|point| point.iter().any(|axis| axis.unsigned_abs() > 10_000))
        || points.iter().collect::<HashSet<_>>().len() != points.len()
        || points[0]
            != *points
                .iter()
                .min()
                .expect("non-empty bounded local outline")
        || (symmetry == BeginnerProtrusionSymmetryV1::Bilateral
            && points
                .iter()
                .any(|point| !points.contains(&[-point[0], point[1]])))
    {
        return false;
    }
    let twice_area = points
        .iter()
        .enumerate()
        .fold(0_i128, |sum, (index, point)| {
            let next = points[(index + 1) % points.len()];
            sum + i128::from(point[0]) * i128::from(next[1])
                - i128::from(next[0]) * i128::from(point[1])
        });
    if twice_area <= 0 {
        return false;
    }
    for first in 0..points.len() {
        let first_end = (first + 1) % points.len();
        for second in (first + 1)..points.len() {
            let second_end = (second + 1) % points.len();
            if first == second_end || first_end == second {
                continue;
            }
            if segments_intersect_v1(
                points[first],
                points[first_end],
                points[second],
                points[second_end],
            ) {
                return false;
            }
        }
    }
    true
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

        let mut parts = BeginnerGenerationConstraintsV1 {
            target_category: Some(BeginnerTargetCategoryV1::Animal),
            target_parts: vec![
                BeginnerTargetPartRecordV1 {
                    kind: BeginnerTargetPartKindV1::Head,
                    count: 1,
                },
                BeginnerTargetPartRecordV1 {
                    kind: BeginnerTargetPartKindV1::Torso,
                    count: 1,
                },
            ],
            ..Default::default()
        };
        assert!(validate_beginner_generation_constraints_v1(&parts));
        parts.target_parts.push(BeginnerTargetPartRecordV1 {
            kind: BeginnerTargetPartKindV1::Head,
            count: 2,
        });
        assert!(validate_beginner_generation_constraints_v1(&parts));
        parts.target_parts[2].count = 8;
        assert!(validate_beginner_generation_constraints_v1(&parts));
        parts.target_parts[2].count = 9;
        assert!(!validate_beginner_generation_constraints_v1(&parts));

        let custom = BeginnerGenerationConstraintsV1 {
            target_category: Some(BeginnerTargetCategoryV1::CustomObject),
            ..Default::default()
        };
        assert!(validate_beginner_generation_constraints_v1(&custom));
        assert_eq!(
            custom_object_display_name_v1(&custom),
            Some("Custom object")
        );
        let json = serde_json::to_string(&custom).unwrap();
        assert!(json.contains("\"target_category\":\"custom_object\""));
        assert!(
            BEGINNER_CUSTOM_OBJECT_DISPLAY_NAME_V1.chars().count()
                <= MAX_BEGINNER_CUSTOM_OBJECT_DISPLAY_NAME_CHARS_V1
        );
        let mut unknown = serde_json::to_value(custom).unwrap();
        unknown["target_category"] = serde_json::json!("custom_object_v2");
        assert!(serde_json::from_value::<BeginnerGenerationConstraintsV1>(unknown).is_err());
        for invalid in [
            "",
            " padded",
            "path/name",
            "path\\name",
            "safe\u{202e}name",
            "e\u{301}",
        ] {
            assert!(!validate_custom_object_display_name_v1(
                Some(BeginnerTargetCategoryV1::CustomObject),
                Some(invalid),
            ));
        }
        assert!(validate_custom_object_display_name_v1(
            Some(BeginnerTargetCategoryV1::CustomObject),
            Some("折り紙 オブジェクト"),
        ));
    }

    #[test]
    fn protrusion_targets_are_versioned_bounded_and_unique() {
        let target = BeginnerProtrusionTargetV1 {
            id: 1,
            count: 2,
            length_tenths_mm: 100,
            thickness_tenths_mm: 20,
            root_width_tenths_mm: None,
            tip_width_tenths_mm: None,
            local_outline_tenths_mm: None,
            position_tenths_mm: [0, 0, 0],
            direction_milli: [1000, 0, 0],
            symmetry: BeginnerProtrusionSymmetryV1::Bilateral,
            curvature_degrees: 15,
            joint: BeginnerProtrusionJointV1::Hinge,
            motion_degrees: [-30, 45],
            side: BeginnerProtrusionSideV1::Either,
            priority: 80,
        };
        let mut constraints = BeginnerGenerationConstraintsV1::default();
        constraints.protrusions.push(target.clone());
        assert!(validate_beginner_generation_constraints_v1(&constraints));
        constraints.protrusions.push(target.clone());
        assert!(!validate_beginner_generation_constraints_v1(&constraints));
        constraints.protrusions.pop();
        constraints.protrusions[0].direction_milli = [0, 0, 0];
        assert!(!validate_beginner_generation_constraints_v1(&constraints));
    }

    #[test]
    fn optional_body_and_taper_geometry_are_bounded_and_old_json_remains_compatible() {
        let old = serde_json::json!({
            "schema_version": 1, "maximum_steps": 60, "detail_level": "standard",
            "target_category": null, "target_parts": [], "skeleton_segments": [],
            "protrusions": [], "bulge_targets": [], "target_asset": null,
            "allowed_techniques": ["valley_fold"]
        });
        let restored: BeginnerGenerationConstraintsV1 = serde_json::from_value(old).unwrap();
        assert_eq!(restored.generic_body_size_tenths_mm, None);
        assert_eq!(restored.generic_body_outline_tenths_mm, None);
        assert_eq!(
            restored.generic_body_outline_mode,
            BeginnerBodyOutlineModeV1::Symmetric
        );

        let mut constraints = BeginnerGenerationConstraintsV1 {
            generic_body_size_tenths_mm: Some([1_200, 800]),
            ..BeginnerGenerationConstraintsV1::default()
        };
        let mut target = BeginnerProtrusionTargetV1 {
            id: 1,
            count: 1,
            length_tenths_mm: 100,
            thickness_tenths_mm: 20,
            root_width_tenths_mm: Some(30),
            tip_width_tenths_mm: Some(10),
            local_outline_tenths_mm: None,
            position_tenths_mm: [0, 0, 0],
            direction_milli: [1_000, 0, 0],
            symmetry: BeginnerProtrusionSymmetryV1::None,
            curvature_degrees: 0,
            joint: BeginnerProtrusionJointV1::Fixed,
            motion_degrees: [0, 0],
            side: BeginnerProtrusionSideV1::Either,
            priority: 50,
        };
        constraints.protrusions.push(target.clone());
        assert!(validate_beginner_generation_constraints_v1(&constraints));
        target.tip_width_tenths_mm = Some(0);
        constraints.protrusions[0] = target;
        assert!(!validate_beginner_generation_constraints_v1(&constraints));
    }

    #[test]
    fn generic_body_outline_requires_canonical_symmetric_simple_polygon() {
        let mut constraints = BeginnerGenerationConstraintsV1 {
            generic_body_outline_tenths_mm: Some(vec![
                [-100, -50],
                [-100, 50],
                [100, 50],
                [100, -50],
            ]),
            ..BeginnerGenerationConstraintsV1::default()
        };
        assert!(validate_beginner_generation_constraints_v1(&constraints));
        constraints.generic_body_outline_tenths_mm =
            Some(vec![[100, -50], [-100, -50], [-100, 50], [100, 50]]);
        assert!(!validate_beginner_generation_constraints_v1(&constraints));
        constraints.generic_body_outline_tenths_mm =
            Some(vec![[-100, -50], [-100, 50], [90, 50], [100, -50]]);
        assert!(!validate_beginner_generation_constraints_v1(&constraints));
        constraints.generic_body_outline_tenths_mm =
            Some(vec![[-100, -50], [100, 50], [-100, 50], [100, -50]]);
        assert!(!validate_beginner_generation_constraints_v1(&constraints));
    }

    #[test]
    fn protrusion_local_outline_is_optional_canonical_and_bilateral_safe() {
        let target = BeginnerProtrusionTargetV1 {
            id: 1,
            count: 2,
            length_tenths_mm: 100,
            thickness_tenths_mm: 20,
            root_width_tenths_mm: None,
            tip_width_tenths_mm: None,
            local_outline_tenths_mm: Some(vec![[-50, -40], [50, -40], [50, 40], [-50, 40]]),
            position_tenths_mm: [0, 0, 0],
            direction_milli: [1_000, 0, 0],
            symmetry: BeginnerProtrusionSymmetryV1::Bilateral,
            curvature_degrees: 0,
            joint: BeginnerProtrusionJointV1::Fixed,
            motion_degrees: [0, 0],
            side: BeginnerProtrusionSideV1::Either,
            priority: 50,
        };
        let mut constraints = BeginnerGenerationConstraintsV1::default();
        constraints.protrusions.push(target.clone());
        assert!(validate_beginner_generation_constraints_v1(&constraints));
        constraints.protrusions[0].local_outline_tenths_mm =
            Some(vec![[-50, -40], [40, -40], [50, 40], [-50, 40]]);
        assert!(!validate_beginner_generation_constraints_v1(&constraints));
    }

    #[test]
    fn general_body_outline_is_explicit_asymmetric_and_counter_clockwise() {
        let mut constraints = BeginnerGenerationConstraintsV1 {
            generic_body_outline_mode: BeginnerBodyOutlineModeV1::General,
            generic_body_outline_tenths_mm: Some(vec![
                [-120, -40],
                [80, -60],
                [100, 50],
                [-70, 80],
            ]),
            ..BeginnerGenerationConstraintsV1::default()
        };
        assert!(validate_beginner_generation_constraints_v1(&constraints));
        constraints.generic_body_outline_tenths_mm =
            Some(vec![[-120, -40], [-70, 80], [100, 50], [80, -60]]);
        assert!(!validate_beginner_generation_constraints_v1(&constraints));
    }

    #[test]
    fn bulge_targets_require_faces_range_direction_amount_and_fingerprint() {
        let target = BeginnerBulgeTargetV1 {
            id: 1,
            face_ids: vec![FaceId::new()],
            range_min_tenths_mm: [-10, -10, -10],
            range_max_tenths_mm: [10, 10, 10],
            direction_milli: [0, 0, 1000],
            amount_tenths_mm: 50,
            source_fold_model_fingerprint: "a".repeat(64),
            reference_surface_binding: None,
        };
        let mut constraints = BeginnerGenerationConstraintsV1::default();
        constraints.bulge_targets.push(target);
        assert!(validate_beginner_generation_constraints_v1(&constraints));
        constraints.bulge_targets[0].direction_milli = [0, 0, 0];
        assert!(!validate_beginner_generation_constraints_v1(&constraints));
    }
}
