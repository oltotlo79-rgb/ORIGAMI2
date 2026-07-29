use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::Arc,
};

use ori_collision::StackedFoldTreeContinuousCertificateV1;
use ori_kinematics::CanonicalHingeAngles;
use thiserror::Error;

use crate::{
    APPLIED_POSE_MODEL_ID_V1, AppliedPoseV1, MAX_REVISION,
    stacked_fold::PreparedStackedFoldRequestedPoseV1,
};

use super::{
    super::Revision, SpeculativeApproximateBlockingObservationV1, SpeculativeUnprovenFoldBindingV1,
    SpeculativeUnprovenFoldMetadataErrorV1,
};

/// Runtime-only one-shot authority to bind a post-Apply continuous proof.
///
/// A ticket is minted only by the atomic speculative Apply path. It cannot be
/// cloned or persisted, and binding it consumes both the ticket and the native
/// certificate.
///
/// ```compile_fail
/// fn requires_clone<T: Clone>() {}
/// requires_clone::<ori_core::SpeculativeUnprovenFoldResolutionTicketV1>();
/// ```
///
/// ```compile_fail
/// fn requires_serialize<T: serde::Serialize>() {}
/// requires_serialize::<ori_core::SpeculativeUnprovenFoldResolutionTicketV1>();
/// ```
#[derive(Debug)]
pub struct SpeculativeUnprovenFoldResolutionTicketV1 {
    editor_instance_anchor: Arc<()>,
    binding: SpeculativeUnprovenFoldBindingV1,
    target_revision: Revision,
    target_geometry_fingerprint: [u8; 32],
    target_applied_pose: AppliedPoseV1,
}

impl SpeculativeUnprovenFoldResolutionTicketV1 {
    pub(super) fn new(
        editor_instance_anchor: Arc<()>,
        binding: SpeculativeUnprovenFoldBindingV1,
        target_revision: Revision,
        target_geometry_fingerprint: [u8; 32],
        target_applied_pose: AppliedPoseV1,
    ) -> Self {
        Self {
            editor_instance_anchor,
            binding,
            target_revision,
            target_geometry_fingerprint,
            target_applied_pose,
        }
    }
}

/// Opaque one-shot certification authority for one exact speculative mark.
///
/// The only production constructor consumes a resolution ticket and a native
/// continuous-path certificate after rebinding both to the same prepared
/// request. Resolution consumes this value and removes only the matching
/// unproven mark.
///
/// ```compile_fail
/// fn requires_clone<T: Clone>() {}
/// requires_clone::<ori_core::SpeculativeUnprovenFoldCertifiedProofV1>();
/// ```
///
/// ```compile_fail
/// fn requires_serialize<T: serde::Serialize>() {}
/// requires_serialize::<ori_core::SpeculativeUnprovenFoldCertifiedProofV1>();
/// ```
///
/// ```compile_fail
/// fn consume(_: ori_core::SpeculativeUnprovenFoldCertifiedProofV1) {}
/// fn use_twice(proof: ori_core::SpeculativeUnprovenFoldCertifiedProofV1) {
///     consume(proof);
///     consume(proof);
/// }
/// ```
#[derive(Debug)]
pub struct SpeculativeUnprovenFoldCertifiedProofV1 {
    editor_instance_anchor: Arc<()>,
    binding: SpeculativeUnprovenFoldBindingV1,
    target_revision: Revision,
    target_geometry_fingerprint: [u8; 32],
    target_applied_pose: AppliedPoseV1,
}

