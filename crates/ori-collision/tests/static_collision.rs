use ori_collision::{
    CENTERED_MID_SURFACE_THICKNESS_MODEL_V1, NATIVE_STATIC_COLLISION_GEOMETRY_PROOF_V1,
    NativeStaticCollisionGeometryProof, StaticCollisionError, StaticCollisionLimits,
    TOPOLOGY_CONTACT_POLICY_V2, prove_static_collision_geometry,
};
use ori_domain::{
    CreasePattern, Edge, EdgeId, EdgeKind, Paper, Point2, ProjectId, Vertex, VertexId,
};
use ori_kinematics::{
    CanonicalHingeAngles, HingeAngle, MATERIAL_TREE_KINEMATICS_MODEL_ID,
    MaterialTreeKinematicsModel, TreeKinematicsLimits,
};
use ori_topology::{FaceExtractionInput, TopologySnapshot, analyze_faces};

struct Fixture {
    pattern: CreasePattern,
    paper: Paper,
    topology: TopologySnapshot,
    hinge: Option<EdgeId>,
}

fn vertex_id(index: u64) -> VertexId {
    serde_json::from_str(&format!("\"00000000-0000-4000-8000-{index:012x}\""))
        .expect("fixed vertex id")
}

fn edge_id(index: u64) -> EdgeId {
    serde_json::from_str(&format!("\"00000000-0000-4000-9000-{index:012x}\""))
        .expect("fixed edge id")
}

fn project_id() -> ProjectId {
    serde_json::from_str("\"00000000-0000-4000-b000-000000000001\"").expect("fixed project id")
}

fn vertex(index: u64, x: f64, y: f64) -> Vertex {
    Vertex {
        id: vertex_id(index),
        position: Point2::new(x, y),
    }
}

fn edge(index: u64, start: VertexId, end: VertexId, kind: EdgeKind) -> Edge {
    Edge {
        id: edge_id(index),
        start,
        end,
        kind,
    }
}

fn fixture(with_hinge: bool) -> Fixture {
    let vertices = vec![
        vertex(1, 0.0, 0.0),
        vertex(2, 5.0, 0.0),
        vertex(3, 10.0, 0.0),
        vertex(4, 10.0, 10.0),
        vertex(5, 5.0, 10.0),
        vertex(6, 0.0, 10.0),
    ];
    let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
    let mut edges = (0..boundary.len())
        .map(|index| {
            edge(
                index as u64 + 1,
                boundary[index],
                boundary[(index + 1) % boundary.len()],
                EdgeKind::Boundary,
            )
        })
        .collect::<Vec<_>>();
    let hinge = with_hinge.then(|| {
        let hinge = edge(7, boundary[1], boundary[4], EdgeKind::Mountain);
        edges.push(hinge.clone());
        hinge.id
    });
    let pattern = CreasePattern { vertices, edges };
    let paper = Paper {
        boundary_vertices: boundary,
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
        topology: report.snapshot.expect("fixture topology"),
        hinge,
    }
}

fn model(fixture: &Fixture) -> MaterialTreeKinematicsModel {
    MaterialTreeKinematicsModel::prepare(
        &fixture.pattern,
        &fixture.paper,
        &fixture.topology,
        TreeKinematicsLimits::default(),
    )
    .expect("material model")
}

fn no_angles() -> CanonicalHingeAngles {
    CanonicalHingeAngles::new(Vec::new()).expect("empty canonical angles")
}

fn assert_error(
    result: Result<NativeStaticCollisionGeometryProof, StaticCollisionError>,
    expected: StaticCollisionError,
) {
    match result {
        Ok(_) => panic!("unexpected static collision geometry proof"),
        Err(actual) => assert_eq!(actual, expected),
    }
}

