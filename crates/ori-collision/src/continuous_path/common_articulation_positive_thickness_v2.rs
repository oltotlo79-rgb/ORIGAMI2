//! Parent-admitted positive-thickness certificate for the 11..=32 extension.
//!
//! V1 intentionally remains fail-closed for shared material features.  This
//! V2 boundary retains the opaque exact parent admission and is therefore the
//! only common-articulation certificate allowed to classify those contacts as
//! topology rather than collision clearance.

use std::{mem::size_of, sync::Arc};

use ori_domain::FaceId;
use ori_kinematics::{
    CanonicalMaterialEdgeBlockDecompositionV1, DyadicMaterialHingeIntervalClosureCertificateV1,
    MaterialHingeGraphAudit, MaterialHingeGraphGeometry,
};
use sha2::{Digest, Sha256};

use super::{
    COMMON_ARTICULATION_POSE_EXTENSION_MIN_BLOCKS_V1,
    CanonicalPositiveThicknessCyclePathControlErrorV1, CommonArticulationPositiveThicknessScopeV1,
    PositiveThicknessContinuousCertificateV1, PositiveThicknessGraphProofScopeV1,
    canonical_positive_cycle_checkpoint_v1,
    certify_canonical_positive_thickness_cycle_schedule_path_in_scope_with_control_v1,
    common_articulation_positive_thickness_graph_scope_v1,
};
use crate::graph_positive_thickness::AdmittedSharedFeatureContactRevalidationInputV2;
use crate::{
    CommonArticulationAdmittedSharedFeatureContactCertificateV2,
    CommonArticulationPositiveThicknessGraphExtensionLimitsV1,
    CommonArticulationPositiveThicknessParentGraphAdmissionLimitsV2,
    CommonArticulationPositiveThicknessParentGraphAdmissionResourcesV2,
    CommonArticulationPositiveThicknessParentGraphAdmissionV2, CooperativeOperationControlV1,
};

pub const COMMON_ARTICULATION_POSITIVE_THICKNESS_CONTINUOUS_CERTIFICATE_EXTENSION_MODEL_ID_V2:
    &str = "common_articulation_positive_thickness_continuous_certificate_extension_v2";
pub const ADMITTED_POSITIVE_THICKNESS_CONTINUOUS_CERTIFICATE_MODEL_ID_V2: &str =
    "admitted_positive_thickness_continuous_certificate_v2";

/// Exact input for a legacy-sized graph whose shared contacts are authenticated
/// by its own exact admission. This is used for the internally derived blocks
/// of a parent-admitted common-articulation composition.
pub struct AdmittedPositiveThicknessCycleSchedulePathInputV2<'a> {
    pub geometry: &'a MaterialHingeGraphGeometry,
    pub audit: &'a MaterialHingeGraphAudit,
    pub fixed_face: FaceId,
    pub schedule: &'a ori_kinematics::CanonicalCycleScheduleV1,
    pub closure: &'a DyadicMaterialHingeIntervalClosureCertificateV1,
    pub paper_thickness_mm: f64,
    pub interval_count: usize,
    pub graph_limits: CommonArticulationPositiveThicknessGraphExtensionLimitsV1,
    pub parent_graph_admission: Arc<CommonArticulationPositiveThicknessParentGraphAdmissionV2>,
}

/// Exact live tuple for revalidating one parent-admitted block certificate.
#[derive(Clone, Copy)]
pub struct AdmittedPositiveThicknessContinuousRevalidationInputV2<'a> {
    pub geometry: &'a MaterialHingeGraphGeometry,
    pub audit: &'a MaterialHingeGraphAudit,
    pub fixed_face: FaceId,
    pub schedule: &'a ori_kinematics::CanonicalCycleScheduleV1,
    pub closure: &'a DyadicMaterialHingeIntervalClosureCertificateV1,
    pub paper_thickness_mm: f64,
    pub graph_limits: CommonArticulationPositiveThicknessGraphExtensionLimitsV1,
    pub admission: &'a CommonArticulationPositiveThicknessParentGraphAdmissionV2,
}

