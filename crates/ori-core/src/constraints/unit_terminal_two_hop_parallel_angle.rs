use super::*;

const MAX_WORK_V1: u64 = 40_000;
const MAX_STORAGE_UNITS_V1: usize = DEFAULT_MAX_CONSTRAINT_PRECHECKS * 2;

/// Finds only the exact five-record terminal-unit subset of the legacy
/// fixed-angle parallel-component candidate.
///
/// Write the fixed-angle terminal vectors as `a` and `b`, the distinct middle
/// vector as `m`, and let `u = 2^-53`. Exact zero unit-length residuals force
/// both pinned terminal hypotenuses to the binary64 value `1.0`. Consequently
/// each production parallel denominator is exactly `hypot(m)`: multiplication
/// by the other `1.0` operand is exact even when the middle hypot is
/// subnormal. A zero or non-finite middle hypot cannot make either division
/// residual zero.
///
/// For a normal middle hypot `h`, the zero-secondary-input branch is exact and
/// the exponent-gap branch has relative error below `2 * u`. Every remaining
/// pinned libm 0.2.16 scaling branch keeps the inputs to Dekker `sq` in
/// biased-exponent range 573 through 1533. The square high/low pairs are exact
/// there: `x * SPLIT` and every square stay finite, while the smallest possible
/// nonzero low square is `2^-1004` and remains normal. The low-to-high sum and
/// correctly rounded software square root therefore give the conservative
/// relative hypot bound `2^-40`. Replaying the two rounded products,
/// subtraction, and zero-producing division then bounds each real
/// terminal/middle determinant by `14 * u * h`. A product overflow or other
/// non-finite numerator cannot compare equal to zero after division by the
/// finite `h`. Both terminal vectors consequently have a production dot
/// bounded away from zero and lie on the same or opposite branch of one
/// unoriented line.
///
/// If `h` is subnormal, the pinned absolute hypot bound
/// `abs(h - |m|) < 2^-40 * |m| + 2^-1075` confines each middle component below
/// `(2^52 + 4096) * 2^-1074`. Write those components on the global
/// minimum-subnormal lattice. Any nonzero rounded cross numerator has
/// magnitude at least one lattice unit, and division by subnormal `h` cannot
/// round that quotient to zero. Both numerators are thus exact zero. Each
/// terminal product is below `(2^52 + 8193) * 2^-1074`, still less than twice
/// the minimum normal, where rounding is a uniform lattice nearest-integer
/// map. Each terminal determinant coefficient is at most one; integer middle
/// radii at least two give raw dot magnitude above `1/2 - 2^-39`, while the
/// only smaller radii are the separately bounded axis and diagonal cases.
/// After the explicitly bounded endpoint product/add roundings,
/// `abs(dot) > 0.49`, `abs(cross) < 1.01`, and every finite atan reduction
/// argument is below three. All pinned atan branches then return below 1.3
/// radians for positive dot or above `PI - 1.3` for negative dot, disjoint
/// from the frozen 90-degree zero enclosure
/// `0x3ff921fb54442d15..=0x3ff921fb54442d1b`, which lies strictly between 1.5
/// and 1.6 radians.
///
/// The scanner deliberately does not reuse a general shortest path. It emits
/// only two distinct unordered parallel pairs `{a, m}` and `{m, b}`, the
/// bit-exact 90-degree angle, and bit-exact unit lengths on both terminals.
/// The independent shape verifier below replays that exact five-ID grammar
/// before the candidate is admitted as a direct proof.
pub(super) fn conflict_v1(
    parallels: &BTreeMap<EdgePairKey, Vec<ConstraintId>>,
    fixed_angles: &BTreeMap<AngleKey, Vec<ScalarAssignment>>,
    fixed_lengths: &BTreeMap<CanonicalId, ScalarGroupSummary>,
    vertex_ids: &BTreeMap<CanonicalId, VertexId>,
    edge_ids: &BTreeMap<CanonicalId, EdgeId>,
    observer: &mut impl bounded_zero_closure::Observer,
) -> Result<Option<DirectConstraintConflictV1>, GeometricConstraintUnknownReasonV1> {
    #[cfg(test)]
    let max_work = TEST_WORK_LIMIT.with(|limit| limit.get().unwrap_or(MAX_WORK_V1));
    #[cfg(not(test))]
    let max_work = MAX_WORK_V1;
    #[cfg(test)]
    let max_storage = TEST_STORAGE_LIMIT.with(|limit| limit.get().unwrap_or(MAX_STORAGE_UNITS_V1));
    #[cfg(not(test))]
    let max_storage = MAX_STORAGE_UNITS_V1;
    #[cfg(test)]
    {
        TEST_WORK_OBSERVED.with(|observed| observed.set(0));
        TEST_STORAGE_OBSERVED.with(|observed| observed.set(0));
    }

    if let Some(reason) =
        preflight_observer_stop_reason(observer, bounded_zero_closure::Phase::ProofSearch, 0)
    {
        return Err(reason);
    }

    let mut work = 0_u64;
    let mut storage = 0_usize;
    let mut graph: BTreeMap<CanonicalId, BTreeMap<CanonicalId, ConstraintId>> = BTreeMap::new();
    for (pair, ids) in parallels {
        charge_work_v1(&mut work, max_work, 1, observer)?;
        if pair.first == pair.second {
            continue;
        }
        let Some(id) = ids.iter().min_by_key(|id| id.canonical_bytes()).copied() else {
            continue;
        };
        reserve_storage_v1(&mut storage, max_storage, 2)?;
        graph.entry(pair.first).or_default().insert(pair.second, id);
        graph.entry(pair.second).or_default().insert(pair.first, id);
    }

    let mut best: Option<DirectConstraintConflictV1> = None;
    for (key, assignments) in fixed_angles {
        charge_work_v1(&mut work, max_work, 1, observer)?;
        charge_work_v1(
            &mut work,
            max_work,
            u64::try_from(assignments.len())
                .map_err(|_| GeometricConstraintUnknownReasonV1::WorkLimitExceeded)?,
            observer,
        )?;
        let Some(angle) = assignments
            .iter()
            .filter(|assignment| assignment.value.to_bits() == 90.0_f64.to_bits())
            .min_by_key(|assignment| assignment.id.canonical_bytes())
        else {
            continue;
        };
        let first_edge = key.edges.first;
        let second_edge = key.edges.second;
        if first_edge == second_edge {
            continue;
        }
        let Some(first_unit) = fixed_lengths
            .get(&first_edge)
            .and_then(ScalarGroupSummary::consistent_assignment)
            .filter(|assignment| assignment.value.to_bits() == 1.0_f64.to_bits())
        else {
            continue;
        };
        let Some(second_unit) = fixed_lengths
            .get(&second_edge)
            .and_then(ScalarGroupSummary::consistent_assignment)
            .filter(|assignment| assignment.value.to_bits() == 1.0_f64.to_bits())
        else {
            continue;
        };

        for (middle_edge, first_parallel_id) in graph.get(&first_edge).into_iter().flatten() {
            charge_work_v1(&mut work, max_work, 1, observer)?;
            if *middle_edge == first_edge || *middle_edge == second_edge {
                continue;
            }
            let Some(second_parallel_id) = graph
                .get(&second_edge)
                .and_then(|neighbors| neighbors.get(middle_edge))
                .copied()
            else {
                continue;
            };
            if *first_parallel_id == second_parallel_id {
                continue;
            }

            let mut constraint_ids = vec![
                *first_parallel_id,
                second_parallel_id,
                angle.id,
                first_unit.id,
                second_unit.id,
            ];
            canonicalize_constraint_ids(&mut constraint_ids);
            if constraint_ids.len() != 5 {
                continue;
            }
            let candidate = DirectConstraintConflictV1 {
                conflict:
                    DirectConstraintConflictKindV1::NonParallelFixedAngleInParallelComponent {
                        vertex: vertex_ids[&key.vertex],
                        first_edge: edge_ids[&first_edge],
                        second_edge: edge_ids[&second_edge],
                        parallel_constraint_count: 2,
                    },
                constraint_ids,
            };
            if best.as_ref().is_none_or(|current| {
                conflict_sort_key(&candidate.conflict)
                    .cmp(&conflict_sort_key(&current.conflict))
                    .then_with(|| {
                        canonical_id_slice_cmp(&candidate.constraint_ids, &current.constraint_ids)
                    })
                    .is_lt()
            }) {
                best = Some(candidate);
            }
        }
    }

    if let Some(reason) =
        preflight_observer_stop_reason(observer, bounded_zero_closure::Phase::ProofSearch, work)
    {
        return Err(reason);
    }
    Ok(best)
}

