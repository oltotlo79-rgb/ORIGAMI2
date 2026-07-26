//! Runtime-only semantics for speculative stacked-fold history metadata.
//!
//! The authority token that permits a speculative Apply belongs to the
//! desktop transaction boundary. This module deliberately contains no token
//! and no proof-authority conversion. It records only the coarse, persisted
//! consequence of an authenticated one-shot decision.

mod binding;
mod editor_operations;
mod ledger;
mod status;

#[cfg(test)]
mod ledger_tests;

pub use binding::{SpeculativeApproximateBlockingObservationV1, SpeculativeUnprovenFoldBindingV1};
pub use ledger::SpeculativeUnprovenFoldStateMarkerV1;
pub use status::{
    SpeculativeUnprovenFoldApplyErrorV1, SpeculativeUnprovenFoldHistoryLocationV1,
    SpeculativeUnprovenFoldMetadataErrorV1, SpeculativeUnprovenFoldProofOutcomeV1,
    SpeculativeUnprovenFoldResolutionErrorV1, SpeculativeUnprovenFoldResolutionReportV1,
    SpeculativeUnprovenFoldStatusCountsV1, SpeculativeUnprovenFoldStatusV1,
    SpeculativeUnprovenFoldSummaryV1, SpeculativeUnprovenFoldUnknownReasonV1,
};

pub const MAX_PENDING_SPECULATIVE_UNPROVEN_FOLDS_V1: usize = super::MAX_EDITOR_HISTORY_ENTRIES;
pub const MAX_RETAINED_SPECULATIVE_UNPROVEN_BASE_MARKS_V1: usize =
    super::MAX_EDITOR_HISTORY_ENTRIES * 2;

pub(super) use ledger::{AppliedBaseUnprovenLedgerV1, AppliedBaseUnprovenMarkV1};
pub(super) use status::SpeculativeUnprovenFoldMarkV1;
