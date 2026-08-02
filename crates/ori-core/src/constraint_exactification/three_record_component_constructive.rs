use std::collections::BTreeSet;

use ori_domain::{CreasePattern, GeometricConstraintDocumentV1, Point2, VertexId};

use super::{
    CurrentRuntimeExactConstraintAssignmentV1,
    pair_constructive::{
        geometry::{
            CanonicalAssignment, apply_translated_assignment, assignment_points_are_distinct,
            ordinary_or_zero,
        },
        pair_canonical_assignment,
    },
    singleton_constructive::{
        MAX_BOUNDED_SINGLETON_COMPOSITION_CONSTRAINTS_V1, single_constraint_canonical_assignment,
    },
};
use crate::{
    ConstraintPreflightV1, GeometricConstraintLimitsV1,
    certify_binary64_exact_geometric_constraint_satisfaction_v1,
    constraint_solver::residual_referenced_vertices_by_record_v1, prepare_geometric_constraints_v1,
};

pub(super) const MAX_THREE_RECORD_COMPONENT_CONSTRUCTIVE_CANDIDATES_V1: usize = 4;
/// The pair and leaf may each reference at most four vertices and must share
/// exactly one, so this narrow template stores at most seven assignments.
pub(super) const MAX_THREE_RECORD_COMPONENT_REFERENCED_VERTICES_V1: usize = 7;
/// A bounded star classifies at most every unordered pair of its 16 records.
pub(super) const MAX_PAIR_PLUS_SINGLETON_STAR_PAIR_CLASSIFICATIONS_V1: usize =
    MAX_BOUNDED_SINGLETON_COMPOSITION_CONSTRAINTS_V1
        * (MAX_BOUNDED_SINGLETON_COMPOSITION_CONSTRAINTS_V1 - 1)
        / 2;
/// Each possible pair may classify every other record as a singleton leaf.
pub(super) const MAX_PAIR_PLUS_SINGLETON_STAR_LEAF_CLASSIFICATIONS_V1: usize =
    MAX_PAIR_PLUS_SINGLETON_STAR_PAIR_CLASSIFICATIONS_V1
        * (MAX_BOUNDED_SINGLETON_COMPOSITION_CONSTRAINTS_V1 - 2);
/// Every unordered pair of the at most 120 ordinary-pair candidates is
/// considered once when classifying a two-core component.
pub(super) const MAX_TWO_PAIR_CORE_COMBINATIONS_V1: usize =
    MAX_PAIR_PLUS_SINGLETON_STAR_PAIR_CLASSIFICATIONS_V1
        * (MAX_PAIR_PLUS_SINGLETON_STAR_PAIR_CLASSIFICATIONS_V1 - 1)
        / 2;
/// Each possible pair-core combination may classify the remaining twelve
/// records as singleton leaves.
pub(super) const MAX_TWO_PAIR_CORE_LEAF_CLASSIFICATIONS_V1: usize =
    MAX_TWO_PAIR_CORE_COMBINATIONS_V1 * (MAX_BOUNDED_SINGLETON_COMPOSITION_CONSTRAINTS_V1 - 4);
/// Two pair records retain at most seven referenced vertices. Each of the
/// remaining fourteen records may add at most three after its one articulation
/// vertex has been identified with the pair.
pub(super) const MAX_PAIR_PLUS_SINGLETON_STAR_REFERENCED_VERTICES_V1: usize =
    7 + (MAX_BOUNDED_SINGLETON_COMPOSITION_CONSTRAINTS_V1 - 2) * 3;
/// Two ordinary pair cores retain at most thirteen referenced vertices after
/// their one articulation is identified. Twelve singleton leaves may each add
/// at most three further vertices, which preserves the existing 49-vertex
/// ceiling.
pub(super) const MAX_TWO_PAIR_CORE_STAR_REFERENCED_VERTICES_V1: usize =
    13 + (MAX_BOUNDED_SINGLETON_COMPOSITION_CONSTRAINTS_V1 - 4) * 3;

