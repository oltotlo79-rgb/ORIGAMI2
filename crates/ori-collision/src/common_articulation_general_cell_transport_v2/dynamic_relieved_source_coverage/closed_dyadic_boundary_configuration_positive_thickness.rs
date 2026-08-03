//! Schedule-bound positive thickness at both closed-dyadic representation boundaries.
//!
//! This Phase 3I prerequisite joins the exact canonical schedule retained by
//! Phase 3H to sealed kinematics evidence for that same schedule's two
//! representation boundaries. For an ordinary schedule those boundaries are
//! normalized Chebyshev `x = -1` and `x = +1`; for a half-angle schedule they
//! are every canonical entry's exact rational `u_domain` lower and upper
//! endpoints, paired by the common application-parameter `0/1` boundary.
//!
//! That parameter correspondence does not itself establish application
//! `t = 0/1` authority or source/target pose identity. This prerequisite also
//! proves no continuous motion, directional layer-order realization, or layer
//! transport.

use ori_kinematics::{
    CanonicalCycleScheduleClosedDyadicBoundaryEvidenceV2, CanonicalCycleScheduleV1,
    CycleScheduleLimitsV1, MaterialHingeGraphGeometry, MaterialHingeGraphInstanceV1,
};
use thiserror::Error;

use super::{
    CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteErrorV2,
    CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteRevalidationInputV2,
    CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteV2,
};

pub const COMMON_ARTICULATION_DYNAMIC_GENERAL_N_CLOSED_DYADIC_BOUNDARY_CONFIGURATION_POSITIVE_THICKNESS_PREREQUISITE_MODEL_ID_V2: &str =
    "common_articulation_dynamic_general_n_closed_dyadic_boundary_configuration_positive_thickness_prerequisite_v2";

const GENERAL_N_MIN_BLOCKS_V2: usize = 33;
pub(crate) const COMPOSITION_WORKSPACE_BYTES_V2: usize = 512;

