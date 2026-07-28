use ori_core::{
    SpeculativeUnprovenFoldBindingV1, SpeculativeUnprovenFoldHistoryLocationV1,
    SpeculativeUnprovenFoldProofOutcomeV1, SpeculativeUnprovenFoldResolutionReportV1,
    SpeculativeUnprovenFoldUnknownReasonV1,
};

use super::{AppState, lock_project};

#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SpeculativeUnprovenFoldResolutionDtoV1 {
    location: &'static str,
    outcome: &'static str,
    reason: Option<&'static str>,
    subsequent_edit_count: u64,
    undo_steps_to_revert: Option<u32>,
}

/// Native-only proof completion boundary. Binding material remains in-process;
/// the returned value deliberately contains only coarse history information.
#[allow(dead_code)]
pub(crate) fn resolve_speculative_unproven_fold_native_v1(
    app_state: &AppState,
    binding: &SpeculativeUnprovenFoldBindingV1,
    outcome: SpeculativeUnprovenFoldProofOutcomeV1,
) -> Result<SpeculativeUnprovenFoldResolutionDtoV1, String> {
    let mut project =
        lock_project(app_state).map_err(|_| "The project is unavailable.".to_owned())?;
    let report = project
        .editor
        .resolve_speculative_unproven_fold_v1(binding, outcome)
        .map_err(|_| "The speculative proof result is stale or invalid.".to_owned())?;
    Ok(resolution_dto_v1(report))
}

pub(super) fn resolution_dto_v1(
    report: SpeculativeUnprovenFoldResolutionReportV1,
) -> SpeculativeUnprovenFoldResolutionDtoV1 {
    let location = match report.location {
        SpeculativeUnprovenFoldHistoryLocationV1::AppliedTrimmedBase => "applied_trimmed_base",
        SpeculativeUnprovenFoldHistoryLocationV1::AppliedRetainedUndo => "applied_retained_undo",
        SpeculativeUnprovenFoldHistoryLocationV1::UnappliedRedo => "unapplied_redo",
    };
    let (outcome, reason) = match report.outcome {
        SpeculativeUnprovenFoldProofOutcomeV1::Certified => ("certified", None),
        SpeculativeUnprovenFoldProofOutcomeV1::Blocked => ("blocked", None),
        SpeculativeUnprovenFoldProofOutcomeV1::Unknown { reason } => (
            "unknown",
            Some(match reason {
                SpeculativeUnprovenFoldUnknownReasonV1::EvidenceInsufficient => {
                    "evidence_insufficient"
                }
                SpeculativeUnprovenFoldUnknownReasonV1::ResourceLimit => "resource_limit",
                SpeculativeUnprovenFoldUnknownReasonV1::Cancelled => "cancelled",
                SpeculativeUnprovenFoldUnknownReasonV1::DeadlineReached => "deadline_reached",
            }),
        ),
    };
    SpeculativeUnprovenFoldResolutionDtoV1 {
        location,
        outcome,
        reason,
        subsequent_edit_count: report.subsequent_edit_count,
        undo_steps_to_revert: report.undo_steps_to_revert,
    }
}

#[cfg(test)]
mod tests {
    use ori_core::{
        SpeculativeUnprovenFoldHistoryLocationV1, SpeculativeUnprovenFoldProofOutcomeV1,
        SpeculativeUnprovenFoldResolutionReportV1, SpeculativeUnprovenFoldUnknownReasonV1,
    };

    use super::*;

    #[test]
    fn resolution_dto_is_exact_and_contains_no_binding_or_geometry_material() {
        let dto = resolution_dto_v1(SpeculativeUnprovenFoldResolutionReportV1 {
            location: SpeculativeUnprovenFoldHistoryLocationV1::AppliedRetainedUndo,
            outcome: SpeculativeUnprovenFoldProofOutcomeV1::Unknown {
                reason: SpeculativeUnprovenFoldUnknownReasonV1::DeadlineReached,
            },
            subsequent_edit_count: 7,
            undo_steps_to_revert: Some(8),
        });
        let json = serde_json::to_value(dto).expect("resolution JSON");
        assert_eq!(
            json,
            serde_json::json!({
                "location": "applied_retained_undo",
                "outcome": "unknown",
                "reason": "deadline_reached",
                "subsequentEditCount": 7,
                "undoStepsToRevert": 8
            })
        );
        let text = json.to_string();
        for forbidden in [
            "project",
            "binding",
            "geometry",
            "fingerprint",
            "coordinate",
            "vertex",
            "edge",
            "face",
        ] {
            assert!(!text.contains(forbidden), "leaked {forbidden}");
        }
    }
}
