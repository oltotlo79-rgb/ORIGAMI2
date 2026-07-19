use std::sync::Arc;

use ori_domain::FaceId;
use ori_kinematics::{
    MATERIAL_TREE_KINEMATICS_MODEL_ID, MaterialTreeKinematicsModel, MaterialTreePose,
};
use thiserror::Error;

use crate::{
    TOPOLOGY_CONTACT_POLICY_V2, TopologyContactDecision,
    cayley::{
        ProvenTransversalScanError, ProvenTransversalScanLimits, ProvenTransversalScanSummary,
        scan_bound_pose_for_proven_transversal_penetration,
    },
    zero_thickness::{
        AuthenticatedZeroThicknessPose, ZeroThicknessAnalysisError, ZeroThicknessAnalysisWork,
        ZeroThicknessGeometryLimits, prepare_authenticated_zero_thickness_pose,
    },
};

/// Initial paper-thickness interpretation used by native collision geometry.
pub const CENTERED_MID_SURFACE_THICKNESS_MODEL_V1: &str = "centered_mid_surface_v1";

/// First opaque native static-collision geometry-proof format.
///
/// Version 1 admits only the complete zero-pair proof for a no-hinge,
/// single-material-face pose. Exact zero-thickness multi-face diagnostics now
/// authenticate and scan every face and triangle pair, but every valid
/// multi-face material tree contains a shared hinge. That pair remains
/// blocking until canonical watertight shared-feature geometry and its finite
/// hinge model exist. Positive-thickness pairs are also still blocking. The
/// public success set therefore has not expanded and the proof identifier
/// remains V1.
///
/// This proof does not claim that the pose is current for a project. A
/// stronger authority boundary must bind the exact proof object to the exact
/// current-pose certificate and generation.
pub const NATIVE_STATIC_COLLISION_GEOMETRY_PROOF_V1: &str =
    "native_static_collision_geometry_proof_v1";

/// Hard bounds applied before a native static analysis may allocate or scan.
///
/// Boundary and triangle counts constrain storage. The triangulation and
/// boundary-relation work fields separately constrain the exact synchronous
/// predicates whose cost is superlinear in those counts. Rational allocation
/// fields bound the count, largest payload and aggregate payload bits of the
/// exact kernel's logical BigInt/BigRational allocations. Every one-short or
/// arithmetic-overflow case fails closed as `ResourceLimitExceeded`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StaticCollisionLimits {
    pub max_faces: usize,
    pub max_unordered_face_pairs: usize,
    pub max_boundary_vertices_per_face: usize,
    pub max_total_boundary_vertices: usize,
    pub max_triangles_per_face: usize,
    pub max_total_triangles: usize,
    pub max_triangulation_work_per_face: usize,
    pub max_total_triangulation_work: usize,
    pub max_registry_authentication_work: usize,
    pub max_triangle_pairs_per_face_pair: usize,
    pub max_total_triangle_pairs: usize,
    pub max_boundary_relation_work_per_face_pair: usize,
    pub max_total_boundary_relation_work: usize,
    pub max_rational_input_bits: usize,
    pub max_total_rational_input_storage_bits: usize,
    pub max_total_rational_retained_clone_bits: usize,
    pub max_rational_operations: usize,
    pub max_rational_intermediate_bits: usize,
    pub max_rational_gcd_fallback_calls: usize,
    pub max_rational_gcd_fallback_input_bits: usize,
    pub max_rational_allocations: usize,
    pub max_rational_allocation_bits: usize,
    pub max_total_rational_allocation_bits: usize,
    pub max_rational_output_bits: usize,
    pub max_total_rational_output_bits: usize,
}

impl Default for StaticCollisionLimits {
    fn default() -> Self {
        Self {
            max_faces: 10_001,
            max_unordered_face_pairs: 50_000_000,
            max_boundary_vertices_per_face: 4_096,
            max_total_boundary_vertices: 50_000,
            max_triangles_per_face: 4_094,
            max_total_triangles: 50_000,
            max_triangulation_work_per_face: 100_000_000,
            max_total_triangulation_work: 500_000_000,
            max_registry_authentication_work: 10_000_000,
            max_triangle_pairs_per_face_pair: 250_000,
            max_total_triangle_pairs: 1_000_000,
            max_boundary_relation_work_per_face_pair: 10_000_000,
            max_total_boundary_relation_work: 40_000_000,
            max_rational_input_bits: 4_096,
            max_total_rational_input_storage_bits: 536_870_912,
            max_total_rational_retained_clone_bits: 4_294_967_296,
            max_rational_operations: 1_000_000_000,
            max_rational_intermediate_bits: 32_768,
            max_rational_gcd_fallback_calls: 1_000_000,
            max_rational_gcd_fallback_input_bits: 8_589_934_592,
            max_rational_allocations: 1_000_000,
            max_rational_allocation_bits: 65_536,
            max_total_rational_allocation_bits: 1_073_741_824,
            max_rational_output_bits: 32_768,
            max_total_rational_output_bits: 1_073_741_824,
        }
    }
}

