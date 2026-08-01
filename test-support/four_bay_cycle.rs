use ori_domain::{
    CreasePattern, Edge, EdgeId, EdgeKind, Paper, Point2, ProjectId, Vertex, VertexId,
};

pub fn four_bay_rational_cycle_pattern() -> (CreasePattern, Paper, Vec<EdgeId>) {
    rational_cycle_bay_pattern(4)
}

/// Four spatially separated six-sector bays sharing one convex central
/// material face.  In every bay the two returned spokes are exact opposite
/// rays; all other spokes may remain flat, so each canonical edge block is
/// the narrow radial-bifold family used by the positive-thickness continuous
/// theorem.
///
/// The six rational directions satisfy Kawasaki's alternating-sector
/// equality (45 + 45 + 90 degrees on both sides) and the 4M/2V assignment
/// satisfies Maekawa.  The bay centres are the corners of a forty-unit square,
/// while every exclusive bay face stays within sqrt(2) of its centre.  The
/// skipped ninety-degree sector of each fan faces the square interior, making
/// the shared face convex and admissible to native flat-layer analysis.
#[allow(dead_code)]
pub fn four_bay_opposite_bifold_pattern() -> (CreasePattern, Paper, Vec<EdgeId>) {
    opposite_bifold_corner_pattern(4)
}

/// Five separated radial-bifold bays sharing one convex central material
/// face. It reuses the four-bay fixture's first four centres and identifiers;
/// the fifth extends the paper boundary on the west, while the two adjacent
/// fan directions expose the required 135-degree pentagon corners. Boundary
/// and non-moving rays remain at least one unit long. Only the authenticated
/// moving pair at those two corners is shorter (`sqrt(5) / 4`), keeping it
/// between five and six paper thicknesses for the 0.1 mm proof fixture. Each
/// bay assigns the exact opposite moving pair Valley and all four stationary
/// rays Mountain, the layer-consistent 4M/2V Maekawa orientation.
#[allow(dead_code)]
pub fn five_bay_opposite_bifold_pattern() -> (CreasePattern, Paper, Vec<EdgeId>) {
    opposite_bifold_corner_pattern(5)
}

/// Six separated radial-bifold bays sharing one convex central material face.
/// The first five bay identities and coordinates are exactly those of
/// [`five_bay_opposite_bifold_pattern`]. The sixth bay mirrors the west bay on
/// the east, while the four square corners expose the corresponding
/// 135-degree central sectors. As in the five-bay fixture, the short opposite
/// pair at each 135-degree corner is Valley and every other ray is Mountain.
#[allow(dead_code)]
pub fn six_bay_opposite_bifold_pattern() -> (CreasePattern, Paper, Vec<EdgeId>) {
    opposite_bifold_corner_pattern(6)
}

/// Seven separated radial-bifold bays sharing one convex central material
/// face with seven strict corners. The first six bay identities and coordinates
/// are exactly those of [`six_bay_opposite_bifold_pattern`]. The seventh bay is
/// a north insertion between the northeast and northwest bays. All seven bays
/// retain three exact opposite ray pairs and the same 4M/2V assignment.
#[allow(dead_code)]
pub fn seven_bay_opposite_bifold_pattern() -> (CreasePattern, Paper, Vec<EdgeId>) {
    opposite_bifold_corner_pattern(7)
}

#[allow(dead_code)]
pub fn two_bay_rational_cycle_pattern() -> (CreasePattern, Paper, Vec<EdgeId>) {
    rational_cycle_bay_pattern(2)
}

#[allow(dead_code)]
pub fn three_bay_rational_cycle_pattern() -> (CreasePattern, Paper, Vec<EdgeId>) {
    rational_cycle_bay_pattern(3)
}

pub fn eight_bay_rational_cycle_pattern() -> (CreasePattern, Paper, Vec<EdgeId>) {
    rational_cycle_bay_pattern(8)
}

pub fn sixteen_bay_rational_cycle_pattern() -> (CreasePattern, Paper, Vec<EdgeId>) {
    rational_cycle_bay_pattern(16)
}

