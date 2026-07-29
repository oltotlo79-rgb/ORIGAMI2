use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::Arc,
};

use ori_collision::{
    CooperativeOperationControlV1, CooperativeOperationStopV1,
    LayeredFourFaceChainContinuousCertificateV1, LayeredFourFaceChainContinuousErrorV1,
    LayeredFourFaceChainContinuousLimitsV1, LayeredThreeFaceContinuousCertificateV1,
    LayeredThreeFaceContinuousErrorV1, LayeredThreeFaceContinuousLimitsV1,
    NativeStackedFoldInitialSampleLayerAdmissionV1, StackedFoldPathDiagnosticErrorV1,
    StackedFoldTreeContinuousCertificateV1,
};
use ori_kinematics::CanonicalHingeAngles;
use thiserror::Error;

use crate::{
    APPLIED_POSE_MODEL_ID_V1, AppliedPoseV1, MAX_REVISION,
    stacked_fold::{
        PreparedStackedFoldRequestIssuerSealV1, PreparedStackedFoldRequestedPoseV1,
        StackedFoldInitialLayerOrderV1,
    },
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
    prepared_request_issuer_seal: Option<PreparedStackedFoldRequestIssuerSealV1>,
}

impl SpeculativeUnprovenFoldResolutionTicketV1 {
    pub(super) fn new(
        editor_instance_anchor: Arc<()>,
        binding: SpeculativeUnprovenFoldBindingV1,
        target_revision: Revision,
        target_geometry_fingerprint: [u8; 32],
        target_applied_pose: AppliedPoseV1,
        prepared_request_issuer_seal: Option<PreparedStackedFoldRequestIssuerSealV1>,
    ) -> Self {
        Self {
            editor_instance_anchor,
            binding,
            target_revision,
            target_geometry_fingerprint,
            target_applied_pose,
            prepared_request_issuer_seal,
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
    pub(super) fn resolution_identity(
        &self,
    ) -> (&Arc<()>, &SpeculativeUnprovenFoldBindingV1, Revision) {
        (
            &self.editor_instance_anchor,
            &self.binding,
            self.target_revision,
        )
    }

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
    #[error("speculative continuous-proof certification was cancelled")]
    Cancelled,
    #[error("speculative continuous-proof certification absolute deadline elapsed")]
    DeadlineExceeded,
    #[error(
        "native continuous-certificate revalidation could not complete with available resources"
    )]
    ResourceUnavailable,
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
    #[error("the resolution ticket was not issued for this exact prepared request instance")]
    PreparedRequestIssuerMismatch,
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
    bind_speculative_unproven_tree_continuous_proof_with_control_v1(
        ticket,
        requested,
        certificate,
        &CooperativeOperationControlV1::unbounded(),
    )
}

