use std::collections::HashMap;

use ori_domain::{CreasePattern, EdgeId, Point2, VertexId};

use crate::{GeometryError, SegmentIntersection, segment_intersection};

/// Identifies which endpoint of an edge references a missing vertex.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeEndpoint {
    Start,
    End,
}

/// A structural or geometric defect found in a crease pattern.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ValidationIssue {
    /// A vertex coordinate is `NaN` or infinite.
    NonFiniteVertex { vertex: VertexId, position: Point2 },
    /// Two distinct vertex records occupy exactly the same position.
    DuplicateVertex {
        first: VertexId,
        duplicate: VertexId,
        position: Point2,
    },
    /// An edge endpoint references a vertex that is absent from the pattern.
    MissingEndpoint {
        edge: EdgeId,
        endpoint: EdgeEndpoint,
        vertex: VertexId,
    },
    /// An edge has equal endpoints, either by ID or by position.
    ZeroLengthEdge { edge: EdgeId },
    /// Two edges intersect without both being split at one shared endpoint.
    UnsplitIntersection {
        first_edge: EdgeId,
        second_edge: EdgeId,
        intersection: SegmentIntersection,
    },
    /// Finite coordinates overflowed during intersection classification.
    IntersectionCalculationFailed {
        first_edge: EdgeId,
        second_edge: EdgeId,
        error: GeometryError,
    },
}

/// Complete validation result. Validation reports every detected issue instead
/// of failing at the first one, so the editor can highlight all affected items.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CreasePatternValidation {
    pub issues: Vec<ValidationIssue>,
}

impl CreasePatternValidation {
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.issues.is_empty()
    }

    #[must_use]
    pub fn into_issues(self) -> Vec<ValidationIssue> {
        self.issues
    }
}

/// Validates basic topology and segment intersections in a crease pattern.
///
/// The validator detects non-finite and duplicate vertices, missing edge
/// endpoints, zero-length edges, crossings, T-junctions, and collinear edge
/// overlaps. A point intersection is considered correctly split only when the
/// two edge records share the same endpoint vertex ID.
#[must_use]
pub fn validate_crease_pattern(pattern: &CreasePattern) -> CreasePatternValidation {
    let mut issues = Vec::new();
    let mut vertices = HashMap::with_capacity(pattern.vertices.len());
    let mut positions = HashMap::with_capacity(pattern.vertices.len());

    for vertex in &pattern.vertices {
        // Keep the first record for a repeated ID so endpoint resolution remains
        // deterministic even for malformed external data.
        vertices.entry(vertex.id).or_insert(vertex.position);

        if !is_finite(vertex.position) {
            issues.push(ValidationIssue::NonFiniteVertex {
                vertex: vertex.id,
                position: vertex.position,
            });
            continue;
        }

        let key = position_key(vertex.position);
        if let Some(first) = positions.get(&key) {
            issues.push(ValidationIssue::DuplicateVertex {
                first: *first,
                duplicate: vertex.id,
                position: vertex.position,
            });
        } else {
            positions.insert(key, vertex.id);
        }
    }

    let mut resolved_edges = Vec::with_capacity(pattern.edges.len());
    for (index, edge) in pattern.edges.iter().enumerate() {
        let start = vertices.get(&edge.start).copied();
        let end = vertices.get(&edge.end).copied();

        if start.is_none() {
            issues.push(ValidationIssue::MissingEndpoint {
                edge: edge.id,
                endpoint: EdgeEndpoint::Start,
                vertex: edge.start,
            });
        }
        if end.is_none() {
            issues.push(ValidationIssue::MissingEndpoint {
                edge: edge.id,
                endpoint: EdgeEndpoint::End,
                vertex: edge.end,
            });
        }

        let (Some(start), Some(end)) = (start, end) else {
            continue;
        };
        if edge.start == edge.end || start == end {
            issues.push(ValidationIssue::ZeroLengthEdge { edge: edge.id });
            continue;
        }
        // Non-finite vertices already have a direct issue. Do not compound it
        // with an issue for every edge pair that happens to reference them.
        if !is_finite(start) || !is_finite(end) {
            continue;
        }

        resolved_edges.push(ResolvedEdge {
            original_index: index,
            id: edge.id,
            start_id: edge.start,
            end_id: edge.end,
            start,
            end,
            bounds: Bounds::from_points(start, end),
        });
    }

    validate_intersections(&resolved_edges, &mut issues);
    CreasePatternValidation { issues }
}

#[derive(Debug, Clone, Copy)]
struct ResolvedEdge {
    original_index: usize,
    id: EdgeId,
    start_id: VertexId,
    end_id: VertexId,
    start: Point2,
    end: Point2,
    bounds: Bounds,
}

