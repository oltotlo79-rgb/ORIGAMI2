//! Private phase-2 proof that the complete exact-`E` intersection of two
//! positive-thickness triangular prisms is contained in one finite hinge
//! corridor.
//!
//! This module deliberately emits no production collision classification,
//! safe-set proof, continuous-collision authority, persistence value, DTO, or
//! mutation authority.  It rejoins the authenticated one-hinge prerequisite
//! and the E/F binding capability, but constructs every predicate input only
//! from canonical exact `E`.  In particular, no componentwise E/F error box
//! is read by the prism kernel or by the corridor predicate.

use std::cmp::Ordering;

use num_rational::BigRational;
use num_traits::{One, Zero};
use ori_kinematics::BoundMaterialTreePose;

use super::ef_boundary::{
    AxisAlignedEfBoundaryCapabilityV1, revalidate_axis_aligned_ef_boundary_v1,
};
use super::exact_prism::{
    ExactPrismIntersectionKind, ExactPrismIntersectionReport, ExactPrismLimits, ExactPrismWork,
    ExactTriangularPrismView, analyze_exact_prism_pair_with_meter_v1,
    exact_prism_hard_cayley_limits,
};
use super::*;

const FACE_COUNT: usize = 2;
const HINGE_COUNT: usize = 1;
const LOCAL_Y_COMPONENT_LIFTS: usize = FACE_COUNT * 3;
const THICKNESS_LIFTS: usize = 1;
const HALF_THICKNESS_DIVISIONS: usize = 1;
const SCALAR_RECONSTRUCTIONS: usize = 1;
const MAX_CORRIDOR_VERTEX_TESTS: usize = 120;

/// Hard exact budget for phase-2B work outside the phase-2A prism kernel.
///
/// The interval-operation ceiling follows the frozen operation table:
/// fixed scalar reconstruction plus at most 120 complete vertex predicates.
/// The remaining ceilings are conservative finite envelopes for canonical
/// rational input sizes; the phase-2A envelope is added separately.
fn exact_e_corridor_local_hard_cayley_limits() -> CayleyLimits {
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
        max_gcd_fallback_calls: 2_048,
        max_gcd_fallback_input_bits: exact.max_gcd_fallback_input_bits,
        max_rational_allocations: 8_192,
        max_rational_allocation_bits: exact.max_rational_allocation_bits,
        max_total_rational_allocation_bits: 67_108_864,
        max_output_bits: 0,
    }
}

fn exact_e_corridor_combined_hard_cayley_limits() -> CayleyLimits {
    let corridor = exact_e_corridor_local_hard_cayley_limits();
    let prism = exact_prism_hard_cayley_limits();
    let sum = |left: usize, right: usize, resource: &'static str| {
        checked_hard_limit_sum(left, right, resource)
            .expect("phase-2 exact hard-limit constants must fit usize")
    };
    CayleyLimits {
        max_precision_rounds: corridor
            .max_precision_rounds
            .max(prism.max_precision_rounds),
        max_guard_bits: corridor.max_guard_bits.max(prism.max_guard_bits),
        max_candidate_bits: corridor.max_candidate_bits.max(prism.max_candidate_bits),
        max_machin_terms_per_series: corridor
            .max_machin_terms_per_series
            .max(prism.max_machin_terms_per_series),
        max_trig_terms_per_series: corridor
            .max_trig_terms_per_series
            .max(prism.max_trig_terms_per_series),
        max_sqrt_refinements: corridor
            .max_sqrt_refinements
            .max(prism.max_sqrt_refinements),
        max_interval_operations: sum(
            corridor.max_interval_operations,
            prism.max_interval_operations,
            "exact_e_corridor_combined_interval_operations",
        ),
        max_shift_bits: corridor.max_shift_bits.max(prism.max_shift_bits),
        max_intermediate_bits: corridor
            .max_intermediate_bits
            .max(prism.max_intermediate_bits),
        max_gcd_fallback_calls: sum(
            corridor.max_gcd_fallback_calls,
            prism.max_gcd_fallback_calls,
            "exact_e_corridor_combined_gcd_calls",
        ),
        max_gcd_fallback_input_bits: sum(
            corridor.max_gcd_fallback_input_bits,
            prism.max_gcd_fallback_input_bits,
            "exact_e_corridor_combined_gcd_input_bits",
        ),
        max_rational_allocations: sum(
            corridor.max_rational_allocations,
            prism.max_rational_allocations,
            "exact_e_corridor_combined_allocations",
        ),
        max_rational_allocation_bits: corridor
            .max_rational_allocation_bits
            .max(prism.max_rational_allocation_bits),
        max_total_rational_allocation_bits: sum(
            corridor.max_total_rational_allocation_bits,
            prism.max_total_rational_allocation_bits,
            "exact_e_corridor_combined_allocation_bits",
        ),
        max_output_bits: corridor.max_output_bits.max(prism.max_output_bits),
    }
}

/// Caller-non-expandable limits for one exact-E corridor proof.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ExactEFiniteHingeCorridorLimits {
    pub(super) max_authenticated_faces: usize,
    pub(super) max_authenticated_hinges: usize,
    pub(super) max_local_y_component_lifts: usize,
    pub(super) max_thickness_lifts: usize,
    pub(super) max_half_thickness_divisions: usize,
    pub(super) max_scalar_reconstructions: usize,
    pub(super) max_corridor_vertex_tests: usize,
    pub(super) prism: ExactPrismLimits,
    pub(super) exact: CayleyLimits,
}

