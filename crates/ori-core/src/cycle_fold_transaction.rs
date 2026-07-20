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

/// Single-use, non-persistable authority for one continuously certified fold.
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
    if certificate.leaves().is_empty()
        || !schedule.matches_binding(geometry, audit, fixed)
        || certificate.schedule_binding_fingerprint_v1()
            != schedule.certificate_binding_fingerprint_v1()
        || certificate
            .leaves()
            .iter()
            .any(|(_, _, leaf)| leaf.fixed_face() != fixed)
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

    fn editor() -> EditorState {
        create_rectangular_sheet(100.0, 100.0, false)
            .unwrap()
            .into_editor_state()
    }

    fn ready(editor: &EditorState, project: ProjectId) -> ReadyCycleFoldTransactionV1 {
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
        let mut token = ready(&editor, project);
        let initial = editor.revision();
        let pattern_before = editor.pattern().clone();
        let applied =
            apply_ready_cycle_fold_transaction_v1(project, &mut editor, &mut token).unwrap();
        assert!(applied.revision > initial);
        let pattern_after = editor.pattern().clone();
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
        assert!(editor.current_applied_pose().is_some());

        let mut stale = ready(&editor, project);
        editor
            .execute(
                editor.revision(),
                crate::Command::UpdateProjectMemo {
                    memo: "ABA".to_owned(),
                },
            )
            .unwrap();
        assert!(matches!(
            apply_ready_cycle_fold_transaction_v1(project, &mut editor, &mut stale),
            Err(CycleFoldTransactionErrorV1::RevisionChanged)
        ));
        assert!(stale.payload.is_some());
    }

    #[test]
    fn project_and_pose_aba_fail_without_consuming_authority() {
        let project = ProjectId::new();
        let mut editor = editor();
        let mut wrong_project = ready(&editor, project);
        assert!(matches!(
            apply_ready_cycle_fold_transaction_v1(
                ProjectId::new(),
                &mut editor,
                &mut wrong_project
            ),
            Err(CycleFoldTransactionErrorV1::ProjectChanged)
        ));
        assert!(wrong_project.payload.is_some());

        let mut pose_changed = ready(&editor, project);
        let replacement = pose_changed.payload.as_ref().unwrap().applied_pose.clone();
        editor.adopt_current_applied_pose(replacement);
        assert!(matches!(
            apply_ready_cycle_fold_transaction_v1(project, &mut editor, &mut pose_changed),
            Err(CycleFoldTransactionErrorV1::PoseChanged)
        ));
        assert!(pose_changed.payload.is_some());
    }
}
