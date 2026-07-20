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
                positions: driving_positions.to_vec(),
                iterations: 0,
                maximum_residual,
                rank: 0,
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
                positions: positions
                    .into_iter()
                    .filter(|(vertex, point)| original.get(vertex).is_none_or(|old| old != point))
                    .collect(),
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
        if maximum_step <= limits.step_tolerance {
            return Err(ConstraintSolveErrorV1::NonConvergent);
        }
    }
    Err(ConstraintSolveErrorV1::NonConvergent)
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
        for row in (rank + 1)..matrix.len() {
            let factor = matrix[row][column] / matrix[rank][column];
            for index in column..columns {
                matrix[row][index] -= factor * matrix[rank][index];
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
            | GeometricConstraintKindV1::Horizontal { .. }
            | GeometricConstraintKindV1::Vertical { .. }
            | GeometricConstraintKindV1::EqualLength { .. }
            | GeometricConstraintKindV1::Parallel { .. }
            | GeometricConstraintKindV1::PointOnLine { .. }
            | GeometricConstraintKindV1::LengthRatio { .. } => {}
            _ => return Err(ConstraintSolveErrorV1::UnsupportedConstraintKind),
        }
    }
    Ok(document.constraints.len())
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
            _ => return Err(ConstraintSolveErrorV1::UnsupportedConstraintKind),
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
    let length = |edge_id| {
        let (x, y) = vector(edge_id);
        x.hypot(y)
    };
    document
        .constraints
        .iter()
        .map(|record| {
            let value = match record.constraint {
                GeometricConstraintKindV1::FixedLength { edge, length_mm } => {
                    length(edge) - length_mm
                }
                GeometricConstraintKindV1::Horizontal { edge } => vector(edge).1,
                GeometricConstraintKindV1::Vertical { edge } => vector(edge).0,
                GeometricConstraintKindV1::EqualLength {
                    first_edge,
                    second_edge,
                } => length(first_edge) - length(second_edge),
                GeometricConstraintKindV1::Parallel {
                    first_edge,
                    second_edge,
                } => {
                    let first = vector(first_edge);
                    let second = vector(second_edge);
                    (first.0 * second.1 - first.1 * second.0)
                        / (first.0.hypot(first.1) * second.0.hypot(second.1))
                }
                GeometricConstraintKindV1::PointOnLine { vertex, line_edge } => {
                    let edge = edges[&line_edge];
                    let start = positions[&edge.start];
                    let point = positions[&vertex];
                    let direction = vector(line_edge);
                    ((point.x - start.x) * direction.1 - (point.y - start.y) * direction.0)
                        / direction.0.hypot(direction.1)
                }
                GeometricConstraintKindV1::LengthRatio {
                    numerator_edge,
                    denominator_edge,
                    ratio,
                } => length(numerator_edge) - ratio * length(denominator_edge),
                _ => return Err(ConstraintSolveErrorV1::UnsupportedConstraintKind),
            };
            if value.is_finite() {
                Ok(value)
            } else {
                Err(ConstraintSolveErrorV1::NonConvergent)
            }
        })
        .collect()
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
        for row in 0..dimension {
            if row == column {
                continue;
            }
            let factor = matrix[row][column];
            for index in column..dimension {
                matrix[row][index] -= factor * matrix[column][index];
            }
            rhs[row] -= factor * rhs[column];
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

    fn driving_placeholder() -> VertexId {
        VertexId::new()
    }
}
