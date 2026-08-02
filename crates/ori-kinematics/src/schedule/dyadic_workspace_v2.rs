use super::*;

/// Conservative allocation inventory for one V2 dyadic schedule evaluation.
///
/// This stays crate-private until a higher-level closure wrapper has fixed its
/// public compatibility surface. `big_rational_payload_bytes` covers the
/// dynamic `BigInt` limbs behind every simultaneously live exact rational;
/// `peak_bytes` additionally includes the outer angle-box allocation and the
/// `BigRational` objects stored in temporary Bernstein vectors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CycleScheduleDyadicWorkspaceBoundV2 {
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
    pub(crate) const fn big_rational_payload_bytes(self) -> usize {
        self.big_rational_payload_bytes
    }

    pub(crate) const fn exact_object_bytes(self) -> usize {
        self.exact_object_bytes
    }

    pub(crate) const fn exact_nonvector_object_bytes(self) -> usize {
        self.exact_nonvector_object_bytes
    }

    pub(crate) const fn peak_bytes(self) -> usize {
        self.peak_bytes
    }
}

#[derive(Debug)]
pub(crate) struct CycleScheduleDyadicEvaluationV2 {
    pub(crate) angle_boxes: Vec<(EdgeId, OutwardIntervalV1)>,
    pub(crate) angle_box_capacity_bytes: usize,
    pub(crate) exact_vector_capacity_peak_bytes: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CycleScheduleDyadicEvaluationErrorV2 {
    Prepare(CycleSchedulePrepareErrorV1),
    WorkspaceLimit,
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

#[derive(Debug, Default)]
struct CycleScheduleExactVectorMeterV2 {
    peak_bytes: usize,
    max_bytes: usize,
}

impl CycleScheduleExactVectorMeterV2 {
    fn new(max_bytes: usize) -> Self {
        Self {
            peak_bytes: 0,
            max_bytes,
        }
    }

    fn observe(&mut self, live_bytes: usize) -> Result<(), CycleScheduleDyadicEvaluationErrorV2> {
        if live_bytes > self.max_bytes {
            return Err(CycleScheduleDyadicEvaluationErrorV2::WorkspaceLimit);
        }
        self.peak_bytes = self.peak_bytes.max(live_bytes);
        Ok(())
    }
}

fn try_exact_workspace_vec_v2(
    capacity: usize,
    base_live_bytes: usize,
    meter: &mut CycleScheduleExactVectorMeterV2,
) -> Result<(Vec<BigRational>, usize), CycleScheduleDyadicEvaluationErrorV2> {
    let logical_bytes = std::mem::size_of::<BigRational>()
        .checked_mul(capacity)
        .and_then(|bytes| bytes.checked_add(base_live_bytes))
        .ok_or(CycleScheduleDyadicEvaluationErrorV2::WorkspaceLimit)?;
    meter.observe(logical_bytes)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(capacity)
        .map_err(|_| CycleScheduleDyadicEvaluationErrorV2::WorkspaceLimit)?;
    let allocation_bytes = std::mem::size_of::<BigRational>()
        .checked_mul(values.capacity())
        .ok_or(CycleScheduleDyadicEvaluationErrorV2::WorkspaceLimit)?;
    meter.observe(
        base_live_bytes
            .checked_add(allocation_bytes)
            .ok_or(CycleScheduleDyadicEvaluationErrorV2::WorkspaceLimit)?,
    )?;
    Ok((values, allocation_bytes))
}

fn affine_reparameterize_power_with_workspace_v2(
    power: &[BigRational],
    domain: &[BigRational; 2],
    max_coefficient_bits: u32,
    max_work: usize,
    base_live_bytes: usize,
    meter: &mut CycleScheduleExactVectorMeterV2,
) -> Result<(Vec<BigRational>, usize), CycleScheduleDyadicEvaluationErrorV2> {
    if power
        .len()
        .checked_mul(power.len())
        .is_none_or(|work| work > max_work)
    {
        return Err(CycleSchedulePrepareErrorV1::ResourceLimit.into());
    }
    let a = &domain[0];
    let width = &domain[1] - a;
    let (mut result, allocation_bytes) =
        try_exact_workspace_vec_v2(power.len(), base_live_bytes, meter)?;
    for _ in 0..power.len() {
        result.push(BigRational::zero());
    }
    for (degree, coefficient) in power.iter().enumerate() {
        for (k, output) in result.iter_mut().enumerate().take(degree + 1) {
            let weight =
                checked_binomial_v1(degree, k).ok_or(CycleSchedulePrepareErrorV1::ResourceLimit)?;
            let a_exponent = i32::try_from(degree - k)
                .map_err(|_| CycleSchedulePrepareErrorV1::ResourceLimit)?;
            let width_exponent =
                i32::try_from(k).map_err(|_| CycleSchedulePrepareErrorV1::ResourceLimit)?;
            *output +=
                coefficient * BigInt::from(weight) * a.pow(a_exponent) * width.pow(width_exponent);
        }
    }
    validate_exact_bits(&result, max_coefficient_bits)?;
    Ok((result, allocation_bytes))
}

#[allow(clippy::too_many_arguments)]
fn prepare_exact_signed_bernstein_certificate_with_workspace_v2(
    power: Vec<BigRational>,
    power_allocation_bytes: usize,
    max_degree: usize,
    max_coefficient_bits: u32,
    max_work: usize,
    allow_endpoint_zero: bool,
    external_live_bytes: usize,
    meter: &mut CycleScheduleExactVectorMeterV2,
) -> Result<(PoleFreeBernsteinCertificateV1, usize), CycleScheduleDyadicEvaluationErrorV2> {
    if power.is_empty()
        || power.len() > max_degree.saturating_add(1)
        || power
            .len()
            .checked_mul(power.len())
            .is_none_or(|work| work > max_work)
    {
        return Err(CycleSchedulePrepareErrorV1::ResourceLimit.into());
    }
    meter.observe(
        external_live_bytes
            .checked_add(power_allocation_bytes)
            .ok_or(CycleScheduleDyadicEvaluationErrorV2::WorkspaceLimit)?,
    )?;
    validate_exact_bits(&power, max_coefficient_bits)?;
    let degree = power.len() - 1;
    let coefficient_count = degree
        .checked_add(1)
        .ok_or(CycleSchedulePrepareErrorV1::ResourceLimit)?;
    let power_live_bytes = external_live_bytes
        .checked_add(power_allocation_bytes)
        .ok_or(CycleScheduleDyadicEvaluationErrorV2::WorkspaceLimit)?;
    let (mut coefficients, coefficient_allocation_bytes) =
        try_exact_workspace_vec_v2(coefficient_count, power_live_bytes, meter)?;
    for i in 0..=degree {
        let mut value = BigRational::zero();
        for (k, coefficient) in power.iter().enumerate().take(i + 1) {
            let numerator =
                checked_binomial_v1(i, k).ok_or(CycleSchedulePrepareErrorV1::ResourceLimit)?;
            let denominator =
                checked_binomial_v1(degree, k).ok_or(CycleSchedulePrepareErrorV1::ResourceLimit)?;
            value +=
                coefficient * BigRational::new(BigInt::from(numerator), BigInt::from(denominator));
        }
        coefficients.push(value);
    }
    validate_exact_bits(&coefficients, max_coefficient_bits)?;
    let strictly_positive = coefficients.iter().all(|value| value.is_positive());
    let strictly_negative = coefficients.iter().all(|value| value.is_negative());
    let endpoint_zero = allow_endpoint_zero
        && coefficients.iter().enumerate().all(|(index, value)| {
            value.is_positive()
                || (value.is_zero() && (index == 0 || index + 1 == coefficients.len()))
        });
    if !strictly_positive && !strictly_negative && !endpoint_zero {
        return Err(CycleSchedulePrepareErrorV1::InvalidInput.into());
    }
    Ok((
        PoleFreeBernsteinCertificateV1 {
            degree,
            positive: strictly_positive || endpoint_zero,
            coefficients,
        },
        coefficient_allocation_bytes,
    ))
}

impl PreparedHalfAngleRationalEntryV1 {
    fn angle_enclosure_dyadic_with_workspace_v2(
        &self,
        depth: u32,
        index: u64,
        max_coefficient_bits: u32,
        max_degree: usize,
        max_work: usize,
        meter: &mut CycleScheduleExactVectorMeterV2,
    ) -> Result<OutwardIntervalV1, CycleScheduleDyadicEvaluationErrorV2> {
        if depth >= 64 || index >= (1u64 << depth) {
            return Err(CycleSchedulePrepareErrorV1::InvalidInput.into());
        }
        if self.numerator_power_coefficients.iter().all(Zero::is_zero) {
            return OutwardIntervalV1::from_rounded(0.0)
                .map_err(|_| CycleSchedulePrepareErrorV1::InvalidInput.into());
        }
        let denominator = BigInt::from(1u64 << depth);
        let width = &self.u_domain[1] - &self.u_domain[0];
        let lower =
            &self.u_domain[0] + &width * BigRational::new(BigInt::from(index), denominator.clone());
        let upper =
            &self.u_domain[0] + width * BigRational::new(BigInt::from(index + 1), denominator);
        let domain = [lower, upper];

        let (numerator_power, numerator_power_bytes) =
            affine_reparameterize_power_with_workspace_v2(
                &self.numerator_power_coefficients,
                &domain,
                max_coefficient_bits,
                max_work,
                0,
                meter,
            )?;
        let (numerator, numerator_bytes) =
            prepare_exact_signed_bernstein_certificate_with_workspace_v2(
                numerator_power,
                numerator_power_bytes,
                max_degree,
                max_coefficient_bits,
                max_work,
                true,
                0,
                meter,
            )?;
        let (denominator_power, denominator_power_bytes) =
            affine_reparameterize_power_with_workspace_v2(
                &self.denominator_power_coefficients,
                &domain,
                max_coefficient_bits,
                max_work,
                numerator_bytes,
                meter,
            )?;
        let (denominator, denominator_bytes) =
            prepare_exact_signed_bernstein_certificate_with_workspace_v2(
                denominator_power,
                denominator_power_bytes,
                max_degree,
                max_coefficient_bits,
                max_work,
                true,
                numerator_bytes,
                meter,
            )?;
        meter.observe(
            numerator_bytes
                .checked_add(denominator_bytes)
                .ok_or(CycleScheduleDyadicEvaluationErrorV2::WorkspaceLimit)?,
        )?;
        if numerator
            .coefficients
            .iter()
            .zip(&denominator.coefficients)
            .any(|(numerator, denominator)| numerator.is_zero() && denominator.is_zero())
        {
            return Err(CycleSchedulePrepareErrorV1::InvalidInput.into());
        }
        evaluate_half_angle_rational_degrees_interval_v1(&numerator, &denominator, max_work)
            .map_err(Into::into)
    }
}

impl CanonicalCycleScheduleV1 {
    /// Classifies the allocation-free ordinary profile admitted by the V2
    /// exact parallel-cut closure recognizer. Moving hinges must all carry one
    /// bit-identical affine `initial + c1*x` profile whose exact-real endpoint
    /// range lies strictly inside 0..180 degrees. Every other hinge must use
    /// the recognized zero encoding (`initial == c0 == 0`, no higher term).
    /// The caller edge slice is in canonical order.
    pub(crate) fn classify_exact_parallel_cut_profile_with_checkpoint_v2<Stop>(
        &self,
        canonical_edges: &[EdgeId],
        max_work: usize,
        mut checkpoint: impl FnMut() -> Result<(), Stop>,
    ) -> Result<
        (Option<ExactParallelCutScheduleProfileV2>, usize),
        ExactParallelCutProfileErrorV2<Stop>,
    > {
        let mut work = 0usize;
        exact_parallel_cut_profile_poll_and_charge_v2(&mut work, max_work, &mut checkpoint)?;
        if !self.half_angle_entries.is_empty() || self.entries.len() != canonical_edges.len() {
            return Ok((None, work));
        }
        let mut reference: Option<&Entry> = None;
        let mut moving_count = 0usize;
        for (entry, expected_edge) in self.entries.iter().zip(canonical_edges) {
            exact_parallel_cut_profile_poll_and_charge_v2(&mut work, max_work, &mut checkpoint)?;
            if entry.edge != *expected_edge || entry.coefficients.is_empty() {
                return Ok((None, work));
            }
            let mut constant = true;
            for coefficient in entry.coefficients.iter().skip(1) {
                exact_parallel_cut_profile_poll_and_charge_v2(
                    &mut work,
                    max_work,
                    &mut checkpoint,
                )?;
                constant &= *coefficient == 0.0;
            }
            if constant {
                if entry.initial != 0.0 || entry.coefficients[0] != 0.0 {
                    return Ok((None, work));
                }
                continue;
            }
            // The theorem deliberately admits only affine schedules. A
            // correctly-rounded endpoint strictly inside the representable
            // bounds implies the exact binary-rational sum is inside them too.
            if entry.coefficients.len() != 2 || entry.coefficients[0] != 0.0 {
                return Ok((None, work));
            }
            let slope = entry.coefficients[1];
            let lower = entry.initial - slope.abs();
            let upper = entry.initial + slope.abs();
            if !lower.is_finite() || !upper.is_finite() || lower <= 0.0 || upper >= 180.0 {
                return Ok((None, work));
            }
            if let Some(reference) = reference {
                if entry.initial.to_bits() != reference.initial.to_bits()
                    || entry.coefficients.len() != reference.coefficients.len()
                {
                    return Ok((None, work));
                }
                for (actual, expected) in entry.coefficients.iter().zip(&reference.coefficients) {
                    exact_parallel_cut_profile_poll_and_charge_v2(
                        &mut work,
                        max_work,
                        &mut checkpoint,
                    )?;
                    if actual.to_bits() != expected.to_bits() {
                        return Ok((None, work));
                    }
                }
            } else {
                reference = Some(entry);
            }
            moving_count = moving_count.saturating_add(1);
        }
        exact_parallel_cut_profile_poll_and_charge_v2(&mut work, max_work, &mut checkpoint)?;
        Ok((
            reference.map(|_| ExactParallelCutScheduleProfileV2 {
                schedule_fingerprint_v2: self.schedule_fingerprint_v2,
                moving_count,
                charged_work: work,
            }),
            work,
        ))
    }

    pub(crate) fn exact_parallel_cut_position_is_moving_v2(
        &self,
        profile: ExactParallelCutScheduleProfileV2,
        position: usize,
    ) -> Option<bool> {
        if profile.schedule_fingerprint_v2 != self.schedule_fingerprint_v2
            || profile.moving_count == 0
        {
            return None;
        }
        let entry = self.entries.get(position)?;
        Some(
            entry
                .coefficients
                .get(1)
                .is_some_and(|coefficient| *coefficient != 0.0),
        )
    }

    pub(crate) const fn exact_parallel_cut_profile_charged_work_v2(
        profile: ExactParallelCutScheduleProfileV2,
    ) -> usize {
        profile.charged_work
    }

    /// V2 evaluator with fallible outer allocation and physical-capacity
    /// reporting for every transient exact-rational vector it creates.
    pub(crate) fn evaluate_angle_box_dyadic_with_workspace_v2(
        &self,
        depth: u32,
        index: u64,
        limits: CycleScheduleLimitsV1,
        prevalidated_bound: CycleScheduleDyadicWorkspaceBoundV2,
        max_schedule_workspace_bytes: usize,
    ) -> Result<CycleScheduleDyadicEvaluationV2, CycleScheduleDyadicEvaluationErrorV2> {
        // The opaque bound is issued by the allocation-free structural scan
        // once at closure entry. Avoid rescanning all retained coefficients on
        // every adaptive leaf.
        if depth >= 64
            || depth > prevalidated_bound.max_depth
            || prevalidated_bound.limits != limits
            || prevalidated_bound.schedule_fingerprint_v2 != self.schedule_fingerprint_v2
        {
            return Err(CycleSchedulePrepareErrorV1::InvalidInput.into());
        }
        let leaf_count = 1u64 << depth;
        if index >= leaf_count {
            return Err(CycleSchedulePrepareErrorV1::InvalidInput.into());
        }
        let carrier_len = if self.half_angle_entries.is_empty() {
            self.entries.len()
        } else {
            self.half_angle_entries.len()
        };
        let logical_angle_bytes = std::mem::size_of::<(EdgeId, OutwardIntervalV1)>()
            .checked_mul(carrier_len)
            .ok_or(CycleScheduleDyadicEvaluationErrorV2::WorkspaceLimit)?;
        if logical_angle_bytes != prevalidated_bound.angle_box_bytes
            || prevalidated_bound
                .angle_box_bytes
                .checked_add(prevalidated_bound.big_rational_payload_bytes)
                .and_then(|bytes| bytes.checked_add(prevalidated_bound.exact_object_bytes))
                != Some(prevalidated_bound.peak_bytes)
        {
            return Err(CycleSchedulePrepareErrorV1::InvalidInput.into());
        }
        let fixed_exact_bytes = prevalidated_bound
            .big_rational_payload_bytes
            .checked_add(prevalidated_bound.exact_nonvector_object_bytes)
            .ok_or(CycleScheduleDyadicEvaluationErrorV2::WorkspaceLimit)?;
        if logical_angle_bytes
            .checked_add(fixed_exact_bytes)
            .is_none_or(|bytes| bytes > max_schedule_workspace_bytes)
        {
            return Err(CycleScheduleDyadicEvaluationErrorV2::WorkspaceLimit);
        }
        let mut angle_boxes = Vec::new();
        angle_boxes
            .try_reserve_exact(carrier_len)
            .map_err(|_| CycleScheduleDyadicEvaluationErrorV2::WorkspaceLimit)?;
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
            let scale = leaf_count as f64;
            let x = OutwardIntervalV1::new(
                -1.0 + 2.0 * index as f64 / scale,
                -1.0 + 2.0 * (index + 1) as f64 / scale,
            )
            .map_err(|_| CycleSchedulePrepareErrorV1::InvalidInput)?;
            for entry in &self.entries {
                if entry
                    .coefficients
                    .iter()
                    .all(|coefficient| *coefficient == 0.0)
                {
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
                if angle.work() > limits.max_work || angle.lower() < 0.0 || angle.upper() > 180.0 {
                    return Err(CycleSchedulePrepareErrorV1::ResourceLimit.into());
                }
                angle_boxes.push((entry.edge, angle));
            }
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
            angle_boxes.push((
                entry.edge(),
                entry.angle_enclosure_dyadic_with_workspace_v2(
                    depth,
                    index,
                    limits.max_coefficient_bits,
                    limits.max_degree,
                    limits.max_work,
                    &mut meter,
                )?,
            ));
        }
        Ok(CycleScheduleDyadicEvaluationV2 {
            angle_boxes,
            angle_box_capacity_bytes,
            exact_vector_capacity_peak_bytes: meter.peak_bytes,
        })
    }

    /// Checked, representation-aware heap upper bound for one dyadic V2
    /// evaluation. This is deliberately computed before exact arithmetic is
    /// entered: `BigInt` owns its limbs internally and therefore cannot expose
    /// a caller-controlled `try_reserve` hook.
    ///
    /// For a half-angle entry, no more than three degree-sized rational
    /// vectors (one retained numerator certificate, one affine power vector,
    /// and one certificate under construction) are live together. A fourth
    /// vector plus fixed expression temporaries is charged as allocator and
    /// evaluation slack. Every rational and standalone `BigInt` temporary is
    /// charged at a bit bound that permits every admitted work item to grow by
    /// one complete affine term before the existing exact-bit validation
    /// rejects it. This is intentionally conservative.
    pub(crate) fn checked_dyadic_workspace_upper_bound_v2(
        &self,
        max_depth: u32,
        limits: CycleScheduleLimitsV1,
    ) -> Option<CycleScheduleDyadicWorkspaceBoundV2> {
        if max_depth >= 64
            || limits.max_hinges == 0
            || limits.max_hinges == usize::MAX
            || limits.max_degree == usize::MAX
            || limits.max_work == 0
            || limits.max_work == usize::MAX
            || limits.max_coefficient_bits == u32::MAX
        {
            return None;
        }
        let hinge_count = if self.half_angle_entries.is_empty() {
            self.entries.len()
        } else {
            self.half_angle_entries.len()
        };
        if hinge_count == 0 || hinge_count > limits.max_hinges {
            return None;
        }
        let degree_slots = limits.max_degree.checked_add(1)?;
        if self.half_angle_entries.is_empty() {
            // The V1 binary64 evaluator does not itself consult max_degree.
            // Make the V2 structural ceiling effective before its output Vec
            // is allocated or any coefficient work is entered.
            if self.entries.iter().any(|entry| {
                entry.coefficients.is_empty()
                    || entry.coefficients.len() > degree_slots
                    || entry.coefficients.len() > limits.max_work
            }) {
                return None;
            }
        } else {
            // A schedule may have been prepared under a different ceiling.
            // Scan every retained exact source before doing arithmetic so a
            // smaller submitted V2 policy cannot undercharge a large stored
            // numerator, denominator, domain, or certificate.
            for entry in &self.half_angle_entries {
                let vectors = [
                    entry.numerator_power_coefficients.as_slice(),
                    entry.denominator_power_coefficients.as_slice(),
                    entry.numerator_certificate.coefficients.as_slice(),
                    entry.denominator_certificate.coefficients.as_slice(),
                ];
                if vectors.iter().any(|values| {
                    values.is_empty()
                        || values.len() > degree_slots
                        || values
                            .len()
                            .checked_mul(values.len())
                            .is_none_or(|work| work > limits.max_work)
                }) {
                    return None;
                }
                if entry
                    .u_domain
                    .iter()
                    .chain(vectors.into_iter().flatten())
                    .any(|value| {
                        value.numer().bits() > u64::from(limits.max_coefficient_bits)
                            || value.denom().bits() > u64::from(limits.max_coefficient_bits)
                    })
                {
                    return None;
                }
            }
        }
        let angle_box_bytes =
            std::mem::size_of::<(EdgeId, OutwardIntervalV1)>().checked_mul(hinge_count)?;
        if self.half_angle_entries.is_empty() {
            return Some(CycleScheduleDyadicWorkspaceBoundV2 {
                schedule_fingerprint_v2: self.schedule_fingerprint_v2,
                limits,
                max_depth,
                angle_box_bytes,
                big_rational_payload_bytes: 0,
                exact_object_bytes: 0,
                exact_nonvector_object_bytes: 0,
                peak_bytes: angle_box_bytes,
            });
        }

        // Exact dyadic domains inherit stored coefficient bits, add at most 64
        // dyadic-index bits, and pass through two rational additions. The
        // multiplier covers numerator/denominator cross-products and signs.
        let source_bits = usize::try_from(limits.max_coefficient_bits)
            .ok()?
            .checked_add(64)?
            .checked_add(128)?;
        let affine_factors = degree_slots.checked_mul(2)?.checked_add(3)?;
        let one_term_bits = source_bits.checked_mul(affine_factors)?.checked_add(256)?;
        let transient_bits = one_term_bits.checked_mul(limits.max_work.checked_add(1)?)?;
        let digit_bits = usize::try_from(usize::BITS).ok()?;
        let digits = transient_bits
            .checked_add(digit_bits.checked_sub(1)?)?
            .checked_div(digit_bits)?
            .max(1);
        // The existing retained-byte audit charges two capacity slots per
        // live digit. Use the same num-bigint growth slack here.
        let one_big_int_payload = digits
            .checked_mul(2)?
            .checked_mul(std::mem::size_of::<usize>())?;
        let live_rationals = degree_slots.checked_mul(4)?.checked_add(32)?;
        // Two BigInts per rational plus standalone denominator, power and
        // binomial-expression temporaries.
        let live_big_ints = live_rationals.checked_mul(2)?.checked_add(32)?;
        let big_rational_payload_bytes = one_big_int_payload.checked_mul(live_big_ints)?;
        let exact_nonvector_object_bytes = std::mem::size_of::<BigRational>()
            .checked_mul(32)?
            .checked_add(std::mem::size_of::<BigInt>().checked_mul(32)?)?;
        let exact_object_bytes = std::mem::size_of::<BigRational>()
            .checked_mul(degree_slots.checked_mul(4)?)?
            .checked_add(exact_nonvector_object_bytes)?;
        let peak_bytes = angle_box_bytes
            .checked_add(big_rational_payload_bytes)?
            .checked_add(exact_object_bytes)?;
        Some(CycleScheduleDyadicWorkspaceBoundV2 {
            schedule_fingerprint_v2: self.schedule_fingerprint_v2,
            limits,
            max_depth,
            angle_box_bytes,
            big_rational_payload_bytes,
            exact_object_bytes,
            exact_nonvector_object_bytes,
            peak_bytes,
        })
    }

    /// Allocation-free binding match with caller-defined cooperative stops.
    /// Every face, audit edge and material hinge is polled before hashing.
    pub(crate) fn matches_binding_with_checkpoint_v2<Stop>(
        &self,
        geometry: &MaterialHingeGraphGeometry,
        audit: &MaterialHingeGraphAudit,
        fixed_face: FaceId,
        checkpoint: &mut impl FnMut() -> Result<(), Stop>,
    ) -> Result<bool, Stop> {
        checkpoint()?;
        if self.fixed_face != fixed_face {
            return Ok(false);
        }
        let mut hash = Sha256::new();
        hash.update(fixed_face.canonical_bytes());
        for face in audit.faces() {
            checkpoint()?;
            hash.update(face.canonical_bytes());
        }
        for edge in audit.spanning_hinges().iter().chain(audit.closure_hinges()) {
            checkpoint()?;
            hash.update(edge.canonical_bytes());
        }
        for hinge in geometry.hinges() {
            checkpoint()?;
            hash.update(hinge.edge().canonical_bytes());
            hash.update(hinge.left_face().canonical_bytes());
            hash.update(hinge.right_face().canonical_bytes());
            hash.update([match hinge.assignment() {
                ori_topology::FoldAssignment::Mountain => 0,
                ori_topology::FoldAssignment::Valley => 1,
            }]);
            for value in [
                hinge.start().x(),
                hinge.start().y(),
                hinge.start().z(),
                hinge.end().x(),
                hinge.end().y(),
                hinge.end().z(),
                hinge.axis().x(),
                hinge.axis().y(),
                hinge.axis().z(),
            ] {
                hash.update(value.to_bits().to_be_bytes());
            }
        }
        checkpoint()?;
        let binding: [u8; 32] = hash.finalize().into();
        Ok(self.binding_fingerprint == binding)
    }
}
