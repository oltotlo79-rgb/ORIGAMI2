//! Exact local necessary-condition checks for flat origami.
//!
//! This module evaluates the Kawasaki-Justin and Maekawa-Justin conditions at
//! every source vertex. Passing both conditions is deliberately reported as
//! "necessary conditions satisfied", never as proof that the whole crease
//! pattern is flat-foldable.

use std::collections::{HashMap, HashSet};

use num_bigint::{BigInt, Sign};
use ori_domain::{CreasePattern, EdgeKind, Paper, Point2, VertexId};
use serde::Serialize;

use crate::{
    admission::build_admitted_embedding,
    dcel::{DcelEmbedding, HalfEdgeIndex, VertexRotation},
};

/// Maximum incident mountain/valley degree evaluated by the exact Kawasaki
/// product. Maekawa counting remains available above this bound.
pub const MAX_EXACT_FOLD_DEGREE: usize = 256;

/// Mathematical model covered by this first local validator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalFlatFoldabilityModel {
    InteriorSingleVertexZeroThicknessV1,
}

/// Aggregate result across every source vertex.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalFlatFoldabilityReportStatus {
    Blocked,
    NotApplicable,
    NecessaryConditionsSatisfied,
    Violated,
    Indeterminate,
}

/// Per-vertex aggregate result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalVertexFoldabilityVerdict {
    NotApplicable,
    Satisfied,
    Violated,
    Indeterminate,
}

/// Stable reason for an unavailable per-vertex theorem result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalFoldabilityReason {
    PaperBoundary,
    CutIncident,
    NoIncidentFoldEdges,
    FoldDegreeLimit,
}

/// Result of one named necessary condition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalFoldabilityConditionStatus {
    Satisfied,
    Violated,
    NotApplicable,
    Indeterminate,
}

/// Exact local result for one stable source vertex.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LocalVertexFoldability {
    pub vertex: VertexId,
    pub fold_degree: usize,
    pub mountain_count: usize,
    pub valley_count: usize,
    pub verdict: LocalVertexFoldabilityVerdict,
    pub reason: Option<LocalFoldabilityReason>,
    pub kawasaki: LocalFoldabilityConditionStatus,
    pub maekawa: LocalFoldabilityConditionStatus,
}

/// Canonically ordered local necessary-condition report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LocalFlatFoldabilityReport {
    pub model: LocalFlatFoldabilityModel,
    pub max_exact_fold_degree: usize,
    pub status: LocalFlatFoldabilityReportStatus,
    pub total_vertices: usize,
    pub applicable_vertices: usize,
    pub satisfied_vertices: usize,
    pub violated_vertices: usize,
    pub not_applicable_vertices: usize,
    pub indeterminate_vertices: usize,
    pub vertices: Vec<LocalVertexFoldability>,
}

impl LocalFlatFoldabilityReport {
    fn blocked() -> Self {
        Self {
            model: LocalFlatFoldabilityModel::InteriorSingleVertexZeroThicknessV1,
            max_exact_fold_degree: MAX_EXACT_FOLD_DEGREE,
            status: LocalFlatFoldabilityReportStatus::Blocked,
            total_vertices: 0,
            applicable_vertices: 0,
            satisfied_vertices: 0,
            violated_vertices: 0,
            not_applicable_vertices: 0,
            indeterminate_vertices: 0,
            vertices: Vec::new(),
        }
    }

    fn analyzed(vertices: Vec<LocalVertexFoldability>) -> Self {
        let satisfied_vertices = vertices
            .iter()
            .filter(|vertex| vertex.verdict == LocalVertexFoldabilityVerdict::Satisfied)
            .count();
        let violated_vertices = vertices
            .iter()
            .filter(|vertex| vertex.verdict == LocalVertexFoldabilityVerdict::Violated)
            .count();
        let not_applicable_vertices = vertices
            .iter()
            .filter(|vertex| vertex.verdict == LocalVertexFoldabilityVerdict::NotApplicable)
            .count();
        let indeterminate_vertices = vertices
            .iter()
            .filter(|vertex| vertex.verdict == LocalVertexFoldabilityVerdict::Indeterminate)
            .count();
        let applicable_vertices = satisfied_vertices + violated_vertices + indeterminate_vertices;
        let status = if violated_vertices != 0 {
            LocalFlatFoldabilityReportStatus::Violated
        } else if indeterminate_vertices != 0 {
            LocalFlatFoldabilityReportStatus::Indeterminate
        } else if satisfied_vertices != 0 {
            LocalFlatFoldabilityReportStatus::NecessaryConditionsSatisfied
        } else {
            LocalFlatFoldabilityReportStatus::NotApplicable
        };

        Self {
            model: LocalFlatFoldabilityModel::InteriorSingleVertexZeroThicknessV1,
            max_exact_fold_degree: MAX_EXACT_FOLD_DEGREE,
            status,
            total_vertices: vertices.len(),
            applicable_vertices,
            satisfied_vertices,
            violated_vertices,
            not_applicable_vertices,
            indeterminate_vertices,
            vertices,
        }
    }
}

