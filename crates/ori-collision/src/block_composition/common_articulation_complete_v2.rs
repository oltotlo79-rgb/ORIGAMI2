//! Complete live-union composition for parent-admitted block certificates.
//!
//! This module is the only bridge from an admitted per-block positive path and
//! its admitted transport proof into the 11..=32 final extension.  The legacy
//! positive certificate remains private proof material throughout: there is
//! deliberately no conversion to `MultiBlockPositiveLayerAuthorityV1` or
//! `CompleteMultiBlockPositiveLayerAuthorityV1`.

use std::{collections::HashSet, mem::size_of};

use ori_domain::{EdgeId, FaceId};
use ori_foldability::LayerOrderSnapshot;
use ori_kinematics::{
    CanonicalCycleScheduleV1, CanonicalMaterialEdgeBlockDecompositionV1,
    CycleScheduleRestrictionErrorV1, CycleScheduleRestrictionStopV1,
    DyadicMaterialHingeIntervalClosureCertificateV1, MaterialHingeGraphAudit,
    MaterialHingeGraphGeometry, MaterialHingeGraphInstanceV1,
};
use sha2::{Digest, Sha256};
use thiserror::Error;

use super::{
    BLOCK_UNION_COMPLETENESS_MAX_ITEMS_V1, BOUNDED_MULTI_BLOCK_EXTENSION_MAX_BLOCKS_V1,
    BOUNDED_MULTI_BLOCK_EXTENSION_MIN_BLOCKS_V1, BlockUnionCompletenessGapReportV1,
    CanonicalBlockBindingV1, MultiBlockAdmissionScopeV1, MultiBlockClosureAuthorityV1,
    block_articulation_incidence_is_tree_v1, canonical_decomposition_block_bindings_v1,
    complete_block_union_matches_live_v1,
};
use crate::continuous_path::common_articulation_positive_thickness_v2::AdmittedPositiveThicknessRetainedEvidenceV2;
use crate::general_cell_transport::AdmittedGeneralCellTransportRetainedEvidenceV2;
use crate::{
    AdmittedGeneralCellTransportInputV2, AdmittedGeneralMultiFaceCellTransportProofV2,
    AdmittedPositiveThicknessContinuousCertificateV2,
    AdmittedPositiveThicknessContinuousRevalidationInputV2,
    CommonArticulationPositiveThicknessContinuousCertificateExtensionV2,
    CommonArticulationPositiveThicknessContinuousRevalidationInputV2,
    CommonArticulationPositiveThicknessGraphExtensionLimitsV1,
    CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2,
    CommonArticulationPositiveThicknessParentGraphAdmissionStopV2,
    CommonArticulationPositiveThicknessParentGraphAdmissionV2, CooperativeOperationControlV1,
    CooperativeOperationStopV1, GeneralMultiFaceCellTransportProofV1,
    PositiveThicknessContinuousCertificateV1,
    revalidate_common_articulation_positive_thickness_parent_graph_admission_with_checkpoint_v2,
};

pub const ADMITTED_MULTI_BLOCK_POSITIVE_LAYER_MODEL_ID_V2: &str =
    "admitted_complete_live_multi_block_positive_layer_authority_v2";

/// Every complete-V2 resource limit has a fixed library ceiling.  In
/// particular, callers cannot turn the quadratic face-pair revalidation into
/// an unbounded scan by submitting a larger value.
pub const COMPLETE_MULTI_BLOCK_POSITIVE_LAYER_MAX_FACE_PAIR_TESTS_V2: usize =
    (BOUNDED_MULTI_BLOCK_EXTENSION_MAX_BLOCKS_V1 + 1) * 32_896;
pub const COMPLETE_MULTI_BLOCK_POSITIVE_LAYER_MAX_LOGICAL_WORK_V2: usize = 4_224_000_000;
pub const COMPLETE_MULTI_BLOCK_POSITIVE_LAYER_MAX_RETAINED_BYTES_V2: usize =
    ori_foldability::DEFAULT_MAX_CERTIFICATE_BYTES;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompleteMultiBlockPositiveLayerLimitsV2 {
    pub max_blocks: usize,
    pub max_faces: usize,
    pub max_hinges: usize,
    pub max_face_pair_tests: usize,
    pub max_logical_work: usize,
    pub max_deep_retained_bytes: usize,
}

impl Default for CompleteMultiBlockPositiveLayerLimitsV2 {
    fn default() -> Self {
        Self {
            max_blocks: BOUNDED_MULTI_BLOCK_EXTENSION_MAX_BLOCKS_V1,
            max_faces: crate::COMMON_ARTICULATION_POSITIVE_THICKNESS_GRAPH_EXTENSION_MAX_FACES_V1,
            max_hinges: BLOCK_UNION_COMPLETENESS_MAX_ITEMS_V1,
            max_face_pair_tests: COMPLETE_MULTI_BLOCK_POSITIVE_LAYER_MAX_FACE_PAIR_TESTS_V2,
            max_logical_work: COMPLETE_MULTI_BLOCK_POSITIVE_LAYER_MAX_LOGICAL_WORK_V2,
            max_deep_retained_bytes: COMPLETE_MULTI_BLOCK_POSITIVE_LAYER_MAX_RETAINED_BYTES_V2,
        }
    }
}

