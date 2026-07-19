use std::collections::{HashMap, HashSet, VecDeque};

use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::Zero;
use ori_domain::{
    CreasePattern, Edge, EdgeId, EdgeKind, FaceId, Paper, Point2, ProjectId, Vertex, VertexId,
};
use ori_kinematics::{
    CanonicalHingeAngles, HingeAngle, MaterialTreeKinematicsModel, MaterialTreePose,
    TreeKinematicsLimits,
};
use ori_topology::{FaceExtractionInput, TopologyIssueSeverity, analyze_faces};

use super::*;

#[derive(Debug, Clone, Copy)]
struct FoldSpec {
    start: usize,
    end: usize,
    kind: EdgeKind,
}

struct PreparedFixture {
    model: MaterialTreeKinematicsModel,
    vertex_ids: Vec<VertexId>,
    hinge_ids: Vec<EdgeId>,
}

fn stress_vertex_id(index: u64) -> VertexId {
    serde_json::from_str(&format!("\"00000000-0000-4000-8c00-{index:012x}\""))
        .expect("fixed stress vertex id")
}

fn stress_edge_id(index: u64) -> EdgeId {
    serde_json::from_str(&format!("\"00000000-0000-4000-9c00-{index:012x}\""))
        .expect("fixed stress edge id")
}

fn stress_project_id() -> ProjectId {
    serde_json::from_str("\"00000000-0000-4000-bc00-000000000001\"")
        .expect("fixed stress project id")
}

fn prepare_polygon_fixture(
    source_revision: u64,
    coordinates: &[(f64, f64)],
    folds: &[FoldSpec],
) -> PreparedFixture {
    assert!(coordinates.len() >= 3);
    let vertices = coordinates
        .iter()
        .enumerate()
        .map(|(index, (x, y))| Vertex {
            id: stress_vertex_id(index as u64 + 1),
            position: Point2::new(*x, *y),
        })
        .collect::<Vec<_>>();
    let vertex_ids = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
    let mut edges = (0..vertex_ids.len())
        .map(|index| Edge {
            id: stress_edge_id(index as u64 + 1),
            start: vertex_ids[index],
            end: vertex_ids[(index + 1) % vertex_ids.len()],
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    let mut hinge_ids = Vec::with_capacity(folds.len());
    for (index, fold) in folds.iter().enumerate() {
        assert!(fold.start < vertex_ids.len());
        assert!(fold.end < vertex_ids.len());
        assert_ne!(fold.start, fold.end);
        let id = stress_edge_id(vertex_ids.len() as u64 + index as u64 + 1);
        hinge_ids.push(id);
        edges.push(Edge {
            id,
            start: vertex_ids[fold.start],
            end: vertex_ids[fold.end],
            kind: fold.kind,
        });
    }
    let pattern = CreasePattern { vertices, edges };
    let paper = Paper {
        boundary_vertices: vertex_ids.clone(),
        ..Paper::default()
    };
    let report = analyze_faces(FaceExtractionInput {
        identity_namespace: stress_project_id(),
        source_revision,
        paper: &paper,
        pattern: &pattern,
    });
    assert!(
        report.issues.is_empty(),
        "revision {source_revision}: {:?}",
        report.issues
    );
    let topology = report.snapshot.expect("stress fixture topology");
    let model = MaterialTreeKinematicsModel::prepare(
        &pattern,
        &paper,
        &topology,
        TreeKinematicsLimits::default(),
    )
    .expect("stress material tree model");
    assert_eq!(
        model
            .hinges()
            .iter()
            .map(|hinge| hinge.edge())
            .collect::<Vec<_>>(),
        hinge_ids
    );
    PreparedFixture {
        model,
        vertex_ids,
        hinge_ids,
    }
}

fn solve_fixture(
    fixture: &PreparedFixture,
    root: FaceId,
    angle_magnitudes: &[f64],
) -> MaterialTreePose {
    assert_eq!(fixture.hinge_ids.len(), angle_magnitudes.len());
    let angles = CanonicalHingeAngles::new(
        fixture
            .hinge_ids
            .iter()
            .zip(angle_magnitudes)
            .map(|(edge, angle)| HingeAngle::new(*edge, *angle).expect("finite stress angle"))
            .collect(),
    )
    .expect("canonical stress angles");
    fixture
        .model
        .solve(Some(root), &angles)
        .expect("stress tree pose")
}

fn exact_fixture_pose<'a>(
    fixture: &'a PreparedFixture,
    pose: &'a MaterialTreePose,
    limits: ExactTreePoseLimits,
) -> RationalCayleyTreePose<'a> {
    prepare_rational_cayley_tree_pose_v1(
        fixture
            .model
            .bind_pose(pose)
            .expect("issuer-bound stress pose"),
        limits,
    )
    .expect("exact stress tree pose")
}

fn exact_face<'a>(pose: &'a RationalCayleyTreePose<'_>, face: FaceId) -> &'a ExactFacePose {
    pose.faces
        .iter()
        .find(|candidate| candidate.face == face)
        .expect("stress exact face")
}