impl ResolvedEdge {
    fn shares_endpoint_with(self, other: Self) -> bool {
        self.start_id == other.start_id
            || self.start_id == other.end_id
            || self.end_id == other.start_id
            || self.end_id == other.end_id
    }
}

#[derive(Debug, Clone, Copy)]
struct Bounds {
    min_x: f64,
    max_x: f64,
    min_y: f64,
    max_y: f64,
}

impl Bounds {
    fn from_points(start: Point2, end: Point2) -> Self {
        Self {
            min_x: start.x.min(end.x),
            max_x: start.x.max(end.x),
            min_y: start.y.min(end.y),
            max_y: start.y.max(end.y),
        }
    }

    fn overlaps_y(self, other: Self) -> bool {
        self.min_y <= other.max_y && other.min_y <= self.max_y
    }
}

fn validate_intersections(edges: &[ResolvedEdge], issues: &mut Vec<ValidationIssue>) {
    // Sweep on the x-axis as a broad phase. This avoids testing every pair for
    // ordinary sparse diagrams while retaining exact narrow-phase semantics.
    let mut by_min_x: Vec<_> = (0..edges.len()).collect();
    by_min_x.sort_unstable_by(|left, right| {
        edges[*left]
            .bounds
            .min_x
            .total_cmp(&edges[*right].bounds.min_x)
            .then_with(|| {
                edges[*left]
                    .original_index
                    .cmp(&edges[*right].original_index)
            })
    });

    let mut found = Vec::new();
    for (position, left_index) in by_min_x.iter().copied().enumerate() {
        let left = edges[left_index];
        for right_index in by_min_x.iter().copied().skip(position + 1) {
            let right = edges[right_index];
            if right.bounds.min_x > left.bounds.max_x {
                break;
            }
            if !left.bounds.overlaps_y(right.bounds) {
                continue;
            }

            let (first, second) = if left.original_index < right.original_index {
                (left, right)
            } else {
                (right, left)
            };
            match segment_intersection(first.start, first.end, second.start, second.end) {
                Ok(SegmentIntersection::None) => {}
                Ok(SegmentIntersection::Point(_)) if first.shares_endpoint_with(second) => {}
                Ok(intersection) => found.push((
                    first.original_index,
                    second.original_index,
                    ValidationIssue::UnsplitIntersection {
                        first_edge: first.id,
                        second_edge: second.id,
                        intersection,
                    },
                )),
                Err(error) => found.push((
                    first.original_index,
                    second.original_index,
                    ValidationIssue::IntersectionCalculationFailed {
                        first_edge: first.id,
                        second_edge: second.id,
                        error,
                    },
                )),
            }
        }
    }

    found.sort_unstable_by_key(|(first, second, _)| (*first, *second));
    issues.extend(found.into_iter().map(|(_, _, issue)| issue));
}

fn is_finite(point: Point2) -> bool {
    point.x.is_finite() && point.y.is_finite()
}

fn position_key(point: Point2) -> (u64, u64) {
    (canonical_bits(point.x), canonical_bits(point.y))
}

fn canonical_bits(value: f64) -> u64 {
    // `-0.0 == 0.0`, so normalize both representations for duplicate checks.
    if value == 0.0 { 0 } else { value.to_bits() }
}

#[cfg(test)]
mod tests {
    use ori_domain::{Edge, EdgeKind, Vertex};

    use super::*;

    fn vertex(x: f64, y: f64) -> Vertex {
        Vertex {
            id: VertexId::new(),
            position: Point2::new(x, y),
        }
    }

    fn edge(start: &Vertex, end: &Vertex) -> Edge {
        Edge {
            id: EdgeId::new(),
            start: start.id,
            end: end.id,
            kind: EdgeKind::Valley,
        }
    }

    #[test]
    fn empty_pattern_is_valid() {
        assert!(validate_crease_pattern(&CreasePattern::empty()).is_valid());
    }

    #[test]
    fn detects_duplicate_and_non_finite_vertices() {
        let first = vertex(2.0, -0.0);
        let duplicate = vertex(2.0, 0.0);
        let invalid = vertex(f64::NAN, 1.0);
        let report = validate_crease_pattern(&CreasePattern {
            vertices: vec![first.clone(), duplicate.clone(), invalid.clone()],
            edges: vec![],
        });

        assert_eq!(report.issues.len(), 2);
        assert_eq!(
            report.issues[0],
            ValidationIssue::DuplicateVertex {
                first: first.id,
                duplicate: duplicate.id,
                position: duplicate.position,
            }
        );
        assert!(matches!(
            report.issues[1],
            ValidationIssue::NonFiniteVertex { vertex, .. } if vertex == invalid.id
        ));
    }

