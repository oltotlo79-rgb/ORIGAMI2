use std::collections::HashSet;

use serde::{Deserialize, Serialize};

pub const BEGINNER_GENERATION_CONSTRAINTS_SCHEMA_VERSION_V1: u32 = 1;
pub const MIN_BEGINNER_GENERATION_STEPS_V1: u16 = 1;
pub const MAX_BEGINNER_GENERATION_STEPS_V1: u16 = 500;
pub const MAX_BEGINNER_ALLOWED_TECHNIQUES_V1: usize = 8;

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BeginnerGenerationConstraintsV1 {
    pub schema_version: u32,
    pub maximum_steps: u16,
    pub detail_level: BeginnerDetailLevelV1,
    #[serde(default)]
    pub target_category: Option<BeginnerTargetCategoryV1>,
    pub allowed_techniques: Vec<BeginnerFoldTechniqueV1>,
}

impl Default for BeginnerGenerationConstraintsV1 {
    fn default() -> Self {
        Self {
            schema_version: BEGINNER_GENERATION_CONSTRAINTS_SCHEMA_VERSION_V1,
            maximum_steps: 60,
            detail_level: BeginnerDetailLevelV1::Standard,
            target_category: None,
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
    {
        return false;
    }
    let mut unique = HashSet::with_capacity(constraints.allowed_techniques.len());
    constraints
        .allowed_techniques
        .iter()
        .all(|technique| unique.insert(*technique))
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
    }
}
