use super::*;

mod clone_resources;
mod validation;

use clone_resources::{
    checked_half_angle_nested_retained_with_checkpoint_v2, clone_half_angle_entry_v2,
};
use validation::{
    audit_covers_block_edge_with_checkpoint_v2, block_edges_are_unique_with_checkpoint_v2,
    block_hinges_are_from_source_with_checkpoint_v2, checked_binding_work_v2,
    edge_is_in_block_poll_only_v2, edge_is_in_block_preflight_v2,
    face_sets_equal_with_checkpoint_v2, slice_contains_face_with_checkpoint_v2,
};
/// Allocation and work ceilings for one owned V2 schedule restriction.
///
/// The source schedule is borrowed and excluded. The retained ceiling covers
/// the returned schedule shell, every physical `Vec` capacity, and all cloned
/// `BigInt` payload reachable through exact rational entries. The peak adds the
/// fixed streaming hash state used while binding the completed restriction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CycleScheduleRestrictionWorkspaceLimitsV2 {
    pub(crate) max_work: usize,
    pub(crate) max_restricted_schedule_retained_bytes: usize,
    pub(crate) max_restriction_peak_bytes: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CycleScheduleRestrictionWorkspaceResourcesV2 {
    pub(crate) charged_work: usize,
    pub(crate) charged_restricted_schedule_retained_upper_bound_bytes: usize,
    pub(crate) charged_restriction_peak_upper_bound_bytes: usize,
}

#[derive(Debug)]
pub(crate) struct WorkspaceBoundedCycleScheduleRestrictionV2 {
    pub(crate) schedule: CanonicalCycleScheduleV1,
    pub(crate) resources: CycleScheduleRestrictionWorkspaceResourcesV2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CycleScheduleRestrictionWorkspaceErrorV2 {
    InvalidInput,
    ResourceLimit,
    Cancelled,
    DeadlineExceeded,
}

#[derive(Debug)]
struct RestrictionMeterV2 {
    limits: CycleScheduleRestrictionWorkspaceLimitsV2,
    work: usize,
    retained_bytes: usize,
    peak_bytes: usize,
}

impl RestrictionMeterV2 {
    fn new(
        limits: CycleScheduleRestrictionWorkspaceLimitsV2,
    ) -> Result<Self, CycleScheduleRestrictionWorkspaceErrorV2> {
        if [
            limits.max_work,
            limits.max_restricted_schedule_retained_bytes,
            limits.max_restriction_peak_bytes,
        ]
        .contains(&usize::MAX)
            || limits.max_work == 0
            || limits.max_restricted_schedule_retained_bytes == 0
            || limits.max_restriction_peak_bytes == 0
        {
            return Err(CycleScheduleRestrictionWorkspaceErrorV2::ResourceLimit);
        }
        Ok(Self {
            limits,
            work: 0,
            retained_bytes: 0,
            peak_bytes: 0,
        })
    }

    fn charge_work(
        &mut self,
        amount: usize,
    ) -> Result<(), CycleScheduleRestrictionWorkspaceErrorV2> {
        self.work = self
            .work
            .checked_add(amount)
            .filter(|work| *work <= self.limits.max_work)
            .ok_or(CycleScheduleRestrictionWorkspaceErrorV2::ResourceLimit)?;
        Ok(())
    }

    fn observe_retained(
        &mut self,
        retained_bytes: usize,
    ) -> Result<(), CycleScheduleRestrictionWorkspaceErrorV2> {
        let peak_bytes = retained_bytes
            .checked_add(std::mem::size_of::<Sha256>())
            .ok_or(CycleScheduleRestrictionWorkspaceErrorV2::ResourceLimit)?;
        if retained_bytes > self.limits.max_restricted_schedule_retained_bytes
            || peak_bytes > self.limits.max_restriction_peak_bytes
        {
            return Err(CycleScheduleRestrictionWorkspaceErrorV2::ResourceLimit);
        }
        self.retained_bytes = self.retained_bytes.max(retained_bytes);
        self.peak_bytes = self.peak_bytes.max(peak_bytes);
        Ok(())
    }

