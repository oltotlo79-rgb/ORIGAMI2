use super::{super::*, wire::*};

fn approximate_observation_to_wire(
    observation: SpeculativeApproximateBlockingObservationV1,
) -> SpeculativeApproximateBlockingObservationWireV1 {
    match observation {
        SpeculativeApproximateBlockingObservationV1::NoBlockingSampleObserved => {
            SpeculativeApproximateBlockingObservationWireV1::NoBlockingSampleObserved
        }
        SpeculativeApproximateBlockingObservationV1::BlockingSampleObserved {
            first_blocking_angle_bits,
        } => SpeculativeApproximateBlockingObservationWireV1::BlockingSampleObserved {
            first_blocking_angle_bits_be: first_blocking_angle_bits.to_be_bytes(),
        },
    }
}

fn approximate_observation_from_wire(
    observation: SpeculativeApproximateBlockingObservationWireV1,
) -> SpeculativeApproximateBlockingObservationV1 {
    match observation {
        SpeculativeApproximateBlockingObservationWireV1::NoBlockingSampleObserved => {
            SpeculativeApproximateBlockingObservationV1::NoBlockingSampleObserved
        }
        SpeculativeApproximateBlockingObservationWireV1::BlockingSampleObserved {
            first_blocking_angle_bits_be,
        } => SpeculativeApproximateBlockingObservationV1::BlockingSampleObserved {
            first_blocking_angle_bits: u64::from_be_bytes(first_blocking_angle_bits_be),
        },
    }
}

fn binding_to_wire(
    binding: &SpeculativeUnprovenFoldBindingV1,
) -> SpeculativeUnprovenFoldBindingWireV1 {
    SpeculativeUnprovenFoldBindingWireV1 {
        project_instance_id: binding.project_instance_id(),
        project_id: binding.project_id(),
        source_revision: binding.source_revision(),
        source_geometry_fingerprint_sha256: binding.source_geometry_fingerprint_sha256().to_owned(),
        pose_generation: binding.pose_generation(),
        request_generation_id: binding.request_generation_id(),
        paper_thickness_bits_be: binding.paper_thickness_bits().to_be_bytes(),
        approximate_blocking_observation: approximate_observation_to_wire(
            binding.approximate_blocking_observation(),
        ),
    }
}

fn binding_from_wire(
    binding: SpeculativeUnprovenFoldBindingWireV1,
) -> Result<SpeculativeUnprovenFoldBindingV1, EditorHistoryErrorV1> {
    SpeculativeUnprovenFoldBindingV1::from_exact_parts(
        binding.project_instance_id,
        binding.project_id,
        binding.source_revision,
        binding.source_geometry_fingerprint_sha256,
        binding.pose_generation,
        binding.request_generation_id,
        u64::from_be_bytes(binding.paper_thickness_bits_be),
        approximate_observation_from_wire(binding.approximate_blocking_observation),
    )
    .map_err(|_| EditorHistoryErrorV1::InvalidSpeculativeUnprovenMetadata)
}

const fn unknown_reason_to_wire(
    reason: SpeculativeUnprovenFoldUnknownReasonV1,
) -> SpeculativeUnprovenFoldUnknownReasonWireV1 {
    match reason {
        SpeculativeUnprovenFoldUnknownReasonV1::EvidenceInsufficient => {
            SpeculativeUnprovenFoldUnknownReasonWireV1::EvidenceInsufficient
        }
        SpeculativeUnprovenFoldUnknownReasonV1::ResourceLimit => {
            SpeculativeUnprovenFoldUnknownReasonWireV1::ResourceLimit
        }
        SpeculativeUnprovenFoldUnknownReasonV1::Cancelled => {
            SpeculativeUnprovenFoldUnknownReasonWireV1::Cancelled
        }
        SpeculativeUnprovenFoldUnknownReasonV1::DeadlineReached => {
            SpeculativeUnprovenFoldUnknownReasonWireV1::DeadlineReached
        }
    }
}

const fn unknown_reason_from_wire(
    reason: SpeculativeUnprovenFoldUnknownReasonWireV1,
) -> SpeculativeUnprovenFoldUnknownReasonV1 {
    match reason {
        SpeculativeUnprovenFoldUnknownReasonWireV1::EvidenceInsufficient => {
            SpeculativeUnprovenFoldUnknownReasonV1::EvidenceInsufficient
        }
        SpeculativeUnprovenFoldUnknownReasonWireV1::ResourceLimit => {
            SpeculativeUnprovenFoldUnknownReasonV1::ResourceLimit
        }
        SpeculativeUnprovenFoldUnknownReasonWireV1::Cancelled => {
            SpeculativeUnprovenFoldUnknownReasonV1::Cancelled
        }
        SpeculativeUnprovenFoldUnknownReasonWireV1::DeadlineReached => {
            SpeculativeUnprovenFoldUnknownReasonV1::DeadlineReached
        }
    }
}

