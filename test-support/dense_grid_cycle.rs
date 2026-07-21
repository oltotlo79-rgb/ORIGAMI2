use ori_domain::{
    CreasePattern, Edge, EdgeId, EdgeKind, Paper, Point2, ProjectId, Vertex, VertexId,
};

#[allow(dead_code)]
pub fn three_by_three_dense_cycle_pattern() -> (CreasePattern, Paper, Vec<EdgeId>) {
    square_dense_cycle_pattern(3)
}

pub fn square_dense_cycle_pattern(side: usize) -> (CreasePattern, Paper, Vec<EdgeId>) {
    rectangular_dense_cycle_pattern(side, side)
}

pub fn rectangular_dense_cycle_pattern(
    columns: usize,
    rows: usize,
) -> (CreasePattern, Paper, Vec<EdgeId>) {
    assert!((3..=7).contains(&columns));
    assert!((3..=7).contains(&rows));
    let width = columns + 1;
    let height = rows + 1;
    let namespace = ProjectId::new();
    let vertices = (0..height)
        .flat_map(|y| {
            (0..width).map(move |x| Vertex {
                id: VertexId::derive_v5(namespace, &[0x71, y as u8, x as u8]),
                position: Point2::new(x as f64 * 20.0, y as f64 * 20.0),
            })
        })
        .collect::<Vec<_>>();
    let vertex = |x: usize, y: usize| vertices[y * width + x].id;
    let mut edges = Vec::new();
    let mut moving = Vec::new();
    for y in 0..height {
        for x in 0..columns {
            let id = EdgeId::derive_v5(namespace, &[0x72, y as u8, x as u8]);
            edges.push(Edge {
                id,
                start: vertex(x, y),
                end: vertex(x + 1, y),
                kind: if y == 0 || y == rows {
                    EdgeKind::Boundary
                } else {
                    EdgeKind::Mountain
                },
            });
        }
    }
    for x in 0..width {
        for y in 0..rows {
            let id = EdgeId::derive_v5(namespace, &[0x73, x as u8, y as u8]);
            let kind = if x == 0 || x == columns {
                EdgeKind::Boundary
            } else {
                EdgeKind::Valley
            };
            if kind != EdgeKind::Boundary {
                moving.push(id);
            }
            edges.push(Edge {
                id,
                start: vertex(x, y),
                end: vertex(x, y + 1),
                kind,
            });
        }
    }
    let boundary_vertices = (0..width)
        .map(|x| vertex(x, 0))
        .chain((1..height).map(|y| vertex(columns, y)))
        .chain((0..columns).rev().map(|x| vertex(x, rows)))
        .chain((1..rows).rev().map(|y| vertex(0, y)))
        .collect();
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