impl CompleteMultiBlockPositiveLayerLimitsV2 {
    fn is_valid_v2(self) -> bool {
        (BOUNDED_MULTI_BLOCK_EXTENSION_MIN_BLOCKS_V1..=BOUNDED_MULTI_BLOCK_EXTENSION_MAX_BLOCKS_V1)
            .contains(&self.max_blocks)
            && (1..=crate::COMMON_ARTICULATION_POSITIVE_THICKNESS_GRAPH_EXTENSION_MAX_FACES_V1)
                .contains(&self.max_faces)
            && (1..=BLOCK_UNION_COMPLETENESS_MAX_ITEMS_V1).contains(&self.max_hinges)
            && (1..=COMPLETE_MULTI_BLOCK_POSITIVE_LAYER_MAX_FACE_PAIR_TESTS_V2)
                .contains(&self.max_face_pair_tests)
            && (1..=COMPLETE_MULTI_BLOCK_POSITIVE_LAYER_MAX_LOGICAL_WORK_V2)
                .contains(&self.max_logical_work)
            && (1..=COMPLETE_MULTI_BLOCK_POSITIVE_LAYER_MAX_RETAINED_BYTES_V2)
                .contains(&self.max_deep_retained_bytes)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CompleteMultiBlockPositiveLayerResourcesV2 {
    block_count: usize,
    face_count: usize,
    hinge_count: usize,
    block_face_occurrences: usize,
    block_hinge_occurrences: usize,
    face_pair_tests: usize,
    transition_count: usize,
    pair_order_count: usize,
    logical_work: usize,
    deep_retained_bytes: usize,
    retained_parent_count: usize,
    retained_parent_alias_count: usize,
}

impl CompleteMultiBlockPositiveLayerResourcesV2 {
    #[must_use]
    pub const fn block_count_v2(self) -> usize {
        self.block_count
    }
    #[must_use]
    pub const fn face_count_v2(self) -> usize {
        self.face_count
    }
    #[must_use]
    pub const fn hinge_count_v2(self) -> usize {
        self.hinge_count
    }
    #[must_use]
    pub const fn block_face_occurrences_v2(self) -> usize {
        self.block_face_occurrences
    }
    #[must_use]
    pub const fn block_hinge_occurrences_v2(self) -> usize {
        self.block_hinge_occurrences
    }
    #[must_use]
    pub const fn face_pair_tests_v2(self) -> usize {
        self.face_pair_tests
    }
    #[must_use]
    pub const fn transition_count_v2(self) -> usize {
        self.transition_count
    }
    #[must_use]
    pub const fn pair_order_count_v2(self) -> usize {
        self.pair_order_count
    }
    #[must_use]
    pub const fn logical_work_v2(self) -> usize {
        self.logical_work
    }
    #[must_use]
    pub const fn deep_retained_bytes_v2(self) -> usize {
        self.deep_retained_bytes
    }
    #[must_use]
    pub const fn retained_parent_count_v2(self) -> usize {
        self.retained_parent_count
    }
    #[must_use]
    pub const fn retained_parent_alias_count_v2(self) -> usize {
        self.retained_parent_alias_count
    }
}

/// One block whose positive path and cell transport were both issued through
/// admitted V2 entry points.  Both proofs are moved into a successful result.
pub struct AdmittedMultiBlockPositiveLayerInputV2<'a> {
    pub geometry: &'a MaterialHingeGraphGeometry,
    pub source: &'a LayerOrderSnapshot,
    pub positive: AdmittedPositiveThicknessContinuousCertificateV2,
    pub layer: AdmittedGeneralMultiFaceCellTransportProofV2,
}

/// Complete issuance tuple.  The legacy closure parent is accepted only as a
/// sealed owner of canonical block schedules/closures; no legacy positive or
/// complete authority is accepted here.
pub struct CompleteMultiBlockPositiveLayerInputV2<'a> {
    pub geometry: &'a MaterialHingeGraphGeometry,
    pub audit: &'a MaterialHingeGraphAudit,
    pub decomposition: &'a CanonicalMaterialEdgeBlockDecompositionV1,
    pub configured_max_blocks: usize,
    pub report: BlockUnionCompletenessGapReportV1,
    pub closure_parent: MultiBlockClosureAuthorityV1,
    pub blocks: Vec<AdmittedMultiBlockPositiveLayerInputV2<'a>>,
    pub source: &'a LayerOrderSnapshot,
    pub block_sources: &'a [&'a LayerOrderSnapshot],
    pub paper_thickness_mm: f64,
    pub issuer_context: [u8; 32],
    pub articulation_layer_fingerprint: [u8; 32],
    pub target_angles: &'a [(EdgeId, f64)],
    pub whole_parent_schedule: &'a CanonicalCycleScheduleV1,
    pub whole_parent_closure: &'a DyadicMaterialHingeIntervalClosureCertificateV1,
    pub whole_parent_positive: CommonArticulationPositiveThicknessContinuousCertificateExtensionV2,
    pub positive_graph_limits: CommonArticulationPositiveThicknessGraphExtensionLimitsV1,
    pub parent_graph_admission: &'a CommonArticulationPositiveThicknessParentGraphAdmissionV2,
    pub limits: CompleteMultiBlockPositiveLayerLimitsV2,
}

#[derive(Clone, Copy)]
pub struct CompleteMultiBlockPositiveLayerRevalidationInputV2<'a> {
    pub geometry: &'a MaterialHingeGraphGeometry,
    pub audit: &'a MaterialHingeGraphAudit,
    pub decomposition: &'a CanonicalMaterialEdgeBlockDecompositionV1,
    pub configured_max_blocks: usize,
    pub source: &'a LayerOrderSnapshot,
    pub block_sources: &'a [&'a LayerOrderSnapshot],
    pub paper_thickness_mm: f64,
    pub issuer_context: [u8; 32],
    pub articulation_layer_fingerprint: [u8; 32],
    pub target_angles: &'a [(EdgeId, f64)],
    pub whole_parent_schedule: &'a CanonicalCycleScheduleV1,
    pub whole_parent_closure: &'a DyadicMaterialHingeIntervalClosureCertificateV1,
    pub positive_graph_limits: CommonArticulationPositiveThicknessGraphExtensionLimitsV1,
    pub parent_graph_admission: &'a CommonArticulationPositiveThicknessParentGraphAdmissionV2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum CompleteMultiBlockPositiveLayerErrorV2 {
    #[error("the complete admitted multi-block input is malformed")]
    InvalidInput,
    #[error("the complete admitted multi-block operation exceeded a fixed resource limit")]
    ResourceLimit,
    #[error("the closure parent or canonical live union does not match")]
    CanonicalBlockPartitionMismatch,
    #[error("an admitted block positive certificate does not replay")]
    PositiveBindingMismatch,
    #[error("an admitted block transport proof does not replay")]
    TransportBindingMismatch,
    #[error("the whole-parent admitted positive certificate does not replay")]
    WholeParentPositiveBindingMismatch,
    #[error("a block schedule is not the exact whole-parent restriction")]
    BlockScheduleRestrictionMismatch,
    #[error("a block layer source is not the exact whole-parent restriction")]
    BlockSourceRestrictionMismatch,
    #[error("the target angle set does not equal the block schedule endpoints")]
    TargetAngleMismatch,
    #[error("the exact parent-graph admission failed: {0}")]
    ParentGraphAdmission(CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2),
    #[error("the retained complete V2 binding does not match the live tuple")]
    BindingMismatch,
    #[error("the complete admitted multi-block operation was cancelled")]
    Cancelled,
    #[error("the complete admitted multi-block operation deadline elapsed")]
    DeadlineExceeded,
}

struct RetainedAdmittedBlockV2 {
    schedule: CanonicalCycleScheduleV1,
    closure: DyadicMaterialHingeIntervalClosureCertificateV1,
    positive: PositiveThicknessContinuousCertificateV1,
    positive_evidence: AdmittedPositiveThicknessRetainedEvidenceV2,
    layer: GeneralMultiFaceCellTransportProofV1,
    layer_evidence: AdmittedGeneralCellTransportRetainedEvidenceV2,
}

/// Opaque, non-authorizing complete live-union evidence for admitted V2 block
/// proofs.  It is non-cloneable and non-serializable, and exposes no legacy
/// proof conversion.
pub struct CompleteMultiBlockPositiveLayerAuthorityV2 {
    issuer: MaterialHingeGraphInstanceV1,
    binding: [u8; 32],
    configured_max_blocks: usize,
    actual_block_count: usize,
    blocks: Vec<CanonicalBlockBindingV1>,
    retained: Vec<RetainedAdmittedBlockV2>,
    whole_parent_positive: CommonArticulationPositiveThicknessContinuousCertificateExtensionV2,
    closure_parent_binding: [u8; 32],
    limits: CompleteMultiBlockPositiveLayerLimitsV2,
    resources: CompleteMultiBlockPositiveLayerResourcesV2,
}

impl std::fmt::Debug for CompleteMultiBlockPositiveLayerAuthorityV2 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompleteMultiBlockPositiveLayerAuthorityV2")
            .field("model_id", &self.model_id_v2())
            .field("binding", &self.binding)
            .field("configured_max_blocks", &self.configured_max_blocks)
            .field("actual_block_count", &self.actual_block_count)
            .field("resources", &self.resources)
            .finish_non_exhaustive()
    }
}

