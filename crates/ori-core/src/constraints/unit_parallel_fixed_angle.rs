use super::*;

/// Adds the exact single-unit 45/135-degree proof and the retained two-unit
/// legacy proof for one canonical parallel edge pair.
pub(super) fn collect_conflicts_v1(
    conflicts: &mut Vec<DirectConstraintConflictV1>,
    pair: EdgePairKey,
    parallel_ids: &[ConstraintId],
    fixed_lengths: &BTreeMap<CanonicalId, ScalarGroupSummary>,
    fixed_angles_by_pair: &BTreeMap<EdgePairKey, Vec<ScalarAssignment>>,
    edge_ids: &BTreeMap<CanonicalId, EdgeId>,
) {
    let first_unit = fixed_lengths
        .get(&pair.first)
        .and_then(ScalarGroupSummary::consistent_assignment)
        .filter(|assignment| assignment.value.to_bits() == 1.0_f64.to_bits());
    let second_unit = fixed_lengths
        .get(&pair.second)
        .and_then(ScalarGroupSummary::consistent_assignment)
        .filter(|assignment| assignment.value.to_bits() == 1.0_f64.to_bits());
    let exact_single_unit_angle = fixed_angles_by_pair.get(&pair).and_then(|assignments| {
        assignments
            .iter()
            .filter(|assignment| is_exact_single_unit_parallel_angle_v1(assignment.value))
            .min_by_key(|assignment| assignment.id.canonical_bytes())
    });
    let canonical_unit = [first_unit, second_unit]
        .into_iter()
        .flatten()
        .min_by_key(|assignment| assignment.id.canonical_bytes());
    if let (Some(parallel_id), Some(angle_assignment), Some(unit)) = (
        parallel_ids.first(),
        exact_single_unit_angle,
        canonical_unit,
    ) {
        // With one pinned unit hypot, the normalized parallel denominator is
        // exactly the other finite hypot. If a non-zero raw cross still divides
        // to signed zero, binary64 product/add error bounds force the
        // corresponding dot to dominate it by more than the frozen 45- or
        // 135-degree exact-zero enclosure permits. Negating either vector
        // changes the dot sign without changing the absolute cross or hypot
        // bounds, so the supplementary case has the same proof boundary.
        // Exact raw zero reaches only the separately rejected zero/pi atan2
        // branches.
        push_conflict(
            conflicts,
            DirectConstraintConflictKindV1::ParallelWithFixedNonParallelAngle {
                first_edge: edge_ids[&pair.first],
                second_edge: edge_ids[&pair.second],
            },
            [*parallel_id, angle_assignment.id, unit.id],
        );
    }
    let legacy_angle = exact_single_unit_angle
        .is_none()
        .then(|| {
            fixed_angles_by_pair.get(&pair).and_then(|assignments| {
                assignments
                    .iter()
                    .find(|assignment| fixed_angle_rejects_zero_cross_binary64_v1(assignment.value))
            })
        })
        .flatten();
    if let (Some(parallel_id), Some(angle_assignment), Some(first_unit), Some(second_unit)) =
        (parallel_ids.first(), legacy_angle, first_unit, second_unit)
    {
        // The solver computes `cross / (hypot(first) * hypot(second))`.
        // These two exact unit-length residuals make that denominator
        // bit-exactly 1.0, so a zero parallel residual proves the raw cross
        // used by FixedAngle is also exactly zero. The frozen angle helper then
        // rejects every reachable atan2(+0, dot) class; no stored-angle
        // inequality is used as evidence.
        push_conflict(
            conflicts,
            DirectConstraintConflictKindV1::ParallelWithFixedNonParallelAngle {
                first_edge: edge_ids[&pair.first],
                second_edge: edge_ids[&pair.second],
            },
            [
                *parallel_id,
                angle_assignment.id,
                first_unit.id,
                second_unit.id,
            ],
        );
    }
}

fn is_exact_single_unit_parallel_angle_v1(angle_degrees: f64) -> bool {
    [45.0_f64, 135.0_f64]
        .into_iter()
        .any(|angle| angle_degrees.to_bits() == angle.to_bits())
}

/// Returns whether the shared production residual rejects every result class
/// reachable when the production absolute cross term is exactly `+0.0`.
///
/// `atan2(+0, dot)` depends on the dot product's sign and signed-zero class.
/// Finite same-orientation vectors cover positive, negative, `+0.0`, and
/// `-0.0`; overflow can additionally produce either infinity or NaN. The
/// representative inputs below are evaluated through the same frozen
/// deterministic `atan2` and fixed-angle helper as the exact certificate
/// instead of assuming stored degree inequality or a hard-coded pi bit
/// pattern.
pub(super) fn fixed_angle_rejects_zero_cross_binary64_v1(angle_degrees: f64) -> bool {
    debug_assert!(angle_degrees.is_finite() && (0.0..=180.0).contains(&angle_degrees));
    [
        0.0,
        -0.0,
        1.0,
        -1.0,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NAN,
    ]
    .into_iter()
    .all(|dot| match deterministic_atan2_v1(0.0, dot) {
        Ok(actual) => {
            let residual = deterministic_fixed_angle_residual_binary64_v1(actual, angle_degrees);
            !residual.is_finite() || residual != 0.0
        }
        Err(_) => true,
    })
}

