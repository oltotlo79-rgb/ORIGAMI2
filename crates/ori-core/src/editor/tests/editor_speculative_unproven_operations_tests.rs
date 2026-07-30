use ori_domain::ProjectId;

use super::{speculative_unproven_test_support::*, *};

struct SpeculativeAllocationFailpointReset;

impl Drop for SpeculativeAllocationFailpointReset {
    fn drop(&mut self) {
        set_speculative_stacked_fold_allocation_failpoint_v1(None);
    }
}

fn assert_speculative_apply_failpoint_is_atomic(
    fixture: &mut SpeculativeFixture,
    resource: SpeculativeUnprovenFoldApplyResourceV1,
) {
    let target_revision = fixture
        .editor
        .revision()
        .checked_add(1)
        .expect("target revision");
    let token = token_for_target(
        fixture,
        target_revision,
        &fixture.target_pattern,
        &fixture.paper,
        &fixture.applied_pose,
    );
    let before = format!("{:?}", fixture.editor);
    let marker_before = fixture.editor.speculative_unproven_fold_state_marker_v1();
    set_speculative_stacked_fold_allocation_failpoint_v1(Some(resource));
    let _reset = SpeculativeAllocationFailpointReset;
    assert!(matches!(
        fixture
            .editor
            .execute_stacked_fold_document_with_unproven_mark_and_resolution_ticket_v1(token),
        Err(
            SpeculativeUnprovenFoldApplyErrorV1::CommitPreparationResourceLimit {
                resource: actual,
            }
        ) if actual == resource
    ));
    set_speculative_stacked_fold_allocation_failpoint_v1(None);
    assert_eq!(format!("{:?}", fixture.editor), before);
    assert_eq!(
        fixture.editor.speculative_unproven_fold_state_marker_v1(),
        marker_before
    );

    let retry_token = token_for_target(
        fixture,
        target_revision,
        &fixture.target_pattern,
        &fixture.paper,
        &fixture.applied_pose,
    );
    fixture
        .editor
        .execute_stacked_fold_document_with_unproven_mark_and_resolution_ticket_v1(retry_token)
        .expect("the unchanged editor remains retryable");
}

#[derive(Debug)]
struct LayeredResolverEditorStateV1 {
    revision: Revision,
    pattern: CreasePattern,
    paper: Paper,
    timeline: InstructionTimeline,
    applied_pose: Option<AppliedPoseV1>,
    undo_history: String,
    redo_history: String,
    undo_history_without_marks: String,
    redo_history_without_marks: String,
    marks: SpeculativeUnprovenFoldStateMarkerV1,
}

fn layered_resolver_editor_state_v1(editor: &EditorState) -> LayeredResolverEditorStateV1 {
    let mut undo_without_marks = editor.undo_stack.clone();
    for entry in &mut undo_without_marks {
        entry.speculative_unproven_fold = None;
    }
    let mut redo_without_marks = editor.redo_stack.clone();
    for entry in &mut redo_without_marks {
        entry.speculative_unproven_fold = None;
    }
    LayeredResolverEditorStateV1 {
        revision: editor.revision(),
        pattern: editor.pattern().clone(),
        paper: editor.paper().clone(),
        timeline: editor.instruction_timeline().clone(),
        applied_pose: editor.current_applied_pose().cloned(),
        undo_history: format!("{:?}", editor.undo_stack),
        redo_history: format!("{:?}", editor.redo_stack),
        undo_history_without_marks: format!("{undo_without_marks:?}"),
        redo_history_without_marks: format!("{redo_without_marks:?}"),
        marks: editor.speculative_unproven_fold_state_marker_v1(),
    }
}

fn assert_layered_resolver_failure_is_bit_identical_v1(
    editor: &EditorState,
    before: &LayeredResolverEditorStateV1,
) {
    assert_eq!(editor.revision(), before.revision);
    assert_eq!(editor.pattern(), &before.pattern);
    assert_eq!(editor.paper(), &before.paper);
    assert_eq!(editor.instruction_timeline(), &before.timeline);
    assert_eq!(editor.current_applied_pose(), before.applied_pose.as_ref());
    assert_eq!(format!("{:?}", editor.undo_stack), before.undo_history);
    assert_eq!(format!("{:?}", editor.redo_stack), before.redo_history);
    assert_eq!(
        editor.speculative_unproven_fold_state_marker_v1(),
        before.marks
    );
}

fn assert_layered_resolver_success_preserves_document_v1(
    editor: &EditorState,
    before: &LayeredResolverEditorStateV1,
) {
    assert_eq!(editor.revision(), before.revision);
    assert_eq!(editor.pattern(), &before.pattern);
    assert_eq!(editor.paper(), &before.paper);
    assert_eq!(editor.instruction_timeline(), &before.timeline);
    assert_eq!(editor.current_applied_pose(), before.applied_pose.as_ref());

    let mut undo_without_marks = editor.undo_stack.clone();
    for entry in &mut undo_without_marks {
        entry.speculative_unproven_fold = None;
    }
    let mut redo_without_marks = editor.redo_stack.clone();
    for entry in &mut redo_without_marks {
        entry.speculative_unproven_fold = None;
    }
    assert_eq!(
        format!("{undo_without_marks:?}"),
        before.undo_history_without_marks
    );
    assert_eq!(
        format!("{redo_without_marks:?}"),
        before.redo_history_without_marks
    );
}

#[test]
fn unproven_mark_survives_undo_redo_and_restores_the_exact_marker() {
    let mut fixture = fixture();
    let binding = binding(
        &fixture,
        SpeculativeApproximateBlockingObservationV1::no_blocking_sample_observed(),
    );
    apply_marked(&mut fixture, binding);
    let applied_marker = fixture.editor.speculative_unproven_fold_state_marker_v1();
    assert_eq!(
        fixture.editor.speculative_unproven_fold_summary_v1(),
        SpeculativeUnprovenFoldSummaryV1 {
            applied: SpeculativeUnprovenFoldStatusCountsV1 {
                awaiting_proof: 1,
                ..SpeculativeUnprovenFoldStatusCountsV1::default()
            },
            ..SpeculativeUnprovenFoldSummaryV1::default()
        }
    );

    fixture.editor.undo(1).expect("undo speculative entry");
    let undone = fixture.editor.speculative_unproven_fold_summary_v1();
    assert_eq!(undone.applied.total(), 0);
    assert_eq!(undone.unapplied_redo.awaiting_proof, 1);

    fixture.editor.redo(2).expect("redo speculative entry");
    assert_eq!(
        fixture.editor.speculative_unproven_fold_state_marker_v1(),
        applied_marker
    );
    assert_eq!(
        fixture
            .editor
            .speculative_unproven_fold_summary_v1()
            .applied
            .awaiting_proof,
        1
    );
}

