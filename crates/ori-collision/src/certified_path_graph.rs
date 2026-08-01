//! Deterministic bounded graph search over a native certified-transition oracle.
//!
//! This module is observation-only. Its result never authorizes project
//! mutation: an oracle must independently certify every admitted transition.

use std::collections::VecDeque;

use ori_domain::{
    FaceId, MAX_PATH_CERTIFICATE_REFERENCE_TRANSITIONS_V1, PATH_CERTIFICATE_REFERENCE_MODEL_ID_V1,
    PathCertificateReferenceV1,
};
use ori_kinematics::{
    CanonicalHingeAngles, DyadicMaterialHingeIntervalClosureCertificateV1,
    GeneratedMultiHingePathCandidateV1, MaterialHingeGraphAudit, MaterialHingeGraphGeometry,
    MaterialTreeKinematicsModel, MaterialTreePose,
};
use sha2::{Digest, Sha256};

use crate::block_composition::{
    CommonArticulationContinuousLayerPathAuthorityV1,
    CommonArticulationContinuousLayerPathRevalidationInputV1,
};
use crate::continuous_path::diagnose_scheduled_cycle_path_v1;

pub const CERTIFIED_PATH_GRAPH_MODEL_ID_V1: &str = "bounded_certified_pose_graph_path_v1";
pub const MAX_CERTIFIED_PATH_GRAPH_STATES_V1: usize = 2_187;
pub const MAX_CERTIFIED_PATH_GRAPH_TRANSITIONS_V1: usize = 20_412;
/// Additional issuer-certified edges layered over a full dyadic adjacency.
pub const MAX_CERTIFIED_PATH_GRAPH_OVERLAY_EDGES_V1: usize = 5;
pub const MAX_CERTIFIED_PATH_GRAPH_CANDIDATES_V1: usize =
    MAX_CERTIFIED_PATH_GRAPH_TRANSITIONS_V1 + MAX_CERTIFIED_PATH_GRAPH_OVERLAY_EDGES_V1;

pub type PoseFingerprintV1 = [u8; 32];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CertifiedPathTransitionCandidateV1 {
    pub source: PoseFingerprintV1,
    pub target: PoseFingerprintV1,
    /// Stable oracle-specific ordering key. It contains no project identity.
    pub candidate_key: [u8; 32],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Opaque evidence issued only after an `ori-collision` native oracle has
/// revalidated the exact transition endpoints.
///
/// External crates cannot manufacture evidence from detached digests:
///
/// ```compile_fail
/// use ori_collision::CertifiedPathTransitionEvidenceV1;
/// let _ = CertifiedPathTransitionEvidenceV1::from_native_oracle(
///     [1; 32], [2; 32], [3; 32], [4; 32], [5; 32], None, None,
/// );
/// ```
pub struct CertifiedPathTransitionEvidenceV1 {
    source: PoseFingerprintV1,
    target: PoseFingerprintV1,
    schedule_certificate: [u8; 32],
    collision_certificate: [u8; 32],
    closure_certificate: [u8; 32],
    issuer_seal: CertifiedPathTransitionIssuerSealV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CertifiedPathTransitionIssuerSealV1 {
    NativeGraph {
        fixed_face: Option<FaceId>,
        fold_model_fingerprint_v1: Option<[u8; 32]>,
    },
    InstructionBoundNative {
        fixed_face: FaceId,
        source_model_binding_sha256: [u8; 32],
    },
    #[cfg(feature = "private-petal-e2e")]
    UntrustedPrivatePetalE2eFixture,
}

impl CertifiedPathTransitionEvidenceV1 {
    #[must_use]
    pub(crate) const fn from_native_oracle(
        source: PoseFingerprintV1,
        target: PoseFingerprintV1,
        schedule_certificate: [u8; 32],
        collision_certificate: [u8; 32],
        closure_certificate: [u8; 32],
        fixed_face: Option<FaceId>,
        fold_model_fingerprint_v1: Option<[u8; 32]>,
    ) -> Self {
        Self {
            source,
            target,
            schedule_certificate,
            collision_certificate,
            closure_certificate,
            issuer_seal: CertifiedPathTransitionIssuerSealV1::NativeGraph {
                fixed_face,
                fold_model_fingerprint_v1,
            },
        }
    }

    const fn from_instruction_bound_native_oracle(
        source: PoseFingerprintV1,
        target: PoseFingerprintV1,
        schedule_certificate: [u8; 32],
        collision_certificate: [u8; 32],
        closure_certificate: [u8; 32],
        fixed_face: FaceId,
        source_model_binding_sha256: [u8; 32],
    ) -> Self {
        Self {
            source,
            target,
            schedule_certificate,
            collision_certificate,
            closure_certificate,
            issuer_seal: CertifiedPathTransitionIssuerSealV1::InstructionBoundNative {
                fixed_face,
                source_model_binding_sha256,
            },
        }
    }

    #[must_use]
    pub const fn source(&self) -> PoseFingerprintV1 {
        self.source
    }
    #[must_use]
    pub const fn target(&self) -> PoseFingerprintV1 {
        self.target
    }
    #[must_use]
    pub const fn schedule_certificate(&self) -> [u8; 32] {
        self.schedule_certificate
    }
    #[must_use]
    pub const fn collision_certificate(&self) -> [u8; 32] {
        self.collision_certificate
    }
    #[must_use]
    pub const fn closure_certificate(&self) -> [u8; 32] {
        self.closure_certificate
    }
}

/// Untrusted fixture boundary for cross-crate private-petal integration tests.
/// Cargo features are not a trust boundary: any external caller may enable
/// this function, so its private seal is always non-native and both registry
/// registration and export attestation reject every certificate containing it.
#[cfg(feature = "private-petal-e2e")]
#[doc(hidden)]
#[must_use]
pub const fn private_petal_e2e_transition_fixture_v1(
    source: PoseFingerprintV1,
    target: PoseFingerprintV1,
    schedule_certificate: [u8; 32],
    collision_certificate: [u8; 32],
    closure_certificate: [u8; 32],
) -> CertifiedPathTransitionEvidenceV1 {
    CertifiedPathTransitionEvidenceV1 {
        source,
        target,
        schedule_certificate,
        collision_certificate,
        closure_certificate,
        issuer_seal: CertifiedPathTransitionIssuerSealV1::UntrustedPrivatePetalE2eFixture,
    }
}

/// Adapts the existing schedule, full-domain closure and bounded CCD oracles
/// into one graph edge. Any missing or mismatched certificate rejects the
/// edge; an unresolved CCD result is never interpreted as collision-free.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn certify_scheduled_cycle_transition_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    candidate: &GeneratedMultiHingePathCandidateV1,
    closure: &DyadicMaterialHingeIntervalClosureCertificateV1,
    interval_count: usize,
) -> Option<CertifiedPathTransitionEvidenceV1> {
    let schedule = candidate.schedule();
    let schedule_source = schedule.evaluate(0.0)?;
    let schedule_target = schedule.evaluate(1.0)?;
    if closure.fixed_face() != fixed_face
        || !closure.every_leaf_covers_graph_v1(geometry)
        || closure.schedule_binding_fingerprint_v2()
            != schedule.certificate_binding_fingerprint_v2()
        || closure.graph_binding_fingerprint_v1() != schedule.graph_binding_fingerprint_v1()
        || !schedule.matches_binding(geometry, audit, fixed_face)
    {
        return None;
    }
    let source = canonical_pose_fingerprint_v1(&schedule_source);
    let target = canonical_pose_fingerprint_v1(&schedule_target);
    if source == target {
        return None;
    }
    let collision = diagnose_scheduled_cycle_path_v1(
        geometry,
        audit,
        fixed_face,
        candidate,
        closure,
        interval_count,
    );
    collision.continuous_certificate_model_id()?;
    let schedule_certificate = schedule.certificate_binding_fingerprint_v2();
    let closure_certificate = hash_certificate_binding(
        b"dyadic_material_hinge_interval_closure_certificate_v1",
        &[
            &schedule_certificate,
            &closure.graph_binding_fingerprint_v1(),
            &closure.partition_binding_fingerprint_v2(),
        ],
    );
    let collision_certificate = hash_certificate_binding(
        b"stacked_fold_cycle_interval_continuous_certificate_v1",
        &[
            &schedule_certificate,
            &closure_certificate,
            &(collision.leaf_count() as u64).to_be_bytes(),
            &(collision.pair_work() as u64).to_be_bytes(),
        ],
    );
    Some(CertifiedPathTransitionEvidenceV1::from_native_oracle(
        source,
        target,
        schedule_certificate,
        collision_certificate,
        closure_certificate,
        Some(fixed_face),
        geometry.fold_model_fingerprint_v1(),
    ))
}

/// Adapts exact full-domain closure and positive-thickness Tree certificates
/// into a scheduled graph edge. The endpoints are derived inside this crate
/// from the revalidated schedule; callers cannot bind the native proof to
/// arbitrary fingerprints.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn certify_positive_thickness_tree_scheduled_transition_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    candidate: &GeneratedMultiHingePathCandidateV1,
    closure: &DyadicMaterialHingeIntervalClosureCertificateV1,
    model: &MaterialTreeKinematicsModel,
    source_pose: &MaterialTreePose,
    target: &CanonicalHingeAngles,
    paper_thickness_mm: f64,
    positive: &crate::PositiveThicknessTreeContinuousCertificateV1,
) -> Option<CertifiedPathTransitionEvidenceV1> {
    let schedule = candidate.schedule();
    let schedule_source = schedule.evaluate(0.0)?;
    let schedule_target = schedule.evaluate(1.0)?;
    if closure.fixed_face() != fixed_face
        || !closure.every_leaf_covers_graph_v1(geometry)
        || closure.schedule_binding_fingerprint_v2()
            != schedule.certificate_binding_fingerprint_v2()
        || closure.graph_binding_fingerprint_v1() != schedule.graph_binding_fingerprint_v1()
        || source_pose.fixed_face() != Some(fixed_face)
        || !schedule.matches_binding(geometry, audit, fixed_face)
        || !bit_exact_pose_angles_match_v1(&schedule_source, source_pose)
        || !bit_exact_canonical_angles_match_v1(&schedule_target, target)
        || !positive.is_for(model, source_pose, target, paper_thickness_mm)
    {
        return None;
    }
    let source = canonical_pose_fingerprint_v1(&schedule_source);
    let target = canonical_pose_fingerprint_v1(&schedule_target);
    if source == target {
        return None;
    }
    let schedule_certificate = schedule.certificate_binding_fingerprint_v2();
    let collision_certificate = positive.binding_fingerprint_v1();
    let closure_certificate = closure.partition_binding_fingerprint_v2();
    Some(CertifiedPathTransitionEvidenceV1::from_native_oracle(
        source,
        target,
        schedule_certificate,
        collision_certificate,
        closure_certificate,
        Some(fixed_face),
        geometry.fold_model_fingerprint_v1(),
    ))
}