/// Opaque admitted certificate for one internally derived material subgraph.
/// It has no public conversion to the legacy certificate type.
#[derive(Debug)]
pub struct AdmittedPositiveThicknessContinuousCertificateV2 {
    inner: PositiveThicknessContinuousCertificateV1,
    shared_contact: CommonArticulationAdmittedSharedFeatureContactCertificateV2,
    binding_fingerprint: [u8; 32],
}

pub(crate) struct AdmittedPositiveThicknessRetainedEvidenceV2 {
    shared_contact: CommonArticulationAdmittedSharedFeatureContactCertificateV2,
    binding_fingerprint: [u8; 32],
}

impl AdmittedPositiveThicknessContinuousCertificateV2 {
    #[must_use]
    pub const fn model_id_v2(&self) -> &'static str {
        ADMITTED_POSITIVE_THICKNESS_CONTINUOUS_CERTIFICATE_MODEL_ID_V2
    }

    #[must_use]
    pub const fn binding_fingerprint_v2(&self) -> [u8; 32] {
        self.binding_fingerprint
    }

    #[must_use]
    pub fn is_for_v2(
        &self,
        input: AdmittedPositiveThicknessContinuousRevalidationInputV2<'_>,
    ) -> bool {
        self.inner.is_for(
            input.geometry,
            input.audit,
            input.fixed_face,
            input.schedule,
            input.closure,
            input.paper_thickness_mm,
        ) && self.shared_contact.is_for_live_schedule_v2(
            AdmittedSharedFeatureContactRevalidationInputV2 {
                geometry: input.geometry,
                audit: input.audit,
                fixed_face: input.fixed_face,
                schedule: input.schedule,
                closure: input.closure,
                graph_limits: input.graph_limits,
                admission: input.admission,
            },
        )
    }

    pub(crate) const fn inner_v2(&self) -> &PositiveThicknessContinuousCertificateV1 {
        &self.inner
    }

    pub(crate) fn retained_parent_graph_admission_v2(
        &self,
    ) -> &CommonArticulationPositiveThicknessParentGraphAdmissionV2 {
        self.shared_contact.retained_parent_graph_admission_v2()
    }

    pub(crate) fn graph_limits_v2(
        &self,
    ) -> CommonArticulationPositiveThicknessGraphExtensionLimitsV1 {
        self.shared_contact.graph_limits_v2()
    }

    #[must_use]
    pub fn checked_deep_retained_bytes_v2(&self) -> Option<usize> {
        size_of::<Self>().checked_add(size_of::<
            CommonArticulationPositiveThicknessParentGraphAdmissionV2,
        >())
    }

    pub(crate) fn into_parts_for_complete_v2(
        self,
    ) -> (
        PositiveThicknessContinuousCertificateV1,
        AdmittedPositiveThicknessRetainedEvidenceV2,
    ) {
        (
            self.inner,
            AdmittedPositiveThicknessRetainedEvidenceV2 {
                shared_contact: self.shared_contact,
                binding_fingerprint: self.binding_fingerprint,
            },
        )
    }
}

impl AdmittedPositiveThicknessRetainedEvidenceV2 {
    pub(crate) fn binding_fingerprint_v2(&self) -> [u8; 32] {
        self.binding_fingerprint
    }

    pub(crate) fn parent_graph_admission_binding_fingerprint_v2(&self) -> [u8; 32] {
        self.shared_contact
            .parent_graph_admission_binding_fingerprint_v2()
    }

    pub(crate) fn graph_limits_v2(
        &self,
    ) -> CommonArticulationPositiveThicknessGraphExtensionLimitsV1 {
        self.shared_contact.graph_limits_v2()
    }