impl CompleteMultiBlockPositiveLayerAuthorityV2 {
    #[must_use]
    pub const fn model_id_v2(&self) -> &'static str {
        ADMITTED_MULTI_BLOCK_POSITIVE_LAYER_MODEL_ID_V2
    }
    #[must_use]
    pub const fn binding_fingerprint_v2(&self) -> [u8; 32] {
        self.binding
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
    pub fn block_count_v2(&self) -> usize {
        self.blocks.len()
    }
    #[must_use]
    pub const fn limits_v2(&self) -> CompleteMultiBlockPositiveLayerLimitsV2 {
        self.limits
    }
    #[must_use]
    pub const fn resources_v2(&self) -> CompleteMultiBlockPositiveLayerResourcesV2 {
        self.resources
    }
    #[must_use]
    pub const fn checked_deep_retained_bytes_v2(&self) -> usize {
        self.resources.deep_retained_bytes
    }
    /// Tight resource envelope for this retained certificate. The configured
    /// block cap remains part of the envelope even when the live graph uses
    /// fewer blocks.
    #[must_use]
    pub const fn exact_resource_limits_v2(&self) -> CompleteMultiBlockPositiveLayerLimitsV2 {
        CompleteMultiBlockPositiveLayerLimitsV2 {
            max_blocks: self.configured_max_blocks,
            max_faces: self.resources.face_count,
            max_hinges: self.resources.hinge_count,
            max_face_pair_tests: self.resources.face_pair_tests,
            max_logical_work: self.resources.logical_work,
            max_deep_retained_bytes: self.resources.deep_retained_bytes,
        }
    }
    /// Checks the retained certificate against a caller-selected envelope
    /// without rerunning geometric proof work.
    pub fn revalidate_resource_limits_v2(
        &self,
        limits: CompleteMultiBlockPositiveLayerLimitsV2,
    ) -> Result<(), CompleteMultiBlockPositiveLayerErrorV2> {
        if !limits.is_valid_v2() {
            return Err(CompleteMultiBlockPositiveLayerErrorV2::InvalidInput);
        }
        if self.configured_max_blocks > limits.max_blocks {
            return Err(CompleteMultiBlockPositiveLayerErrorV2::ResourceLimit);
        }
        enforce_resources_v2(self.resources, limits)
    }
    #[must_use]
    pub fn whole_parent_positive_binding_fingerprint_v2(&self) -> [u8; 32] {
        self.whole_parent_positive.binding_fingerprint_v2()
    }
    #[must_use]
    pub fn parent_graph_admission_binding_fingerprint_v2(&self) -> [u8; 32] {
        self.whole_parent_positive
            .parent_graph_admission_binding_fingerprint_v2()
    }
    pub fn revalidate_v2(
        &self,
        input: CompleteMultiBlockPositiveLayerRevalidationInputV2<'_>,
    ) -> Result<(), CompleteMultiBlockPositiveLayerErrorV2> {
        self.revalidate_with_control_v2(input, &CooperativeOperationControlV1::unbounded())
    }
    pub fn revalidate_with_control_v2(
        &self,
        input: CompleteMultiBlockPositiveLayerRevalidationInputV2<'_>,
        control: &CooperativeOperationControlV1<'_>,
    ) -> Result<(), CompleteMultiBlockPositiveLayerErrorV2> {
        self.revalidate_with_checkpoint_v2(input, control, &mut || complete_checkpoint_v2(control))
    }
    pub(super) fn revalidate_with_checkpoint_v2(
        &self,
        input: CompleteMultiBlockPositiveLayerRevalidationInputV2<'_>,
        control: &CooperativeOperationControlV1<'_>,
        checkpoint: &mut impl FnMut() -> Result<(), CompleteMultiBlockPositiveLayerErrorV2>,
    ) -> Result<(), CompleteMultiBlockPositiveLayerErrorV2> {
        let validated = validate_retained_v2(self, input, control, checkpoint)?;
        checkpoint()?;
        if validated != self.resources
            || complete_binding_v2(self, input, checkpoint)? != self.binding
        {
            return Err(CompleteMultiBlockPositiveLayerErrorV2::BindingMismatch);
        }
        checkpoint()
    }
    pub(super) fn canonical_blocks_v2(&self) -> &[CanonicalBlockBindingV1] {
        &self.blocks
    }
    #[cfg(test)]
    pub(super) fn corrupt_configured_max_for_test_v2(&mut self) {
        self.configured_max_blocks = self.configured_max_blocks.saturating_add(1);
    }
    #[cfg(test)]
    pub(super) fn corrupt_partition_for_test_v2(&mut self) {
        if self.retained.len() > 1 {
            self.retained.swap(0, 1);
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

pub fn issue_complete_multi_block_positive_layer_authority_v2(
    input: CompleteMultiBlockPositiveLayerInputV2<'_>,
) -> Result<CompleteMultiBlockPositiveLayerAuthorityV2, CompleteMultiBlockPositiveLayerErrorV2> {
    issue_complete_multi_block_positive_layer_authority_with_control_v2(
        input,
        &CooperativeOperationControlV1::unbounded(),
    )
}

pub fn issue_complete_multi_block_positive_layer_authority_with_control_v2(
    mut input: CompleteMultiBlockPositiveLayerInputV2<'_>,
    control: &CooperativeOperationControlV1<'_>,
) -> Result<CompleteMultiBlockPositiveLayerAuthorityV2, CompleteMultiBlockPositiveLayerErrorV2> {
    let mut checkpoint = || complete_checkpoint_v2(control);
    checkpoint()?;
    validate_common_input_v2(CompleteCommonValidationInputV2 {
        geometry: input.geometry,
        decomposition: input.decomposition,
        configured_max_blocks: input.configured_max_blocks,
        thickness: input.paper_thickness_mm,
        issuer_context: input.issuer_context,
        layer_fingerprint: input.articulation_layer_fingerprint,
        sources: input.block_sources,
        limits: input.limits,
    })?;
    let expected_scope = MultiBlockAdmissionScopeV1::BoundedExtensionSubmittedSet {
        configured_max_blocks: input.configured_max_blocks,
    };
    let canonical = canonical_decomposition_block_bindings_v1(input.decomposition, control)
        .map_err(map_canonical_error_v2)?;
    if input.report.scope != expected_scope
        || input.closure_parent.scope != expected_scope
        || input.report.scope != input.closure_parent.scope
        || !input.report.is_for(input.geometry)
        || !input.report.complete
        || input.report.blocks != canonical
        || !complete_block_union_matches_live_v1(
            expected_scope,
            &canonical,
            &input.report.live_faces,
            &input.report.live_hinges,
        )
        || input.closure_parent.blocks.len() != canonical.len()
        || input.blocks.len() != canonical.len()
        || input.block_sources.len() != canonical.len()
    {
        return Err(CompleteMultiBlockPositiveLayerErrorV2::CanonicalBlockPartitionMismatch);
    }
    input.blocks.sort_unstable_by_key(block_input_key_v2);
    let mut retained = Vec::new();
    retained
        .try_reserve_exact(input.blocks.len())
        .map_err(|_| CompleteMultiBlockPositiveLayerErrorV2::ResourceLimit)?;
    let mut exact_admissions = HashSet::new();
    revalidate_parent_admission_once_v2(
        input.parent_graph_admission,
        input.geometry,
        &mut exact_admissions,
        &mut checkpoint,
    )?;
    if !input.whole_parent_positive.is_for_v2(
        CommonArticulationPositiveThicknessContinuousRevalidationInputV2 {
            geometry: input.geometry,
            audit: input.audit,
            decomposition: input.decomposition,
            configured_max_blocks: input.configured_max_blocks,
            fixed_face: input.whole_parent_closure.fixed_face(),
            schedule: input.whole_parent_schedule,
            closure: input.whole_parent_closure,
            paper_thickness_mm: input.paper_thickness_mm,
            graph_limits: input.positive_graph_limits,
            parent_graph_admission: input.parent_graph_admission,
        },
    ) {
        return Err(CompleteMultiBlockPositiveLayerErrorV2::WholeParentPositiveBindingMismatch);
    }
    let closure_parent_binding = input.closure_parent.binding;
    if input.closure_parent.thickness_bits != input.paper_thickness_mm.to_bits()
        || input.closure_parent.issuer_context != input.issuer_context
        || recompute_closure_parent_binding_v2(&input.closure_parent, &mut checkpoint)?
            != closure_parent_binding
    {
        return Err(CompleteMultiBlockPositiveLayerErrorV2::CanonicalBlockPartitionMismatch);
    }
    for (((owned, submitted), expected), expected_source) in input
        .closure_parent
        .blocks
        .into_iter()
        .zip(input.blocks)
        .zip(canonical.iter())
        .zip(input.block_sources.iter().copied())
    {
        checkpoint()?;
        let observed = canonical_binding_for_geometry_v2(submitted.geometry, &mut checkpoint)?;
        if observed != *expected
            || owned.edges != expected.edges
            || owned.faces != expected.faces
            || !owned.geometry.same_instance(submitted.geometry)
            || !std::ptr::eq(submitted.source, expected_source)
        {
            return Err(CompleteMultiBlockPositiveLayerErrorV2::CanonicalBlockPartitionMismatch);
        }
        validate_restricted_schedule_v2(
            RestrictedScheduleValidationInputV2 {
                geometry: input.geometry,
                audit: input.audit,
                whole: input.whole_parent_schedule,
                block_geometry: submitted.geometry,
                block_audit: &owned.audit,
                retained: &owned.schedule,
                fixed_face: owned.closure.fixed_face(),
            },
            &mut checkpoint,
        )?;
        let graph_limits = submitted.positive.graph_limits_v2();
        let admission = submitted.positive.retained_parent_graph_admission_v2();
        revalidate_parent_admission_once_v2(
            admission,
            submitted.geometry,
            &mut exact_admissions,
            &mut checkpoint,
        )?;
        if !submitted
            .positive
            .is_for_v2(AdmittedPositiveThicknessContinuousRevalidationInputV2 {
                geometry: submitted.geometry,
                audit: &owned.audit,
                fixed_face: owned.closure.fixed_face(),
                schedule: &owned.schedule,
                closure: &owned.closure,
                paper_thickness_mm: input.paper_thickness_mm,
                graph_limits,
                admission,
            })
        {
            return Err(CompleteMultiBlockPositiveLayerErrorV2::PositiveBindingMismatch);
        }
        if !submitted
            .layer
            .is_for_v2(AdmittedGeneralCellTransportInputV2 {
                geometry: submitted.geometry,
                audit: &owned.audit,
                source: submitted.source,
                schedule: &owned.schedule,
                closure: &owned.closure,
                positive_continuous: &submitted.positive,
                paper_thickness_mm: input.paper_thickness_mm,
                limits: crate::GeneralCellTransportLimitsV1 {
                    max_transitions: owned.closure.leaves().len() + 1,
                    max_cells: usize::MAX,
                    max_layer_records: usize::MAX,
                    max_boundary_samples: usize::MAX,
                },
            })
        {
            // `is_for_v2` does not use transport limits, but spelling the live
            // tuple here keeps this bridge closed if that changes later.
            return Err(CompleteMultiBlockPositiveLayerErrorV2::TransportBindingMismatch);
        }
        let (positive, positive_evidence) = submitted.positive.into_parts_for_complete_v2();
        let (layer, layer_evidence) = submitted.layer.into_parts_for_complete_v2();
        retained.push(RetainedAdmittedBlockV2 {
            schedule: owned.schedule,
            closure: owned.closure,
            positive,
            positive_evidence,
            layer,
            layer_evidence,
        });
    }
    validate_source_restrictions_v2(
        input.source,
        &canonical,
        input.block_sources,
        &mut checkpoint,
    )?;
    validate_target_angles_v2(&retained, input.target_angles, &mut checkpoint)?;
    let mut authority = CompleteMultiBlockPositiveLayerAuthorityV2 {
        issuer: input.geometry.instance_anchor_v1(),
        binding: [0; 32],
        configured_max_blocks: input.configured_max_blocks,
        actual_block_count: canonical.len(),
        blocks: canonical,
        retained,
        whole_parent_positive: input.whole_parent_positive,
        closure_parent_binding,
        limits: input.limits,
        resources: CompleteMultiBlockPositiveLayerResourcesV2::default(),
    };
    authority.resources = retained_resources_v2(&authority, input.geometry, &mut checkpoint)?;
    enforce_resources_v2(authority.resources, authority.limits)?;
    let live = CompleteMultiBlockPositiveLayerRevalidationInputV2 {
        geometry: input.geometry,
        audit: input.audit,
        decomposition: input.decomposition,
        configured_max_blocks: input.configured_max_blocks,
        source: input.source,
        block_sources: input.block_sources,
        paper_thickness_mm: input.paper_thickness_mm,
        issuer_context: input.issuer_context,
        articulation_layer_fingerprint: input.articulation_layer_fingerprint,
        target_angles: input.target_angles,
        whole_parent_schedule: input.whole_parent_schedule,
        whole_parent_closure: input.whole_parent_closure,
        positive_graph_limits: input.positive_graph_limits,
        parent_graph_admission: input.parent_graph_admission,
    };
    authority.binding = complete_binding_v2(&authority, live, &mut checkpoint)?;
    checkpoint()?;
    Ok(authority)
}

struct CompleteCommonValidationInputV2<'a> {
    geometry: &'a MaterialHingeGraphGeometry,
    decomposition: &'a CanonicalMaterialEdgeBlockDecompositionV1,
    configured_max_blocks: usize,
    thickness: f64,
    issuer_context: [u8; 32],
    layer_fingerprint: [u8; 32],
    sources: &'a [&'a LayerOrderSnapshot],
    limits: CompleteMultiBlockPositiveLayerLimitsV2,
}

fn validate_common_input_v2(
    input: CompleteCommonValidationInputV2<'_>,
) -> Result<(), CompleteMultiBlockPositiveLayerErrorV2> {
    let actual = input.decomposition.blocks().len();
    if !input.decomposition.is_for_geometry(input.geometry)
        || !(BOUNDED_MULTI_BLOCK_EXTENSION_MIN_BLOCKS_V1..=input.limits.max_blocks)
            .contains(&input.configured_max_blocks)
        || !(BOUNDED_MULTI_BLOCK_EXTENSION_MIN_BLOCKS_V1..=input.configured_max_blocks)
            .contains(&actual)
        || input.sources.len() != actual
        || !input.thickness.is_finite()
        || input.thickness <= 0.0
        || input.issuer_context == [0; 32]
        || input.layer_fingerprint == [0; 32]
        || !input.limits.is_valid_v2()
    {
        return Err(CompleteMultiBlockPositiveLayerErrorV2::InvalidInput);
    }
    Ok(())
}

fn validate_retained_v2(
    authority: &CompleteMultiBlockPositiveLayerAuthorityV2,
    input: CompleteMultiBlockPositiveLayerRevalidationInputV2<'_>,
    control: &CooperativeOperationControlV1<'_>,
    checkpoint: &mut impl FnMut() -> Result<(), CompleteMultiBlockPositiveLayerErrorV2>,
) -> Result<CompleteMultiBlockPositiveLayerResourcesV2, CompleteMultiBlockPositiveLayerErrorV2> {
    checkpoint()?;
    validate_common_input_v2(CompleteCommonValidationInputV2 {
        geometry: input.geometry,
        decomposition: input.decomposition,
        configured_max_blocks: input.configured_max_blocks,
        thickness: input.paper_thickness_mm,
        issuer_context: input.issuer_context,
        layer_fingerprint: input.articulation_layer_fingerprint,
        sources: input.block_sources,
        limits: authority.limits,
    })?;
    if !authority.issuer.matches(input.geometry)
        || authority.configured_max_blocks != input.configured_max_blocks
        || authority.actual_block_count != input.decomposition.blocks().len()
        || authority.retained.len() != authority.blocks.len()
    {
        return Err(CompleteMultiBlockPositiveLayerErrorV2::BindingMismatch);
    }
    let canonical = canonical_decomposition_block_bindings_v1(input.decomposition, control)
        .map_err(map_canonical_error_v2)?;
    if canonical != authority.blocks || !block_articulation_incidence_is_tree_v1(&canonical) {
        return Err(CompleteMultiBlockPositiveLayerErrorV2::CanonicalBlockPartitionMismatch);
    }
    let whole_admission = authority
        .whole_parent_positive
        .retained_parent_graph_admission_v2();
    if !whole_admission.same_evidence_v2(input.parent_graph_admission) {
        return Err(CompleteMultiBlockPositiveLayerErrorV2::WholeParentPositiveBindingMismatch);
    }
    let mut exact_admissions = HashSet::new();
    revalidate_parent_admission_once_v2(
        input.parent_graph_admission,
        input.geometry,
        &mut exact_admissions,
        checkpoint,
    )?;
    if !authority.whole_parent_positive.is_for_v2(
        CommonArticulationPositiveThicknessContinuousRevalidationInputV2 {
            geometry: input.geometry,
            audit: input.audit,
            decomposition: input.decomposition,
            configured_max_blocks: input.configured_max_blocks,
            fixed_face: input.whole_parent_closure.fixed_face(),
            schedule: input.whole_parent_schedule,
            closure: input.whole_parent_closure,
            paper_thickness_mm: input.paper_thickness_mm,
            graph_limits: input.positive_graph_limits,
            parent_graph_admission: input.parent_graph_admission,
        },
    ) {
        return Err(CompleteMultiBlockPositiveLayerErrorV2::WholeParentPositiveBindingMismatch);
    }
    let mut live_blocks = Vec::new();
    live_blocks
        .try_reserve_exact(input.decomposition.blocks().len())
        .map_err(|_| CompleteMultiBlockPositiveLayerErrorV2::ResourceLimit)?;
    for block in input.decomposition.blocks() {
        checkpoint()?;
        live_blocks.push(block);
    }
    live_blocks.sort_unstable_by_key(|block| {
        block
            .geometry()
            .hinges()
            .iter()
            .map(|hinge| hinge.edge().canonical_bytes())
            .min()
            .unwrap_or([0; 16])
    });
    for (((block, retained), live), source) in authority
        .blocks
        .iter()
        .zip(&authority.retained)
        .zip(live_blocks)
        .zip(input.block_sources.iter().copied())
    {
        checkpoint()?;
        if canonical_binding_for_geometry_v2(live.geometry(), checkpoint)? != *block {
            return Err(CompleteMultiBlockPositiveLayerErrorV2::CanonicalBlockPartitionMismatch);
        }
        validate_restricted_schedule_v2(
            RestrictedScheduleValidationInputV2 {
                geometry: input.geometry,
                audit: input.audit,
                whole: input.whole_parent_schedule,
                block_geometry: live.geometry(),
                block_audit: live.audit(),
                retained: &retained.schedule,
                fixed_face: retained.closure.fixed_face(),
            },
            checkpoint,
        )?;
        let admission = retained
            .positive_evidence
            .retained_parent_graph_admission_v2();
        revalidate_parent_admission_once_v2(
            admission,
            live.geometry(),
            &mut exact_admissions,
            checkpoint,
        )?;
        let graph_limits = retained.positive_evidence.graph_limits_v2();
        if !retained.positive_evidence.is_for_v2(
            live.geometry(),
            live.audit(),
            retained.closure.fixed_face(),
            &retained.schedule,
            &retained.closure,
            graph_limits,
        ) || !retained.positive.is_for(
            live.geometry(),
            live.audit(),
            retained.closure.fixed_face(),
            &retained.schedule,
            &retained.closure,
            input.paper_thickness_mm,
        ) {
            return Err(CompleteMultiBlockPositiveLayerErrorV2::PositiveBindingMismatch);
        }
        checkpoint()?;
        let layer_matches = retained
            .layer
            .is_for_with_checkpoint_v1(
                live.geometry(),
                source,
                &retained.schedule,
                &retained.closure,
                input.paper_thickness_mm,
                || checkpoint_to_stop_v2(checkpoint()),
            )
            .map_err(map_stop_v2)?;
        if !layer_matches
            || !retained
                .layer_evidence
                .is_for_v2(&retained.layer, &retained.positive_evidence)
        {
            return Err(CompleteMultiBlockPositiveLayerErrorV2::TransportBindingMismatch);
        }
    }
    validate_source_restrictions_v2(
        input.source,
        &authority.blocks,
        input.block_sources,
        checkpoint,
    )?;
    validate_target_angles_v2(&authority.retained, input.target_angles, checkpoint)?;
    let resources = retained_resources_v2(authority, input.geometry, checkpoint)?;
    enforce_resources_v2(resources, authority.limits)?;
    Ok(resources)
}

fn block_input_key_v2(input: &AdmittedMultiBlockPositiveLayerInputV2<'_>) -> [u8; 16] {
    input
        .geometry
        .hinges()
        .iter()
        .map(|hinge| hinge.edge().canonical_bytes())
        .min()
        .unwrap_or([0; 16])
}

fn canonical_binding_for_geometry_v2(
    geometry: &MaterialHingeGraphGeometry,
    checkpoint: &mut impl FnMut() -> Result<(), CompleteMultiBlockPositiveLayerErrorV2>,
) -> Result<CanonicalBlockBindingV1, CompleteMultiBlockPositiveLayerErrorV2> {
    let mut edges = Vec::new();
    edges
        .try_reserve_exact(geometry.hinges().len())
        .map_err(|_| CompleteMultiBlockPositiveLayerErrorV2::ResourceLimit)?;
    for hinge in geometry.hinges() {
        checkpoint()?;
        edges.push(hinge.edge());
    }
    edges.sort_unstable_by_key(EdgeId::canonical_bytes);
    let mut faces = Vec::new();
    faces
        .try_reserve_exact(geometry.face_ids().len())
        .map_err(|_| CompleteMultiBlockPositiveLayerErrorV2::ResourceLimit)?;
    for face in geometry.face_ids() {
        checkpoint()?;
        faces.push(*face);
    }
    faces.sort_unstable_by_key(FaceId::canonical_bytes);
    if edges.is_empty()
        || faces.is_empty()
        || edges.windows(2).any(|pair| pair[0] == pair[1])
        || faces.windows(2).any(|pair| pair[0] == pair[1])
    {
        return Err(CompleteMultiBlockPositiveLayerErrorV2::CanonicalBlockPartitionMismatch);
    }
    Ok(CanonicalBlockBindingV1 { edges, faces })
}

fn recompute_closure_parent_binding_v2(
    parent: &MultiBlockClosureAuthorityV1,
    checkpoint: &mut impl FnMut() -> Result<(), CompleteMultiBlockPositiveLayerErrorV2>,
) -> Result<[u8; 32], CompleteMultiBlockPositiveLayerErrorV2> {
    let mut hash = Sha256::new();
    hash.update(super::MULTI_BLOCK_POSITIVE_LAYER_MODEL_ID_V1.as_bytes());
    parent
        .scope
        .bind_closure_domain_v1(parent.blocks.len(), &mut hash);
    hash.update(parent.thickness_bits.to_le_bytes());
    hash.update(parent.issuer_context);
    for block in &parent.blocks {
        checkpoint()?;
        hash.update(block.schedule.graph_binding_fingerprint_v1());
        hash.update(block.schedule.certificate_binding_fingerprint_v2());
        hash.update(block.closure.partition_binding_fingerprint_v2());
        hash.update((block.edges.len() as u64).to_le_bytes());
        for edge in &block.edges {
            checkpoint()?;
            hash.update(edge.canonical_bytes());
        }
        for face in &block.faces {
            checkpoint()?;
            hash.update(face.canonical_bytes());
        }
    }
    Ok(hash.finalize().into())
}

struct RestrictedScheduleValidationInputV2<'a> {
    geometry: &'a MaterialHingeGraphGeometry,
    audit: &'a MaterialHingeGraphAudit,
    whole: &'a CanonicalCycleScheduleV1,
    block_geometry: &'a MaterialHingeGraphGeometry,
    block_audit: &'a MaterialHingeGraphAudit,
    retained: &'a CanonicalCycleScheduleV1,
    fixed_face: FaceId,
}