    fn resources(self) -> CycleScheduleRestrictionWorkspaceResourcesV2 {
        CycleScheduleRestrictionWorkspaceResourcesV2 {
            charged_work: self.work,
            charged_restricted_schedule_retained_upper_bound_bytes: self.retained_bytes,
            charged_restriction_peak_upper_bound_bytes: self.peak_bytes,
        }
    }
}

fn checkpoint_v2(
    checkpoint: &mut impl FnMut() -> Result<(), CycleScheduleRestrictionStopV1>,
) -> Result<(), CycleScheduleRestrictionWorkspaceErrorV2> {
    checkpoint().map_err(|stop| match stop {
        CycleScheduleRestrictionStopV1::Cancelled => {
            CycleScheduleRestrictionWorkspaceErrorV2::Cancelled
        }
        CycleScheduleRestrictionStopV1::DeadlineExceeded => {
            CycleScheduleRestrictionWorkspaceErrorV2::DeadlineExceeded
        }
    })
}

fn checked_schedule_shell_and_outer_bytes_v2(
    entries_capacity: usize,
    half_angle_entries_capacity: usize,
) -> Option<usize> {
    std::mem::size_of::<CanonicalCycleScheduleV1>()
        .checked_add(std::mem::size_of::<Entry>().checked_mul(entries_capacity)?)?
        .checked_add(
            std::mem::size_of::<PreparedHalfAngleRationalEntryV1>()
                .checked_mul(half_angle_entries_capacity)?,
        )
}

impl CanonicalCycleScheduleV1 {
    /// Allocation-free exact-bits comparison against a live pose angle list.
    /// Cooperative polls are interleaved with every scalar Horner step; no
    /// `CanonicalHingeAngles` candidate is materialized.
    pub(crate) fn matches_hinge_angles_at_parameter_with_checkpoint_v2<Stop>(
        &self,
        parameter: f64,
        expected: &CanonicalHingeAngles,
        mut checkpoint: impl FnMut() -> Result<(), Stop>,
    ) -> Result<bool, Stop> {
        checkpoint()?;
        if !parameter.is_finite()
            || expected.as_slice().len() != self.entries.len().max(self.half_angle_entries.len())
        {
            return Ok(false);
        }
        if !self.half_angle_entries.is_empty() {
            if !(0.0..=1.0).contains(&parameter) {
                return Ok(false);
            }
            for (entry, expected) in self.half_angle_entries.iter().zip(expected.as_slice()) {
                checkpoint()?;
                checkpoint()?;
                let Some(lower) = entry.u_domain[0].to_f64() else {
                    return Ok(false);
                };
                checkpoint()?;
                let Some(upper) = entry.u_domain[1].to_f64() else {
                    return Ok(false);
                };
                let u = lower + (upper - lower) * parameter;
                let mut numerator = 0.0_f64;
                for coefficient in entry.numerator_power_coefficients.iter().rev() {
                    checkpoint()?;
                    let Some(coefficient) = coefficient.to_f64() else {
                        return Ok(false);
                    };
                    numerator = numerator * u + coefficient;
                }
                let mut denominator = 0.0_f64;
                for coefficient in entry.denominator_power_coefficients.iter().rev() {
                    checkpoint()?;
                    let Some(coefficient) = coefficient.to_f64() else {
                        return Ok(false);
                    };
                    denominator = denominator * u + coefficient;
                }
                let Some(angle) = deterministic_half_angle_ratio_degrees_v1(numerator, denominator)
                else {
                    return Ok(false);
                };
                if entry.edge() != expected.edge()
                    || angle.to_bits() != expected.angle_degrees().to_bits()
                {
                    return Ok(false);
                }
            }
            checkpoint()?;
            return Ok(true);
        }
        if parameter < self.domain[0] || parameter > self.domain[1] {
            return Ok(false);
        }
        let x =
            (2.0 * parameter - self.domain[0] - self.domain[1]) / (self.domain[1] - self.domain[0]);
        for (entry, expected) in self.entries.iter().zip(expected.as_slice()) {
            checkpoint()?;
            let mut b1 = 0.0;
            let mut b2 = 0.0;
            for coefficient in entry.coefficients.iter().rev() {
                checkpoint()?;
                let b0 = 2.0 * x * b1 - b2 + coefficient;
                b2 = b1;
                b1 = b0;
            }
            let angle = entry.initial + b1 - x * b2;
            if !angle.is_finite()
                || entry.edge != expected.edge()
                || angle.to_bits() != expected.angle_degrees().to_bits()
            {
                return Ok(false);
            }
        }
        checkpoint()?;
        Ok(true)
    }

