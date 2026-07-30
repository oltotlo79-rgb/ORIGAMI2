//! Strict wire contracts and pure admission checks for cycle-pose proposals.
//!
//! Native analysis, proof construction, progress emission, and transaction
//! installation remain in the parent module. This module owns only bounded
//! deserialization, response DTOs, and deterministic request validation.

use ori_domain::{FaceId, ProjectId};
use serde::{Deserialize, Serialize};

use super::{
    CANCELLED_MESSAGE, CYCLE_PATH_RESOURCE_MESSAGE, CYCLE_PATH_UNCERTIFIED_MESSAGE,
    CYCLE_PATH_UNSUPPORTED_MESSAGE, CycleScheduleRequestV1, INVALID_REQUEST_MESSAGE,
    MAX_STACKED_FOLD_ATOMIC_PATH_TRANSITIONS_V1, MAX_STACKED_FOLD_REQUEST_HINGES_V1,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct LinearCandidateRequestV1 {
    pub(super) version: u32,
    pub(super) entries: Vec<LinearCandidateEntryRequestV1>,
    #[serde(default)]
    pub(super) exact_dyadic_path_v1: Option<ExactDyadicPathRequestV1>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct ExactDyadicPathRequestV1 {
    pub(super) version: u32,
    pub(super) segments: Vec<ExactDyadicSegmentRequestV1>,
    pub(super) max_pair_tests: usize,
    pub(super) max_denominator_power: u32,
    pub(super) max_integer_bits: usize,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct ExactDyadicSegmentRequestV1 {
    pub(super) start: ExactDyadicPointRequestV1,
    pub(super) end: ExactDyadicPointRequestV1,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct ExactDyadicPointRequestV1 {
    pub(super) x_numerator: i128,
    pub(super) y_numerator: i128,
    pub(super) denominator_power: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct LinearCandidateEntryRequestV1 {
    pub(super) edge: ori_domain::EdgeId,
    pub(super) initial_angle_degrees: f64,
    pub(super) requested_angle_degrees: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct CertifiedPathGraphRequestV1 {
    pub(super) version: u32,
    pub(super) states: Vec<CertifiedPathGraphStateRequestV1>,
    pub(super) transitions: Vec<CertifiedPathGraphTransitionRequestV1>,
    pub(super) source_state: usize,
    pub(super) target_state: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct CertifiedPathGraphStateRequestV1 {
    pub(super) entries: Vec<CertifiedPathGraphAngleRequestV1>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct CertifiedPathGraphAngleRequestV1 {
    pub(super) edge: ori_domain::EdgeId,
    pub(super) angle_degrees: f64,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct CertifiedPathGraphTransitionRequestV1 {
    pub(super) source_state: usize,
    pub(super) target_state: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CurrentCyclePosePreviewRequestV1 {
    #[serde(default)]
    pub(super) progress_request_id: Option<String>,
    pub(super) expected_project_instance_id: ProjectId,
    pub(super) expected_project_id: ProjectId,
    pub(super) expected_revision: u64,
    pub(super) cycle_schedule_v1: CycleScheduleRequestV1,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CurrentCyclePosePreviewResponseV1 {
    pub(super) version: u32,
    pub(super) transaction_token: ProjectId,
    pub(super) source_revision: u64,
    pub(super) target_revision: u64,
    pub(super) closure_leaf_count: usize,
    pub(super) closure_max_depth: u32,
    pub(super) checked_hinge_count: usize,
    pub(super) total_hinge_count: usize,
    pub(super) continuous_path_certified: bool,
    pub(super) continuous_layer_transport_model_id: Option<&'static str>,
    pub(super) continuous_layer_transition_count: usize,
    pub(super) continuous_layer_pair_order_count: usize,
    pub(super) continuous_layer_target_order_sha256: Option<String>,
    pub(super) source_layer_order: Vec<LayerOrderPairDtoV1>,
    pub(super) target_layer_order: Vec<LayerOrderPairDtoV1>,
    pub(super) authorizes_project_mutation: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct LayerOrderPairDtoV1 {
    pub(super) lower_face: FaceId,
    pub(super) upper_face: FaceId,
}

pub(super) fn validate_progress_request_id_v1(value: Option<&str>) -> Result<Option<&str>, String> {
    match value {
        Some(value)
            if value.is_empty()
                || value.len() > 128
                || !value.bytes().all(|byte| byte.is_ascii_graphic()) =>
        {
            Err(INVALID_REQUEST_MESSAGE.to_owned())
        }
        value => Ok(value),
    }
}

pub(super) fn validate_certified_path_graph_v1(
    request: &CertifiedPathGraphRequestV1,
    live: &ori_kinematics::CanonicalHingeAngles,
) -> Result<Vec<ori_kinematics::CanonicalHingeAngles>, &'static str> {
    if request.version != 1
        || request.states.is_empty()
        || request.states.len() > ori_collision::MAX_CERTIFIED_PATH_GRAPH_STATES_V1
        || request.transitions.is_empty()
        || request.transitions.len() > MAX_STACKED_FOLD_ATOMIC_PATH_TRANSITIONS_V1
        || request.states.iter().any(|state| {
            state.entries.is_empty() || state.entries.len() > MAX_STACKED_FOLD_REQUEST_HINGES_V1
        })
    {
        return Err(CYCLE_PATH_RESOURCE_MESSAGE);
    }
    if request.source_state != 0
        || request.target_state >= request.states.len()
        || request.target_state == request.source_state
    {
        return Err(CYCLE_PATH_UNSUPPORTED_MESSAGE);
    }
    let mut states = Vec::with_capacity(request.states.len());
    for state in &request.states {
        let angles = ori_kinematics::CanonicalHingeAngles::new(
            state
                .entries
                .iter()
                .map(|entry| ori_kinematics::HingeAngle::new(entry.edge, entry.angle_degrees))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| CYCLE_PATH_UNSUPPORTED_MESSAGE)?,
        )
        .map_err(|_| CYCLE_PATH_UNSUPPORTED_MESSAGE)?;
        if angles.as_slice().len() != live.as_slice().len() {
            return Err(CYCLE_PATH_UNSUPPORTED_MESSAGE);
        }
        states.push(angles);
    }
    if states.first() != Some(live)
        || states.iter().enumerate().any(|(index, state)| {
            states[..index]
                .iter()
                .any(|previous| previous.as_slice() == state.as_slice())
        })
    {
        return Err(CYCLE_PATH_UNSUPPORTED_MESSAGE);
    }
    let mut canonical_edges = request
        .transitions
        .iter()
        .map(|edge| (edge.source_state, edge.target_state))
        .collect::<Vec<_>>();
    if canonical_edges.iter().any(|(source, target)| {
        *source >= states.len() || *target >= states.len() || source == target
    }) {
        return Err(CYCLE_PATH_UNSUPPORTED_MESSAGE);
    }
    canonical_edges.sort_unstable();
    if canonical_edges.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(CYCLE_PATH_UNSUPPORTED_MESSAGE);
    }
    Ok(states)
}

pub(super) fn validate_linear_candidate_angles_v1(
    request: &LinearCandidateRequestV1,
    live: &ori_kinematics::CanonicalHingeAngles,
) -> Result<
    (
        ori_kinematics::CanonicalHingeAngles,
        ori_kinematics::CanonicalHingeAngles,
    ),
    (),
> {
    if request.version != 1 {
        return Err(());
    }
    let collect = |requested: bool| {
        ori_kinematics::CanonicalHingeAngles::new(
            request
                .entries
                .iter()
                .map(|entry| {
                    ori_kinematics::HingeAngle::new(
                        entry.edge,
                        if requested {
                            entry.requested_angle_degrees
                        } else {
                            entry.initial_angle_degrees
                        },
                    )
                })
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| ())?,
        )
        .map_err(|_| ())
    };
    let initial = collect(false)?;
    if initial != *live {
        return Err(());
    }
    Ok((initial, collect(true)?))
}

pub(super) fn validate_exact_dyadic_candidate_path_v1(
    request: &ExactDyadicPathRequestV1,
) -> Result<(), &'static str> {
    if request.version != 1 || request.segments.is_empty() {
        return Err(CYCLE_PATH_RESOURCE_MESSAGE);
    }
    let segments = request
        .segments
        .iter()
        .map(|segment| {
            let point = |value: ExactDyadicPointRequestV1| ori_collision::DyadicPointV1 {
                x_numerator: value.x_numerator,
                y_numerator: value.y_numerator,
                denominator_power: value.denominator_power,
            };
            ori_collision::DyadicSegmentV1 {
                start: point(segment.start),
                end: point(segment.end),
            }
        })
        .collect::<Vec<_>>();
    match ori_collision::classify_exact_dyadic_path_self_intersection_v1(
        &segments,
        ori_collision::ExactDyadicIntersectionLimitsV1 {
            max_denominator_power: request.max_denominator_power,
            max_integer_bits: request.max_integer_bits,
        },
        request.max_pair_tests,
    ) {
        Ok(None) => Ok(()),
        Ok(Some(_)) => Err(CYCLE_PATH_UNCERTIFIED_MESSAGE),
        Err(ori_collision::ExactDyadicPathIntersectionErrorV1::ResourceLimit) => {
            Err(CYCLE_PATH_RESOURCE_MESSAGE)
        }
        Err(ori_collision::ExactDyadicPathIntersectionErrorV1::Cancelled) => Err(CANCELLED_MESSAGE),
        Err(ori_collision::ExactDyadicPathIntersectionErrorV1::InvalidSegment) => {
            Err(CYCLE_PATH_UNCERTIFIED_MESSAGE)
        }
    }
}

#[cfg(test)]
#[path = "stacked_fold_cycle_pose_wire_tests.rs"]
mod tests;
