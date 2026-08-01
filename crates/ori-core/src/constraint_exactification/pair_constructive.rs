use ori_domain::{CreasePattern, GeometricConstraintDocumentV1, GeometricConstraintKindV1, Point2};
use ori_numeric::{deterministic_degrees_to_radians_v1, deterministic_sin_cos_degrees_v1};

use self::geometry::{
    Axis, CanonicalAssignment, apply_translated_assignment, assign_axis_length, assign_axis_pair,
    assign_outward_edge, assign_two_edge_lengths, assignment_points_are_distinct, ordinary_or_zero,
    ordinary_positive,
};
use super::CurrentRuntimeExactConstraintAssignmentV1;
use crate::{
    ConstraintPreflightV1, GeometricConstraintLimitsV1,
    certify_binary64_exact_geometric_constraint_satisfaction_v1, prepare_geometric_constraints_v1,
};

mod algebraic;
mod cardinal_rotation;
pub(super) mod geometry;

pub(crate) use algebraic::{
    MAX_PAIR_CONSTRAINT_ALGEBRAIC_CANDIDATES_V1,
    construct_pair_constraint_algebraic_exact_assignment_v1,
};
use cardinal_rotation::construct_cardinal_rotation_pair_candidate_v1;
pub(crate) const MAX_PAIR_CONSTRAINT_CONSTRUCTIVE_CANDIDATES_V1: usize = 4;

