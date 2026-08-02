use super::*;

type CanonicalLayerFaceKeyV2 = ([u8; 32], [u8; 16]);
type CanonicalPairRegistryKeyV2 = (CanonicalLayerFaceKeyV2, CanonicalLayerFaceKeyV2);

pub(crate) struct FacewiseCompactPairAssignmentInputV2<'a> {
    pub(crate) paper: &'a Paper,
    pub(crate) crease_pattern: &'a CreasePattern,
    pub(crate) topology: &'a TopologySnapshot,
    pub(crate) canonical_faces: &'a [LayerFace],
    pub(crate) provenance: GlobalFlatFoldabilityProvenance,
    pub(crate) work_counts: GlobalFlatFoldabilityWorkCounts,
    pub(crate) limits: GlobalFlatFoldabilityLimits,
    pub(crate) variable_count: usize,
    pub(crate) variable_registry_sha256: [u8; 32],
    pub(crate) direction_bits_le: &'a [u8],
    pub(crate) borrowed_live_bytes: usize,
    pub(crate) max_peak_bytes: usize,
}

pub(crate) struct FacewiseCompactPairAssignmentSuccessV2 {
    pub(crate) layer_order: LayerOrderSnapshot,
    pub(crate) work_counts: GlobalFlatFoldabilityWorkCounts,
    pub(crate) observed_peak_bytes: usize,
}

pub(crate) enum FacewiseCompactPairAssignmentFailureV2 {
    Inconclusive(GlobalFlatFoldabilityUnknownReason),
    LiveSourceImpossible,
    RegistryMismatch,
    MalformedAssignment,
    AssignmentRejected,
    Execution(GlobalFlatFoldabilityExecutionError),
}

#[derive(Clone, Copy)]
struct CanonicalPairRegistryEntryV2 {
    first: LayerFace,
    second: LayerFace,
    problem_variable: usize,
    canonical_first_is_problem_first: bool,
}

struct CanonicalPairRegistryV2 {
    entries: Vec<CanonicalPairRegistryEntryV2>,
    digest: [u8; 32],
    storage_bytes: usize,
}

pub(crate) fn reconstruct_layer_order_from_compact_pair_assignment_v2<
    O: GlobalFlatFoldabilityObserver + ?Sized,
