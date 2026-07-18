use ori_domain::{
    CreasePattern, Edge, EdgeId, EdgeKind, FaceId, Paper, Point2, ProjectId, Vertex, VertexId,
};
use ori_kinematics::{
    CALLER_EMBEDDING_OBSERVATION_MODEL_ID, CanonicalHingeAngles, HingeAngle, KinematicsError,
    MATERIAL_TREE_KINEMATICS_MODEL_ID, MaterialTreeKinematicsModel, ObservationTreeKinematicsModel,
    Point3, TreeKinematicsLimits, VertexPosition3, deterministic_sin_cos_degrees,
};
use ori_topology::{
    EdgeIncidence, FaceExtractionInput, FoldAssignment, TopologySnapshot, analyze_faces,
};

struct FoldFixture {
    pattern: CreasePattern,
    paper: Paper,
    topology: TopologySnapshot,
    hinges: Vec<EdgeId>,
    vertices: Vec<VertexId>,
}

fn fixture_vertex_id(index: u64) -> VertexId {
    serde_json::from_str(&format!("\"00000000-0000-4000-8000-{index:012x}\""))
        .expect("fixed vertex id")
}

fn fixture_edge_id(index: u64) -> EdgeId {
    serde_json::from_str(&format!("\"00000000-0000-4000-9000-{index:012x}\""))
        .expect("fixed edge id")
}

fn fixture_face_id(index: u64) -> FaceId {
    serde_json::from_str(&format!("\"00000000-0000-4000-a000-{index:012x}\""))
        .expect("fixed face id")
}

fn fixture_project_id() -> ProjectId {
    serde_json::from_str("\"00000000-0000-4000-b000-000000000001\"").expect("fixed project id")
}

fn vertex(index: u64, x: f64, y: f64) -> Vertex {
    Vertex {
        id: fixture_vertex_id(index),
        position: Point2::new(x, y),
    }
}

fn edge(index: u64, start: VertexId, end: VertexId, kind: EdgeKind) -> Edge {
    Edge {
        id: fixture_edge_id(index),
        start,
        end,
        kind,
    }
}

fn extract_fixture(
    vertices: Vec<Vertex>,
    mut edges: Vec<Edge>,
    boundary: Vec<VertexId>,
    folds: Vec<Edge>,
) -> FoldFixture {
    let hinges = folds.iter().map(|fold| fold.id).collect();
    edges.extend(folds);
    let pattern = CreasePattern { vertices, edges };
    let paper = Paper {
        boundary_vertices: boundary.clone(),
        ..Paper::default()
    };
    let report = analyze_faces(FaceExtractionInput {
        identity_namespace: fixture_project_id(),
        source_revision: 17,
        paper: &paper,
        pattern: &pattern,
    });
    assert!(report.issues.is_empty(), "{:?}", report.issues);
    FoldFixture {
        pattern,
        paper,
        topology: report.snapshot.expect("fixture topology"),
        hinges,
        vertices: boundary,
    }
}

fn single_fold_fixture(assignment: FoldAssignment) -> FoldFixture {
    let vertices = vec![
        vertex(1, 0.0, 0.0),
        vertex(2, 5.0, 0.0),
        vertex(3, 10.0, 0.0),
        vertex(4, 10.0, 10.0),
        vertex(5, 5.0, 10.0),
        vertex(6, 0.0, 10.0),
    ];
    let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
    let edges = (0..boundary.len())
        .map(|index| {
            edge(
                index as u64 + 1,
                boundary[index],
                boundary[(index + 1) % boundary.len()],
                EdgeKind::Boundary,
            )
        })
        .collect();
    let kind = match assignment {
        FoldAssignment::Mountain => EdgeKind::Mountain,
        FoldAssignment::Valley => EdgeKind::Valley,
    };
    let fold = edge(7, boundary[1], boundary[4], kind);
    extract_fixture(vertices, edges, boundary, vec![fold])
}

fn non_commuting_fixture() -> FoldFixture {
    let vertices = vec![
        vertex(1, 0.0, 0.0),
        vertex(2, 2.0, 0.0),
        vertex(3, 3.0, 1.0),
        vertex(4, 1.5, 3.0),
        vertex(5, 0.0, 2.0),
    ];
    let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
    let edges = (0..boundary.len())
        .map(|index| {
            edge(
                index as u64 + 1,
                boundary[index],
                boundary[(index + 1) % boundary.len()],
                EdgeKind::Boundary,
            )
        })
        .collect();
    let first = edge(6, boundary[0], boundary[2], EdgeKind::Mountain);
    let second = edge(7, boundary[0], boundary[3], EdgeKind::Valley);
    extract_fixture(vertices, edges, boundary, vec![first, second])
}

fn planar_fixture() -> FoldFixture {
    let vertices = vec![
        vertex(1, 0.0, 0.0),
        vertex(2, 10.0, 0.0),
        vertex(3, 10.0, 10.0),
        vertex(4, 0.0, 10.0),
    ];
    let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
    let edges = (0..boundary.len())
        .map(|index| {
            edge(
                index as u64 + 1,
                boundary[index],
                boundary[(index + 1) % boundary.len()],
                EdgeKind::Boundary,
            )
        })
        .collect();
    extract_fixture(vertices, edges, boundary, Vec::new())
}

fn model(fixture: &FoldFixture) -> MaterialTreeKinematicsModel {
    MaterialTreeKinematicsModel::prepare(
        &fixture.pattern,
        &fixture.paper,
        &fixture.topology,
        TreeKinematicsLimits::default(),
    )
    .expect("tree model")
}

fn material_position_records(pattern: &CreasePattern) -> Vec<VertexPosition3> {
    pattern
        .vertices
        .iter()
        .map(|vertex| {
            VertexPosition3::new(
                vertex.id,
                Point3::new(vertex.position.x, 0.0, -vertex.position.y)
                    .expect("finite material point"),
            )
        })
        .collect()
}

fn canonical_angles(values: &[(EdgeId, f64)]) -> CanonicalHingeAngles {
    let mut values = values
        .iter()
        .map(|(edge, angle)| HingeAngle::new(*edge, *angle).expect("valid fixture angle"))
        .collect::<Vec<_>>();
    values.sort_by_key(|angle| angle.edge().canonical_bytes());
    CanonicalHingeAngles::new(values).expect("canonical fixture vector")
}

fn hinge_faces(topology: &TopologySnapshot, edge: EdgeId) -> (FaceId, FaceId) {
    topology
        .edge_incidence
        .iter()
        .find_map(|(candidate, incidence)| {
            if *candidate != edge {
                return None;
            }
            match incidence {
                EdgeIncidence::Hinge { left, right, .. } => Some((*left, *right)),
                _ => None,
            }
        })
        .expect("hinge incidence")
}

