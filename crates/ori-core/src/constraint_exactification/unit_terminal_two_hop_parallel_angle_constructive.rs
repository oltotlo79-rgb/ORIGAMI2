use ori_domain::{
    ConstraintId, CreasePattern, Edge, EdgeId, GeometricConstraintDocumentV1,
    GeometricConstraintKindV1, GeometricConstraintRecordV1, Point2, VertexId,
};

use crate::constraint_solver::{
    Binary64ResidualOnlyConstraintSatisfactionV1,
    certify_binary64_residual_only_constraint_overlay_v1,
};

pub(crate) const MAX_UNIT_TERMINAL_TWO_HOP_PARALLEL_ANGLE_RESIDUAL_ONLY_OVERLAY_VERTICES_V1: usize =
    256;

#[derive(Clone, Copy)]
struct ParallelRecord {
    id: ConstraintId,
    first_edge: EdgeId,
    second_edge: EdgeId,
}

#[derive(Clone, Copy)]
struct FixedLengthRecord {
    id: ConstraintId,
    edge: EdgeId,
}

#[derive(Clone, Copy)]
struct CoreShape {
    first_parallel_id: ConstraintId,
    second_parallel_id: ConstraintId,
    angle_id: ConstraintId,
    first_fixed_length_id: ConstraintId,
    second_fixed_length_id: ConstraintId,
    angle_vertex: VertexId,
    first_edge: EdgeId,
    middle_edge: EdgeId,
    second_edge: EdgeId,
}

impl CoreShape {
    fn contains_id(self, id: ConstraintId) -> bool {
        [
            self.first_parallel_id,
            self.second_parallel_id,
            self.angle_id,
            self.first_fixed_length_id,
            self.second_fixed_length_id,
        ]
        .contains(&id)
    }
}

/// Constructs one of the five independent deletion witnesses for the exact
/// unit-terminal, two-hop parallel/fixed-right-angle theorem.
///
/// Only a three-edge common-center star is admitted. The direct theorem does
/// not require that source topology, but this private semantic constructor
/// does: it lets the three stored edge vectors be assigned independently in
/// one bounded four-vertex overlay. Every candidate is accepted only after
/// the complete deletion document is re-evaluated by the unchanged frozen
/// production residual implementation.
pub(crate) fn construct_unit_terminal_two_hop_parallel_angle_residual_exact_deletion_assignment_v1(
    pattern: &CreasePattern,
    core: &[GeometricConstraintRecordV1],
    removed: ConstraintId,
    document: &GeometricConstraintDocumentV1,
) -> Option<Binary64ResidualOnlyConstraintSatisfactionV1> {
    if pattern.vertices.len()
        > MAX_UNIT_TERMINAL_TWO_HOP_PARALLEL_ANGLE_RESIDUAL_ONLY_OVERLAY_VERTICES_V1
    {
        return None;
    }
    let shape = classify_core(core)?;
    if !shape.contains_id(removed) || !is_exact_deletion_document(core, removed, document) {
        return None;
    }

    let first = find_edge(pattern, shape.first_edge)?;
    let middle = find_edge(pattern, shape.middle_edge)?;
    let second = find_edge(pattern, shape.second_edge)?;
    let center = shape.angle_vertex;
    let first_outer = outer_vertex(first, center)?;
    let middle_outer = outer_vertex(middle, center)?;
    let second_outer = outer_vertex(second, center)?;
    if !all_distinct_vertices([center, first_outer, middle_outer, second_outer]) {
        return None;
    }

    let one = Point2::new(1.0, 0.0);
    let vertical_one = Point2::new(0.0, 1.0);
    let huge_terminal = Point2::new(
        f64::from_bits(0x5fed_c4c2_f9c3_bdb0),
        f64::from_bits(0xdfd7_7af1_2e6b_a7b3),
    );
    let huge_middle = Point2::new(
        f64::from_bits(0x5fd7_7af1_2e6b_a7b3),
        f64::from_bits(0x5fed_c4c2_f9c3_bdb0),
    );
    let unit_terminal = Point2::new(
        f64::from_bits(0x3fd7_7af1_2e6b_a7b4),
        f64::from_bits(0x3fed_c4c2_f9c3_bdb1),
    );

    let (first_vector, middle_vector, second_vector) = if removed == shape.first_parallel_id {
        (one, vertical_one, vertical_one)
    } else if removed == shape.second_parallel_id {
        (one, one, vertical_one)
    } else if removed == shape.angle_id {
        (one, one, one)
    } else if removed == shape.first_fixed_length_id {
        (huge_terminal, huge_middle, unit_terminal)
    } else if removed == shape.second_fixed_length_id {
        (unit_terminal, huge_middle, huge_terminal)
    } else {
        return None;
    };

    let assignments = [
        (center, Point2::new(0.0, 0.0)),
        directed_outer_assignment(first, center, first_vector)?,
        directed_outer_assignment(middle, center, middle_vector)?,
        directed_outer_assignment(second, center, second_vector)?,
    ];
    if assignments
        .iter()
        .any(|(_, point)| !point.x.is_finite() || !point.y.is_finite())
        || !all_distinct_vertices(assignments.map(|(vertex, _)| vertex))
    {
        return None;
    }

    let mut overlay = Vec::new();
    overlay.try_reserve_exact(pattern.vertices.len()).ok()?;
    for vertex in &pattern.vertices {
        let point = assignments
            .iter()
            .find_map(|(assigned, point)| (*assigned == vertex.id).then_some(*point))
            .unwrap_or(Point2::new(0.0, 0.0));
        overlay.push((vertex.id, point));
    }
    certify_binary64_residual_only_constraint_overlay_v1(pattern, document, &overlay)
        .ok()
        .flatten()
}

