//! Exact, finite-work intersection kernel for two positive-thickness
//! triangular prisms in the canonical rational pose `E`.
//!
//! This private phase-2A module deliberately emits no hinge-corridor
//! capability, collision classification authority, persistence value, or UI
//! payload.  It accepts only exact mid-surface triangles, exact material
//! normals, and exact positive half-thicknesses.  Componentwise binary64
//! error boxes belong to the later E/F reconciliation gate and are not an
//! input to this kernel.

use std::cmp::Ordering;

use num_rational::BigRational;
use num_traits::{One, Signed, Zero};

use super::super::TotalTermLimits;
use super::{
    CayleyError, CayleyLimits, CayleyWork, ExactPoint3, ExactVector3, STAGE, WorkMeter,
    canonical_point_eq, exact_between, exact_dot, project_cayley_limits, rational_bits,
    rational_storage_bits, try_array3,
};

const PRISM_COUNT: usize = 2;
const SOLID_VERTEX_COUNT: usize = 12;
const FACET_COUNT: usize = 10;
const HALFSPACE_COUNT: usize = 10;
const PRISM_VOLUME_TEST_COUNT: usize = PRISM_COUNT;
const FACET_VERTEX_CHECK_COUNT: usize = 60;
const PLANE_TRIPLE_COUNT: usize = 120;
const MAX_SINGULAR_PLANE_TRIPLES: usize = PLANE_TRIPLE_COUNT;
const MAX_NONSINGULAR_SOLVES: usize = PLANE_TRIPLE_COUNT;
const MAX_MEMBERSHIP_TESTS: usize = PLANE_TRIPLE_COUNT * HALFSPACE_COUNT;
const MAX_CANDIDATE_VERTICES: usize = PLANE_TRIPLE_COUNT;
const MAX_DEDUP_COMPARISONS: usize = MAX_CANDIDATE_VERTICES * (MAX_CANDIDATE_VERTICES - 1) / 2;
const MAX_AFFINE_RANK_TESTS: usize = MAX_CANDIDATE_VERTICES;
const MAX_SUPPORT_PLANE_VERTEX_TESTS: usize = HALFSPACE_COUNT * MAX_CANDIDATE_VERTICES;
const MAX_SUPPORT_PAIR_TESTS: usize = 25;
const INPUT_RATIONALS: usize = PRISM_COUNT * (3 * 3 + 3 + 1);

/// Exact mid-surface description of one right triangular prism.
///
/// `material_normal` must be an exact unit vector perpendicular to the
/// triangle, and `half_thickness` must be strictly positive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ExactTriangularPrismInput {
    pub(super) mid_surface: [ExactPoint3; 3],
    pub(super) material_normal: ExactVector3,
    pub(super) half_thickness: BigRational,
}

/// Borrowed exact-E view used by the shared-meter entry.  A later corridor
/// issuer can point directly into an authenticated exact face pose without
/// performing unmetered `BigRational` clones.
#[derive(Debug, Clone, Copy)]
pub(super) struct ExactTriangularPrismView<'a> {
    pub(super) mid_surface: [&'a ExactPoint3; 3],
    pub(super) material_normal: &'a ExactVector3,
    pub(super) half_thickness: &'a BigRational,
}

impl ExactTriangularPrismInput {
    fn as_view(&self) -> ExactTriangularPrismView<'_> {
        ExactTriangularPrismView {
            mid_surface: [
                &self.mid_surface[0],
                &self.mid_surface[1],
                &self.mid_surface[2],
            ],
            material_normal: &self.material_normal,
            half_thickness: &self.half_thickness,
        }
    }
}

/// Hard, caller-non-expandable limits for one two-prism scan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ExactPrismLimits {
    pub(super) max_prisms: usize,
    pub(super) max_solid_vertices: usize,
    pub(super) max_facets: usize,
    pub(super) max_halfspaces: usize,
    pub(super) max_prism_volume_tests: usize,
    pub(super) max_facet_vertex_checks: usize,
    pub(super) max_plane_triples: usize,
    pub(super) max_singular_plane_triples: usize,
    pub(super) max_nonsingular_solves: usize,
    pub(super) max_membership_tests: usize,
    pub(super) max_candidate_vertices: usize,
    pub(super) max_dedup_comparisons: usize,
    pub(super) max_affine_rank_tests: usize,
    pub(super) max_support_plane_vertex_tests: usize,
    pub(super) max_support_pair_tests: usize,
    pub(super) max_input_rationals: usize,
    pub(super) max_input_rational_storage_bits: usize,
    pub(super) max_total_input_storage_bits: usize,
    pub(super) exact: CayleyLimits,
}

impl Default for ExactPrismLimits {
    fn default() -> Self {
        let exact = exact_prism_hard_cayley_limits();
        Self {
            max_prisms: PRISM_COUNT,
            max_solid_vertices: SOLID_VERTEX_COUNT,
            max_facets: FACET_COUNT,
            max_halfspaces: HALFSPACE_COUNT,
            max_prism_volume_tests: PRISM_VOLUME_TEST_COUNT,
            max_facet_vertex_checks: FACET_VERTEX_CHECK_COUNT,
            max_plane_triples: PLANE_TRIPLE_COUNT,
            max_singular_plane_triples: MAX_SINGULAR_PLANE_TRIPLES,
            max_nonsingular_solves: MAX_NONSINGULAR_SOLVES,
            max_membership_tests: MAX_MEMBERSHIP_TESTS,
            max_candidate_vertices: MAX_CANDIDATE_VERTICES,
            max_dedup_comparisons: MAX_DEDUP_COMPARISONS,
            max_affine_rank_tests: MAX_AFFINE_RANK_TESTS,
            max_support_plane_vertex_tests: MAX_SUPPORT_PLANE_VERTEX_TESTS,
            max_support_pair_tests: MAX_SUPPORT_PAIR_TESTS,
            max_input_rationals: INPUT_RATIONALS,
            max_input_rational_storage_bits: exact.max_intermediate_bits,
            max_total_input_storage_bits: INPUT_RATIONALS * exact.max_intermediate_bits,
            exact,
        }
    }
}

impl ExactPrismLimits {
    pub(super) fn projected(self) -> Self {
        let hard = Self::default();
        Self {
            max_prisms: self.max_prisms.min(hard.max_prisms),
            max_solid_vertices: self.max_solid_vertices.min(hard.max_solid_vertices),
            max_facets: self.max_facets.min(hard.max_facets),
            max_halfspaces: self.max_halfspaces.min(hard.max_halfspaces),
            max_prism_volume_tests: self.max_prism_volume_tests.min(hard.max_prism_volume_tests),
            max_facet_vertex_checks: self
                .max_facet_vertex_checks
                .min(hard.max_facet_vertex_checks),
            max_plane_triples: self.max_plane_triples.min(hard.max_plane_triples),
            max_singular_plane_triples: self
                .max_singular_plane_triples
                .min(hard.max_singular_plane_triples),
            max_nonsingular_solves: self.max_nonsingular_solves.min(hard.max_nonsingular_solves),
            max_membership_tests: self.max_membership_tests.min(hard.max_membership_tests),
            max_candidate_vertices: self.max_candidate_vertices.min(hard.max_candidate_vertices),
            max_dedup_comparisons: self.max_dedup_comparisons.min(hard.max_dedup_comparisons),
            max_affine_rank_tests: self.max_affine_rank_tests.min(hard.max_affine_rank_tests),
            max_support_plane_vertex_tests: self
                .max_support_plane_vertex_tests
                .min(hard.max_support_plane_vertex_tests),
            max_support_pair_tests: self.max_support_pair_tests.min(hard.max_support_pair_tests),
            max_input_rationals: self.max_input_rationals.min(hard.max_input_rationals),
            max_input_rational_storage_bits: self
                .max_input_rational_storage_bits
                .min(hard.max_input_rational_storage_bits),
            max_total_input_storage_bits: self
                .max_total_input_storage_bits
                .min(hard.max_total_input_storage_bits),
            exact: project_cayley_limits(self.exact, hard.exact),
        }
    }
}

