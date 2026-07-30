use ori_collision::{
    LayeredThreeFaceContinuousErrorV1, LayeredThreeFaceContinuousLimitsV1,
    certify_layered_three_face_continuous_path_with_control_v1,
};
use ori_core::{
    SpeculativeUnprovenFoldLayeredThreeFaceCertificationErrorV1,
    bind_speculative_unproven_layered_three_face_continuous_proof_with_control_v1,
};
use ori_kinematics::CanonicalHingeAngles;

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LayeredThreeFaceFallbackDecisionV1 {
    OrdinaryUncertified,
    LayeredAttempt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LayeredAdmissionFailureDispositionV1 {
    Cancelled,
    DeadlineExceeded,
    ResourceUnavailable,
    Uncertified,
    BindingRejected,
}

pub(super) fn is_layered_three_face_fallback_candidate_v1(
    premise: &PostApplyProofPremiseV1,
) -> bool {
    let model = premise.requested.initial().target().model();
    let source = premise.requested.initial().pose().hinge_angles();
    let target = premise.requested.pose().hinge_angles();
    layered_three_face_fallback_decision_v1(
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
    ) == LayeredThreeFaceFallbackDecisionV1::LayeredAttempt
}

pub(super) fn layered_three_face_fallback_decision_v1(
    paper_thickness_mm: f64,
    face_count: usize,
    hinge_count: usize,
    hinge_schedule: impl IntoIterator<Item = (bool, f64, f64)>,
    same_schedule_length: bool,
) -> LayeredThreeFaceFallbackDecisionV1 {
    if paper_thickness_mm.to_bits() != 0.0_f64.to_bits()
        || face_count != 3
        || hinge_count != 2
        || !same_schedule_length
    {
        return LayeredThreeFaceFallbackDecisionV1::OrdinaryUncertified;
    }
    let mut moving = 0;
    let mut stationary_flat = 0;
    let mut schedule_count = 0;
    for (same_edge, source_angle, target_angle) in hinge_schedule {
        schedule_count += 1;
        if !same_edge {
            return LayeredThreeFaceFallbackDecisionV1::OrdinaryUncertified;
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
    if schedule_count == 2 && moving == 1 && stationary_flat == 1 {
        LayeredThreeFaceFallbackDecisionV1::LayeredAttempt
    } else {
        LayeredThreeFaceFallbackDecisionV1::OrdinaryUncertified
    }
}

pub(super) fn run_layered_three_face_fallback_v1(
    premise: PostApplyProofPremiseV1,
    control: &CooperativeOperationControlV1<'_>,
) -> PostApplyProofWorkerCertificateV1 {
    let limits = LayeredThreeFaceContinuousLimitsV1::default();
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
        limits.static_limits,
        &premise.initial_layer_order,
        control,
    ) {
        Ok(admission) => admission,
        Err(error) => return admission_error_to_worker_v1(premise, error),
    };
    if let Some(terminal) = cooperative_stop_terminal_v1(control) {
        return stopped_worker_certificate_v1(premise, terminal);
    }
    let certificate = match certify_layered_three_face_continuous_path_with_control_v1(
        initial.target().model(),
        initial.pose(),
        &target_angles,
        &admission,
        limits,
        control,
    ) {
        Ok(certificate) => certificate,
        Err(error) => return layered_certificate_error_to_worker_v1(premise, error),
    };
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
    match bind_speculative_unproven_layered_three_face_continuous_proof_with_control_v1(
        resolution_ticket,
        &requested,
        &admission,
        limits,
        certificate,
        control,
    ) {
        Ok(proof) => PostApplyProofWorkerCertificateV1::Certified(
            PostApplyProofCertifiedAuthorityV1::LayeredThreeFace(proof),
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
                SpeculativeUnprovenFoldLayeredThreeFaceCertificationErrorV1::Cancelled => {
                    PostApplyProofWorkerCertificateV1::Cancelled(premise)
                }
                SpeculativeUnprovenFoldLayeredThreeFaceCertificationErrorV1::DeadlineExceeded => {
                    PostApplyProofWorkerCertificateV1::DeadlineExceeded(premise)
                }
                SpeculativeUnprovenFoldLayeredThreeFaceCertificationErrorV1::Common(
                    SpeculativeUnprovenFoldCertificationErrorV1::RequestedTargetAngleAllocationFailed
                    | SpeculativeUnprovenFoldCertificationErrorV1::ValidationPanicked
                    | SpeculativeUnprovenFoldCertificationErrorV1::ResourceUnavailable,
                )
                | SpeculativeUnprovenFoldLayeredThreeFaceCertificationErrorV1::ResourceUnavailable => {
                    PostApplyProofWorkerCertificateV1::ResourceUnavailable(premise)
                }
                _ => PostApplyProofWorkerCertificateV1::BindingRejected(premise),
            }
        }
    }
}

pub(super) fn target_angles_for_premise_v1(
    premise: &PostApplyProofPremiseV1,
    control: &CooperativeOperationControlV1<'_>,
) -> Result<CanonicalHingeAngles, PostApplyProofDirectCertificateErrorV1> {
    cooperative_path_checkpoint_v1(control).map_err(classify_direct_certificate_error_v1)?;
    let requested_angles = premise.requested.pose().hinge_angles();
    let mut target_angle_entries = Vec::new();
    target_angle_entries
        .try_reserve_exact(requested_angles.len())
        .map_err(|_| PostApplyProofDirectCertificateErrorV1::ResourceUnavailable)?;
    target_angle_entries.extend_from_slice(requested_angles);
    CanonicalHingeAngles::new(target_angle_entries)
        .map_err(|_| PostApplyProofDirectCertificateErrorV1::BindingRejected)
}

pub(super) fn direct_certificate_error_to_worker_v1(
    premise: PostApplyProofPremiseV1,
    error: PostApplyProofDirectCertificateErrorV1,
) -> PostApplyProofWorkerCertificateV1 {
    match error {
        PostApplyProofDirectCertificateErrorV1::BindingRejected => {
            PostApplyProofWorkerCertificateV1::BindingRejected(premise)
        }
        PostApplyProofDirectCertificateErrorV1::ResourceUnavailable => {
            PostApplyProofWorkerCertificateV1::ResourceUnavailable(premise)
        }
        PostApplyProofDirectCertificateErrorV1::Cancelled => {
            PostApplyProofWorkerCertificateV1::Cancelled(premise)
        }
        PostApplyProofDirectCertificateErrorV1::DeadlineExceeded => {
            PostApplyProofWorkerCertificateV1::DeadlineExceeded(premise)
        }
    }
}

pub(super) fn admission_error_to_worker_v1(
    premise: PostApplyProofPremiseV1,
    error: StackedFoldPathDiagnosticErrorV1,
) -> PostApplyProofWorkerCertificateV1 {
    match layered_admission_failure_disposition_v1(error) {
        LayeredAdmissionFailureDispositionV1::Cancelled => {
            PostApplyProofWorkerCertificateV1::Cancelled(premise)
        }
        LayeredAdmissionFailureDispositionV1::DeadlineExceeded => {
            PostApplyProofWorkerCertificateV1::DeadlineExceeded(premise)
        }
        LayeredAdmissionFailureDispositionV1::ResourceUnavailable => {
            PostApplyProofWorkerCertificateV1::ResourceUnavailable(premise)
        }
        LayeredAdmissionFailureDispositionV1::Uncertified => {
            PostApplyProofWorkerCertificateV1::Uncertified(premise)
        }
        LayeredAdmissionFailureDispositionV1::BindingRejected => {
            PostApplyProofWorkerCertificateV1::BindingRejected(premise)
        }
    }
}

fn layered_admission_failure_disposition_v1(
    error: StackedFoldPathDiagnosticErrorV1,
) -> LayeredAdmissionFailureDispositionV1 {
    match error {
        StackedFoldPathDiagnosticErrorV1::Cancelled => {
            LayeredAdmissionFailureDispositionV1::Cancelled
        }
        StackedFoldPathDiagnosticErrorV1::DeadlineExceeded => {
            LayeredAdmissionFailureDispositionV1::DeadlineExceeded
        }
        // This is the admission API's explicit bounded-work/allocation
        // outcome. It alone warrants the existing resource retry path.
        StackedFoldPathDiagnosticErrorV1::InitialLayerOrderResourceLimit => {
            LayeredAdmissionFailureDispositionV1::ResourceUnavailable
        }
        StackedFoldPathDiagnosticErrorV1::InvalidLimits
        | StackedFoldPathDiagnosticErrorV1::InvalidPath
        | StackedFoldPathDiagnosticErrorV1::PoseIssuerMismatch => {
            LayeredAdmissionFailureDispositionV1::BindingRejected
        }
        // These are fail-closed evidence failures, not proof that a bounded
        // resource was exhausted. Retain the ticket for the normal next
        // stage, then resolve ordinary insufficient evidence if none issues.
        StackedFoldPathDiagnosticErrorV1::PoseUnavailable
        | StackedFoldPathDiagnosticErrorV1::StaticDiagnosisUnavailable
        | StackedFoldPathDiagnosticErrorV1::ProofCacheUnavailable
        | StackedFoldPathDiagnosticErrorV1::StaleProofCacheResult
        | StackedFoldPathDiagnosticErrorV1::InitialLayerOrderUnavailable => {
            LayeredAdmissionFailureDispositionV1::Uncertified
        }
    }
}

pub(super) fn stopped_worker_certificate_v1(
    premise: PostApplyProofPremiseV1,
    terminal: PostApplyProofTerminalV1,
) -> PostApplyProofWorkerCertificateV1 {
    match terminal {
        PostApplyProofTerminalV1::UnknownCancelled => {
            PostApplyProofWorkerCertificateV1::Cancelled(premise)
        }
        PostApplyProofTerminalV1::UnknownDeadlineReached => {
            PostApplyProofWorkerCertificateV1::DeadlineExceeded(premise)
        }
        _ => unreachable!("cooperative control only stops as cancelled or deadline"),
    }
}

fn layered_certificate_error_to_worker_v1(
    premise: PostApplyProofPremiseV1,
    error: LayeredThreeFaceContinuousErrorV1,
) -> PostApplyProofWorkerCertificateV1 {
    match layered_certificate_failure_disposition_v1(error) {
        LayeredAdmissionFailureDispositionV1::Cancelled => {
            PostApplyProofWorkerCertificateV1::Cancelled(premise)
        }
        LayeredAdmissionFailureDispositionV1::DeadlineExceeded => {
            PostApplyProofWorkerCertificateV1::DeadlineExceeded(premise)
        }
        LayeredAdmissionFailureDispositionV1::ResourceUnavailable => {
            PostApplyProofWorkerCertificateV1::ResourceUnavailable(premise)
        }
        LayeredAdmissionFailureDispositionV1::Uncertified => {
            PostApplyProofWorkerCertificateV1::Uncertified(premise)
        }
        LayeredAdmissionFailureDispositionV1::BindingRejected => {
            PostApplyProofWorkerCertificateV1::BindingRejected(premise)
        }
    }
}

fn layered_certificate_failure_disposition_v1(
    error: LayeredThreeFaceContinuousErrorV1,
) -> LayeredAdmissionFailureDispositionV1 {
    match error {
        LayeredThreeFaceContinuousErrorV1::Cancelled => {
            LayeredAdmissionFailureDispositionV1::Cancelled
        }
        LayeredThreeFaceContinuousErrorV1::DeadlineExceeded => {
            LayeredAdmissionFailureDispositionV1::DeadlineExceeded
        }
        LayeredThreeFaceContinuousErrorV1::ResourceLimit => {
            LayeredAdmissionFailureDispositionV1::ResourceUnavailable
        }
        LayeredThreeFaceContinuousErrorV1::UnsupportedTree
        | LayeredThreeFaceContinuousErrorV1::InvalidAngleSchedule
        | LayeredThreeFaceContinuousErrorV1::InitialLayerAdmissionUnavailable
        | LayeredThreeFaceContinuousErrorV1::MovingBoundaryOnlyUnavailable
        | LayeredThreeFaceContinuousErrorV1::NonadjacentIntervalOverlap
        | LayeredThreeFaceContinuousErrorV1::PairPartitionUnavailable
        | LayeredThreeFaceContinuousErrorV1::IssuerMismatch => {
            LayeredAdmissionFailureDispositionV1::Uncertified
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admission_failure_mapper_retries_only_explicit_resource_exhaustion_v1() {
        let cases = [
            (
                StackedFoldPathDiagnosticErrorV1::Cancelled,
                LayeredAdmissionFailureDispositionV1::Cancelled,
            ),
            (
                StackedFoldPathDiagnosticErrorV1::DeadlineExceeded,
                LayeredAdmissionFailureDispositionV1::DeadlineExceeded,
            ),
            (
                StackedFoldPathDiagnosticErrorV1::InitialLayerOrderResourceLimit,
                LayeredAdmissionFailureDispositionV1::ResourceUnavailable,
            ),
            (
                StackedFoldPathDiagnosticErrorV1::InitialLayerOrderUnavailable,
                LayeredAdmissionFailureDispositionV1::Uncertified,
            ),
            (
                StackedFoldPathDiagnosticErrorV1::InvalidPath,
                LayeredAdmissionFailureDispositionV1::BindingRejected,
            ),
            (
                StackedFoldPathDiagnosticErrorV1::PoseIssuerMismatch,
                LayeredAdmissionFailureDispositionV1::BindingRejected,
            ),
            (
                StackedFoldPathDiagnosticErrorV1::StaticDiagnosisUnavailable,
                LayeredAdmissionFailureDispositionV1::Uncertified,
            ),
        ];
        for (error, expected) in cases {
            assert_eq!(layered_admission_failure_disposition_v1(error), expected);
        }
    }

    #[test]
    fn layered_admission_evidence_mismatch_is_not_a_resource_retry_v1() {
        assert_eq!(
            layered_admission_failure_disposition_v1(
                StackedFoldPathDiagnosticErrorV1::InitialLayerOrderUnavailable
            ),
            LayeredAdmissionFailureDispositionV1::Uncertified
        );
        assert_ne!(
            layered_admission_failure_disposition_v1(
                StackedFoldPathDiagnosticErrorV1::InitialLayerOrderUnavailable
            ),
            LayeredAdmissionFailureDispositionV1::ResourceUnavailable
        );
    }

    #[test]
    fn layered_certificate_admission_mismatch_is_uncertified_v1() {
        assert_eq!(
            layered_certificate_failure_disposition_v1(
                LayeredThreeFaceContinuousErrorV1::InitialLayerAdmissionUnavailable
            ),
            LayeredAdmissionFailureDispositionV1::Uncertified
        );
    }
}
