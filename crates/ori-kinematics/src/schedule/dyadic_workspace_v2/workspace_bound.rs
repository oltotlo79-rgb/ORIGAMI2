use super::*;

impl CanonicalCycleScheduleV1 {
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
    pub fn checked_dyadic_workspace_upper_bound_v2(
        &self,
        max_depth: u32,
        limits: CycleScheduleLimitsV1,
    ) -> Option<CycleScheduleDyadicWorkspaceBoundV2> {
        self.checked_dyadic_workspace_upper_bound_impl_v2(max_depth, limits, &mut || true)
    }

    /// Checkpointed structural scan for [`Self::checked_dyadic_workspace_upper_bound_v2`].
    /// Every retained schedule entry, exact vector and exact coefficient is
    /// polled, including a final prepublication poll. This inventory issues no
    /// proof authority; checkpoint count and internal scan granularity are
    /// cooperative-stop implementation details, not a stable API contract.
    pub fn checked_dyadic_workspace_upper_bound_with_checkpoint_v2(
        &self,
        max_depth: u32,
        limits: CycleScheduleLimitsV1,
        mut checkpoint: impl FnMut() -> Result<(), CycleScheduleDyadicEvaluationStopV2>,
    ) -> Result<CycleScheduleDyadicWorkspaceBoundV2, CycleScheduleDyadicEvaluationErrorV2> {
        let mut stopped = None;
        let result =
            self.checked_dyadic_workspace_upper_bound_impl_v2(max_depth, limits, &mut || {
                match checkpoint() {
                    Ok(()) => true,
                    Err(stop) => {
                        stopped = Some(stop);
                        false
                    }
                }
            });
        if let Some(stop) = stopped {
            return Err(match stop {
                CycleScheduleDyadicEvaluationStopV2::Cancelled => {
                    CycleScheduleDyadicEvaluationErrorV2::Cancelled
                }
                CycleScheduleDyadicEvaluationStopV2::DeadlineExceeded => {
                    CycleScheduleDyadicEvaluationErrorV2::DeadlineExceeded
                }
            });
        }
        result.ok_or(CycleScheduleDyadicEvaluationErrorV2::Prepare(
            CycleSchedulePrepareErrorV1::ResourceLimit,
        ))
    }

    fn checked_dyadic_workspace_upper_bound_impl_v2(
        &self,
        max_depth: u32,
        limits: CycleScheduleLimitsV1,
        checkpoint: &mut impl FnMut() -> bool,
    ) -> Option<CycleScheduleDyadicWorkspaceBoundV2> {
        if !checkpoint() {
            return None;
        }
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
            for entry in &self.entries {
                if !checkpoint()
                    || entry.coefficients.is_empty()
                    || entry.coefficients.len() > degree_slots
                    || entry.coefficients.len() > limits.max_work
                {
                    return None;
                }
                for _ in &entry.coefficients {
                    if !checkpoint() {
                        return None;
                    }
                }
            }
        } else {
            // A schedule may have been prepared under a different ceiling.
            // Scan every retained exact source before doing arithmetic so a
            // smaller submitted V2 policy cannot undercharge a large stored
            // numerator, denominator, domain, or certificate.
            for entry in &self.half_angle_entries {
                if !checkpoint() {
                    return None;
                }
                let vectors = [
                    entry.numerator_power_coefficients.as_slice(),
                    entry.denominator_power_coefficients.as_slice(),
                    entry.numerator_certificate.coefficients.as_slice(),
                    entry.denominator_certificate.coefficients.as_slice(),
                ];
                for values in vectors {
                    if !checkpoint()
                        || values.is_empty()
                        || values.len() > degree_slots
                        || values
                            .len()
                            .checked_mul(values.len())
                            .is_none_or(|work| work > limits.max_work)
                    {
                        return None;
                    }
                }
                for value in entry.u_domain.iter().chain(vectors.into_iter().flatten()) {
                    if !checkpoint()
                        || value.numer().bits() > u64::from(limits.max_coefficient_bits)
                        || value.denom().bits() > u64::from(limits.max_coefficient_bits)
                    {
                        return None;
                    }
                }
            }
        }
        let angle_box_bytes =
            std::mem::size_of::<(EdgeId, OutwardIntervalV1)>().checked_mul(hinge_count)?;
        if self.half_angle_entries.is_empty() {
            if !checkpoint() {
                return None;
            }
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
        if !checkpoint() {
            return None;
        }
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
}
