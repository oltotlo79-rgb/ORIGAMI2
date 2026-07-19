//! Private, axis-aligned reconciliation of the exact pose `E` and native
//! binary64 affine pose `F` for the one-hinge/two-triangle positive-thickness
//! prerequisite.
//!
//! This module deliberately emits no collision classification, corridor
//! containment proof, safe-set authority, persistence value, or UI payload.
//! Its capability is usable only with the exact finite-hinge prerequisite
//! that was supplied at issuance.
//!
//! Every stored point, normal, and solid error is an axis-aligned
//! componentwise bound. `*_linf_*` is the maximum of those three component
//! bounds. It is **not** a Euclidean distance:
//!
//! - `point_component_bound_mm[k] = max_boundary |F(v)[k] - E(v)[k]|`;
//! - `normal_component_bound[k] = |F.rotation[k][1] - E.rotation[k][1]|`;
//! - `solid_component_bound_mm[k] = point[k] + h * normal[k]`, where `h` is
//!   the exact rational lift of the binary64 paper thickness divided by two.

use std::cmp::Ordering;

use num_rational::BigRational;
use num_traits::Zero;
use ori_domain::FaceId;
use ori_kinematics::{BoundMaterialTreePose, RigidTransform};

use super::*;

const FACE_COUNT: usize = 2;
const HINGE_COUNT: usize = 1;
const BOUNDARY_OCCURRENCES: usize = 6;
const TRANSFORM_SCALAR_LIFTS: usize = FACE_COUNT * 12;
const TRANSFORM_BIT_BINDINGS: usize = FACE_COUNT * 12;
const SOURCE_COORDINATE_LIFTS: usize = BOUNDARY_OCCURRENCES * 3;
const CURRENT_POINT_RECONSTRUCTIONS: usize = BOUNDARY_OCCURRENCES;
const POINT_COMPONENT_BOUNDS: usize = BOUNDARY_OCCURRENCES * 3;
const NORMAL_COMPONENT_BOUNDS: usize = FACE_COUNT * 3;
const SOLID_COMPONENT_BOUNDS: usize = (FACE_COUNT + 1) * 3;
const FACE_ERROR_RECORDS: usize = FACE_COUNT;
const THICKNESS_LIFTS: usize = 1;
const HALF_THICKNESS_DIVISIONS: usize = 1;

/// Hard, caller-non-expandable limits for the private E/F boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct AxisAlignedEfBoundaryLimits {
    pub(super) max_authenticated_faces: usize,
    pub(super) max_authenticated_hinges: usize,
    pub(super) max_boundary_occurrences: usize,
    pub(super) max_transform_scalar_lifts: usize,
    pub(super) max_transform_bit_bindings: usize,
    pub(super) max_source_coordinate_lifts: usize,
    pub(super) max_current_point_reconstructions: usize,
    pub(super) max_point_component_bounds: usize,
    pub(super) max_normal_component_bounds: usize,
    pub(super) max_solid_component_bounds: usize,
    pub(super) max_face_error_records: usize,
    pub(super) max_thickness_lifts: usize,
    pub(super) max_half_thickness_divisions: usize,
    pub(super) exact: CayleyLimits,
}

impl Default for AxisAlignedEfBoundaryLimits {
    fn default() -> Self {
        Self {
            max_authenticated_faces: FACE_COUNT,
            max_authenticated_hinges: HINGE_COUNT,
            max_boundary_occurrences: BOUNDARY_OCCURRENCES,
            max_transform_scalar_lifts: TRANSFORM_SCALAR_LIFTS,
            max_transform_bit_bindings: TRANSFORM_BIT_BINDINGS,
            max_source_coordinate_lifts: SOURCE_COORDINATE_LIFTS,
            max_current_point_reconstructions: CURRENT_POINT_RECONSTRUCTIONS,
            max_point_component_bounds: POINT_COMPONENT_BOUNDS,
            max_normal_component_bounds: NORMAL_COMPONENT_BOUNDS,
            max_solid_component_bounds: SOLID_COMPONENT_BOUNDS,
            max_face_error_records: FACE_ERROR_RECORDS,
            max_thickness_lifts: THICKNESS_LIFTS,
            max_half_thickness_divisions: HALF_THICKNESS_DIVISIONS,
            exact: ef_boundary_exact_hard_limits(),
        }
    }
}

