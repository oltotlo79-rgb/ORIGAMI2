use ori_collision::{
    CURRENT_POSE_CELL_ORDER_MODEL_ID_V1, CellOrderTransportErrorV1, CellOrderTransportLimitsV1,
    CellOrderTransportResourceV1, NATIVE_CELL_ORDER_TRANSPORT_PROOF_V1, StaticCollisionLimits,
    prove_single_face_cell_order_transport_v1, prove_static_collision_geometry,
    revalidate_single_face_cell_order_transport_v1,
};
use ori_domain::{
    CreasePattern, Edge, EdgeId, EdgeKind, Paper, Point2, ProjectId, Vertex, VertexId,
};
use ori_kinematics::{
    CanonicalHingeAngles, HingeAngle, MaterialTreeKinematicsModel, MaterialTreePose,
    TreeKinematicsLimits,
};
use ori_topology::{FaceExtractionInput, TopologySnapshot, analyze_faces};

struct Fixture {
    pattern: CreasePattern,
    paper: Paper,
    topology: TopologySnapshot,
}

fn vertex_id(index: u64) -> VertexId {
    serde_json::from_str(&format!("\"10000000-0000-4000-8000-{index:012x}\""))
        .expect("fixed vertex ID")
}

fn edge_id(index: u64) -> EdgeId {
    serde_json::from_str(&format!("\"10000000-0000-4000-9000-{index:012x}\""))
        .expect("fixed edge ID")
}

fn project_id() -> ProjectId {
    serde_json::from_str("\"10000000-0000-4000-b000-000000000001\"").expect("fixed project ID")
}

