//! Positive-thickness coverage at the two outer points of the closed dyadic domain.
//!
//! This Phase 3H prerequisite is a consuming promotion of the Phase 3G seal.
//! It proves only that each outer point of the canonical normalized dyadic
//! partition domain belongs to one accepted ordinary leaf and one accepted
//! shared-relief leaf in the retained Phase 3F proof. Production interval
//! evaluation encloses both exact rational endpoints of every closed leaf, so
//! the whole-leaf strict-separation result includes those two outer points.
//!
//! The two partitions may have different leaf depths. “Domain boundary” here
//! is not a source pose, target pose, `t = 0`, or `t = 1` assertion. This type
//! proves no pose equality or realization, signed order, direction
//! preservation, common-leaf invariant, target overlap-cell arrangement, or
//! layer transport.

use thiserror::Error;

use crate::dynamic_general_n_positive_thickness_v2::public_adapter::ClosedDyadicDomainBoundaryCoverageV2;

use super::{
    CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageCertificateV2,
    CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageErrorV2,
    CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageRevalidationInputV2,
    CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageStopV2,
};

pub const COMMON_ARTICULATION_DYNAMIC_GENERAL_N_CLOSED_DYADIC_ENDPOINT_POSITIVE_THICKNESS_PREREQUISITE_MODEL_ID_V2: &str =
    "common_articulation_dynamic_general_n_closed_dyadic_endpoint_positive_thickness_prerequisite_v2";

const GENERAL_N_MIN_BLOCKS_V2: usize = 33;
// Fixed Phase 3H work contract: 19 finite/envelope policy checks, eleven
// checked resource-equation/cap operations, six authenticated theorem
// predicates (four boundary counts plus the two retained proof predicates),
// 25 binding operations (five fixed fields, nineteen scalar fields, and
// finalization), and 77 worst-case replay-policy binding operations: six
// Phase 3H and twelve Phase 3G cap equalities, then the Phase 3F limits digest
// domain tag, all fifty-five scalar hash updates, hash finalization, digest
// equality, and replay-aggregate-cap equality. Delegated Phase 3G proof work
// retains its own separate counters.
const PROMOTION_LIMIT_POLICY_WORK_V2: usize = 19;
const PROMOTION_RESOURCE_WORK_V2: usize = 11;
const PROMOTION_THEOREM_WORK_V2: usize = 6;
const PROMOTION_BINDING_WORK_V2: usize = 25;
const PROMOTION_REPLAY_POLICY_WORK_V2: usize = 77;
pub(crate) const PROMOTION_LOGICAL_WORK_V2: usize = PROMOTION_LIMIT_POLICY_WORK_V2
    + PROMOTION_RESOURCE_WORK_V2
    + PROMOTION_THEOREM_WORK_V2
    + PROMOTION_BINDING_WORK_V2
    + PROMOTION_REPLAY_POLICY_WORK_V2;
pub(crate) const PROMOTION_WORKSPACE_BYTES_V2: usize = 512;

#[path = "closed_dyadic_endpoint_positive_thickness/validation.rs"]
mod validation;

use validation::{
    checkpoint_v2, endpoint_limits_match_v2, preflight_replay_policy_v2, revalidate_coverage_v2,
    validate_endpoint_coverage_v2,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteStopV2 {
    Cancelled,
    DeadlineExceeded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteErrorV2 {
    #[error("the closed-dyadic-endpoint prerequisite exceeds its finite resource envelope")]
    ResourceLimit,
    #[error(
        "the retained Phase 3F proof has no authenticated complete closed-domain boundary coverage"
    )]
    BoundaryCoverageUnavailable,
    #[error("the retained Phase 3G coverage certificate does not replay: {0}")]
    Coverage(CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageErrorV2),
    #[error("the closed-dyadic-endpoint prerequisite does not match the live replay")]
    CertificateBindingMismatch,
    #[error("the closed-dyadic-endpoint prerequisite operation was cancelled")]
    Cancelled,
    #[error("the closed-dyadic-endpoint prerequisite operation deadline elapsed")]
    DeadlineExceeded,
}

/// Finite caps for the O(1) proof promotion and its delegated replay peak.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteLimitsV2
{
    pub max_blocks: usize,
    pub max_retained_coverage_bytes: usize,
    pub max_promotion_logical_work: usize,
    pub max_promotion_workspace_bytes: usize,
    pub max_publication_bytes: usize,
    pub max_aggregate_peak_bytes: usize,
}