struct OrdinaryPairCoreV1 {
    first_record: usize,
    second_record: usize,
    references: BTreeSet<[u8; 16]>,
    assignment: CanonicalAssignment,
}

/// Constructs the deliberately narrow three-record component admitted by the
/// bounded compositor: one existing ordinary pair template plus one singleton
/// leaf joined to that pair through exactly one residual-referenced vertex.
///
/// All three possible pair decompositions are classified before construction.
/// Exactly one must be structurally eligible and constructible; ambiguity and
/// unsupported relations fail closed. The leaf is translated so its sole
/// articulation point is bit-identical to the pair point, and the complete
/// three-record residual language re-certifies every translated candidate.
#[cfg(test)]
pub(super) fn construct_pair_plus_singleton_leaf_exact_assignment_v1(
    pattern: &CreasePattern,
    document: &GeometricConstraintDocumentV1,
) -> Option<CurrentRuntimeExactConstraintAssignmentV1> {
    if document.constraints.len() != 3 {
        return None;
    }
    construct_pair_plus_singleton_star_exact_assignment_v1(pattern, document)
}

/// Constructs one ordinary pair with one through fourteen independent
/// singleton leaves. Every leaf must meet the pair through exactly one
/// residual-referenced articulation vertex and may not meet another leaf away
/// from that pair. All possible unordered pair choices are classified and
/// exactly one complete star decomposition must exist; ambiguity fails closed.
///
/// The record ceiling is shared with the bounded compositor. Consequently the
/// classification performs at most 120 pair and 1,680 leaf classifications,
/// then tries the same four fixed translations as the three-record boundary.
/// Only the complete production residual certificate may escape.
pub(super) fn construct_pair_plus_singleton_star_exact_assignment_v1(
    pattern: &CreasePattern,
    document: &GeometricConstraintDocumentV1,
) -> Option<CurrentRuntimeExactConstraintAssignmentV1> {
    if !(3..=MAX_BOUNDED_SINGLETON_COMPOSITION_CONSTRAINTS_V1).contains(&document.constraints.len())
    {
        return None;
    }
    let prepared =
        prepare_geometric_constraints_v1(pattern, document, GeometricConstraintLimitsV1::default())
            .ok()?;
    if matches!(
        prepared.preflight(),
        ConstraintPreflightV1::DirectConflict { .. }
    ) {
        return None;
    }

    let referenced = residual_referenced_vertices_by_record_v1(pattern, document).ok()?;
    if referenced.len() != document.constraints.len() {
        return None;
    }

    let mut unique = None;
    let mut pair_classifications = 0usize;
    let mut leaf_classifications = 0usize;
    for first_pair in 0..document.constraints.len() {
        for second_pair in (first_pair + 1)..document.constraints.len() {
            pair_classifications = pair_classifications.checked_add(1)?;
            leaf_classifications =
                leaf_classifications.checked_add(document.constraints.len().checked_sub(2)?)?;
            if pair_classifications > MAX_PAIR_PLUS_SINGLETON_STAR_PAIR_CLASSIFICATIONS_V1
                || leaf_classifications > MAX_PAIR_PLUS_SINGLETON_STAR_LEAF_CLASSIFICATIONS_V1
            {
                return None;
            }
            let Some(assignment) =
                eligible_star_assignment(pattern, document, &referenced, first_pair, second_pair)
            else {
                continue;
            };
            if unique.replace(assignment).is_some() {
                return None;
            }
        }
    }

    let assignment = unique?;
    let max_referenced_vertices = if document.constraints.len() == 3 {
        MAX_THREE_RECORD_COMPONENT_REFERENCED_VERTICES_V1
    } else {
        MAX_PAIR_PLUS_SINGLETON_STAR_REFERENCED_VERTICES_V1
    };
    if assignment.len() > max_referenced_vertices || !assignment_points_are_distinct(&assignment) {
        return None;
    }

    let offsets: [Point2; MAX_THREE_RECORD_COMPONENT_CONSTRUCTIVE_CANDIDATES_V1] = [
        Point2::new(0.0, 0.0),
        Point2::new(16.0, 32.0),
        Point2::new(-16.0, 8.0),
        Point2::new(1024.0, -512.0),
    ];
    for offset in offsets {
        let mut candidate = pattern.clone();
        if !apply_translated_assignment(&mut candidate, &assignment, offset) {
            continue;
        }
        let certificate =
            certify_binary64_exact_geometric_constraint_satisfaction_v1(&candidate, document)
                .ok()
                .flatten();
        if let Some(certificate) = certificate {
            return Some(CurrentRuntimeExactConstraintAssignmentV1 {
                pattern: candidate,
                certificate,
            });
        }
    }
    None
}

