use std::collections::{HashMap, HashSet, VecDeque};

use ori_domain::{EdgeId, FaceId};

use super::MaterialHingeGraphAudit;
use crate::{
    CanonicalCycleScheduleV1, MaterialHingeGraphGeometry, TreeHinge,
    transform::{length, scale, subtract},
};

// Exact carrier identity for a bounded non-cactus rectangular grid whose two
// dimensions are both at least two.
//
// The old recognizer searched only `3..=9` dimensions and inferred the grid
// from three aggregate counts. Nine was a fixture limit, not a kinematics
// limit, and aggregate counts do not authenticate a Cartesian grid. The V1
// fast path now stays within the native face/hinge ceilings, performs four
// fixed BFS traversals from the graph corners, and verifies every Cartesian
// adjacency slot before considering the schedule. This keeps recognition
// linear after a bounded factor scan and prevents an unrelated graph with the
// same `(faces, hinges, cycle-rank)` tuple from entering the analytic proof.
const MAX_DENSE_GRID_FACES_V1: usize = 10_001;
const MAX_DENSE_GRID_HINGES_V1: usize = ori_domain::MAX_INSTRUCTION_HINGES_PER_STEP;
const MAX_DENSE_GRID_FACTOR_TESTS_V1: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DenseGridHingeFamilyV1 {
    ColumnBoundary,
    RowBoundary,
}

#[derive(Debug, Clone, Copy)]
struct DenseGridHingeV1<'a> {
    hinge: &'a TreeHinge,
    family: DenseGridHingeFamilyV1,
    carrier: usize,
    segment: usize,
    forward_face: FaceId,
}

#[derive(Debug)]
struct RecognizedDenseGridV1<'a> {
    columns: usize,
    rows: usize,
    hinges: Vec<DenseGridHingeV1<'a>>,
}

fn bounded_dense_grid_dimensions_v1(
    face_count: usize,
    hinge_count: usize,
    closure_count: usize,
) -> Option<(usize, usize)> {
    if !(4..=MAX_DENSE_GRID_FACES_V1).contains(&face_count)
        || hinge_count > MAX_DENSE_GRID_HINGES_V1
    {
        return None;
    }
    let mut factor_tests = 0usize;
    let mut columns = 2usize;
    while columns <= face_count / columns {
        factor_tests = factor_tests.checked_add(1)?;
        if factor_tests > MAX_DENSE_GRID_FACTOR_TESTS_V1 {
            return None;
        }
        if face_count.is_multiple_of(columns) {
            let rows = face_count / columns;
            if rows >= 2 {
                let expected_hinges = face_count
                    .checked_mul(2)?
                    .checked_sub(columns)?
                    .checked_sub(rows)?;
                let expected_closures =
                    columns.checked_sub(1)?.checked_mul(rows.checked_sub(1)?)?;
                if hinge_count == expected_hinges && closure_count == expected_closures {
                    return Some((columns, rows));
                }
            }
        }
        columns = columns.checked_add(1)?;
    }
    None
}

fn dense_grid_bfs_distances_v1(adjacency: &[Vec<usize>], start: usize) -> Option<Vec<usize>> {
    if start >= adjacency.len() {
        return None;
    }
    let mut distances = vec![usize::MAX; adjacency.len()];
    let mut queue = VecDeque::with_capacity(adjacency.len());
    distances[start] = 0;
    queue.push_back(start);
    while let Some(face) = queue.pop_front() {
        let next_distance = distances[face].checked_add(1)?;
        for &next in &adjacency[face] {
            if distances.get(next).copied()? == usize::MAX {
                distances[next] = next_distance;
                queue.push_back(next);
            }
        }
    }
    distances
        .iter()
        .all(|distance| *distance != usize::MAX)
        .then_some(distances)
}

