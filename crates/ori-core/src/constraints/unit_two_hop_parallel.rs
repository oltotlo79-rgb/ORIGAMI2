use super::*;

const MAX_WORK_V1: u64 = 40_000;
const MAX_STORAGE_UNITS_V1: usize = DEFAULT_MAX_CONSTRAINT_PRECHECKS * 2;

#[derive(Clone, Copy)]
struct FirstLegV1 {
    horizontal_edge: CanonicalId,
    horizontal_id: ConstraintId,
    parallel_id: ConstraintId,
}

/// Finds only the sound two-hop, unit-terminal subset of the legacy general
/// parallel-component candidate.
///
/// Write the three edge vectors as `(x, 0)`, `(a, b)`, and `(0, v)`.
/// Deterministic `hypot(0, v) - 1 == 0` forces `|v| == 1` bit-exactly. The
/// second normalized cross is therefore `(+/-a) / hypot(a, b)`. If its rounded
/// binary64 result is zero, either `a` is zero or its exponent is so far below
/// the magnitude that pinned libm 0.2.16 takes the `ex - ey > 64` branch and
/// returns `|b| + |a|`, which rounds bit-exactly to `|b|`. The first normalized
/// cross then has numerator `fl(x * b)` and denominator
/// `fl(|x| * |b|)`: equal magnitudes by the same multiplication. A finite
/// nonzero product yields signed one; underflow yields `0 / 0 == NaN`; and
/// overflow yields `inf / inf == NaN`. None compare equal to zero.
///
/// The deterministic hypot wrapper rejects non-finite vector differences.
/// Signed zeros only change signs in the cases above. Non-unit scales and a
/// third parallel hop can exploit independent underflow/overflow and are
/// deliberately left to the unchanged quarantined graph detector.
pub(super) fn conflict_v1(
    parallels: &BTreeMap<EdgePairKey, Vec<ConstraintId>>,
    horizontal: &BTreeMap<CanonicalId, Vec<ConstraintId>>,
    vertical: &BTreeMap<CanonicalId, Vec<ConstraintId>>,
    fixed_lengths: &BTreeMap<CanonicalId, ScalarGroupSummary>,
    edge_ids: &BTreeMap<CanonicalId, EdgeId>,
    observer: &mut impl bounded_zero_closure::Observer,
) -> Result<Option<DirectConstraintConflictV1>, GeometricConstraintUnknownReasonV1> {
    #[cfg(test)]
    let max_work =
        UNIT_TWO_HOP_PARALLEL_TEST_WORK_LIMIT.with(|limit| limit.get().unwrap_or(MAX_WORK_V1));
    #[cfg(not(test))]
    let max_work = MAX_WORK_V1;
    #[cfg(test)]
    let max_storage = UNIT_TWO_HOP_PARALLEL_TEST_STORAGE_LIMIT
        .with(|limit| limit.get().unwrap_or(MAX_STORAGE_UNITS_V1));
    #[cfg(not(test))]
    let max_storage = MAX_STORAGE_UNITS_V1;
    #[cfg(test)]
    {
        UNIT_TWO_HOP_PARALLEL_TEST_WORK_OBSERVED.with(|observed| observed.set(0));
        UNIT_TWO_HOP_PARALLEL_TEST_STORAGE_OBSERVED.with(|observed| observed.set(0));
    }

    if let Some(reason) =
        preflight_observer_stop_reason(observer, bounded_zero_closure::Phase::ProofSearch, 0)
    {
        return Err(reason);
    }

    let mut work = 0_u64;
    let mut storage = 0_usize;
    let mut first_legs: BTreeMap<CanonicalId, Vec<FirstLegV1>> = BTreeMap::new();
    for (pair, parallel_ids) in parallels {
        charge_work_v1(&mut work, max_work, 1, observer)?;
        let Some(parallel_id) = parallel_ids
            .iter()
            .min_by_key(|id| id.canonical_bytes())
            .copied()
        else {
            continue;
        };
        for (horizontal_edge, middle_edge) in [(pair.first, pair.second), (pair.second, pair.first)]
        {
            let Some(horizontal_id) = horizontal
                .get(&horizontal_edge)
                .and_then(|ids| ids.iter().min_by_key(|id| id.canonical_bytes()))
                .copied()
            else {
                continue;
            };
            reserve_storage_v1(&mut storage, max_storage, 1)?;
            first_legs.entry(middle_edge).or_default().push(FirstLegV1 {
                horizontal_edge,
                horizontal_id,
                parallel_id,
            });
        }
    }

    let mut best: Option<DirectConstraintConflictV1> = None;
    for (pair, second_parallel_ids) in parallels {
        charge_work_v1(&mut work, max_work, 1, observer)?;
        let Some(second_parallel_id) = second_parallel_ids
            .iter()
            .min_by_key(|id| id.canonical_bytes())
            .copied()
        else {
            continue;
        };
        for (middle_edge, vertical_edge) in [(pair.first, pair.second), (pair.second, pair.first)] {
            let Some(vertical_id) = vertical
                .get(&vertical_edge)
                .and_then(|ids| ids.iter().min_by_key(|id| id.canonical_bytes()))
                .copied()
            else {
                continue;
            };
            let Some(unit_length) = fixed_lengths
                .get(&vertical_edge)
                .and_then(ScalarGroupSummary::consistent_assignment)
                .filter(|assignment| assignment.value.to_bits() == 1.0_f64.to_bits())
            else {
                continue;
            };
            for first_leg in first_legs.get(&middle_edge).into_iter().flatten() {
                charge_work_v1(&mut work, max_work, 1, observer)?;
                if first_leg.horizontal_edge == vertical_edge
                    || first_leg.parallel_id == second_parallel_id
                {
                    continue;
                }
                let mut constraint_ids = vec![
                    first_leg.horizontal_id,
                    first_leg.parallel_id,
                    second_parallel_id,
                    vertical_id,
                    unit_length.id,
                ];
                canonicalize_constraint_ids(&mut constraint_ids);
                if constraint_ids.len() != 5 {
                    continue;
                }
                let candidate = DirectConstraintConflictV1 {
                    conflict:
                        DirectConstraintConflictKindV1::PerpendicularOrientationsInParallelComponent {
                            horizontal_edge: edge_ids[&first_leg.horizontal_edge],
                            vertical_edge: edge_ids[&vertical_edge],
                            parallel_constraint_count: 2,
                        },
                    constraint_ids,
                };
                if best.as_ref().is_none_or(|current| {
                    conflict_sort_key(&candidate.conflict)
                        .cmp(&conflict_sort_key(&current.conflict))
                        .then_with(|| {
                            canonical_id_slice_cmp(
                                &candidate.constraint_ids,
                                &current.constraint_ids,
                            )
                        })
                        .is_lt()
                }) {
                    best = Some(candidate);
                }
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
    max_work: u64,
    amount: u64,
    observer: &mut impl bounded_zero_closure::Observer,
) -> Result<(), GeometricConstraintUnknownReasonV1> {
    let previous = *work;
    *work = work
        .checked_add(amount)
        .ok_or(GeometricConstraintUnknownReasonV1::WorkLimitExceeded)?;
    #[cfg(test)]
    UNIT_TWO_HOP_PARALLEL_TEST_WORK_OBSERVED.with(|observed| observed.set(*work));
    if *work > max_work {
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
    UNIT_TWO_HOP_PARALLEL_TEST_STORAGE_OBSERVED.with(|observed| observed.set(*storage));
    (*storage <= maximum)
        .then_some(())
        .ok_or(GeometricConstraintUnknownReasonV1::StorageLimitExceeded)
}

pub(super) fn is_proven_shape_v1(
    candidate: &DirectConstraintConflictV1,
    records: &[GeometricConstraintRecordV1],
) -> bool {
    let DirectConstraintConflictKindV1::PerpendicularOrientationsInParallelComponent {
        horizontal_edge: reported_horizontal,
        vertical_edge: reported_vertical,
        parallel_constraint_count,
    } = &candidate.conflict
    else {
        return false;
    };
    if *parallel_constraint_count != 2
        || candidate.constraint_ids.len() != 5
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

    let mut horizontal = None;
    let mut vertical = None;
    let mut fixed = None;
    let mut parallel_pairs = Vec::new();
    for record in selected {
        match &record.constraint {
            GeometricConstraintKindV1::Horizontal { edge } if horizontal.is_none() => {
                horizontal = Some(*edge);
            }
            GeometricConstraintKindV1::Vertical { edge } if vertical.is_none() => {
                vertical = Some(*edge);
            }
            GeometricConstraintKindV1::FixedLength { edge, length_mm }
                if fixed.is_none() && length_mm.to_bits() == 1.0_f64.to_bits() =>
            {
                fixed = Some(*edge);
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
    let (Some(horizontal), Some(vertical), Some(fixed)) = (horizontal, vertical, fixed) else {
        return false;
    };
    if horizontal != *reported_horizontal
        || vertical != *reported_vertical
        || fixed != vertical
        || horizontal == vertical
        || parallel_pairs.len() != 2
    {
        return false;
    }

    let horizontal_key = horizontal.canonical_bytes();
    let vertical_key = vertical.canonical_bytes();
    let mut all_edges = BTreeSet::new();
    for pair in &parallel_pairs {
        all_edges.extend([pair.first, pair.second]);
    }
    if all_edges.len() != 3
        || !all_edges.contains(&horizontal_key)
        || !all_edges.contains(&vertical_key)
    {
        return false;
    }
    let Some(middle) = all_edges
        .into_iter()
        .find(|edge| *edge != horizontal_key && *edge != vertical_key)
    else {
        return false;
    };
    let first = EdgePairKey {
        first: horizontal_key.min(middle),
        second: horizontal_key.max(middle),
    };
    let second = EdgePairKey {
        first: middle.min(vertical_key),
        second: middle.max(vertical_key),
    };
    parallel_pairs.contains(&first) && parallel_pairs.contains(&second)
}

#[cfg(test)]
std::thread_local! {
    static UNIT_TWO_HOP_PARALLEL_TEST_WORK_LIMIT: std::cell::Cell<Option<u64>> = const {
        std::cell::Cell::new(None)
    };
    static UNIT_TWO_HOP_PARALLEL_TEST_WORK_OBSERVED: std::cell::Cell<u64> = const {
        std::cell::Cell::new(0)
    };
    static UNIT_TWO_HOP_PARALLEL_TEST_STORAGE_LIMIT: std::cell::Cell<Option<usize>> = const {
        std::cell::Cell::new(None)
    };
    static UNIT_TWO_HOP_PARALLEL_TEST_STORAGE_OBSERVED: std::cell::Cell<usize> = const {
        std::cell::Cell::new(0)
    };
}

#[cfg(test)]
#[path = "../constraints_unit_two_hop_parallel_tests.rs"]
mod tests;
