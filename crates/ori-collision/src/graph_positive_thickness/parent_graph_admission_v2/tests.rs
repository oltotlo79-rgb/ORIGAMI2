use super::*;
use ori_domain::{
    CreasePattern, Edge, EdgeId, EdgeKind, Paper, Point2, ProjectId, Vertex, VertexId,
};
use ori_kinematics::{MaterialHingeGraphGeometry, TreeKinematicsLimits};
use ori_topology::{FaceExtractionInput, TopologySnapshot, analyze_faces};

fn two_face_source_v2(namespace: ProjectId) -> (CreasePattern, Paper, TopologySnapshot) {
    let ids = [
        b"bottom-left".as_slice(),
        b"bottom-middle".as_slice(),
        b"bottom-right".as_slice(),
        b"top-right".as_slice(),
        b"top-middle".as_slice(),
        b"top-left".as_slice(),
    ]
    .map(|name| VertexId::derive_v5(namespace, name));
    let vertices = [
        (0.0, 0.0),
        (1.0, 0.0),
        (2.0, 0.0),
        (2.0, 1.0),
        (1.0, 1.0),
        (0.0, 1.0),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, (x, y))| Vertex {
        id: ids[index],
        position: Point2::new(x, y),
    })
    .collect::<Vec<_>>();
    let edge_specs = [
        (0, 1, EdgeKind::Boundary),
        (1, 2, EdgeKind::Boundary),
        (2, 3, EdgeKind::Boundary),
        (3, 4, EdgeKind::Boundary),
        (4, 5, EdgeKind::Boundary),
        (5, 0, EdgeKind::Boundary),
        (1, 4, EdgeKind::Mountain),
    ];
    let edges = edge_specs
        .into_iter()
        .enumerate()
        .map(|(index, (start, end, kind))| Edge {
            id: EdgeId::derive_v5(namespace, &[0xd0, index as u8]),
            start: ids[start],
            end: ids[end],
            kind,
        })
        .collect();
    let pattern = CreasePattern { vertices, edges };
    let paper = Paper {
        boundary_vertices: ids.to_vec(),
        ..Paper::default()
    };
    let topology = analyze_faces(FaceExtractionInput {
        identity_namespace: namespace,
        source_revision: 7,
        paper: &paper,
        pattern: &pattern,
    })
    .snapshot
    .expect("two-face topology");
    (pattern, paper, topology)
}

fn two_face_geometry_v2(namespace: ProjectId) -> MaterialHingeGraphGeometry {
    let (pattern, paper, topology) = two_face_source_v2(namespace);
    MaterialHingeGraphGeometry::prepare(
        &pattern,
        &paper,
        &topology,
        TreeKinematicsLimits::default(),
    )
    .expect("two-face parent geometry")
}

