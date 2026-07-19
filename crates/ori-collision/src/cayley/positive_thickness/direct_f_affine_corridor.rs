//! Private exact proof for the literal binary64 affine hinge corridor.
//!
//! This phase is deliberately disconnected from production collision policy,
//! DTOs, persistence, UI, and mutation authority. It consumes the sealed
//! phase-2B exact-E proof, independently reconstructs the literal-F prisms,
//! reruns their complete exact intersection, and then proves containment in
//! the intersection of two canonical affine half-prisms.

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
    ExactEFiniteHingeCorridorResult, ExactEFiniteHingeCorridorWork,
    ExactEFiniteHingeInteractionKind, revalidate_exact_e_finite_hinge_corridor_v1,
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
const SHARED_ENDPOINT_EQUALITY_TESTS: usize = 2;
const TRANSFORM_SCALAR_LIFTS: usize = FACE_COUNT * 12;
const SOURCE_COORDINATE_LIFTS: usize = FACE_COUNT * 3 * 3;
const AFFINE_POINT_RECONSTRUCTIONS: usize = FACE_COUNT * 3;
const SOLID_VERTEX_CONSTRUCTIONS: usize = FACE_COUNT * 6;
const THICKNESS_LIFTS: usize = 1;
const HALF_THICKNESS_DIVISIONS: usize = 1;
const CANONICAL_INWARD_DIRECTIONS: usize = FACE_COUNT;
const DUAL_BASIS_INVERSIONS: usize = FACE_COUNT;
const AFFINE_HALF_PRISMS: usize = FACE_COUNT;
const HALFSPACE_COUNT: usize = FACE_COUNT * 5;
const PLANE_TRIPLE_COUNT: usize = 120;
const MAX_MEMBERSHIP_TESTS: usize = PLANE_TRIPLE_COUNT * HALFSPACE_COUNT;
const MAX_CORRIDOR_VERTICES: usize = PLANE_TRIPLE_COUNT;
const MAX_DEDUP_COMPARISONS: usize = MAX_CORRIDOR_VERTICES * (MAX_CORRIDOR_VERTICES - 1) / 2;
const NORMAL_RANK_TESTS: usize = HALFSPACE_COUNT;
const RECESSION_NORMAL_PAIRS: usize = 45;
const SIGNED_RECESSION_TESTS: usize = RECESSION_NORMAL_PAIRS * 2;
const RECESSION_MEMBERSHIP_TESTS: usize = SIGNED_RECESSION_TESTS * HALFSPACE_COUNT;
const MAX_PRISM_VERTEX_HALFSPACE_TESTS: usize = 120 * HALFSPACE_COUNT;
const GRAM_INVERSIONS: usize = 1;
const GRAM_POSITIVE_DEFINITE_TESTS: usize = 3;
const MAX_GRAM_QUADRATIC_TESTS: usize = MAX_CORRIDOR_VERTICES;
const MAX_AXIAL_TESTS: usize = 120;

