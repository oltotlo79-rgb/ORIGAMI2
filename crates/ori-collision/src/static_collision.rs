use std::sync::Arc;

use ori_domain::FaceId;
use ori_kinematics::{
    BoundMaterialTreePose, MATERIAL_TREE_KINEMATICS_MODEL_ID, MaterialTreeKinematicsModel,
    MaterialTreePose,
};
use thiserror::Error;

use crate::{
    IntersectionEvidenceV2, TOPOLOGY_CONTACT_POLICY_V2, TopologyContactDecision, TopologyRelation,
    cayley::{
        ProvenTransversalScanError, ProvenTransversalScanLimits, ProvenTransversalScanSummary,
        SharedHingeSolidDiagnosticDispositionV1, SharedHingeSolidDiagnosticErrorV1,
        ZeroThicknessSharedHingeBoundaryDiagnosticErrorV1, diagnose_bound_shared_hinge_solid_v1,
        diagnose_bound_zero_thickness_shared_hinge_boundaries_v1,
        scan_bound_pose_for_proven_transversal_penetration,
    },
    zero_thickness::{
        AuthenticatedZeroThicknessPose, ZeroThicknessAnalysisError, ZeroThicknessAnalysisWork,
        ZeroThicknessGeometryLimits, prepare_authenticated_zero_thickness_pose,
    },
};

/// Initial paper-thickness interpretation used by native collision geometry.
pub const CENTERED_MID_SURFACE_THICKNESS_MODEL_V1: &str = "centered_mid_surface_v1";

/// Maximum number of unordered face-pair diagnostics that may cross the
/// native/renderer boundary in one complete snapshot.
///
/// This is a hard production cap. Caller-provided limits may reduce it but
/// cannot expand it.
pub const NATIVE_STATIC_COLLISION_MAX_PAIR_DIAGNOSTICS_V1: usize = 50_000;

/// First opaque native static-collision geometry-proof format.
///
/// Version 1 admits only the complete zero-pair proof for a no-hinge,
/// single-material-face pose. Exact multi-face diagnostics now authenticate
/// and scan every mid-surface face and triangle pair, but every valid
/// multi-face material tree contains a shared hinge. That pair remains
/// blocking until canonical watertight shared-feature geometry and its finite
/// hinge model exist. A strict mid-surface transversal may now provide a
/// distinct positive-thickness blocking reason, but cannot issue a proof. The
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
    /// Maximum number of zero-thickness hinge pairs submitted to the
    /// watertight exact boundary-only contact theorem.
    pub max_shared_hinge_boundary_diagnostics: usize,
    /// Maximum number of complete two-face/one-hinge solid diagnostics.
    ///
    /// Each admitted diagnostic is additionally constrained by fixed,
    /// non-expandable phase-local exact-arithmetic limits.
    pub max_shared_hinge_solid_diagnostics: usize,
}

impl Default for StaticCollisionLimits {
    fn default() -> Self {
        Self {
            max_faces: 10_001,
            max_unordered_face_pairs: NATIVE_STATIC_COLLISION_MAX_PAIR_DIAGNOSTICS_V1,
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
            max_shared_hinge_boundary_diagnostics: 10_000,
            max_shared_hinge_solid_diagnostics: 1,
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
        "static collision proved {proven_transversal_pairs} zero-thickness penetrating face pairs among {expected_unordered_face_pairs} unordered face pairs"
    )]
    /// Blocking zero-thickness penetration diagnostic.
    ///
    /// The historical public Rust variant and field names are retained for
    /// crate API compatibility. In addition to the Cayley dual-gated transversal path,
    /// this diagnostic now admits issuer-bound exact coplanar positive-area
    /// overlap and exact transversal crossing involving a non-triangular
    /// whole material face.
    ProvenTransversalPenetration {
        expected_unordered_face_pairs: usize,
        proven_transversal_pairs: usize,
        /// First proven pair in canonical `FaceId` byte order.
        ///
        /// This is geometry identity only. It contains no coordinates,
        /// transforms, arithmetic evidence, or internal diagnostic text.
        first_proven_transversal_pair: [FaceId; 2],
    },
    #[error(
        "static collision proved {proven_positive_thickness_pairs} positive-thickness penetrating face pairs among {expected_unordered_face_pairs} unordered face pairs"
    )]
    /// Blocking positive-thickness penetration diagnostic.
    ///
    /// This diagnostic is issued when either the issuer-bound exact-E and
    /// direct-lift-F dual gate proves a strict material mid-surface
    /// transversal, or a connected complete positive-thickness solid
    /// classifier proves `positive_volume_overlap`. Boundary-only contact,
    /// finite-hinge corridor overlap admitted by the centered-mid-surface
    /// model, and every unresolved result remain insufficient. Pair
    /// diagnostics retain the exact provenance so consumers can distinguish
    /// the two affirmative routes.
    ProvenPositiveThicknessPenetration {
        expected_unordered_face_pairs: usize,
        proven_positive_thickness_pairs: usize,
        /// First proven pair in canonical `FaceId` byte order.
        first_proven_positive_thickness_pair: [FaceId; 2],
    },
}

/// Final, read-only classification of one unordered face pair.
///
/// This is diagnostic data, not collision-free authority. In particular,
/// [`Self::Indeterminate`] is blocking and must remain visible to the user.
/// [`Self::CandidateExcluded`] is reserved for same-face policy snapshots;
/// a valid production unordered-pair scan never emits a same-face pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StaticCollisionPairDisposition {
    Separated,
    Touching,
    Allowed,
    Penetrating,
    Indeterminate,
    CandidateExcluded,
}

impl StaticCollisionPairDisposition {
    #[must_use]
    pub const fn identifier(self) -> &'static str {
        match self {
            Self::Separated => "separated",
            Self::Touching => "touching",
            Self::Allowed => "allowed",
            Self::Penetrating => "penetrating",
            Self::Indeterminate => "indeterminate",
            Self::CandidateExcluded => "candidate_excluded",
        }
    }
}

/// Maps one already-authenticated topology/evidence policy cell to the
/// diagnostic vocabulary.
///
/// Constructing the two labels supplied to this function does not authenticate
/// geometry. The production snapshot below invokes this mapping only after the
/// pose-bound exact dispatcher has authenticated both values. A shared-hinge
/// obligation remains `Indeterminate` until the separate finite solid model
/// positively discharges it.
#[must_use]
pub const fn classify_static_collision_pair_disposition(
    topology: TopologyRelation,
    decision: TopologyContactDecision,
) -> StaticCollisionPairDisposition {
    if matches!(topology, TopologyRelation::SameFace) {
        return StaticCollisionPairDisposition::CandidateExcluded;
    }
    match decision {
        TopologyContactDecision::Separated => StaticCollisionPairDisposition::Separated,
        TopologyContactDecision::Touching => StaticCollisionPairDisposition::Touching,
        TopologyContactDecision::AllowedSharedVertexContact => {
            StaticCollisionPairDisposition::Allowed
        }
        TopologyContactDecision::RequiresHingeModel
        | TopologyContactDecision::Indeterminate
        | TopologyContactDecision::IgnoredSelf => StaticCollisionPairDisposition::Indeterminate,
        TopologyContactDecision::Penetrating => StaticCollisionPairDisposition::Penetrating,
    }
}

