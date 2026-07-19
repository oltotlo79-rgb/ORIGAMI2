//! Private phase-2C proof for the direct-lift binary64 affine pose `F`.
//!
//! This phase consumes and reauthenticates the phase-1 finite-hinge token,
//! the E/F coefficient binding, and the phase-2B exact-`E` corridor token.
//! It evaluates the sealed binary64 affine coefficients over canonical rest
//! points in exact rational arithmetic, constructs the two resulting affine
//! triangular prisms, and scans their complete exact intersection against a
//! finite corridor reconstructed from `F` itself.
//!
//! A non-cardinal binary64 rotation is generally not exactly orthogonal after
//! its coefficients are lifted to rationals.  Consequently this module does
//! not weaken the canonical-`E` prism constructor.  It authenticates all six
//! `mid_surface +/- h*n` vertices here and uses the separate private prebuilt
//! prism entry.  No E/F component error box participates in either prism or
//! corridor geometry.  This remains disconnected from production collision
//! classification, safe-set authority, persistence, DTOs, and mutation.

use std::cmp::Ordering;

use num_rational::BigRational;
use num_traits::{One, Zero};
use ori_domain::FaceId;
use ori_kinematics::{BoundMaterialTreePose, RigidTransform};

use super::ef_boundary::{
    AxisAlignedEfBoundaryCapabilityV1, BoundBinary64FaceTransformBits,
    revalidate_axis_aligned_ef_boundary_v1,
};
use super::exact_e_corridor::{
    ExactEFiniteHingeCorridorAnalysis, ExactEFiniteHingeCorridorCapabilityV1,
    ExactEFiniteHingeCorridorLimits, ExactEFiniteHingeCorridorResult,
    ExactEFiniteHingeCorridorWork, ExactEFiniteHingeInteractionKind,
    revalidate_exact_e_finite_hinge_corridor_v1,
};
use super::exact_prism::{
    ExactPrebuiltTriangularPrismView, ExactPrismIntersectionKind, ExactPrismIntersectionReport,
    ExactPrismLimits, ExactPrismWork, analyze_exact_prebuilt_prism_pair_with_meter_v1,
    exact_prism_hard_cayley_limits,
};
use super::*;

const FACE_COUNT: usize = 2;
const HINGE_COUNT: usize = 1;
const FACE_TRANSFORM_BIT_BINDINGS: usize = FACE_COUNT * 12;
const HINGE_PARENT_TRANSFORM_BIT_BINDINGS: usize = 12;
const TRANSFORM_SCALAR_LIFTS: usize = FACE_COUNT * 12;
const SOURCE_COORDINATE_LIFTS: usize = FACE_COUNT * 3 * 3 + 2 * 3;
const AFFINE_POINT_RECONSTRUCTIONS: usize = FACE_COUNT * 3 + 2;
const MATERIAL_NORMAL_COMPONENT_LIFTS: usize = FACE_COUNT * 3;
const SOLID_VERTEX_CONSTRUCTIONS: usize = FACE_COUNT * 6;
const THICKNESS_LIFTS: usize = 1;
const HALF_THICKNESS_DIVISIONS: usize = 1;
const SCALAR_RECONSTRUCTIONS: usize = 1;
const MAX_CORRIDOR_VERTEX_TESTS: usize = 120;

fn direct_f_corridor_local_hard_cayley_limits() -> CayleyLimits {
    let exact = CayleyLimits::default();
    CayleyLimits {
        max_precision_rounds: 0,
        max_guard_bits: 0,
        max_candidate_bits: 0,
        max_machin_terms_per_series: 0,
        max_trig_terms_per_series: 0,
        max_sqrt_refinements: 0,
        max_interval_operations: 16_384,
        max_shift_bits: exact.max_shift_bits,
        max_intermediate_bits: exact.max_intermediate_bits,
        max_gcd_fallback_calls: 4_096,
        max_gcd_fallback_input_bits: exact.max_gcd_fallback_input_bits,
        max_rational_allocations: 16_384,
        max_rational_allocation_bits: exact.max_rational_allocation_bits,
        max_total_rational_allocation_bits: 134_217_728,
        max_output_bits: 0,
    }
}