    /// Rebinds and owns one carrier subset without the legacy restriction
    /// `HashSet`. Outer vectors reserve fallibly; exact `BigInt` clones are
    /// admitted by a representation-aware payload scan before cloning. The
    /// reported byte values are conservative charged upper bounds: the greater
    /// of preflight material and every observed physical capacity. Passing the
    /// full source graph produces an owned parent copy.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn restrict_to_edge_block_with_workspace_and_checkpoint_v2(
        &self,
        source_geometry: &MaterialHingeGraphGeometry,
        source_audit: &MaterialHingeGraphAudit,
        block_geometry: &MaterialHingeGraphGeometry,
        block_audit: &MaterialHingeGraphAudit,
        block_fixed_face: FaceId,
        limits: CycleScheduleRestrictionWorkspaceLimitsV2,
        mut checkpoint: impl FnMut() -> Result<(), CycleScheduleRestrictionStopV1>,
    ) -> Result<WorkspaceBoundedCycleScheduleRestrictionV2, CycleScheduleRestrictionWorkspaceErrorV2>
    {
        checkpoint_v2(&mut checkpoint)?;
        let mut meter = RestrictionMeterV2::new(limits)?;
        meter.charge_work(
            checked_binding_work_v2(source_geometry, source_audit)
                .ok_or(CycleScheduleRestrictionWorkspaceErrorV2::ResourceLimit)?,
        )?;
        meter.charge_work(
            checked_binding_work_v2(block_geometry, block_audit)
                .ok_or(CycleScheduleRestrictionWorkspaceErrorV2::ResourceLimit)?,
        )?;
        let source_binding = binding_fingerprint_with_checkpoint_v1(
            source_geometry,
            source_audit,
            self.fixed_face,
            &mut checkpoint,
        )
        .map_err(|error| match error {
            CycleScheduleRestrictionErrorV1::Prepare(
                CycleSchedulePrepareErrorV1::ResourceLimit,
            ) => CycleScheduleRestrictionWorkspaceErrorV2::ResourceLimit,
            CycleScheduleRestrictionErrorV1::Prepare(_) => {
                CycleScheduleRestrictionWorkspaceErrorV2::InvalidInput
            }
            CycleScheduleRestrictionErrorV1::Cancelled => {
                CycleScheduleRestrictionWorkspaceErrorV2::Cancelled
            }
            CycleScheduleRestrictionErrorV1::DeadlineExceeded => {
                CycleScheduleRestrictionWorkspaceErrorV2::DeadlineExceeded
            }
        })?;
        if self.binding_fingerprint != source_binding || block_geometry.face_ids().is_empty() {
            return Err(CycleScheduleRestrictionWorkspaceErrorV2::InvalidInput);
        }
        for faces in [
            source_geometry.face_ids(),
            source_audit.faces(),
            block_geometry.face_ids(),
            block_audit.faces(),
        ] {
            if !slice_contains_face_with_checkpoint_v2(
                faces,
                block_fixed_face,
                &mut meter,
                &mut checkpoint,
            )? {
                return Err(CycleScheduleRestrictionWorkspaceErrorV2::InvalidInput);
            }
        }
        if !face_sets_equal_with_checkpoint_v2(
            block_geometry.face_ids(),
            block_audit.faces(),
            &mut meter,
            &mut checkpoint,
        )? {
            return Err(CycleScheduleRestrictionWorkspaceErrorV2::InvalidInput);
        }
        if block_audit
            .spanning_hinges()
            .len()
            .checked_add(block_audit.closure_hinges().len())
            .is_none_or(|count| count != block_geometry.hinges().len())
        {
            return Err(CycleScheduleRestrictionWorkspaceErrorV2::InvalidInput);
        }
        for face in block_geometry.face_ids() {
            if !slice_contains_face_with_checkpoint_v2(
                source_geometry.face_ids(),
                *face,
                &mut meter,
                &mut checkpoint,
            )? {
                return Err(CycleScheduleRestrictionWorkspaceErrorV2::InvalidInput);
            }
        }
        for block_hinge in block_geometry.hinges() {
            if !audit_covers_block_edge_with_checkpoint_v2(
                block_audit,
                block_hinge.edge(),
                &mut meter,
                &mut checkpoint,
            )? {
                return Err(CycleScheduleRestrictionWorkspaceErrorV2::InvalidInput);
            }
        }
        if !block_hinges_are_from_source_with_checkpoint_v2(
            block_geometry.hinges(),
            source_geometry.hinges(),
            &mut meter,
            &mut checkpoint,
        )? || !block_edges_are_unique_with_checkpoint_v2(
            block_geometry.hinges(),
            &mut meter,
            &mut checkpoint,
        )? {
            return Err(CycleScheduleRestrictionWorkspaceErrorV2::InvalidInput);
        }
        if block_geometry.hinges().is_empty() {
            return Err(CycleScheduleRestrictionWorkspaceErrorV2::InvalidInput);
        }

        let mut ordinary_count = 0usize;
        let mut half_angle_count = 0usize;
        let mut preflight_retained = std::mem::size_of::<CanonicalCycleScheduleV1>();
        // Two domain polls, one final poll, and one pre-entry poll are charged
        // before any output allocation. Entry/rational fingerprint work is
        // charged below as each selected source entry is classified.
        meter.charge_work(4)?;
        for entry in &self.entries {
            checkpoint_v2(&mut checkpoint)?;
            if edge_is_in_block_preflight_v2(
                entry.edge,
                block_geometry,
                2,
                &mut meter,
                &mut checkpoint,
            )? {
                ordinary_count = ordinary_count
                    .checked_add(1)
                    .ok_or(CycleScheduleRestrictionWorkspaceErrorV2::ResourceLimit)?;
                meter.charge_work(
                    entry
                        .coefficients
                        .len()
                        .checked_mul(2)
                        .and_then(|work| work.checked_add(1))
                        .ok_or(CycleScheduleRestrictionWorkspaceErrorV2::ResourceLimit)?,
                )?;
                preflight_retained = preflight_retained
                    .checked_add(std::mem::size_of::<Entry>())
                    .and_then(|bytes| {
                        std::mem::size_of::<f64>()
                            .checked_mul(entry.coefficients.len())
                            .and_then(|nested| bytes.checked_add(nested))
                    })
                    .ok_or(CycleScheduleRestrictionWorkspaceErrorV2::ResourceLimit)?;
            }
        }
        for entry in &self.half_angle_entries {
            checkpoint_v2(&mut checkpoint)?;
            if edge_is_in_block_preflight_v2(
                entry.edge(),
                block_geometry,
                2,
                &mut meter,
                &mut checkpoint,
            )? {
                half_angle_count = half_angle_count
                    .checked_add(1)
                    .ok_or(CycleScheduleRestrictionWorkspaceErrorV2::ResourceLimit)?;
                meter.charge_work(
                    entry
                        .u_domain
                        .len()
                        .checked_add(entry.numerator_power_coefficients.len())
                        .and_then(|work| {
                            work.checked_add(entry.denominator_power_coefficients.len())
                        })
                        .and_then(|work| work.checked_add(1))
                        .ok_or(CycleScheduleRestrictionWorkspaceErrorV2::ResourceLimit)?,
                )?;
                meter.charge_work(
                    entry
                        .u_domain
                        .len()
                        .checked_add(entry.numerator_power_coefficients.len())
                        .and_then(|work| {
                            work.checked_add(entry.denominator_power_coefficients.len())
                        })
                        .and_then(|work| {
                            work.checked_add(entry.numerator_certificate.coefficients.len())
                        })
                        .and_then(|work| {
                            work.checked_add(entry.denominator_certificate.coefficients.len())
                        })
                        .ok_or(CycleScheduleRestrictionWorkspaceErrorV2::ResourceLimit)?,
                )?;
                let entry_nested = checked_half_angle_nested_retained_with_checkpoint_v2(
                    entry,
                    &mut meter,
                    &mut checkpoint,
                )?;
                preflight_retained = preflight_retained
                    .checked_add(std::mem::size_of::<PreparedHalfAngleRationalEntryV1>())
                    .and_then(|bytes| bytes.checked_add(entry_nested))
                    .ok_or(CycleScheduleRestrictionWorkspaceErrorV2::ResourceLimit)?;
            }
        }
        if ordinary_count
            .checked_add(half_angle_count)
            .ok_or(CycleScheduleRestrictionWorkspaceErrorV2::ResourceLimit)?
            != block_geometry.hinges().len()
        {
            return Err(CycleScheduleRestrictionWorkspaceErrorV2::InvalidInput);
        }
        meter.observe_retained(preflight_retained)?;

        let mut entries = Vec::new();
        entries
            .try_reserve_exact(ordinary_count)
            .map_err(|_| CycleScheduleRestrictionWorkspaceErrorV2::ResourceLimit)?;
        // The first outer reservation may exceed its logical request. Observe
        // that physical capacity together with the second outer vector's
        // logical request before the second allocation is attempted.
        meter.observe_retained(
            checked_schedule_shell_and_outer_bytes_v2(entries.capacity(), half_angle_count)
                .ok_or(CycleScheduleRestrictionWorkspaceErrorV2::ResourceLimit)?,
        )?;
        let mut half_angle_entries = Vec::new();
        half_angle_entries
            .try_reserve_exact(half_angle_count)
            .map_err(|_| CycleScheduleRestrictionWorkspaceErrorV2::ResourceLimit)?;
        let base_outer = checked_schedule_shell_and_outer_bytes_v2(
            entries.capacity(),
            half_angle_entries.capacity(),
        )
        .ok_or(CycleScheduleRestrictionWorkspaceErrorV2::ResourceLimit)?;
        meter.observe_retained(base_outer)?;
        let mut nested_bytes = 0usize;
        for entry in &self.entries {
            checkpoint_v2(&mut checkpoint)?;
            if !edge_is_in_block_poll_only_v2(entry.edge, block_geometry, &mut checkpoint)? {
                continue;
            }
            let mut coefficients = Vec::new();
            let logical_allocation_bytes = std::mem::size_of::<f64>()
                .checked_mul(entry.coefficients.len())
                .ok_or(CycleScheduleRestrictionWorkspaceErrorV2::ResourceLimit)?;
            meter.observe_retained(
                base_outer
                    .checked_add(nested_bytes)
                    .and_then(|bytes| bytes.checked_add(logical_allocation_bytes))
                    .ok_or(CycleScheduleRestrictionWorkspaceErrorV2::ResourceLimit)?,
            )?;
            coefficients
                .try_reserve_exact(entry.coefficients.len())
                .map_err(|_| CycleScheduleRestrictionWorkspaceErrorV2::ResourceLimit)?;
            let allocation_bytes = std::mem::size_of::<f64>()
                .checked_mul(coefficients.capacity())
                .ok_or(CycleScheduleRestrictionWorkspaceErrorV2::ResourceLimit)?;
            meter.observe_retained(
                base_outer
                    .checked_add(nested_bytes)
                    .and_then(|bytes| bytes.checked_add(allocation_bytes))
                    .ok_or(CycleScheduleRestrictionWorkspaceErrorV2::ResourceLimit)?,
            )?;
            for coefficient in &entry.coefficients {
                checkpoint_v2(&mut checkpoint)?;
                coefficients.push(*coefficient);
            }
            nested_bytes = nested_bytes
                .checked_add(allocation_bytes)
                .ok_or(CycleScheduleRestrictionWorkspaceErrorV2::ResourceLimit)?;
            entries.push(Entry {
                edge: entry.edge,
                initial: entry.initial,
                coefficients,
                derivative_bound: entry.derivative_bound,
            });
        }
        for entry in &self.half_angle_entries {
            checkpoint_v2(&mut checkpoint)?;
            if !edge_is_in_block_poll_only_v2(entry.edge(), block_geometry, &mut checkpoint)? {
                continue;
            }
            let (entry, entry_nested_bytes) = clone_half_angle_entry_v2(
                entry,
                base_outer
                    .checked_add(nested_bytes)
                    .ok_or(CycleScheduleRestrictionWorkspaceErrorV2::ResourceLimit)?,
                &mut meter,
                &mut checkpoint,
            )?;
            nested_bytes = nested_bytes
                .checked_add(entry_nested_bytes)
                .ok_or(CycleScheduleRestrictionWorkspaceErrorV2::ResourceLimit)?;
            half_angle_entries.push(entry);
        }
        let schedule = Self {
            binding_fingerprint: binding_fingerprint_with_checkpoint_v1(
                block_geometry,
                block_audit,
                block_fixed_face,
                &mut checkpoint,
            )
            .map_err(|error| match error {
                CycleScheduleRestrictionErrorV1::Prepare(
                    CycleSchedulePrepareErrorV1::ResourceLimit,
                ) => CycleScheduleRestrictionWorkspaceErrorV2::ResourceLimit,
                CycleScheduleRestrictionErrorV1::Prepare(_) => {
                    CycleScheduleRestrictionWorkspaceErrorV2::InvalidInput
                }
                CycleScheduleRestrictionErrorV1::Cancelled => {
                    CycleScheduleRestrictionWorkspaceErrorV2::Cancelled
                }
                CycleScheduleRestrictionErrorV1::DeadlineExceeded => {
                    CycleScheduleRestrictionWorkspaceErrorV2::DeadlineExceeded
                }
            })?,
            schedule_fingerprint_v2: schedule_fingerprint_v2_with_checkpoint_v1(
                self.domain,
                &entries,
                &half_angle_entries,
                &mut checkpoint,
            )
            .map_err(|error| match error {
                CycleScheduleRestrictionErrorV1::Prepare(
                    CycleSchedulePrepareErrorV1::ResourceLimit,
                ) => CycleScheduleRestrictionWorkspaceErrorV2::ResourceLimit,
                CycleScheduleRestrictionErrorV1::Prepare(_) => {
                    CycleScheduleRestrictionWorkspaceErrorV2::InvalidInput
                }
                CycleScheduleRestrictionErrorV1::Cancelled => {
                    CycleScheduleRestrictionWorkspaceErrorV2::Cancelled
                }
                CycleScheduleRestrictionErrorV1::DeadlineExceeded => {
                    CycleScheduleRestrictionWorkspaceErrorV2::DeadlineExceeded
                }
            })?,
            fixed_face: block_fixed_face,
            domain: self.domain,
            entries,
            half_angle_entries,
        };
        let retained_bytes = base_outer
            .checked_add(nested_bytes)
            .ok_or(CycleScheduleRestrictionWorkspaceErrorV2::ResourceLimit)?;
        meter.observe_retained(retained_bytes)?;
        checkpoint_v2(&mut checkpoint)?;
        Ok(WorkspaceBoundedCycleScheduleRestrictionV2 {
            schedule,
            resources: meter.resources(),
        })
    }
}