#[path = "closed_dyadic_boundary_configuration_positive_thickness/validation.rs"]
mod validation;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteStopV2
{
    Cancelled,
    DeadlineExceeded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteErrorV2
{
    #[error("the schedule-bound boundary prerequisite exceeds its finite resource envelope")]
    ResourceLimit,
    #[error("the canonical schedule boundary configuration is unavailable")]
    BoundaryConfigurationUnavailable,
    #[error("the retained Phase 3H endpoint prerequisite does not replay: {0}")]
    EndpointPositiveThickness(
        CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteErrorV2,
    ),
    #[error("the schedule-bound boundary prerequisite does not match the live replay")]
    CertificateBindingMismatch,
    #[error("the schedule-bound boundary prerequisite operation was cancelled")]
    Cancelled,
    #[error("the schedule-bound boundary prerequisite operation deadline elapsed")]
    DeadlineExceeded,
}

/// Replay-bound outer policy for the schedule join and its two delegated proofs.
///
/// The six count/retained/publication/aggregate `max_*` fields are upper caps:
/// issuance may retain genuine slack, but replay must reproduce each cap
/// exactly. Boundary-evidence logical work and workspace are exact identities.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteLimitsV2
{
    pub max_blocks: usize,
    pub max_hinges: usize,
    pub max_schedule_deep_retained_bytes: usize,
    pub max_boundary_evidence_logical_work: usize,
    pub max_boundary_evidence_workspace_bytes: usize,
    pub max_retained_endpoint_prerequisite_bytes: usize,
    pub max_publication_bytes: usize,
    pub max_aggregate_peak_bytes: usize,
}

/// Consuming issue input. Phase 3H is joined to the exact live schedule and
/// geometry instance before schedule-boundary evidence is derived.
pub struct CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteInputV2<
    'a,
> {
    pub geometry: &'a MaterialHingeGraphGeometry,
    pub schedule: &'a CanonicalCycleScheduleV1,
    pub schedule_limits: CycleScheduleLimitsV1,
    pub endpoint_prerequisite:
        CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteV2,
    pub limits:
        CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteLimitsV2,
}

/// Complete live input for replaying both the retained Phase 3H proof and the
/// canonical schedule's boundary evidence.
pub struct CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteRevalidationInputV2<
    'a,
> {
    pub geometry: &'a MaterialHingeGraphGeometry,
    pub schedule: &'a CanonicalCycleScheduleV1,
    pub schedule_limits: CycleScheduleLimitsV1,
    pub endpoint_replay:
        CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteRevalidationInputV2<'a>,
    pub limits:
        CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteLimitsV2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BoundaryConfigurationResourcesV2 {
    retained_endpoint_prerequisite_bytes: usize,
    schedule_deep_retained_bytes_upper_bound: usize,
    boundary_evidence_bytes: usize,
    boundary_evidence_logical_work: usize,
    boundary_evidence_workspace_bytes: usize,
    delegated_endpoint_replay_peak_bytes: usize,
    composition_workspace_bytes: usize,
    publication_bytes: usize,
    aggregate_peak_bytes: usize,
}

/// Opaque proof that Phase 3H's positive-thickness boundary coverage and the
/// two authenticated boundary configurations belong to one exact canonical
/// schedule and one live material-geometry instance.
///
/// It deliberately implements neither `Clone`, serde, `Deref`, raw evidence
/// access, nor conversion back into Phase 3H.
///
/// ```compile_fail
/// use ori_collision::CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteV2;
/// fn require_clone<T: Clone>() {}
/// require_clone::<CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteV2>();
/// ```
///
/// ```compile_fail
/// use ori_collision::CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteV2;
/// fn require_serialize<T: serde::Serialize>() {}
/// require_serialize::<CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteV2>();
/// ```
///
/// ```compile_fail
/// use ori_collision::CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteV2;
/// fn require_deserialize<T: serde::de::DeserializeOwned>() {}
/// require_deserialize::<CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteV2>();
/// ```
///
/// ```compile_fail
/// use std::ops::Deref;
/// use ori_collision::CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteV2;
/// fn require_deref<T: Deref>() {}
/// require_deref::<CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteV2>();
/// ```
///
/// ```compile_fail
/// use ori_collision::CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteV2;
/// fn fabricate() -> CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteV2 {
///     CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteV2 {}
/// }
/// ```
///
/// ```compile_fail
/// use ori_collision::CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteV2;
/// fn expose_raw(value: &CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteV2) {
///     let _ = value.boundary_evidence;
///     let _ = value.binding_fingerprint;
///     let _ = value.issuer_geometry;
/// }
/// ```
///
/// ```compile_fail
/// use ori_collision::{
///     CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteV2,
///     CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteV2,
/// };
/// fn downgrade(
///     value: CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteV2,
/// ) -> CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteV2 {
///     value.into()
/// }
/// ```
pub struct CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteV2
{
    issuer_geometry: MaterialHingeGraphInstanceV1,
    endpoint_prerequisite:
        CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteV2,
    boundary_evidence: CanonicalCycleScheduleClosedDyadicBoundaryEvidenceV2,
    schedule_limits: CycleScheduleLimitsV1,
    resources: BoundaryConfigurationResourcesV2,
    limits:
        CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteLimitsV2,
    binding_fingerprint: [u8; 32],
}

impl std::fmt::Debug
    for CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteV2
{
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct(
                "CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteV2",
            )
            .field("model", &self.model_id_v2())
            .field("actual_block_count", &self.actual_block_count_v2())
            .field("hinge_count", &self.hinge_count_v2())
            .field(
                "canonical_boundary_configuration_count",
                &self.closed_dyadic_boundary_configuration_count_v2(),
            )
            .field("publication_bytes", &self.publication_bytes_v2())
            .field(
                "aggregate_peak_bytes",
                &self.aggregate_peak_bytes_upper_bound_v2(),
            )
            .finish_non_exhaustive()
    }
}

impl CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteV2
{
    #[must_use]
    pub const fn model_id_v2(&self) -> &'static str {
        COMMON_ARTICULATION_DYNAMIC_GENERAL_N_CLOSED_DYADIC_BOUNDARY_CONFIGURATION_POSITIVE_THICKNESS_PREREQUISITE_MODEL_ID_V2
    }

