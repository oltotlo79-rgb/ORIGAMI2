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

pub(super) fn token_for_target(
    fixture: &SpeculativeFixture,
    target_revision: Revision,
    target_pattern: &CreasePattern,
    target_paper: &Paper,
    target_pose: &AppliedPoseV1,
) -> crate::SpeculativeUnprovenFoldTokenV1 {
    token_for_binding_target(
        fixture,
        binding(
            fixture,
            SpeculativeApproximateBlockingObservationV1::no_blocking_sample_observed(),
        ),
        target_revision,
        target_pattern,
        target_paper,
        target_pose,
    )
}

pub(super) fn token_for_binding_target(
    fixture: &SpeculativeFixture,
    binding: SpeculativeUnprovenFoldBindingV1,
    target_revision: Revision,
    target_pattern: &CreasePattern,
    target_paper: &Paper,
    target_pose: &AppliedPoseV1,
) -> crate::SpeculativeUnprovenFoldTokenV1 {
    crate::stacked_fold::issue_speculative_unproven_fold_token_for_test_v1(
        binding,
        fixture.editor.runtime_instance_anchor.clone(),
        fixture.editor.current_applied_pose(),
        target_revision,
        target_pattern,
        target_paper,
        &fixture.timeline,
        &ProjectLayerDocumentV1::default(),
        fixture.editor.beginner_design_profile(),
        target_pose,
    )
    .expect("valid target-bound speculative token")
}

pub(super) fn apply_marked(
    fixture: &mut SpeculativeFixture,
    binding: SpeculativeUnprovenFoldBindingV1,
) {
    let expected_revision = fixture.editor.revision();
    let target_revision = expected_revision
        .checked_add(1)
        .expect("speculative test target revision");
    let token = token_for_binding_target(
        fixture,
        binding,
        target_revision,
        &fixture.target_pattern,
        &fixture.paper,
        &fixture.applied_pose,
    );
    fixture
        .editor
        .execute_stacked_fold_document_with_unproven_mark_v1(token)
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