/// Controlled form of [`bind_speculative_unproven_tree_continuous_proof_v1`].
///
/// Every cooperative stop returns the original one-shot ticket and native
/// certificate in [`SpeculativeUnprovenFoldCertificationFailureV1`].  A
/// stopped operation therefore cannot mint a typed proof and remains exactly
/// retryable with a fresh control.
#[allow(clippy::result_large_err)]
pub fn bind_speculative_unproven_tree_continuous_proof_with_control_v1(
    ticket: SpeculativeUnprovenFoldResolutionTicketV1,
    requested: &PreparedStackedFoldRequestedPoseV1,
    certificate: StackedFoldTreeContinuousCertificateV1,
    control: &CooperativeOperationControlV1<'_>,
) -> Result<SpeculativeUnprovenFoldCertifiedProofV1, SpeculativeUnprovenFoldCertificationFailureV1>
{
    let validation = catch_unwind(AssertUnwindSafe(|| {
        certification_checkpoint_v1(control)?;
        validate_speculative_unproven_tree_continuous_proof_v1(
            &ticket,
            requested,
            &certificate,
            control,
        )
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
    // Recheck outside the unwind boundary immediately before consuming the
    // retry authority. A stop racing completed native revalidation must still
    // return both one-shot inputs instead of minting a proof after withdrawal.
    if let Err(error) = certification_ownership_checkpoint_v1(control) {
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
        prepared_request_issuer_seal: _,
    } = ticket;
    Ok(SpeculativeUnprovenFoldCertifiedProofV1 {
        editor_instance_anchor,
        binding,
        target_revision,
        target_geometry_fingerprint,
        target_applied_pose,
    })
}

/// Opaque one-shot authority bound specifically to the narrow three-face
/// layered continuous theorem. It is deliberately distinct from
/// [`SpeculativeUnprovenFoldCertifiedProofV1`]: a layered initial-admission
/// proof must never be substituted for the ordinary tree-path proof.
///
/// ```compile_fail
/// fn requires_clone<T: Clone>() {}
/// requires_clone::<ori_core::SpeculativeUnprovenFoldLayeredThreeFaceCertifiedProofV1>();
/// ```
///
/// ```compile_fail
/// fn requires_serialize<T: serde::Serialize>() {}
/// requires_serialize::<ori_core::SpeculativeUnprovenFoldLayeredThreeFaceCertifiedProofV1>();
/// ```
///
/// ```compile_fail
/// fn must_not_gain_resolution_power(
///     proof: ori_core::SpeculativeUnprovenFoldLayeredThreeFaceCertifiedProofV1,
/// ) {
///     let _ = proof.into_resolution_parts();
/// }
/// ```
///
/// ```compile_fail
/// fn requires_tree(_: ori_core::SpeculativeUnprovenFoldCertifiedProofV1) {}
/// fn layered_is_not_a_tree(
///     proof: ori_core::SpeculativeUnprovenFoldLayeredThreeFaceCertifiedProofV1,
/// ) {
///     requires_tree(proof);
/// }
/// ```
#[derive(Debug)]
pub struct SpeculativeUnprovenFoldLayeredThreeFaceCertifiedProofV1 {
    editor_instance_anchor: Arc<()>,
    binding: SpeculativeUnprovenFoldBindingV1,
    target_revision: Revision,
    target_geometry_fingerprint: [u8; 32],
    target_applied_pose: AppliedPoseV1,
}

impl SpeculativeUnprovenFoldLayeredThreeFaceCertifiedProofV1 {
    pub(super) fn resolution_identity(
        &self,
    ) -> (&Arc<()>, &SpeculativeUnprovenFoldBindingV1, Revision) {
        (
            &self.editor_instance_anchor,
            &self.binding,
            self.target_revision,
        )
    }

    /// Crate-internal consumption boundary for the exact Awaiting-mark
    /// resolver. This is deliberately unavailable to external callers and
    /// grants no project mutation authority.
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

    /// This read-only binder does not grant project mutation authority.
    #[must_use]
    pub const fn authorizes_project_mutation(&self) -> bool {
        false
    }

    /// Runtime provenance only; this exposes no editor handle or mutation
    /// capability.
    #[must_use]
    pub fn binding(&self) -> &SpeculativeUnprovenFoldBindingV1 {
        &self.binding
    }

    #[must_use]
    pub const fn target_revision(&self) -> Revision {
        self.target_revision
    }

    #[must_use]
    pub const fn target_geometry_fingerprint(&self) -> &[u8; 32] {
        &self.target_geometry_fingerprint
    }

    /// The semantic pose is observation data, not an editor mutation handle.
    #[must_use]
    pub const fn target_applied_pose(&self) -> &AppliedPoseV1 {
        &self.target_applied_pose
    }

    /// Compares opaque runtime issuers without exposing either issuer.
    #[must_use]
    pub fn has_same_editor_instance_as(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.editor_instance_anchor, &other.editor_instance_anchor)
    }
}

/// Recoverable failure from the layered-three-face binding boundary.
#[derive(Debug, Error)]
#[error("{error}")]
pub struct SpeculativeUnprovenFoldLayeredThreeFaceCertificationFailureV1 {
    error: SpeculativeUnprovenFoldLayeredThreeFaceCertificationErrorV1,
    ticket: SpeculativeUnprovenFoldResolutionTicketV1,
    certificate: LayeredThreeFaceContinuousCertificateV1,
}

impl SpeculativeUnprovenFoldLayeredThreeFaceCertificationFailureV1 {
    #[must_use]
    pub const fn error(&self) -> &SpeculativeUnprovenFoldLayeredThreeFaceCertificationErrorV1 {
        &self.error
    }

    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        SpeculativeUnprovenFoldLayeredThreeFaceCertificationErrorV1,
        SpeculativeUnprovenFoldResolutionTicketV1,
        LayeredThreeFaceContinuousCertificateV1,
    ) {
        (self.error, self.ticket, self.certificate)
    }
}

/// Errors from the layered-only binder. The common ticket/request validation
/// is intentionally preserved verbatim, while the final native authority is
/// named separately from ordinary continuous certification.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SpeculativeUnprovenFoldLayeredThreeFaceCertificationErrorV1 {
    #[error("layered three-face certification was cancelled")]
    Cancelled,
    #[error("layered three-face certification absolute deadline elapsed")]
    DeadlineExceeded,
    #[error(
        "native layered three-face certificate revalidation could not complete with available resources"
    )]
    ResourceUnavailable,
    #[error(transparent)]
    Common(#[from] SpeculativeUnprovenFoldCertificationErrorV1),
    #[error(
        "the layered three-face certificate does not certify the exact requested path, admission, and limits"
    )]
    LayeredCertificateMismatch,
}