fn validate_restricted_schedule_v2(
    input: RestrictedScheduleValidationInputV2<'_>,
    checkpoint: &mut impl FnMut() -> Result<(), CompleteMultiBlockPositiveLayerErrorV2>,
) -> Result<(), CompleteMultiBlockPositiveLayerErrorV2> {
    let mut unexpected = None;
    let restricted = input
        .whole
        .restrict_to_edge_block_with_fixed_face_with_checkpoint_v1(
            input.geometry,
            input.audit,
            input.block_geometry,
            input.block_audit,
            input.fixed_face,
            || match checkpoint() {
                Ok(()) => Ok(()),
                Err(CompleteMultiBlockPositiveLayerErrorV2::DeadlineExceeded) => {
                    Err(CycleScheduleRestrictionStopV1::DeadlineExceeded)
                }
                Err(CompleteMultiBlockPositiveLayerErrorV2::Cancelled) => {
                    Err(CycleScheduleRestrictionStopV1::Cancelled)
                }
                Err(error) => {
                    unexpected = Some(error);
                    Err(CycleScheduleRestrictionStopV1::Cancelled)
                }
            },
        );
    if let Some(error) = unexpected {
        return Err(error);
    }
    let restricted = restricted.map_err(|error| match error {
        CycleScheduleRestrictionErrorV1::Cancelled => {
            CompleteMultiBlockPositiveLayerErrorV2::Cancelled
        }
        CycleScheduleRestrictionErrorV1::DeadlineExceeded => {
            CompleteMultiBlockPositiveLayerErrorV2::DeadlineExceeded
        }
        CycleScheduleRestrictionErrorV1::Prepare(_) => {
            CompleteMultiBlockPositiveLayerErrorV2::BlockScheduleRestrictionMismatch
        }
    })?;
    if restricted != *input.retained {
        return Err(CompleteMultiBlockPositiveLayerErrorV2::BlockScheduleRestrictionMismatch);
    }
    Ok(())
}