    pub(crate) fn retained_parent_graph_admission_v2(
        &self,
    ) -> &CommonArticulationPositiveThicknessParentGraphAdmissionV2 {
        self.shared_contact.retained_parent_graph_admission_v2()
    }

    pub(crate) fn is_for_v2(
        &self,
        geometry: &MaterialHingeGraphGeometry,
        audit: &MaterialHingeGraphAudit,
        fixed_face: FaceId,
        schedule: &ori_kinematics::CanonicalCycleScheduleV1,
        closure: &DyadicMaterialHingeIntervalClosureCertificateV1,
        graph_limits: CommonArticulationPositiveThicknessGraphExtensionLimitsV1,
    ) -> bool {
        self.shared_contact.is_for_live_schedule_v2(
            AdmittedSharedFeatureContactRevalidationInputV2 {
                geometry,
                audit,
                fixed_face,
                schedule,
                closure,
                graph_limits,
                admission: self.shared_contact.retained_parent_graph_admission_v2(),
            },
        )
    }

    pub(crate) fn checked_deep_retained_bytes_v2(&self) -> Option<usize> {
        size_of::<Self>().checked_add(size_of::<
            CommonArticulationPositiveThicknessParentGraphAdmissionV2,
        >())
    }
}

pub fn certify_admitted_positive_thickness_cycle_schedule_path_v2(
    input: AdmittedPositiveThicknessCycleSchedulePathInputV2<'_>,
) -> Option<AdmittedPositiveThicknessContinuousCertificateV2> {
    let shared_contact =
        crate::certify_common_articulation_admitted_flat_shared_feature_contacts_v2(
            input.geometry,
            input.audit,
            input.fixed_face,
            input.schedule,
            input.closure,
            input.graph_limits,
            input.parent_graph_admission,
        )
        .ok()?;
    let inner = certify_canonical_positive_thickness_cycle_schedule_path_in_scope_with_control_v1(
        input.geometry,
        input.audit,
        input.fixed_face,
        input.schedule,
        input.closure,
        input.paper_thickness_mm,
        input.interval_count,
        &CooperativeOperationControlV1::unbounded(),
        PositiveThicknessGraphProofScopeV1::CommonArticulationAdmittedExtensionV2 {
            limits: input.graph_limits,
            shared_contact: &shared_contact,
        },
    )
    .ok()
    .flatten()?;
    let mut hash = Sha256::new();
    hash.update(ADMITTED_POSITIVE_THICKNESS_CONTINUOUS_CERTIFICATE_MODEL_ID_V2.as_bytes());
    hash.update(inner.fixed_face.canonical_bytes());
    hash.update(inner.schedule_hash);
    hash.update(inner.closure_hash);
    hash.update(inner.thickness_bits.to_le_bytes());
    hash.update((inner.proof_leaf_count as u64).to_le_bytes());
    hash.update((inner.pair_work as u64).to_le_bytes());
    hash.update(shared_contact.parent_graph_admission_binding_fingerprint_v2());
    let binding_fingerprint = hash.finalize().into();
    Some(AdmittedPositiveThicknessContinuousCertificateV2 {
        inner,
        shared_contact,
        binding_fingerprint,
    })
}

/// Exact issuance tuple.  The admission is moved into the result through an
/// `Arc`; the underlying opaque proof remains non-cloneable and non-serializable.
pub struct CommonArticulationPositiveThicknessCycleSchedulePathExtensionInputV2<'a> {
    pub geometry: &'a MaterialHingeGraphGeometry,
    pub audit: &'a MaterialHingeGraphAudit,
    pub decomposition: &'a CanonicalMaterialEdgeBlockDecompositionV1,
    pub configured_max_blocks: usize,
    pub fixed_face: FaceId,
    pub schedule: &'a ori_kinematics::CanonicalCycleScheduleV1,
    pub closure: &'a DyadicMaterialHingeIntervalClosureCertificateV1,
    pub paper_thickness_mm: f64,
    pub interval_count: usize,
    pub graph_limits: CommonArticulationPositiveThicknessGraphExtensionLimitsV1,
    pub parent_graph_admission: Arc<CommonArticulationPositiveThicknessParentGraphAdmissionV2>,
}

