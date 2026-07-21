use ori_domain::{
    CreasePattern, Edge, EdgeId, EdgeKind, Paper, Point2, ProjectId, Vertex, VertexId,
};

pub fn three_by_three_dense_cycle_pattern() -> (CreasePattern, Paper, Vec<EdgeId>) {
    let namespace = ProjectId::new();
    let vertices = (0..4)
        .flat_map(|y| {
            (0..4).map(move |x| Vertex {
                id: VertexId::derive_v5(namespace, &[0x71, y, x]),
                position: Point2::new(f64::from(x) * 20.0, f64::from(y) * 20.0),
            })
        })
        .collect::<Vec<_>>();
    let vertex = |x: usize, y: usize| vertices[y * 4 + x].id;
    let mut edges = Vec::new();
    let mut moving = Vec::new();
    for y in 0..4 {
        for x in 0..3 {
            let id = EdgeId::derive_v5(namespace, &[0x72, y as u8, x as u8]);
            edges.push(Edge {
                id,
                start: vertex(x, y),
                end: vertex(x + 1, y),
                kind: if y == 0 || y == 3 {
                    EdgeKind::Boundary
                } else {
                    EdgeKind::Mountain
                },
            });
        }
    }
    for x in 0..4 {
        for y in 0..3 {
            let id = EdgeId::derive_v5(namespace, &[0x73, x as u8, y as u8]);
            let kind = if x == 0 || x == 3 {
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
    let boundary_vertices = (0..4)
        .map(|x| vertex(x, 0))
        .chain((1..4).map(|y| vertex(3, y)))
        .chain((0..3).rev().map(|x| vertex(x, 3)))
        .chain((1..3).rev().map(|y| vertex(0, y)))
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
