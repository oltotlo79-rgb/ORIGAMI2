use super::*;

mod binding;
mod evaluate;
mod resources;

pub const CANONICAL_CYCLE_SCHEDULE_CLOSED_DYADIC_BOUNDARY_EVIDENCE_MODEL_ID_V2: &str =
    "canonical_cycle_schedule_closed_dyadic_boundary_evidence_v2";

const CLOSED_DYADIC_BOUNDARY_COUNT_V2: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BoundaryRepresentationV2 {
    Ordinary,
    HalfAngle,
}

impl BoundaryRepresentationV2 {
    pub(super) const fn tag_v2(self) -> u8 {
        match self {
            Self::Ordinary => 0,
            Self::HalfAngle => 1,
        }
    }
}

/// Cooperative stop requested while deriving sealed schedule-boundary evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CycleScheduleClosedDyadicBoundaryStopV2 {
    Cancelled,
    DeadlineExceeded,
}

/// Failure while deriving sealed schedule-representation boundary evidence.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum CycleScheduleClosedDyadicBoundaryErrorV2 {
    #[error(transparent)]
    Prepare(#[from] CycleSchedulePrepareErrorV1),
    #[error("closed dyadic schedule-boundary evidence exceeds its resource limits")]
    ResourceLimit,
    #[error("closed dyadic schedule-boundary evaluation was cancelled")]
    Cancelled,
    #[error("closed dyadic schedule-boundary evaluation deadline elapsed")]
    DeadlineExceeded,
}

/// Advisory, checked resource inventory for one current schedule instance.
///
/// This value is deliberately not accepted by the evidence prover. A schedule
/// with the same semantic fingerprint can have different vector capacities,
/// so the prover always inventories its current schedule again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CycleScheduleClosedDyadicBoundaryResourceBoundV2 {
    pub(super) hinge_count: usize,
    pub(super) schedule_deep_retained_bytes: usize,
    pub(super) scan_logical_work: usize,
    pub(super) logical_work_required: usize,
    pub(super) workspace_peak_bytes: usize,
}

impl CycleScheduleClosedDyadicBoundaryResourceBoundV2 {
    #[must_use]
    pub const fn hinge_count_v2(self) -> usize {
        self.hinge_count
    }

    #[must_use]
    pub const fn schedule_deep_retained_bytes_v2(self) -> usize {
        self.schedule_deep_retained_bytes
    }

    #[must_use]
    pub const fn logical_work_required_v2(self) -> usize {
        self.logical_work_required
    }

    #[must_use]
    pub const fn workspace_peak_bytes_upper_bound_v2(self) -> usize {
        self.workspace_peak_bytes
    }
}

