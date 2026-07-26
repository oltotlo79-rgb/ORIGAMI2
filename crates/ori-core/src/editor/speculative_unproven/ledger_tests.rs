use ori_domain::ProjectId;

use super::*;

fn terminal_mark(project_id: ProjectId) -> SpeculativeUnprovenFoldMarkV1 {
    SpeculativeUnprovenFoldMarkV1 {
        binding: SpeculativeUnprovenFoldBindingV1::new(
            ProjectId::new(),
            project_id,
            0,
            "0".repeat(64),
            0,
            ProjectId::new(),
            0.1,
            SpeculativeApproximateBlockingObservationV1::no_blocking_sample_observed(),
        )
        .expect("valid ledger fixture binding"),
        status: SpeculativeUnprovenFoldStatusV1::ProofBlocked,
    }
}

#[test]
fn terminal_base_marks_collapse_to_bounded_coarse_counts() {
    let project_id = ProjectId::new();
    let mut ledger = AppliedBaseUnprovenLedgerV1::default();
    for _ in 0..=MAX_RETAINED_SPECULATIVE_UNPROVEN_BASE_MARKS_V1 {
        ledger.absorb_trimmed_applied_marks(vec![Some(terminal_mark(project_id))], 0);
    }

    assert_eq!(
        ledger.retained_marks.len(),
        MAX_RETAINED_SPECULATIVE_UNPROVEN_BASE_MARKS_V1
    );
    assert_eq!(ledger.collapsed_terminal.proof_blocked, 1);
    let mut counts = SpeculativeUnprovenFoldStatusCountsV1::default();
    ledger.add_to_counts(&mut counts);
    assert_eq!(
        counts.proof_blocked,
        (MAX_RETAINED_SPECULATIVE_UNPROVEN_BASE_MARKS_V1 + 1) as u64
    );
    assert_eq!(counts.awaiting_proof, 0);
}
