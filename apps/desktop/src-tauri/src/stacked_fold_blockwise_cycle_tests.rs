use super::tests::lock_stacked_fold_read_generation_test;
use super::*;

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