fn validate_source_restrictions_v2(
    whole: &LayerOrderSnapshot,
    blocks: &[CanonicalBlockBindingV1],
    sources: &[&LayerOrderSnapshot],
    checkpoint: &mut impl FnMut() -> Result<(), CompleteMultiBlockPositiveLayerErrorV2>,
) -> Result<(), CompleteMultiBlockPositiveLayerErrorV2> {
    if blocks.len() != sources.len() {
        return Err(CompleteMultiBlockPositiveLayerErrorV2::BlockSourceRestrictionMismatch);
    }
    for (block, source) in blocks.iter().zip(sources) {
        checkpoint()?;
        let retained = whole
            .checked_restricted_deep_retained_bytes_v1(&block.faces)
            .ok_or(CompleteMultiBlockPositiveLayerErrorV2::ResourceLimit)?;
        let expected = whole
            .try_restrict_to_faces_with_retained_byte_limit_v1(&block.faces, retained)
            .map_err(|_| CompleteMultiBlockPositiveLayerErrorV2::ResourceLimit)?;
        if &expected != *source {
            return Err(CompleteMultiBlockPositiveLayerErrorV2::BlockSourceRestrictionMismatch);
        }
    }
    Ok(())
}

fn validate_target_angles_v2(
    retained: &[RetainedAdmittedBlockV2],
    actual: &[(EdgeId, f64)],
    checkpoint: &mut impl FnMut() -> Result<(), CompleteMultiBlockPositiveLayerErrorV2>,
) -> Result<(), CompleteMultiBlockPositiveLayerErrorV2> {
    let mut expected = Vec::new();
    for block in retained {
        checkpoint()?;
        let endpoint = block
            .schedule
            .evaluate(1.0)
            .ok_or(CompleteMultiBlockPositiveLayerErrorV2::TargetAngleMismatch)?;
        for angle in endpoint.as_slice() {
            checkpoint()?;
            expected.push((angle.edge(), angle.angle_degrees()));
        }
    }
    expected.sort_unstable_by_key(|(edge, _)| edge.canonical_bytes());
    let mut actual = actual.to_vec();
    actual.sort_unstable_by_key(|(edge, _)| edge.canonical_bytes());
    if expected.len() != actual.len()
        || expected.windows(2).any(|pair| pair[0].0 == pair[1].0)
        || expected
            .iter()
            .zip(actual)
            .any(|(left, right)| left.0 != right.0 || left.1.to_bits() != right.1.to_bits())
    {
        return Err(CompleteMultiBlockPositiveLayerErrorV2::TargetAngleMismatch);
    }
    Ok(())
}