#[test]
fn exact_limits_one_short_cancel_revalidate_and_tamper_fail_closed_v2() {
    let namespace = ProjectId::schema_namespace([
        0x4f, 0x52, 0x49, 0x47, 0x41, 0x4d, 0x49, 0x32, 0x5f, 0x50, 0x47, 0x41, 0, 0, 0, 2,
    ]);
    let geometry = two_face_geometry_v2(namespace);
    let broad = admit_common_articulation_positive_thickness_parent_graph_v2(
        &geometry,
        CommonArticulationPositiveThicknessParentGraphAdmissionLimitsV2::default(),
    )
    .expect("two exact adjacent material faces are planar");
    let exact = broad.resources_v2().exact_limits_v2();
    let mut admission =
        admit_common_articulation_positive_thickness_parent_graph_v2(&geometry, exact)
            .expect("the inclusive observed envelope is admitted");
    revalidate_common_articulation_positive_thickness_parent_graph_admission_v2(
        &admission, &geometry,
    )
    .expect("unchanged live geometry revalidates");

    let one_short = [
        CommonArticulationPositiveThicknessParentGraphAdmissionLimitsV2 {
            max_faces: exact.max_faces - 1,
            ..exact
        },
        CommonArticulationPositiveThicknessParentGraphAdmissionLimitsV2 {
            max_hinges: exact.max_hinges - 1,
            ..exact
        },
        CommonArticulationPositiveThicknessParentGraphAdmissionLimitsV2 {
            max_boundary_vertex_occurrences: exact.max_boundary_vertex_occurrences - 1,
            ..exact
        },
        CommonArticulationPositiveThicknessParentGraphAdmissionLimitsV2 {
            max_vertices: exact.max_vertices - 1,
            ..exact
        },
        CommonArticulationPositiveThicknessParentGraphAdmissionLimitsV2 {
            max_edges: exact.max_edges - 1,
            ..exact
        },
        CommonArticulationPositiveThicknessParentGraphAdmissionLimitsV2 {
            max_vertex_pairs: exact.max_vertex_pairs - 1,
            ..exact
        },
        CommonArticulationPositiveThicknessParentGraphAdmissionLimitsV2 {
            max_vertex_edge_tests: exact.max_vertex_edge_tests - 1,
            ..exact
        },
        CommonArticulationPositiveThicknessParentGraphAdmissionLimitsV2 {
            max_edge_pair_tests: exact.max_edge_pair_tests - 1,
            ..exact
        },
        CommonArticulationPositiveThicknessParentGraphAdmissionLimitsV2 {
            max_face_pair_tests: exact.max_face_pair_tests - 1,
            ..exact
        },
        CommonArticulationPositiveThicknessParentGraphAdmissionLimitsV2 {
            max_point_in_polygon_edge_tests: exact.max_point_in_polygon_edge_tests - 1,
            ..exact
        },
        CommonArticulationPositiveThicknessParentGraphAdmissionLimitsV2 {
            max_exact_operations: exact.max_exact_operations - 1,
            ..exact
        },
        CommonArticulationPositiveThicknessParentGraphAdmissionLimitsV2 {
            max_logical_work: exact.max_logical_work - 1,
            ..exact
        },
        CommonArticulationPositiveThicknessParentGraphAdmissionLimitsV2 {
            max_workspace_bytes: exact.max_workspace_bytes - 1,
            ..exact
        },
    ];
    for limits in one_short {
        assert_eq!(
            admit_common_articulation_positive_thickness_parent_graph_v2(&geometry, limits)
                .unwrap_err(),
            CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::ResourceLimit
        );
    }

    assert_eq!(
        admit_common_articulation_positive_thickness_parent_graph_with_checkpoint_v2(
            &geometry,
            exact,
            || Err(CommonArticulationPositiveThicknessParentGraphAdmissionStopV2::Cancelled),
        )
        .unwrap_err(),
        CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::Cancelled
    );
    let mut polls = 0usize;
    assert_eq!(
        admit_common_articulation_positive_thickness_parent_graph_with_checkpoint_v2(
            &geometry,
            exact,
            || {
                polls += 1;
                if polls == 2 {
                    Err(
                        CommonArticulationPositiveThicknessParentGraphAdmissionStopV2::DeadlineExceeded,
                    )
                } else {
                    Ok(())
                }
            },
        )
        .unwrap_err(),
        CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::DeadlineExceeded
    );

    let original_resources = admission.resources;
    let original_binding = admission.binding_fingerprint;
    admission.resources.logical_work += 1;
    let resource_tamper_binding = admission_binding_fingerprint_v2(
        admission.identity_namespace,
        admission.source_revision,
        admission.fold_model_fingerprint,
        admission.semantic_graph_digest,
        admission.limits,
        admission.resources,
    )
    .unwrap();
    assert_ne!(resource_tamper_binding, original_binding);
    assert_eq!(
        revalidate_common_articulation_positive_thickness_parent_graph_admission_v2(
            &admission,
            &geometry,
        ),
        Err(
            CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::AdmissionBindingMismatch
        )
    );
    admission.resources = original_resources;

    let (mut reordered_pattern, mut reordered_paper, _) = two_face_source_v2(namespace);
    reordered_pattern.vertices.reverse();
    reordered_pattern.edges.reverse();
    reordered_paper.boundary_vertices.rotate_left(3);
    let reordered_topology = analyze_faces(FaceExtractionInput {
        identity_namespace: namespace,
        source_revision: 7,
        paper: &reordered_paper,
        pattern: &reordered_pattern,
    })
    .snapshot
    .expect("storage-permuted two-face topology");
    let foreign = MaterialHingeGraphGeometry::prepare(
        &reordered_pattern,
        &reordered_paper,
        &reordered_topology,
        TreeKinematicsLimits::default(),
    )
    .expect("storage-permuted equal geometry");
    let foreign_admission = admit_common_articulation_positive_thickness_parent_graph_v2(
        &foreign,
        CommonArticulationPositiveThicknessParentGraphAdmissionLimitsV2::default(),
    )
    .expect("order-invariant semantic graph");
    assert_eq!(
        foreign_admission.semantic_graph_digest_v2(),
        broad.semantic_graph_digest_v2()
    );
    assert_eq!(
        revalidate_common_articulation_positive_thickness_parent_graph_admission_v2(
            &admission,
            &foreign,
        ),
        Err(
            CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::AdmissionBindingMismatch
        ),
        "equal semantics from another issuer remain process-local foreign input"
    );

    admission.semantic_graph_digest[0] ^= 1;
    assert_eq!(
        revalidate_common_articulation_positive_thickness_parent_graph_admission_v2(
            &admission,
            &geometry,
        ),
        Err(
            CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::AdmissionBindingMismatch
        )
    );
}

