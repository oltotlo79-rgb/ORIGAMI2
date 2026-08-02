use super::*;

mod exact_interval;
mod request;
mod workspace_bound;

use exact_interval::CycleScheduleExactVectorMeterV2;

/// Conservative allocation inventory for allocations created or retained by
/// one V2 dyadic schedule evaluation.
///
/// `big_rational_payload_bytes` covers the
/// dynamic `BigInt` limbs behind every simultaneously live exact rational;
/// `peak_bytes` additionally includes the outer angle-box allocation and the
/// `BigRational` objects stored in temporary Bernstein vectors. The borrowed
/// [`CanonicalCycleScheduleV1`] and its pre-existing backing allocations are
/// excluded and remain the caller's retained-input responsibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CycleScheduleDyadicWorkspaceBoundV2 {
    schedule_fingerprint_v2: [u8; 32],
    limits: CycleScheduleLimitsV1,
    max_depth: u32,
    angle_box_bytes: usize,
    big_rational_payload_bytes: usize,
    exact_object_bytes: usize,
    exact_nonvector_object_bytes: usize,
    peak_bytes: usize,
}

impl CycleScheduleDyadicWorkspaceBoundV2 {
    /// Checked upper bound for the retained dyadic angle-box vector.
    #[must_use]
    pub const fn angle_box_bytes(self) -> usize {
        self.angle_box_bytes
    }

    /// Checked upper bound for simultaneously live `BigInt` payloads.
    #[must_use]
    pub const fn big_rational_payload_bytes(self) -> usize {
        self.big_rational_payload_bytes
    }

    /// Checked upper bound for simultaneously live exact object shells.
    #[must_use]
    pub const fn exact_object_bytes(self) -> usize {
        self.exact_object_bytes
    }

    /// Exact object shells that are not already represented by vector capacity.
    #[must_use]
    pub const fn exact_nonvector_object_bytes(self) -> usize {
        self.exact_nonvector_object_bytes
    }

    /// Checked peak additionally owned by schedule evaluation. The borrowed
    /// schedule's existing retained allocation is excluded.
    #[must_use]
    pub const fn peak_bytes(self) -> usize {
        self.peak_bytes
    }
}

/// One workspace-metered dyadic schedule evaluation.
///
/// The exact-arithmetic temporaries have already been dropped. The returned
/// angle-box vector is the only heap allocation retained by this value.
#[derive(Debug)]
pub(crate) struct CycleScheduleDyadicEvaluationV2 {
    angle_boxes: Vec<(EdgeId, OutwardIntervalV1)>,
    angle_box_capacity_bytes: usize,
    exact_vector_capacity_peak_bytes: usize,
}

impl CycleScheduleDyadicEvaluationV2 {
    /// Canonically ordered outward hinge-angle boxes.
    #[must_use]
    pub(crate) fn angle_boxes(&self) -> &[(EdgeId, OutwardIntervalV1)] {
        &self.angle_boxes
    }

    /// Physical capacity bytes retained by [`Self::angle_boxes`].
    #[must_use]
    pub(crate) const fn angle_box_capacity_bytes(&self) -> usize {
        self.angle_box_capacity_bytes
    }

    /// Greatest physically observed exact-vector capacity during evaluation.
    #[must_use]
    pub(crate) const fn exact_vector_capacity_peak_bytes(&self) -> usize {
        self.exact_vector_capacity_peak_bytes
    }

    /// Consumes the metered wrapper and returns its retained angle boxes.
    #[must_use]
    pub(crate) fn into_angle_boxes(self) -> Vec<(EdgeId, OutwardIntervalV1)> {
        self.angle_boxes
    }
}

/// Failure from a workspace-metered dyadic schedule evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CycleScheduleDyadicEvaluationErrorV2 {
    /// The underlying schedule or dyadic request was rejected.
    Prepare(CycleSchedulePrepareErrorV1),
    /// A checked or physically observed allocation exceeded the supplied cap.
    WorkspaceLimit,
    /// Cooperative cancellation was requested.
    Cancelled,
    /// The caller's deadline checkpoint fired.
    DeadlineExceeded,
}

/// Cooperative stop requested during workspace-metered schedule evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CycleScheduleDyadicEvaluationStopV2 {
    /// Stop as soon as the current poll is reached.
    Cancelled,
    /// Stop because the caller's deadline has elapsed.
    DeadlineExceeded,
}

/// Allocation-free witness for the deliberately narrow ordinary affine
/// profile accepted by the V2 exact parallel-cut closure theorem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ExactParallelCutScheduleProfileV2 {
    schedule_fingerprint_v2: [u8; 32],
    moving_count: usize,
    charged_work: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExactParallelCutProfileErrorV2<Stop> {
    Stop(Stop),
    ResourceLimit,
}

