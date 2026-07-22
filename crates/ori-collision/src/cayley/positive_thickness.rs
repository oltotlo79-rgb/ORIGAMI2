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
use ori_domain::{EdgeId, FaceId, VertexId};
use ori_kinematics::{
    BoundMaterialTreePose, Point3, prepare_material_hinge_pair_projection_v1,
    revalidate_material_hinge_pair_projection_v1,
};
use ori_topology::FoldAssignment;

use super::{
    CayleyError, CayleyLimits, CayleyStage, CayleyWork, ExactFacePose, ExactPoint3,
    ExactRigidTransform, ExactTreePoseLimits, ExactVector3, RATIONAL_CAYLEY_TREE_POSE_V1,
    RationalCayleyTreePose, WorkMeter, apply_exact_transform, canonical_point_eq, exact_f64,
    point3_array, prepare_rational_cayley_tree_pose_v1, rational_bits, rational_storage_bits,
    try_array3, verify_exact_rotation,
};

mod direct_f_affine_corridor;
mod direct_f_corridor;
mod ef_boundary;
mod exact_e_corridor;
mod exact_prism;
mod shared_hinge_corridor_admission;
mod shared_hinge_solid_classification;
mod shared_hinge_topology_margin;

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

fn exact_cross_vector(
    left: &ExactVector3,
    right: &ExactVector3,
    meter: &mut WorkMeter<'_>,
) -> Result<ExactVector3, CayleyError> {
    let component = |positive_left: usize,
                     positive_right: usize,
                     negative_left: usize,
                     negative_right: usize,
                     meter: &mut WorkMeter<'_>| {
        let positive = meter.multiply_rational(
            &left.coordinates[positive_left],
            &right.coordinates[positive_right],
            STAGE,
        )?;
        let negative = meter.multiply_rational(
            &left.coordinates[negative_left],
            &right.coordinates[negative_right],
            STAGE,
        )?;
        meter.subtract_rational(&positive, &negative, STAGE)
    };
    Ok(ExactVector3 {
        coordinates: [
            component(1, 2, 2, 1, meter)?,
            component(2, 0, 0, 2, meter)?,
            component(0, 1, 1, 0, meter)?,
        ],
    })
}

fn exact_vector_is_zero(vector: &ExactVector3) -> bool {
    vector.coordinates.iter().all(Zero::is_zero)
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
    let target_edge =
        (exact.bound.model().hinges().len() == 1).then(|| exact.bound.model().hinges()[0].edge());
    analyze_single_triangular_hinge_prerequisites_for_edge_v1(
        exact,
        paper_thickness_mm,
        target_edge,
        limits,
    )
}

fn analyze_single_triangular_hinge_prerequisites_for_edge_v1<'exact, 'pose>(
    exact: &'exact RationalCayleyTreePose<'pose>,
    paper_thickness_mm: f64,
    target_edge: Option<EdgeId>,
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
        target_edge,
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
    target_edge: Option<EdgeId>,
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
        || exact.faces.len() != model.face_ids().len()
        || exact.hinges.len() != model.hinges().len()
        || pose.hinge_angles().len() != model.hinges().len()
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

    let Some(target_edge) = target_edge else {
        return Ok(SingleTriangularHingePrerequisiteResult::Unresolved);
    };
    let mut hinge_matches = model
        .hinges()
        .iter()
        .enumerate()
        .filter(|(_, hinge)| hinge.edge() == target_edge);
    let Some((hinge_index, source_hinge)) = hinge_matches.next() else {
        return Ok(SingleTriangularHingePrerequisiteResult::Unresolved);
    };
    if hinge_matches.next().is_some() {
        return Ok(SingleTriangularHingePrerequisiteResult::Unresolved);
    }
    let exact_hinge = &exact.hinges[hinge_index];
    let pose_angle = &pose.hinge_angles()[hinge_index];
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

    let Some(indexes) = authenticate_triangular_hinge_indexes(exact, source_hinge, hinge_index)
    else {
        return Ok(SingleTriangularHingePrerequisiteResult::Unresolved);
    };
    let left_face = &exact.faces[indexes.left_face_index];
    let right_face = &exact.faces[indexes.right_face_index];
    if left_face.boundary.len() != 3
        || right_face.boundary.len() != 3
        || bound
            .face_boundary(left_face.face)
            .is_none_or(|boundary| boundary.vertices().len() != 3 || boundary.edges().len() != 3)
        || bound
            .face_boundary(right_face.face)
            .is_none_or(|boundary| boundary.vertices().len() != 3 || boundary.edges().len() != 3)
    {
        return Ok(SingleTriangularHingePrerequisiteResult::Unresolved);
    }
    validate_exact_pose_rational_inputs(exact, indexes, limits, work, meter)?;

    let rest_start = exact_point_at_stage(point3_array(source_hinge.start()), meter)?;
    let rest_end = exact_point_at_stage(point3_array(source_hinge.end()), meter)?;
    let left_rest = reauthenticate_exact_face_rest(bound, left_face, meter)?;
    let right_rest = reauthenticate_exact_face_rest(bound, right_face, meter)?;
    let start_vertex = exact_hinge.endpoint_vertices[0];
    let end_vertex = exact_hinge.endpoint_vertices[1];
    let left_pair = cyclic_vertex_pair(left_face, indexes.left_hinge_occurrence);
    let right_pair = cyclic_vertex_pair(right_face, indexes.right_hinge_occurrence);
    if left_pair != [start_vertex, end_vertex]
        || right_pair != [end_vertex, start_vertex]
        || !rest_vertex_matches(left_face, &left_rest, start_vertex, &rest_start)
        || !rest_vertex_matches(left_face, &left_rest, end_vertex, &rest_end)
        || !rest_vertex_matches(right_face, &right_rest, start_vertex, &rest_start)
        || !rest_vertex_matches(right_face, &right_rest, end_vertex, &rest_end)
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
    indexes: AuthenticatedTriangleIndexes,
    limits: &SingleTriangularHingePrerequisiteLimits,
    work: &mut SingleTriangularHingePrerequisiteWork,
    meter: &mut WorkMeter<'_>,
) -> Result<(), CayleyError> {
    for face in [
        &exact.faces[indexes.left_face_index],
        &exact.faces[indexes.right_face_index],
    ] {
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
    for value in exact.hinges[indexes.hinge_index]
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
    hinge_index: usize,
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
        hinge_index,
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

/// Exact zero-thickness proof that one authenticated material-tree hinge is
/// the complete intersection line of its two triangular incident faces.
///
/// `prepare_rational_cayley_tree_pose_v1` first reconstructs one watertight
/// pose: both incident faces reuse the exact hinge vertices. For a
/// non-parallel pair of non-degenerate triangle planes, their complete plane
/// intersection is therefore that hinge line. Because the hinge is a full
/// boundary edge of both triangles, their surface intersection is exactly the
/// finite shared edge and cannot extend into either relative interior.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ZeroThicknessSharedHingeBoundaryDiagnosticSummaryV1 {
    boundary_contact_proven_pairs: Vec<(FaceId, FaceId)>,
    area_overlap_proven_pairs: Vec<(FaceId, FaceId)>,
}

impl ZeroThicknessSharedHingeBoundaryDiagnosticSummaryV1 {
    pub(crate) fn proves_boundary_contact_pair(&self, first: FaceId, second: FaceId) -> bool {
        canonical_face_pair_is_present(&self.boundary_contact_proven_pairs, first, second)
    }

    pub(crate) fn proves_area_overlap_pair(&self, first: FaceId, second: FaceId) -> bool {
        canonical_face_pair_is_present(&self.area_overlap_proven_pairs, first, second)
    }

    pub(crate) fn classified_pairs(&self) -> usize {
        self.boundary_contact_proven_pairs
            .len()
            .saturating_add(self.area_overlap_proven_pairs.len())
    }
}

fn canonical_face_pair_is_present(
    proven_pairs: &[(FaceId, FaceId)],
    first: FaceId,
    second: FaceId,
) -> bool {
    let mut pair = [first, second];
    pair.sort_unstable_by_key(FaceId::canonical_bytes);
    if pair[0] == pair[1] {
        return false;
    }
    proven_pairs
        .binary_search_by(|candidate| {
            candidate
                .0
                .canonical_bytes()
                .cmp(&pair[0].canonical_bytes())
                .then_with(|| {
                    candidate
                        .1
                        .canonical_bytes()
                        .cmp(&pair[1].canonical_bytes())
                })
        })
        .is_ok()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ZeroThicknessSharedHingeBoundaryDiagnosticErrorV1 {
    ResourceLimitExceeded,
    InconsistentPose,
}

pub(crate) fn diagnose_bound_zero_thickness_shared_hinge_boundaries_v1(
    bound: BoundMaterialTreePose<'_>,
    candidate_pairs: &[(FaceId, FaceId)],
) -> Result<
    ZeroThicknessSharedHingeBoundaryDiagnosticSummaryV1,
    ZeroThicknessSharedHingeBoundaryDiagnosticErrorV1,
> {
    if candidate_pairs
        .iter()
        .any(|(first, second)| first.canonical_bytes() >= second.canonical_bytes())
        || !candidate_pairs.windows(2).all(|pairs| {
            pairs[0].0.canonical_bytes() < pairs[1].0.canonical_bytes()
                || (pairs[0].0 == pairs[1].0
                    && pairs[0].1.canonical_bytes() < pairs[1].1.canonical_bytes())
        })
    {
        return Err(ZeroThicknessSharedHingeBoundaryDiagnosticErrorV1::InconsistentPose);
    }
    if candidate_pairs.is_empty() {
        return Ok(ZeroThicknessSharedHingeBoundaryDiagnosticSummaryV1 {
            boundary_contact_proven_pairs: Vec::new(),
            area_overlap_proven_pairs: Vec::new(),
        });
    }
    let exact = match prepare_rational_cayley_tree_pose_v1(bound, ExactTreePoseLimits::default()) {
        Ok(exact) => exact,
        Err(CayleyError::ResourceLimitExceeded { .. }) => {
            return Err(ZeroThicknessSharedHingeBoundaryDiagnosticErrorV1::ResourceLimitExceeded);
        }
        Err(CayleyError::CertificateUnavailable { .. }) => {
            return Ok(ZeroThicknessSharedHingeBoundaryDiagnosticSummaryV1 {
                boundary_contact_proven_pairs: Vec::new(),
                area_overlap_proven_pairs: Vec::new(),
            });
        }
        Err(_) => {
            return Err(ZeroThicknessSharedHingeBoundaryDiagnosticErrorV1::InconsistentPose);
        }
    };
    if !exact.is_for(bound)
        || exact.faces.len() != bound.model().face_ids().len()
        || exact.hinges.len() != bound.model().hinges().len()
    {
        return Err(ZeroThicknessSharedHingeBoundaryDiagnosticErrorV1::InconsistentPose);
    }

    let arithmetic_limits = CayleyLimits::default();
    let mut meter = WorkMeter::new(&arithmetic_limits);
    let mut boundary_contact_proven_pairs = Vec::new();
    boundary_contact_proven_pairs
        .try_reserve_exact(candidate_pairs.len())
        .map_err(|_| ZeroThicknessSharedHingeBoundaryDiagnosticErrorV1::ResourceLimitExceeded)?;
    let mut area_overlap_proven_pairs = Vec::new();
    area_overlap_proven_pairs
        .try_reserve_exact(candidate_pairs.len())
        .map_err(|_| ZeroThicknessSharedHingeBoundaryDiagnosticErrorV1::ResourceLimitExceeded)?;
    for hinge in &exact.hinges {
        let mut pair = [hinge.parent, hinge.child];
        pair.sort_unstable_by_key(FaceId::canonical_bytes);
        if pair[0] == pair[1] {
            return Err(ZeroThicknessSharedHingeBoundaryDiagnosticErrorV1::InconsistentPose);
        }
        if candidate_pairs
            .binary_search_by(|candidate| {
                candidate
                    .0
                    .canonical_bytes()
                    .cmp(&pair[0].canonical_bytes())
                    .then_with(|| {
                        candidate
                            .1
                            .canonical_bytes()
                            .cmp(&pair[1].canonical_bytes())
                    })
            })
            .is_err()
        {
            continue;
        }
        let first = exact
            .faces
            .iter()
            .find(|face| face.face == hinge.parent)
            .ok_or(ZeroThicknessSharedHingeBoundaryDiagnosticErrorV1::InconsistentPose)?;
        let second = exact
            .faces
            .iter()
            .find(|face| face.face == hinge.child)
            .ok_or(ZeroThicknessSharedHingeBoundaryDiagnosticErrorV1::InconsistentPose)?;
        if first.boundary.len() != 3 || second.boundary.len() != 3 {
            continue;
        }
        if hinge.endpoint_vertices[0] == hinge.endpoint_vertices[1]
            || canonical_point_eq(&hinge.world_endpoints[0], &hinge.world_endpoints[1])
            || !exact_face_contains_hinge_endpoints(first, hinge)
            || !exact_face_contains_hinge_endpoints(second, hinge)
        {
            return Err(ZeroThicknessSharedHingeBoundaryDiagnosticErrorV1::InconsistentPose);
        }
        let first_normal = exact_triangle_normal(first, &mut meter)
            .map_err(map_zero_thickness_shared_hinge_boundary_error)?;
        let second_normal = exact_triangle_normal(second, &mut meter)
            .map_err(map_zero_thickness_shared_hinge_boundary_error)?;
        if exact_vector_is_zero(&first_normal) || exact_vector_is_zero(&second_normal) {
            return Err(ZeroThicknessSharedHingeBoundaryDiagnosticErrorV1::InconsistentPose);
        }
        let hinge_direction = exact_between(
            &hinge.world_endpoints[0],
            &hinge.world_endpoints[1],
            &mut meter,
        )
        .map_err(map_zero_thickness_shared_hinge_boundary_error)?;
        if exact_vector_is_zero(&hinge_direction)
            || !exact_dot(&first_normal, &hinge_direction, &mut meter)
                .map_err(map_zero_thickness_shared_hinge_boundary_error)?
                .is_zero()
            || !exact_dot(&second_normal, &hinge_direction, &mut meter)
                .map_err(map_zero_thickness_shared_hinge_boundary_error)?
                .is_zero()
        {
            return Err(ZeroThicknessSharedHingeBoundaryDiagnosticErrorV1::InconsistentPose);
        }
        let plane_intersection = exact_cross_vector(&first_normal, &second_normal, &mut meter)
            .map_err(map_zero_thickness_shared_hinge_boundary_error)?;
        if !exact_vector_is_zero(&plane_intersection) {
            boundary_contact_proven_pairs.push((pair[0], pair[1]));
            continue;
        }

        // Both non-degenerate triangles contain the same exact hinge line.
        // In their now-proven common plane, the sign of the dot product
        // between the two hinge-to-opposite-vertex side normals decides the
        // complete coplanar case. Opposite sides intersect only on the shared
        // boundary edge; the same side has a positive-area overlap in an open
        // strip immediately adjacent to that edge.
        let first_opposite = exact_triangle_opposite_hinge_vertex(first, hinge)?;
        let second_opposite = exact_triangle_opposite_hinge_vertex(second, hinge)?;
        let first_side = exact_cross_vector(
            &hinge_direction,
            &exact_between(&hinge.world_endpoints[0], first_opposite, &mut meter)
                .map_err(map_zero_thickness_shared_hinge_boundary_error)?,
            &mut meter,
        )
        .map_err(map_zero_thickness_shared_hinge_boundary_error)?;
        let second_side = exact_cross_vector(
            &hinge_direction,
            &exact_between(&hinge.world_endpoints[0], second_opposite, &mut meter)
                .map_err(map_zero_thickness_shared_hinge_boundary_error)?,
            &mut meter,
        )
        .map_err(map_zero_thickness_shared_hinge_boundary_error)?;
        if exact_vector_is_zero(&first_side) || exact_vector_is_zero(&second_side) {
            return Err(ZeroThicknessSharedHingeBoundaryDiagnosticErrorV1::InconsistentPose);
        }
        let side_relation = exact_dot(&first_side, &second_side, &mut meter)
            .map_err(map_zero_thickness_shared_hinge_boundary_error)?;
        if side_relation.is_positive() {
            area_overlap_proven_pairs.push((pair[0], pair[1]));
        } else if side_relation.is_negative() {
            boundary_contact_proven_pairs.push((pair[0], pair[1]));
        } else {
            return Err(ZeroThicknessSharedHingeBoundaryDiagnosticErrorV1::InconsistentPose);
        }
    }
    let sort_pairs = |pairs: &mut Vec<(FaceId, FaceId)>| {
        pairs.sort_unstable_by(|left, right| {
            left.0
                .canonical_bytes()
                .cmp(&right.0.canonical_bytes())
                .then_with(|| left.1.canonical_bytes().cmp(&right.1.canonical_bytes()))
        });
    };
    sort_pairs(&mut boundary_contact_proven_pairs);
    sort_pairs(&mut area_overlap_proven_pairs);
    if boundary_contact_proven_pairs
        .windows(2)
        .any(|pairs| pairs[0] == pairs[1])
        || area_overlap_proven_pairs
            .windows(2)
            .any(|pairs| pairs[0] == pairs[1])
        || boundary_contact_proven_pairs
            .iter()
            .any(|pair| canonical_face_pair_is_present(&area_overlap_proven_pairs, pair.0, pair.1))
        || boundary_contact_proven_pairs
            .len()
            .saturating_add(area_overlap_proven_pairs.len())
            > candidate_pairs.len()
    {
        return Err(ZeroThicknessSharedHingeBoundaryDiagnosticErrorV1::InconsistentPose);
    }
    Ok(ZeroThicknessSharedHingeBoundaryDiagnosticSummaryV1 {
        boundary_contact_proven_pairs,
        area_overlap_proven_pairs,
    })
}

fn exact_face_contains_hinge_endpoints(
    face: &ExactFacePose,
    hinge: &super::ExactHingePose,
) -> bool {
    hinge
        .endpoint_vertices
        .iter()
        .zip(&hinge.world_endpoints)
        .all(|(vertex, endpoint)| {
            face.boundary.iter().any(|(candidate, point)| {
                candidate == vertex && canonical_point_eq(point, endpoint)
            })
        })
}

fn exact_triangle_normal(
    face: &ExactFacePose,
    meter: &mut WorkMeter<'_>,
) -> Result<ExactVector3, CayleyError> {
    let first = exact_between(&face.boundary[0].1, &face.boundary[1].1, meter)?;
    let second = exact_between(&face.boundary[0].1, &face.boundary[2].1, meter)?;
    exact_cross_vector(&first, &second, meter)
}

fn exact_triangle_opposite_hinge_vertex<'a>(
    face: &'a ExactFacePose,
    hinge: &super::ExactHingePose,
) -> Result<&'a ExactPoint3, ZeroThicknessSharedHingeBoundaryDiagnosticErrorV1> {
    let mut opposite = face.boundary.iter().filter(|(vertex, _)| {
        *vertex != hinge.endpoint_vertices[0] && *vertex != hinge.endpoint_vertices[1]
    });
    let point = &opposite
        .next()
        .ok_or(ZeroThicknessSharedHingeBoundaryDiagnosticErrorV1::InconsistentPose)?
        .1;
    if opposite.next().is_some() {
        return Err(ZeroThicknessSharedHingeBoundaryDiagnosticErrorV1::InconsistentPose);
    }
    Ok(point)
}

const fn map_zero_thickness_shared_hinge_boundary_error(
    error: CayleyError,
) -> ZeroThicknessSharedHingeBoundaryDiagnosticErrorV1 {
    match error {
        CayleyError::ResourceLimitExceeded { .. } => {
            ZeroThicknessSharedHingeBoundaryDiagnosticErrorV1::ResourceLimitExceeded
        }
        _ => ZeroThicknessSharedHingeBoundaryDiagnosticErrorV1::InconsistentPose,
    }
}

