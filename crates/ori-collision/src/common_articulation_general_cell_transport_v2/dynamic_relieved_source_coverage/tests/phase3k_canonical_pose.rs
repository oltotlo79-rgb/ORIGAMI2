//! Phase 3K assertions attached to the sole genuine N33 integration proof.
//!
//! The preceding Phase 3J proof is consumed here, so the expensive delegated
//! collision replay still runs exactly once for the combined proof chain.

use std::mem::size_of;

use ori_foldability::GlobalFlatLayerOrderSourceAuthorityV2;
use ori_kinematics::{
    CanonicalBinary64PosePairTransformRealizationInputV2,
    CanonicalBinary64PosePairTransformRealizationLimitsV2, ClosedMaterialHingeGraphPose,
    prove_canonical_binary64_pose_pair_transform_realization_evidence_v2,
};

use super::super::*;
use super::support::*;
use crate::CommonArticulationDynamicGeneralNRelievedClearanceLimitsV2;
use crate::dynamic_general_n_positive_thickness_v2::ordinary_interval::tests::{
    relief_support::ReliefFixtureInputV2, support::OrdinaryFixtureV2,
};

#[path = "phase3k_canonical_pose/support.rs"]
mod phase3k_support;
use phase3k_support::*;

type Phase3JLimitsV2 = CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteLimitsV2;
type Phase3JV2 = CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseAngleIdentityPositiveThicknessPrerequisiteV2;
type Phase3KErrorV2 = CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseCanonicalBinary64TransformPositiveThicknessPrerequisiteErrorV2;
type Phase3KLimitsV2 = CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseCanonicalBinary64TransformPositiveThicknessPrerequisiteLimitsV2;
type Phase3KV2 = CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseCanonicalBinary64TransformPositiveThicknessPrerequisiteV2;

