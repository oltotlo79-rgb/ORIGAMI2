use std::collections::HashMap;

use num_bigint::{BigInt, BigUint, Sign};
use ori_domain::{CreasePattern, EdgeId, EdgeKind, Paper, Point2, VertexId};

use crate::{GeometryError, Orientation, SegmentIntersection, ensure_finite, segment_intersection};

// The smallest binary64 value is 2^-1074, so every coordinate product can be
// represented as one integer multiple of this shared exponent.
const EXACT_PRODUCT_EXPONENT: i32 = -2148;

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

/// Identifies one directed edge of the closed paper boundary.
///
/// `index` is the index of `start` in [`Paper::boundary_vertices`]. The end
/// vertex is the next entry, wrapping back to index zero for the closing edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoundaryEdgeRef {
    pub index: usize,
    pub start: VertexId,
    pub end: VertexId,
}

/// A structural or geometric defect found in a paper definition.
#[derive(Debug, Clone, PartialEq)]
pub enum PaperValidationIssue {
    /// Paper thickness is `NaN` or positive/negative infinity.
    NonFiniteThickness { thickness_mm: f64 },
    /// A finite paper thickness is negative. Zero represents an ideal sheet
    /// without physical thickness and is a supported simulation setting.
    NegativeThickness { thickness_mm: f64 },
    /// A closed polygon needs at least three boundary vertex references.
    TooFewBoundaryVertices { count: usize },
    /// The same vertex ID occurs more than once in the boundary order.
    DuplicateBoundaryVertex {
        vertex: VertexId,
        first_index: usize,
        duplicate_index: usize,
    },
    /// A boundary entry references a vertex absent from the crease pattern.
    MissingBoundaryVertex {
        boundary_index: usize,
        vertex: VertexId,
    },
    /// A referenced boundary vertex has a `NaN` or infinite coordinate.
    NonFiniteBoundaryVertex {
        boundary_index: usize,
        vertex: VertexId,
        position: Point2,
    },
    /// No `Boundary` edge record matches an edge required by the ordered
    /// paper boundary. An edge of another kind does not satisfy this rule.
    MissingBoundaryEdge { boundary_edge: BoundaryEdgeRef },
    /// More `Boundary` edge records match an undirected paper-boundary pair
    /// than the boundary's multiplicity requires.
    DuplicateBoundaryEdge {
        boundary_edge: BoundaryEdgeRef,
        first_edge: EdgeId,
        duplicate_edge: EdgeId,
    },
    /// A `Boundary` edge record does not occur in the paper boundary.
    UnexpectedBoundaryEdge {
        edge: EdgeId,
        start: VertexId,
        end: VertexId,
    },
    /// Consecutive boundary entries, including the closing pair, have the same
    /// position and therefore do not form an edge.
    ZeroLengthBoundaryEdge { edge: BoundaryEdgeRef },
    /// Two boundary edges meet anywhere other than the shared endpoint of two
    /// adjacent edges, or adjacent edges overlap along a positive length.
    SelfIntersection {
        first_edge: BoundaryEdgeRef,
        second_edge: BoundaryEdgeRef,
        intersection: SegmentIntersection,
    },
    /// Finite coordinates overflowed while classifying two boundary edges.
    IntersectionCalculationFailed {
        first_edge: BoundaryEdgeRef,
        second_edge: BoundaryEdgeRef,
        error: GeometryError,
    },
    /// The closed boundary has exactly zero signed area.
    ZeroArea { boundary_vertices: Vec<VertexId> },
    /// Finite coordinates overflowed while calculating boundary area.
    AreaCalculationFailed {
        boundary_vertices: Vec<VertexId>,
        error: GeometryError,
    },
}

/// Complete validation result for a paper definition.
///
/// Validation reports every independent issue it can classify, allowing the
/// editor to highlight all affected boundary entries and edges in one pass.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PaperValidation {
    pub issues: Vec<PaperValidationIssue>,
}

impl PaperValidation {
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.issues.is_empty()
    }

    #[must_use]
    pub fn into_issues(self) -> Vec<PaperValidationIssue> {
        self.issues
    }
}