#[test]
fn exact_predicates_reject_cross_t_contact_and_collinear_overlap_v2() {
    let point = |x: i64, z: i64| ExactPointV2 {
        x: BigRational::from_integer(x.into()),
        z: BigRational::from_integer(z.into()),
    };
    let mut meter = AdmissionMeterV2::new(
        CommonArticulationPositiveThicknessParentGraphAdmissionLimitsV2::default(),
    );
    let mut checkpoint = || Ok(());

    assert!(
        segments_intersect_closed_v2(
            &point(0, 0),
            &point(2, 2),
            &point(0, 2),
            &point(2, 0),
            &mut meter,
            &mut checkpoint,
        )
        .unwrap()
    );
    assert!(
        point_on_segment_v2(
            &point(1, 0),
            &point(0, 0),
            &point(2, 0),
            &mut meter,
            &mut checkpoint,
        )
        .unwrap()
    );
    assert!(
        segments_intersect_closed_v2(
            &point(0, 0),
            &point(3, 0),
            &point(1, 0),
            &point(4, 0),
            &mut meter,
            &mut checkpoint,
        )
        .unwrap()
    );
}

fn exact_vertex_for_test_v2(id: VertexId, x: i64, z: i64) -> ExactVertexV2 {
    ExactVertexV2 {
        id,
        point: ExactPointV2 {
            x: BigRational::from_integer(x.into()),
            z: BigRational::from_integer(z.into()),
        },
        x_bits: (x as f64).to_bits(),
        z_bits: (z as f64).to_bits(),
    }
}

fn face_record_for_test_v2(
    face: FaceId,
    boundary: Vec<VertexId>,
    vertices: &[ExactVertexV2],
) -> FaceRecordV2 {
    let boundary_indices = boundary
        .iter()
        .map(|id| {
            vertices
                .iter()
                .position(|vertex| vertex.id == *id)
                .expect("test boundary vertex")
        })
        .collect::<Vec<_>>();
    let mut bounds_indices = [boundary_indices[0]; 4];
    for index in &boundary_indices[1..] {
        if vertices[*index].point.x < vertices[bounds_indices[0]].point.x {
            bounds_indices[0] = *index;
        }
        if vertices[*index].point.x > vertices[bounds_indices[1]].point.x {
            bounds_indices[1] = *index;
        }
        if vertices[*index].point.z < vertices[bounds_indices[2]].point.z {
            bounds_indices[2] = *index;
        }
        if vertices[*index].point.z > vertices[bounds_indices[3]].point.z {
            bounds_indices[3] = *index;
        }
    }
    FaceRecordV2 {
        face,
        digest_boundary: boundary.clone(),
        boundary,
        boundary_indices,
        bounds_indices,
    }
}

