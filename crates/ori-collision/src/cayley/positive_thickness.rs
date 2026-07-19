//! Exact scalar prerequisites for a future positive-thickness hinge model.
//!
//! The scalar gate in this parent module deliberately does **not** classify a
//! prism pair, prove corridor containment, or authorize contact.  A
//! `FiniteRadiusScalarFits` value only proves that the analytic centered-slab
//! radius is finite and no longer than the supplied exact finite hinge
//! segment.  Private child phases now construct the complete exact-`E` prism
//! intersection and prove its containment in one closed finite corridor.
//! Further children independently reconstruct the literal direct-lift
//! binary64 affine pose `F`, rerun its complete prism intersection, and scan
//! both the earlier rigid-limit corridor and a canonical affine half-prism
//! corridor without tolerance.  The affine result is explicitly
//! `ContainedUnadmitted`: no child issues production collision, admission, or
//! safe-set authority, and the future shared-hinge admission gate is separate.
//! The complete positive-thickness path stays private until later code also
//! proves all of the following:
//!
//! - both normals are the co-oriented rest-local `+Y` columns of the two exact
//!   rigid face transforms, not triangle-winding or prism outward normals;
//! - the shared edge and its opposite boundary occurrences are authenticated;
//! - every source triangle lies in the required opposing material half-plane;
//! - at least one source triangle bounds each interaction axially;
//! - every production triangle pair is scanned on both `E` and `F`, with its
//!   complete intersection proved inside the same finite corridor; and
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

