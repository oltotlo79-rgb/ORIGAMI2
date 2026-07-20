//! Exact zero-thickness evidence boundary for authenticated material faces.
//!
//! This module is deliberately private. `TopologyRelation` and
//! `IntersectionEvidenceV2` are public policy labels, not certificates, so a
//! caller-provided triangle or relation must never enter the runtime
//! dispatcher through this boundary.
//!
//! `ori-kinematics` exposes a read-only canonical face-boundary registry from
//! the exact private source retained by `MaterialTreePose`; its prepared-tree
//! boundary has already passed exact simple-polygon, distinct-coordinate and
//! non-zero-edge validation. This module independently revalidates the
//! collision-facing registries, deterministically ear-clips each whole face in
//! exact binary64 arithmetic, and proves complete triangle-pair coverage.
//!
//! A triangle edge is not automatically a material boundary: lower-dimensional
//! contact on an ear-clipping diagonal is checked against both authenticated
//! outer boundaries. If that face-level relation cannot be proved, the result
//! is `Indeterminate`, never a false-safe `Touching`. Shared-vertex allowances
//! require every contact to be the one authenticated vertex. Shared-hinge
//! contact requires complete coverage of the authenticated edge and still
//! returns `RequiresHingeModel`; no finite hinge exception is granted here.
//! The exact affine lift preserves each face plane but deliberately does not
//! weld independently solved face transforms. A noncardinal shared endpoint
//! that is not bit-exact under both affine images is authenticated as a
//! private pose mismatch. The complete raw pair is still scanned, but its
//! result is forced to `Indeterminate`: an arbitrarily small endpoint mismatch
//! can create a false relative-interior crossing or coplanar-area overlap.
//! Canonical watertight hinge geometry is a later prerequisite for admitting
//! those raw diagnostics as collision evidence.

use std::cmp::Ordering;

use num_bigint::{BigInt, Sign};
use num_integer::Integer;
use num_rational::BigRational;
use num_traits::{One, Signed, Zero};
use ori_domain::{EdgeId, FaceId, VertexId};
use ori_kinematics::{MaterialTreePose, Point3, RigidTransform};