/// Exact live tuple for retained V2 certificate revalidation.
#[derive(Clone, Copy)]
pub struct CommonArticulationPositiveThicknessContinuousRevalidationInputV2<'a> {
    pub geometry: &'a MaterialHingeGraphGeometry,
    pub audit: &'a MaterialHingeGraphAudit,
    pub decomposition: &'a CanonicalMaterialEdgeBlockDecompositionV1,
    pub configured_max_blocks: usize,
    pub fixed_face: FaceId,
    pub schedule: &'a ori_kinematics::CanonicalCycleScheduleV1,
    pub closure: &'a DyadicMaterialHingeIntervalClosureCertificateV1,
    pub paper_thickness_mm: f64,
    pub graph_limits: CommonArticulationPositiveThicknessGraphExtensionLimitsV1,
    pub parent_graph_admission: &'a CommonArticulationPositiveThicknessParentGraphAdmissionV2,
}

/// Opaque, process-local positive-thickness certificate under exact parent
/// topology admission.  It intentionally grants no consumer authority.
///
/// ```compile_fail
/// use ori_collision::{
///     CommonArticulationPositiveThicknessContinuousCertificateExtensionV2,
///     PositiveThicknessContinuousCertificateV1,
/// };
/// fn legacy(_: PositiveThicknessContinuousCertificateV1) {}
/// fn cannot_downgrade(value: CommonArticulationPositiveThicknessContinuousCertificateExtensionV2) {
///     legacy(value);
/// }
/// ```
///
/// ```compile_fail
/// use ori_collision::CommonArticulationPositiveThicknessContinuousCertificateExtensionV2;
/// fn require_serialize<T: serde::Serialize>() {}
/// require_serialize::<CommonArticulationPositiveThicknessContinuousCertificateExtensionV2>();
/// ```
#[derive(Clone)]
pub struct CommonArticulationPositiveThicknessContinuousCertificateExtensionV2 {
    inner: PositiveThicknessContinuousCertificateV1,
    configured_max_blocks: usize,
    actual_block_count: usize,
    decomposition_binding: [u8; 32],
    graph_limits: CommonArticulationPositiveThicknessGraphExtensionLimitsV1,
    parent_graph_admission: Arc<CommonArticulationPositiveThicknessParentGraphAdmissionV2>,
    shared_contact: CommonArticulationAdmittedSharedFeatureContactCertificateV2,
    binding_fingerprint: [u8; 32],
}

impl std::fmt::Debug for CommonArticulationPositiveThicknessContinuousCertificateExtensionV2 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CommonArticulationPositiveThicknessContinuousCertificateExtensionV2")
            .field("model_id", &self.model_id_v2())
            .field("configured_max_blocks", &self.configured_max_blocks)
            .field("actual_block_count", &self.actual_block_count)
            .field(
                "parent_graph_admission_binding",
                &self.parent_graph_admission.binding_fingerprint_v2(),
            )
            .field("binding_fingerprint", &self.binding_fingerprint)
            .finish_non_exhaustive()
    }
}