fn hash_certificate_binding(domain: &[u8], fields: &[&[u8]]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(domain);
    for field in fields {
        hash.update((field.len() as u64).to_be_bytes());
        hash.update(field);
    }
    hash.finalize().into()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CertifiedPoseGraphPathCertificateV1 {
    source: PoseFingerprintV1,
    target: PoseFingerprintV1,
    edges: Vec<CertifiedPathTransitionEvidenceV1>,
    explored_state_count: usize,
    evaluated_transition_count: usize,
}

impl CertifiedPoseGraphPathCertificateV1 {
    #[must_use]
    pub const fn model_id(&self) -> &'static str {
        CERTIFIED_PATH_GRAPH_MODEL_ID_V1
    }
    #[must_use]
    pub const fn version(&self) -> u8 {
        1
    }
    #[must_use]
    pub const fn source(&self) -> PoseFingerprintV1 {
        self.source
    }
    #[must_use]
    pub const fn target(&self) -> PoseFingerprintV1 {
        self.target
    }
    #[must_use]
    pub fn edges(&self) -> &[CertifiedPathTransitionEvidenceV1] {
        &self.edges
    }
    /// Fallibly copies this bounded certificate without using an unchecked
    /// `Vec` clone at a fail-closed registry boundary.
    #[must_use]
    pub fn try_clone_v1(&self) -> Option<Self> {
        let mut edges = Vec::new();
        edges.try_reserve_exact(self.edges.len()).ok()?;
        edges.extend_from_slice(&self.edges);
        Some(Self {
            source: self.source,
            target: self.target,
            edges,
            explored_state_count: self.explored_state_count,
            evaluated_transition_count: self.evaluated_transition_count,
        })
    }
    /// Derives a non-authorizing single-transition view for compilers that
    /// verify an ordered multi-segment motion one transition at a time.
    #[must_use]
    pub fn segment_certificate_v1(&self, index: usize) -> Option<Self> {
        let edge = *self.edges.get(index)?;
        let mut edges = Vec::new();
        edges.try_reserve_exact(1).ok()?;
        edges.push(edge);
        Some(Self {
            source: edge.source(),
            target: edge.target(),
            edges,
            explored_state_count: self.explored_state_count,
            evaluated_transition_count: self.evaluated_transition_count,
        })
    }
    #[must_use]
    pub const fn explored_state_count(&self) -> usize {
        self.explored_state_count
    }
    #[must_use]
    pub const fn evaluated_transition_count(&self) -> usize {
        self.evaluated_transition_count
    }
    #[must_use]
    pub const fn authorizes_project_mutation(&self) -> bool {
        false
    }

    /// Returns whether every edge came from a production native issuer. Test
    /// fixtures deliberately return `false` and cannot cross an export or
    /// registry trust boundary.
    #[must_use]
    pub fn is_native_attestable_v1(&self) -> bool {
        match self.edges.first().map(|edge| edge.issuer_seal) {
            Some(CertifiedPathTransitionIssuerSealV1::NativeGraph {
                fixed_face: Some(fixed_face),
                fold_model_fingerprint_v1: Some(fold_model_fingerprint_v1),
            }) => self.edges.iter().all(|edge| {
                matches!(
                    edge.issuer_seal,
                    CertifiedPathTransitionIssuerSealV1::NativeGraph {
                        fixed_face: Some(edge_fixed_face),
                        fold_model_fingerprint_v1: Some(edge_fold_model),
                    } if edge_fixed_face == fixed_face
                        && edge_fold_model == fold_model_fingerprint_v1
                )
            }),
            Some(CertifiedPathTransitionIssuerSealV1::InstructionBoundNative {
                fixed_face,
                source_model_binding_sha256,
            }) => self.edges.iter().all(|edge| {
                matches!(
                    edge.issuer_seal,
                    CertifiedPathTransitionIssuerSealV1::InstructionBoundNative {
                        fixed_face: edge_fixed_face,
                        source_model_binding_sha256: edge_model_binding,
                    } if edge_fixed_face == fixed_face
                        && edge_model_binding == source_model_binding_sha256
                )
            }),
            _ => false,
        }
    }

    /// Fixed material face retained by every native issuer edge. Mixed,
    /// unbound, and test-fixture paths return `None`.
    #[must_use]
    pub fn native_fixed_face_v1(&self) -> Option<FaceId> {
        if !self.is_native_attestable_v1() {
            return None;
        }
        match self.edges.first()?.issuer_seal {
            CertifiedPathTransitionIssuerSealV1::NativeGraph {
                fixed_face: Some(fixed_face),
                ..
            } => Some(fixed_face),
            CertifiedPathTransitionIssuerSealV1::NativeGraph {
                fixed_face: None, ..
            } => None,
            CertifiedPathTransitionIssuerSealV1::InstructionBoundNative { fixed_face, .. } => {
                Some(fixed_face)
            }
            #[cfg(feature = "private-petal-e2e")]
            CertifiedPathTransitionIssuerSealV1::UntrustedPrivatePetalE2eFixture => None,
        }
    }

    /// Returns the exact source-model binding only when every native edge was
    /// issued by the instruction-domain adapter for the same model. Raw graph
    /// certificates deliberately return `None`.
    #[must_use]
    pub fn native_source_model_binding_v1(&self) -> Option<[u8; 32]> {
        if !self.is_native_attestable_v1() {
            return None;
        }
        let CertifiedPathTransitionIssuerSealV1::InstructionBoundNative {
            source_model_binding_sha256,
            ..
        } = self.edges.first()?.issuer_seal
        else {
            return None;
        };
        self.edges
            .iter()
            .all(|edge| {
                matches!(
                    edge.issuer_seal,
                    CertifiedPathTransitionIssuerSealV1::InstructionBoundNative {
                        source_model_binding_sha256: edge_binding,
                        ..
                    } if edge_binding == source_model_binding_sha256
                )
            })
            .then_some(source_model_binding_sha256)
    }

    /// Canonical digest binding every endpoint, transition proof and search count.
    #[must_use]
    pub fn binding_fingerprint_v1(&self) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(b"certified_pose_graph_path_certificate_binding_v1");
        hash.update(self.source);
        hash.update(self.target);
        hash.update((self.edges.len() as u64).to_be_bytes());
        for edge in &self.edges {
            hash.update(edge.source());
            hash.update(edge.target());
            hash.update(edge.schedule_certificate());
            hash.update(edge.collision_certificate());
            hash.update(edge.closure_certificate());
            match edge.issuer_seal {
                CertifiedPathTransitionIssuerSealV1::NativeGraph {
                    fixed_face,
                    fold_model_fingerprint_v1,
                } => {
                    hash.update([1]);
                    match fixed_face {
                        Some(fixed_face) => {
                            hash.update([1]);
                            hash.update(fixed_face.canonical_bytes());
                        }
                        None => hash.update([0]),
                    }
                    match fold_model_fingerprint_v1 {
                        Some(fold_model_fingerprint_v1) => {
                            hash.update([1]);
                            hash.update(fold_model_fingerprint_v1);
                        }
                        None => hash.update([0]),
                    }
                }
                CertifiedPathTransitionIssuerSealV1::InstructionBoundNative {
                    fixed_face,
                    source_model_binding_sha256,
                } => {
                    hash.update([2]);
                    hash.update(fixed_face.canonical_bytes());
                    hash.update(source_model_binding_sha256);
                }
                #[cfg(feature = "private-petal-e2e")]
                CertifiedPathTransitionIssuerSealV1::UntrustedPrivatePetalE2eFixture => {
                    hash.update([0]);
                }
            }
        }
        hash.update((self.explored_state_count as u64).to_be_bytes());
        hash.update((self.evaluated_transition_count as u64).to_be_bytes());
        hash.finalize().into()
    }

    /// Matches a persisted non-authorizing DTO to this exact native
    /// certificate. Segment endpoint references may retain the whole-path
    /// transition count and binding, so an exact edge endpoint is accepted in
    /// addition to the whole path endpoints.
    #[must_use]
    pub fn matches_path_certificate_reference_v1(
        &self,
        reference: &PathCertificateReferenceV1,
    ) -> bool {
        reference.version == 1
            && reference.model_id == PATH_CERTIFICATE_REFERENCE_MODEL_ID_V1
            && !self.edges.is_empty()
            && self.edges.len() <= MAX_PATH_CERTIFICATE_REFERENCE_TRANSITIONS_V1
            && !self.authorizes_project_mutation()
            && self.binding_fingerprint_v1() == reference.binding_sha256
            && self.edges.len() == reference.transition_count
            && ((self.source == reference.source_pose_sha256
                && self.target == reference.target_pose_sha256)
                || self.edges.iter().any(|edge| {
                    edge.source() == reference.source_pose_sha256
                        && edge.target() == reference.target_pose_sha256
                }))
    }
}

