//! Bounded, read-only normalization for automatic intersection repair.
//!
//! This module grants no edit authority. It exists to seal the exact set of
//! point clusters and fail-closed gaps before an atomic editor delta exists.

use std::collections::{BTreeMap, HashMap, HashSet};

use num_bigint::BigInt;
use ori_domain::{
    CreasePattern, EdgeId, MAX_LAYER_EDGE_ASSIGNMENTS, Point2, ProjectLayerDocumentV1, VertexId,
    validate_project_layer_document_against_pattern_v1,
};
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

#[derive(Debug, Clone, PartialEq)]
pub struct BulkAtomicDeltaPrerequisiteV1 {
    source: CreasePattern,
    changed_edges: Vec<EdgeId>,
    reserved_junctions: Vec<VertexId>,
    reserved_segments: Vec<EdgeId>,
    layer_inheritance_count: usize,
    explicit_layer_assignment_count: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AtomicBulkIntersectionDeltaV1 {
    source_pattern: CreasePattern,
    target_pattern: CreasePattern,
    source_layers: ProjectLayerDocumentV1,
    target_layers: ProjectLayerDocumentV1,
    constraint_affected_edges: Vec<EdgeId>,
}

impl AtomicBulkIntersectionDeltaV1 {
    pub const fn authorizes_execution(&self) -> bool {
        false
    }
    pub fn is_for(&self, pattern: &CreasePattern, layers: &ProjectLayerDocumentV1) -> bool {
        &self.source_pattern == pattern && &self.source_layers == layers
    }
    pub fn target_pattern(&self) -> &CreasePattern {
        &self.target_pattern
    }
    pub fn target_layers(&self) -> &ProjectLayerDocumentV1 {
        &self.target_layers
    }
    pub fn inverse_pattern(&self) -> &CreasePattern {
        &self.source_pattern
    }
    pub fn inverse_layers(&self) -> &ProjectLayerDocumentV1 {
        &self.source_layers
    }
    pub fn constraint_affected_edges(&self) -> &[EdgeId] {
        &self.constraint_affected_edges
    }
}

impl BulkAtomicDeltaPrerequisiteV1 {
    pub const fn authorizes_execution(&self) -> bool {
        false
    }
    pub fn is_for(&self, pattern: &CreasePattern) -> bool {
        &self.source == pattern
    }
    pub fn changed_edges(&self) -> &[EdgeId] {
        &self.changed_edges
    }
    pub fn reserved_junctions(&self) -> &[VertexId] {
        &self.reserved_junctions
    }
    pub fn reserved_segments(&self) -> &[EdgeId] {
        &self.reserved_segments
    }
    pub const fn layer_inheritance_count(&self) -> usize {
        self.layer_inheritance_count
    }
    pub const fn explicit_layer_assignment_count(&self) -> usize {
        self.explicit_layer_assignment_count
    }
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

/// Seals the exact ID reservation and explicit layer-inheritance budget an
/// eventual atomic delta would require. This is a prerequisite only: no
/// command, history entry, or mutation authority is produced.
pub fn seal_bulk_atomic_delta_prerequisite_v1(
    pattern: &CreasePattern,
    layers: &ProjectLayerDocumentV1,
    plan: &BulkSubdivisionPlanV1,
    reserved_junctions: &[VertexId],
    reserved_segments: &[EdgeId],
) -> Option<BulkAtomicDeltaPrerequisiteV1> {
    if !plan.is_for(pattern)
        || validate_project_layer_document_against_pattern_v1(layers, pattern).is_err()
    {
        return None;
    }
    let required_junctions = plan
        .edges()
        .iter()
        .flat_map(BulkEdgeSubdivisionPlanV1::points)
        .map(BulkSubdivisionPointV1::point_bits)
        .collect::<HashSet<_>>()
        .len();
    let required_segments = plan.edges().iter().try_fold(0_usize, |count, edge| {
        count.checked_add(edge.points().len().checked_add(1)?)
    })?;
    if required_junctions != reserved_junctions.len()
        || required_segments != reserved_segments.len()
        || required_junctions > MAX_BULK_SUBDIVISION_RECORDS_V1
        || required_segments > MAX_BULK_SUBDIVISION_RECORDS_V1
    {
        return None;
    }
    let existing_vertex_ids = pattern
        .vertices
        .iter()
        .map(|vertex| vertex.id)
        .collect::<HashSet<_>>();
    let existing_edge_ids = pattern
        .edges
        .iter()
        .map(|edge| edge.id)
        .collect::<HashSet<_>>();
    if existing_vertex_ids.len() != pattern.vertices.len()
        || existing_edge_ids.len() != pattern.edges.len()
        || reserved_junctions
            .iter()
            .any(|id| id.canonical_bytes() == [0; 16] || existing_vertex_ids.contains(id))
        || reserved_segments
            .iter()
            .any(|id| id.canonical_bytes() == [0; 16] || existing_edge_ids.contains(id))
        || reserved_junctions
            .iter()
            .copied()
            .collect::<HashSet<_>>()
            .len()
            != reserved_junctions.len()
        || reserved_segments
            .iter()
            .copied()
            .collect::<HashSet<_>>()
            .len()
            != reserved_segments.len()
    {
        return None;
    }
    let mut changed_edges = plan
        .edges()
        .iter()
        .map(BulkEdgeSubdivisionPlanV1::edge)
        .collect::<Vec<_>>();
    changed_edges.sort_unstable_by_key(EdgeId::canonical_bytes);
    if changed_edges.windows(2).any(|pair| pair[0] == pair[1]) {
        return None;
    }
    let explicit_layer_assignment_count =
        plan.edges().iter().try_fold(0_usize, |count, edge| {
            let inherited = layers
                .edge_assignments
                .binary_search_by_key(&edge.edge().canonical_bytes(), |assignment| {
                    assignment.edge.canonical_bytes()
                })
                .ok()
                .map_or(0, |_| edge.points().len() + 1);
            count.checked_add(inherited)
        })?;
    let removed_assignment_count = plan
        .edges()
        .iter()
        .filter(|edge| {
            layers
                .edge_assignments
                .binary_search_by_key(&edge.edge().canonical_bytes(), |assignment| {
                    assignment.edge.canonical_bytes()
                })
                .is_ok()
        })
        .count();
    if layers
        .edge_assignments
        .len()
        .checked_sub(removed_assignment_count)?
        .checked_add(explicit_layer_assignment_count)?
        > MAX_LAYER_EDGE_ASSIGNMENTS
    {
        return None;
    }
    Some(BulkAtomicDeltaPrerequisiteV1 {
        source: pattern.clone(),
        changed_edges,
        reserved_junctions: reserved_junctions.to_vec(),
        reserved_segments: reserved_segments.to_vec(),
        layer_inheritance_count: required_segments,
        explicit_layer_assignment_count,
    })
}

pub fn build_atomic_bulk_intersection_delta_v1(
    pattern: &CreasePattern,
    layers: &ProjectLayerDocumentV1,
    plan: &BulkSubdivisionPlanV1,
    prerequisite: &BulkAtomicDeltaPrerequisiteV1,
) -> Option<AtomicBulkIntersectionDeltaV1> {
    let resealed = seal_bulk_atomic_delta_prerequisite_v1(
        pattern,
        layers,
        plan,
        prerequisite.reserved_junctions(),
        prerequisite.reserved_segments(),
    )?;
    if !plan.is_for(pattern) || &resealed != prerequisite || prerequisite.authorizes_execution() {
        return None;
    }
    // Endpoint identity coalescing is a distinct topology operation. Emitting
    // an n+1 replacement here would create a zero-length segment, so this
    // bounded delta fails closed until that inverse can be represented.
    if plan
        .edges()
        .iter()
        .flat_map(BulkEdgeSubdivisionPlanV1::points)
        .any(|point| {
            point.parameter().numerator() == &BigInt::from(0_u8)
                || point.parameter().numerator() == point.parameter().denominator()
        })
    {
        return None;
    }
    let changed = prerequisite
        .changed_edges()
        .iter()
        .copied()
        .collect::<HashSet<_>>();
    if changed.len() != prerequisite.changed_edges().len()
        || changed
            != plan
                .edges()
                .iter()
                .map(BulkEdgeSubdivisionPlanV1::edge)
                .collect()
    {
        return None;
    }

    let mut point_bits = plan
        .edges()
        .iter()
        .flat_map(BulkEdgeSubdivisionPlanV1::points)
        .map(BulkSubdivisionPointV1::point_bits)
        .collect::<Vec<_>>();
    point_bits.sort_unstable();
    point_bits.dedup();
    let mut junction_ids = prerequisite.reserved_junctions().to_vec();
    junction_ids.sort_unstable_by_key(VertexId::canonical_bytes);
    if point_bits.len() != junction_ids.len() {
        return None;
    }
    let junction_by_point = point_bits
        .into_iter()
        .zip(junction_ids.iter().copied())
        .collect::<HashMap<_, _>>();

    let mut segment_ids = prerequisite.reserved_segments().to_vec();
    segment_ids.sort_unstable_by_key(EdgeId::canonical_bytes);
    let mut segment_cursor = 0_usize;
    let edge_by_id = pattern
        .edges
        .iter()
        .map(|edge| (edge.id, edge))
        .collect::<HashMap<_, _>>();
    let mut new_edges = Vec::with_capacity(segment_ids.len());
    let mut inherited = Vec::new();
    let mut ordered_plans = plan.edges().iter().collect::<Vec<_>>();
    ordered_plans.sort_unstable_by_key(|edge| edge.edge().canonical_bytes());
    for edge_plan in ordered_plans {
        let original = edge_by_id.get(&edge_plan.edge())?;
        let mut vertices = Vec::with_capacity(edge_plan.points().len() + 2);
        vertices.push(original.start);
        for point in edge_plan.points() {
            vertices.push(*junction_by_point.get(&point.point_bits())?);
        }
        vertices.push(original.end);
        let explicit_layer = layers
            .edge_assignments
            .iter()
            .find(|assignment| assignment.edge == original.id)
            .map(|a| a.layer);
        for pair in vertices.windows(2) {
            let id = *segment_ids.get(segment_cursor)?;
            segment_cursor += 1;
            new_edges.push(ori_domain::Edge {
                id,
                start: pair[0],
                end: pair[1],
                kind: original.kind,
            });
            if let Some(layer) = explicit_layer {
                inherited.push(ori_domain::EdgeLayerAssignmentV1 { edge: id, layer });
            }
        }
    }
    if segment_cursor != segment_ids.len() {
        return None;
    }
    let mut target_pattern = pattern.clone();
    target_pattern
        .edges
        .retain(|edge| !changed.contains(&edge.id));
    target_pattern.edges.extend(new_edges);
    target_pattern
        .vertices
        .extend(
            junction_by_point
                .into_iter()
                .map(|(bits, id)| ori_domain::Vertex {
                    id,
                    position: Point2 {
                        x: f64::from_bits(bits[0]),
                        y: f64::from_bits(bits[1]),
                    },
                }),
        );
    target_pattern
        .vertices
        .sort_unstable_by_key(|vertex| vertex.id.canonical_bytes());
    target_pattern
        .edges
        .sort_unstable_by_key(|edge| edge.id.canonical_bytes());
    let mut target_layers = layers.clone();
    target_layers
        .edge_assignments
        .retain(|assignment| !changed.contains(&assignment.edge));
    target_layers.edge_assignments.extend(inherited);
    target_layers
        .edge_assignments
        .sort_unstable_by_key(|assignment| assignment.edge.canonical_bytes());
    if validate_project_layer_document_against_pattern_v1(&target_layers, &target_pattern).is_err()
    {
        return None;
    }
    Some(AtomicBulkIntersectionDeltaV1 {
        source_pattern: pattern.clone(),
        target_pattern,
        source_layers: layers.clone(),
        target_layers,
        constraint_affected_edges: prerequisite.changed_edges().to_vec(),
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
    use ori_domain::{
        Edge, EdgeKind, EdgeLayerAssignmentV1, LayerContentKindV1, LayerId, LayerRecordV1, Vertex,
    };

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
        let layers = ProjectLayerDocumentV1::default();
        let junctions = [ori_domain::VertexId::new()];
        let segments = (0..10).map(|_| EdgeId::new()).collect::<Vec<_>>();
        let prerequisite =
            seal_bulk_atomic_delta_prerequisite_v1(&pattern, &layers, &plan, &junctions, &segments)
                .unwrap();
        assert!(
            build_atomic_bulk_intersection_delta_v1(&pattern, &layers, &plan, &prerequisite)
                .is_none()
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

    fn layers_with_explicit_edge(edge: EdgeId) -> ProjectLayerDocumentV1 {
        let layer = LayerId::new();
        let mut document = ProjectLayerDocumentV1::default();
        document.layers.push(LayerRecordV1 {
            id: layer,
            name: "repair inheritance".into(),
            content_kind: LayerContentKindV1::CreasePattern,
            visible: true,
            locked: false,
            opacity: 1.0,
        });
        document
            .edge_assignments
            .push(EdgeLayerAssignmentV1 { edge, layer });
        document
    }

    #[test]
    fn seals_four_eight_sixteen_atomic_reservations_and_layer_inheritance() {
        for count in [4, 8, 16] {
            let (pattern, carrier) = one_edge_many_points(count, false);
            let registry = normalize_bulk_intersections_v1(&pattern).unwrap();
            let plan = plan_bulk_edge_subdivisions_v1(&pattern, &registry).unwrap();
            let junctions = (0..count)
                .map(|_| ori_domain::VertexId::new())
                .collect::<Vec<_>>();
            let segments = (0..(count * 3 + 1))
                .map(|_| EdgeId::new())
                .collect::<Vec<_>>();
            let prerequisite = seal_bulk_atomic_delta_prerequisite_v1(
                &pattern,
                &layers_with_explicit_edge(carrier),
                &plan,
                &junctions,
                &segments,
            )
            .unwrap();
            assert!(prerequisite.is_for(&pattern));
            assert!(!prerequisite.authorizes_execution());
            assert_eq!(prerequisite.changed_edges().len(), count + 1);
            assert_eq!(prerequisite.reserved_junctions(), junctions);
            assert_eq!(prerequisite.reserved_segments(), segments);
            assert_eq!(prerequisite.layer_inheritance_count(), count * 3 + 1);
            assert_eq!(prerequisite.explicit_layer_assignment_count(), count + 1);
        }
    }

    #[test]
    fn atomic_reservation_rejects_missing_duplicate_existing_and_caps() {
        let (pattern, _) = one_edge_many_points(4, false);
        let registry = normalize_bulk_intersections_v1(&pattern).unwrap();
        let plan = plan_bulk_edge_subdivisions_v1(&pattern, &registry).unwrap();
        let junctions = (0..4)
            .map(|_| ori_domain::VertexId::new())
            .collect::<Vec<_>>();
        let segments = (0..13).map(|_| EdgeId::new()).collect::<Vec<_>>();
        let layers = ProjectLayerDocumentV1::default();
        assert!(
            seal_bulk_atomic_delta_prerequisite_v1(
                &pattern,
                &layers,
                &plan,
                &junctions[..3],
                &segments
            )
            .is_none()
        );
        let mut duplicate_junctions = junctions.clone();
        duplicate_junctions[1] = duplicate_junctions[0];
        assert!(
            seal_bulk_atomic_delta_prerequisite_v1(
                &pattern,
                &layers,
                &plan,
                &duplicate_junctions,
                &segments
            )
            .is_none()
        );
        let mut colliding_segments = segments.clone();
        colliding_segments[0] = pattern.edges[0].id;
        assert!(
            seal_bulk_atomic_delta_prerequisite_v1(
                &pattern,
                &layers,
                &plan,
                &junctions,
                &colliding_segments
            )
            .is_none()
        );

        let mut excessive = plan.clone();
        excessive.edges[0].points =
            vec![excessive.edges[0].points[0].clone(); MAX_BULK_SUBDIVISION_RECORDS_V1 + 1];
        assert!(
            seal_bulk_atomic_delta_prerequisite_v1(
                &pattern, &layers, &excessive, &junctions, &segments
            )
            .is_none()
        );
    }

    #[test]
    fn builds_four_eight_sixteen_complete_delta_and_inverse_without_authority() {
        for count in [4, 8, 16] {
            let (pattern, carrier) = one_edge_many_points(count, count == 8);
            let original = pattern.clone();
            let layers = layers_with_explicit_edge(carrier);
            let registry = normalize_bulk_intersections_v1(&pattern).unwrap();
            let plan = plan_bulk_edge_subdivisions_v1(&pattern, &registry).unwrap();
            let junctions = (0..count)
                .map(|_| ori_domain::VertexId::new())
                .collect::<Vec<_>>();
            let segments = (0..(count * 3 + 1))
                .map(|_| EdgeId::new())
                .collect::<Vec<_>>();
            let prerequisite = seal_bulk_atomic_delta_prerequisite_v1(
                &pattern, &layers, &plan, &junctions, &segments,
            )
            .unwrap();
            let delta =
                build_atomic_bulk_intersection_delta_v1(&pattern, &layers, &plan, &prerequisite)
                    .unwrap();
            assert_eq!(pattern, original);
            assert!(delta.is_for(&pattern, &layers));
            assert!(!delta.authorizes_execution());
            assert_eq!(delta.inverse_pattern(), &pattern);
            assert_eq!(delta.inverse_layers(), &layers);
            assert_eq!(
                delta.target_pattern().vertices.len(),
                pattern.vertices.len() + count
            );
            assert_eq!(delta.target_pattern().edges.len(), count * 3 + 1);
            assert_eq!(delta.target_layers().edge_assignments.len(), count + 1);
            assert_eq!(delta.constraint_affected_edges().len(), count + 1);
            assert!(delta.constraint_affected_edges().iter().all(|id| {
                !delta
                    .target_pattern()
                    .edges
                    .iter()
                    .any(|edge| edge.id == *id)
            }));
        }
    }

    #[test]
    fn delta_revalidates_source_reservations_and_tamper() {
        let (pattern, _) = one_edge_many_points(4, false);
        let layers = ProjectLayerDocumentV1::default();
        let registry = normalize_bulk_intersections_v1(&pattern).unwrap();
        let plan = plan_bulk_edge_subdivisions_v1(&pattern, &registry).unwrap();
        let junctions = (0..4)
            .map(|_| ori_domain::VertexId::new())
            .collect::<Vec<_>>();
        let segments = (0..13).map(|_| EdgeId::new()).collect::<Vec<_>>();
        let prerequisite =
            seal_bulk_atomic_delta_prerequisite_v1(&pattern, &layers, &plan, &junctions, &segments)
                .unwrap();
        let mut tampered_source = pattern.clone();
        tampered_source.vertices[0].position.x = -9.0;
        assert!(
            build_atomic_bulk_intersection_delta_v1(
                &tampered_source,
                &layers,
                &plan,
                &prerequisite
            )
            .is_none()
        );
        let mut tampered_reservation = prerequisite.clone();
        tampered_reservation.reserved_segments[0] = pattern.edges[0].id;
        assert!(
            build_atomic_bulk_intersection_delta_v1(
                &pattern,
                &layers,
                &plan,
                &tampered_reservation
            )
            .is_none()
        );
        let mut tampered_changed = prerequisite.clone();
        tampered_changed.changed_edges.pop();
        assert!(
            build_atomic_bulk_intersection_delta_v1(&pattern, &layers, &plan, &tampered_changed)
                .is_none()
        );
    }
}
