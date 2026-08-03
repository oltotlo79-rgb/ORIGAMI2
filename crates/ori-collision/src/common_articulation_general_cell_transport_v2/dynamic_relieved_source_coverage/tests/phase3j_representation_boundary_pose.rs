//! Phase 3J assertions attached to the sole genuine N33 integration proof.
//!
//! This helper defines no test; Phase 3I consumes its opaque proof and
//! transfers it here exactly once for the combined delegated replay.

use ori_foldability::GlobalFlatLayerOrderSourceAuthorityV2;
use ori_kinematics::{
    CanonicalCycleScheduleRepresentationBoundaryPoseAngleIdentityInputV2,
    CanonicalCycleScheduleRepresentationBoundaryPoseAngleIdentityLimitsV2,
    prove_canonical_cycle_schedule_representation_boundary_pose_angle_identity_evidence_v2,
};

use super::super::*;
use super::support::*;
use crate::CommonArticulationDynamicGeneralNRelievedClearanceLimitsV2;
use crate::dynamic_general_n_positive_thickness_v2::ordinary_interval::tests::{
    relief_support::ReliefFixtureInputV2, support::OrdinaryFixtureV2,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn assert_phase3j_representation_boundary_pose_v2<'a>(
    boundary: CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteV2,
    fixture: &'a OrdinaryFixtureV2,
    policies: &'a ReliefFixtureInputV2,
    public_limits: CommonArticulationDynamicGeneralNRelievedClearanceLimitsV2,
    fresh_authority: &'a GlobalFlatLayerOrderSourceAuthorityV2<'a>,
    coverage_limits: CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageLimitsV2,
    endpoint_limits:
        CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteLimitsV2,
    schedule_limits: ori_kinematics::CycleScheduleLimitsV1,
    limits:
        CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteLimitsV2,
) {
    let lower_angles = fixture.schedule.try_evaluate_v1(-1.0).unwrap();
    let upper_angles = fixture.schedule.try_evaluate_v1(1.0).unwrap();
    let lower_pose = fixture
        .fixture
        .geometry
        .solve_closed(
            &fixture.fixture.audit,
            fixture.fixture.parent_fixed_face,
            &lower_angles,
            fixture.fixture.closure_tolerance,
        )
        .expect("N33 lower representation-boundary pose object");
    let upper_pose = fixture
        .fixture
        .geometry
        .solve_closed(
            &fixture.fixture.audit,
            fixture.fixture.parent_fixed_face,
            &upper_angles,
            fixture.fixture.closure_tolerance,
        )
        .expect("N33 upper representation-boundary pose object");
    let pose_bound = fixture
        .schedule
        .checked_representation_boundary_pose_angle_identity_resource_bound_v2(
            &fixture.fixture.geometry,
            &fixture.fixture.audit,
            &lower_pose,
            &upper_pose,
            schedule_limits,
        )
        .expect("checked N33 pose-angle identity resources");
    let pose_limits = CanonicalCycleScheduleRepresentationBoundaryPoseAngleIdentityLimitsV2 {
        max_hinges: limits.max_hinges,
        max_schedule_deep_retained_bytes: limits.max_schedule_deep_retained_bytes,
        max_representation_boundary_poses_deep_retained_bytes: pose_bound
            .representation_boundary_poses_deep_retained_bytes_v2()
            + 1,
        max_logical_work: pose_bound.logical_work_required_v2(),
        max_workspace_bytes: pose_bound.workspace_peak_bytes_upper_bound_v2(),
    };
    let pose_identity =
        prove_canonical_cycle_schedule_representation_boundary_pose_angle_identity_evidence_v2(
            CanonicalCycleScheduleRepresentationBoundaryPoseAngleIdentityInputV2 {
                geometry: &fixture.fixture.geometry,
                audit: &fixture.fixture.audit,
                schedule: &fixture.schedule,
                closed_boundary_evidence: boundary.closed_boundary_evidence_internal_v2(),
                lower_pose: &lower_pose,
                upper_pose: &upper_pose,
                schedule_limits,
                limits: pose_limits,
            },
        )
        .expect("N33 representation-boundary pose-angle identity");
    assert!(pose_identity.hinge_count_v2() < pose_limits.max_hinges);
    assert!(
        pose_bound.schedule_deep_retained_bytes_v2() < pose_limits.max_schedule_deep_retained_bytes
    );
    assert!(
        pose_bound.representation_boundary_poses_deep_retained_bytes_v2()
            < pose_limits.max_representation_boundary_poses_deep_retained_bytes
    );
    assert_eq!(
        pose_bound.logical_work_required_v2(),
        pose_limits.max_logical_work
    );
    assert_eq!(
        pose_bound.workspace_peak_bytes_upper_bound_v2(),
        pose_limits.max_workspace_bytes
    );
    let mut phase3j_limits = exact_phase3j_limits_v2(&boundary, &pose_identity);
    phase3j_limits.max_retained_boundary_configuration_prerequisite_bytes += 1;
    phase3j_limits.max_publication_bytes += 1;
    phase3j_limits.max_aggregate_peak_bytes += 1;
    let phase3j = prove_common_articulation_dynamic_general_n_closed_dyadic_representation_boundary_pose_angle_identity_positive_thickness_prerequisite_v2(
        CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteInputV2 {
            boundary_configuration_prerequisite: boundary,
            pose_angle_identity_evidence: pose_identity,
            limits: phase3j_limits,
        },
    )
    .expect("Phase 3J joins positive thickness to the two scheduled-angle representation points");
    assert_eq!(phase3j.actual_block_count_v2(), 33);
    assert!(phase3j.actual_block_count_v2() < phase3j_limits.max_blocks);
    assert!(phase3j.hinge_count_v2() < phase3j_limits.max_hinges);
    assert!(
        pose_bound.schedule_deep_retained_bytes_v2()
            < phase3j_limits.max_schedule_deep_retained_bytes
    );
    assert!(
        pose_bound.representation_boundary_poses_deep_retained_bytes_v2()
            < phase3j_limits.max_representation_boundary_poses_deep_retained_bytes
    );
    assert_eq!(
        pose_bound.logical_work_required_v2(),
        phase3j_limits.max_pose_angle_identity_logical_work
    );
    assert_eq!(
        pose_bound.workspace_peak_bytes_upper_bound_v2(),
        phase3j_limits.max_pose_angle_identity_workspace_bytes
    );
    assert_eq!(phase3j.scheduled_angle_representation_point_count_v2(), 2);
    assert!(phase3j.both_scheduled_angle_representation_points_have_positive_thickness_v2());
    assert!(phase3j.matches_pose_instances_v2(&lower_pose, &upper_pose));
    assert!(
        phase3j.retained_boundary_configuration_prerequisite_bytes_v2()
            < phase3j_limits.max_retained_boundary_configuration_prerequisite_bytes
    );
    assert!(phase3j.publication_bytes_v2() < phase3j_limits.max_publication_bytes);
    assert!(
        phase3j.aggregate_peak_bytes_upper_bound_v2() < phase3j_limits.max_aggregate_peak_bytes
    );
    assert!(!phase3j.authorizes_source_target_identity());
    assert!(!phase3j.authorizes_current_requested_identity());
    assert!(!phase3j.authorizes_application_parameter_identity());
    assert!(!phase3j.authorizes_direction());
    assert!(!phase3j.authorizes_layer_order());
    assert!(!phase3j.authorizes_exact_closure());
    assert!(!phase3j.authorizes_transform_realization());
    assert!(!phase3j.authorizes_pose_realization());
    assert!(!phase3j.authorizes_continuous_motion());
    assert!(!phase3j.authorizes_collision_clearance());
    assert!(!phase3j.authorizes_layer_transport());
    assert!(!phase3j.authorizes_project_mutation());
    assert!(!phase3j.authorizes_apply());
    assert!(!phase3j.authorizes_viewer());
    assert!(!phase3j.authorizes_export());
    let debug = format!("{phase3j:?}");
    for secret in [
        "boundary_configuration_prerequisite",
        "pose_angle_identity_evidence",
        "binding_fingerprint",
        "issuer_geometry",
        "lower_pose_instance",
        "upper_pose_instance",
        "schedule_binding",
        "graph_binding",
        "closed_boundary",
        "hinge_angles",
        "transforms",
        "closure",
        "tolerance",
    ] {
        assert!(!debug.contains(secret), "Phase 3J Debug leaked {secret}");
    }

    let required = [
        phase3j.actual_block_count_v2(),
        phase3j.hinge_count_v2(),
        phase3j_limits.max_schedule_deep_retained_bytes,
        phase3j_limits.max_representation_boundary_poses_deep_retained_bytes,
        phase3j_limits.max_pose_angle_identity_logical_work,
        phase3j_limits.max_pose_angle_identity_workspace_bytes,
        phase3j.retained_boundary_configuration_prerequisite_bytes_v2(),
        phase3j.publication_bytes_v2(),
        phase3j.aggregate_peak_bytes_upper_bound_v2(),
    ];
    for (field, cap) in phase3j_limit_values_v2(phase3j_limits)
        .into_iter()
        .enumerate()
    {
        for invalid in [0, cap - 1, usize::MAX] {
            let invalid_limits = set_phase3j_limit_v2(phase3j_limits, field, invalid);
            let expected = if invalid == 0 || invalid == usize::MAX || invalid < required[field] {
                CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteErrorV2::ResourceLimit
            } else {
                CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteErrorV2::CertificateBindingMismatch
            };
            let mut polls = 0usize;
            assert_eq!(
                phase3j.revalidate_with_checkpoint_v2(
                    phase3j_replay_input_v2(
                        boundary_configuration_replay_input_v2(
                            fixture,
                            policies,
                            public_limits,
                            fresh_authority,
                            coverage_limits,
                            endpoint_limits,
                            schedule_limits,
                            limits,
                        ),
                        &fixture.fixture.audit,
                        &lower_pose,
                        &upper_pose,
                        invalid_limits,
                    ),
                    || {
                        polls += 1;
                        Ok(())
                    },
                ),
                Err(expected),
                "Phase 3J limit field {field}, invalid {invalid}",
            );
            assert_eq!(polls, 1);
        }
    }

    let drifted_phase3j_limits = CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteLimitsV2 {
        max_aggregate_peak_bytes: phase3j_limits.max_aggregate_peak_bytes + 1,
        ..phase3j_limits
    };
    let mut polls = 0usize;
    assert_eq!(
        phase3j.revalidate_with_checkpoint_v2(
            phase3j_replay_input_v2(
                boundary_configuration_replay_input_v2(
                    fixture,
                    policies,
                    public_limits,
                    fresh_authority,
                    coverage_limits,
                    endpoint_limits,
                    schedule_limits,
                    limits,
                ),
                &fixture.fixture.audit,
                &lower_pose,
                &upper_pose,
                drifted_phase3j_limits,
            ),
            || {
                polls += 1;
                Ok(())
            },
        ),
        Err(CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteErrorV2::CertificateBindingMismatch)
    );
    assert_eq!(polls, 1);

    for (stop, expected) in [
        (
            CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteStopV2::Cancelled,
            CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteErrorV2::Cancelled,
        ),
        (
            CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteStopV2::DeadlineExceeded,
            CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteErrorV2::DeadlineExceeded,
        ),
    ] {
        assert_eq!(
            phase3j.revalidate_with_checkpoint_v2(
                phase3j_replay_input_v2(
                    boundary_configuration_replay_input_v2(
                        fixture,
                        policies,
                        public_limits,
                        fresh_authority,
                        coverage_limits,
                        endpoint_limits,
                        schedule_limits,
                        limits,
                    ),
                    &fixture.fixture.audit,
                    &lower_pose,
                    &upper_pose,
                    phase3j_limits,
                ),
                || Err(stop),
            ),
            Err(expected)
        );
    }

    let fresh_lower_pose = fixture
        .fixture
        .geometry
        .solve_closed(
            &fixture.fixture.audit,
            fixture.fixture.parent_fixed_face,
            &lower_angles,
            fixture.fixture.closure_tolerance,
        )
        .unwrap();
    for (candidate_lower, candidate_upper) in
        [(&fresh_lower_pose, &upper_pose), (&upper_pose, &lower_pose)]
    {
        let mut polls = 0usize;
        assert_eq!(
            phase3j.revalidate_with_checkpoint_v2(
                phase3j_replay_input_v2(
                    boundary_configuration_replay_input_v2(
                        fixture,
                        policies,
                        public_limits,
                        fresh_authority,
                        coverage_limits,
                        endpoint_limits,
                        schedule_limits,
                        limits,
                    ),
                    &fixture.fixture.audit,
                    candidate_lower,
                    candidate_upper,
                    phase3j_limits,
                ),
                || {
                    polls += 1;
                    Ok(())
                },
            ),
            Err(CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteErrorV2::CertificateBindingMismatch)
        );
        assert_eq!(polls, 1);
    }

    super::phase3k_canonical_pose::assert_phase3k_canonical_pose_v2(
        phase3j,
        fixture,
        policies,
        public_limits,
        fresh_authority,
        coverage_limits,
        endpoint_limits,
        schedule_limits,
        limits,
        phase3j_limits,
        &lower_pose,
        &upper_pose,
    );
}

