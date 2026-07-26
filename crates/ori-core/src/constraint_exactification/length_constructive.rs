use std::collections::{BTreeMap, BTreeSet};

use ori_domain::{
    CreasePattern, EdgeId, GeometricConstraintDocumentV1, GeometricConstraintKindV1, Point2,
    VertexId,
};

use crate::{
    ConstraintPreflightV1, GeometricConstraintLimitsV1,
    constraint_solver::{
        Binary64ResidualOnlyConstraintSatisfactionV1,
        certify_binary64_residual_only_constraint_overlay_v1,
    },
    constraints::length_ratio_residual_binary64_v1,
    prepare_geometric_constraints_v1,
};

pub(crate) const MAX_LENGTH_CONSTRAINT_CONSTRUCTIVE_CONSTRAINTS_V1: usize = 16;
pub(crate) const MAX_LENGTH_CONSTRAINT_CONSTRUCTIVE_EDGES_V1: usize = 32;
const MAX_LENGTH_CONSTRAINT_PROPAGATION_PASSES_V1: usize =
    MAX_LENGTH_CONSTRAINT_CONSTRUCTIVE_EDGES_V1;

type CanonicalId = [u8; 16];

struct ScalarEdge {
    id: EdgeId,
    start: VertexId,
    end: VertexId,
}

/// Constructs one bounded residual-only assignment for a validated,
/// length-only document over pairwise vertex-disjoint source edges.
///
/// This is deliberately not a crease-pattern assignment. Every scalar is
/// propagated in canonical constraint order, embedded in a complete finite
/// vertex overlay, and accepted only after the unchanged production residual
/// evaluator reports exact binary64 zero for the entire original document.
pub(crate) fn construct_length_constraint_residual_exact_assignment_v1(
    pattern: &CreasePattern,
    document: &GeometricConstraintDocumentV1,
) -> Option<Binary64ResidualOnlyConstraintSatisfactionV1> {
    construct_with_pass_limit(
        pattern,
        document,
        MAX_LENGTH_CONSTRAINT_PROPAGATION_PASSES_V1,
    )
}

#[cfg(test)]
pub(crate) fn construct_length_constraint_residual_exact_assignment_with_pass_limit_for_test(
    pattern: &CreasePattern,
    document: &GeometricConstraintDocumentV1,
    maximum_passes: usize,
) -> Option<Binary64ResidualOnlyConstraintSatisfactionV1> {
    construct_with_pass_limit(pattern, document, maximum_passes)
}

fn construct_with_pass_limit(
    pattern: &CreasePattern,
    document: &GeometricConstraintDocumentV1,
    maximum_passes: usize,
) -> Option<Binary64ResidualOnlyConstraintSatisfactionV1> {
    if document.constraints.is_empty()
        || document.constraints.len() > MAX_LENGTH_CONSTRAINT_CONSTRUCTIVE_CONSTRAINTS_V1
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

    let referenced = referenced_length_edges(prepared.constraints())?;
    if referenced.len() > MAX_LENGTH_CONSTRAINT_CONSTRUCTIVE_EDGES_V1 {
        return None;
    }
    let scalar_edges = validated_matching_edges(pattern, referenced)?;
    let mut values = vec![None; scalar_edges.len()];
    seed_fixed_lengths(prepared.constraints(), &scalar_edges, &mut values)?;
    propagate_lengths(
        prepared.constraints(),
        &scalar_edges,
        &mut values,
        maximum_passes,
    )?;
    verify_scalar_residuals(prepared.constraints(), &scalar_edges, &values)?;
    let overlay = complete_overlay(pattern, &scalar_edges, &values)?;
    certify_binary64_residual_only_constraint_overlay_v1(pattern, document, &overlay)
        .ok()
        .flatten()
}

fn referenced_length_edges(
    constraints: &[ori_domain::GeometricConstraintRecordV1],
) -> Option<BTreeMap<CanonicalId, EdgeId>> {
    let mut edges = BTreeMap::new();
    for record in constraints {
        match record.constraint {
            GeometricConstraintKindV1::FixedLength { edge, .. } => {
                edges.insert(edge.canonical_bytes(), edge);
            }
            GeometricConstraintKindV1::EqualLength {
                first_edge,
                second_edge,
            } => {
                edges.insert(first_edge.canonical_bytes(), first_edge);
                edges.insert(second_edge.canonical_bytes(), second_edge);
            }
            GeometricConstraintKindV1::LengthRatio {
                numerator_edge,
                denominator_edge,
                ..
            } => {
                edges.insert(numerator_edge.canonical_bytes(), numerator_edge);
                edges.insert(denominator_edge.canonical_bytes(), denominator_edge);
            }
            _ => return None,
        }
    }
    Some(edges)
}

fn validated_matching_edges(
    pattern: &CreasePattern,
    referenced: BTreeMap<CanonicalId, EdgeId>,
) -> Option<Vec<ScalarEdge>> {
    let source_edges = pattern
        .edges
        .iter()
        .map(|edge| (edge.id.canonical_bytes(), edge))
        .collect::<BTreeMap<_, _>>();
    let mut occupied_vertices = BTreeSet::new();
    let mut result = Vec::with_capacity(referenced.len());
    for (key, id) in referenced {
        let edge = source_edges.get(&key)?;
        if !occupied_vertices.insert(edge.start.canonical_bytes())
            || !occupied_vertices.insert(edge.end.canonical_bytes())
        {
            return None;
        }
        result.push(ScalarEdge {
            id,
            start: edge.start,
            end: edge.end,
        });
    }
    Some(result)
}