/// Binds a speculative Apply ticket to the distinct layered-three-face
/// continuous certificate authority.
///
/// A failed validation, cooperative stop, allocation failure, or caught panic
/// returns both non-cloneable inputs unchanged, so the exact ticket remains
/// retryable. Success consumes both inputs and produces no mutation power.
#[allow(clippy::result_large_err)]
pub fn bind_speculative_unproven_layered_three_face_continuous_proof_v1(
    ticket: SpeculativeUnprovenFoldResolutionTicketV1,
    requested: &PreparedStackedFoldRequestedPoseV1,
    admission: &NativeStackedFoldInitialSampleLayerAdmissionV1<StackedFoldInitialLayerOrderV1>,
    limits: LayeredThreeFaceContinuousLimitsV1,
    certificate: LayeredThreeFaceContinuousCertificateV1,
) -> Result<
    SpeculativeUnprovenFoldLayeredThreeFaceCertifiedProofV1,
    SpeculativeUnprovenFoldLayeredThreeFaceCertificationFailureV1,
> {
    bind_speculative_unproven_layered_three_face_continuous_proof_with_control_v1(
        ticket,
        requested,
        admission,
        limits,
        certificate,
        &CooperativeOperationControlV1::unbounded(),
    )
}

/// Controlled form of [`bind_speculative_unproven_layered_three_face_continuous_proof_v1`].
///
/// Native revalidation receives the same cooperative control. Every stop is
/// mapped to a direct layered-binder stop error while retaining both one-shot
/// inputs for retry.
#[allow(clippy::result_large_err)]
pub fn bind_speculative_unproven_layered_three_face_continuous_proof_with_control_v1(
    ticket: SpeculativeUnprovenFoldResolutionTicketV1,
    requested: &PreparedStackedFoldRequestedPoseV1,
    admission: &NativeStackedFoldInitialSampleLayerAdmissionV1<StackedFoldInitialLayerOrderV1>,
    limits: LayeredThreeFaceContinuousLimitsV1,
    certificate: LayeredThreeFaceContinuousCertificateV1,
    control: &CooperativeOperationControlV1<'_>,
) -> Result<
    SpeculativeUnprovenFoldLayeredThreeFaceCertifiedProofV1,
    SpeculativeUnprovenFoldLayeredThreeFaceCertificationFailureV1,
> {
    let validation = catch_unwind(AssertUnwindSafe(|| {
        certification_checkpoint_v1(control).map_err(map_common_layered_certification_error_v1)?;
        let requested_angles = validate_layered_ticket_request_v1(&ticket, requested, control)?;
        certification_checkpoint_v1(control)?;
        panic_before_native_revalidation_if_forced_v1();
        let certificate_is_for = certificate
            .is_for_with_control_v1(
                requested.initial().target().model(),
                requested.initial().pose(),
                &requested_angles,
                admission,
                limits,
                control,
            )
            .map_err(map_layered_certificate_revalidation_error_v1)?;
        if !certificate_is_for {
            return Err(SpeculativeUnprovenFoldLayeredThreeFaceCertificationErrorV1::LayeredCertificateMismatch);
        }
        certification_checkpoint_v1(control)?;
        Ok(())
    }));
    let error = match validation {
        Ok(Ok(())) => None,
        Ok(Err(error)) => Some(error),
        Err(_) => Some(
            SpeculativeUnprovenFoldLayeredThreeFaceCertificationErrorV1::Common(
                SpeculativeUnprovenFoldCertificationErrorV1::ValidationPanicked,
            ),
        ),
    };
    if let Some(error) = error {
        return Err(
            SpeculativeUnprovenFoldLayeredThreeFaceCertificationFailureV1 {
                error,
                ticket,
                certificate,
            },
        );
    }
    // The validation closure's final checkpoint protects native
    // revalidation. Observe cancellation once more outside that unwind
    // boundary, immediately before the sole ownership-consuming operation.
    // A late stop therefore still returns both one-shot inputs rather than
    // minting an authority after the caller withdrew it.
    if let Err(error) = certification_ownership_checkpoint_v1(control) {
        return Err(
            SpeculativeUnprovenFoldLayeredThreeFaceCertificationFailureV1 {
                error: map_common_layered_certification_error_v1(error),
                ticket,
                certificate,
            },
        );
    }
    let SpeculativeUnprovenFoldResolutionTicketV1 {
        editor_instance_anchor,
        binding,
        target_revision,
        target_geometry_fingerprint,
        target_applied_pose,
        prepared_request_issuer_seal: _,
    } = ticket;
    Ok(SpeculativeUnprovenFoldLayeredThreeFaceCertifiedProofV1 {
        editor_instance_anchor,
        binding,
        target_revision,
        target_geometry_fingerprint,
        target_applied_pose,
    })
}