fn charge_work_v1(
    work: &mut u64,
    maximum: u64,
    amount: u64,
    observer: &mut impl bounded_zero_closure::Observer,
) -> Result<(), GeometricConstraintUnknownReasonV1> {
    let previous = *work;
    *work = work
        .checked_add(amount)
        .ok_or(GeometricConstraintUnknownReasonV1::WorkLimitExceeded)?;
    #[cfg(test)]
    TEST_WORK_OBSERVED.with(|observed| observed.set(*work));
    if *work > maximum {
        return Err(GeometricConstraintUnknownReasonV1::WorkLimitExceeded);
    }
    if previous / 128 != *work / 128
        && let Some(reason) = preflight_observer_stop_reason(
            observer,
            bounded_zero_closure::Phase::ProofSearch,
            *work,
        )
    {
        return Err(reason);
    }
    Ok(())
}

fn reserve_storage_v1(
    storage: &mut usize,
    maximum: usize,
    amount: usize,
) -> Result<(), GeometricConstraintUnknownReasonV1> {
    *storage = storage
        .checked_add(amount)
        .ok_or(GeometricConstraintUnknownReasonV1::StorageLimitExceeded)?;
    #[cfg(test)]
    TEST_STORAGE_OBSERVED.with(|observed| observed.set(*storage));
    (*storage <= maximum)
        .then_some(())
        .ok_or(GeometricConstraintUnknownReasonV1::StorageLimitExceeded)
}

