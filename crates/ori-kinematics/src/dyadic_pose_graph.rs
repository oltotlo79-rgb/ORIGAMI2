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

/// Generates a canonical three-level grid (source, midpoint, target) over the
/// complete live hinge vector. Edges are observation-only candidates: callers
/// must run the existing continuous transition oracle for every edge before
/// admitting it to a certified path search.
pub fn generate_bounded_dyadic_pose_graph_v1(
    source: &CanonicalHingeAngles,
    target: &CanonicalHingeAngles,
    limits: DyadicPoseGraphLimitsV1,
    mut checkpoint: impl FnMut() -> bool,
) -> Result<GeneratedDyadicPoseGraphV1, DyadicPoseGraphGenerationErrorV1> {
    if !checkpoint() {
        return Err(DyadicPoseGraphGenerationErrorV1::Cancelled);
    }
    if source.as_slice().len() != target.as_slice().len()
        || source.as_slice().is_empty()
        || source
            .as_slice()
            .iter()
            .zip(target.as_slice())
            .any(|(a, b)| a.edge() != b.edge())
    {
        return Err(DyadicPoseGraphGenerationErrorV1::BindingMismatch);
    }
    let hinge_count = source.as_slice().len();
    let state_count = 3usize
        .checked_pow(hinge_count as u32)
        .ok_or(DyadicPoseGraphGenerationErrorV1::ResourceLimit)?;
    let transition_count = state_count
        .checked_mul(hinge_count)
        .and_then(|v| v.checked_mul(4))
        .map(|v| v / 3)
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
            .map(|(source, target)| {
                let level = digits % 3;
                digits /= 3;
                let angle = match level {
                    0 => source.angle_degrees(),
                    1 => (source.angle_degrees() + target.angle_degrees()) * 0.5,
                    _ => target.angle_degrees(),
                };
                HingeAngle::new(source.edge(), angle)
                    .expect("midpoint of admitted angles is admitted")
            })
            .collect();
        states.push(CanonicalHingeAngles::new(entries).expect("source order remains canonical"));
    }
    let mut transitions = Vec::with_capacity(transition_count);
    for source_state in 0..state_count {
        for hinge_index in 0..hinge_count {
            if !checkpoint() {
                return Err(DyadicPoseGraphGenerationErrorV1::Cancelled);
            }
            let stride = 3usize.pow(hinge_index as u32);
            let level = (source_state / stride) % 3;
            for target_level in [level.checked_sub(1), (level < 2).then_some(level + 1)]
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
}
