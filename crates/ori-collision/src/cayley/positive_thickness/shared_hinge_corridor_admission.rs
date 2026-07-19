//! Sealed composition gate for one finite shared hinge.
//!
//! This private phase does not classify a production collision, extend a safe
//! set, persist a certificate, serialize a DTO, or mutate a project. It only
//! composes the already sealed exact-`E` and literal direct-`F` complete-prism
//! containment capabilities after proving that both used one bit-exactly
//! identical closed finite corridor.
//!
//! Separate containment in two merely similar corridors is deliberately
//! insufficient. In particular, a non-cardinal binary64 affine pose normally
//! differs from canonical exact `E`; such a boundary mismatch remains a
//! diagnostic and cannot mint this capability.

use std::cmp::Ordering;

use num_rational::BigRational;
use ori_domain::{EdgeId, FaceId, VertexId};
use ori_kinematics::BoundMaterialTreePose;

use super::direct_f_corridor::{
    DirectFFiniteHingeCorridorAnalysis, DirectFFiniteHingeCorridorBoundaryV1,
    DirectFFiniteHingeCorridorCapabilityV1, DirectFFiniteHingeCorridorResult,
    revalidate_direct_f_finite_hinge_corridor_v1,
};
use super::ef_boundary::{AxisAlignedEfBoundaryCapabilityV1, BoundBinary64FaceTransformBits};
use super::exact_e_corridor::{
    ExactEFiniteHingeCorridorAnalysis, ExactEFiniteHingeCorridorBoundaryV1,
    ExactEFiniteHingeCorridorCapabilityV1, ExactEFiniteHingeCorridorResult,
    ExactEFiniteHingeInteractionKind, revalidate_exact_e_finite_hinge_corridor_v1,
};
use super::*;

const FACE_COUNT: usize = 2;
const HINGE_COUNT: usize = 1;
const CORRIDOR_CAPABILITY_REVALIDATIONS: usize = 2;
const SEALED_PRIOR_WORK_BINDINGS: usize = 2;
const ROOT_BINDINGS: usize = 1;
const ANGLE_BINDINGS: usize = 1;
const FACE_IDENTITY_BINDINGS: usize = FACE_COUNT;
const HINGE_IDENTITY_BINDINGS: usize = HINGE_COUNT;
const INTERACTION_KIND_BINDINGS: usize = 1;
const FACE_TRANSFORM_BIT_BINDINGS: usize = FACE_COUNT * 12;
const HINGE_PARENT_TRANSFORM_BIT_BINDINGS: usize = 12;
const BOUNDARY_SCALAR_COMPARISONS: usize = 10;

fn shared_hinge_admission_hard_exact_limits() -> CayleyLimits {
    let exact = CayleyLimits::default();
    CayleyLimits {
        max_precision_rounds: 0,
        max_guard_bits: 0,
        max_candidate_bits: 0,
        max_machin_terms_per_series: 0,
        max_trig_terms_per_series: 0,
        max_sqrt_refinements: 0,
        max_interval_operations: BOUNDARY_SCALAR_COMPARISONS,
        max_shift_bits: exact.max_shift_bits,
        max_intermediate_bits: exact.max_intermediate_bits,
        max_gcd_fallback_calls: BOUNDARY_SCALAR_COMPARISONS,
        max_gcd_fallback_input_bits: exact.max_gcd_fallback_input_bits,
        max_rational_allocations: BOUNDARY_SCALAR_COMPARISONS * 4,
        max_rational_allocation_bits: exact.max_rational_allocation_bits,
        max_total_rational_allocation_bits: exact.max_total_rational_allocation_bits,
        max_output_bits: 0,
    }
}

/// Caller-non-expandable limits for the private composition gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SharedHingeCorridorAdmissionLimitsV1 {
    pub(super) max_authenticated_faces: usize,
    pub(super) max_authenticated_hinges: usize,
    pub(super) max_corridor_capability_revalidations: usize,
    pub(super) max_sealed_prior_work_bindings: usize,
    pub(super) max_root_bindings: usize,
    pub(super) max_angle_bindings: usize,
    pub(super) max_face_identity_bindings: usize,
    pub(super) max_hinge_identity_bindings: usize,
    pub(super) max_interaction_kind_bindings: usize,
    pub(super) max_face_transform_bit_bindings: usize,
    pub(super) max_hinge_parent_transform_bit_bindings: usize,
    pub(super) max_boundary_scalar_comparisons: usize,
    pub(super) exact: CayleyLimits,
}

