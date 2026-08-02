//! Profile-bound stationary whole-parent positive-thickness evidence.
//!
//! This boundary consumes an already sealed general-N layer-source
//! prerequisite, replays that exact live source/profile/limits tuple, and
//! combines it with an exact parent-graph admission and the existing native
//! positive-thickness pair proof. It proves only the submitted stationary
//! zero pose. It is not a continuous-path, collision-clearance, layer-
//! transport, mutation, Apply, or viewer authority.

use std::sync::Arc;

use ori_domain::{FaceId, ProjectId};
use ori_foldability::GlobalFlatFoldabilityModelId;
use ori_kinematics::CommonArticulationWholeParentClosureLimitsV2;
use sha2::{Digest, Sha256};
use thiserror::Error;

use super::*;
use crate::graph_positive_thickness::{
    CheckpointedPositiveThicknessGraphProofErrorV2,
    prove_profile_bound_stationary_positive_thickness_graph_geometry_v2,
};
use crate::{
    CommonArticulationAdmittedPositiveThicknessGraphProofErrorV2,
    CommonArticulationAdmittedPositiveThicknessGraphProofV2,
    CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2,
    CommonArticulationPositiveThicknessParentGraphAdmissionLimitsV2,
    CommonArticulationPositiveThicknessParentGraphAdmissionResourcesV2,
    CommonArticulationPositiveThicknessParentGraphAdmissionStopV2,
    CommonArticulationPositiveThicknessParentGraphAdmissionV2, PositiveThicknessGraphProofErrorV1,
    revalidate_common_articulation_positive_thickness_parent_graph_admission_with_checkpoint_v2,
};

pub const COMMON_ARTICULATION_PROFILE_BOUND_WHOLE_PARENT_POSITIVE_THICKNESS_MODEL_ID_V2: &str =
    "common_articulation_profile_bound_whole_parent_positive_thickness_v2";
pub const COMMON_ARTICULATION_PROFILE_BOUND_WHOLE_PARENT_POSITIVE_THICKNESS_UNPROMOTED_MODEL_ID_V2:
    &str = "common_articulation_profile_bound_whole_parent_positive_thickness_unpromoted_v2";

const GENERAL_N_MIN_BLOCKS_V2: usize = 33;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommonArticulationProfileBoundWholeParentPositiveThicknessStopV2 {
    Cancelled,
    DeadlineExceeded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum CommonArticulationProfileBoundWholeParentPositiveThicknessErrorV2 {
    #[error("the profile-bound stationary positive-thickness input is malformed")]
    InvalidInput,
    #[error("the profile-bound stationary positive-thickness input exceeds its resource envelope")]
    ResourceLimit,
    #[error("the retained general-N layer-source prerequisite does not replay: {0}")]
    Transport(CommonArticulationGeneralCellTransportErrorV2),
    #[error("the exact parent-graph admission does not replay: {0}")]
    ParentAdmission(CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2),
    #[error("the stationary positive-thickness proof failed: {0}")]
    PositiveThickness(CommonArticulationAdmittedPositiveThicknessGraphProofErrorV2),
    #[error("the retained stationary positive-thickness certificate does not match the live input")]
    CertificateBindingMismatch,
    #[error("the stationary positive-thickness operation was cancelled")]
    Cancelled,
    #[error("the stationary positive-thickness operation deadline elapsed")]
    DeadlineExceeded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommonArticulationProfileBoundWholeParentPositiveThicknessUnpromotedReasonV2 {
    /// At least one non-topological face pair lacked a strict separating axis
    /// under the submitted positive thickness.
    StationaryPairEvidenceUnavailable,
}

/// Issuance consumes the unpromoted transport prerequisite so a successful
/// certificate retains the complete sealed source rather than merely copying
/// public digests from it.
pub struct CommonArticulationProfileBoundWholeParentPositiveThicknessInputV2<'a> {
    pub transport_prerequisite: CommonArticulationGeneralCellTransportPrerequisiteV2,
    pub live: CommonArticulationGeneralCellTransportRevalidationInputV2<'a>,
    pub parent_graph_admission: Arc<CommonArticulationPositiveThicknessParentGraphAdmissionV2>,
}

pub struct CommonArticulationProfileBoundWholeParentPositiveThicknessRevalidationInputV2<'a> {
    pub live: CommonArticulationGeneralCellTransportRevalidationInputV2<'a>,
}

