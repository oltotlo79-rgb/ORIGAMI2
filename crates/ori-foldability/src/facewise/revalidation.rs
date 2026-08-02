use super::*;

pub(crate) struct FacewiseLayerOrderRevalidationInputV2<'a> {
    pub(crate) paper: &'a Paper,
    pub(crate) crease_pattern: &'a CreasePattern,
    pub(crate) topology: &'a TopologySnapshot,
    pub(crate) canonical_faces: &'a [LayerFace],
    pub(crate) provenance: GlobalFlatFoldabilityProvenance,
    pub(crate) work_counts: GlobalFlatFoldabilityWorkCounts,
    pub(crate) limits: GlobalFlatFoldabilityLimits,
    pub(crate) snapshot: &'a LayerOrderSnapshot,
    pub(crate) borrowed_live_bytes: usize,
    pub(crate) max_peak_bytes: usize,
}

pub(crate) struct FacewiseLayerOrderRevalidationSuccessV2 {
    pub(crate) work_counts: GlobalFlatFoldabilityWorkCounts,
    pub(crate) borrowed_live_bytes: usize,
    pub(crate) observed_peak_bytes: usize,
    pub(crate) observed_facewise_peak_bytes: usize,
    pub(crate) observed_validation_peak_bytes: usize,
}

pub(crate) enum FacewiseLayerOrderRevalidationFailureV2 {
    Inconclusive(GlobalFlatFoldabilityUnknownReason),
    LiveSourceImpossible,
    CertificateMismatch,
    Execution(GlobalFlatFoldabilityExecutionError),
}

pub(crate) fn revalidate_layer_order_snapshot_v2<O: GlobalFlatFoldabilityObserver + ?Sized>(
    input: FacewiseLayerOrderRevalidationInputV2<'_>,
    observer: &mut O,
) -> Result<FacewiseLayerOrderRevalidationSuccessV2, FacewiseLayerOrderRevalidationFailureV2> {
    let FacewiseLayerOrderRevalidationInputV2 {
        paper,
        crease_pattern,
        topology,
        canonical_faces,
        provenance,
        work_counts,
        limits,
        snapshot,
        borrowed_live_bytes,
        max_peak_bytes,
    } = input;
    let mut runtime = Runtime::new_revalidation(
        observer,
        limits,
        work_counts,
        borrowed_live_bytes,
        max_peak_bytes,
    )
    .map_err(map_revalidation_abort)?;
    let result = revalidate_snapshot_inner(
        paper,
        crease_pattern,
        topology,
        canonical_faces,
        provenance,
        snapshot,
        &mut runtime,
    );
    match result {
        Ok(()) => {
            complete_progress(runtime.observer, runtime.work);
            Ok(FacewiseLayerOrderRevalidationSuccessV2 {
                work_counts: runtime.work,
                borrowed_live_bytes,
                observed_peak_bytes: runtime.observed_peak_bytes(),
                observed_facewise_peak_bytes: runtime.observed_peak_bytes(),
                observed_validation_peak_bytes: 0,
            })
        }
        Err(abort) => {
            complete_progress(runtime.observer, runtime.work);
            Err(map_revalidation_abort(abort))
        }
    }
}

fn map_revalidation_abort(abort: FacewiseAbort) -> FacewiseLayerOrderRevalidationFailureV2 {
    match abort {
        FacewiseAbort::Unknown(GlobalFlatFoldabilityUnknownReason::ProofIncomplete {
            reason: FlatFoldabilityProofIncompleteReason::CertificateReverificationFailed,
        })
        | FacewiseAbort::RequiredLayerOrder(_) => {
            FacewiseLayerOrderRevalidationFailureV2::CertificateMismatch
        }
        FacewiseAbort::Unknown(reason) => {
            FacewiseLayerOrderRevalidationFailureV2::Inconclusive(reason)
        }
        FacewiseAbort::Impossible(_) => {
            FacewiseLayerOrderRevalidationFailureV2::LiveSourceImpossible
        }
        FacewiseAbort::Execution(error) => {
            FacewiseLayerOrderRevalidationFailureV2::Execution(error)
        }
    }
}