/// Sanitized result of the strictly two-face/one-hinge positive-thickness
/// diagnostic path.
///
/// This result is deliberately diagnostic-only. `Allowed` means the complete
/// E/F solid intersection was explained by the authenticated finite hinge
/// model; it does not issue a scene-safe certificate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SharedHingeSolidDiagnosticDispositionV1 {
    Allowed,
    Penetrating,
    Indeterminate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SharedHingeSolidDiagnosticSummaryV1 {
    pub(crate) first_face: FaceId,
    pub(crate) second_face: FaceId,
    pub(crate) evidence: crate::IntersectionEvidenceV2,
    pub(crate) policy_decision: crate::TopologyContactDecision,
    pub(crate) disposition: SharedHingeSolidDiagnosticDispositionV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SharedHingeSolidDiagnosticErrorV1 {
    ResourceLimitExceeded,
    InconsistentPose,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PositiveThicknessPrismPairDispositionV1 {
    Separated,
    Touching,
    SharedHingeCorridorAllowed,
    SharedVertexCorridorAllowed,
    Penetrating,
    Indeterminate,
}

// Version-fixed finite joint envelope. Exact SAT overlap may be admitted only
// when every intersection vertex stays inside this bounded expansion of the
// authenticated shared vertex or hinge segment. It is deliberately small
// relative to the production triangular fixtures and never scales with face
// size, so a transversal extending into either face cannot become joint
// contact merely because the paper is large.
const SHARED_FEATURE_CORRIDOR_HALF_EXTENT_MULTIPLIER_V1: i64 = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PositiveThicknessPrismPairDiagnosticV1 {
    pub(crate) first_face: FaceId,
    pub(crate) second_face: FaceId,
    pub(crate) disposition: PositiveThicknessPrismPairDispositionV1,
}

pub(crate) fn diagnose_bound_positive_thickness_prism_pairs_v1(
    bound: BoundMaterialTreePose<'_>,
    paper_thickness_mm: f64,
    max_unordered_face_pairs: usize,
) -> Result<Vec<PositiveThicknessPrismPairDiagnosticV1>, SharedHingeSolidDiagnosticErrorV1> {
    use exact_prism::{
        ExactPrismIntersectionKind, ExactPrismLimits, ExactTriangularPrismInput,
        analyze_exact_prism_pair_v1,
    };

    if !positive_finite_binary64(paper_thickness_mm)
        || bound.model().face_ids() != bound.pose().face_ids()
        || bound.model().hinges() != bound.pose().hinges()
    {
        return Err(SharedHingeSolidDiagnosticErrorV1::InconsistentPose);
    }
    let exact = prepare_rational_cayley_tree_pose_v1(bound, ExactTreePoseLimits::default())
        .map_err(|error| match error {
            CayleyError::ResourceLimitExceeded { .. } => {
                SharedHingeSolidDiagnosticErrorV1::ResourceLimitExceeded
            }
            _ => SharedHingeSolidDiagnosticErrorV1::InconsistentPose,
        })?;
    let pair_count = exact
        .faces
        .len()
        .checked_mul(exact.faces.len().saturating_sub(1))
        .and_then(|value| value.checked_div(2))
        .ok_or(SharedHingeSolidDiagnosticErrorV1::ResourceLimitExceeded)?;
    if pair_count > max_unordered_face_pairs {
        return Err(SharedHingeSolidDiagnosticErrorV1::ResourceLimitExceeded);
    }
    let half_thickness = BigRational::from_float(paper_thickness_mm)
        .ok_or(SharedHingeSolidDiagnosticErrorV1::InconsistentPose)?
        / BigRational::from_integer(2.into());
    let prism = |face: &ExactFacePose| -> Option<ExactTriangularPrismInput> {
        (face.boundary.len() == 3).then(|| ExactTriangularPrismInput {
            mid_surface: [
                face.boundary[0].1.clone(),
                face.boundary[1].1.clone(),
                face.boundary[2].1.clone(),
            ],
            material_normal: ExactVector3 {
                coordinates: [
                    face.transform.rotation[0][1].clone(),
                    face.transform.rotation[1][1].clone(),
                    face.transform.rotation[2][1].clone(),
                ],
            },
            half_thickness: half_thickness.clone(),
        })
    };
    let mut result = Vec::new();
    result
        .try_reserve_exact(pair_count)
        .map_err(|_| SharedHingeSolidDiagnosticErrorV1::ResourceLimitExceeded)?;
    for first in 0..exact.faces.len() {
        for second in first + 1..exact.faces.len() {
            let (Some(first_prism), Some(second_prism)) =
                (prism(&exact.faces[first]), prism(&exact.faces[second]))
            else {
                return Err(SharedHingeSolidDiagnosticErrorV1::InconsistentPose);
            };
            let mut shared_vertices = [None, None, None];
            let mut shared_vertex_count = 0;
            for entry in &exact.faces[first].boundary {
                if exact.faces[second]
                    .boundary
                    .iter()
                    .any(|(other, _)| *other == entry.0)
                {
                    shared_vertices[shared_vertex_count] = Some(entry);
                    shared_vertex_count += 1;
                }
            }
            let bounds = |input: &ExactTriangularPrismInput| {
                let mut lower: [Option<BigRational>; 3] = [None, None, None];
                let mut upper: [Option<BigRational>; 3] = [None, None, None];
                for point in &input.mid_surface {
                    for sign in [-1_i8, 1_i8] {
                        for axis in 0..3 {
                            let offset =
                                &input.material_normal.coordinates[axis] * &input.half_thickness;
                            let value = if sign < 0 {
                                &point.coordinates[axis] - offset
                            } else {
                                &point.coordinates[axis] + offset
                            };
                            lower[axis] = Some(lower[axis].as_ref().map_or_else(
                                || value.clone(),
                                |current| current.min(&value).clone(),
                            ));
                            upper[axis] = Some(upper[axis].as_ref().map_or_else(
                                || value.clone(),
                                |current| current.max(&value).clone(),
                            ));
                        }
                    }
                }
                (
                    lower.map(|value| value.expect("a triangular prism has vertices")),
                    upper.map(|value| value.expect("a triangular prism has vertices")),
                )
            };
            let (first_lower, first_upper) = bounds(&first_prism);
            let (second_lower, second_upper) = bounds(&second_prism);
            let intersection_lower: [BigRational; 3] = std::array::from_fn(|axis| {
                if first_lower[axis] >= second_lower[axis] {
                    first_lower[axis].clone()
                } else {
                    second_lower[axis].clone()
                }
            });
            let intersection_upper: [BigRational; 3] = std::array::from_fn(|axis| {
                if first_upper[axis] <= second_upper[axis] {
                    first_upper[axis].clone()
                } else {
                    second_upper[axis].clone()
                }
            });
            if (0..3).any(|axis| intersection_lower[axis] > intersection_upper[axis]) {
                result.push(PositiveThicknessPrismPairDiagnosticV1 {
                    first_face: exact.faces[first].face,
                    second_face: exact.faces[second].face,
                    disposition: PositiveThicknessPrismPairDispositionV1::Separated,
                });
                continue;
            }
            let radius = &half_thickness
                * BigRational::from_integer(
                    SHARED_FEATURE_CORRIDOR_HALF_EXTENT_MULTIPLIER_V1.into(),
                );
            let aabb_corridor_disposition = if shared_vertex_count == 1 {
                let center = &shared_vertices[0].expect("one counted shared vertex").1;
                (0..3)
                    .all(|axis| {
                        intersection_lower[axis] >= center.coordinates[axis].clone() - &radius
                            && intersection_upper[axis]
                                <= center.coordinates[axis].clone() + &radius
                    })
                    .then_some(PositiveThicknessPrismPairDispositionV1::SharedVertexCorridorAllowed)
            } else if shared_vertex_count == 2 {
                let first_shared = shared_vertices[0].expect("two counted shared vertices");
                let second_shared = shared_vertices[1].expect("two counted shared vertices");
                (0..3)
                    .all(|axis| {
                        let first_coordinate = &first_shared.1.coordinates[axis];
                        let second_coordinate = &second_shared.1.coordinates[axis];
                        let (lower, upper) = if first_coordinate <= second_coordinate {
                            (first_coordinate, second_coordinate)
                        } else {
                            (second_coordinate, first_coordinate)
                        };
                        intersection_lower[axis] >= lower - &radius
                            && intersection_upper[axis] <= upper + &radius
                    })
                    .then_some(PositiveThicknessPrismPairDispositionV1::SharedHingeCorridorAllowed)
            } else {
                None
            };
            if let Some(disposition) = aabb_corridor_disposition {
                result.push(PositiveThicknessPrismPairDiagnosticV1 {
                    first_face: exact.faces[first].face,
                    second_face: exact.faces[second].face,
                    disposition,
                });
                continue;
            }
            let intersection = analyze_exact_prism_pair_v1(
                &first_prism,
                &second_prism,
                ExactPrismLimits::default(),
            )
            .map_err(|_| SharedHingeSolidDiagnosticErrorV1::ResourceLimitExceeded)?;
            let intersection = intersection
                .intersection
                .ok_or(SharedHingeSolidDiagnosticErrorV1::InconsistentPose)?;
            let shared_vertex_corridor = (shared_vertex_count == 1).then(|| {
                let center = &shared_vertices[0].expect("one counted shared vertex").1;
                let radius = &half_thickness
                    * BigRational::from_integer(
                        SHARED_FEATURE_CORRIDOR_HALF_EXTENT_MULTIPLIER_V1.into(),
                    );
                intersection.canonical_vertices().iter().all(|point| {
                    point
                        .coordinates
                        .iter()
                        .zip(&center.coordinates)
                        .all(|(coordinate, origin)| (coordinate - origin).abs() <= radius)
                })
            }) == Some(true);
            let shared_hinge_corridor = (shared_vertex_count == 2).then(|| {
                let first_shared = shared_vertices[0].expect("two counted shared vertices");
                let second_shared = shared_vertices[1].expect("two counted shared vertices");
                let radius = &half_thickness
                    * BigRational::from_integer(
                        SHARED_FEATURE_CORRIDOR_HALF_EXTENT_MULTIPLIER_V1.into(),
                    );
                intersection.canonical_vertices().iter().all(|point| {
                    (0..3).all(|axis| {
                        let first = &first_shared.1.coordinates[axis];
                        let second = &second_shared.1.coordinates[axis];
                        let lower = first.min(second) - &radius;
                        let upper = first.max(second) + &radius;
                        point.coordinates[axis] >= lower && point.coordinates[axis] <= upper
                    })
                })
            }) == Some(true);
            let disposition = match intersection.kind() {
                ExactPrismIntersectionKind::Empty => {
                    PositiveThicknessPrismPairDispositionV1::Separated
                }
                ExactPrismIntersectionKind::Point
                | ExactPrismIntersectionKind::Line
                | ExactPrismIntersectionKind::CoplanarArea => {
                    PositiveThicknessPrismPairDispositionV1::Touching
                }
                ExactPrismIntersectionKind::PositiveVolume if shared_hinge_corridor => {
                    PositiveThicknessPrismPairDispositionV1::SharedHingeCorridorAllowed
                }
                ExactPrismIntersectionKind::PositiveVolume if shared_vertex_corridor => {
                    PositiveThicknessPrismPairDispositionV1::SharedVertexCorridorAllowed
                }
                ExactPrismIntersectionKind::PositiveVolume => {
                    PositiveThicknessPrismPairDispositionV1::Penetrating
                }
                ExactPrismIntersectionKind::Planar => {
                    PositiveThicknessPrismPairDispositionV1::Indeterminate
                }
            };
            result.push(PositiveThicknessPrismPairDiagnosticV1 {
                first_face: exact.faces[first].face,
                second_face: exact.faces[second].face,
                disposition,
            });
        }
    }
    Ok(result)
}

/// Runs the complete private solid classifier and exports only its sanitized
/// semantic cell. The gate is structurally limited to one positive-thickness
/// pair formed by exactly two triangular faces and their only hinge.
///
/// A three-face V-fold therefore cannot route its vertex-sharing outer pair
/// through this function. Unsupported geometry returns `Ok(None)` and remains
/// an explicit indeterminate pair at the caller.
pub(crate) fn diagnose_bound_shared_hinge_solid_v1(
    bound: BoundMaterialTreePose<'_>,
    paper_thickness_mm: f64,
) -> Result<Option<SharedHingeSolidDiagnosticSummaryV1>, SharedHingeSolidDiagnosticErrorV1> {
    diagnose_bound_shared_hinge_solid_for_edge_v1(bound, paper_thickness_mm, None)
}

pub(crate) fn diagnose_bound_shared_hinge_solid_for_edge_v1(
    bound: BoundMaterialTreePose<'_>,
    paper_thickness_mm: f64,
    target_edge: Option<EdgeId>,
) -> Result<Option<SharedHingeSolidDiagnosticSummaryV1>, SharedHingeSolidDiagnosticErrorV1> {
    use direct_f_corridor::{
        DirectFFiniteHingeCorridorLimits, analyze_direct_f_finite_hinge_corridor_v1,
    };
    use ef_boundary::{AxisAlignedEfBoundaryLimits, analyze_axis_aligned_ef_boundary_v1};
    use exact_e_corridor::{
        ExactEFiniteHingeCorridorLimits, analyze_exact_e_finite_hinge_corridor_v1,
    };
    use shared_hinge_corridor_admission::{
        SharedHingeCorridorAdmissionLimitsV1, analyze_shared_hinge_corridor_admission_v1,
    };
    use shared_hinge_solid_classification::{
        SharedHingePositiveThicknessPairClassV1, SharedHingeSolidClassificationLimitsV1,
        SharedHingeSolidClassificationResultV1, analyze_shared_hinge_solid_classification_v1,
        revalidate_independent_shared_hinge_solid_classification_v1,
        revalidate_shared_hinge_solid_classification_v1,
    };
    use shared_hinge_topology_margin::{
        SharedHingeNativeExactTopologyMarginLimitsV1,
        analyze_shared_hinge_native_exact_topology_margin_v1,
    };

    if !positive_finite_binary64(paper_thickness_mm)
        || bound.model().face_ids() != bound.pose().face_ids()
        || bound.model().hinges() != bound.pose().hinges()
    {
        return Ok(None);
    }
    let target_edge = match target_edge {
        Some(edge) => edge,
        None if bound.model().hinges().len() == 1 => bound.model().hinges()[0].edge(),
        None => return Ok(None),
    };
    let mut matches = bound
        .model()
        .hinges()
        .iter()
        .enumerate()
        .filter(|(_, hinge)| hinge.edge() == target_edge);
    let Some((target_hinge_index, target_hinge)) = matches.next() else {
        return Ok(None);
    };
    if matches.next().is_some() {
        return Ok(None);
    }
    let mut face_pair = [target_hinge.left_face(), target_hinge.right_face()];
    face_pair.sort_unstable_by_key(FaceId::canonical_bytes);

    let exact = match prepare_rational_cayley_tree_pose_v1(bound, ExactTreePoseLimits::default()) {
        Ok(exact) => exact,
        Err(CayleyError::ResourceLimitExceeded { .. }) => {
            return Err(SharedHingeSolidDiagnosticErrorV1::ResourceLimitExceeded);
        }
        Err(CayleyError::InvariantFailure { .. } | CayleyError::BoundTreeInconsistent { .. }) => {
            return Err(SharedHingeSolidDiagnosticErrorV1::InconsistentPose);
        }
        Err(_) => return Ok(None),
    };
    if exact.hinges[target_hinge_index].angle_magnitude_bits == 90.0_f64.to_bits() {
        // At exactly 90 degrees the centered slabs meet the finite corridor
        // boundary. Reversing the canonical hinge endpoint can change the
        // last binary64 bit of direct-F while leaving the physical pose
        // unchanged, so an `Allowed` result here would make identity labels
        // observable in collision semantics. Freeze this exact boundary to
        // the safe, order-independent hold used by the public matrix.
        return Ok(Some(SharedHingeSolidDiagnosticSummaryV1 {
            first_face: face_pair[0],
            second_face: face_pair[1],
            evidence: crate::IntersectionEvidenceV2::Indeterminate,
            policy_decision: crate::TopologyContactDecision::Indeterminate,
            disposition: SharedHingeSolidDiagnosticDispositionV1::Indeterminate,
        }));
    }
    let prerequisite_analysis = analyze_single_triangular_hinge_prerequisites_for_edge_v1(
        &exact,
        paper_thickness_mm,
        Some(target_edge),
        SingleTriangularHingePrerequisiteLimits::default(),
    )
    .map_err(
        |SingleTriangularHingePrerequisiteError::ResourceLimitExceeded| {
            SharedHingeSolidDiagnosticErrorV1::ResourceLimitExceeded
        },
    )?;
    let prerequisite = match &prerequisite_analysis.result {
        SingleTriangularHingePrerequisiteResult::Authenticated(prerequisite) => Some(prerequisite),
        SingleTriangularHingePrerequisiteResult::LayerOffsetUnmodeled
        | SingleTriangularHingePrerequisiteResult::Unresolved => None,
    };
    let ef_analysis = prerequisite
        .map(|prerequisite| {
            analyze_axis_aligned_ef_boundary_v1(
                prerequisite,
                &exact,
                bound,
                paper_thickness_mm,
                AxisAlignedEfBoundaryLimits::default(),
            )
        })
        .transpose()
        .map_err(|_| SharedHingeSolidDiagnosticErrorV1::ResourceLimitExceeded)?;
    let ef = ef_analysis
        .as_ref()
        .and_then(|analysis| analysis.capability.as_ref());
    let exact_e_analysis = analyze_exact_e_finite_hinge_corridor_v1(
        &prerequisite_analysis,
        ef,
        &exact,
        bound,
        paper_thickness_mm,
        ExactEFiniteHingeCorridorLimits::default(),
    )
    .map_err(|_| SharedHingeSolidDiagnosticErrorV1::ResourceLimitExceeded)?;
    let direct_f_analysis = analyze_direct_f_finite_hinge_corridor_v1(
        &prerequisite_analysis,
        ef,
        &exact_e_analysis,
        &exact,
        bound,
        paper_thickness_mm,
        DirectFFiniteHingeCorridorLimits::default(),
    )
    .map_err(|_| SharedHingeSolidDiagnosticErrorV1::ResourceLimitExceeded)?;
    let admission_analysis = analyze_shared_hinge_corridor_admission_v1(
        &prerequisite_analysis,
        ef,
        &exact_e_analysis,
        &direct_f_analysis,
        &exact,
        bound,
        paper_thickness_mm,
        SharedHingeCorridorAdmissionLimitsV1::default(),
    )
    .map_err(|_| SharedHingeSolidDiagnosticErrorV1::ResourceLimitExceeded)?;
    let margin_analysis = analyze_shared_hinge_native_exact_topology_margin_v1(
        &prerequisite_analysis,
        ef,
        &exact,
        bound,
        paper_thickness_mm,
        SharedHingeNativeExactTopologyMarginLimitsV1::default(),
    )
    .map_err(|_| SharedHingeSolidDiagnosticErrorV1::ResourceLimitExceeded)?;
    let classification = analyze_shared_hinge_solid_classification_v1(
        &prerequisite_analysis,
        ef,
        &exact_e_analysis,
        &direct_f_analysis,
        &admission_analysis,
        &margin_analysis,
        &exact,
        bound,
        paper_thickness_mm,
        SharedHingeSolidClassificationLimitsV1::default(),
    )
    .map_err(|_| SharedHingeSolidDiagnosticErrorV1::ResourceLimitExceeded)?;

    let contract = match &classification.result {
        SharedHingeSolidClassificationResultV1::Classified(record) => {
            let (Some(prerequisite), Some(ef), Some((exact_e, _)), Some((direct_f, _))) = (
                prerequisite,
                ef,
                exact_e_analysis.authenticated_contained_capability_and_work(),
                direct_f_analysis.authenticated_contained_capability_and_work(),
            ) else {
                return Err(SharedHingeSolidDiagnosticErrorV1::InconsistentPose);
            };
            let admission = admission_analysis
                .authenticated_admission_capability_and_work()
                .map(|(capability, _)| capability);
            let margin = margin_analysis
                .authenticated_capability_and_work()
                .map(|(capability, _)| capability);
            revalidate_shared_hinge_solid_classification_v1(
                record,
                prerequisite,
                ef,
                exact_e,
                direct_f,
                admission,
                None,
                &exact,
                bound,
                paper_thickness_mm,
            )
            .or_else(|| {
                revalidate_shared_hinge_solid_classification_v1(
                    record,
                    prerequisite,
                    ef,
                    exact_e,
                    direct_f,
                    None,
                    margin,
                    &exact,
                    bound,
                    paper_thickness_mm,
                )
            })
            .map(|revalidated| revalidated.diagnostic_contract())
            .ok_or(SharedHingeSolidDiagnosticErrorV1::InconsistentPose)?
        }
        SharedHingeSolidClassificationResultV1::IndependentlyClassified(record) => {
            let (Some(prerequisite), Some(ef), Some((margin, _))) = (
                prerequisite,
                ef,
                margin_analysis.authenticated_capability_and_work(),
            ) else {
                return Err(SharedHingeSolidDiagnosticErrorV1::InconsistentPose);
            };
            revalidate_independent_shared_hinge_solid_classification_v1(
                record,
                prerequisite,
                ef,
                margin,
                &exact,
                bound,
                paper_thickness_mm,
            )
            .map(|revalidated| revalidated.diagnostic_contract())
            .ok_or(SharedHingeSolidDiagnosticErrorV1::InconsistentPose)?
        }
        SharedHingeSolidClassificationResultV1::EvidenceUnavailable(_) => {
            return Ok(Some(SharedHingeSolidDiagnosticSummaryV1 {
                first_face: face_pair[0],
                second_face: face_pair[1],
                evidence: crate::IntersectionEvidenceV2::Indeterminate,
                policy_decision: crate::TopologyContactDecision::Indeterminate,
                disposition: SharedHingeSolidDiagnosticDispositionV1::Indeterminate,
            }));
        }
    };
    let (class, evidence, policy_decision) = contract;
    let disposition = match class {
        SharedHingePositiveThicknessPairClassV1::SharedFeatureOnlyContact
        | SharedHingePositiveThicknessPairClassV1::AllowedFiniteCorridorOverlap
        | SharedHingePositiveThicknessPairClassV1::AllowedBoundaryContact => {
            SharedHingeSolidDiagnosticDispositionV1::Allowed
        }
        SharedHingePositiveThicknessPairClassV1::PositiveVolumeIntersection => {
            SharedHingeSolidDiagnosticDispositionV1::Penetrating
        }
        SharedHingePositiveThicknessPairClassV1::EvidenceUnavailable => {
            SharedHingeSolidDiagnosticDispositionV1::Indeterminate
        }
    };
    Ok(Some(SharedHingeSolidDiagnosticSummaryV1 {
        first_face: face_pair[0],
        second_face: face_pair[1],
        evidence,
        policy_decision,
        disposition,
    }))
}

/// Opaque, issuer-bound positive-thickness boundary rails for the strictly
/// two-triangle/one-hinge class admitted by the exact corridor classifier.
///
/// The four rails are observations in native material millimetres. They are
/// not a collision clearance or a printable-solid certificate.
#[derive(Debug)]
pub struct NativeSingleHingeThicknessBoundaryV1<'a> {
    bound: BoundMaterialTreePose<'a>,
    paper_thickness_bits: u64,
    hinge: EdgeId,
    left_face: FaceId,
    right_face: FaceId,
    endpoint_vertices: [VertexId; 2],
    left_front: [[f64; 3]; 2],
    left_back: [[f64; 3]; 2],
    right_front: [[f64; 3]; 2],
    right_back: [[f64; 3]; 2],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SingleHingeThicknessBoundaryObservationV1 {
    pub hinge: EdgeId,
    pub left_face: FaceId,
    pub right_face: FaceId,
    pub endpoint_vertices: [VertexId; 2],
    pub left_front: [[f64; 3]; 2],
    pub left_back: [[f64; 3]; 2],
    pub right_front: [[f64; 3]; 2],
    pub right_back: [[f64; 3]; 2],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SingleHingeThicknessBoundaryErrorV1 {
    ResourceLimitExceeded,
    InconsistentPose,
}

impl<'a> NativeSingleHingeThicknessBoundaryV1<'a> {
    #[must_use]
    pub const fn observation(&self) -> SingleHingeThicknessBoundaryObservationV1 {
        SingleHingeThicknessBoundaryObservationV1 {
            hinge: self.hinge,
            left_face: self.left_face,
            right_face: self.right_face,
            endpoint_vertices: self.endpoint_vertices,
            left_front: self.left_front,
            left_back: self.left_back,
            right_front: self.right_front,
            right_back: self.right_back,
        }
    }
}

pub fn prepare_single_hinge_thickness_boundary_v1(
    bound: BoundMaterialTreePose<'_>,
    paper_thickness_mm: f64,
) -> Result<Option<NativeSingleHingeThicknessBoundaryV1<'_>>, SingleHingeThicknessBoundaryErrorV1> {
    if bound.model().hinges().len() != 1 {
        return Ok(None);
    }
    prepare_single_hinge_thickness_boundary_for_edge_v1(
        bound,
        paper_thickness_mm,
        bound.model().hinges()[0].edge(),
        false,
    )
}

fn prepare_single_hinge_thickness_boundary_for_edge_v1(
    bound: BoundMaterialTreePose<'_>,
    paper_thickness_mm: f64,
    target_edge: EdgeId,
    allow_projected_geometry_only: bool,
) -> Result<Option<NativeSingleHingeThicknessBoundaryV1<'_>>, SingleHingeThicknessBoundaryErrorV1> {
    let diagnostic =
        diagnose_bound_shared_hinge_solid_for_edge_v1(bound, paper_thickness_mm, Some(target_edge))
            .map_err(|error| match error {
                SharedHingeSolidDiagnosticErrorV1::ResourceLimitExceeded => {
                    SingleHingeThicknessBoundaryErrorV1::ResourceLimitExceeded
                }
                SharedHingeSolidDiagnosticErrorV1::InconsistentPose => {
                    SingleHingeThicknessBoundaryErrorV1::InconsistentPose
                }
            })?;
    if diagnostic.is_none_or(|summary| {
        summary.disposition != SharedHingeSolidDiagnosticDispositionV1::Allowed
    }) {
        if !allow_projected_geometry_only {
            return Ok(None);
        }
        let exact = prepare_rational_cayley_tree_pose_v1(bound, ExactTreePoseLimits::default())
            .map_err(|error| match error {
                CayleyError::ResourceLimitExceeded { .. } => {
                    SingleHingeThicknessBoundaryErrorV1::ResourceLimitExceeded
                }
                _ => SingleHingeThicknessBoundaryErrorV1::InconsistentPose,
            })?;
        let projected = analyze_single_triangular_hinge_prerequisites_for_edge_v1(
            &exact,
            paper_thickness_mm,
            Some(target_edge),
            SingleTriangularHingePrerequisiteLimits::default(),
        )
        .map_err(|_| SingleHingeThicknessBoundaryErrorV1::ResourceLimitExceeded)?;
        if !matches!(
            projected.result,
            SingleTriangularHingePrerequisiteResult::Authenticated(_)
        ) {
            return Ok(None);
        }
    }
    let model = bound.model();
    let pose = bound.pose();
    let mut hinge_matches = model
        .hinges()
        .iter()
        .filter(|hinge| hinge.edge() == target_edge);
    let Some(hinge) = hinge_matches.next() else {
        return Err(SingleHingeThicknessBoundaryErrorV1::InconsistentPose);
    };
    if hinge_matches.next().is_some() {
        return Err(SingleHingeThicknessBoundaryErrorV1::InconsistentPose);
    }
    let left_transform = pose
        .face_transform(hinge.left_face())
        .ok_or(SingleHingeThicknessBoundaryErrorV1::InconsistentPose)?;
    let right_transform = pose
        .face_transform(hinge.right_face())
        .ok_or(SingleHingeThicknessBoundaryErrorV1::InconsistentPose)?;
    let endpoints = [hinge.start(), hinge.end()];
    let left_boundary = bound
        .face_boundary(hinge.left_face())
        .ok_or(SingleHingeThicknessBoundaryErrorV1::InconsistentPose)?;
    let occurrence = unique_edge_occurrence(left_boundary.edges(), hinge.edge())
        .ok_or(SingleHingeThicknessBoundaryErrorV1::InconsistentPose)?;
    let endpoint_vertices = [
        left_boundary.vertices()[occurrence],
        left_boundary.vertices()[(occurrence + 1) % left_boundary.vertices().len()],
    ];
    let left_world = endpoints.map(|point| {
        left_transform
            .apply_point(point)
            .map(point3_array)
            .map_err(|_| SingleHingeThicknessBoundaryErrorV1::InconsistentPose)
    });
    let right_world = endpoints.map(|point| {
        right_transform
            .apply_point(point)
            .map(point3_array)
            .map_err(|_| SingleHingeThicknessBoundaryErrorV1::InconsistentPose)
    });
    let left_world = [left_world[0]?, left_world[1]?];
    let right_world = [right_world[0]?, right_world[1]?];
    if left_world
        .iter()
        .zip(right_world)
        .any(|(left, right)| left.map(f64::to_bits) != right.map(f64::to_bits))
    {
        return Err(SingleHingeThicknessBoundaryErrorV1::InconsistentPose);
    }
    let local_y = Point3::new(0.0, 1.0, 0.0)
        .map_err(|_| SingleHingeThicknessBoundaryErrorV1::InconsistentPose)?;
    let left_normal = point3_array(
        left_transform
            .apply_vector(local_y)
            .map_err(|_| SingleHingeThicknessBoundaryErrorV1::InconsistentPose)?,
    );
    let right_normal = point3_array(
        right_transform
            .apply_vector(local_y)
            .map_err(|_| SingleHingeThicknessBoundaryErrorV1::InconsistentPose)?,
    );
    let half = paper_thickness_mm / 2.0;
    let offset = |points: [[f64; 3]; 2], normal: [f64; 3], sign: f64| {
        points.map(|point| {
            std::array::from_fn(|axis| {
                let value = point[axis] + sign * half * normal[axis];
                if value == 0.0 { 0.0 } else { value }
            })
        })
    };
    Ok(Some(NativeSingleHingeThicknessBoundaryV1 {
        bound,
        paper_thickness_bits: paper_thickness_mm.to_bits(),
        hinge: hinge.edge(),
        left_face: hinge.left_face(),
        right_face: hinge.right_face(),
        endpoint_vertices,
        left_front: offset(left_world, left_normal, 1.0),
        left_back: offset(left_world, left_normal, -1.0),
        right_front: offset(right_world, right_normal, 1.0),
        right_back: offset(right_world, right_normal, -1.0),
    }))
}

pub fn revalidate_single_hinge_thickness_boundary_v1(
    capability: &NativeSingleHingeThicknessBoundaryV1<'_>,
    bound: BoundMaterialTreePose<'_>,
    paper_thickness_mm: f64,
) -> Option<SingleHingeThicknessBoundaryObservationV1> {
    if !positive_finite_binary64(paper_thickness_mm)
        || capability.paper_thickness_bits != paper_thickness_mm.to_bits()
        || !std::ptr::eq(capability.bound.model(), bound.model())
        || !std::ptr::eq(capability.bound.pose(), bound.pose())
    {
        return None;
    }
    Some(capability.observation())
}

pub const MAX_COMPOSED_THICKNESS_HINGES_V1: usize = 63;

fn prepare_projected_incident_hinge_boundary_v1(
    bound: BoundMaterialTreePose<'_>,
    paper_thickness_mm: f64,
    edge: EdgeId,
) -> Result<Option<NativeSingleHingeThicknessBoundaryV1<'_>>, SingleHingeThicknessBoundaryErrorV1> {
    let projection = prepare_material_hinge_pair_projection_v1(bound, edge)
        .map_err(|_| SingleHingeThicknessBoundaryErrorV1::InconsistentPose)?;
    let Some(input) = revalidate_material_hinge_pair_projection_v1(&projection, bound) else {
        return Err(SingleHingeThicknessBoundaryErrorV1::InconsistentPose);
    };
    if !positive_finite_binary64(paper_thickness_mm)
        || input.boundaries.iter().any(|boundary| boundary.len() != 3)
        || input.face_indexes[0] == input.face_indexes[1]
        || input
            .excluded_face_indexes
            .iter()
            .any(|index| input.face_indexes.contains(index))
    {
        return Ok(None);
    }
    let left = input.world_axis.map(point3_array);
    let right = left;
    let local_y = Point3::new(0.0, 1.0, 0.0)
        .map_err(|_| SingleHingeThicknessBoundaryErrorV1::InconsistentPose)?;
    let normals = input.world_transforms.map(|transform| {
        transform
            .apply_vector(local_y)
            .map(point3_array)
            .map_err(|_| SingleHingeThicknessBoundaryErrorV1::InconsistentPose)
    });
    let normals = [normals[0]?, normals[1]?];
    let half = paper_thickness_mm / 2.0;
    let offset = |points: [[f64; 3]; 2], normal: [f64; 3], sign: f64| {
        points.map(|point| {
            std::array::from_fn(|axis| {
                let value = point[axis] + sign * half * normal[axis];
                if value == 0.0 { 0.0 } else { value }
            })
        })
    };
    let occurrence = input.boundary_edges[0]
        .iter()
        .position(|candidate| *candidate == edge)
        .ok_or(SingleHingeThicknessBoundaryErrorV1::InconsistentPose)?;
    let endpoint_vertices = [
        input.boundaries[0][occurrence],
        input.boundaries[0][(occurrence + 1) % 3],
    ];
    Ok(Some(NativeSingleHingeThicknessBoundaryV1 {
        bound,
        paper_thickness_bits: paper_thickness_mm.to_bits(),
        hinge: edge,
        left_face: input.faces[0],
        right_face: input.faces[1],
        endpoint_vertices,
        left_front: offset(left, normals[0], 1.0),
        left_back: offset(left, normals[0], -1.0),
        right_front: offset(right, normals[1], 1.0),
        right_back: offset(right, normals[1], -1.0),
    }))
}

