fn bounded_multi_block_projective_unfold_schedule_v1(
    hinges: &[ori_domain::EdgeId],
    active_hinges: &[ori_domain::EdgeId],
) -> CycleScheduleRequestV1 {
    let source_numerator = 1_i64;
    let numerator_slope = 1_i64;
    let denominator = 64_i64;
    let requested_angle_degrees = ori_kinematics::deterministic_half_angle_ratio_degrees_v1(
        (source_numerator + numerator_slope) as f64,
        denominator as f64,
    )
    .expect("the projective small-angle endpoint is finite");
    let mut entries = hinges
        .iter()
        .copied()
        .map(|edge| {
            let active = active_hinges.contains(&edge);
            CycleScheduleEntryRequestV1 {
                edge,
                u_domain: [
                    RationalCoefficientRequestV1 {
                        numerator: 0,
                        denominator: 1,
                    },
                    RationalCoefficientRequestV1 {
                        numerator: 1,
                        denominator: 1,
                    },
                ],
                // Active collinear cuts use P(u)=1+u, Q(u)=64. Both
                // endpoints are reconstructed by the same deterministic
                // half-angle evaluator as the live pose. Every other hinge
                // is the exact constant P(u)=0, Q(u)=1 schedule.
                numerator_power_coefficients: if active {
                    vec![
                        RationalCoefficientRequestV1 {
                            numerator: source_numerator,
                            denominator: 1,
                        },
                        RationalCoefficientRequestV1 {
                            numerator: numerator_slope,
                            denominator: 1,
                        },
                    ]
                } else {
                    vec![RationalCoefficientRequestV1 {
                        numerator: 0,
                        denominator: 1,
                    }]
                },
                denominator_power_coefficients: vec![RationalCoefficientRequestV1 {
                    numerator: if active { denominator } else { 1 },
                    denominator: 1,
                }],
                requested_angle_degrees: if active { requested_angle_degrees } else { 0.0 },
            }
        })
        .collect::<Vec<_>>();
    entries.sort_unstable_by_key(|entry| entry.edge.canonical_bytes());
    CycleScheduleRequestV1 {
        version: 1,
        entries,
        endpoint_denominator: None,
    }
}

fn bounded_multi_block_projective_active_source_angle_v1() -> f64 {
    ori_kinematics::deterministic_half_angle_ratio_degrees_v1(1.0, 64.0)
        .expect("the projective small-angle source is finite")
}

#[test]
fn four_and_five_block_opposite_bifolds_preview_apply_and_reopen_history() {
    let _generation_guard = lock_stacked_fold_read_generation_test();
    for block_count in [4, 5] {
        assert_opposite_bifold_lifecycle_v1(block_count);
    }
}

