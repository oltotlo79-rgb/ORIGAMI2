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
    singleton_constructive::single_constraint_canonical_assignment,
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

/// Constructs the deliberately narrow three-record component admitted by the
/// bounded compositor: one existing ordinary pair template plus one singleton
/// leaf joined to that pair through exactly one residual-referenced vertex.
///
/// All three possible pair decompositions are classified before construction.
/// Exactly one must be structurally eligible and constructible; ambiguity and
/// unsupported relations fail closed. The leaf is translated so its sole
/// articulation point is bit-identical to the pair point, and the complete
/// three-record residual language re-certifies every translated candidate.
pub(super) fn construct_pair_plus_singleton_leaf_exact_assignment_v1(
    pattern: &CreasePattern,
    document: &GeometricConstraintDocumentV1,
) -> Option<CurrentRuntimeExactConstraintAssignmentV1> {
    if document.constraints.len() != 3 {
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
    if referenced.len() != 3 {
        return None;
    }

    let mut unique = None;
    for (first_pair, second_pair, leaf) in [(0, 1, 2), (0, 2, 1), (1, 2, 0)] {
        let Some(decomposition) = eligible_decomposition(
            pattern,
            document,
            &referenced,
            first_pair,
            second_pair,
            leaf,
        ) else {
            continue;
        };
        if unique.replace(decomposition).is_some() {
            return None;
        }
    }

    let (pair_assignment, leaf_assignment, articulation) = unique?;
    let assignment = merge_at_articulation(pair_assignment, leaf_assignment, articulation)?;
    if assignment.len() > MAX_THREE_RECORD_COMPONENT_REFERENCED_VERTICES_V1
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

fn eligible_decomposition(
    pattern: &CreasePattern,
    document: &GeometricConstraintDocumentV1,
    referenced: &[Vec<VertexId>],
    first_pair: usize,
    second_pair: usize,
    leaf: usize,
) -> Option<(CanonicalAssignment, CanonicalAssignment, [u8; 16])> {
    let first_references = reference_keys(referenced.get(first_pair)?)?;
    let second_references = reference_keys(referenced.get(second_pair)?)?;
    if first_references.is_disjoint(&second_references) {
        return None;
    }

    let pair_references = first_references
        .union(&second_references)
        .copied()
        .collect::<BTreeSet<_>>();
    let leaf_references = reference_keys(referenced.get(leaf)?)?;
    let mut articulations = pair_references.intersection(&leaf_references).copied();
    let articulation = articulations.next()?;
    if articulations.next().is_some() {
        return None;
    }

    let first_constraint = &document.constraints.get(first_pair)?.constraint;
    let second_constraint = &document.constraints.get(second_pair)?.constraint;
    let pair_assignment =
        pair_canonical_assignment(pattern, first_constraint, second_constraint)
            .or_else(|| pair_canonical_assignment(pattern, second_constraint, first_constraint))?;
    if assignment_keys(&pair_assignment) != pair_references {
        return None;
    }

    let leaf_assignment = single_constraint_canonical_assignment(
        pattern,
        &document.constraints.get(leaf)?.constraint,
    )?;
    if assignment_keys(&leaf_assignment) != leaf_references {
        return None;
    }

    Some((pair_assignment, leaf_assignment, articulation))
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