fn multiply(first: [[f64; 3]; 3], second: [[f64; 3]; 3]) -> [[f64; 3]; 3] {
    let mut result = [[0.0; 3]; 3];
    for (row, target_row) in result.iter_mut().enumerate() {
        for (column, target) in target_row.iter_mut().enumerate() {
            *target = (0..3)
                .map(|index| first[row][index] * second[index][column])
                .sum();
        }
    }
    result
}

fn assert_matrix_close(actual: [[f64; 3]; 3], expected: [[f64; 3]; 3]) {
    for (actual, expected) in actual
        .into_iter()
        .flatten()
        .zip(expected.into_iter().flatten())
    {
        assert!(
            (actual - expected).abs() < 1.0e-12,
            "{actual} != {expected}"
        );
    }
}

#[test]
fn finite_value_constructors_normalize_zero_and_reject_invalid_values() {
    let point = Point3::new(1.0, -0.0, 3.0).expect("finite point");
    assert_eq!(point.x(), 1.0);
    assert_eq!(point.y().to_bits(), 0.0_f64.to_bits());
    assert_eq!(point.z(), 3.0);
    assert!(Point3::new(f64::NAN, 0.0, 0.0).is_err());
    assert!(Point3::new(0.0, f64::INFINITY, 0.0).is_err());

    let edge = fixture_edge_id(1);
    let zero = HingeAngle::new(edge, -0.0).expect("canonical zero");
    assert_eq!(zero.angle_degrees().to_bits(), 0.0_f64.to_bits());
    assert_eq!(
        HingeAngle::new(edge, f64::NAN),
        Err(KinematicsError::NonFiniteHingeAngle { edge })
    );
    for invalid in [-f64::EPSILON, 180.0 + 1.0e-12, f64::INFINITY] {
        assert!(HingeAngle::new(edge, invalid).is_err());
    }
}

#[test]
fn cardinal_trigonometry_and_axis_aligned_rigid_poses_are_exact() {
    assert_eq!(deterministic_sin_cos_degrees(0.0), Ok((0.0, 1.0)));
    assert_eq!(deterministic_sin_cos_degrees(-0.0), Ok((0.0, 1.0)));
    assert_eq!(deterministic_sin_cos_degrees(90.0), Ok((1.0, 0.0)));
    assert_eq!(deterministic_sin_cos_degrees(-90.0), Ok((-1.0, 0.0)));
    assert_eq!(deterministic_sin_cos_degrees(180.0), Ok((0.0, -1.0)));
    assert_eq!(
        deterministic_sin_cos_degrees(180.0 + 1.0e-12),
        Err(KinematicsError::UnrepresentableGeometry)
    );

    let fixture = single_fold_fixture(FoldAssignment::Mountain);
    let model = model(&fixture);
    let (left, right) = hinge_faces(&fixture.topology, fixture.hinges[0]);
    let off_axis = model
        .vertex_position(fixture.vertices[2])
        .expect("right off-axis vertex");
    for angle in [0.0, 90.0, 180.0] {
        let pose = model
            .solve(Some(left), &canonical_angles(&[(fixture.hinges[0], angle)]))
            .expect("cardinal pose");
        assert_eq!(
            pose.face_transform(left).expect("root transform"),
            model.identity_transform()
        );
        let moving = pose.face_transform(right).expect("moving transform");
        if angle == 0.0 {
            assert_eq!(moving, model.identity_transform());
        }
        for value in moving.rotation_rows().into_iter().flatten() {
            assert_eq!(value, value.round(), "axis-aligned cardinal matrix");
        }
        let hinge = &model.hinges()[0];
        assert_eq!(
            moving
                .apply_point(hinge.start())
                .expect("fixed hinge start"),
            hinge.start()
        );
        assert_eq!(
            moving.apply_point(hinge.end()).expect("fixed hinge end"),
            hinge.end()
        );
        let actual = moving.apply_point(off_axis).expect("off-axis point");
        let expected = match angle {
            0.0 => Point3::new(10.0, 0.0, 0.0).expect("expected zero pose"),
            90.0 => Point3::new(5.0, -5.0, 0.0).expect("expected quarter fold"),
            180.0 => Point3::new(0.0, 0.0, 0.0).expect("expected flat fold"),
            _ => unreachable!(),
        };
        assert_eq!(actual, expected);
    }
}

#[test]
fn mountain_valley_sign_uses_canonical_left_right_and_reroots() {
    let mountain_fixture = single_fold_fixture(FoldAssignment::Mountain);
    let valley_fixture = single_fold_fixture(FoldAssignment::Valley);
    let mountain = model(&mountain_fixture);
    let valley = model(&valley_fixture);
    let edge = mountain_fixture.hinges[0];
    let (left, right) = hinge_faces(&mountain_fixture.topology, edge);
    assert_eq!(hinge_faces(&valley_fixture.topology, edge), (left, right));
    let source = mountain
        .vertex_position(mountain_fixture.vertices[2])
        .expect("right-side point");
    let angles = canonical_angles(&[(edge, 90.0)]);
    let mountain_pose = mountain.solve(Some(left), &angles).expect("mountain pose");
    let valley_pose = valley.solve(Some(left), &angles).expect("valley pose");
    let mountain_point = mountain_pose
        .face_transform(right)
        .expect("mountain moving face")
        .apply_point(source)
        .expect("mountain point");
    let valley_point = valley_pose
        .face_transform(right)
        .expect("valley moving face")
        .apply_point(source)
        .expect("valley point");
    assert_eq!(mountain_point.x(), valley_point.x());
    assert_eq!(mountain_point.z(), valley_point.z());
    assert_eq!(mountain_point.y(), -valley_point.y());
    assert!(mountain_point.y() < 0.0);

    let rerooted = mountain.solve(Some(right), &angles).expect("rerooted pose");
    assert_eq!(
        rerooted.face_transform(right).expect("new root"),
        mountain.identity_transform()
    );
    assert_ne!(
        rerooted.face_transform(left).expect("opposite moving face"),
        mountain.identity_transform()
    );

    for (fixture, expected_negative) in [(&mountain_fixture, true), (&valley_fixture, false)] {
        let model = model(fixture);
        let edge = fixture.hinges[0];
        let (left, right) = hinge_faces(&fixture.topology, edge);
        for (root, moving, source_vertex) in [
            (left, right, fixture.vertices[2]),
            (right, left, fixture.vertices[0]),
        ] {
            let pose = model
                .solve(Some(root), &canonical_angles(&[(edge, 90.0)]))
                .expect("left/right rooted pose");
            assert_eq!(
                pose.face_transform(root).expect("root transform"),
                model.identity_transform()
            );
            let transformed = pose
                .face_transform(moving)
                .expect("moving transform")
                .apply_point(
                    model
                        .vertex_position(source_vertex)
                        .expect("off-axis source"),
                )
                .expect("off-axis transformed");
            assert_eq!(transformed.y().is_sign_negative(), expected_negative);
            assert_ne!(transformed.y(), 0.0);
        }
    }
}