#[derive(Debug)]
pub struct CommonArticulationProfileBoundWholeParentPositiveThicknessCertificateV2 {
    graph_proof: CommonArticulationAdmittedPositiveThicknessGraphProofV2,
    transport_prerequisite: CommonArticulationGeneralCellTransportPrerequisiteV2,
    parent_graph_admission: Arc<CommonArticulationPositiveThicknessParentGraphAdmissionV2>,
    profile_binding: [u8; 32],
    transport_binding: [u8; 32],
    source_digest: [u8; 32],
    identity_namespace: ProjectId,
    source_revision: u64,
    fold_model_fingerprint: [u8; 32],
    parent_fixed_face: FaceId,
    paper_thickness_bits: u64,
    actual_block_count: usize,
    face_count: usize,
    hinge_count: usize,
    analyzed_unordered_face_pairs: usize,
    transport_limits: CommonArticulationGeneralCellTransportLimitsV2,
    whole_parent_closure_limits: CommonArticulationWholeParentClosureLimitsV2,
    parent_graph_admission_limits: CommonArticulationPositiveThicknessParentGraphAdmissionLimitsV2,
    parent_graph_admission_resources:
        CommonArticulationPositiveThicknessParentGraphAdmissionResourcesV2,
    binding_fingerprint: [u8; 32],
}

