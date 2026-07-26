use std::collections::{HashMap, HashSet};

use ori_domain::{
    CreasePattern, GeometricConstraintDocumentV1, GeometricConstraintKindV1, Point2, VertexId,
};
use thiserror::Error;

use crate::{
    ConstraintPreflightV1, GeometricConstraintLimitsV1,
    constraints::length_ratio_residual_binary64_v1, prepare_geometric_constraints_v1,
};

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
    /// Reserved for V1 API compatibility. The current validator rejects an
    /// unsupported/invalid kind as `InvalidConstraintDocumentOrGeometry`
    /// before the solver runs, so production code does not emit this variant.
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
    let prepared =
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
    if matches!(
        prepared.preflight(),
        ConstraintPreflightV1::DirectConflict { .. }
    ) {
        return Err(ConstraintSolveErrorV1::NonConvergent);
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
    let prepared =
        prepare_geometric_constraints_v1(pattern, document, GeometricConstraintLimitsV1::default())
            .map_err(|_| ConstraintSolveErrorV1::InvalidConstraintDocumentOrGeometry)?;
    if matches!(
        prepared.preflight(),
        ConstraintPreflightV1::DirectConflict { .. }
    ) {
        return Err(ConstraintSolveErrorV1::NonConvergent);
    }
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
                    GeometricConstraintKindV1::AngleBisector { .. } => 2,
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
    let unit_vector = |edge_id| {
        let (x, y) = vector(edge_id);
        let magnitude = x.hypot(y);
        if !magnitude.is_finite() || magnitude == 0.0 {
            return None;
        }
        let unit = (x / magnitude, y / magnitude);
        (unit.0.is_finite() && unit.1.is_finite()).then_some(unit)
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
                    let direction =
                        unit_vector(line_edge).ok_or(ConstraintSolveErrorV1::NonConvergent)?;
                    vec![(point.x - start.x) * direction.1 - (point.y - start.y) * direction.0]
                }
                GeometricConstraintKindV1::LengthRatio {
                    numerator_edge,
                    denominator_edge,
                    ratio,
                } => vec![length_ratio_residual_binary64_v1(
                    length(numerator_edge),
                    ratio,
                    length(denominator_edge),
                )],
                GeometricConstraintKindV1::FixedAngle {
                    vertex,
                    first_edge,
                    second_edge,
                    angle_degrees,
                } => {
                    let first = outward_vector(first_edge, vertex);
                    let second = outward_vector(second_edge, vertex);
                    let actual = (first.0 * second.1 - first.1 * second.0)
                        .abs()
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
                    let direction =
                        unit_vector(axis_edge).ok_or(ConstraintSolveErrorV1::NonConvergent)?;
                    let first = positions[&first_vertex];
                    let projection =
                        (first.x - origin.x) * direction.0 + (first.y - origin.y) * direction.1;
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
                    let sum_norm = sum_x.hypot(sum_y);
                    let bisector_norm = bisector.0.hypot(bisector.1);
                    let denominator = sum_norm * bisector_norm;
                    let direction_cosine = (sum_x * bisector.0 + sum_y * bisector.1) / denominator;
                    vec![
                        (sum_x * bisector.1 - sum_y * bisector.0) / denominator,
                        (-direction_cosine).max(0.0),
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

    #[derive(Debug, Clone, Copy)]
    enum QuarantinedDirectFamily {
        DifferentFixedAngles,
        DifferentLengthRatios,
        EqualLengthWithNonUnitRatioAndFixedLength,
        NonReciprocalLengthRatiosWithFixedLength,
        // This family has one sound rounded-residual subset. The fixture below
        // deliberately stays on its rounded-zero boundary and must remain
        // solver-required.
        LengthRatioWithIncompatibleFixedLengths,
        NonUnitLengthRatioCycleWithFixedLength,
        InconsistentLengthRatioGraphWithFixedLength,
        PerpendicularOrientationsInParallelComponent,
        NonParallelFixedAngleInParallelComponent,
        ParallelWithFixedNonParallelAngle,
        SameOrientationWithFixedNonParallelAngle,
        PerpendicularOrientationsWithFixedNonRightAngle,
        DifferentRotationalSymmetryAnglesWithFixedRadius,
        NonComplementaryInverseRotationalSymmetryAnglesWithFixedRadius,
        MirrorSymmetryWithPointOnAxisAndFixedSeparation,
        RotationalSymmetryWithCollinearRadius,
    }

    struct QuarantinedCounterexample {
        pattern: CreasePattern,
        document: GeometricConstraintDocumentV1,
        positions: HashMap<VertexId, Point2>,
    }

    #[derive(Default)]
    struct CounterexampleBuilder {
        vertices: Vec<Vertex>,
        edges: Vec<Edge>,
        positions: HashMap<VertexId, Point2>,
    }

    impl CounterexampleBuilder {
        fn vertex(&mut self, solution: Point2) -> VertexId {
            let id = VertexId::new();
            let index = self.vertices.len() as f64 + 1.0;
            self.vertices.push(Vertex {
                id,
                position: Point2::new(index * 7.0, index * index + index * 3.0),
            });
            self.positions.insert(id, solution);
            id
        }

        fn edge(&mut self, start: VertexId, end: VertexId) -> EdgeId {
            let id = EdgeId::new();
            self.edges.push(Edge {
                id,
                start,
                end,
                kind: EdgeKind::Auxiliary,
            });
            id
        }

        fn independent_edge(&mut self, vector: Point2) -> EdgeId {
            let start = self.vertex(Point2::new(0.0, 0.0));
            let end = self.vertex(vector);
            self.edge(start, end)
        }

        fn finish(
            self,
            constraints: impl IntoIterator<Item = GeometricConstraintKindV1>,
        ) -> QuarantinedCounterexample {
            QuarantinedCounterexample {
                pattern: CreasePattern {
                    vertices: self.vertices,
                    edges: self.edges,
                },
                document: GeometricConstraintDocumentV1 {
                    schema_version: GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
                    constraints: constraints
                        .into_iter()
                        .map(|constraint| GeometricConstraintRecordV1 {
                            id: ConstraintId::new(),
                            constraint,
                        })
                        .collect(),
                },
                positions: self.positions,
            }
        }
    }

    fn assert_quarantined_counterexample(
        family: QuarantinedDirectFamily,
        example: QuarantinedCounterexample,
    ) {
        let label = format!("{family:?}");
        let values = residuals(&example.pattern, &example.document, &example.positions)
            .unwrap_or_else(|error| panic!("{label}: counterexample residuals failed: {error:?}"));
        assert!(
            values.iter().all(|value| value.is_finite()),
            "{label}: every counterexample residual must be finite: {values:?}"
        );
        assert_eq!(
            maximum_absolute(&values),
            0.0,
            "{label}: the concrete assignment must satisfy every implemented residual"
        );

        let prepared = prepare_geometric_constraints_v1(
            &example.pattern,
            &example.document,
            GeometricConstraintLimitsV1::default(),
        )
        .unwrap_or_else(|error| {
            panic!("{label}: valid counterexample failed to prepare: {error:?}")
        });
        let preflight = prepared.preflight();
        assert!(
            matches!(
                preflight,
                crate::ConstraintPreflightV1::Unknown {
                    reason:
                        crate::GeometricConstraintUnknownReasonV1::SolverRequiredConstraintKinds,
                    ref unchecked_constraint_ids,
                } if !unchecked_constraint_ids.is_empty()
            ),
            "{label}: preflight must defer instead of certifying unsatisfiability: {preflight:?}"
        );
        assert!(
            matches!(
                crate::find_bounded_direct_mus_v1(&prepared),
                crate::BoundedDirectMusV1::Unknown { .. }
            ),
            "{label}: the bounded oracle must not manufacture a proven MUS"
        );

        let drivers = example
            .pattern
            .vertices
            .iter()
            .map(|vertex| (vertex.id, example.positions[&vertex.id]))
            .collect::<Vec<_>>();
        let preview = solve_geometric_constraints_with_drivers_v1(
            &example.pattern,
            &example.document,
            &drivers,
            ConstraintSolveLimitsV1::default(),
        )
        .unwrap_or_else(|error| {
            panic!("{label}: full-driver zero-residual solution was rejected: {error:?}")
        });
        assert_eq!(preview.maximum_residual, 0.0, "{label}");

        assert!(
            verify_geometric_constraint_solution_v1(&example.pattern, &example.document, f64::MAX,)
                .is_ok(),
            "{label}: verifier must reach its residual check instead of a false preflight rejection"
        );

        let mut reordered_pattern = example.pattern.clone();
        reordered_pattern.vertices.reverse();
        reordered_pattern.edges.reverse();
        let mut reordered_document = example.document.clone();
        reordered_document.constraints.reverse();
        let reordered = prepare_geometric_constraints_v1(
            &reordered_pattern,
            &reordered_document,
            GeometricConstraintLimitsV1::default(),
        )
        .unwrap_or_else(|error| {
            panic!("{label}: reordered counterexample failed to prepare: {error:?}")
        });
        assert_eq!(
            reordered.preflight(),
            preflight,
            "{label}: deferred output must remain canonical"
        );
    }

    fn quarantined_counterexample(family: QuarantinedDirectFamily) -> QuarantinedCounterexample {
        let minimum = f64::from_bits(1);
        let one_up = 1.0_f64.next_up();
        let one_down = 1.0_f64.next_down();
        let mut builder = CounterexampleBuilder::default();

        match family {
            QuarantinedDirectFamily::DifferentFixedAngles => {
                let center = builder.vertex(Point2::new(0.0, 0.0));
                let first_vertex = builder.vertex(Point2::new(1.0, 0.0));
                let second_vertex = builder.vertex(Point2::new(2.0, 0.0));
                let first_edge = builder.edge(center, first_vertex);
                let second_edge = builder.edge(center, second_vertex);
                builder.finish([
                    GeometricConstraintKindV1::FixedAngle {
                        vertex: center,
                        first_edge,
                        second_edge,
                        angle_degrees: f64::from_bits(1),
                    },
                    GeometricConstraintKindV1::FixedAngle {
                        vertex: center,
                        first_edge,
                        second_edge,
                        angle_degrees: f64::from_bits(2),
                    },
                ])
            }
            QuarantinedDirectFamily::DifferentLengthRatios => {
                let first_edge = builder.independent_edge(Point2::new(minimum, 0.0));
                let second_edge = builder.independent_edge(Point2::new(minimum, 0.0));
                builder.finish([
                    GeometricConstraintKindV1::LengthRatio {
                        numerator_edge: first_edge,
                        denominator_edge: second_edge,
                        ratio: one_up,
                    },
                    GeometricConstraintKindV1::LengthRatio {
                        numerator_edge: first_edge,
                        denominator_edge: second_edge,
                        ratio: one_down,
                    },
                ])
            }
            QuarantinedDirectFamily::EqualLengthWithNonUnitRatioAndFixedLength => {
                let first_edge = builder.independent_edge(Point2::new(minimum, 0.0));
                let second_edge = builder.independent_edge(Point2::new(minimum, 0.0));
                builder.finish([
                    GeometricConstraintKindV1::FixedLength {
                        edge: first_edge,
                        length_mm: minimum,
                    },
                    GeometricConstraintKindV1::EqualLength {
                        first_edge,
                        second_edge,
                    },
                    GeometricConstraintKindV1::LengthRatio {
                        numerator_edge: first_edge,
                        denominator_edge: second_edge,
                        ratio: one_up,
                    },
                ])
            }
            QuarantinedDirectFamily::NonReciprocalLengthRatiosWithFixedLength => {
                let first_edge = builder.independent_edge(Point2::new(minimum, 0.0));
                let second_edge = builder.independent_edge(Point2::new(minimum, 0.0));
                builder.finish([
                    GeometricConstraintKindV1::FixedLength {
                        edge: first_edge,
                        length_mm: minimum,
                    },
                    GeometricConstraintKindV1::LengthRatio {
                        numerator_edge: first_edge,
                        denominator_edge: second_edge,
                        ratio: one_up,
                    },
                    GeometricConstraintKindV1::LengthRatio {
                        numerator_edge: second_edge,
                        denominator_edge: first_edge,
                        ratio: one_down,
                    },
                ])
            }
            QuarantinedDirectFamily::LengthRatioWithIncompatibleFixedLengths => {
                let first_edge = builder.independent_edge(Point2::new(minimum, 0.0));
                let second_edge = builder.independent_edge(Point2::new(minimum, 0.0));
                builder.finish([
                    GeometricConstraintKindV1::FixedLength {
                        edge: first_edge,
                        length_mm: minimum,
                    },
                    GeometricConstraintKindV1::FixedLength {
                        edge: second_edge,
                        length_mm: minimum,
                    },
                    GeometricConstraintKindV1::LengthRatio {
                        numerator_edge: first_edge,
                        denominator_edge: second_edge,
                        ratio: one_up,
                    },
                ])
            }
            QuarantinedDirectFamily::NonUnitLengthRatioCycleWithFixedLength => {
                let first_edge = builder.independent_edge(Point2::new(minimum, 0.0));
                let second_edge = builder.independent_edge(Point2::new(minimum, 0.0));
                let third_edge = builder.independent_edge(Point2::new(minimum, 0.0));
                builder.finish([
                    GeometricConstraintKindV1::FixedLength {
                        edge: first_edge,
                        length_mm: minimum,
                    },
                    GeometricConstraintKindV1::LengthRatio {
                        numerator_edge: first_edge,
                        denominator_edge: second_edge,
                        ratio: one_up,
                    },
                    GeometricConstraintKindV1::LengthRatio {
                        numerator_edge: second_edge,
                        denominator_edge: third_edge,
                        ratio: one_up,
                    },
                    GeometricConstraintKindV1::LengthRatio {
                        numerator_edge: third_edge,
                        denominator_edge: first_edge,
                        ratio: one_up,
                    },
                ])
            }
            QuarantinedDirectFamily::InconsistentLengthRatioGraphWithFixedLength => {
                let first_edge = builder.independent_edge(Point2::new(minimum, 0.0));
                let second_edge = builder.independent_edge(Point2::new(minimum, 0.0));
                let third_edge = builder.independent_edge(Point2::new(minimum, 0.0));
                let fourth_edge = builder.independent_edge(Point2::new(minimum, 0.0));
                builder.finish([
                    GeometricConstraintKindV1::FixedLength {
                        edge: first_edge,
                        length_mm: minimum,
                    },
                    GeometricConstraintKindV1::LengthRatio {
                        numerator_edge: first_edge,
                        denominator_edge: second_edge,
                        ratio: one_up,
                    },
                    GeometricConstraintKindV1::LengthRatio {
                        numerator_edge: second_edge,
                        denominator_edge: third_edge,
                        ratio: one_up,
                    },
                    GeometricConstraintKindV1::LengthRatio {
                        numerator_edge: third_edge,
                        denominator_edge: fourth_edge,
                        ratio: one_up,
                    },
                    GeometricConstraintKindV1::LengthRatio {
                        numerator_edge: fourth_edge,
                        denominator_edge: first_edge,
                        ratio: one_up,
                    },
                ])
            }
            QuarantinedDirectFamily::PerpendicularOrientationsInParallelComponent => {
                let vectors = [
                    Point2::new(1.0, 0.0),
                    Point2::new(minimum, 0.0),
                    Point2::new(
                        f64::from_bits(0x3fec_411b_4f6d_2708),
                        f64::from_bits(0x3fde_0bd2_7424_5079),
                    ),
                    Point2::new(f64::from_bits(0xe), f64::from_bits(0x8)),
                    Point2::new(
                        f64::from_bits(0x3feb_6dea_1e76_eade),
                        f64::from_bits(0x3fe0_7b31_20fd_df13),
                    ),
                    Point2::new(minimum, minimum),
                    Point2::new(
                        f64::from_bits(0x3fe0_f519_3eac_dd2a),
                        f64::from_bits(0x3feb_2335_c2cd_a946),
                    ),
                    Point2::new(f64::from_bits(0x8), f64::from_bits(0xe)),
                    Point2::new(
                        f64::from_bits(0x3fdf_071e_edef_a0ee),
                        f64::from_bits(0x3feb_fce2_77d3_39c6),
                    ),
                    Point2::new(0.0, minimum),
                    Point2::new(0.0, 1.0),
                ];
                let edges = vectors
                    .into_iter()
                    .map(|vector| builder.independent_edge(vector))
                    .collect::<Vec<_>>();
                let mut constraints = vec![GeometricConstraintKindV1::Horizontal {
                    edge: edges[0],
                }];
                constraints.extend(edges.windows(2).map(|pair| {
                    GeometricConstraintKindV1::Parallel {
                        first_edge: pair[0],
                        second_edge: pair[1],
                    }
                }));
                constraints.push(GeometricConstraintKindV1::Vertical {
                    edge: *edges.last().expect("parallel path has a final edge"),
                });
                builder.finish(constraints)
            }
            QuarantinedDirectFamily::NonParallelFixedAngleInParallelComponent => {
                let center = builder.vertex(Point2::new(0.0, 0.0));
                let first_vertex = builder.vertex(Point2::new(1.0, 0.0));
                let second_vertex = builder.vertex(Point2::new(2.0, minimum));
                let first_edge = builder.edge(center, first_vertex);
                let second_edge = builder.edge(center, second_vertex);
                let middle_edge = builder.independent_edge(Point2::new(1.0, 0.0));
                builder.finish([
                    GeometricConstraintKindV1::Parallel {
                        first_edge,
                        second_edge: middle_edge,
                    },
                    GeometricConstraintKindV1::Parallel {
                        first_edge: middle_edge,
                        second_edge,
                    },
                    GeometricConstraintKindV1::FixedAngle {
                        vertex: center,
                        first_edge,
                        second_edge,
                        angle_degrees: f64::from_bits(0x39),
                    },
                ])
            }
            QuarantinedDirectFamily::ParallelWithFixedNonParallelAngle => {
                let center = builder.vertex(Point2::new(0.0, 0.0));
                let first_vertex = builder.vertex(Point2::new(1.0, 0.0));
                let second_vertex = builder.vertex(Point2::new(2.0, minimum));
                let first_edge = builder.edge(center, first_vertex);
                let second_edge = builder.edge(center, second_vertex);
                builder.finish([
                    GeometricConstraintKindV1::Parallel {
                        first_edge,
                        second_edge,
                    },
                    GeometricConstraintKindV1::FixedAngle {
                        vertex: center,
                        first_edge,
                        second_edge,
                        angle_degrees: f64::from_bits(0x39),
                    },
                ])
            }
            QuarantinedDirectFamily::SameOrientationWithFixedNonParallelAngle => {
                let center = builder.vertex(Point2::new(0.0, 0.0));
                let first_vertex = builder.vertex(Point2::new(1.0, 0.0));
                let second_vertex = builder.vertex(Point2::new(2.0, 0.0));
                let first_edge = builder.edge(center, first_vertex);
                let second_edge = builder.edge(center, second_vertex);
                builder.finish([
                    GeometricConstraintKindV1::Horizontal { edge: first_edge },
                    GeometricConstraintKindV1::Horizontal { edge: second_edge },
                    GeometricConstraintKindV1::FixedAngle {
                        vertex: center,
                        first_edge,
                        second_edge,
                        angle_degrees: f64::from_bits(1),
                    },
                ])
            }
            QuarantinedDirectFamily::PerpendicularOrientationsWithFixedNonRightAngle => {
                let center = builder.vertex(Point2::new(0.0, 0.0));
                let horizontal_vertex = builder.vertex(Point2::new(1.0, 0.0));
                let vertical_vertex = builder.vertex(Point2::new(0.0, 1.0));
                let horizontal_edge = builder.edge(center, horizontal_vertex);
                let vertical_edge = builder.edge(center, vertical_vertex);
                builder.finish([
                    GeometricConstraintKindV1::Horizontal {
                        edge: horizontal_edge,
                    },
                    GeometricConstraintKindV1::Vertical {
                        edge: vertical_edge,
                    },
                    GeometricConstraintKindV1::FixedAngle {
                        vertex: center,
                        first_edge: horizontal_edge,
                        second_edge: vertical_edge,
                        angle_degrees: 90.0_f64.next_down(),
                    },
                ])
            }
            QuarantinedDirectFamily::DifferentRotationalSymmetryAnglesWithFixedRadius => {
                let center = builder.vertex(Point2::new(0.0, 0.0));
                let source = builder.vertex(Point2::new(minimum, 0.0));
                let target = builder.vertex(Point2::new(minimum, 0.0));
                let radius = builder.edge(center, source);
                builder.finish([
                    GeometricConstraintKindV1::RotationalSymmetry {
                        center_vertex: center,
                        source_vertex: source,
                        target_vertex: target,
                        angle_degrees: f64::from_bits(1),
                    },
                    GeometricConstraintKindV1::RotationalSymmetry {
                        center_vertex: center,
                        source_vertex: source,
                        target_vertex: target,
                        angle_degrees: f64::from_bits(2),
                    },
                    GeometricConstraintKindV1::FixedLength {
                        edge: radius,
                        length_mm: minimum,
                    },
                ])
            }
            QuarantinedDirectFamily::
                NonComplementaryInverseRotationalSymmetryAnglesWithFixedRadius =>
            {
                let center = builder.vertex(Point2::new(0.0, 0.0));
                let source = builder.vertex(Point2::new(minimum, 0.0));
                let target = builder.vertex(Point2::new(minimum, 0.0));
                let radius = builder.edge(center, source);
                builder.finish([
                    GeometricConstraintKindV1::RotationalSymmetry {
                        center_vertex: center,
                        source_vertex: source,
                        target_vertex: target,
                        angle_degrees: f64::from_bits(1),
                    },
                    GeometricConstraintKindV1::RotationalSymmetry {
                        center_vertex: center,
                        source_vertex: target,
                        target_vertex: source,
                        angle_degrees: f64::from_bits(1),
                    },
                    GeometricConstraintKindV1::FixedLength {
                        edge: radius,
                        length_mm: minimum,
                    },
                ])
            }
            QuarantinedDirectFamily::MirrorSymmetryWithPointOnAxisAndFixedSeparation => {
                let axis_start = builder.vertex(Point2::new(0.0, 0.0));
                let axis_end = builder.vertex(Point2::new(1.0, 1.0));
                let first = builder.vertex(Point2::new(2.0, 2.0));
                let reflected = f64::from_bits(0x3fff_ffff_ffff_fffc);
                let second = builder.vertex(Point2::new(reflected, reflected));
                let axis_edge = builder.edge(axis_start, axis_end);
                let separation_edge = builder.edge(first, second);
                builder.finish([
                    GeometricConstraintKindV1::MirrorSymmetry {
                        first_vertex: first,
                        second_vertex: second,
                        axis_edge,
                    },
                    GeometricConstraintKindV1::PointOnLine {
                        vertex: first,
                        line_edge: axis_edge,
                    },
                    GeometricConstraintKindV1::FixedLength {
                        edge: separation_edge,
                        length_mm: f64::from_bits(0x3cd6_a09e_667f_3bcd),
                    },
                ])
            }
            QuarantinedDirectFamily::RotationalSymmetryWithCollinearRadius => {
                let center = builder.vertex(Point2::new(0.0, 0.0));
                let source = builder.vertex(Point2::new(minimum, 0.0));
                let target = builder.vertex(Point2::new(minimum, 0.0));
                let radius = builder.edge(center, source);
                builder.finish([
                    GeometricConstraintKindV1::RotationalSymmetry {
                        center_vertex: center,
                        source_vertex: source,
                        target_vertex: target,
                        angle_degrees: f64::from_bits(1),
                    },
                    GeometricConstraintKindV1::PointOnLine {
                        vertex: target,
                        line_edge: radius,
                    },
                ])
            }
        }
    }

    macro_rules! quarantined_family_regression {
        ($test:ident, $family:ident) => {
            #[test]
            fn $test() {
                let family = QuarantinedDirectFamily::$family;
                assert_quarantined_counterexample(family, quarantined_counterexample(family));
            }
        };
    }

    quarantined_family_regression!(
        different_fixed_angles_are_solver_required,
        DifferentFixedAngles
    );
    quarantined_family_regression!(
        different_length_ratios_are_solver_required,
        DifferentLengthRatios
    );
    quarantined_family_regression!(
        equal_length_nonunit_ratio_fixed_length_is_solver_required,
        EqualLengthWithNonUnitRatioAndFixedLength
    );
    quarantined_family_regression!(
        nonreciprocal_ratios_fixed_length_is_solver_required,
        NonReciprocalLengthRatiosWithFixedLength
    );
    quarantined_family_regression!(
        rounded_compatible_ratio_fixed_lengths_are_solver_required,
        LengthRatioWithIncompatibleFixedLengths
    );
    quarantined_family_regression!(
        nonunit_ratio_cycle_fixed_length_is_solver_required,
        NonUnitLengthRatioCycleWithFixedLength
    );
    quarantined_family_regression!(
        inconsistent_ratio_graph_fixed_length_is_solver_required,
        InconsistentLengthRatioGraphWithFixedLength
    );
    quarantined_family_regression!(
        perpendicular_parallel_component_is_solver_required,
        PerpendicularOrientationsInParallelComponent
    );
    quarantined_family_regression!(
        fixed_angle_parallel_component_is_solver_required,
        NonParallelFixedAngleInParallelComponent
    );
    quarantined_family_regression!(
        parallel_fixed_nonparallel_angle_is_solver_required,
        ParallelWithFixedNonParallelAngle
    );
    quarantined_family_regression!(
        same_orientation_fixed_angle_is_solver_required,
        SameOrientationWithFixedNonParallelAngle
    );
    quarantined_family_regression!(
        perpendicular_orientation_fixed_angle_is_solver_required,
        PerpendicularOrientationsWithFixedNonRightAngle
    );
    quarantined_family_regression!(
        different_rotation_angles_fixed_radius_are_solver_required,
        DifferentRotationalSymmetryAnglesWithFixedRadius
    );
    quarantined_family_regression!(
        inverse_rotation_angles_fixed_radius_are_solver_required,
        NonComplementaryInverseRotationalSymmetryAnglesWithFixedRadius
    );
    quarantined_family_regression!(
        mirror_axis_fixed_separation_is_solver_required,
        MirrorSymmetryWithPointOnAxisAndFixedSeparation
    );
    quarantined_family_regression!(
        collinear_rotation_radius_is_solver_required,
        RotationalSymmetryWithCollinearRadius
    );

    #[test]
    fn incompatible_fixed_lengths_and_ratio_are_rejected_before_numerical_tolerance() {
        let mut builder = CounterexampleBuilder::default();
        let numerator_edge = builder.independent_edge(Point2::new(0.3, 0.0));
        let denominator_edge = builder.independent_edge(Point2::new(0.1, 0.0));
        let example = builder.finish([
            GeometricConstraintKindV1::FixedLength {
                edge: numerator_edge,
                length_mm: 0.3,
            },
            GeometricConstraintKindV1::FixedLength {
                edge: denominator_edge,
                length_mm: 0.1,
            },
            GeometricConstraintKindV1::LengthRatio {
                numerator_edge,
                denominator_edge,
                ratio: 3.0,
            },
        ]);
        let values = residuals(&example.pattern, &example.document, &example.positions)
            .expect("the incompatible ratio residual remains finite");
        assert_eq!(values[..2], [0.0, 0.0]);
        assert_eq!(values[2], length_ratio_residual_binary64_v1(0.3, 3.0, 0.1));
        assert_ne!(values[2], 0.0);

        let prepared = prepare_geometric_constraints_v1(
            &example.pattern,
            &example.document,
            GeometricConstraintLimitsV1::default(),
        )
        .expect("the three individually valid records prepare");
        assert!(matches!(
            prepared.preflight(),
            ConstraintPreflightV1::DirectConflict {
                ref conflicts
            } if conflicts.len() == 1
                && matches!(
                    conflicts[0].conflict(),
                    crate::DirectConstraintConflictKindV1::
                        LengthRatioWithIncompatibleFixedLengths {
                            numerator_edge: actual_numerator,
                            denominator_edge: actual_denominator,
                        } if *actual_numerator == numerator_edge
                            && *actual_denominator == denominator_edge
                )
                && conflicts[0].constraint_ids().len() == 3
        ));

        let drivers = example
            .pattern
            .vertices
            .iter()
            .map(|vertex| (vertex.id, example.positions[&vertex.id]))
            .collect::<Vec<_>>();
        assert_eq!(
            solve_geometric_constraints_with_drivers_v1(
                &example.pattern,
                &example.document,
                &drivers,
                ConstraintSolveLimitsV1::default(),
            ),
            Err(ConstraintSolveErrorV1::NonConvergent),
            "direct preflight must reject before a fully driven near-zero residual is tolerated"
        );
        assert_eq!(
            verify_geometric_constraint_solution_v1(&example.pattern, &example.document, f64::MAX,),
            Err(ConstraintSolveErrorV1::NonConvergent),
            "direct preflight must run before even the largest finite verifier tolerance"
        );
    }

    #[test]
    fn angle_bisector_rejects_the_opposite_reflex_direction() {
        let center = VertexId::new();
        let first_vertex = VertexId::new();
        let second_vertex = VertexId::new();
        let reflex_vertex = VertexId::new();
        let first_edge = EdgeId::new();
        let second_edge = EdgeId::new();
        let bisector_edge = EdgeId::new();
        let pattern = CreasePattern {
            vertices: vec![
                Vertex {
                    id: center,
                    position: Point2::new(0.0, 0.0),
                },
                Vertex {
                    id: first_vertex,
                    position: Point2::new(1.0, 0.0),
                },
                Vertex {
                    id: second_vertex,
                    position: Point2::new(0.0, 1.0),
                },
                Vertex {
                    id: reflex_vertex,
                    position: Point2::new(-1.0, -1.0),
                },
            ],
            edges: vec![
                Edge {
                    id: first_edge,
                    start: center,
                    end: first_vertex,
                    kind: EdgeKind::Auxiliary,
                },
                Edge {
                    id: second_edge,
                    start: center,
                    end: second_vertex,
                    kind: EdgeKind::Auxiliary,
                },
                Edge {
                    id: bisector_edge,
                    start: center,
                    end: reflex_vertex,
                    kind: EdgeKind::Auxiliary,
                },
            ],
        };
        let document = GeometricConstraintDocumentV1 {
            schema_version: GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
            constraints: vec![GeometricConstraintRecordV1 {
                id: ConstraintId::new(),
                constraint: GeometricConstraintKindV1::AngleBisector {
                    vertex: center,
                    first_edge,
                    second_edge,
                    bisector_edge,
                },
            }],
        };
        let positions = pattern
            .vertices
            .iter()
            .map(|vertex| (vertex.id, vertex.position))
            .collect();

        let values = residuals(&pattern, &document, &positions).expect("finite residuals");
        assert!(values[0].abs() <= 1e-12, "opposite rays remain collinear");
        assert!(
            values[1] > 0.99,
            "the reflex direction must carry a hard residual"
        );
    }

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

    #[derive(Debug, Clone, Copy)]
    enum NormalizedEdgeProvider {
        PointOnLine,
        MirrorAxis,
        AngleFirst,
        AngleSecond,
        AngleBisector,
    }

    struct NormalizedEdgeWitnessFixture {
        pattern: CreasePattern,
        center: VertexId,
        target_endpoint: VertexId,
        other_endpoint: VertexId,
        third_endpoint: VertexId,
        mirror_second: VertexId,
        target_edge: EdgeId,
        other_edge: EdgeId,
        third_edge: EdgeId,
    }

    impl NormalizedEdgeWitnessFixture {
        fn new() -> Self {
            let center = VertexId::new();
            let target_endpoint = VertexId::new();
            let other_endpoint = VertexId::new();
            let third_endpoint = VertexId::new();
            let mirror_second = VertexId::new();
            let target_edge = EdgeId::new();
            let other_edge = EdgeId::new();
            let third_edge = EdgeId::new();
            let pattern = CreasePattern {
                vertices: vec![
                    Vertex {
                        id: center,
                        position: Point2::new(0.0, 0.0),
                    },
                    Vertex {
                        id: target_endpoint,
                        position: Point2::new(1.0, 0.0),
                    },
                    Vertex {
                        id: other_endpoint,
                        position: Point2::new(0.0, 1.0),
                    },
                    Vertex {
                        id: third_endpoint,
                        position: Point2::new(-1.0, 0.0),
                    },
                    Vertex {
                        id: mirror_second,
                        position: Point2::new(0.0, -1.0),
                    },
                ],
                edges: vec![
                    Edge {
                        id: target_edge,
                        start: center,
                        end: target_endpoint,
                        kind: EdgeKind::Auxiliary,
                    },
                    Edge {
                        id: other_edge,
                        start: center,
                        end: other_endpoint,
                        kind: EdgeKind::Auxiliary,
                    },
                    Edge {
                        id: third_edge,
                        start: center,
                        end: third_endpoint,
                        kind: EdgeKind::Auxiliary,
                    },
                ],
            };
            Self {
                pattern,
                center,
                target_endpoint,
                other_endpoint,
                third_endpoint,
                mirror_second,
                target_edge,
                other_edge,
                third_edge,
            }
        }

        fn provider(&self, provider: NormalizedEdgeProvider) -> GeometricConstraintKindV1 {
            match provider {
                NormalizedEdgeProvider::PointOnLine => GeometricConstraintKindV1::PointOnLine {
                    vertex: self.other_endpoint,
                    line_edge: self.target_edge,
                },
                NormalizedEdgeProvider::MirrorAxis => GeometricConstraintKindV1::MirrorSymmetry {
                    first_vertex: self.other_endpoint,
                    second_vertex: self.mirror_second,
                    axis_edge: self.target_edge,
                },
                NormalizedEdgeProvider::AngleFirst => GeometricConstraintKindV1::AngleBisector {
                    vertex: self.center,
                    first_edge: self.target_edge,
                    second_edge: self.other_edge,
                    bisector_edge: self.third_edge,
                },
                NormalizedEdgeProvider::AngleSecond => GeometricConstraintKindV1::AngleBisector {
                    vertex: self.center,
                    first_edge: self.other_edge,
                    second_edge: self.target_edge,
                    bisector_edge: self.third_edge,
                },
                NormalizedEdgeProvider::AngleBisector => GeometricConstraintKindV1::AngleBisector {
                    vertex: self.center,
                    first_edge: self.other_edge,
                    second_edge: self.third_edge,
                    bisector_edge: self.target_edge,
                },
            }
        }

        fn positions(&self) -> HashMap<VertexId, Point2> {
            self.pattern
                .vertices
                .iter()
                .map(|vertex| (vertex.id, vertex.position))
                .collect()
        }

        fn satisfying_positions(
            &self,
            provider: NormalizedEdgeProvider,
            horizontal: bool,
        ) -> HashMap<VertexId, Point2> {
            let mut positions = self.positions();
            let mut set = |vertex, x, y| {
                positions.insert(vertex, Point2::new(x, y));
            };
            match (provider, horizontal) {
                (NormalizedEdgeProvider::PointOnLine, true) => {
                    set(self.target_endpoint, 1.0, 0.0);
                    set(self.other_endpoint, 2.0, 0.0);
                }
                (NormalizedEdgeProvider::PointOnLine, false) => {
                    set(self.target_endpoint, 0.0, 1.0);
                    set(self.other_endpoint, 0.0, 2.0);
                }
                (NormalizedEdgeProvider::MirrorAxis, true) => {
                    set(self.target_endpoint, 1.0, 0.0);
                    set(self.other_endpoint, 0.0, 1.0);
                    set(self.mirror_second, 0.0, -1.0);
                }
                (NormalizedEdgeProvider::MirrorAxis, false) => {
                    set(self.target_endpoint, 0.0, 1.0);
                    set(self.other_endpoint, 1.0, 0.0);
                    set(self.mirror_second, -1.0, 0.0);
                }
                (
                    NormalizedEdgeProvider::AngleFirst | NormalizedEdgeProvider::AngleSecond,
                    true,
                ) => {
                    set(self.target_endpoint, 1.0, 0.0);
                    set(self.other_endpoint, 0.0, 1.0);
                    set(self.third_endpoint, 1.0, 1.0);
                }
                (
                    NormalizedEdgeProvider::AngleFirst | NormalizedEdgeProvider::AngleSecond,
                    false,
                ) => {
                    set(self.target_endpoint, 0.0, 1.0);
                    set(self.other_endpoint, -1.0, 0.0);
                    set(self.third_endpoint, -1.0, 1.0);
                }
                (NormalizedEdgeProvider::AngleBisector, true) => {
                    set(self.target_endpoint, 1.0, 0.0);
                    set(self.other_endpoint, 1.0, 1.0);
                    set(self.third_endpoint, 1.0, -1.0);
                }
                (NormalizedEdgeProvider::AngleBisector, false) => {
                    set(self.target_endpoint, 0.0, 1.0);
                    set(self.other_endpoint, 1.0, 1.0);
                    set(self.third_endpoint, -1.0, 1.0);
                }
            }
            positions
        }
    }

    fn solver_document(
        constraints: impl IntoIterator<Item = GeometricConstraintKindV1>,
    ) -> GeometricConstraintDocumentV1 {
        GeometricConstraintDocumentV1 {
            schema_version: GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
            constraints: constraints
                .into_iter()
                .map(|constraint| GeometricConstraintRecordV1 {
                    id: ConstraintId::new(),
                    constraint,
                })
                .collect(),
        }
    }

    #[test]
    fn horizontal_and_vertical_alone_have_a_zero_length_residual_escape() {
        let (pattern, document, _) =
            single_edge(Point2::new(0.0, 0.0), Point2::new(1.0, 0.0), |edge| {
                vec![
                    GeometricConstraintKindV1::Horizontal { edge },
                    GeometricConstraintKindV1::Vertical { edge },
                ]
            });
        let collapsed = pattern
            .vertices
            .iter()
            .map(|vertex| (vertex.id, Point2::new(0.0, 0.0)))
            .collect();
        assert_eq!(
            residuals(&pattern, &document, &collapsed).expect("finite zero-length residuals"),
            vec![0.0, 0.0]
        );
    }

    #[test]
    fn normalized_edge_provider_witnesses_are_semantically_deletion_minimal() {
        let fixture = NormalizedEdgeWitnessFixture::new();
        for provider in [
            NormalizedEdgeProvider::PointOnLine,
            NormalizedEdgeProvider::MirrorAxis,
            NormalizedEdgeProvider::AngleFirst,
            NormalizedEdgeProvider::AngleSecond,
            NormalizedEdgeProvider::AngleBisector,
        ] {
            let horizontal = GeometricConstraintKindV1::Horizontal {
                edge: fixture.target_edge,
            };
            let vertical = GeometricConstraintKindV1::Vertical {
                edge: fixture.target_edge,
            };
            let provider_kind = fixture.provider(provider);
            let full =
                solver_document([horizontal.clone(), vertical.clone(), provider_kind.clone()]);
            let prepared = prepare_geometric_constraints_v1(
                &fixture.pattern,
                &full,
                GeometricConstraintLimitsV1::default(),
            )
            .unwrap_or_else(|error| panic!("{provider:?} source fixture must prepare: {error:?}"));
            assert!(matches!(
                prepared.preflight(),
                ConstraintPreflightV1::DirectConflict { ref conflicts }
                    if conflicts.len() == 1
                        && matches!(
                            conflicts[0].conflict(),
                            crate::DirectConstraintConflictKindV1::HorizontalAndVertical {
                                edge
                            } if *edge == fixture.target_edge
                        )
            ));
            assert_eq!(
                verify_geometric_constraint_solution_v1(&fixture.pattern, &full, f64::MAX),
                Err(ConstraintSolveErrorV1::NonConvergent),
                "{provider:?}: direct preflight must run before numerical tolerance"
            );

            let mut collapsed = fixture.positions();
            collapsed.insert(fixture.target_endpoint, collapsed[&fixture.center]);
            assert!(
                matches!(
                    residuals(&fixture.pattern, &full, &collapsed),
                    Err(ConstraintSolveErrorV1::NonConvergent)
                ),
                "{provider:?}: its normalized edge role must reject collapse"
            );

            let orientations_only = solver_document([horizontal.clone(), vertical.clone()]);
            assert_eq!(
                residuals(&fixture.pattern, &orientations_only, &collapsed)
                    .expect("H/V alone admit their private collapsed residual witness"),
                vec![0.0, 0.0],
                "{provider:?}"
            );

            let horizontal_subset = solver_document([horizontal.clone(), provider_kind.clone()]);
            let horizontal_values = residuals(
                &fixture.pattern,
                &horizontal_subset,
                &fixture.satisfying_positions(provider, true),
            )
            .unwrap_or_else(|error| {
                panic!("{provider:?} horizontal subset must stay finite: {error:?}")
            });
            assert!(
                maximum_absolute(&horizontal_values) <= 1e-12,
                "{provider:?} horizontal subset residuals: {horizontal_values:?}"
            );

            let vertical_subset = solver_document([vertical.clone(), provider_kind]);
            let vertical_values = residuals(
                &fixture.pattern,
                &vertical_subset,
                &fixture.satisfying_positions(provider, false),
            )
            .unwrap_or_else(|error| {
                panic!("{provider:?} vertical subset must stay finite: {error:?}")
            });
            assert!(
                maximum_absolute(&vertical_values) <= 1e-12,
                "{provider:?} vertical subset residuals: {vertical_values:?}"
            );
        }
    }

    #[test]
    fn collinear_rotation_subnormal_zero_is_not_rejected_by_preflight() {
        let center = VertexId::new();
        let source = VertexId::new();
        let target = VertexId::new();
        let radius = EdgeId::new();
        let minimum = f64::from_bits(1);
        let pattern = CreasePattern {
            vertices: vec![
                Vertex {
                    id: center,
                    position: Point2::new(0.0, 0.0),
                },
                Vertex {
                    id: source,
                    position: Point2::new(minimum, 0.0),
                },
                Vertex {
                    id: target,
                    position: Point2::new(-minimum, 0.0),
                },
            ],
            edges: vec![Edge {
                id: radius,
                start: center,
                end: source,
                kind: EdgeKind::Auxiliary,
            }],
        };
        let positions = pattern
            .vertices
            .iter()
            .map(|vertex| (vertex.id, vertex.position))
            .collect::<HashMap<_, _>>();

        for angle_degrees in [180.0_f64.next_down(), 180.0_f64.next_up()] {
            let radians = angle_degrees.to_radians();
            let legacy_rotation = [
                -minimum - minimum * radians.cos(),
                0.0 - minimum * radians.sin(),
            ];
            let legacy_point_on_line = 0.0_f64;
            assert_eq!(
                [legacy_rotation[0], legacy_rotation[1], legacy_point_on_line,],
                [0.0; 3],
                "ordinary binary64 equations manufacture an exact zero"
            );

            let document = GeometricConstraintDocumentV1 {
                schema_version: GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
                constraints: vec![
                    GeometricConstraintRecordV1 {
                        id: ConstraintId::new(),
                        constraint: GeometricConstraintKindV1::RotationalSymmetry {
                            center_vertex: center,
                            source_vertex: source,
                            target_vertex: target,
                            angle_degrees,
                        },
                    },
                    GeometricConstraintRecordV1 {
                        id: ConstraintId::new(),
                        constraint: GeometricConstraintKindV1::PointOnLine {
                            vertex: target,
                            line_edge: radius,
                        },
                    },
                ],
            };
            assert_eq!(
                residuals(&pattern, &document, &positions)
                    .expect("the adversarial ordinary residual is finite"),
                vec![0.0; 3]
            );
            assert!(
                verify_geometric_constraint_solution_v1(&pattern, &document, 1e-7).is_ok(),
                "a real zero-residual counterexample must not be rejected by preflight"
            );
            assert!(
                solve_geometric_constraints_with_drivers_v1(
                    &pattern,
                    &document,
                    &[
                        (center, Point2::new(0.0, 0.0)),
                        (source, Point2::new(minimum, 0.0)),
                        (target, Point2::new(-minimum, 0.0)),
                    ],
                    ConstraintSolveLimitsV1::default(),
                )
                .is_ok(),
                "the fully driven zero-residual counterexample must converge"
            );
        }

        let point_only = GeometricConstraintDocumentV1 {
            schema_version: GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
            constraints: vec![GeometricConstraintRecordV1 {
                id: ConstraintId::new(),
                constraint: GeometricConstraintKindV1::PointOnLine {
                    vertex: target,
                    line_edge: radius,
                },
            }],
        };
        let mut collapsed_line = positions;
        collapsed_line.insert(source, collapsed_line[&center]);
        assert!(
            matches!(
                residuals(&pattern, &point_only, &collapsed_line),
                Err(ConstraintSolveErrorV1::NonConvergent)
            ),
            "a collapsed normalized line is not a satisfying escape"
        );
    }

    #[test]
    fn collinear_rotation_neighbors_defer_to_solver_tolerance() {
        let center = VertexId::new();
        let source = VertexId::new();
        let target = VertexId::new();
        let radius = EdgeId::new();
        let pattern = CreasePattern {
            vertices: vec![
                Vertex {
                    id: center,
                    position: Point2::new(0.0, 0.0),
                },
                Vertex {
                    id: source,
                    position: Point2::new(1.0, 0.0),
                },
                Vertex {
                    id: target,
                    position: Point2::new(-1.0, 0.0),
                },
            ],
            edges: vec![Edge {
                id: radius,
                start: center,
                end: source,
                kind: EdgeKind::Auxiliary,
            }],
        };
        let positions = pattern
            .vertices
            .iter()
            .map(|vertex| (vertex.id, vertex.position))
            .collect::<HashMap<_, _>>();
        for angle_degrees in [180.0_f64.next_down(), 180.0_f64.next_up()] {
            let document = GeometricConstraintDocumentV1 {
                schema_version: GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
                constraints: vec![
                    GeometricConstraintRecordV1 {
                        id: ConstraintId::new(),
                        constraint: GeometricConstraintKindV1::RotationalSymmetry {
                            center_vertex: center,
                            source_vertex: source,
                            target_vertex: target,
                            angle_degrees,
                        },
                    },
                    GeometricConstraintRecordV1 {
                        id: ConstraintId::new(),
                        constraint: GeometricConstraintKindV1::PointOnLine {
                            vertex: target,
                            line_edge: radius,
                        },
                    },
                ],
            };
            let ordinary_maximum = maximum_absolute(
                &residuals(&pattern, &document, &positions)
                    .expect("ordinary-radius residuals remain finite"),
            );
            assert!(
                ordinary_maximum > 0.0
                    && ordinary_maximum < ConstraintSolveLimitsV1::default().residual_tolerance,
                "the exact contradiction is smaller than the numerical acceptance tolerance"
            );
            assert!(
                verify_geometric_constraint_solution_v1(&pattern, &document, 1e-7).is_ok(),
                "preflight must not override the verifier tolerance"
            );
            assert!(
                solve_geometric_constraints_with_drivers_v1(
                    &pattern,
                    &document,
                    &[
                        (center, Point2::new(0.0, 0.0)),
                        (source, Point2::new(1.0, 0.0)),
                        (target, Point2::new(-1.0, 0.0)),
                    ],
                    ConstraintSolveLimitsV1::default(),
                )
                .is_ok(),
                "preflight must not reject a fully driven within-tolerance solution"
            );
            assert!(matches!(
                solve_geometric_constraints_with_drivers_v1(
                    &pattern,
                    &document,
                    &[(center, Point2::new(0.0, 0.0))],
                    ConstraintSolveLimitsV1 {
                        max_constraints: 1,
                        ..ConstraintSolveLimitsV1::default()
                    },
                ),
                Err(ConstraintSolveErrorV1::WorkLimitExceeded)
            ));
            assert!(matches!(
                solve_geometric_constraints_with_drivers_v1(
                    &pattern,
                    &document,
                    &[(VertexId::new(), Point2::new(0.0, 0.0))],
                    ConstraintSolveLimitsV1::default(),
                ),
                Err(ConstraintSolveErrorV1::DrivingVertexMissing)
            ));
            assert!(matches!(
                solve_geometric_constraints_with_drivers_v1(
                    &pattern,
                    &document,
                    &[
                        (center, Point2::new(0.0, 0.0)),
                        (center, Point2::new(0.0, 0.0)),
                    ],
                    ConstraintSolveLimitsV1::default(),
                ),
                Err(ConstraintSolveErrorV1::DrivingVertexMissing)
            ));

            let unrelated = VertexId::new();
            let mut pattern_with_unrelated_vertex = pattern.clone();
            pattern_with_unrelated_vertex.vertices.push(Vertex {
                id: unrelated,
                position: Point2::new(0.0, 2.0),
            });
            assert!(matches!(
                solve_geometric_constraints_with_drivers_v1(
                    &pattern_with_unrelated_vertex,
                    &document,
                    &[(unrelated, Point2::new(0.0, 3.0))],
                    ConstraintSolveLimitsV1::default(),
                ),
                Err(ConstraintSolveErrorV1::UnderConstrained)
            ));
        }

        let mut near_zero_pattern = pattern.clone();
        near_zero_pattern.vertices[2].position = Point2::new(1.0_f64.next_up(), 0.0);
        let near_zero_document = GeometricConstraintDocumentV1 {
            schema_version: GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
            constraints: vec![
                GeometricConstraintRecordV1 {
                    id: ConstraintId::new(),
                    constraint: GeometricConstraintKindV1::RotationalSymmetry {
                        center_vertex: center,
                        source_vertex: source,
                        target_vertex: target,
                        angle_degrees: f64::MIN_POSITIVE,
                    },
                },
                GeometricConstraintRecordV1 {
                    id: ConstraintId::new(),
                    constraint: GeometricConstraintKindV1::PointOnLine {
                        vertex: target,
                        line_edge: radius,
                    },
                },
            ],
        };
        let near_zero_positions = near_zero_pattern
            .vertices
            .iter()
            .map(|vertex| (vertex.id, vertex.position))
            .collect::<HashMap<_, _>>();
        let near_zero_maximum = maximum_absolute(
            &residuals(
                &near_zero_pattern,
                &near_zero_document,
                &near_zero_positions,
            )
            .expect("near-zero-angle residuals stay finite"),
        );
        assert!(
            near_zero_maximum <= ConstraintSolveLimitsV1::default().residual_tolerance,
            "the near-zero exact contradiction lies inside numerical tolerance"
        );
        assert!(
            verify_geometric_constraint_solution_v1(&near_zero_pattern, &near_zero_document, 1e-7,)
                .is_ok()
        );
        assert!(
            solve_geometric_constraints_with_drivers_v1(
                &near_zero_pattern,
                &near_zero_document,
                &[
                    (center, Point2::new(0.0, 0.0)),
                    (source, Point2::new(1.0, 0.0)),
                    (target, Point2::new(1.0_f64.next_up(), 0.0)),
                ],
                ConstraintSolveLimitsV1::default(),
            )
            .is_ok()
        );
    }

    #[test]
    fn collinear_rotation_two_record_witness_is_semantically_deletion_minimal() {
        let center = VertexId::new();
        let source = VertexId::new();
        let target = VertexId::new();
        let radius = EdgeId::new();
        let pattern = CreasePattern {
            vertices: vec![
                Vertex {
                    id: center,
                    position: Point2::new(0.0, 0.0),
                },
                Vertex {
                    id: source,
                    position: Point2::new(1.0, 0.0),
                },
                Vertex {
                    id: target,
                    position: Point2::new(0.0, 1.0),
                },
            ],
            edges: vec![Edge {
                id: radius,
                start: center,
                end: source,
                kind: EdgeKind::Auxiliary,
            }],
        };
        let rotation_only = GeometricConstraintDocumentV1 {
            schema_version: GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
            constraints: vec![GeometricConstraintRecordV1 {
                id: ConstraintId::new(),
                constraint: GeometricConstraintKindV1::RotationalSymmetry {
                    center_vertex: center,
                    source_vertex: source,
                    target_vertex: target,
                    angle_degrees: 90.0,
                },
            }],
        };
        let positions = pattern
            .vertices
            .iter()
            .map(|vertex| (vertex.id, vertex.position))
            .collect::<HashMap<_, _>>();
        assert!(
            maximum_absolute(&residuals(&pattern, &rotation_only, &positions).unwrap()) < 1e-12,
            "the rotation record is satisfiable after deleting PointOnLine"
        );

        let point_only = GeometricConstraintDocumentV1 {
            schema_version: GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
            constraints: vec![GeometricConstraintRecordV1 {
                id: ConstraintId::new(),
                constraint: GeometricConstraintKindV1::PointOnLine {
                    vertex: target,
                    line_edge: radius,
                },
            }],
        };
        let mut point_on_radius = positions;
        point_on_radius.insert(target, Point2::new(2.0, 0.0));
        assert_eq!(
            residuals(&pattern, &point_only, &point_on_radius)
                .expect("the point-only subset is satisfiable"),
            vec![0.0]
        );
    }

    #[test]
    fn mirror_point_on_axis_has_only_the_fixed_separation_blocking_its_collapse() {
        let axis_start = VertexId::new();
        let axis_end = VertexId::new();
        let first = VertexId::new();
        let second = VertexId::new();
        let axis_edge = EdgeId::new();
        let separation_edge = EdgeId::new();
        let pattern = CreasePattern {
            vertices: vec![
                Vertex {
                    id: axis_start,
                    position: Point2::new(-1.0, 0.0),
                },
                Vertex {
                    id: axis_end,
                    position: Point2::new(1.0, 0.0),
                },
                Vertex {
                    id: first,
                    position: Point2::new(0.0, 1.0),
                },
                Vertex {
                    id: second,
                    position: Point2::new(0.0, -1.0),
                },
            ],
            edges: vec![
                Edge {
                    id: axis_edge,
                    start: axis_start,
                    end: axis_end,
                    kind: EdgeKind::Auxiliary,
                },
                Edge {
                    id: separation_edge,
                    start: first,
                    end: second,
                    kind: EdgeKind::Auxiliary,
                },
            ],
        };
        let mirror = GeometricConstraintRecordV1 {
            id: ConstraintId::new(),
            constraint: GeometricConstraintKindV1::MirrorSymmetry {
                first_vertex: first,
                second_vertex: second,
                axis_edge,
            },
        };
        let point_on_axis = GeometricConstraintRecordV1 {
            id: ConstraintId::new(),
            constraint: GeometricConstraintKindV1::PointOnLine {
                vertex: first,
                line_edge: axis_edge,
            },
        };
        let mirror_and_axis = GeometricConstraintDocumentV1 {
            schema_version: GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
            constraints: vec![mirror.clone(), point_on_axis.clone()],
        };
        let mut collapsed = pattern
            .vertices
            .iter()
            .map(|vertex| (vertex.id, vertex.position))
            .collect::<HashMap<_, _>>();
        collapsed.insert(first, Point2::new(0.0, 0.0));
        collapsed.insert(second, Point2::new(0.0, 0.0));
        assert_eq!(
            residuals(&pattern, &mirror_and_axis, &collapsed)
                .expect("the non-degenerate axis keeps all residuals finite"),
            vec![0.0, 0.0, 0.0],
            "without positive separation the mirrored pair may collapse on-axis"
        );
        let mut collapsed_axis = collapsed.clone();
        collapsed_axis.insert(axis_end, collapsed_axis[&axis_start]);
        assert!(matches!(
            residuals(&pattern, &mirror_and_axis, &collapsed_axis),
            Err(ConstraintSolveErrorV1::NonConvergent)
        ));

        let with_fixed_separation = GeometricConstraintDocumentV1 {
            schema_version: GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
            constraints: vec![
                mirror,
                point_on_axis,
                GeometricConstraintRecordV1 {
                    id: ConstraintId::new(),
                    constraint: GeometricConstraintKindV1::FixedLength {
                        edge: separation_edge,
                        length_mm: 2.0,
                    },
                },
            ],
        };
        assert_eq!(
            residuals(&pattern, &with_fixed_separation, &collapsed)
                .expect("the fixed-length residual is finite"),
            vec![0.0, 0.0, 0.0, -2.0],
            "the direct theorem may rely only on this exact positive separation"
        );
    }

    #[test]
    fn mirror_and_point_on_line_share_an_overflow_safe_normalized_axis() {
        fn fixture(
            axis_end_position: Point2,
            first_position: Point2,
            second_position: Point2,
            length_mm: f64,
        ) -> (
            CreasePattern,
            GeometricConstraintDocumentV1,
            HashMap<VertexId, Point2>,
        ) {
            let axis_start = VertexId::new();
            let axis_end = VertexId::new();
            let first = VertexId::new();
            let second = VertexId::new();
            let axis_edge = EdgeId::new();
            let separation_edge = EdgeId::new();
            let pattern = CreasePattern {
                vertices: vec![
                    Vertex {
                        id: axis_start,
                        position: Point2::new(0.0, 0.0),
                    },
                    Vertex {
                        id: axis_end,
                        position: axis_end_position,
                    },
                    Vertex {
                        id: first,
                        position: first_position,
                    },
                    Vertex {
                        id: second,
                        position: second_position,
                    },
                ],
                edges: vec![
                    Edge {
                        id: axis_edge,
                        start: axis_start,
                        end: axis_end,
                        kind: EdgeKind::Auxiliary,
                    },
                    Edge {
                        id: separation_edge,
                        start: first,
                        end: second,
                        kind: EdgeKind::Auxiliary,
                    },
                ],
            };
            let document = GeometricConstraintDocumentV1 {
                schema_version: GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
                constraints: vec![
                    GeometricConstraintRecordV1 {
                        id: ConstraintId::new(),
                        constraint: GeometricConstraintKindV1::MirrorSymmetry {
                            first_vertex: first,
                            second_vertex: second,
                            axis_edge,
                        },
                    },
                    GeometricConstraintRecordV1 {
                        id: ConstraintId::new(),
                        constraint: GeometricConstraintKindV1::PointOnLine {
                            vertex: first,
                            line_edge: axis_edge,
                        },
                    },
                    GeometricConstraintRecordV1 {
                        id: ConstraintId::new(),
                        constraint: GeometricConstraintKindV1::FixedLength {
                            edge: separation_edge,
                            length_mm,
                        },
                    },
                ],
            };
            let positions = pattern
                .vertices
                .iter()
                .map(|vertex| (vertex.id, vertex.position))
                .collect();
            (pattern, document, positions)
        }

        let cases = [
            (
                "squared axis length would overflow",
                fixture(
                    Point2::new(1.0e308, 0.0),
                    Point2::new(1.0, 0.0),
                    Point2::new(-1.0, 0.0),
                    2.0,
                ),
            ),
            (
                "scale-first point-on-line cross product would underflow",
                fixture(
                    Point2::new(f64::from_bits(1), 0.0),
                    Point2::new(1.0, 0.25),
                    Point2::new(1.0, -0.25),
                    0.5,
                ),
            ),
        ];
        for (description, (pattern, document, positions)) in cases {
            let values =
                residuals(&pattern, &document, &positions).expect("extreme axes stay finite");
            assert!(
                values.iter().any(|value| *value != 0.0),
                "{description} must not manufacture a zero-residual counterexample"
            );
            assert!(matches!(
                prepare_geometric_constraints_v1(
                    &pattern,
                    &document,
                    GeometricConstraintLimitsV1::default(),
                )
                .expect("extreme finite fixture prepares")
                .preflight(),
                crate::ConstraintPreflightV1::Unknown {
                    reason:
                        crate::GeometricConstraintUnknownReasonV1::SolverRequiredConstraintKinds,
                    ..
                }
            ));
        }
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

        let mut reversed_document = document.clone();
        reversed_document.constraints[0].constraint = GeometricConstraintKindV1::FixedAngle {
            vertex: center,
            first_edge: forward_y,
            second_edge: reversed_x,
            angle_degrees: 90.0,
        };
        assert!(
            maximum_absolute(&residuals(&pattern, &reversed_document, &positions).unwrap()) < 1e-12
        );

        for (point, angle) in [
            (Point2::new(0.0, -1.0), 90.0),
            (Point2::new(-1.0, 0.0), 180.0),
            (Point2::new(2.0, 0.0), 0.0),
        ] {
            let mut moved = positions.clone();
            moved.insert(y, point);
            let mut angled = document.clone();
            if let GeometricConstraintKindV1::FixedAngle { angle_degrees, .. } =
                &mut angled.constraints[0].constraint
            {
                *angle_degrees = angle;
            }
            assert!(maximum_absolute(&residuals(&pattern, &angled, &moved).unwrap()) < 1e-12);
        }
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
    fn invalid_constraint_document_and_tiny_work_budget_fail_closed() {
        let (pattern, invalid, driving) = single_edge(
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
                &invalid,
                driving,
                Point2 { x: 1.0, y: 1.0 },
                ConstraintSolveLimitsV1::default()
            ),
            Err(ConstraintSolveErrorV1::InvalidConstraintDocumentOrGeometry)
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