/// Constructs two record-disjoint ordinary-pair cores joined through exactly
/// one residual-referenced articulation, plus zero through twelve independent
/// singleton leaves. Each leaf meets the core union in exactly one vertex and
/// no two leaves may share a vertex outside that union.
///
/// Records are first sorted by canonical constraint ID. Every unordered pair
/// is classified once, then every unordered combination of two pair candidates
/// is considered once. Exactly one complete decomposition may reach the four
/// fixed translations. All arithmetic and classification envelopes are
/// checked before a production residual certificate may escape.
pub(super) fn construct_two_pair_core_singleton_star_exact_assignment_v1(
    pattern: &CreasePattern,
    document: &GeometricConstraintDocumentV1,
) -> Option<CurrentRuntimeExactConstraintAssignmentV1> {
    let record_count = document.constraints.len();
    if !(4..=MAX_BOUNDED_SINGLETON_COMPOSITION_CONSTRAINTS_V1).contains(&record_count) {
        return None;
    }
    let (declared_pair_candidates, declared_core_combinations, declared_leaf_classifications) =
        checked_two_pair_core_classification_bounds_v1(record_count)?;
    let prepared =
        prepare_geometric_constraints_v1(pattern, document, GeometricConstraintLimitsV1::default())
            .ok()?;
    if matches!(
        prepared.preflight(),
        ConstraintPreflightV1::DirectConflict { .. }
    ) {
        return None;
    }

    let ordered_document = canonical_document(document);
    let referenced = residual_referenced_vertices_by_record_v1(pattern, &ordered_document).ok()?;
    if referenced.len() != record_count {
        return None;
    }

    let mut cores = Vec::new();
    cores.try_reserve_exact(declared_pair_candidates).ok()?;
    let mut pair_classifications = 0usize;
    for first_record in 0..record_count {
        for second_record in (first_record + 1)..record_count {
            pair_classifications = pair_classifications.checked_add(1)?;
            if pair_classifications > MAX_PAIR_PLUS_SINGLETON_STAR_PAIR_CLASSIFICATIONS_V1 {
                return None;
            }
            if let Some(core) = ordinary_pair_core(
                pattern,
                &ordered_document,
                &referenced,
                first_record,
                second_record,
            ) {
                cores.push(core);
            }
        }
    }
    if pair_classifications != declared_pair_candidates {
        return None;
    }

    let potential_leaves = record_count.checked_sub(4)?;
    let mut unique = None;
    let mut core_combinations = 0usize;
    let mut leaf_classifications = 0usize;
    for (first_core_index, first_core) in cores.iter().enumerate() {
        for second_core in cores.iter().skip(first_core_index + 1) {
            core_combinations = core_combinations.checked_add(1)?;
            leaf_classifications = leaf_classifications.checked_add(potential_leaves)?;
            if core_combinations > declared_core_combinations
                || core_combinations > MAX_TWO_PAIR_CORE_COMBINATIONS_V1
                || leaf_classifications > declared_leaf_classifications
                || leaf_classifications > MAX_TWO_PAIR_CORE_LEAF_CLASSIFICATIONS_V1
            {
                return None;
            }
            let Some(assignment) = eligible_two_pair_core_assignment(
                pattern,
                &ordered_document,
                &referenced,
                first_core,
                second_core,
            ) else {
                continue;
            };
            if unique.replace(assignment).is_some() {
                return None;
            }
        }
    }

    let assignment = unique?;
    if assignment.len() > MAX_TWO_PAIR_CORE_STAR_REFERENCED_VERTICES_V1
        || !assignment_points_are_distinct(&assignment)
    {
        return None;
    }

    let offsets: [Point2; MAX_THREE_RECORD_COMPONENT_CONSTRUCTIVE_CANDIDATES_V1] = [
        Point2::new(0.0, 0.0),
        Point2::new(16.0, 32.0),
        Point2::new(-16.0, 8.0),
        Point2::new(1024.0, -512.0),
    ];
    for offset in offsets {
        let mut candidate = pattern.clone();
        if !apply_translated_assignment(&mut candidate, &assignment, offset) {
            continue;
        }
        let certificate =
            certify_binary64_exact_geometric_constraint_satisfaction_v1(&candidate, document)
                .ok()
                .flatten();
        if let Some(certificate) = certificate {
            return Some(CurrentRuntimeExactConstraintAssignmentV1 {
                pattern: candidate,
                certificate,
            });
        }
    }
    None
}

