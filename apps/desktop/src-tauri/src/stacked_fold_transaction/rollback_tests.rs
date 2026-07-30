use super::*;

#[test]
fn target_pose_reissue_failure_restores_the_complete_editor() {
    let mut project = super::super::initial_project_state();
    let mut project_before = StackedFoldProjectRollbackSnapshotV1::capture(&project);
    let document_before = project.document();
    project.editor = ori_core::EditorState::with_paper(
        ori_domain::CreasePattern::empty(),
        ori_domain::Paper::default(),
    );
    let invalid_pose = InstructionPose {
        model: InstructionPoseModel::DeclarativeOnlyV1,
        source_model_fingerprint: String::new(),
        fixed_face: None,
        hinge_angles: Vec::new(),
    };
    assert!(
        reissue_target_pose_or_rollback(&mut project, &invalid_pose, &mut project_before).is_err()
    );
    assert_eq!(project.document(), document_before);
}

#[test]
fn consumed_pose_image_still_restores_project_before_reporting_rollback_failure() {
    let mut project = super::super::initial_project_state();
    let mut project_before = StackedFoldProjectRollbackSnapshotV1::capture(&project);
    let document_before = project.document();
    project.editor = ori_core::EditorState::with_paper(
        ori_domain::CreasePattern::empty(),
        ori_domain::Paper::default(),
    );
    let mut consumed = crate::applied_pose::consumed_transaction_rollback_for_test_v1(&project)
        .expect("consume only the test rollback image");

    assert!(
        rollback_stacked_fold_apply_v1(
            &mut project,
            &mut project_before,
            &mut consumed,
            None,
            None,
        )
        .is_err()
    );
    assert_eq!(project.document(), document_before);
}

#[test]
fn project_rollback_image_moves_the_exact_origin_once() {
    let mut project = super::super::initial_project_state();
    project.saved_revision = Some(project.editor.revision());
    project.saved_document = Some(project.document());
    project.saved_speculative_unproven_state =
        Some(project.editor.speculative_unproven_fold_state_marker_v1());
    let document_before = project.document();
    let numeric_before = project.numeric_expressions.clone();
    let layer_before = project.current_layer_evidence.clone();
    let saved_revision_before = project.saved_revision;
    let saved_document_before = project.saved_document.clone();
    let saved_marker_before = project.saved_speculative_unproven_state.clone();
    let mut rollback = StackedFoldProjectRollbackSnapshotV1::capture(&project);

    project.editor = ori_core::EditorState::with_paper(
        ori_domain::CreasePattern::empty(),
        ori_domain::Paper::default(),
    );
    project.current_layer_evidence = None;
    project.numeric_expressions = Default::default();
    project.saved_revision = None;
    project.saved_document = None;
    project.saved_speculative_unproven_state = None;

    rollback
        .restore(&mut project)
        .expect("the origin image restores once by move");
    assert_eq!(project.document(), document_before);
    assert_eq!(project.numeric_expressions, numeric_before);
    assert_eq!(project.current_layer_evidence, layer_before);
    assert_eq!(project.saved_revision, saved_revision_before);
    assert_eq!(project.saved_document, saved_document_before);
    assert_eq!(
        project.saved_speculative_unproven_state,
        saved_marker_before
    );
    assert_eq!(
        rollback.restore(&mut project),
        Err(()),
        "the project rollback image is one-shot"
    );
}