/// Adapts an exactly revalidated common-articulation path authority into one
/// native pose-graph transition for instruction persistence.
///
/// The retained authority is revalidated against every live premise before an
/// edge is issued. The schedule endpoints must also match the current closed
/// pose and requested target bit-for-bit. The resulting graph certificate is
/// observation-only and never authorizes project mutation.
#[must_use]
pub fn issue_common_articulation_single_transition_path_v1(
    authority: &CommonArticulationContinuousLayerPathAuthorityV1,
    input: CommonArticulationContinuousLayerPathRevalidationInputV1<'_>,
) -> Option<CertifiedPoseGraphPathCertificateV1> {
    let source_angles = input.schedule.evaluate(0.0)?;
    let target_angles = input.schedule.evaluate(1.0)?;
    if !bit_exact_canonical_angles_match_v1(&source_angles, input.pose.hinge_angles())
        || !bit_exact_target_angles_match_v1(&target_angles, input.target_angles)
        || authority.revalidate_v1(input).is_err()
    {
        return None;
    }

    let source = canonical_pose_fingerprint_v1(&source_angles);
    let target = canonical_pose_fingerprint_v1(&target_angles);
    if source == target {
        return None;
    }
    let schedule_certificate = input.schedule.certificate_binding_fingerprint_v2();
    let authority_certificate = authority.binding_fingerprint_v1();
    let partition_certificate = input.closure.partition_binding_fingerprint_v2();
    let closure_certificate = hash_certificate_binding(
        b"common_articulation_single_transition_closure_certificate_v1",
        &[
            &schedule_certificate,
            &partition_certificate,
            &authority_certificate,
        ],
    );
    let evidence = CertifiedPathTransitionEvidenceV1::from_native_oracle(
        source,
        target,
        schedule_certificate,
        authority_certificate,
        closure_certificate,
        Some(input.pose.fixed_face()),
        input.geometry.fold_model_fingerprint_v1(),
    );
    let candidate = CertifiedPathTransitionCandidateV1 {
        source,
        target,
        candidate_key: authority_certificate,
    };
    match search_certified_pose_graph_v1(&[source, target], &[candidate], source, target, |_| {
        Some(evidence)
    }) {
        CertifiedPathGraphSearchResultV1::Certified(path) => Some(path),
        CertifiedPathGraphSearchResultV1::Indeterminate { .. } => None,
    }
}

fn bit_exact_canonical_angles_match_v1(
    expected: &CanonicalHingeAngles,
    actual: &CanonicalHingeAngles,
) -> bool {
    expected.as_slice().len() == actual.as_slice().len()
        && expected
            .as_slice()
            .iter()
            .zip(actual.as_slice())
            .all(|(expected, actual)| {
                expected.edge() == actual.edge()
                    && expected.angle_degrees().to_bits() == actual.angle_degrees().to_bits()
            })
}

fn bit_exact_pose_angles_match_v1(
    expected: &CanonicalHingeAngles,
    actual: &MaterialTreePose,
) -> bool {
    expected.as_slice().len() == actual.hinge_angles().len()
        && expected
            .as_slice()
            .iter()
            .zip(actual.hinge_angles())
            .all(|(expected, actual)| {
                expected.edge() == actual.edge()
                    && expected.angle_degrees().to_bits() == actual.angle_degrees().to_bits()
            })
}

fn bit_exact_target_angles_match_v1(
    expected: &CanonicalHingeAngles,
    actual: &[(ori_domain::EdgeId, f64)],
) -> bool {
    if expected.as_slice().len() != actual.len() {
        return false;
    }
    let mut canonical = Vec::new();
    if canonical.try_reserve_exact(actual.len()).is_err() {
        return false;
    }
    canonical.extend_from_slice(actual);
    canonical.sort_unstable_by_key(|(edge, _)| edge.canonical_bytes());
    canonical
        .iter()
        .zip(expected.as_slice())
        .all(|((edge, angle), expected)| {
            *edge == expected.edge() && angle.to_bits() == expected.angle_degrees().to_bits()
        })
}

fn canonical_pose_fingerprint_v1(angles: &CanonicalHingeAngles) -> PoseFingerprintV1 {
    let mut hash = Sha256::new();
    hash.update(b"stacked_fold_certified_path_graph_state_v1");
    hash.update((angles.as_slice().len() as u64).to_be_bytes());
    for angle in angles.as_slice() {
        hash.update(angle.edge().canonical_bytes());
        hash.update(angle.angle_degrees().to_bits().to_be_bytes());
    }
    hash.finalize().into()
}

/// Rebinds one already native-issued graph transition to the exact persisted
/// instruction-pose fingerprint domain. The physical endpoints must match the
/// native graph certificate bit-for-bit, so this adapter cannot manufacture a
/// transition for unrelated angles, model text, or a different fixed face.
#[must_use]
pub fn issue_instruction_bound_single_transition_path_v1(
    native: &CertifiedPoseGraphPathCertificateV1,
    source_model_fingerprint: &str,
    fixed_face: FaceId,
    source_angles: &CanonicalHingeAngles,
    target_angles: &CanonicalHingeAngles,
) -> Option<CertifiedPoseGraphPathCertificateV1> {
    issue_instruction_bound_path_v1(
        native,
        source_model_fingerprint,
        fixed_face,
        &[source_angles, target_angles],
    )
}