/// Sanitized, owned diagnosis of one canonical unordered material-face pair.
///
/// The raw evidence and policy decision are exposed so boundary-only contact
/// can be distinguished from a strict transversal. The final disposition may
/// be more conservative than the policy cell when a triangle/triangle
/// transversal has not passed the independent exact-E/direct-F dual gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StaticCollisionPairDiagnostic {
    first_face: FaceId,
    second_face: FaceId,
    topology: TopologyRelation,
    evidence: IntersectionEvidenceV2,
    policy_decision: TopologyContactDecision,
    disposition: StaticCollisionPairDisposition,
    strict_transversal_dual_gate_proven: bool,
    whole_face_overlap_proven: bool,
    shared_hinge_boundary_contact_proven: bool,
    shared_hinge_solid_classified: bool,
}

impl StaticCollisionPairDiagnostic {
    #[must_use]
    pub const fn first_face(&self) -> FaceId {
        self.first_face
    }

    #[must_use]
    pub const fn second_face(&self) -> FaceId {
        self.second_face
    }

    #[must_use]
    pub const fn topology(&self) -> TopologyRelation {
        self.topology
    }

    #[must_use]
    pub const fn evidence(&self) -> IntersectionEvidenceV2 {
        self.evidence
    }

    #[must_use]
    pub const fn policy_decision(&self) -> TopologyContactDecision {
        self.policy_decision
    }

    #[must_use]
    pub const fn disposition(&self) -> StaticCollisionPairDisposition {
        self.disposition
    }

    #[must_use]
    pub const fn strict_transversal_dual_gate_proven(&self) -> bool {
        self.strict_transversal_dual_gate_proven
    }

    #[must_use]
    pub const fn whole_face_overlap_proven(&self) -> bool {
        self.whole_face_overlap_proven
    }

    /// Whether the zero-thickness intersection was exactly proven to be the
    /// complete authenticated shared hinge and nothing outside it.
    #[must_use]
    pub const fn shared_hinge_boundary_contact_proven(&self) -> bool {
        self.shared_hinge_boundary_contact_proven
    }

    #[must_use]
    pub const fn shared_hinge_solid_classified(&self) -> bool {
        self.shared_hinge_solid_classified
    }
}

/// Complete, owned static-collision diagnostic for one exact material pose.
///
/// This value intentionally carries no model, pose, project, or certificate
/// authority. It is suitable for user-visible diagnostics only. Every
/// `Indeterminate` pair is counted explicitly so an unresolved scene cannot
/// be mistaken for a safe scene.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaticCollisionDiagnosticSnapshot {
    face_count: usize,
    expected_unordered_face_pairs: usize,
    pairs: Vec<StaticCollisionPairDiagnostic>,
    separated_pairs: usize,
    touching_pairs: usize,
    allowed_pairs: usize,
    penetrating_pairs: usize,
    indeterminate_pairs: usize,
    candidate_excluded_pairs: usize,
}

impl StaticCollisionDiagnosticSnapshot {
    #[must_use]
    pub const fn face_count(&self) -> usize {
        self.face_count
    }

    #[must_use]
    pub const fn expected_unordered_face_pairs(&self) -> usize {
        self.expected_unordered_face_pairs
    }

    #[must_use]
    pub fn pairs(&self) -> &[StaticCollisionPairDiagnostic] {
        &self.pairs
    }

    #[must_use]
    pub const fn separated_pairs(&self) -> usize {
        self.separated_pairs
    }

    #[must_use]
    pub const fn touching_pairs(&self) -> usize {
        self.touching_pairs
    }

    #[must_use]
    pub const fn allowed_pairs(&self) -> usize {
        self.allowed_pairs
    }

    #[must_use]
    pub const fn penetrating_pairs(&self) -> usize {
        self.penetrating_pairs
    }

    #[must_use]
    pub const fn indeterminate_pairs(&self) -> usize {
        self.indeterminate_pairs
    }

    #[must_use]
    pub const fn candidate_excluded_pairs(&self) -> usize {
        self.candidate_excluded_pairs
    }

    #[must_use]
    pub const fn has_prominent_blocking_hold(&self) -> bool {
        self.penetrating_pairs > 0 || self.indeterminate_pairs > 0
    }
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

#[derive(Debug, Clone, Copy)]
struct ValidatedStaticCollisionInput<'a> {
    bound: BoundMaterialTreePose<'a>,
    face_count: usize,
    expected_unordered_face_pairs: usize,
}

fn validate_static_collision_input<'a>(
    model: &'a MaterialTreeKinematicsModel,
    pose: &'a MaterialTreePose,
    paper_thickness_mm: f64,
    limits: StaticCollisionLimits,
) -> Result<ValidatedStaticCollisionInput<'a>, StaticCollisionError> {
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
    validate_pair_diagnostic_capacity(expected_unordered_face_pairs, limits)?;

    for (index, face) in pose.face_ids().iter().copied().enumerate() {
        if index > 0 && pose.face_ids()[index - 1].canonical_bytes() >= face.canonical_bytes() {
            return Err(StaticCollisionError::InconsistentMaterialPose);
        }
        if pose.face_transform(face).is_none() {
            return Err(StaticCollisionError::InconsistentMaterialPose);
        }
    }

    Ok(ValidatedStaticCollisionInput {
        bound,
        face_count,
        expected_unordered_face_pairs,
    })
}

fn validate_pair_diagnostic_capacity(
    expected_unordered_face_pairs: usize,
    limits: StaticCollisionLimits,
) -> Result<(), StaticCollisionError> {
    if expected_unordered_face_pairs > limits.max_unordered_face_pairs
        || expected_unordered_face_pairs > NATIVE_STATIC_COLLISION_MAX_PAIR_DIAGNOSTICS_V1
    {
        return Err(StaticCollisionError::ResourceLimitExceeded);
    }
    Ok(())
}