fn validate_layered_ticket_request_v1(
    ticket: &SpeculativeUnprovenFoldResolutionTicketV1,
    requested: &PreparedStackedFoldRequestedPoseV1,
    control: &CooperativeOperationControlV1<'_>,
) -> Result<CanonicalHingeAngles, SpeculativeUnprovenFoldLayeredThreeFaceCertificationErrorV1> {
    validate_speculative_ticket_request_common_v1(ticket, requested, control)
        .map_err(map_common_layered_certification_error_v1)
}

fn map_common_layered_certification_error_v1(
    error: SpeculativeUnprovenFoldCertificationErrorV1,
) -> SpeculativeUnprovenFoldLayeredThreeFaceCertificationErrorV1 {
    match error {
        SpeculativeUnprovenFoldCertificationErrorV1::Cancelled => {
            SpeculativeUnprovenFoldLayeredThreeFaceCertificationErrorV1::Cancelled
        }
        SpeculativeUnprovenFoldCertificationErrorV1::DeadlineExceeded => {
            SpeculativeUnprovenFoldLayeredThreeFaceCertificationErrorV1::DeadlineExceeded
        }
        error => SpeculativeUnprovenFoldLayeredThreeFaceCertificationErrorV1::Common(error),
    }
}

const fn map_layered_certificate_revalidation_error_v1(
    error: LayeredThreeFaceContinuousErrorV1,
) -> SpeculativeUnprovenFoldLayeredThreeFaceCertificationErrorV1 {
    match error {
        LayeredThreeFaceContinuousErrorV1::Cancelled => {
            SpeculativeUnprovenFoldLayeredThreeFaceCertificationErrorV1::Cancelled
        }
        LayeredThreeFaceContinuousErrorV1::DeadlineExceeded => {
            SpeculativeUnprovenFoldLayeredThreeFaceCertificationErrorV1::DeadlineExceeded
        }
        LayeredThreeFaceContinuousErrorV1::ResourceLimit => {
            SpeculativeUnprovenFoldLayeredThreeFaceCertificationErrorV1::ResourceUnavailable
        }
        _ => {
            SpeculativeUnprovenFoldLayeredThreeFaceCertificationErrorV1::LayeredCertificateMismatch
        }
    }
}

/// Opaque one-shot authority bound specifically to the narrow four-face-chain
/// layered continuous theorem.
///
/// This authority is deliberately distinct from both the ordinary tree-path
/// proof and the three-face layered proof. It implements neither `Clone` nor
/// serialization, and successful binding grants no project-mutation power.
///
/// ```compile_fail
/// fn requires_clone<T: Clone>() {}
/// requires_clone::<ori_core::SpeculativeUnprovenFoldLayeredFourFaceCertifiedProofV1>();
/// ```
///
/// ```compile_fail
/// fn requires_serialize<T: serde::Serialize>() {}
/// requires_serialize::<ori_core::SpeculativeUnprovenFoldLayeredFourFaceCertifiedProofV1>();
/// ```
///
/// ```compile_fail
/// fn must_not_gain_resolution_power(
///     proof: ori_core::SpeculativeUnprovenFoldLayeredFourFaceCertifiedProofV1,
/// ) {
///     let _ = proof.into_resolution_parts();
/// }
/// ```
///
/// ```compile_fail
/// fn requires_three(
///     _: ori_core::SpeculativeUnprovenFoldLayeredThreeFaceCertifiedProofV1,
/// ) {}
/// fn four_is_not_three(
///     proof: ori_core::SpeculativeUnprovenFoldLayeredFourFaceCertifiedProofV1,
/// ) {
///     requires_three(proof);
/// }
/// ```
///
/// ```compile_fail
/// fn requires_tree(_: ori_core::SpeculativeUnprovenFoldCertifiedProofV1) {}
/// fn four_is_not_tree(
///     proof: ori_core::SpeculativeUnprovenFoldLayeredFourFaceCertifiedProofV1,
/// ) {
///     requires_tree(proof);
/// }
/// ```
#[derive(Debug)]
pub struct SpeculativeUnprovenFoldLayeredFourFaceCertifiedProofV1 {
    editor_instance_anchor: Arc<()>,
    binding: SpeculativeUnprovenFoldBindingV1,
    target_revision: Revision,
    target_geometry_fingerprint: [u8; 32],
    target_applied_pose: AppliedPoseV1,
}

