use super::*;
use ori_collision::{
    NonFlatLayerOrderStructuralSourceV1, StackedFoldPathDiagnosticLimitsV1, StaticCollisionLimits,
    diagnose_static_collision_geometry,
};
use ori_core::{
    DEFAULT_MAX_STACKED_FOLD_NON_FLAT_FACE_PAIRS, ExpectedStackedFoldCreaseV1, FaceLineageLimits,
    StackedFoldGeometryLimitsV1, StackedFoldTopologyBuildLimitsV1,
    diagnose_stacked_fold_requested_path_with_initial_layer_order_v1,
    prepare_stacked_fold_geometry_candidate_v1, prepare_stacked_fold_initial_layer_order_v1,
    prepare_stacked_fold_initial_pose_v1,
    prepare_stacked_fold_non_flat_layer_order_with_thickness_v1,
    prepare_stacked_fold_requested_pose_v1, prepare_stacked_fold_target_graph_audit_v1,
    prepare_stacked_fold_target_model_v1,
};
use ori_domain::{CreasePattern, Edge, EdgeKind, FaceId, Paper, Point2, ProjectId, Vertex};
use ori_foldability::fold_model_fingerprint_v1;
use ori_kinematics::TreeKinematicsLimits;

const FOUR_FACE_POINTS_V1: [(f64, f64); 10] = [
    (0.0, 0.0),
    (500.0, 0.0),
    (1_000.0, 0.0),
    (1_900.0, 0.0),
    (2_400.0, 0.0),
    (2_400.0, 300.0),
    (1_500.0, 300.0),
    (1_400.0, 300.0),
    (100.0, 300.0),
    (0.0, 300.0),
];
const FOUR_FACE_CREASES_V1: [(usize, usize, EdgeKind); 3] = [
    (1, 8, EdgeKind::Mountain),
    (2, 7, EdgeKind::Valley),
    (3, 6, EdgeKind::Mountain),
];
const FOUR_FACE_MOVING_CREASE_V1: usize = 0;
const FOUR_FACE_FIXED_NON_SPLIT_GEOMETRIC_RANK_V1: usize = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FourFaceFixtureSelectionV1 {
    moving_crease: usize,
    fixed_face_geometric_rank: usize,
}

#[derive(Debug)]
struct SourceFaceGeometricRankEntryV1 {
    face: FaceId,
    centroid: (f64, f64),
    canonical_boundary: Vec<(f64, f64)>,
}

fn canonical_geometric_coordinate_v1(value: f64) -> Option<f64> {
    value
        .is_finite()
        .then_some(if value == 0.0 { 0.0 } else { value })
}

fn compare_geometric_point_v1(left: &(f64, f64), right: &(f64, f64)) -> std::cmp::Ordering {
    left.0
        .total_cmp(&right.0)
        .then_with(|| left.1.total_cmp(&right.1))
}

fn compare_geometric_boundary_v1(left: &[(f64, f64)], right: &[(f64, f64)]) -> std::cmp::Ordering {
    left.iter()
        .zip(right)
        .map(|(left, right)| compare_geometric_point_v1(left, right))
        .find(|order| *order != std::cmp::Ordering::Equal)
        .unwrap_or_else(|| left.len().cmp(&right.len()))
}

fn canonical_geometric_boundary_v1(boundary: Vec<(f64, f64)>) -> Option<Vec<(f64, f64)>> {
    if boundary.len() < 3 {
        return None;
    }
    let boundary = boundary
        .into_iter()
        .map(|(x, y)| {
            Some((
                canonical_geometric_coordinate_v1(x)?,
                canonical_geometric_coordinate_v1(y)?,
            ))
        })
        .collect::<Option<Vec<_>>>()?;
    if boundary.iter().enumerate().any(|(index, point)| {
        boundary[index + 1..]
            .iter()
            .any(|candidate| compare_geometric_point_v1(point, candidate).is_eq())
    }) {
        return None;
    }

    let mut canonical = None::<Vec<(f64, f64)>>;
    for reversed in [false, true] {
        for start in 0..boundary.len() {
            let candidate = (0..boundary.len())
                .map(|offset| {
                    let index = if reversed {
                        (start + boundary.len() - offset) % boundary.len()
                    } else {
                        (start + offset) % boundary.len()
                    };
                    boundary[index]
                })
                .collect::<Vec<_>>();
            if canonical
                .as_ref()
                .is_none_or(|current| compare_geometric_boundary_v1(&candidate, current).is_lt())
            {
                canonical = Some(candidate);
            }
        }
    }
    canonical
}