fn assert_opposite_bifold_lifecycle_v1(block_count: usize) {
    let (pattern, paper, moving) = match block_count {
        4 => super::four_bay_cycle_test_support::four_bay_opposite_bifold_pattern(),
        5 => super::four_bay_cycle_test_support::five_bay_opposite_bifold_pattern(),
        _ => unreachable!(),
    };
    assert_eq!(moving.len(), block_count * 2);
    assert!(paper.thickness_mm > 0.0);
    let mut project = super::super::ProjectState::new_with_paper(pattern, paper);
    let topology = project
        .editor
        .topology_analysis_input(project.project_id)
        .analyze();
    let document = topology
        .simulation_snapshot()
        .expect("bounded opposite-bifold topology");
    assert_eq!(document.faces.len(), block_count * 5 + 1);
    assert_eq!(document.hinge_adjacency.len(), block_count * 6);
    let hinges = document
        .hinge_adjacency
        .iter()
        .map(|hinge| hinge.edge)
        .collect::<Vec<_>>();
    let fixed_face = document
        .faces
        .iter()
        .max_by_key(|face| face.outer.half_edges.len())
        .expect("the exterior articulation face")
        .id;
    super::super::applied_pose::tests::install_flat_graph_pose_authority_on_face(
        &mut project,
        hinges.clone(),
        fixed_face,
    );
    let layer_state = GlobalFlatFoldabilityState::default();
    super::super::global_flat_foldability::tests::install_possible_layer_order(
        &layer_state,
        &project,
    );
    let instance = project.instance_id;
    let project_id = project.project_id;
    let revision = project.editor.revision();
    let schedule_request = dense_grid_schedule(&hinges, &moving, 100);
    let app_state = AppState::new(project);
    let transactions =
        super::super::stacked_fold_transaction::StackedFoldTransactionState::default();
    let preview = propose_current_cycle_pose_inner_with_layers(
        None,
        &app_state,
        Some(&layer_state),
        &transactions,
        CurrentCyclePosePreviewRequestV1 {
            progress_request_id: None,
            expected_project_instance_id: instance,
            expected_project_id: project_id,
            expected_revision: revision,
            cycle_schedule_v1: schedule_request,
        },
    )
    .expect("four/five separated radial-bifold blocks certify");
    assert_eq!(preview.source_revision, revision);
    assert_eq!(preview.target_revision, revision + 1);
    assert!(preview.continuous_path_certified);
    assert_eq!(
        preview.continuous_layer_transport_model_id,
        Some(ori_collision::COMMON_ARTICULATION_CONTINUOUS_LAYER_PATH_MODEL_ID_V1)
    );
    assert!(preview.continuous_layer_transition_count > 0);
    assert_eq!(
        preview.continuous_layer_pair_order_count,
        preview.source_layer_order.len()
    );
    assert_eq!(preview.source_layer_order, preview.target_layer_order);
    assert!(!preview.authorizes_project_mutation);
    assert_eq!(
        transactions.pending_token_for_test_v1(),
        Some(preview.transaction_token)
    );
    let applied = super::super::stacked_fold_transaction::apply_stacked_fold_transaction_inner(
        &app_state,
        &layer_state,
        &transactions,
        preview.transaction_token,
    )
    .expect("bounded positive-thickness preview applies atomically");
    assert!(
        super::super::stacked_fold_transaction::apply_stacked_fold_transaction_inner(
            &app_state,
            &layer_state,
            &transactions,
            preview.transaction_token,
        )
        .is_err(),
        "a consumed bounded multi-block preview is one-shot",
    );

    let mut project = super::super::lock_project(&app_state)
        .unwrap_or_else(|_| panic!("project after {block_count}-block Apply"));
    assert_eq!(applied, revision + 1);
    assert_eq!(project.editor.revision(), applied);
    assert_eq!(project.editor.instruction_timeline().steps.len(), 2);
    assert!(
        project.editor.instruction_timeline().steps[0]
            .visual
            .path_certificate_reference_v1
            .is_none()
    );
    assert!(
        project.editor.instruction_timeline().steps[1]
            .visual
            .path_certificate_reference_v1
            .is_some()
    );
    project
        .editor
        .undo(applied)
        .unwrap_or_else(|_| panic!("undo {block_count}-block Apply"));
    assert!(project.editor.instruction_timeline().steps.is_empty());
    let undone = project.editor.revision();
    project
        .editor
        .redo(undone)
        .unwrap_or_else(|_| panic!("redo {block_count}-block Apply"));
    assert_eq!(project.editor.instruction_timeline().steps.len(), 2);
    assert!(
        project.editor.instruction_timeline().steps[0]
            .visual
            .path_certificate_reference_v1
            .is_none()
    );
    assert!(
        project.editor.instruction_timeline().steps[1]
            .visual
            .path_certificate_reference_v1
            .is_some()
    );
    {
        let timeline = project.editor.instruction_timeline();
        let attestation = project
            .trusted_path_certificates
            .export_attestation_v1(project.instance_id, project.project_id, timeline)
            .unwrap_or_else(|_| {
                panic!("live {block_count}-block registry remains internally consistent")
            })
            .unwrap_or_else(|| {
                panic!("real {block_count}-block certificate attests the target timeline step")
            });
        let topology = project
            .editor
            .topology_analysis_input(project.project_id)
            .analyze();
        let snapshot = topology
            .simulation_snapshot()
            .unwrap_or_else(|| panic!("applied {block_count}-block topology remains exportable"));
        let model = project.editor.fold_model_fingerprint_v1();
        let mut one_ulp_tampered = timeline.clone();
        let tampered_angle = one_ulp_tampered.steps[1]
            .pose
            .hinge_angles
            .iter_mut()
            .find(|angle| angle.angle_degrees != 0.0)
            .unwrap_or_else(|| panic!("{block_count}-block target has one moving hinge"));
        tampered_angle.angle_degrees =
            f64::from_bits(tampered_angle.angle_degrees.to_bits().wrapping_add(1));
        for format in [
            ori_formats::InstructionExportFormat::Pdf17,
            ori_formats::InstructionExportFormat::SvgPageZip,
        ] {
            assert!(matches!(
                ori_formats::export_instruction_document(
                    format,
                    &project.name,
                    &model,
                    project.editor.pattern(),
                    project.editor.paper(),
                    timeline,
                    snapshot,
                ),
                Err(ori_formats::InstructionExportError::InvalidPathCertificateReference { .. })
            ));
            let artifact =
                ori_formats::export_instruction_document_with_path_certificate_attestation_v1(
                    format,
                    &project.name,
                    &model,
                    project.editor.pattern(),
                    project.editor.paper(),
                    timeline,
                    snapshot,
                    &attestation,
                )
                .unwrap_or_else(|_| {
                    panic!("the attested closed {block_count}-block graph renders natively")
                });
            assert_eq!(artifact.format, format);
            assert_eq!(artifact.step_count, 2);
            assert!(artifact.page_count >= artifact.step_count);
            assert!(!artifact.bytes.is_empty());
            assert!(matches!(
                ori_formats::export_instruction_document_with_path_certificate_attestation_v1(
                    format,
                    &project.name,
                    &model,
                    project.editor.pattern(),
                    project.editor.paper(),
                    &one_ulp_tampered,
                    snapshot,
                    &attestation,
                ),
                Err(ori_formats::InstructionExportError::InvalidPathCertificateReference { .. })
            ), "one-ULP endpoint drift cannot reuse the exact native path attestation");
        }
    }
    let archive = project
        .project_archive()
        .unwrap_or_else(|_| panic!("archive the redone {block_count}-block Apply"));
    drop(project);

    let mut reopened = super::super::ProjectState::from_project_archive(
        archive,
        std::path::PathBuf::from(format!("{block_count}-block-opposite-bifold.ori2")),
    )
    .unwrap_or_else(|_| panic!("reopen the {block_count}-block archive"));
    assert_eq!(reopened.editor.instruction_timeline().steps.len(), 2);
    assert!(
        reopened.editor.instruction_timeline().steps[0]
            .visual
            .path_certificate_reference_v1
            .is_none()
    );
    assert!(
        reopened.editor.instruction_timeline().steps[1]
            .visual
            .path_certificate_reference_v1
            .is_some()
    );
    assert!(
        reopened
            .applied_pose_authority
            .capture_capability(&reopened)
            .expect("reopened pose authority")
            .is_some()
    );
    let reopened_revision = reopened.editor.revision();
    reopened
        .editor
        .undo(reopened_revision)
        .expect("undo after reopen");
    assert!(reopened.editor.instruction_timeline().steps.is_empty());
    let reopened_undone = reopened.editor.revision();
    reopened
        .editor
        .redo(reopened_undone)
        .expect("redo after reopen");
    assert_eq!(reopened.editor.instruction_timeline().steps.len(), 2);
    assert!(
        reopened.editor.instruction_timeline().steps[0]
            .visual
            .path_certificate_reference_v1
            .is_none()
    );
    assert!(
        reopened.editor.instruction_timeline().steps[1]
            .visual
            .path_certificate_reference_v1
            .is_some()
    );

    let project = super::super::lock_project(&app_state)
        .unwrap_or_else(|_| panic!("project after {block_count}-block lifecycle"));
    assert!(project.editor.revision() > applied);
    assert_eq!(project.editor.instruction_timeline().steps.len(), 2);
}

