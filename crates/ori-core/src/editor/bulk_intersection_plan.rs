//! Bounded, read-only normalization for automatic intersection repair.
//!
//! This module grants no edit authority. It exists to seal the exact set of
//! point clusters and fail-closed gaps before an atomic editor delta exists.

use std::collections::BTreeMap;

use ori_domain::{CreasePattern, EdgeId, Point2};
use ori_geometry::{SegmentIntersection, ValidationIssue, validate_crease_pattern};

pub const MAX_BULK_INTERSECTION_PAIR_WORK_V1: usize = 4_096;
pub const MAX_BULK_INTERSECTION_ITEMS_V1: usize = 4_096;

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
}