    #[test]
    fn detects_missing_endpoints_and_zero_length_edges() {
        let start = vertex(1.0, 1.0);
        let same_position = vertex(1.0, 1.0);
        let missing = VertexId::new();
        let missing_edge = Edge {
            id: EdgeId::new(),
            start: start.id,
            end: missing,
            kind: EdgeKind::Mountain,
        };
        let zero_edge = edge(&start, &same_position);
        let report = validate_crease_pattern(&CreasePattern {
            vertices: vec![start, same_position],
            edges: vec![missing_edge.clone(), zero_edge.clone()],
        });

        assert!(report.issues.contains(&ValidationIssue::MissingEndpoint {
            edge: missing_edge.id,
            endpoint: EdgeEndpoint::End,
            vertex: missing,
        }));
        assert!(
            report
                .issues
                .contains(&ValidationIssue::ZeroLengthEdge { edge: zero_edge.id })
        );
    }

    #[test]
    fn detects_an_unsplit_crossing() {
        let a = vertex(0.0, 0.0);
        let b = vertex(2.0, 2.0);
        let c = vertex(0.0, 2.0);
        let d = vertex(2.0, 0.0);
        let first = edge(&a, &b);
        let second = edge(&c, &d);
        let report = validate_crease_pattern(&CreasePattern {
            vertices: vec![a, b, c, d],
            edges: vec![first.clone(), second.clone()],
        });

        assert_eq!(
            report.issues,
            vec![ValidationIssue::UnsplitIntersection {
                first_edge: first.id,
                second_edge: second.id,
                intersection: SegmentIntersection::Point(Point2::new(1.0, 1.0)),
            }]
        );
    }

    #[test]
    fn detects_a_t_junction_and_collinear_overlap() {
        let a = vertex(0.0, 0.0);
        let b = vertex(4.0, 0.0);
        let c = vertex(2.0, 0.0);
        let d = vertex(2.0, 2.0);
        let e = vertex(3.0, 0.0);
        let f = vertex(5.0, 0.0);
        let horizontal = edge(&a, &b);
        let branch = edge(&c, &d);
        let overlap = edge(&e, &f);
        let report = validate_crease_pattern(&CreasePattern {
            vertices: vec![a, b, c, d, e, f],
            edges: vec![horizontal.clone(), branch.clone(), overlap.clone()],
        });

        assert!(
            report
                .issues
                .contains(&ValidationIssue::UnsplitIntersection {
                    first_edge: horizontal.id,
                    second_edge: branch.id,
                    intersection: SegmentIntersection::Point(Point2::new(2.0, 0.0)),
                })
        );
        assert!(
            report
                .issues
                .contains(&ValidationIssue::UnsplitIntersection {
                    first_edge: horizontal.id,
                    second_edge: overlap.id,
                    intersection: SegmentIntersection::CollinearOverlap,
                })
        );
    }

    #[test]
    fn allows_edges_split_at_a_shared_vertex() {
        let center = vertex(1.0, 1.0);
        let left = vertex(0.0, 1.0);
        let right = vertex(2.0, 1.0);
        let top = vertex(1.0, 2.0);
        let bottom = vertex(1.0, 0.0);
        let pattern = CreasePattern {
            vertices: vec![
                center.clone(),
                left.clone(),
                right.clone(),
                top.clone(),
                bottom.clone(),
            ],
            edges: vec![
                edge(&left, &center),
                edge(&center, &right),
                edge(&top, &center),
                edge(&center, &bottom),
            ],
        };

        assert!(validate_crease_pattern(&pattern).is_valid());
    }

    #[test]
    fn distinct_vertex_ids_at_the_same_endpoint_are_not_considered_split() {
        let a = vertex(0.0, 0.0);
        let first_end = vertex(1.0, 0.0);
        let second_start = vertex(1.0, 0.0);
        let d = vertex(2.0, 0.0);
        let first = edge(&a, &first_end);
        let second = edge(&second_start, &d);
        let report = validate_crease_pattern(&CreasePattern {
            vertices: vec![a, first_end, second_start, d],
            edges: vec![first.clone(), second.clone()],
        });

        assert!(
            report
                .issues
                .iter()
                .any(|issue| matches!(issue, ValidationIssue::DuplicateVertex { .. }))
        );
        assert!(
            report
                .issues
                .contains(&ValidationIssue::UnsplitIntersection {
                    first_edge: first.id,
                    second_edge: second.id,
                    intersection: SegmentIntersection::Point(Point2::new(1.0, 0.0)),
                })
        );
    }
}