pub(super) fn checked_two_pair_core_classification_bounds_v1(
    record_count: usize,
) -> Option<(usize, usize, usize)> {
    let pair_candidates = record_count
        .checked_mul(record_count.checked_sub(1)?)?
        .checked_div(2)?;
    let core_combinations = pair_candidates
        .checked_mul(pair_candidates.checked_sub(1)?)?
        .checked_div(2)?;
    let leaf_classifications = core_combinations.checked_mul(record_count.checked_sub(4)?)?;
    (pair_candidates <= MAX_PAIR_PLUS_SINGLETON_STAR_PAIR_CLASSIFICATIONS_V1
        && core_combinations <= MAX_TWO_PAIR_CORE_COMBINATIONS_V1
        && leaf_classifications <= MAX_TWO_PAIR_CORE_LEAF_CLASSIFICATIONS_V1)
        .then_some((pair_candidates, core_combinations, leaf_classifications))
}

fn canonical_document(document: &GeometricConstraintDocumentV1) -> GeometricConstraintDocumentV1 {
    let mut constraints = document.constraints.clone();
    constraints.sort_unstable_by_key(|record| record.id.canonical_bytes());
    GeometricConstraintDocumentV1 {
        schema_version: document.schema_version,
        constraints,
    }
}

fn eligible_star_assignment(
    pattern: &CreasePattern,
    document: &GeometricConstraintDocumentV1,
    referenced: &[Vec<VertexId>],
    first_pair: usize,
    second_pair: usize,
) -> Option<CanonicalAssignment> {
    let core = ordinary_pair_core(pattern, document, referenced, first_pair, second_pair)?;
    let pair_references = core.references;
    let mut assignment = core.assignment;

    for leaf in 0..document.constraints.len() {
        if leaf == first_pair || leaf == second_pair {
            continue;
        }
        let leaf_references = reference_keys(referenced.get(leaf)?)?;
        let mut articulations = pair_references.intersection(&leaf_references).copied();
        let articulation = articulations.next()?;
        if articulations.next().is_some() {
            return None;
        }

        let leaf_assignment = single_constraint_canonical_assignment(
            pattern,
            &document.constraints.get(leaf)?.constraint,
        )?;
        if assignment_keys(&leaf_assignment) != leaf_references {
            return None;
        }
        assignment = merge_at_articulation(assignment, leaf_assignment, articulation)?;
    }

    Some(assignment)
}

fn ordinary_pair_core(
    pattern: &CreasePattern,
    document: &GeometricConstraintDocumentV1,
    referenced: &[Vec<VertexId>],
    first_record: usize,
    second_record: usize,
) -> Option<OrdinaryPairCoreV1> {
    let first_references = reference_keys(referenced.get(first_record)?)?;
    let second_references = reference_keys(referenced.get(second_record)?)?;
    if first_references.is_disjoint(&second_references) {
        return None;
    }
    let references = first_references
        .union(&second_references)
        .copied()
        .collect::<BTreeSet<_>>();
    let first_constraint = &document.constraints.get(first_record)?.constraint;
    let second_constraint = &document.constraints.get(second_record)?.constraint;
    let assignment = pair_canonical_assignment(pattern, first_constraint, second_constraint)
        .or_else(|| pair_canonical_assignment(pattern, second_constraint, first_constraint))?;
    if assignment_keys(&assignment) != references {
        return None;
    }
    Some(OrdinaryPairCoreV1 {
        first_record,
        second_record,
        references,
        assignment,
    })
}

