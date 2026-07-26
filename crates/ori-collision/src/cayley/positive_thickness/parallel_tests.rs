use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread,
    time::Duration,
};

use ori_domain::{
    CreasePattern, Edge, EdgeId, EdgeKind, Paper, Point2, ProjectId, Vertex, VertexId,
};
use ori_kinematics::{
    CanonicalHingeAngles, HingeAngle, MaterialTreeKinematicsModel, MaterialTreePose,
    TreeKinematicsLimits,
};
use ori_topology::{FaceExtractionInput, analyze_faces};

use super::*;

const FAN_FACE_COUNT: usize = 3;
const FAN_PAIR_COUNT: usize = FAN_FACE_COUNT * (FAN_FACE_COUNT - 1) / 2;

fn vertex_id(index: u64) -> VertexId {
    serde_json::from_str(&format!("\"00000000-0000-4000-8200-{index:012x}\""))
        .expect("fixed vertex id")
}

fn edge_id(index: u64) -> EdgeId {
    serde_json::from_str(&format!("\"00000000-0000-4000-9200-{index:012x}\""))
        .expect("fixed edge id")
}

fn project_id() -> ProjectId {
    serde_json::from_str("\"00000000-0000-4000-b200-000000000001\"").expect("fixed project id")
}

fn fan_model(reordered_sources: bool) -> MaterialTreeKinematicsModel {
    let coordinates = [
        (0.0, 0.0),
        (200.0, 0.0),
        (300.0, 150.0),
        (200.0, 300.0),
        (0.0, 300.0),
    ];
    let mut vertices = coordinates
        .into_iter()
        .enumerate()
        .map(|(index, (x, y))| Vertex {
            id: vertex_id(index as u64 + 1),
            position: Point2::new(x, y),
        })
        .collect::<Vec<_>>();
    let mut boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
    let mut edges = (0..boundary.len())
        .map(|index| Edge {
            id: edge_id(index as u64 + 1),
            start: boundary[index],
            end: boundary[(index + 1) % boundary.len()],
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    for endpoint in 2..boundary.len() - 1 {
        edges.push(Edge {
            id: edge_id(100 + endpoint as u64),
            start: boundary[0],
            end: boundary[endpoint],
            kind: if endpoint % 2 == 0 {
                EdgeKind::Mountain
            } else {
                EdgeKind::Valley
            },
        });
    }
    if reordered_sources {
        vertices.reverse();
        edges.reverse();
        boundary.rotate_left(3);
    }
    let pattern = CreasePattern { vertices, edges };
    let paper = Paper {
        boundary_vertices: boundary,
        ..Paper::default()
    };
    let report = analyze_faces(FaceExtractionInput {
        identity_namespace: project_id(),
        source_revision: 20_260_726,
        paper: &paper,
        pattern: &pattern,
    });
    assert!(report.issues.is_empty(), "{:?}", report.issues);
    let model = MaterialTreeKinematicsModel::prepare(
        &pattern,
        &paper,
        &report.snapshot.expect("fan faces"),
        TreeKinematicsLimits::default(),
    )
    .expect("fan model");
    assert_eq!(model.face_ids().len(), FAN_FACE_COUNT);
    model
}

fn fan_pose(model: &MaterialTreeKinematicsModel) -> MaterialTreePose {
    let angles = CanonicalHingeAngles::new(
        model
            .hinges()
            .iter()
            .map(|hinge| HingeAngle::new(hinge.edge(), 12.0).expect("finite angle"))
            .collect(),
    )
    .expect("canonical fan angles");
    model
        .solve(Some(model.face_ids()[0]), &angles)
        .expect("fan pose")
}

fn parallel_scan(
    model: &MaterialTreeKinematicsModel,
    pose: &MaterialTreePose,
    worker_threads: usize,
) -> PositiveThicknessPrismScanV1 {
    diagnose_bound_positive_thickness_prism_pairs_parallel_v1(
        model.bind_pose(pose).expect("bound fan pose"),
        0.1,
        FAN_PAIR_COUNT,
        PositiveThicknessPrismParallelConfigV1 {
            worker_threads,
            cancellation: None,
        },
    )
    .expect("complete fan scan")
}

#[test]
fn parallel_face_pair_proof_matches_sequential_result_bit_exact() {
    let model = fan_model(false);
    let pose = fan_pose(&model);
    let sequential = diagnose_bound_positive_thickness_prism_pairs_v1(
        model.bind_pose(&pose).unwrap(),
        0.1,
        FAN_PAIR_COUNT,
    )
    .expect("sequential scan");
    let parallel = parallel_scan(&model, &pose, 4);

    assert_eq!(parallel.diagnostics, sequential);
    assert_eq!(parallel.work.expected_pairs, FAN_PAIR_COUNT);
    assert_eq!(parallel.work.completed_pairs, FAN_PAIR_COUNT);
}

#[test]
fn parallel_face_pair_proof_work_total_independent_of_thread_count() {
    let model = fan_model(false);
    let pose = fan_pose(&model);
    let scans = [1, 2, 4, 8].map(|workers| parallel_scan(&model, &pose, workers));

    for scan in &scans[1..] {
        assert_eq!(scan.diagnostics, scans[0].diagnostics);
        assert_eq!(scan.work, scans[0].work);
    }
}

#[test]
fn parallel_face_pair_proof_merge_order_is_canonical_not_completion_order() {
    let model = fan_model(false);
    let pose = fan_pose(&model);
    let completion_order = Arc::new(Mutex::new(Vec::new()));
    let observed = Arc::clone(&completion_order);
    let later_pair_reached_observer = Arc::new(AtomicBool::new(false));
    let later_observed = Arc::clone(&later_pair_reached_observer);
    let scan = diagnose_bound_positive_thickness_prism_pairs_with_observer_v1(
        model.bind_pose(&pose).unwrap(),
        0.1,
        FAN_PAIR_COUNT,
        PositiveThicknessPrismParallelConfigV1 {
            worker_threads: 4,
            cancellation: None,
        },
        &move |index| {
            if index == 0 {
                while !later_observed.load(Ordering::Acquire) {
                    thread::yield_now();
                }
                thread::sleep(Duration::from_millis(10));
            } else {
                later_observed.store(true, Ordering::Release);
            }
            observed.lock().unwrap().push(index);
        },
        &|_| ExactPrismLimits::default(),
    )
    .expect("parallel scan");

    let completion_order = completion_order.lock().unwrap();
    assert_ne!(*completion_order, (0..FAN_PAIR_COUNT).collect::<Vec<_>>());
    let expected_pairs = pose
        .face_ids()
        .iter()
        .enumerate()
        .flat_map(|(first, first_face)| {
            pose.face_ids()[first + 1..]
                .iter()
                .map(move |second_face| (*first_face, *second_face))
        })
        .collect::<Vec<_>>();
    let actual_pairs = scan
        .diagnostics
        .iter()
        .map(|pair| (pair.first_face, pair.second_face))
        .collect::<Vec<_>>();
    assert_eq!(actual_pairs, expected_pairs);
}

#[test]
fn parallel_face_pair_proof_reservation_failure_rejects_before_spawn() {
    let model = fan_model(false);
    let pose = fan_pose(&model);
    let executed = AtomicUsize::new(0);
    let result = diagnose_bound_positive_thickness_prism_pairs_with_observer_v1(
        model.bind_pose(&pose).unwrap(),
        0.1,
        FAN_PAIR_COUNT - 1,
        PositiveThicknessPrismParallelConfigV1 {
            worker_threads: 4,
            cancellation: None,
        },
        &|_| {
            executed.fetch_add(1, Ordering::Relaxed);
        },
        &|_| ExactPrismLimits::default(),
    );

    assert_eq!(
        result,
        Err(PositiveThicknessPrismScanErrorV1::ResourceLimitExceeded)
    );
    assert_eq!(executed.load(Ordering::Relaxed), 0);
}

#[test]
fn parallel_face_pair_proof_single_worker_resource_exhaustion_closes_whole_result() {
    let model = fan_model(false);
    let pose = fan_pose(&model);
    let executed = AtomicUsize::new(0);
    let result = diagnose_bound_positive_thickness_prism_pairs_with_observer_v1(
        model.bind_pose(&pose).unwrap(),
        0.1,
        FAN_PAIR_COUNT,
        PositiveThicknessPrismParallelConfigV1 {
            worker_threads: 4,
            cancellation: None,
        },
        &|_| {
            executed.fetch_add(1, Ordering::Relaxed);
        },
        &|index| {
            let mut limits = ExactPrismLimits::default();
            if index == 1 {
                // One pair receives a one-short local hard envelope. No
                // diagnostics from the other completed pairs may escape.
                limits.max_prisms = 0;
            }
            limits
        },
    );

    assert_eq!(
        result,
        Err(PositiveThicknessPrismScanErrorV1::ResourceLimitExceeded)
    );
    assert_eq!(executed.load(Ordering::Relaxed), FAN_PAIR_COUNT);
}

#[test]
fn parallel_face_pair_proof_cancel_during_partial_completion_yields_unknown() {
    let model = fan_model(false);
    let pose = fan_pose(&model);
    let cancellation = AtomicBool::new(false);
    let executed = AtomicUsize::new(0);
    let result = diagnose_bound_positive_thickness_prism_pairs_with_observer_v1(
        model.bind_pose(&pose).unwrap(),
        0.1,
        FAN_PAIR_COUNT,
        PositiveThicknessPrismParallelConfigV1 {
            worker_threads: 1,
            cancellation: Some(&cancellation),
        },
        &|_| {
            executed.fetch_add(1, Ordering::Relaxed);
            cancellation.store(true, Ordering::Release);
        },
        &|_| ExactPrismLimits::default(),
    );

    assert_eq!(result, Err(PositiveThicknessPrismScanErrorV1::Cancelled));
    assert_eq!(executed.load(Ordering::Relaxed), 1);
}

#[test]
fn parallel_face_pair_proof_reversed_input_order_identical_result() {
    let canonical_model = fan_model(false);
    let canonical_pose = fan_pose(&canonical_model);
    let reordered_model = fan_model(true);
    let reordered_pose = fan_pose(&reordered_model);

    assert_eq!(
        parallel_scan(&canonical_model, &canonical_pose, 4),
        parallel_scan(&reordered_model, &reordered_pose, 4)
    );
}

#[test]
fn parallel_face_pair_public_worker4_result_matches_sequential() {
    let model = fan_model(false);
    let pose = fan_pose(&model);
    let normalize =
        |result: Result<crate::NativeStaticCollisionGeometryProof, crate::StaticCollisionError>| {
            result.map(|proof| {
                (
                    proof.proof_id(),
                    proof.policy_id(),
                    proof.kinematics_model_id(),
                    proof.thickness_model_id(),
                    proof.paper_thickness_bits(),
                    proof.face_count(),
                    proof.expected_unordered_face_pairs(),
                    proof.analyzed_unordered_face_pairs(),
                    proof.expected_triangle_pairs(),
                    proof.analyzed_triangle_pairs(),
                    proof.expected_shared_hinges(),
                    proof.analyzed_shared_hinges(),
                )
            })
        };
    let sequential = crate::prove_static_collision_geometry(
        &model,
        &pose,
        0.1,
        crate::StaticCollisionLimits::default(),
    );
    let parallel = crate::prove_static_collision_geometry_parallel_v1(
        &model,
        &pose,
        0.1,
        crate::StaticCollisionLimits::default(),
        crate::StaticCollisionParallelConfigV1::new(4),
    );

    assert_eq!(normalize(parallel), normalize(sequential));
}

#[test]
fn parallel_face_pair_public_invalid_workers_and_cancellation_fail_closed() {
    let model = fan_model(false);
    let pose = fan_pose(&model);
    for workers in [0, crate::MAX_STATIC_COLLISION_PARALLEL_WORKERS_V1 + 1] {
        assert!(matches!(
            crate::prove_static_collision_geometry_parallel_v1(
                &model,
                &pose,
                0.1,
                crate::StaticCollisionLimits::default(),
                crate::StaticCollisionParallelConfigV1::new(workers),
            ),
            Err(crate::StaticCollisionError::ResourceLimitExceeded)
        ));
    }

    let cancellation = Arc::new(AtomicBool::new(true));
    assert!(matches!(
        crate::prove_static_collision_geometry_parallel_v1(
            &model,
            &pose,
            0.1,
            crate::StaticCollisionLimits::default(),
            crate::StaticCollisionParallelConfigV1::new(4).with_cancellation(cancellation),
        ),
        Err(crate::StaticCollisionError::PairEvidenceUnavailable {
            expected_unordered_face_pairs: FAN_PAIR_COUNT,
        })
    ));
}

#[test]
fn parallel_face_pair_executor_reserves_all_output_before_spawn() {
    let executed = AtomicUsize::new(0);
    let result =
        crate::cayley::parallel_meter::execute_canonical_pairs(usize::MAX, 4, None, |_| {
            executed.fetch_add(1, Ordering::Relaxed);
        });

    assert_eq!(
        result,
        Err(crate::cayley::parallel_meter::CanonicalPairExecutionError::ResourceLimitExceeded)
    );
    assert_eq!(executed.load(Ordering::Relaxed), 0);
}

#[test]
fn parallel_face_pair_late_cancel_after_scan_never_publishes_authority() {
    let model = fan_model(false);
    let pose = fan_pose(&model);
    let cancellation = Arc::new(AtomicBool::new(false));
    let config =
        crate::StaticCollisionParallelConfigV1::new(4).with_cancellation(Arc::clone(&cancellation));
    let observed = AtomicUsize::new(0);
    let result =
        crate::static_collision::prove_static_collision_geometry_with_post_prism_observer_for_test_v1(
            &model,
            &pose,
            0.1,
            crate::StaticCollisionLimits::default(),
            &config,
            &|| {
                observed.fetch_add(1, Ordering::Relaxed);
                cancellation.store(true, Ordering::Release);
            },
        );

    assert!(matches!(
        result,
        Err(crate::StaticCollisionError::PairEvidenceUnavailable {
            expected_unordered_face_pairs: FAN_PAIR_COUNT,
        })
    ));
    assert_eq!(observed.load(Ordering::Relaxed), 1);
}
