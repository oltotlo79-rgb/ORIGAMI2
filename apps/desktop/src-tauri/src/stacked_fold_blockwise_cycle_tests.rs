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
fn three_block_transport_preflight_enforces_the_aggregate_and_every_exact_limit() {
    use ori_collision::GeneralCellTransportLimitsV1;

    use super::stacked_fold_blockwise_cycle::{
        ThreeBlockCellTransportWorkV1, preflight_three_block_transport_aggregate_v1,
    };

    let per_proof = ThreeBlockCellTransportWorkV1 {
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
        .expect("four-proof aggregate");
    let exact = GeneralCellTransportLimitsV1 {
        max_transitions: aggregate.transitions,
        max_cells: aggregate.cells,
        max_layer_records: aggregate.layer_records,
        max_boundary_samples: aggregate.boundary_samples,
    };
    assert_eq!(
        preflight_three_block_transport_aggregate_v1(per_proof, exact),
        Ok(()),
        "each individual proof fits the aggregate limits"
    );
    assert_eq!(
        preflight_three_block_transport_aggregate_v1(aggregate, exact),
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
            preflight_three_block_transport_aggregate_v1(aggregate, one_short),
            Err(CYCLE_PATH_RESOURCE_MESSAGE.to_owned())
        );
    }
}

#[test]
fn three_block_transport_aggregate_addition_rejects_every_overflow() {
    use super::stacked_fold_blockwise_cycle::ThreeBlockCellTransportWorkV1;

    let one = ThreeBlockCellTransportWorkV1 {
        transitions: 1,
        cells: 1,
        layer_records: 1,
        boundary_samples: 1,
        folded_faces: 1,
        maximum_boundary_points: 1,
    };
    for near_overflow in [
        ThreeBlockCellTransportWorkV1 {
            transitions: usize::MAX,
            ..Default::default()
        },
        ThreeBlockCellTransportWorkV1 {
            cells: usize::MAX,
            ..Default::default()
        },
        ThreeBlockCellTransportWorkV1 {
            layer_records: usize::MAX,
            ..Default::default()
        },
        ThreeBlockCellTransportWorkV1 {
            boundary_samples: usize::MAX,
            ..Default::default()
        },
        ThreeBlockCellTransportWorkV1 {
            folded_faces: usize::MAX,
            ..Default::default()
        },
    ] {
        assert_eq!(near_overflow.checked_add_v1(one), None);
    }
}

#[test]
fn three_block_layer_peak_preflight_accepts_exact_rejects_one_short_and_overflow() {
    use super::stacked_fold_blockwise_cycle::{
        checked_three_block_layer_peak_retained_bytes_v1,
        checked_three_block_operation_peak_retained_bytes_v1,
        preflight_three_block_layer_peak_retained_bytes_v1,
    };

    let layer_peak = checked_three_block_layer_peak_retained_bytes_v1(10, 7)
        .expect("three whole-source and two restricted-source copies");
    assert_eq!(layer_peak, 44);
    let peak = checked_three_block_operation_peak_retained_bytes_v1(layer_peak, 5, 7)
        .expect("proof-retained and streaming workspace bytes");
    assert_eq!(peak, 56);
    assert_eq!(
        preflight_three_block_layer_peak_retained_bytes_v1(peak, peak),
        Ok(())
    );
    assert_eq!(
        preflight_three_block_layer_peak_retained_bytes_v1(peak, peak - 1),
        Err(CYCLE_PATH_RESOURCE_MESSAGE.to_owned())
    );
    assert_eq!(
        checked_three_block_layer_peak_retained_bytes_v1(usize::MAX, 0),
        None
    );
    assert_eq!(
        checked_three_block_layer_peak_retained_bytes_v1(0, usize::MAX),
        None
    );
    assert_eq!(
        checked_three_block_operation_peak_retained_bytes_v1(usize::MAX, 1, 0),
        None
    );
    assert_eq!(
        checked_three_block_operation_peak_retained_bytes_v1(0, usize::MAX, 1),
        None
    );
}

#[test]
fn three_block_layer_peak_rejection_happens_before_any_source_clone() {
    use ori_foldability::{
        GLOBAL_FLAT_FOLDABILITY_MODEL_ID, GlobalFlatFoldabilityProvenance, LAYER_ORDER_MODEL_ID,
        LayerFace, LayerOrderDerivation, LayerOrderProvenance, LayerOrderSnapshot,
    };
    use ori_topology::FaceKey;

    use super::stacked_fold_blockwise_cycle::{
        ThreeBlockLayerRetainedBytesV1, materialize_three_block_layer_sources_v1,
        reset_three_block_layer_source_clone_attempts_for_test_v1,
        three_block_layer_source_clone_attempts_for_test_v1,
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
    let no_faces: &[FaceId] = &[];
    let face_sets = [no_faces; 3];
    let plan = ThreeBlockLayerRetainedBytesV1::for_source_v1(&source, face_sets, 5, 7)
        .expect("checked proof-retained and temporary peak");
    reset_three_block_layer_source_clone_attempts_for_test_v1();
    assert_eq!(
        materialize_three_block_layer_sources_v1(&source, face_sets, 5, 7, plan.peak - 1),
        Err(CYCLE_PATH_RESOURCE_MESSAGE.to_owned())
    );
    assert_eq!(
        three_block_layer_source_clone_attempts_for_test_v1(),
        0,
        "peak amplification must be rejected before the whole or restricted source is cloned"
    );
    assert_eq!(
        materialize_three_block_layer_sources_v1(&source, face_sets, usize::MAX, 1, usize::MAX,),
        Err(CYCLE_PATH_RESOURCE_MESSAGE.to_owned())
    );
    assert_eq!(three_block_layer_source_clone_attempts_for_test_v1(), 0);
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
