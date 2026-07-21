use ori_domain::{
    CreasePattern, Edge, EdgeId, EdgeKind, Paper, Point2, ProjectId, Vertex, VertexId,
};

pub fn four_bay_rational_cycle_pattern() -> (CreasePattern, Paper, Vec<EdgeId>) {
    rational_cycle_bay_pattern(4)
}

pub fn eight_bay_rational_cycle_pattern() -> (CreasePattern, Paper, Vec<EdgeId>) {
    rational_cycle_bay_pattern(8)
}

fn rational_cycle_bay_pattern(group_count: usize) -> (CreasePattern, Paper, Vec<EdgeId>) {
    let namespace: ProjectId =
        serde_json::from_str("\"00000000-0000-4000-b000-000000000002\"").unwrap();
    let triples = [
        (3.0, 5.0, 4.0),
        (5.0, 13.0, 12.0),
        (8.0, 17.0, 15.0),
        (7.0, 25.0, 24.0),
        (3.0, 5.0, 4.0),
        (5.0, 13.0, 12.0),
        (8.0, 17.0, 15.0),
        (7.0, 25.0, 24.0),
    ];
    let mut vertices = Vec::new();
    let mut boundary = Vec::new();
    let mut hinge_endpoints = Vec::new();
    let mut centers = Vec::new();
    let first_center_y = -(group_count.saturating_sub(1) as f64) * 20.0;
    for (group, (p, q, leg)) in triples.into_iter().take(group_count).enumerate() {
        let center_y = first_center_y + group as f64 * 40.0;
        let center = Vertex {
            id: VertexId::derive_v5(namespace, &[0x10, group as u8]),
            position: Point2::new(0.0, center_y),
        };
        centers.push(center.id);
        vertices.push(center);
        let directions = [
            (1.0, 0.0),
            (-p / q, leg / q),
            ((2.0 * p * p - q * q) / (q * q), -2.0 * p * leg / (q * q)),
            (p / q, -leg / q),
        ];
        for (local, (x, y)) in directions.into_iter().enumerate() {
            let vertex = Vertex {
                id: VertexId::derive_v5(namespace, &[0x20, group as u8, local as u8]),
                position: Point2::new(x, center_y - y),
            };
            boundary.push(vertex.id);
            hinge_endpoints.push(vertex.id);
            vertices.push(vertex);
        }
        let gateway = Vertex {
            id: VertexId::derive_v5(namespace, &[0x30, group as u8]),
            position: Point2::new(4.0, center_y + 4.0),
        };
        boundary.push(gateway.id);
        vertices.push(gateway);
    }
    let outer = (group_count as f64 - 1.0) * 20.0 + 36.0;
    for (index, (x, y)) in [(10.0, outer), (10.0, -outer)].into_iter().enumerate() {
        let vertex = Vertex {
            id: VertexId::derive_v5(namespace, &[0x40, index as u8]),
            position: Point2::new(x, y),
        };
        boundary.push(vertex.id);
        vertices.push(vertex);
    }
    boundary.reverse();
    let mut edges = (0..boundary.len())
        .map(|index| Edge {
            id: EdgeId::derive_v5(namespace, &[0x50, index as u8]),
            start: boundary[index],
            end: boundary[(index + 1) % boundary.len()],
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    let hinges = (0..group_count * 4)
        .map(|index| EdgeId::derive_v5(namespace, &[0x60, index as u8]))
        .collect::<Vec<_>>();
    edges.extend((0..group_count * 4).map(|index| Edge {
        id: hinges[index],
        start: centers[index / 4],
        end: hinge_endpoints[index],
        kind: if index % 4 == 3 {
            EdgeKind::Mountain
        } else {
            EdgeKind::Valley
        },
    }));
    let paper = Paper {
        boundary_vertices: boundary,
        ..Paper::default()
    };
    (CreasePattern { vertices, edges }, paper, hinges)
}

#[allow(dead_code)]
pub fn four_bay_rational_cycle_pattern_with_reversed_hinges() -> (CreasePattern, Paper, Vec<EdgeId>)
{
    let (mut pattern, paper, hinges) = four_bay_rational_cycle_pattern();
    let boundary_edge_count = paper.boundary_vertices.len();
    pattern.edges[boundary_edge_count..].reverse();
    (pattern, paper, hinges)
}

#[allow(dead_code)]
pub fn eight_bay_rational_cycle_pattern_with_reversed_hinges() -> (CreasePattern, Paper, Vec<EdgeId>)
{
    let (mut pattern, paper, hinges) = eight_bay_rational_cycle_pattern();
    let boundary_edge_count = paper.boundary_vertices.len();
    pattern.edges[boundary_edge_count..].reverse();
    (pattern, paper, hinges)
}
