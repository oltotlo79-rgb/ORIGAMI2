//! Authenticated source-order coverage by the Phase 3F relieved-clearance domain.
//!
//! This boundary proves one deliberately narrow composition fact: every
//! directional pair in the live authenticated flat source names two distinct
//! canonical material faces in the same geometry whose complete unordered
//! face-pair domain was replayed by Phase 3F. It does not prove that the source
//! direction is realized or preserved anywhere along the motion.
//!
//! Source identity is semantic: a separately allocated authority that passes
//! the complete live source replay is accepted. Pointer or report-instance
//! identity is deliberately not invented at this boundary.

use ori_foldability::{GlobalFlatFoldabilityProvenance, GlobalFlatLayerOrderSourceAuthorityV2};
use thiserror::Error;

use super::source_binding::SourceMetricsV2;
use super::validation::{AuthenticatedLayerSourceLimitsV2, SourceCapErrorPolicyV2};
use crate::{
    CommonArticulationDynamicGeneralNRelievedClearanceCertificateV2,
    CommonArticulationDynamicGeneralNRelievedClearanceErrorV2,
    CommonArticulationDynamicGeneralNRelievedClearanceRevalidationInputV2,
    CommonArticulationDynamicGeneralNRelievedClearanceStopV2,
};

pub const COMMON_ARTICULATION_DYNAMIC_GENERAL_N_RELIEVED_SOURCE_ORDER_COVERAGE_MODEL_ID_V2: &str =
    "common_articulation_dynamic_general_n_relieved_source_order_coverage_v2";

const GENERAL_N_MIN_BLOCKS_V2: usize = 33;
const COVERAGE_WORKSPACE_BYTES_V2: usize = 1_024;

mod validation;

use validation::{checkpoint_v2, validate_coverage_v2};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageStopV2 {
    Cancelled,
    DeadlineExceeded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageErrorV2 {
    #[error("the relieved source-order coverage input is malformed")]
    InvalidInput,
    #[error("the relieved source-order coverage input exceeds its finite resource envelope")]
    ResourceLimit,
    #[error("the authenticated source is malformed, stale, or foreign")]
    SourceBindingMismatch,
    #[error("the retained Phase 3F relieved-clearance certificate does not replay: {0}")]
    Clearance(CommonArticulationDynamicGeneralNRelievedClearanceErrorV2),
    #[error("the relieved source-order coverage certificate does not match the live input")]
    CertificateBindingMismatch,
    #[error("the relieved source-order coverage operation was cancelled")]
    Cancelled,
    #[error("the relieved source-order coverage operation deadline elapsed")]
    DeadlineExceeded,
}

/// Finite caps for authenticating one source and publishing one coverage seal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageLimitsV2 {
    pub max_blocks: usize,
    pub max_source_retained_bytes: usize,
    pub max_material_faces: usize,
    pub max_folded_faces: usize,
    pub max_overlap_cells: usize,
    pub max_face_pair_orders: usize,
    pub max_global_order_faces: usize,
    pub max_layer_records: usize,
    pub max_boundary_vertices: usize,
    pub max_source_logical_work: usize,
    pub max_publication_bytes: usize,
    pub max_aggregate_peak_bytes: usize,
}

impl CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageLimitsV2 {
    fn source_limits_v2(self) -> AuthenticatedLayerSourceLimitsV2 {
        AuthenticatedLayerSourceLimitsV2 {
            max_blocks: self.max_blocks,
            max_source_retained_bytes: self.max_source_retained_bytes,
            max_material_faces: self.max_material_faces,
            max_folded_faces: self.max_folded_faces,
            max_overlap_cells: self.max_overlap_cells,
            max_face_pair_orders: self.max_face_pair_orders,
            max_global_order_faces: self.max_global_order_faces,
            max_layer_records: self.max_layer_records,
            max_boundary_vertices: self.max_boundary_vertices,
            max_logical_work: self.max_source_logical_work,
            cap_error_policy: SourceCapErrorPolicyV2::ResourceLimit,
            enforce_derived_caps_during_source_scan: true,
        }
    }
}

/// Issue input. The Phase 3F certificate is consumed and retained by the
/// resulting coverage certificate; the live tuple is replayed in full.
pub struct CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageInputV2<'a> {
    pub clearance: CommonArticulationDynamicGeneralNRelievedClearanceCertificateV2,
    pub live: CommonArticulationDynamicGeneralNRelievedClearanceRevalidationInputV2<'a>,
    pub source_authority: &'a GlobalFlatLayerOrderSourceAuthorityV2<'a>,
    pub limits: CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageLimitsV2,
}

