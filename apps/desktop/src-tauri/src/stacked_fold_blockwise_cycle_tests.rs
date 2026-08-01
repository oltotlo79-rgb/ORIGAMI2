use super::tests::lock_stacked_fold_read_generation_test;
use super::*;

#[test]
fn blockwise_control_gate_rejects_stops_and_accepts_only_current_generation() {
    use std::{
        sync::atomic::{AtomicBool, Ordering},
        time::{Duration, Instant},
    };

    use ori_collision::CooperativeOperationControlV1;

    let _serial = lock_stacked_fold_read_generation_test();
    let original = STACKED_FOLD_READ_GENERATION.load(Ordering::Acquire);
    STACKED_FOLD_READ_GENERATION.store(811, Ordering::Release);
    let active = AtomicBool::new(false);
    let deadline = CooperativeOperationControlV1::new(Some(&active), Instant::now());
    assert_eq!(
        super::stacked_fold_blockwise_cycle::blockwise_cycle_control_gate_v1(811, &deadline),
        Err(
            super::stacked_fold_blockwise_cycle::BlockwiseCycleControlGateErrorV1::DeadlineExceeded
        )
    );
    let cancelled = AtomicBool::new(true);
    let cancelled_control = CooperativeOperationControlV1::new(
        Some(&cancelled),
        Instant::now() + Duration::from_secs(1),
    );
    assert_eq!(
        super::stacked_fold_blockwise_cycle::blockwise_cycle_control_gate_v1(
            811,
            &cancelled_control,
        ),
        Err(super::stacked_fold_blockwise_cycle::BlockwiseCycleControlGateErrorV1::Cancelled)
    );
    let old = CooperativeOperationControlV1::new_with_generation(
        Some(&active),
        &STACKED_FOLD_READ_GENERATION,
        811,
        Instant::now() + Duration::from_secs(1),
    );
    STACKED_FOLD_READ_GENERATION.store(812, Ordering::Release);
    assert_eq!(
        super::stacked_fold_blockwise_cycle::blockwise_cycle_control_gate_v1(811, &old),
        Err(super::stacked_fold_blockwise_cycle::BlockwiseCycleControlGateErrorV1::Cancelled)
    );
    let current = CooperativeOperationControlV1::new_with_generation(
        Some(&active),
        &STACKED_FOLD_READ_GENERATION,
        812,
        Instant::now() + Duration::from_secs(1),
    );
    assert_eq!(
        super::stacked_fold_blockwise_cycle::blockwise_cycle_control_gate_v1(812, &current),
        Ok(())
    );
    STACKED_FOLD_READ_GENERATION.store(original, Ordering::Release);
}

#[test]
fn bounded_multi_block_current_cycle_arity_matches_certified_boundary() {
    use super::stacked_fold_blockwise_cycle::bounded_multi_block_current_cycle_arity_supported_v1;

    for block_count in 3..=9 {
        assert!(
            bounded_multi_block_current_cycle_arity_supported_v1(block_count),
            "{block_count}-block current cycles stay inside the certified production boundary"
        );
    }
    for block_count in [0, 1, 2, 10, 11, usize::MAX] {
        assert!(
            !bounded_multi_block_current_cycle_arity_supported_v1(block_count),
            "{block_count}-block current cycles must fail closed outside the certified production boundary"
        );
    }
}