const fn phase3j_limit_values_v2(
    limits: CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteLimitsV2,
) -> [usize; 9] {
    [
        limits.max_blocks,
        limits.max_hinges,
        limits.max_schedule_deep_retained_bytes,
        limits.max_representation_boundary_poses_deep_retained_bytes,
        limits.max_pose_angle_identity_logical_work,
        limits.max_pose_angle_identity_workspace_bytes,
        limits.max_retained_boundary_configuration_prerequisite_bytes,
        limits.max_publication_bytes,
        limits.max_aggregate_peak_bytes,
    ]
}

fn set_phase3j_limit_v2(
    mut limits: CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteLimitsV2,
    field: usize,
    value: usize,
) -> CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteLimitsV2{
    match field {
        0 => limits.max_blocks = value,
        1 => limits.max_hinges = value,
        2 => limits.max_schedule_deep_retained_bytes = value,
        3 => limits.max_representation_boundary_poses_deep_retained_bytes = value,
        4 => limits.max_pose_angle_identity_logical_work = value,
        5 => limits.max_pose_angle_identity_workspace_bytes = value,
        6 => limits.max_retained_boundary_configuration_prerequisite_bytes = value,
        7 => limits.max_publication_bytes = value,
        8 => limits.max_aggregate_peak_bytes = value,
        _ => unreachable!(),
    }
    limits
}