fn revalidate_parent_admission_once_v2(
    admission: &CommonArticulationPositiveThicknessParentGraphAdmissionV2,
    geometry: &MaterialHingeGraphGeometry,
    observed: &mut HashSet<usize>,
    checkpoint: &mut impl FnMut() -> Result<(), CompleteMultiBlockPositiveLayerErrorV2>,
) -> Result<(), CompleteMultiBlockPositiveLayerErrorV2> {
    let address =
        admission as *const CommonArticulationPositiveThicknessParentGraphAdmissionV2 as usize;
    if !observed.insert(address) {
        return Ok(());
    }
    revalidate_common_articulation_positive_thickness_parent_graph_admission_with_checkpoint_v2(
        admission,
        geometry,
        || match checkpoint() {
            Ok(()) => Ok(()),
            Err(CompleteMultiBlockPositiveLayerErrorV2::DeadlineExceeded) => {
                Err(CommonArticulationPositiveThicknessParentGraphAdmissionStopV2::DeadlineExceeded)
            }
            Err(_) => Err(CommonArticulationPositiveThicknessParentGraphAdmissionStopV2::Cancelled),
        },
    )
    .map_err(map_parent_admission_error_v2)
}

fn retained_resources_v2(
    authority: &CompleteMultiBlockPositiveLayerAuthorityV2,
    geometry: &MaterialHingeGraphGeometry,
    checkpoint: &mut impl FnMut() -> Result<(), CompleteMultiBlockPositiveLayerErrorV2>,
) -> Result<CompleteMultiBlockPositiveLayerResourcesV2, CompleteMultiBlockPositiveLayerErrorV2> {
    let mut resources = CompleteMultiBlockPositiveLayerResourcesV2 {
        block_count: authority.blocks.len(),
        face_count: geometry.face_ids().len(),
        hinge_count: geometry.hinges().len(),
        ..CompleteMultiBlockPositiveLayerResourcesV2::default()
    };
    resources.face_pair_tests = checked_pairs_v2(resources.face_count)?;
    let mut parent_addresses = HashSet::new();
    let whole_parent = authority
        .whole_parent_positive
        .retained_parent_graph_admission_v2();
    parent_addresses.insert(whole_parent as *const _ as usize);
    // The whole-parent extension retains two Arc handles to one admission:
    // one directly and one inside its shared-contact certificate.  Only the
    // allocation is charged once; the inline Arc handles are already in the
    // authority shell.
    let mut parent_references = 2usize;
    resources.logical_work = whole_parent.resources_v2().logical_work_v2();
    let canonical_capacity_bytes = size_of::<CanonicalBlockBindingV1>()
        .checked_mul(authority.blocks.capacity())
        .ok_or(CompleteMultiBlockPositiveLayerErrorV2::ResourceLimit)?;
    let retained_capacity_bytes = size_of::<RetainedAdmittedBlockV2>()
        .checked_mul(authority.retained.capacity())
        .ok_or(CompleteMultiBlockPositiveLayerErrorV2::ResourceLimit)?;
    let mut retained_bytes = size_of::<CompleteMultiBlockPositiveLayerAuthorityV2>()
        .checked_add(canonical_capacity_bytes)
        .and_then(|value| value.checked_add(retained_capacity_bytes))
        .ok_or(CompleteMultiBlockPositiveLayerErrorV2::ResourceLimit)?;
    retained_bytes = retained_bytes
        .checked_add(size_of::<
            CommonArticulationPositiveThicknessParentGraphAdmissionV2,
        >())
        .ok_or(CompleteMultiBlockPositiveLayerErrorV2::ResourceLimit)?;
    for block in &authority.blocks {
        checkpoint()?;
        resources.block_face_occurrences = resources
            .block_face_occurrences
            .checked_add(block.faces.len())
            .ok_or(CompleteMultiBlockPositiveLayerErrorV2::ResourceLimit)?;
        resources.block_hinge_occurrences = resources
            .block_hinge_occurrences
            .checked_add(block.edges.len())
            .ok_or(CompleteMultiBlockPositiveLayerErrorV2::ResourceLimit)?;
        retained_bytes = retained_bytes
            .checked_add(
                size_of::<FaceId>()
                    .checked_mul(block.faces.capacity())
                    .ok_or(CompleteMultiBlockPositiveLayerErrorV2::ResourceLimit)?,
            )
            .and_then(|value| {
                value.checked_add(size_of::<EdgeId>().checked_mul(block.edges.capacity())?)
            })
            .ok_or(CompleteMultiBlockPositiveLayerErrorV2::ResourceLimit)?;
    }
    for (binding, retained) in authority.blocks.iter().zip(&authority.retained) {
        checkpoint()?;
        resources.face_pair_tests = resources
            .face_pair_tests
            .checked_add(checked_pairs_v2(binding.faces.len())?)
            .ok_or(CompleteMultiBlockPositiveLayerErrorV2::ResourceLimit)?;
        resources.transition_count = resources
            .transition_count
            .checked_add(retained.layer.transition_hashes().len())
            .ok_or(CompleteMultiBlockPositiveLayerErrorV2::ResourceLimit)?;
        resources.pair_order_count = resources
            .pair_order_count
            .checked_add(retained.layer.pair_order_count())
            .ok_or(CompleteMultiBlockPositiveLayerErrorV2::ResourceLimit)?;
        let schedule_dynamic = retained
            .schedule
            .checked_deep_retained_bytes_v1()
            .and_then(|value| value.checked_sub(size_of::<CanonicalCycleScheduleV1>()))
            .ok_or(CompleteMultiBlockPositiveLayerErrorV2::ResourceLimit)?;
        let closure_dynamic = retained
            .closure
            .checked_deep_retained_bytes_v1()
            .and_then(|value| {
                value.checked_sub(size_of::<DyadicMaterialHingeIntervalClosureCertificateV1>())
            })
            .ok_or(CompleteMultiBlockPositiveLayerErrorV2::ResourceLimit)?;
        let layer_dynamic = retained
            .layer
            .checked_deep_retained_bytes_v1()
            .and_then(|value| value.checked_sub(size_of::<GeneralMultiFaceCellTransportProofV1>()))
            .ok_or(CompleteMultiBlockPositiveLayerErrorV2::ResourceLimit)?;
        let positive_evidence_dynamic = retained
            .positive_evidence
            .checked_deep_retained_bytes_v2()
            .and_then(|value| {
                value.checked_sub(size_of::<AdmittedPositiveThicknessRetainedEvidenceV2>())
            })
            .and_then(|value| {
                value.checked_sub(size_of::<
                    CommonArticulationPositiveThicknessParentGraphAdmissionV2,
                >())
            })
            .ok_or(CompleteMultiBlockPositiveLayerErrorV2::ResourceLimit)?;
        let layer_evidence_dynamic = retained
            .layer_evidence
            .checked_deep_retained_bytes_v2()
            .and_then(|value| {
                value.checked_sub(size_of::<AdmittedGeneralCellTransportRetainedEvidenceV2>())
            })
            .ok_or(CompleteMultiBlockPositiveLayerErrorV2::ResourceLimit)?;
        retained_bytes = retained_bytes
            .checked_add(schedule_dynamic)
            .and_then(|value| value.checked_add(closure_dynamic))
            .and_then(|value| value.checked_add(layer_dynamic))
            .and_then(|value| value.checked_add(positive_evidence_dynamic))
            .and_then(|value| value.checked_add(layer_evidence_dynamic))
            .ok_or(CompleteMultiBlockPositiveLayerErrorV2::ResourceLimit)?;
        let parent = retained
            .positive_evidence
            .retained_parent_graph_admission_v2();
        parent_references = parent_references
            .checked_add(1)
            .ok_or(CompleteMultiBlockPositiveLayerErrorV2::ResourceLimit)?;
        if parent_addresses.insert(parent as *const _ as usize) {
            retained_bytes = retained_bytes
                .checked_add(size_of::<
                    CommonArticulationPositiveThicknessParentGraphAdmissionV2,
                >())
                .ok_or(CompleteMultiBlockPositiveLayerErrorV2::ResourceLimit)?;
            resources.logical_work = resources
                .logical_work
                .checked_add(parent.resources_v2().logical_work_v2())
                .ok_or(CompleteMultiBlockPositiveLayerErrorV2::ResourceLimit)?;
        }
    }
    resources.logical_work = resources
        .logical_work
        .checked_add(resources.face_pair_tests)
        .and_then(|value| value.checked_add(resources.block_face_occurrences))
        .and_then(|value| value.checked_add(resources.block_hinge_occurrences))
        .and_then(|value| value.checked_add(resources.transition_count))
        .and_then(|value| value.checked_add(resources.pair_order_count))
        .ok_or(CompleteMultiBlockPositiveLayerErrorV2::ResourceLimit)?;
    resources.deep_retained_bytes = retained_bytes;
    resources.retained_parent_count = parent_addresses.len();
    resources.retained_parent_alias_count =
        parent_references.saturating_sub(parent_addresses.len());
    Ok(resources)
}