fn compose_test_transform(
    first: &ExactRigidTransform,
    second: &ExactRigidTransform,
) -> ExactRigidTransform {
    let rotation = std::array::from_fn(|row| {
        std::array::from_fn(|column| {
            (0..3)
                .map(|index| &first.rotation[row][index] * &second.rotation[index][column])
                .sum()
        })
    });
    let translation = ExactVector3 {
        coordinates: std::array::from_fn(|row| {
            first.translation.coordinates[row].clone()
                + (0..3)
                    .map(|column| {
                        &first.rotation[row][column] * &second.translation.coordinates[column]
                    })
                    .sum::<BigRational>()
        }),
    };
    ExactRigidTransform {
        rotation,
        translation,
    }
}

fn inverse_test_transform(transform: &ExactRigidTransform) -> ExactRigidTransform {
    let rotation = std::array::from_fn(|row| {
        std::array::from_fn(|column| transform.rotation[column][row].clone())
    });
    let translation = ExactVector3 {
        coordinates: std::array::from_fn(|row| {
            -(0..3)
                .map(|column| &rotation[row][column] * &transform.translation.coordinates[column])
                .sum::<BigRational>()
        }),
    };
    ExactRigidTransform {
        rotation,
        translation,
    }
}

fn assert_all_vertex_occurrences_are_watertight(
    pose: &RationalCayleyTreePose<'_>,
) -> HashMap<VertexId, (ExactPoint3, usize)> {
    let mut registry = HashMap::<VertexId, (ExactPoint3, usize)>::new();
    for face in &pose.faces {
        for (vertex, point) in &face.boundary {
            match registry.entry(*vertex) {
                std::collections::hash_map::Entry::Vacant(entry) => {
                    entry.insert((point.clone(), 1));
                }
                std::collections::hash_map::Entry::Occupied(mut entry) => {
                    assert_eq!(
                        &entry.get().0,
                        point,
                        "vertex {vertex:?} differs between exact faces"
                    );
                    entry.get_mut().1 += 1;
                }
            }
        }
    }
    registry
}

fn assert_all_hinges_are_watertight(fixture: &PreparedFixture, pose: &RationalCayleyTreePose<'_>) {
    let limits = CayleyLimits {
        max_interval_operations: 10_000_000,
        ..CayleyLimits::default()
    };
    let mut meter = WorkMeter::new(&limits);
    for exact_hinge in &pose.hinges {
        let source_hinge = fixture
            .model
            .hinges()
            .iter()
            .find(|hinge| hinge.edge() == exact_hinge.edge)
            .expect("source stress hinge");
        let start = exact_point(point3_array(source_hinge.start()), &mut meter)
            .expect("exact stress start");
        let end =
            exact_point(point3_array(source_hinge.end()), &mut meter).expect("exact stress end");
        let midpoint = ExactPoint3 {
            coordinates: std::array::from_fn(|index| {
                (&start.coordinates[index] + &end.coordinates[index])
                    / BigRational::from_integer(BigInt::from(2_u8))
            }),
        };
        let parent = &exact_face(pose, exact_hinge.parent).transform;
        let child = &exact_face(pose, exact_hinge.child).transform;
        let parent_start =
            apply_exact_transform(parent, &start, &mut meter).expect("parent stress start");
        let child_start =
            apply_exact_transform(child, &start, &mut meter).expect("child stress start");
        let parent_end =
            apply_exact_transform(parent, &end, &mut meter).expect("parent stress end");
        let child_end = apply_exact_transform(child, &end, &mut meter).expect("child stress end");
        assert_eq!(parent_start, child_start);
        assert_eq!(parent_end, child_end);
        assert_eq!(
            exact_hinge.world_endpoints,
            [parent_start.clone(), parent_end.clone()]
        );
        assert_eq!(
            apply_exact_transform(parent, &midpoint, &mut meter).expect("parent stress midpoint"),
            apply_exact_transform(child, &midpoint, &mut meter).expect("child stress midpoint")
        );
    }
}

fn assert_angle_bits(
    pose: &RationalCayleyTreePose<'_>,
    fixture: &PreparedFixture,
    angle_magnitudes: &[f64],
) {
    let expected = fixture
        .hinge_ids
        .iter()
        .zip(angle_magnitudes)
        .map(|(edge, angle)| (*edge, angle.to_bits()))
        .collect::<HashMap<_, _>>();
    for hinge in &pose.hinges {
        assert_eq!(
            Some(&hinge.angle_magnitude_bits),
            expected.get(&hinge.edge),
            "angle bits for {:?}",
            hinge.edge
        );
    }
}

fn assert_structural_work(
    pose: &RationalCayleyTreePose<'_>,
    faces: usize,
    hinges: usize,
    boundary_occurrences: usize,
    unique_vertices: usize,
) {
    assert_eq!(pose.work.faces, faces);
    assert_eq!(pose.work.hinges, hinges);
    assert_eq!(pose.work.adjacency_entries, hinges * 2);
    assert_eq!(pose.work.boundary_occurrences, boundary_occurrences);
    assert_eq!(pose.work.boundary_edge_index_entries, boundary_occurrences);
    assert_eq!(
        pose.work.boundary_edge_index_operations,
        boundary_occurrences
    );
    assert_eq!(pose.work.unique_vertices, unique_vertices);
}