/// Validates paper thickness and the closed polygon described by the paper's
/// ordered boundary vertex IDs.
///
/// The undirected multiset of consecutive boundary pairs must exactly match
/// the pattern's `Boundary` edge records; other edge kinds never satisfy a
/// required boundary pair.
///
/// Boundary coordinates are resolved from `pattern`. Both clockwise and
/// counter-clockwise simple polygons are accepted, including concave ones.
#[must_use]
pub fn validate_paper(paper: &Paper, pattern: &CreasePattern) -> PaperValidation {
    let mut issues = Vec::new();

    if !paper.thickness_mm.is_finite() {
        issues.push(PaperValidationIssue::NonFiniteThickness {
            thickness_mm: paper.thickness_mm,
        });
    } else if paper.thickness_mm < 0.0 {
        issues.push(PaperValidationIssue::NegativeThickness {
            thickness_mm: paper.thickness_mm,
        });
    }

    let boundary = &paper.boundary_vertices;
    if boundary.len() < 3 {
        issues.push(PaperValidationIssue::TooFewBoundaryVertices {
            count: boundary.len(),
        });
    }

    let mut first_boundary_indices = HashMap::with_capacity(boundary.len());
    for (index, vertex) in boundary.iter().copied().enumerate() {
        if let Some(first_index) = first_boundary_indices.get(&vertex).copied() {
            issues.push(PaperValidationIssue::DuplicateBoundaryVertex {
                vertex,
                first_index,
                duplicate_index: index,
            });
        } else {
            first_boundary_indices.insert(vertex, index);
        }
    }

    validate_boundary_edge_topology(boundary, pattern, &mut issues);

    let mut pattern_vertices = HashMap::with_capacity(pattern.vertices.len());
    for vertex in &pattern.vertices {
        // Match crease-pattern validation by resolving duplicate records with
        // the first occurrence, keeping malformed external data deterministic.
        pattern_vertices.entry(vertex.id).or_insert(vertex.position);
    }

    let mut resolved_vertices = Vec::with_capacity(boundary.len());
    let mut all_vertices_resolved = true;
    for (boundary_index, vertex) in boundary.iter().copied().enumerate() {
        let Some(position) = pattern_vertices.get(&vertex).copied() else {
            issues.push(PaperValidationIssue::MissingBoundaryVertex {
                boundary_index,
                vertex,
            });
            all_vertices_resolved = false;
            resolved_vertices.push(None);
            continue;
        };
        if !is_finite(position) {
            issues.push(PaperValidationIssue::NonFiniteBoundaryVertex {
                boundary_index,
                vertex,
                position,
            });
            all_vertices_resolved = false;
            resolved_vertices.push(None);
            continue;
        }

        resolved_vertices.push(Some(position));
    }

    let mut resolved_edges = Vec::with_capacity(boundary.len());
    if !boundary.is_empty() {
        for edge_index in 0..boundary.len() {
            let end_index = (edge_index + 1) % boundary.len();
            let edge = BoundaryEdgeRef {
                index: edge_index,
                start: boundary[edge_index],
                end: boundary[end_index],
            };
            let (Some(start), Some(end)) =
                (resolved_vertices[edge_index], resolved_vertices[end_index])
            else {
                continue;
            };

            if start == end {
                issues.push(PaperValidationIssue::ZeroLengthBoundaryEdge { edge });
                continue;
            }

            resolved_edges.push(ResolvedBoundaryEdge {
                edge,
                start,
                end,
                bounds: Bounds::from_points(start, end),
            });
        }
    }

    validate_boundary_intersections(&resolved_edges, boundary.len(), &mut issues);

    if boundary.len() >= 3 && all_vertices_resolved {
        let positions: Vec<_> = resolved_vertices
            .iter()
            .map(|position| position.expect("all boundary vertices were resolved"))
            .collect();
        match exact_polygon_orientation(&positions) {
            Ok(Orientation::Collinear) => issues.push(PaperValidationIssue::ZeroArea {
                boundary_vertices: boundary.clone(),
            }),
            Ok(Orientation::Clockwise | Orientation::CounterClockwise) => {
                if let Err(error) = polygon_signed_double_area(&positions) {
                    issues.push(PaperValidationIssue::AreaCalculationFailed {
                        boundary_vertices: boundary.clone(),
                        error,
                    });
                }
            }
            Err(error) => issues.push(PaperValidationIssue::AreaCalculationFailed {
                boundary_vertices: boundary.clone(),
                error,
            }),
        }
    }

    PaperValidation { issues }
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

#[derive(Debug)]
struct BoundaryEdgeGroup {
    expected: Vec<BoundaryEdgeRef>,
    actual: Vec<EdgeId>,
}

fn validate_boundary_edge_topology(
    boundary: &[VertexId],
    pattern: &CreasePattern,
    issues: &mut Vec<PaperValidationIssue>,
) {
    // Both directions point to one group, allowing undirected lookup without
    // requiring an ordering operation on opaque entity IDs. Group and record
    // vectors retain source order for deterministic multiset diagnostics.
    let mut group_by_direction: HashMap<(VertexId, VertexId), usize> =
        HashMap::with_capacity(boundary.len().saturating_mul(2));
    let mut groups: Vec<BoundaryEdgeGroup> = Vec::with_capacity(boundary.len());
    if !boundary.is_empty() {
        for index in 0..boundary.len() {
            let boundary_edge = BoundaryEdgeRef {
                index,
                start: boundary[index],
                end: boundary[(index + 1) % boundary.len()],
            };
            if let Some(group_index) = group_by_direction
                .get(&(boundary_edge.start, boundary_edge.end))
                .copied()
            {
                groups[group_index].expected.push(boundary_edge);
                continue;
            }

            let group_index = groups.len();
            groups.push(BoundaryEdgeGroup {
                expected: vec![boundary_edge],
                actual: Vec::new(),
            });
            group_by_direction.insert((boundary_edge.start, boundary_edge.end), group_index);
            group_by_direction.insert((boundary_edge.end, boundary_edge.start), group_index);
        }
    }

    let mut unexpected = Vec::new();
    for edge in pattern
        .edges
        .iter()
        .filter(|edge| edge.kind == EdgeKind::Boundary)
    {
        if let Some(group_index) = group_by_direction.get(&(edge.start, edge.end)).copied() {
            groups[group_index].actual.push(edge.id);
        } else {
            unexpected.push(PaperValidationIssue::UnexpectedBoundaryEdge {
                edge: edge.id,
                start: edge.start,
                end: edge.end,
            });
        }
    }

    for group in groups {
        if group.actual.len() < group.expected.len() {
            issues.extend(
                group
                    .expected
                    .iter()
                    .skip(group.actual.len())
                    .copied()
                    .map(|boundary_edge| PaperValidationIssue::MissingBoundaryEdge {
                        boundary_edge,
                    }),
            );
        } else if group.actual.len() > group.expected.len() {
            let first_edge = group.actual[0];
            let boundary_edge = group.expected[0];
            issues.extend(group.actual.iter().skip(group.expected.len()).copied().map(
                |duplicate_edge| PaperValidationIssue::DuplicateBoundaryEdge {
                    boundary_edge,
                    first_edge,
                    duplicate_edge,
                },
            ));
        }
    }
    issues.extend(unexpected);
}

#[derive(Debug, Clone, Copy)]
struct ResolvedBoundaryEdge {
    edge: BoundaryEdgeRef,
    start: Point2,
    end: Point2,
    bounds: Bounds,
}

fn validate_boundary_intersections(
    edges: &[ResolvedBoundaryEdge],
    boundary_length: usize,
    issues: &mut Vec<PaperValidationIssue>,
) {
    let mut by_min_x: Vec<_> = (0..edges.len()).collect();
    by_min_x.sort_unstable_by(|left, right| {
        edges[*left]
            .bounds
            .min_x
            .total_cmp(&edges[*right].bounds.min_x)
            .then_with(|| edges[*left].edge.index.cmp(&edges[*right].edge.index))
            .then_with(|| left.cmp(right))
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

            let (first, second) = if left.edge.index < right.edge.index {
                (left, right)
            } else {
                (right, left)
            };
            let adjacent =
                boundary_edges_are_adjacent(first.edge.index, second.edge.index, boundary_length);
            match segment_intersection(first.start, first.end, second.start, second.end) {
                Ok(SegmentIntersection::None) => {}
                Ok(SegmentIntersection::Point(_)) if adjacent => {}
                Ok(intersection) => found.push((
                    first.edge.index,
                    second.edge.index,
                    PaperValidationIssue::SelfIntersection {
                        first_edge: first.edge,
                        second_edge: second.edge,
                        intersection,
                    },
                )),
                Err(error) => found.push((
                    first.edge.index,
                    second.edge.index,
                    PaperValidationIssue::IntersectionCalculationFailed {
                        first_edge: first.edge,
                        second_edge: second.edge,
                        error,
                    },
                )),
            }
        }
    }

    found.sort_unstable_by_key(|(first, second, _)| (*first, *second));
    issues.extend(found.into_iter().map(|(_, _, issue)| issue));
}

fn boundary_edges_are_adjacent(first: usize, second: usize, boundary_length: usize) -> bool {
    boundary_length > 1
        && (first.abs_diff(second) == 1
            || (first == 0 && second == boundary_length - 1)
            || (second == 0 && first == boundary_length - 1))
}

/// Computes twice the signed area of a polygon boundary.
///
/// Every finite binary64 coordinate is expanded exactly and the shoelace sum
/// is accumulated with arbitrary-precision integers. The exact result is
/// rounded to binary64 only once, making its sign and returned bits independent
/// of the starting vertex and summation order. A positive result denotes
/// counter-clockwise order and a negative result denotes clockwise order.
/// Values too small to represent round to signed zero; values too large to
/// represent return [`GeometryError::ArithmeticOverflow`].
pub fn polygon_signed_double_area(points: &[Point2]) -> Result<f64, GeometryError> {
    for point in points {
        ensure_finite("polygon point", *point)?;
    }
    scaled_bigint_to_f64(exact_polygon_double_area(points), EXACT_PRODUCT_EXPONENT)
}

/// Classifies the exact sign of an ordered polygon boundary.
///
/// Unlike [`polygon_signed_double_area`], this function never rounds a
/// non-zero area to positive or negative zero. Callers making topological
/// decisions must use this sign and treat the numeric area as a measurement
/// only. A positive exact shoelace sum is counter-clockwise; a negative sum is
/// clockwise; and an exactly zero sum is collinear/zero-area.
pub fn exact_polygon_orientation(points: &[Point2]) -> Result<Orientation, GeometryError> {
    for point in points {
        ensure_finite("polygon point", *point)?;
    }
    Ok(match exact_polygon_double_area(points).sign() {
        Sign::Minus => Orientation::Clockwise,
        Sign::NoSign => Orientation::Collinear,
        Sign::Plus => Orientation::CounterClockwise,
    })
}

pub(super) fn exact_triangle_orientation(a: Point2, b: Point2, c: Point2) -> Orientation {
    debug_assert!(a.x.is_finite() && a.y.is_finite());
    debug_assert!(b.x.is_finite() && b.y.is_finite());
    debug_assert!(c.x.is_finite() && c.y.is_finite());
    match exact_polygon_double_area(&[a, b, c]).sign() {
        Sign::Minus => Orientation::Clockwise,
        Sign::NoSign => Orientation::Collinear,
        Sign::Plus => Orientation::CounterClockwise,
    }
}

/// Returns the correctly rounded binary64 intersection of two finite
/// binary64 segments that cross strictly inside both segments.
///
/// Every coordinate is first represented as an integer multiple of 2^-1074.
/// The determinant ratio and both resulting coordinates therefore remain
/// exact until one final round-to-nearest, ties-to-even conversion. Exact
/// parameter bounds are checked defensively, and an interior point that rounds
/// onto an input endpoint is rejected because binary64 cannot represent a safe
/// split location for it.
pub(super) fn exact_proper_segment_intersection_point(
    a: Point2,
    b: Point2,
    c: Point2,
    d: Point2,
) -> Result<Point2, GeometryError> {
    debug_assert!(a.x.is_finite() && a.y.is_finite());
    debug_assert!(b.x.is_finite() && b.y.is_finite());
    debug_assert!(c.x.is_finite() && c.y.is_finite());
    debug_assert!(d.x.is_finite() && d.y.is_finite());

    let ax = exact_coordinate_units(a.x);
    let ay = exact_coordinate_units(a.y);
    let bx = exact_coordinate_units(b.x);
    let by = exact_coordinate_units(b.y);
    let cx = exact_coordinate_units(c.x);
    let cy = exact_coordinate_units(c.y);
    let dx = exact_coordinate_units(d.x);
    let dy = exact_coordinate_units(d.y);

    let rx = &bx - &ax;
    let ry = &by - &ay;
    let sx = &dx - &cx;
    let sy = &dy - &cy;
    let cax = &cx - &ax;
    let cay = &cy - &ay;
    let mut denominator = &rx * &sy - &ry * &sx;
    let mut t_numerator = &cax * &sy - &cay * &sx;
    let mut u_numerator = &cax * &ry - &cay * &rx;
    if denominator.sign() == Sign::Minus {
        denominator = -denominator;
        t_numerator = -t_numerator;
        u_numerator = -u_numerator;
    }
    if denominator.sign() == Sign::NoSign
        || t_numerator.sign() != Sign::Plus
        || u_numerator.sign() != Sign::Plus
        || t_numerator >= denominator
        || u_numerator >= denominator
    {
        return Err(GeometryError::ArithmeticOverflow);
    }
    let x_numerator = &ax * &denominator + &rx * &t_numerator;
    let y_numerator = &ay * &denominator + &ry * &t_numerator;

    let point = Point2::new(
        scaled_ratio_to_f64(x_numerator, denominator.clone(), -1074)?,
        scaled_ratio_to_f64(y_numerator, denominator, -1074)?,
    );
    if [a, b, c, d].contains(&point) {
        Err(GeometryError::ArithmeticOverflow)
    } else {
        Ok(point)
    }
}

fn exact_coordinate_units(value: f64) -> BigInt {
    let (significand, exponent) = decompose_f64(value);
    let shift = usize::try_from(exponent + 1074)
        .expect("a finite binary64 exponent is at least the subnormal exponent");
    BigInt::from(significand) << shift
}

fn exact_polygon_double_area(points: &[Point2]) -> BigInt {
    let mut exact_area = BigInt::from(0_u8);
    for index in 0..points.len() {
        let current = points[index];
        let next = points[(index + 1) % points.len()];

        let (current_x, current_x_exponent) = decompose_f64(current.x);
        let (current_y, current_y_exponent) = decompose_f64(current.y);
        let (next_x, next_x_exponent) = decompose_f64(next.x);
        let (next_y, next_y_exponent) = decompose_f64(next.y);
        add_exact_product(
            &mut exact_area,
            current_x,
            current_x_exponent,
            next_y,
            next_y_exponent,
            EXACT_PRODUCT_EXPONENT,
        );
        add_exact_product(
            &mut exact_area,
            -current_y,
            current_y_exponent,
            next_x,
            next_x_exponent,
            EXACT_PRODUCT_EXPONENT,
        );
    }

    exact_area
}

fn decompose_f64(value: f64) -> (i64, i32) {
    debug_assert!(value.is_finite());
    let bits = value.to_bits();
    let exponent_bits = ((bits >> 52) & 0x7ff) as i32;
    let fraction = bits & ((1_u64 << 52) - 1);
    let (significand, exponent) = if exponent_bits == 0 {
        (fraction, -1074)
    } else {
        ((1_u64 << 52) | fraction, exponent_bits - 1075)
    };
    let significand = significand as i64;
    if bits >> 63 == 0 {
        (significand, exponent)
    } else {
        (-significand, exponent)
    }
}

fn add_exact_product(
    accumulator: &mut BigInt,
    left: i64,
    left_exponent: i32,
    right: i64,
    right_exponent: i32,
    common_exponent: i32,
) {
    let product = i128::from(left) * i128::from(right);
    if product == 0 {
        return;
    }
    let shift = usize::try_from(left_exponent + right_exponent - common_exponent)
        .expect("the common product exponent is the minimum binary64 product exponent");
    *accumulator += BigInt::from(product) << shift;
}

fn scaled_bigint_to_f64(value: BigInt, binary_exponent: i32) -> Result<f64, GeometryError> {
    let sign = value.sign();
    if sign == Sign::NoSign {
        return Ok(0.0);
    }
    let magnitude = value.magnitude();
    let leading_exponent = i64::try_from(magnitude.bits() - 1)
        .expect("a binary64 polygon accumulator bit count fits i64")
        + i64::from(binary_exponent);
    if leading_exponent > 1023 {
        return Err(GeometryError::ArithmeticOverflow);
    }

    let mut result = if leading_exponent < -1022 {
        let shift = usize::try_from(-1074_i32 - binary_exponent)
            .expect("subnormal rounding shift is non-negative");
        let units = round_shift_right(magnitude, shift);
        let bits = biguint_to_u64(&units);
        debug_assert!(bits <= 1_u64 << 52);
        f64::from_bits(bits)
    } else {
        let mut shift = usize::try_from(magnitude.bits().saturating_sub(53))
            .expect("a binary64 polygon accumulator shift fits usize");
        let mut significand = round_shift_right(magnitude, shift);
        if significand.bits() > 53 {
            significand >>= 1_usize;
            shift += 1;
        }
        let significand = biguint_to_u64(&significand);
        let scale_exponent = i32::try_from(shift)
            .ok()
            .and_then(|shift| shift.checked_add(binary_exponent))
            .ok_or(GeometryError::ArithmeticOverflow)?;
        let unbiased_exponent = scale_exponent
            .checked_add(52)
            .ok_or(GeometryError::ArithmeticOverflow)?;
        if unbiased_exponent > 1023 {
            return Err(GeometryError::ArithmeticOverflow);
        }
        debug_assert!((-1022..=1023).contains(&unbiased_exponent));
        debug_assert!((1_u64 << 52..1_u64 << 53).contains(&significand));
        let exponent_bits = u64::try_from(unbiased_exponent + 1023)
            .expect("a normal binary64 exponent is non-negative");
        let fraction_bits = significand - (1_u64 << 52);
        f64::from_bits((exponent_bits << 52) | fraction_bits)
    };
    if !result.is_finite() {
        return Err(GeometryError::ArithmeticOverflow);
    }
    if sign == Sign::Minus {
        result = -result;
    }
    Ok(result)
}

fn scaled_ratio_to_f64(
    numerator: BigInt,
    denominator: BigInt,
    binary_exponent: i32,
) -> Result<f64, GeometryError> {
    if denominator.sign() == Sign::NoSign {
        return Err(GeometryError::ArithmeticOverflow);
    }
    if numerator.sign() == Sign::NoSign {
        return Ok(0.0);
    }

    let negative = numerator.sign() != denominator.sign();
    let numerator = numerator.magnitude();
    let denominator = denominator.magnitude();
    let ratio_exponent = floor_log2_ratio(numerator, denominator);
    let mut leading_exponent = ratio_exponent + i64::from(binary_exponent);
    if leading_exponent > 1023 {
        return Err(GeometryError::ArithmeticOverflow);
    }

    let mut bits = if leading_exponent < -1022 {
        let shift = i64::from(binary_exponent) + 1074;
        let units = round_scaled_ratio(numerator, denominator, shift);
        if units.bits() > 53 {
            return Err(GeometryError::ArithmeticOverflow);
        }
        let units = biguint_to_u64(&units);
        if units > 1_u64 << 52 {
            return Err(GeometryError::ArithmeticOverflow);
        }
        units
    } else {
        let shift = i64::from(binary_exponent) - leading_exponent + 52;
        let mut significand = round_scaled_ratio(numerator, denominator, shift);
        if significand.bits() > 53 {
            if significand != BigUint::from(1_u8) << 53_usize {
                return Err(GeometryError::ArithmeticOverflow);
            }
            significand >>= 1_usize;
            leading_exponent += 1;
        }
        if leading_exponent > 1023
            || significand.bits() != 53
            || significand < BigUint::from(1_u8) << 52_usize
        {
            return Err(GeometryError::ArithmeticOverflow);
        }
        let significand = biguint_to_u64(&significand);
        let exponent_bits = u64::try_from(leading_exponent + 1023)
            .map_err(|_| GeometryError::ArithmeticOverflow)?;
        (exponent_bits << 52) | (significand - (1_u64 << 52))
    };

    if negative {
        bits |= 1_u64 << 63;
    }
    let result = f64::from_bits(bits);
    if result.is_finite() {
        Ok(result)
    } else {
        Err(GeometryError::ArithmeticOverflow)
    }
}

fn floor_log2_ratio(numerator: &BigUint, denominator: &BigUint) -> i64 {
    debug_assert!(numerator != &BigUint::from(0_u8));
    debug_assert!(denominator != &BigUint::from(0_u8));
    let candidate = i64::try_from(numerator.bits()).expect("BigUint bit count fits i64")
        - i64::try_from(denominator.bits()).expect("BigUint bit count fits i64");
    let reaches_candidate = if candidate >= 0 {
        numerator >= &(denominator << usize::try_from(candidate).expect("non-negative shift"))
    } else {
        &(numerator << usize::try_from(-candidate).expect("negated shift is non-negative"))
            >= denominator
    };
    if reaches_candidate {
        candidate
    } else {
        candidate - 1
    }
}

fn round_scaled_ratio(numerator: &BigUint, denominator: &BigUint, binary_shift: i64) -> BigUint {
    let (scaled_numerator, scaled_denominator) = if binary_shift >= 0 {
        (
            numerator << usize::try_from(binary_shift).expect("non-negative shift"),
            denominator.clone(),
        )
    } else {
        (
            numerator.clone(),
            denominator << usize::try_from(-binary_shift).expect("negated shift is non-negative"),
        )
    };
    let mut quotient = &scaled_numerator / &scaled_denominator;
    let remainder = scaled_numerator - &quotient * &scaled_denominator;
    let twice_remainder = &remainder << 1_usize;
    if twice_remainder > scaled_denominator
        || (twice_remainder == scaled_denominator && quotient.bit(0))
    {
        quotient += 1_u8;
    }
    quotient
}

fn round_shift_right(value: &BigUint, shift: usize) -> BigUint {
    if shift == 0 {
        return value.clone();
    }
    let mut rounded = value >> shift;
    let remainder = value - (&rounded << shift);
    let halfway = BigUint::from(1_u8) << (shift - 1);
    if remainder > halfway || (remainder == halfway && rounded.bit(0)) {
        rounded += 1_u8;
    }
    rounded
}

fn biguint_to_u64(value: &BigUint) -> u64 {
    let digits = value.to_u64_digits();
    debug_assert!(digits.len() <= 1);
    digits.first().copied().unwrap_or(0)
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
    use ori_domain::{Edge, EdgeKind, Paper, Vertex};

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

    fn paper(vertices: &[Vertex]) -> Paper {
        Paper {
            boundary_vertices: vertices.iter().map(|vertex| vertex.id).collect(),
            ..Paper::default()
        }
    }

    fn pattern(vertices: &[Vertex]) -> CreasePattern {
        let edges = if vertices.is_empty() {
            Vec::new()
        } else {
            (0..vertices.len())
                .map(|index| Edge {
                    id: EdgeId::new(),
                    start: vertices[index].id,
                    end: vertices[(index + 1) % vertices.len()].id,
                    kind: EdgeKind::Boundary,
                })
                .collect()
        };
        CreasePattern {
            vertices: vertices.to_vec(),
            edges,
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

    #[test]
    fn accepts_square_boundaries_in_both_orientations() {
        let vertices = vec![
            vertex(0.0, 0.0),
            vertex(2.0, 0.0),
            vertex(2.0, 2.0),
            vertex(0.0, 2.0),
        ];
        let pattern = pattern(&vertices);
        let counter_clockwise = paper(&vertices);
        let mut clockwise = counter_clockwise.clone();
        clockwise.boundary_vertices.reverse();

        assert!(validate_paper(&counter_clockwise, &pattern).is_valid());
        assert!(validate_paper(&clockwise, &pattern).is_valid());
    }

    #[test]
    fn accepts_a_small_square_far_from_the_origin() {
        let offset = 1_000_000_000.0;
        let vertices = vec![
            vertex(offset, offset),
            vertex(offset + 1.0, offset),
            vertex(offset + 1.0, offset + 1.0),
            vertex(offset, offset + 1.0),
        ];

        assert!(validate_paper(&paper(&vertices), &pattern(&vertices)).is_valid());
    }

    #[test]
    fn exact_polygon_area_is_bit_stable_across_cycle_start_and_reversal() {
        let mut points = vec![
            Point2::new(2.5, -0.2),
            Point2::new(0.0, 3.8),
            Point2::new(-2.5, 0.3),
            Point2::new(0.3, -3.2),
        ];
        let expected = polygon_signed_double_area(&points).expect("finite polygon area");
        for _ in 0..points.len() {
            assert_eq!(
                polygon_signed_double_area(&points)
                    .expect("rotated finite polygon area")
                    .to_bits(),
                expected.to_bits()
            );
            points.rotate_left(1);
        }

        points.reverse();
        assert_eq!(
            polygon_signed_double_area(&points)
                .expect("reversed finite polygon area")
                .to_bits(),
            (-expected).to_bits()
        );

        let mut near_degenerate = vec![
            Point2::new(-9.723_461_371_658_034e-63, -9.723_461_371_658_034e-63),
            Point2::new(-4.687_132_327_120_085e-63, -5.890_285_417_377_298e-63),
            Point2::new(1.348_447_477_636_382_8e-62, 7.940_218_265_707_989e-63),
        ];
        let expected = polygon_signed_double_area(&near_degenerate)
            .expect("near-degenerate finite polygon area");
        assert_ne!(expected, 0.0);
        for _ in 0..near_degenerate.len() {
            assert_eq!(
                polygon_signed_double_area(&near_degenerate)
                    .expect("rotated near-degenerate polygon area")
                    .to_bits(),
                expected.to_bits()
            );
            near_degenerate.rotate_left(1);
        }
    }

    #[test]
    fn exact_polygon_area_handles_subnormal_rounding_and_overflow() {
        let minimum_area_side = f64::from_bits(486_u64 << 52);
        assert_eq!(
            polygon_signed_double_area(&[
                Point2::new(0.0, 0.0),
                Point2::new(minimum_area_side, 0.0),
                Point2::new(0.0, minimum_area_side),
            ]),
            Ok(f64::from_bits(1))
        );

        let underflow_side = f64::from_bits(485_u64 << 52);
        assert_eq!(
            polygon_signed_double_area(&[
                Point2::new(0.0, 0.0),
                Point2::new(underflow_side, 0.0),
                Point2::new(0.0, underflow_side),
            ]),
            Ok(0.0)
        );

        assert_eq!(
            polygon_signed_double_area(&[
                Point2::new(f64::MAX, 0.0),
                Point2::new(0.0, f64::MAX),
                Point2::new(-f64::MAX, 0.0),
            ]),
            Err(GeometryError::ArithmeticOverflow)
        );
    }

    #[test]
    fn exact_ratio_conversion_rounds_binary64_boundaries_to_even() {
        let power = |exponent: usize| BigInt::from(1_u8) << exponent;
        let ratio = |numerator: BigInt, denominator: BigInt, exponent| {
            scaled_ratio_to_f64(numerator, denominator, exponent)
                .expect("representable exact ratio")
        };

        assert_eq!(
            ratio(power(53) + 1_u8, power(53), 0).to_bits(),
            1.0_f64.to_bits()
        );
        assert_eq!(
            ratio(power(53) + 3_u8, power(53), 0).to_bits(),
            1.0_f64.to_bits() + 2
        );
        assert_eq!(
            ratio(BigInt::from(1_u8), BigInt::from(2_u8), -1074).to_bits(),
            0
        );
        assert_eq!(
            ratio(BigInt::from(-1_i8), BigInt::from(2_u8), -1074).to_bits(),
            1_u64 << 63
        );
        assert_eq!(
            ratio(BigInt::from(3_u8), BigInt::from(2_u8), -1074).to_bits(),
            2
        );
        assert_eq!(
            ratio(power(53) - 1_u8, BigInt::from(2_u8), -1074).to_bits(),
            0x0010_0000_0000_0000
        );
        assert_eq!(
            ratio(power(54) - 1_u8, power(53), 0).to_bits(),
            2.0_f64.to_bits()
        );
        assert_eq!(ratio(power(53) - 1_u8, BigInt::from(1_u8), 971), f64::MAX);
        assert_eq!(
            scaled_ratio_to_f64(power(54) - 1_u8, BigInt::from(1_u8), 970),
            Err(GeometryError::ArithmeticOverflow)
        );
    }

    #[test]
    fn exact_polygon_orientation_preserves_sign_below_binary64_area_range() {
        let side = f64::from_bits(485_u64 << 52);
        let mut points = vec![
            Point2::new(0.0, 0.0),
            Point2::new(side, 0.0),
            Point2::new(0.0, side),
        ];
        assert_eq!(polygon_signed_double_area(&points), Ok(0.0));
        assert_eq!(
            exact_polygon_orientation(&points),
            Ok(Orientation::CounterClockwise)
        );

        for _ in 0..points.len() {
            points.rotate_left(1);
            assert_eq!(
                exact_polygon_orientation(&points),
                Ok(Orientation::CounterClockwise)
            );
        }
        points.reverse();
        assert_eq!(
            polygon_signed_double_area(&points)
                .expect("finite underflowing area")
                .to_bits(),
            (-0.0_f64).to_bits()
        );
        assert_eq!(
            exact_polygon_orientation(&points),
            Ok(Orientation::Clockwise)
        );

        assert_eq!(
            exact_polygon_orientation(&[
                Point2::new(0.0, 0.0),
                Point2::new(side, 0.0),
                Point2::new(0.0, 0.0),
            ]),
            Ok(Orientation::Collinear)
        );
        assert!(matches!(
            exact_polygon_orientation(&[Point2::new(f64::NAN, 0.0)]),
            Err(GeometryError::NonFinitePoint {
                argument: "polygon point",
                ..
            })
        ));
    }

    #[test]
    fn accepts_a_simple_concave_boundary() {
        let vertices = vec![
            vertex(0.0, 0.0),
            vertex(3.0, 0.0),
            vertex(3.0, 3.0),
            vertex(1.5, 1.0),
            vertex(0.0, 3.0),
        ];

        assert!(validate_paper(&paper(&vertices), &pattern(&vertices)).is_valid());
    }

    #[test]
    fn reports_missing_duplicate_and_unexpected_boundary_edge_records() {
        let vertices = vec![
            vertex(0.0, 0.0),
            vertex(2.0, 0.0),
            vertex(2.0, 2.0),
            vertex(0.0, 2.0),
        ];
        let wrong_kind = edge(&vertices[0], &vertices[1]);
        let first_duplicate = Edge {
            id: EdgeId::new(),
            start: vertices[1].id,
            end: vertices[2].id,
            kind: EdgeKind::Boundary,
        };
        let second_duplicate = Edge {
            id: EdgeId::new(),
            start: vertices[2].id,
            end: vertices[1].id,
            kind: EdgeKind::Boundary,
        };
        let third_side = Edge {
            id: EdgeId::new(),
            start: vertices[2].id,
            end: vertices[3].id,
            kind: EdgeKind::Boundary,
        };
        let fourth_side = Edge {
            id: EdgeId::new(),
            start: vertices[3].id,
            end: vertices[0].id,
            kind: EdgeKind::Boundary,
        };
        let unexpected_chord = Edge {
            id: EdgeId::new(),
            start: vertices[0].id,
            end: vertices[2].id,
            kind: EdgeKind::Boundary,
        };
        let pattern = CreasePattern {
            vertices: vertices.clone(),
            edges: vec![
                wrong_kind,
                first_duplicate.clone(),
                second_duplicate.clone(),
                third_side,
                fourth_side,
                unexpected_chord.clone(),
            ],
        };

        let report = validate_paper(&paper(&vertices), &pattern);

        assert_eq!(
            report.issues,
            vec![
                PaperValidationIssue::MissingBoundaryEdge {
                    boundary_edge: BoundaryEdgeRef {
                        index: 0,
                        start: vertices[0].id,
                        end: vertices[1].id,
                    },
                },
                PaperValidationIssue::DuplicateBoundaryEdge {
                    boundary_edge: BoundaryEdgeRef {
                        index: 1,
                        start: vertices[1].id,
                        end: vertices[2].id,
                    },
                    first_edge: first_duplicate.id,
                    duplicate_edge: second_duplicate.id,
                },
                PaperValidationIssue::UnexpectedBoundaryEdge {
                    edge: unexpected_chord.id,
                    start: vertices[0].id,
                    end: vertices[2].id,
                },
            ]
        );
    }

    #[test]
    fn boundary_edge_topology_uses_literal_multiset_multiplicity() {
        let vertices = vec![vertex(0.0, 0.0), vertex(2.0, 0.0)];
        let report = validate_paper(&paper(&vertices), &pattern(&vertices));

        assert!(!report.issues.iter().any(|issue| matches!(
            issue,
            PaperValidationIssue::MissingBoundaryEdge { .. }
                | PaperValidationIssue::DuplicateBoundaryEdge { .. }
                | PaperValidationIssue::UnexpectedBoundaryEdge { .. }
        )));
    }

    #[test]
    fn boundary_intersection_sweep_restores_boundary_index_order() {
        let ids: Vec<_> = (0..8).map(|_| VertexId::new()).collect();
        let resolved = [
            ResolvedBoundaryEdge {
                edge: BoundaryEdgeRef {
                    index: 0,
                    start: ids[0],
                    end: ids[1],
                },
                start: Point2::new(10.0, 0.0),
                end: Point2::new(12.0, 2.0),
                bounds: Bounds::from_points(Point2::new(10.0, 0.0), Point2::new(12.0, 2.0)),
            },
            ResolvedBoundaryEdge {
                edge: BoundaryEdgeRef {
                    index: 1,
                    start: ids[2],
                    end: ids[3],
                },
                start: Point2::new(0.0, 0.0),
                end: Point2::new(2.0, 2.0),
                bounds: Bounds::from_points(Point2::new(0.0, 0.0), Point2::new(2.0, 2.0)),
            },
            ResolvedBoundaryEdge {
                edge: BoundaryEdgeRef {
                    index: 3,
                    start: ids[4],
                    end: ids[5],
                },
                start: Point2::new(10.0, 2.0),
                end: Point2::new(12.0, 0.0),
                bounds: Bounds::from_points(Point2::new(10.0, 2.0), Point2::new(12.0, 0.0)),
            },
            ResolvedBoundaryEdge {
                edge: BoundaryEdgeRef {
                    index: 4,
                    start: ids[6],
                    end: ids[7],
                },
                start: Point2::new(0.0, 2.0),
                end: Point2::new(2.0, 0.0),
                bounds: Bounds::from_points(Point2::new(0.0, 2.0), Point2::new(2.0, 0.0)),
            },
        ];
        let mut issues = Vec::new();

        validate_boundary_intersections(&resolved, 8, &mut issues);

        assert!(matches!(
            issues.as_slice(),
            [
                PaperValidationIssue::SelfIntersection {
                    first_edge: BoundaryEdgeRef { index: 0, .. },
                    second_edge: BoundaryEdgeRef { index: 3, .. },
                    ..
                },
                PaperValidationIssue::SelfIntersection {
                    first_edge: BoundaryEdgeRef { index: 1, .. },
                    second_edge: BoundaryEdgeRef { index: 4, .. },
                    ..
                }
            ]
        ));
    }

    #[test]
    fn reports_bow_tie_intersection_and_zero_area() {
        let vertices = vec![
            vertex(0.0, 0.0),
            vertex(2.0, 2.0),
            vertex(0.0, 2.0),
            vertex(2.0, 0.0),
        ];
        let report = validate_paper(&paper(&vertices), &pattern(&vertices));

        assert!(report.issues.iter().any(|issue| matches!(
            issue,
            PaperValidationIssue::SelfIntersection {
                first_edge,
                second_edge,
                intersection: SegmentIntersection::Point(point),
            } if first_edge.index == 0
                && second_edge.index == 2
                && *point == Point2::new(1.0, 1.0)
        )));
        assert!(report.issues.iter().any(|issue| matches!(
            issue,
            PaperValidationIssue::ZeroArea { boundary_vertices }
                if boundary_vertices == &vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>()
        )));
    }

    #[test]
    fn reports_missing_and_duplicate_boundary_ids_with_indices() {
        let vertices = vec![vertex(0.0, 0.0), vertex(2.0, 0.0), vertex(0.0, 2.0)];
        let missing = VertexId::new();
        let paper = Paper {
            boundary_vertices: vec![vertices[0].id, vertices[1].id, missing, vertices[1].id],
            ..Paper::default()
        };
        let report = validate_paper(&paper, &pattern(&vertices));

        assert!(
            report
                .issues
                .contains(&PaperValidationIssue::DuplicateBoundaryVertex {
                    vertex: vertices[1].id,
                    first_index: 1,
                    duplicate_index: 3,
                })
        );
        assert!(
            report
                .issues
                .contains(&PaperValidationIssue::MissingBoundaryVertex {
                    boundary_index: 2,
                    vertex: missing,
                })
        );
    }

    #[test]
    fn reports_non_finite_and_negative_thickness_while_accepting_zero() {
        let vertices = vec![vertex(0.0, 0.0), vertex(1.0, 0.0), vertex(0.0, 1.0)];
        let pattern = pattern(&vertices);

        for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let mut paper = paper(&vertices);
            paper.thickness_mm = invalid;
            let report = validate_paper(&paper, &pattern);
            assert!(matches!(
                report.issues.as_slice(),
                [PaperValidationIssue::NonFiniteThickness { thickness_mm }]
                    if thickness_mm.to_bits() == invalid.to_bits()
            ));
        }

        for valid in [0.0, -0.0] {
            let mut paper = paper(&vertices);
            paper.thickness_mm = valid;
            assert!(validate_paper(&paper, &pattern).is_valid());
        }

        for invalid in [-0.1, -f64::MIN_POSITIVE] {
            let mut paper = paper(&vertices);
            paper.thickness_mm = invalid;
            let report = validate_paper(&paper, &pattern);
            assert_eq!(
                report.issues,
                vec![PaperValidationIssue::NegativeThickness {
                    thickness_mm: invalid,
                }]
            );
        }
    }

    #[test]
    fn reports_too_few_vertices_and_both_kinds_of_zero_length_edge() {
        let only = vertex(0.0, 0.0);
        let single = paper(std::slice::from_ref(&only));
        let report = validate_paper(&single, &pattern(std::slice::from_ref(&only)));
        assert!(
            report
                .issues
                .contains(&PaperValidationIssue::TooFewBoundaryVertices { count: 1 })
        );
        assert!(
            report
                .issues
                .contains(&PaperValidationIssue::ZeroLengthBoundaryEdge {
                    edge: BoundaryEdgeRef {
                        index: 0,
                        start: only.id,
                        end: only.id,
                    },
                })
        );

        let vertices = vec![
            vertex(0.0, 0.0),
            vertex(0.0, 0.0),
            vertex(2.0, 0.0),
            vertex(0.0, 2.0),
            vertex(0.0, 0.0),
        ];
        let report = validate_paper(&paper(&vertices), &pattern(&vertices));
        assert!(report.issues.iter().any(|issue| matches!(
            issue,
            PaperValidationIssue::ZeroLengthBoundaryEdge { edge } if edge.index == 0
        )));
        assert!(report.issues.iter().any(|issue| matches!(
            issue,
            PaperValidationIssue::ZeroLengthBoundaryEdge { edge } if edge.index == 4
        )));
    }

    #[test]
    fn reports_a_zero_area_collinear_boundary() {
        let vertices = vec![vertex(0.0, 0.0), vertex(1.0, 0.0), vertex(2.0, 0.0)];
        let report = validate_paper(&paper(&vertices), &pattern(&vertices));

        assert!(
            report
                .issues
                .iter()
                .any(|issue| matches!(issue, PaperValidationIssue::ZeroArea { .. }))
        );
    }

    #[test]
    fn accepts_exact_nonzero_boundary_when_measured_area_underflows() {
        let side = f64::from_bits(485_u64 << 52);
        let mut vertices = vec![vertex(0.0, 0.0), vertex(side, 0.0), vertex(0.0, side)];
        for expected in [Orientation::CounterClockwise, Orientation::Clockwise] {
            let points = vertices
                .iter()
                .map(|vertex| vertex.position)
                .collect::<Vec<_>>();
            assert_eq!(polygon_signed_double_area(&points), Ok(0.0));
            assert_eq!(exact_polygon_orientation(&points), Ok(expected));
            let report = validate_paper(&paper(&vertices), &pattern(&vertices));
            assert!(report.is_valid(), "unexpected issues: {:?}", report.issues);
            vertices.reverse();
        }
    }

    #[test]
    fn reports_non_finite_boundary_coordinates_at_their_boundary_index() {
        let vertices = vec![vertex(0.0, 0.0), vertex(f64::NAN, 1.0), vertex(0.0, 2.0)];
        let report = validate_paper(&paper(&vertices), &pattern(&vertices));

        assert!(matches!(
            report.issues.as_slice(),
            [PaperValidationIssue::NonFiniteBoundaryVertex {
                boundary_index: 1,
                vertex: affected,
                ..
            }] if *affected == vertices[1].id
        ));
    }
}
