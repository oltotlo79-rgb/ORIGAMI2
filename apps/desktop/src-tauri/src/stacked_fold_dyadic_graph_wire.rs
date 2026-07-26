//! Strict wire contract and response construction for bounded dyadic graph reads.
//!
//! The native graph search remains in the parent module. This module owns only
//! the deserialized request, serialized response, and the closed response
//! vocabulary shared by every bounded-read exit.

use ori_domain::ProjectId;
use serde::{Deserialize, Serialize};

use super::{CycleScheduleRequestV1, DyadicPoseGraphAngleDtoV1, default_dyadic_level_count_v1};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct DyadicPoseGraphReadRequestV1 {
    pub(super) expected_project_instance_id: ProjectId,
    pub(super) expected_project_id: ProjectId,
    pub(super) expected_revision: u64,
    pub(super) target_angles: Vec<DyadicPoseGraphAngleDtoV1>,
    pub(super) max_states: usize,
    pub(super) max_transitions: usize,
    #[serde(default = "default_dyadic_level_count_v1")]
    pub(super) level_count: usize,
    #[serde(default)]
    pub(super) cycle_schedule_v1: Option<CycleScheduleRequestV1>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DyadicPoseGraphReadResponseV1 {
    version: u32,
    project_instance_id: ProjectId,
    project_id: ProjectId,
    revision: u64,
    status: &'static str,
    reason: &'static str,
    state_count: usize,
    transition_count: usize,
    explored_state_count: usize,
    evaluated_transition_count: usize,
    certified_transition_count: usize,
    certificate_binding_sha256: Option<String>,
    positive_thickness_transition_count: usize,
    positive_thickness_certified: bool,
    positive_thickness_binding_sha256: Option<String>,
    layer_transport_transition_count: usize,
    layer_transport_certified: bool,
    layer_transport_binding_sha256: Option<String>,
    mutation_candidate_ready: bool,
    authorizes_project_mutation: bool,
}

impl DyadicPoseGraphReadResponseV1 {
    pub(super) fn mutation_candidate_ready(&self) -> bool {
        self.mutation_candidate_ready
    }

    pub(super) fn certificate_binding_sha256(&self) -> Option<&str> {
        self.certificate_binding_sha256.as_deref()
    }

    pub(super) fn positive_thickness_binding_sha256(&self) -> Option<&str> {
        self.positive_thickness_binding_sha256.as_deref()
    }

    pub(super) fn layer_transport_binding_sha256(&self) -> Option<&str> {
        self.layer_transport_binding_sha256.as_deref()
    }
}

pub(super) fn unsupported_dyadic_graph_response_v1(
    project: &super::super::ProjectState,
) -> DyadicPoseGraphReadResponseV1 {
    dyadic_graph_response(
        project,
        "unsupported",
        0,
        0,
        0,
        0,
        0,
        None,
        0,
        None,
        0,
        None,
    )
}

pub(super) fn dyadic_graph_response(
    project: &super::super::ProjectState,
    status: &'static str,
    state_count: usize,
    transition_count: usize,
    explored_state_count: usize,
    evaluated_transition_count: usize,
    certified_transition_count: usize,
    certificate_binding_sha256: Option<String>,
    positive_thickness_transition_count: usize,
    positive_thickness_binding_sha256: Option<String>,
    layer_transport_transition_count: usize,
    layer_transport_binding_sha256: Option<String>,
) -> DyadicPoseGraphReadResponseV1 {
    let positive_thickness_certified = certified_transition_count > 0
        && positive_thickness_transition_count == certified_transition_count
        && positive_thickness_binding_sha256.is_some();
    let layer_transport_certified = certified_transition_count > 0
        && layer_transport_transition_count == certified_transition_count
        && layer_transport_binding_sha256.is_some();
    DyadicPoseGraphReadResponseV1 {
        version: 1,
        project_instance_id: project.instance_id,
        project_id: project.project_id,
        revision: project.editor.revision(),
        status,
        reason: match status {
            "certified" if positive_thickness_certified && layer_transport_certified => {
                "proof_complete"
            }
            "certified" => "no_certified_path",
            "no_path" => "no_certified_path",
            "resource_limit" => "bounded_resource_limit",
            "cancelled" => "cancelled",
            _ => "unsupported_geometry",
        },
        state_count,
        transition_count,
        explored_state_count,
        evaluated_transition_count,
        certified_transition_count,
        certificate_binding_sha256,
        positive_thickness_transition_count,
        positive_thickness_certified,
        positive_thickness_binding_sha256,
        layer_transport_transition_count,
        layer_transport_certified,
        layer_transport_binding_sha256,
        mutation_candidate_ready: positive_thickness_certified && layer_transport_certified,
        authorizes_project_mutation: false,
    }
}

#[cfg(test)]
pub(super) struct DyadicPoseGraphReadResponseTestV1 {
    pub(super) project_instance_id: ProjectId,
    pub(super) project_id: ProjectId,
    pub(super) revision: u64,
    pub(super) status: &'static str,
    pub(super) reason: &'static str,
    pub(super) state_count: usize,
    pub(super) transition_count: usize,
    pub(super) explored_state_count: usize,
    pub(super) evaluated_transition_count: usize,
    pub(super) certified_transition_count: usize,
    pub(super) certificate_binding_sha256: Option<String>,
    pub(super) positive_thickness_transition_count: usize,
    pub(super) positive_thickness_certified: bool,
    pub(super) positive_thickness_binding_sha256: Option<String>,
    pub(super) layer_transport_transition_count: usize,
    pub(super) layer_transport_certified: bool,
    pub(super) layer_transport_binding_sha256: Option<String>,
    pub(super) mutation_candidate_ready: bool,
    pub(super) authorizes_project_mutation: bool,
}

#[cfg(test)]
impl DyadicPoseGraphReadResponseV1 {
    pub(super) fn into_test_view(self) -> DyadicPoseGraphReadResponseTestV1 {
        DyadicPoseGraphReadResponseTestV1 {
            project_instance_id: self.project_instance_id,
            project_id: self.project_id,
            revision: self.revision,
            status: self.status,
            reason: self.reason,
            state_count: self.state_count,
            transition_count: self.transition_count,
            explored_state_count: self.explored_state_count,
            evaluated_transition_count: self.evaluated_transition_count,
            certified_transition_count: self.certified_transition_count,
            certificate_binding_sha256: self.certificate_binding_sha256,
            positive_thickness_transition_count: self.positive_thickness_transition_count,
            positive_thickness_certified: self.positive_thickness_certified,
            positive_thickness_binding_sha256: self.positive_thickness_binding_sha256,
            layer_transport_transition_count: self.layer_transport_transition_count,
            layer_transport_certified: self.layer_transport_certified,
            layer_transport_binding_sha256: self.layer_transport_binding_sha256,
            mutation_candidate_ready: self.mutation_candidate_ready,
            authorizes_project_mutation: self.authorizes_project_mutation,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dyadic_pose_graph_request_schema_rejects_unknown_fields() {
        let project_id = ProjectId::new();
        let value = serde_json::json!({
            "expectedProjectInstanceId": project_id,
            "expectedProjectId": project_id,
            "expectedRevision": 0,
            "targetAngles": [],
            "maxStates": 1,
            "maxTransitions": 1,
            "levelCount": 3,
            "unexpected": true,
        });
        assert!(serde_json::from_value::<DyadicPoseGraphReadRequestV1>(value).is_err());
    }

    #[test]
    fn unsupported_response_shape_is_exact_and_never_authorizes_mutation() {
        let project = super::super::super::initial_project_state();
        let instance_id = project.instance_id;
        let project_id = project.project_id;
        let revision = project.editor.revision();
        let response = unsupported_dyadic_graph_response_v1(&project);

        assert_eq!(
            serde_json::to_value(response).unwrap(),
            serde_json::json!({
                "version": 1,
                "projectInstanceId": instance_id,
                "projectId": project_id,
                "revision": revision,
                "status": "unsupported",
                "reason": "unsupported_geometry",
                "stateCount": 0,
                "transitionCount": 0,
                "exploredStateCount": 0,
                "evaluatedTransitionCount": 0,
                "certifiedTransitionCount": 0,
                "certificateBindingSha256": null,
                "positiveThicknessTransitionCount": 0,
                "positiveThicknessCertified": false,
                "positiveThicknessBindingSha256": null,
                "layerTransportTransitionCount": 0,
                "layerTransportCertified": false,
                "layerTransportBindingSha256": null,
                "mutationCandidateReady": false,
                "authorizesProjectMutation": false,
            })
        );
    }
}
