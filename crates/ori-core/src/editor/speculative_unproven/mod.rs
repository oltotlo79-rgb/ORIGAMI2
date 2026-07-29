//! Runtime-only semantics for speculative stacked-fold history and its typed
//! post-Apply certification boundary.
//!
//! Persisted marks remain nonauthority metadata. Native one-shot tickets and
//! proofs never implement persistence traits and can only remove the exact
//! awaiting mark to which an atomic Apply bound them.

mod binding;
mod certification;
mod editor_operations;
mod ledger;
mod status;

#[cfg(test)]
mod ledger_tests;

pub use binding::{SpeculativeApproximateBlockingObservationV1, SpeculativeUnprovenFoldBindingV1};
pub use certification::{
    SpeculativeUnprovenFoldCertificationErrorV1, SpeculativeUnprovenFoldCertificationFailureV1,
    SpeculativeUnprovenFoldCertifiedProofV1, SpeculativeUnprovenFoldResolutionTicketV1,
    bind_speculative_unproven_tree_continuous_proof_v1,
};
pub use ledger::SpeculativeUnprovenFoldStateMarkerV1;
pub use status::{
    SpeculativeUnprovenFoldApplyErrorV1, SpeculativeUnprovenFoldApplyResourceV1,
    SpeculativeUnprovenFoldHistoryLocationV1, SpeculativeUnprovenFoldMetadataErrorV1,
    SpeculativeUnprovenFoldProofOutcomeV1, SpeculativeUnprovenFoldResolutionErrorV1,
    SpeculativeUnprovenFoldResolutionReportV1, SpeculativeUnprovenFoldStatusCountsV1,
    SpeculativeUnprovenFoldStatusV1, SpeculativeUnprovenFoldSummaryV1,
    SpeculativeUnprovenFoldUnknownReasonV1,
};

pub const MAX_PENDING_SPECULATIVE_UNPROVEN_FOLDS_V1: usize = super::MAX_EDITOR_HISTORY_ENTRIES;
pub const MAX_RETAINED_SPECULATIVE_UNPROVEN_BASE_MARKS_V1: usize =
    super::MAX_EDITOR_HISTORY_ENTRIES * 2;

pub(super) use ledger::{AppliedBaseUnprovenLedgerV1, AppliedBaseUnprovenMarkV1};
pub(super) use status::SpeculativeUnprovenFoldMarkV1;

#[cfg(test)]
pub(crate) use certification::bind_resolution_ticket_for_test_v1;