#[derive(Debug)]
pub struct NativeTreeHingeThicknessBoundariesV1<'a> {
    bound: BoundMaterialTreePose<'a>,
    paper_thickness_bits: u64,
    hinges: Vec<NativeSingleHingeThicknessBoundaryV1<'a>>,
}

impl NativeTreeHingeThicknessBoundariesV1<'_> {
    #[must_use]
    pub fn hinge_count(&self) -> usize {
        self.hinges.len()
    }
}

pub fn prepare_tree_hinge_thickness_boundaries_v1(
    bound: BoundMaterialTreePose<'_>,
    paper_thickness_mm: f64,
) -> Result<Option<NativeTreeHingeThicknessBoundariesV1<'_>>, SingleHingeThicknessBoundaryErrorV1> {
    prepare_tree_hinge_thickness_boundaries_internal_v1(bound, paper_thickness_mm, true)
}

pub(crate) fn prepare_swept_tree_hinge_thickness_boundaries_v1(
    bound: BoundMaterialTreePose<'_>,
    paper_thickness_mm: f64,
) -> Result<Option<NativeTreeHingeThicknessBoundariesV1<'_>>, SingleHingeThicknessBoundaryErrorV1> {
    prepare_tree_hinge_thickness_boundaries_internal_v1(bound, paper_thickness_mm, false)
}

fn prepare_tree_hinge_thickness_boundaries_internal_v1(
    bound: BoundMaterialTreePose<'_>,
    paper_thickness_mm: f64,
    reject_nonjunction_rail_overlap: bool,
) -> Result<Option<NativeTreeHingeThicknessBoundariesV1<'_>>, SingleHingeThicknessBoundaryErrorV1> {
    if !positive_finite_binary64(paper_thickness_mm)
        || bound.model().hinges().len() < 2
        || bound.model().hinges().len() > MAX_COMPOSED_THICKNESS_HINGES_V1
    {
        return Ok(None);
    }
    let mut hinges = Vec::with_capacity(bound.model().hinges().len());
    for source_hinge in bound.model().hinges() {
        let capability = prepare_projected_incident_hinge_boundary_v1(
            bound,
            paper_thickness_mm,
            source_hinge.edge(),
        )?;
        let Some(capability) = capability else {
            return Ok(None);
        };
        hinges.push(capability);
    }
    for first in 0..hinges.len() {
        for second in first + 1..hinges.len() {
            let shared_junction = hinges[first].endpoint_vertices.iter().any(|vertex| {
                hinges[second]
                    .endpoint_vertices
                    .iter()
                    .any(|candidate| candidate == vertex)
            });
            if reject_nonjunction_rail_overlap
                && thickness_boundary_rails_overlap(
                    hinges[first].observation(),
                    hinges[second].observation(),
                )
                && !shared_junction
            {
                return Ok(None);
            }
        }
    }
    Ok(Some(NativeTreeHingeThicknessBoundariesV1 {
        bound,
        paper_thickness_bits: paper_thickness_mm.to_bits(),
        hinges,
    }))
}

pub fn revalidate_tree_hinge_thickness_boundaries_v1(
    capability: &NativeTreeHingeThicknessBoundariesV1<'_>,
    bound: BoundMaterialTreePose<'_>,
    paper_thickness_mm: f64,
) -> Option<Vec<SingleHingeThicknessBoundaryObservationV1>> {
    if capability.paper_thickness_bits != paper_thickness_mm.to_bits()
        || !std::ptr::eq(capability.bound.model(), bound.model())
        || !std::ptr::eq(capability.bound.pose(), bound.pose())
    {
        return None;
    }
    capability
        .hinges
        .iter()
        .map(|hinge| {
            revalidate_single_hinge_thickness_boundary_v1(hinge, bound, paper_thickness_mm)
        })
        .collect()
}

