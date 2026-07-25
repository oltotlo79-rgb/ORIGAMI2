//! Low-level atomic document transaction for a caller-certified cycle fold.
//!
//! This V1 primitive binds one `EditorState` commit to a project ID, revision,
//! source fold-model fingerprint, and previous persisted pose. It does not
//! authenticate an open-project instance or any runtime pose/layer capability,
//! and it is not the desktop application's mutation authority.
//!
//! The caller-supplied pattern, paper, timeline, layers, and applied pose are
//! not derived from or cryptographically bound to the closure certificate.
//! Callers must separately prove that this payload represents the certified
//! schedule target before preparing a transaction.

use ori_domain::{CreasePattern, InstructionTimeline, Paper, ProjectId, ProjectLayerDocumentV1};
use ori_kinematics::{
    CanonicalCycleScheduleV1, DyadicMaterialHingeIntervalClosureCertificateV1,
    MaterialHingeGraphAudit, MaterialHingeGraphGeometry,
};
use thiserror::Error;

use crate::{AppliedPoseV1, CommandError, CommandResult, EditorState, Revision};

#[derive(Debug, Clone)]
struct CycleFoldPayloadV1 {
    pattern: CreasePattern,
    paper: Paper,
    instruction_timeline: InstructionTimeline,
    project_layers: ProjectLayerDocumentV1,
    applied_pose: AppliedPoseV1,
}

/// Single-use, non-persistable handle for one atomic document commit.
///
/// This handle is not a runtime pose, layer-order, open-instance, or desktop
/// mutation authority. Its caller remains responsible for binding the
/// caller-supplied payload to the certified schedule target.
#[derive(Debug)]
pub struct ReadyCycleFoldTransactionV1 {
    project: ProjectId,
    revision: Revision,
    fold_model_fingerprint: String,
    previous_pose: Option<AppliedPoseV1>,
    payload: Option<CycleFoldPayloadV1>,
}

#[derive(Debug, Error)]
pub enum CycleFoldTransactionErrorV1 {
    #[error("the closure certificate is not bound to this schedule and material graph")]
    BindingMismatch,
    #[error("the project identity changed after preparation")]
    ProjectChanged,
    #[error("the editor revision changed after preparation")]
    RevisionChanged,
    #[error("the fold geometry or hinge semantics changed after preparation")]
    FoldModelChanged,
    #[error("the runtime pose changed after preparation")]
    PoseChanged,
    #[error("the cycle-fold transaction was already consumed")]
    AlreadyConsumed,
    #[error("the prepared document could not be applied atomically")]
    ApplyFailed(#[from] CommandError),
}

#[allow(clippy::too_many_arguments)]
/// Prepares a low-level atomic document transaction after validating the
/// certificate, schedule, and material-graph binding.
///
/// This function does not establish that the caller-supplied document payload
/// corresponds to the certificate's target. That relationship, along with any
/// runtime pose/layer and open-instance authority, must be validated by the
/// caller.
pub fn prepare_cycle_fold_transaction_v1(
    project: ProjectId,
    editor: &EditorState,
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    schedule: &CanonicalCycleScheduleV1,
    certificate: DyadicMaterialHingeIntervalClosureCertificateV1,
    pattern: CreasePattern,
    paper: Paper,
    instruction_timeline: InstructionTimeline,
    project_layers: ProjectLayerDocumentV1,
    applied_pose: AppliedPoseV1,
) -> Result<ReadyCycleFoldTransactionV1, CycleFoldTransactionErrorV1> {
    let fixed = certificate.fixed_face();
    let mut graph_hinges = geometry
        .hinges()
        .iter()
        .map(|hinge| hinge.edge())
        .collect::<Vec<_>>();
    graph_hinges.sort_unstable_by_key(ori_domain::EdgeId::canonical_bytes);
    if !certificate.has_canonical_complete_partition_v1()
        || !schedule.matches_binding(geometry, audit, fixed)
        || certificate.schedule_binding_fingerprint_v1()
            != schedule.certificate_binding_fingerprint_v1()
        || certificate.graph_binding_fingerprint_v1() != schedule.graph_binding_fingerprint_v1()
        || certificate
            .leaves()
            .iter()
            .any(|(_, _, leaf)| leaf.fixed_face() != fixed || leaf.checked_hinges() != graph_hinges)
    {
        return Err(CycleFoldTransactionErrorV1::BindingMismatch);
    }
    Ok(ReadyCycleFoldTransactionV1 {
        project,
        revision: editor.revision(),
        fold_model_fingerprint: editor.fold_model_fingerprint_v1(),
        previous_pose: editor.current_applied_pose().cloned(),
        payload: Some(CycleFoldPayloadV1 {
            pattern,
            paper,
            instruction_timeline,
            project_layers,
            applied_pose,
        }),
    })
}

/// Applies the prepared document payload as one `EditorState` history entry.
///
/// This revalidates the persisted project/revision/model/pose binding only. It
/// neither installs nor authenticates desktop runtime pose or layer authority.
pub fn apply_ready_cycle_fold_transaction_v1(
    project: ProjectId,
    editor: &mut EditorState,
    ready: &mut ReadyCycleFoldTransactionV1,
) -> Result<CommandResult, CycleFoldTransactionErrorV1> {
    if ready.payload.is_none() {
        return Err(CycleFoldTransactionErrorV1::AlreadyConsumed);
    }
    if project != ready.project {
        return Err(CycleFoldTransactionErrorV1::ProjectChanged);
    }
    if editor.revision() != ready.revision {
        return Err(CycleFoldTransactionErrorV1::RevisionChanged);
    }
    if editor.fold_model_fingerprint_v1() != ready.fold_model_fingerprint {
        return Err(CycleFoldTransactionErrorV1::FoldModelChanged);
    }
    if editor.current_applied_pose() != ready.previous_pose.as_ref() {
        return Err(CycleFoldTransactionErrorV1::PoseChanged);
    }
    let payload = ready
        .payload
        .as_ref()
        .cloned()
        .ok_or(CycleFoldTransactionErrorV1::AlreadyConsumed)?;
    match editor.execute_stacked_fold_document(
        ready.revision,
        payload.pattern,
        payload.paper,
        payload.instruction_timeline,
        payload.project_layers,
        payload.applied_pose,
    ) {
        Ok(result) => {
            ready.payload = None;
            Ok(result)
        }
        Err(error) => {
            // Admission failures occur before mutation in EditorState::execute.
            Err(CycleFoldTransactionErrorV1::ApplyFailed(error))
        }
    }
}

#[cfg(test)]
mod tests {
    use ori_domain::{
        FaceId, InstructionPose, InstructionPoseModel, InstructionStep, InstructionStepId,
        InstructionVisual, MIN_INSTRUCTION_DURATION_MS, Point2, ProjectLayerDocumentV1, Vertex,
        VertexId,
    };