/// Proves static collision geometry for one exact native material pose.
///
/// The current implementation intentionally succeeds only for the complete
/// zero-pair case: exactly one material face and no material hinge. A
/// multi-face pose runs authenticated whole-face mid-surface diagnostics, but
/// returns a blocking error at penetration, indeterminate evidence or the
/// mandatory shared-hinge pair. For finite positive thickness, the existing
/// exact-E and direct-lift-F strict transversal is used only as a sufficient
/// condition for a distinct blocking penetration reason. It never expands the
/// collision-free proof set.
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
    let validated = validate_static_collision_input(model, pose, paper_thickness_mm, limits)?;
    let bound = validated.bound;
    let face_count = validated.face_count;
    let expected_unordered_face_pairs = validated.expected_unordered_face_pairs;

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
    let analysis =
        prepare_authenticated_zero_thickness_pose(pose, zero_thickness_geometry_limits(limits))
            .map_err(|error| map_zero_thickness_error(error, expected_unordered_face_pairs))?;
    let (scan, _) =
        scan_authenticated_zero_thickness_pairs(&analysis, pose, expected_unordered_face_pairs)?;
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
    // `scan_authenticated_zero_thickness_pairs` also authenticates that its
    // aggregate is for this exact pose instance, that face identity/order is
    // the pose's canonical registry, and that every unordered face pair and
    // every constituent triangle pair was covered.
    // Multi-face diagnostics cannot issue the public geometry proof yet:
    // every material tree contains at least one shared hinge and the finite
    // hinge model remains mandatory. Keeping the only proof constructor in
    // the zero-pair branch above makes that boundary structural instead of
    // depending on today's decision mix.
    if scan.blocking_unordered_face_pairs > scan.enumerated_unordered_face_pairs {
        return Err(StaticCollisionError::InconsistentMaterialPose);
    }
    if scan.proven_zero_thickness_penetrating_pairs > scan.blocking_unordered_face_pairs
        || (scan.proven_zero_thickness_penetrating_pairs == 0)
            != scan.first_proven_zero_thickness_penetrating_pair.is_none()
    {
        return Err(StaticCollisionError::InconsistentMaterialPose);
    }
    if paper_thickness_mm.to_bits() == 0.0_f64.to_bits()
        && scan.proven_zero_thickness_penetrating_pairs > 0
    {
        let [first, second] = scan
            .first_proven_zero_thickness_penetrating_pair
            .ok_or(StaticCollisionError::InconsistentMaterialPose)?;
        return Err(StaticCollisionError::ProvenTransversalPenetration {
            expected_unordered_face_pairs,
            proven_transversal_pairs: scan.proven_zero_thickness_penetrating_pairs,
            first_proven_transversal_pair: [first, second],
        });
    }
    let is_positive_zero = paper_thickness_mm.to_bits() == 0.0_f64.to_bits();
    let is_positive_thickness = paper_thickness_mm > 0.0;
    let transversal_limits = (is_positive_zero || is_positive_thickness)
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
        return if is_positive_thickness {
            finish_proven_positive_thickness_scan(&transversal, expected_unordered_face_pairs)
        } else {
            finish_proven_transversal_scan(&transversal, expected_unordered_face_pairs)
        };
    }
    Err(StaticCollisionError::PairEvidenceUnavailable {
        expected_unordered_face_pairs,
    })
}

