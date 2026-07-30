//! One-shot native authority for a same-geometry non-flat continuation.
//!
//! This boundary deliberately does not reuse a flat `LayerOrderSnapshot` as
//! the source of a non-flat schedule.  The current core-owned non-flat
//! evidence, the current Tree/graph pose, and the generated schedule at
//! `t = 0` must all contain the exact same hinge-angle bits.  A positive Tree
//! continuous certificate and the core non-flat cell-transport proof are then
//! retained together behind one process-local token.

use std::sync::Mutex;

#[cfg(test)]
use std::{cell::Cell, marker::PhantomData, rc::Rc};

use ori_collision::{
    CertifiedPathGraphSearchResultV1, CertifiedPathTransitionCandidateV1,
    CertifiedPathTransitionEvidenceV1, NonFlatCellTransportLimitsV1, NonFlatCellTransportProofV1,
    PositiveThicknessTreeContinuousCertificateV1, certify_non_flat_cell_transport_with_limits_v1,
    certify_positive_thickness_tree_continuous_path_v1, search_certified_pose_graph_v1,
};
use ori_core::{
    AppliedPoseLimitsV1, StackedFoldNonFlatLayerOrderV1, analyze_global_flat_foldability,
    analyze_local_flat_foldability, prepare_closed_graph_applied_pose_v1,
    revalidate_current_non_flat_layer_order_v1,
};
use ori_domain::{InstructionHingeAngle, ProjectId};
use ori_foldability::{
    ExactAffineTransform, ExactRationalValue, ExactSign, GlobalFlatFoldabilityInput,
    GlobalFlatFoldabilityLimits, GlobalFlatFoldabilityOutcome, LayerOrderSnapshot,
};
use ori_kinematics::{
    CanonicalCycleScheduleV1, CanonicalHingeAngles, DyadicIntervalClosureLimitsV1,
    DyadicMaterialHingeIntervalClosureCertificateV1, GeneratedMultiHingePathCandidateV1,
    HingeAngle, MultiHingePathCandidateLimitsV1, generate_linear_multi_hinge_path_candidate_v1,
};
use ori_topology::{FaceExtractionInput, analyze_faces};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::State;

#[path = "stacked_fold_non_flat_continuation_authority.rs"]
mod authority;
use authority::{
    authority_revalidates_v1, canonical_target_angles_v1, exact_hinge_angles_match_v1,
    freshly_analyze_flat_layer_order_v1, lowercase_hex_v1, map_non_flat_layer_error_v1,
    map_non_flat_transport_error_v1, non_flat_cycle_authority_binding_v1, parse_sha256_v1,
};

use super::super::{
    AppState,
    applied_pose::{
        CurrentAppliedPoseCapability, lock_revalidated_current_applied_pose_for_commit,
        restore_persisted_current_pose_transactional_v1,
    },
    global_flat_foldability::{
        GlobalFlatFoldabilityState, lock_current_layer_order_for_history_mutation,
    },
    lock_project,
    stacked_fold_transaction::{
        CurrentLayerEvidence, StackedFoldProjectRollbackSnapshotV1, rollback_stacked_fold_apply_v1,
    },
};
use super::{
    CYCLE_PATH_RESOURCE_MESSAGE, CYCLE_PATH_UNCERTIFIED_MESSAGE, CYCLE_PATH_UNSUPPORTED_MESSAGE,
    MAX_STACKED_FOLD_REQUEST_HINGES_V1, STALE_MESSAGE, UNAVAILABLE_MESSAGE,
    pose_state_fingerprint_v1, production_cycle_schedule_limits_v1,
};
const NON_FLAT_CYCLE_CONTINUATION_MODEL_ID_V1: &str =
    "native_non_flat_cycle_continuation_authority_v1";
const NON_FLAT_CYCLE_MAX_FACES_V1: usize = 64;
const NON_FLAT_CYCLE_MAX_PAIRS_V1: usize =
    NON_FLAT_CYCLE_MAX_FACES_V1 * (NON_FLAT_CYCLE_MAX_FACES_V1 - 1) / 2;
const NON_FLAT_CYCLE_MAX_BOUNDARY_POINTS_V1: usize =
    NON_FLAT_CYCLE_MAX_PAIRS_V1 * NON_FLAT_CYCLE_MAX_FACES_V1;

