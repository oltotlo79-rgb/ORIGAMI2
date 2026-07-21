use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    BeginnerDetailLevelV1, BeginnerFoldTechniqueV1, BeginnerGenerationConstraintsV1,
    BeginnerProtrusionSymmetryV1, BeginnerSkeletonSegmentV1, BeginnerTargetAssetReferenceV1,
    BeginnerTargetCategoryV1, BeginnerTargetPartKindV1, BeginnerTargetPartRecordV1, CreasePattern,
    Edge, EdgeId, EdgeKind, Point2, ProjectId, Vertex, VertexId,
};

pub const BEGINNER_GENERATOR_SCHEMA_VERSION_V1: u32 = 1;
pub const MAX_BEGINNER_GENERATED_CANDIDATES_V1: usize = 3;
pub const MAX_BEGINNER_GENERATOR_INPUT_VERTICES_V1: usize = 10_000;
pub const BEGINNER_PARAMETER_GRID_SIZE_V1: usize = 27;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BeginnerParameterGridPointV1 {
    pub id: u8,
    pub scale_percent: u8,
    pub spacing_percent: u8,
    pub detail_level: BeginnerDetailLevelV1,
}

#[cfg(test)]
mod parameter_grid_tests {
    use super::*;

    #[test]
    fn grid_is_canonical_bounded_and_hash_sensitive() {
        let grid = beginner_parameter_grid_v1();
        assert_eq!(grid.len(), BEGINNER_PARAMETER_GRID_SIZE_V1);
        for (id, point) in grid.iter().enumerate() {
            assert_eq!(point.id, id as u8);
            assert!((10..=45).contains(&point.scale_percent));
            assert!((20..=80).contains(&point.spacing_percent));
        }
        let hash = beginner_parameter_grid_hash_v1(&grid);
        assert_eq!(
            hash,
            beginner_parameter_grid_hash_v1(&beginner_parameter_grid_v1())
        );
        let mut changed = grid;
        changed[0].scale_percent += 1;
        assert_ne!(hash, beginner_parameter_grid_hash_v1(&changed));
    }

