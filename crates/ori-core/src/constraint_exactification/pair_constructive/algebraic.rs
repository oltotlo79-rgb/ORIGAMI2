use std::collections::BTreeSet;

use ori_domain::{
    CreasePattern, EdgeId, GeometricConstraintDocumentV1, GeometricConstraintKindV1, Point2,
};

use crate::{
    ConstraintPreflightV1, GeometricConstraintLimitsV1,
    constraint_solver::{
        Binary64ResidualOnlyConstraintSatisfactionV1,
        certify_binary64_residual_only_constraint_overlay_v1,
    },
    prepare_geometric_constraints_v1,
};

pub(crate) const MAX_PAIR_CONSTRAINT_ALGEBRAIC_CANDIDATES_V1: usize = 4;

/// Constructs only the explicitly supported two-record algebraic collapses.
///
/// The returned value is an algebraic residual witness, never a crease-pattern
/// assignment. All source geometry and topology remain unchanged. Each
/// candidate is a complete overlay over the source vertex registry and is
/// accepted only by the private full production residual certificate. Rotation
/// roles may therefore coincide in the overlay without weakening the ordinary
/// non-degenerate crease-pattern validation boundary.
pub(crate) fn construct_pair_constraint_algebraic_exact_assignment_v1(
    pattern: &CreasePattern,
    document: &GeometricConstraintDocumentV1,
) -> Option<Binary64ResidualOnlyConstraintSatisfactionV1> {
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
    let collapsed_vertices = algebraic_collapsed_vertex_ids(pattern, first, second)?;
    let anchors: [Point2; MAX_PAIR_CONSTRAINT_ALGEBRAIC_CANDIDATES_V1] = [
        Point2::new(0.0, 0.0),
        Point2::new(-0.0, -0.0),
        Point2::new(16.0, 32.0),
        Point2::new(-16.0, 8.0),
    ];
    for anchor in anchors {
        let overlay = pattern
            .vertices
            .iter()
            .map(|vertex| {
                let position = if collapsed_vertices.contains(&vertex.id.canonical_bytes()) {
                    anchor
                } else {
                    vertex.position
                };
                (vertex.id, position)
            })
            .collect::<Vec<_>>();
        let certificate =
            certify_binary64_residual_only_constraint_overlay_v1(pattern, document, &overlay)
                .ok()
                .flatten();
        if certificate.is_some() {
            return certificate;
        }
    }
    None
}

fn algebraic_collapsed_vertex_ids(
    pattern: &CreasePattern,
    first: &GeometricConstraintKindV1,
    second: &GeometricConstraintKindV1,
) -> Option<BTreeSet<[u8; 16]>> {
    if let (
        GeometricConstraintKindV1::RotationalSymmetry {
            center_vertex: first_center,
            source_vertex: first_source,
            target_vertex: first_target,
            ..
        },
        GeometricConstraintKindV1::RotationalSymmetry {
            center_vertex: second_center,
            source_vertex: second_source,
            target_vertex: second_target,
            ..
        },
    ) = (first, second)
    {
        return Some(
            [
                *first_center,
                *first_source,
                *first_target,
                *second_center,
                *second_source,
                *second_target,
            ]
            .into_iter()
            .map(|vertex| vertex.canonical_bytes())
            .collect(),
        );
    }

    let collapsed_edges = algebraic_collapse_edges(first, second)
        .or_else(|| algebraic_collapse_edges(second, first))?;
    collapsed_vertex_ids(pattern, &collapsed_edges)
}

fn algebraic_collapse_edges(
    first: &GeometricConstraintKindV1,
    second: &GeometricConstraintKindV1,
) -> Option<Vec<EdgeId>> {
    match (first, second) {
        (
            GeometricConstraintKindV1::Horizontal { edge: horizontal },
            GeometricConstraintKindV1::Vertical { edge: vertical },
        ) if horizontal == vertical => Some(vec![*horizontal]),
        (
            GeometricConstraintKindV1::LengthRatio {
                numerator_edge: first_numerator,
                denominator_edge: first_denominator,
                ratio: first_ratio,
            },
            GeometricConstraintKindV1::LengthRatio {
                numerator_edge: second_numerator,
                denominator_edge: second_denominator,
                ratio: second_ratio,
            },
        ) if ordinary_positive(*first_ratio)
            && ordinary_positive(*second_ratio)
            && same_unordered_pair(
                *first_numerator,
                *first_denominator,
                *second_numerator,
                *second_denominator,
            ) =>
        {
            Some(vec![*first_numerator, *first_denominator])
        }
        (
            GeometricConstraintKindV1::EqualLength {
                first_edge,
                second_edge,
            },
            GeometricConstraintKindV1::LengthRatio {
                numerator_edge,
                denominator_edge,
                ratio,
            },
        ) if ordinary_positive(*ratio)
            && same_unordered_pair(
                *first_edge,
                *second_edge,
                *numerator_edge,
                *denominator_edge,
            ) =>
        {
            Some(vec![*first_edge, *second_edge])
        }
        _ => None,
    }
}

fn collapsed_vertex_ids(pattern: &CreasePattern, edges: &[EdgeId]) -> Option<BTreeSet<[u8; 16]>> {
    let mut vertices = BTreeSet::new();
    for edge_id in edges {
        let edge = pattern.edges.iter().find(|edge| edge.id == *edge_id)?;
        vertices.insert(edge.start.canonical_bytes());
        vertices.insert(edge.end.canonical_bytes());
    }
    Some(vertices)
}

fn same_unordered_pair(
    first: EdgeId,
    second: EdgeId,
    other_first: EdgeId,
    other_second: EdgeId,
) -> bool {
    (first == other_first && second == other_second)
        || (first == other_second && second == other_first)
}

fn ordinary_positive(value: f64) -> bool {
    value > 0.0 && value.is_normal()
}