fn exact_parallel_cut_profile_poll_and_charge_v2<Stop>(
    work: &mut usize,
    max_work: usize,
    checkpoint: &mut impl FnMut() -> Result<(), Stop>,
) -> Result<(), ExactParallelCutProfileErrorV2<Stop>> {
    checkpoint().map_err(ExactParallelCutProfileErrorV2::Stop)?;
    *work = work
        .checked_add(1)
        .filter(|value| *value <= max_work)
        .ok_or(ExactParallelCutProfileErrorV2::ResourceLimit)?;
    Ok(())
}

impl From<CycleSchedulePrepareErrorV1> for CycleScheduleDyadicEvaluationErrorV2 {
    fn from(error: CycleSchedulePrepareErrorV1) -> Self {
        Self::Prepare(error)
    }
}

impl CanonicalCycleScheduleV1 {
    /// V2 evaluator with fallible outer allocation and physical-capacity
    /// reporting for every transient exact-rational vector it creates.
    #[cfg(test)]
    pub(crate) fn evaluate_angle_box_dyadic_with_workspace_v2(
        &self,
        depth: u32,
        index: u64,
        limits: CycleScheduleLimitsV1,
        prevalidated_bound: CycleScheduleDyadicWorkspaceBoundV2,
        max_schedule_workspace_bytes: usize,
    ) -> Result<CycleScheduleDyadicEvaluationV2, CycleScheduleDyadicEvaluationErrorV2> {
        self.evaluate_angle_box_dyadic_with_workspace_impl_v2(
            depth,
            index,
            limits,
            prevalidated_bound,
            max_schedule_workspace_bytes,
            &mut || Ok(()),
        )
    }

