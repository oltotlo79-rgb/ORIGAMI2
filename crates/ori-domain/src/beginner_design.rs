use serde::{Deserialize, Serialize};

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
pub const fn validate_beginner_design_profile_v1(profile: &BeginnerDesignProfileV1) -> bool {
    profile.schema_version == BEGINNER_DESIGN_PROFILE_SCHEMA_VERSION_V1
        && profile.shape_fidelity_weight as u16
            + profile.foldability_weight as u16
            + profile.step_count_weight as u16
            + profile.paper_efficiency_weight as u16
            == 100
}
