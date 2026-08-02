//! Positive-thickness graph proof gated by exact parent-topology admission.
//!
//! The legacy V1 entry point deliberately rejects every shared feature.  This
//! module is the only boundary that may classify such a pair as an allowed
//! material-topology contact, and only while holding a live opaque V2 parent
//! admission for the same geometry instance.

use std::sync::Arc;

use ori_domain::FaceId;
use ori_kinematics::{
    CanonicalCycleScheduleV1, ClosedMaterialHingeGraphPose, CommonArticulationResourceProfileV2,
    DyadicMaterialHingeIntervalClosureCertificateV1, MaterialHingeGraphAudit,
    MaterialHingeGraphGeometry, MaterialHingeGraphInstanceV1,
};
use thiserror::Error;

use super::{
    COMMON_ARTICULATION_POSITIVE_THICKNESS_GRAPH_EXTENSION_MAX_FACES_V1,
    CheckpointedPositiveThicknessGraphProofErrorV2,
    CommonArticulationPositiveThicknessGraphExtensionLimitsV1,
    CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2,
    CommonArticulationPositiveThicknessParentGraphAdmissionV2,
    NativePositiveThicknessGraphGeometryProofV1, PositiveThicknessGraphLimitsV1,
    PositiveThicknessGraphProofErrorV1, PositiveThicknessSharedContactScopeV2,
    prove_positive_thickness_graph_geometry_with_max_faces_and_admission_checkpointed_v2,
    prove_positive_thickness_graph_geometry_with_max_faces_and_admission_v2,
    revalidate_common_articulation_positive_thickness_parent_graph_admission_v2,
    validate_common_articulation_positive_thickness_graph_extension_limits_v1,
};

pub const COMMON_ARTICULATION_ADMITTED_POSITIVE_THICKNESS_GRAPH_PROOF_MODEL_ID_V2: &str =
    "common_articulation_admitted_positive_thickness_graph_geometry_proof_v2";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum CommonArticulationAdmittedPositiveThicknessGraphProofErrorV2 {
    #[error("the exact parent-graph admission failed: {0}")]
    ParentAdmission(CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2),
    #[error("the admitted positive-thickness graph proof failed: {0:?}")]
    Geometry(PositiveThicknessGraphProofErrorV1),
    #[error("the shared-feature contact evidence is absent or does not bind the live graph")]
    SharedContactEvidenceUnavailable,
}

/// Opaque proof that every shared-feature contact remains in the exact flat
/// rest-sheet class throughout the submitted schedule.  This is the minimal
/// no-relief theorem: any non-zero hinge schedule must instead supply a future
/// explicit hinge/vertex relief certificate and is rejected here.
#[derive(Debug, Clone)]
pub struct CommonArticulationAdmittedSharedFeatureContactCertificateV2 {
    geometry_instance: MaterialHingeGraphInstanceV1,
    parent_graph_admission: Arc<CommonArticulationPositiveThicknessParentGraphAdmissionV2>,
    schedule_binding: [u8; 32],
    closure_binding: [u8; 32],
    fixed_face: FaceId,
    shared_feature_pair_count: usize,
    graph_limits: CommonArticulationPositiveThicknessGraphExtensionLimitsV1,
}

pub(crate) struct AdmittedSharedFeatureContactRevalidationInputV2<'a> {
    pub geometry: &'a MaterialHingeGraphGeometry,
    pub audit: &'a MaterialHingeGraphAudit,
    pub fixed_face: FaceId,
    pub schedule: &'a CanonicalCycleScheduleV1,
    pub closure: &'a DyadicMaterialHingeIntervalClosureCertificateV1,
    pub graph_limits: CommonArticulationPositiveThicknessGraphExtensionLimitsV1,
    pub admission: &'a CommonArticulationPositiveThicknessParentGraphAdmissionV2,
}