fn thickness_boundary_rails_overlap(
    first: SingleHingeThicknessBoundaryObservationV1,
    second: SingleHingeThicknessBoundaryObservationV1,
) -> bool {
    let first_points = [
        first.left_front,
        first.left_back,
        first.right_front,
        first.right_back,
    ]
    .concat();
    let second_points = [
        second.left_front,
        second.left_back,
        second.right_front,
        second.right_back,
    ]
    .concat();
    let boxes_overlap = (0..3).all(|axis| {
        let first_min = first_points
            .iter()
            .map(|point| point[axis])
            .fold(f64::INFINITY, f64::min);
        let first_max = first_points
            .iter()
            .map(|point| point[axis])
            .fold(f64::NEG_INFINITY, f64::max);
        let second_min = second_points
            .iter()
            .map(|point| point[axis])
            .fold(f64::INFINITY, f64::min);
        let second_max = second_points
            .iter()
            .map(|point| point[axis])
            .fold(f64::NEG_INFINITY, f64::max);
        first_min <= second_max && second_min <= first_max
    });
    if !boxes_overlap {
        return false;
    }
    let shares_material_vertex = first
        .endpoint_vertices
        .iter()
        .any(|vertex| second.endpoint_vertices.contains(vertex));
    !shares_material_vertex
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
    use super::exact_prism::{ExactPrismLimits, ExactPrismWork};
    use super::shared_hinge_corridor_admission::{
        SharedHingeCorridorAdmissionErrorV1, SharedHingeCorridorAdmissionLimitsV1,
        SharedHingeCorridorAdmissionResultV1, SharedHingeCorridorAdmissionWorkV1,
        analyze_shared_hinge_corridor_admission_v1, revalidate_shared_hinge_corridor_admission_v1,
    };
    use super::shared_hinge_solid_classification::{
        SharedHingeCorridorReconciliationV1, SharedHingeEvidenceUnavailableReasonV1,
        SharedHingePositiveThicknessPairClassV1, SharedHingeSolidClassificationErrorV1,
        SharedHingeSolidClassificationLimitsV1, SharedHingeSolidClassificationResultV1,
        SharedHingeSolidClassificationWorkV1, SharedHingeSolidIntersectionDimensionV1,
        analyze_shared_hinge_solid_classification_v1, classify_independent_exact_fixture_for_test,
        policy_contract_for_test, revalidate_shared_hinge_solid_classification_v1,
    };
    use super::shared_hinge_topology_margin::{
        SharedHingeNativeExactTopologyMarginAnalysisV1,
        SharedHingeNativeExactTopologyMarginCapabilityV1,
        SharedHingeNativeExactTopologyMarginErrorV1, SharedHingeNativeExactTopologyMarginLimitsV1,
        SharedHingeNativeExactTopologyMarginResultV1, SharedHingeNativeExactTopologyMarginWorkV1,
        analyze_shared_hinge_native_exact_topology_margin_v1,
        revalidate_shared_hinge_native_exact_topology_margin_v1,
    };
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
        two_triangle_model_with_all_options(
            assignment,
            reordered_sources,
            false,
            coordinates,
            namespace_index,
        )
    }

    fn two_triangle_model_with_all_options(
        assignment: EdgeKind,
        reordered_sources: bool,
        reversed_hinge_endpoints: bool,
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
        let (hinge_start, hinge_end) = if reversed_hinge_endpoints {
            (boundary[2], boundary[0])
        } else {
            (boundary[0], boundary[2])
        };
        edges.push(triangular_edge(5, hinge_start, hinge_end, assignment));
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

    fn three_triangle_chain_model(namespace_index: u64) -> MaterialTreeKinematicsModel {
        let coordinates = [
            (0.0, 0.0),
            (300.0, 0.0),
            (450.0, 200.0),
            (250.0, 450.0),
            (0.0, 300.0),
        ];
        let vertices = coordinates
            .into_iter()
            .enumerate()
            .map(|(index, (x, y))| triangular_vertex(index as u64 + 101, x, y))
            .collect::<Vec<_>>();
        let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
        let mut edges = (0..5)
            .map(|index| {
                triangular_edge(
                    index as u64 + 101,
                    boundary[index],
                    boundary[(index + 1) % 5],
                    EdgeKind::Boundary,
                )
            })
            .collect::<Vec<_>>();
        edges.push(triangular_edge(
            106,
            boundary[0],
            boundary[2],
            EdgeKind::Mountain,
        ));
        edges.push(triangular_edge(
            107,
            boundary[0],
            boundary[3],
            EdgeKind::Valley,
        ));
        let pattern = CreasePattern { vertices, edges };
        let paper = Paper {
            boundary_vertices: boundary,
            ..Paper::default()
        };
        let report = analyze_faces(FaceExtractionInput {
            identity_namespace: triangular_project_id(namespace_index),
            source_revision: 900 + namespace_index,
            paper: &paper,
            pattern: &pattern,
        });
        assert!(report.issues.is_empty(), "{:?}", report.issues);
        MaterialTreeKinematicsModel::prepare(
            &pattern,
            &paper,
            &report.snapshot.expect("three triangular faces"),
            TreeKinematicsLimits::default(),
        )
        .expect("three-triangle chain")
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

    #[test]
    fn public_thickness_boundary_is_issuer_pose_and_thickness_bound() {
        let square = [(0.0, 0.0), (400.0, 0.0), (400.0, 400.0), (0.0, 400.0)];
        let model = two_triangle_model_with_options(EdgeKind::Mountain, false, square, 9_001);
        let admitted_angle = [10.0, 30.0, 45.0, 60.0, 120.0, 150.0, 179.0]
            .into_iter()
            .find(|angle| {
                let candidate = triangular_pose(&model, *angle);
                prepare_single_hinge_thickness_boundary_v1(
                    model.bind_pose(&candidate).unwrap(),
                    0.1,
                )
                .unwrap()
                .is_some()
            })
            .expect("one admitted boundary angle");
        let pose = triangular_pose(&model, admitted_angle);
        let bound = model.bind_pose(&pose).unwrap();
        let capability = prepare_single_hinge_thickness_boundary_v1(bound, 0.1)
            .expect("bounded analysis")
            .expect("admitted boundary");
        let observation = capability.observation();
        assert_eq!(observation.hinge, model.hinges()[0].edge());
        assert!(
            observation
                .left_front
                .iter()
                .flatten()
                .chain(observation.right_back.iter().flatten())
                .all(|value| value.is_finite())
        );
        assert_eq!(
            revalidate_single_hinge_thickness_boundary_v1(&capability, bound, 0.1),
            Some(observation)
        );
        assert!(revalidate_single_hinge_thickness_boundary_v1(&capability, bound, 0.2).is_none());

        let same_angle_aba = triangular_pose(&model, admitted_angle);
        let aba_bound = model.bind_pose(&same_angle_aba).unwrap();
        assert!(
            revalidate_single_hinge_thickness_boundary_v1(&capability, aba_bound, 0.1).is_none()
        );
        let foreign = two_triangle_model_with_options(EdgeKind::Mountain, false, square, 9_002);
        let foreign_pose = triangular_pose(&foreign, admitted_angle);
        assert!(
            revalidate_single_hinge_thickness_boundary_v1(
                &capability,
                foreign.bind_pose(&foreign_pose).unwrap(),
                0.1,
            )
            .is_none()
        );
    }

    #[test]
    fn public_thickness_boundary_holds_exact_angle_boundaries_closed() {
        let square = [(0.0, 0.0), (400.0, 0.0), (400.0, 400.0), (0.0, 400.0)];
        let model = two_triangle_model_with_options(EdgeKind::Valley, false, square, 9_003);
        for angle in [90.0, 180.0] {
            let pose = triangular_pose(&model, angle);
            let result =
                prepare_single_hinge_thickness_boundary_v1(model.bind_pose(&pose).unwrap(), 0.1)
                    .expect("bounded boundary hold");
            assert!(result.is_none(), "{angle}");
        }
        let planar = triangular_pose(&model, 0.0);
        assert!(
            prepare_single_hinge_thickness_boundary_v1(model.bind_pose(&planar).unwrap(), 0.1,)
                .expect("planar boundary")
                .is_some()
        );
        let pose = triangular_pose(&model, 0.0);
        for thickness in [0.0, -0.0, -0.1, f64::INFINITY, f64::NAN] {
            assert!(
                prepare_single_hinge_thickness_boundary_v1(
                    model.bind_pose(&pose).unwrap(),
                    thickness,
                )
                .expect("invalid input remains bounded")
                .is_none()
            );
        }
    }

    #[test]
    fn two_hinge_tree_composes_only_authenticated_shared_endpoints() {
        let model = three_triangle_chain_model(9_004);
        let angle = [10.0, 30.0, 45.0, 60.0, 120.0, 150.0, 179.0]
            .into_iter()
            .find(|angle| {
                let pose = uniform_pose(&model, *angle);
                let bound = model.bind_pose(&pose).unwrap();
                prepare_tree_hinge_thickness_boundaries_v1(bound, 0.1)
                    .is_ok_and(|capability| capability.is_some())
            })
            .expect("one bounded two-hinge composition");
        let pose = uniform_pose(&model, angle);
        let bound = model.bind_pose(&pose).unwrap();
        let capability = prepare_tree_hinge_thickness_boundaries_v1(bound, 0.1)
            .unwrap()
            .expect("repeatable composition");
        assert_eq!(capability.hinge_count(), 2);
        let observations = revalidate_tree_hinge_thickness_boundaries_v1(&capability, bound, 0.1)
            .expect("same tree pose and thickness");
        assert_eq!(observations.len(), 2);
        assert!(
            observations[0]
                .endpoint_vertices
                .iter()
                .any(|vertex| observations[1].endpoint_vertices.contains(vertex))
        );
        assert!(revalidate_tree_hinge_thickness_boundaries_v1(&capability, bound, 0.2).is_none());
        let aba = uniform_pose(&model, pose.hinge_angles()[0].angle_degrees());
        assert!(
            revalidate_tree_hinge_thickness_boundaries_v1(
                &capability,
                model.bind_pose(&aba).unwrap(),
                0.1,
            )
            .is_none()
        );
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
        assert!(revalidate_single_triangular_hinge_prerequisites_v1(
            &swapped_face_indexes,
            &exact,
            0.1,
        )
        .is_none());
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
        let hinge = &mut wrong_parent_child.hinges[0];
        std::mem::swap(&mut hinge.parent, &mut hinge.child);

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

    fn topology_margin_capability<'a, 'prerequisite, 'ef, 'exact, 'pose>(
        analysis: &'a SharedHingeNativeExactTopologyMarginAnalysisV1<
            'prerequisite,
            'ef,
            'exact,
            'pose,
        >,
    ) -> &'a SharedHingeNativeExactTopologyMarginCapabilityV1<'prerequisite, 'ef, 'exact, 'pose>
    {
        analysis
            .authenticated_capability_and_work()
            .expect("native exact topology margin")
            .0
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
        std::thread::scope(|scope| {
            let workers = models
                .iter()
                .map(|(fixture, model)| {
                    scope.spawn(move || {
                        for root in model.face_ids() {
                            for thickness in [0.1, 1.0, 3.0] {
                                for angle in [0.0, 10.0, 90.0, 135.0, 179.0] {
                                    let pose = triangular_pose_with_root(model, angle, *root);
                                    let bound = model.bind_pose(&pose).unwrap();
                                    let exact = triangular_exact_pose(model, &pose);
                                    let prerequisite_analysis =
                                        authenticated_ef_prerequisite(&exact, thickness);
                                    let SingleTriangularHingePrerequisiteResult::Authenticated(
                                        prerequisite,
                                    ) = &prerequisite_analysis.result
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
                    })
                })
                .collect::<Vec<_>>();
            for worker in workers {
                worker.join().expect("exact-E matrix worker");
            }
        });
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
        // Each model is immutable and every certificate is issuer-bound to that
        // model.  Dispatch the four independent fixture matrices together; this
        // preserves all 120 proof/revalidation calls without serially repeating
        // the expensive exact prism scan.
        let totals = std::thread::scope(|scope| {
            models
                .iter()
                .map(|(fixture, model)| {
                    scope.spawn(move || {
                        let mut cases = 0_usize;
                        let mut contained_by_angle = [0_usize; 5];
                        let mut outside_by_angle = [0_usize; 5];
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
                                if *fixture == "mountain/source"
                                    && root == &model.face_ids()[0]
                                    && thickness == 0.1
                                    && angle == 90.0
                                {
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
                        (cases, contained_by_angle, outside_by_angle)
                    })
                })
                .collect::<Vec<_>>()
                .into_iter()
                .map(|worker| worker.join().expect("direct-F matrix worker"))
                .collect::<Vec<_>>()
        });
        let mut cases = 0_usize;
        let mut contained_by_angle = [0_usize; 5];
        let mut outside_by_angle = [0_usize; 5];
        for (worker_cases, worker_contained, worker_outside) in totals {
            cases += worker_cases;
            for angle_index in 0..5 {
                contained_by_angle[angle_index] += worker_contained[angle_index];
                outside_by_angle[angle_index] += worker_outside[angle_index];
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
        let totals = std::thread::scope(|scope| {
            models
                .iter()
                .map(|(fixture, model)| {
                    scope.spawn(move || {
                        let mut cases = 0_usize;
                        let mut contained_unadmitted_by_angle = [0_usize; 5];
                        let mut endpoint_mismatches_by_angle = [0_usize; 5];
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
                        (
                            cases,
                            contained_unadmitted_by_angle,
                            endpoint_mismatches_by_angle,
                        )
                    })
                })
                .collect::<Vec<_>>()
                .into_iter()
                .map(|worker| worker.join().expect("direct-F affine matrix worker"))
                .collect::<Vec<_>>()
        });
        let mut cases = 0_usize;
        let mut contained_unadmitted_by_angle = [0_usize; 5];
        let mut endpoint_mismatches_by_angle = [0_usize; 5];
        for (worker_cases, worker_contained, worker_mismatches) in totals {
            cases += worker_cases;
            for angle_index in 0..5 {
                contained_unadmitted_by_angle[angle_index] += worker_contained[angle_index];
                endpoint_mismatches_by_angle[angle_index] += worker_mismatches[angle_index];
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

    #[test]
    fn shared_hinge_admission_400mm_matrix_requires_one_identical_corridor() {
        let square = [(0.0, 0.0), (400.0, 0.0), (400.0, 400.0), (0.0, 400.0)];
        let models = [
            (
                "mountain/source",
                two_triangle_model_with_options(EdgeKind::Mountain, false, square, 701),
            ),
            (
                "mountain/reordered",
                two_triangle_model_with_options(EdgeKind::Mountain, true, square, 702),
            ),
            (
                "valley/source",
                two_triangle_model_with_options(EdgeKind::Valley, false, square, 703),
            ),
            (
                "valley/reordered",
                two_triangle_model_with_options(EdgeKind::Valley, true, square, 704),
            ),
        ];
        let totals = std::thread::scope(|scope| {
            models
                .iter()
                .map(|(fixture, model)| scope.spawn(move || {
                    let mut cases = 0_usize;
                    let mut admitted_by_angle = [0_usize; 5];
                    let mut boundary_mismatch_by_angle = [0_usize; 5];
                    let mut unresolved_by_angle = [0_usize; 5];
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
                        let direct_f_analysis = analyze_direct_f_finite_hinge_corridor_v1(
                            &prerequisite_analysis,
                            Some(ef),
                            &exact_e_analysis,
                            &exact,
                            bound,
                            thickness,
                            DirectFFiniteHingeCorridorLimits::default(),
                        )
                        .unwrap();
                        let analysis = analyze_shared_hinge_corridor_admission_v1(
                            &prerequisite_analysis,
                            Some(ef),
                            &exact_e_analysis,
                            &direct_f_analysis,
                            &exact,
                            bound,
                            thickness,
                            SharedHingeCorridorAdmissionLimitsV1::default(),
                        )
                        .unwrap_or_else(|error| {
                            panic!("{fixture}, root {root:?}, {thickness} mm at {angle}: {error:?}")
                        });
                        match &analysis.result {
                            SharedHingeCorridorAdmissionResultV1::Admitted(capability) => {
                                admitted_by_angle[angle_index] += 1;
                                assert_eq!(
                                    angle, 0.0,
                                    "only bit-identical E/F corridor boundaries are admitted"
                                );
                                assert_eq!(analysis.work.boundary_scalar_comparisons, 10);
                                let exact_e = exact_e_corridor_capability(&exact_e_analysis);
                                let direct_f = direct_f_corridor_capability(&direct_f_analysis);
                                let rebound = revalidate_shared_hinge_corridor_admission_v1(
                                    capability,
                                    prerequisite,
                                    ef,
                                    exact_e,
                                    direct_f,
                                    &exact,
                                    bound,
                                    thickness,
                                )
                                .expect("the same complete authority stack must revalidate");
                                assert!(std::ptr::eq(rebound.capability, capability.as_ref()));
                            }
                            SharedHingeCorridorAdmissionResultV1::BoundaryMismatch(mismatch) => {
                                boundary_mismatch_by_angle[angle_index] += 1;
                                assert!(
                                    matches!(angle, 10.0 | 135.0 | 179.0),
                                    "{fixture}, root {root:?}, {thickness} mm at {angle}: {mismatch:?}"
                                );
                                assert!(mismatch.mismatch_count > 0);
                                assert_eq!(analysis.work.boundary_scalar_comparisons, 10);
                            }
                            SharedHingeCorridorAdmissionResultV1::Unresolved => {
                                unresolved_by_angle[angle_index] += 1;
                                assert_eq!(
                                    angle, 90.0,
                                    "literal F is strictly outside its baseline corridor only at 90 degrees"
                                );
                                assert_eq!(
                                    analysis.work,
                                    SharedHingeCorridorAdmissionWorkV1::default(),
                                    "an unavailable input capability must not begin admission work"
                                );
                            }
                            SharedHingeCorridorAdmissionResultV1::LayerOffsetUnmodeled => {
                                panic!(
                                    "{fixture}, root {root:?}, {thickness} mm at {angle}: unexpected layer offset"
                                );
                            }
                        }
                    }
                }
            }
                    (
                        cases,
                        admitted_by_angle,
                        boundary_mismatch_by_angle,
                        unresolved_by_angle,
                    )
                }))
                .collect::<Vec<_>>()
                .into_iter()
                .map(|worker| worker.join().expect("shared-hinge admission matrix worker"))
                .collect::<Vec<_>>()
        });
        let mut cases = 0_usize;
        let mut admitted_by_angle = [0_usize; 5];
        let mut boundary_mismatch_by_angle = [0_usize; 5];
        let mut unresolved_by_angle = [0_usize; 5];
        for (worker_cases, worker_admitted, worker_mismatch, worker_unresolved) in totals {
            cases += worker_cases;
            for angle_index in 0..5 {
                admitted_by_angle[angle_index] += worker_admitted[angle_index];
                boundary_mismatch_by_angle[angle_index] += worker_mismatch[angle_index];
                unresolved_by_angle[angle_index] += worker_unresolved[angle_index];
            }
        }
        assert_eq!(cases, 120);
        assert_eq!(admitted_by_angle, [24, 0, 0, 0, 0]);
        assert_eq!(boundary_mismatch_by_angle, [0, 24, 0, 24, 24]);
        assert_eq!(unresolved_by_angle, [0, 0, 24, 0, 0]);
    }

    #[test]
    fn shared_hinge_admission_propagates_all_24_flat_fold_layer_offsets() {
        let square = [(0.0, 0.0), (400.0, 0.0), (400.0, 400.0), (0.0, 400.0)];
        let models = [
            two_triangle_model_with_options(EdgeKind::Mountain, false, square, 705),
            two_triangle_model_with_options(EdgeKind::Mountain, true, square, 706),
            two_triangle_model_with_options(EdgeKind::Valley, false, square, 707),
            two_triangle_model_with_options(EdgeKind::Valley, true, square, 708),
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
                    let direct_f_analysis = analyze_direct_f_finite_hinge_corridor_v1(
                        &prerequisite_analysis,
                        None,
                        &exact_e_analysis,
                        &exact,
                        bound,
                        thickness,
                        DirectFFiniteHingeCorridorLimits::default(),
                    )
                    .unwrap();
                    let analysis = analyze_shared_hinge_corridor_admission_v1(
                        &prerequisite_analysis,
                        None,
                        &exact_e_analysis,
                        &direct_f_analysis,
                        &exact,
                        bound,
                        thickness,
                        SharedHingeCorridorAdmissionLimitsV1::default(),
                    )
                    .unwrap();
                    assert!(matches!(
                        analysis.result,
                        SharedHingeCorridorAdmissionResultV1::LayerOffsetUnmodeled
                    ));
                    assert_eq!(analysis.work, SharedHingeCorridorAdmissionWorkV1::default());
                }
            }
        }
        assert_eq!(cases, 24);
    }

    #[test]
    fn shared_hinge_admission_rejects_each_changed_corridor_boundary_scalar() {
        let model = two_triangle_model();
        let pose = triangular_pose(&model, 0.0);
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
        let mut direct_f_analysis = analyze_direct_f_finite_hinge_corridor_v1(
            &prerequisite_analysis,
            Some(ef),
            &exact_e_analysis,
            &exact,
            bound,
            0.1,
            DirectFFiniteHingeCorridorLimits::default(),
        )
        .unwrap();

        for component_index in 0..10 {
            let DirectFFiniteHingeCorridorResult::Contained(capability) =
                &mut direct_f_analysis.result
            else {
                panic!("direct-F corridor");
            };
            capability.adjust_corridor_boundary_component_for_test(component_index, 1);
            let rejected = analyze_shared_hinge_corridor_admission_v1(
                &prerequisite_analysis,
                Some(ef),
                &exact_e_analysis,
                &direct_f_analysis,
                &exact,
                bound,
                0.1,
                SharedHingeCorridorAdmissionLimitsV1::default(),
            )
            .unwrap();
            assert!(
                matches!(
                    rejected.result,
                    SharedHingeCorridorAdmissionResultV1::BoundaryMismatch(_)
                ),
                "boundary component {component_index}"
            );
            drop(rejected);
            let DirectFFiniteHingeCorridorResult::Contained(capability) =
                &mut direct_f_analysis.result
            else {
                panic!("direct-F corridor");
            };
            capability.adjust_corridor_boundary_component_for_test(component_index, -1);
        }
        assert!(
            analyze_shared_hinge_corridor_admission_v1(
                &prerequisite_analysis,
                Some(ef),
                &exact_e_analysis,
                &direct_f_analysis,
                &exact,
                bound,
                0.1,
                SharedHingeCorridorAdmissionLimitsV1::default(),
            )
            .unwrap()
            .authenticated_admission_capability_and_work()
            .is_some(),
            "all boundary mutations must be restored"
        );
    }

    #[test]
    fn shared_hinge_admission_revalidation_rejects_every_authority_swap_and_f_bit() {
        let model = two_triangle_model();
        let pose = triangular_pose(&model, 0.0);
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
        let duplicate_direct_f_analysis = analyze_direct_f_finite_hinge_corridor_v1(
            &prerequisite_analysis,
            Some(ef),
            &exact_e_analysis,
            &exact,
            bound,
            0.1,
            DirectFFiniteHingeCorridorLimits::default(),
        )
        .unwrap();
        let direct_f = direct_f_corridor_capability(&direct_f_analysis);
        let duplicate_direct_f = direct_f_corridor_capability(&duplicate_direct_f_analysis);
        let mut admission = analyze_shared_hinge_corridor_admission_v1(
            &prerequisite_analysis,
            Some(ef),
            &exact_e_analysis,
            &direct_f_analysis,
            &exact,
            bound,
            0.1,
            SharedHingeCorridorAdmissionLimitsV1::default(),
        )
        .unwrap();
        let duplicate_admission = analyze_shared_hinge_corridor_admission_v1(
            &prerequisite_analysis,
            Some(ef),
            &exact_e_analysis,
            &direct_f_analysis,
            &exact,
            bound,
            0.1,
            SharedHingeCorridorAdmissionLimitsV1::default(),
        )
        .unwrap();
        let capability = admission
            .authenticated_admission_capability_and_work()
            .expect("admission capability")
            .0;
        let duplicate_capability = duplicate_admission
            .authenticated_admission_capability_and_work()
            .expect("independently issued admission capability")
            .0;
        assert!(
            !std::ptr::eq(capability, duplicate_capability),
            "independent issuance must never alias the original token"
        );
        assert!(
            revalidate_shared_hinge_corridor_admission_v1(
                capability,
                prerequisite,
                ef,
                exact_e,
                direct_f,
                &exact,
                bound,
                0.1,
            )
            .is_some()
        );
        for (name, rejected) in [
            (
                "independently issued phase-1 token",
                revalidate_shared_hinge_corridor_admission_v1(
                    capability,
                    duplicate_prerequisite,
                    ef,
                    exact_e,
                    direct_f,
                    &exact,
                    bound,
                    0.1,
                ),
            ),
            (
                "independently issued E/F token",
                revalidate_shared_hinge_corridor_admission_v1(
                    capability,
                    prerequisite,
                    duplicate_ef,
                    exact_e,
                    direct_f,
                    &exact,
                    bound,
                    0.1,
                ),
            ),
            (
                "independently issued exact-E corridor token",
                revalidate_shared_hinge_corridor_admission_v1(
                    capability,
                    prerequisite,
                    ef,
                    duplicate_exact_e,
                    direct_f,
                    &exact,
                    bound,
                    0.1,
                ),
            ),
            (
                "independently issued direct-F corridor token",
                revalidate_shared_hinge_corridor_admission_v1(
                    capability,
                    prerequisite,
                    ef,
                    exact_e,
                    duplicate_direct_f,
                    &exact,
                    bound,
                    0.1,
                ),
            ),
            (
                "independently regenerated exact pose",
                revalidate_shared_hinge_corridor_admission_v1(
                    capability,
                    prerequisite,
                    ef,
                    exact_e,
                    direct_f,
                    &independent_exact,
                    bound,
                    0.1,
                ),
            ),
        ] {
            assert!(rejected.is_none(), "{name}");
        }

        let aba_pose = triangular_pose(&model, 0.0);
        let aba_bound = model.bind_pose(&aba_pose).unwrap();
        let rerooted_pose = triangular_pose_with_root(&model, 0.0, model.face_ids()[1]);
        let rerooted_bound = model.bind_pose(&rerooted_pose).unwrap();
        let one_ulp_pose = triangular_pose(&model, f64::from_bits(1));
        let one_ulp_bound = model.bind_pose(&one_ulp_pose).unwrap();
        for (name, mismatched_bound) in [
            ("same-angle ABA", aba_bound),
            ("different fixed root", rerooted_bound),
            ("one-ULP angle", one_ulp_bound),
        ] {
            assert!(
                revalidate_shared_hinge_corridor_admission_v1(
                    capability,
                    prerequisite,
                    ef,
                    exact_e,
                    direct_f,
                    &exact,
                    mismatched_bound,
                    0.1,
                )
                .is_none(),
                "{name}"
            );
        }
        assert!(
            revalidate_shared_hinge_corridor_admission_v1(
                capability,
                prerequisite,
                ef,
                exact_e,
                direct_f,
                &exact,
                bound,
                next_up(0.1),
            )
            .is_none(),
            "paper thickness is bit-bound"
        );
        let foreign = two_triangle_model_with_options(
            EdgeKind::Mountain,
            false,
            [(0.0, 0.0), (400.0, 0.0), (400.0, 400.0), (0.0, 400.0)],
            709,
        );
        let foreign_pose = triangular_pose(&foreign, 0.0);
        let foreign_bound = foreign.bind_pose(&foreign_pose).unwrap();
        assert!(
            revalidate_shared_hinge_corridor_admission_v1(
                capability,
                prerequisite,
                ef,
                exact_e,
                direct_f,
                &exact,
                foreign_bound,
                0.1,
            )
            .is_none(),
            "foreign model and pose issuer"
        );

        // Release the immutable capability borrow before modifying only the
        // admission token's sealed copies. The upstream direct-F token stays
        // untouched throughout this exhaustive bit-binding check.
        let _ = capability;
        let mut changed_coefficients = 0_usize;
        for face in 0..2 {
            for row in 0..3 {
                for column in 0..3 {
                    let SharedHingeCorridorAdmissionResultV1::Admitted(capability) =
                        &mut admission.result
                    else {
                        panic!("admission capability");
                    };
                    capability.binary64_face_transforms[face].rotation[row][column] ^= 1;
                    let SharedHingeCorridorAdmissionResultV1::Admitted(capability) =
                        &admission.result
                    else {
                        panic!("admission capability");
                    };
                    assert!(
                        revalidate_shared_hinge_corridor_admission_v1(
                            capability,
                            prerequisite,
                            ef,
                            exact_e,
                            direct_f,
                            &exact,
                            bound,
                            0.1,
                        )
                        .is_none(),
                        "face {face} rotation[{row}][{column}]"
                    );
                    let SharedHingeCorridorAdmissionResultV1::Admitted(capability) =
                        &mut admission.result
                    else {
                        panic!("admission capability");
                    };
                    capability.binary64_face_transforms[face].rotation[row][column] ^= 1;
                    changed_coefficients += 1;
                }
            }
            for axis in 0..3 {
                let SharedHingeCorridorAdmissionResultV1::Admitted(capability) =
                    &mut admission.result
                else {
                    panic!("admission capability");
                };
                capability.binary64_face_transforms[face].translation[axis] ^= 1;
                let SharedHingeCorridorAdmissionResultV1::Admitted(capability) = &admission.result
                else {
                    panic!("admission capability");
                };
                assert!(
                    revalidate_shared_hinge_corridor_admission_v1(
                        capability,
                        prerequisite,
                        ef,
                        exact_e,
                        direct_f,
                        &exact,
                        bound,
                        0.1,
                    )
                    .is_none(),
                    "face {face} translation[{axis}]"
                );
                let SharedHingeCorridorAdmissionResultV1::Admitted(capability) =
                    &mut admission.result
                else {
                    panic!("admission capability");
                };
                capability.binary64_face_transforms[face].translation[axis] ^= 1;
                changed_coefficients += 1;
            }
        }
        for row in 0..3 {
            for column in 0..3 {
                let SharedHingeCorridorAdmissionResultV1::Admitted(capability) =
                    &mut admission.result
                else {
                    panic!("admission capability");
                };
                capability.hinge_parent_transform.rotation[row][column] ^= 1;
                let SharedHingeCorridorAdmissionResultV1::Admitted(capability) = &admission.result
                else {
                    panic!("admission capability");
                };
                assert!(
                    revalidate_shared_hinge_corridor_admission_v1(
                        capability,
                        prerequisite,
                        ef,
                        exact_e,
                        direct_f,
                        &exact,
                        bound,
                        0.1,
                    )
                    .is_none(),
                    "hinge-parent rotation[{row}][{column}]"
                );
                let SharedHingeCorridorAdmissionResultV1::Admitted(capability) =
                    &mut admission.result
                else {
                    panic!("admission capability");
                };
                capability.hinge_parent_transform.rotation[row][column] ^= 1;
                changed_coefficients += 1;
            }
        }
        for axis in 0..3 {
            let SharedHingeCorridorAdmissionResultV1::Admitted(capability) = &mut admission.result
            else {
                panic!("admission capability");
            };
            capability.hinge_parent_transform.translation[axis] ^= 1;
            let SharedHingeCorridorAdmissionResultV1::Admitted(capability) = &admission.result
            else {
                panic!("admission capability");
            };
            assert!(
                revalidate_shared_hinge_corridor_admission_v1(
                    capability,
                    prerequisite,
                    ef,
                    exact_e,
                    direct_f,
                    &exact,
                    bound,
                    0.1,
                )
                .is_none(),
                "hinge-parent translation[{axis}]"
            );
            let SharedHingeCorridorAdmissionResultV1::Admitted(capability) = &mut admission.result
            else {
                panic!("admission capability");
            };
            capability.hinge_parent_transform.translation[axis] ^= 1;
            changed_coefficients += 1;
        }
        assert_eq!(changed_coefficients, 36);
        let SharedHingeCorridorAdmissionResultV1::Admitted(capability) = &admission.result else {
            panic!("admission capability");
        };
        assert!(
            revalidate_shared_hinge_corridor_admission_v1(
                capability,
                prerequisite,
                ef,
                exact_e,
                direct_f,
                &exact,
                bound,
                0.1,
            )
            .is_some(),
            "all transform-bit mutations must be restored"
        );
        let original = admission.work.boundary_scalar_comparisons;
        admission.work.boundary_scalar_comparisons = original + 1;
        assert!(
            admission
                .authenticated_admission_capability_and_work()
                .is_none(),
            "analysis work must equal the capability's sealed work"
        );
        admission.work.boundary_scalar_comparisons = original;
        assert!(
            admission
                .authenticated_admission_capability_and_work()
                .is_some()
        );
    }

    fn shared_hinge_admission_limits_from_work(
        work: &SharedHingeCorridorAdmissionWorkV1,
    ) -> SharedHingeCorridorAdmissionLimitsV1 {
        SharedHingeCorridorAdmissionLimitsV1 {
            max_authenticated_faces: work.authenticated_faces,
            max_authenticated_hinges: work.authenticated_hinges,
            max_corridor_capability_revalidations: work.corridor_capability_revalidations,
            max_sealed_prior_work_bindings: work.sealed_prior_work_bindings,
            max_root_bindings: work.root_bindings,
            max_angle_bindings: work.angle_bindings,
            max_face_identity_bindings: work.face_identity_bindings,
            max_hinge_identity_bindings: work.hinge_identity_bindings,
            max_interaction_kind_bindings: work.interaction_kind_bindings,
            max_face_transform_bit_bindings: work.face_transform_bit_bindings,
            max_hinge_parent_transform_bit_bindings: work.hinge_parent_transform_bit_bindings,
            max_boundary_scalar_comparisons: work.boundary_scalar_comparisons,
            exact: cayley_limits_from_observed_work(&work.exact),
        }
    }

    #[test]
    fn shared_hinge_admission_all_structural_and_exact_limits_are_exact_and_one_short() {
        let model = two_triangle_model();
        let pose = triangular_pose(&model, 0.0);
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
        let baseline = analyze_shared_hinge_corridor_admission_v1(
            &prerequisite_analysis,
            Some(ef),
            &exact_e_analysis,
            &direct_f_analysis,
            &exact,
            bound,
            0.1,
            SharedHingeCorridorAdmissionLimitsV1::default(),
        )
        .unwrap();
        assert!(
            baseline
                .authenticated_admission_capability_and_work()
                .is_some()
        );
        let exact_limits = shared_hinge_admission_limits_from_work(&baseline.work);
        let exact_analysis = analyze_shared_hinge_corridor_admission_v1(
            &prerequisite_analysis,
            Some(ef),
            &exact_e_analysis,
            &direct_f_analysis,
            &exact,
            bound,
            0.1,
            exact_limits,
        )
        .unwrap();
        assert!(
            exact_analysis
                .authenticated_admission_capability_and_work()
                .is_some()
        );
        assert_eq!(exact_analysis.work, baseline.work);

        let assert_one_short = |resource: &str, limits: SharedHingeCorridorAdmissionLimitsV1| {
            assert!(
                matches!(
                    analyze_shared_hinge_corridor_admission_v1(
                        &prerequisite_analysis,
                        Some(ef),
                        &exact_e_analysis,
                        &direct_f_analysis,
                        &exact,
                        bound,
                        0.1,
                        limits,
                    ),
                    Err(SharedHingeCorridorAdmissionErrorV1::ResourceLimitExceeded)
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
        structural_one_short!(max_corridor_capability_revalidations);
        structural_one_short!(max_sealed_prior_work_bindings);
        structural_one_short!(max_root_bindings);
        structural_one_short!(max_angle_bindings);
        structural_one_short!(max_face_identity_bindings);
        structural_one_short!(max_hinge_identity_bindings);
        structural_one_short!(max_interaction_kind_bindings);
        structural_one_short!(max_face_transform_bit_bindings);
        structural_one_short!(max_hinge_parent_transform_bit_bindings);
        structural_one_short!(max_boundary_scalar_comparisons);

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

        let oversized = SharedHingeCorridorAdmissionLimitsV1 {
            max_authenticated_faces: usize::MAX,
            max_authenticated_hinges: usize::MAX,
            max_corridor_capability_revalidations: usize::MAX,
            max_sealed_prior_work_bindings: usize::MAX,
            max_root_bindings: usize::MAX,
            max_angle_bindings: usize::MAX,
            max_face_identity_bindings: usize::MAX,
            max_hinge_identity_bindings: usize::MAX,
            max_interaction_kind_bindings: usize::MAX,
            max_face_transform_bit_bindings: usize::MAX,
            max_hinge_parent_transform_bit_bindings: usize::MAX,
            max_boundary_scalar_comparisons: usize::MAX,
            exact: oversized_cayley_limits(),
        };
        let projected = analyze_shared_hinge_corridor_admission_v1(
            &prerequisite_analysis,
            Some(ef),
            &exact_e_analysis,
            &direct_f_analysis,
            &exact,
            bound,
            0.1,
            oversized,
        )
        .unwrap();
        assert!(
            projected
                .authenticated_admission_capability_and_work()
                .is_some()
        );
        assert_eq!(projected.work, baseline.work);
    }

    #[test]
    fn native_exact_topology_margin_400mm_matrix_covers_all_orders_roots_and_angles() {
        let square = [(0.0, 0.0), (400.0, 0.0), (400.0, 400.0), (0.0, 400.0)];
        let models = [
            (
                "mountain/source",
                two_triangle_model_with_options(EdgeKind::Mountain, false, square, 801),
            ),
            (
                "mountain/reordered",
                two_triangle_model_with_options(EdgeKind::Mountain, true, square, 802),
            ),
            (
                "valley/source",
                two_triangle_model_with_options(EdgeKind::Valley, false, square, 803),
            ),
            (
                "valley/reordered",
                two_triangle_model_with_options(EdgeKind::Valley, true, square, 804),
            ),
            (
                "mountain/reversed-endpoints",
                two_triangle_model_with_all_options(EdgeKind::Mountain, false, true, square, 811),
            ),
            (
                "mountain/reordered/reversed-endpoints",
                two_triangle_model_with_all_options(EdgeKind::Mountain, true, true, square, 812),
            ),
            (
                "valley/reversed-endpoints",
                two_triangle_model_with_all_options(EdgeKind::Valley, false, true, square, 813),
            ),
            (
                "valley/reordered/reversed-endpoints",
                two_triangle_model_with_all_options(EdgeKind::Valley, true, true, square, 814),
            ),
        ];
        let mut measured = 0_usize;
        for (fixture, model) in &models {
            for root in model.face_ids() {
                for thickness in [0.1, 1.0, 3.0] {
                    for angle in [10.0, 90.0, 135.0, 179.0] {
                        let pose = triangular_pose_with_root(model, angle, *root);
                        let bound = model.bind_pose(&pose).unwrap();
                        let exact = triangular_exact_pose(model, &pose);
                        let prerequisite_analysis =
                            authenticated_ef_prerequisite(&exact, thickness);
                        let SingleTriangularHingePrerequisiteResult::Authenticated(prerequisite) =
                            &prerequisite_analysis.result
                        else {
                            panic!("{fixture}, {root:?}, {thickness}, {angle}: prerequisite");
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
                        let analysis = analyze_shared_hinge_native_exact_topology_margin_v1(
                            &prerequisite_analysis,
                            Some(ef),
                            &exact,
                            bound,
                            thickness,
                            SharedHingeNativeExactTopologyMarginLimitsV1::default(),
                        )
                        .unwrap();
                        let capability = analysis
                            .authenticated_capability_and_work()
                            .unwrap_or_else(|| {
                                panic!(
                                    "{fixture}, {root:?}, {thickness}, {angle}: {:?}",
                                    analysis.result
                                )
                            })
                            .0;
                        assert!(
                            revalidate_shared_hinge_native_exact_topology_margin_v1(
                                capability,
                                prerequisite,
                                ef,
                                &exact,
                                bound,
                                thickness,
                            )
                            .is_some()
                        );
                        assert_eq!(
                            capability.relative_margin(),
                            &fraction(1, 1_i64 << 44),
                            "binary64 epsilon × 256 is the exact version factor"
                        );
                        assert!(capability.point_margin_mm().is_positive());
                        for component in 0..10 {
                            assert!(
                                capability.corridor_component_error()[component]
                                    <= capability.corridor_component_margin()[component],
                                "{fixture}, {root:?}, {thickness}, {angle}: corridor {component}"
                            );
                        }
                        assert_eq!(
                            capability.corridor_component_margin()[7],
                            integer(0),
                            "the exact lift of one thickness is identical on E and F"
                        );
                        measured += 1;
                    }
                }
            }
        }
        assert_eq!(measured, 192);

        let mut flat = 0_usize;
        for (fixture, model) in &models {
            for root in model.face_ids() {
                for thickness in [0.1, 1.0, 3.0] {
                    let pose = triangular_pose_with_root(model, 180.0, *root);
                    let bound = model.bind_pose(&pose).unwrap();
                    let exact = triangular_exact_pose(model, &pose);
                    let prerequisite_analysis = authenticated_ef_prerequisite(&exact, thickness);
                    let analysis = analyze_shared_hinge_native_exact_topology_margin_v1(
                        &prerequisite_analysis,
                        None,
                        &exact,
                        bound,
                        thickness,
                        SharedHingeNativeExactTopologyMarginLimitsV1::default(),
                    )
                    .unwrap();
                    assert!(
                        matches!(
                            analysis.result,
                            SharedHingeNativeExactTopologyMarginResultV1::LayerOffsetUnmodeled
                        ),
                        "{fixture}, {root:?}, {thickness}"
                    );
                    assert_eq!(
                        analysis.work,
                        SharedHingeNativeExactTopologyMarginWorkV1::default()
                    );
                    flat += 1;
                }
            }
        }
        assert_eq!(flat, 48);
    }

    #[test]
    fn native_exact_topology_margin_rejects_regeneration_aba_foreign_ulp_and_all_sealed_scalars() {
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
        let mut analysis = analyze_shared_hinge_native_exact_topology_margin_v1(
            &prerequisite_analysis,
            Some(ef),
            &exact,
            bound,
            0.1,
            SharedHingeNativeExactTopologyMarginLimitsV1::default(),
        )
        .unwrap();
        let duplicate_analysis = analyze_shared_hinge_native_exact_topology_margin_v1(
            &prerequisite_analysis,
            Some(ef),
            &exact,
            bound,
            0.1,
            SharedHingeNativeExactTopologyMarginLimitsV1::default(),
        )
        .unwrap();
        let capability = topology_margin_capability(&analysis);
        let duplicate_capability = topology_margin_capability(&duplicate_analysis);
        assert!(!std::ptr::eq(capability, duplicate_capability));
        assert!(
            revalidate_shared_hinge_native_exact_topology_margin_v1(
                capability,
                prerequisite,
                ef,
                &exact,
                bound,
                0.1,
            )
            .is_some()
        );
        for (name, rejected) in [
            (
                "independently issued prerequisite",
                revalidate_shared_hinge_native_exact_topology_margin_v1(
                    capability,
                    duplicate_prerequisite,
                    ef,
                    &exact,
                    bound,
                    0.1,
                ),
            ),
            (
                "independently issued E/F boundary",
                revalidate_shared_hinge_native_exact_topology_margin_v1(
                    capability,
                    prerequisite,
                    duplicate_ef,
                    &exact,
                    bound,
                    0.1,
                ),
            ),
            (
                "independently regenerated exact pose",
                revalidate_shared_hinge_native_exact_topology_margin_v1(
                    capability,
                    prerequisite,
                    ef,
                    &independent_exact,
                    bound,
                    0.1,
                ),
            ),
        ] {
            assert!(rejected.is_none(), "{name}");
        }

        let aba_pose = triangular_pose(&model, 135.0);
        let aba_bound = model.bind_pose(&aba_pose).unwrap();
        let rerooted_pose = triangular_pose_with_root(&model, 135.0, model.face_ids()[1]);
        let rerooted_bound = model.bind_pose(&rerooted_pose).unwrap();
        let one_ulp_pose = triangular_pose(&model, next_up(135.0));
        let one_ulp_bound = model.bind_pose(&one_ulp_pose).unwrap();
        for (name, mismatched_bound) in [
            ("same-angle ABA", aba_bound),
            ("different root", rerooted_bound),
            ("one-ULP angle", one_ulp_bound),
        ] {
            assert!(
                revalidate_shared_hinge_native_exact_topology_margin_v1(
                    capability,
                    prerequisite,
                    ef,
                    &exact,
                    mismatched_bound,
                    0.1,
                )
                .is_none(),
                "{name}"
            );
        }
        assert!(
            revalidate_shared_hinge_native_exact_topology_margin_v1(
                capability,
                prerequisite,
                ef,
                &exact,
                bound,
                next_up(0.1),
            )
            .is_none()
        );
        let foreign = two_triangle_model_with_options(
            EdgeKind::Mountain,
            false,
            [(0.0, 0.0), (400.0, 0.0), (400.0, 400.0), (0.0, 400.0)],
            805,
        );
        let foreign_pose = triangular_pose(&foreign, 135.0);
        let foreign_bound = foreign.bind_pose(&foreign_pose).unwrap();
        assert!(
            revalidate_shared_hinge_native_exact_topology_margin_v1(
                capability,
                prerequisite,
                ef,
                &exact,
                foreign_bound,
                0.1,
            )
            .is_none()
        );

        let scalar_count = capability.scalar_count_for_test();
        let _ = capability;
        for scalar in 0..scalar_count {
            let SharedHingeNativeExactTopologyMarginResultV1::Measured(capability) =
                &mut analysis.result
            else {
                panic!("topology-margin capability");
            };
            capability.adjust_scalar_for_test(scalar, 1);
            let SharedHingeNativeExactTopologyMarginResultV1::Measured(capability) =
                &analysis.result
            else {
                panic!("topology-margin capability");
            };
            assert!(
                revalidate_shared_hinge_native_exact_topology_margin_v1(
                    capability,
                    prerequisite,
                    ef,
                    &exact,
                    bound,
                    0.1,
                )
                .is_none(),
                "sealed scalar {scalar}"
            );
            let SharedHingeNativeExactTopologyMarginResultV1::Measured(capability) =
                &mut analysis.result
            else {
                panic!("topology-margin capability");
            };
            capability.adjust_scalar_for_test(scalar, -1);
        }
        assert!(
            scalar_count >= 100,
            "all bound topology scalars are visited"
        );

        let mut changed_coefficients = 0_usize;
        for face in 0..2 {
            for coefficient in 0..12 {
                let SharedHingeNativeExactTopologyMarginResultV1::Measured(capability) =
                    &mut analysis.result
                else {
                    panic!("topology-margin capability");
                };
                capability.flip_face_transform_bit_for_test(face, coefficient);
                let SharedHingeNativeExactTopologyMarginResultV1::Measured(capability) =
                    &analysis.result
                else {
                    panic!("topology-margin capability");
                };
                assert!(
                    revalidate_shared_hinge_native_exact_topology_margin_v1(
                        capability,
                        prerequisite,
                        ef,
                        &exact,
                        bound,
                        0.1,
                    )
                    .is_none(),
                    "face {face}, coefficient {coefficient}"
                );
                let SharedHingeNativeExactTopologyMarginResultV1::Measured(capability) =
                    &mut analysis.result
                else {
                    panic!("topology-margin capability");
                };
                capability.flip_face_transform_bit_for_test(face, coefficient);
                changed_coefficients += 1;
            }
        }
        for coefficient in 0..12 {
            let SharedHingeNativeExactTopologyMarginResultV1::Measured(capability) =
                &mut analysis.result
            else {
                panic!("topology-margin capability");
            };
            capability.flip_hinge_parent_transform_bit_for_test(coefficient);
            let SharedHingeNativeExactTopologyMarginResultV1::Measured(capability) =
                &analysis.result
            else {
                panic!("topology-margin capability");
            };
            assert!(
                revalidate_shared_hinge_native_exact_topology_margin_v1(
                    capability,
                    prerequisite,
                    ef,
                    &exact,
                    bound,
                    0.1,
                )
                .is_none(),
                "hinge-parent coefficient {coefficient}"
            );
            let SharedHingeNativeExactTopologyMarginResultV1::Measured(capability) =
                &mut analysis.result
            else {
                panic!("topology-margin capability");
            };
            capability.flip_hinge_parent_transform_bit_for_test(coefficient);
            changed_coefficients += 1;
        }
        assert_eq!(changed_coefficients, 36);
        let restored = topology_margin_capability(&analysis);
        assert!(
            revalidate_shared_hinge_native_exact_topology_margin_v1(
                restored,
                prerequisite,
                ef,
                &exact,
                bound,
                0.1,
            )
            .is_some()
        );

        let original = analysis.work.corridor_component_scans;
        analysis.work.corridor_component_scans += 1;
        assert!(analysis.authenticated_capability_and_work().is_none());
        analysis.work.corridor_component_scans = original;
        assert!(analysis.authenticated_capability_and_work().is_some());
    }

    #[test]
    fn native_exact_topology_margin_reauthenticates_every_upstream_ef_scalar() {
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
        let scalar_count = {
            let ef_analysis = analyze_axis_aligned_ef_boundary_v1(
                prerequisite,
                &exact,
                bound,
                0.1,
                AxisAlignedEfBoundaryLimits::default(),
            )
            .unwrap();
            ef_capability(&ef_analysis).scalar_count_for_test()
        };
        assert_eq!(scalar_count, 36);
        for scalar in 0..scalar_count {
            let mut ef_analysis = analyze_axis_aligned_ef_boundary_v1(
                prerequisite,
                &exact,
                bound,
                0.1,
                AxisAlignedEfBoundaryLimits::default(),
            )
            .unwrap();
            ef_analysis
                .capability
                .as_mut()
                .expect("E/F capability")
                .adjust_scalar_for_test(scalar, 1);
            let rejected = analyze_shared_hinge_native_exact_topology_margin_v1(
                &prerequisite_analysis,
                ef_analysis.capability.as_ref(),
                &exact,
                bound,
                0.1,
                SharedHingeNativeExactTopologyMarginLimitsV1::default(),
            )
            .unwrap();
            assert!(
                matches!(
                    rejected.result,
                    SharedHingeNativeExactTopologyMarginResultV1::Unresolved
                ),
                "E/F scalar {scalar}"
            );
            assert_eq!(
                rejected.work.ef_scalar_reauthentications, 36,
                "the complete E/F scalar set is scanned before rejection"
            );
        }
    }

    fn topology_margin_limits_from_work(
        work: &SharedHingeNativeExactTopologyMarginWorkV1,
    ) -> SharedHingeNativeExactTopologyMarginLimitsV1 {
        SharedHingeNativeExactTopologyMarginLimitsV1 {
            max_authenticated_faces: work.authenticated_faces,
            max_authenticated_hinges: work.authenticated_hinges,
            max_prerequisite_revalidations: work.prerequisite_revalidations,
            max_ef_boundary_revalidations: work.ef_boundary_revalidations,
            max_root_bindings: work.root_bindings,
            max_angle_bindings: work.angle_bindings,
            max_face_identity_bindings: work.face_identity_bindings,
            max_hinge_identity_bindings: work.hinge_identity_bindings,
            max_endpoint_identity_bindings: work.endpoint_identity_bindings,
            max_source_boundary_occurrences: work.source_boundary_occurrences,
            max_source_coordinate_lifts: work.source_coordinate_lifts,
            max_transform_scalar_lifts: work.transform_scalar_lifts,
            max_face_transform_bit_bindings: work.face_transform_bit_bindings,
            max_hinge_parent_transform_bit_bindings: work.hinge_parent_transform_bit_bindings,
            max_mid_surface_reconstructions: work.mid_surface_reconstructions,
            max_normal_reconstructions: work.normal_reconstructions,
            max_solid_vertex_constructions: work.solid_vertex_constructions,
            max_topology_coordinate_tests: work.topology_coordinate_tests,
            max_shared_endpoint_component_tests: work.shared_endpoint_component_tests,
            max_point_component_error_tests: work.point_component_error_tests,
            max_normal_component_error_tests: work.normal_component_error_tests,
            max_solid_component_error_tests: work.solid_component_error_tests,
            max_ef_scalar_reauthentications: work.ef_scalar_reauthentications,
            max_local_scale_component_scans: work.local_scale_component_scans,
            max_corridor_component_scans: work.corridor_component_scans,
            exact: cayley_limits_from_observed_work(&work.exact),
        }
    }

    #[test]
    fn native_exact_topology_margin_all_structural_and_exact_limits_are_exact_and_one_short() {
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
        let baseline = analyze_shared_hinge_native_exact_topology_margin_v1(
            &prerequisite_analysis,
            Some(ef),
            &exact,
            bound,
            0.1,
            SharedHingeNativeExactTopologyMarginLimitsV1::default(),
        )
        .unwrap();
        assert!(baseline.authenticated_capability_and_work().is_some());
        let exact_limits = topology_margin_limits_from_work(&baseline.work);
        let exact_analysis = analyze_shared_hinge_native_exact_topology_margin_v1(
            &prerequisite_analysis,
            Some(ef),
            &exact,
            bound,
            0.1,
            exact_limits,
        )
        .unwrap();
        assert!(exact_analysis.authenticated_capability_and_work().is_some());
        assert_eq!(exact_analysis.work, baseline.work);

        let assert_one_short =
            |resource: &str, limits: SharedHingeNativeExactTopologyMarginLimitsV1| {
                assert!(
                    matches!(
                        analyze_shared_hinge_native_exact_topology_margin_v1(
                            &prerequisite_analysis,
                            Some(ef),
                            &exact,
                            bound,
                            0.1,
                            limits,
                        ),
                        Err(SharedHingeNativeExactTopologyMarginErrorV1::ResourceLimitExceeded)
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
        structural_one_short!(max_prerequisite_revalidations);
        structural_one_short!(max_ef_boundary_revalidations);
        structural_one_short!(max_root_bindings);
        structural_one_short!(max_angle_bindings);
        structural_one_short!(max_face_identity_bindings);
        structural_one_short!(max_hinge_identity_bindings);
        structural_one_short!(max_endpoint_identity_bindings);
        structural_one_short!(max_source_boundary_occurrences);
        structural_one_short!(max_source_coordinate_lifts);
        structural_one_short!(max_transform_scalar_lifts);
        structural_one_short!(max_face_transform_bit_bindings);
        structural_one_short!(max_hinge_parent_transform_bit_bindings);
        structural_one_short!(max_mid_surface_reconstructions);
        structural_one_short!(max_normal_reconstructions);
        structural_one_short!(max_solid_vertex_constructions);
        structural_one_short!(max_topology_coordinate_tests);
        structural_one_short!(max_shared_endpoint_component_tests);
        structural_one_short!(max_point_component_error_tests);
        structural_one_short!(max_normal_component_error_tests);
        structural_one_short!(max_solid_component_error_tests);
        structural_one_short!(max_ef_scalar_reauthentications);
        structural_one_short!(max_local_scale_component_scans);
        structural_one_short!(max_corridor_component_scans);

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

        let oversized = SharedHingeNativeExactTopologyMarginLimitsV1 {
            max_authenticated_faces: usize::MAX,
            max_authenticated_hinges: usize::MAX,
            max_prerequisite_revalidations: usize::MAX,
            max_ef_boundary_revalidations: usize::MAX,
            max_root_bindings: usize::MAX,
            max_angle_bindings: usize::MAX,
            max_face_identity_bindings: usize::MAX,
            max_hinge_identity_bindings: usize::MAX,
            max_endpoint_identity_bindings: usize::MAX,
            max_source_boundary_occurrences: usize::MAX,
            max_source_coordinate_lifts: usize::MAX,
            max_transform_scalar_lifts: usize::MAX,
            max_face_transform_bit_bindings: usize::MAX,
            max_hinge_parent_transform_bit_bindings: usize::MAX,
            max_mid_surface_reconstructions: usize::MAX,
            max_normal_reconstructions: usize::MAX,
            max_solid_vertex_constructions: usize::MAX,
            max_topology_coordinate_tests: usize::MAX,
            max_shared_endpoint_component_tests: usize::MAX,
            max_point_component_error_tests: usize::MAX,
            max_normal_component_error_tests: usize::MAX,
            max_solid_component_error_tests: usize::MAX,
            max_ef_scalar_reauthentications: usize::MAX,
            max_local_scale_component_scans: usize::MAX,
            max_corridor_component_scans: usize::MAX,
            exact: oversized_cayley_limits(),
        };
        let projected = analyze_shared_hinge_native_exact_topology_margin_v1(
            &prerequisite_analysis,
            Some(ef),
            &exact,
            bound,
            0.1,
            oversized,
        )
        .unwrap();
        assert!(projected.authenticated_capability_and_work().is_some());
        assert_eq!(projected.work, baseline.work);
    }

    fn exact_limits_for_two_works(first: &CayleyWork, second: &CayleyWork) -> CayleyLimits {
        CayleyLimits {
            max_precision_rounds: 0,
            max_guard_bits: 0,
            max_candidate_bits: 0,
            max_machin_terms_per_series: 0,
            max_trig_terms_per_series: 0,
            max_sqrt_refinements: 0,
            max_interval_operations: first.interval_operations.max(second.interval_operations),
            max_shift_bits: first.max_shift_bits.max(second.max_shift_bits),
            max_intermediate_bits: first
                .max_preflight_bits
                .max(first.max_observed_bits)
                .max(second.max_preflight_bits)
                .max(second.max_observed_bits),
            max_gcd_fallback_calls: first.gcd_fallback_calls.max(second.gcd_fallback_calls),
            max_gcd_fallback_input_bits: first
                .gcd_fallback_input_bits
                .max(second.gcd_fallback_input_bits),
            max_rational_allocations: first.rational_allocations.max(second.rational_allocations),
            max_rational_allocation_bits: first
                .max_rational_allocation_bits
                .max(second.max_rational_allocation_bits),
            max_total_rational_allocation_bits: first
                .total_rational_allocation_bits
                .max(second.total_rational_allocation_bits),
            max_output_bits: 0,
        }
    }

    fn prism_limits_for_two_works(
        first: &ExactPrismWork,
        second: &ExactPrismWork,
    ) -> ExactPrismLimits {
        ExactPrismLimits {
            max_prisms: first.prisms.max(second.prisms),
            max_solid_vertices: first.solid_vertices.max(second.solid_vertices),
            max_facets: first.facets.max(second.facets),
            max_halfspaces: first.halfspaces.max(second.halfspaces),
            max_prism_volume_tests: first.prism_volume_tests.max(second.prism_volume_tests),
            max_facet_vertex_checks: first.facet_vertex_checks.max(second.facet_vertex_checks),
            max_plane_triples: first.plane_triples.max(second.plane_triples),
            max_singular_plane_triples: first
                .singular_plane_triples
                .max(second.singular_plane_triples),
            max_nonsingular_solves: first.nonsingular_solves.max(second.nonsingular_solves),
            max_membership_tests: first.membership_tests.max(second.membership_tests),
            max_candidate_vertices: first.candidate_vertices.max(second.candidate_vertices),
            max_dedup_comparisons: first.dedup_comparisons.max(second.dedup_comparisons),
            max_affine_rank_tests: first.affine_rank_tests.max(second.affine_rank_tests),
            max_support_plane_vertex_tests: first
                .support_plane_vertex_tests
                .max(second.support_plane_vertex_tests),
            max_support_pair_tests: first.support_pair_tests.max(second.support_pair_tests),
            max_input_rationals: first.input_rationals.max(second.input_rationals),
            max_input_rational_storage_bits: first
                .max_input_rational_storage_bits
                .max(second.max_input_rational_storage_bits),
            max_total_input_storage_bits: first
                .total_input_storage_bits
                .max(second.total_input_storage_bits),
            exact: exact_limits_for_two_works(&first.exact, &second.exact),
        }
    }

    fn solid_classification_limits_from_work(
        work: &SharedHingeSolidClassificationWorkV1,
    ) -> SharedHingeSolidClassificationLimitsV1 {
        SharedHingeSolidClassificationLimitsV1 {
            max_authenticated_faces: work.authenticated_faces,
            max_authenticated_hinges: work.authenticated_hinges,
            max_unordered_face_pair_count_calculations: work.unordered_face_pair_count_calculations,
            max_unordered_face_pairs: work.unordered_face_pairs,
            max_triangle_pairs: work.triangle_pairs,
            max_prism_complete_scan_checks: work.prism_complete_scan_checks,
            max_upstream_capability_revalidations: work.upstream_capability_revalidations,
            max_sealed_prior_work_bindings: work.sealed_prior_work_bindings,
            max_root_bindings: work.root_bindings,
            max_angle_bindings: work.angle_bindings,
            max_face_identity_bindings: work.face_identity_bindings,
            max_hinge_identity_bindings: work.hinge_identity_bindings,
            max_endpoint_identity_bindings: work.endpoint_identity_bindings,
            max_face_transform_bit_bindings: work.face_transform_bit_bindings,
            max_hinge_parent_transform_bit_bindings: work.hinge_parent_transform_bit_bindings,
            max_interaction_kind_bindings: work.interaction_kind_bindings,
            max_policy_cell_bindings: work.policy_cell_bindings,
            max_classification_seal_bindings: work.classification_seal_bindings,
            max_margin_component_checks: work.margin_component_checks,
            max_independent_corridor_vertex_checks: work.independent_corridor_vertex_checks,
            prism: prism_limits_for_two_works(
                &work.independent_exact_prism,
                &work.independent_direct_prism,
            ),
            corridor_exact: exact_limits_for_two_works(
                &work.independent_exact_corridor,
                &work.independent_direct_corridor,
            ),
        }
    }

    #[test]
    fn shared_hinge_solid_classification_contract_matches_all_eleven_v2_cells() {
        use crate::{
            IntersectionEvidenceV2, TopologyContactDecision, TopologyRelation,
            classify_runtime_topology_contact_v2,
        };

        let expected_shared_hinge_row = [
            TopologyContactDecision::Indeterminate,
            TopologyContactDecision::Indeterminate,
            TopologyContactDecision::Indeterminate,
            TopologyContactDecision::RequiresHingeModel,
            TopologyContactDecision::RequiresHingeModel,
            TopologyContactDecision::RequiresHingeModel,
            TopologyContactDecision::RequiresHingeModel,
            TopologyContactDecision::Penetrating,
            TopologyContactDecision::Penetrating,
            TopologyContactDecision::Penetrating,
            TopologyContactDecision::Indeterminate,
        ];
        for (evidence, expected) in IntersectionEvidenceV2::ALL
            .into_iter()
            .zip(expected_shared_hinge_row)
        {
            assert_eq!(
                classify_runtime_topology_contact_v2(TopologyRelation::SharedHingeEdge, evidence,),
                expected,
                "{}",
                evidence.identifier()
            );
        }

        for (class, evidence, decision) in [
            (
                SharedHingePositiveThicknessPairClassV1::SharedFeatureOnlyContact,
                IntersectionEvidenceV2::SharedFeatureContact,
                TopologyContactDecision::RequiresHingeModel,
            ),
            (
                SharedHingePositiveThicknessPairClassV1::AllowedFiniteCorridorOverlap,
                IntersectionEvidenceV2::SharedFeatureThicknessOverlap,
                TopologyContactDecision::RequiresHingeModel,
            ),
            (
                SharedHingePositiveThicknessPairClassV1::AllowedBoundaryContact,
                IntersectionEvidenceV2::BoundaryAreaContact,
                TopologyContactDecision::RequiresHingeModel,
            ),
            (
                SharedHingePositiveThicknessPairClassV1::PositiveVolumeIntersection,
                IntersectionEvidenceV2::PositiveVolumeOverlap,
                TopologyContactDecision::Penetrating,
            ),
            (
                SharedHingePositiveThicknessPairClassV1::EvidenceUnavailable,
                IntersectionEvidenceV2::Indeterminate,
                TopologyContactDecision::Indeterminate,
            ),
        ] {
            assert_eq!(policy_contract_for_test(class), (evidence, decision));
        }
    }

    #[test]
    fn shared_hinge_solid_classification_both_rank3_outside_corridors_reaches_positive_volume_intersection()
     {
        let prism = [
            point(integer(0), integer(0), integer(1)),
            point(integer(4), integer(0), integer(1)),
            point(integer(0), integer(4), integer(1)),
            point(integer(0), integer(0), integer(-1)),
            point(integer(4), integer(0), integer(-1)),
            point(integer(0), integer(4), integer(-1)),
        ];
        let solids = [prism.clone(), prism];
        let corridor = |half_thickness: i64| {
            [
                integer(0),
                integer(0),
                integer(0),
                integer(4),
                integer(0),
                integer(0),
                integer(16),
                integer(half_thickness),
                integer(1),
                integer(half_thickness * half_thickness * 16),
            ]
        };

        let wide = corridor(10);
        let (wide_class, wide_exact_outside, wide_direct_outside, wide_work) =
            classify_independent_exact_fixture_for_test(&solids, &solids, &wide, &wide).unwrap();
        assert_eq!(wide_class, None);
        assert_eq!((wide_exact_outside, wide_direct_outside), (0, 0));
        assert_eq!(wide_work.independent_exact_prism.plane_triples, 120);
        assert_eq!(wide_work.independent_direct_prism.plane_triples, 120);

        let narrow = corridor(1);
        let (narrow_class, narrow_exact_outside, narrow_direct_outside, narrow_work) =
            classify_independent_exact_fixture_for_test(&solids, &solids, &narrow, &narrow)
                .unwrap();
        assert_eq!(
            narrow_class,
            Some(SharedHingePositiveThicknessPairClassV1::PositiveVolumeIntersection)
        );
        assert!(narrow_exact_outside > 0);
        assert!(narrow_direct_outside > 0);
        assert_eq!(
            narrow_work.independent_corridor_vertex_checks,
            wide_work.independent_corridor_vertex_checks
        );
    }

    #[test]
    fn shared_hinge_solid_classification_full_400mm_matrix_is_order_and_root_invariant() {
        use crate::{IntersectionEvidenceV2, TopologyContactDecision};

        let square = [(0.0, 0.0), (400.0, 0.0), (400.0, 400.0), (0.0, 400.0)];
        let models = [
            (
                "mountain/source",
                two_triangle_model_with_all_options(EdgeKind::Mountain, false, false, square, 901),
            ),
            (
                "mountain/reordered",
                two_triangle_model_with_all_options(EdgeKind::Mountain, true, false, square, 902),
            ),
            (
                "valley/source",
                two_triangle_model_with_all_options(EdgeKind::Valley, false, false, square, 903),
            ),
            (
                "valley/reordered",
                two_triangle_model_with_all_options(EdgeKind::Valley, true, false, square, 904),
            ),
            (
                "mountain/reversed-endpoints",
                two_triangle_model_with_all_options(EdgeKind::Mountain, false, true, square, 905),
            ),
            (
                "mountain/reordered/reversed-endpoints",
                two_triangle_model_with_all_options(EdgeKind::Mountain, true, true, square, 906),
            ),
            (
                "valley/reversed-endpoints",
                two_triangle_model_with_all_options(EdgeKind::Valley, false, true, square, 907),
            ),
            (
                "valley/reordered/reversed-endpoints",
                two_triangle_model_with_all_options(EdgeKind::Valley, true, true, square, 908),
            ),
        ];
        // The eight issuer-bound model matrices share no mutable authority.
        // Batch them per immutable fixture while retaining every root,
        // thickness and angle proof/revalidation below.
        let totals = std::thread::scope(|scope| {
            models
                .chunks(2)
                .map(|model_batch| {
                    scope.spawn(move || {
                        let mut boundary = 0_usize;
                        let mut finite_corridor_volume = 0_usize;
                        let mut direct_unavailable = 0_usize;
                        let mut layer_offset = 0_usize;
                        for (fixture, model) in model_batch {
            for root in model.face_ids() {
                for thickness in [0.1, 1.0, 3.0] {
                    for angle in [0.0, 10.0, 90.0, 135.0, 179.0, 180.0] {
                        let pose = triangular_pose_with_root(model, angle, *root);
                        let bound_pose = model.bind_pose(&pose).unwrap();
                        let exact = triangular_exact_pose(model, &pose);
                        let prerequisite_analysis =
                            authenticated_ef_prerequisite(&exact, thickness);

                        if angle == 180.0 {
                            let exact_e_analysis = analyze_exact_e_finite_hinge_corridor_v1(
                                &prerequisite_analysis,
                                None,
                                &exact,
                                bound_pose,
                                thickness,
                                ExactEFiniteHingeCorridorLimits::default(),
                            )
                            .unwrap();
                            let direct_f_analysis = analyze_direct_f_finite_hinge_corridor_v1(
                                &prerequisite_analysis,
                                None,
                                &exact_e_analysis,
                                &exact,
                                bound_pose,
                                thickness,
                                DirectFFiniteHingeCorridorLimits::default(),
                            )
                            .unwrap();
                            let admission_analysis = analyze_shared_hinge_corridor_admission_v1(
                                &prerequisite_analysis,
                                None,
                                &exact_e_analysis,
                                &direct_f_analysis,
                                &exact,
                                bound_pose,
                                thickness,
                                SharedHingeCorridorAdmissionLimitsV1::default(),
                            )
                            .unwrap();
                            let margin_analysis =
                                analyze_shared_hinge_native_exact_topology_margin_v1(
                                    &prerequisite_analysis,
                                    None,
                                    &exact,
                                    bound_pose,
                                    thickness,
                                    SharedHingeNativeExactTopologyMarginLimitsV1::default(),
                                )
                                .unwrap();
                            let analysis = analyze_shared_hinge_solid_classification_v1(
                                &prerequisite_analysis,
                                None,
                                &exact_e_analysis,
                                &direct_f_analysis,
                                &admission_analysis,
                                &margin_analysis,
                                &exact,
                                bound_pose,
                                thickness,
                                SharedHingeSolidClassificationLimitsV1::default(),
                            )
                            .unwrap();
                            assert!(
                                matches!(
                                    analysis.result,
                                    SharedHingeSolidClassificationResultV1::EvidenceUnavailable(
                                        SharedHingeEvidenceUnavailableReasonV1::LayerOffsetUnmodeled
                                    )
                                ),
                                "{fixture}, {root:?}, {thickness}, {angle}"
                            );
                            assert_eq!(
                                analysis.work,
                                SharedHingeSolidClassificationWorkV1::default()
                            );
                            layer_offset += 1;
                            continue;
                        }

                        let SingleTriangularHingePrerequisiteResult::Authenticated(prerequisite) =
                            &prerequisite_analysis.result
                        else {
                            panic!("{fixture}, {root:?}, {thickness}, {angle}: prerequisite");
                        };
                        let ef_analysis = analyze_axis_aligned_ef_boundary_v1(
                            prerequisite,
                            &exact,
                            bound_pose,
                            thickness,
                            AxisAlignedEfBoundaryLimits::default(),
                        )
                        .unwrap();
                        let ef = ef_capability(&ef_analysis);
                        let exact_e_analysis = analyze_exact_e_finite_hinge_corridor_v1(
                            &prerequisite_analysis,
                            Some(ef),
                            &exact,
                            bound_pose,
                            thickness,
                            ExactEFiniteHingeCorridorLimits::default(),
                        )
                        .unwrap();
                        let direct_f_analysis = analyze_direct_f_finite_hinge_corridor_v1(
                            &prerequisite_analysis,
                            Some(ef),
                            &exact_e_analysis,
                            &exact,
                            bound_pose,
                            thickness,
                            DirectFFiniteHingeCorridorLimits::default(),
                        )
                        .unwrap();
                        let admission_analysis = analyze_shared_hinge_corridor_admission_v1(
                            &prerequisite_analysis,
                            Some(ef),
                            &exact_e_analysis,
                            &direct_f_analysis,
                            &exact,
                            bound_pose,
                            thickness,
                            SharedHingeCorridorAdmissionLimitsV1::default(),
                        )
                        .unwrap();
                        let margin_analysis = analyze_shared_hinge_native_exact_topology_margin_v1(
                            &prerequisite_analysis,
                            Some(ef),
                            &exact,
                            bound_pose,
                            thickness,
                            SharedHingeNativeExactTopologyMarginLimitsV1::default(),
                        )
                        .unwrap();
                        let analysis = analyze_shared_hinge_solid_classification_v1(
                            &prerequisite_analysis,
                            Some(ef),
                            &exact_e_analysis,
                            &direct_f_analysis,
                            &admission_analysis,
                            &margin_analysis,
                            &exact,
                            bound_pose,
                            thickness,
                            SharedHingeSolidClassificationLimitsV1::default(),
                        )
                        .unwrap();

                        if angle == 90.0 {
                            assert!(
                                matches!(
                                    analysis.result,
                                    SharedHingeSolidClassificationResultV1::EvidenceUnavailable(
                                        SharedHingeEvidenceUnavailableReasonV1::
                                            DirectIntersectionUnavailable
                                    )
                                ),
                                "{fixture}, {root:?}, {thickness}, {angle}: {:?}",
                                analysis.result
                            );
                            assert_eq!(analysis.work.independent_exact_prism.prisms, 2);
                            assert_eq!(analysis.work.independent_direct_prism.prisms, 2);
                            assert!(analysis.work.independent_corridor_vertex_checks > 0);
                            direct_unavailable += 1;
                            continue;
                        }

                        let (record, _) = analysis
                            .sealed_non_authoritative_record_and_work()
                            .unwrap_or_else(|| {
                                panic!(
                                    "{fixture}, {root:?}, {thickness}, {angle}: {:?}",
                                    analysis.result
                                )
                            });
                        assert_eq!(record.coverage_for_test(), (1, 1, 1, 1));
                        let exact_e = exact_e_corridor_capability(&exact_e_analysis);
                        let direct_f = direct_f_corridor_capability(&direct_f_analysis);
                        if angle == 0.0 {
                            assert_eq!(
                                record.class_for_test(),
                                SharedHingePositiveThicknessPairClassV1::AllowedBoundaryContact
                            );
                            assert_eq!(
                                record.intersection_for_test(),
                                (
                                    SharedHingeSolidIntersectionDimensionV1::BoundaryArea,
                                    SharedHingeSolidIntersectionDimensionV1::BoundaryArea,
                                )
                            );
                            assert_eq!(
                                record.policy_for_test(),
                                (
                                    IntersectionEvidenceV2::BoundaryAreaContact,
                                    IntersectionEvidenceV2::BoundaryAreaContact,
                                    TopologyContactDecision::RequiresHingeModel,
                                )
                            );
                            assert_eq!(
                                record.reconciliation_for_test(),
                                SharedHingeCorridorReconciliationV1::BitExactIdenticalCorridor
                            );
                            assert_eq!(analysis.work.margin_component_checks, 0);
                            let admission = admission_analysis
                                .authenticated_admission_capability_and_work()
                                .unwrap()
                                .0;
                            assert!(
                                revalidate_shared_hinge_solid_classification_v1(
                                    record,
                                    prerequisite,
                                    ef,
                                    exact_e,
                                    direct_f,
                                    Some(admission),
                                    None,
                                    &exact,
                                    bound_pose,
                                    thickness,
                                )
                                .is_some()
                            );
                            boundary += 1;
                        } else {
                            assert!(matches!(angle, 10.0 | 135.0 | 179.0));
                            assert_eq!(
                                record.class_for_test(),
                                SharedHingePositiveThicknessPairClassV1::
                                    AllowedFiniteCorridorOverlap
                            );
                            assert_eq!(
                                record.intersection_for_test(),
                                (
                                    SharedHingeSolidIntersectionDimensionV1::PositiveVolume,
                                    SharedHingeSolidIntersectionDimensionV1::PositiveVolume,
                                )
                            );
                            assert_eq!(
                                record.policy_for_test(),
                                (
                                    IntersectionEvidenceV2::PositiveVolumeOverlap,
                                    IntersectionEvidenceV2::SharedFeatureThicknessOverlap,
                                    TopologyContactDecision::RequiresHingeModel,
                                )
                            );
                            assert_eq!(
                                record.reconciliation_for_test(),
                                SharedHingeCorridorReconciliationV1::NativeExactTopologyMargin
                            );
                            assert_eq!(analysis.work.margin_component_checks, 10);
                            let margin = topology_margin_capability(&margin_analysis);
                            assert!(
                                revalidate_shared_hinge_solid_classification_v1(
                                    record,
                                    prerequisite,
                                    ef,
                                    exact_e,
                                    direct_f,
                                    None,
                                    Some(margin),
                                    &exact,
                                    bound_pose,
                                    thickness,
                                )
                                .is_some()
                            );
                            finite_corridor_volume += 1;
                        }
                    }
                }
            }
                        }
                        (
                            boundary,
                            finite_corridor_volume,
                            direct_unavailable,
                            layer_offset,
                        )
                    })
                })
                .collect::<Vec<_>>()
                .into_iter()
                .map(|worker| worker.join().expect("shared-hinge matrix worker"))
                .collect::<Vec<_>>()
        });
        let (mut boundary, mut finite_corridor_volume, mut direct_unavailable, mut layer_offset) =
            (0_usize, 0_usize, 0_usize, 0_usize);
        for (worker_boundary, worker_volume, worker_unavailable, worker_layer_offset) in totals {
            boundary += worker_boundary;
            finite_corridor_volume += worker_volume;
            direct_unavailable += worker_unavailable;
            layer_offset += worker_layer_offset;
        }
        assert_eq!(boundary, 48);
        assert_eq!(finite_corridor_volume, 144);
        assert_eq!(direct_unavailable, 48);
        assert_eq!(layer_offset, 48);
    }

    #[test]
    fn shared_hinge_solid_classification_rejects_authority_swaps_aba_ulp_and_all_seals() {
        let model = two_triangle_model();
        let pose = triangular_pose(&model, 135.0);
        let bound = model.bind_pose(&pose).unwrap();
        let exact = triangular_exact_pose(&model, &pose);
        let independently_regenerated_exact = triangular_exact_pose(&model, &pose);
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
        let duplicate_direct_f_analysis = analyze_direct_f_finite_hinge_corridor_v1(
            &prerequisite_analysis,
            Some(ef),
            &exact_e_analysis,
            &exact,
            bound,
            0.1,
            DirectFFiniteHingeCorridorLimits::default(),
        )
        .unwrap();
        let direct_f = direct_f_corridor_capability(&direct_f_analysis);
        let duplicate_direct_f = direct_f_corridor_capability(&duplicate_direct_f_analysis);
        let admission_analysis = analyze_shared_hinge_corridor_admission_v1(
            &prerequisite_analysis,
            Some(ef),
            &exact_e_analysis,
            &direct_f_analysis,
            &exact,
            bound,
            0.1,
            SharedHingeCorridorAdmissionLimitsV1::default(),
        )
        .unwrap();
        let margin_analysis = analyze_shared_hinge_native_exact_topology_margin_v1(
            &prerequisite_analysis,
            Some(ef),
            &exact,
            bound,
            0.1,
            SharedHingeNativeExactTopologyMarginLimitsV1::default(),
        )
        .unwrap();
        let duplicate_margin_analysis = analyze_shared_hinge_native_exact_topology_margin_v1(
            &prerequisite_analysis,
            Some(ef),
            &exact,
            bound,
            0.1,
            SharedHingeNativeExactTopologyMarginLimitsV1::default(),
        )
        .unwrap();
        let margin = topology_margin_capability(&margin_analysis);
        let duplicate_margin = topology_margin_capability(&duplicate_margin_analysis);
        let mut analysis = analyze_shared_hinge_solid_classification_v1(
            &prerequisite_analysis,
            Some(ef),
            &exact_e_analysis,
            &direct_f_analysis,
            &admission_analysis,
            &margin_analysis,
            &exact,
            bound,
            0.1,
            SharedHingeSolidClassificationLimitsV1::default(),
        )
        .unwrap();

        macro_rules! current_record {
            () => {{
                let SharedHingeSolidClassificationResultV1::Classified(record) = &analysis.result
                else {
                    panic!("classification record");
                };
                record.as_ref()
            }};
        }
        macro_rules! assert_same_stack_rejected {
            ($reason:expr) => {
                assert!(
                    revalidate_shared_hinge_solid_classification_v1(
                        current_record!(),
                        prerequisite,
                        ef,
                        exact_e,
                        direct_f,
                        None,
                        Some(margin),
                        &exact,
                        bound,
                        0.1,
                    )
                    .is_none(),
                    "{}",
                    $reason
                );
            };
        }

        assert!(
            revalidate_shared_hinge_solid_classification_v1(
                current_record!(),
                prerequisite,
                ef,
                exact_e,
                direct_f,
                None,
                Some(margin),
                &exact,
                bound,
                0.1,
            )
            .is_some()
        );
        assert!(
            revalidate_shared_hinge_solid_classification_v1(
                current_record!(),
                duplicate_prerequisite,
                ef,
                exact_e,
                direct_f,
                None,
                Some(margin),
                &exact,
                bound,
                0.1,
            )
            .is_none(),
            "independently issued prerequisite"
        );
        assert!(
            revalidate_shared_hinge_solid_classification_v1(
                current_record!(),
                prerequisite,
                duplicate_ef,
                exact_e,
                direct_f,
                None,
                Some(margin),
                &exact,
                bound,
                0.1,
            )
            .is_none(),
            "independently issued E/F boundary"
        );
        assert!(
            revalidate_shared_hinge_solid_classification_v1(
                current_record!(),
                prerequisite,
                ef,
                duplicate_exact_e,
                direct_f,
                None,
                Some(margin),
                &exact,
                bound,
                0.1,
            )
            .is_none(),
            "independently issued exact-E prism/corridor"
        );
        assert!(
            revalidate_shared_hinge_solid_classification_v1(
                current_record!(),
                prerequisite,
                ef,
                exact_e,
                duplicate_direct_f,
                None,
                Some(margin),
                &exact,
                bound,
                0.1,
            )
            .is_none(),
            "independently issued direct-F prism/corridor"
        );
        assert!(
            revalidate_shared_hinge_solid_classification_v1(
                current_record!(),
                prerequisite,
                ef,
                exact_e,
                direct_f,
                None,
                Some(duplicate_margin),
                &exact,
                bound,
                0.1,
            )
            .is_none(),
            "independently issued topology margin"
        );
        assert!(
            revalidate_shared_hinge_solid_classification_v1(
                current_record!(),
                prerequisite,
                ef,
                exact_e,
                direct_f,
                None,
                Some(margin),
                &independently_regenerated_exact,
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
            ("reroot", rerooted_bound),
            ("one-ULP angle", one_ulp_bound),
        ] {
            assert!(
                revalidate_shared_hinge_solid_classification_v1(
                    current_record!(),
                    prerequisite,
                    ef,
                    exact_e,
                    direct_f,
                    None,
                    Some(margin),
                    &exact,
                    mismatched_bound,
                    0.1,
                )
                .is_none(),
                "{name}"
            );
        }
        assert!(
            revalidate_shared_hinge_solid_classification_v1(
                current_record!(),
                prerequisite,
                ef,
                exact_e,
                direct_f,
                None,
                Some(margin),
                &exact,
                bound,
                next_up(0.1),
            )
            .is_none(),
            "one-ULP thickness"
        );

        {
            let SharedHingeSolidClassificationResultV1::Classified(record) = &mut analysis.result
            else {
                panic!("record");
            };
            record.toggle_class_for_test();
        }
        assert!(
            analysis
                .sealed_non_authoritative_record_and_work()
                .is_none()
        );
        assert_same_stack_rejected!("classification seal");
        {
            let SharedHingeSolidClassificationResultV1::Classified(record) = &mut analysis.result
            else {
                panic!("record");
            };
            record.toggle_class_for_test();
        }

        {
            let SharedHingeSolidClassificationResultV1::Classified(record) = &mut analysis.result
            else {
                panic!("record");
            };
            record.increment_coverage_for_test();
        }
        assert_same_stack_rejected!("pair-coverage seal");
        {
            let SharedHingeSolidClassificationResultV1::Classified(record) = &mut analysis.result
            else {
                panic!("record");
            };
            record.decrement_coverage_for_test();
        }

        {
            let SharedHingeSolidClassificationResultV1::Classified(record) = &mut analysis.result
            else {
                panic!("record");
            };
            record.increment_prior_work_for_test();
        }
        assert_same_stack_rejected!("prior-work seal");
        {
            let SharedHingeSolidClassificationResultV1::Classified(record) = &mut analysis.result
            else {
                panic!("record");
            };
            record.decrement_prior_work_for_test();
        }

        for (name, mutate, restore) in [
            ("face identities", 0_u8, 0_u8),
            ("endpoint identities", 1_u8, 1_u8),
        ] {
            let SharedHingeSolidClassificationResultV1::Classified(record) = &mut analysis.result
            else {
                panic!("record");
            };
            match mutate {
                0 => record.swap_face_identities_for_test(),
                1 => record.swap_endpoint_identities_for_test(),
                _ => unreachable!(),
            }
            let _ = record;
            assert_same_stack_rejected!(name);
            let SharedHingeSolidClassificationResultV1::Classified(record) = &mut analysis.result
            else {
                panic!("record");
            };
            match restore {
                0 => record.swap_face_identities_for_test(),
                1 => record.swap_endpoint_identities_for_test(),
                _ => unreachable!(),
            }
        }

        let original_analysis_faces = analysis.work.authenticated_faces;
        analysis.work.authenticated_faces += 1;
        assert!(
            analysis
                .sealed_non_authoritative_record_and_work()
                .is_none()
        );
        analysis.work.authenticated_faces = original_analysis_faces;

        {
            let SharedHingeSolidClassificationResultV1::Classified(record) = &mut analysis.result
            else {
                panic!("record");
            };
            record.increment_record_work_for_test();
        }
        assert_same_stack_rejected!("record-work seal");
        {
            let SharedHingeSolidClassificationResultV1::Classified(record) = &mut analysis.result
            else {
                panic!("record");
            };
            record.decrement_record_work_for_test();
        }

        let mut transformed_coefficients = 0_usize;
        for face in 0..2 {
            for coefficient in 0..12 {
                let SharedHingeSolidClassificationResultV1::Classified(record) =
                    &mut analysis.result
                else {
                    panic!("record");
                };
                record.flip_face_transform_bit_for_test(face, coefficient);
                let _ = record;
                assert_same_stack_rejected!("face-transform seal");
                let SharedHingeSolidClassificationResultV1::Classified(record) =
                    &mut analysis.result
                else {
                    panic!("record");
                };
                record.flip_face_transform_bit_for_test(face, coefficient);
                transformed_coefficients += 1;
            }
        }
        for coefficient in 0..12 {
            let SharedHingeSolidClassificationResultV1::Classified(record) = &mut analysis.result
            else {
                panic!("record");
            };
            record.flip_hinge_parent_transform_bit_for_test(coefficient);
            let _ = record;
            assert_same_stack_rejected!("hinge-parent transform seal");
            let SharedHingeSolidClassificationResultV1::Classified(record) = &mut analysis.result
            else {
                panic!("record");
            };
            record.flip_hinge_parent_transform_bit_for_test(coefficient);
            transformed_coefficients += 1;
        }
        assert_eq!(transformed_coefficients, 36);
        assert!(
            analysis
                .sealed_non_authoritative_record_and_work()
                .is_some()
        );
        assert!(
            revalidate_shared_hinge_solid_classification_v1(
                current_record!(),
                prerequisite,
                ef,
                exact_e,
                direct_f,
                None,
                Some(margin),
                &exact,
                bound,
                0.1,
            )
            .is_some()
        );
    }

    #[test]
    fn shared_hinge_solid_classification_bit_exact_route_rejects_reconciliation_substitution() {
        let model = two_triangle_model();
        let pose = triangular_pose(&model, 0.0);
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
        let exact_e = exact_e_corridor_capability(&exact_e_analysis);
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
        let direct_f = direct_f_corridor_capability(&direct_f_analysis);
        let admission_analysis = analyze_shared_hinge_corridor_admission_v1(
            &prerequisite_analysis,
            Some(ef),
            &exact_e_analysis,
            &direct_f_analysis,
            &exact,
            bound,
            0.1,
            SharedHingeCorridorAdmissionLimitsV1::default(),
        )
        .unwrap();
        let duplicate_admission_analysis = analyze_shared_hinge_corridor_admission_v1(
            &prerequisite_analysis,
            Some(ef),
            &exact_e_analysis,
            &direct_f_analysis,
            &exact,
            bound,
            0.1,
            SharedHingeCorridorAdmissionLimitsV1::default(),
        )
        .unwrap();
        let admission = admission_analysis
            .authenticated_admission_capability_and_work()
            .unwrap()
            .0;
        let duplicate_admission = duplicate_admission_analysis
            .authenticated_admission_capability_and_work()
            .unwrap()
            .0;
        let margin_analysis = analyze_shared_hinge_native_exact_topology_margin_v1(
            &prerequisite_analysis,
            Some(ef),
            &exact,
            bound,
            0.1,
            SharedHingeNativeExactTopologyMarginLimitsV1::default(),
        )
        .unwrap();
        let margin = topology_margin_capability(&margin_analysis);
        let analysis = analyze_shared_hinge_solid_classification_v1(
            &prerequisite_analysis,
            Some(ef),
            &exact_e_analysis,
            &direct_f_analysis,
            &admission_analysis,
            &margin_analysis,
            &exact,
            bound,
            0.1,
            SharedHingeSolidClassificationLimitsV1::default(),
        )
        .unwrap();
        let record = analysis
            .sealed_non_authoritative_record_and_work()
            .unwrap()
            .0;
        assert!(
            revalidate_shared_hinge_solid_classification_v1(
                record,
                prerequisite,
                ef,
                exact_e,
                direct_f,
                Some(admission),
                None,
                &exact,
                bound,
                0.1,
            )
            .is_some()
        );
        assert!(
            revalidate_shared_hinge_solid_classification_v1(
                record,
                prerequisite,
                ef,
                exact_e,
                direct_f,
                Some(duplicate_admission),
                None,
                &exact,
                bound,
                0.1,
            )
            .is_none(),
            "equal independently issued admission is not authority"
        );
        assert!(
            revalidate_shared_hinge_solid_classification_v1(
                record,
                prerequisite,
                ef,
                exact_e,
                direct_f,
                None,
                Some(margin),
                &exact,
                bound,
                0.1,
            )
            .is_none(),
            "a margin token cannot substitute for a bit-exact admission"
        );
        assert!(
            revalidate_shared_hinge_solid_classification_v1(
                record,
                prerequisite,
                ef,
                exact_e,
                direct_f,
                Some(admission),
                Some(margin),
                &exact,
                bound,
                0.1,
            )
            .is_none(),
            "ambiguous dual reconciliation input is rejected"
        );
    }

    #[test]
    fn shared_hinge_solid_classification_all_structural_limits_are_exact_and_one_short() {
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
        let admission_analysis = analyze_shared_hinge_corridor_admission_v1(
            &prerequisite_analysis,
            Some(ef),
            &exact_e_analysis,
            &direct_f_analysis,
            &exact,
            bound,
            0.1,
            SharedHingeCorridorAdmissionLimitsV1::default(),
        )
        .unwrap();
        let margin_analysis = analyze_shared_hinge_native_exact_topology_margin_v1(
            &prerequisite_analysis,
            Some(ef),
            &exact,
            bound,
            0.1,
            SharedHingeNativeExactTopologyMarginLimitsV1::default(),
        )
        .unwrap();
        let baseline = analyze_shared_hinge_solid_classification_v1(
            &prerequisite_analysis,
            Some(ef),
            &exact_e_analysis,
            &direct_f_analysis,
            &admission_analysis,
            &margin_analysis,
            &exact,
            bound,
            0.1,
            SharedHingeSolidClassificationLimitsV1::default(),
        )
        .unwrap();
        assert!(
            baseline
                .sealed_non_authoritative_record_and_work()
                .is_some()
        );
        let exact_limits = solid_classification_limits_from_work(&baseline.work);
        let exact_analysis = analyze_shared_hinge_solid_classification_v1(
            &prerequisite_analysis,
            Some(ef),
            &exact_e_analysis,
            &direct_f_analysis,
            &admission_analysis,
            &margin_analysis,
            &exact,
            bound,
            0.1,
            exact_limits,
        )
        .unwrap();
        assert!(
            exact_analysis
                .sealed_non_authoritative_record_and_work()
                .is_some()
        );
        assert_eq!(exact_analysis.work, baseline.work);

        let assert_one_short = |resource: &str, limits: SharedHingeSolidClassificationLimitsV1| {
            assert!(
                matches!(
                    analyze_shared_hinge_solid_classification_v1(
                        &prerequisite_analysis,
                        Some(ef),
                        &exact_e_analysis,
                        &direct_f_analysis,
                        &admission_analysis,
                        &margin_analysis,
                        &exact,
                        bound,
                        0.1,
                        limits,
                    ),
                    Err(SharedHingeSolidClassificationErrorV1::ResourceLimitExceeded)
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
        one_short!(max_unordered_face_pair_count_calculations);
        one_short!(max_unordered_face_pairs);
        one_short!(max_triangle_pairs);
        one_short!(max_prism_complete_scan_checks);
        one_short!(max_upstream_capability_revalidations);
        one_short!(max_sealed_prior_work_bindings);
        one_short!(max_root_bindings);
        one_short!(max_angle_bindings);
        one_short!(max_face_identity_bindings);
        one_short!(max_hinge_identity_bindings);
        one_short!(max_endpoint_identity_bindings);
        one_short!(max_face_transform_bit_bindings);
        one_short!(max_hinge_parent_transform_bit_bindings);
        one_short!(max_interaction_kind_bindings);
        one_short!(max_policy_cell_bindings);
        one_short!(max_classification_seal_bindings);
        one_short!(max_margin_component_checks);

        let oversized = SharedHingeSolidClassificationLimitsV1 {
            max_authenticated_faces: usize::MAX,
            max_authenticated_hinges: usize::MAX,
            max_unordered_face_pair_count_calculations: usize::MAX,
            max_unordered_face_pairs: usize::MAX,
            max_triangle_pairs: usize::MAX,
            max_prism_complete_scan_checks: usize::MAX,
            max_upstream_capability_revalidations: usize::MAX,
            max_sealed_prior_work_bindings: usize::MAX,
            max_root_bindings: usize::MAX,
            max_angle_bindings: usize::MAX,
            max_face_identity_bindings: usize::MAX,
            max_hinge_identity_bindings: usize::MAX,
            max_endpoint_identity_bindings: usize::MAX,
            max_face_transform_bit_bindings: usize::MAX,
            max_hinge_parent_transform_bit_bindings: usize::MAX,
            max_interaction_kind_bindings: usize::MAX,
            max_policy_cell_bindings: usize::MAX,
            max_classification_seal_bindings: usize::MAX,
            max_margin_component_checks: usize::MAX,
            max_independent_corridor_vertex_checks: usize::MAX,
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
                exact: oversized_cayley_limits(),
            },
            corridor_exact: oversized_cayley_limits(),
        };
        let projected = analyze_shared_hinge_solid_classification_v1(
            &prerequisite_analysis,
            Some(ef),
            &exact_e_analysis,
            &direct_f_analysis,
            &admission_analysis,
            &margin_analysis,
            &exact,
            bound,
            0.1,
            oversized,
        )
        .unwrap();
        assert!(
            projected
                .sealed_non_authoritative_record_and_work()
                .is_some()
        );
        assert_eq!(projected.work, baseline.work);
    }

    #[test]
    fn shared_hinge_solid_classification_independent_fallback_limits_are_exact_and_one_short() {
        let model = two_triangle_model();
        let pose = triangular_pose(&model, 90.0);
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
        let admission_analysis = analyze_shared_hinge_corridor_admission_v1(
            &prerequisite_analysis,
            Some(ef),
            &exact_e_analysis,
            &direct_f_analysis,
            &exact,
            bound,
            0.1,
            SharedHingeCorridorAdmissionLimitsV1::default(),
        )
        .unwrap();
        let margin_analysis = analyze_shared_hinge_native_exact_topology_margin_v1(
            &prerequisite_analysis,
            Some(ef),
            &exact,
            bound,
            0.1,
            SharedHingeNativeExactTopologyMarginLimitsV1::default(),
        )
        .unwrap();
        let analyze = |limits| {
            analyze_shared_hinge_solid_classification_v1(
                &prerequisite_analysis,
                Some(ef),
                &exact_e_analysis,
                &direct_f_analysis,
                &admission_analysis,
                &margin_analysis,
                &exact,
                bound,
                0.1,
                limits,
            )
        };
        let baseline = analyze(SharedHingeSolidClassificationLimitsV1::default()).unwrap();
        assert!(matches!(
            baseline.result,
            SharedHingeSolidClassificationResultV1::EvidenceUnavailable(
                SharedHingeEvidenceUnavailableReasonV1::DirectIntersectionUnavailable
            )
        ));
        assert!(baseline.work.independent_corridor_vertex_checks > 0);

        let exact_limits = solid_classification_limits_from_work(&baseline.work);
        let exact_analysis = analyze(exact_limits).unwrap();
        assert!(matches!(
            exact_analysis.result,
            SharedHingeSolidClassificationResultV1::EvidenceUnavailable(
                SharedHingeEvidenceUnavailableReasonV1::DirectIntersectionUnavailable
            )
        ));
        assert_eq!(exact_analysis.work, baseline.work);

        let assert_one_short = |resource: &str, limits: SharedHingeSolidClassificationLimitsV1| {
            assert!(
                matches!(
                    analyze(limits),
                    Err(SharedHingeSolidClassificationErrorV1::ResourceLimitExceeded)
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
        one_short!(max_unordered_face_pair_count_calculations);
        one_short!(max_unordered_face_pairs);
        one_short!(max_triangle_pairs);
        one_short!(max_prism_complete_scan_checks);
        one_short!(max_upstream_capability_revalidations);
        one_short!(max_sealed_prior_work_bindings);
        one_short!(max_root_bindings);
        one_short!(max_angle_bindings);
        one_short!(max_face_identity_bindings);
        one_short!(max_hinge_identity_bindings);
        one_short!(max_endpoint_identity_bindings);
        one_short!(max_face_transform_bit_bindings);
        one_short!(max_hinge_parent_transform_bit_bindings);
        one_short!(max_interaction_kind_bindings);
        one_short!(max_policy_cell_bindings);
        one_short!(max_classification_seal_bindings);
        one_short!(max_margin_component_checks);
        one_short!(max_independent_corridor_vertex_checks);

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

        macro_rules! corridor_exact_one_short {
            ($field:ident) => {
                if exact_limits.corridor_exact.$field > 0 {
                    let mut limits = exact_limits;
                    limits.corridor_exact.$field -= 1;
                    assert_one_short(concat!("corridor_exact.", stringify!($field)), limits);
                }
            };
        }
        corridor_exact_one_short!(max_interval_operations);
        corridor_exact_one_short!(max_shift_bits);
        corridor_exact_one_short!(max_intermediate_bits);
        corridor_exact_one_short!(max_gcd_fallback_calls);
        corridor_exact_one_short!(max_gcd_fallback_input_bits);
        corridor_exact_one_short!(max_rational_allocations);
        corridor_exact_one_short!(max_rational_allocation_bits);
        corridor_exact_one_short!(max_total_rational_allocation_bits);
    }
}