impl SpeculativeUnprovenFoldLayeredFourFaceCertifiedProofV1 {
    pub(super) fn resolution_identity(
        &self,
    ) -> (&Arc<()>, &SpeculativeUnprovenFoldBindingV1, Revision) {
        (
            &self.editor_instance_anchor,
            &self.binding,
            self.target_revision,
        )
    }

    /// Crate-internal consumption boundary for the exact Awaiting-mark
    /// resolver. External callers receive no editor handle or mutation
    /// capability.
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

    /// This read-only binder does not grant project mutation authority.
    #[must_use]
    pub const fn authorizes_project_mutation(&self) -> bool {
        false
    }

    /// Returns the exact speculative mark binding without exposing an editor
    /// or any mutation capability.
    #[must_use]
    pub fn binding(&self) -> &SpeculativeUnprovenFoldBindingV1 {
        &self.binding
    }

    #[must_use]
    pub const fn target_revision(&self) -> Revision {
        self.target_revision
    }

    #[must_use]
    pub const fn target_geometry_fingerprint(&self) -> &[u8; 32] {
        &self.target_geometry_fingerprint
    }

    /// The semantic pose is observation data, not an editor mutation handle.
    #[must_use]
    pub const fn target_applied_pose(&self) -> &AppliedPoseV1 {
        &self.target_applied_pose
    }

    /// Compares opaque runtime issuers without exposing either issuer.
    #[must_use]
    pub fn has_same_editor_instance_as(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.editor_instance_anchor, &other.editor_instance_anchor)
    }
}

/// Recoverable failure from the layered-four-face binding boundary.
///
/// Every failure retains both non-cloneable inputs so a caller can inspect the
/// typed error and retry the exact ticket/certificate pair with a fresh
/// cooperative control.
#[derive(Debug, Error)]
#[error("{error}")]
pub struct SpeculativeUnprovenFoldLayeredFourFaceCertificationFailureV1 {
    error: SpeculativeUnprovenFoldLayeredFourFaceCertificationErrorV1,
    ticket: SpeculativeUnprovenFoldResolutionTicketV1,
    certificate: LayeredFourFaceChainContinuousCertificateV1,
}

impl SpeculativeUnprovenFoldLayeredFourFaceCertificationFailureV1 {
    #[must_use]
    pub const fn error(&self) -> &SpeculativeUnprovenFoldLayeredFourFaceCertificationErrorV1 {
        &self.error
    }

    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        SpeculativeUnprovenFoldLayeredFourFaceCertificationErrorV1,
        SpeculativeUnprovenFoldResolutionTicketV1,
        LayeredFourFaceChainContinuousCertificateV1,
    ) {
        (self.error, self.ticket, self.certificate)
    }
}

/// Errors from the four-face-chain layered-only binder.
///
/// Common ticket/request failures remain losslessly typed, while failures of
/// the native four-face theorem are kept distinct from ordinary tree-path and
/// three-face certification.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SpeculativeUnprovenFoldLayeredFourFaceCertificationErrorV1 {
    #[error("layered four-face certification was cancelled")]
    Cancelled,
    #[error("layered four-face certification absolute deadline elapsed")]
    DeadlineExceeded,
    #[error(
        "native layered four-face certificate revalidation could not complete with available resources"
    )]
    ResourceUnavailable,
    #[error(transparent)]
    Common(#[from] SpeculativeUnprovenFoldCertificationErrorV1),
    #[error(
        "the layered four-face certificate does not certify the exact requested path, admission, and limits"
    )]
    LayeredCertificateMismatch,
}

/// Binds a speculative Apply ticket to the distinct four-face-chain layered
/// continuous-certificate authority.
///
/// The ticket/request lineage, target semantic pose, initial layer admission,
/// exact limits and native certificate are all revalidated against one common
/// binding. Every ordinary validation failure, cooperative stop or caught
/// panic returns both one-shot inputs unchanged.
#[allow(clippy::result_large_err)]
pub fn bind_speculative_unproven_layered_four_face_chain_continuous_proof_v1(
    ticket: SpeculativeUnprovenFoldResolutionTicketV1,
    requested: &PreparedStackedFoldRequestedPoseV1,
    admission: &NativeStackedFoldInitialSampleLayerAdmissionV1<StackedFoldInitialLayerOrderV1>,
    limits: LayeredFourFaceChainContinuousLimitsV1,
    certificate: LayeredFourFaceChainContinuousCertificateV1,
) -> Result<
    SpeculativeUnprovenFoldLayeredFourFaceCertifiedProofV1,
    SpeculativeUnprovenFoldLayeredFourFaceCertificationFailureV1,
