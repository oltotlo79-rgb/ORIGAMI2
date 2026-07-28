use std::collections::BTreeMap;

use ori_domain::{CreasePattern, Edge, EdgeId, GeometricConstraintKindV1, Point2, VertexId};
use ori_numeric::deterministic_sin_cos_degrees_v1;

type CanonicalAssignment = BTreeMap<[u8; 16], (VertexId, Point2)>;

#[derive(Clone, Copy)]
enum CardinalRotation {
    Identity,
    QuarterTurn,
    HalfTurn,
    ThreeQuarterTurn,
}

impl CardinalRotation {
    fn from_angle_degrees(angle_degrees: f64) -> Option<Self> {
        if !angle_degrees.is_normal() {
            return None;
        }
        let (sin, cos) = deterministic_sin_cos_degrees_v1(angle_degrees).ok()?;
        match (sin.to_bits(), cos.to_bits()) {
            (sin_bits, cos_bits)
                if sin_bits == 0.0_f64.to_bits() && cos_bits == 1.0_f64.to_bits() =>
            {
                Some(Self::Identity)
            }
            (sin_bits, cos_bits)
                if sin_bits == 1.0_f64.to_bits() && cos_bits == 0.0_f64.to_bits() =>
            {
                Some(Self::QuarterTurn)
            }
            (sin_bits, cos_bits)
                if sin_bits == 0.0_f64.to_bits() && cos_bits == (-1.0_f64).to_bits() =>
            {
                Some(Self::HalfTurn)
            }
            (sin_bits, cos_bits)
                if sin_bits == (-1.0_f64).to_bits() && cos_bits == 0.0_f64.to_bits() =>
            {
                Some(Self::ThreeQuarterTurn)
            }
            _ => None,
        }
    }

    const fn apply(self, point: Point2) -> Point2 {
        match self {
            Self::Identity => point,
            Self::QuarterTurn => Point2::new(-point.y, point.x),
            Self::HalfTurn => Point2::new(-point.x, -point.y),
            Self::ThreeQuarterTurn => Point2::new(point.y, -point.x),
        }
    }

    const fn apply_inverse(self, point: Point2) -> Point2 {
        match self {
            Self::Identity => point,
            Self::QuarterTurn => Self::ThreeQuarterTurn.apply(point),
            Self::HalfTurn => Self::HalfTurn.apply(point),
            Self::ThreeQuarterTurn => Self::QuarterTurn.apply(point),
        }
    }
}

#[derive(Clone, Copy)]
struct RotationRoles {
    center: VertexId,
    source: VertexId,
    target: VertexId,
    rotation: CardinalRotation,
}

/// Builds one bounded candidate for the deletion shapes of the exact-cardinal
/// rotational-symmetry conflict.
///
/// One remaining rotation plus a fixed center/source or center/target radius is
/// satisfied by an exact cardinal orbit rooted at the origin. A two-rotation
/// deletion requires coincident role coordinates, which are not a valid crease
/// pattern assignment; that escape is certified separately by the private
/// residual-only algebraic overlay path. This helper is candidate generation
/// only: its sole caller re-evaluates the complete document with the frozen
/// production residuals before any witness escapes.
pub(super) fn construct_cardinal_rotation_pair_candidate_v1(
    pattern: &CreasePattern,
    first: &GeometricConstraintKindV1,
    second: &GeometricConstraintKindV1,
) -> Option<CreasePattern> {
    rotation_with_fixed_radius_candidate(pattern, first, second)
        .or_else(|| rotation_with_fixed_radius_candidate(pattern, second, first))
}

fn rotation_with_fixed_radius_candidate(
    pattern: &CreasePattern,
    rotation: &GeometricConstraintKindV1,
    fixed: &GeometricConstraintKindV1,
) -> Option<CreasePattern> {
    let GeometricConstraintKindV1::RotationalSymmetry {
        center_vertex,
        source_vertex,
        target_vertex,
        angle_degrees,
    } = *rotation
    else {
        return None;
    };
    let GeometricConstraintKindV1::FixedLength { edge, length_mm } = *fixed else {
        return None;
    };
    if !length_mm.is_finite() || length_mm <= 0.0 {
        return None;
    }
    let roles = RotationRoles {
        center: center_vertex,
        source: source_vertex,
        target: target_vertex,
        rotation: CardinalRotation::from_angle_degrees(angle_degrees)?,
    };
    let radius_edge = find_edge(pattern, edge)?;
    let radial = Point2::new(length_mm, 0.0);
    let (source, target) = if edge_has_endpoints(radius_edge, roles.center, roles.source) {
        (radial, roles.rotation.apply(radial))
    } else if edge_has_endpoints(radius_edge, roles.center, roles.target) {
        (roles.rotation.apply_inverse(radial), radial)
    } else {
        return None;
    };
    if !finite_point(source) || !finite_point(target) {
        return None;
    }

    let mut assignment = CanonicalAssignment::new();
    assign_point(&mut assignment, roles.center, Point2::new(0.0, 0.0))?;
    assign_point(&mut assignment, roles.source, source)?;
    assign_point(&mut assignment, roles.target, target)?;
    apply_assignment(pattern, &assignment)
}

fn assign_point(
    assignment: &mut CanonicalAssignment,
    vertex: VertexId,
    point: Point2,
) -> Option<()> {
    if !finite_point(point) {
        return None;
    }
    match assignment.get(&vertex.canonical_bytes()) {
        Some((stored_vertex, stored_point))
            if *stored_vertex != vertex || *stored_point != point =>
        {
            None
        }
        Some(_) => Some(()),
        None => {
            assignment.insert(vertex.canonical_bytes(), (vertex, point));
            Some(())
        }
    }
}

fn apply_assignment(
    pattern: &CreasePattern,
    assignment: &CanonicalAssignment,
) -> Option<CreasePattern> {
    let mut candidate = pattern.clone();
    let mut applied = 0usize;
    for vertex in &mut candidate.vertices {
        let Some((stored_vertex, point)) = assignment.get(&vertex.id.canonical_bytes()) else {
            continue;
        };
        if *stored_vertex != vertex.id {
            return None;
        }
        vertex.position = *point;
        applied = applied.checked_add(1)?;
    }
    (applied == assignment.len()).then_some(candidate)
}

fn find_edge(pattern: &CreasePattern, id: EdgeId) -> Option<&Edge> {
    pattern.edges.iter().find(|edge| edge.id == id)
}

fn edge_has_endpoints(edge: &Edge, first: VertexId, second: VertexId) -> bool {
    (edge.start == first && edge.end == second) || (edge.start == second && edge.end == first)
}

fn finite_point(point: Point2) -> bool {
    point.x.is_finite() && point.y.is_finite()
}