#[allow(clippy::too_many_arguments)]
pub(super) fn assert_phase3k_canonical_pose_v2<'a>(
    phase3j: Phase3JV2,
    fixture: &'a OrdinaryFixtureV2,
    policies: &'a ReliefFixtureInputV2,
    public_limits: CommonArticulationDynamicGeneralNRelievedClearanceLimitsV2,
    fresh_authority: &'a GlobalFlatLayerOrderSourceAuthorityV2<'a>,
    coverage_limits: CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageLimitsV2,
    endpoint_limits:
        CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteLimitsV2,
    schedule_limits: ori_kinematics::CycleScheduleLimitsV1,
    boundary_limits:
        CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteLimitsV2,
    phase3j_limits: Phase3JLimitsV2,
    lower_pose: &'a ClosedMaterialHingeGraphPose,
    upper_pose: &'a ClosedMaterialHingeGraphPose,
) {
    let geometry = &fixture.fixture.geometry;
    let transform_bound = geometry
        .checked_canonical_binary64_pose_pair_transform_realization_resource_bound_v2(
            &fixture.fixture.audit,
            lower_pose,
            upper_pose,
        )
        .expect("checked N33 canonical binary64 transform resources");
    let transform_limits = CanonicalBinary64PosePairTransformRealizationLimitsV2 {
        max_faces: transform_bound.face_count_v2() + 1,
        max_hinges: phase3j_limits.max_hinges,
        max_pose_pair_deep_retained_bytes: phase3j_limits
            .max_representation_boundary_poses_deep_retained_bytes,
        max_logical_work: transform_bound.logical_work_required_v2(),
        max_workspace_bytes: transform_bound.workspace_structural_requirement_bytes_v2() + 1,
    };
    let transform = prove_canonical_binary64_pose_pair_transform_realization_evidence_v2(
        CanonicalBinary64PosePairTransformRealizationInputV2 {
            geometry,
            audit: &fixture.fixture.audit,
            fixed_face: lower_pose.fixed_face(),
            lower_pose,
            upper_pose,
            limits: transform_limits,
        },
    )
    .expect("N33 canonical binary64 representation-boundary transforms");
    assert!(
        transform.proves_both_pose_instances_are_canonical_binary64_transform_realizations_v2()
    );
    assert_eq!(transform.realized_pose_count_v2(), 2);
    assert_eq!(transform.face_count_v2(), transform_bound.face_count_v2());
    assert_eq!(transform.hinge_count_v2(), transform_bound.hinge_count_v2());
    assert_eq!(
        transform.pose_pair_deep_retained_bytes_v2(),
        transform_bound.pose_pair_deep_retained_bytes_v2()
    );
    assert_eq!(
        transform.logical_work_v2(),
        transform_bound.logical_work_required_v2()
    );
    assert_eq!(
        transform.workspace_structural_requirement_bytes_v2(),
        transform_bound.workspace_structural_requirement_bytes_v2()
    );
    assert_eq!(
        transform.workspace_peak_bytes_upper_bound_v2(),
        transform_limits.max_workspace_bytes
    );
    assert!(transform.matches_geometry_instance_v2(geometry));
    assert!(transform.matches_pose_instances_v2(lower_pose, upper_pose));
    assert!(!transform.authorizes_source_target_identity());
    assert!(!transform.authorizes_current_requested_identity());
    assert!(!transform.authorizes_application_parameter_identity());
    assert!(!transform.authorizes_direction());
    assert!(!transform.authorizes_layer_order());
    assert!(!transform.authorizes_exact_closure());
    assert!(!transform.authorizes_transform_realization());
    assert!(!transform.authorizes_pose_realization());
    assert!(!transform.authorizes_continuous_motion());
    assert!(!transform.authorizes_collision_clearance());
    assert!(!transform.authorizes_layer_transport());
    assert!(!transform.authorizes_project_mutation());
    assert!(!transform.authorizes_apply());
    assert!(!transform.authorizes_viewer());
    assert!(!transform.authorizes_export());

    let retained_phase3j = size_of::<Phase3JV2>();
    let retained_transform = size_of_val(&transform);
    let publication = size_of::<Phase3KV2>();
    let delegated_phase3j = phase3j
        .replay_aggregate_peak_cap_internal_v2()
        .checked_add(publication - retained_phase3j)
        .unwrap();
    let transform_replay = publication
        .checked_add(transform_limits.max_pose_pair_deep_retained_bytes)
        .and_then(|value| value.checked_add(transform_limits.max_workspace_bytes))
        .and_then(|value| value.checked_add(retained_transform))
        .unwrap();
    let composition = publication
        + super::super::closed_dyadic_representation_boundary_pose_canonical_binary64_transform_positive_thickness::COMPOSITION_WORKSPACE_BYTES_V2;
    let phase3k_limits = Phase3KLimitsV2 {
        max_blocks: phase3j.block_count_cap_internal_v2(),
        max_faces: transform_limits.max_faces,
        max_hinges: transform_limits.max_hinges,
        max_pose_pair_deep_retained_bytes: transform_limits.max_pose_pair_deep_retained_bytes,
        max_canonical_transform_logical_work: transform_limits.max_logical_work,
        max_canonical_transform_workspace_bytes: transform_limits.max_workspace_bytes,
        max_retained_phase3j_prerequisite_bytes: retained_phase3j,
        max_retained_transform_realization_evidence_bytes: retained_transform,
        max_publication_bytes: publication,
        max_aggregate_peak_bytes: delegated_phase3j.max(transform_replay).max(composition),
    };
    let phase3k = prove_common_articulation_dynamic_general_n_closed_dyadic_representation_boundary_pose_canonical_binary64_transform_positive_thickness_prerequisite_v2(
        CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseCanonicalBinary64TransformPositiveThicknessPrerequisiteInputV2 {
            phase3j,
            transform_realization: transform,
            geometry,
            lower_pose,
            upper_pose,
            limits: phase3k_limits,
        },
    )
    .expect("Phase 3K joins positive thickness to canonical binary64 transforms");

    assert_eq!(phase3k.actual_block_count_v2(), 33);
    assert_eq!(phase3k.face_count_v2(), transform_bound.face_count_v2());
    assert_eq!(phase3k.hinge_count_v2(), transform_bound.hinge_count_v2());
    assert!(phase3k.matches_pose_instances_v2(lower_pose, upper_pose));
    assert!(phase3k.proves_both_scheduled_representation_boundary_pose_objects_have_positive_thickness_and_canonical_binary64_transform_realization_v2());
    assert_eq!(
        phase3k.retained_phase3j_prerequisite_bytes_v2(),
        retained_phase3j
    );
    assert_eq!(
        phase3k.retained_transform_realization_evidence_bytes_v2(),
        retained_transform
    );
    assert_eq!(phase3k.publication_bytes_v2(), publication);
    assert_eq!(
        phase3k.canonical_transform_logical_work_v2(),
        transform_bound.logical_work_required_v2()
    );
    assert_eq!(
        phase3k.canonical_transform_workspace_bytes_upper_bound_v2(),
        transform_limits.max_workspace_bytes
    );
    assert_eq!(
        phase3k.aggregate_peak_bytes_upper_bound_v2(),
        phase3k_limits.max_aggregate_peak_bytes
    );
    assert!(!phase3k.authorizes_source_target_identity());
    assert!(!phase3k.authorizes_current_requested_identity());
    assert!(!phase3k.authorizes_application_parameter_identity());
    assert!(!phase3k.authorizes_direction());
    assert!(!phase3k.authorizes_layer_order());
    assert!(!phase3k.authorizes_exact_closure());
    assert!(!phase3k.authorizes_transform_realization());
    assert!(!phase3k.authorizes_pose_realization());
    assert!(!phase3k.authorizes_continuous_motion());
    assert!(!phase3k.authorizes_collision_clearance());
    assert!(!phase3k.authorizes_layer_transport());
    assert!(!phase3k.authorizes_project_mutation());
    assert!(!phase3k.authorizes_apply());
    assert!(!phase3k.authorizes_viewer());
    assert!(!phase3k.authorizes_export());
    let debug = format!("{phase3k:?}");
    for secret in [
        "phase3j",
        "transform_realization",
        "binding_fingerprint",
        "issuer_geometry",
        "lower_pose_instance",
        "upper_pose_instance",
        "audit_binding",
        "hinge_angles",
        "transforms",
        "closure",
        "tolerance",
    ] {
        assert!(!debug.contains(secret), "Phase 3K Debug leaked {secret}");
    }

    let required = [
        phase3k.actual_block_count_v2(),
        phase3k.face_count_v2(),
        phase3k.hinge_count_v2(),
        transform_bound.pose_pair_deep_retained_bytes_v2(),
        phase3k.canonical_transform_logical_work_v2(),
        transform_bound.workspace_structural_requirement_bytes_v2(),
        phase3k.retained_phase3j_prerequisite_bytes_v2(),
        phase3k.retained_transform_realization_evidence_bytes_v2(),
        phase3k.publication_bytes_v2(),
        phase3k.aggregate_peak_bytes_upper_bound_v2(),
    ];
    for (field, required_value) in required.into_iter().enumerate() {
        for invalid in [0, required_value - 1, usize::MAX] {
            let invalid_limits = set_phase3k_limit_v2(phase3k_limits, field, invalid);
            let mut polls = 0usize;
            assert_eq!(
                phase3k.revalidate_with_checkpoint_v2(
                    phase3k_replay_input_v2(
                        fixture,
                        policies,
                        public_limits,
                        fresh_authority,
                        coverage_limits,
                        endpoint_limits,
                        schedule_limits,
                        boundary_limits,
                        phase3j_limits,
                        lower_pose,
                        upper_pose,
                        transform_limits,
                        invalid_limits,
                    ),
                    || {
                        polls += 1;
                        Ok(())
                    },
                ),
                Err(Phase3KErrorV2::ResourceLimit),
                "Phase 3K limit field {field}, invalid {invalid}",
            );
            assert_eq!(polls, 1);
        }
    }

    for field in 0..10 {
        let drifted = set_phase3k_limit_v2(
            phase3k_limits,
            field,
            phase3k_limit_values_v2(phase3k_limits)[field] + 1,
        );
        let mut polls = 0usize;
        assert_eq!(
            phase3k.revalidate_with_checkpoint_v2(
                phase3k_replay_input_v2(
                    fixture,
                    policies,
                    public_limits,
                    fresh_authority,
                    coverage_limits,
                    endpoint_limits,
                    schedule_limits,
                    boundary_limits,
                    phase3j_limits,
                    lower_pose,
                    upper_pose,
                    transform_limits,
                    drifted,
                ),
                || {
                    polls += 1;
                    Ok(())
                },
            ),
            Err(Phase3KErrorV2::CertificateBindingMismatch),
            "Phase 3K valid limit drift field {field}",
        );
        assert_eq!(polls, 1);
    }

    let drifted_transform_limits = CanonicalBinary64PosePairTransformRealizationLimitsV2 {
        max_workspace_bytes: transform_limits.max_workspace_bytes + 1,
        ..transform_limits
    };
    let mut polls = 0usize;
    assert_eq!(
        phase3k.revalidate_with_checkpoint_v2(
            phase3k_replay_input_v2(
                fixture,
                policies,
                public_limits,
                fresh_authority,
                coverage_limits,
                endpoint_limits,
                schedule_limits,
                boundary_limits,
                phase3j_limits,
                lower_pose,
                upper_pose,
                drifted_transform_limits,
                phase3k_limits,
            ),
            || {
                polls += 1;
                Ok(())
            },
        ),
        Err(Phase3KErrorV2::CertificateBindingMismatch)
    );
    assert_eq!(polls, 1);

    for (stop, expected) in [
        (
            CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseCanonicalBinary64TransformPositiveThicknessPrerequisiteStopV2::Cancelled,
            Phase3KErrorV2::Cancelled,
        ),
        (
            CommonArticulationDynamicGeneralNClosedDyadicRepresentationBoundaryPoseCanonicalBinary64TransformPositiveThicknessPrerequisiteStopV2::DeadlineExceeded,
            Phase3KErrorV2::DeadlineExceeded,
        ),
    ] {
        assert_eq!(
            phase3k.revalidate_with_checkpoint_v2(
                phase3k_replay_input_v2(
                    fixture,
                    policies,
                    public_limits,
                    fresh_authority,
                    coverage_limits,
                    endpoint_limits,
                    schedule_limits,
                    boundary_limits,
                    phase3j_limits,
                    lower_pose,
                    upper_pose,
                    transform_limits,
                    phase3k_limits,
                ),
                || Err(stop),
            ),
            Err(expected)
        );
    }

    let fresh_lower_pose = geometry
        .solve_closed(
            &fixture.fixture.audit,
            lower_pose.fixed_face(),
            lower_pose.hinge_angles(),
            fixture.fixture.closure_tolerance,
        )
        .unwrap();
    for (candidate_lower, candidate_upper) in
        [(&fresh_lower_pose, upper_pose), (upper_pose, lower_pose)]
    {
        let mut polls = 0usize;
        assert_eq!(
            phase3k.revalidate_with_checkpoint_v2(
                phase3k_replay_input_v2(
                    fixture,
                    policies,
                    public_limits,
                    fresh_authority,
                    coverage_limits,
                    endpoint_limits,
                    schedule_limits,
                    boundary_limits,
                    phase3j_limits,
                    candidate_lower,
                    candidate_upper,
                    transform_limits,
                    phase3k_limits,
                ),
                || {
                    polls += 1;
                    Ok(())
                },
            ),
            Err(Phase3KErrorV2::CertificateBindingMismatch)
        );
        assert_eq!(polls, 1);
    }

    let mut successful_polls = 0usize;
    phase3k
        .revalidate_with_checkpoint_v2(
            phase3k_replay_input_v2(
                fixture,
                policies,
                public_limits,
                fresh_authority,
                coverage_limits,
                endpoint_limits,
                schedule_limits,
                boundary_limits,
                phase3j_limits,
                lower_pose,
                upper_pose,
                transform_limits,
                phase3k_limits,
            ),
            || {
                successful_polls += 1;
                Ok(())
            },
        )
        .expect("combined N33 Phase 3K replay");
    assert!(successful_polls > 100);
}
