use serde::{Deserialize, Serialize};

use crate::BeginnerDesignProfileV1;

pub const BEGINNER_CANDIDATE_SCHEMA_VERSION_V1: u32 = 1;
pub const MAX_BEGINNER_CANDIDATES_V1: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BeginnerBulgeTreatmentV1 {
    TargetShapeApproximation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BeginnerElasticityModelV1 {
    NotComputed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BeginnerCandidateKindV1 {
    Recommended,
    ShapeFocused,
    FoldabilityFocused,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BeginnerCandidateScoreV1 {
    pub schema_version: u32,
    pub kind: BeginnerCandidateKindV1,
    pub rank: u8,
    pub total_score: u8,
    pub shape_score: u8,
    pub target_approximation_score: u8,
    pub foldability_score: u8,
    pub step_count_score: u8,
    pub paper_efficiency_score: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BeginnerCandidateInputV1 {
    pub vertex_count: usize,
    pub edge_count: usize,
    pub crease_count: usize,
    pub target_approximation_score: u8,
}

#[must_use]
pub fn score_beginner_candidates_v1(
    input: BeginnerCandidateInputV1,
    profile: &BeginnerDesignProfileV1,
) -> Vec<BeginnerCandidateScoreV1> {
    let complexity = input
        .vertex_count
        .saturating_add(input.edge_count)
        .saturating_add(input.crease_count.saturating_mul(2))
        .min(100);
    let simplicity = 100_u8.saturating_sub(complexity as u8);
    let crease_shape = (50_usize.saturating_add(input.crease_count.min(50))) as u8;
    let shape_base = ((u16::from(crease_shape)
        + u16::from(input.target_approximation_score.min(100)))
        / 2) as u8;
    let variants = [
        (BeginnerCandidateKindV1::Recommended, 0_i16, 0_i16),
        (BeginnerCandidateKindV1::ShapeFocused, 15, -10),
        (BeginnerCandidateKindV1::FoldabilityFocused, -10, 15),
    ];
    let mut candidates = variants
        .into_iter()
        .map(|(kind, shape_delta, foldability_delta)| {
            let shape_score = adjust_score(shape_base, shape_delta);
            let foldability_score = adjust_score(simplicity, foldability_delta);
            let step_count_score = simplicity;
            let paper_efficiency_score =
                100_u8.saturating_sub((input.vertex_count.min(100) / 2) as u8);
            let weighted = u32::from(shape_score) * u32::from(profile.shape_fidelity_weight)
                + u32::from(foldability_score) * u32::from(profile.foldability_weight)
                + u32::from(step_count_score) * u32::from(profile.step_count_weight)
                + u32::from(paper_efficiency_score) * u32::from(profile.paper_efficiency_weight);
            BeginnerCandidateScoreV1 {
                schema_version: BEGINNER_CANDIDATE_SCHEMA_VERSION_V1,
                kind,
                rank: 0,
                total_score: (weighted / 100).min(100) as u8,
                shape_score,
                target_approximation_score: input.target_approximation_score.min(100),
                foldability_score,
                step_count_score,
                paper_efficiency_score,
            }
        })
        .collect::<Vec<_>>();
    candidates.sort_by_key(|candidate| {
        (
            std::cmp::Reverse(candidate.total_score),
            candidate_kind_order(candidate.kind),
        )
    });
    for (index, candidate) in candidates.iter_mut().enumerate() {
        candidate.rank = (index + 1) as u8;
    }
    candidates.truncate(MAX_BEGINNER_CANDIDATES_V1);
    candidates
}

fn adjust_score(score: u8, delta: i16) -> u8 {
    let adjusted = score as i16 + delta;
    if adjusted < 0 {
        0
    } else if adjusted > 100 {
        100
    } else {
        adjusted as u8
    }
}

const fn candidate_kind_order(kind: BeginnerCandidateKindV1) -> u8 {
    match kind {
        BeginnerCandidateKindV1::Recommended => 0,
        BeginnerCandidateKindV1::ShapeFocused => 1,
        BeginnerCandidateKindV1::FoldabilityFocused => 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scoring_is_bounded_deterministic_and_ranked() {
        let input = BeginnerCandidateInputV1 {
            vertex_count: usize::MAX,
            edge_count: usize::MAX,
            crease_count: usize::MAX,
            target_approximation_score: 100,
        };
        let profile = BeginnerDesignProfileV1::default();
        let first = score_beginner_candidates_v1(input, &profile);
        let second = score_beginner_candidates_v1(input, &profile);

        assert_eq!(first, second);
        assert_eq!(first.len(), MAX_BEGINNER_CANDIDATES_V1);
        assert_eq!(
            first
                .iter()
                .map(|candidate| candidate.rank)
                .collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        assert!(
            first
                .windows(2)
                .all(|pair| pair[0].total_score >= pair[1].total_score)
        );
        assert!(first.iter().all(|candidate| candidate.total_score <= 100));
        assert!(
            first
                .iter()
                .all(|candidate| candidate.target_approximation_score == 100)
        );

        let mut unmatched = input;
        unmatched.target_approximation_score = 0;
        let unmatched_scores = score_beginner_candidates_v1(unmatched, &profile);
        assert!(
            first
                .iter()
                .zip(unmatched_scores)
                .all(|(matched, missing)| matched.shape_score > missing.shape_score)
        );
    }
}