pub(in crate::editor::history_persistence) fn mark_to_wire(
    mark: &SpeculativeUnprovenFoldMarkV1,
) -> SpeculativeUnprovenFoldMarkWireV1 {
    let proof_status = match mark.status {
        SpeculativeUnprovenFoldStatusV1::AwaitingProof => {
            SpeculativeUnprovenFoldStatusWireV1::AwaitingProof
        }
        SpeculativeUnprovenFoldStatusV1::ProofBlocked => {
            SpeculativeUnprovenFoldStatusWireV1::ProofBlocked
        }
        SpeculativeUnprovenFoldStatusV1::ProofUnknown { reason } => {
            SpeculativeUnprovenFoldStatusWireV1::ProofUnknown {
                reason: unknown_reason_to_wire(reason),
            }
        }
    };
    SpeculativeUnprovenFoldMarkWireV1 {
        binding: binding_to_wire(&mark.binding),
        proof_status,
    }
}

pub(in crate::editor::history_persistence) fn mark_from_wire(
    mark: SpeculativeUnprovenFoldMarkWireV1,
) -> Result<SpeculativeUnprovenFoldMarkV1, EditorHistoryErrorV1> {
    let status = match mark.proof_status {
        SpeculativeUnprovenFoldStatusWireV1::AwaitingProof => {
            SpeculativeUnprovenFoldStatusV1::AwaitingProof
        }
        SpeculativeUnprovenFoldStatusWireV1::ProofBlocked => {
            SpeculativeUnprovenFoldStatusV1::ProofBlocked
        }
        SpeculativeUnprovenFoldStatusWireV1::ProofUnknown { reason } => {
            SpeculativeUnprovenFoldStatusV1::ProofUnknown {
                reason: unknown_reason_from_wire(reason),
            }
        }
    };
    Ok(SpeculativeUnprovenFoldMarkV1 {
        binding: binding_from_wire(mark.binding)?,
        status,
    })
}

fn status_counts_to_wire(
    counts: SpeculativeUnprovenFoldStatusCountsV1,
) -> SpeculativeUnprovenFoldStatusCountsWireV1 {
    SpeculativeUnprovenFoldStatusCountsWireV1 {
        awaiting_proof: counts.awaiting_proof,
        proof_blocked: counts.proof_blocked,
        unknown_evidence_insufficient: counts.unknown_evidence_insufficient,
        unknown_resource_limit: counts.unknown_resource_limit,
        unknown_cancelled: counts.unknown_cancelled,
        unknown_deadline_reached: counts.unknown_deadline_reached,
    }
}

fn status_counts_from_wire(
    counts: SpeculativeUnprovenFoldStatusCountsWireV1,
) -> SpeculativeUnprovenFoldStatusCountsV1 {
    SpeculativeUnprovenFoldStatusCountsV1 {
        awaiting_proof: counts.awaiting_proof,
        proof_blocked: counts.proof_blocked,
        unknown_evidence_insufficient: counts.unknown_evidence_insufficient,
        unknown_resource_limit: counts.unknown_resource_limit,
        unknown_cancelled: counts.unknown_cancelled,
        unknown_deadline_reached: counts.unknown_deadline_reached,
    }
}

pub(in crate::editor::history_persistence) fn applied_base_to_wire(
    ledger: &AppliedBaseUnprovenLedgerV1,
) -> AppliedBaseUnprovenLedgerWireV1 {
    AppliedBaseUnprovenLedgerWireV1 {
        retained_marks: ledger
            .retained_marks
            .iter()
            .map(|item| AppliedBaseUnprovenMarkWireV1 {
                mark: mark_to_wire(&item.mark),
                subsequent_applied_entries: item.subsequent_applied_entries,
            })
            .collect(),
        collapsed_terminal: status_counts_to_wire(ledger.collapsed_terminal),
    }
}

pub(in crate::editor::history_persistence) fn applied_base_from_wire(
    ledger: AppliedBaseUnprovenLedgerWireV1,
) -> Result<AppliedBaseUnprovenLedgerV1, EditorHistoryErrorV1> {
    Ok(AppliedBaseUnprovenLedgerV1 {
        retained_marks: ledger
            .retained_marks
            .into_iter()
            .map(|item| {
                Ok(AppliedBaseUnprovenMarkV1 {
                    mark: mark_from_wire(item.mark)?,
                    subsequent_applied_entries: item.subsequent_applied_entries,
                })
            })
            .collect::<Result<Vec<_>, EditorHistoryErrorV1>>()?,
        collapsed_terminal: status_counts_from_wire(ledger.collapsed_terminal),
    })
}