impl Default for SharedHingeCorridorAdmissionLimitsV1 {
    fn default() -> Self {
        Self {
            max_authenticated_faces: FACE_COUNT,
            max_authenticated_hinges: HINGE_COUNT,
            max_corridor_capability_revalidations: CORRIDOR_CAPABILITY_REVALIDATIONS,
            max_sealed_prior_work_bindings: SEALED_PRIOR_WORK_BINDINGS,
            max_root_bindings: ROOT_BINDINGS,
            max_angle_bindings: ANGLE_BINDINGS,
            max_face_identity_bindings: FACE_IDENTITY_BINDINGS,
            max_hinge_identity_bindings: HINGE_IDENTITY_BINDINGS,
            max_interaction_kind_bindings: INTERACTION_KIND_BINDINGS,
            max_face_transform_bit_bindings: FACE_TRANSFORM_BIT_BINDINGS,
            max_hinge_parent_transform_bit_bindings: HINGE_PARENT_TRANSFORM_BIT_BINDINGS,
            max_boundary_scalar_comparisons: BOUNDARY_SCALAR_COMPARISONS,
            exact: shared_hinge_admission_hard_exact_limits(),
        }
    }
}

impl SharedHingeCorridorAdmissionLimitsV1 {
    fn projected(self) -> Self {
        let hard = Self::default();
        Self {
            max_authenticated_faces: self
                .max_authenticated_faces
                .min(hard.max_authenticated_faces),
            max_authenticated_hinges: self
                .max_authenticated_hinges
                .min(hard.max_authenticated_hinges),
            max_corridor_capability_revalidations: self
                .max_corridor_capability_revalidations
                .min(hard.max_corridor_capability_revalidations),
            max_sealed_prior_work_bindings: self
                .max_sealed_prior_work_bindings
                .min(hard.max_sealed_prior_work_bindings),
            max_root_bindings: self.max_root_bindings.min(hard.max_root_bindings),
            max_angle_bindings: self.max_angle_bindings.min(hard.max_angle_bindings),
            max_face_identity_bindings: self
                .max_face_identity_bindings
                .min(hard.max_face_identity_bindings),
            max_hinge_identity_bindings: self
                .max_hinge_identity_bindings
                .min(hard.max_hinge_identity_bindings),
            max_interaction_kind_bindings: self
                .max_interaction_kind_bindings
                .min(hard.max_interaction_kind_bindings),
            max_face_transform_bit_bindings: self
                .max_face_transform_bit_bindings
                .min(hard.max_face_transform_bit_bindings),
            max_hinge_parent_transform_bit_bindings: self
                .max_hinge_parent_transform_bit_bindings
                .min(hard.max_hinge_parent_transform_bit_bindings),
            max_boundary_scalar_comparisons: self
                .max_boundary_scalar_comparisons
                .min(hard.max_boundary_scalar_comparisons),
            exact: project_cayley_limits(self.exact, hard.exact),
        }
    }
}

