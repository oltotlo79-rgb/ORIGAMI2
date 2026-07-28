use thiserror::Error;

use super::{super::CommandError, SpeculativeUnprovenFoldBindingV1};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpeculativeUnprovenFoldUnknownReasonV1 {
    EvidenceInsufficient,
    ResourceLimit,
    Cancelled,
    DeadlineReached,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpeculativeUnprovenFoldStatusV1 {
    AwaitingProof,
    ProofBlocked,
    ProofUnknown {
        reason: SpeculativeUnprovenFoldUnknownReasonV1,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpeculativeUnprovenFoldProofOutcomeV1 {
    /// Reserved for a future resolver that requires an opaque, typed proof.
    ///
    /// The generic history resolver rejects this outcome because a binding is
    /// metadata, not certification authority.
    Certified,
    Blocked,
    Unknown {
        reason: SpeculativeUnprovenFoldUnknownReasonV1,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SpeculativeUnprovenFoldMarkV1 {
    pub(crate) binding: SpeculativeUnprovenFoldBindingV1,
    pub(crate) status: SpeculativeUnprovenFoldStatusV1,
}

impl SpeculativeUnprovenFoldMarkV1 {
    pub(crate) fn awaiting(binding: SpeculativeUnprovenFoldBindingV1) -> Self {
        Self {
            binding,
            status: SpeculativeUnprovenFoldStatusV1::AwaitingProof,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SpeculativeUnprovenFoldStatusCountsV1 {
    pub awaiting_proof: u64,
    pub proof_blocked: u64,
    pub unknown_evidence_insufficient: u64,
    pub unknown_resource_limit: u64,
    pub unknown_cancelled: u64,
    pub unknown_deadline_reached: u64,
}

impl SpeculativeUnprovenFoldStatusCountsV1 {
    #[must_use]
    pub fn total(self) -> u64 {
        self.awaiting_proof
            .checked_add(self.proof_blocked)
            .and_then(|value| value.checked_add(self.unknown_evidence_insufficient))
            .and_then(|value| value.checked_add(self.unknown_resource_limit))
            .and_then(|value| value.checked_add(self.unknown_cancelled))
            .and_then(|value| value.checked_add(self.unknown_deadline_reached))
            .expect("unproven-fold status counts are bounded by editor revisions")
    }

    pub(super) fn add_status(&mut self, status: SpeculativeUnprovenFoldStatusV1) {
        let target = match status {
            SpeculativeUnprovenFoldStatusV1::AwaitingProof => &mut self.awaiting_proof,
            SpeculativeUnprovenFoldStatusV1::ProofBlocked => &mut self.proof_blocked,
            SpeculativeUnprovenFoldStatusV1::ProofUnknown {
                reason: SpeculativeUnprovenFoldUnknownReasonV1::EvidenceInsufficient,
            } => &mut self.unknown_evidence_insufficient,
            SpeculativeUnprovenFoldStatusV1::ProofUnknown {
                reason: SpeculativeUnprovenFoldUnknownReasonV1::ResourceLimit,
            } => &mut self.unknown_resource_limit,
            SpeculativeUnprovenFoldStatusV1::ProofUnknown {
                reason: SpeculativeUnprovenFoldUnknownReasonV1::Cancelled,
            } => &mut self.unknown_cancelled,
            SpeculativeUnprovenFoldStatusV1::ProofUnknown {
                reason: SpeculativeUnprovenFoldUnknownReasonV1::DeadlineReached,
            } => &mut self.unknown_deadline_reached,
        };
        *target = target
            .checked_add(1)
            .expect("unproven-fold status count cannot exceed editor revisions");
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SpeculativeUnprovenFoldSummaryV1 {
    pub applied: SpeculativeUnprovenFoldStatusCountsV1,
    pub unapplied_redo: SpeculativeUnprovenFoldStatusCountsV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpeculativeUnprovenFoldHistoryLocationV1 {
    AppliedTrimmedBase,
    AppliedRetainedUndo,
    UnappliedRedo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpeculativeUnprovenFoldResolutionReportV1 {
    pub location: SpeculativeUnprovenFoldHistoryLocationV1,
    pub outcome: SpeculativeUnprovenFoldProofOutcomeV1,
    pub subsequent_edit_count: u64,
    pub undo_steps_to_revert: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SpeculativeUnprovenFoldMetadataErrorV1 {
    #[error("the speculative mark project instance ID must not be nil")]
    NilProjectInstanceId,
    #[error("the speculative mark project ID must not be nil")]
    NilProjectId,
    #[error("the speculative mark request generation ID must not be nil")]
    NilRequestGenerationId,
    #[error("the speculative mark revision is outside the supported range")]
    RevisionOutOfRange,
    #[error("the speculative mark pose generation is outside the supported range")]
    PoseGenerationOutOfRange,
    #[error("the speculative mark geometry fingerprint is not lowercase SHA-256 hex")]
    InvalidGeometryFingerprint,
    #[error("the speculative mark paper thickness is invalid")]
    InvalidPaperThickness,
    #[error("the speculative mark blocking angle is invalid")]
    InvalidBlockingAngle,
}

#[derive(Debug, PartialEq, Error)]
pub enum SpeculativeUnprovenFoldApplyErrorV1 {
    #[error(transparent)]
    InvalidMetadata(#[from] SpeculativeUnprovenFoldMetadataErrorV1),
    #[error("the speculative source revision does not match the current editor")]
    SourceRevisionMismatch,
    #[error("the speculative source geometry fingerprint is stale")]
    SourceGeometryFingerprintMismatch,
    #[error("the speculative paper-thickness bits are stale")]
    PaperThicknessBitsMismatch,
    #[error("an approximate blocking sample forbids speculative Apply")]
    ApproximateBlockingSampleObserved,
    #[error("the speculative request binding already exists in history")]
    DuplicateBinding,
    #[error("the unresolved speculative history limit has been reached")]
    PendingMarkLimitReached,
    #[error(transparent)]
    Command(#[from] CommandError),
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SpeculativeUnprovenFoldResolutionErrorV1 {
    #[error(transparent)]
    InvalidMetadata(#[from] SpeculativeUnprovenFoldMetadataErrorV1),
    #[error("the speculative history binding was not found")]
    BindingNotFound,
    #[error("the speculative history binding occurs more than once")]
    DuplicateBinding,
    #[error("the speculative request identity exists with different binding metadata")]
    BindingMetadataMismatch,
    #[error("the speculative history binding already has a terminal result")]
    AlreadyResolved,
    #[error("certified resolution requires an opaque typed proof")]
    CertifiedRequiresTypedProof,
}