#[test]
fn bounded_multi_block_transport_preflight_accounts_for_whole_parent_and_all_blocks() {
    use ori_collision::GeneralCellTransportLimitsV1;

    use super::stacked_fold_blockwise_cycle::{
        BoundedMultiBlockCellTransportWorkV1, preflight_bounded_multi_block_transport_aggregate_v1,
    };

    let per_proof = BoundedMultiBlockCellTransportWorkV1 {
        transitions: 3,
        cells: 5,
        layer_records: 7,
        boundary_samples: 11,
        folded_faces: 13,
        maximum_boundary_points: 17,
    };
    let aggregate = per_proof
        .checked_add_v1(per_proof)
        .and_then(|work| work.checked_add_v1(per_proof))
        .and_then(|work| work.checked_add_v1(per_proof))
        .and_then(|work| work.checked_add_v1(per_proof))
        .and_then(|work| work.checked_add_v1(per_proof))
        .and_then(|work| work.checked_add_v1(per_proof))
        .and_then(|work| work.checked_add_v1(per_proof))
        .and_then(|work| work.checked_add_v1(per_proof))
        .and_then(|work| work.checked_add_v1(per_proof))
        .expect("whole-parent plus nine-block aggregate");
    assert_eq!(
        (
            aggregate.transitions,
            aggregate.cells,
            aggregate.layer_records,
            aggregate.boundary_samples,
        ),
        (30, 50, 70, 110),
    );
    let exact = GeneralCellTransportLimitsV1 {
        max_transitions: aggregate.transitions,
        max_cells: aggregate.cells,
        max_layer_records: aggregate.layer_records,
        max_boundary_samples: aggregate.boundary_samples,
    };
    assert_eq!(
        preflight_bounded_multi_block_transport_aggregate_v1(per_proof, exact),
        Ok(()),
        "each individual proof fits the aggregate limits"
    );
    assert_eq!(
        preflight_bounded_multi_block_transport_aggregate_v1(aggregate, exact),
        Ok(()),
        "the exact whole-operation aggregate is accepted"
    );

    for one_short in [
        GeneralCellTransportLimitsV1 {
            max_transitions: exact.max_transitions - 1,
            ..exact
        },
        GeneralCellTransportLimitsV1 {
            max_cells: exact.max_cells - 1,
            ..exact
        },
        GeneralCellTransportLimitsV1 {
            max_layer_records: exact.max_layer_records - 1,
            ..exact
        },
        GeneralCellTransportLimitsV1 {
            max_boundary_samples: exact.max_boundary_samples - 1,
            ..exact
        },
    ] {
        assert_eq!(
            preflight_bounded_multi_block_transport_aggregate_v1(aggregate, one_short),
            Err(CYCLE_PATH_RESOURCE_MESSAGE.to_owned())
        );
    }
}

#[test]
fn bounded_multi_block_transport_aggregate_addition_rejects_every_overflow() {
    use super::stacked_fold_blockwise_cycle::BoundedMultiBlockCellTransportWorkV1;

    let one = BoundedMultiBlockCellTransportWorkV1 {
        transitions: 1,
        cells: 1,
        layer_records: 1,
        boundary_samples: 1,
        folded_faces: 1,
        maximum_boundary_points: 1,
    };
    for near_overflow in [
        BoundedMultiBlockCellTransportWorkV1 {
            transitions: usize::MAX,
            ..Default::default()
        },
        BoundedMultiBlockCellTransportWorkV1 {
            cells: usize::MAX,
            ..Default::default()
        },
        BoundedMultiBlockCellTransportWorkV1 {
            layer_records: usize::MAX,
            ..Default::default()
        },
        BoundedMultiBlockCellTransportWorkV1 {
            boundary_samples: usize::MAX,
            ..Default::default()
        },
        BoundedMultiBlockCellTransportWorkV1 {
            folded_faces: usize::MAX,
            ..Default::default()
        },
    ] {
        assert_eq!(near_overflow.checked_add_v1(one), None);
    }
}

