use serde::{Deserialize, Serialize};

use super::{
    DiagnosticScope, DiagnosticsError, DiagnosticsSchema, MAX_DIAGNOSTICS_BYTES,
    StoredDiagnosticCount, StoredDiagnostics,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum DiagnosticsShareSchema {
    #[serde(rename = "origami2.redacted-diagnostics.v2")]
    V2,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SharedUnprovenStatusCountsV2 {
    awaiting_proof: u64,
    proof_blocked: u64,
    unknown_evidence_insufficient: u64,
    unknown_resource_limit: u64,
    unknown_cancelled: u64,
    unknown_deadline_reached: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SharedUnprovenSummaryV2 {
    applied: SharedUnprovenStatusCountsV2,
    unapplied_redo: SharedUnprovenStatusCountsV2,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SharedDiagnosticsV2 {
    schema: DiagnosticsShareSchema,
    unexpected: Vec<StoredDiagnosticCount>,
    #[serde(rename = "speculativeUnprovenFolds")]
    speculative_unproven_folds: SharedUnprovenSummaryV2,
}

impl From<ori_core::SpeculativeUnprovenFoldStatusCountsV1> for SharedUnprovenStatusCountsV2 {
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

impl From<ori_core::SpeculativeUnprovenFoldSummaryV1> for SharedUnprovenSummaryV2 {
    fn from(value: ori_core::SpeculativeUnprovenFoldSummaryV1) -> Self {
        Self {
            applied: value.applied.into(),
            unapplied_redo: value.unapplied_redo.into(),
        }
    }
}

impl SharedDiagnosticsV2 {
    pub(super) fn from_counts_and_unproven(
        counts: &[u8; DiagnosticScope::ALL.len()],
        unproven: ori_core::SpeculativeUnprovenFoldSummaryV1,
    ) -> Self {
        Self {
            schema: DiagnosticsShareSchema::V2,
            unexpected: StoredDiagnostics::from_counts(counts).unexpected,
            speculative_unproven_folds: unproven.into(),
        }
    }

    fn validate(&self) -> Result<(), DiagnosticsError> {
        StoredDiagnostics {
            schema: DiagnosticsSchema::V1,
            unexpected: self.unexpected.clone(),
        }
        .validated_counts()?;
        validate_status_counts(self.speculative_unproven_folds.applied)?;
        validate_status_counts(self.speculative_unproven_folds.unapplied_redo)
    }
}

fn validate_status_counts(counts: SharedUnprovenStatusCountsV2) -> Result<(), DiagnosticsError> {
    [
        counts.awaiting_proof,
        counts.proof_blocked,
        counts.unknown_evidence_insufficient,
        counts.unknown_resource_limit,
        counts.unknown_cancelled,
        counts.unknown_deadline_reached,
    ]
    .into_iter()
    .try_fold(0_u64, u64::checked_add)
    .map(|_| ())
    .ok_or(DiagnosticsError)
}

pub(super) fn serialize_canonical(
    snapshot: &SharedDiagnosticsV2,
) -> Result<Vec<u8>, DiagnosticsError> {
    let bytes = serde_json::to_vec(snapshot).map_err(|_| DiagnosticsError)?;
    validate_canonical(&bytes)?;
    Ok(bytes)
}

pub(super) fn validate_canonical(bytes: &[u8]) -> Result<(), DiagnosticsError> {
    if bytes.len() > MAX_DIAGNOSTICS_BYTES {
        return Err(DiagnosticsError);
    }
    let shared: SharedDiagnosticsV2 =
        serde_json::from_slice(bytes).map_err(|_| DiagnosticsError)?;
    shared.validate()?;
    if serde_json::to_vec(&shared).map_err(|_| DiagnosticsError)? != bytes {
        return Err(DiagnosticsError);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_coarse_summary_is_canonical_and_contains_no_project_material() {
        let shared = SharedDiagnosticsV2::from_counts_and_unproven(
            &[0; DiagnosticScope::ALL.len()],
            ori_core::SpeculativeUnprovenFoldSummaryV1 {
                applied: ori_core::SpeculativeUnprovenFoldStatusCountsV1 {
                    awaiting_proof: 1,
                    proof_blocked: 2,
                    unknown_evidence_insufficient: 3,
                    unknown_resource_limit: 4,
                    unknown_cancelled: 5,
                    unknown_deadline_reached: 6,
                },
                unapplied_redo: ori_core::SpeculativeUnprovenFoldStatusCountsV1 {
                    awaiting_proof: 7,
                    proof_blocked: 8,
                    unknown_evidence_insufficient: 9,
                    unknown_resource_limit: 10,
                    unknown_cancelled: 11,
                    unknown_deadline_reached: 12,
                },
            },
        );
        let bytes = serialize_canonical(&shared).expect("canonical v2");
        let value: serde_json::Value = serde_json::from_slice(&bytes).expect("v2 JSON");
        assert_eq!(
            value["schema"],
            serde_json::json!("origami2.redacted-diagnostics.v2")
        );
        assert_eq!(value.as_object().expect("diagnostics object").len(), 3);
        assert_eq!(
            value["speculativeUnprovenFolds"]["applied"]["unknownDeadlineReached"],
            6
        );
        assert_eq!(
            value["speculativeUnprovenFolds"]["unappliedRedo"]["unknownCancelled"],
            11
        );
        let coarse = value["speculativeUnprovenFolds"].to_string();
        for forbidden in [
            "project",
            "path",
            "binding",
            "geometry",
            "coordinate",
            "fingerprint",
            "vertex",
            "edge",
            "face",
            "error",
        ] {
            assert!(!coarse.contains(forbidden), "leaked {forbidden}");
        }
    }

    #[test]
    fn malformed_unknown_overflow_and_noncanonical_v2_fail_closed() {
        let canonical = serialize_canonical(&SharedDiagnosticsV2::from_counts_and_unproven(
            &[0; DiagnosticScope::ALL.len()],
            ori_core::SpeculativeUnprovenFoldSummaryV1::default(),
        ))
        .expect("canonical v2");
        let mut value: serde_json::Value =
            serde_json::from_slice(&canonical).expect("canonical JSON");
        value["speculativeUnprovenFolds"]["applied"]["future"] = serde_json::json!(1);
        assert!(validate_canonical(&serde_json::to_vec(&value).unwrap()).is_err());

        let mut value: serde_json::Value =
            serde_json::from_slice(&canonical).expect("canonical JSON");
        value["speculativeUnprovenFolds"]["applied"]["awaitingProof"] = serde_json::json!(u64::MAX);
        value["speculativeUnprovenFolds"]["applied"]["proofBlocked"] = serde_json::json!(1);
        assert!(validate_canonical(&serde_json::to_vec(&value).unwrap()).is_err());

        let mut noncanonical = vec![b' '];
        noncanonical.extend(canonical);
        assert!(validate_canonical(&noncanonical).is_err());
    }
}