pub(super) fn exact_prism_hard_cayley_limits() -> CayleyLimits {
    let exact = CayleyLimits::default();
    CayleyLimits {
        max_precision_rounds: 0,
        max_guard_bits: 0,
        max_candidate_bits: 0,
        max_machin_terms_per_series: 0,
        max_trig_terms_per_series: 0,
        max_sqrt_refinements: 0,
        max_interval_operations: 65_536,
        max_shift_bits: exact.max_shift_bits,
        max_intermediate_bits: exact.max_intermediate_bits,
        max_gcd_fallback_calls: 8_192,
        max_gcd_fallback_input_bits: exact.max_gcd_fallback_input_bits,
        max_rational_allocations: 65_536,
        max_rational_allocation_bits: exact.max_rational_allocation_bits,
        max_total_rational_allocation_bits: 268_435_456,
        max_output_bits: 0,
    }
}

/// Exact observed work.  A successful result is returned only after every
/// counter has remained within its projected hard ceiling.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct ExactPrismWork {
    pub(super) prisms: usize,
    pub(super) solid_vertices: usize,
    pub(super) facets: usize,
    pub(super) halfspaces: usize,
    pub(super) prism_volume_tests: usize,
    pub(super) facet_vertex_checks: usize,
    pub(super) plane_triples: usize,
    pub(super) singular_plane_triples: usize,
    pub(super) nonsingular_solves: usize,
    pub(super) membership_tests: usize,
    pub(super) candidate_vertices: usize,
    pub(super) dedup_comparisons: usize,
    pub(super) affine_rank_tests: usize,
    pub(super) support_plane_vertex_tests: usize,
    pub(super) support_pair_tests: usize,
    pub(super) input_rationals: usize,
    pub(super) max_input_rational_storage_bits: usize,
    pub(super) total_input_storage_bits: usize,
    /// Exact arithmetic consumed by this A scan only.  For the shared entry
    /// this is the delta merged into, not a copy of, the outer cumulative
    /// meter.
    pub(super) exact: CayleyWork,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ExactPrismError {
    ResourceLimitExceeded,
}

/// Geometric dimension and boundary-support status of the closed
/// intersection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ExactPrismIntersectionKind {
    Empty,
    Point,
    Line,
    /// Rank-two intersection not proved to be a common opposing support.
    Planar,
    /// Rank-two area on a common pair of opposing prism facets.
    CoplanarArea,
    PositiveVolume,
}

/// Exact pair of opposing source facets that supports a rank-two boundary
/// intersection.  Facet indices are local to the first and second prism,
/// respectively.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ExactOpposingSupportWitness {
    first_prism_facet_index: usize,
    second_prism_facet_index: usize,
}

impl ExactOpposingSupportWitness {
    pub(super) fn first_prism_facet_index(self) -> usize {
        self.first_prism_facet_index
    }

