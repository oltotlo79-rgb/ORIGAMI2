//! Exact, resource-bounded Cayley rotations for a future watertight pose.
//!
//! This module is deliberately private until the exact tree-pose issuer can
//! bind its output to a pose certificate.  In particular, none of the
//! rationals below may be supplied by a caller as collision evidence.

use std::cmp::Ordering;

use num_bigint::{BigInt, BigUint, Sign};
use num_rational::BigRational;
use num_traits::{One, Signed, Zero};

const RATIONAL_CAYLEY_LOCAL_ROTATION_V1: &str = "rational_cayley_local_rotation_v1";
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
}

struct WorkMeter<'a> {
    limits: &'a CayleyLimits,
    work: CayleyWork,
}

impl<'a> WorkMeter<'a> {
    fn new(limits: &'a CayleyLimits) -> Self {
        Self {
            limits,
            work: CayleyWork::default(),
        }
    }

    fn operation(&mut self, stage: CayleyStage) -> Result<(), CayleyError> {
        self.work.interval_operations = self.work.interval_operations.checked_add(1).ok_or(
            CayleyError::ResourceLimitExceeded {
                stage,
                resource: "interval_operations",
            },
        )?;
        if self.work.interval_operations > self.limits.max_interval_operations {
            return Err(CayleyError::ResourceLimitExceeded {
                stage,
                resource: "interval_operations",
            });
        }
        Ok(())
    }

    fn machin_term(&mut self, stage: CayleyStage, local_terms: usize) -> Result<(), CayleyError> {
        if local_terms > self.limits.max_machin_terms_per_series {
            return Err(CayleyError::ResourceLimitExceeded {
                stage,
                resource: "machin_terms",
            });
        }
        self.work.max_machin_series_terms = self.work.max_machin_series_terms.max(local_terms);
        self.work.machin_terms =
            self.work
                .machin_terms
                .checked_add(1)
                .ok_or(CayleyError::ResourceLimitExceeded {
                    stage,
                    resource: "machin_terms",
                })?;
        self.operation(stage)
    }