fn assert_fixture_shape(fixture: &PreparedFixture, faces: usize, hinges: usize) {
    assert_eq!(fixture.model.face_ids().len(), faces);
    assert_eq!(fixture.model.hinges().len(), hinges);
    assert_eq!(fixture.hinge_ids.len(), hinges);
}

fn assert_all_roots_are_watertight_and_congruent(
    fixture: &PreparedFixture,
    angle_magnitudes: &[f64],
    expected_boundary_occurrences: usize,
    common_vertex: VertexId,
    expected_common_occurrences: usize,
) {
    let poses = fixture
        .model
        .face_ids()
        .iter()
        .map(|root| solve_fixture(fixture, *root, angle_magnitudes))
        .collect::<Vec<_>>();
    let exact_poses = poses
        .iter()
        .map(|pose| exact_fixture_pose(fixture, pose, ExactTreePoseLimits::default()))
        .collect::<Vec<_>>();
    let reference = &exact_poses[0];
    for exact in &exact_poses {
        assert_structural_work(
            exact,
            fixture.model.face_ids().len(),
            fixture.model.hinges().len(),
            expected_boundary_occurrences,
            fixture.vertex_ids.len(),
        );
        let registry = assert_all_vertex_occurrences_are_watertight(exact);
        assert_eq!(
            registry
                .get(&common_vertex)
                .map(|(_, occurrences)| *occurrences),
            Some(expected_common_occurrences)
        );
        assert_all_hinges_are_watertight(fixture, exact);
        assert_angle_bits(exact, fixture, angle_magnitudes);

        let root = exact.fixed_face.expect("multi-face stress root");
        let frame_change = inverse_test_transform(&exact_face(reference, root).transform);
        for face in fixture.model.face_ids() {
            assert_eq!(
                compose_test_transform(&frame_change, &exact_face(reference, *face).transform,),
                exact_face(exact, *face).transform,
                "root congruence for {root:?}, face {face:?}"
            );
        }
    }
}

fn assert_central_root_is_watertight(
    fixture: &PreparedFixture,
    angle_magnitudes: &[f64],
    expected_boundary_occurrences: usize,
    common_vertex: VertexId,
    expected_common_occurrences: usize,
) {
    let root = fixture.model.face_ids()[fixture.model.face_ids().len() / 2];
    let pose = solve_fixture(fixture, root, angle_magnitudes);
    let exact = exact_fixture_pose(fixture, &pose, ExactTreePoseLimits::default());
    assert_structural_work(
        &exact,
        fixture.model.face_ids().len(),
        fixture.model.hinges().len(),
        expected_boundary_occurrences,
        fixture.vertex_ids.len(),
    );
    let registry = assert_all_vertex_occurrences_are_watertight(&exact);
    assert_eq!(
        registry
            .get(&common_vertex)
            .map(|(_, occurrences)| *occurrences),
        Some(expected_common_occurrences)
    );
    assert_all_hinges_are_watertight(fixture, &exact);
    assert_angle_bits(&exact, fixture, angle_magnitudes);
}

/// The stored binary64 chords are symmetric and have the same exact rational
/// squared length. Their binary64 length rounds to 400 mm; `h` itself is the
/// pinned binary64 approximation of `200 * sqrt(3)`.
fn symmetric_near_400mm_v_fixture() -> PreparedFixture {
    let h = f64::from_bits(0x4075_a690_0584_fbe5);
    prepare_polygon_fixture(
        401,
        &[
            (0.0, 0.0),
            (200.0, 0.0),
            (400.0, 0.0),
            (400.0, h),
            (400.0, 400.0),
            (0.0, 400.0),
            (0.0, h),
        ],
        &[
            FoldSpec {
                start: 1,
                end: 6,
                kind: EdgeKind::Mountain,
            },
            FoldSpec {
                start: 1,
                end: 3,
                kind: EdgeKind::Mountain,
            },
        ],
    )
}

fn corner_mountain_valley_400mm_fixture(offset: f64) -> PreparedFixture {
    let shifted = |x: f64, y: f64| (x + offset, y - offset);
    prepare_polygon_fixture(
        402,
        &[
            shifted(0.0, 0.0),
            shifted(400.0, 0.0),
            shifted(400.0, 200.0),
            shifted(400.0, 400.0),
            shifted(200.0, 400.0),
            shifted(0.0, 400.0),
        ],
        &[
            FoldSpec {
                start: 0,
                end: 2,
                kind: EdgeKind::Mountain,
            },
            FoldSpec {
                start: 0,
                end: 4,
                kind: EdgeKind::Valley,
            },
        ],
    )
}

fn midpoint_mountain_400mm_fixture() -> PreparedFixture {
    prepare_polygon_fixture(
        403,
        &[
            (0.0, 0.0),
            (200.0, 0.0),
            (400.0, 0.0),
            (400.0, 400.0),
            (0.0, 400.0),
        ],
        &[
            FoldSpec {
                start: 1,
                end: 4,
                kind: EdgeKind::Mountain,
            },
            FoldSpec {
                start: 1,
                end: 3,
                kind: EdgeKind::Mountain,
            },
        ],
    )
}