/// Exact live tuple required to replay a retained coverage certificate.
pub struct CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageRevalidationInputV2<'a> {
    pub live: CommonArticulationDynamicGeneralNRelievedClearanceRevalidationInputV2<'a>,
    pub source_authority: &'a GlobalFlatLayerOrderSourceAuthorityV2<'a>,
    pub limits: CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageLimitsV2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct CoverageResourcesV2 {
    source_logical_work: usize,
    source_retained_bytes: usize,
    clearance_peak_bytes: usize,
    publication_bytes: usize,
    aggregate_peak_bytes: usize,
}

/// Opaque proof that every authenticated source direction lies in Phase 3F's
/// canonical all-face-pair clearance domain.
///
/// This type has private fields and deliberately implements neither `Clone`,
/// serde, `Deref`, raw-source access, nor conversion into either input proof.
///
/// ```compile_fail
/// use ori_collision::CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageCertificateV2;
/// fn require_clone<T: Clone>() {}
/// require_clone::<CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageCertificateV2>();
/// ```
///
/// ```compile_fail
/// use ori_collision::CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageCertificateV2;
/// fn require_serialize<T: serde::Serialize>() {}
/// require_serialize::<CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageCertificateV2>();
/// ```
///
/// ```compile_fail
/// use ori_collision::CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageCertificateV2;
/// fn require_deserialize<T: serde::de::DeserializeOwned>() {}
/// require_deserialize::<CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageCertificateV2>();
/// ```
///
/// ```compile_fail
/// use std::ops::Deref;
/// use ori_collision::CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageCertificateV2;
/// fn require_deref<T: Deref>() {}
/// require_deref::<CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageCertificateV2>();
/// ```
///
/// ```compile_fail
/// use ori_collision::CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageCertificateV2;
/// fn fabricate() -> CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageCertificateV2 {
///     CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageCertificateV2 {}
/// }
/// ```
///
/// ```compile_fail
/// use ori_collision::CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageCertificateV2;
/// fn expose_raw(value: &CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageCertificateV2) {
///     let _ = value.source_digest;
///     let _ = value.binding_fingerprint;
/// }
/// ```
///
/// ```compile_fail
/// use ori_collision::CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageCertificateV2;
/// use ori_foldability::LayerOrderSnapshot;
/// fn require_raw_source<T: AsRef<LayerOrderSnapshot>>() {}
/// require_raw_source::<CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageCertificateV2>();
/// ```
///
/// ```compile_fail
/// use ori_collision::{
///     CommonArticulationDynamicGeneralNRelievedClearanceCertificateV2,
///     CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageCertificateV2,
/// };
/// fn downgrade(value: CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageCertificateV2)
///     -> CommonArticulationDynamicGeneralNRelievedClearanceCertificateV2
/// { value.into() }
/// ```
pub struct CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageCertificateV2 {
    clearance: CommonArticulationDynamicGeneralNRelievedClearanceCertificateV2,
    source_digest: [u8; 32],
    source_provenance: GlobalFlatFoldabilityProvenance,
    source_metrics: SourceMetricsV2,
    resources: CoverageResourcesV2,
    limits: CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageLimitsV2,
    binding_fingerprint: [u8; 32],
}

impl std::fmt::Debug for CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageCertificateV2 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct(
                "CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageCertificateV2",
            )
            .field("model", &self.model_id_v2())
            .field("actual_block_count", &self.actual_block_count_v2())
            .field("material_faces", &self.material_face_count_v2())
            .field("source_order_pairs", &self.source_order_pair_count_v2())
            .field("publication_bytes", &self.publication_bytes_v2())
            .field(
                "aggregate_peak_bytes",
                &self.aggregate_peak_bytes_upper_bound_v2(),
            )
            .finish_non_exhaustive()
    }
}