#[test]
fn proof_failure_updates_only_the_mark_and_reports_subsequent_edits() {
    let mut fixture = fixture();
    let binding = binding(
        &fixture,
        SpeculativeApproximateBlockingObservationV1::no_blocking_sample_observed(),
    );
    apply_marked(&mut fixture, binding.clone());
    for memo in ["later edit 1", "later edit 2"] {
        fixture
            .editor
            .execute(
                fixture.editor.revision(),
                Command::UpdateProjectMemo {
                    memo: memo.to_owned(),
                },
            )
            .expect("later edit");
    }
    let revision = fixture.editor.revision();
    let pattern = fixture.editor.pattern().clone();
    let report = fixture
        .editor
        .resolve_speculative_unproven_fold_v1(
            &binding,
            SpeculativeUnprovenFoldProofOutcomeV1::Blocked,
        )
        .expect("record blocked proof");

    assert_eq!(fixture.editor.revision(), revision);
    assert_eq!(fixture.editor.pattern(), &pattern);
    assert_eq!(
        report,
        SpeculativeUnprovenFoldResolutionReportV1 {
            location: SpeculativeUnprovenFoldHistoryLocationV1::AppliedRetainedUndo,
            outcome: SpeculativeUnprovenFoldProofOutcomeV1::Blocked,
            subsequent_edit_count: 2,
            undo_steps_to_revert: Some(3),
        }
    );
    assert_eq!(
        fixture
            .editor
            .speculative_unproven_fold_summary_v1()
            .applied
            .proof_blocked,
        1
    );
    assert_eq!(
        fixture.editor.resolve_speculative_unproven_fold_v1(
            &binding,
            SpeculativeUnprovenFoldProofOutcomeV1::Blocked,
        ),
        Err(SpeculativeUnprovenFoldResolutionErrorV1::AlreadyResolved)
    );
}

#[test]
fn inspection_is_read_only_and_reports_terminal_status_at_the_live_location() {
    let mut fixture = fixture();
    let binding = binding(
        &fixture,
        SpeculativeApproximateBlockingObservationV1::no_blocking_sample_observed(),
    );
    apply_marked(&mut fixture, binding.clone());
    let revision = fixture.editor.revision();
    let marker = fixture.editor.speculative_unproven_fold_state_marker_v1();

    assert_eq!(
        fixture
            .editor
            .inspect_speculative_unproven_fold_v1(&binding),
        Ok(None)
    );
    assert_eq!(fixture.editor.revision(), revision);
    assert_eq!(
        fixture.editor.speculative_unproven_fold_state_marker_v1(),
        marker
    );

    let outcome = SpeculativeUnprovenFoldProofOutcomeV1::Unknown {
        reason: SpeculativeUnprovenFoldUnknownReasonV1::ResourceLimit,
    };
    let resolved = fixture
        .editor
        .resolve_speculative_unproven_fold_v1(&binding, outcome)
        .expect("resolve the unique awaiting mark");
    assert_eq!(
        fixture
            .editor
            .inspect_speculative_unproven_fold_v1(&binding),
        Ok(Some(resolved))
    );

    fixture
        .editor
        .undo(fixture.editor.revision())
        .expect("move the resolved mark to Redo");
    let inspected = fixture
        .editor
        .inspect_speculative_unproven_fold_v1(&binding)
        .expect("the resolved mark remains inspectable")
        .expect("the mark remains terminal");
    assert_eq!(
        inspected,
        SpeculativeUnprovenFoldResolutionReportV1 {
            location: SpeculativeUnprovenFoldHistoryLocationV1::UnappliedRedo,
            outcome,
            subsequent_edit_count: 0,
            undo_steps_to_revert: None,
        }
    );
}

#[test]
fn generic_certified_resolution_is_rejected_atomically_in_every_history_location() {
    fn assert_binding_location(
        fixture: &SpeculativeFixture,
        binding: &SpeculativeUnprovenFoldBindingV1,
        expected: SpeculativeUnprovenFoldHistoryLocationV1,
    ) {
        let mut lookup_probe = fixture.editor.clone();
        let report = lookup_probe
            .resolve_speculative_unproven_fold_v1(
                binding,
                SpeculativeUnprovenFoldProofOutcomeV1::Blocked,
            )
            .expect("the awaiting binding must exist at the expected history location");
        assert_eq!(report.location, expected);
    }

    fn assert_rejected_without_mutation(
        fixture: &mut SpeculativeFixture,
        binding: &SpeculativeUnprovenFoldBindingV1,
    ) {
        let before_debug = format!("{:?}", fixture.editor);
        let before_marker = fixture.editor.speculative_unproven_fold_state_marker_v1();
        let before_summary = fixture.editor.speculative_unproven_fold_summary_v1();
        let before_history = serde_json::to_value(
            fixture
                .editor
                .export_history_v1(fixture.project_id)
                .expect("export history before rejected certification"),
        )
        .expect("history JSON before rejected certification");

        assert_eq!(
            fixture.editor.resolve_speculative_unproven_fold_v1(
                binding,
                SpeculativeUnprovenFoldProofOutcomeV1::Certified,
            ),
            Err(SpeculativeUnprovenFoldResolutionErrorV1::CertifiedRequiresTypedProof)
        );
        assert_eq!(format!("{:?}", fixture.editor), before_debug);
        assert_eq!(
            fixture.editor.speculative_unproven_fold_state_marker_v1(),
            before_marker
        );
        assert_eq!(
            fixture.editor.speculative_unproven_fold_summary_v1(),
            before_summary
        );
        assert!(
            fixture
                .editor
                .requires_speculative_unproven_fold_feature_v1()
        );
        assert_eq!(
            serde_json::to_value(
                fixture
                    .editor
                    .export_history_v1(fixture.project_id)
                    .expect("export history after rejected certification"),
            )
            .expect("history JSON after rejected certification"),
            before_history
        );
    }

    let mut applied = fixture();
    let applied_binding = binding(
        &applied,
        SpeculativeApproximateBlockingObservationV1::no_blocking_sample_observed(),
    );
    apply_marked(&mut applied, applied_binding.clone());
    assert_binding_location(
        &applied,
        &applied_binding,
        SpeculativeUnprovenFoldHistoryLocationV1::AppliedRetainedUndo,
    );
    assert_rejected_without_mutation(&mut applied, &applied_binding);

    let unregistered_binding = binding(
        &applied,
        SpeculativeApproximateBlockingObservationV1::no_blocking_sample_observed(),
    );
    assert_ne!(
        unregistered_binding.request_generation_id(),
        applied_binding.request_generation_id(),
    );
    let mut lookup_probe = applied.editor.clone();
    assert_eq!(
        lookup_probe.resolve_speculative_unproven_fold_v1(
            &unregistered_binding,
            SpeculativeUnprovenFoldProofOutcomeV1::Blocked,
        ),
        Err(SpeculativeUnprovenFoldResolutionErrorV1::BindingNotFound),
    );
    assert_rejected_without_mutation(&mut applied, &unregistered_binding);

    let mut redo = fixture();
    let redo_binding = binding(
        &redo,
        SpeculativeApproximateBlockingObservationV1::no_blocking_sample_observed(),
    );
    apply_marked(&mut redo, redo_binding.clone());
    redo.editor.undo(1).expect("move marked entry to Redo");
    assert_binding_location(
        &redo,
        &redo_binding,
        SpeculativeUnprovenFoldHistoryLocationV1::UnappliedRedo,
    );
    assert_rejected_without_mutation(&mut redo, &redo_binding);

    let mut trimmed = fixture();
    trimmed
        .editor
        .set_history_entry_limit(1)
        .expect("one-entry history");
    let trimmed_binding = binding(
        &trimmed,
        SpeculativeApproximateBlockingObservationV1::no_blocking_sample_observed(),
    );
    apply_marked(&mut trimmed, trimmed_binding.clone());
    trimmed
        .editor
        .execute(
            trimmed.editor.revision(),
            Command::UpdateProjectMemo {
                memo: "trim marked entry into the applied base".to_owned(),
            },
        )
        .expect("trim marked entry");
    assert_binding_location(
        &trimmed,
        &trimmed_binding,
        SpeculativeUnprovenFoldHistoryLocationV1::AppliedTrimmedBase,
    );
    assert_rejected_without_mutation(&mut trimmed, &trimmed_binding);
}