#[test]
fn one_material_face_has_a_complete_zero_pair_proof_at_all_thicknesses() {
    let fixture = fixture(false);
    let model = model(&fixture);

    for thickness in [-0.0, 0.0, 0.1, 3.0] {
        let pose = model.solve(None, &no_angles()).expect("planar pose");
        let proof = prove_static_collision_geometry(
            &model,
            &pose,
            thickness,
            StaticCollisionLimits::default(),
        )
        .expect("zero-pair proof");

        assert!(proof.is_for_geometry(&model, &pose, thickness));
        assert_eq!(proof.proof_id(), NATIVE_STATIC_COLLISION_GEOMETRY_PROOF_V1);
        assert_eq!(proof.policy_id(), TOPOLOGY_CONTACT_POLICY_V2);
        assert_eq!(
            proof.kinematics_model_id(),
            MATERIAL_TREE_KINEMATICS_MODEL_ID
        );
        assert_eq!(
            proof.thickness_model_id(),
            CENTERED_MID_SURFACE_THICKNESS_MODEL_V1
        );
        assert_eq!(proof.paper_thickness_mm().to_bits(), thickness.to_bits());
        assert_eq!(proof.paper_thickness_bits(), thickness.to_bits());
        if thickness == 0.0 {
            let opposite_zero = f64::from_bits(thickness.to_bits() ^ (1_u64 << 63));
            assert!(!proof.is_for_geometry(&model, &pose, opposite_zero));
        }
        assert_eq!(proof.face_count(), 1);
        assert_eq!(proof.expected_unordered_face_pairs(), 0);
        assert_eq!(proof.analyzed_unordered_face_pairs(), 0);
    }
}

#[test]
fn proof_identity_and_exact_pose_instance_reject_same_angle_aba() {
    let fixture = fixture(false);
    let model = model(&fixture);
    let first_pose = model.solve(None, &no_angles()).expect("first pose");
    let first = prove_static_collision_geometry(
        &model,
        &first_pose,
        fixture.paper.thickness_mm,
        StaticCollisionLimits::default(),
    )
    .expect("first proof");

    let cloned = first.clone();
    assert!(first.same_proof(&cloned));
    assert!(cloned.is_for_geometry(&model, &first_pose, fixture.paper.thickness_mm));

    let second_pose = model
        .solve(None, &no_angles())
        .expect("same-angle ABA pose");
    assert!(!first.is_for_geometry(&model, &second_pose, fixture.paper.thickness_mm));
    let second = prove_static_collision_geometry(
        &model,
        &second_pose,
        fixture.paper.thickness_mm,
        StaticCollisionLimits::default(),
    )
    .expect("second proof");
    assert!(!first.same_proof(&second));
    assert!(!second.is_for_geometry(&model, &first_pose, fixture.paper.thickness_mm));
}

#[test]
fn equal_but_separately_prepared_model_cannot_certify_a_foreign_pose() {
    let fixture = fixture(false);
    let first_model = model(&fixture);
    let second_model = model(&fixture);
    let pose = first_model.solve(None, &no_angles()).expect("first pose");

    assert_error(
        prove_static_collision_geometry(
            &second_model,
            &pose,
            fixture.paper.thickness_mm,
            StaticCollisionLimits::default(),
        ),
        StaticCollisionError::PoseIssuerMismatch,
    );
}

#[test]
fn invalid_thickness_and_resource_exhaustion_never_issue_a_proof() {
    let fixture = fixture(false);
    let model = model(&fixture);
    let pose = model.solve(None, &no_angles()).expect("planar pose");

    for thickness in [-f64::EPSILON, f64::NAN, f64::INFINITY] {
        assert_error(
            prove_static_collision_geometry(
                &model,
                &pose,
                thickness,
                StaticCollisionLimits::default(),
            ),
            StaticCollisionError::InvalidPaperThickness,
        );
    }
    assert_error(
        prove_static_collision_geometry(
            &model,
            &pose,
            fixture.paper.thickness_mm,
            StaticCollisionLimits {
                max_faces: 0,
                max_unordered_face_pairs: 0,
            },
        ),
        StaticCollisionError::ResourceLimitExceeded,
    );
}

#[test]
fn multi_face_pose_is_blocking_until_every_pair_has_native_evidence() {
    let fixture = fixture(true);
    let model = model(&fixture);
    let hinge = fixture.hinge.expect("fixture hinge");
    let angles = CanonicalHingeAngles::new(vec![
        HingeAngle::new(hinge, 90.0).expect("valid hinge angle"),
    ])
    .expect("canonical angle");
    let pose = model
        .solve(Some(model.face_ids()[0]), &angles)
        .expect("folded pose");

    for thickness in [0.0, 0.1, 3.0] {
        assert_error(
            prove_static_collision_geometry(
                &model,
                &pose,
                thickness,
                StaticCollisionLimits::default(),
            ),
            StaticCollisionError::PairEvidenceUnavailable {
                expected_unordered_face_pairs: 1,
            },
        );
    }
    assert_error(
        prove_static_collision_geometry(
            &model,
            &pose,
            fixture.paper.thickness_mm,
            StaticCollisionLimits {
                max_faces: usize::MAX,
                max_unordered_face_pairs: 0,
            },
        ),
        StaticCollisionError::ResourceLimitExceeded,
    );
}
