//! Bounded, read-only normalization for automatic intersection repair.
//!
//! This module grants no edit authority. It exists to seal the exact set of
//! point clusters and fail-closed gaps before an atomic editor delta exists.

use std::collections::{BTreeMap, HashMap};

use num_bigint::BigInt;
use ori_domain::{CreasePattern, EdgeId, Point2};
use ori_geometry::{SegmentIntersection, ValidationIssue, validate_crease_pattern};

pub const MAX_BULK_INTERSECTION_PAIR_WORK_V1: usize = 4_096;
pub const MAX_BULK_INTERSECTION_ITEMS_V1: usize = 4_096;
pub const MAX_BULK_SUBDIVISION_RECORDS_V1: usize = 8_192;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct CanonicalPointBits([u64; 2]);

impl CanonicalPointBits {
    fn new(point: Point2) -> Option<Self> {
        if !point.x.is_finite() || !point.y.is_finite() {
            return None;
        }
        let bits = |value: f64| {
            if value == 0.0 {
                0.0_f64.to_bits()
            } else {
                value.to_bits()
            }
        };
        Some(Self([bits(point.x), bits(point.y)]))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BulkIntersectionGapV1 {
    CollinearOverlap,
    NonFinitePoint,
    ResourceLimit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BulkIntersectionClusterV1 {
    point: CanonicalPointBits,
    edges: Vec<EdgeId>,
}

impl BulkIntersectionClusterV1 {
    #[must_use]
    pub const fn point_bits(&self) -> [u64; 2] {
        self.point.0
    }
    #[must_use]
    pub fn edges(&self) -> &[EdgeId] {
        &self.edges
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BulkIntersectionPlanRegistryV1 {
    source: CreasePattern,
    clusters: Vec<BulkIntersectionClusterV1>,
    gaps: Vec<BulkIntersectionGapV1>,
}

/// Exact rational parameter along one original edge. Both operands are
/// integers in the common binary64 `2^-1074` lattice, so ordering introduces
/// no further floating-point rounding after the registry snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BulkSubdivisionParameterV1 {
    numerator: BigInt,
    denominator: BigInt,
}

impl BulkSubdivisionParameterV1 {
    pub fn numerator(&self) -> &BigInt {
        &self.numerator
    }
    pub fn denominator(&self) -> &BigInt {
        &self.denominator
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BulkSubdivisionPointV1 {
    point: CanonicalPointBits,
    parameter: BulkSubdivisionParameterV1,
}

impl BulkSubdivisionPointV1 {
    pub const fn point_bits(&self) -> [u64; 2] {
        self.point.0
    }
    pub const fn parameter(&self) -> &BulkSubdivisionParameterV1 {
        &self.parameter
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BulkEdgeSubdivisionPlanV1 {
    edge: EdgeId,
    points: Vec<BulkSubdivisionPointV1>,
}

impl BulkEdgeSubdivisionPlanV1 {
    pub const fn edge(&self) -> EdgeId {
        self.edge
    }
    pub fn points(&self) -> &[BulkSubdivisionPointV1] {
        &self.points
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BulkSubdivisionPlanV1 {
    source: CreasePattern,
    edges: Vec<BulkEdgeSubdivisionPlanV1>,
}

impl BulkSubdivisionPlanV1 {
    pub const fn authorizes_execution(&self) -> bool {
        false
    }
    pub fn is_for(&self, pattern: &CreasePattern) -> bool {
        &self.source == pattern
    }
    pub fn edges(&self) -> &[BulkEdgeSubdivisionPlanV1] {
        &self.edges
    }
}

fn exact_f64_lattice(value: f64) -> Option<BigInt> {
    if !value.is_finite() {
        return None;
    }
    let bits = value.to_bits();
    let negative = bits >> 63 != 0;
    let exponent = ((bits >> 52) & 0x7ff) as usize;
    let fraction = bits & ((1_u64 << 52) - 1);
    let (significand, shift) = if exponent == 0 {
        (fraction, 0)
    } else {
        ((1_u64 << 52) | fraction, exponent - 1)
    };
    let value = BigInt::from(significand) << shift;
    Some(if negative { -value } else { value })
}

/// Derives a read-only, canonical subdivision order from a sealed registry.
/// It deliberately cannot execute edits and rechecks the complete source
/// snapshot so a registry cannot be replayed against changed geometry.
pub fn plan_bulk_edge_subdivisions_v1(
    pattern: &CreasePattern,
    registry: &BulkIntersectionPlanRegistryV1,
) -> Option<BulkSubdivisionPlanV1> {
    if !registry.is_for(pattern) || !registry.gaps().is_empty() {
        return None;
    }
    let record_count = registry
        .clusters()
        .iter()
        .try_fold(0_usize, |count, cluster| {
            count.checked_add(cluster.edges().len())
        })?;
    if record_count > MAX_BULK_SUBDIVISION_RECORDS_V1 {
        return None;
    }
    let vertices = pattern
        .vertices
        .iter()
        .map(|vertex| (vertex.id, vertex.position))
        .collect::<HashMap<_, _>>();
    let edge_by_id = pattern
        .edges
        .iter()
        .map(|edge| (edge.id, edge))
        .collect::<HashMap<_, _>>();
    let mut per_edge = HashMap::<EdgeId, Vec<(BigInt, BigInt, CanonicalPointBits)>>::new();
    for cluster in registry.clusters() {
        let point = Point2 {
            x: f64::from_bits(cluster.point.0[0]),
            y: f64::from_bits(cluster.point.0[1]),
        };
        for edge_id in cluster.edges() {
            let edge = edge_by_id.get(edge_id)?;
            let start = vertices.get(&edge.start)?;
            let end = vertices.get(&edge.end)?;
            let use_x = (end.x - start.x).abs() >= (end.y - start.y).abs();
            let (start_coordinate, end_coordinate, point_coordinate) = if use_x {
                (start.x, end.x, point.x)
            } else {
                (start.y, end.y, point.y)
            };
            let mut numerator =
                exact_f64_lattice(point_coordinate)? - exact_f64_lattice(start_coordinate)?;
            let mut denominator =
                exact_f64_lattice(end_coordinate)? - exact_f64_lattice(start_coordinate)?;
            if denominator == BigInt::from(0_u8) {
                return None;
            }
            if denominator < BigInt::from(0_u8) {
                numerator = -numerator;
                denominator = -denominator;
            }
            if numerator < BigInt::from(0_u8) || numerator > denominator {
                return None;
            }
            per_edge
                .entry(*edge_id)
                .or_default()
                .push((numerator, denominator, cluster.point));
        }
    }
    let mut edges = Vec::with_capacity(per_edge.len());
    let mut per_edge = per_edge.into_iter().collect::<Vec<_>>();
    per_edge.sort_unstable_by_key(|(edge, _)| edge.canonical_bytes());
    for (edge, mut points) in per_edge {
        points.sort_by(|left, right| {
            (&left.0 * &right.1)
                .cmp(&(&right.0 * &left.1))
                .then_with(|| left.2.cmp(&right.2))
        });
        points.dedup_by(|left, right| left.2 == right.2);
        edges.push(BulkEdgeSubdivisionPlanV1 {
            edge,
            points: points
                .into_iter()
                .map(|(numerator, denominator, point)| BulkSubdivisionPointV1 {
                    point,
                    parameter: BulkSubdivisionParameterV1 {
                        numerator,
                        denominator,
                    },
                })
                .collect(),
        });
    }
    Some(BulkSubdivisionPlanV1 {
        source: pattern.clone(),
        edges,
    })
}

impl BulkIntersectionPlanRegistryV1 {
    pub const fn authorizes_execution(&self) -> bool {
        false
    }
    pub fn is_for(&self, pattern: &CreasePattern) -> bool {
        &self.source == pattern
    }
    pub fn clusters(&self) -> &[BulkIntersectionClusterV1] {
        &self.clusters
    }
    pub fn gaps(&self) -> &[BulkIntersectionGapV1] {
        &self.gaps
    }
}

pub fn normalize_bulk_intersections_v1(
    pattern: &CreasePattern,
) -> Option<BulkIntersectionPlanRegistryV1> {
    if pattern.vertices.len().checked_add(pattern.edges.len())? > MAX_BULK_INTERSECTION_ITEMS_V1 {
        return None;
    }
    let pair_count = pattern
        .edges
        .len()
        .checked_mul(pattern.edges.len().saturating_sub(1))
        .and_then(|n| n.checked_div(2));
    if pair_count.is_none_or(|count| count > MAX_BULK_INTERSECTION_PAIR_WORK_V1) {
        return None;
    }
    let validation = validate_crease_pattern(pattern);
    let issues = &validation.issues;
    if issues.len() > MAX_BULK_INTERSECTION_PAIR_WORK_V1 {
        return None;
    }
    let mut clusters = BTreeMap::<CanonicalPointBits, Vec<EdgeId>>::new();
    let mut gaps = Vec::new();
    for issue in issues {
        let ValidationIssue::UnsplitIntersection {
            first_edge,
            second_edge,
            intersection,
        } = issue
        else {
            continue;
        };
        match intersection {
            SegmentIntersection::Point(point) => match CanonicalPointBits::new(*point) {
                Some(key) => clusters
                    .entry(key)
                    .or_default()
                    .extend([*first_edge, *second_edge]),
                None => gaps.push(BulkIntersectionGapV1::NonFinitePoint),
            },
            SegmentIntersection::CollinearOverlap => {
                gaps.push(BulkIntersectionGapV1::CollinearOverlap)
            }
            SegmentIntersection::None => gaps.push(BulkIntersectionGapV1::NonFinitePoint),
        }
    }
    let clusters = clusters
        .into_iter()
        .map(|(point, mut edges)| {
            edges.sort_unstable_by_key(EdgeId::canonical_bytes);
            edges.dedup();
            BulkIntersectionClusterV1 { point, edges }
        })
        .collect();
    Some(BulkIntersectionPlanRegistryV1 {
        source: pattern.clone(),
        clusters,
        gaps,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ori_domain::{Edge, EdgeKind, Vertex};

    fn crossing_pattern(count: usize) -> CreasePattern {
        let mut pattern = CreasePattern::empty();
        for index in 0..count {
            let x = index as f64 * 10.0;
            let ids = [
                ori_domain::VertexId::new(),
                ori_domain::VertexId::new(),
                ori_domain::VertexId::new(),
                ori_domain::VertexId::new(),
            ];
            for (id, position) in ids.into_iter().zip([
                Point2 { x: x - 1.0, y: 0.0 },
                Point2 { x: x + 1.0, y: 0.0 },
                Point2 { x, y: -1.0 },
                Point2 { x, y: 1.0 },
            ]) {
                pattern.vertices.push(Vertex { id, position });
            }
            pattern.edges.push(Edge {
                id: EdgeId::new(),
                start: ids[0],
                end: ids[1],
                kind: EdgeKind::Mountain,
            });
            pattern.edges.push(Edge {
                id: EdgeId::new(),
                start: ids[2],
                end: ids[3],
                kind: EdgeKind::Valley,
            });
        }
        pattern
    }

    #[test]
    fn normalizes_four_eight_and_sixteen_exact_clusters_without_authority() {
        for count in [4, 8, 16] {
            let pattern = crossing_pattern(count);
            let report = normalize_bulk_intersections_v1(&pattern).unwrap();
            assert!(report.is_for(&pattern));
            assert_eq!(report.clusters().len(), count);
            assert!(report.clusters().iter().all(|cluster| {
                cluster.edges().len() == 2
                    && cluster
                        .edges()
                        .windows(2)
                        .all(|pair| pair[0].canonical_bytes() < pair[1].canonical_bytes())
                    && cluster.point_bits()[1] == 0.0_f64.to_bits()
            }));
            assert!(report.gaps().is_empty());
            assert!(!report.authorizes_execution());
            let mut foreign = pattern.clone();
            foreign.vertices[0].position.x = -99.0;
            assert!(!report.is_for(&foreign));
        }
    }

    #[test]
    fn preflights_edge_pair_resource_before_validation() {
        assert_eq!(
            CanonicalPointBits::new(Point2 { x: -0.0, y: 0.0 }),
            CanonicalPointBits::new(Point2 { x: 0.0, y: -0.0 })
        );
        let within = crossing_pattern(45); // 90 edges => 4005 pairs.
        assert!(
            !normalize_bulk_intersections_v1(&within)
                .unwrap()
                .gaps()
                .contains(&BulkIntersectionGapV1::ResourceLimit)
        );
        let excessive = crossing_pattern(46); // 92 edges => 4186 pairs.
        assert!(normalize_bulk_intersections_v1(&excessive).is_none());
    }

    fn one_edge_many_points(count: usize, reverse: bool) -> (CreasePattern, EdgeId) {
        let mut pattern = CreasePattern::empty();
        let horizontal = [ori_domain::VertexId::new(), ori_domain::VertexId::new()];
        for (id, position) in horizontal.into_iter().zip([
            Point2 { x: 0.0, y: 0.0 },
            Point2 {
                x: (count + 1) as f64,
                y: 0.0,
            },
        ]) {
            pattern.vertices.push(Vertex { id, position });
        }
        let carrier = EdgeId::new();
        pattern.edges.push(Edge {
            id: carrier,
            start: if reverse {
                horizontal[1]
            } else {
                horizontal[0]
            },
            end: if reverse {
                horizontal[0]
            } else {
                horizontal[1]
            },
            kind: EdgeKind::Mountain,
        });
        for index in 1..=count {
            let ids = [ori_domain::VertexId::new(), ori_domain::VertexId::new()];
            for (id, y) in ids.into_iter().zip([-2.0, 2.0]) {
                pattern.vertices.push(Vertex {
                    id,
                    position: Point2 { x: index as f64, y },
                });
            }
            pattern.edges.push(Edge {
                id: EdgeId::new(),
                start: ids[0],
                end: ids[1],
                kind: EdgeKind::Valley,
            });
        }
        (pattern, carrier)
    }

    #[test]
    fn plans_same_edge_four_eight_sixteen_points_in_exact_parameter_order() {
        for count in [4, 8, 16] {
            for reverse in [false, true] {
                let (pattern, carrier) = one_edge_many_points(count, reverse);
                let registry = normalize_bulk_intersections_v1(&pattern).unwrap();
                let plan = plan_bulk_edge_subdivisions_v1(&pattern, &registry).unwrap();
                assert!(plan.is_for(&pattern));
                assert!(!plan.authorizes_execution());
                let carrier_plan = plan
                    .edges()
                    .iter()
                    .find(|edge| edge.edge() == carrier)
                    .unwrap();
                assert_eq!(carrier_plan.points().len(), count);
                let xs = carrier_plan
                    .points()
                    .iter()
                    .map(|point| f64::from_bits(point.point_bits()[0]))
                    .collect::<Vec<_>>();
                let expected: Vec<_> = if reverse {
                    (1..=count).rev().map(|value| value as f64).collect()
                } else {
                    (1..=count).map(|value| value as f64).collect()
                };
                assert_eq!(xs, expected);
                assert!(carrier_plan.points().windows(2).all(|pair| {
                    pair[0].parameter().numerator() * pair[1].parameter().denominator()
                        < pair[1].parameter().numerator() * pair[0].parameter().denominator()
                }));
            }
        }
    }

    #[test]
    fn plans_vertical_diagonal_endpoint_and_duplicate_cluster_members_once() {
        let mut pattern = CreasePattern::empty();
        let mut add_edge = |start: Point2, end: Point2, kind| {
            let ids = [ori_domain::VertexId::new(), ori_domain::VertexId::new()];
            pattern.vertices.extend([
                Vertex {
                    id: ids[0],
                    position: start,
                },
                Vertex {
                    id: ids[1],
                    position: end,
                },
            ]);
            let id = EdgeId::new();
            pattern.edges.push(Edge {
                id,
                start: ids[0],
                end: ids[1],
                kind,
            });
            id
        };
        let vertical = add_edge(
            Point2 { x: 0.0, y: -2.0 },
            Point2 { x: 0.0, y: 2.0 },
            EdgeKind::Mountain,
        );
        add_edge(
            Point2 { x: -2.0, y: 0.0 },
            Point2 { x: 2.0, y: 0.0 },
            EdgeKind::Valley,
        );
        add_edge(
            Point2 { x: -2.0, y: -2.0 },
            Point2 { x: 2.0, y: 2.0 },
            EdgeKind::Auxiliary,
        );
        add_edge(
            Point2 { x: -2.0, y: 2.0 },
            Point2 { x: 2.0, y: -2.0 },
            EdgeKind::Cut,
        );
        let endpoint = add_edge(
            Point2 { x: 0.0, y: 0.0 },
            Point2 { x: 2.0, y: 1.0 },
            EdgeKind::Auxiliary,
        );
        let registry = normalize_bulk_intersections_v1(&pattern).unwrap();
        let plan = plan_bulk_edge_subdivisions_v1(&pattern, &registry).unwrap();
        let vertical_plan = plan
            .edges()
            .iter()
            .find(|edge| edge.edge() == vertical)
            .unwrap();
        assert_eq!(vertical_plan.points().len(), 1);
        assert_eq!(vertical_plan.points()[0].point_bits(), [0, 0]);
        assert_eq!(
            vertical_plan.points()[0].parameter().numerator() * 2_u8,
            vertical_plan.points()[0].parameter().denominator().clone()
        );
        let endpoint_plan = plan
            .edges()
            .iter()
            .find(|edge| edge.edge() == endpoint)
            .unwrap();
        assert_eq!(
            endpoint_plan.points()[0].parameter().numerator(),
            &BigInt::from(0_u8)
        );
    }

    #[test]
    fn rejects_tampered_source_gaps_and_record_cap_without_authority() {
        let (pattern, _) = one_edge_many_points(4, false);
        let registry = normalize_bulk_intersections_v1(&pattern).unwrap();
        let mut tampered = pattern.clone();
        tampered.vertices[0].position.x = -1.0;
        assert!(plan_bulk_edge_subdivisions_v1(&tampered, &registry).is_none());

        let mut gapped = registry.clone();
        gapped.gaps.push(BulkIntersectionGapV1::CollinearOverlap);
        assert!(plan_bulk_edge_subdivisions_v1(&pattern, &gapped).is_none());

        let cluster = registry.clusters[0].clone();
        let mut excessive = registry.clone();
        excessive.clusters = vec![cluster; MAX_BULK_SUBDIVISION_RECORDS_V1 + 1];
        assert!(plan_bulk_edge_subdivisions_v1(&pattern, &excessive).is_none());
    }
}
