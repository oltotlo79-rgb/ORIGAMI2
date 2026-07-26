use super::*;

#[test]
fn edge_intersection_connection_rejects_target_and_generated_id_ambiguity_atomically() {
    let (_, pattern, paper, first, second) = crossing_edges_editor();
    let command = |first_edge, second_edge, new_vertex, first_new_edge, second_new_edge| {
        Command::ConnectEdgeIntersection {
            first_edge,
            second_edge,
            new_vertex,
            first_new_edge,
            second_new_edge,
        }
    };

    let mut editor = EditorState::with_paper(pattern.clone(), paper.clone());
    assert_intersection_rejected(
        &mut editor,
        command(
            first.id,
            first.id,
            VertexId::new(),
            EdgeId::new(),
            EdgeId::new(),
        ),
        CommandError::EdgeIntersectionTargetsNotDistinct,
    );

    let missing = EdgeId::new();
    let mut editor = EditorState::with_paper(pattern.clone(), paper.clone());
    assert_intersection_rejected(
        &mut editor,
        command(
            first.id,
            missing,
            VertexId::new(),
            EdgeId::new(),
            EdgeId::new(),
        ),
        CommandError::EdgeNotFound(missing),
    );

    let mut ambiguous_pattern = pattern.clone();
    ambiguous_pattern.edges.push(second.clone());
    let mut editor = EditorState::with_paper(ambiguous_pattern, paper.clone());
    assert_intersection_rejected(
        &mut editor,
        command(
            first.id,
            second.id,
            VertexId::new(),
            EdgeId::new(),
            EdgeId::new(),
        ),
        CommandError::EdgeIntersectionTargetEdgeIdAmbiguous { edge: second.id },
    );

    let boundary = pattern.edges[0].id;
    let mut editor = EditorState::with_paper(pattern.clone(), paper.clone());
    assert_intersection_rejected(
        &mut editor,
        command(
            boundary,
            first.id,
            VertexId::new(),
            EdgeId::new(),
            EdgeId::new(),
        ),
        CommandError::EdgeIntersectionBoundaryEdge(boundary),
    );

    let duplicate_new_edge = EdgeId::new();
    let mut editor = EditorState::with_paper(pattern.clone(), paper.clone());
    assert_intersection_rejected(
        &mut editor,
        command(
            first.id,
            second.id,
            VertexId::new(),
            duplicate_new_edge,
            duplicate_new_edge,
        ),
        CommandError::EdgeIntersectionNewEdgeIdsNotDistinct,
    );

    let existing_vertex = pattern.vertices[0].id;
    let mut editor = EditorState::with_paper(pattern.clone(), paper.clone());
    assert_intersection_rejected(
        &mut editor,
        command(
            first.id,
            second.id,
            existing_vertex,
            EdgeId::new(),
            EdgeId::new(),
        ),
        CommandError::VertexAlreadyExists(existing_vertex),
    );

    let boundary_reference_only = VertexId::new();
    let mut malformed_paper = paper.clone();
    malformed_paper
        .boundary_vertices
        .push(boundary_reference_only);
    let mut editor = EditorState::with_paper(pattern.clone(), malformed_paper);
    assert_intersection_rejected(
        &mut editor,
        command(
            first.id,
            second.id,
            boundary_reference_only,
            EdgeId::new(),
            EdgeId::new(),
        ),
        CommandError::VertexAlreadyExists(boundary_reference_only),
    );

    let endpoint_reference_only = VertexId::new();
    let mut malformed_pattern = pattern.clone();
    malformed_pattern.edges.push(Edge {
        id: EdgeId::new(),
        start: endpoint_reference_only,
        end: first.start,
        kind: EdgeKind::Auxiliary,
    });
    let mut editor = EditorState::with_paper(malformed_pattern, paper.clone());
    assert_intersection_rejected(
        &mut editor,
        command(
            first.id,
            second.id,
            endpoint_reference_only,
            EdgeId::new(),
            EdgeId::new(),
        ),
        CommandError::VertexAlreadyExists(endpoint_reference_only),
    );

    let existing_edge = pattern.edges[1].id;
    let mut editor = EditorState::with_paper(pattern.clone(), paper.clone());
    assert_intersection_rejected(
        &mut editor,
        command(
            first.id,
            second.id,
            VertexId::new(),
            existing_edge,
            EdgeId::new(),
        ),
        CommandError::EdgeAlreadyExists(existing_edge),
    );

    let mut editor = EditorState::with_paper(pattern, paper);
    assert_intersection_rejected(
        &mut editor,
        command(
            first.id,
            second.id,
            VertexId::new(),
            EdgeId::new(),
            existing_edge,
        ),
        CommandError::EdgeAlreadyExists(existing_edge),
    );
}

#[test]
fn edge_intersection_connection_rejects_non_proper_geometry_atomically() {
    let cases = [
        (
            [
                Point2::new(0.0, 0.0),
                Point2::new(2.0, 0.0),
                Point2::new(0.0, 2.0),
                Point2::new(2.0, 2.0),
            ],
            CommandError::EdgeIntersectionNotSinglePoint,
        ),
        (
            [
                Point2::new(0.0, 0.0),
                Point2::new(3.0, 0.0),
                Point2::new(1.0, 0.0),
                Point2::new(4.0, 0.0),
            ],
            CommandError::EdgeIntersectionNotSinglePoint,
        ),
        (
            [
                Point2::new(0.0, 0.0),
                Point2::new(4.0, 0.0),
                Point2::new(2.0, 0.0),
                Point2::new(2.0, 2.0),
            ],
            CommandError::EdgeIntersectionNotProper,
        ),
        (
            [
                Point2::new(0.0, 0.0),
                Point2::new(2.0, 2.0),
                Point2::new(2.0, 2.0),
                Point2::new(4.0, 0.0),
            ],
            CommandError::EdgeIntersectionNotProper,
        ),
    ];
    for (points, expected) in cases {
        let (mut editor, first, second) = two_edge_editor(points);
        assert_intersection_rejected(
            &mut editor,
            Command::ConnectEdgeIntersection {
                first_edge: first.id,
                second_edge: second.id,
                new_vertex: VertexId::new(),
                first_new_edge: EdgeId::new(),
                second_new_edge: EdgeId::new(),
            },
            expected,
        );
    }
}