impl AxisAlignedEfBoundaryLimits {
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
            max_transform_scalar_lifts: self
                .max_transform_scalar_lifts
                .min(hard.max_transform_scalar_lifts),
            max_transform_bit_bindings: self
                .max_transform_bit_bindings
                .min(hard.max_transform_bit_bindings),
            max_source_coordinate_lifts: self
                .max_source_coordinate_lifts
                .min(hard.max_source_coordinate_lifts),
            max_current_point_reconstructions: self
                .max_current_point_reconstructions
                .min(hard.max_current_point_reconstructions),
            max_point_component_bounds: self
                .max_point_component_bounds
                .min(hard.max_point_component_bounds),
            max_normal_component_bounds: self
                .max_normal_component_bounds
                .min(hard.max_normal_component_bounds),
            max_solid_component_bounds: self
                .max_solid_component_bounds
                .min(hard.max_solid_component_bounds),
            max_face_error_records: self.max_face_error_records.min(hard.max_face_error_records),
            max_thickness_lifts: self.max_thickness_lifts.min(hard.max_thickness_lifts),
            max_half_thickness_divisions: self
                .max_half_thickness_divisions
                .min(hard.max_half_thickness_divisions),
            exact: project_cayley_limits(self.exact, hard.exact),
        }
    }
}