impl Default for ExactEFiniteHingeCorridorLimits {
    fn default() -> Self {
        Self {
            max_authenticated_faces: FACE_COUNT,
            max_authenticated_hinges: HINGE_COUNT,
            max_local_y_component_lifts: LOCAL_Y_COMPONENT_LIFTS,
            max_thickness_lifts: THICKNESS_LIFTS,
            max_half_thickness_divisions: HALF_THICKNESS_DIVISIONS,
            max_scalar_reconstructions: SCALAR_RECONSTRUCTIONS,
            max_corridor_vertex_tests: MAX_CORRIDOR_VERTEX_TESTS,
            prism: ExactPrismLimits::default(),
            exact: exact_e_corridor_combined_hard_cayley_limits(),
        }
    }
}

impl ExactEFiniteHingeCorridorLimits {
    fn projected(self) -> Self {
        let hard = Self::default();
        Self {
            max_authenticated_faces: self
                .max_authenticated_faces
                .min(hard.max_authenticated_faces),
            max_authenticated_hinges: self
                .max_authenticated_hinges
                .min(hard.max_authenticated_hinges),
            max_local_y_component_lifts: self
                .max_local_y_component_lifts
                .min(hard.max_local_y_component_lifts),
            max_thickness_lifts: self.max_thickness_lifts.min(hard.max_thickness_lifts),
            max_half_thickness_divisions: self
                .max_half_thickness_divisions
                .min(hard.max_half_thickness_divisions),
            max_scalar_reconstructions: self
                .max_scalar_reconstructions
                .min(hard.max_scalar_reconstructions),
            max_corridor_vertex_tests: self
                .max_corridor_vertex_tests
                .min(hard.max_corridor_vertex_tests),
            prism: self.prism.projected(),
            exact: project_cayley_limits(self.exact, hard.exact),
        }
    }
}

/// Exact observed phase-2 work.
///
/// `prism.exact` is the phase-2A delta. `exact` is the cumulative B+A work
/// that phase 3 must resume; the prism delta must not be merged a second time.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct ExactEFiniteHingeCorridorWork {
    pub(super) authenticated_faces: usize,
    pub(super) authenticated_hinges: usize,
    pub(super) local_y_component_lifts: usize,
    pub(super) thickness_lifts: usize,
    pub(super) half_thickness_divisions: usize,
    pub(super) scalar_reconstructions: usize,
    pub(super) corridor_vertex_tests: usize,
    pub(super) prism: ExactPrismWork,
    pub(super) exact: CayleyWork,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ExactEFiniteHingeCorridorError {
    ResourceLimitExceeded,
}

/// Interaction dimension authenticated by the exact prism kernel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ExactEFiniteHingeInteractionKind {
    BoundaryAreaContact,
    PositiveVolume,
}

#[derive(Debug)]
struct ExactEFiniteHingeCorridorGeometry {
    axis: ExactVector3,
    length_squared: BigRational,
    half_thickness: BigRational,
    cosine_half_squared: BigRational,
    radial_limit_product: BigRational,
    left_normal: ExactVector3,
    right_normal: ExactVector3,
}

/// Borrow-bound, non-persistent proof for exact E only.
///
/// It deliberately implements neither `Clone`, `Copy`, nor serialization.
#[derive(Debug)]
pub(super) struct ExactEFiniteHingeCorridorCapabilityV1<'prerequisite, 'ef, 'exact, 'pose> {
    prerequisite: &'prerequisite AuthenticatedSingleTriangularHingePrerequisitesV1<'exact, 'pose>,
    ef_boundary: &'ef AxisAlignedEfBoundaryCapabilityV1<'prerequisite, 'exact, 'pose>,
    exact: &'exact RationalCayleyTreePose<'pose>,
    bound: BoundMaterialTreePose<'pose>,
    paper_thickness_bits: u64,
    left_face_index: usize,
    right_face_index: usize,
    hinge_index: usize,
    interaction_kind: ExactEFiniteHingeInteractionKind,
    geometry: ExactEFiniteHingeCorridorGeometry,
}

impl ExactEFiniteHingeCorridorCapabilityV1<'_, '_, '_, '_> {
    pub(super) fn interaction_kind(&self) -> ExactEFiniteHingeInteractionKind {
        self.interaction_kind
    }

    pub(super) fn length_squared(&self) -> &BigRational {
        &self.geometry.length_squared
    }

    pub(super) fn half_thickness(&self) -> &BigRational {
        &self.geometry.half_thickness
    }

    pub(super) fn cosine_half_squared(&self) -> &BigRational {
        &self.geometry.cosine_half_squared
    }
}

#[derive(Debug)]
pub(super) struct RevalidatedExactEFiniteHingeCorridorCapabilityV1<
    'capability,
    'prerequisite,
    'ef,
    'exact,
    'pose,
> {
    pub(super) capability:
        &'capability ExactEFiniteHingeCorridorCapabilityV1<'prerequisite, 'ef, 'exact, 'pose>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ExactEFiniteHingeCorridorOutside {
    pub(super) interaction_kind: ExactEFiniteHingeInteractionKind,
    pub(super) first_outside_vertex_index: usize,
    pub(super) outside_vertex_count: usize,
    pub(super) axial_before_start: bool,
    pub(super) axial_after_end: bool,
    pub(super) radial_outside: bool,
}

