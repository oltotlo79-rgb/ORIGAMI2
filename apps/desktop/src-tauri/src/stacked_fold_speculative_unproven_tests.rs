fn speculative_tree_request_v1(
    instance: ProjectId,
    project_id: ProjectId,
    revision: u64,
    line_x: f64,
) -> StackedFoldReadRequest {
    StackedFoldReadRequest {
        progress_request_id: None,
        expected_project_instance_id: instance,
        expected_project_id: project_id,
        expected_revision: revision,
        first: [line_x, 0.0, 0.0],
        second: [line_x, 0.0, -400.0],
        fixed_side: FixedSideRequest::Left,
        rotation_direction: RotationDirectionRequest::Positive,
        // The source already contains a parallel crease. Adding and moving
        // only this second crease is sample-nonblocking, while neither the
        // single-hinge nor the all-hinges-moving theorem can certify it.
        requested_angle_degrees: 37.0,
        cycle_schedule_v1: None,
        linear_candidate_v1: None,
        certified_path_graph_v1: None,
    }
}

fn derive_strict_speculative_crossing_line_v1(
    app_state: &AppState,
    layer_state: &GlobalFlatFoldabilityState,
) -> f64 {
    let project = super::super::lock_project(app_state).expect("project");
    let expected_positions = [
        (0.0_f64, 0.0_f64),
        (100.0, 0.0),
        (400.0, 0.0),
        (400.0, 400.0),
        (100.0, 400.0),
        (0.0, 400.0),
    ];
    assert_eq!(
        project
            .editor
            .pattern()
            .vertices
            .iter()
            .map(|vertex| (vertex.position.x, vertex.position.y))
            .collect::<Vec<_>>(),
        expected_positions
    );
    let pose_capability = project
        .applied_pose_authority
        .capture_capability(&project)
        .expect("pose authority lock")
        .expect("flat pose authority");
    let layer_capability = capture_current_layer_order_capability(layer_state, &project)
        .expect("layer authority lock")
        .expect("flat layer authority");
    let snapshot = layer_capability.snapshot();
    assert_eq!(
        snapshot.overlap_cells.len(),
        2,
        "the unequal flat stack must expose one two-ply and one single-ply exact cell"
    );
    let cell_bounds = snapshot
        .overlap_cells
        .iter()
        .map(|cell| {
            let boundary = cell
                .exact_boundary
                .iter()
                .map(|point| {
                    (
                        point.x.to_f64().expect("finite exact x"),
                        point.y.to_f64().expect("finite exact y"),
                    )
                })
                .collect::<Vec<_>>();
            assert_eq!(boundary.len(), 4, "every exact cell must be rectangular");
            let min_x = boundary
                .iter()
                .map(|point| point.0)
                .reduce(f64::min)
                .expect("cell x bound");
            let max_x = boundary
                .iter()
                .map(|point| point.0)
                .reduce(f64::max)
                .expect("cell x bound");
            let min_y = boundary
                .iter()
                .map(|point| point.1)
                .reduce(f64::min)
                .expect("cell y bound");
            let max_y = boundary
                .iter()
                .map(|point| point.1)
                .reduce(f64::max)
                .expect("cell y bound");
            (
                min_x,
                max_x,
                min_y,
                max_y,
                cell.covering_faces.len(),
                boundary,
            )
        })
        .collect::<Vec<_>>();
    let mut exact_cell_shapes = cell_bounds
        .iter()
        .map(|bounds| (bounds.1 - bounds.0, bounds.3 - bounds.2, bounds.4))
        .collect::<Vec<_>>();
    exact_cell_shapes.sort_by(|left, right| {
        left.0
            .total_cmp(&right.0)
            .then_with(|| left.1.total_cmp(&right.1))
            .then_with(|| left.2.cmp(&right.2))
    });
    assert_eq!(
        exact_cell_shapes,
        [(100.0, 400.0, 2), (200.0, 400.0, 1)],
        "the folded source must retain one exact single-ply region outside its overlap, \
         independently of the canonical fixed-face orientation"
    );
    let selected = cell_bounds
        .iter()
        .max_by(|left, right| {
            (left.1 - left.0)
                .total_cmp(&(right.1 - right.0))
                .then_with(|| left.0.total_cmp(&right.0))
        })
        .expect("one widest exact single-ply cell");
    let (min_x, max_x, min_y, max_y) = (selected.0, selected.1, selected.2, selected.3);
    let boundary = &selected.5;
    assert_eq!((min_y, max_y), (0.0, 400.0));
    assert!(min_x.is_finite() && max_x.is_finite() && min_x < max_x);
    let line_x = min_x + (max_x - min_x) / 2.0;
    assert!(
        cell_bounds
            .iter()
            .flat_map(|bounds| &bounds.5)
            .all(|point| point.0 != line_x),
        "the derived line must avoid every exact cell vertex"
    );

    let (model, pose) = pose_capability.tree().expect("flat Tree pose");
    let binding = StackedFoldReadBindingV1::new(
        project.instance_id,
        project.project_id,
        project.editor.revision(),
        pose_capability.generation(),
        layer_capability.generation(),
    );
    let input = FlatEndpointLayerOrderInputV1 {
        identity_namespace: project.project_id,
        source_revision: project.editor.revision(),
        paper: project.editor.paper(),
        pattern: project.editor.pattern(),
        model,
        pose,
        layer_order: snapshot,
    };
    let limits = StackedFoldReadLimitsV1::default();
    let guard =
        capture_stacked_fold_read_guard_v1(binding, input, limits).expect("typed read guard");
    let candidate = StackedFoldLinearCandidateV1::new(
        Point3::new(line_x, 0.0, 0.0).expect("derived first point"),
        Point3::new(line_x, 0.0, -400.0).expect("derived second point"),
        StackedFoldFixedSideV1::Left,
        RotationDirectionRequest::Positive.into(),
        37.0,
    )
    .expect("derived linear candidate");
    let proposal = propose_linear_stacked_fold_read_v1(&guard, binding, input, candidate, limits)
        .unwrap_or_else(|error| {
            panic!(
                "derived line must strictly cross the typed exact cell; \
                 source={expected_positions:?}, cells={cell_bounds:?}, \
                 selected={boundary:?}, line_x={line_x}, error={error:?}"
            )
        });
    let material_map = reverse_map_linear_stacked_fold_material_v1(
        &proposal,
        &guard,
        binding,
        input,
        limits,
        StackedFoldMaterialMapLimitsV1::default(),
    )
    .expect("typed material reverse map");
    let expected_creases = material_map
        .segments()
        .iter()
        .map(|segment| ExpectedStackedFoldCreaseV1 {
            start: segment.start(),
            end: segment.end(),
            kind: segment.assignment(),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        expected_creases.len(),
        1,
        "the exact single-ply cell must add one moving crease"
    );
    let prepared_geometry = prepare_stacked_fold_geometry_candidate_v1(
        project.project_id,
        project.editor.revision(),
        project.editor.pattern(),
        project.editor.paper(),
        snapshot,
        &expected_creases,
        StackedFoldTopologyBuildLimitsV1::default(),
        FaceLineageLimits::default(),
        StackedFoldGeometryLimitsV1::default(),
    )
    .expect("typed geometry candidate");
    let audited_target = prepare_stacked_fold_target_graph_audit_v1(
        prepared_geometry,
        TreeKinematicsLimits::default(),
    )
    .expect("typed target graph audit");
    assert!(
        !audited_target.requires_closure_certificate(),
        "the fixture must remain inside the Tree transaction boundary"
    );
    let prepared_target = prepare_stacked_fold_target_model_v1(
        audited_target.into_geometry(),
        TreeKinematicsLimits::default(),
    )
    .expect("typed target model");
    let prepared_initial_pose = prepare_stacked_fold_initial_pose_v1(prepared_target, model, pose)
        .expect("typed initial pose");
    let initial_collision = diagnose_static_collision_geometry(
        prepared_initial_pose.target().model(),
        prepared_initial_pose.pose(),
        project.editor.paper().thickness_mm,
        StaticCollisionLimits::default(),
    )
    .expect("typed initial collision diagnostic");
    let initial_pair_rows = initial_collision
        .pairs()
        .iter()
        .map(|pair| {
            (
                pair.first_face().canonical_bytes(),
                pair.second_face().canonical_bytes(),
                pair.topology(),
                pair.evidence(),
                pair.policy_decision(),
                pair.disposition(),
                pair.whole_face_overlap_proven(),
                pair.shared_hinge_boundary_contact_proven(),
                pair.shared_hinge_solid_classified(),
            )
        })
        .collect::<Vec<_>>();
    assert!(
        initial_collision.has_prominent_blocking_hold()
            && initial_collision.penetrating_pairs() == 0
            && initial_collision.indeterminate_pairs() == 1
            && initial_collision.pairs().iter().all(|pair| {
                pair.disposition() != ori_collision::StaticCollisionPairDisposition::Indeterminate
                    || pair.evidence()
                        == ori_collision::IntersectionEvidenceV2::SharedFeatureFlatStack
            }),
        "the raw general classifier must fail closed on exactly the source-derived flat-stack \
         pair: penetrating={} indeterminate={}; canonical pair rows={initial_pair_rows:#?}",
        initial_collision.penetrating_pairs(),
        initial_collision.indeterminate_pairs(),
    );
    let moving_hinges = prepared_initial_pose
        .target()
        .geometry()
        .proof()
        .expected_creases()
        .iter()
        .flat_map(|subdivision| subdivision.target_edges().iter().copied())
        .collect::<Vec<_>>();
    let path_limits = StackedFoldPathDiagnosticLimitsV1::default();
    let baseline = diagnose_collective_hinge_path_v1(
        prepared_initial_pose.target().model(),
        prepared_initial_pose.pose(),
        &moving_hinges,
        37.0,
        project.editor.paper().thickness_mm,
        path_limits,
    )
    .expect("typed bounded path diagnostic");
    assert_eq!(
        baseline
            .first_sampled_blocking_angle_degrees()
            .map(f64::to_bits),
        Some(0.0_f64.to_bits()),
        "the ordinary path must preserve the exact initial flat-stack hold"
    );
    let initial_layer_order = prepare_stacked_fold_initial_layer_order_v1(
        &prepared_initial_pose,
        snapshot,
        DEFAULT_MAX_STACKED_FOLD_NON_FLAT_FACE_PAIRS,
    )
    .expect("source-derived initial layer order");
    let prepared_requested_pose =
        prepare_stacked_fold_requested_pose_v1(prepared_initial_pose, 37.0)
            .expect("typed requested pose");
    let diagnostic = diagnose_stacked_fold_requested_path_with_initial_layer_order_v1(
        &prepared_requested_pose,
        project.editor.paper().thickness_mm,
        path_limits,
        &initial_layer_order,
    )
    .expect("strict initial-layer admitted bounded path diagnostic");
    assert!(
        !diagnostic.continuous_clearance_certified(),
        "one new hinge in a larger existing Tree must remain outside V1 certificate models"
    );
    assert_eq!(
        diagnostic.first_sampled_blocking_angle_degrees(),
        None,
        "the layer-admitted speculative path must keep every sampled pose nonblocking: \
         {diagnostic:?}"
    );
    assert_eq!(
        diagnostic.sampled_nonblocking_pose_count(),
        diagnostic.sampled_pose_count(),
        "every approximate sample must remain nonblocking"
    );
    assert!(
        super::super::stacked_fold_transaction::speculative_tree_diagnostic_is_issuable_v1(
            &diagnostic
        )
    );
    let endpoint = diagnose_static_collision_geometry(
        prepared_requested_pose.initial().target().model(),
        prepared_requested_pose.pose(),
        project.editor.paper().thickness_mm,
        StaticCollisionLimits::default(),
    )
    .expect("typed endpoint collision diagnostic");
    assert_eq!(
        endpoint.penetrating_pairs(),
        0,
        "the requested endpoint must remain penetration-free: {endpoint:?}"
    );
    assert_eq!(
        endpoint.indeterminate_pairs(),
        1,
        "only the authenticated persistent flat pair may remain indeterminate: {endpoint:?}"
    );
    let direct_pair_hinge = |first, second| {
        let mut matches = prepared_requested_pose
            .initial()
            .target()
            .model()
            .hinges()
            .iter()
            .filter(|hinge| {
                (hinge.left_face() == first && hinge.right_face() == second)
                    || (hinge.left_face() == second && hinge.right_face() == first)
            });
        let hinge = matches.next().map(|hinge| hinge.edge());
        hinge.filter(|_| matches.next().is_none())
    };
    let persistent_flat_pairs = endpoint
        .pairs()
        .iter()
        .filter(|pair| {
            pair.topology() == ori_collision::TopologyRelation::SharedHingeEdge
                && pair.evidence() == ori_collision::IntersectionEvidenceV2::SharedFeatureFlatStack
                && pair.disposition()
                    == ori_collision::StaticCollisionPairDisposition::Indeterminate
                && direct_pair_hinge(pair.first_face(), pair.second_face())
                    .is_some_and(|hinge| !moving_hinges.contains(&hinge))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        persistent_flat_pairs.len(),
        1,
        "one stationary, source-authenticated flat pair must remain fail-closed: {endpoint:?}"
    );
    let persistent_hinge = direct_pair_hinge(
        persistent_flat_pairs[0].first_face(),
        persistent_flat_pairs[0].second_face(),
    )
    .expect("persistent pair direct hinge");
    assert_eq!(
        prepared_requested_pose
            .pose()
            .hinge_angles()
            .iter()
            .find(|angle| angle.edge() == persistent_hinge)
            .map(|angle| angle.angle_degrees().to_bits()),
        Some(180.0_f64.to_bits())
    );

    let moving_boundary_pairs = endpoint
        .pairs()
        .iter()
        .filter(|pair| {
            pair.topology() == ori_collision::TopologyRelation::SharedHingeEdge
                && pair.evidence() == ori_collision::IntersectionEvidenceV2::SharedFeatureContact
                && pair.disposition() == ori_collision::StaticCollisionPairDisposition::Allowed
                && pair.shared_hinge_boundary_contact_proven()
                && direct_pair_hinge(pair.first_face(), pair.second_face())
                    .is_some_and(|hinge| moving_hinges.contains(&hinge))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        moving_boundary_pairs.len(),
        1,
        "the moving-hinge pair must be independently proven as finite boundary-only contact: \
         {endpoint:?}"
    );
    let moving_hinge = direct_pair_hinge(
        moving_boundary_pairs[0].first_face(),
        moving_boundary_pairs[0].second_face(),
    )
    .expect("moving pair direct hinge");
    assert_eq!(
        prepared_requested_pose
            .pose()
            .hinge_angles()
            .iter()
            .find(|angle| angle.edge() == moving_hinge)
            .map(|angle| angle.angle_degrees().to_bits()),
        Some(37.0_f64.to_bits())
    );
    prepare_stacked_fold_non_flat_layer_order_with_thickness_v1(
        &prepared_requested_pose,
        snapshot,
        project.editor.paper().thickness_mm,
        DEFAULT_MAX_STACKED_FOLD_NON_FLAT_FACE_PAIRS,
    )
    .expect("typed non-flat layer transport");
    line_x
}

fn prepare_speculative_tree_environment_v1() -> (
    AppState,
    GlobalFlatFoldabilityState,
    super::super::stacked_fold_transaction::StackedFoldTransactionState,
    StackedFoldReadRequest,
) {
    let seed = super::super::initial_project_state();
    let positions = [
        ori_domain::Point2::new(0.0, 0.0),
        ori_domain::Point2::new(100.0, 0.0),
        ori_domain::Point2::new(400.0, 0.0),
        ori_domain::Point2::new(400.0, 400.0),
        ori_domain::Point2::new(100.0, 400.0),
        ori_domain::Point2::new(0.0, 400.0),
    ];
    let vertices = positions
        .into_iter()
        .enumerate()
        .map(|(index, position)| ori_domain::Vertex {
            id: fixed_id("9b10", index as u64 + 1),
            position,
        })
        .collect::<Vec<_>>();
    let mut edges = (0..vertices.len())
        .map(|index| ori_domain::Edge {
            id: fixed_id("9b20", index as u64 + 1),
            start: vertices[index].id,
            end: vertices[(index + 1) % vertices.len()].id,
            kind: ori_domain::EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    edges.push(ori_domain::Edge {
        id: fixed_id("9b20", 20),
        start: vertices[1].id,
        end: vertices[4].id,
        kind: ori_domain::EdgeKind::Mountain,
    });
    let pattern = ori_domain::CreasePattern { vertices, edges };
    let mut paper = seed.editor.paper().clone();
    paper.boundary_vertices = pattern.vertices.iter().map(|vertex| vertex.id).collect();
    paper.thickness_mm = 0.0;
    let mut project = super::super::ProjectState::new_with_paper(pattern, paper);
    project.instance_id = fixed_id("9b30", 1);
    project.project_id = fixed_id("9b30", 2);
    project.saved_document = Some(project.document());
    super::super::applied_pose::tests::install_flat_pose_authority(&mut project);
    let instance = project.instance_id;
    let project_id = project.project_id;
    let revision = project.editor.revision();
    let app_state = AppState::new(project);
    let layer_state = GlobalFlatFoldabilityState::default();
    {
        let project = super::super::lock_project(&app_state).expect("project");
        super::super::global_flat_foldability::tests::install_possible_layer_order(
            &layer_state,
            &project,
        );
    }
    let transaction_state =
        super::super::stacked_fold_transaction::StackedFoldTransactionState::default();
    let line_x = derive_strict_speculative_crossing_line_v1(&app_state, &layer_state);
    let request = speculative_tree_request_v1(instance, project_id, revision, line_x);
    (app_state, layer_state, transaction_state, request)
}

pub(crate) fn prepare_speculative_tree_preview_v1() -> (
    AppState,
    GlobalFlatFoldabilityState,
    super::super::stacked_fold_transaction::StackedFoldTransactionState,
    StackedFoldReadResponse,
) {
    let (app_state, layer_state, transaction_state, request) =
        prepare_speculative_tree_environment_v1();
    let response = tauri::async_runtime::block_on(propose_current_stacked_fold_read_inner(
        None,
        &app_state,
        &layer_state,
        &transaction_state,
        request,
    ))
    .expect("speculative Tree preview");
    (app_state, layer_state, transaction_state, response)
}

#[test]
fn tree_samples_without_a_continuous_certificate_publish_only_speculative_authority() {
    let _generation_guard = lock_stacked_fold_read_generation_test();
    let (_, _, transaction_state, response) = prepare_speculative_tree_preview_v1();
    assert_eq!(
        response.transaction_proposal.apply_mode,
        StackedFoldApplyModeDtoV1::SpeculativeUnproven
    );
    assert!(response.transaction_proposal.transaction_token.is_some());
    assert!(response.transaction_proposal.speculative_unproven_available);
    assert!(!response.transaction_proposal.ready_for_atomic_apply);
    assert!(!response.transaction_proposal.authorizes_project_mutation);
    assert!(!response.endpoint_collision.has_blocking_hold);
    assert_eq!(response.endpoint_collision.penetrating_pair_count, 0);
    assert_eq!(response.endpoint_collision.indeterminate_pair_count, 0);
    assert_eq!(
        response.transaction_proposal.failure_classes,
        vec![StackedFoldTransactionFailureClassDto::ContinuousPathUncertified]
    );
    assert!(
        transaction_state
            .speculative_pending_token_for_test_v1()
            .is_some()
    );
}

#[test]
fn prepublication_cancel_stale_and_contract_failure_preserve_the_previous_token() {
    let _generation_guard = lock_stacked_fold_read_generation_test();
    for (action, expected_error) in [
        (1_u8, CANCELLED_MESSAGE),
        (2_u8, STALE_MESSAGE),
        (3_u8, ANALYSIS_FAILED_MESSAGE),
    ] {
        let (app_state, layer_state, transaction_state, request) =
            prepare_speculative_tree_environment_v1();
        let instance = request.expected_project_instance_id;
        let project_id = request.expected_project_id;
        let revision = request.expected_revision;
        let line_x = request.first[0];
        let before = observe_speculative_project_v1(&app_state);
        let previous = tauri::async_runtime::block_on(propose_current_stacked_fold_read_inner(
            None,
            &app_state,
            &layer_state,
            &transaction_state,
            speculative_tree_request_v1(instance, project_id, revision, line_x),
        ))
        .expect("baseline pending proposal");
        let previous_token = speculative_token_v1(&previous);
        assert_eq!(
            transaction_state.speculative_pending_token_for_test_v1(),
            Some(previous_token)
        );
        STACKED_FOLD_PREPUBLICATION_ACTION_V1
            .compare_exchange(0, action, Ordering::AcqRel, Ordering::Acquire)
            .expect("one prepublication test action");

        let error = tauri::async_runtime::block_on(propose_current_stacked_fold_read_inner(
            None,
            &app_state,
            &layer_state,
            &transaction_state,
            request,
        ))
        .expect_err("the injected prepublication race must fail closed");
        assert_eq!(error, expected_error);
        assert_eq!(
            STACKED_FOLD_PREPUBLICATION_ACTION_V1.load(Ordering::Acquire),
            0
        );
        assert_eq!(transaction_state.pending_token_for_test_v1(), None);
        assert_eq!(
            transaction_state.speculative_pending_token_for_test_v1(),
            Some(previous_token),
            "a rejected replacement must not displace the prior valid token"
        );
        assert_speculative_project_observation_v1(&app_state, &before);
        super::super::stacked_fold_transaction::cancel_pending_stacked_fold(
            &transaction_state,
            previous_token,
        )
        .expect("previous token cleanup");

        let retry = tauri::async_runtime::block_on(propose_current_stacked_fold_read_inner(
            None,
            &app_state,
            &layer_state,
            &transaction_state,
            speculative_tree_request_v1(instance, project_id, revision, line_x),
        ))
        .expect("the exact production proposal can be retried after cleanup");
        let retry_token = speculative_token_v1(&retry);
        assert_eq!(
            transaction_state.speculative_pending_token_for_test_v1(),
            Some(retry_token)
        );
        super::super::stacked_fold_transaction::cancel_pending_stacked_fold(
            &transaction_state,
            retry_token,
        )
        .expect("retry token cleanup");
        assert_eq!(transaction_state.pending_token_for_test_v1(), None);
        assert_eq!(
            transaction_state.speculative_pending_token_for_test_v1(),
            None
        );
    }
}

fn speculative_token_v1(response: &StackedFoldReadResponse) -> ProjectId {
    response
        .transaction_proposal
        .transaction_token
        .expect("speculative transaction token")
}

fn apply_speculative_v1(
    app_state: &AppState,
    layer_state: &GlobalFlatFoldabilityState,
    transaction_state: &super::super::stacked_fold_transaction::StackedFoldTransactionState,
    token: ProjectId,
) -> Result<u64, String> {
    super::super::stacked_fold_transaction::apply_speculative_stacked_fold_transaction_inner_v1(
        app_state,
        layer_state,
        transaction_state,
        super::super::stacked_fold_transaction::ApplySpeculativeStackedFoldRequestV1 {
            transaction_token: token,
            explicit_confirmation: true,
        },
    )
}

pub(crate) fn prepare_applied_speculative_project_v1() -> AppState {
    let _generation_guard = lock_stacked_fold_read_generation_test();
    let (app_state, layer_state, transaction_state, response) =
        prepare_speculative_tree_preview_v1();
    let token = speculative_token_v1(&response);
    apply_speculative_v1(&app_state, &layer_state, &transaction_state, token)
        .expect("production speculative preview applies once");
    app_state
}

pub(crate) fn prepare_applied_speculative_project_with_scheduler_v1() -> (
    AppState,
    super::super::stacked_fold_transaction::StackedFoldTransactionState,
    ProjectId,
    ProjectId,
    u64,
) {
    let (app_state, layer_state, transaction_state, response) =
        prepare_speculative_tree_preview_v1();
    let token = speculative_token_v1(&response);
    let revision = apply_speculative_v1(&app_state, &layer_state, &transaction_state, token)
        .expect("production speculative preview applies once");
    let project = super::super::lock_project(&app_state).expect("applied project");
    let instance_id = project.instance_id;
    let project_id = project.project_id;
    drop(project);
    (
        app_state,
        transaction_state,
        instance_id,
        project_id,
        revision,
    )
}

#[derive(Clone)]
struct SpeculativeProjectObservationV1 {
    instance_id: ProjectId,
    project_id: ProjectId,
    document: super::super::ProjectDocument,
    revision: u64,
    history: ori_core::EditorHistoryV1,
    marker: ori_core::SpeculativeUnprovenFoldStateMarkerV1,
    applied_pose_authority: super::super::applied_pose::CurrentAppliedPoseAuthoritySnapshot,
    pair_cache_progress: ori_collision::ProofCacheProgressV1,
    current_layer_evidence: Option<super::super::stacked_fold_transaction::CurrentLayerEvidence>,
    saved_revision: Option<u64>,
    saved_document: Option<super::super::ProjectDocument>,
    saved_speculative_unproven_state: Option<ori_core::SpeculativeUnprovenFoldStateMarkerV1>,
    is_dirty: bool,
}

fn observe_speculative_project_v1(app_state: &AppState) -> SpeculativeProjectObservationV1 {
    let project = super::super::lock_project(app_state).expect("project");
    SpeculativeProjectObservationV1 {
        instance_id: project.instance_id,
        project_id: project.project_id,
        document: project.document(),
        revision: project.editor.revision(),
        history: project
            .editor
            .export_history_v1(project.project_id)
            .expect("canonical editor history"),
        marker: project.editor.speculative_unproven_fold_state_marker_v1(),
        applied_pose_authority: project
            .applied_pose_authority
            .test_snapshot()
            .expect("applied-pose authority snapshot"),
        pair_cache_progress: project
            .applied_pose_authority
            .pair_proof_cache_runtime_v1()
            .progress_v1()
            .expect("pair-cache progress snapshot"),
        current_layer_evidence: project.current_layer_evidence.clone(),
        saved_revision: project.saved_revision,
        saved_document: project.saved_document.clone(),
        saved_speculative_unproven_state: project.saved_speculative_unproven_state.clone(),
        is_dirty: project.is_dirty(),
    }
}

fn assert_speculative_project_observation_v1(
    app_state: &AppState,
    expected: &SpeculativeProjectObservationV1,
) {
    let actual = observe_speculative_project_v1(app_state);
    assert_eq!(actual.instance_id, expected.instance_id);
    assert_eq!(actual.project_id, expected.project_id);
    assert_eq!(actual.document, expected.document);
    assert_eq!(actual.revision, expected.revision);
    assert_eq!(actual.history, expected.history);
    assert_eq!(actual.marker, expected.marker);
    assert_eq!(
        actual.applied_pose_authority,
        expected.applied_pose_authority
    );
    assert_eq!(actual.pair_cache_progress, expected.pair_cache_progress);
    assert_eq!(
        actual.current_layer_evidence,
        expected.current_layer_evidence
    );
    assert_eq!(actual.saved_revision, expected.saved_revision);
    assert_eq!(actual.saved_document, expected.saved_document);
    assert_eq!(
        actual.saved_speculative_unproven_state,
        expected.saved_speculative_unproven_state
    );
    assert_eq!(actual.is_dirty, expected.is_dirty);
}

fn assert_consumed_speculative_token_v1(
    transaction_state: &super::super::stacked_fold_transaction::StackedFoldTransactionState,
    token: ProjectId,
) {
    assert_eq!(
        transaction_state.speculative_pending_token_for_test_v1(),
        None
    );
    assert!(
        super::super::stacked_fold_transaction::cancel_pending_stacked_fold(
            transaction_state,
            token,
        )
        .is_err(),
        "a consumed speculative token must not remain cancellable"
    );
}

#[test]
fn actual_speculative_pending_apply_is_one_atomic_marked_history_entry_and_one_shot() {
    let _generation_guard = lock_stacked_fold_read_generation_test();
    let (app_state, layer_state, transaction_state, response) =
        prepare_speculative_tree_preview_v1();
    let token = speculative_token_v1(&response);
    let before = observe_speculative_project_v1(&app_state);
    assert_eq!(before.history.undo_len(), 0);
    assert!(before.document.instruction_timeline.steps.is_empty());

    let applied = apply_speculative_v1(&app_state, &layer_state, &transaction_state, token)
        .expect("explicitly confirmed speculative Apply");
    assert_eq!(applied, before.revision + 1);

    let after_first = observe_speculative_project_v1(&app_state);
    assert_eq!(after_first.revision, applied);
    assert_eq!(after_first.history.undo_len(), 1);
    assert_eq!(after_first.history.redo_len(), 0);
    assert_eq!(
        after_first.document.instruction_timeline.steps.len(),
        before.document.instruction_timeline.steps.len() + 1
    );
    assert_eq!(
        after_first
            .document
            .instruction_timeline
            .steps
            .last()
            .expect("speculative timeline step")
            .pose
            .source_model_fingerprint,
        response.transaction_proposal.target_fingerprint_sha256
    );
    assert_eq!(
        super::super::lock_project(&app_state)
            .expect("project")
            .editor
            .fold_model_fingerprint_v1(),
        response.transaction_proposal.target_fingerprint_sha256
    );
    let summary = super::super::lock_project(&app_state)
        .expect("project")
        .editor
        .speculative_unproven_fold_summary_v1();
    assert_eq!(summary.applied.awaiting_proof, 1);
    assert_eq!(summary.unapplied_redo.awaiting_proof, 0);

    assert!(apply_speculative_v1(&app_state, &layer_state, &transaction_state, token).is_err());
    assert_speculative_project_observation_v1(&app_state, &after_first);
    assert_consumed_speculative_token_v1(&transaction_state, token);
}

#[test]
fn actual_pending_revision_drift_is_rejected_consumed_and_nonmutating() {
    let _generation_guard = lock_stacked_fold_read_generation_test();
    let (app_state, layer_state, transaction_state, response) =
        prepare_speculative_tree_preview_v1();
    let token = speculative_token_v1(&response);
    {
        let mut project = super::super::lock_project(&app_state).expect("project");
        let revision = project.editor.revision();
        project
            .editor
            .execute(
                revision,
                ori_core::Command::UpdateProjectMemo {
                    memo: "stale speculative revision".to_owned(),
                },
            )
            .expect("advance revision");
    }
    let before_apply = observe_speculative_project_v1(&app_state);
    assert!(apply_speculative_v1(&app_state, &layer_state, &transaction_state, token).is_err());
    assert_speculative_project_observation_v1(&app_state, &before_apply);
    assert_consumed_speculative_token_v1(&transaction_state, token);
}

fn replace_live_editor_without_revision_v1(
    app_state: &AppState,
    mutate: impl FnOnce(&mut super::super::ProjectDocument),
) {
    let mut project = super::super::lock_project(app_state).expect("project");
    assert_eq!(
        project.editor.revision(),
        0,
        "fixture starts at revision zero"
    );
    let mut document = project.document();
    mutate(&mut document);
    let replacement = super::super::ProjectState::from_valid_document(
        document,
        std::path::PathBuf::from("speculative-live-drift.ori"),
    );
    assert_eq!(replacement.editor.revision(), 0);
    project.editor = replacement.editor;
}

#[test]
fn actual_pending_geometry_fingerprint_drift_is_rejected_consumed_and_nonmutating() {
    let _generation_guard = lock_stacked_fold_read_generation_test();
    let (app_state, layer_state, transaction_state, response) =
        prepare_speculative_tree_preview_v1();
    let token = speculative_token_v1(&response);
    let original_fingerprint = super::super::lock_project(&app_state)
        .expect("project")
        .editor
        .fold_model_fingerprint_v1();
    replace_live_editor_without_revision_v1(&app_state, |document| {
        document.paper.cutting_allowed = !document.paper.cutting_allowed;
    });
    let before_apply = observe_speculative_project_v1(&app_state);
    assert_ne!(
        super::super::lock_project(&app_state)
            .expect("project")
            .editor
            .fold_model_fingerprint_v1(),
        original_fingerprint
    );
    assert_eq!(before_apply.revision, 0);
    assert!(apply_speculative_v1(&app_state, &layer_state, &transaction_state, token).is_err());
    assert_speculative_project_observation_v1(&app_state, &before_apply);
    assert_consumed_speculative_token_v1(&transaction_state, token);
}

#[test]
fn actual_pending_pose_generation_drift_is_rejected_consumed_and_nonmutating() {
    let _generation_guard = lock_stacked_fold_read_generation_test();
    let (app_state, layer_state, transaction_state, response) =
        prepare_speculative_tree_preview_v1();
    let token = speculative_token_v1(&response);
    {
        let mut project = super::super::lock_project(&app_state).expect("project");
        super::super::applied_pose::tests::install_flat_pose_authority(&mut project);
    }
    let before_apply = observe_speculative_project_v1(&app_state);
    assert!(apply_speculative_v1(&app_state, &layer_state, &transaction_state, token).is_err());
    assert_speculative_project_observation_v1(&app_state, &before_apply);
    assert_consumed_speculative_token_v1(&transaction_state, token);
}

#[test]
fn actual_pending_layer_generation_drift_is_rejected_consumed_and_nonmutating() {
    let _generation_guard = lock_stacked_fold_read_generation_test();
    let (app_state, layer_state, transaction_state, response) =
        prepare_speculative_tree_preview_v1();
    let token = speculative_token_v1(&response);
    {
        let project = super::super::lock_project(&app_state).expect("project");
        super::super::global_flat_foldability::tests::install_possible_layer_order(
            &layer_state,
            &project,
        );
    }
    let before_apply = observe_speculative_project_v1(&app_state);
    assert!(apply_speculative_v1(&app_state, &layer_state, &transaction_state, token).is_err());
    assert_speculative_project_observation_v1(&app_state, &before_apply);
    assert_consumed_speculative_token_v1(&transaction_state, token);
}

#[test]
fn actual_pending_thickness_one_ulp_drift_is_rejected_consumed_and_nonmutating() {
    let _generation_guard = lock_stacked_fold_read_generation_test();
    let (app_state, layer_state, transaction_state, response) =
        prepare_speculative_tree_preview_v1();
    let token = speculative_token_v1(&response);
    let old_thickness_bits = super::super::lock_project(&app_state)
        .expect("project")
        .editor
        .paper()
        .thickness_mm
        .to_bits();
    replace_live_editor_without_revision_v1(&app_state, |document| {
        document.paper.thickness_mm = f64::from_bits(old_thickness_bits + 1);
    });
    let before_apply = observe_speculative_project_v1(&app_state);
    assert_eq!(
        super::super::lock_project(&app_state)
            .expect("project")
            .editor
            .paper()
            .thickness_mm
            .to_bits(),
        old_thickness_bits + 1
    );
    assert!(apply_speculative_v1(&app_state, &layer_state, &transaction_state, token).is_err());
    assert_speculative_project_observation_v1(&app_state, &before_apply);
    assert_consumed_speculative_token_v1(&transaction_state, token);
}

#[test]
fn actual_pending_project_instance_drift_is_rejected_consumed_and_nonmutating() {
    let _generation_guard = lock_stacked_fold_read_generation_test();
    let (app_state, layer_state, transaction_state, response) =
        prepare_speculative_tree_preview_v1();
    let token = speculative_token_v1(&response);
    super::super::lock_project(&app_state)
        .expect("project")
        .instance_id = ProjectId::new();
    let before_apply = observe_speculative_project_v1(&app_state);
    assert!(apply_speculative_v1(&app_state, &layer_state, &transaction_state, token).is_err());
    assert_speculative_project_observation_v1(&app_state, &before_apply);
    assert_consumed_speculative_token_v1(&transaction_state, token);
}

#[test]
fn actual_pending_project_id_drift_is_rejected_consumed_and_nonmutating() {
    let _generation_guard = lock_stacked_fold_read_generation_test();
    let (app_state, layer_state, transaction_state, response) =
        prepare_speculative_tree_preview_v1();
    let token = speculative_token_v1(&response);
    super::super::lock_project(&app_state)
        .expect("project")
        .project_id = ProjectId::new();
    let before_apply = observe_speculative_project_v1(&app_state);
    assert!(apply_speculative_v1(&app_state, &layer_state, &transaction_state, token).is_err());
    assert_speculative_project_observation_v1(&app_state, &before_apply);
    assert_consumed_speculative_token_v1(&transaction_state, token);
}

#[test]
fn actual_pending_wrong_request_generation_is_rejected_without_consuming_the_real_token() {
    let _generation_guard = lock_stacked_fold_read_generation_test();
    let (app_state, layer_state, transaction_state, response) =
        prepare_speculative_tree_preview_v1();
    let token = speculative_token_v1(&response);
    let before_apply = observe_speculative_project_v1(&app_state);
    assert!(
        apply_speculative_v1(
            &app_state,
            &layer_state,
            &transaction_state,
            ProjectId::new(),
        )
        .is_err()
    );
    assert_speculative_project_observation_v1(&app_state, &before_apply);
    assert_eq!(
        transaction_state.speculative_pending_token_for_test_v1(),
        Some(token)
    );
    apply_speculative_v1(&app_state, &layer_state, &transaction_state, token)
        .expect("the bit-exact production request generation remains valid");
}

#[test]
fn actual_speculative_pending_rejects_cross_mode_apply_without_consuming_it() {
    let _generation_guard = lock_stacked_fold_read_generation_test();
    let (app_state, layer_state, transaction_state, response) =
        prepare_speculative_tree_preview_v1();
    let token = speculative_token_v1(&response);
    let before = observe_speculative_project_v1(&app_state);
    assert!(
        super::super::stacked_fold_transaction::apply_stacked_fold_transaction_inner(
            &app_state,
            &layer_state,
            &transaction_state,
            token,
        )
        .is_err()
    );
    assert_speculative_project_observation_v1(&app_state, &before);
    assert_eq!(
        transaction_state.speculative_pending_token_for_test_v1(),
        Some(token)
    );
    apply_speculative_v1(&app_state, &layer_state, &transaction_state, token)
        .expect("correct explicit speculative command still consumes its own token");
}

#[test]
fn speculative_commit_target_pose_reissue_failure_rolls_back_complete_editor() {
    let _generation_guard = lock_stacked_fold_read_generation_test();
    let (app_state, layer_state, transaction_state, response) =
        prepare_speculative_tree_preview_v1();
    let token = speculative_token_v1(&response);
    let before = observe_speculative_project_v1(&app_state);
    let (pose_capability_before, layer_capability_before) = {
        let project = super::super::lock_project(&app_state).expect("project");
        (
            project
                .applied_pose_authority
                .capture_capability(&project)
                .expect("pose capability capture")
                .expect("current pose capability"),
            super::super::global_flat_foldability::capture_current_layer_order_capability(
                &layer_state,
                &project,
            )
            .expect("layer capability capture")
            .expect("current layer capability"),
        )
    };
    let _target_pose_reissue_failure_guard =
        super::super::stacked_fold_transaction::fail_next_speculative_target_pose_reissue_for_test_v1(
        );
    let error = apply_speculative_v1(&app_state, &layer_state, &transaction_state, token)
        .expect_err("injected target-pose reissue must fail after the editor commit");
    assert_eq!(
        error,
        "The target pose authority could not be installed atomically."
    );
    assert_speculative_project_observation_v1(&app_state, &before);
    {
        let project = super::super::lock_project(&app_state).expect("project");
        assert!(
            project
                .applied_pose_authority
                .revalidate_capability(&project, &pose_capability_before)
                .expect("pose capability revalidation")
                .is_some(),
            "rollback must restore the exact current pose certificate"
        );
        assert!(
            super::super::global_flat_foldability::revalidate_current_layer_order_capability(
                &layer_state,
                &project,
                &layer_capability_before,
            )
            .expect("layer capability revalidation")
            .is_some(),
            "rollback must preserve the exact current layer certificate"
        );
    }
    assert_consumed_speculative_token_v1(&transaction_state, token);
}