pub struct CommonArticulationAdmittedSharedFeatureContactInputV2<'a> {
    pub geometry: &'a MaterialHingeGraphGeometry,
    pub audit: &'a MaterialHingeGraphAudit,
    pub fixed_face: FaceId,
    pub schedule: &'a CanonicalCycleScheduleV1,
    pub closure: &'a DyadicMaterialHingeIntervalClosureCertificateV1,
    pub graph_limits: CommonArticulationPositiveThicknessGraphExtensionLimitsV1,
    pub admission: Arc<CommonArticulationPositiveThicknessParentGraphAdmissionV2>,
}

impl CommonArticulationAdmittedSharedFeatureContactCertificateV2 {
    #[must_use]
    pub fn parent_graph_admission_binding_fingerprint_v2(&self) -> [u8; 32] {
        self.parent_graph_admission.binding_fingerprint_v2()
    }

    #[must_use]
    pub const fn shared_feature_pair_count_v2(&self) -> usize {
        self.shared_feature_pair_count
    }

    pub(crate) fn retained_parent_graph_admission_v2(
        &self,
    ) -> &CommonArticulationPositiveThicknessParentGraphAdmissionV2 {
        self.parent_graph_admission.as_ref()
    }

    pub(crate) const fn graph_limits_v2(
        &self,
    ) -> CommonArticulationPositiveThicknessGraphExtensionLimitsV1 {
        self.graph_limits
    }

    pub(crate) fn proves_shared_contact_for_pose_v2(
        &self,
        geometry: &MaterialHingeGraphGeometry,
        pose: &ClosedMaterialHingeGraphPose,
    ) -> bool {
        self.geometry_instance.matches(geometry)
            && self
                .parent_graph_admission
                .matches_geometry_instance_v2(geometry)
            && pose.is_for_geometry(geometry)
            && pose.fixed_face() == self.fixed_face
            && pose.hinge_angles().as_slice().len() == geometry.hinges().len()
            && pose
                .hinge_angles()
                .as_slice()
                .iter()
                .all(|angle| angle.angle_degrees().to_bits() == 0.0_f64.to_bits())
    }

    pub(crate) fn is_for_live_schedule_v2(
        &self,
        input: AdmittedSharedFeatureContactRevalidationInputV2<'_>,
    ) -> bool {
        if !self.geometry_instance.matches(input.geometry)
            || !self
                .parent_graph_admission
                .same_evidence_v2(input.admission)
            || !input.admission.matches_geometry_instance_v2(input.geometry)
            || self.schedule_binding != input.schedule.certificate_binding_fingerprint_v2()
            || self.closure_binding != input.closure.partition_binding_fingerprint_v2()
            || self.fixed_face != input.fixed_face
            || self.graph_limits != input.graph_limits
            || !input
                .schedule
                .matches_binding(input.geometry, input.audit, input.fixed_face)
            || input.closure.fixed_face() != input.fixed_face
            || input.closure.schedule_binding_fingerprint_v2()
                != input.schedule.certificate_binding_fingerprint_v2()
            || input.closure.graph_binding_fingerprint_v1()
                != input.schedule.graph_binding_fingerprint_v1()
            || !input.closure.every_leaf_covers_graph_v1(input.geometry)
        {
            return false;
        }
        let Some(source) = input.schedule.evaluate(0.0) else {
            return false;
        };
        let Some(target) = input.schedule.evaluate(1.0) else {
            return false;
        };
        source.as_slice().len() == input.geometry.hinges().len()
            && target.as_slice().len() == input.geometry.hinges().len()
            && source
                .as_slice()
                .iter()
                .chain(target.as_slice())
                .all(|angle| angle.angle_degrees().to_bits() == 0.0_f64.to_bits())
            && input.geometry.hinges().iter().all(|hinge| {
                input
                    .schedule
                    .derivative_bound(hinge.edge())
                    .is_some_and(|bound| bound.to_bits() == 0.0_f64.to_bits())
            })
    }
}