fn direct_f_corridor_combined_hard_cayley_limits() -> CayleyLimits {
    let prior = ExactEFiniteHingeCorridorLimits::default().exact;
    let local = direct_f_corridor_local_hard_cayley_limits();
    let prism = exact_prism_hard_cayley_limits();
    let sum = |first: usize, second: usize, third: usize, resource: &'static str| {
        let first_two = checked_hard_limit_sum(first, second, resource)
            .expect("phase-2C exact hard-limit constants must fit usize");
        checked_hard_limit_sum(first_two, third, resource)
            .expect("phase-2C exact hard-limit constants must fit usize")
    };
    CayleyLimits {
        max_precision_rounds: prior
            .max_precision_rounds
            .max(local.max_precision_rounds)
            .max(prism.max_precision_rounds),
        max_guard_bits: prior
            .max_guard_bits
            .max(local.max_guard_bits)
            .max(prism.max_guard_bits),
        max_candidate_bits: prior
            .max_candidate_bits
            .max(local.max_candidate_bits)
            .max(prism.max_candidate_bits),
        max_machin_terms_per_series: prior
            .max_machin_terms_per_series
            .max(local.max_machin_terms_per_series)
            .max(prism.max_machin_terms_per_series),
        max_trig_terms_per_series: prior
            .max_trig_terms_per_series
            .max(local.max_trig_terms_per_series)
            .max(prism.max_trig_terms_per_series),
        max_sqrt_refinements: prior
            .max_sqrt_refinements
            .max(local.max_sqrt_refinements)
            .max(prism.max_sqrt_refinements),
        max_interval_operations: sum(
            prior.max_interval_operations,
            local.max_interval_operations,
            prism.max_interval_operations,
            "direct_f_corridor_combined_interval_operations",
        ),
        max_shift_bits: prior
            .max_shift_bits
            .max(local.max_shift_bits)
            .max(prism.max_shift_bits),
        max_intermediate_bits: prior
            .max_intermediate_bits
            .max(local.max_intermediate_bits)
            .max(prism.max_intermediate_bits),
        max_gcd_fallback_calls: sum(
            prior.max_gcd_fallback_calls,
            local.max_gcd_fallback_calls,
            prism.max_gcd_fallback_calls,
            "direct_f_corridor_combined_gcd_calls",
        ),
        max_gcd_fallback_input_bits: sum(
            prior.max_gcd_fallback_input_bits,
            local.max_gcd_fallback_input_bits,
            prism.max_gcd_fallback_input_bits,
            "direct_f_corridor_combined_gcd_input_bits",
        ),
        max_rational_allocations: sum(
            prior.max_rational_allocations,
            local.max_rational_allocations,
            prism.max_rational_allocations,
            "direct_f_corridor_combined_allocations",
        ),
        max_rational_allocation_bits: prior
            .max_rational_allocation_bits
            .max(local.max_rational_allocation_bits)
            .max(prism.max_rational_allocation_bits),
        max_total_rational_allocation_bits: sum(
            prior.max_total_rational_allocation_bits,
            local.max_total_rational_allocation_bits,
            prism.max_total_rational_allocation_bits,
            "direct_f_corridor_combined_allocation_bits",
        ),
        max_output_bits: prior
            .max_output_bits
            .max(local.max_output_bits)
            .max(prism.max_output_bits),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct DirectFFiniteHingeCorridorLimits {
    pub(super) max_authenticated_faces: usize,
    pub(super) max_authenticated_hinges: usize,
    pub(super) max_face_transform_bit_bindings: usize,
    pub(super) max_hinge_parent_transform_bit_bindings: usize,
    pub(super) max_transform_scalar_lifts: usize,
    pub(super) max_source_coordinate_lifts: usize,
    pub(super) max_affine_point_reconstructions: usize,
    pub(super) max_material_normal_component_lifts: usize,
    pub(super) max_solid_vertex_constructions: usize,
    pub(super) max_thickness_lifts: usize,
    pub(super) max_half_thickness_divisions: usize,
    pub(super) max_scalar_reconstructions: usize,
    pub(super) max_corridor_vertex_tests: usize,
    pub(super) prism: ExactPrismLimits,
    pub(super) local_exact: CayleyLimits,
    pub(super) exact: CayleyLimits,
}

impl Default for DirectFFiniteHingeCorridorLimits {
    fn default() -> Self {
        Self {
            max_authenticated_faces: FACE_COUNT,
            max_authenticated_hinges: HINGE_COUNT,
            max_face_transform_bit_bindings: FACE_TRANSFORM_BIT_BINDINGS,
            max_hinge_parent_transform_bit_bindings: HINGE_PARENT_TRANSFORM_BIT_BINDINGS,
            max_transform_scalar_lifts: TRANSFORM_SCALAR_LIFTS,
            max_source_coordinate_lifts: SOURCE_COORDINATE_LIFTS,
            max_affine_point_reconstructions: AFFINE_POINT_RECONSTRUCTIONS,
            max_material_normal_component_lifts: MATERIAL_NORMAL_COMPONENT_LIFTS,
            max_solid_vertex_constructions: SOLID_VERTEX_CONSTRUCTIONS,
            max_thickness_lifts: THICKNESS_LIFTS,
            max_half_thickness_divisions: HALF_THICKNESS_DIVISIONS,
            max_scalar_reconstructions: SCALAR_RECONSTRUCTIONS,
            max_corridor_vertex_tests: MAX_CORRIDOR_VERTEX_TESTS,
            prism: ExactPrismLimits::default(),
            local_exact: direct_f_corridor_local_hard_cayley_limits(),
            exact: direct_f_corridor_combined_hard_cayley_limits(),
        }
    }
}

impl DirectFFiniteHingeCorridorLimits {
    fn projected(self) -> Self {
        let hard = Self::default();
        Self {
            max_authenticated_faces: self
                .max_authenticated_faces
                .min(hard.max_authenticated_faces),
            max_authenticated_hinges: self
                .max_authenticated_hinges
                .min(hard.max_authenticated_hinges),
            max_face_transform_bit_bindings: self
                .max_face_transform_bit_bindings
                .min(hard.max_face_transform_bit_bindings),
            max_hinge_parent_transform_bit_bindings: self
                .max_hinge_parent_transform_bit_bindings
                .min(hard.max_hinge_parent_transform_bit_bindings),
            max_transform_scalar_lifts: self
                .max_transform_scalar_lifts
                .min(hard.max_transform_scalar_lifts),
            max_source_coordinate_lifts: self
                .max_source_coordinate_lifts
                .min(hard.max_source_coordinate_lifts),
            max_affine_point_reconstructions: self
                .max_affine_point_reconstructions
                .min(hard.max_affine_point_reconstructions),
            max_material_normal_component_lifts: self
                .max_material_normal_component_lifts
                .min(hard.max_material_normal_component_lifts),
            max_solid_vertex_constructions: self
                .max_solid_vertex_constructions
                .min(hard.max_solid_vertex_constructions),
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
            local_exact: project_cayley_limits(self.local_exact, hard.local_exact),
            exact: project_cayley_limits(self.exact, hard.exact),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct DirectFFiniteHingeCorridorWork {
    pub(super) authenticated_faces: usize,
    pub(super) authenticated_hinges: usize,
    pub(super) face_transform_bit_bindings: usize,
    pub(super) hinge_parent_transform_bit_bindings: usize,
    pub(super) transform_scalar_lifts: usize,
    pub(super) source_coordinate_lifts: usize,
    pub(super) affine_point_reconstructions: usize,
    pub(super) material_normal_component_lifts: usize,
    pub(super) solid_vertex_constructions: usize,
    pub(super) thickness_lifts: usize,
    pub(super) half_thickness_divisions: usize,
    pub(super) scalar_reconstructions: usize,
    pub(super) corridor_vertex_tests: usize,
    pub(super) prism: ExactPrismWork,
    pub(super) phase2b_exact: CayleyWork,
    pub(super) local_exact: CayleyWork,
    pub(super) exact: CayleyWork,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DirectFFiniteHingeCorridorError {
    ResourceLimitExceeded,
}

#[derive(Debug)]
struct DirectFCorridorGeometry {
    axis_start: ExactPoint3,
    axis: ExactVector3,
    length_squared: BigRational,
    half_thickness: BigRational,
    cosine_half_squared: BigRational,
    radial_limit_product: BigRational,
    left_normal: ExactVector3,
    right_normal: ExactVector3,
}

#[derive(Debug)]
pub(super) struct DirectFFiniteHingeCorridorCapabilityV1<
    'prerequisite,
    'ef,
    'exact_e_corridor,
    'exact,
    'pose,
> {
    prerequisite: &'prerequisite AuthenticatedSingleTriangularHingePrerequisitesV1<'exact, 'pose>,
    ef_boundary: &'ef AxisAlignedEfBoundaryCapabilityV1<'prerequisite, 'exact, 'pose>,
    exact_e_corridor:
        &'exact_e_corridor ExactEFiniteHingeCorridorCapabilityV1<'prerequisite, 'ef, 'exact, 'pose>,
    exact: &'exact RationalCayleyTreePose<'pose>,
    bound: BoundMaterialTreePose<'pose>,
    paper_thickness_bits: u64,
    left_face_index: usize,
    right_face_index: usize,
    hinge_index: usize,
    interaction_kind: ExactEFiniteHingeInteractionKind,
    pub(super) binary64_face_transforms: [BoundBinary64FaceTransformBits; FACE_COUNT],
    pub(super) hinge_parent_transform: BoundBinary64FaceTransformBits,
    geometry: DirectFCorridorGeometry,
    phase2b_work: ExactEFiniteHingeCorridorWork,
    sealed_work: Option<DirectFFiniteHingeCorridorWork>,
}

impl DirectFFiniteHingeCorridorCapabilityV1<'_, '_, '_, '_, '_> {
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

    pub(super) fn sealed_work(&self) -> Option<&DirectFFiniteHingeCorridorWork> {
        self.sealed_work.as_ref()
    }

    #[cfg(test)]
    pub(super) fn axis_start(&self) -> &ExactPoint3 {
        &self.geometry.axis_start
    }

    #[cfg(test)]
    pub(super) fn axis(&self) -> &ExactVector3 {
        &self.geometry.axis
    }
}

#[derive(Debug)]
pub(super) struct RevalidatedDirectFFiniteHingeCorridorCapabilityV1<
    'capability,
    'prerequisite,
    'ef,
    'exact_e_corridor,
    'exact,
    'pose,
> {
    pub(super) capability: &'capability DirectFFiniteHingeCorridorCapabilityV1<
        'prerequisite,
        'ef,
        'exact_e_corridor,
        'exact,
        'pose,
    >,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DirectFFiniteHingeCorridorOutside {
    pub(super) interaction_kind: ExactEFiniteHingeInteractionKind,
    pub(super) first_outside_vertex_index: usize,
    pub(super) outside_vertex_count: usize,
    pub(super) axial_before_start: bool,
    pub(super) axial_after_end: bool,
    pub(super) radial_outside: bool,
    /// Exact strict excess
    /// `c2*|(x-A)×d|² - h²*|d|²` at the first radial outside vertex.
    pub(super) first_radial_excess: Option<BigRational>,
}

#[derive(Debug)]
pub(super) enum DirectFFiniteHingeCorridorResult<
    'prerequisite,
    'ef,
    'exact_e_corridor,
    'exact,
    'pose,
> {
    Contained(
        Box<
            DirectFFiniteHingeCorridorCapabilityV1<
                'prerequisite,
                'ef,
                'exact_e_corridor,
                'exact,
                'pose,
            >,
        >,
    ),
    Outside(DirectFFiniteHingeCorridorOutside),
    InteractionKindMismatch {
        exact_e: ExactEFiniteHingeInteractionKind,
        direct_f: ExactEFiniteHingeInteractionKind,
    },
    LayerOffsetUnmodeled,
    Unresolved,
}

#[derive(Debug)]
pub(super) struct DirectFFiniteHingeCorridorAnalysis<
    'prerequisite,
    'ef,
    'exact_e_corridor,
    'exact,
    'pose,
> {
    pub(super) result:
        DirectFFiniteHingeCorridorResult<'prerequisite, 'ef, 'exact_e_corridor, 'exact, 'pose>,
    pub(super) work: DirectFFiniteHingeCorridorWork,
}

impl<'prerequisite, 'ef, 'exact_e_corridor, 'exact, 'pose>
    DirectFFiniteHingeCorridorAnalysis<'prerequisite, 'ef, 'exact_e_corridor, 'exact, 'pose>
{
    pub(super) fn authenticated_contained_capability_and_work(
        &self,
    ) -> Option<(
        &DirectFFiniteHingeCorridorCapabilityV1<
            'prerequisite,
            'ef,
            'exact_e_corridor,
            'exact,
            'pose,
        >,
        &DirectFFiniteHingeCorridorWork,
    )> {
        let DirectFFiniteHingeCorridorResult::Contained(capability) = &self.result else {
            return None;
        };
        let sealed_work = capability.sealed_work()?;
        if sealed_work != &self.work {
            return None;
        }
        Some((capability.as_ref(), sealed_work))
    }
}

fn preflight_direct_f_exact_capacity(
    phase2b_work: &ExactEFiniteHingeCorridorWork,
    limits: &DirectFFiniteHingeCorridorLimits,
) -> Result<(), CayleyError> {
    CayleyWork::default().checked_merge(&phase2b_work.exact, &limits.exact, None, STAGE)?;

    for local in [&limits.local_exact, &limits.prism.exact] {
        for (local_maximum, outer_maximum, resource) in [
            (
                local.max_precision_rounds,
                limits.exact.max_precision_rounds,
                "direct_f_reserved_precision_rounds",
            ),
            (
                local.max_guard_bits,
                limits.exact.max_guard_bits,
                "direct_f_reserved_guard_bits",
            ),
            (
                local.max_candidate_bits,
                limits.exact.max_candidate_bits,
                "direct_f_reserved_candidate_bits",
            ),
            (
                local.max_machin_terms_per_series,
                limits.exact.max_machin_terms_per_series,
                "direct_f_reserved_machin_terms",
            ),
            (
                local.max_trig_terms_per_series,
                limits.exact.max_trig_terms_per_series,
                "direct_f_reserved_trig_terms",
            ),
            (
                local.max_sqrt_refinements,
                limits.exact.max_sqrt_refinements,
                "direct_f_reserved_sqrt_refinements",
            ),
            (
                local.max_shift_bits,
                limits.exact.max_shift_bits,
                "direct_f_reserved_shift_bits",
            ),
            (
                local.max_intermediate_bits,
                limits.exact.max_intermediate_bits,
                "direct_f_reserved_intermediate_bits",
            ),
            (
                local.max_rational_allocation_bits,
                limits.exact.max_rational_allocation_bits,
                "direct_f_reserved_rational_allocation_bits",
            ),
            (
                local.max_output_bits,
                limits.exact.max_output_bits,
                "direct_f_reserved_output_bits",
            ),
        ] {
            if local_maximum > outer_maximum {
                return Err(CayleyError::ResourceLimitExceeded {
                    stage: STAGE,
                    resource,
                });
            }
        }
    }

    for (consumed, direct_maximum, prism_maximum, outer_maximum, resource) in [
        (
            phase2b_work.exact.interval_operations,
            limits.local_exact.max_interval_operations,
            limits.prism.exact.max_interval_operations,
            limits.exact.max_interval_operations,
            "direct_f_reserved_interval_operations",
        ),
        (
            phase2b_work.exact.gcd_fallback_calls,
            limits.local_exact.max_gcd_fallback_calls,
            limits.prism.exact.max_gcd_fallback_calls,
            limits.exact.max_gcd_fallback_calls,
            "direct_f_reserved_gcd_fallback_calls",
        ),
        (
            phase2b_work.exact.gcd_fallback_input_bits,
            limits.local_exact.max_gcd_fallback_input_bits,
            limits.prism.exact.max_gcd_fallback_input_bits,
            limits.exact.max_gcd_fallback_input_bits,
            "direct_f_reserved_gcd_fallback_input_bits",
        ),
        (
            phase2b_work.exact.rational_allocations,
            limits.local_exact.max_rational_allocations,
            limits.prism.exact.max_rational_allocations,
            limits.exact.max_rational_allocations,
            "direct_f_reserved_rational_allocations",
        ),
        (
            phase2b_work.exact.total_rational_allocation_bits,
            limits.local_exact.max_total_rational_allocation_bits,
            limits.prism.exact.max_total_rational_allocation_bits,
            limits.exact.max_total_rational_allocation_bits,
            "direct_f_reserved_total_rational_allocation_bits",
        ),
    ] {
        let reserved = consumed
            .checked_add(direct_maximum)
            .and_then(|reserved| reserved.checked_add(prism_maximum))
            .ok_or(CayleyError::ResourceLimitExceeded {
                stage: STAGE,
                resource,
            })?;
        if reserved > outer_maximum {
            return Err(CayleyError::ResourceLimitExceeded {
                stage: STAGE,
                resource,
            });
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn analyze_direct_f_finite_hinge_corridor_v1<
    'prerequisite,
    'ef,
    'exact_e_corridor,
    'exact,
    'pose,
>(
    prerequisite_analysis: &'prerequisite SingleTriangularHingePrerequisiteAnalysis<'exact, 'pose>,
    ef_boundary: Option<&'ef AxisAlignedEfBoundaryCapabilityV1<'prerequisite, 'exact, 'pose>>,
    exact_e_analysis: &'exact_e_corridor ExactEFiniteHingeCorridorAnalysis<
        'prerequisite,
        'ef,
        'exact,
        'pose,
    >,
    exact: &'exact RationalCayleyTreePose<'pose>,
    bound: BoundMaterialTreePose<'pose>,
    paper_thickness_mm: f64,
    limits: DirectFFiniteHingeCorridorLimits,
) -> Result<
    DirectFFiniteHingeCorridorAnalysis<'prerequisite, 'ef, 'exact_e_corridor, 'exact, 'pose>,
    DirectFFiniteHingeCorridorError,
> {
    let limits = limits.projected();
    if !positive_finite_binary64(paper_thickness_mm) {
        return Ok(DirectFFiniteHingeCorridorAnalysis {
            result: DirectFFiniteHingeCorridorResult::Unresolved,
            work: DirectFFiniteHingeCorridorWork::default(),
        });
    }
    let Some((_, phase2b_work)) = exact_e_analysis.authenticated_contained_capability_and_work()
    else {
        let result = match exact_e_analysis.result {
            ExactEFiniteHingeCorridorResult::LayerOffsetUnmodeled => {
                DirectFFiniteHingeCorridorResult::LayerOffsetUnmodeled
            }
            ExactEFiniteHingeCorridorResult::Contained(_)
            | ExactEFiniteHingeCorridorResult::Outside(_)
            | ExactEFiniteHingeCorridorResult::Unresolved => {
                DirectFFiniteHingeCorridorResult::Unresolved
            }
        };
        return Ok(DirectFFiniteHingeCorridorAnalysis {
            result,
            work: DirectFFiniteHingeCorridorWork::default(),
        });
    };
    if let Err(error) = preflight_direct_f_exact_capacity(phase2b_work, &limits) {
        return match error {
            CayleyError::ResourceLimitExceeded { .. } => {
                Err(DirectFFiniteHingeCorridorError::ResourceLimitExceeded)
            }
            _ => Ok(DirectFFiniteHingeCorridorAnalysis {
                result: DirectFFiniteHingeCorridorResult::Unresolved,
                work: DirectFFiniteHingeCorridorWork::default(),
            }),
        };
    }

    let mut work = DirectFFiniteHingeCorridorWork {
        phase2b_exact: phase2b_work.exact.clone(),
        exact: phase2b_work.exact.clone(),
        ..DirectFFiniteHingeCorridorWork::default()
    };
    let mut local_meter = WorkMeter::new(&limits.local_exact);
    let mut prism_meter = WorkMeter::new(&limits.prism.exact);
    let result = calculate_direct_f_finite_hinge_corridor_v1(
        prerequisite_analysis,
        ef_boundary,
        exact_e_analysis,
        phase2b_work,
        exact,
        bound,
        paper_thickness_mm,
        &limits,
        &mut work,
        &mut local_meter,
        &mut prism_meter,
    );
    match result {
        Ok(mut result) => {
            let local_exact = local_meter.work;
            let prism_exact = prism_meter.work;
            if work.prism.exact != prism_exact {
                return Ok(DirectFFiniteHingeCorridorAnalysis {
                    result: DirectFFiniteHingeCorridorResult::Unresolved,
                    work: DirectFFiniteHingeCorridorWork {
                        phase2b_exact: phase2b_work.exact.clone(),
                        exact: phase2b_work.exact.clone(),
                        ..DirectFFiniteHingeCorridorWork::default()
                    },
                });
            }
            let cumulative = phase2b_work
                .exact
                .checked_merge(&local_exact, &limits.exact, None, STAGE)
                .and_then(|work| work.checked_merge(&prism_exact, &limits.exact, None, STAGE));
            let cumulative = match cumulative {
                Ok(cumulative) => cumulative,
                Err(CayleyError::ResourceLimitExceeded { .. }) => {
                    return Err(DirectFFiniteHingeCorridorError::ResourceLimitExceeded);
                }
                Err(_) => {
                    return Ok(DirectFFiniteHingeCorridorAnalysis {
                        result: DirectFFiniteHingeCorridorResult::Unresolved,
                        work: DirectFFiniteHingeCorridorWork {
                            phase2b_exact: phase2b_work.exact.clone(),
                            exact: phase2b_work.exact.clone(),
                            ..DirectFFiniteHingeCorridorWork::default()
                        },
                    });
                }
            };
            work.local_exact = local_exact;
            work.exact = cumulative;
            if let DirectFFiniteHingeCorridorResult::Contained(capability) = &mut result {
                capability.sealed_work = Some(work.clone());
            }
            Ok(DirectFFiniteHingeCorridorAnalysis { result, work })
        }
        Err(CayleyError::ResourceLimitExceeded { .. }) => {
            Err(DirectFFiniteHingeCorridorError::ResourceLimitExceeded)
        }
        Err(_) => Ok(DirectFFiniteHingeCorridorAnalysis {
            result: DirectFFiniteHingeCorridorResult::Unresolved,
            work: DirectFFiniteHingeCorridorWork {
                phase2b_exact: phase2b_work.exact.clone(),
                exact: phase2b_work.exact.clone(),
                ..DirectFFiniteHingeCorridorWork::default()
            },
        }),
    }
}

#[allow(clippy::too_many_arguments)]
fn calculate_direct_f_finite_hinge_corridor_v1<
    'prerequisite,
    'ef,
    'exact_e_corridor,
    'exact,
    'pose,
>(
    prerequisite_analysis: &'prerequisite SingleTriangularHingePrerequisiteAnalysis<'exact, 'pose>,
    ef_boundary: Option<&'ef AxisAlignedEfBoundaryCapabilityV1<'prerequisite, 'exact, 'pose>>,
    exact_e_analysis: &'exact_e_corridor ExactEFiniteHingeCorridorAnalysis<
        'prerequisite,
        'ef,
        'exact,
        'pose,
    >,
    phase2b_work: &ExactEFiniteHingeCorridorWork,
    exact: &'exact RationalCayleyTreePose<'pose>,
    bound: BoundMaterialTreePose<'pose>,
    paper_thickness_mm: f64,
    limits: &DirectFFiniteHingeCorridorLimits,
    work: &mut DirectFFiniteHingeCorridorWork,
    local_meter: &mut WorkMeter<'_>,
    prism_meter: &mut WorkMeter<'_>,
) -> Result<
    DirectFFiniteHingeCorridorResult<'prerequisite, 'ef, 'exact_e_corridor, 'exact, 'pose>,
    CayleyError,
> {
    let prerequisite = match &prerequisite_analysis.result {
        SingleTriangularHingePrerequisiteResult::LayerOffsetUnmodeled => {
            return Ok(DirectFFiniteHingeCorridorResult::LayerOffsetUnmodeled);
        }
        SingleTriangularHingePrerequisiteResult::Unresolved => {
            return Ok(DirectFFiniteHingeCorridorResult::Unresolved);
        }
        SingleTriangularHingePrerequisiteResult::Authenticated(capability) => capability,
    };
    let Some(ef_boundary) = ef_boundary else {
        return Ok(DirectFFiniteHingeCorridorResult::Unresolved);
    };
    let exact_e_corridor = match &exact_e_analysis.result {
        ExactEFiniteHingeCorridorResult::Contained(capability) => capability.as_ref(),
        ExactEFiniteHingeCorridorResult::LayerOffsetUnmodeled => {
            return Ok(DirectFFiniteHingeCorridorResult::LayerOffsetUnmodeled);
        }
        ExactEFiniteHingeCorridorResult::Outside(_)
        | ExactEFiniteHingeCorridorResult::Unresolved => {
            return Ok(DirectFFiniteHingeCorridorResult::Unresolved);
        }
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
        || revalidate_exact_e_finite_hinge_corridor_v1(
            exact_e_corridor,
            prerequisite,
            ef_boundary,
            exact,
            bound,
            paper_thickness_mm,
        )
        .is_none()
    {
        return Ok(DirectFFiniteHingeCorridorResult::Unresolved);
    }

    charge_fixed_work(work, limits)?;
    let left_face_index = prerequisite.left_face_index;
    let right_face_index = prerequisite.right_face_index;
    let hinge_index = prerequisite.hinge_index;
    if exact.faces.len() != FACE_COUNT
        || exact.hinges.len() != HINGE_COUNT
        || left_face_index == right_face_index
        || left_face_index >= FACE_COUNT
        || right_face_index >= FACE_COUNT
        || hinge_index >= HINGE_COUNT
    {
        return Ok(DirectFFiniteHingeCorridorResult::Unresolved);
    }

    let two = BigRational::from_integer(2.into());
    let thickness = exact_f64(paper_thickness_mm, local_meter, STAGE)?;
    let half_thickness = local_meter.divide_rational(&thickness, &two, STAGE)?;
    if local_meter.compare_rational(&half_thickness, &BigRational::zero(), STAGE)?
        != Ordering::Greater
    {
        return Ok(DirectFFiniteHingeCorridorResult::Unresolved);
    }

    let sealed_face_transforms = ef_boundary.binary64_face_transforms;
    let first_face = reconstruct_direct_f_face(
        exact,
        bound,
        0,
        &sealed_face_transforms[0],
        &half_thickness,
        local_meter,
    )?;
    let second_face = reconstruct_direct_f_face(
        exact,
        bound,
        1,
        &sealed_face_transforms[1],
        &half_thickness,
        local_meter,
    )?;
    let (left_face, right_face) = if left_face_index == 0 {
        (&first_face, &second_face)
    } else {
        (&second_face, &first_face)
    };
    if left_face.face != exact.faces[left_face_index].face
        || right_face.face != exact.faces[right_face_index].face
    {
        return Ok(DirectFFiniteHingeCorridorResult::Unresolved);
    }

    let (axis_start, axis, length_squared, hinge_parent_transform) =
        reconstruct_direct_f_hinge_axis(
            exact,
            bound,
            hinge_index,
            &sealed_face_transforms,
            [&first_face.transform, &second_face.transform],
            local_meter,
        )?
        .ok_or(CayleyError::InvariantFailure { stage: STAGE })?;
    let geometry = match reconstruct_direct_f_corridor_geometry(
        axis_start,
        axis,
        length_squared,
        half_thickness,
        &left_face.normal,
        &right_face.normal,
        local_meter,
    )? {
        DirectFCorridorGeometryResult::Ready(geometry) => *geometry,
        DirectFCorridorGeometryResult::LayerOffsetUnmodeled => {
            return Ok(DirectFFiniteHingeCorridorResult::LayerOffsetUnmodeled);
        }
        DirectFCorridorGeometryResult::Unresolved => {
            return Ok(DirectFFiniteHingeCorridorResult::Unresolved);
        }
    };

    let first = ExactPrebuiltTriangularPrismView {
        vertices: std::array::from_fn(|index| &first_face.solid_vertices[index]),
    };
    let second = ExactPrebuiltTriangularPrismView {
        vertices: std::array::from_fn(|index| &second_face.solid_vertices[index]),
    };
    let Some(report) = analyze_exact_prebuilt_prism_pair_with_meter_v1(
        first,
        second,
        limits.prism,
        &mut work.prism,
        prism_meter,
    )?
    else {
        return Ok(DirectFFiniteHingeCorridorResult::Unresolved);
    };
    let Some(interaction_kind) = authenticate_interaction_kind(&report) else {
        return Ok(DirectFFiniteHingeCorridorResult::Unresolved);
    };
    if interaction_kind != exact_e_corridor.interaction_kind() {
        return Ok(DirectFFiniteHingeCorridorResult::InteractionKindMismatch {
            exact_e: exact_e_corridor.interaction_kind(),
            direct_f: interaction_kind,
        });
    }
    if let Some(outside) = scan_complete_intersection_against_corridor(
        &report,
        interaction_kind,
        &geometry,
        limits,
        work,
        local_meter,
    )? {
        return Ok(DirectFFiniteHingeCorridorResult::Outside(outside));
    }

    Ok(DirectFFiniteHingeCorridorResult::Contained(Box::new(
        DirectFFiniteHingeCorridorCapabilityV1 {
            prerequisite,
            ef_boundary,
            exact_e_corridor,
            exact,
            bound,
            paper_thickness_bits: paper_thickness_mm.to_bits(),
            left_face_index,
            right_face_index,
            hinge_index,
            interaction_kind,
            binary64_face_transforms: sealed_face_transforms,
            hinge_parent_transform,
            geometry,
            phase2b_work: phase2b_work.clone(),
            sealed_work: None,
        },
    )))
}

struct DirectFFaceGeometry {
    face: FaceId,
    transform: ExactRigidTransform,
    normal: ExactVector3,
    solid_vertices: [ExactPoint3; 6],
}

fn reconstruct_direct_f_face(
    exact: &RationalCayleyTreePose<'_>,
    bound: BoundMaterialTreePose<'_>,
    face_index: usize,
    sealed_bits: &BoundBinary64FaceTransformBits,
    half_thickness: &BigRational,
    meter: &mut WorkMeter<'_>,
) -> Result<DirectFFaceGeometry, CayleyError> {
    let face = exact
        .faces
        .get(face_index)
        .ok_or(CayleyError::InvariantFailure { stage: STAGE })?;
    if sealed_bits.face != face.face
        || capture_face_transform_bits(bound, face.face).as_ref() != Some(sealed_bits)
    {
        return Err(CayleyError::InvariantFailure { stage: STAGE });
    }
    let source_boundary = bound
        .face_boundary(face.face)
        .filter(|boundary| bound.model().owns_face_boundary(*boundary))
        .ok_or(CayleyError::InvariantFailure { stage: STAGE })?;
    if source_boundary.vertices().len() != 3
        || !source_boundary
            .vertices()
            .iter()
            .copied()
            .eq(face.boundary.iter().map(|(vertex, _)| *vertex))
    {
        return Err(CayleyError::InvariantFailure { stage: STAGE });
    }
    let transform = lift_transform_bits(sealed_bits, meter)?;
    let mut mid_surface = Vec::with_capacity(3);
    for vertex in source_boundary.vertices() {
        let source = bound
            .model()
            .vertex_position(*vertex)
            .ok_or(CayleyError::InvariantFailure { stage: STAGE })?;
        let source = lift_point3(source, meter)?;
        mid_surface.push(apply_exact_transform(&transform, &source, meter)?);
    }
    let mid_surface: [ExactPoint3; 3] = mid_surface
        .try_into()
        .map_err(|_| CayleyError::InvariantFailure { stage: STAGE })?;
    let normal = ExactVector3 {
        coordinates: try_array3(|axis| meter.clone_rational(&transform.rotation[axis][1], STAGE))?,
    };
    let material_offset = ExactVector3 {
        coordinates: try_array3(|axis| {
            meter.multiply_rational(&normal.coordinates[axis], half_thickness, STAGE)
        })?,
    };
    let solid_vertices = [
        exact_offset(&mid_surface[0], &material_offset, true, meter)?,
        exact_offset(&mid_surface[1], &material_offset, true, meter)?,
        exact_offset(&mid_surface[2], &material_offset, true, meter)?,
        exact_offset(&mid_surface[0], &material_offset, false, meter)?,
        exact_offset(&mid_surface[1], &material_offset, false, meter)?,
        exact_offset(&mid_surface[2], &material_offset, false, meter)?,
    ];
    Ok(DirectFFaceGeometry {
        face: face.face,
        transform,
        normal,
        solid_vertices,
    })
}

fn reconstruct_direct_f_hinge_axis(
    exact: &RationalCayleyTreePose<'_>,
    bound: BoundMaterialTreePose<'_>,
    hinge_index: usize,
    sealed_face_transforms: &[BoundBinary64FaceTransformBits; FACE_COUNT],
    lifted_face_transforms: [&ExactRigidTransform; FACE_COUNT],
    meter: &mut WorkMeter<'_>,
) -> Result<
    Option<(
        ExactPoint3,
        ExactVector3,
        BigRational,
        BoundBinary64FaceTransformBits,
    )>,
    CayleyError,
> {
    let exact_hinge = exact
        .hinges
        .get(hinge_index)
        .ok_or(CayleyError::InvariantFailure { stage: STAGE })?;
    let native_hinge = bound
        .model()
        .hinges()
        .get(hinge_index)
        .ok_or(CayleyError::InvariantFailure { stage: STAGE })?;
    if native_hinge.edge() != exact_hinge.edge {
        return Ok(None);
    }
    let parent_face_index = exact
        .faces
        .iter()
        .position(|face| face.face == exact_hinge.parent)
        .ok_or(CayleyError::InvariantFailure { stage: STAGE })?;
    let expected_parent = sealed_face_transforms
        .get(parent_face_index)
        .ok_or(CayleyError::InvariantFailure { stage: STAGE })?;
    let hinge_parent_native = bound
        .pose()
        .hinge_parent_transform(exact_hinge.edge)
        .ok_or(CayleyError::InvariantFailure { stage: STAGE })?;
    let hinge_parent_transform = capture_transform_bits(exact_hinge.parent, hinge_parent_native);
    if &hinge_parent_transform != expected_parent
        || capture_face_transform_bits(bound, exact_hinge.parent).as_ref() != Some(expected_parent)
    {
        return Ok(None);
    }

    // The corridor is intentionally reconstructed from the native parent
    // path.  Child endpoints are neither averaged nor welded to this axis.
    let rest_start = lift_point3(native_hinge.start(), meter)?;
    let rest_end = lift_point3(native_hinge.end(), meter)?;
    let parent_transform = lifted_face_transforms
        .get(parent_face_index)
        .ok_or(CayleyError::InvariantFailure { stage: STAGE })?;
    let axis_start = apply_exact_transform(parent_transform, &rest_start, meter)?;
    let axis_end = apply_exact_transform(parent_transform, &rest_end, meter)?;
    let axis = exact_between(&axis_start, &axis_end, meter)?;
    let length_squared = exact_dot(&axis, &axis, meter)?;
    if meter.compare_rational(&length_squared, &BigRational::zero(), STAGE)? != Ordering::Greater {
        return Ok(None);
    }
    Ok(Some((
        axis_start,
        axis,
        length_squared,
        hinge_parent_transform,
    )))
}

enum DirectFCorridorGeometryResult {
    Ready(Box<DirectFCorridorGeometry>),
    LayerOffsetUnmodeled,
    Unresolved,
}

fn reconstruct_direct_f_corridor_geometry(
    axis_start: ExactPoint3,
    axis: ExactVector3,
    length_squared: BigRational,
    half_thickness: BigRational,
    left_normal: &ExactVector3,
    right_normal: &ExactVector3,
    meter: &mut WorkMeter<'_>,
) -> Result<DirectFCorridorGeometryResult, CayleyError> {
    let zero = BigRational::zero();
    let one = BigRational::one();
    let two = BigRational::from_integer(2.into());
    if meter.compare_rational(&half_thickness, &zero, STAGE)? != Ordering::Greater
        || meter.compare_rational(&length_squared, &zero, STAGE)? != Ordering::Greater
    {
        return Ok(DirectFCorridorGeometryResult::Unresolved);
    }
    let normal_dot = exact_dot(left_normal, right_normal, meter)?;
    let one_plus_dot = meter.add_rational(&one, &normal_dot, STAGE)?;
    let cosine_half_squared = meter.divide_rational(&one_plus_dot, &two, STAGE)?;
    let numerical_flat_fold_limit = exact_f64(f64::EPSILON, meter, STAGE)?;
    if meter.compare_rational(&cosine_half_squared, &numerical_flat_fold_limit, STAGE)?
        != Ordering::Greater
    {
        return Ok(DirectFCorridorGeometryResult::LayerOffsetUnmodeled);
    }
    if meter.compare_rational(&cosine_half_squared, &one, STAGE)? == Ordering::Greater {
        return Ok(DirectFCorridorGeometryResult::Unresolved);
    }
    let half_thickness_squared =
        meter.multiply_rational(&half_thickness, &half_thickness, STAGE)?;
    let finite_axis_bound =
        meter.multiply_rational(&length_squared, &cosine_half_squared, STAGE)?;
    if meter.compare_rational(&half_thickness_squared, &finite_axis_bound, STAGE)?
        == Ordering::Greater
    {
        return Ok(DirectFCorridorGeometryResult::LayerOffsetUnmodeled);
    }
    let radial_limit_product =
        meter.multiply_rational(&half_thickness_squared, &length_squared, STAGE)?;
    Ok(DirectFCorridorGeometryResult::Ready(Box::new(
        DirectFCorridorGeometry {
            axis_start,
            axis,
            length_squared,
            half_thickness,
            cosine_half_squared,
            radial_limit_product,
            left_normal: clone_vector(left_normal, meter)?,
            right_normal: clone_vector(right_normal, meter)?,
        },
    )))
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
    geometry: &DirectFCorridorGeometry,
    limits: &DirectFFiniteHingeCorridorLimits,
    work: &mut DirectFFiniteHingeCorridorWork,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<DirectFFiniteHingeCorridorOutside>, CayleyError> {
    let zero = BigRational::zero();
    let tests_before = work.corridor_vertex_tests;
    let mut first_outside_vertex_index = None;
    let mut outside_vertex_count = 0_usize;
    let mut aggregate = CorridorViolation::default();
    let mut first_radial_excess = None;
    for (vertex_index, vertex) in report.canonical_vertices().iter().enumerate() {
        charge_counter(
            &mut work.corridor_vertex_tests,
            limits.max_corridor_vertex_tests,
            "direct_f_corridor_vertex_tests",
        )?;
        let relative = exact_between(&geometry.axis_start, vertex, meter)?;
        let axial = exact_dot(&relative, &geometry.axis, meter)?;
        let cross = exact_cross(&relative, &geometry.axis, meter)?;
        let radial_squared = exact_dot(&cross, &cross, meter)?;
        let radial_left =
            meter.multiply_rational(&geometry.cosine_half_squared, &radial_squared, STAGE)?;
        let violation = CorridorViolation {
            axial_before_start: meter.compare_rational(&axial, &zero, STAGE)? == Ordering::Less,
            axial_after_end: meter.compare_rational(&axial, &geometry.length_squared, STAGE)?
                == Ordering::Greater,
            radial_outside: meter.compare_rational(
                &radial_left,
                &geometry.radial_limit_product,
                STAGE,
            )? == Ordering::Greater,
        };
        if violation.any() {
            first_outside_vertex_index.get_or_insert(vertex_index);
            if violation.radial_outside && first_radial_excess.is_none() {
                first_radial_excess = Some(meter.subtract_rational(
                    &radial_left,
                    &geometry.radial_limit_product,
                    STAGE,
                )?);
            }
            outside_vertex_count =
                outside_vertex_count
                    .checked_add(1)
                    .ok_or(CayleyError::ResourceLimitExceeded {
                        stage: STAGE,
                        resource: "direct_f_corridor_outside_vertices",
                    })?;
            aggregate.axial_before_start |= violation.axial_before_start;
            aggregate.axial_after_end |= violation.axial_after_end;
            aggregate.radial_outside |= violation.radial_outside;
        }
    }
    let expected_tests = tests_before
        .checked_add(report.canonical_vertices().len())
        .ok_or(CayleyError::ResourceLimitExceeded {
            stage: STAGE,
            resource: "direct_f_corridor_vertex_tests",
        })?;
    if work.corridor_vertex_tests != expected_tests {
        return Err(CayleyError::InvariantFailure { stage: STAGE });
    }
    Ok(
        first_outside_vertex_index.map(|first_outside_vertex_index| {
            DirectFFiniteHingeCorridorOutside {
                interaction_kind,
                first_outside_vertex_index,
                outside_vertex_count,
                axial_before_start: aggregate.axial_before_start,
                axial_after_end: aggregate.axial_after_end,
                radial_outside: aggregate.radial_outside,
                first_radial_excess,
            }
        }),
    )
}

fn lift_transform_bits(
    bits: &BoundBinary64FaceTransformBits,
    meter: &mut WorkMeter<'_>,
) -> Result<ExactRigidTransform, CayleyError> {
    Ok(ExactRigidTransform {
        rotation: try_array3(|row| {
            try_array3(|column| exact_f64(f64::from_bits(bits.rotation[row][column]), meter, STAGE))
        })?,
        translation: ExactVector3 {
            coordinates: try_array3(|axis| {
                exact_f64(f64::from_bits(bits.translation[axis]), meter, STAGE)
            })?,
        },
    })
}

fn lift_point3(
    point: ori_kinematics::Point3,
    meter: &mut WorkMeter<'_>,
) -> Result<ExactPoint3, CayleyError> {
    Ok(ExactPoint3 {
        coordinates: [
            exact_f64(point.x(), meter, STAGE)?,
            exact_f64(point.y(), meter, STAGE)?,
            exact_f64(point.z(), meter, STAGE)?,
        ],
    })
}

fn exact_offset(
    point: &ExactPoint3,
    offset: &ExactVector3,
    add: bool,
    meter: &mut WorkMeter<'_>,
) -> Result<ExactPoint3, CayleyError> {
    Ok(ExactPoint3 {
        coordinates: try_array3(|axis| {
            if add {
                meter.add_rational(&point.coordinates[axis], &offset.coordinates[axis], STAGE)
            } else {
                meter.subtract_rational(&point.coordinates[axis], &offset.coordinates[axis], STAGE)
            }
        })?,
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

fn clone_vector(
    vector: &ExactVector3,
    meter: &mut WorkMeter<'_>,
) -> Result<ExactVector3, CayleyError> {
    Ok(ExactVector3 {
        coordinates: try_array3(|axis| meter.clone_rational(&vector.coordinates[axis], STAGE))?,
    })
}

fn capture_face_transform_bits(
    bound: BoundMaterialTreePose<'_>,
    face: FaceId,
) -> Option<BoundBinary64FaceTransformBits> {
    bound
        .pose()
        .face_transform(face)
        .map(|transform| capture_transform_bits(face, transform))
}

fn capture_transform_bits(
    face: FaceId,
    transform: RigidTransform,
) -> BoundBinary64FaceTransformBits {
    let rotation = transform.rotation_rows();
    let translation = transform.translation();
    BoundBinary64FaceTransformBits {
        face,
        rotation: std::array::from_fn(|row| {
            std::array::from_fn(|column| rotation[row][column].to_bits())
        }),
        translation: [
            translation.x().to_bits(),
            translation.y().to_bits(),
            translation.z().to_bits(),
        ],
    }
}

fn charge_fixed_work(
    work: &mut DirectFFiniteHingeCorridorWork,
    limits: &DirectFFiniteHingeCorridorLimits,
) -> Result<(), CayleyError> {
    for (counter, required, maximum, resource) in [
        (
            &mut work.authenticated_faces,
            FACE_COUNT,
            limits.max_authenticated_faces,
            "direct_f_corridor_faces",
        ),
        (
            &mut work.authenticated_hinges,
            HINGE_COUNT,
            limits.max_authenticated_hinges,
            "direct_f_corridor_hinges",
        ),
        (
            &mut work.face_transform_bit_bindings,
            FACE_TRANSFORM_BIT_BINDINGS,
            limits.max_face_transform_bit_bindings,
            "direct_f_corridor_face_transform_bits",
        ),
        (
            &mut work.hinge_parent_transform_bit_bindings,
            HINGE_PARENT_TRANSFORM_BIT_BINDINGS,
            limits.max_hinge_parent_transform_bit_bindings,
            "direct_f_corridor_hinge_parent_transform_bits",
        ),
        (
            &mut work.transform_scalar_lifts,
            TRANSFORM_SCALAR_LIFTS,
            limits.max_transform_scalar_lifts,
            "direct_f_corridor_transform_lifts",
        ),
        (
            &mut work.source_coordinate_lifts,
            SOURCE_COORDINATE_LIFTS,
            limits.max_source_coordinate_lifts,
            "direct_f_corridor_source_lifts",
        ),
        (
            &mut work.affine_point_reconstructions,
            AFFINE_POINT_RECONSTRUCTIONS,
            limits.max_affine_point_reconstructions,
            "direct_f_corridor_affine_points",
        ),
        (
            &mut work.material_normal_component_lifts,
            MATERIAL_NORMAL_COMPONENT_LIFTS,
            limits.max_material_normal_component_lifts,
            "direct_f_corridor_normal_lifts",
        ),
        (
            &mut work.solid_vertex_constructions,
            SOLID_VERTEX_CONSTRUCTIONS,
            limits.max_solid_vertex_constructions,
            "direct_f_corridor_solid_vertices",
        ),
        (
            &mut work.thickness_lifts,
            THICKNESS_LIFTS,
            limits.max_thickness_lifts,
            "direct_f_corridor_thickness_lifts",
        ),
        (
            &mut work.half_thickness_divisions,
            HALF_THICKNESS_DIVISIONS,
            limits.max_half_thickness_divisions,
            "direct_f_corridor_half_thickness_divisions",
        ),
        (
            &mut work.scalar_reconstructions,
            SCALAR_RECONSTRUCTIONS,
            limits.max_scalar_reconstructions,
            "direct_f_corridor_scalar_reconstructions",
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

#[allow(clippy::too_many_arguments)]
pub(super) fn revalidate_direct_f_finite_hinge_corridor_v1<
    'capability,
    'prerequisite,
    'ef,
    'exact_e_corridor,
    'exact,
    'pose,
>(
    capability: &'capability DirectFFiniteHingeCorridorCapabilityV1<
        'prerequisite,
        'ef,
        'exact_e_corridor,
        'exact,
        'pose,
    >,
    prerequisite: &AuthenticatedSingleTriangularHingePrerequisitesV1<'_, '_>,
    ef_boundary: &AxisAlignedEfBoundaryCapabilityV1<'_, '_, '_>,
    exact_e_corridor: &ExactEFiniteHingeCorridorCapabilityV1<'_, '_, '_, '_>,
    exact: &RationalCayleyTreePose<'_>,
    bound: BoundMaterialTreePose<'_>,
    paper_thickness_mm: f64,
) -> Option<
    RevalidatedDirectFFiniteHingeCorridorCapabilityV1<
        'capability,
        'prerequisite,
        'ef,
        'exact_e_corridor,
        'exact,
        'pose,
    >,
> {
    let sealed_phase2b_work = exact_e_corridor.sealed_work()?;
    let sealed_work = capability.sealed_work()?;
    if !positive_finite_binary64(paper_thickness_mm)
        || capability.phase2b_work != *sealed_phase2b_work
        || sealed_work.phase2b_exact != sealed_phase2b_work.exact
        || !std::ptr::eq(capability.prerequisite, prerequisite)
        || !std::ptr::eq(capability.ef_boundary, ef_boundary)
        || !std::ptr::eq(capability.exact_e_corridor, exact_e_corridor)
        || !std::ptr::eq(capability.exact, exact)
        || capability.paper_thickness_bits != paper_thickness_mm.to_bits()
        || capability.bound.model() != bound.model()
        || !capability.bound.pose().same_instance(bound.pose())
        || !exact.is_for(bound)
        || capability.left_face_index != prerequisite.left_face_index
        || capability.right_face_index != prerequisite.right_face_index
        || capability.hinge_index != prerequisite.hinge_index
        || capability.interaction_kind != exact_e_corridor.interaction_kind()
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
        || revalidate_exact_e_finite_hinge_corridor_v1(
            exact_e_corridor,
            prerequisite,
            ef_boundary,
            exact,
            bound,
            paper_thickness_mm,
        )
        .is_none()
        || capability.binary64_face_transforms != ef_boundary.binary64_face_transforms
    {
        return None;
    }
    for (face_index, exact_face) in exact.faces.iter().enumerate() {
        if capture_face_transform_bits(bound, exact_face.face)?
            != capability.binary64_face_transforms[face_index]
        {
            return None;
        }
    }
    let exact_hinge = exact.hinges.get(capability.hinge_index)?;
    let current_hinge_parent = capture_transform_bits(
        exact_hinge.parent,
        bound.pose().hinge_parent_transform(exact_hinge.edge)?,
    );
    let parent_face_index = exact
        .faces
        .iter()
        .position(|face| face.face == exact_hinge.parent)?;
    if current_hinge_parent != capability.hinge_parent_transform
        || current_hinge_parent != capability.binary64_face_transforms[parent_face_index]
    {
        return None;
    }
    Some(RevalidatedDirectFFiniteHingeCorridorCapabilityV1 { capability })
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
    fn closed_axis_and_radial_boundaries_are_accepted_and_scan_is_complete() {
        let geometry = DirectFCorridorGeometry {
            axis_start: point(0, 0, 0),
            axis: vector(1, 0, 0),
            length_squared: integer(1),
            half_thickness: integer(1),
            cosine_half_squared: integer(1),
            radial_limit_product: integer(1),
            left_normal: vector(0, 1, 0),
            right_normal: vector(0, 1, 0),
        };
        let limits = direct_f_corridor_combined_hard_cayley_limits();
        let mut meter = WorkMeter::new(&limits);
        for vertex in [point(0, 1, 0), point(1, 1, 0)] {
            let relative = exact_between(&geometry.axis_start, &vertex, &mut meter).unwrap();
            let axial = exact_dot(&relative, &geometry.axis, &mut meter).unwrap();
            let cross = exact_cross(&relative, &geometry.axis, &mut meter).unwrap();
            let radial_squared = exact_dot(&cross, &cross, &mut meter).unwrap();
            let radial_left = meter
                .multiply_rational(&geometry.cosine_half_squared, &radial_squared, STAGE)
                .unwrap();
            assert!(axial >= integer(0) && axial <= geometry.length_squared);
            assert!(radial_left <= geometry.radial_limit_product);
        }
    }

    #[test]
    fn c2_epsilon_and_finite_axis_limit_close_to_layer_offset() {
        let limits = direct_f_corridor_local_hard_cayley_limits();
        let mut epsilon_meter = WorkMeter::new(&limits);
        let epsilon = BigRational::from_float(f64::EPSILON).unwrap();
        let result = reconstruct_direct_f_corridor_geometry(
            point(0, 0, 0),
            vector(1, 0, 0),
            integer(1),
            integer(1),
            &vector(0, 1, 0),
            &ExactVector3 {
                coordinates: [integer(0), &integer(2) * &epsilon - integer(1), integer(0)],
            },
            &mut epsilon_meter,
        )
        .unwrap();
        assert!(matches!(
            result,
            DirectFCorridorGeometryResult::LayerOffsetUnmodeled
        ));

        let mut equality_meter = WorkMeter::new(&limits);
        let equality = reconstruct_direct_f_corridor_geometry(
            point(0, 0, 0),
            vector(1, 0, 0),
            integer(1),
            integer(1),
            &vector(0, 1, 0),
            &vector(0, 1, 0),
            &mut equality_meter,
        )
        .unwrap();
        let DirectFCorridorGeometryResult::Ready(equality) = equality else {
            panic!("h² = D²*c2 is the closed R=L boundary");
        };
        assert_eq!(equality.radial_limit_product, integer(1));

        let mut finite_meter = WorkMeter::new(&limits);
        let result = reconstruct_direct_f_corridor_geometry(
            point(0, 0, 0),
            vector(1, 0, 0),
            integer(1),
            integer(2),
            &vector(0, 1, 0),
            &vector(0, 1, 0),
            &mut finite_meter,
        )
        .unwrap();
        assert!(matches!(
            result,
            DirectFCorridorGeometryResult::LayerOffsetUnmodeled
        ));
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