impl CommonArticulationProfileBoundWholeParentPositiveThicknessCertificateV2 {
    #[must_use]
    pub const fn model_id_v2(&self) -> &'static str {
        COMMON_ARTICULATION_PROFILE_BOUND_WHOLE_PARENT_POSITIVE_THICKNESS_MODEL_ID_V2
    }

    #[must_use]
    pub const fn binding_fingerprint_v2(&self) -> [u8; 32] {
        self.binding_fingerprint
    }

    #[must_use]
    pub const fn profile_binding_fingerprint_v2(&self) -> [u8; 32] {
        self.profile_binding
    }

    #[must_use]
    pub const fn transport_binding_fingerprint_v2(&self) -> [u8; 32] {
        self.transport_binding
    }

    #[must_use]
    pub const fn source_digest_v2(&self) -> [u8; 32] {
        self.source_digest
    }

    #[must_use]
    pub const fn identity_namespace_v2(&self) -> ProjectId {
        self.identity_namespace
    }

    #[must_use]
    pub const fn actual_block_count_v2(&self) -> usize {
        self.actual_block_count
    }

    #[must_use]
    pub const fn analyzed_unordered_face_pairs_v2(&self) -> usize {
        self.analyzed_unordered_face_pairs
    }

    #[must_use]
    pub const fn stationary_whole_parent_positive_thickness_proven_v2(&self) -> bool {
        true
    }

    pub fn revalidate_v2(
        &self,
        input: CommonArticulationProfileBoundWholeParentPositiveThicknessRevalidationInputV2<'_>,
    ) -> Result<(), CommonArticulationProfileBoundWholeParentPositiveThicknessErrorV2> {
        self.revalidate_with_checkpoint_v2(input, || Ok(()))
    }

    pub fn revalidate_with_checkpoint_v2(
        &self,
        input: CommonArticulationProfileBoundWholeParentPositiveThicknessRevalidationInputV2<'_>,
        mut checkpoint: impl FnMut() -> Result<
            (),
            CommonArticulationProfileBoundWholeParentPositiveThicknessStopV2,
        >,
    ) -> Result<(), CommonArticulationProfileBoundWholeParentPositiveThicknessErrorV2> {
        checkpoint_v2(&mut checkpoint)?;
        revalidate_transport_v2(&self.transport_prerequisite, &input.live, &mut checkpoint)?;
        revalidate_parent_admission_v2(
            self.parent_graph_admission.as_ref(),
            input.live.geometry,
            &mut checkpoint,
        )?;
        let validated = validate_stationary_scope_v2(
            &self.transport_prerequisite,
            &input.live,
            self.parent_graph_admission.as_ref(),
        )?;
        if !self.graph_proof.is_for_v2(
            input.live.geometry,
            input.live.pose,
            input.live.paper_thickness_mm,
            self.parent_graph_admission.as_ref(),
        ) {
            return Err(
                CommonArticulationProfileBoundWholeParentPositiveThicknessErrorV2::CertificateBindingMismatch,
            );
        }
        let binding_fingerprint =
            certificate_binding_fingerprint_v2(&validated, &self.graph_proof, &mut checkpoint)?;
        checkpoint_v2(&mut checkpoint)?;
        let matches = self.profile_binding == validated.profile_binding
            && self.transport_binding == validated.transport_binding
            && self.source_digest == validated.source_digest
            && self.identity_namespace == validated.identity_namespace
            && self.source_revision == validated.source_revision
            && self.fold_model_fingerprint == validated.fold_model_fingerprint
            && self.parent_fixed_face == validated.parent_fixed_face
            && self.paper_thickness_bits == validated.paper_thickness_bits
            && self.actual_block_count == validated.actual_block_count
            && self.face_count == validated.face_count
            && self.hinge_count == validated.hinge_count
            && self.analyzed_unordered_face_pairs == validated.unordered_face_pairs
            && self.transport_limits == validated.transport_limits
            && self.whole_parent_closure_limits == validated.whole_parent_closure_limits
            && self.parent_graph_admission_limits == validated.parent_graph_admission_limits
            && self.parent_graph_admission_resources == validated.parent_graph_admission_resources
            && self.binding_fingerprint == binding_fingerprint;
        if !matches {
            checkpoint_v2(&mut checkpoint)?;
            return Err(
                CommonArticulationProfileBoundWholeParentPositiveThicknessErrorV2::CertificateBindingMismatch,
            );
        }
        checkpoint_v2(&mut checkpoint)
    }

    #[must_use]
    pub const fn authorizes_continuous_motion(&self) -> bool {
        false
    }
    #[must_use]
    pub const fn authorizes_collision_clearance(&self) -> bool {
        false
    }
    #[must_use]
    pub const fn authorizes_layer_transport(&self) -> bool {
        false
    }
    #[must_use]
    pub const fn authorizes_project_mutation(&self) -> bool {
        false
    }
    #[must_use]
    pub const fn authorizes_apply(&self) -> bool {
        false
    }
    #[must_use]
    pub const fn authorizes_viewer(&self) -> bool {
        false
    }
}

#[derive(Debug)]
pub enum CommonArticulationProfileBoundWholeParentPositiveThicknessOutcomeV2 {
    Proven(Box<CommonArticulationProfileBoundWholeParentPositiveThicknessCertificateV2>),
    Unpromoted {
        prerequisite: Box<CommonArticulationGeneralCellTransportPrerequisiteV2>,
        reason: CommonArticulationProfileBoundWholeParentPositiveThicknessUnpromotedReasonV2,
    },
}