/// A fail-closed native static-collision analysis failure.
///
/// Every error is blocking. In particular,
/// [`StaticCollisionError::PairEvidenceUnavailable`] must never be interpreted
/// as collision-free or as a geometry proof.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum StaticCollisionError {
    #[error("the material pose was issued by a different kinematics model instance")]
    PoseIssuerMismatch,
    #[error("paper thickness must be finite and non-negative")]
    InvalidPaperThickness,
    #[error("static collision work exceeds the configured resource limit")]
    ResourceLimitExceeded,
    #[error("the material pose registry is internally inconsistent")]
    InconsistentMaterialPose,
    #[error(
        "static collision did not establish a nonblocking result for all {expected_unordered_face_pairs} unordered face pairs"
    )]
    PairEvidenceUnavailable {
        expected_unordered_face_pairs: usize,
    },
    #[error(
        "static collision proved {proven_transversal_pairs} transversal penetrating face pairs among {expected_unordered_face_pairs} unordered face pairs"
    )]
    ProvenTransversalPenetration {
        expected_unordered_face_pairs: usize,
        proven_transversal_pairs: usize,
        /// First proven pair in canonical `FaceId` byte order.
        ///
        /// This is geometry identity only. It contains no coordinates,
        /// transforms, arithmetic evidence, or internal diagnostic text.
        first_proven_transversal_pair: [FaceId; 2],
    },
}

#[derive(Debug)]
struct StaticCollisionProof {
    model: MaterialTreeKinematicsModel,
    pose: MaterialTreePose,
    paper_thickness_bits: u64,
    proof_id: &'static str,
    policy_id: &'static str,
    kinematics_model_id: &'static str,
    thickness_model_id: &'static str,
    face_count: usize,
    expected_unordered_face_pairs: usize,
    analyzed_unordered_face_pairs: usize,
    expected_triangle_pairs: usize,
    analyzed_triangle_pairs: usize,
}

/// Opaque geometry proof that one exact native material pose completed static
/// collision analysis without penetration or unresolved indeterminate pairs.
///
/// Clones preserve proof identity. Solving an equal angle vector again creates
/// a different pose and proof identity, so callers can reject same-angle
/// geometry re-solve ABA by checking [`Self::is_for_geometry`] and
/// [`Self::same_proof`].
///
/// This type deliberately carries no project, revision, current-pose
/// certificate, or pose generation. It cannot authorize a project mutation
/// and must not be treated as a current collision certificate.
#[derive(Debug, Clone)]
pub struct NativeStaticCollisionGeometryProof {
    proof: Arc<StaticCollisionProof>,
}

impl NativeStaticCollisionGeometryProof {
    /// Returns whether this proof is bound to the exact model issuer, exact
    /// pose instance, and bit-exact paper thickness supplied by the caller.
    #[must_use]
    pub fn is_for_geometry(
        &self,
        model: &MaterialTreeKinematicsModel,
        pose: &MaterialTreePose,
        paper_thickness_mm: f64,
    ) -> bool {
        self.proof.model == *model
            && self.proof.pose.same_instance(pose)
            && self.proof.paper_thickness_bits == paper_thickness_mm.to_bits()
    }

    /// Returns whether two handles refer to the same issued proof object.
    #[must_use]
    pub fn same_proof(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.proof, &other.proof)
    }

    #[must_use]
    pub fn proof_id(&self) -> &'static str {
        self.proof.proof_id
    }

    #[must_use]
    pub fn policy_id(&self) -> &'static str {
        self.proof.policy_id
    }

    #[must_use]
    pub fn kinematics_model_id(&self) -> &'static str {
        self.proof.kinematics_model_id
    }

    #[must_use]
    pub fn thickness_model_id(&self) -> &'static str {
        self.proof.thickness_model_id
    }

    #[must_use]
    pub fn paper_thickness_mm(&self) -> f64 {
        f64::from_bits(self.proof.paper_thickness_bits)
    }

    #[must_use]
    pub fn paper_thickness_bits(&self) -> u64 {
        self.proof.paper_thickness_bits
    }

    #[must_use]
    pub fn face_count(&self) -> usize {
        self.proof.face_count
    }

    #[must_use]
    pub fn expected_unordered_face_pairs(&self) -> usize {
        self.proof.expected_unordered_face_pairs
    }

    #[must_use]
    pub fn analyzed_unordered_face_pairs(&self) -> usize {
        self.proof.analyzed_unordered_face_pairs
    }

    /// Number of authenticated triangle pairs required by the complete
    /// face-pair analysis. The current public multi-face proof remains
    /// blocking at every tree hinge until the finite hinge model exists.
    #[must_use]
    pub fn expected_triangle_pairs(&self) -> usize {
        self.proof.expected_triangle_pairs
    }

    /// Number of authenticated triangle pairs actually classified.
    #[must_use]
    pub fn analyzed_triangle_pairs(&self) -> usize {
        self.proof.analyzed_triangle_pairs
    }
}