/// Tries a fixed, deliberately incomplete language of exact assignments for
/// two validated constraints.
///
/// Ordinary templates are deterministic from canonical IDs and try only four
/// fixed translations. The exact-cardinal rotation deletion templates use one
/// origin-bound candidate and deliberately admit a positive subnormal radius.
/// The two-rotation zero-radius escape is not a valid crease-pattern assignment
/// and is handled only by the separate residual-only algebraic overlay path.
/// Every template is merely a candidate: the complete frozen production
/// residual API revalidates the whole two-record document before an opaque
/// positive assignment can escape. Unsupported relations, invalid inputs, and
/// conflicting roles therefore fail closed as `None`.
pub(crate) fn construct_pair_constraint_exact_assignment_v1(
    pattern: &CreasePattern,
    document: &GeometricConstraintDocumentV1,
) -> Option<CurrentRuntimeExactConstraintAssignmentV1> {
    if document.constraints.len() != 2 {
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

    let first = &document.constraints[0].constraint;
    let second = &document.constraints[1].constraint;
    if let Some(candidate) = construct_cardinal_rotation_pair_candidate_v1(pattern, first, second) {
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
    let assignment = pair_canonical_assignment(pattern, first, second)
        .or_else(|| pair_canonical_assignment(pattern, second, first))?;
    if !assignment_points_are_distinct(&assignment) {
        return None;
    }

    let offsets: [Point2; MAX_PAIR_CONSTRAINT_CONSTRUCTIVE_CANDIDATES_V1] = [
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

pub(super) fn pair_canonical_assignment(
    pattern: &CreasePattern,
    first: &GeometricConstraintKindV1,
    second: &GeometricConstraintKindV1,
) -> Option<CanonicalAssignment> {
    let mut assignment = CanonicalAssignment::new();
    match (first, second) {
        (
            GeometricConstraintKindV1::FixedLength { edge, length_mm },
            GeometricConstraintKindV1::Horizontal { edge: axis_edge },
        ) if edge == axis_edge => assign_axis_length(
            pattern,
            &mut assignment,
            *edge,
            *length_mm,
            Axis::Horizontal,
        )?,
        (
            GeometricConstraintKindV1::FixedLength { edge, length_mm },
            GeometricConstraintKindV1::Vertical { edge: axis_edge },
        ) if edge == axis_edge => {
            assign_axis_length(pattern, &mut assignment, *edge, *length_mm, Axis::Vertical)?
        }
        (
            GeometricConstraintKindV1::FixedLength {
                edge: first_edge,
                length_mm: first_length,
            },
            GeometricConstraintKindV1::FixedLength {
                edge: second_edge,
                length_mm: second_length,
            },
        ) => assign_two_edge_lengths(
            pattern,
            &mut assignment,
            (*first_edge, *first_length),
            (*second_edge, *second_length),
        )?,
        (
            GeometricConstraintKindV1::EqualLength {
                first_edge,
                second_edge,
            },
            GeometricConstraintKindV1::FixedLength { edge, length_mm },
        ) if edge == first_edge || edge == second_edge => assign_two_edge_lengths(
            pattern,
            &mut assignment,
            (*first_edge, *length_mm),
            (*second_edge, *length_mm),
        )?,
        (
            GeometricConstraintKindV1::LengthRatio {
                numerator_edge,
                denominator_edge,
                ratio,
            },
            GeometricConstraintKindV1::FixedLength { edge, length_mm },
        ) => {
            if !ordinary_positive(*ratio) || !ordinary_positive(*length_mm) {
                return None;
            }
            let (numerator_length, denominator_length) = if edge == denominator_edge {
                (ratio * length_mm, *length_mm)
            } else if edge == numerator_edge {
                (*length_mm, length_mm / ratio)
            } else {
                return None;
            };
            assign_two_edge_lengths(
                pattern,
                &mut assignment,
                (*numerator_edge, numerator_length),
                (*denominator_edge, denominator_length),
            )?;
        }
        (
            GeometricConstraintKindV1::Parallel {
                first_edge,
                second_edge,
            },
            GeometricConstraintKindV1::Horizontal { edge },
        ) if edge == first_edge || edge == second_edge => assign_axis_pair(
            pattern,
            &mut assignment,
            (*first_edge, Axis::Horizontal),
            (*second_edge, Axis::Horizontal),
        )?,
        (
            GeometricConstraintKindV1::Parallel {
                first_edge,
                second_edge,
            },
            GeometricConstraintKindV1::Vertical { edge },
        ) if edge == first_edge || edge == second_edge => assign_axis_pair(
            pattern,
            &mut assignment,
            (*first_edge, Axis::Vertical),
            (*second_edge, Axis::Vertical),
        )?,
        (
            GeometricConstraintKindV1::FixedAngle {
                vertex,
                first_edge,
                second_edge,
                angle_degrees,
            },
            GeometricConstraintKindV1::Horizontal { edge },
        ) => assign_angle_with_axis(
            pattern,
            &mut assignment,
            *vertex,
            *first_edge,
            *second_edge,
            *angle_degrees,
            *edge,
            Axis::Horizontal,
        )?,
        (
            GeometricConstraintKindV1::FixedAngle {
                vertex,
                first_edge,
                second_edge,
                angle_degrees,
            },
            GeometricConstraintKindV1::Vertical { edge },
        ) => assign_angle_with_axis(
            pattern,
            &mut assignment,
            *vertex,
            *first_edge,
            *second_edge,
            *angle_degrees,
            *edge,
            Axis::Vertical,
        )?,
        (
            GeometricConstraintKindV1::Horizontal { edge: first_edge },
            GeometricConstraintKindV1::Horizontal { edge: second_edge },
        ) => assign_axis_pair(
            pattern,
            &mut assignment,
            (*first_edge, Axis::Horizontal),
            (*second_edge, Axis::Horizontal),
        )?,
        (
            GeometricConstraintKindV1::Vertical { edge: first_edge },
            GeometricConstraintKindV1::Vertical { edge: second_edge },
        ) => assign_axis_pair(
            pattern,
            &mut assignment,
            (*first_edge, Axis::Vertical),
            (*second_edge, Axis::Vertical),
        )?,
        (
            GeometricConstraintKindV1::Horizontal {
                edge: horizontal_edge,
            },
            GeometricConstraintKindV1::Vertical {
                edge: vertical_edge,
            },
        ) => assign_axis_pair(
            pattern,
            &mut assignment,
            (*horizontal_edge, Axis::Horizontal),
            (*vertical_edge, Axis::Vertical),
        )?,
        _ => return None,
    }
    Some(assignment)
}

#[allow(clippy::too_many_arguments)]
fn assign_angle_with_axis(
    pattern: &CreasePattern,
    assignment: &mut CanonicalAssignment,
    vertex: ori_domain::VertexId,
    first_edge: ori_domain::EdgeId,
    second_edge: ori_domain::EdgeId,
    angle_degrees: f64,
    axis_edge: ori_domain::EdgeId,
    axis: Axis,
) -> Option<()> {
    let other_edge = if axis_edge == first_edge {
        second_edge
    } else if axis_edge == second_edge {
        first_edge
    } else {
        return None;
    };
    let radians = deterministic_degrees_to_radians_v1(angle_degrees).ok()?;
    let (sin, cos) = deterministic_sin_cos_degrees_v1(angle_degrees).ok()?;
    if !ordinary_or_zero(angle_degrees)
        || !ordinary_or_zero(radians)
        || !ordinary_or_zero(sin)
        || !ordinary_or_zero(cos)
    {
        return None;
    }
    let axis_vector = axis.unit();
    let scale = if angle_degrees == 0.0 { 2.0 } else { 1.0 };
    let rotated = match axis {
        Axis::Horizontal => Point2::new(scale * cos, scale * sin),
        Axis::Vertical => Point2::new(-scale * sin, scale * cos),
    };
    assign_outward_edge(pattern, assignment, vertex, axis_edge, axis_vector)?;
    assign_outward_edge(pattern, assignment, vertex, other_edge, rotated)
}