#[test]
fn redo_only_mark_is_discarded_by_a_new_branch() {
    let mut fixture = fixture();
    let binding = binding(
        &fixture,
        SpeculativeApproximateBlockingObservationV1::no_blocking_sample_observed(),
    );
    apply_marked(&mut fixture, binding);
    fixture.editor.undo(1).expect("undo speculative entry");
    assert_eq!(
        fixture
            .editor
            .speculative_unproven_fold_summary_v1()
            .unapplied_redo
            .awaiting_proof,
        1
    );

    fixture
        .editor
        .execute(
            fixture.editor.revision(),
            Command::UpdateProjectMemo {
                memo: "new branch".to_owned(),
            },
        )
        .expect("replace Redo branch");
    assert_eq!(
        fixture
            .editor
            .speculative_unproven_fold_summary_v1()
            .unapplied_redo
            .total(),
        0
    );
    assert!(
        !fixture
            .editor
            .requires_speculative_unproven_fold_feature_v1()
    );
}

#[test]
fn stable_request_identity_rejects_metadata_drift_at_runtime() {
    let mut fixture = fixture();
    let original = binding(
        &fixture,
        SpeculativeApproximateBlockingObservationV1::no_blocking_sample_observed(),
    );
    apply_marked(&mut fixture, original.clone());
    let drifted = SpeculativeUnprovenFoldBindingV1::new(
        original.project_instance_id(),
        original.project_id(),
        fixture.editor.revision(),
        fixture.editor.fold_model_fingerprint_v1(),
        original.pose_generation() + 1,
        original.request_generation_id(),
        fixture.editor.paper().thickness_mm,
        SpeculativeApproximateBlockingObservationV1::no_blocking_sample_observed(),
    )
    .expect("well-formed drifted binding");

    assert_eq!(
        fixture.editor.resolve_speculative_unproven_fold_v1(
            &drifted,
            SpeculativeUnprovenFoldProofOutcomeV1::Blocked,
        ),
        Err(SpeculativeUnprovenFoldResolutionErrorV1::BindingMetadataMismatch)
    );
    let expected_revision = fixture.editor.revision();
    let token = token_for_binding_target(
        &fixture,
        drifted,
        expected_revision
            .checked_add(1)
            .expect("speculative duplicate target revision"),
        &fixture.target_pattern,
        &fixture.paper,
        &fixture.applied_pose,
    );
    assert_eq!(
        fixture
            .editor
            .execute_stacked_fold_document_with_unproven_mark_v1(token),
        Err(SpeculativeUnprovenFoldApplyErrorV1::DuplicateBinding)
    );
}

#[test]
fn target_command_is_owned_and_bound_to_the_exact_editor_instance() {
    let original = fixture();
    let token = token_for_target(
        &original,
        1,
        &original.target_pattern,
        &original.paper,
        &original.applied_pose,
    );
    let mut separately_created = EditorState::with_paper(
        original.editor.pattern().clone(),
        original.editor.paper().clone(),
    );
    let before = format!("{separately_created:?}");
    assert_eq!(
        separately_created.execute_stacked_fold_document_with_unproven_mark_v1(token),
        Err(SpeculativeUnprovenFoldApplyErrorV1::TargetSealMismatch)
    );
    assert_eq!(format!("{separately_created:?}"), before);

    let original = fixture();
    let token = token_for_target(
        &original,
        1,
        &original.target_pattern,
        &original.paper,
        &original.applied_pose,
    );
    let mut transactional_clone = original.editor.clone();
    transactional_clone
        .execute_stacked_fold_document_with_unproven_mark_v1(token)
        .expect("a rollback clone shares the exact live-editor anchor");
    assert_eq!(transactional_clone.pattern(), &original.target_pattern);
    assert_eq!(
        transactional_clone.instruction_timeline(),
        &original.timeline
    );
    assert_eq!(
        transactional_clone.current_applied_pose(),
        Some(&original.applied_pose)
    );
    assert_eq!(
        transactional_clone
            .current_applied_pose()
            .expect("sealed pose")
            .face_ids(),
        original.applied_pose.face_ids()
    );
    assert_eq!(original.editor.revision(), 0);

    let mut fixture = fixture();
    let token = token_for_target(
        &fixture,
        1,
        &fixture.target_pattern,
        &fixture.paper,
        &fixture.applied_pose,
    );
    fixture
        .editor
        .adopt_current_applied_pose(fixture.applied_pose.clone());
    assert_eq!(
        fixture
            .editor
            .execute_stacked_fold_document_with_unproven_mark_v1(token),
        Err(SpeculativeUnprovenFoldApplyErrorV1::TargetSealMismatch)
    );
    assert_eq!(fixture.editor.revision(), 0);
    assert!(!fixture.editor.can_undo());
}