/// Rebinds every edge of one raw native graph path to an exact ordered series
/// of persisted instruction poses. The ordered state count must be exactly one
/// greater than the edge count, and every raw graph endpoint is rechecked
/// before an instruction-domain edge is issued.
#[must_use]
pub fn issue_instruction_bound_path_v1(
    native: &CertifiedPoseGraphPathCertificateV1,
    source_model_fingerprint: &str,
    fixed_face: FaceId,
    ordered_states: &[&CanonicalHingeAngles],
) -> Option<CertifiedPoseGraphPathCertificateV1> {
    if source_model_fingerprint.len() != 64
        || !source_model_fingerprint
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return None;
    }
    let fold_model_fingerprint_v1 = decode_lower_hex_32_v1(source_model_fingerprint)?;
    if !native.is_native_attestable_v1()
        || native.edges.len().checked_add(1)? != ordered_states.len()
        || native.edges.iter().any(|edge| {
            !matches!(
                edge.issuer_seal,
                CertifiedPathTransitionIssuerSealV1::NativeGraph {
                    fixed_face: Some(edge_fixed_face),
                    fold_model_fingerprint_v1: Some(edge_fold_model),
                } if edge_fixed_face == fixed_face
                    && edge_fold_model == fold_model_fingerprint_v1
            )
        })
    {
        return None;
    }
    let native_binding = native.binding_fingerprint_v1();
    let mut source_model_binding = Sha256::new();
    source_model_binding.update(b"path_certificate_source_model_binding_v1");
    source_model_binding.update(source_model_fingerprint.as_bytes());
    let source_model_binding_sha256 = source_model_binding.finalize().into();
    let mut edges = Vec::new();
    edges.try_reserve_exact(native.edges.len()).ok()?;
    for (index, (native_edge, state_pair)) in native
        .edges
        .iter()
        .copied()
        .zip(ordered_states.windows(2))
        .enumerate()
    {
        let [source_angles, target_angles] = state_pair else {
            return None;
        };
        if native_edge.source() != canonical_pose_fingerprint_v1(source_angles)
            || native_edge.target() != canonical_pose_fingerprint_v1(target_angles)
        {
            return None;
        }
        let source =
            instruction_pose_fingerprint_v1(source_model_fingerprint, fixed_face, source_angles);
        let target =
            instruction_pose_fingerprint_v1(source_model_fingerprint, fixed_face, target_angles);
        if source == target {
            return None;
        }
        let closure_certificate = hash_certificate_binding(
            b"instruction_bound_native_transition_v2",
            &[
                &native_binding,
                &(index as u64).to_be_bytes(),
                source_model_fingerprint.as_bytes(),
                &fixed_face.canonical_bytes(),
            ],
        );
        edges.push(
            CertifiedPathTransitionEvidenceV1::from_instruction_bound_native_oracle(
                source,
                target,
                native_edge.schedule_certificate,
                native_edge.collision_certificate,
                closure_certificate,
                fixed_face,
                source_model_binding_sha256,
            ),
        );
    }
    let source = edges.first()?.source();
    let target = edges.last()?.target();
    Some(CertifiedPoseGraphPathCertificateV1 {
        source,
        target,
        edges,
        explored_state_count: native.explored_state_count,
        evaluated_transition_count: native.evaluated_transition_count,
    })
}

fn decode_lower_hex_32_v1(value: &str) -> Option<[u8; 32]> {
    if value.len() != 64 {
        return None;
    }
    let mut decoded = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        let nibble = |byte| match byte {
            b'0'..=b'9' => Some(byte - b'0'),
            b'a'..=b'f' => Some(byte - b'a' + 10),
            _ => None,
        };
        decoded[index] = (nibble(pair[0])? << 4) | nibble(pair[1])?;
    }
    Some(decoded)
}

fn instruction_pose_fingerprint_v1(
    source_model_fingerprint: &str,
    fixed_face: FaceId,
    angles: &CanonicalHingeAngles,
) -> PoseFingerprintV1 {
    let mut hash = Sha256::new();
    hash.update(b"origami2_instruction_pose_fingerprint_v1");
    hash.update(source_model_fingerprint.as_bytes());
    hash.update(fixed_face.canonical_bytes());
    for angle in angles.as_slice() {
        hash.update(angle.edge().canonical_bytes());
        hash.update(angle.angle_degrees().to_bits().to_be_bytes());
    }
    hash.finalize().into()
}

