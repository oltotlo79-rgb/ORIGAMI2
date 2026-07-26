//! Read-only native boundary for the live stacked-fold hinge registry.
//!
//! This module owns the strict request/response DTOs and the worker-isolated
//! command that derives a bounded target-graph hinge registry. It revalidates
//! both native pose and layer-order authority before publishing observation-
//! only data and never stages or authorizes a project mutation.

use ori_collision::{
    FlatEndpointLayerOrderInputV1, StackedFoldLinearCandidateV1, StackedFoldMaterialMapLimitsV1,
    StackedFoldReadBindingV1, StackedFoldReadLimitsV1, capture_stacked_fold_read_guard_v1,
    propose_linear_stacked_fold_read_v1, reverse_map_linear_stacked_fold_material_v1,
};
use ori_core::{
    ExpectedStackedFoldCreaseV1, FaceLineageLimits, StackedFoldGeometryLimitsV1,
    StackedFoldTopologyBuildLimitsV1, prepare_stacked_fold_geometry_candidate_v1,
    prepare_stacked_fold_initial_graph_pose_v1, prepare_stacked_fold_target_graph_audit_v1,
};
use ori_domain::{EdgeId, ProjectId};
use ori_kinematics::{HingeAngle, Point3, TreeKinematicsLimits};
use serde::{Deserialize, Serialize};
use tauri::State;

