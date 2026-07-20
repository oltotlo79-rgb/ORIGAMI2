//! Blocking-only robust-transversal primitive in actual millimetres.
//!
//! This checkpoint deliberately accepts only intervals expressed in the
//! rational Cayley tree's actual-mm coordinate system.  The zero-thickness
//! classifier's binary64 common-unit coordinates are scaled by `2^1074` and
//! must never enter this API. The private authority bridge below binds the
//! canonical exact pose, the exact object measured by its affine envelope,
//! and the same issuer-bound binary64 pose. It requires a transversal witness
//! both in exact `E` and in an exact rational lift of the actual binary64
//! affine image. Per-face radius boxes authenticate containment only; they
//! are never welded into invented shared points. Positive paper thickness,
//! coplanar/180-degree overlap and finite hinges remain unresolved.
//!
//! The only positive result is a transversal penetration.  Touching,
//! separation, coplanarity, shared-hinge contact, numeric uncertainty and
//! every resource failure all collapse to `Unresolved`.  Consequently this
//! type cannot widen any collision-free set or issue a public geometry proof.

use std::{cmp::Ordering, collections::HashMap};

use num_rational::BigRational;
use num_traits::{One, Signed};
use ori_domain::{FaceId, VertexId};
use ori_kinematics::BoundMaterialTreePose;