impl SpeculativeUnprovenFoldCertifiedProofV1 {
    pub(super) fn into_resolution_parts(
        self,
    ) -> (
        Arc<()>,
        SpeculativeUnprovenFoldBindingV1,
        Revision,
        [u8; 32],
        AppliedPoseV1,
    ) {
        (
            self.editor_instance_anchor,
            self.binding,
            self.target_revision,
            self.target_geometry_fingerprint,
            self.target_applied_pose,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SpeculativeUnprovenFoldCertificationErrorV1 {
    #[error(transparent)]
    InvalidMetadata(#[from] SpeculativeUnprovenFoldMetadataErrorV1),
    #[error("the resolution ticket source revision does not match the prepared request")]
    SourceRevisionMismatch,
    #[error("the resolution ticket target revision does not match the prepared request")]
    TargetRevisionMismatch,
    #[error("the resolution ticket project does not match the prepared request lineage")]
    ProjectLineageMismatch,
    #[error("the resolution ticket source geometry fingerprint does not match the lineage")]
    SourceGeometryFingerprintMismatch,
    #[error("the resolution ticket target geometry fingerprint does not match the lineage")]
    TargetGeometryFingerprintMismatch,
    #[error("the resolution ticket paper-thickness bits do not match the prepared target")]
    PaperThicknessBitsMismatch,
    #[error("the resolution ticket contains a blocking approximate observation")]
    ApproximateBlockingObservationMismatch,
    #[error("the prepared source or target pose is not owned by its target model")]
    RequestedPoseIssuerMismatch,
    #[error("the resolution ticket target semantic pose does not match the prepared target pose")]
    TargetAppliedPoseMismatch,
    #[error("memory for the prepared target hinge-angle vector could not be reserved")]
    RequestedTargetAngleAllocationFailed,
    #[error("the prepared target hinge angles are not canonical")]
    InvalidRequestedTargetAngles,
    #[error("the native continuous certificate does not certify the exact requested path")]
    ContinuousCertificateMismatch,
    #[error("the native certification validation boundary panicked")]
    ValidationPanicked,
}

/// Recoverable failure to bind one resolution ticket to a native certificate.
///
/// The exact validation error can be inspected without releasing either
/// one-shot input. A caller can recover both inputs with [`Self::into_parts`]
/// or recover just the resolution ticket with [`Self::into_ticket`].
///
/// This failure is deliberately neither cloneable nor persistable:
///
/// ```compile_fail
/// fn requires_clone<T: Clone>() {}
/// requires_clone::<ori_core::SpeculativeUnprovenFoldCertificationFailureV1>();
/// ```
///
/// ```compile_fail
/// fn requires_serialize<T: serde::Serialize>() {}
/// requires_serialize::<ori_core::SpeculativeUnprovenFoldCertificationFailureV1>();
/// ```
#[derive(Debug, Error)]
#[error("{error}")]
pub struct SpeculativeUnprovenFoldCertificationFailureV1 {
    error: SpeculativeUnprovenFoldCertificationErrorV1,
    ticket: SpeculativeUnprovenFoldResolutionTicketV1,
    certificate: StackedFoldTreeContinuousCertificateV1,
}

impl SpeculativeUnprovenFoldCertificationFailureV1 {
    #[must_use]
    pub const fn error(&self) -> &SpeculativeUnprovenFoldCertificationErrorV1 {
        &self.error
    }

    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        SpeculativeUnprovenFoldCertificationErrorV1,
        SpeculativeUnprovenFoldResolutionTicketV1,
        StackedFoldTreeContinuousCertificateV1,
    ) {
        (self.error, self.ticket, self.certificate)
    }

    #[must_use]
    pub fn into_ticket(self) -> SpeculativeUnprovenFoldResolutionTicketV1 {
        self.ticket
    }
}

/// Binds an exact post-Apply ticket to an independently completed native Tree
/// continuous-path certificate.
///
/// Every persisted/runtime identity is checked before `certificate.is_for`
/// reruns the native proof against the prepared target model, its source pose,
/// the exact requested target angles, and the exact paper thickness. This
/// boundary fallibly reserves its copied angle vector; allocations retained
/// inside the independently issued collision certificate follow that
/// certificate's own bounded diagnostic contract. Validation borrows both
/// one-shot inputs behind an internal unwind boundary. Only successful
/// validation consumes the ticket; every ordinary validation error or
/// catchable panic returns the original ticket and certificate in
/// [`SpeculativeUnprovenFoldCertificationFailureV1`].
// The large error is intentional: it returns both one-shot inputs without a
// new allocation on the failure path. Boxing would make OOM recovery itself
// require another allocation and could destroy the retry authority.
#[allow(clippy::result_large_err)]
pub fn bind_speculative_unproven_tree_continuous_proof_v1(
    ticket: SpeculativeUnprovenFoldResolutionTicketV1,
    requested: &PreparedStackedFoldRequestedPoseV1,
    certificate: StackedFoldTreeContinuousCertificateV1,
) -> Result<SpeculativeUnprovenFoldCertifiedProofV1, SpeculativeUnprovenFoldCertificationFailureV1>
{
    let validation = catch_unwind(AssertUnwindSafe(|| {
        validate_speculative_unproven_tree_continuous_proof_v1(&ticket, requested, &certificate)
    }));
    let error = match validation {
        Ok(Ok(())) => None,
        Ok(Err(error)) => Some(error),
        Err(_) => Some(SpeculativeUnprovenFoldCertificationErrorV1::ValidationPanicked),
    };
    if let Some(error) = error {
        return Err(SpeculativeUnprovenFoldCertificationFailureV1 {
            error,
            ticket,
            certificate,
        });
    }

    let SpeculativeUnprovenFoldResolutionTicketV1 {
        editor_instance_anchor,
        binding,
        target_revision,
        target_geometry_fingerprint,
        target_applied_pose,
    } = ticket;
    Ok(SpeculativeUnprovenFoldCertifiedProofV1 {
        editor_instance_anchor,
        binding,
        target_revision,
        target_geometry_fingerprint,
        target_applied_pose,
    })
}

fn validate_speculative_unproven_tree_continuous_proof_v1(
    ticket: &SpeculativeUnprovenFoldResolutionTicketV1,
    requested: &PreparedStackedFoldRequestedPoseV1,
    certificate: &StackedFoldTreeContinuousCertificateV1,
) -> Result<(), SpeculativeUnprovenFoldCertificationErrorV1> {
    let SpeculativeUnprovenFoldResolutionTicketV1 {
        binding,
        target_revision,
        target_geometry_fingerprint,
        target_applied_pose,
        ..
    } = ticket;
    binding.validate()?;
    let initial = requested.initial();
    let target = initial.target();
    let lineage = target.geometry().proof().lineage();
    let candidate = target.geometry().candidate();
    if binding.source_revision() != lineage.source_revision() {
        return Err(SpeculativeUnprovenFoldCertificationErrorV1::SourceRevisionMismatch);
    }
    let expected_target_revision = binding
        .source_revision()
        .checked_add(1)
        .filter(|revision| *revision <= MAX_REVISION)
        .ok_or(SpeculativeUnprovenFoldCertificationErrorV1::TargetRevisionMismatch)?;
    if *target_revision != expected_target_revision
        || lineage.target_revision() != expected_target_revision
    {
        return Err(SpeculativeUnprovenFoldCertificationErrorV1::TargetRevisionMismatch);
    }
    if binding.project_id() != lineage.identity_namespace() {
        return Err(SpeculativeUnprovenFoldCertificationErrorV1::ProjectLineageMismatch);
    }
    if !lowercase_sha256_matches_v1(
        lineage.source_fingerprint().0,
        binding.source_geometry_fingerprint_sha256(),
    ) {
        return Err(SpeculativeUnprovenFoldCertificationErrorV1::SourceGeometryFingerprintMismatch);
    }
    if *target_geometry_fingerprint != lineage.target_fingerprint().0 {
        return Err(SpeculativeUnprovenFoldCertificationErrorV1::TargetGeometryFingerprintMismatch);
    }
    if binding.paper_thickness_bits() != candidate.paper.thickness_mm.to_bits() {
        return Err(SpeculativeUnprovenFoldCertificationErrorV1::PaperThicknessBitsMismatch);
    }
    if !matches!(
        binding.approximate_blocking_observation(),
        SpeculativeApproximateBlockingObservationV1::NoBlockingSampleObserved
    ) {
        return Err(
            SpeculativeUnprovenFoldCertificationErrorV1::ApproximateBlockingObservationMismatch,
        );
    }
    if !target.model().owns_pose(initial.pose()) || !target.model().owns_pose(requested.pose()) {
        return Err(SpeculativeUnprovenFoldCertificationErrorV1::RequestedPoseIssuerMismatch);
    }
    if !target_applied_pose_matches_requested_v1(target_applied_pose, requested) {
        return Err(SpeculativeUnprovenFoldCertificationErrorV1::TargetAppliedPoseMismatch);
    }

    if requested_target_angle_allocation_failure_is_forced_v1() {
        return Err(
            SpeculativeUnprovenFoldCertificationErrorV1::RequestedTargetAngleAllocationFailed,
        );
    }
    let mut requested_angle_records = Vec::new();
    requested_angle_records
        .try_reserve_exact(requested.pose().hinge_angles().len())
        .map_err(|_| {
            SpeculativeUnprovenFoldCertificationErrorV1::RequestedTargetAngleAllocationFailed
        })?;
    requested_angle_records.extend_from_slice(requested.pose().hinge_angles());
    let requested_angles = CanonicalHingeAngles::new(requested_angle_records)
        .map_err(|_| SpeculativeUnprovenFoldCertificationErrorV1::InvalidRequestedTargetAngles)?;
    let paper_thickness_mm = f64::from_bits(binding.paper_thickness_bits());
    panic_before_native_revalidation_if_forced_v1();
    if !certificate.is_for(
        target.model(),
        initial.pose(),
        &requested_angles,
        paper_thickness_mm,
    ) {
        return Err(SpeculativeUnprovenFoldCertificationErrorV1::ContinuousCertificateMismatch);
    }

    Ok(())
}

#[cfg(test)]
fn requested_target_angle_allocation_failure_is_forced_v1() -> bool {
    CERTIFICATION_TEST_FAULT_V1
        .with(|fault| fault.get() == Some(CertificationTestFaultV1::TargetAngleAllocation))
}

#[cfg(not(test))]
const fn requested_target_angle_allocation_failure_is_forced_v1() -> bool {
    false
}

#[cfg(test)]
fn panic_before_native_revalidation_if_forced_v1() {
    CERTIFICATION_TEST_FAULT_V1.with(|fault| {
        if fault.get() == Some(CertificationTestFaultV1::NativeRevalidationPanic) {
            panic!("injected native certification revalidation panic");
        }
    });
}

#[cfg(not(test))]
const fn panic_before_native_revalidation_if_forced_v1() {}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CertificationTestFaultV1 {
    TargetAngleAllocation,
    NativeRevalidationPanic,
}

#[cfg(test)]
thread_local! {
    static CERTIFICATION_TEST_FAULT_V1: std::cell::Cell<Option<CertificationTestFaultV1>> =
        const { std::cell::Cell::new(None) };
}

fn lowercase_sha256_matches_v1(bytes: [u8; 32], encoded: &str) -> bool {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    encoded.len() == 64
        && bytes.iter().enumerate().all(|(index, byte)| {
            encoded.as_bytes()[index * 2] == DIGITS[usize::from(byte >> 4)]
                && encoded.as_bytes()[index * 2 + 1] == DIGITS[usize::from(byte & 0x0f)]
        })
}

fn target_applied_pose_matches_requested_v1(
    applied: &AppliedPoseV1,
    requested: &PreparedStackedFoldRequestedPoseV1,
) -> bool {
    let native = requested.pose();
    applied.model_id() == APPLIED_POSE_MODEL_ID_V1
        && applied.face_ids() == native.face_ids()
        && applied.fixed_face() == native.fixed_face()
        && applied.hinge_angles().len() == native.hinge_angles().len()
        && applied
            .hinge_angles()
            .iter()
            .zip(native.hinge_angles())
            .all(|(semantic, native)| {
                semantic.edge() == native.edge()
                    && semantic.angle_degrees().to_bits() == native.angle_degrees().to_bits()
            })
}

#[cfg(test)]
pub(crate) fn bind_resolution_ticket_for_test_v1(
    ticket: SpeculativeUnprovenFoldResolutionTicketV1,
) -> SpeculativeUnprovenFoldCertifiedProofV1 {
    let SpeculativeUnprovenFoldResolutionTicketV1 {
        editor_instance_anchor,
        binding,
        target_revision,
        target_geometry_fingerprint,
        target_applied_pose,
    } = ticket;
    SpeculativeUnprovenFoldCertifiedProofV1 {
        editor_instance_anchor,
        binding,
        target_revision,
        target_geometry_fingerprint,
        target_applied_pose,
    }
}

#[cfg(test)]
mod tests {
    use ori_collision::{
        StackedFoldPathDiagnosticLimitsV1, certify_tree_continuous_path_from_pose_v1,
    };
    use ori_domain::{EdgeKind, ProjectId};
    use ori_foldability::{
        GlobalFlatFoldabilityInput, GlobalFlatFoldabilityLimits, analyze_global_flat_foldability,
    };
    use ori_kinematics::{
        CanonicalHingeAngles, HingeAngle, MaterialTreeKinematicsModel, TreeKinematicsLimits,
    };
    use ori_topology::{FaceExtractionInput, analyze_faces, analyze_local_flat_foldability};

    use crate::{
        AppliedPoseLimitsV1, FaceLineageLimits, PreparedStackedFoldRequestedPoseV1,
        SpeculativeApproximateBlockingObservationV1, StackedFoldGeometryLimitsV1,
        StackedFoldTopologyBuildLimitsV1, create_rectangular_sheet, prepare_applied_pose_v1,
        stacked_fold::{
            ExpectedStackedFoldCreaseV1, prepare_stacked_fold_geometry_candidate_v1,
            prepare_stacked_fold_initial_pose_v1, prepare_stacked_fold_requested_pose_v1,
            prepare_stacked_fold_target_model_v1,
        },
    };

    use super::*;

    struct CertificationFixture {
        ticket: SpeculativeUnprovenFoldResolutionTicketV1,
        requested: PreparedStackedFoldRequestedPoseV1,
        certificate: StackedFoldTreeContinuousCertificateV1,
    }

    #[derive(Debug, Clone, PartialEq)]
    struct ResolutionTicketSnapshotV1 {
        editor_instance_anchor: *const (),
        binding: SpeculativeUnprovenFoldBindingV1,
        target_revision: Revision,
        target_geometry_fingerprint: [u8; 32],
        target_applied_pose: AppliedPoseV1,
    }

    impl ResolutionTicketSnapshotV1 {
        fn capture(ticket: &SpeculativeUnprovenFoldResolutionTicketV1) -> Self {
            Self {
                editor_instance_anchor: Arc::as_ptr(&ticket.editor_instance_anchor),
                binding: ticket.binding.clone(),
                target_revision: ticket.target_revision,
                target_geometry_fingerprint: ticket.target_geometry_fingerprint,
                target_applied_pose: ticket.target_applied_pose.clone(),
            }
        }
    }

    struct CertificationTestFaultGuardV1;

    impl CertificationTestFaultGuardV1 {
        fn set(fault: CertificationTestFaultV1) -> Self {
            CERTIFICATION_TEST_FAULT_V1.with(|active| {
                assert_eq!(active.replace(Some(fault)), None);
            });
            Self
        }
    }

    impl Drop for CertificationTestFaultGuardV1 {
        fn drop(&mut self) {
            CERTIFICATION_TEST_FAULT_V1.with(|active| active.set(None));
        }
    }

    fn assert_recoverable_failure_v1(
        result: Result<
            SpeculativeUnprovenFoldCertifiedProofV1,
            SpeculativeUnprovenFoldCertificationFailureV1,
        >,
        expected_error: SpeculativeUnprovenFoldCertificationErrorV1,
        expected_ticket: &ResolutionTicketSnapshotV1,
    ) -> (
        SpeculativeUnprovenFoldResolutionTicketV1,
        StackedFoldTreeContinuousCertificateV1,
    ) {
        let failure = result.expect_err("certification binding must fail closed");
        assert_eq!(failure.error(), &expected_error);
        let (error, ticket, certificate) = failure.into_parts();
        assert_eq!(error, expected_error);
        let returned_ticket = ResolutionTicketSnapshotV1::capture(&ticket);
        assert_eq!(&returned_ticket, expected_ticket);
        (ticket, certificate)
    }

    fn certification_fixture(angle_degrees: f64) -> CertificationFixture {
        let identity = ProjectId::new();
        let source_revision = 0;
        let sheet = create_rectangular_sheet(80.0, 60.0, false).expect("rectangular sheet");
        let (source_pattern, mut source_paper) = sheet.into_parts();
        // This core fixture has no explicit positive-thickness relief. Its
        // post-Apply binder contract is exercised at exact zero thickness;
        // ori-collision separately covers its relief-aware 0.1 mm issuer.
        source_paper.thickness_mm = 0.0;
        let source_topology = analyze_faces(FaceExtractionInput {
            identity_namespace: identity,
            source_revision,
            paper: &source_paper,
            pattern: &source_pattern,
        })
        .snapshot
        .expect("source topology");
        let local = analyze_local_flat_foldability(&source_paper, &source_pattern);
        let global = analyze_global_flat_foldability(
            GlobalFlatFoldabilityInput::current_with_geometry(
                identity,
                &source_paper,
                &source_pattern,
                &source_topology,
                &local,
            ),
            GlobalFlatFoldabilityLimits::default(),
        )
        .expect("global source proof");
        let source_layer_order = global.layer_order().expect("source layer order").clone();
        let start = source_pattern
            .vertices
            .iter()
            .find(|vertex| vertex.id == source_paper.boundary_vertices[0])
            .expect("first corner")
            .position;
        let end = source_pattern
            .vertices
            .iter()
            .find(|vertex| vertex.id == source_paper.boundary_vertices[2])
            .expect("opposite corner")
            .position;
        let geometry = prepare_stacked_fold_geometry_candidate_v1(
            identity,
            source_revision,
            &source_pattern,
            &source_paper,
            &source_layer_order,
            &[ExpectedStackedFoldCreaseV1 {
                start,
                end,
                kind: EdgeKind::Mountain,
            }],
            StackedFoldTopologyBuildLimitsV1::default(),
            FaceLineageLimits::default(),
            StackedFoldGeometryLimitsV1::default(),
        )
        .expect("prepared target geometry");
        let target =
            prepare_stacked_fold_target_model_v1(geometry, TreeKinematicsLimits::default())
                .expect("target tree model");
        let source_model = MaterialTreeKinematicsModel::prepare(
            &source_pattern,
            &source_paper,
            &source_topology,
            TreeKinematicsLimits::default(),
        )
        .expect("source tree model");
        let source_pose = source_model
            .solve(
                None,
                &CanonicalHingeAngles::new(Vec::new()).expect("empty source angles"),
            )
            .expect("source pose");
        let initial = prepare_stacked_fold_initial_pose_v1(target, &source_model, &source_pose)
            .expect("lift source pose");
        let requested = prepare_stacked_fold_requested_pose_v1(initial, angle_degrees)
            .expect("requested target pose");
        let requested_angles = CanonicalHingeAngles::new(requested.pose().hinge_angles().to_vec())
            .expect("canonical requested angles");
        let certificate = certify_tree_continuous_path_from_pose_v1(
            requested.initial().target().model(),
            requested.initial().pose(),
            &requested_angles,
            source_paper.thickness_mm,
            StackedFoldPathDiagnosticLimitsV1::default(),
        )
        .expect("continuous diagnosis")
        .expect("simple one-hinge path is continuously certified");

        let lineage = requested.initial().target().geometry().proof().lineage();
        let binding = SpeculativeUnprovenFoldBindingV1::new(
            ProjectId::new(),
            identity,
            source_revision,
            lineage.source_fingerprint().to_hex(),
            1,
            ProjectId::new(),
            source_paper.thickness_mm,
            SpeculativeApproximateBlockingObservationV1::no_blocking_sample_observed(),
        )
        .expect("binding");
        let hinge_ids = requested
            .pose()
            .hinges()
            .iter()
            .map(|hinge| hinge.edge())
            .collect::<Vec<_>>();
        let hinge_angles = requested
            .pose()
            .hinge_angles()
            .iter()
            .map(|angle| (angle.edge(), angle.angle_degrees()))
            .collect::<Vec<_>>();
        let target_applied_pose = prepare_applied_pose_v1(
            requested.pose().face_ids(),
            &hinge_ids,
            requested.pose().fixed_face(),
            &hinge_angles,
            AppliedPoseLimitsV1::default(),
        )
        .expect("target semantic pose");
        let ticket = SpeculativeUnprovenFoldResolutionTicketV1::new(
            Arc::new(()),
            binding,
            lineage.target_revision(),
            lineage.target_fingerprint().0,
            target_applied_pose,
        );
        CertificationFixture {
            ticket,
            requested,
            certificate,
        }
    }

    #[test]
    fn exact_ticket_request_and_certificate_mint_one_typed_proof() {
        let fixture = certification_fixture(37.0);
        bind_speculative_unproven_tree_continuous_proof_v1(
            fixture.ticket,
            &fixture.requested,
            fixture.certificate,
        )
        .expect("exact post-Apply certification binding");
    }

    #[test]
    fn requested_angle_allocation_failure_returns_unchanged_inputs_for_retry() {
        let CertificationFixture {
            ticket,
            requested,
            certificate,
        } = certification_fixture(37.0);
        let expected_ticket = ResolutionTicketSnapshotV1::capture(&ticket);
        let result = {
            let _fault =
                CertificationTestFaultGuardV1::set(CertificationTestFaultV1::TargetAngleAllocation);
            bind_speculative_unproven_tree_continuous_proof_v1(ticket, &requested, certificate)
        };
        let (ticket, certificate) = assert_recoverable_failure_v1(
            result,
            SpeculativeUnprovenFoldCertificationErrorV1::RequestedTargetAngleAllocationFailed,
            &expected_ticket,
        );
        bind_speculative_unproven_tree_continuous_proof_v1(ticket, &requested, certificate)
            .expect("the exact returned ticket and certificate remain retryable");
    }

    #[test]
    fn validation_panic_is_caught_before_one_shot_inputs_are_consumed() {
        let CertificationFixture {
            ticket,
            requested,
            certificate,
        } = certification_fixture(37.0);
        let expected_ticket = ResolutionTicketSnapshotV1::capture(&ticket);
        let result = {
            let _fault = CertificationTestFaultGuardV1::set(
                CertificationTestFaultV1::NativeRevalidationPanic,
            );
            bind_speculative_unproven_tree_continuous_proof_v1(ticket, &requested, certificate)
        };
        let (ticket, certificate) = assert_recoverable_failure_v1(
            result,
            SpeculativeUnprovenFoldCertificationErrorV1::ValidationPanicked,
            &expected_ticket,
        );
        bind_speculative_unproven_tree_continuous_proof_v1(ticket, &requested, certificate)
            .expect("the internally caught panic must leave both inputs retryable");
    }

    #[test]
    fn source_fingerprint_comparison_is_exact_and_allocation_free() {
        let bytes = [0xa5; 32];
        assert!(lowercase_sha256_matches_v1(bytes, &"a5".repeat(32)));
        assert!(!lowercase_sha256_matches_v1(bytes, &"A5".repeat(32)));
        assert!(!lowercase_sha256_matches_v1(bytes, &"a5".repeat(31)));
        assert!(!lowercase_sha256_matches_v1([0x5a; 32], &"a5".repeat(32)));
    }

    #[test]
    fn ticket_and_certificate_drift_fail_closed() {
        let mut source_revision = certification_fixture(37.0);
        let original = &source_revision.ticket.binding;
        source_revision.ticket.binding = SpeculativeUnprovenFoldBindingV1::new(
            original.project_instance_id(),
            original.project_id(),
            original.source_revision() + 1,
            original.source_geometry_fingerprint_sha256().to_owned(),
            original.pose_generation(),
            original.request_generation_id(),
            f64::from_bits(original.paper_thickness_bits()),
            original.approximate_blocking_observation(),
        )
        .expect("well-formed source-revision drift");
        let expected_ticket = ResolutionTicketSnapshotV1::capture(&source_revision.ticket);
        assert_recoverable_failure_v1(
            bind_speculative_unproven_tree_continuous_proof_v1(
                source_revision.ticket,
                &source_revision.requested,
                source_revision.certificate,
            ),
            SpeculativeUnprovenFoldCertificationErrorV1::SourceRevisionMismatch,
            &expected_ticket,
        );

        let mut revision = certification_fixture(37.0);
        revision.ticket.target_revision += 1;
        let expected_ticket = ResolutionTicketSnapshotV1::capture(&revision.ticket);
        assert_recoverable_failure_v1(
            bind_speculative_unproven_tree_continuous_proof_v1(
                revision.ticket,
                &revision.requested,
                revision.certificate,
            ),
            SpeculativeUnprovenFoldCertificationErrorV1::TargetRevisionMismatch,
            &expected_ticket,
        );

        let mut fingerprint = certification_fixture(37.0);
        fingerprint.ticket.target_geometry_fingerprint[0] ^= 1;
        let expected_ticket = ResolutionTicketSnapshotV1::capture(&fingerprint.ticket);
        assert_recoverable_failure_v1(
            bind_speculative_unproven_tree_continuous_proof_v1(
                fingerprint.ticket,
                &fingerprint.requested,
                fingerprint.certificate,
            ),
            SpeculativeUnprovenFoldCertificationErrorV1::TargetGeometryFingerprintMismatch,
            &expected_ticket,
        );

        let mut lineage = certification_fixture(37.0);
        let original = &lineage.ticket.binding;
        lineage.ticket.binding = SpeculativeUnprovenFoldBindingV1::new(
            original.project_instance_id(),
            ProjectId::new(),
            original.source_revision(),
            original.source_geometry_fingerprint_sha256().to_owned(),
            original.pose_generation(),
            original.request_generation_id(),
            f64::from_bits(original.paper_thickness_bits()),
            original.approximate_blocking_observation(),
        )
        .expect("well-formed foreign project binding");
        let expected_ticket = ResolutionTicketSnapshotV1::capture(&lineage.ticket);
        assert_recoverable_failure_v1(
            bind_speculative_unproven_tree_continuous_proof_v1(
                lineage.ticket,
                &lineage.requested,
                lineage.certificate,
            ),
            SpeculativeUnprovenFoldCertificationErrorV1::ProjectLineageMismatch,
            &expected_ticket,
        );

        let mut source_fingerprint = certification_fixture(37.0);
        let original = &source_fingerprint.ticket.binding;
        source_fingerprint.ticket.binding = SpeculativeUnprovenFoldBindingV1::new(
            original.project_instance_id(),
            original.project_id(),
            original.source_revision(),
            "00".repeat(32),
            original.pose_generation(),
            original.request_generation_id(),
            f64::from_bits(original.paper_thickness_bits()),
            original.approximate_blocking_observation(),
        )
        .expect("well-formed source-fingerprint drift");
        let expected_ticket = ResolutionTicketSnapshotV1::capture(&source_fingerprint.ticket);
        assert_recoverable_failure_v1(
            bind_speculative_unproven_tree_continuous_proof_v1(
                source_fingerprint.ticket,
                &source_fingerprint.requested,
                source_fingerprint.certificate,
            ),
            SpeculativeUnprovenFoldCertificationErrorV1::SourceGeometryFingerprintMismatch,
            &expected_ticket,
        );

        let mut thickness = certification_fixture(37.0);
        let original = &thickness.ticket.binding;
        thickness.ticket.binding = SpeculativeUnprovenFoldBindingV1::new(
            original.project_instance_id(),
            original.project_id(),
            original.source_revision(),
            original.source_geometry_fingerprint_sha256().to_owned(),
            original.pose_generation(),
            original.request_generation_id(),
            f64::from_bits(original.paper_thickness_bits() + 1),
            original.approximate_blocking_observation(),
        )
        .expect("well-formed thickness-drifted binding");
        let expected_ticket = ResolutionTicketSnapshotV1::capture(&thickness.ticket);
        assert_recoverable_failure_v1(
            bind_speculative_unproven_tree_continuous_proof_v1(
                thickness.ticket,
                &thickness.requested,
                thickness.certificate,
            ),
            SpeculativeUnprovenFoldCertificationErrorV1::PaperThicknessBitsMismatch,
            &expected_ticket,
        );

        let mut observation = certification_fixture(37.0);
        let original = &observation.ticket.binding;
        observation.ticket.binding = SpeculativeUnprovenFoldBindingV1::new(
            original.project_instance_id(),
            original.project_id(),
            original.source_revision(),
            original.source_geometry_fingerprint_sha256().to_owned(),
            original.pose_generation(),
            original.request_generation_id(),
            f64::from_bits(original.paper_thickness_bits()),
            SpeculativeApproximateBlockingObservationV1::blocking_sample_observed(12.5)
                .expect("valid blocking observation"),
        )
        .expect("well-formed blocking-observation drift");
        let expected_ticket = ResolutionTicketSnapshotV1::capture(&observation.ticket);
        assert_recoverable_failure_v1(
            bind_speculative_unproven_tree_continuous_proof_v1(
                observation.ticket,
                &observation.requested,
                observation.certificate,
            ),
            SpeculativeUnprovenFoldCertificationErrorV1::ApproximateBlockingObservationMismatch,
            &expected_ticket,
        );

        let mut semantic_pose = certification_fixture(37.0);
        let native = semantic_pose.requested.pose();
        let hinge_ids = native
            .hinges()
            .iter()
            .map(|hinge| hinge.edge())
            .collect::<Vec<_>>();
        let changed_angles = native
            .hinge_angles()
            .iter()
            .map(|angle| {
                (
                    angle.edge(),
                    f64::from_bits(angle.angle_degrees().to_bits() + 1),
                )
            })
            .collect::<Vec<_>>();
        semantic_pose.ticket.target_applied_pose = prepare_applied_pose_v1(
            native.face_ids(),
            &hinge_ids,
            native.fixed_face(),
            &changed_angles,
            AppliedPoseLimitsV1::default(),
        )
        .expect("different but valid semantic pose");
        let expected_ticket = ResolutionTicketSnapshotV1::capture(&semantic_pose.ticket);
        assert_recoverable_failure_v1(
            bind_speculative_unproven_tree_continuous_proof_v1(
                semantic_pose.ticket,
                &semantic_pose.requested,
                semantic_pose.certificate,
            ),
            SpeculativeUnprovenFoldCertificationErrorV1::TargetAppliedPoseMismatch,
            &expected_ticket,
        );

        let certificate = certification_fixture(37.0);
        let wrong_angles = CanonicalHingeAngles::new(
            certificate
                .requested
                .pose()
                .hinge_angles()
                .iter()
                .map(|angle| {
                    HingeAngle::new(angle.edge(), angle.angle_degrees() + 1.0)
                        .expect("different target angle")
                })
                .collect(),
        )
        .expect("canonical different target angles");
        let wrong_certificate = certify_tree_continuous_path_from_pose_v1(
            certificate.requested.initial().target().model(),
            certificate.requested.initial().pose(),
            &wrong_angles,
            0.0,
            StackedFoldPathDiagnosticLimitsV1::default(),
        )
        .expect("different continuous diagnosis")
        .expect("different simple path is certified");
        let expected_ticket = ResolutionTicketSnapshotV1::capture(&certificate.ticket);
        let failure = bind_speculative_unproven_tree_continuous_proof_v1(
            certificate.ticket,
            &certificate.requested,
            wrong_certificate,
        )
        .expect_err("the wrong target certificate must be rejected");
        assert_eq!(
            failure.error(),
            &SpeculativeUnprovenFoldCertificationErrorV1::ContinuousCertificateMismatch
        );
        let returned_ticket = failure.into_ticket();
        assert_eq!(
            ResolutionTicketSnapshotV1::capture(&returned_ticket),
            expected_ticket
        );
        bind_speculative_unproven_tree_continuous_proof_v1(
            returned_ticket,
            &certificate.requested,
            certificate.certificate,
        )
        .expect("the unchanged ticket can be retried with a new exact certificate");

        let exact = certification_fixture(37.0);
        let foreign = certification_fixture(37.0);
        let expected_ticket = ResolutionTicketSnapshotV1::capture(&exact.ticket);
        assert_recoverable_failure_v1(
            bind_speculative_unproven_tree_continuous_proof_v1(
                exact.ticket,
                &exact.requested,
                foreign.certificate,
            ),
            SpeculativeUnprovenFoldCertificationErrorV1::ContinuousCertificateMismatch,
            &expected_ticket,
        );

        let exact = certification_fixture(37.0);
        let foreign = certification_fixture(37.0);
        let expected_ticket = ResolutionTicketSnapshotV1::capture(&exact.ticket);
        let failure = bind_speculative_unproven_tree_continuous_proof_v1(
            exact.ticket,
            &foreign.requested,
            exact.certificate,
        )
        .expect_err("a foreign prepared request must be rejected");
        assert!(matches!(
            failure.error(),
            &(SpeculativeUnprovenFoldCertificationErrorV1::ProjectLineageMismatch
                | SpeculativeUnprovenFoldCertificationErrorV1::TargetGeometryFingerprintMismatch)
        ));
        let (_, ticket, _) = failure.into_parts();
        assert_eq!(
            ResolutionTicketSnapshotV1::capture(&ticket),
            expected_ticket
        );
    }
}
