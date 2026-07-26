use serde::{Deserialize, Serialize};

use super::super::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::editor::history_persistence) struct SpeculativeUnprovenFoldBindingWireV1 {
    pub(super) project_instance_id: ProjectId,
    pub(super) project_id: ProjectId,
    pub(super) source_revision: u64,
    pub(super) source_geometry_fingerprint_sha256: String,
    pub(super) pose_generation: u64,
    pub(super) request_generation_id: ProjectId,
    pub(super) paper_thickness_bits_be: [u8; 8],
    pub(super) approximate_blocking_observation: SpeculativeApproximateBlockingObservationWireV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub(in crate::editor::history_persistence) enum SpeculativeApproximateBlockingObservationWireV1 {
    NoBlockingSampleObserved,
    BlockingSampleObserved {
        first_blocking_angle_bits_be: [u8; 8],
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(in crate::editor::history_persistence) enum SpeculativeUnprovenFoldUnknownReasonWireV1 {
    EvidenceInsufficient,
    ResourceLimit,
    Cancelled,
    DeadlineReached,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub(in crate::editor::history_persistence) enum SpeculativeUnprovenFoldStatusWireV1 {
    AwaitingProof,
    ProofBlocked,
    ProofUnknown {
        reason: SpeculativeUnprovenFoldUnknownReasonWireV1,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::editor::history_persistence) struct SpeculativeUnprovenFoldMarkWireV1 {
    pub(super) binding: SpeculativeUnprovenFoldBindingWireV1,
    pub(super) proof_status: SpeculativeUnprovenFoldStatusWireV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::editor::history_persistence) struct AppliedBaseUnprovenMarkWireV1 {
    pub(super) mark: SpeculativeUnprovenFoldMarkWireV1,
    pub(super) subsequent_applied_entries: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::editor::history_persistence) struct SpeculativeUnprovenFoldStatusCountsWireV1 {
    pub(super) awaiting_proof: u64,
    pub(super) proof_blocked: u64,
    pub(super) unknown_evidence_insufficient: u64,
    pub(super) unknown_resource_limit: u64,
    pub(super) unknown_cancelled: u64,
    pub(super) unknown_deadline_reached: u64,
}

impl SpeculativeUnprovenFoldStatusCountsWireV1 {
    fn checked_total(self) -> Option<u64> {
        self.awaiting_proof
            .checked_add(self.proof_blocked)
            .and_then(|value| value.checked_add(self.unknown_evidence_insufficient))
            .and_then(|value| value.checked_add(self.unknown_resource_limit))
            .and_then(|value| value.checked_add(self.unknown_cancelled))
            .and_then(|value| value.checked_add(self.unknown_deadline_reached))
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::editor::history_persistence) struct AppliedBaseUnprovenLedgerWireV1 {
    pub(super) retained_marks: Vec<AppliedBaseUnprovenMarkWireV1>,
    pub(super) collapsed_terminal: SpeculativeUnprovenFoldStatusCountsWireV1,
}

impl AppliedBaseUnprovenLedgerWireV1 {
    pub(in crate::editor::history_persistence) fn is_empty(&self) -> bool {
        self.retained_marks.is_empty() && self.collapsed_terminal.checked_total() == Some(0)
    }

    pub(in crate::editor::history_persistence) fn validate_shape(
        &self,
        undo_len: usize,
    ) -> Result<(), EditorHistoryErrorV1> {
        if self.retained_marks.len() > MAX_RETAINED_SPECULATIVE_UNPROVEN_BASE_MARKS_V1
            || self.collapsed_terminal.awaiting_proof != 0
            || self
                .collapsed_terminal
                .checked_total()
                .is_none_or(|total| total > MAX_REVISION)
            || self
                .retained_marks
                .iter()
                .any(|item| item.subsequent_applied_entries > MAX_REVISION)
            || self
                .retained_marks
                .iter()
                .any(|item| item.subsequent_applied_entries < undo_len as u64)
            || self.retained_marks.windows(2).any(|pair| {
                pair[0].subsequent_applied_entries <= pair[1].subsequent_applied_entries
            })
        {
            return Err(EditorHistoryErrorV1::InvalidSpeculativeAppliedBaseLedger);
        }
        Ok(())
    }
}