fn dense_grid_coordinates_v1(
    columns: usize,
    rows: usize,
    corner_indices: &[usize],
    corner_distances: &[Vec<usize>],
) -> Option<Vec<(usize, usize)>> {
    corner_indices.first()?;
    let origin_distances = corner_distances.first()?;
    for x_corner_position in 1..corner_indices.len() {
        for y_corner_position in 1..corner_indices.len() {
            if x_corner_position == y_corner_position {
                continue;
            }
            let x_corner = corner_indices[x_corner_position];
            let y_corner = corner_indices[y_corner_position];
            if origin_distances.get(x_corner).copied()? != columns - 1
                || origin_distances.get(y_corner).copied()? != rows - 1
            {
                continue;
            }
            let x_distances = corner_distances.get(x_corner_position)?;
            let y_distances = corner_distances.get(y_corner_position)?;
            let mut coordinates = Vec::with_capacity(origin_distances.len());
            let mut occupied = vec![false; columns.checked_mul(rows)?];
            let mut valid = true;
            for face in 0..origin_distances.len() {
                let d0 = i64::try_from(origin_distances[face]).ok()?;
                let dx = i64::try_from(x_distances[face]).ok()?;
                let dy = i64::try_from(y_distances[face]).ok()?;
                let x_numerator = d0
                    .checked_sub(dx)?
                    .checked_add(i64::try_from(columns - 1).ok()?)?;
                let y_numerator = d0
                    .checked_sub(dy)?
                    .checked_add(i64::try_from(rows - 1).ok()?)?;
                if x_numerator < 0
                    || y_numerator < 0
                    || x_numerator % 2 != 0
                    || y_numerator % 2 != 0
                {
                    valid = false;
                    break;
                }
                let x = usize::try_from(x_numerator / 2).ok()?;
                let y = usize::try_from(y_numerator / 2).ok()?;
                if x >= columns || y >= rows || origin_distances[face] != x.checked_add(y)? {
                    valid = false;
                    break;
                }
                let slot = y.checked_mul(columns)?.checked_add(x)?;
                if occupied.get(slot).copied().is_none_or(|seen| seen) {
                    valid = false;
                    break;
                }
                occupied[slot] = true;
                coordinates.push((x, y));
            }
            if valid && occupied.into_iter().all(|seen| seen) {
                return Some(coordinates);
            }
        }
    }
    None
}

fn recognize_dense_grid_v1<'a>(
    geometry: &'a MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
) -> Option<RecognizedDenseGridV1<'a>> {
    let (columns, rows) = bounded_dense_grid_dimensions_v1(
        geometry.face_ids().len(),
        geometry.hinges().len(),
        audit.closure_hinges().len(),
    )?;
    let mut faces = geometry.face_ids().to_vec();
    faces.sort_unstable_by_key(FaceId::canonical_bytes);
    if faces.windows(2).any(|pair| pair[0] == pair[1]) || faces != audit.faces() {
        return None;
    }
    let face_indices = faces
        .iter()
        .copied()
        .enumerate()
        .map(|(index, face)| (face, index))
        .collect::<HashMap<_, _>>();
    if face_indices.len() != faces.len() {
        return None;
    }
    let mut adjacency = vec![Vec::new(); faces.len()];
    let mut face_pairs = HashSet::with_capacity(geometry.hinges().len());
    let mut edge_ids = HashSet::with_capacity(geometry.hinges().len());
    for hinge in geometry.hinges() {
        let left = *face_indices.get(&hinge.left_face())?;
        let right = *face_indices.get(&hinge.right_face())?;
        if left == right || !edge_ids.insert(hinge.edge()) {
            return None;
        }
        let pair = if left < right {
            (left, right)
        } else {
            (right, left)
        };
        if !face_pairs.insert(pair) {
            return None;
        }
        adjacency[left].push(right);
        adjacency[right].push(left);
    }
    if adjacency
        .iter()
        .any(|neighbors| !(2..=4).contains(&neighbors.len()))
    {
        return None;
    }
    let mut corners = adjacency
        .iter()
        .enumerate()
        .filter_map(|(index, neighbors)| (neighbors.len() == 2).then_some(index))
        .collect::<Vec<_>>();
    if corners.len() != 4 {
        return None;
    }
    corners.sort_unstable_by_key(|index| faces[*index].canonical_bytes());
    let corner_distances = corners
        .iter()
        .map(|corner| dense_grid_bfs_distances_v1(&adjacency, *corner))
        .collect::<Option<Vec<_>>>()?;
    let coordinates = dense_grid_coordinates_v1(columns, rows, &corners, &corner_distances)?;

    let column_slots = columns.checked_sub(1)?.checked_mul(rows)?;
    let row_slots = columns.checked_mul(rows.checked_sub(1)?)?;
    if column_slots.checked_add(row_slots)? != geometry.hinges().len() {
        return None;
    }
    let mut seen_columns = vec![false; column_slots];
    let mut seen_rows = vec![false; row_slots];
    let mut hinges = Vec::with_capacity(geometry.hinges().len());
    for hinge in geometry.hinges() {
        let left_index = *face_indices.get(&hinge.left_face())?;
        let right_index = *face_indices.get(&hinge.right_face())?;
        let left = coordinates[left_index];
        let right = coordinates[right_index];
        let (family, carrier, segment, forward_index, slot, seen) =
            if left.1 == right.1 && left.0.abs_diff(right.0) == 1 {
                let carrier = left.0.min(right.0);
                let segment = left.1;
                let forward = if left.0 < right.0 {
                    left_index
                } else {
                    right_index
                };
                let slot = carrier.checked_mul(rows)?.checked_add(segment)?;
                (
                    DenseGridHingeFamilyV1::ColumnBoundary,
                    carrier,
                    segment,
                    forward,
                    slot,
                    &mut seen_columns,
                )
            } else if left.0 == right.0 && left.1.abs_diff(right.1) == 1 {
                let carrier = left.1.min(right.1);
                let segment = left.0;
                let forward = if left.1 < right.1 {
                    left_index
                } else {
                    right_index
                };
                let slot = carrier.checked_mul(columns)?.checked_add(segment)?;
                (
                    DenseGridHingeFamilyV1::RowBoundary,
                    carrier,
                    segment,
                    forward,
                    slot,
                    &mut seen_rows,
                )
            } else {
                return None;
            };
        if seen.get(slot).copied().is_none_or(|occupied| occupied) {
            return None;
        }
        seen[slot] = true;
        hinges.push(DenseGridHingeV1 {
            hinge,
            family,
            carrier,
            segment,
            forward_face: faces[forward_index],
        });
    }
    if seen_columns.into_iter().any(|seen| !seen) || seen_rows.into_iter().any(|seen| !seen) {
        return None;
    }
    Some(RecognizedDenseGridV1 {
        columns,
        rows,
        hinges,
    })
}