fn shared_vertex_fan_fixture() -> PreparedFixture {
    let coordinates = (0..8)
        .map(|index| {
            let value = f64::from(index);
            (value, value * value / 8.0)
        })
        .collect::<Vec<_>>();
    let folds = (2..=6)
        .map(|end| FoldSpec {
            start: 0,
            end,
            kind: if end % 2 == 0 {
                EdgeKind::Mountain
            } else {
                EdgeKind::Valley
            },
        })
        .collect::<Vec<_>>();
    prepare_polygon_fixture(404, &coordinates, &folds)
}

fn subnormal_two_hinge_fixture() -> PreparedFixture {
    let subnormal = f64::from_bits(1);
    prepare_polygon_fixture(
        405,
        &[
            (subnormal, 0.0),
            (10.0, 0.0),
            (10.0, 5.0),
            (10.0, 10.0),
            (5.0, 10.0),
            (subnormal, 10.0),
        ],
        &[
            FoldSpec {
                start: 0,
                end: 2,
                kind: EdgeKind::Mountain,
            },
            FoldSpec {
                start: 0,
                end: 4,
                kind: EdgeKind::Valley,
            },
        ],
    )
}

fn deep_nonparallel_chain_fixture(hinge_count: usize) -> PreparedFixture {
    assert!(hinge_count >= 1);
    let vertex_count = hinge_count + 3;
    let coordinates = (0..vertex_count)
        .map(|index| {
            let value = index as f64;
            (value, value * value / 64.0)
        })
        .collect::<Vec<_>>();

    // Repeated right/right/right/left ear removal triangulates the convex
    // polygon with a dual path. Unlike a simple fan, no one vertex belongs to
    // all faces, and consecutive hinge directions are nonparallel.
    let mut low = 0_usize;
    let mut high = vertex_count - 1;
    let mut folds = Vec::with_capacity(hinge_count);
    for index in 0..hinge_count {
        let (start, end) = if index % 4 == 3 {
            let chord = (low + 1, high);
            low += 1;
            chord
        } else {
            let chord = (low, high - 1);
            high -= 1;
            chord
        };
        folds.push(FoldSpec {
            start,
            end,
            kind: if index % 2 == 0 {
                EdgeKind::Mountain
            } else {
                EdgeKind::Valley
            },
        });
    }
    assert_eq!(high - low, 2);
    prepare_polygon_fixture(500 + hinge_count as u64, &coordinates, &folds)
}

fn repeated_deep_angles(hinge_count: usize) -> Vec<f64> {
    const ANGLES: [f64; 4] = [10.0, 91.0, 135.0, 179.0];
    (0..hinge_count)
        .map(|index| ANGLES[index % ANGLES.len()])
        .collect()
}

fn assert_material_dual_is_chain(fixture: &PreparedFixture) {
    let mut adjacency = fixture
        .model
        .face_ids()
        .iter()
        .map(|face| (*face, Vec::<FaceId>::new()))
        .collect::<HashMap<_, _>>();
    for hinge in fixture.model.hinges() {
        adjacency
            .get_mut(&hinge.left_face())
            .expect("left chain face")
            .push(hinge.right_face());
        adjacency
            .get_mut(&hinge.right_face())
            .expect("right chain face")
            .push(hinge.left_face());
    }
    assert!(adjacency.values().all(|neighbors| neighbors.len() <= 2));
    assert_eq!(
        adjacency
            .values()
            .filter(|neighbors| neighbors.len() == 1)
            .count(),
        2
    );
    let start = fixture.model.face_ids()[0];
    let mut visited = HashSet::new();
    let mut queue = VecDeque::from([start]);
    while let Some(face) = queue.pop_front() {
        if !visited.insert(face) {
            continue;
        }
        queue.extend(
            adjacency
                .get(&face)
                .expect("chain adjacency")
                .iter()
                .copied(),
        );
    }
    assert_eq!(visited.len(), fixture.model.face_ids().len());
}

fn assert_consecutive_hinges_are_nonparallel(fixture: &PreparedFixture) {
    for pair in fixture.model.hinges().windows(2) {
        let first_start = pair[0].start();
        let first_end = pair[0].end();
        let second_start = pair[1].start();
        let second_end = pair[1].end();
        let first_x = first_end.x() - first_start.x();
        let first_z = first_end.z() - first_start.z();
        let second_x = second_end.x() - second_start.x();
        let second_z = second_end.z() - second_start.z();
        assert_ne!(
            first_x * second_z - first_z * second_x,
            0.0,
            "consecutive stress hinge axes must not be parallel"
        );
    }
}

