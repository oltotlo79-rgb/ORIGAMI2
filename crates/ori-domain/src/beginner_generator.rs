use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    BeginnerDetailLevelV1, BeginnerFoldTechniqueV1, BeginnerGenerationConstraintsV1,
    BeginnerProtrusionSymmetryV1, BeginnerProtrusionTargetV1, BeginnerSkeletonPointV1,
    BeginnerSkeletonSegmentV1, BeginnerTargetAssetReferenceV1, BeginnerTargetCategoryV1,
    BeginnerTargetPartKindV1, BeginnerTargetPartRecordV1, CreasePattern, Edge, EdgeId, EdgeKind,
    Point2, ProjectId, Vertex, VertexId,
};

mod extended_bilateral_endpoints;
mod radial_endpoints;

pub const BEGINNER_GENERATOR_SCHEMA_VERSION_V1: u32 = 1;
pub const MAX_BEGINNER_GENERATED_CANDIDATES_V1: usize = 3;
pub const MAX_BEGINNER_GENERATOR_INPUT_VERTICES_V1: usize = 10_000;
pub const BEGINNER_PARAMETER_GRID_SIZE_V1: usize = 27;
pub const MAX_BEGINNER_GENERIC_TREE_BARS_V1: usize = 16;
pub const MAX_BEGINNER_GENERIC_TREE_INTERSECTION_PAIRS_V1: usize = 120;
pub const MAX_BEGINNER_GENERIC_TREE_NODES_V1: usize = 17;
pub const MAX_BEGINNER_GENERIC_PROTRUSION_BINDINGS_V1: usize = 14;
pub const MAX_BEGINNER_GENERAL_PROTRUSION_COUNT_V1: u8 = 14;
const MAX_BEGINNER_GENERIC_PROTRUSION_ENDPOINTS_V1: usize = 32;

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
            generic_tree: None,
            reference_consensus: None,
            reference_consensus_summary: None,
            document_authority_sha256: None,
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
    if constraints.protrusions.len() != 5 {
        return None;
    }
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
        || constraints.protrusions.len() != 2
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