mod direct_f_affine_corridor;
mod direct_f_corridor;
mod ef_boundary;
mod exact_e_corridor;
mod exact_prism;

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

    use super::direct_f_affine_corridor::{
        DirectFAffineHingeCorridorAnalysis, DirectFAffineHingeCorridorDiagnosticV1,
        DirectFAffineHingeCorridorError, DirectFAffineHingeCorridorLimits,
        DirectFAffineHingeCorridorResult, DirectFAffineHingeCorridorWork,
        analyze_direct_f_affine_hinge_corridor_v1,
        revalidate_direct_f_affine_hinge_corridor_diagnostic_v1,
    };
    use super::direct_f_corridor::{
        DirectFFiniteHingeCorridorAnalysis, DirectFFiniteHingeCorridorCapabilityV1,
        DirectFFiniteHingeCorridorError, DirectFFiniteHingeCorridorLimits,
        DirectFFiniteHingeCorridorResult, DirectFFiniteHingeCorridorWork,
        analyze_direct_f_finite_hinge_corridor_v1, revalidate_direct_f_finite_hinge_corridor_v1,
    };
    use super::ef_boundary::{
        AxisAlignedEfBoundaryAnalysis, AxisAlignedEfBoundaryCapabilityV1,
        AxisAlignedEfBoundaryError, AxisAlignedEfBoundaryLimits, AxisAlignedEfBoundaryWork,
        analyze_axis_aligned_ef_boundary_v1, revalidate_axis_aligned_ef_boundary_v1,
    };
    use super::exact_e_corridor::{
        ExactEFiniteHingeCorridorAnalysis, ExactEFiniteHingeCorridorCapabilityV1,
        ExactEFiniteHingeCorridorError, ExactEFiniteHingeCorridorLimits,
        ExactEFiniteHingeCorridorResult, ExactEFiniteHingeCorridorWork,
        ExactEFiniteHingeInteractionKind, analyze_exact_e_finite_hinge_corridor_v1,
        revalidate_exact_e_finite_hinge_corridor_v1,
    };
    use super::exact_prism::ExactPrismLimits;
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

    fn direct_affine_point(
        transform: ori_kinematics::RigidTransform,
        source: ori_kinematics::Point3,
    ) -> ExactPoint3 {
        let rows = transform.rotation_rows();
        let translation = transform.translation();
        let source = [
            BigRational::from_float(source.x()).unwrap(),
            BigRational::from_float(source.y()).unwrap(),
            BigRational::from_float(source.z()).unwrap(),
        ];
        let translation = [
            BigRational::from_float(translation.x()).unwrap(),
            BigRational::from_float(translation.y()).unwrap(),
            BigRational::from_float(translation.z()).unwrap(),
        ];
        ExactPoint3 {
            coordinates: std::array::from_fn(|row| {
                let products = std::array::from_fn::<_, 3, _>(|column| {
                    BigRational::from_float(rows[row][column]).unwrap() * &source[column]
                });
                &products[0] + &products[1] + &products[2] + &translation[row]
            }),
        }
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

    fn authenticated_ef_prerequisite<'exact, 'pose>(
        exact: &'exact RationalCayleyTreePose<'pose>,
        paper_thickness_mm: f64,
    ) -> SingleTriangularHingePrerequisiteAnalysis<'exact, 'pose> {
        analyze_single_triangular_hinge_prerequisites_v1(
            exact,
            paper_thickness_mm,
            SingleTriangularHingePrerequisiteLimits::default(),
        )
        .expect("finite-hinge prerequisite analysis")
    }

    fn ef_capability<'a, 'prerequisite, 'exact, 'pose>(
        analysis: &'a AxisAlignedEfBoundaryAnalysis<'prerequisite, 'exact, 'pose>,
    ) -> &'a AxisAlignedEfBoundaryCapabilityV1<'prerequisite, 'exact, 'pose> {
        analysis
            .capability
            .as_ref()
            .expect("E/F boundary must authenticate")
    }

    fn exact_e_corridor_capability<'a, 'prerequisite, 'ef, 'exact, 'pose>(
        analysis: &'a ExactEFiniteHingeCorridorAnalysis<'prerequisite, 'ef, 'exact, 'pose>,
    ) -> &'a ExactEFiniteHingeCorridorCapabilityV1<'prerequisite, 'ef, 'exact, 'pose> {
        let ExactEFiniteHingeCorridorResult::Contained(capability) = &analysis.result else {
            panic!("exact-E finite hinge corridor must be contained");
        };
        capability
    }

    fn direct_f_corridor_capability<'a, 'prerequisite, 'ef, 'exact_e_corridor, 'exact, 'pose>(
        analysis: &'a DirectFFiniteHingeCorridorAnalysis<
            'prerequisite,
            'ef,
            'exact_e_corridor,
            'exact,
            'pose,
        >,
    ) -> &'a DirectFFiniteHingeCorridorCapabilityV1<
        'prerequisite,
        'ef,
        'exact_e_corridor,
        'exact,
        'pose,
    > {
        let DirectFFiniteHingeCorridorResult::Contained(capability) = &analysis.result else {
            panic!("direct-F finite hinge corridor must be contained");
        };
        capability
    }

    fn direct_f_affine_diagnostic<'a, 'prerequisite, 'ef, 'exact_e_corridor, 'exact, 'pose>(
        analysis: &'a DirectFAffineHingeCorridorAnalysis<
            'prerequisite,
            'ef,
            'exact_e_corridor,
            'exact,
            'pose,
        >,
    ) -> &'a DirectFAffineHingeCorridorDiagnosticV1<
        'prerequisite,
        'ef,
        'exact_e_corridor,
        'exact,
        'pose,
    > {
        let DirectFAffineHingeCorridorResult::ContainedUnadmitted(diagnostic) = &analysis.result
        else {
            panic!("direct-F affine corridor must be a contained unadmitted diagnostic");
        };
        diagnostic
    }

    #[test]
    fn exact_e_corridor_native_400mm_matrix_is_contained_and_closed_at_zero() {
        let square = [(0.0, 0.0), (400.0, 0.0), (400.0, 400.0), (0.0, 400.0)];
        let models = [
            (
                "mountain/source",
                two_triangle_model_with_options(EdgeKind::Mountain, false, square, 401),
            ),
            (
                "mountain/reordered",
                two_triangle_model_with_options(EdgeKind::Mountain, true, square, 402),
            ),
            (
                "valley/source",
                two_triangle_model_with_options(EdgeKind::Valley, false, square, 403),
            ),
            (
                "valley/reordered",
                two_triangle_model_with_options(EdgeKind::Valley, true, square, 404),
            ),
        ];
        for (fixture, model) in &models {
            for root in model.face_ids() {
                for thickness in [0.1, 1.0, 3.0] {
                    for angle in [0.0, 10.0, 90.0, 135.0, 179.0] {
                        let pose = triangular_pose_with_root(model, angle, *root);
                        let bound = model.bind_pose(&pose).unwrap();
                        let exact = triangular_exact_pose(model, &pose);
                        let prerequisite_analysis =
                            authenticated_ef_prerequisite(&exact, thickness);
                        let SingleTriangularHingePrerequisiteResult::Authenticated(prerequisite) =
                            &prerequisite_analysis.result
                        else {
                            panic!(
                                "{fixture}, root {root:?}, {thickness} mm at {angle}: prerequisite"
                            );
                        };
                        let ef_analysis = analyze_axis_aligned_ef_boundary_v1(
                            prerequisite,
                            &exact,
                            bound,
                            thickness,
                            AxisAlignedEfBoundaryLimits::default(),
                        )
                        .unwrap();
                        let ef = ef_capability(&ef_analysis);
                        let analysis = analyze_exact_e_finite_hinge_corridor_v1(
                            &prerequisite_analysis,
                            Some(ef),
                            &exact,
                            bound,
                            thickness,
                            ExactEFiniteHingeCorridorLimits::default(),
                        )
                        .unwrap_or_else(|error| {
                            panic!("{fixture}, root {root:?}, {thickness} mm at {angle}: {error:?}")
                        });
                        let capability = exact_e_corridor_capability(&analysis);
                        let expected_kind = if angle == 0.0 {
                            ExactEFiniteHingeInteractionKind::BoundaryAreaContact
                        } else {
                            ExactEFiniteHingeInteractionKind::PositiveVolume
                        };
                        assert_eq!(
                            capability.interaction_kind(),
                            expected_kind,
                            "{fixture}, root {root:?}, {thickness} mm at {angle}"
                        );
                        assert_eq!(capability.length_squared(), &integer(320_000));
                        assert_eq!(
                            capability.half_thickness(),
                            &(BigRational::from_float(thickness).unwrap() / integer(2))
                        );
                        if angle == 0.0 {
                            assert_eq!(capability.cosine_half_squared(), &integer(1));
                            assert_eq!(
                                analysis.work.corridor_vertex_tests, 4,
                                "the four closed rectangle vertices lie on axial/radial boundaries"
                            );
                        } else {
                            assert!(analysis.work.corridor_vertex_tests >= 4);
                        }
                        assert!(
                            analysis.work.corridor_vertex_tests <= 120,
                            "all retained exact intersection vertices are scanned"
                        );
                        assert_eq!(analysis.work.authenticated_faces, 2);
                        assert_eq!(analysis.work.authenticated_hinges, 1);
                        assert_eq!(analysis.work.prism.prisms, 2);
                        assert!(
                            analysis.work.exact.interval_operations
                                >= analysis.work.prism.exact.interval_operations
                        );
                        let rebound = revalidate_exact_e_finite_hinge_corridor_v1(
                            capability,
                            prerequisite,
                            ef,
                            &exact,
                            bound,
                            thickness,
                        )
                        .expect(
                            "all phase-1, E/F, exact-E, native-instance and thickness bindings",
                        );
                        assert!(std::ptr::eq(rebound.capability, capability));
                    }
                }
            }
        }
    }

    #[test]
    fn exact_e_corridor_propagates_180_layer_offset_without_prism_work() {
        let square = [(0.0, 0.0), (400.0, 0.0), (400.0, 400.0), (0.0, 400.0)];
        let models = [
            two_triangle_model_with_options(EdgeKind::Mountain, false, square, 405),
            two_triangle_model_with_options(EdgeKind::Mountain, true, square, 406),
            two_triangle_model_with_options(EdgeKind::Valley, false, square, 407),
            two_triangle_model_with_options(EdgeKind::Valley, true, square, 408),
        ];
        for model in &models {
            for root in model.face_ids() {
                let pose = triangular_pose_with_root(model, 180.0, *root);
                let bound = model.bind_pose(&pose).unwrap();
                let exact = triangular_exact_pose(model, &pose);
                for thickness in [0.1, 1.0, 3.0] {
                    let prerequisite_analysis = authenticated_ef_prerequisite(&exact, thickness);
                    assert!(matches!(
                        prerequisite_analysis.result,
                        SingleTriangularHingePrerequisiteResult::LayerOffsetUnmodeled
                    ));
                    let analysis = analyze_exact_e_finite_hinge_corridor_v1(
                        &prerequisite_analysis,
                        None,
                        &exact,
                        bound,
                        thickness,
                        ExactEFiniteHingeCorridorLimits::default(),
                    )
                    .unwrap();
                    assert!(matches!(
                        analysis.result,
                        ExactEFiniteHingeCorridorResult::LayerOffsetUnmodeled
                    ));
                    assert_eq!(analysis.work, ExactEFiniteHingeCorridorWork::default());
                }
            }
        }
    }

    #[test]
    fn exact_e_corridor_propagates_unresolved_prerequisite_without_work() {
        let model = two_triangle_model();
        let pose = triangular_pose(&model, 90.0);
        let bound = model.bind_pose(&pose).unwrap();
        let exact = triangular_exact_pose(&model, &pose);
        let unresolved = SingleTriangularHingePrerequisiteAnalysis {
            result: SingleTriangularHingePrerequisiteResult::Unresolved,
            work: SingleTriangularHingePrerequisiteWork::default(),
        };

        let analysis = analyze_exact_e_finite_hinge_corridor_v1(
            &unresolved,
            None,
            &exact,
            bound,
            0.1,
            ExactEFiniteHingeCorridorLimits::default(),
        )
        .unwrap();

        assert!(matches!(
            analysis.result,
            ExactEFiniteHingeCorridorResult::Unresolved
        ));
        assert_eq!(analysis.work, ExactEFiniteHingeCorridorWork::default());
    }

    #[test]
    fn exact_e_corridor_rejects_token_exact_aba_foreign_root_ulp_and_f_bit_mismatches() {
        let model = two_triangle_model();
        let pose = triangular_pose(&model, 135.0);
        let bound = model.bind_pose(&pose).unwrap();
        let exact = triangular_exact_pose(&model, &pose);
        let independent_exact = triangular_exact_pose(&model, &pose);
        let prerequisite_analysis = authenticated_ef_prerequisite(&exact, 0.1);
        let duplicate_prerequisite_analysis = authenticated_ef_prerequisite(&exact, 0.1);
        let SingleTriangularHingePrerequisiteResult::Authenticated(prerequisite) =
            &prerequisite_analysis.result
        else {
            panic!("prerequisite");
        };
        let SingleTriangularHingePrerequisiteResult::Authenticated(duplicate_prerequisite) =
            &duplicate_prerequisite_analysis.result
        else {
            panic!("duplicate prerequisite");
        };
        let ef_analysis = analyze_axis_aligned_ef_boundary_v1(
            prerequisite,
            &exact,
            bound,
            0.1,
            AxisAlignedEfBoundaryLimits::default(),
        )
        .unwrap();
        let duplicate_ef_analysis = analyze_axis_aligned_ef_boundary_v1(
            prerequisite,
            &exact,
            bound,
            0.1,
            AxisAlignedEfBoundaryLimits::default(),
        )
        .unwrap();
        let ef = ef_capability(&ef_analysis);
        let duplicate_ef = ef_capability(&duplicate_ef_analysis);
        let corridor_analysis = analyze_exact_e_finite_hinge_corridor_v1(
            &prerequisite_analysis,
            Some(ef),
            &exact,
            bound,
            0.1,
            ExactEFiniteHingeCorridorLimits::default(),
        )
        .unwrap();
        let corridor = exact_e_corridor_capability(&corridor_analysis);
        assert!(
            revalidate_exact_e_finite_hinge_corridor_v1(
                corridor,
                prerequisite,
                ef,
                &exact,
                bound,
                0.1,
            )
            .is_some()
        );
        assert!(
            revalidate_exact_e_finite_hinge_corridor_v1(
                corridor,
                duplicate_prerequisite,
                ef,
                &exact,
                bound,
                0.1,
            )
            .is_none(),
            "an independently issued phase-1 token is not interchangeable"
        );
        assert!(
            revalidate_exact_e_finite_hinge_corridor_v1(
                corridor,
                prerequisite,
                duplicate_ef,
                &exact,
                bound,
                0.1,
            )
            .is_none(),
            "an independently issued E/F capability is not interchangeable"
        );
        assert!(
            revalidate_exact_e_finite_hinge_corridor_v1(
                corridor,
                prerequisite,
                ef,
                &independent_exact,
                bound,
                0.1,
            )
            .is_none(),
            "an independently regenerated exact E fails pointer binding"
        );

        let aba_pose = triangular_pose(&model, 135.0);
        let aba_bound = model.bind_pose(&aba_pose).unwrap();
        assert!(
            revalidate_exact_e_finite_hinge_corridor_v1(
                corridor,
                prerequisite,
                ef,
                &exact,
                aba_bound,
                0.1,
            )
            .is_none(),
            "same-angle re-solve ABA"
        );
        let rerooted_pose = triangular_pose_with_root(&model, 135.0, model.face_ids()[1]);
        let rerooted_bound = model.bind_pose(&rerooted_pose).unwrap();
        assert!(
            revalidate_exact_e_finite_hinge_corridor_v1(
                corridor,
                prerequisite,
                ef,
                &exact,
                rerooted_bound,
                0.1,
            )
            .is_none(),
            "a different root is a different native pose instance"
        );
        let one_ulp_pose = triangular_pose(&model, next_up(135.0));
        let one_ulp_bound = model.bind_pose(&one_ulp_pose).unwrap();
        assert!(
            revalidate_exact_e_finite_hinge_corridor_v1(
                corridor,
                prerequisite,
                ef,
                &exact,
                one_ulp_bound,
                0.1,
            )
            .is_none(),
            "one-ULP angle change is a different native pose instance"
        );
        assert!(
            revalidate_exact_e_finite_hinge_corridor_v1(
                corridor,
                prerequisite,
                ef,
                &exact,
                bound,
                next_up(0.1),
            )
            .is_none(),
            "paper thickness is bound by its complete binary64 bits"
        );

        let foreign = two_triangle_model_with_options(
            EdgeKind::Mountain,
            false,
            [(0.0, 0.0), (400.0, 0.0), (400.0, 400.0), (0.0, 400.0)],
            409,
        );
        let foreign_pose = triangular_pose(&foreign, 135.0);
        let foreign_bound = foreign.bind_pose(&foreign_pose).unwrap();
        assert!(
            revalidate_exact_e_finite_hinge_corridor_v1(
                corridor,
                prerequisite,
                ef,
                &exact,
                foreign_bound,
                0.1,
            )
            .is_none(),
            "foreign model/issuer"
        );

        let mut tampered_ef_analysis = analyze_axis_aligned_ef_boundary_v1(
            prerequisite,
            &exact,
            bound,
            0.1,
            AxisAlignedEfBoundaryLimits::default(),
        )
        .unwrap();
        let tampered_ef = tampered_ef_analysis
            .capability
            .as_mut()
            .expect("tampered E/F capability");
        tampered_ef.binary64_face_transforms[0].rotation[0][0] =
            tampered_ef.binary64_face_transforms[0].rotation[0][0]
                .checked_add(1)
                .unwrap();
        let rejected = analyze_exact_e_finite_hinge_corridor_v1(
            &prerequisite_analysis,
            Some(tampered_ef),
            &exact,
            bound,
            0.1,
            ExactEFiniteHingeCorridorLimits::default(),
        )
        .unwrap();
        assert!(matches!(
            rejected.result,
            ExactEFiniteHingeCorridorResult::Unresolved
        ));
        assert_eq!(rejected.work, ExactEFiniteHingeCorridorWork::default());
    }

    #[test]
    fn exact_e_corridor_geometry_is_independent_of_all_ef_component_boxes() {
        let model = two_triangle_model();
        let pose = triangular_pose(&model, 135.0);
        let bound = model.bind_pose(&pose).unwrap();
        let exact = triangular_exact_pose(&model, &pose);
        let prerequisite_analysis = authenticated_ef_prerequisite(&exact, 3.0);
        let SingleTriangularHingePrerequisiteResult::Authenticated(prerequisite) =
            &prerequisite_analysis.result
        else {
            panic!("prerequisite");
        };
        let baseline_ef_analysis = analyze_axis_aligned_ef_boundary_v1(
            prerequisite,
            &exact,
            bound,
            3.0,
            AxisAlignedEfBoundaryLimits::default(),
        )
        .unwrap();
        let mut mutated_ef_analysis = analyze_axis_aligned_ef_boundary_v1(
            prerequisite,
            &exact,
            bound,
            3.0,
            AxisAlignedEfBoundaryLimits::default(),
        )
        .unwrap();
        let mutated_ef = mutated_ef_analysis
            .capability
            .as_mut()
            .expect("mutated E/F capability");
        let sentinel = integer(999_999);
        for face in &mut mutated_ef.faces {
            face.point_component_bound_mm = std::array::from_fn(|_| sentinel.clone());
            face.normal_component_bound = std::array::from_fn(|_| sentinel.clone());
            face.solid_component_bound_mm = std::array::from_fn(|_| sentinel.clone());
            face.point_linf_bound_mm = sentinel.clone();
            face.normal_linf_bound = sentinel.clone();
            face.solid_linf_bound_mm = sentinel.clone();
        }
        mutated_ef.point_component_bound_mm = std::array::from_fn(|_| sentinel.clone());
        mutated_ef.normal_component_bound = std::array::from_fn(|_| sentinel.clone());
        mutated_ef.solid_component_bound_mm = std::array::from_fn(|_| sentinel.clone());
        mutated_ef.point_linf_bound_mm = sentinel.clone();
        mutated_ef.normal_linf_bound = sentinel.clone();
        mutated_ef.solid_linf_bound_mm = sentinel;

        let baseline = analyze_exact_e_finite_hinge_corridor_v1(
            &prerequisite_analysis,
            Some(ef_capability(&baseline_ef_analysis)),
            &exact,
            bound,
            3.0,
            ExactEFiniteHingeCorridorLimits::default(),
        )
        .unwrap();
        let mutated = analyze_exact_e_finite_hinge_corridor_v1(
            &prerequisite_analysis,
            Some(
                mutated_ef_analysis
                    .capability
                    .as_ref()
                    .expect("mutated E/F capability"),
            ),
            &exact,
            bound,
            3.0,
            ExactEFiniteHingeCorridorLimits::default(),
        )
        .unwrap();
        let baseline_capability = exact_e_corridor_capability(&baseline);
        let mutated_capability = exact_e_corridor_capability(&mutated);
        assert_eq!(
            baseline_capability.interaction_kind(),
            mutated_capability.interaction_kind()
        );
        assert_eq!(
            baseline_capability.length_squared(),
            mutated_capability.length_squared()
        );
        assert_eq!(
            baseline_capability.half_thickness(),
            mutated_capability.half_thickness()
        );
        assert_eq!(
            baseline_capability.cosine_half_squared(),
            mutated_capability.cosine_half_squared()
        );
        assert_eq!(baseline.work, mutated.work);
    }

    fn cayley_limits_from_observed_work(work: &CayleyWork) -> CayleyLimits {
        CayleyLimits {
            max_precision_rounds: 0,
            max_guard_bits: 0,
            max_candidate_bits: 0,
            max_machin_terms_per_series: work.max_machin_series_terms,
            max_trig_terms_per_series: work.max_trig_series_terms,
            max_sqrt_refinements: work.max_sqrt_call_refinements,
            max_interval_operations: work.interval_operations,
            max_shift_bits: work.max_shift_bits,
            max_intermediate_bits: work.max_preflight_bits.max(work.max_observed_bits),
            max_gcd_fallback_calls: work.gcd_fallback_calls,
            max_gcd_fallback_input_bits: work
                .gcd_fallback_input_bits
                .max(work.max_gcd_fallback_call_input_bits),
            max_rational_allocations: work.rational_allocations,
            max_rational_allocation_bits: work.max_rational_allocation_bits,
            max_total_rational_allocation_bits: work.total_rational_allocation_bits,
            max_output_bits: work.max_output_bits,
        }
    }

    fn exact_e_corridor_limits_from_work(
        work: &ExactEFiniteHingeCorridorWork,
    ) -> ExactEFiniteHingeCorridorLimits {
        ExactEFiniteHingeCorridorLimits {
            max_authenticated_faces: work.authenticated_faces,
            max_authenticated_hinges: work.authenticated_hinges,
            max_local_y_component_lifts: work.local_y_component_lifts,
            max_thickness_lifts: work.thickness_lifts,
            max_half_thickness_divisions: work.half_thickness_divisions,
            max_scalar_reconstructions: work.scalar_reconstructions,
            max_corridor_vertex_tests: work.corridor_vertex_tests,
            prism: ExactPrismLimits {
                max_prisms: work.prism.prisms,
                max_solid_vertices: work.prism.solid_vertices,
                max_facets: work.prism.facets,
                max_halfspaces: work.prism.halfspaces,
                max_prism_volume_tests: work.prism.prism_volume_tests,
                max_facet_vertex_checks: work.prism.facet_vertex_checks,
                max_plane_triples: work.prism.plane_triples,
                max_singular_plane_triples: work.prism.singular_plane_triples,
                max_nonsingular_solves: work.prism.nonsingular_solves,
                max_membership_tests: work.prism.membership_tests,
                max_candidate_vertices: work.prism.candidate_vertices,
                max_dedup_comparisons: work.prism.dedup_comparisons,
                max_affine_rank_tests: work.prism.affine_rank_tests,
                max_support_plane_vertex_tests: work.prism.support_plane_vertex_tests,
                max_support_pair_tests: work.prism.support_pair_tests,
                max_input_rationals: work.prism.input_rationals,
                max_input_rational_storage_bits: work.prism.max_input_rational_storage_bits,
                max_total_input_storage_bits: work.prism.total_input_storage_bits,
                exact: cayley_limits_from_observed_work(&work.prism.exact),
            },
            exact: cayley_limits_from_observed_work(&work.exact),
        }
    }

    fn oversized_cayley_limits() -> CayleyLimits {
        CayleyLimits {
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
        }
    }

    #[test]
    fn exact_e_corridor_all_structural_and_exact_counters_have_one_short_limits() {
        let model = two_triangle_model();
        let pose = triangular_pose(&model, 135.0);
        let bound = model.bind_pose(&pose).unwrap();
        let exact = triangular_exact_pose(&model, &pose);
        let prerequisite_analysis = authenticated_ef_prerequisite(&exact, 0.1);
        let SingleTriangularHingePrerequisiteResult::Authenticated(prerequisite) =
            &prerequisite_analysis.result
        else {
            panic!("prerequisite");
        };
        let ef_analysis = analyze_axis_aligned_ef_boundary_v1(
            prerequisite,
            &exact,
            bound,
            0.1,
            AxisAlignedEfBoundaryLimits::default(),
        )
        .unwrap();
        let ef = ef_capability(&ef_analysis);
        let baseline = analyze_exact_e_finite_hinge_corridor_v1(
            &prerequisite_analysis,
            Some(ef),
            &exact,
            bound,
            0.1,
            ExactEFiniteHingeCorridorLimits::default(),
        )
        .unwrap();
        let exact_limits = exact_e_corridor_limits_from_work(&baseline.work);
        let exact_analysis = analyze_exact_e_finite_hinge_corridor_v1(
            &prerequisite_analysis,
            Some(ef),
            &exact,
            bound,
            0.1,
            exact_limits,
        )
        .unwrap();
        assert!(matches!(
            exact_analysis.result,
            ExactEFiniteHingeCorridorResult::Contained(_)
        ));
        assert_eq!(exact_analysis.work, baseline.work);

        let assert_one_short = |resource: &str, limits: ExactEFiniteHingeCorridorLimits| {
            assert!(
                matches!(
                    analyze_exact_e_finite_hinge_corridor_v1(
                        &prerequisite_analysis,
                        Some(ef),
                        &exact,
                        bound,
                        0.1,
                        limits,
                    ),
                    Err(ExactEFiniteHingeCorridorError::ResourceLimitExceeded)
                ),
                "{resource}"
            );
        };
        macro_rules! structural_one_short {
            ($field:ident) => {
                if exact_limits.$field > 0 {
                    let mut limits = exact_limits;
                    limits.$field -= 1;
                    assert_one_short(stringify!($field), limits);
                }
            };
        }
        structural_one_short!(max_authenticated_faces);
        structural_one_short!(max_authenticated_hinges);
        structural_one_short!(max_local_y_component_lifts);
        structural_one_short!(max_thickness_lifts);
        structural_one_short!(max_half_thickness_divisions);
        structural_one_short!(max_scalar_reconstructions);
        structural_one_short!(max_corridor_vertex_tests);

        macro_rules! prism_one_short {
            ($field:ident) => {
                if exact_limits.prism.$field > 0 {
                    let mut limits = exact_limits;
                    limits.prism.$field -= 1;
                    assert_one_short(concat!("prism.", stringify!($field)), limits);
                }
            };
        }
        prism_one_short!(max_prisms);
        prism_one_short!(max_solid_vertices);
        prism_one_short!(max_facets);
        prism_one_short!(max_halfspaces);
        prism_one_short!(max_prism_volume_tests);
        prism_one_short!(max_facet_vertex_checks);
        prism_one_short!(max_plane_triples);
        prism_one_short!(max_singular_plane_triples);
        prism_one_short!(max_nonsingular_solves);
        prism_one_short!(max_membership_tests);
        prism_one_short!(max_candidate_vertices);
        prism_one_short!(max_dedup_comparisons);
        prism_one_short!(max_affine_rank_tests);
        prism_one_short!(max_support_plane_vertex_tests);
        prism_one_short!(max_support_pair_tests);
        prism_one_short!(max_input_rationals);
        prism_one_short!(max_input_rational_storage_bits);
        prism_one_short!(max_total_input_storage_bits);

        macro_rules! prism_exact_one_short {
            ($field:ident) => {
                if exact_limits.prism.exact.$field > 0 {
                    let mut limits = exact_limits;
                    limits.prism.exact.$field -= 1;
                    assert_one_short(concat!("prism.exact.", stringify!($field)), limits);
                }
            };
        }
        prism_exact_one_short!(max_interval_operations);
        prism_exact_one_short!(max_shift_bits);
        prism_exact_one_short!(max_intermediate_bits);
        prism_exact_one_short!(max_gcd_fallback_calls);
        prism_exact_one_short!(max_gcd_fallback_input_bits);
        prism_exact_one_short!(max_rational_allocations);
        prism_exact_one_short!(max_rational_allocation_bits);
        prism_exact_one_short!(max_total_rational_allocation_bits);

        macro_rules! combined_exact_one_short {
            ($field:ident) => {
                if exact_limits.exact.$field > 0 {
                    let mut limits = exact_limits;
                    limits.exact.$field -= 1;
                    assert_one_short(concat!("exact.", stringify!($field)), limits);
                }
            };
        }
        combined_exact_one_short!(max_interval_operations);
        combined_exact_one_short!(max_shift_bits);
        combined_exact_one_short!(max_intermediate_bits);
        combined_exact_one_short!(max_gcd_fallback_calls);
        combined_exact_one_short!(max_gcd_fallback_input_bits);
        combined_exact_one_short!(max_rational_allocations);
        combined_exact_one_short!(max_rational_allocation_bits);
        combined_exact_one_short!(max_total_rational_allocation_bits);

        let oversized_exact = oversized_cayley_limits();
        let oversized = ExactEFiniteHingeCorridorLimits {
            max_authenticated_faces: usize::MAX,
            max_authenticated_hinges: usize::MAX,
            max_local_y_component_lifts: usize::MAX,
            max_thickness_lifts: usize::MAX,
            max_half_thickness_divisions: usize::MAX,
            max_scalar_reconstructions: usize::MAX,
            max_corridor_vertex_tests: usize::MAX,
            prism: ExactPrismLimits {
                max_prisms: usize::MAX,
                max_solid_vertices: usize::MAX,
                max_facets: usize::MAX,
                max_halfspaces: usize::MAX,
                max_prism_volume_tests: usize::MAX,
                max_facet_vertex_checks: usize::MAX,
                max_plane_triples: usize::MAX,
                max_singular_plane_triples: usize::MAX,
                max_nonsingular_solves: usize::MAX,
                max_membership_tests: usize::MAX,
                max_candidate_vertices: usize::MAX,
                max_dedup_comparisons: usize::MAX,
                max_affine_rank_tests: usize::MAX,
                max_support_plane_vertex_tests: usize::MAX,
                max_support_pair_tests: usize::MAX,
                max_input_rationals: usize::MAX,
                max_input_rational_storage_bits: usize::MAX,
                max_total_input_storage_bits: usize::MAX,
                exact: oversized_exact,
            },
            exact: oversized_exact,
        };
        let projected = analyze_exact_e_finite_hinge_corridor_v1(
            &prerequisite_analysis,
            Some(ef),
            &exact,
            bound,
            0.1,
            oversized,
        )
        .unwrap();
        assert!(matches!(
            projected.result,
            ExactEFiniteHingeCorridorResult::Contained(_)
        ));
        assert_eq!(projected.work, baseline.work);
    }

    #[test]
    fn direct_f_400mm_matrix_fixes_96_contained_24_outside_and_minimum_excess_exactly() {
        let square = [(0.0, 0.0), (400.0, 0.0), (400.0, 400.0), (0.0, 400.0)];
        let models = [
            (
                "mountain/source",
                two_triangle_model_with_options(EdgeKind::Mountain, false, square, 501),
            ),
            (
                "mountain/reordered",
                two_triangle_model_with_options(EdgeKind::Mountain, true, square, 502),
            ),
            (
                "valley/source",
                two_triangle_model_with_options(EdgeKind::Valley, false, square, 503),
            ),
            (
                "valley/reordered",
                two_triangle_model_with_options(EdgeKind::Valley, true, square, 504),
            ),
        ];
        let mut cases = 0_usize;
        let mut contained_by_angle = [0_usize; 5];
        let mut outside_by_angle = [0_usize; 5];
        for (fixture, model) in &models {
            for root in model.face_ids() {
                for thickness in [0.1, 1.0, 3.0] {
                    for (angle_index, angle) in
                        [0.0, 10.0, 90.0, 135.0, 179.0].into_iter().enumerate()
                    {
                        cases += 1;
                        let pose = triangular_pose_with_root(model, angle, *root);
                        let bound = model.bind_pose(&pose).unwrap();
                        let exact = triangular_exact_pose(model, &pose);
                        let prerequisite_analysis =
                            authenticated_ef_prerequisite(&exact, thickness);
                        let SingleTriangularHingePrerequisiteResult::Authenticated(prerequisite) =
                            &prerequisite_analysis.result
                        else {
                            panic!(
                                "{fixture}, root {root:?}, {thickness} mm at {angle}: prerequisite"
                            );
                        };
                        let ef_analysis = analyze_axis_aligned_ef_boundary_v1(
                            prerequisite,
                            &exact,
                            bound,
                            thickness,
                            AxisAlignedEfBoundaryLimits::default(),
                        )
                        .unwrap();
                        let ef = ef_capability(&ef_analysis);
                        let exact_e_analysis = analyze_exact_e_finite_hinge_corridor_v1(
                            &prerequisite_analysis,
                            Some(ef),
                            &exact,
                            bound,
                            thickness,
                            ExactEFiniteHingeCorridorLimits::default(),
                        )
                        .unwrap();
                        let exact_e = exact_e_corridor_capability(&exact_e_analysis);
                        let analysis = analyze_direct_f_finite_hinge_corridor_v1(
                            &prerequisite_analysis,
                            Some(ef),
                            &exact_e_analysis,
                            &exact,
                            bound,
                            thickness,
                            DirectFFiniteHingeCorridorLimits::default(),
                        )
                        .unwrap_or_else(|error| {
                            panic!("{fixture}, root {root:?}, {thickness} mm at {angle}: {error:?}")
                        });
                        assert_eq!(
                            analysis.work.prism.input_rationals, 36,
                            "direct-F uses the separately validated six-vertex prism entry"
                        );
                        assert_eq!(analysis.work.phase2b_exact, exact_e_analysis.work.exact);
                        assert_cayley_work_is_monotone(
                            &analysis.work.phase2b_exact,
                            &analysis.work.exact,
                        );
                        assert!(analysis.work.corridor_vertex_tests <= 120);
                        assert!(
                            analysis.work.corridor_vertex_tests
                                <= analysis.work.prism.candidate_vertices
                        );
                        let capability = match &analysis.result {
                            DirectFFiniteHingeCorridorResult::Contained(capability) => {
                                contained_by_angle[angle_index] += 1;
                                assert_ne!(
                                    angle, 90.0,
                                    "literal direct-F must not round strict outside into containment"
                                );
                                capability
                            }
                            DirectFFiniteHingeCorridorResult::Outside(outside) => {
                                outside_by_angle[angle_index] += 1;
                                assert_eq!(
                                    angle, 90.0,
                                    "{fixture}, root {root:?}, {thickness} mm: {outside:?}"
                                );
                                assert_eq!(outside.interaction_kind, exact_e.interaction_kind());
                                assert_eq!(outside.first_outside_vertex_index, 0);
                                assert_eq!(outside.outside_vertex_count, 2);
                                assert!(
                                    analysis.work.corridor_vertex_tests
                                        > outside.outside_vertex_count,
                                    "outside detection must still scan every retained vertex"
                                );
                                assert!(!outside.axial_before_start);
                                assert!(!outside.axial_after_end);
                                assert!(outside.radial_outside);
                                assert!(
                                    outside
                                        .first_radial_excess
                                        .as_ref()
                                        .is_some_and(BigRational::is_positive)
                                );
                                if cases == 3 {
                                    let excess =
                                        outside.first_radial_excess.as_ref().expect("excess");
                                    // Minimal matrix fixture: Mountain/source,
                                    // first root, 0.1 mm, 90 degrees.  The
                                    // strict radial excess is about
                                    // 5.46864692612954e-14 mm^4; it must not
                                    // be rounded into a contained capability.
                                    assert_eq!(
                                        excess,
                                        &BigRational::new(
                                            BigInt::parse_bytes(
                                                b"44993417196633807545963059937356168830766334170625",
                                                10,
                                            )
                                            .unwrap(),
                                            BigInt::parse_bytes(
                                                b"822752278660603021077484591278675252491367932816789931674304512",
                                                10,
                                            )
                                            .unwrap(),
                                        )
                                    );
                                }
                                continue;
                            }
                            other => {
                                panic!(
                                    "{fixture}, root {root:?}, {thickness} mm at {angle}: {other:?}"
                                );
                            }
                        };
                        let expected_kind = if angle == 0.0 {
                            ExactEFiniteHingeInteractionKind::BoundaryAreaContact
                        } else {
                            ExactEFiniteHingeInteractionKind::PositiveVolume
                        };
                        assert_eq!(
                            capability.interaction_kind(),
                            expected_kind,
                            "{fixture}, root {root:?}, {thickness} mm at {angle}"
                        );
                        assert_eq!(capability.interaction_kind(), exact_e.interaction_kind());
                        assert_eq!(capability.length_squared(), &integer(320_000));
                        assert_eq!(
                            capability.half_thickness(),
                            &(BigRational::from_float(thickness).unwrap() / integer(2))
                        );
                        if angle == 0.0 {
                            assert_eq!(capability.cosine_half_squared(), &integer(1));
                            assert_eq!(analysis.work.corridor_vertex_tests, 4);
                        } else {
                            assert!(analysis.work.corridor_vertex_tests >= 4);
                        }
                        let rebound = revalidate_direct_f_finite_hinge_corridor_v1(
                            capability,
                            prerequisite,
                            ef,
                            exact_e,
                            &exact,
                            bound,
                            thickness,
                        )
                        .expect(
                            "phase 1, EF, exact-E, all F bits, axis parent, pose and thickness",
                        );
                        assert!(std::ptr::eq(rebound.capability, capability.as_ref()));
                    }
                }
            }
        }
        assert_eq!(cases, 120);
        assert_eq!(contained_by_angle, [24, 24, 0, 24, 24]);
        assert_eq!(outside_by_angle, [0, 0, 24, 0, 0]);
    }

    #[test]
    fn direct_f_affine_c2_400mm_matrix_is_120_contained_unadmitted_diagnostics() {
        let square = [(0.0, 0.0), (400.0, 0.0), (400.0, 400.0), (0.0, 400.0)];
        let models = [
            (
                "mountain/source",
                two_triangle_model_with_options(EdgeKind::Mountain, false, square, 601),
            ),
            (
                "mountain/reordered",
                two_triangle_model_with_options(EdgeKind::Mountain, true, square, 602),
            ),
            (
                "valley/source",
                two_triangle_model_with_options(EdgeKind::Valley, false, square, 603),
            ),
            (
                "valley/reordered",
                two_triangle_model_with_options(EdgeKind::Valley, true, square, 604),
            ),
        ];
        let mut cases = 0_usize;
        let mut contained_unadmitted_by_angle = [0_usize; 5];
        let mut endpoint_mismatches_by_angle = [0_usize; 5];
        for (fixture, model) in &models {
            for root in model.face_ids() {
                for thickness in [0.1, 1.0, 3.0] {
                    for (angle_index, angle) in
                        [0.0, 10.0, 90.0, 135.0, 179.0].into_iter().enumerate()
                    {
                        cases += 1;
                        let pose = triangular_pose_with_root(model, angle, *root);
                        let bound = model.bind_pose(&pose).unwrap();
                        let exact = triangular_exact_pose(model, &pose);
                        let prerequisite_analysis =
                            authenticated_ef_prerequisite(&exact, thickness);
                        let SingleTriangularHingePrerequisiteResult::Authenticated(prerequisite) =
                            &prerequisite_analysis.result
                        else {
                            panic!(
                                "{fixture}, root {root:?}, {thickness} mm at {angle}: prerequisite"
                            );
                        };
                        let ef_analysis = analyze_axis_aligned_ef_boundary_v1(
                            prerequisite,
                            &exact,
                            bound,
                            thickness,
                            AxisAlignedEfBoundaryLimits::default(),
                        )
                        .unwrap();
                        let ef = ef_capability(&ef_analysis);
                        let exact_e_analysis = analyze_exact_e_finite_hinge_corridor_v1(
                            &prerequisite_analysis,
                            Some(ef),
                            &exact,
                            bound,
                            thickness,
                            ExactEFiniteHingeCorridorLimits::default(),
                        )
                        .unwrap();
                        let exact_e = exact_e_corridor_capability(&exact_e_analysis);
                        let analysis = analyze_direct_f_affine_hinge_corridor_v1(
                            &prerequisite_analysis,
                            Some(ef),
                            &exact_e_analysis,
                            &exact,
                            bound,
                            thickness,
                            DirectFAffineHingeCorridorLimits::default(),
                        )
                        .unwrap_or_else(|error| {
                            panic!("{fixture}, root {root:?}, {thickness} mm at {angle}: {error:?}")
                        });
                        let diagnostic = direct_f_affine_diagnostic(&analysis);
                        contained_unadmitted_by_angle[angle_index] += 1;
                        let expected_kind = if angle == 0.0 {
                            ExactEFiniteHingeInteractionKind::BoundaryAreaContact
                        } else {
                            ExactEFiniteHingeInteractionKind::PositiveVolume
                        };
                        assert_eq!(
                            diagnostic.interaction_kind(),
                            expected_kind,
                            "{fixture}, root {root:?}, {thickness} mm at {angle}"
                        );
                        assert_eq!(diagnostic.interaction_kind(), exact_e.interaction_kind());
                        assert_eq!(diagnostic.length_squared(), &integer(320_000));
                        assert_eq!(
                            diagnostic.half_thickness(),
                            &(BigRational::from_float(thickness).unwrap() / integer(2))
                        );
                        assert!(diagnostic.radius_squared() <= diagnostic.length_squared());
                        assert!(diagnostic.corridor_vertex_count() >= 4);
                        assert_eq!(
                            diagnostic.corridor_affine_rank(),
                            if angle == 0.0 { 2 } else { 3 },
                            "{fixture}, root {root:?}, {thickness} mm at {angle}"
                        );
                        assert!(diagnostic.shared_endpoint_mismatch_count() <= 2);
                        endpoint_mismatches_by_angle[angle_index] +=
                            diagnostic.shared_endpoint_mismatch_count();
                        assert_eq!(analysis.work.plane_triples, 120);
                        assert_eq!(
                            analysis.work.singular_plane_triples + analysis.work.nonsingular_solves,
                            120
                        );
                        assert_eq!(
                            analysis.work.membership_tests,
                            analysis.work.nonsingular_solves * 10
                        );
                        assert_eq!(analysis.work.normal_rank_tests, 10);
                        assert_eq!(analysis.work.recession_normal_pairs, 45);
                        assert_eq!(analysis.work.signed_recession_tests, 90);
                        assert_eq!(analysis.work.recession_membership_tests, 900);
                        assert_eq!(analysis.work.shared_endpoint_equality_tests, 2);
                        assert!(
                            analysis.sealed_diagnostic_result_and_work().is_some(),
                            "the diagnostic work seal must match its public snapshot"
                        );
                        let rebound = revalidate_direct_f_affine_hinge_corridor_diagnostic_v1(
                            diagnostic,
                            prerequisite,
                            ef,
                            exact_e,
                            &exact,
                            bound,
                            thickness,
                        )
                        .expect("C2 diagnostic provenance must revalidate");
                        assert!(std::ptr::eq(rebound, diagnostic));
                    }
                }
            }
        }
        assert_eq!(cases, 120);
        assert_eq!(contained_unadmitted_by_angle, [24, 24, 24, 24, 24]);
        assert_eq!(
            endpoint_mismatches_by_angle,
            [0, 24, 24, 24, 24],
            "every non-cardinal fixture retains one literal-F endpoint drift \
             yet remains diagnostic-only ContainedUnadmitted"
        );
    }

    #[test]
    fn direct_f_affine_c2_propagates_all_24_flat_fold_cases_without_c2_work() {
        let square = [(0.0, 0.0), (400.0, 0.0), (400.0, 400.0), (0.0, 400.0)];
        let models = [
            two_triangle_model_with_options(EdgeKind::Mountain, false, square, 605),
            two_triangle_model_with_options(EdgeKind::Mountain, true, square, 606),
            two_triangle_model_with_options(EdgeKind::Valley, false, square, 607),
            two_triangle_model_with_options(EdgeKind::Valley, true, square, 608),
        ];
        let mut cases = 0_usize;
        for model in &models {
            for root in model.face_ids() {
                let pose = triangular_pose_with_root(model, 180.0, *root);
                let bound = model.bind_pose(&pose).unwrap();
                let exact = triangular_exact_pose(model, &pose);
                for thickness in [0.1, 1.0, 3.0] {
                    cases += 1;
                    let prerequisite_analysis = authenticated_ef_prerequisite(&exact, thickness);
                    let exact_e_analysis = analyze_exact_e_finite_hinge_corridor_v1(
                        &prerequisite_analysis,
                        None,
                        &exact,
                        bound,
                        thickness,
                        ExactEFiniteHingeCorridorLimits::default(),
                    )
                    .unwrap();
                    let analysis = analyze_direct_f_affine_hinge_corridor_v1(
                        &prerequisite_analysis,
                        None,
                        &exact_e_analysis,
                        &exact,
                        bound,
                        thickness,
                        DirectFAffineHingeCorridorLimits::default(),
                    )
                    .unwrap();
                    assert!(matches!(
                        analysis.result,
                        DirectFAffineHingeCorridorResult::LayerOffsetUnmodeled
                    ));
                    assert_eq!(
                        analysis.work,
                        DirectFAffineHingeCorridorWork::default(),
                        "180-degree propagation must not start C2 work"
                    );
                }
            }
        }
        assert_eq!(cases, 24);
    }

    fn direct_f_affine_limits_from_work(
        work: &DirectFAffineHingeCorridorWork,
    ) -> DirectFAffineHingeCorridorLimits {
        DirectFAffineHingeCorridorLimits {
            max_authenticated_faces: work.authenticated_faces,
            max_authenticated_hinges: work.authenticated_hinges,
            max_face_transform_bit_bindings: work.face_transform_bit_bindings,
            max_hinge_parent_transform_bit_bindings: work.hinge_parent_transform_bit_bindings,
            max_shared_endpoint_equality_tests: work.shared_endpoint_equality_tests,
            max_transform_scalar_lifts: work.transform_scalar_lifts,
            max_source_coordinate_lifts: work.source_coordinate_lifts,
            max_affine_point_reconstructions: work.affine_point_reconstructions,
            max_solid_vertex_constructions: work.solid_vertex_constructions,
            max_thickness_lifts: work.thickness_lifts,
            max_half_thickness_divisions: work.half_thickness_divisions,
            max_canonical_inward_directions: work.canonical_inward_directions,
            max_dual_basis_inversions: work.dual_basis_inversions,
            max_affine_half_prisms: work.affine_half_prisms,
            max_halfspaces: work.halfspaces,
            max_plane_triples: work.plane_triples,
            max_singular_plane_triples: work.singular_plane_triples,
            max_nonsingular_solves: work.nonsingular_solves,
            max_membership_tests: work.membership_tests,
            max_corridor_vertices: work.corridor_vertices,
            max_dedup_comparisons: work.dedup_comparisons,
            max_normal_rank_tests: work.normal_rank_tests,
            max_recession_normal_pairs: work.recession_normal_pairs,
            max_signed_recession_tests: work.signed_recession_tests,
            max_recession_membership_tests: work.recession_membership_tests,
            max_prism_vertex_halfspace_tests: work.prism_vertex_halfspace_tests,
            max_corridor_affine_rank_tests: work.corridor_affine_rank_tests,
            max_gram_inversions: work.gram_inversions,
            max_gram_positive_definite_tests: work.gram_positive_definite_tests,
            max_gram_quadratic_tests: work.gram_quadratic_tests,
            max_axial_tests: work.axial_tests,
            prism: ExactPrismLimits {
                max_prisms: work.prism.prisms,
                max_solid_vertices: work.prism.solid_vertices,
                max_facets: work.prism.facets,
                max_halfspaces: work.prism.halfspaces,
                max_prism_volume_tests: work.prism.prism_volume_tests,
                max_facet_vertex_checks: work.prism.facet_vertex_checks,
                max_plane_triples: work.prism.plane_triples,
                max_singular_plane_triples: work.prism.singular_plane_triples,
                max_nonsingular_solves: work.prism.nonsingular_solves,
                max_membership_tests: work.prism.membership_tests,
                max_candidate_vertices: work.prism.candidate_vertices,
                max_dedup_comparisons: work.prism.dedup_comparisons,
                max_affine_rank_tests: work.prism.affine_rank_tests,
                max_support_plane_vertex_tests: work.prism.support_plane_vertex_tests,
                max_support_pair_tests: work.prism.support_pair_tests,
                max_input_rationals: work.prism.input_rationals,
                max_input_rational_storage_bits: work.prism.max_input_rational_storage_bits,
                max_total_input_storage_bits: work.prism.total_input_storage_bits,
                exact: cayley_limits_from_observed_work(&work.prism.exact),
            },
            literal_exact: cayley_limits_from_observed_work(&work.literal_exact),
            local_exact: cayley_limits_from_observed_work(&work.local_exact),
            exact: cayley_limits_from_observed_work(&work.exact),
        }
    }

    #[test]
    fn direct_f_affine_c2_all_structural_and_exact_counters_have_one_short_limits() {
        let model = two_triangle_model_with_options(
            EdgeKind::Mountain,
            false,
            [(0.0, 0.0), (400.0, 0.0), (400.0, 400.0), (0.0, 400.0)],
            609,
        );
        let pose = triangular_pose(&model, 135.0);
        let bound = model.bind_pose(&pose).unwrap();
        let exact = triangular_exact_pose(&model, &pose);
        let prerequisite_analysis = authenticated_ef_prerequisite(&exact, 0.1);
        let SingleTriangularHingePrerequisiteResult::Authenticated(prerequisite) =
            &prerequisite_analysis.result
        else {
            panic!("prerequisite");
        };
        let ef_analysis = analyze_axis_aligned_ef_boundary_v1(
            prerequisite,
            &exact,
            bound,
            0.1,
            AxisAlignedEfBoundaryLimits::default(),
        )
        .unwrap();
        let ef = ef_capability(&ef_analysis);
        let exact_e_analysis = analyze_exact_e_finite_hinge_corridor_v1(
            &prerequisite_analysis,
            Some(ef),
            &exact,
            bound,
            0.1,
            ExactEFiniteHingeCorridorLimits::default(),
        )
        .unwrap();
        let baseline = analyze_direct_f_affine_hinge_corridor_v1(
            &prerequisite_analysis,
            Some(ef),
            &exact_e_analysis,
            &exact,
            bound,
            0.1,
            DirectFAffineHingeCorridorLimits::default(),
        )
        .unwrap();
        assert!(matches!(
            baseline.result,
            DirectFAffineHingeCorridorResult::ContainedUnadmitted(_)
        ));
        assert_eq!(baseline.work.phase2b_exact, exact_e_analysis.work.exact);
        let recombined_exact = exact_e_analysis
            .work
            .exact
            .checked_merge(
                &baseline.work.literal_exact,
                &DirectFAffineHingeCorridorLimits::default().exact,
                None,
                STAGE,
            )
            .and_then(|work| {
                work.checked_merge(
                    &baseline.work.prism.exact,
                    &DirectFAffineHingeCorridorLimits::default().exact,
                    None,
                    STAGE,
                )
            })
            .and_then(|work| {
                work.checked_merge(
                    &baseline.work.local_exact,
                    &DirectFAffineHingeCorridorLimits::default().exact,
                    None,
                    STAGE,
                )
            })
            .unwrap();
        assert_eq!(
            baseline.work.exact, recombined_exact,
            "Phase 2-B, literal-F rerun, prism, and C2-local deltas are each merged once"
        );
        let exact_limits = direct_f_affine_limits_from_work(&baseline.work);
        let exact_analysis = analyze_direct_f_affine_hinge_corridor_v1(
            &prerequisite_analysis,
            Some(ef),
            &exact_e_analysis,
            &exact,
            bound,
            0.1,
            exact_limits,
        )
        .unwrap();
        assert!(matches!(
            exact_analysis.result,
            DirectFAffineHingeCorridorResult::ContainedUnadmitted(_)
        ));
        assert_eq!(exact_analysis.work, baseline.work);

        let assert_one_short = |resource: &str, limits: DirectFAffineHingeCorridorLimits| {
            assert!(
                matches!(
                    analyze_direct_f_affine_hinge_corridor_v1(
                        &prerequisite_analysis,
                        Some(ef),
                        &exact_e_analysis,
                        &exact,
                        bound,
                        0.1,
                        limits,
                    ),
                    Err(DirectFAffineHingeCorridorError::ResourceLimitExceeded)
                ),
                "{resource}"
            );
        };
        macro_rules! structural_one_short {
            ($field:ident) => {
                if exact_limits.$field > 0 {
                    let mut limits = exact_limits;
                    limits.$field -= 1;
                    assert_one_short(stringify!($field), limits);
                }
            };
        }
        structural_one_short!(max_authenticated_faces);
        structural_one_short!(max_authenticated_hinges);
        structural_one_short!(max_face_transform_bit_bindings);
        structural_one_short!(max_hinge_parent_transform_bit_bindings);
        structural_one_short!(max_shared_endpoint_equality_tests);
        structural_one_short!(max_transform_scalar_lifts);
        structural_one_short!(max_source_coordinate_lifts);
        structural_one_short!(max_affine_point_reconstructions);
        structural_one_short!(max_solid_vertex_constructions);
        structural_one_short!(max_thickness_lifts);
        structural_one_short!(max_half_thickness_divisions);
        structural_one_short!(max_canonical_inward_directions);
        structural_one_short!(max_dual_basis_inversions);
        structural_one_short!(max_affine_half_prisms);
        structural_one_short!(max_halfspaces);
        structural_one_short!(max_plane_triples);
        structural_one_short!(max_singular_plane_triples);
        structural_one_short!(max_nonsingular_solves);
        structural_one_short!(max_membership_tests);
        structural_one_short!(max_corridor_vertices);
        structural_one_short!(max_dedup_comparisons);
        structural_one_short!(max_normal_rank_tests);
        structural_one_short!(max_recession_normal_pairs);
        structural_one_short!(max_signed_recession_tests);
        structural_one_short!(max_recession_membership_tests);
        structural_one_short!(max_prism_vertex_halfspace_tests);
        structural_one_short!(max_corridor_affine_rank_tests);
        structural_one_short!(max_gram_inversions);
        structural_one_short!(max_gram_positive_definite_tests);
        structural_one_short!(max_gram_quadratic_tests);
        structural_one_short!(max_axial_tests);

        macro_rules! prism_one_short {
            ($field:ident) => {
                if exact_limits.prism.$field > 0 {
                    let mut limits = exact_limits;
                    limits.prism.$field -= 1;
                    assert_one_short(concat!("prism.", stringify!($field)), limits);
                }
            };
        }
        prism_one_short!(max_prisms);
        prism_one_short!(max_solid_vertices);
        prism_one_short!(max_facets);
        prism_one_short!(max_halfspaces);
        prism_one_short!(max_prism_volume_tests);
        prism_one_short!(max_facet_vertex_checks);
        prism_one_short!(max_plane_triples);
        prism_one_short!(max_singular_plane_triples);
        prism_one_short!(max_nonsingular_solves);
        prism_one_short!(max_membership_tests);
        prism_one_short!(max_candidate_vertices);
        prism_one_short!(max_dedup_comparisons);
        prism_one_short!(max_affine_rank_tests);
        prism_one_short!(max_support_plane_vertex_tests);
        prism_one_short!(max_support_pair_tests);
        prism_one_short!(max_input_rationals);
        prism_one_short!(max_input_rational_storage_bits);
        prism_one_short!(max_total_input_storage_bits);

        macro_rules! local_exact_one_short {
            ($bucket:ident, $field:ident) => {
                if exact_limits.$bucket.$field > 0 {
                    let mut limits = exact_limits;
                    limits.$bucket.$field -= 1;
                    assert_one_short(
                        concat!(stringify!($bucket), ".", stringify!($field)),
                        limits,
                    );
                }
            };
        }
        macro_rules! prism_exact_one_short {
            ($field:ident) => {
                if exact_limits.prism.exact.$field > 0 {
                    let mut limits = exact_limits;
                    limits.prism.exact.$field -= 1;
                    assert_one_short(concat!("prism.exact.", stringify!($field)), limits);
                }
            };
        }
        macro_rules! all_exact_one_short {
            ($macro_name:ident, $($prefix:ident),+ $(,)?) => {
                $(
                    $macro_name!($prefix, max_precision_rounds);
                    $macro_name!($prefix, max_guard_bits);
                    $macro_name!($prefix, max_candidate_bits);
                    $macro_name!($prefix, max_machin_terms_per_series);
                    $macro_name!($prefix, max_trig_terms_per_series);
                    $macro_name!($prefix, max_sqrt_refinements);
                    $macro_name!($prefix, max_interval_operations);
                    $macro_name!($prefix, max_shift_bits);
                    $macro_name!($prefix, max_intermediate_bits);
                    $macro_name!($prefix, max_gcd_fallback_calls);
                    $macro_name!($prefix, max_gcd_fallback_input_bits);
                    $macro_name!($prefix, max_rational_allocations);
                    $macro_name!($prefix, max_rational_allocation_bits);
                    $macro_name!($prefix, max_total_rational_allocation_bits);
                    $macro_name!($prefix, max_output_bits);
                )+
            };
        }
        all_exact_one_short!(local_exact_one_short, literal_exact, local_exact, exact);
        prism_exact_one_short!(max_precision_rounds);
        prism_exact_one_short!(max_guard_bits);
        prism_exact_one_short!(max_candidate_bits);
        prism_exact_one_short!(max_machin_terms_per_series);
        prism_exact_one_short!(max_trig_terms_per_series);
        prism_exact_one_short!(max_sqrt_refinements);
        prism_exact_one_short!(max_interval_operations);
        prism_exact_one_short!(max_shift_bits);
        prism_exact_one_short!(max_intermediate_bits);
        prism_exact_one_short!(max_gcd_fallback_calls);
        prism_exact_one_short!(max_gcd_fallback_input_bits);
        prism_exact_one_short!(max_rational_allocations);
        prism_exact_one_short!(max_rational_allocation_bits);
        prism_exact_one_short!(max_total_rational_allocation_bits);
        prism_exact_one_short!(max_output_bits);
    }

    #[test]
    fn direct_f_affine_c2_rejects_aba_foreign_tokens_thickness_and_all_36_transform_bits() {
        let model = two_triangle_model_with_options(
            EdgeKind::Mountain,
            false,
            [(0.0, 0.0), (400.0, 0.0), (400.0, 400.0), (0.0, 400.0)],
            610,
        );
        let pose = triangular_pose(&model, 135.0);
        let bound = model.bind_pose(&pose).unwrap();
        let exact = triangular_exact_pose(&model, &pose);
        let independent_exact = triangular_exact_pose(&model, &pose);
        let prerequisite_analysis = authenticated_ef_prerequisite(&exact, 0.1);
        let duplicate_prerequisite_analysis = authenticated_ef_prerequisite(&exact, 0.1);
        let SingleTriangularHingePrerequisiteResult::Authenticated(prerequisite) =
            &prerequisite_analysis.result
        else {
            panic!("prerequisite");
        };
        let SingleTriangularHingePrerequisiteResult::Authenticated(duplicate_prerequisite) =
            &duplicate_prerequisite_analysis.result
        else {
            panic!("duplicate prerequisite");
        };
        let ef_analysis = analyze_axis_aligned_ef_boundary_v1(
            prerequisite,
            &exact,
            bound,
            0.1,
            AxisAlignedEfBoundaryLimits::default(),
        )
        .unwrap();
        let duplicate_ef_analysis = analyze_axis_aligned_ef_boundary_v1(
            prerequisite,
            &exact,
            bound,
            0.1,
            AxisAlignedEfBoundaryLimits::default(),
        )
        .unwrap();
        let ef = ef_capability(&ef_analysis);
        let duplicate_ef = ef_capability(&duplicate_ef_analysis);
        let exact_e_analysis = analyze_exact_e_finite_hinge_corridor_v1(
            &prerequisite_analysis,
            Some(ef),
            &exact,
            bound,
            0.1,
            ExactEFiniteHingeCorridorLimits::default(),
        )
        .unwrap();
        let duplicate_exact_e_analysis = analyze_exact_e_finite_hinge_corridor_v1(
            &prerequisite_analysis,
            Some(ef),
            &exact,
            bound,
            0.1,
            ExactEFiniteHingeCorridorLimits::default(),
        )
        .unwrap();
        let exact_e = exact_e_corridor_capability(&exact_e_analysis);
        let duplicate_exact_e = exact_e_corridor_capability(&duplicate_exact_e_analysis);
        let mut analysis = analyze_direct_f_affine_hinge_corridor_v1(
            &prerequisite_analysis,
            Some(ef),
            &exact_e_analysis,
            &exact,
            bound,
            0.1,
            DirectFAffineHingeCorridorLimits::default(),
        )
        .unwrap();

        {
            let DirectFAffineHingeCorridorResult::ContainedUnadmitted(diagnostic) =
                &mut analysis.result
            else {
                panic!("contained unadmitted diagnostic");
            };
            assert!(
                revalidate_direct_f_affine_hinge_corridor_diagnostic_v1(
                    diagnostic,
                    prerequisite,
                    ef,
                    exact_e,
                    &exact,
                    bound,
                    0.1,
                )
                .is_some()
            );
            assert!(
                revalidate_direct_f_affine_hinge_corridor_diagnostic_v1(
                    diagnostic,
                    duplicate_prerequisite,
                    ef,
                    exact_e,
                    &exact,
                    bound,
                    0.1,
                )
                .is_none(),
                "independently issued prerequisite"
            );
            assert!(
                revalidate_direct_f_affine_hinge_corridor_diagnostic_v1(
                    diagnostic,
                    prerequisite,
                    duplicate_ef,
                    exact_e,
                    &exact,
                    bound,
                    0.1,
                )
                .is_none(),
                "independently issued E/F boundary"
            );
            assert!(
                revalidate_direct_f_affine_hinge_corridor_diagnostic_v1(
                    diagnostic,
                    prerequisite,
                    ef,
                    duplicate_exact_e,
                    &exact,
                    bound,
                    0.1,
                )
                .is_none(),
                "independently issued exact-E corridor"
            );
            assert!(
                revalidate_direct_f_affine_hinge_corridor_diagnostic_v1(
                    diagnostic,
                    prerequisite,
                    ef,
                    exact_e,
                    &independent_exact,
                    bound,
                    0.1,
                )
                .is_none(),
                "independently regenerated exact pose"
            );
            assert!(
                revalidate_direct_f_affine_hinge_corridor_diagnostic_v1(
                    diagnostic,
                    prerequisite,
                    ef,
                    exact_e,
                    &exact,
                    bound,
                    next_up(0.1),
                )
                .is_none(),
                "one-ULP paper thickness"
            );

            let aba_pose = triangular_pose(&model, 135.0);
            let aba_bound = model.bind_pose(&aba_pose).unwrap();
            let rerooted_pose = triangular_pose_with_root(&model, 135.0, model.face_ids()[1]);
            let rerooted_bound = model.bind_pose(&rerooted_pose).unwrap();
            let one_ulp_pose = triangular_pose(&model, next_up(135.0));
            let one_ulp_bound = model.bind_pose(&one_ulp_pose).unwrap();
            for (name, mismatched_bound) in [
                ("same-angle ABA", aba_bound),
                ("different fixed root", rerooted_bound),
                ("one-ULP angle", one_ulp_bound),
            ] {
                assert!(
                    revalidate_direct_f_affine_hinge_corridor_diagnostic_v1(
                        diagnostic,
                        prerequisite,
                        ef,
                        exact_e,
                        &exact,
                        mismatched_bound,
                        0.1,
                    )
                    .is_none(),
                    "{name}"
                );
            }

            let mut coefficient_mutations = 0_usize;
            for face_index in 0..2 {
                for coefficient_index in 0..12 {
                    diagnostic.toggle_face_transform_lsb_for_test(face_index, coefficient_index);
                    assert!(
                        revalidate_direct_f_affine_hinge_corridor_diagnostic_v1(
                            diagnostic,
                            prerequisite,
                            ef,
                            exact_e,
                            &exact,
                            bound,
                            0.1,
                        )
                        .is_none(),
                        "face {face_index} coefficient {coefficient_index}"
                    );
                    diagnostic.toggle_face_transform_lsb_for_test(face_index, coefficient_index);
                    coefficient_mutations += 1;
                }
            }
            for coefficient_index in 0..12 {
                diagnostic.toggle_hinge_parent_transform_lsb_for_test(coefficient_index);
                assert!(
                    revalidate_direct_f_affine_hinge_corridor_diagnostic_v1(
                        diagnostic,
                        prerequisite,
                        ef,
                        exact_e,
                        &exact,
                        bound,
                        0.1,
                    )
                    .is_none(),
                    "hinge-parent coefficient {coefficient_index}"
                );
                diagnostic.toggle_hinge_parent_transform_lsb_for_test(coefficient_index);
                coefficient_mutations += 1;
            }
            assert_eq!(coefficient_mutations, 36);
            assert!(
                revalidate_direct_f_affine_hinge_corridor_diagnostic_v1(
                    diagnostic,
                    prerequisite,
                    ef,
                    exact_e,
                    &exact,
                    bound,
                    0.1,
                )
                .is_some(),
                "all test mutations must be restored"
            );
        }

        assert!(analysis.sealed_diagnostic_result_and_work().is_some());
        let original = analysis.work.local_exact.interval_operations;
        analysis.work.local_exact.interval_operations =
            original.checked_add(1).expect("test counter increment");
        assert!(
            analysis.sealed_diagnostic_result_and_work().is_none(),
            "modified public work must not match the sealed diagnostic"
        );
        analysis.work.local_exact.interval_operations = original;
        assert!(analysis.sealed_diagnostic_result_and_work().is_some());
    }

    #[test]
    fn direct_f_affine_c2_large_pivot_drift_remains_an_unadmitted_diagnostic() {
        for (case_index, origin) in [1.0e12, 1.0e15].into_iter().enumerate() {
            let model = two_triangle_model_with_options(
                EdgeKind::Mountain,
                false,
                [
                    (origin, origin),
                    (origin + 400.0, origin),
                    (origin + 400.0, origin + 400.0),
                    (origin, origin + 400.0),
                ],
                611 + case_index as u64,
            );
            for root in model.face_ids() {
                let pose = triangular_pose_with_root(&model, 135.0, *root);
                let bound = model.bind_pose(&pose).unwrap();
                let exact = triangular_exact_pose(&model, &pose);
                let prerequisite_analysis = authenticated_ef_prerequisite(&exact, 0.1);
                let SingleTriangularHingePrerequisiteResult::Authenticated(prerequisite) =
                    &prerequisite_analysis.result
                else {
                    panic!("{origin:e}, root {root:?}: prerequisite");
                };
                let ef_analysis = analyze_axis_aligned_ef_boundary_v1(
                    prerequisite,
                    &exact,
                    bound,
                    0.1,
                    AxisAlignedEfBoundaryLimits::default(),
                )
                .unwrap();
                let ef = ef_capability(&ef_analysis);
                let exact_e_analysis = analyze_exact_e_finite_hinge_corridor_v1(
                    &prerequisite_analysis,
                    Some(ef),
                    &exact,
                    bound,
                    0.1,
                    ExactEFiniteHingeCorridorLimits::default(),
                )
                .unwrap();
                let analysis = analyze_direct_f_affine_hinge_corridor_v1(
                    &prerequisite_analysis,
                    Some(ef),
                    &exact_e_analysis,
                    &exact,
                    bound,
                    0.1,
                    DirectFAffineHingeCorridorLimits::default(),
                )
                .unwrap();
                match &analysis.result {
                    DirectFAffineHingeCorridorResult::ContainedUnadmitted(diagnostic) => {
                        assert_eq!(
                            diagnostic.shared_endpoint_mismatch_count(),
                            2,
                            "large-pivot literal F retains drift at both endpoints; \
                             ContainedUnadmitted remains diagnostic-only"
                        );
                    }
                    other => panic!(
                        "large-pivot geometry must remain contained-but-unadmitted: \
                         {origin:e}, root {root:?}: {other:?}"
                    ),
                }
            }
        }
    }

    fn assert_cayley_work_is_monotone(prior: &CayleyWork, cumulative: &CayleyWork) {
        for (before, after) in [
            (prior.machin_terms, cumulative.machin_terms),
            (prior.trig_terms, cumulative.trig_terms),
            (prior.sqrt_refinements, cumulative.sqrt_refinements),
            (prior.interval_operations, cumulative.interval_operations),
            (prior.gcd_fallback_calls, cumulative.gcd_fallback_calls),
            (
                prior.gcd_fallback_input_bits,
                cumulative.gcd_fallback_input_bits,
            ),
            (prior.rational_allocations, cumulative.rational_allocations),
            (
                prior.total_rational_allocation_bits,
                cumulative.total_rational_allocation_bits,
            ),
        ] {
            assert!(after >= before);
        }
        for (before, after) in [
            (
                prior.max_machin_series_terms,
                cumulative.max_machin_series_terms,
            ),
            (
                prior.max_trig_series_terms,
                cumulative.max_trig_series_terms,
            ),
            (
                prior.max_sqrt_call_refinements,
                cumulative.max_sqrt_call_refinements,
            ),
            (prior.max_shift_bits, cumulative.max_shift_bits),
            (prior.max_preflight_bits, cumulative.max_preflight_bits),
            (prior.max_observed_bits, cumulative.max_observed_bits),
            (
                prior.max_gcd_fallback_call_input_bits,
                cumulative.max_gcd_fallback_call_input_bits,
            ),
            (
                prior.max_rational_allocation_bits,
                cumulative.max_rational_allocation_bits,
            ),
            (prior.max_output_bits, cumulative.max_output_bits),
        ] {
            assert!(after >= before);
        }
    }

    #[test]
    fn direct_f_corridor_propagates_all_24_flat_fold_cases_without_f_work() {
        let square = [(0.0, 0.0), (400.0, 0.0), (400.0, 400.0), (0.0, 400.0)];
        let models = [
            two_triangle_model_with_options(EdgeKind::Mountain, false, square, 505),
            two_triangle_model_with_options(EdgeKind::Mountain, true, square, 506),
            two_triangle_model_with_options(EdgeKind::Valley, false, square, 507),
            two_triangle_model_with_options(EdgeKind::Valley, true, square, 508),
        ];
        let mut cases = 0_usize;
        for model in &models {
            for root in model.face_ids() {
                let pose = triangular_pose_with_root(model, 180.0, *root);
                let bound = model.bind_pose(&pose).unwrap();
                let exact = triangular_exact_pose(model, &pose);
                for thickness in [0.1, 1.0, 3.0] {
                    cases += 1;
                    let prerequisite_analysis = authenticated_ef_prerequisite(&exact, thickness);
                    let exact_e_analysis = analyze_exact_e_finite_hinge_corridor_v1(
                        &prerequisite_analysis,
                        None,
                        &exact,
                        bound,
                        thickness,
                        ExactEFiniteHingeCorridorLimits::default(),
                    )
                    .unwrap();
                    let analysis = analyze_direct_f_finite_hinge_corridor_v1(
                        &prerequisite_analysis,
                        None,
                        &exact_e_analysis,
                        &exact,
                        bound,
                        thickness,
                        DirectFFiniteHingeCorridorLimits::default(),
                    )
                    .unwrap();
                    assert!(matches!(
                        analysis.result,
                        DirectFFiniteHingeCorridorResult::LayerOffsetUnmodeled
                    ));
                    assert_eq!(analysis.work, DirectFFiniteHingeCorridorWork::default());
                }
            }
        }
        assert_eq!(cases, 24);
    }

    #[test]
    fn direct_f_corridor_axis_uses_parent_path_and_does_not_weld_child_endpoint_drift() {
        let model = two_triangle_model_with_options(
            EdgeKind::Mountain,
            false,
            [(0.0, 0.0), (400.0, 0.0), (400.0, 400.0), (0.0, 400.0)],
            509,
        );
        let pose = triangular_pose_with_root(&model, 135.0, model.face_ids()[0]);
        let bound = model.bind_pose(&pose).unwrap();
        let exact = triangular_exact_pose(&model, &pose);
        let prerequisite_analysis = authenticated_ef_prerequisite(&exact, 0.1);
        let SingleTriangularHingePrerequisiteResult::Authenticated(prerequisite) =
            &prerequisite_analysis.result
        else {
            panic!("prerequisite");
        };
        let ef_analysis = analyze_axis_aligned_ef_boundary_v1(
            prerequisite,
            &exact,
            bound,
            0.1,
            AxisAlignedEfBoundaryLimits::default(),
        )
        .unwrap();
        let ef = ef_capability(&ef_analysis);
        let exact_e_analysis = analyze_exact_e_finite_hinge_corridor_v1(
            &prerequisite_analysis,
            Some(ef),
            &exact,
            bound,
            0.1,
            ExactEFiniteHingeCorridorLimits::default(),
        )
        .unwrap();
        let direct_f_analysis = analyze_direct_f_finite_hinge_corridor_v1(
            &prerequisite_analysis,
            Some(ef),
            &exact_e_analysis,
            &exact,
            bound,
            0.1,
            DirectFFiniteHingeCorridorLimits::default(),
        )
        .unwrap();
        let capability = direct_f_corridor_capability(&direct_f_analysis);
        let exact_hinge = &exact.hinges[0];
        let native_hinge = &model.hinges()[0];
        let parent_transform = pose
            .hinge_parent_transform(exact_hinge.edge)
            .expect("native hinge-parent transform");
        assert_eq!(
            parent_transform,
            pose.face_transform(exact_hinge.parent)
                .expect("parent face transform")
        );
        let child_transform = pose
            .face_transform(exact_hinge.child)
            .expect("child face transform");
        let parent_start = direct_affine_point(parent_transform, native_hinge.start());
        let parent_end = direct_affine_point(parent_transform, native_hinge.end());
        let child_start = direct_affine_point(child_transform, native_hinge.start());
        let child_end = direct_affine_point(child_transform, native_hinge.end());
        assert!(
            child_start != parent_start || child_end != parent_end,
            "non-cardinal direct-lift F retains its exact affine endpoint drift"
        );
        assert_eq!(capability.axis_start(), &parent_start);
        let observed_end = ExactPoint3 {
            coordinates: std::array::from_fn(|axis| {
                &capability.axis_start().coordinates[axis] + &capability.axis().coordinates[axis]
            }),
        };
        assert_eq!(observed_end, parent_end);
        assert!(
            capability.axis_start() != &child_start || observed_end != child_end,
            "the corridor axis must not average or weld the child path"
        );
    }

    #[test]
    fn direct_f_corridor_geometry_is_independent_of_every_ef_component_box() {
        let model = two_triangle_model();
        let pose = triangular_pose(&model, 135.0);
        let bound = model.bind_pose(&pose).unwrap();
        let exact = triangular_exact_pose(&model, &pose);
        let prerequisite_analysis = authenticated_ef_prerequisite(&exact, 3.0);
        let SingleTriangularHingePrerequisiteResult::Authenticated(prerequisite) =
            &prerequisite_analysis.result
        else {
            panic!("prerequisite");
        };
        let baseline_ef_analysis = analyze_axis_aligned_ef_boundary_v1(
            prerequisite,
            &exact,
            bound,
            3.0,
            AxisAlignedEfBoundaryLimits::default(),
        )
        .unwrap();
        let mut mutated_ef_analysis = analyze_axis_aligned_ef_boundary_v1(
            prerequisite,
            &exact,
            bound,
            3.0,
            AxisAlignedEfBoundaryLimits::default(),
        )
        .unwrap();
        let mutated_ef = mutated_ef_analysis
            .capability
            .as_mut()
            .expect("mutated E/F capability");
        let sentinel = integer(999_999);
        for face in &mut mutated_ef.faces {
            face.point_component_bound_mm = std::array::from_fn(|_| sentinel.clone());
            face.normal_component_bound = std::array::from_fn(|_| sentinel.clone());
            face.solid_component_bound_mm = std::array::from_fn(|_| sentinel.clone());
            face.point_linf_bound_mm = sentinel.clone();
            face.normal_linf_bound = sentinel.clone();
            face.solid_linf_bound_mm = sentinel.clone();
        }
        mutated_ef.point_component_bound_mm = std::array::from_fn(|_| sentinel.clone());
        mutated_ef.normal_component_bound = std::array::from_fn(|_| sentinel.clone());
        mutated_ef.solid_component_bound_mm = std::array::from_fn(|_| sentinel.clone());
        mutated_ef.point_linf_bound_mm = sentinel.clone();
        mutated_ef.normal_linf_bound = sentinel.clone();
        mutated_ef.solid_linf_bound_mm = sentinel;

        let baseline_ef = ef_capability(&baseline_ef_analysis);
        let mutated_ef = ef_capability(&mutated_ef_analysis);
        let baseline_exact_e = analyze_exact_e_finite_hinge_corridor_v1(
            &prerequisite_analysis,
            Some(baseline_ef),
            &exact,
            bound,
            3.0,
            ExactEFiniteHingeCorridorLimits::default(),
        )
        .unwrap();
        let mutated_exact_e = analyze_exact_e_finite_hinge_corridor_v1(
            &prerequisite_analysis,
            Some(mutated_ef),
            &exact,
            bound,
            3.0,
            ExactEFiniteHingeCorridorLimits::default(),
        )
        .unwrap();
        let baseline = analyze_direct_f_finite_hinge_corridor_v1(
            &prerequisite_analysis,
            Some(baseline_ef),
            &baseline_exact_e,
            &exact,
            bound,
            3.0,
            DirectFFiniteHingeCorridorLimits::default(),
        )
        .unwrap();
        let mutated = analyze_direct_f_finite_hinge_corridor_v1(
            &prerequisite_analysis,
            Some(mutated_ef),
            &mutated_exact_e,
            &exact,
            bound,
            3.0,
            DirectFFiniteHingeCorridorLimits::default(),
        )
        .unwrap();
        let baseline_capability = direct_f_corridor_capability(&baseline);
        let mutated_capability = direct_f_corridor_capability(&mutated);
        assert_eq!(
            baseline_capability.interaction_kind(),
            mutated_capability.interaction_kind()
        );
        assert_eq!(
            baseline_capability.length_squared(),
            mutated_capability.length_squared()
        );
        assert_eq!(
            baseline_capability.half_thickness(),
            mutated_capability.half_thickness()
        );
        assert_eq!(
            baseline_capability.cosine_half_squared(),
            mutated_capability.cosine_half_squared()
        );
        assert_eq!(baseline.work, mutated.work);
    }

    #[test]
    fn direct_f_corridor_rejects_tokens_aba_root_ulp_thickness_and_each_f_coefficient() {
        let model = two_triangle_model();
        let pose = triangular_pose(&model, 135.0);
        let bound = model.bind_pose(&pose).unwrap();
        let exact = triangular_exact_pose(&model, &pose);
        let independent_exact = triangular_exact_pose(&model, &pose);
        let prerequisite_analysis = authenticated_ef_prerequisite(&exact, 0.1);
        let duplicate_prerequisite_analysis = authenticated_ef_prerequisite(&exact, 0.1);
        let SingleTriangularHingePrerequisiteResult::Authenticated(prerequisite) =
            &prerequisite_analysis.result
        else {
            panic!("prerequisite");
        };
        let SingleTriangularHingePrerequisiteResult::Authenticated(duplicate_prerequisite) =
            &duplicate_prerequisite_analysis.result
        else {
            panic!("duplicate prerequisite");
        };
        let ef_analysis = analyze_axis_aligned_ef_boundary_v1(
            prerequisite,
            &exact,
            bound,
            0.1,
            AxisAlignedEfBoundaryLimits::default(),
        )
        .unwrap();
        let duplicate_ef_analysis = analyze_axis_aligned_ef_boundary_v1(
            prerequisite,
            &exact,
            bound,
            0.1,
            AxisAlignedEfBoundaryLimits::default(),
        )
        .unwrap();
        let ef = ef_capability(&ef_analysis);
        let duplicate_ef = ef_capability(&duplicate_ef_analysis);
        let exact_e_analysis = analyze_exact_e_finite_hinge_corridor_v1(
            &prerequisite_analysis,
            Some(ef),
            &exact,
            bound,
            0.1,
            ExactEFiniteHingeCorridorLimits::default(),
        )
        .unwrap();
        let duplicate_exact_e_analysis = analyze_exact_e_finite_hinge_corridor_v1(
            &prerequisite_analysis,
            Some(ef),
            &exact,
            bound,
            0.1,
            ExactEFiniteHingeCorridorLimits::default(),
        )
        .unwrap();
        let exact_e = exact_e_corridor_capability(&exact_e_analysis);
        let duplicate_exact_e = exact_e_corridor_capability(&duplicate_exact_e_analysis);
        let mut direct_analysis = analyze_direct_f_finite_hinge_corridor_v1(
            &prerequisite_analysis,
            Some(ef),
            &exact_e_analysis,
            &exact,
            bound,
            0.1,
            DirectFFiniteHingeCorridorLimits::default(),
        )
        .unwrap();
        let DirectFFiniteHingeCorridorResult::Contained(direct) = &mut direct_analysis.result
        else {
            panic!("direct F corridor");
        };
        assert!(
            revalidate_direct_f_finite_hinge_corridor_v1(
                direct,
                prerequisite,
                ef,
                exact_e,
                &exact,
                bound,
                0.1,
            )
            .is_some()
        );
        assert!(
            revalidate_direct_f_finite_hinge_corridor_v1(
                direct,
                duplicate_prerequisite,
                ef,
                exact_e,
                &exact,
                bound,
                0.1,
            )
            .is_none(),
            "independently issued phase-1 token"
        );
        assert!(
            revalidate_direct_f_finite_hinge_corridor_v1(
                direct,
                prerequisite,
                duplicate_ef,
                exact_e,
                &exact,
                bound,
                0.1,
            )
            .is_none(),
            "independently issued EF token"
        );
        assert!(
            revalidate_direct_f_finite_hinge_corridor_v1(
                direct,
                prerequisite,
                ef,
                duplicate_exact_e,
                &exact,
                bound,
                0.1,
            )
            .is_none(),
            "independently issued exact-E corridor token"
        );
        assert!(
            revalidate_direct_f_finite_hinge_corridor_v1(
                direct,
                prerequisite,
                ef,
                exact_e,
                &independent_exact,
                bound,
                0.1,
            )
            .is_none(),
            "independently regenerated exact pose"
        );

        let aba_pose = triangular_pose(&model, 135.0);
        let aba_bound = model.bind_pose(&aba_pose).unwrap();
        let rerooted_pose = triangular_pose_with_root(&model, 135.0, model.face_ids()[1]);
        let rerooted_bound = model.bind_pose(&rerooted_pose).unwrap();
        let one_ulp_pose = triangular_pose(&model, next_up(135.0));
        let one_ulp_bound = model.bind_pose(&one_ulp_pose).unwrap();
        for (name, mismatched_bound) in [
            ("same-angle ABA", aba_bound),
            ("different fixed root", rerooted_bound),
            ("one-ULP angle", one_ulp_bound),
        ] {
            assert!(
                revalidate_direct_f_finite_hinge_corridor_v1(
                    direct,
                    prerequisite,
                    ef,
                    exact_e,
                    &exact,
                    mismatched_bound,
                    0.1,
                )
                .is_none(),
                "{name}"
            );
        }
        assert!(
            revalidate_direct_f_finite_hinge_corridor_v1(
                direct,
                prerequisite,
                ef,
                exact_e,
                &exact,
                bound,
                next_up(0.1),
            )
            .is_none(),
            "one-ULP paper thickness"
        );

        let foreign = two_triangle_model_with_options(
            EdgeKind::Mountain,
            false,
            [(0.0, 0.0), (400.0, 0.0), (400.0, 400.0), (0.0, 400.0)],
            510,
        );
        let foreign_pose = triangular_pose(&foreign, 135.0);
        let foreign_bound = foreign.bind_pose(&foreign_pose).unwrap();
        assert!(
            revalidate_direct_f_finite_hinge_corridor_v1(
                direct,
                prerequisite,
                ef,
                exact_e,
                &exact,
                foreign_bound,
                0.1,
            )
            .is_none(),
            "foreign model and pose"
        );

        let mut changed_coefficients = 0_usize;
        for face in 0..2 {
            for row in 0..3 {
                for column in 0..3 {
                    let original = direct.binary64_face_transforms[face].rotation[row][column];
                    direct.binary64_face_transforms[face].rotation[row][column] = original ^ 1;
                    assert!(
                        revalidate_direct_f_finite_hinge_corridor_v1(
                            direct,
                            prerequisite,
                            ef,
                            exact_e,
                            &exact,
                            bound,
                            0.1,
                        )
                        .is_none(),
                        "face {face} rotation[{row}][{column}]"
                    );
                    direct.binary64_face_transforms[face].rotation[row][column] = original;
                    changed_coefficients += 1;
                }
            }
            for axis in 0..3 {
                let original = direct.binary64_face_transforms[face].translation[axis];
                direct.binary64_face_transforms[face].translation[axis] = original ^ 1;
                assert!(
                    revalidate_direct_f_finite_hinge_corridor_v1(
                        direct,
                        prerequisite,
                        ef,
                        exact_e,
                        &exact,
                        bound,
                        0.1,
                    )
                    .is_none(),
                    "face {face} translation[{axis}]"
                );
                direct.binary64_face_transforms[face].translation[axis] = original;
                changed_coefficients += 1;
            }
        }
        for row in 0..3 {
            for column in 0..3 {
                let original = direct.hinge_parent_transform.rotation[row][column];
                direct.hinge_parent_transform.rotation[row][column] = original ^ 1;
                assert!(
                    revalidate_direct_f_finite_hinge_corridor_v1(
                        direct,
                        prerequisite,
                        ef,
                        exact_e,
                        &exact,
                        bound,
                        0.1,
                    )
                    .is_none(),
                    "hinge-parent rotation[{row}][{column}]"
                );
                direct.hinge_parent_transform.rotation[row][column] = original;
                changed_coefficients += 1;
            }
        }
        for axis in 0..3 {
            let original = direct.hinge_parent_transform.translation[axis];
            direct.hinge_parent_transform.translation[axis] = original ^ 1;
            assert!(
                revalidate_direct_f_finite_hinge_corridor_v1(
                    direct,
                    prerequisite,
                    ef,
                    exact_e,
                    &exact,
                    bound,
                    0.1,
                )
                .is_none(),
                "hinge-parent translation[{axis}]"
            );
            direct.hinge_parent_transform.translation[axis] = original;
            changed_coefficients += 1;
        }
        assert_eq!(changed_coefficients, 36);
    }

    #[test]
    fn direct_f_rejects_every_modified_phase2b_work_counter_and_seals_its_own_work() {
        let model = two_triangle_model();
        let pose = triangular_pose(&model, 135.0);
        let bound = model.bind_pose(&pose).unwrap();
        let exact = triangular_exact_pose(&model, &pose);
        let prerequisite_analysis = authenticated_ef_prerequisite(&exact, 0.1);
        let SingleTriangularHingePrerequisiteResult::Authenticated(prerequisite) =
            &prerequisite_analysis.result
        else {
            panic!("prerequisite");
        };
        let ef_analysis = analyze_axis_aligned_ef_boundary_v1(
            prerequisite,
            &exact,
            bound,
            0.1,
            AxisAlignedEfBoundaryLimits::default(),
        )
        .unwrap();
        let ef = ef_capability(&ef_analysis);
        let mut exact_e_analysis = analyze_exact_e_finite_hinge_corridor_v1(
            &prerequisite_analysis,
            Some(ef),
            &exact,
            bound,
            0.1,
            ExactEFiniteHingeCorridorLimits::default(),
        )
        .unwrap();
        assert!(
            exact_e_analysis
                .authenticated_contained_capability_and_work()
                .is_some()
        );

        let mut field_count = 0_usize;
        let mut mutation_count = 0_usize;
        macro_rules! reject_work_mutation {
            ($field:expr, $name:expr) => {{
                field_count += 1;
                let original = $field;
                $field = original.checked_add(1).expect("test counter increment");
                let rejected = analyze_direct_f_finite_hinge_corridor_v1(
                    &prerequisite_analysis,
                    Some(ef),
                    &exact_e_analysis,
                    &exact,
                    bound,
                    0.1,
                    DirectFFiniteHingeCorridorLimits::default(),
                )
                .unwrap();
                let rejected_result = matches!(
                    &rejected.result,
                    DirectFFiniteHingeCorridorResult::Unresolved
                );
                let rejected_work_is_empty =
                    rejected.work == DirectFFiniteHingeCorridorWork::default();
                drop(rejected);
                $field = original;
                assert!(rejected_result, "{} upper", $name);
                assert!(
                    rejected_work_is_empty,
                    "{} upper must not start F work",
                    $name
                );
                mutation_count += 1;

                if original > 0 {
                    $field = original - 1;
                    let rejected = analyze_direct_f_finite_hinge_corridor_v1(
                        &prerequisite_analysis,
                        Some(ef),
                        &exact_e_analysis,
                        &exact,
                        bound,
                        0.1,
                        DirectFFiniteHingeCorridorLimits::default(),
                    )
                    .unwrap();
                    let rejected_result = matches!(
                        &rejected.result,
                        DirectFFiniteHingeCorridorResult::Unresolved
                    );
                    let rejected_work_is_empty =
                        rejected.work == DirectFFiniteHingeCorridorWork::default();
                    drop(rejected);
                    $field = original;
                    assert!(rejected_result, "{} lower", $name);
                    assert!(
                        rejected_work_is_empty,
                        "{} lower must not start F work",
                        $name
                    );
                    mutation_count += 1;
                }
            }};
        }
        macro_rules! reject_work_fields {
            ($base:expr, $prefix:literal, [$($field:ident),+ $(,)?]) => {
                $(
                    reject_work_mutation!(
                        ($base).$field,
                        concat!($prefix, stringify!($field))
                    );
                )+
            };
        }

        reject_work_fields!(
            exact_e_analysis.work,
            "phase2b.",
            [
                authenticated_faces,
                authenticated_hinges,
                local_y_component_lifts,
                thickness_lifts,
                half_thickness_divisions,
                scalar_reconstructions,
                corridor_vertex_tests,
            ]
        );
        reject_work_fields!(
            exact_e_analysis.work.prism,
            "phase2b.prism.",
            [
                prisms,
                solid_vertices,
                facets,
                halfspaces,
                prism_volume_tests,
                facet_vertex_checks,
                plane_triples,
                singular_plane_triples,
                nonsingular_solves,
                membership_tests,
                candidate_vertices,
                dedup_comparisons,
                affine_rank_tests,
                support_plane_vertex_tests,
                support_pair_tests,
                input_rationals,
                max_input_rational_storage_bits,
                total_input_storage_bits,
            ]
        );
        reject_work_fields!(
            exact_e_analysis.work.prism.exact,
            "phase2b.prism.exact.",
            [
                interval_operations,
                machin_terms,
                max_machin_series_terms,
                trig_terms,
                max_trig_series_terms,
                sqrt_refinements,
                max_sqrt_call_refinements,
                max_shift_bits,
                max_preflight_bits,
                max_observed_bits,
                gcd_fallback_calls,
                gcd_fallback_input_bits,
                max_gcd_fallback_call_input_bits,
                rational_allocations,
                max_rational_allocation_bits,
                total_rational_allocation_bits,
                max_output_bits,
            ]
        );
        reject_work_fields!(
            exact_e_analysis.work.exact,
            "phase2b.exact.",
            [
                interval_operations,
                machin_terms,
                max_machin_series_terms,
                trig_terms,
                max_trig_series_terms,
                sqrt_refinements,
                max_sqrt_call_refinements,
                max_shift_bits,
                max_preflight_bits,
                max_observed_bits,
                gcd_fallback_calls,
                gcd_fallback_input_bits,
                max_gcd_fallback_call_input_bits,
                rational_allocations,
                max_rational_allocation_bits,
                total_rational_allocation_bits,
                max_output_bits,
            ]
        );
        assert_eq!(field_count, 59);
        assert!(mutation_count >= field_count);
        assert!(
            exact_e_analysis
                .authenticated_contained_capability_and_work()
                .is_some(),
            "all test mutations must be restored"
        );

        let mut direct_analysis = analyze_direct_f_finite_hinge_corridor_v1(
            &prerequisite_analysis,
            Some(ef),
            &exact_e_analysis,
            &exact,
            bound,
            0.1,
            DirectFFiniteHingeCorridorLimits::default(),
        )
        .unwrap();
        assert!(
            direct_analysis
                .authenticated_contained_capability_and_work()
                .is_some()
        );
        let original = direct_analysis.work.local_exact.interval_operations;
        direct_analysis.work.local_exact.interval_operations =
            original.checked_add(1).expect("test counter increment");
        assert!(
            direct_analysis
                .authenticated_contained_capability_and_work()
                .is_none(),
            "future C2 must not consume a direct-F analysis whose work was modified"
        );
        direct_analysis.work.local_exact.interval_operations = original;
        assert!(
            direct_analysis
                .authenticated_contained_capability_and_work()
                .is_some()
        );
    }

    #[test]
    fn direct_f_thickness_boundaries_are_exact_bit_bound_and_fail_closed() {
        let model = two_triangle_model();
        let pose = triangular_pose(&model, 135.0);
        let bound = model.bind_pose(&pose).unwrap();
        let exact = triangular_exact_pose(&model, &pose);

        for thickness in [0.01, f64::from_bits(1)] {
            let prerequisite_analysis = authenticated_ef_prerequisite(&exact, thickness);
            let SingleTriangularHingePrerequisiteResult::Authenticated(prerequisite) =
                &prerequisite_analysis.result
            else {
                panic!("{thickness:e} mm prerequisite");
            };
            let ef_analysis = analyze_axis_aligned_ef_boundary_v1(
                prerequisite,
                &exact,
                bound,
                thickness,
                AxisAlignedEfBoundaryLimits::default(),
            )
            .unwrap();
            let ef = ef_capability(&ef_analysis);
            let exact_e_analysis = analyze_exact_e_finite_hinge_corridor_v1(
                &prerequisite_analysis,
                Some(ef),
                &exact,
                bound,
                thickness,
                ExactEFiniteHingeCorridorLimits::default(),
            )
            .unwrap();
            let exact_e = exact_e_corridor_capability(&exact_e_analysis);
            let direct_analysis = analyze_direct_f_finite_hinge_corridor_v1(
                &prerequisite_analysis,
                Some(ef),
                &exact_e_analysis,
                &exact,
                bound,
                thickness,
                DirectFFiniteHingeCorridorLimits::default(),
            )
            .unwrap();
            let direct = direct_f_corridor_capability(&direct_analysis);
            let expected_half =
                BigRational::from_float(thickness).expect("finite thickness") / integer(2);
            assert!(expected_half.is_positive());
            assert_eq!(
                direct.half_thickness(),
                &expected_half,
                "t/2 must remain exact for {thickness:e} mm"
            );
            assert!(
                revalidate_direct_f_finite_hinge_corridor_v1(
                    direct,
                    prerequisite,
                    ef,
                    exact_e,
                    &exact,
                    bound,
                    thickness,
                )
                .is_some()
            );
            if thickness.to_bits() == 0.01_f64.to_bits() {
                assert!(
                    revalidate_direct_f_finite_hinge_corridor_v1(
                        direct,
                        prerequisite,
                        ef,
                        exact_e,
                        &exact,
                        bound,
                        next_up(thickness),
                    )
                    .is_none(),
                    "0.01 mm is bound by its full binary64 bit pattern"
                );
            }
        }

        let thickness = 0.1;
        let prerequisite_analysis = authenticated_ef_prerequisite(&exact, thickness);
        let SingleTriangularHingePrerequisiteResult::Authenticated(prerequisite) =
            &prerequisite_analysis.result
        else {
            panic!("valid prerequisite");
        };
        let ef_analysis = analyze_axis_aligned_ef_boundary_v1(
            prerequisite,
            &exact,
            bound,
            thickness,
            AxisAlignedEfBoundaryLimits::default(),
        )
        .unwrap();
        let ef = ef_capability(&ef_analysis);
        let exact_e_analysis = analyze_exact_e_finite_hinge_corridor_v1(
            &prerequisite_analysis,
            Some(ef),
            &exact,
            bound,
            thickness,
            ExactEFiniteHingeCorridorLimits::default(),
        )
        .unwrap();
        for invalid in [0.0, -0.0, -0.01, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let rejected = analyze_direct_f_finite_hinge_corridor_v1(
                &prerequisite_analysis,
                Some(ef),
                &exact_e_analysis,
                &exact,
                bound,
                invalid,
                DirectFFiniteHingeCorridorLimits::default(),
            )
            .unwrap();
            assert!(
                matches!(
                    rejected.result,
                    DirectFFiniteHingeCorridorResult::Unresolved
                ),
                "{invalid:?}"
            );
            assert_eq!(
                rejected.work,
                DirectFFiniteHingeCorridorWork::default(),
                "{invalid:?} must not begin F work"
            );
        }
    }

    fn direct_f_corridor_limits_from_work(
        work: &DirectFFiniteHingeCorridorWork,
    ) -> DirectFFiniteHingeCorridorLimits {
        DirectFFiniteHingeCorridorLimits {
            max_authenticated_faces: work.authenticated_faces,
            max_authenticated_hinges: work.authenticated_hinges,
            max_face_transform_bit_bindings: work.face_transform_bit_bindings,
            max_hinge_parent_transform_bit_bindings: work.hinge_parent_transform_bit_bindings,
            max_transform_scalar_lifts: work.transform_scalar_lifts,
            max_source_coordinate_lifts: work.source_coordinate_lifts,
            max_affine_point_reconstructions: work.affine_point_reconstructions,
            max_material_normal_component_lifts: work.material_normal_component_lifts,
            max_solid_vertex_constructions: work.solid_vertex_constructions,
            max_thickness_lifts: work.thickness_lifts,
            max_half_thickness_divisions: work.half_thickness_divisions,
            max_scalar_reconstructions: work.scalar_reconstructions,
            max_corridor_vertex_tests: work.corridor_vertex_tests,
            prism: ExactPrismLimits {
                max_prisms: work.prism.prisms,
                max_solid_vertices: work.prism.solid_vertices,
                max_facets: work.prism.facets,
                max_halfspaces: work.prism.halfspaces,
                max_prism_volume_tests: work.prism.prism_volume_tests,
                max_facet_vertex_checks: work.prism.facet_vertex_checks,
                max_plane_triples: work.prism.plane_triples,
                max_singular_plane_triples: work.prism.singular_plane_triples,
                max_nonsingular_solves: work.prism.nonsingular_solves,
                max_membership_tests: work.prism.membership_tests,
                max_candidate_vertices: work.prism.candidate_vertices,
                max_dedup_comparisons: work.prism.dedup_comparisons,
                max_affine_rank_tests: work.prism.affine_rank_tests,
                max_support_plane_vertex_tests: work.prism.support_plane_vertex_tests,
                max_support_pair_tests: work.prism.support_pair_tests,
                max_input_rationals: work.prism.input_rationals,
                max_input_rational_storage_bits: work.prism.max_input_rational_storage_bits,
                max_total_input_storage_bits: work.prism.total_input_storage_bits,
                exact: cayley_limits_from_observed_work(&work.prism.exact),
            },
            local_exact: cayley_limits_from_observed_work(&work.local_exact),
            exact: cayley_limits_from_observed_work(&work.exact),
        }
    }

    #[test]
    fn direct_f_corridor_all_structural_and_exact_counters_have_one_short_limits() {
        let model = two_triangle_model();
        let pose = triangular_pose(&model, 135.0);
        let bound = model.bind_pose(&pose).unwrap();
        let exact = triangular_exact_pose(&model, &pose);
        let prerequisite_analysis = authenticated_ef_prerequisite(&exact, 0.1);
        let SingleTriangularHingePrerequisiteResult::Authenticated(prerequisite) =
            &prerequisite_analysis.result
        else {
            panic!("prerequisite");
        };
        let ef_analysis = analyze_axis_aligned_ef_boundary_v1(
            prerequisite,
            &exact,
            bound,
            0.1,
            AxisAlignedEfBoundaryLimits::default(),
        )
        .unwrap();
        let ef = ef_capability(&ef_analysis);
        let exact_e_analysis = analyze_exact_e_finite_hinge_corridor_v1(
            &prerequisite_analysis,
            Some(ef),
            &exact,
            bound,
            0.1,
            ExactEFiniteHingeCorridorLimits::default(),
        )
        .unwrap();
        let baseline = analyze_direct_f_finite_hinge_corridor_v1(
            &prerequisite_analysis,
            Some(ef),
            &exact_e_analysis,
            &exact,
            bound,
            0.1,
            DirectFFiniteHingeCorridorLimits::default(),
        )
        .unwrap();
        let exact_limits = direct_f_corridor_limits_from_work(&baseline.work);
        let exact_analysis = analyze_direct_f_finite_hinge_corridor_v1(
            &prerequisite_analysis,
            Some(ef),
            &exact_e_analysis,
            &exact,
            bound,
            0.1,
            exact_limits,
        )
        .unwrap();
        assert!(matches!(
            exact_analysis.result,
            DirectFFiniteHingeCorridorResult::Contained(_)
        ));
        assert_eq!(exact_analysis.work, baseline.work);

        let assert_one_short = |resource: &str, limits: DirectFFiniteHingeCorridorLimits| {
            assert!(
                matches!(
                    analyze_direct_f_finite_hinge_corridor_v1(
                        &prerequisite_analysis,
                        Some(ef),
                        &exact_e_analysis,
                        &exact,
                        bound,
                        0.1,
                        limits,
                    ),
                    Err(DirectFFiniteHingeCorridorError::ResourceLimitExceeded)
                ),
                "{resource}"
            );
        };
        macro_rules! structural_one_short {
            ($field:ident) => {
                if exact_limits.$field > 0 {
                    let mut limits = exact_limits;
                    limits.$field -= 1;
                    assert_one_short(stringify!($field), limits);
                }
            };
        }
        structural_one_short!(max_authenticated_faces);
        structural_one_short!(max_authenticated_hinges);
        structural_one_short!(max_face_transform_bit_bindings);
        structural_one_short!(max_hinge_parent_transform_bit_bindings);
        structural_one_short!(max_transform_scalar_lifts);
        structural_one_short!(max_source_coordinate_lifts);
        structural_one_short!(max_affine_point_reconstructions);
        structural_one_short!(max_material_normal_component_lifts);
        structural_one_short!(max_solid_vertex_constructions);
        structural_one_short!(max_thickness_lifts);
        structural_one_short!(max_half_thickness_divisions);
        structural_one_short!(max_scalar_reconstructions);
        structural_one_short!(max_corridor_vertex_tests);

        macro_rules! prism_one_short {
            ($field:ident) => {
                if exact_limits.prism.$field > 0 {
                    let mut limits = exact_limits;
                    limits.prism.$field -= 1;
                    assert_one_short(concat!("prism.", stringify!($field)), limits);
                }
            };
        }
        prism_one_short!(max_prisms);
        prism_one_short!(max_solid_vertices);
        prism_one_short!(max_facets);
        prism_one_short!(max_halfspaces);
        prism_one_short!(max_prism_volume_tests);
        prism_one_short!(max_facet_vertex_checks);
        prism_one_short!(max_plane_triples);
        prism_one_short!(max_singular_plane_triples);
        prism_one_short!(max_nonsingular_solves);
        prism_one_short!(max_membership_tests);
        prism_one_short!(max_candidate_vertices);
        prism_one_short!(max_dedup_comparisons);
        prism_one_short!(max_affine_rank_tests);
        prism_one_short!(max_support_plane_vertex_tests);
        prism_one_short!(max_support_pair_tests);
        prism_one_short!(max_input_rationals);
        prism_one_short!(max_input_rational_storage_bits);
        prism_one_short!(max_total_input_storage_bits);

        macro_rules! prism_exact_one_short {
            ($field:ident) => {
                if exact_limits.prism.exact.$field > 0 {
                    let mut limits = exact_limits;
                    limits.prism.exact.$field -= 1;
                    assert_one_short(concat!("prism.exact.", stringify!($field)), limits);
                }
            };
        }
        prism_exact_one_short!(max_interval_operations);
        prism_exact_one_short!(max_shift_bits);
        prism_exact_one_short!(max_intermediate_bits);
        prism_exact_one_short!(max_gcd_fallback_calls);
        prism_exact_one_short!(max_gcd_fallback_input_bits);
        prism_exact_one_short!(max_rational_allocations);
        prism_exact_one_short!(max_rational_allocation_bits);
        prism_exact_one_short!(max_total_rational_allocation_bits);

        macro_rules! local_exact_one_short {
            ($field:ident) => {
                if exact_limits.local_exact.$field > 0 {
                    let mut limits = DirectFFiniteHingeCorridorLimits::default();
                    limits.local_exact = exact_limits.local_exact;
                    limits.local_exact.$field -= 1;
                    assert_one_short(concat!("local_exact.", stringify!($field)), limits);
                }
            };
        }
        local_exact_one_short!(max_interval_operations);
        local_exact_one_short!(max_shift_bits);
        local_exact_one_short!(max_intermediate_bits);
        local_exact_one_short!(max_gcd_fallback_calls);
        local_exact_one_short!(max_gcd_fallback_input_bits);
        local_exact_one_short!(max_rational_allocations);
        local_exact_one_short!(max_rational_allocation_bits);
        local_exact_one_short!(max_total_rational_allocation_bits);

        macro_rules! cumulative_exact_one_short {
            ($field:ident) => {
                if exact_limits.exact.$field > 0 {
                    let mut limits = exact_limits;
                    limits.exact.$field -= 1;
                    assert_one_short(concat!("exact.", stringify!($field)), limits);
                }
            };
        }
        cumulative_exact_one_short!(max_interval_operations);
        cumulative_exact_one_short!(max_shift_bits);
        cumulative_exact_one_short!(max_intermediate_bits);
        cumulative_exact_one_short!(max_gcd_fallback_calls);
        cumulative_exact_one_short!(max_gcd_fallback_input_bits);
        cumulative_exact_one_short!(max_rational_allocations);
        cumulative_exact_one_short!(max_rational_allocation_bits);
        cumulative_exact_one_short!(max_total_rational_allocation_bits);

        let oversized_exact = oversized_cayley_limits();
        let oversized = DirectFFiniteHingeCorridorLimits {
            max_authenticated_faces: usize::MAX,
            max_authenticated_hinges: usize::MAX,
            max_face_transform_bit_bindings: usize::MAX,
            max_hinge_parent_transform_bit_bindings: usize::MAX,
            max_transform_scalar_lifts: usize::MAX,
            max_source_coordinate_lifts: usize::MAX,
            max_affine_point_reconstructions: usize::MAX,
            max_material_normal_component_lifts: usize::MAX,
            max_solid_vertex_constructions: usize::MAX,
            max_thickness_lifts: usize::MAX,
            max_half_thickness_divisions: usize::MAX,
            max_scalar_reconstructions: usize::MAX,
            max_corridor_vertex_tests: usize::MAX,
            prism: ExactPrismLimits {
                max_prisms: usize::MAX,
                max_solid_vertices: usize::MAX,
                max_facets: usize::MAX,
                max_halfspaces: usize::MAX,
                max_prism_volume_tests: usize::MAX,
                max_facet_vertex_checks: usize::MAX,
                max_plane_triples: usize::MAX,
                max_singular_plane_triples: usize::MAX,
                max_nonsingular_solves: usize::MAX,
                max_membership_tests: usize::MAX,
                max_candidate_vertices: usize::MAX,
                max_dedup_comparisons: usize::MAX,
                max_affine_rank_tests: usize::MAX,
                max_support_plane_vertex_tests: usize::MAX,
                max_support_pair_tests: usize::MAX,
                max_input_rationals: usize::MAX,
                max_input_rational_storage_bits: usize::MAX,
                max_total_input_storage_bits: usize::MAX,
                exact: oversized_exact,
            },
            local_exact: oversized_exact,
            exact: oversized_exact,
        };
        let projected = analyze_direct_f_finite_hinge_corridor_v1(
            &prerequisite_analysis,
            Some(ef),
            &exact_e_analysis,
            &exact,
            bound,
            0.1,
            oversized,
        )
        .unwrap();
        assert!(matches!(
            projected.result,
            DirectFFiniteHingeCorridorResult::Contained(_)
        ));
        assert_eq!(projected.work, baseline.work);
    }

    #[test]
    fn ef_boundary_thickness_angle_matrix_is_exact_axis_aligned_and_private() {
        let model = two_triangle_model();
        for thickness in [0.1, 1.0, 3.0] {
            for angle in [10.0, 135.0, 179.0] {
                let pose = triangular_pose(&model, angle);
                let bound = model.bind_pose(&pose).expect("issuer-bound pose");
                let exact = triangular_exact_pose(&model, &pose);
                let prerequisite_analysis = authenticated_ef_prerequisite(&exact, thickness);
                let SingleTriangularHingePrerequisiteResult::Authenticated(prerequisite) =
                    &prerequisite_analysis.result
                else {
                    panic!("{thickness} mm at {angle} degrees: prerequisite");
                };
                let analysis = analyze_axis_aligned_ef_boundary_v1(
                    prerequisite,
                    &exact,
                    bound,
                    thickness,
                    AxisAlignedEfBoundaryLimits::default(),
                )
                .unwrap_or_else(|error| panic!("{thickness} mm at {angle} degrees: {error:?}"));
                let capability = ef_capability(&analysis);
                assert!(std::ptr::eq(capability.exact, &exact));
                assert!(std::ptr::eq(capability.prerequisite, prerequisite));
                assert_eq!(capability.paper_thickness_bits, thickness.to_bits());
                assert_eq!(capability.faces.len(), 2);
                assert_eq!(capability.binary64_face_transforms.len(), 2);
                assert_eq!(capability.hinge_index, 0);
                let exact_half_thickness = BigRational::from_float(thickness).unwrap() / integer(2);
                for face in &capability.faces {
                    for axis in 0..3 {
                        assert!(!face.point_component_bound_mm[axis].is_negative());
                        assert!(!face.normal_component_bound[axis].is_negative());
                        assert!(!face.solid_component_bound_mm[axis].is_negative());
                        assert!(
                            face.solid_component_bound_mm[axis]
                                >= face.point_component_bound_mm[axis]
                        );
                        assert_eq!(
                            face.solid_component_bound_mm[axis],
                            &face.point_component_bound_mm[axis]
                                + &exact_half_thickness * &face.normal_component_bound[axis],
                            "per-face axis {axis}: point + h*normal"
                        );
                    }
                    assert_eq!(
                        face.point_linf_bound_mm,
                        *face.point_component_bound_mm.iter().max().unwrap()
                    );
                    assert_eq!(
                        face.normal_linf_bound,
                        *face.normal_component_bound.iter().max().unwrap()
                    );
                    assert_eq!(
                        face.solid_linf_bound_mm,
                        *face.solid_component_bound_mm.iter().max().unwrap()
                    );
                }
                assert_eq!(
                    capability.point_linf_bound_mm,
                    *capability.point_component_bound_mm.iter().max().unwrap()
                );
                assert_eq!(
                    capability.normal_linf_bound,
                    *capability.normal_component_bound.iter().max().unwrap()
                );
                assert_eq!(
                    capability.solid_linf_bound_mm,
                    *capability.solid_component_bound_mm.iter().max().unwrap()
                );
                for axis in 0..3 {
                    assert_eq!(
                        capability.solid_component_bound_mm[axis],
                        &capability.point_component_bound_mm[axis]
                            + &exact_half_thickness * &capability.normal_component_bound[axis],
                        "global axis {axis}: point + h*normal"
                    );
                }
                let rebound = revalidate_axis_aligned_ef_boundary_v1(
                    capability,
                    prerequisite,
                    &exact,
                    bound,
                    thickness,
                )
                .expect("same exact E, F instance, prerequisite, indexes, and thickness");
                assert!(std::ptr::eq(rebound.capability, capability));
            }
        }
    }

    #[test]
    fn ef_boundary_normal_term_proves_point_only_is_insufficient() {
        let model = two_triangle_model();
        let pose = triangular_pose(&model, 135.0);
        let bound = model.bind_pose(&pose).unwrap();
        let exact = triangular_exact_pose(&model, &pose);
        let prerequisite_analysis = authenticated_ef_prerequisite(&exact, 3.0);
        let SingleTriangularHingePrerequisiteResult::Authenticated(prerequisite) =
            &prerequisite_analysis.result
        else {
            panic!("prerequisite");
        };
        let analysis = analyze_axis_aligned_ef_boundary_v1(
            prerequisite,
            &exact,
            bound,
            3.0,
            AxisAlignedEfBoundaryLimits::default(),
        )
        .unwrap();
        let capability = ef_capability(&analysis);
        assert!(
            capability.normal_linf_bound.is_positive(),
            "non-cardinal binary64 F and rational Cayley E must retain normal error"
        );
        assert!(
            capability.faces.iter().any(|face| (0..3).any(|axis| {
                face.solid_component_bound_mm[axis] > face.point_component_bound_mm[axis]
            })),
            "positive thickness must add h*normal_error to at least one axis"
        );
        assert!(
            capability.solid_linf_bound_mm > capability.point_linf_bound_mm,
            "the scalar L∞ solid bound must not collapse to the point-only bound"
        );
    }

    #[test]
    fn ef_boundary_large_translation_and_minimum_subnormal_thickness_remain_exact() {
        let base = 1_000_000_000_000_000.0;
        let translated = two_triangle_model_with_options(
            EdgeKind::Mountain,
            false,
            [
                (base, base),
                (base + 400.0, base),
                (base + 400.0, base + 400.0),
                (base, base + 400.0),
            ],
            301,
        );
        let translated_pose = triangular_pose(&translated, 179.0);
        let translated_bound = translated.bind_pose(&translated_pose).unwrap();
        let translated_exact = triangular_exact_pose(&translated, &translated_pose);
        let translated_prerequisite = authenticated_ef_prerequisite(&translated_exact, 3.0);
        let SingleTriangularHingePrerequisiteResult::Authenticated(prerequisite) =
            &translated_prerequisite.result
        else {
            panic!("large-translation prerequisite");
        };
        let translated_analysis = analyze_axis_aligned_ef_boundary_v1(
            prerequisite,
            &translated_exact,
            translated_bound,
            3.0,
            AxisAlignedEfBoundaryLimits::default(),
        )
        .unwrap();
        assert!(translated_analysis.capability.is_some());

        let model = two_triangle_model();
        let pose = triangular_pose(&model, 135.0);
        let bound = model.bind_pose(&pose).unwrap();
        let exact = triangular_exact_pose(&model, &pose);
        let minimum_subnormal = f64::from_bits(1);
        let prerequisite_analysis = authenticated_ef_prerequisite(&exact, minimum_subnormal);
        let SingleTriangularHingePrerequisiteResult::Authenticated(prerequisite) =
            &prerequisite_analysis.result
        else {
            panic!("minimum-subnormal prerequisite");
        };
        let analysis = analyze_axis_aligned_ef_boundary_v1(
            prerequisite,
            &exact,
            bound,
            minimum_subnormal,
            AxisAlignedEfBoundaryLimits::default(),
        )
        .unwrap();
        let capability = ef_capability(&analysis);
        assert_eq!(capability.paper_thickness_bits, minimum_subnormal.to_bits());
        assert!(analysis.work.exact.max_shift_bits >= 1_074);
        assert!(
            capability.faces.iter().any(|face| (0..3).any(|axis| {
                face.solid_component_bound_mm[axis] > face.point_component_bound_mm[axis]
            })),
            "exact h=t/2 must not underflow the minimum subnormal to zero"
        );
    }

    #[test]
    fn ef_boundary_rejects_separate_exact_aba_foreign_reroot_thickness_faces_and_f_bits() {
        let model = two_triangle_model();
        let pose = triangular_pose(&model, 135.0);
        let bound = model.bind_pose(&pose).unwrap();
        let exact = triangular_exact_pose(&model, &pose);
        let independent_exact = triangular_exact_pose(&model, &pose);
        let prerequisite_analysis = authenticated_ef_prerequisite(&exact, 0.1);
        let duplicate_prerequisite_analysis = authenticated_ef_prerequisite(&exact, 0.1);
        let SingleTriangularHingePrerequisiteResult::Authenticated(prerequisite) =
            &prerequisite_analysis.result
        else {
            panic!("prerequisite");
        };
        let SingleTriangularHingePrerequisiteResult::Authenticated(duplicate_prerequisite) =
            &duplicate_prerequisite_analysis.result
        else {
            panic!("duplicate prerequisite");
        };
        let mismatched_issuer = analyze_axis_aligned_ef_boundary_v1(
            prerequisite,
            &independent_exact,
            bound,
            0.1,
            AxisAlignedEfBoundaryLimits::default(),
        )
        .unwrap();
        assert!(
            mismatched_issuer.capability.is_none(),
            "issuance itself rejects a prerequisite from another exact object"
        );
        let mut analysis = analyze_axis_aligned_ef_boundary_v1(
            prerequisite,
            &exact,
            bound,
            0.1,
            AxisAlignedEfBoundaryLimits::default(),
        )
        .unwrap();
        let capability = analysis.capability.as_mut().expect("E/F capability");

        let cloned_pose = pose.clone();
        let cloned_bound = model.bind_pose(&cloned_pose).unwrap();
        assert!(
            revalidate_axis_aligned_ef_boundary_v1(
                capability,
                prerequisite,
                &exact,
                cloned_bound,
                0.1,
            )
            .is_some(),
            "a clone preserving the same private pose instance is valid"
        );
        assert!(
            revalidate_axis_aligned_ef_boundary_v1(
                capability,
                prerequisite,
                &independent_exact,
                bound,
                0.1,
            )
            .is_none(),
            "independently regenerated E must fail pointer binding"
        );
        assert!(
            revalidate_axis_aligned_ef_boundary_v1(
                capability,
                duplicate_prerequisite,
                &exact,
                bound,
                0.1,
            )
            .is_none(),
            "an independently issued prerequisite token is not interchangeable"
        );

        let aba_pose = triangular_pose(&model, 135.0);
        let aba_bound = model.bind_pose(&aba_pose).unwrap();
        assert!(
            revalidate_axis_aligned_ef_boundary_v1(
                capability,
                prerequisite,
                &exact,
                aba_bound,
                0.1,
            )
            .is_none(),
            "same-angle re-solve ABA"
        );

        let rerooted_pose = triangular_pose_with_root(&model, 135.0, model.face_ids()[1]);
        let rerooted_bound = model.bind_pose(&rerooted_pose).unwrap();
        assert!(
            revalidate_axis_aligned_ef_boundary_v1(
                capability,
                prerequisite,
                &exact,
                rerooted_bound,
                0.1,
            )
            .is_none(),
            "a different fixed root is a different F instance"
        );

        let one_ulp_pose = triangular_pose(&model, next_up(135.0));
        let one_ulp_bound = model.bind_pose(&one_ulp_pose).unwrap();
        assert!(
            revalidate_axis_aligned_ef_boundary_v1(
                capability,
                prerequisite,
                &exact,
                one_ulp_bound,
                0.1,
            )
            .is_none(),
            "a one-ULP angle change is a different F instance"
        );

        let foreign = two_triangle_model_with_options(
            EdgeKind::Mountain,
            false,
            [(0.0, 0.0), (400.0, 0.0), (400.0, 400.0), (0.0, 400.0)],
            302,
        );
        let foreign_pose = triangular_pose(&foreign, 135.0);
        let foreign_bound = foreign.bind_pose(&foreign_pose).unwrap();
        assert!(
            revalidate_axis_aligned_ef_boundary_v1(
                capability,
                prerequisite,
                &exact,
                foreign_bound,
                0.1,
            )
            .is_none(),
            "foreign model/issuer"
        );
        assert!(
            revalidate_axis_aligned_ef_boundary_v1(
                capability,
                prerequisite,
                &exact,
                bound,
                next_up(0.1),
            )
            .is_none(),
            "paper thickness is bound by its complete binary64 bit pattern"
        );

        std::mem::swap(
            &mut capability.left_face_index,
            &mut capability.right_face_index,
        );
        assert!(
            revalidate_axis_aligned_ef_boundary_v1(capability, prerequisite, &exact, bound, 0.1,)
                .is_none(),
            "left/right face-index swap"
        );
        std::mem::swap(
            &mut capability.left_face_index,
            &mut capability.right_face_index,
        );

        capability.faces.swap(0, 1);
        assert!(
            revalidate_axis_aligned_ef_boundary_v1(capability, prerequisite, &exact, bound, 0.1,)
                .is_none(),
            "face-bound record swap"
        );
        capability.faces.swap(0, 1);

        let original_f_bit = capability.binary64_face_transforms[0].rotation[0][0];
        capability.binary64_face_transforms[0].rotation[0][0] = original_f_bit + 1;
        assert!(
            revalidate_axis_aligned_ef_boundary_v1(capability, prerequisite, &exact, bound, 0.1,)
                .is_none(),
            "one-ULP change to a sealed F matrix coefficient"
        );
    }

    fn exact_ef_limits_from_work(work: &AxisAlignedEfBoundaryWork) -> AxisAlignedEfBoundaryLimits {
        AxisAlignedEfBoundaryLimits {
            max_authenticated_faces: work.authenticated_faces,
            max_authenticated_hinges: work.authenticated_hinges,
            max_boundary_occurrences: work.boundary_occurrences,
            max_transform_scalar_lifts: work.transform_scalar_lifts,
            max_transform_bit_bindings: work.transform_bit_bindings,
            max_source_coordinate_lifts: work.source_coordinate_lifts,
            max_current_point_reconstructions: work.current_point_reconstructions,
            max_point_component_bounds: work.point_component_bounds,
            max_normal_component_bounds: work.normal_component_bounds,
            max_solid_component_bounds: work.solid_component_bounds,
            max_face_error_records: work.face_error_records,
            max_thickness_lifts: work.thickness_lifts,
            max_half_thickness_divisions: work.half_thickness_divisions,
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
        }
    }

    #[test]
    fn ef_boundary_all_counters_have_exact_and_one_short_limits() {
        let model = two_triangle_model();
        let pose = triangular_pose(&model, 135.0);
        let bound = model.bind_pose(&pose).unwrap();
        let exact = triangular_exact_pose(&model, &pose);
        let prerequisite_analysis = authenticated_ef_prerequisite(&exact, 0.1);
        let SingleTriangularHingePrerequisiteResult::Authenticated(prerequisite) =
            &prerequisite_analysis.result
        else {
            panic!("prerequisite");
        };
        let baseline = analyze_axis_aligned_ef_boundary_v1(
            prerequisite,
            &exact,
            bound,
            0.1,
            AxisAlignedEfBoundaryLimits::default(),
        )
        .unwrap();
        let exact_limits = exact_ef_limits_from_work(&baseline.work);
        let exact_analysis =
            analyze_axis_aligned_ef_boundary_v1(prerequisite, &exact, bound, 0.1, exact_limits)
                .unwrap();
        assert!(exact_analysis.capability.is_some());
        assert_eq!(exact_analysis.work, baseline.work);

        let assert_one_short = |resource: &str, limits: AxisAlignedEfBoundaryLimits| {
            assert!(
                matches!(
                    analyze_axis_aligned_ef_boundary_v1(prerequisite, &exact, bound, 0.1, limits,),
                    Err(AxisAlignedEfBoundaryError::ResourceLimitExceeded)
                ),
                "{resource}"
            );
        };
        macro_rules! structural_one_short {
            ($field:ident) => {
                if exact_limits.$field > 0 {
                    let mut limits = exact_limits;
                    limits.$field -= 1;
                    assert_one_short(stringify!($field), limits);
                }
            };
        }
        structural_one_short!(max_authenticated_faces);
        structural_one_short!(max_authenticated_hinges);
        structural_one_short!(max_boundary_occurrences);
        structural_one_short!(max_transform_scalar_lifts);
        structural_one_short!(max_transform_bit_bindings);
        structural_one_short!(max_source_coordinate_lifts);
        structural_one_short!(max_current_point_reconstructions);
        structural_one_short!(max_point_component_bounds);
        structural_one_short!(max_normal_component_bounds);
        structural_one_short!(max_solid_component_bounds);
        structural_one_short!(max_face_error_records);
        structural_one_short!(max_thickness_lifts);
        structural_one_short!(max_half_thickness_divisions);

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
    }

    #[test]
    fn ef_boundary_caller_cannot_expand_any_hard_cap() {
        let model = two_triangle_model();
        let pose = triangular_pose(&model, 135.0);
        let bound = model.bind_pose(&pose).unwrap();
        let exact = triangular_exact_pose(&model, &pose);
        let prerequisite_analysis = authenticated_ef_prerequisite(&exact, 0.1);
        let SingleTriangularHingePrerequisiteResult::Authenticated(prerequisite) =
            &prerequisite_analysis.result
        else {
            panic!("prerequisite");
        };
        let baseline = analyze_axis_aligned_ef_boundary_v1(
            prerequisite,
            &exact,
            bound,
            0.1,
            AxisAlignedEfBoundaryLimits::default(),
        )
        .unwrap();
        let oversized = AxisAlignedEfBoundaryLimits {
            max_authenticated_faces: usize::MAX,
            max_authenticated_hinges: usize::MAX,
            max_boundary_occurrences: usize::MAX,
            max_transform_scalar_lifts: usize::MAX,
            max_transform_bit_bindings: usize::MAX,
            max_source_coordinate_lifts: usize::MAX,
            max_current_point_reconstructions: usize::MAX,
            max_point_component_bounds: usize::MAX,
            max_normal_component_bounds: usize::MAX,
            max_solid_component_bounds: usize::MAX,
            max_face_error_records: usize::MAX,
            max_thickness_lifts: usize::MAX,
            max_half_thickness_divisions: usize::MAX,
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
        };
        let projected =
            analyze_axis_aligned_ef_boundary_v1(prerequisite, &exact, bound, 0.1, oversized)
                .unwrap();
        assert!(projected.capability.is_some());
        assert_eq!(projected.work, baseline.work);
    }
}