fn limits_from_observed_tree_work(work: &ExactTreePoseWork) -> ExactTreePoseLimits {
    let defaults = ExactTreePoseLimits::default();
    ExactTreePoseLimits {
        max_faces: work.faces,
        max_hinges: work.hinges,
        max_adjacency_entries: work.adjacency_entries,
        max_boundary_occurrences: work.boundary_occurrences,
        max_boundary_edge_index_entries: work.boundary_edge_index_entries,
        max_boundary_edge_index_operations: work.boundary_edge_index_operations,
        max_unique_vertices: work.unique_vertices,
        max_total_machin_terms: work.exact.machin_terms,
        max_total_trig_terms: work.exact.trig_terms,
        max_total_sqrt_refinements: work.exact.sqrt_refinements,
        max_total_output_bits: work.total_output_bits,
        cayley: CayleyLimits {
            max_machin_terms_per_series: work.exact.max_machin_series_terms,
            max_trig_terms_per_series: work.exact.max_trig_series_terms,
            max_sqrt_refinements: work.exact.max_sqrt_call_refinements,
            max_interval_operations: work.exact.interval_operations,
            max_shift_bits: work.exact.max_shift_bits,
            max_intermediate_bits: work
                .exact
                .max_preflight_bits
                .max(work.exact.max_observed_bits),
            max_output_bits: work.max_output_bits,
            max_gcd_fallback_calls: work.exact.gcd_fallback_calls,
            max_gcd_fallback_input_bits: work.exact.gcd_fallback_input_bits,
            ..defaults.cayley
        },
    }
}

fn exact_material_translation(offset: f64) -> ExactVector3 {
    assert_eq!(offset.fract(), 0.0);
    let value = BigRational::from_integer(BigInt::from(offset as i64));
    ExactVector3 {
        coordinates: [value.clone(), BigRational::zero(), value],
    }
}

fn translated_exact_point(point: &ExactPoint3, offset: &ExactVector3) -> ExactPoint3 {
    ExactPoint3 {
        coordinates: std::array::from_fn(|index| {
            &point.coordinates[index] + &offset.coordinates[index]
        }),
    }
}

fn conjugate_exact_transform_by_translation(
    transform: &ExactRigidTransform,
    offset: &ExactVector3,
) -> ExactRigidTransform {
    let translation = ExactVector3 {
        coordinates: std::array::from_fn(|row| {
            &transform.translation.coordinates[row] + &offset.coordinates[row]
                - (0..3)
                    .map(|column| &transform.rotation[row][column] * &offset.coordinates[column])
                    .sum::<BigRational>()
        }),
    };
    ExactRigidTransform {
        rotation: transform.rotation.clone(),
        translation,
    }
}

#[test]
fn faithful_400mm_reported_v_fixtures_cover_angles_and_rerooting() {
    let equal_length = symmetric_near_400mm_v_fixture();
    assert_fixture_shape(&equal_length, 3, 2);
    for angles in [[10.0, 0.0], [180.0, 180.0]] {
        assert_central_root_is_watertight(
            &equal_length,
            &angles,
            11,
            equal_length.vertex_ids[1],
            3,
        );
    }
    assert_all_roots_are_watertight_and_congruent(
        &equal_length,
        &[180.0, 180.0],
        11,
        equal_length.vertex_ids[1],
        3,
    );

    let corner = corner_mountain_valley_400mm_fixture(0.0);
    assert_fixture_shape(&corner, 3, 2);
    for angles in [
        [10.0, 0.0],
        [0.0, 10.0],
        [45.0, 45.0],
        [91.0, 91.0],
        [135.0, 135.0],
    ] {
        assert_central_root_is_watertight(&corner, &angles, 10, corner.vertex_ids[0], 3);
    }
    assert_all_roots_are_watertight_and_congruent(
        &corner,
        &[91.0, 91.0],
        10,
        corner.vertex_ids[0],
        3,
    );

    let midpoint = midpoint_mountain_400mm_fixture();
    assert_fixture_shape(&midpoint, 3, 2);
    for angles in [[90.0, 90.0], [91.0, 91.0], [135.0, 135.0], [179.0, 179.0]] {
        assert_central_root_is_watertight(&midpoint, &angles, 9, midpoint.vertex_ids[1], 3);
    }
    assert_all_roots_are_watertight_and_congruent(
        &midpoint,
        &[91.0, 91.0],
        9,
        midpoint.vertex_ids[1],
        3,
    );
}