impl CommonArticulationProfileBoundWholeParentPositiveThicknessOutcomeV2 {
    #[must_use]
    pub const fn model_id_v2(&self) -> &'static str {
        match self {
            Self::Proven(_) => {
                COMMON_ARTICULATION_PROFILE_BOUND_WHOLE_PARENT_POSITIVE_THICKNESS_MODEL_ID_V2
            }
            Self::Unpromoted { .. } => {
                COMMON_ARTICULATION_PROFILE_BOUND_WHOLE_PARENT_POSITIVE_THICKNESS_UNPROMOTED_MODEL_ID_V2
            }
        }
    }

    #[must_use]
    pub const fn is_proven_v2(&self) -> bool {
        matches!(self, Self::Proven(_))
    }

    #[must_use]
    pub fn as_proven_v2(
        &self,
    ) -> Option<&CommonArticulationProfileBoundWholeParentPositiveThicknessCertificateV2> {
        match self {
            Self::Proven(certificate) => Some(certificate.as_ref()),
            Self::Unpromoted { .. } => None,
        }
    }

    #[must_use]
    pub fn as_unpromoted_v2(
        &self,
    ) -> Option<(
        &CommonArticulationGeneralCellTransportPrerequisiteV2,
        CommonArticulationProfileBoundWholeParentPositiveThicknessUnpromotedReasonV2,
    )> {
        match self {
            Self::Proven(_) => None,
            Self::Unpromoted {
                prerequisite,
                reason,
            } => Some((prerequisite.as_ref(), *reason)),
        }
    }

    #[must_use]
    pub const fn authorizes_continuous_motion(&self) -> bool {
        false
    }
    #[must_use]
    pub const fn authorizes_collision_clearance(&self) -> bool {
        false
    }
    #[must_use]
    pub const fn authorizes_layer_transport(&self) -> bool {
        false
    }
    #[must_use]
    pub const fn authorizes_project_mutation(&self) -> bool {
        false
    }
    #[must_use]
    pub const fn authorizes_apply(&self) -> bool {
        false
    }
    #[must_use]
    pub const fn authorizes_viewer(&self) -> bool {
        false
    }
}

pub fn prove_common_articulation_profile_bound_whole_parent_positive_thickness_v2(
    input: CommonArticulationProfileBoundWholeParentPositiveThicknessInputV2<'_>,
) -> Result<
    CommonArticulationProfileBoundWholeParentPositiveThicknessOutcomeV2,
    CommonArticulationProfileBoundWholeParentPositiveThicknessErrorV2,
> {
    prove_common_articulation_profile_bound_whole_parent_positive_thickness_with_checkpoint_v2(
        input,
        || Ok(()),
    )
}

pub fn prove_common_articulation_profile_bound_whole_parent_positive_thickness_with_checkpoint_v2(
    input: CommonArticulationProfileBoundWholeParentPositiveThicknessInputV2<'_>,
    mut checkpoint: impl FnMut() -> Result<
        (),
        CommonArticulationProfileBoundWholeParentPositiveThicknessStopV2,
    >,
) -> Result<
    CommonArticulationProfileBoundWholeParentPositiveThicknessOutcomeV2,
    CommonArticulationProfileBoundWholeParentPositiveThicknessErrorV2,
