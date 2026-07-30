use super::*;
use ori_collision::{StaticCollisionLimits, diagnose_static_collision_geometry};
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

const FIVE_FACE_POINTS_V1: [(f64, f64); 12] = [
    (0.0, 0.0),
    (500.0, 0.0),
    (1_000.0, 0.0),
    (1_900.0, 0.0),
    (2_400.0, 0.0),
    (3_300.0, 0.0),
    (3_300.0, 300.0),
    (2_800.0, 300.0),
    (1_500.0, 300.0),
    (1_400.0, 300.0),
    (100.0, 300.0),
    (0.0, 300.0),
];
const FIVE_FACE_CREASES_V1: [(usize, usize, EdgeKind); 4] = [
    (1, 10, EdgeKind::Mountain),
    (2, 9, EdgeKind::Valley),
    (3, 8, EdgeKind::Mountain),
    (4, 7, EdgeKind::Valley),
];
const FIVE_FACE_MOVING_CREASE_V1: usize = 0;
const FIVE_FACE_FIXED_NON_SPLIT_GEOMETRIC_RANK_V1: usize = 1;

fn five_face_source_project_v1(
    moving_crease: usize,
    fixed_face_geometric_rank: usize,
) -> Option<(
    AppState,
    GlobalFlatFoldabilityState,
    crate::applied_pose::CurrentAppliedPoseCapability,
    crate::global_flat_foldability::CurrentLayerOrderCapability,
    ExpectedStackedFoldCreaseV1,
)> {
    let vertices = FIVE_FACE_POINTS_V1
        .iter()
        .enumerate()
        .map(|(index, &(x, y))| Vertex {
            id: fixed_fixture_entity_id_v1("b430", index as u64 + 1),
            position: Point2::new(x, y),
        })
        .collect::<Vec<_>>();
    let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
    let mut edges = (0..boundary.len())
        .map(|index| Edge {
            id: fixed_fixture_entity_id_v1("b431", index as u64 + 1),
            start: boundary[index],
            end: boundary[(index + 1) % boundary.len()],
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    let stationary_hinges = FIVE_FACE_CREASES_V1
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != moving_crease)
        .map(|(index, &(start, end, kind))| {
            let edge = Edge {
                id: fixed_fixture_entity_id_v1("b431", index as u64 + 13),
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
    project.project_id = fixed_fixture_entity_id_v1::<ProjectId>("b432", 1);
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
        .collect::<Option<Vec<(FaceId, Vec<(f64, f64)>)>>>()?;
    let fixed_face = super::layered_four_face::source_face_by_geometric_rank_v1(
        face_boundaries,
        4,
        fixed_face_geometric_rank,
    )?;
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
    let (start, end, kind) = FIVE_FACE_CREASES_V1[moving_crease];
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

fn try_prepare_started_five_face_job_v1(
    moving_crease: usize,
    fixed_face_geometric_rank: usize,
) -> Option<(
    AppState,
    StackedFoldTransactionState,
    PostApplyProofJobRequestV1,
    usize,
)> {
    let (app_state, layer_state, pose_capability, layer_capability, expected) =
        five_face_source_project_v1(moving_crease, fixed_face_geometric_rank)?;
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
    if initial.target().model().face_ids().len() != 5
        || initial.target().model().hinges().len() != 4
    {
        return None;
    }
    let initial_layer_order = prepare_stacked_fold_initial_layer_order_v1(
        &initial,
        layer_capability.snapshot(),
        DEFAULT_MAX_STACKED_FOLD_NON_FLAT_FACE_PAIRS,
    )
    .ok()?;
    let requested = prepare_stacked_fold_requested_pose_v1(initial, 90.0).ok()?;
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
    let total_pair_count = started.total_pair_count;
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
        total_pair_count,
    ))
}

fn prepare_started_five_face_job_v1() -> (
    AppState,
    StackedFoldTransactionState,
    PostApplyProofJobRequestV1,
    usize,
) {
    assert_ne!(
        FIVE_FACE_MOVING_CREASE_V1, FIVE_FACE_FIXED_NON_SPLIT_GEOMETRIC_RANK_V1,
        "the fixed source face must not be the face split by the moving crease"
    );
    try_prepare_started_five_face_job_v1(
        FIVE_FACE_MOVING_CREASE_V1,
        FIVE_FACE_FIXED_NON_SPLIT_GEOMETRIC_RANK_V1,
    )
    .expect(
        "the pinned non-split admission-only five-face/four-hinge fixture must remain \
         production-valid",
    )
}

#[test]
fn an_actual_admission_only_path_finishes_unknown_without_typed_authority() {
    let _generation_guard =
        crate::stacked_fold_read::tests::lock_stacked_fold_read_generation_test();
    let _deadline_override_guard =
        set_next_post_apply_proof_deadline_v1(Duration::from_secs(5 * 60));
    let (app_state, transaction_state, request, total_pair_count) =
        prepare_started_five_face_job_v1();
    {
        let registry = transaction_state
            .3
            .lock()
            .expect("published admission-only job");
        let premise = registry
            .jobs
            .front()
            .and_then(|job| job.premise.as_ref())
            .expect("retained admission-only premise");
        assert_eq!(
            (
                premise
                    .requested
                    .initial()
                    .target()
                    .model()
                    .face_ids()
                    .len(),
                premise.requested.initial().target().model().hinges().len(),
            ),
            (5, 4),
            "the fixture must remain outside both bounded desktop layered issuers"
        );
        assert!(
            run_direct_certificate_v1(
                premise,
                StackedFoldPathDiagnosticLimitsV1 {
                    sample_intervals: POST_APPLY_PROOF_SAMPLE_INTERVALS_V1[0],
                    static_collision: Default::default(),
                },
                &CooperativeOperationControlV1::unbounded(),
            )
            .expect("ordinary native issuer")
            .is_none(),
            "the admission-only fixture must not mint ordinary tree authority"
        );
        assert!(
            !is_layered_three_face_fallback_candidate_v1(premise),
            "five faces must remain outside three-face typed authority"
        );
        assert!(
            !is_layered_four_face_fallback_candidate_v1(premise),
            "five faces must remain outside four-face typed authority"
        );
    }

    let mut progress = None;
    for _ in POST_APPLY_PROOF_SAMPLE_INTERVALS_V1 {
        progress = Some(
            tauri::async_runtime::block_on(poll_post_apply_proof_job_inner_v1(
                &app_state,
                &transaction_state,
                request.clone(),
            ))
            .expect("bounded post-Apply proof stage"),
        );
    }
    let terminal = progress.expect("the fixed schedule has at least one stage");
    assert_eq!(terminal.status, "unknown_evidence_insufficient");
    assert_eq!(terminal.proven_pair_count, 0);
    assert_eq!(terminal.total_pair_count, total_pair_count);

    let project = crate::lock_project(&app_state).expect("unknown project");
    let summary = project.editor.speculative_unproven_fold_summary_v1();
    assert_eq!(summary.applied.awaiting_proof, 0);
    assert_eq!(summary.applied.unknown_evidence_insufficient, 1);
    assert_eq!(summary.applied.total(), 1);
    drop(project);

    let repeated = tauri::async_runtime::block_on(poll_post_apply_proof_job_inner_v1(
        &app_state,
        &transaction_state,
        request,
    ))
    .expect("terminal poll is idempotent");
    assert_eq!(repeated, terminal);
}