fn fixture(reverse_source_collections: bool) -> Fixture {
    let mut vertices = vec![
        Vertex {
            id: vertex_id(1),
            position: Point2::new(0.0, 0.0),
        },
        Vertex {
            id: vertex_id(2),
            position: Point2::new(400.0, 0.0),
        },
        Vertex {
            id: vertex_id(3),
            position: Point2::new(400.0, 400.0),
        },
        Vertex {
            id: vertex_id(4),
            position: Point2::new(0.0, 400.0),
        },
    ];
    let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
    let mut edges = (0..boundary.len())
        .map(|index| Edge {
            id: edge_id(index as u64 + 1),
            start: boundary[index],
            end: boundary[(index + 1) % boundary.len()],
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    if reverse_source_collections {
        vertices.reverse();
        edges.reverse();
    }
    let pattern = CreasePattern { vertices, edges };
    let paper = Paper {
        boundary_vertices: boundary,
        thickness_mm: 0.1,
        ..Paper::default()
    };
    let report = analyze_faces(FaceExtractionInput {
        identity_namespace: project_id(),
        source_revision: 7,
        paper: &paper,
        pattern: &pattern,
    });
    assert!(report.issues.is_empty(), "{:?}", report.issues);
    Fixture {
        pattern,
        paper,
        topology: report.snapshot.expect("single-face topology"),
    }
}

fn model_and_pose(fixture: &Fixture) -> (MaterialTreeKinematicsModel, MaterialTreePose) {
    let model = MaterialTreeKinematicsModel::prepare(
        &fixture.pattern,
        &fixture.paper,
        &fixture.topology,
        TreeKinematicsLimits::default(),
    )
    .expect("single-face material model");
    let angles = CanonicalHingeAngles::new(Vec::new()).expect("empty canonical angles");
    let pose = model
        .solve(None, &angles)
        .expect("single-face material pose");
    (model, pose)
}

fn hinged_fixture() -> Fixture {
    let mut fixture = fixture(false);
    fixture.pattern.edges.push(Edge {
        id: edge_id(5),
        start: vertex_id(1),
        end: vertex_id(3),
        kind: EdgeKind::Mountain,
    });
    fixture.paper.thickness_mm = 0.0;
    let report = analyze_faces(FaceExtractionInput {
        identity_namespace: project_id(),
        source_revision: 8,
        paper: &fixture.paper,
        pattern: &fixture.pattern,
    });
    assert!(report.issues.is_empty(), "{:?}", report.issues);
    fixture.topology = report.snapshot.expect("shared-hinge topology");
    fixture
}

#[test]
fn single_face_pose_mints_one_revalidated_current_world_cell() {
    let fixture = fixture(false);
    let (model, pose) = model_and_pose(&fixture);
    let collision = prove_static_collision_geometry(
        &model,
        &pose,
        fixture.paper.thickness_mm,
        StaticCollisionLimits::default(),
    )
    .expect("zero-pair static proof");

    let proof = prove_single_face_cell_order_transport_v1(
        &model,
        &pose,
        &collision,
        CellOrderTransportLimitsV1::default(),
    )
    .expect("single-face cell order");

    assert_eq!(proof.proof_id(), NATIVE_CELL_ORDER_TRANSPORT_PROOF_V1);
    assert_eq!(proof.model_id(), CURRENT_POSE_CELL_ORDER_MODEL_ID_V1);
    assert_eq!(proof.material_faces(), model.face_ids());
    assert_eq!(proof.cells().len(), 1);
    let cell = &proof.cells()[0];
    assert_eq!(cell.bottom_to_top_faces(), model.face_ids());
    assert_eq!(cell.cell_face(), model.face_ids()[0]);
    assert_eq!(cell.world_boundary().len(), 4);
    assert!(
        cell.world_boundary().iter().all(|point| {
            point.x().is_finite() && point.y().is_finite() && point.z().is_finite()
        })
    );
    assert_eq!(
        proof.paper_thickness_bits(),
        fixture.paper.thickness_mm.to_bits()
    );
    assert!(proof.is_for_geometry_and_collision(&model, &pose, &collision));
    revalidate_single_face_cell_order_transport_v1(
        &proof,
        &model,
        &pose,
        &collision,
        CellOrderTransportLimitsV1::default(),
    )
    .expect("immutable geometry revalidation");
}

#[test]
fn proof_identity_rejects_same_angle_aba_foreign_issuer_and_collision_replacement() {
    let fixture = fixture(false);
    let (model, first_pose) = model_and_pose(&fixture);
    let first_collision =
        prove_static_collision_geometry(&model, &first_pose, 0.1, StaticCollisionLimits::default())
            .expect("first collision proof");
    let proof = prove_single_face_cell_order_transport_v1(
        &model,
        &first_pose,
        &first_collision,
        CellOrderTransportLimitsV1::default(),
    )
    .expect("first layer proof");
    let cloned = proof.clone();
    assert!(proof.same_proof(&cloned));

    let angles = CanonicalHingeAngles::new(Vec::new()).expect("empty canonical angles");
    let second_pose = model.solve(None, &angles).expect("same-angle ABA pose");
    let second_collision = prove_static_collision_geometry(
        &model,
        &second_pose,
        0.1,
        StaticCollisionLimits::default(),
    )
    .expect("second collision proof");
    assert!(!proof.is_for_geometry_and_collision(&model, &second_pose, &second_collision));
    assert_eq!(
        revalidate_single_face_cell_order_transport_v1(
            &proof,
            &model,
            &second_pose,
            &second_collision,
            CellOrderTransportLimitsV1::default(),
        ),
        Err(CellOrderTransportErrorV1::ProofBindingMismatch)
    );

    let reissued_collision =
        prove_static_collision_geometry(&model, &first_pose, 0.1, StaticCollisionLimits::default())
            .expect("same model, pose and thickness collision proof");
    assert!(first_collision.is_for_geometry(&model, &first_pose, 0.1));
    assert!(reissued_collision.is_for_geometry(&model, &first_pose, 0.1));
    assert!(!first_collision.same_proof(&reissued_collision));
    assert!(!proof.is_for_geometry_and_collision(&model, &first_pose, &reissued_collision));
    assert_eq!(
        revalidate_single_face_cell_order_transport_v1(
            &proof,
            &model,
            &first_pose,
            &reissued_collision,
            CellOrderTransportLimitsV1::default(),
        ),
        Err(CellOrderTransportErrorV1::ProofBindingMismatch)
    );

    let replacement_collision =
        prove_static_collision_geometry(&model, &first_pose, 0.2, StaticCollisionLimits::default())
            .expect("same pose, different thickness proof");
    assert!(!proof.is_for_geometry_and_collision(&model, &first_pose, &replacement_collision));

    let (foreign_model, foreign_pose) = model_and_pose(&fixture);
    let foreign_collision = prove_static_collision_geometry(
        &foreign_model,
        &foreign_pose,
        0.1,
        StaticCollisionLimits::default(),
    )
    .expect("foreign collision proof");
    assert_eq!(
        prove_single_face_cell_order_transport_v1(
            &model,
            &foreign_pose,
            &foreign_collision,
            CellOrderTransportLimitsV1::default(),
        ),
        Err(CellOrderTransportErrorV1::PoseIssuerMismatch)
    );
}

#[test]
fn positive_zero_thickness_single_face_is_bit_exact_and_revalidates() {
    let mut fixture = fixture(false);
    fixture.paper.thickness_mm = 0.0;
    let (model, pose) = model_and_pose(&fixture);
    let collision =
        prove_static_collision_geometry(&model, &pose, 0.0, StaticCollisionLimits::default())
            .expect("positive-zero zero-pair collision proof");
    assert_eq!(collision.paper_thickness_bits(), 0.0_f64.to_bits());
    assert_eq!(collision.face_count(), 1);
    assert_eq!(collision.expected_unordered_face_pairs(), 0);
    assert_eq!(collision.analyzed_unordered_face_pairs(), 0);
    assert_eq!(collision.expected_triangle_pairs(), 0);
    assert_eq!(collision.analyzed_triangle_pairs(), 0);

    let proof = prove_single_face_cell_order_transport_v1(
        &model,
        &pose,
        &collision,
        CellOrderTransportLimitsV1::default(),
    )
    .expect("positive-zero single-face cell order");
    assert_eq!(proof.paper_thickness_bits(), 0.0_f64.to_bits());
    revalidate_single_face_cell_order_transport_v1(
        &proof,
        &model,
        &pose,
        &collision,
        CellOrderTransportLimitsV1::default(),
    )
    .expect("positive-zero immutable geometry revalidation");
}

#[test]
fn canonical_cell_is_independent_of_source_collection_order() {
    let prove = |fixture: &Fixture| {
        let (model, pose) = model_and_pose(fixture);
        let collision = prove_static_collision_geometry(
            &model,
            &pose,
            fixture.paper.thickness_mm,
            StaticCollisionLimits::default(),
        )
        .expect("static proof");
        prove_single_face_cell_order_transport_v1(
            &model,
            &pose,
            &collision,
            CellOrderTransportLimitsV1::default(),
        )
        .expect("cell order")
    };
    let forward = prove(&fixture(false));
    let reversed = prove(&fixture(true));

    assert_eq!(forward.material_faces(), reversed.material_faces());
    assert_eq!(
        world_boundary_bits(forward.cells()[0].world_boundary()),
        world_boundary_bits(reversed.cells()[0].world_boundary())
    );
    assert_eq!(
        forward.cells()[0].bottom_to_top_faces(),
        reversed.cells()[0].bottom_to_top_faces()
    );
}

#[test]
fn every_resource_limit_admits_equality_and_rejects_one_less() {
    let fixture = fixture(false);
    let (model, pose) = model_and_pose(&fixture);
    let collision = prove_static_collision_geometry(
        &model,
        &pose,
        fixture.paper.thickness_mm,
        StaticCollisionLimits::default(),
    )
    .expect("static proof");
    let exact = CellOrderTransportLimitsV1 {
        max_faces: 1,
        max_hinges: 0,
        max_cells: 1,
        max_boundary_vertices_per_cell: 4,
        max_total_boundary_vertices: 4,
        max_total_layer_records: 1,
    };
    prove_single_face_cell_order_transport_v1(&model, &pose, &collision, exact)
        .expect("equality is admitted");

    for (limits, resource, actual, maximum) in [
        (
            CellOrderTransportLimitsV1 {
                max_faces: 0,
                ..exact
            },
            CellOrderTransportResourceV1::Faces,
            1,
            0,
        ),
        (
            CellOrderTransportLimitsV1 {
                max_cells: 0,
                ..exact
            },
            CellOrderTransportResourceV1::Cells,
            1,
            0,
        ),
        (
            CellOrderTransportLimitsV1 {
                max_boundary_vertices_per_cell: 3,
                ..exact
            },
            CellOrderTransportResourceV1::BoundaryVerticesPerCell,
            4,
            3,
        ),
        (
            CellOrderTransportLimitsV1 {
                max_total_boundary_vertices: 3,
                ..exact
            },
            CellOrderTransportResourceV1::TotalBoundaryVertices,
            4,
            3,
        ),
        (
            CellOrderTransportLimitsV1 {
                max_total_layer_records: 0,
                ..exact
            },
            CellOrderTransportResourceV1::LayerRecords,
            1,
            0,
        ),
    ] {
        assert_eq!(
            prove_single_face_cell_order_transport_v1(&model, &pose, &collision, limits),
            Err(CellOrderTransportErrorV1::ResourceLimitExceeded {
                resource,
                actual,
                maximum,
            })
        );
    }
}

#[test]
fn shared_hinge_and_unresolved_multiface_pose_classes_fail_closed() {
    let single = fixture(false);
    let (single_model, single_pose) = model_and_pose(&single);
    let unrelated_collision = prove_static_collision_geometry(
        &single_model,
        &single_pose,
        single.paper.thickness_mm,
        StaticCollisionLimits::default(),
    )
    .expect("single-face collision proof");

    let hinged = hinged_fixture();
    let hinged_model = MaterialTreeKinematicsModel::prepare(
        &hinged.pattern,
        &hinged.paper,
        &hinged.topology,
        TreeKinematicsLimits::default(),
    )
    .expect("hinged material model");
    assert_eq!(hinged_model.face_ids().len(), 2);
    assert_eq!(hinged_model.hinges().len(), 1);
    let hinge = hinged_model.hinges()[0].edge();
    for angle in [90.0, 180.0] {
        let angles = CanonicalHingeAngles::new(vec![
            HingeAngle::new(hinge, angle).expect("finite hinge angle"),
        ])
        .expect("canonical hinge angles");
        let pose = hinged_model
            .solve(Some(hinged_model.face_ids()[0]), &angles)
            .expect("hinged pose");

        assert!(
            prove_static_collision_geometry(
                &hinged_model,
                &pose,
                hinged.paper.thickness_mm,
                StaticCollisionLimits::default(),
            )
            .is_err(),
            "a shared hinge has no complete public static proof at {angle} degrees"
        );
        assert_eq!(
            prove_single_face_cell_order_transport_v1(
                &hinged_model,
                &pose,
                &unrelated_collision,
                CellOrderTransportLimitsV1::default(),
            ),
            Err(CellOrderTransportErrorV1::UnsupportedPoseClass {
                faces: 2,
                hinges: 1,
            })
        );
    }
}

fn world_boundary_bits(points: &[ori_kinematics::Point3]) -> Vec<[u64; 3]> {
    points
        .iter()
        .map(|point| {
            [
                point.x().to_bits(),
                point.y().to_bits(),
                point.z().to_bits(),
            ]
        })
        .collect()
}