> {
    checkpoint_v2(&mut checkpoint)?;
    revalidate_transport_v2(&input.transport_prerequisite, &input.live, &mut checkpoint)?;
    revalidate_parent_admission_v2(
        input.parent_graph_admission.as_ref(),
        input.live.geometry,
        &mut checkpoint,
    )?;
    let validated = validate_stationary_scope_v2(
        &input.transport_prerequisite,
        &input.live,
        input.parent_graph_admission.as_ref(),
    )?;
    checkpoint_v2(&mut checkpoint)?;
    let graph_proof = match prove_profile_bound_stationary_positive_thickness_graph_geometry_v2(
        input.live.geometry,
        input.live.pose,
        input.live.paper_thickness_mm,
        input.live.profile,
        input.parent_graph_admission.as_ref(),
        &mut checkpoint,
    ) {
        Ok(proof) => proof,
        Err(CheckpointedPositiveThicknessGraphProofErrorV2::Geometry(error)) => {
            let reason = classify_stationary_graph_failure_v2(
                CommonArticulationAdmittedPositiveThicknessGraphProofErrorV2::Geometry(error),
            )?;
            checkpoint_v2(&mut checkpoint)?;
            return Ok(
                CommonArticulationProfileBoundWholeParentPositiveThicknessOutcomeV2::Unpromoted {
                    prerequisite: Box::new(input.transport_prerequisite),
                    reason,
                },
            );
        }
        Err(CheckpointedPositiveThicknessGraphProofErrorV2::Stopped(stop)) => {
            return Err(match stop {
                CommonArticulationProfileBoundWholeParentPositiveThicknessStopV2::Cancelled => {
                    CommonArticulationProfileBoundWholeParentPositiveThicknessErrorV2::Cancelled
                }
                CommonArticulationProfileBoundWholeParentPositiveThicknessStopV2::DeadlineExceeded => {
                    CommonArticulationProfileBoundWholeParentPositiveThicknessErrorV2::DeadlineExceeded
                }
            });
        }
    };
    checkpoint_v2(&mut checkpoint)?;
    let binding_fingerprint =
        certificate_binding_fingerprint_v2(&validated, &graph_proof, &mut checkpoint)?;
    Ok(
        CommonArticulationProfileBoundWholeParentPositiveThicknessOutcomeV2::Proven(Box::new(
            CommonArticulationProfileBoundWholeParentPositiveThicknessCertificateV2 {
                graph_proof,
                transport_prerequisite: input.transport_prerequisite,
                parent_graph_admission: input.parent_graph_admission,
                profile_binding: validated.profile_binding,
                transport_binding: validated.transport_binding,
                source_digest: validated.source_digest,
                identity_namespace: validated.identity_namespace,
                source_revision: validated.source_revision,
                fold_model_fingerprint: validated.fold_model_fingerprint,
                parent_fixed_face: validated.parent_fixed_face,
                paper_thickness_bits: validated.paper_thickness_bits,
                actual_block_count: validated.actual_block_count,
                face_count: validated.face_count,
                hinge_count: validated.hinge_count,
                analyzed_unordered_face_pairs: validated.unordered_face_pairs,
                transport_limits: validated.transport_limits,
                whole_parent_closure_limits: validated.whole_parent_closure_limits,
                parent_graph_admission_limits: validated.parent_graph_admission_limits,
                parent_graph_admission_resources: validated.parent_graph_admission_resources,
                binding_fingerprint,
            },
        )),
    )
}

fn classify_stationary_graph_failure_v2(
    error: CommonArticulationAdmittedPositiveThicknessGraphProofErrorV2,
) -> Result<
    CommonArticulationProfileBoundWholeParentPositiveThicknessUnpromotedReasonV2,
    CommonArticulationProfileBoundWholeParentPositiveThicknessErrorV2,
> {
    match error {
        CommonArticulationAdmittedPositiveThicknessGraphProofErrorV2::Geometry(
            PositiveThicknessGraphProofErrorV1::PairEvidenceUnavailable,
        ) => Ok(
            CommonArticulationProfileBoundWholeParentPositiveThicknessUnpromotedReasonV2::StationaryPairEvidenceUnavailable,
        ),
        error => Err(
            CommonArticulationProfileBoundWholeParentPositiveThicknessErrorV2::PositiveThickness(
                error,
            ),
        ),
    }
}

struct ValidatedStationaryScopeV2 {
    profile_binding: [u8; 32],
    transport_binding: [u8; 32],
    source_digest: [u8; 32],
    identity_namespace: ProjectId,
    source_revision: u64,
    fold_model_fingerprint: [u8; 32],
    parent_fixed_face: FaceId,
    paper_thickness_bits: u64,
    actual_block_count: usize,
    face_count: usize,
    hinge_count: usize,
    unordered_face_pairs: usize,
    transport_limits: CommonArticulationGeneralCellTransportLimitsV2,
    whole_parent_closure_limits: CommonArticulationWholeParentClosureLimitsV2,
    parent_graph_admission_limits: CommonArticulationPositiveThicknessParentGraphAdmissionLimitsV2,
    parent_graph_admission_resources:
        CommonArticulationPositiveThicknessParentGraphAdmissionResourcesV2,
}

fn validate_stationary_scope_v2(
    transport: &CommonArticulationGeneralCellTransportPrerequisiteV2,
    live: &CommonArticulationGeneralCellTransportRevalidationInputV2<'_>,
    admission: &CommonArticulationPositiveThicknessParentGraphAdmissionV2,
) -> Result<
    ValidatedStationaryScopeV2,
    CommonArticulationProfileBoundWholeParentPositiveThicknessErrorV2,