/// Consuming proof-to-proof promotion input. No live replay is needed to
/// derive a logical consequence from an already authenticated Phase 3G seal.
pub struct CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteInputV2
{
    pub coverage: CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageCertificateV2,
    pub limits:
        CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteLimitsV2,
}

/// Exact Phase 3G live replay plus the outer Phase 3H resource policy.
pub struct CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteRevalidationInputV2<
    'a,
> {
    pub coverage_replay:
        CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageRevalidationInputV2<'a>,
    pub limits:
        CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteLimitsV2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EndpointCoverageResourcesV2 {
    retained_coverage_bytes: usize,
    delegated_replay_peak_bytes: usize,
    promotion_logical_work: usize,
    promotion_workspace_bytes: usize,
    publication_bytes: usize,
    aggregate_peak_bytes: usize,
}

/// Opaque prerequisite for positive-thickness coverage at both closed dyadic
/// domain boundaries.
///
/// It owns the exact Phase 3G certificate from which it was promoted and
/// deliberately implements neither `Clone`, serde, `Deref`, raw leaf/digest
/// access, nor conversion back into Phase 3G.
///
/// ```compile_fail
/// use ori_collision::CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteV2;
/// fn require_clone<T: Clone>() {}
/// require_clone::<CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteV2>();
/// ```
///
/// ```compile_fail
/// use ori_collision::CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteV2;
/// fn require_serialize<T: serde::Serialize>() {}
/// require_serialize::<CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteV2>();
/// ```
///
/// ```compile_fail
/// use ori_collision::CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteV2;
/// fn require_deserialize<T: serde::de::DeserializeOwned>() {}
/// require_deserialize::<CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteV2>();
/// ```
///
/// ```compile_fail
/// use std::ops::Deref;
/// use ori_collision::CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteV2;
/// fn require_deref<T: Deref>() {}
/// require_deref::<CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteV2>();
/// ```
///
/// ```compile_fail
/// use ori_collision::CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteV2;
/// fn fabricate() -> CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteV2 {
///     CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteV2 {}
/// }
/// ```
///
/// ```compile_fail
/// use ori_collision::CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteV2;
/// fn expose_raw(value: &CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteV2) {
///     let _ = value.boundary_coverage;
///     let _ = value.binding_fingerprint;
/// }
/// ```
///
/// ```compile_fail
/// use ori_collision::{
///     CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteV2,
///     CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageCertificateV2,
/// };
/// fn downgrade(
///     value: CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteV2,
/// ) -> CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageCertificateV2 {
///     value.into()
/// }
/// ```
pub struct CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteV2 {
    coverage: CommonArticulationDynamicGeneralNRelievedSourceOrderCoverageCertificateV2,
    boundary_coverage: ClosedDyadicDomainBoundaryCoverageV2,
    resources: EndpointCoverageResourcesV2,
    limits:
        CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteLimitsV2,
    binding_fingerprint: [u8; 32],
}

impl std::fmt::Debug
    for CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteV2
{
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct(
                "CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteV2",
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

impl CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteV2 {
    #[must_use]
    pub const fn model_id_v2(&self) -> &'static str {
        COMMON_ARTICULATION_DYNAMIC_GENERAL_N_CLOSED_DYADIC_ENDPOINT_POSITIVE_THICKNESS_PREREQUISITE_MODEL_ID_V2
    }

    #[must_use]
    pub const fn actual_block_count_v2(&self) -> usize {
        self.coverage.actual_block_count_v2()
    }

    #[must_use]
    pub const fn material_face_count_v2(&self) -> usize {
        self.coverage.material_face_count_v2()
    }

    #[must_use]
    pub const fn source_order_pair_count_v2(&self) -> usize {
        self.coverage.source_order_pair_count_v2()
    }

    #[must_use]
    pub const fn closed_dyadic_domain_boundary_count_v2(&self) -> usize {
        2
    }

    #[must_use]
    pub const fn both_closed_dyadic_domain_boundaries_covered_by_positive_thickness_v2(
        &self,
    ) -> bool {
        self.boundary_coverage.is_complete_v2()
    }

    #[must_use]
    pub const fn retained_coverage_bytes_v2(&self) -> usize {
        self.resources.retained_coverage_bytes
    }

    #[must_use]
    pub const fn promotion_logical_work_v2(&self) -> usize {
        self.resources.promotion_logical_work
    }

    #[must_use]
    pub const fn delegated_replay_peak_bytes_upper_bound_v2(&self) -> usize {
        self.resources.delegated_replay_peak_bytes
    }

    #[must_use]
    pub const fn publication_bytes_v2(&self) -> usize {
        self.resources.publication_bytes
    }

    #[must_use]
    pub const fn aggregate_peak_bytes_upper_bound_v2(&self) -> usize {
        self.resources.aggregate_peak_bytes
    }

    pub(crate) const fn binding_fingerprint_v2(&self) -> [u8; 32] {
        self.binding_fingerprint
    }

    pub(crate) const fn schedule_binding_fingerprint_v2(&self) -> [u8; 32] {
        self.coverage.schedule_binding_fingerprint_v2()
    }

    pub(crate) const fn graph_binding_fingerprint_v1(&self) -> [u8; 32] {
        self.coverage.graph_binding_fingerprint_v1()
    }

    pub(crate) fn matches_geometry_instance_v2(
        &self,
        geometry: &ori_kinematics::MaterialHingeGraphGeometry,
    ) -> bool {
        self.coverage.matches_geometry_instance_v2(geometry)
    }

    pub(crate) const fn replay_aggregate_peak_cap_v2(&self) -> usize {
        self.limits.max_aggregate_peak_bytes
    }

    pub(crate) const fn replay_limits_match_v2(
        &self,
        limits:
            CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteLimitsV2,
    ) -> bool {
        endpoint_limits_match_v2(self.limits, limits)
    }

    pub fn revalidate_v2(
        &self,
        input: CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteRevalidationInputV2<
            '_,
        >,
    ) -> Result<
        (),
        CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteErrorV2,
    > {
        self.revalidate_with_checkpoint_v2(input, || Ok(()))
    }

    pub fn revalidate_with_checkpoint_v2(
        &self,
        input: CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteRevalidationInputV2<
            '_,
        >,
        mut checkpoint: impl FnMut() -> Result<
            (),
            CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteStopV2,
        >,
    ) -> Result<
        (),
        CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteErrorV2,
    > {
        checkpoint_v2(&mut checkpoint)?;
        let validated = validate_endpoint_coverage_v2(&self.coverage, input.limits)?;
        if !endpoint_limits_match_v2(self.limits, input.limits) {
            return Err(
                CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteErrorV2::CertificateBindingMismatch,
            );
        }
        preflight_replay_policy_v2(&self.coverage, &input.coverage_replay)?;
        revalidate_coverage_v2(&self.coverage, input.coverage_replay, &mut checkpoint)?;
        checkpoint_v2(&mut checkpoint)?;
        if self.boundary_coverage != validated.boundary_coverage
            || self.resources != validated.resources
            || self.binding_fingerprint != validated.binding_fingerprint
        {
            return Err(
                CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteErrorV2::CertificateBindingMismatch,
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

pub fn prove_common_articulation_dynamic_general_n_closed_dyadic_endpoint_positive_thickness_prerequisite_v2(
    input: CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteInputV2,
) -> Result<
    CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteV2,
    CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteErrorV2,
> {
    prove_common_articulation_dynamic_general_n_closed_dyadic_endpoint_positive_thickness_prerequisite_with_checkpoint_v2(
        input,
        || Ok(()),
    )
}

pub fn prove_common_articulation_dynamic_general_n_closed_dyadic_endpoint_positive_thickness_prerequisite_with_checkpoint_v2(
    input: CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteInputV2,
    mut checkpoint: impl FnMut() -> Result<
        (),
        CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteStopV2,
    >,
) -> Result<
    CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteV2,
    CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteErrorV2,
> {
    checkpoint_v2(&mut checkpoint)?;
    let validated = validate_endpoint_coverage_v2(&input.coverage, input.limits)?;
    checkpoint_v2(&mut checkpoint)?;
    Ok(
        CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteV2 {
            coverage: input.coverage,
            boundary_coverage: validated.boundary_coverage,
            resources: validated.resources,
            limits: input.limits,
            binding_fingerprint: validated.binding_fingerprint,
        },
    )
}

#[cfg(test)]
#[path = "closed_dyadic_endpoint_positive_thickness/tests.rs"]
mod tests;