    #[must_use]
    pub const fn actual_block_count_v2(&self) -> usize {
        self.endpoint_prerequisite.actual_block_count_v2()
    }

    #[must_use]
    pub const fn material_face_count_v2(&self) -> usize {
        self.endpoint_prerequisite.material_face_count_v2()
    }

    #[must_use]
    pub const fn source_order_pair_count_v2(&self) -> usize {
        self.endpoint_prerequisite.source_order_pair_count_v2()
    }

    #[must_use]
    pub const fn hinge_count_v2(&self) -> usize {
        self.boundary_evidence.hinge_count_v2()
    }

    #[must_use]
    pub const fn closed_dyadic_boundary_configuration_count_v2(&self) -> usize {
        self.boundary_evidence.canonical_boundary_count_v2()
    }

    #[must_use]
    pub const fn both_closed_dyadic_boundary_configurations_have_positive_thickness_v2(
        &self,
    ) -> bool {
        self.endpoint_prerequisite
            .both_closed_dyadic_domain_boundaries_covered_by_positive_thickness_v2()
            && self.boundary_evidence.canonical_boundary_count_v2() == 2
    }

    #[must_use]
    pub const fn retained_endpoint_prerequisite_bytes_v2(&self) -> usize {
        self.resources.retained_endpoint_prerequisite_bytes
    }

    #[must_use]
    pub const fn schedule_deep_retained_bytes_upper_bound_v2(&self) -> usize {
        self.resources.schedule_deep_retained_bytes_upper_bound
    }

    #[must_use]
    pub const fn boundary_evidence_logical_work_v2(&self) -> usize {
        self.resources.boundary_evidence_logical_work
    }

    #[must_use]
    pub const fn boundary_evidence_workspace_bytes_upper_bound_v2(&self) -> usize {
        self.resources.boundary_evidence_workspace_bytes
    }

    #[must_use]
    pub const fn delegated_endpoint_replay_peak_bytes_upper_bound_v2(&self) -> usize {
        self.resources.delegated_endpoint_replay_peak_bytes
    }

    #[must_use]
    pub const fn publication_bytes_v2(&self) -> usize {
        self.resources.publication_bytes
    }

    #[must_use]
    pub const fn aggregate_peak_bytes_upper_bound_v2(&self) -> usize {
        self.resources.aggregate_peak_bytes
    }

    pub(super) const fn issuer_geometry_instance_v2(&self) -> &MaterialHingeGraphInstanceV1 {
        &self.issuer_geometry
    }

    pub(super) const fn schedule_binding_fingerprint_internal_v2(&self) -> [u8; 32] {
        self.boundary_evidence.schedule_binding_fingerprint_v2()
    }

    pub(super) const fn graph_binding_fingerprint_internal_v2(&self) -> [u8; 32] {
        self.endpoint_prerequisite.graph_binding_fingerprint_v1()
    }

    pub(super) const fn closed_boundary_binding_fingerprint_internal_v2(&self) -> [u8; 32] {
        self.boundary_evidence.binding_fingerprint_v2()
    }

    pub(super) const fn closed_boundary_evidence_internal_v2(
        &self,
    ) -> &CanonicalCycleScheduleClosedDyadicBoundaryEvidenceV2 {
        &self.boundary_evidence
    }

    pub(super) const fn schedule_limits_internal_v2(&self) -> CycleScheduleLimitsV1 {
        self.schedule_limits
    }

    pub(super) const fn schedule_deep_retained_bytes_cap_internal_v2(&self) -> usize {
        self.limits.max_schedule_deep_retained_bytes
    }

    pub(super) const fn block_count_cap_internal_v2(&self) -> usize {
        self.limits.max_blocks
    }

    pub(super) const fn hinge_count_cap_internal_v2(&self) -> usize {
        self.limits.max_hinges
    }

    pub(super) const fn replay_aggregate_peak_cap_internal_v2(&self) -> usize {
        self.limits.max_aggregate_peak_bytes
    }