use super::{
    AppState,
    global_flat_foldability::{
        GlobalFlatFoldabilityState, capture_current_layer_order_capability,
        revalidate_current_layer_order_capability,
    },
    lock_project,
    stacked_fold_read::{
        ANALYSIS_FAILED_MESSAGE, BUSY_MESSAGE, FixedSideRequest, INVALID_REQUEST_MESSAGE,
        RotationDirectionRequest, STALE_MESSAGE, UNAVAILABLE_MESSAGE,
    },
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct LiveHingeRegistryRequestV1 {
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    first: [f64; 3],
    second: [f64; 3],
    fixed_side: FixedSideRequest,
    rotation_direction: RotationDirectionRequest,
    requested_angle_degrees: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct LiveHingeRegistryResponseV1 {
    version: u32,
    project_instance_id: ProjectId,
    project_id: ProjectId,
    revision: u64,
    pose_generation: u64,
    graph_fingerprint_sha256: String,
    entries: Vec<LiveGraphHingeAngleDto>,
    authorizes_project_mutation: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct LiveGraphHingeAngleDto {
    edge: EdgeId,
    initial_angle_degrees: f64,
}

pub(super) fn live_hinge_registry(angles: &[HingeAngle]) -> Vec<LiveGraphHingeAngleDto> {
    angles
        .iter()
        .map(|angle| LiveGraphHingeAngleDto {
            edge: angle.edge(),
            initial_angle_degrees: angle.angle_degrees(),
        })
        .collect()
}

#[cfg(test)]
impl LiveHingeRegistryRequestV1 {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn for_test(
        expected_project_instance_id: ProjectId,
        expected_project_id: ProjectId,
        expected_revision: u64,
        first: [f64; 3],
        second: [f64; 3],
        fixed_side: FixedSideRequest,
        rotation_direction: RotationDirectionRequest,
        requested_angle_degrees: f64,
    ) -> Self {
        Self {
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
            first,
            second,
            fixed_side,
            rotation_direction,
            requested_angle_degrees,
        }
    }
}

#[cfg(test)]
impl LiveHingeRegistryResponseV1 {
    pub(super) fn entries_for_test(&self) -> &[LiveGraphHingeAngleDto] {
        &self.entries
    }
}

#[cfg(test)]
impl LiveGraphHingeAngleDto {
    pub(super) fn edge_for_test(&self) -> EdgeId {
        self.edge
    }

    pub(super) fn initial_angle_degrees_for_test(&self) -> f64 {
        self.initial_angle_degrees
    }
}

#[tauri::command]
pub(super) async fn read_live_hinge_registry_v1(
    app_state: State<'_, AppState>,
    foldability_state: State<'_, GlobalFlatFoldabilityState>,
    request: LiveHingeRegistryRequestV1,
) -> Result<LiveHingeRegistryResponseV1, String> {
    read_live_hinge_registry_inner(&app_state, &foldability_state, request).await
}

pub(super) async fn read_live_hinge_registry_inner(
    app_state: &AppState,
    foldability_state: &GlobalFlatFoldabilityState,
    request: LiveHingeRegistryRequestV1,
) -> Result<LiveHingeRegistryResponseV1, String> {
    let worker_permit = app_state
        .try_acquire_native_pose_worker()
        .ok_or_else(|| BUSY_MESSAGE.to_owned())?;
    let (paper, pattern, capability, layer_capability, source_fingerprint) = {
        let project = lock_project(&app_state).map_err(|_| UNAVAILABLE_MESSAGE.to_owned())?;
        if project.instance_id != request.expected_project_instance_id
            || project.project_id != request.expected_project_id
            || project.editor.revision() != request.expected_revision
        {
            return Err(STALE_MESSAGE.to_owned());
        }
        let capability = project
            .applied_pose_authority
            .capture_capability(&project)
            .map_err(|_| UNAVAILABLE_MESSAGE.to_owned())?
            .ok_or_else(|| UNAVAILABLE_MESSAGE.to_owned())?;
        (
            project.editor.paper().clone(),
            project.editor.pattern().clone(),
            capability,
            capture_current_layer_order_capability(&foldability_state, &project)
                .map_err(|_| UNAVAILABLE_MESSAGE.to_owned())?
                .ok_or_else(|| UNAVAILABLE_MESSAGE.to_owned())?,
            project.editor.fold_model_fingerprint_v1(),
        )
    };
    let expected_instance_id = request.expected_project_instance_id;
    let expected_project_id = request.expected_project_id;
    let expected_revision = request.expected_revision;
    let (capability, layer_capability, source_fingerprint, fingerprint, entries) =
        tauri::async_runtime::spawn_blocking(move || {
            let (model, pose) = capability
                .tree()
                .ok_or_else(|| ANALYSIS_FAILED_MESSAGE.to_owned())?;
            let first = Point3::new(request.first[0], request.first[1], request.first[2])
                .map_err(|_| INVALID_REQUEST_MESSAGE.to_owned())?;
            let second = Point3::new(request.second[0], request.second[1], request.second[2])
                .map_err(|_| INVALID_REQUEST_MESSAGE.to_owned())?;
            let candidate = StackedFoldLinearCandidateV1::new(
                first,
                second,
                request.fixed_side.into(),
                request.rotation_direction.into(),
                request.requested_angle_degrees,
            )
            .map_err(|_| INVALID_REQUEST_MESSAGE.to_owned())?;
            let binding = StackedFoldReadBindingV1::new(
                expected_instance_id,
                expected_project_id,
                expected_revision,
                capability.generation(),
                layer_capability.generation(),
            );
            let input = FlatEndpointLayerOrderInputV1 {
                identity_namespace: binding.project_id(),
                source_revision: binding.source_revision(),
                paper: &paper,
                pattern: &pattern,
                model,
                pose,
                layer_order: layer_capability.snapshot(),
            };
            let limits = StackedFoldReadLimitsV1::default();
            let guard = capture_stacked_fold_read_guard_v1(binding, input, limits)
                .map_err(|_| ANALYSIS_FAILED_MESSAGE.to_owned())?;
            let proposal =
                propose_linear_stacked_fold_read_v1(&guard, binding, input, candidate, limits)
                    .map_err(|_| ANALYSIS_FAILED_MESSAGE.to_owned())?;
            let material_map = reverse_map_linear_stacked_fold_material_v1(
                &proposal,
                &guard,
                binding,
                input,
                limits,
                StackedFoldMaterialMapLimitsV1::default(),
            )
            .map_err(|_| ANALYSIS_FAILED_MESSAGE.to_owned())?;
            let expected_creases = material_map
                .segments()
                .iter()
                .map(|segment| ExpectedStackedFoldCreaseV1 {
                    start: segment.start(),
                    end: segment.end(),
                    kind: segment.assignment(),
                })
                .collect::<Vec<_>>();
            let prepared = prepare_stacked_fold_geometry_candidate_v1(
                binding.project_id(),
                binding.source_revision(),
                &pattern,
                &paper,
                layer_capability.snapshot(),
                &expected_creases,
                StackedFoldTopologyBuildLimitsV1::default(),
                FaceLineageLimits::default(),
                StackedFoldGeometryLimitsV1::default(),
            )
            .map_err(|_| ANALYSIS_FAILED_MESSAGE.to_owned())?;
            let fingerprint = prepared.proof().lineage().target_fingerprint().to_hex();
            let audited = prepare_stacked_fold_target_graph_audit_v1(
                prepared,
                TreeKinematicsLimits::default(),
            )
            .map_err(|_| ANALYSIS_FAILED_MESSAGE.to_owned())?;
            let initial = prepare_stacked_fold_initial_graph_pose_v1(audited, model, pose)
                .map_err(|_| ANALYSIS_FAILED_MESSAGE.to_owned())?;
            let entries = live_hinge_registry(initial.pose().hinge_angles().as_slice());
            if entries.len() > 64 {
                return Err(ANALYSIS_FAILED_MESSAGE.to_owned());
            }
            drop(worker_permit);
            Ok::<_, String>((
                capability,
                layer_capability,
                source_fingerprint,
                fingerprint,
                entries,
            ))
        })
        .await
        .map_err(|_| ANALYSIS_FAILED_MESSAGE.to_owned())??;
    {
        let project = lock_project(&app_state).map_err(|_| STALE_MESSAGE.to_owned())?;
        if project.editor.fold_model_fingerprint_v1() != source_fingerprint
            || project
                .applied_pose_authority
                .revalidate_capability(&project, &capability)
                .map_err(|_| STALE_MESSAGE.to_owned())?
                .is_none()
            || revalidate_current_layer_order_capability(
                &foldability_state,
                &project,
                &layer_capability,
            )
            .map_err(|_| STALE_MESSAGE.to_owned())?
            .is_none()
        {
            return Err(STALE_MESSAGE.to_owned());
        }
    }
    Ok(LiveHingeRegistryResponseV1 {
        version: 1,
        project_instance_id: expected_instance_id,
        project_id: expected_project_id,
        revision: expected_revision,
        pose_generation: capability.generation(),
        graph_fingerprint_sha256: fingerprint,
        entries,
        authorizes_project_mutation: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request_for(state: &AppState, expected_revision: u64) -> LiveHingeRegistryRequestV1 {
        let project = lock_project(state).unwrap();
        LiveHingeRegistryRequestV1 {
            expected_project_instance_id: project.instance_id,
            expected_project_id: project.project_id,
            expected_revision,
            first: [0.0, 0.0, 0.0],
            second: [1.0, 0.0, 0.0],
            fixed_side: FixedSideRequest::Left,
            rotation_direction: RotationDirectionRequest::Positive,
            requested_angle_degrees: 1.0,
        }
    }

    #[test]
    fn live_hinge_registry_request_schema_rejects_unknown_fields() {
        let project_id = ProjectId::new();
        let value = serde_json::json!({
            "expectedProjectInstanceId": project_id,
            "expectedProjectId": project_id,
            "expectedRevision": 0,
            "first": [0.0, 0.0, 0.0],
            "second": [1.0, 0.0, 0.0],
            "fixedSide": "left",
            "rotationDirection": "positive",
            "requestedAngleDegrees": 1.0,
            "unexpected": true,
        });
        assert!(serde_json::from_value::<LiveHingeRegistryRequestV1>(value).is_err());
    }

    #[test]
    fn live_hinge_registry_busy_rejection_precedes_state_access_and_is_an_atomic_no_op() {
        let state = AppState::new(super::super::initial_project_state());
        let revision = lock_project(&state).unwrap().editor.revision();
        let request = request_for(&state, revision);
        let permit = state.try_acquire_native_pose_worker().unwrap();
        let error = tauri::async_runtime::block_on(read_live_hinge_registry_inner(
            &state,
            &GlobalFlatFoldabilityState::default(),
            request,
        ))
        .unwrap_err();
        assert_eq!(error, BUSY_MESSAGE);
        assert!(state.native_pose_worker_is_busy());
        let project = lock_project(&state).unwrap();
        assert_eq!(project.editor.revision(), revision);
        assert!(project.editor.instruction_timeline().steps.is_empty());
        drop(project);
        drop(permit);
        assert!(!state.native_pose_worker_is_busy());
    }

    #[test]
    fn stale_live_hinge_registry_binding_is_an_atomic_no_op_and_releases_the_worker() {
        let state = AppState::new(super::super::initial_project_state());
        let revision = lock_project(&state).unwrap().editor.revision();
        let request = request_for(&state, revision + 1);
        let error = tauri::async_runtime::block_on(read_live_hinge_registry_inner(
            &state,
            &GlobalFlatFoldabilityState::default(),
            request,
        ))
        .unwrap_err();
        assert_eq!(error, STALE_MESSAGE);
        assert!(!state.native_pose_worker_is_busy());
        let project = lock_project(&state).unwrap();
        assert_eq!(project.editor.revision(), revision);
        assert!(project.editor.instruction_timeline().steps.is_empty());
    }
}
