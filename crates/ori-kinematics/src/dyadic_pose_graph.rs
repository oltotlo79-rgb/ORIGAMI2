use ori_domain::EdgeId;
use thiserror::Error;

use crate::{CanonicalHingeAngles, HingeAngle};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DyadicPoseGraphLimitsV1 {
    pub max_states: usize,
    pub max_transitions: usize,
}

impl Default for DyadicPoseGraphLimitsV1 {
    fn default() -> Self {
        Self {
            max_states: 32,
            max_transitions: 64,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DyadicPoseGraphTransitionV1 {
    pub source_state: usize,
    pub target_state: usize,
    pub moving_hinge: EdgeId,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GeneratedDyadicPoseGraphV1 {
    states: Vec<CanonicalHingeAngles>,
    transitions: Vec<DyadicPoseGraphTransitionV1>,
    source_state: usize,
    target_state: usize,
}

impl GeneratedDyadicPoseGraphV1 {
    pub fn states(&self) -> &[CanonicalHingeAngles] {
        &self.states
    }
    pub fn transitions(&self) -> &[DyadicPoseGraphTransitionV1] {
        &self.transitions
    }
    pub const fn source_state(&self) -> usize {
        self.source_state
    }
    pub const fn target_state(&self) -> usize {
        self.target_state
    }
    pub const fn authorizes_project_mutation(&self) -> bool {
        false
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum DyadicPoseGraphGenerationErrorV1 {
    #[error("source and target hinge vectors do not have the same canonical binding")]
    BindingMismatch,
    #[error("dyadic pose graph generation exceeded its resource limit")]
    ResourceLimit,
    #[error("dyadic pose graph generation was cancelled")]
    Cancelled,
}

pub fn generate_bounded_collective_pose_graph_v1(
    source: &CanonicalHingeAngles,
    midpoint: &CanonicalHingeAngles,
    target: &CanonicalHingeAngles,
) -> Result<GeneratedDyadicPoseGraphV1, DyadicPoseGraphGenerationErrorV1> {
    if source.as_slice().len() != target.as_slice().len()
        || source.as_slice().len() != midpoint.as_slice().len()
        || source
            .as_slice()
            .iter()
            .zip(midpoint.as_slice())
            .zip(target.as_slice())
            .any(|((source, midpoint), target)| {
                source.edge() != midpoint.edge() || source.edge() != target.edge()
            })
    {
        return Err(DyadicPoseGraphGenerationErrorV1::BindingMismatch);
    }
    let moving_hinge = source
        .as_slice()
        .iter()
        .zip(target.as_slice())
        .find(|(source, target)| {
            source.angle_degrees().to_bits() != target.angle_degrees().to_bits()
        })
        .map(|(source, _)| source.edge())
        .ok_or(DyadicPoseGraphGenerationErrorV1::BindingMismatch)?;
    Ok(GeneratedDyadicPoseGraphV1 {
        states: vec![source.clone(), midpoint.clone(), target.clone()],
        transitions: vec![
            DyadicPoseGraphTransitionV1 {
                source_state: 0,
                target_state: 1,
                moving_hinge,
            },
            DyadicPoseGraphTransitionV1 {
                source_state: 1,
                target_state: 0,
                moving_hinge,
            },
            DyadicPoseGraphTransitionV1 {
                source_state: 1,
                target_state: 2,
                moving_hinge,
            },
            DyadicPoseGraphTransitionV1 {
                source_state: 2,
                target_state: 1,
                moving_hinge,
            },
        ],
        source_state: 0,
        target_state: 2,
    })
}

/// Generates a canonical three-level grid (source, midpoint, target) over the
/// complete live hinge vector. Edges are observation-only candidates: callers
/// must run the existing continuous transition oracle for every edge before
/// admitting it to a certified path search.
pub fn generate_bounded_dyadic_pose_graph_v1(
    source: &CanonicalHingeAngles,
    target: &CanonicalHingeAngles,
    limits: DyadicPoseGraphLimitsV1,
    checkpoint: impl FnMut() -> bool,
) -> Result<GeneratedDyadicPoseGraphV1, DyadicPoseGraphGenerationErrorV1> {
    generate_bounded_dyadic_pose_graph_at_levels_v1(source, target, 3, limits, checkpoint)
}

pub fn generate_bounded_dyadic_pose_graph_at_levels_v1(
    source: &CanonicalHingeAngles,
    target: &CanonicalHingeAngles,
    level_count: usize,
    limits: DyadicPoseGraphLimitsV1,
    mut checkpoint: impl FnMut() -> bool,
) -> Result<GeneratedDyadicPoseGraphV1, DyadicPoseGraphGenerationErrorV1> {
    if !checkpoint() {
        return Err(DyadicPoseGraphGenerationErrorV1::Cancelled);
    }
    if !matches!(level_count, 3 | 5 | 9)
        || source.as_slice().len() != target.as_slice().len()
        || source.as_slice().is_empty()
        || source
            .as_slice()
            .iter()
            .zip(target.as_slice())
            .any(|(a, b)| a.edge() != b.edge())
    {
        return Err(DyadicPoseGraphGenerationErrorV1::BindingMismatch);
    }
    let moving = source
        .as_slice()
        .iter()
        .zip(target.as_slice())
        .enumerate()
        .filter_map(|(index, (source, target))| {
            (source.angle_degrees().to_bits() != target.angle_degrees().to_bits()).then_some(index)
        })
        .collect::<Vec<_>>();
    let hinge_count = moving.len();
    let state_count = level_count
        .checked_pow(hinge_count as u32)
        .ok_or(DyadicPoseGraphGenerationErrorV1::ResourceLimit)?;
    let transition_count = state_count
        .checked_mul(hinge_count)
        .and_then(|v| v.checked_mul(2 * (level_count - 1)))
        .map(|v| v / level_count)
        .ok_or(DyadicPoseGraphGenerationErrorV1::ResourceLimit)?;
    if state_count > limits.max_states || transition_count > limits.max_transitions {
        return Err(DyadicPoseGraphGenerationErrorV1::ResourceLimit);
    }
    let mut states = Vec::with_capacity(state_count);
    for state_index in 0..state_count {
        if !checkpoint() {
            return Err(DyadicPoseGraphGenerationErrorV1::Cancelled);
        }
        let mut digits = state_index;
        let entries = source
            .as_slice()
            .iter()
            .zip(target.as_slice())
            .enumerate()
            .map(|(index, (source, target))| {
                let level = if moving.contains(&index) {
                    let level = digits % level_count;
                    digits /= level_count;
                    level
                } else {
                    0
                };
                let parameter = level as f64 / (level_count - 1) as f64;
                let angle = source.angle_degrees()
                    + (target.angle_degrees() - source.angle_degrees()) * parameter;
                HingeAngle::new(source.edge(), angle)
                    .expect("midpoint of admitted angles is admitted")
            })
            .collect();
        states.push(CanonicalHingeAngles::new(entries).expect("source order remains canonical"));
    }
    let mut transitions = Vec::with_capacity(transition_count);
    for source_state in 0..state_count {
        for (moving_index, hinge_index) in moving.iter().copied().enumerate() {
            if !checkpoint() {
                return Err(DyadicPoseGraphGenerationErrorV1::Cancelled);
            }
            let stride = level_count.pow(moving_index as u32);
            let level = (source_state / stride) % level_count;
            for target_level in [
                level.checked_sub(1),
                (level + 1 < level_count).then_some(level + 1),
            ]
            .into_iter()
            .flatten()
            {
                transitions.push(DyadicPoseGraphTransitionV1 {
                    source_state,
                    target_state: source_state - level * stride + target_level * stride,
                    moving_hinge: source.as_slice()[hinge_index].edge(),
                });
            }
        }
    }
    transitions.sort_unstable_by_key(|edge| {
        (
            edge.source_state,
            edge.target_state,
            edge.moving_hinge.canonical_bytes(),
        )
    });
    Ok(GeneratedDyadicPoseGraphV1 {
        states,
        transitions,
        source_state: 0,
        target_state: state_count - 1,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn angles(edges: [EdgeId; 2], values: [f64; 2]) -> CanonicalHingeAngles {
        let mut entries = edges
            .into_iter()
            .zip(values)
            .map(|(edge, value)| HingeAngle::new(edge, value).unwrap())
            .collect::<Vec<_>>();
        entries.sort_unstable_by_key(|entry| entry.edge().canonical_bytes());
        CanonicalHingeAngles::new(entries).unwrap()
    }

    #[test]
    fn two_hinge_grid_is_canonical_bounded_and_cancellable() {
        let edges = [EdgeId::new(), EdgeId::new()];
        let source = angles(edges, [0.0, 10.0]);
        let target = angles(edges, [90.0, 70.0]);
        let graph = generate_bounded_dyadic_pose_graph_v1(
            &source,
            &target,
            DyadicPoseGraphLimitsV1::default(),
            || true,
        )
        .unwrap();
        assert_eq!(graph.states().len(), 9);
        assert_eq!(graph.transitions().len(), 24);
        for (levels, states, transitions) in [(3, 9, 24), (5, 25, 80), (9, 81, 288)] {
            let selected = generate_bounded_dyadic_pose_graph_at_levels_v1(
                &source,
                &target,
                levels,
                DyadicPoseGraphLimitsV1 {
                    max_states: states,
                    max_transitions: transitions,
                },
                || true,
            )
            .unwrap();
            assert_eq!(selected.states().len(), states);
            assert_eq!(selected.transitions().len(), transitions);
            assert_eq!(
                generate_bounded_dyadic_pose_graph_at_levels_v1(
                    &source,
                    &target,
                    levels,
                    DyadicPoseGraphLimitsV1 {
                        max_states: states.saturating_sub(1),
                        max_transitions: transitions,
                    },
                    || true,
                ),
                Err(DyadicPoseGraphGenerationErrorV1::ResourceLimit)
            );
        }
        assert_eq!(
            generate_bounded_dyadic_pose_graph_at_levels_v1(
                &source,
                &target,
                7,
                DyadicPoseGraphLimitsV1::default(),
                || true,
            ),
            Err(DyadicPoseGraphGenerationErrorV1::BindingMismatch)
        );
        assert!(!graph.authorizes_project_mutation());
        let repeated = generate_bounded_dyadic_pose_graph_v1(
            &source,
            &target,
            DyadicPoseGraphLimitsV1::default(),
            || true,
        )
        .unwrap();
        assert_eq!(graph, repeated);
        assert_eq!(
            generate_bounded_dyadic_pose_graph_v1(
                &source,
                &target,
                DyadicPoseGraphLimitsV1 {
                    max_states: 8,
                    max_transitions: 64
                },
                || true
            ),
            Err(DyadicPoseGraphGenerationErrorV1::ResourceLimit)
        );
        assert_eq!(
            generate_bounded_dyadic_pose_graph_v1(
                &source,
                &target,
                DyadicPoseGraphLimitsV1::default(),
                || false
            ),
            Err(DyadicPoseGraphGenerationErrorV1::Cancelled)
        );
        let tampered = angles([edges[0], EdgeId::new()], [90.0, 70.0]);
        assert_eq!(
            generate_bounded_dyadic_pose_graph_v1(
                &source,
                &tampered,
                DyadicPoseGraphLimitsV1::default(),
                || true
            ),
            Err(DyadicPoseGraphGenerationErrorV1::BindingMismatch)
        );
    }

    #[test]
    fn three_hinge_level_caps_are_exact_and_fail_before_allocation() {
        let mut edges = [EdgeId::new(), EdgeId::new(), EdgeId::new()];
        edges.sort_unstable_by_key(EdgeId::canonical_bytes);
        let vector = |values: [f64; 3]| {
            CanonicalHingeAngles::new(
                edges
                    .into_iter()
                    .zip(values)
                    .map(|(edge, value)| HingeAngle::new(edge, value).unwrap())
                    .collect(),
            )
            .unwrap()
        };
        let source = vector([0.0, 0.0, 0.0]);
        let target = vector([30.0, 60.0, 90.0]);
        for (levels, states, transitions) in [(3, 27, 108), (5, 125, 600)] {
            let graph = generate_bounded_dyadic_pose_graph_at_levels_v1(
                &source,
                &target,
                levels,
                DyadicPoseGraphLimitsV1 {
                    max_states: states,
                    max_transitions: transitions,
                },
                || true,
            )
            .unwrap();
            assert_eq!(graph.states().len(), states);
            assert_eq!(graph.transitions().len(), transitions);
        }
        assert_eq!(
            generate_bounded_dyadic_pose_graph_at_levels_v1(
                &source,
                &target,
                5,
                DyadicPoseGraphLimitsV1 {
                    max_states: 124,
                    max_transitions: 600,
                },
                || true,
            ),
            Err(DyadicPoseGraphGenerationErrorV1::ResourceLimit)
        );
        assert_eq!(
            generate_bounded_dyadic_pose_graph_at_levels_v1(
                &source,
                &target,
                9,
                DyadicPoseGraphLimitsV1 {
                    max_states: 125,
                    max_transitions: 600,
                },
                || true,
            ),
            Err(DyadicPoseGraphGenerationErrorV1::ResourceLimit)
        );
    }

    #[test]
    fn four_hinge_three_level_cap_is_exact_and_checked_before_allocation() {
        let mut edges = [EdgeId::new(), EdgeId::new(), EdgeId::new(), EdgeId::new()];
        edges.sort_unstable_by_key(EdgeId::canonical_bytes);
        let vector = |values: [f64; 4]| {
            CanonicalHingeAngles::new(
                edges
                    .into_iter()
                    .zip(values)
                    .map(|(edge, value)| HingeAngle::new(edge, value).unwrap())
                    .collect(),
            )
            .unwrap()
        };
        let source = vector([0.0, 0.0, 0.0, 0.0]);
        let target = vector([30.0, 60.0, 90.0, 120.0]);
        let graph = generate_bounded_dyadic_pose_graph_at_levels_v1(
            &source,
            &target,
            3,
            DyadicPoseGraphLimitsV1 {
                max_states: 81,
                max_transitions: 432,
            },
            || true,
        )
        .unwrap();
        assert_eq!(graph.states().len(), 81);
        assert_eq!(graph.transitions().len(), 432);
        assert_eq!(
            generate_bounded_dyadic_pose_graph_at_levels_v1(
                &source,
                &target,
                3,
                DyadicPoseGraphLimitsV1 {
                    max_states: 80,
                    max_transitions: 432,
                },
                || true,
            ),
            Err(DyadicPoseGraphGenerationErrorV1::ResourceLimit)
        );
        assert_eq!(
            generate_bounded_dyadic_pose_graph_at_levels_v1(
                &source,
                &target,
                3,
                DyadicPoseGraphLimitsV1 {
                    max_states: 81,
                    max_transitions: 431,
                },
                || true,
            ),
            Err(DyadicPoseGraphGenerationErrorV1::ResourceLimit)
        );
        assert_eq!(
            generate_bounded_dyadic_pose_graph_at_levels_v1(
                &source,
                &target,
                3,
                DyadicPoseGraphLimitsV1 {
                    max_states: 81,
                    max_transitions: 432,
                },
                || false,
            ),
            Err(DyadicPoseGraphGenerationErrorV1::Cancelled)
        );
        let mismatched = vector([30.0, 60.0, 90.0, 120.0]);
        let mut mismatched_entries = mismatched.as_slice().to_vec();
        mismatched_entries[0] = HingeAngle::new(EdgeId::new(), 30.0).unwrap();
        mismatched_entries.sort_unstable_by_key(|entry| entry.edge().canonical_bytes());
        let mismatched = CanonicalHingeAngles::new(mismatched_entries).unwrap();
        assert_eq!(
            generate_bounded_dyadic_pose_graph_at_levels_v1(
                &source,
                &mismatched,
                3,
                DyadicPoseGraphLimitsV1 {
                    max_states: 81,
                    max_transitions: 432,
                },
                || true,
            ),
            Err(DyadicPoseGraphGenerationErrorV1::BindingMismatch)
        );
    }
}