/// Revalidates the complete grammar needed by the terminal-unit
/// fixed-angle/parallel proof.
///
/// Candidate construction and proof admission are intentionally independent:
/// a legacy general graph candidate carrying the same wire tag has only its
/// angle and path records, so it cannot pass this exact five-record parse.
pub(super) fn is_proven_shape_v1(
    candidate: &DirectConstraintConflictV1,
    records: &[GeometricConstraintRecordV1],
) -> bool {
    let DirectConstraintConflictKindV1::NonParallelFixedAngleInParallelComponent {
        vertex: reported_vertex,
        first_edge: reported_first,
        second_edge: reported_second,
        parallel_constraint_count,
    } = &candidate.conflict
    else {
        return false;
    };
    let first_key = reported_first.canonical_bytes();
    let second_key = reported_second.canonical_bytes();
    if *parallel_constraint_count != 2
        || candidate.constraint_ids.len() != 5
        || first_key >= second_key
        || !candidate
            .constraint_ids
            .windows(2)
            .all(|pair| pair[0].canonical_bytes() < pair[1].canonical_bytes())
    {
        return false;
    }

    let cause_ids = candidate
        .constraint_ids
        .iter()
        .map(ConstraintId::canonical_bytes)
        .collect::<BTreeSet<_>>();
    if cause_ids.len() != 5 {
        return false;
    }
    let selected = records
        .iter()
        .filter(|record| cause_ids.contains(&record.id.canonical_bytes()))
        .collect::<Vec<_>>();
    if selected.len() != 5 {
        return false;
    }

    let reported_pair = EdgePairKey {
        first: first_key,
        second: second_key,
    };
    let mut saw_angle = false;
    let mut fixed_edges = Vec::new();
    let mut parallel_pairs = Vec::new();
    for record in selected {
        match &record.constraint {
            GeometricConstraintKindV1::FixedAngle {
                vertex,
                first_edge,
                second_edge,
                angle_degrees,
            } if !saw_angle
                && *vertex == *reported_vertex
                && angle_degrees.to_bits() == 90.0_f64.to_bits()
                && EdgePairKey::unordered(*first_edge, *second_edge) == reported_pair =>
            {
                // Full preparation has already validated that this vertex is
                // common to the two named edges. The verifier still binds the
                // exact record roles and scalar bits to the reported payload.
                saw_angle = true;
            }
            GeometricConstraintKindV1::FixedLength { edge, length_mm }
                if fixed_edges.len() < 2 && length_mm.to_bits() == 1.0_f64.to_bits() =>
            {
                fixed_edges.push(edge.canonical_bytes());
            }
            GeometricConstraintKindV1::Parallel {
                first_edge,
                second_edge,
            } if parallel_pairs.len() < 2 => {
                parallel_pairs.push(EdgePairKey::unordered(*first_edge, *second_edge));
            }
            _ => return false,
        }
    }
    if !saw_angle || fixed_edges.len() != 2 || parallel_pairs.len() != 2 {
        return false;
    }
    fixed_edges.sort_unstable();
    if fixed_edges != [first_key, second_key] {
        return false;
    }

    let parallel_pairs = parallel_pairs.into_iter().collect::<BTreeSet<_>>();
    if parallel_pairs.len() != 2 {
        return false;
    }
    let all_edges = parallel_pairs
        .iter()
        .flat_map(|pair| [pair.first, pair.second])
        .collect::<BTreeSet<_>>();
    if all_edges.len() != 3 || !all_edges.contains(&first_key) || !all_edges.contains(&second_key) {
        return false;
    }
    let Some(middle) = all_edges
        .into_iter()
        .find(|edge| *edge != first_key && *edge != second_key)
    else {
        return false;
    };
    let first_parallel = EdgePairKey {
        first: first_key.min(middle),
        second: first_key.max(middle),
    };
    let second_parallel = EdgePairKey {
        first: middle.min(second_key),
        second: middle.max(second_key),
    };
    parallel_pairs == BTreeSet::from([first_parallel, second_parallel])
}