/// Proves static collision geometry for one exact native material pose.
///
/// The current implementation intentionally succeeds only for the complete
/// zero-pair case: exactly one material face and no material hinge. A
/// zero-thickness multi-face pose runs authenticated whole-face diagnostics,
/// but returns a blocking error at penetration, indeterminate evidence or the
/// mandatory shared-hinge pair. Positive-thickness pairs remain blocking.
///
/// Static `Touching` is admissible only as evidence of zero penetration at
/// this exact pose. Continuous fold execution must still stop at its first
/// touching time; this proof does not authorize motion through contact.
pub fn prove_static_collision_geometry(
    model: &MaterialTreeKinematicsModel,
    pose: &MaterialTreePose,
    paper_thickness_mm: f64,
    limits: StaticCollisionLimits,
) -> Result<NativeStaticCollisionGeometryProof, StaticCollisionError> {
    let bound = model
        .bind_pose(pose)
        .map_err(|_| StaticCollisionError::PoseIssuerMismatch)?;
    if !paper_thickness_mm.is_finite() || paper_thickness_mm < 0.0 {
        return Err(StaticCollisionError::InvalidPaperThickness);
    }

    let face_count = pose.face_ids().len();
    if face_count == 0
        || pose.hinges().len() != face_count.saturating_sub(1)
        || pose.hinge_angles().len() != pose.hinges().len()
        || (pose.hinges().is_empty() && pose.fixed_face().is_some())
        || (!pose.hinges().is_empty() && pose.fixed_face().is_none())
        || !pose
            .hinges()
            .iter()
            .zip(pose.hinge_angles())
            .all(|(hinge, angle)| hinge.edge() == angle.edge())
    {
        return Err(StaticCollisionError::InconsistentMaterialPose);
    }
    if face_count > limits.max_faces {
        return Err(StaticCollisionError::ResourceLimitExceeded);
    }
    let expected_unordered_face_pairs = checked_unordered_pair_count(face_count)?;
    if expected_unordered_face_pairs > limits.max_unordered_face_pairs {
        return Err(StaticCollisionError::ResourceLimitExceeded);
    }

    for (index, face) in pose.face_ids().iter().copied().enumerate() {
        if index > 0 && pose.face_ids()[index - 1].canonical_bytes() >= face.canonical_bytes() {
            return Err(StaticCollisionError::InconsistentMaterialPose);
        }
        if pose.face_transform(face).is_none() {
            return Err(StaticCollisionError::InconsistentMaterialPose);
        }
    }

    if paper_thickness_mm > 0.0 && expected_unordered_face_pairs > 0 {
        return Err(StaticCollisionError::PairEvidenceUnavailable {
            expected_unordered_face_pairs,
        });
    }

    if expected_unordered_face_pairs == 0 {
        // `bind_pose` above proves this came from a private PreparedTree.
        // Material-tree preparation exact-validates every paper and face
        // boundary before it can issue either the model or this pose, so the
        // allocation-free zero-pair proof does not bypass source validity.
        return Ok(NativeStaticCollisionGeometryProof {
            proof: Arc::new(StaticCollisionProof {
                model: model.clone(),
                pose: pose.clone(),
                paper_thickness_bits: paper_thickness_mm.to_bits(),
                proof_id: NATIVE_STATIC_COLLISION_GEOMETRY_PROOF_V1,
                policy_id: TOPOLOGY_CONTACT_POLICY_V2,
                kinematics_model_id: MATERIAL_TREE_KINEMATICS_MODEL_ID,
                thickness_model_id: CENTERED_MID_SURFACE_THICKNESS_MODEL_V1,
                face_count,
                expected_unordered_face_pairs,
                analyzed_unordered_face_pairs: 0,
                expected_triangle_pairs: 0,
                analyzed_triangle_pairs: 0,
            }),
        });
    }
    let analysis = prepare_authenticated_zero_thickness_pose(
        pose,
        ZeroThicknessGeometryLimits {
            max_boundary_vertices_per_face: limits.max_boundary_vertices_per_face,
            max_total_boundary_vertices: limits.max_total_boundary_vertices,
            max_triangles_per_face: limits.max_triangles_per_face,
            max_total_triangles: limits.max_total_triangles,
            max_triangulation_work_per_face: limits.max_triangulation_work_per_face,
            max_total_triangulation_work: limits.max_total_triangulation_work,
            max_registry_authentication_work: limits.max_registry_authentication_work,
            max_triangle_pairs_per_face_pair: limits.max_triangle_pairs_per_face_pair,
            max_total_triangle_pairs: limits.max_total_triangle_pairs,
            max_boundary_relation_work_per_face_pair: limits
                .max_boundary_relation_work_per_face_pair,
            max_total_boundary_relation_work: limits.max_total_boundary_relation_work,
            max_rational_input_bits: limits.max_rational_input_bits,
            max_total_rational_input_storage_bits: limits.max_total_rational_input_storage_bits,
            max_total_rational_retained_clone_bits: limits.max_total_rational_retained_clone_bits,
            max_rational_operations: limits.max_rational_operations,
            max_rational_intermediate_bits: limits.max_rational_intermediate_bits,
            max_rational_gcd_fallback_calls: limits.max_rational_gcd_fallback_calls,
            max_rational_gcd_fallback_input_bits: limits.max_rational_gcd_fallback_input_bits,
            max_rational_allocations: limits.max_rational_allocations,
            max_rational_allocation_bits: limits.max_rational_allocation_bits,
            max_total_rational_allocation_bits: limits.max_total_rational_allocation_bits,
            max_rational_output_bits: limits.max_rational_output_bits,
            max_total_rational_output_bits: limits.max_total_rational_output_bits,
        },
    )
    .map_err(|error| map_zero_thickness_error(error, expected_unordered_face_pairs))?;
    let scan = scan_authenticated_zero_thickness_pairs(
        &analysis,
        face_count,
        expected_unordered_face_pairs,
    )?;
    if scan.enumerated_unordered_face_pairs != expected_unordered_face_pairs {
        return Err(StaticCollisionError::InconsistentMaterialPose);
    }
    if scan.expected_triangle_pairs != analysis.total_triangle_pairs()
        || scan.analyzed_triangle_pairs != analysis.total_triangle_pairs()
    {
        return Err(StaticCollisionError::InconsistentMaterialPose);
    }

    // A blocking decision never short-circuits the canonical pair scan.
    // Resource or evidence failures still fail immediately and atomically.
    // Multi-face diagnostics cannot issue the public geometry proof yet:
    // every material tree contains at least one shared hinge and the finite
    // hinge model remains mandatory. Keeping the only proof constructor in
    // the zero-pair branch above makes that boundary structural instead of
    // depending on today's decision mix.
    if scan.blocking_unordered_face_pairs > scan.enumerated_unordered_face_pairs {
        return Err(StaticCollisionError::InconsistentMaterialPose);
    }
    let transversal_limits = (paper_thickness_mm.to_bits() == 0.0_f64.to_bits())
        .then(|| remaining_proven_transversal_scan_limits(limits, analysis.work()))
        .transpose()?;
    // The legacy diagnostic owns its complete exact face geometry. Release it
    // after all count checks and before the Cayley bridge builds a second
    // authenticated exact representation, so the two full snapshots never
    // contribute to peak retained memory at the same time.
    drop(analysis);
    if let Some(transversal_limits) = transversal_limits {
        let transversal =
            scan_bound_pose_for_proven_transversal_penetration(bound, transversal_limits).map_err(
                |error| map_proven_transversal_scan_error(error, expected_unordered_face_pairs),
            )?;
        return finish_proven_transversal_scan(transversal, expected_unordered_face_pairs);
    }
    Err(StaticCollisionError::PairEvidenceUnavailable {
        expected_unordered_face_pairs,
    })
}

