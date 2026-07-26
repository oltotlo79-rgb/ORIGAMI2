//! Private one-shot preview boundary for certified dyadic pose paths.
//!
//! Serializable requests can only mint or address a native record. Mutation
//! remains authorized exclusively by the retained proof objects after both
//! live authority slots and the project OCC binding are revalidated.

use std::sync::Mutex;

use ori_core::{AppliedPoseLimitsV1, prepare_closed_graph_applied_pose_v1};
use ori_domain::{InstructionHingeAngle, ProjectId};
use serde::{Deserialize, Serialize};
use tauri::State;

use super::super::{
    AppState,
    applied_pose::{
        lock_revalidated_current_applied_pose_for_commit, restore_persisted_current_pose,
    },
    global_flat_foldability::{
        GlobalFlatFoldabilityState, lock_revalidated_current_layer_order_for_commit,
    },
    lock_project,
};
use super::{
    CYCLE_PATH_RESOURCE_MESSAGE, CYCLE_PATH_UNCERTIFIED_MESSAGE, CycleScheduleRequestV1,
    DyadicPathNativeAuthorityV1, DyadicPoseGraphAngleDtoV1, DyadicPoseGraphReadRequestV1,
    INVALID_REQUEST_MESSAGE, MAX_DYADIC_GRAPH_STATES_V1, MAX_DYADIC_GRAPH_TRANSITIONS_V1,
    STALE_MESSAGE, UNAVAILABLE_MESSAGE, default_dyadic_level_count_v1,
    dyadic_request_hinge_counts_are_bounded_v1, pose_state_fingerprint_v1,
    read_bounded_dyadic_pose_graph_inner_v1,
};

#[derive(Default)]
pub(crate) struct DyadicPathPreviewState(Mutex<Option<DyadicPathPreviewRecordV1>>);