#[derive(Debug)]
pub(super) enum ExactEFiniteHingeCorridorResult<'prerequisite, 'ef, 'exact, 'pose> {
    Contained(Box<ExactEFiniteHingeCorridorCapabilityV1<'prerequisite, 'ef, 'exact, 'pose>>),
    Outside(ExactEFiniteHingeCorridorOutside),
    LayerOffsetUnmodeled,
    Unresolved,
}

#[derive(Debug)]
pub(super) struct ExactEFiniteHingeCorridorAnalysis<'prerequisite, 'ef, 'exact, 'pose> {
    pub(super) result: ExactEFiniteHingeCorridorResult<'prerequisite, 'ef, 'exact, 'pose>,
    pub(super) work: ExactEFiniteHingeCorridorWork,
}

/// Rejoins phase 1 and the E/F binding, then proves the complete exact-E
/// intersection against the closed finite corridor.
///
/// This function remains private and disconnected from production policy.
pub(super) fn analyze_exact_e_finite_hinge_corridor_v1<'prerequisite, 'ef, 'exact, 'pose>(
    prerequisite_analysis: &'prerequisite SingleTriangularHingePrerequisiteAnalysis<'exact, 'pose>,
    ef_boundary: Option<&'ef AxisAlignedEfBoundaryCapabilityV1<'prerequisite, 'exact, 'pose>>,
    exact: &'exact RationalCayleyTreePose<'pose>,
    bound: BoundMaterialTreePose<'pose>,
    paper_thickness_mm: f64,
    limits: ExactEFiniteHingeCorridorLimits,
) -> Result<
    ExactEFiniteHingeCorridorAnalysis<'prerequisite, 'ef, 'exact, 'pose>,
    ExactEFiniteHingeCorridorError,
> {
    let limits = limits.projected();
    let mut work = ExactEFiniteHingeCorridorWork::default();
    let mut meter = WorkMeter::new(&limits.exact);
    let result = calculate_exact_e_finite_hinge_corridor_v1(
        prerequisite_analysis,
        ef_boundary,
        exact,
        bound,
        paper_thickness_mm,
        &limits,
        &mut work,
        &mut meter,
    );
    work.exact = meter.work;
    match result {
        Ok(result) => Ok(ExactEFiniteHingeCorridorAnalysis { result, work }),
        Err(CayleyError::ResourceLimitExceeded { .. }) => {
            Err(ExactEFiniteHingeCorridorError::ResourceLimitExceeded)
        }
        Err(_) => Ok(ExactEFiniteHingeCorridorAnalysis {
            result: ExactEFiniteHingeCorridorResult::Unresolved,
            work,
        }),
    }
}

#[allow(clippy::too_many_arguments)]
fn calculate_exact_e_finite_hinge_corridor_v1<'prerequisite, 'ef, 'exact, 'pose>(
    prerequisite_analysis: &'prerequisite SingleTriangularHingePrerequisiteAnalysis<'exact, 'pose>,
    ef_boundary: Option<&'ef AxisAlignedEfBoundaryCapabilityV1<'prerequisite, 'exact, 'pose>>,
    exact: &'exact RationalCayleyTreePose<'pose>,
    bound: BoundMaterialTreePose<'pose>,
    paper_thickness_mm: f64,
    limits: &ExactEFiniteHingeCorridorLimits,
    work: &mut ExactEFiniteHingeCorridorWork,
    meter: &mut WorkMeter<'_>,
) -> Result<ExactEFiniteHingeCorridorResult<'prerequisite, 'ef, 'exact, 'pose>, CayleyError> {
    let prerequisite = match &prerequisite_analysis.result {
        SingleTriangularHingePrerequisiteResult::LayerOffsetUnmodeled => {
            return Ok(ExactEFiniteHingeCorridorResult::LayerOffsetUnmodeled);
        }
        SingleTriangularHingePrerequisiteResult::Unresolved => {
            return Ok(ExactEFiniteHingeCorridorResult::Unresolved);
        }
        SingleTriangularHingePrerequisiteResult::Authenticated(capability) => capability,
    };
    let Some(ef_boundary) = ef_boundary else {
        return Ok(ExactEFiniteHingeCorridorResult::Unresolved);
    };
    if !positive_finite_binary64(paper_thickness_mm)
        || exact.version != RATIONAL_CAYLEY_TREE_POSE_V1
        || !exact.is_for(bound)
        || bound.model() != exact.bound.model()
        || !bound.pose().same_instance(exact.bound.pose())
        || revalidate_single_triangular_hinge_prerequisites_v1(
            prerequisite,
            exact,
            paper_thickness_mm,
        )
        .is_none()
        || revalidate_axis_aligned_ef_boundary_v1(
            ef_boundary,
            prerequisite,
            exact,
            bound,
            paper_thickness_mm,
        )
        .is_none()
    {
        return Ok(ExactEFiniteHingeCorridorResult::Unresolved);
    }

    charge_fixed_work(work, limits)?;
    let left_face_index = prerequisite.left_face_index;
    let right_face_index = prerequisite.right_face_index;
    let hinge_index = prerequisite.hinge_index;
    if left_face_index == right_face_index
        || exact.faces.len() != FACE_COUNT
        || exact.hinges.len() != HINGE_COUNT
        || left_face_index >= FACE_COUNT
        || right_face_index >= FACE_COUNT
        || hinge_index >= HINGE_COUNT
    {
        return Ok(ExactEFiniteHingeCorridorResult::Unresolved);
    }
    let left_face = &exact.faces[left_face_index];
    let right_face = &exact.faces[right_face_index];
    let exact_hinge = &exact.hinges[hinge_index];
    let Some(left_mid_surface) = triangular_mid_surface(left_face) else {
        return Ok(ExactEFiniteHingeCorridorResult::Unresolved);
    };
    let Some(right_mid_surface) = triangular_mid_surface(right_face) else {
        return Ok(ExactEFiniteHingeCorridorResult::Unresolved);
    };

    let geometry = match reconstruct_corridor_geometry(
        exact,
        left_face_index,
        right_face_index,
        hinge_index,
        paper_thickness_mm,
        meter,
    )? {
        CorridorGeometryResult::Ready(geometry) => *geometry,
        CorridorGeometryResult::LayerOffsetUnmodeled => {
            return Ok(ExactEFiniteHingeCorridorResult::LayerOffsetUnmodeled);
        }
        CorridorGeometryResult::Unresolved => {
            return Ok(ExactEFiniteHingeCorridorResult::Unresolved);
        }
    };

    let first = ExactTriangularPrismView {
        mid_surface: left_mid_surface,
        material_normal: &geometry.left_normal,
        half_thickness: &geometry.half_thickness,
    };
    let second = ExactTriangularPrismView {
        mid_surface: right_mid_surface,
        material_normal: &geometry.right_normal,
        half_thickness: &geometry.half_thickness,
    };
    let Some(report) = analyze_exact_prism_pair_with_meter_v1(
        first,
        second,
        limits.prism,
        &mut work.prism,
        meter,
    )?
    else {
        return Ok(ExactEFiniteHingeCorridorResult::Unresolved);
    };
    let Some(interaction_kind) = authenticate_interaction_kind(&report) else {
        return Ok(ExactEFiniteHingeCorridorResult::Unresolved);
    };
    let outside = scan_complete_intersection_against_corridor(
        &report,
        interaction_kind,
        &exact_hinge.world_endpoints[0],
        &geometry,
        limits,
        work,
        meter,
    )?;
    if let Some(outside) = outside {
        return Ok(ExactEFiniteHingeCorridorResult::Outside(outside));
    }

    Ok(ExactEFiniteHingeCorridorResult::Contained(Box::new(
        ExactEFiniteHingeCorridorCapabilityV1 {
            prerequisite,
            ef_boundary,
            exact,
            bound,
            paper_thickness_bits: paper_thickness_mm.to_bits(),
            left_face_index,
            right_face_index,
            hinge_index,
            interaction_kind,
            geometry,
        },
    )))
}