fn remaining_proven_transversal_scan_limits(
    limits: StaticCollisionLimits,
    spent: ZeroThicknessAnalysisWork,
) -> Result<ProvenTransversalScanLimits, StaticCollisionError> {
    let remaining = |limit: usize, used: usize| {
        limit
            .checked_sub(used)
            .ok_or(StaticCollisionError::ResourceLimitExceeded)
    };
    Ok(ProvenTransversalScanLimits {
        max_faces: limits.max_faces,
        max_unordered_face_pairs: limits.max_unordered_face_pairs,
        max_boundary_vertices_per_face: limits.max_boundary_vertices_per_face,
        max_total_boundary_vertices: limits.max_total_boundary_vertices,
        max_total_triangles: remaining(limits.max_total_triangles, spent.total_triangles)?,
        max_total_triangle_pairs: remaining(
            limits.max_total_triangle_pairs,
            spent.total_triangle_pairs,
        )?,
        max_registry_authentication_work: remaining(
            limits.max_registry_authentication_work,
            spent.registry_authentication_work,
        )?,
        max_total_boundary_relation_work: remaining(
            limits.max_total_boundary_relation_work,
            spent.total_boundary_relation_work,
        )?,
        max_rational_input_bits: limits.max_rational_input_bits,
        max_total_rational_input_storage_bits: remaining(
            limits.max_total_rational_input_storage_bits,
            spent.total_rational_input_storage_bits,
        )?,
        max_total_rational_retained_clone_bits: remaining(
            limits.max_total_rational_retained_clone_bits,
            spent.total_rational_retained_clone_bits,
        )?,
        max_rational_operations: remaining(
            limits.max_rational_operations,
            spent.rational_operations,
        )?,
        max_rational_intermediate_bits: limits.max_rational_intermediate_bits,
        max_rational_gcd_fallback_calls: remaining(
            limits.max_rational_gcd_fallback_calls,
            spent.rational_gcd_fallback_calls,
        )?,
        max_rational_gcd_fallback_input_bits: remaining(
            limits.max_rational_gcd_fallback_input_bits,
            spent.rational_gcd_fallback_input_bits,
        )?,
        max_rational_allocations: remaining(
            limits.max_rational_allocations,
            spent.rational_allocations,
        )?,
        max_rational_allocation_bits: limits.max_rational_allocation_bits,
        max_total_rational_allocation_bits: remaining(
            limits.max_total_rational_allocation_bits,
            spent.total_rational_allocation_bits,
        )?,
        max_rational_output_bits: limits.max_rational_output_bits,
        max_total_rational_output_bits: remaining(
            limits.max_total_rational_output_bits,
            spent.total_rational_output_bits,
        )?,
    })
}

