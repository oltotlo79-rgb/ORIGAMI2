use std::collections::BTreeMap;

use ori_domain::{
    CreasePattern, Edge, EdgeId, GeometricConstraintDocumentV1, GeometricConstraintKindV1, Point2,
    VertexId,
};
use ori_numeric::{deterministic_degrees_to_radians_v1, deterministic_sin_cos_degrees_v1};

use super::CurrentRuntimeExactConstraintAssignmentV1;
use crate::{
    ConstraintPreflightV1, GeometricConstraintLimitsV1,
    certify_binary64_exact_geometric_constraint_satisfaction_v1, prepare_geometric_constraints_v1,
};

pub(crate) const MAX_SINGLE_CONSTRAINT_CONSTRUCTIVE_CANDIDATES_V1: usize = 4;
/// Maximum document size accepted by the bounded singleton compositor.
///
/// This ceiling is intentionally independent of the bounded direct-MUS
/// oracle's limit: the two algorithms have different work models and neither
/// constant grants authority to the other.
pub const MAX_BOUNDED_SINGLETON_COMPOSITION_CONSTRAINTS_V1: usize = 16;

/// Constructs a bounded candidate assignment for one validated constraint and
/// returns it only after the complete production residual language reissues an
/// exact current-runtime certificate.
///
/// V1 tries four fixed translations of one canonical template. Referenced edge
/// endpoints are assigned by role, with incident-edge templates expressed as
/// outward vectors from the required vertex and two-edge templates handling a
/// shared endpoint explicitly. Trigonometric templates use the same frozen
/// degree conversion, `sin`, and `cos` operations as the proof residuals.
///
/// Invalid documents, documents containing other than one record, non-finite
/// or subnormal trigonometric components, conflicting shared roles, collapsed
/// geometry, and every candidate with any nonzero production residual return
/// `None`. Failure is not evidence of unsatisfiability.
#[must_use]
pub fn construct_single_constraint_exact_assignment_v1(
    pattern: &CreasePattern,
    document: &GeometricConstraintDocumentV1,
) -> Option<CurrentRuntimeExactConstraintAssignmentV1> {
    if document.constraints.len() != 1 {
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

    let base_assignment =
        single_constraint_canonical_assignment(pattern, &document.constraints[0].constraint)?;
    let offsets: [Point2; MAX_SINGLE_CONSTRAINT_CONSTRUCTIVE_CANDIDATES_V1] = [
        Point2::new(0.0, 0.0),
        Point2::new(16.0, 32.0),
        Point2::new(-16.0, 8.0),
        Point2::new(1024.0, -512.0),
    ];
    for offset in offsets {
        let mut candidate = pattern.clone();
        if !apply_translated_assignment(&mut candidate, &base_assignment, offset) {
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

/// Composes the existing singleton construction for exactly two validated
/// constraints, then re-certifies the complete two-record document.
///
/// This is deliberately not a general pair solver. The complete document first
/// passes through a fixed pair-template constructor supporting a wider but
/// still bounded two-record language; it grants no authority without full
/// residual re-certification. If that prepass is unsupported, each record must
/// produce its own bounded singleton assignment. The fallback requires
/// referenced vertex sets to be disjoint or to assign every shared vertex
/// bit-identical coordinates. Only those referenced coordinates are merged
/// into a detached source-pattern clone, and the complete production residual
/// verifier must then issue an exact certificate for both records together.
///
/// Unsupported singleton templates, conflicting shared assignments, direct
/// conflicts, invalid geometry, and any nonzero full-document residual return
/// `None`. Failure is not evidence of unsatisfiability.
#[must_use]
pub fn construct_two_constraint_exact_assignment_v1(
    pattern: &CreasePattern,
    document: &GeometricConstraintDocumentV1,
) -> Option<CurrentRuntimeExactConstraintAssignmentV1> {
    construct_bounded_singleton_composition_v1(pattern, document, 2)
}

/// Composes the existing singleton construction for exactly three validated
/// constraints, then re-certifies the complete three-record document.
///
/// This is the same deliberately incomplete family as
/// [`construct_two_constraint_exact_assignment_v1`]: every record must have a
/// bounded singleton assignment, and every coordinate assigned to a shared
/// referenced vertex must be bit-identical across all three assignments. The
/// merged candidate grants no authority unless the complete production
/// residual verifier re-certifies all three records together.
///
/// Unsupported templates, any shared-coordinate disagreement, direct
/// conflicts, invalid geometry, and any nonzero full-document residual return
/// `None`. Failure is not evidence of unsatisfiability.
#[must_use]
pub fn construct_three_constraint_exact_assignment_v1(
    pattern: &CreasePattern,
    document: &GeometricConstraintDocumentV1,
) -> Option<CurrentRuntimeExactConstraintAssignmentV1> {
    construct_bounded_singleton_composition_v1(pattern, document, 3)
}

/// Composes the existing singleton construction for exactly four validated
/// constraints, then re-certifies the complete four-record document.
///
/// This remains the same deliberately incomplete family as the two- and
/// three-record constructors. Every record must independently produce a
/// bounded singleton assignment, all assignments for a shared referenced
/// vertex must be bit-identical, and the complete production residual verifier
/// must re-certify the merged candidate before an observational-only
/// assignment is returned.
///
/// Unsupported templates, any shared-coordinate disagreement, direct
/// conflicts, invalid geometry, and any nonzero full-document residual return
/// `None`. Failure is not evidence of unsatisfiability.
#[must_use]
pub fn construct_four_constraint_exact_assignment_v1(
    pattern: &CreasePattern,
    document: &GeometricConstraintDocumentV1,
) -> Option<CurrentRuntimeExactConstraintAssignmentV1> {
    construct_bounded_singleton_composition_v1(pattern, document, 4)
}

/// Composes singleton assignments for a document containing exactly two
/// through sixteen records and re-certifies the complete document.
///
/// This exposes the shared bounded implementation used by the exact-count
/// wrappers without claiming a general constraint solver. Every candidate that
/// can escape is independently checked by the production binary64 residual
/// verifier. On the singleton fallback, every singleton and the final merged
/// candidate are checked separately, and shared referenced vertices must have
/// bit-identical coordinates.
///
/// At exactly two records, a fixed pair-template prepass runs first. It tries
/// at most one cardinal-rotation candidate and four ordinary translated
/// candidates, re-certifying the complete document after every candidate; if
/// none certifies, the singleton merge remains the fail-closed fallback. That
/// path has at most fourteen full residual verifications and four
/// default-limited preparations in total.
///
/// The explicit sixteen-record ceiling bounds both memory and work. At larger
/// counts, with four fixed singleton translations per record, the worst
/// successful path performs at most `16 * 4 + 1 = 65` candidate pattern clones
/// and full residual verifications, plus at most seventeen default-limited
/// preparations (one for the complete document and one for each singleton).
/// Across every supported document size, this is at most 82 bounded
/// preparation-or-verification passes. Work is therefore
/// `O(record_count * (vertices + edges))` with `record_count <= 16`, rather
/// than an unbounded combinatorial assignment search.
///
/// Documents outside the two-through-sixteen range, unsupported templates,
/// shared-coordinate disagreement, direct conflicts, invalid geometry, and
/// nonzero full-document residuals return `None`. Failure is not evidence of
/// unsatisfiability.
#[must_use]
pub fn construct_bounded_singleton_composition_exact_assignment_v1(
    pattern: &CreasePattern,
    document: &GeometricConstraintDocumentV1,
) -> Option<CurrentRuntimeExactConstraintAssignmentV1> {
    construct_bounded_singleton_composition_v1(pattern, document, document.constraints.len())
}

fn construct_bounded_singleton_composition_v1(
    pattern: &CreasePattern,
    document: &GeometricConstraintDocumentV1,
    expected_constraint_count: usize,
) -> Option<CurrentRuntimeExactConstraintAssignmentV1> {
    if document.constraints.len() != expected_constraint_count
        || !(2..=MAX_BOUNDED_SINGLETON_COMPOSITION_CONSTRAINTS_V1)
            .contains(&expected_constraint_count)
    {
        return None;
    }
    if expected_constraint_count == 2
        && let Some(assignment) =
            super::construct_pair_constraint_exact_assignment_v1(pattern, document)
    {
        return Some(assignment);
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

    let mut records = document.constraints.iter().collect::<Vec<_>>();
    records.sort_unstable_by_key(|record| record.id.canonical_bytes());
    let mut merged = CanonicalAssignment::new();
    for record in records {
        let singleton_document = GeometricConstraintDocumentV1 {
            schema_version: document.schema_version,
            constraints: vec![(*record).clone()],
        };
        let singleton =
            construct_single_constraint_exact_assignment_v1(pattern, &singleton_document)?;
        // The canonical singleton template assigns every and only referenced
        // vertex role. Its keys therefore define the bounded merge surface;
        // the actual translated coordinates still come from the independently
        // certified singleton candidate above.
        let referenced = single_constraint_canonical_assignment(pattern, &record.constraint)?;
        for (canonical_id, (vertex, _)) in referenced {
            let assigned = singleton
                .pattern()
                .vertices
                .iter()
                .find(|candidate| candidate.id == vertex)?;
            match merged.get(&canonical_id) {
                Some((existing_vertex, existing_point))
                    if *existing_vertex != vertex
                        || !point_bits_equal(*existing_point, assigned.position) =>
                {
                    return None;
                }
                Some(_) => {}
                None => {
                    merged.insert(canonical_id, (vertex, assigned.position));
                }
            }
        }
    }

    let mut candidate = pattern.clone();
    for (vertex, point) in merged.values() {
        let target = candidate
            .vertices
            .iter_mut()
            .find(|candidate| candidate.id == *vertex)?;
        target.position = *point;
    }
    let certificate =
        certify_binary64_exact_geometric_constraint_satisfaction_v1(&candidate, document)
            .ok()
            .flatten()?;
    Some(CurrentRuntimeExactConstraintAssignmentV1 {
        pattern: candidate,
        certificate,
    })
}

type CanonicalAssignment = BTreeMap<[u8; 16], (VertexId, Point2)>;

fn point_bits_equal(left: Point2, right: Point2) -> bool {
    left.x.to_bits() == right.x.to_bits() && left.y.to_bits() == right.y.to_bits()
}

fn single_constraint_canonical_assignment(
    pattern: &CreasePattern,
    constraint: &GeometricConstraintKindV1,
) -> Option<CanonicalAssignment> {
    let mut assignment = CanonicalAssignment::new();
    match *constraint {
        GeometricConstraintKindV1::FixedLength { edge, length_mm } => assign_edge_segment(
            pattern,
            &mut assignment,
            edge,
            Point2::new(0.0, 0.0),
            Point2::new(length_mm, 0.0),
        )?,
        GeometricConstraintKindV1::FixedAngle {
            vertex,
            first_edge,
            second_edge,
            angle_degrees,
        } => {
            let angle = checked_deterministic_angle(angle_degrees, angle_degrees != 0.0)?;
            let first_length = if angle_degrees == 0.0 { 2.0 } else { 1.0 };
            assign_outward_edge(
                pattern,
                &mut assignment,
                vertex,
                first_edge,
                Point2::new(first_length, 0.0),
            )?;
            assign_outward_edge(
                pattern,
                &mut assignment,
                vertex,
                second_edge,
                Point2::new(angle.cos, angle.sin),
            )?;
        }
        GeometricConstraintKindV1::Horizontal { edge } => assign_edge_segment(
            pattern,
            &mut assignment,
            edge,
            Point2::new(0.0, 0.0),
            Point2::new(1.0, 0.0),
        )?,
        GeometricConstraintKindV1::Vertical { edge } => assign_edge_segment(
            pattern,
            &mut assignment,
            edge,
            Point2::new(0.0, 0.0),
            Point2::new(0.0, 1.0),
        )?,
        GeometricConstraintKindV1::EqualLength {
            first_edge,
            second_edge,
        } => assign_edge_pair(
            pattern,
            &mut assignment,
            first_edge,
            second_edge,
            Point2::new(1.0, 0.0),
            Point2::new(0.0, 1.0),
        )?,
        GeometricConstraintKindV1::Parallel {
            first_edge,
            second_edge,
        } => assign_edge_pair(
            pattern,
            &mut assignment,
            first_edge,
            second_edge,
            Point2::new(1.0, 0.0),
            Point2::new(-1.0, 0.0),
        )?,
        GeometricConstraintKindV1::PointOnLine { vertex, line_edge } => {
            assign_edge_segment(
                pattern,
                &mut assignment,
                line_edge,
                Point2::new(-2.0, 0.0),
                Point2::new(2.0, 0.0),
            )?;
            assign_point(&mut assignment, vertex, Point2::new(0.0, 0.0))?;
        }
        GeometricConstraintKindV1::MirrorSymmetry {
            first_vertex,
            second_vertex,
            axis_edge,
        } => {
            assign_edge_segment(
                pattern,
                &mut assignment,
                axis_edge,
                Point2::new(-2.0, 0.0),
                Point2::new(2.0, 0.0),
            )?;
            assign_point(&mut assignment, first_vertex, Point2::new(0.0, 1.0))?;
            assign_point(&mut assignment, second_vertex, Point2::new(0.0, -1.0))?;
        }
        GeometricConstraintKindV1::RotationalSymmetry {
            center_vertex,
            source_vertex,
            target_vertex,
            angle_degrees,
        } => {
            let angle = checked_deterministic_angle(angle_degrees, true)?;
            assign_point(&mut assignment, center_vertex, Point2::new(0.0, 0.0))?;
            assign_point(&mut assignment, source_vertex, Point2::new(1.0, 0.0))?;
            assign_point(
                &mut assignment,
                target_vertex,
                Point2::new(angle.cos, angle.sin),
            )?;
        }
        GeometricConstraintKindV1::AngleBisector {
            vertex,
            first_edge,
            second_edge,
            bisector_edge,
        } => {
            assign_outward_edge(
                pattern,
                &mut assignment,
                vertex,
                first_edge,
                Point2::new(1.0, 0.0),
            )?;
            assign_outward_edge(
                pattern,
                &mut assignment,
                vertex,
                second_edge,
                Point2::new(0.0, 1.0),
            )?;
            assign_outward_edge(
                pattern,
                &mut assignment,
                vertex,
                bisector_edge,
                Point2::new(1.0, 1.0),
            )?;
        }
        GeometricConstraintKindV1::LengthRatio {
            numerator_edge,
            denominator_edge,
            ratio,
        } => assign_edge_pair(
            pattern,
            &mut assignment,
            numerator_edge,
            denominator_edge,
            Point2::new(ratio, 0.0),
            Point2::new(0.0, 1.0),
        )?,
    }
    Some(assignment)
}

#[derive(Clone, Copy)]
struct DeterministicAngle {
    sin: f64,
    cos: f64,
}

fn checked_deterministic_angle(
    angle_degrees: f64,
    require_nonzero: bool,
) -> Option<DeterministicAngle> {
    let radians = deterministic_degrees_to_radians_v1(angle_degrees).ok()?;
    let (sin, cos) = deterministic_sin_cos_degrees_v1(angle_degrees).ok()?;
    if !ordinary_or_zero(radians)
        || !ordinary_or_zero(sin)
        || !ordinary_or_zero(cos)
        || (require_nonzero && radians == 0.0)
    {
        return None;
    }
    Some(DeterministicAngle { sin, cos })
}

fn ordinary_or_zero(value: f64) -> bool {
    value == 0.0 || value.is_normal()
}

fn assign_edge_segment(
    pattern: &CreasePattern,
    assignment: &mut CanonicalAssignment,
    edge: EdgeId,
    start: Point2,
    end: Point2,
) -> Option<()> {
    let edge = find_edge(pattern, edge)?;
    assign_point(assignment, edge.start, start)?;
    assign_point(assignment, edge.end, end)
}

fn assign_outward_edge(
    pattern: &CreasePattern,
    assignment: &mut CanonicalAssignment,
    vertex: VertexId,
    edge: EdgeId,
    outward: Point2,
) -> Option<()> {
    let edge = find_edge(pattern, edge)?;
    let opposite = opposite_endpoint(edge, vertex)?;
    assign_point(assignment, vertex, Point2::new(0.0, 0.0))?;
    assign_point(assignment, opposite, outward)
}

fn assign_edge_pair(
    pattern: &CreasePattern,
    assignment: &mut CanonicalAssignment,
    first: EdgeId,
    second: EdgeId,
    first_vector: Point2,
    second_vector: Point2,
) -> Option<()> {
    let first = find_edge(pattern, first)?;
    let second = find_edge(pattern, second)?;
    let first_vertices = [first.start, first.end];
    let second_vertices = [second.start, second.end];
    let shared = first_vertices
        .into_iter()
        .filter(|vertex| second_vertices.contains(vertex))
        .collect::<Vec<_>>();
    match shared.as_slice() {
        [] => {
            assign_point(assignment, first.start, Point2::new(0.0, 0.0))?;
            assign_point(assignment, first.end, first_vector)?;
            let second_origin = Point2::new(0.0, 4.0);
            assign_point(assignment, second.start, second_origin)?;
            assign_point(
                assignment,
                second.end,
                checked_add(second_origin, second_vector)?,
            )
        }
        [shared] => {
            let first_other = opposite_endpoint(first, *shared)?;
            let second_other = opposite_endpoint(second, *shared)?;
            assign_point(assignment, *shared, Point2::new(0.0, 0.0))?;
            assign_point(assignment, first_other, first_vector)?;
            assign_point(assignment, second_other, second_vector)
        }
        _ => None,
    }
}

fn opposite_endpoint(edge: &Edge, vertex: VertexId) -> Option<VertexId> {
    if edge.start == vertex {
        Some(edge.end)
    } else if edge.end == vertex {
        Some(edge.start)
    } else {
        None
    }
}

fn find_edge(pattern: &CreasePattern, id: EdgeId) -> Option<&Edge> {
    pattern.edges.iter().find(|edge| edge.id == id)
}

fn assign_point(
    assignment: &mut CanonicalAssignment,
    vertex: VertexId,
    point: Point2,
) -> Option<()> {
    if !point.x.is_finite() || !point.y.is_finite() {
        return None;
    }
    match assignment.get(&vertex.canonical_bytes()) {
        Some((existing_vertex, existing_point))
            if *existing_vertex != vertex || *existing_point != point =>
        {
            None
        }
        Some(_) => Some(()),
        None => {
            assignment.insert(vertex.canonical_bytes(), (vertex, point));
            Some(())
        }
    }
}

fn apply_translated_assignment(
    candidate: &mut CreasePattern,
    assignment: &CanonicalAssignment,
    offset: Point2,
) -> bool {
    for (vertex, point) in assignment.values() {
        let Some(translated) = checked_add(*point, offset) else {
            return false;
        };
        let Some(target) = candidate
            .vertices
            .iter_mut()
            .find(|candidate| candidate.id == *vertex)
        else {
            return false;
        };
        target.position = translated;
    }
    true
}

fn checked_add(left: Point2, right: Point2) -> Option<Point2> {
    let result = Point2::new(left.x + right.x, left.y + right.y);
    (result.x.is_finite() && result.y.is_finite()).then_some(result)
}