> {
    bind_speculative_unproven_layered_four_face_chain_continuous_proof_with_control_v1(
        ticket,
        requested,
        admission,
        limits,
        certificate,
        &CooperativeOperationControlV1::unbounded(),
    )
}

/// Controlled form of
/// [`bind_speculative_unproven_layered_four_face_chain_continuous_proof_v1`].
///
/// Native revalidation receives the exact supplied cooperative control. A
/// stop observed anywhere through the final ownership boundary returns both
/// one-shot inputs and can never mint a typed proof.
#[allow(clippy::result_large_err)]
pub fn bind_speculative_unproven_layered_four_face_chain_continuous_proof_with_control_v1(
    ticket: SpeculativeUnprovenFoldResolutionTicketV1,
    requested: &PreparedStackedFoldRequestedPoseV1,
    admission: &NativeStackedFoldInitialSampleLayerAdmissionV1<StackedFoldInitialLayerOrderV1>,
    limits: LayeredFourFaceChainContinuousLimitsV1,
    certificate: LayeredFourFaceChainContinuousCertificateV1,
    control: &CooperativeOperationControlV1<'_>,
) -> Result<
    SpeculativeUnprovenFoldLayeredFourFaceCertifiedProofV1,
    SpeculativeUnprovenFoldLayeredFourFaceCertificationFailureV1,
> {
    let validation = catch_unwind(AssertUnwindSafe(|| {
        validate_speculative_unproven_layered_four_face_chain_continuous_proof_v1(
            &ticket,
            requested,
            admission,
            limits,
            &certificate,
            control,
        )
    }));
    let error = match validation {
        Ok(Ok(())) => None,
        Ok(Err(error)) => Some(error),
        Err(_) => Some(
            SpeculativeUnprovenFoldLayeredFourFaceCertificationErrorV1::Common(
                SpeculativeUnprovenFoldCertificationErrorV1::ValidationPanicked,
            ),
        ),
    };
    if let Some(error) = error {
        return Err(
            SpeculativeUnprovenFoldLayeredFourFaceCertificationFailureV1 {
                error,
                ticket,
                certificate,
            },
        );
    }

    // Recheck immediately before consuming the retry authority. A
    // cancellation racing the completed native validation therefore remains
    // recoverable and cannot mint an authority after withdrawal.
    if let Err(error) = certification_ownership_checkpoint_v1(control) {
        return Err(
            SpeculativeUnprovenFoldLayeredFourFaceCertificationFailureV1 {
                error: map_common_layered_four_face_certification_error_v1(error),
                ticket,
                certificate,
            },
        );
    }

    let SpeculativeUnprovenFoldResolutionTicketV1 {
        editor_instance_anchor,
        binding,
        target_revision,
        target_geometry_fingerprint,
        target_applied_pose,
        prepared_request_issuer_seal: _,
    } = ticket;
    Ok(SpeculativeUnprovenFoldLayeredFourFaceCertifiedProofV1 {
        editor_instance_anchor,
        binding,
        target_revision,
        target_geometry_fingerprint,
        target_applied_pose,
    })
}

fn validate_speculative_unproven_layered_four_face_chain_continuous_proof_v1(
    ticket: &SpeculativeUnprovenFoldResolutionTicketV1,
    requested: &PreparedStackedFoldRequestedPoseV1,
    admission: &NativeStackedFoldInitialSampleLayerAdmissionV1<StackedFoldInitialLayerOrderV1>,
    limits: LayeredFourFaceChainContinuousLimitsV1,
    certificate: &LayeredFourFaceChainContinuousCertificateV1,
    control: &CooperativeOperationControlV1<'_>,
) -> Result<(), SpeculativeUnprovenFoldLayeredFourFaceCertificationErrorV1> {
    certification_checkpoint_v1(control)
        .map_err(map_common_layered_four_face_certification_error_v1)?;
    let requested_angles =
        validate_speculative_ticket_request_common_v1(ticket, requested, control)
            .map_err(map_common_layered_four_face_certification_error_v1)?;
    certification_checkpoint_v1(control)
        .map_err(map_common_layered_four_face_certification_error_v1)?;
    panic_before_native_revalidation_if_forced_v1();
    let certificate_is_for = certificate
        .is_for_with_control_v1(
            requested.initial().target().model(),
            requested.initial().pose(),
            &requested_angles,
            admission,
            limits,
            control,
        )
        .map_err(map_layered_four_face_certificate_revalidation_error_v1)?;
    if !certificate_is_for {
        return Err(
            SpeculativeUnprovenFoldLayeredFourFaceCertificationErrorV1::LayeredCertificateMismatch,
        );
    }
    certification_checkpoint_v1(control)
        .map_err(map_common_layered_four_face_certification_error_v1)?;
    Ok(())
}