    fn trig_term(&mut self, stage: CayleyStage, local_terms: usize) -> Result<(), CayleyError> {
        if local_terms > self.limits.max_trig_terms_per_series {
            return Err(CayleyError::ResourceLimitExceeded {
                stage,
                resource: "trig_terms",
            });
        }
        self.work.max_trig_series_terms = self.work.max_trig_series_terms.max(local_terms);
        self.work.trig_terms =
            self.work
                .trig_terms
                .checked_add(1)
                .ok_or(CayleyError::ResourceLimitExceeded {
                    stage,
                    resource: "trig_terms",
                })?;
        self.operation(stage)
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
        self.work.max_sqrt_call_refinements =
            self.work.max_sqrt_call_refinements.max(local_refinements);
        self.work.sqrt_refinements = self.work.sqrt_refinements.checked_add(1).ok_or(
            CayleyError::ResourceLimitExceeded {
                stage,
                resource: "sqrt_refinements",
            },
        )?;
        self.operation(stage)
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

    fn add_rational(
        &mut self,
        left: &BigRational,
        right: &BigRational,
        stage: CayleyStage,
    ) -> Result<BigRational, CayleyError> {
        self.operation(stage)?;
        let left_product = bigint_bits(left.numer())
            .checked_add(bigint_bits(right.denom()))
            .ok_or(CayleyError::ResourceLimitExceeded {
                stage,
                resource: "intermediate_bits",
            })?;
        let right_product = bigint_bits(right.numer())
            .checked_add(bigint_bits(left.denom()))
            .ok_or(CayleyError::ResourceLimitExceeded {
                stage,
                resource: "intermediate_bits",
            })?;
        let denominator = bigint_bits(left.denom())
            .checked_add(bigint_bits(right.denom()))
            .ok_or(CayleyError::ResourceLimitExceeded {
                stage,
                resource: "intermediate_bits",
            })?;
        self.preflight_value_bits(stage, left_product.max(right_product).saturating_add(1))?;
        self.preflight_value_bits(stage, denominator)?;
        let result = left + right;
        self.observe_rational(stage, &result)?;
        Ok(result)
    }

    fn subtract_rational(
        &mut self,
        left: &BigRational,
        right: &BigRational,
        stage: CayleyStage,
    ) -> Result<BigRational, CayleyError> {
        self.add_rational(left, &-right, stage)
    }

    fn multiply_rational(
        &mut self,
        left: &BigRational,
        right: &BigRational,
        stage: CayleyStage,
    ) -> Result<BigRational, CayleyError> {
        self.operation(stage)?;
        self.preflight_product_bits(stage, bigint_bits(left.numer()), bigint_bits(right.numer()))?;
        self.preflight_product_bits(stage, bigint_bits(left.denom()), bigint_bits(right.denom()))?;
        let result = left * right;
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
        self.preflight_product_bits(stage, bigint_bits(left.numer()), bigint_bits(right.denom()))?;
        self.preflight_product_bits(stage, bigint_bits(left.denom()), bigint_bits(right.numer()))?;
        let result = left / right;
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
    let pivot = exact_point(pivot, &mut meter)?;
    let end = exact_point(end, &mut meter)?;
    let absolute_degrees = exact_f64(angle_magnitude_degrees, &mut meter, CayleyStage::Input)?;
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

    let raw_direction = exact_between(&pivot, &end, &mut meter)?;
    let (direction, delta) = normalize_direction(&raw_direction, &mut meter)?;

    if absolute_degrees.is_zero() {
        let rotation = identity_matrix();
        let translation = zero_vector();
        verify_invariants(
            &rotation,
            &translation,
            &pivot,
            &end,
            &direction,
            &mut meter,
        )?;
        observe_transform_output(&rotation, &translation, &mut meter)?;
        return Ok(ExactLocalRotation {
            rotation,
            translation,
            certificate: ExactAngleCertificate::Exact { target_degrees },
            work: meter.work,
            version: RATIONAL_CAYLEY_LOCAL_ROTATION_V1,
        });
    }

    if absolute_degrees == maximum {
        let rotation = half_turn(&direction, &delta, &mut meter)?;
        let translation = fixed_point_translation(&rotation, &pivot, &mut meter)?;
        verify_invariants(
            &rotation,
            &translation,
            &pivot,
            &end,
            &direction,
            &mut meter,
        )?;
        observe_transform_output(&rotation, &translation, &mut meter)?;
        return Ok(ExactLocalRotation {
            rotation,
            translation,
            certificate: ExactAngleCertificate::Exact { target_degrees },
            work: meter.work,
            version: RATIONAL_CAYLEY_LOCAL_ROTATION_V1,
        });
    }

    let acceptance = adjacent_angle_acceptance(angle_magnitude_degrees, &mut meter)?;

    if absolute_degrees == BigRational::from_integer(BigInt::from(90_u8))
        && let Some(root) = exact_rational_square_root(&delta, &mut meter)?
    {
        let sign = if rotation_sign < 0 {
            -BigRational::one()
        } else {
            BigRational::one()
        };
        let parameter = meter.divide_rational(&sign, &root, CayleyStage::Candidate)?;
        let rotation = cayley_matrix(&direction, &delta, &parameter, &mut meter)?;
        let translation = fixed_point_translation(&rotation, &pivot, &mut meter)?;
        verify_invariants(
            &rotation,
            &translation,
            &pivot,
            &end,
            &direction,
            &mut meter,
        )?;
        observe_transform_output(&rotation, &translation, &mut meter)?;
        return Ok(ExactLocalRotation {
            rotation,
            translation,
            certificate: ExactAngleCertificate::Exact { target_degrees },
            work: meter.work,
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
            &mut meter,
        ) {
            Ok((rotation, certificate)) => {
                let translation = fixed_point_translation(&rotation, &pivot, &mut meter)?;
                verify_invariants(
                    &rotation,
                    &translation,
                    &pivot,
                    &end,
                    &direction,
                    &mut meter,
                )?;
                observe_transform_output(&rotation, &translation, &mut meter)?;
                return Ok(ExactLocalRotation {
                    rotation,
                    translation,
                    certificate,
                    work: meter.work,
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
    Ok(
        DyadicInterval::from_rational_outward(&exact, work_precision, meter, CayleyStage::Pi)?
            .to_rational(meter, CayleyStage::Pi)?,
    )
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
    let coordinates = try_array3(|row| {
        let mut result = translation.coordinates[row].clone();
        for (coefficient, coordinate) in rotation[row].iter().zip(&point.coordinates) {
            let product = meter.multiply_rational(coefficient, coordinate, CayleyStage::Matrix)?;
            result = meter.add_rational(&result, &product, CayleyStage::Matrix)?;
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

fn rational_bits(value: &BigRational) -> usize {
    bigint_bits(value.numer()).max(bigint_bits(value.denom()))
}

fn try_array3<T>(
    mut element: impl FnMut(usize) -> Result<T, CayleyError>,
) -> Result<[T; 3], CayleyError> {
    Ok([element(0)?, element(1)?, element(2)?])
}

#[cfg(test)]
mod tests {
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
        assert!(matches!(
            local_rotation_v1(pivot, end, 179.0, 1, exact),
            Err(CayleyError::ResourceLimitExceeded {
                resource: "intermediate_bits",
                ..
            })
        ));

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
}