/// Proves the flat shared-contact theorem and retains the exact admission.
pub fn certify_common_articulation_admitted_flat_shared_feature_contacts_v2(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    schedule: &CanonicalCycleScheduleV1,
    closure: &DyadicMaterialHingeIntervalClosureCertificateV1,
    graph_limits: CommonArticulationPositiveThicknessGraphExtensionLimitsV1,
    admission: Arc<CommonArticulationPositiveThicknessParentGraphAdmissionV2>,
) -> Result<
    CommonArticulationAdmittedSharedFeatureContactCertificateV2,
    CommonArticulationAdmittedPositiveThicknessGraphProofErrorV2,
> {
    certify_common_articulation_admitted_flat_shared_feature_contacts_with_checkpoint_v2(
        CommonArticulationAdmittedSharedFeatureContactInputV2 {
            geometry,
            audit,
            fixed_face,
            schedule,
            closure,
            graph_limits,
            admission,
        },
        || Ok(()),
    )
}

/// Cooperative form of the exact flat shared-contact proof. Every face and
/// unordered face-pair batch is interruptible, and the fixed 257-face V1
/// extension envelope is validated before any quadratic scan.
pub fn certify_common_articulation_admitted_flat_shared_feature_contacts_with_checkpoint_v2<F>(
    input: CommonArticulationAdmittedSharedFeatureContactInputV2<'_>,
    mut checkpoint: F,
) -> Result<
    CommonArticulationAdmittedSharedFeatureContactCertificateV2,
    CommonArticulationAdmittedPositiveThicknessGraphProofErrorV2,