#[allow(dead_code)]
pub fn thirty_two_bay_rational_cycle_pattern() -> (CreasePattern, Paper, Vec<EdgeId>) {
    rational_cycle_bay_pattern(32)
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
    for (group, (p, q, leg)) in triples.into_iter().cycle().take(group_count).enumerate() {
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

fn opposite_bifold_corner_pattern(group_count: usize) -> (CreasePattern, Paper, Vec<EdgeId>) {
    assert!(matches!(group_count, 4 | 5 | 6 | 7));
    let namespace: ProjectId =
        serde_json::from_str("\"00000000-0000-4000-b000-000000000006\"").unwrap();
    let mut vertices = Vec::new();
    let mut boundary = Vec::new();
    let mut hinge_endpoints = Vec::new();
    let mut centers = Vec::new();
    let directions = [
        [
            (0.0, 1.0),
            (-1.0, 1.0),
            (-1.0, 0.0),
            (0.0, -1.0),
            (1.0, -1.0),
            (1.0, 0.0),
        ],
        [
            (-1.0, 0.0),
            (-1.0, -1.0),
            (0.0, -1.0),
            (1.0, 0.0),
            (1.0, 1.0),
            (0.0, 1.0),
        ],
        [
            (0.0, -1.0),
            (1.0, -1.0),
            (1.0, 0.0),
            (0.0, 1.0),
            (-1.0, 1.0),
            (-1.0, 0.0),
        ],
        [
            (1.0, 0.0),
            (1.0, 1.0),
            (0.0, 1.0),
            (-1.0, 0.0),
            (-1.0, -1.0),
            (0.0, -1.0),
        ],
        [
            (1.0, 1.0),
            (0.0, 1.0),
            (-1.0, 1.0),
            (-1.0, -1.0),
            (0.0, -1.0),
            (1.0, -1.0),
        ],
        [
            (-1.0, -1.0),
            (0.0, -1.0),
            (1.0, -1.0),
            (1.0, 1.0),
            (0.0, 1.0),
            (-1.0, 1.0),
        ],
        [
            (1.0, -0.5),
            (0.546875, 0.0),
            (1.0, 0.5),
            (-1.0, 0.5),
            (-0.546875, 0.0),
            (-1.0, -0.5),
        ],
    ];
    // A convex pentagonal shared face needs the two west square corners to
    // expose a 135-degree interior sector.  The boundary rays and the other
    // fan rays retain unit-or-greater length.  Only the supporting-line inner
    // opposite pair is kept inside the theorem's six-thickness vertex
    // corridor (sqrt(0.3125) at 0.1 mm thickness).
    let five_first_directions = [
        (-1.0, 1.0),
        (-0.5, 0.25),
        (-1.0, 0.0),
        (1.0, -1.0),
        (0.5, -0.25),
        (1.0, 0.0),
    ];
    let five_fourth_directions = [
        (1.0, 0.0),
        (0.5, 0.25),
        (1.0, 1.0),
        (-1.0, 0.0),
        (-0.5, -0.25),
        (-1.0, -1.0),
    ];
    let six_second_directions = [
        (-1.0, 0.0),
        (-0.5, -0.25),
        (-1.0, -1.0),
        (1.0, 0.0),
        (0.5, 0.25),
        (1.0, 1.0),
    ];
    let six_third_directions = [
        (1.0, -1.0),
        (0.5, -0.25),
        (1.0, 0.0),
        (-1.0, 1.0),
        (-0.5, 0.25),
        (-1.0, 0.0),
    ];
    // In the seven-bay polygon, the new north corner lies at (0, 30). The
    // neighboring northeast and northwest fans point their skipped sector at
    // the exact vectors to that corner. The short moving rays use the 21-28-35
    // triple scaled by 1/64, so their length is exactly 35/64 mm.
    let seven_third_directions = [
        (1.0, -1.0),
        (0.4375, -0.328125),
        (1.0, -0.5),
        (-1.0, 1.0),
        (-0.4375, 0.328125),
        (-1.0, 0.5),
    ];
    let seven_fourth_directions = [
        (1.0, 0.5),
        (0.4375, 0.328125),
        (1.0, 1.0),
        (-1.0, -0.5),
        (-0.4375, -0.328125),
        (-1.0, -1.0),
    ];
    for (group, ((center_x, center_y), default_directions)) in [
        (-20.0, -20.0),
        (20.0, -20.0),
        (20.0, 20.0),
        (-20.0, 20.0),
        (-40.0, 0.0),
        (40.0, 0.0),
        (0.0, 30.0),
    ]
    .into_iter()
    .zip(directions)
    .take(group_count)
    .enumerate()
    {
        let group_directions = match (group_count, group) {
            (5, 0) => five_first_directions,
            (5, 3) => five_fourth_directions,
            (6, 0) => five_first_directions,
            (6, 1) => six_second_directions,
            (6, 2) => six_third_directions,
            (6, 3) => five_fourth_directions,
            (7, 0) => five_first_directions,
            (7, 1) => six_second_directions,
            (7, 2) => seven_third_directions,
            (7, 3) => seven_fourth_directions,
            _ => default_directions,
        };
        let center = Vertex {
            id: VertexId::derive_v5(namespace, &[0x10, group as u8]),
            position: Point2::new(center_x, center_y),
        };
        centers.push(center.id);
        vertices.push(center);
        // Counter-clockwise exterior fan walk. The first four groups rotate
        // ninety degrees around the square; the optional fifth extends the
        // paper boundary west, the sixth mirrors that bay east, and the
        // seventh inserts a shallow north corner. All three ray pairs are
        // exact opposites. The skipped ray-five-to-ray-zero sector belongs to
        // the common articulation face. Every non-right convex corner returns
        // pair one/four as moving; each 90-degree bay returns pair zero/three.
        for (local, (x, y)) in group_directions.into_iter().enumerate() {
            let vertex = Vertex {
                id: VertexId::derive_v5(namespace, &[0x20, group as u8, local as u8]),
                position: Point2::new(center_x + x, center_y + y),
            };
            boundary.push(vertex.id);
            hinge_endpoints.push(vertex.id);
            vertices.push(vertex);
        }
    }

    if group_count == 6 {
        // Keep the first five group identities stable while walking the new
        // east bay between the southeast and northeast groups on the outer
        // paper boundary.
        let original = boundary;
        boundary = Vec::with_capacity(original.len());
        for group in [0, 1, 5, 2, 3, 4] {
            boundary.extend_from_slice(&original[group * 6..(group + 1) * 6]);
        }
    } else if group_count == 7 {
        // Insert the north bay between the existing northeast and northwest
        // identities without changing any of the first six vertex or hinge
        // identifiers.
        let original = boundary;
        boundary = Vec::with_capacity(original.len());
        for group in [0, 1, 5, 2, 6, 3, 4] {
            boundary.extend_from_slice(&original[group * 6..(group + 1) * 6]);
        }
    }

    let mut edges = (0..boundary.len())
        .map(|index| Edge {
            id: EdgeId::derive_v5(namespace, &[0x50, index as u8]),
            start: boundary[index],
            end: boundary[(index + 1) % boundary.len()],
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    let hinges = (0..group_count * 6)
        .map(|index| EdgeId::derive_v5(namespace, &[0x60, index as u8]))
        .collect::<Vec<_>>();
    edges.extend((0..group_count * 6).map(|index| {
        let group = index / 6;
        let local = index % 6;
        let kind = if matches!(group_count, 5 | 6 | 7) {
            let moving_pair = if matches!(
                (group_count, group),
                (5, 0 | 3) | (6, 0..=3) | (7, 0..=3 | 6)
            ) {
                [1, 4]
            } else {
                [0, 3]
            };
            if moving_pair.contains(&local) {
                EdgeKind::Valley
            } else {
                EdgeKind::Mountain
            }
        } else if matches!(local, 0 | 1 | 3 | 4) {
            EdgeKind::Mountain
        } else {
            EdgeKind::Valley
        };
        Edge {
            id: hinges[index],
            start: centers[group],
            end: hinge_endpoints[index],
            kind,
        }
    }));
    let moving = (0..group_count)
        .flat_map(|group| {
            let (first, opposite) = if matches!(
                (group_count, group),
                (5, 0 | 3) | (6, 0..=3) | (7, 0..=3 | 6)
            ) {
                (1, 4)
            } else {
                (0, 3)
            };
            [hinges[group * 6 + first], hinges[group * 6 + opposite]]
        })
        .collect();
    let paper = Paper {
        boundary_vertices: boundary,
        ..Paper::default()
    };
    (CreasePattern { vertices, edges }, paper, moving)
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

#[allow(dead_code)]
pub fn sixteen_bay_rational_cycle_pattern_with_reversed_hinges()
-> (CreasePattern, Paper, Vec<EdgeId>) {
    let (mut pattern, paper, hinges) = sixteen_bay_rational_cycle_pattern();
    let boundary_edge_count = paper.boundary_vertices.len();
    pattern.edges[boundary_edge_count..].reverse();
    (pattern, paper, hinges)
}