/// Produces a complete, user-visible pair classification for one exact native
/// material pose without issuing collision-free authority.
///
/// Every unordered face pair is retained. Shared-element-only contact cannot
/// become penetration. Conversely, a policy-level penetrating label that has
/// not passed the production admission gate is reported as `Indeterminate`,
/// never silently omitted.
pub fn diagnose_static_collision_geometry(
    model: &MaterialTreeKinematicsModel,
    pose: &MaterialTreePose,
    paper_thickness_mm: f64,
    limits: StaticCollisionLimits,
) -> Result<StaticCollisionDiagnosticSnapshot, StaticCollisionError> {
    let validated = validate_static_collision_input(model, pose, paper_thickness_mm, limits)?;
    let face_count = validated.face_count;
    let expected_unordered_face_pairs = validated.expected_unordered_face_pairs;
    if expected_unordered_face_pairs == 0 {
        return Ok(StaticCollisionDiagnosticSnapshot {
            face_count,
            expected_unordered_face_pairs,
            pairs: Vec::new(),
            separated_pairs: 0,
            touching_pairs: 0,
            allowed_pairs: 0,
            penetrating_pairs: 0,
            indeterminate_pairs: 0,
            candidate_excluded_pairs: 0,
        });
    }

    let analysis =
        prepare_authenticated_zero_thickness_pose(pose, zero_thickness_geometry_limits(limits))
            .map_err(|error| map_zero_thickness_error(error, expected_unordered_face_pairs))?;
    let (scan, authenticated_pairs) =
        scan_authenticated_zero_thickness_pairs(&analysis, pose, expected_unordered_face_pairs)?;
    validate_zero_thickness_diagnostic_scan(&scan, &analysis, expected_unordered_face_pairs)?;

    let is_positive_zero = paper_thickness_mm.to_bits() == 0.0_f64.to_bits();
    let is_positive_thickness = paper_thickness_mm > 0.0;
    // A diagnostic snapshot is deliberately more complete than the blocking
    // proof entry. Even when one whole-face overlap is already sufficient to
    // block, scan the remaining triangle pairs so an unrelated dual-gate
    // transversal is not omitted from the per-pair result.
    let run_transversal_scan = is_positive_zero || is_positive_thickness;
    let transversal_limits = run_transversal_scan
        .then(|| remaining_proven_transversal_scan_limits(limits, analysis.work()))
        .transpose()?;
    drop(analysis);

    let transversal = transversal_limits
        .map(|transversal_limits| {
            scan_bound_pose_for_proven_transversal_penetration(validated.bound, transversal_limits)
                .map_err(|error| {
                    map_proven_transversal_scan_error(error, expected_unordered_face_pairs)
                })
        })
        .transpose()?;
    if let Some(transversal) = transversal.as_ref() {
        validate_proven_transversal_scan(transversal, expected_unordered_face_pairs)?;
    }
    let mut shared_hinge_boundary_candidates = Vec::new();
    if is_positive_zero {
        for pair in &authenticated_pairs {
            let strict_transversal_dual_gate_proven = transversal
                .as_ref()
                .is_some_and(|scan| scan.proves_pair(pair.first_face, pair.second_face));
            let whole_face_overlap_proven = pair.proves_zero_thickness_penetration;
            let raw_boundary_contact_proven = matches!(
                (pair.topology, pair.evidence, pair.decision,),
                (
                    TopologyRelation::SharedHingeEdge,
                    IntersectionEvidenceV2::SharedFeatureContact,
                    TopologyContactDecision::RequiresHingeModel,
                )
            );
            if matches!(pair.topology, TopologyRelation::SharedHingeEdge)
                && !strict_transversal_dual_gate_proven
                && !whole_face_overlap_proven
                && !raw_boundary_contact_proven
            {
                let next_candidate_count = shared_hinge_boundary_candidates
                    .len()
                    .checked_add(1)
                    .ok_or(StaticCollisionError::ResourceLimitExceeded)?;
                if next_candidate_count > limits.max_shared_hinge_boundary_diagnostics {
                    return Err(StaticCollisionError::ResourceLimitExceeded);
                }
                shared_hinge_boundary_candidates
                    .try_reserve_exact(1)
                    .map_err(|_| StaticCollisionError::ResourceLimitExceeded)?;
                shared_hinge_boundary_candidates.push((pair.first_face, pair.second_face));
            }
        }
    }
    let shared_hinge_boundary = (!shared_hinge_boundary_candidates.is_empty())
        .then(|| {
            diagnose_bound_zero_thickness_shared_hinge_boundaries_v1(
                validated.bound,
                &shared_hinge_boundary_candidates,
            )
            .map_err(map_zero_thickness_shared_hinge_boundary_diagnostic_error)
        })
        .transpose()?;
    if let Some(summary) = shared_hinge_boundary.as_ref() {
        let submitted_classified_pairs = shared_hinge_boundary_candidates
            .iter()
            .filter(|(first, second)| {
                summary.proves_boundary_contact_pair(*first, *second)
                    || summary.proves_area_overlap_pair(*first, *second)
            })
            .count();
        if summary.classified_pairs() > shared_hinge_boundary_candidates.len()
            || submitted_classified_pairs != summary.classified_pairs()
        {
            return Err(StaticCollisionError::InconsistentMaterialPose);
        }
    }
    let shared_hinge_solid = if is_positive_thickness && face_count == 2 && pose.hinges().len() == 1
    {
        if limits.max_shared_hinge_solid_diagnostics < 1 {
            return Err(StaticCollisionError::ResourceLimitExceeded);
        }
        diagnose_bound_shared_hinge_solid_v1(validated.bound, paper_thickness_mm)
            .map_err(map_shared_hinge_solid_diagnostic_error)?
    } else {
        None
    };

    let mut pairs = Vec::new();
    pairs
        .try_reserve_exact(expected_unordered_face_pairs)
        .map_err(|_| StaticCollisionError::ResourceLimitExceeded)?;
    let mut shared_hinge_solid_consumed = false;
    for pair in authenticated_pairs {
        let strict_transversal_dual_gate_proven = transversal
            .as_ref()
            .is_some_and(|scan| scan.proves_pair(pair.first_face, pair.second_face));
        let watertight_shared_hinge_area_overlap_proven =
            shared_hinge_boundary.as_ref().is_some_and(|summary| {
                summary.proves_area_overlap_pair(pair.first_face, pair.second_face)
            });
        let whole_face_overlap_proven = is_positive_zero
            && (pair.proves_zero_thickness_penetration
                || watertight_shared_hinge_area_overlap_proven);
        if strict_transversal_dual_gate_proven && whole_face_overlap_proven {
            return Err(StaticCollisionError::InconsistentMaterialPose);
        }
        let mut evidence = pair.evidence;
        let mut policy_decision = pair.decision;
        if strict_transversal_dual_gate_proven {
            evidence = IntersectionEvidenceV2::TransversalCrossing;
            policy_decision = TopologyContactDecision::Penetrating;
        } else if whole_face_overlap_proven {
            evidence = IntersectionEvidenceV2::CoplanarAreaOverlap;
            policy_decision = TopologyContactDecision::Penetrating;
        }
        let mut disposition = if strict_transversal_dual_gate_proven || whole_face_overlap_proven {
            StaticCollisionPairDisposition::Penetrating
        } else {
            let policy = classify_static_collision_pair_disposition(pair.topology, pair.decision);
            if matches!(policy, StaticCollisionPairDisposition::Penetrating) {
                // A raw exact triangle relation is not allowed to bypass the
                // independent exact-E/direct-F admission gate. Positive-
                // thickness coplanar overlap likewise remains an explicit
                // layer-order hold under the centered-mid-surface V1 model.
                StaticCollisionPairDisposition::Indeterminate
            } else {
                policy
            }
        };
        let raw_shared_hinge_boundary_contact_proven = matches!(
            (pair.topology, pair.evidence, pair.decision,),
            (
                TopologyRelation::SharedHingeEdge,
                IntersectionEvidenceV2::SharedFeatureContact,
                TopologyContactDecision::RequiresHingeModel,
            )
        );
        let watertight_shared_hinge_boundary_contact_proven =
            shared_hinge_boundary.as_ref().is_some_and(|summary| {
                summary.proves_boundary_contact_pair(pair.first_face, pair.second_face)
            });
        let shared_hinge_boundary_contact_proven = is_positive_zero
            && !strict_transversal_dual_gate_proven
            && !whole_face_overlap_proven
            && matches!(pair.topology, TopologyRelation::SharedHingeEdge)
            && (raw_shared_hinge_boundary_contact_proven
                || watertight_shared_hinge_boundary_contact_proven);
        if shared_hinge_boundary_contact_proven {
            // With zero thickness there is no material corridor to discharge:
            // authenticated `SharedFeatureContact` proves that every exact
            // contact interval lies on, and collectively covers, only the
            // complete shared hinge. That boundary-only contact is allowed.
            evidence = IntersectionEvidenceV2::SharedFeatureContact;
            policy_decision = TopologyContactDecision::RequiresHingeModel;
            disposition = StaticCollisionPairDisposition::Allowed;
        }
        let mut shared_hinge_solid_classified = false;
        if let Some(shared_hinge) = shared_hinge_solid.as_ref().filter(|shared_hinge| {
            shared_hinge.first_face == pair.first_face
                && shared_hinge.second_face == pair.second_face
        }) {
            if shared_hinge_solid_consumed
                || !matches!(pair.topology, TopologyRelation::SharedHingeEdge)
                || strict_transversal_dual_gate_proven
                || whole_face_overlap_proven
                || shared_hinge_boundary_contact_proven
            {
                return Err(StaticCollisionError::InconsistentMaterialPose);
            }
            shared_hinge_solid_consumed = true;
            shared_hinge_solid_classified = true;
            evidence = shared_hinge.evidence;
            policy_decision = shared_hinge.policy_decision;
            disposition = match shared_hinge.disposition {
                SharedHingeSolidDiagnosticDispositionV1::Allowed => {
                    StaticCollisionPairDisposition::Allowed
                }
                SharedHingeSolidDiagnosticDispositionV1::Penetrating => {
                    StaticCollisionPairDisposition::Penetrating
                }
                SharedHingeSolidDiagnosticDispositionV1::Indeterminate => {
                    StaticCollisionPairDisposition::Indeterminate
                }
            };
        }
        pairs.push(StaticCollisionPairDiagnostic {
            first_face: pair.first_face,
            second_face: pair.second_face,
            topology: pair.topology,
            evidence,
            policy_decision,
            disposition,
            strict_transversal_dual_gate_proven,
            whole_face_overlap_proven,
            shared_hinge_boundary_contact_proven,
            shared_hinge_solid_classified,
        });
    }
    if shared_hinge_solid.is_some() != shared_hinge_solid_consumed {
        return Err(StaticCollisionError::InconsistentMaterialPose);
    }
    build_static_collision_diagnostic_snapshot(face_count, expected_unordered_face_pairs, pairs)
}