#[cfg(test)]
thread_local! {
    static FAIL_NEXT_NON_FLAT_POSE_REISSUE_V1: Cell<bool> = const { Cell::new(false) };
}

#[cfg(test)]
struct NonFlatPoseReissueFailureGuardV1 {
    _not_send_or_sync: PhantomData<Rc<()>>,
}

#[cfg(test)]
impl Drop for NonFlatPoseReissueFailureGuardV1 {
    fn drop(&mut self) {
        FAIL_NEXT_NON_FLAT_POSE_REISSUE_V1.set(false);
    }
}

#[cfg(test)]
fn fail_next_non_flat_pose_reissue_for_test_v1() -> NonFlatPoseReissueFailureGuardV1 {
    FAIL_NEXT_NON_FLAT_POSE_REISSUE_V1.with(|slot| {
        assert!(!slot.replace(true), "a pose-reissue fault is already armed");
    });
    NonFlatPoseReissueFailureGuardV1 {
        _not_send_or_sync: PhantomData,
    }
}

#[cfg(test)]
fn take_non_flat_pose_reissue_failure_for_test_v1() -> bool {
    FAIL_NEXT_NON_FLAT_POSE_REISSUE_V1.with(Cell::take)
}

#[derive(Clone, Copy)]
struct NonFlatCycleContinuationLimitsV1 {
    max_face_pairs: usize,
    transport: NonFlatCellTransportLimitsV1,
}

impl Default for NonFlatCycleContinuationLimitsV1 {
    fn default() -> Self {
        Self {
            max_face_pairs: NON_FLAT_CYCLE_MAX_PAIRS_V1,
            transport: NonFlatCellTransportLimitsV1 {
                max_faces: NON_FLAT_CYCLE_MAX_FACES_V1,
                max_cells: NON_FLAT_CYCLE_MAX_PAIRS_V1,
                max_pairs: NON_FLAT_CYCLE_MAX_PAIRS_V1,
                max_boundary_points: NON_FLAT_CYCLE_MAX_BOUNDARY_POINTS_V1,
            },
        }
    }
}

#[derive(Default)]
pub(crate) struct NonFlatCycleContinuationState(Mutex<Option<NonFlatCycleContinuationRecordV1>>);

struct NonFlatCycleContinuationRecordV1 {
    token: ProjectId,
    project_instance_id: ProjectId,
    project_id: ProjectId,
    revision: u64,
    source_fingerprint: [u8; 32],
    pose_generation: u64,
    authority_binding: [u8; 32],
    authority: NonFlatCycleContinuationAuthorityV1,
}