#[test]
fn non_parallel_multi_hinge_pose_composes_parent_before_local() {
    let fixture = non_commuting_fixture();
    assert_eq!(fixture.topology.faces.len(), 3);
    assert_eq!(fixture.hinges.len(), 2);
    let model = model(&fixture);
    let first = fixture.hinges[0];
    let second = fixture.hinges[1];
    let (first_left, first_right) = hinge_faces(&fixture.topology, first);
    let (second_left, second_right) = hinge_faces(&fixture.topology, second);
    let root = if first_left == second_left || first_left == second_right {
        first_right
    } else {
        first_left
    };
    let far = if second_left == first_left || second_left == first_right {
        second_right
    } else {
        second_left
    };
    let first_only = model
        .solve(
            Some(root),
            &canonical_angles(&[(first, 41.0), (second, 0.0)]),
        )
        .expect("first-only pose");
    let second_only = model
        .solve(
            Some(root),
            &canonical_angles(&[(first, 0.0), (second, 63.0)]),
        )
        .expect("second-only pose");
    let combined = model
        .solve(
            Some(root),
            &canonical_angles(&[(first, 41.0), (second, 63.0)]),
        )
        .expect("combined pose");
    let first_rotation = first_only
        .face_transform(far)
        .expect("far follows first hinge")
        .rotation_rows();
    let second_rotation = second_only
        .face_transform(far)
        .expect("far follows second hinge")
        .rotation_rows();
    let combined_rotation = combined
        .face_transform(far)
        .expect("far combined transform")
        .rotation_rows();
    assert_matrix_close(combined_rotation, multiply(first_rotation, second_rotation));
    let reversed = multiply(second_rotation, first_rotation);
    assert!(
        combined_rotation
            .into_iter()
            .flatten()
            .zip(reversed.into_iter().flatten())
            .any(|(actual, wrong)| (actual - wrong).abs() > 1.0e-6),
        "non-parallel rotations must not commute"
    );
}

#[test]
fn source_storage_topology_storage_and_edge_direction_do_not_change_pose() {
    let fixture = non_commuting_fixture();
    let baseline = model(&fixture);
    let root = fixture.topology.faces[0].id;
    let angles = canonical_angles(&[(fixture.hinges[0], 37.25), (fixture.hinges[1], 82.5)]);
    let expected = baseline.solve(Some(root), &angles).expect("baseline pose");
    assert!(
        baseline
            .face_ids()
            .windows(2)
            .all(|pair| pair[0].canonical_bytes() < pair[1].canonical_bytes())
    );
    assert!(
        baseline
            .hinges()
            .windows(2)
            .all(|pair| pair[0].edge().canonical_bytes() < pair[1].edge().canonical_bytes())
    );

    let mut pattern = fixture.pattern.clone();
    pattern.vertices.reverse();
    pattern.edges.reverse();
    for source in &mut pattern.edges {
        std::mem::swap(&mut source.start, &mut source.end);
    }
    let mut topology = fixture.topology.clone();
    topology.faces.reverse();
    for face in &mut topology.faces {
        face.outer.half_edges.rotate_left(1);
    }
    topology.edge_incidence.reverse();
    topology.hinge_adjacency.reverse();
    let reordered = MaterialTreeKinematicsModel::prepare(
        &pattern,
        &fixture.paper,
        &topology,
        TreeKinematicsLimits::default(),
    )
    .expect("reordered model");
    assert_eq!(baseline.face_ids(), reordered.face_ids());
    assert_eq!(
        baseline
            .hinges()
            .iter()
            .map(|hinge| hinge.edge())
            .collect::<Vec<_>>(),
        reordered
            .hinges()
            .iter()
            .map(|hinge| hinge.edge())
            .collect::<Vec<_>>()
    );
    let actual = reordered
        .solve(Some(root), &angles)
        .expect("reordered pose");
    for face in baseline.face_ids() {
        assert_eq!(
            actual.face_transform(*face),
            expected.face_transform(*face),
            "face {face:?}"
        );
    }
    for hinge in baseline.hinges() {
        assert_eq!(
            actual.hinge_parent_transform(hinge.edge()),
            expected.hinge_parent_transform(hinge.edge()),
            "hinge {:?}",
            hinge.edge()
        );
    }
}

fn material_face_boundary_signatures(
    model: &MaterialTreeKinematicsModel,
) -> Vec<(FaceId, Vec<VertexId>, Vec<EdgeId>)> {
    model
        .face_ids()
        .iter()
        .map(|face| {
            let boundary = model
                .face_boundary(*face)
                .expect("registered face boundary");
            (
                boundary.face(),
                boundary.vertices().to_vec(),
                boundary.edges().to_vec(),
            )
        })
        .collect()
}

#[test]
fn material_face_boundaries_are_canonical_pose_consistent_and_issuer_bound() {
    let fixture = non_commuting_fixture();
    let material = model(&fixture);
    let angles = canonical_angles(
        &fixture
            .hinges
            .iter()
            .copied()
            .map(|edge| (edge, 0.0))
            .collect::<Vec<_>>(),
    );
    let fixed_face = material.face_ids()[0];
    let pose = material
        .solve(Some(fixed_face), &angles)
        .expect("material pose");

    for face in material.face_ids() {
        let from_model = material.face_boundary(*face).expect("model boundary");
        let from_pose = pose.face_boundary(*face).expect("pose boundary");
        let bound = material.bind_pose(&pose).expect("bound pose");
        let from_bound = bound.face_boundary(*face).expect("bound boundary");

        assert_eq!(from_model, from_pose);
        assert_eq!(from_model, from_bound);
        assert_eq!(from_model.face(), *face);
        assert_eq!(from_model.vertices().len(), from_model.edges().len());
        assert!(from_model.vertices().len() >= 3);
        assert!(material.owns_face_boundary(from_model));
        assert!(pose.owns_face_boundary(from_model));
    }
    let unknown = fixture_face_id(999);
    assert!(material.face_boundary(unknown).is_none());
    assert!(pose.face_boundary(unknown).is_none());

    let independent = model(&fixture);
    let independent_pose = independent
        .solve(Some(fixed_face), &angles)
        .expect("second material pose");
    let face = material.face_ids()[0];
    let first = material.face_boundary(face).expect("first boundary");
    let second = independent.face_boundary(face).expect("second boundary");
    assert_ne!(first, second, "separate preparation is a separate issuer");
    assert!(!independent.owns_face_boundary(first));
    assert!(!independent_pose.owns_face_boundary(first));
    assert!(!material.owns_face_boundary(second));
    assert!(!pose.owns_face_boundary(second));
}