    use super::*;
    use crate::{AppliedPoseLimitsV1, create_rectangular_sheet, prepare_applied_pose_v1};

    #[derive(Debug, Clone, PartialEq)]
    struct EditorObservation {
        revision: Revision,
        pattern: CreasePattern,
        paper: Paper,
        timeline: InstructionTimeline,
        layers: ProjectLayerDocumentV1,
        pose: Option<AppliedPoseV1>,
        can_undo: bool,
        can_redo: bool,
    }

    fn observe(editor: &EditorState) -> EditorObservation {
        EditorObservation {
            revision: editor.revision(),
            pattern: editor.pattern().clone(),
            paper: editor.paper().clone(),
            timeline: editor.instruction_timeline().clone(),
            layers: editor.project_layers().clone(),
            pose: editor.current_applied_pose().cloned(),
            can_undo: editor.can_undo(),
            can_redo: editor.can_redo(),
        }
    }

    fn editor() -> EditorState {
        create_rectangular_sheet(100.0, 100.0, false)
            .unwrap()
            .into_editor_state()
    }

    fn ready(editor: &EditorState, project: ProjectId) -> ReadyCycleFoldTransactionV1 {
        // Tests construct the low-level handle directly because this module
        // verifies atomic EditorState behavior, not desktop runtime authority
        // or a certificate-to-payload semantic binding.
        let face = FaceId::new();
        let pose = prepare_applied_pose_v1(&[face], &[], None, &[], AppliedPoseLimitsV1::default())
            .unwrap();
        let mut pattern = editor.pattern().clone();
        pattern.vertices.push(Vertex {
            id: VertexId::new(),
            position: Point2::new(50.0, 50.0),
        });
        let mut instruction_timeline = editor.instruction_timeline().clone();
        instruction_timeline.steps.push(InstructionStep {
            id: InstructionStepId::new(),
            title: "Cycle fold".to_owned(),
            description: String::new(),
            caution: String::new(),
            duration_ms: MIN_INSTRUCTION_DURATION_MS,
            visual: InstructionVisual::default(),
            pose: InstructionPose {
                model: InstructionPoseModel::AbsoluteHingeAnglesV1,
                source_model_fingerprint: crate::fold_model_fingerprint_v1(
                    &pattern,
                    editor.paper(),
                ),
                fixed_face: None,
                hinge_angles: Vec::new(),
            },
        });
        ReadyCycleFoldTransactionV1 {
            project,
            revision: editor.revision(),
            fold_model_fingerprint: editor.fold_model_fingerprint_v1(),
            previous_pose: editor.current_applied_pose().cloned(),
            payload: Some(CycleFoldPayloadV1 {
                pattern,
                paper: editor.paper().clone(),
                instruction_timeline,
                project_layers: ProjectLayerDocumentV1::default(),
                applied_pose: pose,
            }),
        }
    }

