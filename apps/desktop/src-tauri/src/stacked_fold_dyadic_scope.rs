//! Strict fail-closed admission for the bounded dyadic geometry scope.
//!
//! The read command and graph exploration remain in the parent module. This
//! module owns only deterministic document and topology scope validation.

pub(super) fn strict_dyadic_geometry_is_in_scope_v1(project: &super::ProjectState) -> bool {
    let topology = project
        .editor
        .topology_analysis_input(project.project_id)
        .analyze();
    let Some(snapshot) = topology.simulation_snapshot() else {
        return false;
    };
    if !strict_dyadic_topology_snapshot_is_in_scope_v1(snapshot) {
        return false;
    }
    let pattern = project.editor.pattern();
    if pattern
        .edges
        .iter()
        .any(|edge| edge.kind == ori_domain::EdgeKind::Cut)
    {
        return false;
    }
    let boundary = &project.editor.paper().boundary_vertices;
    if boundary.len() < 3
        || boundary
            .iter()
            .copied()
            .collect::<std::collections::HashSet<_>>()
            .len()
            != boundary.len()
    {
        return false;
    }
    let point = |id| {
        pattern
            .vertices
            .iter()
            .find(|vertex| vertex.id == id)
            .map(|vertex| vertex.position)
            .filter(|point| point.x.is_finite() && point.y.is_finite())
    };
    let Some(points) = boundary
        .iter()
        .copied()
        .map(point)
        .collect::<Option<Vec<_>>>()
    else {
        return false;
    };
    if points.iter().enumerate().any(|(index, first)| {
        let second = points[(index + 1) % points.len()];
        first.x.to_bits() == second.x.to_bits() && first.y.to_bits() == second.y.to_bits()
    }) {
        return false;
    }
    let cross =
        |first: ori_domain::Point2, second: ori_domain::Point2, third: ori_domain::Point2| {
            (second.x - first.x) * (third.y - first.y) - (second.y - first.y) * (third.x - first.x)
        };
    let on_segment =
        |first: ori_domain::Point2, second: ori_domain::Point2, point: ori_domain::Point2| {
            cross(first, second, point) == 0.0
                && point.x >= first.x.min(second.x)
                && point.x <= first.x.max(second.x)
                && point.y >= first.y.min(second.y)
                && point.y <= first.y.max(second.y)
        };
    let intersects = |first: ori_domain::Point2,
                      second: ori_domain::Point2,
                      third: ori_domain::Point2,
                      fourth: ori_domain::Point2| {
        let values = [
            cross(first, second, third),
            cross(first, second, fourth),
            cross(third, fourth, first),
            cross(third, fourth, second),
        ];
        (values[0].is_sign_positive() != values[1].is_sign_positive()
            && values[2].is_sign_positive() != values[3].is_sign_positive()
            && values.iter().all(|value| *value != 0.0))
            || on_segment(first, second, third)
            || on_segment(first, second, fourth)
            || on_segment(third, fourth, first)
            || on_segment(third, fourth, second)
    };
    for first in 0..points.len() {
        for second in (first + 1)..points.len() {
            if second == first + 1 || (first == 0 && second + 1 == points.len()) {
                continue;
            }
            if intersects(
                points[first],
                points[(first + 1) % points.len()],
                points[second],
                points[(second + 1) % points.len()],
            ) {
                return false;
            }
        }
    }
    let mut orientation = 0_i8;
    for index in 0..boundary.len() {
        let turn = cross(
            points[index],
            points[(index + 1) % points.len()],
            points[(index + 2) % points.len()],
        );
        if turn == 0.0 {
            continue;
        }
        let sign = if turn.is_sign_positive() { 1 } else { -1 };
        if orientation == 0 {
            orientation = sign;
        } else if orientation != sign {
            return false;
        }
    }
    orientation != 0
}

fn strict_dyadic_topology_snapshot_is_in_scope_v1(
    snapshot: &ori_topology::TopologySnapshot,
) -> bool {
    snapshot.material_components.len() == 1
        && snapshot
            .faces
            .iter()
            .all(|face| face.holes.is_empty() && face.seams.is_empty())
}