fn classify_core(core: &[GeometricConstraintRecordV1]) -> Option<CoreShape> {
    if core.len() != 5 || !all_distinct_constraint_ids(core) {
        return None;
    }

    let mut angle = None;
    let mut parallels = [None, None];
    let mut fixed_lengths = [None, None];
    for record in core {
        match &record.constraint {
            GeometricConstraintKindV1::Parallel {
                first_edge,
                second_edge,
            } => insert_two(
                &mut parallels,
                ParallelRecord {
                    id: record.id,
                    first_edge: *first_edge,
                    second_edge: *second_edge,
                },
            )?,
            GeometricConstraintKindV1::FixedAngle {
                vertex,
                first_edge,
                second_edge,
                angle_degrees,
            } if angle_degrees.to_bits() == 90.0_f64.to_bits() && angle.is_none() => {
                angle = Some((record.id, *vertex, *first_edge, *second_edge));
            }
            GeometricConstraintKindV1::FixedLength { edge, length_mm }
                if length_mm.to_bits() == 1.0_f64.to_bits() =>
            {
                insert_two(
                    &mut fixed_lengths,
                    FixedLengthRecord {
                        id: record.id,
                        edge: *edge,
                    },
                )?;
            }
            _ => return None,
        }
    }

    let (angle_id, angle_vertex, first_edge, second_edge) = angle?;
    if first_edge == second_edge {
        return None;
    }
    let [Some(first_parallel), Some(second_parallel)] = parallels else {
        return None;
    };
    let [Some(first_fixed), Some(second_fixed)] = fixed_lengths else {
        return None;
    };
    let fixed_records = [first_fixed, second_fixed];
    let first_fixed = fixed_records
        .into_iter()
        .find(|fixed| fixed.edge == first_edge)?;
    let second_fixed = fixed_records
        .into_iter()
        .find(|fixed| fixed.edge == second_edge)?;
    if first_fixed.id == second_fixed.id {
        return None;
    }

    let parallel_records = [first_parallel, second_parallel];
    let mut middle_edge = None;
    for record in parallel_records {
        for edge in [record.first_edge, record.second_edge] {
            if edge == first_edge || edge == second_edge {
                continue;
            }
            match middle_edge {
                Some(existing) if existing != edge => return None,
                Some(_) => {}
                None => middle_edge = Some(edge),
            }
        }
    }
    let middle_edge = middle_edge?;
    let first_parallel = parallel_records
        .into_iter()
        .find(|record| pair_matches(*record, first_edge, middle_edge))?;
    let second_parallel = parallel_records
        .into_iter()
        .find(|record| pair_matches(*record, middle_edge, second_edge))?;
    if first_parallel.id == second_parallel.id {
        return None;
    }

    Some(CoreShape {
        first_parallel_id: first_parallel.id,
        second_parallel_id: second_parallel.id,
        angle_id,
        first_fixed_length_id: first_fixed.id,
        second_fixed_length_id: second_fixed.id,
        angle_vertex,
        first_edge,
        middle_edge,
        second_edge,
    })
}

fn insert_two<T: Copy>(slots: &mut [Option<T>; 2], value: T) -> Option<()> {
    if slots[0].is_none() {
        slots[0] = Some(value);
        Some(())
    } else if slots[1].is_none() {
        slots[1] = Some(value);
        Some(())
    } else {
        None
    }
}

fn all_distinct_constraint_ids(core: &[GeometricConstraintRecordV1]) -> bool {
    core.iter()
        .enumerate()
        .all(|(index, record)| core[index + 1..].iter().all(|other| record.id != other.id))
}

fn is_exact_deletion_document(
    core: &[GeometricConstraintRecordV1],
    removed: ConstraintId,
    document: &GeometricConstraintDocumentV1,
) -> bool {
    document.constraints.len() == core.len().saturating_sub(1)
        && core
            .iter()
            .filter(|record| record.id != removed)
            .all(|record| {
                document
                    .constraints
                    .iter()
                    .any(|candidate| candidate == record)
            })
        && document
            .constraints
            .iter()
            .all(|record| record.id != removed)
}

fn pair_matches(record: ParallelRecord, left: EdgeId, right: EdgeId) -> bool {
    (record.first_edge == left && record.second_edge == right)
        || (record.first_edge == right && record.second_edge == left)
}

fn find_edge(pattern: &CreasePattern, id: EdgeId) -> Option<&Edge> {
    pattern.edges.iter().find(|edge| edge.id == id)
}

fn outer_vertex(edge: &Edge, center: VertexId) -> Option<VertexId> {
    if edge.start == center && edge.end != center {
        Some(edge.end)
    } else if edge.end == center && edge.start != center {
        Some(edge.start)
    } else {
        None
    }
}

fn all_distinct_vertices<const N: usize>(vertices: [VertexId; N]) -> bool {
    vertices
        .iter()
        .enumerate()
        .all(|(index, vertex)| vertices[index + 1..].iter().all(|other| vertex != other))
}

fn directed_outer_assignment(
    edge: &Edge,
    center: VertexId,
    vector: Point2,
) -> Option<(VertexId, Point2)> {
    if edge.start == center && edge.end != center {
        Some((edge.end, vector))
    } else if edge.end == center && edge.start != center {
        Some((edge.start, Point2::new(-vector.x, -vector.y)))
    } else {
        None
    }
}
