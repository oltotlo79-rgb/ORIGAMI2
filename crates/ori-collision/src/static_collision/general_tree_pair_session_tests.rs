//! Whole-pose aggregation regressions for one-exact shared-hinge sessions.

use std::cell::Cell;

use ori_domain::{
    CreasePattern, Edge, EdgeId, EdgeKind, Paper, Point2, ProjectId, Vertex, VertexId,
};
use ori_kinematics::{
    CanonicalHingeAngles, HingeAngle, MaterialTreeKinematicsModel, MaterialTreePose,
    TreeKinematicsLimits,
};
use ori_topology::{FaceExtractionInput, analyze_faces};

use super::*;
use crate::cayley::{
    SharedHingeSolidDiagnosticErrorV1, prepare_shared_hinge_pair_diagnostic_session_v1,
};

fn vertex_id(index: u64) -> VertexId {
    serde_json::from_str(&format!("\"00000000-0000-4000-8100-{index:012x}\""))
        .expect("fixed vertex id")
}

fn edge_id(index: u64) -> EdgeId {
    serde_json::from_str(&format!("\"00000000-0000-4000-9100-{index:012x}\""))
        .expect("fixed edge id")
}

fn project_id(index: u64) -> ProjectId {
    serde_json::from_str(&format!("\"00000000-0000-4000-b100-{index:012x}\""))
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

fn triangle_fan_model(face_count: usize, namespace_index: u64) -> MaterialTreeKinematicsModel {
    let vertices = (0..face_count + 2)
        .map(|index| {
            let x = index as f64 * 20.0;
            vertex(index as u64 + 1, x, x * x / 400.0)
        })
        .collect::<Vec<_>>();
    let boundary = vertices.iter().map(|entry| entry.id).collect::<Vec<_>>();
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
    edges.extend((2..boundary.len() - 1).map(|index| {
        edge(
            boundary.len() as u64 + index as u64 + 1,
            boundary[0],
            boundary[index],
            EdgeKind::Mountain,
        )
    }));
    let pattern = CreasePattern { vertices, edges };
    let paper = Paper {
        boundary_vertices: boundary,
        ..Paper::default()
    };
    let report = analyze_faces(FaceExtractionInput {
        identity_namespace: project_id(namespace_index),
        source_revision: face_count as u64,
        paper: &paper,
        pattern: &pattern,
    });
    assert!(report.issues.is_empty(), "{:?}", report.issues);
    let topology = report.snapshot.expect("triangle-fan topology");
    let model = MaterialTreeKinematicsModel::prepare(
        &pattern,
        &paper,
        &topology,
        TreeKinematicsLimits::default(),
    )
    .expect("triangle-fan material tree");
    assert_eq!(model.face_ids().len(), face_count);
    assert_eq!(model.hinges().len(), face_count - 1);
    model
}

fn uniform_tree_pose(
    model: &MaterialTreeKinematicsModel,
    angle_degrees: f64,
    root: ori_domain::FaceId,
) -> MaterialTreePose {
    let angles = CanonicalHingeAngles::new(
        model
            .hinges()
            .iter()
            .map(|hinge| HingeAngle::new(hinge.edge(), angle_degrees).unwrap())
            .collect(),
    )
    .expect("canonical triangle-fan angles");
    model
        .solve(Some(root), &angles)
        .expect("triangle-fan material pose")
}

#[test]
fn controlled_static_diagnostic_cancellation_publishes_no_partial_snapshot() {
    let model = triangle_fan_model(2, 9_199);
    let pose = uniform_tree_pose(&model, 0.0, model.face_ids()[0]);
    let cancelled = std::sync::atomic::AtomicBool::new(true);
    let control = crate::CooperativeOperationControlV1::new(
        Some(&cancelled),
        std::time::Instant::now() + std::time::Duration::from_secs(1),
    );

    assert_eq!(
        diagnose_static_collision_geometry_with_control_v1(
            &model,
            &pose,
            0.0,
            StaticCollisionLimits::default(),
            &control,
        ),
        Err(StaticCollisionError::Cancelled)
    );
}

#[test]
fn four_eight_and_sixteen_face_proofs_cover_every_root_and_edge_with_one_session() {
    for (case, face_count, thickness) in [(0_u64, 4_usize, 0.1_f64), (1, 8, 1.0), (2, 16, 3.0)] {
        let model = triangle_fan_model(face_count, 9_200 + case);
        let expected_pairs = face_count * (face_count - 1) / 2;
        for root in model.face_ids().iter().copied() {
            let pose = uniform_tree_pose(&model, 0.0, root);
            let prepared_count = Cell::new(0_usize);
            let proof =
                prove_static_collision_geometry_with_shared_hinge_session_observer_for_test_v1(
                    &model,
                    &pose,
                    thickness,
                    StaticCollisionLimits::default(),
                    &|| prepared_count.set(prepared_count.get() + 1),
                )
                .expect("complete general-tree static proof");

            assert_eq!(prepared_count.get(), 1, "faces={face_count}, root={root:?}");
            assert!(proof.is_for_geometry(&model, &pose, thickness));
            assert_eq!(proof.expected_unordered_face_pairs(), expected_pairs);
            assert_eq!(proof.analyzed_unordered_face_pairs(), expected_pairs);
            assert_eq!(proof.expected_triangle_pairs(), expected_pairs);
            assert_eq!(proof.analyzed_triangle_pairs(), expected_pairs);
            assert_eq!(proof.expected_shared_hinges(), face_count - 1);
            assert_eq!(proof.analyzed_shared_hinges(), face_count - 1);
        }
    }
}

#[test]
fn static_session_boundary_rejects_one_short_and_hard_cap_plus_one() {
    let model = triangle_fan_model(4, 9_210);
    let pose = uniform_tree_pose(&model, 0.0, model.face_ids()[0]);
    let prepared_count = Cell::new(0_usize);
    let result = prove_static_collision_geometry_with_shared_hinge_session_observer_for_test_v1(
        &model,
        &pose,
        0.1,
        StaticCollisionLimits {
            max_shared_hinge_solid_diagnostics: model.hinges().len() - 1,
            ..StaticCollisionLimits::default()
        },
        &|| prepared_count.set(prepared_count.get() + 1),
    );
    assert!(matches!(
        result,
        Err(StaticCollisionError::ResourceLimitExceeded)
    ));
    assert_eq!(prepared_count.get(), 0);

    let over_hard_cap =
        triangle_fan_model(crate::cayley::MAX_COMPOSED_THICKNESS_HINGES_V1 + 2, 9_211);
    let over_pose = uniform_tree_pose(&over_hard_cap, 0.0, over_hard_cap.face_ids()[0]);
    let over_bound = over_hard_cap
        .bind_pose(&over_pose)
        .expect("bound hard-cap-plus-one pose");
    assert_eq!(
        prepare_shared_hinge_pair_diagnostic_session_v1(over_bound, 0.1).unwrap_err(),
        SharedHingeSolidDiagnosticErrorV1::ResourceLimitExceeded
    );
}

#[test]
fn aggregation_rejects_missing_or_misordered_edge_diagnostics() {
    let model = triangle_fan_model(4, 9_220);
    let first = &model.hinges()[0];
    let second = &model.hinges()[1];
    let expected_pairs = 6;

    assert_eq!(
        shared_hinge_coverage_from_diagnostic_v1(first, None, expected_pairs),
        Err(StaticCollisionError::PairEvidenceUnavailable {
            expected_unordered_face_pairs: expected_pairs,
        })
    );

    let wrong_edge = SharedHingeSolidDiagnosticSummaryV1 {
        first_face: second.left_face(),
        second_face: second.right_face(),
        evidence: IntersectionEvidenceV2::Indeterminate,
        policy_decision: TopologyContactDecision::Indeterminate,
        disposition: SharedHingeSolidDiagnosticDispositionV1::Allowed,
    };
    assert_eq!(
        shared_hinge_coverage_from_diagnostic_v1(first, Some(wrong_edge), expected_pairs),
        Err(StaticCollisionError::InconsistentMaterialPose)
    );

    let reversed_but_canonical = SharedHingeSolidDiagnosticSummaryV1 {
        first_face: first.right_face(),
        second_face: first.left_face(),
        evidence: IntersectionEvidenceV2::Indeterminate,
        policy_decision: TopologyContactDecision::Indeterminate,
        disposition: SharedHingeSolidDiagnosticDispositionV1::Allowed,
    };
    let coverage = shared_hinge_coverage_from_diagnostic_v1(
        first,
        Some(reversed_but_canonical),
        expected_pairs,
    )
    .expect("canonical pair order is semantic");
    assert_eq!(coverage.hinge, *first);
    assert_eq!(
        coverage.disposition,
        SharedHingeCoverageDispositionV1::IndependentSolidAllowed
    );
}

#[test]
fn session_revalidation_rejects_unknown_edges_and_every_issuer_drift() {
    let model = triangle_fan_model(4, 9_230);
    let pose = uniform_tree_pose(&model, 0.0, model.face_ids()[0]);
    let bound = model.bind_pose(&pose).expect("bound original pose");
    let session = prepare_shared_hinge_pair_diagnostic_session_v1(bound, 0.1)
        .expect("bounded original session")
        .expect("supported original tree");
    assert!(session.revalidates_for(bound, 0.1));
    assert_eq!(session.diagnose(None).expect("missing target"), None);
    assert_eq!(
        session
            .diagnose(Some(edge_id(999_999)))
            .expect("unknown target"),
        None
    );

    let aba_pose = uniform_tree_pose(&model, 0.0, model.face_ids()[0]);
    let aba_bound = model.bind_pose(&aba_pose).expect("bound ABA pose");
    assert!(!session.revalidates_for(aba_bound, 0.1));
    let rerooted_pose = uniform_tree_pose(&model, 0.0, model.face_ids()[3]);
    let rerooted_bound = model
        .bind_pose(&rerooted_pose)
        .expect("bound rerooted pose");
    assert!(!session.revalidates_for(rerooted_bound, 0.1));
    assert!(!session.revalidates_for(bound, f64::from_bits(0.1_f64.to_bits() + 1)));

    let foreign_model = triangle_fan_model(4, 9_230);
    assert_eq!(foreign_model.face_ids(), model.face_ids());
    assert_eq!(foreign_model.hinges(), model.hinges());
    let foreign_pose = uniform_tree_pose(&foreign_model, 0.0, foreign_model.face_ids()[0]);
    let foreign_bound = foreign_model
        .bind_pose(&foreign_pose)
        .expect("bound same-ID foreign pose");
    assert!(!session.revalidates_for(foreign_bound, 0.1));
}