fn literal_f_local_hard_cayley_limits() -> CayleyLimits {
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

fn affine_corridor_local_hard_cayley_limits() -> CayleyLimits {
    let exact = CayleyLimits::default();
    CayleyLimits {
        max_precision_rounds: 0,
        max_guard_bits: 0,
        max_candidate_bits: 0,
        max_machin_terms_per_series: 0,
        max_trig_terms_per_series: 0,
        max_sqrt_refinements: 0,
        max_interval_operations: 262_144,
        max_shift_bits: exact.max_shift_bits,
        max_intermediate_bits: exact.max_intermediate_bits,
        max_gcd_fallback_calls: 32_768,
        max_gcd_fallback_input_bits: 268_435_456,
        max_rational_allocations: 262_144,
        max_rational_allocation_bits: exact.max_rational_allocation_bits,
        max_total_rational_allocation_bits: 536_870_912,
        max_output_bits: 0,
    }
}

fn affine_corridor_combined_hard_cayley_limits() -> CayleyLimits {
    let prior = super::exact_e_corridor::ExactEFiniteHingeCorridorLimits::default().exact;
    let literal = literal_f_local_hard_cayley_limits();
    let prism = exact_prism_hard_cayley_limits();
    let local = affine_corridor_local_hard_cayley_limits();
    let sum4 =
        |first: usize, second: usize, third: usize, fourth: usize, resource: &'static str| {
            let first_two = checked_hard_limit_sum(first, second, resource)
                .expect("C2 hard-limit constants must fit usize");
            let first_three = checked_hard_limit_sum(first_two, third, resource)
                .expect("C2 hard-limit constants must fit usize");
            checked_hard_limit_sum(first_three, fourth, resource)
                .expect("C2 hard-limit constants must fit usize")
        };
    CayleyLimits {
        max_precision_rounds: prior
            .max_precision_rounds
            .max(literal.max_precision_rounds)
            .max(prism.max_precision_rounds)
            .max(local.max_precision_rounds),
        max_guard_bits: prior
            .max_guard_bits
            .max(literal.max_guard_bits)
            .max(prism.max_guard_bits)
            .max(local.max_guard_bits),
        max_candidate_bits: prior
            .max_candidate_bits
            .max(literal.max_candidate_bits)
            .max(prism.max_candidate_bits)
            .max(local.max_candidate_bits),
        max_machin_terms_per_series: prior
            .max_machin_terms_per_series
            .max(literal.max_machin_terms_per_series)
            .max(prism.max_machin_terms_per_series)
            .max(local.max_machin_terms_per_series),
        max_trig_terms_per_series: prior
            .max_trig_terms_per_series
            .max(literal.max_trig_terms_per_series)
            .max(prism.max_trig_terms_per_series)
            .max(local.max_trig_terms_per_series),
        max_sqrt_refinements: prior
            .max_sqrt_refinements
            .max(literal.max_sqrt_refinements)
            .max(prism.max_sqrt_refinements)
            .max(local.max_sqrt_refinements),
        max_interval_operations: sum4(
            prior.max_interval_operations,
            literal.max_interval_operations,
            prism.max_interval_operations,
            local.max_interval_operations,
            "affine_corridor_combined_interval_operations",
        ),
        max_shift_bits: prior
            .max_shift_bits
            .max(literal.max_shift_bits)
            .max(prism.max_shift_bits)
            .max(local.max_shift_bits),
        max_intermediate_bits: prior
            .max_intermediate_bits
            .max(literal.max_intermediate_bits)
            .max(prism.max_intermediate_bits)
            .max(local.max_intermediate_bits),
        max_gcd_fallback_calls: sum4(
            prior.max_gcd_fallback_calls,
            literal.max_gcd_fallback_calls,
            prism.max_gcd_fallback_calls,
            local.max_gcd_fallback_calls,
            "affine_corridor_combined_gcd_calls",
        ),
        max_gcd_fallback_input_bits: sum4(
            prior.max_gcd_fallback_input_bits,
            literal.max_gcd_fallback_input_bits,
            prism.max_gcd_fallback_input_bits,
            local.max_gcd_fallback_input_bits,
            "affine_corridor_combined_gcd_input_bits",
        ),
        max_rational_allocations: sum4(
            prior.max_rational_allocations,
            literal.max_rational_allocations,
            prism.max_rational_allocations,
            local.max_rational_allocations,
            "affine_corridor_combined_allocations",
        ),
        max_rational_allocation_bits: prior
            .max_rational_allocation_bits
            .max(literal.max_rational_allocation_bits)
            .max(prism.max_rational_allocation_bits)
            .max(local.max_rational_allocation_bits),
        max_total_rational_allocation_bits: sum4(
            prior.max_total_rational_allocation_bits,
            literal.max_total_rational_allocation_bits,
            prism.max_total_rational_allocation_bits,
            local.max_total_rational_allocation_bits,
            "affine_corridor_combined_allocation_bits",
        ),
        max_output_bits: prior
            .max_output_bits
            .max(literal.max_output_bits)
            .max(prism.max_output_bits)
            .max(local.max_output_bits),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct DirectFAffineHingeCorridorLimits {
    pub(super) max_authenticated_faces: usize,
    pub(super) max_authenticated_hinges: usize,
    pub(super) max_face_transform_bit_bindings: usize,
    pub(super) max_hinge_parent_transform_bit_bindings: usize,
    pub(super) max_shared_endpoint_equality_tests: usize,
    pub(super) max_transform_scalar_lifts: usize,
    pub(super) max_source_coordinate_lifts: usize,
    pub(super) max_affine_point_reconstructions: usize,
    pub(super) max_solid_vertex_constructions: usize,
    pub(super) max_thickness_lifts: usize,
    pub(super) max_half_thickness_divisions: usize,
    pub(super) max_canonical_inward_directions: usize,
    pub(super) max_dual_basis_inversions: usize,
    pub(super) max_affine_half_prisms: usize,
    pub(super) max_halfspaces: usize,
    pub(super) max_plane_triples: usize,
    pub(super) max_singular_plane_triples: usize,
    pub(super) max_nonsingular_solves: usize,
    pub(super) max_membership_tests: usize,
    pub(super) max_corridor_vertices: usize,
    pub(super) max_dedup_comparisons: usize,
    pub(super) max_normal_rank_tests: usize,
    pub(super) max_recession_normal_pairs: usize,
    pub(super) max_signed_recession_tests: usize,
    pub(super) max_recession_membership_tests: usize,
    pub(super) max_prism_vertex_halfspace_tests: usize,
    pub(super) max_corridor_affine_rank_tests: usize,
    pub(super) max_gram_inversions: usize,
    pub(super) max_gram_positive_definite_tests: usize,
    pub(super) max_gram_quadratic_tests: usize,
    pub(super) max_axial_tests: usize,
    pub(super) prism: ExactPrismLimits,
    pub(super) literal_exact: CayleyLimits,
    pub(super) local_exact: CayleyLimits,
    pub(super) exact: CayleyLimits,
}

impl Default for DirectFAffineHingeCorridorLimits {
    fn default() -> Self {
        Self {
            max_authenticated_faces: FACE_COUNT,
            max_authenticated_hinges: HINGE_COUNT,
            max_face_transform_bit_bindings: FACE_TRANSFORM_BIT_BINDINGS,
            max_hinge_parent_transform_bit_bindings: HINGE_PARENT_TRANSFORM_BIT_BINDINGS,
            max_shared_endpoint_equality_tests: SHARED_ENDPOINT_EQUALITY_TESTS,
            max_transform_scalar_lifts: TRANSFORM_SCALAR_LIFTS,
            max_source_coordinate_lifts: SOURCE_COORDINATE_LIFTS,
            max_affine_point_reconstructions: AFFINE_POINT_RECONSTRUCTIONS,
            max_solid_vertex_constructions: SOLID_VERTEX_CONSTRUCTIONS,
            max_thickness_lifts: THICKNESS_LIFTS,
            max_half_thickness_divisions: HALF_THICKNESS_DIVISIONS,
            max_canonical_inward_directions: CANONICAL_INWARD_DIRECTIONS,
            max_dual_basis_inversions: DUAL_BASIS_INVERSIONS,
            max_affine_half_prisms: AFFINE_HALF_PRISMS,
            max_halfspaces: HALFSPACE_COUNT,
            max_plane_triples: PLANE_TRIPLE_COUNT,
            max_singular_plane_triples: PLANE_TRIPLE_COUNT,
            max_nonsingular_solves: PLANE_TRIPLE_COUNT,
            max_membership_tests: MAX_MEMBERSHIP_TESTS,
            max_corridor_vertices: MAX_CORRIDOR_VERTICES,
            max_dedup_comparisons: MAX_DEDUP_COMPARISONS,
            max_normal_rank_tests: NORMAL_RANK_TESTS,
            max_recession_normal_pairs: RECESSION_NORMAL_PAIRS,
            max_signed_recession_tests: SIGNED_RECESSION_TESTS,
            max_recession_membership_tests: RECESSION_MEMBERSHIP_TESTS,
            max_prism_vertex_halfspace_tests: MAX_PRISM_VERTEX_HALFSPACE_TESTS,
            max_corridor_affine_rank_tests: MAX_CORRIDOR_VERTICES,
            max_gram_inversions: GRAM_INVERSIONS,
            max_gram_positive_definite_tests: GRAM_POSITIVE_DEFINITE_TESTS,
            max_gram_quadratic_tests: MAX_GRAM_QUADRATIC_TESTS,
            max_axial_tests: MAX_AXIAL_TESTS,
            prism: ExactPrismLimits::default(),
            literal_exact: literal_f_local_hard_cayley_limits(),
            local_exact: affine_corridor_local_hard_cayley_limits(),
            exact: affine_corridor_combined_hard_cayley_limits(),
        }
    }
}

impl DirectFAffineHingeCorridorLimits {
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
            max_shared_endpoint_equality_tests: self
                .max_shared_endpoint_equality_tests
                .min(hard.max_shared_endpoint_equality_tests),
            max_transform_scalar_lifts: self
                .max_transform_scalar_lifts
                .min(hard.max_transform_scalar_lifts),
            max_source_coordinate_lifts: self
                .max_source_coordinate_lifts
                .min(hard.max_source_coordinate_lifts),
            max_affine_point_reconstructions: self
                .max_affine_point_reconstructions
                .min(hard.max_affine_point_reconstructions),
            max_solid_vertex_constructions: self
                .max_solid_vertex_constructions
                .min(hard.max_solid_vertex_constructions),
            max_thickness_lifts: self.max_thickness_lifts.min(hard.max_thickness_lifts),
            max_half_thickness_divisions: self
                .max_half_thickness_divisions
                .min(hard.max_half_thickness_divisions),
            max_canonical_inward_directions: self
                .max_canonical_inward_directions
                .min(hard.max_canonical_inward_directions),
            max_dual_basis_inversions: self
                .max_dual_basis_inversions
                .min(hard.max_dual_basis_inversions),
            max_affine_half_prisms: self.max_affine_half_prisms.min(hard.max_affine_half_prisms),
            max_halfspaces: self.max_halfspaces.min(hard.max_halfspaces),
            max_plane_triples: self.max_plane_triples.min(hard.max_plane_triples),
            max_singular_plane_triples: self
                .max_singular_plane_triples
                .min(hard.max_singular_plane_triples),
            max_nonsingular_solves: self.max_nonsingular_solves.min(hard.max_nonsingular_solves),
            max_membership_tests: self.max_membership_tests.min(hard.max_membership_tests),
            max_corridor_vertices: self.max_corridor_vertices.min(hard.max_corridor_vertices),
            max_dedup_comparisons: self.max_dedup_comparisons.min(hard.max_dedup_comparisons),
            max_normal_rank_tests: self.max_normal_rank_tests.min(hard.max_normal_rank_tests),
            max_recession_normal_pairs: self
                .max_recession_normal_pairs
                .min(hard.max_recession_normal_pairs),
            max_signed_recession_tests: self
                .max_signed_recession_tests
                .min(hard.max_signed_recession_tests),
            max_recession_membership_tests: self
                .max_recession_membership_tests
                .min(hard.max_recession_membership_tests),
            max_prism_vertex_halfspace_tests: self
                .max_prism_vertex_halfspace_tests
                .min(hard.max_prism_vertex_halfspace_tests),
            max_corridor_affine_rank_tests: self
                .max_corridor_affine_rank_tests
                .min(hard.max_corridor_affine_rank_tests),
            max_gram_inversions: self.max_gram_inversions.min(hard.max_gram_inversions),
            max_gram_positive_definite_tests: self
                .max_gram_positive_definite_tests
                .min(hard.max_gram_positive_definite_tests),
            max_gram_quadratic_tests: self
                .max_gram_quadratic_tests
                .min(hard.max_gram_quadratic_tests),
            max_axial_tests: self.max_axial_tests.min(hard.max_axial_tests),
            prism: self.prism.projected(),
            literal_exact: project_cayley_limits(self.literal_exact, hard.literal_exact),
            local_exact: project_cayley_limits(self.local_exact, hard.local_exact),
            exact: project_cayley_limits(self.exact, hard.exact),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct DirectFAffineHingeCorridorWork {
    pub(super) authenticated_faces: usize,
    pub(super) authenticated_hinges: usize,
    pub(super) face_transform_bit_bindings: usize,
    pub(super) hinge_parent_transform_bit_bindings: usize,
    pub(super) shared_endpoint_equality_tests: usize,
    pub(super) transform_scalar_lifts: usize,
    pub(super) source_coordinate_lifts: usize,
    pub(super) affine_point_reconstructions: usize,
    pub(super) solid_vertex_constructions: usize,
    pub(super) thickness_lifts: usize,
    pub(super) half_thickness_divisions: usize,
    pub(super) canonical_inward_directions: usize,
    pub(super) dual_basis_inversions: usize,
    pub(super) affine_half_prisms: usize,
    pub(super) halfspaces: usize,
    pub(super) plane_triples: usize,
    pub(super) singular_plane_triples: usize,
    pub(super) nonsingular_solves: usize,
    pub(super) membership_tests: usize,
    pub(super) corridor_vertices: usize,
    pub(super) dedup_comparisons: usize,
    pub(super) normal_rank_tests: usize,
    pub(super) recession_normal_pairs: usize,
    pub(super) signed_recession_tests: usize,
    pub(super) recession_membership_tests: usize,
    pub(super) prism_vertex_halfspace_tests: usize,
    pub(super) corridor_affine_rank_tests: usize,
    pub(super) gram_inversions: usize,
    pub(super) gram_positive_definite_tests: usize,
    pub(super) gram_quadratic_tests: usize,
    pub(super) axial_tests: usize,
    pub(super) prism: ExactPrismWork,
    pub(super) phase2b_exact: CayleyWork,
    pub(super) literal_exact: CayleyWork,
    pub(super) local_exact: CayleyWork,
    pub(super) exact: CayleyWork,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DirectFAffineHingeCorridorError {
    ResourceLimitExceeded,
}

#[derive(Debug)]
struct DirectFAffineAuthority<'prerequisite, 'ef, 'exact_e_corridor, 'exact, 'pose> {
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
    binary64_face_transforms: [BoundBinary64FaceTransformBits; FACE_COUNT],
    hinge_parent_transform: BoundBinary64FaceTransformBits,
    phase2b_work: ExactEFiniteHingeCorridorWork,
}

#[derive(Debug)]
struct DirectFAffineCorridorDiagnosticGeometry {
    half_thickness: BigRational,
    length_squared: BigRational,
    radius_squared: BigRational,
    corridor_vertex_count: usize,
    corridor_affine_rank: u8,
    shared_endpoint_mismatch_count: usize,
}

/// A sealed private geometry diagnostic only.
///
/// This type deliberately carries no collision-admission, allowance, safe-set,
/// mutation, or production authority. In particular, `P_F` containment in the
/// affine corridor does not prove that the literal-F parent/child boundary
/// drift is admissible. A future production gate must additionally borrow a
/// separately sealed shared-hinge admission capability.
#[derive(Debug)]
pub(super) struct DirectFAffineHingeCorridorDiagnosticV1<
    'prerequisite,
    'ef,
    'exact_e_corridor,
    'exact,
    'pose,
> {
    authority: DirectFAffineAuthority<'prerequisite, 'ef, 'exact_e_corridor, 'exact, 'pose>,
    geometry: DirectFAffineCorridorDiagnosticGeometry,
    sealed_work: Option<DirectFAffineHingeCorridorWork>,
}

impl DirectFAffineHingeCorridorDiagnosticV1<'_, '_, '_, '_, '_> {
    pub(super) fn interaction_kind(&self) -> ExactEFiniteHingeInteractionKind {
        self.authority.interaction_kind
    }

    pub(super) fn half_thickness(&self) -> &BigRational {
        &self.geometry.half_thickness
    }

    pub(super) fn length_squared(&self) -> &BigRational {
        &self.geometry.length_squared
    }

    pub(super) fn radius_squared(&self) -> &BigRational {
        &self.geometry.radius_squared
    }

    pub(super) fn corridor_vertex_count(&self) -> usize {
        self.geometry.corridor_vertex_count
    }

    pub(super) fn corridor_affine_rank(&self) -> u8 {
        self.geometry.corridor_affine_rank
    }

    pub(super) fn shared_endpoint_mismatch_count(&self) -> usize {
        self.geometry.shared_endpoint_mismatch_count
    }

    #[cfg(test)]
    pub(super) fn toggle_face_transform_lsb_for_test(
        &mut self,
        face_index: usize,
        coefficient_index: usize,
    ) {
        let transform = &mut self.authority.binary64_face_transforms[face_index];
        if coefficient_index < 9 {
            transform.rotation[coefficient_index / 3][coefficient_index % 3] ^= 1;
        } else {
            transform.translation[coefficient_index - 9] ^= 1;
        }
    }

    #[cfg(test)]
    pub(super) fn toggle_hinge_parent_transform_lsb_for_test(&mut self, coefficient_index: usize) {
        let transform = &mut self.authority.hinge_parent_transform;
        if coefficient_index < 9 {
            transform.rotation[coefficient_index / 3][coefficient_index % 3] ^= 1;
        } else {
            transform.translation[coefficient_index - 9] ^= 1;
        }
    }
}

/// A sealed private Outside diagnostic, never collision or admission authority.
#[derive(Debug)]
pub(super) struct DirectFAffineHingeCorridorOutsideV1<
    'prerequisite,
    'ef,
    'exact_e_corridor,
    'exact,
    'pose,
> {
    authority: DirectFAffineAuthority<'prerequisite, 'ef, 'exact_e_corridor, 'exact, 'pose>,
    pub(super) first_outside_vertex_index: usize,
    pub(super) outside_vertex_count: usize,
    pub(super) violated_halfspace_count: usize,
    pub(super) axial_outside_vertex_count: usize,
    sealed_work: Option<DirectFAffineHingeCorridorWork>,
}

#[derive(Debug)]
pub(super) enum DirectFAffineHingeCorridorResult<
    'prerequisite,
    'ef,
    'exact_e_corridor,
    'exact,
    'pose,
> {
    ContainedUnadmitted(
        Box<
            DirectFAffineHingeCorridorDiagnosticV1<
                'prerequisite,
                'ef,
                'exact_e_corridor,
                'exact,
                'pose,
            >,
        >,
    ),
    Outside(
        Box<
            DirectFAffineHingeCorridorOutsideV1<
                'prerequisite,
                'ef,
                'exact_e_corridor,
                'exact,
                'pose,
            >,
        >,
    ),
    LayerOffsetUnmodeled,
    Unresolved,
}

#[derive(Debug)]
pub(super) struct DirectFAffineHingeCorridorAnalysis<
    'prerequisite,
    'ef,
    'exact_e_corridor,
    'exact,
    'pose,
> {
    pub(super) result:
        DirectFAffineHingeCorridorResult<'prerequisite, 'ef, 'exact_e_corridor, 'exact, 'pose>,
    pub(super) work: DirectFAffineHingeCorridorWork,
}

impl<'prerequisite, 'ef, 'exact_e_corridor, 'exact, 'pose>
    DirectFAffineHingeCorridorAnalysis<'prerequisite, 'ef, 'exact_e_corridor, 'exact, 'pose>
{
    /// Returns sealed diagnostic provenance. This is not collision-admission
    /// authority and must not be consumed by production collision policy.
    pub(super) fn sealed_diagnostic_result_and_work(
        &self,
    ) -> Option<(
        &DirectFAffineHingeCorridorResult<'prerequisite, 'ef, 'exact_e_corridor, 'exact, 'pose>,
        &DirectFAffineHingeCorridorWork,
    )> {
        let sealed = match &self.result {
            DirectFAffineHingeCorridorResult::ContainedUnadmitted(diagnostic) => {
                diagnostic.sealed_work.as_ref()?
            }
            DirectFAffineHingeCorridorResult::Outside(outside) => outside.sealed_work.as_ref()?,
            DirectFAffineHingeCorridorResult::LayerOffsetUnmodeled
            | DirectFAffineHingeCorridorResult::Unresolved => return None,
        };
        if sealed != &self.work {
            return None;
        }
        Some((&self.result, sealed))
    }
}

fn preflight_affine_exact_capacity(
    phase2b: &ExactEFiniteHingeCorridorWork,
    limits: &DirectFAffineHingeCorridorLimits,
) -> Result<(), CayleyError> {
    CayleyWork::default().checked_merge(&phase2b.exact, &limits.exact, None, STAGE)?;
    for local in [
        &limits.literal_exact,
        &limits.prism.exact,
        &limits.local_exact,
    ] {
        for (local_maximum, outer_maximum, resource) in [
            (
                local.max_precision_rounds,
                limits.exact.max_precision_rounds,
                "affine_reserved_precision_rounds",
            ),
            (
                local.max_guard_bits,
                limits.exact.max_guard_bits,
                "affine_reserved_guard_bits",
            ),
            (
                local.max_candidate_bits,
                limits.exact.max_candidate_bits,
                "affine_reserved_candidate_bits",
            ),
            (
                local.max_machin_terms_per_series,
                limits.exact.max_machin_terms_per_series,
                "affine_reserved_machin_terms",
            ),
            (
                local.max_trig_terms_per_series,
                limits.exact.max_trig_terms_per_series,
                "affine_reserved_trig_terms",
            ),
            (
                local.max_sqrt_refinements,
                limits.exact.max_sqrt_refinements,
                "affine_reserved_sqrt_refinements",
            ),
            (
                local.max_shift_bits,
                limits.exact.max_shift_bits,
                "affine_reserved_shift_bits",
            ),
            (
                local.max_intermediate_bits,
                limits.exact.max_intermediate_bits,
                "affine_reserved_intermediate_bits",
            ),
            (
                local.max_rational_allocation_bits,
                limits.exact.max_rational_allocation_bits,
                "affine_reserved_rational_allocation_bits",
            ),
            (
                local.max_output_bits,
                limits.exact.max_output_bits,
                "affine_reserved_output_bits",
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
    for (consumed, literal, prism, local, outer, resource) in [
        (
            phase2b.exact.interval_operations,
            limits.literal_exact.max_interval_operations,
            limits.prism.exact.max_interval_operations,
            limits.local_exact.max_interval_operations,
            limits.exact.max_interval_operations,
            "affine_reserved_interval_operations",
        ),
        (
            phase2b.exact.gcd_fallback_calls,
            limits.literal_exact.max_gcd_fallback_calls,
            limits.prism.exact.max_gcd_fallback_calls,
            limits.local_exact.max_gcd_fallback_calls,
            limits.exact.max_gcd_fallback_calls,
            "affine_reserved_gcd_fallback_calls",
        ),
        (
            phase2b.exact.gcd_fallback_input_bits,
            limits.literal_exact.max_gcd_fallback_input_bits,
            limits.prism.exact.max_gcd_fallback_input_bits,
            limits.local_exact.max_gcd_fallback_input_bits,
            limits.exact.max_gcd_fallback_input_bits,
            "affine_reserved_gcd_fallback_input_bits",
        ),
        (
            phase2b.exact.rational_allocations,
            limits.literal_exact.max_rational_allocations,
            limits.prism.exact.max_rational_allocations,
            limits.local_exact.max_rational_allocations,
            limits.exact.max_rational_allocations,
            "affine_reserved_rational_allocations",
        ),
        (
            phase2b.exact.total_rational_allocation_bits,
            limits.literal_exact.max_total_rational_allocation_bits,
            limits.prism.exact.max_total_rational_allocation_bits,
            limits.local_exact.max_total_rational_allocation_bits,
            limits.exact.max_total_rational_allocation_bits,
            "affine_reserved_total_rational_allocation_bits",
        ),
    ] {
        let reserved = consumed
            .checked_add(literal)
            .and_then(|value| value.checked_add(prism))
            .and_then(|value| value.checked_add(local))
            .ok_or(CayleyError::ResourceLimitExceeded {
                stage: STAGE,
                resource,
            })?;
        if reserved > outer {
            return Err(CayleyError::ResourceLimitExceeded {
                stage: STAGE,
                resource,
            });
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn analyze_direct_f_affine_hinge_corridor_v1<
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
    limits: DirectFAffineHingeCorridorLimits,
) -> Result<
    DirectFAffineHingeCorridorAnalysis<'prerequisite, 'ef, 'exact_e_corridor, 'exact, 'pose>,
    DirectFAffineHingeCorridorError,
> {
    let limits = limits.projected();
    if !positive_finite_binary64(paper_thickness_mm) {
        return Ok(DirectFAffineHingeCorridorAnalysis {
            result: DirectFAffineHingeCorridorResult::Unresolved,
            work: DirectFAffineHingeCorridorWork::default(),
        });
    }
    let Some((_, phase2b_work)) = exact_e_analysis.authenticated_contained_capability_and_work()
    else {
        let result = match exact_e_analysis.result {
            ExactEFiniteHingeCorridorResult::LayerOffsetUnmodeled => {
                DirectFAffineHingeCorridorResult::LayerOffsetUnmodeled
            }
            ExactEFiniteHingeCorridorResult::Contained(_)
            | ExactEFiniteHingeCorridorResult::Outside(_)
            | ExactEFiniteHingeCorridorResult::Unresolved => {
                DirectFAffineHingeCorridorResult::Unresolved
            }
        };
        return Ok(DirectFAffineHingeCorridorAnalysis {
            result,
            work: DirectFAffineHingeCorridorWork::default(),
        });
    };
    if let Err(error) = preflight_affine_exact_capacity(phase2b_work, &limits) {
        return match error {
            CayleyError::ResourceLimitExceeded { .. } => {
                Err(DirectFAffineHingeCorridorError::ResourceLimitExceeded)
            }
            _ => Ok(DirectFAffineHingeCorridorAnalysis {
                result: DirectFAffineHingeCorridorResult::Unresolved,
                work: DirectFAffineHingeCorridorWork::default(),
            }),
        };
    }

    let mut work = DirectFAffineHingeCorridorWork {
        phase2b_exact: phase2b_work.exact.clone(),
        exact: phase2b_work.exact.clone(),
        ..DirectFAffineHingeCorridorWork::default()
    };
    let mut literal_meter = WorkMeter::new(&limits.literal_exact);
    let mut prism_meter = WorkMeter::new(&limits.prism.exact);
    let mut local_meter = WorkMeter::new(&limits.local_exact);
    let result = calculate_direct_f_affine_hinge_corridor_v1(
        prerequisite_analysis,
        ef_boundary,
        exact_e_analysis,
        phase2b_work,
        exact,
        bound,
        paper_thickness_mm,
        &limits,
        &mut work,
        &mut literal_meter,
        &mut prism_meter,
        &mut local_meter,
    );
    match result {
        Ok(mut result) => {
            let literal_exact = literal_meter.work;
            let prism_exact = prism_meter.work;
            let local_exact = local_meter.work;
            if work.prism.exact != prism_exact {
                return Ok(unresolved_after_phase2b(phase2b_work));
            }
            let cumulative = phase2b_work
                .exact
                .checked_merge(&literal_exact, &limits.exact, None, STAGE)
                .and_then(|value| value.checked_merge(&prism_exact, &limits.exact, None, STAGE))
                .and_then(|value| value.checked_merge(&local_exact, &limits.exact, None, STAGE));
            let cumulative = match cumulative {
                Ok(cumulative) => cumulative,
                Err(CayleyError::ResourceLimitExceeded { .. }) => {
                    return Err(DirectFAffineHingeCorridorError::ResourceLimitExceeded);
                }
                Err(_) => return Ok(unresolved_after_phase2b(phase2b_work)),
            };
            work.literal_exact = literal_exact;
            work.local_exact = local_exact;
            work.exact = cumulative;
            match &mut result {
                DirectFAffineHingeCorridorResult::ContainedUnadmitted(diagnostic) => {
                    diagnostic.sealed_work = Some(work.clone());
                }
                DirectFAffineHingeCorridorResult::Outside(outside) => {
                    outside.sealed_work = Some(work.clone());
                }
                DirectFAffineHingeCorridorResult::LayerOffsetUnmodeled
                | DirectFAffineHingeCorridorResult::Unresolved => {}
            }
            Ok(DirectFAffineHingeCorridorAnalysis { result, work })
        }
        Err(CayleyError::ResourceLimitExceeded { .. }) => {
            Err(DirectFAffineHingeCorridorError::ResourceLimitExceeded)
        }
        Err(_) => Ok(unresolved_after_phase2b(phase2b_work)),
    }
}

fn unresolved_after_phase2b<'prerequisite, 'ef, 'exact_e_corridor, 'exact, 'pose>(
    phase2b: &ExactEFiniteHingeCorridorWork,
) -> DirectFAffineHingeCorridorAnalysis<'prerequisite, 'ef, 'exact_e_corridor, 'exact, 'pose> {
    DirectFAffineHingeCorridorAnalysis {
        result: DirectFAffineHingeCorridorResult::Unresolved,
        work: DirectFAffineHingeCorridorWork {
            phase2b_exact: phase2b.exact.clone(),
            exact: phase2b.exact.clone(),
            ..DirectFAffineHingeCorridorWork::default()
        },
    }
}

#[derive(Debug)]
struct LiteralAffineFace {
    face: FaceId,
    transform: ExactRigidTransform,
    source_vertices: [ExactPoint3; 3],
    source_vertex_ids: [ori_domain::VertexId; 3],
    world_vertices: [ExactPoint3; 3],
    solid_vertices: [ExactPoint3; 6],
}

#[derive(Debug)]
struct CanonicalAffineFrame {
    start: ExactPoint3,
    axis: ExactVector3,
    inward: ExactVector3,
    thickness: ExactVector3,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ClosedAffineHalfspace {
    normal: ExactVector3,
    offset: BigRational,
    face_index: usize,
    constraint_index: usize,
}

#[derive(Debug)]
struct ParentGramGeometry {
    axis_start: ExactPoint3,
    axis: ExactVector3,
    gram: [[BigRational; 3]; 3],
    length_squared: BigRational,
}

#[derive(Debug, Default)]
struct CompleteOutsideScan {
    first_outside_vertex_index: Option<usize>,
    outside_vertex_count: usize,
    violated_halfspace_count: usize,
}

#[allow(clippy::too_many_arguments)]
fn calculate_direct_f_affine_hinge_corridor_v1<
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
    limits: &DirectFAffineHingeCorridorLimits,
    work: &mut DirectFAffineHingeCorridorWork,
    literal_meter: &mut WorkMeter<'_>,
    prism_meter: &mut WorkMeter<'_>,
    local_meter: &mut WorkMeter<'_>,
) -> Result<
    DirectFAffineHingeCorridorResult<'prerequisite, 'ef, 'exact_e_corridor, 'exact, 'pose>,
    CayleyError,
> {
    let prerequisite = match &prerequisite_analysis.result {
        SingleTriangularHingePrerequisiteResult::Authenticated(capability) => capability,
        SingleTriangularHingePrerequisiteResult::LayerOffsetUnmodeled => {
            return Ok(DirectFAffineHingeCorridorResult::LayerOffsetUnmodeled);
        }
        SingleTriangularHingePrerequisiteResult::Unresolved => {
            return Ok(DirectFAffineHingeCorridorResult::Unresolved);
        }
    };
    let Some(ef_boundary) = ef_boundary else {
        return Ok(DirectFAffineHingeCorridorResult::Unresolved);
    };
    let exact_e_corridor = match &exact_e_analysis.result {
        ExactEFiniteHingeCorridorResult::Contained(capability) => capability.as_ref(),
        ExactEFiniteHingeCorridorResult::LayerOffsetUnmodeled => {
            return Ok(DirectFAffineHingeCorridorResult::LayerOffsetUnmodeled);
        }
        ExactEFiniteHingeCorridorResult::Outside(_)
        | ExactEFiniteHingeCorridorResult::Unresolved => {
            return Ok(DirectFAffineHingeCorridorResult::Unresolved);
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
        || exact_e_corridor.sealed_work() != Some(phase2b_work)
    {
        return Ok(DirectFAffineHingeCorridorResult::Unresolved);
    }

    charge_literal_fixed_work(work, limits)?;
    let left_face_index = prerequisite.left_face_index;
    let right_face_index = prerequisite.right_face_index;
    let hinge_index = prerequisite.hinge_index;
    if exact.faces.len() != FACE_COUNT
        || exact.hinges.len() != HINGE_COUNT
        || left_face_index == right_face_index
        || left_face_index >= FACE_COUNT
        || right_face_index >= FACE_COUNT
        || hinge_index >= HINGE_COUNT
        || exact.faces.iter().any(|face| face.boundary.len() != 3)
    {
        return Ok(DirectFAffineHingeCorridorResult::Unresolved);
    }

    let thickness = exact_f64(paper_thickness_mm, literal_meter, STAGE)?;
    let two = BigRational::from_integer(2.into());
    let half_thickness = literal_meter.divide_rational(&thickness, &two, STAGE)?;
    if literal_meter.compare_rational(&half_thickness, &BigRational::zero(), STAGE)?
        != Ordering::Greater
    {
        return Ok(DirectFAffineHingeCorridorResult::Unresolved);
    }

    let sealed_face_transforms = ef_boundary.binary64_face_transforms;
    let faces = [
        reconstruct_literal_affine_face(
            exact,
            bound,
            0,
            &sealed_face_transforms[0],
            &half_thickness,
            literal_meter,
        )?,
        reconstruct_literal_affine_face(
            exact,
            bound,
            1,
            &sealed_face_transforms[1],
            &half_thickness,
            literal_meter,
        )?,
    ];
    let shared_endpoint_mismatch_count =
        compare_literal_shared_hinge_endpoints(&faces, exact, hinge_index, limits, work)?;
    let hinge_parent_transform =
        authenticate_hinge_parent_transform(exact, bound, hinge_index, &sealed_face_transforms)
            .ok_or(CayleyError::InvariantFailure { stage: STAGE })?;

    let first_prism = ExactPrebuiltTriangularPrismView {
        vertices: std::array::from_fn(|index| &faces[0].solid_vertices[index]),
    };
    let second_prism = ExactPrebuiltTriangularPrismView {
        vertices: std::array::from_fn(|index| &faces[1].solid_vertices[index]),
    };
    let Some(report) = analyze_exact_prebuilt_prism_pair_with_meter_v1(
        first_prism,
        second_prism,
        limits.prism,
        &mut work.prism,
        prism_meter,
    )?
    else {
        return Ok(DirectFAffineHingeCorridorResult::Unresolved);
    };
    let Some(interaction_kind) = authenticate_interaction_kind(&report) else {
        return Ok(DirectFAffineHingeCorridorResult::Unresolved);
    };
    if interaction_kind != exact_e_corridor.interaction_kind() {
        return Ok(DirectFAffineHingeCorridorResult::Unresolved);
    }

    let Some(halfspaces) = build_affine_corridor_halfspaces(
        &faces,
        exact,
        hinge_index,
        &half_thickness,
        limits,
        work,
        local_meter,
    )?
    else {
        return Ok(DirectFAffineHingeCorridorResult::Unresolved);
    };
    let outside = scan_complete_prism_vertices_against_halfspaces(
        &report,
        &halfspaces,
        limits,
        work,
        local_meter,
    )?;
    if let Some(first_outside_vertex_index) = outside.first_outside_vertex_index {
        return Ok(DirectFAffineHingeCorridorResult::Outside(Box::new(
            DirectFAffineHingeCorridorOutsideV1 {
                authority: make_authority(
                    prerequisite,
                    ef_boundary,
                    exact_e_corridor,
                    exact,
                    bound,
                    paper_thickness_mm,
                    sealed_face_transforms,
                    hinge_parent_transform,
                    phase2b_work,
                ),
                first_outside_vertex_index,
                outside_vertex_count: outside.outside_vertex_count,
                violated_halfspace_count: outside.violated_halfspace_count,
                axial_outside_vertex_count: 0,
                sealed_work: None,
            },
        )));
    }

    // `P_F` is a positive interaction and every one of its vertices is in
    // `C_F`; therefore `C_F` is nonempty before recession is interpreted.
    let corridor_vertices =
        enumerate_complete_corridor_vertices(&halfspaces, limits, work, local_meter)?;
    if corridor_vertices.is_empty() {
        return Ok(DirectFAffineHingeCorridorResult::Unresolved);
    }
    let normal_rank = authenticate_normal_rank(&halfspaces, limits, work, local_meter)?;
    if normal_rank != 3 {
        return Ok(DirectFAffineHingeCorridorResult::LayerOffsetUnmodeled);
    }
    if authenticate_bounded_recession_cone(&halfspaces, limits, work, local_meter)? {
        return Ok(DirectFAffineHingeCorridorResult::LayerOffsetUnmodeled);
    }
    let corridor_affine_rank =
        authenticate_point_affine_rank(&corridor_vertices, limits, work, local_meter)?;
    if corridor_affine_rank < 2 {
        return Ok(DirectFAffineHingeCorridorResult::Unresolved);
    }

    let parent_index = exact
        .faces
        .iter()
        .position(|face| face.face == exact.hinges[hinge_index].parent)
        .ok_or(CayleyError::InvariantFailure { stage: STAGE })?;
    let parent_gram = build_parent_gram_geometry(
        &faces[parent_index],
        exact,
        hinge_index,
        limits,
        work,
        local_meter,
    )?;
    let (radius_squared, radius_exceeds_length) =
        scan_corridor_gram_radius(&corridor_vertices, &parent_gram, limits, work, local_meter)?;
    let axial_outside =
        scan_complete_prism_axial_bounds(&report, &parent_gram, limits, work, local_meter)?;
    if radius_exceeds_length {
        return Ok(DirectFAffineHingeCorridorResult::LayerOffsetUnmodeled);
    }
    if let Some((first_outside_vertex_index, outside_vertex_count)) = axial_outside {
        return Ok(DirectFAffineHingeCorridorResult::Outside(Box::new(
            DirectFAffineHingeCorridorOutsideV1 {
                authority: make_authority(
                    prerequisite,
                    ef_boundary,
                    exact_e_corridor,
                    exact,
                    bound,
                    paper_thickness_mm,
                    sealed_face_transforms,
                    hinge_parent_transform,
                    phase2b_work,
                ),
                first_outside_vertex_index,
                outside_vertex_count,
                violated_halfspace_count: 0,
                axial_outside_vertex_count: outside_vertex_count,
                sealed_work: None,
            },
        )));
    }

    Ok(DirectFAffineHingeCorridorResult::ContainedUnadmitted(
        Box::new(DirectFAffineHingeCorridorDiagnosticV1 {
            authority: make_authority(
                prerequisite,
                ef_boundary,
                exact_e_corridor,
                exact,
                bound,
                paper_thickness_mm,
                sealed_face_transforms,
                hinge_parent_transform,
                phase2b_work,
            ),
            geometry: DirectFAffineCorridorDiagnosticGeometry {
                half_thickness,
                length_squared: parent_gram.length_squared,
                radius_squared,
                corridor_vertex_count: corridor_vertices.len(),
                corridor_affine_rank,
                shared_endpoint_mismatch_count,
            },
            sealed_work: None,
        }),
    ))
}

#[allow(clippy::too_many_arguments)]
fn make_authority<'prerequisite, 'ef, 'exact_e_corridor, 'exact, 'pose>(
    prerequisite: &'prerequisite AuthenticatedSingleTriangularHingePrerequisitesV1<'exact, 'pose>,
    ef_boundary: &'ef AxisAlignedEfBoundaryCapabilityV1<'prerequisite, 'exact, 'pose>,
    exact_e_corridor: &'exact_e_corridor ExactEFiniteHingeCorridorCapabilityV1<
        'prerequisite,
        'ef,
        'exact,
        'pose,
    >,
    exact: &'exact RationalCayleyTreePose<'pose>,
    bound: BoundMaterialTreePose<'pose>,
    paper_thickness_mm: f64,
    binary64_face_transforms: [BoundBinary64FaceTransformBits; FACE_COUNT],
    hinge_parent_transform: BoundBinary64FaceTransformBits,
    phase2b_work: &ExactEFiniteHingeCorridorWork,
) -> DirectFAffineAuthority<'prerequisite, 'ef, 'exact_e_corridor, 'exact, 'pose> {
    DirectFAffineAuthority {
        prerequisite,
        ef_boundary,
        exact_e_corridor,
        exact,
        bound,
        paper_thickness_bits: paper_thickness_mm.to_bits(),
        left_face_index: prerequisite.left_face_index,
        right_face_index: prerequisite.right_face_index,
        hinge_index: prerequisite.hinge_index,
        interaction_kind: exact_e_corridor.interaction_kind(),
        binary64_face_transforms,
        hinge_parent_transform,
        phase2b_work: phase2b_work.clone(),
    }
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

fn charge_literal_fixed_work(
    work: &mut DirectFAffineHingeCorridorWork,
    limits: &DirectFAffineHingeCorridorLimits,
) -> Result<(), CayleyError> {
    for (counter, required, maximum, resource) in [
        (
            &mut work.authenticated_faces,
            FACE_COUNT,
            limits.max_authenticated_faces,
            "affine_corridor_faces",
        ),
        (
            &mut work.authenticated_hinges,
            HINGE_COUNT,
            limits.max_authenticated_hinges,
            "affine_corridor_hinges",
        ),
        (
            &mut work.face_transform_bit_bindings,
            FACE_TRANSFORM_BIT_BINDINGS,
            limits.max_face_transform_bit_bindings,
            "affine_corridor_face_transform_bits",
        ),
        (
            &mut work.hinge_parent_transform_bit_bindings,
            HINGE_PARENT_TRANSFORM_BIT_BINDINGS,
            limits.max_hinge_parent_transform_bit_bindings,
            "affine_corridor_hinge_parent_transform_bits",
        ),
        (
            &mut work.transform_scalar_lifts,
            TRANSFORM_SCALAR_LIFTS,
            limits.max_transform_scalar_lifts,
            "affine_corridor_transform_lifts",
        ),
        (
            &mut work.source_coordinate_lifts,
            SOURCE_COORDINATE_LIFTS,
            limits.max_source_coordinate_lifts,
            "affine_corridor_source_lifts",
        ),
        (
            &mut work.affine_point_reconstructions,
            AFFINE_POINT_RECONSTRUCTIONS,
            limits.max_affine_point_reconstructions,
            "affine_corridor_affine_points",
        ),
        (
            &mut work.solid_vertex_constructions,
            SOLID_VERTEX_CONSTRUCTIONS,
            limits.max_solid_vertex_constructions,
            "affine_corridor_solid_vertices",
        ),
        (
            &mut work.thickness_lifts,
            THICKNESS_LIFTS,
            limits.max_thickness_lifts,
            "affine_corridor_thickness_lifts",
        ),
        (
            &mut work.half_thickness_divisions,
            HALF_THICKNESS_DIVISIONS,
            limits.max_half_thickness_divisions,
            "affine_corridor_half_thickness_divisions",
        ),
    ] {
        set_fixed_counter(counter, required, maximum, resource)?;
    }
    Ok(())
}

fn reconstruct_literal_affine_face(
    exact: &RationalCayleyTreePose<'_>,
    bound: BoundMaterialTreePose<'_>,
    face_index: usize,
    sealed_bits: &BoundBinary64FaceTransformBits,
    half_thickness: &BigRational,
    meter: &mut WorkMeter<'_>,
) -> Result<LiteralAffineFace, CayleyError> {
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
    let source_vertex_ids: [ori_domain::VertexId; 3] = source_boundary
        .vertices()
        .try_into()
        .map_err(|_| CayleyError::InvariantFailure { stage: STAGE })?;
    let transform = lift_transform_bits(sealed_bits, meter)?;
    let mut source_vertices = Vec::with_capacity(3);
    let mut world_vertices = Vec::with_capacity(3);
    for vertex in source_vertex_ids {
        let source = bound
            .model()
            .vertex_position(vertex)
            .ok_or(CayleyError::InvariantFailure { stage: STAGE })?;
        let source = lift_point3(source, meter)?;
        let world = apply_exact_transform(&transform, &source, meter)?;
        source_vertices.push(source);
        world_vertices.push(world);
    }
    let source_vertices: [ExactPoint3; 3] = source_vertices
        .try_into()
        .map_err(|_| CayleyError::InvariantFailure { stage: STAGE })?;
    let world_vertices: [ExactPoint3; 3] = world_vertices
        .try_into()
        .map_err(|_| CayleyError::InvariantFailure { stage: STAGE })?;
    let thickness_direction = ExactVector3 {
        coordinates: try_array3(|axis| meter.clone_rational(&transform.rotation[axis][1], STAGE))?,
    };
    let material_offset = exact_scale_vector(&thickness_direction, half_thickness, meter)?;
    let solid_vertices = [
        exact_offset_point(&world_vertices[0], &material_offset, true, meter)?,
        exact_offset_point(&world_vertices[1], &material_offset, true, meter)?,
        exact_offset_point(&world_vertices[2], &material_offset, true, meter)?,
        exact_offset_point(&world_vertices[0], &material_offset, false, meter)?,
        exact_offset_point(&world_vertices[1], &material_offset, false, meter)?,
        exact_offset_point(&world_vertices[2], &material_offset, false, meter)?,
    ];
    Ok(LiteralAffineFace {
        face: face.face,
        transform,
        source_vertices,
        source_vertex_ids,
        world_vertices,
        solid_vertices,
    })
}

fn authenticate_hinge_parent_transform(
    exact: &RationalCayleyTreePose<'_>,
    bound: BoundMaterialTreePose<'_>,
    hinge_index: usize,
    sealed_face_transforms: &[BoundBinary64FaceTransformBits; FACE_COUNT],
) -> Option<BoundBinary64FaceTransformBits> {
    let exact_hinge = exact.hinges.get(hinge_index)?;
    let native_hinge = bound.model().hinges().get(hinge_index)?;
    if native_hinge.edge() != exact_hinge.edge {
        return None;
    }
    let parent_face_index = exact
        .faces
        .iter()
        .position(|face| face.face == exact_hinge.parent)?;
    let expected_parent = sealed_face_transforms.get(parent_face_index)?;
    let native = bound.pose().hinge_parent_transform(exact_hinge.edge)?;
    let observed = capture_transform_bits(exact_hinge.parent, native);
    (observed == *expected_parent
        && capture_face_transform_bits(bound, exact_hinge.parent).as_ref() == Some(expected_parent))
    .then_some(observed)
}

fn compare_literal_shared_hinge_endpoints(
    faces: &[LiteralAffineFace; FACE_COUNT],
    exact: &RationalCayleyTreePose<'_>,
    hinge_index: usize,
    limits: &DirectFAffineHingeCorridorLimits,
    work: &mut DirectFAffineHingeCorridorWork,
) -> Result<usize, CayleyError> {
    let exact_hinge = exact
        .hinges
        .get(hinge_index)
        .ok_or(CayleyError::InvariantFailure { stage: STAGE })?;
    let tests_before = work.shared_endpoint_equality_tests;
    let mut mismatch_count = 0_usize;
    for endpoint in exact_hinge.endpoint_vertices {
        charge_counter(
            &mut work.shared_endpoint_equality_tests,
            limits.max_shared_endpoint_equality_tests,
            "affine_corridor_shared_endpoint_equality_tests",
        )?;
        let left_index = faces[0]
            .source_vertex_ids
            .iter()
            .position(|vertex| *vertex == endpoint)
            .ok_or(CayleyError::InvariantFailure { stage: STAGE })?;
        let right_index = faces[1]
            .source_vertex_ids
            .iter()
            .position(|vertex| *vertex == endpoint)
            .ok_or(CayleyError::InvariantFailure { stage: STAGE })?;
        if !canonical_point_eq(
            &faces[0].world_vertices[left_index],
            &faces[1].world_vertices[right_index],
        ) {
            mismatch_count =
                mismatch_count
                    .checked_add(1)
                    .ok_or(CayleyError::ResourceLimitExceeded {
                        stage: STAGE,
                        resource: "affine_corridor_shared_endpoint_mismatches",
                    })?;
        }
    }
    if checked_counter_delta(
        work.shared_endpoint_equality_tests,
        tests_before,
        "affine_corridor_shared_endpoint_equality_tests",
    )? != SHARED_ENDPOINT_EQUALITY_TESTS
    {
        return Err(CayleyError::InvariantFailure { stage: STAGE });
    }
    Ok(mismatch_count)
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

fn build_affine_corridor_halfspaces(
    faces: &[LiteralAffineFace; FACE_COUNT],
    exact: &RationalCayleyTreePose<'_>,
    hinge_index: usize,
    half_thickness: &BigRational,
    limits: &DirectFAffineHingeCorridorLimits,
    work: &mut DirectFAffineHingeCorridorWork,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<[ClosedAffineHalfspace; HALFSPACE_COUNT]>, CayleyError> {
    let exact_hinge = exact
        .hinges
        .get(hinge_index)
        .ok_or(CayleyError::InvariantFailure { stage: STAGE })?;
    let mut halfspaces = Vec::with_capacity(HALFSPACE_COUNT);
    for (face_index, face) in faces.iter().enumerate() {
        if face.face != exact.faces[face_index].face {
            return Ok(None);
        }
        let Some(start_index) = face
            .source_vertex_ids
            .iter()
            .position(|vertex| *vertex == exact_hinge.endpoint_vertices[0])
        else {
            return Ok(None);
        };
        let Some(end_index) = face
            .source_vertex_ids
            .iter()
            .position(|vertex| *vertex == exact_hinge.endpoint_vertices[1])
        else {
            return Ok(None);
        };
        if start_index == end_index {
            return Ok(None);
        }
        let opposite_indexes: Vec<usize> = (0..3)
            .filter(|index| *index != start_index && *index != end_index)
            .collect();
        let [opposite_index] = opposite_indexes.as_slice() else {
            return Ok(None);
        };
        let source_start = &face.source_vertices[start_index];
        let source_end = &face.source_vertices[end_index];
        let source_opposite = &face.source_vertices[*opposite_index];
        let source_axis = exact_between(source_start, source_end, meter)?;
        let source_to_opposite = exact_between(source_start, source_opposite, meter)?;
        let axis_squared = exact_dot(&source_axis, &source_axis, meter)?;
        if meter.compare_rational(&axis_squared, &BigRational::zero(), STAGE)? != Ordering::Greater
        {
            return Ok(None);
        }
        let projection_numerator = exact_dot(&source_to_opposite, &source_axis, meter)?;
        let projection = meter.divide_rational(&projection_numerator, &axis_squared, STAGE)?;
        let projected_axis = exact_scale_vector(&source_axis, &projection, meter)?;
        let source_inward = exact_subtract_vectors(&source_to_opposite, &projected_axis, meter)?;
        let inward_squared = exact_dot(&source_inward, &source_inward, meter)?;
        let perpendicular = exact_dot(&source_axis, &source_inward, meter)?;
        let inward_support = exact_dot(&source_inward, &source_to_opposite, meter)?;
        if meter.compare_rational(&inward_squared, &BigRational::zero(), STAGE)?
            != Ordering::Greater
            || meter.compare_rational(&inward_support, &inward_squared, STAGE)? != Ordering::Equal
            || !perpendicular.is_zero()
        {
            return Ok(None);
        }
        charge_counter(
            &mut work.canonical_inward_directions,
            limits.max_canonical_inward_directions,
            "affine_corridor_canonical_inward_directions",
        )?;

        let frame = CanonicalAffineFrame {
            start: clone_point(&face.world_vertices[start_index], meter)?,
            axis: apply_exact_linear(&face.transform.rotation, &source_axis, meter)?,
            inward: apply_exact_linear(&face.transform.rotation, &source_inward, meter)?,
            thickness: ExactVector3 {
                coordinates: try_array3(|axis| {
                    meter.clone_rational(&face.transform.rotation[axis][1], STAGE)
                })?,
            },
        };
        let Some(face_halfspaces) =
            build_frame_halfspaces(&frame, half_thickness, face_index, limits, work, meter)?
        else {
            return Ok(None);
        };
        halfspaces.extend(face_halfspaces);
    }
    let halfspaces: [ClosedAffineHalfspace; HALFSPACE_COUNT] = halfspaces
        .try_into()
        .map_err(|_| CayleyError::InvariantFailure { stage: STAGE })?;
    Ok(Some(halfspaces))
}

fn build_frame_halfspaces(
    frame: &CanonicalAffineFrame,
    half_thickness: &BigRational,
    face_index: usize,
    limits: &DirectFAffineHingeCorridorLimits,
    work: &mut DirectFAffineHingeCorridorWork,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<[ClosedAffineHalfspace; 5]>, CayleyError> {
    let inward_cross_thickness = exact_cross(&frame.inward, &frame.thickness, meter)?;
    let determinant = exact_dot(&frame.axis, &inward_cross_thickness, meter)?;
    if determinant.is_zero() {
        return Ok(None);
    }
    let thickness_cross_axis = exact_cross(&frame.thickness, &frame.axis, meter)?;
    let axis_cross_inward = exact_cross(&frame.axis, &frame.inward, meter)?;
    let lambda = exact_divide_vector(&inward_cross_thickness, &determinant, meter)?;
    let alpha = exact_divide_vector(&thickness_cross_axis, &determinant, meter)?;
    let beta = exact_divide_vector(&axis_cross_inward, &determinant, meter)?;
    charge_counter(
        &mut work.dual_basis_inversions,
        limits.max_dual_basis_inversions,
        "affine_corridor_dual_basis_inversions",
    )?;

    let zero = BigRational::zero();
    let one = BigRational::one();
    for (dual, expected) in [
        (&lambda, [&one, &zero, &zero]),
        (&alpha, [&zero, &one, &zero]),
        (&beta, [&zero, &zero, &one]),
    ] {
        for (basis, expected) in [
            (&frame.axis, expected[0]),
            (&frame.inward, expected[1]),
            (&frame.thickness, expected[2]),
        ] {
            let observed = exact_dot(dual, basis, meter)?;
            if meter.compare_rational(&observed, expected, STAGE)? != Ordering::Equal {
                return Ok(None);
            }
        }
    }
    if meter.compare_rational(half_thickness, &zero, STAGE)? != Ordering::Greater {
        return Ok(None);
    }

    let lambda_start = exact_dot_point(&lambda, &frame.start, meter)?;
    let alpha_start = exact_dot_point(&alpha, &frame.start, meter)?;
    let beta_start = exact_dot_point(&beta, &frame.start, meter)?;
    let neg_lambda = exact_negate_vector(&lambda, meter)?;
    let neg_alpha = exact_negate_vector(&alpha, meter)?;
    let neg_beta = exact_negate_vector(&beta, meter)?;
    let lower_lambda_offset = meter.negate_rational(&lambda_start, STAGE)?;
    let upper_lambda_offset = meter.add_rational(&lambda_start, &one, STAGE)?;
    let alpha_offset = meter.negate_rational(&alpha_start, STAGE)?;
    let lower_beta_offset = meter.subtract_rational(half_thickness, &beta_start, STAGE)?;
    let upper_beta_offset = meter.add_rational(half_thickness, &beta_start, STAGE)?;
    let halfspaces = [
        ClosedAffineHalfspace {
            normal: neg_lambda,
            offset: lower_lambda_offset,
            face_index,
            constraint_index: 0,
        },
        ClosedAffineHalfspace {
            normal: clone_vector(&lambda, meter)?,
            offset: upper_lambda_offset,
            face_index,
            constraint_index: 1,
        },
        ClosedAffineHalfspace {
            normal: neg_alpha,
            offset: alpha_offset,
            face_index,
            constraint_index: 2,
        },
        ClosedAffineHalfspace {
            normal: neg_beta,
            offset: lower_beta_offset,
            face_index,
            constraint_index: 3,
        },
        ClosedAffineHalfspace {
            normal: beta,
            offset: upper_beta_offset,
            face_index,
            constraint_index: 4,
        },
    ];
    charge_counter(
        &mut work.affine_half_prisms,
        limits.max_affine_half_prisms,
        "affine_corridor_half_prisms",
    )?;
    for _ in &halfspaces {
        charge_counter(
            &mut work.halfspaces,
            limits.max_halfspaces,
            "affine_corridor_halfspaces",
        )?;
    }
    Ok(Some(halfspaces))
}

fn scan_complete_prism_vertices_against_halfspaces(
    report: &ExactPrismIntersectionReport,
    halfspaces: &[ClosedAffineHalfspace; HALFSPACE_COUNT],
    limits: &DirectFAffineHingeCorridorLimits,
    work: &mut DirectFAffineHingeCorridorWork,
    meter: &mut WorkMeter<'_>,
) -> Result<CompleteOutsideScan, CayleyError> {
    let mut outside = CompleteOutsideScan::default();
    for (vertex_index, vertex) in report.canonical_vertices().iter().enumerate() {
        let mut vertex_outside = false;
        for halfspace in halfspaces {
            charge_counter(
                &mut work.prism_vertex_halfspace_tests,
                limits.max_prism_vertex_halfspace_tests,
                "affine_corridor_prism_vertex_halfspace_tests",
            )?;
            let value = exact_dot_point(&halfspace.normal, vertex, meter)?;
            if meter.compare_rational(&value, &halfspace.offset, STAGE)? == Ordering::Greater {
                vertex_outside = true;
                outside.violated_halfspace_count = outside
                    .violated_halfspace_count
                    .checked_add(1)
                    .ok_or(CayleyError::ResourceLimitExceeded {
                        stage: STAGE,
                        resource: "affine_corridor_violated_halfspaces",
                    })?;
            }
        }
        if vertex_outside {
            outside
                .first_outside_vertex_index
                .get_or_insert(vertex_index);
            outside.outside_vertex_count = outside.outside_vertex_count.checked_add(1).ok_or(
                CayleyError::ResourceLimitExceeded {
                    stage: STAGE,
                    resource: "affine_corridor_outside_vertices",
                },
            )?;
        }
    }
    let expected = report
        .canonical_vertices()
        .len()
        .checked_mul(HALFSPACE_COUNT)
        .ok_or(CayleyError::ResourceLimitExceeded {
            stage: STAGE,
            resource: "affine_corridor_prism_vertex_halfspace_tests",
        })?;
    if work.prism_vertex_halfspace_tests != expected {
        return Err(CayleyError::InvariantFailure { stage: STAGE });
    }
    Ok(outside)
}

fn enumerate_complete_corridor_vertices(
    halfspaces: &[ClosedAffineHalfspace; HALFSPACE_COUNT],
    limits: &DirectFAffineHingeCorridorLimits,
    work: &mut DirectFAffineHingeCorridorWork,
    meter: &mut WorkMeter<'_>,
) -> Result<Vec<ExactPoint3>, CayleyError> {
    let triples_before = work.plane_triples;
    let singular_before = work.singular_plane_triples;
    let nonsingular_before = work.nonsingular_solves;
    let membership_before = work.membership_tests;
    let mut vertices = Vec::with_capacity(MAX_CORRIDOR_VERTICES);

    for first in 0..HALFSPACE_COUNT - 2 {
        for second in first + 1..HALFSPACE_COUNT - 1 {
            for third in second + 1..HALFSPACE_COUNT {
                charge_counter(
                    &mut work.plane_triples,
                    limits.max_plane_triples,
                    "affine_corridor_plane_triples",
                )?;
                let planes = [&halfspaces[first], &halfspaces[second], &halfspaces[third]];
                let cross = exact_cross(&planes[1].normal, &planes[2].normal, meter)?;
                let determinant = exact_dot(&planes[0].normal, &cross, meter)?;
                if determinant.is_zero() {
                    charge_counter(
                        &mut work.singular_plane_triples,
                        limits.max_singular_plane_triples,
                        "affine_corridor_singular_plane_triples",
                    )?;
                    continue;
                }
                charge_counter(
                    &mut work.nonsingular_solves,
                    limits.max_nonsingular_solves,
                    "affine_corridor_nonsingular_solves",
                )?;
                let candidate = solve_affine_plane_triple(&planes, &determinant, meter)?;
                let mut inside_all = true;
                for halfspace in halfspaces {
                    charge_counter(
                        &mut work.membership_tests,
                        limits.max_membership_tests,
                        "affine_corridor_membership_tests",
                    )?;
                    let value = exact_dot_point(&halfspace.normal, &candidate, meter)?;
                    inside_all &= meter.compare_rational(&value, &halfspace.offset, STAGE)?
                        != Ordering::Greater;
                }
                if !inside_all {
                    continue;
                }

                let mut duplicate = false;
                for retained in &vertices {
                    charge_counter(
                        &mut work.dedup_comparisons,
                        limits.max_dedup_comparisons,
                        "affine_corridor_dedup_comparisons",
                    )?;
                    if canonical_point_eq(retained, &candidate) {
                        duplicate = true;
                        break;
                    }
                }
                if !duplicate {
                    charge_counter(
                        &mut work.corridor_vertices,
                        limits.max_corridor_vertices,
                        "affine_corridor_vertices",
                    )?;
                    vertices.push(candidate);
                }
            }
        }
    }

    let triple_delta = checked_counter_delta(
        work.plane_triples,
        triples_before,
        "affine_corridor_plane_triples",
    )?;
    let singular_delta = checked_counter_delta(
        work.singular_plane_triples,
        singular_before,
        "affine_corridor_singular_plane_triples",
    )?;
    let nonsingular_delta = checked_counter_delta(
        work.nonsingular_solves,
        nonsingular_before,
        "affine_corridor_nonsingular_solves",
    )?;
    let membership_delta = checked_counter_delta(
        work.membership_tests,
        membership_before,
        "affine_corridor_membership_tests",
    )?;
    let accounted = singular_delta.checked_add(nonsingular_delta).ok_or(
        CayleyError::ResourceLimitExceeded {
            stage: STAGE,
            resource: "affine_corridor_plane_triples",
        },
    )?;
    let expected_memberships = nonsingular_delta.checked_mul(HALFSPACE_COUNT).ok_or(
        CayleyError::ResourceLimitExceeded {
            stage: STAGE,
            resource: "affine_corridor_membership_tests",
        },
    )?;
    if triple_delta != PLANE_TRIPLE_COUNT
        || accounted != PLANE_TRIPLE_COUNT
        || membership_delta != expected_memberships
        || work.corridor_vertices != vertices.len()
    {
        return Err(CayleyError::InvariantFailure { stage: STAGE });
    }
    Ok(vertices)
}

fn solve_affine_plane_triple(
    planes: &[&ClosedAffineHalfspace; 3],
    determinant: &BigRational,
    meter: &mut WorkMeter<'_>,
) -> Result<ExactPoint3, CayleyError> {
    if determinant.is_zero() {
        return Err(CayleyError::InvariantFailure { stage: STAGE });
    }
    let cross12 = exact_cross(&planes[1].normal, &planes[2].normal, meter)?;
    let cross20 = exact_cross(&planes[2].normal, &planes[0].normal, meter)?;
    let cross01 = exact_cross(&planes[0].normal, &planes[1].normal, meter)?;
    let term0 = exact_scale_vector(&cross12, &planes[0].offset, meter)?;
    let term1 = exact_scale_vector(&cross20, &planes[1].offset, meter)?;
    let term2 = exact_scale_vector(&cross01, &planes[2].offset, meter)?;
    let first_sum = exact_add_vectors(&term0, &term1, meter)?;
    let numerator = exact_add_vectors(&first_sum, &term2, meter)?;
    Ok(ExactPoint3 {
        coordinates: try_array3(|axis| {
            meter.divide_rational(&numerator.coordinates[axis], determinant, STAGE)
        })?,
    })
}

fn authenticate_normal_rank(
    halfspaces: &[ClosedAffineHalfspace; HALFSPACE_COUNT],
    limits: &DirectFAffineHingeCorridorLimits,
    work: &mut DirectFAffineHingeCorridorWork,
    meter: &mut WorkMeter<'_>,
) -> Result<u8, CayleyError> {
    let tests_before = work.normal_rank_tests;
    let mut rank = 0_u8;
    let mut first_direction = None;
    let mut spanning_normal = None;
    for halfspace in halfspaces {
        charge_counter(
            &mut work.normal_rank_tests,
            limits.max_normal_rank_tests,
            "affine_corridor_normal_rank_tests",
        )?;
        match rank {
            0 => {
                if !exact_vector_is_zero(&halfspace.normal) {
                    first_direction = Some(clone_vector(&halfspace.normal, meter)?);
                    rank = 1;
                }
            }
            1 => {
                let first_direction = first_direction
                    .as_ref()
                    .ok_or(CayleyError::InvariantFailure { stage: STAGE })?;
                let cross = exact_cross(first_direction, &halfspace.normal, meter)?;
                if !exact_vector_is_zero(&cross) {
                    spanning_normal = Some(cross);
                    rank = 2;
                }
            }
            2 => {
                let spanning_normal = spanning_normal
                    .as_ref()
                    .ok_or(CayleyError::InvariantFailure { stage: STAGE })?;
                let determinant = exact_dot(spanning_normal, &halfspace.normal, meter)?;
                if !determinant.is_zero() {
                    rank = 3;
                }
            }
            3 => {}
            _ => return Err(CayleyError::InvariantFailure { stage: STAGE }),
        }
    }
    if checked_counter_delta(
        work.normal_rank_tests,
        tests_before,
        "affine_corridor_normal_rank_tests",
    )? != NORMAL_RANK_TESTS
    {
        return Err(CayleyError::InvariantFailure { stage: STAGE });
    }
    Ok(rank)
}

fn authenticate_bounded_recession_cone(
    halfspaces: &[ClosedAffineHalfspace; HALFSPACE_COUNT],
    limits: &DirectFAffineHingeCorridorLimits,
    work: &mut DirectFAffineHingeCorridorWork,
    meter: &mut WorkMeter<'_>,
) -> Result<bool, CayleyError> {
    let pairs_before = work.recession_normal_pairs;
    let signed_before = work.signed_recession_tests;
    let memberships_before = work.recession_membership_tests;
    let mut has_nonzero_recession_direction = false;

    for first in 0..HALFSPACE_COUNT - 1 {
        for second in first + 1..HALFSPACE_COUNT {
            charge_counter(
                &mut work.recession_normal_pairs,
                limits.max_recession_normal_pairs,
                "affine_corridor_recession_normal_pairs",
            )?;
            let positive =
                exact_cross(&halfspaces[first].normal, &halfspaces[second].normal, meter)?;
            for negate in [false, true] {
                charge_counter(
                    &mut work.signed_recession_tests,
                    limits.max_signed_recession_tests,
                    "affine_corridor_signed_recession_tests",
                )?;
                let direction = if negate {
                    exact_negate_vector(&positive, meter)?
                } else {
                    clone_vector(&positive, meter)?
                };
                let mut satisfies_all = true;
                for halfspace in halfspaces {
                    charge_counter(
                        &mut work.recession_membership_tests,
                        limits.max_recession_membership_tests,
                        "affine_corridor_recession_membership_tests",
                    )?;
                    let value = exact_dot(&halfspace.normal, &direction, meter)?;
                    satisfies_all &= meter.compare_rational(&value, &BigRational::zero(), STAGE)?
                        != Ordering::Greater;
                }
                has_nonzero_recession_direction |=
                    !exact_vector_is_zero(&direction) && satisfies_all;
            }
        }
    }

    if checked_counter_delta(
        work.recession_normal_pairs,
        pairs_before,
        "affine_corridor_recession_normal_pairs",
    )? != RECESSION_NORMAL_PAIRS
        || checked_counter_delta(
            work.signed_recession_tests,
            signed_before,
            "affine_corridor_signed_recession_tests",
        )? != SIGNED_RECESSION_TESTS
        || checked_counter_delta(
            work.recession_membership_tests,
            memberships_before,
            "affine_corridor_recession_membership_tests",
        )? != RECESSION_MEMBERSHIP_TESTS
    {
        return Err(CayleyError::InvariantFailure { stage: STAGE });
    }
    Ok(has_nonzero_recession_direction)
}

fn authenticate_point_affine_rank(
    vertices: &[ExactPoint3],
    limits: &DirectFAffineHingeCorridorLimits,
    work: &mut DirectFAffineHingeCorridorWork,
    meter: &mut WorkMeter<'_>,
) -> Result<u8, CayleyError> {
    let Some(base) = vertices.first() else {
        return Err(CayleyError::InvariantFailure { stage: STAGE });
    };
    let tests_before = work.corridor_affine_rank_tests;
    let mut rank = 0_u8;
    let mut first_direction = None;
    let mut spanning_normal = None;
    for candidate in vertices {
        charge_counter(
            &mut work.corridor_affine_rank_tests,
            limits.max_corridor_affine_rank_tests,
            "affine_corridor_affine_rank_tests",
        )?;
        match rank {
            0 => {
                let direction = exact_between(base, candidate, meter)?;
                if !exact_vector_is_zero(&direction) {
                    first_direction = Some(direction);
                    rank = 1;
                }
            }
            1 => {
                let direction = exact_between(base, candidate, meter)?;
                let first_direction = first_direction
                    .as_ref()
                    .ok_or(CayleyError::InvariantFailure { stage: STAGE })?;
                let cross = exact_cross(first_direction, &direction, meter)?;
                if !exact_vector_is_zero(&cross) {
                    spanning_normal = Some(cross);
                    rank = 2;
                }
            }
            2 => {
                let direction = exact_between(base, candidate, meter)?;
                let spanning_normal = spanning_normal
                    .as_ref()
                    .ok_or(CayleyError::InvariantFailure { stage: STAGE })?;
                let determinant = exact_dot(spanning_normal, &direction, meter)?;
                if !determinant.is_zero() {
                    rank = 3;
                }
            }
            3 => {}
            _ => return Err(CayleyError::InvariantFailure { stage: STAGE }),
        }
    }
    if checked_counter_delta(
        work.corridor_affine_rank_tests,
        tests_before,
        "affine_corridor_affine_rank_tests",
    )? != vertices.len()
    {
        return Err(CayleyError::InvariantFailure { stage: STAGE });
    }
    Ok(rank)
}

fn build_parent_gram_geometry(
    parent: &LiteralAffineFace,
    exact: &RationalCayleyTreePose<'_>,
    hinge_index: usize,
    limits: &DirectFAffineHingeCorridorLimits,
    work: &mut DirectFAffineHingeCorridorWork,
    meter: &mut WorkMeter<'_>,
) -> Result<ParentGramGeometry, CayleyError> {
    charge_counter(
        &mut work.gram_inversions,
        limits.max_gram_inversions,
        "affine_corridor_gram_inversions",
    )?;
    let columns = try_array3(|column| {
        Ok(ExactVector3 {
            coordinates: try_array3(|row| {
                meter.clone_rational(&parent.transform.rotation[row][column], STAGE)
            })?,
        })
    })?;
    let cross12 = exact_cross(&columns[1], &columns[2], meter)?;
    let determinant = exact_dot(&columns[0], &cross12, meter)?;
    if determinant.is_zero() {
        return Err(CayleyError::InvariantFailure { stage: STAGE });
    }
    let cross20 = exact_cross(&columns[2], &columns[0], meter)?;
    let cross01 = exact_cross(&columns[0], &columns[1], meter)?;
    let inverse_rows = [
        exact_divide_vector(&cross12, &determinant, meter)?,
        exact_divide_vector(&cross20, &determinant, meter)?,
        exact_divide_vector(&cross01, &determinant, meter)?,
    ];
    let gram = try_array3(|row| {
        try_array3(|column| {
            let products = try_array3(|inverse_row| {
                meter.multiply_rational(
                    &inverse_rows[inverse_row].coordinates[row],
                    &inverse_rows[inverse_row].coordinates[column],
                    STAGE,
                )
            })?;
            let first_two = meter.add_rational(&products[0], &products[1], STAGE)?;
            meter.add_rational(&first_two, &products[2], STAGE)
        })
    })?;
    for (row, row_values) in gram.iter().enumerate() {
        for (column, value) in row_values.iter().enumerate() {
            if value != &gram[column][row] {
                return Err(CayleyError::InvariantFailure { stage: STAGE });
            }
        }
    }

    let first_minor = meter.clone_rational(&gram[0][0], STAGE)?;
    let diagonal_product = meter.multiply_rational(&gram[0][0], &gram[1][1], STAGE)?;
    let off_diagonal_product = meter.multiply_rational(&gram[0][1], &gram[1][0], STAGE)?;
    let second_minor = meter.subtract_rational(&diagonal_product, &off_diagonal_product, STAGE)?;
    let determinant_gram = exact_matrix_determinant(&gram, meter)?;
    let mut positive_definite = true;
    for principal_minor in [&first_minor, &second_minor, &determinant_gram] {
        charge_counter(
            &mut work.gram_positive_definite_tests,
            limits.max_gram_positive_definite_tests,
            "affine_corridor_gram_positive_definite_tests",
        )?;
        positive_definite &=
            meter.compare_rational(principal_minor, &BigRational::zero(), STAGE)?
                == Ordering::Greater;
    }
    if !positive_definite {
        return Err(CayleyError::InvariantFailure { stage: STAGE });
    }

    let exact_hinge = exact
        .hinges
        .get(hinge_index)
        .ok_or(CayleyError::InvariantFailure { stage: STAGE })?;
    if parent.face != exact_hinge.parent {
        return Err(CayleyError::InvariantFailure { stage: STAGE });
    }
    let start_index = parent
        .source_vertex_ids
        .iter()
        .position(|vertex| *vertex == exact_hinge.endpoint_vertices[0])
        .ok_or(CayleyError::InvariantFailure { stage: STAGE })?;
    let end_index = parent
        .source_vertex_ids
        .iter()
        .position(|vertex| *vertex == exact_hinge.endpoint_vertices[1])
        .ok_or(CayleyError::InvariantFailure { stage: STAGE })?;
    if start_index == end_index {
        return Err(CayleyError::InvariantFailure { stage: STAGE });
    }
    let axis_start = clone_point(&parent.world_vertices[start_index], meter)?;
    let axis = exact_between(
        &parent.world_vertices[start_index],
        &parent.world_vertices[end_index],
        meter,
    )?;
    let gram_axis = exact_matrix_vector(&gram, &axis, meter)?;
    let length_squared = exact_dot(&axis, &gram_axis, meter)?;
    if meter.compare_rational(&length_squared, &BigRational::zero(), STAGE)? != Ordering::Greater {
        return Err(CayleyError::InvariantFailure { stage: STAGE });
    }

    Ok(ParentGramGeometry {
        axis_start,
        axis,
        gram,
        length_squared,
    })
}

fn scan_corridor_gram_radius(
    vertices: &[ExactPoint3],
    parent: &ParentGramGeometry,
    limits: &DirectFAffineHingeCorridorLimits,
    work: &mut DirectFAffineHingeCorridorWork,
    meter: &mut WorkMeter<'_>,
) -> Result<(BigRational, bool), CayleyError> {
    let tests_before = work.gram_quadratic_tests;
    let gram_axis = exact_matrix_vector(&parent.gram, &parent.axis, meter)?;
    let mut radius_squared = BigRational::zero();
    for vertex in vertices {
        charge_counter(
            &mut work.gram_quadratic_tests,
            limits.max_gram_quadratic_tests,
            "affine_corridor_gram_quadratic_tests",
        )?;
        let radius = exact_between(&parent.axis_start, vertex, meter)?;
        let gram_radius = exact_matrix_vector(&parent.gram, &radius, meter)?;
        let radius_norm_squared = exact_dot(&radius, &gram_radius, meter)?;
        let axial_numerator = exact_dot(&radius, &gram_axis, meter)?;
        let axial_numerator_squared =
            meter.multiply_rational(&axial_numerator, &axial_numerator, STAGE)?;
        let axial_component =
            meter.divide_rational(&axial_numerator_squared, &parent.length_squared, STAGE)?;
        let quadratic = meter.subtract_rational(&radius_norm_squared, &axial_component, STAGE)?;
        if meter.compare_rational(&quadratic, &BigRational::zero(), STAGE)? == Ordering::Less {
            return Err(CayleyError::InvariantFailure { stage: STAGE });
        }
        if meter.compare_rational(&quadratic, &radius_squared, STAGE)? == Ordering::Greater {
            radius_squared = quadratic;
        }
    }
    if checked_counter_delta(
        work.gram_quadratic_tests,
        tests_before,
        "affine_corridor_gram_quadratic_tests",
    )? != vertices.len()
    {
        return Err(CayleyError::InvariantFailure { stage: STAGE });
    }
    let exceeds = meter.compare_rational(&radius_squared, &parent.length_squared, STAGE)?
        == Ordering::Greater;
    Ok((radius_squared, exceeds))
}

fn scan_complete_prism_axial_bounds(
    report: &ExactPrismIntersectionReport,
    parent: &ParentGramGeometry,
    limits: &DirectFAffineHingeCorridorLimits,
    work: &mut DirectFAffineHingeCorridorWork,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<(usize, usize)>, CayleyError> {
    let tests_before = work.axial_tests;
    let gram_axis = exact_matrix_vector(&parent.gram, &parent.axis, meter)?;
    let mut first_outside = None;
    let mut outside_count = 0_usize;
    for (vertex_index, vertex) in report.canonical_vertices().iter().enumerate() {
        charge_counter(
            &mut work.axial_tests,
            limits.max_axial_tests,
            "affine_corridor_axial_tests",
        )?;
        let radius = exact_between(&parent.axis_start, vertex, meter)?;
        let axial = exact_dot(&radius, &gram_axis, meter)?;
        let before_start =
            meter.compare_rational(&axial, &BigRational::zero(), STAGE)? == Ordering::Less;
        let after_end =
            meter.compare_rational(&axial, &parent.length_squared, STAGE)? == Ordering::Greater;
        if before_start || after_end {
            first_outside.get_or_insert(vertex_index);
            outside_count =
                outside_count
                    .checked_add(1)
                    .ok_or(CayleyError::ResourceLimitExceeded {
                        stage: STAGE,
                        resource: "affine_corridor_axial_outside_vertices",
                    })?;
        }
    }
    if checked_counter_delta(
        work.axial_tests,
        tests_before,
        "affine_corridor_axial_tests",
    )? != report.canonical_vertices().len()
    {
        return Err(CayleyError::InvariantFailure { stage: STAGE });
    }
    Ok(first_outside.map(|index| (index, outside_count)))
}

fn exact_offset_point(
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

fn exact_scale_vector(
    vector: &ExactVector3,
    scalar: &BigRational,
    meter: &mut WorkMeter<'_>,
) -> Result<ExactVector3, CayleyError> {
    Ok(ExactVector3 {
        coordinates: try_array3(|axis| {
            meter.multiply_rational(&vector.coordinates[axis], scalar, STAGE)
        })?,
    })
}

fn exact_add_vectors(
    first: &ExactVector3,
    second: &ExactVector3,
    meter: &mut WorkMeter<'_>,
) -> Result<ExactVector3, CayleyError> {
    Ok(ExactVector3 {
        coordinates: try_array3(|axis| {
            meter.add_rational(&first.coordinates[axis], &second.coordinates[axis], STAGE)
        })?,
    })
}

fn exact_subtract_vectors(
    first: &ExactVector3,
    second: &ExactVector3,
    meter: &mut WorkMeter<'_>,
) -> Result<ExactVector3, CayleyError> {
    Ok(ExactVector3 {
        coordinates: try_array3(|axis| {
            meter.subtract_rational(&first.coordinates[axis], &second.coordinates[axis], STAGE)
        })?,
    })
}

fn exact_divide_vector(
    vector: &ExactVector3,
    scalar: &BigRational,
    meter: &mut WorkMeter<'_>,
) -> Result<ExactVector3, CayleyError> {
    if scalar.is_zero() {
        return Err(CayleyError::InvariantFailure { stage: STAGE });
    }
    Ok(ExactVector3 {
        coordinates: try_array3(|axis| {
            meter.divide_rational(&vector.coordinates[axis], scalar, STAGE)
        })?,
    })
}

fn exact_negate_vector(
    vector: &ExactVector3,
    meter: &mut WorkMeter<'_>,
) -> Result<ExactVector3, CayleyError> {
    Ok(ExactVector3 {
        coordinates: try_array3(|axis| meter.negate_rational(&vector.coordinates[axis], STAGE))?,
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

fn exact_dot_point(
    vector: &ExactVector3,
    point: &ExactPoint3,
    meter: &mut WorkMeter<'_>,
) -> Result<BigRational, CayleyError> {
    let products = try_array3(|axis| {
        meter.multiply_rational(&vector.coordinates[axis], &point.coordinates[axis], STAGE)
    })?;
    let first_two = meter.add_rational(&products[0], &products[1], STAGE)?;
    meter.add_rational(&first_two, &products[2], STAGE)
}

fn exact_matrix_vector(
    matrix: &[[BigRational; 3]; 3],
    vector: &ExactVector3,
    meter: &mut WorkMeter<'_>,
) -> Result<ExactVector3, CayleyError> {
    Ok(ExactVector3 {
        coordinates: try_array3(|row| {
            let products = try_array3(|column| {
                meter.multiply_rational(&matrix[row][column], &vector.coordinates[column], STAGE)
            })?;
            let first_two = meter.add_rational(&products[0], &products[1], STAGE)?;
            meter.add_rational(&first_two, &products[2], STAGE)
        })?,
    })
}

fn exact_matrix_determinant(
    matrix: &[[BigRational; 3]; 3],
    meter: &mut WorkMeter<'_>,
) -> Result<BigRational, CayleyError> {
    let rows = try_array3(|row| {
        Ok(ExactVector3 {
            coordinates: try_array3(|column| meter.clone_rational(&matrix[row][column], STAGE))?,
        })
    })?;
    let cross = exact_cross(&rows[1], &rows[2], meter)?;
    exact_dot(&rows[0], &cross, meter)
}

fn apply_exact_linear(
    matrix: &[[BigRational; 3]; 3],
    vector: &ExactVector3,
    meter: &mut WorkMeter<'_>,
) -> Result<ExactVector3, CayleyError> {
    exact_matrix_vector(matrix, vector, meter)
}

fn clone_vector(
    vector: &ExactVector3,
    meter: &mut WorkMeter<'_>,
) -> Result<ExactVector3, CayleyError> {
    Ok(ExactVector3 {
        coordinates: try_array3(|axis| meter.clone_rational(&vector.coordinates[axis], STAGE))?,
    })
}

fn clone_point(point: &ExactPoint3, meter: &mut WorkMeter<'_>) -> Result<ExactPoint3, CayleyError> {
    Ok(ExactPoint3 {
        coordinates: try_array3(|axis| meter.clone_rational(&point.coordinates[axis], STAGE))?,
    })
}

fn exact_vector_is_zero(vector: &ExactVector3) -> bool {
    vector.coordinates.iter().all(BigRational::is_zero)
}

fn checked_counter_delta(
    after: usize,
    before: usize,
    resource: &'static str,
) -> Result<usize, CayleyError> {
    after
        .checked_sub(before)
        .ok_or(CayleyError::ResourceLimitExceeded {
            stage: STAGE,
            resource,
        })
}

fn set_fixed_counter(
    counter: &mut usize,
    required: usize,
    maximum: usize,
    resource: &'static str,
) -> Result<(), CayleyError> {
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
    *counter = counter
        .checked_add(1)
        .ok_or(CayleyError::ResourceLimitExceeded {
            stage: STAGE,
            resource,
        })?;
    if *counter > maximum {
        return Err(CayleyError::ResourceLimitExceeded {
            stage: STAGE,
            resource,
        });
    }
    Ok(())
}

/// Revalidates only the provenance of a private C2 geometry diagnostic.
///
/// A successful return is explicitly not shared-hinge admission, collision
/// allowance, a safe-set witness, or production authority.
#[allow(clippy::too_many_arguments)]
pub(super) fn revalidate_direct_f_affine_hinge_corridor_diagnostic_v1<
    'diagnostic,
    'prerequisite,
    'ef,
    'exact_e_corridor,
    'exact,
    'pose,
>(
    diagnostic: &'diagnostic DirectFAffineHingeCorridorDiagnosticV1<
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
    &'diagnostic DirectFAffineHingeCorridorDiagnosticV1<
        'prerequisite,
        'ef,
        'exact_e_corridor,
        'exact,
        'pose,
    >,
> {
    let authority = &diagnostic.authority;
    let sealed_phase2b = exact_e_corridor.sealed_work()?;
    let sealed_work = diagnostic.sealed_work.as_ref()?;
    if !positive_finite_binary64(paper_thickness_mm)
        || authority.phase2b_work != *sealed_phase2b
        || sealed_work.phase2b_exact != sealed_phase2b.exact
        || sealed_work.shared_endpoint_equality_tests != SHARED_ENDPOINT_EQUALITY_TESTS
        || diagnostic.geometry.shared_endpoint_mismatch_count > SHARED_ENDPOINT_EQUALITY_TESTS
        || !std::ptr::eq(authority.prerequisite, prerequisite)
        || !std::ptr::eq(authority.ef_boundary, ef_boundary)
        || !std::ptr::eq(authority.exact_e_corridor, exact_e_corridor)
        || !std::ptr::eq(authority.exact, exact)
        || authority.paper_thickness_bits != paper_thickness_mm.to_bits()
        || authority.bound.model() != bound.model()
        || !authority.bound.pose().same_instance(bound.pose())
        || !exact.is_for(bound)
        || exact.faces.len() != FACE_COUNT
        || exact.hinges.len() != HINGE_COUNT
        || authority.left_face_index != prerequisite.left_face_index
        || authority.right_face_index != prerequisite.right_face_index
        || authority.hinge_index != prerequisite.hinge_index
        || authority.interaction_kind != exact_e_corridor.interaction_kind()
        || authority.binary64_face_transforms != ef_boundary.binary64_face_transforms
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
        return None;
    }
    for (face_index, exact_face) in exact.faces.iter().enumerate() {
        if capture_face_transform_bits(bound, exact_face.face)?
            != authority.binary64_face_transforms[face_index]
        {
            return None;
        }
    }
    let exact_hinge = exact.hinges.get(authority.hinge_index)?;
    let current_hinge_parent = capture_transform_bits(
        exact_hinge.parent,
        bound.pose().hinge_parent_transform(exact_hinge.edge)?,
    );
    let parent_face_index = exact
        .faces
        .iter()
        .position(|face| face.face == exact_hinge.parent)?;
    if current_hinge_parent != authority.hinge_parent_transform
        || current_hinge_parent != authority.binary64_face_transforms[parent_face_index]
    {
        return None;
    }
    Some(diagnostic)
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

    fn halfspace(
        normal: ExactVector3,
        offset: i64,
        constraint_index: usize,
    ) -> ClosedAffineHalfspace {
        ClosedAffineHalfspace {
            normal,
            offset: integer(offset),
            face_index: constraint_index / 5,
            constraint_index: constraint_index % 5,
        }
    }

    fn bounded_box_halfspaces() -> [ClosedAffineHalfspace; HALFSPACE_COUNT] {
        [
            halfspace(vector(-1, 0, 0), 0, 0),
            halfspace(vector(1, 0, 0), 1, 1),
            halfspace(vector(0, -1, 0), 0, 2),
            halfspace(vector(0, 1, 0), 1, 3),
            halfspace(vector(0, 0, -1), 1, 4),
            halfspace(vector(0, 0, 1), 1, 5),
            halfspace(vector(-1, 0, 0), 0, 6),
            halfspace(vector(1, 0, 0), 1, 7),
            halfspace(vector(0, -1, 0), 0, 8),
            halfspace(vector(0, 1, 0), 1, 9),
        ]
    }

    fn unbounded_half_strip_halfspaces() -> [ClosedAffineHalfspace; HALFSPACE_COUNT] {
        [
            halfspace(vector(-1, 0, 0), 0, 0),
            halfspace(vector(1, 0, 0), 1, 1),
            halfspace(vector(0, 0, -1), 1, 2),
            halfspace(vector(0, 0, 1), 1, 3),
            halfspace(vector(0, -1, 0), 0, 4),
            halfspace(vector(0, -1, 0), 0, 5),
            halfspace(vector(-1, 0, 0), 0, 6),
            halfspace(vector(1, 0, 0), 1, 7),
            halfspace(vector(0, 0, -1), 1, 8),
            halfspace(vector(0, 0, 1), 1, 9),
        ]
    }

    fn contains(halfspaces: &[ClosedAffineHalfspace], point: &ExactPoint3) -> bool {
        halfspaces.iter().all(|halfspace| {
            let value = &halfspace.normal.coordinates[0] * &point.coordinates[0]
                + &halfspace.normal.coordinates[1] * &point.coordinates[1]
                + &halfspace.normal.coordinates[2] * &point.coordinates[2];
            value <= halfspace.offset
        })
    }

    fn identity_gram_parent() -> ParentGramGeometry {
        ParentGramGeometry {
            axis_start: point(0, 0, 0),
            axis: vector(1, 0, 0),
            gram: [
                [integer(1), integer(0), integer(0)],
                [integer(0), integer(1), integer(0)],
                [integer(0), integer(0), integer(1)],
            ],
            length_squared: integer(1),
        }
    }

    #[test]
    fn complete_120_triple_and_90_signed_recession_scans_prove_box_bounded() {
        let limits = DirectFAffineHingeCorridorLimits::default();
        let mut work = DirectFAffineHingeCorridorWork::default();
        let mut meter = WorkMeter::new(&limits.local_exact);
        let halfspaces = bounded_box_halfspaces();
        let vertices =
            enumerate_complete_corridor_vertices(&halfspaces, &limits, &mut work, &mut meter)
                .unwrap();
        assert_eq!(vertices.len(), 8);
        assert_eq!(work.plane_triples, PLANE_TRIPLE_COUNT);
        assert_eq!(
            work.singular_plane_triples + work.nonsingular_solves,
            PLANE_TRIPLE_COUNT
        );
        assert_eq!(
            work.membership_tests,
            work.nonsingular_solves * HALFSPACE_COUNT
        );
        assert_eq!(
            authenticate_normal_rank(&halfspaces, &limits, &mut work, &mut meter).unwrap(),
            3
        );
        assert!(
            !authenticate_bounded_recession_cone(&halfspaces, &limits, &mut work, &mut meter,)
                .unwrap()
        );
        assert_eq!(work.recession_normal_pairs, RECESSION_NORMAL_PAIRS);
        assert_eq!(work.signed_recession_tests, SIGNED_RECESSION_TESTS);
        assert_eq!(work.recession_membership_tests, RECESSION_MEMBERSHIP_TESTS);
    }

    #[test]
    fn complete_recession_scan_detects_unbounded_half_strip() {
        let limits = DirectFAffineHingeCorridorLimits::default();
        let mut work = DirectFAffineHingeCorridorWork::default();
        let mut meter = WorkMeter::new(&limits.local_exact);
        let halfspaces = unbounded_half_strip_halfspaces();
        assert_eq!(
            authenticate_normal_rank(&halfspaces, &limits, &mut work, &mut meter).unwrap(),
            3
        );
        assert!(
            authenticate_bounded_recession_cone(&halfspaces, &limits, &mut work, &mut meter,)
                .unwrap()
        );
        assert_eq!(work.recession_normal_pairs, RECESSION_NORMAL_PAIRS);
        assert_eq!(work.signed_recession_tests, SIGNED_RECESSION_TESTS);
        assert_eq!(work.recession_membership_tests, RECESSION_MEMBERSHIP_TESTS);
    }

    #[test]
    fn canonical_half_prism_is_scale_and_endpoint_order_invariant_but_not_shear_invariant() {
        let limits = DirectFAffineHingeCorridorLimits::default();
        let frames = [
            CanonicalAffineFrame {
                start: point(0, 0, 0),
                axis: vector(2, 0, 0),
                inward: vector(0, 3, 0),
                thickness: vector(0, 0, 1),
            },
            CanonicalAffineFrame {
                start: point(0, 0, 0),
                axis: vector(2, 0, 0),
                inward: vector(0, 6, 0),
                thickness: vector(0, 0, 1),
            },
            CanonicalAffineFrame {
                start: point(2, 0, 0),
                axis: vector(-2, 0, 0),
                inward: vector(0, 3, 0),
                thickness: vector(0, 0, 1),
            },
            CanonicalAffineFrame {
                start: point(0, 0, 0),
                axis: vector(2, 0, 0),
                inward: vector(1, 3, 0),
                thickness: vector(0, 0, 1),
            },
        ];
        let mut generated = Vec::new();
        for (face_index, frame) in frames.iter().enumerate() {
            let mut work = DirectFAffineHingeCorridorWork::default();
            let mut meter = WorkMeter::new(&limits.local_exact);
            generated.push(
                build_frame_halfspaces(
                    frame,
                    &integer(1),
                    face_index,
                    &limits,
                    &mut work,
                    &mut meter,
                )
                .unwrap()
                .unwrap(),
            );
        }
        for x in -1..=3 {
            for y in -1..=4 {
                for z in -2..=2 {
                    let candidate = point(x, y, z);
                    let baseline = contains(&generated[0], &candidate);
                    assert_eq!(baseline, contains(&generated[1], &candidate));
                    assert_eq!(baseline, contains(&generated[2], &candidate));
                }
            }
        }
        let shear_witness = point(0, 3, 0);
        assert!(contains(&generated[0], &shear_witness));
        assert!(!contains(&generated[3], &shear_witness));
    }

    #[test]
    fn gram_radius_uses_closed_r_equals_l_and_rejects_r_greater_than_l() {
        let limits = DirectFAffineHingeCorridorLimits::default();
        let parent = identity_gram_parent();
        let mut equality_work = DirectFAffineHingeCorridorWork::default();
        let mut equality_meter = WorkMeter::new(&limits.local_exact);
        let (radius_squared, exceeds) = scan_corridor_gram_radius(
            &[point(0, 1, 0), point(1, -1, 0)],
            &parent,
            &limits,
            &mut equality_work,
            &mut equality_meter,
        )
        .unwrap();
        assert_eq!(radius_squared, integer(1));
        assert!(!exceeds, "R=L is a closed finite-corridor boundary");

        let mut outside_work = DirectFAffineHingeCorridorWork::default();
        let mut outside_meter = WorkMeter::new(&limits.local_exact);
        let (radius_squared, exceeds) = scan_corridor_gram_radius(
            &[point(0, 2, 0)],
            &parent,
            &limits,
            &mut outside_work,
            &mut outside_meter,
        )
        .unwrap();
        assert_eq!(radius_squared, integer(4));
        assert!(exceeds, "R>L requires LayerOffsetUnmodeled");
    }

    #[test]
    fn structural_counter_overflow_is_checked() {
        let mut counter = usize::MAX;
        assert!(matches!(
            charge_counter(&mut counter, usize::MAX, "overflow"),
            Err(CayleyError::ResourceLimitExceeded { .. })
        ));
        assert_eq!(counter, usize::MAX);
    }
}