    pub(super) const fn binding_fingerprint_internal_v2(&self) -> [u8; 32] {
        self.binding_fingerprint
    }

    pub(super) fn cheap_replay_tuple_matches_internal_v2(
        &self,
        input: &CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteRevalidationInputV2<'_>,
    ) -> bool {
        let replay_geometry = input.endpoint_replay.coverage_replay.live.geometry;
        let replay_schedule = input.endpoint_replay.coverage_replay.live.parent_schedule;
        validation::limits_match_v2(self.limits, input.limits)
            && self.schedule_limits == input.schedule_limits
            && self
                .endpoint_prerequisite
                .replay_limits_match_v2(input.endpoint_replay.limits)
            && self.issuer_geometry.matches(input.geometry)
            && self.issuer_geometry.matches(replay_geometry)
            && self
                .endpoint_prerequisite
                .matches_geometry_instance_v2(input.geometry)
            && self
                .endpoint_prerequisite
                .matches_geometry_instance_v2(replay_geometry)
            && self.endpoint_prerequisite.schedule_binding_fingerprint_v2()
                == self.boundary_evidence.schedule_binding_fingerprint_v2()
            && self.boundary_evidence.schedule_binding_fingerprint_v2()
                == input.schedule.certificate_binding_fingerprint_v2()
            && self.boundary_evidence.schedule_binding_fingerprint_v2()
                == replay_schedule.certificate_binding_fingerprint_v2()
            && self.endpoint_prerequisite.graph_binding_fingerprint_v1()
                == input.schedule.graph_binding_fingerprint_v1()
            && self.endpoint_prerequisite.graph_binding_fingerprint_v1()
                == replay_schedule.graph_binding_fingerprint_v1()
    }

    pub fn revalidate_v2(
        &self,
        input: CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteRevalidationInputV2<'_>,
    ) -> Result<
        (),
        CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteErrorV2,
    > {
        self.revalidate_with_checkpoint_v2(input, || Ok(()))
    }

    pub fn revalidate_with_checkpoint_v2(
        &self,
        input: CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteRevalidationInputV2<'_>,
        mut checkpoint: impl FnMut() -> Result<
            (),
            CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteStopV2,
        >,
    ) -> Result<
        (),
        CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteErrorV2,
    > {
        validation::revalidate_v2(self, input, &mut checkpoint)
    }

    #[must_use]
    pub const fn authorizes_continuous_motion(&self) -> bool { false }
    #[must_use]
    pub const fn authorizes_collision_clearance(&self) -> bool { false }
    #[must_use]
    pub const fn authorizes_layer_transport(&self) -> bool { false }
    #[must_use]
    pub const fn authorizes_project_mutation(&self) -> bool { false }
    #[must_use]
    pub const fn authorizes_apply(&self) -> bool { false }
    #[must_use]
    pub const fn authorizes_viewer(&self) -> bool { false }
    #[must_use]
    pub const fn authorizes_export(&self) -> bool { false }
}

pub fn prove_common_articulation_dynamic_general_n_closed_dyadic_boundary_configuration_positive_thickness_prerequisite_v2(
    input: CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteInputV2<'_>,
) -> Result<
    CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteV2,
    CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteErrorV2,
>{
    prove_common_articulation_dynamic_general_n_closed_dyadic_boundary_configuration_positive_thickness_prerequisite_with_checkpoint_v2(
        input,
        || Ok(()),
    )
}

pub fn prove_common_articulation_dynamic_general_n_closed_dyadic_boundary_configuration_positive_thickness_prerequisite_with_checkpoint_v2(
    input: CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteInputV2<'_>,
    mut checkpoint: impl FnMut() -> Result<
        (),
        CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteStopV2,
    >,
) -> Result<
    CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteV2,
    CommonArticulationDynamicGeneralNClosedDyadicBoundaryConfigurationPositiveThicknessPrerequisiteErrorV2,
>{
    validation::issue_v2(input, &mut checkpoint)
}

#[cfg(test)]
#[path = "closed_dyadic_boundary_configuration_positive_thickness/tests.rs"]
mod tests;