>
where
    F: FnMut() -> Result<(), super::CommonArticulationPositiveThicknessParentGraphAdmissionStopV2>,
{
    let mut poll = || {
        checkpoint().map_err(|stop| {
            CommonArticulationAdmittedPositiveThicknessGraphProofErrorV2::ParentAdmission(
                match stop {
                    super::CommonArticulationPositiveThicknessParentGraphAdmissionStopV2::Cancelled => super::CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::Cancelled,
                    super::CommonArticulationPositiveThicknessParentGraphAdmissionStopV2::DeadlineExceeded => super::CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::DeadlineExceeded,
                },
            )
        })
    };
    poll()?;
    validate_common_articulation_positive_thickness_graph_extension_limits_v1(input.graph_limits)
        .map_err(CommonArticulationAdmittedPositiveThicknessGraphProofErrorV2::Geometry)?;
    let face_count = input.geometry.face_ids().len();
    if !(3..=COMMON_ARTICULATION_POSITIVE_THICKNESS_GRAPH_EXTENSION_MAX_FACES_V1)
        .contains(&face_count)
    {
        return Err(
            CommonArticulationAdmittedPositiveThicknessGraphProofErrorV2::Geometry(
                PositiveThicknessGraphProofErrorV1::ResourceLimit,
            ),
        );
    }
    if !input.admission.matches_geometry_instance_v2(input.geometry)
        || !input
            .schedule
            .matches_binding(input.geometry, input.audit, input.fixed_face)
        || input.closure.fixed_face() != input.fixed_face
        || input.closure.schedule_binding_fingerprint_v2()
            != input.schedule.certificate_binding_fingerprint_v2()
        || input.closure.graph_binding_fingerprint_v1()
            != input.schedule.graph_binding_fingerprint_v1()
        || !input.closure.every_leaf_covers_graph_v1(input.geometry)
    {
        return Err(
            CommonArticulationAdmittedPositiveThicknessGraphProofErrorV2::SharedContactEvidenceUnavailable,
        );
    }
    let source = input.schedule.evaluate(0.0).ok_or(
        CommonArticulationAdmittedPositiveThicknessGraphProofErrorV2::SharedContactEvidenceUnavailable,
    )?;
    let target = input.schedule.evaluate(1.0).ok_or(
        CommonArticulationAdmittedPositiveThicknessGraphProofErrorV2::SharedContactEvidenceUnavailable,
    )?;
    if source.as_slice().len() != input.geometry.hinges().len()
        || target.as_slice().len() != input.geometry.hinges().len()
        || source
            .as_slice()
            .iter()
            .chain(target.as_slice())
            .any(|angle| angle.angle_degrees().to_bits() != 0.0_f64.to_bits())
        || input.geometry.hinges().iter().any(|hinge| {
            input
                .schedule
                .derivative_bound(hinge.edge())
                .is_none_or(|bound| bound.to_bits() != 0.0_f64.to_bits())
        })
    {
        return Err(
            CommonArticulationAdmittedPositiveThicknessGraphProofErrorV2::SharedContactEvidenceUnavailable,
        );
    }
    let unordered_pairs = face_count
        .checked_mul(face_count.saturating_sub(1))
        .and_then(|count| count.checked_div(2))
        .filter(|count| *count <= input.graph_limits.max_unordered_face_pairs)
        .ok_or(
            CommonArticulationAdmittedPositiveThicknessGraphProofErrorV2::Geometry(
                PositiveThicknessGraphProofErrorV1::ResourceLimit,
            ),
        )?;
    let mut shared_feature_pair_count = 0usize;
    for first_index in 0..face_count {
        poll()?;
        let first_boundary = input
            .geometry
            .face_boundary_vertices(input.geometry.face_ids()[first_index])
            .ok_or(
                CommonArticulationAdmittedPositiveThicknessGraphProofErrorV2::Geometry(
                    PositiveThicknessGraphProofErrorV1::InvalidInput,
                ),
            )?;
        if first_boundary.len() > 64 {
            return Err(
                CommonArticulationAdmittedPositiveThicknessGraphProofErrorV2::Geometry(
                    PositiveThicknessGraphProofErrorV1::ResourceLimit,
                ),
            );
        }
        for second in &input.geometry.face_ids()[first_index + 1..] {
            poll()?;
            let second_boundary = input.geometry.face_boundary_vertices(*second).ok_or(
                CommonArticulationAdmittedPositiveThicknessGraphProofErrorV2::Geometry(
                    PositiveThicknessGraphProofErrorV1::InvalidInput,
                ),
            )?;
            if second_boundary.len() > 64 {
                return Err(
                    CommonArticulationAdmittedPositiveThicknessGraphProofErrorV2::Geometry(
                        PositiveThicknessGraphProofErrorV1::ResourceLimit,
                    ),
                );
            }
            if first_boundary
                .iter()
                .any(|vertex| second_boundary.contains(vertex))
            {
                shared_feature_pair_count = shared_feature_pair_count
                    .checked_add(1)
                    .filter(|count| *count <= input.graph_limits.max_shared_feature_pairs)
                    .ok_or(
                        CommonArticulationAdmittedPositiveThicknessGraphProofErrorV2::Geometry(
                            PositiveThicknessGraphProofErrorV1::ResourceLimit,
                        ),
                    )?;
            }
        }
    }
    debug_assert_eq!(unordered_pairs, face_count * (face_count - 1) / 2);
    Ok(
        CommonArticulationAdmittedSharedFeatureContactCertificateV2 {
            geometry_instance: input.geometry.instance_anchor_v1(),
            parent_graph_admission: input.admission,
            schedule_binding: input.schedule.certificate_binding_fingerprint_v2(),
            closure_binding: input.closure.partition_binding_fingerprint_v2(),
            fixed_face: input.fixed_face,
            shared_feature_pair_count,
            graph_limits: input.graph_limits,
        },
    )
}

/// Opaque pose proof whose shared-feature classification is bound to one
/// exact, process-local parent-graph admission.
#[derive(Debug)]
pub struct CommonArticulationAdmittedPositiveThicknessGraphProofV2 {
    inner: NativePositiveThicknessGraphGeometryProofV1,
    parent_admission_binding: [u8; 32],
    parent_semantic_graph_digest: [u8; 32],
}

