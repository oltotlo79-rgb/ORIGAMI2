use ori_domain::ProjectId;

use super::{speculative_unproven_test_support::*, *};

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
    assert_eq!(
        fixture
            .editor
            .execute_stacked_fold_document_with_unproven_mark_v1(
                fixture.editor.revision(),
                fixture.target_pattern.clone(),
                fixture.paper.clone(),
                fixture.timeline.clone(),
                ProjectLayerDocumentV1::default(),
                fixture.applied_pose.clone(),
                drifted,
            ),
        Err(SpeculativeUnprovenFoldApplyErrorV1::DuplicateBinding)
    );
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
        let result = fixture
            .editor
            .execute_stacked_fold_document_with_unproven_mark_v1(
                0,
                fixture.target_pattern.clone(),
                fixture.paper.clone(),
                fixture.timeline.clone(),
                ProjectLayerDocumentV1::default(),
                fixture.applied_pose.clone(),
                binding,
            );
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
    assert_eq!(
        fixture
            .editor
            .execute_stacked_fold_document_with_unproven_mark_v1(
                0,
                fixture.target_pattern.clone(),
                fixture.paper.clone(),
                fixture.timeline.clone(),
                ProjectLayerDocumentV1::default(),
                fixture.applied_pose.clone(),
                blocking,
            ),
        Err(SpeculativeUnprovenFoldApplyErrorV1::ApproximateBlockingSampleObserved)
    );
    assert_eq!(fixture.editor.revision(), 0);
    assert!(!fixture.editor.can_undo());
}
