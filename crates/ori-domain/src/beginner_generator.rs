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

pub fn generate_beginner_plans_v1(
    namespace: ProjectId,
    source: &CreasePattern,
    boundary_vertices: &[VertexId],
    constraints: &BeginnerGenerationConstraintsV1,
) -> Result<Vec<BeginnerGeneratedPlanV1>, BeginnerGeneratorErrorV1> {
    if source.vertices.len() > MAX_BEGINNER_GENERATOR_INPUT_VERTICES_V1 {
        return Err(BeginnerGeneratorErrorV1::ResourceLimit);
    }
    if boundary_vertices.len() != 4 {
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
    if ![min_x, max_x, min_y, max_y].into_iter().all(f64::is_finite)
        || min_x >= max_x
        || min_y >= max_y
        || !points.iter().all(|point| {
            (point.x == min_x || point.x == max_x) && (point.y == min_y || point.y == max_y)
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
            if part_count(BeginnerTargetPartKindV1::Horn) == 1
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
                    if part_count(BeginnerTargetPartKindV1::Leg) == 4 {
                        (
                            4,
                            true,
                            BeginnerGeneratedPlanKindV1::SymmetricFourLegBase,
                            "symmetric_four_leg_base",
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
                if constraints.skeleton_segments.len() < if vertical { 3 } else { 2 }
                    || !has_bilateral_skeleton(constraints)
                    || !has_bilateral_protrusion_count(constraints, required_count)
                {
                    return Err(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate);
                }
                let endpoints =
                    parameterized_symmetric_endpoints(constraints, required_count, vertical)
                        .ok_or(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate)?;
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
            if part_count(BeginnerTargetPartKindV1::Wing) == 2
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
    target.map_or_else(
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
    )
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
    let thickness_ratio =
        f64::from(target.thickness_tenths_mm) / f64::from(u32::try_from(span_x.min(span_y)).ok()?);
    if !(0.02..=0.45).contains(&length_ratio) || !(0.001..=0.25).contains(&thickness_ratio) {
        return None;
    }
    let priority_scale = 0.75 + f64::from(target.priority) / 400.0;
    let direction_scale = f64::from(primary_direction.unsigned_abs()) / 1_000.0;
    let reach = length_ratio * priority_scale * direction_scale;
    let spread = (thickness_ratio * 2.0).clamp(0.05, 0.2);
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
    let center = Point2::new((min_x + max_x) / 2.0, (min_y + max_y) / 2.0);
    let center_id = source
        .vertices
        .iter()
        .find(|vertex| vertex.position == center)
        .map_or_else(
            || VertexId::derive_v5(namespace, format!("{prefix}-center").as_bytes()),
            |vertex| vertex.id,
        );
    let mut vertices = vec![Vertex {
        id: center_id,
        position: center,
    }];
    let mut edges = Vec::with_capacity(endpoints.len());
    for (index, (x_ratio, y_ratio)) in endpoints.iter().copied().enumerate() {
        let position = Point2::new(
            min_x + (max_x - min_x) * x_ratio,
            min_y + (max_y - min_y) * y_ratio,
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
            id: EdgeId::derive_v5(namespace, format!("{prefix}-e-{index}").as_bytes()),
            start: center_id,
            end: id,
            kind: edge_kind,
        });
    }
    BeginnerGeneratedPlanV1 {
        schema_version: BEGINNER_GENERATOR_SCHEMA_VERSION_V1,
        kind: plan_kind,
        crease_pattern: CreasePattern { vertices, edges },
        instruction_codes: vec![instruction.to_owned()],
        target_parts: constraints.target_parts.clone(),
        skeleton_segments: constraints.skeleton_segments.clone(),
        target_asset: constraints.target_asset,
    }
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
        let mut tail_target = horn_tail.protrusions[0];
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
        complete_animal.protrusions.push(legs);
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