#[test]
fn material_face_boundary_registry_ignores_cycle_start_and_source_edge_direction() {
    let fixture = non_commuting_fixture();
    let baseline = model(&fixture);
    let expected = material_face_boundary_signatures(&baseline);

    for (_, vertices, edges) in &expected {
        let tokens = edges
            .iter()
            .enumerate()
            .map(|(index, edge)| {
                (
                    edge.canonical_bytes(),
                    vertices[index].canonical_bytes(),
                    vertices[(index + 1) % vertices.len()].canonical_bytes(),
                )
            })
            .collect::<Vec<_>>();
        let minimum = tokens.iter().min().expect("nonempty face");
        assert_eq!(&tokens[0], minimum, "cycle start must be canonical");
    }

    let mut pattern = fixture.pattern.clone();
    pattern.vertices.reverse();
    pattern.edges.reverse();
    for source in &mut pattern.edges {
        std::mem::swap(&mut source.start, &mut source.end);
    }
    let mut topology = fixture.topology.clone();
    topology.faces.reverse();
    for (index, face) in topology.faces.iter_mut().enumerate() {
        let length = face.outer.half_edges.len();
        face.outer.half_edges.rotate_left((index + 1) % length);
    }
    topology.edge_incidence.reverse();
    topology.hinge_adjacency.reverse();

    let reordered = MaterialTreeKinematicsModel::prepare(
        &pattern,
        &fixture.paper,
        &topology,
        TreeKinematicsLimits::default(),
    )
    .expect("reordered model");
    assert_eq!(material_face_boundary_signatures(&reordered), expected);
}

#[test]
fn material_face_boundaries_preserve_shared_vertex_and_shared_hinge_relations() {
    let fixture = non_commuting_fixture();
    let material = model(&fixture);

    for hinge in material.hinges() {
        let left = material
            .face_boundary(hinge.left_face())
            .expect("left face boundary");
        let right = material
            .face_boundary(hinge.right_face())
            .expect("right face boundary");
        assert!(left.edges().contains(&hinge.edge()));
        assert!(right.edges().contains(&hinge.edge()));
        let shared_vertices = left
            .vertices()
            .iter()
            .filter(|vertex| right.vertices().contains(vertex))
            .copied()
            .collect::<Vec<_>>();
        assert_eq!(
            shared_vertices.len(),
            2,
            "one shared hinge has two endpoints"
        );
    }

    let mut found_vertex_only_pair = false;
    for (index, first_face) in material.face_ids().iter().enumerate() {
        let first = material.face_boundary(*first_face).expect("first boundary");
        for second_face in &material.face_ids()[index + 1..] {
            let second = material
                .face_boundary(*second_face)
                .expect("second boundary");
            let shared_edges = first
                .edges()
                .iter()
                .filter(|edge| second.edges().contains(edge))
                .count();
            let shared_vertices = first
                .vertices()
                .iter()
                .filter(|vertex| second.vertices().contains(vertex))
                .count();
            if shared_edges == 0 && shared_vertices == 1 {
                found_vertex_only_pair = true;
            }
        }
    }
    assert!(
        found_vertex_only_pair,
        "the V-fold fixture must retain its vertex-only face relation"
    );
}

#[test]
fn caller_coordinate_embedding_is_finite_and_uniform_scale_independent() {
    let fixture = non_commuting_fixture();
    let material = model(&fixture);
    assert_eq!(material.model_id(), MATERIAL_TREE_KINEMATICS_MODEL_ID);
    let positions = fixture
        .pattern
        .vertices
        .iter()
        .map(|vertex| {
            VertexPosition3::new(
                vertex.id,
                Point3::new(
                    vertex.position.x * 3.0 + 7.0,
                    2.0,
                    -vertex.position.y * 3.0 - 11.0,
                )
                .expect("scaled finite point"),
            )
        })
        .collect::<Vec<_>>();
    let scaled = ObservationTreeKinematicsModel::prepare_with_positions(
        &fixture.pattern,
        &fixture.paper,
        &fixture.topology,
        &positions,
        TreeKinematicsLimits::default(),
    )
    .expect("scaled embedding");
    assert_eq!(scaled.model_id(), CALLER_EMBEDDING_OBSERVATION_MODEL_ID);
    let mut reordered_positions = positions.clone();
    reordered_positions.reverse();
    let reordered_scaled = ObservationTreeKinematicsModel::prepare_with_positions(
        &fixture.pattern,
        &fixture.paper,
        &fixture.topology,
        &reordered_positions,
        TreeKinematicsLimits::default(),
    )
    .expect("reordered scaled embedding");
    let root = fixture.topology.faces[0].id;
    let angles = canonical_angles(&[(fixture.hinges[0], 31.0), (fixture.hinges[1], 77.0)]);
    let material_pose = material.solve(Some(root), &angles).expect("material pose");
    let scaled_pose = scaled.solve(Some(root), &angles).expect("scaled pose");
    let reordered_scaled_pose = reordered_scaled
        .solve(Some(root), &angles)
        .expect("reordered scaled pose");
    let vertex = fixture.vertices[1];
    let material_point = material.vertex_position(vertex).expect("material vertex");
    let scaled_point = scaled.vertex_position(vertex).expect("scaled vertex");
    for face in material.face_ids() {
        let expected = material_pose
            .face_transform(*face)
            .expect("material transform")
            .apply_point(material_point)
            .expect("material point");
        let actual = scaled_pose
            .face_transform(*face)
            .expect("scaled transform")
            .apply_point(scaled_point)
            .expect("scaled point");
        assert!((actual.x() - (expected.x() * 3.0 + 7.0)).abs() < 1.0e-11);
        assert!((actual.y() - (expected.y() * 3.0 + 2.0)).abs() < 1.0e-11);
        assert!((actual.z() - (expected.z() * 3.0 - 11.0)).abs() < 1.0e-11);
        assert_eq!(
            reordered_scaled_pose.face_transform(*face),
            scaled_pose.face_transform(*face)
        );
    }
}

