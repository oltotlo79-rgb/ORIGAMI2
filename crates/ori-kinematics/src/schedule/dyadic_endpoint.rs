//! Directed binary64 enclosure for normalized dyadic schedule leaves.

use super::{CycleSchedulePrepareErrorV1, OutwardIntervalV1};

/// Outward binary64 enclosure of one normalized Chebyshev dyadic leaf.
///
/// Each endpoint is an exact signed integer divided by `2^depth`. When more
/// than 53 significant bits are needed, this constructs the two adjacent
/// representable dyadic values by integer truncation instead of relying on an
/// inward `u64 as f64` conversion.
pub(super) fn ordinary_dyadic_chebyshev_interval_v2(
    depth: u32,
    index: u64,
) -> Result<OutwardIntervalV1, CycleSchedulePrepareErrorV1> {
    if depth >= 64 || index >= (1_u64 << depth) {
        return Err(CycleSchedulePrepareErrorV1::InvalidInput);
    }
    let endpoint_bounds = |endpoint_index: u64| -> Option<(f64, f64)> {
        if endpoint_index > (1_u64 << depth) {
            return None;
        }
        let denominator = 1_i128 << depth;
        let numerator = 2_i128
            .checked_mul(i128::from(endpoint_index))?
            .checked_sub(denominator)?;
        if numerator == 0 {
            return Some((0.0, 0.0));
        }
        let negative = numerator < 0;
        let magnitude = numerator.unsigned_abs();
        let bit_length = u128::BITS - magnitude.leading_zeros();
        let shift = bit_length.saturating_sub(53);
        let truncated = magnitude >> shift;
        let remainder_mask = if shift == 0 { 0 } else { (1_u128 << shift) - 1 };
        let has_remainder = magnitude & remainder_mask != 0;
        let exponent = i32::try_from(shift).ok()? - i32::try_from(depth).ok()?;
        let scale_exact = |significand: u128| -> Option<f64> {
            let significand = u64::try_from(significand).ok()? as f64;
            if exponent >= 0 {
                Some(significand * (1_u64 << u32::try_from(exponent).ok()?) as f64)
            } else {
                Some(significand / (1_u64 << exponent.unsigned_abs()) as f64)
            }
        };
        let lower_magnitude = scale_exact(truncated)?;
        let upper_magnitude = if has_remainder {
            scale_exact(truncated.checked_add(1)?)?
        } else {
            lower_magnitude
        };
        Some(if negative {
            (-upper_magnitude, -lower_magnitude)
        } else {
            (lower_magnitude, upper_magnitude)
        })
    };
    let lower = endpoint_bounds(index)
        .ok_or(CycleSchedulePrepareErrorV1::ResourceLimit)?
        .0;
    let upper = endpoint_bounds(
        index
            .checked_add(1)
            .ok_or(CycleSchedulePrepareErrorV1::ResourceLimit)?,
    )
    .ok_or(CycleSchedulePrepareErrorV1::ResourceLimit)?
    .1;
    OutwardIntervalV1::new(lower, upper).map_err(|_| CycleSchedulePrepareErrorV1::InvalidInput)
}

#[cfg(test)]
mod tests {
    use num_bigint::BigInt;
    use num_rational::BigRational;

    use super::*;

    #[test]
    fn depth_54_high_index_interval_contains_both_exact_endpoints() {
        let depth = 54;
        let index = (1_u64 << 53) + 3;
        let interval = ordinary_dyadic_chebyshev_interval_v2(depth, index).unwrap();
        let denominator = BigInt::from(1_u64 << depth);
        let exact = |endpoint_index: u64| {
            BigRational::new(
                BigInt::from(2_u64 * endpoint_index) - &denominator,
                denominator.clone(),
            )
        };
        let exact_lower = exact(index);
        let exact_upper = exact(index + 1);
        let outward_lower = BigRational::from_float(interval.lower()).unwrap();
        let outward_upper = BigRational::from_float(interval.upper()).unwrap();
        assert!(outward_lower <= exact_lower);
        assert!(outward_upper >= exact_upper);

        let legacy_lower = -1.0 + 2.0 * index as f64 / (1_u64 << depth) as f64;
        assert!(BigRational::from_float(legacy_lower).unwrap() > exact_lower);
    }

    #[test]
    fn depths_54_through_63_enclose_signed_boundary_and_midpoint_endpoints() {
        for depth in 54..=63 {
            let leaf_count = 1_u64 << depth;
            let midpoint = leaf_count / 2;
            for index in [
                0,
                1,
                midpoint - 1,
                midpoint,
                midpoint + 1,
                leaf_count - 2,
                leaf_count - 1,
            ] {
                let interval = ordinary_dyadic_chebyshev_interval_v2(depth, index).unwrap();
                let denominator = BigInt::from(leaf_count);
                let exact = |endpoint_index: u64| {
                    BigRational::new(
                        BigInt::from(2_u8) * BigInt::from(endpoint_index) - &denominator,
                        denominator.clone(),
                    )
                };
                assert!(
                    BigRational::from_float(interval.lower()).unwrap() <= exact(index),
                    "depth={depth} index={index} lower endpoint"
                );
                assert!(
                    BigRational::from_float(interval.upper()).unwrap() >= exact(index + 1),
                    "depth={depth} index={index} upper endpoint"
                );
                assert!(interval.lower() >= -1.0 && interval.upper() <= 1.0);
            }
        }
    }
}
