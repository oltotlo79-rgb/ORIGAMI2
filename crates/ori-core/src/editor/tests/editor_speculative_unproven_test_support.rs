use ori_domain::{
    ElementMetadataDocumentV1, InstructionHingeAngle, InstructionPoseModel,
    MIN_INSTRUCTION_DURATION_MS,
};

use super::*;

pub(super) struct SpeculativeFixture {
    pub(super) editor: EditorState,
    pub(super) target_pattern: CreasePattern,
    pub(super) paper: Paper,
    pub(super) timeline: InstructionTimeline,
    pub(super) applied_pose: AppliedPoseV1,
    pub(super) project_id: ProjectId,
    pub(super) project_instance_id: ProjectId,
}

pub(super) fn fixture() -> SpeculativeFixture {
    let sheet = crate::create_rectangular_sheet(80.0, 60.0, false).expect("rectangular sheet");
    let (source_pattern, mut paper) = sheet.into_parts();
    paper.thickness_mm = 0.1;
    let mut target_pattern = source_pattern.clone();
    let hinge = EdgeId::new();
    target_pattern.edges.push(Edge {
        id: hinge,
        start: paper.boundary_vertices[0],
        end: paper.boundary_vertices[2],
        kind: EdgeKind::Mountain,
    });
    let timeline = InstructionTimeline {
        steps: vec![InstructionStep {
            id: InstructionStepId::new(),
            title: "Speculative stacked fold".to_owned(),
            description: String::new(),
            caution: String::new(),
            duration_ms: MIN_INSTRUCTION_DURATION_MS,
            visual: InstructionVisual::default(),
            pose: InstructionPose {
                model: InstructionPoseModel::AbsoluteHingeAnglesV1,
                source_model_fingerprint: crate::fold_model_fingerprint::fold_model_fingerprint_v1(
                    &target_pattern,
                    &paper,
                ),
                fixed_face: Some(FaceId::new()),
                hinge_angles: vec![InstructionHingeAngle {
                    edge: hinge,
                    angle_degrees: 90.0,
                }],
            },
        }],
    };
    SpeculativeFixture {
        editor: EditorState::with_paper(source_pattern, paper.clone()),
        target_pattern,
        paper,
        timeline,
        applied_pose: runtime_pose(90.0),
        project_id: ProjectId::new(),
        project_instance_id: ProjectId::new(),
    }
}

pub(super) fn binding(
    fixture: &SpeculativeFixture,
    observation: SpeculativeApproximateBlockingObservationV1,
) -> SpeculativeUnprovenFoldBindingV1 {
    SpeculativeUnprovenFoldBindingV1::new(
        fixture.project_instance_id,
        fixture.project_id,
        fixture.editor.revision(),
        fixture.editor.fold_model_fingerprint_v1(),
        7,
        ProjectId::new(),
        fixture.editor.paper().thickness_mm,
        observation,
    )
    .expect("valid speculative binding")
}

pub(super) fn apply_marked(
    fixture: &mut SpeculativeFixture,
    binding: SpeculativeUnprovenFoldBindingV1,
) {
    fixture
        .editor
        .execute_stacked_fold_document_with_unproven_mark_v1(
            fixture.editor.revision(),
            fixture.target_pattern.clone(),
            fixture.paper.clone(),
            fixture.timeline.clone(),
            ProjectLayerDocumentV1::default(),
            fixture.applied_pose.clone(),
            binding,
        )
        .expect("atomic speculative Apply");
}

pub(super) fn reopen(
    editor: &EditorState,
    history: EditorHistoryV1,
) -> Result<EditorState, EditorHistoryErrorV1> {
    EditorState::with_all_document_parts_memo_and_history_v1(
        editor.pattern().clone(),
        editor.paper().clone(),
        editor.instruction_timeline().clone(),
        editor.geometric_constraints().clone(),
        editor.project_layers().clone(),
        ElementMetadataDocumentV1::default(),
        editor.project_memo().to_owned(),
        history,
    )
}