#[test]
fn huge_dyadic_translation_preserves_the_exact_pose_by_conjugacy() {
    let base = corner_mountain_valley_400mm_fixture(0.0);
    assert_fixture_shape(&base, 3, 2);
    let angles = [91.0, 135.0];
    let root = base.model.face_ids()[base.model.face_ids().len() / 2];
    let base_pose = solve_fixture(&base, root, &angles);
    let base_exact = exact_fixture_pose(&base, &base_pose, ExactTreePoseLimits::default());

    for offset in [
        -1_000_000_000_000_000.0,
        -3_000_000_000_000.0,
        -1_000_000_000_000.0,
        1_000_000_000_000.0,
        3_000_000_000_000.0,
        1_000_000_000_000_000.0,
    ] {
        assert_ne!(offset + 200.0, offset);
        assert_ne!(offset + 400.0, offset);
        let translated = corner_mountain_valley_400mm_fixture(offset);
        assert_fixture_shape(&translated, 3, 2);
        assert_eq!(translated.model.face_ids(), base.model.face_ids());
        let pose = solve_fixture(&translated, root, &angles);
        let exact = exact_fixture_pose(&translated, &pose, ExactTreePoseLimits::default());
        assert_structural_work(&exact, 3, 2, 10, 6);
        assert_all_vertex_occurrences_are_watertight(&exact);
        assert_all_hinges_are_watertight(&translated, &exact);
        assert_angle_bits(&exact, &translated, &angles);

        let exact_offset = exact_material_translation(offset);
        for face in base.model.face_ids() {
            let base_face = exact_face(&base_exact, *face);
            let translated_face = exact_face(&exact, *face);
            assert_eq!(
                translated_face.transform,
                conjugate_exact_transform_by_translation(&base_face.transform, &exact_offset,),
                "transform conjugacy at offset {offset}, face {face:?}"
            );
            assert_eq!(translated_face.boundary.len(), base_face.boundary.len());
            for ((base_vertex, base_point), (translated_vertex, translated_point)) in
                base_face.boundary.iter().zip(&translated_face.boundary)
            {
                assert_eq!(translated_vertex, base_vertex);
                assert_eq!(
                    translated_point,
                    &translated_exact_point(base_point, &exact_offset),
                    "world point translation at offset {offset}, vertex {base_vertex:?}"
                );
            }
        }
        for (base_hinge, translated_hinge) in base_exact.hinges.iter().zip(&exact.hinges) {
            assert_eq!(translated_hinge.edge, base_hinge.edge);
            assert_eq!(translated_hinge.parent, base_hinge.parent);
            assert_eq!(translated_hinge.child, base_hinge.child);
            assert_eq!(translated_hinge.rotation_sign, base_hinge.rotation_sign);
            assert_eq!(
                translated_hinge.angle_magnitude_bits,
                base_hinge.angle_magnitude_bits
            );
            assert_eq!(translated_hinge.certificate, base_hinge.certificate);
            assert_eq!(
                translated_hinge.endpoint_vertices,
                base_hinge.endpoint_vertices
            );
            for (base_point, translated_point) in base_hinge
                .world_endpoints
                .iter()
                .zip(&translated_hinge.world_endpoints)
            {
                assert_eq!(
                    translated_point,
                    &translated_exact_point(base_point, &exact_offset)
                );
            }
        }
    }
}

#[test]
fn tree_minimum_subnormal_angle_remains_nonzero_and_watertight() {
    let fixture = subnormal_two_hinge_fixture();
    assert_fixture_shape(&fixture, 3, 2);
    let smallest = f64::from_bits(1);
    let largest_subnormal = f64::from_bits((1_u64 << 52) - 1);
    let angles = [smallest, largest_subnormal];
    let root = fixture.model.face_ids()[fixture.model.face_ids().len() / 2];
    let pose = solve_fixture(&fixture, root, &angles);
    let baseline = exact_fixture_pose(&fixture, &pose, ExactTreePoseLimits::default());
    assert_structural_work(&baseline, 3, 2, 10, 6);
    let registry = assert_all_vertex_occurrences_are_watertight(&baseline);
    assert_eq!(
        registry.get(&fixture.vertex_ids[0]).map(|entry| entry.1),
        Some(3)
    );
    assert_all_hinges_are_watertight(&fixture, &baseline);
    assert_angle_bits(&baseline, &fixture, &angles);
    let subnormal_hinge = baseline
        .hinges
        .iter()
        .find(|hinge| hinge.angle_magnitude_bits == smallest.to_bits())
        .expect("subnormal exact hinge");
    let ExactAngleCertificate::Bounded(certificate) = &subnormal_hinge.certificate else {
        panic!("subnormal angle must carry a bounded certificate");
    };
    assert!(!certificate.parameter.is_zero());
    assert!(certificate.max_error_degrees < certificate.acceptance_degrees);

    let limits = ExactTreePoseLimits::default();
    assert!(baseline.work.exact.max_shift_bits <= limits.cayley.max_shift_bits);
    assert!(baseline.work.exact.max_preflight_bits <= limits.cayley.max_intermediate_bits);
    assert!(baseline.work.max_output_bits <= limits.cayley.max_output_bits);
    assert!(baseline.work.total_output_bits <= limits.max_total_output_bits);
    assert!(baseline.work.exact.interval_operations <= limits.cayley.max_interval_operations);
    assert!(baseline.work.exact.machin_terms <= limits.max_total_machin_terms);
    assert!(baseline.work.exact.trig_terms <= limits.max_total_trig_terms);
    assert!(baseline.work.exact.sqrt_refinements <= limits.max_total_sqrt_refinements);
    assert!(
        baseline.work.exact.gcd_fallback_calls > 0,
        "the minimum-subnormal stress path must exercise reduced rational preflight"
    );
    assert!(
        baseline.work.exact.gcd_fallback_input_bits > 0,
        "the minimum-subnormal stress path must account every GCD input bit"
    );

    let exact = limits_from_observed_tree_work(&baseline.work);
    assert!(
        prepare_rational_cayley_tree_pose_v1(fixture.model.bind_pose(&pose).unwrap(), exact,)
            .is_ok()
    );

    let mut one_short = exact;
    one_short.cayley.max_gcd_fallback_calls -= 1;
    assert!(matches!(
        prepare_rational_cayley_tree_pose_v1(fixture.model.bind_pose(&pose).unwrap(), one_short,),
        Err(CayleyError::ResourceLimitExceeded {
            resource: "gcd_fallback_calls",
            ..
        })
    ));

    let mut one_short = exact;
    one_short.cayley.max_gcd_fallback_input_bits -= 1;
    assert!(matches!(
        prepare_rational_cayley_tree_pose_v1(fixture.model.bind_pose(&pose).unwrap(), one_short,),
        Err(CayleyError::ResourceLimitExceeded {
            resource: "gcd_fallback_input_bits",
            ..
        })
    ));
}