#[test]
fn five_block_radial_bifold_tamper_fails_closed_without_transaction() {
    let _generation_guard = lock_stacked_fold_read_generation_test();
    let (pattern, paper, moving) =
        super::four_bay_cycle_test_support::five_bay_opposite_bifold_pattern();
    let first_center = pattern
        .edges
        .iter()
        .find(|edge| edge.id == moving[0])
        .expect("first genuine active radial hinge")
        .start;
    let adjacent_nonopposite = pattern
        .edges
        .iter()
        .find(|edge| edge.start == first_center && !moving.contains(&edge.id))
        .expect("adjacent non-opposite radial hinge")
        .id;
    let mut tampered_moving = moving.clone();
    tampered_moving[1] = adjacent_nonopposite;

    let mut project = super::super::ProjectState::new_with_paper(pattern, paper);
    let topology = project
        .editor
        .topology_analysis_input(project.project_id)
        .analyze();
    let document = topology
        .simulation_snapshot()
        .expect("five-block opposite-bifold topology");
    assert_eq!(
        (document.faces.len(), document.hinge_adjacency.len()),
        (26, 30)
    );
    let hinges = document
        .hinge_adjacency
        .iter()
        .map(|hinge| hinge.edge)
        .collect::<Vec<_>>();
    let fixed_face = document
        .faces
        .iter()
        .max_by_key(|face| face.outer.half_edges.len())
        .expect("five-block exterior articulation face")
        .id;
    super::super::applied_pose::tests::install_flat_graph_pose_authority_on_face(
        &mut project,
        hinges.clone(),
        fixed_face,
    );
    let layer_state = GlobalFlatFoldabilityState::default();
    super::super::global_flat_foldability::tests::install_possible_layer_order(
        &layer_state,
        &project,
    );
    let instance = project.instance_id;
    let project_id = project.project_id;
    let revision = project.editor.revision();
    let app_state = AppState::new(project);
    let transactions =
        super::super::stacked_fold_transaction::StackedFoldTransactionState::default();
    assert!(
        propose_current_cycle_pose_inner_with_layers(
            None,
            &app_state,
            Some(&layer_state),
            &transactions,
            CurrentCyclePosePreviewRequestV1 {
                progress_request_id: None,
                expected_project_instance_id: instance,
                expected_project_id: project_id,
                expected_revision: revision,
                cycle_schedule_v1: dense_grid_schedule(&hinges, &tampered_moving, 100),
            },
        )
        .is_err(),
        "a non-opposite active pair must not impersonate a radial bifold",
    );
    assert_eq!(transactions.pending_token_for_test_v1(), None);
    let project = super::super::lock_project(&app_state)
        .expect("project after five-block radial-bifold rejection");
    assert_eq!(project.editor.revision(), revision);
    assert!(project.editor.instruction_timeline().steps.is_empty());
}

