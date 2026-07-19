//! Exact, resource-bounded Cayley rotations for a future watertight pose.
//!
//! This module is deliberately private until the exact tree-pose issuer can
//! bind its output to a pose certificate.  In particular, none of the
//! rationals below may be supplied by a caller as collision evidence.

use std::{
    cmp::Ordering,
    collections::{HashMap, VecDeque},
};

use num_bigint::{BigInt, BigUint, Sign};
use num_integer::Integer;
use num_rational::BigRational;
use num_traits::{One, Signed, Zero};
use ori_domain::{EdgeId, FaceId, VertexId};
use ori_kinematics::{BoundMaterialTreePose, Point3};
use ori_topology::FoldAssignment;

const RATIONAL_CAYLEY_LOCAL_ROTATION_V1: &str = "rational_cayley_local_rotation_v1";
const RATIONAL_CAYLEY_TREE_POSE_V1: &str = "rational_cayley_tree_pose_v1";
const DEGREE_180: u16 = 180;
const DEGREE_360: u16 = 360;
const DEFAULT_GUARD_BITS: usize = 64;
const DEFAULT_CANDIDATE_BITS: usize = 80;
const PRECISION_ROUNDS: [usize; 6] = [128, 256, 512, 1_024, 2_048, 4_096];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CayleyLimits {
    max_precision_rounds: usize,
    max_guard_bits: usize,
    max_candidate_bits: usize,
    max_machin_terms_per_series: usize,
    max_trig_terms_per_series: usize,
    max_sqrt_refinements: usize,
    max_interval_operations: usize,
    max_shift_bits: usize,
    max_intermediate_bits: usize,
    max_gcd_fallback_calls: usize,
    max_gcd_fallback_input_bits: usize,
    max_rational_allocations: usize,
    max_rational_allocation_bits: usize,
    max_total_rational_allocation_bits: usize,
    max_output_bits: usize,
}