fn ef_boundary_exact_hard_limits() -> CayleyLimits {
    let exact = CayleyLimits::default();
    CayleyLimits {
        max_precision_rounds: 0,
        max_guard_bits: 0,
        max_candidate_bits: 0,
        max_machin_terms_per_series: 0,
        max_trig_terms_per_series: 0,
        max_sqrt_refinements: 0,
        max_interval_operations: 8_192,
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

/// Exact observed work. Every structural field has a fixed hard ceiling and
/// participates in exact/one-short regression.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct AxisAlignedEfBoundaryWork {
    pub(super) authenticated_faces: usize,
    pub(super) authenticated_hinges: usize,
    pub(super) boundary_occurrences: usize,
    pub(super) transform_scalar_lifts: usize,
    pub(super) transform_bit_bindings: usize,
    pub(super) source_coordinate_lifts: usize,
    pub(super) current_point_reconstructions: usize,
    pub(super) point_component_bounds: usize,
    pub(super) normal_component_bounds: usize,
    pub(super) solid_component_bounds: usize,
    pub(super) face_error_records: usize,
    pub(super) thickness_lifts: usize,
    pub(super) half_thickness_divisions: usize,
    pub(super) exact: CayleyWork,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AxisAlignedEfBoundaryError {
    ResourceLimitExceeded,
}

/// One face's exact axis-aligned error bounds.
///
/// The `*_linf_*` values are component maxima, not Euclidean norms.
#[derive(Debug)]
pub(super) struct AxisAlignedFaceEfErrorBounds {
    pub(super) face: FaceId,
    pub(super) point_component_bound_mm: [BigRational; 3],
    pub(super) normal_component_bound: [BigRational; 3],
    pub(super) solid_component_bound_mm: [BigRational; 3],
    pub(super) point_linf_bound_mm: BigRational,
    pub(super) normal_linf_bound: BigRational,
    pub(super) solid_linf_bound_mm: BigRational,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct BoundBinary64FaceTransformBits {
    pub(super) face: FaceId,
    pub(super) rotation: [[u64; 3]; 3],
    pub(super) translation: [u64; 3],
}

/// Non-serializable, non-cloneable E/F boundary capability.
///
/// It retains the exact prerequisite by borrow, the exact pose by pointer,
/// the native model/pose issuer pair, bit-exact thickness, authenticated
/// left/right/hinge indexes, and all binary64 transform coefficient bits.
/// No caller-provided radius or transform can enlarge these bounds.
///
/// `faces` retains each face's componentwise result. The component arrays on
/// this capability take the componentwise maximum across both faces; the
/// global solid array is then recomputed conservatively as global
/// `point[k] + h * normal[k]`. All `*_linf_*` fields are exact maxima over
/// the corresponding three-axis array.
#[derive(Debug)]
pub(super) struct AxisAlignedEfBoundaryCapabilityV1<'prerequisite, 'exact, 'pose> {
    pub(super) prerequisite:
        &'prerequisite AuthenticatedSingleTriangularHingePrerequisitesV1<'exact, 'pose>,
    pub(super) exact: &'exact RationalCayleyTreePose<'pose>,
    pub(super) bound: BoundMaterialTreePose<'pose>,
    pub(super) paper_thickness_bits: u64,
    pub(super) left_face_index: usize,
    pub(super) right_face_index: usize,
    pub(super) hinge_index: usize,
    pub(super) binary64_face_transforms: [BoundBinary64FaceTransformBits; FACE_COUNT],
    pub(super) faces: [AxisAlignedFaceEfErrorBounds; FACE_COUNT],
    pub(super) point_component_bound_mm: [BigRational; 3],
    pub(super) normal_component_bound: [BigRational; 3],
    pub(super) solid_component_bound_mm: [BigRational; 3],
    pub(super) point_linf_bound_mm: BigRational,
    pub(super) normal_linf_bound: BigRational,
    pub(super) solid_linf_bound_mm: BigRational,
}

#[cfg(test)]
impl AxisAlignedEfBoundaryCapabilityV1<'_, '_, '_> {
    pub(super) const fn scalar_count_for_test(&self) -> usize {
        FACE_COUNT * 12 + 12
    }

    pub(super) fn adjust_scalar_for_test(&mut self, target: usize, delta: i64) {
        let delta = BigRational::from_integer(delta.into());
        let mut current = 0_usize;
        let mut found = false;
        for face in &mut self.faces {
            for values in [
                &mut face.point_component_bound_mm,
                &mut face.normal_component_bound,
                &mut face.solid_component_bound_mm,
            ] {
                for value in values {
                    if current == target {
                        *value += &delta;
                        found = true;
                    }
                    current += 1;
                }
            }
            for value in [
                &mut face.point_linf_bound_mm,
                &mut face.normal_linf_bound,
                &mut face.solid_linf_bound_mm,
            ] {
                if current == target {
                    *value += &delta;
                    found = true;
                }
                current += 1;
            }
        }
        for values in [
            &mut self.point_component_bound_mm,
            &mut self.normal_component_bound,
            &mut self.solid_component_bound_mm,
        ] {
            for value in values {
                if current == target {
                    *value += &delta;
                    found = true;
                }
                current += 1;
            }
        }
        for value in [
            &mut self.point_linf_bound_mm,
            &mut self.normal_linf_bound,
            &mut self.solid_linf_bound_mm,
        ] {
            if current == target {
                *value += &delta;
                found = true;
            }
            current += 1;
        }
        assert_eq!(current, self.scalar_count_for_test());
        assert!(found, "E/F scalar {target} must exist");
    }
}

#[derive(Debug)]
pub(super) struct RevalidatedAxisAlignedEfBoundaryCapabilityV1<
    'capability,
    'prerequisite,
    'exact,
    'pose,
> {
    pub(super) capability:
        &'capability AxisAlignedEfBoundaryCapabilityV1<'prerequisite, 'exact, 'pose>,
}

#[derive(Debug)]
pub(super) struct AxisAlignedEfBoundaryAnalysis<'prerequisite, 'exact, 'pose> {
    pub(super) capability: Option<AxisAlignedEfBoundaryCapabilityV1<'prerequisite, 'exact, 'pose>>,
    pub(super) work: AxisAlignedEfBoundaryWork,
}

