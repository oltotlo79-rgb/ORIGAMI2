use ori_collision::{
    CENTERED_MID_SURFACE_THICKNESS_MODEL_V1, NATIVE_STATIC_COLLISION_GEOMETRY_PROOF_V1,
    NativeStaticCollisionGeometryProof, StaticCollisionError, StaticCollisionLimits,
    TOPOLOGY_CONTACT_POLICY_V2, prove_static_collision_geometry,
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