#[test]
fn six_block_strip_remains_outside_bounded_multi_block_current_cycle_authority() {
    let _generation_guard = lock_stacked_fold_read_generation_test();
    let namespace = ProjectId::new();
    let (pattern, paper, hinges) = common_articulation_strip_fixture_v1(7);
    assert_eq!(hinges.len(), 6);
    let document = analyze_faces(FaceExtractionInput {
        identity_namespace: namespace,
        source_revision: 1,
        paper: &paper,
        pattern: &pattern,
    })
    .snapshot
    .expect("six-block strip topology");
    assert_eq!(document.faces.len(), 7);
    assert_eq!(document.hinge_adjacency.len(), 6);

    let mut project = super::super::ProjectState::new_with_paper(pattern, paper);
    project.project_id = namespace;
    super::super::applied_pose::tests::install_tree_pose_authority_at_angle_on_face(
        &mut project,
        hinges.clone(),
        document.faces[0].id,
        bounded_multi_block_projective_active_source_angle_v1(),
    );
    let layer_state = GlobalFlatFoldabilityState::default();
    super::super::global_flat_foldability::tests::install_possible_layer_order(
        &layer_state,
        &project,
    );
    let instance = project.instance_id;
    let project_id = project.project_id;
    let revision = project.editor.revision();
    let app_state = AppState::new(project);
    let transactions =
        super::super::stacked_fold_transaction::StackedFoldTransactionState::default();
    let error = propose_current_cycle_pose_inner_with_layers(
        None,
        &app_state,
        Some(&layer_state),
        &transactions,
        CurrentCyclePosePreviewRequestV1 {
            progress_request_id: None,
            expected_project_instance_id: instance,
            expected_project_id: project_id,
            expected_revision: revision,
            cycle_schedule_v1: bounded_multi_block_projective_unfold_schedule_v1(&hinges, &hinges),
        },
    )
    .expect_err("six canonical blocks remain outside the exact 3..=5 production boundary");
    assert_eq!(error, CYCLE_PATH_UNCERTIFIED_MESSAGE);
    assert_eq!(transactions.pending_token_for_test_v1(), None);
    let project =
        super::super::lock_project(&app_state).expect("project after six-block rejection");
    assert_eq!(project.editor.revision(), revision);
    assert!(project.editor.instruction_timeline().steps.is_empty());
}

#[test]
fn regular_quad_petal_authority_shape_rejects_wrong_counts_and_tree_substitution() {
    assert!(exact_three_graph_segment_shape_v1(3, 3));
    assert!(!exact_three_graph_segment_shape_v1(2, 2));
    assert!(!exact_three_graph_segment_shape_v1(4, 4));
    assert!(!exact_three_graph_segment_shape_v1(3, 2));
    assert!(!exact_three_graph_segment_shape_v1(3, 0));
}

// The production cancellation generation is intentionally process-wide.
// Serialize tests that advance it so parallel test scheduling cannot make
// an unrelated preview observe a foreign cancellation.
static STACKED_FOLD_READ_GENERATION_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub(crate) fn lock_stacked_fold_read_generation_test() -> std::sync::MutexGuard<'static, ()> {
    STACKED_FOLD_READ_GENERATION_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[test]
fn pair_cache_binding_is_strictly_positive_finite_only() {
    let project_instance_id = ProjectId::new();
    let project_id = ProjectId::new();
    let binding = |thickness| {
        positive_pair_proof_cache_binding_v1(
            project_instance_id,
            project_id,
            1,
            [0x91; 32],
            1,
            thickness,
        )
    };

    assert!(
        binding(f64::MIN_POSITIVE)
            .expect("positive binding")
            .is_some()
    );
    assert_eq!(binding(0.0).expect("positive-zero fallback"), None);
    assert_eq!(binding(-0.0).expect("negative-zero fallback"), None);
    assert_eq!(
        binding(-f64::MIN_POSITIVE).expect("negative fallback"),
        None
    );
    assert_eq!(binding(f64::NAN).expect("NaN fallback"), None);
    assert_eq!(binding(f64::INFINITY).expect("infinity fallback"), None);
    assert_eq!(
        binding(f64::NEG_INFINITY).expect("negative-infinity fallback"),
        None
    );
}