/// Measures `F-E` only after consuming the existing finite-hinge
/// prerequisite. This function remains disconnected from production
/// collision classification.
pub(super) fn analyze_axis_aligned_ef_boundary_v1<'prerequisite, 'exact, 'pose>(
    prerequisite: &'prerequisite AuthenticatedSingleTriangularHingePrerequisitesV1<'exact, 'pose>,
    exact: &'exact RationalCayleyTreePose<'pose>,
    bound: BoundMaterialTreePose<'pose>,
    paper_thickness_mm: f64,
    limits: AxisAlignedEfBoundaryLimits,
) -> Result<AxisAlignedEfBoundaryAnalysis<'prerequisite, 'exact, 'pose>, AxisAlignedEfBoundaryError>
{
    let limits = limits.projected();
    let mut work = AxisAlignedEfBoundaryWork::default();
    let mut meter = WorkMeter::new(&limits.exact);
    let result = calculate_axis_aligned_ef_boundary_v1(
        prerequisite,
        exact,
        bound,
        paper_thickness_mm,
        &limits,
        &mut work,
        &mut meter,
    );
    work.exact = meter.work;
    match result {
        Ok(capability) => Ok(AxisAlignedEfBoundaryAnalysis { capability, work }),
        Err(CayleyError::ResourceLimitExceeded { .. }) => {
            Err(AxisAlignedEfBoundaryError::ResourceLimitExceeded)
        }
        Err(_) => Ok(AxisAlignedEfBoundaryAnalysis {
            capability: None,
            work,
        }),
    }
}

fn calculate_axis_aligned_ef_boundary_v1<'prerequisite, 'exact, 'pose>(
    prerequisite: &'prerequisite AuthenticatedSingleTriangularHingePrerequisitesV1<'exact, 'pose>,
    exact: &'exact RationalCayleyTreePose<'pose>,
    bound: BoundMaterialTreePose<'pose>,
    paper_thickness_mm: f64,
    limits: &AxisAlignedEfBoundaryLimits,
    work: &mut AxisAlignedEfBoundaryWork,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<AxisAlignedEfBoundaryCapabilityV1<'prerequisite, 'exact, 'pose>>, CayleyError> {
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
    {
        return Ok(None);
    }

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
        return Ok(None);
    }
    charge_fixed_work(work, limits)?;

    let binary64_face_transforms = [
        capture_binary64_transform_bits(bound, exact.faces[0].face)
            .ok_or(CayleyError::InvariantFailure { stage: STAGE })?,
        capture_binary64_transform_bits(bound, exact.faces[1].face)
            .ok_or(CayleyError::InvariantFailure { stage: STAGE })?,
    ];

    let thickness = exact_f64(paper_thickness_mm, meter, STAGE)?;
    let two = BigRational::from_integer(2.into());
    let half_thickness = meter.divide_rational(&thickness, &two, STAGE)?;
    if meter.compare_rational(&half_thickness, &BigRational::zero(), STAGE)? != Ordering::Greater {
        return Ok(None);
    }

    let faces = [
        calculate_face_bounds(
            exact,
            bound,
            0,
            &binary64_face_transforms[0],
            &half_thickness,
            meter,
        )?,
        calculate_face_bounds(
            exact,
            bound,
            1,
            &binary64_face_transforms[1],
            &half_thickness,
            meter,
        )?,
    ];
    let point_component_bound_mm = componentwise_max(
        &faces[0].point_component_bound_mm,
        &faces[1].point_component_bound_mm,
        meter,
    )?;
    let normal_component_bound = componentwise_max(
        &faces[0].normal_component_bound,
        &faces[1].normal_component_bound,
        meter,
    )?;
    let solid_component_bound_mm = solid_component_bounds(
        &point_component_bound_mm,
        &normal_component_bound,
        &half_thickness,
        meter,
    )?;
    let point_linf_bound_mm = linf_component_bound(&point_component_bound_mm, meter)?;
    let normal_linf_bound = linf_component_bound(&normal_component_bound, meter)?;
    let solid_linf_bound_mm = linf_component_bound(&solid_component_bound_mm, meter)?;

    Ok(Some(AxisAlignedEfBoundaryCapabilityV1 {
        prerequisite,
        exact,
        bound,
        paper_thickness_bits: paper_thickness_mm.to_bits(),
        left_face_index,
        right_face_index,
        hinge_index,
        binary64_face_transforms,
        faces,
        point_component_bound_mm,
        normal_component_bound,
        solid_component_bound_mm,
        point_linf_bound_mm,
        normal_linf_bound,
        solid_linf_bound_mm,
    }))
}