> {
    let profile = live.profile;
    let maximum = profile.maximum_v2();
    let actual = profile.actual_v2();
    let configured_max_blocks = profile.configured_max_blocks_v2();
    let actual_block_count = profile.actual_block_count_v2();
    let face_count = live.geometry.face_ids().len();
    let hinge_count = live.geometry.hinges().len();
    let unordered_face_pairs = face_count
        .checked_mul(face_count.checked_sub(1).ok_or(
            CommonArticulationProfileBoundWholeParentPositiveThicknessErrorV2::ResourceLimit,
        )?)
        .and_then(|value| value.checked_div(2))
        .ok_or(CommonArticulationProfileBoundWholeParentPositiveThicknessErrorV2::ResourceLimit)?;
    let source = live.source_authority.provenance_v2();
    let identity_namespace = source
        .identity_namespace
        .ok_or(CommonArticulationProfileBoundWholeParentPositiveThicknessErrorV2::InvalidInput)?;
    let fold_model_fingerprint = source
        .source_fingerprint
        .map(|fingerprint| fingerprint.0)
        .ok_or(CommonArticulationProfileBoundWholeParentPositiveThicknessErrorV2::InvalidInput)?;
    let admission_limits = admission.limits_v2();
    let admission_resources = admission.resources_v2();
    if source.model_id != GlobalFlatFoldabilityModelId::ConvexFacesFacewiseV1
        || configured_max_blocks < GENERAL_N_MIN_BLOCKS_V2
        || actual_block_count < GENERAL_N_MIN_BLOCKS_V2
        || actual_block_count > configured_max_blocks
        || maximum.block_count_v2() != configured_max_blocks
        || actual.block_count_v2() != actual_block_count
        || face_count != actual.face_count_v2()
        || hinge_count != actual.hinge_count_v2()
        || unordered_face_pairs != actual.unordered_face_pair_count_v2()
        || face_count > maximum.face_count_v2()
        || hinge_count > maximum.hinge_count_v2()
        || unordered_face_pairs > maximum.unordered_face_pair_count_v2()
        || live.limits.max_blocks != configured_max_blocks
        || transport.actual_block_count_v2() != actual_block_count
        || !live.source_authority.is_current_v2()
        || live.geometry.source_identity_namespace_v1() != Some(identity_namespace)
        || live.geometry.source_revision_v1() != Some(source.source_revision)
        || live.geometry.fold_model_fingerprint_v1() != Some(fold_model_fingerprint)
        || admission.identity_namespace_v2() != identity_namespace
        || admission.source_revision_v2() != source.source_revision
        || admission.fold_model_fingerprint_v2() != fold_model_fingerprint
        || !admission.matches_geometry_instance_v2(live.geometry)
        || admission_limits.max_faces != maximum.face_count_v2()
        || admission_limits.max_hinges != maximum.hinge_count_v2()
        || admission_limits.max_face_pair_tests != maximum.unordered_face_pair_count_v2()
        || admission_resources.face_count_v2() != face_count
        || admission_resources.hinge_count_v2() != hinge_count
        || admission_resources.face_pair_tests_v2() != unordered_face_pairs
        || live.pose.fixed_face() != live.parent_fixed_face
        || live.pose.hinge_angles().as_slice().len() != hinge_count
        || live
            .pose
            .hinge_angles()
            .as_slice()
            .iter()
            .any(|angle| angle.angle_degrees().to_bits() != 0.0_f64.to_bits())
        || live.whole_parent_closure.parent_closure_leaves_v2() != 1
        || !live.paper_thickness_mm.is_finite()
        || live.paper_thickness_mm <= 0.0
    {
        return Err(
            CommonArticulationProfileBoundWholeParentPositiveThicknessErrorV2::ResourceLimit,
        );
    }
    let source_angles = live
        .parent_schedule
        .evaluate(0.0)
        .ok_or(CommonArticulationProfileBoundWholeParentPositiveThicknessErrorV2::InvalidInput)?;
    let target_angles = live
        .parent_schedule
        .evaluate(1.0)
        .ok_or(CommonArticulationProfileBoundWholeParentPositiveThicknessErrorV2::InvalidInput)?;
    if source_angles.as_slice().len() != hinge_count
        || target_angles.as_slice().len() != hinge_count
        || source_angles
            .as_slice()
            .iter()
            .chain(target_angles.as_slice())
            .any(|angle| angle.angle_degrees().to_bits() != 0.0_f64.to_bits())
        || live.geometry.hinges().iter().any(|hinge| {
            live.parent_schedule
                .derivative_bound(hinge.edge())
                .is_none_or(|bound| bound.to_bits() != 0.0_f64.to_bits())
        })
    {
        return Err(
            CommonArticulationProfileBoundWholeParentPositiveThicknessErrorV2::InvalidInput,
        );
    }
    Ok(ValidatedStationaryScopeV2 {
        profile_binding: profile.binding_fingerprint_v2(),
        transport_binding: transport.binding_fingerprint_v2(),
        source_digest: transport.source_digest_v2(),
        identity_namespace,
        source_revision: source.source_revision,
        fold_model_fingerprint,
        parent_fixed_face: live.parent_fixed_face,
        paper_thickness_bits: live.paper_thickness_mm.to_bits(),
        actual_block_count,
        face_count,
        hinge_count,
        unordered_face_pairs,
        transport_limits: live.limits,
        whole_parent_closure_limits: live.whole_parent_closure_limits,
        parent_graph_admission_limits: admission_limits,
        parent_graph_admission_resources: admission_resources,
    })
}

