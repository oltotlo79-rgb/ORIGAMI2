use ori_domain::{
    CreasePattern, Edge, EdgeId, EdgeKind, FaceId, Paper, Point2, ProjectId, Vertex, VertexId,
};
use ori_topology::{
    EdgeIncidence, FaceExtractionInput, FoldAssignment, HalfEdgeRef, TopologyIssue,
    TopologyIssueKind, TopologyIssueSeverity, TopologySnapshot, analyze_faces,
    extract_faces_strict,
};
use serde::de::DeserializeOwned;

const SOURCE_REVISION: u64 = 73;

#[derive(Clone)]
struct SixVertexFixture {
    namespace: ProjectId,
    paper: Paper,
    pattern: CreasePattern,
    vertices: [VertexId; 6],
    boundary_edges: [EdgeId; 6],
    fold: EdgeId,
}

fn fixed_id<T: DeserializeOwned>(suffix: u64) -> T {
    serde_json::from_str(&format!("\"00000000-0000-0000-0000-{suffix:012x}\""))
        .expect("fixed UUID fixture")
}

fn parsed_id<T: DeserializeOwned>(value: &str) -> T {
    serde_json::from_str(&format!("\"{value}\"")).expect("valid UUID fixture")
}

fn hex_32(value: &str) -> [u8; 32] {
    assert_eq!(
        value.len(),
        64,
        "SHA-256 fixture must contain 64 hex digits"
    );
    let mut bytes = [0_u8; 32];
    for (index, output) in bytes.iter_mut().enumerate() {
        let offset = index * 2;
        *output = u8::from_str_radix(&value[offset..offset + 2], 16)
            .expect("SHA-256 fixture contains only hex digits");
    }
    bytes
}