#[test]
fn stale_or_blocking_bindings_reject_atomically_before_apply() {
    fn assert_rejected(
        mut fixture: SpeculativeFixture,
        binding: SpeculativeUnprovenFoldBindingV1,
        expected: SpeculativeUnprovenFoldApplyErrorV1,
    ) {
        let before = (
            fixture.editor.pattern().clone(),
            fixture.editor.revision(),
            fixture.editor.speculative_unproven_fold_state_marker_v1(),
        );
        let token = token_for_binding_target(
            &fixture,
            binding,
            1,
            &fixture.target_pattern,
            &fixture.paper,
            &fixture.applied_pose,
        );
        let result = fixture
            .editor
            .execute_stacked_fold_document_with_unproven_mark_v1(token);
        assert_eq!(result, Err(expected));
        assert_eq!(
            (
                fixture.editor.pattern().clone(),
                fixture.editor.revision(),
                fixture.editor.speculative_unproven_fold_state_marker_v1(),
            ),
            before
        );
    }

    let revision_fixture = fixture();
    let revision_binding = SpeculativeUnprovenFoldBindingV1::new(
        revision_fixture.project_instance_id,
        revision_fixture.project_id,
        1,
        revision_fixture.editor.fold_model_fingerprint_v1(),
        1,
        ProjectId::new(),
        revision_fixture.editor.paper().thickness_mm,
        SpeculativeApproximateBlockingObservationV1::no_blocking_sample_observed(),
    )
    .expect("well-formed stale revision");
    assert_rejected(
        revision_fixture,
        revision_binding,
        SpeculativeUnprovenFoldApplyErrorV1::SourceRevisionMismatch,
    );

    let fingerprint_fixture = fixture();
    let fingerprint_binding = SpeculativeUnprovenFoldBindingV1::new(
        fingerprint_fixture.project_instance_id,
        fingerprint_fixture.project_id,
        0,
        "0".repeat(64),
        1,
        ProjectId::new(),
        fingerprint_fixture.editor.paper().thickness_mm,
        SpeculativeApproximateBlockingObservationV1::no_blocking_sample_observed(),
    )
    .expect("well-formed stale fingerprint");
    assert_rejected(
        fingerprint_fixture,
        fingerprint_binding,
        SpeculativeUnprovenFoldApplyErrorV1::SourceGeometryFingerprintMismatch,
    );

    let thickness_fixture = fixture();
    let thickness_binding = SpeculativeUnprovenFoldBindingV1::new(
        thickness_fixture.project_instance_id,
        thickness_fixture.project_id,
        0,
        thickness_fixture.editor.fold_model_fingerprint_v1(),
        1,
        ProjectId::new(),
        f64::from_bits(thickness_fixture.editor.paper().thickness_mm.to_bits() + 1),
        SpeculativeApproximateBlockingObservationV1::no_blocking_sample_observed(),
    )
    .expect("well-formed stale thickness");
    assert_rejected(
        thickness_fixture,
        thickness_binding,
        SpeculativeUnprovenFoldApplyErrorV1::PaperThicknessBitsMismatch,
    );

    let mut fixture = fixture();
    let blocking = binding(
        &fixture,
        SpeculativeApproximateBlockingObservationV1::blocking_sample_observed(45.0)
            .expect("valid observed angle"),
    );
    let token = token_for_binding_target(
        &fixture,
        blocking,
        1,
        &fixture.target_pattern,
        &fixture.paper,
        &fixture.applied_pose,
    );
    assert_eq!(
        fixture
            .editor
            .execute_stacked_fold_document_with_unproven_mark_v1(token),
        Err(SpeculativeUnprovenFoldApplyErrorV1::ApproximateBlockingSampleObserved)
    );
    assert_eq!(fixture.editor.revision(), 0);
    assert!(!fixture.editor.can_undo());
}

#[test]
fn typed_certification_removes_only_the_mark_in_every_history_location() {
    fn assert_document_state_unchanged(
        fixture: &SpeculativeFixture,
        revision: Revision,
        pattern: &CreasePattern,
        timeline: &InstructionTimeline,
        pose: &AppliedPoseV1,
        undo_len: usize,
        redo_len: usize,
    ) {
        assert_eq!(fixture.editor.revision(), revision);
        assert_eq!(fixture.editor.pattern(), pattern);
        assert_eq!(fixture.editor.instruction_timeline(), timeline);
        assert_eq!(fixture.editor.current_applied_pose(), Some(pose));
        assert_eq!(fixture.editor.undo_stack.len(), undo_len);
        assert_eq!(fixture.editor.redo_stack.len(), redo_len);
    }

    let mut retained = fixture();
    let retained_binding = binding(
        &retained,
        SpeculativeApproximateBlockingObservationV1::no_blocking_sample_observed(),
    );
    let retained_ticket = apply_marked_with_ticket(&mut retained, retained_binding.clone());
    let retained_before = (
        retained.editor.revision(),
        retained.editor.pattern().clone(),
        retained.editor.instruction_timeline().clone(),
        retained
            .editor
            .current_applied_pose()
            .expect("applied target pose")
            .clone(),
        retained.editor.undo_stack.len(),
        retained.editor.redo_stack.len(),
    );
    assert_eq!(
        retained
            .editor
            .resolve_speculative_unproven_fold_certified_v1(proof_for_ticket(retained_ticket))
            .expect("resolve retained Undo mark"),
        SpeculativeUnprovenFoldResolutionReportV1 {
            location: SpeculativeUnprovenFoldHistoryLocationV1::AppliedRetainedUndo,
            outcome: SpeculativeUnprovenFoldProofOutcomeV1::Certified,
            subsequent_edit_count: 0,
            undo_steps_to_revert: Some(1),
        }
    );
    assert_document_state_unchanged(
        &retained,
        retained_before.0,
        &retained_before.1,
        &retained_before.2,
        &retained_before.3,
        retained_before.4,
        retained_before.5,
    );
    assert_eq!(
        retained.editor.speculative_unproven_fold_summary_v1(),
        SpeculativeUnprovenFoldSummaryV1::default()
    );
    assert_eq!(
        retained
            .editor
            .inspect_speculative_unproven_fold_v1(&retained_binding),
        Err(SpeculativeUnprovenFoldResolutionErrorV1::BindingNotFound)
    );

    let mut redo = fixture();
    let redo_binding = binding(
        &redo,
        SpeculativeApproximateBlockingObservationV1::no_blocking_sample_observed(),
    );
    let redo_ticket = apply_marked_with_ticket(&mut redo, redo_binding);
    redo.editor
        .undo(redo.editor.revision())
        .expect("move to Redo");
    let redo_revision = redo.editor.revision();
    let redo_pattern = redo.editor.pattern().clone();
    let redo_timeline = redo.editor.instruction_timeline().clone();
    let redo_undo_len = redo.editor.undo_stack.len();
    let redo_redo_len = redo.editor.redo_stack.len();
    assert_eq!(
        redo.editor
            .resolve_speculative_unproven_fold_certified_v1(proof_for_ticket(redo_ticket))
            .expect("resolve Redo mark"),
        SpeculativeUnprovenFoldResolutionReportV1 {
            location: SpeculativeUnprovenFoldHistoryLocationV1::UnappliedRedo,
            outcome: SpeculativeUnprovenFoldProofOutcomeV1::Certified,
            subsequent_edit_count: 0,
            undo_steps_to_revert: None,
        }
    );
    assert_eq!(redo.editor.revision(), redo_revision);
    assert_eq!(redo.editor.pattern(), &redo_pattern);
    assert_eq!(redo.editor.instruction_timeline(), &redo_timeline);
    assert_eq!(redo.editor.undo_stack.len(), redo_undo_len);
    assert_eq!(redo.editor.redo_stack.len(), redo_redo_len);
    assert_eq!(
        redo.editor.speculative_unproven_fold_summary_v1(),
        SpeculativeUnprovenFoldSummaryV1::default()
    );

    let mut trimmed = fixture();
    trimmed
        .editor
        .set_history_entry_limit(1)
        .expect("one-entry history");
    let trimmed_binding = binding(
        &trimmed,
        SpeculativeApproximateBlockingObservationV1::no_blocking_sample_observed(),
    );
    let trimmed_ticket = apply_marked_with_ticket(&mut trimmed, trimmed_binding);
    trimmed
        .editor
        .execute(
            trimmed.editor.revision(),
            Command::UpdateProjectMemo {
                memo: "trim the speculative entry".to_owned(),
            },
        )
        .expect("trim marked entry");
    let trimmed_revision = trimmed.editor.revision();
    let trimmed_pattern = trimmed.editor.pattern().clone();
    let trimmed_undo_len = trimmed.editor.undo_stack.len();
    assert_eq!(
        trimmed
            .editor
            .resolve_speculative_unproven_fold_certified_v1(proof_for_ticket(trimmed_ticket))
            .expect("resolve applied-base mark"),
        SpeculativeUnprovenFoldResolutionReportV1 {
            location: SpeculativeUnprovenFoldHistoryLocationV1::AppliedTrimmedBase,
            outcome: SpeculativeUnprovenFoldProofOutcomeV1::Certified,
            subsequent_edit_count: 1,
            undo_steps_to_revert: None,
        }
    );
    assert_eq!(trimmed.editor.revision(), trimmed_revision);
    assert_eq!(trimmed.editor.pattern(), &trimmed_pattern);
    assert_eq!(trimmed.editor.undo_stack.len(), trimmed_undo_len);
    assert_eq!(
        trimmed.editor.speculative_unproven_fold_summary_v1(),
        SpeculativeUnprovenFoldSummaryV1::default()
    );
}