/// Evaluates local flat-foldability necessary conditions without mutating the
/// source document.
///
/// Invalid identity, paper, participating geometry, cut policy, or containment
/// blocks the whole report. Auxiliary-only draft geometry is ignored by
/// admission and is still returned as a not-applicable source vertex.
#[must_use]
pub fn analyze_local_flat_foldability(
    paper: &Paper,
    pattern: &CreasePattern,
) -> LocalFlatFoldabilityReport {
    let Ok(embedding) = build_admitted_embedding(paper, pattern) else {
        return LocalFlatFoldabilityReport::blocked();
    };
    let positions = pattern
        .vertices
        .iter()
        .map(|vertex| (vertex.id, vertex.position))
        .collect::<HashMap<_, _>>();
    let rotations = embedding
        .rotations
        .iter()
        .map(|rotation| (rotation.vertex, rotation))
        .collect::<HashMap<_, _>>();
    let boundary_vertices = paper
        .boundary_vertices
        .iter()
        .copied()
        .collect::<HashSet<_>>();
    let mut vertices = pattern
        .vertices
        .iter()
        .map(|vertex| vertex.id)
        .collect::<Vec<_>>();
    vertices.sort_by_key(VertexId::canonical_bytes);

    let vertices = vertices
        .into_iter()
        .map(|vertex| {
            analyze_vertex(
                vertex,
                rotations.get(&vertex).copied(),
                &embedding,
                &positions,
                &boundary_vertices,
            )
        })
        .collect::<Option<Vec<_>>>();
    vertices.map_or_else(
        LocalFlatFoldabilityReport::blocked,
        LocalFlatFoldabilityReport::analyzed,
    )
}

fn analyze_vertex(
    vertex: VertexId,
    rotation: Option<&VertexRotation>,
    embedding: &DcelEmbedding,
    positions: &HashMap<VertexId, Point2>,
    boundary_vertices: &HashSet<VertexId>,
) -> Option<LocalVertexFoldability> {
    let outgoing = rotation.map_or(&[][..], |rotation| rotation.outgoing.as_slice());
    let incident = outgoing
        .iter()
        .map(|half_edge| embedding.half_edges.get(half_edge.0))
        .collect::<Option<Vec<_>>>()?;
    let fold_half_edges = outgoing
        .iter()
        .copied()
        .filter(|half_edge| {
            embedding
                .half_edges
                .get(half_edge.0)
                .is_some_and(|edge| matches!(edge.kind, EdgeKind::Mountain | EdgeKind::Valley))
        })
        .collect::<Vec<_>>();
    let mountain_count = incident
        .iter()
        .filter(|edge| edge.kind == EdgeKind::Mountain)
        .count();
    let valley_count = incident
        .iter()
        .filter(|edge| edge.kind == EdgeKind::Valley)
        .count();
    let fold_degree = mountain_count + valley_count;

    let not_applicable_reason = if boundary_vertices.contains(&vertex)
        || incident.iter().any(|edge| edge.kind == EdgeKind::Boundary)
    {
        Some(LocalFoldabilityReason::PaperBoundary)
    } else if incident.iter().any(|edge| edge.kind == EdgeKind::Cut) {
        Some(LocalFoldabilityReason::CutIncident)
    } else if fold_degree == 0 {
        Some(LocalFoldabilityReason::NoIncidentFoldEdges)
    } else {
        None
    };
    if let Some(reason) = not_applicable_reason {
        return Some(LocalVertexFoldability {
            vertex,
            fold_degree,
            mountain_count,
            valley_count,
            verdict: LocalVertexFoldabilityVerdict::NotApplicable,
            reason: Some(reason),
            kawasaki: LocalFoldabilityConditionStatus::NotApplicable,
            maekawa: LocalFoldabilityConditionStatus::NotApplicable,
        });
    }

    let maekawa = if mountain_count.abs_diff(valley_count) == 2 {
        LocalFoldabilityConditionStatus::Satisfied
    } else {
        LocalFoldabilityConditionStatus::Violated
    };
    let kawasaki = if !fold_degree.is_multiple_of(2) {
        LocalFoldabilityConditionStatus::Violated
    } else if fold_degree > MAX_EXACT_FOLD_DEGREE {
        LocalFoldabilityConditionStatus::Indeterminate
    } else if exact_kawasaki_condition(vertex, &fold_half_edges, embedding, positions)? {
        LocalFoldabilityConditionStatus::Satisfied
    } else {
        LocalFoldabilityConditionStatus::Violated
    };
    let (verdict, reason) = if matches!(
        (kawasaki, maekawa),
        (LocalFoldabilityConditionStatus::Violated, _)
            | (_, LocalFoldabilityConditionStatus::Violated)
    ) {
        (LocalVertexFoldabilityVerdict::Violated, None)
    } else if kawasaki == LocalFoldabilityConditionStatus::Indeterminate {
        (
            LocalVertexFoldabilityVerdict::Indeterminate,
            Some(LocalFoldabilityReason::FoldDegreeLimit),
        )
    } else {
        (LocalVertexFoldabilityVerdict::Satisfied, None)
    };

    Some(LocalVertexFoldability {
        vertex,
        fold_degree,
        mountain_count,
        valley_count,
        verdict,
        reason,
        kawasaki,
        maekawa,
    })
}