>(
    input: FacewiseCompactPairAssignmentInputV2<'_>,
    observer: &mut O,
) -> Result<FacewiseCompactPairAssignmentSuccessV2, FacewiseCompactPairAssignmentFailureV2> {
    let FacewiseCompactPairAssignmentInputV2 {
        paper,
        crease_pattern,
        topology,
        canonical_faces,
        provenance,
        work_counts,
        limits,
        variable_count,
        variable_registry_sha256,
        direction_bits_le,
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
    .map_err(map_compact_abort)?;
    let result = reconstruct_compact_assignment_inner_v2(
        paper,
        crease_pattern,
        topology,
        canonical_faces,
        provenance,
        variable_count,
        variable_registry_sha256,
        direction_bits_le,
        &mut runtime,
    );
    match result {
        Ok(layer_order) => {
            complete_progress(runtime.observer, runtime.work);
            debug_assert_eq!(runtime.work.search_nodes, 0);
            Ok(FacewiseCompactPairAssignmentSuccessV2 {
                layer_order,
                work_counts: runtime.work,
                observed_peak_bytes: runtime.observed_peak_bytes(),
            })
        }
        Err(failure) => {
            complete_progress(runtime.observer, runtime.work);
            Err(failure)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn reconstruct_compact_assignment_inner_v2<O: GlobalFlatFoldabilityObserver + ?Sized>(
    paper: &Paper,
    crease_pattern: &CreasePattern,
    topology: &TopologySnapshot,
    canonical_faces: &[LayerFace],
    provenance: GlobalFlatFoldabilityProvenance,
    expected_variable_count: usize,
    expected_registry_digest: [u8; 32],
    direction_bits_le: &[u8],
    runtime: &mut Runtime<'_, O>,
) -> Result<LayerOrderSnapshot, FacewiseCompactPairAssignmentFailureV2> {
    runtime
        .advance(
            GlobalFlatFoldabilityPhase::BuildingFlatEmbedding,
            Some(canonical_faces.len()),
        )
        .map_err(map_compact_abort)?;
    let embedding = build_flat_embedding(paper, crease_pattern, topology, canonical_faces, runtime)
        .map_err(map_compact_abort)?;
    runtime
        .advance(GlobalFlatFoldabilityPhase::BuildingOverlapArrangement, None)
        .map_err(map_compact_abort)?;
    let pairs = build_overlap_pairs(&embedding.faces, runtime).map_err(map_compact_abort)?;
    runtime
        .set_overlap_pairs(pairs.len())
        .map_err(map_compact_abort)?;
    let cells =
        build_overlap_cells(&embedding.faces, &pairs, runtime).map_err(map_compact_abort)?;
    runtime
        .set_overlap_cells(cells.len())
        .map_err(map_compact_abort)?;
    runtime
        .advance(GlobalFlatFoldabilityPhase::BuildingConstraints, None)
        .map_err(map_compact_abort)?;
    let problem = build_constraint_problem(&embedding, &pairs, &cells, runtime, true)
        .map_err(map_compact_abort)?;
    if problem.variables.len() != runtime.work.overlap_face_pairs
        || problem.constraints.len() != runtime.work.constraints
    {
        return Err(FacewiseCompactPairAssignmentFailureV2::Execution(
            internal_error(),
        ));
    }

    let registry =
        canonical_pair_registry_v2(&embedding, &problem, runtime).map_err(map_compact_abort)?;
    if expected_variable_count != registry.entries.len()
        || expected_registry_digest != registry.digest
    {
        return Err(FacewiseCompactPairAssignmentFailureV2::RegistryMismatch);
    }
    let expected_bytes = compact_assignment_byte_len_v2(expected_variable_count)
        .ok_or(FacewiseCompactPairAssignmentFailureV2::MalformedAssignment)?;
    if direction_bits_le.len() != expected_bytes
        || compact_assignment_has_nonzero_tail_v2(direction_bits_le, expected_variable_count)
    {
        return Err(FacewiseCompactPairAssignmentFailureV2::MalformedAssignment);
    }

    let requested_assignment_bytes = runtime
        .allocation_bytes(problem.variables.len(), std::mem::size_of::<bool>())
        .map_err(map_compact_abort)?;
    runtime
        .add_constraint_storage(requested_assignment_bytes)
        .map_err(map_compact_abort)?;
    let mut assignment = Vec::new();
    assignment
        .try_reserve_exact(problem.variables.len())
        .map_err(|_| map_compact_abort(runtime.exact_storage_limit_failure(usize::MAX)))?;
    let actual_assignment_bytes = runtime
        .allocation_bytes(assignment.capacity(), std::mem::size_of::<bool>())
        .map_err(map_compact_abort)?;
    if actual_assignment_bytes > requested_assignment_bytes {
        runtime
            .add_constraint_storage(actual_assignment_bytes - requested_assignment_bytes)
            .map_err(map_compact_abort)?;
    }
    checkpointed_extend(
        &mut assignment,
        (0..problem.variables.len()).map(|_| false),
        runtime,
    )
    .map_err(map_compact_abort)?;
    for (registry_index, entry) in registry.entries.iter().enumerate() {
        runtime.checkpoint(None).map_err(map_compact_abort)?;
        let canonical_first_is_lower =
            direction_bits_le[registry_index / 8] & (1_u8 << (registry_index % 8)) != 0;
        assignment[entry.problem_variable] = if entry.canonical_first_is_problem_first {
            canonical_first_is_lower
        } else {
            !canonical_first_is_lower
        };
    }
    let registry_storage_bytes = registry.storage_bytes;
    drop(registry);
    runtime
        .release_constraint_storage(registry_storage_bytes)
        .map_err(map_compact_abort)?;

    runtime
        .advance(
            GlobalFlatFoldabilityPhase::VerifyingCertificate,
            Some(problem.constraints.len()),
        )
        .map_err(map_compact_abort)?;
    verify_facewise_certificate(&embedding, &pairs, &cells, &problem, &assignment, runtime)
        .map_err(map_compact_abort)?;
    if runtime.work.search_nodes != 0 {
        return Err(FacewiseCompactPairAssignmentFailureV2::Execution(
            internal_error(),
        ));
    }

    runtime
        .add_certificate_structure_storage(
            runtime
                .allocation_bytes(
                    problem.variables.len(),
                    std::mem::size_of::<((usize, usize), bool)>(),
                )
                .map_err(map_compact_abort)?,
        )
        .map_err(map_compact_abort)?;
    let pair_values = PairValues::try_from_parallel(&problem.variables, &assignment, runtime)
        .map_err(map_compact_abort)?;
    drop(assignment);
    drop(problem);
    runtime.clear_constraint_storage();
    build_layer_order_snapshot(&embedding, &cells, &pair_values, provenance, runtime)
        .map_err(map_compact_abort)
}

fn canonical_pair_registry_v2<O: GlobalFlatFoldabilityObserver + ?Sized>(
    embedding: &FlatEmbedding,
    problem: &ConstraintProblem,
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<CanonicalPairRegistryV2> {
    let requested_bytes = runtime.allocation_bytes(
        problem.variables.len(),
        std::mem::size_of::<CanonicalPairRegistryEntryV2>(),
    )?;
    runtime.add_constraint_storage(requested_bytes)?;
    let mut entries = Vec::new();
    entries
        .try_reserve_exact(problem.variables.len())
        .map_err(|_| runtime.exact_storage_limit_failure(usize::MAX))?;
    let actual_bytes = runtime.allocation_bytes(
        entries.capacity(),
        std::mem::size_of::<CanonicalPairRegistryEntryV2>(),
    )?;
    if actual_bytes > requested_bytes {
        runtime.add_constraint_storage(actual_bytes - requested_bytes)?;
    }
    for (problem_variable, &(problem_first, problem_second)) in problem.variables.iter().enumerate()
    {
        runtime.checkpoint(None)?;
        let Some(first) = embedding
            .faces
            .get(problem_first)
            .map(|face| face.source.layer)
        else {
            return Err(FacewiseAbort::Execution(internal_error()));
        };
        let Some(second) = embedding
            .faces
            .get(problem_second)
            .map(|face| face.source.layer)
        else {
            return Err(FacewiseAbort::Execution(internal_error()));
        };
        let first_key = canonical_layer_face_key_v2(first);
        let second_key = canonical_layer_face_key_v2(second);
        if first_key == second_key {
            return Err(FacewiseAbort::Execution(internal_error()));
        }
        let (first, second, canonical_first_is_problem_first) = if first_key < second_key {
            (first, second, true)
        } else {
            (second, first, false)
        };
        entries.push(CanonicalPairRegistryEntryV2 {
            first,
            second,
            problem_variable,
            canonical_first_is_problem_first,
        });
    }
    let digest = canonicalize_and_hash_pair_registry_entries_v2(&mut entries, runtime)?;
    Ok(CanonicalPairRegistryV2 {
        entries,
        digest,
        storage_bytes: actual_bytes,
    })
}

fn canonicalize_and_hash_pair_registry_entries_v2<O: GlobalFlatFoldabilityObserver + ?Sized>(
    entries: &mut [CanonicalPairRegistryEntryV2],
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<[u8; 32]> {
    checkpointed_sort_unstable_by(entries, runtime, |first, second| {
        canonical_pair_registry_entry_key_v2(first)
            .cmp(&canonical_pair_registry_entry_key_v2(second))
    })?;
    for adjacent in entries.windows(2) {
        runtime.checkpoint(None)?;
        if canonical_pair_registry_entry_key_v2(&adjacent[0])
            >= canonical_pair_registry_entry_key_v2(&adjacent[1])
        {
            return Err(FacewiseAbort::Execution(internal_error()));
        }
    }
    let count = u64::try_from(entries.len())
        .map_err(|_| runtime.exact_storage_limit_failure(usize::MAX))?;
    let mut hash = Sha256::new();
    hash.update(crate::GLOBAL_FLAT_LAYER_ORDER_PAIR_REGISTRY_DOMAIN_V2);
    hash.update(count.to_le_bytes());
    for entry in entries.iter() {
        runtime.checkpoint(None)?;
        hash.update(entry.first.face_key.0);
        hash.update(entry.first.face_id.canonical_bytes());
        hash.update(entry.second.face_key.0);
        hash.update(entry.second.face_id.canonical_bytes());
    }
    runtime.checkpoint(None)?;
    Ok(hash.finalize().into())
}

fn canonical_layer_face_key_v2(face: LayerFace) -> CanonicalLayerFaceKeyV2 {
    (face.face_key.0, face.face_id.canonical_bytes())
}

fn canonical_pair_registry_entry_key_v2(
    entry: &CanonicalPairRegistryEntryV2,
) -> CanonicalPairRegistryKeyV2 {
    (
        canonical_layer_face_key_v2(entry.first),
        canonical_layer_face_key_v2(entry.second),
    )
}

pub(crate) const fn compact_assignment_byte_len_v2(variable_count: usize) -> Option<usize> {
    (variable_count / 8).checked_add((!variable_count.is_multiple_of(8)) as usize)
}

pub(crate) fn compact_assignment_has_nonzero_tail_v2(
    direction_bits_le: &[u8],
    variable_count: usize,
) -> bool {
    let remainder = variable_count % 8;
    remainder != 0
        && direction_bits_le
            .last()
            .is_none_or(|last| *last & (u8::MAX << remainder) != 0)
}

fn map_compact_abort(abort: FacewiseAbort) -> FacewiseCompactPairAssignmentFailureV2 {
    match abort {
        FacewiseAbort::Unknown(GlobalFlatFoldabilityUnknownReason::ProofIncomplete {
            reason: FlatFoldabilityProofIncompleteReason::CertificateReverificationFailed,
        })
        | FacewiseAbort::RequiredLayerOrder(_) => {
            FacewiseCompactPairAssignmentFailureV2::AssignmentRejected
        }
        FacewiseAbort::Unknown(reason) => {
            FacewiseCompactPairAssignmentFailureV2::Inconclusive(reason)
        }
        FacewiseAbort::Impossible(_) => {
            FacewiseCompactPairAssignmentFailureV2::LiveSourceImpossible
        }
        FacewiseAbort::Execution(error) => FacewiseCompactPairAssignmentFailureV2::Execution(error),
    }
}

#[cfg(test)]
pub(crate) fn compact_assignment_from_snapshot_for_test_v2(
    snapshot: &LayerOrderSnapshot,
) -> (usize, [u8; 32], Vec<u8>) {
    let mut observer = crate::NoopGlobalFlatFoldabilityObserver;
    let mut runtime = Runtime::new(
        &mut observer,
        GlobalFlatFoldabilityLimits::default(),
        GlobalFlatFoldabilityWorkCounts::default(),
    );
    let mut entries = Vec::with_capacity(snapshot.face_pair_orders.len());
    for (problem_variable, order) in snapshot.face_pair_orders.iter().enumerate() {
        let lower_key = canonical_layer_face_key_v2(order.lower_face);
        let upper_key = canonical_layer_face_key_v2(order.upper_face);
        assert_ne!(lower_key, upper_key);
        let (first, second, canonical_first_is_lower) = if lower_key < upper_key {
            (order.lower_face, order.upper_face, true)
        } else {
            (order.upper_face, order.lower_face, false)
        };
        entries.push(CanonicalPairRegistryEntryV2 {
            first,
            second,
            problem_variable,
            canonical_first_is_problem_first: canonical_first_is_lower,
        });
    }
    let digest = canonicalize_and_hash_pair_registry_entries_v2(&mut entries, &mut runtime)
        .expect("test snapshot registry is canonicalizable");
    let mut bits = vec![0_u8; compact_assignment_byte_len_v2(entries.len()).unwrap()];
    for (index, entry) in entries.iter().enumerate() {
        if entry.canonical_first_is_problem_first {
            bits[index / 8] |= 1_u8 << (index % 8);
        }
    }
    (entries.len(), digest, bits)
}