use crate::{
    IntersectionEvidenceV2, TopologyContactDecision, TopologyRelation,
    classify_runtime_topology_contact_v2,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ZeroThicknessAnalysisError {
    EvidenceUnavailable,
    ResourceLimitExceeded,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct RationalWork {
    max_input_bits: usize,
    total_input_storage_bits: usize,
    total_retained_clone_bits: usize,
    operations: usize,
    max_preflight_bits: usize,
    max_intermediate_bits: usize,
    gcd_fallback_calls: usize,
    gcd_fallback_input_bits: usize,
    max_gcd_fallback_call_input_bits: usize,
    rational_allocations: usize,
    max_rational_allocation_bits: usize,
    total_rational_allocation_bits: usize,
    max_output_bits: usize,
    total_output_bits: usize,
}

// Allocation counters are logical exact-kernel allocations: every BigInt
// magnitude or BigRational clone explicitly produced at this boundary is
// precharged by a safe payload-bit upper bound. The workspace pins
// num-bigint 0.4.8 and num-rational 0.4.2. Their internal arithmetic scratch is
// additionally bounded by the operation/intermediate-bit limits and, for
// Stein GCD, by both the call and aggregate-input-bit limits. This keeps the
// public contract independent of allocator capacity rounding while still
// placing finite bounds on every production path.
//
// Vec backing stores are a separate structural resource: every production
// numeric Vec uses `try_reserve_exact` only after its face, boundary, triangle,
// pair, interval or clipping bound has been checked. Its retained rational
// payloads are still charged here when cloned or constructed.
struct RationalWorkMeter<'a> {
    limits: &'a ZeroThicknessGeometryLimits,
    work: RationalWork,
}

impl<'a> RationalWorkMeter<'a> {
    fn new(limits: &'a ZeroThicknessGeometryLimits) -> Self {
        Self {
            limits,
            work: RationalWork::default(),
        }
    }

    #[cfg(test)]
    fn unlimited(limits: &'a ZeroThicknessGeometryLimits) -> Self {
        Self::new(limits)
    }

    fn checked_allocation_work(
        &self,
        allocation_bits: &[usize],
    ) -> Result<(usize, usize, usize), ZeroThicknessAnalysisError> {
        let count = self
            .work
            .rational_allocations
            .checked_add(allocation_bits.len())
            .ok_or(ZeroThicknessAnalysisError::ResourceLimitExceeded)?;
        if count > self.limits.max_rational_allocations {
            return Err(ZeroThicknessAnalysisError::ResourceLimitExceeded);
        }

        let mut maximum = self.work.max_rational_allocation_bits;
        let mut total = self.work.total_rational_allocation_bits;
        for bits in allocation_bits {
            if *bits > self.limits.max_rational_allocation_bits {
                return Err(ZeroThicknessAnalysisError::ResourceLimitExceeded);
            }
            total = total
                .checked_add(*bits)
                .ok_or(ZeroThicknessAnalysisError::ResourceLimitExceeded)?;
            if total > self.limits.max_total_rational_allocation_bits {
                return Err(ZeroThicknessAnalysisError::ResourceLimitExceeded);
            }
            maximum = maximum.max(*bits);
        }
        Ok((count, maximum, total))
    }

    fn commit_allocation_work(&mut self, checked: (usize, usize, usize)) {
        self.work.rational_allocations = checked.0;
        self.work.max_rational_allocation_bits = checked.1;
        self.work.total_rational_allocation_bits = checked.2;
    }

    fn charge_allocations(
        &mut self,
        allocation_bits: &[usize],
    ) -> Result<(), ZeroThicknessAnalysisError> {
        let checked = self.checked_allocation_work(allocation_bits)?;
        self.commit_allocation_work(checked);
        Ok(())
    }

    fn zero(&mut self) -> Result<BigRational, ZeroThicknessAnalysisError> {
        self.preflight_value_bits(1)?;
        self.charge_allocations(&[0, 1])?;
        Ok(BigRational::zero())
    }

    fn one(&mut self) -> Result<BigRational, ZeroThicknessAnalysisError> {
        self.preflight_value_bits(1)?;
        self.charge_allocations(&[1, 1])?;
        Ok(BigRational::one())
    }

    fn positive_integer(&mut self, value: u64) -> Result<BigRational, ZeroThicknessAnalysisError> {
        if value == 0 {
            return self.zero();
        }
        let numerator_bits = usize::try_from(u64::BITS - value.leading_zeros())
            .map_err(|_| ZeroThicknessAnalysisError::ResourceLimitExceeded)?;
        self.preflight_value_bits(numerator_bits)?;
        self.charge_allocations(&[numerator_bits, 1])?;
        Ok(BigRational::from_integer(BigInt::from(value)))
    }

    fn clone_temporary(
        &mut self,
        value: &BigRational,
    ) -> Result<BigRational, ZeroThicknessAnalysisError> {
        self.preflight_value_bits(rational_bits(value))?;
        self.charge_allocations(&[bigint_bits(value.numer()), bigint_bits(value.denom())])?;
        Ok(value.clone())
    }

    fn negate_temporary(
        &mut self,
        value: &BigRational,
    ) -> Result<BigRational, ZeroThicknessAnalysisError> {
        self.preflight_value_bits(rational_bits(value))?;
        self.charge_allocations(&[bigint_bits(value.numer()), bigint_bits(value.denom())])?;
        Ok(-value)
    }

    fn input_binary64_common_unit(
        &mut self,
        value: f64,
    ) -> Result<BigRational, ZeroThicknessAnalysisError> {
        let bits = value.to_bits();
        let negative = bits >> 63 != 0;
        let exponent = ((bits >> 52) & 0x7ff) as usize;
        let fraction = bits & ((1_u64 << 52) - 1);
        let (significand, shift) = if exponent == 0 {
            (fraction, 0)
        } else {
            (fraction | (1_u64 << 52), exponent - 1)
        };
        let significand_bits = if significand == 0 {
            0
        } else {
            usize::try_from(u64::BITS - significand.leading_zeros())
                .map_err(|_| ZeroThicknessAnalysisError::ResourceLimitExceeded)?
        };
        let numerator_bits = shifted_nonzero_bits(significand_bits, shift)?;
        self.charge_input_shape(
            numerator_bits.max(1),
            numerator_bits
                .checked_add(1)
                .ok_or(ZeroThicknessAnalysisError::ResourceLimitExceeded)?,
            &[significand_bits, numerator_bits, 1],
        )?;
        let mut integer = BigInt::from(significand) << shift;
        if negative {
            integer = -integer;
        }
        let result = BigRational::from_integer(integer);
        debug_assert_eq!(rational_bits(&result), numerator_bits.max(1));
        Ok(result)
    }

    fn input_binary64_scalar(
        &mut self,
        value: f64,
    ) -> Result<BigRational, ZeroThicknessAnalysisError> {
        const COMMON_UNIT_SHIFT: usize = 1_074;
        let bits = value.to_bits();
        let negative = bits >> 63 != 0;
        let exponent = ((bits >> 52) & 0x7ff) as usize;
        let fraction = bits & ((1_u64 << 52) - 1);
        let (significand, common_shift) = if exponent == 0 {
            (fraction, 0)
        } else {
            (fraction | (1_u64 << 52), exponent - 1)
        };
        if significand == 0 {
            self.charge_input_shape(1, 1, &[0, 1])?;
            let result = BigRational::zero();
            return Ok(result);
        }

        let trailing = usize::try_from(significand.trailing_zeros())
            .map_err(|_| ZeroThicknessAnalysisError::ResourceLimitExceeded)?;
        let cancellable = trailing
            .checked_add(common_shift)
            .ok_or(ZeroThicknessAnalysisError::ResourceLimitExceeded)?
            .min(COMMON_UNIT_SHIFT);
        let reduced_significand = significand >> trailing.min(cancellable);
        let residual_numerator_shift = common_shift
            .checked_add(trailing)
            .and_then(|value| value.checked_sub(cancellable))
            .ok_or(ZeroThicknessAnalysisError::ResourceLimitExceeded)?;
        let denominator_shift = COMMON_UNIT_SHIFT
            .checked_sub(cancellable)
            .ok_or(ZeroThicknessAnalysisError::ResourceLimitExceeded)?;
        let significand_bits = usize::try_from(u64::BITS - reduced_significand.leading_zeros())
            .map_err(|_| ZeroThicknessAnalysisError::ResourceLimitExceeded)?;
        let numerator_bits = shifted_nonzero_bits(significand_bits, residual_numerator_shift)?;
        let denominator_bits = shifted_nonzero_bits(1, denominator_shift)?;
        self.charge_input_shape(
            numerator_bits.max(denominator_bits),
            numerator_bits
                .checked_add(denominator_bits)
                .ok_or(ZeroThicknessAnalysisError::ResourceLimitExceeded)?,
            &[significand_bits, numerator_bits, 1, denominator_bits],
        )?;
        let mut numerator = BigInt::from(reduced_significand) << residual_numerator_shift;
        if negative {
            numerator = -numerator;
        }
        let denominator = BigInt::one() << denominator_shift;
        let result = BigRational::new_raw(numerator, denominator);
        debug_assert_eq!(rational_bits(&result), numerator_bits.max(denominator_bits));
        Ok(result)
    }

    fn charge_input_shape(
        &mut self,
        bits: usize,
        storage: usize,
        allocation_bits: &[usize],
    ) -> Result<(), ZeroThicknessAnalysisError> {
        let total = self
            .work
            .total_input_storage_bits
            .checked_add(storage)
            .ok_or(ZeroThicknessAnalysisError::ResourceLimitExceeded)?;
        if bits > self.limits.max_rational_input_bits
            || total > self.limits.max_total_rational_input_storage_bits
        {
            return Err(ZeroThicknessAnalysisError::ResourceLimitExceeded);
        }
        let allocation = self.checked_allocation_work(allocation_bits)?;
        self.work.max_input_bits = self.work.max_input_bits.max(bits);
        self.work.total_input_storage_bits = total;
        self.commit_allocation_work(allocation);
        Ok(())
    }

    fn clone_retained(
        &mut self,
        value: &BigRational,
    ) -> Result<BigRational, ZeroThicknessAnalysisError> {
        self.preflight_value_bits(rational_bits(value))?;
        let storage = rational_storage_bits(value)?;
        let total = self
            .work
            .total_retained_clone_bits
            .checked_add(storage)
            .ok_or(ZeroThicknessAnalysisError::ResourceLimitExceeded)?;
        if total > self.limits.max_total_rational_retained_clone_bits {
            return Err(ZeroThicknessAnalysisError::ResourceLimitExceeded);
        }
        let allocation = self
            .checked_allocation_work(&[bigint_bits(value.numer()), bigint_bits(value.denom())])?;
        self.work.total_retained_clone_bits = total;
        self.commit_allocation_work(allocation);
        Ok(value.clone())
    }

    fn operation(&mut self) -> Result<(), ZeroThicknessAnalysisError> {
        let next = self
            .work
            .operations
            .checked_add(1)
            .ok_or(ZeroThicknessAnalysisError::ResourceLimitExceeded)?;
        if next > self.limits.max_rational_operations {
            return Err(ZeroThicknessAnalysisError::ResourceLimitExceeded);
        }
        self.work.operations = next;
        Ok(())
    }

    fn preflight_value_bits(&mut self, bits: usize) -> Result<(), ZeroThicknessAnalysisError> {
        self.work.max_preflight_bits = self.work.max_preflight_bits.max(bits);
        if bits > self.limits.max_rational_intermediate_bits {
            return Err(ZeroThicknessAnalysisError::ResourceLimitExceeded);
        }
        Ok(())
    }

    fn gcd_fallback(
        &mut self,
        left: &BigInt,
        right: &BigInt,
    ) -> Result<BigInt, ZeroThicknessAnalysisError> {
        let left_bits = bigint_bits(left);
        let right_bits = bigint_bits(right);
        self.preflight_value_bits(left_bits.max(right_bits))?;
        let call_input_bits = left_bits
            .checked_add(right_bits)
            .ok_or(ZeroThicknessAnalysisError::ResourceLimitExceeded)?;
        let calls = self
            .work
            .gcd_fallback_calls
            .checked_add(1)
            .ok_or(ZeroThicknessAnalysisError::ResourceLimitExceeded)?;
        let total_input_bits = self
            .work
            .gcd_fallback_input_bits
            .checked_add(call_input_bits)
            .ok_or(ZeroThicknessAnalysisError::ResourceLimitExceeded)?;
        if calls > self.limits.max_rational_gcd_fallback_calls
            || total_input_bits > self.limits.max_rational_gcd_fallback_input_bits
        {
            return Err(ZeroThicknessAnalysisError::ResourceLimitExceeded);
        }
        let output_bits = gcd_bits_upper_bound(left, right);
        self.preflight_value_bits(output_bits)?;
        // num-bigint 0.4.8 implements Stein GCD by cloning both magnitudes and
        // mutating those two buffers in place before returning the surviving
        // magnitude. Charge both working clones and the returned integer
        // before invoking the dependency.
        let allocation = self.checked_allocation_work(&[left_bits, right_bits, output_bits])?;
        self.work.gcd_fallback_calls = calls;
        self.work.gcd_fallback_input_bits = total_input_bits;
        self.work.max_gcd_fallback_call_input_bits = self
            .work
            .max_gcd_fallback_call_input_bits
            .max(call_input_bits);
        self.commit_allocation_work(allocation);
        let result = left.gcd(right);
        debug_assert!(bigint_bits(&result) <= output_bits);
        Ok(result)
    }

    fn normalize_refined(
        &mut self,
        numerator: BigInt,
        denominator: BigInt,
    ) -> Result<BigRational, ZeroThicknessAnalysisError> {
        if denominator.is_zero() {
            return Err(ZeroThicknessAnalysisError::EvidenceUnavailable);
        }
        let gcd = self.gcd_fallback(&numerator, &denominator)?;
        let numerator_bits = quotient_bits_upper_bound(&numerator, &gcd)?;
        let denominator_bits = quotient_bits_upper_bound(&denominator, &gcd)?;
        self.preflight_value_bits(numerator_bits)?;
        self.preflight_value_bits(denominator_bits)?;
        self.charge_allocations(&[numerator_bits, denominator_bits])?;
        let mut numerator = numerator / &gcd;
        let mut denominator = denominator / gcd;
        debug_assert!(bigint_bits(&numerator) <= numerator_bits);
        debug_assert!(bigint_bits(&denominator) <= denominator_bits);
        if denominator.is_negative() {
            numerator = -numerator;
            denominator = -denominator;
        }
        Ok(BigRational::new_raw(numerator, denominator))
    }

    fn add(
        &mut self,
        left: &BigRational,
        right: &BigRational,
    ) -> Result<BigRational, ZeroThicknessAnalysisError> {
        self.add_or_subtract(left, right, false)
    }

    fn subtract(
        &mut self,
        left: &BigRational,
        right: &BigRational,
    ) -> Result<BigRational, ZeroThicknessAnalysisError> {
        self.add_or_subtract(left, right, true)
    }

    fn add_or_subtract(
        &mut self,
        left: &BigRational,
        right: &BigRational,
        subtract: bool,
    ) -> Result<BigRational, ZeroThicknessAnalysisError> {
        self.operation()?;
        self.preflight_value_bits(rational_bits(left))?;
        self.preflight_value_bits(rational_bits(right))?;
        if right.is_zero() {
            return self.clone_temporary(left);
        }
        if left.is_zero() {
            let result = if subtract {
                self.negate_temporary(right)?
            } else {
                self.clone_temporary(right)?
            };
            self.observe_intermediate(&result)?;
            return Ok(result);
        }

        if left.denom() == right.denom() {
            let numerator_bits =
                sum_bits_upper_bound(bigint_bits(left.numer()), bigint_bits(right.numer()))?;
            let denominator_bits = bigint_bits(left.denom());
            self.preflight_value_bits(numerator_bits)?;
            self.preflight_value_bits(denominator_bits)?;
            self.charge_allocations(&[numerator_bits, denominator_bits])?;
            let numerator = if subtract {
                left.numer() - right.numer()
            } else {
                left.numer() + right.numer()
            };
            let denominator = left.denom().clone();
            debug_assert!(bigint_bits(&numerator) <= numerator_bits);
            debug_assert!(bigint_bits(&denominator) <= denominator_bits);
            let result = self.normalize_refined(numerator, denominator)?;
            self.observe_intermediate(&result)?;
            return Ok(result);
        }

        let denominator_gcd = self.gcd_fallback(left.denom(), right.denom())?;
        let left_multiplier_bits = quotient_bits_upper_bound(right.denom(), &denominator_gcd)?;
        let right_multiplier_bits = quotient_bits_upper_bound(left.denom(), &denominator_gcd)?;
        self.preflight_value_bits(left_multiplier_bits)?;
        self.preflight_value_bits(right_multiplier_bits)?;
        self.charge_allocations(&[left_multiplier_bits, right_multiplier_bits])?;
        let left_multiplier = right.denom() / &denominator_gcd;
        let right_multiplier = left.denom() / denominator_gcd;
        debug_assert!(bigint_bits(&left_multiplier) <= left_multiplier_bits);
        debug_assert!(bigint_bits(&right_multiplier) <= right_multiplier_bits);

        let left_product_bits =
            product_bits_upper_bound(bigint_bits(left.numer()), bigint_bits(&left_multiplier))?;
        let right_product_bits =
            product_bits_upper_bound(bigint_bits(right.numer()), bigint_bits(&right_multiplier))?;
        let numerator_bits = sum_bits_upper_bound(left_product_bits, right_product_bits)?;
        let denominator_bits =
            product_bits_upper_bound(bigint_bits(left.denom()), bigint_bits(&left_multiplier))?;
        for bits in [
            left_product_bits,
            right_product_bits,
            numerator_bits,
            denominator_bits,
        ] {
            self.preflight_value_bits(bits)?;
        }
        self.charge_allocations(&[
            left_product_bits,
            right_product_bits,
            numerator_bits,
            denominator_bits,
        ])?;
        let left_numerator = left.numer() * &left_multiplier;
        let right_numerator = right.numer() * &right_multiplier;
        let numerator = if subtract {
            left_numerator - right_numerator
        } else {
            left_numerator + right_numerator
        };
        let denominator = left.denom() * left_multiplier;
        debug_assert!(bigint_bits(&numerator) <= numerator_bits);
        debug_assert!(bigint_bits(&denominator) <= denominator_bits);
        let result = self.normalize_refined(numerator, denominator)?;
        self.observe_intermediate(&result)?;
        Ok(result)
    }

    fn multiply(
        &mut self,
        left: &BigRational,
        right: &BigRational,
    ) -> Result<BigRational, ZeroThicknessAnalysisError> {
        self.operation()?;
        self.preflight_value_bits(rational_bits(left))?;
        self.preflight_value_bits(rational_bits(right))?;
        if left.is_zero() || right.is_zero() {
            return self.zero();
        }
        let numerator_gcd = self.gcd_fallback(left.numer(), right.denom())?;
        let denominator_gcd = self.gcd_fallback(left.denom(), right.numer())?;
        let left_numerator_bits = quotient_bits_upper_bound(left.numer(), &numerator_gcd)?;
        let right_denominator_bits = quotient_bits_upper_bound(right.denom(), &numerator_gcd)?;
        let right_numerator_bits = quotient_bits_upper_bound(right.numer(), &denominator_gcd)?;
        let left_denominator_bits = quotient_bits_upper_bound(left.denom(), &denominator_gcd)?;
        for bits in [
            left_numerator_bits,
            right_denominator_bits,
            right_numerator_bits,
            left_denominator_bits,
        ] {
            self.preflight_value_bits(bits)?;
        }
        self.charge_allocations(&[
            left_numerator_bits,
            right_denominator_bits,
            right_numerator_bits,
            left_denominator_bits,
        ])?;
        let left_numerator = left.numer() / &numerator_gcd;
        let right_denominator = right.denom() / numerator_gcd;
        let right_numerator = right.numer() / &denominator_gcd;
        let left_denominator = left.denom() / denominator_gcd;
        let numerator_bits =
            product_bits_upper_bound(bigint_bits(&left_numerator), bigint_bits(&right_numerator))?;
        let denominator_bits = product_bits_upper_bound(
            bigint_bits(&left_denominator),
            bigint_bits(&right_denominator),
        )?;
        self.preflight_value_bits(numerator_bits)?;
        self.preflight_value_bits(denominator_bits)?;
        self.charge_allocations(&[numerator_bits, denominator_bits])?;
        let numerator = left_numerator * right_numerator;
        let denominator = left_denominator * right_denominator;
        debug_assert!(bigint_bits(&numerator) <= numerator_bits);
        debug_assert!(bigint_bits(&denominator) <= denominator_bits);
        let result = BigRational::new_raw(numerator, denominator);
        self.observe_intermediate(&result)?;
        Ok(result)
    }

    fn divide(
        &mut self,
        left: &BigRational,
        right: &BigRational,
    ) -> Result<BigRational, ZeroThicknessAnalysisError> {
        if right.is_zero() {
            return Err(ZeroThicknessAnalysisError::EvidenceUnavailable);
        }
        self.operation()?;
        self.preflight_value_bits(rational_bits(left))?;
        self.preflight_value_bits(rational_bits(right))?;
        if left.is_zero() {
            return self.zero();
        }

        let numerator_gcd = self.gcd_fallback(left.numer(), right.numer())?;
        let denominator_gcd = self.gcd_fallback(left.denom(), right.denom())?;
        let left_numerator_bits = quotient_bits_upper_bound(left.numer(), &numerator_gcd)?;
        let right_numerator_bits = quotient_bits_upper_bound(right.numer(), &numerator_gcd)?;
        let right_denominator_bits = quotient_bits_upper_bound(right.denom(), &denominator_gcd)?;
        let left_denominator_bits = quotient_bits_upper_bound(left.denom(), &denominator_gcd)?;
        for bits in [
            left_numerator_bits,
            right_numerator_bits,
            right_denominator_bits,
            left_denominator_bits,
        ] {
            self.preflight_value_bits(bits)?;
        }
        self.charge_allocations(&[
            left_numerator_bits,
            right_numerator_bits,
            right_denominator_bits,
            left_denominator_bits,
        ])?;
        let left_numerator = left.numer() / &numerator_gcd;
        let right_numerator = right.numer() / numerator_gcd;
        let right_denominator = right.denom() / &denominator_gcd;
        let left_denominator = left.denom() / denominator_gcd;
        let numerator_bits = product_bits_upper_bound(
            bigint_bits(&left_numerator),
            bigint_bits(&right_denominator),
        )?;
        let denominator_bits = product_bits_upper_bound(
            bigint_bits(&left_denominator),
            bigint_bits(&right_numerator),
        )?;
        self.preflight_value_bits(numerator_bits)?;
        self.preflight_value_bits(denominator_bits)?;
        self.charge_allocations(&[numerator_bits, denominator_bits])?;
        let mut numerator = left_numerator * right_denominator;
        let mut denominator = left_denominator * right_numerator;
        debug_assert!(bigint_bits(&numerator) <= numerator_bits);
        debug_assert!(bigint_bits(&denominator) <= denominator_bits);
        if denominator.is_negative() {
            numerator = -numerator;
            denominator = -denominator;
        }
        let result = BigRational::new_raw(numerator, denominator);
        self.observe_intermediate(&result)?;
        Ok(result)
    }

    fn compare(
        &mut self,
        left: &BigRational,
        right: &BigRational,
    ) -> Result<Ordering, ZeroThicknessAnalysisError> {
        self.operation()?;
        self.preflight_value_bits(rational_bits(left))?;
        self.preflight_value_bits(rational_bits(right))?;
        match (left.numer().sign(), right.numer().sign()) {
            (Sign::Minus, Sign::NoSign | Sign::Plus) | (Sign::NoSign, Sign::Plus) => {
                return Ok(Ordering::Less);
            }
            (Sign::NoSign | Sign::Plus, Sign::Minus) | (Sign::Plus, Sign::NoSign) => {
                return Ok(Ordering::Greater);
            }
            (Sign::NoSign, Sign::NoSign) => return Ok(Ordering::Equal),
            (Sign::Minus, Sign::Minus) | (Sign::Plus, Sign::Plus) => {}
        }
        if left.denom() == right.denom() {
            return Ok(left.numer().cmp(right.numer()));
        }

        let denominator_gcd = self.gcd_fallback(left.denom(), right.denom())?;
        let left_multiplier_bits = quotient_bits_upper_bound(right.denom(), &denominator_gcd)?;
        let right_multiplier_bits = quotient_bits_upper_bound(left.denom(), &denominator_gcd)?;
        let left_product_bits =
            product_bits_upper_bound(bigint_bits(left.numer()), left_multiplier_bits)?;
        let right_product_bits =
            product_bits_upper_bound(bigint_bits(right.numer()), right_multiplier_bits)?;
        for bits in [
            left_multiplier_bits,
            right_multiplier_bits,
            left_product_bits,
            right_product_bits,
        ] {
            self.preflight_value_bits(bits)?;
        }
        self.charge_allocations(&[
            left_multiplier_bits,
            right_multiplier_bits,
            left_product_bits,
            right_product_bits,
        ])?;
        let left_multiplier = right.denom() / &denominator_gcd;
        let right_multiplier = left.denom() / denominator_gcd;
        let left_product = left.numer() * left_multiplier;
        let right_product = right.numer() * right_multiplier;
        debug_assert!(bigint_bits(&left_product) <= left_product_bits);
        debug_assert!(bigint_bits(&right_product) <= right_product_bits);
        Ok(left_product.cmp(&right_product))
    }

    fn equal(
        &mut self,
        left: &BigRational,
        right: &BigRational,
    ) -> Result<bool, ZeroThicknessAnalysisError> {
        self.operation()?;
        self.preflight_value_bits(rational_bits(left))?;
        self.preflight_value_bits(rational_bits(right))?;
        Ok(rational_structurally_eq(left, right))
    }

    fn observe_intermediate(
        &mut self,
        value: &BigRational,
    ) -> Result<(), ZeroThicknessAnalysisError> {
        let bits = rational_bits(value);
        self.work.max_intermediate_bits = self.work.max_intermediate_bits.max(bits);
        if bits > self.limits.max_rational_intermediate_bits {
            return Err(ZeroThicknessAnalysisError::ResourceLimitExceeded);
        }
        Ok(())
    }

    fn output(&mut self, value: &BigRational) -> Result<(), ZeroThicknessAnalysisError> {
        let bits = rational_bits(value);
        let storage = rational_storage_bits(value)?;
        let total = self
            .work
            .total_output_bits
            .checked_add(storage)
            .ok_or(ZeroThicknessAnalysisError::ResourceLimitExceeded)?;
        if bits > self.limits.max_rational_output_bits
            || total > self.limits.max_total_rational_output_bits
        {
            return Err(ZeroThicknessAnalysisError::ResourceLimitExceeded);
        }
        self.work.max_output_bits = self.work.max_output_bits.max(bits);
        self.work.total_output_bits = total;
        Ok(())
    }
}

fn bigint_bits(value: &BigInt) -> usize {
    value.bits() as usize
}

fn rational_bits(value: &BigRational) -> usize {
    bigint_bits(value.numer()).max(bigint_bits(value.denom()))
}

fn rational_storage_bits(value: &BigRational) -> Result<usize, ZeroThicknessAnalysisError> {
    bigint_bits(value.numer())
        .checked_add(bigint_bits(value.denom()))
        .ok_or(ZeroThicknessAnalysisError::ResourceLimitExceeded)
}

/// Allocation-free equality for the canonical rational representation issued
/// by this module. `Ratio::eq` delegates to rational ordering and may allocate
/// for unlike denominators, so production predicates must compare the already
/// normalized numerator and denominator fields directly.
fn rational_structurally_eq(left: &BigRational, right: &BigRational) -> bool {
    left.numer() == right.numer() && left.denom() == right.denom()
}

fn shifted_nonzero_bits(bits: usize, shift: usize) -> Result<usize, ZeroThicknessAnalysisError> {
    if bits == 0 {
        Ok(0)
    } else {
        bits.checked_add(shift)
            .ok_or(ZeroThicknessAnalysisError::ResourceLimitExceeded)
    }
}

fn product_bits_upper_bound(
    left_bits: usize,
    right_bits: usize,
) -> Result<usize, ZeroThicknessAnalysisError> {
    if left_bits == 0 || right_bits == 0 {
        return Ok(0);
    }
    left_bits
        .checked_add(right_bits)
        .ok_or(ZeroThicknessAnalysisError::ResourceLimitExceeded)
}

fn sum_bits_upper_bound(
    left_bits: usize,
    right_bits: usize,
) -> Result<usize, ZeroThicknessAnalysisError> {
    left_bits
        .max(right_bits)
        .checked_add(1)
        .ok_or(ZeroThicknessAnalysisError::ResourceLimitExceeded)
}

fn quotient_bits_upper_bound(
    dividend: &BigInt,
    divisor: &BigInt,
) -> Result<usize, ZeroThicknessAnalysisError> {
    if divisor.is_zero() {
        return Err(ZeroThicknessAnalysisError::EvidenceUnavailable);
    }
    Ok(bigint_bits(dividend))
}

fn gcd_bits_upper_bound(left: &BigInt, right: &BigInt) -> usize {
    match (left.is_zero(), right.is_zero()) {
        (true, true) => 0,
        (true, false) => bigint_bits(right),
        (false, true) => bigint_bits(left),
        (false, false) => bigint_bits(left).min(bigint_bits(right)),
    }
}

fn try_array3<T>(
    mut element: impl FnMut(usize) -> Result<T, ZeroThicknessAnalysisError>,
) -> Result<[T; 3], ZeroThicknessAnalysisError> {
    Ok([element(0)?, element(1)?, element(2)?])
}

#[cfg(test)]
fn unlimited_rational_limits() -> ZeroThicknessGeometryLimits {
    ZeroThicknessGeometryLimits {
        max_boundary_vertices_per_face: usize::MAX,
        max_total_boundary_vertices: usize::MAX,
        max_triangles_per_face: usize::MAX,
        max_total_triangles: usize::MAX,
        max_triangulation_work_per_face: usize::MAX,
        max_total_triangulation_work: usize::MAX,
        max_registry_authentication_work: usize::MAX,
        max_triangle_pairs_per_face_pair: usize::MAX,
        max_total_triangle_pairs: usize::MAX,
        max_boundary_relation_work_per_face_pair: usize::MAX,
        max_total_boundary_relation_work: usize::MAX,
        max_rational_input_bits: usize::MAX,
        max_total_rational_input_storage_bits: usize::MAX,
        max_total_rational_retained_clone_bits: usize::MAX,
        max_rational_operations: usize::MAX,
        max_rational_intermediate_bits: usize::MAX,
        max_rational_gcd_fallback_calls: usize::MAX,
        max_rational_gcd_fallback_input_bits: usize::MAX,
        max_rational_allocations: usize::MAX,
        max_rational_allocation_bits: usize::MAX,
        max_total_rational_allocation_bits: usize::MAX,
        max_rational_output_bits: usize::MAX,
        max_total_rational_output_bits: usize::MAX,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RestBoundaryVertex {
    id: VertexId,
    point: ExactPoint3,
}

#[cfg(test)]
fn triangulate_rest_boundary(
    boundary: &[RestBoundaryVertex],
    max_boundary_vertices: usize,
    max_triangles: usize,
    max_work: usize,
) -> Result<Vec<[usize; 3]>, ZeroThicknessAnalysisError> {
    let limits = unlimited_rational_limits();
    let mut meter = RationalWorkMeter::unlimited(&limits);
    triangulate_rest_boundary_metered(
        boundary,
        max_boundary_vertices,
        max_triangles,
        max_work,
        &mut meter,
    )
}

fn triangulate_rest_boundary_metered(
    boundary: &[RestBoundaryVertex],
    max_boundary_vertices: usize,
    max_triangles: usize,
    max_work: usize,
    meter: &mut RationalWorkMeter<'_>,
) -> Result<Vec<[usize; 3]>, ZeroThicknessAnalysisError> {
    if boundary.len() > max_boundary_vertices {
        return Err(ZeroThicknessAnalysisError::ResourceLimitExceeded);
    }
    if boundary.len() < 3 {
        return Err(ZeroThicknessAnalysisError::EvidenceUnavailable);
    }
    if estimated_triangulation_work(boundary.len())? > max_work {
        return Err(ZeroThicknessAnalysisError::ResourceLimitExceeded);
    }
    for index in 0..boundary.len() {
        if boundary[..index]
            .iter()
            .any(|previous| previous.id == boundary[index].id)
        {
            return Err(ZeroThicknessAnalysisError::EvidenceUnavailable);
        }
        for previous in &boundary[..index] {
            if previous
                .point
                .equal_metered(&boundary[index].point, meter)?
            {
                return Err(ZeroThicknessAnalysisError::EvidenceUnavailable);
            }
        }
    }
    if !is_simple_rest_boundary(boundary, meter)? {
        return Err(ZeroThicknessAnalysisError::EvidenceUnavailable);
    }

    let mut active = Vec::new();
    active
        .try_reserve_exact(boundary.len())
        .map_err(|_| ZeroThicknessAnalysisError::ResourceLimitExceeded)?;
    active.extend(0..boundary.len());
    remove_collinear_rest_vertices(boundary, &mut active, meter)?;
    if active.len() < 3 || !simplified_boundary_covers_original_edges(boundary, &active, meter)? {
        return Err(ZeroThicknessAnalysisError::EvidenceUnavailable);
    }
    let mut simplified = Vec::new();
    simplified
        .try_reserve_exact(active.len())
        .map_err(|_| ZeroThicknessAnalysisError::ResourceLimitExceeded)?;
    simplified.extend(active.iter().copied());

    let polygon_area = indexed_polygon_double_area(boundary, &active, meter)?;
    if polygon_area.is_zero() {
        return Err(ZeroThicknessAnalysisError::EvidenceUnavailable);
    }
    let positive_orientation = polygon_area.is_positive();
    let expected_triangles = active
        .len()
        .checked_sub(2)
        .ok_or(ZeroThicknessAnalysisError::EvidenceUnavailable)?;
    if expected_triangles > max_triangles {
        return Err(ZeroThicknessAnalysisError::ResourceLimitExceeded);
    }
    let mut triangles = Vec::new();
    triangles
        .try_reserve_exact(expected_triangles)
        .map_err(|_| ZeroThicknessAnalysisError::ResourceLimitExceeded)?;

    while active.len() > 3 {
        let mut ear_position: Option<usize> = None;
        for position in 0..active.len() {
            if is_exact_ear(boundary, &active, position, positive_orientation, meter)?
                && ear_position.is_none_or(|selected| {
                    boundary[active[position]].id.canonical_bytes()
                        < boundary[active[selected]].id.canonical_bytes()
                })
            {
                ear_position = Some(position);
            }
        }
        let ear_position = ear_position.ok_or(ZeroThicknessAnalysisError::EvidenceUnavailable)?;
        let previous = active[(ear_position + active.len() - 1) % active.len()];
        let current = active[ear_position];
        let next = active[(ear_position + 1) % active.len()];
        triangles.push(canonical_triangle_cycle(
            boundary,
            [previous, current, next],
        ));
        active.remove(ear_position);
    }
    triangles.push(canonical_triangle_cycle(
        boundary,
        [active[0], active[1], active[2]],
    ));
    triangles.sort_unstable_by_key(|triangle| {
        triangle.map(|index| boundary[index].id.canonical_bytes())
    });

    if triangles.len() != expected_triangles
        || !triangulation_covers_boundary(
            boundary,
            &simplified,
            &triangles,
            &polygon_area,
            positive_orientation,
            meter,
        )?
    {
        return Err(ZeroThicknessAnalysisError::EvidenceUnavailable);
    }
    Ok(triangles)
}

fn is_simple_rest_boundary(
    boundary: &[RestBoundaryVertex],
    meter: &mut RationalWorkMeter<'_>,
) -> Result<bool, ZeroThicknessAnalysisError> {
    for first in 0..boundary.len() {
        let first_next = (first + 1) % boundary.len();
        for second in (first + 1)..boundary.len() {
            let second_next = (second + 1) % boundary.len();
            if first == second
                || first == second_next
                || first_next == second
                || first_next == second_next
            {
                continue;
            }
            if exact_segments_intersect(
                &boundary[first].point,
                &boundary[first_next].point,
                &boundary[second].point,
                &boundary[second_next].point,
                meter,
            )? {
                return Ok(false);
            }
        }
    }
    Ok(!rest_polygon_double_area(boundary, meter)?.is_zero())
}

fn remove_collinear_rest_vertices(
    boundary: &[RestBoundaryVertex],
    active: &mut Vec<usize>,
    meter: &mut RationalWorkMeter<'_>,
) -> Result<(), ZeroThicknessAnalysisError> {
    loop {
        let mut saw_collinear = false;
        let mut selected: Option<usize> = None;
        for position in 0..active.len() {
            let previous = active[(position + active.len() - 1) % active.len()];
            let current = active[position];
            let next = active[(position + 1) % active.len()];
            if !exact_orientation(
                &boundary[previous].point,
                &boundary[current].point,
                &boundary[next].point,
                meter,
            )?
            .is_zero()
            {
                continue;
            }
            saw_collinear = true;
            if !exact_point_on_segment(
                &boundary[current].point,
                &boundary[previous].point,
                &boundary[next].point,
                meter,
            )? {
                continue;
            }
            if selected.is_none_or(|candidate| {
                boundary[current].id.canonical_bytes()
                    < boundary[active[candidate]].id.canonical_bytes()
            }) {
                selected = Some(position);
            }
        }
        if !saw_collinear {
            return Ok(());
        }
        let Some(position) = selected else {
            return Err(ZeroThicknessAnalysisError::EvidenceUnavailable);
        };
        active.remove(position);
        if active.len() < 3 {
            return Err(ZeroThicknessAnalysisError::EvidenceUnavailable);
        }
    }
}

fn simplified_boundary_covers_original_edges(
    boundary: &[RestBoundaryVertex],
    simplified: &[usize],
    meter: &mut RationalWorkMeter<'_>,
) -> Result<bool, ZeroThicknessAnalysisError> {
    for index in 0..boundary.len() {
        let next = (index + 1) % boundary.len();
        let mut covered = false;
        for edge in 0..simplified.len() {
            let start = simplified[edge];
            let end = simplified[(edge + 1) % simplified.len()];
            if exact_point_on_segment(
                &boundary[index].point,
                &boundary[start].point,
                &boundary[end].point,
                meter,
            )? && exact_point_on_segment(
                &boundary[next].point,
                &boundary[start].point,
                &boundary[end].point,
                meter,
            )? {
                covered = true;
                break;
            }
        }
        if !covered {
            return Ok(false);
        }
    }
    Ok(true)
}

fn is_exact_ear(
    boundary: &[RestBoundaryVertex],
    active: &[usize],
    position: usize,
    positive_orientation: bool,
    meter: &mut RationalWorkMeter<'_>,
) -> Result<bool, ZeroThicknessAnalysisError> {
    let previous = active[(position + active.len() - 1) % active.len()];
    let current = active[position];
    let next = active[(position + 1) % active.len()];
    let orientation = exact_orientation(
        &boundary[previous].point,
        &boundary[current].point,
        &boundary[next].point,
        meter,
    )?;
    if orientation.is_zero() || orientation.is_positive() != positive_orientation {
        return Ok(false);
    }
    for candidate in active.iter().copied() {
        if candidate != previous
            && candidate != current
            && candidate != next
            && exact_point_in_or_on_triangle(
                &boundary[candidate].point,
                &boundary[previous].point,
                &boundary[current].point,
                &boundary[next].point,
                positive_orientation,
                meter,
            )?
        {
            return Ok(false);
        }
    }
    for edge in 0..active.len() {
        let start = active[edge];
        let end = active[(edge + 1) % active.len()];
        if start == previous || start == next || end == previous || end == next {
            continue;
        }
        if exact_segments_intersect(
            &boundary[previous].point,
            &boundary[next].point,
            &boundary[start].point,
            &boundary[end].point,
            meter,
        )? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn canonical_triangle_cycle(boundary: &[RestBoundaryVertex], triangle: [usize; 3]) -> [usize; 3] {
    let start = (0..3)
        .min_by_key(|position| boundary[triangle[*position]].id.canonical_bytes())
        .unwrap_or(0);
    [
        triangle[start],
        triangle[(start + 1) % 3],
        triangle[(start + 2) % 3],
    ]
}

fn triangulation_covers_boundary(
    boundary: &[RestBoundaryVertex],
    simplified: &[usize],
    triangles: &[[usize; 3]],
    polygon_area: &BigRational,
    positive_orientation: bool,
    meter: &mut RationalWorkMeter<'_>,
) -> Result<bool, ZeroThicknessAnalysisError> {
    let mut area = meter.zero()?;
    let Some(edge_capacity) = triangles.len().checked_mul(3) else {
        return Err(ZeroThicknessAnalysisError::ResourceLimitExceeded);
    };
    let mut triangle_edges = Vec::new();
    triangle_edges
        .try_reserve_exact(edge_capacity)
        .map_err(|_| ZeroThicknessAnalysisError::ResourceLimitExceeded)?;
    let mut boundary_edges = Vec::new();
    boundary_edges
        .try_reserve_exact(simplified.len())
        .map_err(|_| ZeroThicknessAnalysisError::ResourceLimitExceeded)?;
    for edge in 0..simplified.len() {
        boundary_edges.push(canonical_index_edge(
            simplified[edge],
            simplified[(edge + 1) % simplified.len()],
        ));
    }
    boundary_edges.sort_unstable();
    for triangle in triangles {
        let triangle_area = exact_orientation(
            &boundary[triangle[0]].point,
            &boundary[triangle[1]].point,
            &boundary[triangle[2]].point,
            meter,
        )?;
        if triangle_area.is_zero() || triangle_area.is_positive() != positive_orientation {
            return Ok(false);
        }
        area = meter.add(&area, &triangle_area)?;
        for edge in 0..3 {
            triangle_edges.push(canonical_index_edge(
                triangle[edge],
                triangle[(edge + 1) % 3],
            ));
        }
    }
    triangle_edges.sort_unstable();
    let mut position = 0;
    while position < triangle_edges.len() {
        let edge = triangle_edges[position];
        let mut end = position + 1;
        while end < triangle_edges.len() && triangle_edges[end] == edge {
            end += 1;
        }
        let expected = if boundary_edges.binary_search(&edge).is_ok() {
            1
        } else {
            2
        };
        if end - position != expected {
            return Ok(false);
        }
        position = end;
    }
    meter.equal(&area, polygon_area)
}

const fn canonical_index_edge(first: usize, second: usize) -> (usize, usize) {
    if first <= second {
        (first, second)
    } else {
        (second, first)
    }
}

fn indexed_polygon_double_area(
    boundary: &[RestBoundaryVertex],
    indices: &[usize],
    meter: &mut RationalWorkMeter<'_>,
) -> Result<BigRational, ZeroThicknessAnalysisError> {
    let mut area = meter.zero()?;
    for index in 0..indices.len() {
        let current = &boundary[indices[index]].point;
        let next = &boundary[indices[(index + 1) % indices.len()]].point;
        let first = meter.multiply(current.coordinate(0), next.coordinate(2))?;
        let second = meter.multiply(current.coordinate(2), next.coordinate(0))?;
        let term = meter.subtract(&first, &second)?;
        area = meter.add(&area, &term)?;
    }
    Ok(area)
}

fn rest_polygon_double_area(
    boundary: &[RestBoundaryVertex],
    meter: &mut RationalWorkMeter<'_>,
) -> Result<BigRational, ZeroThicknessAnalysisError> {
    let mut area = meter.zero()?;
    for index in 0..boundary.len() {
        let current = &boundary[index].point;
        let next = &boundary[(index + 1) % boundary.len()].point;
        let first = meter.multiply(current.coordinate(0), next.coordinate(2))?;
        let second = meter.multiply(current.coordinate(2), next.coordinate(0))?;
        let term = meter.subtract(&first, &second)?;
        area = meter.add(&area, &term)?;
    }
    Ok(area)
}

fn estimated_triangulation_work(
    boundary_vertices: usize,
) -> Result<usize, ZeroThicknessAnalysisError> {
    let square = boundary_vertices
        .checked_mul(boundary_vertices)
        .ok_or(ZeroThicknessAnalysisError::ResourceLimitExceeded)?;
    let cube = square
        .checked_mul(boundary_vertices)
        .ok_or(ZeroThicknessAnalysisError::ResourceLimitExceeded)?;
    cube.checked_mul(4)
        .and_then(|value| {
            square
                .checked_mul(12)
                .and_then(|extra| value.checked_add(extra))
        })
        .and_then(|value| {
            boundary_vertices
                .checked_mul(32)
                .and_then(|extra| value.checked_add(extra))
        })
        .ok_or(ZeroThicknessAnalysisError::ResourceLimitExceeded)
}

fn exact_orientation(
    first: &ExactPoint3,
    second: &ExactPoint3,
    third: &ExactPoint3,
    meter: &mut RationalWorkMeter<'_>,
) -> Result<BigRational, ZeroThicknessAnalysisError> {
    projected_line_value(first, second, third, 0, 2, meter)
}

fn exact_point_on_segment(
    point: &ExactPoint3,
    start: &ExactPoint3,
    end: &ExactPoint3,
    meter: &mut RationalWorkMeter<'_>,
) -> Result<bool, ZeroThicknessAnalysisError> {
    if !exact_orientation(start, end, point, meter)?.is_zero() {
        return Ok(false);
    }
    for axis in 0..3 {
        let (minimum, maximum) =
            if meter.compare(start.coordinate(axis), end.coordinate(axis))? != Ordering::Greater {
                (start.coordinate(axis), end.coordinate(axis))
            } else {
                (end.coordinate(axis), start.coordinate(axis))
            };
        if meter.compare(minimum, point.coordinate(axis))? == Ordering::Greater
            || meter.compare(point.coordinate(axis), maximum)? == Ordering::Greater
        {
            return Ok(false);
        }
    }
    Ok(true)
}

fn exact_segments_intersect(
    first_start: &ExactPoint3,
    first_end: &ExactPoint3,
    second_start: &ExactPoint3,
    second_end: &ExactPoint3,
    meter: &mut RationalWorkMeter<'_>,
) -> Result<bool, ZeroThicknessAnalysisError> {
    let first_first = exact_orientation(first_start, first_end, second_start, meter)?;
    let first_second = exact_orientation(first_start, first_end, second_end, meter)?;
    let second_first = exact_orientation(second_start, second_end, first_start, meter)?;
    let second_second = exact_orientation(second_start, second_end, first_end, meter)?;
    Ok((first_first.is_zero()
        && exact_point_on_segment(second_start, first_start, first_end, meter)?)
        || (first_second.is_zero()
            && exact_point_on_segment(second_end, first_start, first_end, meter)?)
        || (second_first.is_zero()
            && exact_point_on_segment(first_start, second_start, second_end, meter)?)
        || (second_second.is_zero()
            && exact_point_on_segment(first_end, second_start, second_end, meter)?)
        || (first_first.is_positive() != first_second.is_positive()
            && second_first.is_positive() != second_second.is_positive()))
}

fn exact_point_in_or_on_triangle(
    point: &ExactPoint3,
    first: &ExactPoint3,
    second: &ExactPoint3,
    third: &ExactPoint3,
    positive_orientation: bool,
    meter: &mut RationalWorkMeter<'_>,
) -> Result<bool, ZeroThicknessAnalysisError> {
    let orientations = [
        exact_orientation(first, second, point, meter)?,
        exact_orientation(second, third, point, meter)?,
        exact_orientation(third, first, point, meter)?,
    ];
    Ok(orientations.iter().all(|orientation| {
        orientation.is_zero() || orientation.is_positive() == positive_orientation
    }))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ZeroThicknessGeometryLimits {
    pub max_boundary_vertices_per_face: usize,
    pub max_total_boundary_vertices: usize,
    pub max_triangles_per_face: usize,
    pub max_total_triangles: usize,
    pub max_triangulation_work_per_face: usize,
    pub max_total_triangulation_work: usize,
    pub max_registry_authentication_work: usize,
    pub max_triangle_pairs_per_face_pair: usize,
    pub max_total_triangle_pairs: usize,
    pub max_boundary_relation_work_per_face_pair: usize,
    pub max_total_boundary_relation_work: usize,
    pub max_rational_input_bits: usize,
    pub max_total_rational_input_storage_bits: usize,
    pub max_total_rational_retained_clone_bits: usize,
    pub max_rational_operations: usize,
    pub max_rational_intermediate_bits: usize,
    pub max_rational_gcd_fallback_calls: usize,
    pub max_rational_gcd_fallback_input_bits: usize,
    pub max_rational_allocations: usize,
    pub max_rational_allocation_bits: usize,
    pub max_total_rational_allocation_bits: usize,
    pub max_rational_output_bits: usize,
    pub max_total_rational_output_bits: usize,
}

#[derive(Debug, Clone, PartialEq)]
struct AuthenticatedBoundaryVertex {
    id: VertexId,
    rest: Point3,
    current: ExactPoint3,
}

#[derive(Debug, Clone, PartialEq)]
struct AuthenticatedFace {
    id: FaceId,
    boundary: Vec<AuthenticatedBoundaryVertex>,
    edges: Vec<EdgeId>,
    triangles: Vec<[ExactPoint3; 3]>,
    material_normal: ExactVector3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ZeroThicknessAnalysisWork {
    pub(super) total_triangles: usize,
    pub(super) registry_authentication_work: usize,
    pub(super) total_triangle_pairs: usize,
    pub(super) total_boundary_relation_work: usize,
    pub(super) total_rational_input_storage_bits: usize,
    pub(super) total_rational_retained_clone_bits: usize,
    pub(super) rational_operations: usize,
    pub(super) rational_gcd_fallback_calls: usize,
    pub(super) rational_gcd_fallback_input_bits: usize,
    pub(super) rational_allocations: usize,
    pub(super) total_rational_allocation_bits: usize,
    pub(super) total_rational_output_bits: usize,
}

#[derive(Debug)]
pub(super) struct AuthenticatedZeroThicknessPose<'a> {
    pose: &'a MaterialTreePose,
    faces: Vec<AuthenticatedFace>,
    pair_dispatches: Vec<Result<PairDispatch, ZeroThicknessAnalysisError>>,
    rational_work: RationalWork,
    total_triangles: usize,
    registry_authentication_work: usize,
    total_triangle_pairs: usize,
    total_boundary_relation_work: usize,
}

impl AuthenticatedZeroThicknessPose<'_> {
    pub(super) fn is_for_pose(&self, pose: &MaterialTreePose) -> bool {
        self.pose.same_instance(pose)
    }

    pub(super) fn face_id(&self, face_index: usize) -> Option<FaceId> {
        self.faces.get(face_index).map(|face| face.id)
    }

    pub(super) fn face_boundary_vertex_count(&self, face_index: usize) -> Option<usize> {
        self.faces.get(face_index).map(|face| face.boundary.len())
    }

    pub(super) const fn face_count(&self) -> usize {
        self.faces.len()
    }

    pub(super) fn dispatch_pair(
        &self,
        first_face_index: usize,
        second_face_index: usize,
    ) -> Result<PairDispatch, ZeroThicknessAnalysisError> {
        if first_face_index >= second_face_index || second_face_index >= self.faces.len() {
            return Err(ZeroThicknessAnalysisError::EvidenceUnavailable);
        }
        let index =
            canonical_unordered_pair_index(self.faces.len(), first_face_index, second_face_index)?;
        self.pair_dispatches
            .get(index)
            .copied()
            .ok_or(ZeroThicknessAnalysisError::EvidenceUnavailable)?
    }

    pub(super) const fn total_triangle_pairs(&self) -> usize {
        self.total_triangle_pairs
    }

    pub(super) const fn work(&self) -> ZeroThicknessAnalysisWork {
        ZeroThicknessAnalysisWork {
            total_triangles: self.total_triangles,
            registry_authentication_work: self.registry_authentication_work,
            total_triangle_pairs: self.total_triangle_pairs,
            total_boundary_relation_work: self.total_boundary_relation_work,
            total_rational_input_storage_bits: self.rational_work.total_input_storage_bits,
            total_rational_retained_clone_bits: self.rational_work.total_retained_clone_bits,
            rational_operations: self.rational_work.operations,
            rational_gcd_fallback_calls: self.rational_work.gcd_fallback_calls,
            rational_gcd_fallback_input_bits: self.rational_work.gcd_fallback_input_bits,
            rational_allocations: self.rational_work.rational_allocations,
            total_rational_allocation_bits: self.rational_work.total_rational_allocation_bits,
            total_rational_output_bits: self.rational_work.total_output_bits,
        }
    }

    #[cfg(test)]
    fn rational_work(&self) -> &RationalWork {
        &self.rational_work
    }
}

pub(super) fn prepare_authenticated_zero_thickness_pose(
    pose: &MaterialTreePose,
    limits: ZeroThicknessGeometryLimits,
) -> Result<AuthenticatedZeroThicknessPose<'_>, ZeroThicknessAnalysisError> {
    let mut meter = RationalWorkMeter::new(&limits);
    let face_ids = pose.face_ids();
    if face_ids.is_empty()
        || !face_ids
            .windows(2)
            .all(|pair| pair[0].canonical_bytes() < pair[1].canonical_bytes())
    {
        return Err(ZeroThicknessAnalysisError::EvidenceUnavailable);
    }
    let mut faces = Vec::new();
    faces
        .try_reserve_exact(face_ids.len())
        .map_err(|_| ZeroThicknessAnalysisError::ResourceLimitExceeded)?;
    let mut total_boundary_vertices = 0_usize;
    let mut total_triangles = 0_usize;
    let mut total_triangulation_work = 0_usize;

    for face_id in face_ids.iter().copied() {
        let boundary_view = pose
            .face_boundary(face_id)
            .filter(|boundary| pose.owns_face_boundary(*boundary))
            .ok_or(ZeroThicknessAnalysisError::EvidenceUnavailable)?;
        if boundary_view.face() != face_id
            || boundary_view.vertices().len() != boundary_view.edges().len()
            || boundary_view.vertices().len() < 3
        {
            return Err(ZeroThicknessAnalysisError::EvidenceUnavailable);
        }
        let boundary_count = boundary_view.vertices().len();
        if boundary_count > limits.max_boundary_vertices_per_face {
            return Err(ZeroThicknessAnalysisError::ResourceLimitExceeded);
        }
        let triangulation_work = estimated_triangulation_work(boundary_count)?;
        if triangulation_work > limits.max_triangulation_work_per_face {
            return Err(ZeroThicknessAnalysisError::ResourceLimitExceeded);
        }
        total_triangulation_work = total_triangulation_work
            .checked_add(triangulation_work)
            .ok_or(ZeroThicknessAnalysisError::ResourceLimitExceeded)?;
        if total_triangulation_work > limits.max_total_triangulation_work {
            return Err(ZeroThicknessAnalysisError::ResourceLimitExceeded);
        }
        total_boundary_vertices = total_boundary_vertices
            .checked_add(boundary_count)
            .ok_or(ZeroThicknessAnalysisError::ResourceLimitExceeded)?;
        if total_boundary_vertices > limits.max_total_boundary_vertices {
            return Err(ZeroThicknessAnalysisError::ResourceLimitExceeded);
        }
        if has_duplicate_ids(boundary_view.vertices()) || has_duplicate_ids(boundary_view.edges()) {
            return Err(ZeroThicknessAnalysisError::EvidenceUnavailable);
        }
        let transform = pose
            .face_transform(face_id)
            .ok_or(ZeroThicknessAnalysisError::EvidenceUnavailable)?;
        let exact_transform = ExactAffineTransform::from_transform_metered(transform, &mut meter)?;
        let material_normal = exact_transform.transformed_local_y(&mut meter)?;
        if material_normal.is_zero() {
            return Err(ZeroThicknessAnalysisError::EvidenceUnavailable);
        }
        let mut rest_boundary = Vec::new();
        rest_boundary
            .try_reserve_exact(boundary_count)
            .map_err(|_| ZeroThicknessAnalysisError::ResourceLimitExceeded)?;
        let mut boundary = Vec::new();
        boundary
            .try_reserve_exact(boundary_count)
            .map_err(|_| ZeroThicknessAnalysisError::ResourceLimitExceeded)?;
        for vertex in boundary_view.vertices().iter().copied() {
            let rest = pose
                .vertex_position(vertex)
                .filter(|point| point.y() == 0.0)
                .ok_or(ZeroThicknessAnalysisError::EvidenceUnavailable)?;
            // Applying the binary64 matrix in exact rational arithmetic keeps
            // every vertex of one rigid material face constructively
            // coplanar. Rounding each transformed vertex back to binary64
            // independently would destroy that invariant for four or more
            // vertices and could create false crossing evidence.
            let exact_rest = ExactPoint3::from_input_point(rest, &mut meter)?;
            let current = exact_transform.apply_point_metered(&exact_rest, &mut meter)?;
            current.observe_output(&mut meter)?;
            rest_boundary.push(RestBoundaryVertex {
                id: vertex,
                point: exact_rest,
            });
            boundary.push(AuthenticatedBoundaryVertex {
                id: vertex,
                rest,
                current,
            });
        }
        let triangle_indices = triangulate_rest_boundary_metered(
            &rest_boundary,
            limits.max_boundary_vertices_per_face,
            limits.max_triangles_per_face,
            limits.max_triangulation_work_per_face,
            &mut meter,
        )?;
        total_triangles = total_triangles
            .checked_add(triangle_indices.len())
            .ok_or(ZeroThicknessAnalysisError::ResourceLimitExceeded)?;
        if total_triangles > limits.max_total_triangles {
            return Err(ZeroThicknessAnalysisError::ResourceLimitExceeded);
        }
        let mut triangles = Vec::new();
        triangles
            .try_reserve_exact(triangle_indices.len())
            .map_err(|_| ZeroThicknessAnalysisError::ResourceLimitExceeded)?;
        for triangle in triangle_indices {
            let points = [
                boundary[triangle[0]].current.clone_retained(&mut meter)?,
                boundary[triangle[1]].current.clone_retained(&mut meter)?,
                boundary[triangle[2]].current.clone_retained(&mut meter)?,
            ];
            let triangle = ExactTriangle::from_exact_points_metered(points, &mut meter)?;
            if triangle.normal.is_zero() {
                return Err(ZeroThicknessAnalysisError::EvidenceUnavailable);
            }
            triangles.push(triangle.points);
        }
        let mut edges = Vec::new();
        edges
            .try_reserve_exact(boundary_view.edges().len())
            .map_err(|_| ZeroThicknessAnalysisError::ResourceLimitExceeded)?;
        edges.extend_from_slice(boundary_view.edges());
        faces.push(AuthenticatedFace {
            id: face_id,
            boundary,
            edges,
            triangles,
            material_normal,
        });
    }

    let registry_authentication_work = estimated_registry_authentication_work(
        total_boundary_vertices,
        pose.hinges().len(),
        faces.len(),
    )?;
    if registry_authentication_work > limits.max_registry_authentication_work {
        return Err(ZeroThicknessAnalysisError::ResourceLimitExceeded);
    }
    validate_authenticated_hinge_and_edge_registries(pose, &faces)?;
    let mut total_triangle_pairs = 0_usize;
    let mut total_boundary_relation_work = 0_usize;
    for first in 0..faces.len() {
        for second in (first + 1)..faces.len() {
            let pair_count = faces[first]
                .triangles
                .len()
                .checked_mul(faces[second].triangles.len())
                .ok_or(ZeroThicknessAnalysisError::ResourceLimitExceeded)?;
            if pair_count > limits.max_triangle_pairs_per_face_pair {
                return Err(ZeroThicknessAnalysisError::ResourceLimitExceeded);
            }
            total_triangle_pairs = total_triangle_pairs
                .checked_add(pair_count)
                .ok_or(ZeroThicknessAnalysisError::ResourceLimitExceeded)?;
            if total_triangle_pairs > limits.max_total_triangle_pairs {
                return Err(ZeroThicknessAnalysisError::ResourceLimitExceeded);
            }
            let boundary_relation_work = estimated_boundary_relation_work(
                pair_count,
                faces[first].triangles.len(),
                faces[second].triangles.len(),
                faces[first].boundary.len(),
                faces[second].boundary.len(),
                pose.hinges().len(),
            )?;
            if boundary_relation_work > limits.max_boundary_relation_work_per_face_pair {
                return Err(ZeroThicknessAnalysisError::ResourceLimitExceeded);
            }
            total_boundary_relation_work = total_boundary_relation_work
                .checked_add(boundary_relation_work)
                .ok_or(ZeroThicknessAnalysisError::ResourceLimitExceeded)?;
            if total_boundary_relation_work > limits.max_total_boundary_relation_work {
                return Err(ZeroThicknessAnalysisError::ResourceLimitExceeded);
            }
        }
    }

    let pair_capacity = faces
        .len()
        .checked_mul(faces.len().saturating_sub(1))
        .and_then(|value| value.checked_div(2))
        .ok_or(ZeroThicknessAnalysisError::ResourceLimitExceeded)?;
    let mut pair_dispatches = Vec::new();
    pair_dispatches
        .try_reserve_exact(pair_capacity)
        .map_err(|_| ZeroThicknessAnalysisError::ResourceLimitExceeded)?;
    for first in 0..faces.len() {
        for second in (first + 1)..faces.len() {
            let result = authenticate_face_pair_topology_metered(
                pose,
                &faces[first],
                &faces[second],
                &mut meter,
            )
            .and_then(|topology| {
                aggregate_authenticated_face_pair_metered(
                    &faces[first],
                    &faces[second],
                    &topology,
                    limits.max_triangle_pairs_per_face_pair,
                    limits.max_boundary_relation_work_per_face_pair,
                    pose.hinges().len(),
                    &mut meter,
                )
            });
            if matches!(
                result,
                Err(ZeroThicknessAnalysisError::ResourceLimitExceeded)
            ) {
                return Err(ZeroThicknessAnalysisError::ResourceLimitExceeded);
            }
            pair_dispatches.push(result);
        }
    }
    if pair_dispatches.len() != pair_capacity {
        return Err(ZeroThicknessAnalysisError::EvidenceUnavailable);
    }
    let rational_work = meter.work;
    Ok(AuthenticatedZeroThicknessPose {
        pose,
        faces,
        pair_dispatches,
        rational_work,
        total_triangles,
        registry_authentication_work,
        total_triangle_pairs,
        total_boundary_relation_work,
    })
}

fn canonical_unordered_pair_index(
    face_count: usize,
    first: usize,
    second: usize,
) -> Result<usize, ZeroThicknessAnalysisError> {
    if first >= second || second >= face_count {
        return Err(ZeroThicknessAnalysisError::EvidenceUnavailable);
    }
    let before = first
        .checked_mul(
            face_count
                .checked_mul(2)
                .and_then(|value| value.checked_sub(first))
                .and_then(|value| value.checked_sub(1))
                .ok_or(ZeroThicknessAnalysisError::ResourceLimitExceeded)?,
        )
        .and_then(|value| value.checked_div(2))
        .ok_or(ZeroThicknessAnalysisError::ResourceLimitExceeded)?;
    before
        .checked_add(
            second
                .checked_sub(first)
                .and_then(|value| value.checked_sub(1))
                .ok_or(ZeroThicknessAnalysisError::EvidenceUnavailable)?,
        )
        .ok_or(ZeroThicknessAnalysisError::ResourceLimitExceeded)
}

fn estimated_boundary_relation_work(
    triangle_pairs: usize,
    first_triangles: usize,
    second_triangles: usize,
    first_boundary_vertices: usize,
    second_boundary_vertices: usize,
    hinges: usize,
) -> Result<usize, ZeroThicknessAnalysisError> {
    fn sort_scan_work(vertices: usize) -> Option<usize> {
        let log_factor = usize::BITS.checked_sub(vertices.max(1).leading_zeros())? as usize;
        vertices.checked_mul(log_factor.checked_add(1)?)
    }
    let classification_work = sort_scan_work(first_boundary_vertices)
        .and_then(|first| {
            sort_scan_work(second_boundary_vertices).and_then(|second| first.checked_add(second))
        })
        .and_then(|per_pair| triangle_pairs.checked_mul(per_pair));
    let topology_work = first_boundary_vertices
        .checked_mul(second_boundary_vertices)
        .and_then(|shared_vertices| shared_vertices.checked_mul(2))
        .and_then(|shared_features| shared_features.checked_add(hinges));
    let face_line_work = estimated_face_line_intersection_work(
        first_triangles,
        second_triangles,
        first_boundary_vertices,
        second_boundary_vertices,
    )?;
    classification_work
        .and_then(|classification| {
            topology_work.and_then(|topology| classification.checked_add(topology))
        })
        .and_then(|work| work.checked_add(face_line_work))
        .ok_or(ZeroThicknessAnalysisError::ResourceLimitExceeded)
}

fn estimated_face_line_intersection_work(
    first_triangles: usize,
    second_triangles: usize,
    first_boundary_vertices: usize,
    second_boundary_vertices: usize,
) -> Result<usize, ZeroThicknessAnalysisError> {
    fn sort_work(values: usize) -> Option<usize> {
        let log_factor = usize::BITS.checked_sub(values.max(1).leading_zeros())? as usize;
        values
            .checked_mul(log_factor.checked_add(1)?)
            .and_then(|work| work.checked_mul(4))
    }

    let interval_count = first_triangles
        .checked_add(second_triangles)
        .and_then(|triangles| triangles.checked_add(first_boundary_vertices))
        .and_then(|work| work.checked_add(second_boundary_vertices))
        .ok_or(ZeroThicknessAnalysisError::ResourceLimitExceeded)?;
    let event_count = interval_count
        .checked_mul(2)
        .ok_or(ZeroThicknessAnalysisError::ResourceLimitExceeded)?;
    let linear_work = interval_count
        .checked_mul(64)
        .ok_or(ZeroThicknessAnalysisError::ResourceLimitExceeded)?;
    [
        first_triangles,
        second_triangles,
        first_boundary_vertices,
        second_boundary_vertices,
        event_count,
    ]
    .into_iter()
    .try_fold(linear_work, |work, values| {
        sort_work(values)
            .and_then(|sort| work.checked_add(sort))
            .ok_or(ZeroThicknessAnalysisError::ResourceLimitExceeded)
    })
}

fn estimated_registry_authentication_work(
    total_boundary_vertices: usize,
    hinges: usize,
    faces: usize,
) -> Result<usize, ZeroThicknessAnalysisError> {
    // Conservative upper bound for the loops below:
    // - B²: one full edge-occurrence scan for every boundary edge;
    // - 2BH: one full occurrence scan for every hinge plus the hinge lookup
    //   repeated for every boundary edge;
    // - H²: worst-case hinge-transform registry lookup;
    // - 2HF: both face-membership scans for every hinge;
    // - linear terms: canonical-order, occurrence and branch overhead.
    // Keep this formula and its fixed 10/2/3 = 201 contract test together.
    let boundary_square = total_boundary_vertices
        .checked_mul(total_boundary_vertices)
        .ok_or(ZeroThicknessAnalysisError::ResourceLimitExceeded)?;
    let boundary_hinge_twice = total_boundary_vertices
        .checked_mul(hinges)
        .and_then(|work| work.checked_mul(2))
        .ok_or(ZeroThicknessAnalysisError::ResourceLimitExceeded)?;
    let hinge_square = hinges
        .checked_mul(hinges)
        .ok_or(ZeroThicknessAnalysisError::ResourceLimitExceeded)?;
    let hinge_face_twice = hinges
        .checked_mul(faces)
        .and_then(|work| work.checked_mul(2))
        .ok_or(ZeroThicknessAnalysisError::ResourceLimitExceeded)?;
    let hinge_overhead = hinges
        .checked_mul(16)
        .ok_or(ZeroThicknessAnalysisError::ResourceLimitExceeded)?;
    boundary_square
        .checked_add(boundary_hinge_twice)
        .and_then(|work| work.checked_add(hinge_square))
        .and_then(|work| work.checked_add(hinge_face_twice))
        .and_then(|work| work.checked_add(hinge_overhead))
        .and_then(|work| work.checked_add(total_boundary_vertices))
        .and_then(|work| work.checked_add(faces))
        .ok_or(ZeroThicknessAnalysisError::ResourceLimitExceeded)
}

fn has_duplicate_ids<T: Copy + Eq>(values: &[T]) -> bool {
    (0..values.len()).any(|index| values[..index].contains(&values[index]))
}

fn validate_authenticated_hinge_and_edge_registries(
    pose: &MaterialTreePose,
    faces: &[AuthenticatedFace],
) -> Result<(), ZeroThicknessAnalysisError> {
    if !pose
        .hinges()
        .windows(2)
        .all(|pair| pair[0].edge().canonical_bytes() < pair[1].edge().canonical_bytes())
    {
        return Err(ZeroThicknessAnalysisError::EvidenceUnavailable);
    }
    for hinge in pose.hinges() {
        if hinge.left_face() == hinge.right_face()
            || faces.iter().all(|face| face.id != hinge.left_face())
            || faces.iter().all(|face| face.id != hinge.right_face())
            || pose.hinge_parent_transform(hinge.edge()).is_none()
        {
            return Err(ZeroThicknessAnalysisError::EvidenceUnavailable);
        }
        let occurrences = edge_occurrences(faces, hinge.edge())?;
        if occurrences.len() != 2 {
            return Err(ZeroThicknessAnalysisError::EvidenceUnavailable);
        }
        let left = occurrences
            .iter()
            .find(|occurrence| occurrence.face == hinge.left_face())
            .ok_or(ZeroThicknessAnalysisError::EvidenceUnavailable)?;
        let right = occurrences
            .iter()
            .find(|occurrence| occurrence.face == hinge.right_face())
            .ok_or(ZeroThicknessAnalysisError::EvidenceUnavailable)?;
        if left.start_id != right.end_id
            || left.end_id != right.start_id
            || left.start_rest != hinge.start()
            || left.end_rest != hinge.end()
            || right.start_rest != hinge.end()
            || right.end_rest != hinge.start()
        {
            return Err(ZeroThicknessAnalysisError::EvidenceUnavailable);
        }
    }

    for face in faces {
        for edge in face.edges.iter().copied() {
            let occurrences = edge_occurrences(faces, edge)?;
            let hinges = pose
                .hinges()
                .iter()
                .filter(|hinge| hinge.edge() == edge)
                .count();
            if !matches!((occurrences.len(), hinges), (1, 0) | (2, 1)) {
                return Err(ZeroThicknessAnalysisError::EvidenceUnavailable);
            }
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct AuthenticatedEdgeOccurrence {
    face: FaceId,
    start_id: VertexId,
    end_id: VertexId,
    start_rest: Point3,
    end_rest: Point3,
}

fn edge_occurrences(
    faces: &[AuthenticatedFace],
    edge: EdgeId,
) -> Result<Vec<AuthenticatedEdgeOccurrence>, ZeroThicknessAnalysisError> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(3)
        .map_err(|_| ZeroThicknessAnalysisError::ResourceLimitExceeded)?;
    for face in faces {
        for (index, candidate) in face.edges.iter().copied().enumerate() {
            if candidate != edge || result.len() == 3 {
                continue;
            }
            let start = &face.boundary[index];
            let end = &face.boundary[(index + 1) % face.boundary.len()];
            result.push(AuthenticatedEdgeOccurrence {
                face: face.id,
                start_id: start.id,
                end_id: end.id,
                start_rest: start.rest,
                end_rest: end.rest,
            });
        }
    }
    Ok(result)
}

#[cfg(test)]
fn authenticate_face_pair_topology(
    pose: &MaterialTreePose,
    first: &AuthenticatedFace,
    second: &AuthenticatedFace,
) -> Result<AuthenticatedTopology, ZeroThicknessAnalysisError> {
    let limits = unlimited_rational_limits();
    let mut meter = RationalWorkMeter::unlimited(&limits);
    authenticate_face_pair_topology_metered(pose, first, second, &mut meter)
}

fn authenticate_face_pair_topology_metered(
    pose: &MaterialTreePose,
    first: &AuthenticatedFace,
    second: &AuthenticatedFace,
    meter: &mut RationalWorkMeter<'_>,
) -> Result<AuthenticatedTopology, ZeroThicknessAnalysisError> {
    if first.id == second.id {
        return Err(ZeroThicknessAnalysisError::EvidenceUnavailable);
    }
    let mut shared_vertices = Vec::new();
    shared_vertices
        .try_reserve_exact(3)
        .map_err(|_| ZeroThicknessAnalysisError::ResourceLimitExceeded)?;
    for (first_index, first_vertex) in first.boundary.iter().enumerate() {
        for (second_index, second_vertex) in second.boundary.iter().enumerate() {
            if first_vertex.id == second_vertex.id {
                if shared_vertices.len() == 3 {
                    return Err(ZeroThicknessAnalysisError::EvidenceUnavailable);
                }
                shared_vertices.push((first_index, second_index));
            }
        }
    }
    let mut shared_edges = Vec::new();
    shared_edges
        .try_reserve_exact(2)
        .map_err(|_| ZeroThicknessAnalysisError::ResourceLimitExceeded)?;
    for (first_index, first_edge) in first.edges.iter().enumerate() {
        for (second_index, second_edge) in second.edges.iter().enumerate() {
            if first_edge == second_edge {
                if shared_edges.len() == 2 {
                    return Err(ZeroThicknessAnalysisError::EvidenceUnavailable);
                }
                shared_edges.push((first_index, second_index, *first_edge));
            }
        }
    }
    let mut connecting_hinge = None;
    for hinge in pose.hinges() {
        if !unordered_face_pair_eq(first.id, second.id, hinge.left_face(), hinge.right_face()) {
            continue;
        }
        if connecting_hinge.replace(hinge).is_some() {
            return Err(ZeroThicknessAnalysisError::EvidenceUnavailable);
        }
    }

    match (
        shared_vertices.as_slice(),
        shared_edges.as_slice(),
        connecting_hinge,
    ) {
        ([], [], None) => Ok(AuthenticatedTopology::NoSharedFeature),
        ([(first_index, second_index)], [], None) => {
            let first_point = first.boundary[*first_index].current.clone_retained(meter)?;
            let second_point = second.boundary[*second_index]
                .current
                .clone_retained(meter)?;
            if !first_point.equal_metered(&second_point, meter)? {
                return Ok(AuthenticatedTopology::SharedVertexPoseMismatch);
            }
            Ok(AuthenticatedTopology::SharedVertex(first_point))
        }
        (
            [(first_first, second_first), (first_second, second_second)],
            [(first_edge_index, second_edge_index, edge)],
            Some(hinge),
        ) if *edge == hinge.edge() => {
            let first_start = &first.boundary[*first_edge_index];
            let first_end = &first.boundary[(*first_edge_index + 1) % first.boundary.len()];
            let second_start = &second.boundary[*second_edge_index];
            let second_end = &second.boundary[(*second_edge_index + 1) % second.boundary.len()];
            let shared_ids = [
                (
                    first.boundary[*first_first].id,
                    second.boundary[*second_first].id,
                ),
                (
                    first.boundary[*first_second].id,
                    second.boundary[*second_second].id,
                ),
            ];
            if !shared_ids
                .iter()
                .all(|(first_id, second_id)| first_id == second_id)
                || first_start.id != second_end.id
                || first_end.id != second_start.id
            {
                return Err(ZeroThicknessAnalysisError::EvidenceUnavailable);
            }
            if !first_start
                .current
                .equal_metered(&second_end.current, meter)?
                || !first_end
                    .current
                    .equal_metered(&second_start.current, meter)?
            {
                // Never epsilon-weld independently rounded hinge transforms.
                // The source feature is authenticated, so the pair can still
                // complete its raw diagnostic scan. Its final evidence is
                // forced to Indeterminate until one canonical watertight
                // geometry source exists.
                return Ok(AuthenticatedTopology::SharedHingePoseMismatch);
            }
            Ok(AuthenticatedTopology::SharedHingeEdge {
                start: first_start.current.clone_retained(meter)?,
                end: first_end.current.clone_retained(meter)?,
            })
        }
        _ => Err(ZeroThicknessAnalysisError::EvidenceUnavailable),
    }
}

fn unordered_face_pair_eq(
    first: FaceId,
    second: FaceId,
    candidate_first: FaceId,
    candidate_second: FaceId,
) -> bool {
    (first == candidate_first && second == candidate_second)
        || (first == candidate_second && second == candidate_first)
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "SameFace exists only for adversarial classifier tests; canonical runtime pair indexes reject it before construction"
    )
)]
enum AuthenticatedTopology {
    NoSharedFeature,
    SharedVertex(ExactPoint3),
    SharedVertexPoseMismatch,
    SharedHingeEdge {
        start: ExactPoint3,
        end: ExactPoint3,
    },
    SharedHingePoseMismatch,
    SameFace,
}

impl AuthenticatedTopology {
    const fn relation(&self) -> TopologyRelation {
        match self {
            Self::NoSharedFeature => TopologyRelation::NoSharedFeature,
            Self::SharedVertex(_) | Self::SharedVertexPoseMismatch => {
                TopologyRelation::SharedVertex
            }
            Self::SharedHingeEdge { .. } | Self::SharedHingePoseMismatch => {
                TopologyRelation::SharedHingeEdge
            }
            Self::SameFace => TopologyRelation::SameFace,
        }
    }

    const fn is_pose_mismatch(&self) -> bool {
        matches!(
            self,
            Self::SharedVertexPoseMismatch | Self::SharedHingePoseMismatch
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
#[cfg(test)]
struct AuthenticatedTrianglePair {
    first: [Point3; 3],
    second: [Point3; 3],
    topology: AuthenticatedTopology,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct PairDispatch {
    topology: TopologyRelation,
    evidence: IntersectionEvidenceV2,
    decision: TopologyContactDecision,
    expected_triangle_pairs: usize,
    analyzed_triangle_pairs: usize,
}

impl PairDispatch {
    pub(super) const fn topology(&self) -> TopologyRelation {
        self.topology
    }

    pub(super) const fn evidence(&self) -> IntersectionEvidenceV2 {
        self.evidence
    }

    pub(super) const fn decision(&self) -> TopologyContactDecision {
        self.decision
    }

    pub(super) const fn expected_triangle_pairs(&self) -> usize {
        self.expected_triangle_pairs
    }

    pub(super) const fn analyzed_triangle_pairs(&self) -> usize {
        self.analyzed_triangle_pairs
    }

    pub(super) const fn has_complete_coverage(&self) -> bool {
        self.expected_triangle_pairs == self.analyzed_triangle_pairs
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExactInterval {
    start: BigRational,
    end: BigRational,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FaceLevelLineEvidence {
    NotApplicable,
    NoPositiveLine,
    BoundaryOnly,
    Transversal,
    Indeterminate,
}

#[derive(Debug)]
struct FaceLineSlice {
    coverage: Vec<ExactInterval>,
    material_boundary: Vec<ExactInterval>,
}

#[cfg(test)]
fn classify_face_level_line_intersection(
    first: &AuthenticatedFace,
    second: &AuthenticatedFace,
) -> Result<FaceLevelLineEvidence, ZeroThicknessAnalysisError> {
    let limits = unlimited_rational_limits();
    let mut meter = RationalWorkMeter::unlimited(&limits);
    classify_face_level_line_intersection_metered(first, second, &mut meter)
}

fn classify_face_level_line_intersection_metered(
    first: &AuthenticatedFace,
    second: &AuthenticatedFace,
    meter: &mut RationalWorkMeter<'_>,
) -> Result<FaceLevelLineEvidence, ZeroThicknessAnalysisError> {
    let Some(first_plane) = authenticated_face_support_plane(first, meter)? else {
        return Ok(FaceLevelLineEvidence::Indeterminate);
    };
    let Some(second_plane) = authenticated_face_support_plane(second, meter)? else {
        return Ok(FaceLevelLineEvidence::Indeterminate);
    };
    let line_direction = first_plane.normal.cross(&second_plane.normal, meter)?;
    let Some(axis) = line_direction
        .coordinates
        .iter()
        .position(|coordinate| !coordinate.is_zero())
    else {
        return Ok(FaceLevelLineEvidence::NotApplicable);
    };

    let interval_bound = first
        .triangles
        .len()
        .checked_add(second.triangles.len())
        .and_then(|count| count.checked_add(first.boundary.len()))
        .and_then(|count| count.checked_add(second.boundary.len()))
        .ok_or(ZeroThicknessAnalysisError::ResourceLimitExceeded)?;
    let event_bound = interval_bound
        .checked_mul(2)
        .ok_or(ZeroThicknessAnalysisError::ResourceLimitExceeded)?;
    let mut events = Vec::new();
    events
        .try_reserve_exact(event_bound)
        .map_err(|_| ZeroThicknessAnalysisError::ResourceLimitExceeded)?;

    let Some(first_slice) = build_face_line_slice(first, &second_plane, axis, &mut events, meter)?
    else {
        return Ok(FaceLevelLineEvidence::Indeterminate);
    };
    let Some(second_slice) = build_face_line_slice(second, &first_plane, axis, &mut events, meter)?
    else {
        return Ok(FaceLevelLineEvidence::Indeterminate);
    };
    if events.len() > event_bound
        || !material_boundary_is_covered(&first_slice, meter)?
        || !material_boundary_is_covered(&second_slice, meter)?
    {
        return Ok(FaceLevelLineEvidence::Indeterminate);
    }

    sort_rationals_metered(&mut events, meter)?;
    dedup_sorted_rationals_metered(&mut events, meter)?;
    if events.len() < 2 {
        return Ok(FaceLevelLineEvidence::NoPositiveLine);
    }

    let mut first_coverage_cursor = 0;
    let mut second_coverage_cursor = 0;
    let mut first_boundary_cursor = 0;
    let mut second_boundary_cursor = 0;
    let mut has_common_positive_cell = false;
    for event_pair in events.windows(2) {
        let [start, end] = event_pair else {
            return Ok(FaceLevelLineEvidence::Indeterminate);
        };
        if meter.compare(start, end)? != Ordering::Less {
            return Ok(FaceLevelLineEvidence::Indeterminate);
        }
        let sum = meter.add(start, end)?;
        let two = meter.positive_integer(2)?;
        let midpoint = meter.divide(&sum, &two)?;
        if !exact_interval_union_contains(
            &first_slice.coverage,
            &midpoint,
            &mut first_coverage_cursor,
            meter,
        )? || !exact_interval_union_contains(
            &second_slice.coverage,
            &midpoint,
            &mut second_coverage_cursor,
            meter,
        )? {
            continue;
        }
        has_common_positive_cell = true;
        let first_is_boundary = exact_interval_union_contains(
            &first_slice.material_boundary,
            &midpoint,
            &mut first_boundary_cursor,
            meter,
        )?;
        let second_is_boundary = exact_interval_union_contains(
            &second_slice.material_boundary,
            &midpoint,
            &mut second_boundary_cursor,
            meter,
        )?;
        if !first_is_boundary && !second_is_boundary {
            return Ok(FaceLevelLineEvidence::Transversal);
        }
    }

    Ok(if has_common_positive_cell {
        FaceLevelLineEvidence::BoundaryOnly
    } else {
        FaceLevelLineEvidence::NoPositiveLine
    })
}

fn authenticated_face_support_plane(
    face: &AuthenticatedFace,
    meter: &mut RationalWorkMeter<'_>,
) -> Result<Option<ExactTriangle>, ZeroThicknessAnalysisError> {
    let Some(first) = face.triangles.first() else {
        return Ok(None);
    };
    let plane = ExactTriangle::from_exact_points_metered(
        [
            first[0].clone_retained(meter)?,
            first[1].clone_retained(meter)?,
            first[2].clone_retained(meter)?,
        ],
        meter,
    )?;
    if plane.normal.is_zero() {
        return Ok(None);
    }
    for vertex in &face.boundary {
        if !plane
            .signed_plane_distance_metered(&vertex.current, meter)?
            .is_zero()
        {
            return Ok(None);
        }
    }
    for points in &face.triangles {
        let triangle = ExactTriangle::from_exact_points_metered(
            [
                points[0].clone_retained(meter)?,
                points[1].clone_retained(meter)?,
                points[2].clone_retained(meter)?,
            ],
            meter,
        )?;
        if triangle.normal.is_zero() {
            return Ok(None);
        }
        for point in &triangle.points {
            if !plane.signed_plane_distance_metered(point, meter)?.is_zero() {
                return Ok(None);
            }
        }
    }
    Ok(Some(plane))
}

fn build_face_line_slice(
    face: &AuthenticatedFace,
    other_plane: &ExactTriangle,
    axis: usize,
    events: &mut Vec<BigRational>,
    meter: &mut RationalWorkMeter<'_>,
) -> Result<Option<FaceLineSlice>, ZeroThicknessAnalysisError> {
    let mut coverage = Vec::new();
    coverage
        .try_reserve_exact(face.triangles.len())
        .map_err(|_| ZeroThicknessAnalysisError::ResourceLimitExceeded)?;
    for triangle in &face.triangles {
        let triangle = ExactTriangle::from_exact_points_metered(
            [
                triangle[0].clone_retained(meter)?,
                triangle[1].clone_retained(meter)?,
                triangle[2].clone_retained(meter)?,
            ],
            meter,
        )?;
        let distances = try_array3(|index| {
            other_plane.signed_plane_distance_metered(&triangle.points[index], meter)
        })?;
        if distances.iter().all(Zero::is_zero) {
            return Ok(None);
        }
        let Some(cut) = triangle_plane_cut(&triangle, &distances, meter)? else {
            return Ok(None);
        };
        for point in &cut.points {
            events.push(meter.clone_retained(point.coordinate(axis))?);
        }
        if cut.points.len() == 2 {
            let Some(interval) =
                exact_interval_from_coordinates(&cut.points[0], &cut.points[1], axis, meter)?
            else {
                return Ok(None);
            };
            coverage.push(interval);
        }
    }

    let mut material_boundary = Vec::new();
    material_boundary
        .try_reserve_exact(face.boundary.len())
        .map_err(|_| ZeroThicknessAnalysisError::ResourceLimitExceeded)?;
    for index in 0..face.boundary.len() {
        let start = &face.boundary[index].current;
        let end = &face.boundary[(index + 1) % face.boundary.len()].current;
        let start_distance = other_plane.signed_plane_distance_metered(start, meter)?;
        let end_distance = other_plane.signed_plane_distance_metered(end, meter)?;
        if start_distance.is_zero() && end_distance.is_zero() {
            let Some(interval) = exact_interval_from_coordinates(start, end, axis, meter)? else {
                return Ok(None);
            };
            events.push(meter.clone_retained(&interval.start)?);
            events.push(meter.clone_retained(&interval.end)?);
            material_boundary.push(interval);
        } else if start_distance.is_zero() {
            events.push(meter.clone_retained(start.coordinate(axis))?);
        } else if end_distance.is_zero() {
            events.push(meter.clone_retained(end.coordinate(axis))?);
        } else if start_distance.is_positive() != end_distance.is_positive() {
            let denominator = meter.subtract(&start_distance, &end_distance)?;
            if denominator.is_zero() {
                return Ok(None);
            }
            let parameter = meter.divide(&start_distance, &denominator)?;
            let point = start.interpolate_metered(end, &parameter, meter)?;
            events.push(meter.clone_retained(point.coordinate(axis))?);
        }
    }

    normalize_exact_intervals(&mut coverage, meter)?;
    normalize_exact_intervals(&mut material_boundary, meter)?;
    Ok(Some(FaceLineSlice {
        coverage,
        material_boundary,
    }))
}

fn exact_interval_from_coordinates(
    start: &ExactPoint3,
    end: &ExactPoint3,
    axis: usize,
    meter: &mut RationalWorkMeter<'_>,
) -> Result<Option<ExactInterval>, ZeroThicknessAnalysisError> {
    let first = meter.clone_retained(start.coordinate(axis))?;
    let second = meter.clone_retained(end.coordinate(axis))?;
    Ok(match meter.compare(&first, &second)? {
        Ordering::Less => Some(ExactInterval {
            start: first,
            end: second,
        }),
        Ordering::Greater => Some(ExactInterval {
            start: second,
            end: first,
        }),
        Ordering::Equal => None,
    })
}

fn normalize_exact_intervals(
    intervals: &mut Vec<ExactInterval>,
    meter: &mut RationalWorkMeter<'_>,
) -> Result<(), ZeroThicknessAnalysisError> {
    sort_intervals_metered(intervals, meter)?;
    if intervals.is_empty() {
        return Ok(());
    }
    let mut write = 0;
    for read in 1..intervals.len() {
        let candidate = ExactInterval {
            start: meter.clone_retained(&intervals[read].start)?,
            end: meter.clone_retained(&intervals[read].end)?,
        };
        if meter.compare(&candidate.start, &intervals[write].end)? != Ordering::Greater {
            if meter.compare(&candidate.end, &intervals[write].end)? == Ordering::Greater {
                intervals[write].end = candidate.end;
            }
        } else {
            write += 1;
            intervals[write] = candidate;
        }
    }
    intervals.truncate(write + 1);
    Ok(())
}

fn sort_rationals_metered(
    values: &mut [BigRational],
    meter: &mut RationalWorkMeter<'_>,
) -> Result<(), ZeroThicknessAnalysisError> {
    let mut failure = None;
    values.sort_unstable_by(|left, right| {
        if failure.is_some() {
            return Ordering::Equal;
        }
        match meter.compare(left, right) {
            Ok(ordering) => ordering,
            Err(error) => {
                failure = Some(error);
                Ordering::Equal
            }
        }
    });
    failure.map_or(Ok(()), Err)
}

fn dedup_sorted_rationals_metered(
    values: &mut Vec<BigRational>,
    meter: &mut RationalWorkMeter<'_>,
) -> Result<(), ZeroThicknessAnalysisError> {
    if values.is_empty() {
        return Ok(());
    }
    let mut write = 0;
    for read in 1..values.len() {
        if !meter.equal(&values[write], &values[read])? {
            write += 1;
            values.swap(write, read);
        }
    }
    values.truncate(write + 1);
    Ok(())
}

fn sort_intervals_metered(
    values: &mut [ExactInterval],
    meter: &mut RationalWorkMeter<'_>,
) -> Result<(), ZeroThicknessAnalysisError> {
    let mut failure = None;
    values.sort_unstable_by(|left, right| {
        if failure.is_some() {
            return Ordering::Equal;
        }
        let ordering = meter
            .compare(&left.start, &right.start)
            .and_then(|ordering| {
                if ordering == Ordering::Equal {
                    meter.compare(&left.end, &right.end)
                } else {
                    Ok(ordering)
                }
            });
        match ordering {
            Ok(ordering) => ordering,
            Err(error) => {
                failure = Some(error);
                Ordering::Equal
            }
        }
    });
    failure.map_or(Ok(()), Err)
}

fn sort_rational_pairs_metered(
    values: &mut [(BigRational, BigRational)],
    meter: &mut RationalWorkMeter<'_>,
) -> Result<(), ZeroThicknessAnalysisError> {
    let mut failure = None;
    values.sort_unstable_by(|left, right| {
        if failure.is_some() {
            return Ordering::Equal;
        }
        let ordering = meter.compare(&left.0, &right.0).and_then(|ordering| {
            if ordering == Ordering::Equal {
                meter.compare(&left.1, &right.1)
            } else {
                Ok(ordering)
            }
        });
        match ordering {
            Ok(ordering) => ordering,
            Err(error) => {
                failure = Some(error);
                Ordering::Equal
            }
        }
    });
    failure.map_or(Ok(()), Err)
}

fn material_boundary_is_covered(
    slice: &FaceLineSlice,
    meter: &mut RationalWorkMeter<'_>,
) -> Result<bool, ZeroThicknessAnalysisError> {
    let mut coverage_index = 0;
    'boundaries: for boundary in &slice.material_boundary {
        while coverage_index < slice.coverage.len()
            && meter.compare(&slice.coverage[coverage_index].end, &boundary.start)?
                == Ordering::Less
        {
            coverage_index += 1;
        }
        let Some(coverage) = slice.coverage.get(coverage_index) else {
            return Ok(false);
        };
        if meter.compare(&coverage.start, &boundary.start)? == Ordering::Greater
            || meter.compare(&coverage.end, &boundary.end)? == Ordering::Less
        {
            return Ok(false);
        }
        continue 'boundaries;
    }
    Ok(true)
}

fn exact_interval_union_contains(
    intervals: &[ExactInterval],
    coordinate: &BigRational,
    cursor: &mut usize,
    meter: &mut RationalWorkMeter<'_>,
) -> Result<bool, ZeroThicknessAnalysisError> {
    while *cursor < intervals.len()
        && meter.compare(&intervals[*cursor].end, coordinate)? == Ordering::Less
    {
        *cursor += 1;
    }
    let Some(interval) = intervals.get(*cursor) else {
        return Ok(false);
    };
    Ok(
        meter.compare(&interval.start, coordinate)? != Ordering::Greater
            && meter.compare(coordinate, &interval.end)? != Ordering::Greater,
    )
}

#[cfg(test)]
fn aggregate_authenticated_face_pair(
    first: &AuthenticatedFace,
    second: &AuthenticatedFace,
    topology: &AuthenticatedTopology,
    max_triangle_pairs: usize,
    max_boundary_relation_work: usize,
    hinges: usize,
) -> Result<PairDispatch, ZeroThicknessAnalysisError> {
    let limits = unlimited_rational_limits();
    let mut meter = RationalWorkMeter::unlimited(&limits);
    aggregate_authenticated_face_pair_metered(
        first,
        second,
        topology,
        max_triangle_pairs,
        max_boundary_relation_work,
        hinges,
        &mut meter,
    )
}

fn aggregate_authenticated_face_pair_metered(
    first: &AuthenticatedFace,
    second: &AuthenticatedFace,
    topology: &AuthenticatedTopology,
    max_triangle_pairs: usize,
    max_boundary_relation_work: usize,
    hinges: usize,
    meter: &mut RationalWorkMeter<'_>,
) -> Result<PairDispatch, ZeroThicknessAnalysisError> {
    if first.triangles.is_empty()
        || second.triangles.is_empty()
        || matches!(topology, AuthenticatedTopology::SameFace)
    {
        return Err(ZeroThicknessAnalysisError::EvidenceUnavailable);
    }
    let expected = first
        .triangles
        .len()
        .checked_mul(second.triangles.len())
        .ok_or(ZeroThicknessAnalysisError::ResourceLimitExceeded)?;
    if expected > max_triangle_pairs {
        return Err(ZeroThicknessAnalysisError::ResourceLimitExceeded);
    }
    let boundary_relation_work = estimated_boundary_relation_work(
        expected,
        first.triangles.len(),
        second.triangles.len(),
        first.boundary.len(),
        second.boundary.len(),
        hinges,
    )?;
    if boundary_relation_work > max_boundary_relation_work {
        return Err(ZeroThicknessAnalysisError::ResourceLimitExceeded);
    }

    let mut analyzed = 0_usize;
    let mut has_transversal = false;
    let mut has_coplanar_area = false;
    let mut has_exact_indeterminate = false;
    let mut has_artificial_boundary_artifact = false;
    let mut point_contacts = 0_usize;
    let mut line_contacts = 0_usize;
    let mut all_contacts_match_shared_feature = true;
    let mut hinge_intervals = Vec::new();
    if matches!(topology, AuthenticatedTopology::SharedHingeEdge { .. }) {
        hinge_intervals
            .try_reserve_exact(expected)
            .map_err(|_| ZeroThicknessAnalysisError::ResourceLimitExceeded)?;
    }

    for first_triangle in &first.triangles {
        let first_triangle = ExactTriangle::from_exact_points_metered(
            [
                first_triangle[0].clone_retained(meter)?,
                first_triangle[1].clone_retained(meter)?,
                first_triangle[2].clone_retained(meter)?,
            ],
            meter,
        )?;
        for second_triangle in &second.triangles {
            let second_triangle = ExactTriangle::from_exact_points_metered(
                [
                    second_triangle[0].clone_retained(meter)?,
                    second_triangle[1].clone_retained(meter)?,
                    second_triangle[2].clone_retained(meter)?,
                ],
                meter,
            )?;
            analyzed = analyzed
                .checked_add(1)
                .ok_or(ZeroThicknessAnalysisError::ResourceLimitExceeded)?;
            match classify_triangle_intersection_metered(&first_triangle, &second_triangle, meter)?
            {
                ExactIntersection::Separated => {}
                ExactIntersection::Point(point) => {
                    if !exact_point_on_material_boundary(&point, first, meter)?
                        && !exact_point_on_material_boundary(&point, second, meter)?
                    {
                        has_artificial_boundary_artifact = true;
                        continue;
                    }
                    point_contacts = point_contacts
                        .checked_add(1)
                        .ok_or(ZeroThicknessAnalysisError::ResourceLimitExceeded)?;
                    match topology {
                        AuthenticatedTopology::SharedVertex(shared) => {
                            all_contacts_match_shared_feature &=
                                point.equal_metered(shared, meter)?;
                        }
                        AuthenticatedTopology::SharedHingeEdge { start, end } => {
                            if let Some(parameter) =
                                exact_segment_parameter(&point, start, end, meter)?
                            {
                                hinge_intervals
                                    .push((meter.clone_retained(&parameter)?, parameter));
                            } else {
                                all_contacts_match_shared_feature = false;
                            }
                        }
                        AuthenticatedTopology::SharedVertexPoseMismatch
                        | AuthenticatedTopology::SharedHingePoseMismatch => {
                            all_contacts_match_shared_feature = false;
                        }
                        AuthenticatedTopology::NoSharedFeature
                        | AuthenticatedTopology::SameFace => {}
                    }
                }
                ExactIntersection::BoundaryLine { start, end } => {
                    let first_material_boundary =
                        exact_segment_on_material_boundary(&start, &end, first, meter)?;
                    let second_material_boundary =
                        exact_segment_on_material_boundary(&start, &end, second, meter)?;
                    if !first_material_boundary && !second_material_boundary {
                        // A triangle-local boundary can be an ear-clipping
                        // diagonal. Defer it to the exact whole-face interval
                        // union below: only a strict positive-length overlap
                        // of both relative interiors proves Transversal.
                        has_artificial_boundary_artifact = true;
                        continue;
                    }
                    line_contacts = line_contacts
                        .checked_add(1)
                        .ok_or(ZeroThicknessAnalysisError::ResourceLimitExceeded)?;
                    match topology {
                        AuthenticatedTopology::SharedVertex(_) => {
                            all_contacts_match_shared_feature = false;
                        }
                        AuthenticatedTopology::SharedHingeEdge {
                            start: shared_start,
                            end: shared_end,
                        } => {
                            match (
                                exact_segment_parameter(&start, shared_start, shared_end, meter)?,
                                exact_segment_parameter(&end, shared_start, shared_end, meter)?,
                            ) {
                                (Some(first_parameter), Some(second_parameter)) => {
                                    if meter.compare(&first_parameter, &second_parameter)?
                                        != Ordering::Greater
                                    {
                                        hinge_intervals.push((first_parameter, second_parameter));
                                    } else {
                                        hinge_intervals.push((second_parameter, first_parameter));
                                    }
                                }
                                _ => all_contacts_match_shared_feature = false,
                            }
                        }
                        AuthenticatedTopology::SharedVertexPoseMismatch
                        | AuthenticatedTopology::SharedHingePoseMismatch => {
                            all_contacts_match_shared_feature = false;
                        }
                        AuthenticatedTopology::NoSharedFeature
                        | AuthenticatedTopology::SameFace => {}
                    }
                }
                ExactIntersection::CoplanarArea => has_coplanar_area = true,
                ExactIntersection::Transversal => has_transversal = true,
                ExactIntersection::Indeterminate => has_exact_indeterminate = true,
            }
        }
    }
    if analyzed != expected {
        return Err(ZeroThicknessAnalysisError::EvidenceUnavailable);
    }

    let mut unresolved_artificial_boundary_artifact = has_artificial_boundary_artifact;
    if has_artificial_boundary_artifact && !has_transversal && !has_coplanar_area {
        match classify_face_level_line_intersection_metered(first, second, meter)? {
            FaceLevelLineEvidence::Transversal => {
                has_transversal = true;
                unresolved_artificial_boundary_artifact = false;
            }
            FaceLevelLineEvidence::BoundaryOnly => {
                line_contacts = line_contacts
                    .checked_add(1)
                    .ok_or(ZeroThicknessAnalysisError::ResourceLimitExceeded)?;
                all_contacts_match_shared_feature = false;
                unresolved_artificial_boundary_artifact = false;
            }
            FaceLevelLineEvidence::NotApplicable
            | FaceLevelLineEvidence::NoPositiveLine
            | FaceLevelLineEvidence::Indeterminate => {}
        }
    }
    let has_indeterminate = has_exact_indeterminate || unresolved_artificial_boundary_artifact;

    let evidence = if topology.is_pose_mismatch() {
        // Raw affine images are useful diagnostics, but even an arbitrarily
        // small shared-feature disagreement can manufacture a positive-length
        // relative-interior crossing or a positive-area coplanar overlap.
        // Preserve complete pair coverage while refusing both false-safe and
        // false-penetrating conclusions.
        IntersectionEvidenceV2::Indeterminate
    } else if has_transversal {
        IntersectionEvidenceV2::TransversalCrossing
    } else if has_coplanar_area {
        IntersectionEvidenceV2::CoplanarAreaOverlap
    } else if has_indeterminate {
        IntersectionEvidenceV2::Indeterminate
    } else {
        let contact_count = point_contacts
            .checked_add(line_contacts)
            .ok_or(ZeroThicknessAnalysisError::ResourceLimitExceeded)?;
        match topology {
            AuthenticatedTopology::NoSharedFeature => {
                generic_contact_evidence(point_contacts, line_contacts)
            }
            AuthenticatedTopology::SharedVertex(_) => {
                if contact_count == 0 {
                    IntersectionEvidenceV2::Indeterminate
                } else if all_contacts_match_shared_feature {
                    if exact_material_normals_are_cooriented(
                        &first.material_normal,
                        &second.material_normal,
                        meter,
                    )? {
                        IntersectionEvidenceV2::SharedFeatureContact
                    } else {
                        IntersectionEvidenceV2::Indeterminate
                    }
                } else {
                    generic_contact_evidence(point_contacts, line_contacts)
                }
            }
            AuthenticatedTopology::SharedHingeEdge { .. } => {
                if contact_count > 0
                    && all_contacts_match_shared_feature
                    && exact_intervals_cover_unit_segment(&mut hinge_intervals, meter)?
                {
                    IntersectionEvidenceV2::SharedFeatureContact
                } else if contact_count == 0 {
                    IntersectionEvidenceV2::Indeterminate
                } else {
                    generic_contact_evidence(point_contacts, line_contacts)
                }
            }
            AuthenticatedTopology::SharedVertexPoseMismatch
            | AuthenticatedTopology::SharedHingePoseMismatch => {
                IntersectionEvidenceV2::Indeterminate
            }
            AuthenticatedTopology::SameFace => IntersectionEvidenceV2::Indeterminate,
        }
    };
    Ok(PairDispatch {
        topology: topology.relation(),
        evidence,
        decision: classify_runtime_topology_contact_v2(topology.relation(), evidence),
        expected_triangle_pairs: expected,
        analyzed_triangle_pairs: analyzed,
    })
}

fn generic_contact_evidence(point_contacts: usize, line_contacts: usize) -> IntersectionEvidenceV2 {
    if line_contacts > 0 {
        IntersectionEvidenceV2::BoundaryLineContact
    } else if point_contacts > 0 {
        IntersectionEvidenceV2::PointContact
    } else {
        IntersectionEvidenceV2::Separated
    }
}

fn exact_material_normals_are_cooriented(
    first: &ExactVector3,
    second: &ExactVector3,
    meter: &mut RationalWorkMeter<'_>,
) -> Result<bool, ZeroThicknessAnalysisError> {
    let dot = first.dot(second, meter)?;
    let threshold = meter.input_binary64_scalar(1.0e-10)?;
    Ok(meter.compare(&dot, &threshold)? == Ordering::Greater)
}

fn exact_segment_parameter(
    point: &ExactPoint3,
    start: &ExactPoint3,
    end: &ExactPoint3,
    meter: &mut RationalWorkMeter<'_>,
) -> Result<Option<BigRational>, ZeroThicknessAnalysisError> {
    let direction = ExactVector3::between(start, end, meter)?;
    let offset = ExactVector3::between(start, point, meter)?;
    if direction.is_zero() || !offset.cross(&direction, meter)?.is_zero() {
        return Ok(None);
    }
    let Some(axis) = (0..3).find(|axis| !direction.coordinates[*axis].is_zero()) else {
        return Ok(None);
    };
    let numerator = meter.subtract(point.coordinate(axis), start.coordinate(axis))?;
    let parameter = meter.divide(&numerator, &direction.coordinates[axis])?;
    let zero = meter.zero()?;
    let one = meter.one()?;
    Ok((meter.compare(&parameter, &zero)? != Ordering::Less
        && meter.compare(&parameter, &one)? != Ordering::Greater)
        .then_some(parameter))
}

fn exact_unbounded_line_parameter(
    point: &ExactPoint3,
    start: &ExactPoint3,
    end: &ExactPoint3,
    meter: &mut RationalWorkMeter<'_>,
) -> Result<Option<BigRational>, ZeroThicknessAnalysisError> {
    let direction = ExactVector3::between(start, end, meter)?;
    let offset = ExactVector3::between(start, point, meter)?;
    if direction.is_zero() || !offset.cross(&direction, meter)?.is_zero() {
        return Ok(None);
    }
    let Some(axis) = (0..3).find(|axis| !direction.coordinates[*axis].is_zero()) else {
        return Ok(None);
    };
    let numerator = meter.subtract(point.coordinate(axis), start.coordinate(axis))?;
    Ok(Some(
        meter.divide(&numerator, &direction.coordinates[axis])?,
    ))
}

fn exact_point_on_material_boundary(
    point: &ExactPoint3,
    face: &AuthenticatedFace,
    meter: &mut RationalWorkMeter<'_>,
) -> Result<bool, ZeroThicknessAnalysisError> {
    for index in 0..face.boundary.len() {
        let start = &face.boundary[index].current;
        let end = &face.boundary[(index + 1) % face.boundary.len()].current;
        if exact_segment_parameter(point, start, end, meter)?.is_some() {
            return Ok(true);
        }
    }
    Ok(false)
}

fn exact_segment_on_material_boundary(
    segment_start: &ExactPoint3,
    segment_end: &ExactPoint3,
    face: &AuthenticatedFace,
    meter: &mut RationalWorkMeter<'_>,
) -> Result<bool, ZeroThicknessAnalysisError> {
    if segment_start.equal_metered(segment_end, meter)? {
        return exact_point_on_material_boundary(segment_start, face, meter);
    }
    let mut intervals = Vec::new();
    intervals
        .try_reserve_exact(face.boundary.len())
        .map_err(|_| ZeroThicknessAnalysisError::ResourceLimitExceeded)?;
    for index in 0..face.boundary.len() {
        let edge_start = &face.boundary[index].current;
        let edge_end = &face.boundary[(index + 1) % face.boundary.len()].current;
        let (Some(first), Some(second)) = (
            exact_unbounded_line_parameter(edge_start, segment_start, segment_end, meter)?,
            exact_unbounded_line_parameter(edge_end, segment_start, segment_end, meter)?,
        ) else {
            continue;
        };
        let (line_start, line_end) = if meter.compare(&first, &second)? != Ordering::Greater {
            (first, second)
        } else {
            (second, first)
        };
        let zero = meter.zero()?;
        let start = if meter.compare(&line_start, &zero)? == Ordering::Less {
            zero
        } else {
            line_start
        };
        let one = meter.one()?;
        let end = if meter.compare(&line_end, &one)? == Ordering::Greater {
            one
        } else {
            line_end
        };
        if meter.compare(&start, &end)? != Ordering::Greater {
            intervals.push((start, end));
        }
    }
    exact_intervals_cover_unit_segment(&mut intervals, meter)
}

fn exact_intervals_cover_unit_segment(
    intervals: &mut [(BigRational, BigRational)],
    meter: &mut RationalWorkMeter<'_>,
) -> Result<bool, ZeroThicknessAnalysisError> {
    if intervals.is_empty() {
        return Ok(false);
    }
    sort_rational_pairs_metered(intervals, meter)?;
    let zero = meter.zero()?;
    if meter.compare(&intervals[0].0, &zero)? != Ordering::Equal {
        return Ok(false);
    }
    let mut covered = meter.clone_retained(&intervals[0].1)?;
    for (start, end) in &intervals[1..] {
        if meter.compare(start, &covered)? == Ordering::Greater {
            return Ok(false);
        }
        if meter.compare(end, &covered)? == Ordering::Greater {
            covered = meter.clone_retained(end)?;
        }
    }
    let one = meter.one()?;
    Ok(meter.compare(&covered, &one)? == Ordering::Equal)
}

#[cfg(test)]
fn dispatch_authenticated_zero_thickness_pair(pair: &AuthenticatedTrianglePair) -> PairDispatch {
    let topology = pair.topology.relation();
    if matches!(topology, TopologyRelation::SameFace) {
        return PairDispatch {
            topology,
            evidence: IntersectionEvidenceV2::Indeterminate,
            decision: TopologyContactDecision::Indeterminate,
            expected_triangle_pairs: 1,
            analyzed_triangle_pairs: 1,
        };
    }

    let first = ExactTriangle::from_points(pair.first);
    let second = ExactTriangle::from_points(pair.second);
    let intersection = classify_triangle_intersection(&first, &second);
    let evidence = evidence_for_authenticated_topology(intersection, &pair.topology);
    PairDispatch {
        topology,
        evidence,
        decision: classify_runtime_topology_contact_v2(topology, evidence),
        expected_triangle_pairs: 1,
        analyzed_triangle_pairs: 1,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExactPoint3 {
    coordinates: [BigRational; 3],
}

impl ExactPoint3 {
    fn from_input_point(
        point: Point3,
        meter: &mut RationalWorkMeter<'_>,
    ) -> Result<Self, ZeroThicknessAnalysisError> {
        Ok(Self {
            coordinates: [
                meter.input_binary64_common_unit(point.x())?,
                meter.input_binary64_common_unit(point.y())?,
                meter.input_binary64_common_unit(point.z())?,
            ],
        })
    }

    #[cfg(test)]
    fn from_point(point: Point3) -> Self {
        Self {
            coordinates: [
                exact_binary64(point.x()),
                exact_binary64(point.y()),
                exact_binary64(point.z()),
            ],
        }
    }

    fn coordinate(&self, index: usize) -> &BigRational {
        &self.coordinates[index]
    }

    fn equal_metered(
        &self,
        other: &Self,
        meter: &mut RationalWorkMeter<'_>,
    ) -> Result<bool, ZeroThicknessAnalysisError> {
        for (left, right) in self.coordinates.iter().zip(&other.coordinates) {
            if !meter.equal(left, right)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn clone_retained(
        &self,
        meter: &mut RationalWorkMeter<'_>,
    ) -> Result<Self, ZeroThicknessAnalysisError> {
        Ok(Self {
            coordinates: try_array3(|index| meter.clone_retained(&self.coordinates[index]))?,
        })
    }

    fn observe_output(
        &self,
        meter: &mut RationalWorkMeter<'_>,
    ) -> Result<(), ZeroThicknessAnalysisError> {
        for coordinate in &self.coordinates {
            meter.output(coordinate)?;
        }
        Ok(())
    }

    fn interpolate_metered(
        &self,
        other: &Self,
        parameter: &BigRational,
        meter: &mut RationalWorkMeter<'_>,
    ) -> Result<Self, ZeroThicknessAnalysisError> {
        Ok(Self {
            coordinates: try_array3(|index| {
                let delta = meter.subtract(&other.coordinates[index], &self.coordinates[index])?;
                let scaled = meter.multiply(parameter, &delta)?;
                meter.add(&self.coordinates[index], &scaled)
            })?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExactAffineTransform {
    rotation: [[BigRational; 3]; 3],
    translation: ExactPoint3,
}

impl ExactAffineTransform {
    #[cfg(test)]
    fn from_transform(transform: RigidTransform) -> Self {
        let limits = unlimited_rational_limits();
        let mut meter = RationalWorkMeter::unlimited(&limits);
        Self::from_transform_metered(transform, &mut meter).expect("unlimited exact test transform")
    }

    fn from_transform_metered(
        transform: RigidTransform,
        meter: &mut RationalWorkMeter<'_>,
    ) -> Result<Self, ZeroThicknessAnalysisError> {
        let rows = transform.rotation_rows();
        Ok(Self {
            rotation: [
                try_array3(|column| meter.input_binary64_scalar(rows[0][column]))?,
                try_array3(|column| meter.input_binary64_scalar(rows[1][column]))?,
                try_array3(|column| meter.input_binary64_scalar(rows[2][column]))?,
            ],
            translation: ExactPoint3::from_input_point(transform.translation(), meter)?,
        })
    }

    #[cfg(test)]
    fn apply_point(&self, point: &ExactPoint3) -> ExactPoint3 {
        let limits = unlimited_rational_limits();
        let mut meter = RationalWorkMeter::unlimited(&limits);
        self.apply_point_metered(point, &mut meter)
            .expect("unlimited exact test affine application")
    }

    fn apply_point_metered(
        &self,
        point: &ExactPoint3,
        meter: &mut RationalWorkMeter<'_>,
    ) -> Result<ExactPoint3, ZeroThicknessAnalysisError> {
        Ok(ExactPoint3 {
            coordinates: try_array3(|row| {
                let mut coordinate = meter.clone_temporary(&self.translation.coordinates[row])?;
                for column in 0..3 {
                    let product =
                        meter.multiply(&self.rotation[row][column], &point.coordinates[column])?;
                    coordinate = meter.add(&coordinate, &product)?;
                }
                Ok(coordinate)
            })?,
        })
    }

    fn transformed_local_y(
        &self,
        meter: &mut RationalWorkMeter<'_>,
    ) -> Result<ExactVector3, ZeroThicknessAnalysisError> {
        let result = ExactVector3 {
            coordinates: try_array3(|row| meter.clone_retained(&self.rotation[row][1]))?,
        };
        result.observe_output(meter)?;
        Ok(result)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExactVector3 {
    coordinates: [BigRational; 3],
}

impl ExactVector3 {
    fn between(
        start: &ExactPoint3,
        end: &ExactPoint3,
        meter: &mut RationalWorkMeter<'_>,
    ) -> Result<Self, ZeroThicknessAnalysisError> {
        Ok(Self {
            coordinates: try_array3(|index| {
                meter.subtract(&end.coordinates[index], &start.coordinates[index])
            })?,
        })
    }

    fn cross(
        &self,
        other: &Self,
        meter: &mut RationalWorkMeter<'_>,
    ) -> Result<Self, ZeroThicknessAnalysisError> {
        let [ax, ay, az] = &self.coordinates;
        let [bx, by, bz] = &other.coordinates;
        let ay_bz = meter.multiply(ay, bz)?;
        let az_by = meter.multiply(az, by)?;
        let az_bx = meter.multiply(az, bx)?;
        let ax_bz = meter.multiply(ax, bz)?;
        let ax_by = meter.multiply(ax, by)?;
        let ay_bx = meter.multiply(ay, bx)?;
        Ok(Self {
            coordinates: [
                meter.subtract(&ay_bz, &az_by)?,
                meter.subtract(&az_bx, &ax_bz)?,
                meter.subtract(&ax_by, &ay_bx)?,
            ],
        })
    }

    fn dot(
        &self,
        other: &Self,
        meter: &mut RationalWorkMeter<'_>,
    ) -> Result<BigRational, ZeroThicknessAnalysisError> {
        let mut result = meter.zero()?;
        for (left, right) in self.coordinates.iter().zip(&other.coordinates) {
            let product = meter.multiply(left, right)?;
            result = meter.add(&result, &product)?;
        }
        Ok(result)
    }

    fn observe_output(
        &self,
        meter: &mut RationalWorkMeter<'_>,
    ) -> Result<(), ZeroThicknessAnalysisError> {
        for coordinate in &self.coordinates {
            meter.output(coordinate)?;
        }
        Ok(())
    }

    fn is_zero(&self) -> bool {
        self.coordinates.iter().all(Zero::is_zero)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExactTriangle {
    points: [ExactPoint3; 3],
    normal: ExactVector3,
}

impl ExactTriangle {
    #[cfg(test)]
    fn from_points(points: [Point3; 3]) -> Self {
        let points = points.map(ExactPoint3::from_point);
        Self::from_exact_points(points)
    }

    #[cfg(test)]
    fn from_exact_points(points: [ExactPoint3; 3]) -> Self {
        let limits = unlimited_rational_limits();
        let mut meter = RationalWorkMeter::unlimited(&limits);
        Self::from_exact_points_metered(points, &mut meter).expect("unlimited exact test triangle")
    }

    fn from_exact_points_metered(
        points: [ExactPoint3; 3],
        meter: &mut RationalWorkMeter<'_>,
    ) -> Result<Self, ZeroThicknessAnalysisError> {
        let first_edge = ExactVector3::between(&points[0], &points[1], meter)?;
        let second_edge = ExactVector3::between(&points[0], &points[2], meter)?;
        let normal = first_edge.cross(&second_edge, meter)?;
        Ok(Self { points, normal })
    }

    fn signed_plane_distance_metered(
        &self,
        point: &ExactPoint3,
        meter: &mut RationalWorkMeter<'_>,
    ) -> Result<BigRational, ZeroThicknessAnalysisError> {
        self.normal.dot(
            &ExactVector3::between(&self.points[0], point, meter)?,
            meter,
        )
    }

    #[cfg(test)]
    fn signed_plane_distance(&self, point: &ExactPoint3) -> BigRational {
        let limits = unlimited_rational_limits();
        let mut meter = RationalWorkMeter::unlimited(&limits);
        self.signed_plane_distance_metered(point, &mut meter)
            .expect("unlimited exact test plane distance")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ExactIntersection {
    Separated,
    Point(ExactPoint3),
    BoundaryLine {
        start: ExactPoint3,
        end: ExactPoint3,
    },
    CoplanarArea,
    Transversal,
    Indeterminate,
}

#[cfg(test)]
fn classify_triangle_intersection(
    first: &ExactTriangle,
    second: &ExactTriangle,
) -> ExactIntersection {
    let limits = unlimited_rational_limits();
    let mut meter = RationalWorkMeter::unlimited(&limits);
    classify_triangle_intersection_metered(first, second, &mut meter)
        .expect("unlimited exact test classification")
}

fn classify_triangle_intersection_metered(
    first: &ExactTriangle,
    second: &ExactTriangle,
    meter: &mut RationalWorkMeter<'_>,
) -> Result<ExactIntersection, ZeroThicknessAnalysisError> {
    if first.normal.is_zero() || second.normal.is_zero() {
        return Ok(ExactIntersection::Indeterminate);
    }

    let second_distances =
        try_array3(|index| first.signed_plane_distance_metered(&second.points[index], meter))?;
    if strictly_same_side(&second_distances) {
        return Ok(ExactIntersection::Separated);
    }
    let first_distances =
        try_array3(|index| second.signed_plane_distance_metered(&first.points[index], meter))?;
    if strictly_same_side(&first_distances) {
        return Ok(ExactIntersection::Separated);
    }

    if second_distances.iter().all(Zero::is_zero) {
        if !first_distances.iter().all(Zero::is_zero) {
            return Ok(ExactIntersection::Indeterminate);
        }
        return classify_coplanar_intersection(first, second, meter);
    }

    classify_non_coplanar_intersection(first, second, &first_distances, &second_distances, meter)
}

fn strictly_same_side(distances: &[BigRational; 3]) -> bool {
    distances.iter().all(|value| value.is_positive())
        || distances.iter().all(|value| value.is_negative())
}

#[derive(Debug)]
struct PlaneCut {
    points: Vec<ExactPoint3>,
    is_boundary_edge: bool,
}

fn triangle_plane_cut(
    triangle: &ExactTriangle,
    distances: &[BigRational; 3],
    meter: &mut RationalWorkMeter<'_>,
) -> Result<Option<PlaneCut>, ZeroThicknessAnalysisError> {
    let zero_count = distances.iter().filter(|value| value.is_zero()).count();
    let mut points = Vec::new();
    points
        .try_reserve_exact(2)
        .map_err(|_| ZeroThicknessAnalysisError::ResourceLimitExceeded)?;

    for (point, distance) in triangle.points.iter().zip(distances) {
        if distance.is_zero()
            && !push_unique_bounded(&mut points, point.clone_retained(meter)?, 2, meter)?
        {
            return Ok(None);
        }
    }
    for index in 0..3 {
        let next = (index + 1) % 3;
        if (distances[index].is_positive() && distances[next].is_positive())
            || (distances[index].is_negative() && distances[next].is_negative())
            || distances[index].is_zero()
            || distances[next].is_zero()
        {
            continue;
        }
        let denominator = meter.subtract(&distances[index], &distances[next])?;
        if denominator.is_zero() {
            return Ok(None);
        }
        let parameter = meter.divide(&distances[index], &denominator)?;
        if !push_unique_bounded(
            &mut points,
            triangle.points[index].interpolate_metered(
                &triangle.points[next],
                &parameter,
                meter,
            )?,
            2,
            meter,
        )? {
            return Ok(None);
        }
    }

    if points.len() > 2 {
        return Ok(None);
    }
    Ok(Some(PlaneCut {
        points,
        is_boundary_edge: zero_count == 2,
    }))
}

fn classify_non_coplanar_intersection(
    first: &ExactTriangle,
    second: &ExactTriangle,
    first_distances: &[BigRational; 3],
    second_distances: &[BigRational; 3],
    meter: &mut RationalWorkMeter<'_>,
) -> Result<ExactIntersection, ZeroThicknessAnalysisError> {
    let Some(first_cut) = triangle_plane_cut(first, first_distances, meter)? else {
        return Ok(ExactIntersection::Indeterminate);
    };
    let Some(second_cut) = triangle_plane_cut(second, second_distances, meter)? else {
        return Ok(ExactIntersection::Indeterminate);
    };
    if first_cut.points.is_empty() || second_cut.points.is_empty() {
        return Ok(ExactIntersection::Separated);
    }

    let axis = match varying_axis(&first_cut.points, meter)? {
        Some(axis) => Some(axis),
        None => varying_axis(&second_cut.points, meter)?,
    };
    let Some(axis) = axis else {
        return Ok(
            if first_cut.points[0].equal_metered(&second_cut.points[0], meter)? {
                ExactIntersection::Point(first_cut.points[0].clone_retained(meter)?)
            } else {
                ExactIntersection::Separated
            },
        );
    };
    let first_interval = cut_interval(&first_cut, axis, meter)?;
    let second_interval = cut_interval(&second_cut, axis, meter)?;
    let overlap_start = if meter.compare(&first_interval.0, &second_interval.0)? == Ordering::Less {
        second_interval.0
    } else {
        first_interval.0
    };
    let overlap_end = if meter.compare(&first_interval.1, &second_interval.1)? == Ordering::Greater
    {
        second_interval.1
    } else {
        first_interval.1
    };
    Ok(match meter.compare(&overlap_start, &overlap_end)? {
        Ordering::Greater => ExactIntersection::Separated,
        Ordering::Equal => {
            point_at_coordinate(&first_cut, &second_cut, axis, &overlap_start, meter)?
                .map_or(ExactIntersection::Indeterminate, ExactIntersection::Point)
        }
        Ordering::Less => {
            if !first_cut.is_boundary_edge && !second_cut.is_boundary_edge {
                return Ok(ExactIntersection::Transversal);
            }
            let Some(start) =
                point_at_coordinate(&first_cut, &second_cut, axis, &overlap_start, meter)?
            else {
                return Ok(ExactIntersection::Indeterminate);
            };
            let Some(end) =
                point_at_coordinate(&first_cut, &second_cut, axis, &overlap_end, meter)?
            else {
                return Ok(ExactIntersection::Indeterminate);
            };
            ExactIntersection::BoundaryLine { start, end }
        }
    })
}

fn varying_axis(
    points: &[ExactPoint3],
    meter: &mut RationalWorkMeter<'_>,
) -> Result<Option<usize>, ZeroThicknessAnalysisError> {
    let Some(first) = points.first() else {
        return Ok(None);
    };
    let Some(second) = points.get(1) else {
        return Ok(None);
    };
    for index in 0..3 {
        if !meter.equal(first.coordinate(index), second.coordinate(index))? {
            return Ok(Some(index));
        }
    }
    Ok(None)
}

fn cut_interval(
    cut: &PlaneCut,
    axis: usize,
    meter: &mut RationalWorkMeter<'_>,
) -> Result<(BigRational, BigRational), ZeroThicknessAnalysisError> {
    let first = meter.clone_retained(cut.points[0].coordinate(axis))?;
    let second = if let Some(point) = cut.points.get(1) {
        meter.clone_retained(point.coordinate(axis))?
    } else {
        meter.clone_retained(&first)?
    };
    Ok(if meter.compare(&first, &second)? != Ordering::Greater {
        (first, second)
    } else {
        (second, first)
    })
}

fn point_at_coordinate(
    first: &PlaneCut,
    second: &PlaneCut,
    axis: usize,
    coordinate: &BigRational,
    meter: &mut RationalWorkMeter<'_>,
) -> Result<Option<ExactPoint3>, ZeroThicknessAnalysisError> {
    if let Some(point) = point_on_cut_at_coordinate(first, axis, coordinate, meter)? {
        Ok(Some(point))
    } else {
        point_on_cut_at_coordinate(second, axis, coordinate, meter)
    }
}

fn point_on_cut_at_coordinate(
    cut: &PlaneCut,
    axis: usize,
    coordinate: &BigRational,
    meter: &mut RationalWorkMeter<'_>,
) -> Result<Option<ExactPoint3>, ZeroThicknessAnalysisError> {
    let start = &cut.points[0];
    let Some(end) = cut.points.get(1) else {
        return if meter.equal(start.coordinate(axis), coordinate)? {
            Ok(Some(start.clone_retained(meter)?))
        } else {
            Ok(None)
        };
    };
    let denominator = meter.subtract(end.coordinate(axis), start.coordinate(axis))?;
    if denominator.is_zero() {
        return Ok(None);
    }
    let numerator = meter.subtract(coordinate, start.coordinate(axis))?;
    let parameter = meter.divide(&numerator, &denominator)?;
    Ok(Some(start.interpolate_metered(end, &parameter, meter)?))
}

fn classify_coplanar_intersection(
    first: &ExactTriangle,
    second: &ExactTriangle,
    meter: &mut RationalWorkMeter<'_>,
) -> Result<ExactIntersection, ZeroThicknessAnalysisError> {
    let Some(drop_axis) = first
        .normal
        .coordinates
        .iter()
        .position(|component| !component.is_zero())
    else {
        return Ok(ExactIntersection::Indeterminate);
    };
    let [first_axis, second_axis] = projected_axes(drop_axis);
    let clip_orientation = projected_line_value(
        &second.points[0],
        &second.points[1],
        &second.points[2],
        first_axis,
        second_axis,
        meter,
    )?;
    if clip_orientation.is_zero() {
        return Ok(ExactIntersection::Indeterminate);
    }

    let mut polygon = Vec::new();
    polygon
        .try_reserve_exact(3)
        .map_err(|_| ZeroThicknessAnalysisError::ResourceLimitExceeded)?;
    for point in &first.points {
        polygon.push(point.clone_retained(meter)?);
    }
    for edge_index in 0..3 {
        let edge_start = &second.points[edge_index];
        let edge_end = &second.points[(edge_index + 1) % 3];
        let Some(clipped) = clip_polygon_against_line(
            &polygon,
            edge_start,
            edge_end,
            clip_orientation.is_positive(),
            first_axis,
            second_axis,
            meter,
        )?
        else {
            return Ok(ExactIntersection::Indeterminate);
        };
        polygon = clipped;
        if polygon.is_empty() {
            return Ok(ExactIntersection::Separated);
        }
    }
    if !deduplicate_polygon(&mut polygon, meter)? {
        return Ok(ExactIntersection::Indeterminate);
    }

    Ok(match polygon.as_slice() {
        [] => ExactIntersection::Separated,
        [point] => ExactIntersection::Point(point.clone_retained(meter)?),
        [start, end] => line_or_point(start, end, meter)?,
        _ => {
            let area = projected_polygon_double_area(&polygon, first_axis, second_axis, meter)?;
            if area.is_zero() {
                collapsed_polygon_intersection(&polygon, first_axis, second_axis, meter)?
                    .unwrap_or(ExactIntersection::Indeterminate)
            } else {
                ExactIntersection::CoplanarArea
            }
        }
    })
}

const fn projected_axes(drop_axis: usize) -> [usize; 2] {
    match drop_axis {
        0 => [1, 2],
        1 => [0, 2],
        _ => [0, 1],
    }
}

fn projected_line_value(
    start: &ExactPoint3,
    end: &ExactPoint3,
    point: &ExactPoint3,
    first_axis: usize,
    second_axis: usize,
    meter: &mut RationalWorkMeter<'_>,
) -> Result<BigRational, ZeroThicknessAnalysisError> {
    let first_line = meter.subtract(end.coordinate(first_axis), start.coordinate(first_axis))?;
    let first_point =
        meter.subtract(point.coordinate(second_axis), start.coordinate(second_axis))?;
    let second_line = meter.subtract(end.coordinate(second_axis), start.coordinate(second_axis))?;
    let second_point =
        meter.subtract(point.coordinate(first_axis), start.coordinate(first_axis))?;
    let first_product = meter.multiply(&first_line, &first_point)?;
    let second_product = meter.multiply(&second_line, &second_point)?;
    meter.subtract(&first_product, &second_product)
}

fn clip_polygon_against_line(
    polygon: &[ExactPoint3],
    line_start: &ExactPoint3,
    line_end: &ExactPoint3,
    positive_inside: bool,
    first_axis: usize,
    second_axis: usize,
    meter: &mut RationalWorkMeter<'_>,
) -> Result<Option<Vec<ExactPoint3>>, ZeroThicknessAnalysisError> {
    if polygon.is_empty() {
        return Ok(Some(Vec::new()));
    }
    let mut result = Vec::new();
    let result_bound = polygon
        .len()
        .checked_add(1)
        .ok_or(ZeroThicknessAnalysisError::ResourceLimitExceeded)?;
    result
        .try_reserve_exact(result_bound)
        .map_err(|_| ZeroThicknessAnalysisError::ResourceLimitExceeded)?;

    for index in 0..polygon.len() {
        let current = &polygon[index];
        let next = &polygon[(index + 1) % polygon.len()];
        let current_value = projected_line_value(
            line_start,
            line_end,
            current,
            first_axis,
            second_axis,
            meter,
        )?;
        let next_value =
            projected_line_value(line_start, line_end, next, first_axis, second_axis, meter)?;
        let current_inside = is_inside(&current_value, positive_inside);
        let next_inside = is_inside(&next_value, positive_inside);

        if current_inside != next_inside {
            let denominator = meter.subtract(&current_value, &next_value)?;
            if denominator.is_zero() {
                return Ok(None);
            }
            let parameter = meter.divide(&current_value, &denominator)?;
            if !push_unique_bounded(
                &mut result,
                current.interpolate_metered(next, &parameter, meter)?,
                result_bound,
                meter,
            )? {
                return Ok(None);
            }
        }
        if next_inside
            && !push_unique_bounded(
                &mut result,
                next.clone_retained(meter)?,
                result_bound,
                meter,
            )?
        {
            return Ok(None);
        }
    }
    if result.len() > 1 {
        let first = result
            .first()
            .ok_or(ZeroThicknessAnalysisError::EvidenceUnavailable)?;
        let last = result
            .last()
            .ok_or(ZeroThicknessAnalysisError::EvidenceUnavailable)?;
        if first.equal_metered(last, meter)? {
            result.pop();
        }
    }
    Ok(Some(result))
}

fn is_inside(value: &BigRational, positive_inside: bool) -> bool {
    value.is_zero() || value.is_positive() == positive_inside
}

fn deduplicate_polygon(
    polygon: &mut Vec<ExactPoint3>,
    meter: &mut RationalWorkMeter<'_>,
) -> Result<bool, ZeroThicknessAnalysisError> {
    let mut unique = Vec::new();
    unique
        .try_reserve_exact(polygon.len())
        .map_err(|_| ZeroThicknessAnalysisError::ResourceLimitExceeded)?;
    let bound = polygon.len();
    for point in polygon.drain(..) {
        if !push_unique_bounded(&mut unique, point, bound, meter)? {
            return Ok(false);
        }
    }
    *polygon = unique;
    Ok(true)
}

fn projected_polygon_double_area(
    polygon: &[ExactPoint3],
    first_axis: usize,
    second_axis: usize,
    meter: &mut RationalWorkMeter<'_>,
) -> Result<BigRational, ZeroThicknessAnalysisError> {
    let mut area = meter.zero()?;
    for index in 0..polygon.len() {
        let current = &polygon[index];
        let next = &polygon[(index + 1) % polygon.len()];
        let first = meter.multiply(current.coordinate(first_axis), next.coordinate(second_axis))?;
        let second =
            meter.multiply(current.coordinate(second_axis), next.coordinate(first_axis))?;
        let term = meter.subtract(&first, &second)?;
        area = meter.add(&area, &term)?;
    }
    Ok(area)
}

fn collapsed_polygon_intersection(
    polygon: &[ExactPoint3],
    first_axis: usize,
    second_axis: usize,
    meter: &mut RationalWorkMeter<'_>,
) -> Result<Option<ExactIntersection>, ZeroThicknessAnalysisError> {
    let mut ordered = Vec::new();
    ordered
        .try_reserve_exact(polygon.len())
        .map_err(|_| ZeroThicknessAnalysisError::ResourceLimitExceeded)?;
    ordered.extend(polygon);
    let mut failure = None;
    ordered.sort_unstable_by(|left, right| {
        if failure.is_some() {
            return Ordering::Equal;
        }
        let ordering = meter
            .compare(left.coordinate(first_axis), right.coordinate(first_axis))
            .and_then(|ordering| {
                if ordering == Ordering::Equal {
                    meter.compare(left.coordinate(second_axis), right.coordinate(second_axis))
                } else {
                    Ok(ordering)
                }
            });
        match ordering {
            Ok(ordering) => ordering,
            Err(error) => {
                failure = Some(error);
                Ordering::Equal
            }
        }
    });
    if let Some(error) = failure {
        return Err(error);
    }
    Ok(Some(match (ordered.first(), ordered.last()) {
        (Some(start), Some(end)) => line_or_point(start, end, meter)?,
        _ => ExactIntersection::Separated,
    }))
}

fn line_or_point(
    start: &ExactPoint3,
    end: &ExactPoint3,
    meter: &mut RationalWorkMeter<'_>,
) -> Result<ExactIntersection, ZeroThicknessAnalysisError> {
    Ok(if start.equal_metered(end, meter)? {
        ExactIntersection::Point(start.clone_retained(meter)?)
    } else {
        ExactIntersection::BoundaryLine {
            start: start.clone_retained(meter)?,
            end: end.clone_retained(meter)?,
        }
    })
}

fn push_unique_bounded(
    points: &mut Vec<ExactPoint3>,
    point: ExactPoint3,
    bound: usize,
    meter: &mut RationalWorkMeter<'_>,
) -> Result<bool, ZeroThicknessAnalysisError> {
    for previous in points.iter() {
        if previous.equal_metered(&point, meter)? {
            return Ok(true);
        }
    }
    if points.len() < bound {
        points.push(point);
        Ok(true)
    } else {
        Ok(false)
    }
}

#[cfg(test)]
fn evidence_for_authenticated_topology(
    intersection: ExactIntersection,
    topology: &AuthenticatedTopology,
) -> IntersectionEvidenceV2 {
    match intersection {
        ExactIntersection::Separated => IntersectionEvidenceV2::Separated,
        ExactIntersection::Point(point)
            if matches!(
                topology,
                AuthenticatedTopology::SharedVertex(shared)
                    if point == *shared
            ) =>
        {
            IntersectionEvidenceV2::SharedFeatureContact
        }
        ExactIntersection::Point(_) => IntersectionEvidenceV2::PointContact,
        ExactIntersection::BoundaryLine { start, end }
            if matches!(
                topology,
                AuthenticatedTopology::SharedHingeEdge {
                    start: shared_start,
                    end: shared_end
                } if unordered_segment_eq(
                    &start,
                    &end,
                    shared_start,
                    shared_end,
                )
            ) =>
        {
            IntersectionEvidenceV2::SharedFeatureContact
        }
        ExactIntersection::BoundaryLine { .. } => IntersectionEvidenceV2::BoundaryLineContact,
        ExactIntersection::CoplanarArea => IntersectionEvidenceV2::CoplanarAreaOverlap,
        ExactIntersection::Transversal => IntersectionEvidenceV2::TransversalCrossing,
        ExactIntersection::Indeterminate => IntersectionEvidenceV2::Indeterminate,
    }
}

#[cfg(test)]
fn unordered_segment_eq(
    first_start: &ExactPoint3,
    first_end: &ExactPoint3,
    second_start: &ExactPoint3,
    second_end: &ExactPoint3,
) -> bool {
    (first_start == second_start && first_end == second_end)
        || (first_start == second_end && first_end == second_start)
}

/// Converts one finite binary64 matrix coefficient into its exact scalar
/// value. This is distinct from the common-unit point representation below:
/// multiplying two common-unit values would introduce an extra `2^1074`
/// scale and would not be an affine transform.
#[cfg(test)]
fn exact_binary64_scalar(value: f64) -> BigRational {
    exact_binary64(value) / BigRational::from_integer(BigInt::one() << 1074_usize)
}

/// Converts one finite binary64 coordinate into exact integer units of
/// `2^-1074`. `Point3` has already rejected non-finite values.
#[cfg(test)]
fn exact_binary64(value: f64) -> BigRational {
    let bits = value.to_bits();
    let negative = bits >> 63 != 0;
    let exponent = ((bits >> 52) & 0x7ff) as usize;
    let fraction = bits & ((1_u64 << 52) - 1);
    let (significand, shift) = if exponent == 0 {
        (fraction, 0)
    } else {
        (fraction | (1_u64 << 52), exponent - 1)
    };
    let mut integer = BigInt::from(significand) << shift;
    if negative {
        integer = -integer;
    }
    BigRational::from_integer(integer)
}

#[cfg(test)]
mod tests {
    use ori_domain::{
        CreasePattern, Edge, EdgeId, EdgeKind, Paper, Point2, ProjectId, Vertex, VertexId,
    };
    use ori_kinematics::{
        CanonicalHingeAngles, HingeAngle, MaterialTreeKinematicsModel, TreeKinematicsLimits,
    };
    use ori_topology::{FaceExtractionInput, analyze_faces};

    use crate::{StaticCollisionError, StaticCollisionLimits, prove_static_collision_geometry};

    use super::*;

    const TRIANGLE_PERMUTATIONS: [[usize; 3]; 6] = [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ];

    fn point(x: f64, y: f64, z: f64) -> Point3 {
        Point3::new(x, y, z).expect("finite test point")
    }

    fn triangle(points: [[f64; 3]; 3]) -> [Point3; 3] {
        points.map(|[x, y, z]| point(x, y, z))
    }

    fn vertex_id(index: u64) -> VertexId {
        serde_json::from_str(&format!("\"00000000-0000-4000-8000-{index:012x}\""))
            .expect("fixed vertex id")
    }

    fn edge_id(index: u64) -> EdgeId {
        serde_json::from_str(&format!("\"00000000-0000-4000-9000-{index:012x}\""))
            .expect("fixed edge id")
    }

    fn face_id(index: u64) -> FaceId {
        serde_json::from_str(&format!("\"00000000-0000-4000-a000-{index:012x}\""))
            .expect("fixed face id")
    }

    fn project_id() -> ProjectId {
        serde_json::from_str("\"00000000-0000-4000-b000-000000000001\"").expect("fixed project id")
    }

    fn domain_vertex(index: u64, x: f64, y: f64) -> Vertex {
        Vertex {
            id: vertex_id(index),
            position: Point2::new(x, y),
        }
    }

    fn domain_edge(index: u64, start: VertexId, end: VertexId, kind: EdgeKind) -> Edge {
        Edge {
            id: edge_id(index),
            start,
            end,
            kind,
        }
    }

    fn zero_thickness_limits() -> ZeroThicknessGeometryLimits {
        ZeroThicknessGeometryLimits {
            max_boundary_vertices_per_face: 64,
            max_total_boundary_vertices: 256,
            max_triangles_per_face: 62,
            max_total_triangles: 256,
            max_triangulation_work_per_face: 10_000_000,
            max_total_triangulation_work: 40_000_000,
            max_registry_authentication_work: 1_000_000,
            max_triangle_pairs_per_face_pair: 4_096,
            max_total_triangle_pairs: 16_384,
            max_boundary_relation_work_per_face_pair: 1_000_000,
            max_total_boundary_relation_work: 4_000_000,
            max_rational_input_bits: 4_096,
            max_total_rational_input_storage_bits: 16_777_216,
            max_total_rational_retained_clone_bits: 134_217_728,
            max_rational_operations: 10_000_000,
            max_rational_intermediate_bits: 32_768,
            max_rational_gcd_fallback_calls: 100_000,
            max_rational_gcd_fallback_input_bits: 536_870_912,
            max_rational_allocations: 1_000_000,
            max_rational_allocation_bits: 65_536,
            max_total_rational_allocation_bits: 1_073_741_824,
            max_rational_output_bits: 32_768,
            max_total_rational_output_bits: 33_554_432,
        }
    }

    fn corner_v_model_and_pose() -> (MaterialTreeKinematicsModel, MaterialTreePose) {
        let vertices = vec![
            domain_vertex(1, 0.0, 0.0),
            domain_vertex(2, 10.0, 0.0),
            domain_vertex(3, 10.0, 5.0),
            domain_vertex(4, 10.0, 10.0),
            domain_vertex(5, 5.0, 10.0),
            domain_vertex(6, 0.0, 10.0),
        ];
        let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
        let mut edges = (0..boundary.len())
            .map(|index| {
                domain_edge(
                    index as u64 + 1,
                    boundary[index],
                    boundary[(index + 1) % boundary.len()],
                    EdgeKind::Boundary,
                )
            })
            .collect::<Vec<_>>();
        let first_hinge = domain_edge(7, boundary[0], boundary[2], EdgeKind::Mountain);
        let second_hinge = domain_edge(8, boundary[0], boundary[4], EdgeKind::Valley);
        edges.extend([first_hinge.clone(), second_hinge.clone()]);
        let pattern = CreasePattern { vertices, edges };
        let paper = Paper {
            boundary_vertices: boundary,
            ..Paper::default()
        };
        let report = analyze_faces(FaceExtractionInput {
            identity_namespace: project_id(),
            source_revision: 11,
            paper: &paper,
            pattern: &pattern,
        });
        assert!(report.issues.is_empty(), "{:?}", report.issues);
        let topology = report.snapshot.expect("corner V topology");
        let model = MaterialTreeKinematicsModel::prepare(
            &pattern,
            &paper,
            &topology,
            TreeKinematicsLimits::default(),
        )
        .expect("corner V model");
        let angles = CanonicalHingeAngles::new(vec![
            HingeAngle::new(first_hinge.id, 0.0).expect("first angle"),
            HingeAngle::new(second_hinge.id, 0.0).expect("second angle"),
        ])
        .expect("canonical V angles");
        let pose = model
            .solve(Some(model.face_ids()[0]), &angles)
            .expect("planar V pose");
        (model, pose)
    }

    fn midpoint_mountain_model() -> MaterialTreeKinematicsModel {
        let vertices = vec![
            domain_vertex(21, 0.0, 0.0),
            domain_vertex(22, 5.0, 0.0),
            domain_vertex(23, 10.0, 0.0),
            domain_vertex(24, 10.0, 10.0),
            domain_vertex(25, 0.0, 10.0),
        ];
        let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
        let mut edges = (0..boundary.len())
            .map(|index| {
                domain_edge(
                    index as u64 + 21,
                    boundary[index],
                    boundary[(index + 1) % boundary.len()],
                    EdgeKind::Boundary,
                )
            })
            .collect::<Vec<_>>();
        edges.push(domain_edge(
            26,
            boundary[1],
            boundary[3],
            EdgeKind::Mountain,
        ));
        edges.push(domain_edge(
            27,
            boundary[1],
            boundary[4],
            EdgeKind::Mountain,
        ));
        let pattern = CreasePattern { vertices, edges };
        let paper = Paper {
            boundary_vertices: boundary,
            ..Paper::default()
        };
        let report = analyze_faces(FaceExtractionInput {
            identity_namespace: project_id(),
            source_revision: 12,
            paper: &paper,
            pattern: &pattern,
        });
        assert!(report.issues.is_empty(), "{:?}", report.issues);
        MaterialTreeKinematicsModel::prepare(
            &pattern,
            &paper,
            &report.snapshot.expect("midpoint topology"),
            TreeKinematicsLimits::default(),
        )
        .expect("midpoint mountain model")
    }

    fn only_non_hinge_face_pair(model: &MaterialTreeKinematicsModel) -> [FaceId; 2] {
        let mut pairs = model
            .face_ids()
            .iter()
            .copied()
            .enumerate()
            .flat_map(|(first_index, first)| {
                model.face_ids()[first_index + 1..]
                    .iter()
                    .copied()
                    .map(move |second| [first, second])
            })
            .filter(|pair| {
                !model.hinges().iter().any(|hinge| {
                    let mut hinge_pair = [hinge.left_face(), hinge.right_face()];
                    hinge_pair.sort_unstable_by_key(FaceId::canonical_bytes);
                    hinge_pair == *pair
                })
            })
            .collect::<Vec<_>>();
        assert_eq!(pairs.len(), 1, "three-face V has one non-hinge pair");
        pairs.pop().expect("non-hinge pair")
    }

    fn solve_two_hinge_pose(
        model: &MaterialTreeKinematicsModel,
        angle_degrees: [f64; 2],
    ) -> MaterialTreePose {
        assert_eq!(model.hinges().len(), angle_degrees.len());
        let angles = CanonicalHingeAngles::new(
            model
                .hinges()
                .iter()
                .zip(angle_degrees)
                .map(|(hinge, angle)| {
                    HingeAngle::new(hinge.edge(), angle).expect("two-hinge fixture angle")
                })
                .collect(),
        )
        .expect("canonical two-hinge fixture angles");
        model
            .solve(Some(model.face_ids()[0]), &angles)
            .expect("two-hinge fixture pose")
    }

    fn only_vertex_shared_outer_pair(
        pose: &MaterialTreePose,
        analysis: &AuthenticatedZeroThicknessPose,
    ) -> (AuthenticatedTopology, PairDispatch) {
        let mut found = None;
        for first in 0..analysis.faces.len() {
            for second in (first + 1)..analysis.faces.len() {
                let first_face = &analysis.faces[first];
                let second_face = &analysis.faces[second];
                let shared_vertices = first_face
                    .boundary
                    .iter()
                    .filter(|vertex| {
                        second_face
                            .boundary
                            .iter()
                            .any(|candidate| candidate.id == vertex.id)
                    })
                    .count();
                let shared_edges = first_face
                    .edges
                    .iter()
                    .filter(|edge| second_face.edges.contains(edge))
                    .count();
                if shared_vertices != 1 || shared_edges != 0 {
                    continue;
                }
                let topology = authenticate_face_pair_topology(pose, first_face, second_face)
                    .expect("authenticated outer-pair topology");
                let dispatch = analysis
                    .dispatch_pair(first, second)
                    .expect("complete authenticated outer pair");
                assert!(
                    found.replace((topology, dispatch)).is_none(),
                    "fixture must have exactly one vertex-only outer pair"
                );
            }
        }
        found.expect("fixture vertex-only outer pair")
    }

    fn rest_boundary(points: &[[f64; 2]]) -> Vec<RestBoundaryVertex> {
        points
            .iter()
            .enumerate()
            .map(|(index, [x, z])| RestBoundaryVertex {
                id: vertex_id(index as u64 + 1),
                point: ExactPoint3::from_point(point(*x, 0.0, *z)),
            })
            .collect()
    }

    /// Synthetic geometry for testing the private face-level aggregate only.
    ///
    /// Production never accepts this constructor: topology is authenticated
    /// separately from `MaterialTreePose`. Distinct per-face ID ranges keep
    /// even these untrusted fixtures consistent with `NoSharedFeature`.
    fn synthetic_untrusted_face(
        id: FaceId,
        boundary_points: &[[f64; 3]],
        triangle_indices: &[[usize; 3]],
    ) -> AuthenticatedFace {
        synthetic_untrusted_face_with_material_normal(
            id,
            boundary_points,
            triangle_indices,
            [0.0, 1.0, 0.0],
        )
    }

    fn synthetic_untrusted_face_with_material_normal(
        id: FaceId,
        boundary_points: &[[f64; 3]],
        triangle_indices: &[[usize; 3]],
        material_normal: [f64; 3],
    ) -> AuthenticatedFace {
        let id_offset = u64::from(id.canonical_bytes()[15]) * 100;
        let boundary = boundary_points
            .iter()
            .enumerate()
            .map(|(index, [x, y, z])| AuthenticatedBoundaryVertex {
                id: vertex_id(id_offset + index as u64 + 1),
                rest: point(*x, *y, *z),
                current: ExactPoint3::from_point(point(*x, *y, *z)),
            })
            .collect::<Vec<_>>();
        let edges = (0..boundary.len())
            .map(|index| edge_id(id_offset + index as u64 + 1))
            .collect::<Vec<_>>();
        let triangles = triangle_indices
            .iter()
            .map(|indices| indices.map(|index| boundary[index].current.clone()))
            .collect();
        AuthenticatedFace {
            id,
            boundary,
            edges,
            triangles,
            material_normal: ExactVector3 {
                coordinates: material_normal.map(exact_binary64_scalar),
            },
        }
    }

    fn no_shared(first: [[f64; 3]; 3], second: [[f64; 3]; 3]) -> PairDispatch {
        dispatch_authenticated_zero_thickness_pair(&AuthenticatedTrianglePair {
            first: triangle(first),
            second: triangle(second),
            topology: AuthenticatedTopology::NoSharedFeature,
        })
    }

    const fn single_dispatch(
        evidence: IntersectionEvidenceV2,
        decision: TopologyContactDecision,
    ) -> PairDispatch {
        single_dispatch_for(TopologyRelation::NoSharedFeature, evidence, decision)
    }

    const fn single_dispatch_for(
        topology: TopologyRelation,
        evidence: IntersectionEvidenceV2,
        decision: TopologyContactDecision,
    ) -> PairDispatch {
        PairDispatch {
            topology,
            evidence,
            decision,
            expected_triangle_pairs: 1,
            analyzed_triangle_pairs: 1,
        }
    }

    #[test]
    fn exact_triangulation_covers_convex_concave_and_collinear_boundaries() {
        let cases = [
            (
                rest_boundary(&[[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0]]),
                2,
            ),
            (
                rest_boundary(&[[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [2.0, 2.0], [0.0, 4.0]]),
                3,
            ),
            (
                rest_boundary(&[[0.0, 0.0], [2.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0]]),
                2,
            ),
        ];

        for (boundary, expected_count) in cases {
            let triangles = triangulate_rest_boundary(
                &boundary,
                boundary.len(),
                boundary.len() - 2,
                usize::MAX,
            )
            .expect("simple boundary");
            assert_eq!(triangles.len(), expected_count);
            assert!(triangles.iter().all(|triangle| {
                !ExactTriangle::from_exact_points(
                    triangle.map(|index| boundary[index].point.clone()),
                )
                .normal
                .is_zero()
            }));
        }
    }

    #[test]
    fn exact_triangulation_is_cycle_invariant_and_rejects_invalid_or_underbounded_work() {
        let boundary = rest_boundary(&[[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [2.0, 2.0], [0.0, 4.0]]);
        let expected = triangulate_rest_boundary(&boundary, 5, 3, usize::MAX)
            .expect("baseline concave triangulation")
            .into_iter()
            .map(|triangle| triangle.map(|index| boundary[index].id.canonical_bytes()))
            .collect::<Vec<_>>();
        for rotation in 0..boundary.len() {
            let mut rotated = boundary.clone();
            rotated.rotate_left(rotation);
            let actual = triangulate_rest_boundary(&rotated, 5, 3, usize::MAX)
                .expect("rotated concave triangulation")
                .into_iter()
                .map(|triangle| triangle.map(|index| rotated[index].id.canonical_bytes()))
                .collect::<Vec<_>>();
            assert_eq!(actual, expected, "rotation {rotation}");
        }

        assert_eq!(
            triangulate_rest_boundary(&boundary, 4, 3, usize::MAX),
            Err(ZeroThicknessAnalysisError::ResourceLimitExceeded)
        );
        assert_eq!(
            triangulate_rest_boundary(&boundary, 5, 2, usize::MAX),
            Err(ZeroThicknessAnalysisError::ResourceLimitExceeded)
        );
        let required_work = estimated_triangulation_work(boundary.len()).expect("small work");
        assert_eq!(
            triangulate_rest_boundary(&boundary, 5, 3, required_work - 1),
            Err(ZeroThicknessAnalysisError::ResourceLimitExceeded)
        );

        let bow_tie = rest_boundary(&[[0.0, 0.0], [4.0, 4.0], [0.0, 4.0], [4.0, 0.0]]);
        assert_eq!(
            triangulate_rest_boundary(&bow_tie, 4, 2, usize::MAX),
            Err(ZeroThicknessAnalysisError::EvidenceUnavailable)
        );
        let mut duplicate_coordinate =
            rest_boundary(&[[0.0, 0.0], [4.0, 0.0], [4.0, 0.0], [0.0, 4.0]]);
        duplicate_coordinate[2].id = vertex_id(99);
        assert_eq!(
            triangulate_rest_boundary(&duplicate_coordinate, 4, 2, usize::MAX),
            Err(ZeroThicknessAnalysisError::EvidenceUnavailable)
        );
        assert_eq!(
            estimated_triangulation_work(usize::MAX),
            Err(ZeroThicknessAnalysisError::ResourceLimitExceeded)
        );
        assert_eq!(
            estimated_boundary_relation_work(
                usize::MAX,
                usize::MAX,
                usize::MAX,
                usize::MAX,
                usize::MAX,
                usize::MAX,
            ),
            Err(ZeroThicknessAnalysisError::ResourceLimitExceeded)
        );
        assert_eq!(
            estimated_registry_authentication_work(usize::MAX, usize::MAX, usize::MAX),
            Err(ZeroThicknessAnalysisError::ResourceLimitExceeded)
        );
    }

    #[test]
    fn authenticated_corner_v_proves_vertex_only_contact_and_blocks_both_hinges() {
        let (_model, pose) = corner_v_model_and_pose();
        let analysis = prepare_authenticated_zero_thickness_pose(&pose, zero_thickness_limits())
            .expect("authenticated corner V geometry");
        assert_eq!(analysis.faces.len(), 3);

        let mut allowed_vertices = 0;
        let mut required_hinges = 0;
        let mut expected_triangle_pairs = 0_usize;
        let mut analyzed_triangle_pairs = 0_usize;
        for first in 0..analysis.faces.len() {
            for second in (first + 1)..analysis.faces.len() {
                let dispatch = analysis
                    .dispatch_pair(first, second)
                    .expect("complete authenticated pair");
                assert!(dispatch.has_complete_coverage());
                expected_triangle_pairs += dispatch.expected_triangle_pairs();
                analyzed_triangle_pairs += dispatch.analyzed_triangle_pairs();
                match dispatch.decision() {
                    TopologyContactDecision::AllowedSharedVertexContact => {
                        allowed_vertices += 1;
                        assert_eq!(
                            dispatch.evidence(),
                            IntersectionEvidenceV2::SharedFeatureContact
                        );
                    }
                    TopologyContactDecision::RequiresHingeModel => {
                        required_hinges += 1;
                        assert_eq!(
                            dispatch.evidence(),
                            IntersectionEvidenceV2::SharedFeatureContact
                        );
                    }
                    other => panic!("unexpected corner V classification: {other:?}"),
                }
            }
        }
        assert_eq!(allowed_vertices, 1);
        assert_eq!(required_hinges, 2);
        assert_eq!(expected_triangle_pairs, analysis.total_triangle_pairs());
        assert_eq!(analyzed_triangle_pairs, analysis.total_triangle_pairs());
        assert_eq!(
            analysis.dispatch_pair(0, 0),
            Err(ZeroThicknessAnalysisError::EvidenceUnavailable)
        );
        assert_eq!(
            analysis.dispatch_pair(1, 0),
            Err(ZeroThicknessAnalysisError::EvidenceUnavailable)
        );
        assert_eq!(
            analysis.dispatch_pair(0, analysis.faces.len()),
            Err(ZeroThicknessAnalysisError::EvidenceUnavailable)
        );
    }

    #[test]
    fn authenticated_corner_v_shared_vertex_stays_nonpenetrating_across_reported_angles() {
        let (model, _planar_pose) = corner_v_model_and_pose();
        for angle_degrees in [
            [10.0, 0.0],
            [0.0, 10.0],
            [45.0, 45.0],
            [90.0, 90.0],
            [91.0, 91.0],
            [135.0, 135.0],
            [179.0, 179.0],
        ] {
            let pose = solve_two_hinge_pose(&model, angle_degrees);
            let analysis =
                prepare_authenticated_zero_thickness_pose(&pose, zero_thickness_limits())
                    .expect("authenticated folded V geometry");
            let (topology, dispatch) = only_vertex_shared_outer_pair(&pose, &analysis);
            assert!(
                matches!(topology, AuthenticatedTopology::SharedVertex(_)),
                "{angle_degrees:?}: {topology:?}"
            );
            assert!(dispatch.has_complete_coverage(), "{angle_degrees:?}");
            assert_eq!(
                dispatch.evidence(),
                IntersectionEvidenceV2::SharedFeatureContact,
                "{angle_degrees:?}: {dispatch:?}"
            );
            assert_eq!(
                dispatch.decision(),
                TopologyContactDecision::AllowedSharedVertexContact,
                "{angle_degrees:?}: {dispatch:?}"
            );
            for first in 0..analysis.faces.len() {
                for second in (first + 1)..analysis.faces.len() {
                    let pair = analysis
                        .dispatch_pair(first, second)
                        .expect("complete corner V pair");
                    assert!(
                        matches!(
                            pair.decision(),
                            TopologyContactDecision::AllowedSharedVertexContact
                                | TopologyContactDecision::RequiresHingeModel
                                | TopologyContactDecision::Indeterminate
                        ),
                        "{angle_degrees:?}: {pair:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn exact_affine_pose_image_preserves_whole_face_coplanarity() {
        let (model, _planar_pose) = corner_v_model_and_pose();
        let angles = CanonicalHingeAngles::new(
            model
                .hinges()
                .iter()
                .zip([37.0, 73.0])
                .map(|(hinge, angle)| HingeAngle::new(hinge.edge(), angle).expect("angle"))
                .collect(),
        )
        .expect("canonical angles");
        let pose = model
            .solve(Some(model.face_ids()[0]), &angles)
            .expect("noncardinal pose");
        let mut checked_nontriangle = false;
        for face in pose.face_ids().iter().copied() {
            let boundary = pose.face_boundary(face).expect("source boundary");
            if boundary.vertices().len() < 4 {
                continue;
            }
            let transform = ExactAffineTransform::from_transform(
                pose.face_transform(face).expect("face transform"),
            );
            let points = boundary
                .vertices()
                .iter()
                .map(|vertex| {
                    transform.apply_point(&ExactPoint3::from_point(
                        pose.vertex_position(*vertex).expect("rest position"),
                    ))
                })
                .collect::<Vec<_>>();
            let mut plane = None;
            'triples: for first in 0..points.len() {
                for second in (first + 1)..points.len() {
                    for third in (second + 1)..points.len() {
                        let triangle = ExactTriangle::from_exact_points([
                            points[first].clone(),
                            points[second].clone(),
                            points[third].clone(),
                        ]);
                        if !triangle.normal.is_zero() {
                            plane = Some(triangle);
                            break 'triples;
                        }
                    }
                }
            }
            let plane = plane.expect("nondegenerate face plane");
            assert!(
                points
                    .iter()
                    .all(|point| plane.signed_plane_distance(point).is_zero())
            );
            checked_nontriangle = true;
        }
        assert!(checked_nontriangle);
    }

    #[test]
    fn noncardinal_slanted_hinge_disagreement_is_explicitly_indeterminate() {
        let (model, _planar_pose) = corner_v_model_and_pose();
        let angles = CanonicalHingeAngles::new(
            model
                .hinges()
                .iter()
                .zip([37.0, 0.0])
                .map(|(hinge, angle)| HingeAngle::new(hinge.edge(), angle).expect("angle"))
                .collect(),
        )
        .expect("canonical angles");
        let pose = model
            .solve(Some(model.face_ids()[0]), &angles)
            .expect("noncardinal slanted-hinge pose");
        let analysis = prepare_authenticated_zero_thickness_pose(&pose, zero_thickness_limits())
            .expect("authenticated faces");
        let results = (0..analysis.faces.len())
            .flat_map(|first| {
                ((first + 1)..analysis.faces.len()).map(move |second| (first, second))
            })
            .map(|(first, second)| analysis.dispatch_pair(first, second))
            .collect::<Vec<_>>();
        assert_eq!(
            results
                .iter()
                .filter(|result| {
                    result.as_ref().is_ok_and(|dispatch| {
                        dispatch.decision() == TopologyContactDecision::AllowedSharedVertexContact
                    })
                })
                .count(),
            1,
            "{results:?}"
        );
        assert!(
            results.iter().any(|result| {
                matches!(
                    result,
                    Ok(PairDispatch {
                        evidence: IntersectionEvidenceV2::Indeterminate,
                        decision: TopologyContactDecision::Indeterminate,
                        ..
                    })
                )
            }),
            "non-watertight slanted hinge must remain explicitly indeterminate: {results:?}"
        );
        assert!(
            results.iter().all(Result::is_ok),
            "authenticated source pairs must retain complete diagnostic coverage: {results:?}"
        );
    }

    #[test]
    fn authenticated_corner_v_at_full_fold_reports_real_area_overlap() {
        let (model, _planar_pose) = corner_v_model_and_pose();
        let angles = CanonicalHingeAngles::new(
            model
                .hinges()
                .iter()
                .map(|hinge| HingeAngle::new(hinge.edge(), 180.0).expect("full fold"))
                .collect(),
        )
        .expect("canonical full-fold angles");
        let pose = model
            .solve(Some(model.face_ids()[0]), &angles)
            .expect("full-fold V pose");
        let analysis = prepare_authenticated_zero_thickness_pose(&pose, zero_thickness_limits())
            .expect("authenticated full-fold faces");
        assert!(analysis.is_for_pose(&pose));
        assert_eq!(analysis.face_count(), pose.face_ids().len());
        for (index, face) in pose.face_ids().iter().copied().enumerate() {
            assert_eq!(analysis.face_id(index), Some(face));
            assert_eq!(
                analysis.face_boundary_vertex_count(index),
                pose.face_boundary(face)
                    .map(|boundary| boundary.vertices().len())
            );
        }
        let same_angle_aba_pose = solve_two_hinge_pose(&model, [180.0, 180.0]);
        assert!(
            !analysis.is_for_pose(&same_angle_aba_pose),
            "an equal-angle re-solve must not inherit the authenticated aggregate"
        );
        let mut shared_vertex_pairs = 0;
        for first in 0..analysis.faces.len() {
            for second in (first + 1)..analysis.faces.len() {
                let first_face = &analysis.faces[first];
                let second_face = &analysis.faces[second];
                let shared_vertices = first_face
                    .boundary
                    .iter()
                    .filter(|vertex| {
                        second_face
                            .boundary
                            .iter()
                            .any(|candidate| candidate.id == vertex.id)
                    })
                    .count();
                let shared_edges = first_face
                    .edges
                    .iter()
                    .filter(|edge| second_face.edges.contains(edge))
                    .count();
                if shared_vertices != 1 || shared_edges != 0 {
                    continue;
                }
                let topology = authenticate_face_pair_topology(&pose, first_face, second_face)
                    .expect("authenticated full-fold topology");
                if !matches!(topology, AuthenticatedTopology::SharedVertex(_)) {
                    continue;
                }
                shared_vertex_pairs += 1;
                let dispatch = analysis
                    .dispatch_pair(first, second)
                    .expect("complete full-fold vertex pair");
                assert_eq!(
                    dispatch.evidence(),
                    IntersectionEvidenceV2::CoplanarAreaOverlap
                );
                assert_eq!(dispatch.decision(), TopologyContactDecision::Penetrating);
            }
        }
        assert_eq!(shared_vertex_pairs, 1);
    }

    #[test]
    fn public_full_fold_affirmative_honors_every_additive_ledger_exactly() {
        let (model, _planar_pose) = corner_v_model_and_pose();
        let pose = solve_two_hinge_pose(&model, [180.0, 180.0]);
        let baseline = prepare_authenticated_zero_thickness_pose(&pose, zero_thickness_limits())
            .expect("baseline full-fold exact aggregate");
        let work = baseline.work();
        let total_boundary_vertices = baseline
            .faces
            .iter()
            .map(|face| face.boundary.len())
            .sum::<usize>();
        let total_triangulation_work = baseline
            .faces
            .iter()
            .map(|face| estimated_triangulation_work(face.boundary.len()).expect("small face"))
            .sum::<usize>();
        let unordered_face_pairs = baseline.pair_dispatches.len();
        let expected = StaticCollisionError::ProvenTransversalPenetration {
            expected_unordered_face_pairs: unordered_face_pairs,
            proven_transversal_pairs: 1,
            first_proven_transversal_pair: only_non_hinge_face_pair(&model),
        };

        type LimitSetter = fn(&mut StaticCollisionLimits, usize);
        let boundaries: [(&str, usize, LimitSetter); 16] = [
            ("faces", baseline.faces.len(), |limits, value| {
                limits.max_faces = value
            }),
            (
                "unordered_face_pairs",
                unordered_face_pairs,
                |limits, value| limits.max_unordered_face_pairs = value,
            ),
            (
                "total_boundary_vertices",
                total_boundary_vertices,
                |limits, value| limits.max_total_boundary_vertices = value,
            ),
            ("total_triangles", work.total_triangles, |limits, value| {
                limits.max_total_triangles = value
            }),
            (
                "total_triangulation_work",
                total_triangulation_work,
                |limits, value| limits.max_total_triangulation_work = value,
            ),
            (
                "registry_authentication_work",
                work.registry_authentication_work,
                |limits, value| limits.max_registry_authentication_work = value,
            ),
            (
                "total_triangle_pairs",
                work.total_triangle_pairs,
                |limits, value| limits.max_total_triangle_pairs = value,
            ),
            (
                "total_boundary_relation_work",
                work.total_boundary_relation_work,
                |limits, value| limits.max_total_boundary_relation_work = value,
            ),
            (
                "total_rational_input_storage_bits",
                work.total_rational_input_storage_bits,
                |limits, value| limits.max_total_rational_input_storage_bits = value,
            ),
            (
                "total_rational_retained_clone_bits",
                work.total_rational_retained_clone_bits,
                |limits, value| limits.max_total_rational_retained_clone_bits = value,
            ),
            (
                "rational_operations",
                work.rational_operations,
                |limits, value| limits.max_rational_operations = value,
            ),
            (
                "rational_gcd_fallback_calls",
                work.rational_gcd_fallback_calls,
                |limits, value| limits.max_rational_gcd_fallback_calls = value,
            ),
            (
                "rational_gcd_fallback_input_bits",
                work.rational_gcd_fallback_input_bits,
                |limits, value| limits.max_rational_gcd_fallback_input_bits = value,
            ),
            (
                "rational_allocations",
                work.rational_allocations,
                |limits, value| limits.max_rational_allocations = value,
            ),
            (
                "total_rational_allocation_bits",
                work.total_rational_allocation_bits,
                |limits, value| limits.max_total_rational_allocation_bits = value,
            ),
            (
                "total_rational_output_bits",
                work.total_rational_output_bits,
                |limits, value| limits.max_total_rational_output_bits = value,
            ),
        ];

        for (name, required, set) in boundaries {
            assert!(required > 0, "{name} must be exercised");
            let mut exact = StaticCollisionLimits::default();
            set(&mut exact, required);
            assert_eq!(
                prove_static_collision_geometry(&model, &pose, 0.0, exact)
                    .expect_err("exact public budget must retain the blocking affirmative"),
                expected,
                "{name} exact limit"
            );

            let mut one_short = StaticCollisionLimits::default();
            set(&mut one_short, required - 1);
            assert_eq!(
                prove_static_collision_geometry(&model, &pose, 0.0, one_short)
                    .expect_err("one-short public budget must fail atomically"),
                StaticCollisionError::ResourceLimitExceeded,
                "{name} one-short limit"
            );
        }
    }

    #[test]
    fn midpoint_mountain_pair_baseline_holds_deep_angles_until_exact_tree_connection() {
        let model = midpoint_mountain_model();
        for angle in [10.0, 45.0, 90.0, 91.0, 135.0, 179.0, 180.0] {
            let pose = solve_two_hinge_pose(&model, [angle, angle]);
            let analysis =
                prepare_authenticated_zero_thickness_pose(&pose, zero_thickness_limits())
                    .expect("authenticated midpoint faces");
            let (topology, dispatch) = only_vertex_shared_outer_pair(&pose, &analysis);
            assert!(dispatch.has_complete_coverage(), "{angle}: {dispatch:?}");

            // This is a fail-closed baseline for the independently rounded
            // binary64 tree pose, not the final geometric classification.
            // The exact tree must recover shared-vertex contact at 10/45,
            // keep 90/91 blocking without a pose-mismatch reason, prove
            // TransversalCrossing/Penetrating at 135/179, and prove the
            // full-fold CoplanarAreaOverlap/Penetrating at 180 degrees.
            assert_eq!(
                topology,
                AuthenticatedTopology::SharedVertexPoseMismatch,
                "{angle}: {topology:?}"
            );
            assert_eq!(
                dispatch.evidence(),
                IntersectionEvidenceV2::Indeterminate,
                "{angle}: {dispatch:?}"
            );
            assert_eq!(
                dispatch.decision(),
                TopologyContactDecision::Indeterminate,
                "{angle}: {dispatch:?}"
            );
        }
    }

    #[test]
    fn reported_three_face_poses_remain_blocking_in_public_proof_at_all_baseline_thicknesses() {
        let (corner_model, _planar_corner) = corner_v_model_and_pose();
        let corner_pose = solve_two_hinge_pose(&corner_model, [10.0, 0.0]);
        let midpoint_model = midpoint_mountain_model();
        let midpoint_pose = solve_two_hinge_pose(&midpoint_model, [135.0, 135.0]);

        for thickness in [0.0, 0.1, 1.0, 3.0] {
            assert_eq!(
                prove_static_collision_geometry(
                    &corner_model,
                    &corner_pose,
                    thickness,
                    StaticCollisionLimits::default(),
                )
                .expect_err("corner pose must remain blocking"),
                StaticCollisionError::PairEvidenceUnavailable {
                    expected_unordered_face_pairs: 3,
                },
                "corner-v:{thickness}"
            );
        }
        assert_eq!(
            prove_static_collision_geometry(
                &midpoint_model,
                &midpoint_pose,
                0.0,
                StaticCollisionLimits::default(),
            )
            .expect_err("midpoint crossing must remain blocking"),
            StaticCollisionError::ProvenTransversalPenetration {
                expected_unordered_face_pairs: 3,
                proven_transversal_pairs: 1,
                first_proven_transversal_pair: only_non_hinge_face_pair(&midpoint_model),
            },
        );
        for thickness in [0.1, 1.0, 3.0] {
            assert_eq!(
                prove_static_collision_geometry(
                    &midpoint_model,
                    &midpoint_pose,
                    thickness,
                    StaticCollisionLimits::default(),
                )
                .expect_err("positive-thickness midpoint pose must remain blocking"),
                StaticCollisionError::ProvenPositiveThicknessPenetration {
                    expected_unordered_face_pairs: 3,
                    proven_positive_thickness_pairs: 1,
                    first_proven_positive_thickness_pair: only_non_hinge_face_pair(&midpoint_model),
                },
                "midpoint-mountain:{thickness}"
            );
        }
    }

    #[test]
    fn authenticated_pose_limits_are_checked_at_every_one_short_boundary() {
        let (_model, pose) = corner_v_model_and_pose();
        let baseline = prepare_authenticated_zero_thickness_pose(&pose, zero_thickness_limits())
            .expect("baseline authenticated geometry");
        let maximum_boundary = baseline
            .faces
            .iter()
            .map(|face| face.boundary.len())
            .max()
            .expect("face");
        let total_boundary = baseline
            .faces
            .iter()
            .map(|face| face.boundary.len())
            .sum::<usize>();
        let maximum_triangles = baseline
            .faces
            .iter()
            .map(|face| face.triangles.len())
            .max()
            .expect("face");
        let total_triangles = baseline
            .faces
            .iter()
            .map(|face| face.triangles.len())
            .sum::<usize>();
        let maximum_triangulation_work = baseline
            .faces
            .iter()
            .map(|face| estimated_triangulation_work(face.boundary.len()).expect("small face work"))
            .max()
            .expect("face");
        let total_triangulation_work = baseline
            .faces
            .iter()
            .map(|face| estimated_triangulation_work(face.boundary.len()).expect("small face work"))
            .sum::<usize>();
        let pair_metrics = (0..baseline.faces.len())
            .flat_map(|first| {
                ((first + 1)..baseline.faces.len()).map(move |second| (first, second))
            })
            .map(|(first, second)| {
                let triangle_pairs =
                    baseline.faces[first].triangles.len() * baseline.faces[second].triangles.len();
                let boundary_work = estimated_boundary_relation_work(
                    triangle_pairs,
                    baseline.faces[first].triangles.len(),
                    baseline.faces[second].triangles.len(),
                    baseline.faces[first].boundary.len(),
                    baseline.faces[second].boundary.len(),
                    pose.hinges().len(),
                )
                .expect("small boundary work");
                (triangle_pairs, boundary_work)
            })
            .collect::<Vec<_>>();
        let maximum_triangle_pairs = pair_metrics
            .iter()
            .map(|(triangle_pairs, _)| *triangle_pairs)
            .max()
            .expect("pair");
        let maximum_boundary_work = pair_metrics
            .iter()
            .map(|(_, boundary_work)| *boundary_work)
            .max()
            .expect("pair");
        let total_boundary_work = pair_metrics
            .iter()
            .map(|(_, boundary_work)| *boundary_work)
            .sum::<usize>();
        assert_eq!(total_boundary, 10);
        assert_eq!(pose.hinges().len(), 2);
        assert_eq!(baseline.faces.len(), 3);
        const REGISTRY_AUTHENTICATION_WORK_CONTRACT: usize =
            10 * 10 + 2 * 10 * 2 + 2 * 2 + 2 * 3 * 2 + 2 * 16 + 10 + 3;
        assert_eq!(REGISTRY_AUTHENTICATION_WORK_CONTRACT, 201);
        assert_eq!(
            estimated_registry_authentication_work(10, 2, 3),
            Ok(REGISTRY_AUTHENTICATION_WORK_CONTRACT)
        );

        for limits in [
            ZeroThicknessGeometryLimits {
                max_boundary_vertices_per_face: maximum_boundary - 1,
                ..zero_thickness_limits()
            },
            ZeroThicknessGeometryLimits {
                max_total_boundary_vertices: total_boundary - 1,
                ..zero_thickness_limits()
            },
            ZeroThicknessGeometryLimits {
                max_triangles_per_face: maximum_triangles - 1,
                ..zero_thickness_limits()
            },
            ZeroThicknessGeometryLimits {
                max_total_triangles: total_triangles - 1,
                ..zero_thickness_limits()
            },
            ZeroThicknessGeometryLimits {
                max_triangulation_work_per_face: maximum_triangulation_work - 1,
                ..zero_thickness_limits()
            },
            ZeroThicknessGeometryLimits {
                max_total_triangulation_work: total_triangulation_work - 1,
                ..zero_thickness_limits()
            },
            ZeroThicknessGeometryLimits {
                max_registry_authentication_work: REGISTRY_AUTHENTICATION_WORK_CONTRACT - 1,
                ..zero_thickness_limits()
            },
            ZeroThicknessGeometryLimits {
                max_triangle_pairs_per_face_pair: maximum_triangle_pairs - 1,
                ..zero_thickness_limits()
            },
            ZeroThicknessGeometryLimits {
                max_total_triangle_pairs: baseline.total_triangle_pairs() - 1,
                ..zero_thickness_limits()
            },
            ZeroThicknessGeometryLimits {
                max_boundary_relation_work_per_face_pair: maximum_boundary_work - 1,
                ..zero_thickness_limits()
            },
            ZeroThicknessGeometryLimits {
                max_total_boundary_relation_work: total_boundary_work - 1,
                ..zero_thickness_limits()
            },
        ] {
            assert!(matches!(
                prepare_authenticated_zero_thickness_pose(&pose, limits),
                Err(ZeroThicknessAnalysisError::ResourceLimitExceeded)
            ));
        }
    }

    #[test]
    fn rational_pose_meter_accepts_every_exact_limit_and_rejects_every_one_short_limit() {
        fn pair_table(
            analysis: &AuthenticatedZeroThicknessPose<'_>,
        ) -> Result<Vec<PairDispatch>, ZeroThicknessAnalysisError> {
            let mut table = Vec::new();
            for first in 0..analysis.faces.len() {
                for second in (first + 1)..analysis.faces.len() {
                    table.push(analysis.dispatch_pair(first, second)?);
                }
            }
            Ok(table)
        }

        let (_model, pose) = corner_v_model_and_pose();
        let baseline = prepare_authenticated_zero_thickness_pose(&pose, zero_thickness_limits())
            .expect("baseline metered geometry");
        let baseline_table = pair_table(&baseline).expect("baseline complete pair table");
        let work = baseline.rational_work().clone();
        let mut lower = 0_usize;
        let mut upper = work.max_preflight_bits.max(work.max_intermediate_bits);
        while lower < upper {
            let candidate = lower + (upper - lower) / 2;
            let limits = ZeroThicknessGeometryLimits {
                max_rational_intermediate_bits: candidate,
                ..zero_thickness_limits()
            };
            if prepare_authenticated_zero_thickness_pose(&pose, limits).is_ok() {
                upper = candidate;
            } else {
                lower = candidate + 1;
            }
        }
        let required_intermediate = lower;

        type LimitSetter = fn(&mut ZeroThicknessGeometryLimits, usize);
        let boundaries: [(&str, usize, LimitSetter); 10] = [
            ("input_bits", work.max_input_bits, |limits, value| {
                limits.max_rational_input_bits = value
            }),
            (
                "total_input_storage_bits",
                work.total_input_storage_bits,
                |limits, value| limits.max_total_rational_input_storage_bits = value,
            ),
            (
                "total_retained_clone_bits",
                work.total_retained_clone_bits,
                |limits, value| limits.max_total_rational_retained_clone_bits = value,
            ),
            ("operations", work.operations, |limits, value| {
                limits.max_rational_operations = value
            }),
            (
                "rational_allocations",
                work.rational_allocations,
                |limits, value| limits.max_rational_allocations = value,
            ),
            (
                "rational_allocation_bits",
                work.max_rational_allocation_bits,
                |limits, value| limits.max_rational_allocation_bits = value,
            ),
            (
                "total_rational_allocation_bits",
                work.total_rational_allocation_bits,
                |limits, value| limits.max_total_rational_allocation_bits = value,
            ),
            (
                "intermediate_bits",
                required_intermediate,
                |limits, value| limits.max_rational_intermediate_bits = value,
            ),
            ("output_bits", work.max_output_bits, |limits, value| {
                limits.max_rational_output_bits = value
            }),
            (
                "total_output_bits",
                work.total_output_bits,
                |limits, value| limits.max_total_rational_output_bits = value,
            ),
        ];

        for (name, required, set) in boundaries {
            assert!(required > 0, "{name} must be exercised by the fixture");
            let mut exact = zero_thickness_limits();
            set(&mut exact, required);
            let exact_analysis = prepare_authenticated_zero_thickness_pose(&pose, exact)
                .unwrap_or_else(|error| {
                    panic!("{name} exact limit {required} must succeed: {error:?}")
                });
            assert_eq!(
                pair_table(&exact_analysis).expect("exact-limit pair table"),
                baseline_table,
                "{name} exact limit must preserve every classification"
            );

            let mut one_short = zero_thickness_limits();
            set(&mut one_short, required - 1);
            assert!(
                matches!(
                    prepare_authenticated_zero_thickness_pose(&pose, one_short),
                    Err(ZeroThicknessAnalysisError::ResourceLimitExceeded)
                ),
                "{name} one-short limit {} must fail",
                required - 1
            );
        }
    }

    #[test]
    fn rational_gcd_limits_are_exact_and_one_short_and_arithmetic_failures_are_closed() {
        fn gcd_case(
            limits: &ZeroThicknessGeometryLimits,
        ) -> Result<RationalWork, ZeroThicknessAnalysisError> {
            let denominator = (BigInt::one() << 256_usize) + BigInt::one();
            let left = BigRational::new_raw(BigInt::one(), denominator.clone());
            let right = BigRational::new_raw(BigInt::one(), denominator);
            let mut meter = RationalWorkMeter::new(limits);
            let result = meter.add(&left, &right)?;
            assert_eq!(
                result,
                BigRational::new(
                    BigInt::from(2_u8),
                    (BigInt::one() << 256_usize) + BigInt::one()
                )
            );
            Ok(meter.work)
        }

        let mut generous = unlimited_rational_limits();
        generous.max_rational_intermediate_bits = 300;
        let baseline = gcd_case(&generous).expect("bounded refined GCD path");
        assert_eq!(baseline.gcd_fallback_calls, 1);
        assert!(baseline.gcd_fallback_input_bits > 0);
        assert!(baseline.max_gcd_fallback_call_input_bits > 0);

        let mut exact = generous;
        exact.max_rational_gcd_fallback_calls = baseline.gcd_fallback_calls;
        exact.max_rational_gcd_fallback_input_bits = baseline.gcd_fallback_input_bits;
        assert!(gcd_case(&exact).is_ok());

        let mut calls_one_short = exact;
        calls_one_short.max_rational_gcd_fallback_calls = baseline.gcd_fallback_calls - 1;
        assert_eq!(
            gcd_case(&calls_one_short),
            Err(ZeroThicknessAnalysisError::ResourceLimitExceeded)
        );
        let mut bits_one_short = exact;
        bits_one_short.max_rational_gcd_fallback_input_bits = baseline.gcd_fallback_input_bits - 1;
        assert_eq!(
            gcd_case(&bits_one_short),
            Err(ZeroThicknessAnalysisError::ResourceLimitExceeded)
        );

        assert_eq!(
            shifted_nonzero_bits(1, usize::MAX),
            Err(ZeroThicknessAnalysisError::ResourceLimitExceeded)
        );
        let mut overflow_meter = RationalWorkMeter::new(&generous);
        overflow_meter.work.operations = usize::MAX;
        assert_eq!(
            overflow_meter.add(&BigRational::one(), &BigRational::one()),
            Err(ZeroThicknessAnalysisError::ResourceLimitExceeded)
        );

        let mut division_meter = RationalWorkMeter::new(&generous);
        assert_eq!(
            division_meter.divide(&BigRational::one(), &BigRational::zero()),
            Err(ZeroThicknessAnalysisError::EvidenceUnavailable)
        );
        assert_eq!(division_meter.work.operations, 0);
    }

    #[test]
    fn rational_allocation_limits_cover_fast_fallback_sign_and_compare_paths() {
        fn clone_fast_path(
            limits: &ZeroThicknessGeometryLimits,
        ) -> Result<RationalWork, ZeroThicknessAnalysisError> {
            let mut meter = RationalWorkMeter::new(limits);
            let result = meter.add(&BigRational::one(), &BigRational::zero())?;
            assert_eq!(result, BigRational::one());
            Ok(meter.work)
        }

        fn compare_fallback_path(
            limits: &ZeroThicknessGeometryLimits,
        ) -> Result<RationalWork, ZeroThicknessAnalysisError> {
            let left = BigRational::new(BigInt::one(), BigInt::from(3_u8));
            let right = BigRational::new(BigInt::from(2_u8), BigInt::from(5_u8));
            let mut meter = RationalWorkMeter::new(limits);
            assert_eq!(meter.compare(&left, &right)?, Ordering::Less);
            Ok(meter.work)
        }

        fn all_primitive_paths(
            limits: &ZeroThicknessGeometryLimits,
        ) -> Result<RationalWork, ZeroThicknessAnalysisError> {
            let mut meter = RationalWorkMeter::new(limits);
            let zero = meter.zero()?;
            let one = meter.one()?;
            let two = meter.positive_integer(2)?;
            let common = meter.input_binary64_common_unit(-1.5)?;
            let scalar = meter.input_binary64_scalar(0.25)?;
            let retained = meter.clone_retained(&common)?;
            let negated = meter.negate_temporary(&retained)?;
            let one_third = BigRational::new(BigInt::one(), BigInt::from(3_u8));
            let one_half = BigRational::new(BigInt::one(), BigInt::from(2_u8));
            let same_denominator = meter.add(&one_third, &one_third)?;
            let unlike_denominator = meter.add(&one_third, &one_half)?;
            let difference = meter.subtract(&same_denominator, &one_third)?;
            let product = meter.multiply(&unlike_denominator, &negated)?;
            let quotient = meter.divide(&product, &two)?;
            assert_eq!(meter.compare(&difference, &one_third)?, Ordering::Equal);
            assert!(meter.equal(&zero, &BigRational::zero())?);
            assert_eq!(one, BigRational::one());
            assert_eq!(scalar, BigRational::new(BigInt::one(), BigInt::from(4_u8)));
            meter.output(&quotient)?;
            Ok(meter.work)
        }

        for run in [
            clone_fast_path
                as fn(
                    &ZeroThicknessGeometryLimits,
                ) -> Result<RationalWork, ZeroThicknessAnalysisError>,
            compare_fallback_path,
            all_primitive_paths,
        ] {
            let generous = unlimited_rational_limits();
            let baseline = run(&generous).expect("allocation baseline");
            assert!(baseline.rational_allocations > 0);
            assert!(baseline.max_rational_allocation_bits > 0);
            assert!(baseline.total_rational_allocation_bits > 0);

            let mut exact = generous;
            exact.max_rational_allocations = baseline.rational_allocations;
            exact.max_rational_allocation_bits = baseline.max_rational_allocation_bits;
            exact.max_total_rational_allocation_bits = baseline.total_rational_allocation_bits;
            assert!(run(&exact).is_ok(), "every exact allocation limit succeeds");

            let mut one_short = exact;
            one_short.max_rational_allocations -= 1;
            assert_eq!(
                run(&one_short),
                Err(ZeroThicknessAnalysisError::ResourceLimitExceeded)
            );
            one_short = exact;
            one_short.max_rational_allocation_bits -= 1;
            assert_eq!(
                run(&one_short),
                Err(ZeroThicknessAnalysisError::ResourceLimitExceeded)
            );
            one_short = exact;
            one_short.max_total_rational_allocation_bits -= 1;
            assert_eq!(
                run(&one_short),
                Err(ZeroThicknessAnalysisError::ResourceLimitExceeded)
            );
        }

        let mut zero_intermediate = unlimited_rational_limits();
        zero_intermediate.max_rational_intermediate_bits = 0;
        let mut constant_meter = RationalWorkMeter::new(&zero_intermediate);
        assert_eq!(
            constant_meter.zero(),
            Err(ZeroThicknessAnalysisError::ResourceLimitExceeded)
        );
        assert_eq!(constant_meter.work.rational_allocations, 0);
        let mut constant_meter = RationalWorkMeter::new(&zero_intermediate);
        assert_eq!(
            constant_meter.one(),
            Err(ZeroThicknessAnalysisError::ResourceLimitExceeded)
        );
        assert_eq!(constant_meter.work.rational_allocations, 0);

        let mut no_allocations = unlimited_rational_limits();
        no_allocations.max_rational_allocations = 0;
        no_allocations.max_rational_allocation_bits = 0;
        no_allocations.max_total_rational_allocation_bits = 0;
        let mut sign_meter = RationalWorkMeter::new(&no_allocations);
        assert_eq!(
            sign_meter.compare(&BigRational::from_integer((-1).into()), &BigRational::one()),
            Ok(Ordering::Less)
        );
        assert_eq!(sign_meter.work.operations, 1);
        assert_eq!(sign_meter.work.rational_allocations, 0);

        let unlimited = unlimited_rational_limits();
        let mut overflow_meter = RationalWorkMeter::new(&unlimited);
        overflow_meter.work.rational_allocations = usize::MAX;
        let before = overflow_meter.work.clone();
        assert_eq!(
            overflow_meter.clone_temporary(&BigRational::one()),
            Err(ZeroThicknessAnalysisError::ResourceLimitExceeded)
        );
        assert_eq!(
            overflow_meter.work.rational_allocations,
            before.rational_allocations
        );
        assert_eq!(
            overflow_meter.work.total_rational_allocation_bits,
            before.total_rational_allocation_bits
        );

        let mut overflow_meter = RationalWorkMeter::new(&unlimited);
        overflow_meter.work.total_rational_allocation_bits = usize::MAX;
        let before = overflow_meter.work.clone();
        assert_eq!(
            overflow_meter.clone_temporary(&BigRational::one()),
            Err(ZeroThicknessAnalysisError::ResourceLimitExceeded)
        );
        assert_eq!(
            overflow_meter.work.rational_allocations,
            before.rational_allocations
        );
        assert_eq!(
            overflow_meter.work.total_rational_allocation_bits,
            before.total_rational_allocation_bits
        );
    }

    #[test]
    fn adversarial_16k_compare_is_allocation_bounded_and_canonical_equality_is_structural() {
        let denominator = BigInt::one() << 16_384_usize;
        let left = BigRational::new_raw(BigInt::one(), denominator.clone());
        let right = BigRational::new_raw(BigInt::one(), denominator + BigInt::one());
        let generous = unlimited_rational_limits();
        let mut baseline_meter = RationalWorkMeter::new(&generous);
        assert_eq!(baseline_meter.compare(&left, &right), Ok(Ordering::Greater));
        let baseline = baseline_meter.work;
        assert!(baseline.max_rational_allocation_bits >= 16_384);

        let mut exact = generous;
        exact.max_rational_allocations = baseline.rational_allocations;
        exact.max_rational_allocation_bits = baseline.max_rational_allocation_bits;
        exact.max_total_rational_allocation_bits = baseline.total_rational_allocation_bits;
        let mut exact_meter = RationalWorkMeter::new(&exact);
        assert_eq!(exact_meter.compare(&left, &right), Ok(Ordering::Greater));
        assert_eq!(exact_meter.work, baseline);

        let mut one_short = exact;
        one_short.max_total_rational_allocation_bits -= 1;
        let mut one_short_meter = RationalWorkMeter::new(&one_short);
        assert_eq!(
            one_short_meter.compare(&left, &right),
            Err(ZeroThicknessAnalysisError::ResourceLimitExceeded)
        );

        let half = BigRational::new(BigInt::one(), BigInt::from(2_u8));
        let reduced_half = BigRational::new(BigInt::from(2_u8), BigInt::from(4_u8));
        let mut equality_meter = RationalWorkMeter::new(&generous);
        assert_eq!(equality_meter.equal(&half, &reduced_half), Ok(true));
        assert_eq!(equality_meter.work.rational_allocations, 0);
    }

    #[test]
    fn every_metered_rational_constructor_preserves_the_canonical_representation() {
        fn assert_canonical(value: &BigRational) {
            assert!(value.denom().is_positive());
            assert_eq!(value.numer().gcd(value.denom()), BigInt::one());
            if value.is_zero() {
                assert_eq!(value.denom(), &BigInt::one());
            }
        }

        let limits = unlimited_rational_limits();
        let mut meter = RationalWorkMeter::new(&limits);
        let mut values = vec![
            meter.input_binary64_common_unit(-0.0).expect("common zero"),
            meter
                .input_binary64_common_unit(1.5)
                .expect("common finite"),
            meter
                .input_binary64_common_unit(f64::from_bits(1))
                .expect("common subnormal"),
            meter.input_binary64_scalar(-1.5).expect("scalar finite"),
            meter
                .input_binary64_scalar(f64::from_bits(1))
                .expect("scalar subnormal"),
        ];
        let one_third = BigRational::new(BigInt::one(), BigInt::from(3_u8));
        let one_half = BigRational::new(BigInt::one(), BigInt::from(2_u8));
        let negative_two_fifths = BigRational::new(BigInt::from(-2_i8), BigInt::from(5_u8));
        values.push(meter.add(&one_third, &one_third).expect("same denominator"));
        values.push(
            meter
                .add(&one_third, &one_half)
                .expect("unlike denominator"),
        );
        values.push(
            meter
                .subtract(&one_half, &one_half)
                .expect("cancel to zero"),
        );
        values.push(
            meter
                .multiply(&one_third, &negative_two_fifths)
                .expect("cross-cancelled product"),
        );
        values.push(
            meter
                .divide(&negative_two_fifths, &one_third)
                .expect("cross-cancelled quotient"),
        );

        for value in &values {
            assert_canonical(value);
            let dependency_clone = BigRational::new(value.numer().clone(), value.denom().clone());
            assert_eq!(
                rational_structurally_eq(value, &dependency_clone),
                value.cmp(&dependency_clone) == Ordering::Equal
            );
        }
    }

    #[test]
    fn prepared_pair_dispatch_is_order_independent_and_never_mutates_the_global_meter() {
        let (_model, pose) = corner_v_model_and_pose();
        let analysis = prepare_authenticated_zero_thickness_pose(&pose, zero_thickness_limits())
            .expect("fully prepared pair table");
        let before = analysis.rational_work().clone();
        let pairs = [(0, 1), (0, 2), (1, 2)];
        let forward = pairs.map(|(first, second)| {
            analysis
                .dispatch_pair(first, second)
                .expect("prepared canonical pair")
        });
        let reverse = [pairs[2], pairs[1], pairs[0]].map(|(first, second)| {
            analysis
                .dispatch_pair(first, second)
                .expect("prepared reverse-order pair")
        });
        assert_eq!(forward, [reverse[2], reverse[1], reverse[0]]);
        assert_eq!(
            analysis.dispatch_pair(1, 0),
            Err(ZeroThicknessAnalysisError::EvidenceUnavailable)
        );
        assert_eq!(analysis.rational_work(), &before);

        for face_count in 2..32 {
            let mut expected = 0;
            for first in 0..face_count {
                for second in (first + 1)..face_count {
                    assert_eq!(
                        canonical_unordered_pair_index(face_count, first, second),
                        Ok(expected)
                    );
                    expected += 1;
                }
            }
            assert_eq!(expected, face_count * (face_count - 1) / 2);
        }
    }

    #[test]
    fn arbitrarily_small_pose_mismatch_never_authorizes_false_transversal_or_coplanar_overlap() {
        let horizontal = synthetic_untrusted_face(
            face_id(41),
            &[
                [0.0, 0.0, -1.0],
                [1.0, 0.0, -1.0],
                [1.0, 0.0, 1.0],
                [0.0, 0.0, 1.0],
            ],
            &[[0, 1, 2], [0, 2, 3]],
        );

        for exponent in [10, 20, 40, 50] {
            let epsilon = 2.0_f64.powi(-exponent);
            let false_transversal = synthetic_untrusted_face(
                face_id(42),
                &[
                    [epsilon, -epsilon, -1.0],
                    [epsilon, 1.0 - epsilon, -1.0],
                    [epsilon, 1.0 - epsilon, 1.0],
                    [epsilon, -epsilon, 1.0],
                ],
                &[[0, 1, 2], [0, 2, 3]],
            );
            let false_coplanar_overlap = synthetic_untrusted_face(
                face_id(43),
                &[
                    [-1.0 + epsilon, 0.0, -1.0],
                    [epsilon, 0.0, -1.0],
                    [epsilon, 0.0, 1.0],
                    [-1.0 + epsilon, 0.0, 1.0],
                ],
                &[[0, 1, 2], [0, 2, 3]],
            );

            for (candidate, raw_evidence) in [
                (
                    &false_transversal,
                    IntersectionEvidenceV2::TransversalCrossing,
                ),
                (
                    &false_coplanar_overlap,
                    IntersectionEvidenceV2::CoplanarAreaOverlap,
                ),
            ] {
                let raw = aggregate_authenticated_face_pair(
                    &horizontal,
                    candidate,
                    &AuthenticatedTopology::NoSharedFeature,
                    4,
                    usize::MAX,
                    1,
                )
                .expect("complete raw diagnostic");
                assert_eq!(raw.evidence(), raw_evidence, "2^-{exponent}: {raw:?}");
                assert_eq!(
                    raw.decision(),
                    TopologyContactDecision::Penetrating,
                    "2^-{exponent}: {raw:?}"
                );
                assert!(raw.has_complete_coverage());

                for topology in [
                    AuthenticatedTopology::SharedVertexPoseMismatch,
                    AuthenticatedTopology::SharedHingePoseMismatch,
                ] {
                    let expected = PairDispatch {
                        topology: topology.relation(),
                        evidence: IntersectionEvidenceV2::Indeterminate,
                        decision: TopologyContactDecision::Indeterminate,
                        expected_triangle_pairs: 4,
                        analyzed_triangle_pairs: 4,
                    };
                    let forward = aggregate_authenticated_face_pair(
                        &horizontal,
                        candidate,
                        &topology,
                        4,
                        usize::MAX,
                        1,
                    )
                    .expect("complete forward pose-mismatch diagnostic");
                    let reverse = aggregate_authenticated_face_pair(
                        candidate,
                        &horizontal,
                        &topology,
                        4,
                        usize::MAX,
                        1,
                    )
                    .expect("complete reverse pose-mismatch diagnostic");
                    assert_eq!(forward, expected, "forward 2^-{exponent}: {raw_evidence:?}");
                    assert_eq!(reverse, expected, "reverse 2^-{exponent}: {raw_evidence:?}");
                }
            }
        }
    }

    #[test]
    fn face_pair_aggregation_preserves_all_exact_diagnostics_and_order() {
        let first = synthetic_untrusted_face(
            face_id(1),
            &[[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [0.0, 2.0, 0.0]],
            &[[0, 1, 2]],
        );
        let cases = [
            (
                synthetic_untrusted_face(
                    face_id(2),
                    &[[3.0, 3.0, 0.0], [4.0, 3.0, 0.0], [3.0, 4.0, 0.0]],
                    &[[0, 1, 2]],
                ),
                IntersectionEvidenceV2::Separated,
                TopologyContactDecision::Separated,
            ),
            (
                synthetic_untrusted_face(
                    face_id(2),
                    &[[2.0, 0.0, 0.0], [3.0, 0.0, 0.0], [2.0, -1.0, 0.0]],
                    &[[0, 1, 2]],
                ),
                IntersectionEvidenceV2::PointContact,
                TopologyContactDecision::Touching,
            ),
            (
                synthetic_untrusted_face(
                    face_id(2),
                    &[[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [1.0, -1.0, 0.0]],
                    &[[0, 1, 2]],
                ),
                IntersectionEvidenceV2::BoundaryLineContact,
                TopologyContactDecision::Touching,
            ),
            (
                synthetic_untrusted_face(
                    face_id(2),
                    &[[0.5, 0.5, 0.0], [1.5, 0.25, 0.0], [0.25, 1.5, 0.0]],
                    &[[0, 1, 2]],
                ),
                IntersectionEvidenceV2::CoplanarAreaOverlap,
                TopologyContactDecision::Penetrating,
            ),
            (
                synthetic_untrusted_face(
                    face_id(2),
                    &[[0.5, 0.25, -1.0], [0.5, 0.25, 1.0], [0.5, 1.5, 0.0]],
                    &[[0, 1, 2]],
                ),
                IntersectionEvidenceV2::TransversalCrossing,
                TopologyContactDecision::Penetrating,
            ),
        ];
        for (second, evidence, decision) in cases {
            let forward = aggregate_authenticated_face_pair(
                &first,
                &second,
                &AuthenticatedTopology::NoSharedFeature,
                1,
                usize::MAX,
                0,
            )
            .expect("forward aggregate");
            let reverse = aggregate_authenticated_face_pair(
                &second,
                &first,
                &AuthenticatedTopology::NoSharedFeature,
                1,
                usize::MAX,
                0,
            )
            .expect("reverse aggregate");
            assert_eq!(forward, single_dispatch(evidence, decision));
            assert_eq!(reverse, forward);
            assert!(forward.has_complete_coverage());
        }
    }

    #[test]
    fn face_pair_aggregate_never_allows_wrong_vertex_or_partial_hinge_contact() {
        let first = synthetic_untrusted_face(
            face_id(11),
            &[[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [0.0, 2.0, 0.0]],
            &[[0, 1, 2]],
        );
        let point_second = synthetic_untrusted_face(
            face_id(12),
            &[[2.0, 0.0, 0.0], [3.0, 0.0, 0.0], [2.0, -1.0, 0.0]],
            &[[0, 1, 2]],
        );
        let wrong_vertex = aggregate_authenticated_face_pair(
            &first,
            &point_second,
            &AuthenticatedTopology::SharedVertex(ExactPoint3::from_point(point(0.0, 2.0, 0.0))),
            1,
            usize::MAX,
            0,
        )
        .expect("complete wrong-vertex aggregate");
        assert_eq!(
            wrong_vertex.evidence(),
            IntersectionEvidenceV2::PointContact
        );
        assert_eq!(wrong_vertex.decision(), TopologyContactDecision::Touching);

        let partial_line = synthetic_untrusted_face(
            face_id(13),
            &[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.5, -1.0, 0.0]],
            &[[0, 1, 2]],
        );
        let partial_hinge = aggregate_authenticated_face_pair(
            &first,
            &partial_line,
            &AuthenticatedTopology::SharedHingeEdge {
                start: ExactPoint3::from_point(point(0.0, 0.0, 0.0)),
                end: ExactPoint3::from_point(point(2.0, 0.0, 0.0)),
            },
            1,
            usize::MAX,
            0,
        )
        .expect("complete partial-hinge aggregate");
        assert_eq!(
            partial_hinge.evidence(),
            IntersectionEvidenceV2::BoundaryLineContact
        );
        assert_eq!(
            partial_hinge.decision(),
            TopologyContactDecision::Indeterminate
        );
    }

    #[test]
    fn private_aggregate_reaches_remaining_shared_topology_policy_cells() {
        let first = synthetic_untrusted_face(
            face_id(51),
            &[[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [0.0, 2.0, 0.0]],
            &[[0, 1, 2]],
        );
        let shared_vertex =
            AuthenticatedTopology::SharedVertex(ExactPoint3::from_point(point(0.0, 0.0, 0.0)));
        let shared_hinge = AuthenticatedTopology::SharedHingeEdge {
            start: ExactPoint3::from_point(point(0.0, 0.0, 0.0)),
            end: ExactPoint3::from_point(point(2.0, 0.0, 0.0)),
        };
        let shared_vertex_line = synthetic_untrusted_face(
            face_id(52),
            &[[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [1.0, -1.0, 0.0]],
            &[[0, 1, 2]],
        );
        let shared_vertex_transversal = synthetic_untrusted_face(
            face_id(53),
            &[[0.0, 0.0, 0.0], [0.5, 0.25, -1.0], [0.5, 0.25, 1.0]],
            &[[0, 1, 2]],
        );
        let shared_hinge_area = synthetic_untrusted_face(
            face_id(54),
            &[[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [0.5, 0.5, 0.0]],
            &[[0, 1, 2]],
        );
        let shared_hinge_without_contact = synthetic_untrusted_face(
            face_id(55),
            &[[3.0, 3.0, 0.0], [4.0, 3.0, 0.0], [3.0, 4.0, 0.0]],
            &[[0, 1, 2]],
        );

        for (second, topology, evidence, decision) in [
            (
                &shared_vertex_line,
                &shared_vertex,
                IntersectionEvidenceV2::BoundaryLineContact,
                TopologyContactDecision::Touching,
            ),
            (
                &shared_vertex_transversal,
                &shared_vertex,
                IntersectionEvidenceV2::TransversalCrossing,
                TopologyContactDecision::Penetrating,
            ),
            (
                &shared_hinge_area,
                &shared_hinge,
                IntersectionEvidenceV2::CoplanarAreaOverlap,
                TopologyContactDecision::Penetrating,
            ),
            (
                &shared_hinge_without_contact,
                &shared_hinge,
                IntersectionEvidenceV2::Indeterminate,
                TopologyContactDecision::Indeterminate,
            ),
        ] {
            let expected = single_dispatch_for(topology.relation(), evidence, decision);
            let forward =
                aggregate_authenticated_face_pair(&first, second, topology, 1, usize::MAX, 1)
                    .expect("complete forward shared-topology witness");
            let reverse =
                aggregate_authenticated_face_pair(second, &first, topology, 1, usize::MAX, 1)
                    .expect("complete reverse shared-topology witness");
            assert_eq!(forward, expected);
            assert_eq!(reverse, expected);
        }
    }

    #[test]
    fn shared_vertex_allowance_requires_strictly_cooriented_material_normals() {
        let threshold = 1.0e-10_f64;
        let cases = [
            (
                f64::from_bits(threshold.to_bits() - 1),
                IntersectionEvidenceV2::Indeterminate,
                TopologyContactDecision::Indeterminate,
            ),
            (
                threshold,
                IntersectionEvidenceV2::Indeterminate,
                TopologyContactDecision::Indeterminate,
            ),
            (
                f64::from_bits(threshold.to_bits() + 1),
                IntersectionEvidenceV2::SharedFeatureContact,
                TopologyContactDecision::AllowedSharedVertexContact,
            ),
        ];
        let first_points = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        let second_points = [[0.0, 0.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, 1.0]];
        let shared = ExactPoint3::from_point(point(0.0, 0.0, 0.0));

        for (second_normal_x, evidence, decision) in cases {
            for first_permutation in TRIANGLE_PERMUTATIONS {
                for second_permutation in TRIANGLE_PERMUTATIONS {
                    let first = synthetic_untrusted_face_with_material_normal(
                        face_id(11),
                        &permute(first_points, first_permutation),
                        &[[0, 1, 2]],
                        [1.0, 0.0, 0.0],
                    );
                    let second = synthetic_untrusted_face_with_material_normal(
                        face_id(12),
                        &permute(second_points, second_permutation),
                        &[[0, 1, 2]],
                        [second_normal_x, 1.0, 0.0],
                    );
                    let expected =
                        single_dispatch_for(TopologyRelation::SharedVertex, evidence, decision);
                    assert_eq!(
                        aggregate_authenticated_face_pair(
                            &first,
                            &second,
                            &AuthenticatedTopology::SharedVertex(shared.clone()),
                            1,
                            usize::MAX,
                            0,
                        )
                        .expect("forward shared-vertex aggregate"),
                        expected,
                        "forward:{second_normal_x}:{first_permutation:?}:{second_permutation:?}"
                    );
                    assert_eq!(
                        aggregate_authenticated_face_pair(
                            &second,
                            &first,
                            &AuthenticatedTopology::SharedVertex(shared.clone()),
                            1,
                            usize::MAX,
                            0,
                        )
                        .expect("reverse shared-vertex aggregate"),
                        expected,
                        "reverse:{second_normal_x}:{first_permutation:?}:{second_permutation:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn face_level_interval_union_proves_crossing_through_triangulation_diagonals() {
        let square = synthetic_untrusted_face(
            face_id(1),
            &[
                [0.0, 0.0, 0.0],
                [2.0, 0.0, 0.0],
                [2.0, 0.0, 2.0],
                [0.0, 0.0, 2.0],
            ],
            &[[0, 1, 2], [0, 2, 3]],
        );
        let crossing = synthetic_untrusted_face(
            face_id(2),
            &[[0.0, -1.0, 0.0], [2.0, -1.0, 2.0], [1.0, 1.0, 1.0]],
            &[[0, 1, 2]],
        );
        let dispatch = aggregate_authenticated_face_pair(
            &square,
            &crossing,
            &AuthenticatedTopology::NoSharedFeature,
            2,
            usize::MAX,
            0,
        )
        .expect("complete internal-diagonal aggregate");
        assert_eq!(
            dispatch.evidence(),
            IntersectionEvidenceV2::TransversalCrossing
        );
        assert_eq!(dispatch.decision(), TopologyContactDecision::Penetrating);
        assert_eq!(dispatch.expected_triangle_pairs(), 2);
        assert_eq!(dispatch.analyzed_triangle_pairs(), 2);

        let mut reversed_square = square.clone();
        reversed_square.triangles.reverse();
        let reverse = aggregate_authenticated_face_pair(
            &crossing,
            &reversed_square,
            &AuthenticatedTopology::NoSharedFeature,
            2,
            usize::MAX,
            0,
        )
        .expect("reordered internal-diagonal aggregate");
        assert_eq!(reverse, dispatch);

        let audited_horizontal_square = synthetic_untrusted_face(
            face_id(3),
            &[
                [0.0, 0.0, 0.0],
                [2.0, 0.0, 0.0],
                [2.0, 2.0, 0.0],
                [0.0, 2.0, 0.0],
            ],
            &[[0, 1, 2], [0, 2, 3]],
        );
        let audited_vertical_square = synthetic_untrusted_face(
            face_id(4),
            &[
                [0.0, 0.0, -1.0],
                [2.0, 2.0, -1.0],
                [2.0, 2.0, 1.0],
                [0.0, 0.0, 1.0],
            ],
            &[[0, 1, 2], [0, 2, 3]],
        );
        let audited = aggregate_authenticated_face_pair(
            &audited_horizontal_square,
            &audited_vertical_square,
            &AuthenticatedTopology::NoSharedFeature,
            4,
            usize::MAX,
            0,
        )
        .expect("complete four-pair diagonal aggregate");
        assert_eq!(
            audited.evidence(),
            IntersectionEvidenceV2::TransversalCrossing
        );
        assert_eq!(audited.decision(), TopologyContactDecision::Penetrating);
        assert_eq!(audited.expected_triangle_pairs(), 4);
        assert_eq!(audited.analyzed_triangle_pairs(), 4);
    }

    #[test]
    fn face_level_interval_union_distinguishes_material_edge_from_two_artificial_diagonals() {
        let material_edge_face = synthetic_untrusted_face(
            face_id(21),
            &[[0.0, 0.0, 0.0], [2.0, 2.0, 0.0], [0.0, 2.0, 0.0]],
            &[[0, 1, 2]],
        );
        let edge_against_interior = synthetic_untrusted_face(
            face_id(22),
            &[[0.0, 0.0, -1.0], [2.0, 2.0, -1.0], [1.0, 1.0, 1.0]],
            &[[0, 1, 2]],
        );
        assert_eq!(
            classify_face_level_line_intersection(&material_edge_face, &edge_against_interior)
                .expect("bounded material-edge classifier"),
            FaceLevelLineEvidence::BoundaryOnly
        );
        let material_edge_dispatch = aggregate_authenticated_face_pair(
            &material_edge_face,
            &edge_against_interior,
            &AuthenticatedTopology::NoSharedFeature,
            1,
            usize::MAX,
            0,
        )
        .expect("material edge against other-face interior");
        assert_eq!(
            material_edge_dispatch.evidence(),
            IntersectionEvidenceV2::BoundaryLineContact
        );
        assert_eq!(
            material_edge_dispatch.decision(),
            TopologyContactDecision::Touching
        );

        let horizontal_square = synthetic_untrusted_face(
            face_id(23),
            &[
                [0.0, 0.0, 0.0],
                [2.0, 0.0, 0.0],
                [2.0, 2.0, 0.0],
                [0.0, 2.0, 0.0],
            ],
            &[[0, 1, 2], [0, 2, 3]],
        );
        let vertical_diamond = synthetic_untrusted_face(
            face_id(24),
            &[
                [0.0, 0.0, 0.0],
                [1.0, 1.0, 1.0],
                [2.0, 2.0, 0.0],
                [1.0, 1.0, -1.0],
            ],
            &[[0, 1, 2], [0, 2, 3]],
        );
        assert_eq!(
            classify_face_level_line_intersection(&horizontal_square, &vertical_diamond)
                .expect("bounded artificial-diagonal classifier"),
            FaceLevelLineEvidence::Transversal
        );
        let required_work =
            estimated_boundary_relation_work(4, 2, 2, 4, 4, 0).expect("small exact line work");
        assert_eq!(
            aggregate_authenticated_face_pair(
                &horizontal_square,
                &vertical_diamond,
                &AuthenticatedTopology::NoSharedFeature,
                4,
                required_work - 1,
                0,
            ),
            Err(ZeroThicknessAnalysisError::ResourceLimitExceeded)
        );

        let expected = PairDispatch {
            topology: TopologyRelation::NoSharedFeature,
            evidence: IntersectionEvidenceV2::TransversalCrossing,
            decision: TopologyContactDecision::Penetrating,
            expected_triangle_pairs: 4,
            analyzed_triangle_pairs: 4,
        };
        for reverse_horizontal in [false, true] {
            for reverse_vertical in [false, true] {
                let mut horizontal = horizontal_square.clone();
                let mut vertical = vertical_diamond.clone();
                if reverse_horizontal {
                    horizontal.triangles.reverse();
                }
                if reverse_vertical {
                    vertical.triangles.reverse();
                }
                assert_eq!(
                    aggregate_authenticated_face_pair(
                        &horizontal,
                        &vertical,
                        &AuthenticatedTopology::NoSharedFeature,
                        4,
                        required_work,
                        0,
                    )
                    .expect("forward complete artificial-diagonal coverage"),
                    expected
                );
                assert_eq!(
                    aggregate_authenticated_face_pair(
                        &vertical,
                        &horizontal,
                        &AuthenticatedTopology::NoSharedFeature,
                        4,
                        required_work,
                        0,
                    )
                    .expect("reverse complete artificial-diagonal coverage"),
                    expected
                );
            }
        }
    }

    #[test]
    fn face_level_interval_events_preserve_concave_gaps_and_boundary_to_interior_transitions() {
        let disconnected_points = [
            [0.0, -1.0],
            [1.0, -1.0],
            [1.0, 0.5],
            [2.0, 0.5],
            [2.0, -1.0],
            [3.0, -1.0],
            [3.0, 1.0],
            [0.0, 1.0],
        ];
        let disconnected_boundary = rest_boundary(&disconnected_points);
        let disconnected_triangles =
            triangulate_rest_boundary(&disconnected_boundary, 8, 6, usize::MAX)
                .expect("concave face triangulation");
        let disconnected_points = disconnected_points.map(|[x, z]| [x, 0.0, z]).to_vec();
        let disconnected_face =
            synthetic_untrusted_face(face_id(31), &disconnected_points, &disconnected_triangles);
        let face_inside_gap = synthetic_untrusted_face(
            face_id(32),
            &[
                [1.25, -1.0, 0.0],
                [1.75, -1.0, 0.0],
                [1.75, 1.0, 0.0],
                [1.25, 1.0, 0.0],
            ],
            &[[0, 1, 2], [0, 2, 3]],
        );
        assert_eq!(
            classify_face_level_line_intersection(&disconnected_face, &face_inside_gap)
                .expect("bounded concave-gap classifier"),
            FaceLevelLineEvidence::NoPositiveLine
        );

        let transition_points = [[0.0, 0.0], [1.0, 0.0], [2.0, -1.0], [2.0, 1.0], [0.0, 1.0]];
        let transition_boundary = rest_boundary(&transition_points);
        let transition_triangles =
            triangulate_rest_boundary(&transition_boundary, 5, 3, usize::MAX)
                .expect("boundary-to-interior face triangulation");
        let transition_points = transition_points.map(|[x, z]| [x, 0.0, z]).to_vec();
        let transition_face =
            synthetic_untrusted_face(face_id(33), &transition_points, &transition_triangles);
        let boundary_cell = synthetic_untrusted_face(
            face_id(34),
            &[
                [0.25, -1.0, 0.0],
                [0.75, -1.0, 0.0],
                [0.75, 1.0, 0.0],
                [0.25, 1.0, 0.0],
            ],
            &[[0, 1, 2], [0, 2, 3]],
        );
        let interior_cell = synthetic_untrusted_face(
            face_id(35),
            &[
                [1.25, -1.0, 0.0],
                [1.75, -1.0, 0.0],
                [1.75, 1.0, 0.0],
                [1.25, 1.0, 0.0],
            ],
            &[[0, 1, 2], [0, 2, 3]],
        );
        assert_eq!(
            classify_face_level_line_intersection(&transition_face, &boundary_cell)
                .expect("bounded material-boundary cell classifier"),
            FaceLevelLineEvidence::BoundaryOnly
        );
        assert_eq!(
            classify_face_level_line_intersection(&transition_face, &interior_cell)
                .expect("bounded relative-interior cell classifier"),
            FaceLevelLineEvidence::Transversal
        );

        let boundary_dispatch = aggregate_authenticated_face_pair(
            &transition_face,
            &boundary_cell,
            &AuthenticatedTopology::NoSharedFeature,
            6,
            usize::MAX,
            0,
        )
        .expect("complete boundary-cell aggregate");
        assert_eq!(
            boundary_dispatch.evidence(),
            IntersectionEvidenceV2::BoundaryLineContact
        );
        assert_eq!(
            boundary_dispatch.decision(),
            TopologyContactDecision::Touching
        );
        let interior_dispatch = aggregate_authenticated_face_pair(
            &transition_face,
            &interior_cell,
            &AuthenticatedTopology::NoSharedFeature,
            6,
            usize::MAX,
            0,
        )
        .expect("complete interior-cell aggregate");
        assert_eq!(
            interior_dispatch.evidence(),
            IntersectionEvidenceV2::TransversalCrossing
        );
        assert_eq!(
            interior_dispatch.decision(),
            TopologyContactDecision::Penetrating
        );
    }

    fn permute(points: [[f64; 3]; 3], permutation: [usize; 3]) -> [[f64; 3]; 3] {
        permutation.map(|index| points[index])
    }

    #[test]
    fn clear_zero_thickness_intersection_dimensions_reach_the_v2_runtime_dispatcher() {
        let first = [[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [0.0, 2.0, 0.0]];
        let cases = [
            (
                [[3.0, 3.0, 0.0], [4.0, 3.0, 0.0], [3.0, 4.0, 0.0]],
                IntersectionEvidenceV2::Separated,
                TopologyContactDecision::Separated,
            ),
            (
                [[2.0, 0.0, 0.0], [3.0, 0.0, 0.0], [2.0, -1.0, 0.0]],
                IntersectionEvidenceV2::PointContact,
                TopologyContactDecision::Touching,
            ),
            (
                [[1.0, 0.0, 0.0], [1.0, -1.0, 0.0], [2.0, -1.0, 0.0]],
                IntersectionEvidenceV2::PointContact,
                TopologyContactDecision::Touching,
            ),
            (
                [[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [1.0, -1.0, 0.0]],
                IntersectionEvidenceV2::BoundaryLineContact,
                TopologyContactDecision::Touching,
            ),
            (
                [[1.0, 0.0, 0.0], [3.0, 0.0, 0.0], [1.0, -1.0, 0.0]],
                IntersectionEvidenceV2::BoundaryLineContact,
                TopologyContactDecision::Touching,
            ),
            (
                [[0.5, 0.5, 0.0], [1.5, 0.25, 0.0], [0.25, 1.5, 0.0]],
                IntersectionEvidenceV2::CoplanarAreaOverlap,
                TopologyContactDecision::Penetrating,
            ),
            (
                [[0.5, 0.25, -1.0], [0.5, 0.25, 1.0], [0.5, 1.5, 0.0]],
                IntersectionEvidenceV2::TransversalCrossing,
                TopologyContactDecision::Penetrating,
            ),
        ];

        for (second, evidence, decision) in cases {
            for first_permutation in TRIANGLE_PERMUTATIONS {
                for second_permutation in TRIANGLE_PERMUTATIONS {
                    assert_eq!(
                        no_shared(
                            permute(first, first_permutation),
                            permute(second, second_permutation)
                        ),
                        single_dispatch(evidence, decision),
                        "{evidence:?}:{first_permutation:?}:{second_permutation:?}"
                    );
                    assert_eq!(
                        no_shared(
                            permute(second, second_permutation),
                            permute(first, first_permutation)
                        ),
                        single_dispatch(evidence, decision),
                        "swapped:{evidence:?}:{first_permutation:?}:{second_permutation:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn intersecting_support_planes_with_disjoint_cut_intervals_are_separated() {
        let horizontal = [[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [0.0, 2.0, 0.0]];
        let vertical = [[3.0, 0.0, -1.0], [3.0, 0.0, 1.0], [3.0, 1.0, 0.0]];
        let expected = single_dispatch(
            IntersectionEvidenceV2::Separated,
            TopologyContactDecision::Separated,
        );
        assert_eq!(no_shared(horizontal, vertical), expected);
        assert_eq!(no_shared(vertical, horizontal), expected);
    }

    #[test]
    fn exact_shared_feature_is_the_only_route_to_a_topology_allowance() {
        let first = triangle([[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [0.0, 2.0, 0.0]]);
        let point_second = triangle([[2.0, 0.0, 0.0], [3.0, 0.0, 0.0], [2.0, -1.0, 0.0]]);
        let point_pair = AuthenticatedTrianglePair {
            first,
            second: point_second,
            topology: AuthenticatedTopology::SharedVertex(ExactPoint3::from_point(point(
                2.0, 0.0, 0.0,
            ))),
        };
        assert_eq!(
            dispatch_authenticated_zero_thickness_pair(&point_pair),
            single_dispatch_for(
                TopologyRelation::SharedVertex,
                IntersectionEvidenceV2::SharedFeatureContact,
                TopologyContactDecision::AllowedSharedVertexContact,
            )
        );

        let line_second = triangle([[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [1.0, -1.0, 0.0]]);
        let hinge_pair = AuthenticatedTrianglePair {
            first,
            second: line_second,
            topology: AuthenticatedTopology::SharedHingeEdge {
                start: ExactPoint3::from_point(point(0.0, 0.0, 0.0)),
                end: ExactPoint3::from_point(point(2.0, 0.0, 0.0)),
            },
        };
        assert_eq!(
            dispatch_authenticated_zero_thickness_pair(&hinge_pair),
            single_dispatch_for(
                TopologyRelation::SharedHingeEdge,
                IntersectionEvidenceV2::SharedFeatureContact,
                TopologyContactDecision::RequiresHingeModel,
            )
        );
    }

    #[test]
    fn mismatched_or_partial_shared_geometry_never_enters_a_feature_allowance() {
        let first = triangle([[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [0.0, 2.0, 0.0]]);
        let point_second = triangle([[2.0, 0.0, 0.0], [3.0, 0.0, 0.0], [2.0, -1.0, 0.0]]);
        let wrong_vertex = AuthenticatedTrianglePair {
            first,
            second: point_second,
            topology: AuthenticatedTopology::SharedVertex(ExactPoint3::from_point(point(
                0.0, 2.0, 0.0,
            ))),
        };
        assert_eq!(
            dispatch_authenticated_zero_thickness_pair(&wrong_vertex),
            single_dispatch_for(
                TopologyRelation::SharedVertex,
                IntersectionEvidenceV2::PointContact,
                TopologyContactDecision::Touching,
            )
        );

        let line_second = triangle([[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [1.0, -1.0, 0.0]]);
        let partial_hinge = AuthenticatedTrianglePair {
            first,
            second: line_second,
            topology: AuthenticatedTopology::SharedHingeEdge {
                start: ExactPoint3::from_point(point(0.0, 0.0, 0.0)),
                end: ExactPoint3::from_point(point(3.0, 0.0, 0.0)),
            },
        };
        assert_eq!(
            dispatch_authenticated_zero_thickness_pair(&partial_hinge),
            single_dispatch_for(
                TopologyRelation::SharedHingeEdge,
                IntersectionEvidenceV2::BoundaryLineContact,
                TopologyContactDecision::Indeterminate,
            )
        );

        let area_overlap = AuthenticatedTrianglePair {
            first,
            second: triangle([[0.5, 0.5, 0.0], [1.5, 0.25, 0.0], [0.25, 1.5, 0.0]]),
            topology: AuthenticatedTopology::SharedVertex(ExactPoint3::from_point(point(
                0.0, 0.0, 0.0,
            ))),
        };
        assert_eq!(
            dispatch_authenticated_zero_thickness_pair(&area_overlap),
            single_dispatch_for(
                TopologyRelation::SharedVertex,
                IntersectionEvidenceV2::CoplanarAreaOverlap,
                TopologyContactDecision::Penetrating,
            )
        );
    }

    #[test]
    fn same_face_arrival_and_unrepresentable_triangle_fail_closed() {
        let triangle = triangle([[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [0.0, 2.0, 0.0]]);
        let same_face = AuthenticatedTrianglePair {
            first: triangle,
            second: triangle,
            topology: AuthenticatedTopology::SameFace,
        };
        assert_eq!(
            dispatch_authenticated_zero_thickness_pair(&same_face),
            single_dispatch_for(
                TopologyRelation::SameFace,
                IntersectionEvidenceV2::Indeterminate,
                TopologyContactDecision::Indeterminate,
            )
        );

        let degenerate = no_shared(
            [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]],
            [[0.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        );
        assert_eq!(
            degenerate,
            single_dispatch(
                IntersectionEvidenceV2::Indeterminate,
                TopologyContactDecision::Indeterminate,
            )
        );
    }

    #[test]
    fn exact_binary64_conversion_has_no_arithmetic_overflow_fallback() {
        assert_eq!(exact_binary64(-0.0), BigRational::zero());
        assert_eq!(
            exact_binary64(f64::from_bits(1)),
            BigRational::from_integer(BigInt::from(1_u8))
        );
        assert_eq!(
            exact_binary64(1.0),
            BigRational::from_integer(BigInt::from(1_u8) << 1074_usize)
        );
        assert_eq!(
            exact_binary64(f64::MAX),
            BigRational::from_integer(BigInt::from((1_u64 << 53) - 1) << 2045_usize)
        );
        assert_eq!(exact_binary64_scalar(1.0), BigRational::one());
        let source = ExactPoint3::from_point(point(0.1, -2.5, f64::from_bits(1)));
        assert_eq!(
            ExactAffineTransform::from_transform(RigidTransform::identity()).apply_point(&source),
            source
        );
    }

    #[test]
    fn subnormal_and_near_maximum_coordinates_keep_exact_classification() {
        let subnormal = f64::from_bits(1);
        let twice_subnormal = f64::from_bits(2);
        assert_eq!(
            triangulate_rest_boundary(
                &rest_boundary(&[
                    [0.0, 0.0],
                    [twice_subnormal, 0.0],
                    [twice_subnormal, twice_subnormal],
                    [0.0, twice_subnormal],
                ]),
                4,
                2,
                usize::MAX,
            )
            .expect("subnormal square")
            .len(),
            2
        );
        assert_eq!(
            no_shared(
                [
                    [0.0, 0.0, 0.0],
                    [twice_subnormal, 0.0, 0.0],
                    [0.0, twice_subnormal, 0.0],
                ],
                [
                    [twice_subnormal, 0.0, 0.0],
                    [twice_subnormal + subnormal, 0.0, 0.0],
                    [twice_subnormal, -subnormal, 0.0],
                ],
            ),
            single_dispatch(
                IntersectionEvidenceV2::PointContact,
                TopologyContactDecision::Touching,
            )
        );

        let maximum = f64::MAX;
        let previous = f64::from_bits(maximum.to_bits() - 1);
        let before_previous = f64::from_bits(maximum.to_bits() - 2);
        assert_eq!(
            triangulate_rest_boundary(
                &rest_boundary(&[
                    [previous, previous],
                    [maximum, previous],
                    [maximum, maximum],
                    [previous, maximum],
                ]),
                4,
                2,
                usize::MAX,
            )
            .expect("near-maximum square")
            .len(),
            2
        );
        assert_eq!(
            no_shared(
                [
                    [maximum, maximum, 0.0],
                    [previous, maximum, 0.0],
                    [maximum, previous, 0.0],
                ],
                [
                    [-maximum, -maximum, 0.0],
                    [-previous, -maximum, 0.0],
                    [-maximum, -before_previous, 0.0],
                ],
            ),
            single_dispatch(
                IntersectionEvidenceV2::Separated,
                TopologyContactDecision::Separated,
            )
        );
    }
}