fn dense_grid_directed_axis_v1(record: DenseGridHingeV1<'_>) -> Option<[f64; 3]> {
    let assignment_sign = match record.hinge.assignment() {
        ori_topology::FoldAssignment::Mountain => 1.0,
        ori_topology::FoldAssignment::Valley => -1.0,
    };
    let traversal_sign = if record.hinge.left_face() == record.forward_face {
        1.0
    } else if record.hinge.right_face() == record.forward_face {
        -1.0
    } else {
        return None;
    };
    let sign = assignment_sign * traversal_sign;
    let axis = record.hinge.axis();
    let directed = [axis.x() * sign, axis.y() * sign, axis.z() * sign];
    directed.into_iter().all(f64::is_finite).then_some(directed)
}

fn dense_grid_valid_axis_line_v1(record: DenseGridHingeV1<'_>) -> bool {
    let Ok(delta) = subtract(record.hinge.end(), record.hinge.start()) else {
        return false;
    };
    let Ok(delta_length) = length(delta) else {
        return false;
    };
    let Ok(expected_axis) = scale(delta, 1.0 / delta_length) else {
        return false;
    };
    // The native kinematics generator rotates about `(start, axis)`. Authenticate
    // the stored axis by replaying the same normalized endpoint-delta
    // construction used by material-graph preparation. Requiring an exact
    // floating cross product between that rounded unit axis and its pre-
    // normalization delta is stronger than the model: a non-cardinal binary64
    // segment can replay to the identical axis while the redundant cross rounds
    // to one ULP away from zero.
    expected_axis == record.hinge.axis()
}

fn dense_grid_same_directed_line_v1(
    reference: DenseGridHingeV1<'_>,
    candidate: DenseGridHingeV1<'_>,
) -> bool {
    let (Some(reference_axis), Some(candidate_axis)) = (
        dense_grid_directed_axis_v1(reference),
        dense_grid_directed_axis_v1(candidate),
    ) else {
        return false;
    };
    if reference_axis != candidate_axis {
        return false;
    }
    let Ok(reference_delta) = subtract(reference.hinge.end(), reference.hinge.start()) else {
        return false;
    };
    // Stored axes are rounded normalizations of their endpoint deltas. Their
    // exact equality above authenticates the directed rotation generator, but
    // using a rounded unit axis again for the line-incidence determinant can
    // create a one-ULP nonzero cross product for a genuinely collinear
    // non-cardinal segment. Test both candidate endpoints against the raw,
    // replay-authenticated reference delta instead. If the binary64 points are
    // exactly collinear, both products in each determinant have the same exact
    // real value and therefore round identically.
    [candidate.hinge.start(), candidate.hinge.end()]
        .into_iter()
        .all(|point| {
            let Ok(offset) = subtract(point, reference.hinge.start()) else {
                return false;
            };
            let cross = [
                offset.y() * reference_delta.z() - offset.z() * reference_delta.y(),
                offset.z() * reference_delta.x() - offset.x() * reference_delta.z(),
                offset.x() * reference_delta.y() - offset.y() * reference_delta.x(),
            ];
            cross.into_iter().all(|value| value == 0.0)
        })
}