fn map_common_layered_four_face_certification_error_v1(
    error: SpeculativeUnprovenFoldCertificationErrorV1,
) -> SpeculativeUnprovenFoldLayeredFourFaceCertificationErrorV1 {
    match error {
        SpeculativeUnprovenFoldCertificationErrorV1::Cancelled => {
            SpeculativeUnprovenFoldLayeredFourFaceCertificationErrorV1::Cancelled
        }
        SpeculativeUnprovenFoldCertificationErrorV1::DeadlineExceeded => {
            SpeculativeUnprovenFoldLayeredFourFaceCertificationErrorV1::DeadlineExceeded
        }
        error => SpeculativeUnprovenFoldLayeredFourFaceCertificationErrorV1::Common(error),
    }
}

const fn map_layered_four_face_certificate_revalidation_error_v1(
    error: LayeredFourFaceChainContinuousErrorV1,
) -> SpeculativeUnprovenFoldLayeredFourFaceCertificationErrorV1 {
    match error {
        LayeredFourFaceChainContinuousErrorV1::Cancelled => {
            SpeculativeUnprovenFoldLayeredFourFaceCertificationErrorV1::Cancelled
        }
        LayeredFourFaceChainContinuousErrorV1::DeadlineExceeded => {
            SpeculativeUnprovenFoldLayeredFourFaceCertificationErrorV1::DeadlineExceeded
        }
        LayeredFourFaceChainContinuousErrorV1::ResourceLimit => {
            SpeculativeUnprovenFoldLayeredFourFaceCertificationErrorV1::ResourceUnavailable
        }
        _ => SpeculativeUnprovenFoldLayeredFourFaceCertificationErrorV1::LayeredCertificateMismatch,
    }
}

fn validate_speculative_unproven_tree_continuous_proof_v1(
    ticket: &SpeculativeUnprovenFoldResolutionTicketV1,
    requested: &PreparedStackedFoldRequestedPoseV1,
    certificate: &StackedFoldTreeContinuousCertificateV1,
    control: &CooperativeOperationControlV1<'_>,
) -> Result<(), SpeculativeUnprovenFoldCertificationErrorV1> {
    let requested_angles =
        validate_speculative_ticket_request_common_v1(ticket, requested, control)?;
    let initial = requested.initial();
    let target = initial.target();
    let paper_thickness_mm = f64::from_bits(ticket.binding.paper_thickness_bits());
    certification_checkpoint_v1(control)?;
    panic_before_native_revalidation_if_forced_v1();
    let certificate_is_for = certificate
        .is_for_with_control_v1(
            target.model(),
            initial.pose(),
            &requested_angles,
            paper_thickness_mm,
            control,
        )
        .map_err(map_continuous_certificate_revalidation_error_v1)?;
    certification_checkpoint_v1(control)?;
    if !certificate_is_for {
        return Err(SpeculativeUnprovenFoldCertificationErrorV1::ContinuousCertificateMismatch);
    }
    // Keep the final observed stop boundary immediately before consuming the
    // one-shot authority into a typed proof.
    certification_checkpoint_v1(control)?;

    Ok(())
}

fn validate_speculative_ticket_request_common_v1(
    ticket: &SpeculativeUnprovenFoldResolutionTicketV1,
    requested: &PreparedStackedFoldRequestedPoseV1,
    control: &CooperativeOperationControlV1<'_>,
) -> Result<CanonicalHingeAngles, SpeculativeUnprovenFoldCertificationErrorV1> {
    if !ticket
        .prepared_request_issuer_seal
        .as_ref()
        .is_some_and(|seal| seal.authenticates(requested))
    {
        return Err(SpeculativeUnprovenFoldCertificationErrorV1::PreparedRequestIssuerMismatch);
    }
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
    certification_checkpoint_v1(control)?;
    Ok(requested_angles)
}

fn certification_checkpoint_v1(
    control: &CooperativeOperationControlV1<'_>,
) -> Result<(), SpeculativeUnprovenFoldCertificationErrorV1> {
    control.checkpoint().map_err(|stop| match stop {
        CooperativeOperationStopV1::Cancelled => {
            SpeculativeUnprovenFoldCertificationErrorV1::Cancelled
        }
        CooperativeOperationStopV1::DeadlineExceeded => {
            SpeculativeUnprovenFoldCertificationErrorV1::DeadlineExceeded
        }
    })
}

