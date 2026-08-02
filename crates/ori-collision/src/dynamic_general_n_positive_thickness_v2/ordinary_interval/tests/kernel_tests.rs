use super::super::*;
use super::binding_assertions::assert_ordinary_binding_covers_boundary_and_resource_fields_v2;
use super::negative::assert_overlapping_ordinary_pair_is_not_certified_v2;
use super::support::{
    N33, N34, bridge_revalidation_input_v2, input_v2, n33_fixture_v2, n34_fixture_v2,
    strict_limits_v2,
};
use super::workspace_tests::{assert_phase_accounting_v2, assert_schedule_checkpoint_contract_v2};

#[test]
fn n33_n34_nonzero_general_n_kernel_resources_binding_and_stops() {
    let n33 = n33_fixture_v2();
    assert_overlapping_ordinary_pair_is_not_certified_v2(n33);
    assert_eq!(n33.fixture.profile.actual_block_count_v2(), N33);
    let limits33 = strict_limits_v2(n33);
    let input33 = input_v2(n33, limits33);
    let mut count_cap = limits33;
    count_cap.max_faces -= 1;
    let mut count_cap_polls = 0usize;
    assert_eq!(
        prove_ordinary_interval_clearance_with_checkpoint_v2(input_v2(n33, count_cap), || {
            count_cap_polls += 1;
            Ok(())
        })
        .unwrap_err(),
        OrdinaryIntervalErrorV2::ResourceLimit
    );
    assert_eq!(count_cap_polls, 1, "count caps precede caller-sized scans");
    let assert_nonzero_schedule = |fixture: &super::support::OrdinaryFixtureV2| {
        let moving_edge = fixture
            .fixture
            .geometry
            .hinges()
            .iter()
            .find(|hinge| {
                fixture
                    .schedule
                    .derivative_bound(hinge.edge())
                    .is_some_and(|bound| bound > 0.0)
            })
            .expect("moving hinge")
            .edge();
        let endpoint = |parameter| {
            fixture
                .schedule
                .evaluate(parameter)
                .unwrap()
                .as_slice()
                .iter()
                .find(|angle| angle.edge() == moving_edge)
                .unwrap()
                .angle_degrees()
        };
        assert_ne!(endpoint(-1.0).to_bits(), endpoint(1.0).to_bits());
    };
    assert_nonzero_schedule(n33);
    let mut no_stop = || Ok(());
    let mut validated33 =
        resources::validate_input_v2(&input33, &mut no_stop).expect("N33 exact resource preflight");
    assert_phase_accounting_v2(&validated33);
    assert_schedule_checkpoint_contract_v2(n33);
    assert_eq!(
        limits33.max_bridge_retained_bytes,
        validated33.resources.charged_bridge_retained_bytes
    );
    assert_eq!(
        limits33.max_bridge_revalidation_peak_bytes,
        validated33.resources.charged_bridge_revalidation_peak_bytes
    );
    assert_eq!(
        limits33.max_schedule_retained_bytes,
        validated33.resources.charged_schedule_retained_bytes
    );
    assert_eq!(
        limits33.max_session_shell_bytes,
        validated33.resources.charged_session_shell_bytes
    );
    assert_eq!(
        limits33.max_bridge_partition_search_work_per_node,
        validated33.resources.charged_bridge_partition_search_work
            / validated33.resources.charged_interval_nodes
    );
    assert_eq!(
        limits33.max_interval_registry_workspace_bytes,
        validated33
            .resources
            .charged_interval_registry_workspace_bytes
    );
    assert_eq!(
        limits33.max_interval_registry_retained_bytes,
        validated33
            .resources
            .charged_interval_registry_retained_bytes
    );

    assert_eq!(
        validated33
            .interval_transform_session
            .prepare_leaf_with_checkpoint_v2(
                0,
                0,
                limits33.schedule_limits,
                validated33.schedule_workspace_bound,
                limits33.max_schedule_evaluation_workspace_bytes,
                limits33.max_bridge_partition_search_work_per_node,
                &validated33.interval_transform_workspace_bound,
                || Ok(()),
            )
            .unwrap_err(),
        ori_kinematics::CommonArticulationDynamicClosureIntervalTransformLeafErrorV2::Inconclusive
    );
    assert_eq!(
        validated33
            .interval_transform_session
            .prepare_leaf_with_checkpoint_v2(
                0,
                0,
                limits33.schedule_limits,
                validated33.schedule_workspace_bound,
                0,
                limits33.max_bridge_partition_search_work_per_node,
                &validated33.interval_transform_workspace_bound,
                || Ok(()),
            )
            .unwrap_err(),
        ori_kinematics::CommonArticulationDynamicClosureIntervalTransformLeafErrorV2::ResourceLimit
    );
    assert_eq!(
        validated33
            .interval_transform_session
            .prepare_leaf_with_checkpoint_v2(
                0,
                0,
                limits33.schedule_limits,
                validated33.schedule_workspace_bound,
                limits33.max_schedule_evaluation_workspace_bytes + 1,
                limits33.max_bridge_partition_search_work_per_node,
                &validated33.interval_transform_workspace_bound,
                || Ok(()),
            )
            .unwrap_err(),
        ori_kinematics::CommonArticulationDynamicClosureIntervalTransformLeafErrorV2::ResourceLimit
    );
    assert_eq!(
        validated33
            .interval_transform_session
            .prepare_leaf_with_checkpoint_v2(
                0,
                0,
                limits33.schedule_limits,
                validated33.schedule_workspace_bound,
                limits33.max_schedule_evaluation_workspace_bytes,
                limits33.max_bridge_partition_search_work_per_node - 1,
                &validated33.interval_transform_workspace_bound,
                || Ok(()),
            )
            .unwrap_err(),
        ori_kinematics::CommonArticulationDynamicClosureIntervalTransformLeafErrorV2::ResourceLimit
    );
    assert_eq!(
        validated33
            .interval_transform_session
            .prepare_leaf_with_checkpoint_v2(
                0,
                0,
                ori_kinematics::CycleScheduleLimitsV1 {
                    max_hinges: 0,
                    ..limits33.schedule_limits
                },
                validated33.schedule_workspace_bound,
                limits33.max_schedule_evaluation_workspace_bytes,
                limits33.max_bridge_partition_search_work_per_node,
                &validated33.interval_transform_workspace_bound,
                || Ok(()),
            )
            .unwrap_err(),
        ori_kinematics::CommonArticulationDynamicClosureIntervalTransformLeafErrorV2::InvalidInput
    );
    let leaf = (1..=limits33.max_collision_depth)
        .flat_map(|depth| (0..(1_u64 << depth)).map(move |index| (depth, index)))
        .find_map(|(depth, index)| {
            validated33
                .interval_transform_session
                .prepare_leaf_with_checkpoint_v2(
                    depth,
                    index,
                    limits33.schedule_limits,
                    validated33.schedule_workspace_bound,
                    limits33.max_schedule_evaluation_workspace_bytes,
                    limits33.max_bridge_partition_search_work_per_node,
                    &validated33.interval_transform_workspace_bound,
                    || Ok(()),
                )
                .ok()
        })
        .expect("at least one collision leaf refines a sealed bridge leaf");
    for debug in [
        format!("{:?}", validated33.interval_transform_session),
        format!("{leaf:?}"),
    ] {
        assert!(!debug.contains("poses"));
        assert!(!debug.contains("angle_boxes"));
        assert!(!debug.contains("input_binding"));
        assert!(!debug.contains("partition"));
        assert!(!debug.contains("bridge_binding"));
    }
    assert!(
        leaf.transform_for_canonical_face_position_v2(
            &n33.fixture.geometry,
            0,
            n33.fixture.geometry.face_ids()[0],
        )
        .is_some()
    );
    assert_eq!(
        validated33
            .interval_transform_session
            .prepare_leaf_with_checkpoint_v2(
                64,
                u64::MAX,
                limits33.schedule_limits,
                validated33.schedule_workspace_bound,
                1,
                0,
                &validated33.interval_transform_workspace_bound,
                || Err(ori_kinematics::CommonArticulationDynamicClosureBridgeStopV2::Cancelled),
            )
            .unwrap_err(),
        ori_kinematics::CommonArticulationDynamicClosureIntervalTransformLeafErrorV2::Cancelled
    );

    let mut successful_publication_polls = 0usize;
    let evidence33 = prove_ordinary_interval_clearance_with_checkpoint_v2(input33, || {
        successful_publication_polls += 1;
        Ok(())
    })
    .expect("N33 nonzero ordinary-pair interval clearance");
    let mut final_publication_polls = 0usize;
    assert_eq!(
        prove_ordinary_interval_clearance_with_checkpoint_v2(input33, || {
            final_publication_polls += 1;
            if final_publication_polls == successful_publication_polls {
                Err(OrdinaryIntervalStopV2::DeadlineExceeded)
            } else {
                Ok(())
            }
        })
        .unwrap_err(),
        OrdinaryIntervalErrorV2::DeadlineExceeded
    );
    assert!(evidence33.accepted_leaf_count > 0);
    assert!(evidence33.processed_interval_node_count >= evidence33.accepted_leaf_count);
    assert_eq!(
        evidence33.certified_ordinary_pair_leaf_count,
        evidence33
            .accepted_leaf_count
            .checked_mul(evidence33.resources.ordinary_face_pairs)
            .unwrap()
    );
    assert_eq!(evidence33.resources.face_count, 8 * N33 + 1);
    assert_eq!(evidence33.resources.hinge_count, 12 * N33);
    assert!(evidence33.resources.ordinary_face_pairs > 0);
    assert!(evidence33.certified_ordinary_pair_leaf_count > 0);
    assert_ordinary_binding_covers_boundary_and_resource_fields_v2(
        input33,
        &mut validated33,
        &evidence33,
    );

    let mut one_short_schedule = limits33;
    one_short_schedule.max_schedule_evaluation_workspace_bytes -= 1;
    assert_eq!(
        prove_ordinary_interval_clearance_v2(input_v2(n33, one_short_schedule)).unwrap_err(),
        OrdinaryIntervalErrorV2::ResourceLimit
    );
    let mut over_schedule_workspace = limits33;
    over_schedule_workspace.max_schedule_evaluation_workspace_bytes += 1;
    prove_ordinary_interval_clearance_v2(input_v2(n33, over_schedule_workspace))
        .expect("caller workspace policy is an upper bound, not an exact request");
    let mut one_short_builder = limits33;
    one_short_builder.max_interval_registry_workspace_bytes -= 1;
    assert_eq!(
        prove_ordinary_interval_clearance_v2(input_v2(n33, one_short_builder)).unwrap_err(),
        OrdinaryIntervalErrorV2::ResourceLimit
    );
    macro_rules! one_short_limit {
        ($field:ident) => {{
            let mut candidate = limits33;
            candidate.$field -= 1;
            assert_eq!(
                prove_ordinary_interval_clearance_v2(input_v2(n33, candidate)).unwrap_err(),
                OrdinaryIntervalErrorV2::ResourceLimit,
                "one-short {} must fail",
                stringify!($field)
            );
        }};
    }
    one_short_limit!(max_bridge_retained_bytes);
    one_short_limit!(max_faces);
    one_short_limit!(max_hinges);
    one_short_limit!(max_boundary_vertex_occurrences);
    one_short_limit!(max_excluded_shared_pairs);
    one_short_limit!(max_bridge_revalidation_peak_bytes);
    one_short_limit!(max_schedule_retained_bytes);
    one_short_limit!(max_session_shell_bytes);
    one_short_limit!(max_bridge_partition_search_work_per_node);
    one_short_limit!(max_interval_registry_validation_work_per_node);
    one_short_limit!(max_interval_registry_sort_comparisons_per_node);
    one_short_limit!(max_interval_registry_retained_bytes);
    one_short_limit!(max_shared_feature_membership_tests);
    one_short_limit!(max_ordinary_pair_node_tests);
    one_short_limit!(max_logical_work);
    one_short_limit!(max_publication_bytes);
    one_short_limit!(max_aggregate_peak_bytes);
    let mut one_short_schedule_work = limits33;
    one_short_schedule_work.schedule_limits.max_work = 14;
    assert_eq!(
        prove_ordinary_interval_clearance_v2(input_v2(n33, one_short_schedule_work)).unwrap_err(),
        OrdinaryIntervalErrorV2::ResourceLimit
    );
    let mut one_short_peak = limits33;
    one_short_peak.max_temporary_bytes = evidence33.resources.charged_temporary_bytes - 1;
    assert_eq!(
        prove_ordinary_interval_clearance_v2(input_v2(n33, one_short_peak)).unwrap_err(),
        OrdinaryIntervalErrorV2::ResourceLimit
    );
    let mut overflow_policy = limits33;
    overflow_policy.max_logical_work = usize::MAX;
    assert_eq!(
        prove_ordinary_interval_clearance_v2(input_v2(n33, overflow_policy)).unwrap_err(),
        OrdinaryIntervalErrorV2::ResourceLimit
    );
    assert_eq!(
        prove_ordinary_interval_clearance_with_checkpoint_v2(
            input_v2(n33, overflow_policy),
            || Err(OrdinaryIntervalStopV2::Cancelled),
        )
        .unwrap_err(),
        OrdinaryIntervalErrorV2::Cancelled
    );

    let mut substituted = n33.excluded_shared_pairs.clone();
    let replaced = substituted[0];
    let faces = n33.fixture.geometry.face_ids();
    let replacement = (0..faces.len())
        .flat_map(|first| {
            (first + 1..faces.len()).map(move |second| {
                OrdinaryIntervalFacePairV2::new(faces[first], faces[second])
                    .expect("distinct canonical faces")
            })
        })
        .find(|pair| {
            n33.excluded_shared_pairs
                .binary_search_by(|candidate| compare_pair_v2(candidate, pair))
                .is_err()
        })
        .expect("fixture has an ordinary pair");
    assert_ne!(replacement, replaced);
    assert!(
        substituted[1..]
            .binary_search_by(|candidate| compare_pair_v2(candidate, &replacement))
            .is_err()
    );
    substituted[0] = replacement;
    substituted.sort_unstable_by(compare_pair_v2);
    let substituted_input = OrdinaryIntervalInputV2 {
        excluded_shared_pairs: &substituted,
        ..input_v2(n33, limits33)
    };
    assert!(matches!(
        prove_ordinary_interval_clearance_v2(substituted_input),
        Err(OrdinaryIntervalErrorV2::ExcludedSharedPairCoverageMismatch
            | OrdinaryIntervalErrorV2::DuplicateExcludedSharedPair)
    ));

    let n34 = n34_fixture_v2();
    assert_eq!(n34.fixture.profile.actual_block_count_v2(), N34);
    let limits34 = strict_limits_v2(n34);
    let input34 = input_v2(n34, limits34);
    assert_nonzero_schedule(n34);
    let mut no_stop = || Ok(());
    let validated34 = resources::validate_input_v2(&input34, &mut no_stop)
        .expect("N34 nonzero schedule exact preflight");
    assert_phase_accounting_v2(&validated34);
    for error in [
        validated33
            .interval_transform_session
            .prepare_leaf_with_checkpoint_v2(
                0,
                0,
                limits33.schedule_limits,
                validated34.schedule_workspace_bound,
                limits33.max_schedule_evaluation_workspace_bytes,
                limits33.max_bridge_partition_search_work_per_node,
                &validated33.interval_transform_workspace_bound,
                || Ok(()),
            )
            .unwrap_err(),
        validated33
            .interval_transform_session
            .prepare_leaf_with_checkpoint_v2(
                0,
                0,
                limits33.schedule_limits,
                validated33.schedule_workspace_bound,
                limits33.max_schedule_evaluation_workspace_bytes,
                limits33.max_bridge_partition_search_work_per_node,
                &validated34.interval_transform_workspace_bound,
                || Ok(()),
            )
            .unwrap_err(),
    ] {
        assert_eq!(
            error,
            ori_kinematics::CommonArticulationDynamicClosureIntervalTransformLeafErrorV2::InvalidInput
        );
    }
    assert!(
        leaf.transform_for_canonical_face_position_v2(
            &n34.fixture.geometry,
            0,
            n34.fixture.geometry.face_ids()[0],
        )
        .is_none()
    );
    assert_eq!(
        n34.bridge
            .prepare_interval_transform_session_v2(bridge_revalidation_input_v2(n33))
            .unwrap_err(),
        ori_kinematics::CommonArticulationDynamicClosureBridgeErrorV2::IssuerMismatch
    );
    let mut foreign_schedule = bridge_revalidation_input_v2(n33);
    foreign_schedule.parent_schedule = &n34.schedule;
    assert!(matches!(
        n33.bridge
            .prepare_interval_transform_session_v2(foreign_schedule),
        Err(
            ori_kinematics::CommonArticulationDynamicClosureBridgeErrorV2::IssuerMismatch
                | ori_kinematics::CommonArticulationDynamicClosureBridgeErrorV2::InvalidInput
        )
    ));
    let mut foreign_geometry = bridge_revalidation_input_v2(n33);
    foreign_geometry.geometry = &n34.fixture.geometry;
    assert!(matches!(
        n33.bridge
            .prepare_interval_transform_session_v2(foreign_geometry),
        Err(
            ori_kinematics::CommonArticulationDynamicClosureBridgeErrorV2::IssuerMismatch
                | ori_kinematics::CommonArticulationDynamicClosureBridgeErrorV2::InvalidInput
        )
    ));
    assert_eq!(validated34.resources.face_count, 8 * N34 + 1);
    assert_eq!(validated34.resources.hinge_count, 12 * N34);
    assert_eq!(
        validated34.resources.charged_logical_work,
        limits34.max_logical_work
    );
    assert!(validated34.resources.ordinary_face_pairs > 0);
    let evidence34 = prove_ordinary_interval_clearance_v2(input34)
        .expect("N34 nonzero ordinary-pair interval clearance");
    assert!(evidence34.resources.ordinary_face_pairs > 0);
    assert!(evidence34.certified_ordinary_pair_leaf_count > 0);
}