use super::{
    super::{
        CayleyError, CayleyLimits, CayleyStage, CayleyWork, ExactFacePose, ExactRigidTransform,
        ExactTreePoseLimits, ExactVector3, RATIONAL_CAYLEY_TREE_POSE_V1, RationalCayleyTreePose,
        TotalTermLimits, WorkMeter, apply_exact_transform, canonical_point_eq, exact_f64,
        exact_point, point3_array, prepare_rational_cayley_tree_pose_v1,
    },
    MeasuredBinary64AffineEnvelope, MeasuredEnvelopeError, MeasuredEnvelopeLimits,
    MeasuredFaceEnvelope, measure_binary64_affine_envelope,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BlockingOnlyDecision {
    ProvenPenetrating,
    Unresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PairTopology {
    NoSharedFeature,
    SharedVertex {
        first_vertex: usize,
        second_vertex: usize,
    },
    SharedHinge,
    SameFace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CayleyResumeLimits {
    max_machin_terms_per_series: usize,
    max_trig_terms_per_series: usize,
    max_sqrt_refinements: usize,
    max_interval_operations: usize,
    max_shift_bits: usize,
    max_intermediate_bits: usize,
    max_gcd_fallback_calls: usize,
    max_gcd_fallback_input_bits: usize,
    max_rational_allocations: usize,
    max_rational_allocation_bits: usize,
    max_total_rational_allocation_bits: usize,
    max_output_bits: usize,
}

impl CayleyResumeLimits {
    fn as_cayley_limits(self) -> CayleyLimits {
        CayleyLimits {
            // A resumed containment meter is intentionally incapable of
            // issuing a new trigonometric candidate. The referenced exact
            // pose already passed these issuer-only controls while it was
            // created by the private tree-pose issuer.
            max_precision_rounds: 0,
            max_guard_bits: 0,
            max_candidate_bits: 0,
            max_machin_terms_per_series: self.max_machin_terms_per_series,
            max_trig_terms_per_series: self.max_trig_terms_per_series,
            max_sqrt_refinements: self.max_sqrt_refinements,
            max_interval_operations: self.max_interval_operations,
            max_shift_bits: self.max_shift_bits,
            max_intermediate_bits: self.max_intermediate_bits,
            max_gcd_fallback_calls: self.max_gcd_fallback_calls,
            max_gcd_fallback_input_bits: self.max_gcd_fallback_input_bits,
            max_rational_allocations: self.max_rational_allocations,
            max_rational_allocation_bits: self.max_rational_allocation_bits,
            max_total_rational_allocation_bits: self.max_total_rational_allocation_bits,
            max_output_bits: self.max_output_bits,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AuthenticatedTriangleBlockingLimits {
    max_faces: usize,
    max_hinges: usize,
    max_boundary_occurrences: usize,
    max_triangular_faces: usize,
    max_unordered_face_pairs: usize,
    max_triangular_face_pairs: usize,
    max_topology_vertex_checks: usize,
    max_hinge_boundary_index_entries: usize,
    max_hinge_face_lookups: usize,
    max_hinge_vertex_lookups: usize,
    max_predicate_calls: usize,
    max_result_records: usize,
    max_total_machin_terms: usize,
    max_total_trig_terms: usize,
    max_total_sqrt_refinements: usize,
    exact: CayleyResumeLimits,
}

const AUTHENTICATED_TRIANGLE_BLOCKING_HARD_LIMITS: AuthenticatedTriangleBlockingLimits =
    AuthenticatedTriangleBlockingLimits {
        max_faces: 10_001,
        max_hinges: 10_000,
        max_boundary_occurrences: 1_000_000,
        max_triangular_faces: 50_000,
        max_unordered_face_pairs: 1_000_000,
        max_triangular_face_pairs: 1_000_000,
        max_topology_vertex_checks: 9_000_000,
        max_hinge_boundary_index_entries: 1_000_000,
        max_hinge_face_lookups: 20_000,
        max_hinge_vertex_lookups: 40_000,
        max_predicate_calls: 2_000_000,
        max_result_records: 1_000_000,
        max_total_machin_terms: 4_000_000,
        max_total_trig_terms: 8_000_000,
        max_total_sqrt_refinements: 640_000,
        exact: CayleyResumeLimits {
            max_machin_terms_per_series: 2_048,
            max_trig_terms_per_series: 2_048,
            max_sqrt_refinements: 32,
            max_interval_operations: 80_000_000,
            max_shift_bits: 8_192,
            max_intermediate_bits: 32_768,
            max_gcd_fallback_calls: 1_000_000,
            max_gcd_fallback_input_bits: 2_147_483_648,
            max_rational_allocations: 3_000_000,
            max_rational_allocation_bits: 65_536,
            max_total_rational_allocation_bits: 2_500_000_000,
            max_output_bits: 16_384,
        },
    };

impl Default for AuthenticatedTriangleBlockingLimits {
    fn default() -> Self {
        AUTHENTICATED_TRIANGLE_BLOCKING_HARD_LIMITS
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct AuthenticatedTriangleBlockingWork {
    faces: usize,
    hinges: usize,
    boundary_occurrences: usize,
    triangular_faces: usize,
    unordered_face_pairs: usize,
    expected_triangular_face_pairs: usize,
    analyzed_triangular_face_pairs: usize,
    topology_vertex_checks: usize,
    hinge_boundary_index_entries: usize,
    hinge_relation_records: usize,
    hinge_face_lookups: usize,
    hinge_vertex_lookups: usize,
    exact_predicate_calls: usize,
    observed_predicate_calls: usize,
    unsupported_pair_records: usize,
    result_records: usize,
    exact: CayleyWork,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum AuthenticatedTriangleBlockingError {
    UnsupportedPaperThickness,
    AuthorityMismatch,
    InconsistentPose,
    ResourceLimitExceeded { resource: &'static str },
    Exact(CayleyError),
}

impl From<CayleyError> for AuthenticatedTriangleBlockingError {
    fn from(value: CayleyError) -> Self {
        Self::Exact(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AuthenticatedTrianglePairDecision {
    first: FaceId,
    second: FaceId,
    decision: BlockingOnlyDecision,
}

/// Sealed blocking diagnostics for one exact pose/envelope object pair.
///
/// This private value intentionally carries the issuing objects and bit-exact
/// paper-thickness input. It cannot construct a public collision-free proof,
/// and every unsupported or uncertain pair remains `Unresolved`.
#[derive(Debug)]
struct AuthenticatedTriangleBlockingScan<'scan, 'exact, 'pose> {
    exact: &'scan RationalCayleyTreePose<'pose>,
    measured: &'scan MeasuredBinary64AffineEnvelope<'exact, 'pose>,
    bound: BoundMaterialTreePose<'pose>,
    paper_thickness_bits: u64,
    pairs: Vec<AuthenticatedTrianglePairDecision>,
    work: AuthenticatedTriangleBlockingWork,
}

impl AuthenticatedTriangleBlockingScan<'_, '_, '_> {
    fn decision(&self, first: FaceId, second: FaceId) -> Option<BlockingOnlyDecision> {
        let (first, second) = canonical_face_pair(first, second)?;
        self.pairs
            .binary_search_by(|record| {
                record
                    .first
                    .canonical_bytes()
                    .cmp(&first.canonical_bytes())
                    .then_with(|| {
                        record
                            .second
                            .canonical_bytes()
                            .cmp(&second.canonical_bytes())
                    })
            })
            .ok()
            .and_then(|index| self.pairs.get(index))
            .map(|record| record.decision)
    }

    fn proven_penetrating_pairs(&self) -> usize {
        self.pairs
            .iter()
            .filter(|record| record.decision == BlockingOnlyDecision::ProvenPenetrating)
            .count()
    }

    fn is_for(
        &self,
        exact: &RationalCayleyTreePose<'_>,
        measured: &MeasuredBinary64AffineEnvelope<'_, '_>,
        bound: BoundMaterialTreePose<'_>,
        paper_thickness_mm: f64,
    ) -> bool {
        std::ptr::eq(self.exact, exact)
            && std::ptr::eq(self.measured, measured)
            && self.exact.is_for(bound)
            && self.measured.is_for(exact, bound)
            && self.bound.model() == bound.model()
            && self.bound.pose().same_instance(bound.pose())
            && self.paper_thickness_bits == paper_thickness_mm.to_bits()
    }
}

/// Owned, crate-private result of the zero-thickness transversal blocking scan.
///
/// No exact pose, measured envelope, pair decision record, or borrowed
/// authority token crosses this boundary. A positive count therefore only
/// reports the blocking-only dual-gate result; it cannot be reused as a
/// collision-free proof.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProvenTransversalScanSummary {
    pub(crate) enumerated_pairs: usize,
    pub(crate) proven_transversal_pairs: usize,
    pub(crate) first_proven_transversal_pair: Option<(FaceId, FaceId)>,
    pub(crate) proven_transversal_pair_ids: Vec<(FaceId, FaceId)>,
}

impl ProvenTransversalScanSummary {
    pub(crate) fn proves_pair(&self, first: FaceId, second: FaceId) -> bool {
        let Some((first, second)) = canonical_face_pair(first, second) else {
            return false;
        };
        self.proven_transversal_pair_ids
            .binary_search_by(|pair| {
                pair.0
                    .canonical_bytes()
                    .cmp(&first.canonical_bytes())
                    .then_with(|| pair.1.canonical_bytes().cmp(&second.canonical_bytes()))
            })
            .is_ok()
    }
}

/// Coarse failure classes for the sealed zero-thickness transversal scan.
///
/// Keeping this enum fieldless prevents exact-predicate internals and
/// resource-accounting details from becoming a caller-controlled proof API.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProvenTransversalScanError {
    EvidenceUnavailable,
    ResourceLimitExceeded,
    InconsistentPose,
}

/// Caller budget shared by the public static scan and this private bridge.
///
/// These fields intentionally mirror only resource classes common to both
/// kernels. The bridge projects them onto every exact-tree, containment and
/// blocking counter, while retaining each stage's private hard default as an
/// unexpandable ceiling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ProvenTransversalScanLimits {
    pub(crate) max_faces: usize,
    pub(crate) max_unordered_face_pairs: usize,
    pub(crate) max_boundary_vertices_per_face: usize,
    pub(crate) max_total_boundary_vertices: usize,
    pub(crate) max_total_triangles: usize,
    pub(crate) max_total_triangle_pairs: usize,
    pub(crate) max_registry_authentication_work: usize,
    pub(crate) max_total_boundary_relation_work: usize,
    pub(crate) max_rational_input_bits: usize,
    pub(crate) max_total_rational_input_storage_bits: usize,
    pub(crate) max_total_rational_retained_clone_bits: usize,
    pub(crate) max_rational_operations: usize,
    pub(crate) max_rational_intermediate_bits: usize,
    pub(crate) max_rational_gcd_fallback_calls: usize,
    pub(crate) max_rational_gcd_fallback_input_bits: usize,
    pub(crate) max_rational_allocations: usize,
    pub(crate) max_rational_allocation_bits: usize,
    pub(crate) max_total_rational_allocation_bits: usize,
    pub(crate) max_rational_output_bits: usize,
    pub(crate) max_total_rational_output_bits: usize,
}

impl Default for ProvenTransversalScanLimits {
    fn default() -> Self {
        Self {
            max_faces: 10_001,
            max_unordered_face_pairs: 1_000_000,
            max_boundary_vertices_per_face: 1_000_000,
            max_total_boundary_vertices: 1_000_000,
            max_total_triangles: 50_000,
            max_total_triangle_pairs: 1_000_000,
            max_registry_authentication_work: 50_005_000,
            max_total_boundary_relation_work: 9_000_000,
            max_rational_input_bits: 16_384,
            max_total_rational_input_storage_bits: 256_000_000,
            max_total_rational_retained_clone_bits: 2_500_000_000,
            max_rational_operations: 80_000_000,
            max_rational_intermediate_bits: 32_768,
            max_rational_gcd_fallback_calls: 1_000_000,
            max_rational_gcd_fallback_input_bits: 2_147_483_648,
            max_rational_allocations: 3_000_000,
            max_rational_allocation_bits: 65_536,
            max_total_rational_allocation_bits: 2_500_000_000,
            max_rational_output_bits: 16_384,
            max_total_rational_output_bits: 512_000_000,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ProjectedProvenTransversalScanLimits {
    shared: ProvenTransversalScanLimits,
    exact: ExactTreePoseLimits,
    measured: MeasuredEnvelopeLimits,
    blocking: AuthenticatedTriangleBlockingLimits,
    max_boundary_vertices_per_face: usize,
}

/// Runs the complete authenticated zero-thickness transversal bridge.
///
/// The exact Cayley pose, its pointer-bound binary64 envelope, and the
/// authenticated dual-gate scan coexist only in this borrow scope. The paper
/// thickness is the bit-exact positive zero literal. Caller limits may shrink
/// every shared resource class, but private hard defaults remain unexpandable.
pub(crate) fn scan_bound_pose_for_proven_transversal_penetration(
    bound: BoundMaterialTreePose<'_>,
    limits: ProvenTransversalScanLimits,
) -> Result<ProvenTransversalScanSummary, ProvenTransversalScanError> {
    let limits = project_proven_transversal_scan_limits(limits)?;
    preflight_proven_transversal_shape(
        bound,
        limits.exact.max_faces,
        limits.blocking.max_hinges,
        limits.blocking.max_unordered_face_pairs,
        limits.max_boundary_vertices_per_face,
        limits.exact.max_boundary_occurrences,
        limits.blocking.max_triangular_faces,
        limits.blocking.max_triangular_face_pairs,
        limits.shared.max_registry_authentication_work,
        limits.shared.max_total_boundary_relation_work,
    )?;
    let exact =
        prepare_rational_cayley_tree_pose_v1(bound, limits.exact).map_err(map_cayley_scan_error)?;
    let measured_limits = measured_limits_after_exact(limits.measured, &limits.shared, &exact)?;
    let measured = measure_binary64_affine_envelope(&exact, bound, measured_limits)
        .map_err(map_measured_scan_error)?;
    let scan = scan_authenticated_triangle_pairs_blocking_only(
        &exact,
        &measured,
        bound,
        0.0_f64,
        limits.blocking,
    )
    .map_err(map_authenticated_scan_error)?;

    let proven_transversal_pair_ids = scan
        .pairs
        .iter()
        .filter(|pair| pair.decision == BlockingOnlyDecision::ProvenPenetrating)
        .map(|pair| (pair.first, pair.second))
        .collect::<Vec<_>>();
    let first_proven_transversal_pair = proven_transversal_pair_ids.first().copied();
    Ok(ProvenTransversalScanSummary {
        enumerated_pairs: scan.pairs.len(),
        proven_transversal_pairs: proven_transversal_pair_ids.len(),
        first_proven_transversal_pair,
        proven_transversal_pair_ids,
    })
}

fn project_proven_transversal_scan_limits(
    requested: ProvenTransversalScanLimits,
) -> Result<ProjectedProvenTransversalScanLimits, ProvenTransversalScanError> {
    let hard_shared = ProvenTransversalScanLimits::default();
    let capped_input_storage_bits = requested
        .max_total_rational_input_storage_bits
        .min(hard_shared.max_total_rational_input_storage_bits);
    let capped_retained_clone_bits = requested
        .max_total_rational_retained_clone_bits
        .min(hard_shared.max_total_rational_retained_clone_bits);
    let capped_allocation_bits = requested
        .max_total_rational_allocation_bits
        .min(hard_shared.max_total_rational_allocation_bits);
    let capped_output_bits = requested
        .max_total_rational_output_bits
        .min(hard_shared.max_total_rational_output_bits);
    let requested_payload_bits = capped_input_storage_bits
        .min(capped_retained_clone_bits)
        .min(capped_allocation_bits)
        .min(capped_output_bits);
    let shared = ProvenTransversalScanLimits {
        max_faces: requested.max_faces.min(hard_shared.max_faces),
        max_unordered_face_pairs: requested
            .max_unordered_face_pairs
            .min(hard_shared.max_unordered_face_pairs),
        max_boundary_vertices_per_face: requested
            .max_boundary_vertices_per_face
            .min(hard_shared.max_boundary_vertices_per_face),
        max_total_boundary_vertices: requested
            .max_total_boundary_vertices
            .min(hard_shared.max_total_boundary_vertices),
        max_total_triangles: requested
            .max_total_triangles
            .min(hard_shared.max_total_triangles),
        max_total_triangle_pairs: requested
            .max_total_triangle_pairs
            .min(hard_shared.max_total_triangle_pairs),
        max_registry_authentication_work: requested
            .max_registry_authentication_work
            .min(hard_shared.max_registry_authentication_work),
        max_total_boundary_relation_work: requested
            .max_total_boundary_relation_work
            .min(hard_shared.max_total_boundary_relation_work),
        max_rational_input_bits: requested
            .max_rational_input_bits
            .min(hard_shared.max_rational_input_bits),
        max_total_rational_input_storage_bits: capped_input_storage_bits
            .min(requested_payload_bits),
        max_total_rational_retained_clone_bits: capped_retained_clone_bits
            .min(requested_payload_bits),
        max_rational_operations: requested
            .max_rational_operations
            .min(hard_shared.max_rational_operations),
        max_rational_intermediate_bits: requested
            .max_rational_intermediate_bits
            .min(hard_shared.max_rational_intermediate_bits),
        max_rational_gcd_fallback_calls: requested
            .max_rational_gcd_fallback_calls
            .min(hard_shared.max_rational_gcd_fallback_calls),
        max_rational_gcd_fallback_input_bits: requested
            .max_rational_gcd_fallback_input_bits
            .min(hard_shared.max_rational_gcd_fallback_input_bits),
        max_rational_allocations: requested
            .max_rational_allocations
            .min(hard_shared.max_rational_allocations),
        max_rational_allocation_bits: requested
            .max_rational_allocation_bits
            .min(hard_shared.max_rational_allocation_bits),
        max_total_rational_allocation_bits: capped_allocation_bits.min(requested_payload_bits),
        max_rational_output_bits: requested
            .max_rational_output_bits
            .min(hard_shared.max_rational_output_bits),
        max_total_rational_output_bits: capped_output_bits.min(requested_payload_bits),
    };
    let hard_exact = ExactTreePoseLimits::default();
    let hard_measured = MeasuredEnvelopeLimits::default();
    let hard_blocking = AUTHENTICATED_TRIANGLE_BLOCKING_HARD_LIMITS;
    let max_hinges = shared
        .max_faces
        .saturating_sub(1)
        .min(hard_exact.max_hinges)
        .min(hard_measured.max_hinges)
        .min(hard_blocking.max_hinges);
    let doubled_hinges = projected_product(max_hinges, 2)?;
    let transform_scalars = projected_product(shared.max_faces, 12)?;
    let point_components = projected_product(shared.max_total_boundary_vertices, 3)?;
    let hinge_feature_points = projected_product(max_hinges, 3)?;
    let hinge_path_checks = projected_product(hinge_feature_points, 3)?;
    let hinge_component_checks = projected_product(hinge_path_checks, 3)?;
    let boundary_point_transforms = projected_product(shared.max_total_boundary_vertices, 2)?;
    let hinge_point_transforms = projected_product(hinge_feature_points, 5)?;
    let exact_point_transforms = projected_sum(boundary_point_transforms, hinge_point_transforms)?;
    let source_scalars = projected_product(shared.max_total_boundary_vertices, 3)?;
    let input_scalars = projected_sum(
        projected_sum(transform_scalars, source_scalars)?,
        max_hinges,
    )?;
    let total_depth = projected_product(shared.max_faces, shared.max_faces.saturating_sub(1))? / 2;
    let topology_vertex_checks = projected_product(shared.max_total_triangle_pairs, 9)?;
    let predicate_calls = projected_product(shared.max_total_triangle_pairs, 2)?;
    let exact = ExactTreePoseLimits {
        max_faces: shared.max_faces.min(hard_exact.max_faces),
        max_hinges,
        max_adjacency_entries: doubled_hinges
            .min(shared.max_registry_authentication_work)
            .min(hard_exact.max_adjacency_entries),
        max_boundary_occurrences: shared
            .max_total_boundary_vertices
            .min(hard_exact.max_boundary_occurrences),
        max_boundary_edge_index_entries: shared
            .max_total_boundary_vertices
            .min(shared.max_registry_authentication_work)
            .min(hard_exact.max_boundary_edge_index_entries),
        max_boundary_edge_index_operations: shared
            .max_registry_authentication_work
            .min(hard_exact.max_boundary_edge_index_operations),
        max_unique_vertices: shared
            .max_total_boundary_vertices
            .min(hard_exact.max_unique_vertices),
        max_total_machin_terms: shared
            .max_rational_operations
            .min(hard_exact.max_total_machin_terms),
        max_total_trig_terms: shared
            .max_rational_operations
            .min(hard_exact.max_total_trig_terms),
        max_total_sqrt_refinements: shared
            .max_rational_operations
            .min(hard_exact.max_total_sqrt_refinements),
        max_total_output_bits: shared
            .max_total_rational_output_bits
            .min(hard_exact.max_total_output_bits),
        cayley: project_cayley_limits(hard_exact.cayley, &shared),
    };
    let measured = MeasuredEnvelopeLimits {
        max_faces: shared.max_faces.min(hard_measured.max_faces),
        max_hinges,
        max_adjacency_entries: max_hinges
            .min(shared.max_registry_authentication_work)
            .min(hard_measured.max_adjacency_entries),
        max_depth: max_hinges.min(hard_measured.max_depth),
        max_total_depth: total_depth
            .min(shared.max_registry_authentication_work)
            .min(hard_measured.max_total_depth),
        max_transform_scalars: transform_scalars
            .min(shared.max_registry_authentication_work)
            .min(hard_measured.max_transform_scalars),
        max_boundary_occurrences: shared
            .max_total_boundary_vertices
            .min(hard_measured.max_boundary_occurrences),
        max_point_components: point_components
            .min(shared.max_registry_authentication_work)
            .min(hard_measured.max_point_components),
        max_unique_vertices: shared
            .max_total_boundary_vertices
            .min(hard_measured.max_unique_vertices),
        max_shared_occurrence_checks: shared
            .max_total_boundary_vertices
            .min(shared.max_registry_authentication_work)
            .min(hard_measured.max_shared_occurrence_checks),
        max_hinge_feature_points: hinge_feature_points
            .min(shared.max_registry_authentication_work)
            .min(hard_measured.max_hinge_feature_points),
        max_exact_hinge_path_checks: hinge_path_checks
            .min(shared.max_registry_authentication_work)
            .min(hard_measured.max_exact_hinge_path_checks),
        max_binary64_hinge_path_checks: hinge_path_checks
            .min(shared.max_registry_authentication_work)
            .min(hard_measured.max_binary64_hinge_path_checks),
        max_hinge_component_checks: hinge_component_checks
            .min(shared.max_registry_authentication_work)
            .min(hard_measured.max_hinge_component_checks),
        max_hinge_transform_checks: max_hinges
            .min(shared.max_registry_authentication_work)
            .min(hard_measured.max_hinge_transform_checks),
        max_certificate_reads: max_hinges
            .min(shared.max_registry_authentication_work)
            .min(hard_measured.max_certificate_reads),
        max_exact_point_transforms: exact_point_transforms
            .min(shared.max_registry_authentication_work)
            .min(hard_measured.max_exact_point_transforms),
        max_input_scalars: input_scalars
            .min(shared.max_registry_authentication_work)
            .min(hard_measured.max_input_scalars),
        max_input_bits: shared
            .max_rational_input_bits
            .min(hard_measured.max_input_bits),
        max_total_input_bits: shared
            .max_total_rational_input_storage_bits
            .min(shared.max_total_rational_retained_clone_bits)
            .min(hard_measured.max_total_input_bits),
        max_output_bits: shared
            .max_rational_output_bits
            .min(hard_measured.max_output_bits),
        max_total_output_bits: shared
            .max_total_rational_output_bits
            .min(shared.max_total_rational_retained_clone_bits)
            .min(hard_measured.max_total_output_bits),
        exact: project_cayley_limits(hard_measured.exact, &shared),
    };
    let blocking = AuthenticatedTriangleBlockingLimits {
        max_faces: shared.max_faces.min(hard_blocking.max_faces),
        max_hinges,
        max_boundary_occurrences: shared
            .max_total_boundary_vertices
            .min(hard_blocking.max_boundary_occurrences),
        max_triangular_faces: shared
            .max_total_triangles
            .min(hard_blocking.max_triangular_faces),
        max_unordered_face_pairs: shared
            .max_unordered_face_pairs
            .min(hard_blocking.max_unordered_face_pairs),
        max_triangular_face_pairs: shared
            .max_total_triangle_pairs
            .min(hard_blocking.max_triangular_face_pairs),
        max_topology_vertex_checks: topology_vertex_checks
            .min(shared.max_total_boundary_relation_work)
            .min(hard_blocking.max_topology_vertex_checks),
        max_hinge_boundary_index_entries: shared
            .max_total_boundary_vertices
            .min(shared.max_registry_authentication_work)
            .min(hard_blocking.max_hinge_boundary_index_entries),
        max_hinge_face_lookups: doubled_hinges
            .min(shared.max_registry_authentication_work)
            .min(hard_blocking.max_hinge_face_lookups),
        max_hinge_vertex_lookups: projected_product(max_hinges, 4)?
            .min(shared.max_registry_authentication_work)
            .min(hard_blocking.max_hinge_vertex_lookups),
        max_predicate_calls: predicate_calls
            .min(shared.max_rational_operations)
            .min(hard_blocking.max_predicate_calls),
        max_result_records: shared
            .max_unordered_face_pairs
            .min(hard_blocking.max_result_records),
        max_total_machin_terms: shared
            .max_rational_operations
            .min(hard_blocking.max_total_machin_terms),
        max_total_trig_terms: shared
            .max_rational_operations
            .min(hard_blocking.max_total_trig_terms),
        max_total_sqrt_refinements: shared
            .max_rational_operations
            .min(hard_blocking.max_total_sqrt_refinements),
        exact: project_cayley_resume_limits(hard_blocking.exact, &shared),
    };
    let max_boundary_vertices_per_face = shared.max_boundary_vertices_per_face;
    Ok(ProjectedProvenTransversalScanLimits {
        shared,
        exact,
        measured,
        blocking,
        max_boundary_vertices_per_face,
    })
}

fn measured_limits_after_exact(
    mut measured: MeasuredEnvelopeLimits,
    shared: &ProvenTransversalScanLimits,
    exact: &RationalCayleyTreePose<'_>,
) -> Result<MeasuredEnvelopeLimits, ProvenTransversalScanError> {
    let exact_work = &exact.work.exact;
    measured.exact.max_interval_operations =
        measured
            .exact
            .max_interval_operations
            .min(checked_remaining(
                shared.max_rational_operations,
                exact_work.interval_operations,
            )?);
    measured.exact.max_gcd_fallback_calls =
        measured.exact.max_gcd_fallback_calls.min(checked_remaining(
            shared.max_rational_gcd_fallback_calls,
            exact_work.gcd_fallback_calls,
        )?);
    measured.exact.max_gcd_fallback_input_bits =
        measured
            .exact
            .max_gcd_fallback_input_bits
            .min(checked_remaining(
                shared.max_rational_gcd_fallback_input_bits,
                exact_work.gcd_fallback_input_bits,
            )?);
    measured.exact.max_rational_allocations =
        measured
            .exact
            .max_rational_allocations
            .min(checked_remaining(
                shared.max_rational_allocations,
                exact_work.rational_allocations,
            )?);
    measured.exact.max_total_rational_allocation_bits = measured
        .exact
        .max_total_rational_allocation_bits
        .min(checked_remaining(
            shared.max_total_rational_allocation_bits,
            exact_work.total_rational_allocation_bits,
        )?);
    let remaining_payload_bits = checked_remaining(
        shared.max_total_rational_allocation_bits,
        exact_work.total_rational_allocation_bits,
    )?;
    measured.max_total_input_bits = measured.max_total_input_bits.min(remaining_payload_bits);
    measured.max_total_output_bits = measured
        .max_total_output_bits
        .min(remaining_payload_bits)
        .min(checked_remaining(
            shared.max_total_rational_output_bits,
            exact.work.total_output_bits,
        )?);
    Ok(measured)
}

fn checked_remaining(
    available: usize,
    consumed: usize,
) -> Result<usize, ProvenTransversalScanError> {
    available
        .checked_sub(consumed)
        .ok_or(ProvenTransversalScanError::ResourceLimitExceeded)
}

fn project_cayley_limits(hard: CayleyLimits, shared: &ProvenTransversalScanLimits) -> CayleyLimits {
    CayleyLimits {
        max_precision_rounds: hard
            .max_precision_rounds
            .min(shared.max_rational_operations),
        max_guard_bits: hard
            .max_guard_bits
            .min(shared.max_rational_intermediate_bits),
        max_candidate_bits: hard
            .max_candidate_bits
            .min(shared.max_rational_intermediate_bits),
        max_machin_terms_per_series: hard
            .max_machin_terms_per_series
            .min(shared.max_rational_operations),
        max_trig_terms_per_series: hard
            .max_trig_terms_per_series
            .min(shared.max_rational_operations),
        max_sqrt_refinements: hard
            .max_sqrt_refinements
            .min(shared.max_rational_operations),
        max_interval_operations: hard
            .max_interval_operations
            .min(shared.max_rational_operations),
        max_shift_bits: hard
            .max_shift_bits
            .min(shared.max_rational_intermediate_bits),
        max_intermediate_bits: hard
            .max_intermediate_bits
            .min(shared.max_rational_intermediate_bits),
        max_gcd_fallback_calls: hard
            .max_gcd_fallback_calls
            .min(shared.max_rational_gcd_fallback_calls),
        max_gcd_fallback_input_bits: hard
            .max_gcd_fallback_input_bits
            .min(shared.max_rational_gcd_fallback_input_bits),
        max_rational_allocations: hard
            .max_rational_allocations
            .min(shared.max_rational_allocations),
        max_rational_allocation_bits: hard
            .max_rational_allocation_bits
            .min(shared.max_rational_allocation_bits),
        max_total_rational_allocation_bits: hard
            .max_total_rational_allocation_bits
            .min(shared.max_total_rational_allocation_bits)
            .min(shared.max_total_rational_retained_clone_bits),
        max_output_bits: hard.max_output_bits.min(shared.max_rational_output_bits),
    }
}

fn project_cayley_resume_limits(
    hard: CayleyResumeLimits,
    shared: &ProvenTransversalScanLimits,
) -> CayleyResumeLimits {
    CayleyResumeLimits {
        max_machin_terms_per_series: hard
            .max_machin_terms_per_series
            .min(shared.max_rational_operations),
        max_trig_terms_per_series: hard
            .max_trig_terms_per_series
            .min(shared.max_rational_operations),
        max_sqrt_refinements: hard
            .max_sqrt_refinements
            .min(shared.max_rational_operations),
        max_interval_operations: hard
            .max_interval_operations
            .min(shared.max_rational_operations),
        max_shift_bits: hard
            .max_shift_bits
            .min(shared.max_rational_intermediate_bits),
        max_intermediate_bits: hard
            .max_intermediate_bits
            .min(shared.max_rational_intermediate_bits),
        max_gcd_fallback_calls: hard
            .max_gcd_fallback_calls
            .min(shared.max_rational_gcd_fallback_calls),
        max_gcd_fallback_input_bits: hard
            .max_gcd_fallback_input_bits
            .min(shared.max_rational_gcd_fallback_input_bits),
        max_rational_allocations: hard
            .max_rational_allocations
            .min(shared.max_rational_allocations),
        max_rational_allocation_bits: hard
            .max_rational_allocation_bits
            .min(shared.max_rational_allocation_bits),
        max_total_rational_allocation_bits: hard
            .max_total_rational_allocation_bits
            .min(shared.max_total_rational_allocation_bits)
            .min(shared.max_total_rational_retained_clone_bits),
        max_output_bits: hard.max_output_bits.min(shared.max_rational_output_bits),
    }
}

#[allow(clippy::too_many_arguments)]
fn preflight_proven_transversal_shape(
    bound: BoundMaterialTreePose<'_>,
    max_faces: usize,
    max_hinges: usize,
    max_unordered_face_pairs: usize,
    max_boundary_vertices_per_face: usize,
    max_total_boundary_vertices: usize,
    max_triangular_faces: usize,
    max_triangular_face_pairs: usize,
    max_registry_authentication_work: usize,
    max_total_boundary_relation_work: usize,
) -> Result<(), ProvenTransversalScanError> {
    let faces = bound.model().face_ids();
    let face_count = faces.len();
    let hinge_count = bound.model().hinges().len();
    if faces.is_empty() || face_count > max_faces || hinge_count > max_hinges {
        return Err(ProvenTransversalScanError::ResourceLimitExceeded);
    }
    let unordered_face_pairs = projected_unordered_pair_count(face_count)?;
    if unordered_face_pairs > max_unordered_face_pairs {
        return Err(ProvenTransversalScanError::ResourceLimitExceeded);
    }
    // Reserve all face/hinge/depth work before the first boundary registry
    // read. Boundary-dependent work is then reserved monotonically below.
    let registry_base_work = projected_registry_reservation(face_count, hinge_count, 0)?;
    if registry_base_work > max_registry_authentication_work
        || unordered_face_pairs > max_total_boundary_relation_work
    {
        return Err(ProvenTransversalScanError::ResourceLimitExceeded);
    }
    let mut total_boundary_vertices = 0_usize;
    let mut triangular_faces = 0_usize;
    for face in faces {
        let boundary = bound
            .face_boundary(*face)
            .filter(|boundary| bound.model().owns_face_boundary(*boundary))
            .ok_or(ProvenTransversalScanError::InconsistentPose)?;
        let boundary_vertices = boundary.vertices().len();
        if boundary_vertices > max_boundary_vertices_per_face {
            return Err(ProvenTransversalScanError::ResourceLimitExceeded);
        }
        total_boundary_vertices = projected_sum(total_boundary_vertices, boundary_vertices)?;
        if total_boundary_vertices > max_total_boundary_vertices {
            return Err(ProvenTransversalScanError::ResourceLimitExceeded);
        }
        let registry_work =
            projected_registry_reservation(face_count, hinge_count, total_boundary_vertices)?;
        if registry_work > max_registry_authentication_work {
            return Err(ProvenTransversalScanError::ResourceLimitExceeded);
        }
        if boundary_vertices == 3 {
            triangular_faces = projected_sum(triangular_faces, 1)?;
            let triangular_face_pairs = projected_unordered_pair_count(triangular_faces)?;
            let boundary_relation_work = projected_boundary_relation_reservation(
                unordered_face_pairs,
                triangular_face_pairs,
            )?;
            if boundary_relation_work > max_total_boundary_relation_work {
                return Err(ProvenTransversalScanError::ResourceLimitExceeded);
            }
        }
    }
    if triangular_faces > max_triangular_faces
        || projected_unordered_pair_count(triangular_faces)? > max_triangular_face_pairs
    {
        return Err(ProvenTransversalScanError::ResourceLimitExceeded);
    }
    Ok(())
}

fn projected_registry_reservation(
    face_count: usize,
    hinge_count: usize,
    boundary_occurrences: usize,
) -> Result<usize, ProvenTransversalScanError> {
    let maximum_total_depth = projected_product(face_count, face_count.saturating_sub(1))? / 2;
    projected_sum(
        projected_sum(
            projected_product(face_count, 24)?,
            projected_product(boundary_occurrences, 16)?,
        )?,
        projected_sum(projected_product(hinge_count, 76)?, maximum_total_depth)?,
    )
}

fn projected_boundary_relation_reservation(
    unordered_face_pairs: usize,
    triangular_face_pairs: usize,
) -> Result<usize, ProvenTransversalScanError> {
    projected_sum(
        projected_product(triangular_face_pairs, 11)?,
        unordered_face_pairs,
    )
}

fn projected_unordered_pair_count(count: usize) -> Result<usize, ProvenTransversalScanError> {
    Ok(projected_product(count, count.saturating_sub(1))? / 2)
}

fn projected_product(first: usize, second: usize) -> Result<usize, ProvenTransversalScanError> {
    first
        .checked_mul(second)
        .ok_or(ProvenTransversalScanError::ResourceLimitExceeded)
}

fn projected_sum(first: usize, second: usize) -> Result<usize, ProvenTransversalScanError> {
    first
        .checked_add(second)
        .ok_or(ProvenTransversalScanError::ResourceLimitExceeded)
}

fn map_cayley_scan_error(error: CayleyError) -> ProvenTransversalScanError {
    match error {
        CayleyError::ResourceLimitExceeded { .. } => {
            ProvenTransversalScanError::ResourceLimitExceeded
        }
        CayleyError::CertificateUnavailable { .. } => {
            ProvenTransversalScanError::EvidenceUnavailable
        }
        CayleyError::NonFiniteInput { .. }
        | CayleyError::AngleOutOfRange { .. }
        | CayleyError::InvalidRotationSign { .. }
        | CayleyError::DegenerateAxis { .. }
        | CayleyError::InvariantFailure { .. }
        | CayleyError::BoundTreeInconsistent { .. } => ProvenTransversalScanError::InconsistentPose,
    }
}

fn map_measured_scan_error(error: MeasuredEnvelopeError) -> ProvenTransversalScanError {
    match error {
        MeasuredEnvelopeError::ResourceLimitExceeded { .. } => {
            ProvenTransversalScanError::ResourceLimitExceeded
        }
        MeasuredEnvelopeError::AuthorityMismatch | MeasuredEnvelopeError::InconsistentPose => {
            ProvenTransversalScanError::InconsistentPose
        }
    }
}

fn map_authenticated_scan_error(
    error: AuthenticatedTriangleBlockingError,
) -> ProvenTransversalScanError {
    match error {
        AuthenticatedTriangleBlockingError::ResourceLimitExceeded { .. } => {
            ProvenTransversalScanError::ResourceLimitExceeded
        }
        AuthenticatedTriangleBlockingError::Exact(error) => map_cayley_scan_error(error),
        AuthenticatedTriangleBlockingError::UnsupportedPaperThickness
        | AuthenticatedTriangleBlockingError::AuthorityMismatch
        | AuthenticatedTriangleBlockingError::InconsistentPose => {
            ProvenTransversalScanError::InconsistentPose
        }
    }
}

/// Dimension-neutral closed rational interval used by the robust predicate.
///
/// Coordinates acquire the actual-mm contract only when stored in
/// [`ActualMillimetrePointInterval`]. Determinants and interpolation
/// parameters deliberately reuse this scalar container without claiming that
/// their dimensions are millimetres.
#[derive(Debug, Clone)]
struct ClosedRationalInterval {
    lower: BigRational,
    upper: BigRational,
}

impl ClosedRationalInterval {
    #[cfg(test)]
    fn point(value: BigRational) -> Self {
        Self {
            lower: value.clone(),
            upper: value,
        }
    }

    #[cfg(test)]
    fn inflated(value: BigRational, radius: BigRational) -> Self {
        assert!(!radius.is_negative());
        Self {
            lower: &value - &radius,
            upper: value + radius,
        }
    }

    fn add(&self, other: &Self, meter: &mut WorkMeter<'_>) -> Result<Self, CayleyError> {
        Self::ordered(
            meter.add_rational(&self.lower, &other.lower, CayleyStage::Containment)?,
            meter.add_rational(&self.upper, &other.upper, CayleyStage::Containment)?,
            meter,
        )
    }

    fn subtract(&self, other: &Self, meter: &mut WorkMeter<'_>) -> Result<Self, CayleyError> {
        Self::ordered(
            meter.subtract_rational(&self.lower, &other.upper, CayleyStage::Containment)?,
            meter.subtract_rational(&self.upper, &other.lower, CayleyStage::Containment)?,
            meter,
        )
    }

    fn multiply(&self, other: &Self, meter: &mut WorkMeter<'_>) -> Result<Self, CayleyError> {
        let products = [
            meter.multiply_rational(&self.lower, &other.lower, CayleyStage::Containment)?,
            meter.multiply_rational(&self.lower, &other.upper, CayleyStage::Containment)?,
            meter.multiply_rational(&self.upper, &other.lower, CayleyStage::Containment)?,
            meter.multiply_rational(&self.upper, &other.upper, CayleyStage::Containment)?,
        ];
        Self::hull(products, meter)
    }

    fn divide(&self, other: &Self, meter: &mut WorkMeter<'_>) -> Result<Option<Self>, CayleyError> {
        if other.contains_zero(meter)? {
            return Ok(None);
        }
        let quotients = [
            meter.divide_rational(&self.lower, &other.lower, CayleyStage::Containment)?,
            meter.divide_rational(&self.lower, &other.upper, CayleyStage::Containment)?,
            meter.divide_rational(&self.upper, &other.lower, CayleyStage::Containment)?,
            meter.divide_rational(&self.upper, &other.upper, CayleyStage::Containment)?,
        ];
        Self::hull(quotients, meter).map(Some)
    }

    fn hull(values: [BigRational; 4], meter: &mut WorkMeter<'_>) -> Result<Self, CayleyError> {
        let mut minimum = &values[0];
        let mut maximum = &values[0];
        for value in &values[1..] {
            if meter.compare_rational(value, minimum, CayleyStage::Containment)? == Ordering::Less {
                minimum = value;
            }
            if meter.compare_rational(value, maximum, CayleyStage::Containment)?
                == Ordering::Greater
            {
                maximum = value;
            }
        }
        Self::ordered(
            meter.clone_rational(minimum, CayleyStage::Containment)?,
            meter.clone_rational(maximum, CayleyStage::Containment)?,
            meter,
        )
    }

    fn ordered(
        lower: BigRational,
        upper: BigRational,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Self, CayleyError> {
        if meter.compare_rational(&lower, &upper, CayleyStage::Containment)? == Ordering::Greater {
            return Err(CayleyError::InvariantFailure {
                stage: CayleyStage::Containment,
            });
        }
        Ok(Self { lower, upper })
    }

    fn contains_zero(&self, meter: &mut WorkMeter<'_>) -> Result<bool, CayleyError> {
        meter.operation(CayleyStage::Containment)?;
        Ok(!self.lower.is_positive() && !self.upper.is_negative())
    }

    fn strict_sign(&self, meter: &mut WorkMeter<'_>) -> Result<Option<StrictSign>, CayleyError> {
        meter.operation(CayleyStage::Containment)?;
        Ok(if self.lower.is_positive() {
            Some(StrictSign::Positive)
        } else if self.upper.is_negative() {
            Some(StrictSign::Negative)
        } else {
            None
        })
    }

    fn strictly_inside_unit_interval(
        &self,
        meter: &mut WorkMeter<'_>,
    ) -> Result<bool, CayleyError> {
        meter.operation(CayleyStage::Containment)?;
        Ok(self.lower.is_positive()
            && self.upper.is_positive()
            && self.upper.numer() < self.upper.denom())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StrictSign {
    Negative,
    Positive,
}

#[derive(Debug, Clone)]
struct ActualMillimetrePointInterval {
    coordinates: [ClosedRationalInterval; 3],
}

impl ActualMillimetrePointInterval {
    fn subtract(
        &self,
        other: &Self,
        meter: &mut WorkMeter<'_>,
    ) -> Result<[ClosedRationalInterval; 3], CayleyError> {
        try_array3(|axis| self.coordinates[axis].subtract(&other.coordinates[axis], meter))
    }

    fn interpolate(
        &self,
        end: &Self,
        parameter: &ClosedRationalInterval,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Self, CayleyError> {
        let delta = end.subtract(self, meter)?;
        Ok(Self {
            coordinates: try_array3(|axis| {
                self.coordinates[axis].add(&delta[axis].multiply(parameter, meter)?, meter)
            })?,
        })
    }
}

#[derive(Debug, Clone)]
struct ActualMillimetreTriangleEnvelope {
    points: [ActualMillimetrePointInterval; 3],
}

#[derive(Debug, Clone)]
struct PreparedAuthenticatedTriangle {
    exact: ActualMillimetreTriangleEnvelope,
    observed: ActualMillimetreTriangleEnvelope,
}

fn scan_authenticated_triangle_pairs_blocking_only<'scan, 'exact, 'pose>(
    exact: &'scan RationalCayleyTreePose<'pose>,
    measured: &'scan MeasuredBinary64AffineEnvelope<'exact, 'pose>,
    bound: BoundMaterialTreePose<'pose>,
    paper_thickness_mm: f64,
    limits: AuthenticatedTriangleBlockingLimits,
) -> Result<
    AuthenticatedTriangleBlockingScan<'scan, 'exact, 'pose>,
    AuthenticatedTriangleBlockingError,
> {
    validate_authenticated_triangle_blocking_limits(&limits)?;
    if paper_thickness_mm.to_bits() != 0.0_f64.to_bits() {
        return Err(AuthenticatedTriangleBlockingError::UnsupportedPaperThickness);
    }
    if !exact.is_for(bound) || !measured.is_for(exact, bound) {
        return Err(AuthenticatedTriangleBlockingError::AuthorityMismatch);
    }
    if exact.version != RATIONAL_CAYLEY_TREE_POSE_V1
        || exact.fixed_face != bound.pose().fixed_face()
        || exact.faces.is_empty()
        || exact.faces.len() != measured.faces.len()
        || exact.faces.len() != exact.work.faces
        || exact.hinges.len() != exact.work.hinges
        || bound.model().hinges().len() != exact.hinges.len()
        || bound.pose().hinge_angles().len() != exact.hinges.len()
        || measured.work.faces != exact.faces.len()
        || measured.work.hinges != exact.hinges.len()
    {
        return Err(AuthenticatedTriangleBlockingError::InconsistentPose);
    }

    let face_count = exact.faces.len();
    let hinge_count = exact.hinges.len();
    bridge_check_count(face_count, limits.max_faces, "faces")?;
    bridge_check_count(hinge_count, limits.max_hinges, "hinges")?;
    let expected_hinge_face_lookups = hinge_count.checked_mul(2).ok_or(
        AuthenticatedTriangleBlockingError::ResourceLimitExceeded {
            resource: "hinge_face_lookups",
        },
    )?;
    let expected_hinge_vertex_lookups = hinge_count.checked_mul(4).ok_or(
        AuthenticatedTriangleBlockingError::ResourceLimitExceeded {
            resource: "hinge_vertex_lookups",
        },
    )?;
    bridge_check_count(
        expected_hinge_face_lookups,
        limits.max_hinge_face_lookups,
        "hinge_face_lookups",
    )?;
    bridge_check_count(
        expected_hinge_vertex_lookups,
        limits.max_hinge_vertex_lookups,
        "hinge_vertex_lookups",
    )?;
    let expected_pairs = checked_unordered_pair_count(face_count)?;
    bridge_check_count(
        expected_pairs,
        limits.max_unordered_face_pairs,
        "unordered_face_pairs",
    )?;
    bridge_check_count(expected_pairs, limits.max_result_records, "result_records")?;

    let model_faces = bound.model().face_ids();
    if model_faces.len() != face_count
        || !exact
            .faces
            .iter()
            .zip(model_faces)
            .all(|(face, expected)| face.face == *expected)
        || !exact
            .faces
            .windows(2)
            .all(|pair| pair[0].face.canonical_bytes() < pair[1].face.canonical_bytes())
        || !measured
            .faces
            .iter()
            .zip(&exact.faces)
            .all(|(radius, face)| radius.face == face.face)
    {
        return Err(AuthenticatedTriangleBlockingError::InconsistentPose);
    }

    let mut work = AuthenticatedTriangleBlockingWork {
        faces: face_count,
        hinges: hinge_count,
        unordered_face_pairs: expected_pairs,
        ..AuthenticatedTriangleBlockingWork::default()
    };
    let mut boundary_occurrences = 0_usize;
    let mut triangular_faces = 0_usize;
    for (face, measured_face) in exact.faces.iter().zip(&measured.faces) {
        if face.boundary.len() < 3 || measured_face.face != face.face {
            return Err(AuthenticatedTriangleBlockingError::InconsistentPose);
        }
        boundary_occurrences = boundary_occurrences
            .checked_add(face.boundary.len())
            .ok_or(AuthenticatedTriangleBlockingError::ResourceLimitExceeded {
                resource: "boundary_occurrences",
            })?;
        bridge_check_count(
            boundary_occurrences,
            limits.max_boundary_occurrences,
            "boundary_occurrences",
        )?;
        if face.boundary.len() == 3 {
            if face.boundary[0].0 == face.boundary[1].0
                || face.boundary[1].0 == face.boundary[2].0
                || face.boundary[2].0 == face.boundary[0].0
            {
                return Err(AuthenticatedTriangleBlockingError::InconsistentPose);
            }
            triangular_faces = triangular_faces.checked_add(1).ok_or(
                AuthenticatedTriangleBlockingError::ResourceLimitExceeded {
                    resource: "triangular_faces",
                },
            )?;
            bridge_check_count(
                triangular_faces,
                limits.max_triangular_faces,
                "triangular_faces",
            )?;
        }
    }
    if boundary_occurrences != exact.work.boundary_occurrences
        || boundary_occurrences != measured.work.boundary_occurrences
    {
        return Err(AuthenticatedTriangleBlockingError::InconsistentPose);
    }
    work.boundary_occurrences = boundary_occurrences;
    work.triangular_faces = triangular_faces;
    let expected_triangular_face_pairs = checked_unordered_pair_count(triangular_faces)?;
    bridge_check_count(
        expected_triangular_face_pairs,
        limits.max_triangular_face_pairs,
        "triangular_face_pairs",
    )?;
    let expected_topology_vertex_checks = expected_triangular_face_pairs.checked_mul(9).ok_or(
        AuthenticatedTriangleBlockingError::ResourceLimitExceeded {
            resource: "topology_vertex_checks",
        },
    )?;
    bridge_check_count(
        expected_topology_vertex_checks,
        limits.max_topology_vertex_checks,
        "topology_vertex_checks",
    )?;
    let expected_predicate_calls = expected_triangular_face_pairs.checked_mul(2).ok_or(
        AuthenticatedTriangleBlockingError::ResourceLimitExceeded {
            resource: "predicate_calls",
        },
    )?;
    bridge_check_count(
        expected_predicate_calls,
        limits.max_predicate_calls,
        "predicate_calls",
    )?;
    work.expected_triangular_face_pairs = expected_triangular_face_pairs;
    work.unsupported_pair_records = expected_pairs
        .checked_sub(expected_triangular_face_pairs)
        .ok_or(AuthenticatedTriangleBlockingError::InconsistentPose)?;

    let total_terms = TotalTermLimits {
        machin_terms: limits.max_total_machin_terms,
        trig_terms: limits.max_total_trig_terms,
        sqrt_refinements: limits.max_total_sqrt_refinements,
    };
    let exact_limits = limits.exact.as_cayley_limits();
    let mut meter = WorkMeter::resume(
        &exact_limits,
        Some(total_terms),
        &exact.work.exact,
        CayleyStage::Containment,
    )?;
    meter.merge_work(&measured.work.exact, CayleyStage::Containment)?;

    let hinge_relations = authenticate_hinge_relations(exact, &limits, &mut work)?;
    let mut triangles = Vec::new();
    triangles.try_reserve_exact(face_count).map_err(|_| {
        AuthenticatedTriangleBlockingError::ResourceLimitExceeded { resource: "faces" }
    })?;
    for (face, radius) in exact.faces.iter().zip(&measured.faces) {
        triangles.push(if face.boundary.len() == 3 {
            Some(prepare_authenticated_triangle(
                face, radius, bound, &mut meter,
            )?)
        } else {
            None
        });
    }

    let mut pairs = Vec::new();
    pairs.try_reserve_exact(expected_pairs).map_err(|_| {
        AuthenticatedTriangleBlockingError::ResourceLimitExceeded {
            resource: "result_records",
        }
    })?;
    let mut topology_vertex_checks = 0_usize;
    for first_index in 0..face_count {
        for second_index in (first_index + 1)..face_count {
            let first_face = &exact.faces[first_index];
            let second_face = &exact.faces[second_index];
            let decision = match (&triangles[first_index], &triangles[second_index]) {
                (Some(first), Some(second)) => {
                    bridge_increment(
                        &mut work.analyzed_triangular_face_pairs,
                        limits.max_triangular_face_pairs,
                        "triangular_face_pairs",
                    )?;
                    let topology = derive_pair_topology(
                        first_face,
                        second_face,
                        &hinge_relations,
                        &mut topology_vertex_checks,
                        &limits,
                    )?;
                    bridge_increment(
                        &mut work.exact_predicate_calls,
                        limits.max_predicate_calls,
                        "predicate_calls",
                    )?;
                    let exact_decision = classify_transversal_with_meter(
                        &first.exact,
                        &second.exact,
                        topology,
                        &mut meter,
                    )?;
                    bridge_increment(
                        &mut work.observed_predicate_calls,
                        limits.max_predicate_calls,
                        "predicate_calls",
                    )?;
                    let observed_decision = classify_transversal_with_meter(
                        &first.observed,
                        &second.observed,
                        topology,
                        &mut meter,
                    )?;
                    if exact_decision == BlockingOnlyDecision::ProvenPenetrating
                        && observed_decision == BlockingOnlyDecision::ProvenPenetrating
                    {
                        BlockingOnlyDecision::ProvenPenetrating
                    } else {
                        BlockingOnlyDecision::Unresolved
                    }
                }
                _ => BlockingOnlyDecision::Unresolved,
            };
            pairs.push(AuthenticatedTrianglePairDecision {
                first: first_face.face,
                second: second_face.face,
                decision,
            });
        }
    }
    work.topology_vertex_checks = topology_vertex_checks;
    work.result_records = pairs.len();
    if pairs.len() != expected_pairs
        || work.analyzed_triangular_face_pairs != expected_triangular_face_pairs
        || work.exact_predicate_calls != expected_triangular_face_pairs
        || work.observed_predicate_calls != expected_triangular_face_pairs
        || work
            .exact_predicate_calls
            .checked_add(work.observed_predicate_calls)
            != Some(expected_predicate_calls)
        || work.topology_vertex_checks != expected_topology_vertex_checks
        || work.hinge_relation_records != hinge_count
        || work.hinge_boundary_index_entries != boundary_occurrences
        || work.hinge_face_lookups != expected_hinge_face_lookups
        || work.hinge_vertex_lookups != expected_hinge_vertex_lookups
        || work
            .unsupported_pair_records
            .checked_add(work.analyzed_triangular_face_pairs)
            != Some(expected_pairs)
    {
        return Err(AuthenticatedTriangleBlockingError::InconsistentPose);
    }
    work.exact = meter.work;
    Ok(AuthenticatedTriangleBlockingScan {
        exact,
        measured,
        bound,
        paper_thickness_bits: paper_thickness_mm.to_bits(),
        pairs,
        work,
    })
}

fn prepare_authenticated_triangle(
    face: &ExactFacePose,
    measured: &MeasuredFaceEnvelope,
    bound: BoundMaterialTreePose<'_>,
    meter: &mut WorkMeter<'_>,
) -> Result<PreparedAuthenticatedTriangle, CayleyError> {
    if face.boundary.len() != 3 || measured.face != face.face {
        return Err(CayleyError::InvariantFailure {
            stage: CayleyStage::Containment,
        });
    }
    for radius in &measured.radius {
        if !valid_canonical_rational(radius, meter)? || radius.is_negative() {
            return Err(CayleyError::InvariantFailure {
                stage: CayleyStage::Containment,
            });
        }
    }
    let binary64_transform = exact_binary64_affine_transform(
        bound
            .pose()
            .face_transform(face.face)
            .ok_or(CayleyError::InvariantFailure {
                stage: CayleyStage::Containment,
            })?,
        meter,
    )?;
    let observed_points = try_array3(|vertex| {
        let source = bound
            .model()
            .vertex_position(face.boundary[vertex].0)
            .filter(|point| point.y() == 0.0)
            .ok_or(CayleyError::InvariantFailure {
                stage: CayleyStage::Containment,
            })?;
        let source = exact_point(point3_array(source), meter)?;
        let observed = apply_exact_transform(&binary64_transform, &source, meter)?;
        for axis in 0..3 {
            let delta = meter.subtract_rational(
                &observed.coordinates[axis],
                &face.boundary[vertex].1.coordinates[axis],
                CayleyStage::Containment,
            )?;
            let delta = meter.absolute_rational(&delta, CayleyStage::Containment)?;
            if meter.compare_rational(&delta, &measured.radius[axis], CayleyStage::Containment)?
                == Ordering::Greater
            {
                return Err(CayleyError::InvariantFailure {
                    stage: CayleyStage::Containment,
                });
            }
        }
        exact_point_interval(&observed.coordinates, meter)
    })?;
    Ok(PreparedAuthenticatedTriangle {
        exact: ActualMillimetreTriangleEnvelope {
            points: try_array3(|vertex| {
                exact_point_interval(&face.boundary[vertex].1.coordinates, meter)
            })?,
        },
        observed: ActualMillimetreTriangleEnvelope {
            points: observed_points,
        },
    })
}

fn exact_binary64_affine_transform(
    transform: ori_kinematics::RigidTransform,
    meter: &mut WorkMeter<'_>,
) -> Result<ExactRigidTransform, CayleyError> {
    let rows = transform.rotation_rows();
    let translation = transform.translation();
    Ok(ExactRigidTransform {
        rotation: try_array3(|row| {
            try_array3(|column| exact_f64(rows[row][column], meter, CayleyStage::Containment))
        })?,
        translation: ExactVector3 {
            coordinates: [
                exact_f64(translation.x(), meter, CayleyStage::Containment)?,
                exact_f64(translation.y(), meter, CayleyStage::Containment)?,
                exact_f64(translation.z(), meter, CayleyStage::Containment)?,
            ],
        },
    })
}

fn exact_point_interval(
    point: &[BigRational; 3],
    meter: &mut WorkMeter<'_>,
) -> Result<ActualMillimetrePointInterval, CayleyError> {
    Ok(ActualMillimetrePointInterval {
        coordinates: try_array3(|axis| {
            if !valid_canonical_rational(&point[axis], meter)? {
                return Err(CayleyError::InvariantFailure {
                    stage: CayleyStage::Containment,
                });
            }
            ClosedRationalInterval::ordered(
                meter.clone_rational(&point[axis], CayleyStage::Containment)?,
                meter.clone_rational(&point[axis], CayleyStage::Containment)?,
                meter,
            )
        })?,
    })
}

fn authenticate_hinge_relations(
    exact: &RationalCayleyTreePose<'_>,
    limits: &AuthenticatedTriangleBlockingLimits,
    work: &mut AuthenticatedTriangleBlockingWork,
) -> Result<HashMap<(FaceId, FaceId), [VertexId; 2]>, AuthenticatedTriangleBlockingError> {
    bridge_check_count(exact.hinges.len(), limits.max_hinges, "hinges")?;
    bridge_check_count(
        exact.work.boundary_occurrences,
        limits.max_hinge_boundary_index_entries,
        "hinge_boundary_index_entries",
    )?;
    let mut occurrences = HashMap::<(FaceId, VertexId), (usize, usize)>::new();
    occurrences
        .try_reserve(exact.work.boundary_occurrences)
        .map_err(
            |_| AuthenticatedTriangleBlockingError::ResourceLimitExceeded {
                resource: "hinge_boundary_index_entries",
            },
        )?;
    for (face_index, face) in exact.faces.iter().enumerate() {
        for (boundary_index, (vertex, _)) in face.boundary.iter().enumerate() {
            bridge_increment(
                &mut work.hinge_boundary_index_entries,
                limits.max_hinge_boundary_index_entries,
                "hinge_boundary_index_entries",
            )?;
            if occurrences
                .insert((face.face, *vertex), (face_index, boundary_index))
                .is_some()
            {
                return Err(AuthenticatedTriangleBlockingError::InconsistentPose);
            }
        }
    }
    let mut relations = HashMap::new();
    relations.try_reserve(exact.hinges.len()).map_err(|_| {
        AuthenticatedTriangleBlockingError::ResourceLimitExceeded { resource: "hinges" }
    })?;
    for hinge in &exact.hinges {
        let key = canonical_face_pair(hinge.parent, hinge.child)
            .ok_or(AuthenticatedTriangleBlockingError::InconsistentPose)?;
        if hinge.endpoint_vertices[0] == hinge.endpoint_vertices[1] {
            return Err(AuthenticatedTriangleBlockingError::InconsistentPose);
        }
        for face_id in [hinge.parent, hinge.child] {
            bridge_increment(
                &mut work.hinge_face_lookups,
                limits.max_hinge_face_lookups,
                "hinge_face_lookups",
            )?;
            let face_index = exact
                .faces
                .binary_search_by_key(&face_id.canonical_bytes(), |face| {
                    face.face.canonical_bytes()
                })
                .map_err(|_| AuthenticatedTriangleBlockingError::InconsistentPose)?;
            for (vertex, endpoint) in hinge.endpoint_vertices.iter().zip(&hinge.world_endpoints) {
                bridge_increment(
                    &mut work.hinge_vertex_lookups,
                    limits.max_hinge_vertex_lookups,
                    "hinge_vertex_lookups",
                )?;
                let (occurrence_face_index, boundary_index) = occurrences
                    .get(&(face_id, *vertex))
                    .copied()
                    .ok_or(AuthenticatedTriangleBlockingError::InconsistentPose)?;
                if occurrence_face_index != face_index
                    || exact.faces[face_index]
                        .boundary
                        .get(boundary_index)
                        .is_none_or(|(candidate, point)| {
                            candidate != vertex || !canonical_point_eq(point, endpoint)
                        })
                {
                    return Err(AuthenticatedTriangleBlockingError::InconsistentPose);
                }
            }
        }
        if relations.insert(key, hinge.endpoint_vertices).is_some() {
            return Err(AuthenticatedTriangleBlockingError::InconsistentPose);
        }
        bridge_increment(
            &mut work.hinge_relation_records,
            limits.max_hinges,
            "hinges",
        )?;
    }
    Ok(relations)
}

fn derive_pair_topology(
    first: &ExactFacePose,
    second: &ExactFacePose,
    hinge_relations: &HashMap<(FaceId, FaceId), [VertexId; 2]>,
    topology_vertex_checks: &mut usize,
    limits: &AuthenticatedTriangleBlockingLimits,
) -> Result<PairTopology, AuthenticatedTriangleBlockingError> {
    if first.face == second.face || first.boundary.len() != 3 || second.boundary.len() != 3 {
        return Err(AuthenticatedTriangleBlockingError::InconsistentPose);
    }
    let mut shared = [None; 3];
    let mut shared_count = 0_usize;
    for (first_index, (first_vertex, first_point)) in first.boundary.iter().enumerate() {
        for (second_index, (second_vertex, second_point)) in second.boundary.iter().enumerate() {
            bridge_increment(
                topology_vertex_checks,
                limits.max_topology_vertex_checks,
                "topology_vertex_checks",
            )?;
            if first_vertex == second_vertex {
                if !canonical_point_eq(first_point, second_point) || shared_count >= shared.len() {
                    return Err(AuthenticatedTriangleBlockingError::InconsistentPose);
                }
                shared[shared_count] = Some((first_index, second_index, *first_vertex));
                shared_count += 1;
            }
        }
    }
    match shared_count {
        0 => Ok(PairTopology::NoSharedFeature),
        1 => {
            let (first_vertex, second_vertex, _) =
                shared[0].ok_or(AuthenticatedTriangleBlockingError::InconsistentPose)?;
            Ok(PairTopology::SharedVertex {
                first_vertex,
                second_vertex,
            })
        }
        2 => {
            let key = canonical_face_pair(first.face, second.face)
                .ok_or(AuthenticatedTriangleBlockingError::InconsistentPose)?;
            let endpoints = hinge_relations
                .get(&key)
                .ok_or(AuthenticatedTriangleBlockingError::InconsistentPose)?;
            let first_shared = shared[0]
                .ok_or(AuthenticatedTriangleBlockingError::InconsistentPose)?
                .2;
            let second_shared = shared[1]
                .ok_or(AuthenticatedTriangleBlockingError::InconsistentPose)?
                .2;
            if !same_unordered_vertex_pair(*endpoints, [first_shared, second_shared]) {
                return Err(AuthenticatedTriangleBlockingError::InconsistentPose);
            }
            Ok(PairTopology::SharedHinge)
        }
        _ => Err(AuthenticatedTriangleBlockingError::InconsistentPose),
    }
}

fn same_unordered_vertex_pair(first: [VertexId; 2], second: [VertexId; 2]) -> bool {
    (first[0] == second[0] && first[1] == second[1])
        || (first[0] == second[1] && first[1] == second[0])
}

fn canonical_face_pair(first: FaceId, second: FaceId) -> Option<(FaceId, FaceId)> {
    match first.canonical_bytes().cmp(&second.canonical_bytes()) {
        Ordering::Less => Some((first, second)),
        Ordering::Greater => Some((second, first)),
        Ordering::Equal => None,
    }
}

fn checked_unordered_pair_count(count: usize) -> Result<usize, AuthenticatedTriangleBlockingError> {
    count
        .checked_mul(count.saturating_sub(1))
        .and_then(|product| product.checked_div(2))
        .ok_or(AuthenticatedTriangleBlockingError::ResourceLimitExceeded {
            resource: "unordered_face_pairs",
        })
}

fn validate_authenticated_triangle_blocking_limits(
    limits: &AuthenticatedTriangleBlockingLimits,
) -> Result<(), AuthenticatedTriangleBlockingError> {
    let hard = AUTHENTICATED_TRIANGLE_BLOCKING_HARD_LIMITS;
    let structural_limits = [
        (limits.max_faces, hard.max_faces),
        (limits.max_hinges, hard.max_hinges),
        (
            limits.max_boundary_occurrences,
            hard.max_boundary_occurrences,
        ),
        (limits.max_triangular_faces, hard.max_triangular_faces),
        (
            limits.max_unordered_face_pairs,
            hard.max_unordered_face_pairs,
        ),
        (
            limits.max_triangular_face_pairs,
            hard.max_triangular_face_pairs,
        ),
        (
            limits.max_topology_vertex_checks,
            hard.max_topology_vertex_checks,
        ),
        (
            limits.max_hinge_boundary_index_entries,
            hard.max_hinge_boundary_index_entries,
        ),
        (limits.max_hinge_face_lookups, hard.max_hinge_face_lookups),
        (
            limits.max_hinge_vertex_lookups,
            hard.max_hinge_vertex_lookups,
        ),
        (limits.max_predicate_calls, hard.max_predicate_calls),
        (limits.max_result_records, hard.max_result_records),
        (limits.max_total_machin_terms, hard.max_total_machin_terms),
        (limits.max_total_trig_terms, hard.max_total_trig_terms),
        (
            limits.max_total_sqrt_refinements,
            hard.max_total_sqrt_refinements,
        ),
    ];
    let exact_limits = [
        (
            limits.exact.max_machin_terms_per_series,
            hard.exact.max_machin_terms_per_series,
        ),
        (
            limits.exact.max_trig_terms_per_series,
            hard.exact.max_trig_terms_per_series,
        ),
        (
            limits.exact.max_sqrt_refinements,
            hard.exact.max_sqrt_refinements,
        ),
        (
            limits.exact.max_interval_operations,
            hard.exact.max_interval_operations,
        ),
        (limits.exact.max_shift_bits, hard.exact.max_shift_bits),
        (
            limits.exact.max_intermediate_bits,
            hard.exact.max_intermediate_bits,
        ),
        (
            limits.exact.max_gcd_fallback_calls,
            hard.exact.max_gcd_fallback_calls,
        ),
        (
            limits.exact.max_gcd_fallback_input_bits,
            hard.exact.max_gcd_fallback_input_bits,
        ),
        (
            limits.exact.max_rational_allocations,
            hard.exact.max_rational_allocations,
        ),
        (
            limits.exact.max_rational_allocation_bits,
            hard.exact.max_rational_allocation_bits,
        ),
        (
            limits.exact.max_total_rational_allocation_bits,
            hard.exact.max_total_rational_allocation_bits,
        ),
        (limits.exact.max_output_bits, hard.exact.max_output_bits),
    ];
    if structural_limits
        .into_iter()
        .chain(exact_limits)
        .any(|(configured, ceiling)| configured > ceiling)
    {
        Err(AuthenticatedTriangleBlockingError::ResourceLimitExceeded {
            resource: "configured_hard_ceiling",
        })
    } else {
        Ok(())
    }
}

fn bridge_check_count(
    count: usize,
    maximum: usize,
    resource: &'static str,
) -> Result<(), AuthenticatedTriangleBlockingError> {
    if count > maximum {
        Err(AuthenticatedTriangleBlockingError::ResourceLimitExceeded { resource })
    } else {
        Ok(())
    }
}

fn bridge_increment(
    count: &mut usize,
    maximum: usize,
    resource: &'static str,
) -> Result<(), AuthenticatedTriangleBlockingError> {
    let next = count
        .checked_add(1)
        .ok_or(AuthenticatedTriangleBlockingError::ResourceLimitExceeded { resource })?;
    bridge_check_count(next, maximum, resource)?;
    *count = next;
    Ok(())
}

fn classify_transversal_blocking_only(
    first: &ActualMillimetreTriangleEnvelope,
    second: &ActualMillimetreTriangleEnvelope,
    topology: PairTopology,
    limits: CayleyLimits,
) -> BlockingOnlyDecision {
    classify_transversal_metered(first, second, topology, limits)
        .map(|(decision, _)| decision)
        .unwrap_or(BlockingOnlyDecision::Unresolved)
}

fn classify_transversal_metered(
    first: &ActualMillimetreTriangleEnvelope,
    second: &ActualMillimetreTriangleEnvelope,
    topology: PairTopology,
    limits: CayleyLimits,
) -> Result<(BlockingOnlyDecision, CayleyWork), CayleyError> {
    let mut meter = WorkMeter::new(&limits);
    let decision = classify_transversal_with_meter(first, second, topology, &mut meter)?;
    Ok((decision, meter.work))
}

fn classify_transversal_with_meter(
    first: &ActualMillimetreTriangleEnvelope,
    second: &ActualMillimetreTriangleEnvelope,
    topology: PairTopology,
    meter: &mut WorkMeter<'_>,
) -> Result<BlockingOnlyDecision, CayleyError> {
    if !valid_triangle_input(first, meter)? || !valid_triangle_input(second, meter)? {
        return Ok(BlockingOnlyDecision::Unresolved);
    }
    if matches!(topology, PairTopology::SharedHinge | PairTopology::SameFace) {
        return Ok(BlockingOnlyDecision::Unresolved);
    }
    let shared = match topology {
        PairTopology::NoSharedFeature => None,
        PairTopology::SharedVertex {
            first_vertex,
            second_vertex,
        } if first_vertex < 3 && second_vertex < 3 => Some((first_vertex, second_vertex)),
        PairTopology::SharedVertex { .. } => {
            return Ok(BlockingOnlyDecision::Unresolved);
        }
        PairTopology::SharedHinge | PairTopology::SameFace => unreachable!(),
    };
    if stable_projection(first, meter)?.is_none() || stable_projection(second, meter)?.is_none() {
        return Ok(BlockingOnlyDecision::Unresolved);
    }

    let mut proven = false;
    for edge in 0..3 {
        if shared.is_none_or(|(vertex, _)| !edge_contains_vertex(edge, vertex))
            && edge_strictly_pierces_triangle(first, edge, second, meter)?
        {
            proven = true;
        }
    }
    for edge in 0..3 {
        if shared.is_none_or(|(_, vertex)| !edge_contains_vertex(edge, vertex))
            && edge_strictly_pierces_triangle(second, edge, first, meter)?
        {
            proven = true;
        }
    }
    if let Some((first_vertex, second_vertex)) = shared
        && shared_vertex_interior_segments_overlap(
            first,
            first_vertex,
            second,
            second_vertex,
            meter,
        )?
    {
        proven = true;
    }
    Ok(if proven {
        BlockingOnlyDecision::ProvenPenetrating
    } else {
        BlockingOnlyDecision::Unresolved
    })
}

fn valid_triangle_input(
    triangle: &ActualMillimetreTriangleEnvelope,
    meter: &mut WorkMeter<'_>,
) -> Result<bool, CayleyError> {
    for point in &triangle.points {
        for interval in &point.coordinates {
            if !valid_canonical_rational(&interval.lower, meter)?
                || !valid_canonical_rational(&interval.upper, meter)?
                || meter.compare_rational(
                    &interval.lower,
                    &interval.upper,
                    CayleyStage::Containment,
                )? == Ordering::Greater
            {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

fn valid_canonical_rational(
    value: &BigRational,
    meter: &mut WorkMeter<'_>,
) -> Result<bool, CayleyError> {
    meter.operation(CayleyStage::Containment)?;
    if !value.denom().is_positive() {
        return Ok(false);
    }
    Ok(meter
        .gcd_fallback(value.numer(), value.denom(), CayleyStage::Containment)?
        .is_one())
}

const fn edge_contains_vertex(edge: usize, vertex: usize) -> bool {
    edge == vertex || (edge + 1) % 3 == vertex
}

fn edge_strictly_pierces_triangle(
    source: &ActualMillimetreTriangleEnvelope,
    edge: usize,
    target: &ActualMillimetreTriangleEnvelope,
    meter: &mut WorkMeter<'_>,
) -> Result<bool, CayleyError> {
    let start = &source.points[edge];
    let end = &source.points[(edge + 1) % 3];
    let start_distance = signed_plane_distance(target, start, meter)?;
    let end_distance = signed_plane_distance(target, end, meter)?;
    let Some(start_sign) = start_distance.strict_sign(meter)? else {
        return Ok(false);
    };
    let Some(end_sign) = end_distance.strict_sign(meter)? else {
        return Ok(false);
    };
    if start_sign == end_sign {
        return Ok(false);
    }

    let denominator = start_distance.subtract(&end_distance, meter)?;
    if denominator.contains_zero(meter)? {
        return Ok(false);
    }
    let Some(parameter) = start_distance.divide(&denominator, meter)? else {
        return Ok(false);
    };
    if !parameter.strictly_inside_unit_interval(meter)? {
        return Ok(false);
    }
    let intersection = start.interpolate(end, &parameter, meter)?;
    point_strictly_inside_triangle_projection(&intersection, target, meter)
}

/// Proves a positive-length transversal through the relative interiors of
/// two triangles that share exactly one topological vertex.
///
/// The ordinary edge-piercing witness is intentionally strict and therefore
/// cannot see a segment whose far endpoint lies on both opposite edges. This
/// path is therefore admitted only when both shared occurrences are the same
/// exact singleton. Nonzero or distinct boxes are rejected: per-face affine
/// envelopes do not carry the correlation needed to weld them soundly. Each
/// opposite edge must cross the other plane at a strict interior parameter.
/// The two cut points and common vertex then lie on the planes' common line;
/// a strictly positive direction dot product proves that both
/// relative-interior segments occupy the same ray instead of meeting only at
/// the shared point.
fn shared_vertex_interior_segments_overlap(
    first: &ActualMillimetreTriangleEnvelope,
    first_shared: usize,
    second: &ActualMillimetreTriangleEnvelope,
    second_shared: usize,
    meter: &mut WorkMeter<'_>,
) -> Result<bool, CayleyError> {
    if first_shared >= 3 || second_shared >= 3 {
        return Ok(false);
    }
    if !topological_vertex_is_exactly_coincident(
        &first.points[first_shared],
        &second.points[second_shared],
        meter,
    )? {
        return Ok(false);
    }
    let shared = ActualMillimetrePointInterval {
        coordinates: try_array3(|axis| {
            ClosedRationalInterval::ordered(
                meter.clone_rational(
                    &first.points[first_shared].coordinates[axis].lower,
                    CayleyStage::Containment,
                )?,
                meter.clone_rational(
                    &first.points[first_shared].coordinates[axis].upper,
                    CayleyStage::Containment,
                )?,
                meter,
            )
        })?,
    };
    let first_points = std::array::from_fn(|index| {
        if index == first_shared {
            &shared
        } else {
            &first.points[index]
        }
    });
    let second_points = std::array::from_fn(|index| {
        if index == second_shared {
            &shared
        } else {
            &second.points[index]
        }
    });
    let Some(first_cut) =
        opposite_edge_plane_cut(&first_points, first_shared, &second_points, meter)?
    else {
        return Ok(false);
    };
    let Some(second_cut) =
        opposite_edge_plane_cut(&second_points, second_shared, &first_points, meter)?
    else {
        return Ok(false);
    };
    let first_direction = first_cut.subtract(&shared, meter)?;
    let second_direction = second_cut.subtract(&shared, meter)?;
    Ok(
        dot(&first_direction, &second_direction, meter)?.strict_sign(meter)?
            == Some(StrictSign::Positive),
    )
}

fn topological_vertex_is_exactly_coincident(
    first: &ActualMillimetrePointInterval,
    second: &ActualMillimetrePointInterval,
    meter: &mut WorkMeter<'_>,
) -> Result<bool, CayleyError> {
    for axis in 0..3 {
        if meter.compare_rational(
            &first.coordinates[axis].lower,
            &first.coordinates[axis].upper,
            CayleyStage::Containment,
        )? != Ordering::Equal
            || meter.compare_rational(
                &second.coordinates[axis].lower,
                &second.coordinates[axis].upper,
                CayleyStage::Containment,
            )? != Ordering::Equal
            || meter.compare_rational(
                &first.coordinates[axis].lower,
                &second.coordinates[axis].lower,
                CayleyStage::Containment,
            )? != Ordering::Equal
        {
            return Ok(false);
        }
    }
    Ok(true)
}

fn opposite_edge_plane_cut(
    source: &[&ActualMillimetrePointInterval; 3],
    shared_vertex: usize,
    target: &[&ActualMillimetrePointInterval; 3],
    meter: &mut WorkMeter<'_>,
) -> Result<Option<ActualMillimetrePointInterval>, CayleyError> {
    let start = source[(shared_vertex + 1) % 3];
    let end = source[(shared_vertex + 2) % 3];
    let start_distance = signed_plane_distance_points(target, start, meter)?;
    let end_distance = signed_plane_distance_points(target, end, meter)?;
    let Some(start_sign) = start_distance.strict_sign(meter)? else {
        return Ok(None);
    };
    let Some(end_sign) = end_distance.strict_sign(meter)? else {
        return Ok(None);
    };
    if start_sign == end_sign {
        return Ok(None);
    }
    let denominator = start_distance.subtract(&end_distance, meter)?;
    if denominator.contains_zero(meter)? {
        return Ok(None);
    }
    let Some(parameter) = start_distance.divide(&denominator, meter)? else {
        return Ok(None);
    };
    if !parameter.strictly_inside_unit_interval(meter)? {
        return Ok(None);
    }
    start.interpolate(end, &parameter, meter).map(Some)
}

fn signed_plane_distance_points(
    triangle: &[&ActualMillimetrePointInterval; 3],
    point: &ActualMillimetrePointInterval,
    meter: &mut WorkMeter<'_>,
) -> Result<ClosedRationalInterval, CayleyError> {
    let first = triangle[1].subtract(triangle[0], meter)?;
    let second = triangle[2].subtract(triangle[0], meter)?;
    let offset = point.subtract(triangle[0], meter)?;
    determinant(&first, &second, &offset, meter)
}

fn dot(
    first: &[ClosedRationalInterval; 3],
    second: &[ClosedRationalInterval; 3],
    meter: &mut WorkMeter<'_>,
) -> Result<ClosedRationalInterval, CayleyError> {
    let first_term = first[0].multiply(&second[0], meter)?;
    let second_term = first[1].multiply(&second[1], meter)?;
    let third_term = first[2].multiply(&second[2], meter)?;
    first_term.add(&second_term, meter)?.add(&third_term, meter)
}

fn signed_plane_distance(
    triangle: &ActualMillimetreTriangleEnvelope,
    point: &ActualMillimetrePointInterval,
    meter: &mut WorkMeter<'_>,
) -> Result<ClosedRationalInterval, CayleyError> {
    let first = triangle.points[1].subtract(&triangle.points[0], meter)?;
    let second = triangle.points[2].subtract(&triangle.points[0], meter)?;
    let offset = point.subtract(&triangle.points[0], meter)?;
    determinant(&first, &second, &offset, meter)
}

fn stable_projection(
    triangle: &ActualMillimetreTriangleEnvelope,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<[usize; 2]>, CayleyError> {
    let first = triangle.points[1].subtract(&triangle.points[0], meter)?;
    let second = triangle.points[2].subtract(&triangle.points[0], meter)?;
    let normal = cross(&first, &second, meter)?;
    for (axis, component) in normal.iter().enumerate() {
        if component.strict_sign(meter)?.is_some() {
            return Ok(Some(projected_axes(axis)));
        }
    }
    Ok(None)
}

fn point_strictly_inside_triangle_projection(
    point: &ActualMillimetrePointInterval,
    triangle: &ActualMillimetreTriangleEnvelope,
    meter: &mut WorkMeter<'_>,
) -> Result<bool, CayleyError> {
    let Some([first_axis, second_axis]) = stable_projection(triangle, meter)? else {
        return Ok(false);
    };
    for edge in 0..3 {
        let start = &triangle.points[edge];
        let end = &triangle.points[(edge + 1) % 3];
        let opposite = &triangle.points[(edge + 2) % 3];
        let point_side = projected_orientation(start, end, point, first_axis, second_axis, meter)?;
        let interior_side =
            projected_orientation(start, end, opposite, first_axis, second_axis, meter)?;
        let Some(point_sign) = point_side.strict_sign(meter)? else {
            return Ok(false);
        };
        let Some(interior_sign) = interior_side.strict_sign(meter)? else {
            return Ok(false);
        };
        if point_sign != interior_sign {
            return Ok(false);
        }
    }
    Ok(true)
}

fn projected_orientation(
    start: &ActualMillimetrePointInterval,
    end: &ActualMillimetrePointInterval,
    point: &ActualMillimetrePointInterval,
    first_axis: usize,
    second_axis: usize,
    meter: &mut WorkMeter<'_>,
) -> Result<ClosedRationalInterval, CayleyError> {
    let line_first = end.coordinates[first_axis].subtract(&start.coordinates[first_axis], meter)?;
    let line_second =
        end.coordinates[second_axis].subtract(&start.coordinates[second_axis], meter)?;
    let point_first =
        point.coordinates[first_axis].subtract(&start.coordinates[first_axis], meter)?;
    let point_second =
        point.coordinates[second_axis].subtract(&start.coordinates[second_axis], meter)?;
    let first_product = line_first.multiply(&point_second, meter)?;
    let second_product = line_second.multiply(&point_first, meter)?;
    first_product.subtract(&second_product, meter)
}

fn determinant(
    first: &[ClosedRationalInterval; 3],
    second: &[ClosedRationalInterval; 3],
    third: &[ClosedRationalInterval; 3],
    meter: &mut WorkMeter<'_>,
) -> Result<ClosedRationalInterval, CayleyError> {
    let crossed = cross(second, third, meter)?;
    let first_term = first[0].multiply(&crossed[0], meter)?;
    let second_term = first[1].multiply(&crossed[1], meter)?;
    let third_term = first[2].multiply(&crossed[2], meter)?;
    first_term.add(&second_term, meter)?.add(&third_term, meter)
}

fn cross(
    first: &[ClosedRationalInterval; 3],
    second: &[ClosedRationalInterval; 3],
    meter: &mut WorkMeter<'_>,
) -> Result<[ClosedRationalInterval; 3], CayleyError> {
    try_array3(|axis| {
        let next = (axis + 1) % 3;
        let last = (axis + 2) % 3;
        let forward = first[next].multiply(&second[last], meter)?;
        let backward = first[last].multiply(&second[next], meter)?;
        forward.subtract(&backward, meter)
    })
}

const fn projected_axes(drop_axis: usize) -> [usize; 2] {
    match drop_axis {
        0 => [1, 2],
        1 => [0, 2],
        _ => [0, 1],
    }
}

fn try_array3<T>(
    mut element: impl FnMut(usize) -> Result<T, CayleyError>,
) -> Result<[T; 3], CayleyError> {
    Ok([element(0)?, element(1)?, element(2)?])
}

#[cfg(test)]
mod tests {
    use num_bigint::BigInt;
    use num_traits::{One, Zero};

    use super::super::super::stress_tests::{
        PreparedFixture, corner_mountain_valley_400mm_fixture,
        corner_mountain_valley_400mm_fixture_with_reversed_source_collections, exact_fixture_pose,
        midpoint_mountain_400mm_fixture,
        midpoint_mountain_400mm_fixture_with_reversed_source_collections, solve_fixture,
        triangle_and_quadrilateral_fixture,
    };
    use super::super::{MeasuredEnvelopeLimits, measure_binary64_affine_envelope};
    use super::*;

    fn rational(numerator: i64, denominator: i64) -> BigRational {
        BigRational::new(BigInt::from(numerator), BigInt::from(denominator))
    }

    fn point(x: i64, y: i64, z: i64) -> ActualMillimetrePointInterval {
        ActualMillimetrePointInterval {
            coordinates: [x, y, z].map(|value| {
                ClosedRationalInterval::point(BigRational::from_integer(value.into()))
            }),
        }
    }

    fn inflated_point(coordinates: [i64; 3], radius: BigRational) -> ActualMillimetrePointInterval {
        ActualMillimetrePointInterval {
            coordinates: coordinates.map(|value| {
                ClosedRationalInterval::inflated(
                    BigRational::from_integer(value.into()),
                    radius.clone(),
                )
            }),
        }
    }

    fn horizontal_target() -> ActualMillimetreTriangleEnvelope {
        ActualMillimetreTriangleEnvelope {
            points: [point(-2, -2, 0), point(2, -2, 0), point(0, 2, 0)],
        }
    }

    fn robust_piercer() -> ActualMillimetreTriangleEnvelope {
        ActualMillimetreTriangleEnvelope {
            points: [point(0, 0, -1), point(0, 0, 1), point(1, 0, 1)],
        }
    }

    fn inflated_triangle(
        triangle: &ActualMillimetreTriangleEnvelope,
        radius: BigRational,
    ) -> ActualMillimetreTriangleEnvelope {
        ActualMillimetreTriangleEnvelope {
            points: triangle
                .points
                .clone()
                .map(|point| ActualMillimetrePointInterval {
                    coordinates: point.coordinates.map(|coordinate| {
                        ClosedRationalInterval::inflated(coordinate.lower, radius.clone())
                    }),
                }),
        }
    }

    const TRIANGLE_PERMUTATIONS: [[usize; 3]; 6] = [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ];

    fn permuted_triangle(
        triangle: &ActualMillimetreTriangleEnvelope,
        permutation: [usize; 3],
    ) -> ActualMillimetreTriangleEnvelope {
        ActualMillimetreTriangleEnvelope {
            points: permutation.map(|index| triangle.points[index].clone()),
        }
    }

    fn transformed_triangle(
        triangle: &ActualMillimetreTriangleEnvelope,
        scale: BigRational,
        translation: [BigRational; 3],
    ) -> ActualMillimetreTriangleEnvelope {
        assert!(scale.is_positive());
        ActualMillimetreTriangleEnvelope {
            points: triangle
                .points
                .clone()
                .map(|point| ActualMillimetrePointInterval {
                    coordinates: std::array::from_fn(|axis| ClosedRationalInterval {
                        lower: &point.coordinates[axis].lower * &scale + &translation[axis],
                        upper: &point.coordinates[axis].upper * &scale + &translation[axis],
                    }),
                }),
        }
    }

    fn coordinate_permuted_and_reflected_triangle(
        triangle: &ActualMillimetreTriangleEnvelope,
        permutation: [usize; 3],
        reflection_mask: u8,
    ) -> ActualMillimetreTriangleEnvelope {
        ActualMillimetreTriangleEnvelope {
            points: triangle
                .points
                .clone()
                .map(|point| ActualMillimetrePointInterval {
                    coordinates: std::array::from_fn(|axis| {
                        let source = &point.coordinates[permutation[axis]];
                        if reflection_mask & (1 << axis) == 0 {
                            source.clone()
                        } else {
                            ClosedRationalInterval {
                                lower: -source.upper.clone(),
                                upper: -source.lower.clone(),
                            }
                        }
                    }),
                }),
        }
    }

    #[test]
    fn robust_actual_mm_transversal_is_the_only_positive_result() {
        let radius = rational(1, 10_000);
        let first = inflated_triangle(&robust_piercer(), radius.clone());
        let second = inflated_triangle(&horizontal_target(), radius);
        assert_eq!(
            classify_transversal_blocking_only(
                &first,
                &second,
                PairTopology::NoSharedFeature,
                CayleyLimits::default(),
            ),
            BlockingOnlyDecision::ProvenPenetrating
        );
        assert_eq!(
            classify_transversal_blocking_only(
                &second,
                &first,
                PairTopology::NoSharedFeature,
                CayleyLimits::default(),
            ),
            BlockingOnlyDecision::ProvenPenetrating
        );
    }

    #[test]
    fn point_line_and_coplanar_contact_never_become_penetration() {
        let target = horizontal_target();
        let point_touch = ActualMillimetreTriangleEnvelope {
            points: [point(0, 0, 0), point(0, 0, 1), point(1, 0, 1)],
        };
        let line_touch = ActualMillimetreTriangleEnvelope {
            points: [point(-1, -2, 0), point(1, -2, 0), point(0, -2, 1)],
        };
        let coplanar = ActualMillimetreTriangleEnvelope {
            points: [point(-1, -1, 0), point(1, -1, 0), point(0, 1, 0)],
        };
        for candidate in [&point_touch, &line_touch, &coplanar] {
            assert_eq!(
                classify_transversal_blocking_only(
                    candidate,
                    &target,
                    PairTopology::NoSharedFeature,
                    CayleyLimits::default(),
                ),
                BlockingOnlyDecision::Unresolved
            );
        }
    }

    #[test]
    fn shared_vertex_excludes_only_incident_edges_and_never_shared_point_contact() {
        let target = horizontal_target();
        let strict_away_from_shared = ActualMillimetreTriangleEnvelope {
            points: [point(-2, -2, 0), point(0, 0, -1), point(0, 0, 1)],
        };
        assert_eq!(
            classify_transversal_blocking_only(
                &strict_away_from_shared,
                &target,
                PairTopology::SharedVertex {
                    first_vertex: 0,
                    second_vertex: 0,
                },
                CayleyLimits::default(),
            ),
            BlockingOnlyDecision::ProvenPenetrating
        );

        let shared_point_only = ActualMillimetreTriangleEnvelope {
            points: [point(-2, -2, 0), point(-2, -2, 1), point(-1, -2, 1)],
        };
        assert_eq!(
            classify_transversal_blocking_only(
                &shared_point_only,
                &target,
                PairTopology::SharedVertex {
                    first_vertex: 0,
                    second_vertex: 0,
                },
                CayleyLimits::default(),
            ),
            BlockingOnlyDecision::Unresolved
        );

        for first_permutation in TRIANGLE_PERMUTATIONS {
            for second_permutation in TRIANGLE_PERMUTATIONS {
                let first = permuted_triangle(&strict_away_from_shared, first_permutation);
                let second = permuted_triangle(&target, second_permutation);
                let first_vertex = first_permutation
                    .iter()
                    .position(|source| *source == 0)
                    .expect("shared first vertex");
                let second_vertex = second_permutation
                    .iter()
                    .position(|source| *source == 0)
                    .expect("shared second vertex");
                assert_eq!(
                    classify_transversal_blocking_only(
                        &first,
                        &second,
                        PairTopology::SharedVertex {
                            first_vertex,
                            second_vertex,
                        },
                        CayleyLimits::default(),
                    ),
                    BlockingOnlyDecision::ProvenPenetrating
                );
            }
        }
    }

    #[test]
    fn shared_vertex_positive_length_interior_segment_is_not_only_boundary_contact() {
        let first = ActualMillimetreTriangleEnvelope {
            points: [point(0, 0, 0), point(2, -1, 0), point(2, 1, 0)],
        };
        let same_ray = ActualMillimetreTriangleEnvelope {
            points: [point(0, 0, 0), point(2, 0, -1), point(2, 0, 1)],
        };
        let opposite_ray = ActualMillimetreTriangleEnvelope {
            points: [point(0, 0, 0), point(-2, 0, -1), point(-2, 0, 1)],
        };
        let topology = PairTopology::SharedVertex {
            first_vertex: 0,
            second_vertex: 0,
        };
        assert_eq!(
            classify_transversal_blocking_only(
                &first,
                &same_ray,
                topology,
                CayleyLimits::default(),
            ),
            BlockingOnlyDecision::ProvenPenetrating
        );
        assert_eq!(
            classify_transversal_blocking_only(
                &first,
                &opposite_ray,
                topology,
                CayleyLimits::default(),
            ),
            BlockingOnlyDecision::Unresolved
        );
        assert_eq!(
            classify_transversal_blocking_only(
                &inflated_triangle(&first, rational(1, 1_000)),
                &inflated_triangle(&same_ray, rational(1, 1_000)),
                topology,
                CayleyLimits::default(),
            ),
            BlockingOnlyDecision::Unresolved
        );
        assert_eq!(
            classify_transversal_blocking_only(
                &inflated_triangle(&first, BigRational::from_integer(3.into())),
                &inflated_triangle(&same_ray, BigRational::from_integer(3.into())),
                topology,
                CayleyLimits::default(),
            ),
            BlockingOnlyDecision::Unresolved
        );

        for first_permutation in TRIANGLE_PERMUTATIONS {
            for second_permutation in TRIANGLE_PERMUTATIONS {
                let reordered_first = permuted_triangle(&first, first_permutation);
                let reordered_second = permuted_triangle(&same_ray, second_permutation);
                let first_vertex = first_permutation
                    .iter()
                    .position(|source| *source == 0)
                    .expect("first shared vertex");
                let second_vertex = second_permutation
                    .iter()
                    .position(|source| *source == 0)
                    .expect("second shared vertex");
                for (left, left_vertex, right, right_vertex) in [
                    (
                        &reordered_first,
                        first_vertex,
                        &reordered_second,
                        second_vertex,
                    ),
                    (
                        &reordered_second,
                        second_vertex,
                        &reordered_first,
                        first_vertex,
                    ),
                ] {
                    assert_eq!(
                        classify_transversal_blocking_only(
                            left,
                            right,
                            PairTopology::SharedVertex {
                                first_vertex: left_vertex,
                                second_vertex: right_vertex,
                            },
                            CayleyLimits::default(),
                        ),
                        BlockingOnlyDecision::ProvenPenetrating
                    );
                }
            }
        }
        for coordinate_permutation in TRIANGLE_PERMUTATIONS {
            for reflection_mask in 0..8 {
                let transformed_first = coordinate_permuted_and_reflected_triangle(
                    &first,
                    coordinate_permutation,
                    reflection_mask,
                );
                let transformed_second = coordinate_permuted_and_reflected_triangle(
                    &same_ray,
                    coordinate_permutation,
                    reflection_mask,
                );
                assert_eq!(
                    classify_transversal_blocking_only(
                        &transformed_first,
                        &transformed_second,
                        topology,
                        CayleyLimits::default(),
                    ),
                    BlockingOnlyDecision::ProvenPenetrating
                );
            }
        }
    }

    #[test]
    fn independent_shared_vertex_boxes_are_never_welded_into_a_false_transversal() {
        let first = ActualMillimetreTriangleEnvelope {
            points: [point(0, 0, 0), point(5, -2, 0), point(5, 2, 0)],
        };
        let second = ActualMillimetreTriangleEnvelope {
            points: [
                point(0, 0, 0),
                ActualMillimetrePointInterval {
                    coordinates: [
                        ClosedRationalInterval::point(rational(1, 4)),
                        ClosedRationalInterval::point(BigRational::zero()),
                        ClosedRationalInterval::point(rational(-1, 8)),
                    ],
                },
                ActualMillimetrePointInterval {
                    coordinates: [
                        ClosedRationalInterval::point(rational(1, 4)),
                        ClosedRationalInterval::point(BigRational::zero()),
                        ClosedRationalInterval::point(rational(1, 8)),
                    ],
                },
            ],
        };
        let topology = PairTopology::SharedVertex {
            first_vertex: 0,
            second_vertex: 0,
        };
        assert_eq!(
            classify_transversal_blocking_only(&first, &second, topology, CayleyLimits::default(),),
            BlockingOnlyDecision::ProvenPenetrating
        );

        let half = rational(1, 2);
        let wide_first = ActualMillimetreTriangleEnvelope {
            points: first.points.clone().map(|point| {
                let mut coordinates = point.coordinates;
                coordinates[1] =
                    ClosedRationalInterval::inflated(coordinates[1].lower.clone(), half.clone());
                ActualMillimetrePointInterval { coordinates }
            }),
        };
        assert_eq!(
            classify_transversal_blocking_only(
                &wide_first,
                &second,
                topology,
                CayleyLimits::default(),
            ),
            BlockingOnlyDecision::Unresolved,
            "an independent per-face box cannot manufacture a common vertex"
        );

        let shifted_first = transformed_triangle(
            &first,
            BigRational::one(),
            [BigRational::zero(), -half, BigRational::zero()],
        );
        assert_eq!(
            classify_transversal_blocking_only(
                &shifted_first,
                &second,
                topology,
                CayleyLimits::default(),
            ),
            BlockingOnlyDecision::Unresolved,
            "this concrete affine realization inside the wide box is separated"
        );
    }

    #[test]
    fn shared_hinge_same_face_and_invalid_shared_vertex_are_sealed_unresolved() {
        let first = robust_piercer();
        let second = horizontal_target();
        for topology in [
            PairTopology::SharedHinge,
            PairTopology::SameFace,
            PairTopology::SharedVertex {
                first_vertex: 3,
                second_vertex: 0,
            },
        ] {
            assert_eq!(
                classify_transversal_blocking_only(
                    &first,
                    &second,
                    topology,
                    CayleyLimits::default(),
                ),
                BlockingOnlyDecision::Unresolved
            );
        }
    }

    #[test]
    fn denominator_parameter_and_interior_margins_must_all_be_strict() {
        let target = horizontal_target();
        let uncertain_endpoint = ActualMillimetreTriangleEnvelope {
            points: [
                inflated_point([0, 0, 0], BigRational::one()),
                point(0, 0, 1),
                point(1, 0, 1),
            ],
        };
        let boundary_piercer = ActualMillimetreTriangleEnvelope {
            points: [point(0, -2, -1), point(0, -2, 1), point(1, -2, 1)],
        };
        let zero_width = ActualMillimetreTriangleEnvelope {
            points: [
                point(0, 0, -1),
                ActualMillimetrePointInterval {
                    coordinates: [
                        ClosedRationalInterval::point(BigRational::zero()),
                        ClosedRationalInterval::point(BigRational::zero()),
                        ClosedRationalInterval {
                            lower: -BigRational::one(),
                            upper: BigRational::one(),
                        },
                    ],
                },
                point(1, 0, 1),
            ],
        };
        for candidate in [&uncertain_endpoint, &boundary_piercer, &zero_width] {
            assert_eq!(
                classify_transversal_blocking_only(
                    candidate,
                    &target,
                    PairTopology::NoSharedFeature,
                    CayleyLimits::default(),
                ),
                BlockingOnlyDecision::Unresolved
            );
        }
    }

    #[test]
    fn every_vertex_permutation_and_pair_direction_preserves_the_result() {
        let first = robust_piercer();
        let second = horizontal_target();
        for first_permutation in TRIANGLE_PERMUTATIONS {
            for second_permutation in TRIANGLE_PERMUTATIONS {
                let reordered_first = permuted_triangle(&first, first_permutation);
                let reordered_second = permuted_triangle(&second, second_permutation);
                for (left, right) in [
                    (&reordered_first, &reordered_second),
                    (&reordered_second, &reordered_first),
                ] {
                    assert_eq!(
                        classify_transversal_blocking_only(
                            left,
                            right,
                            PairTopology::NoSharedFeature,
                            CayleyLimits::default(),
                        ),
                        BlockingOnlyDecision::ProvenPenetrating
                    );
                }
            }
        }
    }

    #[test]
    fn every_coordinate_axis_and_reflection_preserves_the_robust_result() {
        let first = robust_piercer();
        let second = horizontal_target();
        let mut observed_normal_axes = [false; 3];
        for coordinate_permutation in TRIANGLE_PERMUTATIONS {
            let normal_axis = coordinate_permutation
                .iter()
                .position(|source_axis| *source_axis == 2)
                .expect("original Z normal must remain on one coordinate axis");
            observed_normal_axes[normal_axis] = true;
            for reflection_mask in 0..8 {
                let transformed_first = coordinate_permuted_and_reflected_triangle(
                    &first,
                    coordinate_permutation,
                    reflection_mask,
                );
                let transformed_second = coordinate_permuted_and_reflected_triangle(
                    &second,
                    coordinate_permutation,
                    reflection_mask,
                );
                for (left, right) in [
                    (&transformed_first, &transformed_second),
                    (&transformed_second, &transformed_first),
                ] {
                    assert_eq!(
                        classify_transversal_blocking_only(
                            left,
                            right,
                            PairTopology::NoSharedFeature,
                            CayleyLimits::default(),
                        ),
                        BlockingOnlyDecision::ProvenPenetrating,
                        "coordinate permutation {coordinate_permutation:?}, reflection mask \
                         {reflection_mask:#05b}",
                    );
                }
            }
        }
        assert_eq!(observed_normal_axes, [true; 3]);
    }

    #[test]
    fn positive_scale_translation_and_strict_radius_margin_are_invariant() {
        let first = robust_piercer();
        let second = horizontal_target();
        for (scale, translation) in [
            (
                rational(1, 100),
                [
                    BigRational::from_integer((-1_000_000_000_000_i64).into()),
                    rational(7, 3),
                    BigRational::from_integer(1_000_000_000_000_i64.into()),
                ],
            ),
            (
                BigRational::one(),
                [
                    BigRational::zero(),
                    BigRational::zero(),
                    BigRational::zero(),
                ],
            ),
            (
                BigRational::from_integer(1_000_000_i64.into()),
                [
                    BigRational::from_integer(1_000_000_000_000_i64.into()),
                    rational(-11, 5),
                    BigRational::from_integer((-1_000_000_000_000_i64).into()),
                ],
            ),
        ] {
            let transformed_first =
                transformed_triangle(&first, scale.clone(), translation.clone());
            let transformed_second = transformed_triangle(&second, scale, translation);
            assert_eq!(
                classify_transversal_blocking_only(
                    &transformed_first,
                    &transformed_second,
                    PairTopology::NoSharedFeature,
                    CayleyLimits::default(),
                ),
                BlockingOnlyDecision::ProvenPenetrating
            );
        }

        for (radius, expected) in [
            (rational(1, 10_000), BlockingOnlyDecision::ProvenPenetrating),
            (BigRational::one(), BlockingOnlyDecision::Unresolved),
        ] {
            assert_eq!(
                classify_transversal_blocking_only(
                    &inflated_triangle(&first, radius.clone()),
                    &inflated_triangle(&second, radius),
                    PairTopology::NoSharedFeature,
                    CayleyLimits::default(),
                ),
                expected
            );
        }
    }

    #[test]
    fn strict_radius_threshold_has_a_certified_rational_bracket() {
        let first = robust_piercer();
        let second = horizontal_target();
        let denominator = 1_000_000_000_000_000_i64;
        let below = rational(66_521_651_620_659, denominator);
        let above = rational(66_521_651_620_660, denominator);

        // At this fixture's first edge, the strict t.upper < 1 margin changes
        // sign with 16r^3 + 46r^2 + 57r - 4. Its unique positive root lies
        // inside this one-quadrillionth-wide rational bracket.
        let threshold_polynomial = |radius: &BigRational| {
            BigRational::from_integer(16.into()) * radius.pow(3)
                + BigRational::from_integer(46.into()) * radius.pow(2)
                + BigRational::from_integer(57.into()) * radius
                - BigRational::from_integer(4.into())
        };
        assert!(threshold_polynomial(&below).is_negative());
        assert!(threshold_polynomial(&above).is_positive());

        // The rational-root theorem leaves only these positive candidates.
        // None is a root, so an exact equality fixture cannot be represented
        // by the BigRational radius accepted by this primitive.
        for candidate in [
            rational(1, 16),
            rational(1, 8),
            rational(1, 4),
            rational(1, 2),
            BigRational::one(),
            rational(2, 1),
            rational(4, 1),
        ] {
            assert!(!threshold_polynomial(&candidate).is_zero());
        }

        assert_eq!(
            classify_transversal_blocking_only(
                &inflated_triangle(&first, below.clone()),
                &inflated_triangle(&second, below),
                PairTopology::NoSharedFeature,
                CayleyLimits::default(),
            ),
            BlockingOnlyDecision::ProvenPenetrating,
        );
        assert_eq!(
            classify_transversal_blocking_only(
                &inflated_triangle(&first, above.clone()),
                &inflated_triangle(&second, above),
                PairTopology::NoSharedFeature,
                CayleyLimits::default(),
            ),
            BlockingOnlyDecision::Unresolved,
        );
    }

    #[test]
    fn malformed_interval_or_noncanonical_ratio_is_unresolved() {
        let second = horizontal_target();

        let mut reversed = robust_piercer();
        reversed.points[0].coordinates[0] = ClosedRationalInterval {
            lower: BigRational::one(),
            upper: BigRational::zero(),
        };

        let mut noncanonical = robust_piercer();
        noncanonical.points[0].coordinates[0].lower = BigRational::new_raw(2.into(), 2.into());

        let mut negative_denominator = robust_piercer();
        negative_denominator.points[0].coordinates[0].lower =
            BigRational::new_raw(1.into(), (-1).into());

        for malformed in [&reversed, &noncanonical, &negative_denominator] {
            assert_eq!(
                classify_transversal_blocking_only(
                    malformed,
                    &second,
                    PairTopology::NoSharedFeature,
                    CayleyLimits::default(),
                ),
                BlockingOnlyDecision::Unresolved
            );
        }
    }

    #[test]
    fn degenerate_triangles_and_inflated_shared_point_contact_are_unresolved() {
        let robust = robust_piercer();
        let collinear = ActualMillimetreTriangleEnvelope {
            points: [point(-2, 0, 0), point(0, 0, 0), point(2, 0, 0)],
        };
        let collapsed = ActualMillimetreTriangleEnvelope {
            points: [point(0, 0, 0), point(0, 0, 0), point(0, 0, 0)],
        };
        for degenerate in [&collinear, &collapsed] {
            for (first, second) in [(&robust, degenerate), (degenerate, &robust)] {
                assert_eq!(
                    classify_transversal_blocking_only(
                        first,
                        second,
                        PairTopology::NoSharedFeature,
                        CayleyLimits::default(),
                    ),
                    BlockingOnlyDecision::Unresolved,
                );
            }
        }

        let radius = rational(1, 10_000);
        let target = inflated_triangle(&horizontal_target(), radius.clone());
        let shared_point_only = inflated_triangle(
            &ActualMillimetreTriangleEnvelope {
                points: [point(-2, -2, 0), point(-2, -2, 1), point(-1, -2, 1)],
            },
            radius,
        );
        for (first, second) in [(&shared_point_only, &target), (&target, &shared_point_only)] {
            assert_eq!(
                classify_transversal_blocking_only(
                    first,
                    second,
                    PairTopology::SharedVertex {
                        first_vertex: 0,
                        second_vertex: 0,
                    },
                    CayleyLimits::default(),
                ),
                BlockingOnlyDecision::Unresolved,
            );
        }
    }

    #[test]
    fn a_later_resource_failure_revokes_an_already_observed_witness() {
        let first = robust_piercer();
        let second = horizontal_target();
        let default_limits = CayleyLimits::default();
        let mut prefix_meter = WorkMeter::new(&default_limits);
        assert!(valid_triangle_input(&first, &mut prefix_meter).expect("first input"));
        assert!(valid_triangle_input(&second, &mut prefix_meter).expect("second input"));
        assert!(
            stable_projection(&first, &mut prefix_meter)
                .expect("first projection")
                .is_some()
        );
        assert!(
            stable_projection(&second, &mut prefix_meter)
                .expect("second projection")
                .is_some()
        );
        assert!(
            edge_strictly_pierces_triangle(&first, 0, &second, &mut prefix_meter)
                .expect("first edge witness")
        );
        let operations_after_witness = prefix_meter.work.interval_operations;

        let mut one_short_for_the_next_operation = default_limits;
        one_short_for_the_next_operation.max_interval_operations = operations_after_witness;
        let mut exact_prefix_meter = WorkMeter::new(&one_short_for_the_next_operation);
        assert!(valid_triangle_input(&first, &mut exact_prefix_meter).expect("first exact input"));
        assert!(
            valid_triangle_input(&second, &mut exact_prefix_meter).expect("second exact input")
        );
        assert!(
            stable_projection(&first, &mut exact_prefix_meter)
                .expect("first exact projection")
                .is_some()
        );
        assert!(
            stable_projection(&second, &mut exact_prefix_meter)
                .expect("second exact projection")
                .is_some()
        );
        assert!(
            edge_strictly_pierces_triangle(&first, 0, &second, &mut exact_prefix_meter)
                .expect("witness at the exact prefix limit")
        );
        assert_eq!(
            exact_prefix_meter.work.interval_operations,
            operations_after_witness
        );
        assert!(matches!(
            edge_strictly_pierces_triangle(&first, 1, &second, &mut exact_prefix_meter),
            Err(CayleyError::ResourceLimitExceeded {
                stage: CayleyStage::Containment,
                resource: "interval_operations",
            })
        ));
        assert_eq!(
            classify_transversal_blocking_only(
                &first,
                &second,
                PairTopology::NoSharedFeature,
                one_short_for_the_next_operation,
            ),
            BlockingOnlyDecision::Unresolved,
        );
    }

    #[test]
    fn exact_resource_limit_accepts_observed_work_and_one_short_fails_closed() {
        let first = robust_piercer();
        let second = horizontal_target();
        let (_, work) = classify_transversal_metered(
            &first,
            &second,
            PairTopology::NoSharedFeature,
            CayleyLimits::default(),
        )
        .expect("baseline robust predicate");
        assert!(work.interval_operations > 0);

        let exact = CayleyLimits {
            max_interval_operations: work.interval_operations,
            max_intermediate_bits: work.max_preflight_bits.max(work.max_observed_bits),
            max_gcd_fallback_calls: work.gcd_fallback_calls,
            max_gcd_fallback_input_bits: work.gcd_fallback_input_bits,
            max_rational_allocations: work.rational_allocations,
            max_rational_allocation_bits: work.max_rational_allocation_bits,
            max_total_rational_allocation_bits: work.total_rational_allocation_bits,
            ..CayleyLimits::default()
        };
        assert_eq!(
            classify_transversal_metered(&first, &second, PairTopology::NoSharedFeature, exact,)
                .expect("exact observed resource limits")
                .0,
            BlockingOnlyDecision::ProvenPenetrating
        );

        macro_rules! one_short {
            ($field:ident) => {{
                let mut one_short = exact;
                assert!(
                    one_short.$field > 0,
                    "{} must be exercised",
                    stringify!($field)
                );
                one_short.$field -= 1;
                assert_eq!(
                    classify_transversal_blocking_only(
                        &first,
                        &second,
                        PairTopology::NoSharedFeature,
                        one_short,
                    ),
                    BlockingOnlyDecision::Unresolved,
                    "{} one-short must fail closed",
                    stringify!($field)
                );
            }};
        }

        one_short!(max_interval_operations);
        one_short!(max_intermediate_bits);
        one_short!(max_gcd_fallback_calls);
        one_short!(max_gcd_fallback_input_bits);
        one_short!(max_rational_allocations);
        one_short!(max_rational_allocation_bits);
        one_short!(max_total_rational_allocation_bits);
    }

    fn authenticated_fixture_decision_matrix(
        fixture: &PreparedFixture,
        cases: &[([f64; 2], usize)],
    ) -> Vec<([u64; 2], FaceId, Vec<AuthenticatedTrianglePairDecision>)> {
        let roots = fixture.model.face_ids();
        assert_eq!(roots.len(), 3);
        let mut matrix = Vec::new();
        for (angles, expected_proven) in cases {
            for root in roots {
                let pose = solve_fixture(fixture, *root, angles);
                let bound = fixture
                    .model
                    .bind_pose(&pose)
                    .expect("issuer-bound bridge pose");
                let exact = exact_fixture_pose(
                    fixture,
                    &pose,
                    super::super::super::ExactTreePoseLimits::default(),
                );
                let measured = measure_binary64_affine_envelope(
                    &exact,
                    bound,
                    MeasuredEnvelopeLimits::default(),
                )
                .expect("measured exact-pose envelope");
                let scan = scan_authenticated_triangle_pairs_blocking_only(
                    &exact,
                    &measured,
                    bound,
                    0.0,
                    AuthenticatedTriangleBlockingLimits::default(),
                )
                .expect("authenticated actual-mm blocking scan");
                assert!(scan.is_for(&exact, &measured, bound, 0.0));
                assert_eq!(scan.paper_thickness_bits, 0.0_f64.to_bits());
                assert_eq!(scan.pairs.len(), 3);
                assert_eq!(scan.work.faces, 3);
                assert_eq!(scan.work.unordered_face_pairs, 3);
                assert_eq!(scan.work.result_records, 3);
                assert_eq!(
                    scan.proven_penetrating_pairs(),
                    *expected_proven,
                    "angles {angles:?}, root {root:?}"
                );
                for record in &scan.pairs {
                    assert_eq!(
                        scan.decision(record.first, record.second),
                        Some(record.decision)
                    );
                    assert_eq!(
                        scan.decision(record.second, record.first),
                        Some(record.decision)
                    );
                }
                matrix.push((
                    [angles[0].to_bits(), angles[1].to_bits()],
                    *root,
                    scan.pairs,
                ));
            }
        }
        matrix
    }

    #[test]
    fn authenticated_400mm_bridge_fixes_corner_false_positives_and_deep_midpoint_negatives() {
        let corner_cases = [
            ([10.0, 0.0], 0),
            ([0.0, 10.0], 0),
            ([45.0, 45.0], 0),
            ([90.0, 90.0], 0),
            ([91.0, 91.0], 0),
            ([135.0, 135.0], 0),
            ([179.0, 179.0], 0),
            ([180.0, 180.0], 0),
        ];
        let corner_baseline = authenticated_fixture_decision_matrix(
            &corner_mountain_valley_400mm_fixture(0.0),
            &corner_cases,
        );
        let corner_reversed = authenticated_fixture_decision_matrix(
            &corner_mountain_valley_400mm_fixture_with_reversed_source_collections(0.0),
            &corner_cases,
        );
        assert_eq!(
            corner_reversed, corner_baseline,
            "source collection order must preserve every canonical pair decision"
        );

        let midpoint_cases = [
            ([90.0, 90.0], 0),
            ([91.0, 91.0], 0),
            ([135.0, 135.0], 1),
            ([179.0, 179.0], 1),
            ([180.0, 180.0], 0),
        ];
        let midpoint_baseline = authenticated_fixture_decision_matrix(
            &midpoint_mountain_400mm_fixture(),
            &midpoint_cases,
        );
        let midpoint_reversed = authenticated_fixture_decision_matrix(
            &midpoint_mountain_400mm_fixture_with_reversed_source_collections(),
            &midpoint_cases,
        );
        assert_eq!(
            midpoint_reversed, midpoint_baseline,
            "source collection order must preserve every canonical pair decision"
        );
    }

    #[test]
    fn bound_pose_facade_returns_only_an_owned_canonical_summary() {
        let fixture = midpoint_mountain_400mm_fixture();
        let root = fixture.model.face_ids()[1];
        let pose = solve_fixture(&fixture, root, &[135.0, 135.0]);
        let bound = fixture
            .model
            .bind_pose(&pose)
            .expect("issuer-bound facade pose");

        let summary = scan_bound_pose_for_proven_transversal_penetration(
            bound,
            ProvenTransversalScanLimits::default(),
        )
        .expect("sealed zero-thickness transversal scan");
        assert_eq!(
            summary,
            ProvenTransversalScanSummary {
                enumerated_pairs: 3,
                proven_transversal_pairs: 1,
                first_proven_transversal_pair: summary.first_proven_transversal_pair,
                proven_transversal_pair_ids: summary.proven_transversal_pair_ids.clone(),
            }
        );
        let (first, second) = summary
            .first_proven_transversal_pair
            .expect("fixture has one proven transversal pair");
        assert!(first.canonical_bytes() < second.canonical_bytes());
    }

    #[test]
    fn bound_pose_facade_collapses_internal_errors_to_three_atomic_classes() {
        assert_eq!(
            map_cayley_scan_error(CayleyError::CertificateUnavailable {
                stage: CayleyStage::Candidate,
            }),
            ProvenTransversalScanError::EvidenceUnavailable
        );
        assert_eq!(
            map_cayley_scan_error(CayleyError::ResourceLimitExceeded {
                stage: CayleyStage::Tree,
                resource: "faces",
            }),
            ProvenTransversalScanError::ResourceLimitExceeded
        );
        assert_eq!(
            map_cayley_scan_error(CayleyError::BoundTreeInconsistent {
                stage: CayleyStage::Tree,
            }),
            ProvenTransversalScanError::InconsistentPose
        );
        assert_eq!(
            map_measured_scan_error(MeasuredEnvelopeError::ResourceLimitExceeded {
                resource: "point_components",
            }),
            ProvenTransversalScanError::ResourceLimitExceeded
        );
        assert_eq!(
            map_measured_scan_error(MeasuredEnvelopeError::AuthorityMismatch),
            ProvenTransversalScanError::InconsistentPose
        );
        assert_eq!(
            map_authenticated_scan_error(AuthenticatedTriangleBlockingError::Exact(
                CayleyError::CertificateUnavailable {
                    stage: CayleyStage::Containment,
                },
            )),
            ProvenTransversalScanError::EvidenceUnavailable
        );
        assert_eq!(
            map_authenticated_scan_error(
                AuthenticatedTriangleBlockingError::UnsupportedPaperThickness,
            ),
            ProvenTransversalScanError::InconsistentPose
        );
    }

    #[test]
    fn facade_hard_defaults_cannot_be_expanded_by_oversized_caller_caps() {
        let hard = ProvenTransversalScanLimits::default();
        let expected =
            project_proven_transversal_scan_limits(hard).expect("hard default projection");

        macro_rules! assert_clamped {
            ($field:ident) => {{
                let mut oversized = hard;
                oversized.$field = usize::MAX;
                assert_eq!(
                    project_proven_transversal_scan_limits(oversized)
                        .expect("oversized cap must clamp"),
                    expected,
                    "{} expanded a private hard ceiling",
                    stringify!($field)
                );
            }};
        }

        assert_clamped!(max_faces);
        assert_clamped!(max_unordered_face_pairs);
        assert_clamped!(max_boundary_vertices_per_face);
        assert_clamped!(max_total_boundary_vertices);
        assert_clamped!(max_total_triangles);
        assert_clamped!(max_total_triangle_pairs);
        assert_clamped!(max_registry_authentication_work);
        assert_clamped!(max_total_boundary_relation_work);
        assert_clamped!(max_rational_input_bits);
        assert_clamped!(max_total_rational_input_storage_bits);
        assert_clamped!(max_total_rational_retained_clone_bits);
        assert_clamped!(max_rational_operations);
        assert_clamped!(max_rational_intermediate_bits);
        assert_clamped!(max_rational_gcd_fallback_calls);
        assert_clamped!(max_rational_gcd_fallback_input_bits);
        assert_clamped!(max_rational_allocations);
        assert_clamped!(max_rational_allocation_bits);
        assert_clamped!(max_total_rational_allocation_bits);
        assert_clamped!(max_rational_output_bits);
        assert_clamped!(max_total_rational_output_bits);
    }

    #[test]
    fn facade_rational_budget_is_sequential_and_every_one_short_is_atomic() {
        let fixture = midpoint_mountain_400mm_fixture();
        let root = fixture.model.face_ids()[1];
        let pose = solve_fixture(&fixture, root, &[135.0, 135.0]);
        let bound = fixture
            .model
            .bind_pose(&pose)
            .expect("issuer-bound sequential-budget pose");
        let hard = ProvenTransversalScanLimits::default();
        let projected = project_proven_transversal_scan_limits(hard).expect("hard projection");
        let exact =
            prepare_rational_cayley_tree_pose_v1(bound, projected.exact).expect("exact prefix");
        let measured_limits =
            measured_limits_after_exact(projected.measured, &projected.shared, &exact)
                .expect("measured remaining budget");
        assert_eq!(
            measured_limits.exact.max_rational_allocations,
            projected
                .measured
                .exact
                .max_rational_allocations
                .min(hard.max_rational_allocations - exact.work.exact.rational_allocations)
        );
        let measured = measure_binary64_affine_envelope(&exact, bound, measured_limits)
            .expect("measured prefix");
        let scan = scan_authenticated_triangle_pairs_blocking_only(
            &exact,
            &measured,
            bound,
            0.0,
            projected.blocking,
        )
        .expect("complete baseline scan");
        let exact_allocations = exact.work.exact.rational_allocations;
        let measured_allocations = measured.work.exact.rational_allocations;
        let prefix_allocations = exact_allocations
            .checked_add(measured_allocations)
            .expect("small fixture allocation sum");
        let total_allocations = scan.work.exact.rational_allocations;
        assert!(exact_allocations > 0);
        assert!(measured_allocations > 0);
        assert!(total_allocations > prefix_allocations);

        let mut zero = hard;
        zero.max_rational_allocations = 0;
        assert_eq!(
            scan_bound_pose_for_proven_transversal_penetration(bound, zero),
            Err(ProvenTransversalScanError::ResourceLimitExceeded)
        );

        let mut measured_one_short = hard;
        measured_one_short.max_rational_allocations = prefix_allocations - 1;
        assert_eq!(
            scan_bound_pose_for_proven_transversal_penetration(bound, measured_one_short),
            Err(ProvenTransversalScanError::ResourceLimitExceeded)
        );

        let mut blocking_one_short = hard;
        blocking_one_short.max_rational_allocations = total_allocations - 1;
        assert_eq!(
            scan_bound_pose_for_proven_transversal_penetration(bound, blocking_one_short),
            Err(ProvenTransversalScanError::ResourceLimitExceeded)
        );

        let mut underflow = projected.shared;
        underflow.max_rational_allocations = exact_allocations - 1;
        assert_eq!(
            measured_limits_after_exact(projected.measured, &underflow, &exact),
            Err(ProvenTransversalScanError::ResourceLimitExceeded)
        );
    }

    fn observed_three_stage_registry_work(
        exact: &RationalCayleyTreePose<'_>,
        measured: &MeasuredBinary64AffineEnvelope<'_, '_>,
        scan: &AuthenticatedTriangleBlockingScan<'_, '_, '_>,
    ) -> usize {
        [
            exact.work.adjacency_entries,
            exact.work.boundary_occurrences,
            exact.work.boundary_edge_index_entries,
            exact.work.boundary_edge_index_operations,
            exact.work.unique_vertices,
            measured.work.adjacency_entries,
            measured.work.total_depth,
            measured.work.transform_scalars,
            measured.work.boundary_occurrences,
            measured.work.point_components,
            measured.work.unique_vertices,
            measured.work.shared_occurrence_checks,
            measured.work.hinge_feature_points,
            measured.work.exact_hinge_path_checks,
            measured.work.binary64_hinge_path_checks,
            measured.work.hinge_component_checks,
            measured.work.hinge_transform_checks,
            measured.work.certificate_reads,
            measured.work.exact_point_transforms,
            measured.work.input_scalars,
            scan.work.boundary_occurrences,
            scan.work.hinge_boundary_index_entries,
            scan.work.hinge_relation_records,
            scan.work.hinge_face_lookups,
            scan.work.hinge_vertex_lookups,
        ]
        .into_iter()
        .try_fold(0_usize, usize::checked_add)
        .expect("small fixture registry work")
    }

    fn observed_three_stage_boundary_relation_work(
        scan: &AuthenticatedTriangleBlockingScan<'_, '_, '_>,
    ) -> usize {
        [
            scan.work.topology_vertex_checks,
            scan.work.exact_predicate_calls,
            scan.work.observed_predicate_calls,
            scan.work.result_records,
        ]
        .into_iter()
        .try_fold(0_usize, usize::checked_add)
        .expect("small fixture boundary relation work")
    }

    fn preflight_with_projected_limits(
        bound: BoundMaterialTreePose<'_>,
        limits: &ProjectedProvenTransversalScanLimits,
    ) -> Result<(), ProvenTransversalScanError> {
        preflight_proven_transversal_shape(
            bound,
            limits.exact.max_faces,
            limits.blocking.max_hinges,
            limits.blocking.max_unordered_face_pairs,
            limits.max_boundary_vertices_per_face,
            limits.exact.max_boundary_occurrences,
            limits.blocking.max_triangular_faces,
            limits.blocking.max_triangular_face_pairs,
            limits.shared.max_registry_authentication_work,
            limits.shared.max_total_boundary_relation_work,
        )
    }

    #[test]
    fn structural_reservation_bounds_every_three_stage_counter_and_one_short_is_preflight_atomic() {
        let cases = [
            (
                corner_mountain_valley_400mm_fixture(0.0),
                vec![135.0, 135.0],
            ),
            (midpoint_mountain_400mm_fixture(), vec![179.0, 179.0]),
            (triangle_and_quadrilateral_fixture(), vec![37.0]),
        ];
        for (fixture, angles) in cases {
            let root = fixture.model.face_ids()[0];
            let pose = solve_fixture(&fixture, root, &angles);
            let bound = fixture
                .model
                .bind_pose(&pose)
                .expect("issuer-bound structural fixture");
            let hard = ProvenTransversalScanLimits::default();
            let projected = project_proven_transversal_scan_limits(hard).expect("hard projection");
            let exact = prepare_rational_cayley_tree_pose_v1(bound, projected.exact)
                .expect("structural exact prefix");
            let measured_limits =
                measured_limits_after_exact(projected.measured, &projected.shared, &exact)
                    .expect("structural measured remaining");
            let measured = measure_binary64_affine_envelope(&exact, bound, measured_limits)
                .expect("structural measured prefix");
            let scan = scan_authenticated_triangle_pairs_blocking_only(
                &exact,
                &measured,
                bound,
                0.0,
                projected.blocking,
            )
            .expect("structural blocking scan");

            let registry_reservation = projected_registry_reservation(
                exact.work.faces,
                exact.work.hinges,
                exact.work.boundary_occurrences,
            )
            .expect("registry reservation");
            let relation_reservation = projected_boundary_relation_reservation(
                scan.work.unordered_face_pairs,
                scan.work.expected_triangular_face_pairs,
            )
            .expect("boundary relation reservation");
            assert!(
                observed_three_stage_registry_work(&exact, &measured, &scan)
                    <= registry_reservation
            );
            assert!(observed_three_stage_boundary_relation_work(&scan) <= relation_reservation);

            let exact_caps = ProvenTransversalScanLimits {
                max_registry_authentication_work: registry_reservation,
                max_total_boundary_relation_work: relation_reservation,
                ..hard
            };
            let exact_projection =
                project_proven_transversal_scan_limits(exact_caps).expect("exact structural caps");
            assert_eq!(
                preflight_with_projected_limits(bound, &exact_projection),
                Ok(())
            );
            assert!(scan_bound_pose_for_proven_transversal_penetration(bound, exact_caps).is_ok());

            let registry_one_short = ProvenTransversalScanLimits {
                max_registry_authentication_work: registry_reservation - 1,
                ..exact_caps
            };
            let registry_one_short_projection =
                project_proven_transversal_scan_limits(registry_one_short)
                    .expect("registry one-short projection");
            assert_eq!(
                preflight_with_projected_limits(bound, &registry_one_short_projection),
                Err(ProvenTransversalScanError::ResourceLimitExceeded)
            );
            assert_eq!(
                scan_bound_pose_for_proven_transversal_penetration(bound, registry_one_short),
                Err(ProvenTransversalScanError::ResourceLimitExceeded)
            );

            let relation_one_short = ProvenTransversalScanLimits {
                max_total_boundary_relation_work: relation_reservation - 1,
                ..exact_caps
            };
            let relation_one_short_projection =
                project_proven_transversal_scan_limits(relation_one_short)
                    .expect("relation one-short projection");
            assert_eq!(
                preflight_with_projected_limits(bound, &relation_one_short_projection),
                Err(ProvenTransversalScanError::ResourceLimitExceeded)
            );
            assert_eq!(
                scan_bound_pose_for_proven_transversal_penetration(bound, relation_one_short),
                Err(ProvenTransversalScanError::ResourceLimitExceeded)
            );
        }
    }

    #[test]
    fn facade_structural_aggregate_caps_fail_before_returning_a_partial_summary() {
        let fixture = midpoint_mountain_400mm_fixture();
        let root = fixture.model.face_ids()[1];
        let pose = solve_fixture(&fixture, root, &[135.0, 135.0]);
        let bound = fixture
            .model
            .bind_pose(&pose)
            .expect("issuer-bound structural-budget pose");

        for limited in [
            ProvenTransversalScanLimits {
                max_registry_authentication_work: 0,
                ..ProvenTransversalScanLimits::default()
            },
            ProvenTransversalScanLimits {
                max_total_boundary_relation_work: 0,
                ..ProvenTransversalScanLimits::default()
            },
            ProvenTransversalScanLimits {
                max_unordered_face_pairs: 2,
                ..ProvenTransversalScanLimits::default()
            },
            ProvenTransversalScanLimits {
                max_total_boundary_vertices: 8,
                ..ProvenTransversalScanLimits::default()
            },
        ] {
            assert_eq!(
                scan_bound_pose_for_proven_transversal_penetration(bound, limited),
                Err(ProvenTransversalScanError::ResourceLimitExceeded)
            );
        }
    }

    #[test]
    fn authenticated_bridge_records_mixed_polygon_pairs_as_unsupported() {
        let fixture = triangle_and_quadrilateral_fixture();
        let roots = fixture.model.face_ids();
        assert_eq!(roots.len(), 2);
        for root in roots {
            let pose = solve_fixture(&fixture, *root, &[37.0]);
            let bound = fixture
                .model
                .bind_pose(&pose)
                .expect("issuer-bound mixed-face pose");
            let exact = exact_fixture_pose(
                &fixture,
                &pose,
                super::super::super::ExactTreePoseLimits::default(),
            );
            let measured =
                measure_binary64_affine_envelope(&exact, bound, MeasuredEnvelopeLimits::default())
                    .expect("mixed-face measured envelope");
            let scan = scan_authenticated_triangle_pairs_blocking_only(
                &exact,
                &measured,
                bound,
                0.0,
                AuthenticatedTriangleBlockingLimits::default(),
            )
            .expect("mixed-face scan");

            assert_eq!(scan.pairs.len(), 1);
            assert_eq!(scan.work.faces, 2);
            assert_eq!(scan.work.triangular_faces, 1);
            assert_eq!(scan.work.unordered_face_pairs, 1);
            assert_eq!(scan.work.expected_triangular_face_pairs, 0);
            assert_eq!(scan.work.analyzed_triangular_face_pairs, 0);
            assert_eq!(scan.work.exact_predicate_calls, 0);
            assert_eq!(scan.work.observed_predicate_calls, 0);
            assert_eq!(scan.work.unsupported_pair_records, 1);
            assert_eq!(scan.work.result_records, 1);
            assert_eq!(scan.proven_penetrating_pairs(), 0);
            let record = scan.pairs[0];
            assert_eq!(record.decision, BlockingOnlyDecision::Unresolved);
            assert_eq!(
                scan.decision(record.first, record.second),
                Some(BlockingOnlyDecision::Unresolved)
            );
            assert_eq!(
                scan.decision(record.second, record.first),
                Some(BlockingOnlyDecision::Unresolved)
            );
        }
    }

    #[test]
    fn authenticated_bridge_requires_positive_zero_and_the_exact_measured_object() {
        let fixture = midpoint_mountain_400mm_fixture();
        let root = fixture.model.face_ids()[1];
        let pose = solve_fixture(&fixture, root, &[135.0, 135.0]);
        let bound = fixture
            .model
            .bind_pose(&pose)
            .expect("issuer-bound bridge pose");
        let exact = exact_fixture_pose(
            &fixture,
            &pose,
            super::super::super::ExactTreePoseLimits::default(),
        );
        let independently_regenerated_exact = exact_fixture_pose(
            &fixture,
            &pose,
            super::super::super::ExactTreePoseLimits::default(),
        );
        let measured =
            measure_binary64_affine_envelope(&exact, bound, MeasuredEnvelopeLimits::default())
                .expect("measured exact-pose envelope");

        for unsupported in [
            -0.0,
            0.1,
            1.0,
            3.0,
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
        ] {
            assert!(matches!(
                scan_authenticated_triangle_pairs_blocking_only(
                    &exact,
                    &measured,
                    bound,
                    unsupported,
                    AuthenticatedTriangleBlockingLimits::default(),
                ),
                Err(AuthenticatedTriangleBlockingError::UnsupportedPaperThickness)
            ));
        }
        assert!(matches!(
            scan_authenticated_triangle_pairs_blocking_only(
                &independently_regenerated_exact,
                &measured,
                bound,
                0.0,
                AuthenticatedTriangleBlockingLimits::default(),
            ),
            Err(AuthenticatedTriangleBlockingError::AuthorityMismatch)
        ));
        let scan = scan_authenticated_triangle_pairs_blocking_only(
            &exact,
            &measured,
            bound,
            0.0,
            AuthenticatedTriangleBlockingLimits::default(),
        )
        .expect("identity-matched bridge");
        assert!(scan.is_for(&exact, &measured, bound, 0.0));
        assert!(!scan.is_for(&independently_regenerated_exact, &measured, bound, 0.0));
        assert_eq!(scan.proven_penetrating_pairs(), 1);

        let assert_foreign_bound_rejected = |foreign_bound| {
            assert!(matches!(
                scan_authenticated_triangle_pairs_blocking_only(
                    &exact,
                    &measured,
                    foreign_bound,
                    0.0,
                    AuthenticatedTriangleBlockingLimits::default(),
                ),
                Err(AuthenticatedTriangleBlockingError::AuthorityMismatch)
            ));
        };
        let same_values_pose = solve_fixture(&fixture, root, &[135.0, 135.0]);
        assert_foreign_bound_rejected(
            fixture
                .model
                .bind_pose(&same_values_pose)
                .expect("ABA pose bound"),
        );
        let rerooted_pose = solve_fixture(&fixture, fixture.model.face_ids()[0], &[135.0, 135.0]);
        assert_foreign_bound_rejected(
            fixture
                .model
                .bind_pose(&rerooted_pose)
                .expect("rerooted pose bound"),
        );
        let next_angle = f64::from_bits(135.0_f64.to_bits() + 1);
        let next_pose = solve_fixture(&fixture, root, &[next_angle, 135.0]);
        assert_foreign_bound_rejected(
            fixture
                .model
                .bind_pose(&next_pose)
                .expect("one-ULP pose bound"),
        );
        let foreign_fixture = midpoint_mountain_400mm_fixture();
        let foreign_pose = solve_fixture(
            &foreign_fixture,
            foreign_fixture.model.face_ids()[1],
            &[135.0, 135.0],
        );
        assert_foreign_bound_rejected(
            foreign_fixture
                .model
                .bind_pose(&foreign_pose)
                .expect("foreign issuer pose bound"),
        );

        let mut underreported =
            measure_binary64_affine_envelope(&exact, bound, MeasuredEnvelopeLimits::default())
                .expect("second measured envelope");
        let mut changed = false;
        'faces: for face in &mut underreported.faces {
            for radius in &mut face.radius {
                if radius.is_positive() {
                    *radius = BigRational::zero();
                    changed = true;
                    break 'faces;
                }
            }
        }
        assert!(changed, "fixture must exercise a nonzero measured radius");
        assert!(
            scan_authenticated_triangle_pairs_blocking_only(
                &exact,
                &underreported,
                bound,
                0.0,
                AuthenticatedTriangleBlockingLimits::default(),
            )
            .is_err(),
            "the direct binary64 lift must remain inside every authenticated radius"
        );
    }

    #[test]
    fn authenticated_bridge_accepts_exact_limits_and_every_one_short_is_atomic() {
        let fixture = midpoint_mountain_400mm_fixture();
        let root = fixture.model.face_ids()[1];
        let pose = solve_fixture(&fixture, root, &[135.0, 135.0]);
        let bound = fixture
            .model
            .bind_pose(&pose)
            .expect("issuer-bound bridge pose");
        let exact = exact_fixture_pose(
            &fixture,
            &pose,
            super::super::super::ExactTreePoseLimits::default(),
        );
        let measured =
            measure_binary64_affine_envelope(&exact, bound, MeasuredEnvelopeLimits::default())
                .expect("measured exact-pose envelope");
        let baseline = scan_authenticated_triangle_pairs_blocking_only(
            &exact,
            &measured,
            bound,
            0.0,
            AuthenticatedTriangleBlockingLimits::default(),
        )
        .expect("baseline bridge");
        assert_eq!(
            baseline.pairs.first().map(|record| record.decision),
            Some(BlockingOnlyDecision::ProvenPenetrating),
            "the first canonical witness makes a late one-short an atomicity canary"
        );
        let work = baseline.work.clone();
        let exact_limits = AuthenticatedTriangleBlockingLimits {
            max_faces: work.faces,
            max_hinges: work.hinges,
            max_boundary_occurrences: work.boundary_occurrences,
            max_triangular_faces: work.triangular_faces,
            max_unordered_face_pairs: work.unordered_face_pairs,
            max_triangular_face_pairs: work.expected_triangular_face_pairs,
            max_topology_vertex_checks: work.topology_vertex_checks,
            max_hinge_boundary_index_entries: work.hinge_boundary_index_entries,
            max_hinge_face_lookups: work.hinge_face_lookups,
            max_hinge_vertex_lookups: work.hinge_vertex_lookups,
            max_predicate_calls: work
                .exact_predicate_calls
                .checked_add(work.observed_predicate_calls)
                .expect("predicate calls"),
            max_result_records: work.result_records,
            max_total_machin_terms: work.exact.machin_terms,
            max_total_trig_terms: work.exact.trig_terms,
            max_total_sqrt_refinements: work.exact.sqrt_refinements,
            exact: CayleyResumeLimits {
                max_machin_terms_per_series: work.exact.max_machin_series_terms,
                max_trig_terms_per_series: work.exact.max_trig_series_terms,
                max_sqrt_refinements: work.exact.max_sqrt_call_refinements,
                max_interval_operations: work.exact.interval_operations,
                max_shift_bits: work.exact.max_shift_bits,
                max_intermediate_bits: work.exact.max_preflight_bits,
                max_gcd_fallback_calls: work.exact.gcd_fallback_calls,
                max_gcd_fallback_input_bits: work.exact.gcd_fallback_input_bits,
                max_rational_allocations: work.exact.rational_allocations,
                max_rational_allocation_bits: work.exact.max_rational_allocation_bits,
                max_total_rational_allocation_bits: work.exact.total_rational_allocation_bits,
                max_output_bits: work.exact.max_output_bits,
            },
        };
        let run = |limits| {
            scan_authenticated_triangle_pairs_blocking_only(&exact, &measured, bound, 0.0, limits)
        };
        let exact_scan = run(exact_limits).expect("all exact observed limits");
        assert_eq!(exact_scan.work, work);
        assert_eq!(exact_scan.proven_penetrating_pairs(), 1);

        macro_rules! structural_one_short {
            ($field:ident, $resource:literal) => {{
                let mut one_short = exact_limits;
                assert!(one_short.$field > 0, "{} is exercised", stringify!($field));
                one_short.$field -= 1;
                assert_eq!(
                    run(one_short).expect_err(concat!(
                        stringify!($field),
                        " one-short must return no scan"
                    )),
                    AuthenticatedTriangleBlockingError::ResourceLimitExceeded {
                        resource: $resource
                    },
                    "{} one-short must fail at its own resource",
                    stringify!($field),
                );
            }};
        }
        structural_one_short!(max_faces, "faces");
        structural_one_short!(max_hinges, "hinges");
        structural_one_short!(max_boundary_occurrences, "boundary_occurrences");
        structural_one_short!(max_triangular_faces, "triangular_faces");
        structural_one_short!(max_unordered_face_pairs, "unordered_face_pairs");
        structural_one_short!(max_triangular_face_pairs, "triangular_face_pairs");
        structural_one_short!(max_topology_vertex_checks, "topology_vertex_checks");
        structural_one_short!(
            max_hinge_boundary_index_entries,
            "hinge_boundary_index_entries"
        );
        structural_one_short!(max_hinge_face_lookups, "hinge_face_lookups");
        structural_one_short!(max_hinge_vertex_lookups, "hinge_vertex_lookups");
        structural_one_short!(max_predicate_calls, "predicate_calls");
        structural_one_short!(max_result_records, "result_records");

        macro_rules! exact_one_short {
            ($field:ident, $resource:literal) => {{
                let mut one_short = exact_limits;
                assert!(
                    one_short.exact.$field > 0,
                    "{} is exercised",
                    stringify!($field)
                );
                one_short.exact.$field -= 1;
                match run(one_short).expect_err(concat!(
                    "exact ",
                    stringify!($field),
                    " one-short must return no scan"
                )) {
                    AuthenticatedTriangleBlockingError::Exact(
                        CayleyError::ResourceLimitExceeded { resource, .. },
                    ) => assert_eq!(
                        resource,
                        $resource,
                        "exact {} one-short must fail at the expected resource",
                        stringify!($field),
                    ),
                    other => panic!(
                        "exact {} one-short returned the wrong error: {other:?}",
                        stringify!($field)
                    ),
                }
            }};
        }
        exact_one_short!(max_machin_terms_per_series, "machin_terms");
        exact_one_short!(max_trig_terms_per_series, "trig_terms");
        exact_one_short!(max_sqrt_refinements, "sqrt_refinements");
        exact_one_short!(max_interval_operations, "interval_operations");
        exact_one_short!(max_shift_bits, "shift_bits");
        // A tighter intermediate preflight can select the metered GCD
        // fallback. Every other exact limit is already fixed to its observed
        // minimum, so that alternate path reaches its input-bit boundary
        // first. The important contract is still atomic rejection.
        exact_one_short!(max_intermediate_bits, "gcd_fallback_input_bits");
        exact_one_short!(max_gcd_fallback_calls, "gcd_fallback_calls");
        exact_one_short!(max_gcd_fallback_input_bits, "gcd_fallback_input_bits");
        exact_one_short!(max_rational_allocations, "rational_allocations");
        exact_one_short!(max_rational_allocation_bits, "rational_allocation_bits");
        exact_one_short!(
            max_total_rational_allocation_bits,
            "total_rational_allocation_bits"
        );
        exact_one_short!(max_output_bits, "output_bits");

        macro_rules! total_term_one_short {
            ($field:ident, $resource:literal) => {{
                let mut one_short = exact_limits;
                assert!(one_short.$field > 0, "{} is exercised", stringify!($field));
                one_short.$field -= 1;
                match run(one_short).expect_err(concat!(
                    stringify!($field),
                    " one-short must return no scan"
                )) {
                    AuthenticatedTriangleBlockingError::Exact(
                        CayleyError::ResourceLimitExceeded { resource, .. },
                    ) => assert_eq!(
                        resource,
                        $resource,
                        "{} one-short must fail at its own resource",
                        stringify!($field),
                    ),
                    other => panic!(
                        "{} one-short returned the wrong error: {other:?}",
                        stringify!($field)
                    ),
                }
            }};
        }
        total_term_one_short!(max_total_machin_terms, "total_machin_terms");
        total_term_one_short!(max_total_trig_terms, "total_trig_terms");
        total_term_one_short!(max_total_sqrt_refinements, "total_sqrt_refinements");

        macro_rules! structural_over_hard_ceiling {
            ($field:ident) => {{
                let mut over_hard_ceiling = AUTHENTICATED_TRIANGLE_BLOCKING_HARD_LIMITS;
                over_hard_ceiling.$field = over_hard_ceiling
                    .$field
                    .checked_add(1)
                    .expect("hard ceiling has room for the rejection probe");
                assert!(
                    matches!(
                        run(over_hard_ceiling),
                        Err(AuthenticatedTriangleBlockingError::ResourceLimitExceeded {
                            resource: "configured_hard_ceiling"
                        })
                    ),
                    "{} above its hard ceiling must return no scan",
                    stringify!($field)
                );
            }};
        }
        structural_over_hard_ceiling!(max_faces);
        structural_over_hard_ceiling!(max_hinges);
        structural_over_hard_ceiling!(max_boundary_occurrences);
        structural_over_hard_ceiling!(max_triangular_faces);
        structural_over_hard_ceiling!(max_unordered_face_pairs);
        structural_over_hard_ceiling!(max_triangular_face_pairs);
        structural_over_hard_ceiling!(max_topology_vertex_checks);
        structural_over_hard_ceiling!(max_hinge_boundary_index_entries);
        structural_over_hard_ceiling!(max_hinge_face_lookups);
        structural_over_hard_ceiling!(max_hinge_vertex_lookups);
        structural_over_hard_ceiling!(max_predicate_calls);
        structural_over_hard_ceiling!(max_result_records);
        structural_over_hard_ceiling!(max_total_machin_terms);
        structural_over_hard_ceiling!(max_total_trig_terms);
        structural_over_hard_ceiling!(max_total_sqrt_refinements);

        macro_rules! exact_over_hard_ceiling {
            ($field:ident) => {{
                let mut over_hard_ceiling = AUTHENTICATED_TRIANGLE_BLOCKING_HARD_LIMITS;
                over_hard_ceiling.exact.$field = over_hard_ceiling
                    .exact
                    .$field
                    .checked_add(1)
                    .expect("hard ceiling has room for the rejection probe");
                assert!(
                    matches!(
                        run(over_hard_ceiling),
                        Err(AuthenticatedTriangleBlockingError::ResourceLimitExceeded {
                            resource: "configured_hard_ceiling"
                        })
                    ),
                    "exact {} above its hard ceiling must return no scan",
                    stringify!($field)
                );
            }};
        }
        exact_over_hard_ceiling!(max_machin_terms_per_series);
        exact_over_hard_ceiling!(max_trig_terms_per_series);
        exact_over_hard_ceiling!(max_sqrt_refinements);
        exact_over_hard_ceiling!(max_interval_operations);
        exact_over_hard_ceiling!(max_shift_bits);
        exact_over_hard_ceiling!(max_intermediate_bits);
        exact_over_hard_ceiling!(max_gcd_fallback_calls);
        exact_over_hard_ceiling!(max_gcd_fallback_input_bits);
        exact_over_hard_ceiling!(max_rational_allocations);
        exact_over_hard_ceiling!(max_rational_allocation_bits);
        exact_over_hard_ceiling!(max_total_rational_allocation_bits);
        exact_over_hard_ceiling!(max_output_bits);
    }
}