fn validate_unproven_mark_for_entry(
    mark: &SpeculativeUnprovenFoldMarkV1,
    forward: &Command,
    inverse: &Inverse,
    project_id: ProjectId,
) -> Result<(), EditorHistoryErrorV1> {
    mark.binding
        .validate()
        .map_err(|_| EditorHistoryErrorV1::InvalidSpeculativeUnprovenMetadata)?;
    if mark.binding.project_id() != project_id
        || !matches!(
            mark.binding.approximate_blocking_observation(),
            SpeculativeApproximateBlockingObservationV1::NoBlockingSampleObserved
        )
    {
        return Err(EditorHistoryErrorV1::InvalidSpeculativeUnprovenMetadata);
    }
    let (
        Command::ApplyStackedFoldDocument { .. },
        Inverse::RestoreStackedFoldDocument { pattern, paper, .. },
    ) = (forward, inverse)
    else {
        return Err(EditorHistoryErrorV1::InvalidSpeculativeUnprovenMetadata);
    };
    if mark.binding.source_geometry_fingerprint_sha256()
        != crate::fold_model_fingerprint::fold_model_fingerprint_v1(pattern, paper)
        || mark.binding.paper_thickness_bits() != paper.thickness_mm.to_bits()
    {
        return Err(EditorHistoryErrorV1::InvalidSpeculativeUnprovenMetadata);
    }
    Ok(())
}

pub(in crate::editor::history_persistence) fn validate_editor_unproven_history(
    editor: &EditorState,
    project_id: ProjectId,
) -> Result<(), EditorHistoryErrorV1> {
    let ledger = &editor.applied_base_unproven;
    if ledger.retained_marks.len() > MAX_RETAINED_SPECULATIVE_UNPROVEN_BASE_MARKS_V1
        || ledger.collapsed_terminal.awaiting_proof != 0
        || ledger.collapsed_terminal.total() > MAX_REVISION
        || ledger
            .retained_marks
            .iter()
            .any(|item| item.subsequent_applied_entries > MAX_REVISION)
        || ledger
            .retained_marks
            .iter()
            .any(|item| item.subsequent_applied_entries < editor.undo_stack.len() as u64)
        || ledger
            .retained_marks
            .windows(2)
            .any(|pair| pair[0].subsequent_applied_entries <= pair[1].subsequent_applied_entries)
    {
        return Err(EditorHistoryErrorV1::InvalidSpeculativeAppliedBaseLedger);
    }

    let mut bindings = Vec::new();
    let mut pending = 0_usize;
    for item in &ledger.retained_marks {
        validate_base_mark(&item.mark, project_id)?;
        collect_binding(&item.mark, &mut bindings, &mut pending)?;
    }
    for entry in editor.undo_stack.iter().chain(&editor.redo_stack) {
        if let Some(mark) = &entry.speculative_unproven_fold {
            validate_unproven_mark_for_entry(mark, &entry.forward, &entry.inverse, project_id)?;
            collect_binding(mark, &mut bindings, &mut pending)?;
        }
    }
    if pending > MAX_PENDING_SPECULATIVE_UNPROVEN_FOLDS_V1 {
        return Err(EditorHistoryErrorV1::TooManyPendingSpeculativeEntries);
    }
    Ok(())
}

fn validate_base_mark(
    mark: &SpeculativeUnprovenFoldMarkV1,
    project_id: ProjectId,
) -> Result<(), EditorHistoryErrorV1> {
    mark.binding
        .validate()
        .map_err(|_| EditorHistoryErrorV1::InvalidSpeculativeUnprovenMetadata)?;
    if mark.binding.project_id() != project_id
        || !matches!(
            mark.binding.approximate_blocking_observation(),
            SpeculativeApproximateBlockingObservationV1::NoBlockingSampleObserved
        )
    {
        return Err(EditorHistoryErrorV1::InvalidSpeculativeUnprovenMetadata);
    }
    Ok(())
}

fn collect_binding(
    mark: &SpeculativeUnprovenFoldMarkV1,
    bindings: &mut Vec<SpeculativeUnprovenFoldBindingV1>,
    pending: &mut usize,
) -> Result<(), EditorHistoryErrorV1> {
    if bindings
        .iter()
        .any(|existing| existing.has_same_request_identity(&mark.binding))
    {
        return Err(EditorHistoryErrorV1::DuplicateSpeculativeBinding);
    }
    bindings.push(mark.binding.clone());
    if mark.status == SpeculativeUnprovenFoldStatusV1::AwaitingProof {
        *pending = pending
            .checked_add(1)
            .ok_or(EditorHistoryErrorV1::TooManyPendingSpeculativeEntries)?;
    }
    Ok(())
}
