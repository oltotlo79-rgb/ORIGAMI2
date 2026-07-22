use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    AssetId, BeginnerGenerationConstraintsV1, BeginnerSemanticLandmarkProvenanceV1,
    BeginnerTargetPartKindV1, validate_beginner_generation_constraints_v1,
};

pub const BEGINNER_DESIGN_PROFILE_SCHEMA_VERSION_V1: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BeginnerDesignPresetV1 {
    Balanced,
    ShapePriority,
    FoldabilityPriority,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BeginnerDesignProfileV1 {
    pub schema_version: u32,
    pub preset: BeginnerDesignPresetV1,
    pub shape_fidelity_weight: u8,
    pub foldability_weight: u8,
    pub step_count_weight: u8,
    pub paper_efficiency_weight: u8,
    #[serde(default)]
    pub generation_constraints: BeginnerGenerationConstraintsV1,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generation_provenance: Option<BeginnerGenerationProvenanceV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference_surface_landmarks_tenths_mm: Option<Vec<[i32; 3]>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outline_edit_authority: Option<BeginnerOutlineEditAuthorityV1>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub archived_reference_model_asset_ids: Vec<AssetId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BeginnerOutlineEditAuthorityV1 {
    pub schema_version: u32,
    pub source_asset_id: AssetId,
    pub source_sha256: [u8; 32],
    pub edits: Vec<BeginnerOutlineEditRecordV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum BeginnerOutlineEditRecordV1 {
    SplitVertical {
        source_candidate_id: u8,
        split_x: u32,
        fragment_kinds: [BeginnerTargetPartKindV1; 2],
    },
    Merge {
        source_candidate_ids: [u8; 2],
        merged_kind: BeginnerTargetPartKindV1,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BeginnerGenerationProvenanceV1 {
    pub schema_version: u32,
    pub topology_authority_sha256: [u8; 32],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fold_path_certificate_sha256: Option<[u8; 32]>,
    pub confidence_score: u8,
    pub confidence_reasons: Vec<String>,
    pub explicit_override: bool,
    pub source_asset_fingerprint: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_landmark_provenance: Option<BeginnerSemanticLandmarkProvenanceV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generic_tree: Option<BeginnerGenericTreeProvenanceV1>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BeginnerGenericTreeSourceV1 {
    ImageSilhouette,
    GlbGeometry,
    ManualSkeleton,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BeginnerGenericTreeOrientationV1 {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BeginnerGenericTreeProvenanceV1 {
    pub schema_version: u32,
    pub source: BeginnerGenericTreeSourceV1,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asset_content_sha256: Option<[u8; 32]>,
    pub tree_topology_sha256: [u8; 32],
    pub normalized_length_ratios: Vec<u32>,
    pub orientation: BeginnerGenericTreeOrientationV1,
    pub generator_version: u16,
    pub authorizes_apply: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instruction_proposal: Option<BeginnerGenericTreeInstructionProposalV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BeginnerGenericTreeInstructionProposalV1 {
    pub schema_version: u32,
    pub topology_sha256: [u8; 32],
    pub generator_version: u16,
    pub authorizes_apply: bool,
    pub physical_motion_proof: bool,
    pub steps: Vec<BeginnerGenericTreeInstructionStepV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BeginnerGenericTreeInstructionStepV1 {
    pub canonical_crease_id: String,
    pub tree_depth: u8,
    pub assignment: String,
    pub target_branch: String,
    pub fixed_side: String,
    pub caution: String,
}

impl Default for BeginnerDesignProfileV1 {
    fn default() -> Self {
        Self {
            schema_version: BEGINNER_DESIGN_PROFILE_SCHEMA_VERSION_V1,
            preset: BeginnerDesignPresetV1::Balanced,
            shape_fidelity_weight: 35,
            foldability_weight: 35,
            step_count_weight: 15,
            paper_efficiency_weight: 15,
            generation_constraints: BeginnerGenerationConstraintsV1::default(),
            generation_provenance: None,
            reference_surface_landmarks_tenths_mm: None,
            outline_edit_authority: None,
            archived_reference_model_asset_ids: Vec::new(),
        }
    }
}

impl BeginnerDesignProfileV1 {
    #[must_use]
    pub fn is_default(&self) -> bool {
        self == &Self::default()
    }
}

#[must_use]
pub fn validate_beginner_design_profile_v1(profile: &BeginnerDesignProfileV1) -> bool {
    profile.schema_version == BEGINNER_DESIGN_PROFILE_SCHEMA_VERSION_V1
        && profile.shape_fidelity_weight as u16
            + profile.foldability_weight as u16
            + profile.step_count_weight as u16
            + profile.paper_efficiency_weight as u16
            == 100
        && validate_beginner_generation_constraints_v1(&profile.generation_constraints)
        && profile
            .generation_provenance
            .as_ref()
            .is_none_or(validate_beginner_generation_provenance_v1)
        && profile
            .reference_surface_landmarks_tenths_mm
            .as_ref()
            .is_none_or(|landmarks| !landmarks.is_empty() && landmarks.len() <= 256)
        && profile
            .outline_edit_authority
            .as_ref()
            .is_none_or(|authority| {
                authority.schema_version == 1
                    && !authority.edits.is_empty()
                    && authority.edits.len() <= 8
                    && authority.edits.iter().all(|edit| match edit {
                        BeginnerOutlineEditRecordV1::SplitVertical { fragment_kinds, .. } => {
                            fragment_kinds[0] != fragment_kinds[1]
                        }
                        BeginnerOutlineEditRecordV1::Merge {
                            source_candidate_ids,
                            ..
                        } => source_candidate_ids[0] < source_candidate_ids[1],
                    })
            })
        && profile.archived_reference_model_asset_ids.len() <= 8
        && profile
            .archived_reference_model_asset_ids
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len()
            == profile.archived_reference_model_asset_ids.len()
}

/// Validates the bounded, versioned provenance independently of its profile.
#[must_use]
pub fn validate_beginner_generation_provenance_v1(
    provenance: &BeginnerGenerationProvenanceV1,
) -> bool {
    provenance.schema_version == 1
        && provenance.confidence_score <= 100
        && !provenance.source_asset_fingerprint.is_empty()
        && provenance.source_asset_fingerprint.len() <= 128
        && provenance.confidence_reasons.len() <= 8
        && provenance
            .confidence_reasons
            .iter()
            .all(|reason| !reason.is_empty() && reason.len() <= 64)
        && provenance
            .semantic_landmark_provenance
            .as_ref()
            .is_none_or(|semantic| {
                let (expected_roles, hash_domain): (&[&str], &[u8]) =
                    match semantic.ordered_bindings.len() {
                        10 => (
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
                        4 => (
                            &["head", "tail", "fin_left", "fin_right"],
                            b"ORIGAMI2_ASYMMETRIC_FISH_RAY_GROUP_V1",
                        ),
                        _ => return false,
                    };
                semantic.schema_version == 1
                    && semantic
                        .ordered_bindings
                        .iter()
                        .enumerate()
                        .all(|(index, binding)| {
                            usize::from(binding.ordinal) == index
                                && binding.role == expected_roles[index]
                                && binding.physical_ray < 4
                        })
                    && semantic.physical_ray_group_sha256.iter().enumerate().all(
                        |(physical_ray, actual)| {
                            let mut hash = Sha256::new();
                            hash.update(hash_domain);
                            hash.update([physical_ray as u8]);
                            for binding in semantic
                                .ordered_bindings
                                .iter()
                                .filter(|binding| usize::from(binding.physical_ray) == physical_ray)
                            {
                                hash.update([binding.ordinal]);
                                hash.update(binding.role.as_bytes());
                            }
                            <[u8; 32]>::from(hash.finalize()) == *actual
                        },
                    )
            })
        && provenance.generic_tree.as_ref().is_none_or(|tree| {
            tree.schema_version == 1
                && tree.generator_version == 1
                && !tree.authorizes_apply
                && !tree.normalized_length_ratios.is_empty()
                && tree.normalized_length_ratios.len() <= 16
                && tree
                    .normalized_length_ratios
                    .iter()
                    .all(|ratio| *ratio >= 1_000_000)
                && tree.instruction_proposal.as_ref().is_none_or(|proposal| {
                    proposal.schema_version == 1
                        && proposal.generator_version == 1
                        && !proposal.authorizes_apply
                        && !proposal.physical_motion_proof
                        && proposal.topology_sha256 == tree.tree_topology_sha256
                        && !proposal.steps.is_empty()
                        && proposal.steps.len() <= 16
                        && proposal.steps.windows(2).all(|pair| {
                            (pair[0].tree_depth, &pair[0].canonical_crease_id)
                                < (pair[1].tree_depth, &pair[1].canonical_crease_id)
                        })
                        && proposal.steps.iter().all(|step| {
                            !step.canonical_crease_id.is_empty()
                                && step.canonical_crease_id.len() <= 64
                                && matches!(step.assignment.as_str(), "mountain" | "valley")
                                && !step.target_branch.is_empty()
                                && step.target_branch.len() <= 96
                                && matches!(step.fixed_side.as_str(), "root" | "leaf")
                                && !step.caution.is_empty()
                                && step.caution.len() <= 256
                        })
                })
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile_with_generic_tree() -> BeginnerDesignProfileV1 {
        BeginnerDesignProfileV1 {
            generation_provenance: Some(BeginnerGenerationProvenanceV1 {
                schema_version: 1,
                topology_authority_sha256: [1; 32],
                fold_path_certificate_sha256: None,
                confidence_score: 80,
                confidence_reasons: vec!["bounded_tree".into()],
                explicit_override: false,
                source_asset_fingerprint: "asset:version".into(),
                semantic_landmark_provenance: None,
                generic_tree: Some(BeginnerGenericTreeProvenanceV1 {
                    schema_version: 1,
                    source: BeginnerGenericTreeSourceV1::ImageSilhouette,
                    asset_content_sha256: Some([2; 32]),
                    tree_topology_sha256: [3; 32],
                    normalized_length_ratios: vec![1_000_000, 1_500_000],
                    orientation: BeginnerGenericTreeOrientationV1::Horizontal,
                    generator_version: 1,
                    authorizes_apply: false,
                    instruction_proposal: Some(BeginnerGenericTreeInstructionProposalV1 {
                        schema_version: 1,
                        topology_sha256: [3; 32],
                        generator_version: 1,
                        authorizes_apply: false,
                        physical_motion_proof: false,
                        steps: vec![BeginnerGenericTreeInstructionStepV1 {
                            canonical_crease_id: "tree-river-0000".into(),
                            tree_depth: 0,
                            assignment: "valley".into(),
                            target_branch: "branch-0000".into(),
                            fixed_side: "root".into(),
                            caution: "Read-only; no motion proof.".into(),
                        }],
                    }),
                }),
            }),
            ..BeginnerDesignProfileV1::default()
        }
    }

    fn profile_with_edit(edit: BeginnerOutlineEditRecordV1) -> BeginnerDesignProfileV1 {
        BeginnerDesignProfileV1 {
            outline_edit_authority: Some(BeginnerOutlineEditAuthorityV1 {
                schema_version: 1,
                source_asset_id: AssetId::new(),
                source_sha256: [7; 32],
                edits: vec![edit],
            }),
            ..BeginnerDesignProfileV1::default()
        }
    }

    #[test]
    fn outline_edit_authority_round_trips_with_profile() {
        let profile = profile_with_edit(BeginnerOutlineEditRecordV1::SplitVertical {
            source_candidate_id: 2,
            split_x: 41,
            fragment_kinds: [
                BeginnerTargetPartKindV1::Head,
                BeginnerTargetPartKindV1::Torso,
            ],
        });
        assert!(validate_beginner_design_profile_v1(&profile));

        let json = serde_json::to_string(&profile).expect("serialize profile");
        let decoded: BeginnerDesignProfileV1 =
            serde_json::from_str(&json).expect("deserialize profile");
        assert_eq!(decoded, profile);
    }

    #[test]
    fn outline_edit_authority_rejects_ambiguous_split_and_unordered_merge() {
        let ambiguous = profile_with_edit(BeginnerOutlineEditRecordV1::SplitVertical {
            source_candidate_id: 2,
            split_x: 41,
            fragment_kinds: [
                BeginnerTargetPartKindV1::Head,
                BeginnerTargetPartKindV1::Head,
            ],
        });
        assert!(!validate_beginner_design_profile_v1(&ambiguous));

        let unordered = profile_with_edit(BeginnerOutlineEditRecordV1::Merge {
            source_candidate_ids: [4, 1],
            merged_kind: BeginnerTargetPartKindV1::Wing,
        });
        assert!(!validate_beginner_design_profile_v1(&unordered));
    }

    #[test]
    fn generic_tree_provenance_round_trips_without_apply_authority() {
        let profile = profile_with_generic_tree();
        assert!(validate_beginner_design_profile_v1(&profile));
        let json = serde_json::to_string(&profile).expect("serialize profile");
        let decoded: BeginnerDesignProfileV1 =
            serde_json::from_str(&json).expect("deserialize profile");
        assert_eq!(decoded, profile);
        assert!(
            !decoded
                .generation_provenance
                .and_then(|value| value.generic_tree)
                .expect("generic tree provenance")
                .authorizes_apply
        );
    }

    #[test]
    fn generic_tree_provenance_strictly_rejects_tampering() {
        for mutate in [
            |tree: &mut BeginnerGenericTreeProvenanceV1| tree.schema_version = 2,
            |tree: &mut BeginnerGenericTreeProvenanceV1| tree.generator_version = 2,
            |tree: &mut BeginnerGenericTreeProvenanceV1| tree.authorizes_apply = true,
            |tree: &mut BeginnerGenericTreeProvenanceV1| tree.normalized_length_ratios[0] = 0,
        ] {
            let mut profile = profile_with_generic_tree();
            mutate(
                profile
                    .generation_provenance
                    .as_mut()
                    .and_then(|value| value.generic_tree.as_mut())
                    .expect("generic tree provenance"),
            );
            assert!(!validate_beginner_design_profile_v1(&profile));
        }
    }

    #[test]
    fn legacy_generation_provenance_without_generic_tree_remains_compatible() {
        let mut json =
            serde_json::to_value(profile_with_generic_tree()).expect("serialize profile");
        json["generation_provenance"]
            .as_object_mut()
            .expect("generation provenance")
            .remove("generic_tree");
        let decoded: BeginnerDesignProfileV1 =
            serde_json::from_value(json).expect("deserialize legacy profile");
        assert!(validate_beginner_design_profile_v1(&decoded));
        assert!(
            decoded
                .generation_provenance
                .expect("generation provenance")
                .generic_tree
                .is_none()
        );
    }
}