fn eligible_two_pair_core_assignment(
    pattern: &CreasePattern,
    document: &GeometricConstraintDocumentV1,
    referenced: &[Vec<VertexId>],
    first_core: &OrdinaryPairCoreV1,
    second_core: &OrdinaryPairCoreV1,
) -> Option<CanonicalAssignment> {
    if first_core.first_record == second_core.first_record
        || first_core.first_record == second_core.second_record
        || first_core.second_record == second_core.first_record
        || first_core.second_record == second_core.second_record
    {
        return None;
    }
    let mut articulations = first_core
        .references
        .intersection(&second_core.references)
        .copied();
    let articulation = articulations.next()?;
    if articulations.next().is_some() {
        return None;
    }
    let core_references = first_core
        .references
        .union(&second_core.references)
        .copied()
        .collect::<BTreeSet<_>>();
    let mut assignment = merge_at_articulation(
        first_core.assignment.clone(),
        second_core.assignment.clone(),
        articulation,
    )?;
    let mut leaf_external_references = BTreeSet::new();

    for leaf in 0..document.constraints.len() {
        if leaf == first_core.first_record
            || leaf == first_core.second_record
            || leaf == second_core.first_record
            || leaf == second_core.second_record
        {
            continue;
        }
        let leaf_references = reference_keys(referenced.get(leaf)?)?;
        let mut articulations = core_references.intersection(&leaf_references).copied();
        let articulation = articulations.next()?;
        if articulations.next().is_some() {
            return None;
        }
        for external in leaf_references.difference(&core_references).copied() {
            if !leaf_external_references.insert(external) {
                return None;
            }
        }
        let leaf_assignment = single_constraint_canonical_assignment(
            pattern,
            &document.constraints.get(leaf)?.constraint,
        )?;
        if assignment_keys(&leaf_assignment) != leaf_references {
            return None;
        }
        assignment = merge_at_articulation(assignment, leaf_assignment, articulation)?;
    }

    Some(assignment)
}

fn reference_keys(vertices: &[VertexId]) -> Option<BTreeSet<[u8; 16]>> {
    let keys = vertices
        .iter()
        .map(VertexId::canonical_bytes)
        .collect::<BTreeSet<_>>();
    (keys.len() == vertices.len()).then_some(keys)
}

fn assignment_keys(assignment: &CanonicalAssignment) -> BTreeSet<[u8; 16]> {
    assignment.keys().copied().collect()
}

fn merge_at_articulation(
    mut pair_assignment: CanonicalAssignment,
    leaf_assignment: CanonicalAssignment,
    articulation: [u8; 16],
) -> Option<CanonicalAssignment> {
    let (pair_vertex, pair_point) = *pair_assignment.get(&articulation)?;
    let (leaf_vertex, leaf_point) = *leaf_assignment.get(&articulation)?;
    if pair_vertex != leaf_vertex {
        return None;
    }
    let translation = checked_subtract(pair_point, leaf_point)?;

    for (key, (vertex, point)) in leaf_assignment {
        if key == articulation {
            continue;
        }
        let translated = checked_add(point, translation)?;
        if pair_assignment.insert(key, (vertex, translated)).is_some() {
            return None;
        }
    }
    Some(pair_assignment)
}

fn checked_add(left: Point2, right: Point2) -> Option<Point2> {
    checked_point(left.x + right.x, left.y + right.y)
}

fn checked_subtract(left: Point2, right: Point2) -> Option<Point2> {
    checked_point(left.x - right.x, left.y - right.y)
}

fn checked_point(x: f64, y: f64) -> Option<Point2> {
    (ordinary_or_zero(x) && ordinary_or_zero(y)).then_some(Point2::new(x, y))
}
