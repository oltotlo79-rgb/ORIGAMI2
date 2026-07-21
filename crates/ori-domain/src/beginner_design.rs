use serde::{Deserialize, Serialize};

use crate::{BeginnerGenerationConstraintsV1, validate_beginner_generation_constraints_v1};

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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BeginnerGenerationProvenanceV1 {
    pub schema_version: u32,
    pub topology_authority_sha256: [u8; 32],
    pub confidence_score: u8,
    pub confidence_reasons: Vec<String>,
    pub explicit_override: bool,
    pub source_asset_fingerprint: String,
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
            .is_none_or(|provenance| {
                provenance.schema_version == 1
                    && provenance.confidence_score <= 100
                    && !provenance.source_asset_fingerprint.is_empty()
                    && provenance.source_asset_fingerprint.len() <= 128
                    && provenance.confidence_reasons.len() <= 8
                    && provenance
                        .confidence_reasons
                        .iter()
                        .all(|reason| !reason.is_empty() && reason.len() <= 64)
            })
}
