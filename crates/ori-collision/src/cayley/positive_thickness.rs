//! Exact scalar prerequisites for a future positive-thickness hinge model.
//!
//! This module deliberately does **not** classify a prism pair, prove that an
//! intersection is contained in a hinge corridor, or authorize contact.  A
//! `FiniteRadiusScalarFits` value only proves that the analytic centered-slab
//! radius is finite and no longer than the supplied exact finite hinge
//! segment.
//! It stays private until later code also proves all of the following:
//!
//! - both normals are the co-oriented rest-local `+Y` columns of the two exact
//!   rigid face transforms, not triangle-winding or prism outward normals;
//! - the shared edge and its opposite boundary occurrences are authenticated;
//! - every source triangle lies in the required opposing material half-plane;
//! - at least one source triangle bounds each interaction axially;
//! - every triangle pair is scanned and its complete intersection is proved
//!   inside the finite corridor; and
//! - the canonical exact pose `E` is reconciled with the stored binary64 pose
//!   `F`, including both position and normal error.
//!
//! In particular, equality `R = L` in `E` alone is not production authority.
//! No world-coordinate tolerance may enlarge `L`.  The exact scalar gate runs
//! before any future point predicate, because `t = 0` together with
//! `cos²(theta/2) = 0` would otherwise make the cleared radial inequality
//! vacuously true for every point.

use std::cmp::Ordering;

use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{One, Signed, Zero};
use ori_domain::{FaceId, VertexId};
use ori_topology::FoldAssignment;

use super::{
    CayleyError, CayleyLimits, CayleyStage, CayleyWork, ExactFacePose, ExactPoint3,
    ExactRigidTransform, ExactVector3, RATIONAL_CAYLEY_TREE_POSE_V1, RationalCayleyTreePose,
    WorkMeter, apply_exact_transform, canonical_point_eq, exact_f64, point3_array, rational_bits,
    rational_storage_bits, try_array3, verify_exact_rotation,
};

const STAGE: CayleyStage = CayleyStage::Containment;
const SCALAR_INPUT_RATIONALS: usize = 13;
const TRIANGULAR_HINGE_INPUT_RATIONALS: usize = 48;

/// Hard, non-expandable limits for this scalar prerequisite.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FiniteRadiusScalarLimits {
    max_input_rational_storage_bits: usize,
    max_total_input_storage_bits: usize,
    max_interval_operations: usize,
    max_shift_bits: usize,
    max_intermediate_bits: usize,
    max_gcd_fallback_calls: usize,
    max_gcd_fallback_input_bits: usize,
    max_rational_allocations: usize,
    max_rational_allocation_bits: usize,
    max_total_rational_allocation_bits: usize,
}

impl Default for FiniteRadiusScalarLimits {
    fn default() -> Self {
        let exact = CayleyLimits::default();
        Self {
            max_input_rational_storage_bits: exact.max_intermediate_bits,
            max_total_input_storage_bits: SCALAR_INPUT_RATIONALS * exact.max_intermediate_bits,
            max_interval_operations: 256,
            max_shift_bits: exact.max_shift_bits,
            max_intermediate_bits: exact.max_intermediate_bits,
            max_gcd_fallback_calls: 64,
            max_gcd_fallback_input_bits: 2_097_152,
            max_rational_allocations: 256,
            max_rational_allocation_bits: exact.max_rational_allocation_bits,
            max_total_rational_allocation_bits: SCALAR_INPUT_RATIONALS
                * exact.max_rational_allocation_bits,
        }
    }
}

impl FiniteRadiusScalarLimits {
    fn projected(self) -> Self {
        let hard = Self::default();
        Self {
            max_input_rational_storage_bits: self
                .max_input_rational_storage_bits
                .min(hard.max_input_rational_storage_bits),
            max_total_input_storage_bits: self
                .max_total_input_storage_bits
                .min(hard.max_total_input_storage_bits),
            max_interval_operations: self
                .max_interval_operations
                .min(hard.max_interval_operations),
            max_shift_bits: self.max_shift_bits.min(hard.max_shift_bits),
            max_intermediate_bits: self.max_intermediate_bits.min(hard.max_intermediate_bits),
            max_gcd_fallback_calls: self.max_gcd_fallback_calls.min(hard.max_gcd_fallback_calls),
            max_gcd_fallback_input_bits: self
                .max_gcd_fallback_input_bits
                .min(hard.max_gcd_fallback_input_bits),
            max_rational_allocations: self
                .max_rational_allocations
                .min(hard.max_rational_allocations),
            max_rational_allocation_bits: self
                .max_rational_allocation_bits
                .min(hard.max_rational_allocation_bits),
            max_total_rational_allocation_bits: self
                .max_total_rational_allocation_bits
                .min(hard.max_total_rational_allocation_bits),
        }
    }

    fn exact(self) -> CayleyLimits {
        let hard = CayleyLimits::default();
        CayleyLimits {
            max_precision_rounds: 0,
            max_guard_bits: 0,
            max_candidate_bits: 0,
            max_machin_terms_per_series: 0,
            max_trig_terms_per_series: 0,
            max_sqrt_refinements: 0,
            max_interval_operations: self.max_interval_operations,
            max_shift_bits: self.max_shift_bits,
            max_intermediate_bits: self.max_intermediate_bits,
            max_gcd_fallback_calls: self.max_gcd_fallback_calls,
            max_gcd_fallback_input_bits: self.max_gcd_fallback_input_bits,
            max_rational_allocations: self.max_rational_allocations,
            max_rational_allocation_bits: self.max_rational_allocation_bits,
            max_total_rational_allocation_bits: self.max_total_rational_allocation_bits,
            max_output_bits: hard.max_output_bits,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct FiniteRadiusScalarWork {
    input_rationals: usize,
    max_input_rational_storage_bits: usize,
    total_input_storage_bits: usize,
    exact: CayleyWork,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FiniteRadiusScalarError {
    ResourceLimitExceeded,
}

/// Borrow-bound evidence for the analytic scalar inequality only.
///
/// The fields intentionally remain private.  A later containment predicate
/// may borrow this token, but it must not be converted directly into a
/// topology-policy decision.
#[derive(Debug)]
struct FiniteRadiusScalarFits<'a> {
    paper_thickness_bits: u64,
    hinge_start: &'a ExactPoint3,
    hinge_end: &'a ExactPoint3,
    left_cooriented_mid_surface_normal: &'a ExactVector3,
    right_cooriented_mid_surface_normal: &'a ExactVector3,
}

#[derive(Debug)]
enum AnalyticFiniteRadiusResult<'a> {
    FitsFiniteSegment(FiniteRadiusScalarFits<'a>),
    LayerOffsetUnmodeled,
    Unresolved,
}

#[derive(Debug)]
struct FiniteRadiusScalarAnalysis<'a> {
    result: AnalyticFiniteRadiusResult<'a>,
    work: FiniteRadiusScalarWork,
}

#[derive(Debug, Clone, Copy)]
struct ExactHingeAxis<'a> {
    start: &'a ExactPoint3,
    end: &'a ExactPoint3,
}

#[derive(Debug, Clone, Copy)]
struct CoorientedMidSurfaceNormals<'a> {
    left_local_y: &'a ExactVector3,
    right_local_y: &'a ExactVector3,
}

fn analyze_finite_radius_scalar<'a>(
    paper_thickness_mm: f64,
    hinge: ExactHingeAxis<'a>,
    normals: CoorientedMidSurfaceNormals<'a>,
    limits: FiniteRadiusScalarLimits,
) -> Result<FiniteRadiusScalarAnalysis<'a>, FiniteRadiusScalarError> {
    let limits = limits.projected();
    let exact_limits = limits.exact();
    let mut meter = WorkMeter::new(&exact_limits);
    let mut work = FiniteRadiusScalarWork::default();

    // Inspect the original bits before any arithmetic.  In particular,
    // `f64::MIN_POSITIVE / 2` and the minimum subnormal must not be rounded
    // through a binary64 half-thickness temporary.
    if !positive_finite_binary64(paper_thickness_mm) {
        return Ok(FiniteRadiusScalarAnalysis {
            result: AnalyticFiniteRadiusResult::Unresolved,
            work,
        });
    }

    let result = calculate_finite_radius_scalar(
        paper_thickness_mm,
        hinge,
        normals,
        &limits,
        &mut work,
        &mut meter,
    );
    work.exact = meter.work;
    match result {
        Ok(result) => Ok(FiniteRadiusScalarAnalysis { result, work }),
        Err(CayleyError::ResourceLimitExceeded { .. }) => {
            Err(FiniteRadiusScalarError::ResourceLimitExceeded)
        }
        Err(_) => Ok(FiniteRadiusScalarAnalysis {
            result: AnalyticFiniteRadiusResult::Unresolved,
            work,
        }),
    }
}

fn calculate_finite_radius_scalar<'a>(
    paper_thickness_mm: f64,
    hinge: ExactHingeAxis<'a>,
    normals: CoorientedMidSurfaceNormals<'a>,
    limits: &FiniteRadiusScalarLimits,
    work: &mut FiniteRadiusScalarWork,
    meter: &mut WorkMeter<'_>,
) -> Result<AnalyticFiniteRadiusResult<'a>, CayleyError> {
    let paper_thickness = exact_f64(paper_thickness_mm, meter, STAGE)?;
    let Some(paper_thickness) = prepare_rational_input(&paper_thickness, limits, work, meter)?
    else {
        return Ok(AnalyticFiniteRadiusResult::Unresolved);
    };
    let Some(start) = prepare_point_input(hinge.start, limits, work, meter)? else {
        return Ok(AnalyticFiniteRadiusResult::Unresolved);
    };
    let Some(end) = prepare_point_input(hinge.end, limits, work, meter)? else {
        return Ok(AnalyticFiniteRadiusResult::Unresolved);
    };
    let Some(left_normal) = prepare_vector_input(normals.left_local_y, limits, work, meter)? else {
        return Ok(AnalyticFiniteRadiusResult::Unresolved);
    };
    let Some(right_normal) = prepare_vector_input(normals.right_local_y, limits, work, meter)?
    else {
        return Ok(AnalyticFiniteRadiusResult::Unresolved);
    };
    if work.input_rationals != SCALAR_INPUT_RATIONALS {
        return Err(CayleyError::InvariantFailure { stage: STAGE });
    }

    let zero = BigRational::zero();
    let one = BigRational::one();
    let two = BigRational::from_integer(BigInt::from(2_u8));
    let negative_one = -BigRational::one();

    // Compute h from the exact lift of t.  Computing t/2 in binary64 first
    // would underflow the minimum subnormal to zero.
    let half_thickness = meter.divide_rational(&paper_thickness, &two, STAGE)?;
    if meter.compare_rational(&half_thickness, &zero, STAGE)? != Ordering::Greater {
        return Ok(AnalyticFiniteRadiusResult::Unresolved);
    }
    let half_thickness_squared =
        meter.multiply_rational(&half_thickness, &half_thickness, STAGE)?;

    let axis = exact_between(&start, &end, meter)?;
    let length_squared = exact_dot(&axis, &axis, meter)?;
    if meter.compare_rational(&length_squared, &zero, STAGE)? != Ordering::Greater {
        return Ok(AnalyticFiniteRadiusResult::Unresolved);
    }

    let left_norm_squared = exact_dot(&left_normal, &left_normal, meter)?;
    let right_norm_squared = exact_dot(&right_normal, &right_normal, meter)?;
    if meter.compare_rational(&left_norm_squared, &one, STAGE)? != Ordering::Equal
        || meter.compare_rational(&right_norm_squared, &one, STAGE)? != Ordering::Equal
    {
        return Ok(AnalyticFiniteRadiusResult::Unresolved);
    }

    // The half-slab radius is a two-dimensional cross-section about the hinge
    // axis.  Any axial normal component invalidates that derivation.
    let left_axis_dot = exact_dot(&axis, &left_normal, meter)?;
    let right_axis_dot = exact_dot(&axis, &right_normal, meter)?;
    if meter.compare_rational(&left_axis_dot, &zero, STAGE)? != Ordering::Equal
        || meter.compare_rational(&right_axis_dot, &zero, STAGE)? != Ordering::Equal
    {
        return Ok(AnalyticFiniteRadiusResult::Unresolved);
    }

    let normal_dot = exact_dot(&left_normal, &right_normal, meter)?;
    if meter.compare_rational(&normal_dot, &negative_one, STAGE)? == Ordering::Less
        || meter.compare_rational(&normal_dot, &one, STAGE)? == Ordering::Greater
    {
        return Ok(AnalyticFiniteRadiusResult::Unresolved);
    }
    let one_plus_dot = meter.add_rational(&one, &normal_dot, STAGE)?;
    let cosine_half_squared = meter.divide_rational(&one_plus_dot, &two, STAGE)?;
    match meter.compare_rational(&cosine_half_squared, &zero, STAGE)? {
        Ordering::Less => return Ok(AnalyticFiniteRadiusResult::Unresolved),
        Ordering::Equal => return Ok(AnalyticFiniteRadiusResult::LayerOffsetUnmodeled),
        Ordering::Greater => {}
    }
    if meter.compare_rational(&cosine_half_squared, &one, STAGE)? == Ordering::Greater {
        return Ok(AnalyticFiniteRadiusResult::Unresolved);
    }

    // `f64::EPSILON` is exactly 2^-52. Build the version constant through the
    // same bit-exact input lift so its shift, operation, and intermediate size
    // are covered by the meter.
    let numerical_flat_fold_limit = exact_f64(f64::EPSILON, meter, STAGE)?;
    if meter.compare_rational(&cosine_half_squared, &numerical_flat_fold_limit, STAGE)?
        != Ordering::Greater
    {
        return Ok(AnalyticFiniteRadiusResult::LayerOffsetUnmodeled);
    }

    // Version-fixed numerical singularity inherited from the centered-slab
    // policy: positive thickness requires cos²(theta/2) > 2^-52.  Equality is
    // rejected.  This is independent of the analytic R <= L inequality and
    // prevents an unstable near-flat-fold capability.
    // h² <= L² cos²(theta/2) is exactly R <= L, without constructing
    // R = h / cos(theta/2).  It remains finite as theta approaches 180°.
    let finite_axis_bound =
        meter.multiply_rational(&length_squared, &cosine_half_squared, STAGE)?;
    if meter.compare_rational(&half_thickness_squared, &finite_axis_bound, STAGE)?
        == Ordering::Greater
    {
        return Ok(AnalyticFiniteRadiusResult::LayerOffsetUnmodeled);
    }

    Ok(AnalyticFiniteRadiusResult::FitsFiniteSegment(
        FiniteRadiusScalarFits {
            paper_thickness_bits: paper_thickness_mm.to_bits(),
            hinge_start: hinge.start,
            hinge_end: hinge.end,
            left_cooriented_mid_surface_normal: normals.left_local_y,
            right_cooriented_mid_surface_normal: normals.right_local_y,
        },
    ))
}