    #[test]
    fn asymmetric_fish_semantic_provenance_is_ordered_hashed_and_serde_stable() {
        let semantic = asymmetric_insect_semantic_provenance(
            BeginnerGeneratedPlanKindV1::AsymmetricFishLandmarkBase,
        )
        .expect("fish semantic provenance");
        assert_eq!(
            semantic
                .ordered_bindings
                .iter()
                .map(|binding| binding.role.as_str())
                .collect::<Vec<_>>(),
            ["head", "tail", "fin_left", "fin_right"]
        );
        let bytes = serde_json::to_vec(&semantic).unwrap();
        assert_eq!(
            serde_json::from_slice::<BeginnerSemanticLandmarkProvenanceV1>(&bytes).unwrap(),
            semantic
        );
        let provenance = crate::BeginnerGenerationProvenanceV1 {
            schema_version: 1,
            topology_authority_sha256: [1; 32],
            fold_path_certificate_sha256: Some([2; 32]),
            confidence_score: 100,
            confidence_reasons: vec!["native_topology_witness".to_owned()],
            explicit_override: false,
            source_asset_fingerprint: "none".to_owned(),
            semantic_landmark_provenance: Some(semantic.clone()),
        };
        assert!(crate::validate_beginner_generation_provenance_v1(
            &provenance
        ));
        let mut tampered = provenance;
        tampered
            .semantic_landmark_provenance
            .as_mut()
            .unwrap()
            .physical_ray_group_sha256[0][0] ^= 1;
        assert!(!crate::validate_beginner_generation_provenance_v1(
            &tampered
        ));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BeginnerParameterGridHashV1(pub [u8; 32]);

#[must_use]
pub fn beginner_parameter_grid_v1()
-> [BeginnerParameterGridPointV1; BEGINNER_PARAMETER_GRID_SIZE_V1] {
    let scales = [10, 27, 45];
    let spacings = [20, 50, 80];
    let details = [
        BeginnerDetailLevelV1::Simple,
        BeginnerDetailLevelV1::Standard,
        BeginnerDetailLevelV1::Detailed,
    ];
    std::array::from_fn(|id| {
        let detail_index = id / 9;
        let scale_index = (id % 9) / 3;
        let spacing_index = id % 3;
        BeginnerParameterGridPointV1 {
            id: id as u8,
            scale_percent: scales[scale_index],
            spacing_percent: spacings[spacing_index],
            detail_level: details[detail_index],
        }
    })
}

#[must_use]
pub fn beginner_parameter_grid_hash_v1(
    grid: &[BeginnerParameterGridPointV1],
) -> BeginnerParameterGridHashV1 {
    let mut hash = Sha256::new();
    hash.update(b"ORIGAMI2_BEGINNER_PARAMETER_GRID_V1");
    hash.update((grid.len() as u64).to_be_bytes());
    for point in grid {
        hash.update([
            point.id,
            point.scale_percent,
            point.spacing_percent,
            match point.detail_level {
                BeginnerDetailLevelV1::Simple => 0,
                BeginnerDetailLevelV1::Standard => 1,
                BeginnerDetailLevelV1::Detailed => 2,
            },
        ]);
    }
    BeginnerParameterGridHashV1(hash.finalize().into())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BeginnerGeneratedPlanKindV1 {
    SymmetricFourLegBase,
    SymmetricWingBase,
    SymmetricBirdBase,
    AsymmetricBirdLandmarkBase,
    AsymmetricFourLegLandmarkBase,
    AsymmetricInsectLandmarkBase,
    AsymmetricFishLandmarkBase,
    SymmetricFishBase,
    SymmetricEarBase,
    SymmetricHornBase,
    SymmetricAntennaBase,
    SymmetricInsectLegPairBase,
    SymmetricSixLegBase,
    CenterAxisTailBase,
    CenterAxisHornBase,
    CenterAxisAntennaBase,
    CompositeTailEarBase,
    CompositeHornEarBase,
    CompositeHornTailBase,
    CompositeHornTailEarBase,
    CompositeCompleteAnimalBase,
    CompositeCompleteWingedAnimalBase,
    CompositeGenericTargetBase,
    CompositeWingAntennaBase,
    CompositeCompleteInsectBase,
    VerticalBookFold,
    HorizontalBookFold,
    DiagonalFold,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BeginnerGeneratedPlanV1 {
    pub schema_version: u32,
    pub kind: BeginnerGeneratedPlanKindV1,
    pub crease_pattern: CreasePattern,
    pub instruction_codes: Vec<String>,
    pub target_parts: Vec<BeginnerTargetPartRecordV1>,
    pub skeleton_segments: Vec<BeginnerSkeletonSegmentV1>,
    pub target_asset: Option<BeginnerTargetAssetReferenceV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_landmark_provenance: Option<BeginnerSemanticLandmarkProvenanceV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BeginnerSemanticLandmarkProvenanceV1 {
    pub schema_version: u32,
    pub ordered_bindings: Vec<BeginnerSemanticLandmarkBindingV1>,
    pub physical_ray_group_sha256: [[u8; 32]; 4],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BeginnerSemanticLandmarkBindingV1 {
    pub ordinal: u8,
    pub role: String,
    pub physical_ray: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BeginnerSymmetricParameterEstimateV1 {
    pub protrusion_count: u8,
    pub scale_percent: u8,
    pub spacing_percent: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BeginnerSymmetricParameterCandidateV1 {
    pub id: u8,
    pub scale_percent: u8,
    pub spacing_percent: u8,
    pub approximation_score: u8,
    pub complexity_score: u8,
    pub required_protrusion_count: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BeginnerBilateralPairBindingV1 {
    pub pair_index: u8,
    pub protrusion_id: u16,
    pub center_y_tenths_mm: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BeginnerTailEarBindingV1 {
    pub tail_protrusion_id: u16,
    pub ear_pair_protrusion_id: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BeginnerHornEarBindingV1 {
    pub horn_protrusion_id: u16,
    pub ear_pair_protrusion_id: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BeginnerHornTailBindingV1 {
    pub horn_protrusion_id: u16,
    pub tail_protrusion_id: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BeginnerHornTailEarBindingV1 {
    pub horn_protrusion_id: u16,
    pub tail_protrusion_id: u16,
    pub ear_pair_protrusion_id: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BeginnerCompleteAnimalBindingV1 {
    pub horn_protrusion_id: u16,
    pub tail_protrusion_id: u16,
    pub ear_pair_protrusion_id: u16,
    pub leg_protrusion_id: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BeginnerCompleteWingedAnimalBindingV1 {
    pub animal: BeginnerCompleteAnimalBindingV1,
    pub wing_pair_protrusion_id: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BeginnerWingAntennaBindingV1 {
    pub wing_pair_protrusion_id: u16,
    pub antenna_pair_protrusion_id: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BeginnerCompleteInsectBindingV1 {
    pub leg_pair_protrusion_ids: [u16; 3],
    pub wing_pair_protrusion_id: u16,
    pub antenna_pair_protrusion_id: u16,
}

#[must_use]
pub fn insect_complete_bindings_v1(
    constraints: &BeginnerGenerationConstraintsV1,
) -> Option<BeginnerCompleteInsectBindingV1> {
    let count = |kind| {
        constraints
            .target_parts
            .iter()
            .find(|part| part.kind == kind)
            .map_or(0, |part| part.count)
    };
    if constraints.target_category != Some(BeginnerTargetCategoryV1::Insect)
        || count(BeginnerTargetPartKindV1::Leg) != 6
        || count(BeginnerTargetPartKindV1::Wing) != 2
        || count(BeginnerTargetPartKindV1::Antenna) != 2
        || [
            BeginnerTargetPartKindV1::Leg,
            BeginnerTargetPartKindV1::Wing,
            BeginnerTargetPartKindV1::Antenna,
        ]
        .into_iter()
        .any(|kind| {
            constraints
                .target_parts
                .iter()
                .filter(|part| part.kind == kind)
                .count()
                != 1
        })
    {
        return None;
    }
    let wing = constraints.protrusions.iter().find(|target| {
        target.count == 2
            && target.symmetry == BeginnerProtrusionSymmetryV1::Bilateral
            && target.direction_milli[0] != 0
            && target.direction_milli[1] == 0
            && target.priority == 60
    })?;
    let antenna = constraints.protrusions.iter().find(|target| {
        target.count == 2
            && target.symmetry == BeginnerProtrusionSymmetryV1::Bilateral
            && target.direction_milli[1] != 0
            && target.direction_milli[0] == 0
            && target.priority == 60
    })?;
    let mut legs = constraints
        .protrusions
        .iter()
        .filter(|target| {
            target.count == 2
                && target.symmetry == BeginnerProtrusionSymmetryV1::Bilateral
                && target.direction_milli[0] != 0
                && target.direction_milli[1] == 0
                && target.priority == 50
        })
        .collect::<Vec<_>>();
    legs.sort_by_key(|target| (target.position_tenths_mm[1], target.id));
    if legs.len() != 3
        || legs
            .windows(2)
            .any(|pair| pair[0].position_tenths_mm[1] >= pair[1].position_tenths_mm[1])
    {
        return None;
    }
    let ids = [legs[0].id, legs[1].id, legs[2].id, wing.id, antenna.id];
    let mut unique = ids;
    unique.sort_unstable();
    if unique.windows(2).any(|pair| pair[0] == pair[1]) {
        return None;
    }
    Some(BeginnerCompleteInsectBindingV1 {
        leg_pair_protrusion_ids: [legs[0].id, legs[1].id, legs[2].id],
        wing_pair_protrusion_id: wing.id,
        antenna_pair_protrusion_id: antenna.id,
    })
}

#[must_use]
pub fn insect_wing_antenna_bindings_v1(
    constraints: &BeginnerGenerationConstraintsV1,
) -> Option<BeginnerWingAntennaBindingV1> {
    let count = |kind| {
        constraints
            .target_parts
            .iter()
            .find(|part| part.kind == kind)
            .map_or(0, |part| part.count)
    };
    if constraints.target_category != Some(BeginnerTargetCategoryV1::Insect)
        || count(BeginnerTargetPartKindV1::Wing) != 2
        || count(BeginnerTargetPartKindV1::Antenna) != 2
    {
        return None;
    }
    let pairs = constraints
        .protrusions
        .iter()
        .filter(|target| {
            target.count == 2 && target.symmetry == BeginnerProtrusionSymmetryV1::Bilateral
        })
        .collect::<Vec<_>>();
    if pairs.len() != 2 {
        return None;
    }
    let wing = pairs
        .iter()
        .find(|target| target.direction_milli[0] != 0 && target.direction_milli[1] == 0)?;
    let antenna = pairs
        .iter()
        .find(|target| target.direction_milli[1] != 0 && target.direction_milli[0] == 0)?;
    (wing.id != antenna.id).then_some(BeginnerWingAntennaBindingV1 {
        wing_pair_protrusion_id: wing.id,
        antenna_pair_protrusion_id: antenna.id,
    })
}

#[must_use]
pub fn animal_horn_tail_ear_bindings_v1(
    constraints: &BeginnerGenerationConstraintsV1,
) -> Option<BeginnerHornTailEarBindingV1> {
    let count = |kind| {
        constraints
            .target_parts
            .iter()
            .find(|part| part.kind == kind)
            .map_or(0, |part| part.count)
    };
    if constraints.target_category != Some(BeginnerTargetCategoryV1::Animal)
        || count(BeginnerTargetPartKindV1::Horn) != 1
        || count(BeginnerTargetPartKindV1::Tail) != 1
        || count(BeginnerTargetPartKindV1::Ear) != 2
    {
        return None;
    }
    let horn = constraints.protrusions.iter().find(|target| {
        target.count == 1
            && target.symmetry == BeginnerProtrusionSymmetryV1::None
            && target.direction_milli[1] != 0
            && target.direction_milli[0] == 0
    })?;
    let tail = constraints.protrusions.iter().find(|target| {
        target.count == 1
            && target.symmetry == BeginnerProtrusionSymmetryV1::None
            && target.direction_milli[0] != 0
            && target.direction_milli[1] == 0
    })?;
    let ears = constraints.protrusions.iter().find(|target| {
        target.count == 2 && target.symmetry == BeginnerProtrusionSymmetryV1::Bilateral
    })?;
    let mut ids = [horn.id, tail.id, ears.id];
    ids.sort_unstable();
    (ids[0] != ids[1] && ids[1] != ids[2]).then_some(BeginnerHornTailEarBindingV1 {
        horn_protrusion_id: horn.id,
        tail_protrusion_id: tail.id,
        ear_pair_protrusion_id: ears.id,
    })
}

#[must_use]
pub fn animal_complete_bindings_v1(
    constraints: &BeginnerGenerationConstraintsV1,
) -> Option<BeginnerCompleteAnimalBindingV1> {
    if constraints.protrusions.len() != 4 {
        return None;
    }
    let base = animal_horn_tail_ear_bindings_v1(constraints)?;
    let legs = constraints
        .target_parts
        .iter()
        .filter(|part| part.kind == BeginnerTargetPartKindV1::Leg)
        .collect::<Vec<_>>();
    if legs.len() != 1 || legs[0].count != 4 {
        return None;
    }
    let leg_targets = constraints
        .protrusions
        .iter()
        .filter(|target| {
            target.count == 4 && target.symmetry == BeginnerProtrusionSymmetryV1::Bilateral
        })
        .collect::<Vec<_>>();
    if leg_targets.len() != 1 {
        return None;
    }
    let leg_protrusion_id = leg_targets[0].id;
    let mut ids = [
        base.horn_protrusion_id,
        base.tail_protrusion_id,
        base.ear_pair_protrusion_id,
        leg_protrusion_id,
    ];
    ids.sort_unstable();
    ids.windows(2)
        .all(|pair| pair[0] != pair[1])
        .then_some(BeginnerCompleteAnimalBindingV1 {
            horn_protrusion_id: base.horn_protrusion_id,
            tail_protrusion_id: base.tail_protrusion_id,
            ear_pair_protrusion_id: base.ear_pair_protrusion_id,
            leg_protrusion_id,
        })
}

#[must_use]
pub fn animal_complete_winged_bindings_v1(
    constraints: &BeginnerGenerationConstraintsV1,
) -> Option<BeginnerCompleteWingedAnimalBindingV1> {
    if constraints.protrusions.len() != 5
        || constraints
            .target_parts
            .iter()
            .filter(|part| part.kind == BeginnerTargetPartKindV1::Wing)
            .map(|part| part.count)
            .sum::<u8>()
            != 2
    {
        return None;
    }
    let wing = constraints.protrusions.last()?;
    if wing.count != 2 || wing.symmetry != BeginnerProtrusionSymmetryV1::Bilateral {
        return None;
    }
    let mut animal_constraints = constraints.clone();
    animal_constraints.protrusions.pop();
    animal_constraints
        .target_parts
        .retain(|part| part.kind != BeginnerTargetPartKindV1::Wing);
    let animal = animal_complete_bindings_v1(&animal_constraints)?;
    let animal_ids = [
        animal.horn_protrusion_id,
        animal.tail_protrusion_id,
        animal.ear_pair_protrusion_id,
        animal.leg_protrusion_id,
    ];
    (!animal_ids.contains(&wing.id)).then_some(BeginnerCompleteWingedAnimalBindingV1 {
        animal,
        wing_pair_protrusion_id: wing.id,
    })
}

#[must_use]
pub fn animal_horn_tail_bindings_v1(
    constraints: &BeginnerGenerationConstraintsV1,
) -> Option<BeginnerHornTailBindingV1> {
    let count = |kind| {
        constraints
            .target_parts
            .iter()
            .find(|part| part.kind == kind)
            .map_or(0, |part| part.count)
    };
    if constraints.target_category != Some(BeginnerTargetCategoryV1::Animal)
        || count(BeginnerTargetPartKindV1::Horn) != 1
        || count(BeginnerTargetPartKindV1::Tail) != 1
    {
        return None;
    }
    let singles = constraints
        .protrusions
        .iter()
        .filter(|target| target.count == 1 && target.symmetry == BeginnerProtrusionSymmetryV1::None)
        .collect::<Vec<_>>();
    if singles.len() != 2 {
        return None;
    }
    let horn = singles
        .iter()
        .find(|target| target.direction_milli[1] != 0 && target.direction_milli[0] == 0)?;
    let tail = singles
        .iter()
        .find(|target| target.direction_milli[0] != 0 && target.direction_milli[1] == 0)?;
    (horn.id != tail.id).then_some(BeginnerHornTailBindingV1 {
        horn_protrusion_id: horn.id,
        tail_protrusion_id: tail.id,
    })
}

#[must_use]
pub fn animal_horn_ear_bindings_v1(
    constraints: &BeginnerGenerationConstraintsV1,
) -> Option<BeginnerHornEarBindingV1> {
    let count = |kind| {
        constraints
            .target_parts
            .iter()
            .find(|part| part.kind == kind)
            .map_or(0, |part| part.count)
    };
    if constraints.target_category != Some(BeginnerTargetCategoryV1::Animal)
        || count(BeginnerTargetPartKindV1::Horn) != 1
        || count(BeginnerTargetPartKindV1::Ear) != 2
    {
        return None;
    }
    let horn = constraints
        .protrusions
        .iter()
        .filter(|target| target.count == 1 && target.symmetry == BeginnerProtrusionSymmetryV1::None)
        .collect::<Vec<_>>();
    let ears = constraints
        .protrusions
        .iter()
        .filter(|target| {
            target.count == 2 && target.symmetry == BeginnerProtrusionSymmetryV1::Bilateral
        })
        .collect::<Vec<_>>();
    (horn.len() == 1 && ears.len() == 1 && horn[0].id != ears[0].id).then_some(
        BeginnerHornEarBindingV1 {
            horn_protrusion_id: horn[0].id,
            ear_pair_protrusion_id: ears[0].id,
        },
    )
}

#[must_use]
pub fn animal_tail_ear_bindings_v1(
    constraints: &BeginnerGenerationConstraintsV1,
) -> Option<BeginnerTailEarBindingV1> {
    let count = |kind| {
        constraints
            .target_parts
            .iter()
            .find(|part| part.kind == kind)
            .map_or(0, |part| part.count)
    };
    if constraints.target_category != Some(BeginnerTargetCategoryV1::Animal)
        || count(BeginnerTargetPartKindV1::Tail) != 1
        || count(BeginnerTargetPartKindV1::Ear) != 2
    {
        return None;
    }
    let tail = constraints
        .protrusions
        .iter()
        .filter(|target| target.count == 1 && target.symmetry == BeginnerProtrusionSymmetryV1::None)
        .collect::<Vec<_>>();
    let ears = constraints
        .protrusions
        .iter()
        .filter(|target| {
            target.count == 2 && target.symmetry == BeginnerProtrusionSymmetryV1::Bilateral
        })
        .collect::<Vec<_>>();
    (tail.len() == 1 && ears.len() == 1 && tail[0].id != ears[0].id).then_some(
        BeginnerTailEarBindingV1 {
            tail_protrusion_id: tail[0].id,
            ear_pair_protrusion_id: ears[0].id,
        },
    )
}

#[must_use]
pub fn insect_three_pair_bindings_v1(
    constraints: &BeginnerGenerationConstraintsV1,
) -> Option<[BeginnerBilateralPairBindingV1; 3]> {
    if constraints.target_category != Some(BeginnerTargetCategoryV1::Insect)
        || constraints
            .target_parts
            .iter()
            .find(|part| part.kind == BeginnerTargetPartKindV1::Leg)
            .map_or(0, |part| part.count)
            != 6
    {
        return None;
    }
    let (minimum_x, maximum_x, minimum_y, maximum_y) =
        skeleton_bounds(&constraints.skeleton_segments)?;
    let axis_twice = minimum_x.checked_add(maximum_x)?;
    let mut pairs = constraints
        .protrusions
        .iter()
        .filter(|target| {
            target.count == 2
                && target.symmetry == BeginnerProtrusionSymmetryV1::Bilateral
                && target.direction_milli[0] != 0
                && target.position_tenths_mm[0].checked_mul(2) == Some(axis_twice)
                && (minimum_y..=maximum_y).contains(&target.position_tenths_mm[1])
        })
        .collect::<Vec<_>>();
    if pairs.len() != 3 {
        return None;
    }
    pairs.sort_by_key(|target| (target.position_tenths_mm[1], target.id));
    if pairs
        .windows(2)
        .any(|pair| pair[0].position_tenths_mm[1] >= pair[1].position_tenths_mm[1])
    {
        return None;
    }
    Some(std::array::from_fn(|index| {
        BeginnerBilateralPairBindingV1 {
            pair_index: index as u8,
            protrusion_id: pairs[index].id,
            center_y_tenths_mm: pairs[index].position_tenths_mm[1],
        }
    }))
}

#[must_use]
pub fn symmetric_parameter_candidates_v1(
    estimate: BeginnerSymmetricParameterEstimateV1,
) -> [BeginnerSymmetricParameterCandidateV1; 3] {
    let variants = [
        (estimate.scale_percent, estimate.spacing_percent),
        (
            estimate.scale_percent.saturating_sub(5).max(10),
            estimate.spacing_percent.saturating_sub(10).max(20),
        ),
        (
            (estimate.scale_percent + 5).min(45),
            (estimate.spacing_percent + 10).min(80),
        ),
    ];
    variants.map(|(scale_percent, spacing_percent)| {
        let id = if scale_percent == estimate.scale_percent {
            0
        } else if scale_percent < estimate.scale_percent {
            1
        } else {
            2
        };
        let deviation = scale_percent
            .abs_diff(estimate.scale_percent)
            .saturating_add(spacing_percent.abs_diff(estimate.spacing_percent) / 2);
        BeginnerSymmetricParameterCandidateV1 {
            id,
            scale_percent,
            spacing_percent,
            approximation_score: 100_u8.saturating_sub(deviation.saturating_mul(3)),
            complexity_score: 20 + estimate.protrusion_count.saturating_mul(10) + scale_percent / 5,
            required_protrusion_count: estimate.protrusion_count,
        }
    })
}

#[must_use]
pub fn estimate_symmetric_parameters_v1(
    constraints: &BeginnerGenerationConstraintsV1,
) -> Option<BeginnerSymmetricParameterEstimateV1> {
    let count = |kind| {
        constraints
            .target_parts
            .iter()
            .find(|part| part.kind == kind)
            .map_or(0, |part| part.count)
    };
    if count(BeginnerTargetPartKindV1::Head) != 1 || count(BeginnerTargetPartKindV1::Torso) != 1 {
        return None;
    }
    let protrusion_count = match constraints.target_category? {
        BeginnerTargetCategoryV1::Animal
            if count(BeginnerTargetPartKindV1::Leg) == 4
                && count(BeginnerTargetPartKindV1::Horn) == 1
                && count(BeginnerTargetPartKindV1::Tail) == 1
                && count(BeginnerTargetPartKindV1::Ear) == 2 =>
        {
            8
        }
        BeginnerTargetCategoryV1::Animal if count(BeginnerTargetPartKindV1::Leg) == 4 => 4,
        BeginnerTargetCategoryV1::Animal if count(BeginnerTargetPartKindV1::Wing) == 2 => 2,
        BeginnerTargetCategoryV1::Animal if count(BeginnerTargetPartKindV1::Fin) == 2 => 2,
        BeginnerTargetCategoryV1::Animal if count(BeginnerTargetPartKindV1::Ear) == 2 => 2,
        BeginnerTargetCategoryV1::Animal if count(BeginnerTargetPartKindV1::Horn) == 2 => 2,
        BeginnerTargetCategoryV1::Animal
            if count(BeginnerTargetPartKindV1::Horn) == 1
                && count(BeginnerTargetPartKindV1::Tail) == 1
                && count(BeginnerTargetPartKindV1::Ear) == 2 =>
        {
            4
        }
        BeginnerTargetCategoryV1::Animal
            if count(BeginnerTargetPartKindV1::Tail) == 1
                && count(BeginnerTargetPartKindV1::Ear) == 2 =>
        {
            3
        }
        BeginnerTargetCategoryV1::Animal
            if count(BeginnerTargetPartKindV1::Horn) == 1
                && count(BeginnerTargetPartKindV1::Ear) == 2 =>
        {
            3
        }
        BeginnerTargetCategoryV1::Animal
            if count(BeginnerTargetPartKindV1::Horn) == 1
                && count(BeginnerTargetPartKindV1::Tail) == 1 =>
        {
            2
        }
        BeginnerTargetCategoryV1::Animal if count(BeginnerTargetPartKindV1::Tail) == 1 => 1,
        BeginnerTargetCategoryV1::Animal if count(BeginnerTargetPartKindV1::Horn) == 1 => 1,
        BeginnerTargetCategoryV1::Insect
            if count(BeginnerTargetPartKindV1::Wing) == 2
                && count(BeginnerTargetPartKindV1::Antenna) == 2
                && count(BeginnerTargetPartKindV1::Leg) == 6 =>
        {
            10
        }
        BeginnerTargetCategoryV1::Insect
            if count(BeginnerTargetPartKindV1::Wing) == 2
                && count(BeginnerTargetPartKindV1::Antenna) == 2 =>
        {
            4
        }
        BeginnerTargetCategoryV1::Insect if count(BeginnerTargetPartKindV1::Wing) == 2 => 2,
        BeginnerTargetCategoryV1::Insect if count(BeginnerTargetPartKindV1::Antenna) == 2 => 2,
        BeginnerTargetCategoryV1::Insect if count(BeginnerTargetPartKindV1::Antenna) == 1 => 1,
        BeginnerTargetCategoryV1::Insect if count(BeginnerTargetPartKindV1::Leg) == 2 => 2,
        BeginnerTargetCategoryV1::Insect if count(BeginnerTargetPartKindV1::Leg) == 6 => 6,
        _ => return None,
    };
    let scale_percent = match constraints.detail_level {
        crate::BeginnerDetailLevelV1::Simple => 20,
        crate::BeginnerDetailLevelV1::Standard => 25,
        crate::BeginnerDetailLevelV1::Detailed => 30,
    };
    Some(BeginnerSymmetricParameterEstimateV1 {
        protrusion_count,
        scale_percent,
        spacing_percent: if protrusion_count == 4 { 35 } else { 50 },
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BeginnerGeneratorErrorV1 {
    ResourceLimit,
    UnsupportedPaper,
    UnsupportedTechniques,
    MissingTargetCategory,
    MissingRequiredParts,
    UnsupportedAnimalTemplate,
    UnsupportedInsectTemplate,
}

fn canonical_asymmetric_quad(points: &[Point2]) -> Option<(Point2, Vec<Point2>)> {
    if points.len() != 4 {
        return None;
    }
    let center = Point2::new(
        points[0].x + points[2].x - points[3].x,
        points[0].y + points[2].y - points[3].y,
    );
    let scale = points[0].x - center.x;
    let height = 3.0_f64.sqrt() * scale / 2.0;
    let expected = [
        (scale, 0.0),
        (-scale / 2.0, height),
        (-scale / 2.0, -height),
        (scale / 2.0, -height),
    ];
    (scale > 0.0
        && points.iter().zip(expected).all(|(point, (x, y))| {
            (point.x - center.x - x).abs() <= 1.0e-12 && (point.y - center.y - y).abs() <= 1.0e-12
        }))
    .then(|| (center, points.to_vec()))
}

pub fn generate_beginner_plans_v1(
    namespace: ProjectId,
    source: &CreasePattern,
    boundary_vertices: &[VertexId],
    constraints: &BeginnerGenerationConstraintsV1,
) -> Result<Vec<BeginnerGeneratedPlanV1>, BeginnerGeneratorErrorV1> {
    if source.vertices.len() > MAX_BEGINNER_GENERATOR_INPUT_VERTICES_V1 {
        return Err(BeginnerGeneratorErrorV1::ResourceLimit);
    }
    if !(4..=MAX_BEGINNER_GENERATOR_INPUT_VERTICES_V1).contains(&boundary_vertices.len()) {
        return Err(BeginnerGeneratorErrorV1::UnsupportedPaper);
    }
    let mut boundary_ids = std::collections::HashSet::with_capacity(boundary_vertices.len());
    if boundary_vertices.iter().any(|id| !boundary_ids.insert(*id)) {
        return Err(BeginnerGeneratorErrorV1::UnsupportedPaper);
    }
    let points = boundary_vertices
        .iter()
        .map(|id| {
            source
                .vertices
                .iter()
                .find(|vertex| vertex.id == *id)
                .map(|vertex| vertex.position)
        })
        .collect::<Option<Vec<_>>>()
        .ok_or(BeginnerGeneratorErrorV1::UnsupportedPaper)?;
    let min_x = points
        .iter()
        .map(|point| point.x)
        .fold(f64::INFINITY, f64::min);
    let max_x = points
        .iter()
        .map(|point| point.x)
        .fold(f64::NEG_INFINITY, f64::max);
    let min_y = points
        .iter()
        .map(|point| point.y)
        .fold(f64::INFINITY, f64::min);
    let max_y = points
        .iter()
        .map(|point| point.y)
        .fold(f64::NEG_INFINITY, f64::max);
    let canonical_asymmetric = canonical_asymmetric_quad(&points).is_some();
    if ![min_x, max_x, min_y, max_y].into_iter().all(f64::is_finite)
        || min_x >= max_x
        || min_y >= max_y
        || !canonical_asymmetric
            && !points.iter().all(|point| {
                (point.x == min_x || point.x == max_x) && (min_y..=max_y).contains(&point.y)
                    || (point.y == min_y || point.y == max_y) && (min_x..=max_x).contains(&point.x)
            })
        || !canonical_asymmetric
            && ![
                (min_x, min_y),
                (max_x, min_y),
                (max_x, max_y),
                (min_x, max_y),
            ]
            .into_iter()
            .all(|corner| points.iter().any(|point| (point.x, point.y) == corner))
        || boundary_vertices.len() > 4
            && points
                .iter()
                .zip(points.iter().cycle().skip(1))
                .take(points.len())
                .any(|(first, second)| {
                    first == second
                        || !((first.x == second.x && (first.x == min_x || first.x == max_x))
                            || (first.y == second.y && (first.y == min_y || first.y == max_y)))
                })
    {
        return Err(BeginnerGeneratorErrorV1::UnsupportedPaper);
    }
    let allows_valley = constraints
        .allowed_techniques
        .contains(&BeginnerFoldTechniqueV1::ValleyFold);
    let allows_mountain = constraints
        .allowed_techniques
        .contains(&BeginnerFoldTechniqueV1::MountainFold);
    if !allows_valley && !allows_mountain {
        return Err(BeginnerGeneratorErrorV1::UnsupportedTechniques);
    }
    let target_category = constraints
        .target_category
        .ok_or(BeginnerGeneratorErrorV1::MissingTargetCategory)?;
    let part_count = |kind| {
        constraints
            .target_parts
            .iter()
            .find(|part| part.kind == kind)
            .map_or(0, |part| part.count)
    };
    if part_count(BeginnerTargetPartKindV1::Head) != 1
        || part_count(BeginnerTargetPartKindV1::Torso) != 1
    {
        return Err(BeginnerGeneratorErrorV1::MissingRequiredParts);
    }
    let kind = if allows_valley {
        EdgeKind::Valley
    } else {
        EdgeKind::Mountain
    };
    let template = match target_category {
        BeginnerTargetCategoryV1::Animal => {
            let feature_records = constraints
                .target_parts
                .iter()
                .filter(|part| {
                    !matches!(
                        part.kind,
                        BeginnerTargetPartKindV1::Head | BeginnerTargetPartKindV1::Torso
                    )
                })
                .count();
            let horn = part_count(BeginnerTargetPartKindV1::Horn) == 1;
            let tail = part_count(BeginnerTargetPartKindV1::Tail) == 1;
            let ears = part_count(BeginnerTargetPartKindV1::Ear) == 2;
            let legs = part_count(BeginnerTargetPartKindV1::Leg) == 4;
            let wings = part_count(BeginnerTargetPartKindV1::Wing) == 2;
            let asymmetric_landmark_fish = tail
                && part_count(BeginnerTargetPartKindV1::Fin) == 2
                && constraints
                    .protrusions
                    .iter()
                    .filter(|target| {
                        target.count == 1 && target.symmetry == BeginnerProtrusionSymmetryV1::None
                    })
                    .count()
                    >= 3;
            let known_composite = feature_records == 2 && (horn && (tail || ears) || tail && ears)
                || feature_records == 3 && horn && tail && ears
                || feature_records == 4 && horn && tail && ears && legs
                || feature_records == 5 && horn && tail && ears && legs && wings;
            if asymmetric_landmark_fish {
                symmetric_template(
                    namespace,
                    source,
                    BeginnerGeneratedPlanKindV1::AsymmetricFishLandmarkBase,
                    kind,
                    min_x,
                    max_x,
                    min_y,
                    max_y,
                    &[(1.0, 0.5), (0.25, 1.0), (0.25, 0.0), (0.75, 0.0)],
                    "asymmetric_fish_landmark_base",
                    constraints,
                )
            } else if feature_records >= 2 && !known_composite {
                let endpoints = bounded_generic_composite_endpoints(constraints)
                    .ok_or(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate)?;
                symmetric_template(
                    namespace,
                    source,
                    BeginnerGeneratedPlanKindV1::CompositeGenericTargetBase,
                    kind,
                    min_x,
                    max_x,
                    min_y,
                    max_y,
                    &endpoints,
                    "composite_generic_target_base",
                    constraints,
                )
            } else if part_count(BeginnerTargetPartKindV1::Horn) == 1
                && part_count(BeginnerTargetPartKindV1::Tail) == 1
                && part_count(BeginnerTargetPartKindV1::Ear) == 2
            {
                let complete = animal_complete_bindings_v1(constraints);
                let winged_complete = animal_complete_winged_bindings_v1(constraints);
                let bindings = animal_horn_tail_ear_bindings_v1(constraints)
                    .ok_or(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate)?;
                let mut horn_only = constraints.clone();
                horn_only
                    .protrusions
                    .retain(|target| target.id == bindings.horn_protrusion_id);
                let horn = parameterized_center_axis_endpoint(&horn_only, true)
                    .ok_or(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate)?;
                let mut tail_only = constraints.clone();
                tail_only
                    .protrusions
                    .retain(|target| target.id == bindings.tail_protrusion_id);
                let tail = parameterized_center_axis_endpoint(&tail_only, false)
                    .ok_or(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate)?;
                let mut ear_only = constraints.clone();
                ear_only
                    .protrusions
                    .retain(|target| target.id == bindings.ear_pair_protrusion_id);
                let ears = parameterized_symmetric_endpoints(&ear_only, 2, false)
                    .ok_or(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate)?;
                let mut endpoints = vec![horn, tail];
                endpoints.extend(ears);
                if let Some(complete) = complete.or(winged_complete.map(|binding| binding.animal)) {
                    let mut leg_only = constraints.clone();
                    leg_only
                        .protrusions
                        .retain(|target| target.id == complete.leg_protrusion_id);
                    endpoints.extend(
                        parameterized_symmetric_endpoints(&leg_only, 4, true)
                            .ok_or(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate)?,
                    );
                    if let Some(winged) = winged_complete {
                        let mut wing_only = constraints.clone();
                        wing_only
                            .protrusions
                            .retain(|target| target.id == winged.wing_pair_protrusion_id);
                        endpoints.extend(
                            parameterized_symmetric_endpoints(&wing_only, 2, false)
                                .ok_or(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate)?,
                        );
                    }
                } else if part_count(BeginnerTargetPartKindV1::Leg) != 0 {
                    return Err(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate);
                }
                symmetric_template(
                    namespace,
                    source,
                    if winged_complete.is_some() {
                        BeginnerGeneratedPlanKindV1::CompositeCompleteWingedAnimalBase
                    } else if complete.is_some() {
                        BeginnerGeneratedPlanKindV1::CompositeCompleteAnimalBase
                    } else {
                        BeginnerGeneratedPlanKindV1::CompositeHornTailEarBase
                    },
                    kind,
                    min_x,
                    max_x,
                    min_y,
                    max_y,
                    &endpoints,
                    if winged_complete.is_some() {
                        "composite_complete_winged_animal_base"
                    } else if complete.is_some() {
                        "composite_complete_animal_base"
                    } else {
                        "composite_horn_tail_ear_base"
                    },
                    constraints,
                )
            } else if part_count(BeginnerTargetPartKindV1::Horn) == 1
                && part_count(BeginnerTargetPartKindV1::Tail) == 1
            {
                let bindings = animal_horn_tail_bindings_v1(constraints)
                    .ok_or(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate)?;
                let mut horn_only = constraints.clone();
                horn_only
                    .protrusions
                    .retain(|target| target.id == bindings.horn_protrusion_id);
                let horn = parameterized_center_axis_endpoint(&horn_only, true)
                    .ok_or(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate)?;
                let mut tail_only = constraints.clone();
                tail_only
                    .protrusions
                    .retain(|target| target.id == bindings.tail_protrusion_id);
                let tail = parameterized_center_axis_endpoint(&tail_only, false)
                    .ok_or(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate)?;
                symmetric_template(
                    namespace,
                    source,
                    BeginnerGeneratedPlanKindV1::CompositeHornTailBase,
                    kind,
                    min_x,
                    max_x,
                    min_y,
                    max_y,
                    &[horn, tail],
                    "composite_horn_tail_base",
                    constraints,
                )
            } else if part_count(BeginnerTargetPartKindV1::Horn) == 1
                && part_count(BeginnerTargetPartKindV1::Ear) == 2
            {
                let bindings = animal_horn_ear_bindings_v1(constraints)
                    .ok_or(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate)?;
                let horn = parameterized_center_axis_endpoint(constraints, true)
                    .ok_or(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate)?;
                let mut isolated = constraints.clone();
                isolated
                    .protrusions
                    .retain(|target| target.id == bindings.ear_pair_protrusion_id);
                let ears = parameterized_symmetric_endpoints(&isolated, 2, false)
                    .ok_or(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate)?;
                let mut endpoints = vec![horn];
                endpoints.extend(ears);
                symmetric_template(
                    namespace,
                    source,
                    BeginnerGeneratedPlanKindV1::CompositeHornEarBase,
                    kind,
                    min_x,
                    max_x,
                    min_y,
                    max_y,
                    &endpoints,
                    "composite_horn_ear_base",
                    constraints,
                )
            } else if part_count(BeginnerTargetPartKindV1::Tail) == 1
                && part_count(BeginnerTargetPartKindV1::Ear) == 2
            {
                let bindings = animal_tail_ear_bindings_v1(constraints)
                    .ok_or(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate)?;
                let tail = parameterized_center_axis_endpoint(constraints, false)
                    .ok_or(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate)?;
                let mut isolated = constraints.clone();
                isolated
                    .protrusions
                    .retain(|target| target.id == bindings.ear_pair_protrusion_id);
                let ears = parameterized_symmetric_endpoints(&isolated, 2, false)
                    .ok_or(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate)?;
                let mut endpoints = vec![tail];
                endpoints.extend(ears);
                symmetric_template(
                    namespace,
                    source,
                    BeginnerGeneratedPlanKindV1::CompositeTailEarBase,
                    kind,
                    min_x,
                    max_x,
                    min_y,
                    max_y,
                    &endpoints,
                    "composite_tail_ear_base",
                    constraints,
                )
            } else if part_count(BeginnerTargetPartKindV1::Horn) == 1 {
                let endpoint = parameterized_center_axis_endpoint(constraints, true)
                    .ok_or(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate)?;
                symmetric_template(
                    namespace,
                    source,
                    BeginnerGeneratedPlanKindV1::CenterAxisHornBase,
                    kind,
                    min_x,
                    max_x,
                    min_y,
                    max_y,
                    &[endpoint],
                    "center_axis_horn_base",
                    constraints,
                )
            } else if part_count(BeginnerTargetPartKindV1::Tail) == 1 {
                let endpoint = parameterized_center_axis_endpoint(constraints, false)
                    .ok_or(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate)?;
                symmetric_template(
                    namespace,
                    source,
                    BeginnerGeneratedPlanKindV1::CenterAxisTailBase,
                    kind,
                    min_x,
                    max_x,
                    min_y,
                    max_y,
                    &[endpoint],
                    "center_axis_tail_base",
                    constraints,
                )
            } else {
                let (required_count, vertical, plan_kind, instruction) =
                    if part_count(BeginnerTargetPartKindV1::Leg) == 4
                        && constraints
                            .protrusions
                            .iter()
                            .filter(|target| {
                                target.count == 1
                                    && target.symmetry == BeginnerProtrusionSymmetryV1::None
                            })
                            .count()
                            == 4
                    {
                        (
                            4,
                            true,
                            BeginnerGeneratedPlanKindV1::AsymmetricFourLegLandmarkBase,
                            "asymmetric_four_leg_landmark_base",
                        )
                    } else if part_count(BeginnerTargetPartKindV1::Leg) == 4 {
                        (
                            4,
                            true,
                            BeginnerGeneratedPlanKindV1::SymmetricFourLegBase,
                            "symmetric_four_leg_base",
                        )
                    } else if part_count(BeginnerTargetPartKindV1::Wing) == 2
                        && constraints
                            .protrusions
                            .iter()
                            .filter(|target| {
                                target.count == 1
                                    && target.symmetry == BeginnerProtrusionSymmetryV1::None
                            })
                            .count()
                            == 2
                    {
                        (
                            2,
                            false,
                            BeginnerGeneratedPlanKindV1::AsymmetricBirdLandmarkBase,
                            "asymmetric_bird_landmark_base",
                        )
                    } else if part_count(BeginnerTargetPartKindV1::Wing) == 2 {
                        (
                            2,
                            false,
                            BeginnerGeneratedPlanKindV1::SymmetricBirdBase,
                            "symmetric_bird_base",
                        )
                    } else if part_count(BeginnerTargetPartKindV1::Fin) == 2 {
                        (
                            2,
                            false,
                            BeginnerGeneratedPlanKindV1::SymmetricFishBase,
                            "symmetric_fish_base",
                        )
                    } else if part_count(BeginnerTargetPartKindV1::Ear) == 2 {
                        (
                            2,
                            false,
                            BeginnerGeneratedPlanKindV1::SymmetricEarBase,
                            "symmetric_ear_base",
                        )
                    } else if part_count(BeginnerTargetPartKindV1::Horn) == 2 {
                        (
                            2,
                            false,
                            BeginnerGeneratedPlanKindV1::SymmetricHornBase,
                            "symmetric_horn_base",
                        )
                    } else {
                        return Err(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate);
                    };
                let asymmetric = matches!(
                    plan_kind,
                    BeginnerGeneratedPlanKindV1::AsymmetricBirdLandmarkBase
                        | BeginnerGeneratedPlanKindV1::AsymmetricFourLegLandmarkBase
                );
                if constraints.skeleton_segments.len() < if vertical { 3 } else { 2 }
                    || (!asymmetric && !has_bilateral_skeleton(constraints))
                    || (!asymmetric && !has_bilateral_protrusion_count(constraints, required_count))
                {
                    return Err(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate);
                }
                let endpoints = if asymmetric {
                    let _landmarks = bounded_generic_composite_endpoints(constraints)
                        .ok_or(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate)?;
                    vec![(1.0, 0.5), (0.25, 1.0), (0.25, 0.0), (0.75, 0.0)]
                } else {
                    parameterized_symmetric_endpoints(constraints, required_count, vertical)
                        .ok_or(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate)?
                        .to_vec()
                };
                symmetric_template(
                    namespace,
                    source,
                    plan_kind,
                    kind,
                    min_x,
                    max_x,
                    min_y,
                    max_y,
                    &endpoints,
                    instruction,
                    constraints,
                )
            }
        }
        BeginnerTargetCategoryV1::Insect => {
            let feature_records = constraints
                .target_parts
                .iter()
                .filter(|part| {
                    !matches!(
                        part.kind,
                        BeginnerTargetPartKindV1::Head | BeginnerTargetPartKindV1::Torso
                    )
                })
                .count();
            let wing_antenna = part_count(BeginnerTargetPartKindV1::Wing) == 2
                && part_count(BeginnerTargetPartKindV1::Antenna) == 2;
            let asymmetric_landmark_insect = part_count(BeginnerTargetPartKindV1::Tail) == 1
                && part_count(BeginnerTargetPartKindV1::Wing) == 2
                && part_count(BeginnerTargetPartKindV1::Leg) == 6
                && constraints
                    .protrusions
                    .iter()
                    .filter(|target| {
                        target.count == 1 && target.symmetry == BeginnerProtrusionSymmetryV1::None
                    })
                    .count()
                    >= 7;
            let known_composite = feature_records == 2 && wing_antenna
                || feature_records == 3
                    && wing_antenna
                    && part_count(BeginnerTargetPartKindV1::Leg) == 6;
            if asymmetric_landmark_insect {
                let endpoints = [(1.0, 0.5), (0.25, 1.0), (0.25, 0.0), (0.75, 0.0)];
                symmetric_template(
                    namespace,
                    source,
                    BeginnerGeneratedPlanKindV1::AsymmetricInsectLandmarkBase,
                    kind,
                    min_x,
                    max_x,
                    min_y,
                    max_y,
                    &endpoints,
                    "asymmetric_insect_landmark_base",
                    constraints,
                )
            } else if feature_records >= 2 && !known_composite {
                let endpoints = bounded_generic_composite_endpoints(constraints)
                    .ok_or(BeginnerGeneratorErrorV1::UnsupportedInsectTemplate)?;
                symmetric_template(
                    namespace,
                    source,
                    BeginnerGeneratedPlanKindV1::CompositeGenericTargetBase,
                    kind,
                    min_x,
                    max_x,
                    min_y,
                    max_y,
                    &endpoints,
                    "composite_generic_target_base",
                    constraints,
                )
            } else if part_count(BeginnerTargetPartKindV1::Wing) == 2
                && part_count(BeginnerTargetPartKindV1::Antenna) == 2
                && part_count(BeginnerTargetPartKindV1::Leg) == 6
            {
                let bindings = insect_complete_bindings_v1(constraints)
                    .ok_or(BeginnerGeneratorErrorV1::UnsupportedInsectTemplate)?;
                let mut endpoints = Vec::with_capacity(20);
                for (id, vertical) in [
                    (bindings.wing_pair_protrusion_id, false),
                    (bindings.antenna_pair_protrusion_id, true),
                ] {
                    let mut isolated = constraints.clone();
                    isolated.protrusions.retain(|target| target.id == id);
                    endpoints.extend(
                        parameterized_symmetric_endpoints(&isolated, 2, vertical)
                            .ok_or(BeginnerGeneratorErrorV1::UnsupportedInsectTemplate)?,
                    );
                }
                for id in bindings.leg_pair_protrusion_ids {
                    let mut isolated = constraints.clone();
                    isolated.protrusions.retain(|target| target.id == id);
                    endpoints.extend(
                        parameterized_symmetric_endpoints(&isolated, 2, false)
                            .ok_or(BeginnerGeneratorErrorV1::UnsupportedInsectTemplate)?,
                    );
                }
                symmetric_template(
                    namespace,
                    source,
                    BeginnerGeneratedPlanKindV1::CompositeCompleteInsectBase,
                    kind,
                    min_x,
                    max_x,
                    min_y,
                    max_y,
                    &endpoints,
                    "composite_complete_insect_base",
                    constraints,
                )
            } else if part_count(BeginnerTargetPartKindV1::Wing) == 2
                && part_count(BeginnerTargetPartKindV1::Antenna) == 2
            {
                let bindings = insect_wing_antenna_bindings_v1(constraints)
                    .ok_or(BeginnerGeneratorErrorV1::UnsupportedInsectTemplate)?;
                let mut wing_only = constraints.clone();
                wing_only
                    .protrusions
                    .retain(|target| target.id == bindings.wing_pair_protrusion_id);
                let wings = parameterized_symmetric_endpoints(&wing_only, 2, false)
                    .ok_or(BeginnerGeneratorErrorV1::UnsupportedInsectTemplate)?;
                let mut antenna_only = constraints.clone();
                antenna_only
                    .protrusions
                    .retain(|target| target.id == bindings.antenna_pair_protrusion_id);
                let antennae = parameterized_symmetric_endpoints(&antenna_only, 2, true)
                    .ok_or(BeginnerGeneratorErrorV1::UnsupportedInsectTemplate)?;
                let mut endpoints = wings.to_vec();
                endpoints.extend(antennae);
                symmetric_template(
                    namespace,
                    source,
                    BeginnerGeneratedPlanKindV1::CompositeWingAntennaBase,
                    kind,
                    min_x,
                    max_x,
                    min_y,
                    max_y,
                    &endpoints,
                    "composite_wing_antenna_base",
                    constraints,
                )
            } else if part_count(BeginnerTargetPartKindV1::Antenna) == 1 {
                let endpoint = parameterized_center_axis_endpoint(constraints, true)
                    .ok_or(BeginnerGeneratorErrorV1::UnsupportedInsectTemplate)?;
                symmetric_template(
                    namespace,
                    source,
                    BeginnerGeneratedPlanKindV1::CenterAxisAntennaBase,
                    kind,
                    min_x,
                    max_x,
                    min_y,
                    max_y,
                    &[endpoint],
                    "center_axis_antenna_base",
                    constraints,
                )
            } else if part_count(BeginnerTargetPartKindV1::Leg) == 6 {
                let bindings = insect_three_pair_bindings_v1(constraints)
                    .ok_or(BeginnerGeneratorErrorV1::UnsupportedInsectTemplate)?;
                let mut endpoints = Vec::with_capacity(12);
                for binding in bindings {
                    let mut isolated = constraints.clone();
                    isolated
                        .protrusions
                        .retain(|target| target.id == binding.protrusion_id);
                    endpoints.extend(
                        parameterized_symmetric_endpoints(&isolated, 2, false)
                            .ok_or(BeginnerGeneratorErrorV1::UnsupportedInsectTemplate)?,
                    );
                }
                symmetric_template(
                    namespace,
                    source,
                    BeginnerGeneratedPlanKindV1::SymmetricSixLegBase,
                    kind,
                    min_x,
                    max_x,
                    min_y,
                    max_y,
                    &endpoints,
                    "symmetric_six_leg_base",
                    constraints,
                )
            } else {
                let (plan_kind, instruction) = if part_count(BeginnerTargetPartKindV1::Wing) == 2 {
                    (
                        BeginnerGeneratedPlanKindV1::SymmetricWingBase,
                        "symmetric_wing_base",
                    )
                } else if part_count(BeginnerTargetPartKindV1::Antenna) == 2 {
                    (
                        BeginnerGeneratedPlanKindV1::SymmetricAntennaBase,
                        "symmetric_antenna_base",
                    )
                } else if part_count(BeginnerTargetPartKindV1::Leg) == 2 {
                    (
                        BeginnerGeneratedPlanKindV1::SymmetricInsectLegPairBase,
                        "symmetric_insect_leg_pair_base",
                    )
                } else {
                    return Err(BeginnerGeneratorErrorV1::UnsupportedInsectTemplate);
                };
                if constraints.skeleton_segments.len() < 2
                    || !has_bilateral_skeleton(constraints)
                    || !has_bilateral_protrusion_count(constraints, 2)
                {
                    return Err(BeginnerGeneratorErrorV1::UnsupportedInsectTemplate);
                }
                let endpoints = parameterized_symmetric_endpoints(constraints, 2, false)
                    .ok_or(BeginnerGeneratorErrorV1::UnsupportedInsectTemplate)?;
                symmetric_template(
                    namespace,
                    source,
                    plan_kind,
                    kind,
                    min_x,
                    max_x,
                    min_y,
                    max_y,
                    &endpoints,
                    instruction,
                    constraints,
                )
            }
        }
    };
    let animal_variants = [
        (
            BeginnerGeneratedPlanKindV1::VerticalBookFold,
            Point2::new((min_x + max_x) / 2.0, min_y),
            Point2::new((min_x + max_x) / 2.0, max_y),
            "book_fold_vertical",
        ),
        (
            BeginnerGeneratedPlanKindV1::HorizontalBookFold,
            Point2::new(min_x, (min_y + max_y) / 2.0),
            Point2::new(max_x, (min_y + max_y) / 2.0),
            "book_fold_horizontal",
        ),
        (
            BeginnerGeneratedPlanKindV1::DiagonalFold,
            Point2::new(min_x, min_y),
            Point2::new(max_x, max_y),
            "diagonal_fold",
        ),
    ];
    let variants = match target_category {
        BeginnerTargetCategoryV1::Animal => animal_variants,
        BeginnerTargetCategoryV1::Insect => {
            [animal_variants[2], animal_variants[0], animal_variants[1]]
        }
    };
    let mut plans = vec![template];
    plans.extend(
        variants
            .into_iter()
            .take(MAX_BEGINNER_GENERATED_CANDIDATES_V1 - 1)
            .map(|(plan_kind, start, end, instruction)| {
                let prefix = format!("beginner-plan-{plan_kind:?}");
                let start_id = source
                    .vertices
                    .iter()
                    .find(|vertex| vertex.position == start)
                    .map_or_else(
                        || VertexId::derive_v5(namespace, format!("{prefix}-start").as_bytes()),
                        |vertex| vertex.id,
                    );
                let end_id = source
                    .vertices
                    .iter()
                    .find(|vertex| vertex.position == end)
                    .map_or_else(
                        || VertexId::derive_v5(namespace, format!("{prefix}-end").as_bytes()),
                        |vertex| vertex.id,
                    );
                BeginnerGeneratedPlanV1 {
                    schema_version: BEGINNER_GENERATOR_SCHEMA_VERSION_V1,
                    kind: plan_kind,
                    crease_pattern: CreasePattern {
                        vertices: vec![
                            Vertex {
                                id: start_id,
                                position: start,
                            },
                            Vertex {
                                id: end_id,
                                position: end,
                            },
                        ],
                        edges: vec![Edge {
                            id: EdgeId::derive_v5(namespace, format!("{prefix}-edge").as_bytes()),
                            start: start_id,
                            end: end_id,
                            kind,
                        }],
                    },
                    instruction_codes: vec![instruction.to_owned()],
                    target_parts: constraints.target_parts.clone(),
                    skeleton_segments: constraints.skeleton_segments.clone(),
                    target_asset: constraints.target_asset,
                    semantic_landmark_provenance: None,
                }
            }),
    );
    Ok(plans)
}

fn parameterized_center_axis_endpoint(
    constraints: &BeginnerGenerationConstraintsV1,
    vertical: bool,
) -> Option<(f64, f64)> {
    let target = constraints.protrusions.iter().find(|target| {
        target.count == 1 && target.symmetry == BeginnerProtrusionSymmetryV1::None
    })?;
    let (minimum_x, maximum_x, minimum_y, maximum_y) =
        skeleton_bounds(&constraints.skeleton_segments)?;
    let span_x = maximum_x.checked_sub(minimum_x)?;
    let span_y = maximum_y.checked_sub(minimum_y)?;
    if span_x <= 0
        || span_y <= 0
        || target.position_tenths_mm[0].checked_mul(2)? != minimum_x.checked_add(maximum_x)?
    {
        return None;
    }
    let primary_span = if vertical { span_y } else { span_x };
    let primary_direction = if vertical {
        target.direction_milli[1]
    } else {
        target.direction_milli[0]
    };
    let length_ratio = f64::from(target.length_tenths_mm) / f64::from(primary_span as u32);
    if !(0.02..=0.45).contains(&length_ratio) || primary_direction == 0 {
        return None;
    }
    let center_y =
        f64::from(target.position_tenths_mm[1].checked_sub(minimum_y)?) / f64::from(span_y as u32);
    let reach = length_ratio
        * (0.75 + f64::from(target.priority) / 400.0)
        * f64::from(primary_direction.unsigned_abs())
        / 1_000.0;
    let point = if vertical {
        (
            0.5,
            if primary_direction < 0 {
                center_y - reach
            } else {
                center_y + reach
            },
        )
    } else {
        (
            if primary_direction < 0 {
                0.5 - reach
            } else {
                0.5 + reach
            },
            center_y,
        )
    };
    ((0.0..1.0).contains(&point.0) && (0.0..1.0).contains(&point.1)).then_some(point)
}

#[must_use]
pub fn beginner_target_approximation_score_v1(constraints: &BeginnerGenerationConstraintsV1) -> u8 {
    if !crate::validate_beginner_generation_constraints_v1(constraints) {
        return 0;
    }
    let feature_records: usize = constraints
        .target_parts
        .iter()
        .filter(|part| {
            !matches!(
                part.kind,
                BeginnerTargetPartKindV1::Head | BeginnerTargetPartKindV1::Torso
            )
        })
        .map(|part| usize::from(part.count))
        .sum();
    if feature_records >= 2
        && feature_records == constraints.protrusions.len()
        && bounded_generic_composite_endpoints(constraints).is_none()
    {
        return 0;
    }
    let target = match constraints.target_category {
        Some(BeginnerTargetCategoryV1::Animal) => {
            if constraints
                .target_parts
                .iter()
                .any(|part| part.kind == BeginnerTargetPartKindV1::Horn && part.count == 1)
            {
                parameterized_center_axis_endpoint(constraints, true).and_then(|_| {
                    constraints.protrusions.iter().find(|target| {
                        target.count == 1 && target.symmetry == BeginnerProtrusionSymmetryV1::None
                    })
                })
            } else if constraints
                .target_parts
                .iter()
                .any(|part| part.kind == BeginnerTargetPartKindV1::Tail && part.count == 1)
            {
                parameterized_center_axis_endpoint(constraints, false).and_then(|_| {
                    constraints.protrusions.iter().find(|target| {
                        target.count == 1 && target.symmetry == BeginnerProtrusionSymmetryV1::None
                    })
                })
            } else {
                let (count, vertical) = if constraints
                    .target_parts
                    .iter()
                    .any(|part| part.kind == BeginnerTargetPartKindV1::Leg && part.count == 4)
                {
                    (4, true)
                } else {
                    (2, false)
                };
                parameterized_symmetric_endpoints(constraints, count, vertical).and_then(|_| {
                    constraints
                        .protrusions
                        .iter()
                        .find(|target| target.count == count)
                })
            }
        }
        Some(BeginnerTargetCategoryV1::Insect) => {
            if let Some(bindings) = insect_complete_bindings_v1(constraints) {
                let ordered = [
                    (bindings.wing_pair_protrusion_id, false),
                    (bindings.antenna_pair_protrusion_id, true),
                    (bindings.leg_pair_protrusion_ids[0], false),
                    (bindings.leg_pair_protrusion_ids[1], false),
                    (bindings.leg_pair_protrusion_ids[2], false),
                ];
                ordered
                    .into_iter()
                    .all(|(id, vertical)| {
                        let mut isolated = constraints.clone();
                        isolated.protrusions.retain(|target| target.id == id);
                        parameterized_symmetric_endpoints(&isolated, 2, vertical).is_some()
                    })
                    .then(|| {
                        constraints
                            .protrusions
                            .iter()
                            .find(|target| target.id == bindings.wing_pair_protrusion_id)
                    })
                    .flatten()
            } else if constraints
                .target_parts
                .iter()
                .any(|part| part.kind == BeginnerTargetPartKindV1::Antenna && part.count == 1)
            {
                parameterized_center_axis_endpoint(constraints, true).and_then(|_| {
                    constraints.protrusions.iter().find(|target| {
                        target.count == 1 && target.symmetry == BeginnerProtrusionSymmetryV1::None
                    })
                })
            } else {
                parameterized_symmetric_endpoints(constraints, 2, false).and_then(|_| {
                    constraints
                        .protrusions
                        .iter()
                        .find(|target| target.count == 2)
                })
            }
        }
        None => None,
    };
    let base = target.map_or_else(
        || {
            if constraints.protrusions.is_empty() {
                estimate_symmetric_parameters_v1(constraints).map_or(0, |estimate| {
                    40 + estimate.scale_percent + estimate.spacing_percent / 5
                })
            } else {
                0
            }
        },
        |target| 60 + target.priority.min(100) * 2 / 5,
    );
    let body_detail = constraints
        .generic_body_outline_tenths_mm
        .as_ref()
        .map_or(0, |outline| outline.len().saturating_sub(4));
    let local_detail = constraints
        .protrusions
        .iter()
        .filter_map(|target| target.local_outline_tenths_mm.as_ref())
        .map(|outline| outline.len().saturating_sub(3))
        .sum::<usize>();
    let contour_bonus =
        u8::try_from(body_detail.saturating_add(local_detail).min(15)).unwrap_or(15);
    base.saturating_add(contour_bonus).min(100)
}

fn has_bilateral_protrusion_count(
    constraints: &BeginnerGenerationConstraintsV1,
    count: u8,
) -> bool {
    constraints.protrusions.iter().any(|target| {
        target.count == count && target.symmetry == BeginnerProtrusionSymmetryV1::Bilateral
    })
}

fn has_bilateral_skeleton(constraints: &BeginnerGenerationConstraintsV1) -> bool {
    let minimum_x = constraints
        .skeleton_segments
        .iter()
        .flat_map(|segment| [segment.start.x_tenths_mm, segment.end.x_tenths_mm])
        .min();
    let maximum_x = constraints
        .skeleton_segments
        .iter()
        .flat_map(|segment| [segment.start.x_tenths_mm, segment.end.x_tenths_mm])
        .max();
    let Some(axis_twice) = minimum_x
        .zip(maximum_x)
        .and_then(|(minimum, maximum)| minimum.checked_add(maximum))
    else {
        return false;
    };
    constraints.skeleton_segments.iter().all(|segment| {
        let mirror_start = (
            axis_twice.checked_sub(segment.start.x_tenths_mm),
            segment.start.y_tenths_mm,
        );
        let mirror_end = (
            axis_twice.checked_sub(segment.end.x_tenths_mm),
            segment.end.y_tenths_mm,
        );
        constraints.skeleton_segments.iter().any(|candidate| {
            candidate.thickness_tenths_mm == segment.thickness_tenths_mm
                && (mirror_start.0 == Some(candidate.start.x_tenths_mm)
                    && mirror_start.1 == candidate.start.y_tenths_mm
                    && mirror_end.0 == Some(candidate.end.x_tenths_mm)
                    && mirror_end.1 == candidate.end.y_tenths_mm
                    || mirror_start.0 == Some(candidate.end.x_tenths_mm)
                        && mirror_start.1 == candidate.end.y_tenths_mm
                        && mirror_end.0 == Some(candidate.start.x_tenths_mm)
                        && mirror_end.1 == candidate.start.y_tenths_mm)
        })
    })
}

fn bounded_generic_composite_endpoints(
    constraints: &BeginnerGenerationConstraintsV1,
) -> Option<Vec<(f64, f64)>> {
    if !(2..=8).contains(&constraints.protrusions.len())
        || constraints
            .protrusions
            .windows(2)
            .any(|pair| pair[0].id >= pair[1].id)
    {
        return None;
    }
    let feature_records: usize = constraints
        .target_parts
        .iter()
        .filter(|part| {
            !matches!(
                part.kind,
                BeginnerTargetPartKindV1::Head | BeginnerTargetPartKindV1::Torso
            )
        })
        .map(|part| usize::from(part.count))
        .sum();
    let feature_kinds = constraints
        .target_parts
        .iter()
        .filter(|part| {
            !matches!(
                part.kind,
                BeginnerTargetPartKindV1::Head | BeginnerTargetPartKindV1::Torso
            )
        })
        .count();
    if feature_records != constraints.protrusions.len()
        && feature_kinds != constraints.protrusions.len()
    {
        return None;
    }
    let (minimum_x, maximum_x, minimum_y, maximum_y) =
        skeleton_bounds(&constraints.skeleton_segments)?;
    let skeleton_body = [
        u32::try_from(maximum_x.checked_sub(minimum_x)?).ok()?,
        u32::try_from(maximum_y.checked_sub(minimum_y)?).ok()?,
    ];
    let available_body = if let Some(outline) = &constraints.generic_body_outline_tenths_mm {
        if outline.iter().any(|point| {
            !(minimum_x..=maximum_x).contains(&point[0])
                || !(minimum_y..=maximum_y).contains(&point[1])
        }) {
            return None;
        }
        let outline_min_x = outline.iter().map(|point| point[0]).min()?;
        let outline_max_x = outline.iter().map(|point| point[0]).max()?;
        let outline_min_y = outline.iter().map(|point| point[1]).min()?;
        let outline_max_y = outline.iter().map(|point| point[1]).max()?;
        [
            u32::try_from(outline_max_x.checked_sub(outline_min_x)?).ok()?,
            u32::try_from(outline_max_y.checked_sub(outline_min_y)?).ok()?,
        ]
    } else {
        skeleton_body
    };
    let body = constraints
        .generic_body_size_tenths_mm
        .unwrap_or(available_body);
    if body
        .iter()
        .zip(available_body)
        .any(|(target, available)| *target == 0 || *target > available)
    {
        return None;
    }
    let mut endpoints = Vec::with_capacity(constraints.protrusions.len() * 4);
    for target in &constraints.protrusions {
        let root_width = target
            .root_width_tenths_mm
            .unwrap_or(u32::from(target.thickness_tenths_mm));
        let tip_width = target.tip_width_tenths_mm.unwrap_or(root_width);
        if tip_width == 0 || tip_width > root_width || root_width > body[0].min(body[1]) {
            return None;
        }
        if target
            .local_outline_tenths_mm
            .as_ref()
            .is_some_and(|outline| {
                outline.iter().any(|point| {
                    let Some(x) = target.position_tenths_mm[0].checked_add(point[0]) else {
                        return true;
                    };
                    let Some(y) = target.position_tenths_mm[1].checked_add(point[1]) else {
                        return true;
                    };
                    !(minimum_x..=maximum_x).contains(&x) || !(minimum_y..=maximum_y).contains(&y)
                })
            })
        {
            return None;
        }
        let mut isolated = constraints.clone();
        isolated
            .protrusions
            .retain(|candidate| candidate.id == target.id);
        let candidates = match (target.count, target.symmetry) {
            (1, BeginnerProtrusionSymmetryV1::None) => {
                vec![
                    parameterized_center_axis_endpoint(
                        &isolated,
                        target.direction_milli[1].unsigned_abs()
                            >= target.direction_milli[0].unsigned_abs(),
                    )
                    .or_else(|| parameterized_landmark_endpoint(&isolated))?,
                ]
            }
            (2 | 4, BeginnerProtrusionSymmetryV1::Bilateral) => parameterized_symmetric_endpoints(
                &isolated,
                target.count,
                target.direction_milli[1].unsigned_abs() > target.direction_milli[0].unsigned_abs(),
            )?
            .to_vec(),
            _ => return None,
        };
        if candidates.iter().any(|candidate| {
            endpoints.iter().any(|existing: &(f64, f64)| {
                (existing.0 - candidate.0).abs() < f64::EPSILON
                    && (existing.1 - candidate.1).abs() < f64::EPSILON
            })
        }) {
            return None;
        }
        endpoints.extend(candidates);
    }
    Some(endpoints)
}

fn parameterized_landmark_endpoint(
    constraints: &BeginnerGenerationConstraintsV1,
) -> Option<(f64, f64)> {
    let target = constraints.protrusions.iter().find(|target| {
        target.count == 1 && target.symmetry == BeginnerProtrusionSymmetryV1::None
    })?;
    let (minimum_x, maximum_x, minimum_y, maximum_y) =
        skeleton_bounds(&constraints.skeleton_segments)?;
    let span_x = maximum_x.checked_sub(minimum_x)?;
    let span_y = maximum_y.checked_sub(minimum_y)?;
    if span_x <= 0 || span_y <= 0 {
        return None;
    }
    let vertical =
        target.direction_milli[1].unsigned_abs() >= target.direction_milli[0].unsigned_abs();
    let primary_span = if vertical { span_y } else { span_x };
    let primary_direction = if vertical {
        target.direction_milli[1]
    } else {
        target.direction_milli[0]
    };
    let length_ratio = f64::from(target.length_tenths_mm) / f64::from(primary_span as u32);
    if !(0.02..=0.45).contains(&length_ratio) || primary_direction == 0 {
        return None;
    }
    let x =
        f64::from(target.position_tenths_mm[0].checked_sub(minimum_x)?) / f64::from(span_x as u32);
    let y =
        f64::from(target.position_tenths_mm[1].checked_sub(minimum_y)?) / f64::from(span_y as u32);
    let reach = length_ratio
        * (0.75 + f64::from(target.priority) / 400.0)
        * f64::from(primary_direction.unsigned_abs())
        / 1_000.0;
    let point = if vertical {
        (
            x,
            if primary_direction < 0 {
                y - reach
            } else {
                y + reach
            },
        )
    } else {
        (
            if primary_direction < 0 {
                x - reach
            } else {
                x + reach
            },
            y,
        )
    };
    ((0.0..1.0).contains(&point.0) && (0.0..1.0).contains(&point.1)).then_some(point)
}

fn parameterized_symmetric_endpoints(
    constraints: &BeginnerGenerationConstraintsV1,
    count: u8,
    vertical: bool,
) -> Option<[(f64, f64); 4]> {
    let target = constraints.protrusions.iter().find(|target| {
        target.count == count && target.symmetry == BeginnerProtrusionSymmetryV1::Bilateral
    })?;
    let (minimum_x, maximum_x, minimum_y, maximum_y) =
        skeleton_bounds(constraints.skeleton_segments.as_slice())?;
    let span_x = maximum_x.checked_sub(minimum_x)?;
    let span_y = maximum_y.checked_sub(minimum_y)?;
    if span_x <= 0 || span_y <= 0 {
        return None;
    }
    let axis_twice = minimum_x.checked_add(maximum_x)?;
    if target.position_tenths_mm[0].checked_mul(2)? != axis_twice
        || !(minimum_y..=maximum_y).contains(&target.position_tenths_mm[1])
    {
        return None;
    }
    let primary_direction = if vertical {
        target.direction_milli[1]
    } else {
        target.direction_milli[0]
    };
    if primary_direction == 0 {
        return None;
    }
    let primary_span = if vertical { span_y } else { span_x };
    let length_ratio = f64::from(target.length_tenths_mm) / f64::from(primary_span as u32);
    let root_width = target
        .root_width_tenths_mm
        .unwrap_or(u32::from(target.thickness_tenths_mm));
    let tip_width = target.tip_width_tenths_mm.unwrap_or(root_width);
    let width_ratio = f64::from(root_width.saturating_add(tip_width))
        / 2.0
        / f64::from(u32::try_from(span_x.min(span_y)).ok()?);
    if !(0.02..=0.45).contains(&length_ratio) || !(0.001..=0.25).contains(&width_ratio) {
        return None;
    }
    let priority_scale = 0.75 + f64::from(target.priority) / 400.0;
    let direction_scale = f64::from(primary_direction.unsigned_abs()) / 1_000.0;
    let reach = length_ratio * priority_scale * direction_scale;
    let spread = (width_ratio * 2.0).clamp(0.05, 0.2);
    let center_offset = target.position_tenths_mm[1].checked_sub(minimum_y)?;
    let center_y = f64::from(center_offset) / f64::from(span_y as u32);
    let endpoints = if vertical {
        [
            (0.5 - spread, center_y - reach),
            (0.5 + spread, center_y - reach),
            (0.5 - spread, center_y + reach),
            (0.5 + spread, center_y + reach),
        ]
    } else {
        [
            (0.5 - reach, center_y - spread),
            (0.5 - reach, center_y + spread),
            (0.5 + reach, center_y - spread),
            (0.5 + reach, center_y + spread),
        ]
    };
    endpoints
        .iter()
        .all(|(x, y)| (0.0..1.0).contains(x) && (0.0..1.0).contains(y))
        .then_some(endpoints)
}

fn skeleton_bounds(segments: &[BeginnerSkeletonSegmentV1]) -> Option<(i32, i32, i32, i32)> {
    Some((
        segments
            .iter()
            .flat_map(|segment| [segment.start.x_tenths_mm, segment.end.x_tenths_mm])
            .min()?,
        segments
            .iter()
            .flat_map(|segment| [segment.start.x_tenths_mm, segment.end.x_tenths_mm])
            .max()?,
        segments
            .iter()
            .flat_map(|segment| [segment.start.y_tenths_mm, segment.end.y_tenths_mm])
            .min()?,
        segments
            .iter()
            .flat_map(|segment| [segment.start.y_tenths_mm, segment.end.y_tenths_mm])
            .max()?,
    ))
}

#[allow(clippy::too_many_arguments)]
fn symmetric_template(
    namespace: ProjectId,
    source: &CreasePattern,
    plan_kind: BeginnerGeneratedPlanKindV1,
    edge_kind: EdgeKind,
    min_x: f64,
    max_x: f64,
    min_y: f64,
    max_y: f64,
    endpoints: &[(f64, f64)],
    instruction: &str,
    constraints: &BeginnerGenerationConstraintsV1,
) -> BeginnerGeneratedPlanV1 {
    let prefix = format!("beginner-plan-{plan_kind:?}");
    let asymmetric_landmark = matches!(
        plan_kind,
        BeginnerGeneratedPlanKindV1::AsymmetricBirdLandmarkBase
            | BeginnerGeneratedPlanKindV1::AsymmetricFourLegLandmarkBase
            | BeginnerGeneratedPlanKindV1::AsymmetricInsectLandmarkBase
            | BeginnerGeneratedPlanKindV1::AsymmetricFishLandmarkBase
    );
    let canonical_quad = asymmetric_landmark
        .then(|| {
            let points = source
                .vertices
                .iter()
                .map(|vertex| vertex.position)
                .collect::<Vec<_>>();
            canonical_asymmetric_quad(&points)
        })
        .flatten();
    let asymmetric_namespace = ProjectId::schema_namespace([
        0x01, 0x90, 0x00, 0x00, 0x00, 0x00, 0x70, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04,
        0x97,
    ]);
    let center = canonical_quad.as_ref().map_or_else(
        || Point2::new((min_x + max_x) / 2.0, (min_y + max_y) / 2.0),
        |(center, _)| *center,
    );
    let center_id = source
        .vertices
        .iter()
        .find(|vertex| vertex.position == center)
        .map_or_else(
            || {
                if canonical_quad.is_some() {
                    VertexId::derive_v5(asymmetric_namespace, b"vertex-4")
                } else {
                    VertexId::derive_v5(namespace, format!("{prefix}-center").as_bytes())
                }
            },
            |vertex| vertex.id,
        );
    let mut vertices = vec![Vertex {
        id: center_id,
        position: center,
    }];
    let mut edges = Vec::with_capacity(endpoints.len());
    let mut asymmetric_edge_ids = (0..endpoints.len())
        .map(|index| {
            if !asymmetric_landmark {
                return EdgeId::derive_v5(namespace, format!("{prefix}-e-{index}").as_bytes());
            }
            EdgeId::derive_v5(asymmetric_namespace, &(index as u64).to_be_bytes())
        })
        .collect::<Vec<_>>();
    if asymmetric_landmark {
        asymmetric_edge_ids.sort_unstable_by_key(EdgeId::canonical_bytes);
    }
    for (index, (x_ratio, y_ratio)) in endpoints.iter().copied().enumerate() {
        let position = canonical_quad.as_ref().map_or_else(
            || {
                Point2::new(
                    min_x + (max_x - min_x) * x_ratio,
                    min_y + (max_y - min_y) * y_ratio,
                )
            },
            |(_, points)| points[index],
        );
        let id = source
            .vertices
            .iter()
            .find(|vertex| vertex.position == position)
            .map_or_else(
                || VertexId::derive_v5(namespace, format!("{prefix}-v-{index}").as_bytes()),
                |vertex| vertex.id,
            );
        if !vertices.iter().any(|vertex| vertex.id == id) {
            vertices.push(Vertex { id, position });
        }
        edges.push(Edge {
            id: asymmetric_edge_ids[index],
            start: if asymmetric_landmark { id } else { center_id },
            end: if asymmetric_landmark { center_id } else { id },
            kind: if asymmetric_landmark && index == 3 {
                EdgeKind::Mountain
            } else {
                edge_kind
            },
        });
    }
    if let Some(outline) = &constraints.generic_body_outline_tenths_mm {
        let Some((skeleton_min_x, skeleton_max_x, skeleton_min_y, skeleton_max_y)) =
            skeleton_bounds(&constraints.skeleton_segments)
        else {
            return BeginnerGeneratedPlanV1 {
                schema_version: BEGINNER_GENERATOR_SCHEMA_VERSION_V1,
                kind: plan_kind,
                crease_pattern: CreasePattern { vertices, edges },
                instruction_codes: vec![instruction.to_owned()],
                target_parts: constraints.target_parts.clone(),
                skeleton_segments: constraints.skeleton_segments.clone(),
                target_asset: constraints.target_asset,
                semantic_landmark_provenance: asymmetric_insect_semantic_provenance(plan_kind),
            };
        };
        let skeleton_span_x = f64::from(skeleton_max_x - skeleton_min_x);
        let skeleton_span_y = f64::from(skeleton_max_y - skeleton_min_y);
        let outline_ids = outline
            .iter()
            .enumerate()
            .map(|(index, point)| {
                let position = Point2::new(
                    min_x
                        + (max_x - min_x) * f64::from(point[0] - skeleton_min_x) / skeleton_span_x,
                    min_y
                        + (max_y - min_y) * f64::from(point[1] - skeleton_min_y) / skeleton_span_y,
                );
                let id =
                    VertexId::derive_v5(namespace, format!("{prefix}-body-v-{index}").as_bytes());
                vertices.push(Vertex { id, position });
                id
            })
            .collect::<Vec<_>>();
        for index in 0..outline_ids.len() {
            edges.push(Edge {
                id: EdgeId::derive_v5(namespace, format!("{prefix}-body-e-{index}").as_bytes()),
                start: outline_ids[index],
                end: outline_ids[(index + 1) % outline_ids.len()],
                kind: edge_kind,
            });
        }
    }
    if let Some((skeleton_min_x, skeleton_max_x, skeleton_min_y, skeleton_max_y)) =
        skeleton_bounds(&constraints.skeleton_segments)
    {
        let skeleton_span_x = f64::from(skeleton_max_x - skeleton_min_x);
        let skeleton_span_y = f64::from(skeleton_max_y - skeleton_min_y);
        for target in &constraints.protrusions {
            let Some(outline) = &target.local_outline_tenths_mm else {
                continue;
            };
            let outline_ids = outline
                .iter()
                .enumerate()
                .map(|(index, point)| {
                    let x = target.position_tenths_mm[0] + point[0];
                    let y = target.position_tenths_mm[1] + point[1];
                    let position = Point2::new(
                        min_x + (max_x - min_x) * f64::from(x - skeleton_min_x) / skeleton_span_x,
                        min_y + (max_y - min_y) * f64::from(y - skeleton_min_y) / skeleton_span_y,
                    );
                    let id = VertexId::derive_v5(
                        namespace,
                        format!("{prefix}-local-{}-v-{index}", target.id).as_bytes(),
                    );
                    vertices.push(Vertex { id, position });
                    id
                })
                .collect::<Vec<_>>();
            for index in 0..outline_ids.len() {
                edges.push(Edge {
                    id: EdgeId::derive_v5(
                        namespace,
                        format!("{prefix}-local-{}-e-{index}", target.id).as_bytes(),
                    ),
                    start: outline_ids[index],
                    end: outline_ids[(index + 1) % outline_ids.len()],
                    kind: edge_kind,
                });
            }
        }
    }
    BeginnerGeneratedPlanV1 {
        schema_version: BEGINNER_GENERATOR_SCHEMA_VERSION_V1,
        kind: plan_kind,
        crease_pattern: CreasePattern { vertices, edges },
        instruction_codes: vec![instruction.to_owned()],
        target_parts: constraints.target_parts.clone(),
        skeleton_segments: constraints.skeleton_segments.clone(),
        target_asset: constraints.target_asset,
        semantic_landmark_provenance: asymmetric_insect_semantic_provenance(plan_kind),
    }
}

fn asymmetric_insect_semantic_provenance(
    plan_kind: BeginnerGeneratedPlanKindV1,
) -> Option<BeginnerSemanticLandmarkProvenanceV1> {
    let (roles, hash_domain): (&[&str], &[u8]) = match plan_kind {
        BeginnerGeneratedPlanKindV1::AsymmetricInsectLandmarkBase => (
            &[
                "head",
                "tail",
                "wing_left",
                "wing_right",
                "leg_front_left",
                "leg_front_right",
                "leg_middle_left",
                "leg_middle_right",
                "leg_rear_left",
                "leg_rear_right",
            ],
            b"ORIGAMI2_ASYMMETRIC_INSECT_RAY_GROUP_V1",
        ),
        BeginnerGeneratedPlanKindV1::AsymmetricFishLandmarkBase => (
            &["head", "tail", "fin_left", "fin_right"],
            b"ORIGAMI2_ASYMMETRIC_FISH_RAY_GROUP_V1",
        ),
        _ => return None,
    };
    let ordered_bindings = roles
        .into_iter()
        .enumerate()
        .map(|(ordinal, role)| BeginnerSemanticLandmarkBindingV1 {
            ordinal: u8::try_from(ordinal).expect("ten semantic landmarks fit in u8"),
            role: (*role).to_owned(),
            physical_ray: u8::try_from(ordinal % 4).expect("four physical rays fit in u8"),
        })
        .collect::<Vec<_>>();
    let physical_ray_group_sha256 = std::array::from_fn(|physical_ray| {
        let mut hash = Sha256::new();
        hash.update(hash_domain);
        hash.update([physical_ray as u8]);
        for binding in ordered_bindings
            .iter()
            .filter(|binding| usize::from(binding.physical_ray) == physical_ray)
        {
            hash.update([binding.ordinal]);
            hash.update(binding.role.as_bytes());
        }
        hash.finalize().into()
    });
    Some(BeginnerSemanticLandmarkProvenanceV1 {
        schema_version: 1,
        ordered_bindings,
        physical_ray_group_sha256,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generator_is_bounded_deterministic_and_fail_closed() {
        let namespace = ProjectId::new();
        let ids = ["a", "b", "c", "d"].map(|name| VertexId::derive_v5(namespace, name.as_bytes()));
        let source = CreasePattern {
            vertices: ids
                .into_iter()
                .zip([
                    Point2::new(0.0, 0.0),
                    Point2::new(10.0, 0.0),
                    Point2::new(10.0, 10.0),
                    Point2::new(0.0, 10.0),
                ])
                .map(|(id, position)| Vertex { id, position })
                .collect(),
            edges: Vec::new(),
        };
        let constraints = BeginnerGenerationConstraintsV1 {
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
                BeginnerTargetPartRecordV1 {
                    kind: BeginnerTargetPartKindV1::Leg,
                    count: 4,
                },
            ],
            skeleton_segments: vec![
                skeleton(1, -10, 0, 0, 10),
                skeleton(2, 10, 0, 0, 10),
                skeleton(3, 0, -10, 0, 10),
            ],
            protrusions: vec![bilateral_protrusion(1, 4)],
            ..BeginnerGenerationConstraintsV1::default()
        };
        let first = generate_beginner_plans_v1(namespace, &source, &ids, &constraints).unwrap();
        let second = generate_beginner_plans_v1(namespace, &source, &ids, &constraints).unwrap();
        assert_eq!(beginner_target_approximation_score_v1(&constraints), 92);
        assert_eq!(first, second);
        assert_eq!(first.len(), 3);
        assert_eq!(
            first[0].kind,
            BeginnerGeneratedPlanKindV1::SymmetricFourLegBase
        );
        assert_eq!(first[0].crease_pattern.edges.len(), 4);
        assert!(
            first[1..]
                .iter()
                .all(|plan| plan.crease_pattern.edges.len() == 1)
        );
        for (part_kind, expected_kind) in [
            (
                BeginnerTargetPartKindV1::Wing,
                BeginnerGeneratedPlanKindV1::SymmetricBirdBase,
            ),
            (
                BeginnerTargetPartKindV1::Fin,
                BeginnerGeneratedPlanKindV1::SymmetricFishBase,
            ),
            (
                BeginnerTargetPartKindV1::Ear,
                BeginnerGeneratedPlanKindV1::SymmetricEarBase,
            ),
            (
                BeginnerTargetPartKindV1::Horn,
                BeginnerGeneratedPlanKindV1::SymmetricHornBase,
            ),
        ] {
            let mut family = constraints.clone();
            family.target_parts[2] = BeginnerTargetPartRecordV1 {
                kind: part_kind,
                count: 2,
            };
            family.skeleton_segments.truncate(2);
            family.protrusions[0] = bilateral_protrusion(1, 2);
            let plans = generate_beginner_plans_v1(namespace, &source, &ids, &family).unwrap();
            assert_eq!(plans[0].kind, expected_kind);
            assert_eq!(plans[0].crease_pattern.edges.len(), 4);
            assert_eq!(beginner_target_approximation_score_v1(&family), 92);
        }
        let mut tail = constraints.clone();
        tail.target_parts[2] = BeginnerTargetPartRecordV1 {
            kind: BeginnerTargetPartKindV1::Tail,
            count: 1,
        };
        tail.protrusions[0] = bilateral_protrusion(1, 1);
        tail.protrusions[0].symmetry = BeginnerProtrusionSymmetryV1::None;
        tail.protrusions[0].direction_milli = [1_000, 0, 0];
        let tail_plans = generate_beginner_plans_v1(namespace, &source, &ids, &tail).unwrap();
        assert_eq!(
            tail_plans[0].kind,
            BeginnerGeneratedPlanKindV1::CenterAxisTailBase
        );
        assert_eq!(tail_plans[0].crease_pattern.vertices.len(), 2);
        assert_eq!(tail_plans[0].crease_pattern.edges.len(), 1);
        let mut composite = tail.clone();
        composite.target_parts.push(BeginnerTargetPartRecordV1 {
            kind: BeginnerTargetPartKindV1::Ear,
            count: 2,
        });
        composite.protrusions.push(bilateral_protrusion(2, 2));
        let composite_plans =
            generate_beginner_plans_v1(namespace, &source, &ids, &composite).unwrap();
        assert_eq!(
            composite_plans[0].kind,
            BeginnerGeneratedPlanKindV1::CompositeTailEarBase
        );
        assert_eq!(composite_plans[0].crease_pattern.vertices.len(), 6);
        assert_eq!(composite_plans[0].crease_pattern.edges.len(), 5);
        assert_eq!(
            animal_tail_ear_bindings_v1(&composite),
            Some(BeginnerTailEarBindingV1 {
                tail_protrusion_id: 1,
                ear_pair_protrusion_id: 2
            })
        );
        tail.protrusions[0].symmetry = BeginnerProtrusionSymmetryV1::Bilateral;
        assert_eq!(
            generate_beginner_plans_v1(namespace, &source, &ids, &tail),
            Err(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate)
        );
        let mut horn = constraints.clone();
        horn.target_parts[2] = BeginnerTargetPartRecordV1 {
            kind: BeginnerTargetPartKindV1::Horn,
            count: 1,
        };
        horn.protrusions[0] = bilateral_protrusion(1, 1);
        horn.protrusions[0].symmetry = BeginnerProtrusionSymmetryV1::None;
        horn.protrusions[0].direction_milli = [0, -1_000, 0];
        let horn_plans = generate_beginner_plans_v1(namespace, &source, &ids, &horn).unwrap();
        assert_eq!(
            horn_plans[0].kind,
            BeginnerGeneratedPlanKindV1::CenterAxisHornBase
        );
        assert_eq!(horn_plans[0].crease_pattern.vertices.len(), 2);
        assert_eq!(horn_plans[0].crease_pattern.edges.len(), 1);
        let mut horn_tail = horn.clone();
        horn_tail.target_parts.push(BeginnerTargetPartRecordV1 {
            kind: BeginnerTargetPartKindV1::Tail,
            count: 1,
        });
        let mut tail_target = horn_tail.protrusions[0].clone();
        tail_target.id = 2;
        tail_target.direction_milli = [1_000, 0, 0];
        horn_tail.protrusions.push(tail_target);
        let horn_tail_plans =
            generate_beginner_plans_v1(namespace, &source, &ids, &horn_tail).unwrap();
        assert_eq!(
            horn_tail_plans[0].kind,
            BeginnerGeneratedPlanKindV1::CompositeHornTailBase
        );
        assert_eq!(horn_tail_plans[0].crease_pattern.edges.len(), 2);
        assert_eq!(
            animal_horn_tail_bindings_v1(&horn_tail),
            Some(BeginnerHornTailBindingV1 {
                horn_protrusion_id: 1,
                tail_protrusion_id: 2,
            })
        );
        let mut triple = horn_tail.clone();
        triple.target_parts.push(BeginnerTargetPartRecordV1 {
            kind: BeginnerTargetPartKindV1::Ear,
            count: 2,
        });
        triple.protrusions.push(bilateral_protrusion(3, 2));
        let triple_plans = generate_beginner_plans_v1(namespace, &source, &ids, &triple).unwrap();
        assert_eq!(
            triple_plans[0].kind,
            BeginnerGeneratedPlanKindV1::CompositeHornTailEarBase
        );
        assert_eq!(triple_plans[0].crease_pattern.vertices.len(), 7);
        assert_eq!(triple_plans[0].crease_pattern.edges.len(), 6);
        assert_eq!(
            animal_horn_tail_ear_bindings_v1(&triple),
            Some(BeginnerHornTailEarBindingV1 {
                horn_protrusion_id: 1,
                tail_protrusion_id: 2,
                ear_pair_protrusion_id: 3,
            })
        );
        let mut complete_animal = triple.clone();
        complete_animal
            .target_parts
            .push(BeginnerTargetPartRecordV1 {
                kind: BeginnerTargetPartKindV1::Leg,
                count: 4,
            });
        let mut legs = bilateral_protrusion(4, 4);
        legs.direction_milli = [0, 1_000, 0];
        complete_animal.protrusions.push(legs.clone());
        let complete_animal_plans =
            generate_beginner_plans_v1(namespace, &source, &ids, &complete_animal).unwrap();
        assert_eq!(
            complete_animal_plans[0].kind,
            BeginnerGeneratedPlanKindV1::CompositeCompleteAnimalBase
        );
        assert_eq!(complete_animal_plans[0].crease_pattern.vertices.len(), 11);
        assert_eq!(complete_animal_plans[0].crease_pattern.edges.len(), 10);
        assert_eq!(
            animal_complete_bindings_v1(&complete_animal),
            Some(BeginnerCompleteAnimalBindingV1 {
                horn_protrusion_id: 1,
                tail_protrusion_id: 2,
                ear_pair_protrusion_id: 3,
                leg_protrusion_id: 4,
            })
        );
        let mut duplicate_leg = complete_animal.clone();
        duplicate_leg.protrusions.push(legs);
        assert_eq!(animal_complete_bindings_v1(&duplicate_leg), None);
        let mut missing_leg = complete_animal.clone();
        missing_leg.protrusions.retain(|target| target.id != 4);
        assert_eq!(animal_complete_bindings_v1(&missing_leg), None);
        let mut winged_animal = complete_animal.clone();
        winged_animal.target_parts.push(BeginnerTargetPartRecordV1 {
            kind: BeginnerTargetPartKindV1::Wing,
            count: 2,
        });
        let mut wings = bilateral_protrusion(5, 2);
        wings.priority = 60;
        winged_animal.protrusions.push(wings);
        let winged_plans =
            generate_beginner_plans_v1(namespace, &source, &ids, &winged_animal).unwrap();
        assert_eq!(
            winged_plans[0].kind,
            BeginnerGeneratedPlanKindV1::CompositeCompleteWingedAnimalBase
        );
        assert_eq!(winged_plans[0].crease_pattern.vertices.len(), 15);
        assert_eq!(winged_plans[0].crease_pattern.edges.len(), 14);
        assert_eq!(
            animal_complete_winged_bindings_v1(&winged_animal),
            Some(BeginnerCompleteWingedAnimalBindingV1 {
                animal: BeginnerCompleteAnimalBindingV1 {
                    horn_protrusion_id: 1,
                    tail_protrusion_id: 2,
                    ear_pair_protrusion_id: 3,
                    leg_protrusion_id: 4,
                },
                wing_pair_protrusion_id: 5,
            })
        );
        assert_eq!(animal_complete_bindings_v1(&winged_animal), None);
        let mut forged_wing = winged_animal.clone();
        forged_wing.protrusions[4].id = 4;
        assert_eq!(animal_complete_winged_bindings_v1(&forged_wing), None);
        let mut horn_ear = horn.clone();
        horn_ear.target_parts.push(BeginnerTargetPartRecordV1 {
            kind: BeginnerTargetPartKindV1::Ear,
            count: 2,
        });
        horn_ear.protrusions.push(bilateral_protrusion(2, 2));
        let horn_ear_plans =
            generate_beginner_plans_v1(namespace, &source, &ids, &horn_ear).unwrap();
        assert_eq!(
            horn_ear_plans[0].kind,
            BeginnerGeneratedPlanKindV1::CompositeHornEarBase
        );
        assert_eq!(horn_ear_plans[0].crease_pattern.edges.len(), 5);
        assert_eq!(
            animal_horn_ear_bindings_v1(&horn_ear),
            Some(BeginnerHornEarBindingV1 {
                horn_protrusion_id: 1,
                ear_pair_protrusion_id: 2,
            })
        );
        let mut generic = constraints.clone();
        generic.target_parts.push(BeginnerTargetPartRecordV1 {
            kind: BeginnerTargetPartKindV1::Fin,
            count: 2,
        });
        let mut fin = bilateral_protrusion(2, 2);
        fin.priority = 60;
        generic.protrusions.push(fin);
        let generic_plans = generate_beginner_plans_v1(namespace, &source, &ids, &generic).unwrap();
        assert_eq!(
            generic_plans[0].kind,
            BeginnerGeneratedPlanKindV1::CompositeGenericTargetBase
        );
        assert_eq!(generic_plans[0].crease_pattern.vertices.len(), 9);
        assert_eq!(generic_plans[0].crease_pattern.edges.len(), 8);
        let mut locally_outlined = generic.clone();
        locally_outlined.protrusions[0].local_outline_tenths_mm =
            Some(vec![[-2, -1], [0, -2], [2, -1], [1, 2], [-1, 2]]);
        let local_plans =
            generate_beginner_plans_v1(namespace, &source, &ids, &locally_outlined).unwrap();
        assert_eq!(local_plans[0].crease_pattern.vertices.len(), 14);
        assert_eq!(local_plans[0].crease_pattern.edges.len(), 13);
        assert!(
            beginner_target_approximation_score_v1(&locally_outlined)
                > beginner_target_approximation_score_v1(&generic)
        );
        let mut one_over_local_limit = locally_outlined.clone();
        one_over_local_limit.protrusions[0].local_outline_tenths_mm = Some(vec![
            [-4, -2],
            [-2, -4],
            [0, -5],
            [2, -4],
            [4, -2],
            [4, 2],
            [2, 4],
            [0, 5],
            [-2, 4],
        ]);
        assert_eq!(
            beginner_target_approximation_score_v1(&one_over_local_limit),
            0
        );
        locally_outlined.protrusions[0].local_outline_tenths_mm =
            Some(vec![[-20, -1], [20, -1], [0, 2]]);
        assert_eq!(
            generate_beginner_plans_v1(namespace, &source, &ids, &locally_outlined),
            Err(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate)
        );
        let mut outlined_generic = generic.clone();
        outlined_generic.generic_body_outline_tenths_mm =
            Some(vec![[-5, -5], [-5, 5], [5, 5], [5, -5]]);
        let outlined_plans =
            generate_beginner_plans_v1(namespace, &source, &ids, &outlined_generic).unwrap();
        assert_eq!(outlined_plans[0].crease_pattern.vertices.len(), 13);
        assert_eq!(outlined_plans[0].crease_pattern.edges.len(), 12);
        let mut general_outline = generic.clone();
        general_outline.generic_body_outline_mode = crate::BeginnerBodyOutlineModeV1::General;
        general_outline.generic_body_outline_tenths_mm =
            Some(vec![[-5, -5], [5, -5], [4, 5], [-3, 5]]);
        let general_plans =
            generate_beginner_plans_v1(namespace, &source, &ids, &general_outline).unwrap();
        assert_eq!(general_plans[0].crease_pattern.vertices.len(), 13);
        assert_eq!(general_plans[0].crease_pattern.edges.len(), 12);
        let mut tapered_generic = generic.clone();
        tapered_generic.protrusions[1].root_width_tenths_mm = Some(1);
        tapered_generic.protrusions[1].tip_width_tenths_mm = Some(1);
        let tapered_plans =
            generate_beginner_plans_v1(namespace, &source, &ids, &tapered_generic).unwrap();
        assert_ne!(
            tapered_plans[0].crease_pattern.vertices,
            generic_plans[0].crease_pattern.vertices
        );
        tapered_generic.protrusions[1].tip_width_tenths_mm = Some(2);
        assert_eq!(
            generate_beginner_plans_v1(namespace, &source, &ids, &tapered_generic),
            Err(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate)
        );
        tapered_generic.protrusions[1].tip_width_tenths_mm = Some(1);
        tapered_generic.generic_body_size_tenths_mm = Some([1_000_000, 1_000_000]);
        assert_eq!(
            generate_beginner_plans_v1(namespace, &source, &ids, &tapered_generic),
            Err(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate)
        );
        let mut intersecting_generic = generic.clone();
        let mut overlapping = intersecting_generic.protrusions[0].clone();
        overlapping.id = 2;
        intersecting_generic.protrusions[1] = overlapping;
        intersecting_generic.target_parts.last_mut().unwrap().count = 1;
        assert_eq!(
            generate_beginner_plans_v1(namespace, &source, &ids, &intersecting_generic),
            Err(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate)
        );
        let mut reordered_generic = generic.clone();
        reordered_generic.protrusions.reverse();
        assert_eq!(
            generate_beginner_plans_v1(namespace, &source, &ids, &reordered_generic),
            Err(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate)
        );
        let mut higher_priority = constraints.clone();
        higher_priority.protrusions[0].priority = 100;
        let scaled =
            generate_beginner_plans_v1(namespace, &source, &ids, &higher_priority).unwrap();
        assert_ne!(
            first[0].crease_pattern.vertices,
            scaled[0].crease_pattern.vertices
        );
        let mut shorter_direction = constraints.clone();
        shorter_direction.protrusions[0].direction_milli[1] = 500;
        let direction_scaled =
            generate_beginner_plans_v1(namespace, &source, &ids, &shorter_direction).unwrap();
        assert_ne!(
            first[0].crease_pattern.vertices,
            direction_scaled[0].crease_pattern.vertices
        );
        shorter_direction.protrusions[0].direction_milli[1] = 0;
        assert_eq!(
            generate_beginner_plans_v1(namespace, &source, &ids, &shorter_direction),
            Err(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate)
        );
        let mut outside_paper = constraints.clone();
        outside_paper.protrusions[0].length_tenths_mm = 10;
        assert_eq!(beginner_target_approximation_score_v1(&outside_paper), 0);
        assert_eq!(
            generate_beginner_plans_v1(namespace, &source, &ids, &outside_paper),
            Err(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate)
        );
        assert_eq!(
            generate_beginner_plans_v1(namespace, &source, &ids[..3], &constraints),
            Err(BeginnerGeneratorErrorV1::UnsupportedPaper)
        );
    }

    #[test]
    fn wing_template_is_explicit_and_unsupported_inputs_fail_closed() {
        let namespace = ProjectId::new();
        let ids = ["a", "b", "c", "d"].map(|name| VertexId::derive_v5(namespace, name.as_bytes()));
        let source = CreasePattern {
            vertices: ids
                .into_iter()
                .zip([
                    Point2::new(0.0, 0.0),
                    Point2::new(20.0, 0.0),
                    Point2::new(20.0, 10.0),
                    Point2::new(0.0, 10.0),
                ])
                .map(|(id, position)| Vertex { id, position })
                .collect(),
            edges: Vec::new(),
        };
        let mut constraints = BeginnerGenerationConstraintsV1 {
            target_category: Some(BeginnerTargetCategoryV1::Insect),
            target_parts: vec![
                BeginnerTargetPartRecordV1 {
                    kind: BeginnerTargetPartKindV1::Head,
                    count: 1,
                },
                BeginnerTargetPartRecordV1 {
                    kind: BeginnerTargetPartKindV1::Torso,
                    count: 1,
                },
                BeginnerTargetPartRecordV1 {
                    kind: BeginnerTargetPartKindV1::Wing,
                    count: 2,
                },
            ],
            skeleton_segments: vec![skeleton(1, -10, 0, 0, 10), skeleton(2, 10, 0, 0, 10)],
            protrusions: vec![bilateral_protrusion(1, 2)],
            ..BeginnerGenerationConstraintsV1::default()
        };
        let plans = generate_beginner_plans_v1(namespace, &source, &ids, &constraints).unwrap();
        assert_eq!(
            plans[0].kind,
            BeginnerGeneratedPlanKindV1::SymmetricWingBase
        );
        assert_eq!(plans[0].crease_pattern.edges.len(), 4);
        let mut asymmetric = constraints.clone();
        asymmetric.target_category = Some(BeginnerTargetCategoryV1::Animal);
        let mut left = bilateral_protrusion(1, 1);
        left.symmetry = BeginnerProtrusionSymmetryV1::None;
        left.position_tenths_mm = [-4, 0, 0];
        left.direction_milli = [-1_000, 200, 0];
        let mut right = bilateral_protrusion(2, 1);
        right.symmetry = BeginnerProtrusionSymmetryV1::None;
        right.position_tenths_mm = [5, 1, 0];
        right.direction_milli = [1_000, -100, 0];
        asymmetric.protrusions = vec![left, right];
        let asymmetric_plans =
            generate_beginner_plans_v1(namespace, &source, &ids, &asymmetric).unwrap();
        assert_eq!(
            asymmetric_plans[0].kind,
            BeginnerGeneratedPlanKindV1::AsymmetricBirdLandmarkBase
        );
        let mut antenna = constraints.clone();
        antenna.target_parts[2].kind = BeginnerTargetPartKindV1::Antenna;
        let antenna_plans = generate_beginner_plans_v1(namespace, &source, &ids, &antenna).unwrap();
        assert_eq!(
            antenna_plans[0].kind,
            BeginnerGeneratedPlanKindV1::SymmetricAntennaBase
        );
        let mut wing_antenna = constraints.clone();
        wing_antenna.target_parts.push(BeginnerTargetPartRecordV1 {
            kind: BeginnerTargetPartKindV1::Antenna,
            count: 2,
        });
        let mut antenna_target = bilateral_protrusion(2, 2);
        antenna_target.direction_milli = [0, -1_000, 0];
        antenna_target.length_tenths_mm = 4;
        wing_antenna.protrusions.push(antenna_target);
        let composite_plans =
            generate_beginner_plans_v1(namespace, &source, &ids, &wing_antenna).unwrap();
        assert_eq!(
            composite_plans[0].kind,
            BeginnerGeneratedPlanKindV1::CompositeWingAntennaBase
        );
        assert_eq!(composite_plans[0].crease_pattern.vertices.len(), 9);
        assert_eq!(composite_plans[0].crease_pattern.edges.len(), 8);
        assert_eq!(
            insect_wing_antenna_bindings_v1(&wing_antenna),
            Some(BeginnerWingAntennaBindingV1 {
                wing_pair_protrusion_id: 1,
                antenna_pair_protrusion_id: 2,
            })
        );
        let mut complete = wing_antenna.clone();
        complete.target_parts.push(BeginnerTargetPartRecordV1 {
            kind: BeginnerTargetPartKindV1::Leg,
            count: 6,
        });
        complete.protrusions[0].priority = 60;
        complete.protrusions[1].priority = 60;
        for (index, y) in [3, 5, 7].into_iter().enumerate() {
            let mut leg = bilateral_protrusion(index as u16 + 3, 2);
            leg.priority = 50;
            leg.position_tenths_mm[1] = y;
            complete.protrusions.push(leg);
        }
        let complete_plans =
            generate_beginner_plans_v1(namespace, &source, &ids, &complete).unwrap();
        assert_eq!(
            complete_plans[0].kind,
            BeginnerGeneratedPlanKindV1::CompositeCompleteInsectBase
        );
        assert_eq!(complete_plans[0].crease_pattern.vertices.len(), 21);
        assert_eq!(complete_plans[0].crease_pattern.edges.len(), 20);
        assert_eq!(
            insect_complete_bindings_v1(&complete)
                .unwrap()
                .leg_pair_protrusion_ids,
            [3, 4, 5]
        );
        let mut reordered = complete.clone();
        reordered.protrusions.reverse();
        let reordered_binding = insect_complete_bindings_v1(&reordered).unwrap();
        assert_eq!(reordered_binding.wing_pair_protrusion_id, 1);
        assert_eq!(reordered_binding.antenna_pair_protrusion_id, 2);
        assert_eq!(reordered_binding.leg_pair_protrusion_ids, [3, 4, 5]);
        assert_eq!(
            beginner_target_approximation_score_v1(&reordered),
            beginner_target_approximation_score_v1(&complete)
        );

        let mut duplicate_id = complete.clone();
        duplicate_id.protrusions[4].id = duplicate_id.protrusions[3].id;
        assert_eq!(insect_complete_bindings_v1(&duplicate_id), None);
        let mut duplicate_position = complete.clone();
        duplicate_position.protrusions[4].position_tenths_mm[1] =
            duplicate_position.protrusions[3].position_tenths_mm[1];
        assert_eq!(insect_complete_bindings_v1(&duplicate_position), None);
        let mut oversized = complete.clone();
        let mut extra_leg = bilateral_protrusion(6, 2);
        extra_leg.priority = 50;
        extra_leg.position_tenths_mm[1] = 9;
        oversized.protrusions.push(extra_leg);
        assert_eq!(insect_complete_bindings_v1(&oversized), None);
        let mut ambiguous_priority = complete.clone();
        ambiguous_priority.protrusions[2].priority = 60;
        assert_eq!(insect_complete_bindings_v1(&ambiguous_priority), None);
        let mut duplicate_part = complete.clone();
        duplicate_part
            .target_parts
            .push(BeginnerTargetPartRecordV1 {
                kind: BeginnerTargetPartKindV1::Wing,
                count: 2,
            });
        assert_eq!(insect_complete_bindings_v1(&duplicate_part), None);
        let mut single_antenna = antenna.clone();
        single_antenna.target_parts[2].count = 1;
        single_antenna.protrusions[0].count = 1;
        single_antenna.protrusions[0].symmetry = BeginnerProtrusionSymmetryV1::None;
        single_antenna.protrusions[0].direction_milli = [0, -1_000, 0];
        single_antenna.protrusions[0].length_tenths_mm = 4;
        let single_antenna_plans =
            generate_beginner_plans_v1(namespace, &source, &ids, &single_antenna).unwrap();
        assert_eq!(
            single_antenna_plans[0].kind,
            BeginnerGeneratedPlanKindV1::CenterAxisAntennaBase
        );
        assert_eq!(single_antenna_plans[0].crease_pattern.edges.len(), 1);
        let mut leg_pair = constraints.clone();
        leg_pair.target_parts[2].kind = BeginnerTargetPartKindV1::Leg;
        let leg_plans = generate_beginner_plans_v1(namespace, &source, &ids, &leg_pair).unwrap();
        assert_eq!(
            leg_plans[0].kind,
            BeginnerGeneratedPlanKindV1::SymmetricInsectLegPairBase
        );
        let mut complete_legs = constraints.clone();
        complete_legs.target_parts[2] = BeginnerTargetPartRecordV1 {
            kind: BeginnerTargetPartKindV1::Leg,
            count: 6,
        };
        complete_legs.protrusions = [3_i32, 5, 7]
            .into_iter()
            .enumerate()
            .map(|(index, center_y)| {
                let mut target = bilateral_protrusion(index as u16 + 1, 2);
                target.position_tenths_mm[1] = center_y;
                target
            })
            .collect();
        assert_eq!(
            insect_three_pair_bindings_v1(&complete_legs),
            Some([
                BeginnerBilateralPairBindingV1 {
                    pair_index: 0,
                    protrusion_id: 1,
                    center_y_tenths_mm: 3
                },
                BeginnerBilateralPairBindingV1 {
                    pair_index: 1,
                    protrusion_id: 2,
                    center_y_tenths_mm: 5
                },
                BeginnerBilateralPairBindingV1 {
                    pair_index: 2,
                    protrusion_id: 3,
                    center_y_tenths_mm: 7
                },
            ])
        );
        let complete_plans =
            generate_beginner_plans_v1(namespace, &source, &ids, &complete_legs).unwrap();
        assert_eq!(
            complete_plans[0].kind,
            BeginnerGeneratedPlanKindV1::SymmetricSixLegBase
        );
        assert_eq!(complete_plans[0].crease_pattern.vertices.len(), 13);
        assert_eq!(complete_plans[0].crease_pattern.edges.len(), 12);
        complete_legs.protrusions[2].position_tenths_mm[1] = 5;
        assert_eq!(insect_three_pair_bindings_v1(&complete_legs), None);
        constraints.skeleton_segments[1].end.y_tenths_mm = 11;
        assert_eq!(
            generate_beginner_plans_v1(namespace, &source, &ids, &constraints),
            Err(BeginnerGeneratorErrorV1::UnsupportedInsectTemplate)
        );
        constraints.skeleton_segments[1].end.y_tenths_mm = 10;
        constraints.protrusions[0].symmetry = BeginnerProtrusionSymmetryV1::None;
        assert_eq!(
            generate_beginner_plans_v1(namespace, &source, &ids, &constraints),
            Err(BeginnerGeneratorErrorV1::UnsupportedInsectTemplate)
        );
    }

    #[test]
    fn target_parts_estimate_bounded_symmetric_parameters_deterministically() {
        let constraints = BeginnerGenerationConstraintsV1 {
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
                BeginnerTargetPartRecordV1 {
                    kind: BeginnerTargetPartKindV1::Leg,
                    count: 4,
                },
            ],
            ..BeginnerGenerationConstraintsV1::default()
        };
        assert_eq!(
            estimate_symmetric_parameters_v1(&constraints),
            Some(BeginnerSymmetricParameterEstimateV1 {
                protrusion_count: 4,
                scale_percent: 25,
                spacing_percent: 35,
            })
        );
        let mut ambiguous = constraints;
        ambiguous.target_parts[2].count = 3;
        assert_eq!(estimate_symmetric_parameters_v1(&ambiguous), None);

        let candidates = symmetric_parameter_candidates_v1(BeginnerSymmetricParameterEstimateV1 {
            protrusion_count: 4,
            scale_percent: 25,
            spacing_percent: 35,
        });
        assert_eq!(candidates.len(), 3);
        assert_eq!(candidates[0].approximation_score, 100);
        assert!(candidates[1].approximation_score < candidates[0].approximation_score);
        assert!(candidates[2].complexity_score > candidates[0].complexity_score);
    }

    fn skeleton(
        id: u16,
        start_x: i32,
        start_y: i32,
        end_x: i32,
        end_y: i32,
    ) -> BeginnerSkeletonSegmentV1 {
        BeginnerSkeletonSegmentV1 {
            id,
            start: crate::BeginnerSkeletonPointV1 {
                x_tenths_mm: start_x,
                y_tenths_mm: start_y,
            },
            end: crate::BeginnerSkeletonPointV1 {
                x_tenths_mm: end_x,
                y_tenths_mm: end_y,
            },
            thickness_tenths_mm: 10,
        }
    }

    fn bilateral_protrusion(id: u16, count: u8) -> crate::BeginnerProtrusionTargetV1 {
        crate::BeginnerProtrusionTargetV1 {
            id,
            count,
            length_tenths_mm: 5,
            thickness_tenths_mm: 2,
            root_width_tenths_mm: None,
            tip_width_tenths_mm: None,
            local_outline_tenths_mm: None,
            position_tenths_mm: [0, if count == 2 { 5 } else { 0 }, 0],
            direction_milli: if count == 2 {
                [1_000, 0, 0]
            } else {
                [0, 1_000, 0]
            },
            symmetry: BeginnerProtrusionSymmetryV1::Bilateral,
            curvature_degrees: 0,
            joint: crate::BeginnerProtrusionJointV1::Hinge,
            motion_degrees: [0, 45],
            side: crate::BeginnerProtrusionSideV1::Either,
            priority: 80,
        }
    }
}
