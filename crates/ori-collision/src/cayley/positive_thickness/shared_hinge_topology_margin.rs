//! Private native-exact topology margin for one authenticated shared hinge.
//!
//! This phase gives a later composition gate a translation-independent,
//! exact-rational ceiling for the small world-coordinate difference between
//! canonical exact pose `E` and the native binary64 affine pose `F`.  It does
//! not classify a collision, authorize a shared-hinge contact, extend a safe
//! set, persist a certificate, serialize a DTO, or mutate a project.
//!
//! The version-fixed relative factor is `f64::EPSILON * 256 = 2^-44`.  The
//! length scale is derived only from authenticated source material and exact
//! `E` solid geometry; absolute world-coordinate magnitude and caller input
//! cannot enlarge it.  Issuance additionally reconstructs and fully scans:
//!
//! - both occurrences of both shared endpoints;
//! - the finite axial coordinate of every mid-surface and solid vertex;
//! - each face's canonical inward material half-space;
//! - each face's local-`+Y` material normal and both thickness offsets; and
//! - all ten scalars of the exact-`E` and native-`F` finite-corridor boundary.
//!
//! The resulting capability is a sealed prerequisite only.  In particular,
//! a boundary delta below the stored ceiling is not itself collision or
//! contact authority.

use std::cmp::Ordering;

use num_rational::BigRational;
use num_traits::{One, Signed, Zero};
use ori_domain::{EdgeId, FaceId, VertexId};
use ori_kinematics::{BoundMaterialTreePose, RigidTransform};

use super::ef_boundary::{
    AxisAlignedEfBoundaryCapabilityV1, AxisAlignedFaceEfErrorBounds,
    BoundBinary64FaceTransformBits, revalidate_axis_aligned_ef_boundary_v1,
};
use super::*;

const FACE_COUNT: usize = 2;
const HINGE_COUNT: usize = 1;
const VERTICES_PER_FACE: usize = 3;
const SOLID_VERTICES_PER_FACE: usize = 6;
const CORRIDOR_COMPONENT_COUNT: usize = 10;
const RELATIVE_MARGIN_ULP_FACTOR: i64 = 256;

const PREREQUISITE_REVALIDATIONS: usize = 1;
const EF_BOUNDARY_REVALIDATIONS: usize = 1;
const ROOT_BINDINGS: usize = 1;
const ANGLE_BINDINGS: usize = 1;
const FACE_IDENTITY_BINDINGS: usize = FACE_COUNT;
const HINGE_IDENTITY_BINDINGS: usize = 5;
const ENDPOINT_IDENTITY_BINDINGS: usize = 2;
const SOURCE_BOUNDARY_OCCURRENCES: usize = FACE_COUNT * VERTICES_PER_FACE;
const SOURCE_COORDINATE_LIFTS: usize = (SOURCE_BOUNDARY_OCCURRENCES + 2) * 3;
const TRANSFORM_SCALAR_LIFTS: usize = FACE_COUNT * 12;
const FACE_TRANSFORM_BIT_BINDINGS: usize = FACE_COUNT * 12;
const HINGE_PARENT_TRANSFORM_BIT_BINDINGS: usize = 12;
const MID_SURFACE_RECONSTRUCTIONS: usize = FACE_COUNT * VERTICES_PER_FACE * 2;
const NORMAL_RECONSTRUCTIONS: usize = FACE_COUNT * 2;
const SOLID_VERTEX_CONSTRUCTIONS: usize = FACE_COUNT * SOLID_VERTICES_PER_FACE * 2;
const TOPOLOGY_COORDINATE_TESTS: usize =
    FACE_COUNT * 2 * (VERTICES_PER_FACE + SOLID_VERTICES_PER_FACE) * 3;
const SHARED_ENDPOINT_COMPONENT_TESTS: usize = 2 * 3;
const POINT_COMPONENT_ERROR_TESTS: usize = SOURCE_BOUNDARY_OCCURRENCES * 3;
const NORMAL_COMPONENT_ERROR_TESTS: usize = FACE_COUNT * 3;
const SOLID_COMPONENT_ERROR_TESTS: usize = FACE_COUNT * SOLID_VERTICES_PER_FACE * 3;
const EF_SCALAR_REAUTHENTICATIONS: usize = FACE_COUNT * 12 + 12;
const LOCAL_SCALE_COMPONENT_SCANS: usize =
    SOURCE_BOUNDARY_OCCURRENCES * 3 + FACE_COUNT * SOLID_VERTICES_PER_FACE * 3;
const CORRIDOR_COMPONENT_SCANS: usize = CORRIDOR_COMPONENT_COUNT;

fn topology_margin_exact_hard_limits() -> CayleyLimits {
    let exact = CayleyLimits::default();
    CayleyLimits {
        max_precision_rounds: 0,
        max_guard_bits: 0,
        max_candidate_bits: 0,
        max_machin_terms_per_series: 0,
        max_trig_terms_per_series: 0,
        max_sqrt_refinements: 0,
        max_interval_operations: 131_072,
        max_shift_bits: exact.max_shift_bits,
        max_intermediate_bits: exact.max_intermediate_bits,
        max_gcd_fallback_calls: 16_384,
        max_gcd_fallback_input_bits: exact.max_gcd_fallback_input_bits,
        max_rational_allocations: 131_072,
        max_rational_allocation_bits: exact.max_rational_allocation_bits,
        max_total_rational_allocation_bits: 536_870_912,
        max_output_bits: 0,
    }
}

/// Caller-non-expandable limits for the private topology-margin issuer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SharedHingeNativeExactTopologyMarginLimitsV1 {
    pub(super) max_authenticated_faces: usize,
    pub(super) max_authenticated_hinges: usize,
    pub(super) max_prerequisite_revalidations: usize,
    pub(super) max_ef_boundary_revalidations: usize,
    pub(super) max_root_bindings: usize,
    pub(super) max_angle_bindings: usize,
    pub(super) max_face_identity_bindings: usize,
    pub(super) max_hinge_identity_bindings: usize,
    pub(super) max_endpoint_identity_bindings: usize,
    pub(super) max_source_boundary_occurrences: usize,
    pub(super) max_source_coordinate_lifts: usize,
    pub(super) max_transform_scalar_lifts: usize,
    pub(super) max_face_transform_bit_bindings: usize,
    pub(super) max_hinge_parent_transform_bit_bindings: usize,
    pub(super) max_mid_surface_reconstructions: usize,
    pub(super) max_normal_reconstructions: usize,
    pub(super) max_solid_vertex_constructions: usize,
    pub(super) max_topology_coordinate_tests: usize,
    pub(super) max_shared_endpoint_component_tests: usize,
    pub(super) max_point_component_error_tests: usize,
    pub(super) max_normal_component_error_tests: usize,
    pub(super) max_solid_component_error_tests: usize,
    pub(super) max_ef_scalar_reauthentications: usize,
    pub(super) max_local_scale_component_scans: usize,
    pub(super) max_corridor_component_scans: usize,
    pub(super) exact: CayleyLimits,
}

impl Default for SharedHingeNativeExactTopologyMarginLimitsV1 {
    fn default() -> Self {
        Self {
            max_authenticated_faces: FACE_COUNT,
            max_authenticated_hinges: HINGE_COUNT,
            max_prerequisite_revalidations: PREREQUISITE_REVALIDATIONS,
            max_ef_boundary_revalidations: EF_BOUNDARY_REVALIDATIONS,
            max_root_bindings: ROOT_BINDINGS,
            max_angle_bindings: ANGLE_BINDINGS,
            max_face_identity_bindings: FACE_IDENTITY_BINDINGS,
            max_hinge_identity_bindings: HINGE_IDENTITY_BINDINGS,
            max_endpoint_identity_bindings: ENDPOINT_IDENTITY_BINDINGS,
            max_source_boundary_occurrences: SOURCE_BOUNDARY_OCCURRENCES,
            max_source_coordinate_lifts: SOURCE_COORDINATE_LIFTS,
            max_transform_scalar_lifts: TRANSFORM_SCALAR_LIFTS,
            max_face_transform_bit_bindings: FACE_TRANSFORM_BIT_BINDINGS,
            max_hinge_parent_transform_bit_bindings: HINGE_PARENT_TRANSFORM_BIT_BINDINGS,
            max_mid_surface_reconstructions: MID_SURFACE_RECONSTRUCTIONS,
            max_normal_reconstructions: NORMAL_RECONSTRUCTIONS,
            max_solid_vertex_constructions: SOLID_VERTEX_CONSTRUCTIONS,
            max_topology_coordinate_tests: TOPOLOGY_COORDINATE_TESTS,
            max_shared_endpoint_component_tests: SHARED_ENDPOINT_COMPONENT_TESTS,
            max_point_component_error_tests: POINT_COMPONENT_ERROR_TESTS,
            max_normal_component_error_tests: NORMAL_COMPONENT_ERROR_TESTS,
            max_solid_component_error_tests: SOLID_COMPONENT_ERROR_TESTS,
            max_ef_scalar_reauthentications: EF_SCALAR_REAUTHENTICATIONS,
            max_local_scale_component_scans: LOCAL_SCALE_COMPONENT_SCANS,
            max_corridor_component_scans: CORRIDOR_COMPONENT_SCANS,
            exact: topology_margin_exact_hard_limits(),
        }
    }
}

impl SharedHingeNativeExactTopologyMarginLimitsV1 {
    fn projected(self) -> Self {
        let hard = Self::default();
        Self {
            max_authenticated_faces: self
                .max_authenticated_faces
                .min(hard.max_authenticated_faces),
            max_authenticated_hinges: self
                .max_authenticated_hinges
                .min(hard.max_authenticated_hinges),
            max_prerequisite_revalidations: self
                .max_prerequisite_revalidations
                .min(hard.max_prerequisite_revalidations),
            max_ef_boundary_revalidations: self
                .max_ef_boundary_revalidations
                .min(hard.max_ef_boundary_revalidations),
            max_root_bindings: self.max_root_bindings.min(hard.max_root_bindings),
            max_angle_bindings: self.max_angle_bindings.min(hard.max_angle_bindings),
            max_face_identity_bindings: self
                .max_face_identity_bindings
                .min(hard.max_face_identity_bindings),
            max_hinge_identity_bindings: self
                .max_hinge_identity_bindings
                .min(hard.max_hinge_identity_bindings),
            max_endpoint_identity_bindings: self
                .max_endpoint_identity_bindings
                .min(hard.max_endpoint_identity_bindings),
            max_source_boundary_occurrences: self
                .max_source_boundary_occurrences
                .min(hard.max_source_boundary_occurrences),
            max_source_coordinate_lifts: self
                .max_source_coordinate_lifts
                .min(hard.max_source_coordinate_lifts),
            max_transform_scalar_lifts: self
                .max_transform_scalar_lifts
                .min(hard.max_transform_scalar_lifts),
            max_face_transform_bit_bindings: self
                .max_face_transform_bit_bindings
                .min(hard.max_face_transform_bit_bindings),
            max_hinge_parent_transform_bit_bindings: self
                .max_hinge_parent_transform_bit_bindings
                .min(hard.max_hinge_parent_transform_bit_bindings),
            max_mid_surface_reconstructions: self
                .max_mid_surface_reconstructions
                .min(hard.max_mid_surface_reconstructions),
            max_normal_reconstructions: self
                .max_normal_reconstructions
                .min(hard.max_normal_reconstructions),
            max_solid_vertex_constructions: self
                .max_solid_vertex_constructions
                .min(hard.max_solid_vertex_constructions),
            max_topology_coordinate_tests: self
                .max_topology_coordinate_tests
                .min(hard.max_topology_coordinate_tests),
            max_shared_endpoint_component_tests: self
                .max_shared_endpoint_component_tests
                .min(hard.max_shared_endpoint_component_tests),
            max_point_component_error_tests: self
                .max_point_component_error_tests
                .min(hard.max_point_component_error_tests),
            max_normal_component_error_tests: self
                .max_normal_component_error_tests
                .min(hard.max_normal_component_error_tests),
            max_solid_component_error_tests: self
                .max_solid_component_error_tests
                .min(hard.max_solid_component_error_tests),
            max_ef_scalar_reauthentications: self
                .max_ef_scalar_reauthentications
                .min(hard.max_ef_scalar_reauthentications),
            max_local_scale_component_scans: self
                .max_local_scale_component_scans
                .min(hard.max_local_scale_component_scans),
            max_corridor_component_scans: self
                .max_corridor_component_scans
                .min(hard.max_corridor_component_scans),
            exact: project_cayley_limits(self.exact, hard.exact),
        }
    }
}