fn enforce_resources_v2(
    resources: CompleteMultiBlockPositiveLayerResourcesV2,
    limits: CompleteMultiBlockPositiveLayerLimitsV2,
) -> Result<(), CompleteMultiBlockPositiveLayerErrorV2> {
    if resources.block_count > limits.max_blocks
        || resources.face_count > limits.max_faces
        || resources.hinge_count > limits.max_hinges
        || resources.face_pair_tests > limits.max_face_pair_tests
        || resources.logical_work > limits.max_logical_work
        || resources.deep_retained_bytes > limits.max_deep_retained_bytes
    {
        return Err(CompleteMultiBlockPositiveLayerErrorV2::ResourceLimit);
    }
    Ok(())
}

fn checked_pairs_v2(count: usize) -> Result<usize, CompleteMultiBlockPositiveLayerErrorV2> {
    count
        .checked_mul(count.saturating_sub(1))
        .and_then(|value| value.checked_div(2))
        .ok_or(CompleteMultiBlockPositiveLayerErrorV2::ResourceLimit)
}

fn complete_binding_v2(
    authority: &CompleteMultiBlockPositiveLayerAuthorityV2,
    input: CompleteMultiBlockPositiveLayerRevalidationInputV2<'_>,
    checkpoint: &mut impl FnMut() -> Result<(), CompleteMultiBlockPositiveLayerErrorV2>,
) -> Result<[u8; 32], CompleteMultiBlockPositiveLayerErrorV2> {
    let mut hash = Sha256::new();
    hash.update(ADMITTED_MULTI_BLOCK_POSITIVE_LAYER_MODEL_ID_V2.as_bytes());
    for value in [
        BOUNDED_MULTI_BLOCK_EXTENSION_MIN_BLOCKS_V1,
        authority.configured_max_blocks,
        authority.actual_block_count,
        authority.limits.max_blocks,
        authority.limits.max_faces,
        authority.limits.max_hinges,
        authority.limits.max_face_pair_tests,
        authority.limits.max_logical_work,
        authority.limits.max_deep_retained_bytes,
        authority.resources.block_count,
        authority.resources.face_count,
        authority.resources.hinge_count,
        authority.resources.block_face_occurrences,
        authority.resources.block_hinge_occurrences,
        authority.resources.face_pair_tests,
        authority.resources.transition_count,
        authority.resources.pair_order_count,
        authority.resources.logical_work,
        authority.resources.deep_retained_bytes,
        authority.resources.retained_parent_count,
        authority.resources.retained_parent_alias_count,
    ] {
        hash.update((value as u64).to_le_bytes());
    }
    hash.update(authority.closure_parent_binding);
    hash.update(
        input
            .whole_parent_schedule
            .certificate_binding_fingerprint_v2(),
    );
    hash.update(
        input
            .whole_parent_closure
            .partition_binding_fingerprint_v2(),
    );
    hash.update(input.paper_thickness_mm.to_bits().to_le_bytes());
    hash.update(input.issuer_context);
    hash.update(input.articulation_layer_fingerprint);
    hash.update(authority.whole_parent_positive.binding_fingerprint_v2());
    update_admission_binding_v2(&mut hash, input.parent_graph_admission);
    update_layer_source_binding_v2(&mut hash, input.source);
    hash.update((input.block_sources.len() as u64).to_le_bytes());
    for source in input.block_sources {
        checkpoint()?;
        update_layer_source_binding_v2(&mut hash, source);
    }
    for block in &authority.blocks {
        checkpoint()?;
        hash.update((block.edges.len() as u64).to_le_bytes());
        for edge in &block.edges {
            checkpoint()?;
            hash.update(edge.canonical_bytes());
        }
        hash.update((block.faces.len() as u64).to_le_bytes());
        for face in &block.faces {
            checkpoint()?;
            hash.update(face.canonical_bytes());
        }
    }
    for retained in &authority.retained {
        checkpoint()?;
        hash.update(retained.schedule.certificate_binding_fingerprint_v2());
        hash.update(retained.closure.partition_binding_fingerprint_v2());
        hash.update(retained.positive_evidence.binding_fingerprint_v2());
        hash.update(retained.layer_evidence.binding_fingerprint_v2());
        hash.update(
            retained
                .layer_evidence
                .parent_graph_admission_binding_fingerprint_v2(),
        );
        update_admission_binding_v2(
            &mut hash,
            retained
                .positive_evidence
                .retained_parent_graph_admission_v2(),
        );
        hash.update(retained.layer.target_order_hash());
        hash.update((retained.layer.transition_hashes().len() as u64).to_le_bytes());
        for item in retained.layer.transition_hashes() {
            checkpoint()?;
            hash.update(item);
        }
    }
    let mut targets = input.target_angles.to_vec();
    targets.sort_unstable_by_key(|(edge, _)| edge.canonical_bytes());
    for (edge, angle) in targets {
        checkpoint()?;
        hash.update(edge.canonical_bytes());
        hash.update(angle.to_bits().to_le_bytes());
    }
    Ok(hash.finalize().into())
}