fn exact_kawasaki_condition(
    vertex: VertexId,
    ordered_fold_half_edges: &[HalfEdgeIndex],
    embedding: &DcelEmbedding,
    positions: &HashMap<VertexId, Point2>,
) -> Option<bool> {
    let origin = positions.get(&vertex).copied()?;
    let destinations = ordered_fold_half_edges
        .iter()
        .map(|half_edge| {
            let destination = embedding.half_edges.get(half_edge.0)?.destination;
            positions.get(&destination).copied()
        })
        .collect::<Option<Vec<_>>>()?;
    exact_ordered_rays_kawasaki_condition(origin, &destinations)
}

fn exact_ordered_rays_kawasaki_condition(
    origin: Point2,
    ordered_destinations: &[Point2],
) -> Option<bool> {
    let rays = ordered_destinations
        .iter()
        .map(|destination| ExactComplex::from_ray(origin, *destination))
        .collect::<Option<Vec<_>>>()?;
    let even = balanced_product(&rays.iter().step_by(2).cloned().collect::<Vec<_>>());
    let odd = balanced_product(&rays.iter().skip(1).step_by(2).cloned().collect::<Vec<_>>());
    let cross = &even.real * &odd.imag - &even.imag * &odd.real;
    let dot = &even.real * &odd.real + &even.imag * &odd.imag;
    Some(cross.sign() == Sign::NoSign && dot.sign() == Sign::Minus)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExactComplex {
    real: BigInt,
    imag: BigInt,
}

impl ExactComplex {
    fn one() -> Self {
        Self {
            real: BigInt::from(1_u8),
            imag: BigInt::from(0_u8),
        }
    }

    fn from_ray(origin: Point2, destination: Point2) -> Option<Self> {
        if ![origin.x, origin.y, destination.x, destination.y]
            .into_iter()
            .all(f64::is_finite)
        {
            return None;
        }
        let ray = Self {
            real: exact_coordinate_units(destination.x) - exact_coordinate_units(origin.x),
            imag: exact_coordinate_units(destination.y) - exact_coordinate_units(origin.y),
        }
        .power_of_two_primitive();
        (ray.real.sign() != Sign::NoSign || ray.imag.sign() != Sign::NoSign).then_some(ray)
    }

    fn multiply(self, other: Self) -> Self {
        let real = &self.real * &other.real - &self.imag * &other.imag;
        let imag = self.real * other.imag + self.imag * other.real;
        Self { real, imag }.power_of_two_primitive()
    }

    fn power_of_two_primitive(mut self) -> Self {
        let common_shift = match (self.real.trailing_zeros(), self.imag.trailing_zeros()) {
            (Some(real), Some(imag)) => real.min(imag),
            (Some(real), None) => real,
            (None, Some(imag)) => imag,
            (None, None) => 0,
        };
        if common_shift != 0 {
            let shift = usize::try_from(common_shift)
                .expect("a binary64-derived trailing-zero count fits usize");
            self.real >>= shift;
            self.imag >>= shift;
        }
        self
    }
}

fn balanced_product(values: &[ExactComplex]) -> ExactComplex {
    match values {
        [] => ExactComplex::one(),
        [value] => value.clone(),
        _ => {
            let middle = values.len() / 2;
            balanced_product(&values[..middle]).multiply(balanced_product(&values[middle..]))
        }
    }
}

/// Returns a finite binary64 coordinate as an integer multiple of 2^-1074.
fn exact_coordinate_units(value: f64) -> BigInt {
    debug_assert!(value.is_finite());
    let bits = value.to_bits();
    let exponent_bits = ((bits >> 52) & 0x7ff) as i32;
    let fraction = bits & ((1_u64 << 52) - 1);
    let (significand, exponent) = if exponent_bits == 0 {
        (fraction, -1074)
    } else {
        ((1_u64 << 52) | fraction, exponent_bits - 1075)
    };
    let significand = if bits >> 63 == 0 {
        BigInt::from(significand)
    } else {
        -BigInt::from(significand)
    };
    let shift = usize::try_from(exponent + 1074)
        .expect("a finite binary64 exponent is at least the subnormal exponent");
    significand << shift
}

#[cfg(test)]
mod tests {
    use ori_domain::{Edge, Vertex};
    use serde::de::DeserializeOwned;

    use super::*;

    fn fixed_id<T: DeserializeOwned>(suffix: u64) -> T {
        serde_json::from_str(&format!("\"00000000-0000-0000-0000-{suffix:012x}\""))
            .expect("fixed UUID")
    }

    fn vertex(suffix: u64, x: f64, y: f64) -> Vertex {
        Vertex {
            id: fixed_id(suffix),
            position: Point2::new(x, y),
        }
    }

    fn edge(suffix: u64, start: &Vertex, end: &Vertex, kind: EdgeKind) -> Edge {
        Edge {
            id: fixed_id(suffix),
            start: start.id,
            end: end.id,
            kind,
        }
    }

    struct SheetFixture {
        paper: Paper,
        pattern: CreasePattern,
        center: VertexId,
    }

    fn octagonal_sheet(fold_endpoint_indices: &[usize], assignments: &[EdgeKind]) -> SheetFixture {
        assert_eq!(fold_endpoint_indices.len(), assignments.len());
        let boundary_positions = [
            Point2::new(0.0, 0.0),
            Point2::new(10.0, 0.0),
            Point2::new(20.0, 0.0),
            Point2::new(20.0, 10.0),
            Point2::new(20.0, 20.0),
            Point2::new(10.0, 20.0),
            Point2::new(0.0, 20.0),
            Point2::new(0.0, 10.0),
        ];
        let mut vertices = boundary_positions
            .into_iter()
            .enumerate()
            .map(|(index, position)| vertex(0x100 + index as u64, position.x, position.y))
            .collect::<Vec<_>>();
        let center = vertex(0x200, 10.0, 10.0);
        let center_id = center.id;
        vertices.push(center.clone());
        let mut edges = (0..boundary_positions.len())
            .map(|index| {
                edge(
                    0x300 + index as u64,
                    &vertices[index],
                    &vertices[(index + 1) % boundary_positions.len()],
                    EdgeKind::Boundary,
                )
            })
            .collect::<Vec<_>>();
        edges.extend(
            fold_endpoint_indices
                .iter()
                .copied()
                .zip(assignments.iter().copied())
                .enumerate()
                .map(|(index, (endpoint, kind))| {
                    edge(0x400 + index as u64, &center, &vertices[endpoint], kind)
                }),
        );
        let paper = Paper {
            boundary_vertices: vertices[..boundary_positions.len()]
                .iter()
                .map(|vertex| vertex.id)
                .collect(),
            ..Paper::default()
        };
        SheetFixture {
            paper,
            pattern: CreasePattern { vertices, edges },
            center: center_id,
        }
    }

    fn report_for_center(
        fixture: &SheetFixture,
    ) -> (LocalFlatFoldabilityReport, LocalVertexFoldability) {
        let report = analyze_local_flat_foldability(&fixture.paper, &fixture.pattern);
        let center = report
            .vertices
            .iter()
            .find(|vertex| vertex.vertex == fixture.center)
            .expect("center report")
            .clone();
        (report, center)
    }

    #[test]
    fn cardinal_degree_four_satisfies_both_necessary_conditions() {
        let fixture = octagonal_sheet(
            &[3, 5, 7, 1],
            &[
                EdgeKind::Mountain,
                EdgeKind::Mountain,
                EdgeKind::Mountain,
                EdgeKind::Valley,
            ],
        );

        let (report, center) = report_for_center(&fixture);

        assert_eq!(
            report.status,
            LocalFlatFoldabilityReportStatus::NecessaryConditionsSatisfied
        );
        assert_eq!(report.total_vertices, 9);
        assert_eq!(report.applicable_vertices, 1);
        assert_eq!(report.satisfied_vertices, 1);
        assert_eq!(report.not_applicable_vertices, 8);
        assert_eq!(center.fold_degree, 4);
        assert_eq!(center.mountain_count, 3);
        assert_eq!(center.valley_count, 1);
        assert_eq!(center.verdict, LocalVertexFoldabilityVerdict::Satisfied);
        assert_eq!(center.kawasaki, LocalFoldabilityConditionStatus::Satisfied);
        assert_eq!(center.maekawa, LocalFoldabilityConditionStatus::Satisfied);
        assert_eq!(center.reason, None);
    }

    #[test]
    fn kawasaki_and_maekawa_failures_remain_independent() {
        let kawasaki_fixture = octagonal_sheet(
            &[3, 5, 7, 0],
            &[
                EdgeKind::Mountain,
                EdgeKind::Mountain,
                EdgeKind::Mountain,
                EdgeKind::Valley,
            ],
        );
        let maekawa_fixture = octagonal_sheet(
            &[3, 5, 7, 1],
            &[
                EdgeKind::Mountain,
                EdgeKind::Mountain,
                EdgeKind::Valley,
                EdgeKind::Valley,
            ],
        );

        let (_, kawasaki) = report_for_center(&kawasaki_fixture);
        let (_, maekawa) = report_for_center(&maekawa_fixture);

        assert_eq!(
            (kawasaki.kawasaki, kawasaki.maekawa, kawasaki.verdict),
            (
                LocalFoldabilityConditionStatus::Violated,
                LocalFoldabilityConditionStatus::Satisfied,
                LocalVertexFoldabilityVerdict::Violated,
            )
        );
        assert_eq!(
            (maekawa.kawasaki, maekawa.maekawa, maekawa.verdict),
            (
                LocalFoldabilityConditionStatus::Satisfied,
                LocalFoldabilityConditionStatus::Violated,
                LocalVertexFoldabilityVerdict::Violated,
            )
        );
    }

    #[test]
    fn degree_two_and_degree_six_are_supported_instead_of_assuming_degree_four() {
        let degree_two = octagonal_sheet(&[3, 7], &[EdgeKind::Mountain, EdgeKind::Mountain]);
        // E, NE, N, W, SW, S has repeating sector angles
        // 45, 45, 90, 45, 45, 90 degrees.
        let degree_six = octagonal_sheet(
            &[3, 4, 5, 7, 0, 1],
            &[
                EdgeKind::Mountain,
                EdgeKind::Mountain,
                EdgeKind::Mountain,
                EdgeKind::Mountain,
                EdgeKind::Valley,
                EdgeKind::Valley,
            ],
        );

        for (fixture, expected_degree) in [(degree_two, 2), (degree_six, 6)] {
            let (_, center) = report_for_center(&fixture);
            assert_eq!(center.fold_degree, expected_degree);
            assert_eq!(center.verdict, LocalVertexFoldabilityVerdict::Satisfied);
            assert_eq!(center.kawasaki, LocalFoldabilityConditionStatus::Satisfied);
            assert_eq!(center.maekawa, LocalFoldabilityConditionStatus::Satisfied);
        }
    }

    #[test]
    fn report_is_invariant_under_storage_boundary_and_edge_direction_changes() {
        let fixture = octagonal_sheet(
            &[3, 5, 7, 1],
            &[
                EdgeKind::Mountain,
                EdgeKind::Mountain,
                EdgeKind::Mountain,
                EdgeKind::Valley,
            ],
        );
        let expected = analyze_local_flat_foldability(&fixture.paper, &fixture.pattern);
        let mut transformed_paper = fixture.paper.clone();
        transformed_paper.boundary_vertices.rotate_left(3);
        transformed_paper.boundary_vertices.reverse();
        let mut transformed_pattern = fixture.pattern.clone();
        transformed_pattern.vertices.reverse();
        transformed_pattern.edges.reverse();
        for edge in &mut transformed_pattern.edges {
            std::mem::swap(&mut edge.start, &mut edge.end);
        }

        assert_eq!(
            analyze_local_flat_foldability(&transformed_paper, &transformed_pattern),
            expected
        );
    }

    #[test]
    fn conditions_are_invariant_under_translation_power_of_two_scale_and_assignment_swap() {
        let fixture = octagonal_sheet(
            &[3, 5, 7, 1],
            &[
                EdgeKind::Mountain,
                EdgeKind::Mountain,
                EdgeKind::Mountain,
                EdgeKind::Valley,
            ],
        );
        let expected = analyze_local_flat_foldability(&fixture.paper, &fixture.pattern);

        let mut translated_pattern = fixture.pattern.clone();
        for vertex in &mut translated_pattern.vertices {
            vertex.position.x += 32.0;
            vertex.position.y -= 64.0;
        }
        assert_eq!(
            analyze_local_flat_foldability(&fixture.paper, &translated_pattern),
            expected
        );

        let mut scaled_pattern = fixture.pattern.clone();
        for vertex in &mut scaled_pattern.vertices {
            vertex.position.x *= 8.0;
            vertex.position.y *= 8.0;
        }
        assert_eq!(
            analyze_local_flat_foldability(&fixture.paper, &scaled_pattern),
            expected
        );

        let mut swapped_pattern = fixture.pattern.clone();
        for edge in &mut swapped_pattern.edges {
            edge.kind = match edge.kind {
                EdgeKind::Mountain => EdgeKind::Valley,
                EdgeKind::Valley => EdgeKind::Mountain,
                other => other,
            };
        }
        let swapped = analyze_local_flat_foldability(&fixture.paper, &swapped_pattern);
        assert_eq!(swapped.status, expected.status);
        assert_eq!(swapped.total_vertices, expected.total_vertices);
        assert_eq!(swapped.applicable_vertices, expected.applicable_vertices);
        assert_eq!(swapped.satisfied_vertices, expected.satisfied_vertices);
        assert_eq!(swapped.violated_vertices, expected.violated_vertices);
        assert_eq!(
            swapped.not_applicable_vertices,
            expected.not_applicable_vertices
        );
        assert_eq!(
            swapped.indeterminate_vertices,
            expected.indeterminate_vertices
        );
        for (actual, original) in swapped.vertices.iter().zip(&expected.vertices) {
            assert_eq!(actual.vertex, original.vertex);
            assert_eq!(actual.fold_degree, original.fold_degree);
            assert_eq!(actual.mountain_count, original.valley_count);
            assert_eq!(actual.valley_count, original.mountain_count);
            assert_eq!(actual.verdict, original.verdict);
            assert_eq!(actual.reason, original.reason);
            assert_eq!(actual.kawasaki, original.kawasaki);
            assert_eq!(actual.maekawa, original.maekawa);
        }
    }

    #[test]
    fn odd_degree_violates_both_conditions_without_becoming_not_applicable() {
        let fixture = octagonal_sheet(
            &[3, 5, 7],
            &[EdgeKind::Mountain, EdgeKind::Mountain, EdgeKind::Valley],
        );

        let (_, center) = report_for_center(&fixture);

        assert_eq!(center.fold_degree, 3);
        assert_eq!(center.kawasaki, LocalFoldabilityConditionStatus::Violated);
        assert_eq!(center.maekawa, LocalFoldabilityConditionStatus::Violated);
        assert_eq!(center.verdict, LocalVertexFoldabilityVerdict::Violated);
        assert_eq!(center.reason, None);
    }

    #[test]
    fn dangling_crease_reaches_local_conditions_without_requiring_face_walks() {
        let mut fixture = octagonal_sheet(&[], &[]);
        let center = fixture
            .pattern
            .vertices
            .iter()
            .find(|vertex| vertex.id == fixture.center)
            .expect("center")
            .clone();
        let interior_end = vertex(0x540, 15.0, 10.0);
        fixture.pattern.vertices.push(interior_end.clone());
        fixture
            .pattern
            .edges
            .push(edge(0x541, &center, &interior_end, EdgeKind::Mountain));

        let report = analyze_local_flat_foldability(&fixture.paper, &fixture.pattern);

        assert_ne!(report.status, LocalFlatFoldabilityReportStatus::Blocked);
        for vertex in [center.id, interior_end.id] {
            let result = report
                .vertices
                .iter()
                .find(|result| result.vertex == vertex)
                .expect("dangling crease endpoint report");
            assert_eq!(result.fold_degree, 1);
            assert_eq!(result.verdict, LocalVertexFoldabilityVerdict::Violated);
            assert_eq!(result.kawasaki, LocalFoldabilityConditionStatus::Violated);
            assert_eq!(result.maekawa, LocalFoldabilityConditionStatus::Violated);
        }
    }

    #[test]
    fn every_vertex_is_returned_in_canonical_id_order_with_stable_reasons() {
        let fixture = octagonal_sheet(&[], &[]);
        let mut pattern = fixture.pattern;
        let auxiliary_only = vertex(0x050, f64::NAN, f64::INFINITY);
        let isolated = vertex(0x051, 1000.0, 1000.0);
        let missing: VertexId = fixed_id(0x052);
        pattern
            .vertices
            .extend([isolated.clone(), auxiliary_only.clone()]);
        pattern.edges.push(Edge {
            id: fixed_id(0x053),
            start: auxiliary_only.id,
            end: missing,
            kind: EdgeKind::Auxiliary,
        });

        let report = analyze_local_flat_foldability(&fixture.paper, &pattern);

        assert_eq!(
            report.status,
            LocalFlatFoldabilityReportStatus::NotApplicable
        );
        assert_eq!(report.total_vertices, pattern.vertices.len());
        assert_eq!(report.applicable_vertices, 0);
        assert_eq!(report.not_applicable_vertices, pattern.vertices.len());
        assert!(
            report.vertices.windows(2).all(|pair| {
                pair[0].vertex.canonical_bytes() < pair[1].vertex.canonical_bytes()
            })
        );
        for id in [auxiliary_only.id, isolated.id] {
            let vertex = report
                .vertices
                .iter()
                .find(|vertex| vertex.vertex == id)
                .expect("draft vertex report");
            assert_eq!(
                vertex.reason,
                Some(LocalFoldabilityReason::NoIncidentFoldEdges)
            );
            assert_eq!(
                vertex.kawasaki,
                LocalFoldabilityConditionStatus::NotApplicable
            );
            assert_eq!(
                vertex.maekawa,
                LocalFoldabilityConditionStatus::NotApplicable
            );
        }
        assert!(
            report
                .vertices
                .iter()
                .filter(|vertex| { vertex.reason == Some(LocalFoldabilityReason::PaperBoundary) })
                .count()
                >= fixture.paper.boundary_vertices.len()
        );
    }

    #[test]
    fn an_incident_cut_is_not_applicable_but_does_not_block_other_vertices() {
        let mut fixture = octagonal_sheet(
            &[3, 5, 7, 1],
            &[
                EdgeKind::Mountain,
                EdgeKind::Mountain,
                EdgeKind::Mountain,
                EdgeKind::Valley,
            ],
        );
        fixture.paper.cutting_allowed = true;
        let center = fixture
            .pattern
            .vertices
            .iter()
            .find(|vertex| vertex.id == fixture.center)
            .expect("center")
            .clone();
        let cut_end = vertex(0x500, 15.0, 15.0);
        fixture.pattern.vertices.push(cut_end.clone());
        fixture
            .pattern
            .edges
            .push(edge(0x501, &center, &cut_end, EdgeKind::Cut));

        let (report, center) = report_for_center(&fixture);

        assert_ne!(report.status, LocalFlatFoldabilityReportStatus::Blocked);
        assert_eq!(center.verdict, LocalVertexFoldabilityVerdict::NotApplicable);
        assert_eq!(center.reason, Some(LocalFoldabilityReason::CutIncident));
        assert_eq!(
            center.kawasaki,
            LocalFoldabilityConditionStatus::NotApplicable
        );
        assert_eq!(
            center.maekawa,
            LocalFoldabilityConditionStatus::NotApplicable
        );
    }

    #[test]
    fn prohibited_cut_duplicate_ids_and_same_ray_multiedges_block_without_partial_counts() {
        let mut prohibited_cut = octagonal_sheet(&[], &[]);
        let center = prohibited_cut
            .pattern
            .vertices
            .iter()
            .find(|vertex| vertex.id == prohibited_cut.center)
            .expect("center")
            .clone();
        let cut_end = vertex(0x510, 15.0, 15.0);
        prohibited_cut.pattern.vertices.push(cut_end.clone());
        prohibited_cut
            .pattern
            .edges
            .push(edge(0x511, &center, &cut_end, EdgeKind::Cut));

        let mut duplicate = octagonal_sheet(&[], &[]);
        duplicate
            .pattern
            .vertices
            .push(duplicate.pattern.vertices[0].clone());

        let mut same_ray = octagonal_sheet(&[], &[]);
        let center = same_ray
            .pattern
            .vertices
            .iter()
            .find(|vertex| vertex.id == same_ray.center)
            .expect("center")
            .clone();
        let near = vertex(0x520, 15.0, 10.0);
        let far = same_ray.pattern.vertices[3].clone();
        same_ray.pattern.vertices.push(near.clone());
        same_ray.pattern.edges.extend([
            edge(0x521, &center, &near, EdgeKind::Mountain),
            edge(0x522, &center, &far, EdgeKind::Valley),
        ]);

        for fixture in [prohibited_cut, duplicate, same_ray] {
            let report = analyze_local_flat_foldability(&fixture.paper, &fixture.pattern);
            assert_eq!(report, LocalFlatFoldabilityReport::blocked());
        }
    }

    fn high_degree_sheet(fold_degree: usize, mountain_count: usize) -> SheetFixture {
        assert!(fold_degree >= 4 && fold_degree.is_multiple_of(2));
        assert!(mountain_count <= fold_degree);
        let half = fold_degree / 2;
        let mut vertices = (0..half)
            .map(|index| {
                let x = -((half - 1) as f64) + 2.0 * index as f64;
                vertex(0x1000 + index as u64, x, -1000.0)
            })
            .collect::<Vec<_>>();
        vertices.extend((0..half).rev().map(|index| {
            let x = -((half - 1) as f64) + 2.0 * index as f64;
            vertex(0x2000 + index as u64, x, 1000.0)
        }));
        let center = vertex(0x3000, 0.0, 0.0);
        let center_id = center.id;
        let boundary_len = vertices.len();
        let mut edges = (0..boundary_len)
            .map(|index| {
                edge(
                    0x4000 + index as u64,
                    &vertices[index],
                    &vertices[(index + 1) % boundary_len],
                    EdgeKind::Boundary,
                )
            })
            .collect::<Vec<_>>();
        edges.extend((0..fold_degree).map(|index| {
            edge(
                0x5000 + index as u64,
                &center,
                &vertices[index],
                if index < mountain_count {
                    EdgeKind::Mountain
                } else {
                    EdgeKind::Valley
                },
            )
        }));
        let paper = Paper {
            boundary_vertices: vertices.iter().map(|vertex| vertex.id).collect(),
            ..Paper::default()
        };
        vertices.push(center);
        SheetFixture {
            paper,
            pattern: CreasePattern { vertices, edges },
            center: center_id,
        }
    }

    #[test]
    fn degree_limit_only_makes_kawasaki_indeterminate_and_maekawa_still_decides() {
        let satisfiable_maekawa =
            high_degree_sheet(MAX_EXACT_FOLD_DEGREE + 2, MAX_EXACT_FOLD_DEGREE / 2 + 2);
        let violated_maekawa =
            high_degree_sheet(MAX_EXACT_FOLD_DEGREE + 2, MAX_EXACT_FOLD_DEGREE / 2 + 1);

        let (indeterminate_report, indeterminate) = report_for_center(&satisfiable_maekawa);
        assert_eq!(
            indeterminate_report.status,
            LocalFlatFoldabilityReportStatus::Indeterminate
        );
        assert_eq!(
            indeterminate.kawasaki,
            LocalFoldabilityConditionStatus::Indeterminate
        );
        assert_eq!(
            indeterminate.maekawa,
            LocalFoldabilityConditionStatus::Satisfied
        );
        assert_eq!(
            indeterminate.verdict,
            LocalVertexFoldabilityVerdict::Indeterminate
        );
        assert_eq!(
            indeterminate.reason,
            Some(LocalFoldabilityReason::FoldDegreeLimit)
        );

        let (violated_report, violated) = report_for_center(&violated_maekawa);
        assert_eq!(
            violated_report.status,
            LocalFlatFoldabilityReportStatus::Violated
        );
        assert_eq!(
            violated.kawasaki,
            LocalFoldabilityConditionStatus::Indeterminate
        );
        assert_eq!(violated.maekawa, LocalFoldabilityConditionStatus::Violated);
        assert_eq!(violated.verdict, LocalVertexFoldabilityVerdict::Violated);
        assert_eq!(violated.reason, None);
    }

    #[test]
    fn exact_degree_cap_is_evaluated_instead_of_becoming_indeterminate() {
        let fixture = high_degree_sheet(MAX_EXACT_FOLD_DEGREE, MAX_EXACT_FOLD_DEGREE / 2 + 1);

        let (_, center) = report_for_center(&fixture);

        assert_ne!(
            center.kawasaki,
            LocalFoldabilityConditionStatus::Indeterminate
        );
        assert_eq!(center.maekawa, LocalFoldabilityConditionStatus::Satisfied);
    }

    #[test]
    fn exact_products_reject_a_subnormal_near_miss_without_an_epsilon() {
        let subnormal = f64::from_bits(1);
        let origin = Point2::new(0.0, 0.0);
        let exact = [
            Point2::new(1.0, 0.0),
            Point2::new(0.0, 1.0),
            Point2::new(-1.0, 0.0),
            Point2::new(0.0, -1.0),
        ];
        let near_miss = [exact[0], Point2::new(subnormal, 1.0), exact[2], exact[3]];

        assert_eq!(
            exact_ordered_rays_kawasaki_condition(origin, &exact),
            Some(true)
        );
        assert_eq!(
            exact_ordered_rays_kawasaki_condition(origin, &near_miss),
            Some(false)
        );
    }

    #[test]
    fn exact_products_handle_overflowing_subtraction_and_independent_ray_lengths() {
        let center_x = -f64::MAX / 2.0;
        let origin = Point2::new(center_x, 0.0);
        let extreme = [
            Point2::new(f64::MAX, 0.0),
            Point2::new(center_x, f64::MAX),
            Point2::new(-f64::MAX, 0.0),
            Point2::new(center_x, -f64::MAX),
        ];
        assert!((extreme[0].x - origin.x).is_infinite());
        assert_eq!(
            exact_ordered_rays_kawasaki_condition(origin, &extreme),
            Some(true)
        );

        let scaled = [
            Point2::new(0.5, 0.0),
            Point2::new(0.0, 8.0),
            Point2::new(-32.0, 0.0),
            Point2::new(0.0, -0.25),
        ];
        assert_eq!(
            exact_ordered_rays_kawasaki_condition(Point2::new(0.0, 0.0), &scaled),
            Some(true)
        );
    }

    #[test]
    fn serialized_contract_uses_only_fixed_snake_case_statuses_and_null_reason() {
        let fixture = octagonal_sheet(
            &[3, 5, 7, 1],
            &[
                EdgeKind::Mountain,
                EdgeKind::Mountain,
                EdgeKind::Mountain,
                EdgeKind::Valley,
            ],
        );
        let (report, center) = report_for_center(&fixture);

        let report_json = serde_json::to_value(report).expect("serialize report");
        let center_json = serde_json::to_value(center).expect("serialize center");

        assert_eq!(
            report_json["model"],
            "interior_single_vertex_zero_thickness_v1"
        );
        assert_eq!(report_json["status"], "necessary_conditions_satisfied");
        assert_eq!(report_json["max_exact_fold_degree"], MAX_EXACT_FOLD_DEGREE);
        assert_eq!(center_json["verdict"], "satisfied");
        assert_eq!(center_json["reason"], serde_json::Value::Null);
        assert_eq!(center_json["kawasaki"], "satisfied");
        assert_eq!(center_json["maekawa"], "satisfied");
    }
}