impl CommonArticulationAdmittedPositiveThicknessGraphProofV2 {
    #[must_use]
    pub const fn model_id_v2(&self) -> &'static str {
        COMMON_ARTICULATION_ADMITTED_POSITIVE_THICKNESS_GRAPH_PROOF_MODEL_ID_V2
    }

    #[must_use]
    pub const fn parent_admission_binding_fingerprint_v2(&self) -> [u8; 32] {
        self.parent_admission_binding
    }

    #[must_use]
    pub const fn parent_semantic_graph_digest_v2(&self) -> [u8; 32] {
        self.parent_semantic_graph_digest
    }

    #[must_use]
    pub const fn analyzed_unordered_face_pairs_v2(&self) -> usize {
        self.inner.analyzed_unordered_face_pairs()
    }

    #[must_use]
    pub fn is_for_v2(
        &self,
        geometry: &MaterialHingeGraphGeometry,
        pose: &ClosedMaterialHingeGraphPose,
        paper_thickness_mm: f64,
        admission: &CommonArticulationPositiveThicknessParentGraphAdmissionV2,
    ) -> bool {
        admission.matches_geometry_instance_v2(geometry)
            && self.parent_admission_binding == admission.binding_fingerprint_v2()
            && self.parent_semantic_graph_digest == admission.semantic_graph_digest_v2()
            && self
                .inner
                .is_for_geometry(geometry, pose, paper_thickness_mm)
    }
}

/// Issues a V2 graph proof after repeating the complete exact parent-admission
/// scan.  Continuous-path issuance uses the crate-private prevalidated form so
/// the immutable parent graph is scanned once per outer operation, not once
/// per sampled pose.
pub fn prove_common_articulation_admitted_positive_thickness_graph_geometry_v2(
    geometry: &MaterialHingeGraphGeometry,
    pose: &ClosedMaterialHingeGraphPose,
    paper_thickness_mm: f64,
    limits: CommonArticulationPositiveThicknessGraphExtensionLimitsV1,
    admission: &CommonArticulationPositiveThicknessParentGraphAdmissionV2,
    shared_contact: &CommonArticulationAdmittedSharedFeatureContactCertificateV2,
) -> Result<
    CommonArticulationAdmittedPositiveThicknessGraphProofV2,
    CommonArticulationAdmittedPositiveThicknessGraphProofErrorV2,
> {
    revalidate_common_articulation_positive_thickness_parent_graph_admission_v2(
        admission, geometry,
    )
    .map_err(CommonArticulationAdmittedPositiveThicknessGraphProofErrorV2::ParentAdmission)?;
    if !admission.same_evidence_v2(shared_contact.retained_parent_graph_admission_v2()) {
        return Err(
            CommonArticulationAdmittedPositiveThicknessGraphProofErrorV2::SharedContactEvidenceUnavailable,
        );
    }
    prove_common_articulation_admitted_positive_thickness_graph_geometry_prevalidated_v2(
        geometry,
        pose,
        paper_thickness_mm,
        limits,
        shared_contact,
    )
    .map_err(CommonArticulationAdmittedPositiveThicknessGraphProofErrorV2::Geometry)
}

pub(crate) fn prove_common_articulation_admitted_positive_thickness_graph_geometry_prevalidated_v2(
    geometry: &MaterialHingeGraphGeometry,
    pose: &ClosedMaterialHingeGraphPose,
    paper_thickness_mm: f64,
    limits: CommonArticulationPositiveThicknessGraphExtensionLimitsV1,
    shared_contact: &CommonArticulationAdmittedSharedFeatureContactCertificateV2,
) -> Result<
    CommonArticulationAdmittedPositiveThicknessGraphProofV2,
    PositiveThicknessGraphProofErrorV1,
> {
    let admission = shared_contact.retained_parent_graph_admission_v2();
    if !admission.matches_geometry_instance_v2(geometry)
        || !shared_contact.proves_shared_contact_for_pose_v2(geometry, pose)
    {
        return Err(PositiveThicknessGraphProofErrorV1::InvalidInput);
    }
    let inner = prove_positive_thickness_graph_geometry_with_max_faces_and_admission_v2(
        geometry,
        pose,
        paper_thickness_mm,
        PositiveThicknessGraphLimitsV1 {
            max_unordered_face_pairs: limits.max_unordered_face_pairs,
            max_shared_feature_pairs: limits.max_shared_feature_pairs,
        },
        COMMON_ARTICULATION_POSITIVE_THICKNESS_GRAPH_EXTENSION_MAX_FACES_V1,
        Some(PositiveThicknessSharedContactScopeV2::Continuous(
            shared_contact,
        )),
    )?;
    Ok(CommonArticulationAdmittedPositiveThicknessGraphProofV2 {
        inner,
        parent_admission_binding: admission.binding_fingerprint_v2(),
        parent_semantic_graph_digest: admission.semantic_graph_digest_v2(),
    })
}