    /// Workspace-metered private dyadic evaluation with cooperative stops. It
    /// polls at entry, within every schedule-entry and coefficient loop,
    /// through exact Bernstein preparation, and immediately before return.
    pub(crate) fn evaluate_angle_box_dyadic_with_workspace_and_checkpoint_v2(
        &self,
        depth: u32,
        index: u64,
        limits: CycleScheduleLimitsV1,
        prevalidated_bound: CycleScheduleDyadicWorkspaceBoundV2,
        max_schedule_workspace_bytes: usize,
        mut checkpoint: impl FnMut() -> Result<(), CycleScheduleDyadicEvaluationStopV2>,
    ) -> Result<CycleScheduleDyadicEvaluationV2, CycleScheduleDyadicEvaluationErrorV2> {
        self.evaluate_angle_box_dyadic_with_workspace_impl_v2(
            depth,
            index,
            limits,
            prevalidated_bound,
            max_schedule_workspace_bytes,
            &mut || {
                checkpoint().map_err(|stop| match stop {
                    CycleScheduleDyadicEvaluationStopV2::Cancelled => {
                        CycleScheduleDyadicEvaluationErrorV2::Cancelled
                    }
                    CycleScheduleDyadicEvaluationStopV2::DeadlineExceeded => {
                        CycleScheduleDyadicEvaluationErrorV2::DeadlineExceeded
                    }
                })
            },
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn evaluate_angle_box_dyadic_with_workspace_impl_v2(
        &self,
        depth: u32,
        index: u64,
        limits: CycleScheduleLimitsV1,
        prevalidated_bound: CycleScheduleDyadicWorkspaceBoundV2,
        max_schedule_workspace_bytes: usize,
        checkpoint: &mut impl FnMut() -> Result<(), CycleScheduleDyadicEvaluationErrorV2>,
    ) -> Result<CycleScheduleDyadicEvaluationV2, CycleScheduleDyadicEvaluationErrorV2> {
        self.validate_dyadic_workspace_request_impl_v2(
            depth,
            limits,
            prevalidated_bound,
            max_schedule_workspace_bytes,
            checkpoint,
        )?;
        let leaf_count = 1u64 << depth;
        if index >= leaf_count {
            return Err(CycleSchedulePrepareErrorV1::InvalidInput.into());
        }
        let carrier_len = if self.half_angle_entries.is_empty() {
            self.entries.len()
        } else {
            self.half_angle_entries.len()
        };
        let fixed_exact_bytes = prevalidated_bound
            .big_rational_payload_bytes
            .checked_add(prevalidated_bound.exact_nonvector_object_bytes)
            .ok_or(CycleScheduleDyadicEvaluationErrorV2::WorkspaceLimit)?;
        if prevalidated_bound
            .angle_box_bytes
            .checked_add(fixed_exact_bytes)
            .is_none_or(|bytes| bytes > max_schedule_workspace_bytes)
        {
            return Err(CycleScheduleDyadicEvaluationErrorV2::WorkspaceLimit);
        }
        let mut angle_boxes = Vec::new();
        angle_boxes
            .try_reserve_exact(carrier_len)
            .map_err(|_| CycleScheduleDyadicEvaluationErrorV2::WorkspaceLimit)?;
        checkpoint()?;
        let angle_box_capacity_bytes = std::mem::size_of::<(EdgeId, OutwardIntervalV1)>()
            .checked_mul(angle_boxes.capacity())
            .ok_or(CycleScheduleDyadicEvaluationErrorV2::WorkspaceLimit)?;
        if angle_box_capacity_bytes
            .checked_add(fixed_exact_bytes)
            .is_none_or(|bytes| bytes > max_schedule_workspace_bytes)
        {
            return Err(CycleScheduleDyadicEvaluationErrorV2::WorkspaceLimit);
        }

        if self.half_angle_entries.is_empty() {
            let x = ordinary_dyadic_chebyshev_interval_v2(depth, index)?;
            for entry in &self.entries {
                checkpoint()?;
                let mut constant_zero = true;
                for coefficient in &entry.coefficients {
                    checkpoint()?;
                    constant_zero &= *coefficient == 0.0;
                }
                if constant_zero {
                    let angle = OutwardIntervalV1::new(entry.initial, entry.initial)
                        .map_err(|_| CycleSchedulePrepareErrorV1::InvalidInput)?;
                    angle_boxes.push((entry.edge, angle));
                    continue;
                }
                let zero = OutwardIntervalV1::new(0.0, 0.0)
                    .map_err(|_| CycleSchedulePrepareErrorV1::InvalidInput)?;
                let two = OutwardIntervalV1::from_rounded(2.0)
                    .map_err(|_| CycleSchedulePrepareErrorV1::InvalidInput)?;
                let mut b1 = zero;
                let mut b2 = zero;
                for coefficient in entry.coefficients.iter().rev() {
                    checkpoint()?;
                    let coefficient = OutwardIntervalV1::from_rounded(*coefficient)
                        .map_err(|_| CycleSchedulePrepareErrorV1::InvalidInput)?;
                    let b0 = two
                        .mul(x)
                        .and_then(|value| value.mul(b1))
                        .and_then(|value| value.sub(b2))
                        .and_then(|value| value.add(coefficient))
                        .map_err(|_| CycleSchedulePrepareErrorV1::ResourceLimit)?;
                    b2 = b1;
                    b1 = b0;
                }
                let initial = OutwardIntervalV1::from_rounded(entry.initial)
                    .map_err(|_| CycleSchedulePrepareErrorV1::InvalidInput)?;
                let angle = initial
                    .add(b1)
                    .and_then(|value| value.sub(x.mul(b2)?))
                    .map_err(|_| CycleSchedulePrepareErrorV1::ResourceLimit)?;
                if angle.work() > limits.max_work {
                    return Err(CycleSchedulePrepareErrorV1::ResourceLimit.into());
                }
                if angle.lower() < 0.0 || angle.upper() > 180.0 {
                    return Err(CycleSchedulePrepareErrorV1::AngleRange.into());
                }
                angle_boxes.push((entry.edge, angle));
            }
            checkpoint()?;
            return Ok(CycleScheduleDyadicEvaluationV2 {
                angle_boxes,
                angle_box_capacity_bytes,
                exact_vector_capacity_peak_bytes: 0,
            });
        }

        let max_exact_vector_capacity_bytes = max_schedule_workspace_bytes
            .checked_sub(angle_box_capacity_bytes)
            .and_then(|bytes| bytes.checked_sub(fixed_exact_bytes))
            .ok_or(CycleScheduleDyadicEvaluationErrorV2::WorkspaceLimit)?;
        let mut meter = CycleScheduleExactVectorMeterV2::new(max_exact_vector_capacity_bytes);
        for entry in &self.half_angle_entries {
            checkpoint()?;
            angle_boxes.push((
                entry.edge(),
                entry.angle_enclosure_dyadic_with_workspace_v2(
                    depth,
                    index,
                    limits.max_coefficient_bits,
                    limits.max_degree,
                    limits.max_work,
                    &mut meter,
                    checkpoint,
                )?,
            ));
        }
        checkpoint()?;
        Ok(CycleScheduleDyadicEvaluationV2 {
            angle_boxes,
            angle_box_capacity_bytes,
            exact_vector_capacity_peak_bytes: meter.peak_bytes,
        })
    }
}