impl CommonArticulationPositiveThicknessContinuousCertificateExtensionV2 {
    #[must_use]
    pub const fn model_id_v2(&self) -> &'static str {
        COMMON_ARTICULATION_POSITIVE_THICKNESS_CONTINUOUS_CERTIFICATE_EXTENSION_MODEL_ID_V2
    }

    #[must_use]
    pub const fn configured_max_blocks_v2(&self) -> usize {
        self.configured_max_blocks
    }

    #[must_use]
    pub const fn actual_block_count_v2(&self) -> usize {
        self.actual_block_count
    }

    #[must_use]
    pub const fn binding_fingerprint_v2(&self) -> [u8; 32] {
        self.binding_fingerprint
    }

    #[must_use]
    pub fn parent_graph_admission_binding_fingerprint_v2(&self) -> [u8; 32] {
        self.parent_graph_admission.binding_fingerprint_v2()
    }

    #[must_use]
    pub fn parent_graph_semantic_digest_v2(&self) -> [u8; 32] {
        self.parent_graph_admission.semantic_graph_digest_v2()
    }

    #[must_use]
    pub fn parent_graph_admission_limits_v2(
        &self,
    ) -> CommonArticulationPositiveThicknessParentGraphAdmissionLimitsV2 {
        self.parent_graph_admission.limits_v2()
    }

    #[must_use]
    pub fn parent_graph_admission_resources_v2(
        &self,
    ) -> CommonArticulationPositiveThicknessParentGraphAdmissionResourcesV2 {
        self.parent_graph_admission.resources_v2()
    }

    #[must_use]
    pub const fn checked_deep_retained_bytes_v2(&self) -> Option<usize> {
        size_of::<Self>().checked_add(size_of::<
            CommonArticulationPositiveThicknessParentGraphAdmissionV2,
        >())
    }

    #[must_use]
    pub fn is_for_v2(
        &self,
        input: CommonArticulationPositiveThicknessContinuousRevalidationInputV2<'_>,
    ) -> bool {
        if self.graph_limits != input.graph_limits
            || !self
                .parent_graph_admission
                .same_evidence_v2(input.parent_graph_admission)
            || !self
                .parent_graph_admission
                .matches_geometry_instance_v2(input.geometry)
            || !input
                .parent_graph_admission
                .matches_geometry_instance_v2(input.geometry)
            || !self.shared_contact.is_for_live_schedule_v2(
                AdmittedSharedFeatureContactRevalidationInputV2 {
                    geometry: input.geometry,
                    audit: input.audit,
                    fixed_face: input.fixed_face,
                    schedule: input.schedule,
                    closure: input.closure,
                    graph_limits: input.graph_limits,
                    admission: input.parent_graph_admission,
                },
            )
        {
            return false;
        }
        let Some(scope) = common_articulation_positive_thickness_graph_scope_v1(
            input.geometry,
            input.decomposition,
            input.configured_max_blocks,
            input.graph_limits,
        ) else {
            return false;
        };
        self.configured_max_blocks == scope.configured_max_blocks
            && self.actual_block_count == scope.actual_block_count
            && self.decomposition_binding == scope.decomposition_binding
            && self.inner.is_for(
                input.geometry,
                input.audit,
                input.fixed_face,
                input.schedule,
                input.closure,
                input.paper_thickness_mm,
            )
            && self.binding_fingerprint
                == common_articulation_positive_thickness_continuous_extension_binding_v2(
                    &self.inner,
                    scope,
                    self.parent_graph_admission.as_ref(),
                )
    }

    pub(crate) fn retained_parent_graph_admission_v2(
        &self,
    ) -> &CommonArticulationPositiveThicknessParentGraphAdmissionV2 {
        self.parent_graph_admission.as_ref()
    }

    pub(crate) fn issue_general_transport_extension_v2(
        &self,
        input: crate::general_cell_transport::CommonArticulationGeneralCellTransportExtensionInputV1<
            '_,
        >,
    ) -> Result<
        crate::general_cell_transport::CommonArticulationGeneralMultiFaceCellTransportProofExtensionV1,
        crate::general_cell_transport::GeneralCellTransportErrorV1,
    >{
        crate::general_cell_transport::certify_common_articulation_general_transport_with_scoped_positive_material_v2(
            input,
            CommonArticulationPositiveThicknessScopedMaterialV2 {
                scope: self,
                inner: &self.inner,
            },
        )
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

pub(crate) struct CommonArticulationPositiveThicknessScopedMaterialV2<'a> {
    scope: &'a CommonArticulationPositiveThicknessContinuousCertificateExtensionV2,
    inner: &'a PositiveThicknessContinuousCertificateV1,
}