#[test]
fn production_pair_cache_control_observes_cancel_and_accepts_fresh_retry() {
    let _generation_guard = lock_stacked_fold_read_generation_test();
    let first = begin_stacked_fold_read_generation_v1().expect("first generation");
    let first_control =
        stacked_fold_pair_cache_control_v1(first, Instant::now() + Duration::from_secs(30));
    assert_eq!(first_control.check_v1(), Ok(()));

    cancel_current_stacked_fold_read_inner_v1().expect("cancel first generation");
    assert_eq!(
        first_control.check_v1(),
        Err(ori_collision::ProofCacheErrorV1::Cancelled)
    );

    let retry = begin_stacked_fold_read_generation_v1().expect("retry generation");
    assert_ne!(retry, first);
    let retry_control =
        stacked_fold_pair_cache_control_v1(retry, Instant::now() + Duration::from_secs(30));
    assert_eq!(retry_control.check_v1(), Ok(()));
}

#[test]
fn request_scoped_cancel_only_advances_the_exact_active_scope() {
    let _generation_guard = lock_stacked_fold_read_generation_test();
    let request_id = "stacked-fold:scope-a".to_owned();
    let scoped = begin_stacked_fold_read_generation_for_request_v1(Some(request_id.clone()))
        .expect("scoped generation");

    cancel_current_stacked_fold_read_inner_v1().expect("legacy cancel is a scoped no-op");
    cancel_current_stacked_fold_read_request_inner_v1("stacked-fold:stale".to_owned())
        .expect("stale scoped cancel is idempotent");
    assert_eq!(
        STACKED_FOLD_READ_GENERATION.load(Ordering::Acquire),
        scoped,
        "neither a late legacy cancel nor a stale scoped cancel may cancel the active scope",
    );

    assert_eq!(
        cancel_current_stacked_fold_read_request_inner_v1(" ".to_owned()),
        Err(INVALID_REQUEST_MESSAGE.to_owned()),
    );
    assert_eq!(
        STACKED_FOLD_READ_GENERATION.load(Ordering::Acquire),
        scoped,
        "malformed cancellation must not mutate the generation",
    );

    cancel_current_stacked_fold_read_request_inner_v1(request_id.clone())
        .expect("exact scoped cancel");
    let cancelled = STACKED_FOLD_READ_GENERATION.load(Ordering::Acquire);
    assert!(cancelled > scoped);
    cancel_current_stacked_fold_read_request_inner_v1(request_id)
        .expect("repeated scoped cancel is idempotent");
    assert_eq!(
        STACKED_FOLD_READ_GENERATION.load(Ordering::Acquire),
        cancelled,
    );

    let legacy = begin_stacked_fold_read_generation_v1().expect("legacy generation");
    cancel_current_stacked_fold_read_request_inner_v1("stacked-fold:old".to_owned())
        .expect("scoped cancel cannot cancel a legacy generation");
    assert_eq!(STACKED_FOLD_READ_GENERATION.load(Ordering::Acquire), legacy,);
    cancel_current_stacked_fold_read_inner_v1().expect("exact legacy cancel");
    assert!(STACKED_FOLD_READ_GENERATION.load(Ordering::Acquire) > legacy);
}

#[test]
fn scoped_generation_lease_clears_only_its_own_live_request() {
    let _generation_guard = lock_stacked_fold_read_generation_test();
    let first = begin_stacked_fold_read_scope_v1(Some("stacked-fold:lease-a".to_owned()))
        .expect("first scoped lease");
    let first_generation = first.generation();
    let replacement = begin_stacked_fold_read_scope_v1(Some("stacked-fold:lease-b".to_owned()))
        .expect("replacement scoped lease");
    let replacement_generation = replacement.generation();
    assert!(replacement_generation > first_generation);

    drop(first);
    cancel_current_stacked_fold_read_inner_v1()
        .expect("legacy cancel cannot clear the replacement scoped lease");
    assert_eq!(
        STACKED_FOLD_READ_GENERATION.load(Ordering::Acquire),
        replacement_generation,
        "dropping an obsolete lease must not clear the replacement request ID",
    );
    cancel_current_stacked_fold_read_request_inner_v1("stacked-fold:lease-a".to_owned())
        .expect("obsolete scoped cancel remains a no-op");
    assert_eq!(
        STACKED_FOLD_READ_GENERATION.load(Ordering::Acquire),
        replacement_generation,
    );
    drop(replacement);

    let completed_generation = STACKED_FOLD_READ_GENERATION.load(Ordering::Acquire);
    cancel_current_stacked_fold_read_inner_v1()
        .expect("legacy cancellation is available after natural scope completion");
    assert!(
        STACKED_FOLD_READ_GENERATION.load(Ordering::Acquire) > completed_generation,
        "natural completion must release scoped cancellation ownership",
    );
}