fn positive_finite_binary64(value: f64) -> bool {
    let bits = value.to_bits();
    bits >> 63 == 0 && bits & 0x7fff_ffff_ffff_ffff != 0 && value.is_finite()
}

fn prepare_point_input(
    point: &ExactPoint3,
    limits: &FiniteRadiusScalarLimits,
    work: &mut FiniteRadiusScalarWork,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<ExactPoint3>, CayleyError> {
    let Some(x) = prepare_rational_input(&point.coordinates[0], limits, work, meter)? else {
        return Ok(None);
    };
    let Some(y) = prepare_rational_input(&point.coordinates[1], limits, work, meter)? else {
        return Ok(None);
    };
    let Some(z) = prepare_rational_input(&point.coordinates[2], limits, work, meter)? else {
        return Ok(None);
    };
    Ok(Some(ExactPoint3 {
        coordinates: [x, y, z],
    }))
}

fn prepare_vector_input(
    vector: &ExactVector3,
    limits: &FiniteRadiusScalarLimits,
    work: &mut FiniteRadiusScalarWork,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<ExactVector3>, CayleyError> {
    let Some(x) = prepare_rational_input(&vector.coordinates[0], limits, work, meter)? else {
        return Ok(None);
    };
    let Some(y) = prepare_rational_input(&vector.coordinates[1], limits, work, meter)? else {
        return Ok(None);
    };
    let Some(z) = prepare_rational_input(&vector.coordinates[2], limits, work, meter)? else {
        return Ok(None);
    };
    Ok(Some(ExactVector3 {
        coordinates: [x, y, z],
    }))
}

fn prepare_rational_input(
    value: &BigRational,
    limits: &FiniteRadiusScalarLimits,
    work: &mut FiniteRadiusScalarWork,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<BigRational>, CayleyError> {
    let storage_bits = rational_storage_bits(value, STAGE)?;
    charge_input_storage(work, storage_bits, limits)?;
    meter.operation(STAGE)?;
    meter.preflight_value_bits(STAGE, rational_bits(value))?;
    if !value.denom().is_positive()
        || !meter
            .gcd_fallback(value.numer(), value.denom(), STAGE)?
            .is_one()
    {
        return Ok(None);
    }
    meter.clone_rational(value, STAGE).map(Some)
}

fn charge_input_storage(
    work: &mut FiniteRadiusScalarWork,
    storage_bits: usize,
    limits: &FiniteRadiusScalarLimits,
) -> Result<(), CayleyError> {
    if storage_bits > limits.max_input_rational_storage_bits {
        return Err(CayleyError::ResourceLimitExceeded {
            stage: STAGE,
            resource: "input_rational_storage_bits",
        });
    }
    let input_rationals =
        work.input_rationals
            .checked_add(1)
            .ok_or(CayleyError::ResourceLimitExceeded {
                stage: STAGE,
                resource: "input_rationals",
            })?;
    let total_input_storage_bits = work
        .total_input_storage_bits
        .checked_add(storage_bits)
        .ok_or(CayleyError::ResourceLimitExceeded {
            stage: STAGE,
            resource: "total_input_storage_bits",
        })?;
    if input_rationals > SCALAR_INPUT_RATIONALS
        || total_input_storage_bits > limits.max_total_input_storage_bits
    {
        return Err(CayleyError::ResourceLimitExceeded {
            stage: STAGE,
            resource: "total_input_storage_bits",
        });
    }
    work.input_rationals = input_rationals;
    work.max_input_rational_storage_bits = work.max_input_rational_storage_bits.max(storage_bits);
    work.total_input_storage_bits = total_input_storage_bits;
    Ok(())
}

fn exact_between(
    start: &ExactPoint3,
    end: &ExactPoint3,
    meter: &mut WorkMeter<'_>,
) -> Result<ExactVector3, CayleyError> {
    Ok(ExactVector3 {
        coordinates: try_array3(|axis| {
            meter.subtract_rational(&end.coordinates[axis], &start.coordinates[axis], STAGE)
        })?,
    })
}

fn exact_dot(
    left: &ExactVector3,
    right: &ExactVector3,
    meter: &mut WorkMeter<'_>,
) -> Result<BigRational, CayleyError> {
    let products = try_array3(|axis| {
        meter.multiply_rational(&left.coordinates[axis], &right.coordinates[axis], STAGE)
    })?;
    let first_two = meter.add_rational(&products[0], &products[1], STAGE)?;
    meter.add_rational(&first_two, &products[2], STAGE)
}

/// Hard limits for a private one-hinge/two-triangle prerequisite issuer.
///
/// This is deliberately a separate budget from the scalar gate.  Exhausting
/// either budget returns no analysis and therefore cannot leak a partially
/// authenticated capability.  The scalar gate is invoked exactly once, so
/// the immutable sum of both hard budgets is also checked with
/// `CayleyWork::checked_merge`; callers cannot move unused work from one
/// budget into the other.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SingleTriangularHingePrerequisiteLimits {
    max_authenticated_faces: usize,
    max_authenticated_hinges: usize,
    max_boundary_occurrences: usize,
    max_source_coordinate_lifts: usize,
    max_current_point_reconstructions: usize,
    max_rotation_authentications: usize,
    max_local_y_component_lifts: usize,
    max_rest_orientation_tests: usize,
    max_axial_vertex_tests: usize,
    max_input_rational_storage_bits: usize,
    max_total_input_storage_bits: usize,
    exact: CayleyLimits,
    scalar: FiniteRadiusScalarLimits,
}

impl Default for SingleTriangularHingePrerequisiteLimits {
    fn default() -> Self {
        let exact = CayleyLimits::default();
        Self {
            max_authenticated_faces: 2,
            max_authenticated_hinges: 1,
            max_boundary_occurrences: 6,
            max_source_coordinate_lifts: 24,
            max_current_point_reconstructions: 6,
            max_rotation_authentications: 2,
            max_local_y_component_lifts: 6,
            max_rest_orientation_tests: 2,
            max_axial_vertex_tests: 6,
            max_input_rational_storage_bits: exact.max_intermediate_bits,
            max_total_input_storage_bits: TRIANGULAR_HINGE_INPUT_RATIONALS
                * exact.max_intermediate_bits,
            exact: prerequisite_exact_hard_limits(),
            scalar: FiniteRadiusScalarLimits::default(),
        }
    }
}

impl SingleTriangularHingePrerequisiteLimits {
    fn projected(self) -> Self {
        let hard = Self::default();
        Self {
            max_authenticated_faces: self
                .max_authenticated_faces
                .min(hard.max_authenticated_faces),
            max_authenticated_hinges: self
                .max_authenticated_hinges
                .min(hard.max_authenticated_hinges),
            max_boundary_occurrences: self
                .max_boundary_occurrences
                .min(hard.max_boundary_occurrences),
            max_source_coordinate_lifts: self
                .max_source_coordinate_lifts
                .min(hard.max_source_coordinate_lifts),
            max_current_point_reconstructions: self
                .max_current_point_reconstructions
                .min(hard.max_current_point_reconstructions),
            max_rotation_authentications: self
                .max_rotation_authentications
                .min(hard.max_rotation_authentications),
            max_local_y_component_lifts: self
                .max_local_y_component_lifts
                .min(hard.max_local_y_component_lifts),
            max_rest_orientation_tests: self
                .max_rest_orientation_tests
                .min(hard.max_rest_orientation_tests),
            max_axial_vertex_tests: self.max_axial_vertex_tests.min(hard.max_axial_vertex_tests),
            max_input_rational_storage_bits: self
                .max_input_rational_storage_bits
                .min(hard.max_input_rational_storage_bits),
            max_total_input_storage_bits: self
                .max_total_input_storage_bits
                .min(hard.max_total_input_storage_bits),
            exact: project_cayley_limits(self.exact, hard.exact),
            scalar: self.scalar.projected(),
        }
    }
}

fn prerequisite_exact_hard_limits() -> CayleyLimits {
    let exact = CayleyLimits::default();
    CayleyLimits {
        max_precision_rounds: 0,
        max_guard_bits: 0,
        max_candidate_bits: 0,
        max_machin_terms_per_series: 0,
        max_trig_terms_per_series: 0,
        max_sqrt_refinements: 0,
        max_interval_operations: 4_096,
        max_shift_bits: exact.max_shift_bits,
        max_intermediate_bits: exact.max_intermediate_bits,
        max_gcd_fallback_calls: 512,
        max_gcd_fallback_input_bits: exact.max_gcd_fallback_input_bits,
        max_rational_allocations: 4_096,
        max_rational_allocation_bits: exact.max_rational_allocation_bits,
        max_total_rational_allocation_bits: 16_777_216,
        max_output_bits: 0,
    }
}

fn project_cayley_limits(requested: CayleyLimits, hard: CayleyLimits) -> CayleyLimits {
    CayleyLimits {
        max_precision_rounds: requested
            .max_precision_rounds
            .min(hard.max_precision_rounds),
        max_guard_bits: requested.max_guard_bits.min(hard.max_guard_bits),
        max_candidate_bits: requested.max_candidate_bits.min(hard.max_candidate_bits),
        max_machin_terms_per_series: requested
            .max_machin_terms_per_series
            .min(hard.max_machin_terms_per_series),
        max_trig_terms_per_series: requested
            .max_trig_terms_per_series
            .min(hard.max_trig_terms_per_series),
        max_sqrt_refinements: requested
            .max_sqrt_refinements
            .min(hard.max_sqrt_refinements),
        max_interval_operations: requested
            .max_interval_operations
            .min(hard.max_interval_operations),
        max_shift_bits: requested.max_shift_bits.min(hard.max_shift_bits),
        max_intermediate_bits: requested
            .max_intermediate_bits
            .min(hard.max_intermediate_bits),
        max_gcd_fallback_calls: requested
            .max_gcd_fallback_calls
            .min(hard.max_gcd_fallback_calls),
        max_gcd_fallback_input_bits: requested
            .max_gcd_fallback_input_bits
            .min(hard.max_gcd_fallback_input_bits),
        max_rational_allocations: requested
            .max_rational_allocations
            .min(hard.max_rational_allocations),
        max_rational_allocation_bits: requested
            .max_rational_allocation_bits
            .min(hard.max_rational_allocation_bits),
        max_total_rational_allocation_bits: requested
            .max_total_rational_allocation_bits
            .min(hard.max_total_rational_allocation_bits),
        max_output_bits: requested.max_output_bits.min(hard.max_output_bits),
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct SingleTriangularHingePrerequisiteWork {
    authenticated_faces: usize,
    authenticated_hinges: usize,
    boundary_occurrences: usize,
    source_coordinate_lifts: usize,
    current_point_reconstructions: usize,
    rotation_authentications: usize,
    local_y_component_lifts: usize,
    rest_orientation_tests: usize,
    axial_vertex_tests: usize,
    input_rationals: usize,
    max_input_rational_storage_bits: usize,
    total_input_storage_bits: usize,
    exact: CayleyWork,
    scalar: FiniteRadiusScalarWork,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SingleTriangularHingePrerequisiteError {
    ResourceLimitExceeded,
}

/// Non-forgeable, borrow-bound prerequisites for one exact triangular hinge.
///
/// The token intentionally retains only the authenticated issuer, indexes
/// into that issuer, and the bit-exact thickness input used by the scalar
/// proof.  It neither owns copied geometry nor carries a policy decision, and
/// it deliberately implements neither `Clone`, `Copy`, nor any persistence
/// trait.
#[derive(Debug)]
struct AuthenticatedSingleTriangularHingePrerequisitesV1<'exact, 'pose> {
    exact: &'exact RationalCayleyTreePose<'pose>,
    paper_thickness_bits: u64,
    left_face_index: usize,
    right_face_index: usize,
    hinge_index: usize,
}

/// A private consumer-side view that can only be minted after rebinding the
/// exact issuer, authenticated indexes, and the bit-exact thickness input.
#[derive(Debug)]
struct RevalidatedSingleTriangularHingePrerequisitesV1<'capability, 'exact, 'pose> {
    capability: &'capability AuthenticatedSingleTriangularHingePrerequisitesV1<'exact, 'pose>,
}

#[derive(Debug)]
enum SingleTriangularHingePrerequisiteResult<'exact, 'pose> {
    Authenticated(AuthenticatedSingleTriangularHingePrerequisitesV1<'exact, 'pose>),
    LayerOffsetUnmodeled,
    Unresolved,
}

#[derive(Debug)]
struct SingleTriangularHingePrerequisiteAnalysis<'exact, 'pose> {
    result: SingleTriangularHingePrerequisiteResult<'exact, 'pose>,
    work: SingleTriangularHingePrerequisiteWork,
}

#[derive(Debug, Clone, Copy)]
struct AuthenticatedTriangleIndexes {
    left_face_index: usize,
    right_face_index: usize,
    hinge_index: usize,
    left_hinge_occurrence: usize,
    right_hinge_occurrence: usize,
}

/// Authenticates only the finite-hinge prerequisites that can be proved from
/// the native exact pose today.
///
/// This remains private and is not connected to collision classification,
/// the production safe set, or the UI.  In particular, it does not prove that
/// a complete prism intersection is contained in the finite corridor.
fn analyze_single_triangular_hinge_prerequisites_v1<'exact, 'pose>(
    exact: &'exact RationalCayleyTreePose<'pose>,
    paper_thickness_mm: f64,
    limits: SingleTriangularHingePrerequisiteLimits,
) -> Result<
    SingleTriangularHingePrerequisiteAnalysis<'exact, 'pose>,
    SingleTriangularHingePrerequisiteError,
> {
    let limits = limits.projected();
    let mut work = SingleTriangularHingePrerequisiteWork::default();
    let mut meter = WorkMeter::new(&limits.exact);
    let result = calculate_single_triangular_hinge_prerequisites_v1(
        exact,
        paper_thickness_mm,
        &limits,
        &mut work,
        &mut meter,
    );
    work.exact = meter.work;
    match result {
        Ok(result) => Ok(SingleTriangularHingePrerequisiteAnalysis { result, work }),
        Err(CayleyError::ResourceLimitExceeded { .. }) => {
            Err(SingleTriangularHingePrerequisiteError::ResourceLimitExceeded)
        }
        Err(_) => Ok(SingleTriangularHingePrerequisiteAnalysis {
            result: SingleTriangularHingePrerequisiteResult::Unresolved,
            work,
        }),
    }
}

fn calculate_single_triangular_hinge_prerequisites_v1<'exact, 'pose>(
    exact: &'exact RationalCayleyTreePose<'pose>,
    paper_thickness_mm: f64,
    limits: &SingleTriangularHingePrerequisiteLimits,
    work: &mut SingleTriangularHingePrerequisiteWork,
    meter: &mut WorkMeter<'_>,
) -> Result<SingleTriangularHingePrerequisiteResult<'exact, 'pose>, CayleyError> {
    if !positive_finite_binary64(paper_thickness_mm)
        || exact.version != RATIONAL_CAYLEY_TREE_POSE_V1
    {
        return Ok(SingleTriangularHingePrerequisiteResult::Unresolved);
    }

    let bound = exact.bound;
    let model = bound.model();
    let pose = bound.pose();
    let rebound = match model.bind_pose(pose) {
        Ok(rebound) => rebound,
        Err(_) => return Ok(SingleTriangularHingePrerequisiteResult::Unresolved),
    };
    if !exact.is_for(rebound)
        || model.model_id() != ori_kinematics::MATERIAL_TREE_KINEMATICS_MODEL_ID
        || model.face_ids() != pose.face_ids()
        || model.hinges() != pose.hinges()
        || exact.fixed_face != pose.fixed_face()
        || model.face_ids().len() != 2
        || model.hinges().len() != 1
        || exact.faces.len() != 2
        || exact.hinges.len() != 1
        || pose.hinge_angles().len() != 1
        || exact.faces.iter().any(|face| {
            face.boundary.len() != 3
                || bound.face_boundary(face.face).is_none_or(|boundary| {
                    boundary.vertices().len() != 3 || boundary.edges().len() != 3
                })
        })
    {
        return Ok(SingleTriangularHingePrerequisiteResult::Unresolved);
    }
    charge_fixed_prerequisite_work(work, limits)?;

    if exact
        .faces
        .iter()
        .zip(model.face_ids())
        .any(|(candidate, expected)| candidate.face != *expected)
    {
        return Ok(SingleTriangularHingePrerequisiteResult::Unresolved);
    }

    let source_hinge = &model.hinges()[0];
    let exact_hinge = &exact.hinges[0];
    let pose_angle = &pose.hinge_angles()[0];
    let base_rotation_sign = match source_hinge.assignment() {
        FoldAssignment::Mountain => 1_i8,
        FoldAssignment::Valley => -1_i8,
    };
    let expected_rotation_sign = if exact_hinge.parent == source_hinge.left_face()
        && exact_hinge.child == source_hinge.right_face()
    {
        Some(base_rotation_sign)
    } else if exact_hinge.parent == source_hinge.right_face()
        && exact_hinge.child == source_hinge.left_face()
    {
        Some(-base_rotation_sign)
    } else {
        None
    };
    if exact_hinge.edge != source_hinge.edge()
        || pose_angle.edge() != source_hinge.edge()
        || exact_hinge.angle_magnitude_bits != pose_angle.angle_degrees().to_bits()
        || expected_rotation_sign != Some(exact_hinge.rotation_sign)
        || exact_hinge.endpoint_vertices[0] == exact_hinge.endpoint_vertices[1]
    {
        return Ok(SingleTriangularHingePrerequisiteResult::Unresolved);
    }

    validate_exact_pose_rational_inputs(exact, limits, work, meter)?;
    let Some(indexes) = authenticate_triangular_hinge_indexes(exact, source_hinge) else {
        return Ok(SingleTriangularHingePrerequisiteResult::Unresolved);
    };

    let rest_faces_by_index = [
        reauthenticate_exact_face_rest(bound, &exact.faces[0], meter)?,
        reauthenticate_exact_face_rest(bound, &exact.faces[1], meter)?,
    ];

    let rest_start = exact_point_at_stage(point3_array(source_hinge.start()), meter)?;
    let rest_end = exact_point_at_stage(point3_array(source_hinge.end()), meter)?;
    let left_face = &exact.faces[indexes.left_face_index];
    let right_face = &exact.faces[indexes.right_face_index];
    let left_rest = &rest_faces_by_index[indexes.left_face_index];
    let right_rest = &rest_faces_by_index[indexes.right_face_index];
    let start_vertex = exact_hinge.endpoint_vertices[0];
    let end_vertex = exact_hinge.endpoint_vertices[1];
    let left_pair = cyclic_vertex_pair(left_face, indexes.left_hinge_occurrence);
    let right_pair = cyclic_vertex_pair(right_face, indexes.right_hinge_occurrence);
    if left_pair != [start_vertex, end_vertex]
        || right_pair != [end_vertex, start_vertex]
        || !rest_vertex_matches(left_face, left_rest, start_vertex, &rest_start)
        || !rest_vertex_matches(left_face, left_rest, end_vertex, &rest_end)
        || !rest_vertex_matches(right_face, right_rest, start_vertex, &rest_start)
        || !rest_vertex_matches(right_face, right_rest, end_vertex, &rest_end)
    {
        return Ok(SingleTriangularHingePrerequisiteResult::Unresolved);
    }

    for (endpoint_index, vertex) in [start_vertex, end_vertex].into_iter().enumerate() {
        let Some(left_current) = current_vertex(left_face, vertex) else {
            return Ok(SingleTriangularHingePrerequisiteResult::Unresolved);
        };
        let Some(right_current) = current_vertex(right_face, vertex) else {
            return Ok(SingleTriangularHingePrerequisiteResult::Unresolved);
        };
        if !canonical_point_eq(left_current, &exact_hinge.world_endpoints[endpoint_index])
            || !canonical_point_eq(right_current, &exact_hinge.world_endpoints[endpoint_index])
        {
            return Ok(SingleTriangularHingePrerequisiteResult::Unresolved);
        }
    }

    let left_opposite = (indexes.left_hinge_occurrence + 2) % 3;
    let right_opposite = (indexes.right_hinge_occurrence + 2) % 3;
    let left_orientation = exact_rest_support_orientation_xz(
        &rest_start,
        &rest_end,
        &left_rest[left_opposite],
        meter,
    )?;
    let right_orientation = exact_rest_support_orientation_xz(
        &rest_start,
        &rest_end,
        &right_rest[right_opposite],
        meter,
    )?;
    let zero = BigRational::zero();
    let left_order = meter.compare_rational(&left_orientation, &zero, STAGE)?;
    let right_order = meter.compare_rational(&right_orientation, &zero, STAGE)?;
    if left_order == Ordering::Equal || right_order == Ordering::Equal || left_order == right_order
    {
        return Ok(SingleTriangularHingePrerequisiteResult::Unresolved);
    }

    let axis = exact_between(
        &exact_hinge.world_endpoints[0],
        &exact_hinge.world_endpoints[1],
        meter,
    )?;
    let length_squared = exact_dot(&axis, &axis, meter)?;
    let left_axially_bounded = all_triangle_vertices_within_axis(
        left_face,
        &exact_hinge.world_endpoints[0],
        &axis,
        &length_squared,
        meter,
    )?;
    let right_axially_bounded = all_triangle_vertices_within_axis(
        right_face,
        &exact_hinge.world_endpoints[0],
        &axis,
        &length_squared,
        meter,
    )?;
    if !left_axially_bounded && !right_axially_bounded {
        return Ok(SingleTriangularHingePrerequisiteResult::Unresolved);
    }

    let left_normal = exact_local_y(&left_face.transform, meter)?;
    let right_normal = exact_local_y(&right_face.transform, meter)?;
    let scalar = analyze_finite_radius_scalar(
        paper_thickness_mm,
        ExactHingeAxis {
            start: &exact_hinge.world_endpoints[0],
            end: &exact_hinge.world_endpoints[1],
        },
        CoorientedMidSurfaceNormals {
            left_local_y: &left_normal,
            right_local_y: &right_normal,
        },
        limits.scalar,
    )
    .map_err(|FiniteRadiusScalarError::ResourceLimitExceeded| {
        CayleyError::ResourceLimitExceeded {
            stage: STAGE,
            resource: "finite_radius_scalar",
        }
    })?;
    work.scalar = scalar.work;
    check_combined_exact_hard_cap(&meter.work, &work.scalar.exact)?;
    match scalar.result {
        AnalyticFiniteRadiusResult::FitsFiniteSegment(_) => {}
        AnalyticFiniteRadiusResult::LayerOffsetUnmodeled => {
            return Ok(SingleTriangularHingePrerequisiteResult::LayerOffsetUnmodeled);
        }
        AnalyticFiniteRadiusResult::Unresolved => {
            return Ok(SingleTriangularHingePrerequisiteResult::Unresolved);
        }
    }

    Ok(SingleTriangularHingePrerequisiteResult::Authenticated(
        AuthenticatedSingleTriangularHingePrerequisitesV1 {
            exact,
            paper_thickness_bits: paper_thickness_mm.to_bits(),
            left_face_index: indexes.left_face_index,
            right_face_index: indexes.right_face_index,
            hinge_index: indexes.hinge_index,
        },
    ))
}

fn revalidate_single_triangular_hinge_prerequisites_v1<'capability, 'exact, 'pose>(
    capability: &'capability AuthenticatedSingleTriangularHingePrerequisitesV1<'exact, 'pose>,
    exact: &'exact RationalCayleyTreePose<'pose>,
    paper_thickness_mm: f64,
) -> Option<RevalidatedSingleTriangularHingePrerequisitesV1<'capability, 'exact, 'pose>> {
    if !positive_finite_binary64(paper_thickness_mm)
        || capability.paper_thickness_bits != paper_thickness_mm.to_bits()
        || !std::ptr::eq(capability.exact, exact)
        || capability.left_face_index == capability.right_face_index
    {
        return None;
    }
    let left = exact.faces.get(capability.left_face_index)?;
    let right = exact.faces.get(capability.right_face_index)?;
    let exact_hinge = exact.hinges.get(capability.hinge_index)?;
    let source_hinge = exact.bound.model().hinges().get(capability.hinge_index)?;
    if exact.version != RATIONAL_CAYLEY_TREE_POSE_V1
        || exact_hinge.edge != source_hinge.edge()
        || left.face != source_hinge.left_face()
        || right.face != source_hinge.right_face()
        || exact_hinge.endpoint_vertices[0] == exact_hinge.endpoint_vertices[1]
    {
        return None;
    }
    Some(RevalidatedSingleTriangularHingePrerequisitesV1 { capability })
}

fn check_combined_exact_hard_cap(
    geometry: &CayleyWork,
    scalar: &CayleyWork,
) -> Result<(), CayleyError> {
    let geometry_hard = prerequisite_exact_hard_limits();
    let scalar_hard = FiniteRadiusScalarLimits::default().exact();
    let combined = CayleyLimits {
        max_precision_rounds: 0,
        max_guard_bits: 0,
        max_candidate_bits: 0,
        max_machin_terms_per_series: geometry_hard
            .max_machin_terms_per_series
            .max(scalar_hard.max_machin_terms_per_series),
        max_trig_terms_per_series: geometry_hard
            .max_trig_terms_per_series
            .max(scalar_hard.max_trig_terms_per_series),
        max_sqrt_refinements: geometry_hard
            .max_sqrt_refinements
            .max(scalar_hard.max_sqrt_refinements),
        max_interval_operations: checked_hard_limit_sum(
            geometry_hard.max_interval_operations,
            scalar_hard.max_interval_operations,
            "combined_interval_operations",
        )?,
        max_shift_bits: geometry_hard.max_shift_bits.max(scalar_hard.max_shift_bits),
        max_intermediate_bits: geometry_hard
            .max_intermediate_bits
            .max(scalar_hard.max_intermediate_bits),
        max_gcd_fallback_calls: checked_hard_limit_sum(
            geometry_hard.max_gcd_fallback_calls,
            scalar_hard.max_gcd_fallback_calls,
            "combined_gcd_fallback_calls",
        )?,
        max_gcd_fallback_input_bits: checked_hard_limit_sum(
            geometry_hard.max_gcd_fallback_input_bits,
            scalar_hard.max_gcd_fallback_input_bits,
            "combined_gcd_fallback_input_bits",
        )?,
        max_rational_allocations: checked_hard_limit_sum(
            geometry_hard.max_rational_allocations,
            scalar_hard.max_rational_allocations,
            "combined_rational_allocations",
        )?,
        max_rational_allocation_bits: geometry_hard
            .max_rational_allocation_bits
            .max(scalar_hard.max_rational_allocation_bits),
        max_total_rational_allocation_bits: checked_hard_limit_sum(
            geometry_hard.max_total_rational_allocation_bits,
            scalar_hard.max_total_rational_allocation_bits,
            "combined_total_rational_allocation_bits",
        )?,
        max_output_bits: geometry_hard
            .max_output_bits
            .max(scalar_hard.max_output_bits),
    };
    geometry
        .checked_merge(scalar, &combined, None, STAGE)
        .map(|_| ())
}

fn checked_hard_limit_sum(
    left: usize,
    right: usize,
    resource: &'static str,
) -> Result<usize, CayleyError> {
    left.checked_add(right)
        .ok_or(CayleyError::ResourceLimitExceeded {
            stage: STAGE,
            resource,
        })
}

fn charge_fixed_prerequisite_work(
    work: &mut SingleTriangularHingePrerequisiteWork,
    limits: &SingleTriangularHingePrerequisiteLimits,
) -> Result<(), CayleyError> {
    for (required, maximum, resource) in [
        (2, limits.max_authenticated_faces, "authenticated_faces"),
        (1, limits.max_authenticated_hinges, "authenticated_hinges"),
        (6, limits.max_boundary_occurrences, "boundary_occurrences"),
        (
            24,
            limits.max_source_coordinate_lifts,
            "source_coordinate_lifts",
        ),
        (
            6,
            limits.max_current_point_reconstructions,
            "current_point_reconstructions",
        ),
        (
            2,
            limits.max_rotation_authentications,
            "rotation_authentications",
        ),
        (
            6,
            limits.max_local_y_component_lifts,
            "local_y_component_lifts",
        ),
        (
            2,
            limits.max_rest_orientation_tests,
            "rest_orientation_tests",
        ),
        (6, limits.max_axial_vertex_tests, "axial_vertex_tests"),
    ] {
        if required > maximum {
            return Err(CayleyError::ResourceLimitExceeded {
                stage: STAGE,
                resource,
            });
        }
    }
    work.authenticated_faces = 2;
    work.authenticated_hinges = 1;
    work.boundary_occurrences = 6;
    work.source_coordinate_lifts = 24;
    work.current_point_reconstructions = 6;
    work.rotation_authentications = 2;
    work.local_y_component_lifts = 6;
    work.rest_orientation_tests = 2;
    work.axial_vertex_tests = 6;
    Ok(())
}

fn validate_exact_pose_rational_inputs(
    exact: &RationalCayleyTreePose<'_>,
    limits: &SingleTriangularHingePrerequisiteLimits,
    work: &mut SingleTriangularHingePrerequisiteWork,
    meter: &mut WorkMeter<'_>,
) -> Result<(), CayleyError> {
    for face in &exact.faces {
        for value in face
            .transform
            .rotation
            .iter()
            .flatten()
            .chain(&face.transform.translation.coordinates)
            .chain(
                face.boundary
                    .iter()
                    .flat_map(|(_, point)| &point.coordinates),
            )
        {
            validate_prerequisite_rational_input(value, limits, work, meter)?;
        }
    }
    for value in exact.hinges[0]
        .world_endpoints
        .iter()
        .flat_map(|point| &point.coordinates)
    {
        validate_prerequisite_rational_input(value, limits, work, meter)?;
    }
    if work.input_rationals != TRIANGULAR_HINGE_INPUT_RATIONALS {
        return Err(CayleyError::InvariantFailure { stage: STAGE });
    }
    Ok(())
}

fn reauthenticate_exact_face_rest(
    bound: ori_kinematics::BoundMaterialTreePose<'_>,
    exact_face: &ExactFacePose,
    meter: &mut WorkMeter<'_>,
) -> Result<[ExactPoint3; 3], CayleyError> {
    let model = bound.model();
    let pose = bound.pose();
    let boundary = bound
        .face_boundary(exact_face.face)
        .filter(|boundary| {
            boundary.face() == exact_face.face
                && model.owns_face_boundary(*boundary)
                && pose.owns_face_boundary(*boundary)
                && boundary.vertices().len() == 3
                && boundary.edges().len() == 3
                && exact_face.boundary.len() == 3
        })
        .ok_or(CayleyError::BoundTreeInconsistent { stage: STAGE })?;
    if exact_face
        .boundary
        .iter()
        .zip(boundary.vertices())
        .any(|((vertex, _), expected)| vertex != expected)
    {
        return Err(CayleyError::BoundTreeInconsistent { stage: STAGE });
    }

    verify_exact_rotation(&exact_face.transform.rotation, meter)?;
    try_array3(|vertex_index| {
        let vertex = boundary.vertices()[vertex_index];
        let source = model
            .vertex_position(vertex)
            .ok_or(CayleyError::BoundTreeInconsistent { stage: STAGE })?;
        let rest = exact_point_at_stage(point3_array(source), meter)?;
        let reconstructed = apply_exact_transform(&exact_face.transform, &rest, meter)?;
        if !canonical_point_eq(&reconstructed, &exact_face.boundary[vertex_index].1) {
            return Err(CayleyError::BoundTreeInconsistent { stage: STAGE });
        }
        Ok(rest)
    })
}

fn validate_prerequisite_rational_input(
    value: &BigRational,
    limits: &SingleTriangularHingePrerequisiteLimits,
    work: &mut SingleTriangularHingePrerequisiteWork,
    meter: &mut WorkMeter<'_>,
) -> Result<(), CayleyError> {
    let storage_bits = rational_storage_bits(value, STAGE)?;
    let next_count =
        work.input_rationals
            .checked_add(1)
            .ok_or(CayleyError::ResourceLimitExceeded {
                stage: STAGE,
                resource: "input_rationals",
            })?;
    let next_total = work
        .total_input_storage_bits
        .checked_add(storage_bits)
        .ok_or(CayleyError::ResourceLimitExceeded {
            stage: STAGE,
            resource: "total_input_storage_bits",
        })?;
    if next_count > TRIANGULAR_HINGE_INPUT_RATIONALS
        || storage_bits > limits.max_input_rational_storage_bits
        || next_total > limits.max_total_input_storage_bits
    {
        return Err(CayleyError::ResourceLimitExceeded {
            stage: STAGE,
            resource: if storage_bits > limits.max_input_rational_storage_bits {
                "input_rational_storage_bits"
            } else {
                "total_input_storage_bits"
            },
        });
    }
    meter.operation(STAGE)?;
    meter.preflight_value_bits(STAGE, rational_bits(value))?;
    if !value.denom().is_positive()
        || !meter
            .gcd_fallback(value.numer(), value.denom(), STAGE)?
            .is_one()
    {
        return Err(CayleyError::BoundTreeInconsistent { stage: STAGE });
    }
    work.input_rationals = next_count;
    work.max_input_rational_storage_bits = work.max_input_rational_storage_bits.max(storage_bits);
    work.total_input_storage_bits = next_total;
    Ok(())
}

fn authenticate_triangular_hinge_indexes(
    exact: &RationalCayleyTreePose<'_>,
    source_hinge: &ori_kinematics::TreeHinge,
) -> Option<AuthenticatedTriangleIndexes> {
    let left_face_index = unique_exact_face_index(exact, source_hinge.left_face())?;
    let right_face_index = unique_exact_face_index(exact, source_hinge.right_face())?;
    if left_face_index == right_face_index {
        return None;
    }
    let left_boundary = exact.bound.face_boundary(source_hinge.left_face())?;
    let right_boundary = exact.bound.face_boundary(source_hinge.right_face())?;
    let left_hinge_occurrence = unique_edge_occurrence(left_boundary.edges(), source_hinge.edge())?;
    let right_hinge_occurrence =
        unique_edge_occurrence(right_boundary.edges(), source_hinge.edge())?;
    let total_occurrences = [left_boundary.edges(), right_boundary.edges()]
        .into_iter()
        .flatten()
        .filter(|edge| **edge == source_hinge.edge())
        .count();
    if total_occurrences != 2 {
        return None;
    }
    Some(AuthenticatedTriangleIndexes {
        left_face_index,
        right_face_index,
        hinge_index: 0,
        left_hinge_occurrence,
        right_hinge_occurrence,
    })
}

fn unique_exact_face_index(exact: &RationalCayleyTreePose<'_>, face: FaceId) -> Option<usize> {
    let mut matches = exact
        .faces
        .iter()
        .enumerate()
        .filter(|(_, candidate)| candidate.face == face);
    let (index, _) = matches.next()?;
    matches.next().is_none().then_some(index)
}

fn unique_edge_occurrence(edges: &[ori_domain::EdgeId], edge: ori_domain::EdgeId) -> Option<usize> {
    let mut matches = edges
        .iter()
        .enumerate()
        .filter(|(_, candidate)| **candidate == edge);
    let (index, _) = matches.next()?;
    matches.next().is_none().then_some(index)
}

fn cyclic_vertex_pair(face: &ExactFacePose, occurrence: usize) -> [VertexId; 2] {
    [
        face.boundary[occurrence].0,
        face.boundary[(occurrence + 1) % 3].0,
    ]
}

fn rest_vertex_matches(
    face: &ExactFacePose,
    rest: &[ExactPoint3; 3],
    vertex: VertexId,
    expected: &ExactPoint3,
) -> bool {
    face.boundary
        .iter()
        .position(|(candidate, _)| *candidate == vertex)
        .is_some_and(|index| canonical_point_eq(&rest[index], expected))
}

fn current_vertex(face: &ExactFacePose, vertex: VertexId) -> Option<&ExactPoint3> {
    let mut matches = face
        .boundary
        .iter()
        .filter(|(candidate, _)| *candidate == vertex);
    let (_, point) = matches.next()?;
    matches.next().is_none().then_some(point)
}

fn exact_point_at_stage(
    values: [f64; 3],
    meter: &mut WorkMeter<'_>,
) -> Result<ExactPoint3, CayleyError> {
    Ok(ExactPoint3 {
        coordinates: [
            exact_f64(values[0], meter, STAGE)?,
            exact_f64(values[1], meter, STAGE)?,
            exact_f64(values[2], meter, STAGE)?,
        ],
    })
}

fn exact_rest_support_orientation_xz(
    start: &ExactPoint3,
    end: &ExactPoint3,
    point: &ExactPoint3,
    meter: &mut WorkMeter<'_>,
) -> Result<BigRational, CayleyError> {
    let edge_x = meter.subtract_rational(&end.coordinates[0], &start.coordinates[0], STAGE)?;
    let edge_z = meter.subtract_rational(&end.coordinates[2], &start.coordinates[2], STAGE)?;
    let point_x = meter.subtract_rational(&point.coordinates[0], &start.coordinates[0], STAGE)?;
    let point_z = meter.subtract_rational(&point.coordinates[2], &start.coordinates[2], STAGE)?;
    let first = meter.multiply_rational(&edge_x, &point_z, STAGE)?;
    let second = meter.multiply_rational(&edge_z, &point_x, STAGE)?;
    meter.subtract_rational(&first, &second, STAGE)
}

fn exact_local_y(
    transform: &ExactRigidTransform,
    meter: &mut WorkMeter<'_>,
) -> Result<ExactVector3, CayleyError> {
    Ok(ExactVector3 {
        coordinates: try_array3(|row| meter.clone_rational(&transform.rotation[row][1], STAGE))?,
    })
}

fn all_triangle_vertices_within_axis(
    face: &ExactFacePose,
    start: &ExactPoint3,
    axis: &ExactVector3,
    length_squared: &BigRational,
    meter: &mut WorkMeter<'_>,
) -> Result<bool, CayleyError> {
    let zero = BigRational::zero();
    let mut all_inside = true;
    for (_, point) in &face.boundary {
        let offset = exact_between(start, point, meter)?;
        let projection = exact_dot(&offset, axis, meter)?;
        let lower = meter.compare_rational(&projection, &zero, STAGE)?;
        let upper = meter.compare_rational(&projection, length_squared, STAGE)?;
        all_inside &= lower != Ordering::Less && upper != Ordering::Greater;
    }
    Ok(all_inside)
}

#[cfg(test)]
mod tests {
    use ori_domain::{
        CreasePattern, Edge, EdgeId, EdgeKind, Paper, Point2, ProjectId, Vertex, VertexId,
    };
    use ori_kinematics::{
        CanonicalHingeAngles, HingeAngle, MaterialTreeKinematicsModel, MaterialTreePose,
        TreeKinematicsLimits,
    };
    use ori_topology::{FaceExtractionInput, analyze_faces};

    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum ResultKind {
        Fits,
        LayerOffsetUnmodeled,
        Unresolved,
    }

    fn integer(value: i64) -> BigRational {
        BigRational::from_integer(BigInt::from(value))
    }

    fn fraction(numerator: i64, denominator: i64) -> BigRational {
        BigRational::new(BigInt::from(numerator), BigInt::from(denominator))
    }

    fn point(x: BigRational, y: BigRational, z: BigRational) -> ExactPoint3 {
        ExactPoint3 {
            coordinates: [x, y, z],
        }
    }

    fn vector(x: BigRational, y: BigRational, z: BigRational) -> ExactVector3 {
        ExactVector3 {
            coordinates: [x, y, z],
        }
    }

    fn origin() -> ExactPoint3 {
        point(integer(0), integer(0), integer(0))
    }

    fn z_axis(length: BigRational) -> ExactPoint3 {
        point(integer(0), integer(0), length)
    }

    fn local_y() -> ExactVector3 {
        vector(integer(0), integer(1), integer(0))
    }

    fn rational_unit_normal_from_half_tangent(
        numerator: i64,
        denominator: i64,
        mountain_sign: i64,
    ) -> ExactVector3 {
        let p = BigInt::from(numerator);
        let q = BigInt::from(denominator);
        let p_squared = &p * &p;
        let q_squared = &q * &q;
        let sum = &p_squared + &q_squared;
        let sine = BigRational::new(BigInt::from(2) * &p * &q * mountain_sign, sum.clone());
        let cosine = BigRational::new(q_squared - p_squared, sum);
        vector(sine, cosine, integer(0))
    }

    fn classify<'a>(
        thickness: f64,
        start: &'a ExactPoint3,
        end: &'a ExactPoint3,
        left: &'a ExactVector3,
        right: &'a ExactVector3,
        limits: FiniteRadiusScalarLimits,
    ) -> Result<FiniteRadiusScalarAnalysis<'a>, FiniteRadiusScalarError> {
        analyze_finite_radius_scalar(
            thickness,
            ExactHingeAxis { start, end },
            CoorientedMidSurfaceNormals {
                left_local_y: left,
                right_local_y: right,
            },
            limits,
        )
    }

    fn triangular_vertex_id(index: u64) -> VertexId {
        serde_json::from_str(&format!("\"00000000-0000-4000-8100-{index:012x}\""))
            .expect("fixed triangular vertex id")
    }

    fn triangular_edge_id(index: u64) -> EdgeId {
        serde_json::from_str(&format!("\"00000000-0000-4000-9100-{index:012x}\""))
            .expect("fixed triangular edge id")
    }

    fn triangular_project_id(index: u64) -> ProjectId {
        serde_json::from_str(&format!("\"00000000-0000-4000-b100-{index:012x}\""))
            .expect("fixed triangular project id")
    }

    fn triangular_vertex(index: u64, x: f64, y: f64) -> Vertex {
        Vertex {
            id: triangular_vertex_id(index),
            position: Point2::new(x, y),
        }
    }

    fn triangular_edge(index: u64, start: VertexId, end: VertexId, kind: EdgeKind) -> Edge {
        Edge {
            id: triangular_edge_id(index),
            start,
            end,
            kind,
        }
    }

    fn two_triangle_model_with_options(
        assignment: EdgeKind,
        reordered_sources: bool,
        coordinates: [(f64, f64); 4],
        namespace_index: u64,
    ) -> MaterialTreeKinematicsModel {
        assert!(matches!(assignment, EdgeKind::Mountain | EdgeKind::Valley));
        let mut vertices = coordinates
            .into_iter()
            .enumerate()
            .map(|(index, (x, y))| triangular_vertex(index as u64 + 1, x, y))
            .collect::<Vec<_>>();
        let mut boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
        let mut edges = (0..4)
            .map(|index| {
                triangular_edge(
                    index as u64 + 1,
                    boundary[index],
                    boundary[(index + 1) % 4],
                    EdgeKind::Boundary,
                )
            })
            .collect::<Vec<_>>();
        edges.push(triangular_edge(5, boundary[0], boundary[2], assignment));
        if reordered_sources {
            vertices.reverse();
            edges.reverse();
            boundary.rotate_left(2);
        }
        let pattern = CreasePattern { vertices, edges };
        let paper = Paper {
            boundary_vertices: boundary,
            ..Paper::default()
        };
        let report = analyze_faces(FaceExtractionInput {
            identity_namespace: triangular_project_id(namespace_index),
            source_revision: 700 + namespace_index,
            paper: &paper,
            pattern: &pattern,
        });
        assert!(report.issues.is_empty(), "{:?}", report.issues);
        MaterialTreeKinematicsModel::prepare(
            &pattern,
            &paper,
            &report.snapshot.expect("two triangular faces"),
            TreeKinematicsLimits::default(),
        )
        .expect("two-triangle material model")
    }

    fn two_triangle_model() -> MaterialTreeKinematicsModel {
        two_triangle_model_with_options(
            EdgeKind::Mountain,
            false,
            [(0.0, 0.0), (400.0, 0.0), (400.0, 400.0), (0.0, 400.0)],
            1,
        )
    }

    fn unsupported_polygon_model(
        coordinates: &[(f64, f64)],
        crease_endpoint_indexes: &[(usize, usize, EdgeKind)],
        namespace_index: u64,
    ) -> MaterialTreeKinematicsModel {
        let vertices = coordinates
            .iter()
            .enumerate()
            .map(|(index, (x, y))| triangular_vertex(index as u64 + 1, *x, *y))
            .collect::<Vec<_>>();
        let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
        let mut edges = (0..boundary.len())
            .map(|index| {
                triangular_edge(
                    index as u64 + 20,
                    boundary[index],
                    boundary[(index + 1) % boundary.len()],
                    EdgeKind::Boundary,
                )
            })
            .collect::<Vec<_>>();
        for (crease_index, (start, end, assignment)) in crease_endpoint_indexes.iter().enumerate() {
            edges.push(triangular_edge(
                crease_index as u64 + 100,
                boundary[*start],
                boundary[*end],
                *assignment,
            ));
        }
        let pattern = CreasePattern { vertices, edges };
        let paper = Paper {
            boundary_vertices: boundary,
            ..Paper::default()
        };
        let report = analyze_faces(FaceExtractionInput {
            identity_namespace: triangular_project_id(namespace_index),
            source_revision: 800 + namespace_index,
            paper: &paper,
            pattern: &pattern,
        });
        assert!(report.issues.is_empty(), "{:?}", report.issues);
        MaterialTreeKinematicsModel::prepare(
            &pattern,
            &paper,
            &report.snapshot.expect("unsupported-shape topology"),
            TreeKinematicsLimits::default(),
        )
        .expect("unsupported-shape material model")
    }

    fn triangular_pose(
        model: &MaterialTreeKinematicsModel,
        angle_degrees: f64,
    ) -> MaterialTreePose {
        triangular_pose_with_root(model, angle_degrees, model.face_ids()[0])
    }

    fn triangular_pose_with_root(
        model: &MaterialTreeKinematicsModel,
        angle_degrees: f64,
        root: FaceId,
    ) -> MaterialTreePose {
        let hinge = &model.hinges()[0];
        let angles = CanonicalHingeAngles::new(vec![
            HingeAngle::new(hinge.edge(), angle_degrees).expect("finite test angle"),
        ])
        .expect("canonical one-hinge angles");
        model
            .solve(Some(root), &angles)
            .expect("non-planar triangular pose")
    }

    fn uniform_pose(model: &MaterialTreeKinematicsModel, angle_degrees: f64) -> MaterialTreePose {
        let angles = CanonicalHingeAngles::new(
            model
                .hinges()
                .iter()
                .map(|hinge| HingeAngle::new(hinge.edge(), angle_degrees).unwrap())
                .collect(),
        )
        .expect("canonical uniform angles");
        let fixed_face = (!model.hinges().is_empty()).then_some(model.face_ids()[0]);
        model
            .solve(fixed_face, &angles)
            .expect("uniform native material pose")
    }

    fn triangular_exact_pose<'a>(
        model: &'a MaterialTreeKinematicsModel,
        pose: &'a MaterialTreePose,
    ) -> RationalCayleyTreePose<'a> {
        super::super::prepare_rational_cayley_tree_pose_v1(
            model.bind_pose(pose).expect("issuer-bound triangular pose"),
            super::super::ExactTreePoseLimits::default(),
        )
        .expect("exact triangular pose")
    }

    fn prerequisite_kind(
        analysis: &SingleTriangularHingePrerequisiteAnalysis<'_, '_>,
    ) -> ResultKind {
        match analysis.result {
            SingleTriangularHingePrerequisiteResult::Authenticated(_) => ResultKind::Fits,
            SingleTriangularHingePrerequisiteResult::LayerOffsetUnmodeled => {
                ResultKind::LayerOffsetUnmodeled
            }
            SingleTriangularHingePrerequisiteResult::Unresolved => ResultKind::Unresolved,
        }
    }

    fn kind(analysis: &FiniteRadiusScalarAnalysis<'_>) -> ResultKind {
        match &analysis.result {
            AnalyticFiniteRadiusResult::FitsFiniteSegment(_) => ResultKind::Fits,
            AnalyticFiniteRadiusResult::LayerOffsetUnmodeled => ResultKind::LayerOffsetUnmodeled,
            AnalyticFiniteRadiusResult::Unresolved => ResultKind::Unresolved,
        }
    }

    fn next_up(value: f64) -> f64 {
        assert!(value.is_finite() && value > 0.0);
        f64::from_bits(value.to_bits() + 1)
    }

    fn next_down(value: f64) -> f64 {
        assert!(value.is_finite() && value > 0.0);
        f64::from_bits(value.to_bits() - 1)
    }

    #[test]
    fn native_two_triangle_prerequisite_matrix_is_private_fail_closed_and_non_dividing_at_180() {
        let square = [(0.0, 0.0), (400.0, 0.0), (400.0, 400.0), (0.0, 400.0)];
        let models = [
            (
                "mountain/source-order",
                two_triangle_model_with_options(EdgeKind::Mountain, false, square, 1),
            ),
            (
                "valley/source-order",
                two_triangle_model_with_options(EdgeKind::Valley, false, square, 2),
            ),
            (
                "mountain/reordered-sources",
                two_triangle_model_with_options(EdgeKind::Mountain, true, square, 3),
            ),
        ];
        for (fixture, model) in &models {
            for root in model.face_ids() {
                for thickness in [0.1, 1.0, 3.0] {
                    for angle in [0.0, 10.0, 90.0, 135.0, 179.0] {
                        let pose = triangular_pose_with_root(model, angle, *root);
                        let exact = triangular_exact_pose(model, &pose);
                        let analysis = analyze_single_triangular_hinge_prerequisites_v1(
                            &exact,
                            thickness,
                            SingleTriangularHingePrerequisiteLimits::default(),
                        )
                        .unwrap_or_else(|error| {
                            panic!(
                                "{fixture}, root {root:?}, {thickness} mm at {angle} degrees: {error:?}"
                            )
                        });
                        assert_eq!(
                            prerequisite_kind(&analysis),
                            ResultKind::Fits,
                            "{fixture}, root {root:?}, {thickness} mm at {angle} degrees"
                        );
                        let SingleTriangularHingePrerequisiteResult::Authenticated(capability) =
                            analysis.result
                        else {
                            panic!("valid native triangular hinge must mint its private token");
                        };
                        assert!(std::ptr::eq(capability.exact, &exact));
                        assert_eq!(capability.paper_thickness_bits, thickness.to_bits());
                        assert_eq!(capability.hinge_index, 0);
                        assert_eq!(
                            exact.faces[capability.left_face_index].face,
                            model.hinges()[0].left_face()
                        );
                        assert_eq!(
                            exact.faces[capability.right_face_index].face,
                            model.hinges()[0].right_face()
                        );
                    }
                }
            }
        }

        for (fixture, model) in &models {
            for root in model.face_ids() {
                let pose = triangular_pose_with_root(model, 180.0, *root);
                let exact = triangular_exact_pose(model, &pose);
                for thickness in [0.1, 1.0, 3.0] {
                    let analysis = analyze_single_triangular_hinge_prerequisites_v1(
                        &exact,
                        thickness,
                        SingleTriangularHingePrerequisiteLimits::default(),
                    )
                    .unwrap();
                    assert_eq!(
                        prerequisite_kind(&analysis),
                        ResultKind::LayerOffsetUnmodeled,
                        "{fixture}, root {root:?}, {thickness} mm at 180 degrees"
                    );
                }
            }
        }

        let model = two_triangle_model();
        let pose = triangular_pose(&model, 90.0);
        let exact = triangular_exact_pose(&model, &pose);
        for thickness in [0.0, -0.0, -0.1, f64::INFINITY, f64::NAN] {
            let analysis = analyze_single_triangular_hinge_prerequisites_v1(
                &exact,
                thickness,
                SingleTriangularHingePrerequisiteLimits::default(),
            )
            .unwrap();
            assert_eq!(
                prerequisite_kind(&analysis),
                ResultKind::Unresolved,
                "thickness bits {:x}",
                thickness.to_bits()
            );
        }
    }

    #[test]
    fn left_face_at_exact_index_one_uses_its_own_rest_array() {
        let square = [(0.0, 0.0), (400.0, 0.0), (400.0, 400.0), (0.0, 400.0)];
        let model = (10..=128)
            .map(|namespace| {
                two_triangle_model_with_options(EdgeKind::Mountain, false, square, namespace)
            })
            .find(|candidate| candidate.face_ids()[1] == candidate.hinges()[0].left_face())
            .expect("at least one deterministic namespace orders left face second");
        let pose = triangular_pose_with_root(&model, 90.0, model.face_ids()[0]);
        let exact = triangular_exact_pose(&model, &pose);
        let analysis = analyze_single_triangular_hinge_prerequisites_v1(
            &exact,
            0.1,
            SingleTriangularHingePrerequisiteLimits::default(),
        )
        .unwrap();
        let SingleTriangularHingePrerequisiteResult::Authenticated(capability) = analysis.result
        else {
            panic!("left-index-one fixture must authenticate");
        };
        assert_eq!(capability.left_face_index, 1);
        assert_eq!(capability.right_face_index, 0);
    }

    #[test]
    fn private_consumer_rebinds_exact_pointer_indexes_and_bit_exact_thickness() {
        let model = two_triangle_model();
        let pose = triangular_pose(&model, 90.0);
        let exact = triangular_exact_pose(&model, &pose);
        let independently_regenerated_exact = triangular_exact_pose(&model, &pose);
        let analysis = analyze_single_triangular_hinge_prerequisites_v1(
            &exact,
            0.1,
            SingleTriangularHingePrerequisiteLimits::default(),
        )
        .unwrap();
        let SingleTriangularHingePrerequisiteResult::Authenticated(capability) = &analysis.result
        else {
            panic!("0.1 mm fixture must authenticate");
        };
        let rebound = revalidate_single_triangular_hinge_prerequisites_v1(capability, &exact, 0.1)
            .expect("same issuer object, indexes, and thickness bits");
        assert!(std::ptr::eq(rebound.capability, capability));
        for mismatched_thickness in [1.0, 3.0, next_up(0.1)] {
            assert!(
                revalidate_single_triangular_hinge_prerequisites_v1(
                    capability,
                    &exact,
                    mismatched_thickness,
                )
                .is_none(),
                "0.1 mm token reused as bits {:x}",
                mismatched_thickness.to_bits()
            );
        }
        assert!(
            revalidate_single_triangular_hinge_prerequisites_v1(
                capability,
                &independently_regenerated_exact,
                0.1,
            )
            .is_none(),
            "equal geometry from another exact object must not pass pointer rebinding"
        );
        let swapped_face_indexes = AuthenticatedSingleTriangularHingePrerequisitesV1 {
            exact: &exact,
            paper_thickness_bits: 0.1_f64.to_bits(),
            left_face_index: capability.right_face_index,
            right_face_index: capability.left_face_index,
            hinge_index: capability.hinge_index,
        };
        assert!(
            revalidate_single_triangular_hinge_prerequisites_v1(
                &swapped_face_indexes,
                &exact,
                0.1,
            )
            .is_none()
        );
        let out_of_range_hinge = AuthenticatedSingleTriangularHingePrerequisitesV1 {
            exact: &exact,
            paper_thickness_bits: 0.1_f64.to_bits(),
            left_face_index: capability.left_face_index,
            right_face_index: capability.right_face_index,
            hinge_index: usize::MAX,
        };
        assert!(
            revalidate_single_triangular_hinge_prerequisites_v1(&out_of_range_hinge, &exact, 0.1,)
                .is_none()
        );

        for thickness in [1.0, 3.0] {
            let analysis = analyze_single_triangular_hinge_prerequisites_v1(
                &exact,
                thickness,
                SingleTriangularHingePrerequisiteLimits::default(),
            )
            .unwrap();
            let SingleTriangularHingePrerequisiteResult::Authenticated(capability) =
                &analysis.result
            else {
                panic!("{thickness} mm fixture must authenticate");
            };
            assert!(
                revalidate_single_triangular_hinge_prerequisites_v1(capability, &exact, thickness,)
                    .is_some()
            );
            assert!(
                revalidate_single_triangular_hinge_prerequisites_v1(capability, &exact, 0.1,)
                    .is_none()
            );
        }
    }

    #[test]
    fn finite_axis_helper_accepts_one_bounded_face_and_rejects_both_outside() {
        // Current native topology rejects a one-bounded/one-outside diagonal
        // as a non-convex fold sheet, so that policy row is exercised at this
        // extracted exact helper rather than by forging a native issuer.
        let limits = prerequisite_exact_hard_limits();
        let mut meter = WorkMeter::new(&limits);
        let start = point(integer(0), integer(0), integer(0));
        let end = point(integer(0), integer(0), integer(100));
        let axis = exact_between(&start, &end, &mut meter).unwrap();
        let length_squared = exact_dot(&axis, &axis, &mut meter).unwrap();
        let face = |third_z: i64, face_suffix: u64| ExactFacePose {
            face: serde_json::from_str(&format!("\"00000000-0000-4000-a100-{face_suffix:012x}\""))
                .unwrap(),
            transform: ExactRigidTransform {
                rotation: super::super::identity_matrix(),
                translation: vector(integer(0), integer(0), integer(0)),
            },
            boundary: vec![
                (triangular_vertex_id(1), start.clone()),
                (triangular_vertex_id(2), end.clone()),
                (
                    triangular_vertex_id(face_suffix + 10),
                    point(integer(10), integer(0), integer(third_z)),
                ),
            ],
        };
        let bounded = face(50, 1);
        let above_end = face(200, 2);
        let below_start = face(-100, 3);
        assert!(
            all_triangle_vertices_within_axis(&bounded, &start, &axis, &length_squared, &mut meter)
                .unwrap()
        );
        assert!(
            !all_triangle_vertices_within_axis(
                &above_end,
                &start,
                &axis,
                &length_squared,
                &mut meter
            )
            .unwrap()
        );
        assert!(
            !all_triangle_vertices_within_axis(
                &below_start,
                &start,
                &axis,
                &length_squared,
                &mut meter
            )
            .unwrap()
        );
        assert!(
            all_triangle_vertices_within_axis(&bounded, &start, &axis, &length_squared, &mut meter)
                .unwrap()
                || all_triangle_vertices_within_axis(
                    &above_end,
                    &start,
                    &axis,
                    &length_squared,
                    &mut meter
                )
                .unwrap()
        );
        assert!(
            !all_triangle_vertices_within_axis(
                &below_start,
                &start,
                &axis,
                &length_squared,
                &mut meter
            )
            .unwrap()
                && !all_triangle_vertices_within_axis(
                    &above_end,
                    &start,
                    &axis,
                    &length_squared,
                    &mut meter
                )
                .unwrap()
        );

        // A convex quadrilateral can put the two opposite vertices beyond
        // opposite finite-axis endpoints, so the negative row also reaches
        // the full native analyzer.
        let native_both_outside = two_triangle_model_with_options(
            EdgeKind::Mountain,
            false,
            [(0.0, 0.0), (100.0, 250.0), (0.0, 100.0), (-100.0, -150.0)],
            130,
        );
        let pose = triangular_pose(&native_both_outside, 90.0);
        let exact = triangular_exact_pose(&native_both_outside, &pose);
        let analysis = analyze_single_triangular_hinge_prerequisites_v1(
            &exact,
            0.1,
            SingleTriangularHingePrerequisiteLimits::default(),
        )
        .unwrap();
        assert_eq!(prerequisite_kind(&analysis), ResultKind::Unresolved);
    }

    #[test]
    fn unsupported_face_and_hinge_cardinalities_remain_unresolved() {
        let no_hinge = unsupported_polygon_model(
            &[(0.0, 0.0), (400.0, 0.0), (400.0, 400.0), (0.0, 400.0)],
            &[],
            140,
        );
        let triangle_and_quad = unsupported_polygon_model(
            &[
                (0.0, 0.0),
                (400.0, 0.0),
                (500.0, 200.0),
                (400.0, 400.0),
                (0.0, 400.0),
            ],
            &[(0, 2, EdgeKind::Mountain)],
            141,
        );
        let multiple_hinges = unsupported_polygon_model(
            &[
                (0.0, 0.0),
                (400.0, 0.0),
                (500.0, 200.0),
                (400.0, 400.0),
                (0.0, 400.0),
            ],
            &[(0, 2, EdgeKind::Mountain), (0, 3, EdgeKind::Valley)],
            142,
        );
        for (label, model) in [
            ("zero hinges", no_hinge),
            ("triangle plus quad", triangle_and_quad),
            ("multiple hinges", multiple_hinges),
        ] {
            let pose = uniform_pose(&model, 90.0);
            let exact = triangular_exact_pose(&model, &pose);
            let analysis = analyze_single_triangular_hinge_prerequisites_v1(
                &exact,
                0.1,
                SingleTriangularHingePrerequisiteLimits::default(),
            )
            .unwrap_or_else(|error| panic!("{label}: {error:?}"));
            assert_eq!(
                prerequisite_kind(&analysis),
                ResultKind::Unresolved,
                "{label}"
            );
        }
    }

    #[test]
    fn native_prerequisite_reauthenticates_version_topology_boundary_transform_and_endpoints() {
        let model = two_triangle_model();
        let pose = triangular_pose(&model, 90.0);

        let mut wrong_version = triangular_exact_pose(&model, &pose);
        wrong_version.version = "not_the_native_exact_pose_version";

        let mut wrong_boundary = triangular_exact_pose(&model, &pose);
        wrong_boundary.faces[0].boundary.swap(0, 1);

        let mut wrong_transform = triangular_exact_pose(&model, &pose);
        wrong_transform.faces[0].transform.rotation[0][0] = integer(2);

        let mut wrong_endpoint = triangular_exact_pose(&model, &pose);
        wrong_endpoint.hinges[0].world_endpoints[0].coordinates[0] += integer(1);

        let mut noncanonical = triangular_exact_pose(&model, &pose);
        noncanonical.faces[0].boundary[0].1.coordinates[0] =
            BigRational::new_raw(BigInt::from(0), BigInt::from(2));

        let mut wrong_fixed_root = triangular_exact_pose(&model, &pose);
        wrong_fixed_root.fixed_face = model
            .face_ids()
            .iter()
            .copied()
            .find(|face| Some(*face) != pose.fixed_face());

        let mut wrong_face_order = triangular_exact_pose(&model, &pose);
        wrong_face_order.faces.swap(0, 1);

        let mut wrong_face_registry = triangular_exact_pose(&model, &pose);
        wrong_face_registry.faces[0].face = wrong_face_registry.faces[1].face;

        let mut wrong_parent_child = triangular_exact_pose(&model, &pose);
        let original_parent = wrong_parent_child.hinges[0].parent;
        wrong_parent_child.hinges[0].parent = wrong_parent_child.hinges[0].child;
        wrong_parent_child.hinges[0].child = original_parent;

        let mut wrong_rotation_sign = triangular_exact_pose(&model, &pose);
        wrong_rotation_sign.hinges[0].rotation_sign = -wrong_rotation_sign.hinges[0].rotation_sign;

        let mut wrong_angle_bits = triangular_exact_pose(&model, &pose);
        wrong_angle_bits.hinges[0].angle_magnitude_bits = 91.0_f64.to_bits();

        let mut wrong_edge = triangular_exact_pose(&model, &pose);
        wrong_edge.hinges[0].edge = triangular_edge_id(999);

        let mut wrong_endpoint_order = triangular_exact_pose(&model, &pose);
        wrong_endpoint_order.hinges[0].endpoint_vertices.swap(0, 1);

        for (label, exact) in [
            ("version", wrong_version),
            ("boundary order", wrong_boundary),
            ("transform", wrong_transform),
            ("current endpoint", wrong_endpoint),
            ("noncanonical exact coordinate", noncanonical),
            ("fixed root", wrong_fixed_root),
            ("face order", wrong_face_order),
            ("face registry", wrong_face_registry),
            ("hinge parent/child", wrong_parent_child),
            ("hinge rotation sign", wrong_rotation_sign),
            ("hinge angle bits", wrong_angle_bits),
            ("hinge edge", wrong_edge),
            ("hinge endpoint vertex order", wrong_endpoint_order),
        ] {
            let analysis = analyze_single_triangular_hinge_prerequisites_v1(
                &exact,
                0.1,
                SingleTriangularHingePrerequisiteLimits::default(),
            )
            .unwrap_or_else(|error| panic!("{label}: {error:?}"));
            assert_eq!(
                prerequisite_kind(&analysis),
                ResultKind::Unresolved,
                "{label}"
            );
        }
    }

    #[test]
    fn rest_support_and_finite_axis_predicates_cover_strict_positive_and_negative_rows() {
        // A native material topology already rejects same-side or collinear
        // hinge incidences.  Test those impossible issuer rows directly at
        // the exact predicate boundary so later refactors cannot weaken the
        // strict-opposite requirement.
        let limits = prerequisite_exact_hard_limits();
        let mut meter = WorkMeter::new(&limits);
        let start = point(integer(0), integer(0), integer(0));
        let end = point(integer(0), integer(0), integer(-2));
        let left = point(integer(-1), integer(0), integer(-1));
        let right = point(integer(1), integer(0), integer(-1));
        let collinear = point(integer(0), integer(0), integer(-1));
        let left_side = exact_rest_support_orientation_xz(&start, &end, &left, &mut meter).unwrap();
        let right_side =
            exact_rest_support_orientation_xz(&start, &end, &right, &mut meter).unwrap();
        let on_line =
            exact_rest_support_orientation_xz(&start, &end, &collinear, &mut meter).unwrap();
        assert!(left_side.is_negative());
        assert!(right_side.is_positive());
        assert!(on_line.is_zero());

        let face = ExactFacePose {
            face: serde_json::from_str("\"00000000-0000-4000-a100-000000000001\"").unwrap(),
            transform: ExactRigidTransform {
                rotation: super::super::identity_matrix(),
                translation: vector(integer(0), integer(0), integer(0)),
            },
            boundary: vec![
                (triangular_vertex_id(1), start.clone()),
                (triangular_vertex_id(2), end.clone()),
                (
                    triangular_vertex_id(3),
                    point(integer(1), integer(0), integer(-1)),
                ),
            ],
        };
        let axis = exact_between(&start, &end, &mut meter).unwrap();
        let length_squared = exact_dot(&axis, &axis, &mut meter).unwrap();
        assert!(
            all_triangle_vertices_within_axis(&face, &start, &axis, &length_squared, &mut meter)
                .unwrap()
        );

        let mut beyond_both_endpoints = face;
        beyond_both_endpoints.boundary[2].1 = point(integer(1), integer(0), integer(-3));
        assert!(
            !all_triangle_vertices_within_axis(
                &beyond_both_endpoints,
                &start,
                &axis,
                &length_squared,
                &mut meter
            )
            .unwrap()
        );
    }

    #[test]
    fn triangular_prerequisite_all_observed_counters_have_exact_and_one_short_limits() {
        let model = two_triangle_model();
        let pose = triangular_pose(&model, 90.0);
        let exact = triangular_exact_pose(&model, &pose);
        let baseline = analyze_single_triangular_hinge_prerequisites_v1(
            &exact,
            0.1,
            SingleTriangularHingePrerequisiteLimits::default(),
        )
        .unwrap();
        assert_eq!(prerequisite_kind(&baseline), ResultKind::Fits);
        let work = &baseline.work;

        let exact_limits = SingleTriangularHingePrerequisiteLimits {
            max_authenticated_faces: work.authenticated_faces,
            max_authenticated_hinges: work.authenticated_hinges,
            max_boundary_occurrences: work.boundary_occurrences,
            max_source_coordinate_lifts: work.source_coordinate_lifts,
            max_current_point_reconstructions: work.current_point_reconstructions,
            max_rotation_authentications: work.rotation_authentications,
            max_local_y_component_lifts: work.local_y_component_lifts,
            max_rest_orientation_tests: work.rest_orientation_tests,
            max_axial_vertex_tests: work.axial_vertex_tests,
            max_input_rational_storage_bits: work.max_input_rational_storage_bits,
            max_total_input_storage_bits: work.total_input_storage_bits,
            exact: CayleyLimits {
                max_precision_rounds: 0,
                max_guard_bits: 0,
                max_candidate_bits: 0,
                max_machin_terms_per_series: 0,
                max_trig_terms_per_series: 0,
                max_sqrt_refinements: 0,
                max_interval_operations: work.exact.interval_operations,
                max_shift_bits: work.exact.max_shift_bits,
                max_intermediate_bits: work
                    .exact
                    .max_preflight_bits
                    .max(work.exact.max_observed_bits),
                max_gcd_fallback_calls: work.exact.gcd_fallback_calls,
                max_gcd_fallback_input_bits: work.exact.gcd_fallback_input_bits,
                max_rational_allocations: work.exact.rational_allocations,
                max_rational_allocation_bits: work.exact.max_rational_allocation_bits,
                max_total_rational_allocation_bits: work.exact.total_rational_allocation_bits,
                max_output_bits: 0,
            },
            scalar: FiniteRadiusScalarLimits {
                max_input_rational_storage_bits: work.scalar.max_input_rational_storage_bits,
                max_total_input_storage_bits: work.scalar.total_input_storage_bits,
                max_interval_operations: work.scalar.exact.interval_operations,
                max_shift_bits: work.scalar.exact.max_shift_bits,
                max_intermediate_bits: work
                    .scalar
                    .exact
                    .max_preflight_bits
                    .max(work.scalar.exact.max_observed_bits),
                max_gcd_fallback_calls: work.scalar.exact.gcd_fallback_calls,
                max_gcd_fallback_input_bits: work.scalar.exact.gcd_fallback_input_bits,
                max_rational_allocations: work.scalar.exact.rational_allocations,
                max_rational_allocation_bits: work.scalar.exact.max_rational_allocation_bits,
                max_total_rational_allocation_bits: work
                    .scalar
                    .exact
                    .total_rational_allocation_bits,
            },
        };
        let exact_analysis =
            analyze_single_triangular_hinge_prerequisites_v1(&exact, 0.1, exact_limits).unwrap();
        assert_eq!(prerequisite_kind(&exact_analysis), ResultKind::Fits);
        assert_eq!(exact_analysis.work, baseline.work);

        let assert_one_short = |resource: &str, limits: SingleTriangularHingePrerequisiteLimits| {
            assert!(
                matches!(
                    analyze_single_triangular_hinge_prerequisites_v1(&exact, 0.1, limits),
                    Err(SingleTriangularHingePrerequisiteError::ResourceLimitExceeded)
                ),
                "{resource}"
            );
        };
        macro_rules! one_short {
            ($field:ident) => {
                if exact_limits.$field > 0 {
                    let mut limits = exact_limits;
                    limits.$field -= 1;
                    assert_one_short(stringify!($field), limits);
                }
            };
        }
        one_short!(max_authenticated_faces);
        one_short!(max_authenticated_hinges);
        one_short!(max_boundary_occurrences);
        one_short!(max_source_coordinate_lifts);
        one_short!(max_current_point_reconstructions);
        one_short!(max_rotation_authentications);
        one_short!(max_local_y_component_lifts);
        one_short!(max_rest_orientation_tests);
        one_short!(max_axial_vertex_tests);
        one_short!(max_input_rational_storage_bits);
        one_short!(max_total_input_storage_bits);

        macro_rules! exact_one_short {
            ($field:ident) => {
                if exact_limits.exact.$field > 0 {
                    let mut limits = exact_limits;
                    limits.exact.$field -= 1;
                    assert_one_short(concat!("exact.", stringify!($field)), limits);
                }
            };
        }
        exact_one_short!(max_interval_operations);
        exact_one_short!(max_shift_bits);
        exact_one_short!(max_intermediate_bits);
        exact_one_short!(max_gcd_fallback_calls);
        exact_one_short!(max_gcd_fallback_input_bits);
        exact_one_short!(max_rational_allocations);
        exact_one_short!(max_rational_allocation_bits);
        exact_one_short!(max_total_rational_allocation_bits);

        macro_rules! scalar_one_short {
            ($field:ident) => {
                if exact_limits.scalar.$field > 0 {
                    let mut limits = exact_limits;
                    limits.scalar.$field -= 1;
                    assert_one_short(concat!("scalar.", stringify!($field)), limits);
                }
            };
        }
        scalar_one_short!(max_input_rational_storage_bits);
        scalar_one_short!(max_total_input_storage_bits);
        scalar_one_short!(max_interval_operations);
        scalar_one_short!(max_shift_bits);
        scalar_one_short!(max_intermediate_bits);
        scalar_one_short!(max_gcd_fallback_calls);
        scalar_one_short!(max_gcd_fallback_input_bits);
        scalar_one_short!(max_rational_allocations);
        scalar_one_short!(max_rational_allocation_bits);
        scalar_one_short!(max_total_rational_allocation_bits);
    }

    #[test]
    fn triangular_prerequisite_caller_limits_cannot_expand_hard_caps() {
        let model = two_triangle_model();
        let pose = triangular_pose(&model, 90.0);
        let exact = triangular_exact_pose(&model, &pose);
        let baseline = analyze_single_triangular_hinge_prerequisites_v1(
            &exact,
            0.1,
            SingleTriangularHingePrerequisiteLimits::default(),
        )
        .unwrap();
        let oversized = SingleTriangularHingePrerequisiteLimits {
            max_authenticated_faces: usize::MAX,
            max_authenticated_hinges: usize::MAX,
            max_boundary_occurrences: usize::MAX,
            max_source_coordinate_lifts: usize::MAX,
            max_current_point_reconstructions: usize::MAX,
            max_rotation_authentications: usize::MAX,
            max_local_y_component_lifts: usize::MAX,
            max_rest_orientation_tests: usize::MAX,
            max_axial_vertex_tests: usize::MAX,
            max_input_rational_storage_bits: usize::MAX,
            max_total_input_storage_bits: usize::MAX,
            exact: CayleyLimits {
                max_precision_rounds: usize::MAX,
                max_guard_bits: usize::MAX,
                max_candidate_bits: usize::MAX,
                max_machin_terms_per_series: usize::MAX,
                max_trig_terms_per_series: usize::MAX,
                max_sqrt_refinements: usize::MAX,
                max_interval_operations: usize::MAX,
                max_shift_bits: usize::MAX,
                max_intermediate_bits: usize::MAX,
                max_gcd_fallback_calls: usize::MAX,
                max_gcd_fallback_input_bits: usize::MAX,
                max_rational_allocations: usize::MAX,
                max_rational_allocation_bits: usize::MAX,
                max_total_rational_allocation_bits: usize::MAX,
                max_output_bits: usize::MAX,
            },
            scalar: FiniteRadiusScalarLimits {
                max_input_rational_storage_bits: usize::MAX,
                max_total_input_storage_bits: usize::MAX,
                max_interval_operations: usize::MAX,
                max_shift_bits: usize::MAX,
                max_intermediate_bits: usize::MAX,
                max_gcd_fallback_calls: usize::MAX,
                max_gcd_fallback_input_bits: usize::MAX,
                max_rational_allocations: usize::MAX,
                max_rational_allocation_bits: usize::MAX,
                max_total_rational_allocation_bits: usize::MAX,
            },
        };
        let projected =
            analyze_single_triangular_hinge_prerequisites_v1(&exact, 0.1, oversized).unwrap();
        assert_eq!(prerequisite_kind(&projected), ResultKind::Fits);
        assert_eq!(projected.work, baseline.work);
    }

    #[test]
    fn triangular_prerequisite_combined_exact_cap_is_fixed_and_overflow_checked() {
        let model = two_triangle_model();
        let pose = triangular_pose(&model, 90.0);
        let exact = triangular_exact_pose(&model, &pose);
        let baseline = analyze_single_triangular_hinge_prerequisites_v1(
            &exact,
            0.1,
            SingleTriangularHingePrerequisiteLimits::default(),
        )
        .unwrap();
        check_combined_exact_hard_cap(&baseline.work.exact, &baseline.work.scalar.exact)
            .expect("the two independently bounded meters fit their fixed aggregate cap");

        let overflowed_geometry = CayleyWork {
            interval_operations: usize::MAX,
            ..CayleyWork::default()
        };
        assert!(matches!(
            check_combined_exact_hard_cap(
                &overflowed_geometry,
                &CayleyWork {
                    interval_operations: 1,
                    ..CayleyWork::default()
                }
            ),
            Err(CayleyError::ResourceLimitExceeded {
                resource: "interval_operations",
                ..
            })
        ));
    }

    #[test]
    fn angle_and_thickness_matrix_has_a_finite_non_dividing_180_degree_boundary() {
        let start = origin();
        let end = z_axis(integer(1_000));
        let left = local_y();
        let cases = [
            (
                "0",
                rational_unit_normal_from_half_tangent(0, 1, 1),
                ResultKind::Fits,
            ),
            (
                "10",
                rational_unit_normal_from_half_tangent(7, 80, 1),
                ResultKind::Fits,
            ),
            (
                "90",
                rational_unit_normal_from_half_tangent(1, 1, 1),
                ResultKind::Fits,
            ),
            (
                "135",
                rational_unit_normal_from_half_tangent(12, 5, 1),
                ResultKind::Fits,
            ),
            (
                "179",
                rational_unit_normal_from_half_tangent(22_918, 200, 1),
                ResultKind::Fits,
            ),
            (
                "180",
                vector(integer(0), integer(-1), integer(0)),
                ResultKind::LayerOffsetUnmodeled,
            ),
        ];
        for thickness in [0.1, 1.0, 3.0] {
            for (angle, right, expected) in &cases {
                let analysis = classify(
                    thickness,
                    &start,
                    &end,
                    &left,
                    right,
                    FiniteRadiusScalarLimits::default(),
                )
                .unwrap_or_else(|error| panic!("{thickness} mm at {angle} degrees: {error:?}"));
                assert_eq!(
                    kind(&analysis),
                    *expected,
                    "{thickness} mm at {angle} degrees"
                );
            }
        }
    }

    #[test]
    fn exact_r_equals_l_is_closed_and_adjacent_binary64_thicknesses_split() {
        // nL·nR = -7/25, so cos²(theta/2) = 9/25.  With h=3 and
        // L=5, h² = L² cos² exactly.
        let start = origin();
        let end = z_axis(integer(5));
        let left = vector(integer(1), integer(0), integer(0));
        let right = vector(fraction(-7, 25), fraction(24, 25), integer(0));
        for (thickness, expected) in [
            (next_down(6.0), ResultKind::Fits),
            (6.0, ResultKind::Fits),
            (next_up(6.0), ResultKind::LayerOffsetUnmodeled),
        ] {
            let analysis = classify(
                thickness,
                &start,
                &end,
                &left,
                &right,
                FiniteRadiusScalarLimits::default(),
            )
            .unwrap();
            assert_eq!(
                kind(&analysis),
                expected,
                "thickness bits {:x}",
                thickness.to_bits()
            );
            if thickness.to_bits() == 6.0_f64.to_bits() {
                let AnalyticFiniteRadiusResult::FitsFiniteSegment(capability) = &analysis.result
                else {
                    panic!("exact equality must return its borrow-bound scalar capability");
                };
                assert_eq!(capability.paper_thickness_bits, thickness.to_bits());
                assert!(std::ptr::eq(capability.hinge_start, &start));
                assert!(std::ptr::eq(capability.hinge_end, &end));
                assert!(std::ptr::eq(
                    capability.left_cooriented_mid_surface_normal,
                    &left,
                ));
                assert!(std::ptr::eq(
                    capability.right_cooriented_mid_surface_normal,
                    &right,
                ));
            }
        }
    }

    #[test]
    fn frontend_binary64_ninety_degree_equality_is_strictly_outside_in_exact_lift() {
        let start = origin();
        let left = local_y();
        let right = vector(integer(1), integer(0), integer(0));
        for thickness in [0.1, 1.0, 3.0] {
            let rounded_length = (thickness / 2.0) / (0.5_f64).sqrt();
            for (length, expected) in [
                (next_down(rounded_length), ResultKind::LayerOffsetUnmodeled),
                (rounded_length, ResultKind::LayerOffsetUnmodeled),
                (next_up(rounded_length), ResultKind::Fits),
            ] {
                let end = z_axis(
                    BigRational::from_float(length).expect("positive finite binary64 length"),
                );
                let analysis = classify(
                    thickness,
                    &start,
                    &end,
                    &left,
                    &right,
                    FiniteRadiusScalarLimits::default(),
                )
                .unwrap();
                assert_eq!(
                    kind(&analysis),
                    expected,
                    "{thickness} mm, length bits {:x}",
                    length.to_bits()
                );
            }
        }
    }

    #[test]
    fn zero_signed_zero_and_nonfinite_thickness_never_create_a_capability() {
        let start = origin();
        let end = z_axis(integer(1));
        let left = local_y();
        let opposite = vector(integer(0), integer(-1), integer(0));
        for thickness in [0.0, -0.0, -0.1, f64::INFINITY, f64::NEG_INFINITY, f64::NAN] {
            let analysis = classify(
                thickness,
                &start,
                &end,
                &left,
                &opposite,
                FiniteRadiusScalarLimits::default(),
            )
            .unwrap();
            assert_eq!(kind(&analysis), ResultKind::Unresolved);
            assert_eq!(analysis.work, FiniteRadiusScalarWork::default());
        }
    }

    #[test]
    fn minimum_subnormal_is_halved_exactly_without_underflow() {
        let start = origin();
        let end = z_axis(integer(1));
        let normal = local_y();
        let minimum_subnormal = f64::from_bits(1);
        let analysis = classify(
            minimum_subnormal,
            &start,
            &end,
            &normal,
            &normal,
            FiniteRadiusScalarLimits::default(),
        )
        .unwrap();
        assert_eq!(kind(&analysis), ResultKind::Fits);
        assert!(analysis.work.exact.max_shift_bits >= 1_074);
    }

    #[test]
    fn zero_thickness_and_flat_fold_cannot_pass_the_degenerate_radial_inequality() {
        let start = origin();
        let end = z_axis(integer(1));
        let left = local_y();
        let right = vector(integer(0), integer(-1), integer(0));
        let zero = classify(
            0.0,
            &start,
            &end,
            &left,
            &right,
            FiniteRadiusScalarLimits::default(),
        )
        .unwrap();
        assert_eq!(kind(&zero), ResultKind::Unresolved);

        let positive = classify(
            f64::from_bits(1),
            &start,
            &end,
            &left,
            &right,
            FiniteRadiusScalarLimits::default(),
        )
        .unwrap();
        assert_eq!(kind(&positive), ResultKind::LayerOffsetUnmodeled);
    }

    #[test]
    fn numerical_flat_fold_singularity_is_rejected_even_when_analytic_radius_fits() {
        let start = origin();
        let end = z_axis(integer(1_000_000));
        let left = local_y();
        let right = rational_unit_normal_from_half_tangent(67_108_864, 1, 1);
        let analysis = classify(
            f64::from_bits(1),
            &start,
            &end,
            &left,
            &right,
            FiniteRadiusScalarLimits::default(),
        )
        .unwrap();
        assert_eq!(kind(&analysis), ResultKind::LayerOffsetUnmodeled);
    }

    #[test]
    fn degenerate_axis_nonunit_normals_axial_components_and_noncanonical_inputs_are_unresolved() {
        let start = origin();
        let end = z_axis(integer(1));
        let left = local_y();
        let right = vector(integer(1), integer(0), integer(0));

        let cases = [
            (origin(), local_y(), right.clone(), "zero-length hinge axis"),
            (
                end.clone(),
                vector(integer(0), integer(2), integer(0)),
                right.clone(),
                "non-unit left normal",
            ),
            (
                end.clone(),
                vector(integer(0), integer(0), integer(1)),
                right.clone(),
                "normal with hinge-axis component",
            ),
            (
                end.clone(),
                vector(
                    BigRational::new_raw(BigInt::from(0), BigInt::from(2)),
                    BigRational::new_raw(BigInt::from(2), BigInt::from(2)),
                    integer(0),
                ),
                right,
                "noncanonical exact input",
            ),
        ];
        for (case_end, case_left, case_right, label) in cases {
            let analysis = classify(
                0.1,
                &start,
                &case_end,
                &case_left,
                &case_right,
                FiniteRadiusScalarLimits::default(),
            )
            .unwrap();
            assert_eq!(kind(&analysis), ResultKind::Unresolved, "{label}");
        }

        let valid_right = vector(integer(1), integer(0), integer(0));
        let valid = classify(
            0.1,
            &start,
            &end,
            &left,
            &valid_right,
            FiniteRadiusScalarLimits::default(),
        )
        .unwrap();
        assert_eq!(kind(&valid), ResultKind::Fits);
    }

    #[test]
    fn endpoint_normal_face_and_fold_sign_reversal_preserve_the_scalar_result() {
        let start = point(integer(11), integer(-7), integer(5));
        let end = point(integer(11), integer(-7), integer(10));
        let left = local_y();
        let mountain = rational_unit_normal_from_half_tangent(12, 5, 1);
        let valley = rational_unit_normal_from_half_tangent(12, 5, -1);
        let baseline = classify(
            0.1,
            &start,
            &end,
            &left,
            &mountain,
            FiniteRadiusScalarLimits::default(),
        )
        .unwrap();
        assert_eq!(kind(&baseline), ResultKind::Fits);

        for analysis in [
            classify(
                0.1,
                &end,
                &start,
                &left,
                &mountain,
                FiniteRadiusScalarLimits::default(),
            ),
            classify(
                0.1,
                &start,
                &end,
                &mountain,
                &left,
                FiniteRadiusScalarLimits::default(),
            ),
            classify(
                0.1,
                &start,
                &end,
                &left,
                &valley,
                FiniteRadiusScalarLimits::default(),
            ),
        ] {
            assert_eq!(kind(&analysis.unwrap()), ResultKind::Fits);
        }
    }

    #[test]
    fn common_exact_rigid_transform_and_large_translation_preserve_the_result() {
        let start = origin();
        let end = z_axis(integer(5));
        let left = vector(integer(1), integer(0), integer(0));
        let right = vector(fraction(-7, 25), fraction(24, 25), integer(0));
        let baseline = classify(
            6.0,
            &start,
            &end,
            &left,
            &right,
            FiniteRadiusScalarLimits::default(),
        )
        .unwrap();
        assert_eq!(kind(&baseline), ResultKind::Fits);

        // Proper coordinate permutation (x,y,z)->(z,x,y), followed by a
        // large exact common translation.
        let translation = [
            integer(1_000_000_000_000_000),
            integer(-3_000_000_000_000),
            integer(7),
        ];
        let transform_point = |source: &ExactPoint3| {
            point(
                &source.coordinates[2] + &translation[0],
                &source.coordinates[0] + &translation[1],
                &source.coordinates[1] + &translation[2],
            )
        };
        let transform_vector = |source: &ExactVector3| {
            vector(
                source.coordinates[2].clone(),
                source.coordinates[0].clone(),
                source.coordinates[1].clone(),
            )
        };
        let transformed_start = transform_point(&start);
        let transformed_end = transform_point(&end);
        let transformed_left = transform_vector(&left);
        let transformed_right = transform_vector(&right);
        let transformed = classify(
            6.0,
            &transformed_start,
            &transformed_end,
            &transformed_left,
            &transformed_right,
            FiniteRadiusScalarLimits::default(),
        )
        .unwrap();
        assert_eq!(kind(&transformed), ResultKind::Fits);
    }

    #[test]
    fn opposite_prism_winding_normal_is_not_a_cooriented_flat_input() {
        let start = origin();
        let end = z_axis(integer(1));
        let left_local_y = local_y();
        let right_prism_outward = vector(integer(0), integer(-1), integer(0));
        let analysis = classify(
            0.1,
            &start,
            &end,
            &left_local_y,
            &right_prism_outward,
            FiniteRadiusScalarLimits::default(),
        )
        .unwrap();
        assert_eq!(kind(&analysis), ResultKind::LayerOffsetUnmodeled);
    }

    #[test]
    fn every_observed_resource_has_an_exact_and_one_short_boundary() {
        let start = origin();
        let end = z_axis(integer(5));
        let left = vector(integer(1), integer(0), integer(0));
        let right = vector(fraction(-7, 25), fraction(24, 25), integer(0));
        let baseline = classify(
            6.0,
            &start,
            &end,
            &left,
            &right,
            FiniteRadiusScalarLimits::default(),
        )
        .unwrap();
        assert_eq!(kind(&baseline), ResultKind::Fits);
        let work = &baseline.work;

        let exact_limits = FiniteRadiusScalarLimits {
            max_input_rational_storage_bits: work.max_input_rational_storage_bits,
            max_total_input_storage_bits: work.total_input_storage_bits,
            max_interval_operations: work.exact.interval_operations,
            max_shift_bits: work.exact.max_shift_bits,
            max_intermediate_bits: work
                .exact
                .max_preflight_bits
                .max(work.exact.max_observed_bits),
            max_gcd_fallback_calls: work.exact.gcd_fallback_calls,
            max_gcd_fallback_input_bits: work.exact.gcd_fallback_input_bits,
            max_rational_allocations: work.exact.rational_allocations,
            max_rational_allocation_bits: work.exact.max_rational_allocation_bits,
            max_total_rational_allocation_bits: work.exact.total_rational_allocation_bits,
        };
        let exact = classify(6.0, &start, &end, &left, &right, exact_limits).unwrap();
        assert_eq!(kind(&exact), ResultKind::Fits);
        assert_eq!(exact.work, baseline.work);

        let mut one_short_cases = Vec::new();
        macro_rules! one_short {
            ($field:ident) => {
                if exact_limits.$field > 0 {
                    let mut limits = exact_limits;
                    limits.$field -= 1;
                    one_short_cases.push((stringify!($field), limits));
                }
            };
        }
        one_short!(max_input_rational_storage_bits);
        one_short!(max_total_input_storage_bits);
        one_short!(max_interval_operations);
        one_short!(max_shift_bits);
        one_short!(max_intermediate_bits);
        one_short!(max_gcd_fallback_calls);
        one_short!(max_gcd_fallback_input_bits);
        one_short!(max_rational_allocations);
        one_short!(max_rational_allocation_bits);
        one_short!(max_total_rational_allocation_bits);
        for (resource, limits) in one_short_cases {
            assert!(
                matches!(
                    classify(6.0, &start, &end, &left, &right, limits),
                    Err(FiniteRadiusScalarError::ResourceLimitExceeded)
                ),
                "{resource}"
            );
        }
    }

    #[test]
    fn caller_limits_cannot_expand_hard_caps_and_input_accounting_overflow_is_atomic() {
        let start = origin();
        let end = z_axis(integer(5));
        let left = vector(integer(1), integer(0), integer(0));
        let right = vector(fraction(-7, 25), fraction(24, 25), integer(0));
        let baseline = classify(
            6.0,
            &start,
            &end,
            &left,
            &right,
            FiniteRadiusScalarLimits::default(),
        )
        .unwrap();
        let oversized = FiniteRadiusScalarLimits {
            max_input_rational_storage_bits: usize::MAX,
            max_total_input_storage_bits: usize::MAX,
            max_interval_operations: usize::MAX,
            max_shift_bits: usize::MAX,
            max_intermediate_bits: usize::MAX,
            max_gcd_fallback_calls: usize::MAX,
            max_gcd_fallback_input_bits: usize::MAX,
            max_rational_allocations: usize::MAX,
            max_rational_allocation_bits: usize::MAX,
            max_total_rational_allocation_bits: usize::MAX,
        };
        let projected = classify(6.0, &start, &end, &left, &right, oversized).unwrap();
        assert_eq!(kind(&projected), kind(&baseline));
        assert_eq!(projected.work, baseline.work);

        let mut overflow = FiniteRadiusScalarWork {
            input_rationals: 1,
            max_input_rational_storage_bits: 1,
            total_input_storage_bits: usize::MAX,
            exact: CayleyWork::default(),
        };
        assert!(matches!(
            charge_input_storage(&mut overflow, 1, &FiniteRadiusScalarLimits::default()),
            Err(CayleyError::ResourceLimitExceeded {
                resource: "total_input_storage_bits",
                ..
            })
        ));
        assert_eq!(overflow.total_input_storage_bits, usize::MAX);
        assert_eq!(overflow.input_rationals, 1);
    }
}