#[test]
fn bounded_multi_block_layer_peak_preflight_is_arity_independent_and_checked() {
    use super::stacked_fold_blockwise_cycle::{
        checked_bounded_multi_block_layer_peak_retained_bytes_v1,
        checked_bounded_multi_block_operation_peak_retained_bytes_v1,
        preflight_bounded_multi_block_layer_peak_retained_bytes_v1,
    };

    let exact_three_restricted_sum = 2 + 2 + 3;
    let exact_four_restricted_sum = 1 + 2 + 2 + 2;
    assert_eq!(exact_three_restricted_sum, exact_four_restricted_sum);
    let three_block_layer_peak =
        checked_bounded_multi_block_layer_peak_retained_bytes_v1(10, exact_three_restricted_sum)
            .expect("three whole-source and two restricted-source copies");
    let four_block_layer_peak =
        checked_bounded_multi_block_layer_peak_retained_bytes_v1(10, exact_four_restricted_sum)
            .expect("multiplicity is independent of the number of restricted sources");
    assert_eq!(three_block_layer_peak, 44);
    assert_eq!(four_block_layer_peak, three_block_layer_peak);
    let peak =
        checked_bounded_multi_block_operation_peak_retained_bytes_v1(four_block_layer_peak, 5, 7)
            .expect("proof-retained and streaming workspace bytes");
    assert_eq!(peak, 56);
    assert_eq!(
        preflight_bounded_multi_block_layer_peak_retained_bytes_v1(peak, peak),
        Ok(())
    );
    assert_eq!(
        preflight_bounded_multi_block_layer_peak_retained_bytes_v1(peak, peak - 1),
        Err(CYCLE_PATH_RESOURCE_MESSAGE.to_owned())
    );
    assert_eq!(
        checked_bounded_multi_block_layer_peak_retained_bytes_v1(usize::MAX, 0),
        None
    );
    assert_eq!(
        checked_bounded_multi_block_layer_peak_retained_bytes_v1(0, usize::MAX),
        None
    );
    assert_eq!(
        checked_bounded_multi_block_operation_peak_retained_bytes_v1(usize::MAX, 1, 0),
        None
    );
    assert_eq!(
        checked_bounded_multi_block_operation_peak_retained_bytes_v1(0, usize::MAX, 1),
        None
    );
}