fn calculate_face_bounds(
    exact: &RationalCayleyTreePose<'_>,
    bound: BoundMaterialTreePose<'_>,
    face_index: usize,
    expected_transform_bits: &BoundBinary64FaceTransformBits,
    half_thickness: &BigRational,
    meter: &mut WorkMeter<'_>,
) -> Result<AxisAlignedFaceEfErrorBounds, CayleyError> {
    let face = exact
        .faces
        .get(face_index)
        .ok_or(CayleyError::InvariantFailure { stage: STAGE })?;
    if expected_transform_bits.face != face.face {
        return Err(CayleyError::InvariantFailure { stage: STAGE });
    }
    let transform = bound
        .pose()
        .face_transform(face.face)
        .ok_or(CayleyError::InvariantFailure { stage: STAGE })?;
    let lifted_transform = lift_binary64_transform(transform, meter)?;
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

    let mut point_component_bound_mm = std::array::from_fn(|_| BigRational::zero());
    for (vertex, exact_current) in &face.boundary {
        let source = bound
            .model()
            .vertex_position(*vertex)
            .ok_or(CayleyError::InvariantFailure { stage: STAGE })?;
        let source = exact_point_at_stage(point3_array(source), meter)?;
        let observed = apply_exact_transform(&lifted_transform, &source, meter)?;
        for (axis, current_bound) in point_component_bound_mm.iter_mut().enumerate() {
            let delta = meter.subtract_rational(
                &observed.coordinates[axis],
                &exact_current.coordinates[axis],
                STAGE,
            )?;
            let magnitude = meter.absolute_rational(&delta, STAGE)?;
            if meter.compare_rational(&magnitude, current_bound, STAGE)? == Ordering::Greater {
                *current_bound = magnitude;
            }
        }
    }

    let mut normal_component_bound = std::array::from_fn(|_| BigRational::zero());
    for (axis, current_bound) in normal_component_bound.iter_mut().enumerate() {
        let delta = meter.subtract_rational(
            &lifted_transform.rotation[axis][1],
            &face.transform.rotation[axis][1],
            STAGE,
        )?;
        *current_bound = meter.absolute_rational(&delta, STAGE)?;
    }
    let solid_component_bound_mm = solid_component_bounds(
        &point_component_bound_mm,
        &normal_component_bound,
        half_thickness,
        meter,
    )?;
    let point_linf_bound_mm = linf_component_bound(&point_component_bound_mm, meter)?;
    let normal_linf_bound = linf_component_bound(&normal_component_bound, meter)?;
    let solid_linf_bound_mm = linf_component_bound(&solid_component_bound_mm, meter)?;
    Ok(AxisAlignedFaceEfErrorBounds {
        face: face.face,
        point_component_bound_mm,
        normal_component_bound,
        solid_component_bound_mm,
        point_linf_bound_mm,
        normal_linf_bound,
        solid_linf_bound_mm,
    })
}