#[test]
fn material_pose_retains_private_issuer_identity_and_its_own_source_geometry() {
    let fixture = non_commuting_fixture();
    let first = model(&fixture);
    let cloned_model = first.clone();
    let independently_prepared = model(&fixture);
    let root = fixture.topology.faces[0].id;
    let angles = canonical_angles(&[(fixture.hinges[0], -0.0), (fixture.hinges[1], 77.0)]);
    let pose = first.solve(Some(root), &angles).expect("first pose");
    let cloned_pose = pose.clone();

    assert_eq!(first, cloned_model);
    assert_ne!(first, independently_prepared);
    assert!(first.owns_pose(&pose));
    assert!(cloned_model.owns_pose(&pose));
    assert!(!independently_prepared.owns_pose(&pose));
    assert!(first.bind_pose(&pose).is_ok());
    assert!(cloned_model.bind_pose(&pose).is_ok());
    assert!(matches!(
        independently_prepared.bind_pose(&pose),
        Err(KinematicsError::MaterialPoseIssuerMismatch)
    ));
    assert_eq!(pose, cloned_pose);
    assert!(pose.same_instance(&cloned_pose));
    assert_eq!(pose.fixed_face(), Some(root));
    assert_eq!(pose.hinge_angles(), angles.as_slice());
    assert_eq!(
        pose.hinge_angles()[0].angle_degrees().to_bits(),
        0.0_f64.to_bits()
    );
    assert_eq!(pose.face_ids(), first.face_ids());
    assert_eq!(pose.hinges(), first.hinges());
    for vertex in &fixture.vertices {
        assert_eq!(
            pose.vertex_position(*vertex),
            first.vertex_position(*vertex)
        );
    }

    let repeated = first
        .solve(Some(root), &angles)
        .expect("separately issued same-angle pose");
    assert!(first.owns_pose(&repeated));
    assert!(!pose.same_instance(&repeated));
    assert_ne!(pose, repeated);

    let independent_pose = independently_prepared
        .solve(Some(root), &angles)
        .expect("independent pose");
    assert!(!first.owns_pose(&independent_pose));
    assert!(!pose.same_instance(&independent_pose));
    assert_ne!(pose, independent_pose);

    let mut scaled_pattern = fixture.pattern.clone();
    for vertex in &mut scaled_pattern.vertices {
        vertex.position.x *= 2.0;
        vertex.position.y *= 2.0;
    }
    let scaled_report = analyze_faces(FaceExtractionInput {
        identity_namespace: fixture_project_id(),
        source_revision: 17,
        paper: &fixture.paper,
        pattern: &scaled_pattern,
    });
    assert!(
        scaled_report.issues.is_empty(),
        "{:?}",
        scaled_report.issues
    );
    let scaled_topology = scaled_report.snapshot.expect("scaled topology");
    let scaled_model = MaterialTreeKinematicsModel::prepare(
        &scaled_pattern,
        &fixture.paper,
        &scaled_topology,
        TreeKinematicsLimits::default(),
    )
    .expect("scaled model with identical source identifiers");
    assert_eq!(first.face_ids(), scaled_model.face_ids());
    let scaled_pose = scaled_model
        .solve(Some(root), &angles)
        .expect("scaled pose");
    assert!(!first.owns_pose(&scaled_pose));
    assert!(matches!(
        first.bind_pose(&scaled_pose),
        Err(KinematicsError::MaterialPoseIssuerMismatch)
    ));
    assert_ne!(
        pose.vertex_position(fixture.vertices[1]),
        scaled_pose.vertex_position(fixture.vertices[1])
    );
}

#[test]
fn material_topology_ignores_isolated_draft_vertices_and_auxiliary_geometry() {
    let fixture = non_commuting_fixture();
    let baseline = model(&fixture);
    let root = fixture.topology.faces[0].id;
    let angles = canonical_angles(&[(fixture.hinges[0], 31.0), (fixture.hinges[1], 77.0)]);
    let expected = baseline.solve(Some(root), &angles).expect("baseline pose");

    let finite_isolated = fixture_vertex_id(98);
    let nonfinite_isolated = fixture_vertex_id(99);
    let auxiliary = edge(
        99,
        fixture_vertex_id(998),
        fixture_vertex_id(999),
        EdgeKind::Auxiliary,
    );
    let mut pattern = fixture.pattern.clone();
    pattern.vertices.push(vertex(98, 30.0, 30.0));
    pattern.vertices.push(vertex(99, f64::NAN, f64::INFINITY));
    pattern.edges.push(auxiliary.clone());
    let mut topology = fixture.topology.clone();
    topology
        .edge_incidence
        .push((auxiliary.id, EdgeIncidence::AuxiliaryIgnored));
    topology
        .edge_incidence
        .sort_unstable_by_key(|(edge, _)| edge.canonical_bytes());

    let material = MaterialTreeKinematicsModel::prepare(
        &pattern,
        &fixture.paper,
        &topology,
        TreeKinematicsLimits::default(),
    )
    .expect("material participants only");
    assert_eq!(material.face_ids(), baseline.face_ids());
    assert_eq!(material.hinges(), baseline.hinges());
    assert_eq!(material.vertex_position(finite_isolated), None);
    assert_eq!(material.vertex_position(nonfinite_isolated), None);
    let material_pose = material.solve(Some(root), &angles).expect("material pose");
    assert_eq!(material_pose.vertex_position(finite_isolated), None);
    for face in baseline.face_ids() {
        assert_eq!(
            material_pose.face_transform(*face),
            expected.face_transform(*face)
        );
    }

    let participant_positions = material_position_records(&fixture.pattern);
    let participant_only = ObservationTreeKinematicsModel::prepare_with_positions(
        &pattern,
        &fixture.paper,
        &topology,
        &participant_positions,
        TreeKinematicsLimits::default(),
    )
    .expect("participant-only observation positions");
    assert_eq!(participant_only.vertex_position(finite_isolated), None);
    let participant_pose = participant_only
        .solve(Some(root), &angles)
        .expect("participant-only pose");
    for face in baseline.face_ids() {
        assert_eq!(
            participant_pose.face_transform(*face),
            expected.face_transform(*face)
        );
    }

    let mut positions_with_isolated = participant_positions;
    positions_with_isolated.push(VertexPosition3::new(
        finite_isolated,
        Point3::new(30.0, 0.0, -30.0).expect("finite isolated observation"),
    ));
    let with_extra_position = ObservationTreeKinematicsModel::prepare_with_positions(
        &pattern,
        &fixture.paper,
        &topology,
        &positions_with_isolated,
        TreeKinematicsLimits::default(),
    )
    .expect("extra isolated observation position");
    assert_eq!(with_extra_position.vertex_position(finite_isolated), None);
}