#[test]
fn sixteen_hinge_nonparallel_chain_is_watertight_with_bounded_work() {
    const HINGES: usize = 16;
    let fixture = deep_nonparallel_chain_fixture(HINGES);
    assert_fixture_shape(&fixture, HINGES + 1, HINGES);
    assert_material_dual_is_chain(&fixture);
    assert_consecutive_hinges_are_nonparallel(&fixture);
    let angles = repeated_deep_angles(HINGES);
    let root = fixture.model.face_ids()[fixture.model.face_ids().len() / 2];
    let pose = solve_fixture(&fixture, root, &angles);
    let exact = exact_fixture_pose(&fixture, &pose, ExactTreePoseLimits::default());
    assert_structural_work(&exact, HINGES + 1, HINGES, 3 * (HINGES + 1), HINGES + 3);
    assert_all_vertex_occurrences_are_watertight(&exact);
    assert_all_hinges_are_watertight(&fixture, &exact);
    assert_angle_bits(&exact, &fixture, &angles);

    let limits = ExactTreePoseLimits::default();
    assert!(exact.work.exact.interval_operations <= limits.cayley.max_interval_operations);
    assert!(exact.work.exact.machin_terms <= limits.max_total_machin_terms);
    assert!(exact.work.exact.trig_terms <= limits.max_total_trig_terms);
    assert!(exact.work.exact.sqrt_refinements <= limits.max_total_sqrt_refinements);
    assert!(exact.work.exact.max_shift_bits <= limits.cayley.max_shift_bits);
    assert!(exact.work.exact.max_preflight_bits <= limits.cayley.max_intermediate_bits);
    assert!(exact.work.max_output_bits <= limits.cayley.max_output_bits);
    assert!(exact.work.total_output_bits <= limits.max_total_output_bits);
}

#[test]
fn sixty_four_hinge_chain_fails_closed_before_exact_work_when_resource_denied() {
    const HINGES: usize = 64;
    let fixture = deep_nonparallel_chain_fixture(HINGES);
    assert_fixture_shape(&fixture, HINGES + 1, HINGES);
    assert_material_dual_is_chain(&fixture);
    assert_consecutive_hinges_are_nonparallel(&fixture);
    let angles = repeated_deep_angles(HINGES);
    let root = fixture.model.face_ids()[fixture.model.face_ids().len() / 2];
    let pose = solve_fixture(&fixture, root, &angles);
    let limits = ExactTreePoseLimits {
        max_faces: HINGES,
        ..ExactTreePoseLimits::default()
    };
    assert!(matches!(
        prepare_rational_cayley_tree_pose_v1(fixture.model.bind_pose(&pose).unwrap(), limits,),
        Err(CayleyError::ResourceLimitExceeded {
            stage: CayleyStage::Tree,
            resource: "faces",
        })
    ));
}

