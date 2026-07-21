use ori_collision::{
    CENTERED_MID_SURFACE_THICKNESS_MODEL_V1, IntersectionEvidenceV2,
    NATIVE_STATIC_COLLISION_GEOMETRY_PROOF_V1, NativeStaticCollisionGeometryProof,
    StaticCollisionError, StaticCollisionLimits, StaticCollisionPairDisposition,
    TOPOLOGY_CONTACT_POLICY_V2, TopologyContactDecision, TopologyRelation,
    classify_runtime_topology_contact_v2, classify_static_collision_pair_disposition,
    diagnose_static_collision_geometry, prove_static_collision_geometry,
};
use ori_domain::{
    CreasePattern, Edge, EdgeId, EdgeKind, FaceId, Paper, Point2, ProjectId, Vertex, VertexId,
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

struct TwoHingeFixture {
    model: MaterialTreeKinematicsModel,
    hinges: [EdgeId; 2],
}

struct TriangleFanFixture {
    model: MaterialTreeKinematicsModel,
    hinges: Vec<EdgeId>,
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
    project_id_variant(1)
}

fn project_id_variant(index: u64) -> ProjectId {
    serde_json::from_str(&format!("\"00000000-0000-4000-b000-{index:012x}\""))
        .expect("fixed project id")
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

fn two_hinge_fixture(
    source_revision: u64,
    coordinates: &[(f64, f64)],
    folds: &[(usize, usize, EdgeKind); 2],
    reverse_source_collections: bool,
) -> TwoHingeFixture {
    let mut vertices = coordinates
        .iter()
        .enumerate()
        .map(|(index, &(x, y))| vertex(index as u64 + 1, x, y))
        .collect::<Vec<_>>();
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
    let hinges: [Edge; 2] = std::array::from_fn(|index| {
        let (start, end, kind) = folds[index];
        edge(
            boundary.len() as u64 + index as u64 + 1,
            boundary[start],
            boundary[end],
            kind,
        )
    });
    edges.extend(hinges.iter().cloned());
    if reverse_source_collections {
        vertices.reverse();
        edges.reverse();
    }
    let pattern = CreasePattern { vertices, edges };
    let paper = Paper {
        boundary_vertices: boundary,
        ..Paper::default()
    };
    let report = analyze_faces(FaceExtractionInput {
        identity_namespace: project_id(),
        source_revision,
        paper: &paper,
        pattern: &pattern,
    });
    assert!(report.issues.is_empty(), "{:?}", report.issues);
    let topology = report.snapshot.expect("two-hinge topology");
    let model = MaterialTreeKinematicsModel::prepare(
        &pattern,
        &paper,
        &topology,
        TreeKinematicsLimits::default(),
    )
    .expect("two-hinge material model");
    assert_eq!(model.face_ids().len(), 3);
    assert_eq!(
        model
            .hinges()
            .iter()
            .map(|hinge| hinge.edge())
            .collect::<Vec<_>>(),
        hinges.iter().map(|hinge| hinge.id).collect::<Vec<_>>()
    );
    TwoHingeFixture {
        model,
        hinges: [hinges[0].id, hinges[1].id],
    }
}

fn triangle_fan_fixture(face_count: usize, reverse_source: bool) -> TriangleFanFixture {
    let mut vertices = (0..face_count + 2)
        .map(|index| {
            let x = index as f64 * 20.0;
            vertex(index as u64 + 100, x, x * x / 400.0)
        })
        .collect::<Vec<_>>();
    let boundary = vertices.iter().map(|entry| entry.id).collect::<Vec<_>>();
    let mut edges = (0..boundary.len())
        .map(|index| {
            edge(
                index as u64 + 100,
                boundary[index],
                boundary[(index + 1) % boundary.len()],
                EdgeKind::Boundary,
            )
        })
        .collect::<Vec<_>>();
    let hinges = (2..boundary.len() - 1)
        .map(|index| {
            edge(
                index as u64 + 200,
                boundary[0],
                boundary[index],
                EdgeKind::Mountain,
            )
        })
        .collect::<Vec<_>>();
    let hinge_ids = hinges.iter().map(|hinge| hinge.id).collect::<Vec<_>>();
    edges.extend(hinges);
    if reverse_source {
        vertices.reverse();
        edges.reverse();
    }
    let pattern = CreasePattern { vertices, edges };
    let paper = Paper {
        boundary_vertices: boundary,
        ..Paper::default()
    };
    let report = analyze_faces(FaceExtractionInput {
        identity_namespace: project_id_variant(face_count as u64 + 20),
        source_revision: face_count as u64,
        paper: &paper,
        pattern: &pattern,
    });
    assert!(report.issues.is_empty(), "{:?}", report.issues);
    let topology = report.snapshot.expect("fan topology");
    let model = MaterialTreeKinematicsModel::prepare(
        &pattern,
        &paper,
        &topology,
        TreeKinematicsLimits::default(),
    )
    .expect("fan model");
    assert_eq!(model.face_ids().len(), face_count);
    TriangleFanFixture {
        model,
        hinges: hinge_ids,
    }
}

fn midpoint_mountain_400mm_fixture(reverse_source_collections: bool) -> TwoHingeFixture {
    two_hinge_fixture(
        8,
        &[
            (0.0, 0.0),
            (200.0, 0.0),
            (400.0, 0.0),
            (400.0, 400.0),
            (0.0, 400.0),
        ],
        &[(1, 4, EdgeKind::Mountain), (1, 3, EdgeKind::Mountain)],
        reverse_source_collections,
    )
}

fn corner_mountain_valley_400mm_fixture(reverse_source_collections: bool) -> TwoHingeFixture {
    two_hinge_fixture(
        9,
        &[
            (0.0, 0.0),
            (400.0, 0.0),
            (400.0, 200.0),
            (400.0, 400.0),
            (200.0, 400.0),
            (0.0, 400.0),
        ],
        &[(0, 2, EdgeKind::Mountain), (0, 4, EdgeKind::Valley)],
        reverse_source_collections,
    )
}

fn corner_mountain_mountain_400mm_fixture(reverse_source_collections: bool) -> TwoHingeFixture {
    two_hinge_fixture(
        11,
        &[
            (0.0, 0.0),
            (400.0, 0.0),
            (400.0, 200.0),
            (400.0, 400.0),
            (200.0, 400.0),
            (0.0, 400.0),
        ],
        &[(0, 2, EdgeKind::Mountain), (0, 4, EdgeKind::Mountain)],
        reverse_source_collections,
    )
}

fn corner_mountain_mountain_quadrilateral_400mm_fixture(
    reverse_source_collections: bool,
) -> TwoHingeFixture {
    two_hinge_fixture(
        10,
        &[
            (0.0, 0.0),
            (200.0, 0.0),
            (400.0, 0.0),
            (400.0, 200.0),
            (400.0, 400.0),
            (200.0, 400.0),
            (0.0, 400.0),
        ],
        &[(0, 3, EdgeKind::Mountain), (0, 5, EdgeKind::Mountain)],
        reverse_source_collections,
    )
}

fn triangular_shared_hinge_400mm_fixture(
    assignment: EdgeKind,
    reverse_source_collections: bool,
    reverse_hinge_endpoints: bool,
) -> (MaterialTreeKinematicsModel, EdgeId) {
    let mut vertices = vec![
        vertex(1, 0.0, 0.0),
        vertex(2, 400.0, 0.0),
        vertex(3, 400.0, 400.0),
        vertex(4, 0.0, 400.0),
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
    let (start, end) = if reverse_hinge_endpoints {
        (boundary[2], boundary[0])
    } else {
        (boundary[0], boundary[2])
    };
    let hinge = edge(5, start, end, assignment);
    edges.push(hinge.clone());
    if reverse_source_collections {
        vertices.reverse();
        edges.reverse();
    }
    let pattern = CreasePattern { vertices, edges };
    let paper = Paper {
        boundary_vertices: boundary,
        ..Paper::default()
    };
    let report = analyze_faces(FaceExtractionInput {
        identity_namespace: project_id(),
        source_revision: 12,
        paper: &paper,
        pattern: &pattern,
    });
    assert!(report.issues.is_empty(), "{:?}", report.issues);
    let topology = report.snapshot.expect("triangular hinge topology");
    let model = MaterialTreeKinematicsModel::prepare(
        &pattern,
        &paper,
        &topology,
        TreeKinematicsLimits::default(),
    )
    .expect("triangular hinge model");
    assert_eq!(model.face_ids().len(), 2);
    assert_eq!(model.hinges().len(), 1);
    (model, hinge.id)
}

fn triangular_shared_hinge_40x30_identity_fixture(
    vertex_identity: [u64; 4],
    identity_namespace: ProjectId,
) -> (MaterialTreeKinematicsModel, EdgeId) {
    let coordinates = [(0.0, 0.0), (40.0, 0.0), (40.0, 30.0), (0.0, 30.0)];
    let vertices = coordinates
        .into_iter()
        .zip(vertex_identity)
        .map(|((x, y), identity)| vertex(identity, x, y))
        .collect::<Vec<_>>();
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
    let hinge = edge(5, boundary[0], boundary[2], EdgeKind::Mountain);
    edges.push(hinge.clone());
    let pattern = CreasePattern { vertices, edges };
    let paper = Paper {
        boundary_vertices: boundary,
        thickness_mm: 0.1,
        ..Paper::default()
    };
    let report = analyze_faces(FaceExtractionInput {
        identity_namespace,
        source_revision: 12,
        paper: &paper,
        pattern: &pattern,
    });
    assert!(report.issues.is_empty(), "{:?}", report.issues);
    let topology = report.snapshot.expect("identity fixture topology");
    let model = MaterialTreeKinematicsModel::prepare(
        &pattern,
        &paper,
        &topology,
        TreeKinematicsLimits::default(),
    )
    .expect("identity fixture material model");
    assert_eq!(model.face_ids().len(), 2);
    assert_eq!(model.hinges().len(), 1);
    (model, hinge.id)
}

fn only_non_hinge_face_pair(model: &MaterialTreeKinematicsModel) -> [FaceId; 2] {
    let mut pairs = model
        .face_ids()
        .iter()
        .copied()
        .enumerate()
        .flat_map(|(first_index, first)| {
            model.face_ids()[first_index + 1..]
                .iter()
                .copied()
                .map(move |second| [first, second])
        })
        .filter(|pair| {
            !model.hinges().iter().any(|hinge| {
                let mut hinge_pair = [hinge.left_face(), hinge.right_face()];
                hinge_pair.sort_unstable_by_key(FaceId::canonical_bytes);
                hinge_pair == *pair
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(pairs.len(), 1, "three-face V has one non-hinge pair");
    pairs.pop().expect("non-hinge pair")
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
        assert_eq!(proof.expected_triangle_pairs(), 0);
        assert_eq!(proof.analyzed_triangle_pairs(), 0);
    }

    let pose = model.solve(None, &no_angles()).expect("planar pose");
    let proof = prove_static_collision_geometry(
        &model,
        &pose,
        fixture.paper.thickness_mm,
        StaticCollisionLimits {
            max_faces: 1,
            max_unordered_face_pairs: 0,
            max_boundary_vertices_per_face: 0,
            max_total_boundary_vertices: 0,
            max_triangles_per_face: 0,
            max_total_triangles: 0,
            max_triangulation_work_per_face: 0,
            max_total_triangulation_work: 0,
            max_registry_authentication_work: 0,
            max_triangle_pairs_per_face_pair: 0,
            max_total_triangle_pairs: 0,
            max_boundary_relation_work_per_face_pair: 0,
            max_total_boundary_relation_work: 0,
            max_rational_input_bits: 0,
            max_total_rational_input_storage_bits: 0,
            max_total_rational_retained_clone_bits: 0,
            max_rational_operations: 0,
            max_rational_intermediate_bits: 0,
            max_rational_gcd_fallback_calls: 0,
            max_rational_gcd_fallback_input_bits: 0,
            max_rational_allocations: 0,
            max_rational_allocation_bits: 0,
            max_total_rational_allocation_bits: 0,
            max_rational_output_bits: 0,
            max_total_rational_output_bits: 0,
            max_shared_hinge_boundary_diagnostics: 0,
            max_shared_hinge_solid_diagnostics: 0,
        },
    )
    .expect("zero-pair proof does not allocate pair geometry");
    assert_eq!(proof.expected_triangle_pairs(), 0);
    assert_eq!(proof.analyzed_triangle_pairs(), 0);
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
                ..StaticCollisionLimits::default()
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
                ..StaticCollisionLimits::default()
            },
        ),
        StaticCollisionError::ResourceLimitExceeded,
    );
}

#[test]
fn public_entry_reports_midpoint_transversal_matrix_without_collection_or_root_bias() {
    for reverse_source_collections in [false, true] {
        let fixture = midpoint_mountain_400mm_fixture(reverse_source_collections);
        let expected_proven_pair = only_non_hinge_face_pair(&fixture.model);
        for (angle, proven_transversal_pairs) in
            [(90.0, 0), (91.0, 0), (135.0, 1), (179.0, 1), (180.0, 0)]
        {
            let angles = CanonicalHingeAngles::new(
                fixture
                    .hinges
                    .iter()
                    .copied()
                    .map(|hinge| HingeAngle::new(hinge, angle).expect("valid midpoint angle"))
                    .collect(),
            )
            .expect("canonical midpoint angles");
            for root in fixture.model.face_ids().iter().copied() {
                let pose = fixture
                    .model
                    .solve(Some(root), &angles)
                    .expect("folded midpoint pose");
                let actual = prove_static_collision_geometry(
                    &fixture.model,
                    &pose,
                    0.0,
                    StaticCollisionLimits::default(),
                )
                .expect_err("multi-face midpoint pose must remain blocking");
                let expected = if proven_transversal_pairs > 0 {
                    StaticCollisionError::ProvenTransversalPenetration {
                        expected_unordered_face_pairs: 3,
                        proven_transversal_pairs,
                        first_proven_transversal_pair: expected_proven_pair,
                    }
                } else {
                    StaticCollisionError::PairEvidenceUnavailable {
                        expected_unordered_face_pairs: 3,
                    }
                };
                assert_eq!(
                    actual, expected,
                    "angle {angle}, root {root:?}, reversed {reverse_source_collections}"
                );
            }
        }
    }
}

#[test]
fn positive_thickness_mid_surface_transversal_is_blocking_without_order_or_root_bias() {
    const THICKNESSES: [f64; 3] = [0.1, 1.0, 3.0];
    const CASES: [(f64, bool); 5] = [
        (90.0, false),
        (91.0, false),
        (135.0, true),
        (179.0, true),
        (180.0, false),
    ];

    for reverse_source_collections in [false, true] {
        let fixture = midpoint_mountain_400mm_fixture(reverse_source_collections);
        let expected_proven_pair = only_non_hinge_face_pair(&fixture.model);
        for thickness in THICKNESSES {
            for (angle, is_proven) in CASES {
                let angles = CanonicalHingeAngles::new(
                    fixture
                        .hinges
                        .iter()
                        .copied()
                        .map(|hinge| HingeAngle::new(hinge, angle).expect("valid midpoint angle"))
                        .collect(),
                )
                .expect("canonical midpoint angles");
                for root in fixture.model.face_ids().iter().copied() {
                    let pose = fixture
                        .model
                        .solve(Some(root), &angles)
                        .expect("folded midpoint pose");
                    let expected = if is_proven {
                        StaticCollisionError::ProvenPositiveThicknessPenetration {
                            expected_unordered_face_pairs: 3,
                            proven_positive_thickness_pairs: 1,
                            first_proven_positive_thickness_pair: expected_proven_pair,
                        }
                    } else {
                        StaticCollisionError::PairEvidenceUnavailable {
                            expected_unordered_face_pairs: 3,
                        }
                    };
                    assert_error(
                        prove_static_collision_geometry(
                            &fixture.model,
                            &pose,
                            thickness,
                            StaticCollisionLimits::default(),
                        ),
                        expected,
                    );
                }
            }
        }
    }
}

#[test]
fn public_entry_never_promotes_corner_shared_vertex_contact_to_transversal_penetration() {
    const CASES: [[f64; 2]; 7] = [
        [10.0, 0.0],
        [0.0, 10.0],
        [45.0, 45.0],
        [90.0, 90.0],
        [91.0, 91.0],
        [135.0, 135.0],
        [179.0, 179.0],
    ];

    for reverse_source_collections in [false, true] {
        let fixture = corner_mountain_valley_400mm_fixture(reverse_source_collections);
        for angle_pair in CASES {
            let angles = CanonicalHingeAngles::new(
                fixture
                    .hinges
                    .iter()
                    .copied()
                    .zip(angle_pair)
                    .map(|(hinge, angle)| {
                        HingeAngle::new(hinge, angle).expect("valid corner angle")
                    })
                    .collect(),
            )
            .expect("canonical corner angles");
            for root in fixture.model.face_ids().iter().copied() {
                let pose = fixture
                    .model
                    .solve(Some(root), &angles)
                    .expect("folded corner pose");
                assert_error(
                    prove_static_collision_geometry(
                        &fixture.model,
                        &pose,
                        0.0,
                        StaticCollisionLimits::default(),
                    ),
                    StaticCollisionError::PairEvidenceUnavailable {
                        expected_unordered_face_pairs: 3,
                    },
                );
            }
        }
    }
}

#[test]
fn positive_thickness_corner_contact_never_becomes_mid_surface_penetration() {
    const THICKNESSES: [f64; 3] = [0.1, 1.0, 3.0];
    const CASES: [[f64; 2]; 8] = [
        [10.0, 0.0],
        [0.0, 10.0],
        [45.0, 45.0],
        [90.0, 90.0],
        [91.0, 91.0],
        [135.0, 135.0],
        [179.0, 179.0],
        [180.0, 180.0],
    ];

    for reverse_source_collections in [false, true] {
        let fixture = corner_mountain_valley_400mm_fixture(reverse_source_collections);
        for thickness in THICKNESSES {
            for angle_pair in CASES {
                let angles = CanonicalHingeAngles::new(
                    fixture
                        .hinges
                        .iter()
                        .copied()
                        .zip(angle_pair)
                        .map(|(hinge, angle)| {
                            HingeAngle::new(hinge, angle).expect("valid corner angle")
                        })
                        .collect(),
                )
                .expect("canonical corner angles");
                for root in fixture.model.face_ids().iter().copied() {
                    let pose = fixture
                        .model
                        .solve(Some(root), &angles)
                        .expect("folded corner pose");
                    assert_error(
                        prove_static_collision_geometry(
                            &fixture.model,
                            &pose,
                            thickness,
                            StaticCollisionLimits::default(),
                        ),
                        StaticCollisionError::PairEvidenceUnavailable {
                            expected_unordered_face_pairs: 3,
                        },
                    );
                }
            }
        }
    }
}

#[test]
fn public_diagnostic_freezes_every_topology_by_evidence_policy_cell() {
    use StaticCollisionPairDisposition::{
        Allowed, CandidateExcluded, Indeterminate, Penetrating, Separated, Touching,
    };

    const EXPECTED: [[StaticCollisionPairDisposition; 11]; 4] = [
        [
            Separated,
            Touching,
            Touching,
            Touching,
            Indeterminate,
            Indeterminate,
            Indeterminate,
            Penetrating,
            Penetrating,
            Penetrating,
            Indeterminate,
        ],
        [
            Indeterminate,
            Touching,
            Touching,
            Touching,
            Allowed,
            Allowed,
            Indeterminate,
            Penetrating,
            Penetrating,
            Penetrating,
            Indeterminate,
        ],
        [
            Indeterminate,
            Indeterminate,
            Indeterminate,
            Indeterminate,
            Indeterminate,
            Indeterminate,
            Indeterminate,
            Penetrating,
            Penetrating,
            Penetrating,
            Indeterminate,
        ],
        [CandidateExcluded; 11],
    ];

    for (topology_index, topology) in TopologyRelation::ALL.into_iter().enumerate() {
        for (evidence_index, evidence) in IntersectionEvidenceV2::ALL.into_iter().enumerate() {
            let decision = classify_runtime_topology_contact_v2(topology, evidence);
            assert_eq!(
                classify_static_collision_pair_disposition(topology, decision),
                EXPECTED[topology_index][evidence_index],
                "{} × {}",
                topology.identifier(),
                evidence.identifier()
            );
        }
    }

    // These five columns are the user-facing separated / point / line /
    // boundary-area / transversal snapshot requested by the collision audit.
    let contact_columns = [
        IntersectionEvidenceV2::Separated,
        IntersectionEvidenceV2::PointContact,
        IntersectionEvidenceV2::BoundaryLineContact,
        IntersectionEvidenceV2::BoundaryAreaContact,
        IntersectionEvidenceV2::TransversalCrossing,
    ];
    let identifiers = TopologyRelation::ALL.map(|topology| {
        contact_columns.map(|evidence| {
            classify_static_collision_pair_disposition(
                topology,
                classify_runtime_topology_contact_v2(topology, evidence),
            )
            .identifier()
        })
    });
    assert_eq!(
        identifiers,
        [
            [
                "separated",
                "touching",
                "touching",
                "touching",
                "penetrating",
            ],
            [
                "indeterminate",
                "touching",
                "touching",
                "touching",
                "penetrating",
            ],
            [
                "indeterminate",
                "indeterminate",
                "indeterminate",
                "indeterminate",
                "penetrating",
            ],
            ["candidate_excluded"; 5],
        ]
    );
}

#[test]
fn public_diagnostic_connects_shared_hinge_solid_only_for_two_triangular_faces() {
    const CASES: [(f64, StaticCollisionPairDisposition, IntersectionEvidenceV2); 6] = [
        (
            0.0,
            StaticCollisionPairDisposition::Allowed,
            IntersectionEvidenceV2::BoundaryAreaContact,
        ),
        (
            10.0,
            StaticCollisionPairDisposition::Allowed,
            IntersectionEvidenceV2::SharedFeatureThicknessOverlap,
        ),
        (
            90.0,
            StaticCollisionPairDisposition::Indeterminate,
            IntersectionEvidenceV2::Indeterminate,
        ),
        (
            135.0,
            StaticCollisionPairDisposition::Allowed,
            IntersectionEvidenceV2::SharedFeatureThicknessOverlap,
        ),
        (
            179.0,
            StaticCollisionPairDisposition::Allowed,
            IntersectionEvidenceV2::SharedFeatureThicknessOverlap,
        ),
        (
            180.0,
            StaticCollisionPairDisposition::Indeterminate,
            IntersectionEvidenceV2::Indeterminate,
        ),
    ];

    for assignment in [EdgeKind::Mountain, EdgeKind::Valley] {
        for reverse_source_collections in [false, true] {
            for reverse_hinge_endpoints in [false, true] {
                let (model, hinge) = triangular_shared_hinge_400mm_fixture(
                    assignment,
                    reverse_source_collections,
                    reverse_hinge_endpoints,
                );
                for root in model.face_ids().iter().copied() {
                    for thickness in [0.1, 1.0, 3.0] {
                        for (angle, expected_disposition, expected_evidence) in CASES {
                            let angles = CanonicalHingeAngles::new(vec![
                                HingeAngle::new(hinge, angle).expect("valid hinge angle"),
                            ])
                            .expect("canonical hinge angle");
                            let pose = model
                                .solve(Some(root), &angles)
                                .expect("triangular hinge pose");
                            let snapshot = diagnose_static_collision_geometry(
                                &model,
                                &pose,
                                thickness,
                                StaticCollisionLimits::default(),
                            )
                            .expect("complete shared-hinge diagnostic");
                            assert_eq!(snapshot.pairs().len(), 1);
                            let pair = snapshot.pairs()[0];
                            assert_eq!(pair.topology(), TopologyRelation::SharedHingeEdge);
                            assert!(pair.shared_hinge_solid_classified());
                            assert_eq!(
                                pair.disposition(),
                                expected_disposition,
                                "{assignment:?}, source reversed {reverse_source_collections}, \
                                 endpoints reversed {reverse_hinge_endpoints}, {root:?}, \
                                 {thickness} mm, {angle}°"
                            );
                            assert_eq!(pair.evidence(), expected_evidence);
                            assert!(!pair.strict_transversal_dual_gate_proven());
                            assert!(!pair.whole_face_overlap_proven());
                            assert!(!pair.shared_hinge_boundary_contact_proven());
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn positive_thickness_exact_prism_safe_proof_admits_shared_hinge_with_finite_bounds() {
    let cases = [
        (EdgeKind::Mountain, false, false, false, 0.1),
        (EdgeKind::Mountain, true, false, true, 1.0),
        (EdgeKind::Valley, false, true, false, 3.0),
    ];
    for (assignment, reverse_source, reverse_endpoints, reverse_root, thickness) in cases {
        let (model, hinge) =
            triangular_shared_hinge_400mm_fixture(assignment, reverse_source, reverse_endpoints);
        let root = if reverse_root {
            *model.face_ids().last().unwrap()
        } else {
            model.face_ids()[0]
        };
        let angles =
            CanonicalHingeAngles::new(vec![HingeAngle::new(hinge, 10.0).unwrap()]).unwrap();
        let pose = model.solve(Some(root), &angles).unwrap();
        let proof = prove_static_collision_geometry(
            &model,
            &pose,
            thickness,
            StaticCollisionLimits::default(),
        )
        .expect("complete exact-prism shared-hinge proof");
        assert!(proof.is_for_geometry(&model, &pose, thickness));
        assert_eq!(proof.expected_unordered_face_pairs(), 1);
        assert_eq!(proof.analyzed_unordered_face_pairs(), 1);
        assert_eq!(proof.expected_triangle_pairs(), 1);
        assert_eq!(proof.analyzed_triangle_pairs(), 1);
    }
    let (model, hinge) = triangular_shared_hinge_400mm_fixture(EdgeKind::Mountain, false, false);
    let root = model.face_ids()[0];
    let pose = model
        .solve(
            Some(root),
            &CanonicalHingeAngles::new(vec![HingeAngle::new(hinge, 10.0).unwrap()]).unwrap(),
        )
        .unwrap();
    assert!(matches!(
        prove_static_collision_geometry(
            &model,
            &pose,
            0.1,
            StaticCollisionLimits {
                max_shared_hinge_solid_diagnostics: 0,
                ..StaticCollisionLimits::default()
            },
        ),
        Err(StaticCollisionError::ResourceLimitExceeded)
    ));
    let boundary_pose = model
        .solve(
            Some(root),
            &CanonicalHingeAngles::new(vec![HingeAngle::new(hinge, 90.0).unwrap()]).unwrap(),
        )
        .unwrap();
    assert!(matches!(
        prove_static_collision_geometry(
            &model,
            &boundary_pose,
            0.1,
            StaticCollisionLimits::default(),
        ),
        Err(StaticCollisionError::PairEvidenceUnavailable { .. })
    ));
}

#[test]
fn three_face_positive_thickness_proof_admits_the_finite_vertex_corridor() {
    for (reverse_source, reverse_root, thickness) in
        [(false, false, 0.1), (true, true, 1.0), (false, true, 3.0)]
    {
        let fixture = midpoint_mountain_400mm_fixture(reverse_source);
        let root = if reverse_root {
            *fixture.model.face_ids().last().unwrap()
        } else {
            fixture.model.face_ids()[0]
        };
        let angles = CanonicalHingeAngles::new(
            fixture
                .hinges
                .iter()
                .copied()
                .map(|hinge| HingeAngle::new(hinge, 10.0).unwrap())
                .collect(),
        )
        .unwrap();
        let pose = fixture.model.solve(Some(root), &angles).unwrap();
        let proof = prove_static_collision_geometry(
            &fixture.model,
            &pose,
            thickness,
            StaticCollisionLimits::default(),
        )
        .expect("two hinge corridors and one vertex corridor");
        assert!(proof.is_for_geometry(&fixture.model, &pose, thickness));
        assert_eq!(proof.expected_unordered_face_pairs(), 3);
        assert_eq!(proof.analyzed_unordered_face_pairs(), 3);
        if thickness == 0.1 {
            assert!(matches!(
                prove_static_collision_geometry(
                    &fixture.model,
                    &pose,
                    thickness,
                    StaticCollisionLimits {
                        max_shared_hinge_solid_diagnostics: 2,
                        ..StaticCollisionLimits::default()
                    },
                ),
                Err(StaticCollisionError::ResourceLimitExceeded)
            ));
        }
    }
}

#[test]
fn four_to_sixteen_face_positive_thickness_fans_scan_every_pair() {
    for (face_count, reverse_source, reverse_root, thickness) in [
        (4, false, false, 0.1),
        (8, true, true, 1.0),
        (16, false, true, 3.0),
    ] {
        let fixture = triangle_fan_fixture(face_count, reverse_source);
        let root = if reverse_root {
            *fixture.model.face_ids().last().unwrap()
        } else {
            fixture.model.face_ids()[0]
        };
        let angles = CanonicalHingeAngles::new(
            fixture
                .hinges
                .iter()
                .copied()
                .map(|hinge| HingeAngle::new(hinge, 0.0).unwrap())
                .collect(),
        )
        .unwrap();
        let pose = fixture.model.solve(Some(root), &angles).unwrap();
        let expected_pairs = face_count * (face_count - 1) / 2;
        let proof = prove_static_collision_geometry(
            &fixture.model,
            &pose,
            thickness,
            StaticCollisionLimits::default(),
        )
        .expect("bounded fan proof");
        assert_eq!(proof.expected_unordered_face_pairs(), expected_pairs);
        assert_eq!(proof.analyzed_unordered_face_pairs(), expected_pairs);
        assert!(proof.is_for_geometry(&fixture.model, &pose, thickness));
        if face_count == 16 {
            assert!(matches!(
                prove_static_collision_geometry(
                    &fixture.model,
                    &pose,
                    thickness,
                    StaticCollisionLimits {
                        max_shared_hinge_solid_diagnostics: expected_pairs - 1,
                        ..StaticCollisionLimits::default()
                    },
                ),
                Err(StaticCollisionError::ResourceLimitExceeded)
            ));
        }
    }
}

#[test]
fn positive_thickness_ninety_degree_hold_is_identity_and_root_invariant() {
    for namespace_index in 1_u64..=8 {
        for first in 1_u64..=4 {
            for second in 1_u64..=4 {
                for third in 1_u64..=4 {
                    for fourth in 1_u64..=4 {
                        let identity = [first, second, third, fourth];
                        if identity
                            .iter()
                            .enumerate()
                            .any(|(index, value)| identity[..index].contains(value))
                        {
                            continue;
                        }
                        let (model, hinge) = triangular_shared_hinge_40x30_identity_fixture(
                            identity,
                            project_id_variant(namespace_index),
                        );
                        let angles = CanonicalHingeAngles::new(vec![
                            HingeAngle::new(hinge, 90.0).expect("valid right angle"),
                        ])
                        .expect("canonical right angle");
                        for root in model.face_ids().iter().copied() {
                            let pose = model
                                .solve(Some(root), &angles)
                                .expect("identity-permuted pose");
                            let snapshot = diagnose_static_collision_geometry(
                                &model,
                                &pose,
                                0.1,
                                StaticCollisionLimits::default(),
                            )
                            .expect("identity-permuted diagnostic");
                            assert_eq!(snapshot.pairs().len(), 1);
                            let pair = snapshot.pairs()[0];
                            assert_eq!(pair.topology(), TopologyRelation::SharedHingeEdge);
                            assert_eq!(pair.evidence(), IntersectionEvidenceV2::Indeterminate);
                            assert_eq!(
                                pair.policy_decision(),
                                TopologyContactDecision::Indeterminate
                            );
                            assert_eq!(
                                pair.disposition(),
                                StaticCollisionPairDisposition::Indeterminate,
                                "namespace {namespace_index}, identity {identity:?}, root {root:?}"
                            );
                            assert!(!pair.strict_transversal_dual_gate_proven());
                            assert!(!pair.whole_face_overlap_proven());
                            assert!(!pair.shared_hinge_boundary_contact_proven());
                            assert!(pair.shared_hinge_solid_classified());
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn zero_thickness_shared_hinge_boundary_contact_is_allowed_until_area_overlap() {
    const CASES: [(f64, StaticCollisionPairDisposition); 5] = [
        (0.0, StaticCollisionPairDisposition::Allowed),
        (10.0, StaticCollisionPairDisposition::Allowed),
        (90.0, StaticCollisionPairDisposition::Allowed),
        (179.0, StaticCollisionPairDisposition::Allowed),
        (180.0, StaticCollisionPairDisposition::Penetrating),
    ];

    for assignment in [EdgeKind::Mountain, EdgeKind::Valley] {
        for reverse_source_collections in [false, true] {
            for reverse_hinge_endpoints in [false, true] {
                let (model, hinge) = triangular_shared_hinge_400mm_fixture(
                    assignment,
                    reverse_source_collections,
                    reverse_hinge_endpoints,
                );
                for root in model.face_ids().iter().copied() {
                    for (angle, expected_disposition) in CASES {
                        let angles = CanonicalHingeAngles::new(vec![
                            HingeAngle::new(hinge, angle).expect("valid hinge angle"),
                        ])
                        .expect("canonical hinge angle");
                        let pose = model
                            .solve(Some(root), &angles)
                            .expect("zero-thickness triangular hinge pose");
                        let snapshot = diagnose_static_collision_geometry(
                            &model,
                            &pose,
                            0.0,
                            StaticCollisionLimits::default(),
                        )
                        .expect("complete zero-thickness shared-hinge diagnostic");
                        assert_eq!(snapshot.expected_unordered_face_pairs(), 1);
                        assert_eq!(snapshot.pairs().len(), 1);
                        let pair = snapshot.pairs()[0];
                        assert_eq!(pair.topology(), TopologyRelation::SharedHingeEdge);
                        assert_eq!(
                            pair.disposition(),
                            expected_disposition,
                            "{assignment:?}, source reversed {reverse_source_collections}, \
                             endpoints reversed {reverse_hinge_endpoints}, {root:?}, {angle}°"
                        );
                        assert!(!pair.shared_hinge_solid_classified());
                        if angle < 180.0 {
                            assert_eq!(
                                pair.evidence(),
                                IntersectionEvidenceV2::SharedFeatureContact
                            );
                            assert_eq!(
                                pair.policy_decision(),
                                TopologyContactDecision::RequiresHingeModel
                            );
                            assert!(pair.shared_hinge_boundary_contact_proven());
                            assert!(!pair.strict_transversal_dual_gate_proven());
                            assert!(!pair.whole_face_overlap_proven());
                            assert_eq!(snapshot.allowed_pairs(), 1);
                            assert_eq!(snapshot.penetrating_pairs(), 0);
                            assert_eq!(snapshot.indeterminate_pairs(), 0);
                        } else {
                            assert!(!pair.shared_hinge_boundary_contact_proven());
                            assert!(
                                pair.whole_face_overlap_proven()
                                    || pair.strict_transversal_dual_gate_proven(),
                                "a full-fold area overlap must carry an exact penetration proof"
                            );
                            assert_eq!(snapshot.allowed_pairs(), 0);
                            assert_eq!(snapshot.penetrating_pairs(), 1);
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn shared_hinge_solid_diagnostic_has_an_explicit_public_resource_gate() {
    let (model, hinge) = triangular_shared_hinge_400mm_fixture(EdgeKind::Mountain, false, false);
    let angles = CanonicalHingeAngles::new(vec![
        HingeAngle::new(hinge, 10.0).expect("valid hinge angle"),
    ])
    .expect("canonical hinge angle");
    let pose = model
        .solve(Some(model.face_ids()[0]), &angles)
        .expect("triangular hinge pose");
    assert_eq!(
        diagnose_static_collision_geometry(
            &model,
            &pose,
            0.1,
            StaticCollisionLimits {
                max_shared_hinge_solid_diagnostics: 0,
                ..StaticCollisionLimits::default()
            },
        ),
        Err(StaticCollisionError::ResourceLimitExceeded)
    );
}

#[test]
fn zero_thickness_shared_hinge_boundary_gate_counts_only_submitted_pairs() {
    let fixture = midpoint_mountain_400mm_fixture(false);
    let model_hinges = fixture.model.hinges();
    let root = fixture
        .model
        .face_ids()
        .iter()
        .copied()
        .find(|face| {
            model_hinges
                .iter()
                .all(|hinge| hinge.left_face() == *face || hinge.right_face() == *face)
        })
        .expect("the fan's center triangle is incident to both hinges");
    let solve = |degrees: [f64; 2]| {
        let angles = CanonicalHingeAngles::new(
            fixture
                .hinges
                .iter()
                .copied()
                .zip(degrees)
                .map(|(hinge, angle)| {
                    HingeAngle::new(hinge, angle).expect("valid resource-gate angle")
                })
                .collect(),
        )
        .expect("canonical resource-gate angles");
        fixture
            .model
            .solve(Some(root), &angles)
            .expect("resource-gate pose")
    };

    // One unfolded hinge is already classified directly by the raw exact
    // topology scan. Only the nonzero hinge is submitted to the watertight
    // theorem, so a limit of exactly one succeeds and zero fails.
    let mixed = solve([0.0, 10.0]);
    assert_eq!(
        diagnose_static_collision_geometry(
            &fixture.model,
            &mixed,
            0.0,
            StaticCollisionLimits {
                max_shared_hinge_boundary_diagnostics: 0,
                ..StaticCollisionLimits::default()
            },
        ),
        Err(StaticCollisionError::ResourceLimitExceeded)
    );
    let mixed_snapshot = diagnose_static_collision_geometry(
        &fixture.model,
        &mixed,
        0.0,
        StaticCollisionLimits {
            max_shared_hinge_boundary_diagnostics: 1,
            ..StaticCollisionLimits::default()
        },
    )
    .expect("one submitted pair fits an exact limit of one");
    assert_eq!(
        mixed_snapshot
            .pairs()
            .iter()
            .filter(|pair| matches!(pair.topology(), TopologyRelation::SharedHingeEdge))
            .count(),
        2
    );

    // Both nonzero hinges require the theorem. A one-short limit rejects the
    // snapshot atomically; the exact submitted-pair limit succeeds.
    let two_candidates = solve([10.0, 10.0]);
    assert_eq!(
        diagnose_static_collision_geometry(
            &fixture.model,
            &two_candidates,
            0.0,
            StaticCollisionLimits {
                max_shared_hinge_boundary_diagnostics: 1,
                ..StaticCollisionLimits::default()
            },
        ),
        Err(StaticCollisionError::ResourceLimitExceeded)
    );
    let exact_snapshot = diagnose_static_collision_geometry(
        &fixture.model,
        &two_candidates,
        0.0,
        StaticCollisionLimits {
            max_shared_hinge_boundary_diagnostics: 2,
            ..StaticCollisionLimits::default()
        },
    )
    .expect("two submitted pairs fit an exact limit of two");
    assert_eq!(exact_snapshot.penetrating_pairs(), 0);
    assert_eq!(
        exact_snapshot
            .pairs()
            .iter()
            .filter(|pair| {
                matches!(pair.topology(), TopologyRelation::SharedHingeEdge)
                    && pair.shared_hinge_boundary_contact_proven()
            })
            .count(),
        2
    );
}

#[test]
fn public_diagnostic_corner_mountain_valley_matrix_never_reports_penetration() {
    const THICKNESSES: [f64; 3] = [0.0, 0.1, 1.0];
    const CASES: [[f64; 2]; 5] = [
        [10.0, 0.0],
        [0.0, 10.0],
        [45.0, 45.0],
        [91.0, 91.0],
        [135.0, 135.0],
    ];

    for reverse_source_collections in [false, true] {
        let fixture = corner_mountain_valley_400mm_fixture(reverse_source_collections);
        let outer_pair = only_non_hinge_face_pair(&fixture.model);
        for thickness in THICKNESSES {
            for angle_pair in CASES {
                let angles = CanonicalHingeAngles::new(
                    fixture
                        .hinges
                        .iter()
                        .copied()
                        .zip(angle_pair)
                        .map(|(hinge, angle)| {
                            HingeAngle::new(hinge, angle).expect("valid corner angle")
                        })
                        .collect(),
                )
                .expect("canonical corner angles");
                for root in fixture.model.face_ids().iter().copied() {
                    let pose = fixture
                        .model
                        .solve(Some(root), &angles)
                        .expect("folded corner pose");
                    let diagnostic = diagnose_static_collision_geometry(
                        &fixture.model,
                        &pose,
                        thickness,
                        StaticCollisionLimits::default(),
                    )
                    .expect("complete corner diagnostic");
                    assert_eq!(
                        diagnostic.penetrating_pairs(),
                        0,
                        "{thickness} mm, {angle_pair:?}, {root:?}, reversed \
                         {reverse_source_collections}"
                    );
                    let outer = diagnostic
                        .pairs()
                        .iter()
                        .find(|pair| [pair.first_face(), pair.second_face()] == outer_pair)
                        .expect("outer shared-vertex pair");
                    assert_eq!(outer.topology(), TopologyRelation::SharedVertex);
                    assert_eq!(
                        outer.evidence(),
                        IntersectionEvidenceV2::SharedFeatureContact
                    );
                    assert_eq!(
                        outer.policy_decision(),
                        TopologyContactDecision::AllowedSharedVertexContact
                    );
                    assert_eq!(outer.disposition(), StaticCollisionPairDisposition::Allowed);
                    assert!(!outer.strict_transversal_dual_gate_proven());
                    assert!(!outer.whole_face_overlap_proven());
                    assert!(
                        !outer.shared_hinge_solid_classified(),
                        "a three-face V outer pair must never enter the two-face hinge gate"
                    );
                }
            }
        }
    }
}

#[test]
fn public_diagnostic_mountain_mountain_matrix_is_never_silent() {
    const THICKNESSES: [f64; 3] = [0.0, 0.1, 3.0];
    const ANGLES: [f64; 3] = [90.0, 135.0, 179.0];

    for reverse_source_collections in [false, true] {
        let fixture = corner_mountain_mountain_400mm_fixture(reverse_source_collections);
        let outer_pair = only_non_hinge_face_pair(&fixture.model);
        for thickness in THICKNESSES {
            for angle in ANGLES {
                let angles = CanonicalHingeAngles::new(
                    fixture
                        .hinges
                        .iter()
                        .copied()
                        .map(|hinge| HingeAngle::new(hinge, angle).expect("valid mountain angle"))
                        .collect(),
                )
                .expect("canonical mountain angles");
                for root in fixture.model.face_ids().iter().copied() {
                    let pose = fixture
                        .model
                        .solve(Some(root), &angles)
                        .expect("folded mountain pose");
                    let diagnostic = diagnose_static_collision_geometry(
                        &fixture.model,
                        &pose,
                        thickness,
                        StaticCollisionLimits::default(),
                    )
                    .expect("complete mountain diagnostic");
                    let outer = diagnostic
                        .pairs()
                        .iter()
                        .find(|pair| [pair.first_face(), pair.second_face()] == outer_pair)
                        .expect("outer mountain pair");
                    if angle > 90.0 {
                        assert_eq!(
                            outer.disposition(),
                            StaticCollisionPairDisposition::Penetrating,
                            "{thickness} mm, {angle}°, {root:?}, reversed \
                             {reverse_source_collections}"
                        );
                        assert!(outer.strict_transversal_dual_gate_proven());
                        assert!(diagnostic.penetrating_pairs() > 0);
                    } else {
                        assert!(
                            matches!(
                                outer.disposition(),
                                StaticCollisionPairDisposition::Touching
                                    | StaticCollisionPairDisposition::Indeterminate
                                    | StaticCollisionPairDisposition::Penetrating
                            ),
                            "90° must be an explicit contact, hold, or penetration: \
                             {thickness} mm, {root:?}, reversed {reverse_source_collections}: \
                             {outer:?}"
                        );
                        assert!(
                            diagnostic.has_prominent_blocking_hold()
                                || diagnostic.touching_pairs() > 0,
                            "90° cannot be silent"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn public_diagnostic_midpoint_mountain_matrix_matches_the_reported_crossing_layout() {
    const THICKNESSES: [f64; 3] = [0.0, 0.1, 3.0];
    const ANGLES: [f64; 3] = [90.0, 135.0, 179.0];

    for reverse_source_collections in [false, true] {
        let fixture = midpoint_mountain_400mm_fixture(reverse_source_collections);
        let outer_pair = only_non_hinge_face_pair(&fixture.model);
        for thickness in THICKNESSES {
            for angle in ANGLES {
                let angles = CanonicalHingeAngles::new(
                    fixture
                        .hinges
                        .iter()
                        .copied()
                        .map(|hinge| HingeAngle::new(hinge, angle).expect("valid midpoint angle"))
                        .collect(),
                )
                .expect("canonical midpoint angles");
                for root in fixture.model.face_ids().iter().copied() {
                    let pose = fixture
                        .model
                        .solve(Some(root), &angles)
                        .expect("folded midpoint pose");
                    let diagnostic = diagnose_static_collision_geometry(
                        &fixture.model,
                        &pose,
                        thickness,
                        StaticCollisionLimits::default(),
                    )
                    .expect("complete midpoint diagnostic");
                    let outer = diagnostic
                        .pairs()
                        .iter()
                        .find(|pair| [pair.first_face(), pair.second_face()] == outer_pair)
                        .expect("reported non-hinge midpoint pair");
                    if angle > 90.0 {
                        assert_eq!(
                            outer.disposition(),
                            StaticCollisionPairDisposition::Penetrating,
                            "reported midpoint layout: {thickness} mm, {angle}°, {root:?}, \
                             reversed {reverse_source_collections}"
                        );
                        assert!(outer.strict_transversal_dual_gate_proven());
                        assert!(diagnostic.penetrating_pairs() > 0);
                    } else {
                        assert!(
                            matches!(
                                outer.disposition(),
                                StaticCollisionPairDisposition::Touching
                                    | StaticCollisionPairDisposition::Indeterminate
                                    | StaticCollisionPairDisposition::Penetrating
                            ),
                            "reported midpoint 90° must be explicit: {thickness} mm, {root:?}, \
                             reversed {reverse_source_collections}: {outer:?}"
                        );
                        assert!(
                            diagnostic.has_prominent_blocking_hold()
                                || diagnostic.touching_pairs() > 0,
                            "reported midpoint 90° cannot be silent"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn corner_mountain_mountain_single_hinge_motion_never_becomes_penetration() {
    const THICKNESSES: [f64; 3] = [0.0, 0.1, 1.0];
    const CASES: [[f64; 2]; 4] = [[10.0, 0.0], [0.0, 10.0], [45.0, 0.0], [0.0, 45.0]];

    for reverse_source_collections in [false, true] {
        let fixture = corner_mountain_mountain_400mm_fixture(reverse_source_collections);
        let outer_pair = only_non_hinge_face_pair(&fixture.model);
        for thickness in THICKNESSES {
            for angle_pair in CASES {
                let angles = CanonicalHingeAngles::new(
                    fixture
                        .hinges
                        .iter()
                        .copied()
                        .zip(angle_pair)
                        .map(|(hinge, angle)| {
                            HingeAngle::new(hinge, angle).expect("valid one-sided corner angle")
                        })
                        .collect(),
                )
                .expect("canonical one-sided corner angles");
                for root in fixture.model.face_ids().iter().copied() {
                    let pose = fixture
                        .model
                        .solve(Some(root), &angles)
                        .expect("one-sided corner pose");
                    let diagnostic = diagnose_static_collision_geometry(
                        &fixture.model,
                        &pose,
                        thickness,
                        StaticCollisionLimits::default(),
                    )
                    .expect("complete one-sided corner diagnostic");
                    assert_eq!(
                        diagnostic.penetrating_pairs(),
                        0,
                        "{thickness} mm, {angle_pair:?}, {root:?}, reversed \
                         {reverse_source_collections}"
                    );
                    let outer = diagnostic
                        .pairs()
                        .iter()
                        .find(|pair| [pair.first_face(), pair.second_face()] == outer_pair)
                        .expect("one-sided outer shared-vertex pair");
                    assert_eq!(outer.topology(), TopologyRelation::SharedVertex);
                    assert_eq!(
                        outer.evidence(),
                        IntersectionEvidenceV2::SharedFeatureContact
                    );
                    assert_eq!(
                        outer.policy_decision(),
                        TopologyContactDecision::AllowedSharedVertexContact
                    );
                    assert_eq!(outer.disposition(), StaticCollisionPairDisposition::Allowed);
                    assert!(!outer.strict_transversal_dual_gate_proven());
                    assert!(!outer.whole_face_overlap_proven());
                    assert!(!outer.shared_hinge_boundary_contact_proven());
                    assert!(!outer.shared_hinge_solid_classified());
                }
            }
        }
    }
}

#[test]
fn public_diagnostic_snapshot_is_identical_after_source_collection_reversal() {
    let source = corner_mountain_valley_400mm_fixture(false);
    let reversed = corner_mountain_valley_400mm_fixture(true);
    assert_eq!(source.model.face_ids(), reversed.model.face_ids());
    assert_eq!(source.hinges, reversed.hinges);

    let angles_for = |fixture: &TwoHingeFixture| {
        CanonicalHingeAngles::new(
            fixture
                .hinges
                .iter()
                .copied()
                .map(|hinge| HingeAngle::new(hinge, 91.0).expect("valid angle"))
                .collect(),
        )
        .expect("canonical angles")
    };
    let source_angles = angles_for(&source);
    let reversed_angles = angles_for(&reversed);
    for root in source.model.face_ids().iter().copied() {
        let source_pose = source
            .model
            .solve(Some(root), &source_angles)
            .expect("source pose");
        let reversed_pose = reversed
            .model
            .solve(Some(root), &reversed_angles)
            .expect("reversed pose");
        for thickness in [0.0, 0.1, 1.0] {
            let source_snapshot = diagnose_static_collision_geometry(
                &source.model,
                &source_pose,
                thickness,
                StaticCollisionLimits::default(),
            )
            .expect("source diagnostic");
            let reversed_snapshot = diagnose_static_collision_geometry(
                &reversed.model,
                &reversed_pose,
                thickness,
                StaticCollisionLimits::default(),
            )
            .expect("reversed diagnostic");
            assert_eq!(
                source_snapshot, reversed_snapshot,
                "{thickness} mm, {root:?}"
            );
            assert!(source_snapshot.pairs().windows(2).all(|pairs| {
                pairs[0].first_face().canonical_bytes() < pairs[1].first_face().canonical_bytes()
                    || (pairs[0].first_face() == pairs[1].first_face()
                        && pairs[0].second_face().canonical_bytes()
                            < pairs[1].second_face().canonical_bytes())
            }));
        }
    }
}

#[test]
fn whole_face_penetration_does_not_omit_other_pair_diagnostics() {
    let fixture = corner_mountain_valley_400mm_fixture(false);
    let outer_pair = only_non_hinge_face_pair(&fixture.model);
    let angles = CanonicalHingeAngles::new(
        fixture
            .hinges
            .iter()
            .copied()
            .map(|hinge| HingeAngle::new(hinge, 180.0).expect("valid full fold"))
            .collect(),
    )
    .expect("canonical full fold");
    for root in fixture.model.face_ids().iter().copied() {
        let pose = fixture
            .model
            .solve(Some(root), &angles)
            .expect("full-fold pose");
        let snapshot = diagnose_static_collision_geometry(
            &fixture.model,
            &pose,
            0.0,
            StaticCollisionLimits::default(),
        )
        .expect("complete full-fold diagnostic");
        assert_eq!(snapshot.pairs().len(), 3);
        assert_eq!(snapshot.expected_unordered_face_pairs(), 3);
        assert_eq!(
            snapshot
                .pairs()
                .iter()
                .filter(|pair| pair.whole_face_overlap_proven())
                .map(|pair| [pair.first_face(), pair.second_face()])
                .collect::<Vec<_>>(),
            vec![outer_pair]
        );
        assert!(
            snapshot
                .pairs()
                .iter()
                .all(|pair| pair.disposition() != StaticCollisionPairDisposition::CandidateExcluded)
        );
        assert_eq!(snapshot.penetrating_pairs(), 1);
        assert_eq!(snapshot.indeterminate_pairs(), 2);
    }
}

#[test]
fn public_entry_promotes_exact_full_fold_coplanar_area_without_order_bias() {
    for reverse_source_collections in [false, true] {
        let fixture = corner_mountain_valley_400mm_fixture(reverse_source_collections);
        let expected_pair = only_non_hinge_face_pair(&fixture.model);
        let angles = CanonicalHingeAngles::new(
            fixture
                .hinges
                .iter()
                .copied()
                .map(|hinge| HingeAngle::new(hinge, 180.0).expect("valid full-fold angle"))
                .collect(),
        )
        .expect("canonical full-fold angles");
        for root in fixture.model.face_ids().iter().copied() {
            let pose = fixture
                .model
                .solve(Some(root), &angles)
                .expect("full-fold corner pose");
            assert_error(
                prove_static_collision_geometry(
                    &fixture.model,
                    &pose,
                    0.0,
                    StaticCollisionLimits::default(),
                ),
                StaticCollisionError::ProvenTransversalPenetration {
                    expected_unordered_face_pairs: 3,
                    proven_transversal_pairs: 1,
                    first_proven_transversal_pair: expected_pair,
                },
            );
            for thickness in [-0.0, 0.1, 3.0] {
                assert_error(
                    prove_static_collision_geometry(
                        &fixture.model,
                        &pose,
                        thickness,
                        StaticCollisionLimits::default(),
                    ),
                    StaticCollisionError::PairEvidenceUnavailable {
                        expected_unordered_face_pairs: 3,
                    },
                );
            }
        }
    }
}

#[test]
fn public_entry_promotes_exact_quadrilateral_transversal_without_order_bias() {
    for reverse_source_collections in [false, true] {
        let fixture =
            corner_mountain_mountain_quadrilateral_400mm_fixture(reverse_source_collections);
        let expected_pair = only_non_hinge_face_pair(&fixture.model);
        assert!(
            expected_pair.iter().copied().any(|face| {
                fixture
                    .model
                    .face_boundary(face)
                    .is_some_and(|boundary| boundary.vertices().len() > 3)
            }),
            "the proven outer pair must exercise a non-triangular material face"
        );
        let angles = CanonicalHingeAngles::new(
            fixture
                .hinges
                .iter()
                .copied()
                .map(|hinge| HingeAngle::new(hinge, 135.0).expect("valid deep-fold angle"))
                .collect(),
        )
        .expect("canonical deep-fold angles");
        for root in fixture.model.face_ids().iter().copied() {
            let pose = fixture
                .model
                .solve(Some(root), &angles)
                .expect("deep-fold quadrilateral pose");
            assert_error(
                prove_static_collision_geometry(
                    &fixture.model,
                    &pose,
                    0.0,
                    StaticCollisionLimits::default(),
                ),
                StaticCollisionError::ProvenTransversalPenetration {
                    expected_unordered_face_pairs: 3,
                    proven_transversal_pairs: 1,
                    first_proven_transversal_pair: expected_pair,
                },
            );
        }
    }
}

#[test]
fn triangular_legacy_transversal_cannot_bypass_the_cayley_dual_gate() {
    let fixture = corner_mountain_mountain_400mm_fixture(false);
    let expected_pair = only_non_hinge_face_pair(&fixture.model);
    assert!(expected_pair.iter().copied().all(|face| {
        fixture
            .model
            .face_boundary(face)
            .is_some_and(|boundary| boundary.vertices().len() == 3)
    }));
    let angles = CanonicalHingeAngles::new(
        fixture
            .hinges
            .iter()
            .copied()
            .map(|hinge| HingeAngle::new(hinge, 135.0).expect("valid deep-fold angle"))
            .collect(),
    )
    .expect("canonical deep-fold angles");
    let pose = fixture
        .model
        .solve(Some(fixture.model.face_ids()[0]), &angles)
        .expect("deep-fold triangular pose");
    let total_legacy_triangle_pairs = fixture
        .model
        .face_ids()
        .iter()
        .copied()
        .enumerate()
        .flat_map(|(first_index, first)| {
            fixture.model.face_ids()[first_index + 1..]
                .iter()
                .copied()
                .map(move |second| [first, second])
        })
        .map(|[first, second]| {
            let first_triangles = fixture
                .model
                .face_boundary(first)
                .expect("first boundary")
                .vertices()
                .len()
                - 2;
            let second_triangles = fixture
                .model
                .face_boundary(second)
                .expect("second boundary")
                .vertices()
                .len()
                - 2;
            first_triangles * second_triangles
        })
        .sum::<usize>();

    assert_error(
        prove_static_collision_geometry(
            &fixture.model,
            &pose,
            0.0,
            StaticCollisionLimits {
                max_total_triangle_pairs: total_legacy_triangle_pairs,
                ..StaticCollisionLimits::default()
            },
        ),
        StaticCollisionError::ResourceLimitExceeded,
    );
    assert_error(
        prove_static_collision_geometry(
            &fixture.model,
            &pose,
            0.0,
            StaticCollisionLimits::default(),
        ),
        StaticCollisionError::ProvenTransversalPenetration {
            expected_unordered_face_pairs: 3,
            proven_transversal_pairs: 1,
            first_proven_transversal_pair: expected_pair,
        },
    );
}

#[test]
fn signed_zero_keeps_the_existing_contract_and_positive_thickness_has_its_own_reason() {
    let fixture = midpoint_mountain_400mm_fixture(false);
    let angles = CanonicalHingeAngles::new(
        fixture
            .hinges
            .iter()
            .copied()
            .map(|hinge| HingeAngle::new(hinge, 135.0).expect("valid midpoint angle"))
            .collect(),
    )
    .expect("canonical midpoint angles");
    let pose = fixture
        .model
        .solve(Some(fixture.model.face_ids()[0]), &angles)
        .expect("folded midpoint pose");

    // Signed negative zero retains the previous fail-closed result and cannot
    // inherit either affirmative reason.
    assert_error(
        prove_static_collision_geometry(
            &fixture.model,
            &pose,
            -0.0,
            StaticCollisionLimits::default(),
        ),
        StaticCollisionError::PairEvidenceUnavailable {
            expected_unordered_face_pairs: 3,
        },
    );
    assert_error(
        prove_static_collision_geometry(
            &fixture.model,
            &pose,
            0.1,
            StaticCollisionLimits::default(),
        ),
        StaticCollisionError::ProvenPositiveThicknessPenetration {
            expected_unordered_face_pairs: 3,
            proven_positive_thickness_pairs: 1,
            first_proven_positive_thickness_pair: only_non_hinge_face_pair(&fixture.model),
        },
    );

    // The three legacy triangle-pair classifications consume the complete
    // caller budget before the three-pair Cayley bridge starts. Reusing that
    // budget as a fresh allowance would silently double the configured work.
    assert_error(
        prove_static_collision_geometry(
            &fixture.model,
            &pose,
            0.0,
            StaticCollisionLimits {
                max_total_triangle_pairs: 3,
                ..StaticCollisionLimits::default()
            },
        ),
        StaticCollisionError::ResourceLimitExceeded,
    );
    assert_error(
        prove_static_collision_geometry(
            &fixture.model,
            &pose,
            0.0,
            StaticCollisionLimits {
                max_total_triangles: 3,
                ..StaticCollisionLimits::default()
            },
        ),
        StaticCollisionError::ResourceLimitExceeded,
    );
    assert_error(
        prove_static_collision_geometry(
            &fixture.model,
            &pose,
            0.0,
            StaticCollisionLimits {
                max_total_triangles: 6,
                max_total_triangle_pairs: 6,
                ..StaticCollisionLimits::default()
            },
        ),
        StaticCollisionError::ProvenTransversalPenetration {
            expected_unordered_face_pairs: 3,
            proven_transversal_pairs: 1,
            first_proven_transversal_pair: only_non_hinge_face_pair(&fixture.model),
        },
    );
}

#[test]
fn positive_thickness_mid_surface_reason_never_uses_weaker_or_unbound_evidence() {
    let midpoint = midpoint_mountain_400mm_fixture(false);
    let midpoint_angles = CanonicalHingeAngles::new(
        midpoint
            .hinges
            .iter()
            .copied()
            .map(|hinge| HingeAngle::new(hinge, 135.0).expect("valid midpoint angle"))
            .collect(),
    )
    .expect("canonical midpoint angles");
    let first_pose = midpoint
        .model
        .solve(Some(midpoint.model.face_ids()[0]), &midpoint_angles)
        .expect("first midpoint pose");
    let aba_pose = midpoint
        .model
        .solve(Some(midpoint.model.face_ids()[0]), &midpoint_angles)
        .expect("same-angle ABA midpoint pose");
    assert!(!first_pose.same_instance(&aba_pose));
    for pose in [&first_pose, &aba_pose] {
        assert_error(
            prove_static_collision_geometry(
                &midpoint.model,
                pose,
                1.0,
                StaticCollisionLimits {
                    max_total_triangles: 3,
                    ..StaticCollisionLimits::default()
                },
            ),
            StaticCollisionError::ResourceLimitExceeded,
        );
    }

    let one_hinge = fixture(true);
    let one_hinge_model = model(&one_hinge);
    let hinge = one_hinge.hinge.expect("one hinge");
    let one_hinge_angles = CanonicalHingeAngles::new(vec![
        HingeAngle::new(hinge, 135.0).expect("valid hinge angle"),
    ])
    .expect("canonical one-hinge angle");
    let one_hinge_pose = one_hinge_model
        .solve(Some(one_hinge_model.face_ids()[0]), &one_hinge_angles)
        .expect("one-hinge pose");
    assert_error(
        prove_static_collision_geometry(
            &one_hinge_model,
            &one_hinge_pose,
            1.0,
            StaticCollisionLimits::default(),
        ),
        StaticCollisionError::PairEvidenceUnavailable {
            expected_unordered_face_pairs: 1,
        },
    );

    let nontriangle = corner_mountain_mountain_quadrilateral_400mm_fixture(false);
    let nontriangle_angles = CanonicalHingeAngles::new(
        nontriangle
            .hinges
            .iter()
            .copied()
            .map(|hinge| HingeAngle::new(hinge, 135.0).expect("valid nontriangle angle"))
            .collect(),
    )
    .expect("canonical nontriangle angles");
    let nontriangle_pose = nontriangle
        .model
        .solve(Some(nontriangle.model.face_ids()[0]), &nontriangle_angles)
        .expect("nontriangle pose");
    assert_error(
        prove_static_collision_geometry(
            &nontriangle.model,
            &nontriangle_pose,
            1.0,
            StaticCollisionLimits::default(),
        ),
        StaticCollisionError::PairEvidenceUnavailable {
            expected_unordered_face_pairs: 3,
        },
    );
}