#[test]
fn scoped_pre_cancel_is_bounded_and_consumed_before_registration() {
    let _generation_guard = lock_stacked_fold_read_generation_test();
    let request_id = "stacked-fold:pre-cancelled".to_owned();
    let before = STACKED_FOLD_READ_GENERATION.load(Ordering::Acquire);
    cancel_current_stacked_fold_read_request_inner_v1(request_id.clone())
        .expect("pre-registration cancellation");
    assert_eq!(
        STACKED_FOLD_READ_GENERATION.load(Ordering::Acquire),
        before,
        "pre-cancelling a request with no active owner must not cancel another generation",
    );
    assert_eq!(
        begin_stacked_fold_read_scope_v1(Some(request_id.clone()))
            .expect_err("the first matching registration must consume its pre-cancel"),
        CANCELLED_MESSAGE,
    );
    assert_eq!(
        STACKED_FOLD_READ_GENERATION.load(Ordering::Acquire),
        before,
        "consuming a pre-cancel must not create a generation",
    );
    let retry = begin_stacked_fold_read_scope_v1(Some(request_id))
        .expect("a consumed pre-cancel is one-shot");
    assert!(retry.generation() > before);
    drop(retry);

    let mut publication = StackedFoldReadPublicationStateV1 {
        active_request_id: None,
        pre_cancelled_request_ids: VecDeque::new(),
    };
    for index in 0..=MAX_PRE_CANCELLED_STACKED_FOLD_READ_REQUESTS_V1 {
        remember_pre_cancelled_request_id_v1(
            &mut publication,
            format!("stacked-fold:bounded-pre-cancel:{index}"),
        );
    }
    assert_eq!(
        publication.pre_cancelled_request_ids.len(),
        MAX_PRE_CANCELLED_STACKED_FOLD_READ_REQUESTS_V1,
    );
    assert_eq!(
        publication
            .pre_cancelled_request_ids
            .front()
            .map(String::as_str),
        Some("stacked-fold:bounded-pre-cancel:1"),
        "the bounded ledger must evict only its oldest entry",
    );
}

#[test]
fn scoped_worker_waiter_yields_to_the_exact_replacement_generation() {
    let _generation_guard = lock_stacked_fold_read_generation_test();
    let state = AppState::new(two_hinge_tree_project(0.0));
    let held = state
        .try_acquire_native_pose_worker()
        .expect("simulated obsolete worker owns the permit");

    let second_id = "stacked-fold:waiter-b".to_owned();
    let second_scope =
        begin_stacked_fold_read_scope_v1(Some(second_id.clone())).expect("second request scope");
    let second_generation = second_scope.generation();
    let second_gate = state.1.clone();
    let second_waiter = tauri::async_runtime::spawn(async move {
        second_gate
            .acquire_notified_while(move || {
                STACKED_FOLD_READ_GENERATION.load(Ordering::Acquire) == second_generation
            })
            .await
    });
    wait_for_native_pose_worker_waiters_v1(&state, 1);

    cancel_current_stacked_fold_read_request_inner_v1(second_id)
        .expect("the exact second request is cancelled while waiting");
    let third_scope = begin_stacked_fold_read_scope_v1(Some("stacked-fold:waiter-c".to_owned()))
        .expect("third replacement scope");
    let third_generation = third_scope.generation();
    let third_gate = state.1.clone();
    let third_waiter = tauri::async_runtime::spawn(async move {
        third_gate
            .acquire_notified_while(move || {
                STACKED_FOLD_READ_GENERATION.load(Ordering::Acquire) == third_generation
            })
            .await
    });
    wait_for_native_pose_worker_waiters_v1(&state, 2);

    drop(held);
    let second_result =
        tauri::async_runtime::block_on(second_waiter).expect("second waiter task joins");
    assert!(
        second_result.is_none(),
        "a superseded waiter must not retain newly released worker capacity",
    );
    let third_permit = tauri::async_runtime::block_on(third_waiter)
        .expect("third waiter task joins")
        .expect("the exact replacement obtains worker capacity");
    assert!(state.native_pose_worker_is_busy());
    drop(third_permit);
    assert!(!state.native_pose_worker_is_busy());
    drop(second_scope);
    drop(third_scope);
}

fn wait_for_native_pose_worker_waiters_v1(state: &AppState, expected: usize) {
    for _ in 0..100_000 {
        if state.1.waiting_count() == expected {
            return;
        }
        std::thread::yield_now();
    }
    panic!(
        "native pose worker waiter count did not reach {expected}; observed {}",
        state.1.waiting_count(),
    );
}