#[test]
fn typed_certification_failures_leave_every_mark_unchanged() {
    let mut foreign_source = fixture();
    let foreign_binding = binding(
        &foreign_source,
        SpeculativeApproximateBlockingObservationV1::no_blocking_sample_observed(),
    );
    let foreign_ticket = apply_marked_with_ticket(&mut foreign_source, foreign_binding.clone());
    let mut foreign_editor = EditorState::with_paper(
        foreign_source.editor.pattern().clone(),
        foreign_source.editor.paper().clone(),
    );
    let foreign_before = format!("{foreign_editor:?}");
    assert_eq!(
        foreign_editor
            .resolve_speculative_unproven_fold_certified_v1(proof_for_ticket(foreign_ticket)),
        Err(SpeculativeUnprovenFoldResolutionErrorV1::ForeignEditor)
    );
    assert_eq!(format!("{foreign_editor:?}"), foreign_before);
    assert_eq!(
        foreign_source
            .editor
            .speculative_unproven_fold_summary_v1()
            .applied
            .awaiting_proof,
        1
    );

    let mut missing = fixture();
    let mut same_anchor_without_mark = missing.editor.clone();
    let missing_binding = binding(
        &missing,
        SpeculativeApproximateBlockingObservationV1::no_blocking_sample_observed(),
    );
    let missing_ticket = apply_marked_with_ticket(&mut missing, missing_binding);
    let missing_before = format!("{same_anchor_without_mark:?}");
    assert_eq!(
        same_anchor_without_mark
            .resolve_speculative_unproven_fold_certified_v1(proof_for_ticket(missing_ticket)),
        Err(SpeculativeUnprovenFoldResolutionErrorV1::BindingNotFound)
    );
    assert_eq!(format!("{same_anchor_without_mark:?}"), missing_before);

    let mut terminal = fixture();
    let terminal_binding = binding(
        &terminal,
        SpeculativeApproximateBlockingObservationV1::no_blocking_sample_observed(),
    );
    let terminal_ticket = apply_marked_with_ticket(&mut terminal, terminal_binding.clone());
    terminal
        .editor
        .resolve_speculative_unproven_fold_v1(
            &terminal_binding,
            SpeculativeUnprovenFoldProofOutcomeV1::Blocked,
        )
        .expect("record terminal result");
    let terminal_marker = terminal.editor.speculative_unproven_fold_state_marker_v1();
    assert_eq!(
        terminal
            .editor
            .resolve_speculative_unproven_fold_certified_v1(proof_for_ticket(terminal_ticket)),
        Err(SpeculativeUnprovenFoldResolutionErrorV1::AlreadyResolved)
    );
    assert_eq!(
        terminal.editor.speculative_unproven_fold_state_marker_v1(),
        terminal_marker
    );

    let mut duplicate = fixture();
    let duplicate_binding = binding(
        &duplicate,
        SpeculativeApproximateBlockingObservationV1::no_blocking_sample_observed(),
    );
    let duplicate_ticket = apply_marked_with_ticket(&mut duplicate, duplicate_binding);
    duplicate
        .editor
        .execute(
            duplicate.editor.revision(),
            Command::UpdateProjectMemo {
                memo: "second history entry".to_owned(),
            },
        )
        .expect("append history entry");
    let duplicated_mark = duplicate.editor.undo_stack[0]
        .speculative_unproven_fold
        .clone();
    duplicate.editor.undo_stack[1].speculative_unproven_fold = duplicated_mark;
    let duplicate_marker = duplicate.editor.speculative_unproven_fold_state_marker_v1();
    assert_eq!(
        duplicate
            .editor
            .resolve_speculative_unproven_fold_certified_v1(proof_for_ticket(duplicate_ticket)),
        Err(SpeculativeUnprovenFoldResolutionErrorV1::DuplicateBinding)
    );
    assert_eq!(
        duplicate.editor.speculative_unproven_fold_state_marker_v1(),
        duplicate_marker
    );

    let mut metadata_drift = fixture();
    let exact_binding = binding(
        &metadata_drift,
        SpeculativeApproximateBlockingObservationV1::no_blocking_sample_observed(),
    );
    let metadata_ticket = apply_marked_with_ticket(&mut metadata_drift, exact_binding.clone());
    let drifted_binding = SpeculativeUnprovenFoldBindingV1::new(
        exact_binding.project_instance_id(),
        exact_binding.project_id(),
        exact_binding.source_revision(),
        exact_binding
            .source_geometry_fingerprint_sha256()
            .to_owned(),
        exact_binding.pose_generation() + 1,
        exact_binding.request_generation_id(),
        f64::from_bits(exact_binding.paper_thickness_bits()),
        exact_binding.approximate_blocking_observation(),
    )
    .expect("well-formed metadata drift");
    metadata_drift.editor.undo_stack[0]
        .speculative_unproven_fold
        .as_mut()
        .expect("awaiting mark")
        .binding = drifted_binding;
    let metadata_marker = metadata_drift
        .editor
        .speculative_unproven_fold_state_marker_v1();
    assert_eq!(
        metadata_drift
            .editor
            .resolve_speculative_unproven_fold_certified_v1(proof_for_ticket(metadata_ticket)),
        Err(SpeculativeUnprovenFoldResolutionErrorV1::BindingMetadataMismatch)
    );
    assert_eq!(
        metadata_drift
            .editor
            .speculative_unproven_fold_state_marker_v1(),
        metadata_marker
    );
}