#[test]
fn concave_adjacent_faces_pass_local_wedges_but_same_side_fails_v2() {
    let namespace = ProjectId::schema_namespace([
        0x4f, 0x52, 0x49, 0x47, 0x41, 0x4d, 0x49, 0x32, 0x5f, 0x57, 0x45, 0x44, 0, 0, 0, 2,
    ]);
    let ids = (0_u8..10)
        .map(|index| VertexId::derive_v5(namespace, &[index]))
        .collect::<Vec<_>>();
    let vertices = [
        (0, 0),
        (4, 0),
        (5, -1),
        (5, 2),
        (6, 2),
        (6, -2),
        (-1, -2),
        (2, 1),
        (2, -1),
        (20, 20),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, (x, z))| exact_vertex_for_test_v2(ids[index], x, z))
    .collect::<Vec<_>>();
    let first_face_id = FaceId::derive_v5(namespace, b"concave-forward");
    let second_face_id = FaceId::derive_v5(namespace, b"opposite-side");
    let first = face_record_for_test_v2(
        first_face_id,
        vec![ids[0], ids[1], ids[2], ids[3], ids[4], ids[5], ids[6]],
        &vertices,
    );
    let second = face_record_for_test_v2(second_face_id, vec![ids[1], ids[0], ids[7]], &vertices);
    let edge = GraphEdgeV2 {
        first: ids[0],
        second: ids[1],
        first_index: 0,
        second_index: 1,
        first_face: first_face_id,
        second_face: Some(second_face_id),
        first_forward: true,
        second_forward: false,
        has_canonical_hinge: true,
    };
    let mut meter = AdmissionMeterV2::new(
        CommonArticulationPositiveThicknessParentGraphAdmissionLimitsV2::default(),
    );
    let mut checkpoint = || Ok(());
    validate_exact_face_geometry_v2(&first, &vertices, &mut meter, &mut checkpoint)
        .expect("simple clockwise concave face");
    validate_exact_face_geometry_v2(&second, &vertices, &mut meter, &mut checkpoint)
        .expect("simple clockwise opposite face");
    validate_face_pair_shared_features_v2(
        &first,
        &second,
        Some(edge),
        &vertices,
        &mut meter,
        &mut checkpoint,
    )
    .expect("exactly one shared edge feature");
    validate_adjacent_face_half_planes_v2(
        &first,
        &second,
        edge,
        &vertices,
        &mut meter,
        &mut checkpoint,
    )
    .expect("remote concave wrap does not replace local wedge classification");
    assert!(
        !face_has_strictly_contained_vertex_v2(
            &first,
            &second,
            &vertices,
            &mut meter,
            &mut checkpoint,
        )
        .unwrap()
    );
    assert!(
        !face_has_strictly_contained_vertex_v2(
            &second,
            &first,
            &vertices,
            &mut meter,
            &mut checkpoint,
        )
        .unwrap()
    );

    let same_side =
        face_record_for_test_v2(second_face_id, vec![ids[1], ids[0], ids[8]], &vertices);
    assert_eq!(
        validate_adjacent_face_half_planes_v2(
            &first,
            &same_side,
            edge,
            &vertices,
            &mut meter,
            &mut checkpoint,
        ),
        Err(CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::NonPlanarProjection)
    );
}