fn fixed_id<T: serde::de::DeserializeOwned>(group: &str, index: u64) -> T {
    serde_json::from_str(&format!("\"00000000-0000-4000-{group}-{index:012x}\"")).unwrap()
}

fn set_fixed_cycle_fixture_identity_v1(
    project: &mut super::super::ProjectState,
    fixture_family: u8,
    fixture_case: u16,
) {
    let base = u64::from(fixture_family) * 0x1_0000 + u64::from(fixture_case) * 2;
    project.instance_id = fixed_id("a500", base);
    project.project_id = fixed_id("a500", base + 1);
}

fn automatic_opposite_pairs(
    project: &super::super::ProjectState,
    snapshot: &ori_topology::TopologySnapshot,
) -> Vec<[ori_domain::EdgeId; 2]> {
    let geometry = ori_kinematics::MaterialHingeGraphGeometry::prepare(
        project.editor.pattern(),
        project.editor.paper(),
        snapshot,
        ori_kinematics::TreeKinematicsLimits::default(),
    )
    .unwrap();
    let audit = ori_kinematics::MaterialHingeGraphAudit::prepare(
        snapshot,
        ori_kinematics::TreeKinematicsLimits::default(),
    )
    .unwrap();
    let count = geometry.hinges().len();
    ori_kinematics::enumerate_even_single_vertex_opposite_pairs_v1(
        &geometry,
        &audit,
        count * (count - 1) / 2,
    )
    .unwrap()
}