#[test]
fn complete_canonical_angle_vector_rejects_every_mismatch() {
    let fixture = single_fold_fixture(FoldAssignment::Mountain);
    let model = model(&fixture);
    let known = fixture.hinges[0];
    let unknown = fixture_edge_id(99);
    let root = fixture.topology.faces[0].id;

    let duplicate = vec![
        HingeAngle::new(known, 10.0).expect("angle"),
        HingeAngle::new(known, 20.0).expect("angle"),
    ];
    assert_eq!(
        CanonicalHingeAngles::new(duplicate),
        Err(KinematicsError::DuplicateHingeAngle { edge: known })
    );
    let noncanonical = vec![
        HingeAngle::new(unknown, 10.0).expect("angle"),
        HingeAngle::new(known, 20.0).expect("angle"),
    ];
    assert_eq!(
        CanonicalHingeAngles::new(noncanonical),
        Err(KinematicsError::NonCanonicalHingeAngles {
            previous_edge: unknown,
            edge: known,
        })
    );

    assert_eq!(
        model.solve(
            Some(root),
            &CanonicalHingeAngles::new(Vec::new()).expect("empty canonical vector")
        ),
        Err(KinematicsError::MissingHingeAngle { edge: known })
    );
    assert_eq!(
        model.solve(
            Some(root),
            &canonical_angles(&[(known, 10.0), (unknown, 20.0)])
        ),
        Err(KinematicsError::ExtraHingeAngle { edge: unknown })
    );
    assert_eq!(
        model.solve(Some(root), &canonical_angles(&[(unknown, 10.0)])),
        Err(KinematicsError::UnknownHingeAngle { edge: unknown })
    );
    assert_eq!(
        model.solve(None, &canonical_angles(&[(known, 10.0)])),
        Err(KinematicsError::MissingFixedFace)
    );
    let unknown_face = fixture_face_id(999);
    assert_eq!(
        model.solve(Some(unknown_face), &canonical_angles(&[(known, 10.0)])),
        Err(KinematicsError::UnknownFixedFace { face: unknown_face })
    );
}

#[test]
fn planar_model_requires_no_root_and_no_angles() {
    let fixture = planar_fixture();
    let model = model(&fixture);
    let empty = CanonicalHingeAngles::new(Vec::new()).expect("empty vector");
    let pose = model.solve(None, &empty).expect("planar identity pose");
    let face = fixture.topology.faces[0].id;
    assert_eq!(pose.face_transform(face), Some(model.identity_transform()));
    assert_eq!(pose.fixed_face(), None);
    assert!(pose.hinge_angles().is_empty());
    assert_eq!(pose.face_ids(), &[face]);
    assert!(pose.hinges().is_empty());
    assert_eq!(
        model.solve(Some(face), &empty),
        Err(KinematicsError::UnexpectedFixedFace { face })
    );
    let unknown = fixture_edge_id(999);
    assert_eq!(
        model.solve(None, &canonical_angles(&[(unknown, 10.0)])),
        Err(KinematicsError::ExtraHingeAngle { edge: unknown })
    );
}

#[test]
fn paper_boundary_cycle_start_and_direction_are_observationally_invariant() {
    let fixture = non_commuting_fixture();
    let baseline = model(&fixture);
    let expected_boundaries = material_face_boundary_signatures(&baseline);
    let root = baseline.face_ids()[0];
    let angles = canonical_angles(&[(fixture.hinges[0], 19.0), (fixture.hinges[1], 73.0)]);
    let expected = baseline.solve(Some(root), &angles).expect("baseline pose");
    for mut boundary in [
        {
            let mut value = fixture.paper.boundary_vertices.clone();
            value.rotate_left(2);
            value
        },
        {
            let mut value = fixture.paper.boundary_vertices.clone();
            value.reverse();
            value
        },
    ] {
        let mut paper = fixture.paper.clone();
        paper.boundary_vertices = std::mem::take(&mut boundary);
        let actual_model = MaterialTreeKinematicsModel::prepare(
            &fixture.pattern,
            &paper,
            &fixture.topology,
            TreeKinematicsLimits::default(),
        )
        .expect("equivalent boundary cycle");
        let actual = actual_model
            .solve(Some(root), &angles)
            .expect("equivalent boundary pose");
        assert_eq!(
            material_face_boundary_signatures(&actual_model),
            expected_boundaries
        );
        for face in baseline.face_ids() {
            assert_eq!(actual.face_transform(*face), expected.face_transform(*face));
        }
    }
}