fn finish_proven_transversal_scan(
    scan: ProvenTransversalScanSummary,
    expected_unordered_face_pairs: usize,
) -> Result<NativeStaticCollisionGeometryProof, StaticCollisionError> {
    let first_pair_is_canonical = scan
        .first_proven_transversal_pair
        .is_none_or(|(first, second)| first.canonical_bytes() < second.canonical_bytes());
    if scan.enumerated_pairs != expected_unordered_face_pairs
        || scan.proven_transversal_pairs > scan.enumerated_pairs
        || (scan.proven_transversal_pairs == 0) != scan.first_proven_transversal_pair.is_none()
        || !first_pair_is_canonical
    {
        return Err(StaticCollisionError::InconsistentMaterialPose);
    }
    if scan.proven_transversal_pairs > 0 {
        let (first, second) = scan
            .first_proven_transversal_pair
            .ok_or(StaticCollisionError::InconsistentMaterialPose)?;
        return Err(StaticCollisionError::ProvenTransversalPenetration {
            expected_unordered_face_pairs,
            proven_transversal_pairs: scan.proven_transversal_pairs,
            first_proven_transversal_pair: [first, second],
        });
    }
    Err(StaticCollisionError::PairEvidenceUnavailable {
        expected_unordered_face_pairs,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ZeroThicknessDiagnosticScan {
    enumerated_unordered_face_pairs: usize,
    expected_triangle_pairs: usize,
    analyzed_triangle_pairs: usize,
    blocking_unordered_face_pairs: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ZeroThicknessPairRecord {
    expected_triangle_pairs: usize,
    analyzed_triangle_pairs: usize,
    is_blocking: bool,
}

fn scan_authenticated_zero_thickness_pairs(
    analysis: &AuthenticatedZeroThicknessPose<'_>,
    face_count: usize,
    expected_unordered_face_pairs: usize,
) -> Result<ZeroThicknessDiagnosticScan, StaticCollisionError> {
    scan_zero_thickness_pair_records(
        face_count,
        expected_unordered_face_pairs,
        |first_face_index, second_face_index| {
            let dispatch = analysis
                .dispatch_pair(first_face_index, second_face_index)
                .map_err(|error| map_zero_thickness_error(error, expected_unordered_face_pairs))?;
            if !dispatch.has_complete_coverage() {
                return Err(StaticCollisionError::InconsistentMaterialPose);
            }
            let decision = dispatch.decision();
            // Retain an explicit evidence read until the complete native
            // diagnostic snapshot carries the value itself.
            let _evidence = dispatch.evidence();
            Ok(ZeroThicknessPairRecord {
                expected_triangle_pairs: dispatch.expected_triangle_pairs(),
                analyzed_triangle_pairs: dispatch.analyzed_triangle_pairs(),
                is_blocking: !matches!(
                    decision,
                    TopologyContactDecision::Separated
                        | TopologyContactDecision::Touching
                        | TopologyContactDecision::AllowedSharedVertexContact
                ),
            })
        },
    )
}

fn scan_zero_thickness_pair_records(
    face_count: usize,
    expected_unordered_face_pairs: usize,
    mut record_for: impl FnMut(usize, usize) -> Result<ZeroThicknessPairRecord, StaticCollisionError>,
) -> Result<ZeroThicknessDiagnosticScan, StaticCollisionError> {
    let mut pair_work = UnorderedFacePairs::new(face_count);
    let mut expected_triangle_pairs = 0_usize;
    let mut analyzed_triangle_pairs = 0_usize;
    let mut blocking_unordered_face_pairs = 0_usize;
    for (first_face_index, second_face_index) in pair_work.by_ref() {
        let record = record_for(first_face_index, second_face_index)?;
        if record.expected_triangle_pairs != record.analyzed_triangle_pairs {
            return Err(StaticCollisionError::InconsistentMaterialPose);
        }
        expected_triangle_pairs = expected_triangle_pairs
            .checked_add(record.expected_triangle_pairs)
            .ok_or(StaticCollisionError::ResourceLimitExceeded)?;
        analyzed_triangle_pairs = analyzed_triangle_pairs
            .checked_add(record.analyzed_triangle_pairs)
            .ok_or(StaticCollisionError::ResourceLimitExceeded)?;
        if record.is_blocking {
            blocking_unordered_face_pairs = blocking_unordered_face_pairs
                .checked_add(1)
                .ok_or(StaticCollisionError::ResourceLimitExceeded)?;
        }
    }
    if pair_work.enumerated() != expected_unordered_face_pairs {
        return Err(StaticCollisionError::InconsistentMaterialPose);
    }
    Ok(ZeroThicknessDiagnosticScan {
        enumerated_unordered_face_pairs: pair_work.enumerated(),
        expected_triangle_pairs,
        analyzed_triangle_pairs,
        blocking_unordered_face_pairs,
    })
}

fn map_zero_thickness_error(
    error: ZeroThicknessAnalysisError,
    expected_unordered_face_pairs: usize,
) -> StaticCollisionError {
    match error {
        ZeroThicknessAnalysisError::EvidenceUnavailable => {
            StaticCollisionError::PairEvidenceUnavailable {
                expected_unordered_face_pairs,
            }
        }
        ZeroThicknessAnalysisError::ResourceLimitExceeded => {
            StaticCollisionError::ResourceLimitExceeded
        }
    }
}

fn map_proven_transversal_scan_error(
    error: ProvenTransversalScanError,
    expected_unordered_face_pairs: usize,
) -> StaticCollisionError {
    match error {
        ProvenTransversalScanError::EvidenceUnavailable => {
            StaticCollisionError::PairEvidenceUnavailable {
                expected_unordered_face_pairs,
            }
        }
        ProvenTransversalScanError::ResourceLimitExceeded => {
            StaticCollisionError::ResourceLimitExceeded
        }
        ProvenTransversalScanError::InconsistentPose => {
            StaticCollisionError::InconsistentMaterialPose
        }
    }
}

#[derive(Debug, Clone)]
struct UnorderedFacePairs {
    face_count: usize,
    first: usize,
    second: usize,
    enumerated: usize,
}

impl UnorderedFacePairs {
    const fn new(face_count: usize) -> Self {
        Self {
            face_count,
            first: 0,
            second: 1,
            enumerated: 0,
        }
    }

    const fn enumerated(&self) -> usize {
        self.enumerated
    }
}

impl Iterator for UnorderedFacePairs {
    type Item = (usize, usize);

    fn next(&mut self) -> Option<Self::Item> {
        if self.first >= self.face_count || self.second >= self.face_count {
            return None;
        }
        let pair = (self.first, self.second);
        self.enumerated = self.enumerated.checked_add(1)?;
        self.second += 1;
        if self.second == self.face_count {
            self.first += 1;
            self.second = self.first.saturating_add(1);
        }
        Some(pair)
    }
}

fn checked_unordered_pair_count(face_count: usize) -> Result<usize, StaticCollisionError> {
    let Some(previous) = face_count.checked_sub(1) else {
        return Ok(0);
    };
    let (first, second) = if face_count.is_multiple_of(2) {
        (face_count / 2, previous)
    } else {
        (face_count, previous / 2)
    };
    first
        .checked_mul(second)
        .ok_or(StaticCollisionError::ResourceLimitExceeded)
}

#[cfg(test)]
mod tests {
    use ori_domain::FaceId;

    use super::{
        ProvenTransversalScanError, ProvenTransversalScanSummary, StaticCollisionError,
        StaticCollisionLimits, UnorderedFacePairs, ZeroThicknessAnalysisWork,
        ZeroThicknessPairRecord, checked_unordered_pair_count, finish_proven_transversal_scan,
        map_proven_transversal_scan_error, remaining_proven_transversal_scan_limits,
        scan_zero_thickness_pair_records,
    };

    #[test]
    fn default_rational_allocation_limits_are_finite_and_cover_one_value() {
        let limits = StaticCollisionLimits::default();
        assert_ne!(limits.max_rational_allocations, usize::MAX);
        assert_ne!(limits.max_rational_allocation_bits, usize::MAX);
        assert_ne!(limits.max_total_rational_allocation_bits, usize::MAX);
        assert!(limits.max_rational_allocations > 0);
        assert!(
            limits.max_rational_allocation_bits
                >= limits
                    .max_rational_input_bits
                    .max(limits.max_rational_intermediate_bits)
                    .max(limits.max_rational_output_bits)
        );
        assert!(limits.max_total_rational_allocation_bits >= limits.max_rational_allocation_bits);
    }

    #[test]
    fn unordered_pair_arithmetic_is_exact_and_overflow_safe() {
        assert_eq!(checked_unordered_pair_count(0), Ok(0));
        assert_eq!(checked_unordered_pair_count(1), Ok(0));
        assert_eq!(checked_unordered_pair_count(2), Ok(1));
        assert_eq!(checked_unordered_pair_count(3), Ok(3));
        assert_eq!(checked_unordered_pair_count(4), Ok(6));
        assert_eq!(
            checked_unordered_pair_count(usize::MAX),
            Err(StaticCollisionError::ResourceLimitExceeded)
        );
    }

    #[test]
    fn unordered_pair_iterator_covers_every_pair_once_in_canonical_order() {
        for face_count in 0..=8 {
            let expected = checked_unordered_pair_count(face_count).expect("small pair count");
            let mut pairs = UnorderedFacePairs::new(face_count);
            let actual = pairs.by_ref().collect::<Vec<_>>();
            assert_eq!(actual.len(), expected);
            assert_eq!(pairs.enumerated(), expected);
            for (position, &(first, second)) in actual.iter().enumerate() {
                assert!(first < second);
                assert!(second < face_count);
                assert!(
                    actual[..position]
                        .iter()
                        .all(|previous| *previous != (first, second))
                );
            }
            assert!(actual.windows(2).all(|pair| pair[0] < pair[1]));
        }
    }

    #[test]
    fn diagnostic_scan_does_not_stop_after_the_first_blocking_pair() {
        let mut visited = Vec::new();
        let scan = scan_zero_thickness_pair_records(3, 3, |first, second| {
            visited.push((first, second));
            Ok(ZeroThicknessPairRecord {
                expected_triangle_pairs: 2,
                analyzed_triangle_pairs: 2,
                is_blocking: visited.len() == 1,
            })
        })
        .expect("complete diagnostic scan");

        assert_eq!(visited, vec![(0, 1), (0, 2), (1, 2)]);
        assert_eq!(scan.enumerated_unordered_face_pairs, 3);
        assert_eq!(scan.expected_triangle_pairs, 6);
        assert_eq!(scan.analyzed_triangle_pairs, 6);
        assert_eq!(scan.blocking_unordered_face_pairs, 1);
    }

    #[test]
    fn diagnostic_scan_rejects_incomplete_or_miscounted_coverage() {
        assert_eq!(
            scan_zero_thickness_pair_records(2, 1, |_, _| {
                Ok(ZeroThicknessPairRecord {
                    expected_triangle_pairs: 1,
                    analyzed_triangle_pairs: 0,
                    is_blocking: true,
                })
            }),
            Err(StaticCollisionError::InconsistentMaterialPose)
        );

        let mut calls = 0;
        assert_eq!(
            scan_zero_thickness_pair_records(3, 2, |_, _| {
                calls += 1;
                Ok(ZeroThicknessPairRecord {
                    expected_triangle_pairs: 1,
                    analyzed_triangle_pairs: 1,
                    is_blocking: false,
                })
            }),
            Err(StaticCollisionError::InconsistentMaterialPose)
        );
        assert_eq!(
            calls, 3,
            "expected-pair mismatch is checked after the complete scan"
        );
    }

    #[test]
    fn diagnostic_scan_counts_nonblocking_pairs_and_rejects_total_overflow() {
        let scan = scan_zero_thickness_pair_records(3, 3, |_, _| {
            Ok(ZeroThicknessPairRecord {
                expected_triangle_pairs: 1,
                analyzed_triangle_pairs: 1,
                is_blocking: false,
            })
        })
        .expect("complete nonblocking diagnostic scan");
        assert_eq!(scan.blocking_unordered_face_pairs, 0);

        let mut calls = 0;
        assert_eq!(
            scan_zero_thickness_pair_records(3, 3, |_, _| {
                calls += 1;
                let count = if calls == 1 { usize::MAX } else { 1 };
                Ok(ZeroThicknessPairRecord {
                    expected_triangle_pairs: count,
                    analyzed_triangle_pairs: count,
                    is_blocking: false,
                })
            }),
            Err(StaticCollisionError::ResourceLimitExceeded)
        );
        assert_eq!(calls, 2);
    }

    #[test]
    fn transversal_summary_and_error_mapping_fail_closed() {
        let mut proven_pair = [FaceId::new(), FaceId::new()];
        proven_pair.sort_unstable_by_key(FaceId::canonical_bytes);
        assert_eq!(
            finish_proven_transversal_scan(
                ProvenTransversalScanSummary {
                    enumerated_pairs: 3,
                    proven_transversal_pairs: 1,
                    first_proven_transversal_pair: Some((proven_pair[0], proven_pair[1])),
                },
                3,
            )
            .expect_err("affirmative pair remains blocking"),
            StaticCollisionError::ProvenTransversalPenetration {
                expected_unordered_face_pairs: 3,
                proven_transversal_pairs: 1,
                first_proven_transversal_pair: proven_pair,
            }
        );
        assert_eq!(
            finish_proven_transversal_scan(
                ProvenTransversalScanSummary {
                    enumerated_pairs: 3,
                    proven_transversal_pairs: 0,
                    first_proven_transversal_pair: None,
                },
                3,
            )
            .expect_err("zero affirmative pairs retain unavailable evidence"),
            StaticCollisionError::PairEvidenceUnavailable {
                expected_unordered_face_pairs: 3,
            }
        );
        for summary in [
            ProvenTransversalScanSummary {
                enumerated_pairs: 2,
                proven_transversal_pairs: 0,
                first_proven_transversal_pair: None,
            },
            ProvenTransversalScanSummary {
                enumerated_pairs: 3,
                proven_transversal_pairs: 4,
                first_proven_transversal_pair: None,
            },
            ProvenTransversalScanSummary {
                enumerated_pairs: 3,
                proven_transversal_pairs: 1,
                first_proven_transversal_pair: None,
            },
            ProvenTransversalScanSummary {
                enumerated_pairs: 3,
                proven_transversal_pairs: 1,
                first_proven_transversal_pair: Some((proven_pair[1], proven_pair[0])),
            },
        ] {
            assert_eq!(
                finish_proven_transversal_scan(summary, 3)
                    .expect_err("inconsistent affirmative summary"),
                StaticCollisionError::InconsistentMaterialPose
            );
        }
        assert_eq!(
            map_proven_transversal_scan_error(ProvenTransversalScanError::EvidenceUnavailable, 3),
            StaticCollisionError::PairEvidenceUnavailable {
                expected_unordered_face_pairs: 3,
            }
        );
        assert_eq!(
            map_proven_transversal_scan_error(ProvenTransversalScanError::ResourceLimitExceeded, 3,),
            StaticCollisionError::ResourceLimitExceeded
        );
        assert_eq!(
            map_proven_transversal_scan_error(ProvenTransversalScanError::InconsistentPose, 3),
            StaticCollisionError::InconsistentMaterialPose
        );
    }

    #[test]
    fn transversal_budget_subtracts_legacy_ledgers_and_preserves_only_nonadditive_caps() {
        let limits = StaticCollisionLimits::default();
        let spent = ZeroThicknessAnalysisWork {
            total_triangles: 11,
            registry_authentication_work: 12,
            total_triangle_pairs: 13,
            total_boundary_relation_work: 14,
            total_rational_input_storage_bits: 21,
            total_rational_retained_clone_bits: 22,
            rational_operations: 15,
            rational_gcd_fallback_calls: 16,
            rational_gcd_fallback_input_bits: 17,
            rational_allocations: 18,
            total_rational_allocation_bits: 19,
            total_rational_output_bits: 20,
        };
        let remaining = remaining_proven_transversal_scan_limits(limits, spent)
            .expect("legacy work fits the public budget");
        assert_eq!(
            remaining.max_total_triangles,
            limits.max_total_triangles - spent.total_triangles
        );
        assert_eq!(
            remaining.max_registry_authentication_work,
            limits.max_registry_authentication_work - spent.registry_authentication_work
        );
        assert_eq!(
            remaining.max_total_triangle_pairs,
            limits.max_total_triangle_pairs - spent.total_triangle_pairs
        );
        assert_eq!(
            remaining.max_total_boundary_relation_work,
            limits.max_total_boundary_relation_work - spent.total_boundary_relation_work
        );
        assert_eq!(
            remaining.max_rational_operations,
            limits.max_rational_operations - spent.rational_operations
        );
        assert_eq!(
            remaining.max_rational_gcd_fallback_calls,
            limits.max_rational_gcd_fallback_calls - spent.rational_gcd_fallback_calls
        );
        assert_eq!(
            remaining.max_rational_gcd_fallback_input_bits,
            limits.max_rational_gcd_fallback_input_bits - spent.rational_gcd_fallback_input_bits
        );
        assert_eq!(
            remaining.max_rational_allocations,
            limits.max_rational_allocations - spent.rational_allocations
        );
        assert_eq!(
            remaining.max_total_rational_allocation_bits,
            limits.max_total_rational_allocation_bits - spent.total_rational_allocation_bits
        );
        assert_eq!(
            remaining.max_total_rational_output_bits,
            limits.max_total_rational_output_bits - spent.total_rational_output_bits
        );
        assert_eq!(
            remaining.max_total_boundary_vertices,
            limits.max_total_boundary_vertices
        );
        assert_eq!(remaining.max_faces, limits.max_faces);
        assert_eq!(
            remaining.max_unordered_face_pairs,
            limits.max_unordered_face_pairs
        );
        assert_eq!(
            remaining.max_boundary_vertices_per_face,
            limits.max_boundary_vertices_per_face
        );
        assert_eq!(
            remaining.max_rational_input_bits,
            limits.max_rational_input_bits
        );
        assert_eq!(
            remaining.max_rational_intermediate_bits,
            limits.max_rational_intermediate_bits
        );
        assert_eq!(
            remaining.max_rational_allocation_bits,
            limits.max_rational_allocation_bits
        );
        assert_eq!(
            remaining.max_rational_output_bits,
            limits.max_rational_output_bits
        );
        assert_eq!(
            remaining.max_total_rational_input_storage_bits,
            limits.max_total_rational_input_storage_bits - spent.total_rational_input_storage_bits
        );
        assert_eq!(
            remaining.max_total_rational_retained_clone_bits,
            limits.max_total_rational_retained_clone_bits
                - spent.total_rational_retained_clone_bits
        );

        assert_eq!(
            remaining_proven_transversal_scan_limits(
                limits,
                ZeroThicknessAnalysisWork {
                    rational_allocations: limits.max_rational_allocations + 1,
                    ..spent
                },
            ),
            Err(StaticCollisionError::ResourceLimitExceeded)
        );
    }
}