fn uncertified_rational_kawasaki_project(
    numerator: f64,
    denominator: f64,
    complement: f64,
) -> (super::super::ProjectState, Vec<ori_domain::EdgeId>) {
    use ori_domain::{CreasePattern, Edge, EdgeKind, Paper, Point2, Vertex};
    let ratio = numerator / denominator;
    let sine = complement / denominator;
    let points = [
        (1.0, 0.0),
        (-ratio, sine),
        (2.0 * ratio * ratio - 1.0, -2.0 * ratio * sine),
        (ratio, -sine),
        (0.0, 0.0),
    ];
    let vertices = points
        .into_iter()
        .map(|(x, y)| Vertex {
            id: ori_domain::VertexId::new(),
            position: Point2::new(x * 100.0, y * 100.0),
        })
        .collect::<Vec<_>>();
    let boundary = vertices[..4]
        .iter()
        .map(|vertex| vertex.id)
        .collect::<Vec<_>>();
    let center = vertices[4].id;
    let mut edges = (0..4)
        .map(|index| Edge {
            id: ori_domain::EdgeId::new(),
            start: boundary[index],
            end: boundary[(index + 1) % 4],
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    let hinges = (0..4)
        .map(|_| ori_domain::EdgeId::new())
        .collect::<Vec<_>>();
    edges.extend((0..4).map(|index| Edge {
        id: hinges[index],
        start: boundary[index],
        end: center,
        kind: if index == 3 {
            EdgeKind::Mountain
        } else {
            EdgeKind::Valley
        },
    }));
    (
        super::super::ProjectState::new_with_paper(
            CreasePattern { vertices, edges },
            Paper {
                boundary_vertices: boundary,
                ..Paper::default()
            },
        ),
        hinges,
    )
}

fn two_hinge_tree_project(paper_thickness_mm: f64) -> super::super::ProjectState {
    use ori_domain::{CreasePattern, Edge, EdgeKind, Paper, Point2, Vertex};
    let points = [
        (0.0, 0.0),
        (33.0, 0.0),
        (66.0, 0.0),
        (100.0, 0.0),
        (100.0, 100.0),
        (66.0, 100.0),
        (33.0, 100.0),
        (0.0, 100.0),
    ];
    let vertices = points
        .into_iter()
        .enumerate()
        .map(|(index, (x, y))| Vertex {
            id: fixed_id("7100", index as u64 + 1),
            position: Point2::new(x, y),
        })
        .collect::<Vec<_>>();
    let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
    let mut edges = (0..boundary.len())
        .map(|index| Edge {
            id: fixed_id("7200", index as u64 + 1),
            start: boundary[index],
            end: boundary[(index + 1) % boundary.len()],
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    edges.extend([
        Edge {
            id: fixed_id("7200", 20),
            start: boundary[1],
            end: boundary[6],
            kind: EdgeKind::Mountain,
        },
        Edge {
            id: fixed_id("7200", 21),
            start: boundary[2],
            end: boundary[5],
            kind: EdgeKind::Valley,
        },
    ]);
    let mut project = super::super::ProjectState::new_with_paper(
        CreasePattern { vertices, edges },
        Paper {
            boundary_vertices: boundary,
            thickness_mm: paper_thickness_mm,
            ..Paper::default()
        },
    );
    project.instance_id = fixed_id("7300", 1);
    project.project_id = fixed_id("7300", 2);
    project
}

fn four_hinge_tree_project() -> super::super::ProjectState {
    use ori_domain::{CreasePattern, Edge, EdgeKind, Paper, Point2, Vertex};
    let points = [
        (0.0, 0.0),
        (300.0, 0.0),
        (520.0, 120.0),
        (620.0, 350.0),
        (480.0, 580.0),
        (200.0, 650.0),
        (0.0, 320.0),
    ];
    let vertices = points
        .into_iter()
        .enumerate()
        .map(|(index, (x, y))| Vertex {
            id: fixed_id("7400", index as u64 + 1),
            position: Point2::new(x, y),
        })
        .collect::<Vec<_>>();
    let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
    let mut edges = (0..boundary.len())
        .map(|index| Edge {
            id: fixed_id("7500", index as u64 + 1),
            start: boundary[index],
            end: boundary[(index + 1) % boundary.len()],
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    for (index, end) in [2, 3, 4, 5].into_iter().enumerate() {
        edges.push(Edge {
            id: fixed_id("7500", index as u64 + 20),
            start: boundary[0],
            end: boundary[end],
            kind: if index % 2 == 0 {
                EdgeKind::Mountain
            } else {
                EdgeKind::Valley
            },
        });
    }
    super::super::ProjectState::new_with_paper(
        CreasePattern { vertices, edges },
        Paper {
            boundary_vertices: boundary,
            ..Paper::default()
        },
    )
}

fn five_hinge_tree_project() -> super::super::ProjectState {
    positive_tree_project(5)
}

fn six_hinge_tree_project() -> super::super::ProjectState {
    positive_tree_project(6)
}

fn seven_hinge_tree_project() -> super::super::ProjectState {
    positive_tree_project(7)
}

fn eight_hinge_tree_project() -> super::super::ProjectState {
    positive_tree_project(8)
}

fn positive_tree_project(hinge_count: usize) -> super::super::ProjectState {
    use ori_domain::{CreasePattern, Edge, EdgeKind, Paper, Point2, Vertex};
    let points: Vec<(f64, f64)> = match hinge_count {
        5 => vec![
            (0.0, 0.0),
            (300.0, 0.0),
            (520.0, 90.0),
            (680.0, 280.0),
            (650.0, 500.0),
            (450.0, 680.0),
            (180.0, 700.0),
            (0.0, 340.0),
        ],
        6 => vec![
            (0.0, 0.0),
            (300.0, 0.0),
            (530.0, 70.0),
            (700.0, 220.0),
            (760.0, 430.0),
            (620.0, 640.0),
            (380.0, 760.0),
            (140.0, 720.0),
            (0.0, 360.0),
        ],
        7 => vec![
            (0.0, 0.0),
            (300.0, 0.0),
            (540.0, 60.0),
            (730.0, 190.0),
            (840.0, 380.0),
            (810.0, 580.0),
            (650.0, 760.0),
            (410.0, 850.0),
            (150.0, 780.0),
            (0.0, 390.0),
        ],
        8 => {
            let radius = 41_i64.pow(9);
            let mut directions = Vec::with_capacity(11);
            directions.push((0.0, 0.0));
            let (mut real, mut imaginary) = (1_i64, 0_i64);
            for power in 0..=9_u32 {
                let scale = 41_i64.pow(9 - power);
                directions.push(((real * scale) as f64, (imaginary * scale) as f64));
                (real, imaginary) = (real * 40 - imaginary * 9, real * 9 + imaginary * 40);
            }
            debug_assert_eq!(directions[1], (radius as f64, 0.0));
            directions
        }
        _ => unreachable!("positive Tree fixture only covers 5..=8 hinges"),
    };
    let vertices = points
        .iter()
        .enumerate()
        .map(|(index, &(x, y))| Vertex {
            id: fixed_id("7600", index as u64 + 1),
            position: Point2::new(x, y),
        })
        .collect::<Vec<_>>();
    let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
    let mut edges = (0..boundary.len())
        .map(|index| Edge {
            id: fixed_id("7700", index as u64 + 1),
            start: boundary[index],
            end: boundary[(index + 1) % boundary.len()],
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    for (index, end) in (2..=hinge_count + 1).enumerate() {
        edges.push(Edge {
            id: fixed_id("7700", index as u64 + 20),
            start: boundary[0],
            end: boundary[end],
            kind: if index % 2 == 0 {
                EdgeKind::Mountain
            } else {
                EdgeKind::Valley
            },
        });
    }
    super::super::ProjectState::new_with_paper(
        CreasePattern { vertices, edges },
        Paper {
            boundary_vertices: boundary,
            ..Paper::default()
        },
    )
}