#[test]
fn bounded_multi_block_layer_peak_materializes_only_after_exact_preflight() {
    use ori_foldability::{
        GLOBAL_FLAT_FOLDABILITY_MODEL_ID, GlobalFlatFoldabilityProvenance, LAYER_ORDER_MODEL_ID,
        LayerFace, LayerOrderDerivation, LayerOrderProvenance, LayerOrderSnapshot,
    };
    use ori_topology::FaceKey;

    use super::stacked_fold_blockwise_cycle::{
        BoundedMultiBlockLayerRetainedBytesV1,
        bounded_multi_block_layer_source_clone_attempts_for_test_v1,
        checked_bounded_multi_block_layer_peak_retained_bytes_v1,
        materialize_bounded_multi_block_layer_sources_v1,
        reset_bounded_multi_block_layer_source_clone_attempts_for_test_v1,
    };

    let _serial = lock_stacked_fold_read_generation_test();
    let face = LayerFace {
        face_id: FaceId::new(),
        face_key: FaceKey([0; 32]),
    };
    let source = LayerOrderSnapshot {
        model_id: LAYER_ORDER_MODEL_ID,
        material_faces: vec![face],
        global_bottom_to_top: Some(vec![face]),
        provenance: LayerOrderProvenance {
            source: GlobalFlatFoldabilityProvenance {
                identity_namespace: None,
                source_revision: 0,
                source_fingerprint: None,
                model_id: GLOBAL_FLAT_FOLDABILITY_MODEL_ID,
            },
            derivation: LayerOrderDerivation::SingleFace { face },
        },
        reference_face: Some(face),
        folded_faces: Vec::new(),
        overlap_cells: Vec::new(),
        face_pair_orders: Vec::new(),
        proof_summary: None,
    };
    let selected_face_ids = [face.face_id];
    let selected_faces: &[FaceId] = &selected_face_ids;
    let three_face_sets = [selected_faces; 3];
    let four_face_sets = [selected_faces; 4];
    let five_face_sets = [selected_faces; 5];
    let six_face_sets = [selected_faces; 6];
    let seven_face_sets = [selected_faces; 7];
    let eight_face_sets = [selected_faces; 8];
    let nine_face_sets = [selected_faces; 9];
    let three_plan =
        BoundedMultiBlockLayerRetainedBytesV1::for_source_v1(&source, &three_face_sets, 5, 7)
            .expect("checked proof-retained and temporary peak");
    let four_plan =
        BoundedMultiBlockLayerRetainedBytesV1::for_source_v1(&source, &four_face_sets, 5, 7)
            .expect("four-block retained-byte plan");
    let five_plan =
        BoundedMultiBlockLayerRetainedBytesV1::for_source_v1(&source, &five_face_sets, 5, 7)
            .expect("five-block retained-byte plan");
    let six_plan =
        BoundedMultiBlockLayerRetainedBytesV1::for_source_v1(&source, &six_face_sets, 5, 7)
            .expect("six-block retained-byte plan");
    let seven_plan =
        BoundedMultiBlockLayerRetainedBytesV1::for_source_v1(&source, &seven_face_sets, 5, 7)
            .expect("seven-block retained-byte plan");
    let eight_plan =
        BoundedMultiBlockLayerRetainedBytesV1::for_source_v1(&source, &eight_face_sets, 5, 7)
            .expect("eight-block retained-byte plan");
    let nine_plan =
        BoundedMultiBlockLayerRetainedBytesV1::for_source_v1(&source, &nine_face_sets, 5, 7)
            .expect("nine-block retained-byte plan");
    assert_eq!(three_plan.block_sources.len(), 3);
    assert_eq!(four_plan.block_sources.len(), 4);
    assert_eq!(five_plan.block_sources.len(), 5);
    assert_eq!(six_plan.block_sources.len(), 6);
    assert_eq!(seven_plan.block_sources.len(), 7);
    assert_eq!(eight_plan.block_sources.len(), 8);
    assert_eq!(nine_plan.block_sources.len(), 9);
    for plan in [
        &three_plan,
        &four_plan,
        &five_plan,
        &six_plan,
        &seven_plan,
        &eight_plan,
        &nine_plan,
    ] {
        let restricted_sum = plan
            .block_sources
            .iter()
            .try_fold(0usize, |sum, retained| sum.checked_add(*retained))
            .expect("bounded restricted-source sum");
        let source_peak = checked_bounded_multi_block_layer_peak_retained_bytes_v1(
            plan.whole_source,
            restricted_sum,
        )
        .expect("bounded source-retention peak");
        assert_eq!(
            plan.peak,
            source_peak + plan.proof_retained + plan.peak_temporary
        );
    }
    for unsupported_face_sets in [&[selected_faces; 2][..], &[selected_faces; 10][..]] {
        assert_eq!(
            BoundedMultiBlockLayerRetainedBytesV1::for_source_v1(
                &source,
                unsupported_face_sets,
                5,
                7,
            ),
            Err(CYCLE_PATH_UNCERTIFIED_MESSAGE.to_owned())
        );
    }
    reset_bounded_multi_block_layer_source_clone_attempts_for_test_v1();
    assert_eq!(
        materialize_bounded_multi_block_layer_sources_v1(
            &source,
            &nine_face_sets,
            5,
            7,
            nine_plan.peak - 1,
        ),
        Err(CYCLE_PATH_RESOURCE_MESSAGE.to_owned())
    );
    assert_eq!(
        bounded_multi_block_layer_source_clone_attempts_for_test_v1(),
        0,
        "peak amplification must be rejected before the whole or restricted source is cloned"
    );
    let (whole_source, block_sources, materialized_plan) =
        materialize_bounded_multi_block_layer_sources_v1(
            &source,
            &nine_face_sets,
            5,
            7,
            nine_plan.peak,
        )
        .expect("the exact nine-block retained-byte peak is accepted");
    assert_eq!(materialized_plan, nine_plan);
    assert_eq!(block_sources.len(), 9);
    assert_eq!(
        bounded_multi_block_layer_source_clone_attempts_for_test_v1(),
        10,
        "one whole source and all nine restricted sources are materialized"
    );
    assert!(
        whole_source
            .checked_deep_retained_bytes_v1()
            .is_some_and(|actual| actual <= materialized_plan.whole_source)
    );
    assert!(
        block_sources
            .iter()
            .zip(&materialized_plan.block_sources)
            .all(|(source, maximum)| source
                .checked_deep_retained_bytes_v1()
                .is_some_and(|actual| actual <= *maximum))
    );
    reset_bounded_multi_block_layer_source_clone_attempts_for_test_v1();
    assert_eq!(
        materialize_bounded_multi_block_layer_sources_v1(
            &source,
            &nine_face_sets,
            usize::MAX,
            1,
            usize::MAX,
        ),
        Err(CYCLE_PATH_RESOURCE_MESSAGE.to_owned())
    );
    assert_eq!(
        bounded_multi_block_layer_source_clone_attempts_for_test_v1(),
        0
    );
}