fn triangular_mid_surface(face: &ExactFacePose) -> Option<[&ExactPoint3; 3]> {
    if face.boundary.len() != 3 {
        return None;
    }
    Some([
        &face.boundary[0].1,
        &face.boundary[1].1,
        &face.boundary[2].1,
    ])
}

enum CorridorGeometryResult {
    Ready(Box<ExactEFiniteHingeCorridorGeometry>),
    LayerOffsetUnmodeled,
    Unresolved,
}

fn reconstruct_corridor_geometry(
    exact: &RationalCayleyTreePose<'_>,
    left_face_index: usize,
    right_face_index: usize,
    hinge_index: usize,
    paper_thickness_mm: f64,
    meter: &mut WorkMeter<'_>,
) -> Result<CorridorGeometryResult, CayleyError> {
    let Some(left_face) = exact.faces.get(left_face_index) else {
        return Ok(CorridorGeometryResult::Unresolved);
    };
    let Some(right_face) = exact.faces.get(right_face_index) else {
        return Ok(CorridorGeometryResult::Unresolved);
    };
    let Some(hinge) = exact.hinges.get(hinge_index) else {
        return Ok(CorridorGeometryResult::Unresolved);
    };
    let zero = BigRational::zero();
    let one = BigRational::one();
    let two = BigRational::from_integer(2.into());

    // Lift t first and divide in exact arithmetic.  `t/2` in binary64 would
    // underflow for the minimum positive subnormal.
    let thickness = exact_f64(paper_thickness_mm, meter, STAGE)?;
    let half_thickness = meter.divide_rational(&thickness, &two, STAGE)?;
    if meter.compare_rational(&half_thickness, &zero, STAGE)? != Ordering::Greater {
        return Ok(CorridorGeometryResult::Unresolved);
    }

    let axis = exact_between(&hinge.world_endpoints[0], &hinge.world_endpoints[1], meter)?;
    let length_squared = exact_dot(&axis, &axis, meter)?;
    if meter.compare_rational(&length_squared, &zero, STAGE)? != Ordering::Greater {
        return Ok(CorridorGeometryResult::Unresolved);
    }
    let left_normal = exact_local_y(&left_face.transform, meter)?;
    let right_normal = exact_local_y(&right_face.transform, meter)?;
    let left_norm_squared = exact_dot(&left_normal, &left_normal, meter)?;
    let right_norm_squared = exact_dot(&right_normal, &right_normal, meter)?;
    if meter.compare_rational(&left_norm_squared, &one, STAGE)? != Ordering::Equal
        || meter.compare_rational(&right_norm_squared, &one, STAGE)? != Ordering::Equal
    {
        return Ok(CorridorGeometryResult::Unresolved);
    }
    let left_axis_dot = exact_dot(&axis, &left_normal, meter)?;
    let right_axis_dot = exact_dot(&axis, &right_normal, meter)?;
    if meter.compare_rational(&left_axis_dot, &zero, STAGE)? != Ordering::Equal
        || meter.compare_rational(&right_axis_dot, &zero, STAGE)? != Ordering::Equal
    {
        return Ok(CorridorGeometryResult::Unresolved);
    }

    let normal_dot = exact_dot(&left_normal, &right_normal, meter)?;
    let one_plus_dot = meter.add_rational(&one, &normal_dot, STAGE)?;
    let cosine_half_squared = meter.divide_rational(&one_plus_dot, &two, STAGE)?;
    let numerical_flat_fold_limit = exact_f64(f64::EPSILON, meter, STAGE)?;
    let half_thickness_squared =
        meter.multiply_rational(&half_thickness, &half_thickness, STAGE)?;
    let radial_limit_product = match classify_corridor_scalars(
        &half_thickness_squared,
        &length_squared,
        &cosine_half_squared,
        &numerical_flat_fold_limit,
        meter,
    )? {
        CorridorScalarResult::Ready {
            radial_limit_product,
        } => radial_limit_product,
        CorridorScalarResult::LayerOffsetUnmodeled => {
            return Ok(CorridorGeometryResult::LayerOffsetUnmodeled);
        }
        CorridorScalarResult::Unresolved => {
            return Ok(CorridorGeometryResult::Unresolved);
        }
    };
    Ok(CorridorGeometryResult::Ready(Box::new(
        ExactEFiniteHingeCorridorGeometry {
            axis,
            length_squared,
            half_thickness,
            cosine_half_squared,
            radial_limit_product,
            left_normal,
            right_normal,
        },
    )))
}