fn validate_zero_thickness_diagnostic_scan(
    scan: &ZeroThicknessDiagnosticScan,
    analysis: &AuthenticatedZeroThicknessPose<'_>,
    expected_unordered_face_pairs: usize,
) -> Result<(), StaticCollisionError> {
    if scan.enumerated_unordered_face_pairs != expected_unordered_face_pairs
        || scan.expected_triangle_pairs != analysis.total_triangle_pairs()
        || scan.analyzed_triangle_pairs != analysis.total_triangle_pairs()
        || scan.blocking_unordered_face_pairs > scan.enumerated_unordered_face_pairs
        || scan.proven_zero_thickness_penetrating_pairs > scan.blocking_unordered_face_pairs
        || (scan.proven_zero_thickness_penetrating_pairs == 0)
            != scan.first_proven_zero_thickness_penetrating_pair.is_none()
    {
        return Err(StaticCollisionError::InconsistentMaterialPose);
    }
    Ok(())
}

const fn map_shared_hinge_solid_diagnostic_error(
    error: SharedHingeSolidDiagnosticErrorV1,
) -> StaticCollisionError {
    match error {
        SharedHingeSolidDiagnosticErrorV1::ResourceLimitExceeded => {
            StaticCollisionError::ResourceLimitExceeded
        }
        SharedHingeSolidDiagnosticErrorV1::InconsistentPose => {
            StaticCollisionError::InconsistentMaterialPose
        }
    }
}

const fn map_zero_thickness_shared_hinge_boundary_diagnostic_error(
    error: ZeroThicknessSharedHingeBoundaryDiagnosticErrorV1,
) -> StaticCollisionError {
    match error {
        ZeroThicknessSharedHingeBoundaryDiagnosticErrorV1::ResourceLimitExceeded => {
            StaticCollisionError::ResourceLimitExceeded
        }
        ZeroThicknessSharedHingeBoundaryDiagnosticErrorV1::InconsistentPose => {
            StaticCollisionError::InconsistentMaterialPose
        }
    }
}

fn build_static_collision_diagnostic_snapshot(
    face_count: usize,
    expected_unordered_face_pairs: usize,
    pairs: Vec<StaticCollisionPairDiagnostic>,
) -> Result<StaticCollisionDiagnosticSnapshot, StaticCollisionError> {
    if pairs.len() != expected_unordered_face_pairs
        || pairs.iter().any(|pair| {
            pair.first_face.canonical_bytes() >= pair.second_face.canonical_bytes()
                || matches!(pair.topology, TopologyRelation::SameFace)
                || (pair.shared_hinge_boundary_contact_proven
                    && (!matches!(pair.topology, TopologyRelation::SharedHingeEdge)
                        || !matches!(pair.evidence, IntersectionEvidenceV2::SharedFeatureContact)
                        || !matches!(
                            pair.policy_decision,
                            TopologyContactDecision::RequiresHingeModel
                        )
                        || !matches!(pair.disposition, StaticCollisionPairDisposition::Allowed)
                        || pair.strict_transversal_dual_gate_proven
                        || pair.whole_face_overlap_proven
                        || pair.shared_hinge_solid_classified))
                || (pair.strict_transversal_dual_gate_proven
                    && (!matches!(pair.evidence, IntersectionEvidenceV2::TransversalCrossing)
                        || !matches!(pair.policy_decision, TopologyContactDecision::Penetrating)
                        || !matches!(
                            pair.disposition,
                            StaticCollisionPairDisposition::Penetrating
                        )
                        || pair.whole_face_overlap_proven
                        || pair.shared_hinge_boundary_contact_proven
                        || pair.shared_hinge_solid_classified))
                || (pair.whole_face_overlap_proven
                    && (!matches!(pair.evidence, IntersectionEvidenceV2::CoplanarAreaOverlap)
                        || !matches!(pair.policy_decision, TopologyContactDecision::Penetrating)
                        || !matches!(
                            pair.disposition,
                            StaticCollisionPairDisposition::Penetrating
                        )
                        || pair.strict_transversal_dual_gate_proven
                        || pair.shared_hinge_boundary_contact_proven
                        || pair.shared_hinge_solid_classified))
        })
        || !pairs.windows(2).all(|pair| {
            pair[0].first_face.canonical_bytes() < pair[1].first_face.canonical_bytes()
                || (pair[0].first_face == pair[1].first_face
                    && pair[0].second_face.canonical_bytes()
                        < pair[1].second_face.canonical_bytes())
        })
    {
        return Err(StaticCollisionError::InconsistentMaterialPose);
    }

    let count = |disposition| {
        pairs
            .iter()
            .filter(|pair| pair.disposition == disposition)
            .count()
    };
    let separated_pairs = count(StaticCollisionPairDisposition::Separated);
    let touching_pairs = count(StaticCollisionPairDisposition::Touching);
    let allowed_pairs = count(StaticCollisionPairDisposition::Allowed);
    let penetrating_pairs = count(StaticCollisionPairDisposition::Penetrating);
    let indeterminate_pairs = count(StaticCollisionPairDisposition::Indeterminate);
    let candidate_excluded_pairs = count(StaticCollisionPairDisposition::CandidateExcluded);
    if separated_pairs
        .checked_add(touching_pairs)
        .and_then(|count| count.checked_add(allowed_pairs))
        .and_then(|count| count.checked_add(penetrating_pairs))
        .and_then(|count| count.checked_add(indeterminate_pairs))
        .and_then(|count| count.checked_add(candidate_excluded_pairs))
        != Some(expected_unordered_face_pairs)
        || candidate_excluded_pairs != 0
    {
        return Err(StaticCollisionError::InconsistentMaterialPose);
    }
    Ok(StaticCollisionDiagnosticSnapshot {
        face_count,
        expected_unordered_face_pairs,
        pairs,
        separated_pairs,
        touching_pairs,
        allowed_pairs,
        penetrating_pairs,
        indeterminate_pairs,
        candidate_excluded_pairs,
    })
}