struct DyadicPathPreviewRecordV1 {
    token: ProjectId,
    project_instance_id: ProjectId,
    project_id: ProjectId,
    revision: u64,
    target_binding: [u8; 32],
    path_binding: String,
    positive_binding: String,
    layer_binding: String,
    authority: Option<DyadicPathNativeAuthorityV1>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct DyadicPathPreviewRequestV1 {
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
    pub(super) expected_path_binding_sha256: String,
    pub(super) expected_positive_thickness_binding_sha256: String,
    pub(super) expected_layer_transport_binding_sha256: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DyadicPathPreviewResponseV1 {
    pub(super) version: u32,
    pub(super) preview_token: ProjectId,
    pub(super) project_instance_id: ProjectId,
    pub(super) project_id: ProjectId,
    pub(super) revision: u64,
    pub(super) target_binding_sha256: String,
    pub(super) path_binding_sha256: String,
    pub(super) positive_thickness_binding_sha256: String,
    pub(super) layer_transport_binding_sha256: String,
    pub(super) authorizes_project_mutation: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ApplyDyadicPathPreviewRequestV1 {
    pub(super) preview_token: ProjectId,
    pub(super) expected_project_instance_id: ProjectId,
    pub(super) expected_project_id: ProjectId,
    pub(super) expected_revision: u64,
    pub(super) expected_target_binding_sha256: String,
    pub(super) expected_path_binding_sha256: String,
    pub(super) expected_positive_thickness_binding_sha256: String,
    pub(super) expected_layer_transport_binding_sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CancelDyadicPathPreviewRequestV1 {
    preview_token: ProjectId,
}

#[tauri::command]
pub(crate) fn mint_dyadic_pose_path_preview_v1(
    app_state: State<'_, AppState>,
    foldability_state: State<'_, GlobalFlatFoldabilityState>,
    preview_state: State<'_, DyadicPathPreviewState>,
    request: DyadicPathPreviewRequestV1,
) -> Result<DyadicPathPreviewResponseV1, String> {
    mint_dyadic_pose_path_preview_inner_v1(&app_state, &foldability_state, &preview_state, request)
}

pub(super) fn mint_dyadic_pose_path_preview_inner_v1(
    app_state: &AppState,
    foldability_state: &GlobalFlatFoldabilityState,
    preview_state: &DyadicPathPreviewState,
    request: DyadicPathPreviewRequestV1,
) -> Result<DyadicPathPreviewResponseV1, String> {
    let valid_hash = |value: &str| {
        value.len() == 64
            && value
                .as_bytes()
                .iter()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    };
    if !valid_hash(&request.expected_path_binding_sha256)
        || !valid_hash(&request.expected_positive_thickness_binding_sha256)
        || !valid_hash(&request.expected_layer_transport_binding_sha256)
    {
        return Err(INVALID_REQUEST_MESSAGE.to_owned());
    }
    if !dyadic_request_hinge_counts_are_bounded_v1(
        request.target_angles.len(),
        request
            .cycle_schedule_v1
            .as_ref()
            .map(|schedule| schedule.entries.len()),
    ) {
        return Err(CYCLE_PATH_RESOURCE_MESSAGE.to_owned());
    }
    if request.max_states > MAX_DYADIC_GRAPH_STATES_V1
        || request.max_transitions > MAX_DYADIC_GRAPH_TRANSITIONS_V1
    {
        return Err(CYCLE_PATH_RESOURCE_MESSAGE.to_owned());
    }
    let mut target_entries = request
        .target_angles
        .iter()
        .map(|entry| ori_kinematics::HingeAngle::new(entry.edge, entry.angle_degrees))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| INVALID_REQUEST_MESSAGE.to_owned())?;
    target_entries.sort_unstable_by_key(|entry| entry.edge().canonical_bytes());
    let target = ori_kinematics::CanonicalHingeAngles::new(target_entries)
        .map_err(|_| INVALID_REQUEST_MESSAGE.to_owned())?;
    let target_binding = pose_state_fingerprint_v1(&target);
    let mut native_authority = None;
    let observed = read_bounded_dyadic_pose_graph_inner_v1(
        app_state,
        Some(foldability_state),
        DyadicPoseGraphReadRequestV1 {
            expected_project_instance_id: request.expected_project_instance_id,
            expected_project_id: request.expected_project_id,
            expected_revision: request.expected_revision,
            target_angles: request.target_angles,
            max_states: request.max_states,
            max_transitions: request.max_transitions,
            level_count: request.level_count,
            cycle_schedule_v1: request.cycle_schedule_v1,
        },
        Some(&mut native_authority),
    )?;
    if !observed.mutation_candidate_ready()
        || observed.certificate_binding_sha256()
            != Some(request.expected_path_binding_sha256.as_str())
        || observed.positive_thickness_binding_sha256()
            != Some(request.expected_positive_thickness_binding_sha256.as_str())
        || observed.layer_transport_binding_sha256()
            != Some(request.expected_layer_transport_binding_sha256.as_str())
    {
        return Err(CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned());
    }
    let project = lock_project(app_state).map_err(|_| UNAVAILABLE_MESSAGE.to_owned())?;
    if project.instance_id != request.expected_project_instance_id
        || project.project_id != request.expected_project_id
        || project.editor.revision() != request.expected_revision
    {
        return Err(STALE_MESSAGE.to_owned());
    }
    let token = ProjectId::new();
    let record = DyadicPathPreviewRecordV1 {
        token,
        project_instance_id: project.instance_id,
        project_id: project.project_id,
        revision: project.editor.revision(),
        target_binding,
        path_binding: request.expected_path_binding_sha256.clone(),
        positive_binding: request.expected_positive_thickness_binding_sha256.clone(),
        layer_binding: request.expected_layer_transport_binding_sha256.clone(),
        authority: Some(native_authority.ok_or_else(|| CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned())?),
    };
    *preview_state
        .0
        .lock()
        .map_err(|_| UNAVAILABLE_MESSAGE.to_owned())? = Some(record);
    Ok(DyadicPathPreviewResponseV1 {
        version: 1,
        preview_token: token,
        project_instance_id: project.instance_id,
        project_id: project.project_id,
        revision: project.editor.revision(),
        target_binding_sha256: target_binding
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
        path_binding_sha256: request.expected_path_binding_sha256,
        positive_thickness_binding_sha256: request.expected_positive_thickness_binding_sha256,
        layer_transport_binding_sha256: request.expected_layer_transport_binding_sha256,
        authorizes_project_mutation: false,
    })
}

/// Consumes a fully bound dyadic preview only after its private proof objects
/// and both live authority slots have been revalidated atomically.
#[tauri::command]
pub(crate) fn apply_dyadic_pose_path_preview_v1(
    app_state: State<'_, AppState>,
    foldability_state: State<'_, GlobalFlatFoldabilityState>,
    preview_state: State<'_, DyadicPathPreviewState>,
    request: ApplyDyadicPathPreviewRequestV1,
) -> Result<u64, String> {
    apply_dyadic_pose_path_preview_inner_v1(&app_state, &foldability_state, &preview_state, request)
}

pub(super) fn apply_dyadic_pose_path_preview_inner_v1(
    app_state: &AppState,
    foldability_state: &GlobalFlatFoldabilityState,
    preview_state: &DyadicPathPreviewState,
    request: ApplyDyadicPathPreviewRequestV1,
) -> Result<u64, String> {
    let mut project = lock_project(app_state).map_err(|_| UNAVAILABLE_MESSAGE.to_owned())?;
    if project.instance_id != request.expected_project_instance_id
        || project.project_id != request.expected_project_id
        || project.editor.revision() != request.expected_revision
    {
        return Err(STALE_MESSAGE.to_owned());
    }
    let target_binding = request
        .expected_target_binding_sha256
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            std::str::from_utf8(pair)
                .ok()
                .and_then(|value| u8::from_str_radix(value, 16).ok())
        })
        .collect::<Option<Vec<_>>>()
        .filter(|value| value.len() == 32)
        .ok_or_else(|| INVALID_REQUEST_MESSAGE.to_owned())?;
    let mut slot = preview_state
        .0
        .lock()
        .map_err(|_| UNAVAILABLE_MESSAGE.to_owned())?;
    let record = slot
        .as_ref()
        .ok_or_else(|| CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned())?;
    if record.token != request.preview_token
        || record.project_instance_id != request.expected_project_instance_id
        || record.project_id != request.expected_project_id
        || record.revision != request.expected_revision
        || record.target_binding.as_slice() != target_binding
        || record.path_binding != request.expected_path_binding_sha256
        || record.positive_binding != request.expected_positive_thickness_binding_sha256
        || record.layer_binding != request.expected_layer_transport_binding_sha256
        || !record.authority.as_ref().is_some_and(|authority| {
            authority.revalidates_private_proofs_v1(record.target_binding, &record.path_binding)
        })
    {
        return Err(CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned());
    }
    let authority = record
        .authority
        .as_ref()
        .ok_or_else(|| CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned())?;
    let (geometry, _audit, pose) = authority
        .pose_capability
        .graph()
        .ok_or_else(|| CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned())?;
    let pose_guard =
        lock_revalidated_current_applied_pose_for_commit(&project, &authority.pose_capability)
            .map_err(|_| UNAVAILABLE_MESSAGE.to_owned())?
            .ok_or_else(|| STALE_MESSAGE.to_owned())?;
    let layer_guard = lock_revalidated_current_layer_order_for_commit(
        // The capability retains the exact source snapshot used by every proof.
        // Holding this guard through the document commit closes replacement races.
        foldability_state,
        &project,
        &authority.layer_capability,
    );
    let layer_guard = layer_guard
        .map_err(|_| UNAVAILABLE_MESSAGE.to_owned())?
        .ok_or_else(|| STALE_MESSAGE.to_owned())?;
    let face_ids = geometry.face_ids().to_vec();
    let hinge_ids = geometry
        .hinges()
        .iter()
        .map(|hinge| hinge.edge())
        .collect::<Vec<_>>();
    let hinge_angles = authority
        .target_angles
        .as_slice()
        .iter()
        .map(|angle| (angle.edge(), angle.angle_degrees()))
        .collect::<Vec<_>>();
    let applied_pose = prepare_closed_graph_applied_pose_v1(
        &face_ids,
        &hinge_ids,
        pose.fixed_face(),
        &hinge_angles,
        AppliedPoseLimitsV1::default(),
    )
    .map_err(|_| CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned())?;
    let source_model_fingerprint = project.editor.fold_model_fingerprint_v1();
    let source_angles = pose
        .hinge_angles()
        .as_slice()
        .iter()
        .map(|angle| InstructionHingeAngle {
            edge: angle.edge(),
            angle_degrees: angle.angle_degrees(),
        })
        .collect::<Vec<_>>();
    let transition_targets = authority
        .edges
        .iter()
        .map(|edge| {
            edge.schedule
                .evaluate(1.0)
                .ok_or_else(|| CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned())
                .map(|angles| {
                    angles
                        .as_slice()
                        .iter()
                        .map(|angle| InstructionHingeAngle {
                            edge: angle.edge(),
                            angle_degrees: angle.angle_degrees(),
                        })
                        .collect::<Vec<_>>()
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let timeline = ori_instructions::append_certified_dyadic_path_timeline_v1(
        project.editor.instruction_timeline(),
        "Certified dyadic pose path",
        &source_model_fingerprint,
        pose.fixed_face(),
        &source_angles,
        &transition_targets,
        &authority.path,
    )
    .map_err(|_| CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned())?;
    let persisted_pose = timeline
        .steps
        .last()
        .map(|step| step.pose.clone())
        .ok_or_else(|| CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned())?;
    let editor_before = project.editor.clone();
    let pattern = project.editor.pattern().clone();
    let paper = project.editor.paper().clone();
    let layers = project.editor.project_layers().clone();
    let result = project
        .editor
        .execute_stacked_fold_document(
            record.revision,
            pattern,
            paper,
            timeline,
            layers,
            applied_pose,
        )
        .map_err(|_| CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned())?;
    drop(pose_guard);
    if restore_persisted_current_pose(&mut project, &persisted_pose).is_err() {
        project.editor = editor_before;
        return Err(UNAVAILABLE_MESSAGE.to_owned());
    }
    layer_guard.invalidate_after_project_mutation();
    slot.take();
    Ok(result.revision)
}

#[tauri::command]
pub(crate) fn cancel_dyadic_pose_path_preview_v1(
    preview_state: State<'_, DyadicPathPreviewState>,
    request: CancelDyadicPathPreviewRequestV1,
) -> Result<(), String> {
    cancel_dyadic_pose_path_preview_inner_v1(&preview_state, request.preview_token)
}

pub(super) fn cancel_dyadic_pose_path_preview_inner_v1(
    preview_state: &DyadicPathPreviewState,
    token: ProjectId,
) -> Result<(), String> {
    let mut slot = preview_state
        .0
        .lock()
        .map_err(|_| UNAVAILABLE_MESSAGE.to_owned())?;
    if slot.as_ref().is_none_or(|record| record.token != token) {
        return Err(CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned());
    }
    slot.take();
    Ok(())
}

#[cfg(test)]
impl DyadicPathPreviewState {
    pub(super) fn is_empty_for_test(&self) -> bool {
        self.0.lock().unwrap().is_none()
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn install_record_for_test(
        &self,
        token: ProjectId,
        project_instance_id: ProjectId,
        project_id: ProjectId,
        revision: u64,
        target_binding: [u8; 32],
        path_binding: String,
        positive_binding: String,
        layer_binding: String,
        authority: Option<DyadicPathNativeAuthorityV1>,
    ) {
        *self.0.lock().unwrap() = Some(DyadicPathPreviewRecordV1 {
            token,
            project_instance_id,
            project_id,
            revision,
            target_binding,
            path_binding,
            positive_binding,
            layer_binding,
            authority,
        });
    }
}

#[cfg(test)]
#[path = "stacked_fold_dyadic_preview_tests.rs"]
mod tests;