/// Exact observed local work. Upstream work is authenticated by token
/// identity and is deliberately not merged a second time.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct SharedHingeNativeExactTopologyMarginWorkV1 {
    pub(super) authenticated_faces: usize,
    pub(super) authenticated_hinges: usize,
    pub(super) prerequisite_revalidations: usize,
    pub(super) ef_boundary_revalidations: usize,
    pub(super) root_bindings: usize,
    pub(super) angle_bindings: usize,
    pub(super) face_identity_bindings: usize,
    pub(super) hinge_identity_bindings: usize,
    pub(super) endpoint_identity_bindings: usize,
    pub(super) source_boundary_occurrences: usize,
    pub(super) source_coordinate_lifts: usize,
    pub(super) transform_scalar_lifts: usize,
    pub(super) face_transform_bit_bindings: usize,
    pub(super) hinge_parent_transform_bit_bindings: usize,
    pub(super) mid_surface_reconstructions: usize,
    pub(super) normal_reconstructions: usize,
    pub(super) solid_vertex_constructions: usize,
    pub(super) topology_coordinate_tests: usize,
    pub(super) shared_endpoint_component_tests: usize,
    pub(super) point_component_error_tests: usize,
    pub(super) normal_component_error_tests: usize,
    pub(super) solid_component_error_tests: usize,
    pub(super) ef_scalar_reauthentications: usize,
    pub(super) local_scale_component_scans: usize,
    pub(super) corridor_component_scans: usize,
    pub(super) exact: CayleyWork,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SharedHingeNativeExactTopologyMarginErrorV1 {
    ResourceLimitExceeded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SharedHingeTopologyMarginRelationV1 {
    SharedEndpoint,
    FiniteAxis,
    MaterialHalfSpace,
    MaterialNormalOrSolid,
    PointError,
    NormalError,
    SolidError,
    CorridorBoundary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SharedHingeTopologyMarginViolationV1 {
    pub(super) first_relation: SharedHingeTopologyMarginRelationV1,
    pub(super) violation_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BoundHingeAngleBitsV1 {
    edge: EdgeId,
    angle_degrees_bits: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BoundFaceTopologyIdentityV1 {
    face: FaceId,
    boundary_vertices: [VertexId; VERTICES_PER_FACE],
    boundary_edges: [EdgeId; VERTICES_PER_FACE],
    hinge_occurrence: usize,
    opposite_vertex: VertexId,
}

/// Scalar geometry retained by the sealed capability.  Every rational is
/// duplicated into a private issuance seal, so in-module adversarial mutation
/// cannot turn one previously scanned topology into another.
#[derive(Debug, Clone, PartialEq, Eq)]
struct SharedHingeNativeExactTopologyMarginGeometryV1 {
    source_endpoints: [ExactPoint3; 2],
    exact_endpoints: [ExactPoint3; 2],
    direct_endpoint_occurrences: [[ExactPoint3; 2]; FACE_COUNT],
    source_halfspace_support: [BigRational; FACE_COUNT],
    exact_frame_determinants: [BigRational; FACE_COUNT],
    direct_frame_determinants: [BigRational; FACE_COUNT],
    exact_face_normals: [ExactVector3; FACE_COUNT],
    direct_face_normals: [ExactVector3; FACE_COUNT],
    exact_solid_bounds: [[ExactPoint3; 2]; FACE_COUNT],
    direct_solid_bounds: [[ExactPoint3; 2]; FACE_COUNT],
    exact_axis_start: ExactPoint3,
    exact_axis: ExactVector3,
    exact_length_squared: BigRational,
    direct_axis_start: ExactPoint3,
    direct_axis: ExactVector3,
    direct_length_squared: BigRational,
    half_thickness_mm: BigRational,
    relative_margin: BigRational,
    local_scale_mm: BigRational,
    point_margin_mm: BigRational,
    normal_margin: BigRational,
    solid_margin_mm: BigRational,
    observed_shared_endpoint_component_error_mm: [BigRational; 3],
    observed_point_component_error_mm: [BigRational; 3],
    observed_normal_component_error: [BigRational; 3],
    observed_solid_component_error_mm: [BigRational; 3],
    corridor_component_error: [BigRational; CORRIDOR_COMPONENT_COUNT],
    corridor_component_margin: [BigRational; CORRIDOR_COMPONENT_COUNT],
    source_axially_bounded: [bool; FACE_COUNT],
    exact_axially_bounded: [bool; FACE_COUNT],
    direct_axially_bounded: [bool; FACE_COUNT],
}

/// Non-cloneable, non-serializable native-exact topology-margin capability.
///
/// It grants no production classification or safety authority. A future
/// private consumer must borrow and revalidate this exact object.
#[derive(Debug)]
pub(super) struct SharedHingeNativeExactTopologyMarginCapabilityV1<
    'prerequisite,
    'ef,
    'exact,
    'pose,
> {
    prerequisite: &'prerequisite AuthenticatedSingleTriangularHingePrerequisitesV1<'exact, 'pose>,
    ef_boundary: &'ef AxisAlignedEfBoundaryCapabilityV1<'prerequisite, 'exact, 'pose>,
    exact: &'exact RationalCayleyTreePose<'pose>,
    bound: BoundMaterialTreePose<'pose>,
    paper_thickness_bits: u64,
    fixed_face: FaceId,
    face_topology: [BoundFaceTopologyIdentityV1; FACE_COUNT],
    hinge_edge: EdgeId,
    hinge_parent: FaceId,
    hinge_child: FaceId,
    hinge_endpoint_vertices: [VertexId; 2],
    hinge_angle: BoundHingeAngleBitsV1,
    binary64_face_transforms: [BoundBinary64FaceTransformBits; FACE_COUNT],
    hinge_parent_transform: BoundBinary64FaceTransformBits,
    geometry: SharedHingeNativeExactTopologyMarginGeometryV1,
    sealed_geometry: Option<SharedHingeNativeExactTopologyMarginGeometryV1>,
    sealed_work: Option<SharedHingeNativeExactTopologyMarginWorkV1>,
}

impl SharedHingeNativeExactTopologyMarginCapabilityV1<'_, '_, '_, '_> {
    pub(super) fn sealed_work(&self) -> Option<&SharedHingeNativeExactTopologyMarginWorkV1> {
        self.sealed_work.as_ref()
    }

    pub(super) fn corridor_component_margin(&self) -> &[BigRational; CORRIDOR_COMPONENT_COUNT] {
        &self.geometry.corridor_component_margin
    }

    pub(super) fn corridor_component_error(&self) -> &[BigRational; CORRIDOR_COMPONENT_COUNT] {
        &self.geometry.corridor_component_error
    }

    pub(super) fn relative_margin(&self) -> &BigRational {
        &self.geometry.relative_margin
    }

    pub(super) fn point_margin_mm(&self) -> &BigRational {
        &self.geometry.point_margin_mm
    }

    #[cfg(test)]
    pub(super) fn scalar_count_for_test(&self) -> usize {
        scalar_count(&self.geometry)
    }

    #[cfg(test)]
    pub(super) fn adjust_scalar_for_test(&mut self, target: usize, delta: i64) {
        let mut current = 0_usize;
        let mut found = false;
        visit_geometry_scalars_mut(&mut self.geometry, &mut |value| {
            if current == target {
                *value += BigRational::from_integer(delta.into());
                found = true;
            }
            current += 1;
        });
        assert!(found, "topology-margin scalar {target} must exist");
    }

    #[cfg(test)]
    pub(super) fn flip_face_transform_bit_for_test(&mut self, face: usize, coefficient: usize) {
        if coefficient < 9 {
            self.binary64_face_transforms[face].rotation[coefficient / 3][coefficient % 3] ^= 1;
        } else {
            self.binary64_face_transforms[face].translation[coefficient - 9] ^= 1;
        }
    }

    #[cfg(test)]
    pub(super) fn flip_hinge_parent_transform_bit_for_test(&mut self, coefficient: usize) {
        if coefficient < 9 {
            self.hinge_parent_transform.rotation[coefficient / 3][coefficient % 3] ^= 1;
        } else {
            self.hinge_parent_transform.translation[coefficient - 9] ^= 1;
        }
    }
}

#[derive(Debug)]
pub(super) struct RevalidatedSharedHingeNativeExactTopologyMarginCapabilityV1<
    'capability,
    'prerequisite,
    'ef,
    'exact,
    'pose,
> {
    pub(super) capability: &'capability SharedHingeNativeExactTopologyMarginCapabilityV1<
        'prerequisite,
        'ef,
        'exact,
        'pose,
    >,
}

#[derive(Debug)]
pub(super) enum SharedHingeNativeExactTopologyMarginResultV1<'prerequisite, 'ef, 'exact, 'pose> {
    Measured(
        Box<SharedHingeNativeExactTopologyMarginCapabilityV1<'prerequisite, 'ef, 'exact, 'pose>>,
    ),
    OutsideMargin(SharedHingeTopologyMarginViolationV1),
    LayerOffsetUnmodeled,
    Unresolved,
}

#[derive(Debug)]
pub(super) struct SharedHingeNativeExactTopologyMarginAnalysisV1<'prerequisite, 'ef, 'exact, 'pose>
{
    pub(super) result:
        SharedHingeNativeExactTopologyMarginResultV1<'prerequisite, 'ef, 'exact, 'pose>,
    pub(super) work: SharedHingeNativeExactTopologyMarginWorkV1,
}

impl<'prerequisite, 'ef, 'exact, 'pose>
    SharedHingeNativeExactTopologyMarginAnalysisV1<'prerequisite, 'ef, 'exact, 'pose>
{
    pub(super) fn authenticated_capability_and_work(
        &self,
    ) -> Option<(
        &SharedHingeNativeExactTopologyMarginCapabilityV1<'prerequisite, 'ef, 'exact, 'pose>,
        &SharedHingeNativeExactTopologyMarginWorkV1,
    )> {
        let SharedHingeNativeExactTopologyMarginResultV1::Measured(capability) = &self.result
        else {
            return None;
        };
        let sealed_work = capability.sealed_work()?;
        if sealed_work != &self.work
            || capability.sealed_geometry.as_ref() != Some(&capability.geometry)
        {
            return None;
        }
        Some((capability.as_ref(), sealed_work))
    }
}

#[derive(Debug)]
struct ReconstructedFaceTopology {
    identity: BoundFaceTopologyIdentityV1,
    source_vertices: [ExactPoint3; VERTICES_PER_FACE],
    exact_mid_surface: [ExactPoint3; VERTICES_PER_FACE],
    direct_mid_surface: [ExactPoint3; VERTICES_PER_FACE],
    exact_normal: ExactVector3,
    direct_normal: ExactVector3,
    exact_solid: [ExactPoint3; SOLID_VERTICES_PER_FACE],
    direct_solid: [ExactPoint3; SOLID_VERTICES_PER_FACE],
    source_halfspace_support: BigRational,
    source_axially_bounded: bool,
    exact_frame: ExactMaterialFrame,
    direct_frame: ExactMaterialFrame,
}

#[derive(Debug)]
struct ExactMaterialFrame {
    start: ExactPoint3,
    axis: ExactVector3,
    inward: ExactVector3,
    normal: ExactVector3,
    determinant: BigRational,
}

#[derive(Debug, Default)]
struct TopologyScanSummary {
    axially_bounded: bool,
    halfspace_violation_count: usize,
    normal_solid_violation_count: usize,
}

#[derive(Debug, Default)]
struct ViolationAccumulator {
    first_relation: Option<SharedHingeTopologyMarginRelationV1>,
    count: usize,
}

type ComponentErrorTriplet = ([BigRational; 3], [BigRational; 3], [BigRational; 3]);

impl ViolationAccumulator {
    fn record(&mut self, relation: SharedHingeTopologyMarginRelationV1) -> Result<(), CayleyError> {
        self.first_relation.get_or_insert(relation);
        self.count = self
            .count
            .checked_add(1)
            .ok_or(CayleyError::ResourceLimitExceeded {
                stage: STAGE,
                resource: "topology_margin_violations",
            })?;
        Ok(())
    }

    fn finish(self) -> Option<SharedHingeTopologyMarginViolationV1> {
        self.first_relation
            .map(|first_relation| SharedHingeTopologyMarginViolationV1 {
                first_relation,
                violation_count: self.count,
            })
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn analyze_shared_hinge_native_exact_topology_margin_v1<
    'prerequisite,
    'ef,
    'exact,
    'pose,
>(
    prerequisite_analysis: &'prerequisite SingleTriangularHingePrerequisiteAnalysis<'exact, 'pose>,
    ef_boundary: Option<&'ef AxisAlignedEfBoundaryCapabilityV1<'prerequisite, 'exact, 'pose>>,
    exact: &'exact RationalCayleyTreePose<'pose>,
    bound: BoundMaterialTreePose<'pose>,
    paper_thickness_mm: f64,
    limits: SharedHingeNativeExactTopologyMarginLimitsV1,
) -> Result<
    SharedHingeNativeExactTopologyMarginAnalysisV1<'prerequisite, 'ef, 'exact, 'pose>,
    SharedHingeNativeExactTopologyMarginErrorV1,
> {
    let limits = limits.projected();
    let mut work = SharedHingeNativeExactTopologyMarginWorkV1::default();
    let mut meter = WorkMeter::new(&limits.exact);
    let result = calculate_shared_hinge_native_exact_topology_margin_v1(
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
        Ok(mut result) => {
            if let SharedHingeNativeExactTopologyMarginResultV1::Measured(capability) = &mut result
            {
                capability.sealed_work = Some(work.clone());
            }
            Ok(SharedHingeNativeExactTopologyMarginAnalysisV1 { result, work })
        }
        Err(CayleyError::ResourceLimitExceeded { .. }) => {
            Err(SharedHingeNativeExactTopologyMarginErrorV1::ResourceLimitExceeded)
        }
        Err(_) => Ok(SharedHingeNativeExactTopologyMarginAnalysisV1 {
            result: SharedHingeNativeExactTopologyMarginResultV1::Unresolved,
            work,
        }),
    }
}

#[allow(clippy::too_many_arguments)]
fn calculate_shared_hinge_native_exact_topology_margin_v1<'prerequisite, 'ef, 'exact, 'pose>(
    prerequisite_analysis: &'prerequisite SingleTriangularHingePrerequisiteAnalysis<'exact, 'pose>,
    ef_boundary: Option<&'ef AxisAlignedEfBoundaryCapabilityV1<'prerequisite, 'exact, 'pose>>,
    exact: &'exact RationalCayleyTreePose<'pose>,
    bound: BoundMaterialTreePose<'pose>,
    paper_thickness_mm: f64,
    limits: &SharedHingeNativeExactTopologyMarginLimitsV1,
    work: &mut SharedHingeNativeExactTopologyMarginWorkV1,
    meter: &mut WorkMeter<'_>,
) -> Result<
    SharedHingeNativeExactTopologyMarginResultV1<'prerequisite, 'ef, 'exact, 'pose>,
    CayleyError,
> {
    if matches!(
        prerequisite_analysis.result,
        SingleTriangularHingePrerequisiteResult::LayerOffsetUnmodeled
    ) {
        return Ok(SharedHingeNativeExactTopologyMarginResultV1::LayerOffsetUnmodeled);
    }
    let SingleTriangularHingePrerequisiteResult::Authenticated(prerequisite) =
        &prerequisite_analysis.result
    else {
        return Ok(SharedHingeNativeExactTopologyMarginResultV1::Unresolved);
    };
    let Some(ef_boundary) = ef_boundary else {
        return Ok(SharedHingeNativeExactTopologyMarginResultV1::Unresolved);
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
        return Ok(SharedHingeNativeExactTopologyMarginResultV1::Unresolved);
    }

    let Some(fixed_face) = exact.fixed_face else {
        return Ok(SharedHingeNativeExactTopologyMarginResultV1::Unresolved);
    };
    let Some(exact_hinge) = exact.hinges.first() else {
        return Ok(SharedHingeNativeExactTopologyMarginResultV1::Unresolved);
    };
    let Some(native_hinge) = bound.model().hinges().first() else {
        return Ok(SharedHingeNativeExactTopologyMarginResultV1::Unresolved);
    };
    let native_angles = bound.pose().hinge_angles();
    let Some(native_angle) = native_angles.first().copied() else {
        return Ok(SharedHingeNativeExactTopologyMarginResultV1::Unresolved);
    };
    if exact.faces.len() != FACE_COUNT
        || exact.hinges.len() != HINGE_COUNT
        || bound.model().face_ids().len() != FACE_COUNT
        || native_angles.len() != HINGE_COUNT
        || bound.pose().fixed_face() != Some(fixed_face)
        || native_hinge.edge() != exact_hinge.edge
        || native_angle.edge() != exact_hinge.edge
        || native_angle.angle_degrees().to_bits() != exact_hinge.angle_magnitude_bits
        || exact_hinge.endpoint_vertices[0] == exact_hinge.endpoint_vertices[1]
    {
        return Ok(SharedHingeNativeExactTopologyMarginResultV1::Unresolved);
    }

    charge_fixed_work(work, limits)?;
    let thickness = exact_f64(paper_thickness_mm, meter, STAGE)?;
    let two = BigRational::from_integer(2.into());
    let half_thickness = meter.divide_rational(&thickness, &two, STAGE)?;
    if meter.compare_rational(&half_thickness, &BigRational::zero(), STAGE)? != Ordering::Greater {
        return Ok(SharedHingeNativeExactTopologyMarginResultV1::Unresolved);
    }

    let face_transform_bits = ef_boundary.binary64_face_transforms;
    let direct_transforms = [
        lift_transform_bits(&face_transform_bits[0], meter)?,
        lift_transform_bits(&face_transform_bits[1], meter)?,
    ];
    let faces = [
        reconstruct_face_topology(
            exact,
            bound,
            exact_hinge,
            0,
            &direct_transforms[0],
            &half_thickness,
            meter,
        )?,
        reconstruct_face_topology(
            exact,
            bound,
            exact_hinge,
            1,
            &direct_transforms[1],
            &half_thickness,
            meter,
        )?,
    ];
    if faces[0].identity.face != exact.faces[0].face
        || faces[1].identity.face != exact.faces[1].face
        || faces[prerequisite.left_face_index].identity.face != native_hinge.left_face()
        || faces[prerequisite.right_face_index].identity.face != native_hinge.right_face()
        || meter.compare_rational(
            &faces[0].source_halfspace_support,
            &BigRational::zero(),
            STAGE,
        )? == Ordering::Equal
        || meter.compare_rational(
            &faces[1].source_halfspace_support,
            &BigRational::zero(),
            STAGE,
        )? == Ordering::Equal
        || faces[0].source_halfspace_support.is_positive()
            == faces[1].source_halfspace_support.is_positive()
    {
        return Ok(SharedHingeNativeExactTopologyMarginResultV1::Unresolved);
    }

    let source_endpoints = [
        lift_model_vertex(exact_hinge.endpoint_vertices[0], bound, meter)?,
        lift_model_vertex(exact_hinge.endpoint_vertices[1], bound, meter)?,
    ];
    let exact_endpoints = [
        clone_point(&exact_hinge.world_endpoints[0], meter)?,
        clone_point(&exact_hinge.world_endpoints[1], meter)?,
    ];
    for face in &faces {
        for endpoint in 0..2 {
            let vertex = exact_hinge.endpoint_vertices[endpoint];
            let Some(index) = face
                .identity
                .boundary_vertices
                .iter()
                .position(|candidate| *candidate == vertex)
            else {
                return Ok(SharedHingeNativeExactTopologyMarginResultV1::Unresolved);
            };
            if !canonical_point_eq(&face.source_vertices[index], &source_endpoints[endpoint])
                || !canonical_point_eq(&face.exact_mid_surface[index], &exact_endpoints[endpoint])
            {
                return Ok(SharedHingeNativeExactTopologyMarginResultV1::Unresolved);
            }
        }
    }

    let relative_margin = {
        let epsilon = exact_f64(f64::EPSILON, meter, STAGE)?;
        let factor = BigRational::from_integer(RELATIVE_MARGIN_ULP_FACTOR.into());
        meter.multiply_rational(&epsilon, &factor, STAGE)?
    };
    let local_scale = calculate_local_scale(&faces, &thickness, limits, work, meter)?;
    let point_margin = meter.multiply_rational(&local_scale, &relative_margin, STAGE)?;
    let normal_margin = meter.clone_rational(&relative_margin, STAGE)?;
    let normal_solid_allowance = meter.multiply_rational(&half_thickness, &normal_margin, STAGE)?;
    let solid_margin = meter.add_rational(&point_margin, &normal_solid_allowance, STAGE)?;

    let mut violations = ViolationAccumulator::default();
    let mut exact_axially_bounded = [false; FACE_COUNT];
    let mut direct_axially_bounded = [false; FACE_COUNT];
    for face_index in 0..FACE_COUNT {
        let exact_scan = scan_material_frame(
            &faces[face_index].exact_frame,
            &faces[face_index].exact_mid_surface,
            &faces[face_index].exact_solid,
            &half_thickness,
            limits,
            work,
            meter,
        )?;
        let direct_scan = scan_material_frame(
            &faces[face_index].direct_frame,
            &faces[face_index].direct_mid_surface,
            &faces[face_index].direct_solid,
            &half_thickness,
            limits,
            work,
            meter,
        )?;
        exact_axially_bounded[face_index] = exact_scan.axially_bounded;
        direct_axially_bounded[face_index] = direct_scan.axially_bounded;
        for _ in 0..exact_scan.halfspace_violation_count + direct_scan.halfspace_violation_count {
            violations.record(SharedHingeTopologyMarginRelationV1::MaterialHalfSpace)?;
        }
        for _ in
            0..exact_scan.normal_solid_violation_count + direct_scan.normal_solid_violation_count
        {
            violations.record(SharedHingeTopologyMarginRelationV1::MaterialNormalOrSolid)?;
        }
        if exact_scan.axially_bounded != faces[face_index].source_axially_bounded
            || direct_scan.axially_bounded != faces[face_index].source_axially_bounded
        {
            violations.record(SharedHingeTopologyMarginRelationV1::FiniteAxis)?;
        }
    }
    let source_axially_bounded = [
        faces[0].source_axially_bounded,
        faces[1].source_axially_bounded,
    ];
    if !source_axially_bounded.iter().any(|bounded| *bounded)
        || !exact_axially_bounded.iter().any(|bounded| *bounded)
        || !direct_axially_bounded.iter().any(|bounded| *bounded)
    {
        violations.record(SharedHingeTopologyMarginRelationV1::FiniteAxis)?;
    }

    let (observed_point_error, observed_normal_error, observed_solid_error) =
        scan_ef_geometry_errors(
            &faces,
            &point_margin,
            &normal_margin,
            &solid_margin,
            limits,
            work,
            meter,
            &mut violations,
        )?;
    if !reauthenticate_ef_scalars(
        ef_boundary,
        &faces,
        &observed_point_error,
        &observed_normal_error,
        &half_thickness,
        limits,
        work,
        meter,
    )? {
        return Ok(SharedHingeNativeExactTopologyMarginResultV1::Unresolved);
    }
    let (direct_endpoint_occurrences, observed_shared_endpoint_error) =
        scan_shared_endpoint_errors(
            &faces,
            exact_hinge.endpoint_vertices,
            &point_margin,
            limits,
            work,
            meter,
            &mut violations,
        )?;

    let exact_axis_start = clone_point(&exact_endpoints[0], meter)?;
    let exact_axis = exact_between(&exact_endpoints[0], &exact_endpoints[1], meter)?;
    let exact_length_squared = exact_dot(&exact_axis, &exact_axis, meter)?;
    let parent_face_index = exact
        .faces
        .iter()
        .position(|face| face.face == exact_hinge.parent)
        .ok_or(CayleyError::InvariantFailure { stage: STAGE })?;
    let parent_start_index = faces[parent_face_index]
        .identity
        .boundary_vertices
        .iter()
        .position(|vertex| *vertex == exact_hinge.endpoint_vertices[0])
        .ok_or(CayleyError::InvariantFailure { stage: STAGE })?;
    let parent_end_index = faces[parent_face_index]
        .identity
        .boundary_vertices
        .iter()
        .position(|vertex| *vertex == exact_hinge.endpoint_vertices[1])
        .ok_or(CayleyError::InvariantFailure { stage: STAGE })?;
    let direct_axis_start = clone_point(
        &faces[parent_face_index].direct_mid_surface[parent_start_index],
        meter,
    )?;
    let direct_axis = exact_between(
        &direct_axis_start,
        &faces[parent_face_index].direct_mid_surface[parent_end_index],
        meter,
    )?;
    let direct_length_squared = exact_dot(&direct_axis, &direct_axis, meter)?;
    if meter.compare_rational(&exact_length_squared, &BigRational::zero(), STAGE)?
        != Ordering::Greater
        || meter.compare_rational(&direct_length_squared, &BigRational::zero(), STAGE)?
            != Ordering::Greater
    {
        return Ok(SharedHingeNativeExactTopologyMarginResultV1::Unresolved);
    }
    let (corridor_component_error, corridor_component_margin) = scan_corridor_component_errors(
        &exact_axis_start,
        &exact_axis,
        &exact_length_squared,
        &direct_axis_start,
        &direct_axis,
        &direct_length_squared,
        &faces,
        prerequisite,
        &half_thickness,
        &point_margin,
        &normal_margin,
        limits,
        work,
        meter,
        &mut violations,
    )?;

    let hinge_parent_transform =
        capture_hinge_parent_transform(bound, exact_hinge.edge, exact_hinge.parent)
            .ok_or(CayleyError::InvariantFailure { stage: STAGE })?;
    if hinge_parent_transform != face_transform_bits[parent_face_index] {
        return Ok(SharedHingeNativeExactTopologyMarginResultV1::Unresolved);
    }

    if let Some(violation) = violations.finish() {
        return Ok(SharedHingeNativeExactTopologyMarginResultV1::OutsideMargin(
            violation,
        ));
    }

    let exact_solid_bounds = [
        point_bounds(&faces[0].exact_solid, meter)?,
        point_bounds(&faces[1].exact_solid, meter)?,
    ];
    let direct_solid_bounds = [
        point_bounds(&faces[0].direct_solid, meter)?,
        point_bounds(&faces[1].direct_solid, meter)?,
    ];
    let geometry = SharedHingeNativeExactTopologyMarginGeometryV1 {
        source_endpoints,
        exact_endpoints,
        direct_endpoint_occurrences,
        source_halfspace_support: [
            meter.clone_rational(&faces[0].source_halfspace_support, STAGE)?,
            meter.clone_rational(&faces[1].source_halfspace_support, STAGE)?,
        ],
        exact_frame_determinants: [
            meter.clone_rational(&faces[0].exact_frame.determinant, STAGE)?,
            meter.clone_rational(&faces[1].exact_frame.determinant, STAGE)?,
        ],
        direct_frame_determinants: [
            meter.clone_rational(&faces[0].direct_frame.determinant, STAGE)?,
            meter.clone_rational(&faces[1].direct_frame.determinant, STAGE)?,
        ],
        exact_face_normals: [
            clone_vector(&faces[0].exact_normal, meter)?,
            clone_vector(&faces[1].exact_normal, meter)?,
        ],
        direct_face_normals: [
            clone_vector(&faces[0].direct_normal, meter)?,
            clone_vector(&faces[1].direct_normal, meter)?,
        ],
        exact_solid_bounds,
        direct_solid_bounds,
        exact_axis_start,
        exact_axis,
        exact_length_squared,
        direct_axis_start,
        direct_axis,
        direct_length_squared,
        half_thickness_mm: half_thickness,
        relative_margin,
        local_scale_mm: local_scale,
        point_margin_mm: point_margin,
        normal_margin,
        solid_margin_mm: solid_margin,
        observed_shared_endpoint_component_error_mm: observed_shared_endpoint_error,
        observed_point_component_error_mm: observed_point_error,
        observed_normal_component_error: observed_normal_error,
        observed_solid_component_error_mm: observed_solid_error,
        corridor_component_error,
        corridor_component_margin,
        source_axially_bounded,
        exact_axially_bounded,
        direct_axially_bounded,
    };

    let sealed_geometry = clone_topology_geometry(&geometry, meter)?;
    Ok(SharedHingeNativeExactTopologyMarginResultV1::Measured(
        Box::new(SharedHingeNativeExactTopologyMarginCapabilityV1 {
            prerequisite,
            ef_boundary,
            exact,
            bound,
            paper_thickness_bits: paper_thickness_mm.to_bits(),
            fixed_face,
            face_topology: [faces[0].identity, faces[1].identity],
            hinge_edge: exact_hinge.edge,
            hinge_parent: exact_hinge.parent,
            hinge_child: exact_hinge.child,
            hinge_endpoint_vertices: exact_hinge.endpoint_vertices,
            hinge_angle: BoundHingeAngleBitsV1 {
                edge: native_angle.edge(),
                angle_degrees_bits: native_angle.angle_degrees().to_bits(),
            },
            binary64_face_transforms: face_transform_bits,
            hinge_parent_transform,
            geometry,
            sealed_geometry: Some(sealed_geometry),
            sealed_work: None,
        }),
    ))
}

fn reconstruct_face_topology(
    exact: &RationalCayleyTreePose<'_>,
    bound: BoundMaterialTreePose<'_>,
    exact_hinge: &super::super::ExactHingePose,
    face_index: usize,
    direct_transform: &ExactRigidTransform,
    half_thickness: &BigRational,
    meter: &mut WorkMeter<'_>,
) -> Result<ReconstructedFaceTopology, CayleyError> {
    let exact_face = exact
        .faces
        .get(face_index)
        .ok_or(CayleyError::InvariantFailure { stage: STAGE })?;
    let source_boundary = bound
        .face_boundary(exact_face.face)
        .filter(|boundary| {
            bound.model().owns_face_boundary(*boundary)
                && bound.pose().owns_face_boundary(*boundary)
                && boundary.vertices().len() == VERTICES_PER_FACE
                && boundary.edges().len() == VERTICES_PER_FACE
        })
        .ok_or(CayleyError::BoundTreeInconsistent { stage: STAGE })?;
    let boundary_vertices: [VertexId; VERTICES_PER_FACE] = source_boundary
        .vertices()
        .try_into()
        .map_err(|_| CayleyError::InvariantFailure { stage: STAGE })?;
    let boundary_edges: [EdgeId; VERTICES_PER_FACE] = source_boundary
        .edges()
        .try_into()
        .map_err(|_| CayleyError::InvariantFailure { stage: STAGE })?;
    if exact_face.boundary.len() != VERTICES_PER_FACE
        || !exact_face
            .boundary
            .iter()
            .map(|(vertex, _)| *vertex)
            .eq(boundary_vertices)
    {
        return Err(CayleyError::BoundTreeInconsistent { stage: STAGE });
    }
    let hinge_occurrence = unique_edge_occurrence(&boundary_edges, exact_hinge.edge)
        .ok_or(CayleyError::BoundTreeInconsistent { stage: STAGE })?;
    let start_index =
        unique_vertex_occurrence(&boundary_vertices, exact_hinge.endpoint_vertices[0])
            .ok_or(CayleyError::BoundTreeInconsistent { stage: STAGE })?;
    let end_index = unique_vertex_occurrence(&boundary_vertices, exact_hinge.endpoint_vertices[1])
        .ok_or(CayleyError::BoundTreeInconsistent { stage: STAGE })?;
    if start_index == end_index
        || !matches!(
            (
                boundary_vertices[hinge_occurrence],
                boundary_vertices[(hinge_occurrence + 1) % VERTICES_PER_FACE],
            ),
            (start, end)
                if (start == exact_hinge.endpoint_vertices[0]
                    && end == exact_hinge.endpoint_vertices[1])
                    || (start == exact_hinge.endpoint_vertices[1]
                        && end == exact_hinge.endpoint_vertices[0])
        )
    {
        return Err(CayleyError::BoundTreeInconsistent { stage: STAGE });
    }
    let opposite_index = (0..VERTICES_PER_FACE)
        .find(|index| *index != start_index && *index != end_index)
        .ok_or(CayleyError::BoundTreeInconsistent { stage: STAGE })?;
    let identity = BoundFaceTopologyIdentityV1 {
        face: exact_face.face,
        boundary_vertices,
        boundary_edges,
        hinge_occurrence,
        opposite_vertex: boundary_vertices[opposite_index],
    };

    let source_vertices =
        try_array3(|index| lift_model_vertex(boundary_vertices[index], bound, meter))?;
    let exact_mid_surface = try_array3(|index| {
        let reconstructed =
            apply_exact_transform(&exact_face.transform, &source_vertices[index], meter)?;
        if !canonical_point_eq(&reconstructed, &exact_face.boundary[index].1) {
            return Err(CayleyError::BoundTreeInconsistent { stage: STAGE });
        }
        Ok(reconstructed)
    })?;
    let direct_mid_surface = try_array3(|index| {
        apply_exact_transform(direct_transform, &source_vertices[index], meter)
    })?;
    let exact_normal = exact_local_y(&exact_face.transform, meter)?;
    let direct_normal = ExactVector3 {
        coordinates: try_array3(|axis| {
            meter.clone_rational(&direct_transform.rotation[axis][1], STAGE)
        })?,
    };
    let exact_solid =
        construct_solid_vertices(&exact_mid_surface, &exact_normal, half_thickness, meter)?;
    let direct_solid =
        construct_solid_vertices(&direct_mid_surface, &direct_normal, half_thickness, meter)?;

    let source_start = &source_vertices[start_index];
    let source_end = &source_vertices[end_index];
    let source_opposite = &source_vertices[opposite_index];
    let source_axis = exact_between(source_start, source_end, meter)?;
    let source_to_opposite = exact_between(source_start, source_opposite, meter)?;
    let axis_squared = exact_dot(&source_axis, &source_axis, meter)?;
    if meter.compare_rational(&axis_squared, &BigRational::zero(), STAGE)? != Ordering::Greater {
        return Err(CayleyError::BoundTreeInconsistent { stage: STAGE });
    }
    let projection_numerator = exact_dot(&source_to_opposite, &source_axis, meter)?;
    let projection = meter.divide_rational(&projection_numerator, &axis_squared, STAGE)?;
    let projected_axis = scale_vector(&source_axis, &projection, meter)?;
    let source_inward = subtract_vectors(&source_to_opposite, &projected_axis, meter)?;
    let source_halfspace_support = exact_dot(&source_inward, &source_to_opposite, meter)?;
    if meter.compare_rational(&source_halfspace_support, &BigRational::zero(), STAGE)?
        != Ordering::Greater
    {
        return Err(CayleyError::BoundTreeInconsistent { stage: STAGE });
    }
    let source_orientation =
        exact_rest_support_orientation_xz(source_start, source_end, source_opposite, meter)?;
    if source_orientation.is_zero() {
        return Err(CayleyError::BoundTreeInconsistent { stage: STAGE });
    }
    let source_axially_bounded = source_vertices.iter().try_fold(true, |all_inside, point| {
        let offset = exact_between(source_start, point, meter)?;
        let axial = exact_dot(&offset, &source_axis, meter)?;
        Ok::<bool, CayleyError>(
            all_inside
                && meter.compare_rational(&axial, &BigRational::zero(), STAGE)? != Ordering::Less
                && meter.compare_rational(&axial, &axis_squared, STAGE)? != Ordering::Greater,
        )
    })?;

    let exact_frame = build_material_frame(
        &exact_face.transform,
        source_start,
        &source_axis,
        &source_inward,
        meter,
    )?;
    let direct_frame = build_material_frame(
        direct_transform,
        source_start,
        &source_axis,
        &source_inward,
        meter,
    )?;
    if exact_frame.determinant.is_zero()
        || direct_frame.determinant.is_zero()
        || exact_frame.determinant.is_positive() != direct_frame.determinant.is_positive()
    {
        return Err(CayleyError::BoundTreeInconsistent { stage: STAGE });
    }

    Ok(ReconstructedFaceTopology {
        identity,
        source_vertices,
        exact_mid_surface,
        direct_mid_surface,
        exact_normal,
        direct_normal,
        exact_solid,
        direct_solid,
        source_halfspace_support: source_orientation,
        source_axially_bounded,
        exact_frame,
        direct_frame,
    })
}

fn unique_vertex_occurrence(vertices: &[VertexId], vertex: VertexId) -> Option<usize> {
    let mut matches = vertices
        .iter()
        .enumerate()
        .filter(|(_, candidate)| **candidate == vertex);
    let (index, _) = matches.next()?;
    matches.next().is_none().then_some(index)
}

fn lift_model_vertex(
    vertex: VertexId,
    bound: BoundMaterialTreePose<'_>,
    meter: &mut WorkMeter<'_>,
) -> Result<ExactPoint3, CayleyError> {
    let point = bound
        .model()
        .vertex_position(vertex)
        .ok_or(CayleyError::BoundTreeInconsistent { stage: STAGE })?;
    exact_point_at_stage(point3_array(point), meter)
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

fn construct_solid_vertices(
    mid_surface: &[ExactPoint3; VERTICES_PER_FACE],
    normal: &ExactVector3,
    half_thickness: &BigRational,
    meter: &mut WorkMeter<'_>,
) -> Result<[ExactPoint3; SOLID_VERTICES_PER_FACE], CayleyError> {
    let offset = scale_vector(normal, half_thickness, meter)?;
    Ok([
        offset_point(&mid_surface[0], &offset, true, meter)?,
        offset_point(&mid_surface[1], &offset, true, meter)?,
        offset_point(&mid_surface[2], &offset, true, meter)?,
        offset_point(&mid_surface[0], &offset, false, meter)?,
        offset_point(&mid_surface[1], &offset, false, meter)?,
        offset_point(&mid_surface[2], &offset, false, meter)?,
    ])
}

fn offset_point(
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

fn build_material_frame(
    transform: &ExactRigidTransform,
    source_start: &ExactPoint3,
    source_axis: &ExactVector3,
    source_inward: &ExactVector3,
    meter: &mut WorkMeter<'_>,
) -> Result<ExactMaterialFrame, CayleyError> {
    let start = apply_exact_transform(transform, source_start, meter)?;
    let axis = apply_linear(&transform.rotation, source_axis, meter)?;
    let inward = apply_linear(&transform.rotation, source_inward, meter)?;
    let normal = ExactVector3 {
        coordinates: try_array3(|row| meter.clone_rational(&transform.rotation[row][1], STAGE))?,
    };
    let inward_cross_normal = cross(&inward, &normal, meter)?;
    let determinant = exact_dot(&axis, &inward_cross_normal, meter)?;
    Ok(ExactMaterialFrame {
        start,
        axis,
        inward,
        normal,
        determinant,
    })
}

fn apply_linear(
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

fn scale_vector(
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

fn subtract_vectors(
    left: &ExactVector3,
    right: &ExactVector3,
    meter: &mut WorkMeter<'_>,
) -> Result<ExactVector3, CayleyError> {
    Ok(ExactVector3 {
        coordinates: try_array3(|axis| {
            meter.subtract_rational(&left.coordinates[axis], &right.coordinates[axis], STAGE)
        })?,
    })
}

fn cross(
    left: &ExactVector3,
    right: &ExactVector3,
    meter: &mut WorkMeter<'_>,
) -> Result<ExactVector3, CayleyError> {
    let first = |a: usize,
                 b: usize,
                 c: usize,
                 d: usize,
                 meter: &mut WorkMeter<'_>|
     -> Result<BigRational, CayleyError> {
        let positive =
            meter.multiply_rational(&left.coordinates[a], &right.coordinates[b], STAGE)?;
        let negative =
            meter.multiply_rational(&left.coordinates[c], &right.coordinates[d], STAGE)?;
        meter.subtract_rational(&positive, &negative, STAGE)
    };
    Ok(ExactVector3 {
        coordinates: [
            first(1, 2, 2, 1, meter)?,
            first(2, 0, 0, 2, meter)?,
            first(0, 1, 1, 0, meter)?,
        ],
    })
}

fn material_coordinates(
    frame: &ExactMaterialFrame,
    point: &ExactPoint3,
    meter: &mut WorkMeter<'_>,
) -> Result<[BigRational; 3], CayleyError> {
    if frame.determinant.is_zero() {
        return Err(CayleyError::BoundTreeInconsistent { stage: STAGE });
    }
    let offset = exact_between(&frame.start, point, meter)?;
    let inward_cross_normal = cross(&frame.inward, &frame.normal, meter)?;
    let normal_cross_axis = cross(&frame.normal, &frame.axis, meter)?;
    let axis_cross_inward = cross(&frame.axis, &frame.inward, meter)?;
    let numerators = [
        exact_dot(&offset, &inward_cross_normal, meter)?,
        exact_dot(&offset, &normal_cross_axis, meter)?,
        exact_dot(&offset, &axis_cross_inward, meter)?,
    ];
    try_array3(|coordinate| {
        meter.divide_rational(&numerators[coordinate], &frame.determinant, STAGE)
    })
}

fn scan_material_frame(
    frame: &ExactMaterialFrame,
    mid_surface: &[ExactPoint3; VERTICES_PER_FACE],
    solid: &[ExactPoint3; SOLID_VERTICES_PER_FACE],
    half_thickness: &BigRational,
    limits: &SharedHingeNativeExactTopologyMarginLimitsV1,
    work: &mut SharedHingeNativeExactTopologyMarginWorkV1,
    meter: &mut WorkMeter<'_>,
) -> Result<TopologyScanSummary, CayleyError> {
    let mut summary = TopologyScanSummary {
        axially_bounded: true,
        ..TopologyScanSummary::default()
    };
    for (solid_kind, points) in [(false, mid_surface.as_slice()), (true, solid.as_slice())] {
        for (index, point) in points.iter().enumerate() {
            let coordinates = material_coordinates(frame, point, meter)?;
            for _ in 0..3 {
                charge_counter(
                    &mut work.topology_coordinate_tests,
                    limits.max_topology_coordinate_tests,
                    "topology_margin_coordinate_tests",
                )?;
            }
            let axial_inside =
                meter.compare_rational(&coordinates[0], &BigRational::zero(), STAGE)?
                    != Ordering::Less
                    && meter.compare_rational(&coordinates[0], &BigRational::one(), STAGE)?
                        != Ordering::Greater;
            summary.axially_bounded &= axial_inside;
            if meter.compare_rational(&coordinates[1], &BigRational::zero(), STAGE)?
                == Ordering::Less
            {
                summary.halfspace_violation_count = summary
                    .halfspace_violation_count
                    .checked_add(1)
                    .ok_or(CayleyError::ResourceLimitExceeded {
                        stage: STAGE,
                        resource: "topology_margin_halfspace_violations",
                    })?;
            }
            let expected_beta = if !solid_kind {
                BigRational::zero()
            } else if index < VERTICES_PER_FACE {
                meter.clone_rational(half_thickness, STAGE)?
            } else {
                meter.negate_rational(half_thickness, STAGE)?
            };
            if meter.compare_rational(&coordinates[2], &expected_beta, STAGE)? != Ordering::Equal {
                summary.normal_solid_violation_count = summary
                    .normal_solid_violation_count
                    .checked_add(1)
                    .ok_or(CayleyError::ResourceLimitExceeded {
                        stage: STAGE,
                        resource: "topology_margin_normal_solid_violations",
                    })?;
            }
        }
    }
    Ok(summary)
}

fn calculate_local_scale(
    faces: &[ReconstructedFaceTopology; FACE_COUNT],
    thickness: &BigRational,
    limits: &SharedHingeNativeExactTopologyMarginLimitsV1,
    work: &mut SharedHingeNativeExactTopologyMarginWorkV1,
    meter: &mut WorkMeter<'_>,
) -> Result<BigRational, CayleyError> {
    let mut source_min: [Option<BigRational>; 3] = std::array::from_fn(|_| None);
    let mut source_max: [Option<BigRational>; 3] = std::array::from_fn(|_| None);
    let mut exact_min: [Option<BigRational>; 3] = std::array::from_fn(|_| None);
    let mut exact_max: [Option<BigRational>; 3] = std::array::from_fn(|_| None);
    for face in faces {
        for point in &face.source_vertices {
            update_scale_bounds(point, &mut source_min, &mut source_max, limits, work, meter)?;
        }
        for point in &face.exact_solid {
            update_scale_bounds(point, &mut exact_min, &mut exact_max, limits, work, meter)?;
        }
    }
    if work.local_scale_component_scans != LOCAL_SCALE_COMPONENT_SCANS {
        return Err(CayleyError::InvariantFailure { stage: STAGE });
    }
    let mut local_scale = BigRational::one();
    if meter.compare_rational(thickness, &local_scale, STAGE)? == Ordering::Greater {
        local_scale = meter.clone_rational(thickness, STAGE)?;
    }
    for axis in 0..3 {
        for (minimum, maximum) in [
            (&source_min[axis], &source_max[axis]),
            (&exact_min[axis], &exact_max[axis]),
        ] {
            let (Some(minimum), Some(maximum)) = (minimum, maximum) else {
                return Err(CayleyError::InvariantFailure { stage: STAGE });
            };
            let span = meter.subtract_rational(maximum, minimum, STAGE)?;
            if meter.compare_rational(&span, &local_scale, STAGE)? == Ordering::Greater {
                local_scale = span;
            }
        }
    }
    if meter.compare_rational(&local_scale, &BigRational::zero(), STAGE)? != Ordering::Greater {
        return Err(CayleyError::BoundTreeInconsistent { stage: STAGE });
    }
    Ok(local_scale)
}

fn update_scale_bounds(
    point: &ExactPoint3,
    minimum: &mut [Option<BigRational>; 3],
    maximum: &mut [Option<BigRational>; 3],
    limits: &SharedHingeNativeExactTopologyMarginLimitsV1,
    work: &mut SharedHingeNativeExactTopologyMarginWorkV1,
    meter: &mut WorkMeter<'_>,
) -> Result<(), CayleyError> {
    for axis in 0..3 {
        charge_counter(
            &mut work.local_scale_component_scans,
            limits.max_local_scale_component_scans,
            "topology_margin_local_scale_components",
        )?;
        match (&minimum[axis], &maximum[axis]) {
            (Some(current_minimum), Some(current_maximum)) => {
                if meter.compare_rational(&point.coordinates[axis], current_minimum, STAGE)?
                    == Ordering::Less
                {
                    minimum[axis] = Some(meter.clone_rational(&point.coordinates[axis], STAGE)?);
                }
                if meter.compare_rational(&point.coordinates[axis], current_maximum, STAGE)?
                    == Ordering::Greater
                {
                    maximum[axis] = Some(meter.clone_rational(&point.coordinates[axis], STAGE)?);
                }
            }
            (None, None) => {
                minimum[axis] = Some(meter.clone_rational(&point.coordinates[axis], STAGE)?);
                maximum[axis] = Some(meter.clone_rational(&point.coordinates[axis], STAGE)?);
            }
            _ => return Err(CayleyError::InvariantFailure { stage: STAGE }),
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn scan_ef_geometry_errors(
    faces: &[ReconstructedFaceTopology; FACE_COUNT],
    point_margin: &BigRational,
    normal_margin: &BigRational,
    solid_margin: &BigRational,
    limits: &SharedHingeNativeExactTopologyMarginLimitsV1,
    work: &mut SharedHingeNativeExactTopologyMarginWorkV1,
    meter: &mut WorkMeter<'_>,
    violations: &mut ViolationAccumulator,
) -> Result<ComponentErrorTriplet, CayleyError> {
    let mut point_maximum = std::array::from_fn(|_| BigRational::zero());
    let mut normal_maximum = std::array::from_fn(|_| BigRational::zero());
    let mut solid_maximum = std::array::from_fn(|_| BigRational::zero());
    for face in faces {
        for vertex in 0..VERTICES_PER_FACE {
            scan_point_component_error(
                &face.exact_mid_surface[vertex],
                &face.direct_mid_surface[vertex],
                point_margin,
                &mut point_maximum,
                &mut work.point_component_error_tests,
                limits.max_point_component_error_tests,
                "topology_margin_point_error_components",
                SharedHingeTopologyMarginRelationV1::PointError,
                meter,
                violations,
            )?;
        }
        scan_vector_component_error(
            &face.exact_normal,
            &face.direct_normal,
            normal_margin,
            &mut normal_maximum,
            &mut work.normal_component_error_tests,
            limits.max_normal_component_error_tests,
            "topology_margin_normal_error_components",
            SharedHingeTopologyMarginRelationV1::NormalError,
            meter,
            violations,
        )?;
        for vertex in 0..SOLID_VERTICES_PER_FACE {
            scan_point_component_error(
                &face.exact_solid[vertex],
                &face.direct_solid[vertex],
                solid_margin,
                &mut solid_maximum,
                &mut work.solid_component_error_tests,
                limits.max_solid_component_error_tests,
                "topology_margin_solid_error_components",
                SharedHingeTopologyMarginRelationV1::SolidError,
                meter,
                violations,
            )?;
        }
    }
    if work.point_component_error_tests != POINT_COMPONENT_ERROR_TESTS
        || work.normal_component_error_tests != NORMAL_COMPONENT_ERROR_TESTS
        || work.solid_component_error_tests != SOLID_COMPONENT_ERROR_TESTS
    {
        return Err(CayleyError::InvariantFailure { stage: STAGE });
    }
    Ok((point_maximum, normal_maximum, solid_maximum))
}

#[allow(clippy::too_many_arguments)]
fn scan_point_component_error(
    exact: &ExactPoint3,
    direct: &ExactPoint3,
    margin: &BigRational,
    maximum: &mut [BigRational; 3],
    counter: &mut usize,
    maximum_count: usize,
    resource: &'static str,
    relation: SharedHingeTopologyMarginRelationV1,
    meter: &mut WorkMeter<'_>,
    violations: &mut ViolationAccumulator,
) -> Result<(), CayleyError> {
    for (axis, current_maximum) in maximum.iter_mut().enumerate() {
        charge_counter(counter, maximum_count, resource)?;
        let delta =
            meter.subtract_rational(&direct.coordinates[axis], &exact.coordinates[axis], STAGE)?;
        let magnitude = meter.absolute_rational(&delta, STAGE)?;
        if meter.compare_rational(&magnitude, current_maximum, STAGE)? == Ordering::Greater {
            *current_maximum = meter.clone_rational(&magnitude, STAGE)?;
        }
        if meter.compare_rational(&magnitude, margin, STAGE)? == Ordering::Greater {
            violations.record(relation)?;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn scan_vector_component_error(
    exact: &ExactVector3,
    direct: &ExactVector3,
    margin: &BigRational,
    maximum: &mut [BigRational; 3],
    counter: &mut usize,
    maximum_count: usize,
    resource: &'static str,
    relation: SharedHingeTopologyMarginRelationV1,
    meter: &mut WorkMeter<'_>,
    violations: &mut ViolationAccumulator,
) -> Result<(), CayleyError> {
    for (axis, current_maximum) in maximum.iter_mut().enumerate() {
        charge_counter(counter, maximum_count, resource)?;
        let delta =
            meter.subtract_rational(&direct.coordinates[axis], &exact.coordinates[axis], STAGE)?;
        let magnitude = meter.absolute_rational(&delta, STAGE)?;
        if meter.compare_rational(&magnitude, current_maximum, STAGE)? == Ordering::Greater {
            *current_maximum = meter.clone_rational(&magnitude, STAGE)?;
        }
        if meter.compare_rational(&magnitude, margin, STAGE)? == Ordering::Greater {
            violations.record(relation)?;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn reauthenticate_ef_scalars(
    ef_boundary: &AxisAlignedEfBoundaryCapabilityV1<'_, '_, '_>,
    faces: &[ReconstructedFaceTopology; FACE_COUNT],
    global_point_error: &[BigRational; 3],
    global_normal_error: &[BigRational; 3],
    half_thickness: &BigRational,
    limits: &SharedHingeNativeExactTopologyMarginLimitsV1,
    work: &mut SharedHingeNativeExactTopologyMarginWorkV1,
    meter: &mut WorkMeter<'_>,
) -> Result<bool, CayleyError> {
    let mut all_equal = true;
    for (face_index, face) in faces.iter().enumerate() {
        let point =
            component_max_for_points(&face.exact_mid_surface, &face.direct_mid_surface, meter)?;
        let normal = component_error_for_vectors(&face.exact_normal, &face.direct_normal, meter)?;
        let solid = solid_error_bound(&point, &normal, half_thickness, meter)?;
        let expected = &ef_boundary.faces[face_index];
        all_equal &= expected.face == face.identity.face;
        all_equal &=
            compare_ef_face_record(expected, &point, &normal, &solid, limits, work, meter)?;
    }
    let global_solid = solid_error_bound(
        global_point_error,
        global_normal_error,
        half_thickness,
        meter,
    )?;
    all_equal &= compare_ef_record_scalars(
        [
            &ef_boundary.point_component_bound_mm,
            &ef_boundary.normal_component_bound,
            &ef_boundary.solid_component_bound_mm,
        ],
        [
            &ef_boundary.point_linf_bound_mm,
            &ef_boundary.normal_linf_bound,
            &ef_boundary.solid_linf_bound_mm,
        ],
        [global_point_error, global_normal_error, &global_solid],
        limits,
        work,
        meter,
    )?;
    if work.ef_scalar_reauthentications != EF_SCALAR_REAUTHENTICATIONS {
        return Err(CayleyError::InvariantFailure { stage: STAGE });
    }
    Ok(all_equal)
}

#[allow(clippy::too_many_arguments)]
fn compare_ef_face_record(
    observed: &AxisAlignedFaceEfErrorBounds,
    point: &[BigRational; 3],
    normal: &[BigRational; 3],
    solid: &[BigRational; 3],
    limits: &SharedHingeNativeExactTopologyMarginLimitsV1,
    work: &mut SharedHingeNativeExactTopologyMarginWorkV1,
    meter: &mut WorkMeter<'_>,
) -> Result<bool, CayleyError> {
    compare_ef_record_scalars(
        [
            &observed.point_component_bound_mm,
            &observed.normal_component_bound,
            &observed.solid_component_bound_mm,
        ],
        [
            &observed.point_linf_bound_mm,
            &observed.normal_linf_bound,
            &observed.solid_linf_bound_mm,
        ],
        [point, normal, solid],
        limits,
        work,
        meter,
    )
}

#[allow(clippy::too_many_arguments)]
fn compare_ef_record_scalars(
    observed_components: [&[BigRational; 3]; 3],
    observed_linf: [&BigRational; 3],
    expected_components: [&[BigRational; 3]; 3],
    limits: &SharedHingeNativeExactTopologyMarginLimitsV1,
    work: &mut SharedHingeNativeExactTopologyMarginWorkV1,
    meter: &mut WorkMeter<'_>,
) -> Result<bool, CayleyError> {
    let mut all_equal = true;
    for group in 0..3 {
        for axis in 0..3 {
            charge_counter(
                &mut work.ef_scalar_reauthentications,
                limits.max_ef_scalar_reauthentications,
                "topology_margin_ef_scalar_reauthentications",
            )?;
            all_equal &= meter.compare_rational(
                &observed_components[group][axis],
                &expected_components[group][axis],
                STAGE,
            )? == Ordering::Equal;
        }
        let expected_linf = component_linf(expected_components[group], meter)?;
        charge_counter(
            &mut work.ef_scalar_reauthentications,
            limits.max_ef_scalar_reauthentications,
            "topology_margin_ef_scalar_reauthentications",
        )?;
        all_equal &=
            meter.compare_rational(observed_linf[group], &expected_linf, STAGE)? == Ordering::Equal;
    }
    Ok(all_equal)
}

fn component_max_for_points(
    exact: &[ExactPoint3; VERTICES_PER_FACE],
    direct: &[ExactPoint3; VERTICES_PER_FACE],
    meter: &mut WorkMeter<'_>,
) -> Result<[BigRational; 3], CayleyError> {
    let mut maximum = std::array::from_fn(|_| BigRational::zero());
    for vertex in 0..VERTICES_PER_FACE {
        for (axis, current_maximum) in maximum.iter_mut().enumerate() {
            let delta = meter.subtract_rational(
                &direct[vertex].coordinates[axis],
                &exact[vertex].coordinates[axis],
                STAGE,
            )?;
            let magnitude = meter.absolute_rational(&delta, STAGE)?;
            if meter.compare_rational(&magnitude, current_maximum, STAGE)? == Ordering::Greater {
                *current_maximum = magnitude;
            }
        }
    }
    Ok(maximum)
}

fn component_error_for_vectors(
    exact: &ExactVector3,
    direct: &ExactVector3,
    meter: &mut WorkMeter<'_>,
) -> Result<[BigRational; 3], CayleyError> {
    try_array3(|axis| {
        let delta =
            meter.subtract_rational(&direct.coordinates[axis], &exact.coordinates[axis], STAGE)?;
        meter.absolute_rational(&delta, STAGE)
    })
}

fn solid_error_bound(
    point: &[BigRational; 3],
    normal: &[BigRational; 3],
    half_thickness: &BigRational,
    meter: &mut WorkMeter<'_>,
) -> Result<[BigRational; 3], CayleyError> {
    try_array3(|axis| {
        let normal_offset = meter.multiply_rational(half_thickness, &normal[axis], STAGE)?;
        meter.add_rational(&point[axis], &normal_offset, STAGE)
    })
}

fn component_linf(
    components: &[BigRational; 3],
    meter: &mut WorkMeter<'_>,
) -> Result<BigRational, CayleyError> {
    let mut maximum = meter.clone_rational(&components[0], STAGE)?;
    for component in &components[1..] {
        if meter.compare_rational(component, &maximum, STAGE)? == Ordering::Greater {
            maximum = meter.clone_rational(component, STAGE)?;
        }
    }
    Ok(maximum)
}

#[allow(clippy::too_many_arguments)]
fn scan_shared_endpoint_errors(
    faces: &[ReconstructedFaceTopology; FACE_COUNT],
    endpoint_vertices: [VertexId; 2],
    point_margin: &BigRational,
    limits: &SharedHingeNativeExactTopologyMarginLimitsV1,
    work: &mut SharedHingeNativeExactTopologyMarginWorkV1,
    meter: &mut WorkMeter<'_>,
    violations: &mut ViolationAccumulator,
) -> Result<([[ExactPoint3; 2]; FACE_COUNT], [BigRational; 3]), CayleyError> {
    let indexes = [
        [
            unique_vertex_occurrence(&faces[0].identity.boundary_vertices, endpoint_vertices[0])
                .ok_or(CayleyError::InvariantFailure { stage: STAGE })?,
            unique_vertex_occurrence(&faces[0].identity.boundary_vertices, endpoint_vertices[1])
                .ok_or(CayleyError::InvariantFailure { stage: STAGE })?,
        ],
        [
            unique_vertex_occurrence(&faces[1].identity.boundary_vertices, endpoint_vertices[0])
                .ok_or(CayleyError::InvariantFailure { stage: STAGE })?,
            unique_vertex_occurrence(&faces[1].identity.boundary_vertices, endpoint_vertices[1])
                .ok_or(CayleyError::InvariantFailure { stage: STAGE })?,
        ],
    ];
    let occurrences = [
        [
            clone_point(&faces[0].direct_mid_surface[indexes[0][0]], meter)?,
            clone_point(&faces[0].direct_mid_surface[indexes[0][1]], meter)?,
        ],
        [
            clone_point(&faces[1].direct_mid_surface[indexes[1][0]], meter)?,
            clone_point(&faces[1].direct_mid_surface[indexes[1][1]], meter)?,
        ],
    ];
    let mut maximum = std::array::from_fn(|_| BigRational::zero());
    for (first, second) in occurrences[0].iter().zip(&occurrences[1]) {
        for (axis, current_maximum) in maximum.iter_mut().enumerate() {
            charge_counter(
                &mut work.shared_endpoint_component_tests,
                limits.max_shared_endpoint_component_tests,
                "topology_margin_shared_endpoint_components",
            )?;
            let delta = meter.subtract_rational(
                &second.coordinates[axis],
                &first.coordinates[axis],
                STAGE,
            )?;
            let magnitude = meter.absolute_rational(&delta, STAGE)?;
            if meter.compare_rational(&magnitude, current_maximum, STAGE)? == Ordering::Greater {
                *current_maximum = meter.clone_rational(&magnitude, STAGE)?;
            }
            if meter.compare_rational(&magnitude, point_margin, STAGE)? == Ordering::Greater {
                violations.record(SharedHingeTopologyMarginRelationV1::SharedEndpoint)?;
            }
        }
    }
    if work.shared_endpoint_component_tests != SHARED_ENDPOINT_COMPONENT_TESTS {
        return Err(CayleyError::InvariantFailure { stage: STAGE });
    }
    Ok((occurrences, maximum))
}

#[allow(clippy::too_many_arguments)]
fn scan_corridor_component_errors(
    exact_axis_start: &ExactPoint3,
    exact_axis: &ExactVector3,
    exact_length_squared: &BigRational,
    direct_axis_start: &ExactPoint3,
    direct_axis: &ExactVector3,
    direct_length_squared: &BigRational,
    faces: &[ReconstructedFaceTopology; FACE_COUNT],
    prerequisite: &AuthenticatedSingleTriangularHingePrerequisitesV1<'_, '_>,
    half_thickness: &BigRational,
    point_margin: &BigRational,
    normal_margin: &BigRational,
    limits: &SharedHingeNativeExactTopologyMarginLimitsV1,
    work: &mut SharedHingeNativeExactTopologyMarginWorkV1,
    meter: &mut WorkMeter<'_>,
    violations: &mut ViolationAccumulator,
) -> Result<
    (
        [BigRational; CORRIDOR_COMPONENT_COUNT],
        [BigRational; CORRIDOR_COMPONENT_COUNT],
    ),
    CayleyError,
> {
    let exact_left = &faces[prerequisite.left_face_index].exact_normal;
    let exact_right = &faces[prerequisite.right_face_index].exact_normal;
    let direct_left = &faces[prerequisite.left_face_index].direct_normal;
    let direct_right = &faces[prerequisite.right_face_index].direct_normal;
    let exact_normal_dot = exact_dot(exact_left, exact_right, meter)?;
    let direct_normal_dot = exact_dot(direct_left, direct_right, meter)?;
    let one = BigRational::one();
    let two = BigRational::from_integer(2.into());
    let exact_cosine_half_squared = {
        let numerator = meter.add_rational(&one, &exact_normal_dot, STAGE)?;
        meter.divide_rational(&numerator, &two, STAGE)?
    };
    let direct_cosine_half_squared = {
        let numerator = meter.add_rational(&one, &direct_normal_dot, STAGE)?;
        meter.divide_rational(&numerator, &two, STAGE)?
    };
    let half_thickness_squared = meter.multiply_rational(half_thickness, half_thickness, STAGE)?;
    let exact_radial_limit_product =
        meter.multiply_rational(&half_thickness_squared, exact_length_squared, STAGE)?;
    let direct_radial_limit_product =
        meter.multiply_rational(&half_thickness_squared, direct_length_squared, STAGE)?;

    let axis_component_margin = meter.add_rational(point_margin, point_margin, STAGE)?;
    let length_squared_margin =
        squared_length_delta_margin(exact_axis, &axis_component_margin, meter)?;
    let cosine_half_squared_margin =
        normal_dot_delta_margin(exact_left, exact_right, normal_margin, meter)?;
    let radial_limit_product_margin =
        meter.multiply_rational(&half_thickness_squared, &length_squared_margin, STAGE)?;

    let exact_values = [
        meter.clone_rational(&exact_axis_start.coordinates[0], STAGE)?,
        meter.clone_rational(&exact_axis_start.coordinates[1], STAGE)?,
        meter.clone_rational(&exact_axis_start.coordinates[2], STAGE)?,
        meter.clone_rational(&exact_axis.coordinates[0], STAGE)?,
        meter.clone_rational(&exact_axis.coordinates[1], STAGE)?,
        meter.clone_rational(&exact_axis.coordinates[2], STAGE)?,
        meter.clone_rational(exact_length_squared, STAGE)?,
        meter.clone_rational(half_thickness, STAGE)?,
        exact_cosine_half_squared,
        exact_radial_limit_product,
    ];
    let direct_values = [
        meter.clone_rational(&direct_axis_start.coordinates[0], STAGE)?,
        meter.clone_rational(&direct_axis_start.coordinates[1], STAGE)?,
        meter.clone_rational(&direct_axis_start.coordinates[2], STAGE)?,
        meter.clone_rational(&direct_axis.coordinates[0], STAGE)?,
        meter.clone_rational(&direct_axis.coordinates[1], STAGE)?,
        meter.clone_rational(&direct_axis.coordinates[2], STAGE)?,
        meter.clone_rational(direct_length_squared, STAGE)?,
        meter.clone_rational(half_thickness, STAGE)?,
        direct_cosine_half_squared,
        direct_radial_limit_product,
    ];
    let component_margin = [
        meter.clone_rational(point_margin, STAGE)?,
        meter.clone_rational(point_margin, STAGE)?,
        meter.clone_rational(point_margin, STAGE)?,
        meter.clone_rational(&axis_component_margin, STAGE)?,
        meter.clone_rational(&axis_component_margin, STAGE)?,
        meter.clone_rational(&axis_component_margin, STAGE)?,
        length_squared_margin,
        BigRational::zero(),
        cosine_half_squared_margin,
        radial_limit_product_margin,
    ];
    let mut component_error = std::array::from_fn(|_| BigRational::zero());
    for component in 0..CORRIDOR_COMPONENT_COUNT {
        charge_counter(
            &mut work.corridor_component_scans,
            limits.max_corridor_component_scans,
            "topology_margin_corridor_components",
        )?;
        let delta =
            meter.subtract_rational(&direct_values[component], &exact_values[component], STAGE)?;
        component_error[component] = meter.absolute_rational(&delta, STAGE)?;
        if meter.compare_rational(
            &component_error[component],
            &component_margin[component],
            STAGE,
        )? == Ordering::Greater
        {
            violations.record(SharedHingeTopologyMarginRelationV1::CorridorBoundary)?;
        }
    }
    if work.corridor_component_scans != CORRIDOR_COMPONENT_SCANS {
        return Err(CayleyError::InvariantFailure { stage: STAGE });
    }
    Ok((component_error, component_margin))
}

fn squared_length_delta_margin(
    exact_axis: &ExactVector3,
    axis_component_margin: &BigRational,
    meter: &mut WorkMeter<'_>,
) -> Result<BigRational, CayleyError> {
    let two = BigRational::from_integer(2.into());
    let margin_squared =
        meter.multiply_rational(axis_component_margin, axis_component_margin, STAGE)?;
    let mut total = BigRational::zero();
    for component in &exact_axis.coordinates {
        let magnitude = meter.absolute_rational(component, STAGE)?;
        let doubled = meter.multiply_rational(&magnitude, &two, STAGE)?;
        let linear = meter.multiply_rational(&doubled, axis_component_margin, STAGE)?;
        let term = meter.add_rational(&linear, &margin_squared, STAGE)?;
        total = meter.add_rational(&total, &term, STAGE)?;
    }
    Ok(total)
}

fn normal_dot_delta_margin(
    exact_left: &ExactVector3,
    exact_right: &ExactVector3,
    normal_margin: &BigRational,
    meter: &mut WorkMeter<'_>,
) -> Result<BigRational, CayleyError> {
    let margin_squared = meter.multiply_rational(normal_margin, normal_margin, STAGE)?;
    let mut dot_margin = BigRational::zero();
    for axis in 0..3 {
        let left_magnitude = meter.absolute_rational(&exact_left.coordinates[axis], STAGE)?;
        let right_magnitude = meter.absolute_rational(&exact_right.coordinates[axis], STAGE)?;
        let left_term = meter.multiply_rational(&left_magnitude, normal_margin, STAGE)?;
        let right_term = meter.multiply_rational(&right_magnitude, normal_margin, STAGE)?;
        let linear = meter.add_rational(&left_term, &right_term, STAGE)?;
        let term = meter.add_rational(&linear, &margin_squared, STAGE)?;
        dot_margin = meter.add_rational(&dot_margin, &term, STAGE)?;
    }
    let two = BigRational::from_integer(2.into());
    meter.divide_rational(&dot_margin, &two, STAGE)
}

fn point_bounds<const N: usize>(
    points: &[ExactPoint3; N],
    meter: &mut WorkMeter<'_>,
) -> Result<[ExactPoint3; 2], CayleyError> {
    let first = points
        .first()
        .ok_or(CayleyError::InvariantFailure { stage: STAGE })?;
    let mut minimum = clone_point(first, meter)?;
    let mut maximum = clone_point(first, meter)?;
    for point in &points[1..] {
        for axis in 0..3 {
            if meter.compare_rational(
                &point.coordinates[axis],
                &minimum.coordinates[axis],
                STAGE,
            )? == Ordering::Less
            {
                minimum.coordinates[axis] =
                    meter.clone_rational(&point.coordinates[axis], STAGE)?;
            }
            if meter.compare_rational(
                &point.coordinates[axis],
                &maximum.coordinates[axis],
                STAGE,
            )? == Ordering::Greater
            {
                maximum.coordinates[axis] =
                    meter.clone_rational(&point.coordinates[axis], STAGE)?;
            }
        }
    }
    Ok([minimum, maximum])
}

fn clone_topology_geometry(
    geometry: &SharedHingeNativeExactTopologyMarginGeometryV1,
    meter: &mut WorkMeter<'_>,
) -> Result<SharedHingeNativeExactTopologyMarginGeometryV1, CayleyError> {
    Ok(SharedHingeNativeExactTopologyMarginGeometryV1 {
        source_endpoints: clone_point_array(&geometry.source_endpoints, meter)?,
        exact_endpoints: clone_point_array(&geometry.exact_endpoints, meter)?,
        direct_endpoint_occurrences: [
            clone_point_array(&geometry.direct_endpoint_occurrences[0], meter)?,
            clone_point_array(&geometry.direct_endpoint_occurrences[1], meter)?,
        ],
        source_halfspace_support: clone_rational_array(&geometry.source_halfspace_support, meter)?,
        exact_frame_determinants: clone_rational_array(&geometry.exact_frame_determinants, meter)?,
        direct_frame_determinants: clone_rational_array(
            &geometry.direct_frame_determinants,
            meter,
        )?,
        exact_face_normals: clone_vector_array(&geometry.exact_face_normals, meter)?,
        direct_face_normals: clone_vector_array(&geometry.direct_face_normals, meter)?,
        exact_solid_bounds: [
            clone_point_array(&geometry.exact_solid_bounds[0], meter)?,
            clone_point_array(&geometry.exact_solid_bounds[1], meter)?,
        ],
        direct_solid_bounds: [
            clone_point_array(&geometry.direct_solid_bounds[0], meter)?,
            clone_point_array(&geometry.direct_solid_bounds[1], meter)?,
        ],
        exact_axis_start: clone_point(&geometry.exact_axis_start, meter)?,
        exact_axis: clone_vector(&geometry.exact_axis, meter)?,
        exact_length_squared: meter.clone_rational(&geometry.exact_length_squared, STAGE)?,
        direct_axis_start: clone_point(&geometry.direct_axis_start, meter)?,
        direct_axis: clone_vector(&geometry.direct_axis, meter)?,
        direct_length_squared: meter.clone_rational(&geometry.direct_length_squared, STAGE)?,
        half_thickness_mm: meter.clone_rational(&geometry.half_thickness_mm, STAGE)?,
        relative_margin: meter.clone_rational(&geometry.relative_margin, STAGE)?,
        local_scale_mm: meter.clone_rational(&geometry.local_scale_mm, STAGE)?,
        point_margin_mm: meter.clone_rational(&geometry.point_margin_mm, STAGE)?,
        normal_margin: meter.clone_rational(&geometry.normal_margin, STAGE)?,
        solid_margin_mm: meter.clone_rational(&geometry.solid_margin_mm, STAGE)?,
        observed_shared_endpoint_component_error_mm: clone_rational_array(
            &geometry.observed_shared_endpoint_component_error_mm,
            meter,
        )?,
        observed_point_component_error_mm: clone_rational_array(
            &geometry.observed_point_component_error_mm,
            meter,
        )?,
        observed_normal_component_error: clone_rational_array(
            &geometry.observed_normal_component_error,
            meter,
        )?,
        observed_solid_component_error_mm: clone_rational_array(
            &geometry.observed_solid_component_error_mm,
            meter,
        )?,
        corridor_component_error: clone_rational_array(&geometry.corridor_component_error, meter)?,
        corridor_component_margin: clone_rational_array(
            &geometry.corridor_component_margin,
            meter,
        )?,
        source_axially_bounded: geometry.source_axially_bounded,
        exact_axially_bounded: geometry.exact_axially_bounded,
        direct_axially_bounded: geometry.direct_axially_bounded,
    })
}

fn clone_rational_array<const N: usize>(
    values: &[BigRational; N],
    meter: &mut WorkMeter<'_>,
) -> Result<[BigRational; N], CayleyError> {
    let mut output = Vec::with_capacity(N);
    for value in values {
        output.push(meter.clone_rational(value, STAGE)?);
    }
    output
        .try_into()
        .map_err(|_| CayleyError::InvariantFailure { stage: STAGE })
}

fn clone_point_array<const N: usize>(
    points: &[ExactPoint3; N],
    meter: &mut WorkMeter<'_>,
) -> Result<[ExactPoint3; N], CayleyError> {
    let mut output = Vec::with_capacity(N);
    for point in points {
        output.push(clone_point(point, meter)?);
    }
    output
        .try_into()
        .map_err(|_| CayleyError::InvariantFailure { stage: STAGE })
}

fn clone_vector_array<const N: usize>(
    vectors: &[ExactVector3; N],
    meter: &mut WorkMeter<'_>,
) -> Result<[ExactVector3; N], CayleyError> {
    let mut output = Vec::with_capacity(N);
    for vector in vectors {
        output.push(clone_vector(vector, meter)?);
    }
    output
        .try_into()
        .map_err(|_| CayleyError::InvariantFailure { stage: STAGE })
}

fn clone_point(point: &ExactPoint3, meter: &mut WorkMeter<'_>) -> Result<ExactPoint3, CayleyError> {
    Ok(ExactPoint3 {
        coordinates: try_array3(|axis| meter.clone_rational(&point.coordinates[axis], STAGE))?,
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

fn capture_face_transform(
    bound: BoundMaterialTreePose<'_>,
    face: FaceId,
) -> Option<BoundBinary64FaceTransformBits> {
    bound
        .pose()
        .face_transform(face)
        .map(|transform| capture_transform_bits(face, transform))
}

fn capture_hinge_parent_transform(
    bound: BoundMaterialTreePose<'_>,
    edge: EdgeId,
    parent: FaceId,
) -> Option<BoundBinary64FaceTransformBits> {
    bound
        .pose()
        .hinge_parent_transform(edge)
        .map(|transform| capture_transform_bits(parent, transform))
}

fn charge_fixed_work(
    work: &mut SharedHingeNativeExactTopologyMarginWorkV1,
    limits: &SharedHingeNativeExactTopologyMarginLimitsV1,
) -> Result<(), CayleyError> {
    for (counter, required, maximum, resource) in [
        (
            &mut work.authenticated_faces,
            FACE_COUNT,
            limits.max_authenticated_faces,
            "topology_margin_authenticated_faces",
        ),
        (
            &mut work.authenticated_hinges,
            HINGE_COUNT,
            limits.max_authenticated_hinges,
            "topology_margin_authenticated_hinges",
        ),
        (
            &mut work.prerequisite_revalidations,
            PREREQUISITE_REVALIDATIONS,
            limits.max_prerequisite_revalidations,
            "topology_margin_prerequisite_revalidations",
        ),
        (
            &mut work.ef_boundary_revalidations,
            EF_BOUNDARY_REVALIDATIONS,
            limits.max_ef_boundary_revalidations,
            "topology_margin_ef_boundary_revalidations",
        ),
        (
            &mut work.root_bindings,
            ROOT_BINDINGS,
            limits.max_root_bindings,
            "topology_margin_root_bindings",
        ),
        (
            &mut work.angle_bindings,
            ANGLE_BINDINGS,
            limits.max_angle_bindings,
            "topology_margin_angle_bindings",
        ),
        (
            &mut work.face_identity_bindings,
            FACE_IDENTITY_BINDINGS,
            limits.max_face_identity_bindings,
            "topology_margin_face_identities",
        ),
        (
            &mut work.hinge_identity_bindings,
            HINGE_IDENTITY_BINDINGS,
            limits.max_hinge_identity_bindings,
            "topology_margin_hinge_identities",
        ),
        (
            &mut work.endpoint_identity_bindings,
            ENDPOINT_IDENTITY_BINDINGS,
            limits.max_endpoint_identity_bindings,
            "topology_margin_endpoint_identities",
        ),
        (
            &mut work.source_boundary_occurrences,
            SOURCE_BOUNDARY_OCCURRENCES,
            limits.max_source_boundary_occurrences,
            "topology_margin_source_boundary_occurrences",
        ),
        (
            &mut work.source_coordinate_lifts,
            SOURCE_COORDINATE_LIFTS,
            limits.max_source_coordinate_lifts,
            "topology_margin_source_coordinate_lifts",
        ),
        (
            &mut work.transform_scalar_lifts,
            TRANSFORM_SCALAR_LIFTS,
            limits.max_transform_scalar_lifts,
            "topology_margin_transform_scalar_lifts",
        ),
        (
            &mut work.face_transform_bit_bindings,
            FACE_TRANSFORM_BIT_BINDINGS,
            limits.max_face_transform_bit_bindings,
            "topology_margin_face_transform_bits",
        ),
        (
            &mut work.hinge_parent_transform_bit_bindings,
            HINGE_PARENT_TRANSFORM_BIT_BINDINGS,
            limits.max_hinge_parent_transform_bit_bindings,
            "topology_margin_hinge_parent_transform_bits",
        ),
        (
            &mut work.mid_surface_reconstructions,
            MID_SURFACE_RECONSTRUCTIONS,
            limits.max_mid_surface_reconstructions,
            "topology_margin_mid_surface_reconstructions",
        ),
        (
            &mut work.normal_reconstructions,
            NORMAL_RECONSTRUCTIONS,
            limits.max_normal_reconstructions,
            "topology_margin_normal_reconstructions",
        ),
        (
            &mut work.solid_vertex_constructions,
            SOLID_VERTEX_CONSTRUCTIONS,
            limits.max_solid_vertex_constructions,
            "topology_margin_solid_vertex_constructions",
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

fn live_face_topology(
    exact: &RationalCayleyTreePose<'_>,
    bound: BoundMaterialTreePose<'_>,
    face_index: usize,
    hinge_edge: EdgeId,
    endpoint_vertices: [VertexId; 2],
) -> Option<BoundFaceTopologyIdentityV1> {
    let exact_face = exact.faces.get(face_index)?;
    let boundary = bound.face_boundary(exact_face.face).filter(|boundary| {
        bound.model().owns_face_boundary(*boundary)
            && bound.pose().owns_face_boundary(*boundary)
            && boundary.vertices().len() == VERTICES_PER_FACE
            && boundary.edges().len() == VERTICES_PER_FACE
    })?;
    let boundary_vertices: [VertexId; VERTICES_PER_FACE] = boundary.vertices().try_into().ok()?;
    let boundary_edges: [EdgeId; VERTICES_PER_FACE] = boundary.edges().try_into().ok()?;
    if !exact_face
        .boundary
        .iter()
        .map(|(vertex, _)| *vertex)
        .eq(boundary_vertices)
    {
        return None;
    }
    let hinge_occurrence = unique_edge_occurrence(&boundary_edges, hinge_edge)?;
    let start_index = unique_vertex_occurrence(&boundary_vertices, endpoint_vertices[0])?;
    let end_index = unique_vertex_occurrence(&boundary_vertices, endpoint_vertices[1])?;
    let opposite_index =
        (0..VERTICES_PER_FACE).find(|index| *index != start_index && *index != end_index)?;
    Some(BoundFaceTopologyIdentityV1 {
        face: exact_face.face,
        boundary_vertices,
        boundary_edges,
        hinge_occurrence,
        opposite_vertex: boundary_vertices[opposite_index],
    })
}

/// Rebinds every upstream token, topology identity, pose/root/angle binding,
/// transform coefficient, issuance seal, and work seal.
#[allow(clippy::too_many_arguments)]
pub(super) fn revalidate_shared_hinge_native_exact_topology_margin_v1<
    'capability,
    'prerequisite,
    'ef,
    'exact,
    'pose,
>(
    capability: &'capability SharedHingeNativeExactTopologyMarginCapabilityV1<
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
    RevalidatedSharedHingeNativeExactTopologyMarginCapabilityV1<
        'capability,
        'prerequisite,
        'ef,
        'exact,
        'pose,
    >,
> {
    let exact_hinge = exact.hinges.first()?;
    let native_hinge = bound.model().hinges().first()?;
    let native_angles = bound.pose().hinge_angles();
    let native_angle = native_angles.first().copied()?;
    let live_face_topology = [
        live_face_topology(
            exact,
            bound,
            0,
            exact_hinge.edge,
            exact_hinge.endpoint_vertices,
        )?,
        live_face_topology(
            exact,
            bound,
            1,
            exact_hinge.edge,
            exact_hinge.endpoint_vertices,
        )?,
    ];
    let live_face_transforms = [
        capture_face_transform(bound, exact.faces.first()?.face)?,
        capture_face_transform(bound, exact.faces.get(1)?.face)?,
    ];
    let live_hinge_parent_transform =
        capture_hinge_parent_transform(bound, exact_hinge.edge, exact_hinge.parent)?;
    if !positive_finite_binary64(paper_thickness_mm)
        || capability.sealed_work.is_none()
        || capability.sealed_geometry.as_ref() != Some(&capability.geometry)
        || !std::ptr::eq(capability.prerequisite, prerequisite)
        || !std::ptr::eq(capability.ef_boundary, ef_boundary)
        || !std::ptr::eq(capability.exact, exact)
        || capability.paper_thickness_bits != paper_thickness_mm.to_bits()
        || capability.bound.model() != bound.model()
        || !capability.bound.pose().same_instance(bound.pose())
        || !exact.is_for(bound)
        || exact.version != RATIONAL_CAYLEY_TREE_POSE_V1
        || exact.faces.len() != FACE_COUNT
        || exact.hinges.len() != HINGE_COUNT
        || native_angles.len() != HINGE_COUNT
        || exact.fixed_face != Some(capability.fixed_face)
        || bound.pose().fixed_face() != Some(capability.fixed_face)
        || capability.face_topology != live_face_topology
        || capability.hinge_edge != exact_hinge.edge
        || capability.hinge_edge != native_hinge.edge()
        || capability.hinge_parent != exact_hinge.parent
        || capability.hinge_child != exact_hinge.child
        || capability.hinge_endpoint_vertices != exact_hinge.endpoint_vertices
        || capability.hinge_angle.edge != native_angle.edge()
        || capability.hinge_angle.angle_degrees_bits != native_angle.angle_degrees().to_bits()
        || capability.hinge_angle.angle_degrees_bits != exact_hinge.angle_magnitude_bits
        || capability.binary64_face_transforms != live_face_transforms
        || capability.binary64_face_transforms != ef_boundary.binary64_face_transforms
        || capability.hinge_parent_transform != live_hinge_parent_transform
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
    Some(RevalidatedSharedHingeNativeExactTopologyMarginCapabilityV1 { capability })
}

#[cfg(test)]
fn scalar_count(geometry: &SharedHingeNativeExactTopologyMarginGeometryV1) -> usize {
    let mut clone = geometry.clone();
    let mut count = 0_usize;
    visit_geometry_scalars_mut(&mut clone, &mut |_| count += 1);
    count
}

#[cfg(test)]
fn visit_point_scalars_mut(point: &mut ExactPoint3, visit: &mut impl FnMut(&mut BigRational)) {
    for coordinate in &mut point.coordinates {
        visit(coordinate);
    }
}

#[cfg(test)]
fn visit_vector_scalars_mut(vector: &mut ExactVector3, visit: &mut impl FnMut(&mut BigRational)) {
    for coordinate in &mut vector.coordinates {
        visit(coordinate);
    }
}

#[cfg(test)]
fn visit_geometry_scalars_mut(
    geometry: &mut SharedHingeNativeExactTopologyMarginGeometryV1,
    visit: &mut impl FnMut(&mut BigRational),
) {
    for point in &mut geometry.source_endpoints {
        visit_point_scalars_mut(point, visit);
    }
    for point in &mut geometry.exact_endpoints {
        visit_point_scalars_mut(point, visit);
    }
    for face in &mut geometry.direct_endpoint_occurrences {
        for point in face {
            visit_point_scalars_mut(point, visit);
        }
    }
    for value in &mut geometry.source_halfspace_support {
        visit(value);
    }
    for value in &mut geometry.exact_frame_determinants {
        visit(value);
    }
    for value in &mut geometry.direct_frame_determinants {
        visit(value);
    }
    for normal in &mut geometry.exact_face_normals {
        visit_vector_scalars_mut(normal, visit);
    }
    for normal in &mut geometry.direct_face_normals {
        visit_vector_scalars_mut(normal, visit);
    }
    for face in &mut geometry.exact_solid_bounds {
        for point in face {
            visit_point_scalars_mut(point, visit);
        }
    }
    for face in &mut geometry.direct_solid_bounds {
        for point in face {
            visit_point_scalars_mut(point, visit);
        }
    }
    visit_point_scalars_mut(&mut geometry.exact_axis_start, visit);
    visit_vector_scalars_mut(&mut geometry.exact_axis, visit);
    visit(&mut geometry.exact_length_squared);
    visit_point_scalars_mut(&mut geometry.direct_axis_start, visit);
    visit_vector_scalars_mut(&mut geometry.direct_axis, visit);
    visit(&mut geometry.direct_length_squared);
    for value in [
        &mut geometry.half_thickness_mm,
        &mut geometry.relative_margin,
        &mut geometry.local_scale_mm,
        &mut geometry.point_margin_mm,
        &mut geometry.normal_margin,
        &mut geometry.solid_margin_mm,
    ] {
        visit(value);
    }
    for values in [
        &mut geometry.observed_shared_endpoint_component_error_mm,
        &mut geometry.observed_point_component_error_mm,
        &mut geometry.observed_normal_component_error,
        &mut geometry.observed_solid_component_error_mm,
    ] {
        for value in values {
            visit(value);
        }
    }
    for value in &mut geometry.corridor_component_error {
        visit(value);
    }
    for value in &mut geometry.corridor_component_margin {
        visit(value);
    }
}