#[cfg(test)]
std::thread_local! {
    static TEST_WORK_LIMIT: std::cell::Cell<Option<u64>> = const {
        std::cell::Cell::new(None)
    };
    static TEST_WORK_OBSERVED: std::cell::Cell<u64> = const {
        std::cell::Cell::new(0)
    };
    static TEST_STORAGE_LIMIT: std::cell::Cell<Option<usize>> = const {
        std::cell::Cell::new(None)
    };
    static TEST_STORAGE_OBSERVED: std::cell::Cell<usize> = const {
        std::cell::Cell::new(0)
    };
}

#[cfg(test)]
pub(crate) fn replace_test_limits_v1(
    limits: (Option<u64>, Option<usize>),
) -> (Option<u64>, Option<usize>) {
    let previous_work = TEST_WORK_LIMIT.with(|slot| slot.replace(limits.0));
    let previous_storage = TEST_STORAGE_LIMIT.with(|slot| slot.replace(limits.1));
    (previous_work, previous_storage)
}

#[cfg(test)]
pub(crate) fn test_observed_v1() -> (u64, usize) {
    (
        TEST_WORK_OBSERVED.with(std::cell::Cell::get),
        TEST_STORAGE_OBSERVED.with(std::cell::Cell::get),
    )
}

#[cfg(test)]
pub(crate) fn charge_work_for_test_v1(
    work: &mut u64,
    maximum: u64,
    amount: u64,
) -> Result<(), GeometricConstraintUnknownReasonV1> {
    charge_work_v1(
        work,
        maximum,
        amount,
        &mut bounded_zero_closure::NoopObserver,
    )
}

#[cfg(test)]
pub(crate) fn reserve_storage_for_test_v1(
    storage: &mut usize,
    maximum: usize,
    amount: usize,
) -> Result<(), GeometricConstraintUnknownReasonV1> {
    reserve_storage_v1(storage, maximum, amount)
}