/// Exact observed local work. Prior phase work is authenticated by borrow and
/// seal equality, not merged again.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct SharedHingeCorridorAdmissionWorkV1 {
    pub(super) authenticated_faces: usize,
    pub(super) authenticated_hinges: usize,
    pub(super) corridor_capability_revalidations: usize,
    pub(super) sealed_prior_work_bindings: usize,
    pub(super) root_bindings: usize,
    pub(super) angle_bindings: usize,
    pub(super) face_identity_bindings: usize,
    pub(super) hinge_identity_bindings: usize,
    pub(super) interaction_kind_bindings: usize,
    pub(super) face_transform_bit_bindings: usize,
    pub(super) hinge_parent_transform_bit_bindings: usize,
    pub(super) boundary_scalar_comparisons: usize,
    pub(super) exact: CayleyWork,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SharedHingeCorridorAdmissionErrorV1 {
    ResourceLimitExceeded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SharedHingeCorridorBoundaryComponentV1 {
    AxisStartX,
    AxisStartY,
    AxisStartZ,
    AxisX,
    AxisY,
    AxisZ,
    LengthSquared,
    HalfThickness,
    CosineHalfSquared,
    RadialLimitProduct,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SharedHingeCorridorBoundaryMismatchV1 {
    pub(super) first_component: SharedHingeCorridorBoundaryComponentV1,
    pub(super) mismatch_count: usize,
}

/// One angle record retained without turning value equality into authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BoundHingeAngleBitsV1 {
    edge: EdgeId,
    angle_degrees_bits: u64,
}

/// Sealed, borrow-bound proof that exact `E` and literal direct `F` complete
/// prism intersections were both proved inside one identical finite corridor.
///
/// This is intentionally not `Clone`, `Copy`, `Serialize`, or `Deserialize`.
/// It has no public constructor and grants no collision decision, safe-set,
/// persistence, DTO, continuous-collision, or project-mutation authority.
#[derive(Debug)]
pub(super) struct SharedHingeCorridorAdmissionCapabilityV1<
    'prerequisite,
    'ef,
    'exact_e_corridor,
    'direct_f_corridor,
    'exact,
    'pose,
> {
    prerequisite: &'prerequisite AuthenticatedSingleTriangularHingePrerequisitesV1<'exact, 'pose>,
    ef_boundary: &'ef AxisAlignedEfBoundaryCapabilityV1<'prerequisite, 'exact, 'pose>,
    exact_e_corridor:
        &'exact_e_corridor ExactEFiniteHingeCorridorCapabilityV1<'prerequisite, 'ef, 'exact, 'pose>,
    direct_f_corridor: &'direct_f_corridor DirectFFiniteHingeCorridorCapabilityV1<
        'prerequisite,
        'ef,
        'exact_e_corridor,
        'exact,
        'pose,
    >,
    exact: &'exact RationalCayleyTreePose<'pose>,
    bound: BoundMaterialTreePose<'pose>,
    paper_thickness_bits: u64,
    fixed_face: FaceId,
    face_ids: [FaceId; FACE_COUNT],
    hinge_edge: EdgeId,
    hinge_parent: FaceId,
    hinge_child: FaceId,
    hinge_endpoint_vertices: [VertexId; 2],
    hinge_angle: BoundHingeAngleBitsV1,
    interaction_kind: ExactEFiniteHingeInteractionKind,
    pub(super) binary64_face_transforms: [BoundBinary64FaceTransformBits; FACE_COUNT],
    pub(super) hinge_parent_transform: BoundBinary64FaceTransformBits,
    sealed_work: Option<SharedHingeCorridorAdmissionWorkV1>,
}

impl SharedHingeCorridorAdmissionCapabilityV1<'_, '_, '_, '_, '_, '_> {
    pub(super) fn sealed_work(&self) -> Option<&SharedHingeCorridorAdmissionWorkV1> {
        self.sealed_work.as_ref()
    }
}

#[derive(Debug)]
pub(super) struct RevalidatedSharedHingeCorridorAdmissionCapabilityV1<
    'capability,
    'prerequisite,
    'ef,
    'exact_e_corridor,
    'direct_f_corridor,
    'exact,
    'pose,
> {
    pub(super) capability: &'capability SharedHingeCorridorAdmissionCapabilityV1<
        'prerequisite,
        'ef,
        'exact_e_corridor,
        'direct_f_corridor,
        'exact,
        'pose,
    >,
}

#[derive(Debug)]
pub(super) enum SharedHingeCorridorAdmissionResultV1<
    'prerequisite,
    'ef,
    'exact_e_corridor,
    'direct_f_corridor,
    'exact,
    'pose,
> {
    Admitted(
        Box<
            SharedHingeCorridorAdmissionCapabilityV1<
                'prerequisite,
                'ef,
                'exact_e_corridor,
                'direct_f_corridor,
                'exact,
                'pose,
            >,
        >,
    ),
    BoundaryMismatch(SharedHingeCorridorBoundaryMismatchV1),
    LayerOffsetUnmodeled,
    Unresolved,
}

#[derive(Debug)]
pub(super) struct SharedHingeCorridorAdmissionAnalysisV1<
    'prerequisite,
    'ef,
    'exact_e_corridor,
    'direct_f_corridor,
    'exact,
    'pose,
> {
    pub(super) result: SharedHingeCorridorAdmissionResultV1<
        'prerequisite,
        'ef,
        'exact_e_corridor,
        'direct_f_corridor,
        'exact,
        'pose,
    >,
    pub(super) work: SharedHingeCorridorAdmissionWorkV1,
}

impl<'prerequisite, 'ef, 'exact_e_corridor, 'direct_f_corridor, 'exact, 'pose>
    SharedHingeCorridorAdmissionAnalysisV1<
        'prerequisite,
        'ef,
        'exact_e_corridor,
        'direct_f_corridor,
        'exact,
        'pose,
    >
{
    pub(super) fn authenticated_admission_capability_and_work(
        &self,
    ) -> Option<(
        &SharedHingeCorridorAdmissionCapabilityV1<
            'prerequisite,
            'ef,
            'exact_e_corridor,
            'direct_f_corridor,
            'exact,
            'pose,
        >,
        &SharedHingeCorridorAdmissionWorkV1,
    )> {
        let SharedHingeCorridorAdmissionResultV1::Admitted(capability) = &self.result else {
            return None;
        };
        let sealed_work = capability.sealed_work()?;
        if sealed_work != &self.work {
            return None;
        }
        Some((capability.as_ref(), sealed_work))
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn analyze_shared_hinge_corridor_admission_v1<
    'prerequisite,
    'ef,
    'exact_e_corridor,
    'direct_f_corridor,
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
    direct_f_analysis: &'direct_f_corridor DirectFFiniteHingeCorridorAnalysis<
        'prerequisite,
        'ef,
        'exact_e_corridor,
        'exact,
        'pose,
    >,
    exact: &'exact RationalCayleyTreePose<'pose>,
    bound: BoundMaterialTreePose<'pose>,
    paper_thickness_mm: f64,
    limits: SharedHingeCorridorAdmissionLimitsV1,
) -> Result<
    SharedHingeCorridorAdmissionAnalysisV1<
        'prerequisite,
        'ef,
        'exact_e_corridor,
        'direct_f_corridor,
        'exact,
        'pose,
    >,
    SharedHingeCorridorAdmissionErrorV1,
> {
    let limits = limits.projected();
    let mut work = SharedHingeCorridorAdmissionWorkV1::default();
    let mut meter = WorkMeter::new(&limits.exact);
    let result = calculate_shared_hinge_corridor_admission_v1(
        prerequisite_analysis,
        ef_boundary,
        exact_e_analysis,
        direct_f_analysis,
        exact,
        bound,
        paper_thickness_mm,
        &limits,
        &mut work,
        &mut meter,
    );
    work.exact = meter.work;
    match result {
        Ok(mut result) => {
            if let SharedHingeCorridorAdmissionResultV1::Admitted(capability) = &mut result {
                capability.sealed_work = Some(work.clone());
            }
            Ok(SharedHingeCorridorAdmissionAnalysisV1 { result, work })
        }
        Err(CayleyError::ResourceLimitExceeded { .. }) => {
            Err(SharedHingeCorridorAdmissionErrorV1::ResourceLimitExceeded)
        }
        Err(_) => Ok(SharedHingeCorridorAdmissionAnalysisV1 {
            result: SharedHingeCorridorAdmissionResultV1::Unresolved,
            work,
        }),
    }
}

#[allow(clippy::too_many_arguments)]
fn calculate_shared_hinge_corridor_admission_v1<
    'prerequisite,
    'ef,
    'exact_e_corridor,
    'direct_f_corridor,
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
    direct_f_analysis: &'direct_f_corridor DirectFFiniteHingeCorridorAnalysis<
        'prerequisite,
        'ef,
        'exact_e_corridor,
        'exact,
        'pose,
    >,
    exact: &'exact RationalCayleyTreePose<'pose>,
    bound: BoundMaterialTreePose<'pose>,
    paper_thickness_mm: f64,
    limits: &SharedHingeCorridorAdmissionLimitsV1,
    work: &mut SharedHingeCorridorAdmissionWorkV1,
    meter: &mut WorkMeter<'_>,
) -> Result<
    SharedHingeCorridorAdmissionResultV1<
        'prerequisite,
        'ef,
        'exact_e_corridor,
        'direct_f_corridor,
        'exact,
        'pose,
    >,
    CayleyError,
> {
    if matches!(
        &prerequisite_analysis.result,
        SingleTriangularHingePrerequisiteResult::LayerOffsetUnmodeled
    ) || matches!(
        &exact_e_analysis.result,
        ExactEFiniteHingeCorridorResult::LayerOffsetUnmodeled
    ) || matches!(
        &direct_f_analysis.result,
        DirectFFiniteHingeCorridorResult::LayerOffsetUnmodeled
    ) {
        return Ok(SharedHingeCorridorAdmissionResultV1::LayerOffsetUnmodeled);
    }
    let SingleTriangularHingePrerequisiteResult::Authenticated(prerequisite) =
        &prerequisite_analysis.result
    else {
        return Ok(SharedHingeCorridorAdmissionResultV1::Unresolved);
    };
    let Some(ef_boundary) = ef_boundary else {
        return Ok(SharedHingeCorridorAdmissionResultV1::Unresolved);
    };
    let Some((exact_e_corridor, _)) =
        exact_e_analysis.authenticated_contained_capability_and_work()
    else {
        return Ok(SharedHingeCorridorAdmissionResultV1::Unresolved);
    };
    let Some((direct_f_corridor, _)) =
        direct_f_analysis.authenticated_contained_capability_and_work()
    else {
        return Ok(SharedHingeCorridorAdmissionResultV1::Unresolved);
    };
    if !positive_finite_binary64(paper_thickness_mm)
        || exact.version != RATIONAL_CAYLEY_TREE_POSE_V1
        || !exact.is_for(bound)
        || bound.model() != exact.bound.model()
        || !bound.pose().same_instance(exact.bound.pose())
        || revalidate_exact_e_finite_hinge_corridor_v1(
            exact_e_corridor,
            prerequisite,
            ef_boundary,
            exact,
            bound,
            paper_thickness_mm,
        )
        .is_none()
        || revalidate_direct_f_finite_hinge_corridor_v1(
            direct_f_corridor,
            prerequisite,
            ef_boundary,
            exact_e_corridor,
            exact,
            bound,
            paper_thickness_mm,
        )
        .is_none()
    {
        return Ok(SharedHingeCorridorAdmissionResultV1::Unresolved);
    }

    let Some(fixed_face) = exact.fixed_face else {
        return Ok(SharedHingeCorridorAdmissionResultV1::Unresolved);
    };
    let Some(exact_hinge) = exact.hinges.first() else {
        return Ok(SharedHingeCorridorAdmissionResultV1::Unresolved);
    };
    let native_angles = bound.pose().hinge_angles();
    let Some(native_angle) = native_angles.first().copied() else {
        return Ok(SharedHingeCorridorAdmissionResultV1::Unresolved);
    };
    if exact.faces.len() != FACE_COUNT
        || exact.hinges.len() != HINGE_COUNT
        || native_angles.len() != HINGE_COUNT
        || bound.pose().fixed_face() != Some(fixed_face)
        || native_angle.edge() != exact_hinge.edge
        || native_angle.angle_degrees().to_bits() != exact_hinge.angle_magnitude_bits
    {
        return Ok(SharedHingeCorridorAdmissionResultV1::Unresolved);
    }

    charge_fixed_work(work, limits)?;
    let exact_boundary = exact_e_corridor
        .corridor_boundary()
        .ok_or(CayleyError::InvariantFailure { stage: STAGE })?;
    let direct_boundary = direct_f_corridor.corridor_boundary();
    if let Some(mismatch) =
        compare_corridor_boundaries(exact_boundary, direct_boundary, limits, work, meter)?
    {
        return Ok(SharedHingeCorridorAdmissionResultV1::BoundaryMismatch(
            mismatch,
        ));
    }

    let binary64_face_transforms = direct_f_corridor.binary64_face_transforms;
    let hinge_parent_transform = direct_f_corridor.hinge_parent_transform_bits();
    let face_ids = [exact.faces[0].face, exact.faces[1].face];
    Ok(SharedHingeCorridorAdmissionResultV1::Admitted(Box::new(
        SharedHingeCorridorAdmissionCapabilityV1 {
            prerequisite,
            ef_boundary,
            exact_e_corridor,
            direct_f_corridor,
            exact,
            bound,
            paper_thickness_bits: paper_thickness_mm.to_bits(),
            fixed_face,
            face_ids,
            hinge_edge: exact_hinge.edge,
            hinge_parent: exact_hinge.parent,
            hinge_child: exact_hinge.child,
            hinge_endpoint_vertices: exact_hinge.endpoint_vertices,
            hinge_angle: BoundHingeAngleBitsV1 {
                edge: native_angle.edge(),
                angle_degrees_bits: native_angle.angle_degrees().to_bits(),
            },
            interaction_kind: exact_e_corridor.interaction_kind(),
            binary64_face_transforms,
            hinge_parent_transform,
            sealed_work: None,
        },
    )))
}

fn charge_fixed_work(
    work: &mut SharedHingeCorridorAdmissionWorkV1,
    limits: &SharedHingeCorridorAdmissionLimitsV1,
) -> Result<(), CayleyError> {
    for (counter, required, maximum, resource) in [
        (
            &mut work.authenticated_faces,
            FACE_COUNT,
            limits.max_authenticated_faces,
            "shared_hinge_admission_faces",
        ),
        (
            &mut work.authenticated_hinges,
            HINGE_COUNT,
            limits.max_authenticated_hinges,
            "shared_hinge_admission_hinges",
        ),
        (
            &mut work.corridor_capability_revalidations,
            CORRIDOR_CAPABILITY_REVALIDATIONS,
            limits.max_corridor_capability_revalidations,
            "shared_hinge_admission_capability_revalidations",
        ),
        (
            &mut work.sealed_prior_work_bindings,
            SEALED_PRIOR_WORK_BINDINGS,
            limits.max_sealed_prior_work_bindings,
            "shared_hinge_admission_prior_work_bindings",
        ),
        (
            &mut work.root_bindings,
            ROOT_BINDINGS,
            limits.max_root_bindings,
            "shared_hinge_admission_root_bindings",
        ),
        (
            &mut work.angle_bindings,
            ANGLE_BINDINGS,
            limits.max_angle_bindings,
            "shared_hinge_admission_angle_bindings",
        ),
        (
            &mut work.face_identity_bindings,
            FACE_IDENTITY_BINDINGS,
            limits.max_face_identity_bindings,
            "shared_hinge_admission_face_identities",
        ),
        (
            &mut work.hinge_identity_bindings,
            HINGE_IDENTITY_BINDINGS,
            limits.max_hinge_identity_bindings,
            "shared_hinge_admission_hinge_identities",
        ),
        (
            &mut work.interaction_kind_bindings,
            INTERACTION_KIND_BINDINGS,
            limits.max_interaction_kind_bindings,
            "shared_hinge_admission_interaction_kind",
        ),
        (
            &mut work.face_transform_bit_bindings,
            FACE_TRANSFORM_BIT_BINDINGS,
            limits.max_face_transform_bit_bindings,
            "shared_hinge_admission_face_transform_bits",
        ),
        (
            &mut work.hinge_parent_transform_bit_bindings,
            HINGE_PARENT_TRANSFORM_BIT_BINDINGS,
            limits.max_hinge_parent_transform_bit_bindings,
            "shared_hinge_admission_hinge_parent_transform_bits",
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

fn compare_corridor_boundaries(
    exact: ExactEFiniteHingeCorridorBoundaryV1<'_>,
    direct: DirectFFiniteHingeCorridorBoundaryV1<'_>,
    limits: &SharedHingeCorridorAdmissionLimitsV1,
    work: &mut SharedHingeCorridorAdmissionWorkV1,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<SharedHingeCorridorBoundaryMismatchV1>, CayleyError> {
    let comparisons: [(
        SharedHingeCorridorBoundaryComponentV1,
        &BigRational,
        &BigRational,
    ); BOUNDARY_SCALAR_COMPARISONS] = [
        (
            SharedHingeCorridorBoundaryComponentV1::AxisStartX,
            &exact.axis_start.coordinates[0],
            &direct.axis_start.coordinates[0],
        ),
        (
            SharedHingeCorridorBoundaryComponentV1::AxisStartY,
            &exact.axis_start.coordinates[1],
            &direct.axis_start.coordinates[1],
        ),
        (
            SharedHingeCorridorBoundaryComponentV1::AxisStartZ,
            &exact.axis_start.coordinates[2],
            &direct.axis_start.coordinates[2],
        ),
        (
            SharedHingeCorridorBoundaryComponentV1::AxisX,
            &exact.axis.coordinates[0],
            &direct.axis.coordinates[0],
        ),
        (
            SharedHingeCorridorBoundaryComponentV1::AxisY,
            &exact.axis.coordinates[1],
            &direct.axis.coordinates[1],
        ),
        (
            SharedHingeCorridorBoundaryComponentV1::AxisZ,
            &exact.axis.coordinates[2],
            &direct.axis.coordinates[2],
        ),
        (
            SharedHingeCorridorBoundaryComponentV1::LengthSquared,
            exact.length_squared,
            direct.length_squared,
        ),
        (
            SharedHingeCorridorBoundaryComponentV1::HalfThickness,
            exact.half_thickness,
            direct.half_thickness,
        ),
        (
            SharedHingeCorridorBoundaryComponentV1::CosineHalfSquared,
            exact.cosine_half_squared,
            direct.cosine_half_squared,
        ),
        (
            SharedHingeCorridorBoundaryComponentV1::RadialLimitProduct,
            exact.radial_limit_product,
            direct.radial_limit_product,
        ),
    ];
    let mut first_component = None;
    let mut mismatch_count = 0_usize;
    for (component, exact_scalar, direct_scalar) in comparisons {
        charge_counter(
            &mut work.boundary_scalar_comparisons,
            limits.max_boundary_scalar_comparisons,
            "shared_hinge_admission_boundary_scalar_comparisons",
        )?;
        if meter.compare_rational(exact_scalar, direct_scalar, STAGE)? != Ordering::Equal {
            first_component.get_or_insert(component);
            mismatch_count =
                mismatch_count
                    .checked_add(1)
                    .ok_or(CayleyError::ResourceLimitExceeded {
                        stage: STAGE,
                        resource: "shared_hinge_admission_boundary_mismatches",
                    })?;
        }
    }
    if work.boundary_scalar_comparisons != BOUNDARY_SCALAR_COMPARISONS {
        return Err(CayleyError::InvariantFailure { stage: STAGE });
    }
    Ok(
        first_component.map(|first_component| SharedHingeCorridorBoundaryMismatchV1 {
            first_component,
            mismatch_count,
        }),
    )
}

fn corridor_boundaries_are_identical(
    exact: ExactEFiniteHingeCorridorBoundaryV1<'_>,
    direct: DirectFFiniteHingeCorridorBoundaryV1<'_>,
) -> bool {
    exact.axis_start == direct.axis_start
        && exact.axis == direct.axis
        && exact.length_squared == direct.length_squared
        && exact.half_thickness == direct.half_thickness
        && exact.cosine_half_squared == direct.cosine_half_squared
        && exact.radial_limit_product == direct.radial_limit_product
}

/// Revalidates all authority and scalar-boundary bindings. This remains a
/// private prerequisite for a later policy gate, not a collision decision.
#[allow(clippy::too_many_arguments)]
pub(super) fn revalidate_shared_hinge_corridor_admission_v1<
    'capability,
    'prerequisite,
    'ef,
    'exact_e_corridor,
    'direct_f_corridor,
    'exact,
    'pose,
>(
    capability: &'capability SharedHingeCorridorAdmissionCapabilityV1<
        'prerequisite,
        'ef,
        'exact_e_corridor,
        'direct_f_corridor,
        'exact,
        'pose,
    >,
    prerequisite: &AuthenticatedSingleTriangularHingePrerequisitesV1<'_, '_>,
    ef_boundary: &AxisAlignedEfBoundaryCapabilityV1<'_, '_, '_>,
    exact_e_corridor: &ExactEFiniteHingeCorridorCapabilityV1<'_, '_, '_, '_>,
    direct_f_corridor: &DirectFFiniteHingeCorridorCapabilityV1<'_, '_, '_, '_, '_>,
    exact: &RationalCayleyTreePose<'_>,
    bound: BoundMaterialTreePose<'_>,
    paper_thickness_mm: f64,
) -> Option<
    RevalidatedSharedHingeCorridorAdmissionCapabilityV1<
        'capability,
        'prerequisite,
        'ef,
        'exact_e_corridor,
        'direct_f_corridor,
        'exact,
        'pose,
    >,
> {
    let exact_boundary = exact_e_corridor.corridor_boundary()?;
    let direct_boundary = direct_f_corridor.corridor_boundary();
    let exact_hinge = exact.hinges.first()?;
    let native_angles = bound.pose().hinge_angles();
    let native_angle = native_angles.first().copied()?;
    if !positive_finite_binary64(paper_thickness_mm)
        || capability.sealed_work().is_none()
        || !std::ptr::eq(capability.prerequisite, prerequisite)
        || !std::ptr::eq(capability.ef_boundary, ef_boundary)
        || !std::ptr::eq(capability.exact_e_corridor, exact_e_corridor)
        || !std::ptr::eq(capability.direct_f_corridor, direct_f_corridor)
        || !std::ptr::eq(capability.exact, exact)
        || capability.paper_thickness_bits != paper_thickness_mm.to_bits()
        || capability.bound.model() != bound.model()
        || !capability.bound.pose().same_instance(bound.pose())
        || !exact.is_for(bound)
        || exact.faces.len() != FACE_COUNT
        || exact.hinges.len() != HINGE_COUNT
        || native_angles.len() != HINGE_COUNT
        || exact.fixed_face != Some(capability.fixed_face)
        || bound.pose().fixed_face() != Some(capability.fixed_face)
        || capability.face_ids != [exact.faces[0].face, exact.faces[1].face]
        || capability.hinge_edge != exact_hinge.edge
        || capability.hinge_parent != exact_hinge.parent
        || capability.hinge_child != exact_hinge.child
        || capability.hinge_endpoint_vertices != exact_hinge.endpoint_vertices
        || capability.hinge_angle.edge != native_angle.edge()
        || capability.hinge_angle.angle_degrees_bits != native_angle.angle_degrees().to_bits()
        || capability.hinge_angle.angle_degrees_bits != exact_hinge.angle_magnitude_bits
        || capability.interaction_kind != exact_e_corridor.interaction_kind()
        || capability.interaction_kind != direct_f_corridor.interaction_kind()
        || capability.binary64_face_transforms != direct_f_corridor.binary64_face_transforms
        || capability.hinge_parent_transform != direct_f_corridor.hinge_parent_transform_bits()
        || revalidate_exact_e_finite_hinge_corridor_v1(
            exact_e_corridor,
            prerequisite,
            ef_boundary,
            exact,
            bound,
            paper_thickness_mm,
        )
        .is_none()
        || revalidate_direct_f_finite_hinge_corridor_v1(
            direct_f_corridor,
            prerequisite,
            ef_boundary,
            exact_e_corridor,
            exact,
            bound,
            paper_thickness_mm,
        )
        .is_none()
        || !corridor_boundaries_are_identical(exact_boundary, direct_boundary)
    {
        return None;
    }
    Some(RevalidatedSharedHingeCorridorAdmissionCapabilityV1 { capability })
}