/// Independently replays the exact three-record grammar admitted by the
/// single-unit 45/135-degree parallel/angle proof.
///
/// Full preparation has already proved that every referenced edge exists and
/// that the fixed-angle vertex is common to both edges. This verifier does not
/// trust candidate construction for record roles, scalar bits, canonical
/// ordering, or the reported edge pair.
// The historical function name is retained because it is cited by the
// evidence manifest. Its accepted grammar now includes the exact
// supplementary 135-degree case as well as 45 degrees.
pub(super) fn is_proven_exact_forty_five_single_unit_parallel_angle_shape_v1(
    candidate: &DirectConstraintConflictV1,
    records: &[GeometricConstraintRecordV1],
) -> bool {
    let DirectConstraintConflictKindV1::ParallelWithFixedNonParallelAngle {
        first_edge: reported_first,
        second_edge: reported_second,
    } = &candidate.conflict
    else {
        return false;
    };
    let reported_pair = EdgePairKey::unordered(*reported_first, *reported_second);
    if reported_first.canonical_bytes() != reported_pair.first
        || reported_second.canonical_bytes() != reported_pair.second
        || reported_pair.first == reported_pair.second
        || candidate.constraint_ids.len() != 3
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
    if cause_ids.len() != 3 {
        return false;
    }
    let selected = records
        .iter()
        .filter(|record| cause_ids.contains(&record.id.canonical_bytes()))
        .collect::<Vec<_>>();
    if selected.len() != 3 {
        return false;
    }

    let mut saw_parallel = false;
    let mut saw_angle = false;
    let mut saw_unit = false;
    for record in selected {
        match &record.constraint {
            GeometricConstraintKindV1::Parallel {
                first_edge,
                second_edge,
            } if !saw_parallel
                && EdgePairKey::unordered(*first_edge, *second_edge) == reported_pair =>
            {
                saw_parallel = true;
            }
            GeometricConstraintKindV1::FixedAngle {
                first_edge,
                second_edge,
                angle_degrees,
                ..
            } if !saw_angle
                && is_exact_single_unit_parallel_angle_v1(*angle_degrees)
                && EdgePairKey::unordered(*first_edge, *second_edge) == reported_pair =>
            {
                saw_angle = true;
            }
            GeometricConstraintKindV1::FixedLength { edge, length_mm }
                if !saw_unit
                    && length_mm.to_bits() == 1.0_f64.to_bits()
                    && [reported_pair.first, reported_pair.second]
                        .contains(&edge.canonical_bytes()) =>
            {
                saw_unit = true;
            }
            _ => return false,
        }
    }
    saw_parallel && saw_angle && saw_unit
}

#[cfg(test)]
pub(crate) fn is_proven_exact_forty_five_single_unit_parallel_angle_shape_for_test_v1(
    first_edge: EdgeId,
    second_edge: EdgeId,
    constraint_ids: Vec<ConstraintId>,
    records: &[GeometricConstraintRecordV1],
) -> bool {
    is_proven_exact_forty_five_single_unit_parallel_angle_shape_v1(
        &DirectConstraintConflictV1 {
            conflict: DirectConstraintConflictKindV1::ParallelWithFixedNonParallelAngle {
                first_edge,
                second_edge,
            },
            constraint_ids,
        },
        records,
    )
}

/// Revalidates the legacy four-record form retained outside the exact
/// single-unit supplementary pair.
pub(super) fn is_proven_legacy_two_unit_parallel_angle_shape_v1(
    candidate: &DirectConstraintConflictV1,
    records: &[GeometricConstraintRecordV1],
) -> bool {
    let DirectConstraintConflictKindV1::ParallelWithFixedNonParallelAngle {
        first_edge: reported_first,
        second_edge: reported_second,
    } = &candidate.conflict
    else {
        return false;
    };
    let reported_pair = EdgePairKey::unordered(*reported_first, *reported_second);
    if reported_first.canonical_bytes() != reported_pair.first
        || reported_second.canonical_bytes() != reported_pair.second
        || reported_pair.first == reported_pair.second
        || candidate.constraint_ids.len() != 4
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
    if cause_ids.len() != 4 {
        return false;
    }
    let selected = records
        .iter()
        .filter(|record| cause_ids.contains(&record.id.canonical_bytes()))
        .collect::<Vec<_>>();
    if selected.len() != 4 {
        return false;
    }

    let mut saw_parallel = false;
    let mut saw_angle = false;
    let mut fixed_edges = Vec::new();
    for record in selected {
        match &record.constraint {
            GeometricConstraintKindV1::Parallel {
                first_edge,
                second_edge,
            } if !saw_parallel
                && EdgePairKey::unordered(*first_edge, *second_edge) == reported_pair =>
            {
                saw_parallel = true;
            }
            GeometricConstraintKindV1::FixedAngle {
                first_edge,
                second_edge,
                angle_degrees,
                ..
            } if !saw_angle
                && !is_exact_single_unit_parallel_angle_v1(*angle_degrees)
                && fixed_angle_rejects_zero_cross_binary64_v1(*angle_degrees)
                && EdgePairKey::unordered(*first_edge, *second_edge) == reported_pair =>
            {
                saw_angle = true;
            }
            GeometricConstraintKindV1::FixedLength { edge, length_mm }
                if fixed_edges.len() < 2 && length_mm.to_bits() == 1.0_f64.to_bits() =>
            {
                fixed_edges.push(edge.canonical_bytes());
            }
            _ => return false,
        }
    }
    fixed_edges.sort_unstable();
    saw_parallel && saw_angle && fixed_edges == [reported_pair.first, reported_pair.second]
}