fn revalidate_transport_v2(
    transport: &CommonArticulationGeneralCellTransportPrerequisiteV2,
    live: &CommonArticulationGeneralCellTransportRevalidationInputV2<'_>,
    checkpoint: &mut impl FnMut() -> Result<
        (),
        CommonArticulationProfileBoundWholeParentPositiveThicknessStopV2,
    >,
) -> Result<(), CommonArticulationProfileBoundWholeParentPositiveThicknessErrorV2> {
    let mut transport_checkpoint = || {
        checkpoint().map_err(|stop| match stop {
            CommonArticulationProfileBoundWholeParentPositiveThicknessStopV2::Cancelled => {
                CommonArticulationGeneralCellTransportStopV2::Cancelled
            }
            CommonArticulationProfileBoundWholeParentPositiveThicknessStopV2::DeadlineExceeded => {
                CommonArticulationGeneralCellTransportStopV2::DeadlineExceeded
            }
        })
    };
    transport
        .revalidate_borrowed_with_checkpoint_v2(live, &mut transport_checkpoint)
        .map_err(map_transport_error_v2)
}

fn revalidate_parent_admission_v2(
    admission: &CommonArticulationPositiveThicknessParentGraphAdmissionV2,
    geometry: &ori_kinematics::MaterialHingeGraphGeometry,
    checkpoint: &mut impl FnMut() -> Result<
        (),
        CommonArticulationProfileBoundWholeParentPositiveThicknessStopV2,
    >,
) -> Result<(), CommonArticulationProfileBoundWholeParentPositiveThicknessErrorV2> {
    revalidate_common_articulation_positive_thickness_parent_graph_admission_with_checkpoint_v2(
        admission,
        geometry,
        || {
            checkpoint().map_err(|stop| match stop {
                CommonArticulationProfileBoundWholeParentPositiveThicknessStopV2::Cancelled => {
                    CommonArticulationPositiveThicknessParentGraphAdmissionStopV2::Cancelled
                }
                CommonArticulationProfileBoundWholeParentPositiveThicknessStopV2::DeadlineExceeded => {
                    CommonArticulationPositiveThicknessParentGraphAdmissionStopV2::DeadlineExceeded
                }
            })
        },
    )
    .map_err(map_parent_admission_error_v2)
}

fn map_transport_error_v2(
    error: CommonArticulationGeneralCellTransportErrorV2,
) -> CommonArticulationProfileBoundWholeParentPositiveThicknessErrorV2 {
    match error {
        CommonArticulationGeneralCellTransportErrorV2::Cancelled => {
            CommonArticulationProfileBoundWholeParentPositiveThicknessErrorV2::Cancelled
        }
        CommonArticulationGeneralCellTransportErrorV2::DeadlineExceeded => {
            CommonArticulationProfileBoundWholeParentPositiveThicknessErrorV2::DeadlineExceeded
        }
        error => {
            CommonArticulationProfileBoundWholeParentPositiveThicknessErrorV2::Transport(error)
        }
    }
}

