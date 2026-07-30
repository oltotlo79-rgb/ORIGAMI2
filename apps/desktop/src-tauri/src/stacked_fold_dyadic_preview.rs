//! Private one-shot preview boundary for certified dyadic pose paths.
//!
//! Serializable requests can only mint or address a native record. Mutation
//! remains authorized exclusively by the retained proof objects after both
//! live authority slots and the project OCC binding are revalidated.

use std::sync::Mutex;

#[cfg(test)]
use std::{cell::Cell, marker::PhantomData, rc::Rc};

use ori_core::{AppliedPoseLimitsV1, prepare_closed_graph_applied_pose_v1};
use ori_domain::{InstructionHingeAngle, ProjectId};
use serde::{Deserialize, Serialize};
use tauri::State;

use super::super::stacked_fold_transaction::StackedFoldProjectRollbackSnapshotV1;
#[cfg(test)]
use super::super::stacked_fold_transaction::rollback_stacked_fold_apply_v1;
use super::super::{
    AppState,
    applied_pose::{
        lock_revalidated_current_applied_pose_for_commit,
        restore_persisted_current_pose_transactional_v1,
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
    read_bounded_dyadic_pose_graph_inner_v1, validate_progress_request_id_v1,
    with_current_cycle_publication_v1,
};

#[derive(Default)]
pub(crate) struct DyadicPathPreviewState(Mutex<Option<DyadicPathPreviewRecordV1>>);

#[cfg(test)]
thread_local! {
    static NEXT_DYADIC_APPLY_AFTER_POSE_REISSUE_FAILURE_TOKEN_V1: Cell<u64> =
        const { Cell::new(0) };
    static FAIL_NEXT_DYADIC_APPLY_AFTER_POSE_REISSUE_V1: Cell<Option<u64>> =
        const { Cell::new(None) };
}

#[cfg(test)]
#[must_use = "the dyadic Apply fault remains armed only while this guard is held"]
pub(super) struct ArmedDyadicApplyAfterPoseReissueFailureGuardV1 {
    token: u64,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

#[cfg(test)]
impl Drop for ArmedDyadicApplyAfterPoseReissueFailureGuardV1 {
    fn drop(&mut self) {
        FAIL_NEXT_DYADIC_APPLY_AFTER_POSE_REISSUE_V1.with(|slot| {
            if slot.get() == Some(self.token) {
                slot.set(None);
            }
        });
    }
}

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
    #[serde(default)]
    pub(super) progress_request_id: Option<String>,
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
    validate_progress_request_id_v1(request.progress_request_id.as_deref())?;
    if !dyadic_request_hinge_counts_are_bounded_v1(
        request.target_angles.len(),
        request
            .cycle_schedule_v1
            .as_ref()
            .map(|schedule| schedule.entries.len()),
    ) {
        return Err(CYCLE_PATH_RESOURCE_MESSAGE.to_owned());
    }
    if !(1..=MAX_DYADIC_GRAPH_STATES_V1).contains(&request.max_states)
        || !(1..=MAX_DYADIC_GRAPH_TRANSITIONS_V1).contains(&request.max_transitions)
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
            progress_request_id: request.progress_request_id.clone(),
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
    let mut native_authority =
        native_authority.ok_or_else(|| CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned())?;
    let read_scope = native_authority
        .read_scope
        .take()
        .ok_or_else(|| CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned())?;
    let read_generation = read_scope.generation();
    let project = lock_project(app_state).map_err(|_| UNAVAILABLE_MESSAGE.to_owned())?;
    if project.instance_id != request.expected_project_instance_id
        || project.project_id != request.expected_project_id
        || project.editor.revision() != request.expected_revision
    {
        return Err(STALE_MESSAGE.to_owned());
    }
    let token = ProjectId::new();
    let target_binding_sha256 = target_binding
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let path_binding = request.expected_path_binding_sha256;
    let positive_binding = request.expected_positive_thickness_binding_sha256;
    let layer_binding = request.expected_layer_transport_binding_sha256;
    let record = DyadicPathPreviewRecordV1 {
        token,
        project_instance_id: project.instance_id,
        project_id: project.project_id,
        revision: project.editor.revision(),
        target_binding,
        path_binding: path_binding.clone(),
        positive_binding: positive_binding.clone(),
        layer_binding: layer_binding.clone(),
        authority: Some(native_authority),
    };
    let response = DyadicPathPreviewResponseV1 {
        version: 1,
        preview_token: token,
        project_instance_id: project.instance_id,
        project_id: project.project_id,
        revision: project.editor.revision(),
        target_binding_sha256,
        path_binding_sha256: path_binding,
        positive_thickness_binding_sha256: positive_binding,
        layer_transport_binding_sha256: layer_binding,
        authorizes_project_mutation: false,
    };
    with_current_cycle_publication_v1(read_generation, || {
        let mut slot = preview_state
            .0
            .try_lock()
            .map_err(|_| UNAVAILABLE_MESSAGE.to_owned())?;
        *slot = Some(record);
        Ok(())
    })?;
    Ok(response)
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
    let mut layer_guard = layer_guard
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
    // Keep the exact project/layer/pose-cache images armed until the target
    // pose has been reissued and the source layer authority is invalidated.
    // This matches the certified Tree Apply rollback boundary.
    let mut project_before = StackedFoldProjectRollbackSnapshotV1::capture(&project);
    let layer_before = layer_guard.capture_rollback_snapshot_v1();
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
    let pose_rollback =
        match restore_persisted_current_pose_transactional_v1(&mut project, &persisted_pose) {
            Ok(rollback) => rollback,
            Err(_) => {
                // The transactional pose helper restored its own armed image
                // before returning; complete the remaining project -> layer
                // restoration without exposing the committed editor.
                project_before
                    .restore(&mut project)
                    .map_err(|_| UNAVAILABLE_MESSAGE.to_owned())?;
                layer_guard.restore_rollback_snapshot_v1(&layer_before);
                return Err(UNAVAILABLE_MESSAGE.to_owned());
            }
        };
    #[cfg(test)]
    let mut pose_rollback = pose_rollback;
    #[cfg(test)]
    if take_dyadic_apply_after_pose_reissue_failure_for_test_v1() {
        rollback_stacked_fold_apply_v1(
            &mut project,
            &mut project_before,
            &mut pose_rollback,
            Some(&mut layer_guard),
            Some(&layer_before),
        )
        .map_err(|_| UNAVAILABLE_MESSAGE.to_owned())?;
        return Err(UNAVAILABLE_MESSAGE.to_owned());
    }
    layer_guard.invalidate_after_project_mutation();
    pose_rollback.disarm();
    drop(layer_guard);
    *slot = None;
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
    *slot = None;
    Ok(())
}

#[cfg(test)]
pub(super) fn fail_next_dyadic_apply_after_pose_reissue_for_test_v1()
-> ArmedDyadicApplyAfterPoseReissueFailureGuardV1 {
    FAIL_NEXT_DYADIC_APPLY_AFTER_POSE_REISSUE_V1.with(|slot| {
        assert!(
            slot.get().is_none(),
            "a dyadic Apply failure is already armed"
        );
    });
    let token = NEXT_DYADIC_APPLY_AFTER_POSE_REISSUE_FAILURE_TOKEN_V1.with(|next| {
        let token = next
            .get()
            .checked_add(1)
            .expect("dyadic Apply failure token overflow");
        next.set(token);
        token
    });
    FAIL_NEXT_DYADIC_APPLY_AFTER_POSE_REISSUE_V1.with(|slot| slot.set(Some(token)));
    ArmedDyadicApplyAfterPoseReissueFailureGuardV1 {
        token,
        _not_send_or_sync: PhantomData,
    }
}

#[cfg(test)]
fn take_dyadic_apply_after_pose_reissue_failure_for_test_v1() -> bool {
    FAIL_NEXT_DYADIC_APPLY_AFTER_POSE_REISSUE_V1
        .with(Cell::take)
        .is_some()
}

#[cfg(test)]
mod fault_guard_tests {
    use super::{
        fail_next_dyadic_apply_after_pose_reissue_for_test_v1,
        take_dyadic_apply_after_pose_reissue_failure_for_test_v1,
    };

    #[test]
    fn dyadic_apply_failure_guard_preserves_old_arm_and_clears_every_exit_path() {
        let original_guard = fail_next_dyadic_apply_after_pose_reissue_for_test_v1();
        let duplicate = std::panic::catch_unwind(|| {
            let _duplicate_guard = fail_next_dyadic_apply_after_pose_reissue_for_test_v1();
        });
        assert!(duplicate.is_err());
        assert!(
            take_dyadic_apply_after_pose_reissue_failure_for_test_v1(),
            "a rejected duplicate arm cannot replace the original fault"
        );

        let replacement_guard = fail_next_dyadic_apply_after_pose_reissue_for_test_v1();
        drop(original_guard);
        assert!(
            take_dyadic_apply_after_pose_reissue_failure_for_test_v1(),
            "a consumed guard cannot clear an equal later arm with a new token"
        );
        drop(replacement_guard);

        let early_return: Result<(), ()> = {
            let _guard = fail_next_dyadic_apply_after_pose_reissue_for_test_v1();
            Err(())
        };
        assert_eq!(early_return, Err(()));
        assert!(!take_dyadic_apply_after_pose_reissue_failure_for_test_v1());

        let unwound = std::panic::catch_unwind(|| {
            let _guard = fail_next_dyadic_apply_after_pose_reissue_for_test_v1();
            panic!("inject dyadic Apply fault setup unwind");
        });
        assert!(unwound.is_err());
        assert!(!take_dyadic_apply_after_pose_reissue_failure_for_test_v1());

        let consumed_guard = fail_next_dyadic_apply_after_pose_reissue_for_test_v1();
        assert!(take_dyadic_apply_after_pose_reissue_failure_for_test_v1());
        drop(consumed_guard);
        assert!(
            !take_dyadic_apply_after_pose_reissue_failure_for_test_v1(),
            "dropping a consumed guard cannot clear or synthesize another fault"
        );
    }
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