fn lift_binary64_transform(
    transform: RigidTransform,
    meter: &mut WorkMeter<'_>,
) -> Result<ExactRigidTransform, CayleyError> {
    let rows = transform.rotation_rows();
    let translation = transform.translation();
    Ok(ExactRigidTransform {
        rotation: try_array3(|row| {
            try_array3(|column| exact_f64(rows[row][column], meter, STAGE))
        })?,
        translation: ExactVector3 {
            coordinates: [
                exact_f64(translation.x(), meter, STAGE)?,
                exact_f64(translation.y(), meter, STAGE)?,
                exact_f64(translation.z(), meter, STAGE)?,
            ],
        },
    })
}

fn capture_binary64_transform_bits(
    bound: BoundMaterialTreePose<'_>,
    face: FaceId,
) -> Option<BoundBinary64FaceTransformBits> {
    let transform = bound.pose().face_transform(face)?;
    let rows = transform.rotation_rows();
    let translation = transform.translation();
    Some(BoundBinary64FaceTransformBits {
        face,
        rotation: std::array::from_fn(|row| {
            std::array::from_fn(|column| rows[row][column].to_bits())
        }),
        translation: [
            translation.x().to_bits(),
            translation.y().to_bits(),
            translation.z().to_bits(),
        ],
    })
}

fn componentwise_max(
    first: &[BigRational; 3],
    second: &[BigRational; 3],
    meter: &mut WorkMeter<'_>,
) -> Result<[BigRational; 3], CayleyError> {
    try_array3(|axis| {
        let selected =
            if meter.compare_rational(&first[axis], &second[axis], STAGE)? == Ordering::Less {
                &second[axis]
            } else {
                &first[axis]
            };
        meter.clone_rational(selected, STAGE)
    })
}

fn solid_component_bounds(
    point_component_bound_mm: &[BigRational; 3],
    normal_component_bound: &[BigRational; 3],
    half_thickness: &BigRational,
    meter: &mut WorkMeter<'_>,
) -> Result<[BigRational; 3], CayleyError> {
    try_array3(|axis| {
        let normal_offset =
            meter.multiply_rational(half_thickness, &normal_component_bound[axis], STAGE)?;
        meter.add_rational(&point_component_bound_mm[axis], &normal_offset, STAGE)
    })
}

fn linf_component_bound(
    component_bounds: &[BigRational; 3],
    meter: &mut WorkMeter<'_>,
) -> Result<BigRational, CayleyError> {
    let mut maximum = meter.clone_rational(&component_bounds[0], STAGE)?;
    for component in &component_bounds[1..] {
        if meter.compare_rational(component, &maximum, STAGE)? == Ordering::Greater {
            maximum = meter.clone_rational(component, STAGE)?;
        }
    }
    Ok(maximum)
}