fn map_parent_admission_error_v2(
    error: CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2,
) -> CommonArticulationProfileBoundWholeParentPositiveThicknessErrorV2 {
    match error {
        CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::Cancelled => {
            CommonArticulationProfileBoundWholeParentPositiveThicknessErrorV2::Cancelled
        }
        CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::DeadlineExceeded => {
            CommonArticulationProfileBoundWholeParentPositiveThicknessErrorV2::DeadlineExceeded
        }
        error => {
            CommonArticulationProfileBoundWholeParentPositiveThicknessErrorV2::ParentAdmission(
                error,
            )
        }
    }
}

fn checkpoint_v2(
    checkpoint: &mut impl FnMut() -> Result<
        (),
        CommonArticulationProfileBoundWholeParentPositiveThicknessStopV2,
    >,
) -> Result<(), CommonArticulationProfileBoundWholeParentPositiveThicknessErrorV2> {
    checkpoint().map_err(|stop| match stop {
        CommonArticulationProfileBoundWholeParentPositiveThicknessStopV2::Cancelled => {
            CommonArticulationProfileBoundWholeParentPositiveThicknessErrorV2::Cancelled
        }
        CommonArticulationProfileBoundWholeParentPositiveThicknessStopV2::DeadlineExceeded => {
            CommonArticulationProfileBoundWholeParentPositiveThicknessErrorV2::DeadlineExceeded
        }
    })
}

fn certificate_binding_fingerprint_v2(
    value: &ValidatedStationaryScopeV2,
    graph_proof: &CommonArticulationAdmittedPositiveThicknessGraphProofV2,
    checkpoint: &mut impl FnMut() -> Result<
        (),
        CommonArticulationProfileBoundWholeParentPositiveThicknessStopV2,
    >,
) -> Result<[u8; 32], CommonArticulationProfileBoundWholeParentPositiveThicknessErrorV2> {
    let mut hash = Sha256::new();
    hash.update(COMMON_ARTICULATION_PROFILE_BOUND_WHOLE_PARENT_POSITIVE_THICKNESS_MODEL_ID_V2);
    for binding in [
        value.profile_binding,
        value.transport_binding,
        value.source_digest,
        graph_proof.parent_admission_binding_fingerprint_v2(),
        graph_proof.parent_semantic_graph_digest_v2(),
    ] {
        checkpoint_v2(checkpoint)?;
        hash.update(binding);
    }
    hash.update(value.identity_namespace.canonical_bytes());
    hash.update(value.source_revision.to_le_bytes());
    hash.update(value.fold_model_fingerprint);
    hash.update(value.parent_fixed_face.canonical_bytes());
    hash.update(value.paper_thickness_bits.to_le_bytes());
    for number in [
        value.actual_block_count,
        value.face_count,
        value.hinge_count,
        value.unordered_face_pairs,
        graph_proof.analyzed_unordered_face_pairs_v2(),
    ] {
        hash.update(
            u64::try_from(number)
                .map_err(|_| {
                    CommonArticulationProfileBoundWholeParentPositiveThicknessErrorV2::ResourceLimit
                })?
                .to_le_bytes(),
        );
    }
    for number in [
        value.parent_graph_admission_resources.face_count_v2(),
        value.parent_graph_admission_resources.hinge_count_v2(),
        value.parent_graph_admission_resources.face_pair_tests_v2(),
    ] {
        hash.update(
            u64::try_from(number)
                .map_err(|_| {
                    CommonArticulationProfileBoundWholeParentPositiveThicknessErrorV2::ResourceLimit
                })?
                .to_le_bytes(),
        );
    }
    checkpoint_v2(checkpoint)?;
    Ok(hash.finalize().into())
}

#[cfg(test)]
mod tests;