fn certification_ownership_checkpoint_v1(
    control: &CooperativeOperationControlV1<'_>,
) -> Result<(), SpeculativeUnprovenFoldCertificationErrorV1> {
    #[cfg(test)]
    if CERTIFICATION_TEST_FAULT_V1
        .with(|fault| fault.get() == Some(CertificationTestFaultV1::LateOwnershipCancellation))
    {
        return Err(SpeculativeUnprovenFoldCertificationErrorV1::Cancelled);
    }
    certification_checkpoint_v1(control)
}

const fn map_continuous_certificate_revalidation_error_v1(
    error: StackedFoldPathDiagnosticErrorV1,
) -> SpeculativeUnprovenFoldCertificationErrorV1 {
    match error {
        StackedFoldPathDiagnosticErrorV1::Cancelled => {
            SpeculativeUnprovenFoldCertificationErrorV1::Cancelled
        }
        StackedFoldPathDiagnosticErrorV1::DeadlineExceeded => {
            SpeculativeUnprovenFoldCertificationErrorV1::DeadlineExceeded
        }
        StackedFoldPathDiagnosticErrorV1::PoseUnavailable
        | StackedFoldPathDiagnosticErrorV1::StaticDiagnosisUnavailable
        | StackedFoldPathDiagnosticErrorV1::ProofCacheUnavailable
        | StackedFoldPathDiagnosticErrorV1::StaleProofCacheResult
        | StackedFoldPathDiagnosticErrorV1::InitialLayerOrderUnavailable
        | StackedFoldPathDiagnosticErrorV1::InitialLayerOrderResourceLimit => {
            SpeculativeUnprovenFoldCertificationErrorV1::ResourceUnavailable
        }
        StackedFoldPathDiagnosticErrorV1::InvalidLimits
        | StackedFoldPathDiagnosticErrorV1::InvalidPath
        | StackedFoldPathDiagnosticErrorV1::PoseIssuerMismatch => {
            SpeculativeUnprovenFoldCertificationErrorV1::ContinuousCertificateMismatch
        }
    }
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
    LateOwnershipCancellation,
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
        prepared_request_issuer_seal: _,
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
pub(crate) fn bind_layered_resolution_ticket_for_test_v1(
    ticket: SpeculativeUnprovenFoldResolutionTicketV1,
) -> SpeculativeUnprovenFoldLayeredThreeFaceCertifiedProofV1 {
    let SpeculativeUnprovenFoldResolutionTicketV1 {
        editor_instance_anchor,
        binding,
        target_revision,
        target_geometry_fingerprint,
        target_applied_pose,
        prepared_request_issuer_seal: _,
    } = ticket;
    SpeculativeUnprovenFoldLayeredThreeFaceCertifiedProofV1 {
        editor_instance_anchor,
        binding,
        target_revision,
        target_geometry_fingerprint,
        target_applied_pose,
    }
}

#[cfg(test)]
pub(crate) fn bind_layered_four_face_resolution_ticket_for_test_v1(
    ticket: SpeculativeUnprovenFoldResolutionTicketV1,
) -> SpeculativeUnprovenFoldLayeredFourFaceCertifiedProofV1 {
    let SpeculativeUnprovenFoldResolutionTicketV1 {
        editor_instance_anchor,
        binding,
        target_revision,
        target_geometry_fingerprint,
        target_applied_pose,
        prepared_request_issuer_seal: _,
    } = ticket;
    SpeculativeUnprovenFoldLayeredFourFaceCertifiedProofV1 {
        editor_instance_anchor,
        binding,
        target_revision,
        target_geometry_fingerprint,
        target_applied_pose,
    }
}

#[cfg(test)]
pub(crate) fn bind_layered_resolution_ticket_with_target_revision_for_test_v1(
    ticket: SpeculativeUnprovenFoldResolutionTicketV1,
    target_revision: Revision,
) -> SpeculativeUnprovenFoldLayeredThreeFaceCertifiedProofV1 {
    let SpeculativeUnprovenFoldResolutionTicketV1 {
        editor_instance_anchor,
        binding,
        target_geometry_fingerprint,
        target_applied_pose,
        ..
    } = ticket;
    SpeculativeUnprovenFoldLayeredThreeFaceCertifiedProofV1 {
        editor_instance_anchor,
        binding,
        target_revision,
        target_geometry_fingerprint,
        target_applied_pose,
    }
}

#[cfg(test)]
mod tests;