#[test]
fn recoverable_typed_resolvers_return_the_exact_proof_for_owner_retry() {
    let mut tree = fixture();
    let tree_binding = binding(
        &tree,
        SpeculativeApproximateBlockingObservationV1::no_blocking_sample_observed(),
    );
    let tree_ticket = apply_marked_with_ticket(&mut tree, tree_binding);
    let mut foreign_tree_editor = EditorState::new(CreasePattern::empty());
    let tree_failure = foreign_tree_editor
        .try_resolve_speculative_unproven_fold_certified_v1(proof_for_ticket(tree_ticket))
        .expect_err("a foreign editor must reject the Tree proof");
    assert_eq!(
        tree_failure.error(),
        &SpeculativeUnprovenFoldResolutionErrorV1::ForeignEditor
    );
    tree.editor
        .try_resolve_speculative_unproven_fold_certified_v1(tree_failure.into_proof())
        .expect("the exact returned Tree proof resolves on its owner");
    assert_eq!(
        tree.editor.speculative_unproven_fold_summary_v1(),
        SpeculativeUnprovenFoldSummaryV1::default()
    );

    let mut three_face = fixture();
    let three_face_binding = binding(
        &three_face,
        SpeculativeApproximateBlockingObservationV1::no_blocking_sample_observed(),
    );
    let three_face_ticket = apply_marked_with_ticket(&mut three_face, three_face_binding);
    let mut foreign_three_face_editor = EditorState::new(CreasePattern::empty());
    let three_face_failure = foreign_three_face_editor
        .try_resolve_speculative_unproven_fold_layered_three_face_certified_v1(
            layered_proof_for_ticket(three_face_ticket),
        )
        .expect_err("a foreign editor must reject the three-face proof");
    assert_eq!(
        three_face_failure.error(),
        &SpeculativeUnprovenFoldResolutionErrorV1::ForeignEditor
    );
    three_face
        .editor
        .try_resolve_speculative_unproven_fold_layered_three_face_certified_v1(
            three_face_failure.into_proof(),
        )
        .expect("the exact returned three-face proof resolves on its owner");
    assert_eq!(
        three_face.editor.speculative_unproven_fold_summary_v1(),
        SpeculativeUnprovenFoldSummaryV1::default()
    );

    let mut four_face = fixture();
    let four_face_binding = binding(
        &four_face,
        SpeculativeApproximateBlockingObservationV1::no_blocking_sample_observed(),
    );
    let four_face_ticket = apply_marked_with_ticket(&mut four_face, four_face_binding);
    let mut foreign_four_face_editor = EditorState::new(CreasePattern::empty());
    let four_face_failure = foreign_four_face_editor
        .try_resolve_speculative_unproven_fold_layered_four_face_certified_v1(
            layered_four_face_proof_for_ticket(four_face_ticket),
        )
        .expect_err("a foreign editor must reject the four-face proof");
    assert_eq!(
        four_face_failure.error(),
        &SpeculativeUnprovenFoldResolutionErrorV1::ForeignEditor
    );
    four_face
        .editor
        .try_resolve_speculative_unproven_fold_layered_four_face_certified_v1(
            four_face_failure.into_proof(),
        )
        .expect("the exact returned four-face proof resolves on its owner");
    assert_eq!(
        four_face.editor.speculative_unproven_fold_summary_v1(),
        SpeculativeUnprovenFoldSummaryV1::default()
    );

    let mut missing_mark = fixture();
    let mut same_owner_snapshot_without_mark = missing_mark.editor.clone();
    let missing_binding = binding(
        &missing_mark,
        SpeculativeApproximateBlockingObservationV1::no_blocking_sample_observed(),
    );
    let missing_ticket = apply_marked_with_ticket(&mut missing_mark, missing_binding);
    let missing_failure = same_owner_snapshot_without_mark
        .try_resolve_speculative_unproven_fold_certified_v1(proof_for_ticket(missing_ticket))
        .expect_err("the owner snapshot without the mark must reject the proof");
    assert_eq!(
        missing_failure.error(),
        &SpeculativeUnprovenFoldResolutionErrorV1::BindingNotFound
    );
    missing_mark
        .editor
        .try_resolve_speculative_unproven_fold_certified_v1(missing_failure.into_proof())
        .expect("a mark-location rejection returns the exact proof for retry");
}

#[test]
fn recoverable_typed_resolvers_return_exact_tree_three_and_four_proofs_after_precommit_panic() {
    let mut tree = fixture();
    let tree_binding = binding(
        &tree,
        SpeculativeApproximateBlockingObservationV1::no_blocking_sample_observed(),
    );
    let tree_ticket = apply_marked_with_ticket(&mut tree, tree_binding);
    let tree_before = layered_resolver_editor_state_v1(&tree.editor);
    let tree_failure = with_certified_resolution_precommit_panic_v1(|| {
        tree.editor
            .try_resolve_speculative_unproven_fold_certified_v1(proof_for_ticket(tree_ticket))
            .expect_err("the injected Tree precommit panic must be caught")
    });
    assert_eq!(
        tree_failure.error(),
        &SpeculativeUnprovenFoldResolutionErrorV1::ResolutionPanicked
    );
    assert_layered_resolver_failure_is_bit_identical_v1(&tree.editor, &tree_before);
    tree.editor
        .try_resolve_speculative_unproven_fold_certified_v1(tree_failure.into_proof())
        .expect("the exact Tree proof remains retryable after the caught panic");

    let mut three_face = fixture();
    let three_face_binding = binding(
        &three_face,
        SpeculativeApproximateBlockingObservationV1::no_blocking_sample_observed(),
    );
    let three_face_ticket = apply_marked_with_ticket(&mut three_face, three_face_binding);
    let three_face_before = layered_resolver_editor_state_v1(&three_face.editor);
    let three_face_failure = with_certified_resolution_precommit_panic_v1(|| {
        three_face
            .editor
            .try_resolve_speculative_unproven_fold_layered_three_face_certified_v1(
                layered_proof_for_ticket(three_face_ticket),
            )
            .expect_err("the injected three-face precommit panic must be caught")
    });
    assert_eq!(
        three_face_failure.error(),
        &SpeculativeUnprovenFoldResolutionErrorV1::ResolutionPanicked
    );
    assert_layered_resolver_failure_is_bit_identical_v1(&three_face.editor, &three_face_before);
    three_face
        .editor
        .try_resolve_speculative_unproven_fold_layered_three_face_certified_v1(
            three_face_failure.into_proof(),
        )
        .expect("the exact three-face proof remains retryable after the caught panic");

    let mut four_face = fixture();
    let four_face_binding = binding(
        &four_face,
        SpeculativeApproximateBlockingObservationV1::no_blocking_sample_observed(),
    );
    let four_face_ticket = apply_marked_with_ticket(&mut four_face, four_face_binding);
    let four_face_before = layered_resolver_editor_state_v1(&four_face.editor);
    let four_face_failure = with_certified_resolution_precommit_panic_v1(|| {
        four_face
            .editor
            .try_resolve_speculative_unproven_fold_layered_four_face_certified_v1(
                layered_four_face_proof_for_ticket(four_face_ticket),
            )
            .expect_err("the injected four-face precommit panic must be caught")
    });
    assert_eq!(
        four_face_failure.error(),
        &SpeculativeUnprovenFoldResolutionErrorV1::ResolutionPanicked
    );
    assert_layered_resolver_failure_is_bit_identical_v1(&four_face.editor, &four_face_before);
    four_face
        .editor
        .try_resolve_speculative_unproven_fold_layered_four_face_certified_v1(
            four_face_failure.into_proof(),
        )
        .expect("the exact four-face proof remains retryable after the caught panic");
}