    #[test]
    fn ready_transaction_is_single_use_and_revision_bound() {
        let project = ProjectId::new();
        let mut editor = editor();
        editor
            .set_history_entry_limit(1)
            .expect("minimum history endpoint");
        let mut token = ready(&editor, project);
        let initial = editor.revision();
        let pattern_before = editor.pattern().clone();
        let applied =
            apply_ready_cycle_fold_transaction_v1(project, &mut editor, &mut token).unwrap();
        assert!(applied.revision > initial);
        let pattern_after = editor.pattern().clone();
        let paper_after = editor.paper().clone();
        let timeline_after = editor.instruction_timeline().clone();
        let layers_after = editor.project_layers().clone();
        let pose_after = editor.current_applied_pose().cloned();
        assert_ne!(pattern_after, pattern_before);
        assert!(editor.current_applied_pose().is_some());
        assert!(matches!(
            apply_ready_cycle_fold_transaction_v1(project, &mut editor, &mut token),
            Err(CycleFoldTransactionErrorV1::AlreadyConsumed)
        ));
        editor.undo(editor.revision()).unwrap();
        assert_eq!(editor.pattern(), &pattern_before);
        assert!(editor.current_applied_pose().is_none());
        editor.redo(editor.revision()).unwrap();
        assert_eq!(editor.pattern(), &pattern_after);
        assert_eq!(editor.paper(), &paper_after);
        assert_eq!(editor.instruction_timeline(), &timeline_after);
        assert_eq!(editor.project_layers(), &layers_after);
        assert_eq!(editor.current_applied_pose(), pose_after.as_ref());
        assert!(
            !editor.can_redo(),
            "one apply remains exactly one history entry"
        );

        let mut stale = ready(&editor, project);
        editor
            .execute(
                editor.revision(),
                crate::Command::UpdateProjectMemo {
                    memo: "ABA".to_owned(),
                },
            )
            .unwrap();
        let before_stale_rejection = observe(&editor);
        assert!(matches!(
            apply_ready_cycle_fold_transaction_v1(project, &mut editor, &mut stale),
            Err(CycleFoldTransactionErrorV1::RevisionChanged)
        ));
        assert!(stale.payload.is_some());
        assert_eq!(observe(&editor), before_stale_rejection);
    }

    #[test]
    fn project_and_pose_aba_fail_without_consuming_authority() {
        let project = ProjectId::new();
        let mut editor = editor();
        let mut wrong_project = ready(&editor, project);
        let before_wrong_project = observe(&editor);
        assert!(matches!(
            apply_ready_cycle_fold_transaction_v1(
                ProjectId::new(),
                &mut editor,
                &mut wrong_project
            ),
            Err(CycleFoldTransactionErrorV1::ProjectChanged)
        ));
        assert!(wrong_project.payload.is_some());
        assert_eq!(observe(&editor), before_wrong_project);

        let mut pose_changed = ready(&editor, project);
        let replacement = pose_changed.payload.as_ref().unwrap().applied_pose.clone();
        editor.adopt_current_applied_pose(replacement);
        let before_pose_rejection = observe(&editor);
        assert!(matches!(
            apply_ready_cycle_fold_transaction_v1(project, &mut editor, &mut pose_changed),
            Err(CycleFoldTransactionErrorV1::PoseChanged)
        ));
        assert!(pose_changed.payload.is_some());
        assert_eq!(observe(&editor), before_pose_rejection);
    }

    #[test]
    fn apply_failure_is_atomic_and_keeps_the_single_use_token_retryable() {
        let project = ProjectId::new();
        let mut editor = editor();
        editor
            .set_history_entry_limit(1)
            .expect("minimum history endpoint");
        let mut token = ready(&editor, project);
        let payload = token.payload.as_mut().expect("native-only payload");
        payload.paper.thickness_mm =
            f64::from_bits(payload.paper.thickness_mm.to_bits().saturating_add(1));
        let before = observe(&editor);

        assert!(matches!(
            apply_ready_cycle_fold_transaction_v1(project, &mut editor, &mut token),
            Err(CycleFoldTransactionErrorV1::ApplyFailed(
                CommandError::InvalidStackedFoldDocument
            ))
        ));
        assert_eq!(observe(&editor), before);
        assert!(
            token.payload.is_some(),
            "a failed apply must not consume the opaque authority"
        );
    }
}