fn geometric_boundary_centroid_v1(boundary: &[(f64, f64)]) -> Option<(f64, f64)> {
    let mut points = boundary.to_vec();
    points.sort_unstable_by(compare_geometric_point_v1);
    let (mut x, mut y) = (0.0_f64, 0.0_f64);
    for point in points {
        x += point.0;
        y += point.1;
        if !x.is_finite() || !y.is_finite() {
            return None;
        }
    }
    let denominator = boundary.len() as f64;
    Some((
        canonical_geometric_coordinate_v1(x / denominator)?,
        canonical_geometric_coordinate_v1(y / denominator)?,
    ))
}

pub(super) fn source_face_by_geometric_rank_v1(
    faces: Vec<(FaceId, Vec<(f64, f64)>)>,
    expected_face_count: usize,
    geometric_rank: usize,
) -> Option<FaceId> {
    if expected_face_count == 0
        || faces.len() != expected_face_count
        || geometric_rank >= expected_face_count
        || faces.iter().enumerate().any(|(index, (face, _))| {
            faces[index + 1..]
                .iter()
                .any(|(candidate, _)| candidate == face)
        })
    {
        return None;
    }
    let mut ranked = faces
        .into_iter()
        .map(|(face, boundary)| {
            let canonical_boundary = canonical_geometric_boundary_v1(boundary)?;
            let centroid = geometric_boundary_centroid_v1(&canonical_boundary)?;
            Some(SourceFaceGeometricRankEntryV1 {
                face,
                centroid,
                canonical_boundary,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    let compare = |left: &SourceFaceGeometricRankEntryV1,
                   right: &SourceFaceGeometricRankEntryV1| {
        compare_geometric_point_v1(&left.centroid, &right.centroid).then_with(|| {
            compare_geometric_boundary_v1(&left.canonical_boundary, &right.canonical_boundary)
        })
    };
    ranked.sort_unstable_by(compare);
    if ranked
        .windows(2)
        .any(|window| compare(&window[0], &window[1]).is_eq())
    {
        return None;
    }
    ranked.get(geometric_rank).map(|entry| entry.face)
}

fn four_face_source_project_v1(
    moving_crease: usize,
    fixed_face_geometric_rank: usize,
) -> Option<(
    AppState,
    GlobalFlatFoldabilityState,
    crate::applied_pose::CurrentAppliedPoseCapability,
    crate::global_flat_foldability::CurrentLayerOrderCapability,
    ExpectedStackedFoldCreaseV1,
)> {
    let vertices = FOUR_FACE_POINTS_V1
        .iter()
        .enumerate()
        .map(|(index, &(x, y))| Vertex {
            id: fixed_fixture_entity_id_v1("b420", index as u64 + 1),
            position: Point2::new(x, y),
        })
        .collect::<Vec<_>>();
    let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
    let mut edges = (0..boundary.len())
        .map(|index| Edge {
            id: fixed_fixture_entity_id_v1("b421", index as u64 + 1),
            start: boundary[index],
            end: boundary[(index + 1) % boundary.len()],
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    let stationary_hinges = FOUR_FACE_CREASES_V1
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != moving_crease)
        .map(|(index, &(start, end, kind))| {
            let edge = Edge {
                id: fixed_fixture_entity_id_v1("b421", index as u64 + 11),
                start: boundary[start],
                end: boundary[end],
                kind,
            };
            edges.push(edge.clone());
            edge.id
        })
        .collect::<Vec<_>>();
    let pattern = CreasePattern { vertices, edges };
    let paper = Paper {
        boundary_vertices: boundary,
        thickness_mm: 0.0,
        ..Paper::default()
    };
    let mut project = crate::ProjectState::new_with_paper(pattern, paper);
    project.project_id = fixed_fixture_entity_id_v1::<ProjectId>("b422", 1);
    let topology = project
        .editor
        .topology_analysis_input(project.project_id)
        .analyze();
    let snapshot = topology.simulation_snapshot()?;
    let face_boundaries = snapshot
        .faces
        .iter()
        .map(|face| {
            let boundary = face
                .outer
                .half_edges
                .iter()
                .map(|half_edge| {
                    project
                        .editor
                        .pattern()
                        .vertices
                        .iter()
                        .find(|vertex| vertex.id == half_edge.origin)
                        .map(|vertex| (vertex.position.x, vertex.position.y))
                })
                .collect::<Option<Vec<_>>>()?;
            Some((face.id, boundary))
        })
        .collect::<Option<Vec<_>>>()?;
    let fixed_face =
        source_face_by_geometric_rank_v1(face_boundaries, 3, fixed_face_geometric_rank)?;
    crate::applied_pose::tests::install_pose_authority_with_angles(
        &mut project,
        stationary_hinges
            .into_iter()
            .map(|edge| (edge, 180.0))
            .collect(),
        fixed_face,
    )
    .ok()?;
    let app_state = AppState::new(project);
    let layer_state = GlobalFlatFoldabilityState::default();
    {
        let project = crate::lock_project(&app_state).ok()?;
        let installed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            crate::global_flat_foldability::tests::install_possible_layer_order(
                &layer_state,
                &project,
            );
        }));
        if installed.is_err() {
            return None;
        }
    }
    let project = crate::lock_project(&app_state).ok()?;
    let pose_capability = project
        .applied_pose_authority
        .capture_capability(&project)
        .ok()??;
    let layer_capability = crate::global_flat_foldability::capture_current_layer_order_capability(
        &layer_state,
        &project,
    )
    .ok()??;
    let (start, end, kind) = FOUR_FACE_CREASES_V1[moving_crease];
    let expected = ExpectedStackedFoldCreaseV1 {
        start: project.editor.pattern().vertices[start].position,
        end: project.editor.pattern().vertices[end].position,
        kind,
    };
    drop(project);
    Some((
        app_state,
        layer_state,
        pose_capability,
        layer_capability,
        expected,
    ))
}

fn try_prepare_started_four_face_job_v1(
    moving_crease: usize,
    fixed_face_geometric_rank: usize,
    target_angle_degrees: f64,
) -> Option<(
    AppState,
    StackedFoldTransactionState,
    PostApplyProofJobRequestV1,
    FourFaceFixtureSelectionV1,
)> {
    let (app_state, layer_state, pose_capability, layer_capability, expected) =
        four_face_source_project_v1(moving_crease, fixed_face_geometric_rank)?;
    let project = crate::lock_project(&app_state).ok()?;
    let (source_model, source_pose) = pose_capability.tree()?;
    let source_revision = project.editor.revision();
    let source_fingerprint =
        fold_model_fingerprint_v1(project.editor.pattern(), project.editor.paper()).0;
    let prepared_geometry = prepare_stacked_fold_geometry_candidate_v1(
        project.project_id,
        source_revision,
        project.editor.pattern(),
        project.editor.paper(),
        layer_capability.snapshot(),
        &[expected],
        StackedFoldTopologyBuildLimitsV1::default(),
        FaceLineageLimits::default(),
        StackedFoldGeometryLimitsV1::default(),
    )
    .ok()?;
    let target = prepare_stacked_fold_target_graph_audit_v1(
        prepared_geometry,
        TreeKinematicsLimits::default(),
    )
    .ok()?;
    if target.requires_closure_certificate() {
        return None;
    }
    let source_fixed_face = source_pose.fixed_face()?;
    let fixed_face_lineage = target
        .geometry()
        .proof()
        .lineage()
        .records()
        .iter()
        .find(|record| record.source().face_id == source_fixed_face)?;
    if fixed_face_lineage.descendants().len() != 1 {
        return None;
    }
    let target = prepare_stacked_fold_target_model_v1(
        target.into_geometry(),
        TreeKinematicsLimits::default(),
    )
    .ok()?;
    let initial = prepare_stacked_fold_initial_pose_v1(target, source_model, source_pose).ok()?;
    if initial.target().model().face_ids().len() != 4
        || initial.target().model().hinges().len() != 3
    {
        return None;
    }
    let initial_layer_order = prepare_stacked_fold_initial_layer_order_v1(
        &initial,
        layer_capability.snapshot(),
        DEFAULT_MAX_STACKED_FOLD_NON_FLAT_FACE_PAIRS,
    )
    .ok()?;
    let requested = prepare_stacked_fold_requested_pose_v1(initial, target_angle_degrees).ok()?;
    let source_angles = requested.initial().pose().hinge_angles();
    let target_angles = requested.pose().hinge_angles();
    if layered_four_face_fallback_decision_v1(
        0.0,
        requested.initial().target().model().face_ids().len(),
        requested.initial().target().model().hinges().len(),
        source_angles
            .iter()
            .zip(target_angles)
            .map(|(source, target)| {
                (
                    source.edge() == target.edge(),
                    source.angle_degrees(),
                    target.angle_degrees(),
                )
            }),
        source_angles.len() == target_angles.len(),
    ) != LayeredFourFaceFallbackDecisionV1::LayeredAttempt
    {
        return None;
    }
    let continuous = diagnose_stacked_fold_requested_path_with_initial_layer_order_v1(
        &requested,
        0.0,
        StackedFoldPathDiagnosticLimitsV1::default(),
        &initial_layer_order,
    )
    .ok()?;
    if !crate::stacked_fold_transaction::speculative_tree_diagnostic_is_issuable_v1(&continuous) {
        return None;
    }
    let layer_order = prepare_stacked_fold_non_flat_layer_order_with_thickness_v1(
        &requested,
        layer_capability.snapshot(),
        0.0,
        DEFAULT_MAX_STACKED_FOLD_NON_FLAT_FACE_PAIRS,
    )
    .ok()?;
    let endpoint = diagnose_static_collision_geometry(
        requested.initial().target().model(),
        requested.pose(),
        0.0,
        StaticCollisionLimits::default(),
    )
    .ok()?;
    if endpoint.penetrating_pairs() != 0
        || endpoint
            .pairs()
            .iter()
            .filter(|pair| {
                pair.disposition() == ori_collision::StaticCollisionPairDisposition::Indeterminate
            })
            .any(|pair| {
                pair.evidence() != ori_collision::IntersectionEvidenceV2::SharedFeatureFlatStack
            })
    {
        return None;
    }
    let instance_id = project.instance_id;
    let project_id = project.project_id;
    let pose_generation = pose_capability.generation();
    let layer_generation = layer_capability.generation();
    drop(project);

    let transaction_state = StackedFoldTransactionState::default();
    let token = crate::stacked_fold_transaction::install_pending_speculative_stacked_fold_v1(
        &transaction_state,
        crate::stacked_fold_transaction::PendingSpeculativeStackedFoldPremisesV1 {
            expected_instance_id: instance_id,
            expected_project_id: project_id,
            expected_revision: source_revision,
            expected_source_fingerprint: source_fingerprint,
            expected_pose_generation: pose_generation,
            expected_layer_generation: layer_generation,
            requested,
            continuous,
            diagnostic_paper_thickness_bits: 0.0_f64.to_bits(),
            paper_thickness_mm: 0.0,
            initial_layer_order,
            layer_order,
            endpoint_has_blocking_hold: false,
            endpoint_penetrating_pair_count: 0,
            endpoint_indeterminate_pair_count: 0,
        },
        pose_capability,
        layer_capability,
    )
    .ok()?;
    let revision =
        crate::stacked_fold_transaction::apply_speculative_stacked_fold_transaction_inner_v1(
            &app_state,
            &layer_state,
            &transaction_state,
            crate::stacked_fold_transaction::ApplySpeculativeStackedFoldRequestV1 {
                transaction_token: token,
                explicit_confirmation: true,
            },
        )
        .ok()?;
    let started = start_post_apply_proof_job_inner_v1(
        &app_state,
        &transaction_state,
        StartPostApplyProofJobRequestV1 {
            version: POST_APPLY_PROOF_PROTOCOL_VERSION_V1,
            project_instance_id: instance_id,
            project_id,
            revision,
        },
    )
    .ok()?;
    Some((
        app_state,
        transaction_state,
        PostApplyProofJobRequestV1 {
            version: POST_APPLY_PROOF_PROTOCOL_VERSION_V1,
            project_instance_id: instance_id,
            project_id,
            revision,
            job_token: started.job_token,
        },
        FourFaceFixtureSelectionV1 {
            moving_crease,
            fixed_face_geometric_rank,
        },
    ))
}

fn prepare_started_four_face_job_v1(
    target_angle_degrees: f64,
) -> (
    AppState,
    StackedFoldTransactionState,
    PostApplyProofJobRequestV1,
    FourFaceFixtureSelectionV1,
) {
    assert_ne!(
        FOUR_FACE_MOVING_CREASE_V1, FOUR_FACE_FIXED_NON_SPLIT_GEOMETRIC_RANK_V1,
        "the fixed source face must not be the face split by the moving crease"
    );
    try_prepare_started_four_face_job_v1(
        FOUR_FACE_MOVING_CREASE_V1,
        FOUR_FACE_FIXED_NON_SPLIT_GEOMETRIC_RANK_V1,
        target_angle_degrees,
    )
    .expect(
        "the pinned non-split desktop four-face/three-hinge fixture must remain production-valid",
    )
}

fn take_retained_premise_v1(
    transaction_state: &StackedFoldTransactionState,
) -> PostApplyProofPremiseV1 {
    transaction_state
        .3
        .lock()
        .expect("post-Apply registry")
        .jobs
        .front_mut()
        .and_then(|job| job.premise.take())
        .expect("retained four-face premise")
}

fn prepare_four_face_certified_resolution_job_v1() -> (AppState, PostApplyProofJobV1) {
    let (app_state, transaction_state, _, _) = prepare_started_four_face_job_v1(90.0);
    let mut job = {
        let mut registry = transaction_state.3.lock().expect("post-Apply registry");
        let job = registry.jobs.pop_front().expect("published four-face job");
        registry.retained_bytes = registry.retained_bytes.saturating_sub(job.retained_bytes);
        registry.deadline_scheduler_registered = false;
        job
    };
    let premise = job.premise.take().expect("retained four-face premise");
    let certificate =
        run_layered_four_face_fallback_v1(premise, &CooperativeOperationControlV1::unbounded());
    let PostApplyProofWorkerCertificateV1::Certified(
        PostApplyProofCertifiedAuthorityV1::LayeredFourFace(proof),
    ) = certificate
    else {
        panic!("production four-face chain must issue its distinct typed authority");
    };
    job.resolution_report = None;
    job.state = PostApplyProofJobStateV1::Resolving {
        run_generation: 1,
        resolution: PostApplyProofResolutionV1::Certified(
            PostApplyProofCertifiedAuthorityV1::LayeredFourFace(proof),
        ),
    };
    (app_state, job)
}

include!("layered_four_face_cases.rs");
