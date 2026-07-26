//! Read-only native boundary for bounded even-cycle candidate discovery.
//!
//! This module owns the strict wire DTOs and the one observation-only command
//! that enumerates same-assignment opposite hinge pairs and certified Kawasaki
//! endpoints. It never stages or authorizes a project mutation.

use ori_collision::diagnose_scheduled_cycle_path_v1;
use ori_kinematics::{CycleBasisLimitsV1, DyadicIntervalClosureLimitsV1};
use serde::{Deserialize, Serialize};
use tauri::State;

use super::{
    AppState, lock_project,
    stacked_fold_read::{STALE_MESSAGE, UNAVAILABLE_MESSAGE, production_cycle_schedule_limits_v1},
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct EvenCycleCandidatesRequestV1 {
    expected_project_instance_id: ori_domain::ProjectId,
    expected_project_id: ori_domain::ProjectId,
    expected_revision: u64,
    max_pair_tests: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct EvenCycleCandidatesResponseV1 {
    version: u32,
    project_instance_id: ori_domain::ProjectId,
    project_id: ori_domain::ProjectId,
    revision: u64,
    status: &'static str,
    reason: &'static str,
    candidates: Vec<EvenCycleCandidateDtoV1>,
    kawasaki_endpoints: Vec<KawasakiEndpointCandidateDtoV1>,
    authorizes_project_mutation: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct KawasakiEndpointCandidateDtoV1 {
    version: u32,
    endpoint_denominator: u64,
    closure_status: &'static str,
    collision_status: &'static str,
    authorizes_apply: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EvenCycleCandidateDtoV1 {
    version: u32,
    edges: [ori_domain::EdgeId; 2],
    reason: &'static str,
}

#[cfg(test)]
impl EvenCycleCandidatesRequestV1 {
    pub(super) fn for_test(
        expected_project_instance_id: ori_domain::ProjectId,
        expected_project_id: ori_domain::ProjectId,
        expected_revision: u64,
        max_pair_tests: usize,
    ) -> Self {
        Self {
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
            max_pair_tests,
        }
    }
}

#[cfg(test)]
impl EvenCycleCandidatesResponseV1 {
    pub(super) fn kawasaki_endpoint_outcomes_for_test(
        &self,
    ) -> Vec<(&'static str, &'static str, bool)> {
        self.kawasaki_endpoints
            .iter()
            .map(|candidate| {
                (
                    candidate.closure_status,
                    candidate.collision_status,
                    candidate.authorizes_apply,
                )
            })
            .collect()
    }
}

#[tauri::command]
pub(super) fn read_even_cycle_candidates_v1(
    app_state: State<'_, AppState>,
    request: EvenCycleCandidatesRequestV1,
) -> Result<EvenCycleCandidatesResponseV1, String> {
    read_even_cycle_candidates_inner_v1(&app_state, request)
}

pub(super) fn read_even_cycle_candidates_inner_v1(
    app_state: &AppState,
    request: EvenCycleCandidatesRequestV1,
) -> Result<EvenCycleCandidatesResponseV1, String> {
    let project = lock_project(app_state).map_err(|_| UNAVAILABLE_MESSAGE.to_owned())?;
    if project.instance_id != request.expected_project_instance_id
        || project.project_id != request.expected_project_id
        || project.editor.revision() != request.expected_revision
    {
        return Err(STALE_MESSAGE.to_owned());
    }
    let pose_capability = project
        .applied_pose_authority
        .capture_capability(&project)
        .ok()
        .flatten();
    let graph = pose_capability
        .as_ref()
        .and_then(|capability| capability.graph());
    let (status, reason, candidates) = match graph {
        None => (
            "unsupported",
            "current_pose_is_not_a_material_hinge_graph",
            Vec::new(),
        ),
        Some((geometry, audit, _)) => {
            match ori_kinematics::enumerate_even_single_vertex_opposite_pairs_v1(
                geometry,
                audit,
                request.max_pair_tests,
            ) {
                Ok(pairs) if pairs.is_empty() => {
                    ("none", "no_same_assignment_opposite_pair", Vec::new())
                }
                Ok(pairs) => ("ready", "same_assignment_geometrically_opposite", pairs),
                Err(ori_kinematics::KinematicsError::ResourceLimitExceeded) => {
                    ("resource_limit", "pair_test_limit_exceeded", Vec::new())
                }
                Err(_) => (
                    "unsupported",
                    "not_a_bounded_even_single_vertex_cycle",
                    Vec::new(),
                ),
            }
        }
    };
    let kawasaki_endpoints = graph.map_or_else(Vec::new, |(geometry, audit, pose)| {
        [1_u64, 2, 4, 8, 16]
            .into_iter()
            .filter_map(|endpoint_denominator| {
                let generated = ori_kinematics::generate_bounded_degree_four_kawasaki_path_candidate_at_dyadic_endpoint_v1(
                    geometry, audit, pose.fixed_face(), endpoint_denominator,
                    production_cycle_schedule_limits_v1(),
                ).ok()?;
                let closure = geometry.prove_simultaneous_cycle_basis_schedule_closure_v1(
                    audit, pose.fixed_face(), generated.schedule(),
                    ori_core::STACKED_FOLD_GRAPH_CLOSURE_TOLERANCE_V1,
                    CycleBasisLimitsV1::default(),
                    DyadicIntervalClosureLimitsV1 {
                        max_depth: 16, max_leaves: 65_536, max_work: 1_048_576,
                        schedule_limits: production_cycle_schedule_limits_v1(),
                    },
                ).ok()?;
                let continuous = diagnose_scheduled_cycle_path_v1(
                    geometry, audit, pose.fixed_face(), &generated, closure.closure(), 32,
                );
                let certified = continuous.continuous_certificate_model_id().is_some();
                Some(KawasakiEndpointCandidateDtoV1 {
                    version: 1,
                    endpoint_denominator,
                    closure_status: "certified",
                    collision_status: if certified { "certified" } else { "uncertified" },
                    authorizes_apply: false,
                })
            })
            .collect()
    });
    Ok(EvenCycleCandidatesResponseV1 {
        version: 1,
        project_instance_id: project.instance_id,
        project_id: project.project_id,
        revision: project.editor.revision(),
        status,
        reason,
        candidates: candidates
            .into_iter()
            .map(|edges| EvenCycleCandidateDtoV1 {
                version: 1,
                edges,
                reason: "same_assignment_geometrically_opposite",
            })
            .collect(),
        kawasaki_endpoints,
        authorizes_project_mutation: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn even_cycle_candidate_request_schema_rejects_unknown_fields() {
        let project_id = ori_domain::ProjectId::new();
        let value = serde_json::json!({
            "expectedProjectInstanceId": project_id,
            "expectedProjectId": project_id,
            "expectedRevision": 0,
            "maxPairTests": 1,
            "unexpected": true,
        });
        assert!(serde_json::from_value::<EvenCycleCandidatesRequestV1>(value).is_err());
    }

    #[test]
    fn stale_even_cycle_candidate_binding_is_an_atomic_no_op() {
        let state = AppState::new(super::super::initial_project_state());
        let (instance_id, project_id, revision) = {
            let project = lock_project(&state).unwrap();
            (
                project.instance_id,
                project.project_id,
                project.editor.revision(),
            )
        };
        let error = read_even_cycle_candidates_inner_v1(
            &state,
            EvenCycleCandidatesRequestV1 {
                expected_project_instance_id: instance_id,
                expected_project_id: project_id,
                expected_revision: revision + 1,
                max_pair_tests: 1,
            },
        )
        .unwrap_err();
        assert_eq!(error, STALE_MESSAGE);
        let project = lock_project(&state).unwrap();
        assert_eq!(project.editor.revision(), revision);
        assert!(project.editor.instruction_timeline().steps.is_empty());
    }

    #[test]
    fn even_cycle_candidate_pair_limit_fails_closed_without_mutation_authority() {
        let (mut project, hinges) =
            super::super::applied_pose::tests::flat_foldable_cross_cycle_project();
        super::super::applied_pose::tests::install_flat_graph_pose_authority(&mut project, hinges);
        let instance_id = project.instance_id;
        let project_id = project.project_id;
        let revision = project.editor.revision();
        let state = AppState::new(project);

        let response = read_even_cycle_candidates_inner_v1(
            &state,
            EvenCycleCandidatesRequestV1 {
                expected_project_instance_id: instance_id,
                expected_project_id: project_id,
                expected_revision: revision,
                max_pair_tests: 0,
            },
        )
        .unwrap();
        assert_eq!(response.status, "resource_limit");
        assert_eq!(response.reason, "pair_test_limit_exceeded");
        assert!(response.candidates.is_empty());
        assert!(!response.authorizes_project_mutation);
        assert!(
            response
                .kawasaki_endpoints
                .iter()
                .all(|candidate| !candidate.authorizes_apply)
        );
        let project = lock_project(&state).unwrap();
        assert_eq!(project.editor.revision(), revision);
        assert!(project.editor.instruction_timeline().steps.is_empty());
    }
}