fn six_vertex_rectangle() -> SixVertexFixture {
    // Collinear subdivisions B and E deliberately exercise weak convexity.
    let vertices = [
        fixed_id(0x101),
        fixed_id(0x102),
        fixed_id(0x103),
        fixed_id(0x104),
        fixed_id(0x105),
        fixed_id(0x106),
    ];
    let boundary_edges = [
        fixed_id(0x201),
        fixed_id(0x202),
        fixed_id(0x203),
        fixed_id(0x204),
        fixed_id(0x205),
        fixed_id(0x206),
    ];
    let fold = fixed_id(0x301);
    let positions = [
        Point2::new(0.0, 0.0),
        Point2::new(2.0, 0.0),
        Point2::new(4.0, 0.0),
        Point2::new(4.0, 4.0),
        Point2::new(2.0, 4.0),
        Point2::new(0.0, 4.0),
    ];
    let vertex_records = vertices
        .iter()
        .zip(positions)
        .map(|(id, position)| Vertex { id: *id, position })
        .collect::<Vec<_>>();
    let mut edges = (0..vertices.len())
        .map(|index| Edge {
            id: boundary_edges[index],
            start: vertices[index],
            end: vertices[(index + 1) % vertices.len()],
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    edges.push(Edge {
        id: fold,
        start: vertices[1],
        end: vertices[4],
        kind: EdgeKind::Mountain,
    });

    SixVertexFixture {
        namespace: fixed_id(1),
        paper: Paper {
            boundary_vertices: vertices.to_vec(),
            ..Paper::default()
        },
        pattern: CreasePattern {
            vertices: vertex_records,
            edges,
        },
        vertices,
        boundary_edges,
        fold,
    }
}

fn strict(fixture: &SixVertexFixture) -> TopologySnapshot {
    extract_faces_strict(FaceExtractionInput {
        identity_namespace: fixture.namespace,
        source_revision: SOURCE_REVISION,
        paper: &fixture.paper,
        pattern: &fixture.pattern,
    })
    .expect("the supported single-fold fixture must extract")
}

fn incidence(snapshot: &TopologySnapshot, edge: EdgeId) -> EdgeIncidence {
    snapshot
        .edge_incidence
        .iter()
        .find_map(|(candidate, incidence)| (*candidate == edge).then_some(*incidence))
        .expect("source edge has an incidence record")
}

#[test]
fn fixed_single_fold_matches_stable_face_and_hinge_golden_vectors() {
    let fixture = six_vertex_rectangle();
    let snapshot = strict(&fixture);
    let west_id: FaceId = parsed_id("491335be-af2f-5668-b4b7-82e65d7d6906");
    let east_id: FaceId = parsed_id("1c1017ca-ef9d-5c7b-8234-72ae79f610a0");
    let west_key = hex_32("ac67b7ad2f5e9d55d710505f672f56df45aa2d5e38612286686af369efe46c57");
    let east_key = hex_32("eb94f43e9db24696e3aec0dd85bd6053bd046819cacd949afddd101f44c1772b");

    assert_eq!(snapshot.source_revision, SOURCE_REVISION);
    assert_eq!(snapshot.faces.len(), 2);
    assert_eq!(snapshot.edge_incidence.len(), 7);
    assert_eq!(snapshot.hinge_adjacency.len(), 1);
    assert!(
        snapshot
            .faces
            .windows(2)
            .all(|faces| faces[0].key < faces[1].key),
        "faces are serialized in canonical FaceKey order"
    );
    assert!(
        snapshot
            .edge_incidence
            .windows(2)
            .all(|edges| edges[0].0.canonical_bytes() < edges[1].0.canonical_bytes()),
        "incidence records are serialized in canonical EdgeId order"
    );

    let west = snapshot
        .faces
        .iter()
        .find(|face| face.id == west_id)
        .expect("west face");
    assert_eq!(west.key.0, west_key);
    assert_eq!(west.area, 8.0);
    assert_eq!(west.outer.signed_double_area, 16.0);
    assert_eq!(
        west.outer.half_edges,
        vec![
            HalfEdgeRef {
                edge: fixture.boundary_edges[0],
                origin: fixture.vertices[0],
                destination: fixture.vertices[1],
            },
            HalfEdgeRef {
                edge: fixture.fold,
                origin: fixture.vertices[1],
                destination: fixture.vertices[4],
            },
            HalfEdgeRef {
                edge: fixture.boundary_edges[4],
                origin: fixture.vertices[4],
                destination: fixture.vertices[5],
            },
            HalfEdgeRef {
                edge: fixture.boundary_edges[5],
                origin: fixture.vertices[5],
                destination: fixture.vertices[0],
            },
        ]
    );

    let east = snapshot
        .faces
        .iter()
        .find(|face| face.id == east_id)
        .expect("east face");
    assert_eq!(east.key.0, east_key);
    assert_eq!(east.area, 8.0);
    assert_eq!(east.outer.signed_double_area, 16.0);
    assert_eq!(
        east.outer.half_edges,
        vec![
            HalfEdgeRef {
                edge: fixture.boundary_edges[1],
                origin: fixture.vertices[1],
                destination: fixture.vertices[2],
            },
            HalfEdgeRef {
                edge: fixture.boundary_edges[2],
                origin: fixture.vertices[2],
                destination: fixture.vertices[3],
            },
            HalfEdgeRef {
                edge: fixture.boundary_edges[3],
                origin: fixture.vertices[3],
                destination: fixture.vertices[4],
            },
            HalfEdgeRef {
                edge: fixture.fold,
                origin: fixture.vertices[4],
                destination: fixture.vertices[1],
            },
        ]
    );

    for edge in [
        fixture.boundary_edges[0],
        fixture.boundary_edges[4],
        fixture.boundary_edges[5],
    ] {
        assert_eq!(
            incidence(&snapshot, edge),
            EdgeIncidence::Boundary { material: west_id }
        );
    }
    for edge in [
        fixture.boundary_edges[1],
        fixture.boundary_edges[2],
        fixture.boundary_edges[3],
    ] {
        assert_eq!(
            incidence(&snapshot, edge),
            EdgeIncidence::Boundary { material: east_id }
        );
    }
    assert_eq!(
        incidence(&snapshot, fixture.fold),
        EdgeIncidence::Hinge {
            left: west_id,
            right: east_id,
            assignment: FoldAssignment::Mountain,
        }
    );
    assert_eq!(snapshot.hinge_adjacency[0].edge, fixture.fold);
    assert_eq!(snapshot.hinge_adjacency[0].first, west_id);
    assert_eq!(snapshot.hinge_adjacency[0].second, east_id);
    assert_eq!(
        snapshot.hinge_adjacency[0].assignment,
        FoldAssignment::Mountain
    );
}

#[test]
fn equivalent_storage_and_direction_transforms_preserve_the_complete_snapshot() {
    let fixture = six_vertex_rectangle();
    let expected = strict(&fixture);

    let mut cyclic = fixture.clone();
    cyclic.paper.boundary_vertices.rotate_left(3);
    assert_eq!(strict(&cyclic), expected, "cyclic boundary start");

    let mut clockwise = fixture.clone();
    clockwise.paper.boundary_vertices.reverse();
    assert_eq!(strict(&clockwise), expected, "clockwise boundary storage");

    let mut reversed_endpoints = fixture.clone();
    for edge in &mut reversed_endpoints.pattern.edges {
        std::mem::swap(&mut edge.start, &mut edge.end);
    }
    assert_eq!(
        strict(&reversed_endpoints),
        expected,
        "source edge endpoint directions"
    );

    let mut reordered = fixture.clone();
    reordered.pattern.vertices.reverse();
    reordered.pattern.edges.reverse();
    assert_eq!(strict(&reordered), expected, "source record order");

    let mut all_transforms = fixture;
    all_transforms.paper.boundary_vertices.rotate_right(2);
    all_transforms.paper.boundary_vertices.reverse();
    all_transforms.pattern.vertices.rotate_left(2);
    all_transforms.pattern.edges.reverse();
    for edge in &mut all_transforms.pattern.edges {
        std::mem::swap(&mut edge.start, &mut edge.end);
    }
    assert_eq!(strict(&all_transforms), expected, "combined transforms");
}

#[test]
fn mountain_to_valley_changes_only_the_hinge_assignment() {
    let fixture = six_vertex_rectangle();
    let mountain = strict(&fixture);
    let mut valley_fixture = fixture.clone();
    valley_fixture
        .pattern
        .edges
        .iter_mut()
        .find(|edge| edge.id == fixture.fold)
        .expect("fold record")
        .kind = EdgeKind::Valley;
    let valley = strict(&valley_fixture);

    let mut expected = mountain.clone();
    for (_, incidence) in &mut expected.edge_incidence {
        if let EdgeIncidence::Hinge { assignment, .. } = incidence {
            *assignment = FoldAssignment::Valley;
        }
    }
    expected.hinge_adjacency[0].assignment = FoldAssignment::Valley;

    assert_eq!(valley, expected);
    assert_eq!(valley.faces, mountain.faces);
}

#[test]
fn malformed_auxiliary_geometry_is_ignored_without_changing_material_faces() {
    let fixture = six_vertex_rectangle();
    let baseline = strict(&fixture);
    let mut malformed = fixture.clone();
    let non_finite_vertex = fixed_id(0x901);
    let missing_vertex = fixed_id(0x902);
    malformed.pattern.vertices.push(Vertex {
        id: non_finite_vertex,
        position: Point2::new(f64::NAN, f64::INFINITY),
    });
    let auxiliary_edges: [EdgeId; 3] = [fixed_id(0x401), fixed_id(0x402), fixed_id(0x403)];
    malformed.pattern.edges.extend([
        // Crosses the admitted fold in its strict interior.
        Edge {
            id: auxiliary_edges[0],
            start: fixture.vertices[0],
            end: fixture.vertices[3],
            kind: EdgeKind::Auxiliary,
        },
        // Coincides with the admitted fold but remains annotation-only.
        Edge {
            id: auxiliary_edges[1],
            start: fixture.vertices[1],
            end: fixture.vertices[4],
            kind: EdgeKind::Auxiliary,
        },
        // References both a non-finite vertex and a completely missing endpoint.
        Edge {
            id: auxiliary_edges[2],
            start: non_finite_vertex,
            end: missing_vertex,
            kind: EdgeKind::Auxiliary,
        },
    ]);

    let snapshot = strict(&malformed);

    assert_eq!(snapshot.faces, baseline.faces);
    assert_eq!(
        incidence(&snapshot, fixture.fold),
        incidence(&baseline, fixture.fold)
    );
    for edge in auxiliary_edges {
        assert_eq!(incidence(&snapshot, edge), EdgeIncidence::AuxiliaryIgnored);
    }
}

#[test]
fn duplicate_ids_remain_fatal_even_when_the_duplicate_record_is_auxiliary_only() {
    let fixture = six_vertex_rectangle();
    let mut duplicate_vertex = fixture.clone();
    duplicate_vertex.pattern.vertices.push(Vertex {
        id: fixture.vertices[0],
        position: Point2::new(f64::NAN, f64::INFINITY),
    });
    let vertex_report = analyze_faces(FaceExtractionInput {
        identity_namespace: fixture.namespace,
        source_revision: SOURCE_REVISION,
        paper: &duplicate_vertex.paper,
        pattern: &duplicate_vertex.pattern,
    });
    assert_eq!(
        vertex_report.issues,
        vec![TopologyIssue {
            severity: TopologyIssueSeverity::Fatal,
            kind: TopologyIssueKind::DuplicateVertexId {
                vertex: fixture.vertices[0],
            },
        }]
    );

    let mut duplicate_edge = fixture.clone();
    duplicate_edge.pattern.edges.push(Edge {
        id: fixture.fold,
        start: fixed_id(0x999),
        end: fixed_id(0x998),
        kind: EdgeKind::Auxiliary,
    });
    let edge_report = analyze_faces(FaceExtractionInput {
        identity_namespace: fixture.namespace,
        source_revision: SOURCE_REVISION,
        paper: &duplicate_edge.paper,
        pattern: &duplicate_edge.pattern,
    });
    assert_eq!(
        edge_report.issues,
        vec![TopologyIssue {
            severity: TopologyIssueSeverity::Fatal,
            kind: TopologyIssueKind::DuplicateEdgeId { edge: fixture.fold },
        }]
    );
}

#[test]
fn multiple_valid_fold_chords_are_rejected_in_canonical_edge_order() {
    let fixture = six_vertex_rectangle();
    let mut multiple = fixture.clone();
    let earlier_fold = fixed_id(0x300);
    multiple.pattern.edges.insert(
        0,
        Edge {
            id: earlier_fold,
            start: fixture.vertices[0],
            end: fixture.vertices[4],
            kind: EdgeKind::Valley,
        },
    );
    multiple.pattern.edges.reverse();

    let report = analyze_faces(FaceExtractionInput {
        identity_namespace: fixture.namespace,
        source_revision: SOURCE_REVISION,
        paper: &multiple.paper,
        pattern: &multiple.pattern,
    });

    assert!(report.snapshot.is_none());
    assert_eq!(
        report.issues,
        vec![TopologyIssue {
            severity: TopologyIssueSeverity::BlocksSimulation,
            kind: TopologyIssueKind::TooManyActiveFoldEdges {
                edges: vec![earlier_fold, fixture.fold],
            },
        }]
    );
}

#[test]
fn non_convex_sheet_is_blocked_with_a_stable_reflex_vertex_diagnostic() {
    let vertex_ids = [
        fixed_id(0x101),
        fixed_id(0x102),
        fixed_id(0x103),
        fixed_id(0x104),
        fixed_id(0x105),
        fixed_id(0x106),
    ];
    let edge_ids = [
        fixed_id(0x201),
        fixed_id(0x202),
        fixed_id(0x203),
        fixed_id(0x204),
        fixed_id(0x205),
        fixed_id(0x206),
    ];
    let fold = fixed_id(0x301);
    let positions = [
        Point2::new(0.0, 0.0),
        Point2::new(4.0, 0.0),
        Point2::new(4.0, 4.0),
        Point2::new(3.0, 2.0),
        Point2::new(1.0, 4.0),
        Point2::new(0.0, 4.0),
    ];
    let vertices = vertex_ids
        .iter()
        .zip(positions)
        .map(|(id, position)| Vertex { id: *id, position })
        .collect::<Vec<_>>();
    let mut edges = (0..vertex_ids.len())
        .map(|index| Edge {
            id: edge_ids[index],
            start: vertex_ids[index],
            end: vertex_ids[(index + 1) % vertex_ids.len()],
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    edges.push(Edge {
        id: fold,
        start: vertex_ids[1],
        end: vertex_ids[4],
        kind: EdgeKind::Mountain,
    });
    let paper = Paper {
        boundary_vertices: vertex_ids.to_vec(),
        ..Paper::default()
    };
    let pattern = CreasePattern { vertices, edges };

    let report = analyze_faces(FaceExtractionInput {
        identity_namespace: fixed_id(1),
        source_revision: SOURCE_REVISION,
        paper: &paper,
        pattern: &pattern,
    });

    assert!(report.snapshot.is_none());
    assert_eq!(
        report.issues,
        vec![TopologyIssue {
            severity: TopologyIssueSeverity::BlocksSimulation,
            kind: TopologyIssueKind::UnsupportedNonConvexFoldSheet {
                edge: fold,
                vertex: vertex_ids[3],
            },
        }]
    );
}

#[test]
fn fold_endpoint_inside_the_sheet_is_blocked_before_topology_construction() {
    let fixture = six_vertex_rectangle();
    let mut interior = fixture.clone();
    let interior_vertex = fixed_id(0x150);
    interior.pattern.vertices.push(Vertex {
        id: interior_vertex,
        position: Point2::new(2.0, 2.0),
    });
    let fold = interior
        .pattern
        .edges
        .iter_mut()
        .find(|edge| edge.id == fixture.fold)
        .expect("fold record");
    fold.start = interior_vertex;
    fold.end = fixture.vertices[4];

    let report = analyze_faces(FaceExtractionInput {
        identity_namespace: fixture.namespace,
        source_revision: SOURCE_REVISION,
        paper: &interior.paper,
        pattern: &interior.pattern,
    });

    assert_eq!(
        report.issues,
        vec![TopologyIssue {
            severity: TopologyIssueSeverity::BlocksSimulation,
            kind: TopologyIssueKind::FoldEndpointNotOnBoundary {
                edge: fixture.fold,
                vertex: interior_vertex,
            },
        }]
    );
}

#[test]
fn deserializing_an_older_snapshot_defaults_the_adjacency_list() {
    let fixture = six_vertex_rectangle();
    let snapshot = strict(&fixture);
    let mut serialized = serde_json::to_value(&snapshot).expect("serialize snapshot");
    serialized
        .as_object_mut()
        .expect("snapshot object")
        .remove("hinge_adjacency");

    let restored: TopologySnapshot =
        serde_json::from_value(serialized).expect("deserialize legacy snapshot");

    assert_eq!(restored.source_revision, snapshot.source_revision);
    assert_eq!(restored.faces, snapshot.faces);
    assert_eq!(restored.edge_incidence, snapshot.edge_incidence);
    assert!(restored.hinge_adjacency.is_empty());
}