#[test]
fn seventeen_cell_current_cycle_uses_blockwise_fallback_end_to_end() {
    use ori_kinematics::{MaterialHingeGraphGeometry, TreeKinematicsLimits};
    use ori_topology::FaceExtractionInput;

    let _generation_guard = lock_stacked_fold_read_generation_test();
    let (blocks, (pattern, paper, moving)) =
        super::miura_cactus_test_support::independent_three_by_three_miura_blocks_with_document();
    let project_id = ProjectId::new();
    let block_faces = blocks.each_ref().map(|(pattern, paper, _)| {
        let topology = analyze_faces(FaceExtractionInput {
            identity_namespace: project_id,
            source_revision: 1,
            paper,
            pattern,
        })
        .snapshot
        .unwrap();
        MaterialHingeGraphGeometry::prepare(
            pattern,
            paper,
            &topology,
            TreeKinematicsLimits::default(),
        )
        .unwrap()
        .face_ids()
        .to_vec()
    });
    let articulation = *block_faces[0]
        .iter()
        .find(|face| block_faces[1].contains(face))
        .expect("shared articulation");
    let mut project = super::super::ProjectState::new_with_paper(pattern, paper);
    project.project_id = project_id;
    let topology = project
        .editor
        .topology_analysis_input(project.project_id)
        .analyze();
    let snapshot = topology.simulation_snapshot().unwrap();
    assert_eq!(snapshot.faces.len(), 17);
    let hinges = snapshot
        .hinge_adjacency
        .iter()
        .map(|hinge| hinge.edge)
        .collect::<Vec<_>>();
    super::super::applied_pose::tests::install_flat_graph_pose_authority_on_face(
        &mut project,
        hinges.clone(),
        articulation,
    );
    let layer_state = GlobalFlatFoldabilityState::default();
    super::super::global_flat_foldability::tests::install_possible_layer_order(
        &layer_state,
        &project,
    );
    let layer_capability =
        super::super::global_flat_foldability::capture_current_layer_order_capability(
            &layer_state,
            &project,
        )
        .unwrap()
        .expect("17-face layer capability");
    assert!(
        layer_capability
            .snapshot()
            .overlap_cells
            .iter()
            .any(|cell| {
                block_faces
                    .iter()
                    .filter(|faces| {
                        cell.covering_faces
                            .iter()
                            .all(|face| faces.contains(&face.face_id))
                            && cell
                                .bottom_to_top_faces
                                .iter()
                                .all(|face| faces.contains(face))
                    })
                    .count()
                    != 1
            })
    );
    let instance = project.instance_id;
    let revision = project.editor.revision();
    let state = AppState::new(project);
    let layer_state = GlobalFlatFoldabilityState::default();
    {
        let project = super::super::lock_project(&state).unwrap();
        super::super::global_flat_foldability::tests::install_possible_layer_order(
            &layer_state,
            &project,
        );
    }
    let tree_only_result = tauri::async_runtime::block_on(read_live_hinge_registry_inner(
        &state,
        &layer_state,
        LiveHingeRegistryRequestV1::for_test(
            instance,
            project_id,
            revision,
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            FixedSideRequest::Left,
            RotationDirectionRequest::Positive,
            1.0,
        ),
    ));
    assert_eq!(
        tree_only_result.expect_err("graph pose must fail closed"),
        ANALYSIS_FAILED_MESSAGE
    );
    let transactions =
        super::super::stacked_fold_transaction::StackedFoldTransactionState::default();
    let request = |expected_revision, schedule| CurrentCyclePosePreviewRequestV1 {
        progress_request_id: None,
        expected_project_instance_id: instance,
        expected_project_id: project_id,
        expected_revision,
        cycle_schedule_v1: schedule,
    };

    let active = moving
        .iter()
        .copied()
        .filter(|edge_id| {
            let project = super::super::lock_project(&state).unwrap();
            let edge = project
                .editor
                .pattern()
                .edges
                .iter()
                .find(|edge| edge.id == *edge_id)
                .unwrap();
            let y = project
                .editor
                .pattern()
                .vertices
                .iter()
                .find(|vertex| vertex.id == edge.start)
                .unwrap()
                .position
                .y;
            y == -20.0 || y == 20.0
        })
        .collect::<Vec<_>>();
    let schedule = dense_grid_schedule(&hinges, &active, 64);

    let mut duplicate_edge_schedule = schedule.clone();
    duplicate_edge_schedule.entries[1].edge = duplicate_edge_schedule.entries[0].edge;
    assert_eq!(
        propose_current_cycle_pose_inner_with_layers(
            None,
            &state,
            Some(&layer_state),
            &transactions,
            request(revision, duplicate_edge_schedule),
        )
        .unwrap_err(),
        CYCLE_PATH_UNSUPPORTED_MESSAGE
    );
    let mut noncanonical_schedule = schedule.clone();
    noncanonical_schedule.entries.swap(0, 1);
    assert_eq!(
        propose_current_cycle_pose_inner_with_layers(
            None,
            &state,
            Some(&layer_state),
            &transactions,
            request(revision, noncanonical_schedule),
        )
        .unwrap_err(),
        CYCLE_PATH_UNSUPPORTED_MESSAGE
    );
    let mut oversized_coefficients = schedule.clone();
    let coefficient = oversized_coefficients.entries[0].numerator_power_coefficients[0];
    oversized_coefficients.entries[0].numerator_power_coefficients =
        vec![coefficient; MAX_CYCLE_SCHEDULE_COEFFICIENTS_V1 + 2];
    assert_eq!(
        propose_current_cycle_pose_inner_with_layers(
            None,
            &state,
            Some(&layer_state),
            &transactions,
            request(revision, oversized_coefficients),
        )
        .unwrap_err(),
        CYCLE_PATH_RESOURCE_MESSAGE
    );
    let mut oversized_schedule = schedule.clone();
    while oversized_schedule.entries.len() <= MAX_STACKED_FOLD_REQUEST_HINGES_V1 {
        oversized_schedule
            .entries
            .push(oversized_schedule.entries[0].clone());
    }
    assert_eq!(
        propose_current_cycle_pose_inner_with_layers(
            None,
            &state,
            Some(&layer_state),
            &transactions,
            request(revision, oversized_schedule),
        )
        .unwrap_err(),
        CYCLE_PATH_UNSUPPORTED_MESSAGE
    );
    assert_eq!(transactions.pending_token_for_test_v1(), None);
    assert_eq!(
        super::super::lock_project(&state)
            .unwrap()
            .editor
            .revision(),
        revision
    );

    assert_eq!(
        propose_current_cycle_pose_inner(
            None,
            &state,
            &transactions,
            request(revision, schedule.clone()),
        )
        .unwrap_err(),
        CYCLE_PATH_UNCERTIFIED_MESSAGE
    );
    assert_eq!(transactions.pending_token_for_test_v1(), None);
    assert_eq!(
        super::super::lock_project(&state)
            .unwrap()
            .editor
            .revision(),
        revision
    );

    let mut malformed = schedule.clone();
    malformed.entries[0].denominator_power_coefficients[0].numerator = 0;
    assert_eq!(
        propose_current_cycle_pose_inner_with_layers(
            None,
            &state,
            Some(&layer_state),
            &transactions,
            request(revision, malformed),
        )
        .unwrap_err(),
        CYCLE_PATH_UNSUPPORTED_MESSAGE
    );
    assert_eq!(transactions.pending_token_for_test_v1(), None);
    assert_eq!(
        super::super::lock_project(&state)
            .unwrap()
            .editor
            .revision(),
        revision
    );

    assert_eq!(
        propose_current_cycle_pose_inner_with_layers(
            None,
            &state,
            Some(&layer_state),
            &transactions,
            request(revision, schedule.clone()),
        )
        .unwrap_err(),
        CYCLE_PATH_UNCERTIFIED_MESSAGE,
        "cross-block overlap evidence must fail closed before preview"
    );
    assert_eq!(transactions.pending_token_for_test_v1(), None);
    assert_eq!(
        super::super::lock_project(&state)
            .unwrap()
            .editor
            .revision(),
        revision
    );
    let unknown_block = serde_json::json!({
        "version": 1,
        "entries": [],
        "blockProofV1": { "forged": true }
    });
    assert!(serde_json::from_value::<CycleScheduleRequestV1>(unknown_block).is_err());
    let unknown_proof = serde_json::json!({
        "version": 1,
        "entries": [],
        "proof": "forged"
    });
    assert!(serde_json::from_value::<CycleScheduleRequestV1>(unknown_proof).is_err());
}