struct NonFlatCycleContinuationAuthorityV1 {
    pose_capability: CurrentAppliedPoseCapability,
    generated: GeneratedMultiHingePathCandidateV1,
    closure: DyadicMaterialHingeIntervalClosureCertificateV1,
    positive: PositiveThicknessTreeContinuousCertificateV1,
    source: StackedFoldNonFlatLayerOrderV1,
    target: StackedFoldNonFlatLayerOrderV1,
    transport: NonFlatCellTransportProofV1<StackedFoldNonFlatLayerOrderV1>,
    path: ori_collision::CertifiedPoseGraphPathCertificateV1,
    paper_thickness_bits: u64,
    binding: [u8; 32],
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct NonFlatCycleContinuationPreviewRequestV1 {
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    target_angles: Vec<NonFlatCycleContinuationAngleV1>,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct NonFlatCycleContinuationAngleV1 {
    edge: ori_domain::EdgeId,
    angle_degrees: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NonFlatCycleContinuationPreviewResponseV1 {
    version: u32,
    model_id: &'static str,
    preview_token: ProjectId,
    project_instance_id: ProjectId,
    project_id: ProjectId,
    source_revision: u64,
    target_revision: u64,
    source_pose_sha256: String,
    target_pose_sha256: String,
    authority_binding_sha256: String,
    continuous_path_certified: bool,
    non_flat_cell_transport_certified: bool,
    transported_cell_count: usize,
    transported_pair_count: usize,
    authorizes_project_mutation: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ApplyNonFlatCycleContinuationRequestV1 {
    preview_token: ProjectId,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    expected_target_pose_sha256: String,
    expected_authority_binding_sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CancelNonFlatCycleContinuationRequestV1 {
    preview_token: ProjectId,
}

#[tauri::command]
pub(crate) fn mint_non_flat_cycle_continuation_v1(
    app_state: State<'_, AppState>,
    state: State<'_, NonFlatCycleContinuationState>,
    request: NonFlatCycleContinuationPreviewRequestV1,
) -> Result<NonFlatCycleContinuationPreviewResponseV1, String> {
    mint_non_flat_cycle_continuation_inner_v1(
        &app_state,
        &state,
        request,
        NonFlatCycleContinuationLimitsV1::default(),
    )
}

fn mint_non_flat_cycle_continuation_inner_v1(
    app_state: &AppState,
    state: &NonFlatCycleContinuationState,
    request: NonFlatCycleContinuationPreviewRequestV1,
    limits: NonFlatCycleContinuationLimitsV1,
) -> Result<NonFlatCycleContinuationPreviewResponseV1, String> {
    if request.target_angles.is_empty()
        || request.target_angles.len() > MAX_STACKED_FOLD_REQUEST_HINGES_V1
    {
        return Err(CYCLE_PATH_RESOURCE_MESSAGE.to_owned());
    }
    let target = canonical_target_angles_v1(&request.target_angles)?;
    if target.as_slice().iter().any(|angle| {
        !angle.angle_degrees().is_finite()
            || angle.angle_degrees() < 0.0
            || angle.angle_degrees() >= 180.0
            || angle.angle_degrees().to_bits() == (-0.0_f64).to_bits()
    }) {
        return Err(CYCLE_PATH_UNSUPPORTED_MESSAGE.to_owned());
    }

    let (
        pattern,
        paper,
        source_fingerprint,
        pose_capability,
        source_evidence,
        project_instance_id,
        project_id,
        revision,
    ) = {
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
            .map_err(|_| UNAVAILABLE_MESSAGE.to_owned())?
            .ok_or_else(|| CYCLE_PATH_UNSUPPORTED_MESSAGE.to_owned())?;
        let source_evidence = match &project.current_layer_evidence {
            Some(CurrentLayerEvidence::NonFlat(value)) => value.clone(),
            Some(CurrentLayerEvidence::CertifiedFlat(_)) | None => {
                return Err(CYCLE_PATH_UNSUPPORTED_MESSAGE.to_owned());
            }
        };
        (
            project.editor.pattern().clone(),
            project.editor.paper().clone(),
            ori_foldability::fold_model_fingerprint_v1(
                project.editor.pattern(),
                project.editor.paper(),
            )
            .0,
            pose_capability,
            source_evidence,
            project.instance_id,
            project.project_id,
            project.editor.revision(),
        )
    };

    let target_revision = revision
        .checked_add(1)
        .filter(|value| *value <= ori_core::MAX_REVISION)
        .ok_or_else(|| CYCLE_PATH_RESOURCE_MESSAGE.to_owned())?;
    let paper_thickness_mm = paper.thickness_mm;
    if !paper_thickness_mm.is_finite() || paper_thickness_mm <= 0.0 {
        return Err(CYCLE_PATH_UNSUPPORTED_MESSAGE.to_owned());
    }
    let (tree_model, tree_pose) = pose_capability
        .tree()
        .ok_or_else(|| CYCLE_PATH_UNSUPPORTED_MESSAGE.to_owned())?;
    let (geometry, audit, graph_pose) = pose_capability
        .graph()
        .ok_or_else(|| CYCLE_PATH_UNSUPPORTED_MESSAGE.to_owned())?;
    if geometry.face_ids().len() > NON_FLAT_CYCLE_MAX_FACES_V1
        || geometry.hinges().len() != target.as_slice().len()
        || !exact_hinge_angles_match_v1(
            tree_pose.hinge_angles(),
            graph_pose.hinge_angles().as_slice(),
        )
        || !exact_hinge_angles_match_v1(tree_pose.hinge_angles(), source_evidence.hinge_angles())
        || tree_pose.hinge_angles().iter().any(|angle| {
            !angle.angle_degrees().is_finite()
                || angle.angle_degrees() < 0.0
                || angle.angle_degrees() >= 180.0
                || angle.angle_degrees().to_bits() == (-0.0_f64).to_bits()
        })
        || source_evidence.identity_namespace() != project_id
        || source_evidence.target_revision() != revision
        || source_evidence.target_fingerprint().0 != source_fingerprint
        || source_evidence.fixed_face() != tree_pose.fixed_face()
        || source_evidence.fixed_face() != Some(graph_pose.fixed_face())
    {
        return Err(CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned());
    }

    let generated = generate_linear_multi_hinge_path_candidate_v1(
        geometry,
        audit,
        graph_pose.fixed_face(),
        graph_pose.hinge_angles(),
        &target,
        MultiHingePathCandidateLimitsV1 {
            max_hinges: MAX_STACKED_FOLD_REQUEST_HINGES_V1,
            max_candidates: 1,
            max_work: MAX_STACKED_FOLD_REQUEST_HINGES_V1 * 2,
        },
    )
    .map_err(|error| match error {
        ori_kinematics::MultiHingePathCandidateErrorV1::ResourceLimit => {
            CYCLE_PATH_RESOURCE_MESSAGE.to_owned()
        }
        _ => CYCLE_PATH_UNSUPPORTED_MESSAGE.to_owned(),
    })?;
    let schedule = generated.schedule();
    let schedule_source = schedule
        .evaluate(0.0)
        .ok_or_else(|| CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned())?;
    let schedule_target = schedule
        .evaluate(1.0)
        .ok_or_else(|| CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned())?;
    if !exact_hinge_angles_match_v1(schedule_source.as_slice(), tree_pose.hinge_angles())
        || !exact_hinge_angles_match_v1(schedule_target.as_slice(), target.as_slice())
    {
        return Err(CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned());
    }
    let closure = geometry
        .prove_dyadic_schedule_closure_v1(
            audit,
            graph_pose.fixed_face(),
            schedule,
            ori_core::STACKED_FOLD_GRAPH_CLOSURE_TOLERANCE_V1,
            DyadicIntervalClosureLimitsV1 {
                max_depth: 8,
                max_leaves: 256,
                max_work: 1_048_576,
                schedule_limits: production_cycle_schedule_limits_v1(),
            },
        )
        .map_err(|_| CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned())?;
    let positive = certify_positive_thickness_tree_continuous_path_v1(
        tree_model,
        tree_pose,
        &target,
        paper_thickness_mm,
    )
    .ok_or_else(|| CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned())?;

    let source_flat = freshly_analyze_flat_layer_order_v1(project_id, revision, &pattern, &paper)?;
    let freshly_revalidated_source = revalidate_current_non_flat_layer_order_v1(
        project_id,
        revision,
        &pattern,
        &paper,
        tree_pose.fixed_face(),
        &schedule_source,
        &source_flat,
        limits.max_face_pairs,
    )
    .map_err(map_non_flat_layer_error_v1)?;
    if freshly_revalidated_source != source_evidence {
        return Err(CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned());
    }
    let target_flat =
        freshly_analyze_flat_layer_order_v1(project_id, target_revision, &pattern, &paper)?;
    let target_evidence = revalidate_current_non_flat_layer_order_v1(
        project_id,
        target_revision,
        &pattern,
        &paper,
        tree_pose.fixed_face(),
        &target,
        &target_flat,
        limits.max_face_pairs,
    )
    .map_err(map_non_flat_layer_error_v1)?;
    if !exact_hinge_angles_match_v1(target_evidence.hinge_angles(), target.as_slice()) {
        return Err(CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned());
    }
    let transport = certify_non_flat_cell_transport_with_limits_v1(
        &source_evidence,
        &target_evidence,
        limits.transport,
    )
    .map_err(map_non_flat_transport_error_v1)?;

    let source_pose = pose_state_fingerprint_v1(&schedule_source);
    let target_pose = pose_state_fingerprint_v1(&target);
    let transition = CertifiedPathTransitionEvidenceV1::from_native_oracle(
        source_pose,
        target_pose,
        schedule.certificate_binding_fingerprint_v2(),
        positive.binding_fingerprint_v1(),
        closure.partition_binding_fingerprint_v2(),
    );
    let path = match search_certified_pose_graph_v1(
        &[source_pose, target_pose],
        &[CertifiedPathTransitionCandidateV1 {
            source: source_pose,
            target: target_pose,
            candidate_key: schedule.certificate_binding_fingerprint_v2(),
        }],
        source_pose,
        target_pose,
        |_| Some(transition),
    ) {
        CertifiedPathGraphSearchResultV1::Certified(value) => value,
        CertifiedPathGraphSearchResultV1::Indeterminate { .. } => {
            return Err(CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned());
        }
    };
    let binding = non_flat_cycle_authority_binding_v1(
        project_id,
        revision,
        source_fingerprint,
        pose_capability.generation(),
        schedule,
        &closure,
        &positive,
        &source_evidence,
        &target_evidence,
        &path,
        paper_thickness_mm,
    );
    let authority = NonFlatCycleContinuationAuthorityV1 {
        pose_capability,
        generated,
        closure,
        positive,
        source: source_evidence,
        target: target_evidence,
        transport,
        path,
        paper_thickness_bits: paper_thickness_mm.to_bits(),
        binding,
    };
    if !authority_revalidates_v1(
        &authority,
        project_id,
        revision,
        source_fingerprint,
        authority.pose_capability.generation(),
    ) {
        return Err(CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned());
    }

    let token = ProjectId::new();
    let response = NonFlatCycleContinuationPreviewResponseV1 {
        version: 1,
        model_id: NON_FLAT_CYCLE_CONTINUATION_MODEL_ID_V1,
        preview_token: token,
        project_instance_id,
        project_id,
        source_revision: revision,
        target_revision,
        source_pose_sha256: lowercase_hex_v1(source_pose),
        target_pose_sha256: lowercase_hex_v1(target_pose),
        authority_binding_sha256: lowercase_hex_v1(binding),
        continuous_path_certified: true,
        non_flat_cell_transport_certified: true,
        transported_cell_count: authority.target.overlap_cell_count(),
        transported_pair_count: authority.target.face_pair_order_count(),
        authorizes_project_mutation: false,
    };

    // Detached proof construction must not create a publication race. Rebind
    // every live source slot immediately before making the token observable.
    let project = lock_project(app_state).map_err(|_| UNAVAILABLE_MESSAGE.to_owned())?;
    let live_fingerprint = ori_foldability::fold_model_fingerprint_v1(
        project.editor.pattern(),
        project.editor.paper(),
    )
    .0;
    let live_source = match &project.current_layer_evidence {
        Some(CurrentLayerEvidence::NonFlat(value)) => value,
        Some(CurrentLayerEvidence::CertifiedFlat(_)) | None => {
            return Err(CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned());
        }
    };
    if project.instance_id != project_instance_id
        || project.project_id != project_id
        || project.editor.revision() != revision
        || live_fingerprint != source_fingerprint
        || live_source != &authority.source
        || project
            .applied_pose_authority
            .revalidate_capability(&project, &authority.pose_capability)
            .map_err(|_| UNAVAILABLE_MESSAGE.to_owned())?
            .is_none()
        || !authority_revalidates_v1(
            &authority,
            project_id,
            revision,
            source_fingerprint,
            authority.pose_capability.generation(),
        )
    {
        return Err(STALE_MESSAGE.to_owned());
    }
    let mut slot = state
        .0
        .try_lock()
        .map_err(|_| UNAVAILABLE_MESSAGE.to_owned())?;
    let retired = slot.replace(NonFlatCycleContinuationRecordV1 {
        token,
        project_instance_id,
        project_id,
        revision,
        source_fingerprint,
        pose_generation: authority.pose_capability.generation(),
        authority_binding: binding,
        authority,
    });
    drop(slot);
    drop(project);
    drop(retired);
    Ok(response)
}

#[tauri::command]
pub(crate) fn apply_non_flat_cycle_continuation_v1(
    app_state: State<'_, AppState>,
    foldability_state: State<'_, GlobalFlatFoldabilityState>,
    state: State<'_, NonFlatCycleContinuationState>,
    request: ApplyNonFlatCycleContinuationRequestV1,
) -> Result<u64, String> {
    apply_non_flat_cycle_continuation_inner_v1(&app_state, &foldability_state, &state, request)
}

fn apply_non_flat_cycle_continuation_inner_v1(
    app_state: &AppState,
    foldability_state: &GlobalFlatFoldabilityState,
    state: &NonFlatCycleContinuationState,
    request: ApplyNonFlatCycleContinuationRequestV1,
) -> Result<u64, String> {
    // Token equality is the first request-dependent validation.  Consume a
    // matching token before parsing caller echoes or consulting mutable
    // project state, so malformed/stale/tampered matching attempts cannot be
    // corrected and replayed.
    let mut slot = state.0.lock().map_err(|_| UNAVAILABLE_MESSAGE.to_owned())?;
    let current_record = slot
        .as_ref()
        .ok_or_else(|| CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned())?;
    if current_record.token != request.preview_token {
        return Err(CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned());
    }
    // A matching Apply attempt consumes the token before any proof-dependent
    // branch, so neither a failed validation nor a rollback can replay it.
    let record = slot
        .take()
        .ok_or_else(|| CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned())?;
    drop(slot);
    let expected_target_pose = parse_sha256_v1(&request.expected_target_pose_sha256)?;
    let expected_binding = parse_sha256_v1(&request.expected_authority_binding_sha256)?;
    let mut project = lock_project(app_state).map_err(|_| UNAVAILABLE_MESSAGE.to_owned())?;
    if project.instance_id != request.expected_project_instance_id
        || project.project_id != request.expected_project_id
        || project.editor.revision() != request.expected_revision
    {
        return Err(STALE_MESSAGE.to_owned());
    }
    let current_source = match &project.current_layer_evidence {
        Some(CurrentLayerEvidence::NonFlat(value)) => value,
        Some(CurrentLayerEvidence::CertifiedFlat(_)) | None => {
            return Err(CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned());
        }
    };
    if record.project_instance_id != request.expected_project_instance_id
        || record.project_id != request.expected_project_id
        || record.revision != request.expected_revision
        || record.source_fingerprint
            != ori_foldability::fold_model_fingerprint_v1(
                project.editor.pattern(),
                project.editor.paper(),
            )
            .0
        || record.authority_binding != expected_binding
        || record.authority.binding != expected_binding
        || pose_state_fingerprint_v1(
            &CanonicalHingeAngles::new(record.authority.target.hinge_angles().to_vec())
                .map_err(|_| CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned())?,
        ) != expected_target_pose
        || current_source != &record.authority.source
        || !authority_revalidates_v1(
            &record.authority,
            record.project_id,
            record.revision,
            record.source_fingerprint,
            record.pose_generation,
        )
    {
        return Err(CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned());
    }
    if project
        .applied_pose_authority
        .revalidate_capability(&project, &record.authority.pose_capability)
        .map_err(|_| UNAVAILABLE_MESSAGE.to_owned())?
        .is_none()
    {
        return Err(STALE_MESSAGE.to_owned());
    }
    let pose_guard = lock_revalidated_current_applied_pose_for_commit(
        &project,
        &record.authority.pose_capability,
    )
    .map_err(|_| UNAVAILABLE_MESSAGE.to_owned())?
    .ok_or_else(|| STALE_MESSAGE.to_owned())?;
    let mut layer_guard = lock_current_layer_order_for_history_mutation(foldability_state)
        .map_err(|_| UNAVAILABLE_MESSAGE.to_owned())?;
    let (geometry, _, source_pose) = record
        .authority
        .pose_capability
        .graph()
        .ok_or_else(|| CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned())?;
    let target_angles = CanonicalHingeAngles::new(record.authority.target.hinge_angles().to_vec())
        .map_err(|_| CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned())?;
    let target_angle_pairs = target_angles
        .as_slice()
        .iter()
        .map(|angle| (angle.edge(), angle.angle_degrees()))
        .collect::<Vec<_>>();
    let face_ids = geometry.face_ids().to_vec();
    let hinge_ids = geometry
        .hinges()
        .iter()
        .map(|hinge| hinge.edge())
        .collect::<Vec<_>>();
    let applied_pose = prepare_closed_graph_applied_pose_v1(
        &face_ids,
        &hinge_ids,
        source_pose.fixed_face(),
        &target_angle_pairs,
        AppliedPoseLimitsV1::default(),
    )
    .map_err(|_| CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned())?;
    let source_angles = source_pose
        .hinge_angles()
        .as_slice()
        .iter()
        .map(|angle| InstructionHingeAngle {
            edge: angle.edge(),
            angle_degrees: angle.angle_degrees(),
        })
        .collect::<Vec<_>>();
    let transition_targets = vec![
        target_angles
            .as_slice()
            .iter()
            .map(|angle| InstructionHingeAngle {
                edge: angle.edge(),
                angle_degrees: angle.angle_degrees(),
            })
            .collect::<Vec<_>>(),
    ];
    let timeline = ori_instructions::append_certified_dyadic_path_timeline_v1(
        project.editor.instruction_timeline(),
        "Certified non-flat cycle continuation",
        &project.editor.fold_model_fingerprint_v1(),
        source_pose.fixed_face(),
        &source_angles,
        &transition_targets,
        &record.authority.path,
    )
    .map_err(|_| CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned())?;
    let persisted_pose = timeline
        .steps
        .last()
        .map(|step| step.pose.clone())
        .ok_or_else(|| CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned())?;
    let mut project_before = StackedFoldProjectRollbackSnapshotV1::capture(&project);
    let layer_before = layer_guard.capture_rollback_snapshot_v1();
    let next_pattern = project.editor.pattern().clone();
    let next_paper = project.editor.paper().clone();
    let next_project_layers = project.editor.project_layers().clone();
    let result = match project.editor.execute_stacked_fold_document(
        record.revision,
        next_pattern,
        next_paper,
        timeline,
        next_project_layers,
        applied_pose,
    ) {
        Ok(result) => result,
        Err(_) => {
            drop(pose_guard);
            project_before
                .restore(&mut project)
                .map_err(|_| UNAVAILABLE_MESSAGE.to_owned())?;
            layer_guard.restore_rollback_snapshot_v1(&layer_before);
            return Err(CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned());
        }
    };
    drop(pose_guard);
    #[cfg(test)]
    let pose_reissue = if take_non_flat_pose_reissue_failure_for_test_v1() {
        super::super::applied_pose::restore_persisted_current_pose_failing_after_prepare_for_test_v1(
            &mut project,
            &persisted_pose,
        )
    } else {
        restore_persisted_current_pose_transactional_v1(&mut project, &persisted_pose)
    };
    #[cfg(not(test))]
    let pose_reissue =
        restore_persisted_current_pose_transactional_v1(&mut project, &persisted_pose);
    let mut pose_rollback = match pose_reissue {
        Ok(value) => value,
        Err(_) => {
            project_before
                .restore(&mut project)
                .map_err(|_| UNAVAILABLE_MESSAGE.to_owned())?;
            layer_guard.restore_rollback_snapshot_v1(&layer_before);
            return Err(UNAVAILABLE_MESSAGE.to_owned());
        }
    };
    if result.revision != record.authority.target.target_revision()
        || ori_foldability::fold_model_fingerprint_v1(
            project.editor.pattern(),
            project.editor.paper(),
        )
        .0 != record.authority.target.target_fingerprint().0
    {
        rollback_stacked_fold_apply_v1(
            &mut project,
            &mut project_before,
            &mut pose_rollback,
            Some(&mut layer_guard),
            Some(&layer_before),
        )
        .map_err(|_| UNAVAILABLE_MESSAGE.to_owned())?;
        return Err(CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned());
    }
    project.current_layer_evidence = Some(CurrentLayerEvidence::NonFlat(
        record.authority.target.clone(),
    ));
    layer_guard.invalidate_after_project_mutation();
    pose_rollback.disarm();
    drop(layer_guard);
    Ok(result.revision)
}

#[tauri::command]
pub(crate) fn cancel_non_flat_cycle_continuation_v1(
    state: State<'_, NonFlatCycleContinuationState>,
    request: CancelNonFlatCycleContinuationRequestV1,
) -> Result<(), String> {
    cancel_non_flat_cycle_continuation_inner_v1(&state, request.preview_token)
}

fn cancel_non_flat_cycle_continuation_inner_v1(
    state: &NonFlatCycleContinuationState,
    token: ProjectId,
) -> Result<(), String> {
    let mut slot = state.0.lock().map_err(|_| UNAVAILABLE_MESSAGE.to_owned())?;
    if slot.as_ref().is_none_or(|record| record.token != token) {
        return Err(CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned());
    }
    let retired = slot.take();
    drop(slot);
    drop(retired);
    Ok(())
}

#[cfg(test)]
#[path = "stacked_fold_non_flat_continuation_tests.rs"]
mod tests;