fn charge_fixed_work(
    work: &mut AxisAlignedEfBoundaryWork,
    limits: &AxisAlignedEfBoundaryLimits,
) -> Result<(), CayleyError> {
    for (required, maximum, resource) in [
        (
            FACE_COUNT,
            limits.max_authenticated_faces,
            "ef_authenticated_faces",
        ),
        (
            HINGE_COUNT,
            limits.max_authenticated_hinges,
            "ef_authenticated_hinges",
        ),
        (
            BOUNDARY_OCCURRENCES,
            limits.max_boundary_occurrences,
            "ef_boundary_occurrences",
        ),
        (
            TRANSFORM_SCALAR_LIFTS,
            limits.max_transform_scalar_lifts,
            "ef_transform_scalar_lifts",
        ),
        (
            TRANSFORM_BIT_BINDINGS,
            limits.max_transform_bit_bindings,
            "ef_transform_bit_bindings",
        ),
        (
            SOURCE_COORDINATE_LIFTS,
            limits.max_source_coordinate_lifts,
            "ef_source_coordinate_lifts",
        ),
        (
            CURRENT_POINT_RECONSTRUCTIONS,
            limits.max_current_point_reconstructions,
            "ef_current_point_reconstructions",
        ),
        (
            POINT_COMPONENT_BOUNDS,
            limits.max_point_component_bounds,
            "ef_point_component_bounds",
        ),
        (
            NORMAL_COMPONENT_BOUNDS,
            limits.max_normal_component_bounds,
            "ef_normal_component_bounds",
        ),
        (
            SOLID_COMPONENT_BOUNDS,
            limits.max_solid_component_bounds,
            "ef_solid_component_bounds",
        ),
        (
            FACE_ERROR_RECORDS,
            limits.max_face_error_records,
            "ef_face_error_records",
        ),
        (
            THICKNESS_LIFTS,
            limits.max_thickness_lifts,
            "ef_thickness_lifts",
        ),
        (
            HALF_THICKNESS_DIVISIONS,
            limits.max_half_thickness_divisions,
            "ef_half_thickness_divisions",
        ),
    ] {
        if required > maximum {
            return Err(CayleyError::ResourceLimitExceeded {
                stage: STAGE,
                resource,
            });
        }
    }
    *work = AxisAlignedEfBoundaryWork {
        authenticated_faces: FACE_COUNT,
        authenticated_hinges: HINGE_COUNT,
        boundary_occurrences: BOUNDARY_OCCURRENCES,
        transform_scalar_lifts: TRANSFORM_SCALAR_LIFTS,
        transform_bit_bindings: TRANSFORM_BIT_BINDINGS,
        source_coordinate_lifts: SOURCE_COORDINATE_LIFTS,
        current_point_reconstructions: CURRENT_POINT_RECONSTRUCTIONS,
        point_component_bounds: POINT_COMPONENT_BOUNDS,
        normal_component_bounds: NORMAL_COMPONENT_BOUNDS,
        solid_component_bounds: SOLID_COMPONENT_BOUNDS,
        face_error_records: FACE_ERROR_RECORDS,
        thickness_lifts: THICKNESS_LIFTS,
        half_thickness_divisions: HALF_THICKNESS_DIVISIONS,
        exact: CayleyWork::default(),
    };
    Ok(())
}

/// Rebinds every authority and binary64 coefficient before a future private
/// consumer may inspect the stored axis-aligned bounds.
pub(super) fn revalidate_axis_aligned_ef_boundary_v1<'capability, 'prerequisite, 'exact, 'pose>(
    capability: &'capability AxisAlignedEfBoundaryCapabilityV1<'prerequisite, 'exact, 'pose>,
    prerequisite: &AuthenticatedSingleTriangularHingePrerequisitesV1<'_, '_>,
    exact: &RationalCayleyTreePose<'_>,
    bound: BoundMaterialTreePose<'_>,
    paper_thickness_mm: f64,
) -> Option<RevalidatedAxisAlignedEfBoundaryCapabilityV1<'capability, 'prerequisite, 'exact, 'pose>>
{
    if !positive_finite_binary64(paper_thickness_mm)
        || !std::ptr::eq(capability.prerequisite, prerequisite)
        || !std::ptr::eq(capability.exact, exact)
        || capability.paper_thickness_bits != paper_thickness_mm.to_bits()
        || capability.bound.model() != bound.model()
        || !capability.bound.pose().same_instance(bound.pose())
        || !exact.is_for(bound)
        || capability.left_face_index == capability.right_face_index
        || capability.left_face_index != prerequisite.left_face_index
        || capability.right_face_index != prerequisite.right_face_index
        || capability.hinge_index != prerequisite.hinge_index
        || revalidate_single_triangular_hinge_prerequisites_v1(
            prerequisite,
            exact,
            paper_thickness_mm,
        )
        .is_none()
        || capability.faces.len() != exact.faces.len()
    {
        return None;
    }
    for face_index in 0..FACE_COUNT {
        let exact_face = exact.faces.get(face_index)?;
        if capability.faces[face_index].face != exact_face.face
            || capability.binary64_face_transforms[face_index].face != exact_face.face
            || capture_binary64_transform_bits(bound, exact_face.face)?
                != capability.binary64_face_transforms[face_index]
        {
            return None;
        }
    }
    Some(RevalidatedAxisAlignedEfBoundaryCapabilityV1 { capability })
}