enum CorridorScalarResult {
    Ready { radial_limit_product: BigRational },
    LayerOffsetUnmodeled,
    Unresolved,
}

fn classify_corridor_scalars(
    half_thickness_squared: &BigRational,
    length_squared: &BigRational,
    cosine_half_squared: &BigRational,
    numerical_flat_fold_limit: &BigRational,
    meter: &mut WorkMeter<'_>,
) -> Result<CorridorScalarResult, CayleyError> {
    let zero = BigRational::zero();
    let one = BigRational::one();
    if meter.compare_rational(cosine_half_squared, &zero, STAGE)? == Ordering::Less
        || meter.compare_rational(cosine_half_squared, &one, STAGE)? == Ordering::Greater
        || meter.compare_rational(half_thickness_squared, &zero, STAGE)? != Ordering::Greater
        || meter.compare_rational(length_squared, &zero, STAGE)? != Ordering::Greater
    {
        return Ok(CorridorScalarResult::Unresolved);
    }
    if meter.compare_rational(cosine_half_squared, numerical_flat_fold_limit, STAGE)?
        != Ordering::Greater
    {
        return Ok(CorridorScalarResult::LayerOffsetUnmodeled);
    }
    let finite_axis_bound = meter.multiply_rational(length_squared, cosine_half_squared, STAGE)?;
    if meter.compare_rational(half_thickness_squared, &finite_axis_bound, STAGE)?
        == Ordering::Greater
    {
        return Ok(CorridorScalarResult::LayerOffsetUnmodeled);
    }
    Ok(CorridorScalarResult::Ready {
        radial_limit_product: meter.multiply_rational(
            half_thickness_squared,
            length_squared,
            STAGE,
        )?,
    })
}