/// Native issuer for a private three-stage petal transaction. Each input must
/// already be a native-issued single-transition certificate; this only joins
/// an exact contiguous chain and cannot manufacture transition evidence.
pub fn issue_private_three_segment_path_v1(
    segments: [CertifiedPoseGraphPathCertificateV1; 3],
) -> Option<CertifiedPoseGraphPathCertificateV1> {
    let fixed_face = segments[0].native_fixed_face_v1()?;
    let issuer_seal = segments[0].edges.first()?.issuer_seal;
    if segments.iter().any(|segment| {
        segment.edges.len() != 1
            || !segment.is_native_attestable_v1()
            || segment.native_fixed_face_v1() != Some(fixed_face)
            || segment
                .edges
                .first()
                .is_none_or(|edge| edge.issuer_seal != issuer_seal)
    }) || segments[0].target != segments[1].source
        || segments[1].target != segments[2].source
    {
        return None;
    }
    let explored_state_count = segments.iter().try_fold(0usize, |sum, segment| {
        sum.checked_add(segment.explored_state_count)
    })?;
    let evaluated_transition_count = segments.iter().try_fold(0usize, |sum, segment| {
        sum.checked_add(segment.evaluated_transition_count)
    })?;
    let source = segments[0].source;
    let target = segments[2].target;
    let mut edges = Vec::new();
    edges.try_reserve_exact(3).ok()?;
    for segment in segments {
        edges.push(segment.edges.into_iter().next()?);
    }
    Some(CertifiedPoseGraphPathCertificateV1 {
        source,
        target,
        edges,
        explored_state_count,
        evaluated_transition_count,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CertifiedPathGraphIndeterminateReasonV1 {
    ResourceLimit,
    NoCertifiedPath,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CertifiedPathGraphSearchResultV1 {
    Certified(CertifiedPoseGraphPathCertificateV1),
    Indeterminate {
        reason: CertifiedPathGraphIndeterminateReasonV1,
        explored_state_count: usize,
        evaluated_transition_count: usize,
    },
}

/// Detached observation emitted while a bounded search is running.
///
/// Progress is never certificate evidence and never authorizes mutation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CertifiedPathGraphProgressV1 {
    pub explored_state_count: usize,
    pub evaluated_transition_count: usize,
    pub state_limit: usize,
    pub transition_limit: usize,
}

/// Runs canonical breadth-first search within
/// [`MAX_CERTIFIED_PATH_GRAPH_STATES_V1`] states and
/// [`MAX_CERTIFIED_PATH_GRAPH_CANDIDATES_V1`] candidate transitions. The
/// oracle is called once per reachable canonical candidate; only exact
/// source/target-bound evidence is admitted.
pub fn search_certified_pose_graph_v1(
    states: &[PoseFingerprintV1],
    transitions: &[CertifiedPathTransitionCandidateV1],
    source: PoseFingerprintV1,
    target: PoseFingerprintV1,
    oracle: impl FnMut(&CertifiedPathTransitionCandidateV1) -> Option<CertifiedPathTransitionEvidenceV1>,
) -> CertifiedPathGraphSearchResultV1 {
    search_certified_pose_graph_with_progress_v1(
        states,
        transitions,
        source,
        target,
        || true,
        |_| {},
        oracle,
    )
}

/// Cancellable form of [`search_certified_pose_graph_v1`]. The checkpoint is
/// observed before every state, every transition oracle call and certificate
/// publication. Cancellation never publishes a partial path.
pub fn search_certified_pose_graph_with_checkpoint_v1(
    states: &[PoseFingerprintV1],
    transitions: &[CertifiedPathTransitionCandidateV1],
    source: PoseFingerprintV1,
    target: PoseFingerprintV1,
    checkpoint: impl FnMut() -> bool,
    oracle: impl FnMut(&CertifiedPathTransitionCandidateV1) -> Option<CertifiedPathTransitionEvidenceV1>,
) -> CertifiedPathGraphSearchResultV1 {
    search_certified_pose_graph_with_progress_v1(
        states,
        transitions,
        source,
        target,
        checkpoint,
        |_| {},
        oracle,
    )
}

/// Cancellable search with bounded monotonic progress observations. Progress
/// is detached from the eventual certificate and may be discarded at any time.
pub fn search_certified_pose_graph_with_progress_v1(
    states: &[PoseFingerprintV1],
    transitions: &[CertifiedPathTransitionCandidateV1],
    source: PoseFingerprintV1,
    target: PoseFingerprintV1,
    mut checkpoint: impl FnMut() -> bool,
    mut progress: impl FnMut(CertifiedPathGraphProgressV1),
    mut oracle: impl FnMut(
        &CertifiedPathTransitionCandidateV1,
    ) -> Option<CertifiedPathTransitionEvidenceV1>,
) -> CertifiedPathGraphSearchResultV1 {
    let publish_progress = |progress: &mut dyn FnMut(CertifiedPathGraphProgressV1),
                            explored_state_count,
                            evaluated_transition_count| {
        progress(CertifiedPathGraphProgressV1 {
            explored_state_count,
            evaluated_transition_count,
            state_limit: MAX_CERTIFIED_PATH_GRAPH_STATES_V1,
            transition_limit: MAX_CERTIFIED_PATH_GRAPH_CANDIDATES_V1,
        });
    };
    publish_progress(&mut progress, 0, 0);
    if !checkpoint() {
        return indeterminate(CertifiedPathGraphIndeterminateReasonV1::Cancelled, 0, 0);
    }
    if states.is_empty()
        || states.len() > MAX_CERTIFIED_PATH_GRAPH_STATES_V1
        || transitions.len() > MAX_CERTIFIED_PATH_GRAPH_CANDIDATES_V1
    {
        return indeterminate(CertifiedPathGraphIndeterminateReasonV1::ResourceLimit, 0, 0);
    }
    let mut canonical_states = Vec::new();
    if canonical_states.try_reserve_exact(states.len()).is_err() {
        return indeterminate(CertifiedPathGraphIndeterminateReasonV1::ResourceLimit, 0, 0);
    }
    canonical_states.extend_from_slice(states);
    canonical_states.sort_unstable();
    canonical_states.dedup();
    if canonical_states.len() != states.len()
        || canonical_states.binary_search(&source).is_err()
        || canonical_states.binary_search(&target).is_err()
    {
        return indeterminate(
            CertifiedPathGraphIndeterminateReasonV1::NoCertifiedPath,
            0,
            0,
        );
    }
    if source == target {
        return CertifiedPathGraphSearchResultV1::Certified(CertifiedPoseGraphPathCertificateV1 {
            source,
            target,
            edges: Vec::new(),
            explored_state_count: 1,
            evaluated_transition_count: 0,
        });
    }

    let mut canonical_transitions = Vec::new();
    if canonical_transitions
        .try_reserve_exact(transitions.len())
        .is_err()
    {
        return indeterminate(CertifiedPathGraphIndeterminateReasonV1::ResourceLimit, 0, 0);
    }
    canonical_transitions.extend_from_slice(transitions);
    canonical_transitions
        .sort_unstable_by_key(|edge| (edge.source, edge.target, edge.candidate_key));
    canonical_transitions.dedup();
    let Ok(source_index) = canonical_states.binary_search(&source) else {
        return indeterminate(
            CertifiedPathGraphIndeterminateReasonV1::NoCertifiedPath,
            0,
            0,
        );
    };
    let Ok(target_index) = canonical_states.binary_search(&target) else {
        return indeterminate(
            CertifiedPathGraphIndeterminateReasonV1::NoCertifiedPath,
            0,
            0,
        );
    };
    let mut queue = VecDeque::new();
    if queue.try_reserve_exact(canonical_states.len()).is_err() {
        return indeterminate(CertifiedPathGraphIndeterminateReasonV1::ResourceLimit, 0, 0);
    }
    queue.push_back(source_index);
    let mut parents = Vec::new();
    if parents.try_reserve_exact(canonical_states.len()).is_err() {
        return indeterminate(CertifiedPathGraphIndeterminateReasonV1::ResourceLimit, 0, 0);
    }
    parents.resize(canonical_states.len(), None);
    let mut visited = Vec::new();
    if visited.try_reserve_exact(canonical_states.len()).is_err() {
        return indeterminate(CertifiedPathGraphIndeterminateReasonV1::ResourceLimit, 0, 0);
    }
    visited.resize(canonical_states.len(), false);
    visited[source_index] = true;
    let mut evaluated = 0usize;
    let mut explored = 0usize;

    while let Some(current_index) = queue.pop_front() {
        if !checkpoint() {
            return indeterminate(
                CertifiedPathGraphIndeterminateReasonV1::Cancelled,
                explored,
                evaluated,
            );
        }
        explored += 1;
        publish_progress(&mut progress, explored, evaluated);
        let current = canonical_states[current_index];
        let range = canonical_transition_range_v1(&canonical_transitions, current);
        for candidate in &canonical_transitions[range] {
            if !checkpoint() {
                return indeterminate(
                    CertifiedPathGraphIndeterminateReasonV1::Cancelled,
                    explored,
                    evaluated,
                );
            }
            let Ok(candidate_target_index) = canonical_states.binary_search(&candidate.target)
            else {
                continue;
            };
            evaluated += 1;
            publish_progress(&mut progress, explored, evaluated);
            let Some(evidence) = oracle(candidate) else {
                continue;
            };
            if evidence.source != candidate.source || evidence.target != candidate.target {
                continue;
            }
            if visited[candidate_target_index] {
                continue;
            }
            visited[candidate_target_index] = true;
            parents[candidate_target_index] = Some((current_index, evidence));
            if candidate_target_index == target_index {
                let mut edges = Vec::new();
                if edges.try_reserve_exact(canonical_states.len()).is_err() {
                    return indeterminate(
                        CertifiedPathGraphIndeterminateReasonV1::ResourceLimit,
                        explored,
                        evaluated,
                    );
                }
                let mut cursor_index = target_index;
                while cursor_index != source_index {
                    let Some((parent_index, edge)) = parents[cursor_index] else {
                        return indeterminate(
                            CertifiedPathGraphIndeterminateReasonV1::NoCertifiedPath,
                            explored,
                            evaluated,
                        );
                    };
                    edges.push(edge);
                    cursor_index = parent_index;
                }
                edges.reverse();
                if !checkpoint() {
                    return indeterminate(
                        CertifiedPathGraphIndeterminateReasonV1::Cancelled,
                        explored,
                        evaluated,
                    );
                }
                return CertifiedPathGraphSearchResultV1::Certified(
                    CertifiedPoseGraphPathCertificateV1 {
                        source,
                        target,
                        edges,
                        explored_state_count: explored,
                        evaluated_transition_count: evaluated,
                    },
                );
            }
            queue.push_back(candidate_target_index);
        }
    }
    indeterminate(
        CertifiedPathGraphIndeterminateReasonV1::NoCertifiedPath,
        explored,
        evaluated,
    )
}

fn canonical_transition_range_v1(
    transitions: &[CertifiedPathTransitionCandidateV1],
    source: PoseFingerprintV1,
) -> std::ops::Range<usize> {
    let start = transitions.partition_point(|transition| transition.source < source);
    let end = transitions.partition_point(|transition| transition.source <= source);
    start..end
}

#[cfg(test)]
fn canonical_transition_adjacency_v1(
    transitions: &[CertifiedPathTransitionCandidateV1],
) -> std::collections::BTreeMap<PoseFingerprintV1, std::ops::Range<usize>> {
    let mut adjacency = std::collections::BTreeMap::new();
    let mut start = 0;
    while start < transitions.len() {
        let source = transitions[start].source;
        let range = canonical_transition_range_v1(transitions, source);
        start = range.end;
        adjacency.insert(source, range);
    }
    adjacency
}

fn indeterminate(
    reason: CertifiedPathGraphIndeterminateReasonV1,
    explored_state_count: usize,
    evaluated_transition_count: usize,
) -> CertifiedPathGraphSearchResultV1 {
    CertifiedPathGraphSearchResultV1::Indeterminate {
        reason,
        explored_state_count,
        evaluated_transition_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ori_domain::EdgeId;
    use ori_kinematics::{
        CanonicalHingeAngles, DyadicPoseGraphLimitsV1, HingeAngle,
        generate_bounded_dyadic_pose_graph_at_levels_v1, generate_bounded_dyadic_pose_graph_v1,
    };

    fn fingerprint(value: u8) -> PoseFingerprintV1 {
        [value; 32]
    }

    fn large_fingerprint(value: usize) -> PoseFingerprintV1 {
        let mut fingerprint = [0; 32];
        fingerprint[24..].copy_from_slice(&(value as u64).to_be_bytes());
        fingerprint
    }
    fn candidate(source: u8, target: u8, key: u8) -> CertifiedPathTransitionCandidateV1 {
        CertifiedPathTransitionCandidateV1 {
            source: fingerprint(source),
            target: fingerprint(target),
            candidate_key: fingerprint(key),
        }
    }
    fn certify(
        candidate: &CertifiedPathTransitionCandidateV1,
    ) -> CertifiedPathTransitionEvidenceV1 {
        CertifiedPathTransitionEvidenceV1::from_native_oracle(
            candidate.source,
            candidate.target,
            fingerprint(10),
            fingerprint(11),
            fingerprint(12),
            None,
            None,
        )
    }

    #[test]
    fn common_articulation_adapter_endpoint_matching_is_canonical_and_bit_exact() {
        let mut edges = [EdgeId::new(), EdgeId::new()];
        edges.sort_unstable_by_key(EdgeId::canonical_bytes);
        let expected = CanonicalHingeAngles::new(vec![
            HingeAngle::new(edges[0], 0.0).unwrap(),
            HingeAngle::new(edges[1], 45.0).unwrap(),
        ])
        .unwrap();
        let reversed = vec![(edges[1], 45.0), (edges[0], 0.0)];
        assert!(bit_exact_target_angles_match_v1(&expected, &reversed));
        assert!(bit_exact_canonical_angles_match_v1(&expected, &expected));

        let mut one_bit = reversed.clone();
        one_bit[0].1 = f64::from_bits(one_bit[0].1.to_bits() ^ 1);
        assert!(!bit_exact_target_angles_match_v1(&expected, &one_bit));
        assert!(!bit_exact_target_angles_match_v1(
            &expected,
            &[(edges[0], 0.0), (edges[0], 45.0)],
        ));

        let drifted = CanonicalHingeAngles::new(vec![
            HingeAngle::new(edges[0], f64::from_bits(1)).unwrap(),
            HingeAngle::new(edges[1], 45.0).unwrap(),
        ])
        .unwrap();
        assert!(!bit_exact_canonical_angles_match_v1(&expected, &drifted));
        assert_ne!(
            canonical_pose_fingerprint_v1(&expected),
            canonical_pose_fingerprint_v1(&drifted),
        );
    }

    #[test]
    fn instruction_adapter_requires_geometry_model_binding_for_every_multi_edge_state_v1() {
        let fixed_face = FaceId::new();
        let edge = EdgeId::new();
        let state = |angle_degrees| {
            CanonicalHingeAngles::new(vec![
                HingeAngle::new(edge, angle_degrees).expect("finite test angle"),
            ])
            .expect("canonical test state")
        };
        let source_state = state(5.0);
        let middle_state = state(25.0);
        let target_state = state(45.0);
        let graph_states = [
            canonical_pose_fingerprint_v1(&source_state),
            canonical_pose_fingerprint_v1(&middle_state),
            canonical_pose_fingerprint_v1(&target_state),
        ];
        let fold_model_fingerprint_v1 = [0x77; 32];
        let edge_evidence = |index: usize| {
            CertifiedPathTransitionEvidenceV1::from_native_oracle(
                graph_states[index],
                graph_states[index + 1],
                fingerprint(10 + index as u8),
                fingerprint(20 + index as u8),
                fingerprint(30 + index as u8),
                Some(fixed_face),
                Some(fold_model_fingerprint_v1),
            )
        };
        let native = CertifiedPoseGraphPathCertificateV1 {
            source: graph_states[0],
            target: graph_states[2],
            edges: vec![edge_evidence(0), edge_evidence(1)],
            explored_state_count: 3,
            evaluated_transition_count: 2,
        };
        assert!(native.is_native_attestable_v1());
        assert_eq!(native.native_source_model_binding_v1(), None);
        let model = "77".repeat(32);
        let ordered_states = [&source_state, &middle_state, &target_state];
        let bound = issue_instruction_bound_path_v1(&native, &model, fixed_face, &ordered_states)
            .expect("the geometry-derived model and every exact state agree");
        assert_eq!(bound.edges().len(), 2);
        assert_eq!(
            bound.source(),
            instruction_pose_fingerprint_v1(&model, fixed_face, &source_state)
        );
        assert_eq!(
            bound.target(),
            instruction_pose_fingerprint_v1(&model, fixed_face, &target_state)
        );
        assert!(bound.native_source_model_binding_v1().is_some());

        assert!(
            issue_instruction_bound_path_v1(
                &native,
                &"88".repeat(32),
                fixed_face,
                &ordered_states,
            )
            .is_none(),
            "caller-selected model text cannot relabel native geometry evidence"
        );
        assert!(
            issue_instruction_bound_path_v1(
                &native,
                &model,
                fixed_face,
                &[&source_state, &target_state, &middle_state],
            )
            .is_none(),
            "every multi-edge endpoint must match in exact order"
        );
    }

    #[cfg(feature = "private-petal-e2e")]
    #[test]
    fn private_three_segment_issuer_accepts_only_contiguous_native_single_edges() {
        let fixed_face = FaceId::new();
        let segment_with_model =
            |source: u8,
             target: u8,
             issuer_fixed_face: FaceId,
             fold_model_fingerprint_v1: [u8; 32]| {
                let candidate = candidate(source, target, source);
                CertifiedPoseGraphPathCertificateV1 {
                    source: fingerprint(source),
                    target: fingerprint(target),
                    edges: vec![CertifiedPathTransitionEvidenceV1::from_native_oracle(
                        candidate.source,
                        candidate.target,
                        fingerprint(10),
                        fingerprint(11),
                        fingerprint(12),
                        Some(issuer_fixed_face),
                        Some(fold_model_fingerprint_v1),
                    )],
                    explored_state_count: 2,
                    evaluated_transition_count: 1,
                }
            };
        let segment = |source: u8, target: u8, issuer_fixed_face: FaceId| {
            segment_with_model(source, target, issuer_fixed_face, [0x77; 32])
        };
        let instruction_segment = |source: u8, target: u8| CertifiedPoseGraphPathCertificateV1 {
            source: fingerprint(source),
            target: fingerprint(target),
            edges: vec![
                CertifiedPathTransitionEvidenceV1::from_instruction_bound_native_oracle(
                    fingerprint(source),
                    fingerprint(target),
                    fingerprint(10),
                    fingerprint(11),
                    fingerprint(12),
                    fixed_face,
                    [0x99; 32],
                ),
            ],
            explored_state_count: 2,
            evaluated_transition_count: 1,
        };
        let parent = issue_private_three_segment_path_v1([
            segment(1, 2, fixed_face),
            segment(2, 3, fixed_face),
            segment(3, 4, fixed_face),
        ])
        .unwrap();
        assert_eq!(parent.edges().len(), 3);
        assert_eq!(parent.source(), fingerprint(1));
        assert_eq!(parent.target(), fingerprint(4));
        assert!(
            issue_private_three_segment_path_v1([
                segment(2, 3, fixed_face),
                segment(1, 2, fixed_face),
                segment(3, 4, fixed_face),
            ])
            .is_none()
        );
        assert!(
            issue_private_three_segment_path_v1([
                segment(1, 2, fixed_face),
                segment(2, 4, fixed_face),
                segment(3, 4, fixed_face),
            ])
            .is_none()
        );
        assert!(
            issue_private_three_segment_path_v1([
                segment(1, 2, fixed_face),
                segment(2, 3, FaceId::new()),
                segment(3, 4, fixed_face),
            ])
            .is_none(),
            "mixed issuer fixed faces cannot form one native path"
        );
        assert!(
            issue_private_three_segment_path_v1([
                segment(1, 2, fixed_face),
                segment_with_model(2, 3, fixed_face, [0x88; 32]),
                segment(3, 4, fixed_face),
            ])
            .is_none(),
            "mixed native fold-model bindings cannot form one private path"
        );
        assert!(
            issue_private_three_segment_path_v1([
                segment(1, 2, fixed_face),
                instruction_segment(2, 3),
                segment(3, 4, fixed_face),
            ])
            .is_none(),
            "raw graph and instruction-bound seal kinds cannot be joined"
        );
        let untrusted_edge = private_petal_e2e_transition_fixture_v1(
            fingerprint(2),
            fingerprint(3),
            fingerprint(10),
            fingerprint(11),
            fingerprint(12),
        );
        let untrusted_segment = CertifiedPoseGraphPathCertificateV1 {
            source: fingerprint(2),
            target: fingerprint(3),
            edges: vec![untrusted_edge],
            explored_state_count: 2,
            evaluated_transition_count: 1,
        };
        assert!(
            issue_private_three_segment_path_v1([
                segment(1, 2, fixed_face),
                untrusted_segment,
                segment(3, 4, fixed_face),
            ])
            .is_none(),
            "an untrusted feature fixture cannot enter the private native issuer"
        );
    }

    #[test]
    fn generated_two_hinge_grid_supports_certified_detours_and_fails_closed() {
        let mut edges = [EdgeId::new(), EdgeId::new()];
        edges.sort_unstable_by_key(EdgeId::canonical_bytes);
        let angles = |values: [f64; 2]| {
            CanonicalHingeAngles::new(
                edges
                    .into_iter()
                    .zip(values)
                    .map(|(edge, value)| HingeAngle::new(edge, value).unwrap())
                    .collect(),
            )
            .unwrap()
        };
        let generated = generate_bounded_dyadic_pose_graph_v1(
            &angles([0.0, 0.0]),
            &angles([90.0, 120.0]),
            DyadicPoseGraphLimitsV1::default(),
            || true,
        )
        .unwrap();
        let states = (0..generated.states().len())
            .map(|index| fingerprint(index as u8 + 1))
            .collect::<Vec<_>>();
        let candidates = generated
            .transitions()
            .iter()
            .map(|edge| CertifiedPathTransitionCandidateV1 {
                source: states[edge.source_state],
                target: states[edge.target_state],
                candidate_key: if edge.moving_hinge == edges[0] {
                    fingerprint(1)
                } else {
                    fingerprint(2)
                },
            })
            .collect::<Vec<_>>();
        let source = states[generated.source_state()];
        let target = states[generated.target_state()];
        assert!(matches!(
            search_certified_pose_graph_v1(&states, &candidates, source, target, |candidate| Some(
                certify(candidate)
            )),
            CertifiedPathGraphSearchResultV1::Certified(_)
        ));
        let blocked = candidates[0];
        assert!(matches!(
            search_certified_pose_graph_v1(&states, &candidates, source, target, |candidate| {
                (*candidate != blocked).then(|| certify(candidate))
            }),
            CertifiedPathGraphSearchResultV1::Certified(_)
        ));
        assert!(matches!(
            search_certified_pose_graph_v1(&states, &candidates, source, target, |candidate| {
                (candidate.source != source).then(|| certify(candidate))
            }),
            CertifiedPathGraphSearchResultV1::Indeterminate {
                reason: CertifiedPathGraphIndeterminateReasonV1::NoCertifiedPath,
                ..
            }
        ));
        assert!(matches!(
            search_certified_pose_graph_v1(&states, &candidates, source, target, |candidate| {
                let mut evidence = certify(candidate);
                evidence.source = fingerprint(250);
                Some(evidence)
            }),
            CertifiedPathGraphSearchResultV1::Indeterminate {
                reason: CertifiedPathGraphIndeterminateReasonV1::NoCertifiedPath,
                ..
            }
        ));
        let mut reordered = candidates.clone();
        reordered.reverse();
        let first =
            search_certified_pose_graph_v1(&states, &candidates, source, target, |candidate| {
                Some(certify(candidate))
            });
        let second =
            search_certified_pose_graph_v1(&states, &reordered, source, target, |candidate| {
                Some(certify(candidate))
            });
        let (
            CertifiedPathGraphSearchResultV1::Certified(first),
            CertifiedPathGraphSearchResultV1::Certified(second),
        ) = (first, second)
        else {
            panic!("both canonical searches certify")
        };
        assert_eq!(
            first.binding_fingerprint_v1(),
            second.binding_fingerprint_v1()
        );
    }

    #[test]
    fn canonical_bfs_uses_only_certified_edges_and_binds_all_edge_certificates() {
        let states = [
            fingerprint(3),
            fingerprint(1),
            fingerprint(4),
            fingerprint(2),
        ];
        let transitions = [
            candidate(2, 4, 3),
            candidate(1, 3, 2),
            candidate(3, 4, 2),
            candidate(1, 2, 1),
        ];
        let result = search_certified_pose_graph_v1(
            &states,
            &transitions,
            fingerprint(1),
            fingerprint(4),
            |candidate| Some(certify(candidate)),
        );
        let CertifiedPathGraphSearchResultV1::Certified(certificate) = result else {
            panic!("a certified route must be found");
        };
        assert_eq!(certificate.model_id(), CERTIFIED_PATH_GRAPH_MODEL_ID_V1);
        assert_eq!(certificate.version(), 1);
        assert!(!certificate.authorizes_project_mutation());
        assert_eq!(
            certificate
                .edges()
                .iter()
                .map(|edge| (edge.source(), edge.target()))
                .collect::<Vec<_>>(),
            vec![
                (fingerprint(1), fingerprint(2)),
                (fingerprint(2), fingerprint(4))
            ],
        );
        assert!(certificate.edges().iter().all(|edge| {
            edge.schedule_certificate() == fingerprint(10)
                && edge.collision_certificate() == fingerprint(11)
                && edge.closure_certificate() == fingerprint(12)
        }));
        let binding = certificate.binding_fingerprint_v1();
        assert_ne!(binding, [0; 32]);
        let reference = PathCertificateReferenceV1 {
            version: 1,
            model_id: PATH_CERTIFICATE_REFERENCE_MODEL_ID_V1.to_owned(),
            binding_sha256: binding,
            source_pose_sha256: certificate.edges()[0].source(),
            target_pose_sha256: certificate.edges()[0].target(),
            source_model_binding_sha256: fingerprint(20),
            transition_count: certificate.edges().len(),
        };
        assert!(certificate.matches_path_certificate_reference_v1(&reference));
        let mut drifted = reference.clone();
        drifted.binding_sha256[0] ^= 1;
        assert!(!certificate.matches_path_certificate_reference_v1(&drifted));
        drifted = reference.clone();
        drifted.transition_count -= 1;
        assert!(!certificate.matches_path_certificate_reference_v1(&drifted));
        drifted = reference.clone();
        drifted.source_pose_sha256[0] ^= 1;
        assert!(!certificate.matches_path_certificate_reference_v1(&drifted));
        drifted = reference.clone();
        drifted.version = 2;
        assert!(!certificate.matches_path_certificate_reference_v1(&drifted));

        let mut reversed = transitions;
        reversed.reverse();
        let repeated = search_certified_pose_graph_v1(
            &states,
            &reversed,
            fingerprint(1),
            fingerprint(4),
            |candidate| Some(certify(candidate)),
        );
        assert_eq!(
            repeated,
            CertifiedPathGraphSearchResultV1::Certified(certificate),
            "candidate enumeration order must not change the certificate"
        );
        let CertifiedPathGraphSearchResultV1::Certified(repeated_certificate) = repeated else {
            unreachable!();
        };
        assert_eq!(repeated_certificate.binding_fingerprint_v1(), binding);
    }

    #[test]
    fn uncertified_and_misbound_edges_never_form_a_path() {
        let result = search_certified_pose_graph_v1(
            &[fingerprint(1), fingerprint(2), fingerprint(3)],
            &[candidate(1, 2, 1), candidate(2, 3, 1)],
            fingerprint(1),
            fingerprint(3),
            |candidate| {
                (candidate.source == fingerprint(2)).then(|| {
                    CertifiedPathTransitionEvidenceV1::from_native_oracle(
                        fingerprint(9),
                        candidate.target,
                        fingerprint(10),
                        fingerprint(11),
                        fingerprint(12),
                        None,
                        None,
                    )
                })
            },
        );
        assert!(matches!(
            result,
            CertifiedPathGraphSearchResultV1::Indeterminate {
                reason: CertifiedPathGraphIndeterminateReasonV1::NoCertifiedPath,
                ..
            }
        ));
    }

    #[test]
    fn hard_bounds_return_resource_indeterminate_never_impossible() {
        let states = vec![fingerprint(1); MAX_CERTIFIED_PATH_GRAPH_STATES_V1 + 1];
        assert!(matches!(
            search_certified_pose_graph_v1(&states, &[], fingerprint(1), fingerprint(2), |_| None,),
            CertifiedPathGraphSearchResultV1::Indeterminate {
                reason: CertifiedPathGraphIndeterminateReasonV1::ResourceLimit,
                ..
            }
        ));
        let transitions = vec![candidate(1, 2, 1); MAX_CERTIFIED_PATH_GRAPH_CANDIDATES_V1 + 1];
        assert!(matches!(
            search_certified_pose_graph_v1(
                &[fingerprint(1), fingerprint(2)],
                &transitions,
                fingerprint(1),
                fingerprint(2),
                |_| None,
            ),
            CertifiedPathGraphSearchResultV1::Indeterminate {
                reason: CertifiedPathGraphIndeterminateReasonV1::ResourceLimit,
                ..
            }
        ));
    }

    #[test]
    fn cancellation_is_cooperative_and_never_publishes_a_partial_certificate() {
        let mut checkpoints = 0;
        let mut oracle_calls = 0;
        let result = search_certified_pose_graph_with_checkpoint_v1(
            &[fingerprint(1), fingerprint(2), fingerprint(3)],
            &[candidate(1, 2, 1), candidate(2, 3, 1)],
            fingerprint(1),
            fingerprint(3),
            || {
                checkpoints += 1;
                checkpoints < 5
            },
            |candidate| {
                oracle_calls += 1;
                Some(certify(candidate))
            },
        );
        assert!(matches!(
            result,
            CertifiedPathGraphSearchResultV1::Indeterminate {
                reason: CertifiedPathGraphIndeterminateReasonV1::Cancelled,
                ..
            }
        ));
        assert_eq!(oracle_calls, 1);
    }

    #[test]
    fn progress_is_monotonic_bounded_and_detached_from_the_certificate() {
        let mut observations = Vec::new();
        let result = search_certified_pose_graph_with_progress_v1(
            &[fingerprint(1), fingerprint(2), fingerprint(3)],
            &[candidate(1, 2, 1), candidate(2, 3, 1)],
            fingerprint(1),
            fingerprint(3),
            || true,
            |value| observations.push(value),
            |candidate| Some(certify(candidate)),
        );
        assert!(matches!(
            result,
            CertifiedPathGraphSearchResultV1::Certified(_)
        ));
        assert_eq!(
            observations.first().copied(),
            Some(CertifiedPathGraphProgressV1 {
                explored_state_count: 0,
                evaluated_transition_count: 0,
                state_limit: MAX_CERTIFIED_PATH_GRAPH_STATES_V1,
                transition_limit: MAX_CERTIFIED_PATH_GRAPH_CANDIDATES_V1,
            })
        );
        assert!(observations.windows(2).all(|pair| {
            pair[0].explored_state_count <= pair[1].explored_state_count
                && pair[0].evaluated_transition_count <= pair[1].evaluated_transition_count
        }));
        assert!(observations.iter().all(|value| {
            value.explored_state_count <= value.state_limit
                && value.evaluated_transition_count <= value.transition_limit
                && value.state_limit == MAX_CERTIFIED_PATH_GRAPH_STATES_V1
                && value.transition_limit == MAX_CERTIFIED_PATH_GRAPH_CANDIDATES_V1
        }));
    }

    #[test]
    fn quarter_level_unlocks_a_certified_obstacle_detour() {
        let mut edges = [EdgeId::new(), EdgeId::new()];
        edges.sort_unstable_by_key(EdgeId::canonical_bytes);
        let angles = |values: [f64; 2]| {
            CanonicalHingeAngles::new(
                edges
                    .into_iter()
                    .zip(values)
                    .map(|(edge, value)| HingeAngle::new(edge, value).unwrap())
                    .collect(),
            )
            .unwrap()
        };
        for (levels, expected_certified) in [(3, false), (5, true)] {
            let graph = generate_bounded_dyadic_pose_graph_at_levels_v1(
                &angles([0.0, 0.0]),
                &angles([90.0, 120.0]),
                levels,
                DyadicPoseGraphLimitsV1 {
                    max_states: 25,
                    max_transitions: 80,
                },
                || true,
            )
            .unwrap();
            let states = (0..graph.states().len())
                .map(|index| fingerprint(index as u8 + 1))
                .collect::<Vec<_>>();
            let candidates = graph
                .transitions()
                .iter()
                .map(|edge| CertifiedPathTransitionCandidateV1 {
                    source: states[edge.source_state],
                    target: states[edge.target_state],
                    candidate_key: fingerprint(200),
                })
                .collect::<Vec<_>>();
            let allowed = |fingerprint: PoseFingerprintV1| {
                let index = states.iter().position(|value| *value == fingerprint)?;
                let values = graph.states()[index].as_slice();
                let x = values[0].angle_degrees();
                let y = values[1].angle_degrees();
                Some((y == 0.0 && x <= 22.5) || x == 22.5 || (y == 120.0 && x >= 22.5))
            };
            let searched = search_certified_pose_graph_v1(
                &states,
                &candidates,
                states[graph.source_state()],
                states[graph.target_state()],
                |candidate| {
                    (allowed(candidate.source) == Some(true)
                        && allowed(candidate.target) == Some(true))
                    .then(|| certify(candidate))
                },
            );
            assert_eq!(
                matches!(searched, CertifiedPathGraphSearchResultV1::Certified(_)),
                expected_certified
            );
            if !expected_certified {
                assert!(matches!(
                    searched,
                    CertifiedPathGraphSearchResultV1::Indeterminate {
                        reason: CertifiedPathGraphIndeterminateReasonV1::NoCertifiedPath,
                        ..
                    }
                ));
            }
        }
    }

    #[test]
    fn four_hinge_three_level_graph_is_certified_and_fails_closed() {
        let mut edges = [EdgeId::new(), EdgeId::new(), EdgeId::new(), EdgeId::new()];
        edges.sort_unstable_by_key(EdgeId::canonical_bytes);
        let angles = |values: [f64; 4]| {
            CanonicalHingeAngles::new(
                edges
                    .into_iter()
                    .zip(values)
                    .map(|(edge, value)| HingeAngle::new(edge, value).unwrap())
                    .collect(),
            )
            .unwrap()
        };
        let graph = generate_bounded_dyadic_pose_graph_at_levels_v1(
            &angles([0.0, 0.0, 0.0, 0.0]),
            &angles([30.0, 60.0, 90.0, 120.0]),
            3,
            DyadicPoseGraphLimitsV1 {
                max_states: 81,
                max_transitions: 432,
            },
            || true,
        )
        .unwrap();
        let states = (0..graph.states().len())
            .map(|index| fingerprint(index as u8 + 1))
            .collect::<Vec<_>>();
        let candidates = graph
            .transitions()
            .iter()
            .enumerate()
            .map(|(index, edge)| CertifiedPathTransitionCandidateV1 {
                source: states[edge.source_state],
                target: states[edge.target_state],
                candidate_key: fingerprint((index % 127) as u8 + 100),
            })
            .collect::<Vec<_>>();
        let source = states[graph.source_state()];
        let target = states[graph.target_state()];
        assert!(matches!(
            search_certified_pose_graph_v1(&states, &candidates, source, target, |candidate| {
                Some(certify(candidate))
            }),
            CertifiedPathGraphSearchResultV1::Certified(_)
        ));
        assert!(matches!(
            search_certified_pose_graph_v1(&states, &candidates, source, target, |candidate| {
                (candidate.source != source).then(|| certify(candidate))
            }),
            CertifiedPathGraphSearchResultV1::Indeterminate {
                reason: CertifiedPathGraphIndeterminateReasonV1::NoCertifiedPath,
                ..
            }
        ));
        assert!(matches!(
            search_certified_pose_graph_with_checkpoint_v1(
                &states,
                &candidates,
                source,
                target,
                || false,
                |candidate| Some(certify(candidate)),
            ),
            CertifiedPathGraphSearchResultV1::Indeterminate {
                reason: CertifiedPathGraphIndeterminateReasonV1::Cancelled,
                ..
            }
        ));
    }

    #[test]
    fn seven_hinge_adjacency_visits_each_canonical_edge_at_most_once() {
        let mut edges = [
            EdgeId::new(),
            EdgeId::new(),
            EdgeId::new(),
            EdgeId::new(),
            EdgeId::new(),
            EdgeId::new(),
            EdgeId::new(),
        ];
        edges.sort_unstable_by_key(EdgeId::canonical_bytes);
        let angles = |value: f64| {
            CanonicalHingeAngles::new(
                edges
                    .into_iter()
                    .map(|edge| HingeAngle::new(edge, value).unwrap())
                    .collect(),
            )
            .unwrap()
        };
        let graph = generate_bounded_dyadic_pose_graph_at_levels_v1(
            &angles(0.0),
            &angles(1.0),
            3,
            DyadicPoseGraphLimitsV1 {
                max_states: 2_187,
                max_transitions: 20_412,
            },
            || true,
        )
        .unwrap();
        let states = (0..graph.states().len())
            .map(large_fingerprint)
            .collect::<Vec<_>>();
        let mut candidates = graph
            .transitions()
            .iter()
            .enumerate()
            .map(|(index, edge)| CertifiedPathTransitionCandidateV1 {
                source: states[edge.source_state],
                target: states[edge.target_state],
                candidate_key: large_fingerprint(index + states.len()),
            })
            .collect::<Vec<_>>();
        candidates.sort_unstable_by_key(|edge| (edge.source, edge.target, edge.candidate_key));
        candidates.dedup();
        let adjacency = canonical_transition_adjacency_v1(&candidates);
        assert_eq!(
            adjacency.values().map(|range| range.len()).sum::<usize>(),
            20_412
        );
        assert!(adjacency.len() <= 2_187);
        assert!(adjacency.iter().all(|(source, range)| {
            candidates[range.clone()]
                .iter()
                .all(|edge| edge.source == *source)
        }));
        let mut evaluated = 0usize;
        let searched = search_certified_pose_graph_v1(
            &states,
            &candidates,
            states[graph.source_state()],
            states[graph.target_state()],
            |candidate| {
                evaluated += 1;
                Some(certify(candidate))
            },
        );
        assert!(matches!(
            searched,
            CertifiedPathGraphSearchResultV1::Certified(_)
        ));
        assert!(evaluated <= candidates.len());
    }
}