fn update_layer_source_binding_v2(hash: &mut Sha256, source: &LayerOrderSnapshot) {
    hash.update(b"origami2/complete-multi-block-positive-layer/source/v2");
    hash.update(match source.model_id {
        ori_foldability::LayerOrderModelId::FacewiseLayerOrderV1 => [1],
    });
    let provenance = source.provenance.source;
    match provenance.identity_namespace {
        Some(namespace) => {
            hash.update([1]);
            hash.update(namespace.canonical_bytes());
        }
        None => hash.update([0]),
    }
    hash.update(provenance.source_revision.to_le_bytes());
    match provenance.source_fingerprint {
        Some(fingerprint) => {
            hash.update([1]);
            hash.update(fingerprint.0);
        }
        None => hash.update([0]),
    }
    hash.update(match provenance.model_id {
        ori_foldability::GlobalFlatFoldabilityModelId::ConvexFacesFacewiseV1 => [1],
    });
    for value in [
        source.material_faces.len(),
        source.folded_faces.len(),
        source.overlap_cells.len(),
        source.face_pair_orders.len(),
    ] {
        hash.update((value as u64).to_le_bytes());
    }
}

fn update_admission_binding_v2(
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

fn map_canonical_error_v2(
    error: super::CommonArticulationBlockComposedPathErrorV1,
) -> CompleteMultiBlockPositiveLayerErrorV2 {
    match error {
        super::CommonArticulationBlockComposedPathErrorV1::Cancelled => {
            CompleteMultiBlockPositiveLayerErrorV2::Cancelled
        }
        super::CommonArticulationBlockComposedPathErrorV1::DeadlineExceeded => {
            CompleteMultiBlockPositiveLayerErrorV2::DeadlineExceeded
        }
        super::CommonArticulationBlockComposedPathErrorV1::ResourceLimit => {
            CompleteMultiBlockPositiveLayerErrorV2::ResourceLimit
        }
        _ => CompleteMultiBlockPositiveLayerErrorV2::CanonicalBlockPartitionMismatch,
    }
}

fn map_parent_admission_error_v2(
    error: CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2,
) -> CompleteMultiBlockPositiveLayerErrorV2 {
    match error {
        CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::Cancelled => {
            CompleteMultiBlockPositiveLayerErrorV2::Cancelled
        }
        CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::DeadlineExceeded => {
            CompleteMultiBlockPositiveLayerErrorV2::DeadlineExceeded
        }
        CommonArticulationPositiveThicknessParentGraphAdmissionErrorV2::ResourceLimit => {
            CompleteMultiBlockPositiveLayerErrorV2::ResourceLimit
        }
        error => CompleteMultiBlockPositiveLayerErrorV2::ParentGraphAdmission(error),
    }
}

fn checkpoint_to_stop_v2(
    result: Result<(), CompleteMultiBlockPositiveLayerErrorV2>,
) -> Result<(), CooperativeOperationStopV1> {
    result.map_err(|error| match error {
        CompleteMultiBlockPositiveLayerErrorV2::DeadlineExceeded => {
            CooperativeOperationStopV1::DeadlineExceeded
        }
        _ => CooperativeOperationStopV1::Cancelled,
    })
}

fn map_stop_v2(stop: CooperativeOperationStopV1) -> CompleteMultiBlockPositiveLayerErrorV2 {
    match stop {
        CooperativeOperationStopV1::Cancelled => CompleteMultiBlockPositiveLayerErrorV2::Cancelled,
        CooperativeOperationStopV1::DeadlineExceeded => {
            CompleteMultiBlockPositiveLayerErrorV2::DeadlineExceeded
        }
    }
}

fn complete_checkpoint_v2(
    control: &CooperativeOperationControlV1<'_>,
) -> Result<(), CompleteMultiBlockPositiveLayerErrorV2> {
    control.checkpoint().map_err(map_stop_v2)
}