/// Crate-private adapter for the canonical general-N stationary boundary.
///
/// This deliberately does not widen the public 257-face extension contract.
/// The configured and actual pair ceilings come from the sealed V2 profile,
/// while the exact parent admission must carry matching configured face,
/// hinge, and face-pair limits and matching actual observations. Shared
/// topology contacts are admitted only at the bit-identical zero pose.
pub(crate) fn prove_profile_bound_stationary_positive_thickness_graph_geometry_v2<S>(
    geometry: &MaterialHingeGraphGeometry,
    pose: &ClosedMaterialHingeGraphPose,
    paper_thickness_mm: f64,
    profile: &CommonArticulationResourceProfileV2,
    admission: &CommonArticulationPositiveThicknessParentGraphAdmissionV2,
    checkpoint: &mut impl FnMut() -> Result<(), S>,
) -> Result<
    CommonArticulationAdmittedPositiveThicknessGraphProofV2,
    CheckpointedPositiveThicknessGraphProofErrorV2<S>,
> {
    const GENERAL_N_MIN_BLOCKS_V2: usize = 33;

    let configured_max_blocks = profile.configured_max_blocks_v2();
    let actual_block_count = profile.actual_block_count_v2();
    let maximum = profile.maximum_v2();
    let actual = profile.actual_v2();
    let admission_limits = admission.limits_v2();
    let admission_resources = admission.resources_v2();
    if configured_max_blocks < GENERAL_N_MIN_BLOCKS_V2
        || actual_block_count < GENERAL_N_MIN_BLOCKS_V2
        || actual_block_count > configured_max_blocks
        || maximum.block_count_v2() != configured_max_blocks
        || actual.block_count_v2() != actual_block_count
        || geometry.face_ids().len() != actual.face_count_v2()
        || geometry.hinges().len() != actual.hinge_count_v2()
        || actual.face_count_v2() > maximum.face_count_v2()
        || actual.hinge_count_v2() > maximum.hinge_count_v2()
        || actual.unordered_face_pair_count_v2() > maximum.unordered_face_pair_count_v2()
        || admission_limits.max_faces != maximum.face_count_v2()
        || admission_limits.max_hinges != maximum.hinge_count_v2()
        || admission_limits.max_face_pair_tests != maximum.unordered_face_pair_count_v2()
        || admission_resources.face_count_v2() != actual.face_count_v2()
        || admission_resources.hinge_count_v2() != actual.hinge_count_v2()
        || admission_resources.face_pair_tests_v2() != actual.unordered_face_pair_count_v2()
        || !admission.matches_geometry_instance_v2(geometry)
    {
        return Err(CheckpointedPositiveThicknessGraphProofErrorV2::Geometry(
            PositiveThicknessGraphProofErrorV1::ResourceLimit,
        ));
    }

    let inner =
        prove_positive_thickness_graph_geometry_with_max_faces_and_admission_checkpointed_v2(
            geometry,
            pose,
            paper_thickness_mm,
            PositiveThicknessGraphLimitsV1 {
                max_unordered_face_pairs: maximum.unordered_face_pair_count_v2(),
                max_shared_feature_pairs: maximum.unordered_face_pair_count_v2(),
            },
            maximum.face_count_v2(),
            Some(PositiveThicknessSharedContactScopeV2::StationaryParent(
                admission,
            )),
            checkpoint,
        )?;
    Ok(CommonArticulationAdmittedPositiveThicknessGraphProofV2 {
        inner,
        parent_admission_binding: admission.binding_fingerprint_v2(),
        parent_semantic_graph_digest: admission.semantic_graph_digest_v2(),
    })
}