/// Opaque evidence for the two boundaries of a canonical schedule's own
/// normalized closed-dyadic representation domain.
///
/// For an ordinary schedule, the sealed records are direct Clenshaw values at
/// normalized `x = -1` and `x = +1`. For a half-angle schedule, they are
/// outward boxes at the two exact rational `u_domain` endpoints. This value is
/// not a pose, source/target authority, application `t = 0/1` authority,
/// motion proof, layer-transport proof, or mutation capability.
///
/// It intentionally implements neither `Clone`, serde, `Deref`, nor raw angle
/// or interval access.
///
/// ```compile_fail
/// use ori_kinematics::CanonicalCycleScheduleClosedDyadicBoundaryEvidenceV2;
/// fn require_clone<T: Clone>() {}
/// require_clone::<CanonicalCycleScheduleClosedDyadicBoundaryEvidenceV2>();
/// ```
///
/// ```compile_fail
/// use ori_kinematics::CanonicalCycleScheduleClosedDyadicBoundaryEvidenceV2;
/// fn require_serialize<T: serde::Serialize>() {}
/// require_serialize::<CanonicalCycleScheduleClosedDyadicBoundaryEvidenceV2>();
/// ```
///
/// ```compile_fail
/// use ori_kinematics::CanonicalCycleScheduleClosedDyadicBoundaryEvidenceV2;
/// fn require_deserialize<T: for<'de> serde::Deserialize<'de>>() {}
/// require_deserialize::<CanonicalCycleScheduleClosedDyadicBoundaryEvidenceV2>();
/// ```
///
/// ```compile_fail
/// use ori_kinematics::CanonicalCycleScheduleClosedDyadicBoundaryEvidenceV2;
/// fn require_deref<T: std::ops::Deref>() {}
/// require_deref::<CanonicalCycleScheduleClosedDyadicBoundaryEvidenceV2>();
/// ```
///
/// ```compile_fail
/// use ori_kinematics::CanonicalCycleScheduleClosedDyadicBoundaryEvidenceV2;
/// fn fabricate() -> CanonicalCycleScheduleClosedDyadicBoundaryEvidenceV2 {
///     CanonicalCycleScheduleClosedDyadicBoundaryEvidenceV2 {}
/// }
/// ```
///
/// ```compile_fail
/// use ori_kinematics::CanonicalCycleScheduleClosedDyadicBoundaryEvidenceV2;
/// fn expose_raw(value: &CanonicalCycleScheduleClosedDyadicBoundaryEvidenceV2) {
///     let _ = value.lower_boundary_binding_fingerprint;
/// }
/// ```
pub struct CanonicalCycleScheduleClosedDyadicBoundaryEvidenceV2 {
    schedule_binding_fingerprint: [u8; 32],
    graph_binding_fingerprint: [u8; 32],
    lower_boundary_binding_fingerprint: [u8; 32],
    upper_boundary_binding_fingerprint: [u8; 32],
    binding_fingerprint: [u8; 32],
    hinge_count: usize,
    logical_work: usize,
    workspace_peak_bytes: usize,
}

impl std::fmt::Debug for CanonicalCycleScheduleClosedDyadicBoundaryEvidenceV2 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CanonicalCycleScheduleClosedDyadicBoundaryEvidenceV2")
            .field("model", &self.model_id_v2())
            .field(
                "canonical_boundary_count",
                &self.canonical_boundary_count_v2(),
            )
            .field("hinge_count", &self.hinge_count)
            .field("logical_work", &self.logical_work)
            .field("workspace_peak_bytes", &self.workspace_peak_bytes)
            .finish_non_exhaustive()
    }
}

impl CanonicalCycleScheduleClosedDyadicBoundaryEvidenceV2 {
    #[must_use]
    pub const fn model_id_v2(&self) -> &'static str {
        CANONICAL_CYCLE_SCHEDULE_CLOSED_DYADIC_BOUNDARY_EVIDENCE_MODEL_ID_V2
    }

    #[must_use]
    pub const fn canonical_boundary_count_v2(&self) -> usize {
        CLOSED_DYADIC_BOUNDARY_COUNT_V2
    }

    #[must_use]
    pub const fn hinge_count_v2(&self) -> usize {
        self.hinge_count
    }

    #[must_use]
    pub const fn logical_work_v2(&self) -> usize {
        self.logical_work
    }

    #[must_use]
    pub const fn workspace_peak_bytes_upper_bound_v2(&self) -> usize {
        self.workspace_peak_bytes
    }

    #[doc(hidden)]
    #[must_use]
    pub const fn schedule_binding_fingerprint_v2(&self) -> [u8; 32] {
        self.schedule_binding_fingerprint
    }

    #[doc(hidden)]
    #[must_use]
    pub const fn binding_fingerprint_v2(&self) -> [u8; 32] {
        let _sealed_components = (
            self.graph_binding_fingerprint,
            self.lower_boundary_binding_fingerprint,
            self.upper_boundary_binding_fingerprint,
        );
        self.binding_fingerprint
    }
}