impl Default for CayleyLimits {
    fn default() -> Self {
        Self {
            max_precision_rounds: PRECISION_ROUNDS.len(),
            max_guard_bits: DEFAULT_GUARD_BITS,
            max_candidate_bits: DEFAULT_CANDIDATE_BITS,
            max_machin_terms_per_series: 2_048,
            max_trig_terms_per_series: 2_048,
            max_sqrt_refinements: 32,
            max_interval_operations: 100_000,
            max_shift_bits: 8_192,
            max_intermediate_bits: 32_768,
            max_gcd_fallback_calls: 4_096,
            max_gcd_fallback_input_bits: 67_108_864,
            max_rational_allocations: 1_000_000,
            max_rational_allocation_bits: 65_536,
            max_total_rational_allocation_bits: 1_073_741_824,
            max_output_bits: 16_384,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CayleyStage {
    Input,
    Axis,
    Pi,
    Trigonometry,
    SquareRoot,
    Candidate,
    Matrix,
    Output,
    Tree,
    Containment,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CayleyError {
    NonFiniteInput {
        stage: CayleyStage,
    },
    AngleOutOfRange {
        stage: CayleyStage,
    },
    InvalidRotationSign {
        stage: CayleyStage,
    },
    DegenerateAxis {
        stage: CayleyStage,
    },
    ResourceLimitExceeded {
        stage: CayleyStage,
        resource: &'static str,
    },
    CertificateUnavailable {
        stage: CayleyStage,
    },
    InvariantFailure {
        stage: CayleyStage,
    },
    BoundTreeInconsistent {
        stage: CayleyStage,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct CayleyWork {
    interval_operations: usize,
    machin_terms: usize,
    max_machin_series_terms: usize,
    trig_terms: usize,
    max_trig_series_terms: usize,
    sqrt_refinements: usize,
    max_sqrt_call_refinements: usize,
    max_shift_bits: usize,
    max_preflight_bits: usize,
    max_observed_bits: usize,
    gcd_fallback_calls: usize,
    gcd_fallback_input_bits: usize,
    max_gcd_fallback_call_input_bits: usize,
    rational_allocations: usize,
    max_rational_allocation_bits: usize,
    total_rational_allocation_bits: usize,
    max_output_bits: usize,
}

impl CayleyWork {
    fn checked_merge(
        &self,
        additional: &Self,
        limits: &CayleyLimits,
        total_term_limits: Option<TotalTermLimits>,
        stage: CayleyStage,
    ) -> Result<Self, CayleyError> {
        let merged = Self {
            interval_operations: checked_work_sum(
                self.interval_operations,
                additional.interval_operations,
                stage,
                "interval_operations",
            )?,
            machin_terms: checked_work_sum(
                self.machin_terms,
                additional.machin_terms,
                stage,
                "machin_terms",
            )?,
            max_machin_series_terms: self
                .max_machin_series_terms
                .max(additional.max_machin_series_terms),
            trig_terms: checked_work_sum(
                self.trig_terms,
                additional.trig_terms,
                stage,
                "trig_terms",
            )?,
            max_trig_series_terms: self
                .max_trig_series_terms
                .max(additional.max_trig_series_terms),
            sqrt_refinements: checked_work_sum(
                self.sqrt_refinements,
                additional.sqrt_refinements,
                stage,
                "sqrt_refinements",
            )?,
            max_sqrt_call_refinements: self
                .max_sqrt_call_refinements
                .max(additional.max_sqrt_call_refinements),
            max_shift_bits: self.max_shift_bits.max(additional.max_shift_bits),
            max_preflight_bits: self.max_preflight_bits.max(additional.max_preflight_bits),
            max_observed_bits: self.max_observed_bits.max(additional.max_observed_bits),
            gcd_fallback_calls: checked_work_sum(
                self.gcd_fallback_calls,
                additional.gcd_fallback_calls,
                stage,
                "gcd_fallback_calls",
            )?,
            gcd_fallback_input_bits: checked_work_sum(
                self.gcd_fallback_input_bits,
                additional.gcd_fallback_input_bits,
                stage,
                "gcd_fallback_input_bits",
            )?,
            max_gcd_fallback_call_input_bits: self
                .max_gcd_fallback_call_input_bits
                .max(additional.max_gcd_fallback_call_input_bits),
            rational_allocations: checked_work_sum(
                self.rational_allocations,
                additional.rational_allocations,
                stage,
                "rational_allocations",
            )?,
            max_rational_allocation_bits: self
                .max_rational_allocation_bits
                .max(additional.max_rational_allocation_bits),
            total_rational_allocation_bits: checked_work_sum(
                self.total_rational_allocation_bits,
                additional.total_rational_allocation_bits,
                stage,
                "total_rational_allocation_bits",
            )?,
            max_output_bits: self.max_output_bits.max(additional.max_output_bits),
        };
        merged.validate(limits, total_term_limits, stage)?;
        Ok(merged)
    }

    fn validate(
        &self,
        limits: &CayleyLimits,
        total_term_limits: Option<TotalTermLimits>,
        stage: CayleyStage,
    ) -> Result<(), CayleyError> {
        for (actual, maximum, resource) in [
            (
                self.interval_operations,
                limits.max_interval_operations,
                "interval_operations",
            ),
            (
                self.max_machin_series_terms,
                limits.max_machin_terms_per_series,
                "machin_terms",
            ),
            (
                self.max_trig_series_terms,
                limits.max_trig_terms_per_series,
                "trig_terms",
            ),
            (
                self.max_sqrt_call_refinements,
                limits.max_sqrt_refinements,
                "sqrt_refinements",
            ),
            (self.max_shift_bits, limits.max_shift_bits, "shift_bits"),
            (
                self.max_preflight_bits,
                limits.max_intermediate_bits,
                "intermediate_bits",
            ),
            (
                self.gcd_fallback_calls,
                limits.max_gcd_fallback_calls,
                "gcd_fallback_calls",
            ),
            (
                self.gcd_fallback_input_bits,
                limits.max_gcd_fallback_input_bits,
                "gcd_fallback_input_bits",
            ),
            (
                self.max_gcd_fallback_call_input_bits,
                limits.max_gcd_fallback_input_bits,
                "gcd_fallback_input_bits",
            ),
            (
                self.rational_allocations,
                limits.max_rational_allocations,
                "rational_allocations",
            ),
            (
                self.max_rational_allocation_bits,
                limits.max_rational_allocation_bits,
                "rational_allocation_bits",
            ),
            (
                self.total_rational_allocation_bits,
                limits.max_total_rational_allocation_bits,
                "total_rational_allocation_bits",
            ),
            (self.max_output_bits, limits.max_output_bits, "output_bits"),
        ] {
            if actual > maximum {
                return Err(CayleyError::ResourceLimitExceeded { stage, resource });
            }
        }
        let maximum_observed = limits.max_intermediate_bits.max(limits.max_output_bits);
        if self.max_observed_bits > maximum_observed {
            return Err(CayleyError::ResourceLimitExceeded {
                stage,
                resource: "observed_bits",
            });
        }
        if self.machin_terms > limits.max_interval_operations {
            return Err(CayleyError::ResourceLimitExceeded {
                stage,
                resource: "machin_terms",
            });
        }
        if self.trig_terms > limits.max_interval_operations {
            return Err(CayleyError::ResourceLimitExceeded {
                stage,
                resource: "trig_terms",
            });
        }
        if self.sqrt_refinements > limits.max_interval_operations {
            return Err(CayleyError::ResourceLimitExceeded {
                stage,
                resource: "sqrt_refinements",
            });
        }
        if let Some(total) = total_term_limits {
            for (actual, maximum, resource) in [
                (self.machin_terms, total.machin_terms, "total_machin_terms"),
                (self.trig_terms, total.trig_terms, "total_trig_terms"),
                (
                    self.sqrt_refinements,
                    total.sqrt_refinements,
                    "total_sqrt_refinements",
                ),
            ] {
                if actual > maximum {
                    return Err(CayleyError::ResourceLimitExceeded { stage, resource });
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct TotalTermLimits {
    machin_terms: usize,
    trig_terms: usize,
    sqrt_refinements: usize,
}

struct WorkMeter<'a> {
    limits: &'a CayleyLimits,
    total_term_limits: Option<TotalTermLimits>,
    work: CayleyWork,
}

impl<'a> WorkMeter<'a> {
    fn new(limits: &'a CayleyLimits) -> Self {
        Self {
            limits,
            total_term_limits: None,
            work: CayleyWork::default(),
        }
    }

    fn resume(
        limits: &'a CayleyLimits,
        total_term_limits: Option<TotalTermLimits>,
        consumed: &CayleyWork,
        stage: CayleyStage,
    ) -> Result<Self, CayleyError> {
        let work =
            CayleyWork::default().checked_merge(consumed, limits, total_term_limits, stage)?;
        Ok(Self {
            limits,
            total_term_limits,
            work,
        })
    }

    fn merge_work(
        &mut self,
        additional: &CayleyWork,
        stage: CayleyStage,
    ) -> Result<(), CayleyError> {
        let merged =
            self.work
                .checked_merge(additional, self.limits, self.total_term_limits, stage)?;
        self.work = merged;
        Ok(())
    }

    fn with_total_term_limits(
        limits: &'a CayleyLimits,
        total_term_limits: TotalTermLimits,
    ) -> Self {
        Self {
            limits,
            total_term_limits: Some(total_term_limits),
            work: CayleyWork::default(),
        }
    }

    fn operation(&mut self, stage: CayleyStage) -> Result<(), CayleyError> {
        let next = self.work.interval_operations.checked_add(1).ok_or(
            CayleyError::ResourceLimitExceeded {
                stage,
                resource: "interval_operations",
            },
        )?;
        if next > self.limits.max_interval_operations {
            return Err(CayleyError::ResourceLimitExceeded {
                stage,
                resource: "interval_operations",
            });
        }
        self.work.interval_operations = next;
        Ok(())
    }

    fn machin_term(&mut self, stage: CayleyStage, local_terms: usize) -> Result<(), CayleyError> {
        if local_terms > self.limits.max_machin_terms_per_series {
            return Err(CayleyError::ResourceLimitExceeded {
                stage,
                resource: "machin_terms",
            });
        }
        let next =
            self.work
                .machin_terms
                .checked_add(1)
                .ok_or(CayleyError::ResourceLimitExceeded {
                    stage,
                    resource: "machin_terms",
                })?;
        if self
            .total_term_limits
            .is_some_and(|limits| next > limits.machin_terms)
        {
            return Err(CayleyError::ResourceLimitExceeded {
                stage: CayleyStage::Tree,
                resource: "total_machin_terms",
            });
        }
        self.operation(stage)?;
        self.work.max_machin_series_terms = self.work.max_machin_series_terms.max(local_terms);
        self.work.machin_terms = next;
        Ok(())
    }

    fn trig_term(&mut self, stage: CayleyStage, local_terms: usize) -> Result<(), CayleyError> {
        if local_terms > self.limits.max_trig_terms_per_series {
            return Err(CayleyError::ResourceLimitExceeded {
                stage,
                resource: "trig_terms",
            });
        }
        let next =
            self.work
                .trig_terms
                .checked_add(1)
                .ok_or(CayleyError::ResourceLimitExceeded {
                    stage,
                    resource: "trig_terms",
                })?;
        if self
            .total_term_limits
            .is_some_and(|limits| next > limits.trig_terms)
        {
            return Err(CayleyError::ResourceLimitExceeded {
                stage: CayleyStage::Tree,
                resource: "total_trig_terms",
            });
        }
        self.operation(stage)?;
        self.work.max_trig_series_terms = self.work.max_trig_series_terms.max(local_terms);
        self.work.trig_terms = next;
        Ok(())
    }

    fn sqrt_refinement(
        &mut self,
        stage: CayleyStage,
        local_refinements: usize,
    ) -> Result<(), CayleyError> {
        if local_refinements > self.limits.max_sqrt_refinements {
            return Err(CayleyError::ResourceLimitExceeded {
                stage,
                resource: "sqrt_refinements",
            });
        }
        let next = self.work.sqrt_refinements.checked_add(1).ok_or(
            CayleyError::ResourceLimitExceeded {
                stage,
                resource: "sqrt_refinements",
            },
        )?;
        if self
            .total_term_limits
            .is_some_and(|limits| next > limits.sqrt_refinements)
        {
            return Err(CayleyError::ResourceLimitExceeded {
                stage: CayleyStage::Tree,
                resource: "total_sqrt_refinements",
            });
        }
        self.operation(stage)?;
        self.work.max_sqrt_call_refinements =
            self.work.max_sqrt_call_refinements.max(local_refinements);
        self.work.sqrt_refinements = next;
        Ok(())
    }

    fn shift(&mut self, stage: CayleyStage, bits: usize) -> Result<(), CayleyError> {
        self.work.max_shift_bits = self.work.max_shift_bits.max(bits);
        if bits > self.limits.max_shift_bits {
            return Err(CayleyError::ResourceLimitExceeded {
                stage,
                resource: "shift_bits",
            });
        }
        Ok(())
    }

    fn preflight_product_bits(
        &mut self,
        stage: CayleyStage,
        first: usize,
        second: usize,
    ) -> Result<(), CayleyError> {
        let bits = first
            .checked_add(second)
            .ok_or(CayleyError::ResourceLimitExceeded {
                stage,
                resource: "intermediate_bits",
            })?;
        self.work.max_preflight_bits = self.work.max_preflight_bits.max(bits);
        if bits > self.limits.max_intermediate_bits {
            return Err(CayleyError::ResourceLimitExceeded {
                stage,
                resource: "intermediate_bits",
            });
        }
        Ok(())
    }

    fn preflight_value_bits(&mut self, stage: CayleyStage, bits: usize) -> Result<(), CayleyError> {
        self.work.max_preflight_bits = self.work.max_preflight_bits.max(bits);
        if bits > self.limits.max_intermediate_bits {
            return Err(CayleyError::ResourceLimitExceeded {
                stage,
                resource: "intermediate_bits",
            });
        }
        Ok(())
    }

    fn preflight_shifted_value(
        &mut self,
        stage: CayleyStage,
        value_bits: usize,
        shift: usize,
    ) -> Result<(), CayleyError> {
        self.shift(stage, shift)?;
        let shifted_bits =
            value_bits
                .checked_add(shift)
                .ok_or(CayleyError::ResourceLimitExceeded {
                    stage,
                    resource: "intermediate_bits",
                })?;
        self.preflight_value_bits(stage, shifted_bits)
    }

    fn clone_rational(
        &mut self,
        value: &BigRational,
        stage: CayleyStage,
    ) -> Result<BigRational, CayleyError> {
        // Copying performs no arithmetic or GCD work. Its CPU and memory cost
        // is bounded independently by the allocation count and storage-bit
        // limits, so it deliberately does not consume an interval operation.
        self.preflight_value_bits(stage, rational_bits(value))?;
        self.charge_rational_allocations(&[rational_storage_bits(value, stage)?], stage)?;
        Ok(value.clone())
    }

    fn negate_rational(
        &mut self,
        value: &BigRational,
        stage: CayleyStage,
    ) -> Result<BigRational, CayleyError> {
        // A sign change copies one canonical rational without changing its
        // magnitude. The dedicated allocation budget is the complete work
        // bound; no interval-arithmetic operation is consumed.
        self.preflight_value_bits(stage, rational_bits(value))?;
        self.charge_rational_allocations(&[rational_storage_bits(value, stage)?], stage)?;
        Ok(-value)
    }

    fn compare_rational(
        &mut self,
        left: &BigRational,
        right: &BigRational,
        stage: CayleyStage,
    ) -> Result<Ordering, CayleyError> {
        self.operation(stage)?;
        self.preflight_value_bits(stage, rational_bits(left))?;
        self.preflight_value_bits(stage, rational_bits(right))?;
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
        let denominator_gcd = self.gcd_fallback(left.denom(), right.denom(), stage)?;
        let left_multiplier_bits =
            quotient_bits_upper_bound(right.denom(), &denominator_gcd, stage)?;
        let right_multiplier_bits =
            quotient_bits_upper_bound(left.denom(), &denominator_gcd, stage)?;
        let left_cross_bits =
            product_bits_upper_bound(bigint_bits(left.numer()), left_multiplier_bits, stage)?;
        let right_cross_bits =
            product_bits_upper_bound(bigint_bits(right.numer()), right_multiplier_bits, stage)?;
        for bits in [
            left_multiplier_bits,
            right_multiplier_bits,
            left_cross_bits,
            right_cross_bits,
        ] {
            self.preflight_value_bits(stage, bits)?;
        }
        self.charge_rational_allocations(
            &[
                left_multiplier_bits,
                right_multiplier_bits,
                left_cross_bits,
                right_cross_bits,
            ],
            stage,
        )?;
        let left_multiplier = right.denom() / &denominator_gcd;
        let right_multiplier = left.denom() / denominator_gcd;
        debug_assert!(bigint_bits(&left_multiplier) <= left_multiplier_bits);
        debug_assert!(bigint_bits(&right_multiplier) <= right_multiplier_bits);
        let left_cross = left.numer() * left_multiplier;
        let right_cross = right.numer() * right_multiplier;
        debug_assert!(bigint_bits(&left_cross) <= left_cross_bits);
        debug_assert!(bigint_bits(&right_cross) <= right_cross_bits);
        Ok(left_cross.cmp(&right_cross))
    }

    fn charge_rational_allocations(
        &mut self,
        allocation_bits: &[usize],
        stage: CayleyStage,
    ) -> Result<(), CayleyError> {
        let count = self
            .work
            .rational_allocations
            .checked_add(allocation_bits.len())
            .ok_or(CayleyError::ResourceLimitExceeded {
                stage,
                resource: "rational_allocations",
            })?;
        if count > self.limits.max_rational_allocations {
            return Err(CayleyError::ResourceLimitExceeded {
                stage,
                resource: "rational_allocations",
            });
        }
        let mut total = self.work.total_rational_allocation_bits;
        let mut maximum = self.work.max_rational_allocation_bits;
        for bits in allocation_bits {
            if *bits > self.limits.max_rational_allocation_bits {
                return Err(CayleyError::ResourceLimitExceeded {
                    stage,
                    resource: "rational_allocation_bits",
                });
            }
            total = total
                .checked_add(*bits)
                .ok_or(CayleyError::ResourceLimitExceeded {
                    stage,
                    resource: "total_rational_allocation_bits",
                })?;
            if total > self.limits.max_total_rational_allocation_bits {
                return Err(CayleyError::ResourceLimitExceeded {
                    stage,
                    resource: "total_rational_allocation_bits",
                });
            }
            maximum = maximum.max(*bits);
        }
        self.work.rational_allocations = count;
        self.work.max_rational_allocation_bits = maximum;
        self.work.total_rational_allocation_bits = total;
        Ok(())
    }

    fn gcd_fallback(
        &mut self,
        left: &BigInt,
        right: &BigInt,
        stage: CayleyStage,
    ) -> Result<BigInt, CayleyError> {
        let left_bits = bigint_bits(left);
        let right_bits = bigint_bits(right);
        self.preflight_value_bits(stage, left_bits.max(right_bits))?;
        let call_input_bits =
            left_bits
                .checked_add(right_bits)
                .ok_or(CayleyError::ResourceLimitExceeded {
                    stage,
                    resource: "gcd_fallback_input_bits",
                })?;
        let next_calls = self.work.gcd_fallback_calls.checked_add(1).ok_or(
            CayleyError::ResourceLimitExceeded {
                stage,
                resource: "gcd_fallback_calls",
            },
        )?;
        let next_input_bits = self
            .work
            .gcd_fallback_input_bits
            .checked_add(call_input_bits)
            .ok_or(CayleyError::ResourceLimitExceeded {
                stage,
                resource: "gcd_fallback_input_bits",
            })?;
        if next_calls > self.limits.max_gcd_fallback_calls {
            return Err(CayleyError::ResourceLimitExceeded {
                stage,
                resource: "gcd_fallback_calls",
            });
        }
        if next_input_bits > self.limits.max_gcd_fallback_input_bits {
            return Err(CayleyError::ResourceLimitExceeded {
                stage,
                resource: "gcd_fallback_input_bits",
            });
        }
        self.work.gcd_fallback_calls = next_calls;
        self.work.gcd_fallback_input_bits = next_input_bits;
        self.work.max_gcd_fallback_call_input_bits = self
            .work
            .max_gcd_fallback_call_input_bits
            .max(call_input_bits);
        Ok(left.gcd(right))
    }

    fn normalize_refined_rational(
        &mut self,
        numerator: BigInt,
        denominator: BigInt,
        stage: CayleyStage,
    ) -> Result<BigRational, CayleyError> {
        if denominator.is_zero() {
            return Err(CayleyError::InvariantFailure { stage });
        }
        let gcd = self.gcd_fallback(&numerator, &denominator, stage)?;
        let mut numerator = numerator / &gcd;
        let mut denominator = denominator / gcd;
        if denominator.is_negative() {
            numerator = -numerator;
            denominator = -denominator;
        }
        Ok(BigRational::new_raw(numerator, denominator))
    }

    fn add_rational(
        &mut self,
        left: &BigRational,
        right: &BigRational,
        stage: CayleyStage,
    ) -> Result<BigRational, CayleyError> {
        self.add_or_subtract_rational(left, right, false, stage)
    }

    fn subtract_rational(
        &mut self,
        left: &BigRational,
        right: &BigRational,
        stage: CayleyStage,
    ) -> Result<BigRational, CayleyError> {
        self.add_or_subtract_rational(left, right, true, stage)
    }

    fn add_or_subtract_rational(
        &mut self,
        left: &BigRational,
        right: &BigRational,
        subtract: bool,
        stage: CayleyStage,
    ) -> Result<BigRational, CayleyError> {
        self.operation(stage)?;
        let raw_left_product = bigint_bits(left.numer())
            .checked_add(bigint_bits(right.denom()))
            .ok_or(CayleyError::ResourceLimitExceeded {
                stage,
                resource: "intermediate_bits",
            })?;
        let raw_right_product = bigint_bits(right.numer())
            .checked_add(bigint_bits(left.denom()))
            .ok_or(CayleyError::ResourceLimitExceeded {
                stage,
                resource: "intermediate_bits",
            })?;
        let raw_denominator = bigint_bits(left.denom())
            .checked_add(bigint_bits(right.denom()))
            .ok_or(CayleyError::ResourceLimitExceeded {
                stage,
                resource: "intermediate_bits",
            })?;
        let raw_numerator = raw_left_product
            .max(raw_right_product)
            .checked_add(1)
            .ok_or(CayleyError::ResourceLimitExceeded {
                stage,
                resource: "intermediate_bits",
            })?;
        if raw_numerator.max(raw_denominator) <= self.limits.max_intermediate_bits {
            self.preflight_value_bits(stage, raw_numerator)?;
            self.preflight_value_bits(stage, raw_denominator)?;
            let result = if subtract { left - right } else { left + right };
            self.observe_rational(stage, &result)?;
            return Ok(result);
        }

        // `num-rational` adds and subtracts over the LCM rather than over the
        // raw product. Only enter this more expensive path when the
        // conservative fast-path bound would reject an operation.
        let (left_multiplier, right_multiplier) = if left.denom() == right.denom() {
            (BigInt::one(), BigInt::one())
        } else {
            let gcd = self.gcd_fallback(left.denom(), right.denom(), stage)?;
            (right.denom() / &gcd, left.denom() / gcd)
        };
        let left_product = refined_product_bits(left.numer(), &left_multiplier, stage)?;
        let right_product = refined_product_bits(right.numer(), &right_multiplier, stage)?;
        let denominator = refined_product_bits(left.denom(), &left_multiplier, stage)?;
        let numerator = left_product.max(right_product).checked_add(1).ok_or(
            CayleyError::ResourceLimitExceeded {
                stage,
                resource: "intermediate_bits",
            },
        )?;
        self.preflight_value_bits(stage, numerator)?;
        self.preflight_value_bits(stage, denominator)?;
        let left_numerator = left.numer() * &left_multiplier;
        let right_numerator = right.numer() * &right_multiplier;
        let denominator = left.denom() * left_multiplier;
        let numerator = if subtract {
            left_numerator - right_numerator
        } else {
            left_numerator + right_numerator
        };
        let result = self.normalize_refined_rational(numerator, denominator, stage)?;
        self.observe_rational(stage, &result)?;
        Ok(result)
    }

    fn multiply_rational(
        &mut self,
        left: &BigRational,
        right: &BigRational,
        stage: CayleyStage,
    ) -> Result<BigRational, CayleyError> {
        self.operation(stage)?;
        let raw_numerator = bigint_bits(left.numer())
            .checked_add(bigint_bits(right.numer()))
            .ok_or(CayleyError::ResourceLimitExceeded {
                stage,
                resource: "intermediate_bits",
            })?;
        let raw_denominator = bigint_bits(left.denom())
            .checked_add(bigint_bits(right.denom()))
            .ok_or(CayleyError::ResourceLimitExceeded {
                stage,
                resource: "intermediate_bits",
            })?;
        if raw_numerator.max(raw_denominator) <= self.limits.max_intermediate_bits {
            self.preflight_value_bits(stage, raw_numerator)?;
            self.preflight_value_bits(stage, raw_denominator)?;
            let result = left * right;
            self.observe_rational(stage, &result)?;
            return Ok(result);
        }

        // Mirror `num-rational`'s cross-cancellation before either product is
        // constructed. The result is already canonical because both inputs
        // are canonical and all cross factors have been removed.
        let numerator_gcd = self.gcd_fallback(left.numer(), right.denom(), stage)?;
        let denominator_gcd = self.gcd_fallback(left.denom(), right.numer(), stage)?;
        let left_numerator = left.numer() / &numerator_gcd;
        let right_denominator = right.denom() / numerator_gcd;
        let right_numerator = right.numer() / &denominator_gcd;
        let left_denominator = left.denom() / denominator_gcd;
        self.preflight_value_bits(
            stage,
            refined_product_bits(&left_numerator, &right_numerator, stage)?,
        )?;
        self.preflight_value_bits(
            stage,
            refined_product_bits(&left_denominator, &right_denominator, stage)?,
        )?;
        let result = BigRational::new_raw(
            left_numerator * right_numerator,
            left_denominator * right_denominator,
        );
        self.observe_rational(stage, &result)?;
        Ok(result)
    }

    fn divide_rational(
        &mut self,
        left: &BigRational,
        right: &BigRational,
        stage: CayleyStage,
    ) -> Result<BigRational, CayleyError> {
        if right.is_zero() {
            return Err(CayleyError::CertificateUnavailable { stage });
        }
        self.operation(stage)?;
        let raw_numerator = bigint_bits(left.numer())
            .checked_add(bigint_bits(right.denom()))
            .ok_or(CayleyError::ResourceLimitExceeded {
                stage,
                resource: "intermediate_bits",
            })?;
        let raw_denominator = bigint_bits(left.denom())
            .checked_add(bigint_bits(right.numer()))
            .ok_or(CayleyError::ResourceLimitExceeded {
                stage,
                resource: "intermediate_bits",
            })?;
        if raw_numerator.max(raw_denominator) <= self.limits.max_intermediate_bits {
            self.preflight_value_bits(stage, raw_numerator)?;
            self.preflight_value_bits(stage, raw_denominator)?;
            let result = left / right;
            self.observe_rational(stage, &result)?;
            return Ok(result);
        }

        let numerator_gcd = self.gcd_fallback(left.numer(), right.numer(), stage)?;
        let denominator_gcd = self.gcd_fallback(left.denom(), right.denom(), stage)?;
        let left_numerator = left.numer() / &numerator_gcd;
        let right_numerator = right.numer() / numerator_gcd;
        let right_denominator = right.denom() / &denominator_gcd;
        let left_denominator = left.denom() / denominator_gcd;
        self.preflight_value_bits(
            stage,
            refined_product_bits(&left_numerator, &right_denominator, stage)?,
        )?;
        self.preflight_value_bits(
            stage,
            refined_product_bits(&left_denominator, &right_numerator, stage)?,
        )?;
        let mut numerator = left_numerator * right_denominator;
        let mut denominator = left_denominator * right_numerator;
        if denominator.is_negative() {
            numerator = -numerator;
            denominator = -denominator;
        }
        let result = BigRational::new_raw(numerator, denominator);
        self.observe_rational(stage, &result)?;
        Ok(result)
    }

    fn observe_rational(
        &mut self,
        stage: CayleyStage,
        value: &BigRational,
    ) -> Result<(), CayleyError> {
        let bits = rational_bits(value);
        self.work.max_observed_bits = self.work.max_observed_bits.max(bits);
        if bits > self.limits.max_intermediate_bits {
            return Err(CayleyError::ResourceLimitExceeded {
                stage,
                resource: "intermediate_bits",
            });
        }
        Ok(())
    }

    fn observe_output(&mut self, value: &BigRational) -> Result<(), CayleyError> {
        let bits = rational_bits(value);
        self.work.max_observed_bits = self.work.max_observed_bits.max(bits);
        self.work.max_output_bits = self.work.max_output_bits.max(bits);
        if bits > self.limits.max_output_bits {
            return Err(CayleyError::ResourceLimitExceeded {
                stage: CayleyStage::Output,
                resource: "output_bits",
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExactPoint3 {
    coordinates: [BigRational; 3],
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExactVector3 {
    coordinates: [BigRational; 3],
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExactLocalRotation {
    rotation: [[BigRational; 3]; 3],
    translation: ExactVector3,
    certificate: ExactAngleCertificate,
    work: CayleyWork,
    version: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ExactTreePoseLimits {
    max_faces: usize,
    max_hinges: usize,
    max_adjacency_entries: usize,
    max_boundary_occurrences: usize,
    max_boundary_edge_index_entries: usize,
    max_boundary_edge_index_operations: usize,
    max_unique_vertices: usize,
    max_total_machin_terms: usize,
    max_total_trig_terms: usize,
    max_total_sqrt_refinements: usize,
    max_total_output_bits: usize,
    cayley: CayleyLimits,
}

impl Default for ExactTreePoseLimits {
    fn default() -> Self {
        Self {
            max_faces: 10_001,
            max_hinges: 10_000,
            max_adjacency_entries: 20_000,
            max_boundary_occurrences: 1_000_000,
            max_boundary_edge_index_entries: 1_000_000,
            max_boundary_edge_index_operations: 1_000_000,
            max_unique_vertices: 1_000_000,
            max_total_machin_terms: 4_000_000,
            max_total_trig_terms: 8_000_000,
            max_total_sqrt_refinements: 640_000,
            max_total_output_bits: 128_000_000,
            cayley: CayleyLimits {
                max_interval_operations: 10_000_000,
                // A tree reuses one meter for every hinge. Keep the fallback
                // finite while allowing the per-kernel budget to aggregate.
                max_gcd_fallback_calls: 262_144,
                max_gcd_fallback_input_bits: 1_073_741_824,
                ..CayleyLimits::default()
            },
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct ExactTreePoseWork {
    faces: usize,
    hinges: usize,
    adjacency_entries: usize,
    boundary_occurrences: usize,
    boundary_edge_index_entries: usize,
    boundary_edge_index_operations: usize,
    unique_vertices: usize,
    max_output_bits: usize,
    total_output_bits: usize,
    exact: CayleyWork,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExactRigidTransform {
    rotation: [[BigRational; 3]; 3],
    translation: ExactVector3,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExactFacePose {
    face: FaceId,
    transform: ExactRigidTransform,
    boundary: Vec<(VertexId, ExactPoint3)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExactHingePose {
    edge: EdgeId,
    parent: FaceId,
    child: FaceId,
    rotation_sign: i8,
    angle_magnitude_bits: u64,
    certificate: ExactAngleCertificate,
    endpoint_vertices: [VertexId; 2],
    world_endpoints: [ExactPoint3; 2],
}

#[derive(Debug)]
struct RationalCayleyTreePose<'a> {
    bound: BoundMaterialTreePose<'a>,
    fixed_face: Option<FaceId>,
    faces: Vec<ExactFacePose>,
    hinges: Vec<ExactHingePose>,
    work: ExactTreePoseWork,
    version: &'static str,
}

impl RationalCayleyTreePose<'_> {
    fn is_for(&self, bound: BoundMaterialTreePose<'_>) -> bool {
        self.bound.model() == bound.model() && self.bound.pose().same_instance(bound.pose())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ExactAngleCertificate {
    Exact { target_degrees: BigRational },
    Bounded(Box<BoundedAngleCertificate>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BoundedAngleCertificate {
    target_degrees: BigRational,
    precision_bits: usize,
    parameter: BigRational,
    target_half_tangent: RationalInterval,
    realized_half_tangent: RationalInterval,
    max_error_radians: BigRational,
    max_error_degrees: BigRational,
    acceptance_degrees: BigRational,
    pi: RationalInterval,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RationalInterval {
    lower: BigRational,
    upper: BigRational,
}

impl RationalInterval {
    fn new(lower: BigRational, upper: BigRational) -> Result<Self, CayleyError> {
        if lower > upper {
            return Err(CayleyError::CertificateUnavailable {
                stage: CayleyStage::Candidate,
            });
        }
        Ok(Self { lower, upper })
    }

    fn point(value: BigRational) -> Self {
        Self {
            lower: value.clone(),
            upper: value,
        }
    }
}

/// An integer interval whose represented values are divided by `2^precision`.
///
/// Every multiplication and division rounds its lower endpoint down and its
/// upper endpoint up.  Consequently recurrence rounding can widen, but never
/// invalidate, a trigonometric enclosure.
#[derive(Debug, Clone, PartialEq, Eq)]
struct DyadicInterval {
    lower: BigInt,
    upper: BigInt,
    precision: usize,
}

impl DyadicInterval {
    fn from_rational_outward(
        interval: &RationalInterval,
        precision: usize,
        meter: &mut WorkMeter<'_>,
        stage: CayleyStage,
    ) -> Result<Self, CayleyError> {
        meter.preflight_shifted_value(stage, 1, precision)?;
        meter.operation(stage)?;
        let scale = BigInt::one() << precision;
        meter.preflight_product_bits(
            stage,
            bigint_bits(interval.lower.numer()),
            bigint_bits(&scale),
        )?;
        meter.preflight_product_bits(
            stage,
            bigint_bits(interval.upper.numer()),
            bigint_bits(&scale),
        )?;
        let lower = div_floor(&(interval.lower.numer() * &scale), interval.lower.denom());
        let upper = div_ceil(&(interval.upper.numer() * &scale), interval.upper.denom());
        meter.work.max_observed_bits = meter
            .work
            .max_observed_bits
            .max(bigint_bits(&lower))
            .max(bigint_bits(&upper));
        Ok(Self {
            lower,
            upper,
            precision,
        })
    }

    fn point(value: BigInt, precision: usize) -> Self {
        Self {
            lower: value.clone(),
            upper: value,
            precision,
        }
    }

    fn zero(precision: usize) -> Self {
        Self::point(BigInt::zero(), precision)
    }

    fn add(
        &self,
        other: &Self,
        meter: &mut WorkMeter<'_>,
        stage: CayleyStage,
    ) -> Result<Self, CayleyError> {
        self.require_same_precision(other, stage)?;
        meter.operation(stage)?;
        meter.preflight_value_bits(
            stage,
            bigint_bits(&self.lower)
                .max(bigint_bits(&self.upper))
                .max(bigint_bits(&other.lower))
                .max(bigint_bits(&other.upper))
                .saturating_add(1),
        )?;
        Ok(Self {
            lower: &self.lower + &other.lower,
            upper: &self.upper + &other.upper,
            precision: self.precision,
        })
    }

    fn subtract(
        &self,
        other: &Self,
        meter: &mut WorkMeter<'_>,
        stage: CayleyStage,
    ) -> Result<Self, CayleyError> {
        self.require_same_precision(other, stage)?;
        meter.operation(stage)?;
        meter.preflight_value_bits(
            stage,
            bigint_bits(&self.lower)
                .max(bigint_bits(&self.upper))
                .max(bigint_bits(&other.lower))
                .max(bigint_bits(&other.upper))
                .saturating_add(1),
        )?;
        Ok(Self {
            lower: &self.lower - &other.upper,
            upper: &self.upper - &other.lower,
            precision: self.precision,
        })
    }

    fn multiply(
        &self,
        other: &Self,
        meter: &mut WorkMeter<'_>,
        stage: CayleyStage,
    ) -> Result<Self, CayleyError> {
        self.require_same_precision(other, stage)?;
        meter.operation(stage)?;
        meter.preflight_shifted_value(stage, 1, self.precision)?;
        meter.preflight_product_bits(
            stage,
            bigint_bits(&self.lower).max(bigint_bits(&self.upper)),
            bigint_bits(&other.lower).max(bigint_bits(&other.upper)),
        )?;
        let products = [
            &self.lower * &other.lower,
            &self.lower * &other.upper,
            &self.upper * &other.lower,
            &self.upper * &other.upper,
        ];
        let min_product = products.iter().min().expect("four products").to_owned();
        let max_product = products.iter().max().expect("four products").to_owned();
        let denominator = BigInt::one() << self.precision;
        Ok(Self {
            lower: div_floor(&min_product, &denominator),
            upper: div_ceil(&max_product, &denominator),
            precision: self.precision,
        })
    }

    fn divide_positive_integer(
        &self,
        divisor: usize,
        meter: &mut WorkMeter<'_>,
        stage: CayleyStage,
    ) -> Result<Self, CayleyError> {
        if divisor == 0 {
            return Err(CayleyError::CertificateUnavailable { stage });
        }
        meter.operation(stage)?;
        let divisor = BigInt::from(divisor);
        Ok(Self {
            lower: div_floor(&self.lower, &divisor),
            upper: div_ceil(&self.upper, &divisor),
            precision: self.precision,
        })
    }

    fn to_rational(
        &self,
        meter: &mut WorkMeter<'_>,
        stage: CayleyStage,
    ) -> Result<RationalInterval, CayleyError> {
        meter.preflight_shifted_value(stage, 1, self.precision)?;
        let denominator = BigInt::one() << self.precision;
        Ok(RationalInterval {
            lower: BigRational::new(self.lower.clone(), denominator.clone()),
            upper: BigRational::new(self.upper.clone(), denominator),
        })
    }

    fn require_same_precision(&self, other: &Self, stage: CayleyStage) -> Result<(), CayleyError> {
        if self.precision != other.precision {
            return Err(CayleyError::CertificateUnavailable { stage });
        }
        Ok(())
    }
}

fn local_rotation_v1(
    pivot: [f64; 3],
    end: [f64; 3],
    angle_magnitude_degrees: f64,
    rotation_sign: i8,
    limits: CayleyLimits,
) -> Result<ExactLocalRotation, CayleyError> {
    let mut meter = WorkMeter::new(&limits);
    local_rotation_v1_with_meter(
        pivot,
        end,
        angle_magnitude_degrees,
        rotation_sign,
        &mut meter,
    )
}

fn local_rotation_v1_with_meter(
    pivot: [f64; 3],
    end: [f64; 3],
    angle_magnitude_degrees: f64,
    rotation_sign: i8,
    meter: &mut WorkMeter<'_>,
) -> Result<ExactLocalRotation, CayleyError> {
    let limits = *meter.limits;
    if !matches!(rotation_sign, -1 | 1) {
        return Err(CayleyError::InvalidRotationSign {
            stage: CayleyStage::Input,
        });
    }
    if limits.max_guard_bits < DEFAULT_GUARD_BITS {
        return Err(CayleyError::ResourceLimitExceeded {
            stage: CayleyStage::Input,
            resource: "guard_bits",
        });
    }
    if limits.max_candidate_bits < DEFAULT_CANDIDATE_BITS {
        return Err(CayleyError::ResourceLimitExceeded {
            stage: CayleyStage::Input,
            resource: "candidate_bits",
        });
    }
    let pivot = exact_point(pivot, meter)?;
    let end = exact_point(end, meter)?;
    let absolute_degrees = exact_f64(angle_magnitude_degrees, meter, CayleyStage::Input)?;
    let maximum = BigRational::from_integer(BigInt::from(DEGREE_180));
    if absolute_degrees.is_negative() || absolute_degrees > maximum {
        return Err(CayleyError::AngleOutOfRange {
            stage: CayleyStage::Input,
        });
    }
    let target_degrees = if rotation_sign < 0 {
        -absolute_degrees.clone()
    } else {
        absolute_degrees.clone()
    };

    let raw_direction = exact_between(&pivot, &end, meter)?;
    let (direction, delta) = normalize_direction(&raw_direction, meter)?;

    if absolute_degrees.is_zero() {
        let rotation = identity_matrix();
        let translation = zero_vector();
        verify_invariants(&rotation, &translation, &pivot, &end, &direction, meter)?;
        observe_transform_output(&rotation, &translation, meter)?;
        return Ok(ExactLocalRotation {
            rotation,
            translation,
            certificate: ExactAngleCertificate::Exact { target_degrees },
            work: meter.work.clone(),
            version: RATIONAL_CAYLEY_LOCAL_ROTATION_V1,
        });
    }

    if absolute_degrees == maximum {
        let rotation = half_turn(&direction, &delta, meter)?;
        let translation = fixed_point_translation(&rotation, &pivot, meter)?;
        verify_invariants(&rotation, &translation, &pivot, &end, &direction, meter)?;
        observe_transform_output(&rotation, &translation, meter)?;
        return Ok(ExactLocalRotation {
            rotation,
            translation,
            certificate: ExactAngleCertificate::Exact { target_degrees },
            work: meter.work.clone(),
            version: RATIONAL_CAYLEY_LOCAL_ROTATION_V1,
        });
    }

    let acceptance = adjacent_angle_acceptance(angle_magnitude_degrees, meter)?;

    if absolute_degrees == BigRational::from_integer(BigInt::from(90_u8))
        && let Some(root) = exact_rational_square_root(&delta, meter)?
    {
        let sign = if rotation_sign < 0 {
            -BigRational::one()
        } else {
            BigRational::one()
        };
        let parameter = meter.divide_rational(&sign, &root, CayleyStage::Candidate)?;
        let rotation = cayley_matrix(&direction, &delta, &parameter, meter)?;
        let translation = fixed_point_translation(&rotation, &pivot, meter)?;
        verify_invariants(&rotation, &translation, &pivot, &end, &direction, meter)?;
        observe_transform_output(&rotation, &translation, meter)?;
        return Ok(ExactLocalRotation {
            rotation,
            translation,
            certificate: ExactAngleCertificate::Exact { target_degrees },
            work: meter.work.clone(),
            version: RATIONAL_CAYLEY_LOCAL_ROTATION_V1,
        });
    }

    let allowed_rounds = limits.max_precision_rounds.min(PRECISION_ROUNDS.len());
    for &precision in &PRECISION_ROUNDS[..allowed_rounds] {
        let checkpoint = meter.work.clone();
        match approximate_angle(
            &absolute_degrees,
            rotation_sign < 0,
            angle_magnitude_degrees,
            &direction,
            &delta,
            &acceptance,
            precision,
            meter,
        ) {
            Ok((rotation, certificate)) => {
                let translation = fixed_point_translation(&rotation, &pivot, meter)?;
                verify_invariants(&rotation, &translation, &pivot, &end, &direction, meter)?;
                observe_transform_output(&rotation, &translation, meter)?;
                return Ok(ExactLocalRotation {
                    rotation,
                    translation,
                    certificate,
                    work: meter.work.clone(),
                    version: RATIONAL_CAYLEY_LOCAL_ROTATION_V1,
                });
            }
            Err(CayleyError::CertificateUnavailable { .. }) => {
                // A refinement round is a deterministic retry. Work remains
                // charged, while the prior max-bit observation is retained.
                meter.work.max_observed_bits = meter
                    .work
                    .max_observed_bits
                    .max(checkpoint.max_observed_bits);
            }
            Err(error) => return Err(error),
        }
    }
    Err(CayleyError::CertificateUnavailable {
        stage: CayleyStage::Candidate,
    })
}

#[derive(Debug, Clone, Copy)]
struct ExactTreeNeighbor {
    face: FaceId,
    hinge_index: usize,
    rotation_sign: i8,
}

#[derive(Debug)]
struct AuthenticatedBoundaryEdgeIndex {
    entries: HashMap<(FaceId, EdgeId), [VertexId; 2]>,
    boundary_occurrences: usize,
    operations: usize,
}

/// Builds a watertight rational pose bound to one exact native pose issuer.
///
/// This proof intentionally does not contain or authorize the binary64
/// renderer snapshot. Renderer containment and the collision safe-set
/// connection remain later gates.
fn prepare_rational_cayley_tree_pose_v1<'a>(
    bound: BoundMaterialTreePose<'a>,
    limits: ExactTreePoseLimits,
) -> Result<RationalCayleyTreePose<'a>, CayleyError> {
    let model = bound.model();
    let pose = bound.pose();
    let face_ids = model.face_ids();
    let hinges = model.hinges();
    let angles = pose.hinge_angles();
    let face_count = face_ids.len();
    let hinge_count = hinges.len();

    if face_count == 0 || face_count > limits.max_faces || hinge_count > limits.max_hinges {
        return Err(CayleyError::ResourceLimitExceeded {
            stage: CayleyStage::Tree,
            resource: if face_count > limits.max_faces || face_count == 0 {
                "faces"
            } else {
                "hinges"
            },
        });
    }
    if hinge_count
        .checked_add(1)
        .is_none_or(|expected| expected != face_count)
        && !(hinge_count == 0 && face_count == 1)
    {
        return Err(CayleyError::BoundTreeInconsistent {
            stage: CayleyStage::Tree,
        });
    }
    if !strictly_canonical_faces(face_ids)
        || !strictly_canonical_hinges(hinges)
        || angles.len() != hinge_count
        || !hinges.iter().zip(angles).all(|(hinge, angle)| {
            hinge.edge() == angle.edge()
                && angle.angle_degrees().is_finite()
                && (0.0..=180.0).contains(&angle.angle_degrees())
        })
    {
        return Err(CayleyError::BoundTreeInconsistent {
            stage: CayleyStage::Tree,
        });
    }

    let adjacency_entries =
        hinge_count
            .checked_mul(2)
            .ok_or(CayleyError::ResourceLimitExceeded {
                stage: CayleyStage::Tree,
                resource: "adjacency_entries",
            })?;
    if adjacency_entries > limits.max_adjacency_entries {
        return Err(CayleyError::ResourceLimitExceeded {
            stage: CayleyStage::Tree,
            resource: "adjacency_entries",
        });
    }

    let boundary_edge_index = build_authenticated_boundary_edge_index(bound, face_ids, &limits)?;
    let boundary_occurrences = boundary_edge_index.boundary_occurrences;

    let fixed_face = pose.fixed_face();
    let root = if hinge_count == 0 {
        if fixed_face.is_some() {
            return Err(CayleyError::BoundTreeInconsistent {
                stage: CayleyStage::Tree,
            });
        }
        face_ids[0]
    } else {
        let root = fixed_face.ok_or(CayleyError::BoundTreeInconsistent {
            stage: CayleyStage::Tree,
        })?;
        if face_ids
            .binary_search_by_key(&root.canonical_bytes(), FaceId::canonical_bytes)
            .is_err()
        {
            return Err(CayleyError::BoundTreeInconsistent {
                stage: CayleyStage::Tree,
            });
        }
        root
    };
    if pose.face_transform(root) != Some(model.identity_transform()) {
        return Err(CayleyError::BoundTreeInconsistent {
            stage: CayleyStage::Tree,
        });
    }

    let mut meter = WorkMeter::with_total_term_limits(
        &limits.cayley,
        TotalTermLimits {
            machin_terms: limits.max_total_machin_terms,
            trig_terms: limits.max_total_trig_terms,
            sqrt_refinements: limits.max_total_sqrt_refinements,
        },
    );
    let mut adjacency = HashMap::<FaceId, Vec<ExactTreeNeighbor>>::new();
    adjacency
        .try_reserve(face_count)
        .map_err(|_| CayleyError::ResourceLimitExceeded {
            stage: CayleyStage::Tree,
            resource: "faces",
        })?;
    for face in face_ids {
        adjacency.insert(*face, Vec::new());
    }
    for (hinge_index, hinge) in hinges.iter().enumerate() {
        if hinge.left_face() == hinge.right_face()
            || !adjacency.contains_key(&hinge.left_face())
            || !adjacency.contains_key(&hinge.right_face())
        {
            return Err(CayleyError::BoundTreeInconsistent {
                stage: CayleyStage::Tree,
            });
        }
        let base_sign = match hinge.assignment() {
            FoldAssignment::Mountain => 1_i8,
            FoldAssignment::Valley => -1_i8,
        };
        for (face, neighbor, rotation_sign) in [
            (hinge.left_face(), hinge.right_face(), base_sign),
            (hinge.right_face(), hinge.left_face(), -base_sign),
        ] {
            let neighbors = adjacency
                .get_mut(&face)
                .ok_or(CayleyError::BoundTreeInconsistent {
                    stage: CayleyStage::Tree,
                })?;
            neighbors
                .try_reserve(1)
                .map_err(|_| CayleyError::ResourceLimitExceeded {
                    stage: CayleyStage::Tree,
                    resource: "adjacency_entries",
                })?;
            neighbors.push(ExactTreeNeighbor {
                face: neighbor,
                hinge_index,
                rotation_sign,
            });
        }
    }
    for neighbors in adjacency.values_mut() {
        neighbors
            .sort_unstable_by_key(|neighbor| hinges[neighbor.hinge_index].edge().canonical_bytes());
    }

    let mut max_output_bits = 0_usize;
    let mut total_output_bits = 0_usize;
    let mut transforms = HashMap::<FaceId, ExactRigidTransform>::new();
    transforms
        .try_reserve(face_count)
        .map_err(|_| CayleyError::ResourceLimitExceeded {
            stage: CayleyStage::Tree,
            resource: "faces",
        })?;
    let root_transform = exact_identity_transform();
    charge_transform_output(
        &root_transform,
        &mut max_output_bits,
        &mut total_output_bits,
        limits.cayley.max_output_bits,
        limits.max_total_output_bits,
    )?;
    transforms.insert(root, root_transform);
    let mut hinge_poses = Vec::<Option<ExactHingePose>>::new();
    hinge_poses
        .try_reserve_exact(hinge_count)
        .map_err(|_| CayleyError::ResourceLimitExceeded {
            stage: CayleyStage::Tree,
            resource: "hinges",
        })?;
    hinge_poses.resize_with(hinge_count, || None);
    let mut queue = VecDeque::new();
    queue
        .try_reserve(face_count)
        .map_err(|_| CayleyError::ResourceLimitExceeded {
            stage: CayleyStage::Tree,
            resource: "faces",
        })?;
    queue.push_back(root);

    while let Some(parent_face) = queue.pop_front() {
        meter.operation(CayleyStage::Tree)?;
        let parent = transforms
            .get(&parent_face)
            .ok_or(CayleyError::BoundTreeInconsistent {
                stage: CayleyStage::Tree,
            })?;
        charge_transform_output(
            parent,
            &mut max_output_bits,
            &mut total_output_bits,
            limits.cayley.max_output_bits,
            limits.max_total_output_bits,
        )?;
        let parent = parent.clone();
        let neighbors = adjacency
            .get(&parent_face)
            .ok_or(CayleyError::BoundTreeInconsistent {
                stage: CayleyStage::Tree,
            })?;
        for neighbor in neighbors {
            if transforms.contains_key(&neighbor.face) {
                continue;
            }
            let hinge =
                hinges
                    .get(neighbor.hinge_index)
                    .ok_or(CayleyError::BoundTreeInconsistent {
                        stage: CayleyStage::Tree,
                    })?;
            if pose.hinge_parent_transform(hinge.edge()) != pose.face_transform(parent_face) {
                return Err(CayleyError::BoundTreeInconsistent {
                    stage: CayleyStage::Tree,
                });
            }
            let angle =
                angles
                    .get(neighbor.hinge_index)
                    .ok_or(CayleyError::BoundTreeInconsistent {
                        stage: CayleyStage::Tree,
                    })?;
            let local = local_rotation_v1_with_meter(
                point3_array(hinge.start()),
                point3_array(hinge.end()),
                angle.angle_degrees(),
                neighbor.rotation_sign,
                &mut meter,
            )?;
            check_tree_exact_aggregates(&meter.work, &limits)?;
            let local_transform = ExactRigidTransform {
                rotation: local.rotation,
                translation: local.translation,
            };
            let child = compose_exact_transform(&parent, &local_transform, &mut meter)?;
            verify_exact_rotation(&child.rotation, &mut meter)?;

            let rest_start = exact_point(point3_array(hinge.start()), &mut meter)?;
            let rest_end = exact_point(point3_array(hinge.end()), &mut meter)?;
            let parent_start = apply_exact_transform(&parent, &rest_start, &mut meter)?;
            let parent_end = apply_exact_transform(&parent, &rest_end, &mut meter)?;
            let child_start = apply_exact_transform(&child, &rest_start, &mut meter)?;
            let child_end = apply_exact_transform(&child, &rest_end, &mut meter)?;
            if parent_start != child_start || parent_end != child_end {
                return Err(CayleyError::InvariantFailure {
                    stage: CayleyStage::Tree,
                });
            }
            let endpoint_vertices = authenticated_hinge_endpoint_vertices(
                bound,
                &boundary_edge_index,
                hinge,
                &rest_start,
                &rest_end,
                &mut meter,
            )?;
            charge_transform_output(
                &child,
                &mut max_output_bits,
                &mut total_output_bits,
                limits.cayley.max_output_bits,
                limits.max_total_output_bits,
            )?;
            charge_angle_certificate_output(
                &local.certificate,
                &mut max_output_bits,
                &mut total_output_bits,
                limits.cayley.max_output_bits,
                limits.max_total_output_bits,
            )?;
            for endpoint in [&parent_start, &parent_end] {
                charge_point_output(
                    endpoint,
                    &mut max_output_bits,
                    &mut total_output_bits,
                    limits.cayley.max_output_bits,
                    limits.max_total_output_bits,
                )?;
            }
            if hinge_poses[neighbor.hinge_index]
                .replace(ExactHingePose {
                    edge: hinge.edge(),
                    parent: parent_face,
                    child: neighbor.face,
                    rotation_sign: neighbor.rotation_sign,
                    angle_magnitude_bits: angle.angle_degrees().to_bits(),
                    certificate: local.certificate,
                    endpoint_vertices,
                    world_endpoints: [parent_start, parent_end],
                })
                .is_some()
                || transforms.insert(neighbor.face, child).is_some()
            {
                return Err(CayleyError::BoundTreeInconsistent {
                    stage: CayleyStage::Tree,
                });
            }
            queue.push_back(neighbor.face);
        }
    }
    if transforms.len() != face_count || hinge_poses.iter().any(Option::is_none) {
        return Err(CayleyError::BoundTreeInconsistent {
            stage: CayleyStage::Tree,
        });
    }
    check_tree_exact_aggregates(&meter.work, &limits)?;

    let mut vertex_registry = HashMap::<VertexId, ExactPoint3>::new();
    vertex_registry
        .try_reserve(boundary_occurrences.min(limits.max_unique_vertices))
        .map_err(|_| CayleyError::ResourceLimitExceeded {
            stage: CayleyStage::Tree,
            resource: "unique_vertices",
        })?;
    let mut faces = Vec::new();
    faces
        .try_reserve_exact(face_count)
        .map_err(|_| CayleyError::ResourceLimitExceeded {
            stage: CayleyStage::Tree,
            resource: "faces",
        })?;
    for face in face_ids {
        let transform = transforms
            .remove(face)
            .ok_or(CayleyError::BoundTreeInconsistent {
                stage: CayleyStage::Tree,
            })?;
        verify_exact_rotation(&transform.rotation, &mut meter)?;
        let source_boundary =
            bound
                .face_boundary(*face)
                .ok_or(CayleyError::BoundTreeInconsistent {
                    stage: CayleyStage::Tree,
                })?;
        let mut boundary = Vec::new();
        boundary
            .try_reserve_exact(source_boundary.vertices().len())
            .map_err(|_| CayleyError::ResourceLimitExceeded {
                stage: CayleyStage::Tree,
                resource: "boundary_occurrences",
            })?;
        for vertex in source_boundary.vertices() {
            let source =
                model
                    .vertex_position(*vertex)
                    .ok_or(CayleyError::BoundTreeInconsistent {
                        stage: CayleyStage::Tree,
                    })?;
            let rest = exact_point(point3_array(source), &mut meter)?;
            let current = apply_exact_transform(&transform, &rest, &mut meter)?;
            charge_point_output(
                &current,
                &mut max_output_bits,
                &mut total_output_bits,
                limits.cayley.max_output_bits,
                limits.max_total_output_bits,
            )?;
            // The registry and the per-face boundary both retain a copy
            // during validation, so reserve logical storage for the clone
            // before allocating it.
            charge_point_output(
                &current,
                &mut max_output_bits,
                &mut total_output_bits,
                limits.cayley.max_output_bits,
                limits.max_total_output_bits,
            )?;
            if !vertex_registry.contains_key(vertex)
                && vertex_registry.len() >= limits.max_unique_vertices
            {
                return Err(CayleyError::ResourceLimitExceeded {
                    stage: CayleyStage::Tree,
                    resource: "unique_vertices",
                });
            }
            match vertex_registry.entry(*vertex) {
                std::collections::hash_map::Entry::Vacant(entry) => {
                    entry.insert(current.clone());
                }
                std::collections::hash_map::Entry::Occupied(entry) if entry.get() == &current => {}
                std::collections::hash_map::Entry::Occupied(_) => {
                    return Err(CayleyError::InvariantFailure {
                        stage: CayleyStage::Tree,
                    });
                }
            }
            boundary.push((*vertex, current));
        }
        faces.push(ExactFacePose {
            face: *face,
            transform,
            boundary,
        });
    }

    let mut hinges = Vec::new();
    hinges
        .try_reserve_exact(hinge_count)
        .map_err(|_| CayleyError::ResourceLimitExceeded {
            stage: CayleyStage::Tree,
            resource: "hinges",
        })?;
    for hinge in hinge_poses {
        let hinge = hinge.ok_or(CayleyError::BoundTreeInconsistent {
            stage: CayleyStage::Tree,
        })?;
        for (vertex, endpoint) in hinge.endpoint_vertices.iter().zip(&hinge.world_endpoints) {
            if vertex_registry.get(vertex) != Some(endpoint) {
                return Err(CayleyError::InvariantFailure {
                    stage: CayleyStage::Tree,
                });
            }
        }
        hinges.push(hinge);
    }
    check_tree_exact_aggregates(&meter.work, &limits)?;

    Ok(RationalCayleyTreePose {
        bound,
        fixed_face,
        faces,
        hinges,
        work: ExactTreePoseWork {
            faces: face_count,
            hinges: hinge_count,
            adjacency_entries,
            boundary_occurrences,
            boundary_edge_index_entries: boundary_edge_index.entries.len(),
            boundary_edge_index_operations: boundary_edge_index.operations,
            unique_vertices: vertex_registry.len(),
            max_output_bits: max_output_bits.max(meter.work.max_output_bits),
            total_output_bits,
            exact: meter.work,
        },
        version: RATIONAL_CAYLEY_TREE_POSE_V1,
    })
}

fn strictly_canonical_faces(faces: &[FaceId]) -> bool {
    !faces.is_empty()
        && faces
            .windows(2)
            .all(|pair| pair[0].canonical_bytes() < pair[1].canonical_bytes())
}

fn strictly_canonical_hinges(hinges: &[ori_kinematics::TreeHinge]) -> bool {
    hinges
        .windows(2)
        .all(|pair| pair[0].edge().canonical_bytes() < pair[1].edge().canonical_bytes())
}

fn point3_array(point: Point3) -> [f64; 3] {
    [point.x(), point.y(), point.z()]
}

fn exact_identity_transform() -> ExactRigidTransform {
    ExactRigidTransform {
        rotation: identity_matrix(),
        translation: zero_vector(),
    }
}

fn compose_exact_transform(
    parent: &ExactRigidTransform,
    local: &ExactRigidTransform,
    meter: &mut WorkMeter<'_>,
) -> Result<ExactRigidTransform, CayleyError> {
    let rotation = try_array3(|row| {
        try_array3(|column| {
            let mut value = BigRational::zero();
            for index in 0..3 {
                let product = meter.multiply_rational(
                    &parent.rotation[row][index],
                    &local.rotation[index][column],
                    CayleyStage::Tree,
                )?;
                value = meter.add_rational(&value, &product, CayleyStage::Tree)?;
            }
            Ok(value)
        })
    })?;
    let translation = ExactVector3 {
        coordinates: try_array3(|row| {
            let mut value = parent.translation.coordinates[row].clone();
            for column in 0..3 {
                let product = meter.multiply_rational(
                    &parent.rotation[row][column],
                    &local.translation.coordinates[column],
                    CayleyStage::Tree,
                )?;
                value = meter.add_rational(&value, &product, CayleyStage::Tree)?;
            }
            Ok(value)
        })?,
    };
    Ok(ExactRigidTransform {
        rotation,
        translation,
    })
}

fn apply_exact_transform(
    transform: &ExactRigidTransform,
    point: &ExactPoint3,
    meter: &mut WorkMeter<'_>,
) -> Result<ExactPoint3, CayleyError> {
    apply_point(&transform.rotation, &transform.translation, point, meter)
}

fn verify_exact_rotation(
    rotation: &[[BigRational; 3]; 3],
    meter: &mut WorkMeter<'_>,
) -> Result<(), CayleyError> {
    for row in 0..3 {
        for column in 0..3 {
            let mut value = BigRational::zero();
            for rotation_row in rotation {
                let product = meter.multiply_rational(
                    &rotation_row[row],
                    &rotation_row[column],
                    CayleyStage::Tree,
                )?;
                value = meter.add_rational(&value, &product, CayleyStage::Tree)?;
            }
            let expected = if row == column {
                BigRational::one()
            } else {
                BigRational::zero()
            };
            if value != expected {
                return Err(CayleyError::InvariantFailure {
                    stage: CayleyStage::Tree,
                });
            }
        }
    }
    if determinant(rotation, meter)? != BigRational::one() {
        return Err(CayleyError::InvariantFailure {
            stage: CayleyStage::Tree,
        });
    }
    Ok(())
}

fn build_authenticated_boundary_edge_index(
    bound: BoundMaterialTreePose<'_>,
    faces: &[FaceId],
    limits: &ExactTreePoseLimits,
) -> Result<AuthenticatedBoundaryEdgeIndex, CayleyError> {
    let mut entries = HashMap::<(FaceId, EdgeId), [VertexId; 2]>::new();
    let mut boundary_occurrences = 0_usize;
    let mut operations = 0_usize;

    for face in faces {
        if bound.pose().face_transform(*face).is_none() {
            return Err(CayleyError::BoundTreeInconsistent {
                stage: CayleyStage::Tree,
            });
        }
        let boundary = bound
            .face_boundary(*face)
            .ok_or(CayleyError::BoundTreeInconsistent {
                stage: CayleyStage::Tree,
            })?;
        let occurrence_count = boundary.vertices().len();
        if boundary.face() != *face
            || occurrence_count < 3
            || occurrence_count != boundary.edges().len()
        {
            return Err(CayleyError::BoundTreeInconsistent {
                stage: CayleyStage::Tree,
            });
        }

        let next_boundary_occurrences = boundary_occurrences.checked_add(occurrence_count).ok_or(
            CayleyError::ResourceLimitExceeded {
                stage: CayleyStage::Tree,
                resource: "boundary_occurrences",
            },
        )?;
        if next_boundary_occurrences > limits.max_boundary_occurrences {
            return Err(CayleyError::ResourceLimitExceeded {
                stage: CayleyStage::Tree,
                resource: "boundary_occurrences",
            });
        }
        let next_entries = entries.len().checked_add(occurrence_count).ok_or(
            CayleyError::ResourceLimitExceeded {
                stage: CayleyStage::Tree,
                resource: "boundary_edge_index_entries",
            },
        )?;
        if next_entries > limits.max_boundary_edge_index_entries {
            return Err(CayleyError::ResourceLimitExceeded {
                stage: CayleyStage::Tree,
                resource: "boundary_edge_index_entries",
            });
        }
        let next_operations =
            operations
                .checked_add(occurrence_count)
                .ok_or(CayleyError::ResourceLimitExceeded {
                    stage: CayleyStage::Tree,
                    resource: "boundary_edge_index_operations",
                })?;
        if next_operations > limits.max_boundary_edge_index_operations {
            return Err(CayleyError::ResourceLimitExceeded {
                stage: CayleyStage::Tree,
                resource: "boundary_edge_index_operations",
            });
        }
        entries
            .try_reserve(occurrence_count)
            .map_err(|_| CayleyError::ResourceLimitExceeded {
                stage: CayleyStage::Tree,
                resource: "boundary_edge_index_entries",
            })?;

        for (index, edge) in boundary.edges().iter().enumerate() {
            let endpoints = [
                boundary.vertices()[index],
                boundary.vertices()[(index + 1) % occurrence_count],
            ];
            if entries.insert((*face, *edge), endpoints).is_some() {
                return Err(CayleyError::BoundTreeInconsistent {
                    stage: CayleyStage::Tree,
                });
            }
        }
        boundary_occurrences = next_boundary_occurrences;
        operations = next_operations;
    }

    if entries.len() != boundary_occurrences {
        return Err(CayleyError::BoundTreeInconsistent {
            stage: CayleyStage::Tree,
        });
    }
    Ok(AuthenticatedBoundaryEdgeIndex {
        entries,
        boundary_occurrences,
        operations,
    })
}

fn authenticated_hinge_endpoint_vertices(
    bound: BoundMaterialTreePose<'_>,
    boundary_edge_index: &AuthenticatedBoundaryEdgeIndex,
    hinge: &ori_kinematics::TreeHinge,
    rest_start: &ExactPoint3,
    rest_end: &ExactPoint3,
    meter: &mut WorkMeter<'_>,
) -> Result<[VertexId; 2], CayleyError> {
    let left_pair = *boundary_edge_index
        .entries
        .get(&(hinge.left_face(), hinge.edge()))
        .ok_or(CayleyError::BoundTreeInconsistent {
            stage: CayleyStage::Tree,
        })?;
    let right_pair = *boundary_edge_index
        .entries
        .get(&(hinge.right_face(), hinge.edge()))
        .ok_or(CayleyError::BoundTreeInconsistent {
            stage: CayleyStage::Tree,
        })?;
    if !unordered_vertex_pair_eq(left_pair, right_pair) {
        return Err(CayleyError::BoundTreeInconsistent {
            stage: CayleyStage::Tree,
        });
    }
    let first =
        bound
            .model()
            .vertex_position(left_pair[0])
            .ok_or(CayleyError::BoundTreeInconsistent {
                stage: CayleyStage::Tree,
            })?;
    let second =
        bound
            .model()
            .vertex_position(left_pair[1])
            .ok_or(CayleyError::BoundTreeInconsistent {
                stage: CayleyStage::Tree,
            })?;
    let first = exact_point(point3_array(first), meter)?;
    let second = exact_point(point3_array(second), meter)?;
    if first == *rest_start && second == *rest_end {
        Ok(left_pair)
    } else if first == *rest_end && second == *rest_start {
        Ok([left_pair[1], left_pair[0]])
    } else {
        Err(CayleyError::BoundTreeInconsistent {
            stage: CayleyStage::Tree,
        })
    }
}

fn unordered_vertex_pair_eq(first: [VertexId; 2], second: [VertexId; 2]) -> bool {
    first == second || first == [second[1], second[0]]
}

fn check_tree_exact_aggregates(
    work: &CayleyWork,
    limits: &ExactTreePoseLimits,
) -> Result<(), CayleyError> {
    for (actual, maximum, resource) in [
        (
            work.machin_terms,
            limits.max_total_machin_terms,
            "total_machin_terms",
        ),
        (
            work.trig_terms,
            limits.max_total_trig_terms,
            "total_trig_terms",
        ),
        (
            work.sqrt_refinements,
            limits.max_total_sqrt_refinements,
            "total_sqrt_refinements",
        ),
    ] {
        if actual > maximum {
            return Err(CayleyError::ResourceLimitExceeded {
                stage: CayleyStage::Tree,
                resource,
            });
        }
    }
    Ok(())
}

fn charge_transform_output(
    transform: &ExactRigidTransform,
    max_observed: &mut usize,
    total: &mut usize,
    maximum_per_value: usize,
    maximum: usize,
) -> Result<(), CayleyError> {
    for value in transform
        .rotation
        .iter()
        .flat_map(|row| row.iter())
        .chain(transform.translation.coordinates.iter())
    {
        charge_rational_output(value, max_observed, total, maximum_per_value, maximum)?;
    }
    Ok(())
}

fn charge_point_output(
    point: &ExactPoint3,
    max_observed: &mut usize,
    total: &mut usize,
    maximum_per_value: usize,
    maximum: usize,
) -> Result<(), CayleyError> {
    for coordinate in &point.coordinates {
        charge_rational_output(coordinate, max_observed, total, maximum_per_value, maximum)?;
    }
    Ok(())
}

fn charge_angle_certificate_output(
    certificate: &ExactAngleCertificate,
    max_observed: &mut usize,
    total: &mut usize,
    maximum_per_value: usize,
    maximum: usize,
) -> Result<(), CayleyError> {
    match certificate {
        ExactAngleCertificate::Exact { target_degrees } => charge_rational_output(
            target_degrees,
            max_observed,
            total,
            maximum_per_value,
            maximum,
        ),
        ExactAngleCertificate::Bounded(certificate) => {
            for value in [
                &certificate.target_degrees,
                &certificate.parameter,
                &certificate.target_half_tangent.lower,
                &certificate.target_half_tangent.upper,
                &certificate.realized_half_tangent.lower,
                &certificate.realized_half_tangent.upper,
                &certificate.max_error_radians,
                &certificate.max_error_degrees,
                &certificate.acceptance_degrees,
                &certificate.pi.lower,
                &certificate.pi.upper,
            ] {
                charge_rational_output(value, max_observed, total, maximum_per_value, maximum)?;
            }
            Ok(())
        }
    }
}

fn charge_rational_output(
    value: &BigRational,
    max_observed: &mut usize,
    total: &mut usize,
    maximum_per_value: usize,
    maximum: usize,
) -> Result<(), CayleyError> {
    let value_bits = rational_bits(value);
    *max_observed = (*max_observed).max(value_bits);
    if value_bits > maximum_per_value {
        return Err(CayleyError::ResourceLimitExceeded {
            stage: CayleyStage::Tree,
            resource: "output_bits",
        });
    }
    let storage_bits = bigint_bits(value.numer())
        .checked_add(bigint_bits(value.denom()))
        .ok_or(CayleyError::ResourceLimitExceeded {
            stage: CayleyStage::Tree,
            resource: "total_output_bits",
        })?;
    let next_total = total
        .checked_add(storage_bits)
        .ok_or(CayleyError::ResourceLimitExceeded {
            stage: CayleyStage::Tree,
            resource: "total_output_bits",
        })?;
    if next_total > maximum {
        return Err(CayleyError::ResourceLimitExceeded {
            stage: CayleyStage::Tree,
            resource: "total_output_bits",
        });
    }
    *total = next_total;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn approximate_angle(
    absolute_degrees: &BigRational,
    negative: bool,
    original_angle: f64,
    direction: &ExactVector3,
    delta: &BigRational,
    acceptance: &BigRational,
    precision: usize,
    meter: &mut WorkMeter<'_>,
) -> Result<([[BigRational; 3]; 3], ExactAngleCertificate), CayleyError> {
    let pi = machin_pi_interval(precision, meter)?;
    let half_angle = half_angle_interval(absolute_degrees, &pi, meter)?;
    let twice_half_upper = meter.multiply_rational(
        &half_angle.upper,
        &BigRational::from_integer(BigInt::from(2_u8)),
        CayleyStage::Trigonometry,
    )?;
    if twice_half_upper >= pi.lower {
        return Err(CayleyError::CertificateUnavailable {
            stage: CayleyStage::Trigonometry,
        });
    }

    let half_tangent = tangent_interval(&half_angle, precision, meter)?;
    if half_tangent.lower.is_negative() || half_tangent.upper <= BigRational::zero() {
        return Err(CayleyError::CertificateUnavailable {
            stage: CayleyStage::Trigonometry,
        });
    }
    let sqrt_delta = square_root_interval(delta, precision, meter)?;
    if sqrt_delta.lower <= BigRational::zero() {
        return Err(CayleyError::CertificateUnavailable {
            stage: CayleyStage::SquareRoot,
        });
    }
    let parameter_interval = RationalInterval::new(
        meter.divide_rational(
            &half_tangent.lower,
            &sqrt_delta.upper,
            CayleyStage::Candidate,
        )?,
        meter.divide_rational(
            &half_tangent.upper,
            &sqrt_delta.lower,
            CayleyStage::Candidate,
        )?,
    )?;
    let unsigned_parameter =
        round_interval_midpoint_to_dyadic(&parameter_interval, DEFAULT_CANDIDATE_BITS, meter)?;
    if unsigned_parameter <= BigRational::zero() {
        return Err(CayleyError::CertificateUnavailable {
            stage: CayleyStage::Candidate,
        });
    }
    let realized_half_tangent = RationalInterval::new(
        meter.multiply_rational(
            &unsigned_parameter,
            &sqrt_delta.lower,
            CayleyStage::Candidate,
        )?,
        meter.multiply_rational(
            &unsigned_parameter,
            &sqrt_delta.upper,
            CayleyStage::Candidate,
        )?,
    )?;
    if realized_half_tangent.lower <= BigRational::zero() {
        return Err(CayleyError::CertificateUnavailable {
            stage: CayleyStage::Candidate,
        });
    }

    let first_gap = meter
        .subtract_rational(
            &half_tangent.upper,
            &realized_half_tangent.lower,
            CayleyStage::Candidate,
        )?
        .max(BigRational::zero());
    let second_gap = meter
        .subtract_rational(
            &realized_half_tangent.upper,
            &half_tangent.lower,
            CayleyStage::Candidate,
        )?
        .max(BigRational::zero());
    let endpoint_error = first_gap.max(second_gap);
    let minimum = half_tangent
        .lower
        .clone()
        .min(realized_half_tangent.lower.clone());
    let minimum_squared = meter.multiply_rational(&minimum, &minimum, CayleyStage::Candidate)?;
    let denominator = meter.add_rational(
        &BigRational::one(),
        &minimum_squared,
        CayleyStage::Candidate,
    )?;
    let twice_error = meter.multiply_rational(
        &BigRational::from_integer(BigInt::from(2_u8)),
        &endpoint_error,
        CayleyStage::Candidate,
    )?;
    let max_error_radians =
        meter.divide_rational(&twice_error, &denominator, CayleyStage::Candidate)?;
    let degree_numerator = meter.multiply_rational(
        &max_error_radians,
        &BigRational::from_integer(BigInt::from(DEGREE_180)),
        CayleyStage::Candidate,
    )?;
    let max_error_degrees =
        meter.divide_rational(&degree_numerator, &pi.lower, CayleyStage::Candidate)?;
    if max_error_degrees >= *acceptance {
        return Err(CayleyError::CertificateUnavailable {
            stage: CayleyStage::Candidate,
        });
    }

    let parameter = if negative {
        -unsigned_parameter
    } else {
        unsigned_parameter
    };
    let rotation = cayley_matrix(direction, delta, &parameter, meter)?;
    let magnitude = exact_f64(original_angle, meter, CayleyStage::Input)?;
    let target_degrees = if negative { -magnitude } else { magnitude };
    let certificate = ExactAngleCertificate::Bounded(Box::new(BoundedAngleCertificate {
        target_degrees,
        precision_bits: precision,
        parameter,
        target_half_tangent: half_tangent,
        realized_half_tangent,
        max_error_radians,
        max_error_degrees,
        acceptance_degrees: acceptance.clone(),
        pi,
    }));
    Ok((rotation, certificate))
}

fn exact_point(values: [f64; 3], meter: &mut WorkMeter<'_>) -> Result<ExactPoint3, CayleyError> {
    Ok(ExactPoint3 {
        coordinates: [
            exact_f64(values[0], meter, CayleyStage::Input)?,
            exact_f64(values[1], meter, CayleyStage::Input)?,
            exact_f64(values[2], meter, CayleyStage::Input)?,
        ],
    })
}

fn exact_f64(
    value: f64,
    meter: &mut WorkMeter<'_>,
    stage: CayleyStage,
) -> Result<BigRational, CayleyError> {
    if !value.is_finite() {
        return Err(CayleyError::NonFiniteInput { stage });
    }
    meter.operation(stage)?;
    let bits = value.to_bits();
    let exponent = ((bits >> 52) & 0x7ff) as i32;
    let fraction = bits & ((1_u64 << 52) - 1);
    if exponent == 0 && fraction == 0 {
        return Ok(BigRational::zero());
    }
    let (significand, binary_exponent) = if exponent == 0 {
        (fraction, -1_074_i32)
    } else {
        (fraction | (1_u64 << 52), exponent - 1_023_i32 - 52_i32)
    };
    let mut numerator = BigInt::from(significand);
    let mut denominator = BigInt::one();
    if binary_exponent >= 0 {
        let shift =
            usize::try_from(binary_exponent).map_err(|_| CayleyError::ResourceLimitExceeded {
                stage,
                resource: "shift_bits",
            })?;
        meter.preflight_shifted_value(stage, bigint_bits(&numerator), shift)?;
        numerator <<= shift;
    } else {
        let shift =
            usize::try_from(-binary_exponent).map_err(|_| CayleyError::ResourceLimitExceeded {
                stage,
                resource: "shift_bits",
            })?;
        meter.preflight_shifted_value(stage, bigint_bits(&denominator), shift)?;
        denominator <<= shift;
    }
    if bits >> 63 != 0 {
        numerator = -numerator;
    }
    let result = BigRational::new(numerator, denominator);
    meter.observe_rational(stage, &result)?;
    Ok(result)
}

fn exact_between(
    start: &ExactPoint3,
    end: &ExactPoint3,
    meter: &mut WorkMeter<'_>,
) -> Result<ExactVector3, CayleyError> {
    let coordinates = try_array3(|index| {
        meter.subtract_rational(
            &end.coordinates[index],
            &start.coordinates[index],
            CayleyStage::Axis,
        )
    })?;
    Ok(ExactVector3 { coordinates })
}

fn normalize_direction(
    raw: &ExactVector3,
    meter: &mut WorkMeter<'_>,
) -> Result<(ExactVector3, BigRational), CayleyError> {
    let scale = raw
        .coordinates
        .iter()
        .map(Signed::abs)
        .max()
        .expect("three coordinates");
    if scale.is_zero() {
        return Err(CayleyError::DegenerateAxis {
            stage: CayleyStage::Axis,
        });
    }
    let coordinates = try_array3(|index| {
        meter.divide_rational(&raw.coordinates[index], &scale, CayleyStage::Axis)
    })?;
    let direction = ExactVector3 { coordinates };
    let mut delta = BigRational::zero();
    for coordinate in &direction.coordinates {
        let square = meter.multiply_rational(coordinate, coordinate, CayleyStage::Axis)?;
        delta = meter.add_rational(&delta, &square, CayleyStage::Axis)?;
    }
    Ok((direction, delta))
}

fn adjacent_angle_acceptance(
    angle: f64,
    meter: &mut WorkMeter<'_>,
) -> Result<BigRational, CayleyError> {
    let magnitude = angle.abs();
    if !(0.0 < magnitude && magnitude < 180.0) {
        return Err(CayleyError::CertificateUnavailable {
            stage: CayleyStage::Candidate,
        });
    }
    let bits = magnitude.to_bits();
    let previous = f64::from_bits(bits - 1);
    let next = f64::from_bits(bits + 1);
    let exact = exact_f64(magnitude, meter, CayleyStage::Candidate)?;
    let previous = exact_f64(previous, meter, CayleyStage::Candidate)?;
    let next = exact_f64(next, meter, CayleyStage::Candidate)?;
    let previous_gap = meter.subtract_rational(&exact, &previous, CayleyStage::Candidate)?;
    let next_gap = meter.subtract_rational(&next, &exact, CayleyStage::Candidate)?;
    let gap = previous_gap.min(next_gap);
    meter.divide_rational(
        &gap,
        &BigRational::from_integer(BigInt::from(4_u8)),
        CayleyStage::Candidate,
    )
}

fn machin_pi_interval(
    precision: usize,
    meter: &mut WorkMeter<'_>,
) -> Result<RationalInterval, CayleyError> {
    if precision == 0 {
        return Err(CayleyError::CertificateUnavailable {
            stage: CayleyStage::Pi,
        });
    }
    let work_precision =
        precision
            .checked_add(DEFAULT_GUARD_BITS)
            .ok_or(CayleyError::ResourceLimitExceeded {
                stage: CayleyStage::Pi,
                resource: "shift_bits",
            })?;
    let tolerance_precision =
        work_precision
            .checked_add(8)
            .ok_or(CayleyError::ResourceLimitExceeded {
                stage: CayleyStage::Pi,
                resource: "shift_bits",
            })?;
    meter.preflight_shifted_value(CayleyStage::Pi, 1, tolerance_precision)?;
    let tolerance = BigRational::new(BigInt::one(), BigInt::one() << tolerance_precision);
    let first = atan_inverse_interval(5, &tolerance, meter)?;
    let second = atan_inverse_interval(239, &tolerance, meter)?;
    let first_lower = meter.multiply_rational(
        &BigRational::from_integer(BigInt::from(16_u8)),
        &first.lower,
        CayleyStage::Pi,
    )?;
    let second_upper = meter.multiply_rational(
        &BigRational::from_integer(BigInt::from(4_u8)),
        &second.upper,
        CayleyStage::Pi,
    )?;
    let lower = meter.subtract_rational(&first_lower, &second_upper, CayleyStage::Pi)?;
    let first_upper = meter.multiply_rational(
        &BigRational::from_integer(BigInt::from(16_u8)),
        &first.upper,
        CayleyStage::Pi,
    )?;
    let second_lower = meter.multiply_rational(
        &BigRational::from_integer(BigInt::from(4_u8)),
        &second.lower,
        CayleyStage::Pi,
    )?;
    let upper = meter.subtract_rational(&first_upper, &second_lower, CayleyStage::Pi)?;
    let exact = RationalInterval::new(lower, upper)?;
    DyadicInterval::from_rational_outward(&exact, work_precision, meter, CayleyStage::Pi)?
        .to_rational(meter, CayleyStage::Pi)
}

fn atan_inverse_interval(
    inverse: usize,
    tolerance: &BigRational,
    meter: &mut WorkMeter<'_>,
) -> Result<RationalInterval, CayleyError> {
    let inverse_squared =
        BigRational::from_integer(BigInt::from(inverse.checked_mul(inverse).ok_or(
            CayleyError::ResourceLimitExceeded {
                stage: CayleyStage::Pi,
                resource: "intermediate_bits",
            },
        )?));
    let mut term = BigRational::new(BigInt::one(), BigInt::from(inverse));
    let mut sum = BigRational::zero();
    let mut index = 0_usize;
    loop {
        let local_terms = index
            .checked_add(1)
            .ok_or(CayleyError::ResourceLimitExceeded {
                stage: CayleyStage::Pi,
                resource: "machin_terms",
            })?;
        meter.machin_term(CayleyStage::Pi, local_terms)?;
        if index.is_multiple_of(2) {
            sum = meter.add_rational(&sum, &term, CayleyStage::Pi)?;
        } else {
            sum = meter.subtract_rational(&sum, &term, CayleyStage::Pi)?;
        }

        let numerator_factor = index
            .checked_mul(2)
            .and_then(|value| value.checked_add(1))
            .ok_or(CayleyError::ResourceLimitExceeded {
                stage: CayleyStage::Pi,
                resource: "machin_terms",
            })?;
        let denominator_factor =
            numerator_factor
                .checked_add(2)
                .ok_or(CayleyError::ResourceLimitExceeded {
                    stage: CayleyStage::Pi,
                    resource: "machin_terms",
                })?;
        let term_numerator = meter.multiply_rational(
            &term,
            &BigRational::from_integer(BigInt::from(numerator_factor)),
            CayleyStage::Pi,
        )?;
        let term_denominator = meter.multiply_rational(
            &inverse_squared,
            &BigRational::from_integer(BigInt::from(denominator_factor)),
            CayleyStage::Pi,
        )?;
        term = meter.divide_rational(&term_numerator, &term_denominator, CayleyStage::Pi)?;
        if term <= *tolerance {
            return if index.is_multiple_of(2) {
                RationalInterval::new(meter.subtract_rational(&sum, &term, CayleyStage::Pi)?, sum)
            } else {
                RationalInterval::new(
                    sum.clone(),
                    meter.add_rational(&sum, &term, CayleyStage::Pi)?,
                )
            };
        }
        index = index
            .checked_add(1)
            .ok_or(CayleyError::ResourceLimitExceeded {
                stage: CayleyStage::Pi,
                resource: "machin_terms",
            })?;
    }
}

fn half_angle_interval(
    degrees: &BigRational,
    pi: &RationalInterval,
    meter: &mut WorkMeter<'_>,
) -> Result<RationalInterval, CayleyError> {
    let denominator = BigRational::from_integer(BigInt::from(DEGREE_360));
    let lower_numerator = meter.multiply_rational(degrees, &pi.lower, CayleyStage::Trigonometry)?;
    let upper_numerator = meter.multiply_rational(degrees, &pi.upper, CayleyStage::Trigonometry)?;
    RationalInterval::new(
        meter.divide_rational(&lower_numerator, &denominator, CayleyStage::Trigonometry)?,
        meter.divide_rational(&upper_numerator, &denominator, CayleyStage::Trigonometry)?,
    )
}

fn tangent_interval(
    half_angle: &RationalInterval,
    precision: usize,
    meter: &mut WorkMeter<'_>,
) -> Result<RationalInterval, CayleyError> {
    let (sin, cos) = sine_cosine_interval(half_angle, precision, meter)?;
    if sin.lower.is_negative() || cos.lower <= BigRational::zero() {
        return Err(CayleyError::CertificateUnavailable {
            stage: CayleyStage::Trigonometry,
        });
    }
    RationalInterval::new(
        meter.divide_rational(&sin.lower, &cos.upper, CayleyStage::Trigonometry)?,
        meter.divide_rational(&sin.upper, &cos.lower, CayleyStage::Trigonometry)?,
    )
}

fn sine_cosine_interval(
    angle: &RationalInterval,
    precision: usize,
    meter: &mut WorkMeter<'_>,
) -> Result<(RationalInterval, RationalInterval), CayleyError> {
    if angle.lower.is_zero() && angle.upper.is_zero() {
        return Ok((
            RationalInterval::point(BigRational::zero()),
            RationalInterval::point(BigRational::one()),
        ));
    }
    let exponent =
        binary_exponent_floor(&angle.upper).ok_or(CayleyError::CertificateUnavailable {
            stage: CayleyStage::Trigonometry,
        })?;
    let extra = if exponent < 0 {
        usize::try_from(-exponent).map_err(|_| CayleyError::ResourceLimitExceeded {
            stage: CayleyStage::Trigonometry,
            resource: "shift_bits",
        })?
    } else {
        0
    };
    let work_precision = precision
        .checked_add(DEFAULT_GUARD_BITS)
        .and_then(|value| value.checked_add(extra))
        .ok_or(CayleyError::ResourceLimitExceeded {
            stage: CayleyStage::Trigonometry,
            resource: "shift_bits",
        })?;
    meter.shift(CayleyStage::Trigonometry, work_precision)?;

    let lower_x = DyadicInterval::from_rational_outward(
        &RationalInterval::point(angle.lower.clone()),
        work_precision,
        meter,
        CayleyStage::Trigonometry,
    )?;
    let upper_x = DyadicInterval::from_rational_outward(
        &RationalInterval::point(angle.upper.clone()),
        work_precision,
        meter,
        CayleyStage::Trigonometry,
    )?;
    let lower_x = DyadicInterval::point(lower_x.lower, work_precision);
    let upper_x = DyadicInterval::point(upper_x.upper, work_precision);
    meter.preflight_shifted_value(CayleyStage::Trigonometry, 1, DEFAULT_GUARD_BITS)?;
    let tolerance_units = BigInt::one() << DEFAULT_GUARD_BITS;
    let sin_lower = taylor_point(TrigKind::Sin, &lower_x, &tolerance_units, meter)?;
    let sin_upper = taylor_point(TrigKind::Sin, &upper_x, &tolerance_units, meter)?;
    let cos_lower = taylor_point(TrigKind::Cos, &upper_x, &tolerance_units, meter)?;
    let cos_upper = taylor_point(TrigKind::Cos, &lower_x, &tolerance_units, meter)?;
    let sin_lower = sin_lower.to_rational(meter, CayleyStage::Trigonometry)?;
    let sin_upper = sin_upper.to_rational(meter, CayleyStage::Trigonometry)?;
    let cos_lower = cos_lower.to_rational(meter, CayleyStage::Trigonometry)?;
    let cos_upper = cos_upper.to_rational(meter, CayleyStage::Trigonometry)?;
    let sin = RationalInterval::new(sin_lower.lower, sin_upper.upper)?;
    let cos = RationalInterval::new(cos_lower.lower, cos_upper.upper)?;
    Ok((sin, cos))
}

#[derive(Debug, Clone, Copy)]
enum TrigKind {
    Sin,
    Cos,
}

fn taylor_point(
    kind: TrigKind,
    x: &DyadicInterval,
    tolerance_units: &BigInt,
    meter: &mut WorkMeter<'_>,
) -> Result<DyadicInterval, CayleyError> {
    if x.lower != x.upper || x.lower.is_negative() {
        return Err(CayleyError::CertificateUnavailable {
            stage: CayleyStage::Trigonometry,
        });
    }
    let x_squared = x.multiply(x, meter, CayleyStage::Trigonometry)?;
    let mut term = match kind {
        TrigKind::Sin => x.clone(),
        TrigKind::Cos => {
            meter.preflight_shifted_value(CayleyStage::Trigonometry, 1, x.precision)?;
            DyadicInterval::point(BigInt::one() << x.precision, x.precision)
        }
    };
    let mut sum = DyadicInterval::zero(x.precision);
    let mut index = 0_usize;
    loop {
        let local_terms = index
            .checked_add(1)
            .ok_or(CayleyError::ResourceLimitExceeded {
                stage: CayleyStage::Trigonometry,
                resource: "trig_terms",
            })?;
        meter.trig_term(CayleyStage::Trigonometry, local_terms)?;
        let positive = index.is_multiple_of(2);
        sum = if positive {
            sum.add(&term, meter, CayleyStage::Trigonometry)?
        } else {
            sum.subtract(&term, meter, CayleyStage::Trigonometry)?
        };
        let (first, second) = match kind {
            TrigKind::Sin => (
                index.checked_mul(2).and_then(|value| value.checked_add(2)),
                index.checked_mul(2).and_then(|value| value.checked_add(3)),
            ),
            TrigKind::Cos => (
                index.checked_mul(2).and_then(|value| value.checked_add(1)),
                index.checked_mul(2).and_then(|value| value.checked_add(2)),
            ),
        };
        let divisor = first
            .and_then(|first| second.and_then(|second| first.checked_mul(second)))
            .ok_or(CayleyError::ResourceLimitExceeded {
                stage: CayleyStage::Trigonometry,
                resource: "trig_terms",
            })?;
        let next = term
            .multiply(&x_squared, meter, CayleyStage::Trigonometry)?
            .divide_positive_integer(divisor, meter, CayleyStage::Trigonometry)?;
        if next.upper <= *tolerance_units {
            meter.operation(CayleyStage::Trigonometry)?;
            meter.preflight_value_bits(
                CayleyStage::Trigonometry,
                bigint_bits(&sum.lower)
                    .max(bigint_bits(&sum.upper))
                    .max(bigint_bits(&next.upper))
                    .saturating_add(1),
            )?;
            return if positive {
                // The next omitted term is negative. For every exact x,
                // S-next <= f(x) <= S. Use a hull rather than interval
                // subtraction: subtracting next.lower from the upper endpoint
                // would describe S-next, not the alternating remainder bound.
                Ok(DyadicInterval {
                    lower: &sum.lower - &next.upper,
                    upper: sum.upper,
                    precision: sum.precision,
                })
            } else {
                // The next omitted term is positive: S <= f(x) <= S+next.
                Ok(DyadicInterval {
                    lower: sum.lower,
                    upper: &sum.upper + &next.upper,
                    precision: sum.precision,
                })
            };
        }
        term = next;
        index = index
            .checked_add(1)
            .ok_or(CayleyError::ResourceLimitExceeded {
                stage: CayleyStage::Trigonometry,
                resource: "trig_terms",
            })?;
    }
}

fn square_root_interval(
    value: &BigRational,
    precision: usize,
    meter: &mut WorkMeter<'_>,
) -> Result<RationalInterval, CayleyError> {
    if value <= &BigRational::zero() {
        return Err(CayleyError::CertificateUnavailable {
            stage: CayleyStage::SquareRoot,
        });
    }
    let shift = precision
        .checked_mul(2)
        .ok_or(CayleyError::ResourceLimitExceeded {
            stage: CayleyStage::SquareRoot,
            resource: "shift_bits",
        })?;
    meter.shift(CayleyStage::SquareRoot, shift)?;
    let numerator = positive_biguint(value.numer(), CayleyStage::SquareRoot)?;
    let denominator = positive_biguint(value.denom(), CayleyStage::SquareRoot)?;
    if bigint_bits(value.numer())
        .checked_add(shift)
        .is_none_or(|bits| bits > meter.limits.max_intermediate_bits)
    {
        return Err(CayleyError::ResourceLimitExceeded {
            stage: CayleyStage::SquareRoot,
            resource: "intermediate_bits",
        });
    }
    meter.preflight_shifted_value(CayleyStage::SquareRoot, numerator.bits() as usize, shift)?;
    let scaled_numerator = numerator << shift;
    let quotient = &scaled_numerator / &denominator;
    let root = integer_sqrt_floor(&quotient, meter)?;
    let lower = BigRational::new(
        BigInt::from_biguint(Sign::Plus, root.clone()),
        BigInt::one() << precision,
    );
    meter.preflight_product_bits(
        CayleyStage::SquareRoot,
        root.bits() as usize,
        root.bits() as usize,
    )?;
    let root_square = &root * &root;
    meter.preflight_product_bits(
        CayleyStage::SquareRoot,
        root_square.bits() as usize,
        denominator.bits() as usize,
    )?;
    let exact = root_square * &denominator == scaled_numerator;
    if !exact {
        meter.preflight_value_bits(
            CayleyStage::SquareRoot,
            (root.bits() as usize).saturating_add(1),
        )?;
    }
    let upper_root = if exact { root } else { root + BigUint::one() };
    let upper = BigRational::new(
        BigInt::from_biguint(Sign::Plus, upper_root),
        BigInt::one() << precision,
    );
    RationalInterval::new(lower, upper)
}

fn exact_rational_square_root(
    value: &BigRational,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<BigRational>, CayleyError> {
    let numerator = positive_biguint(value.numer(), CayleyStage::SquareRoot)?;
    let denominator = positive_biguint(value.denom(), CayleyStage::SquareRoot)?;
    let numerator_root = integer_sqrt_floor(&numerator, meter)?;
    meter.preflight_product_bits(
        CayleyStage::SquareRoot,
        numerator_root.bits() as usize,
        numerator_root.bits() as usize,
    )?;
    if &numerator_root * &numerator_root != numerator {
        return Ok(None);
    }
    let denominator_root = integer_sqrt_floor(&denominator, meter)?;
    meter.preflight_product_bits(
        CayleyStage::SquareRoot,
        denominator_root.bits() as usize,
        denominator_root.bits() as usize,
    )?;
    if &denominator_root * &denominator_root != denominator {
        return Ok(None);
    }
    Ok(Some(BigRational::new(
        BigInt::from_biguint(Sign::Plus, numerator_root),
        BigInt::from_biguint(Sign::Plus, denominator_root),
    )))
}

fn integer_sqrt_floor(value: &BigUint, meter: &mut WorkMeter<'_>) -> Result<BigUint, CayleyError> {
    if value.is_zero() {
        return Ok(BigUint::zero());
    }
    let bit_length = value.bits() as usize;
    let theoretical_refinements = bit_length
        .checked_ilog2()
        .map_or(2_usize, |log| log as usize + 3);
    meter.work.max_sqrt_call_refinements = meter
        .work
        .max_sqrt_call_refinements
        .max(theoretical_refinements);
    if theoretical_refinements > meter.limits.max_sqrt_refinements {
        return Err(CayleyError::ResourceLimitExceeded {
            stage: CayleyStage::SquareRoot,
            resource: "sqrt_refinements",
        });
    }
    let initial_shift = bit_length.div_ceil(2);
    meter.preflight_shifted_value(CayleyStage::SquareRoot, 1, initial_shift)?;
    let mut root = BigUint::one() << initial_shift;
    let mut local_refinements = 0_usize;
    loop {
        local_refinements =
            local_refinements
                .checked_add(1)
                .ok_or(CayleyError::ResourceLimitExceeded {
                    stage: CayleyStage::SquareRoot,
                    resource: "sqrt_refinements",
                })?;
        meter.sqrt_refinement(CayleyStage::SquareRoot, local_refinements)?;
        meter.preflight_value_bits(
            CayleyStage::SquareRoot,
            (root.bits() as usize)
                .max(value.bits() as usize)
                .saturating_add(1),
        )?;
        let next = (&root + value / &root) >> 1_usize;
        if next >= root {
            meter.preflight_product_bits(
                CayleyStage::SquareRoot,
                root.bits() as usize,
                root.bits() as usize,
            )?;
            let square = &root * &root;
            meter.preflight_value_bits(
                CayleyStage::SquareRoot,
                (root.bits() as usize).saturating_add(1),
            )?;
            let successor = &root + BigUint::one();
            meter.preflight_product_bits(
                CayleyStage::SquareRoot,
                successor.bits() as usize,
                successor.bits() as usize,
            )?;
            let successor_square = &successor * &successor;
            if square > *value || successor_square <= *value {
                return Err(CayleyError::InvariantFailure {
                    stage: CayleyStage::SquareRoot,
                });
            }
            return Ok(root);
        }
        root = next;
    }
}

fn round_interval_midpoint_to_dyadic(
    interval: &RationalInterval,
    significant_bits: usize,
    meter: &mut WorkMeter<'_>,
) -> Result<BigRational, CayleyError> {
    if significant_bits == 0 || significant_bits > meter.limits.max_candidate_bits {
        return Err(CayleyError::ResourceLimitExceeded {
            stage: CayleyStage::Candidate,
            resource: "candidate_bits",
        });
    }
    meter.operation(CayleyStage::Candidate)?;
    let midpoint_sum =
        meter.add_rational(&interval.lower, &interval.upper, CayleyStage::Candidate)?;
    let midpoint = meter.divide_rational(
        &midpoint_sum,
        &BigRational::from_integer(BigInt::from(2_u8)),
        CayleyStage::Candidate,
    )?;
    if midpoint <= BigRational::zero() {
        return Err(CayleyError::CertificateUnavailable {
            stage: CayleyStage::Candidate,
        });
    }
    let exponent = binary_exponent_floor(&midpoint).ok_or(CayleyError::CertificateUnavailable {
        stage: CayleyStage::Candidate,
    })?;
    let significant_bits_i64 =
        i64::try_from(significant_bits).map_err(|_| CayleyError::ResourceLimitExceeded {
            stage: CayleyStage::Candidate,
            resource: "candidate_bits",
        })?;
    let step_exponent = exponent.checked_sub(significant_bits_i64 - 1).ok_or(
        CayleyError::ResourceLimitExceeded {
            stage: CayleyStage::Candidate,
            resource: "shift_bits",
        },
    )?;
    let scaled = if step_exponent >= 0 {
        let shift =
            usize::try_from(step_exponent).map_err(|_| CayleyError::ResourceLimitExceeded {
                stage: CayleyStage::Candidate,
                resource: "shift_bits",
            })?;
        meter.preflight_shifted_value(CayleyStage::Candidate, 1, shift)?;
        meter.divide_rational(
            &midpoint,
            &BigRational::from_integer(BigInt::one() << shift),
            CayleyStage::Candidate,
        )?
    } else {
        let shift =
            usize::try_from(-step_exponent).map_err(|_| CayleyError::ResourceLimitExceeded {
                stage: CayleyStage::Candidate,
                resource: "shift_bits",
            })?;
        meter.preflight_shifted_value(CayleyStage::Candidate, 1, shift)?;
        meter.multiply_rational(
            &midpoint,
            &BigRational::from_integer(BigInt::one() << shift),
            CayleyStage::Candidate,
        )?
    };
    let rounded = round_rational_nearest_even(&scaled, meter)?;
    let result = if step_exponent >= 0 {
        let shift =
            usize::try_from(step_exponent).map_err(|_| CayleyError::ResourceLimitExceeded {
                stage: CayleyStage::Candidate,
                resource: "shift_bits",
            })?;
        meter.preflight_shifted_value(CayleyStage::Candidate, bigint_bits(&rounded), shift)?;
        BigRational::from_integer(rounded << shift)
    } else {
        let shift =
            usize::try_from(-step_exponent).map_err(|_| CayleyError::ResourceLimitExceeded {
                stage: CayleyStage::Candidate,
                resource: "shift_bits",
            })?;
        meter.preflight_shifted_value(CayleyStage::Candidate, 1, shift)?;
        BigRational::new(rounded, BigInt::one() << shift)
    };
    meter.observe_rational(CayleyStage::Candidate, &result)?;
    Ok(result)
}

fn round_rational_nearest_even(
    value: &BigRational,
    meter: &mut WorkMeter<'_>,
) -> Result<BigInt, CayleyError> {
    debug_assert!(!value.is_negative());
    meter.operation(CayleyStage::Candidate)?;
    let quotient = value.numer() / value.denom();
    let remainder = value.numer() % value.denom();
    meter.preflight_shifted_value(CayleyStage::Candidate, bigint_bits(&remainder), 1)?;
    meter.preflight_value_bits(
        CayleyStage::Candidate,
        bigint_bits(&quotient).saturating_add(1),
    )?;
    let rounded = match (&remainder << 1_usize).cmp(value.denom()) {
        Ordering::Less => quotient,
        Ordering::Greater => quotient + 1,
        Ordering::Equal => {
            if (&quotient & BigInt::one()).is_zero() {
                quotient
            } else {
                quotient + 1
            }
        }
    };
    meter.preflight_value_bits(CayleyStage::Candidate, bigint_bits(&rounded))?;
    Ok(rounded)
}

fn half_turn(
    direction: &ExactVector3,
    delta: &BigRational,
    meter: &mut WorkMeter<'_>,
) -> Result<[[BigRational; 3]; 3], CayleyError> {
    let two = BigRational::from_integer(BigInt::from(2_u8));
    try_array3(|row| {
        try_array3(|column| {
            let identity = if row == column {
                -BigRational::one()
            } else {
                BigRational::zero()
            };
            let twice_row =
                meter.multiply_rational(&two, &direction.coordinates[row], CayleyStage::Matrix)?;
            let outer = meter.multiply_rational(
                &twice_row,
                &direction.coordinates[column],
                CayleyStage::Matrix,
            )?;
            let quotient = meter.divide_rational(&outer, delta, CayleyStage::Matrix)?;
            meter.add_rational(&identity, &quotient, CayleyStage::Matrix)
        })
    })
}

fn cayley_matrix(
    direction: &ExactVector3,
    delta: &BigRational,
    parameter: &BigRational,
    meter: &mut WorkMeter<'_>,
) -> Result<[[BigRational; 3]; 3], CayleyError> {
    // This is exactly
    // ((1-a)I + 2s² ddᵀ + 2s K(d)) / (1+a),
    // where K(d)x=d×x and a=s²(d·d).  No trigonometric value participates
    // in the matrix proof; the interval certificate only justifies how close
    // this exact rational rotation is to the requested binary64 angle.
    let parameter_squared = meter.multiply_rational(parameter, parameter, CayleyStage::Matrix)?;
    let a = meter.multiply_rational(&parameter_squared, delta, CayleyStage::Matrix)?;
    let denominator = meter.add_rational(&BigRational::one(), &a, CayleyStage::Matrix)?;
    let two = BigRational::from_integer(BigInt::from(2_u8));
    let cross = cross_matrix(direction);
    try_array3(|row| {
        try_array3(|column| {
            let diagonal = if row == column {
                meter.subtract_rational(&BigRational::one(), &a, CayleyStage::Matrix)?
            } else {
                BigRational::zero()
            };
            let twice_parameter_squared =
                meter.multiply_rational(&two, &parameter_squared, CayleyStage::Matrix)?;
            let outer_row = meter.multiply_rational(
                &twice_parameter_squared,
                &direction.coordinates[row],
                CayleyStage::Matrix,
            )?;
            let outer = meter.multiply_rational(
                &outer_row,
                &direction.coordinates[column],
                CayleyStage::Matrix,
            )?;
            let twice_parameter = meter.multiply_rational(&two, parameter, CayleyStage::Matrix)?;
            let cross_term = meter.multiply_rational(
                &twice_parameter,
                &cross[row][column],
                CayleyStage::Matrix,
            )?;
            let diagonal_and_outer = meter.add_rational(&diagonal, &outer, CayleyStage::Matrix)?;
            let numerator =
                meter.add_rational(&diagonal_and_outer, &cross_term, CayleyStage::Matrix)?;
            meter.divide_rational(&numerator, &denominator, CayleyStage::Matrix)
        })
    })
}

fn cross_matrix(direction: &ExactVector3) -> [[BigRational; 3]; 3] {
    let [x, y, z] = &direction.coordinates;
    [
        [BigRational::zero(), -z, y.clone()],
        [z.clone(), BigRational::zero(), -x],
        [-y, x.clone(), BigRational::zero()],
    ]
}

fn fixed_point_translation(
    rotation: &[[BigRational; 3]; 3],
    pivot: &ExactPoint3,
    meter: &mut WorkMeter<'_>,
) -> Result<ExactVector3, CayleyError> {
    let coordinates = try_array3(|row| {
        let mut transformed = BigRational::zero();
        for (coefficient, coordinate) in rotation[row].iter().zip(&pivot.coordinates) {
            let product = meter.multiply_rational(coefficient, coordinate, CayleyStage::Matrix)?;
            transformed = meter.add_rational(&transformed, &product, CayleyStage::Matrix)?;
        }
        meter.subtract_rational(&pivot.coordinates[row], &transformed, CayleyStage::Matrix)
    })?;
    Ok(ExactVector3 { coordinates })
}

fn verify_invariants(
    rotation: &[[BigRational; 3]; 3],
    translation: &ExactVector3,
    pivot: &ExactPoint3,
    end: &ExactPoint3,
    direction: &ExactVector3,
    meter: &mut WorkMeter<'_>,
) -> Result<(), CayleyError> {
    for row in 0..3 {
        for column in 0..3 {
            let mut value = BigRational::zero();
            for rotation_row in rotation {
                let product = meter.multiply_rational(
                    &rotation_row[row],
                    &rotation_row[column],
                    CayleyStage::Matrix,
                )?;
                value = meter.add_rational(&value, &product, CayleyStage::Matrix)?;
            }
            let expected = if row == column {
                BigRational::one()
            } else {
                BigRational::zero()
            };
            if value != expected {
                return Err(CayleyError::InvariantFailure {
                    stage: CayleyStage::Matrix,
                });
            }
        }
    }
    if determinant(rotation, meter)? != BigRational::one() {
        return Err(CayleyError::InvariantFailure {
            stage: CayleyStage::Matrix,
        });
    }
    let rotated_direction = apply_vector(rotation, direction, meter)?;
    if rotated_direction != *direction {
        return Err(CayleyError::InvariantFailure {
            stage: CayleyStage::Matrix,
        });
    }
    if apply_point(rotation, translation, pivot, meter)? != *pivot
        || apply_point(rotation, translation, end, meter)? != *end
    {
        return Err(CayleyError::InvariantFailure {
            stage: CayleyStage::Matrix,
        });
    }
    Ok(())
}

fn observe_transform_output(
    rotation: &[[BigRational; 3]; 3],
    translation: &ExactVector3,
    meter: &mut WorkMeter<'_>,
) -> Result<(), CayleyError> {
    for value in rotation
        .iter()
        .flat_map(|row| row.iter())
        .chain(translation.coordinates.iter())
    {
        meter.observe_output(value)?;
    }
    Ok(())
}

fn apply_vector(
    rotation: &[[BigRational; 3]; 3],
    vector: &ExactVector3,
    meter: &mut WorkMeter<'_>,
) -> Result<ExactVector3, CayleyError> {
    let coordinates = try_array3(|row| {
        let mut result = BigRational::zero();
        for (coefficient, coordinate) in rotation[row].iter().zip(&vector.coordinates) {
            let product = meter.multiply_rational(coefficient, coordinate, CayleyStage::Matrix)?;
            result = meter.add_rational(&result, &product, CayleyStage::Matrix)?;
        }
        Ok(result)
    })?;
    Ok(ExactVector3 { coordinates })
}

fn apply_point(
    rotation: &[[BigRational; 3]; 3],
    translation: &ExactVector3,
    point: &ExactPoint3,
    meter: &mut WorkMeter<'_>,
) -> Result<ExactPoint3, CayleyError> {
    apply_point_at_stage(rotation, translation, point, meter, CayleyStage::Matrix)
}

fn apply_point_at_stage(
    rotation: &[[BigRational; 3]; 3],
    translation: &ExactVector3,
    point: &ExactPoint3,
    meter: &mut WorkMeter<'_>,
    stage: CayleyStage,
) -> Result<ExactPoint3, CayleyError> {
    let coordinates = try_array3(|row| {
        let mut result = translation.coordinates[row].clone();
        for (coefficient, coordinate) in rotation[row].iter().zip(&point.coordinates) {
            let product = meter.multiply_rational(coefficient, coordinate, stage)?;
            result = meter.add_rational(&result, &product, stage)?;
        }
        Ok(result)
    })?;
    Ok(ExactPoint3 { coordinates })
}

fn determinant(
    matrix: &[[BigRational; 3]; 3],
    meter: &mut WorkMeter<'_>,
) -> Result<BigRational, CayleyError> {
    let minor_00_left =
        meter.multiply_rational(&matrix[1][1], &matrix[2][2], CayleyStage::Matrix)?;
    let minor_00_right =
        meter.multiply_rational(&matrix[1][2], &matrix[2][1], CayleyStage::Matrix)?;
    let minor_00 = meter.subtract_rational(&minor_00_left, &minor_00_right, CayleyStage::Matrix)?;
    let term_00 = meter.multiply_rational(&matrix[0][0], &minor_00, CayleyStage::Matrix)?;

    let minor_01_left =
        meter.multiply_rational(&matrix[1][0], &matrix[2][2], CayleyStage::Matrix)?;
    let minor_01_right =
        meter.multiply_rational(&matrix[1][2], &matrix[2][0], CayleyStage::Matrix)?;
    let minor_01 = meter.subtract_rational(&minor_01_left, &minor_01_right, CayleyStage::Matrix)?;
    let term_01 = meter.multiply_rational(&matrix[0][1], &minor_01, CayleyStage::Matrix)?;

    let minor_02_left =
        meter.multiply_rational(&matrix[1][0], &matrix[2][1], CayleyStage::Matrix)?;
    let minor_02_right =
        meter.multiply_rational(&matrix[1][1], &matrix[2][0], CayleyStage::Matrix)?;
    let minor_02 = meter.subtract_rational(&minor_02_left, &minor_02_right, CayleyStage::Matrix)?;
    let term_02 = meter.multiply_rational(&matrix[0][2], &minor_02, CayleyStage::Matrix)?;

    let first_two = meter.subtract_rational(&term_00, &term_01, CayleyStage::Matrix)?;
    meter.add_rational(&first_two, &term_02, CayleyStage::Matrix)
}

fn identity_matrix() -> [[BigRational; 3]; 3] {
    std::array::from_fn(|row| {
        std::array::from_fn(|column| {
            if row == column {
                BigRational::one()
            } else {
                BigRational::zero()
            }
        })
    })
}

fn zero_vector() -> ExactVector3 {
    ExactVector3 {
        coordinates: std::array::from_fn(|_| BigRational::zero()),
    }
}

fn binary_exponent_floor(value: &BigRational) -> Option<i64> {
    if value <= &BigRational::zero() {
        return None;
    }
    let numerator = value.numer().magnitude();
    let denominator = value.denom().magnitude();
    let mut exponent =
        i64::try_from(numerator.bits()).ok()? - i64::try_from(denominator.bits()).ok()?;
    let below = if exponent >= 0 {
        numerator < &(denominator << usize::try_from(exponent).ok()?)
    } else {
        &(numerator << usize::try_from(-exponent).ok()?) < denominator
    };
    if below {
        exponent -= 1;
    }
    Some(exponent)
}

fn positive_biguint(value: &BigInt, stage: CayleyStage) -> Result<BigUint, CayleyError> {
    value
        .to_biguint()
        .filter(|value| !value.is_zero())
        .ok_or(CayleyError::CertificateUnavailable { stage })
}

fn div_floor(numerator: &BigInt, denominator: &BigInt) -> BigInt {
    debug_assert!(denominator.is_positive());
    let quotient = numerator / denominator;
    let remainder = numerator % denominator;
    if numerator.is_negative() && !remainder.is_zero() {
        quotient - 1
    } else {
        quotient
    }
}

fn div_ceil(numerator: &BigInt, denominator: &BigInt) -> BigInt {
    debug_assert!(denominator.is_positive());
    let quotient = numerator / denominator;
    let remainder = numerator % denominator;
    if numerator.is_positive() && !remainder.is_zero() {
        quotient + 1
    } else {
        quotient
    }
}

fn bigint_bits(value: &BigInt) -> usize {
    value.bits() as usize
}

fn rational_storage_bits(value: &BigRational, stage: CayleyStage) -> Result<usize, CayleyError> {
    bigint_bits(value.numer())
        .checked_add(bigint_bits(value.denom()))
        .ok_or(CayleyError::ResourceLimitExceeded {
            stage,
            resource: "rational_allocation_bits",
        })
}

fn checked_work_sum(
    current: usize,
    additional: usize,
    stage: CayleyStage,
    resource: &'static str,
) -> Result<usize, CayleyError> {
    current
        .checked_add(additional)
        .ok_or(CayleyError::ResourceLimitExceeded { stage, resource })
}

fn refined_product_bits(
    left: &BigInt,
    right: &BigInt,
    stage: CayleyStage,
) -> Result<usize, CayleyError> {
    product_bits_upper_bound(bigint_bits(left), bigint_bits(right), stage)
}

fn product_bits_upper_bound(
    left_bits: usize,
    right_bits: usize,
    stage: CayleyStage,
) -> Result<usize, CayleyError> {
    if left_bits == 0 || right_bits == 0 {
        return Ok(0);
    }
    if left_bits == 1 {
        return Ok(right_bits);
    }
    if right_bits == 1 {
        return Ok(left_bits);
    }
    left_bits
        .checked_add(right_bits)
        .ok_or(CayleyError::ResourceLimitExceeded {
            stage,
            resource: "intermediate_bits",
        })
}

fn quotient_bits_upper_bound(
    dividend: &BigInt,
    divisor: &BigInt,
    stage: CayleyStage,
) -> Result<usize, CayleyError> {
    let dividend_bits = bigint_bits(dividend);
    let divisor_bits = bigint_bits(divisor);
    if divisor_bits == 0 {
        return Err(CayleyError::InvariantFailure { stage });
    }
    if dividend_bits == 0 {
        return Ok(0);
    }
    dividend_bits
        .checked_sub(divisor_bits)
        .and_then(|bits| bits.checked_add(1))
        .ok_or(CayleyError::InvariantFailure { stage })
}

fn rational_bits(value: &BigRational) -> usize {
    bigint_bits(value.numer()).max(bigint_bits(value.denom()))
}

fn try_array3<T>(
    mut element: impl FnMut(usize) -> Result<T, CayleyError>,
) -> Result<[T; 3], CayleyError> {
    Ok([element(0)?, element(1)?, element(2)?])
}

mod containment;

#[cfg(test)]
mod stress_tests;

#[cfg(test)]
mod tests {
    use ori_domain::{CreasePattern, Edge, EdgeKind, Paper, Point2, ProjectId, Vertex};
    use ori_kinematics::{
        CanonicalHingeAngles, HingeAngle, MaterialTreeKinematicsModel, MaterialTreePose,
        TreeKinematicsLimits,
    };
    use ori_topology::{FaceExtractionInput, analyze_faces};

    use super::*;

    fn limits() -> CayleyLimits {
        CayleyLimits::default()
    }

    fn rotate(axis_end: [f64; 3], degrees: f64) -> ExactLocalRotation {
        let sign = if degrees.is_sign_negative() { -1 } else { 1 };
        local_rotation_v1([0.0, 0.0, 0.0], axis_end, degrees.abs(), sign, limits())
            .expect("certified rotation")
    }

    fn rational(integer: i64) -> BigRational {
        BigRational::from_integer(BigInt::from(integer))
    }

    fn transpose(matrix: &[[BigRational; 3]; 3]) -> [[BigRational; 3]; 3] {
        std::array::from_fn(|row| std::array::from_fn(|column| matrix[column][row].clone()))
    }

    fn tree_vertex_id(index: u64) -> VertexId {
        serde_json::from_str(&format!("\"00000000-0000-4000-8000-{index:012x}\""))
            .expect("fixed vertex id")
    }

    fn tree_edge_id(index: u64) -> EdgeId {
        serde_json::from_str(&format!("\"00000000-0000-4000-9000-{index:012x}\""))
            .expect("fixed edge id")
    }

    fn tree_project_id() -> ProjectId {
        serde_json::from_str("\"00000000-0000-4000-b000-0000000000c1\"").expect("fixed project id")
    }

    fn tree_vertex(index: u64, x: f64, y: f64) -> Vertex {
        Vertex {
            id: tree_vertex_id(index),
            position: Point2::new(x, y),
        }
    }

    fn tree_edge(index: u64, start: VertexId, end: VertexId, kind: EdgeKind) -> Edge {
        Edge {
            id: tree_edge_id(index),
            start,
            end,
            kind,
        }
    }

    /// Three material faces joined by two diagonal hinges that meet at one
    /// paper corner. This exercises both shared-edge watertightness and the
    /// non-edge shared corner of the two outer faces.
    fn diagonal_v_tree_model(reordered: bool) -> MaterialTreeKinematicsModel {
        let mut vertices = vec![
            tree_vertex(1, 0.0, 0.0),
            tree_vertex(2, 10.0, 0.0),
            tree_vertex(3, 10.0, 5.0),
            tree_vertex(4, 10.0, 10.0),
            tree_vertex(5, 5.0, 10.0),
            tree_vertex(6, 0.0, 10.0),
        ];
        let mut boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
        let mut edges = (0..boundary.len())
            .map(|index| {
                tree_edge(
                    index as u64 + 1,
                    boundary[index],
                    boundary[(index + 1) % boundary.len()],
                    EdgeKind::Boundary,
                )
            })
            .collect::<Vec<_>>();
        edges.push(tree_edge(7, boundary[0], boundary[2], EdgeKind::Mountain));
        edges.push(tree_edge(8, boundary[0], boundary[4], EdgeKind::Valley));
        if reordered {
            vertices.reverse();
            edges.reverse();
            boundary.rotate_left(3);
        }
        let pattern = CreasePattern { vertices, edges };
        let paper = Paper {
            boundary_vertices: boundary,
            ..Paper::default()
        };
        let report = analyze_faces(FaceExtractionInput {
            identity_namespace: tree_project_id(),
            source_revision: 193,
            paper: &paper,
            pattern: &pattern,
        });
        assert!(report.issues.is_empty(), "{:?}", report.issues);
        MaterialTreeKinematicsModel::prepare(
            &pattern,
            &paper,
            &report.snapshot.expect("diagonal V topology"),
            TreeKinematicsLimits::default(),
        )
        .expect("diagonal V material model")
    }

    fn single_face_tree_model() -> MaterialTreeKinematicsModel {
        let vertices = vec![
            tree_vertex(21, 0.0, 0.0),
            tree_vertex(22, 8.0, 0.0),
            tree_vertex(23, 8.0, 6.0),
            tree_vertex(24, 0.0, 6.0),
        ];
        let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
        let edges = (0..boundary.len())
            .map(|index| {
                tree_edge(
                    index as u64 + 21,
                    boundary[index],
                    boundary[(index + 1) % boundary.len()],
                    EdgeKind::Boundary,
                )
            })
            .collect();
        let pattern = CreasePattern { vertices, edges };
        let paper = Paper {
            boundary_vertices: boundary,
            ..Paper::default()
        };
        let report = analyze_faces(FaceExtractionInput {
            identity_namespace: tree_project_id(),
            source_revision: 194,
            paper: &paper,
            pattern: &pattern,
        });
        assert!(report.issues.is_empty(), "{:?}", report.issues);
        MaterialTreeKinematicsModel::prepare(
            &pattern,
            &paper,
            &report.snapshot.expect("single face topology"),
            TreeKinematicsLimits::default(),
        )
        .expect("single face material model")
    }

    fn diagonal_v_angles(model: &MaterialTreeKinematicsModel) -> CanonicalHingeAngles {
        CanonicalHingeAngles::new(
            model
                .hinges()
                .iter()
                .map(|hinge| {
                    let angle = if hinge.edge() == tree_edge_id(7) {
                        37.25
                    } else {
                        128.5
                    };
                    HingeAngle::new(hinge.edge(), angle).unwrap()
                })
                .collect(),
        )
        .expect("canonical V angles")
    }

    fn solve_diagonal_v(model: &MaterialTreeKinematicsModel, root: FaceId) -> MaterialTreePose {
        model
            .solve(Some(root), &diagonal_v_angles(model))
            .expect("diagonal V pose")
    }

    fn exact_tree_pose<'a>(
        model: &'a MaterialTreeKinematicsModel,
        pose: &'a MaterialTreePose,
        limits: ExactTreePoseLimits,
    ) -> RationalCayleyTreePose<'a> {
        prepare_rational_cayley_tree_pose_v1(
            model.bind_pose(pose).expect("issuer-bound pose"),
            limits,
        )
        .expect("watertight exact tree pose")
    }

    fn exact_face<'a>(pose: &'a RationalCayleyTreePose<'_>, face: FaceId) -> &'a ExactFacePose {
        pose.faces
            .binary_search_by_key(&face.canonical_bytes(), |candidate| {
                candidate.face.canonical_bytes()
            })
            .ok()
            .and_then(|index| pose.faces.get(index))
            .expect("exact face")
    }

    fn inverse_exact_transform(
        transform: &ExactRigidTransform,
        meter: &mut WorkMeter<'_>,
    ) -> ExactRigidTransform {
        let rotation = std::array::from_fn(|row| {
            std::array::from_fn(|column| transform.rotation[column][row].clone())
        });
        let translation = ExactVector3 {
            coordinates: try_array3(|row| {
                let mut value = BigRational::zero();
                for (coefficient, coordinate) in
                    rotation[row].iter().zip(&transform.translation.coordinates)
                {
                    let product =
                        meter.multiply_rational(coefficient, coordinate, CayleyStage::Tree)?;
                    value = meter.add_rational(&value, &product, CayleyStage::Tree)?;
                }
                Ok(-value)
            })
            .unwrap(),
        };
        ExactRigidTransform {
            rotation,
            translation,
        }
    }

    #[test]
    fn issuer_bound_single_face_tree_is_exact_identity_without_hinges() {
        let model = single_face_tree_model();
        let angles = CanonicalHingeAngles::new(Vec::new()).unwrap();
        let pose = model.solve(None, &angles).unwrap();
        let exact = exact_tree_pose(&model, &pose, ExactTreePoseLimits::default());
        assert_eq!(exact.fixed_face, None);
        assert_eq!(exact.faces.len(), 1);
        assert!(exact.hinges.is_empty());
        assert_eq!(exact.faces[0].transform, exact_identity_transform());
        assert_eq!(exact.work.faces, 1);
        assert_eq!(exact.work.hinges, 0);
        assert_eq!(exact.work.exact.machin_terms, 0);
        assert_eq!(exact.work.exact.trig_terms, 0);
        assert_eq!(exact.work.exact.sqrt_refinements, 0);
    }

    #[test]
    fn issuer_bound_diagonal_v_tree_is_watertight_and_rejects_aba() {
        let model = diagonal_v_tree_model(false);
        let root = model.face_ids()[1];
        let pose = solve_diagonal_v(&model, root);
        let bound = model.bind_pose(&pose).unwrap();
        let exact =
            prepare_rational_cayley_tree_pose_v1(bound, ExactTreePoseLimits::default()).unwrap();

        assert!(exact.is_for(bound));
        assert_eq!(exact.version, RATIONAL_CAYLEY_TREE_POSE_V1);
        assert_eq!(exact.fixed_face, Some(root));
        assert_eq!(exact.faces.len(), model.face_ids().len());
        assert_eq!(exact.hinges.len(), model.hinges().len());
        assert_eq!(exact.work.faces, 3);
        assert_eq!(exact.work.hinges, 2);
        assert_eq!(exact.work.adjacency_entries, 4);
        assert_eq!(
            exact.work.boundary_edge_index_entries,
            exact.work.boundary_occurrences
        );
        assert_eq!(
            exact.work.boundary_edge_index_operations,
            exact.work.boundary_occurrences
        );
        assert!(exact.work.max_output_bits >= exact.work.exact.max_output_bits);
        assert_eq!(
            exact_face(&exact, root).transform,
            exact_identity_transform()
        );

        let corner = tree_vertex_id(1);
        let corner_images = exact
            .faces
            .iter()
            .flat_map(|face| &face.boundary)
            .filter(|(vertex, _)| *vertex == corner)
            .map(|(_, point)| point)
            .collect::<Vec<_>>();
        assert_eq!(corner_images.len(), 3);
        assert!(corner_images.windows(2).all(|pair| pair[0] == pair[1]));

        let limits = CayleyLimits {
            max_interval_operations: 1_000_000,
            ..CayleyLimits::default()
        };
        let mut meter = WorkMeter::new(&limits);
        for hinge_pose in &exact.hinges {
            let hinge = model
                .hinges()
                .iter()
                .find(|hinge| hinge.edge() == hinge_pose.edge)
                .unwrap();
            let base = match hinge.assignment() {
                FoldAssignment::Mountain => 1,
                FoldAssignment::Valley => -1,
            };
            let expected_sign = if hinge_pose.parent == hinge.left_face() {
                base
            } else {
                -base
            };
            assert_eq!(hinge_pose.rotation_sign, expected_sign);
            assert_eq!(
                pose.hinge_parent_transform(hinge_pose.edge),
                pose.face_transform(hinge_pose.parent)
            );

            let start = exact_point(point3_array(hinge.start()), &mut meter).unwrap();
            let end = exact_point(point3_array(hinge.end()), &mut meter).unwrap();
            let midpoint = ExactPoint3 {
                coordinates: std::array::from_fn(|index| {
                    (&start.coordinates[index] + &end.coordinates[index]) / rational(2)
                }),
            };
            let parent_midpoint = apply_exact_transform(
                &exact_face(&exact, hinge_pose.parent).transform,
                &midpoint,
                &mut meter,
            )
            .unwrap();
            let child_midpoint = apply_exact_transform(
                &exact_face(&exact, hinge_pose.child).transform,
                &midpoint,
                &mut meter,
            )
            .unwrap();
            assert_eq!(parent_midpoint, child_midpoint);
        }

        let cloned_pose = pose.clone();
        assert!(exact.is_for(model.bind_pose(&cloned_pose).unwrap()));
        let repeated = solve_diagonal_v(&model, root);
        assert!(!exact.is_for(model.bind_pose(&repeated).unwrap()));
        let independent = diagonal_v_tree_model(false);
        let independent_pose = solve_diagonal_v(&independent, root);
        assert!(!exact.is_for(independent.bind_pose(&independent_pose).unwrap()));
    }

    #[test]
    fn exact_tree_rerooting_is_one_global_exact_frame_change() {
        let model = diagonal_v_tree_model(false);
        let first_root = model.face_ids()[0];
        let second_root = *model.face_ids().last().unwrap();
        let first_pose = solve_diagonal_v(&model, first_root);
        let second_pose = solve_diagonal_v(&model, second_root);
        let first = exact_tree_pose(&model, &first_pose, ExactTreePoseLimits::default());
        let second = exact_tree_pose(&model, &second_pose, ExactTreePoseLimits::default());
        let limits = CayleyLimits {
            max_interval_operations: 1_000_000,
            ..CayleyLimits::default()
        };
        let mut meter = WorkMeter::new(&limits);
        let frame_change =
            inverse_exact_transform(&exact_face(&first, second_root).transform, &mut meter);
        for face in model.face_ids() {
            let normalized = compose_exact_transform(
                &frame_change,
                &exact_face(&first, *face).transform,
                &mut meter,
            )
            .unwrap();
            assert_eq!(normalized, exact_face(&second, *face).transform);
        }
    }

    #[test]
    fn exact_tree_pose_is_invariant_to_source_collection_order() {
        let first_model = diagonal_v_tree_model(false);
        let reordered_model = diagonal_v_tree_model(true);
        assert_eq!(first_model.face_ids(), reordered_model.face_ids());
        let root = first_model.face_ids()[0];
        let first_pose = solve_diagonal_v(&first_model, root);
        let reordered_pose = solve_diagonal_v(&reordered_model, root);
        let first = exact_tree_pose(&first_model, &first_pose, ExactTreePoseLimits::default());
        let reordered = exact_tree_pose(
            &reordered_model,
            &reordered_pose,
            ExactTreePoseLimits::default(),
        );
        assert_eq!(first.faces, reordered.faces);
        assert_eq!(first.hinges, reordered.hinges);
        assert_eq!(first.work, reordered.work);
    }

    #[test]
    fn exact_tree_aggregate_limits_accept_observed_boundary_and_reject_one_short() {
        let model = diagonal_v_tree_model(false);
        let root = model.face_ids()[0];
        let pose = solve_diagonal_v(&model, root);
        let baseline = exact_tree_pose(&model, &pose, ExactTreePoseLimits::default());

        let mut exact = ExactTreePoseLimits::default();
        exact.max_faces = baseline.work.faces;
        exact.max_hinges = baseline.work.hinges;
        exact.max_adjacency_entries = baseline.work.adjacency_entries;
        exact.max_boundary_occurrences = baseline.work.boundary_occurrences;
        exact.max_boundary_edge_index_entries = baseline.work.boundary_edge_index_entries;
        exact.max_boundary_edge_index_operations = baseline.work.boundary_edge_index_operations;
        exact.max_unique_vertices = baseline.work.unique_vertices;
        exact.max_total_machin_terms = baseline.work.exact.machin_terms;
        exact.max_total_trig_terms = baseline.work.exact.trig_terms;
        exact.max_total_sqrt_refinements = baseline.work.exact.sqrt_refinements;
        exact.max_total_output_bits = baseline.work.total_output_bits;
        exact.cayley.max_output_bits = baseline.work.max_output_bits;
        exact.cayley.max_interval_operations = baseline.work.exact.interval_operations;
        assert!(baseline.work.exact.machin_terms > 0);
        assert!(baseline.work.exact.trig_terms > 0);
        assert!(baseline.work.exact.sqrt_refinements > 0);
        assert!(baseline.work.max_output_bits > 0);
        assert!(
            prepare_rational_cayley_tree_pose_v1(model.bind_pose(&pose).unwrap(), exact).is_ok()
        );

        let mut one_short = exact;
        one_short.max_faces -= 1;
        assert!(matches!(
            prepare_rational_cayley_tree_pose_v1(model.bind_pose(&pose).unwrap(), one_short),
            Err(CayleyError::ResourceLimitExceeded {
                resource: "faces",
                ..
            })
        ));
        let mut one_short = exact;
        one_short.max_hinges -= 1;
        assert!(matches!(
            prepare_rational_cayley_tree_pose_v1(model.bind_pose(&pose).unwrap(), one_short),
            Err(CayleyError::ResourceLimitExceeded {
                resource: "hinges",
                ..
            })
        ));
        let mut one_short = exact;
        one_short.max_adjacency_entries -= 1;
        assert!(matches!(
            prepare_rational_cayley_tree_pose_v1(model.bind_pose(&pose).unwrap(), one_short),
            Err(CayleyError::ResourceLimitExceeded {
                resource: "adjacency_entries",
                ..
            })
        ));
        let mut one_short = exact;
        one_short.max_boundary_occurrences -= 1;
        assert!(matches!(
            prepare_rational_cayley_tree_pose_v1(model.bind_pose(&pose).unwrap(), one_short),
            Err(CayleyError::ResourceLimitExceeded {
                resource: "boundary_occurrences",
                ..
            })
        ));
        let mut one_short = exact;
        one_short.max_unique_vertices -= 1;
        assert!(matches!(
            prepare_rational_cayley_tree_pose_v1(model.bind_pose(&pose).unwrap(), one_short),
            Err(CayleyError::ResourceLimitExceeded {
                resource: "unique_vertices",
                ..
            })
        ));
        let mut one_short = exact;
        one_short.max_boundary_edge_index_entries -= 1;
        assert!(matches!(
            prepare_rational_cayley_tree_pose_v1(model.bind_pose(&pose).unwrap(), one_short),
            Err(CayleyError::ResourceLimitExceeded {
                resource: "boundary_edge_index_entries",
                ..
            })
        ));
        let mut one_short = exact;
        one_short.max_boundary_edge_index_operations -= 1;
        assert!(matches!(
            prepare_rational_cayley_tree_pose_v1(model.bind_pose(&pose).unwrap(), one_short),
            Err(CayleyError::ResourceLimitExceeded {
                resource: "boundary_edge_index_operations",
                ..
            })
        ));
        let mut one_short = exact;
        one_short.max_total_machin_terms -= 1;
        assert!(matches!(
            prepare_rational_cayley_tree_pose_v1(model.bind_pose(&pose).unwrap(), one_short),
            Err(CayleyError::ResourceLimitExceeded {
                resource: "total_machin_terms",
                ..
            })
        ));
        let mut one_short = exact;
        one_short.max_total_trig_terms -= 1;
        assert!(matches!(
            prepare_rational_cayley_tree_pose_v1(model.bind_pose(&pose).unwrap(), one_short),
            Err(CayleyError::ResourceLimitExceeded {
                resource: "total_trig_terms",
                ..
            })
        ));
        let mut one_short = exact;
        one_short.max_total_sqrt_refinements -= 1;
        assert!(matches!(
            prepare_rational_cayley_tree_pose_v1(model.bind_pose(&pose).unwrap(), one_short),
            Err(CayleyError::ResourceLimitExceeded {
                resource: "total_sqrt_refinements",
                ..
            })
        ));
        let mut one_short = exact;
        one_short.cayley.max_output_bits -= 1;
        assert!(matches!(
            prepare_rational_cayley_tree_pose_v1(model.bind_pose(&pose).unwrap(), one_short),
            Err(CayleyError::ResourceLimitExceeded {
                resource: "output_bits",
                ..
            })
        ));
        let mut one_short = exact;
        one_short.max_total_output_bits -= 1;
        assert!(matches!(
            prepare_rational_cayley_tree_pose_v1(model.bind_pose(&pose).unwrap(), one_short),
            Err(CayleyError::ResourceLimitExceeded {
                resource: "total_output_bits",
                ..
            })
        ));
        let mut one_short = exact;
        one_short.cayley.max_interval_operations -= 1;
        assert!(matches!(
            prepare_rational_cayley_tree_pose_v1(model.bind_pose(&pose).unwrap(), one_short),
            Err(CayleyError::ResourceLimitExceeded {
                resource: "interval_operations",
                ..
            })
        ));
    }

    #[test]
    fn exact_f64_rejects_nonfinite_and_normalizes_signed_zero() {
        let limits = limits();
        let mut meter = WorkMeter::new(&limits);
        assert_eq!(
            exact_f64(-0.0, &mut meter, CayleyStage::Input).unwrap(),
            BigRational::zero()
        );
        assert_eq!(
            exact_f64(f64::from_bits(1), &mut meter, CayleyStage::Input).unwrap(),
            BigRational::new(BigInt::one(), BigInt::one() << 1_074_usize)
        );
        assert_eq!(
            exact_f64(f64::MAX, &mut meter, CayleyStage::Input).unwrap(),
            BigRational::from_integer(BigInt::from((1_u64 << 53) - 1) << 971_usize)
        );
        for value in [
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NAN,
            f64::from_bits(0x7ff8_1234_5678_9abc),
        ] {
            assert!(matches!(
                exact_f64(value, &mut meter, CayleyStage::Input),
                Err(CayleyError::NonFiniteInput { .. })
            ));
        }
    }

    #[test]
    fn invalid_angles_and_degenerate_axes_fail_closed() {
        for angle in [
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            -1.0,
            180.000_000_000_000_03,
            -180.000_000_000_000_03,
        ] {
            assert!(
                local_rotation_v1([0.0, 0.0, 0.0], [0.0, 1.0, 0.0], angle, 1, limits()).is_err()
            );
        }
        assert!(matches!(
            local_rotation_v1([1.0, 2.0, 3.0], [1.0, 2.0, 3.0], 90.0, 1, limits()),
            Err(CayleyError::DegenerateAxis { .. })
        ));
        for invalid_sign in [-2, 0, 2] {
            assert!(matches!(
                local_rotation_v1(
                    [0.0, 0.0, 0.0],
                    [0.0, 1.0, 0.0],
                    90.0,
                    invalid_sign,
                    limits()
                ),
                Err(CayleyError::InvalidRotationSign { .. })
            ));
        }
    }

    #[test]
    fn zero_and_half_turns_are_exact() {
        let positive_zero = rotate([0.0, 1.0, 0.0], 0.0);
        let negative_zero = rotate([0.0, 1.0, 0.0], -0.0);
        assert_eq!(positive_zero.rotation, identity_matrix());
        assert_eq!(positive_zero, negative_zero);

        let positive = rotate([0.0, 1.0, 0.0], 180.0);
        let negative = rotate([0.0, 1.0, 0.0], -180.0);
        assert_eq!(positive.rotation, negative.rotation);
        assert_eq!(
            positive.rotation,
            [
                [rational(-1), rational(0), rational(0)],
                [rational(0), rational(1), rational(0)],
                [rational(0), rational(0), rational(-1)],
            ]
        );
        assert!(matches!(
            positive.certificate,
            ExactAngleCertificate::Exact { .. }
        ));
    }

    #[test]
    fn square_axis_ninety_degree_rotations_are_exact() {
        let positive = rotate([0.0, 1.0, 0.0], 90.0);
        let negative = rotate([0.0, 1.0, 0.0], -90.0);
        assert_eq!(
            positive.rotation,
            [
                [rational(0), rational(0), rational(1)],
                [rational(0), rational(1), rational(0)],
                [rational(-1), rational(0), rational(0)],
            ]
        );
        assert_eq!(negative.rotation, transpose(&positive.rotation));

        let three_four = rotate([3.0, 4.0, 0.0], 90.0);
        assert!(matches!(
            three_four.certificate,
            ExactAngleCertificate::Exact { .. }
        ));
    }

    #[test]
    fn nonsquare_axis_and_deep_angles_have_strict_certificates() {
        for angle in [
            f64::from_bits(1),
            -f64::from_bits(1),
            f64::from_bits(90.0_f64.to_bits() - 1),
            90.0,
            f64::from_bits(90.0_f64.to_bits() + 1),
            179.0,
            f64::from_bits(180.0_f64.to_bits() - 1),
        ] {
            let rotation = rotate([1.0, 1.0, 0.0], angle);
            let ExactAngleCertificate::Bounded(certificate) = rotation.certificate else {
                panic!("non-square axis must carry a bounded certificate");
            };
            let BoundedAngleCertificate {
                max_error_degrees,
                acceptance_degrees,
                parameter,
                realized_half_tangent,
                ..
            } = *certificate;
            assert!(max_error_degrees < acceptance_degrees);
            assert!(!parameter.is_zero());
            assert!(realized_half_tangent.lower > BigRational::zero());
        }
    }

    #[test]
    fn signs_transposes_and_axis_reversal_are_consistent() {
        for angle in [10.0, 90.0, 179.0] {
            let positive = rotate([1.0, 1.0, 0.0], angle);
            let negative = rotate([1.0, 1.0, 0.0], -angle);
            assert_eq!(negative.rotation, transpose(&positive.rotation));

            let reversed =
                local_rotation_v1([1.0, 1.0, 0.0], [0.0, 0.0, 0.0], angle, -1, limits()).unwrap();
            assert_eq!(reversed.rotation, positive.rotation);
        }
    }

    #[test]
    fn result_is_bit_deterministic() {
        let first = local_rotation_v1(
            [0.125, -7.0, f64::from_bits(1)],
            [3.0, 4.0, 5.0],
            179.0,
            1,
            limits(),
        )
        .unwrap();
        let second = local_rotation_v1(
            [0.125, -7.0, f64::from_bits(1)],
            [3.0, 4.0, 5.0],
            179.0,
            1,
            limits(),
        )
        .unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn extreme_binary64_axis_remains_exactly_invariant() {
        let result = local_rotation_v1(
            [f64::MAX, -f64::MAX, f64::from_bits(1)],
            [-f64::MAX, f64::MAX, -f64::from_bits(1)],
            179.0,
            1,
            limits(),
        )
        .unwrap();
        let limits = limits();
        let mut meter = WorkMeter::new(&limits);
        assert_eq!(
            determinant(&result.rotation, &mut meter).unwrap(),
            BigRational::one()
        );
    }

    #[test]
    fn pi_sqrt_and_trig_intervals_contain_exact_landmarks() {
        let limits = limits();
        let mut meter = WorkMeter::new(&limits);
        let pi = machin_pi_interval(128, &mut meter).unwrap();
        let decimal_scale = BigInt::from(10_u8).pow(50);
        let audited_lower = BigRational::new(
            BigInt::parse_bytes(b"314159265358979323846264338327950288419716939937510", 10)
                .unwrap(),
            decimal_scale.clone(),
        );
        let audited_upper = BigRational::new(
            BigInt::parse_bytes(b"314159265358979323846264338327950288419716939937511", 10)
                .unwrap(),
            decimal_scale,
        );
        assert!(audited_lower < pi.lower);
        assert!(pi.upper < audited_upper);

        let sqrt = square_root_interval(&rational(2), 128, &mut meter).unwrap();
        assert!(&sqrt.lower * &sqrt.lower <= rational(2));
        assert!(&sqrt.upper * &sqrt.upper >= rational(2));

        let half_angle =
            RationalInterval::new(&pi.lower / rational(4), &pi.upper / rational(4)).unwrap();
        let tangent = tangent_interval(&half_angle, 128, &mut meter).unwrap();
        assert!(tangent.lower <= BigRational::one());
        assert!(tangent.upper >= BigRational::one());
    }

    #[test]
    fn deterministic_dyadic_property_matrix_preserves_all_exact_symmetries() {
        let angles = [
            f64::from_bits(1),
            10.0,
            45.0,
            f64::from_bits(90.0_f64.to_bits() - 1),
            90.0,
            f64::from_bits(90.0_f64.to_bits() + 1),
            179.0,
            f64::from_bits(180.0_f64.to_bits() - 1),
        ];
        let mut state = 0x5eed_cafe_f00d_baad_u64;
        let mut next_coordinate = || {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let integer = ((state >> 32) % 257) as i32 - 128;
            f64::from(integer) / 8.0
        };

        for case in 0..32 {
            let pivot = [next_coordinate(), next_coordinate(), next_coordinate()];
            let mut end = [next_coordinate(), next_coordinate(), next_coordinate()];
            if end == pivot {
                end[case % 3] += 0.125;
            }
            let magnitude = angles[case % angles.len()];
            let positive = local_rotation_v1(pivot, end, magnitude, 1, limits()).unwrap();
            let repeat = local_rotation_v1(pivot, end, magnitude, 1, limits()).unwrap();
            let negative = local_rotation_v1(pivot, end, magnitude, -1, limits()).unwrap();
            let reversed = local_rotation_v1(end, pivot, magnitude, -1, limits()).unwrap();

            assert_eq!(positive, repeat, "determinism case {case}");
            assert_eq!(
                negative.rotation,
                transpose(&positive.rotation),
                "sign transpose case {case}"
            );
            assert_eq!(
                reversed.rotation, positive.rotation,
                "axis/sign reversal case {case}"
            );
        }
    }

    #[test]
    fn actual_kernel_work_accepts_exact_limits_and_rejects_one_short() {
        let pivot = [0.125, -7.0, f64::from_bits(1)];
        let end = [3.0, 4.0, 5.0];
        let baseline = local_rotation_v1(pivot, end, 179.0, 1, limits()).unwrap();

        let mut exact = limits();
        exact.max_interval_operations = baseline.work.interval_operations;
        assert!(local_rotation_v1(pivot, end, 179.0, 1, exact).is_ok());
        exact.max_interval_operations -= 1;
        assert!(matches!(
            local_rotation_v1(pivot, end, 179.0, 1, exact),
            Err(CayleyError::ResourceLimitExceeded {
                resource: "interval_operations",
                ..
            })
        ));

        let mut exact = limits();
        exact.max_machin_terms_per_series = baseline.work.max_machin_series_terms;
        assert!(local_rotation_v1(pivot, end, 179.0, 1, exact).is_ok());
        exact.max_machin_terms_per_series -= 1;
        assert!(matches!(
            local_rotation_v1(pivot, end, 179.0, 1, exact),
            Err(CayleyError::ResourceLimitExceeded {
                stage: CayleyStage::Pi,
                resource: "machin_terms",
            })
        ));

        let mut exact = limits();
        exact.max_trig_terms_per_series = baseline.work.max_trig_series_terms;
        assert!(local_rotation_v1(pivot, end, 179.0, 1, exact).is_ok());
        exact.max_trig_terms_per_series -= 1;
        assert!(matches!(
            local_rotation_v1(pivot, end, 179.0, 1, exact),
            Err(CayleyError::ResourceLimitExceeded {
                stage: CayleyStage::Trigonometry,
                resource: "trig_terms",
            })
        ));

        let mut exact = limits();
        exact.max_sqrt_refinements = baseline.work.max_sqrt_call_refinements;
        assert!(local_rotation_v1(pivot, end, 179.0, 1, exact).is_ok());
        exact.max_sqrt_refinements -= 1;
        assert!(matches!(
            local_rotation_v1(pivot, end, 179.0, 1, exact),
            Err(CayleyError::ResourceLimitExceeded {
                stage: CayleyStage::SquareRoot,
                resource: "sqrt_refinements",
            })
        ));

        let mut exact = limits();
        exact.max_intermediate_bits = baseline.work.max_preflight_bits;
        assert!(local_rotation_v1(pivot, end, 179.0, 1, exact).is_ok());
        exact.max_intermediate_bits -= 1;
        match local_rotation_v1(pivot, end, 179.0, 1, exact) {
            Err(CayleyError::ResourceLimitExceeded {
                resource: "intermediate_bits",
                ..
            }) => {}
            Ok(refined) => {
                // The baseline records the cheap raw bound. Lowering that
                // bound can legitimately activate the GCD-aware proof and
                // expose a smaller, still-safe exact requirement.
                assert!(refined.work.gcd_fallback_calls > 0);
                assert!(refined.work.max_preflight_bits <= exact.max_intermediate_bits);
            }
            Err(error) => panic!("unexpected refined-preflight error: {error:?}"),
        }

        let mut exact = limits();
        exact.max_shift_bits = baseline.work.max_shift_bits;
        assert!(local_rotation_v1(pivot, end, 179.0, 1, exact).is_ok());
        exact.max_shift_bits -= 1;
        assert!(matches!(
            local_rotation_v1(pivot, end, 179.0, 1, exact),
            Err(CayleyError::ResourceLimitExceeded {
                resource: "shift_bits",
                ..
            })
        ));

        let actual_output_bits = baseline
            .rotation
            .iter()
            .flat_map(|row| row.iter())
            .chain(baseline.translation.coordinates.iter())
            .map(rational_bits)
            .max()
            .unwrap();
        assert_eq!(baseline.work.max_output_bits, actual_output_bits);
        let mut exact = limits();
        exact.max_output_bits = actual_output_bits;
        assert!(local_rotation_v1(pivot, end, 179.0, 1, exact).is_ok());
        exact.max_output_bits -= 1;
        assert!(matches!(
            local_rotation_v1(pivot, end, 179.0, 1, exact),
            Err(CayleyError::ResourceLimitExceeded {
                stage: CayleyStage::Output,
                resource: "output_bits",
            })
        ));
    }

    #[test]
    fn fixed_proof_precision_and_shift_caps_fail_closed() {
        let mut insufficient = limits();
        insufficient.max_guard_bits = DEFAULT_GUARD_BITS - 1;
        assert!(matches!(
            local_rotation_v1([0.0, 0.0, 0.0], [1.0, 1.0, 0.0], 10.0, 1, insufficient),
            Err(CayleyError::ResourceLimitExceeded {
                resource: "guard_bits",
                ..
            })
        ));

        let mut insufficient = limits();
        insufficient.max_candidate_bits = DEFAULT_CANDIDATE_BITS - 1;
        assert!(matches!(
            local_rotation_v1([0.0, 0.0, 0.0], [1.0, 1.0, 0.0], 10.0, 1, insufficient),
            Err(CayleyError::ResourceLimitExceeded {
                resource: "candidate_bits",
                ..
            })
        ));

        let subnormal = f64::from_bits(1);
        let mut insufficient = limits();
        insufficient.max_shift_bits = 1_024;
        assert!(matches!(
            local_rotation_v1([0.0, 0.0, 0.0], [1.0, 1.0, 0.0], subnormal, 1, insufficient),
            Err(CayleyError::ResourceLimitExceeded {
                resource: "shift_bits",
                ..
            })
        ));
        assert!(
            local_rotation_v1([0.0, 0.0, 0.0], [1.0, 1.0, 0.0], subnormal, 1, limits()).is_ok()
        );

        let mut one_round = limits();
        one_round.max_precision_rounds = 1;
        assert!(local_rotation_v1([0.0, 0.0, 0.0], [1.0, 1.0, 0.0], 179.0, 1, one_round).is_ok());
        one_round.max_precision_rounds = 0;
        assert!(matches!(
            local_rotation_v1([0.0, 0.0, 0.0], [1.0, 1.0, 0.0], 179.0, 1, one_round),
            Err(CayleyError::CertificateUnavailable { .. })
        ));
    }

    #[test]
    fn binary64_angle_boundary_table_is_certified_for_both_axis_classes_and_signs() {
        let largest_subnormal = f64::from_bits((1_u64 << 52) - 1);
        let boundaries = [
            f64::from_bits(1),
            largest_subnormal,
            f64::MIN_POSITIVE,
            f64::from_bits(90.0_f64.to_bits() - 1),
            90.0,
            f64::from_bits(90.0_f64.to_bits() + 1),
            f64::from_bits(180.0_f64.to_bits() - 1),
        ];
        for axis in [[0.0, 1.0, 0.0], [1.0, 1.0, 0.0]] {
            for magnitude in boundaries {
                for sign in [-1, 1] {
                    let result =
                        local_rotation_v1([0.0, 0.0, 0.0], axis, magnitude, sign, limits())
                            .unwrap_or_else(|error| {
                                panic!(
                                    "axis={axis:?}, magnitude={magnitude:?}, sign={sign}: {error:?}"
                                )
                            });
                    if let ExactAngleCertificate::Bounded(certificate) = result.certificate {
                        assert!(certificate.max_error_degrees < certificate.acceptance_degrees);
                        assert!(!certificate.parameter.is_zero());
                    }
                }
            }
        }
        for rejected in [f64::from_bits(180.0_f64.to_bits() + 1), f64::MAX] {
            for sign in [-1, 1] {
                assert!(matches!(
                    local_rotation_v1([0.0, 0.0, 0.0], [1.0, 1.0, 0.0], rejected, sign, limits()),
                    Err(CayleyError::AngleOutOfRange { .. })
                ));
            }
        }
    }

    #[test]
    fn proof_intervals_nest_and_contain_trigonometric_landmarks() {
        let limits = limits();
        let mut meter_128 = WorkMeter::new(&limits);
        let pi_128 = machin_pi_interval(128, &mut meter_128).unwrap();
        let sqrt_2_128 = square_root_interval(&rational(2), 128, &mut meter_128).unwrap();
        let mut meter_256 = WorkMeter::new(&limits);
        let pi_256 = machin_pi_interval(256, &mut meter_256).unwrap();
        let sqrt_2_256 = square_root_interval(&rational(2), 256, &mut meter_256).unwrap();

        assert!(pi_128.lower <= pi_256.lower);
        assert!(pi_256.upper <= pi_128.upper);
        assert!(&pi_256.upper - &pi_256.lower <= &pi_128.upper - &pi_128.lower);
        assert!(sqrt_2_128.lower <= sqrt_2_256.lower);
        assert!(sqrt_2_256.upper <= sqrt_2_128.upper);
        assert!(&sqrt_2_256.upper - &sqrt_2_256.lower <= &sqrt_2_128.upper - &sqrt_2_128.lower);

        let perfect = square_root_interval(&rational(4), 128, &mut meter_128).unwrap();
        assert_eq!(perfect, RationalInterval::point(rational(2)));
        assert_ne!(sqrt_2_128.lower, sqrt_2_128.upper);

        let zero = RationalInterval::point(BigRational::zero());
        let (sin_zero, cos_zero) = sine_cosine_interval(&zero, 128, &mut meter_128).unwrap();
        assert_eq!(sin_zero, RationalInterval::point(BigRational::zero()));
        assert_eq!(cos_zero, RationalInterval::point(BigRational::one()));

        let angle_sixth_128 =
            RationalInterval::new(&pi_128.lower / rational(6), &pi_128.upper / rational(6))
                .unwrap();
        let angle_sixth_256 =
            RationalInterval::new(&pi_256.lower / rational(6), &pi_256.upper / rational(6))
                .unwrap();
        let (sin_sixth_128, cos_sixth_128) =
            sine_cosine_interval(&angle_sixth_128, 128, &mut meter_128).unwrap();
        let (sin_sixth_256, cos_sixth_256) =
            sine_cosine_interval(&angle_sixth_256, 256, &mut meter_256).unwrap();
        assert!(sin_sixth_128.lower <= BigRational::new(1.into(), 2.into()));
        assert!(sin_sixth_128.upper >= BigRational::new(1.into(), 2.into()));
        let three_quarters = BigRational::new(3.into(), 4.into());
        assert!(&cos_sixth_128.lower * &cos_sixth_128.lower <= three_quarters);
        assert!(&cos_sixth_128.upper * &cos_sixth_128.upper >= three_quarters);
        assert!(sin_sixth_128.lower <= sin_sixth_256.lower);
        assert!(sin_sixth_256.upper <= sin_sixth_128.upper);
        assert!(cos_sixth_128.lower <= cos_sixth_256.lower);
        assert!(cos_sixth_256.upper <= cos_sixth_128.upper);

        let angle_quarter =
            RationalInterval::new(&pi_256.lower / rational(4), &pi_256.upper / rational(4))
                .unwrap();
        let (sin_quarter, cos_quarter) =
            sine_cosine_interval(&angle_quarter, 256, &mut meter_256).unwrap();
        let one_half = BigRational::new(1.into(), 2.into());
        for interval in [sin_quarter, cos_quarter] {
            assert!(&interval.lower * &interval.lower <= one_half);
            assert!(&interval.upper * &interval.upper >= one_half);
        }
    }

    #[test]
    fn maximum_precision_engines_complete_inside_default_resources() {
        let limits = limits();
        let mut meter = WorkMeter::new(&limits);
        let pi = machin_pi_interval(4_096, &mut meter).unwrap();
        let sqrt_2 = square_root_interval(&rational(2), 4_096, &mut meter).unwrap();
        assert!(&sqrt_2.lower * &sqrt_2.lower <= rational(2));
        assert!(&sqrt_2.upper * &sqrt_2.upper >= rational(2));

        let quarter =
            RationalInterval::new(&pi.lower / rational(4), &pi.upper / rational(4)).unwrap();
        let tangent = tangent_interval(&quarter, 4_096, &mut meter).unwrap();
        assert!(tangent.lower <= BigRational::one());
        assert!(tangent.upper >= BigRational::one());
        assert!(meter.work.interval_operations <= limits.max_interval_operations);
        assert!(meter.work.max_shift_bits <= limits.max_shift_bits);
        assert!(meter.work.max_preflight_bits <= limits.max_intermediate_bits);
    }

    #[test]
    fn resource_boundaries_accept_exact_limit_and_reject_one_short() {
        let base = limits();

        let mut exact = base;
        exact.max_interval_operations = 1;
        let mut meter = WorkMeter::new(&exact);
        assert!(meter.operation(CayleyStage::Input).is_ok());
        assert!(meter.operation(CayleyStage::Input).is_err());

        let mut exact = base;
        exact.max_machin_terms_per_series = 1;
        let mut meter = WorkMeter::new(&exact);
        assert!(meter.machin_term(CayleyStage::Pi, 1).is_ok());
        assert!(meter.machin_term(CayleyStage::Pi, 2).is_err());

        let mut exact = base;
        exact.max_trig_terms_per_series = 1;
        let mut meter = WorkMeter::new(&exact);
        assert!(meter.trig_term(CayleyStage::Trigonometry, 1).is_ok());
        assert!(meter.trig_term(CayleyStage::Trigonometry, 2).is_err());

        let mut exact = base;
        exact.max_sqrt_refinements = 1;
        let mut meter = WorkMeter::new(&exact);
        assert!(meter.sqrt_refinement(CayleyStage::SquareRoot, 1).is_ok());
        assert!(meter.sqrt_refinement(CayleyStage::SquareRoot, 2).is_err());

        let mut exact = base;
        exact.max_shift_bits = 8;
        let mut meter = WorkMeter::new(&exact);
        assert!(meter.shift(CayleyStage::Candidate, 8).is_ok());
        assert!(meter.shift(CayleyStage::Candidate, 9).is_err());

        let mut exact = base;
        exact.max_intermediate_bits = 8;
        let mut meter = WorkMeter::new(&exact);
        assert!(
            meter
                .preflight_product_bits(CayleyStage::Matrix, 4, 4)
                .is_ok()
        );
        assert!(
            meter
                .preflight_product_bits(CayleyStage::Matrix, 4, 5)
                .is_err()
        );

        let mut exact = base;
        exact.max_output_bits = 8;
        let mut meter = WorkMeter::new(&exact);
        assert!(meter.observe_output(&rational(128)).is_ok());
        assert!(meter.observe_output(&rational(256)).is_err());
    }

    #[test]
    fn rational_preflight_fast_path_avoids_fallback_gcd_accounting() {
        let mut exact = limits();
        exact.max_intermediate_bits = 5;
        let mut meter = WorkMeter::new(&exact);
        let left = BigRational::new(1.into(), 3.into());
        let right = BigRational::new(1.into(), 5.into());

        assert_eq!(
            meter
                .add_rational(&left, &right, CayleyStage::Matrix)
                .unwrap(),
            &left + &right
        );
        assert_eq!(meter.work.max_preflight_bits, 5);
        assert_eq!(meter.work.gcd_fallback_calls, 0);
        assert_eq!(meter.work.gcd_fallback_input_bits, 0);
    }

    #[test]
    fn gcd_refined_add_sub_accept_equal_and_lcm_denominators() {
        let mut equal_limits = limits();
        equal_limits.max_intermediate_bits = 4;
        equal_limits.max_gcd_fallback_calls = 8;
        equal_limits.max_gcd_fallback_input_bits = 128;

        let equal_left = BigRational::new(5.into(), 7.into());
        let equal_right = BigRational::new(6.into(), 7.into());
        let mut meter = WorkMeter::new(&equal_limits);
        let sum = meter
            .add_rational(&equal_left, &equal_right, CayleyStage::Tree)
            .unwrap();
        assert_eq!(sum, &equal_left + &equal_right);
        assert!(sum.denom().is_positive());
        assert_eq!(sum.numer().gcd(sum.denom()), BigInt::one());
        assert_eq!(meter.work.gcd_fallback_calls, 1);
        assert_eq!(meter.work.gcd_fallback_input_bits, 7);

        let mut meter = WorkMeter::new(&equal_limits);
        let difference = meter
            .subtract_rational(&equal_left, &equal_right, CayleyStage::Tree)
            .unwrap();
        assert_eq!(difference, &equal_left - &equal_right);
        assert!(difference.denom().is_positive());
        assert_eq!(difference.numer().gcd(difference.denom()), BigInt::one());

        let lcm_left = BigRational::new(1.into(), 126.into());
        let lcm_right = BigRational::new(1.into(), 63.into());
        let mut lcm_limits = equal_limits;
        lcm_limits.max_intermediate_bits = 8;
        let mut meter = WorkMeter::new(&lcm_limits);
        let sum = meter
            .add_rational(&lcm_left, &lcm_right, CayleyStage::Tree)
            .unwrap();
        assert_eq!(sum, &lcm_left + &lcm_right);
        assert!(sum.denom().is_positive());
        assert_eq!(sum.numer().gcd(sum.denom()), BigInt::one());
        assert_eq!(meter.work.gcd_fallback_calls, 2);
        assert_eq!(meter.work.gcd_fallback_input_bits, 22);
    }

    #[test]
    fn gcd_refined_mul_div_cross_cancel_and_preserve_canonical_sign() {
        let mut exact = limits();
        exact.max_intermediate_bits = 8;
        exact.max_gcd_fallback_calls = 2;
        exact.max_gcd_fallback_input_bits = 28;

        let left = BigRational::new(127.into(), 126.into());
        let positive_reciprocal = BigRational::new(126.into(), 127.into());
        let negative_reciprocal = BigRational::new((-126).into(), 127.into());
        for right in [&positive_reciprocal, &negative_reciprocal] {
            let mut meter = WorkMeter::new(&exact);
            let product = meter
                .multiply_rational(&left, right, CayleyStage::Tree)
                .unwrap();
            assert_eq!(product, &left * right);
            assert!(product.denom().is_positive());
            assert_eq!(product.numer().gcd(product.denom()), BigInt::one());
            assert_eq!(meter.work.gcd_fallback_calls, 2);
            assert_eq!(meter.work.gcd_fallback_input_bits, 28);
            assert_eq!(meter.work.max_gcd_fallback_call_input_bits, 14);
        }

        for right in [&left, &-left.clone()] {
            let mut meter = WorkMeter::new(&exact);
            let quotient = meter
                .divide_rational(&left, right, CayleyStage::Tree)
                .unwrap();
            assert_eq!(quotient, &left / right);
            assert!(quotient.denom().is_positive());
            assert_eq!(quotient.numer().gcd(quotient.denom()), BigInt::one());
            assert_eq!(meter.work.gcd_fallback_calls, 2);
            assert_eq!(meter.work.gcd_fallback_input_bits, 28);
        }
    }

    #[test]
    fn gcd_refined_limits_accept_exact_and_fail_closed_one_short() {
        let left = BigRational::new(5.into(), 7.into());
        let right = BigRational::new(6.into(), 7.into());
        let mut exact = limits();
        exact.max_intermediate_bits = 4;
        exact.max_gcd_fallback_calls = 1;
        exact.max_gcd_fallback_input_bits = 7;

        let mut meter = WorkMeter::new(&exact);
        assert_eq!(
            meter
                .add_rational(&left, &right, CayleyStage::Tree)
                .unwrap(),
            &left + &right
        );
        assert_eq!(meter.work.max_preflight_bits, 4);
        assert_eq!(meter.work.gcd_fallback_calls, 1);
        assert_eq!(meter.work.gcd_fallback_input_bits, 7);

        let mut one_short = exact;
        one_short.max_intermediate_bits -= 1;
        assert!(matches!(
            WorkMeter::new(&one_short).add_rational(&left, &right, CayleyStage::Tree),
            Err(CayleyError::ResourceLimitExceeded {
                resource: "intermediate_bits",
                ..
            })
        ));

        let mut one_short = exact;
        one_short.max_gcd_fallback_calls -= 1;
        let mut meter = WorkMeter::new(&one_short);
        assert!(matches!(
            meter.add_rational(&left, &right, CayleyStage::Tree),
            Err(CayleyError::ResourceLimitExceeded {
                resource: "gcd_fallback_calls",
                ..
            })
        ));
        assert_eq!(meter.work.gcd_fallback_calls, 0);
        assert_eq!(meter.work.gcd_fallback_input_bits, 0);

        let mut one_short = exact;
        one_short.max_gcd_fallback_input_bits -= 1;
        let mut meter = WorkMeter::new(&one_short);
        assert!(matches!(
            meter.add_rational(&left, &right, CayleyStage::Tree),
            Err(CayleyError::ResourceLimitExceeded {
                resource: "gcd_fallback_input_bits",
                ..
            })
        ));
        assert_eq!(meter.work.gcd_fallback_calls, 0);
        assert_eq!(meter.work.gcd_fallback_input_bits, 0);
    }

    #[test]
    fn gcd_fallback_accounting_overflow_fails_before_running_gcd() {
        let mut exact = limits();
        exact.max_intermediate_bits = 8;
        exact.max_gcd_fallback_calls = usize::MAX;
        exact.max_gcd_fallback_input_bits = usize::MAX;

        let mut meter = WorkMeter::new(&exact);
        meter.work.gcd_fallback_calls = usize::MAX;
        assert!(matches!(
            meter.gcd_fallback(&5.into(), &7.into(), CayleyStage::Tree),
            Err(CayleyError::ResourceLimitExceeded {
                resource: "gcd_fallback_calls",
                ..
            })
        ));
        assert_eq!(meter.work.gcd_fallback_calls, usize::MAX);
        assert_eq!(meter.work.gcd_fallback_input_bits, 0);

        let mut meter = WorkMeter::new(&exact);
        meter.work.gcd_fallback_input_bits = usize::MAX;
        assert!(matches!(
            meter.gcd_fallback(&5.into(), &7.into(), CayleyStage::Tree),
            Err(CayleyError::ResourceLimitExceeded {
                resource: "gcd_fallback_input_bits",
                ..
            })
        ));
        assert_eq!(meter.work.gcd_fallback_calls, 0);
        assert_eq!(meter.work.gcd_fallback_input_bits, usize::MAX);
    }

    #[test]
    fn gcd_refined_preflight_rejects_true_coprime_growth() {
        let mut exact = limits();
        exact.max_intermediate_bits = 8;
        exact.max_gcd_fallback_calls = 2;
        exact.max_gcd_fallback_input_bits = 28;
        let left = BigRational::new(127.into(), 125.into());
        let right = BigRational::new(123.into(), 121.into());
        let mut meter = WorkMeter::new(&exact);

        assert!(matches!(
            meter.multiply_rational(&left, &right, CayleyStage::Tree),
            Err(CayleyError::ResourceLimitExceeded {
                resource: "intermediate_bits",
                ..
            })
        ));
        assert_eq!(meter.work.gcd_fallback_calls, 2);
        assert_eq!(meter.work.gcd_fallback_input_bits, 28);
    }

    #[test]
    fn tree_gcd_fallback_budget_is_finite_and_bounded_by_operation_budget() {
        let local = CayleyLimits::default();
        let tree = ExactTreePoseLimits::default().cayley;
        assert!(tree.max_gcd_fallback_calls > local.max_gcd_fallback_calls);
        assert!(tree.max_gcd_fallback_input_bits > local.max_gcd_fallback_input_bits);
        assert!(tree.max_gcd_fallback_calls <= tree.max_interval_operations);
        assert_ne!(tree.max_gcd_fallback_calls, usize::MAX);
        assert_ne!(tree.max_gcd_fallback_input_bits, usize::MAX);
    }

    #[test]
    fn tree_total_term_limits_reject_before_incrementing_the_excess_term() {
        let base = limits();
        let totals = TotalTermLimits {
            machin_terms: 1,
            trig_terms: 1,
            sqrt_refinements: 1,
        };
        let mut meter = WorkMeter::with_total_term_limits(&base, totals);

        assert!(meter.machin_term(CayleyStage::Pi, 1).is_ok());
        assert!(matches!(
            meter.machin_term(CayleyStage::Pi, 2),
            Err(CayleyError::ResourceLimitExceeded {
                stage: CayleyStage::Tree,
                resource: "total_machin_terms",
            })
        ));
        assert_eq!(meter.work.machin_terms, 1);

        assert!(meter.trig_term(CayleyStage::Trigonometry, 1).is_ok());
        assert!(matches!(
            meter.trig_term(CayleyStage::Trigonometry, 2),
            Err(CayleyError::ResourceLimitExceeded {
                stage: CayleyStage::Tree,
                resource: "total_trig_terms",
            })
        ));
        assert_eq!(meter.work.trig_terms, 1);

        assert!(meter.sqrt_refinement(CayleyStage::SquareRoot, 1).is_ok());
        assert!(matches!(
            meter.sqrt_refinement(CayleyStage::SquareRoot, 2),
            Err(CayleyError::ResourceLimitExceeded {
                stage: CayleyStage::Tree,
                resource: "total_sqrt_refinements",
            })
        ));
        assert_eq!(meter.work.sqrt_refinements, 1);
        assert_eq!(meter.work.interval_operations, 3);
    }

    fn merge_work_fixtures() -> (CayleyWork, CayleyWork, CayleyWork) {
        let first = CayleyWork {
            interval_operations: 11,
            machin_terms: 2,
            max_machin_series_terms: 2,
            trig_terms: 3,
            max_trig_series_terms: 2,
            sqrt_refinements: 1,
            max_sqrt_call_refinements: 1,
            max_shift_bits: 7,
            max_preflight_bits: 13,
            max_observed_bits: 15,
            gcd_fallback_calls: 2,
            gcd_fallback_input_bits: 17,
            max_gcd_fallback_call_input_bits: 9,
            rational_allocations: 3,
            max_rational_allocation_bits: 11,
            total_rational_allocation_bits: 24,
            max_output_bits: 10,
        };
        let second = CayleyWork {
            interval_operations: 19,
            machin_terms: 5,
            max_machin_series_terms: 4,
            trig_terms: 2,
            max_trig_series_terms: 1,
            sqrt_refinements: 3,
            max_sqrt_call_refinements: 2,
            max_shift_bits: 9,
            max_preflight_bits: 19,
            max_observed_bits: 18,
            gcd_fallback_calls: 3,
            gcd_fallback_input_bits: 23,
            max_gcd_fallback_call_input_bits: 12,
            rational_allocations: 4,
            max_rational_allocation_bits: 13,
            total_rational_allocation_bits: 31,
            max_output_bits: 14,
        };
        let expected = CayleyWork {
            interval_operations: 30,
            machin_terms: 7,
            max_machin_series_terms: 4,
            trig_terms: 5,
            max_trig_series_terms: 2,
            sqrt_refinements: 4,
            max_sqrt_call_refinements: 2,
            max_shift_bits: 9,
            max_preflight_bits: 19,
            max_observed_bits: 18,
            gcd_fallback_calls: 5,
            gcd_fallback_input_bits: 40,
            max_gcd_fallback_call_input_bits: 12,
            rational_allocations: 7,
            max_rational_allocation_bits: 13,
            total_rational_allocation_bits: 55,
            max_output_bits: 14,
        };
        (first, second, expected)
    }

    fn exact_merge_limits(expected: &CayleyWork) -> (CayleyLimits, TotalTermLimits) {
        let mut limits = limits();
        limits.max_interval_operations = expected.interval_operations;
        limits.max_machin_terms_per_series = expected.max_machin_series_terms;
        limits.max_trig_terms_per_series = expected.max_trig_series_terms;
        limits.max_sqrt_refinements = expected.max_sqrt_call_refinements;
        limits.max_shift_bits = expected.max_shift_bits;
        limits.max_intermediate_bits = expected.max_preflight_bits;
        limits.max_gcd_fallback_calls = expected.gcd_fallback_calls;
        limits.max_gcd_fallback_input_bits = expected.gcd_fallback_input_bits;
        limits.max_rational_allocations = expected.rational_allocations;
        limits.max_rational_allocation_bits = expected.max_rational_allocation_bits;
        limits.max_total_rational_allocation_bits = expected.total_rational_allocation_bits;
        limits.max_output_bits = expected.max_output_bits;
        let totals = TotalTermLimits {
            machin_terms: expected.machin_terms,
            trig_terms: expected.trig_terms,
            sqrt_refinements: expected.sqrt_refinements,
        };
        (limits, totals)
    }

    #[test]
    fn checked_work_resume_and_merge_are_monotonic_and_accept_exact_limits() {
        let (first, second, expected) = merge_work_fixtures();
        let (limits, totals) = exact_merge_limits(&expected);
        let mut meter =
            WorkMeter::resume(&limits, Some(totals), &first, CayleyStage::Containment).unwrap();
        assert_eq!(meter.work, first);
        meter.merge_work(&second, CayleyStage::Containment).unwrap();
        assert_eq!(meter.work, expected);

        for (before, after) in [
            (first.interval_operations, expected.interval_operations),
            (first.machin_terms, expected.machin_terms),
            (first.trig_terms, expected.trig_terms),
            (first.sqrt_refinements, expected.sqrt_refinements),
            (first.gcd_fallback_calls, expected.gcd_fallback_calls),
            (
                first.gcd_fallback_input_bits,
                expected.gcd_fallback_input_bits,
            ),
            (first.rational_allocations, expected.rational_allocations),
            (
                first.total_rational_allocation_bits,
                expected.total_rational_allocation_bits,
            ),
        ] {
            assert!(after >= before);
        }
        for (before, after) in [
            (
                first.max_machin_series_terms,
                expected.max_machin_series_terms,
            ),
            (first.max_trig_series_terms, expected.max_trig_series_terms),
            (
                first.max_sqrt_call_refinements,
                expected.max_sqrt_call_refinements,
            ),
            (first.max_shift_bits, expected.max_shift_bits),
            (first.max_preflight_bits, expected.max_preflight_bits),
            (first.max_observed_bits, expected.max_observed_bits),
            (
                first.max_gcd_fallback_call_input_bits,
                expected.max_gcd_fallback_call_input_bits,
            ),
            (
                first.max_rational_allocation_bits,
                expected.max_rational_allocation_bits,
            ),
            (first.max_output_bits, expected.max_output_bits),
        ] {
            assert!(after >= before);
        }
    }

    #[test]
    fn checked_work_merge_rejects_every_one_short_limit_without_mutation() {
        let (first, second, expected) = merge_work_fixtures();
        let (exact, totals) = exact_merge_limits(&expected);

        macro_rules! one_short_limit {
            ($field:ident, $resource:literal) => {{
                let mut one_short = exact;
                one_short.$field -= 1;
                assert!(matches!(
                    first.checked_merge(
                        &second,
                        &one_short,
                        Some(totals),
                        CayleyStage::Containment
                    ),
                    Err(CayleyError::ResourceLimitExceeded {
                        resource: $resource,
                        ..
                    })
                ));
            }};
        }
        one_short_limit!(max_interval_operations, "interval_operations");
        one_short_limit!(max_machin_terms_per_series, "machin_terms");
        one_short_limit!(max_trig_terms_per_series, "trig_terms");
        one_short_limit!(max_sqrt_refinements, "sqrt_refinements");
        one_short_limit!(max_shift_bits, "shift_bits");
        one_short_limit!(max_intermediate_bits, "intermediate_bits");
        one_short_limit!(max_gcd_fallback_calls, "gcd_fallback_calls");
        one_short_limit!(max_gcd_fallback_input_bits, "gcd_fallback_input_bits");
        one_short_limit!(max_rational_allocations, "rational_allocations");
        one_short_limit!(max_rational_allocation_bits, "rational_allocation_bits");
        one_short_limit!(
            max_total_rational_allocation_bits,
            "total_rational_allocation_bits"
        );
        one_short_limit!(max_output_bits, "output_bits");

        for (one_short, resource) in [
            (
                TotalTermLimits {
                    machin_terms: totals.machin_terms - 1,
                    ..totals
                },
                "total_machin_terms",
            ),
            (
                TotalTermLimits {
                    trig_terms: totals.trig_terms - 1,
                    ..totals
                },
                "total_trig_terms",
            ),
            (
                TotalTermLimits {
                    sqrt_refinements: totals.sqrt_refinements - 1,
                    ..totals
                },
                "total_sqrt_refinements",
            ),
        ] {
            assert!(matches!(
                first.checked_merge(
                    &second,
                    &exact,
                    Some(one_short),
                    CayleyStage::Containment
                ),
                Err(CayleyError::ResourceLimitExceeded {
                    resource: actual,
                    ..
                }) if actual == resource
            ));
        }

        let mut meter =
            WorkMeter::resume(&exact, Some(totals), &first, CayleyStage::Containment).unwrap();
        let before = meter.work.clone();
        let oversized = CayleyWork {
            interval_operations: expected.interval_operations,
            ..CayleyWork::default()
        };
        assert!(
            meter
                .merge_work(&oversized, CayleyStage::Containment)
                .is_err()
        );
        assert_eq!(meter.work, before);
    }

    #[test]
    fn checked_work_merge_rejects_usize_overflow_for_every_additive_counter() {
        let mut limits = limits();
        limits.max_interval_operations = usize::MAX;
        limits.max_gcd_fallback_calls = usize::MAX;
        limits.max_gcd_fallback_input_bits = usize::MAX;
        limits.max_rational_allocations = usize::MAX;
        limits.max_total_rational_allocation_bits = usize::MAX;

        macro_rules! overflow {
            ($field:ident, $resource:literal) => {{
                let left = CayleyWork {
                    $field: usize::MAX,
                    ..CayleyWork::default()
                };
                let right = CayleyWork {
                    $field: 1,
                    ..CayleyWork::default()
                };
                assert!(matches!(
                    left.checked_merge(&right, &limits, None, CayleyStage::Containment),
                    Err(CayleyError::ResourceLimitExceeded {
                        resource: $resource,
                        ..
                    })
                ));
            }};
        }
        overflow!(interval_operations, "interval_operations");
        overflow!(machin_terms, "machin_terms");
        overflow!(trig_terms, "trig_terms");
        overflow!(sqrt_refinements, "sqrt_refinements");
        overflow!(gcd_fallback_calls, "gcd_fallback_calls");
        overflow!(gcd_fallback_input_bits, "gcd_fallback_input_bits");
        overflow!(rational_allocations, "rational_allocations");
        overflow!(
            total_rational_allocation_bits,
            "total_rational_allocation_bits"
        );
    }

    fn adversarial_rationals_16k() -> (BigRational, BigRational) {
        let left = BigRational::new(
            BigInt::one() << 16_383_usize,
            (BigInt::one() << 16_382_usize) + BigInt::one(),
        );
        let right = BigRational::new(
            (BigInt::one() << 16_383_usize) + BigInt::one(),
            BigInt::one() << 16_382_usize,
        );
        assert_eq!(rational_bits(&left), 16_384);
        assert_eq!(rational_bits(&right), 16_384);
        (left, right)
    }

    #[test]
    fn rational_clone_and_negate_charge_before_allocating_16k_values() {
        let (left, right) = adversarial_rationals_16k();
        let mut generous = limits();
        generous.max_intermediate_bits = 16_384;
        let mut baseline = WorkMeter::new(&generous);
        assert_eq!(
            baseline
                .clone_rational(&left, CayleyStage::Containment)
                .unwrap(),
            left
        );
        assert_eq!(
            baseline
                .negate_rational(&right, CayleyStage::Containment)
                .unwrap(),
            -right.clone()
        );
        assert_eq!(baseline.work.rational_allocations, 2);
        assert_eq!(baseline.work.interval_operations, 0);
        assert_eq!(baseline.work.gcd_fallback_calls, 0);
        assert!(baseline.work.max_rational_allocation_bits >= 16_384);
        assert!(
            baseline.work.total_rational_allocation_bits
                >= baseline.work.max_rational_allocation_bits
        );

        let mut exact = generous;
        exact.max_rational_allocations = baseline.work.rational_allocations;
        exact.max_rational_allocation_bits = baseline.work.max_rational_allocation_bits;
        exact.max_total_rational_allocation_bits = baseline.work.total_rational_allocation_bits;
        let mut exact_meter = WorkMeter::new(&exact);
        exact_meter
            .clone_rational(&left, CayleyStage::Containment)
            .unwrap();
        exact_meter
            .negate_rational(&right, CayleyStage::Containment)
            .unwrap();
        assert_eq!(exact_meter.work, baseline.work);

        let mut one_short = exact;
        one_short.max_rational_allocations -= 1;
        let mut meter = WorkMeter::new(&one_short);
        meter
            .clone_rational(&left, CayleyStage::Containment)
            .unwrap();
        let before = meter.work.clone();
        assert!(matches!(
            meter.negate_rational(&right, CayleyStage::Containment),
            Err(CayleyError::ResourceLimitExceeded {
                resource: "rational_allocations",
                ..
            })
        ));
        assert_eq!(meter.work.rational_allocations, before.rational_allocations);
        assert_eq!(
            meter.work.total_rational_allocation_bits,
            before.total_rational_allocation_bits
        );

        let mut one_short = exact;
        one_short.max_rational_allocation_bits -= 1;
        let mut meter = WorkMeter::new(&one_short);
        assert!(matches!(
            meter.clone_rational(&left, CayleyStage::Containment),
            Err(CayleyError::ResourceLimitExceeded {
                resource: "rational_allocation_bits",
                ..
            })
        ));
        assert_eq!(meter.work.rational_allocations, 0);

        let mut one_short = exact;
        one_short.max_total_rational_allocation_bits -= 1;
        let mut meter = WorkMeter::new(&one_short);
        meter
            .clone_rational(&left, CayleyStage::Containment)
            .unwrap();
        let before = meter.work.clone();
        assert!(matches!(
            meter.negate_rational(&right, CayleyStage::Containment),
            Err(CayleyError::ResourceLimitExceeded {
                resource: "total_rational_allocation_bits",
                ..
            })
        ));
        assert_eq!(meter.work.rational_allocations, before.rational_allocations);
        assert_eq!(
            meter.work.total_rational_allocation_bits,
            before.total_rational_allocation_bits
        );

        let mut one_short = exact;
        one_short.max_intermediate_bits -= 1;
        let mut meter = WorkMeter::new(&one_short);
        assert!(matches!(
            meter.clone_rational(&left, CayleyStage::Containment),
            Err(CayleyError::ResourceLimitExceeded {
                resource: "intermediate_bits",
                ..
            })
        ));
        assert_eq!(meter.work.rational_allocations, 0);
        assert_eq!(meter.work.total_rational_allocation_bits, 0);
    }

    #[test]
    fn rational_allocation_charge_overflow_is_atomic() {
        let mut unlimited = limits();
        unlimited.max_rational_allocations = usize::MAX;
        unlimited.max_rational_allocation_bits = usize::MAX;
        unlimited.max_total_rational_allocation_bits = usize::MAX;

        let mut count_meter = WorkMeter::new(&unlimited);
        count_meter.work.rational_allocations = usize::MAX;
        let count_before = count_meter.work.clone();
        assert!(matches!(
            count_meter.charge_rational_allocations(&[1], CayleyStage::Containment),
            Err(CayleyError::ResourceLimitExceeded {
                resource: "rational_allocations",
                ..
            })
        ));
        assert_eq!(count_meter.work, count_before);

        let mut total_meter = WorkMeter::new(&unlimited);
        total_meter.work.total_rational_allocation_bits = usize::MAX;
        let total_before = total_meter.work.clone();
        assert!(matches!(
            total_meter.charge_rational_allocations(&[1], CayleyStage::Containment),
            Err(CayleyError::ResourceLimitExceeded {
                resource: "total_rational_allocation_bits",
                ..
            })
        ));
        assert_eq!(total_meter.work, total_before);
    }

    #[test]
    fn rational_compare_charges_both_cross_products_before_allocating() {
        let (left, right) = adversarial_rationals_16k();
        let mut generous = limits();
        generous.max_intermediate_bits = 32_768;
        let mut baseline = WorkMeter::new(&generous);
        assert_eq!(
            baseline
                .compare_rational(&left, &right, CayleyStage::Containment)
                .unwrap(),
            left.cmp(&right)
        );
        assert_eq!(baseline.work.interval_operations, 1);
        assert_eq!(baseline.work.gcd_fallback_calls, 1);
        assert_eq!(baseline.work.rational_allocations, 4);
        assert!(baseline.work.max_preflight_bits >= 32_767);

        let mut exact = generous;
        exact.max_interval_operations = baseline.work.interval_operations;
        exact.max_intermediate_bits = baseline.work.max_preflight_bits;
        exact.max_gcd_fallback_calls = baseline.work.gcd_fallback_calls;
        exact.max_gcd_fallback_input_bits = baseline.work.gcd_fallback_input_bits;
        exact.max_rational_allocations = baseline.work.rational_allocations;
        exact.max_rational_allocation_bits = baseline.work.max_rational_allocation_bits;
        exact.max_total_rational_allocation_bits = baseline.work.total_rational_allocation_bits;
        let mut exact_meter = WorkMeter::new(&exact);
        assert_eq!(
            exact_meter
                .compare_rational(&left, &right, CayleyStage::Containment)
                .unwrap(),
            left.cmp(&right)
        );
        assert_eq!(exact_meter.work, baseline.work);

        for (one_short, resource) in [
            (
                {
                    let mut value = exact;
                    value.max_interval_operations -= 1;
                    value
                },
                "interval_operations",
            ),
            (
                {
                    let mut value = exact;
                    value.max_intermediate_bits -= 1;
                    value
                },
                "intermediate_bits",
            ),
            (
                {
                    let mut value = exact;
                    value.max_gcd_fallback_calls -= 1;
                    value
                },
                "gcd_fallback_calls",
            ),
            (
                {
                    let mut value = exact;
                    value.max_gcd_fallback_input_bits -= 1;
                    value
                },
                "gcd_fallback_input_bits",
            ),
            (
                {
                    let mut value = exact;
                    value.max_rational_allocations -= 1;
                    value
                },
                "rational_allocations",
            ),
            (
                {
                    let mut value = exact;
                    value.max_rational_allocation_bits -= 1;
                    value
                },
                "rational_allocation_bits",
            ),
            (
                {
                    let mut value = exact;
                    value.max_total_rational_allocation_bits -= 1;
                    value
                },
                "total_rational_allocation_bits",
            ),
        ] {
            let mut meter = WorkMeter::new(&one_short);
            assert!(matches!(
                meter.compare_rational(&left, &right, CayleyStage::Containment),
                Err(CayleyError::ResourceLimitExceeded {
                    resource: actual,
                    ..
                }) if actual == resource
            ));
            assert_eq!(meter.work.rational_allocations, 0);
            assert_eq!(meter.work.total_rational_allocation_bits, 0);
        }
    }

    #[test]
    fn rational_compare_preflights_gcd_reduced_cross_products() {
        let shared_denominator = BigInt::one() << 16_000_usize;
        let left = BigRational::new(
            (BigInt::one() << 16_000_usize) + BigInt::one(),
            BigInt::from(3_u8) * &shared_denominator,
        );
        let right = BigRational::new(
            (BigInt::one() << 16_000_usize) + BigInt::from(3_u8),
            BigInt::from(5_u8) * shared_denominator,
        );
        let raw_cross_bits =
            refined_product_bits(left.numer(), right.denom(), CayleyStage::Containment).unwrap();

        let mut generous = limits();
        generous.max_intermediate_bits = raw_cross_bits;
        let mut baseline = WorkMeter::new(&generous);
        assert_eq!(
            baseline
                .compare_rational(&left, &right, CayleyStage::Containment)
                .unwrap(),
            left.cmp(&right)
        );
        assert_eq!(baseline.work.interval_operations, 1);
        assert_eq!(baseline.work.gcd_fallback_calls, 1);
        assert!(baseline.work.max_preflight_bits < raw_cross_bits);

        let mut reduced_only = generous;
        reduced_only.max_intermediate_bits = baseline.work.max_preflight_bits;
        let mut meter = WorkMeter::new(&reduced_only);
        assert_eq!(
            meter
                .compare_rational(&left, &right, CayleyStage::Containment)
                .unwrap(),
            left.cmp(&right)
        );
        assert_eq!(meter.work, baseline.work);
    }

    #[test]
    fn rational_compare_fast_paths_do_not_consume_gcd_or_allocation_budgets() {
        let cases = [
            (
                BigRational::from_integer(BigInt::from(-1_i8)),
                BigRational::from_integer(BigInt::from(2_u8)),
            ),
            (
                BigRational::zero(),
                BigRational::from_integer(BigInt::from(2_u8)),
            ),
            (BigRational::zero(), BigRational::zero()),
            (
                BigRational::new(BigInt::one(), BigInt::from(7_u8)),
                BigRational::new(BigInt::from(2_u8), BigInt::from(7_u8)),
            ),
            (
                BigRational::new(BigInt::from(-2_i8), BigInt::from(7_u8)),
                BigRational::new(BigInt::from(-1_i8), BigInt::from(7_u8)),
            ),
        ];
        for (left, right) in cases {
            let mut strict = limits();
            strict.max_gcd_fallback_calls = 0;
            strict.max_gcd_fallback_input_bits = 0;
            strict.max_rational_allocations = 0;
            strict.max_rational_allocation_bits = 0;
            strict.max_total_rational_allocation_bits = 0;
            let mut meter = WorkMeter::new(&strict);
            assert_eq!(
                meter
                    .compare_rational(&left, &right, CayleyStage::Containment)
                    .unwrap(),
                left.cmp(&right)
            );
            assert_eq!(meter.work.interval_operations, 1);
            assert_eq!(meter.work.gcd_fallback_calls, 0);
            assert_eq!(meter.work.rational_allocations, 0);
            assert_eq!(meter.work.total_rational_allocation_bits, 0);
        }
    }

    #[test]
    fn rational_compare_fast_paths_still_preflight_both_inputs() {
        let left = BigRational::new(BigInt::one() << 64_usize, BigInt::one());
        let right = BigRational::new(BigInt::one() << 65_usize, BigInt::one());
        let mut strict = limits();
        strict.max_intermediate_bits = 64;
        strict.max_gcd_fallback_calls = 0;
        strict.max_rational_allocations = 0;
        let mut meter = WorkMeter::new(&strict);
        assert!(matches!(
            meter.compare_rational(&left, &right, CayleyStage::Containment),
            Err(CayleyError::ResourceLimitExceeded {
                resource: "intermediate_bits",
                ..
            })
        ));
        assert_eq!(meter.work.gcd_fallback_calls, 0);
        assert_eq!(meter.work.rational_allocations, 0);
    }

    #[test]
    fn subtract_16k_value_does_not_allocate_a_negated_operand() {
        let (left, right) = adversarial_rationals_16k();
        let mut exact = limits();
        exact.max_intermediate_bits = 32_768;
        exact.max_rational_allocations = 0;
        exact.max_rational_allocation_bits = 0;
        exact.max_total_rational_allocation_bits = 0;
        let mut meter = WorkMeter::new(&exact);
        assert_eq!(
            meter
                .subtract_rational(&left, &right, CayleyStage::Containment)
                .unwrap(),
            &left - &right
        );
        assert_eq!(meter.work.rational_allocations, 0);
        assert_eq!(meter.work.total_rational_allocation_bits, 0);

        let mut one_short = exact;
        one_short.max_intermediate_bits = meter.work.max_preflight_bits - 1;
        assert!(matches!(
            WorkMeter::new(&one_short).subtract_rational(&left, &right, CayleyStage::Containment),
            Err(CayleyError::ResourceLimitExceeded {
                resource: "intermediate_bits",
                ..
            })
        ));
    }

    #[test]
    fn total_output_storage_charges_numerator_and_denominator_bits() {
        let value = BigRational::new(BigInt::from(5_u8), BigInt::from(8_u8));
        assert_eq!(rational_bits(&value), 4);
        let mut max_observed = 0;
        let mut total = 0;
        charge_rational_output(&value, &mut max_observed, &mut total, 4, 7).unwrap();
        assert_eq!(max_observed, 4);
        assert_eq!(total, 7);

        let mut max_observed = 0;
        let mut total = 0;
        assert!(matches!(
            charge_rational_output(&value, &mut max_observed, &mut total, 4, 6),
            Err(CayleyError::ResourceLimitExceeded {
                resource: "total_output_bits",
                ..
            })
        ));
        assert_eq!(max_observed, 4);
        assert_eq!(total, 0);

        let mut max_observed = 0;
        let mut total = 0;
        assert!(matches!(
            charge_rational_output(&value, &mut max_observed, &mut total, 3, usize::MAX),
            Err(CayleyError::ResourceLimitExceeded {
                resource: "output_bits",
                ..
            })
        ));
        assert_eq!(max_observed, 4);
        assert_eq!(total, 0);
    }
}