#[test]
fn layered_typed_proof_resolves_only_its_exact_awaiting_mark() {
    let mut exact = fixture();
    let exact_binding = binding(
        &exact,
        SpeculativeApproximateBlockingObservationV1::no_blocking_sample_observed(),
    );
    let exact_ticket = apply_marked_with_ticket(&mut exact, exact_binding.clone());
    let exact_before = layered_resolver_editor_state_v1(&exact.editor);
    assert_eq!(
        exact
            .editor
            .resolve_speculative_unproven_fold_layered_three_face_certified_v1(
                layered_proof_for_ticket(exact_ticket),
            )
            .expect("exact layered proof resolves its Awaiting mark")
            .outcome,
        SpeculativeUnprovenFoldProofOutcomeV1::Certified
    );
    assert_layered_resolver_success_preserves_document_v1(&exact.editor, &exact_before);
    let exact_after_marks = exact.editor.speculative_unproven_fold_state_marker_v1();
    assert_eq!(
        exact_before.marks.applied_base,
        exact_after_marks.applied_base
    );
    assert_eq!(exact_before.marks.redo_marks, exact_after_marks.redo_marks);
    assert_eq!(exact_before.marks.undo_marks.len(), 1);
    assert!(matches!(
        exact_before.marks.undo_marks.as_slice(),
        [Some(mark)] if mark.binding == exact_binding
            && mark.status == SpeculativeUnprovenFoldStatusV1::AwaitingProof
    ));
    assert_eq!(exact_after_marks.undo_marks, vec![None]);
    assert_eq!(
        exact.editor.speculative_unproven_fold_summary_v1(),
        SpeculativeUnprovenFoldSummaryV1::default()
    );
}

#[test]
fn layered_typed_proof_negative_matrix_preserves_the_exact_editor_state() {
    let mut target_revision_drift = fixture();
    let target_revision_drift_binding = binding(
        &target_revision_drift,
        SpeculativeApproximateBlockingObservationV1::no_blocking_sample_observed(),
    );
    let target_revision_drift_ticket =
        apply_marked_with_ticket(&mut target_revision_drift, target_revision_drift_binding);
    let target_revision_drift_before =
        layered_resolver_editor_state_v1(&target_revision_drift.editor);
    let drifted_target_revision = target_revision_drift
        .editor
        .revision()
        .checked_add(1)
        .expect("drifted target revision");
    assert_eq!(
        target_revision_drift
            .editor
            .resolve_speculative_unproven_fold_layered_three_face_certified_v1(
                layered_proof_for_ticket_with_target_revision(
                    target_revision_drift_ticket,
                    drifted_target_revision,
                ),
            ),
        Err(SpeculativeUnprovenFoldResolutionErrorV1::InvalidCertifiedProof)
    );
    assert_layered_resolver_failure_is_bit_identical_v1(
        &target_revision_drift.editor,
        &target_revision_drift_before,
    );

    let mut metadata_drift = fixture();
    let exact_binding = binding(
        &metadata_drift,
        SpeculativeApproximateBlockingObservationV1::no_blocking_sample_observed(),
    );
    let metadata_ticket = apply_marked_with_ticket(&mut metadata_drift, exact_binding.clone());
    let drifted_binding = SpeculativeUnprovenFoldBindingV1::new(
        exact_binding.project_instance_id(),
        exact_binding.project_id(),
        exact_binding.source_revision(),
        exact_binding
            .source_geometry_fingerprint_sha256()
            .to_owned(),
        exact_binding.pose_generation() + 1,
        exact_binding.request_generation_id(),
        f64::from_bits(exact_binding.paper_thickness_bits()),
        exact_binding.approximate_blocking_observation(),
    )
    .expect("well-formed metadata drift");
    metadata_drift.editor.undo_stack[0]
        .speculative_unproven_fold
        .as_mut()
        .expect("awaiting mark")
        .binding = drifted_binding;
    let metadata_drift_before = layered_resolver_editor_state_v1(&metadata_drift.editor);
    assert_eq!(
        metadata_drift
            .editor
            .resolve_speculative_unproven_fold_layered_three_face_certified_v1(
                layered_proof_for_ticket(metadata_ticket),
            ),
        Err(SpeculativeUnprovenFoldResolutionErrorV1::BindingMetadataMismatch)
    );
    assert_layered_resolver_failure_is_bit_identical_v1(
        &metadata_drift.editor,
        &metadata_drift_before,
    );

    let mut duplicate = fixture();
    let duplicate_binding = binding(
        &duplicate,
        SpeculativeApproximateBlockingObservationV1::no_blocking_sample_observed(),
    );
    let duplicate_ticket = apply_marked_with_ticket(&mut duplicate, duplicate_binding);
    duplicate
        .editor
        .execute(
            duplicate.editor.revision(),
            Command::UpdateProjectMemo {
                memo: "second history entry".to_owned(),
            },
        )
        .expect("append history entry");
    let duplicated_mark = duplicate.editor.undo_stack[0]
        .speculative_unproven_fold
        .clone();
    duplicate.editor.undo_stack[1].speculative_unproven_fold = duplicated_mark;
    let duplicate_before = layered_resolver_editor_state_v1(&duplicate.editor);
    assert_eq!(
        duplicate
            .editor
            .resolve_speculative_unproven_fold_layered_three_face_certified_v1(
                layered_proof_for_ticket(duplicate_ticket),
            ),
        Err(SpeculativeUnprovenFoldResolutionErrorV1::DuplicateBinding)
    );
    assert_layered_resolver_failure_is_bit_identical_v1(&duplicate.editor, &duplicate_before);

    let mut missing = fixture();
    let mut same_anchor_without_mark = missing.editor.clone();
    let missing_binding = binding(
        &missing,
        SpeculativeApproximateBlockingObservationV1::no_blocking_sample_observed(),
    );
    let missing_ticket = apply_marked_with_ticket(&mut missing, missing_binding);
    let missing_before = layered_resolver_editor_state_v1(&same_anchor_without_mark);
    let missing_source_before = layered_resolver_editor_state_v1(&missing.editor);
    assert_eq!(
        same_anchor_without_mark.resolve_speculative_unproven_fold_layered_three_face_certified_v1(
            layered_proof_for_ticket(missing_ticket),
        ),
        Err(SpeculativeUnprovenFoldResolutionErrorV1::BindingNotFound)
    );
    assert_layered_resolver_failure_is_bit_identical_v1(&same_anchor_without_mark, &missing_before);
    assert_layered_resolver_failure_is_bit_identical_v1(&missing.editor, &missing_source_before);

    let mut source = fixture();
    let foreign_binding = binding(
        &source,
        SpeculativeApproximateBlockingObservationV1::no_blocking_sample_observed(),
    );
    let foreign_ticket = apply_marked_with_ticket(&mut source, foreign_binding);
    let mut foreign_editor = EditorState::with_paper(
        source.editor.pattern().clone(),
        source.editor.paper().clone(),
    );
    let foreign_before = layered_resolver_editor_state_v1(&foreign_editor);
    let foreign_source_before = layered_resolver_editor_state_v1(&source.editor);
    assert_eq!(
        foreign_editor.resolve_speculative_unproven_fold_layered_three_face_certified_v1(
            layered_proof_for_ticket(foreign_ticket),
        ),
        Err(SpeculativeUnprovenFoldResolutionErrorV1::ForeignEditor)
    );
    assert_layered_resolver_failure_is_bit_identical_v1(&foreign_editor, &foreign_before);
    assert_layered_resolver_failure_is_bit_identical_v1(&source.editor, &foreign_source_before);

    let mut blocked = fixture();
    let blocked_binding = binding(
        &blocked,
        SpeculativeApproximateBlockingObservationV1::no_blocking_sample_observed(),
    );
    let blocked_ticket = apply_marked_with_ticket(&mut blocked, blocked_binding.clone());
    blocked
        .editor
        .resolve_speculative_unproven_fold_v1(
            &blocked_binding,
            SpeculativeUnprovenFoldProofOutcomeV1::Blocked,
        )
        .expect("record blocked terminal mark");
    let blocked_before = layered_resolver_editor_state_v1(&blocked.editor);
    assert_eq!(
        blocked
            .editor
            .resolve_speculative_unproven_fold_layered_three_face_certified_v1(
                layered_proof_for_ticket(blocked_ticket),
            ),
        Err(SpeculativeUnprovenFoldResolutionErrorV1::AlreadyResolved)
    );
    assert_layered_resolver_failure_is_bit_identical_v1(&blocked.editor, &blocked_before);

    let mut unknown = fixture();
    let unknown_binding = binding(
        &unknown,
        SpeculativeApproximateBlockingObservationV1::no_blocking_sample_observed(),
    );
    let unknown_ticket = apply_marked_with_ticket(&mut unknown, unknown_binding.clone());
    unknown
        .editor
        .resolve_speculative_unproven_fold_v1(
            &unknown_binding,
            SpeculativeUnprovenFoldProofOutcomeV1::Unknown {
                reason: SpeculativeUnprovenFoldUnknownReasonV1::ResourceLimit,
            },
        )
        .expect("record unknown terminal mark");
    let unknown_before = layered_resolver_editor_state_v1(&unknown.editor);
    assert_eq!(
        unknown
            .editor
            .resolve_speculative_unproven_fold_layered_three_face_certified_v1(
                layered_proof_for_ticket(unknown_ticket),
            ),
        Err(SpeculativeUnprovenFoldResolutionErrorV1::AlreadyResolved)
    );
    assert_layered_resolver_failure_is_bit_identical_v1(&unknown.editor, &unknown_before);
}