impl CanonicalCycleScheduleV1 {
    pub fn checked_closed_dyadic_boundary_resource_bound_v2(
        &self,
        limits: CycleScheduleLimitsV1,
    ) -> Result<
        CycleScheduleClosedDyadicBoundaryResourceBoundV2,
        CycleScheduleClosedDyadicBoundaryErrorV2,
    > {
        self.checked_closed_dyadic_boundary_resource_bound_with_checkpoint_v2(limits, || Ok(()))
    }

    pub fn checked_closed_dyadic_boundary_resource_bound_with_checkpoint_v2(
        &self,
        limits: CycleScheduleLimitsV1,
        mut checkpoint: impl FnMut() -> Result<(), CycleScheduleClosedDyadicBoundaryStopV2>,
    ) -> Result<
        CycleScheduleClosedDyadicBoundaryResourceBoundV2,
        CycleScheduleClosedDyadicBoundaryErrorV2,
    > {
        resources::checked_resource_bound_v2(self, limits, &mut checkpoint)
    }

    pub fn prove_closed_dyadic_boundary_evidence_v2(
        &self,
        limits: CycleScheduleLimitsV1,
        max_logical_work: usize,
        max_workspace_bytes: usize,
    ) -> Result<
        CanonicalCycleScheduleClosedDyadicBoundaryEvidenceV2,
        CycleScheduleClosedDyadicBoundaryErrorV2,
    > {
        self.prove_closed_dyadic_boundary_evidence_with_checkpoint_v2(
            limits,
            max_logical_work,
            max_workspace_bytes,
            || Ok(()),
        )
    }

    pub fn prove_closed_dyadic_boundary_evidence_with_checkpoint_v2(
        &self,
        limits: CycleScheduleLimitsV1,
        max_logical_work: usize,
        max_workspace_bytes: usize,
        mut checkpoint: impl FnMut() -> Result<(), CycleScheduleClosedDyadicBoundaryStopV2>,
    ) -> Result<
        CanonicalCycleScheduleClosedDyadicBoundaryEvidenceV2,
        CycleScheduleClosedDyadicBoundaryErrorV2,
    > {
        resources::checkpoint_v2(&mut checkpoint)?;
        let bound = resources::checked_resource_bound_v2(self, limits, &mut checkpoint)?;
        if max_logical_work == 0
            || max_logical_work == usize::MAX
            || max_logical_work != bound.logical_work_required
            || max_workspace_bytes == 0
            || max_workspace_bytes == usize::MAX
            || max_workspace_bytes != bound.workspace_peak_bytes
        {
            return Err(CycleScheduleClosedDyadicBoundaryErrorV2::ResourceLimit);
        }

        let evaluated = evaluate::evaluate_boundaries_v2(
            self,
            limits,
            bound,
            max_logical_work,
            &mut checkpoint,
        )?;
        resources::checkpoint_v2(&mut checkpoint)?;
        let binding_fingerprint = binding::evidence_binding_fingerprint_v2(
            evaluated.representation,
            self.schedule_fingerprint_v2,
            self.binding_fingerprint,
            evaluated.lower_binding,
            evaluated.upper_binding,
            evaluated.hinge_count,
            limits,
            bound.logical_work_required,
            bound.workspace_peak_bytes,
        )?;
        resources::checkpoint_v2(&mut checkpoint)?;

        Ok(CanonicalCycleScheduleClosedDyadicBoundaryEvidenceV2 {
            schedule_binding_fingerprint: self.schedule_fingerprint_v2,
            graph_binding_fingerprint: self.binding_fingerprint,
            lower_boundary_binding_fingerprint: evaluated.lower_binding,
            upper_boundary_binding_fingerprint: evaluated.upper_binding,
            binding_fingerprint,
            hinge_count: evaluated.hinge_count,
            logical_work: bound.logical_work_required,
            workspace_peak_bytes: bound.workspace_peak_bytes,
        })
    }
}

#[cfg(test)]
#[path = "closed_dyadic_boundary_v2/tests.rs"]
mod tests;
