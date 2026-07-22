use std::collections::{BTreeMap, BTreeSet};

use ori_domain::{
    CreasePattern, Edge, EdgeId, EdgeKind, Paper, Point2, ProjectId, Vertex, VertexId,
};

pub type MiuraPatternFixture = (CreasePattern, Paper, Vec<EdgeId>);
pub type IndependentMiuraBlocksWithDocument = ([MiuraPatternFixture; 2], MiuraPatternFixture);

pub fn two_patch_miura_cactus_pattern() -> (CreasePattern, Paper, Vec<EdgeId>) {
    let cells = [
        (0_i8, 0_i8),
        (-1, 0),
        (0, 1),
        (-1, 1),
        (1, 0),
        (0, -1),
        (1, -1),
    ];
    pattern_for_cells(&cells, ProjectId::new())
}

pub fn independent_three_by_three_miura_blocks() -> [(CreasePattern, Paper, Vec<EdgeId>); 2] {
    independent_three_by_three_miura_blocks_with_document().0
}

pub fn independent_three_by_three_miura_blocks_with_document() -> IndependentMiuraBlocksWithDocument
{
    let namespace = ProjectId::new();
    let northwest = (-2..=0)
        .flat_map(|x| (0..=2).map(move |y| (x, y)))
        .collect::<Vec<_>>();
    let southeast = (0..=2)
        .flat_map(|x| (-2..=0).map(move |y| (x, y)))
        .collect::<Vec<_>>();
    let combined = northwest
        .iter()
        .chain(&southeast)
        .copied()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    (
        [
            pattern_for_cells(&northwest, namespace),
            pattern_for_cells(&southeast, namespace),
        ],
        pattern_for_cells(&combined, namespace),
    )
}

fn pattern_for_cells(
    cells: &[(i8, i8)],
    namespace: ProjectId,
) -> (CreasePattern, Paper, Vec<EdgeId>) {
    let mut points = BTreeSet::new();
    let mut incidence = BTreeMap::<((i8, i8), (i8, i8)), (usize, (i8, i8), (i8, i8))>::new();
    for &(x, y) in cells {
        let corners = [(x, y), (x + 1, y), (x + 1, y + 1), (x, y + 1)];
        points.extend(corners);
        for index in 0..4 {
            let start = corners[index];
            let end = corners[(index + 1) % 4];
            let key = if start < end {
                (start, end)
            } else {
                (end, start)
            };
            incidence
                .entry(key)
                .and_modify(|entry| entry.0 += 1)
                .or_insert((1, start, end));
        }
    }
    let vertices = points
        .iter()
        .map(|&(x, y)| Vertex {
            id: VertexId::derive_v5(namespace, &[0xc1, (x + 4) as u8, (y + 4) as u8]),
            position: Point2::new(f64::from(x) * 20.0, f64::from(y) * 20.0),
        })
        .collect::<Vec<_>>();
    let vertex = |point: (i8, i8)| {
        vertices[points
            .iter()
            .position(|candidate| *candidate == point)
            .unwrap()]
        .id
    };
    let mut moving = Vec::new();
    let edges = incidence
        .iter()
        .map(|(&(a, b), &(count, start, end))| {
            let id = EdgeId::derive_v5(
                namespace,
                &[
                    0xc2,
                    (a.0 + 4) as u8,
                    (a.1 + 4) as u8,
                    (b.0 + 4) as u8,
                    (b.1 + 4) as u8,
                ],
            );
            let kind = if count == 1 {
                EdgeKind::Boundary
            } else if a.1 == b.1 {
                moving.push(id);
                EdgeKind::Mountain
            } else if a.1.rem_euclid(2) == 0 {
                EdgeKind::Valley
            } else {
                EdgeKind::Mountain
            };
            Edge {
                id,
                start: vertex(start),
                end: vertex(end),
                kind,
            }
        })
        .collect::<Vec<_>>();
    let directed = incidence
        .values()
        .filter(|(count, _, _)| *count == 1)
        .map(|(_, start, end)| (*start, *end))
        .collect::<Vec<_>>();
    let mut boundary = vec![directed[0].0];
    while boundary.len() < directed.len() {
        let cursor = *boundary.last().unwrap();
        let next = directed
            .iter()
            .find(|(start, _)| *start == cursor)
            .unwrap()
            .1;
        boundary.push(next);
    }
    let boundary_vertices = boundary.into_iter().map(vertex).collect();
    (
        CreasePattern { vertices, edges },
        Paper {
            boundary_vertices,
            thickness_mm: 0.1,
            ..Paper::default()
        },
        moving,
    )
}