    pub(super) fn second_prism_facet_index(self) -> usize {
        self.second_prism_facet_index
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ExactPrismIntersectionReport {
    kind: ExactPrismIntersectionKind,
    affine_rank: Option<u8>,
    opposing_support: Option<ExactOpposingSupportWitness>,
    canonical_vertices: Vec<ExactPoint3>,
}

impl ExactPrismIntersectionReport {
    pub(super) fn kind(&self) -> ExactPrismIntersectionKind {
        self.kind
    }

    pub(super) fn affine_rank(&self) -> Option<u8> {
        self.affine_rank
    }

    pub(super) fn opposing_support(&self) -> Option<ExactOpposingSupportWitness> {
        self.opposing_support
    }

    pub(super) fn common_opposing_support(&self) -> bool {
        self.opposing_support.is_some()
    }

    pub(super) fn canonical_vertices(&self) -> &[ExactPoint3] {
        &self.canonical_vertices
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ExactPrismAnalysis {
    /// `None` means the supplied exact solid was invalid; it is never a
    /// partial resource-exhausted result.
    pub(super) intersection: Option<ExactPrismIntersectionReport>,
    pub(super) work: ExactPrismWork,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ClosedHalfspace {
    normal: ExactVector3,
    offset: BigRational,
    prism_index: usize,
    facet_index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExactTriangularPrism {
    vertices: [ExactPoint3; 6],
    halfspaces: [ClosedHalfspace; 5],
}

/// Computes the complete closed intersection of two validated exact
/// triangular prisms.  This phase-2A API is intentionally not connected to
/// production collision authority.
pub(super) fn analyze_exact_prism_pair_v1(
    first: &ExactTriangularPrismInput,
    second: &ExactTriangularPrismInput,
    limits: ExactPrismLimits,
) -> Result<ExactPrismAnalysis, ExactPrismError> {
    let limits = limits.projected();
    let mut work = ExactPrismWork::default();
    let mut meter = WorkMeter::new(&limits.exact);
    let result = analyze_exact_prism_pair_with_meter_v1(
        first.as_view(),
        second.as_view(),
        limits,
        &mut work,
        &mut meter,
    );
    work.exact = meter.work;
    match result {
        Ok(intersection) => Ok(ExactPrismAnalysis { intersection, work }),
        Err(CayleyError::ResourceLimitExceeded { .. }) => {
            Err(ExactPrismError::ResourceLimitExceeded)
        }
        Err(_) => Ok(ExactPrismAnalysis {
            intersection: None,
            work,
        }),
    }
}

/// Shared-meter entry for the later finite-corridor phase.
///
/// The incoming meter may have a larger combined phase budget and may already
/// contain prior corridor work.  Before any A work, this function snapshots
/// the cumulative meter and non-destructively reserves the complete local A
/// hard envelope inside the remaining outer capacity.  Arithmetic then runs
/// through a local meter, so all additive and maximum-style A limits are live
/// even when an earlier phase has already observed a larger maximum.  This is
/// one shared cumulative finite budget: only the measured A delta is checked
/// into the original cumulative meter, which remains available to phase 3 and
/// is never reset.  Any error invalidates the enclosing analysis and returns
/// no partial report.
pub(super) fn analyze_exact_prism_pair_with_meter_v1(
    first: ExactTriangularPrismView<'_>,
    second: ExactTriangularPrismView<'_>,
    limits: ExactPrismLimits,
    work: &mut ExactPrismWork,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<ExactPrismIntersectionReport>, CayleyError> {
    let limits = limits.projected();
    if !structural_work_is_empty(work) {
        return Err(CayleyError::InvariantFailure { stage: STAGE });
    }
    let exact_start = meter.work.clone();
    preflight_exact_prism_capacity(
        &exact_start,
        &limits.exact,
        meter.limits,
        meter.total_term_limits,
    )?;
    let mut local_meter = WorkMeter::new(&limits.exact);
    let result = calculate_exact_prism_pair(first, second, &limits, work, &mut local_meter);
    let exact_delta = local_meter.work;
    let expected_end =
        exact_start.checked_merge(&exact_delta, meter.limits, meter.total_term_limits, STAGE)?;
    meter.merge_work(&exact_delta, STAGE)?;
    if meter.work != expected_end {
        return Err(CayleyError::InvariantFailure { stage: STAGE });
    }
    work.exact = exact_delta;
    result
}

fn calculate_exact_prism_pair(
    first: ExactTriangularPrismView<'_>,
    second: ExactTriangularPrismView<'_>,
    limits: &ExactPrismLimits,
    work: &mut ExactPrismWork,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<ExactPrismIntersectionReport>, CayleyError> {
    charge_fixed_geometry(work, limits)?;

    let Some(first) = prepare_prism_input(first, limits, work, meter)? else {
        return Ok(None);
    };
    let Some(second) = prepare_prism_input(second, limits, work, meter)? else {
        return Ok(None);
    };
    let Some(first) = build_validated_prism(&first, 0, limits, work, meter)? else {
        return Ok(None);
    };
    let Some(second) = build_validated_prism(&second, 1, limits, work, meter)? else {
        return Ok(None);
    };
    let report = intersect_validated_prisms(&first, &second, limits, work, meter)?;
    Ok(Some(report))
}

fn preflight_exact_prism_capacity(
    start: &CayleyWork,
    local: &CayleyLimits,
    outer: &CayleyLimits,
    total_term_limits: Option<TotalTermLimits>,
) -> Result<(), CayleyError> {
    CayleyWork::default().checked_merge(start, outer, total_term_limits, STAGE)?;
    for (local_maximum, outer_maximum, resource) in [
        (
            local.max_precision_rounds,
            outer.max_precision_rounds,
            "precision_rounds",
        ),
        (local.max_guard_bits, outer.max_guard_bits, "guard_bits"),
        (
            local.max_candidate_bits,
            outer.max_candidate_bits,
            "candidate_bits",
        ),
        (
            local.max_machin_terms_per_series,
            outer.max_machin_terms_per_series,
            "machin_terms",
        ),
        (
            local.max_trig_terms_per_series,
            outer.max_trig_terms_per_series,
            "trig_terms",
        ),
        (
            local.max_sqrt_refinements,
            outer.max_sqrt_refinements,
            "sqrt_refinements",
        ),
        (local.max_shift_bits, outer.max_shift_bits, "shift_bits"),
        (
            local.max_intermediate_bits,
            outer.max_intermediate_bits,
            "intermediate_bits",
        ),
        (
            local.max_rational_allocation_bits,
            outer.max_rational_allocation_bits,
            "rational_allocation_bits",
        ),
        (local.max_output_bits, outer.max_output_bits, "output_bits"),
    ] {
        if local_maximum > outer_maximum {
            return Err(CayleyError::ResourceLimitExceeded {
                stage: STAGE,
                resource,
            });
        }
    }
    for (consumed, local_maximum, outer_maximum, resource) in [
        (
            start.interval_operations,
            local.max_interval_operations,
            outer.max_interval_operations,
            "interval_operations",
        ),
        (
            start.gcd_fallback_calls,
            local.max_gcd_fallback_calls,
            outer.max_gcd_fallback_calls,
            "gcd_fallback_calls",
        ),
        (
            start.gcd_fallback_input_bits,
            local.max_gcd_fallback_input_bits,
            outer.max_gcd_fallback_input_bits,
            "gcd_fallback_input_bits",
        ),
        (
            start.rational_allocations,
            local.max_rational_allocations,
            outer.max_rational_allocations,
            "rational_allocations",
        ),
        (
            start.total_rational_allocation_bits,
            local.max_total_rational_allocation_bits,
            outer.max_total_rational_allocation_bits,
            "total_rational_allocation_bits",
        ),
    ] {
        let reserved =
            consumed
                .checked_add(local_maximum)
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
    if let Some(total) = total_term_limits {
        for (consumed, maximum, resource) in [
            (start.machin_terms, total.machin_terms, "total_machin_terms"),
            (start.trig_terms, total.trig_terms, "total_trig_terms"),
            (
                start.sqrt_refinements,
                total.sqrt_refinements,
                "total_sqrt_refinements",
            ),
        ] {
            if consumed > maximum {
                return Err(CayleyError::ResourceLimitExceeded {
                    stage: STAGE,
                    resource,
                });
            }
        }
    }
    Ok(())
}

fn structural_work_is_empty(work: &ExactPrismWork) -> bool {
    work.prisms == 0
        && work.solid_vertices == 0
        && work.facets == 0
        && work.halfspaces == 0
        && work.prism_volume_tests == 0
        && work.facet_vertex_checks == 0
        && work.plane_triples == 0
        && work.singular_plane_triples == 0
        && work.nonsingular_solves == 0
        && work.membership_tests == 0
        && work.candidate_vertices == 0
        && work.dedup_comparisons == 0
        && work.affine_rank_tests == 0
        && work.support_plane_vertex_tests == 0
        && work.support_pair_tests == 0
        && work.input_rationals == 0
        && work.max_input_rational_storage_bits == 0
        && work.total_input_storage_bits == 0
        && work.exact == CayleyWork::default()
}

fn charge_fixed_geometry(
    work: &mut ExactPrismWork,
    limits: &ExactPrismLimits,
) -> Result<(), CayleyError> {
    set_fixed_counter(
        &mut work.prisms,
        PRISM_COUNT,
        limits.max_prisms,
        "exact_prism_prisms",
    )?;
    set_fixed_counter(
        &mut work.solid_vertices,
        SOLID_VERTEX_COUNT,
        limits.max_solid_vertices,
        "exact_prism_solid_vertices",
    )?;
    set_fixed_counter(
        &mut work.facets,
        FACET_COUNT,
        limits.max_facets,
        "exact_prism_facets",
    )?;
    set_fixed_counter(
        &mut work.halfspaces,
        HALFSPACE_COUNT,
        limits.max_halfspaces,
        "exact_prism_halfspaces",
    )
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

fn prepare_prism_input(
    input: ExactTriangularPrismView<'_>,
    limits: &ExactPrismLimits,
    work: &mut ExactPrismWork,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<ExactTriangularPrismInput>, CayleyError> {
    let Some(mid_surface) =
        try_array3(|vertex| prepare_point_input(input.mid_surface[vertex], limits, work, meter))?
            .into_iter()
            .collect::<Option<Vec<_>>>()
    else {
        return Ok(None);
    };
    let mid_surface: [ExactPoint3; 3] = mid_surface
        .try_into()
        .map_err(|_| CayleyError::InvariantFailure { stage: STAGE })?;
    let Some(material_normal) = prepare_vector_input(input.material_normal, limits, work, meter)?
    else {
        return Ok(None);
    };
    let Some(half_thickness) = prepare_rational_input(input.half_thickness, limits, work, meter)?
    else {
        return Ok(None);
    };
    Ok(Some(ExactTriangularPrismInput {
        mid_surface,
        material_normal,
        half_thickness,
    }))
}

fn prepare_point_input(
    point: &ExactPoint3,
    limits: &ExactPrismLimits,
    work: &mut ExactPrismWork,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<ExactPoint3>, CayleyError> {
    let Some(coordinates) =
        try_array3(|axis| prepare_rational_input(&point.coordinates[axis], limits, work, meter))?
            .into_iter()
            .collect::<Option<Vec<_>>>()
    else {
        return Ok(None);
    };
    let coordinates: [BigRational; 3] = coordinates
        .try_into()
        .map_err(|_| CayleyError::InvariantFailure { stage: STAGE })?;
    Ok(Some(ExactPoint3 { coordinates }))
}

fn prepare_vector_input(
    vector: &ExactVector3,
    limits: &ExactPrismLimits,
    work: &mut ExactPrismWork,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<ExactVector3>, CayleyError> {
    let Some(coordinates) =
        try_array3(|axis| prepare_rational_input(&vector.coordinates[axis], limits, work, meter))?
            .into_iter()
            .collect::<Option<Vec<_>>>()
    else {
        return Ok(None);
    };
    let coordinates: [BigRational; 3] = coordinates
        .try_into()
        .map_err(|_| CayleyError::InvariantFailure { stage: STAGE })?;
    Ok(Some(ExactVector3 { coordinates }))
}

fn prepare_rational_input(
    value: &BigRational,
    limits: &ExactPrismLimits,
    work: &mut ExactPrismWork,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<BigRational>, CayleyError> {
    let storage_bits = rational_storage_bits(value, STAGE)?;
    if storage_bits > limits.max_input_rational_storage_bits {
        return Err(CayleyError::ResourceLimitExceeded {
            stage: STAGE,
            resource: "exact_prism_input_rational_storage_bits",
        });
    }
    let input_rationals =
        work.input_rationals
            .checked_add(1)
            .ok_or(CayleyError::ResourceLimitExceeded {
                stage: STAGE,
                resource: "exact_prism_input_rationals",
            })?;
    let total_input_storage_bits = work
        .total_input_storage_bits
        .checked_add(storage_bits)
        .ok_or(CayleyError::ResourceLimitExceeded {
            stage: STAGE,
            resource: "exact_prism_total_input_storage_bits",
        })?;
    if input_rationals > limits.max_input_rationals {
        return Err(CayleyError::ResourceLimitExceeded {
            stage: STAGE,
            resource: "exact_prism_input_rationals",
        });
    }
    if total_input_storage_bits > limits.max_total_input_storage_bits {
        return Err(CayleyError::ResourceLimitExceeded {
            stage: STAGE,
            resource: "exact_prism_total_input_storage_bits",
        });
    }
    work.input_rationals = input_rationals;
    work.max_input_rational_storage_bits = work.max_input_rational_storage_bits.max(storage_bits);
    work.total_input_storage_bits = total_input_storage_bits;

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

fn build_validated_prism(
    input: &ExactTriangularPrismInput,
    prism_index: usize,
    limits: &ExactPrismLimits,
    work: &mut ExactPrismWork,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<ExactTriangularPrism>, CayleyError> {
    let zero = BigRational::zero();
    let one = BigRational::one();
    if meter.compare_rational(&input.half_thickness, &zero, STAGE)? != Ordering::Greater {
        return Ok(None);
    }

    let edge01 = exact_between(&input.mid_surface[0], &input.mid_surface[1], meter)?;
    let edge02 = exact_between(&input.mid_surface[0], &input.mid_surface[2], meter)?;
    let triangle_normal = exact_cross(&edge01, &edge02, meter)?;
    if exact_vector_is_zero(&triangle_normal) {
        return Ok(None);
    }
    let normal_squared = exact_dot(&input.material_normal, &input.material_normal, meter)?;
    if meter.compare_rational(&normal_squared, &one, STAGE)? != Ordering::Equal {
        return Ok(None);
    }
    for edge in [&edge01, &edge02] {
        let perpendicular = exact_dot(&input.material_normal, edge, meter)?;
        if meter.compare_rational(&perpendicular, &zero, STAGE)? != Ordering::Equal {
            return Ok(None);
        }
    }

    let material_offset = exact_scale_vector(&input.material_normal, &input.half_thickness, meter)?;
    let vertices = [
        exact_offset_point(&input.mid_surface[0], &material_offset, true, meter)?,
        exact_offset_point(&input.mid_surface[1], &material_offset, true, meter)?,
        exact_offset_point(&input.mid_surface[2], &material_offset, true, meter)?,
        exact_offset_point(&input.mid_surface[0], &material_offset, false, meter)?,
        exact_offset_point(&input.mid_surface[1], &material_offset, false, meter)?,
        exact_offset_point(&input.mid_surface[2], &material_offset, false, meter)?,
    ];
    charge_counter(
        &mut work.prism_volume_tests,
        limits.max_prism_volume_tests,
        "exact_prism_volume_tests",
    )?;
    let solid_edge01 = exact_between(&vertices[0], &vertices[1], meter)?;
    let solid_edge02 = exact_between(&vertices[0], &vertices[2], meter)?;
    let solid_edge03 = exact_between(&vertices[0], &vertices[3], meter)?;
    let solid_base_normal = exact_cross(&solid_edge01, &solid_edge02, meter)?;
    let signed_six_volume = exact_dot(&solid_base_normal, &solid_edge03, meter)?;
    if signed_six_volume.is_zero() {
        return Ok(None);
    }
    let centroid = exact_triangle_centroid(&input.mid_surface, meter)?;
    let facet_vertices: [&[usize]; 5] = [
        &[0, 1, 2],
        &[3, 4, 5],
        &[0, 1, 4, 3],
        &[1, 2, 5, 4],
        &[2, 0, 3, 5],
    ];
    let mut incidence = [0_usize; 6];
    let mut halfspaces = Vec::with_capacity(5);
    for (facet_index, expected_vertices) in facet_vertices.iter().enumerate() {
        let Some(halfspace) = build_validated_halfspace(
            &vertices,
            &centroid,
            expected_vertices,
            prism_index,
            facet_index,
            limits,
            work,
            &mut incidence,
            meter,
        )?
        else {
            return Ok(None);
        };
        halfspaces.push(halfspace);
    }
    if incidence != [3; 6] {
        return Ok(None);
    }
    let halfspaces: [ClosedHalfspace; 5] = halfspaces
        .try_into()
        .map_err(|_| CayleyError::InvariantFailure { stage: STAGE })?;
    Ok(Some(ExactTriangularPrism {
        vertices,
        halfspaces,
    }))
}

#[allow(clippy::too_many_arguments)]
fn build_validated_halfspace(
    vertices: &[ExactPoint3; 6],
    centroid: &ExactPoint3,
    expected_vertices: &[usize],
    prism_index: usize,
    facet_index: usize,
    limits: &ExactPrismLimits,
    work: &mut ExactPrismWork,
    incidence: &mut [usize; 6],
    meter: &mut WorkMeter<'_>,
) -> Result<Option<ClosedHalfspace>, CayleyError> {
    let first = expected_vertices[0];
    let edge01 = exact_between(&vertices[first], &vertices[expected_vertices[1]], meter)?;
    let edge02 = exact_between(&vertices[first], &vertices[expected_vertices[2]], meter)?;
    let mut normal = exact_cross(&edge01, &edge02, meter)?;
    if exact_vector_is_zero(&normal) {
        return Ok(None);
    }
    let mut offset = exact_dot_point(&normal, &vertices[first], meter)?;
    let centroid_dot = exact_dot_point(&normal, centroid, meter)?;
    let centroid_side = meter.subtract_rational(&centroid_dot, &offset, STAGE)?;
    match meter.compare_rational(&centroid_side, &BigRational::zero(), STAGE)? {
        Ordering::Equal => return Ok(None),
        Ordering::Greater => {
            normal = exact_negate_vector(&normal, meter)?;
            offset = meter.negate_rational(&offset, STAGE)?;
        }
        Ordering::Less => {}
    }

    let oriented_centroid = exact_dot_point(&normal, centroid, meter)?;
    if meter.compare_rational(&oriented_centroid, &offset, STAGE)? != Ordering::Less {
        return Ok(None);
    }

    let mut statuses = [Ordering::Less; 6];
    for (vertex_index, vertex) in vertices.iter().enumerate() {
        charge_counter(
            &mut work.facet_vertex_checks,
            limits.max_facet_vertex_checks,
            "exact_prism_facet_vertex_checks",
        )?;
        let value = exact_dot_point(&normal, vertex, meter)?;
        let status = meter.compare_rational(&value, &offset, STAGE)?;
        if status == Ordering::Greater {
            return Ok(None);
        }
        if status == Ordering::Equal {
            incidence[vertex_index] = incidence[vertex_index].checked_add(1).ok_or(
                CayleyError::ResourceLimitExceeded {
                    stage: STAGE,
                    resource: "exact_prism_vertex_incidence",
                },
            )?;
        }
        statuses[vertex_index] = status;
    }
    for (vertex_index, status) in statuses.into_iter().enumerate() {
        let expected_on_facet = expected_vertices.contains(&vertex_index);
        if (status == Ordering::Equal) != expected_on_facet {
            return Ok(None);
        }
    }
    Ok(Some(ClosedHalfspace {
        normal,
        offset,
        prism_index,
        facet_index,
    }))
}

fn intersect_validated_prisms(
    first: &ExactTriangularPrism,
    second: &ExactTriangularPrism,
    limits: &ExactPrismLimits,
    work: &mut ExactPrismWork,
    meter: &mut WorkMeter<'_>,
) -> Result<ExactPrismIntersectionReport, CayleyError> {
    debug_assert_eq!(
        first.vertices.len() + second.vertices.len(),
        SOLID_VERTEX_COUNT
    );
    let halfspaces: Vec<&ClosedHalfspace> = first
        .halfspaces
        .iter()
        .chain(second.halfspaces.iter())
        .collect();
    if halfspaces.len() != HALFSPACE_COUNT {
        return Err(CayleyError::InvariantFailure { stage: STAGE });
    }

    let mut canonical_vertices = Vec::new();
    for first_plane in 0..HALFSPACE_COUNT - 2 {
        for second_plane in first_plane + 1..HALFSPACE_COUNT - 1 {
            for third_plane in second_plane + 1..HALFSPACE_COUNT {
                charge_counter(
                    &mut work.plane_triples,
                    limits.max_plane_triples,
                    "exact_prism_plane_triples",
                )?;
                let planes = [
                    halfspaces[first_plane],
                    halfspaces[second_plane],
                    halfspaces[third_plane],
                ];
                let determinant = plane_triple_determinant(&planes, meter)?;
                if determinant.is_zero() {
                    charge_counter(
                        &mut work.singular_plane_triples,
                        limits.max_singular_plane_triples,
                        "exact_prism_singular_plane_triples",
                    )?;
                    continue;
                }
                charge_counter(
                    &mut work.nonsingular_solves,
                    limits.max_nonsingular_solves,
                    "exact_prism_nonsingular_solves",
                )?;
                let candidate = solve_nonsingular_plane_triple(&planes, &determinant, meter)?;
                let mut inside_all = true;
                for halfspace in &halfspaces {
                    charge_counter(
                        &mut work.membership_tests,
                        limits.max_membership_tests,
                        "exact_prism_membership_tests",
                    )?;
                    let value = exact_dot_point(&halfspace.normal, &candidate, meter)?;
                    inside_all &= meter.compare_rational(&value, &halfspace.offset, STAGE)?
                        != Ordering::Greater;
                }
                if !inside_all {
                    continue;
                }
                charge_counter(
                    &mut work.candidate_vertices,
                    limits.max_candidate_vertices,
                    "exact_prism_candidate_vertices",
                )?;
                let mut duplicate = false;
                for retained in &canonical_vertices {
                    charge_counter(
                        &mut work.dedup_comparisons,
                        limits.max_dedup_comparisons,
                        "exact_prism_dedup_comparisons",
                    )?;
                    if canonical_point_eq(retained, &candidate) {
                        duplicate = true;
                        break;
                    }
                }
                if !duplicate {
                    canonical_vertices.push(candidate);
                }
            }
        }
    }
    let accounted_plane_triples = work
        .singular_plane_triples
        .checked_add(work.nonsingular_solves)
        .ok_or(CayleyError::ResourceLimitExceeded {
            stage: STAGE,
            resource: "exact_prism_plane_triples",
        })?;
    if work.plane_triples != PLANE_TRIPLE_COUNT || accounted_plane_triples != PLANE_TRIPLE_COUNT {
        return Err(CayleyError::InvariantFailure { stage: STAGE });
    }

    classify_intersection(canonical_vertices, &halfspaces, limits, work, meter)
}

fn plane_triple_determinant(
    planes: &[&ClosedHalfspace; 3],
    meter: &mut WorkMeter<'_>,
) -> Result<BigRational, CayleyError> {
    let cross12 = exact_cross(&planes[1].normal, &planes[2].normal, meter)?;
    exact_dot(&planes[0].normal, &cross12, meter)
}

fn solve_nonsingular_plane_triple(
    planes: &[&ClosedHalfspace; 3],
    determinant: &BigRational,
    meter: &mut WorkMeter<'_>,
) -> Result<ExactPoint3, CayleyError> {
    debug_assert!(!determinant.is_zero());
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

fn classify_intersection(
    canonical_vertices: Vec<ExactPoint3>,
    halfspaces: &[&ClosedHalfspace],
    limits: &ExactPrismLimits,
    work: &mut ExactPrismWork,
    meter: &mut WorkMeter<'_>,
) -> Result<ExactPrismIntersectionReport, CayleyError> {
    let Some(base) = canonical_vertices.first() else {
        return Ok(ExactPrismIntersectionReport {
            kind: ExactPrismIntersectionKind::Empty,
            affine_rank: None,
            opposing_support: None,
            canonical_vertices,
        });
    };

    // Incremental affine basis: every retained vertex is charged and examined
    // at most once, keeping the complete rank proof within 120 vertex tests.
    let mut rank = 0_u8;
    let mut first_direction = None;
    let mut spanning_normal = None;
    for candidate in canonical_vertices.iter().skip(1) {
        charge_counter(
            &mut work.affine_rank_tests,
            limits.max_affine_rank_tests,
            "exact_prism_affine_rank_tests",
        )?;
        let direction = exact_between(base, candidate, meter)?;
        match rank {
            0 => {
                if !exact_vector_is_zero(&direction) {
                    first_direction = Some(direction);
                    rank = 1;
                }
            }
            1 => {
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
                let spanning_normal = spanning_normal
                    .as_ref()
                    .ok_or(CayleyError::InvariantFailure { stage: STAGE })?;
                let determinant = exact_dot(spanning_normal, &direction, meter)?;
                if !determinant.is_zero() {
                    rank = 3;
                    break;
                }
            }
            _ => return Err(CayleyError::InvariantFailure { stage: STAGE }),
        }
    }

    match rank {
        0 => Ok(ExactPrismIntersectionReport {
            kind: ExactPrismIntersectionKind::Point,
            affine_rank: Some(0),
            opposing_support: None,
            canonical_vertices,
        }),
        1 => Ok(ExactPrismIntersectionReport {
            kind: ExactPrismIntersectionKind::Line,
            affine_rank: Some(1),
            opposing_support: None,
            canonical_vertices,
        }),
        2 => {
            let opposing_support =
                find_common_opposing_support(&canonical_vertices, halfspaces, limits, work, meter)?;
            Ok(ExactPrismIntersectionReport {
                kind: if opposing_support.is_some() {
                    ExactPrismIntersectionKind::CoplanarArea
                } else {
                    ExactPrismIntersectionKind::Planar
                },
                affine_rank: Some(2),
                opposing_support,
                canonical_vertices,
            })
        }
        3 => Ok(ExactPrismIntersectionReport {
            kind: ExactPrismIntersectionKind::PositiveVolume,
            affine_rank: Some(3),
            opposing_support: None,
            canonical_vertices,
        }),
        _ => Err(CayleyError::InvariantFailure { stage: STAGE }),
    }
}

fn find_common_opposing_support(
    vertices: &[ExactPoint3],
    halfspaces: &[&ClosedHalfspace],
    limits: &ExactPrismLimits,
    work: &mut ExactPrismWork,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<ExactOpposingSupportWitness>, CayleyError> {
    let mut active = [true; HALFSPACE_COUNT];
    for (plane_index, halfspace) in halfspaces.iter().enumerate() {
        for vertex in vertices {
            charge_counter(
                &mut work.support_plane_vertex_tests,
                limits.max_support_plane_vertex_tests,
                "exact_prism_support_plane_vertex_tests",
            )?;
            let value = exact_dot_point(&halfspace.normal, vertex, meter)?;
            active[plane_index] &=
                meter.compare_rational(&value, &halfspace.offset, STAGE)? == Ordering::Equal;
        }
    }

    let mut witness = None;
    for first_index in 0..5 {
        for second_index in 5..10 {
            charge_counter(
                &mut work.support_pair_tests,
                limits.max_support_pair_tests,
                "exact_prism_support_pair_tests",
            )?;
            let first = halfspaces[first_index];
            let second = halfspaces[second_index];
            debug_assert_eq!(first.prism_index, 0);
            debug_assert_eq!(second.prism_index, 1);
            debug_assert!(first.facet_index < 5);
            debug_assert!(second.facet_index < 5);
            if !active[first_index] || !active[second_index] {
                continue;
            }
            let cross = exact_cross(&first.normal, &second.normal, meter)?;
            let dot = exact_dot(&first.normal, &second.normal, meter)?;
            if witness.is_none() && exact_vector_is_zero(&cross) && dot.is_negative() {
                witness = Some(ExactOpposingSupportWitness {
                    first_prism_facet_index: first.facet_index,
                    second_prism_facet_index: second.facet_index,
                });
            }
        }
    }
    Ok(witness)
}

fn exact_triangle_centroid(
    triangle: &[ExactPoint3; 3],
    meter: &mut WorkMeter<'_>,
) -> Result<ExactPoint3, CayleyError> {
    let three = BigRational::from_integer(3.into());
    Ok(ExactPoint3 {
        coordinates: try_array3(|axis| {
            let first_two = meter.add_rational(
                &triangle[0].coordinates[axis],
                &triangle[1].coordinates[axis],
                STAGE,
            )?;
            let sum = meter.add_rational(&first_two, &triangle[2].coordinates[axis], STAGE)?;
            meter.divide_rational(&sum, &three, STAGE)
        })?,
    })
}

fn exact_offset_point(
    point: &ExactPoint3,
    offset: &ExactVector3,
    subtract: bool,
    meter: &mut WorkMeter<'_>,
) -> Result<ExactPoint3, CayleyError> {
    Ok(ExactPoint3 {
        coordinates: try_array3(|axis| {
            if subtract {
                meter.subtract_rational(&point.coordinates[axis], &offset.coordinates[axis], STAGE)
            } else {
                meter.add_rational(&point.coordinates[axis], &offset.coordinates[axis], STAGE)
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

fn exact_vector_is_zero(vector: &ExactVector3) -> bool {
    vector.coordinates.iter().all(BigRational::is_zero)
}

#[cfg(test)]
mod tests {
    use num_bigint::BigInt;
    use num_integer::Integer;
    use num_rational::BigRational;
    use num_traits::{One, Signed};

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

    fn prism(mid_surface: [[i64; 3]; 3]) -> ExactTriangularPrismInput {
        ExactTriangularPrismInput {
            mid_surface: mid_surface.map(|[x, y, z]| point(x, y, z)),
            material_normal: vector(0, 0, 1),
            half_thickness: integer(1),
        }
    }

    fn first_prism() -> ExactTriangularPrismInput {
        prism([[0, 0, 0], [2, 0, 0], [0, 2, 0]])
    }

    fn report(
        first: &ExactTriangularPrismInput,
        second: &ExactTriangularPrismInput,
    ) -> ExactPrismAnalysis {
        analyze_exact_prism_pair_v1(first, second, ExactPrismLimits::default()).unwrap()
    }

    fn kind(
        first: &ExactTriangularPrismInput,
        second: &ExactTriangularPrismInput,
    ) -> ExactPrismIntersectionKind {
        report(first, second).intersection.unwrap().kind
    }

    fn same_vertex_set(first: &[ExactPoint3], second: &[ExactPoint3]) -> bool {
        first.len() == second.len()
            && first.iter().all(|point| {
                second
                    .iter()
                    .any(|candidate| canonical_point_eq(point, candidate))
            })
    }

    fn limits_from_work(work: &ExactPrismWork) -> ExactPrismLimits {
        ExactPrismLimits {
            max_prisms: work.prisms,
            max_solid_vertices: work.solid_vertices,
            max_facets: work.facets,
            max_halfspaces: work.halfspaces,
            max_prism_volume_tests: work.prism_volume_tests,
            max_facet_vertex_checks: work.facet_vertex_checks,
            max_plane_triples: work.plane_triples,
            max_singular_plane_triples: work.singular_plane_triples,
            max_nonsingular_solves: work.nonsingular_solves,
            max_membership_tests: work.membership_tests,
            max_candidate_vertices: work.candidate_vertices,
            max_dedup_comparisons: work.dedup_comparisons,
            max_affine_rank_tests: work.affine_rank_tests,
            max_support_plane_vertex_tests: work.support_plane_vertex_tests,
            max_support_pair_tests: work.support_pair_tests,
            max_input_rationals: work.input_rationals,
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
        }
    }

    #[test]
    fn synthetic_empty_point_line_area_and_volume_are_distinguished() {
        let first = first_prism();
        let empty = prism([[0, 0, 4], [2, 0, 4], [0, 2, 4]]);
        let point_contact = prism([[2, 0, 2], [4, 0, 2], [2, -2, 2]]);
        let line_contact = prism([[2, 0, 0], [4, 0, 0], [2, -2, 0]]);
        let area_contact = prism([[0, 0, 2], [2, 0, 2], [0, 2, 2]]);
        let volume = first.clone();

        assert_eq!(kind(&first, &empty), ExactPrismIntersectionKind::Empty);
        assert_eq!(
            kind(&first, &point_contact),
            ExactPrismIntersectionKind::Point
        );
        assert_eq!(
            kind(&first, &line_contact),
            ExactPrismIntersectionKind::Line
        );
        assert_eq!(
            kind(&first, &area_contact),
            ExactPrismIntersectionKind::CoplanarArea
        );
        assert_eq!(
            kind(&first, &volume),
            ExactPrismIntersectionKind::PositiveVolume
        );
    }

    #[test]
    fn kind_rank_support_and_vertex_count_invariants_are_sealed_together() {
        let first = first_prism();
        let fixtures = [
            (
                prism([[0, 0, 4], [2, 0, 4], [0, 2, 4]]),
                ExactPrismIntersectionKind::Empty,
                None,
                false,
                0,
            ),
            (
                prism([[2, 0, 2], [4, 0, 2], [2, -2, 2]]),
                ExactPrismIntersectionKind::Point,
                Some(0),
                false,
                1,
            ),
            (
                prism([[2, 0, 0], [4, 0, 0], [2, -2, 0]]),
                ExactPrismIntersectionKind::Line,
                Some(1),
                false,
                2,
            ),
            (
                prism([[0, 0, 2], [2, 0, 2], [0, 2, 2]]),
                ExactPrismIntersectionKind::CoplanarArea,
                Some(2),
                true,
                3,
            ),
            (
                first.clone(),
                ExactPrismIntersectionKind::PositiveVolume,
                Some(3),
                false,
                6,
            ),
        ];

        for (second, expected_kind, rank, support, vertices) in fixtures {
            let intersection = report(&first, &second).intersection.unwrap();
            assert_eq!(intersection.kind(), expected_kind);
            assert_eq!(intersection.affine_rank(), rank);
            assert_eq!(intersection.common_opposing_support(), support);
            assert_eq!(intersection.opposing_support().is_some(), support);
            assert_eq!(intersection.canonical_vertices().len(), vertices);
        }
    }

    #[test]
    fn one_prism_is_six_vertices_five_closed_facets_and_nonzero_volume() {
        let input = first_prism();
        let limits = ExactPrismLimits::default();
        let mut work = ExactPrismWork::default();
        let mut meter = WorkMeter::new(&limits.exact);
        charge_fixed_geometry(&mut work, &limits).unwrap();
        let prepared = prepare_prism_input(input.as_view(), &limits, &mut work, &mut meter)
            .unwrap()
            .unwrap();
        let solid = build_validated_prism(&prepared, 0, &limits, &mut work, &mut meter)
            .unwrap()
            .unwrap();

        assert_eq!(solid.vertices.len(), 6);
        assert_eq!(solid.halfspaces.len(), 5);
        assert_eq!(work.prism_volume_tests, 1);
        assert_eq!(work.facet_vertex_checks, 30);
        for vertex in &solid.vertices {
            let incident = solid
                .halfspaces
                .iter()
                .filter(|halfspace| {
                    let value = (0..3)
                        .map(|axis| &halfspace.normal.coordinates[axis] * &vertex.coordinates[axis])
                        .sum::<BigRational>();
                    assert!(value <= halfspace.offset);
                    value == halfspace.offset
                })
                .count();
            assert_eq!(incident, 3);
        }
    }

    #[test]
    fn invalid_or_noncanonical_prism_inputs_never_issue_a_report() {
        let valid = first_prism();
        let mut fixtures = Vec::new();

        let mut zero_thickness = valid.clone();
        zero_thickness.half_thickness = integer(0);
        fixtures.push(zero_thickness);

        let mut negative_thickness = valid.clone();
        negative_thickness.half_thickness = integer(-1);
        fixtures.push(negative_thickness);

        let mut nonunit_normal = valid.clone();
        nonunit_normal.material_normal = vector(0, 0, 2);
        fixtures.push(nonunit_normal);

        let mut nonperpendicular_normal = valid.clone();
        nonperpendicular_normal.material_normal = vector(1, 0, 0);
        fixtures.push(nonperpendicular_normal);

        let mut degenerate_triangle = valid.clone();
        degenerate_triangle.mid_surface[2] = point(4, 0, 0);
        fixtures.push(degenerate_triangle);

        let mut noncanonical = valid.clone();
        noncanonical.half_thickness = BigRational::new_raw(BigInt::from(2), BigInt::from(2));
        fixtures.push(noncanonical);

        for invalid in fixtures {
            let analysis =
                analyze_exact_prism_pair_v1(&invalid, &valid, ExactPrismLimits::default()).unwrap();
            assert!(analysis.intersection.is_none());
        }
    }

    #[test]
    fn feasible_plane_triples_are_canonically_deduplicated() {
        let first = first_prism();
        let analysis = report(&first, &first);
        let intersection = analysis.intersection.unwrap();
        assert_eq!(
            intersection.kind(),
            ExactPrismIntersectionKind::PositiveVolume
        );
        assert_eq!(intersection.canonical_vertices().len(), 6);
        assert!(analysis.work.candidate_vertices > 6);
        assert!(analysis.work.dedup_comparisons > 0);
        for coordinate in intersection
            .canonical_vertices()
            .iter()
            .flat_map(|point| &point.coordinates)
        {
            assert!(coordinate.denom().is_positive());
            assert!(coordinate.numer().gcd(coordinate.denom()).is_one());
        }
    }

    #[test]
    fn every_plane_triple_is_scanned_and_singular_triples_are_not_solved() {
        let analysis = report(&first_prism(), &first_prism());
        assert_eq!(analysis.work.plane_triples, PLANE_TRIPLE_COUNT);
        assert!(analysis.work.singular_plane_triples > 0);
        assert_eq!(
            analysis.work.singular_plane_triples + analysis.work.nonsingular_solves,
            PLANE_TRIPLE_COUNT
        );
        assert_eq!(
            analysis.work.membership_tests,
            analysis.work.nonsingular_solves * HALFSPACE_COUNT
        );
    }

    #[test]
    fn triangle_vertex_order_prism_order_and_huge_translation_preserve_kind() {
        let first = first_prism();
        let second = prism([[0, 0, 2], [2, 0, 2], [0, 2, 2]]);
        let expected = report(&first, &second).intersection.unwrap();
        let permutations = [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ];
        for first_order in permutations {
            for second_order in permutations {
                let mut reordered_first = first.clone();
                reordered_first.mid_surface =
                    first_order.map(|index| first.mid_surface[index].clone());
                let mut reordered_second = second.clone();
                reordered_second.mid_surface =
                    second_order.map(|index| second.mid_surface[index].clone());
                let observed = report(&reordered_first, &reordered_second)
                    .intersection
                    .unwrap();
                assert_eq!(observed.kind(), expected.kind());
                assert_eq!(observed.affine_rank(), expected.affine_rank());
                assert_eq!(
                    observed.common_opposing_support(),
                    expected.common_opposing_support()
                );
                assert!(same_vertex_set(
                    observed.canonical_vertices(),
                    expected.canonical_vertices()
                ));
            }
        }
        let swapped = report(&second, &first).intersection.unwrap();
        assert_eq!(swapped.kind(), expected.kind());
        assert_eq!(swapped.affine_rank(), expected.affine_rank());
        assert!(same_vertex_set(
            swapped.canonical_vertices(),
            expected.canonical_vertices()
        ));

        let translate = |mut input: ExactTriangularPrismInput| {
            for point in &mut input.mid_surface {
                for coordinate in &mut point.coordinates {
                    *coordinate += integer(1_000_000_000_000_000);
                }
            }
            input
        };
        let translated = report(&translate(first), &translate(second))
            .intersection
            .unwrap();
        assert_eq!(translated.kind(), expected.kind());
        assert_eq!(translated.affine_rank(), expected.affine_rank());
        assert_eq!(
            translated.common_opposing_support(),
            expected.common_opposing_support()
        );
        assert_eq!(
            translated.canonical_vertices().len(),
            expected.canonical_vertices().len()
        );
    }

    #[test]
    fn closed_halfspaces_keep_exact_boundary_candidates() {
        let first = first_prism();
        let cap_contact = prism([[0, 0, 2], [2, 0, 2], [0, 2, 2]]);
        let analysis = report(&first, &cap_contact);
        let intersection = analysis.intersection.unwrap();
        assert_eq!(intersection.kind, ExactPrismIntersectionKind::CoplanarArea);
        assert_eq!(intersection.affine_rank, Some(2));
        assert!(intersection.common_opposing_support());
        let witness = intersection.opposing_support().unwrap();
        assert_eq!(witness.first_prism_facet_index(), 1);
        assert_eq!(witness.second_prism_facet_index(), 0);
        assert_eq!(intersection.canonical_vertices.len(), 3);
        assert!(
            intersection
                .canonical_vertices
                .iter()
                .all(|point| point.coordinates[2] == integer(1))
        );
    }

    #[test]
    fn candidate_after_one_short_limit_fails_atomically() {
        let first = first_prism();
        let baseline = report(&first, &first);
        assert!(baseline.work.candidate_vertices > 0);
        let limits = ExactPrismLimits {
            max_candidate_vertices: baseline.work.candidate_vertices - 1,
            ..ExactPrismLimits::default()
        };

        assert_eq!(
            analyze_exact_prism_pair_v1(&first, &first, limits),
            Err(ExactPrismError::ResourceLimitExceeded)
        );
    }

    #[test]
    fn every_observed_counter_has_exact_and_one_short_limits() {
        let first = first_prism();
        let second = prism([[0, 0, 2], [2, 0, 2], [0, 2, 2]]);
        let baseline = report(&first, &second);
        let exact_limits = limits_from_work(&baseline.work);
        let exact = analyze_exact_prism_pair_v1(&first, &second, exact_limits).unwrap();
        assert!(exact.intersection.is_some());
        assert_eq!(exact, baseline);

        let assert_one_short = |resource: &str, limits: ExactPrismLimits| {
            assert_eq!(
                analyze_exact_prism_pair_v1(&first, &second, limits),
                Err(ExactPrismError::ResourceLimitExceeded),
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
        structural_one_short!(max_prisms);
        structural_one_short!(max_solid_vertices);
        structural_one_short!(max_facets);
        structural_one_short!(max_halfspaces);
        structural_one_short!(max_prism_volume_tests);
        structural_one_short!(max_facet_vertex_checks);
        structural_one_short!(max_plane_triples);
        structural_one_short!(max_singular_plane_triples);
        structural_one_short!(max_nonsingular_solves);
        structural_one_short!(max_membership_tests);
        structural_one_short!(max_candidate_vertices);
        structural_one_short!(max_dedup_comparisons);
        structural_one_short!(max_affine_rank_tests);
        structural_one_short!(max_support_plane_vertex_tests);
        structural_one_short!(max_support_pair_tests);
        structural_one_short!(max_input_rationals);
        structural_one_short!(max_input_rational_storage_bits);
        structural_one_short!(max_total_input_storage_bits);

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
    fn shared_cumulative_budget_preserves_prior_work_merges_a_delta_and_never_resets() {
        let first = first_prism();
        let second = prism([[0, 0, 2], [2, 0, 2], [0, 2, 2]]);
        let baseline = report(&first, &second);
        let prior_operations = 7;
        let mut combined = exact_prism_hard_cayley_limits();
        combined.max_interval_operations = combined
            .max_interval_operations
            .checked_add(prior_operations)
            .unwrap();
        let mut meter = WorkMeter::new(&combined);
        for _ in 0..prior_operations {
            meter.operation(STAGE).unwrap();
        }
        let before = meter.work.clone();
        let mut work = ExactPrismWork::default();
        let intersection = analyze_exact_prism_pair_with_meter_v1(
            first.as_view(),
            second.as_view(),
            ExactPrismLimits::default(),
            &mut work,
            &mut meter,
        )
        .unwrap();

        assert!(intersection.is_some());
        assert_eq!(work, baseline.work);
        assert_eq!(
            meter.work.interval_operations,
            before.interval_operations + work.exact.interval_operations
        );
        assert_eq!(
            meter.work.gcd_fallback_calls,
            before.gcd_fallback_calls + work.exact.gcd_fallback_calls
        );
        let after_a = meter.work.interval_operations;
        meter.operation(STAGE).unwrap();
        assert_eq!(meter.work.interval_operations, after_a + 1);
    }

    #[test]
    fn outer_one_short_reservation_fails_before_any_a_work_or_meter_mutation() {
        let first = first_prism();
        let local = ExactPrismLimits::default();
        let mut outer = exact_prism_hard_cayley_limits();
        outer.max_interval_operations = local.exact.max_interval_operations - 1;
        let mut meter = WorkMeter::new(&outer);
        let before = meter.work.clone();
        let mut work = ExactPrismWork::default();

        assert!(matches!(
            analyze_exact_prism_pair_with_meter_v1(
                first.as_view(),
                first.as_view(),
                local,
                &mut work,
                &mut meter,
            ),
            Err(CayleyError::ResourceLimitExceeded { .. })
        ));
        assert_eq!(work, ExactPrismWork::default());
        assert_eq!(meter.work, before);
    }

    #[test]
    fn prior_large_outer_maximum_cannot_hide_a_local_one_short_failure() {
        let first = first_prism();
        let second = prism([[0, 0, 2], [2, 0, 2], [0, 2, 2]]);
        let baseline = report(&first, &second);
        let mut local = limits_from_work(&baseline.work);
        assert!(local.exact.max_intermediate_bits > 0);
        local.exact.max_intermediate_bits -= 1;

        let mut combined = exact_prism_hard_cayley_limits();
        combined.max_intermediate_bits = exact_prism_hard_cayley_limits()
            .max_intermediate_bits
            .checked_mul(2)
            .unwrap();
        combined.max_rational_allocations =
            combined.max_rational_allocations.checked_add(1).unwrap();
        combined.max_total_rational_allocation_bits = combined
            .max_total_rational_allocation_bits
            .checked_add(2_048)
            .unwrap();
        let mut meter = WorkMeter::new(&combined);
        let large = BigRational::from_integer(BigInt::from(1) << 1_024);
        meter.clone_rational(&large, STAGE).unwrap();
        assert!(meter.work.max_preflight_bits > local.exact.max_intermediate_bits);
        let before = meter.work.clone();
        let mut work = ExactPrismWork::default();

        assert!(matches!(
            analyze_exact_prism_pair_with_meter_v1(
                first.as_view(),
                second.as_view(),
                local,
                &mut work,
                &mut meter,
            ),
            Err(CayleyError::ResourceLimitExceeded { .. })
        ));
        assert!(work.exact.max_preflight_bits <= local.exact.max_intermediate_bits);
        assert_eq!(
            meter.work.interval_operations,
            before.interval_operations + work.exact.interval_operations
        );
    }

    #[test]
    fn counter_and_capacity_overflow_fail_without_wrapping() {
        let mut counter = usize::MAX;
        assert!(matches!(
            charge_counter(&mut counter, usize::MAX, "overflow"),
            Err(CayleyError::ResourceLimitExceeded { .. })
        ));
        assert_eq!(counter, usize::MAX);
        assert!(matches!(
            preflight_exact_prism_capacity(
                &CayleyWork {
                    interval_operations: usize::MAX,
                    ..CayleyWork::default()
                },
                &CayleyLimits {
                    max_interval_operations: 1,
                    ..exact_prism_hard_cayley_limits()
                },
                &CayleyLimits {
                    max_interval_operations: usize::MAX,
                    ..exact_prism_hard_cayley_limits()
                },
                None,
            ),
            Err(CayleyError::ResourceLimitExceeded { .. })
        ));

        let limits = ExactPrismLimits::default();
        let mut work = ExactPrismWork {
            total_input_storage_bits: usize::MAX,
            ..ExactPrismWork::default()
        };
        let mut meter = WorkMeter::new(&limits.exact);
        assert!(matches!(
            prepare_rational_input(&integer(1), &limits, &mut work, &mut meter),
            Err(CayleyError::ResourceLimitExceeded { .. })
        ));
        assert_eq!(work.total_input_storage_bits, usize::MAX);
    }

    #[test]
    fn caller_cannot_expand_any_hard_limit() {
        let first = first_prism();
        let baseline = report(&first, &first);
        let oversized = ExactPrismLimits {
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
        let observed = analyze_exact_prism_pair_v1(&first, &first, oversized).unwrap();
        assert_eq!(observed, baseline);
    }
}
