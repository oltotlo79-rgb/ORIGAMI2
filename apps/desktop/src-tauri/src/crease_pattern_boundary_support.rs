//! Shared fail-closed boundary guards for crease-pattern format workflows.
//!
//! Format-specific staging, conversion, export, and wire DTOs remain in their
//! owning modules. This module owns only the active-edge containment invariant
//! shared by FOLD import, SVG import, and crease-pattern export.

use super::*;

const MAX_ACTIVE_EDGE_CONTAINMENT_TESTS: usize = 1_000_000;

pub(super) fn validate_active_edge_containment(
    project: &ProjectState,
    format_label: &str,
) -> Result<(), String> {
    let positions = project
        .editor
        .pattern()
        .vertices
        .iter()
        .map(|vertex| (vertex.id, vertex.position))
        .collect::<HashMap<_, _>>();
    let boundary = project
        .editor
        .paper()
        .boundary_vertices
        .iter()
        .map(|vertex| {
            positions
                .get(vertex)
                .copied()
                .ok_or_else(|| format!("converted {format_label} boundary could not be resolved"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let active_edges = project
        .editor
        .pattern()
        .edges
        .iter()
        .filter(|edge| {
            matches!(
                edge.kind,
                EdgeKind::Mountain | EdgeKind::Valley | EdgeKind::Cut
            )
        })
        .collect::<Vec<_>>();
    let containment_tests = active_edges
        .len()
        .checked_mul(boundary.len())
        .ok_or_else(|| format!("converted {format_label} containment work is not representable"))?;
    if containment_tests > MAX_ACTIVE_EDGE_CONTAINMENT_TESTS {
        return Err(format!(
            "converted {format_label} needs {containment_tests} containment tests; the limit is {MAX_ACTIVE_EDGE_CONTAINMENT_TESTS}"
        ));
    }

    let mut outside_count = 0;
    for edge in active_edges {
        let start = positions
            .get(&edge.start)
            .copied()
            .ok_or_else(|| format!("converted {format_label} edge start could not be resolved"))?;
        let end = positions
            .get(&edge.end)
            .copied()
            .ok_or_else(|| format!("converted {format_label} edge end could not be resolved"))?;
        let relation = segment_midpoint_polygon_relation(start, end, &boundary).map_err(|_| {
            format!("converted {format_label} edge containment could not be classified")
        })?;
        if relation != PointPolygonRelation::Inside {
            outside_count += 1;
        }
    }
    if outside_count > 0 {
        return Err(format!(
            "converted {format_label} has {outside_count} active edge(s) outside the paper boundary"
        ));
    }
    Ok(())
}