fn animal_standalone_horn_tail_ear_bindings_v1(
    constraints: &BeginnerGenerationConstraintsV1,
) -> Option<BeginnerHornTailEarBindingV1> {
    (constraints.protrusions.len() == 3)
        .then(|| animal_horn_tail_ear_bindings_v1(constraints))
        .flatten()
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
        || constraints.protrusions.len() != 2
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
        || constraints.protrusions.len() != 2
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
        || constraints.protrusions.len() != 2
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
        || constraints.protrusions.len() != 3
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BeginnerSemanticTemplateV1 {
    AnimalCompleteWinged,
    AnimalComplete,
    AnimalHornTailEar,
    AnimalTailEar,
    AnimalHornEar,
    AnimalHornTail,
    AnimalFourLeg,
    AnimalWingPair,
    AnimalAsymmetricFish,
    AnimalFinPair,
    AnimalEarPair,
    AnimalHornPair,
    AnimalTail,
    AnimalHorn,
    InsectComplete,
    InsectWingAntenna,
    InsectAsymmetricLandmarks,
    InsectWingPair,
    InsectFourWings,
    InsectAntennaPair,
    InsectAntenna,
    InsectLegPair,
    InsectSixLeg,
    General(u8),
}

impl BeginnerSemanticTemplateV1 {
    const fn protrusion_count(self) -> u8 {
        match self {
            Self::AnimalCompleteWinged | Self::InsectComplete => 10,
            Self::AnimalComplete => 8,
            Self::AnimalHornTailEar
            | Self::InsectWingAntenna
            | Self::AnimalFourLeg
            | Self::InsectFourWings => 4,
            Self::AnimalTailEar | Self::AnimalHornEar | Self::AnimalAsymmetricFish => 3,
            Self::AnimalHornTail
            | Self::AnimalWingPair
            | Self::AnimalFinPair
            | Self::AnimalEarPair
            | Self::AnimalHornPair
            | Self::InsectWingPair
            | Self::InsectAntennaPair
            | Self::InsectLegPair => 2,
            Self::AnimalTail | Self::AnimalHorn | Self::InsectAntenna => 1,
            Self::InsectAsymmetricLandmarks => 7,
            Self::InsectSixLeg => 6,
            Self::General(count) => count,
        }
    }
}

fn exact_target_part_signature_v1(
    constraints: &BeginnerGenerationConstraintsV1,
    category: BeginnerTargetCategoryV1,
    feature_parts: &[(BeginnerTargetPartKindV1, u8)],
) -> bool {
    if constraints.target_category != Some(category) {
        return false;
    }
    let expected_records = feature_parts.len()
        + if category == BeginnerTargetCategoryV1::CustomObject {
            0
        } else {
            2
        };
    if constraints.target_parts.len() != expected_records {
        return false;
    }
    let has_exactly_one = |kind, count| {
        constraints
            .target_parts
            .iter()
            .filter(|part| part.kind == kind && part.count == count)
            .count()
            == 1
    };
    (category == BeginnerTargetCategoryV1::CustomObject
        || has_exactly_one(BeginnerTargetPartKindV1::Head, 1)
            && has_exactly_one(BeginnerTargetPartKindV1::Torso, 1))
        && feature_parts
            .iter()
            .all(|(kind, count)| has_exactly_one(*kind, *count))
}

fn general_semantic_protrusion_count_v1(
    constraints: &BeginnerGenerationConstraintsV1,
) -> Option<u8> {
    let category = constraints.target_category?;
    let mut kinds = std::collections::HashSet::with_capacity(constraints.target_parts.len());
    if !constraints
        .target_parts
        .iter()
        .all(|part| kinds.insert(part.kind))
    {
        return None;
    }
    if category != BeginnerTargetCategoryV1::CustomObject
        && (!constraints
            .target_parts
            .iter()
            .any(|part| part.kind == BeginnerTargetPartKindV1::Head && part.count == 1)
            || !constraints
                .target_parts
                .iter()
                .any(|part| part.kind == BeginnerTargetPartKindV1::Torso && part.count == 1))
    {
        return None;
    }
    let feature_parts = constraints
        .target_parts
        .iter()
        .filter(|part| {
            !matches!(
                part.kind,
                BeginnerTargetPartKindV1::Head | BeginnerTargetPartKindV1::Torso
            )
        })
        .collect::<Vec<_>>();
    let feature_count =
        if category == BeginnerTargetCategoryV1::CustomObject && feature_parts.is_empty() {
            constraints
                .protrusions
                .iter()
                .try_fold(0_u8, |total, target| total.checked_add(target.count))?
        } else {
            feature_parts
                .into_iter()
                .try_fold(0_u8, |total, part| total.checked_add(part.count))?
        };
    (2..=MAX_BEGINNER_GENERAL_PROTRUSION_COUNT_V1)
        .contains(&feature_count)
        .then_some(feature_count)
}

fn bounded_generic_physical_endpoint_count_v1(
    constraints: &BeginnerGenerationConstraintsV1,
) -> Option<usize> {
    if !(1..=MAX_BEGINNER_GENERIC_PROTRUSION_BINDINGS_V1).contains(&constraints.protrusions.len())
        || constraints
            .protrusions
            .windows(2)
            .any(|pair| pair[0].id >= pair[1].id)
    {
        return None;
    }
    let endpoint_count = constraints
        .protrusions
        .iter()
        .try_fold(0_usize, |total, target| {
            (1..=8)
                .contains(&target.count)
                .then(|| total.checked_add(usize::from(target.count)))
                .flatten()
        })?;
    (endpoint_count <= MAX_BEGINNER_GENERIC_PROTRUSION_ENDPOINTS_V1).then_some(endpoint_count)
}

fn general_semantic_physical_counts_match_v1(
    constraints: &BeginnerGenerationConstraintsV1,
    semantic_count: u8,
) -> bool {
    bounded_generic_physical_endpoint_count_v1(constraints) == Some(usize::from(semantic_count))
}

fn semantic_template_v1(
    constraints: &BeginnerGenerationConstraintsV1,
) -> Option<BeginnerSemanticTemplateV1> {
    use BeginnerSemanticTemplateV1 as Template;
    use BeginnerTargetCategoryV1::{Animal, Insect};
    use BeginnerTargetPartKindV1::{Antenna, Ear, Fin, Horn, Leg, Tail, Wing};

    let exact = |category, parts: &[(BeginnerTargetPartKindV1, u8)]| {
        exact_target_part_signature_v1(constraints, category, parts)
    };
    let specialized = match constraints.target_category? {
        Animal
            if exact(
                Animal,
                &[(Horn, 1), (Tail, 1), (Ear, 2), (Leg, 4), (Wing, 2)],
            ) =>
        {
            Some(Template::AnimalCompleteWinged)
        }
        Animal if exact(Animal, &[(Horn, 1), (Tail, 1), (Ear, 2), (Leg, 4)]) => {
            Some(Template::AnimalComplete)
        }
        Animal if exact(Animal, &[(Horn, 1), (Tail, 1), (Ear, 2)]) => {
            Some(Template::AnimalHornTailEar)
        }
        Animal if exact(Animal, &[(Tail, 1), (Ear, 2)]) => Some(Template::AnimalTailEar),
        Animal if exact(Animal, &[(Horn, 1), (Ear, 2)]) => Some(Template::AnimalHornEar),
        Animal if exact(Animal, &[(Horn, 1), (Tail, 1)]) => Some(Template::AnimalHornTail),
        Animal if exact(Animal, &[(Leg, 4)]) => Some(Template::AnimalFourLeg),
        Animal if exact(Animal, &[(Wing, 2)]) => Some(Template::AnimalWingPair),
        Animal if exact(Animal, &[(Tail, 1), (Fin, 2)]) => Some(Template::AnimalAsymmetricFish),
        Animal if exact(Animal, &[(Fin, 2)]) => Some(Template::AnimalFinPair),
        Animal if exact(Animal, &[(Ear, 2)]) => Some(Template::AnimalEarPair),
        Animal if exact(Animal, &[(Horn, 2)]) => Some(Template::AnimalHornPair),
        Animal if exact(Animal, &[(Tail, 1)]) => Some(Template::AnimalTail),
        Animal if exact(Animal, &[(Horn, 1)]) => Some(Template::AnimalHorn),
        Insect if exact(Insect, &[(Wing, 2), (Antenna, 2), (Leg, 6)]) => {
            Some(Template::InsectComplete)
        }
        Insect if exact(Insect, &[(Wing, 2), (Antenna, 2)]) => Some(Template::InsectWingAntenna),
        Insect if exact(Insect, &[(Tail, 1), (Wing, 2), (Leg, 6)]) => {
            Some(Template::InsectAsymmetricLandmarks)
        }
        Insect if exact(Insect, &[(Wing, 4)]) => Some(Template::InsectFourWings),
        Insect if exact(Insect, &[(Wing, 2)]) => Some(Template::InsectWingPair),
        Insect if exact(Insect, &[(Antenna, 2)]) => Some(Template::InsectAntennaPair),
        Insect if exact(Insect, &[(Antenna, 1)]) => Some(Template::InsectAntenna),
        Insect if exact(Insect, &[(Leg, 2)]) => Some(Template::InsectLegPair),
        Insect if exact(Insect, &[(Leg, 6)]) => Some(Template::InsectSixLeg),
        _ => None,
    };
    specialized.or_else(|| general_semantic_protrusion_count_v1(constraints).map(Template::General))
}

/// Returns the plan family selected by the exact semantic target signature.
///
/// This is deliberately shared with native grid apply so extra or duplicate part
/// records cannot be ignored by one runtime while another runtime treats the
/// same profile as a generic target.
#[must_use]
pub fn beginner_expected_generated_plan_kind_v1(
    constraints: &BeginnerGenerationConstraintsV1,
) -> Option<BeginnerGeneratedPlanKindV1> {
    use BeginnerGeneratedPlanKindV1 as Kind;
    use BeginnerSemanticTemplateV1 as Template;

    if constraints.target_category == Some(BeginnerTargetCategoryV1::CustomObject) {
        return Some(Kind::CompositeGenericTargetBase);
    }
    match semantic_template_v1(constraints)? {
        Template::AnimalCompleteWinged => Some(Kind::CompositeCompleteWingedAnimalBase),
        Template::AnimalComplete => Some(Kind::CompositeCompleteAnimalBase),
        Template::AnimalHornTailEar => Some(Kind::CompositeHornTailEarBase),
        Template::AnimalTailEar => Some(Kind::CompositeTailEarBase),
        Template::AnimalHornEar => Some(Kind::CompositeHornEarBase),
        Template::AnimalHornTail => Some(Kind::CompositeHornTailBase),
        Template::AnimalFourLeg => Some(
            if exact_ordered_asymmetric_landmarks_v1(constraints, 4, 3) {
                Kind::AsymmetricFourLegLandmarkBase
            } else {
                Kind::SymmetricFourLegBase
            },
        ),
        Template::AnimalWingPair => Some(
            if exact_ordered_asymmetric_landmarks_v1(constraints, 2, 2) {
                Kind::AsymmetricBirdLandmarkBase
            } else {
                Kind::SymmetricBirdBase
            },
        ),
        Template::AnimalAsymmetricFish => Some(Kind::AsymmetricFishLandmarkBase),
        Template::AnimalFinPair => Some(Kind::SymmetricFishBase),
        Template::AnimalEarPair => Some(Kind::SymmetricEarBase),
        Template::AnimalHornPair => Some(Kind::SymmetricHornBase),
        Template::AnimalTail => Some(Kind::CenterAxisTailBase),
        Template::AnimalHorn => Some(Kind::CenterAxisHornBase),
        Template::InsectComplete => Some(Kind::CompositeCompleteInsectBase),
        Template::InsectWingAntenna => Some(Kind::CompositeWingAntennaBase),
        Template::InsectAsymmetricLandmarks => Some(Kind::AsymmetricInsectLandmarkBase),
        Template::InsectWingPair | Template::InsectFourWings => Some(Kind::SymmetricWingBase),
        Template::InsectAntennaPair => Some(Kind::SymmetricAntennaBase),
        Template::InsectAntenna => Some(Kind::CenterAxisAntennaBase),
        Template::InsectLegPair => Some(Kind::SymmetricInsectLegPairBase),
        Template::InsectSixLeg => Some(Kind::SymmetricSixLegBase),
        Template::General(_) => Some(Kind::CompositeGenericTargetBase),
    }
}

const fn beginner_plan_requires_mixed_kawasaki_techniques_v1(
    kind: BeginnerGeneratedPlanKindV1,
) -> bool {
    matches!(
        kind,
        BeginnerGeneratedPlanKindV1::AsymmetricBirdLandmarkBase
            | BeginnerGeneratedPlanKindV1::AsymmetricFourLegLandmarkBase
            | BeginnerGeneratedPlanKindV1::AsymmetricInsectLandmarkBase
            | BeginnerGeneratedPlanKindV1::AsymmetricFishLandmarkBase
    )
}

#[must_use]
pub fn estimate_symmetric_parameters_v1(
    constraints: &BeginnerGenerationConstraintsV1,
) -> Option<BeginnerSymmetricParameterEstimateV1> {
    let template = semantic_template_v1(constraints)?;
    let protrusion_count = template.protrusion_count();
    if matches!(template, BeginnerSemanticTemplateV1::General(_))
        && !general_semantic_physical_counts_match_v1(constraints, protrusion_count)
    {
        return None;
    }
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
    if constraints.allowed_techniques.len() > crate::MAX_BEGINNER_ALLOWED_TECHNIQUES_V1 {
        return Err(BeginnerGeneratorErrorV1::UnsupportedTechniques);
    }
    let allows_valley = constraints
        .allowed_techniques
        .contains(&BeginnerFoldTechniqueV1::ValleyFold);
    let allows_mountain = constraints
        .allowed_techniques
        .contains(&BeginnerFoldTechniqueV1::MountainFold);
    let mut unique_techniques =
        std::collections::HashSet::with_capacity(constraints.allowed_techniques.len());
    if !constraints
        .allowed_techniques
        .iter()
        .all(|technique| unique_techniques.insert(*technique))
        || (!allows_valley && !allows_mountain)
    {
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
    let semantic_template = semantic_template_v1(constraints);
    if target_category != BeginnerTargetCategoryV1::CustomObject
        && (part_count(BeginnerTargetPartKindV1::Head) != 1
            || part_count(BeginnerTargetPartKindV1::Torso) != 1)
    {
        return Err(BeginnerGeneratorErrorV1::MissingRequiredParts);
    }
    if !crate::validate_beginner_generation_constraints_v1(constraints)
        || !generic_body_outline_within_skeleton_bounds_v1(constraints)
    {
        return Err(match target_category {
            BeginnerTargetCategoryV1::Insect => BeginnerGeneratorErrorV1::UnsupportedInsectTemplate,
            BeginnerTargetCategoryV1::Animal | BeginnerTargetCategoryV1::CustomObject => {
                BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate
            }
        });
    }
    let kind = if allows_valley {
        EdgeKind::Valley
    } else {
        EdgeKind::Mountain
    };
    let mut template = match target_category {
        BeginnerTargetCategoryV1::CustomObject => {
            let tree_ratios = bounded_tree_skeleton_length_ratios(&constraints.skeleton_segments)
                .ok_or(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate)?;
            let endpoints = bounded_generic_composite_endpoints(constraints)
                .ok_or(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate)?;
            let instruction = format!(
                "bounded_tree_river_axial_v1:{}",
                tree_ratios
                    .iter()
                    .map(u32::to_string)
                    .collect::<Vec<_>>()
                    .join(",")
            );
            let plan = symmetric_template(
                namespace,
                source,
                BeginnerGeneratedPlanKindV1::CompositeGenericTargetBase,
                kind,
                min_x,
                max_x,
                min_y,
                max_y,
                &endpoints,
                &instruction,
                constraints,
            );
            append_bounded_radial_tree_graph(
                plan,
                constraints,
                namespace,
                min_x,
                max_x,
                min_y,
                max_y,
            )
            .ok_or(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate)?
        }
        BeginnerTargetCategoryV1::Animal => {
            let semantic_template =
                semantic_template.ok_or(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate)?;
            let asymmetric_landmark_fish = semantic_template
                == BeginnerSemanticTemplateV1::AnimalAsymmetricFish
                && constraints
                    .protrusions
                    .iter()
                    .filter(|target| {
                        target.count == 1 && target.symmetry == BeginnerProtrusionSymmetryV1::None
                    })
                    .count()
                    >= 3;
            if matches!(semantic_template, BeginnerSemanticTemplateV1::General(_)) {
                let tree_ratios =
                    bounded_tree_skeleton_length_ratios(&constraints.skeleton_segments)
                        .ok_or(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate)?;
                let endpoints = bounded_generic_composite_endpoints(constraints)
                    .ok_or(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate)?;
                let instruction = format!(
                    "bounded_tree_river_axial_v1:{}",
                    tree_ratios
                        .iter()
                        .map(u32::to_string)
                        .collect::<Vec<_>>()
                        .join(",")
                );
                let plan = symmetric_template(
                    namespace,
                    source,
                    BeginnerGeneratedPlanKindV1::CompositeGenericTargetBase,
                    kind,
                    min_x,
                    max_x,
                    min_y,
                    max_y,
                    &endpoints,
                    &instruction,
                    constraints,
                );
                append_bounded_radial_tree_graph(
                    plan,
                    constraints,
                    namespace,
                    min_x,
                    max_x,
                    min_y,
                    max_y,
                )
                .ok_or(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate)?
            } else if asymmetric_landmark_fish {
                if !exact_ordered_asymmetric_landmarks_v1(constraints, 3, 0) {
                    return Err(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate);
                }
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
            } else if semantic_template == BeginnerSemanticTemplateV1::AnimalAsymmetricFish {
                return Err(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate);
            } else if part_count(BeginnerTargetPartKindV1::Horn) == 1
                && part_count(BeginnerTargetPartKindV1::Tail) == 1
                && part_count(BeginnerTargetPartKindV1::Ear) == 2
            {
                let complete = animal_complete_bindings_v1(constraints);
                let winged_complete = animal_complete_winged_bindings_v1(constraints);
                let bindings = if complete.is_some() || winged_complete.is_some() {
                    animal_horn_tail_ear_bindings_v1(constraints)
                } else if part_count(BeginnerTargetPartKindV1::Leg) == 0 {
                    animal_standalone_horn_tail_ear_bindings_v1(constraints)
                } else {
                    None
                }
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
                let ears = if winged_complete.is_some() {
                    parameterized_symmetric_endpoints_for_target(
                        ear_only
                            .protrusions
                            .first()
                            .ok_or(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate)?,
                        &ear_only.skeleton_segments,
                        2,
                        false,
                    )
                    .map(|four| vec![four[1], four[3]])
                } else {
                    parameterized_symmetric_endpoints(&ear_only, 2, false).map(Vec::from)
                }
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
                        let wings = parameterized_symmetric_endpoints_for_target(
                            wing_only
                                .protrusions
                                .first()
                                .ok_or(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate)?,
                            &wing_only.skeleton_segments,
                            2,
                            false,
                        )
                        .map(|four| [four[0], four[2]])
                        .ok_or(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate)?;
                        endpoints.extend(wings);
                    }
                } else if part_count(BeginnerTargetPartKindV1::Leg) != 0 {
                    return Err(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate);
                }
                let radial_directions = endpoints
                    .iter()
                    .map(|(x, y)| (x - 0.5, y - 0.5))
                    .collect::<Vec<_>>();
                if radial_directions
                    .iter()
                    .any(|direction| direction.0 == 0.0 && direction.1 == 0.0)
                    || radial_directions.iter().enumerate().any(|(index, left)| {
                        radial_directions.iter().skip(index + 1).any(|right| {
                            let cross = left.0 * right.1 - left.1 * right.0;
                            let dot = left.0 * right.0 + left.1 * right.1;
                            cross.abs() <= f64::EPSILON && dot > 0.0
                        })
                    })
                {
                    // Distinct semantic targets must remain distinct after
                    // radial extension to the paper boundary. Collinear rays
                    // in the same direction would otherwise create different
                    // edge IDs over one geometric hinge.
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
                let endpoint = exact_single_center_axis_target_and_endpoint_v1(constraints, true)
                    .map(|(_, endpoint)| endpoint)
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
                let endpoint = exact_single_center_axis_target_and_endpoint_v1(constraints, false)
                    .map(|(_, endpoint)| endpoint)
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
                let endpoints = if asymmetric {
                    if !exact_ordered_asymmetric_landmarks_v1(
                        constraints,
                        usize::from(required_count),
                        if vertical { 3 } else { 2 },
                    ) {
                        return Err(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate);
                    }
                    vec![(1.0, 0.5), (0.25, 1.0), (0.25, 0.0), (0.75, 0.0)]
                } else {
                    exact_single_bilateral_template_target_and_endpoints_v1(
                        constraints,
                        required_count,
                        vertical,
                        if vertical { 3 } else { 2 },
                    )
                    .map(|(_, endpoints)| endpoints.to_vec())
                    .ok_or(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate)?
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
            let semantic_template =
                semantic_template.ok_or(BeginnerGeneratorErrorV1::UnsupportedInsectTemplate)?;
            let asymmetric_landmark_insect = semantic_template
                == BeginnerSemanticTemplateV1::InsectAsymmetricLandmarks
                && constraints
                    .protrusions
                    .iter()
                    .filter(|target| {
                        target.count == 1 && target.symmetry == BeginnerProtrusionSymmetryV1::None
                    })
                    .count()
                    >= 7;
            if matches!(semantic_template, BeginnerSemanticTemplateV1::General(_)) {
                let tree_ratios =
                    bounded_tree_skeleton_length_ratios(&constraints.skeleton_segments)
                        .ok_or(BeginnerGeneratorErrorV1::UnsupportedInsectTemplate)?;
                let endpoints = bounded_generic_composite_endpoints(constraints)
                    .ok_or(BeginnerGeneratorErrorV1::UnsupportedInsectTemplate)?;
                let instruction = format!(
                    "bounded_tree_river_axial_v1:{}",
                    tree_ratios
                        .iter()
                        .map(u32::to_string)
                        .collect::<Vec<_>>()
                        .join(",")
                );
                let plan = symmetric_template(
                    namespace,
                    source,
                    BeginnerGeneratedPlanKindV1::CompositeGenericTargetBase,
                    kind,
                    min_x,
                    max_x,
                    min_y,
                    max_y,
                    &endpoints,
                    &instruction,
                    constraints,
                );
                append_bounded_radial_tree_graph(
                    plan,
                    constraints,
                    namespace,
                    min_x,
                    max_x,
                    min_y,
                    max_y,
                )
                .ok_or(BeginnerGeneratorErrorV1::UnsupportedInsectTemplate)?
            } else if asymmetric_landmark_insect {
                if !exact_ordered_asymmetric_landmarks_v1(constraints, 7, 0) {
                    return Err(BeginnerGeneratorErrorV1::UnsupportedInsectTemplate);
                }
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
            } else if semantic_template == BeginnerSemanticTemplateV1::InsectAsymmetricLandmarks {
                return Err(BeginnerGeneratorErrorV1::UnsupportedInsectTemplate);
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
                let endpoint = exact_single_center_axis_target_and_endpoint_v1(constraints, true)
                    .map(|(_, endpoint)| endpoint)
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
                let wing_count = part_count(BeginnerTargetPartKindV1::Wing);
                let (required_count, plan_kind, instruction) = if matches!(wing_count, 2 | 4) {
                    (
                        wing_count,
                        BeginnerGeneratedPlanKindV1::SymmetricWingBase,
                        "symmetric_wing_base",
                    )
                } else if part_count(BeginnerTargetPartKindV1::Antenna) == 2 {
                    (
                        2,
                        BeginnerGeneratedPlanKindV1::SymmetricAntennaBase,
                        "symmetric_antenna_base",
                    )
                } else if part_count(BeginnerTargetPartKindV1::Leg) == 2 {
                    (
                        2,
                        BeginnerGeneratedPlanKindV1::SymmetricInsectLegPairBase,
                        "symmetric_insect_leg_pair_base",
                    )
                } else {
                    return Err(BeginnerGeneratorErrorV1::UnsupportedInsectTemplate);
                };
                let endpoints = exact_single_bilateral_template_target_and_endpoints_v1(
                    constraints,
                    required_count,
                    false,
                    2,
                )
                .map(|(_, endpoints)| endpoints)
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
    if beginner_plan_requires_mixed_kawasaki_techniques_v1(template.kind)
        && (!allows_valley || !allows_mountain)
    {
        return Err(BeginnerGeneratorErrorV1::UnsupportedTechniques);
    }
    if template.kind == BeginnerGeneratedPlanKindV1::CompositeGenericTargetBase {
        let error = match target_category {
            BeginnerTargetCategoryV1::Insect => BeginnerGeneratorErrorV1::UnsupportedInsectTemplate,
            BeginnerTargetCategoryV1::Animal | BeginnerTargetCategoryV1::CustomObject => {
                BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate
            }
        };
        template.skeleton_segments =
            canonical_bounded_tree_segments(&constraints.skeleton_segments).ok_or(error)?;
    }
    if target_category == BeginnerTargetCategoryV1::CustomObject {
        return Ok(vec![template]);
    }
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
        BeginnerTargetCategoryV1::CustomObject => unreachable!("returned above"),
    };
    let variant_skeleton_segments =
        if template.kind == BeginnerGeneratedPlanKindV1::CompositeGenericTargetBase {
            template.skeleton_segments.clone()
        } else {
            constraints.skeleton_segments.clone()
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
                    skeleton_segments: variant_skeleton_segments.clone(),
                    target_asset: constraints.target_asset,
                    semantic_landmark_provenance: None,
                }
            }),
    );
    Ok(plans)
}

fn protrusion_local_outline_within_bounds_v1(
    target: &BeginnerProtrusionTargetV1,
    bounds: (i32, i32, i32, i32),
) -> bool {
    let (minimum_x, maximum_x, minimum_y, maximum_y) = bounds;
    target
        .local_outline_tenths_mm
        .as_deref()
        .is_none_or(|outline| {
            outline.iter().all(|point| {
                target.position_tenths_mm[0]
                    .checked_add(point[0])
                    .is_some_and(|x| (minimum_x..=maximum_x).contains(&x))
                    && target.position_tenths_mm[1]
                        .checked_add(point[1])
                        .is_some_and(|y| (minimum_y..=maximum_y).contains(&y))
            })
        })
}

fn generic_body_outline_within_bounds_v1(
    outline: &[[i32; 2]],
    bounds: (i32, i32, i32, i32),
) -> bool {
    let (minimum_x, maximum_x, minimum_y, maximum_y) = bounds;
    maximum_x
        .checked_sub(minimum_x)
        .is_some_and(|span| span > 0)
        && maximum_y
            .checked_sub(minimum_y)
            .is_some_and(|span| span > 0)
        && outline.iter().all(|point| {
            (minimum_x..=maximum_x).contains(&point[0])
                && (minimum_y..=maximum_y).contains(&point[1])
        })
}

fn generic_body_outline_within_skeleton_bounds_v1(
    constraints: &BeginnerGenerationConstraintsV1,
) -> bool {
    let Some(outline) = constraints.generic_body_outline_tenths_mm.as_deref() else {
        return true;
    };
    skeleton_bounds(&constraints.skeleton_segments)
        .is_some_and(|bounds| generic_body_outline_within_bounds_v1(outline, bounds))
}

fn protrusion_local_outlines_within_skeleton_bounds_v1(
    constraints: &BeginnerGenerationConstraintsV1,
) -> bool {
    if constraints
        .protrusions
        .iter()
        .all(|target| target.local_outline_tenths_mm.is_none())
    {
        return true;
    }
    let Some(bounds) = skeleton_bounds(&constraints.skeleton_segments) else {
        return false;
    };
    constraints
        .protrusions
        .iter()
        .all(|target| protrusion_local_outline_within_bounds_v1(target, bounds))
}

fn exact_ordered_asymmetric_landmarks_v1(
    constraints: &BeginnerGenerationConstraintsV1,
    required_count: usize,
    minimum_skeleton_segments: usize,
) -> bool {
    constraints.skeleton_segments.len() >= minimum_skeleton_segments
        && constraints.protrusions.len() == required_count
        && constraints
            .protrusions
            .windows(2)
            .all(|pair| pair[0].id < pair[1].id)
        && constraints.protrusions.iter().all(|target| {
            target.count == 1
                && target.symmetry == BeginnerProtrusionSymmetryV1::None
                && target.direction_milli != [0, 0, 0]
        })
        && protrusion_local_outlines_within_skeleton_bounds_v1(constraints)
}

fn parameterized_center_axis_endpoint(
    constraints: &BeginnerGenerationConstraintsV1,
    vertical: bool,
) -> Option<(f64, f64)> {
    let target = constraints.protrusions.iter().find(|target| {
        target.count == 1 && target.symmetry == BeginnerProtrusionSymmetryV1::None
    })?;
    parameterized_center_axis_endpoint_for_target(target, &constraints.skeleton_segments, vertical)
}

fn parameterized_center_axis_endpoint_for_target(
    target: &BeginnerProtrusionTargetV1,
    skeleton_segments: &[BeginnerSkeletonSegmentV1],
    vertical: bool,
) -> Option<(f64, f64)> {
    if target.count != 1 || target.symmetry != BeginnerProtrusionSymmetryV1::None {
        return None;
    }
    let bounds = skeleton_bounds(skeleton_segments)?;
    if !protrusion_local_outline_within_bounds_v1(target, bounds) {
        return None;
    }
    let (minimum_x, maximum_x, minimum_y, maximum_y) = bounds;
    let span_x = maximum_x.checked_sub(minimum_x)?;
    let span_y = maximum_y.checked_sub(minimum_y)?;
    if span_x <= 0
        || span_y <= 0
        || target.position_tenths_mm[0].checked_mul(2)? != minimum_x.checked_add(maximum_x)?
        || !(minimum_y..=maximum_y).contains(&target.position_tenths_mm[1])
    {
        return None;
    }
    let primary_span = if vertical { span_y } else { span_x };
    let primary_direction = if vertical {
        target.direction_milli[1]
    } else {
        target.direction_milli[0]
    };
    let length_ratio =
        f64::from(target.length_tenths_mm) / f64::from(u32::try_from(primary_span).ok()?);
    if !(0.02..=0.45).contains(&length_ratio) || primary_direction == 0 {
        return None;
    }
    let center_y = f64::from(target.position_tenths_mm[1].checked_sub(minimum_y)?)
        / f64::from(u32::try_from(span_y).ok()?);
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

fn exact_single_center_axis_target_and_endpoint_v1(
    constraints: &BeginnerGenerationConstraintsV1,
    vertical: bool,
) -> Option<(&BeginnerProtrusionTargetV1, (f64, f64))> {
    let [target] = constraints.protrusions.as_slice() else {
        return None;
    };
    let endpoint = parameterized_center_axis_endpoint_for_target(
        target,
        &constraints.skeleton_segments,
        vertical,
    )?;
    Some((target, endpoint))
}

fn exact_single_bilateral_template_target_and_endpoints_v1(
    constraints: &BeginnerGenerationConstraintsV1,
    count: u8,
    vertical: bool,
    minimum_skeleton_segments: usize,
) -> Option<(&BeginnerProtrusionTargetV1, [(f64, f64); 4])> {
    if constraints.skeleton_segments.len() < minimum_skeleton_segments
        || !has_bilateral_skeleton(constraints)
    {
        return None;
    }
    let [target] = constraints.protrusions.as_slice() else {
        return None;
    };
    let endpoints = parameterized_symmetric_endpoints_for_target(
        target,
        &constraints.skeleton_segments,
        count,
        vertical,
    )?;
    Some((target, endpoints))
}

#[must_use]
pub fn beginner_uses_bounded_generic_target_base_v1(
    constraints: &BeginnerGenerationConstraintsV1,
) -> bool {
    match constraints.target_category {
        Some(BeginnerTargetCategoryV1::CustomObject) => true,
        Some(BeginnerTargetCategoryV1::Animal | BeginnerTargetCategoryV1::Insect) => matches!(
            semantic_template_v1(constraints),
            Some(BeginnerSemanticTemplateV1::General(_))
        ),
        None => false,
    }
}

fn target_by_id_v1(
    constraints: &BeginnerGenerationConstraintsV1,
    id: u16,
) -> Option<&BeginnerProtrusionTargetV1> {
    constraints
        .protrusions
        .iter()
        .find(|target| target.id == id)
}

fn validated_isolated_center_axis_target_v1(
    constraints: &BeginnerGenerationConstraintsV1,
    id: u16,
    vertical: bool,
) -> Option<&BeginnerProtrusionTargetV1> {
    let target = target_by_id_v1(constraints, id)?;
    let mut isolated = constraints.clone();
    isolated.protrusions.retain(|candidate| candidate.id == id);
    parameterized_center_axis_endpoint(&isolated, vertical).map(|_| target)
}

fn validated_isolated_symmetric_target_v1(
    constraints: &BeginnerGenerationConstraintsV1,
    id: u16,
    count: u8,
    vertical: bool,
) -> Option<&BeginnerProtrusionTargetV1> {
    let target = target_by_id_v1(constraints, id)?;
    let mut isolated = constraints.clone();
    isolated.protrusions.retain(|candidate| candidate.id == id);
    parameterized_symmetric_endpoints(&isolated, count, vertical).map(|_| target)
}

fn highest_priority_target_v1(
    constraints: &BeginnerGenerationConstraintsV1,
) -> Option<&BeginnerProtrusionTargetV1> {
    constraints
        .protrusions
        .iter()
        .max_by_key(|target| (target.priority, std::cmp::Reverse(target.id)))
}

fn animal_target_approximation_score_target_v1(
    constraints: &BeginnerGenerationConstraintsV1,
) -> Option<&BeginnerProtrusionTargetV1> {
    let part_count = |kind| {
        constraints
            .target_parts
            .iter()
            .find(|part| part.kind == kind)
            .map_or(0, |part| part.count)
    };
    let single_landmarks = constraints
        .protrusions
        .iter()
        .filter(|target| target.count == 1 && target.symmetry == BeginnerProtrusionSymmetryV1::None)
        .count();
    let asymmetric_landmark_fish = part_count(BeginnerTargetPartKindV1::Tail) == 1
        && part_count(BeginnerTargetPartKindV1::Fin) == 2
        && single_landmarks >= 3;
    let asymmetric_count =
        if part_count(BeginnerTargetPartKindV1::Leg) == 4 && single_landmarks == 4 {
            Some((4_u8, 3_usize))
        } else if part_count(BeginnerTargetPartKindV1::Wing) == 2 && single_landmarks == 2 {
            Some((2, 2))
        } else {
            None
        };
    if asymmetric_landmark_fish {
        return exact_ordered_asymmetric_landmarks_v1(constraints, 3, 0)
            .then(|| highest_priority_target_v1(constraints))
            .flatten();
    }

    let horn = part_count(BeginnerTargetPartKindV1::Horn) == 1;
    let tail = part_count(BeginnerTargetPartKindV1::Tail) == 1;
    let ears = part_count(BeginnerTargetPartKindV1::Ear) == 2;
    if horn && tail && ears {
        let complete = animal_complete_bindings_v1(constraints);
        let winged_complete = animal_complete_winged_bindings_v1(constraints);
        let bindings = if complete.is_some() || winged_complete.is_some() {
            animal_horn_tail_ear_bindings_v1(constraints)
        } else if part_count(BeginnerTargetPartKindV1::Leg) == 0 {
            animal_standalone_horn_tail_ear_bindings_v1(constraints)
        } else {
            None
        }?;
        let horn_target = validated_isolated_center_axis_target_v1(
            constraints,
            bindings.horn_protrusion_id,
            true,
        )?;
        validated_isolated_center_axis_target_v1(constraints, bindings.tail_protrusion_id, false)?;
        validated_isolated_symmetric_target_v1(
            constraints,
            bindings.ear_pair_protrusion_id,
            2,
            false,
        )?;

        if let Some(binding) = complete.or(winged_complete.map(|winged| winged.animal)) {
            validated_isolated_symmetric_target_v1(
                constraints,
                binding.leg_protrusion_id,
                4,
                true,
            )?;
            if let Some(winged) = winged_complete {
                validated_isolated_symmetric_target_v1(
                    constraints,
                    winged.wing_pair_protrusion_id,
                    2,
                    false,
                )?;
            }
        } else if part_count(BeginnerTargetPartKindV1::Leg) != 0 {
            return None;
        }
        return Some(horn_target);
    }
    if horn && tail {
        let bindings = animal_horn_tail_bindings_v1(constraints)?;
        let horn_target = validated_isolated_center_axis_target_v1(
            constraints,
            bindings.horn_protrusion_id,
            true,
        )?;
        validated_isolated_center_axis_target_v1(constraints, bindings.tail_protrusion_id, false)?;
        return Some(horn_target);
    }
    if horn && ears {
        let bindings = animal_horn_ear_bindings_v1(constraints)?;
        let horn_target = validated_isolated_center_axis_target_v1(
            constraints,
            bindings.horn_protrusion_id,
            true,
        )?;
        validated_isolated_symmetric_target_v1(
            constraints,
            bindings.ear_pair_protrusion_id,
            2,
            false,
        )?;
        return Some(horn_target);
    }
    if tail && ears {
        let bindings = animal_tail_ear_bindings_v1(constraints)?;
        let tail_target = validated_isolated_center_axis_target_v1(
            constraints,
            bindings.tail_protrusion_id,
            false,
        )?;
        validated_isolated_symmetric_target_v1(
            constraints,
            bindings.ear_pair_protrusion_id,
            2,
            false,
        )?;
        return Some(tail_target);
    }
    if horn {
        return exact_single_center_axis_target_and_endpoint_v1(constraints, true)
            .map(|(target, _)| target);
    }
    if tail {
        return exact_single_center_axis_target_and_endpoint_v1(constraints, false)
            .map(|(target, _)| target);
    }
    if let Some((required_count, minimum_skeleton_segments)) = asymmetric_count {
        return exact_ordered_asymmetric_landmarks_v1(
            constraints,
            usize::from(required_count),
            minimum_skeleton_segments,
        )
        .then(|| highest_priority_target_v1(constraints))
        .flatten();
    }
    let (count, vertical, minimum_skeleton_segments) =
        if part_count(BeginnerTargetPartKindV1::Leg) == 4 {
            (4, true, 3)
        } else if [
            BeginnerTargetPartKindV1::Wing,
            BeginnerTargetPartKindV1::Fin,
            BeginnerTargetPartKindV1::Ear,
            BeginnerTargetPartKindV1::Horn,
        ]
        .into_iter()
        .any(|kind| part_count(kind) == 2)
        {
            (2, false, 2)
        } else {
            return None;
        };
    exact_single_bilateral_template_target_and_endpoints_v1(
        constraints,
        count,
        vertical,
        minimum_skeleton_segments,
    )
    .map(|(target, _)| target)
}

#[must_use]
pub fn beginner_target_approximation_score_v1(constraints: &BeginnerGenerationConstraintsV1) -> u8 {
    if !crate::validate_beginner_generation_constraints_v1(constraints) {
        return 0;
    }
    let part_count = |kind| {
        constraints
            .target_parts
            .iter()
            .find(|part| part.kind == kind)
            .map_or(0, |part| part.count)
    };
    if matches!(
        constraints.target_category,
        Some(BeginnerTargetCategoryV1::Animal | BeginnerTargetCategoryV1::Insect)
    ) && (part_count(BeginnerTargetPartKindV1::Head) != 1
        || part_count(BeginnerTargetPartKindV1::Torso) != 1)
    {
        return 0;
    }
    if matches!(
        constraints.target_category,
        Some(BeginnerTargetCategoryV1::Animal | BeginnerTargetCategoryV1::Insect)
    ) && semantic_template_v1(constraints).is_none()
    {
        return 0;
    }
    if !constraints
        .allowed_techniques
        .contains(&BeginnerFoldTechniqueV1::ValleyFold)
        && !constraints
            .allowed_techniques
            .contains(&BeginnerFoldTechniqueV1::MountainFold)
    {
        return 0;
    }
    if beginner_expected_generated_plan_kind_v1(constraints)
        .is_some_and(beginner_plan_requires_mixed_kawasaki_techniques_v1)
        && (!constraints
            .allowed_techniques
            .contains(&BeginnerFoldTechniqueV1::ValleyFold)
            || !constraints
                .allowed_techniques
                .contains(&BeginnerFoldTechniqueV1::MountainFold))
    {
        return 0;
    }
    if !generic_body_outline_within_skeleton_bounds_v1(constraints) {
        return 0;
    }
    let uses_generic_target = beginner_uses_bounded_generic_target_base_v1(constraints);
    let generic_target = if uses_generic_target {
        bounded_generic_composite_endpoints(constraints)
            .and_then(|_| bounded_generic_tree_graph_is_supported_v1(constraints).then_some(()))
            .and_then(|()| {
                constraints
                    .protrusions
                    .iter()
                    .max_by_key(|target| (target.priority, std::cmp::Reverse(target.id)))
            })
    } else {
        None
    };
    if uses_generic_target && generic_target.is_none() {
        return 0;
    }
    let target = if uses_generic_target {
        generic_target
    } else {
        match constraints.target_category {
            Some(BeginnerTargetCategoryV1::Animal) => {
                animal_target_approximation_score_target_v1(constraints)
            }
            Some(BeginnerTargetCategoryV1::Insect) => {
                let part_count = |kind| {
                    constraints
                        .target_parts
                        .iter()
                        .find(|part| part.kind == kind)
                        .map_or(0, |part| part.count)
                };
                let asymmetric_landmark_insect = part_count(BeginnerTargetPartKindV1::Tail) == 1
                    && part_count(BeginnerTargetPartKindV1::Wing) == 2
                    && part_count(BeginnerTargetPartKindV1::Leg) == 6
                    && constraints
                        .protrusions
                        .iter()
                        .filter(|target| {
                            target.count == 1
                                && target.symmetry == BeginnerProtrusionSymmetryV1::None
                        })
                        .count()
                        >= 7;
                let declared_complete = part_count(BeginnerTargetPartKindV1::Wing) == 2
                    && part_count(BeginnerTargetPartKindV1::Antenna) == 2
                    && part_count(BeginnerTargetPartKindV1::Leg) == 6;
                let declared_wing_antenna = part_count(BeginnerTargetPartKindV1::Wing) == 2
                    && part_count(BeginnerTargetPartKindV1::Antenna) == 2;
                if asymmetric_landmark_insect {
                    exact_ordered_asymmetric_landmarks_v1(constraints, 7, 0)
                        .then(|| {
                            constraints.protrusions.iter().max_by_key(|target| {
                                (target.priority, std::cmp::Reverse(target.id))
                            })
                        })
                        .flatten()
                } else if declared_complete {
                    insect_complete_bindings_v1(constraints).and_then(|bindings| {
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
                    })
                } else if declared_wing_antenna {
                    insect_wing_antenna_bindings_v1(constraints).and_then(|bindings| {
                        let ordered = [
                            (bindings.wing_pair_protrusion_id, false),
                            (bindings.antenna_pair_protrusion_id, true),
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
                    })
                } else if constraints
                    .target_parts
                    .iter()
                    .any(|part| part.kind == BeginnerTargetPartKindV1::Antenna && part.count == 1)
                {
                    exact_single_center_axis_target_and_endpoint_v1(constraints, true)
                        .map(|(target, _)| target)
                } else if part_count(BeginnerTargetPartKindV1::Leg) == 6 {
                    insect_three_pair_bindings_v1(constraints).and_then(|bindings| {
                        let target = validated_isolated_symmetric_target_v1(
                            constraints,
                            bindings[0].protrusion_id,
                            2,
                            false,
                        )?;
                        bindings[1..]
                            .iter()
                            .all(|binding| {
                                validated_isolated_symmetric_target_v1(
                                    constraints,
                                    binding.protrusion_id,
                                    2,
                                    false,
                                )
                                .is_some()
                            })
                            .then_some(target)
                    })
                } else {
                    let supported_count = if part_count(BeginnerTargetPartKindV1::Wing) == 4 {
                        Some(4)
                    } else {
                        [
                            BeginnerTargetPartKindV1::Wing,
                            BeginnerTargetPartKindV1::Antenna,
                            BeginnerTargetPartKindV1::Leg,
                        ]
                        .into_iter()
                        .any(|kind| part_count(kind) == 2)
                        .then_some(2)
                    };
                    supported_count.and_then(|count| {
                        exact_single_bilateral_template_target_and_endpoints_v1(
                            constraints,
                            count,
                            false,
                            2,
                        )
                        .map(|(target, _)| target)
                    })
                }
            }
            Some(BeginnerTargetCategoryV1::CustomObject) => None,
            None => None,
        }
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
    if base == 0 {
        return 0;
    }
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
    let surface_bulge_bonus = u8::try_from(
        constraints
            .bulge_targets
            .iter()
            .filter(|target| target.reference_surface_binding.is_some())
            .count()
            .min(5),
    )
    .unwrap_or(5);
    base.saturating_add(contour_bonus)
        .saturating_add(surface_bulge_bonus)
        .min(100)
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

fn try_endpoint_candidates_v1<const COUNT: usize>(
    candidates: [(f64, f64); COUNT],
) -> Option<Vec<(f64, f64)>> {
    let mut result = Vec::new();
    result.try_reserve_exact(COUNT).ok()?;
    result.extend(candidates);
    Some(result)
}

fn parameterized_exact_bilateral_pair_endpoints_v1(
    target: &BeginnerProtrusionTargetV1,
    skeleton_segments: &[BeginnerSkeletonSegmentV1],
    vertical: bool,
) -> Option<[(f64, f64); 2]> {
    let four =
        parameterized_symmetric_endpoints_for_target(target, skeleton_segments, 2, vertical)?;
    // A bilateral direction denotes an unoriented axis: reversing its sign
    // must preserve the same canonical pair. Collapse the four-corner width
    // envelope onto that axis instead of selecting one biased side.
    let pair = if vertical {
        [
            ((four[0].0 + four[1].0) / 2.0, four[0].1),
            ((four[2].0 + four[3].0) / 2.0, four[2].1),
        ]
    } else {
        [
            (four[0].0, (four[0].1 + four[1].1) / 2.0),
            (four[2].0, (four[2].1 + four[3].1) / 2.0),
        ]
    };
    pair.iter()
        .all(|(x, y)| (0.0..1.0).contains(x) && (0.0..1.0).contains(y))
        .then_some(pair)
}

fn bounded_generic_composite_endpoints(
    constraints: &BeginnerGenerationConstraintsV1,
) -> Option<Vec<(f64, f64)>> {
    bounded_tree_skeleton_length_ratios(&constraints.skeleton_segments)?;
    let physical_endpoint_count = bounded_generic_physical_endpoint_count_v1(constraints)?;
    let mut part_kinds = std::collections::HashSet::with_capacity(constraints.target_parts.len());
    if !constraints
        .target_parts
        .iter()
        .all(|part| part_kinds.insert(part.kind))
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
    if constraints.target_category == Some(BeginnerTargetCategoryV1::CustomObject) {
        if feature_records != 0 && feature_records != physical_endpoint_count {
            return None;
        }
    } else if feature_records != physical_endpoint_count {
        return None;
    }
    let (minimum_x, maximum_x, minimum_y, maximum_y) =
        skeleton_bounds(&constraints.skeleton_segments)?;
    let skeleton_body = [
        u32::try_from(maximum_x.checked_sub(minimum_x)?).ok()?,
        u32::try_from(maximum_y.checked_sub(minimum_y)?).ok()?,
    ];
    let available_body = if let Some(outline) = &constraints.generic_body_outline_tenths_mm {
        if !generic_body_outline_within_bounds_v1(
            outline,
            (minimum_x, maximum_x, minimum_y, maximum_y),
        ) {
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
    let mut endpoints = Vec::new();
    endpoints
        .try_reserve_exact(MAX_BEGINNER_GENERIC_PROTRUSION_ENDPOINTS_V1)
        .ok()?;
    for target in &constraints.protrusions {
        let root_width = target
            .root_width_tenths_mm
            .unwrap_or(u32::from(target.thickness_tenths_mm));
        let tip_width = target.tip_width_tenths_mm.unwrap_or(root_width);
        if tip_width == 0 || tip_width > root_width || root_width > body[0].min(body[1]) {
            return None;
        }
        if !protrusion_local_outline_within_bounds_v1(
            target,
            (minimum_x, maximum_x, minimum_y, maximum_y),
        ) {
            return None;
        }
        let candidates = match (target.count, target.symmetry) {
            (1, BeginnerProtrusionSymmetryV1::None) => {
                try_endpoint_candidates_v1([parameterized_center_axis_endpoint_for_target(
                    target,
                    &constraints.skeleton_segments,
                    target.direction_milli[1].unsigned_abs()
                        >= target.direction_milli[0].unsigned_abs(),
                )
                .or_else(|| {
                    parameterized_landmark_endpoint_for_target(
                        target,
                        &constraints.skeleton_segments,
                    )
                })?])?
            }
            (2, BeginnerProtrusionSymmetryV1::Bilateral) => {
                let vertical = target.direction_milli[1].unsigned_abs()
                    > target.direction_milli[0].unsigned_abs();
                try_endpoint_candidates_v1(parameterized_exact_bilateral_pair_endpoints_v1(
                    target,
                    &constraints.skeleton_segments,
                    vertical,
                )?)?
            }
            (4, BeginnerProtrusionSymmetryV1::Bilateral) => {
                let vertical = target.direction_milli[1].unsigned_abs()
                    > target.direction_milli[0].unsigned_abs();
                try_endpoint_candidates_v1(parameterized_symmetric_endpoints_for_target(
                    target,
                    &constraints.skeleton_segments,
                    target.count,
                    vertical,
                )?)?
            }
            (6 | 8, BeginnerProtrusionSymmetryV1::Bilateral) => {
                extended_bilateral_endpoints::parameterized_extended_bilateral_endpoints_v1(
                    target,
                    &constraints.skeleton_segments,
                )?
            }
            (2..=8, BeginnerProtrusionSymmetryV1::Radial) => {
                radial_endpoints::parameterized_radial_endpoints_v1(
                    target,
                    &constraints.skeleton_segments,
                )?
            }
            _ => return None,
        };
        if candidates.len() != usize::from(target.count)
            || endpoints
                .len()
                .checked_add(candidates.len())
                .is_none_or(|count| count > MAX_BEGINNER_GENERIC_PROTRUSION_ENDPOINTS_V1)
            || candidates.iter().enumerate().any(|(index, candidate)| {
                endpoints
                    .iter()
                    .chain(&candidates[..index])
                    .any(|existing: &(f64, f64)| {
                        (existing.0 - candidate.0).abs() < f64::EPSILON
                            && (existing.1 - candidate.1).abs() < f64::EPSILON
                    })
            })
        {
            return None;
        }
        endpoints.extend(candidates);
    }
    Some(endpoints)
}

fn bounded_generic_tree_graph_is_supported_v1(
    constraints: &BeginnerGenerationConstraintsV1,
) -> bool {
    let empty_plan = BeginnerGeneratedPlanV1 {
        schema_version: BEGINNER_GENERATOR_SCHEMA_VERSION_V1,
        kind: BeginnerGeneratedPlanKindV1::CompositeGenericTargetBase,
        crease_pattern: CreasePattern {
            vertices: Vec::new(),
            edges: Vec::new(),
        },
        instruction_codes: Vec::new(),
        target_parts: Vec::new(),
        skeleton_segments: Vec::new(),
        target_asset: None,
        semantic_landmark_provenance: None,
    };
    append_bounded_radial_tree_graph(
        empty_plan,
        constraints,
        ProjectId::schema_namespace([0x67; 16]),
        0.0,
        1.0,
        0.0,
        1.0,
    )
    .is_some()
}

fn bounded_tree_segments_intersect_beyond_shared_endpoint_v1(
    left: &BeginnerSkeletonSegmentV1,
    right: &BeginnerSkeletonSegmentV1,
) -> bool {
    let point = |value: BeginnerSkeletonPointV1| (value.x_tenths_mm, value.y_tenths_mm);
    let a = point(left.start);
    let b = point(left.end);
    let c = point(right.start);
    let d = point(right.end);
    let orient = |first: (i32, i32), second: (i32, i32), third: (i32, i32)| {
        (i128::from(second.0) - i128::from(first.0)) * (i128::from(third.1) - i128::from(first.1))
            - (i128::from(second.1) - i128::from(first.1))
                * (i128::from(third.0) - i128::from(first.0))
    };
    let on_closed_segment = |first: (i32, i32), second: (i32, i32), value: (i32, i32)| {
        orient(first, second, value) == 0
            && (first.0.min(second.0)..=first.0.max(second.0)).contains(&value.0)
            && (first.1.min(second.1)..=first.1.max(second.1)).contains(&value.1)
    };

    let first_shared = (a == c || a == d).then_some(a);
    let second_shared = (b == c || b == d).then_some(b);
    if first_shared.is_some() && second_shared.is_some() {
        return true;
    }
    if let Some(shared) = first_shared.or(second_shared) {
        let left_other = if a == shared { b } else { a };
        let right_other = if c == shared { d } else { c };
        if orient(shared, left_other, right_other) != 0 {
            return false;
        }
        let left_vector = (
            i128::from(left_other.0) - i128::from(shared.0),
            i128::from(left_other.1) - i128::from(shared.1),
        );
        let right_vector = (
            i128::from(right_other.0) - i128::from(shared.0),
            i128::from(right_other.1) - i128::from(shared.1),
        );
        return left_vector.0 * right_vector.0 + left_vector.1 * right_vector.1 >= 0;
    }

    let orientations = [
        orient(a, b, c),
        orient(a, b, d),
        orient(c, d, a),
        orient(c, d, b),
    ];
    (orientations[0] == 0 && on_closed_segment(a, b, c))
        || (orientations[1] == 0 && on_closed_segment(a, b, d))
        || (orientations[2] == 0 && on_closed_segment(c, d, a))
        || (orientations[3] == 0 && on_closed_segment(c, d, b))
        || ((orientations[0] < 0 && orientations[1] > 0
            || orientations[0] > 0 && orientations[1] < 0)
            && (orientations[2] < 0 && orientations[3] > 0
                || orientations[2] > 0 && orientations[3] < 0))
}

#[derive(Debug, Clone, Copy)]
struct BeginnerGenericAuxiliaryLayoutV1 {
    inner_min_x: f64,
    inner_max_x: f64,
    inner_min_y: f64,
    inner_max_y: f64,
    block_count: usize,
}

impl BeginnerGenericAuxiliaryLayoutV1 {
    fn map(self, block_index: usize, x_ratio: f64, y_ratio: f64) -> Option<Point2> {
        if block_index >= self.block_count
            || !x_ratio.is_finite()
            || !y_ratio.is_finite()
            || !(0.0..=1.0).contains(&x_ratio)
            || !(0.0..=1.0).contains(&y_ratio)
        {
            return None;
        }
        let width = self.inner_max_x - self.inner_min_x;
        let height = self.inner_max_y - self.inner_min_y;
        if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
            return None;
        }
        let block_width = width / self.block_count as f64;
        let block_min_x = self.inner_min_x + block_width * (block_index as f64 + 1.0 / 8.0);
        let block_max_x = self.inner_min_x + block_width * (block_index as f64 + 7.0 / 8.0);
        let block_min_y = self.inner_min_y + height / 8.0;
        let block_max_y = self.inner_min_y + height * 7.0 / 8.0;
        let position = Point2::new(
            block_min_x + (block_max_x - block_min_x) * x_ratio,
            block_min_y + (block_max_y - block_min_y) * y_ratio,
        );
        (position.x.is_finite() && position.y.is_finite()).then_some(position)
    }
}

fn beginner_generic_auxiliary_block_count_v1(
    constraints: &BeginnerGenerationConstraintsV1,
) -> Option<usize> {
    // The bounded tree is always the final block. Keeping every contour in a
    // distinct preceding block makes the full auxiliary graph disjoint while
    // preserving each contour's independently normalized shape.
    1_usize
        .checked_add(usize::from(
            constraints.generic_body_outline_tenths_mm.is_some(),
        ))?
        .checked_add(
            constraints
                .protrusions
                .iter()
                .filter(|target| target.local_outline_tenths_mm.is_some())
                .count(),
        )
}

fn beginner_generic_radial_center_v1(
    pattern: &CreasePattern,
    min_x: f64,
    max_x: f64,
    min_y: f64,
    max_y: f64,
) -> Option<VertexId> {
    let physical_edges = pattern
        .edges
        .iter()
        .filter(|edge| matches!(edge.kind, EdgeKind::Mountain | EdgeKind::Valley))
        .collect::<Vec<_>>();
    if !(1..=MAX_BEGINNER_GENERIC_PROTRUSION_ENDPOINTS_V1 + 5).contains(&physical_edges.len()) {
        return None;
    }
    let position_for = |id| {
        pattern
            .vertices
            .iter()
            .find(|vertex| vertex.id == id)
            .map(|vertex| vertex.position)
    };
    let first = physical_edges.first()?;
    let mut center_candidates = [first.start, first.end].into_iter().filter(|candidate| {
        physical_edges
            .iter()
            .all(|edge| edge.start == *candidate || edge.end == *candidate)
            && position_for(*candidate).is_some_and(|position| {
                position.x.is_finite()
                    && position.y.is_finite()
                    && (min_x..max_x).contains(&position.x)
                    && (min_y..max_y).contains(&position.y)
            })
    });
    let center_id = center_candidates.next()?;
    if center_candidates.next().is_some() {
        return None;
    }
    let center = position_for(center_id)?;
    if !center.x.is_finite()
        || !center.y.is_finite()
        || !(min_x..max_x).contains(&center.x)
        || !(min_y..max_y).contains(&center.y)
    {
        return None;
    }
    let mut endpoints = Vec::with_capacity(physical_edges.len());
    for edge in physical_edges {
        let endpoint_id = if edge.start == center_id {
            edge.end
        } else if edge.end == center_id {
            edge.start
        } else {
            return None;
        };
        let endpoint = position_for(endpoint_id)?;
        if !endpoint.x.is_finite()
            || !endpoint.y.is_finite()
            || endpoint == center
            || !((endpoint.x == min_x || endpoint.x == max_x)
                && (min_y..=max_y).contains(&endpoint.y)
                || (endpoint.y == min_y || endpoint.y == max_y)
                    && (min_x..=max_x).contains(&endpoint.x))
            || endpoints.contains(&endpoint)
        {
            return None;
        }
        endpoints.push(endpoint);
    }
    Some(center_id)
}

fn beginner_generic_auxiliary_layout_v1(
    pattern: &CreasePattern,
    center_id: VertexId,
    min_x: f64,
    max_x: f64,
    min_y: f64,
    max_y: f64,
    block_count: usize,
) -> Option<BeginnerGenericAuxiliaryLayoutV1> {
    if block_count == 0 || max_x <= min_x || max_y <= min_y {
        return None;
    }
    let center = pattern
        .vertices
        .iter()
        .find(|vertex| vertex.id == center_id)?
        .position;
    let position_for = |id| {
        pattern
            .vertices
            .iter()
            .find(|vertex| vertex.id == id)
            .map(|vertex| vertex.position)
    };
    let mut bottom_ray_x = pattern
        .edges
        .iter()
        .filter(|edge| matches!(edge.kind, EdgeKind::Mountain | EdgeKind::Valley))
        .filter_map(|edge| {
            let endpoint_id = if edge.start == center_id {
                edge.end
            } else if edge.end == center_id {
                edge.start
            } else {
                return None;
            };
            let endpoint = position_for(endpoint_id)?;
            (endpoint.y == min_y).then_some(endpoint.x)
        })
        .collect::<Vec<_>>();
    // The paper corners bound the bottom radial sectors. Rays to the other
    // three sides cannot enter one of these sectors, and every physical bottom
    // support ray partitions the interval below.
    bottom_ray_x.extend([min_x, max_x]);
    bottom_ray_x.sort_unstable_by(f64::total_cmp);
    bottom_ray_x.dedup();
    let (gap_min_x, gap_max_x) = bottom_ray_x
        .windows(2)
        .filter_map(|pair| {
            let width = pair[1] - pair[0];
            (width.is_finite() && width > 0.0).then_some((pair[0], pair[1], width))
        })
        .max_by(|left, right| {
            left.2
                .total_cmp(&right.2)
                // Stable tie-break: the leftmost open sector wins.
                .then_with(|| right.0.total_cmp(&left.0))
        })
        .map(|(left, right, _)| (left, right))?;

    // Use a shallow, axis-aligned rectangle strictly inside the open triangle
    // bounded by the two adjacent bottom rays and the common pivot. Axis-aligned
    // scaling preserves the independently normalized body/local/tree witnesses.
    let shallow_depth = 1.0 / 64.0;
    let deep_depth = 2.0 / 64.0;
    let ray_x = |boundary_x: f64, depth: f64| boundary_x + (center.x - boundary_x) * depth;
    let open_min_x = ray_x(gap_min_x, shallow_depth).max(ray_x(gap_min_x, deep_depth));
    let open_max_x = ray_x(gap_max_x, shallow_depth).min(ray_x(gap_max_x, deep_depth));
    let open_min_y = min_y + (center.y - min_y) * shallow_depth;
    let open_max_y = min_y + (center.y - min_y) * deep_depth;
    let open_width = open_max_x - open_min_x;
    let open_height = open_max_y - open_min_y;
    if ![
        open_min_x,
        open_max_x,
        open_min_y,
        open_max_y,
        open_width,
        open_height,
    ]
    .into_iter()
    .all(f64::is_finite)
        || open_width <= 0.0
        || open_height <= 0.0
    {
        return None;
    }
    let layout = BeginnerGenericAuxiliaryLayoutV1 {
        inner_min_x: open_min_x + open_width / 16.0,
        inner_max_x: open_max_x - open_width / 16.0,
        inner_min_y: open_min_y,
        inner_max_y: open_max_y,
        block_count,
    };
    (layout.inner_min_x < layout.inner_max_x && layout.inner_min_y < layout.inner_max_y)
        .then_some(layout)
}

fn beginner_generated_pattern_is_planar_v1(pattern: &CreasePattern) -> bool {
    let mut positions = std::collections::HashMap::with_capacity(pattern.vertices.len());
    let mut occupied_positions = Vec::with_capacity(pattern.vertices.len());
    for vertex in &pattern.vertices {
        if !vertex.position.x.is_finite()
            || !vertex.position.y.is_finite()
            || positions.insert(vertex.id, vertex.position).is_some()
            || occupied_positions.contains(&vertex.position)
        {
            return false;
        }
        occupied_positions.push(vertex.position);
    }
    let mut edge_ids = std::collections::HashSet::with_capacity(pattern.edges.len());
    let mut segments = Vec::with_capacity(pattern.edges.len());
    for edge in &pattern.edges {
        let Some(start) = positions.get(&edge.start).copied() else {
            return false;
        };
        let Some(end) = positions.get(&edge.end).copied() else {
            return false;
        };
        if edge.start == edge.end || start == end || !edge_ids.insert(edge.id) {
            return false;
        }
        segments.push((edge, start, end));
    }
    let orient = |first: Point2, second: Point2, third: Point2| {
        (second.x - first.x) * (third.y - first.y) - (second.y - first.y) * (third.x - first.x)
    };
    let on_closed_segment = |first: Point2, second: Point2, value: Point2| {
        orient(first, second, value) == 0.0
            && (first.x.min(second.x)..=first.x.max(second.x)).contains(&value.x)
            && (first.y.min(second.y)..=first.y.max(second.y)).contains(&value.y)
    };
    let mut checked_pairs = 0_usize;
    for (index, (left, a, b)) in segments.iter().enumerate() {
        for (right, c, d) in segments.iter().skip(index + 1) {
            checked_pairs = match checked_pairs.checked_add(1) {
                Some(count) if count <= 32_768 => count,
                _ => return false,
            };
            let first_shared = (left.start == right.start || left.start == right.end).then_some(*a);
            let second_shared = (left.end == right.start || left.end == right.end).then_some(*b);
            if first_shared.is_some() && second_shared.is_some() {
                return false;
            }
            if let Some(shared) = first_shared.or(second_shared) {
                let left_other = if *a == shared { *b } else { *a };
                let right_other = if *c == shared { *d } else { *c };
                if orient(shared, left_other, right_other) != 0.0 {
                    continue;
                }
                let left_vector = (left_other.x - shared.x, left_other.y - shared.y);
                let right_vector = (right_other.x - shared.x, right_other.y - shared.y);
                if left_vector.0 * right_vector.0 + left_vector.1 * right_vector.1 >= 0.0 {
                    return false;
                }
                continue;
            }
            let orientations = [
                orient(*a, *b, *c),
                orient(*a, *b, *d),
                orient(*c, *d, *a),
                orient(*c, *d, *b),
            ];
            let intersects = (orientations[0] == 0.0 && on_closed_segment(*a, *b, *c))
                || (orientations[1] == 0.0 && on_closed_segment(*a, *b, *d))
                || (orientations[2] == 0.0 && on_closed_segment(*c, *d, *a))
                || (orientations[3] == 0.0 && on_closed_segment(*c, *d, *b))
                || ((orientations[0] < 0.0 && orientations[1] > 0.0
                    || orientations[0] > 0.0 && orientations[1] < 0.0)
                    && (orientations[2] < 0.0 && orientations[3] > 0.0
                        || orientations[2] > 0.0 && orientations[3] < 0.0));
            if intersects {
                return false;
            }
        }
    }
    true
}

#[allow(clippy::too_many_arguments)]
fn append_bounded_radial_tree_graph(
    mut plan: BeginnerGeneratedPlanV1,
    constraints: &BeginnerGenerationConstraintsV1,
    namespace: ProjectId,
    min_x: f64,
    max_x: f64,
    min_y: f64,
    max_y: f64,
) -> Option<BeginnerGeneratedPlanV1> {
    let namespace = if plan.semantic_landmark_provenance.is_some() {
        ProjectId::schema_namespace([
            0x01, 0x90, 0x00, 0x00, 0x00, 0x00, 0x70, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x05, 0x97,
        ])
    } else {
        namespace
    };
    let ratios = bounded_tree_skeleton_length_ratios(&constraints.skeleton_segments)?;
    let segments = canonical_bounded_tree_segments(&constraints.skeleton_segments)?;
    let point = |value: BeginnerSkeletonPointV1| (value.x_tenths_mm, value.y_tenths_mm);
    let mut checked_pairs = 0_usize;
    for (index, left) in segments.iter().enumerate() {
        for right in segments.iter().skip(index + 1) {
            checked_pairs = checked_pairs.checked_add(1)?;
            if checked_pairs > MAX_BEGINNER_GENERIC_TREE_INTERSECTION_PAIRS_V1 {
                return None;
            }
            if bounded_tree_segments_intersect_beyond_shared_endpoint_v1(left, right) {
                return None;
            }
        }
    }
    let mut degree = std::collections::BTreeMap::<(i32, i32), usize>::new();
    for segment in &segments {
        *degree.entry(point(segment.start)).or_default() += 1;
        *degree.entry(point(segment.end)).or_default() += 1;
    }
    if degree.len() > MAX_BEGINNER_GENERIC_TREE_NODES_V1 {
        return None;
    }
    let leaf_count = degree.values().filter(|degree| **degree == 1).count();
    if !(2..=MAX_BEGINNER_GENERIC_TREE_BARS_V1).contains(&leaf_count)
        || constraints.protrusions.is_empty()
    {
        return None;
    }
    let (source_min_x, source_max_x, source_min_y, source_max_y) =
        skeleton_bounds(&constraints.skeleton_segments)?;
    let source_width = f64::from(source_max_x.checked_sub(source_min_x)?);
    let source_height = f64::from(source_max_y.checked_sub(source_min_y)?);
    if source_width <= 0.0 || source_height <= 0.0 || max_x <= min_x || max_y <= min_y {
        return None;
    }
    let had_physical_edges = plan
        .crease_pattern
        .edges
        .iter()
        .any(|edge| matches!(edge.kind, EdgeKind::Mountain | EdgeKind::Valley));
    let auxiliary_layout = if had_physical_edges {
        let center_id =
            beginner_generic_radial_center_v1(&plan.crease_pattern, min_x, max_x, min_y, max_y)?;
        Some(beginner_generic_auxiliary_layout_v1(
            &plan.crease_pattern,
            center_id,
            min_x,
            max_x,
            min_y,
            max_y,
            beginner_generic_auxiliary_block_count_v1(constraints)?,
        )?)
    } else {
        None
    };
    let tree_block_index = auxiliary_layout.map(|layout| layout.block_count.saturating_sub(1));
    let map = |source: (i32, i32)| {
        let x_ratio = f64::from(source.0.checked_sub(source_min_x)?) / source_width;
        let y_ratio = f64::from(source.1.checked_sub(source_min_y)?) / source_height;
        auxiliary_layout.map_or_else(
            || {
                Some(Point2::new(
                    min_x + (max_x - min_x) * (0.05 + x_ratio * 0.1),
                    min_y + (max_y - min_y) * (0.05 + y_ratio * 0.1),
                ))
            },
            |layout| layout.map(tree_block_index?, x_ratio, y_ratio),
        )
    };
    let mut vertex_ids = std::collections::BTreeMap::new();
    for source in degree.keys().copied() {
        let position = map(source)?;
        if !position.x.is_finite()
            || !position.y.is_finite()
            || !(min_x..=max_x).contains(&position.x)
            || !(min_y..=max_y).contains(&position.y)
        {
            return None;
        }
        let id = plan
            .crease_pattern
            .vertices
            .iter()
            .find(|vertex| vertex.position == position)
            .map_or_else(
                || {
                    VertexId::derive_v5(
                        namespace,
                        format!("bounded-tree-node:{}:{}", source.0, source.1).as_bytes(),
                    )
                },
                |vertex| vertex.id,
            );
        vertex_ids.insert(source, id);
        if !plan
            .crease_pattern
            .vertices
            .iter()
            .any(|vertex| vertex.id == id)
        {
            plan.crease_pattern.vertices.push(Vertex { id, position });
        }
    }
    for (index, segment) in segments.iter().enumerate() {
        let start = *vertex_ids.get(&point(segment.start))?;
        let end = *vertex_ids.get(&point(segment.end))?;
        plan.crease_pattern.edges.push(Edge {
            id: EdgeId::derive_v5(
                namespace,
                format!("bounded-tree-river:{index}:{}", ratios[index]).as_bytes(),
            ),
            start,
            end,
            // This compact corner graph is a bounded tree-method witness, not
            // a material hinge. Keeping it auxiliary prevents disconnected
            // planning metadata from entering the physical fold topology.
            kind: EdgeKind::Auxiliary,
        });
    }
    plan.instruction_codes.push(format!(
        "bounded_tree_branch_topology_v1:nodes={}:leaves={}:bars={}",
        degree.len(),
        leaf_count,
        segments.len()
    ));
    plan.skeleton_segments = segments;
    (!had_physical_edges || beginner_generated_pattern_is_planar_v1(&plan.crease_pattern))
        .then_some(plan)
}

fn canonical_bounded_tree_segments(
    segments: &[BeginnerSkeletonSegmentV1],
) -> Option<Vec<BeginnerSkeletonSegmentV1>> {
    if segments.is_empty() || segments.len() > MAX_BEGINNER_GENERIC_TREE_BARS_V1 {
        return None;
    }
    let mut canonical = segments.to_vec();
    canonical.sort_unstable_by_key(|segment| segment.id);
    if canonical.windows(2).any(|pair| pair[0].id == pair[1].id) {
        return None;
    }
    for segment in &mut canonical {
        let start = (segment.start.x_tenths_mm, segment.start.y_tenths_mm);
        let end = (segment.end.x_tenths_mm, segment.end.y_tenths_mm);
        if end < start {
            std::mem::swap(&mut segment.start, &mut segment.end);
        }
    }
    Some(canonical)
}

/// Canonicalizes a bounded generic tree exactly as the generator does.
///
/// Native provenance and clients that revalidate generated instructions must use
/// this helper rather than maintaining another sort/orientation implementation.
#[must_use]
pub fn canonical_beginner_generic_tree_segments_v1(
    segments: &[BeginnerSkeletonSegmentV1],
) -> Option<Vec<BeginnerSkeletonSegmentV1>> {
    canonical_bounded_tree_segments(segments)
}

fn bounded_tree_skeleton_length_ratios(segments: &[BeginnerSkeletonSegmentV1]) -> Option<Vec<u32>> {
    let segments = canonical_bounded_tree_segments(segments)?;
    let point = |point: BeginnerSkeletonPointV1| (point.x_tenths_mm, point.y_tenths_mm);
    let points = segments
        .iter()
        .flat_map(|segment| [point(segment.start), point(segment.end)])
        .collect::<std::collections::BTreeSet<_>>();
    if points.len() != segments.len() + 1
        || segments
            .iter()
            .any(|segment| point(segment.start) == point(segment.end))
    {
        return None;
    }
    let mut reached = std::collections::BTreeSet::from([point(segments[0].start)]);
    while reached.len() < points.len() {
        let before = reached.len();
        for segment in &segments {
            let start = point(segment.start);
            let end = point(segment.end);
            if reached.contains(&start) {
                reached.insert(end);
            }
            if reached.contains(&end) {
                reached.insert(start);
            }
        }
        if reached.len() == before {
            return None;
        }
    }
    let squared = segments
        .iter()
        .map(|segment| {
            let dx = i64::from(segment.end.x_tenths_mm) - i64::from(segment.start.x_tenths_mm);
            let dy = i64::from(segment.end.y_tenths_mm) - i64::from(segment.start.y_tenths_mm);
            u64::try_from(dx.saturating_mul(dx).saturating_add(dy.saturating_mul(dy))).ok()
        })
        .collect::<Option<Vec<_>>>()?;
    let minimum = *squared.iter().min()?;
    if minimum == 0 {
        return None;
    }
    squared
        .into_iter()
        .map(|value| u32::try_from(value.saturating_mul(1_000_000).checked_div(minimum)?).ok())
        .collect()
}

/// Returns the canonical, integer-normalized squared-length ratios used in the
/// `bounded_tree_river_axial_v1` instruction.
#[must_use]
pub fn beginner_generic_tree_length_ratios_v1(
    segments: &[BeginnerSkeletonSegmentV1],
) -> Option<Vec<u32>> {
    bounded_tree_skeleton_length_ratios(segments)
}

fn parameterized_landmark_endpoint_for_target(
    target: &BeginnerProtrusionTargetV1,
    skeleton_segments: &[BeginnerSkeletonSegmentV1],
) -> Option<(f64, f64)> {
    if target.count != 1 || target.symmetry != BeginnerProtrusionSymmetryV1::None {
        return None;
    }
    let (minimum_x, maximum_x, minimum_y, maximum_y) = skeleton_bounds(skeleton_segments)?;
    let span_x = maximum_x.checked_sub(minimum_x)?;
    let span_y = maximum_y.checked_sub(minimum_y)?;
    if span_x <= 0
        || span_y <= 0
        || !(minimum_x..=maximum_x).contains(&target.position_tenths_mm[0])
        || !(minimum_y..=maximum_y).contains(&target.position_tenths_mm[1])
    {
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
    let length_ratio =
        f64::from(target.length_tenths_mm) / f64::from(u32::try_from(primary_span).ok()?);
    if !(0.02..=0.45).contains(&length_ratio) || primary_direction == 0 {
        return None;
    }
    let x = f64::from(target.position_tenths_mm[0].checked_sub(minimum_x)?)
        / f64::from(u32::try_from(span_x).ok()?);
    let y = f64::from(target.position_tenths_mm[1].checked_sub(minimum_y)?)
        / f64::from(u32::try_from(span_y).ok()?);
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
    parameterized_symmetric_endpoints_for_target(
        target,
        constraints.skeleton_segments.as_slice(),
        count,
        vertical,
    )
}

fn parameterized_symmetric_endpoints_for_target(
    target: &BeginnerProtrusionTargetV1,
    skeleton_segments: &[BeginnerSkeletonSegmentV1],
    count: u8,
    vertical: bool,
) -> Option<[(f64, f64); 4]> {
    if target.count != count || target.symmetry != BeginnerProtrusionSymmetryV1::Bilateral {
        return None;
    }
    let bounds = skeleton_bounds(skeleton_segments)?;
    if !protrusion_local_outline_within_bounds_v1(target, bounds) {
        return None;
    }
    let (minimum_x, maximum_x, minimum_y, maximum_y) = bounds;
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
    let length_ratio =
        f64::from(target.length_tenths_mm) / f64::from(u32::try_from(primary_span).ok()?);
    let root_width = target
        .root_width_tenths_mm
        .unwrap_or(u32::from(target.thickness_tenths_mm));
    let tip_width = target.tip_width_tenths_mm.unwrap_or(root_width);
    let width_ratio = f64::from(root_width.checked_add(tip_width)?)
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
    let center_y = f64::from(center_offset) / f64::from(u32::try_from(span_y).ok()?);
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
        let target_position = canonical_quad.as_ref().map_or_else(
            || {
                Point2::new(
                    min_x + (max_x - min_x) * x_ratio,
                    min_y + (max_y - min_y) * y_ratio,
                )
            },
            |(_, points)| points[index],
        );
        let position = if canonical_quad.is_some() {
            target_position
        } else {
            let dx = target_position.x - center.x;
            let dy = target_position.y - center.y;
            let x_scale = if dx > 0.0 {
                (max_x - center.x) / dx
            } else if dx < 0.0 {
                (min_x - center.x) / dx
            } else {
                f64::INFINITY
            };
            let y_scale = if dy > 0.0 {
                (max_y - center.y) / dy
            } else if dy < 0.0 {
                (min_y - center.y) / dy
            } else {
                f64::INFINITY
            };
            let scale = x_scale.min(y_scale);
            Point2::new(center.x + dx * scale, center.y + dy * scale)
        };
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
    let mut instruction_codes = vec![instruction.to_owned()];
    let needs_even_radial_support = endpoints.len() >= 6 && endpoints.len().is_multiple_of(2);
    let needs_symmetric_four_leg_radial_support =
        plan_kind == BeginnerGeneratedPlanKindV1::SymmetricFourLegBase && endpoints.len() == 4;
    let needs_small_generic_radial_support = plan_kind
        == BeginnerGeneratedPlanKindV1::CompositeGenericTargetBase
        && matches!(endpoints.len(), 2 | 4);
    let needs_odd_generic_radial_support = plan_kind
        == BeginnerGeneratedPlanKindV1::CompositeGenericTargetBase
        && matches!(endpoints.len(), 3 | 5 | 7 | 9 | 11 | 13);
    if !asymmetric_landmark
        && (needs_even_radial_support
            || needs_symmetric_four_leg_radial_support
            || needs_small_generic_radial_support
            || needs_odd_generic_radial_support)
    {
        let corners = [
            Point2::new(min_x, min_y),
            Point2::new(max_x, min_y),
            Point2::new(max_x, max_y),
            Point2::new(min_x, max_y),
        ];
        let edge_reaches_position = |edge: &Edge, vertices: &[Vertex], position: Point2| {
            let position_for = |id| {
                vertices
                    .iter()
                    .find(|vertex| vertex.id == id)
                    .map(|vertex| vertex.position)
            };
            (edge.start == center_id && position_for(edge.end) == Some(position))
                || (edge.end == center_id && position_for(edge.start) == Some(position))
        };
        let mut support_edges = Vec::with_capacity(corners.len() + 1);
        for (index, corner) in corners.into_iter().enumerate() {
            let already_covered = edges
                .iter()
                .any(|edge| edge_reaches_position(edge, &vertices, corner));
            if already_covered {
                continue;
            }
            let Some(corner_vertex) = source
                .vertices
                .iter()
                .filter(|vertex| vertex.position == corner)
                .min_by_key(|vertex| vertex.id.canonical_bytes())
                .cloned()
            else {
                continue;
            };
            if !vertices.iter().any(|vertex| vertex.id == corner_vertex.id) {
                vertices.push(corner_vertex.clone());
            }
            support_edges.push(Edge {
                id: EdgeId::derive_v5(
                    namespace,
                    format!("{prefix}-corner-support-e-{index}").as_bytes(),
                ),
                start: center_id,
                end: corner_vertex.id,
                kind: edge_kind,
            });
        }
        let minimum_radial_hinges =
            if needs_small_generic_radial_support || needs_symmetric_four_leg_radial_support {
                6
            } else {
                0
            };
        loop {
            let Some(total_radial_hinges) = edges.len().checked_add(support_edges.len()) else {
                break;
            };
            if total_radial_hinges >= minimum_radial_hinges && total_radial_hinges.is_multiple_of(2)
            {
                break;
            }
            // An odd radial single-vertex cycle cannot carry the strict
            // opposite-hinge schedule used by the graph certificate. Small
            // generic fans and the symmetric four-leg family additionally
            // require at least six distinct rays to enter that theorem.
            // Deterministic non-corner boundary rays satisfy both bounds
            // without changing semantic feature order. At most fourteen
            // semantic rays and four corner supports exist in either bounded
            // case, so these 63 dyadic positions are sufficient.
            let extra_support = (1_u8..64).find_map(|slot| {
                let position = Point2::new(min_x + (max_x - min_x) * f64::from(slot) / 64.0, min_y);
                let already_covered = edges
                    .iter()
                    .chain(&support_edges)
                    .any(|edge| edge_reaches_position(edge, &vertices, position));
                (!already_covered).then_some((slot, position))
            });
            let Some((slot, position)) = extra_support else {
                break;
            };
            let extra_vertex = vertices
                .iter()
                .chain(&source.vertices)
                .filter(|vertex| vertex.position == position)
                .min_by_key(|vertex| vertex.id.canonical_bytes())
                .cloned()
                .unwrap_or_else(|| Vertex {
                    id: VertexId::derive_v5(
                        namespace,
                        format!("{prefix}-parity-support-v-{slot}").as_bytes(),
                    ),
                    position,
                });
            if !vertices.iter().any(|vertex| vertex.id == extra_vertex.id) {
                vertices.push(extra_vertex.clone());
            }
            support_edges.push(Edge {
                id: EdgeId::derive_v5(
                    namespace,
                    format!("{prefix}-parity-support-e-{slot}").as_bytes(),
                ),
                start: center_id,
                end: extra_vertex.id,
                kind: edge_kind,
            });
        }
        let added_supports = support_edges.len();
        support_edges.append(&mut edges);
        edges = support_edges;
        let covered_corners = corners
            .into_iter()
            .filter(|corner| {
                edges
                    .iter()
                    .any(|edge| edge_reaches_position(edge, &vertices, *corner))
            })
            .count();
        if covered_corners == corners.len()
            && edges.len() >= minimum_radial_hinges
            && edges.len().is_multiple_of(2)
        {
            instruction_codes.push(format!(
                "bounded_radial_corner_support_v1:added={added_supports}:covered={covered_corners}"
            ));
        }
    }
    let generic_auxiliary_layout = (plan_kind
        == BeginnerGeneratedPlanKindV1::CompositeGenericTargetBase)
        .then(|| {
            let pattern = CreasePattern {
                vertices: vertices.clone(),
                edges: edges.clone(),
            };
            let center_id =
                beginner_generic_radial_center_v1(&pattern, min_x, max_x, min_y, max_y)?;
            beginner_generic_auxiliary_layout_v1(
                &pattern,
                center_id,
                min_x,
                max_x,
                min_y,
                max_y,
                beginner_generic_auxiliary_block_count_v1(constraints)?,
            )
        })
        .flatten();
    if let Some(outline) = &constraints.generic_body_outline_tenths_mm {
        let Some((skeleton_min_x, skeleton_max_x, skeleton_min_y, skeleton_max_y)) =
            skeleton_bounds(&constraints.skeleton_segments)
        else {
            return BeginnerGeneratedPlanV1 {
                schema_version: BEGINNER_GENERATOR_SCHEMA_VERSION_V1,
                kind: plan_kind,
                crease_pattern: CreasePattern { vertices, edges },
                instruction_codes,
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
                let x_ratio = f64::from(point[0] - skeleton_min_x) / skeleton_span_x;
                let y_ratio = f64::from(point[1] - skeleton_min_y) / skeleton_span_y;
                let position = generic_auxiliary_layout
                    .and_then(|layout| layout.map(0, x_ratio, y_ratio))
                    .unwrap_or_else(|| {
                        Point2::new(
                            min_x + (max_x - min_x) * x_ratio,
                            min_y + (max_y - min_y) * y_ratio,
                        )
                    });
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
                // Body contours are placement guides bound by provenance, not
                // additional disconnected physical hinges.
                kind: EdgeKind::Auxiliary,
            });
        }
    }
    if let Some((skeleton_min_x, skeleton_max_x, skeleton_min_y, skeleton_max_y)) =
        skeleton_bounds(&constraints.skeleton_segments)
    {
        let skeleton_span_x = f64::from(skeleton_max_x - skeleton_min_x);
        let skeleton_span_y = f64::from(skeleton_max_y - skeleton_min_y);
        let local_block_start = usize::from(constraints.generic_body_outline_tenths_mm.is_some());
        let mut local_block_ordinal = 0_usize;
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
                    let x_ratio = f64::from(x - skeleton_min_x) / skeleton_span_x;
                    let y_ratio = f64::from(y - skeleton_min_y) / skeleton_span_y;
                    let position = generic_auxiliary_layout
                        .and_then(|layout| {
                            layout.map(local_block_start + local_block_ordinal, x_ratio, y_ratio)
                        })
                        .unwrap_or_else(|| {
                            Point2::new(
                                min_x + (max_x - min_x) * x_ratio,
                                min_y + (max_y - min_y) * y_ratio,
                            )
                        });
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
                    // Local protrusion contours remain exact guide cycles and
                    // are excluded from the material fold graph.
                    kind: EdgeKind::Auxiliary,
                });
            }
            local_block_ordinal += 1;
        }
    }
    BeginnerGeneratedPlanV1 {
        schema_version: BEGINNER_GENERATOR_SCHEMA_VERSION_V1,
        kind: plan_kind,
        crease_pattern: CreasePattern { vertices, edges },
        instruction_codes,
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
        .iter()
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

    fn radial_corner_support_added(plan: &BeginnerGeneratedPlanV1) -> usize {
        let support_codes = plan
            .instruction_codes
            .iter()
            .filter(|code| code.starts_with("bounded_radial_corner_support_v1:"))
            .collect::<Vec<_>>();
        let [support_code] = support_codes.as_slice() else {
            panic!("one canonical radial-corner support instruction is required");
        };
        let (added, covered) = support_code
            .strip_prefix("bounded_radial_corner_support_v1:added=")
            .and_then(|payload| payload.split_once(":covered="))
            .expect("canonical radial-corner support instruction");
        let added = added.parse::<usize>().expect("bounded support count");
        assert!(added <= 5);
        assert_eq!(covered, "4");
        added
    }

    #[test]
    fn radial_corner_support_is_canonical_and_duplicate_corner_safe() {
        let namespace = ProjectId::schema_namespace([0x73; 16]);
        let (_ids, mut source) = square_source(namespace);
        source.vertices.push(Vertex {
            id: VertexId::derive_v5(namespace, b"duplicate-bottom-left"),
            position: Point2::new(0.0, 0.0),
        });
        source.vertices.extend(
            [
                b"duplicate-parity-a".as_slice(),
                b"duplicate-parity-b".as_slice(),
            ]
            .map(|seed| Vertex {
                id: VertexId::derive_v5(namespace, seed),
                position: Point2::new(10.0 / 64.0, 0.0),
            }),
        );
        let endpoints = [
            (0.0, 0.4),
            (1.0, 0.6),
            (0.4, 0.0),
            (0.6, 1.0),
            (0.2, 0.3),
            (0.8, 0.7),
        ];
        let generate = |source: &CreasePattern| {
            symmetric_template(
                namespace,
                source,
                BeginnerGeneratedPlanKindV1::CompositeGenericTargetBase,
                EdgeKind::Valley,
                0.0,
                10.0,
                0.0,
                10.0,
                &endpoints,
                "bounded_test_radial_fan",
                &BeginnerGenerationConstraintsV1::default(),
            )
        };
        let plan = generate(&source);
        let mut reordered_source = source.clone();
        reordered_source.vertices.reverse();
        assert_eq!(generate(&reordered_source), plan);
        assert_eq!(radial_corner_support_added(&plan), 4);
        assert_eq!(plan.crease_pattern.vertices.len(), 11);
        assert_eq!(plan.crease_pattern.edges.len(), 10);

        let center = Point2::new(5.0, 5.0);
        let center_id = plan
            .crease_pattern
            .vertices
            .iter()
            .find(|vertex| vertex.position == center)
            .expect("radial center")
            .id;
        for (index, corner) in [
            Point2::new(0.0, 0.0),
            Point2::new(10.0, 0.0),
            Point2::new(10.0, 10.0),
            Point2::new(0.0, 10.0),
        ]
        .into_iter()
        .enumerate()
        {
            let edge = &plan.crease_pattern.edges[index];
            assert_eq!(
                edge.id,
                EdgeId::derive_v5(
                    namespace,
                    format!(
                        "beginner-plan-{:?}-corner-support-e-{index}",
                        BeginnerGeneratedPlanKindV1::CompositeGenericTargetBase
                    )
                    .as_bytes(),
                )
            );
            assert_eq!(edge.start, center_id);
            assert_eq!(edge.kind, EdgeKind::Valley);
            let expected_corner_id = source
                .vertices
                .iter()
                .filter(|vertex| vertex.position == corner)
                .min_by_key(|vertex| vertex.id.canonical_bytes())
                .expect("source paper corner")
                .id;
            assert_eq!(edge.end, expected_corner_id);
        }

        let odd_endpoints = [endpoints[0], endpoints[1], endpoints[2]];
        let generate_odd = |source: &CreasePattern| {
            symmetric_template(
                namespace,
                source,
                BeginnerGeneratedPlanKindV1::CompositeGenericTargetBase,
                EdgeKind::Valley,
                0.0,
                10.0,
                0.0,
                10.0,
                &odd_endpoints,
                "bounded_test_odd_radial_fan",
                &BeginnerGenerationConstraintsV1::default(),
            )
        };
        let odd_plan = generate_odd(&source);
        assert_eq!(generate_odd(&reordered_source), odd_plan);
        assert_eq!(radial_corner_support_added(&odd_plan), 5);
        assert_eq!(odd_plan.crease_pattern.vertices.len(), 9);
        assert_eq!(odd_plan.crease_pattern.edges.len(), 8);
        let parity_edge = &odd_plan.crease_pattern.edges[4];
        assert_eq!(
            parity_edge.id,
            EdgeId::derive_v5(
                namespace,
                format!(
                    "beginner-plan-{:?}-parity-support-e-1",
                    BeginnerGeneratedPlanKindV1::CompositeGenericTargetBase
                )
                .as_bytes(),
            )
        );
        assert_eq!(parity_edge.start, center_id);
        assert_eq!(parity_edge.kind, EdgeKind::Valley);
        assert_ne!(parity_edge.start, parity_edge.end);
        assert_eq!(
            parity_edge.end,
            source
                .vertices
                .iter()
                .filter(|vertex| vertex.position == Point2::new(10.0 / 64.0, 0.0))
                .min_by_key(|vertex| vertex.id.canonical_bytes())
                .expect("canonical duplicate parity support vertex")
                .id
        );
        let parity_position = odd_plan
            .crease_pattern
            .vertices
            .iter()
            .find(|vertex| vertex.id == parity_edge.end)
            .expect("canonical odd-fan parity support vertex")
            .position;
        assert_eq!(parity_position, Point2::new(10.0 / 64.0, 0.0));
        assert!(
            ![
                Point2::new(0.0, 0.0),
                Point2::new(10.0, 0.0),
                Point2::new(10.0, 10.0),
                Point2::new(0.0, 10.0),
            ]
            .contains(&parity_position)
        );

        let (_clean_ids, clean_source) = square_source(namespace);
        let colliding_endpoints = [(1.0 / 64.0, 0.0), endpoints[0], endpoints[1]];
        let collision_safe = symmetric_template(
            namespace,
            &clean_source,
            BeginnerGeneratedPlanKindV1::CompositeGenericTargetBase,
            EdgeKind::Valley,
            0.0,
            10.0,
            0.0,
            10.0,
            &colliding_endpoints,
            "bounded_test_collision_safe_odd_radial_fan",
            &BeginnerGenerationConstraintsV1::default(),
        );
        assert_eq!(radial_corner_support_added(&collision_safe), 5);
        assert_eq!(
            collision_safe.crease_pattern.edges[4].id,
            EdgeId::derive_v5(
                namespace,
                format!(
                    "beginner-plan-{:?}-parity-support-e-2",
                    BeginnerGeneratedPlanKindV1::CompositeGenericTargetBase
                )
                .as_bytes(),
            )
        );
        assert!(
            collision_safe
                .crease_pattern
                .edges
                .iter()
                .all(|edge| edge.start != edge.end)
        );
        assert_eq!(
            collision_safe
                .crease_pattern
                .edges
                .iter()
                .map(|edge| edge.end)
                .collect::<std::collections::HashSet<_>>()
                .len(),
            collision_safe.crease_pattern.edges.len()
        );
    }

    #[test]
    fn bounded_tree_ratios_reject_cycles_limits_and_degenerate_bars() {
        let bar = |id: u16, start: (i32, i32), end: (i32, i32)| BeginnerSkeletonSegmentV1 {
            id,
            start: BeginnerSkeletonPointV1 {
                x_tenths_mm: start.0,
                y_tenths_mm: start.1,
            },
            end: BeginnerSkeletonPointV1 {
                x_tenths_mm: end.0,
                y_tenths_mm: end.1,
            },
            thickness_tenths_mm: 1,
        };
        let tree = vec![bar(0, (0, 0), (10, 0)), bar(1, (10, 0), (10, 20))];
        assert_eq!(
            bounded_tree_skeleton_length_ratios(&tree),
            Some(vec![1_000_000, 4_000_000])
        );
        let eight = (0..8)
            .map(|id| bar(id, (id as i32, 0), (id as i32 + 1, 0)))
            .collect::<Vec<_>>();
        assert_eq!(
            bounded_tree_skeleton_length_ratios(&eight).unwrap().len(),
            8
        );
        assert!(
            bounded_tree_skeleton_length_ratios(&[
                bar(0, (0, 0), (10, 0)),
                bar(1, (10, 0), (5, 10)),
                bar(2, (5, 10), (0, 0)),
            ])
            .is_none()
        );
        assert!(bounded_tree_skeleton_length_ratios(&[bar(0, (0, 0), (0, 0))]).is_none());
        let nine = (0..9)
            .map(|id| bar(id, (id as i32, 0), (id as i32 + 1, 0)))
            .collect::<Vec<_>>();
        assert_eq!(bounded_tree_skeleton_length_ratios(&nine).unwrap().len(), 9);
        let sixteen = (0..16)
            .map(|id| bar(id, (id as i32, 0), (id as i32 + 1, 0)))
            .collect::<Vec<_>>();
        assert_eq!(
            bounded_tree_skeleton_length_ratios(&sixteen).unwrap().len(),
            16
        );
        let mut reversed_sixteen = sixteen.clone();
        reversed_sixteen.reverse();
        assert_eq!(
            bounded_tree_skeleton_length_ratios(&reversed_sixteen),
            bounded_tree_skeleton_length_ratios(&sixteen)
        );
        let seventeen = (0..17)
            .map(|id| bar(id, (id as i32, 0), (id as i32 + 1, 0)))
            .collect::<Vec<_>>();
        assert!(bounded_tree_skeleton_length_ratios(&seventeen).is_none());
        let mut reversed_seventeen = seventeen;
        reversed_seventeen.reverse();
        assert!(bounded_tree_skeleton_length_ratios(&reversed_seventeen).is_none());

        let crossing = vec![
            bar(0, (-10, 0), (10, 0)),
            bar(1, (0, -10), (0, 10)),
            bar(2, (10, 0), (0, 10)),
        ];
        let constraints = BeginnerGenerationConstraintsV1 {
            skeleton_segments: crossing,
            protrusions: vec![bilateral_protrusion(1, 2), bilateral_protrusion(2, 2)],
            ..BeginnerGenerationConstraintsV1::default()
        };
        let plan = BeginnerGeneratedPlanV1 {
            schema_version: 1,
            kind: BeginnerGeneratedPlanKindV1::CompositeGenericTargetBase,
            crease_pattern: CreasePattern {
                vertices: Vec::new(),
                edges: Vec::new(),
            },
            instruction_codes: Vec::new(),
            target_parts: Vec::new(),
            skeleton_segments: Vec::new(),
            target_asset: None,
            semantic_landmark_provenance: None,
        };
        assert!(
            append_bounded_radial_tree_graph(
                plan,
                &constraints,
                ProjectId::new(),
                0.0,
                100.0,
                0.0,
                100.0,
            )
            .is_none()
        );
    }

    #[test]
    fn generic_tree_segment_order_is_canonical_and_sorted_fixture_stays_bit_exact() {
        let bar = |id: u16, start: (i32, i32), end: (i32, i32)| BeginnerSkeletonSegmentV1 {
            id,
            start: BeginnerSkeletonPointV1 {
                x_tenths_mm: start.0,
                y_tenths_mm: start.1,
            },
            end: BeginnerSkeletonPointV1 {
                x_tenths_mm: end.0,
                y_tenths_mm: end.1,
            },
            thickness_tenths_mm: 1,
        };
        let constraints = BeginnerGenerationConstraintsV1 {
            skeleton_segments: vec![
                bar(10, (0, 0), (10, 0)),
                bar(20, (10, 0), (10, 20)),
                bar(30, (10, 20), (30, 20)),
                bar(40, (30, 20), (30, 50)),
            ],
            protrusions: vec![bilateral_protrusion(1, 2)],
            ..BeginnerGenerationConstraintsV1::default()
        };
        let namespace = ProjectId::schema_namespace([
            0x01, 0x90, 0x00, 0x00, 0x00, 0x00, 0x70, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x09, 0x91,
        ]);
        let generate = |constraints: &BeginnerGenerationConstraintsV1| {
            let ratios = bounded_tree_skeleton_length_ratios(&constraints.skeleton_segments)?;
            let plan = BeginnerGeneratedPlanV1 {
                schema_version: 1,
                kind: BeginnerGeneratedPlanKindV1::CompositeGenericTargetBase,
                crease_pattern: CreasePattern {
                    vertices: Vec::new(),
                    edges: Vec::new(),
                },
                instruction_codes: vec![format!(
                    "bounded_tree_river_axial_v1:{}",
                    ratios
                        .iter()
                        .map(u32::to_string)
                        .collect::<Vec<_>>()
                        .join(",")
                )],
                target_parts: Vec::new(),
                skeleton_segments: constraints.skeleton_segments.clone(),
                target_asset: None,
                semantic_landmark_provenance: None,
            };
            append_bounded_radial_tree_graph(plan, constraints, namespace, 0.0, 100.0, 0.0, 100.0)
        };
        let generated = generate(&constraints).expect("sorted fixture");
        assert_eq!(
            generated
                .crease_pattern
                .edges
                .iter()
                .map(|edge| edge.kind)
                .collect::<Vec<_>>(),
            [
                EdgeKind::Auxiliary,
                EdgeKind::Auxiliary,
                EdgeKind::Auxiliary,
                EdgeKind::Auxiliary,
            ]
        );
        assert_eq!(
            generated.instruction_codes,
            [
                "bounded_tree_river_axial_v1:1000000,4000000,4000000,9000000",
                "bounded_tree_branch_topology_v1:nodes=5:leaves=2:bars=4",
            ]
        );
        let json = serde_json::to_string(&generated).unwrap();
        assert_eq!(
            <[u8; 32]>::from(Sha256::digest(json.as_bytes())),
            [
                0x9d, 0xb0, 0xcc, 0x67, 0x47, 0x32, 0x4c, 0x3c, 0xd0, 0x56, 0xb0, 0x76, 0x54, 0x64,
                0xb1, 0xc7, 0x99, 0x47, 0x4a, 0x68, 0xc3, 0xe5, 0x22, 0x99, 0x5a, 0xf0, 0x60, 0x8c,
                0xd4, 0x6d, 0x2f, 0xac,
            ],
            "the current generator-v1 auxiliary tree-guide checkpoint, including edge IDs, must stay bit-exact"
        );

        let mut reversed = constraints.clone();
        reversed.skeleton_segments.reverse();
        assert_eq!(generate(&reversed), Some(generated.clone()));
        let mut all_endpoints_reversed = constraints.clone();
        for segment in &mut all_endpoints_reversed.skeleton_segments {
            std::mem::swap(&mut segment.start, &mut segment.end);
        }
        assert_eq!(generate(&all_endpoints_reversed), Some(generated.clone()));
        let mut one_endpoint_reversed = constraints.clone();
        let segment = &mut one_endpoint_reversed.skeleton_segments[2];
        std::mem::swap(&mut segment.start, &mut segment.end);
        assert_eq!(generate(&one_endpoint_reversed), Some(generated.clone()));
        let mut shuffled = constraints.clone();
        shuffled.skeleton_segments = [2, 0, 3, 1]
            .map(|index| constraints.skeleton_segments[index])
            .to_vec();
        for segment in &mut shuffled.skeleton_segments {
            std::mem::swap(&mut segment.start, &mut segment.end);
        }
        assert_eq!(generate(&shuffled), Some(generated));

        let mut duplicate = constraints;
        duplicate.skeleton_segments[1].id = duplicate.skeleton_segments[0].id;
        assert!(generate(&duplicate).is_none());
    }

    #[test]
    fn generic_custom_tree_generates_bounded_radial_protrusion_groups() {
        let namespace = ProjectId::schema_namespace([
            0x01, 0x90, 0x00, 0x00, 0x00, 0x00, 0x70, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x09, 0x92,
        ]);
        let ids = ["a", "b", "c", "d"].map(|name| VertexId::derive_v5(namespace, name.as_bytes()));
        let source = CreasePattern {
            vertices: ids
                .iter()
                .copied()
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
        let radial =
            |id: u16, count: u8, position: (i32, i32), direction: (i16, i16), length: u32| {
                crate::BeginnerProtrusionTargetV1 {
                    id,
                    count,
                    length_tenths_mm: length,
                    thickness_tenths_mm: 10,
                    root_width_tenths_mm: None,
                    tip_width_tenths_mm: None,
                    local_outline_tenths_mm: None,
                    position_tenths_mm: [position.0, position.1, 0],
                    direction_milli: [direction.0, direction.1, 0],
                    symmetry: BeginnerProtrusionSymmetryV1::Radial,
                    curvature_degrees: 0,
                    joint: crate::BeginnerProtrusionJointV1::Fixed,
                    motion_degrees: [0, 0],
                    side: crate::BeginnerProtrusionSideV1::Either,
                    priority: 100,
                }
            };
        let constraints = BeginnerGenerationConstraintsV1 {
            target_category: Some(BeginnerTargetCategoryV1::CustomObject),
            skeleton_segments: vec![
                skeleton(10, 0, 0, 1_000, 0),
                skeleton(20, 1_000, 0, 1_000, 1_000),
            ],
            protrusions: vec![
                radial(1, 3, (400, 400), (1_000, 0), 100),
                radial(2, 2, (650, 650), (0, 1_000), 80),
            ],
            ..BeginnerGenerationConstraintsV1::default()
        };
        assert!(crate::validate_beginner_generation_constraints_v1(
            &constraints
        ));
        assert_eq!(beginner_target_approximation_score_v1(&constraints), 100);
        let endpoints =
            bounded_generic_composite_endpoints(&constraints).expect("radial target endpoints");
        assert_eq!(endpoints.len(), 5);
        assert!((endpoints[0].0 - 0.5).abs() <= f64::EPSILON);
        assert!((endpoints[0].1 - 0.4).abs() <= f64::EPSILON);
        assert!((endpoints[3].0 - 0.65).abs() <= f64::EPSILON);
        assert!((endpoints[3].1 - 0.73).abs() <= f64::EPSILON);

        let generated = generate_beginner_plans_v1(namespace, &source, &ids, &constraints).unwrap();
        assert_eq!(generated.len(), 1);
        assert_eq!(
            generated[0].kind,
            BeginnerGeneratedPlanKindV1::CompositeGenericTargetBase
        );
        assert!(
            beginner_generated_pattern_is_planar_v1(&generated[0].crease_pattern),
            "support-complete generic fan and its relocated tree must be planar"
        );
        let support_count = radial_corner_support_added(&generated[0]);
        assert_eq!(support_count, 5);
        assert_eq!(generated[0].crease_pattern.edges.len(), 7 + support_count);
        assert_eq!(
            generated[0].instruction_codes[2],
            "bounded_tree_branch_topology_v1:nodes=3:leaves=2:bars=2"
        );

        let mut reversed = constraints.clone();
        reversed.skeleton_segments.reverse();
        assert_eq!(
            generate_beginner_plans_v1(namespace, &source, &ids, &reversed),
            Ok(generated)
        );

        let mut over_endpoint_limit = constraints;
        over_endpoint_limit.protrusions = (0_u16..5)
            .map(|index| {
                radial(
                    index + 1,
                    8,
                    (200 + i32::from(index) * 150, 200 + i32::from(index) * 120),
                    (1_000, 0),
                    20,
                )
            })
            .collect();
        assert!(bounded_generic_composite_endpoints(&over_endpoint_limit).is_none());
    }

    #[test]
    fn generic_exact_bilateral_pair_is_axis_centered_and_sign_canonical() {
        let segments = vec![
            skeleton(10, -100, -100, 100, -100),
            skeleton(20, 100, -100, 100, 100),
        ];
        let endpoints = |direction| {
            let mut target = bilateral_protrusion(1, 2);
            target.length_tenths_mm = 20;
            target.thickness_tenths_mm = 10;
            target.position_tenths_mm = [0, 0, 0];
            target.direction_milli = direction;
            bounded_generic_composite_endpoints(&BeginnerGenerationConstraintsV1 {
                target_category: Some(BeginnerTargetCategoryV1::CustomObject),
                skeleton_segments: segments.clone(),
                protrusions: vec![target],
                ..BeginnerGenerationConstraintsV1::default()
            })
            .expect("exact bilateral pair endpoints")
        };

        let horizontal = endpoints([1_000, 0, 0]);
        assert_eq!(horizontal.len(), 2);
        assert_eq!(
            horizontal
                .iter()
                .map(|(_, y)| y.to_bits())
                .collect::<Vec<_>>(),
            vec![0.5_f64.to_bits(); 2]
        );
        assert_eq!(endpoints([-1_000, 0, 0]), horizontal);

        let vertical = endpoints([0, 1_000, 0]);
        assert_eq!(vertical.len(), 2);
        assert_eq!(
            vertical
                .iter()
                .map(|(x, _)| x.to_bits())
                .collect::<Vec<_>>(),
            vec![0.5_f64.to_bits(); 2]
        );
        assert_eq!(endpoints([0, -1_000, 0]), vertical);
        assert_ne!(horizontal, vertical);
    }

    #[test]
    fn generic_custom_tree_generates_six_and_eight_bilateral_protrusions() {
        let namespace = ProjectId::schema_namespace([
            0x01, 0x90, 0x00, 0x00, 0x00, 0x00, 0x70, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x09, 0x93,
        ]);
        let ids = ["a", "b", "c", "d"].map(|name| VertexId::derive_v5(namespace, name.as_bytes()));
        let source = CreasePattern {
            vertices: ids
                .iter()
                .copied()
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
        let mut six = bilateral_protrusion(1, 6);
        six.length_tenths_mm = 20;
        six.position_tenths_mm = [0, -50, 0];
        six.direction_milli = [1_000, 0, 0];
        let mut eight = bilateral_protrusion(2, 8);
        eight.length_tenths_mm = 20;
        eight.position_tenths_mm = [0, 50, 0];
        eight.direction_milli = [0, 1_000, 0];
        let constraints = BeginnerGenerationConstraintsV1 {
            target_category: Some(BeginnerTargetCategoryV1::CustomObject),
            skeleton_segments: vec![
                skeleton(10, -100, -100, 100, -100),
                skeleton(20, 100, -100, 100, 100),
            ],
            protrusions: vec![six, eight],
            ..BeginnerGenerationConstraintsV1::default()
        };
        assert!(crate::validate_beginner_generation_constraints_v1(
            &constraints
        ));
        let endpoints = bounded_generic_composite_endpoints(&constraints)
            .expect("extended bilateral endpoints");
        assert_eq!(endpoints.len(), 14);
        for pair in endpoints[..6]
            .chunks_exact(2)
            .chain(endpoints[6..].chunks_exact(2))
        {
            assert!((pair[0].0 + pair[1].0 - 1.0).abs() <= f64::EPSILON);
            assert!((pair[0].1 - pair[1].1).abs() <= f64::EPSILON);
        }

        let generated = generate_beginner_plans_v1(namespace, &source, &ids, &constraints).unwrap();
        assert_eq!(generated.len(), 1);
        assert_eq!(
            generated[0].kind,
            BeginnerGeneratedPlanKindV1::CompositeGenericTargetBase
        );
        assert!(
            beginner_generated_pattern_is_planar_v1(&generated[0].crease_pattern),
            "the larger support-complete generic fan must retain planar auxiliary placement"
        );
        assert_eq!(
            generated[0].crease_pattern.edges.len(),
            16 + radial_corner_support_added(&generated[0])
        );

        let mut reversed = constraints.clone();
        reversed.skeleton_segments.reverse();
        assert_eq!(
            generate_beginner_plans_v1(namespace, &source, &ids, &reversed),
            Ok(generated)
        );

        let mut off_axis = constraints;
        off_axis.protrusions[0].position_tenths_mm[0] = 1;
        assert!(crate::validate_beginner_generation_constraints_v1(
            &off_axis
        ));
        assert_eq!(
            generate_beginner_plans_v1(namespace, &source, &ids, &off_axis),
            Err(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate)
        );
    }

    #[test]
    fn generic_legacy_candidate_buffers_are_bounded_and_arithmetic_fail_closed() {
        let single = try_endpoint_candidates_v1([(0.25, 0.75)]).expect("one bounded candidate");
        assert_eq!(single.as_slice(), &[(0.25, 0.75)]);
        let four =
            try_endpoint_candidates_v1([(0.25, 0.25), (0.25, 0.75), (0.75, 0.25), (0.75, 0.75)])
                .expect("four bounded candidates");
        assert_eq!(four.len(), 4);
        assert_eq!(four[0], (0.25, 0.25));
        assert_eq!(four[3], (0.75, 0.75));

        for count in [2, 4] {
            let mut target = bilateral_protrusion(1, count);
            target.length_tenths_mm = 5;
            target.position_tenths_mm = [0, 0, 0];
            let constraints = BeginnerGenerationConstraintsV1 {
                skeleton_segments: vec![
                    skeleton(10, -100, -100, 100, -100),
                    skeleton(20, 100, -100, 100, 100),
                ],
                protrusions: vec![target],
                ..BeginnerGenerationConstraintsV1::default()
            };
            let vertical = count == 4;
            let candidates = parameterized_symmetric_endpoints(&constraints, count, vertical)
                .expect("legacy bilateral candidates");
            assert_eq!(
                parameterized_symmetric_endpoints_for_target(
                    &constraints.protrusions[0],
                    &constraints.skeleton_segments,
                    count,
                    vertical,
                ),
                Some(candidates)
            );
            let buffered =
                try_endpoint_candidates_v1(candidates).expect("bounded legacy candidate buffer");
            assert_eq!(buffered.len(), 4);
            assert_eq!(buffered.as_slice(), candidates.as_slice());

            let mut width_overflow = constraints;
            width_overflow.protrusions[0].root_width_tenths_mm = Some(u32::MAX);
            width_overflow.protrusions[0].tip_width_tenths_mm = Some(u32::MAX);
            assert!(parameterized_symmetric_endpoints(&width_overflow, count, vertical).is_none());
        }

        let axis_overflow = BeginnerGenerationConstraintsV1 {
            skeleton_segments: vec![
                skeleton(10, 1, 0, i32::MAX, 0),
                skeleton(20, i32::MAX, 0, i32::MAX, 100),
            ],
            protrusions: vec![bilateral_protrusion(1, 2)],
            ..BeginnerGenerationConstraintsV1::default()
        };
        assert!(parameterized_symmetric_endpoints(&axis_overflow, 2, false).is_none());

        let span_overflow = BeginnerGenerationConstraintsV1 {
            skeleton_segments: vec![
                skeleton(10, i32::MIN, 0, i32::MAX, 0),
                skeleton(20, i32::MAX, 0, i32::MAX, 100),
            ],
            protrusions: vec![bilateral_protrusion(1, 2)],
            ..BeginnerGenerationConstraintsV1::default()
        };
        assert!(parameterized_symmetric_endpoints(&span_overflow, 2, false).is_none());
    }

    #[test]
    fn bounded_tree_closed_segment_intersection_preserves_only_endpoint_contact() {
        let horizontal = skeleton(1, 0, 0, 10, 0);
        assert!(!bounded_tree_segments_intersect_beyond_shared_endpoint_v1(
            &horizontal,
            &skeleton(2, 0, 0, 0, 10),
        ));
        assert!(!bounded_tree_segments_intersect_beyond_shared_endpoint_v1(
            &horizontal,
            &skeleton(2, 10, 0, 20, 0),
        ));
        assert!(bounded_tree_segments_intersect_beyond_shared_endpoint_v1(
            &horizontal,
            &skeleton(2, 0, 0, 5, 0),
        ));
        assert!(bounded_tree_segments_intersect_beyond_shared_endpoint_v1(
            &horizontal,
            &skeleton(2, 5, 0, 0, 0),
        ));
        assert!(bounded_tree_segments_intersect_beyond_shared_endpoint_v1(
            &horizontal,
            &skeleton(2, 5, 0, 15, 0),
        ));
        assert!(bounded_tree_segments_intersect_beyond_shared_endpoint_v1(
            &skeleton(1, -10, 0, 10, 0),
            &skeleton(2, 0, -10, 0, 10),
        ));
        assert!(bounded_tree_segments_intersect_beyond_shared_endpoint_v1(
            &horizontal,
            &skeleton(2, 5, 0, 5, 10),
        ));
        assert!(!bounded_tree_segments_intersect_beyond_shared_endpoint_v1(
            &horizontal,
            &skeleton(2, 20, 0, 30, 0),
        ));

        let overlap = BeginnerGenerationConstraintsV1 {
            skeleton_segments: vec![
                skeleton(10, 0, 0, 100, 0),
                skeleton(20, 0, 0, 50, 0),
                skeleton(30, 0, 0, 0, 100),
            ],
            protrusions: vec![single_protrusion(1, [0, 50, 0], [0, 1_000, 0])],
            ..BeginnerGenerationConstraintsV1::default()
        };
        assert!(bounded_tree_skeleton_length_ratios(&overlap.skeleton_segments).is_some());
        assert!(!bounded_generic_tree_graph_is_supported_v1(&overlap));

        let separated_collinear = BeginnerGenerationConstraintsV1 {
            skeleton_segments: vec![
                skeleton(10, 0, 0, 10, 0),
                skeleton(20, 10, 0, 10, 10),
                skeleton(30, 10, 10, 20, 10),
                skeleton(40, 20, 10, 20, 0),
                skeleton(50, 20, 0, 30, 0),
            ],
            protrusions: vec![single_protrusion(1, [15, 5, 0], [0, 1_000, 0])],
            ..BeginnerGenerationConstraintsV1::default()
        };
        assert!(bounded_generic_tree_graph_is_supported_v1(
            &separated_collinear
        ));
    }

    #[test]
    fn generic_insect_plan_includes_tree_topology_and_rejects_crossing_bars() {
        let namespace = ProjectId::schema_namespace([0x68; 16]);
        let (ids, source) = square_source(namespace);
        let constraints = BeginnerGenerationConstraintsV1 {
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
                    kind: BeginnerTargetPartKindV1::Tail,
                    count: 1,
                },
                BeginnerTargetPartRecordV1 {
                    kind: BeginnerTargetPartKindV1::Fin,
                    count: 1,
                },
            ],
            skeleton_segments: vec![
                skeleton(10, -100, -100, 100, -100),
                skeleton(20, 100, -100, 100, 100),
            ],
            protrusions: vec![
                single_protrusion(1, [0, -50, 0], [0, 1_000, 0]),
                single_protrusion(2, [50, 50, 0], [1_000, 0, 0]),
            ],
            ..BeginnerGenerationConstraintsV1::default()
        };
        assert!(crate::validate_beginner_generation_constraints_v1(
            &constraints
        ));
        assert!(beginner_uses_bounded_generic_target_base_v1(&constraints));
        assert_eq!(beginner_target_approximation_score_v1(&constraints), 92);
        let generated = generate_beginner_plans_v1(namespace, &source, &ids, &constraints).unwrap();
        assert_eq!(
            generated[0].kind,
            BeginnerGeneratedPlanKindV1::CompositeGenericTargetBase
        );
        assert_eq!(radial_corner_support_added(&generated[0]), 4);
        assert_eq!(generated[0].crease_pattern.edges.len(), 8);
        assert!(
            generated[0].crease_pattern.edges[..4]
                .iter()
                .all(|edge| ids.contains(&edge.end)),
            "the four canonical paper-corner supports must prefix semantic feature rays"
        );
        assert!(
            generated[0].crease_pattern.edges[4..6].iter().all(|edge| {
                matches!(edge.kind, EdgeKind::Mountain | EdgeKind::Valley)
                    && !ids.contains(&edge.end)
            }),
            "the two semantic feature rays must follow the support prefix"
        );
        assert!(
            generated[0].crease_pattern.edges[6..]
                .iter()
                .all(|edge| edge.kind == EdgeKind::Auxiliary),
            "the two bounded-tree bars must remain the final auxiliary suffix"
        );
        assert_eq!(generated[0].instruction_codes.len(), 3);
        assert_eq!(
            generated[0].instruction_codes[1],
            "bounded_radial_corner_support_v1:added=4:covered=4"
        );
        assert_eq!(
            generated[0].instruction_codes[2],
            "bounded_tree_branch_topology_v1:nodes=3:leaves=2:bars=2"
        );

        for allowed_techniques in [
            vec![BeginnerFoldTechniqueV1::ValleyFold],
            vec![BeginnerFoldTechniqueV1::MountainFold],
            vec![
                BeginnerFoldTechniqueV1::ValleyFold,
                BeginnerFoldTechniqueV1::MountainFold,
            ],
        ] {
            let mut technique_case = constraints.clone();
            technique_case.allowed_techniques = allowed_techniques;
            let technique_plans =
                generate_beginner_plans_v1(namespace, &source, &ids, &technique_case).unwrap();
            assert_eq!(radial_corner_support_added(&technique_plans[0]), 4);
            assert!(
                technique_plans[0].crease_pattern.edges[..6]
                    .iter()
                    .all(|edge| matches!(edge.kind, EdgeKind::Mountain | EdgeKind::Valley)),
                "support-prefixed count-two features are the complete physical hinge prefix"
            );
            assert_eq!(
                technique_plans[0].crease_pattern.edges[6..]
                    .iter()
                    .map(|edge| edge.kind)
                    .collect::<Vec<_>>(),
                [EdgeKind::Auxiliary, EdgeKind::Auxiliary],
                "skeleton bars are topology/provenance guides, not extra physical hinges"
            );
        }
        let mut no_tree_fold = constraints.clone();
        no_tree_fold.allowed_techniques = vec![BeginnerFoldTechniqueV1::InsideReverseFold];
        assert!(
            bounded_generic_tree_graph_is_supported_v1(&no_tree_fold),
            "tree geometry support is independent of the physical fold-technique preflight"
        );
        assert_eq!(
            generate_beginner_plans_v1(namespace, &source, &ids, &no_tree_fold),
            Err(BeginnerGeneratorErrorV1::UnsupportedTechniques)
        );

        let mut crossing = constraints;
        crossing.skeleton_segments = vec![
            skeleton(10, -100, 0, 100, 0),
            skeleton(20, 0, -100, 0, 100),
            skeleton(30, 100, 0, 0, 100),
        ];
        assert!(crate::validate_beginner_generation_constraints_v1(
            &crossing
        ));
        assert!(bounded_generic_composite_endpoints(&crossing).is_some());
        assert!(!bounded_generic_tree_graph_is_supported_v1(&crossing));
        assert_eq!(beginner_target_approximation_score_v1(&crossing), 0);
        assert_eq!(
            generate_beginner_plans_v1(namespace, &source, &ids, &crossing),
            Err(BeginnerGeneratorErrorV1::UnsupportedInsectTemplate)
        );
    }

    #[test]
    fn generic_tree_score_and_error_category_share_the_generation_preflight() {
        let namespace = ProjectId::schema_namespace([0x69; 16]);
        let (ids, source) = square_source(namespace);
        let mut custom = BeginnerGenerationConstraintsV1 {
            target_category: Some(BeginnerTargetCategoryV1::CustomObject),
            skeleton_segments: vec![
                skeleton(10, -100, -100, 100, -100),
                skeleton(20, 100, -100, 100, 100),
            ],
            protrusions: vec![
                single_protrusion(1, [0, -50, 0], [0, 1_000, 0]),
                single_protrusion(2, [50, 50, 0], [1_000, 0, 0]),
            ],
            ..BeginnerGenerationConstraintsV1::default()
        };
        assert!(crate::validate_beginner_generation_constraints_v1(&custom));
        assert_eq!(beginner_target_approximation_score_v1(&custom), 92);
        assert!(generate_beginner_plans_v1(namespace, &source, &ids, &custom).is_ok());

        let mut animal = custom.clone();
        animal.target_category = Some(BeginnerTargetCategoryV1::Animal);
        animal.target_parts = vec![
            BeginnerTargetPartRecordV1 {
                kind: BeginnerTargetPartKindV1::Head,
                count: 1,
            },
            BeginnerTargetPartRecordV1 {
                kind: BeginnerTargetPartKindV1::Torso,
                count: 1,
            },
            BeginnerTargetPartRecordV1 {
                kind: BeginnerTargetPartKindV1::Tail,
                count: 1,
            },
            BeginnerTargetPartRecordV1 {
                kind: BeginnerTargetPartKindV1::Fin,
                count: 1,
            },
        ];
        assert!(crate::validate_beginner_generation_constraints_v1(&animal));
        assert!(beginner_uses_bounded_generic_target_base_v1(&animal));
        assert_eq!(beginner_target_approximation_score_v1(&animal), 92);
        assert_eq!(
            generate_beginner_plans_v1(namespace, &source, &ids, &animal).unwrap()[0].kind,
            BeginnerGeneratedPlanKindV1::CompositeGenericTargetBase
        );

        custom.skeleton_segments = vec![
            skeleton(10, -100, 0, 100, 0),
            skeleton(20, 0, -100, 0, 100),
            skeleton(30, 100, 0, 0, 100),
        ];
        assert!(bounded_generic_composite_endpoints(&custom).is_some());
        assert!(!bounded_generic_tree_graph_is_supported_v1(&custom));
        assert_eq!(beginner_target_approximation_score_v1(&custom), 0);
        assert_eq!(
            generate_beginner_plans_v1(namespace, &source, &ids, &custom),
            Err(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate)
        );

        animal.skeleton_segments = custom.skeleton_segments;
        assert!(crate::validate_beginner_generation_constraints_v1(&animal));
        assert_eq!(beginner_target_approximation_score_v1(&animal), 0);
        assert_eq!(
            generate_beginner_plans_v1(namespace, &source, &ids, &animal),
            Err(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate)
        );
    }

    #[test]
    fn detail_bonuses_do_not_revive_an_unsupported_zero_base_score() {
        let namespace = ProjectId::schema_namespace([0x6b; 16]);
        let (ids, source) = square_source(namespace);
        let constraints = BeginnerGenerationConstraintsV1 {
            generic_body_outline_tenths_mm: Some(vec![[-2, -1], [-1, 2], [1, 2], [2, -1], [0, -2]]),
            ..BeginnerGenerationConstraintsV1::default()
        };
        assert!(crate::validate_beginner_generation_constraints_v1(
            &constraints
        ));
        assert_eq!(beginner_target_approximation_score_v1(&constraints), 0);
        assert_eq!(
            generate_beginner_plans_v1(namespace, &source, &ids, &constraints),
            Err(BeginnerGeneratorErrorV1::MissingTargetCategory)
        );
    }

    #[test]
    fn center_axis_single_templates_require_exactly_one_target() {
        let namespace = ProjectId::schema_namespace([0x6c; 16]);
        let (ids, source) = square_source(namespace);
        for (category, part_kind, vertical, expected_kind, expected_error) in [
            (
                BeginnerTargetCategoryV1::Animal,
                BeginnerTargetPartKindV1::Horn,
                true,
                BeginnerGeneratedPlanKindV1::CenterAxisHornBase,
                BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate,
            ),
            (
                BeginnerTargetCategoryV1::Animal,
                BeginnerTargetPartKindV1::Tail,
                false,
                BeginnerGeneratedPlanKindV1::CenterAxisTailBase,
                BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate,
            ),
            (
                BeginnerTargetCategoryV1::Insect,
                BeginnerTargetPartKindV1::Antenna,
                true,
                BeginnerGeneratedPlanKindV1::CenterAxisAntennaBase,
                BeginnerGeneratorErrorV1::UnsupportedInsectTemplate,
            ),
        ] {
            let mut target = bilateral_protrusion(1, 1);
            target.symmetry = BeginnerProtrusionSymmetryV1::None;
            target.direction_milli = if vertical {
                [0, -1_000, 0]
            } else {
                [1_000, 0, 0]
            };
            let mut constraints = BeginnerGenerationConstraintsV1 {
                target_category: Some(category),
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
                        kind: part_kind,
                        count: 1,
                    },
                ],
                skeleton_segments: vec![
                    skeleton(1, -10, 0, 0, 10),
                    skeleton(2, 10, 0, 0, 10),
                    skeleton(3, 0, -10, 0, 10),
                ],
                protrusions: vec![target.clone()],
                ..BeginnerGenerationConstraintsV1::default()
            };
            assert!(crate::validate_beginner_generation_constraints_v1(
                &constraints
            ));
            assert_eq!(beginner_target_approximation_score_v1(&constraints), 92);
            assert_eq!(
                generate_beginner_plans_v1(namespace, &source, &ids, &constraints).unwrap()[0].kind,
                expected_kind
            );

            let mut contained_outline = constraints.clone();
            contained_outline.protrusions[0].local_outline_tenths_mm =
                Some(vec![[-1, -1], [1, -1], [0, 2]]);
            assert!(crate::validate_beginner_generation_constraints_v1(
                &contained_outline
            ));
            assert_eq!(
                beginner_target_approximation_score_v1(&contained_outline),
                92
            );
            assert_eq!(
                generate_beginner_plans_v1(namespace, &source, &ids, &contained_outline).unwrap()
                    [0]
                .kind,
                expected_kind
            );
            let mut outside_outline = constraints.clone();
            outside_outline.protrusions[0].local_outline_tenths_mm =
                Some(vec![[-1, -1], [11, -1], [0, 2]]);
            assert!(crate::validate_beginner_generation_constraints_v1(
                &outside_outline
            ));
            assert_eq!(beginner_target_approximation_score_v1(&outside_outline), 0);
            assert_eq!(
                generate_beginner_plans_v1(namespace, &source, &ids, &outside_outline),
                Err(expected_error)
            );
            let mut outside_root = constraints.clone();
            outside_root.protrusions[0].position_tenths_mm[1] = 11;
            assert!(crate::validate_beginner_generation_constraints_v1(
                &outside_root
            ));
            assert_eq!(beginner_target_approximation_score_v1(&outside_root), 0);
            assert_eq!(
                generate_beginner_plans_v1(namespace, &source, &ids, &outside_root),
                Err(expected_error)
            );

            let mut extra = target;
            extra.id = 2;
            extra.priority = 100;
            constraints.protrusions.push(extra);
            assert!(crate::validate_beginner_generation_constraints_v1(
                &constraints
            ));
            assert_eq!(beginner_target_approximation_score_v1(&constraints), 0);
            assert_eq!(
                generate_beginner_plans_v1(namespace, &source, &ids, &constraints),
                Err(expected_error)
            );
            constraints.protrusions.reverse();
            assert_eq!(beginner_target_approximation_score_v1(&constraints), 0);
            assert_eq!(
                generate_beginner_plans_v1(namespace, &source, &ids, &constraints),
                Err(expected_error)
            );

            constraints.protrusions.clear();
            assert_eq!(beginner_target_approximation_score_v1(&constraints), 75);
            assert_eq!(
                generate_beginner_plans_v1(namespace, &source, &ids, &constraints),
                Err(expected_error)
            );
        }
        let mut overflow = bilateral_protrusion(1, 1);
        overflow.position_tenths_mm[0] = i32::MAX;
        overflow.local_outline_tenths_mm = Some(vec![[1, 0], [0, 1], [-1, 0]]);
        assert!(!protrusion_local_outline_within_bounds_v1(
            &overflow,
            (i32::MIN, i32::MAX, i32::MIN, i32::MAX)
        ));
    }

    #[test]
    fn bilateral_single_templates_require_exactly_one_target() {
        let namespace = ProjectId::schema_namespace([0x6d; 16]);
        let (ids, source) = square_source(namespace);
        for (category, part_kind, count, expected_kind, expected_error, expected_empty_score) in [
            (
                BeginnerTargetCategoryV1::Animal,
                BeginnerTargetPartKindV1::Leg,
                4,
                BeginnerGeneratedPlanKindV1::SymmetricFourLegBase,
                BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate,
                72,
            ),
            (
                BeginnerTargetCategoryV1::Animal,
                BeginnerTargetPartKindV1::Wing,
                2,
                BeginnerGeneratedPlanKindV1::SymmetricBirdBase,
                BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate,
                75,
            ),
            (
                BeginnerTargetCategoryV1::Animal,
                BeginnerTargetPartKindV1::Fin,
                2,
                BeginnerGeneratedPlanKindV1::SymmetricFishBase,
                BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate,
                75,
            ),
            (
                BeginnerTargetCategoryV1::Animal,
                BeginnerTargetPartKindV1::Ear,
                2,
                BeginnerGeneratedPlanKindV1::SymmetricEarBase,
                BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate,
                75,
            ),
            (
                BeginnerTargetCategoryV1::Animal,
                BeginnerTargetPartKindV1::Horn,
                2,
                BeginnerGeneratedPlanKindV1::SymmetricHornBase,
                BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate,
                75,
            ),
            (
                BeginnerTargetCategoryV1::Insect,
                BeginnerTargetPartKindV1::Wing,
                2,
                BeginnerGeneratedPlanKindV1::SymmetricWingBase,
                BeginnerGeneratorErrorV1::UnsupportedInsectTemplate,
                75,
            ),
            (
                BeginnerTargetCategoryV1::Insect,
                BeginnerTargetPartKindV1::Antenna,
                2,
                BeginnerGeneratedPlanKindV1::SymmetricAntennaBase,
                BeginnerGeneratorErrorV1::UnsupportedInsectTemplate,
                75,
            ),
            (
                BeginnerTargetCategoryV1::Insect,
                BeginnerTargetPartKindV1::Leg,
                2,
                BeginnerGeneratedPlanKindV1::SymmetricInsectLegPairBase,
                BeginnerGeneratorErrorV1::UnsupportedInsectTemplate,
                75,
            ),
        ] {
            let target = bilateral_protrusion(1, count);
            let mut constraints = BeginnerGenerationConstraintsV1 {
                target_category: Some(category),
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
                        kind: part_kind,
                        count,
                    },
                ],
                skeleton_segments: vec![
                    skeleton(1, -10, 0, 0, 10),
                    skeleton(2, 10, 0, 0, 10),
                    skeleton(3, 0, -10, 0, 10),
                ],
                protrusions: vec![target.clone()],
                ..BeginnerGenerationConstraintsV1::default()
            };
            assert!(crate::validate_beginner_generation_constraints_v1(
                &constraints
            ));
            assert_eq!(beginner_target_approximation_score_v1(&constraints), 92);
            assert_eq!(
                generate_beginner_plans_v1(namespace, &source, &ids, &constraints).unwrap()[0].kind,
                expected_kind
            );

            let mut contained_outline = constraints.clone();
            contained_outline.protrusions[0].local_outline_tenths_mm =
                Some(vec![[-2, -1], [2, -1], [2, 1], [-2, 1]]);
            assert!(crate::validate_beginner_generation_constraints_v1(
                &contained_outline
            ));
            assert_eq!(
                beginner_target_approximation_score_v1(&contained_outline),
                93
            );
            assert_eq!(
                generate_beginner_plans_v1(namespace, &source, &ids, &contained_outline).unwrap()
                    [0]
                .kind,
                expected_kind
            );
            let mut outside_outline = constraints.clone();
            outside_outline.protrusions[0].local_outline_tenths_mm =
                Some(vec![[-11, -1], [11, -1], [11, 1], [-11, 1]]);
            assert!(crate::validate_beginner_generation_constraints_v1(
                &outside_outline
            ));
            assert_eq!(beginner_target_approximation_score_v1(&outside_outline), 0);
            assert_eq!(
                generate_beginner_plans_v1(namespace, &source, &ids, &outside_outline),
                Err(expected_error)
            );
            let mut contained_body_outline = constraints.clone();
            contained_body_outline.generic_body_outline_tenths_mm =
                Some(vec![[-5, -5], [-5, 5], [5, 5], [5, -5]]);
            assert!(crate::validate_beginner_generation_constraints_v1(
                &contained_body_outline
            ));
            assert_eq!(
                beginner_target_approximation_score_v1(&contained_body_outline),
                92
            );
            assert_eq!(
                generate_beginner_plans_v1(namespace, &source, &ids, &contained_body_outline)
                    .unwrap()[0]
                    .kind,
                expected_kind
            );
            let mut outside_body_outline = constraints.clone();
            outside_body_outline.generic_body_outline_tenths_mm =
                Some(vec![[-20, -20], [-20, 20], [20, 20], [20, -20]]);
            assert!(crate::validate_beginner_generation_constraints_v1(
                &outside_body_outline
            ));
            assert_eq!(
                beginner_target_approximation_score_v1(&outside_body_outline),
                0
            );
            assert_eq!(
                generate_beginner_plans_v1(namespace, &source, &ids, &outside_body_outline),
                Err(expected_error)
            );

            let mut extra = target;
            extra.id = 2;
            extra.priority = 100;
            constraints.protrusions.push(extra);
            assert!(crate::validate_beginner_generation_constraints_v1(
                &constraints
            ));
            assert_eq!(beginner_target_approximation_score_v1(&constraints), 0);
            assert_eq!(
                generate_beginner_plans_v1(namespace, &source, &ids, &constraints),
                Err(expected_error)
            );
            constraints.protrusions.reverse();
            assert_eq!(beginner_target_approximation_score_v1(&constraints), 0);
            assert_eq!(
                generate_beginner_plans_v1(namespace, &source, &ids, &constraints),
                Err(expected_error)
            );

            constraints.protrusions.clear();
            assert_eq!(
                beginner_target_approximation_score_v1(&constraints),
                expected_empty_score
            );
            assert_eq!(
                generate_beginner_plans_v1(namespace, &source, &ids, &constraints),
                Err(expected_error)
            );
        }
    }

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
        assert_eq!(radial_corner_support_added(&first[0]), 4);
        assert_eq!(first[0].crease_pattern.edges.len(), 8);
        assert!(
            first[0].crease_pattern.edges[..4]
                .iter()
                .all(|edge| ids.contains(&edge.end)),
            "the four deterministic paper-corner support creases must prefix the four semantic leg rays"
        );
        assert!(
            first[0].crease_pattern.edges[4..]
                .iter()
                .all(|edge| !ids.contains(&edge.end)),
            "the semantic four-leg target remains the exact four-edge suffix"
        );
        assert!(
            first[0]
                .crease_pattern
                .edges
                .iter()
                .all(|edge| edge.kind == EdgeKind::Valley),
            "the default valley-first choice applies uniformly to semantic and support creases"
        );
        for (technique, expected_kind) in [
            (BeginnerFoldTechniqueV1::ValleyFold, EdgeKind::Valley),
            (BeginnerFoldTechniqueV1::MountainFold, EdgeKind::Mountain),
        ] {
            let mut single_technique = constraints.clone();
            single_technique.allowed_techniques = vec![technique];
            let plan =
                generate_beginner_plans_v1(namespace, &source, &ids, &single_technique).unwrap();
            assert_eq!(radial_corner_support_added(&plan[0]), 4);
            assert_eq!(plan[0].crease_pattern.edges.len(), 8);
            assert!(
                plan[0]
                    .crease_pattern
                    .edges
                    .iter()
                    .all(|edge| edge.kind == expected_kind),
                "the symmetric four-leg template must honor its sole allowed fold technique"
            );
        }
        assert!(
            first[1..]
                .iter()
                .all(|plan| plan.crease_pattern.edges.len() == 1)
        );
        let mut missing_head = constraints.clone();
        missing_head
            .target_parts
            .retain(|part| part.kind != BeginnerTargetPartKindV1::Head);
        assert!(crate::validate_beginner_generation_constraints_v1(
            &missing_head
        ));
        assert_eq!(beginner_target_approximation_score_v1(&missing_head), 0);
        assert_eq!(
            generate_beginner_plans_v1(namespace, &source, &ids, &missing_head),
            Err(BeginnerGeneratorErrorV1::MissingRequiredParts)
        );
        let mut unsupported_techniques = constraints.clone();
        unsupported_techniques.allowed_techniques =
            vec![BeginnerFoldTechniqueV1::InsideReverseFold];
        assert!(crate::validate_beginner_generation_constraints_v1(
            &unsupported_techniques
        ));
        assert_eq!(
            beginner_target_approximation_score_v1(&unsupported_techniques),
            0
        );
        assert_eq!(
            generate_beginner_plans_v1(namespace, &source, &ids, &unsupported_techniques),
            Err(BeginnerGeneratorErrorV1::UnsupportedTechniques)
        );
        let mut duplicate_techniques = constraints.clone();
        duplicate_techniques
            .allowed_techniques
            .push(BeginnerFoldTechniqueV1::ValleyFold);
        assert!(!crate::validate_beginner_generation_constraints_v1(
            &duplicate_techniques
        ));
        assert_eq!(
            beginner_target_approximation_score_v1(&duplicate_techniques),
            0
        );
        assert_eq!(
            generate_beginner_plans_v1(namespace, &source, &ids, &duplicate_techniques),
            Err(BeginnerGeneratorErrorV1::UnsupportedTechniques)
        );

        let mut invalid_constraints = constraints.clone();
        invalid_constraints.skeleton_segments[1].id = invalid_constraints.skeleton_segments[0].id;
        assert!(!crate::validate_beginner_generation_constraints_v1(
            &invalid_constraints
        ));
        assert_eq!(
            beginner_target_approximation_score_v1(&invalid_constraints),
            0
        );
        assert_eq!(
            generate_beginner_plans_v1(namespace, &source, &ids, &invalid_constraints),
            Err(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate)
        );
        let mut invalid_insect = invalid_constraints.clone();
        invalid_insect.target_category = Some(BeginnerTargetCategoryV1::Insect);
        assert_eq!(
            generate_beginner_plans_v1(namespace, &source, &ids, &invalid_insect),
            Err(BeginnerGeneratorErrorV1::UnsupportedInsectTemplate)
        );
        let mut invalid_custom = invalid_constraints.clone();
        invalid_custom.target_category = Some(BeginnerTargetCategoryV1::CustomObject);
        assert_eq!(
            generate_beginner_plans_v1(namespace, &source, &ids, &invalid_custom),
            Err(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate)
        );
        let mut invalid_without_category = invalid_constraints.clone();
        invalid_without_category.target_category = None;
        assert_eq!(
            generate_beginner_plans_v1(namespace, &source, &ids, &invalid_without_category),
            Err(BeginnerGeneratorErrorV1::MissingTargetCategory)
        );
        let mut invalid_without_head = invalid_constraints.clone();
        invalid_without_head
            .target_parts
            .retain(|part| part.kind != BeginnerTargetPartKindV1::Head);
        assert_eq!(
            generate_beginner_plans_v1(namespace, &source, &ids, &invalid_without_head),
            Err(BeginnerGeneratorErrorV1::MissingRequiredParts)
        );
        assert_eq!(
            generate_beginner_plans_v1(namespace, &source, &ids[..3], &invalid_constraints),
            Err(BeginnerGeneratorErrorV1::UnsupportedPaper)
        );
        let mut short_leg_skeleton = constraints.clone();
        short_leg_skeleton.skeleton_segments.truncate(2);
        assert!(has_bilateral_skeleton(&short_leg_skeleton));
        assert_eq!(
            beginner_target_approximation_score_v1(&short_leg_skeleton),
            0
        );
        assert_eq!(
            generate_beginner_plans_v1(namespace, &source, &ids, &short_leg_skeleton),
            Err(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate)
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
            let mut non_bilateral = family.clone();
            non_bilateral.skeleton_segments[1].end.y_tenths_mm += 1;
            assert!(crate::validate_beginner_generation_constraints_v1(
                &non_bilateral
            ));
            assert_eq!(beginner_target_approximation_score_v1(&non_bilateral), 0);
            assert_eq!(
                generate_beginner_plans_v1(namespace, &source, &ids, &non_bilateral),
                Err(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate)
            );
        }
        let mut general_animal_family = constraints.clone();
        general_animal_family.target_parts[2] = BeginnerTargetPartRecordV1 {
            kind: BeginnerTargetPartKindV1::Antenna,
            count: 2,
        };
        general_animal_family.skeleton_segments.truncate(2);
        general_animal_family.protrusions[0] = bilateral_protrusion(1, 2);
        assert!(crate::validate_beginner_generation_constraints_v1(
            &general_animal_family
        ));
        let general_plans =
            generate_beginner_plans_v1(namespace, &source, &ids, &general_animal_family).unwrap();
        assert_eq!(
            general_plans[0].kind,
            BeginnerGeneratedPlanKindV1::CompositeGenericTargetBase
        );
        assert_eq!(radial_corner_support_added(&general_plans[0]), 4);
        assert_eq!(general_plans[0].crease_pattern.edges.len(), 8);
        assert!(
            general_plans[0].crease_pattern.edges[..4]
                .iter()
                .all(|edge| ids.contains(&edge.end)),
            "general count-two support must occupy the four-edge paper-corner prefix"
        );
        assert!(
            general_plans[0].crease_pattern.edges[4..6]
                .iter()
                .all(|edge| {
                    matches!(edge.kind, EdgeKind::Mountain | EdgeKind::Valley)
                        && !ids.contains(&edge.end)
                }),
            "both semantic antenna rays must follow the support prefix"
        );
        assert!(
            general_plans[0].crease_pattern.edges[6..]
                .iter()
                .all(|edge| edge.kind == EdgeKind::Auxiliary),
            "the bounded-tree bars must remain the final auxiliary suffix"
        );
        assert!(
            general_plans[0]
                .instruction_codes
                .iter()
                .any(|code| code == "bounded_radial_corner_support_v1:added=4:covered=4")
        );
        assert_eq!(
            beginner_target_approximation_score_v1(&general_animal_family),
            92
        );
        let mut duplicate_general = general_animal_family;
        duplicate_general
            .target_parts
            .push(BeginnerTargetPartRecordV1 {
                kind: BeginnerTargetPartKindV1::Antenna,
                count: 2,
            });
        assert!(crate::validate_beginner_generation_constraints_v1(
            &duplicate_general
        ));
        assert_eq!(
            beginner_expected_generated_plan_kind_v1(&duplicate_general),
            None
        );
        assert_eq!(
            beginner_target_approximation_score_v1(&duplicate_general),
            0
        );
        assert_eq!(
            generate_beginner_plans_v1(namespace, &source, &ids, &duplicate_general),
            Err(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate)
        );
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
        assert_eq!(beginner_target_approximation_score_v1(&composite), 92);
        let mut oversized_tail_ears = composite.clone();
        oversized_tail_ears
            .protrusions
            .push(bilateral_protrusion(3, 4));
        assert_eq!(animal_tail_ear_bindings_v1(&oversized_tail_ears), None);
        assert_eq!(
            beginner_target_approximation_score_v1(&oversized_tail_ears),
            0
        );
        assert_eq!(
            generate_beginner_plans_v1(namespace, &source, &ids, &oversized_tail_ears),
            Err(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate)
        );
        let mut invalid_tail_ears = composite.clone();
        invalid_tail_ears.protrusions[1].position_tenths_mm[0] = 1;
        assert!(animal_tail_ear_bindings_v1(&invalid_tail_ears).is_some());
        assert_eq!(
            beginner_target_approximation_score_v1(&invalid_tail_ears),
            0
        );
        assert_eq!(
            generate_beginner_plans_v1(namespace, &source, &ids, &invalid_tail_ears),
            Err(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate)
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
        assert_eq!(beginner_target_approximation_score_v1(&horn_tail), 92);
        let mut oversized_horn_tail = horn_tail.clone();
        oversized_horn_tail
            .protrusions
            .push(bilateral_protrusion(3, 4));
        assert_eq!(animal_horn_tail_bindings_v1(&oversized_horn_tail), None);
        assert_eq!(
            beginner_target_approximation_score_v1(&oversized_horn_tail),
            0
        );
        assert_eq!(
            generate_beginner_plans_v1(namespace, &source, &ids, &oversized_horn_tail),
            Err(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate)
        );
        let mut reversed_horn_tail = horn_tail.clone();
        reversed_horn_tail.protrusions.reverse();
        assert_eq!(
            beginner_target_approximation_score_v1(&reversed_horn_tail),
            92
        );
        assert_eq!(
            generate_beginner_plans_v1(namespace, &source, &ids, &reversed_horn_tail).unwrap(),
            horn_tail_plans
        );
        let mut invalid_tail = horn_tail.clone();
        invalid_tail.protrusions[1].position_tenths_mm[0] = 1;
        invalid_tail.protrusions[1].local_outline_tenths_mm =
            Some(vec![[-1, -1], [1, -1], [1, 1], [-1, 1]]);
        assert!(crate::validate_beginner_generation_constraints_v1(
            &invalid_tail
        ));
        assert!(animal_horn_tail_bindings_v1(&invalid_tail).is_some());
        assert_eq!(beginner_target_approximation_score_v1(&invalid_tail), 0);
        assert_eq!(
            generate_beginner_plans_v1(namespace, &source, &ids, &invalid_tail),
            Err(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate)
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
        let triple_supports = radial_corner_support_added(&triple_plans[0]);
        assert_eq!(
            triple_plans[0].crease_pattern.vertices.len(),
            7 + triple_supports
        );
        assert_eq!(
            triple_plans[0].crease_pattern.edges.len(),
            6 + triple_supports
        );
        assert_eq!(
            animal_horn_tail_ear_bindings_v1(&triple),
            Some(BeginnerHornTailEarBindingV1 {
                horn_protrusion_id: 1,
                tail_protrusion_id: 2,
                ear_pair_protrusion_id: 3,
            })
        );
        assert_eq!(beginner_target_approximation_score_v1(&triple), 92);
        let mut oversized_triple = triple.clone();
        oversized_triple
            .protrusions
            .push(bilateral_protrusion(4, 4));
        assert!(animal_horn_tail_ear_bindings_v1(&oversized_triple).is_some());
        assert_eq!(
            animal_standalone_horn_tail_ear_bindings_v1(&oversized_triple),
            None
        );
        assert_eq!(beginner_target_approximation_score_v1(&oversized_triple), 0);
        assert_eq!(
            generate_beginner_plans_v1(namespace, &source, &ids, &oversized_triple),
            Err(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate)
        );
        let mut invalid_ears = triple.clone();
        invalid_ears.protrusions[2].position_tenths_mm[0] = 1;
        assert!(animal_horn_tail_ear_bindings_v1(&invalid_ears).is_some());
        assert_eq!(beginner_target_approximation_score_v1(&invalid_ears), 0);
        assert_eq!(
            generate_beginner_plans_v1(namespace, &source, &ids, &invalid_ears),
            Err(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate)
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
        let complete_animal_supports = radial_corner_support_added(&complete_animal_plans[0]);
        assert_eq!(
            complete_animal_plans[0].crease_pattern.vertices.len(),
            11 + complete_animal_supports
        );
        assert_eq!(
            complete_animal_plans[0].crease_pattern.edges.len(),
            10 + complete_animal_supports
        );
        assert_eq!(
            animal_complete_bindings_v1(&complete_animal),
            Some(BeginnerCompleteAnimalBindingV1 {
                horn_protrusion_id: 1,
                tail_protrusion_id: 2,
                ear_pair_protrusion_id: 3,
                leg_protrusion_id: 4,
            })
        );
        assert_eq!(beginner_target_approximation_score_v1(&complete_animal), 92);
        let mut invalid_legs = complete_animal.clone();
        invalid_legs.protrusions[3].position_tenths_mm[0] = 1;
        assert!(animal_complete_bindings_v1(&invalid_legs).is_some());
        assert_eq!(beginner_target_approximation_score_v1(&invalid_legs), 0);
        assert_eq!(
            generate_beginner_plans_v1(namespace, &source, &ids, &invalid_legs),
            Err(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate)
        );
        let mut duplicate_leg = complete_animal.clone();
        duplicate_leg.protrusions.push(legs);
        assert_eq!(animal_complete_bindings_v1(&duplicate_leg), None);
        assert_eq!(beginner_target_approximation_score_v1(&duplicate_leg), 0);
        assert_eq!(
            generate_beginner_plans_v1(namespace, &source, &ids, &duplicate_leg),
            Err(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate)
        );
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
        let winged_supports = radial_corner_support_added(&winged_plans[0]);
        assert_eq!(
            winged_plans[0].crease_pattern.vertices.len(),
            11 + winged_supports
        );
        assert_eq!(
            winged_plans[0].crease_pattern.edges.len(),
            10 + winged_supports
        );
        let semantic_endpoints = winged_plans[0].crease_pattern.edges
            [winged_supports..winged_supports + 10]
            .iter()
            .map(|edge| {
                let endpoint_id = if winged_plans[0]
                    .crease_pattern
                    .vertices
                    .iter()
                    .find(|vertex| vertex.id == edge.start)
                    .is_some_and(|vertex| vertex.position == Point2::new(5.0, 5.0))
                {
                    edge.end
                } else {
                    edge.start
                };
                winged_plans[0]
                    .crease_pattern
                    .vertices
                    .iter()
                    .find(|vertex| vertex.id == endpoint_id)
                    .expect("winged semantic endpoint")
                    .position
            })
            .collect::<Vec<_>>();
        assert!(
            semantic_endpoints.iter().enumerate().all(|(index, point)| {
                semantic_endpoints
                    .iter()
                    .skip(index + 1)
                    .all(|other| point != other)
            }),
            "all ten winged-animal semantic rays must reach distinct boundary positions"
        );
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
        assert_eq!(beginner_target_approximation_score_v1(&winged_animal), 92);
        let mut collapsed_semantic_rays = winged_animal.clone();
        collapsed_semantic_rays.protrusions[2].position_tenths_mm[1] = -4;
        collapsed_semantic_rays.protrusions[4].position_tenths_mm[1] = 4;
        collapsed_semantic_rays.protrusions[4].priority =
            collapsed_semantic_rays.protrusions[2].priority;
        assert!(crate::validate_beginner_generation_constraints_v1(
            &collapsed_semantic_rays
        ));
        assert_eq!(
            generate_beginner_plans_v1(namespace, &source, &ids, &collapsed_semantic_rays),
            Err(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate),
            "meaning-distinct ear and wing IDs must not authorize coincident boundary rays"
        );
        let mut invalid_wings = winged_animal.clone();
        invalid_wings.protrusions[4].position_tenths_mm[0] = 1;
        assert!(animal_complete_winged_bindings_v1(&invalid_wings).is_some());
        assert_eq!(beginner_target_approximation_score_v1(&invalid_wings), 0);
        assert_eq!(
            generate_beginner_plans_v1(namespace, &source, &ids, &invalid_wings),
            Err(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate)
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
        assert_eq!(beginner_target_approximation_score_v1(&horn_ear), 92);
        let mut oversized_horn_ears = horn_ear.clone();
        oversized_horn_ears
            .protrusions
            .push(bilateral_protrusion(3, 4));
        assert_eq!(animal_horn_ear_bindings_v1(&oversized_horn_ears), None);
        assert_eq!(
            beginner_target_approximation_score_v1(&oversized_horn_ears),
            0
        );
        assert_eq!(
            generate_beginner_plans_v1(namespace, &source, &ids, &oversized_horn_ears),
            Err(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate)
        );
        let mut invalid_horn_ears = horn_ear.clone();
        invalid_horn_ears.protrusions[1].position_tenths_mm[0] = 1;
        assert!(animal_horn_ear_bindings_v1(&invalid_horn_ears).is_some());
        assert_eq!(
            beginner_target_approximation_score_v1(&invalid_horn_ears),
            0
        );
        assert_eq!(
            generate_beginner_plans_v1(namespace, &source, &ids, &invalid_horn_ears),
            Err(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate)
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
        // A grouped bilateral count=2 contributes exactly two physical
        // endpoints; the former four-endpoint expansion double-counted it.
        let generic_supports = radial_corner_support_added(&generic_plans[0]);
        assert_eq!(
            generic_plans[0].crease_pattern.vertices.len(),
            11 + generic_supports
        );
        assert_eq!(
            generic_plans[0].crease_pattern.edges.len(),
            9 + generic_supports
        );
        for (position, direction) in [([10, 0, 0], [-1_000, 0, 0]), ([1, 10, 0], [0, -1_000, 0])] {
            let mut boundary_landmark_root = generic.clone();
            // Keep the compact semantic total synchronized with the physical
            // grouped records while changing the first group into one landmark.
            boundary_landmark_root.target_parts[2].count = 1;
            boundary_landmark_root.protrusions[0].count = 1;
            boundary_landmark_root.protrusions[0].symmetry = BeginnerProtrusionSymmetryV1::None;
            boundary_landmark_root.protrusions[0].position_tenths_mm = position;
            boundary_landmark_root.protrusions[0].direction_milli = direction;
            assert!(crate::validate_beginner_generation_constraints_v1(
                &boundary_landmark_root
            ));
            assert_eq!(
                beginner_target_approximation_score_v1(&boundary_landmark_root),
                92
            );
            assert_eq!(
                generate_beginner_plans_v1(namespace, &source, &ids, &boundary_landmark_root)
                    .unwrap()[0]
                    .kind,
                BeginnerGeneratedPlanKindV1::CompositeGenericTargetBase
            );
        }
        for (position, direction) in [([11, 0, 0], [-1_000, 0, 0]), ([1, 11, 0], [0, -1_000, 0])] {
            let mut outside_landmark_root = generic.clone();
            outside_landmark_root.target_parts[2].count = 1;
            outside_landmark_root.protrusions[0].count = 1;
            outside_landmark_root.protrusions[0].symmetry = BeginnerProtrusionSymmetryV1::None;
            outside_landmark_root.protrusions[0].position_tenths_mm = position;
            outside_landmark_root.protrusions[0].direction_milli = direction;
            assert!(crate::validate_beginner_generation_constraints_v1(
                &outside_landmark_root
            ));
            assert!(beginner_uses_bounded_generic_target_base_v1(
                &outside_landmark_root
            ));
            assert_eq!(
                beginner_target_approximation_score_v1(&outside_landmark_root),
                0
            );
            assert_eq!(
                generate_beginner_plans_v1(namespace, &source, &ids, &outside_landmark_root),
                Err(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate)
            );
        }
        let mut reversed_skeleton = generic.clone();
        reversed_skeleton.skeleton_segments.reverse();
        assert_eq!(
            generate_beginner_plans_v1(namespace, &source, &ids, &reversed_skeleton).unwrap(),
            generic_plans
        );
        let mut all_endpoints_reversed = generic.clone();
        for segment in &mut all_endpoints_reversed.skeleton_segments {
            std::mem::swap(&mut segment.start, &mut segment.end);
        }
        assert_eq!(
            generate_beginner_plans_v1(namespace, &source, &ids, &all_endpoints_reversed).unwrap(),
            generic_plans
        );
        let mut one_endpoint_reversed = generic.clone();
        let segment = &mut one_endpoint_reversed.skeleton_segments[1];
        std::mem::swap(&mut segment.start, &mut segment.end);
        assert_eq!(
            generate_beginner_plans_v1(namespace, &source, &ids, &one_endpoint_reversed).unwrap(),
            generic_plans
        );
        let mut shuffled_skeleton = generic.clone();
        shuffled_skeleton.skeleton_segments = [2, 0, 1]
            .map(|index| generic.skeleton_segments[index])
            .to_vec();
        for segment in &mut shuffled_skeleton.skeleton_segments {
            std::mem::swap(&mut segment.start, &mut segment.end);
        }
        assert_eq!(
            generate_beginner_plans_v1(namespace, &source, &ids, &shuffled_skeleton).unwrap(),
            generic_plans
        );
        let mut locally_outlined = generic.clone();
        locally_outlined.protrusions[0].local_outline_tenths_mm =
            Some(vec![[-2, -1], [0, -2], [2, -1], [1, 2], [-1, 2]]);
        let local_plans =
            generate_beginner_plans_v1(namespace, &source, &ids, &locally_outlined).unwrap();
        // The five-point local outline is additive to the corrected
        // count=2 physical endpoint base (not the former duplicated four).
        let local_supports = radial_corner_support_added(&local_plans[0]);
        assert_eq!(
            local_plans[0].crease_pattern.vertices.len(),
            16 + local_supports
        );
        assert_eq!(
            local_plans[0].crease_pattern.edges.len(),
            14 + local_supports
        );
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
        // Four body-outline records are additive to the same corrected
        // count=2 physical endpoint base.
        let outlined_supports = radial_corner_support_added(&outlined_plans[0]);
        assert_eq!(
            outlined_plans[0].crease_pattern.vertices.len(),
            15 + outlined_supports
        );
        assert_eq!(
            outlined_plans[0].crease_pattern.edges.len(),
            13 + outlined_supports
        );
        let mut general_outline = generic.clone();
        general_outline.generic_body_outline_mode = crate::BeginnerBodyOutlineModeV1::General;
        general_outline.generic_body_outline_tenths_mm =
            Some(vec![[-5, -5], [5, -5], [4, 5], [-3, 5]]);
        let general_plans =
            generate_beginner_plans_v1(namespace, &source, &ids, &general_outline).unwrap();
        let general_supports = radial_corner_support_added(&general_plans[0]);
        assert_eq!(
            general_plans[0].crease_pattern.vertices.len(),
            15 + general_supports
        );
        assert_eq!(
            general_plans[0].crease_pattern.edges.len(),
            13 + general_supports
        );
        let mut tapered_generic = generic.clone();
        tapered_generic.protrusions[1].root_width_tenths_mm = Some(1);
        tapered_generic.protrusions[1].tip_width_tenths_mm = Some(1);
        let tapered_plans =
            generate_beginner_plans_v1(namespace, &source, &ids, &tapered_generic).unwrap();
        assert_eq!(
            tapered_plans, generic_plans,
            "explicit root/tip widths equal to the canonical implicit thickness must normalize identically"
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
    fn asymmetric_landmark_families_receive_generation_consistent_scores() {
        let namespace = ProjectId::schema_namespace([0x6a; 16]);
        let (ids, source) = square_source(namespace);
        let skeleton_segments = vec![
            skeleton(1, -10, 0, 0, 10),
            skeleton(2, 10, 0, 0, 10),
            skeleton(3, 0, -10, 0, 10),
        ];
        let landmarks = |count: u16| {
            (1..=count)
                .map(|id| {
                    single_protrusion(
                        id,
                        [i32::from(id) - 5, i32::from(id % 3) - 1, 0],
                        if id % 2 == 0 {
                            [1_000, -100, 0]
                        } else {
                            [-1_000, 100, 0]
                        },
                    )
                })
                .collect::<Vec<_>>()
        };
        let base_parts = || {
            vec![
                BeginnerTargetPartRecordV1 {
                    kind: BeginnerTargetPartKindV1::Head,
                    count: 1,
                },
                BeginnerTargetPartRecordV1 {
                    kind: BeginnerTargetPartKindV1::Torso,
                    count: 1,
                },
            ]
        };

        let mut four_leg_parts = base_parts();
        four_leg_parts.push(BeginnerTargetPartRecordV1 {
            kind: BeginnerTargetPartKindV1::Leg,
            count: 4,
        });
        let four_leg = BeginnerGenerationConstraintsV1 {
            target_category: Some(BeginnerTargetCategoryV1::Animal),
            target_parts: four_leg_parts,
            skeleton_segments: skeleton_segments.clone(),
            protrusions: landmarks(4),
            ..BeginnerGenerationConstraintsV1::default()
        };

        let mut fish_parts = base_parts();
        fish_parts.extend([
            BeginnerTargetPartRecordV1 {
                kind: BeginnerTargetPartKindV1::Tail,
                count: 1,
            },
            BeginnerTargetPartRecordV1 {
                kind: BeginnerTargetPartKindV1::Fin,
                count: 2,
            },
        ]);
        let fish = BeginnerGenerationConstraintsV1 {
            target_category: Some(BeginnerTargetCategoryV1::Animal),
            target_parts: fish_parts,
            skeleton_segments: skeleton_segments.clone(),
            protrusions: landmarks(3),
            ..BeginnerGenerationConstraintsV1::default()
        };

        let mut insect_parts = base_parts();
        insect_parts.extend([
            BeginnerTargetPartRecordV1 {
                kind: BeginnerTargetPartKindV1::Tail,
                count: 1,
            },
            BeginnerTargetPartRecordV1 {
                kind: BeginnerTargetPartKindV1::Wing,
                count: 2,
            },
            BeginnerTargetPartRecordV1 {
                kind: BeginnerTargetPartKindV1::Leg,
                count: 6,
            },
        ]);
        let insect = BeginnerGenerationConstraintsV1 {
            target_category: Some(BeginnerTargetCategoryV1::Insect),
            target_parts: insect_parts,
            skeleton_segments,
            protrusions: landmarks(7),
            ..BeginnerGenerationConstraintsV1::default()
        };

        for (constraints, expected_kind, expected_error) in [
            (
                four_leg.clone(),
                BeginnerGeneratedPlanKindV1::AsymmetricFourLegLandmarkBase,
                BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate,
            ),
            (
                fish.clone(),
                BeginnerGeneratedPlanKindV1::AsymmetricFishLandmarkBase,
                BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate,
            ),
            (
                insect,
                BeginnerGeneratedPlanKindV1::AsymmetricInsectLandmarkBase,
                BeginnerGeneratorErrorV1::UnsupportedInsectTemplate,
            ),
        ] {
            assert!(crate::validate_beginner_generation_constraints_v1(
                &constraints
            ));
            assert_eq!(beginner_target_approximation_score_v1(&constraints), 92);
            assert_eq!(
                generate_beginner_plans_v1(namespace, &source, &ids, &constraints).unwrap()[0].kind,
                expected_kind
            );
            let mut contained_outline = constraints.clone();
            contained_outline.protrusions[0].local_outline_tenths_mm =
                Some(vec![[-1, -1], [1, -1], [0, 2]]);
            assert!(crate::validate_beginner_generation_constraints_v1(
                &contained_outline
            ));
            assert_eq!(
                beginner_target_approximation_score_v1(&contained_outline),
                92
            );
            assert_eq!(
                generate_beginner_plans_v1(namespace, &source, &ids, &contained_outline).unwrap()
                    [0]
                .kind,
                expected_kind
            );
            let mut outside_outline = constraints.clone();
            outside_outline.protrusions[0].local_outline_tenths_mm =
                Some(vec![[-20, -1], [20, -1], [0, 2]]);
            assert!(crate::validate_beginner_generation_constraints_v1(
                &outside_outline
            ));
            assert_eq!(beginner_target_approximation_score_v1(&outside_outline), 0);
            assert_eq!(
                generate_beginner_plans_v1(namespace, &source, &ids, &outside_outline),
                Err(expected_error)
            );
            let mut reordered = constraints.clone();
            reordered.protrusions.reverse();
            assert!(crate::validate_beginner_generation_constraints_v1(
                &reordered
            ));
            assert_eq!(beginner_target_approximation_score_v1(&reordered), 0);
            assert_eq!(
                generate_beginner_plans_v1(namespace, &source, &ids, &reordered),
                Err(expected_error)
            );
            let mut oversized = constraints.clone();
            let next_id = oversized
                .protrusions
                .last()
                .and_then(|target| target.id.checked_add(1))
                .expect("bounded landmark fixture");
            oversized.protrusions.push(bilateral_protrusion(next_id, 2));
            assert!(crate::validate_beginner_generation_constraints_v1(
                &oversized
            ));
            assert_eq!(beginner_target_approximation_score_v1(&oversized), 0);
            assert_eq!(
                generate_beginner_plans_v1(namespace, &source, &ids, &oversized),
                Err(expected_error)
            );
        }
        let mut zero_span_fish = fish;
        zero_span_fish.skeleton_segments = vec![skeleton(1, 0, -10, 0, 10)];
        zero_span_fish.generic_body_outline_tenths_mm =
            Some(vec![[-2, -1], [-1, 2], [1, 2], [2, -1], [0, -2]]);
        assert!(crate::validate_beginner_generation_constraints_v1(
            &zero_span_fish
        ));
        assert_eq!(beginner_target_approximation_score_v1(&zero_span_fish), 0);
        assert_eq!(
            generate_beginner_plans_v1(namespace, &source, &ids, &zero_span_fish),
            Err(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate)
        );
        zero_span_fish.skeleton_segments.clear();
        assert!(crate::validate_beginner_generation_constraints_v1(
            &zero_span_fish
        ));
        assert_eq!(beginner_target_approximation_score_v1(&zero_span_fish), 0);
        assert_eq!(
            generate_beginner_plans_v1(namespace, &source, &ids, &zero_span_fish),
            Err(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate)
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
        assert_eq!(beginner_target_approximation_score_v1(&constraints), 92);
        assert_eq!(
            generate_beginner_plans_v1(namespace, &source, &ids, &constraints).unwrap(),
            plans
        );
        let mut four_wings = constraints.clone();
        four_wings.target_parts[2].count = 4;
        four_wings.protrusions[0] = bilateral_protrusion(1, 4);
        four_wings.protrusions[0].position_tenths_mm[1] = 5;
        four_wings.protrusions[0].direction_milli = [1_000, 0, 0];
        assert!(crate::validate_beginner_generation_constraints_v1(
            &four_wings
        ));
        let four_wing_plans =
            generate_beginner_plans_v1(namespace, &source, &ids, &four_wings).unwrap();
        assert_eq!(
            four_wing_plans[0].kind,
            BeginnerGeneratedPlanKindV1::SymmetricWingBase
        );
        assert_eq!(four_wing_plans[0].crease_pattern.edges.len(), 4);
        assert_eq!(beginner_target_approximation_score_v1(&four_wings), 92);
        let mut estimated_four_wings = four_wings.clone();
        estimated_four_wings.protrusions.clear();
        assert_eq!(
            estimate_symmetric_parameters_v1(&estimated_four_wings)
                .unwrap()
                .protrusion_count,
            4
        );

        let assert_unsupported_four_wings = |invalid: &BeginnerGenerationConstraintsV1| {
            assert!(crate::validate_beginner_generation_constraints_v1(invalid));
            assert_eq!(beginner_target_approximation_score_v1(invalid), 0);
            assert_eq!(
                generate_beginner_plans_v1(namespace, &source, &ids, invalid),
                Err(BeginnerGeneratorErrorV1::UnsupportedInsectTemplate)
            );
        };
        let mut wrong_direction = four_wings.clone();
        wrong_direction.protrusions[0].direction_milli = [0, 1_000, 0];
        assert_unsupported_four_wings(&wrong_direction);
        let mut wrong_symmetry = four_wings.clone();
        wrong_symmetry.protrusions[0].symmetry = BeginnerProtrusionSymmetryV1::Radial;
        assert_unsupported_four_wings(&wrong_symmetry);
        let mut multiple_targets = four_wings.clone();
        let mut extra_wings = multiple_targets.protrusions[0].clone();
        extra_wings.id = 2;
        multiple_targets.protrusions.push(extra_wings);
        assert_unsupported_four_wings(&multiple_targets);
        let mut outside_root = four_wings.clone();
        outside_root.protrusions[0].position_tenths_mm[1] = 11;
        assert_unsupported_four_wings(&outside_root);

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
        assert_eq!(beginner_target_approximation_score_v1(&asymmetric), 92);
        let mut reordered_asymmetric = asymmetric.clone();
        reordered_asymmetric.protrusions.reverse();
        assert_eq!(
            beginner_target_approximation_score_v1(&reordered_asymmetric),
            0
        );
        assert_eq!(
            generate_beginner_plans_v1(namespace, &source, &ids, &reordered_asymmetric),
            Err(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate)
        );
        let mut antenna = constraints.clone();
        antenna.target_parts[2].kind = BeginnerTargetPartKindV1::Antenna;
        let antenna_plans = generate_beginner_plans_v1(namespace, &source, &ids, &antenna).unwrap();
        assert_eq!(
            antenna_plans[0].kind,
            BeginnerGeneratedPlanKindV1::SymmetricAntennaBase
        );
        assert_eq!(beginner_target_approximation_score_v1(&antenna), 92);
        let mut general_insect_family = constraints.clone();
        general_insect_family.target_parts[2].kind = BeginnerTargetPartKindV1::Fin;
        general_insect_family.target_parts[2].count = 3;
        general_insect_family.skeleton_segments = vec![
            skeleton(10, -100, -100, 100, -100),
            skeleton(20, 100, -100, 100, 100),
        ];
        general_insect_family.protrusions = vec![
            single_protrusion(1, [0, -50, 0], [1_000, 0, 0]),
            single_protrusion(2, [0, 0, 0], [-1_000, 0, 0]),
            single_protrusion(3, [0, 50, 0], [1_000, 0, 0]),
        ];
        assert!(crate::validate_beginner_generation_constraints_v1(
            &general_insect_family
        ));
        assert_eq!(
            beginner_target_approximation_score_v1(&general_insect_family),
            92
        );
        assert_eq!(
            generate_beginner_plans_v1(namespace, &source, &ids, &general_insect_family).unwrap()
                [0]
            .kind,
            BeginnerGeneratedPlanKindV1::CompositeGenericTargetBase
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
        assert!(!beginner_uses_bounded_generic_target_base_v1(&wing_antenna));
        assert_eq!(beginner_target_approximation_score_v1(&wing_antenna), 92);
        let composite_plans =
            generate_beginner_plans_v1(namespace, &source, &ids, &wing_antenna).unwrap();
        assert_eq!(
            composite_plans[0].kind,
            BeginnerGeneratedPlanKindV1::CompositeWingAntennaBase
        );
        let composite_supports = radial_corner_support_added(&composite_plans[0]);
        assert_eq!(
            composite_plans[0].crease_pattern.vertices.len(),
            9 + composite_supports
        );
        assert_eq!(
            composite_plans[0].crease_pattern.edges.len(),
            8 + composite_supports
        );
        assert_eq!(
            insect_wing_antenna_bindings_v1(&wing_antenna),
            Some(BeginnerWingAntennaBindingV1 {
                wing_pair_protrusion_id: 1,
                antenna_pair_protrusion_id: 2,
            })
        );
        let mut oversized_wing_antenna = wing_antenna.clone();
        oversized_wing_antenna
            .protrusions
            .push(bilateral_protrusion(3, 4));
        assert_eq!(
            insect_wing_antenna_bindings_v1(&oversized_wing_antenna),
            None
        );
        assert_eq!(
            beginner_target_approximation_score_v1(&oversized_wing_antenna),
            0
        );
        assert_eq!(
            generate_beginner_plans_v1(namespace, &source, &ids, &oversized_wing_antenna),
            Err(BeginnerGeneratorErrorV1::UnsupportedInsectTemplate)
        );
        let mut incomplete_wing_antenna = wing_antenna.clone();
        incomplete_wing_antenna.protrusions.truncate(1);
        assert!(crate::validate_beginner_generation_constraints_v1(
            &incomplete_wing_antenna
        ));
        assert_eq!(
            insect_wing_antenna_bindings_v1(&incomplete_wing_antenna),
            None
        );
        assert_eq!(
            beginner_target_approximation_score_v1(&incomplete_wing_antenna),
            0
        );
        assert_eq!(
            generate_beginner_plans_v1(namespace, &source, &ids, &incomplete_wing_antenna),
            Err(BeginnerGeneratorErrorV1::UnsupportedInsectTemplate)
        );
        for invalid_target_index in 0..2 {
            let mut invalid_composite = wing_antenna.clone();
            invalid_composite.protrusions[invalid_target_index].position_tenths_mm[0] = 1;
            assert!(crate::validate_beginner_generation_constraints_v1(
                &invalid_composite
            ));
            assert!(insect_wing_antenna_bindings_v1(&invalid_composite).is_some());
            assert_eq!(
                beginner_target_approximation_score_v1(&invalid_composite),
                0
            );
            assert_eq!(
                generate_beginner_plans_v1(namespace, &source, &ids, &invalid_composite),
                Err(BeginnerGeneratorErrorV1::UnsupportedInsectTemplate)
            );
        }
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
        let complete_supports = radial_corner_support_added(&complete_plans[0]);
        assert_eq!(
            complete_plans[0].crease_pattern.vertices.len(),
            21 + complete_supports
        );
        assert_eq!(
            complete_plans[0].crease_pattern.edges.len(),
            20 + complete_supports
        );
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
        assert_eq!(beginner_target_approximation_score_v1(&oversized), 0);
        assert_eq!(
            generate_beginner_plans_v1(namespace, &source, &ids, &oversized),
            Err(BeginnerGeneratorErrorV1::UnsupportedInsectTemplate)
        );
        let mut incomplete = complete.clone();
        incomplete.protrusions.retain(|target| target.id >= 3);
        assert!(crate::validate_beginner_generation_constraints_v1(
            &incomplete
        ));
        assert_eq!(insect_complete_bindings_v1(&incomplete), None);
        assert!(insect_three_pair_bindings_v1(&incomplete).is_some());
        assert_eq!(beginner_target_approximation_score_v1(&incomplete), 0);
        assert_eq!(
            generate_beginner_plans_v1(namespace, &source, &ids, &incomplete),
            Err(BeginnerGeneratorErrorV1::UnsupportedInsectTemplate)
        );
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
        assert_eq!(beginner_target_approximation_score_v1(&leg_pair), 92);
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
        assert_eq!(beginner_target_approximation_score_v1(&complete_legs), 92);
        let mut oversized_complete_legs = complete_legs.clone();
        oversized_complete_legs
            .protrusions
            .push(bilateral_protrusion(4, 4));
        assert_eq!(
            insect_three_pair_bindings_v1(&oversized_complete_legs),
            None
        );
        assert_eq!(
            beginner_target_approximation_score_v1(&oversized_complete_legs),
            0
        );
        assert_eq!(
            generate_beginner_plans_v1(namespace, &source, &ids, &oversized_complete_legs),
            Err(BeginnerGeneratorErrorV1::UnsupportedInsectTemplate)
        );
        let complete_plans =
            generate_beginner_plans_v1(namespace, &source, &ids, &complete_legs).unwrap();
        assert_eq!(
            complete_plans[0].kind,
            BeginnerGeneratedPlanKindV1::SymmetricSixLegBase
        );
        let complete_supports = radial_corner_support_added(&complete_plans[0]);
        assert_eq!(
            complete_plans[0].crease_pattern.vertices.len(),
            13 + complete_supports
        );
        assert_eq!(
            complete_plans[0].crease_pattern.edges.len(),
            12 + complete_supports
        );

        let mut priority_order = complete_legs.clone();
        for (target, priority) in priority_order.protrusions.iter_mut().zip([40, 70, 100]) {
            target.priority = priority;
        }
        assert_eq!(beginner_target_approximation_score_v1(&priority_order), 76);
        let priority_plans =
            generate_beginner_plans_v1(namespace, &source, &ids, &priority_order).unwrap();
        priority_order.protrusions.reverse();
        assert_eq!(beginner_target_approximation_score_v1(&priority_order), 76);
        assert_eq!(
            generate_beginner_plans_v1(namespace, &source, &ids, &priority_order).unwrap(),
            priority_plans
        );

        let mut invalid_third_pair = complete_legs.clone();
        invalid_third_pair.protrusions[2].length_tenths_mm = 10;
        assert!(insect_three_pair_bindings_v1(&invalid_third_pair).is_some());
        assert_eq!(
            beginner_target_approximation_score_v1(&invalid_third_pair),
            0
        );
        assert_eq!(
            generate_beginner_plans_v1(namespace, &source, &ids, &invalid_third_pair),
            Err(BeginnerGeneratorErrorV1::UnsupportedInsectTemplate)
        );

        complete_legs.protrusions[2].position_tenths_mm[1] = 5;
        assert_eq!(insect_three_pair_bindings_v1(&complete_legs), None);
        assert_eq!(beginner_target_approximation_score_v1(&complete_legs), 0);
        assert_eq!(
            generate_beginner_plans_v1(namespace, &source, &ids, &complete_legs),
            Err(BeginnerGeneratorErrorV1::UnsupportedInsectTemplate)
        );
        constraints.skeleton_segments[1].end.y_tenths_mm = 11;
        assert!(crate::validate_beginner_generation_constraints_v1(
            &constraints
        ));
        assert_eq!(beginner_target_approximation_score_v1(&constraints), 0);
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
        assert_eq!(beginner_target_approximation_score_v1(&constraints), 72);
        let mut ambiguous = constraints;
        ambiguous.target_parts[2].count = 3;
        assert_eq!(estimate_symmetric_parameters_v1(&ambiguous), None);
        assert_eq!(
            beginner_expected_generated_plan_kind_v1(&ambiguous),
            Some(BeginnerGeneratedPlanKindV1::CompositeGenericTargetBase)
        );

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

    #[test]
    fn aggregate_general_signatures_two_through_fourteen_reach_generation() {
        let namespace = ProjectId::schema_namespace([0x6a; 16]);
        let (ids, source) = square_source(namespace);
        for category in [
            BeginnerTargetCategoryV1::CustomObject,
            BeginnerTargetCategoryV1::Animal,
            BeginnerTargetCategoryV1::Insect,
        ] {
            for count in 2_u8..=MAX_BEGINNER_GENERAL_PROTRUSION_COUNT_V1 {
                let mut target_parts = Vec::new();
                if category != BeginnerTargetCategoryV1::CustomObject {
                    target_parts.extend([
                        BeginnerTargetPartRecordV1 {
                            kind: BeginnerTargetPartKindV1::Head,
                            count: 1,
                        },
                        BeginnerTargetPartRecordV1 {
                            kind: BeginnerTargetPartKindV1::Torso,
                            count: 1,
                        },
                    ]);
                }
                if count == 2 {
                    target_parts.extend([
                        BeginnerTargetPartRecordV1 {
                            kind: BeginnerTargetPartKindV1::Fin,
                            count: 1,
                        },
                        BeginnerTargetPartRecordV1 {
                            kind: BeginnerTargetPartKindV1::Tail,
                            count: 1,
                        },
                    ]);
                } else {
                    target_parts.push(BeginnerTargetPartRecordV1 {
                        kind: BeginnerTargetPartKindV1::Fin,
                        count: count.min(crate::MAX_BEGINNER_TARGET_PART_COUNT_V1),
                    });
                    if count > crate::MAX_BEGINNER_TARGET_PART_COUNT_V1 {
                        target_parts.push(BeginnerTargetPartRecordV1 {
                            kind: BeginnerTargetPartKindV1::Tail,
                            count: count - crate::MAX_BEGINNER_TARGET_PART_COUNT_V1,
                        });
                    }
                }
                let protrusions = (0..count)
                    .map(|index| {
                        single_protrusion(
                            u16::from(index) + 1,
                            [
                                0,
                                -100 + (i32::from(index) + 1) * 200 / (i32::from(count) + 1),
                                0,
                            ],
                            if index % 2 == 0 {
                                [1_000, 0, 0]
                            } else {
                                [-1_000, 0, 0]
                            },
                        )
                    })
                    .collect::<Vec<_>>();
                let constraints = BeginnerGenerationConstraintsV1 {
                    target_category: Some(category),
                    target_parts: target_parts.clone(),
                    skeleton_segments: vec![
                        skeleton(10, -100, -100, 100, -100),
                        skeleton(20, 100, -100, 100, 100),
                    ],
                    protrusions,
                    ..BeginnerGenerationConstraintsV1::default()
                };
                assert!(crate::validate_beginner_generation_constraints_v1(
                    &constraints
                ));
                assert_eq!(
                    estimate_symmetric_parameters_v1(&constraints)
                        .map(|estimate| estimate.protrusion_count),
                    Some(count),
                    "{category:?} compact count {count}"
                );
                assert_eq!(
                    beginner_expected_generated_plan_kind_v1(&constraints),
                    Some(BeginnerGeneratedPlanKindV1::CompositeGenericTargetBase)
                );
                assert!(beginner_uses_bounded_generic_target_base_v1(&constraints));
                let plans =
                    generate_beginner_plans_v1(namespace, &source, &ids, &constraints).unwrap();
                assert_eq!(
                    plans[0].kind,
                    BeginnerGeneratedPlanKindV1::CompositeGenericTargetBase
                );
                assert_eq!(plans[0].target_parts, target_parts);
                let supports = radial_corner_support_added(&plans[0]);
                let expected_instruction_count = 3;
                if matches!(count, 2 | 4) {
                    assert_eq!(
                        supports, 4,
                        "small even generic fans must add every paper corner before certification"
                    );
                    assert_eq!(
                        plans[0]
                            .crease_pattern
                            .edges
                            .iter()
                            .filter(|edge| {
                                matches!(edge.kind, EdgeKind::Mountain | EdgeKind::Valley)
                            })
                            .count(),
                        usize::from(count) + supports,
                        "small even generic fans must preserve every semantic ray after the support prefix"
                    );
                    assert!(
                        usize::from(count) + supports >= 6
                            && (usize::from(count) + supports).is_multiple_of(2),
                        "small generic fans must enter the six-or-more even radial theorem"
                    );
                }
                if count == 13 {
                    assert_eq!(
                        supports, 5,
                        "count thirteen must add one parity ray after the four corner supports"
                    );
                    assert_eq!(
                        plans[0]
                            .crease_pattern
                            .edges
                            .iter()
                            .filter(|edge| {
                                matches!(edge.kind, EdgeKind::Mountain | EdgeKind::Valley)
                            })
                            .count(),
                        18,
                        "count thirteen must enter the existing even radial theorem"
                    );
                }
                if count == MAX_BEGINNER_GENERAL_PROTRUSION_COUNT_V1 {
                    assert_eq!(
                        supports, 4,
                        "count fourteen must consume the four bounded corner-support slots"
                    );
                    assert_eq!(
                        plans[0]
                            .crease_pattern
                            .edges
                            .iter()
                            .filter(|edge| {
                                matches!(edge.kind, EdgeKind::Mountain | EdgeKind::Valley)
                            })
                            .count(),
                        18,
                        "count fourteen must remain below the native radial theorem boundary"
                    );
                }
                assert_eq!(plans[0].instruction_codes.len(), expected_instruction_count);
                assert_eq!(
                    plans[0].crease_pattern.edges.len(),
                    usize::from(count) + 2 + supports
                );
            }
        }
    }

    #[test]
    fn direct_custom_grouped_targets_two_through_eight_preserve_endpoint_count() {
        let namespace = ProjectId::schema_namespace([0x6b; 16]);
        let (ids, source) = square_source(namespace);
        for count in 2_u8..=8 {
            let mut grouped = single_protrusion(65_535, [500, 500, 0], [1_000, 0, 0]);
            grouped.count = count;
            grouped.length_tenths_mm = 100;
            grouped.thickness_tenths_mm = 10;
            grouped.symmetry = if count % 2 == 0 {
                BeginnerProtrusionSymmetryV1::Bilateral
            } else {
                BeginnerProtrusionSymmetryV1::Radial
            };
            let constraints = BeginnerGenerationConstraintsV1 {
                target_category: Some(BeginnerTargetCategoryV1::CustomObject),
                target_parts: vec![BeginnerTargetPartRecordV1 {
                    kind: BeginnerTargetPartKindV1::Fin,
                    count,
                }],
                skeleton_segments: vec![
                    skeleton(1, 0, 0, 1_000, 0),
                    skeleton(2, 1_000, 0, 1_000, 1_000),
                ],
                protrusions: vec![grouped],
                ..BeginnerGenerationConstraintsV1::default()
            };
            assert_eq!(
                estimate_symmetric_parameters_v1(&constraints)
                    .map(|estimate| estimate.protrusion_count),
                Some(count)
            );
            assert!(beginner_target_approximation_score_v1(&constraints) > 0);
            let plan = &generate_beginner_plans_v1(namespace, &source, &ids, &constraints)
                .expect("grouped custom target")[0];
            assert_eq!(
                plan.kind,
                BeginnerGeneratedPlanKindV1::CompositeGenericTargetBase
            );
            assert!(
                beginner_generated_pattern_is_planar_v1(&plan.crease_pattern),
                "generic count {count} must relocate its tree away from every physical ray"
            );
            assert_eq!(
                plan.crease_pattern.edges.len(),
                usize::from(count)
                    + constraints.skeleton_segments.len()
                    + if matches!(count, 2 | 3 | 4 | 5 | 6 | 7 | 8) {
                        radial_corner_support_added(plan)
                    } else {
                        0
                    }
            );
        }

        let mut mismatched = single_protrusion(1, [500, 500, 0], [1_000, 0, 0]);
        mismatched.count = 3;
        mismatched.length_tenths_mm = 100;
        mismatched.thickness_tenths_mm = 10;
        mismatched.symmetry = BeginnerProtrusionSymmetryV1::Radial;
        let mismatch = BeginnerGenerationConstraintsV1 {
            target_category: Some(BeginnerTargetCategoryV1::CustomObject),
            target_parts: vec![BeginnerTargetPartRecordV1 {
                kind: BeginnerTargetPartKindV1::Fin,
                count: 5,
            }],
            skeleton_segments: vec![
                skeleton(1, 0, 0, 1_000, 0),
                skeleton(2, 1_000, 0, 1_000, 1_000),
            ],
            protrusions: vec![mismatched],
            ..BeginnerGenerationConstraintsV1::default()
        };
        assert_eq!(estimate_symmetric_parameters_v1(&mismatch), None);
        assert_eq!(beginner_target_approximation_score_v1(&mismatch), 0);
        assert_eq!(
            generate_beginner_plans_v1(namespace, &source, &ids, &mismatch),
            Err(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate)
        );
    }

    #[test]
    fn custom_single_feature_preview_relocates_tree_but_stays_outside_grid_domain() {
        let namespace = ProjectId::schema_namespace([0x6e; 16]);
        let (ids, source) = square_source(namespace);
        let mut target = single_protrusion(1, [500, 500, 0], [1_000, 0, 0]);
        target.length_tenths_mm = 100;
        target.thickness_tenths_mm = 10;
        let constraints = BeginnerGenerationConstraintsV1 {
            target_category: Some(BeginnerTargetCategoryV1::CustomObject),
            target_parts: vec![BeginnerTargetPartRecordV1 {
                kind: BeginnerTargetPartKindV1::Fin,
                count: 1,
            }],
            skeleton_segments: vec![
                skeleton(1, 0, 0, 1_000, 0),
                skeleton(2, 1_000, 0, 1_000, 1_000),
            ],
            protrusions: vec![target],
            ..BeginnerGenerationConstraintsV1::default()
        };
        assert!(crate::validate_beginner_generation_constraints_v1(
            &constraints
        ));
        assert_eq!(estimate_symmetric_parameters_v1(&constraints), None);
        let plan = &generate_beginner_plans_v1(namespace, &source, &ids, &constraints)
            .expect("single-feature custom preview")[0];
        assert_eq!(
            plan.kind,
            BeginnerGeneratedPlanKindV1::CompositeGenericTargetBase
        );
        assert_eq!(plan.crease_pattern.edges.len(), 3);
        assert!(beginner_generated_pattern_is_planar_v1(
            &plan.crease_pattern
        ));
        assert!(
            plan.instruction_codes
                .iter()
                .all(|code| !code.starts_with("bounded_radial_corner_support_v1:"))
        );
    }

    #[test]
    fn general_semantic_count_fifteen_remains_outside_the_v1_parameter_domain() {
        let namespace = ProjectId::schema_namespace([0x6d; 16]);
        let (ids, source) = square_source(namespace);
        let constraints = BeginnerGenerationConstraintsV1 {
            target_category: Some(BeginnerTargetCategoryV1::Animal),
            target_parts: [
                (BeginnerTargetPartKindV1::Head, 1),
                (BeginnerTargetPartKindV1::Torso, 1),
                (BeginnerTargetPartKindV1::Fin, 8),
                (BeginnerTargetPartKindV1::Tail, 7),
            ]
            .into_iter()
            .map(|(kind, count)| BeginnerTargetPartRecordV1 { kind, count })
            .collect(),
            skeleton_segments: vec![
                skeleton(10, -100, -100, 100, -100),
                skeleton(20, 100, -100, 100, 100),
            ],
            protrusions: (0_u16..15)
                .map(|index| {
                    single_protrusion(
                        index + 1,
                        [0, -90 + i32::from(index) * 16, 0],
                        if index % 2 == 0 {
                            [1_000, 0, 0]
                        } else {
                            [-1_000, 0, 0]
                        },
                    )
                })
                .collect(),
            ..BeginnerGenerationConstraintsV1::default()
        };
        assert!(crate::validate_beginner_generation_constraints_v1(
            &constraints
        ));
        assert_eq!(general_semantic_protrusion_count_v1(&constraints), None);
        assert_eq!(estimate_symmetric_parameters_v1(&constraints), None);
        assert_eq!(beginner_expected_generated_plan_kind_v1(&constraints), None);
        assert_eq!(beginner_target_approximation_score_v1(&constraints), 0);
        assert_eq!(
            generate_beginner_plans_v1(namespace, &source, &ids, &constraints),
            Err(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate)
        );
    }

    #[test]
    fn custom_generic_binding_record_cap_accepts_fourteen_and_rejects_fifteen() {
        let namespace = ProjectId::schema_namespace([0x6c; 16]);
        let (ids, source) = square_source(namespace);
        let constraints = |bindings: u16| BeginnerGenerationConstraintsV1 {
            target_category: Some(BeginnerTargetCategoryV1::CustomObject),
            skeleton_segments: vec![
                skeleton(1, 0, 0, 1_000, 0),
                skeleton(2, 1_000, 0, 1_000, 1_000),
            ],
            protrusions: (0..bindings)
                .map(|index| {
                    let mut target = single_protrusion(
                        index,
                        [500, 100 + i32::from(index) * 60, 0],
                        if index % 2 == 0 {
                            [1_000, 0, 0]
                        } else {
                            [-1_000, 0, 0]
                        },
                    );
                    target.length_tenths_mm = 100;
                    target.thickness_tenths_mm = 10;
                    target
                })
                .collect(),
            ..BeginnerGenerationConstraintsV1::default()
        };
        let fourteen = constraints(14);
        assert!(beginner_target_approximation_score_v1(&fourteen) > 0);
        let plan =
            generate_beginner_plans_v1(namespace, &source, &ids, &fourteen).expect("14 bindings");
        assert!(beginner_generated_pattern_is_planar_v1(
            &plan[0].crease_pattern
        ));
        assert_eq!(
            plan[0].crease_pattern.edges.len(),
            16 + radial_corner_support_added(&plan[0])
        );

        let fifteen = constraints(15);
        assert_eq!(beginner_target_approximation_score_v1(&fifteen), 0);
        assert_eq!(
            generate_beginner_plans_v1(namespace, &source, &ids, &fifteen),
            Err(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate)
        );
    }

    #[test]
    fn custom_estimate_uses_explicit_features_then_physical_fallback() {
        let physical = vec![
            single_protrusion(0, [0, -50, 0], [1_000, 0, 0]),
            single_protrusion(255, [0, 0, 0], [-1_000, 0, 0]),
            single_protrusion(u16::MAX, [0, 50, 0], [1_000, 0, 0]),
        ];
        for target_parts in [
            Vec::new(),
            vec![
                BeginnerTargetPartRecordV1 {
                    kind: BeginnerTargetPartKindV1::Head,
                    count: 1,
                },
                BeginnerTargetPartRecordV1 {
                    kind: BeginnerTargetPartKindV1::Torso,
                    count: 1,
                },
            ],
            vec![BeginnerTargetPartRecordV1 {
                kind: BeginnerTargetPartKindV1::Torso,
                count: 1,
            }],
        ] {
            let constraints = BeginnerGenerationConstraintsV1 {
                target_category: Some(BeginnerTargetCategoryV1::CustomObject),
                target_parts,
                protrusions: physical.clone(),
                ..BeginnerGenerationConstraintsV1::default()
            };
            assert_eq!(
                estimate_symmetric_parameters_v1(&constraints)
                    .map(|estimate| estimate.protrusion_count),
                Some(3)
            );
            assert_eq!(
                beginner_expected_generated_plan_kind_v1(&constraints),
                Some(BeginnerGeneratedPlanKindV1::CompositeGenericTargetBase)
            );
        }

        let explicit = BeginnerGenerationConstraintsV1 {
            target_category: Some(BeginnerTargetCategoryV1::CustomObject),
            target_parts: vec![BeginnerTargetPartRecordV1 {
                kind: BeginnerTargetPartKindV1::Fin,
                count: 5,
            }],
            protrusions: physical,
            ..BeginnerGenerationConstraintsV1::default()
        };
        assert_eq!(
            estimate_symmetric_parameters_v1(&explicit).map(|estimate| estimate.protrusion_count),
            None
        );
    }

    #[test]
    fn exact_specialized_signatures_do_not_ignore_duplicates_or_extra_parts() {
        let parts = |features: &[(BeginnerTargetPartKindV1, u8)]| {
            [
                &[
                    (BeginnerTargetPartKindV1::Head, 1),
                    (BeginnerTargetPartKindV1::Torso, 1),
                ][..],
                features,
            ]
            .concat()
            .into_iter()
            .map(|(kind, count)| BeginnerTargetPartRecordV1 { kind, count })
            .collect::<Vec<_>>()
        };
        let specialized = BeginnerGenerationConstraintsV1 {
            target_category: Some(BeginnerTargetCategoryV1::Animal),
            target_parts: parts(&[(BeginnerTargetPartKindV1::Wing, 2)]),
            ..BeginnerGenerationConstraintsV1::default()
        };
        assert_eq!(
            beginner_expected_generated_plan_kind_v1(&specialized),
            Some(BeginnerGeneratedPlanKindV1::SymmetricBirdBase)
        );

        let mut with_extra = specialized.clone();
        with_extra.target_parts.push(BeginnerTargetPartRecordV1 {
            kind: BeginnerTargetPartKindV1::Fin,
            count: 3,
        });
        assert_eq!(
            beginner_expected_generated_plan_kind_v1(&with_extra),
            Some(BeginnerGeneratedPlanKindV1::CompositeGenericTargetBase)
        );
        assert_eq!(
            estimate_symmetric_parameters_v1(&with_extra).map(|estimate| estimate.protrusion_count),
            None
        );

        let mut duplicate = specialized;
        duplicate.target_parts.push(BeginnerTargetPartRecordV1 {
            kind: BeginnerTargetPartKindV1::Wing,
            count: 2,
        });
        assert_eq!(beginner_expected_generated_plan_kind_v1(&duplicate), None);
        assert_eq!(estimate_symmetric_parameters_v1(&duplicate), None);
    }

    #[test]
    fn complete_winged_animal_estimate_includes_the_wing_pair() {
        let mut constraints = BeginnerGenerationConstraintsV1 {
            target_category: Some(BeginnerTargetCategoryV1::Animal),
            target_parts: [
                (BeginnerTargetPartKindV1::Head, 1),
                (BeginnerTargetPartKindV1::Torso, 1),
                (BeginnerTargetPartKindV1::Horn, 1),
                (BeginnerTargetPartKindV1::Tail, 1),
                (BeginnerTargetPartKindV1::Ear, 2),
                (BeginnerTargetPartKindV1::Leg, 4),
            ]
            .into_iter()
            .map(|(kind, count)| BeginnerTargetPartRecordV1 { kind, count })
            .collect(),
            ..BeginnerGenerationConstraintsV1::default()
        };
        let complete = estimate_symmetric_parameters_v1(&constraints).unwrap();
        assert_eq!(complete.protrusion_count, 8);

        constraints.target_parts.push(BeginnerTargetPartRecordV1 {
            kind: BeginnerTargetPartKindV1::Wing,
            count: 2,
        });
        let winged = estimate_symmetric_parameters_v1(&constraints).unwrap();
        assert_eq!(winged.protrusion_count, 10);
        assert!(
            symmetric_parameter_candidates_v1(winged)
                .iter()
                .all(|candidate| candidate.required_protrusion_count == 10)
        );
    }

    #[test]
    fn asymmetric_fish_estimate_counts_three_ordered_landmarks() {
        let parts = |features: &[(BeginnerTargetPartKindV1, u8)]| {
            [
                &[
                    (BeginnerTargetPartKindV1::Head, 1),
                    (BeginnerTargetPartKindV1::Torso, 1),
                ][..],
                features,
            ]
            .concat()
            .into_iter()
            .map(|(kind, count)| BeginnerTargetPartRecordV1 { kind, count })
            .collect::<Vec<_>>()
        };
        let estimate = |features: &[(BeginnerTargetPartKindV1, u8)]| {
            estimate_symmetric_parameters_v1(&BeginnerGenerationConstraintsV1 {
                target_category: Some(BeginnerTargetCategoryV1::Animal),
                target_parts: parts(features),
                ..BeginnerGenerationConstraintsV1::default()
            })
            .unwrap()
        };

        let fish = estimate(&[
            (BeginnerTargetPartKindV1::Tail, 1),
            (BeginnerTargetPartKindV1::Fin, 2),
        ]);
        assert_eq!(fish.protrusion_count, 3);
        assert!(
            symmetric_parameter_candidates_v1(fish)
                .iter()
                .all(|candidate| candidate.required_protrusion_count == 3)
        );
        assert_eq!(
            estimate(&[(BeginnerTargetPartKindV1::Fin, 2)]).protrusion_count,
            2
        );
        assert_eq!(
            estimate(&[(BeginnerTargetPartKindV1::Tail, 1)]).protrusion_count,
            1
        );
        assert_eq!(
            estimate(&[
                (BeginnerTargetPartKindV1::Horn, 1),
                (BeginnerTargetPartKindV1::Ear, 2),
            ])
            .protrusion_count,
            3
        );
        assert_eq!(
            estimate(&[
                (BeginnerTargetPartKindV1::Tail, 1),
                (BeginnerTargetPartKindV1::Ear, 2),
            ])
            .protrusion_count,
            3
        );
        assert_eq!(
            estimate(&[
                (BeginnerTargetPartKindV1::Horn, 1),
                (BeginnerTargetPartKindV1::Tail, 1),
            ])
            .protrusion_count,
            2
        );
        assert_eq!(
            estimate(&[
                (BeginnerTargetPartKindV1::Horn, 1),
                (BeginnerTargetPartKindV1::Tail, 1),
                (BeginnerTargetPartKindV1::Ear, 2),
            ])
            .protrusion_count,
            4
        );
    }

    #[test]
    fn asymmetric_insect_estimate_counts_seven_ordered_landmarks() {
        let estimate = |features: &[(BeginnerTargetPartKindV1, u8)]| {
            estimate_symmetric_parameters_v1(&BeginnerGenerationConstraintsV1 {
                target_category: Some(BeginnerTargetCategoryV1::Insect),
                target_parts: [
                    &[
                        (BeginnerTargetPartKindV1::Head, 1),
                        (BeginnerTargetPartKindV1::Torso, 1),
                    ][..],
                    features,
                ]
                .concat()
                .into_iter()
                .map(|(kind, count)| BeginnerTargetPartRecordV1 { kind, count })
                .collect(),
                ..BeginnerGenerationConstraintsV1::default()
            })
            .unwrap()
        };

        let insect = estimate(&[
            (BeginnerTargetPartKindV1::Tail, 1),
            (BeginnerTargetPartKindV1::Wing, 2),
            (BeginnerTargetPartKindV1::Leg, 6),
        ]);
        assert_eq!(
            insect,
            BeginnerSymmetricParameterEstimateV1 {
                protrusion_count: 7,
                scale_percent: 25,
                spacing_percent: 50,
            }
        );
        let candidates = symmetric_parameter_candidates_v1(insect);
        assert_eq!(
            candidates.map(|candidate| candidate.complexity_score),
            [95, 94, 96]
        );
        assert!(
            candidates
                .iter()
                .all(|candidate| candidate.required_protrusion_count == 7)
        );

        assert_eq!(
            estimate(&[(BeginnerTargetPartKindV1::Wing, 2)]).protrusion_count,
            2
        );
        assert_eq!(
            estimate(&[(BeginnerTargetPartKindV1::Leg, 6)]).protrusion_count,
            6
        );
        assert_eq!(
            estimate(&[
                (BeginnerTargetPartKindV1::Wing, 2),
                (BeginnerTargetPartKindV1::Antenna, 2),
            ])
            .protrusion_count,
            4
        );
        assert_eq!(
            estimate(&[
                (BeginnerTargetPartKindV1::Wing, 2),
                (BeginnerTargetPartKindV1::Antenna, 2),
                (BeginnerTargetPartKindV1::Leg, 6),
            ])
            .protrusion_count,
            10
        );
        assert_eq!(
            estimate(&[(BeginnerTargetPartKindV1::Wing, 4)]).protrusion_count,
            4
        );
    }

    fn square_source(namespace: ProjectId) -> ([VertexId; 4], CreasePattern) {
        let ids = ["a", "b", "c", "d"].map(|name| VertexId::derive_v5(namespace, name.as_bytes()));
        let source = CreasePattern {
            vertices: ids
                .iter()
                .copied()
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
        (ids, source)
    }

    fn single_protrusion(
        id: u16,
        position_tenths_mm: [i32; 3],
        direction_milli: [i16; 3],
    ) -> BeginnerProtrusionTargetV1 {
        BeginnerProtrusionTargetV1 {
            id,
            count: 1,
            length_tenths_mm: 20,
            thickness_tenths_mm: 2,
            root_width_tenths_mm: None,
            tip_width_tenths_mm: None,
            local_outline_tenths_mm: None,
            position_tenths_mm,
            direction_milli,
            symmetry: BeginnerProtrusionSymmetryV1::None,
            curvature_degrees: 0,
            joint: crate::BeginnerProtrusionJointV1::Fixed,
            motion_degrees: [0, 0],
            side: crate::BeginnerProtrusionSideV1::Either,
            priority: 80,
        }
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