const fn zero_thickness_geometry_limits(
    limits: StaticCollisionLimits,
) -> ZeroThicknessGeometryLimits {
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
        max_boundary_relation_work_per_face_pair: limits.max_boundary_relation_work_per_face_pair,
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
    }
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
    scan: &ProvenTransversalScanSummary,
    expected_unordered_face_pairs: usize,
) -> Result<NativeStaticCollisionGeometryProof, StaticCollisionError> {
    validate_proven_transversal_scan(scan, expected_unordered_face_pairs)?;
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

fn finish_proven_positive_thickness_scan(
    scan: &ProvenTransversalScanSummary,
    expected_unordered_face_pairs: usize,
) -> Result<NativeStaticCollisionGeometryProof, StaticCollisionError> {
    validate_proven_transversal_scan(scan, expected_unordered_face_pairs)?;
    if scan.proven_transversal_pairs > 0 {
        let (first, second) = scan
            .first_proven_transversal_pair
            .ok_or(StaticCollisionError::InconsistentMaterialPose)?;
        return Err(StaticCollisionError::ProvenPositiveThicknessPenetration {
            expected_unordered_face_pairs,
            proven_positive_thickness_pairs: scan.proven_transversal_pairs,
            first_proven_positive_thickness_pair: [first, second],
        });
    }
    Err(StaticCollisionError::PairEvidenceUnavailable {
        expected_unordered_face_pairs,
    })
}

fn validate_proven_transversal_scan(
    scan: &ProvenTransversalScanSummary,
    expected_unordered_face_pairs: usize,
) -> Result<(), StaticCollisionError> {
    let first_pair_is_canonical = scan
        .first_proven_transversal_pair
        .is_none_or(|(first, second)| first.canonical_bytes() < second.canonical_bytes());
    let all_pair_ids_are_canonical_and_sorted = scan
        .proven_transversal_pair_ids
        .iter()
        .all(|(first, second)| first.canonical_bytes() < second.canonical_bytes())
        && scan.proven_transversal_pair_ids.windows(2).all(|pairs| {
            let first = pairs[0];
            let second = pairs[1];
            first.0.canonical_bytes() < second.0.canonical_bytes()
                || (first.0 == second.0 && first.1.canonical_bytes() < second.1.canonical_bytes())
        });
    if scan.enumerated_pairs != expected_unordered_face_pairs
        || scan.proven_transversal_pairs > scan.enumerated_pairs
        || scan.proven_transversal_pairs != scan.proven_transversal_pair_ids.len()
        || (scan.proven_transversal_pairs == 0) != scan.first_proven_transversal_pair.is_none()
        || scan.first_proven_transversal_pair != scan.proven_transversal_pair_ids.first().copied()
        || !first_pair_is_canonical
        || !all_pair_ids_are_canonical_and_sorted
    {
        return Err(StaticCollisionError::InconsistentMaterialPose);
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ZeroThicknessDiagnosticScan {
    enumerated_unordered_face_pairs: usize,
    expected_triangle_pairs: usize,
    analyzed_triangle_pairs: usize,
    blocking_unordered_face_pairs: usize,
    proven_zero_thickness_penetrating_pairs: usize,
    first_proven_zero_thickness_penetrating_pair: Option<[FaceId; 2]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ZeroThicknessPairRecord {
    expected_triangle_pairs: usize,
    analyzed_triangle_pairs: usize,
    is_blocking: bool,
    proves_zero_thickness_penetration: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AuthenticatedZeroThicknessPairDiagnostic {
    first_face: FaceId,
    second_face: FaceId,
    topology: TopologyRelation,
    evidence: IntersectionEvidenceV2,
    decision: TopologyContactDecision,
    proves_zero_thickness_penetration: bool,
}

fn scan_authenticated_zero_thickness_pairs(
    analysis: &AuthenticatedZeroThicknessPose<'_>,
    pose: &MaterialTreePose,
    expected_unordered_face_pairs: usize,
) -> Result<
    (
        ZeroThicknessDiagnosticScan,
        Vec<AuthenticatedZeroThicknessPairDiagnostic>,
    ),
    StaticCollisionError,
> {
    if !analysis.is_for_pose(pose)
        || analysis.face_count() != pose.face_ids().len()
        || pose
            .face_ids()
            .iter()
            .copied()
            .enumerate()
            .any(|(index, face)| analysis.face_id(index) != Some(face))
    {
        return Err(StaticCollisionError::InconsistentMaterialPose);
    }
    let mut diagnostics = Vec::new();
    diagnostics
        .try_reserve_exact(expected_unordered_face_pairs)
        .map_err(|_| StaticCollisionError::ResourceLimitExceeded)?;
    let scan = scan_zero_thickness_pair_records(
        pose.face_ids(),
        expected_unordered_face_pairs,
        |first_face_index, second_face_index| {
            let dispatch = analysis
                .dispatch_pair(first_face_index, second_face_index)
                .map_err(|error| map_zero_thickness_error(error, expected_unordered_face_pairs))?;
            if !dispatch.has_complete_coverage() {
                return Err(StaticCollisionError::InconsistentMaterialPose);
            }
            let decision = dispatch.decision();
            let evidence = dispatch.evidence();
            let is_penetrating_geometry = matches!(
                evidence,
                IntersectionEvidenceV2::TransversalCrossing
                    | IntersectionEvidenceV2::CoplanarAreaOverlap
            );
            if is_penetrating_geometry != matches!(decision, TopologyContactDecision::Penetrating) {
                return Err(StaticCollisionError::InconsistentMaterialPose);
            }
            let first_boundary_vertices = analysis
                .face_boundary_vertex_count(first_face_index)
                .ok_or(StaticCollisionError::InconsistentMaterialPose)?;
            let second_boundary_vertices = analysis
                .face_boundary_vertex_count(second_face_index)
                .ok_or(StaticCollisionError::InconsistentMaterialPose)?;
            let proves_zero_thickness_penetration =
                legacy_dispatch_proves_zero_thickness_penetration(
                    evidence,
                    decision,
                    first_boundary_vertices,
                    second_boundary_vertices,
                );
            let mut face_pair = [
                pose.face_ids()[first_face_index],
                pose.face_ids()[second_face_index],
            ];
            face_pair.sort_unstable_by_key(FaceId::canonical_bytes);
            diagnostics.push(AuthenticatedZeroThicknessPairDiagnostic {
                first_face: face_pair[0],
                second_face: face_pair[1],
                topology: dispatch.topology(),
                evidence,
                decision,
                proves_zero_thickness_penetration,
            });
            Ok(ZeroThicknessPairRecord {
                expected_triangle_pairs: dispatch.expected_triangle_pairs(),
                analyzed_triangle_pairs: dispatch.analyzed_triangle_pairs(),
                is_blocking: !matches!(
                    decision,
                    TopologyContactDecision::Separated
                        | TopologyContactDecision::Touching
                        | TopologyContactDecision::AllowedSharedVertexContact
                ),
                proves_zero_thickness_penetration,
            })
        },
    )?;
    diagnostics.sort_unstable_by(|first, second| {
        first
            .first_face
            .canonical_bytes()
            .cmp(&second.first_face.canonical_bytes())
            .then_with(|| {
                first
                    .second_face
                    .canonical_bytes()
                    .cmp(&second.second_face.canonical_bytes())
            })
    });
    if diagnostics.len() != expected_unordered_face_pairs {
        return Err(StaticCollisionError::InconsistentMaterialPose);
    }
    Ok((scan, diagnostics))
}

const fn legacy_dispatch_proves_zero_thickness_penetration(
    evidence: IntersectionEvidenceV2,
    decision: TopologyContactDecision,
    first_boundary_vertices: usize,
    second_boundary_vertices: usize,
) -> bool {
    // Exact coplanar positive-area overlap is a whole-face affirmative.
    // Exact transversal evidence is admitted here only where the Cayley
    // triangle-only bridge cannot represent at least one whole material
    // face. A triangle/triangle transversal must continue through Cayley's
    // exact-E plus direct-lift-F dual gate below; legacy evidence alone must
    // never weaken that established admission rule.
    if !matches!(decision, TopologyContactDecision::Penetrating) {
        return false;
    }
    match evidence {
        IntersectionEvidenceV2::CoplanarAreaOverlap => true,
        IntersectionEvidenceV2::TransversalCrossing => {
            first_boundary_vertices > 3 || second_boundary_vertices > 3
        }
        _ => false,
    }
}

fn scan_zero_thickness_pair_records(
    face_ids: &[FaceId],
    expected_unordered_face_pairs: usize,
    mut record_for: impl FnMut(usize, usize) -> Result<ZeroThicknessPairRecord, StaticCollisionError>,
) -> Result<ZeroThicknessDiagnosticScan, StaticCollisionError> {
    if !face_ids
        .windows(2)
        .all(|pair| pair[0].canonical_bytes() < pair[1].canonical_bytes())
    {
        return Err(StaticCollisionError::InconsistentMaterialPose);
    }
    let mut pair_work = UnorderedFacePairs::new(face_ids.len());
    let mut expected_triangle_pairs = 0_usize;
    let mut analyzed_triangle_pairs = 0_usize;
    let mut blocking_unordered_face_pairs = 0_usize;
    let mut proven_zero_thickness_penetrating_pairs = 0_usize;
    let mut first_proven_zero_thickness_penetrating_pair = None;
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
        if record.proves_zero_thickness_penetration {
            if !record.is_blocking {
                return Err(StaticCollisionError::InconsistentMaterialPose);
            }
            proven_zero_thickness_penetrating_pairs = proven_zero_thickness_penetrating_pairs
                .checked_add(1)
                .ok_or(StaticCollisionError::ResourceLimitExceeded)?;
            first_proven_zero_thickness_penetrating_pair
                .get_or_insert([face_ids[first_face_index], face_ids[second_face_index]]);
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
        proven_zero_thickness_penetrating_pairs,
        first_proven_zero_thickness_penetrating_pair,
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
        NATIVE_STATIC_COLLISION_MAX_PAIR_DIAGNOSTICS_V1, ProvenTransversalScanError,
        ProvenTransversalScanSummary, StaticCollisionError, StaticCollisionLimits,
        UnorderedFacePairs, ZeroThicknessAnalysisWork, ZeroThicknessPairRecord,
        checked_unordered_pair_count, finish_proven_positive_thickness_scan,
        finish_proven_transversal_scan, legacy_dispatch_proves_zero_thickness_penetration,
        map_proven_transversal_scan_error, remaining_proven_transversal_scan_limits,
        scan_zero_thickness_pair_records, validate_pair_diagnostic_capacity,
    };
    use crate::{IntersectionEvidenceV2, TopologyContactDecision};

    fn canonical_face_ids(count: usize) -> Vec<FaceId> {
        let mut faces = (0..count).map(|_| FaceId::new()).collect::<Vec<_>>();
        faces.sort_unstable_by_key(FaceId::canonical_bytes);
        faces
    }

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
    fn pair_diagnostic_ipc_cap_accepts_exact_and_rejects_one_over() {
        let exact = NATIVE_STATIC_COLLISION_MAX_PAIR_DIAGNOSTICS_V1;
        assert_eq!(
            validate_pair_diagnostic_capacity(
                exact,
                StaticCollisionLimits {
                    max_unordered_face_pairs: exact,
                    ..StaticCollisionLimits::default()
                },
            ),
            Ok(())
        );
        assert_eq!(
            validate_pair_diagnostic_capacity(
                exact + 1,
                StaticCollisionLimits {
                    max_unordered_face_pairs: exact + 1,
                    ..StaticCollisionLimits::default()
                },
            ),
            Err(StaticCollisionError::ResourceLimitExceeded)
        );
        assert_eq!(
            validate_pair_diagnostic_capacity(
                exact,
                StaticCollisionLimits {
                    max_unordered_face_pairs: exact - 1,
                    ..StaticCollisionLimits::default()
                },
            ),
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
        let faces = canonical_face_ids(3);
        let mut visited = Vec::new();
        let scan = scan_zero_thickness_pair_records(&faces, 3, |first, second| {
            visited.push((first, second));
            Ok(ZeroThicknessPairRecord {
                expected_triangle_pairs: 2,
                analyzed_triangle_pairs: 2,
                is_blocking: visited.len() == 1,
                proves_zero_thickness_penetration: visited.len() == 1,
            })
        })
        .expect("complete diagnostic scan");

        assert_eq!(visited, vec![(0, 1), (0, 2), (1, 2)]);
        assert_eq!(scan.enumerated_unordered_face_pairs, 3);
        assert_eq!(scan.expected_triangle_pairs, 6);
        assert_eq!(scan.analyzed_triangle_pairs, 6);
        assert_eq!(scan.blocking_unordered_face_pairs, 1);
        assert_eq!(scan.proven_zero_thickness_penetrating_pairs, 1);
        assert_eq!(
            scan.first_proven_zero_thickness_penetrating_pair,
            Some([faces[0], faces[1]])
        );
    }

    #[test]
    fn diagnostic_scan_rejects_incomplete_or_miscounted_coverage() {
        let two_faces = canonical_face_ids(2);
        assert_eq!(
            scan_zero_thickness_pair_records(&two_faces, 1, |_, _| {
                Ok(ZeroThicknessPairRecord {
                    expected_triangle_pairs: 1,
                    analyzed_triangle_pairs: 0,
                    is_blocking: true,
                    proves_zero_thickness_penetration: false,
                })
            }),
            Err(StaticCollisionError::InconsistentMaterialPose)
        );

        let three_faces = canonical_face_ids(3);
        let mut calls = 0;
        assert_eq!(
            scan_zero_thickness_pair_records(&three_faces, 2, |_, _| {
                calls += 1;
                Ok(ZeroThicknessPairRecord {
                    expected_triangle_pairs: 1,
                    analyzed_triangle_pairs: 1,
                    is_blocking: false,
                    proves_zero_thickness_penetration: false,
                })
            }),
            Err(StaticCollisionError::InconsistentMaterialPose)
        );
        assert_eq!(
            calls, 3,
            "expected-pair mismatch is checked after the complete scan"
        );

        let mut noncanonical_faces = three_faces;
        noncanonical_faces.swap(0, 1);
        assert_eq!(
            scan_zero_thickness_pair_records(&noncanonical_faces, 3, |_, _| unreachable!()),
            Err(StaticCollisionError::InconsistentMaterialPose)
        );
    }

    #[test]
    fn diagnostic_scan_counts_nonblocking_pairs_and_rejects_total_overflow() {
        let faces = canonical_face_ids(3);
        let scan = scan_zero_thickness_pair_records(&faces, 3, |_, _| {
            Ok(ZeroThicknessPairRecord {
                expected_triangle_pairs: 1,
                analyzed_triangle_pairs: 1,
                is_blocking: false,
                proves_zero_thickness_penetration: false,
            })
        })
        .expect("complete nonblocking diagnostic scan");
        assert_eq!(scan.blocking_unordered_face_pairs, 0);
        assert_eq!(scan.proven_zero_thickness_penetrating_pairs, 0);
        assert_eq!(scan.first_proven_zero_thickness_penetrating_pair, None);

        let mut calls = 0;
        assert_eq!(
            scan_zero_thickness_pair_records(&faces, 3, |_, _| {
                calls += 1;
                let count = if calls == 1 { usize::MAX } else { 1 };
                Ok(ZeroThicknessPairRecord {
                    expected_triangle_pairs: count,
                    analyzed_triangle_pairs: count,
                    is_blocking: false,
                    proves_zero_thickness_penetration: false,
                })
            }),
            Err(StaticCollisionError::ResourceLimitExceeded)
        );
        assert_eq!(calls, 2);

        assert_eq!(
            scan_zero_thickness_pair_records(&faces[..2], 1, |_, _| {
                Ok(ZeroThicknessPairRecord {
                    expected_triangle_pairs: 1,
                    analyzed_triangle_pairs: 1,
                    is_blocking: false,
                    proves_zero_thickness_penetration: true,
                })
            }),
            Err(StaticCollisionError::InconsistentMaterialPose)
        );
    }

    #[test]
    fn legacy_affirmative_keeps_triangle_transversal_behind_the_cayley_dual_gate() {
        assert!(!legacy_dispatch_proves_zero_thickness_penetration(
            IntersectionEvidenceV2::TransversalCrossing,
            TopologyContactDecision::Penetrating,
            3,
            3,
        ));
        assert!(legacy_dispatch_proves_zero_thickness_penetration(
            IntersectionEvidenceV2::TransversalCrossing,
            TopologyContactDecision::Penetrating,
            4,
            3,
        ));
        assert!(legacy_dispatch_proves_zero_thickness_penetration(
            IntersectionEvidenceV2::CoplanarAreaOverlap,
            TopologyContactDecision::Penetrating,
            3,
            3,
        ));
        for evidence in [
            IntersectionEvidenceV2::PointContact,
            IntersectionEvidenceV2::BoundaryLineContact,
            IntersectionEvidenceV2::SharedFeatureContact,
            IntersectionEvidenceV2::BoundaryAreaContact,
        ] {
            assert!(!legacy_dispatch_proves_zero_thickness_penetration(
                evidence,
                TopologyContactDecision::Touching,
                4,
                4,
            ));
        }
    }

    #[test]
    fn transversal_summary_and_error_mapping_fail_closed() {
        let mut proven_pair = [FaceId::new(), FaceId::new()];
        proven_pair.sort_unstable_by_key(FaceId::canonical_bytes);
        assert_eq!(
            finish_proven_transversal_scan(
                &ProvenTransversalScanSummary {
                    enumerated_pairs: 3,
                    proven_transversal_pairs: 1,
                    first_proven_transversal_pair: Some((proven_pair[0], proven_pair[1])),
                    proven_transversal_pair_ids: vec![(proven_pair[0], proven_pair[1])],
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
            finish_proven_positive_thickness_scan(
                &ProvenTransversalScanSummary {
                    enumerated_pairs: 3,
                    proven_transversal_pairs: 1,
                    first_proven_transversal_pair: Some((proven_pair[0], proven_pair[1])),
                    proven_transversal_pair_ids: vec![(proven_pair[0], proven_pair[1])],
                },
                3,
            )
            .expect_err("positive-thickness affirmative pair remains blocking"),
            StaticCollisionError::ProvenPositiveThicknessPenetration {
                expected_unordered_face_pairs: 3,
                proven_positive_thickness_pairs: 1,
                first_proven_positive_thickness_pair: proven_pair,
            }
        );
        assert_eq!(
            finish_proven_transversal_scan(
                &ProvenTransversalScanSummary {
                    enumerated_pairs: 3,
                    proven_transversal_pairs: 0,
                    first_proven_transversal_pair: None,
                    proven_transversal_pair_ids: Vec::new(),
                },
                3,
            )
            .expect_err("zero affirmative pairs retain unavailable evidence"),
            StaticCollisionError::PairEvidenceUnavailable {
                expected_unordered_face_pairs: 3,
            }
        );
        assert_eq!(
            finish_proven_positive_thickness_scan(
                &ProvenTransversalScanSummary {
                    enumerated_pairs: 3,
                    proven_transversal_pairs: 0,
                    first_proven_transversal_pair: None,
                    proven_transversal_pair_ids: Vec::new(),
                },
                3,
            )
            .expect_err("zero positive-thickness pairs retain unavailable evidence"),
            StaticCollisionError::PairEvidenceUnavailable {
                expected_unordered_face_pairs: 3,
            }
        );
        for summary in [
            ProvenTransversalScanSummary {
                enumerated_pairs: 2,
                proven_transversal_pairs: 0,
                first_proven_transversal_pair: None,
                proven_transversal_pair_ids: Vec::new(),
            },
            ProvenTransversalScanSummary {
                enumerated_pairs: 3,
                proven_transversal_pairs: 4,
                first_proven_transversal_pair: None,
                proven_transversal_pair_ids: Vec::new(),
            },
            ProvenTransversalScanSummary {
                enumerated_pairs: 3,
                proven_transversal_pairs: 1,
                first_proven_transversal_pair: None,
                proven_transversal_pair_ids: vec![(proven_pair[0], proven_pair[1])],
            },
            ProvenTransversalScanSummary {
                enumerated_pairs: 3,
                proven_transversal_pairs: 1,
                first_proven_transversal_pair: Some((proven_pair[1], proven_pair[0])),
                proven_transversal_pair_ids: vec![(proven_pair[1], proven_pair[0])],
            },
        ] {
            assert_eq!(
                finish_proven_transversal_scan(&summary, 3)
                    .expect_err("inconsistent affirmative summary"),
                StaticCollisionError::InconsistentMaterialPose
            );
            assert_eq!(
                finish_proven_positive_thickness_scan(&summary, 3)
                    .expect_err("positive-thickness summary uses the same validation"),
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