#[test]
fn shared_vertex_overlapping_interior_wedges_fail_closed_v2() {
    let namespace = ProjectId::schema_namespace([
        0x4f, 0x52, 0x49, 0x47, 0x41, 0x4d, 0x49, 0x32, 0x5f, 0x57, 0x45, 0x44, 0, 0, 0, 3,
    ]);
    let ids = (0_u8..7)
        .map(|index| VertexId::derive_v5(namespace, &[index]))
        .collect::<Vec<_>>();
    let vertices = [(0, 0), (2, 0), (2, -2), (0, -2), (2, 1), (1, -2), (-2, 1)]
        .into_iter()
        .enumerate()
        .map(|(index, (x, z))| exact_vertex_for_test_v2(ids[index], x, z))
        .collect::<Vec<_>>();
    let first = face_record_for_test_v2(
        FaceId::derive_v5(namespace, b"first-wedge"),
        vec![ids[0], ids[1], ids[2], ids[3]],
        &vertices,
    );
    let overlapping = face_record_for_test_v2(
        FaceId::derive_v5(namespace, b"overlapping-wedge"),
        vec![ids[0], ids[4], ids[5]],
        &vertices,
    );
    let mut meter = AdmissionMeterV2::new(
        CommonArticulationPositiveThicknessParentGraphAdmissionLimitsV2::default(),
    );
    let mut checkpoint = || Ok(());
    assert!(
        shared_vertex_wedges_overlap_v2(
            &first,
            &overlapping,
            ids[0],
            &vertices,
            &mut meter,
            &mut checkpoint,
        )
        .unwrap()
    );
    assert_eq!(
        validate_face_pair_shared_features_v2(
            &first,
            &overlapping,
            None,
            &vertices,
            &mut meter,
            &mut checkpoint,
        ),
        Err(CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::NonPlanarProjection)
    );
}

#[test]
fn exact_axis_and_upstream_malformed_sources_fail_closed_v2() {
    let point = |x: i64, z: i64| ExactPointV2 {
        x: BigRational::from_integer(x.into()),
        z: BigRational::from_integer(z.into()),
    };
    let mut meter = AdmissionMeterV2::new(
        CommonArticulationPositiveThicknessParentGraphAdmissionLimitsV2::default(),
    );
    let mut checkpoint = || Ok(());
    validate_exact_hinge_axis_v2(
        &point(0, 0),
        &point(2, 0),
        Point3::new(1.0, 0.0, 0.0).unwrap(),
        &mut meter,
        &mut checkpoint,
    )
    .expect("exact forward-parallel hinge axis");
    assert_eq!(
        validate_exact_hinge_axis_v2(
            &point(0, 0),
            &point(2, 0),
            Point3::new(1.0, 0.0, 1.0).unwrap(),
            &mut meter,
            &mut checkpoint,
        ),
        Err(CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::InvalidInput)
    );

    let namespace = ProjectId::new();
    let (pattern, paper, topology) = two_face_source_v2(namespace);
    let mut duplicate = pattern.clone();
    duplicate.vertices[1].position = duplicate.vertices[0].position;
    assert!(
        MaterialHingeGraphGeometry::prepare(
            &duplicate,
            &paper,
            &topology,
            TreeKinematicsLimits::default(),
        )
        .is_err()
    );
    let mut zero_edge = pattern.clone();
    zero_edge.edges[0].end = zero_edge.edges[0].start;
    assert!(
        MaterialHingeGraphGeometry::prepare(
            &zero_edge,
            &paper,
            &topology,
            TreeKinematicsLimits::default(),
        )
        .is_err()
    );
    let mut nonfinite = pattern.clone();
    nonfinite.vertices[0].position.x = f64::NAN;
    assert!(
        MaterialHingeGraphGeometry::prepare(
            &nonfinite,
            &paper,
            &topology,
            TreeKinematicsLimits::default(),
        )
        .is_err()
    );
    let mut self_crossing = pattern;
    let first_position = self_crossing.vertices[1].position;
    self_crossing.vertices[1].position = self_crossing.vertices[4].position;
    self_crossing.vertices[4].position = first_position;
    assert!(
        MaterialHingeGraphGeometry::prepare(
            &self_crossing,
            &paper,
            &topology,
            TreeKinematicsLimits::default(),
        )
        .is_err()
    );
}

#[test]
fn canonical_general_n_parent_fixture_is_admitted_v2() {
    let fixture = crate::common_articulation_clearance_v2::test_support::miura_fixture_v2();
    let admission = admit_common_articulation_positive_thickness_parent_graph_v2(
        &fixture.geometry,
        CommonArticulationPositiveThicknessParentGraphAdmissionLimitsV2::default(),
    )
    .expect("canonical N=33 parent graph has an exact planar XZ embedding");
    assert_eq!(admission.resources_v2().face_count_v2(), 265);
    assert_eq!(admission.resources_v2().hinge_count_v2(), 396);
    assert_ne!(admission.semantic_graph_digest_v2(), [0; 32]);
}
