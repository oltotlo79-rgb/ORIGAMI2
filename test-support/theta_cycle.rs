use ori_domain::{
    CreasePattern, Edge, EdgeId, EdgeKind, Paper, Point2, ProjectId, Vertex, VertexId,
};

pub fn theta_shared_hinge_pattern() -> (CreasePattern, Paper, Vec<EdgeId>, Vec<EdgeId>) {
    let namespace: ProjectId =
        serde_json::from_str("\"00000000-0000-4000-b000-000000000003\"").unwrap();
    let points = [
        (-3.0, 0.0),
        (-1.0, -2.0),
        (1.0, -2.0),
        (3.0, 0.0),
        (1.0, 2.0),
        (-1.0, 2.0),
        (-1.0, 0.0),
        (1.0, 0.0),
    ];
    let vertices = points
        .into_iter()
        .enumerate()
        .map(|(index, (x, y))| Vertex {
            id: VertexId::derive_v5(namespace, &[index as u8]),
            position: Point2::new(x, y),
        })
        .collect::<Vec<_>>();
    let boundary = vertices[..6]
        .iter()
        .map(|vertex| vertex.id)
        .collect::<Vec<_>>();
    let mut edges = (0..6)
        .map(|index| Edge {
            id: EdgeId::derive_v5(namespace, &[0x10, index as u8]),
            start: boundary[index],
            end: boundary[(index + 1) % 6],
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    let mut hinges = Vec::new();
    let mut moving = Vec::new();
    for (index, (start, end)) in [(6, 0), (6, 1), (6, 5), (6, 7), (7, 2), (7, 3), (7, 4)]
        .into_iter()
        .enumerate()
    {
        let id = EdgeId::derive_v5(namespace, &[0x20, index as u8]);
        let is_moving = matches!(index, 0 | 3 | 5);
        hinges.push(id);
        if is_moving {
            moving.push(id);
        }
        edges.push(Edge {
            id,
            start: vertices[start].id,
            end: vertices[end].id,
            kind: if is_moving {
                EdgeKind::Mountain
            } else {
                EdgeKind::Valley
            },
        });
    }
    (
        CreasePattern { vertices, edges },
        Paper {
            boundary_vertices: boundary,
            ..Paper::default()
        },
        hinges,
        moving,
    )
}
