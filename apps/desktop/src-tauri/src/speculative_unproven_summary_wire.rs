use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SpeculativeUnprovenFoldSummaryDtoV1 {
    pub(super) applied: SpeculativeUnprovenFoldStatusCountsDtoV1,
    pub(super) unapplied_redo: SpeculativeUnprovenFoldStatusCountsDtoV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SpeculativeUnprovenFoldStatusCountsDtoV1 {
    pub(super) awaiting_proof: u64,
    pub(super) proof_blocked: u64,
    pub(super) unknown_evidence_insufficient: u64,
    pub(super) unknown_resource_limit: u64,
    pub(super) unknown_cancelled: u64,
    pub(super) unknown_deadline_reached: u64,
}

impl From<ori_core::SpeculativeUnprovenFoldStatusCountsV1>
    for SpeculativeUnprovenFoldStatusCountsDtoV1
{
    fn from(value: ori_core::SpeculativeUnprovenFoldStatusCountsV1) -> Self {
        Self {
            awaiting_proof: value.awaiting_proof,
            proof_blocked: value.proof_blocked,
            unknown_evidence_insufficient: value.unknown_evidence_insufficient,
            unknown_resource_limit: value.unknown_resource_limit,
            unknown_cancelled: value.unknown_cancelled,
            unknown_deadline_reached: value.unknown_deadline_reached,
        }
    }
}

impl From<ori_core::SpeculativeUnprovenFoldSummaryV1> for SpeculativeUnprovenFoldSummaryDtoV1 {
    fn from(value: ori_core::SpeculativeUnprovenFoldSummaryV1) -> Self {
        Self {
            applied: value.applied.into(),
            unapplied_redo: value.unapplied_redo.into(),
        }
    }
}