#[test]
fn malformed_source_paper_topology_and_observation_records_fail_closed() {
    let fixture = non_commuting_fixture();
    let limits = TreeKinematicsLimits::default();
    let reject_material = |pattern: &CreasePattern, paper: &Paper, topology: &TopologySnapshot| {
        assert_eq!(
            MaterialTreeKinematicsModel::prepare(pattern, paper, topology, limits),
            Err(KinematicsError::UnsupportedTopology)
        );
    };

    let mut duplicate_vertex = fixture.pattern.clone();
    duplicate_vertex
        .vertices
        .push(duplicate_vertex.vertices[0].clone());
    reject_material(&duplicate_vertex, &fixture.paper, &fixture.topology);

    let mut missing_vertex = fixture.pattern.clone();
    missing_vertex.vertices.remove(0);
    reject_material(&missing_vertex, &fixture.paper, &fixture.topology);

    let mut duplicate_edge = fixture.pattern.clone();
    duplicate_edge.edges.push(duplicate_edge.edges[0].clone());
    reject_material(&duplicate_edge, &fixture.paper, &fixture.topology);

    let mut missing_edge = fixture.pattern.clone();
    missing_edge
        .edges
        .retain(|edge| edge.id != fixture.hinges[0]);
    reject_material(&missing_edge, &fixture.paper, &fixture.topology);

    let mut extra_edge = fixture.pattern.clone();
    extra_edge.edges.push(edge(
        99,
        fixture.vertices[1],
        fixture.vertices[3],
        EdgeKind::Auxiliary,
    ));
    reject_material(&extra_edge, &fixture.paper, &fixture.topology);

    let mut duplicate_boundary = fixture.paper.clone();
    duplicate_boundary
        .boundary_vertices
        .push(duplicate_boundary.boundary_vertices[0]);
    reject_material(&fixture.pattern, &duplicate_boundary, &fixture.topology);

    let mut unknown_boundary = fixture.paper.clone();
    unknown_boundary.boundary_vertices[0] = fixture_vertex_id(999);
    reject_material(&fixture.pattern, &unknown_boundary, &fixture.topology);

    let mut too_short_boundary = fixture.paper.clone();
    too_short_boundary.boundary_vertices.truncate(2);
    reject_material(&fixture.pattern, &too_short_boundary, &fixture.topology);

    let mut boundary_edge_mismatch = fixture.pattern.clone();
    let boundary_edge = boundary_edge_mismatch
        .edges
        .iter_mut()
        .find(|edge| edge.kind == EdgeKind::Boundary)
        .expect("boundary edge");
    boundary_edge.end = fixture.vertices[3];
    reject_material(&boundary_edge_mismatch, &fixture.paper, &fixture.topology);

    for invalid_thickness in [-0.1, f64::NAN, f64::INFINITY] {
        let mut paper = fixture.paper.clone();
        paper.thickness_mm = invalid_thickness;
        assert_eq!(
            MaterialTreeKinematicsModel::prepare(
                &fixture.pattern,
                &paper,
                &fixture.topology,
                limits,
            ),
            Err(KinematicsError::UnrepresentableGeometry)
        );
    }

    let mut duplicate_face_id = fixture.topology.clone();
    duplicate_face_id.faces[1].id = duplicate_face_id.faces[0].id;
    reject_material(&fixture.pattern, &fixture.paper, &duplicate_face_id);

    let mut duplicate_face_key = fixture.topology.clone();
    duplicate_face_key.faces[1].key = duplicate_face_key.faces[0].key;
    reject_material(&fixture.pattern, &fixture.paper, &duplicate_face_key);

    let mut duplicate_incidence = fixture.topology.clone();
    duplicate_incidence
        .edge_incidence
        .push(duplicate_incidence.edge_incidence[0]);
    reject_material(&fixture.pattern, &fixture.paper, &duplicate_incidence);

    let mut missing_incidence = fixture.topology.clone();
    missing_incidence.edge_incidence.remove(0);
    reject_material(&fixture.pattern, &fixture.paper, &missing_incidence);

    let mut unknown_incidence = fixture.topology.clone();
    unknown_incidence.edge_incidence[0].0 = fixture_edge_id(999);
    reject_material(&fixture.pattern, &fixture.paper, &unknown_incidence);

    let mut swapped_hinge_incidence = fixture.topology.clone();
    let (_, incidence) = swapped_hinge_incidence
        .edge_incidence
        .iter_mut()
        .find(|(edge, _)| *edge == fixture.hinges[0])
        .expect("hinge incidence");
    let EdgeIncidence::Hinge {
        left,
        right,
        assignment: _,
    } = incidence
    else {
        panic!("hinge incidence fixture");
    };
    std::mem::swap(left, right);
    reject_material(&fixture.pattern, &fixture.paper, &swapped_hinge_incidence);

    let mut unknown_adjacency_face = fixture.topology.clone();
    unknown_adjacency_face.hinge_adjacency[0].first = fixture_face_id(999);
    reject_material(&fixture.pattern, &fixture.paper, &unknown_adjacency_face);

    let mut noncanonical_adjacency = fixture.topology.clone();
    let adjacent = &mut noncanonical_adjacency.hinge_adjacency[0];
    std::mem::swap(&mut adjacent.first, &mut adjacent.second);
    reject_material(&fixture.pattern, &fixture.paper, &noncanonical_adjacency);

    let mut duplicate_adjacency_edge = fixture.topology.clone();
    duplicate_adjacency_edge.hinge_adjacency[1] = duplicate_adjacency_edge.hinge_adjacency[0];
    reject_material(&fixture.pattern, &fixture.paper, &duplicate_adjacency_edge);

    let mut same_face_adjacency = fixture.topology.clone();
    same_face_adjacency.hinge_adjacency[0].second = same_face_adjacency.hinge_adjacency[0].first;
    reject_material(&fixture.pattern, &fixture.paper, &same_face_adjacency);

    let mut unknown_endpoint = fixture.pattern.clone();
    unknown_endpoint
        .edges
        .iter_mut()
        .find(|edge| edge.id == fixture.hinges[0])
        .expect("hinge edge")
        .start = fixture_vertex_id(999);
    reject_material(&unknown_endpoint, &fixture.paper, &fixture.topology);

    let mut broken_walk = fixture.topology.clone();
    broken_walk.faces[0].outer.half_edges[0].destination = fixture_vertex_id(999);
    reject_material(&fixture.pattern, &fixture.paper, &broken_walk);

    let mut broken_known_walk = fixture.topology.clone();
    broken_known_walk.faces[0].outer.half_edges[0].destination = fixture.vertices[3];
    reject_material(&fixture.pattern, &fixture.paper, &broken_known_walk);

    let mut wrong_source_edge_walk = fixture.topology.clone();
    let original_edge = wrong_source_edge_walk.faces[0].outer.half_edges[0].edge;
    let replacement_edge = fixture
        .pattern
        .edges
        .iter()
        .find(|edge| edge.id != original_edge)
        .expect("different source edge")
        .id;
    wrong_source_edge_walk.faces[0].outer.half_edges[0].edge = replacement_edge;
    reject_material(&fixture.pattern, &fixture.paper, &wrong_source_edge_walk);

    let positions = material_position_records(&fixture.pattern);
    ObservationTreeKinematicsModel::prepare_with_positions(
        &fixture.pattern,
        &fixture.paper,
        &fixture.topology,
        &positions,
        limits,
    )
    .expect("complete observation positions");

    let mut missing_position = positions.clone();
    missing_position.pop();
    assert_eq!(
        ObservationTreeKinematicsModel::prepare_with_positions(
            &fixture.pattern,
            &fixture.paper,
            &fixture.topology,
            &missing_position,
            limits,
        ),
        Err(KinematicsError::UnrepresentableGeometry)
    );

    let mut duplicate_position = positions.clone();
    duplicate_position[1] = duplicate_position[0];
    assert_eq!(
        ObservationTreeKinematicsModel::prepare_with_positions(
            &fixture.pattern,
            &fixture.paper,
            &fixture.topology,
            &duplicate_position,
            limits,
        ),
        Err(KinematicsError::UnsupportedTopology)
    );

    let mut extra_position = positions.clone();
    extra_position.push(VertexPosition3::new(
        fixture_vertex_id(999),
        Point3::new(1.0, 2.0, 3.0).expect("finite point"),
    ));
    assert_eq!(
        ObservationTreeKinematicsModel::prepare_with_positions(
            &fixture.pattern,
            &fixture.paper,
            &fixture.topology,
            &extra_position,
            limits,
        ),
        Err(KinematicsError::UnsupportedTopology)
    );

    let mut unknown_position = positions;
    unknown_position[0] = VertexPosition3::new(
        fixture_vertex_id(999),
        Point3::new(1.0, 2.0, 3.0).expect("finite point"),
    );
    assert_eq!(
        ObservationTreeKinematicsModel::prepare_with_positions(
            &fixture.pattern,
            &fixture.paper,
            &fixture.topology,
            &unknown_position,
            limits,
        ),
        Err(KinematicsError::UnsupportedTopology)
    );

    let mut zero_hinge_positions = material_position_records(&fixture.pattern);
    let hinge = fixture
        .pattern
        .edges
        .iter()
        .find(|edge| edge.id == fixture.hinges[0])
        .expect("hinge source");
    let start = zero_hinge_positions
        .iter()
        .find(|position| position.vertex() == hinge.start)
        .expect("hinge start position")
        .position();
    let end = zero_hinge_positions
        .iter_mut()
        .find(|position| position.vertex() == hinge.end)
        .expect("hinge end position");
    *end = VertexPosition3::new(hinge.end, start);
    assert_eq!(
        ObservationTreeKinematicsModel::prepare_with_positions(
            &fixture.pattern,
            &fixture.paper,
            &fixture.topology,
            &zero_hinge_positions,
            limits,
        ),
        Err(KinematicsError::UnrepresentableGeometry)
    );
}

