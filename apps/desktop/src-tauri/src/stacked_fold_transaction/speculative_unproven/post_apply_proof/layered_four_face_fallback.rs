//! Exact post-Apply fallback for the narrow four-face/three-hinge chain
//! theorem. No other topology, schedule, thickness, or evidence class enters
//! this route.

use ori_collision::{
    LayeredFourFaceChainContinuousErrorV1, LayeredFourFaceChainContinuousLimitsV1,
    MAX_DYADIC_FACE_TRANSFORM_LEAVES_V1,
    certify_layered_four_face_chain_continuous_path_with_control_v1,
};
use ori_core::{
    SpeculativeUnprovenFoldLayeredFourFaceCertificationErrorV1,
    bind_speculative_unproven_layered_four_face_chain_continuous_proof_with_control_v1,
};

use super::layered_three_face_fallback::{
    admission_error_to_worker_v1, direct_certificate_error_to_worker_v1,
    stopped_worker_certificate_v1,
};
use super::*;

const MAX_LAYERED_FOUR_FACE_DYADIC_DEPTH_V1: u8 = 7;
const MAX_LAYERED_FOUR_FACE_LEAVES_V1: usize = MAX_DYADIC_FACE_TRANSFORM_LEAVES_V1;
const _: () =
    assert!(MAX_LAYERED_FOUR_FACE_LEAVES_V1 == 1 << MAX_LAYERED_FOUR_FACE_DYADIC_DEPTH_V1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LayeredFourFaceFallbackDecisionV1 {
    OrdinaryUncertified,
    LayeredAttempt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LayeredFourFaceFailureDispositionV1 {
    Cancelled,
    DeadlineExceeded,
    ResourceUnavailable,
    Uncertified,
    BindingRejected,
}

pub(super) fn is_layered_four_face_fallback_candidate_v1(
    premise: &PostApplyProofPremiseV1,
) -> bool {
    let model = premise.requested.initial().target().model();
    let source = premise.requested.initial().pose().hinge_angles();
    let target = premise.requested.pose().hinge_angles();
    layered_four_face_fallback_decision_v1(
        premise.paper_thickness_mm,
        model.face_ids().len(),
        model.hinges().len(),
        source.iter().zip(target).map(|(source, target)| {
            (
                source.edge() == target.edge(),
                source.angle_degrees(),
                target.angle_degrees(),
            )
        }),
        source.len() == target.len(),
    ) == LayeredFourFaceFallbackDecisionV1::LayeredAttempt
}

pub(super) fn layered_four_face_fallback_decision_v1(
    paper_thickness_mm: f64,
    face_count: usize,
    hinge_count: usize,
    hinge_schedule: impl IntoIterator<Item = (bool, f64, f64)>,
    same_schedule_length: bool,
) -> LayeredFourFaceFallbackDecisionV1 {
    if paper_thickness_mm.to_bits() != 0.0_f64.to_bits()
        || face_count != 4
        || hinge_count != 3
        || !same_schedule_length
    {
        return LayeredFourFaceFallbackDecisionV1::OrdinaryUncertified;
    }
    let mut moving = 0_usize;
    let mut stationary_flat = 0_usize;
    let mut schedule_count = 0_usize;
    for (same_edge, source_angle, target_angle) in hinge_schedule {
        schedule_count += 1;
        if !same_edge {
            return LayeredFourFaceFallbackDecisionV1::OrdinaryUncertified;
        }
        if source_angle.to_bits() == 0.0_f64.to_bits() && target_angle > 0.0 && target_angle < 180.0
        {
            moving += 1;
        } else if source_angle.to_bits() == 180.0_f64.to_bits()
            && target_angle.to_bits() == 180.0_f64.to_bits()
        {
            stationary_flat += 1;
        }
    }
    if schedule_count == 3 && moving == 1 && stationary_flat == 2 {
        LayeredFourFaceFallbackDecisionV1::LayeredAttempt
    } else {
        LayeredFourFaceFallbackDecisionV1::OrdinaryUncertified
    }
}

pub(super) fn run_layered_four_face_fallback_v1(
    premise: PostApplyProofPremiseV1,
    control: &CooperativeOperationControlV1<'_>,
) -> PostApplyProofWorkerCertificateV1 {
    let target_angles = match target_angles_for_premise_v1(&premise, control) {
        Ok(target_angles) => target_angles,
        Err(error) => return direct_certificate_error_to_worker_v1(premise, error),
    };
    if let Some(terminal) = cooperative_stop_terminal_v1(control) {
        return stopped_worker_certificate_v1(premise, terminal);
    }
    let initial = premise.requested.initial();
    let admission = match prepare_stacked_fold_initial_sample_layer_admission_with_control_v1(
        initial.target().model(),
        initial.pose(),
        premise.paper_thickness_mm,
        LayeredFourFaceChainContinuousLimitsV1::default().static_limits,
        &premise.initial_layer_order,
        control,
    ) {
        Ok(admission) => admission,
        Err(error) => return admission_error_to_worker_v1(premise, error),
    };
    if let Some(terminal) = cooperative_stop_terminal_v1(control) {
        return stopped_worker_certificate_v1(premise, terminal);
    }
    let first_depth = next_four_face_depth_v1(None, MAX_LAYERED_FOUR_FACE_DYADIC_DEPTH_V1)
        .expect("the bounded depth family always starts at zero");
    let (limits, certificate) = match certify_four_face_progressive_depths_v1(
        initial.target().model(),
        initial.pose(),
        &target_angles,
        &admission,
        first_depth,
        control,
    ) {
        Ok(value) => value,
        Err(error) => return four_face_certificate_error_to_worker_v1(premise, error),
    };
    // The native issuer's final internal checkpoint and this caller-side
    // checkpoint form an explicit hand-off boundary. A stop observed after
    // native authority issuance cannot reach the one-shot core binder.
    if let Some(terminal) = cooperative_stop_terminal_v1(control) {
        drop(certificate);
        return stopped_worker_certificate_v1(premise, terminal);
    }
    let PostApplyProofPremiseV1 {
        resolution_ticket,
        binding,
        requested,
        initial_layer_order,
        target_revision,
        target_fingerprint,
        target_pose_generation,
        paper_thickness_mm,
    } = premise;
    match bind_speculative_unproven_layered_four_face_chain_continuous_proof_with_control_v1(
        resolution_ticket,
        &requested,
        &admission,
        limits,
        certificate,
        control,
    ) {
        Ok(proof) => PostApplyProofWorkerCertificateV1::Certified(
            PostApplyProofCertifiedAuthorityV1::LayeredFourFace(proof),
        ),
        Err(failure) => {
            let error = failure.error().clone();
            let (_, resolution_ticket, recovered_certificate) = failure.into_parts();
            drop(recovered_certificate);
            let premise = PostApplyProofPremiseV1 {
                resolution_ticket,
                binding,
                requested,
                initial_layer_order,
                target_revision,
                target_fingerprint,
                target_pose_generation,
                paper_thickness_mm,
            };
            match error {
                SpeculativeUnprovenFoldLayeredFourFaceCertificationErrorV1::Cancelled => {
                    PostApplyProofWorkerCertificateV1::Cancelled(premise)
                }
                SpeculativeUnprovenFoldLayeredFourFaceCertificationErrorV1::DeadlineExceeded => {
                    PostApplyProofWorkerCertificateV1::DeadlineExceeded(premise)
                }
                SpeculativeUnprovenFoldLayeredFourFaceCertificationErrorV1::Common(
                    SpeculativeUnprovenFoldCertificationErrorV1::RequestedTargetAngleAllocationFailed
                    | SpeculativeUnprovenFoldCertificationErrorV1::ValidationPanicked
                    | SpeculativeUnprovenFoldCertificationErrorV1::ResourceUnavailable,
                )
                | SpeculativeUnprovenFoldLayeredFourFaceCertificationErrorV1::ResourceUnavailable => {
                    PostApplyProofWorkerCertificateV1::ResourceUnavailable(premise)
                }
                _ => PostApplyProofWorkerCertificateV1::BindingRejected(premise),
            }
        }
    }
}

fn certify_four_face_progressive_depths_v1(
    model: &ori_kinematics::MaterialTreeKinematicsModel,
    source_pose: &ori_kinematics::MaterialTreePose,
    target_angles: &ori_kinematics::CanonicalHingeAngles,
    admission: &ori_collision::NativeStackedFoldInitialSampleLayerAdmissionV1<
        StackedFoldInitialLayerOrderV1,
    >,
    first_depth: u8,
    control: &CooperativeOperationControlV1<'_>,
) -> Result<
    (
        LayeredFourFaceChainContinuousLimitsV1,
        ori_collision::LayeredFourFaceChainContinuousCertificateV1,
    ),
    LayeredFourFaceChainContinuousErrorV1,
> {
    run_four_face_progressive_depths_with_v1(first_depth, control, |limits| {
        certify_layered_four_face_chain_continuous_path_with_control_v1(
            model,
            source_pose,
            target_angles,
            admission,
            limits,
            control,
        )
    })
}

fn run_four_face_progressive_depths_with_v1<T>(
    first_depth: u8,
    control: &CooperativeOperationControlV1<'_>,
    mut issue: impl FnMut(
        LayeredFourFaceChainContinuousLimitsV1,
    ) -> Result<T, LayeredFourFaceChainContinuousErrorV1>,
) -> Result<(LayeredFourFaceChainContinuousLimitsV1, T), LayeredFourFaceChainContinuousErrorV1> {
    let mut depth = first_depth;
    loop {
        four_face_checkpoint_v1(control)?;
        let limits = layered_four_face_limits_for_depth_v1(depth)
            .ok_or(LayeredFourFaceChainContinuousErrorV1::ResourceLimit)?;
        let result = issue(limits);
        four_face_checkpoint_v1(control)?;
        match result {
            Ok(authority) => return Ok((limits, authority)),
            Err(LayeredFourFaceChainContinuousErrorV1::NonadjacentIntervalOverlap) => {
                let Some(next) =
                    next_four_face_depth_v1(Some(depth), MAX_LAYERED_FOUR_FACE_DYADIC_DEPTH_V1)
                else {
                    return Err(LayeredFourFaceChainContinuousErrorV1::NonadjacentIntervalOverlap);
                };
                depth = next;
            }
            Err(error) => return Err(error),
        }
    }
}

fn layered_four_face_limits_for_depth_v1(
    depth: u8,
) -> Option<LayeredFourFaceChainContinuousLimitsV1> {
    let required_leaves = 1_usize.checked_shl(u32::from(depth))?;
    layered_four_face_limits_for_depth_and_cap_v1(depth, required_leaves)
}

fn layered_four_face_limits_for_depth_and_cap_v1(
    depth: u8,
    max_leaves: usize,
) -> Option<LayeredFourFaceChainContinuousLimitsV1> {
    if depth > MAX_LAYERED_FOUR_FACE_DYADIC_DEPTH_V1 {
        return None;
    }
    let required_leaves = 1_usize.checked_shl(u32::from(depth))?;
    if max_leaves != required_leaves || max_leaves > MAX_LAYERED_FOUR_FACE_LEAVES_V1 {
        return None;
    }
    Some(LayeredFourFaceChainContinuousLimitsV1 {
        dyadic_depth: depth,
        max_leaves,
        ..LayeredFourFaceChainContinuousLimitsV1::default()
    })
}

const fn next_four_face_depth_v1(current: Option<u8>, maximum: u8) -> Option<u8> {
    match current {
        None if maximum <= MAX_LAYERED_FOUR_FACE_DYADIC_DEPTH_V1 => Some(0),
        None => None,
        Some(current) if current < maximum && current < MAX_LAYERED_FOUR_FACE_DYADIC_DEPTH_V1 => {
            Some(current + 1)
        }
        Some(_) => None,
    }
}

fn four_face_checkpoint_v1(
    control: &CooperativeOperationControlV1<'_>,
) -> Result<(), LayeredFourFaceChainContinuousErrorV1> {
    control.checkpoint().map_err(|stop| match stop {
        CooperativeOperationStopV1::Cancelled => LayeredFourFaceChainContinuousErrorV1::Cancelled,
        CooperativeOperationStopV1::DeadlineExceeded => {
            LayeredFourFaceChainContinuousErrorV1::DeadlineExceeded
        }
    })
}

fn four_face_certificate_error_to_worker_v1(
    premise: PostApplyProofPremiseV1,
    error: LayeredFourFaceChainContinuousErrorV1,
) -> PostApplyProofWorkerCertificateV1 {
    match four_face_certificate_failure_disposition_v1(error) {
        LayeredFourFaceFailureDispositionV1::Cancelled => {
            PostApplyProofWorkerCertificateV1::Cancelled(premise)
        }
        LayeredFourFaceFailureDispositionV1::DeadlineExceeded => {
            PostApplyProofWorkerCertificateV1::DeadlineExceeded(premise)
        }
        LayeredFourFaceFailureDispositionV1::ResourceUnavailable => {
            PostApplyProofWorkerCertificateV1::ResourceUnavailable(premise)
        }
        LayeredFourFaceFailureDispositionV1::Uncertified => {
            PostApplyProofWorkerCertificateV1::Uncertified(premise)
        }
        LayeredFourFaceFailureDispositionV1::BindingRejected => {
            PostApplyProofWorkerCertificateV1::BindingRejected(premise)
        }
    }
}

fn four_face_certificate_failure_disposition_v1(
    error: LayeredFourFaceChainContinuousErrorV1,
) -> LayeredFourFaceFailureDispositionV1 {
    match error {
        LayeredFourFaceChainContinuousErrorV1::Cancelled => {
            LayeredFourFaceFailureDispositionV1::Cancelled
        }
        LayeredFourFaceChainContinuousErrorV1::DeadlineExceeded => {
            LayeredFourFaceFailureDispositionV1::DeadlineExceeded
        }
        LayeredFourFaceChainContinuousErrorV1::ResourceLimit => {
            LayeredFourFaceFailureDispositionV1::ResourceUnavailable
        }
        LayeredFourFaceChainContinuousErrorV1::IssuerMismatch => {
            LayeredFourFaceFailureDispositionV1::BindingRejected
        }
        LayeredFourFaceChainContinuousErrorV1::UnsupportedTree
        | LayeredFourFaceChainContinuousErrorV1::InvalidAngleSchedule
        | LayeredFourFaceChainContinuousErrorV1::InitialLayerAdmissionUnavailable
        | LayeredFourFaceChainContinuousErrorV1::MovingBoundaryOnlyUnavailable
        | LayeredFourFaceChainContinuousErrorV1::StationaryLayerTransportUnavailable
        | LayeredFourFaceChainContinuousErrorV1::NonadjacentIntervalOverlap
        | LayeredFourFaceChainContinuousErrorV1::PairPartitionUnavailable => {
            LayeredFourFaceFailureDispositionV1::Uncertified
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn four_face_failure_mapper_preserves_typed_stop_and_resource_outcomes_v1() {
        let cases = [
            (
                LayeredFourFaceChainContinuousErrorV1::Cancelled,
                LayeredFourFaceFailureDispositionV1::Cancelled,
            ),
            (
                LayeredFourFaceChainContinuousErrorV1::DeadlineExceeded,
                LayeredFourFaceFailureDispositionV1::DeadlineExceeded,
            ),
            (
                LayeredFourFaceChainContinuousErrorV1::ResourceLimit,
                LayeredFourFaceFailureDispositionV1::ResourceUnavailable,
            ),
            (
                LayeredFourFaceChainContinuousErrorV1::IssuerMismatch,
                LayeredFourFaceFailureDispositionV1::BindingRejected,
            ),
            (
                LayeredFourFaceChainContinuousErrorV1::InitialLayerAdmissionUnavailable,
                LayeredFourFaceFailureDispositionV1::Uncertified,
            ),
            (
                LayeredFourFaceChainContinuousErrorV1::StationaryLayerTransportUnavailable,
                LayeredFourFaceFailureDispositionV1::Uncertified,
            ),
        ];
        for (error, expected) in cases {
            assert_eq!(
                four_face_certificate_failure_disposition_v1(error),
                expected
            );
        }
    }

    #[test]
    fn progressive_depth_family_is_finite_and_binds_exact_leaf_limits_v1() {
        let mut depths = Vec::new();
        let mut current = None;
        while let Some(depth) =
            next_four_face_depth_v1(current, MAX_LAYERED_FOUR_FACE_DYADIC_DEPTH_V1)
        {
            let limits = layered_four_face_limits_for_depth_v1(depth).expect("bounded limits");
            assert_eq!(limits.dyadic_depth, depth);
            assert_eq!(limits.max_leaves, 1_usize << depth);
            depths.push(depth);
            current = Some(depth);
        }
        assert_eq!(
            depths,
            (0..=MAX_LAYERED_FOUR_FACE_DYADIC_DEPTH_V1).collect::<Vec<_>>()
        );
        assert_eq!(
            layered_four_face_limits_for_depth_v1(MAX_LAYERED_FOUR_FACE_DYADIC_DEPTH_V1)
                .expect("maximum depth")
                .max_leaves,
            MAX_LAYERED_FOUR_FACE_LEAVES_V1
        );
        assert!(
            layered_four_face_limits_for_depth_v1(MAX_LAYERED_FOUR_FACE_DYADIC_DEPTH_V1 + 1)
                .is_none()
        );
        assert!(
            layered_four_face_limits_for_depth_and_cap_v1(
                MAX_LAYERED_FOUR_FACE_DYADIC_DEPTH_V1,
                MAX_LAYERED_FOUR_FACE_LEAVES_V1 - 1,
            )
            .is_none(),
            "one-short max_leaves must fail before native issuance"
        );
    }

    #[test]
    fn progressive_depth_retries_only_overlap_and_binds_the_successful_limits_v1() {
        let mut visited = Vec::new();
        let (limits, authority) = run_four_face_progressive_depths_with_v1(
            0,
            &CooperativeOperationControlV1::unbounded(),
            |limits| {
                visited.push(limits.dyadic_depth);
                if limits.dyadic_depth < 2 {
                    Err(LayeredFourFaceChainContinuousErrorV1::NonadjacentIntervalOverlap)
                } else {
                    Ok("issued")
                }
            },
        )
        .expect("higher-depth authority");
        assert_eq!(visited, [0, 1, 2]);
        assert_eq!(limits.dyadic_depth, 2);
        assert_eq!(limits.max_leaves, 4);
        assert_eq!(authority, "issued");

        let mut all_overlap_attempts = 0_usize;
        assert_eq!(
            run_four_face_progressive_depths_with_v1(
                0,
                &CooperativeOperationControlV1::unbounded(),
                |_| {
                    all_overlap_attempts += 1;
                    Err::<(), _>(LayeredFourFaceChainContinuousErrorV1::NonadjacentIntervalOverlap)
                },
            ),
            Err(LayeredFourFaceChainContinuousErrorV1::NonadjacentIntervalOverlap)
        );
        assert_eq!(
            all_overlap_attempts,
            usize::from(MAX_LAYERED_FOUR_FACE_DYADIC_DEPTH_V1) + 1
        );
    }

    #[test]
    fn progressive_depth_observes_cancellation_between_attempts_v1() {
        let cancelled = std::sync::atomic::AtomicBool::new(false);
        let control = CooperativeOperationControlV1::new(
            Some(&cancelled),
            std::time::Instant::now() + std::time::Duration::from_secs(1),
        );
        let mut attempts = 0_usize;
        assert_eq!(
            run_four_face_progressive_depths_with_v1(0, &control, |_| {
                attempts += 1;
                cancelled.store(true, std::sync::atomic::Ordering::Release);
                Err::<(), _>(LayeredFourFaceChainContinuousErrorV1::NonadjacentIntervalOverlap)
            }),
            Err(LayeredFourFaceChainContinuousErrorV1::Cancelled)
        );
        assert_eq!(attempts, 1);
    }

    #[test]
    fn progressive_depth_family_stops_immediately_for_cancel_and_deadline_v1() {
        let cancelled = std::sync::atomic::AtomicBool::new(true);
        assert_eq!(
            four_face_checkpoint_v1(&CooperativeOperationControlV1::new(
                Some(&cancelled),
                std::time::Instant::now() + std::time::Duration::from_secs(1),
            )),
            Err(LayeredFourFaceChainContinuousErrorV1::Cancelled)
        );
        assert_eq!(
            four_face_checkpoint_v1(&CooperativeOperationControlV1::new(
                None,
                std::time::Instant::now(),
            )),
            Err(LayeredFourFaceChainContinuousErrorV1::DeadlineExceeded)
        );
    }
}