fn seed_fixed_lengths(
    constraints: &[ori_domain::GeometricConstraintRecordV1],
    edges: &[ScalarEdge],
    values: &mut [Option<f64>],
) -> Option<()> {
    for record in constraints {
        if let GeometricConstraintKindV1::FixedLength { edge, length_mm } = record.constraint {
            assign_exact(values, edge_index(edges, edge)?, length_mm)?;
        }
    }
    Some(())
}

fn propagate_lengths(
    constraints: &[ori_domain::GeometricConstraintRecordV1],
    edges: &[ScalarEdge],
    values: &mut [Option<f64>],
    maximum_passes: usize,
) -> Option<()> {
    for _ in 0..maximum_passes {
        let mut changed = false;
        for record in constraints {
            match record.constraint {
                GeometricConstraintKindV1::FixedLength { .. } => {}
                GeometricConstraintKindV1::EqualLength {
                    first_edge,
                    second_edge,
                } => {
                    let first = edge_index(edges, first_edge)?;
                    let second = edge_index(edges, second_edge)?;
                    changed |= propagate_equal(values, first, second)?;
                }
                GeometricConstraintKindV1::LengthRatio {
                    numerator_edge,
                    denominator_edge,
                    ratio,
                } => {
                    let numerator = edge_index(edges, numerator_edge)?;
                    let denominator = edge_index(edges, denominator_edge)?;
                    changed |= propagate_ratio(values, numerator, denominator, ratio)?;
                }
                _ => return None,
            }
        }
        if values.iter().all(Option::is_some) {
            return Some(());
        }
        if !changed {
            for value in values.iter_mut().filter(|value| value.is_none()) {
                *value = Some(0.0);
            }
            return Some(());
        }
    }
    values.iter().all(Option::is_some).then_some(())
}

fn propagate_equal(values: &mut [Option<f64>], first: usize, second: usize) -> Option<bool> {
    match (values[first], values[second]) {
        (Some(left), Some(right)) => (left.to_bits() == right.to_bits()).then_some(false),
        (Some(value), None) => {
            assign_exact(values, second, value)?;
            Some(true)
        }
        (None, Some(value)) => {
            assign_exact(values, first, value)?;
            Some(true)
        }
        (None, None) => Some(false),
    }
}

fn propagate_ratio(
    values: &mut [Option<f64>],
    numerator: usize,
    denominator: usize,
    ratio: f64,
) -> Option<bool> {
    match (values[numerator], values[denominator]) {
        (Some(actual), Some(base)) => {
            let expected = ratio * base;
            (expected.is_finite() && actual.to_bits() == expected.to_bits()).then_some(false)
        }
        (None, Some(base)) => {
            assign_exact(values, numerator, ratio * base)?;
            Some(true)
        }
        (Some(actual), None) => {
            assign_exact(values, denominator, actual / ratio)?;
            Some(true)
        }
        (None, None) => Some(false),
    }
}

fn assign_exact(values: &mut [Option<f64>], index: usize, candidate: f64) -> Option<()> {
    if !candidate.is_finite() || candidate < 0.0 {
        return None;
    }
    let candidate = if candidate == 0.0 { 0.0 } else { candidate };
    match values[index] {
        Some(existing) if existing.to_bits() != candidate.to_bits() => None,
        Some(_) => Some(()),
        None => {
            values[index] = Some(candidate);
            Some(())
        }
    }
}

fn verify_scalar_residuals(
    constraints: &[ori_domain::GeometricConstraintRecordV1],
    edges: &[ScalarEdge],
    values: &[Option<f64>],
) -> Option<()> {
    for record in constraints {
        let residual = match record.constraint {
            GeometricConstraintKindV1::FixedLength { edge, length_mm } => {
                values[edge_index(edges, edge)?]? - length_mm
            }
            GeometricConstraintKindV1::EqualLength {
                first_edge,
                second_edge,
            } => values[edge_index(edges, first_edge)?]? - values[edge_index(edges, second_edge)?]?,
            GeometricConstraintKindV1::LengthRatio {
                numerator_edge,
                denominator_edge,
                ratio,
            } => length_ratio_residual_binary64_v1(
                values[edge_index(edges, numerator_edge)?]?,
                ratio,
                values[edge_index(edges, denominator_edge)?]?,
            ),
            _ => return None,
        };
        if !residual.is_finite() || residual != 0.0 {
            return None;
        }
    }
    Some(())
}

fn complete_overlay(
    pattern: &CreasePattern,
    edges: &[ScalarEdge],
    values: &[Option<f64>],
) -> Option<Vec<(VertexId, Point2)>> {
    let mut assigned = BTreeMap::new();
    for (edge, length) in edges.iter().zip(values) {
        assigned.insert(edge.start.canonical_bytes(), Point2::new(0.0, 0.0));
        assigned.insert(edge.end.canonical_bytes(), Point2::new((*length)?, 0.0));
    }
    Some(
        pattern
            .vertices
            .iter()
            .map(|vertex| {
                (
                    vertex.id,
                    assigned
                        .get(&vertex.id.canonical_bytes())
                        .copied()
                        .unwrap_or(vertex.position),
                )
            })
            .collect(),
    )
}

fn edge_index(edges: &[ScalarEdge], id: EdgeId) -> Option<usize> {
    edges
        .binary_search_by_key(&id.canonical_bytes(), |edge| edge.id.canonical_bytes())
        .ok()
}