#[test]
fn malformed_cycle_disconnection_assignment_and_geometry_fail_closed() {
    let fixture = non_commuting_fixture();
    let limits = TreeKinematicsLimits::default();

    let mut cycle = fixture.topology.clone();
    cycle.hinge_adjacency.push(cycle.hinge_adjacency[0]);
    assert_eq!(
        MaterialTreeKinematicsModel::prepare(&fixture.pattern, &fixture.paper, &cycle, limits),
        Err(KinematicsError::UnsupportedTopology)
    );

    let mut disconnected = fixture.topology.clone();
    let first = disconnected.hinge_adjacency[0];
    let second_edge = disconnected.hinge_adjacency[1].edge;
    disconnected.hinge_adjacency[1].first = first.first;
    disconnected.hinge_adjacency[1].second = first.second;
    let first_incidence = disconnected
        .edge_incidence
        .iter()
        .find_map(|(edge, incidence)| (*edge == first.edge).then_some(*incidence))
        .expect("first incidence");
    let second_incidence = disconnected
        .edge_incidence
        .iter_mut()
        .find(|(edge, _)| *edge == second_edge)
        .expect("second incidence");
    *second_incidence = (
        second_edge,
        match first_incidence {
            EdgeIncidence::Hinge {
                left,
                right,
                assignment: _,
            } => EdgeIncidence::Hinge {
                left,
                right,
                assignment: disconnected.hinge_adjacency[1].assignment,
            },
            _ => panic!("hinge fixture incidence"),
        },
    );
    assert_eq!(
        MaterialTreeKinematicsModel::prepare(
            &fixture.pattern,
            &fixture.paper,
            &disconnected,
            limits
        ),
        Err(KinematicsError::UnsupportedTopology)
    );

    let mut assignment = fixture.topology.clone();
    assignment.hinge_adjacency[0].assignment = match assignment.hinge_adjacency[0].assignment {
        FoldAssignment::Mountain => FoldAssignment::Valley,
        FoldAssignment::Valley => FoldAssignment::Mountain,
    };
    assert_eq!(
        MaterialTreeKinematicsModel::prepare(&fixture.pattern, &fixture.paper, &assignment, limits),
        Err(KinematicsError::UnsupportedTopology)
    );

    let mut nonfinite = fixture.pattern.clone();
    nonfinite.vertices[0].position.x = f64::NAN;
    assert_eq!(
        MaterialTreeKinematicsModel::prepare(&nonfinite, &fixture.paper, &fixture.topology, limits),
        Err(KinematicsError::UnrepresentableGeometry)
    );

    let mut overflowing_positions = material_position_records(&fixture.pattern);
    let hinge = fixture
        .pattern
        .edges
        .iter()
        .find(|edge| edge.id == fixture.hinges[0])
        .expect("hinge source");
    let start = overflowing_positions
        .iter_mut()
        .find(|position| position.vertex() == hinge.start)
        .expect("hinge start");
    *start = VertexPosition3::new(
        hinge.start,
        Point3::new(f64::MAX, 0.0, 0.0).expect("finite maximum"),
    );
    let end = overflowing_positions
        .iter_mut()
        .find(|position| position.vertex() == hinge.end)
        .expect("hinge end");
    *end = VertexPosition3::new(
        hinge.end,
        Point3::new(-f64::MAX, 0.0, 0.0).expect("finite negative maximum"),
    );
    assert_eq!(
        ObservationTreeKinematicsModel::prepare_with_positions(
            &fixture.pattern,
            &fixture.paper,
            &fixture.topology,
            &overflowing_positions,
            limits,
        ),
        Err(KinematicsError::UnrepresentableGeometry)
    );
}

#[test]
fn every_model_resource_limit_accepts_exact_boundary_and_rejects_one_more() {
    let fixture = non_commuting_fixture();
    let face_boundary_vertices = fixture
        .topology
        .faces
        .iter()
        .map(|face| face.outer.half_edges.len())
        .sum();
    let exact = TreeKinematicsLimits {
        max_source_vertices: fixture.pattern.vertices.len(),
        max_source_edges: fixture.pattern.edges.len(),
        max_paper_boundary_vertices: fixture.paper.boundary_vertices.len(),
        max_faces: fixture.topology.faces.len(),
        max_edge_incidences: fixture.topology.edge_incidence.len(),
        max_hinges: fixture.topology.hinge_adjacency.len(),
        max_face_boundary_vertices: face_boundary_vertices,
        max_adjacency_entries: fixture.topology.hinge_adjacency.len() * 2,
    };
    let exact_model = MaterialTreeKinematicsModel::prepare(
        &fixture.pattern,
        &fixture.paper,
        &fixture.topology,
        exact,
    )
    .expect("all exact limits");
    assert_eq!(
        exact_model
            .face_ids()
            .iter()
            .map(|face| {
                exact_model
                    .face_boundary(*face)
                    .expect("bounded face registry")
                    .vertices()
                    .len()
            })
            .sum::<usize>(),
        face_boundary_vertices
    );

    for too_small in [
        TreeKinematicsLimits {
            max_source_vertices: exact.max_source_vertices - 1,
            ..exact
        },
        TreeKinematicsLimits {
            max_source_edges: exact.max_source_edges - 1,
            ..exact
        },
        TreeKinematicsLimits {
            max_paper_boundary_vertices: exact.max_paper_boundary_vertices - 1,
            ..exact
        },
        TreeKinematicsLimits {
            max_faces: exact.max_faces - 1,
            ..exact
        },
        TreeKinematicsLimits {
            max_edge_incidences: exact.max_edge_incidences - 1,
            ..exact
        },
        TreeKinematicsLimits {
            max_hinges: exact.max_hinges - 1,
            ..exact
        },
        TreeKinematicsLimits {
            max_face_boundary_vertices: exact.max_face_boundary_vertices - 1,
            ..exact
        },
        TreeKinematicsLimits {
            max_adjacency_entries: exact.max_adjacency_entries - 1,
            ..exact
        },
    ] {
        assert!(matches!(
            MaterialTreeKinematicsModel::prepare(
                &fixture.pattern,
                &fixture.paper,
                &fixture.topology,
                too_small
            ),
            Err(KinematicsError::ResourceLimitExceeded)
        ));
    }
}