fn revalidate_snapshot_inner<O: GlobalFlatFoldabilityObserver + ?Sized>(
    paper: &Paper,
    crease_pattern: &CreasePattern,
    topology: &TopologySnapshot,
    canonical_faces: &[LayerFace],
    provenance: GlobalFlatFoldabilityProvenance,
    snapshot: &LayerOrderSnapshot,
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<()> {
    runtime.advance(
        GlobalFlatFoldabilityPhase::BuildingFlatEmbedding,
        Some(canonical_faces.len()),
    )?;
    let embedding =
        build_flat_embedding(paper, crease_pattern, topology, canonical_faces, runtime)?;
    runtime.advance(GlobalFlatFoldabilityPhase::BuildingOverlapArrangement, None)?;
    let pairs = build_overlap_pairs(&embedding.faces, runtime)?;
    runtime.set_overlap_pairs(pairs.len())?;
    let cells = build_overlap_cells(&embedding.faces, &pairs, runtime)?;
    runtime.set_overlap_cells(cells.len())?;
    runtime.advance(GlobalFlatFoldabilityPhase::BuildingConstraints, None)?;
    let problem = build_constraint_problem(&embedding, &pairs, &cells, runtime, true)?;
    if problem.variables.len() != runtime.work.overlap_face_pairs
        || problem.constraints.len() != runtime.work.constraints
    {
        return Err(certificate_failure());
    }
    let assignment = decode_snapshot_assignment(snapshot, &embedding, &problem, runtime)?;
    runtime.advance(
        GlobalFlatFoldabilityPhase::VerifyingCertificate,
        Some(problem.constraints.len()),
    )?;
    verify_facewise_certificate(&embedding, &pairs, &cells, &problem, &assignment, runtime)?;

    runtime.add_certificate_structure_storage(runtime.allocation_bytes(
        problem.variables.len(),
        std::mem::size_of::<((usize, usize), bool)>(),
    )?)?;
    let pair_values = checkpointed_pair_values(&problem.variables, &assignment, runtime)?;
    drop(assignment);
    drop(problem);
    runtime.clear_constraint_storage();

    let Some(summary) = snapshot.proof_summary else {
        return Err(certificate_failure());
    };
    runtime.set_certificate_bytes(summary.certificate_bytes)?;
    verify_revalidated_layer_order_snapshot(
        snapshot,
        &embedding,
        &cells,
        &pair_values,
        provenance,
        runtime,
    )?;
    runtime.checkpoint(None)
}

fn decode_snapshot_assignment<O: GlobalFlatFoldabilityObserver + ?Sized>(
    snapshot: &LayerOrderSnapshot,
    embedding: &FlatEmbedding,
    problem: &ConstraintProblem,
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<Vec<bool>> {
    if snapshot.face_pair_orders.len() != problem.variables.len() {
        return Err(certificate_failure());
    }
    for adjacent in embedding.faces.windows(2) {
        runtime.checkpoint(None)?;
        let first = adjacent[0].source.layer;
        let second = adjacent[1].source.layer;
        if (first.face_key, first.face_id.canonical_bytes())
            >= (second.face_key, second.face_id.canonical_bytes())
        {
            return Err(certificate_failure());
        }
    }
    let assignment_bytes =
        runtime.allocation_bytes(problem.variables.len(), std::mem::size_of::<bool>())?;
    let seen_bytes =
        runtime.allocation_bytes(problem.variables.len(), std::mem::size_of::<u8>())?;
    runtime.add_constraint_storage(
        assignment_bytes
            .checked_add(seen_bytes)
            .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?,
    )?;
    let mut assignment = Vec::new();
    assignment
        .try_reserve_exact(problem.variables.len())
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    checkpointed_extend(
        &mut assignment,
        (0..problem.variables.len()).map(|_| false),
        runtime,
    )?;
    let mut seen = Vec::new();
    seen.try_reserve_exact(problem.variables.len())
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    checkpointed_extend(
        &mut seen,
        (0..problem.variables.len()).map(|_| 0_u8),
        runtime,
    )?;

    for order in &snapshot.face_pair_orders {
        runtime.checkpoint(None)?;
        let Some(lower) = trusted_required_face_index(embedding, order.lower_face) else {
            return Err(certificate_failure());
        };
        let Some(upper) = trusted_required_face_index(embedding, order.upper_face) else {
            return Err(certificate_failure());
        };
        if lower == upper {
            return Err(certificate_failure());
        }
        let pair = ordered_pair(lower, upper);
        let Ok(variable) = problem.variables.binary_search(&pair) else {
            return Err(certificate_failure());
        };
        if seen[variable] != 0 {
            return Err(certificate_failure());
        }
        seen[variable] = 1;
        assignment[variable] = upper == pair.1;
    }
    validate_assignment_completeness(&seen, &problem.fixed_assignments, &assignment, runtime)?;
    drop(seen);
    runtime.release_constraint_storage(seen_bytes)?;
    runtime.checkpoint(None)?;
    Ok(assignment)
}

fn validate_assignment_completeness<O: GlobalFlatFoldabilityObserver + ?Sized>(
    seen: &[u8],
    fixed_assignments: &[Option<bool>],
    assignment: &[bool],
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<()> {
    if seen.len() != assignment.len() || fixed_assignments.len() != assignment.len() {
        return Err(certificate_failure());
    }
    for value in seen {
        runtime.checkpoint(None)?;
        if *value == 0 {
            return Err(certificate_failure());
        }
    }
    for (fixed, value) in fixed_assignments.iter().zip(assignment) {
        runtime.checkpoint(None)?;
        if fixed.is_some_and(|fixed| fixed != *value) {
            return Err(certificate_failure());
        }
    }
    runtime.checkpoint(None)
}

fn checkpointed_pair_values<O: GlobalFlatFoldabilityObserver + ?Sized>(
    variables: &[(usize, usize)],
    assignment: &[bool],
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<PairValues> {
    if variables.len() != assignment.len() {
        return Err(certificate_failure());
    }
    let mut values = Vec::new();
    values
        .try_reserve_exact(variables.len())
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    for (variable, value) in variables.iter().copied().zip(assignment.iter().copied()) {
        runtime.checkpoint(None)?;
        values.push((variable, value));
    }
    runtime.checkpoint(None)?;
    Ok(PairValues(values))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct StopAfter {
        remaining: usize,
        stop: GlobalFlatFoldabilityCheckpoint,
    }

    impl GlobalFlatFoldabilityObserver for StopAfter {
        fn checkpoint(&mut self) -> GlobalFlatFoldabilityCheckpoint {
            if self.remaining == 0 {
                self.stop
            } else {
                self.remaining -= 1;
                GlobalFlatFoldabilityCheckpoint::Continue
            }
        }
    }

    #[test]
    fn assignment_completeness_scans_can_stop_mid_seen_and_fixed_passes() {
        const RECORDS: usize = 4_096;
        let seen = vec![1_u8; RECORDS];
        let fixed = vec![None; RECORDS];
        let assignment = vec![false; RECORDS];

        let mut cancel = StopAfter {
            remaining: 128,
            stop: GlobalFlatFoldabilityCheckpoint::Cancelled,
        };
        let mut runtime = Runtime::new_revalidation(
            &mut cancel,
            GlobalFlatFoldabilityLimits::default(),
            GlobalFlatFoldabilityWorkCounts::default(),
            0,
            usize::MAX,
        )
        .expect("unbounded test runtime");
        assert!(matches!(
            validate_assignment_completeness(&seen, &fixed, &assignment, &mut runtime),
            Err(FacewiseAbort::Execution(
                GlobalFlatFoldabilityExecutionError::Cancelled
            ))
        ));

        let mut deadline = StopAfter {
            remaining: RECORDS + 128,
            stop: GlobalFlatFoldabilityCheckpoint::DeadlineReached,
        };
        let mut runtime = Runtime::new_revalidation(
            &mut deadline,
            GlobalFlatFoldabilityLimits::default(),
            GlobalFlatFoldabilityWorkCounts::default(),
            0,
            usize::MAX,
        )
        .expect("unbounded test runtime");
        assert!(matches!(
            validate_assignment_completeness(&seen, &fixed, &assignment, &mut runtime),
            Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::TimeLimitReached {
                    phase: GlobalFlatFoldabilityPhase::ValidatingLocalConditions
                }
            ))
        ));
    }
}