fn dense_grid_motion_has_exact_carriers_v1(
    grid: &RecognizedDenseGridV1<'_>,
    moving: &HashSet<EdgeId>,
) -> bool {
    let matches_family = |family, carrier_count: usize, segment_count: usize| {
        let family_records = grid
            .hinges
            .iter()
            .copied()
            .filter(|record| record.family == family)
            .collect::<Vec<_>>();
        if Some(family_records.len()) != carrier_count.checked_mul(segment_count)
            || grid
                .hinges
                .iter()
                .any(|record| record.family != family && moving.contains(&record.hinge.edge()))
        {
            return false;
        }
        let selected_carriers = (0..carrier_count)
            .filter(|carrier| {
                family_records.iter().any(|record| {
                    record.carrier == *carrier && moving.contains(&record.hinge.edge())
                })
            })
            .collect::<Vec<_>>();
        if selected_carriers.is_empty() {
            return false;
        }
        // Every path orthogonal to this family crosses the selected carriers
        // in the same canonical order. A complete carrier gives every such
        // path the same exact directed generator and the same bit-identical
        // collective profile; an unselected carrier and the other family are
        // exact zero identities. Therefore any non-empty carrier subset gives
        // every parallel row/column the same transform sequence. This proves
        // path independence without assuming that distinct carrier rotations
        // commute, and it is why selection need not be limited to one or all.
        for carrier in selected_carriers {
            let mut records = family_records
                .iter()
                .copied()
                .filter(|record| record.carrier == carrier)
                .collect::<Vec<_>>();
            records.sort_unstable_by_key(|record| record.segment);
            if records.len() != segment_count
                || records.iter().enumerate().any(|(segment, record)| {
                    record.segment != segment || !moving.contains(&record.hinge.edge())
                })
                || records
                    .iter()
                    .any(|record| !dense_grid_valid_axis_line_v1(*record))
            {
                return false;
            }
            let Some(reference) = records.first().copied() else {
                return false;
            };
            if records
                .iter()
                .copied()
                .skip(1)
                .any(|record| !dense_grid_same_directed_line_v1(reference, record))
            {
                return false;
            }
        }
        family_records
            .iter()
            .filter(|record| moving.contains(&record.hinge.edge()))
            .count()
            == moving.len()
    };
    matches_family(
        DenseGridHingeFamilyV1::ColumnBoundary,
        grid.columns - 1,
        grid.rows,
    ) || matches_family(
        DenseGridHingeFamilyV1::RowBoundary,
        grid.rows - 1,
        grid.columns,
    )
}

pub(super) fn dense_parallel_grid_cycle_closure_premises_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    schedule: &CanonicalCycleScheduleV1,
    tolerance: f64,
) -> bool {
    let Some(grid) = recognize_dense_grid_v1(geometry, audit) else {
        return false;
    };
    if !tolerance.is_finite() || tolerance < 0.0 {
        return false;
    }
    let Some(moving_edges) = schedule.collective_profile_edges_v1() else {
        return false;
    };
    let moving = moving_edges.into_iter().collect::<HashSet<_>>();
    if moving.is_empty()
        || moving.len()
            != grid
                .hinges
                .iter()
                .filter(|record| moving.contains(&record.hinge.edge()))
                .count()
    {
        return false;
    }
    if !dense_grid_motion_has_exact_carriers_v1(&grid, &moving) {
        return false;
    }
    let (Some(initial), Some(midpoint), Some(target)) = (
        schedule.evaluate(0.0),
        schedule.evaluate(0.5),
        schedule.evaluate(1.0),
    ) else {
        return false;
    };
    let initial_by_edge = initial
        .as_slice()
        .iter()
        .map(|angle| (angle.edge(), angle.angle_degrees().to_bits()))
        .collect::<HashMap<_, _>>();
    if initial_by_edge.len() != grid.hinges.len()
        || grid.hinges.iter().any(|record| {
            !moving.contains(&record.hinge.edge())
                && (!schedule.is_exact_constant_profile_v1(record.hinge.edge())
                    || initial_by_edge.get(&record.hinge.edge()).copied()
                        != Some(0.0_f64.to_bits()))
        })
    {
        return false;
    }
    let Ok(_pose) = geometry.solve_closed(audit, fixed_face, &initial, tolerance) else {
        return false;
    };
    [midpoint, target].into_iter().all(|angles| {
        geometry
            .solve_closed(audit, fixed_face, &angles, tolerance)
            .is_ok()
    })
}

#[cfg(test)]
#[path = "dense_grid_tests.rs"]
mod tests;