impl<'a> CommonArticulationPositiveThicknessScopedMaterialV2<'a> {
    pub(crate) const fn scope(
        &self,
    ) -> &'a CommonArticulationPositiveThicknessContinuousCertificateExtensionV2 {
        self.scope
    }

    pub(crate) const fn inner(&self) -> &'a PositiveThicknessContinuousCertificateV1 {
        self.inner
    }
}

/// Issues a V2 certificate from an already-sealed exact admission.  Because
/// both the admission and geometry are immutable and process-local, issuance
/// checks their instance anchor; outer final issuance performs the expensive
/// complete exact replay once for the composed operation.
pub fn certify_common_articulation_positive_thickness_cycle_schedule_path_extension_v2(
    input: CommonArticulationPositiveThicknessCycleSchedulePathExtensionInputV2<'_>,
) -> Option<CommonArticulationPositiveThicknessContinuousCertificateExtensionV2> {
    certify_common_articulation_positive_thickness_cycle_schedule_path_extension_with_control_v2(
        input,
        &CooperativeOperationControlV1::unbounded(),
    )
    .ok()
    .flatten()
}

pub fn certify_common_articulation_positive_thickness_cycle_schedule_path_extension_with_control_v2(
    input: CommonArticulationPositiveThicknessCycleSchedulePathExtensionInputV2<'_>,
    control: &CooperativeOperationControlV1<'_>,
) -> Result<
    Option<CommonArticulationPositiveThicknessContinuousCertificateExtensionV2>,
    CanonicalPositiveThicknessCyclePathControlErrorV1,
> {
    canonical_positive_cycle_checkpoint_v1(control)?;
    if !input
        .parent_graph_admission
        .matches_geometry_instance_v2(input.geometry)
    {
        return Ok(None);
    }
    let Some(scope) = common_articulation_positive_thickness_graph_scope_v1(
        input.geometry,
        input.decomposition,
        input.configured_max_blocks,
        input.graph_limits,
    ) else {
        return Ok(None);
    };
    let shared_contact = match crate::certify_common_articulation_admitted_flat_shared_feature_contacts_with_checkpoint_v2(
        crate::CommonArticulationAdmittedSharedFeatureContactInputV2 {
            geometry: input.geometry,
            audit: input.audit,
            fixed_face: input.fixed_face,
            schedule: input.schedule,
            closure: input.closure,
            graph_limits: input.graph_limits,
            admission: Arc::clone(&input.parent_graph_admission),
        },
        || {
            control.checkpoint().map_err(|stop| match stop {
                crate::CooperativeOperationStopV1::Cancelled => {
                    crate::CommonArticulationPositiveThicknessParentGraphAdmissionStopV2::Cancelled
                }
                crate::CooperativeOperationStopV1::DeadlineExceeded => {
                    crate::CommonArticulationPositiveThicknessParentGraphAdmissionStopV2::DeadlineExceeded
                }
            })
        },
    ) {
        Ok(evidence) => evidence,
        Err(crate::CommonArticulationAdmittedPositiveThicknessGraphProofErrorV2::ParentAdmission(
            crate::CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::Cancelled,
        )) => return Err(CanonicalPositiveThicknessCyclePathControlErrorV1::Cancelled),
        Err(crate::CommonArticulationAdmittedPositiveThicknessGraphProofErrorV2::ParentAdmission(
            crate::CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::DeadlineExceeded,
        )) => return Err(CanonicalPositiveThicknessCyclePathControlErrorV1::DeadlineExceeded),
        Err(_) => return Ok(None),
    };
    let inner = certify_canonical_positive_thickness_cycle_schedule_path_in_scope_with_control_v1(
        input.geometry,
        input.audit,
        input.fixed_face,
        input.schedule,
        input.closure,
        input.paper_thickness_mm,
        input.interval_count,
        control,
        PositiveThicknessGraphProofScopeV1::CommonArticulationAdmittedExtensionV2 {
            limits: input.graph_limits,
            shared_contact: &shared_contact,
        },
    )?;
    canonical_positive_cycle_checkpoint_v1(control)?;
    Ok(inner.map(|inner| {
        let binding_fingerprint =
            common_articulation_positive_thickness_continuous_extension_binding_v2(
                &inner,
                scope,
                input.parent_graph_admission.as_ref(),
            );
        CommonArticulationPositiveThicknessContinuousCertificateExtensionV2 {
            inner,
            configured_max_blocks: scope.configured_max_blocks,
            actual_block_count: scope.actual_block_count,
            decomposition_binding: scope.decomposition_binding,
            graph_limits: scope.limits,
            parent_graph_admission: input.parent_graph_admission,
            shared_contact,
            binding_fingerprint,
        }
    }))
}