impl CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageCertificateV2 {
    #[must_use]
    pub const fn model_id_v2(&self) -> &'static str {
        COMMON_ARTICULATION_DYNAMIC_GENERAL_N_RELIEVED_SOURCE_ORDER_COVERAGE_MODEL_ID_V2
    }

    #[must_use]
    pub const fn actual_block_count_v2(&self) -> usize {
        self.clearance.actual_block_count_v2()
    }

    #[must_use]
    pub const fn material_face_count_v2(&self) -> usize {
        self.source_metrics.material_faces
    }

    #[must_use]
    pub const fn source_order_pair_count_v2(&self) -> usize {
        self.source_metrics.face_pair_orders
    }

    #[must_use]
    pub const fn source_validation_logical_work_v2(&self) -> usize {
        self.resources.source_logical_work
    }

    #[must_use]
    pub const fn source_retained_bytes_upper_bound_v2(&self) -> usize {
        self.resources.source_retained_bytes
    }

    #[must_use]
    pub const fn publication_bytes_v2(&self) -> usize {
        self.resources.publication_bytes
    }

    #[must_use]
    pub const fn aggregate_peak_bytes_upper_bound_v2(&self) -> usize {
        self.resources.aggregate_peak_bytes
    }

    #[must_use]
    pub const fn all_source_order_pairs_covered_by_relieved_clearance_v2(&self) -> bool {
        true
    }

    pub fn revalidate_v2(
        &self,
        input: CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageRevalidationInputV2<'_>,
    ) -> Result<(), CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageErrorV2> {
        self.revalidate_with_checkpoint_v2(input, || Ok(()))
    }

    pub fn revalidate_with_checkpoint_v2(
        &self,
        input: CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageRevalidationInputV2<'_>,
        mut checkpoint: impl FnMut() -> Result<
            (),
            CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageStopV2,
        >,
    ) -> Result<(), CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageErrorV2> {
        checkpoint_v2(&mut checkpoint)?;
        let validated = validate_coverage_v2(
            &self.clearance,
            input.live,
            input.source_authority,
            input.limits,
            &mut checkpoint,
        )?;
        if self.source_digest != validated.source.digest
            || self.source_provenance != validated.source.provenance
            || self.source_metrics != validated.source.metrics
            || self.resources != validated.resources
            || !coverage_limits_match_v2(self.limits, input.limits)
            || self.binding_fingerprint != validated.binding_fingerprint
        {
            checkpoint_v2(&mut checkpoint)?;
            return Err(
                CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageErrorV2::CertificateBindingMismatch,
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
    #[must_use]
    pub const fn authorizes_export(&self) -> bool {
        false
    }
}

const fn coverage_limits_match_v2(
    retained: CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageLimitsV2,
    live: CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageLimitsV2,
) -> bool {
    retained.max_blocks == live.max_blocks
        && retained.max_source_retained_bytes == live.max_source_retained_bytes
        && retained.max_material_faces == live.max_material_faces
        && retained.max_folded_faces == live.max_folded_faces
        && retained.max_overlap_cells == live.max_overlap_cells
        && retained.max_face_pair_orders == live.max_face_pair_orders
        && retained.max_global_order_faces == live.max_global_order_faces
        && retained.max_layer_records == live.max_layer_records
        && retained.max_boundary_vertices == live.max_boundary_vertices
        && retained.max_source_logical_work == live.max_source_logical_work
        && retained.max_publication_bytes == live.max_publication_bytes
        && retained.max_aggregate_peak_bytes == live.max_aggregate_peak_bytes
}

pub fn prove_common_articulation_dynamic_general_n_relieved_source_order_coverage_v2(
    input: CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageInputV2<'_>,
) -> Result<
    CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageCertificateV2,
    CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageErrorV2,
> {
    prove_common_articulation_dynamic_general_n_relieved_source_order_coverage_with_checkpoint_v2(
        input,
        || Ok(()),
    )
}

pub fn prove_common_articulation_dynamic_general_n_relieved_source_order_coverage_with_checkpoint_v2(
    input: CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageInputV2<'_>,
    mut checkpoint: impl FnMut() -> Result<
        (),
        CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageStopV2,
    >,
) -> Result<
    CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageCertificateV2,
    CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageErrorV2,
> {
    checkpoint_v2(&mut checkpoint)?;
    let validated = validate_coverage_v2(
        &input.clearance,
        input.live,
        input.source_authority,
        input.limits,
        &mut checkpoint,
    )?;
    checkpoint_v2(&mut checkpoint)?;
    Ok(
        CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageCertificateV2 {
            clearance: input.clearance,
            source_digest: validated.source.digest,
            source_provenance: validated.source.provenance,
            source_metrics: validated.source.metrics,
            resources: validated.resources,
            limits: input.limits,
            binding_fingerprint: validated.binding_fingerprint,
        },
    )
}

#[cfg(test)]
#[path = "dynamic_relieved_source_coverage/tests.rs"]
mod tests;
