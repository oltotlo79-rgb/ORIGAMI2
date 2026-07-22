use std::collections::{HashMap, HashSet};

use ori_domain::{
    CreasePattern, GeometricConstraintDocumentV1, GeometricConstraintKindV1, Point2, VertexId,
};
use thiserror::Error;

use crate::{GeometricConstraintLimitsV1, prepare_geometric_constraints_v1};

const REGULARIZATION: f64 = 1e-10;
const DERIVATIVE_STEP: f64 = 1e-6;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConstraintSolveLimitsV1 {
    pub max_vertices: usize,
    pub max_constraints: usize,
    pub max_iterations: usize,
    pub max_work: usize,
    pub residual_tolerance: f64,
    pub step_tolerance: f64,
}

impl Default for ConstraintSolveLimitsV1 {
    fn default() -> Self {
        Self {
            max_vertices: 256,
            max_constraints: 1_024,
            max_iterations: 32,
            max_work: 20_000_000,
            residual_tolerance: 1e-7,
            step_tolerance: 1e-9,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConstraintSolvePreviewV1 {
    pub positions: Vec<(VertexId, Point2)>,
    pub iterations: usize,
    pub maximum_residual: f64,
    pub rank: usize,
    pub degrees_of_freedom: usize,
    pub equation_count: usize,
    pub condition_estimate: f64,
}

#[derive(Debug, Clone, PartialEq, Error)]
pub enum ConstraintSolveErrorV1 {
    #[error("solver limits are invalid")]
    InvalidLimits,
    #[error("the driving vertex is missing")]
    DrivingVertexMissing,
    #[error("the driving position is non-finite")]
    NonFiniteDrivingPosition,
    #[error("the constraint document or geometry is invalid")]
    InvalidConstraintDocumentOrGeometry,
    #[error("the system contains a constraint kind not supported by this solver")]
    UnsupportedConstraintKind,
    #[error("the system does not constrain the driving component")]
    UnderConstrained,
    #[error("the solver work limit was exceeded")]
    WorkLimitExceeded,
    #[error("the normal system is rank deficient")]
    RankDeficient,
    #[error("the bounded solver did not converge")]
    NonConvergent,
}

pub fn solve_geometric_constraints_v1(
    pattern: &CreasePattern,
    document: &GeometricConstraintDocumentV1,
    driving_vertex: VertexId,
    driving_position: Point2,
    limits: ConstraintSolveLimitsV1,
) -> Result<ConstraintSolvePreviewV1, ConstraintSolveErrorV1> {
    solve_geometric_constraints_with_drivers_v1(
        pattern,
        document,
        &[(driving_vertex, driving_position)],
        limits,
    )
}

pub fn solve_geometric_constraints_with_drivers_v1(
    pattern: &CreasePattern,
    document: &GeometricConstraintDocumentV1,
    driving_positions: &[(VertexId, Point2)],
    limits: ConstraintSolveLimitsV1,
) -> Result<ConstraintSolvePreviewV1, ConstraintSolveErrorV1> {
    validate_limits(limits)?;
    if driving_positions.is_empty()
        || driving_positions
            .iter()
            .any(|(_, point)| !point.x.is_finite() || !point.y.is_finite())
    {
        return Err(ConstraintSolveErrorV1::NonFiniteDrivingPosition);
    }
    prepare_geometric_constraints_v1(pattern, document, GeometricConstraintLimitsV1::default())
        .map_err(|_| ConstraintSolveErrorV1::InvalidConstraintDocumentOrGeometry)?;
    if pattern.vertices.len() > limits.max_vertices
        || document.constraints.len() > limits.max_constraints
    {
        return Err(ConstraintSolveErrorV1::WorkLimitExceeded);
    }
    let mut positions = pattern
        .vertices
        .iter()
        .map(|vertex| (vertex.id, vertex.position))
        .collect::<HashMap<_, _>>();
    let original = positions.clone();
    let mut drivers = HashSet::with_capacity(driving_positions.len());
    for (vertex, point) in driving_positions {
        if !drivers.insert(*vertex) || positions.insert(*vertex, *point).is_none() {
            return Err(ConstraintSolveErrorV1::DrivingVertexMissing);
        }
    }
    let involved = involved_vertices(pattern, document)?;
    if drivers.iter().any(|vertex| !involved.contains(vertex)) {
        return Err(ConstraintSolveErrorV1::UnderConstrained);
    }
    let mut variables = involved
        .into_iter()
        .filter(|vertex| !drivers.contains(vertex))
        .collect::<Vec<_>>();
    variables.sort_by_key(VertexId::canonical_bytes);
    if variables.is_empty() {
        let residuals = residuals(pattern, document, &positions)?;
        let maximum_residual = maximum_absolute(&residuals);
        return (maximum_residual <= limits.residual_tolerance)
            .then_some(ConstraintSolvePreviewV1 {
                positions: sorted_positions(driving_positions.to_vec()),
                iterations: 0,
                maximum_residual,
                // With no free variables every admitted equation has already
                // been satisfied by the complete driver set. Report the
                // effective solved rank so UI classification does not label
                // a fully determined system over-constrained.
                rank: residuals.len(),
                degrees_of_freedom: 0,
                equation_count: residuals.len(),
                condition_estimate: 1.0,
            })
            .ok_or(ConstraintSolveErrorV1::NonConvergent);
    }
    let dimension = variables
        .len()
        .checked_mul(2)
        .ok_or(ConstraintSolveErrorV1::WorkLimitExceeded)?;
    let mut work = 0usize;
    for iteration in 0..limits.max_iterations {
        let hard = residuals(pattern, document, &positions)?;
        let maximum_residual = maximum_absolute(&hard);
        if maximum_residual <= limits.residual_tolerance {
            let diagnostics = rank_diagnostics(pattern, document, &positions, &variables)?;
            return Ok(ConstraintSolvePreviewV1 {
                positions: sorted_positions(
                    positions
                        .into_iter()
                        .filter(|(vertex, point)| {
                            original.get(vertex).is_none_or(|old| old != point)
                        })
                        .collect(),
                ),
                iterations: iteration,
                maximum_residual,
                rank: diagnostics.0,
                degrees_of_freedom: dimension.saturating_sub(diagnostics.0),
                equation_count: hard.len(),
                condition_estimate: diagnostics.1,
            });
        }
        let rows = hard
            .len()
            .checked_add(dimension)
            .ok_or(ConstraintSolveErrorV1::WorkLimitExceeded)?;
        charge(
            &mut work,
            rows.checked_mul(dimension)
                .and_then(|value| value.checked_mul(dimension))
                .ok_or(ConstraintSolveErrorV1::WorkLimitExceeded)?,
            limits.max_work,
        )?;
        let mut residual = hard;
        let regularization_scale = REGULARIZATION.sqrt();
        for vertex in &variables {
            let point = positions[vertex];
            let base = original[vertex];
            residual.push((point.x - base.x) * regularization_scale);
            residual.push((point.y - base.y) * regularization_scale);
        }
        let mut jacobian = vec![vec![0.0; dimension]; rows];
        for column in 0..dimension {
            let vertex = variables[column / 2];
            let axis = column % 2;
            let mut perturbed = positions.clone();
            let point = perturbed.get_mut(&vertex).expect("indexed variable");
            if axis == 0 {
                point.x += DERIVATIVE_STEP
            } else {
                point.y += DERIVATIVE_STEP
            }
            let shifted = residuals(pattern, document, &perturbed)?;
            for (row, value) in shifted.into_iter().enumerate() {
                jacobian[row][column] = (value - residual[row]) / DERIVATIVE_STEP;
            }
            jacobian[hard_len(document)? + column][column] = regularization_scale;
        }
        let mut normal = vec![vec![0.0; dimension]; dimension];
        let mut rhs = vec![0.0; dimension];
        for row in 0..rows {
            for left in 0..dimension {
                rhs[left] -= jacobian[row][left] * residual[row];
                for right in 0..dimension {
                    normal[left][right] += jacobian[row][left] * jacobian[row][right];
                }
            }
        }
        let delta = solve_dense(normal, rhs)?;
        let maximum_step = maximum_absolute(&delta);
        for (index, vertex) in variables.iter().enumerate() {
            let point = positions.get_mut(vertex).expect("indexed variable");
            point.x += delta[index * 2];
            point.y += delta[index * 2 + 1];
            if !point.x.is_finite() || !point.y.is_finite() {
                return Err(ConstraintSolveErrorV1::NonConvergent);
            }
        }
        let updated = residuals(pattern, document, &positions)?;
        let updated_maximum_residual = maximum_absolute(&updated);
        if updated_maximum_residual <= limits.residual_tolerance {
            let diagnostics = rank_diagnostics(pattern, document, &positions, &variables)?;
            return Ok(ConstraintSolvePreviewV1 {
                positions: sorted_positions(
                    positions
                        .into_iter()
                        .filter(|(vertex, point)| {
                            original.get(vertex).is_none_or(|old| old != point)
                        })
                        .collect(),
                ),
                iterations: iteration + 1,
                maximum_residual: updated_maximum_residual,
                rank: diagnostics.0,
                degrees_of_freedom: dimension.saturating_sub(diagnostics.0),
                equation_count: updated.len(),
                condition_estimate: diagnostics.1,
            });
        }
        if maximum_step <= limits.step_tolerance {
            return Err(ConstraintSolveErrorV1::NonConvergent);
        }
    }
    Err(ConstraintSolveErrorV1::NonConvergent)
}

fn sorted_positions(mut positions: Vec<(VertexId, Point2)>) -> Vec<(VertexId, Point2)> {
    positions.sort_by_key(|(vertex, _)| vertex.canonical_bytes());
    positions
}

fn rank_diagnostics(
    pattern: &CreasePattern,
    document: &GeometricConstraintDocumentV1,
    positions: &HashMap<VertexId, Point2>,
    variables: &[VertexId],
) -> Result<(usize, f64), ConstraintSolveErrorV1> {
    let base = residuals(pattern, document, positions)?;
    let columns = variables.len() * 2;
    let mut matrix = vec![vec![0.0; columns]; base.len()];
    for column in 0..columns {
        let mut shifted_positions = positions.clone();
        let point = shifted_positions
            .get_mut(&variables[column / 2])
            .expect("indexed variable");
        if column % 2 == 0 {
            point.x += DERIVATIVE_STEP;
        } else {
            point.y += DERIVATIVE_STEP;
        }
        for (row, shifted) in residuals(pattern, document, &shifted_positions)?
            .into_iter()
            .enumerate()
        {
            matrix[row][column] = (shifted - base[row]) / DERIVATIVE_STEP;
        }
    }
    let mut rank = 0;
    let mut smallest = f64::INFINITY;
    let mut largest: f64 = 0.0;
    for column in 0..columns {
        let Some(pivot) = (rank..matrix.len()).max_by(|left, right| {
            matrix[*left][column]
                .abs()
                .total_cmp(&matrix[*right][column].abs())
        }) else {
            break;
        };
        let value = matrix[pivot][column].abs();
        if value <= 1e-10 {
            continue;
        }
        matrix.swap(rank, pivot);
        smallest = smallest.min(value);
        largest = largest.max(value);
        let (processed, remaining) = matrix.split_at_mut(rank + 1);
        let pivot_row = &processed[rank];
        for row in remaining {
            let factor = row[column] / pivot_row[column];
            for (value, pivot) in row[column..columns]
                .iter_mut()
                .zip(&pivot_row[column..columns])
            {
                *value -= factor * pivot;
            }
        }
        rank += 1;
        if rank == matrix.len() {
            break;
        }
    }
    Ok((rank, if rank == 0 { 1.0 } else { largest / smallest }))
}

/// Verifies a complete candidate pattern against every solver-supported hard constraint.
///
/// Unsupported, invalid, degenerate, or non-finite systems fail closed.
pub fn verify_geometric_constraint_solution_v1(
    pattern: &CreasePattern,
    document: &GeometricConstraintDocumentV1,
    residual_tolerance: f64,
) -> Result<f64, ConstraintSolveErrorV1> {
    if !residual_tolerance.is_finite() || residual_tolerance <= 0.0 {
        return Err(ConstraintSolveErrorV1::InvalidLimits);
    }
    prepare_geometric_constraints_v1(pattern, document, GeometricConstraintLimitsV1::default())
        .map_err(|_| ConstraintSolveErrorV1::InvalidConstraintDocumentOrGeometry)?;
    hard_len(document)?;
    let positions = pattern
        .vertices
        .iter()
        .map(|vertex| (vertex.id, vertex.position))
        .collect::<HashMap<_, _>>();
    let maximum = maximum_absolute(&residuals(pattern, document, &positions)?);
    if maximum <= residual_tolerance {
        Ok(maximum)
    } else {
        Err(ConstraintSolveErrorV1::NonConvergent)
    }
}

fn validate_limits(limits: ConstraintSolveLimitsV1) -> Result<(), ConstraintSolveErrorV1> {
    if limits.max_vertices == 0
        || limits.max_vertices > 256
        || limits.max_constraints == 0
        || limits.max_constraints > 1_024
        || limits.max_iterations == 0
        || limits.max_iterations > 32
        || limits.max_work == 0
        || limits.max_work > 20_000_000
        || !limits.residual_tolerance.is_finite()
        || limits.residual_tolerance <= 0.0
        || !limits.step_tolerance.is_finite()
        || limits.step_tolerance <= 0.0
    {
        return Err(ConstraintSolveErrorV1::InvalidLimits);
    }
    Ok(())
}

fn charge(work: &mut usize, amount: usize, maximum: usize) -> Result<(), ConstraintSolveErrorV1> {
    *work = work
        .checked_add(amount)
        .ok_or(ConstraintSolveErrorV1::WorkLimitExceeded)?;
    if *work > maximum {
        Err(ConstraintSolveErrorV1::WorkLimitExceeded)
    } else {
        Ok(())
    }
}

fn hard_len(document: &GeometricConstraintDocumentV1) -> Result<usize, ConstraintSolveErrorV1> {
    for record in &document.constraints {
        match record.constraint {
            GeometricConstraintKindV1::FixedLength { .. }
            | GeometricConstraintKindV1::FixedAngle { .. }
            | GeometricConstraintKindV1::Horizontal { .. }
            | GeometricConstraintKindV1::Vertical { .. }
            | GeometricConstraintKindV1::EqualLength { .. }
            | GeometricConstraintKindV1::Parallel { .. }
            | GeometricConstraintKindV1::PointOnLine { .. }
            | GeometricConstraintKindV1::LengthRatio { .. }
            | GeometricConstraintKindV1::MirrorSymmetry { .. }
            | GeometricConstraintKindV1::RotationalSymmetry { .. }
            | GeometricConstraintKindV1::AngleBisector { .. } => {}
        }
    }
    document
        .constraints
        .iter()
        .try_fold(0usize, |count, record| {
            count
                .checked_add(match record.constraint {
                    GeometricConstraintKindV1::MirrorSymmetry { .. }
                    | GeometricConstraintKindV1::RotationalSymmetry { .. } => 2,
                    _ => 1,
                })
                .ok_or(ConstraintSolveErrorV1::WorkLimitExceeded)
        })
}

fn involved_vertices(
    pattern: &CreasePattern,
    document: &GeometricConstraintDocumentV1,
) -> Result<HashSet<VertexId>, ConstraintSolveErrorV1> {
    hard_len(document)?;
    let edges = pattern
        .edges
        .iter()
        .map(|edge| (edge.id, edge))
        .collect::<HashMap<_, _>>();
    let mut result = HashSet::new();
    for record in &document.constraints {
        match record.constraint {
            GeometricConstraintKindV1::FixedLength { edge, .. }
            | GeometricConstraintKindV1::Horizontal { edge }
            | GeometricConstraintKindV1::Vertical { edge } => {
                add_edge_vertices(&edges, &mut result, edge)
            }
            GeometricConstraintKindV1::EqualLength {
                first_edge,
                second_edge,
            }
            | GeometricConstraintKindV1::Parallel {
                first_edge,
                second_edge,
            } => {
                add_edge_vertices(&edges, &mut result, first_edge);
                add_edge_vertices(&edges, &mut result, second_edge);
            }
            GeometricConstraintKindV1::PointOnLine { vertex, line_edge } => {
                result.insert(vertex);
                add_edge_vertices(&edges, &mut result, line_edge);
            }
            GeometricConstraintKindV1::LengthRatio {
                numerator_edge,
                denominator_edge,
                ..
            } => {
                add_edge_vertices(&edges, &mut result, numerator_edge);
                add_edge_vertices(&edges, &mut result, denominator_edge);
            }
            GeometricConstraintKindV1::FixedAngle {
                vertex,
                first_edge,
                second_edge,
                ..
            } => {
                result.insert(vertex);
                add_edge_vertices(&edges, &mut result, first_edge);
                add_edge_vertices(&edges, &mut result, second_edge);
            }
            GeometricConstraintKindV1::MirrorSymmetry {
                first_vertex,
                second_vertex,
                axis_edge,
            } => {
                result.insert(first_vertex);
                result.insert(second_vertex);
                add_edge_vertices(&edges, &mut result, axis_edge);
            }
            GeometricConstraintKindV1::RotationalSymmetry {
                center_vertex,
                source_vertex,
                target_vertex,
                ..
            } => {
                result.extend([center_vertex, source_vertex, target_vertex]);
            }
            GeometricConstraintKindV1::AngleBisector {
                vertex,
                first_edge,
                second_edge,
                bisector_edge,
            } => {
                result.insert(vertex);
                add_edge_vertices(&edges, &mut result, first_edge);
                add_edge_vertices(&edges, &mut result, second_edge);
                add_edge_vertices(&edges, &mut result, bisector_edge);
            }
        }
    }
    Ok(result)
}

fn add_edge_vertices(
    edges: &HashMap<ori_domain::EdgeId, &ori_domain::Edge>,
    result: &mut HashSet<VertexId>,
    id: ori_domain::EdgeId,
) {
    let edge = edges[&id];
    result.insert(edge.start);
    result.insert(edge.end);
}

fn residuals(
    pattern: &CreasePattern,
    document: &GeometricConstraintDocumentV1,
    positions: &HashMap<VertexId, Point2>,
) -> Result<Vec<f64>, ConstraintSolveErrorV1> {
    let edges = pattern
        .edges
        .iter()
        .map(|edge| (edge.id, edge))
        .collect::<HashMap<_, _>>();
    let vector = |edge_id| {
        let edge = edges[&edge_id];
        let start = positions[&edge.start];
        let end = positions[&edge.end];
        (end.x - start.x, end.y - start.y)
    };
    let outward_vector = |edge_id, vertex_id| {
        let edge = edges[&edge_id];
        let vertex = positions[&vertex_id];
        let opposite = if edge.start == vertex_id {
            positions[&edge.end]
        } else {
            positions[&edge.start]
        };
        (opposite.x - vertex.x, opposite.y - vertex.y)
    };
    let length = |edge_id| {
        let (x, y) = vector(edge_id);
        x.hypot(y)
    };
    document
        .constraints
        .iter()
        .map(|record| {
            let values = match record.constraint {
                GeometricConstraintKindV1::FixedLength { edge, length_mm } => {
                    vec![length(edge) - length_mm]
                }
                GeometricConstraintKindV1::Horizontal { edge } => vec![vector(edge).1],
                GeometricConstraintKindV1::Vertical { edge } => vec![vector(edge).0],
                GeometricConstraintKindV1::EqualLength {
                    first_edge,
                    second_edge,
                } => vec![length(first_edge) - length(second_edge)],
                GeometricConstraintKindV1::Parallel {
                    first_edge,
                    second_edge,
                } => {
                    let first = vector(first_edge);
                    let second = vector(second_edge);
                    vec![
                        (first.0 * second.1 - first.1 * second.0)
                            / (first.0.hypot(first.1) * second.0.hypot(second.1)),
                    ]
                }
                GeometricConstraintKindV1::PointOnLine { vertex, line_edge } => {
                    let edge = edges[&line_edge];
                    let start = positions[&edge.start];
                    let point = positions[&vertex];
                    let direction = vector(line_edge);
                    vec![
                        ((point.x - start.x) * direction.1 - (point.y - start.y) * direction.0)
                            / direction.0.hypot(direction.1),
                    ]
                }
                GeometricConstraintKindV1::LengthRatio {
                    numerator_edge,
                    denominator_edge,
                    ratio,
                } => vec![length(numerator_edge) - ratio * length(denominator_edge)],
                GeometricConstraintKindV1::FixedAngle {
                    vertex,
                    first_edge,
                    second_edge,
                    angle_degrees,
                } => {
                    let first = outward_vector(first_edge, vertex);
                    let second = outward_vector(second_edge, vertex);
                    let actual = (first.0 * second.1 - first.1 * second.0)
                        .atan2(first.0 * second.0 + first.1 * second.1);
                    vec![wrap_angle(actual - angle_degrees.to_radians())]
                }
                GeometricConstraintKindV1::MirrorSymmetry {
                    first_vertex,
                    second_vertex,
                    axis_edge,
                } => {
                    let axis = edges[&axis_edge];
                    let origin = positions[&axis.start];
                    let direction = vector(axis_edge);
                    let norm = direction.0 * direction.0 + direction.1 * direction.1;
                    let first = positions[&first_vertex];
                    let projection = ((first.x - origin.x) * direction.0
                        + (first.y - origin.y) * direction.1)
                        / norm;
                    let reflected = Point2::new(
                        2.0 * (origin.x + projection * direction.0) - first.x,
                        2.0 * (origin.y + projection * direction.1) - first.y,
                    );
                    let second = positions[&second_vertex];
                    vec![second.x - reflected.x, second.y - reflected.y]
                }
                GeometricConstraintKindV1::RotationalSymmetry {
                    center_vertex,
                    source_vertex,
                    target_vertex,
                    angle_degrees,
                } => {
                    let center = positions[&center_vertex];
                    let source = positions[&source_vertex];
                    let target = positions[&target_vertex];
                    let angle = angle_degrees.to_radians();
                    let x = source.x - center.x;
                    let y = source.y - center.y;
                    vec![
                        target.x - center.x - (x * angle.cos() - y * angle.sin()),
                        target.y - center.y - (x * angle.sin() + y * angle.cos()),
                    ]
                }
                GeometricConstraintKindV1::AngleBisector {
                    vertex,
                    first_edge,
                    second_edge,
                    bisector_edge,
                } => {
                    let first = outward_vector(first_edge, vertex);
                    let second = outward_vector(second_edge, vertex);
                    let bisector = outward_vector(bisector_edge, vertex);
                    let sum_x =
                        first.0 / first.0.hypot(first.1) + second.0 / second.0.hypot(second.1);
                    let sum_y =
                        first.1 / first.0.hypot(first.1) + second.1 / second.0.hypot(second.1);
                    vec![
                        (sum_x * bisector.1 - sum_y * bisector.0)
                            / (sum_x.hypot(sum_y) * bisector.0.hypot(bisector.1)),
                    ]
                }
            };
            if values.iter().all(|value| value.is_finite()) {
                Ok(values)
            } else {
                Err(ConstraintSolveErrorV1::NonConvergent)
            }
        })
        .collect::<Result<Vec<_>, _>>()
        .map(|rows| rows.into_iter().flatten().collect())
}

fn wrap_angle(angle: f64) -> f64 {
    (angle + std::f64::consts::PI).rem_euclid(2.0 * std::f64::consts::PI) - std::f64::consts::PI
}

fn maximum_absolute(values: &[f64]) -> f64 {
    values
        .iter()
        .fold(0.0, |maximum, value| maximum.max(value.abs()))
}

fn solve_dense(
    mut matrix: Vec<Vec<f64>>,
    mut rhs: Vec<f64>,
) -> Result<Vec<f64>, ConstraintSolveErrorV1> {
    let dimension = rhs.len();
    for column in 0..dimension {
        let pivot = (column..dimension)
            .max_by(|left, right| {
                matrix[*left][column]
                    .abs()
                    .total_cmp(&matrix[*right][column].abs())
            })
            .expect("nonempty pivot range");
        if matrix[pivot][column].abs() <= 1e-14 {
            return Err(ConstraintSolveErrorV1::RankDeficient);
        }
        matrix.swap(column, pivot);
        rhs.swap(column, pivot);
        let divisor = matrix[column][column];
        for value in &mut matrix[column][column..] {
            *value /= divisor;
        }
        rhs[column] /= divisor;
        let pivot_row = matrix[column].clone();
        for (row_index, row) in matrix.iter_mut().enumerate() {
            if row_index == column {
                continue;
            }
            let factor = row[column];
            for (value, pivot) in row[column..dimension]
                .iter_mut()
                .zip(&pivot_row[column..dimension])
            {
                *value -= factor * pivot;
            }
            rhs[row_index] -= factor * rhs[column];
        }
    }
    if rhs.iter().all(|value| value.is_finite()) {
        Ok(rhs)
    } else {
        Err(ConstraintSolveErrorV1::NonConvergent)
    }
}

#[cfg(test)]
mod tests {
    use ori_domain::{
        ConstraintId, Edge, EdgeId, EdgeKind, GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
        GeometricConstraintRecordV1, Vertex,
    };

    use super::*;

    fn single_edge(
        start: Point2,
        end: Point2,
        constraints: impl FnOnce(EdgeId) -> Vec<GeometricConstraintKindV1>,
    ) -> (CreasePattern, GeometricConstraintDocumentV1, VertexId) {
        let start_id = VertexId::new();
        let end_id = VertexId::new();
        let edge_id = EdgeId::new();
        let pattern = CreasePattern {
            vertices: vec![
                Vertex {
                    id: start_id,
                    position: start,
                },
                Vertex {
                    id: end_id,
                    position: end,
                },
            ],
            edges: vec![Edge {
                id: edge_id,
                start: start_id,
                end: end_id,
                kind: EdgeKind::Auxiliary,
            }],
        };
        let document = GeometricConstraintDocumentV1 {
            schema_version: GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
            constraints: constraints(edge_id)
                .into_iter()
                .map(|constraint| GeometricConstraintRecordV1 {
                    id: ConstraintId::new(),
                    constraint,
                })
                .collect(),
        };
        (pattern, document, start_id)
    }

    #[test]
    fn fixed_angle_uses_vectors_pointing_outward_from_the_declared_vertex() {
        let center = VertexId::new();
        let x = VertexId::new();
        let y = VertexId::new();
        let reversed_x = EdgeId::new();
        let forward_y = EdgeId::new();
        let pattern = CreasePattern {
            vertices: vec![
                Vertex {
                    id: center,
                    position: Point2::new(0.0, 0.0),
                },
                Vertex {
                    id: x,
                    position: Point2::new(1.0, 0.0),
                },
                Vertex {
                    id: y,
                    position: Point2::new(0.0, 1.0),
                },
            ],
            edges: vec![
                Edge {
                    id: reversed_x,
                    start: x,
                    end: center,
                    kind: EdgeKind::Auxiliary,
                },
                Edge {
                    id: forward_y,
                    start: center,
                    end: y,
                    kind: EdgeKind::Auxiliary,
                },
            ],
        };
        let document = GeometricConstraintDocumentV1 {
            schema_version: GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
            constraints: vec![GeometricConstraintRecordV1 {
                id: ConstraintId::new(),
                constraint: GeometricConstraintKindV1::FixedAngle {
                    vertex: center,
                    first_edge: reversed_x,
                    second_edge: forward_y,
                    angle_degrees: 90.0,
                },
            }],
        };
        let positions = pattern
            .vertices
            .iter()
            .map(|vertex| (vertex.id, vertex.position))
            .collect();

        assert!(maximum_absolute(&residuals(&pattern, &document, &positions).unwrap()) < 1e-12);
    }

    #[test]
    fn horizontal_constraint_follows_driving_vertex_without_mutating_input() {
        let (pattern, document, driving) = single_edge(
            Point2 { x: 0.0, y: 0.0 },
            Point2 { x: 4.0, y: 0.0 },
            |edge| vec![GeometricConstraintKindV1::Horizontal { edge }],
        );

        let preview = solve_geometric_constraints_v1(
            &pattern,
            &document,
            driving,
            Point2 { x: 1.0, y: 3.0 },
            ConstraintSolveLimitsV1::default(),
        )
        .expect("bounded solve");

        assert!(preview.maximum_residual <= 1e-7);
        assert_eq!(pattern.vertices[0].position, Point2 { x: 0.0, y: 0.0 });
        assert!(
            preview
                .positions
                .iter()
                .any(|(id, point)| *id != driving && (point.y - 3.0).abs() <= 1e-7)
        );
    }

    #[test]
    fn final_allowed_iteration_can_report_newly_converged_solution() {
        let (pattern, document, driving) = single_edge(
            Point2 { x: 0.0, y: 0.0 },
            Point2 { x: 4.0, y: 0.0 },
            |edge| vec![GeometricConstraintKindV1::Horizontal { edge }],
        );
        let limits = ConstraintSolveLimitsV1 {
            max_iterations: 1,
            ..ConstraintSolveLimitsV1::default()
        };

        let preview = solve_geometric_constraints_v1(
            &pattern,
            &document,
            driving,
            Point2 { x: 1.0, y: 3.0 },
            limits,
        )
        .expect("the first and final update converges");
        assert_eq!(preview.iterations, 1);
        assert!(preview.maximum_residual <= limits.residual_tolerance);
    }

    #[test]
    fn complete_driver_set_is_not_reported_over_constrained() {
        let (pattern, document, first) = single_edge(
            Point2 { x: 0.0, y: 0.0 },
            Point2 { x: 4.0, y: 0.0 },
            |edge| vec![GeometricConstraintKindV1::Horizontal { edge }],
        );
        let second = pattern.vertices[1].id;
        let preview = solve_geometric_constraints_with_drivers_v1(
            &pattern,
            &document,
            &[
                (first, Point2 { x: 1.0, y: 2.0 }),
                (second, Point2 { x: 5.0, y: 2.0 }),
            ],
            ConstraintSolveLimitsV1::default(),
        )
        .expect("complete drivers satisfy the equation");

        assert_eq!(preview.rank, preview.equation_count);
        assert_eq!(preview.degrees_of_freedom, 0);
    }

    #[test]
    fn unsupported_and_tiny_work_budget_fail_closed() {
        let (pattern, unsupported, driving) = single_edge(
            Point2 { x: 0.0, y: 0.0 },
            Point2 { x: 4.0, y: 0.0 },
            |edge| {
                vec![GeometricConstraintKindV1::FixedAngle {
                    vertex: driving_placeholder(),
                    first_edge: edge,
                    second_edge: edge,
                    angle_degrees: 0.0,
                }]
            },
        );
        assert!(matches!(
            solve_geometric_constraints_v1(
                &pattern,
                &unsupported,
                driving,
                Point2 { x: 1.0, y: 1.0 },
                ConstraintSolveLimitsV1::default()
            ),
            Err(ConstraintSolveErrorV1::InvalidConstraintDocumentOrGeometry)
                | Err(ConstraintSolveErrorV1::UnsupportedConstraintKind)
        ));

        let (pattern, document, driving) = single_edge(
            Point2 { x: 0.0, y: 0.0 },
            Point2 { x: 4.0, y: 0.0 },
            |edge| vec![GeometricConstraintKindV1::Horizontal { edge }],
        );
        let limits = ConstraintSolveLimitsV1 {
            max_work: 1,
            ..ConstraintSolveLimitsV1::default()
        };
        assert_eq!(
            solve_geometric_constraints_v1(
                &pattern,
                &document,
                driving,
                Point2 { x: 0.0, y: 2.0 },
                limits
            ),
            Err(ConstraintSolveErrorV1::WorkLimitExceeded)
        );
    }

    #[test]
    fn two_vertex_driver_supports_edge_rotation_and_length_change() {
        let (pattern, document, start) = single_edge(
            Point2 { x: 0.0, y: 0.0 },
            Point2 { x: 4.0, y: 0.0 },
            |edge| vec![GeometricConstraintKindV1::Vertical { edge }],
        );
        let end = pattern.vertices[1].id;
        let preview = solve_geometric_constraints_with_drivers_v1(
            &pattern,
            &document,
            &[
                (start, Point2 { x: 3.0, y: 2.0 }),
                (end, Point2 { x: 3.0, y: 9.0 }),
            ],
            ConstraintSolveLimitsV1::default(),
        )
        .expect("vertical translated, rotated, and resized edge");
        assert_eq!(preview.positions.len(), 2);
        assert_eq!(preview.maximum_residual, 0.0);
        assert_eq!(preview.degrees_of_freedom, 0);
    }

    #[test]
    fn constraint_input_order_does_not_change_the_solution() {
        let (pattern, mut document, driving) = single_edge(
            Point2 { x: 0.0, y: 0.0 },
            Point2 { x: 4.0, y: 0.0 },
            |edge| {
                vec![
                    GeometricConstraintKindV1::Horizontal { edge },
                    GeometricConstraintKindV1::FixedLength {
                        edge,
                        length_mm: 4.0,
                    },
                ]
            },
        );
        let first = solve_geometric_constraints_v1(
            &pattern,
            &document,
            driving,
            Point2 { x: 2.0, y: 3.0 },
            ConstraintSolveLimitsV1::default(),
        )
        .unwrap();
        document.constraints.reverse();
        let second = solve_geometric_constraints_v1(
            &pattern,
            &document,
            driving,
            Point2 { x: 2.0, y: 3.0 },
            ConstraintSolveLimitsV1::default(),
        )
        .unwrap();
        assert_eq!(first.positions, second.positions);
        assert_eq!(first.rank, second.rank);
    }

    #[test]
    fn contradiction_degeneracy_nonfinite_and_ten_thousand_vertices_fail_closed() {
        let (pattern, document, start) = single_edge(
            Point2 { x: 0.0, y: 0.0 },
            Point2 { x: 1.0, y: 0.0 },
            |edge| {
                vec![GeometricConstraintKindV1::FixedLength {
                    edge,
                    length_mm: 1.0,
                }]
            },
        );
        let end = pattern.vertices[1].id;
        assert!(matches!(
            solve_geometric_constraints_with_drivers_v1(
                &pattern,
                &document,
                &[
                    (start, Point2 { x: 0.0, y: 0.0 }),
                    (end, Point2 { x: 2.0, y: 0.0 }),
                ],
                ConstraintSolveLimitsV1::default(),
            ),
            Err(ConstraintSolveErrorV1::NonConvergent)
        ));
        assert!(matches!(
            solve_geometric_constraints_v1(
                &pattern,
                &document,
                start,
                Point2 {
                    x: f64::NAN,
                    y: 0.0
                },
                ConstraintSolveLimitsV1::default(),
            ),
            Err(ConstraintSolveErrorV1::NonFiniteDrivingPosition)
        ));

        let mut large = CreasePattern::empty();
        large.vertices = (0..10_000)
            .map(|index| Vertex {
                id: VertexId::new(),
                position: Point2::new(index as f64, 0.0),
            })
            .collect();
        let started = std::time::Instant::now();
        assert!(matches!(
            solve_geometric_constraints_v1(
                &large,
                &GeometricConstraintDocumentV1::default(),
                large.vertices[0].id,
                Point2::new(0.0, 0.0),
                ConstraintSolveLimitsV1::default(),
            ),
            Err(ConstraintSolveErrorV1::WorkLimitExceeded)
        ));
        assert!(
            started.elapsed() < std::time::Duration::from_secs(2),
            "10,000-element admission must remain bounded"
        );
    }

    #[test]
    fn every_v1_constraint_kind_has_a_dedicated_converged_fixture() {
        let center = VertexId::new();
        let x = VertexId::new();
        let y = VertexId::new();
        let diagonal = VertexId::new();
        let negative_y = VertexId::new();
        let mirror_first = VertexId::new();
        let mirror_second = VertexId::new();
        let line_point = VertexId::new();
        let vertices = [
            (center, 0.0, 0.0),
            (x, 1.0, 0.0),
            (y, 0.0, 1.0),
            (diagonal, 1.0, 1.0),
            (negative_y, 0.0, -1.0),
            (mirror_first, 1.0, 1.0),
            (mirror_second, 1.0, -1.0),
            (line_point, 0.5, 0.5),
        ];
        let edge_x = EdgeId::new();
        let edge_y = EdgeId::new();
        let edge_diagonal = EdgeId::new();
        let edge_parallel = EdgeId::new();
        let pattern = CreasePattern {
            vertices: vertices
                .into_iter()
                .map(|(id, x, y)| Vertex {
                    id,
                    position: Point2::new(x, y),
                })
                .collect(),
            edges: vec![
                Edge {
                    id: edge_x,
                    start: center,
                    end: x,
                    kind: EdgeKind::Auxiliary,
                },
                Edge {
                    id: edge_y,
                    start: center,
                    end: y,
                    kind: EdgeKind::Auxiliary,
                },
                Edge {
                    id: edge_diagonal,
                    start: center,
                    end: diagonal,
                    kind: EdgeKind::Auxiliary,
                },
                Edge {
                    id: edge_parallel,
                    start: negative_y,
                    end: mirror_second,
                    kind: EdgeKind::Auxiliary,
                },
            ],
        };
        let fixtures = vec![
            (
                center,
                GeometricConstraintKindV1::FixedLength {
                    edge: edge_x,
                    length_mm: 1.0,
                },
            ),
            (
                center,
                GeometricConstraintKindV1::FixedAngle {
                    vertex: center,
                    first_edge: edge_x,
                    second_edge: edge_y,
                    angle_degrees: 90.0,
                },
            ),
            (
                center,
                GeometricConstraintKindV1::Horizontal { edge: edge_x },
            ),
            (center, GeometricConstraintKindV1::Vertical { edge: edge_y }),
            (
                center,
                GeometricConstraintKindV1::EqualLength {
                    first_edge: edge_x,
                    second_edge: edge_y,
                },
            ),
            (
                center,
                GeometricConstraintKindV1::Parallel {
                    first_edge: edge_x,
                    second_edge: edge_parallel,
                },
            ),
            (
                line_point,
                GeometricConstraintKindV1::PointOnLine {
                    vertex: line_point,
                    line_edge: edge_diagonal,
                },
            ),
            (
                center,
                GeometricConstraintKindV1::MirrorSymmetry {
                    first_vertex: mirror_first,
                    second_vertex: mirror_second,
                    axis_edge: edge_x,
                },
            ),
            (
                center,
                GeometricConstraintKindV1::RotationalSymmetry {
                    center_vertex: center,
                    source_vertex: x,
                    target_vertex: y,
                    angle_degrees: 90.0,
                },
            ),
            (
                center,
                GeometricConstraintKindV1::AngleBisector {
                    vertex: center,
                    first_edge: edge_x,
                    second_edge: edge_y,
                    bisector_edge: edge_diagonal,
                },
            ),
            (
                center,
                GeometricConstraintKindV1::LengthRatio {
                    numerator_edge: edge_x,
                    denominator_edge: edge_y,
                    ratio: 1.0,
                },
            ),
        ];
        for (fixture_index, (driving, constraint)) in fixtures.iter().cloned().enumerate() {
            let document = GeometricConstraintDocumentV1 {
                schema_version: GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
                constraints: vec![GeometricConstraintRecordV1 {
                    id: ConstraintId::new(),
                    constraint,
                }],
            };
            let position = pattern
                .vertices
                .iter()
                .find(|vertex| vertex.id == driving)
                .unwrap()
                .position;
            let preview = solve_geometric_constraints_v1(
                &pattern,
                &document,
                driving,
                position,
                ConstraintSolveLimitsV1::default(),
            )
            .unwrap_or_else(|error| panic!("fixture {fixture_index} must converge: {error:?}"));
            assert!(preview.maximum_residual <= 1e-7);
        }
        let mut combined = GeometricConstraintDocumentV1 {
            schema_version: GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
            constraints: fixtures
                .into_iter()
                .map(|(_, constraint)| GeometricConstraintRecordV1 {
                    id: ConstraintId::new(),
                    constraint,
                })
                .collect(),
        };
        let forward = solve_geometric_constraints_v1(
            &pattern,
            &combined,
            center,
            Point2::new(0.0, 0.0),
            ConstraintSolveLimitsV1::default(),
        )
        .expect("combined forward order");
        combined.constraints.reverse();
        let reverse = solve_geometric_constraints_v1(
            &pattern,
            &combined,
            center,
            Point2::new(0.0, 0.0),
            ConstraintSolveLimitsV1::default(),
        )
        .expect("combined reverse order");
        assert_eq!(forward.positions, reverse.positions);
        assert!(forward.maximum_residual <= 1e-7);
        assert!(reverse.maximum_residual <= 1e-7);
    }

    fn driving_placeholder() -> VertexId {
        VertexId::new()
    }
}