#[test]
fn observed_deep_chain_aggregate_limits_accept_exact_and_reject_one_short() {
    const HINGES: usize = 8;
    let fixture = deep_nonparallel_chain_fixture(HINGES);
    assert_fixture_shape(&fixture, HINGES + 1, HINGES);
    assert_material_dual_is_chain(&fixture);
    let angles = repeated_deep_angles(HINGES);
    let root = fixture.model.face_ids()[fixture.model.face_ids().len() / 2];
    let pose = solve_fixture(&fixture, root, &angles);
    let baseline = exact_fixture_pose(&fixture, &pose, ExactTreePoseLimits::default());
    assert_structural_work(&baseline, 9, 8, 27, 11);
    let exact = limits_from_observed_tree_work(&baseline.work);
    assert!(
        prepare_rational_cayley_tree_pose_v1(fixture.model.bind_pose(&pose).unwrap(), exact,)
            .is_ok()
    );

    if exact.cayley.max_gcd_fallback_calls > 0 {
        let mut one_short = exact;
        one_short.cayley.max_gcd_fallback_calls -= 1;
        assert!(matches!(
            prepare_rational_cayley_tree_pose_v1(
                fixture.model.bind_pose(&pose).unwrap(),
                one_short,
            ),
            Err(CayleyError::ResourceLimitExceeded {
                resource: "gcd_fallback_calls",
                ..
            })
        ));
    }

    if exact.cayley.max_gcd_fallback_input_bits > 0 {
        let mut one_short = exact;
        one_short.cayley.max_gcd_fallback_input_bits -= 1;
        assert!(matches!(
            prepare_rational_cayley_tree_pose_v1(
                fixture.model.bind_pose(&pose).unwrap(),
                one_short,
            ),
            Err(CayleyError::ResourceLimitExceeded {
                resource: "gcd_fallback_input_bits",
                ..
            })
        ));
    }

    let mut one_short = exact;
    one_short.max_total_machin_terms -= 1;
    assert!(matches!(
        prepare_rational_cayley_tree_pose_v1(fixture.model.bind_pose(&pose).unwrap(), one_short,),
        Err(CayleyError::ResourceLimitExceeded {
            resource: "total_machin_terms",
            ..
        })
    ));

    let mut one_short = exact;
    one_short.cayley.max_interval_operations -= 1;
    assert!(matches!(
        prepare_rational_cayley_tree_pose_v1(fixture.model.bind_pose(&pose).unwrap(), one_short,),
        Err(CayleyError::ResourceLimitExceeded {
            resource: "interval_operations",
            ..
        })
    ));

    let mut one_short = exact;
    one_short.max_total_output_bits -= 1;
    assert!(matches!(
        prepare_rational_cayley_tree_pose_v1(fixture.model.bind_pose(&pose).unwrap(), one_short,),
        Err(CayleyError::ResourceLimitExceeded {
            resource: "total_output_bits",
            ..
        })
    ));
}

#[test]
fn source_precision_collapse_is_rejected_before_exact_authority_exists() {
    let offset = 2_f64.powi(62);
    assert_eq!(offset + 400.0, offset);
    let coordinates = [
        (offset, offset),
        (offset + 400.0, offset),
        (offset + 400.0, offset + 400.0),
        (offset, offset + 400.0),
    ];
    assert!(coordinates.windows(2).any(|pair| pair[0] == pair[1]));
    let vertices = coordinates
        .iter()
        .enumerate()
        .map(|(index, (x, y))| Vertex {
            id: stress_vertex_id(10_000 + index as u64),
            position: Point2::new(*x, *y),
        })
        .collect::<Vec<_>>();
    let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
    let edges = (0..boundary.len())
        .map(|index| Edge {
            id: stress_edge_id(10_000 + index as u64),
            start: boundary[index],
            end: boundary[(index + 1) % boundary.len()],
            kind: EdgeKind::Boundary,
        })
        .collect();
    let pattern = CreasePattern { vertices, edges };
    let paper = Paper {
        boundary_vertices: boundary,
        ..Paper::default()
    };
    let report = analyze_faces(FaceExtractionInput {
        identity_namespace: stress_project_id(),
        source_revision: 999,
        paper: &paper,
        pattern: &pattern,
    });
    assert!(report.snapshot.is_none());
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.severity == TopologyIssueSeverity::Fatal)
    );
}

#[test]
fn shared_vertex_fan_checks_every_nonadjacent_occurrence() {
    let fixture = shared_vertex_fan_fixture();
    assert_fixture_shape(&fixture, 6, 5);
    let angles = [10.0, 45.0, 91.0, 135.0, 179.0];
    let root = fixture.model.face_ids()[fixture.model.face_ids().len() / 2];
    let pose = solve_fixture(&fixture, root, &angles);
    let exact = exact_fixture_pose(&fixture, &pose, ExactTreePoseLimits::default());
    assert_structural_work(&exact, 6, 5, 18, 8);
    let registry = assert_all_vertex_occurrences_are_watertight(&exact);
    assert_eq!(
        registry.get(&fixture.vertex_ids[0]).map(|entry| entry.1),
        Some(6)
    );
    assert_all_hinges_are_watertight(&fixture, &exact);
    assert_angle_bits(&exact, &fixture, &angles);

    let common_faces = exact
        .faces
        .iter()
        .filter(|face| {
            face.boundary
                .iter()
                .any(|(vertex, _)| *vertex == fixture.vertex_ids[0])
        })
        .map(|face| face.face)
        .collect::<HashSet<_>>();
    assert_eq!(common_faces.len(), 6);
    let adjacent_pairs = fixture
        .model
        .hinges()
        .iter()
        .map(|hinge| {
            let mut pair = [hinge.left_face(), hinge.right_face()];
            pair.sort_unstable_by_key(FaceId::canonical_bytes);
            pair
        })
        .collect::<HashSet<_>>();
    assert_eq!(adjacent_pairs.len(), 5);
    let mut vertex_only_pairs = 0;
    let faces = common_faces.into_iter().collect::<Vec<_>>();
    for first in 0..faces.len() {
        for second in (first + 1)..faces.len() {
            let mut pair = [faces[first], faces[second]];
            pair.sort_unstable_by_key(FaceId::canonical_bytes);
            if !adjacent_pairs.contains(&pair) {
                vertex_only_pairs += 1;
            }
        }
    }
    assert_eq!(vertex_only_pairs, 10);
}