fn authenticate_interaction_kind(
    report: &ExactPrismIntersectionReport,
) -> Option<ExactEFiniteHingeInteractionKind> {
    match (
        report.kind(),
        report.affine_rank(),
        report.opposing_support(),
        report.canonical_vertices().len(),
    ) {
        (ExactPrismIntersectionKind::PositiveVolume, Some(3), None, vertex_count)
            if vertex_count >= 4 =>
        {
            Some(ExactEFiniteHingeInteractionKind::PositiveVolume)
        }
        (ExactPrismIntersectionKind::CoplanarArea, Some(2), Some(witness), vertex_count)
            if vertex_count >= 3
                && witness.first_prism_facet_index() < 5
                && witness.second_prism_facet_index() < 5 =>
        {
            Some(ExactEFiniteHingeInteractionKind::BoundaryAreaContact)
        }
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct CorridorViolation {
    axial_before_start: bool,
    axial_after_end: bool,
    radial_outside: bool,
}

impl CorridorViolation {
    fn any(self) -> bool {
        self.axial_before_start || self.axial_after_end || self.radial_outside
    }
}

fn scan_complete_intersection_against_corridor(
    report: &ExactPrismIntersectionReport,
    interaction_kind: ExactEFiniteHingeInteractionKind,
    axis_start: &ExactPoint3,
    geometry: &ExactEFiniteHingeCorridorGeometry,
    limits: &ExactEFiniteHingeCorridorLimits,
    work: &mut ExactEFiniteHingeCorridorWork,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<ExactEFiniteHingeCorridorOutside>, CayleyError> {
    scan_vertices_against_corridor(
        report.canonical_vertices(),
        interaction_kind,
        axis_start,
        geometry,
        limits,
        work,
        meter,
    )
}

fn scan_vertices_against_corridor(
    vertices: &[ExactPoint3],
    interaction_kind: ExactEFiniteHingeInteractionKind,
    axis_start: &ExactPoint3,
    geometry: &ExactEFiniteHingeCorridorGeometry,
    limits: &ExactEFiniteHingeCorridorLimits,
    work: &mut ExactEFiniteHingeCorridorWork,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<ExactEFiniteHingeCorridorOutside>, CayleyError> {
    let zero = BigRational::zero();
    let tests_before = work.corridor_vertex_tests;
    let mut first_outside_vertex_index = None;
    let mut outside_vertex_count = 0_usize;
    let mut aggregate = CorridorViolation::default();
    for (vertex_index, vertex) in vertices.iter().enumerate() {
        charge_counter(
            &mut work.corridor_vertex_tests,
            limits.max_corridor_vertex_tests,
            "exact_e_corridor_vertex_tests",
        )?;
        let violation = classify_corridor_vertex(
            vertex,
            axis_start,
            &geometry.axis,
            &geometry.length_squared,
            &geometry.cosine_half_squared,
            &geometry.radial_limit_product,
            &zero,
            meter,
        )?;
        if violation.any() {
            first_outside_vertex_index.get_or_insert(vertex_index);
            outside_vertex_count =
                outside_vertex_count
                    .checked_add(1)
                    .ok_or(CayleyError::ResourceLimitExceeded {
                        stage: STAGE,
                        resource: "exact_e_corridor_outside_vertices",
                    })?;
            aggregate.axial_before_start |= violation.axial_before_start;
            aggregate.axial_after_end |= violation.axial_after_end;
            aggregate.radial_outside |= violation.radial_outside;
        }
    }
    let expected_tests =
        tests_before
            .checked_add(vertices.len())
            .ok_or(CayleyError::ResourceLimitExceeded {
                stage: STAGE,
                resource: "exact_e_corridor_vertex_tests",
            })?;
    if work.corridor_vertex_tests != expected_tests {
        return Err(CayleyError::InvariantFailure { stage: STAGE });
    }
    Ok(
        first_outside_vertex_index.map(|first_outside_vertex_index| {
            ExactEFiniteHingeCorridorOutside {
                interaction_kind,
                first_outside_vertex_index,
                outside_vertex_count,
                axial_before_start: aggregate.axial_before_start,
                axial_after_end: aggregate.axial_after_end,
                radial_outside: aggregate.radial_outside,
            }
        }),
    )
}

#[allow(clippy::too_many_arguments)]
fn classify_corridor_vertex(
    vertex: &ExactPoint3,
    axis_start: &ExactPoint3,
    axis: &ExactVector3,
    length_squared: &BigRational,
    cosine_half_squared: &BigRational,
    radial_limit_product: &BigRational,
    zero: &BigRational,
    meter: &mut WorkMeter<'_>,
) -> Result<CorridorViolation, CayleyError> {
    let relative = exact_between(axis_start, vertex, meter)?;
    let axial = exact_dot(&relative, axis, meter)?;
    let cross = exact_cross(&relative, axis, meter)?;
    let radial_squared = exact_dot(&cross, &cross, meter)?;
    let radial_left = meter.multiply_rational(cosine_half_squared, &radial_squared, STAGE)?;
    Ok(CorridorViolation {
        axial_before_start: meter.compare_rational(&axial, zero, STAGE)? == Ordering::Less,
        axial_after_end: meter.compare_rational(&axial, length_squared, STAGE)?
            == Ordering::Greater,
        radial_outside: meter.compare_rational(&radial_left, radial_limit_product, STAGE)?
            == Ordering::Greater,
    })
}

fn exact_cross(
    first: &ExactVector3,
    second: &ExactVector3,
    meter: &mut WorkMeter<'_>,
) -> Result<ExactVector3, CayleyError> {
    const COMPONENTS: [(usize, usize, usize, usize); 3] =
        [(1, 2, 2, 1), (2, 0, 0, 2), (0, 1, 1, 0)];
    Ok(ExactVector3 {
        coordinates: try_array3(|axis| {
            let (first_left, second_left, first_right, second_right) = COMPONENTS[axis];
            let left = meter.multiply_rational(
                &first.coordinates[first_left],
                &second.coordinates[second_left],
                STAGE,
            )?;
            let right = meter.multiply_rational(
                &first.coordinates[first_right],
                &second.coordinates[second_right],
                STAGE,
            )?;
            meter.subtract_rational(&left, &right, STAGE)
        })?,
    })
}

fn charge_fixed_work(
    work: &mut ExactEFiniteHingeCorridorWork,
    limits: &ExactEFiniteHingeCorridorLimits,
) -> Result<(), CayleyError> {
    for (counter, required, maximum, resource) in [
        (
            &mut work.authenticated_faces,
            FACE_COUNT,
            limits.max_authenticated_faces,
            "exact_e_corridor_faces",
        ),
        (
            &mut work.authenticated_hinges,
            HINGE_COUNT,
            limits.max_authenticated_hinges,
            "exact_e_corridor_hinges",
        ),
        (
            &mut work.local_y_component_lifts,
            LOCAL_Y_COMPONENT_LIFTS,
            limits.max_local_y_component_lifts,
            "exact_e_corridor_local_y_lifts",
        ),
        (
            &mut work.thickness_lifts,
            THICKNESS_LIFTS,
            limits.max_thickness_lifts,
            "exact_e_corridor_thickness_lifts",
        ),
        (
            &mut work.half_thickness_divisions,
            HALF_THICKNESS_DIVISIONS,
            limits.max_half_thickness_divisions,
            "exact_e_corridor_half_thickness_divisions",
        ),
        (
            &mut work.scalar_reconstructions,
            SCALAR_RECONSTRUCTIONS,
            limits.max_scalar_reconstructions,
            "exact_e_corridor_scalar_reconstructions",
        ),
    ] {
        set_fixed_counter(counter, required, maximum, resource)?;
    }
    Ok(())
}

fn set_fixed_counter(
    counter: &mut usize,
    required: usize,
    maximum: usize,
    resource: &'static str,
) -> Result<(), CayleyError> {
    if *counter != 0 {
        return Err(CayleyError::InvariantFailure { stage: STAGE });
    }
    if required > maximum {
        return Err(CayleyError::ResourceLimitExceeded {
            stage: STAGE,
            resource,
        });
    }
    *counter = required;
    Ok(())
}

fn charge_counter(
    counter: &mut usize,
    maximum: usize,
    resource: &'static str,
) -> Result<(), CayleyError> {
    let next = counter
        .checked_add(1)
        .ok_or(CayleyError::ResourceLimitExceeded {
            stage: STAGE,
            resource,
        })?;
    if next > maximum {
        return Err(CayleyError::ResourceLimitExceeded {
            stage: STAGE,
            resource,
        });
    }
    *counter = next;
    Ok(())
}

/// Revalidates every authority binding before a future private phase may
/// consume the exact-E corridor capability.
#[allow(clippy::too_many_arguments)]
pub(super) fn revalidate_exact_e_finite_hinge_corridor_v1<
    'capability,
    'prerequisite,
    'ef,
    'exact,
    'pose,
>(
    capability: &'capability ExactEFiniteHingeCorridorCapabilityV1<
        'prerequisite,
        'ef,
        'exact,
        'pose,
    >,
    prerequisite: &AuthenticatedSingleTriangularHingePrerequisitesV1<'_, '_>,
    ef_boundary: &AxisAlignedEfBoundaryCapabilityV1<'_, '_, '_>,
    exact: &RationalCayleyTreePose<'_>,
    bound: BoundMaterialTreePose<'_>,
    paper_thickness_mm: f64,
) -> Option<
    RevalidatedExactEFiniteHingeCorridorCapabilityV1<
        'capability,
        'prerequisite,
        'ef,
        'exact,
        'pose,
    >,
> {
    if !positive_finite_binary64(paper_thickness_mm)
        || !std::ptr::eq(capability.prerequisite, prerequisite)
        || !std::ptr::eq(capability.ef_boundary, ef_boundary)
        || !std::ptr::eq(capability.exact, exact)
        || capability.paper_thickness_bits != paper_thickness_mm.to_bits()
        || capability.bound.model() != bound.model()
        || !capability.bound.pose().same_instance(bound.pose())
        || !exact.is_for(bound)
        || capability.left_face_index != prerequisite.left_face_index
        || capability.right_face_index != prerequisite.right_face_index
        || capability.hinge_index != prerequisite.hinge_index
        || revalidate_single_triangular_hinge_prerequisites_v1(
            prerequisite,
            exact,
            paper_thickness_mm,
        )
        .is_none()
        || revalidate_axis_aligned_ef_boundary_v1(
            ef_boundary,
            prerequisite,
            exact,
            bound,
            paper_thickness_mm,
        )
        .is_none()
    {
        return None;
    }
    Some(RevalidatedExactEFiniteHingeCorridorCapabilityV1 { capability })
}

#[cfg(test)]
mod tests {
    use num_bigint::BigInt;

    use super::*;

    fn integer(value: i64) -> BigRational {
        BigRational::from_integer(BigInt::from(value))
    }

    fn point(x: i64, y: i64, z: i64) -> ExactPoint3 {
        ExactPoint3 {
            coordinates: [integer(x), integer(y), integer(z)],
        }
    }

    fn vector(x: i64, y: i64, z: i64) -> ExactVector3 {
        ExactVector3 {
            coordinates: [integer(x), integer(y), integer(z)],
        }
    }

    #[test]
    fn closed_axis_endpoints_radial_boundary_and_r_equals_l_are_accepted() {
        let limits = exact_e_corridor_local_hard_cayley_limits();
        let mut meter = WorkMeter::new(&limits);
        let start = point(0, 0, 0);
        let axis = vector(1, 0, 0);
        let length_squared = integer(1);
        let cosine_half_squared = integer(1);
        let radial_limit_product = integer(1);
        let zero = integer(0);

        for vertex in [point(0, 1, 0), point(1, 1, 0)] {
            let violation = classify_corridor_vertex(
                &vertex,
                &start,
                &axis,
                &length_squared,
                &cosine_half_squared,
                &radial_limit_product,
                &zero,
                &mut meter,
            )
            .unwrap();
            assert!(!violation.any());
        }
    }

    #[test]
    fn strict_axial_and_radial_outside_are_distinguished() {
        let limits = exact_e_corridor_local_hard_cayley_limits();
        let mut meter = WorkMeter::new(&limits);
        let start = point(0, 0, 0);
        let axis = vector(1, 0, 0);
        let length_squared = integer(1);
        let cosine_half_squared = integer(1);
        let radial_limit_product = integer(1);
        let zero = integer(0);
        let cases = [
            (point(-1, 0, 0), (true, false, false)),
            (point(2, 0, 0), (false, true, false)),
            (point(0, 2, 0), (false, false, true)),
        ];
        for (vertex, expected) in cases {
            let observed = classify_corridor_vertex(
                &vertex,
                &start,
                &axis,
                &length_squared,
                &cosine_half_squared,
                &radial_limit_product,
                &zero,
                &mut meter,
            )
            .unwrap();
            assert_eq!(
                (
                    observed.axial_before_start,
                    observed.axial_after_end,
                    observed.radial_outside,
                ),
                expected
            );
        }
    }

    #[test]
    fn first_outside_vertex_does_not_short_circuit_complete_scan() {
        let exact_limits = exact_e_corridor_combined_hard_cayley_limits();
        let mut meter = WorkMeter::new(&exact_limits);
        let limits = ExactEFiniteHingeCorridorLimits::default();
        let geometry = ExactEFiniteHingeCorridorGeometry {
            axis: vector(1, 0, 0),
            length_squared: integer(1),
            half_thickness: integer(1),
            cosine_half_squared: integer(1),
            radial_limit_product: integer(1),
            left_normal: vector(0, 1, 0),
            right_normal: vector(0, 1, 0),
        };
        let vertices = [
            point(-1, 0, 0),
            point(0, 0, 0),
            point(1, 1, 0),
            point(2, 0, 0),
            point(0, 2, 0),
        ];
        let preexisting_tests = 2;
        let mut work = ExactEFiniteHingeCorridorWork {
            corridor_vertex_tests: preexisting_tests,
            ..ExactEFiniteHingeCorridorWork::default()
        };
        let outside = scan_vertices_against_corridor(
            &vertices,
            ExactEFiniteHingeInteractionKind::PositiveVolume,
            &point(0, 0, 0),
            &geometry,
            &limits,
            &mut work,
            &mut meter,
        )
        .unwrap()
        .expect("three strict outside vertices");
        assert_eq!(
            work.corridor_vertex_tests,
            preexisting_tests + vertices.len()
        );
        assert_eq!(outside.first_outside_vertex_index, 0);
        assert_eq!(outside.outside_vertex_count, 3);
        assert!(outside.axial_before_start);
        assert!(outside.axial_after_end);
        assert!(outside.radial_outside);
    }

    #[test]
    fn scalar_gate_rejects_epsilon_and_r_greater_l_but_accepts_r_equals_l() {
        let limits = exact_e_corridor_local_hard_cayley_limits();
        let epsilon = BigRational::new(BigInt::from(1), BigInt::from(1_u64 << 52));

        let mut epsilon_meter = WorkMeter::new(&limits);
        assert!(matches!(
            classify_corridor_scalars(
                &integer(1),
                &integer(1),
                &epsilon,
                &epsilon,
                &mut epsilon_meter,
            )
            .unwrap(),
            CorridorScalarResult::LayerOffsetUnmodeled
        ));

        let mut equality_meter = WorkMeter::new(&limits);
        let equality = classify_corridor_scalars(
            &integer(1),
            &integer(2),
            &BigRational::new(BigInt::from(1), BigInt::from(2)),
            &epsilon,
            &mut equality_meter,
        )
        .unwrap();
        let CorridorScalarResult::Ready {
            radial_limit_product,
        } = equality
        else {
            panic!("h² = D²c² is the closed R=L boundary");
        };
        assert_eq!(radial_limit_product, integer(2));

        let mut outside_meter = WorkMeter::new(&limits);
        assert!(matches!(
            classify_corridor_scalars(
                &integer(2),
                &integer(2),
                &BigRational::new(BigInt::from(1), BigInt::from(2)),
                &epsilon,
                &mut outside_meter,
            )
            .unwrap(),
            CorridorScalarResult::LayerOffsetUnmodeled
        ));
    }

    #[test]
    fn combined_exact_limits_follow_cayley_work_additive_and_maximum_semantics() {
        let corridor = exact_e_corridor_local_hard_cayley_limits();
        let prism = exact_prism_hard_cayley_limits();
        let combined = exact_e_corridor_combined_hard_cayley_limits();
        for (observed, expected) in [
            (
                combined.max_interval_operations,
                corridor.max_interval_operations + prism.max_interval_operations,
            ),
            (
                combined.max_gcd_fallback_calls,
                corridor.max_gcd_fallback_calls + prism.max_gcd_fallback_calls,
            ),
            (
                combined.max_gcd_fallback_input_bits,
                corridor.max_gcd_fallback_input_bits + prism.max_gcd_fallback_input_bits,
            ),
            (
                combined.max_rational_allocations,
                corridor.max_rational_allocations + prism.max_rational_allocations,
            ),
            (
                combined.max_total_rational_allocation_bits,
                corridor.max_total_rational_allocation_bits
                    + prism.max_total_rational_allocation_bits,
            ),
        ] {
            assert_eq!(observed, expected);
        }
        for (observed, expected) in [
            (
                combined.max_machin_terms_per_series,
                corridor
                    .max_machin_terms_per_series
                    .max(prism.max_machin_terms_per_series),
            ),
            (
                combined.max_trig_terms_per_series,
                corridor
                    .max_trig_terms_per_series
                    .max(prism.max_trig_terms_per_series),
            ),
            (
                combined.max_sqrt_refinements,
                corridor
                    .max_sqrt_refinements
                    .max(prism.max_sqrt_refinements),
            ),
            (
                combined.max_shift_bits,
                corridor.max_shift_bits.max(prism.max_shift_bits),
            ),
            (
                combined.max_intermediate_bits,
                corridor
                    .max_intermediate_bits
                    .max(prism.max_intermediate_bits),
            ),
            (
                combined.max_rational_allocation_bits,
                corridor
                    .max_rational_allocation_bits
                    .max(prism.max_rational_allocation_bits),
            ),
            (
                combined.max_output_bits,
                corridor.max_output_bits.max(prism.max_output_bits),
            ),
        ] {
            assert_eq!(observed, expected);
        }
    }

    #[test]
    fn counter_overflow_is_checked() {
        let mut counter = usize::MAX;
        assert!(matches!(
            charge_counter(&mut counter, usize::MAX, "overflow"),
            Err(CayleyError::ResourceLimitExceeded { .. })
        ));
        assert_eq!(counter, usize::MAX);
    }
}