fn common_articulation_positive_thickness_continuous_extension_binding_v2(
    inner: &PositiveThicknessContinuousCertificateV1,
    scope: CommonArticulationPositiveThicknessScopeV1,
    admission: &CommonArticulationPositiveThicknessParentGraphAdmissionV2,
) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(
        COMMON_ARTICULATION_POSITIVE_THICKNESS_CONTINUOUS_CERTIFICATE_EXTENSION_MODEL_ID_V2
            .as_bytes(),
    );
    for value in [
        COMMON_ARTICULATION_POSE_EXTENSION_MIN_BLOCKS_V1,
        scope.configured_max_blocks,
        scope.actual_block_count,
        scope.limits.max_unordered_face_pairs,
        scope.limits.max_shared_feature_pairs,
        inner.proof_leaf_count,
        inner.pair_work,
    ] {
        hash.update((value as u64).to_le_bytes());
    }
    hash.update(scope.decomposition_binding);
    hash.update(inner.fixed_face.canonical_bytes());
    hash.update(inner.schedule_hash);
    hash.update(inner.closure_hash);
    hash.update(inner.thickness_bits.to_le_bytes());
    update_parent_graph_admission_material_v2(&mut hash, admission);
    hash.finalize().into()
}

fn update_parent_graph_admission_material_v2(
    hash: &mut Sha256,
    admission: &CommonArticulationPositiveThicknessParentGraphAdmissionV2,
) {
    hash.update(admission.model_id_v2().as_bytes());
    hash.update(admission.identity_namespace_v2().canonical_bytes());
    hash.update(admission.source_revision_v2().to_le_bytes());
    hash.update(admission.fold_model_fingerprint_v2());
    hash.update(admission.semantic_graph_digest_v2());
    hash.update(admission.binding_fingerprint_v2());
    let limits = admission.limits_v2();
    for value in [
        limits.max_faces,
        limits.max_hinges,
        limits.max_boundary_vertex_occurrences,
        limits.max_vertices,
        limits.max_edges,
        limits.max_vertex_pairs,
        limits.max_vertex_edge_tests,
        limits.max_edge_pair_tests,
        limits.max_face_pair_tests,
        limits.max_point_in_polygon_edge_tests,
        limits.max_exact_operations,
        limits.max_logical_work,
        limits.max_workspace_bytes,
    ] {
        hash.update((value as u64).to_le_bytes());
    }
    let resources = admission.resources_v2();
    for value in [
        resources.face_count_v2(),
        resources.hinge_count_v2(),
        resources.boundary_vertex_occurrences_v2(),
        resources.vertex_count_v2(),
        resources.edge_count_v2(),
        resources.vertex_pair_tests_v2(),
        resources.vertex_edge_tests_v2(),
        resources.edge_pair_tests_v2(),
        resources.face_pair_tests_v2(),
        resources.point_in_polygon_edge_tests_v2(),
        resources.exact_operations_v2(),
        resources.logical_work_v2(),
        resources.workspace_bytes_upper_bound_v2(),
    ] {
        hash.update((value as u64).to_le_bytes());
    }
}