#[test]
fn every_speculative_commit_allocation_failpoint_is_atomic_and_retryable() {
    for resource in [
        SpeculativeUnprovenFoldApplyResourceV1::HistoryMarkBinding,
        SpeculativeUnprovenFoldApplyResourceV1::TargetPattern,
        SpeculativeUnprovenFoldApplyResourceV1::TargetPaper,
        SpeculativeUnprovenFoldApplyResourceV1::TargetInstructionTimeline,
        SpeculativeUnprovenFoldApplyResourceV1::TargetProjectLayers,
        SpeculativeUnprovenFoldApplyResourceV1::TargetBeginnerDesignProfile,
        SpeculativeUnprovenFoldApplyResourceV1::CurrentTargetPose,
        SpeculativeUnprovenFoldApplyResourceV1::ResolutionTicketTargetPose,
        SpeculativeUnprovenFoldApplyResourceV1::UndoHistoryEntries,
    ] {
        assert_speculative_apply_failpoint_is_atomic(&mut fixture(), resource);
    }

    let mut trimming = fixture();
    trimming
        .editor
        .set_history_entry_limit(1)
        .expect("one-entry history");
    trimming
        .editor
        .execute(
            0,
            Command::UpdateProjectMemo {
                memo: "entry carrying the pre-existing mark".to_owned(),
            },
        )
        .expect("seed one history entry");
    let retained_binding = binding(
        &trimming,
        SpeculativeApproximateBlockingObservationV1::no_blocking_sample_observed(),
    );
    trimming.editor.undo_stack[0].speculative_unproven_fold =
        Some(SpeculativeUnprovenFoldMarkV1::awaiting(retained_binding));
    assert_speculative_apply_failpoint_is_atomic(
        &mut trimming,
        SpeculativeUnprovenFoldApplyResourceV1::RetainedBaseMarks,
    );
    assert_eq!(
        trimming
            .editor
            .speculative_unproven_fold_summary_v1()
            .applied
            .awaiting_proof,
        2
    );
}

#[test]
fn speculative_owned_commit_keeps_deep_profile_current_and_both_history_snapshots() {
    let mut fixture = fixture();
    let mut rich_profile = fixture.editor.beginner_design_profile().clone();
    rich_profile.reference_surface_landmarks_tenths_mm =
        Some(vec![[10, 20, 30], [40, 50, 60], [70, 80, 90]]);
    fixture
        .editor
        .restore_beginner_design_profile(rich_profile.clone())
        .expect("valid deep profile");

    let target_revision = fixture.editor.revision() + 1;
    let token = token_for_target(
        &fixture,
        target_revision,
        &fixture.target_pattern,
        &fixture.paper,
        &fixture.applied_pose,
    );
    fixture
        .editor
        .execute_stacked_fold_document_with_unproven_mark_and_resolution_ticket_v1(token)
        .expect("owned speculative commit");
    assert_eq!(fixture.editor.beginner_design_profile(), &rich_profile);

    fixture
        .editor
        .restore_beginner_design_profile(BeginnerDesignProfileV1::default())
        .expect("replace only the live snapshot");
    fixture
        .editor
        .undo(target_revision)
        .expect("inverse owns its independent deep source snapshot");
    assert_eq!(fixture.editor.beginner_design_profile(), &rich_profile);
    fixture
        .editor
        .restore_beginner_design_profile(BeginnerDesignProfileV1::default())
        .expect("replace the restored live snapshot");
    let stored_forward_profile = match &fixture
        .editor
        .redo_stack
        .last()
        .expect("the speculative entry must be available for Redo")
        .forward
    {
        Command::ApplyStackedFoldDocument(command) => command.beginner_design_profile.as_ref(),
        other => panic!("unexpected speculative Redo command: {other:?}"),
    };
    assert_eq!(
        stored_forward_profile, &rich_profile,
        "the forward history entry owns its independent deep target snapshot"
    );
    fixture
        .editor
        .restore_beginner_design_profile(rich_profile.clone())
        .expect("restore the exact source profile required by authenticated Redo");
    fixture
        .editor
        .redo(target_revision + 1)
        .expect("forward owns its independent deep target snapshot");
    assert_eq!(fixture.editor.beginner_design_profile(), &rich_profile);
}
