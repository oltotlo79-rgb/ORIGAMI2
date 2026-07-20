//! Current-project wrapper for native static-collision geometry evidence.
//!
//! This is the deliberately limited C boundary. It can currently certify only
//! the complete zero-pair case supported by `ori-collision`; it grants neither
//! project-mutation nor SIM-010 authority.

use std::{
    error::Error,
    fmt,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    sync::Arc,
};

use ori_collision::{
    CENTERED_MID_SURFACE_THICKNESS_MODEL_V1, NATIVE_STATIC_COLLISION_GEOMETRY_PROOF_V1,
    NativeStaticCollisionGeometryProof, StaticCollisionDiagnosticSnapshot, StaticCollisionError,
    StaticCollisionLimits, StaticCollisionPairDiagnostic, StaticCollisionPairDisposition,
    TOPOLOGY_CONTACT_POLICY_V2, diagnose_static_collision_geometry,
    prove_static_collision_geometry,
};
use ori_domain::{FaceId, ProjectId};
use ori_kinematics::{
    MATERIAL_TREE_KINEMATICS_MODEL_ID, MaterialTreeKinematicsModel, MaterialTreePose,
};
use serde::Serialize;

use super::{
    CurrentAppliedPoseBindingResponse, CurrentAppliedPoseCapability,
    current_applied_pose_capability_matches_locked_slot,
    current_applied_pose_certificate_is_internally_consistent, current_applied_pose_claims_match,
};
use crate::{AppState, ProjectState, lock_project};

/// Fixed-category failure at the current static-collision boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CurrentStaticCollisionError {
    LockUnavailable,
    PoseAuthorityUnavailable,
    GeometryBlocking(StaticCollisionError),
    InternalInconsistency,
}

impl fmt::Display for CurrentStaticCollisionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::LockUnavailable => "the current static-collision authority is unavailable",
            Self::PoseAuthorityUnavailable => "a current native applied pose is unavailable",
            Self::GeometryBlocking(_) => {
                "native static-collision geometry did not produce a complete safe proof"
            }
            Self::InternalInconsistency => {
                "the current static-collision certificate failed an internal consistency check"
            }
        })
    }
}

impl Error for CurrentStaticCollisionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::GeometryBlocking(error) => Some(error),
            _ => None,
        }
    }
}

/// Read-only production result. Only a successfully minted, still-current C
/// certificate may produce `CertifiedNonblocking`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum CurrentStaticCollisionDiagnosticStatus {
    CertifiedNonblocking,
    Blocking,
    Unavailable,
}

/// Stable, redacted reason categories for IPC. Internal error text and exact
/// arithmetic evidence never cross the command boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum CurrentStaticCollisionDiagnosticReason {
    #[serde(rename = "proven_zero_thickness_penetration")]
    ProvenTransversalPenetration,
    ProvenPositiveThicknessPenetration,
    EvidenceUnavailable,
    ResourceLimitExceeded,
    InconsistentState,
    PoseAuthorityUnavailable,
}

/// Canonically ordered identity of the first proven penetrating face pair.
///
/// For positive paper thickness this is either an issuer-bound exact-E and
/// direct-lift-F mid-surface transversal or a complete connected solid
/// classifier's positive-volume overlap. For zero thickness it may also be a
/// coplanar positive-area overlap. Pair diagnostics retain the route. It is
/// never point/line/shared-point contact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CurrentStaticCollisionFacePair {
    first_face_id: FaceId,
    second_face_id: FaceId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CurrentStaticCollisionPairClassificationCounts {
    separated: usize,
    touching: usize,
    allowed: usize,
    penetrating: usize,
    indeterminate: usize,
    candidate_excluded: usize,
}

impl From<&StaticCollisionDiagnosticSnapshot> for CurrentStaticCollisionPairClassificationCounts {
    fn from(snapshot: &StaticCollisionDiagnosticSnapshot) -> Self {
        Self {
            separated: snapshot.separated_pairs(),
            touching: snapshot.touching_pairs(),
            allowed: snapshot.allowed_pairs(),
            penetrating: snapshot.penetrating_pairs(),
            indeterminate: snapshot.indeterminate_pairs(),
            candidate_excluded: snapshot.candidate_excluded_pairs(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CurrentStaticCollisionPairDiagnostic {
    first_face_id: FaceId,
    second_face_id: FaceId,
    topology: &'static str,
    evidence: &'static str,
    policy_decision: &'static str,
    disposition: &'static str,
    strict_transversal_dual_gate_proven: bool,
    whole_face_overlap_proven: bool,
    shared_hinge_boundary_contact_proven: bool,
    shared_hinge_solid_classified: bool,
}

impl From<&StaticCollisionPairDiagnostic> for CurrentStaticCollisionPairDiagnostic {
    fn from(pair: &StaticCollisionPairDiagnostic) -> Self {
        Self {
            first_face_id: pair.first_face(),
            second_face_id: pair.second_face(),
            topology: pair.topology().identifier(),
            evidence: pair.evidence().identifier(),
            policy_decision: pair.policy_decision().identifier(),
            disposition: pair.disposition().identifier(),
            strict_transversal_dual_gate_proven: pair.strict_transversal_dual_gate_proven(),
            whole_face_overlap_proven: pair.whole_face_overlap_proven(),
            shared_hinge_boundary_contact_proven: pair.shared_hinge_boundary_contact_proven(),
            shared_hinge_solid_classified: pair.shared_hinge_solid_classified(),
        }
    }
}

/// Sanitized static-collision diagnosis for the current native pose.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CurrentStaticCollisionDiagnosticResponse {
    binding: Option<CurrentAppliedPoseBindingResponse>,
    status: CurrentStaticCollisionDiagnosticStatus,
    reason: Option<CurrentStaticCollisionDiagnosticReason>,
    expected_unordered_face_pairs: Option<usize>,
    #[serde(rename = "provenPenetratingPairs")]
    proven_transversal_pairs: Option<usize>,
    #[serde(rename = "firstProvenPenetratingPair")]
    first_proven_transversal_pair: Option<CurrentStaticCollisionFacePair>,
    pair_classification_counts: Option<CurrentStaticCollisionPairClassificationCounts>,
    pair_diagnostics: Option<Vec<CurrentStaticCollisionPairDiagnostic>>,
}

const CURRENT_STATIC_COLLISION_DIAGNOSTIC_FAILED_MESSAGE: &str =
    "現在の衝突判定を完了できませんでした。もう一度実行してください。";

struct CurrentStaticCollisionClaims {
    project_instance_id: ProjectId,
    project_id: ProjectId,
    revision: u64,
    fold_model_fingerprint: Arc<str>,
    pose_generation: u64,
    paper_thickness_bits: u64,
    policy_id: &'static str,
    kinematics_model_id: &'static str,
    thickness_model_id: &'static str,
    proof_id: &'static str,
    proof_identity: NativeStaticCollisionGeometryProof,
}

/// Lock-free worker output. Possession does not prove that B is still current.
struct PreparedCurrentStaticCollision {
    pose_capability: CurrentAppliedPoseCapability,
    geometry_proof: NativeStaticCollisionGeometryProof,
    claims: CurrentStaticCollisionClaims,
}

/// A blocking worker failure retains the exact, non-clonable B capability so
/// the diagnostic command can revalidate that same authority before attaching
/// its binding to a response.
struct FailedCurrentStaticCollisionPreparation {
    pose_capability: CurrentAppliedPoseCapability,
    error: CurrentStaticCollisionError,
    diagnostic_snapshot: Option<StaticCollisionDiagnosticSnapshot>,
}

impl fmt::Debug for FailedCurrentStaticCollisionPreparation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FailedCurrentStaticCollisionPreparation")
            .field("error", &self.error)
            .field("diagnostic_snapshot", &self.diagnostic_snapshot)
            .finish_non_exhaustive()
    }
}

struct CurrentStaticCollisionCertificateData {
    /// Moving B into C prevents a caller from reconstructing C from IDs.
    pose_capability: CurrentAppliedPoseCapability,
    geometry_proof: NativeStaticCollisionGeometryProof,
    claims: CurrentStaticCollisionClaims,
}

/// Opaque in-process evidence that one exact B pose completed native static
/// collision analysis and was still current when C was minted.
///
/// Cloning this wrapper preserves the private certificate identity. A
/// separate proof/mint cycle deliberately creates another identity. This type
/// is intentionally not serializable and never authorizes a mutation.
#[derive(Clone)]
pub(crate) struct CurrentStaticCollisionCertificate {
    certificate: Arc<CurrentStaticCollisionCertificateData>,
}

impl CurrentStaticCollisionCertificate {
    #[must_use]
    pub(super) fn same_certificate(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.certificate, &other.certificate)
    }

    #[must_use]
    pub(super) const fn authorizes_project_mutation(&self) -> bool {
        false
    }

    #[must_use]
    pub(super) const fn authorizes_sim_010(&self) -> bool {
        false
    }
}

/// Read-only access made available only while B is revalidated under the
/// fixed `project -> pose` lock order.
pub(crate) struct CurrentStaticCollisionView<'a> {
    certificate: &'a CurrentStaticCollisionCertificateData,
}

impl CurrentStaticCollisionView<'_> {
    #[must_use]
    pub(super) fn model(&self) -> &MaterialTreeKinematicsModel {
        self.certificate
            .pose_capability
            .claims
            .native_pose
            .tree()
            .expect("tree collision view")
            .0
    }

    #[must_use]
    pub(super) fn pose(&self) -> &MaterialTreePose {
        self.certificate
            .pose_capability
            .claims
            .native_pose
            .tree()
            .expect("tree collision view")
            .1
    }

    #[must_use]
    pub(super) fn geometry_proof(&self) -> &NativeStaticCollisionGeometryProof {
        &self.certificate.geometry_proof
    }

    #[must_use]
    pub(super) const fn pose_generation(&self) -> u64 {
        self.certificate.claims.pose_generation
    }

    #[must_use]
    pub(super) const fn paper_thickness_bits(&self) -> u64 {
        self.certificate.claims.paper_thickness_bits
    }

    #[must_use]
    pub(super) const fn policy_id(&self) -> &'static str {
        self.certificate.claims.policy_id
    }

    #[must_use]
    pub(super) const fn kinematics_model_id(&self) -> &'static str {
        self.certificate.claims.kinematics_model_id
    }

    #[must_use]
    pub(super) const fn thickness_model_id(&self) -> &'static str {
        self.certificate.claims.thickness_model_id
    }

    #[must_use]
    pub(super) const fn proof_id(&self) -> &'static str {
        self.certificate.claims.proof_id
    }
}

/// Captures B, performs the native geometry proof without either live lock,
/// and mints C only after exact B revalidation.
pub(crate) fn certify_current_static_collision(
    app_state: &AppState,
    limits: StaticCollisionLimits,
) -> Result<Option<CurrentStaticCollisionCertificate>, CurrentStaticCollisionError> {
    let Some(capability) = capture_current_pose_capability(app_state)? else {
        return Ok(None);
    };
    let prepared = prepare_static_collision(capability, limits).map_err(|failure| failure.error)?;
    mint_current_static_collision(app_state, prepared)
}

/// Runs the current-pose static diagnosis away from both live locks and
/// returns only stable, redacted categories. The certificate is intentionally
/// ephemeral because this command is observation-only.
///
/// The process-wide permit covers this complete command. Serializing the
/// preceding apply response with this later inspection remains the frontend
/// coordinator's responsibility because that transaction spans two IPC calls.
pub(crate) async fn inspect_current_static_collision(
    app_state: &AppState,
) -> Result<CurrentStaticCollisionDiagnosticResponse, String> {
    inspect_current_static_collision_with_limits(app_state, StaticCollisionLimits::default()).await
}

async fn inspect_current_static_collision_with_limits(
    app_state: &AppState,
    limits: StaticCollisionLimits,
) -> Result<CurrentStaticCollisionDiagnosticResponse, String> {
    let permit = app_state
        .try_acquire_native_pose_worker()
        .ok_or_else(|| CURRENT_STATIC_COLLISION_DIAGNOSTIC_FAILED_MESSAGE.to_owned())?;
    let capability = match capture_current_pose_capability(app_state) {
        Ok(Some(capability)) => capability,
        Ok(None) => {
            return Ok(CurrentStaticCollisionDiagnosticResponse::pose_unavailable(
                None,
            ));
        }
        Err(error) => return diagnostic_response_from_error(error, None),
    };
    let binding = CurrentAppliedPoseBindingResponse::from_claims(&capability.claims);
    let (permit, prepared) = tauri::async_runtime::spawn_blocking(move || {
        (
            permit,
            prepare_static_collision_for_diagnostic(capability, limits),
        )
    })
    .await
    .map_err(|_| CURRENT_STATIC_COLLISION_DIAGNOSTIC_FAILED_MESSAGE.to_owned())?;

    let prepared = match prepared {
        Ok(prepared) => prepared,
        Err(failure) => {
            return diagnostic_response_from_revalidated_failure(app_state, *failure);
        }
    };
    let response = match mint_current_static_collision(app_state, prepared) {
        Ok(Some(certificate)) => {
            let response =
                CurrentStaticCollisionDiagnosticResponse::certified_nonblocking(&certificate);
            drop(certificate);
            Ok(response)
        }
        Ok(None) => Ok(CurrentStaticCollisionDiagnosticResponse::pose_unavailable(
            None,
        )),
        Err(error) => diagnostic_response_from_error(error, Some(binding)),
    };
    drop(permit);
    response
}

impl CurrentStaticCollisionDiagnosticResponse {
    fn certified_nonblocking(certificate: &CurrentStaticCollisionCertificate) -> Self {
        Self {
            binding: Some(CurrentAppliedPoseBindingResponse::from_claims(
                &certificate.certificate.pose_capability.claims,
            )),
            status: CurrentStaticCollisionDiagnosticStatus::CertifiedNonblocking,
            reason: None,
            expected_unordered_face_pairs: Some(
                certificate
                    .certificate
                    .geometry_proof
                    .expected_unordered_face_pairs(),
            ),
            proven_transversal_pairs: Some(0),
            first_proven_transversal_pair: None,
            pair_classification_counts: Some(CurrentStaticCollisionPairClassificationCounts {
                separated: 0,
                touching: 0,
                allowed: 0,
                penetrating: 0,
                indeterminate: 0,
                candidate_excluded: 0,
            }),
            pair_diagnostics: Some(Vec::new()),
        }
    }

    const fn pose_unavailable(binding: Option<CurrentAppliedPoseBindingResponse>) -> Self {
        Self {
            binding,
            status: CurrentStaticCollisionDiagnosticStatus::Unavailable,
            reason: Some(CurrentStaticCollisionDiagnosticReason::PoseAuthorityUnavailable),
            expected_unordered_face_pairs: None,
            proven_transversal_pairs: None,
            first_proven_transversal_pair: None,
            pair_classification_counts: None,
            pair_diagnostics: None,
        }
    }

    const fn blocking(
        binding: Option<CurrentAppliedPoseBindingResponse>,
        reason: CurrentStaticCollisionDiagnosticReason,
    ) -> Self {
        Self {
            binding,
            status: CurrentStaticCollisionDiagnosticStatus::Blocking,
            reason: Some(reason),
            expected_unordered_face_pairs: None,
            proven_transversal_pairs: None,
            first_proven_transversal_pair: None,
            pair_classification_counts: None,
            pair_diagnostics: None,
        }
    }

    fn with_snapshot(mut self, snapshot: &StaticCollisionDiagnosticSnapshot) -> Self {
        self.pair_classification_counts = Some(snapshot.into());
        self.pair_diagnostics = Some(
            snapshot
                .pairs()
                .iter()
                .map(CurrentStaticCollisionPairDiagnostic::from)
                .collect(),
        );
        self
    }
}

fn diagnostic_response_from_error(
    error: CurrentStaticCollisionError,
    binding: Option<CurrentAppliedPoseBindingResponse>,
) -> Result<CurrentStaticCollisionDiagnosticResponse, String> {
    diagnostic_response_from_error_with_snapshot(error, binding, None)
}

fn diagnostic_response_from_error_with_snapshot(
    error: CurrentStaticCollisionError,
    binding: Option<CurrentAppliedPoseBindingResponse>,
    snapshot: Option<&StaticCollisionDiagnosticSnapshot>,
) -> Result<CurrentStaticCollisionDiagnosticResponse, String> {
    let mut response = match error {
        CurrentStaticCollisionError::LockUnavailable => {
            return Err(CURRENT_STATIC_COLLISION_DIAGNOSTIC_FAILED_MESSAGE.to_owned());
        }
        CurrentStaticCollisionError::PoseAuthorityUnavailable => {
            CurrentStaticCollisionDiagnosticResponse::pose_unavailable(None)
        }
        CurrentStaticCollisionError::InternalInconsistency => {
            CurrentStaticCollisionDiagnosticResponse::blocking(
                binding,
                CurrentStaticCollisionDiagnosticReason::InconsistentState,
            )
        }
        CurrentStaticCollisionError::GeometryBlocking(error) => match error {
            StaticCollisionError::PoseIssuerMismatch
            | StaticCollisionError::InvalidPaperThickness
            | StaticCollisionError::InconsistentMaterialPose => {
                CurrentStaticCollisionDiagnosticResponse::blocking(
                    binding,
                    CurrentStaticCollisionDiagnosticReason::InconsistentState,
                )
            }
            StaticCollisionError::ResourceLimitExceeded => {
                CurrentStaticCollisionDiagnosticResponse::blocking(
                    binding,
                    CurrentStaticCollisionDiagnosticReason::ResourceLimitExceeded,
                )
            }
            StaticCollisionError::PairEvidenceUnavailable {
                expected_unordered_face_pairs,
            } => CurrentStaticCollisionDiagnosticResponse {
                binding,
                status: CurrentStaticCollisionDiagnosticStatus::Blocking,
                reason: Some(CurrentStaticCollisionDiagnosticReason::EvidenceUnavailable),
                expected_unordered_face_pairs: Some(expected_unordered_face_pairs),
                proven_transversal_pairs: None,
                first_proven_transversal_pair: None,
                pair_classification_counts: None,
                pair_diagnostics: None,
            },
            StaticCollisionError::ProvenTransversalPenetration {
                expected_unordered_face_pairs,
                proven_transversal_pairs,
                first_proven_transversal_pair: [first_face_id, second_face_id],
            } => CurrentStaticCollisionDiagnosticResponse {
                binding,
                status: CurrentStaticCollisionDiagnosticStatus::Blocking,
                reason: Some(CurrentStaticCollisionDiagnosticReason::ProvenTransversalPenetration),
                expected_unordered_face_pairs: Some(expected_unordered_face_pairs),
                proven_transversal_pairs: Some(proven_transversal_pairs),
                first_proven_transversal_pair: Some(CurrentStaticCollisionFacePair {
                    first_face_id,
                    second_face_id,
                }),
                pair_classification_counts: None,
                pair_diagnostics: None,
            },
            StaticCollisionError::ProvenPositiveThicknessPenetration {
                expected_unordered_face_pairs,
                proven_positive_thickness_pairs,
                first_proven_positive_thickness_pair: [first_face_id, second_face_id],
            } => CurrentStaticCollisionDiagnosticResponse {
                binding,
                status: CurrentStaticCollisionDiagnosticStatus::Blocking,
                reason: Some(
                    CurrentStaticCollisionDiagnosticReason::ProvenPositiveThicknessPenetration,
                ),
                expected_unordered_face_pairs: Some(expected_unordered_face_pairs),
                proven_transversal_pairs: Some(proven_positive_thickness_pairs),
                first_proven_transversal_pair: Some(CurrentStaticCollisionFacePair {
                    first_face_id,
                    second_face_id,
                }),
                pair_classification_counts: None,
                pair_diagnostics: None,
            },
        },
    };
    if let Some(snapshot) = snapshot {
        response = response.with_snapshot(snapshot);
    }
    Ok(response)
}

fn diagnostic_response_from_revalidated_failure(
    app_state: &AppState,
    failure: FailedCurrentStaticCollisionPreparation,
) -> Result<CurrentStaticCollisionDiagnosticResponse, String> {
    let FailedCurrentStaticCollisionPreparation {
        pose_capability,
        error,
        diagnostic_snapshot,
    } = failure;
    let binding = CurrentAppliedPoseBindingResponse::from_claims(&pose_capability.claims);
    // Construct the bound blocking DTO inside the exact-B revalidation
    // closure, while both project and pose locks remain held. A later change
    // after those locks are released is rejected by the frontend's binding
    // gate; it can never turn this response into authority for another pose.
    let revalidated = super::with_revalidated_current_applied_pose_capability(
        app_state,
        &pose_capability,
        move |_, _| {
            diagnostic_response_from_error_with_snapshot(
                error,
                Some(binding),
                diagnostic_snapshot.as_ref(),
            )
        },
    )
    .map_err(|_| CURRENT_STATIC_COLLISION_DIAGNOSTIC_FAILED_MESSAGE.to_owned())?;
    match revalidated {
        Some(response) => response,
        None => Ok(CurrentStaticCollisionDiagnosticResponse::pose_unavailable(
            None,
        )),
    }
}

/// Revalidates the embedded B capability and runs an observation-only action
/// while the project and pose slot remain locked.
///
/// The action must not re-enter an operation that locks the project or pose
/// authority because these mutexes are deliberately non-reentrant. A future
/// consumer needing lock-free work must first capture an owned immutable
/// snapshot, release these locks, and revalidate this certificate again before
/// any authoritative commit.
pub(crate) fn with_revalidated_current_static_collision_certificate<R>(
    app_state: &AppState,
    certificate: &CurrentStaticCollisionCertificate,
    action: impl FnOnce(CurrentStaticCollisionView<'_>) -> R,
) -> Result<Option<R>, CurrentStaticCollisionError> {
    let project =
        lock_project(app_state).map_err(|_| CurrentStaticCollisionError::LockUnavailable)?;
    let data = certificate.certificate.as_ref();
    let capability = &data.pose_capability;
    let authority = project.applied_pose_authority.clone();
    if !Arc::ptr_eq(&authority.0, &capability.slot) {
        return Ok(None);
    }
    let slot = authority.lock().map_err(map_pose_authority_error)?;
    let Some(current) = slot.current.as_ref() else {
        return Ok(None);
    };
    if !current_applied_pose_capability_matches_locked_slot(&slot, &project, capability, current)
        || !current_static_collision_certificate_is_internally_consistent(data)
        || !current_static_collision_claims_are_current(&data.claims, &project)
    {
        return Ok(None);
    }

    // Keep both guards outside the unwind boundary. If an observation panics,
    // catching it here lets us drop both mutex guards while the thread is not
    // panicking, so neither project nor pose authority becomes poisoned. The
    // original panic resumes only after both locks are healthy and released.
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        action(CurrentStaticCollisionView { certificate: data })
    }));
    drop(slot);
    drop(project);
    match outcome {
        Ok(output) => Ok(Some(output)),
        Err(payload) => resume_unwind(payload),
    }
}

fn capture_current_pose_capability(
    app_state: &AppState,
) -> Result<Option<CurrentAppliedPoseCapability>, CurrentStaticCollisionError> {
    let project =
        lock_project(app_state).map_err(|_| CurrentStaticCollisionError::LockUnavailable)?;
    project
        .applied_pose_authority
        .capture_capability(&project)
        .map_err(map_pose_authority_error)
}

fn prepare_static_collision_for_diagnostic(
    capability: CurrentAppliedPoseCapability,
    limits: StaticCollisionLimits,
) -> Result<PreparedCurrentStaticCollision, Box<FailedCurrentStaticCollisionPreparation>> {
    if !detached_pose_capability_is_internally_consistent(&capability) {
        return Err(Box::new(FailedCurrentStaticCollisionPreparation {
            pose_capability: capability,
            error: CurrentStaticCollisionError::InternalInconsistency,
            diagnostic_snapshot: None,
        }));
    }
    let paper_thickness_mm = f64::from_bits(capability.claims.paper_thickness_bits);
    let Some((model, pose)) = capability.claims.native_pose.tree() else {
        return Err(Box::new(FailedCurrentStaticCollisionPreparation {
            pose_capability: capability,
            error: CurrentStaticCollisionError::InternalInconsistency,
            diagnostic_snapshot: None,
        }));
    };
    let snapshot = match diagnose_static_collision_geometry(model, pose, paper_thickness_mm, limits)
    {
        Ok(snapshot) => snapshot,
        Err(error) => {
            return Err(Box::new(FailedCurrentStaticCollisionPreparation {
                pose_capability: capability,
                error: map_static_collision_error(error),
                diagnostic_snapshot: None,
            }));
        }
    };
    if snapshot.expected_unordered_face_pairs() == 0 {
        // The public proof success set is currently exactly this allocation-
        // free single-face case. Mint through the authoritative proof entry
        // instead of reconstructing its opaque identity from diagnostics.
        return prepare_static_collision(capability, limits);
    }
    let error = blocking_error_from_diagnostic_snapshot(&snapshot, paper_thickness_mm);
    Err(Box::new(FailedCurrentStaticCollisionPreparation {
        pose_capability: capability,
        error: CurrentStaticCollisionError::GeometryBlocking(error),
        diagnostic_snapshot: Some(snapshot),
    }))
}

fn blocking_error_from_diagnostic_snapshot(
    snapshot: &StaticCollisionDiagnosticSnapshot,
    paper_thickness_mm: f64,
) -> StaticCollisionError {
    let expected_unordered_face_pairs = snapshot.expected_unordered_face_pairs();
    let first_penetrating_pair = snapshot
        .pairs()
        .iter()
        .find(|pair| {
            matches!(
                pair.disposition(),
                StaticCollisionPairDisposition::Penetrating
            )
        })
        .map(|pair| [pair.first_face(), pair.second_face()]);
    if let Some(first_pair) = first_penetrating_pair {
        if paper_thickness_mm > 0.0 {
            return StaticCollisionError::ProvenPositiveThicknessPenetration {
                expected_unordered_face_pairs,
                proven_positive_thickness_pairs: snapshot.penetrating_pairs(),
                first_proven_positive_thickness_pair: first_pair,
            };
        }
        if paper_thickness_mm.to_bits() == 0.0_f64.to_bits() {
            return StaticCollisionError::ProvenTransversalPenetration {
                expected_unordered_face_pairs,
                proven_transversal_pairs: snapshot.penetrating_pairs(),
                first_proven_transversal_pair: first_pair,
            };
        }
    }
    StaticCollisionError::PairEvidenceUnavailable {
        expected_unordered_face_pairs,
    }
}

fn prepare_static_collision(
    capability: CurrentAppliedPoseCapability,
    limits: StaticCollisionLimits,
) -> Result<PreparedCurrentStaticCollision, Box<FailedCurrentStaticCollisionPreparation>> {
    if !detached_pose_capability_is_internally_consistent(&capability) {
        return Err(Box::new(FailedCurrentStaticCollisionPreparation {
            pose_capability: capability,
            error: CurrentStaticCollisionError::InternalInconsistency,
            diagnostic_snapshot: None,
        }));
    }
    let paper_thickness_mm = f64::from_bits(capability.claims.paper_thickness_bits);
    // Every native geometry error remains blocking here. In particular, a
    // proven transversal penetration exits before Prepared B or certificate C
    // can be constructed.
    let Some((model, pose)) = capability.claims.native_pose.tree() else {
        return Err(Box::new(FailedCurrentStaticCollisionPreparation {
            pose_capability: capability,
            error: CurrentStaticCollisionError::InternalInconsistency,
            diagnostic_snapshot: None,
        }));
    };
    let geometry_proof =
        match prove_static_collision_geometry(model, pose, paper_thickness_mm, limits) {
            Ok(proof) => proof,
            Err(error) => {
                return Err(Box::new(FailedCurrentStaticCollisionPreparation {
                    pose_capability: capability,
                    error: map_static_collision_error(error),
                    diagnostic_snapshot: None,
                }));
            }
        };
    let pose_claims = &capability.claims;
    let claims = CurrentStaticCollisionClaims {
        project_instance_id: pose_claims.project_instance_id,
        project_id: pose_claims.project_id,
        revision: pose_claims.revision,
        fold_model_fingerprint: Arc::clone(&pose_claims.fold_model_fingerprint),
        pose_generation: pose_claims.generation,
        paper_thickness_bits: pose_claims.paper_thickness_bits,
        policy_id: pose_claims.contact_policy_id,
        kinematics_model_id: pose_claims.kinematics_model_id,
        thickness_model_id: pose_claims.thickness_model_id,
        proof_id: geometry_proof.proof_id(),
        proof_identity: geometry_proof.clone(),
    };
    let prepared = PreparedCurrentStaticCollision {
        pose_capability: capability,
        geometry_proof,
        claims,
    };
    if !prepared_static_collision_is_internally_consistent(&prepared) {
        let PreparedCurrentStaticCollision {
            pose_capability, ..
        } = prepared;
        return Err(Box::new(FailedCurrentStaticCollisionPreparation {
            pose_capability,
            error: CurrentStaticCollisionError::InternalInconsistency,
            diagnostic_snapshot: None,
        }));
    }
    Ok(prepared)
}

fn map_static_collision_error(error: StaticCollisionError) -> CurrentStaticCollisionError {
    CurrentStaticCollisionError::GeometryBlocking(error)
}

fn mint_current_static_collision(
    app_state: &AppState,
    prepared: PreparedCurrentStaticCollision,
) -> Result<Option<CurrentStaticCollisionCertificate>, CurrentStaticCollisionError> {
    let project =
        lock_project(app_state).map_err(|_| CurrentStaticCollisionError::LockUnavailable)?;
    let authority = project.applied_pose_authority.clone();
    if !Arc::ptr_eq(&authority.0, &prepared.pose_capability.slot) {
        return Ok(None);
    }
    let slot = authority.lock().map_err(map_pose_authority_error)?;
    let Some(current) = slot.current.as_ref() else {
        return Ok(None);
    };
    if !current_applied_pose_capability_matches_locked_slot(
        &slot,
        &project,
        &prepared.pose_capability,
        current,
    ) {
        return Ok(None);
    }
    if !prepared_static_collision_is_internally_consistent(&prepared)
        || !current_static_collision_claims_are_current(&prepared.claims, &project)
    {
        return Err(CurrentStaticCollisionError::InternalInconsistency);
    }

    // C changes no live state. Release both locks before the only remaining
    // allocation so allocator failure cannot poison project or pose authority.
    // A concurrent invalidation after this point is handled by mandatory B
    // revalidation on every later use.
    drop(slot);
    drop(project);
    let certificate = Arc::new(CurrentStaticCollisionCertificateData {
        pose_capability: prepared.pose_capability,
        geometry_proof: prepared.geometry_proof,
        claims: prepared.claims,
    });
    if !current_static_collision_certificate_is_internally_consistent(&certificate) {
        return Err(CurrentStaticCollisionError::InternalInconsistency);
    }
    Ok(Some(CurrentStaticCollisionCertificate { certificate }))
}

fn detached_pose_capability_is_internally_consistent(
    capability: &CurrentAppliedPoseCapability,
) -> bool {
    current_applied_pose_certificate_is_internally_consistent(&capability.certificate)
        && current_applied_pose_claims_match(&capability.claims, &capability.certificate.claims)
}

fn prepared_static_collision_is_internally_consistent(
    prepared: &PreparedCurrentStaticCollision,
) -> bool {
    detached_pose_capability_is_internally_consistent(&prepared.pose_capability)
        && static_collision_claims_match_pose(
            &prepared.claims,
            &prepared.pose_capability,
            &prepared.geometry_proof,
        )
}

fn current_static_collision_certificate_is_internally_consistent(
    certificate: &CurrentStaticCollisionCertificateData,
) -> bool {
    detached_pose_capability_is_internally_consistent(&certificate.pose_capability)
        && static_collision_claims_match_pose(
            &certificate.claims,
            &certificate.pose_capability,
            &certificate.geometry_proof,
        )
}

fn static_collision_claims_match_pose(
    claims: &CurrentStaticCollisionClaims,
    capability: &CurrentAppliedPoseCapability,
    geometry_proof: &NativeStaticCollisionGeometryProof,
) -> bool {
    let pose_claims = &capability.claims;
    claims.project_instance_id == pose_claims.project_instance_id
        && claims.project_id == pose_claims.project_id
        && claims.revision == pose_claims.revision
        && Arc::ptr_eq(
            &claims.fold_model_fingerprint,
            &pose_claims.fold_model_fingerprint,
        )
        && claims.fold_model_fingerprint.as_ref() == pose_claims.fold_model_fingerprint.as_ref()
        && claims.pose_generation == pose_claims.generation
        && claims.paper_thickness_bits == pose_claims.paper_thickness_bits
        && claims.policy_id == TOPOLOGY_CONTACT_POLICY_V2
        && claims.policy_id == pose_claims.contact_policy_id
        && claims.kinematics_model_id == MATERIAL_TREE_KINEMATICS_MODEL_ID
        && claims.kinematics_model_id == pose_claims.kinematics_model_id
        && claims.thickness_model_id == CENTERED_MID_SURFACE_THICKNESS_MODEL_V1
        && claims.thickness_model_id == pose_claims.thickness_model_id
        && claims.proof_id == NATIVE_STATIC_COLLISION_GEOMETRY_PROOF_V1
        && claims.proof_id == geometry_proof.proof_id()
        && claims.proof_identity.same_proof(geometry_proof)
        && geometry_proof.policy_id() == claims.policy_id
        && geometry_proof.kinematics_model_id() == claims.kinematics_model_id
        && geometry_proof.thickness_model_id() == claims.thickness_model_id
        && geometry_proof.paper_thickness_bits() == claims.paper_thickness_bits
        && pose_claims.native_pose.tree().is_some_and(|(model, pose)| {
            model.bind_pose(pose).is_ok()
                && geometry_proof.is_for_geometry(
                    model,
                    pose,
                    f64::from_bits(claims.paper_thickness_bits),
                )
        })
        && geometry_proof.face_count() == pose_claims.material_faces.len()
        && geometry_proof.expected_unordered_face_pairs()
            == geometry_proof.analyzed_unordered_face_pairs()
}

fn current_static_collision_claims_are_current(
    claims: &CurrentStaticCollisionClaims,
    project: &ProjectState,
) -> bool {
    claims.project_instance_id == project.instance_id
        && claims.project_id == project.project_id
        && claims.revision == project.editor.revision()
        && claims.fold_model_fingerprint.as_ref() == project.editor.fold_model_fingerprint_v1()
        && claims.paper_thickness_bits == project.editor.paper().thickness_mm.to_bits()
}

fn map_pose_authority_error(error: super::PoseAuthorityError) -> CurrentStaticCollisionError {
    match error {
        super::PoseAuthorityError::LockUnavailable => CurrentStaticCollisionError::LockUnavailable,
        super::PoseAuthorityError::InternalInconsistency => {
            CurrentStaticCollisionError::InternalInconsistency
        }
        _ => CurrentStaticCollisionError::PoseAuthorityUnavailable,
    }
}

#[cfg(test)]
mod tests {
    use std::{marker::PhantomData, path::PathBuf};

    use ori_collision::{StaticCollisionError, prove_static_collision_geometry};
    use ori_core::{Command, create_rectangular_sheet};
    use ori_domain::{
        CreasePattern, Edge, EdgeId, EdgeKind, FaceId, Paper, Point2, Vertex, VertexId,
    };
    use ori_kinematics::{CanonicalHingeAngles, MaterialTreeKinematicsModel, TreeKinematicsLimits};

    use super::*;
    use crate::{
        ProjectState, applied_pose::NativePoseHingeAngleRequest, applied_pose::NativePoseRequest,
        commit_project_replacement, execute_command as execute_bound_command,
    };

    fn execute_command(
        project: &mut ProjectState,
        expected_project_id: ProjectId,
        expected_revision: u64,
        command: Command,
    ) -> Result<crate::ProjectSnapshot, String> {
        execute_bound_command(
            project,
            project.instance_id,
            expected_project_id,
            expected_revision,
            command,
        )
    }

    fn no_hinge_project_with_thickness(thickness_mm: f64) -> ProjectState {
        let sheet = create_rectangular_sheet(40.0, 30.0, false).expect("rectangle fixture");
        let (pattern, mut paper) = sheet.into_parts();
        paper.thickness_mm = thickness_mm;
        ProjectState::new_with_paper(pattern, paper)
    }

    fn no_hinge_project() -> ProjectState {
        no_hinge_project_with_thickness(Paper::default().thickness_mm)
    }

    fn midpoint_mountain_400mm_project() -> (ProjectState, [EdgeId; 2]) {
        midpoint_mountain_400mm_project_with_thickness(0.0)
    }

    fn midpoint_mountain_400mm_project_with_thickness(
        thickness_mm: f64,
    ) -> (ProjectState, [EdgeId; 2]) {
        let coordinates = [
            (0.0, 0.0),
            (200.0, 0.0),
            (400.0, 0.0),
            (400.0, 400.0),
            (0.0, 400.0),
        ];
        let vertices = coordinates
            .into_iter()
            .map(|(x, y)| Vertex {
                id: VertexId::new(),
                position: Point2::new(x, y),
            })
            .collect::<Vec<_>>();
        let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
        let mut edges = (0..boundary.len())
            .map(|index| Edge {
                id: EdgeId::new(),
                start: boundary[index],
                end: boundary[(index + 1) % boundary.len()],
                kind: EdgeKind::Boundary,
            })
            .collect::<Vec<_>>();
        let hinges = [EdgeId::new(), EdgeId::new()];
        edges.extend([
            Edge {
                id: hinges[0],
                start: boundary[1],
                end: boundary[4],
                kind: EdgeKind::Mountain,
            },
            Edge {
                id: hinges[1],
                start: boundary[1],
                end: boundary[3],
                kind: EdgeKind::Mountain,
            },
        ]);
        let pattern = CreasePattern { vertices, edges };
        let paper = Paper {
            boundary_vertices: boundary,
            thickness_mm,
            ..Paper::default()
        };
        (ProjectState::new_with_paper(pattern, paper), hinges)
    }

    fn only_non_hinge_face_pair(model: &MaterialTreeKinematicsModel) -> [FaceId; 2] {
        let mut pairs = model
            .face_ids()
            .iter()
            .copied()
            .enumerate()
            .flat_map(|(first_index, first)| {
                model.face_ids()[first_index + 1..]
                    .iter()
                    .copied()
                    .map(move |second| [first, second])
            })
            .filter(|pair| {
                !model.hinges().iter().any(|hinge| {
                    let mut hinge_pair = [hinge.left_face(), hinge.right_face()];
                    hinge_pair.sort_unstable_by_key(FaceId::canonical_bytes);
                    hinge_pair == *pair
                })
            })
            .collect::<Vec<_>>();
        assert_eq!(pairs.len(), 1, "three-face V has one non-hinge pair");
        pairs.pop().expect("non-hinge pair")
    }

    fn request_for(project: &ProjectState) -> NativePoseRequest {
        NativePoseRequest {
            expected_project_instance_id: project.instance_id,
            expected_project_id: project.project_id,
            expected_revision: project.editor.revision(),
            fixed_face_id: None,
            complete_hinge_angles: Vec::new(),
        }
    }

    fn adopt_request(project: &mut ProjectState, request: NativePoseRequest) {
        let authority = project.applied_pose_authority.clone();
        let prepared = authority
            .capture_request(project, request)
            .expect("capture")
            .prepare()
            .expect("prepare");
        authority
            .commit_prepared(project, prepared)
            .expect("commit");
    }

    fn adopt_no_hinge_pose(project: &mut ProjectState) {
        let request = request_for(project);
        adopt_request(project, request);
    }

    fn adopt_no_hinge_pose_in_state(state: &AppState) {
        let (authority, captured) = {
            let project = state.0.lock().expect("project lock");
            let authority = project.applied_pose_authority.clone();
            let captured = authority
                .capture_request(&project, request_for(&project))
                .expect("capture");
            (authority, captured)
        };
        let prepared = captured.prepare().expect("prepare");
        let mut project = state.0.lock().expect("project lock");
        authority
            .commit_prepared(&mut project, prepared)
            .expect("commit");
    }

    fn certified_no_hinge_state(
        thickness_mm: f64,
    ) -> (AppState, CurrentStaticCollisionCertificate) {
        let mut project = no_hinge_project_with_thickness(thickness_mm);
        adopt_no_hinge_pose(&mut project);
        let state = AppState::new(project);
        let certificate =
            certify_current_static_collision(&state, StaticCollisionLimits::default())
                .expect("certification")
                .expect("current certificate");
        (state, certificate)
    }

    fn prepared_from_current(state: &AppState) -> PreparedCurrentStaticCollision {
        let capability = capture_current_pose_capability(state)
            .expect("capture")
            .expect("current pose");
        prepare_static_collision(capability, StaticCollisionLimits::default()).expect("proof")
    }

    fn current_binding(state: &AppState) -> CurrentAppliedPoseBindingResponse {
        let capability = capture_current_pose_capability(state)
            .expect("capture")
            .expect("current pose");
        CurrentAppliedPoseBindingResponse::from_claims(&capability.claims)
    }

    fn adopted_no_hinge_state() -> AppState {
        let mut project = no_hinge_project();
        adopt_no_hinge_pose(&mut project);
        AppState::new(project)
    }

    fn blocking_failure_from_current(state: &AppState) -> FailedCurrentStaticCollisionPreparation {
        let capability = capture_current_pose_capability(state)
            .expect("capture")
            .expect("current pose");
        match prepare_static_collision(
            capability,
            StaticCollisionLimits {
                max_faces: 0,
                ..StaticCollisionLimits::default()
            },
        ) {
            Ok(_) => panic!("zero face limit must block preparation"),
            Err(failure) => *failure,
        }
    }

    fn unavailable_diagnostic() -> CurrentStaticCollisionDiagnosticResponse {
        CurrentStaticCollisionDiagnosticResponse::pose_unavailable(None)
    }

    #[test]
    fn busy_process_worker_rejects_inspection_without_touching_current_pose() {
        let state = adopted_no_hinge_state();
        let binding_before = current_binding(&state);
        let (authority, authority_before, document_before) = {
            let project = state.0.lock().expect("project lock");
            let authority = project.applied_pose_authority.clone();
            let authority_before = authority.test_snapshot().expect("authority snapshot");
            (authority, authority_before, project.document())
        };
        let permit = state
            .try_acquire_native_pose_worker()
            .expect("first process worker permit");

        let error = tauri::async_runtime::block_on(inspect_current_static_collision_with_limits(
            &state,
            StaticCollisionLimits::default(),
        ))
        .expect_err("busy worker must reject another inspection");
        assert_eq!(error, CURRENT_STATIC_COLLISION_DIAGNOSTIC_FAILED_MESSAGE);
        assert_eq!(
            authority.test_snapshot().expect("unchanged authority"),
            authority_before
        );
        {
            let project = state.0.lock().expect("project lock");
            assert_eq!(project.document(), document_before);
        }
        assert_eq!(current_binding(&state), binding_before);

        drop(permit);
        assert!(!state.native_pose_worker_is_busy());
        let diagnosis = tauri::async_runtime::block_on(
            inspect_current_static_collision_with_limits(&state, StaticCollisionLimits::default()),
        )
        .expect("released permit must allow inspection");
        assert_eq!(
            diagnosis.status,
            CurrentStaticCollisionDiagnosticStatus::CertifiedNonblocking
        );
        assert!(!state.native_pose_worker_is_busy());
    }

    #[test]
    fn current_blocking_failure_keeps_its_exact_binding() {
        let state = adopted_no_hinge_state();
        let expected_binding = current_binding(&state);
        let response = diagnostic_response_from_revalidated_failure(
            &state,
            blocking_failure_from_current(&state),
        )
        .expect("current blocking response");
        assert_eq!(
            response,
            CurrentStaticCollisionDiagnosticResponse {
                binding: Some(expected_binding),
                status: CurrentStaticCollisionDiagnosticStatus::Blocking,
                reason: Some(CurrentStaticCollisionDiagnosticReason::ResourceLimitExceeded),
                expected_unordered_face_pairs: None,
                proven_transversal_pairs: None,
                first_proven_transversal_pair: None,
                pair_classification_counts: None,
                pair_diagnostics: None,
            }
        );
    }

    #[test]
    fn post_return_pose_change_is_visible_to_the_frontend_binding_gate() {
        let state = adopted_no_hinge_state();
        let response = diagnostic_response_from_revalidated_failure(
            &state,
            blocking_failure_from_current(&state),
        )
        .expect("bound blocking response");
        let returned_binding = response.binding.expect("current response binding");

        // Native exact-B revalidation covered response construction. Once the
        // locks are released a later pose may legitimately replace it, and the
        // frontend must reject the old DTO by this complete binding mismatch.
        adopt_no_hinge_pose_in_state(&state);
        assert_ne!(returned_binding, current_binding(&state));
        assert_eq!(
            response.status,
            CurrentStaticCollisionDiagnosticStatus::Blocking
        );
    }

    #[test]
    fn stale_blocking_failure_is_unbound_after_readoption_edit_reopen_or_foreign_slot() {
        let same_angle_state = adopted_no_hinge_state();
        let same_angle_failure = blocking_failure_from_current(&same_angle_state);
        let original_binding = current_binding(&same_angle_state);
        adopt_no_hinge_pose_in_state(&same_angle_state);
        assert_ne!(current_binding(&same_angle_state), original_binding);
        assert_eq!(
            diagnostic_response_from_revalidated_failure(&same_angle_state, same_angle_failure,)
                .expect("same-angle stale diagnosis"),
            unavailable_diagnostic()
        );

        let edited_state = adopted_no_hinge_state();
        let edited_failure = blocking_failure_from_current(&edited_state);
        {
            let mut project = edited_state.0.lock().expect("project lock");
            let project_id = project.project_id;
            let revision = project.editor.revision();
            execute_command(
                &mut project,
                project_id,
                revision,
                Command::AddVertex {
                    id: VertexId::new(),
                    position: Point2::new(1.0, 1.0),
                },
            )
            .expect("edit");
        }
        assert_eq!(
            diagnostic_response_from_revalidated_failure(&edited_state, edited_failure)
                .expect("post-edit stale diagnosis"),
            unavailable_diagnostic()
        );

        let reopened_state = adopted_no_hinge_state();
        let reopened_failure = blocking_failure_from_current(&reopened_state);
        {
            let mut project = reopened_state.0.lock().expect("project lock");
            let document = project.document();
            let replacement =
                ProjectState::from_document(document, PathBuf::from("same-project.ori2"));
            commit_project_replacement(&mut project, replacement).expect("reopen");
        }
        assert_eq!(
            diagnostic_response_from_revalidated_failure(&reopened_state, reopened_failure)
                .expect("post-reopen stale diagnosis"),
            unavailable_diagnostic()
        );

        let source_state = adopted_no_hinge_state();
        let foreign_failure = blocking_failure_from_current(&source_state);
        let foreign_state = adopted_no_hinge_state();
        assert_eq!(
            diagnostic_response_from_revalidated_failure(&foreign_state, foreign_failure)
                .expect("foreign-slot stale diagnosis"),
            unavailable_diagnostic()
        );
    }

    #[test]
    fn single_face_certificate_is_opaque_observational_and_identity_preserving() {
        let (state, certificate) = certified_no_hinge_state(0.1);
        let cloned = certificate.clone();
        assert!(certificate.same_certificate(&cloned));
        assert!(!certificate.authorizes_project_mutation());
        assert!(!certificate.authorizes_sim_010());

        let observed =
            with_revalidated_current_static_collision_certificate(&state, &certificate, |view| {
                assert_eq!(view.model().face_ids().len(), 1);
                assert!(view.pose().hinges().is_empty());
                assert_eq!(view.pose_generation(), 1);
                assert_eq!(view.paper_thickness_bits(), 0.1_f64.to_bits());
                assert_eq!(view.policy_id(), TOPOLOGY_CONTACT_POLICY_V2);
                assert_eq!(
                    view.kinematics_model_id(),
                    MATERIAL_TREE_KINEMATICS_MODEL_ID
                );
                assert_eq!(
                    view.thickness_model_id(),
                    CENTERED_MID_SURFACE_THICKNESS_MODEL_V1
                );
                assert_eq!(view.proof_id(), NATIVE_STATIC_COLLISION_GEOMETRY_PROOF_V1);
                assert_eq!(view.geometry_proof().face_count(), 1);
                assert_eq!(view.geometry_proof().expected_unordered_face_pairs(), 0);
                assert_eq!(view.geometry_proof().analyzed_unordered_face_pairs(), 0);
            })
            .expect("revalidate");
        assert!(observed.is_some());

        let separately_minted =
            certify_current_static_collision(&state, StaticCollisionLimits::default())
                .expect("second certification")
                .expect("second certificate");
        assert!(!certificate.same_certificate(&separately_minted));
    }

    #[test]
    fn panicking_observation_releases_both_locks_without_poisoning_them() {
        let (state, certificate) = certified_no_hinge_state(0.1);

        let panic = catch_unwind(AssertUnwindSafe(|| {
            let _ = with_revalidated_current_static_collision_certificate(
                &state,
                &certificate,
                |_| -> () { panic!("observation panic fixture") },
            );
        }));
        assert!(panic.is_err());

        assert!(
            state.0.try_lock().is_ok(),
            "project lock must remain available after observation panic"
        );
        let authority = {
            let project = state.0.lock().expect("healthy project lock");
            project.applied_pose_authority.clone()
        };
        assert!(
            authority.0.try_lock().is_ok(),
            "pose lock must remain available after observation panic"
        );
        assert!(
            with_revalidated_current_static_collision_certificate(&state, &certificate, |_| (),)
                .expect("healthy revalidation after panic")
                .is_some(),
            "the same still-current certificate must remain usable"
        );
    }

    #[test]
    fn certificate_does_not_implement_serialize() {
        struct Check<T: ?Sized>(PhantomData<T>);
        trait AmbiguousIfSerialize<A> {
            fn marker() {}
        }
        impl<T: ?Sized> AmbiguousIfSerialize<()> for Check<T> {}
        impl<T: ?Sized + serde::Serialize> AmbiguousIfSerialize<u8> for Check<T> {}

        let _ = <Check<CurrentStaticCollisionCertificate> as AmbiguousIfSerialize<_>>::marker;
    }

    #[test]
    fn native_geometry_proof_runs_after_project_and_pose_locks_are_released() {
        let mut project = no_hinge_project();
        adopt_no_hinge_pose(&mut project);
        let state = AppState::new(project);
        let capability = capture_current_pose_capability(&state)
            .expect("capture")
            .expect("current pose");

        assert!(state.0.try_lock().is_ok(), "project lock leaked into proof");
        assert!(
            capability.slot.try_lock().is_ok(),
            "pose lock leaked into proof"
        );
        let prepared = prepare_static_collision(capability, StaticCollisionLimits::default())
            .expect("lock-free proof");
        assert!(
            mint_current_static_collision(&state, prepared)
                .expect("mint")
                .is_some()
        );
    }

    #[test]
    fn same_angle_readoption_edit_reopen_and_foreign_slot_are_rejected() {
        let (state, certificate) = certified_no_hinge_state(0.1);

        adopt_no_hinge_pose_in_state(&state);
        assert!(
            with_revalidated_current_static_collision_certificate(&state, &certificate, |_| (),)
                .expect("same-angle revalidation")
                .is_none()
        );

        let fresh = certify_current_static_collision(&state, StaticCollisionLimits::default())
            .expect("fresh certification")
            .expect("fresh certificate");
        {
            let mut project = state.0.lock().expect("project lock");
            let project_id = project.project_id;
            let revision = project.editor.revision();
            execute_command(
                &mut project,
                project_id,
                revision,
                Command::AddVertex {
                    id: VertexId::new(),
                    position: Point2::new(1.0, 1.0),
                },
            )
            .expect("edit");
        }
        assert!(
            with_revalidated_current_static_collision_certificate(&state, &fresh, |_| ())
                .expect("post-edit revalidation")
                .is_none()
        );

        adopt_no_hinge_pose_in_state(&state);
        let before_reopen =
            certify_current_static_collision(&state, StaticCollisionLimits::default())
                .expect("pre-reopen certification")
                .expect("pre-reopen certificate");
        {
            let mut project = state.0.lock().expect("project lock");
            let document = project.document();
            let replacement =
                ProjectState::from_document(document, PathBuf::from("same-project.ori2"));
            commit_project_replacement(&mut project, replacement).expect("reopen");
        }
        assert!(
            with_revalidated_current_static_collision_certificate(&state, &before_reopen, |_| (),)
                .expect("post-reopen revalidation")
                .is_none()
        );

        let mut other_project = no_hinge_project();
        adopt_no_hinge_pose(&mut other_project);
        let other_state = AppState::new(other_project);
        assert!(
            with_revalidated_current_static_collision_certificate(
                &other_state,
                &before_reopen,
                |_| (),
            )
            .expect("foreign-slot revalidation")
            .is_none()
        );
    }

    #[test]
    fn stale_between_lock_free_proof_and_mint_is_rejected() {
        let mut project = no_hinge_project();
        adopt_no_hinge_pose(&mut project);
        let state = AppState::new(project);
        let prepared = prepared_from_current(&state);

        adopt_no_hinge_pose_in_state(&state);
        assert!(
            mint_current_static_collision(&state, prepared)
                .expect("stale mint")
                .is_none()
        );
    }

    #[test]
    fn proof_identity_pose_issuer_and_zero_sign_mismatches_are_rejected() {
        let (state, _) = certified_no_hinge_state(-0.0);

        let mut proof_identity_mismatch = prepared_from_current(&state);
        let pose_claims = &proof_identity_mismatch.pose_capability.claims;
        let (model, pose) = pose_claims.native_pose.tree().unwrap();
        proof_identity_mismatch.geometry_proof =
            prove_static_collision_geometry(model, pose, -0.0, StaticCollisionLimits::default())
                .expect("second exact proof");
        assert!(matches!(
            mint_current_static_collision(&state, proof_identity_mismatch),
            Err(CurrentStaticCollisionError::InternalInconsistency)
        ));

        let mut pose_mismatch = prepared_from_current(&state);
        let pose_claims = &pose_mismatch.pose_capability.claims;
        let (model, _) = pose_claims.native_pose.tree().unwrap();
        let second_pose = pose_claims
            .native_pose
            .tree()
            .unwrap()
            .0
            .solve(
                None,
                &CanonicalHingeAngles::new(Vec::new()).expect("empty angles"),
            )
            .expect("same-angle pose");
        let second_pose_proof = prove_static_collision_geometry(
            model,
            &second_pose,
            -0.0,
            StaticCollisionLimits::default(),
        )
        .expect("same-angle proof");
        pose_mismatch.claims.proof_identity = second_pose_proof.clone();
        pose_mismatch.geometry_proof = second_pose_proof;
        assert!(matches!(
            mint_current_static_collision(&state, pose_mismatch),
            Err(CurrentStaticCollisionError::InternalInconsistency)
        ));

        let mut issuer_mismatch = prepared_from_current(&state);
        let pose_claims = &issuer_mismatch.pose_capability.claims;
        let foreign_model = MaterialTreeKinematicsModel::prepare(
            pose_claims.topology_input.pattern(),
            pose_claims.topology_input.paper(),
            pose_claims.topology.as_ref(),
            TreeKinematicsLimits::default(),
        )
        .expect("foreign model");
        let foreign_pose = foreign_model
            .solve(
                None,
                &CanonicalHingeAngles::new(Vec::new()).expect("empty angles"),
            )
            .expect("foreign pose");
        let foreign_proof = prove_static_collision_geometry(
            &foreign_model,
            &foreign_pose,
            -0.0,
            StaticCollisionLimits::default(),
        )
        .expect("foreign proof");
        issuer_mismatch.claims.proof_identity = foreign_proof.clone();
        issuer_mismatch.geometry_proof = foreign_proof;
        assert!(matches!(
            mint_current_static_collision(&state, issuer_mismatch),
            Err(CurrentStaticCollisionError::InternalInconsistency)
        ));

        let mut zero_sign_mismatch = prepared_from_current(&state);
        let pose_claims = &zero_sign_mismatch.pose_capability.claims;
        assert_eq!(pose_claims.paper_thickness_bits, (-0.0_f64).to_bits());
        let (model, pose) = pose_claims.native_pose.tree().unwrap();
        let positive_zero_proof =
            prove_static_collision_geometry(model, pose, 0.0, StaticCollisionLimits::default())
                .expect("positive-zero proof");
        zero_sign_mismatch.claims.proof_identity = positive_zero_proof.clone();
        zero_sign_mismatch.geometry_proof = positive_zero_proof;
        assert!(matches!(
            mint_current_static_collision(&state, zero_sign_mismatch),
            Err(CurrentStaticCollisionError::InternalInconsistency)
        ));
    }

    #[test]
    fn actual_midpoint_transversal_penetration_blocks_before_certificate_c_can_be_minted() {
        let (project, hinges) = midpoint_mountain_400mm_project();
        let topology = project
            .editor
            .topology_analysis_input(project.project_id)
            .analyze()
            .simulation_snapshot()
            .expect("three-face midpoint topology")
            .clone();
        assert_eq!(topology.faces.len(), 3);
        let mut complete_hinge_angles = hinges
            .into_iter()
            .map(|edge_id| NativePoseHingeAngleRequest {
                edge_id,
                angle_degrees: 135.0,
            })
            .collect::<Vec<_>>();
        complete_hinge_angles.sort_by_key(|angle| angle.edge_id.canonical_bytes());
        let request = NativePoseRequest {
            expected_project_instance_id: project.instance_id,
            expected_project_id: project.project_id,
            expected_revision: project.editor.revision(),
            fixed_face_id: Some(topology.faces[0].id),
            complete_hinge_angles,
        };
        let state = AppState::new(project);
        let applied = tauri::async_runtime::block_on(
            crate::applied_pose::apply_current_native_pose(&state, request),
        )
        .expect("production native-pose adoption");
        let applied_encoded = serde_json::to_value(applied).expect("serialize apply response");
        assert_eq!(
            applied_encoded["binding"]["poseGeneration"], "1",
            "the public generation token must not lose u64 precision in JavaScript"
        );
        let expected_pair = {
            let capability = capture_current_pose_capability(&state)
                .expect("capture")
                .expect("current production pose");
            assert_eq!(
                applied.binding,
                CurrentAppliedPoseBindingResponse::from_claims(&capability.claims)
            );
            only_non_hinge_face_pair(capability.claims.native_pose.tree().unwrap().0)
        };
        let expected_binding = applied.binding;

        let error = match certify_current_static_collision(&state, StaticCollisionLimits::default())
        {
            Ok(_) => panic!("proven transversal must not mint certificate C"),
            Err(error) => error,
        };
        assert_eq!(
            error,
            CurrentStaticCollisionError::GeometryBlocking(
                StaticCollisionError::ProvenTransversalPenetration {
                    expected_unordered_face_pairs: 3,
                    proven_transversal_pairs: 1,
                    first_proven_transversal_pair: expected_pair,
                },
            ),
            "a real proven transversal must return before Prepared C or certificate C exists"
        );

        let diagnosis = tauri::async_runtime::block_on(
            inspect_current_static_collision_with_limits(&state, StaticCollisionLimits::default()),
        )
        .expect("redacted production diagnosis");
        assert_eq!(diagnosis.binding, Some(expected_binding));
        assert_eq!(
            diagnosis.status,
            CurrentStaticCollisionDiagnosticStatus::Blocking
        );
        assert_eq!(
            diagnosis.reason,
            Some(CurrentStaticCollisionDiagnosticReason::ProvenTransversalPenetration)
        );
        assert_eq!(diagnosis.expected_unordered_face_pairs, Some(3));
        assert_eq!(diagnosis.proven_transversal_pairs, Some(1));
        assert_eq!(
            diagnosis.first_proven_transversal_pair,
            Some(CurrentStaticCollisionFacePair {
                first_face_id: expected_pair[0],
                second_face_id: expected_pair[1],
            })
        );
        assert_eq!(
            diagnosis.pair_classification_counts,
            Some(CurrentStaticCollisionPairClassificationCounts {
                separated: 0,
                touching: 0,
                allowed: 2,
                penetrating: 1,
                indeterminate: 0,
                candidate_excluded: 0,
            })
        );
        let pair_diagnostics = diagnosis
            .pair_diagnostics
            .as_ref()
            .expect("complete pair diagnostics");
        assert_eq!(pair_diagnostics.len(), 3);
        assert_eq!(
            pair_diagnostics
                .iter()
                .filter(|pair| pair.disposition == "penetrating")
                .map(|pair| [pair.first_face_id, pair.second_face_id])
                .collect::<Vec<_>>(),
            vec![expected_pair]
        );
        assert_eq!(
            pair_diagnostics
                .iter()
                .filter(|pair| pair.disposition == "allowed")
                .count(),
            2
        );
        assert_eq!(
            pair_diagnostics
                .iter()
                .filter(|pair| pair.shared_hinge_boundary_contact_proven)
                .count(),
            2
        );
        let encoded = serde_json::to_value(&diagnosis).expect("serialize diagnosis");
        assert_eq!(encoded["status"], "blocking");
        assert_eq!(encoded["reason"], "proven_zero_thickness_penetration");
        assert_eq!(encoded["expectedUnorderedFacePairs"], 3);
        assert_eq!(encoded["provenPenetratingPairs"], 1);
        assert_eq!(encoded["binding"]["poseGeneration"], "1");
        assert_eq!(
            encoded["firstProvenPenetratingPair"]["firstFaceId"],
            serde_json::to_value(expected_pair[0]).expect("serialize first face")
        );
        assert_eq!(
            encoded["firstProvenPenetratingPair"]["secondFaceId"],
            serde_json::to_value(expected_pair[1]).expect("serialize second face")
        );
        let object = encoded.as_object().expect("diagnosis object");
        assert_eq!(object.len(), 8, "IPC schema must stay narrow and redacted");
        for forbidden in [
            "coordinates",
            "transform",
            "geometryProof",
            "internalError",
            "message",
        ] {
            assert!(
                !object.contains_key(forbidden),
                "raw internal field leaked: {forbidden}"
            );
        }
    }

    #[test]
    fn positive_thickness_mid_surface_transversal_has_a_distinct_redacted_reason() {
        let (project, hinges) = midpoint_mountain_400mm_project_with_thickness(1.0);
        let topology = project
            .editor
            .topology_analysis_input(project.project_id)
            .analyze()
            .simulation_snapshot()
            .expect("three-face midpoint topology")
            .clone();
        let mut complete_hinge_angles = hinges
            .into_iter()
            .map(|edge_id| NativePoseHingeAngleRequest {
                edge_id,
                angle_degrees: 135.0,
            })
            .collect::<Vec<_>>();
        complete_hinge_angles.sort_by_key(|angle| angle.edge_id.canonical_bytes());
        let request = NativePoseRequest {
            expected_project_instance_id: project.instance_id,
            expected_project_id: project.project_id,
            expected_revision: project.editor.revision(),
            fixed_face_id: Some(topology.faces[0].id),
            complete_hinge_angles,
        };
        let state = AppState::new(project);
        let applied = tauri::async_runtime::block_on(
            crate::applied_pose::apply_current_native_pose(&state, request),
        )
        .expect("production native-pose adoption");
        let expected_pair = {
            let capability = capture_current_pose_capability(&state)
                .expect("capture")
                .expect("current production pose");
            only_non_hinge_face_pair(capability.claims.native_pose.tree().unwrap().0)
        };

        let error = match certify_current_static_collision(&state, StaticCollisionLimits::default())
        {
            Ok(_) => panic!("positive-thickness transversal must block certificate C"),
            Err(error) => error,
        };
        assert_eq!(
            error,
            CurrentStaticCollisionError::GeometryBlocking(
                StaticCollisionError::ProvenPositiveThicknessPenetration {
                    expected_unordered_face_pairs: 3,
                    proven_positive_thickness_pairs: 1,
                    first_proven_positive_thickness_pair: expected_pair,
                },
            )
        );

        let diagnosis = tauri::async_runtime::block_on(
            inspect_current_static_collision_with_limits(&state, StaticCollisionLimits::default()),
        )
        .expect("redacted positive-thickness diagnosis");
        assert_eq!(diagnosis.binding, Some(applied.binding));
        assert_eq!(
            diagnosis.status,
            CurrentStaticCollisionDiagnosticStatus::Blocking
        );
        assert_eq!(
            diagnosis.reason,
            Some(CurrentStaticCollisionDiagnosticReason::ProvenPositiveThicknessPenetration)
        );
        assert_eq!(diagnosis.expected_unordered_face_pairs, Some(3));
        assert_eq!(diagnosis.proven_transversal_pairs, Some(1));
        assert_eq!(
            diagnosis.first_proven_transversal_pair,
            Some(CurrentStaticCollisionFacePair {
                first_face_id: expected_pair[0],
                second_face_id: expected_pair[1],
            })
        );
        assert_eq!(
            diagnosis.pair_classification_counts,
            Some(CurrentStaticCollisionPairClassificationCounts {
                separated: 0,
                touching: 0,
                allowed: 0,
                penetrating: 1,
                indeterminate: 2,
                candidate_excluded: 0,
            })
        );
        let pair_diagnostics = diagnosis
            .pair_diagnostics
            .as_ref()
            .expect("complete positive-thickness pair diagnostics");
        assert_eq!(pair_diagnostics.len(), 3);
        assert_eq!(
            pair_diagnostics
                .iter()
                .filter(|pair| pair.disposition == "penetrating")
                .map(|pair| [pair.first_face_id, pair.second_face_id])
                .collect::<Vec<_>>(),
            vec![expected_pair]
        );
        assert_eq!(
            pair_diagnostics
                .iter()
                .filter(|pair| pair.disposition == "indeterminate")
                .count(),
            2
        );
        let encoded = serde_json::to_value(&diagnosis).expect("serialize diagnosis");
        assert_eq!(encoded["reason"], "proven_positive_thickness_penetration");
        assert_eq!(encoded["provenPenetratingPairs"], 1);
        assert_eq!(
            encoded["firstProvenPenetratingPair"]["firstFaceId"],
            serde_json::to_value(expected_pair[0]).expect("serialize first face")
        );
        assert_eq!(
            encoded["firstProvenPenetratingPair"]["secondFaceId"],
            serde_json::to_value(expected_pair[1]).expect("serialize second face")
        );
    }

    #[test]
    fn multi_face_pose_remains_blocking_until_all_pair_evidence_exists() {
        let sheet = create_rectangular_sheet(40.0, 30.0, false).expect("rectangle fixture");
        let (mut pattern, paper) = sheet.into_parts();
        let hinge = EdgeId::new();
        pattern.edges.push(Edge {
            id: hinge,
            start: pattern.vertices[0].id,
            end: pattern.vertices[2].id,
            kind: EdgeKind::Mountain,
        });
        let mut project = ProjectState::new_with_paper(pattern, paper);
        let analysis = project
            .editor
            .topology_analysis_input(project.project_id)
            .analyze();
        let topology = analysis.simulation_snapshot().expect("two-face topology");
        assert_eq!(topology.faces.len(), 2);
        let request = NativePoseRequest {
            expected_project_instance_id: project.instance_id,
            expected_project_id: project.project_id,
            expected_revision: project.editor.revision(),
            fixed_face_id: Some(topology.faces[0].id),
            complete_hinge_angles: vec![NativePoseHingeAngleRequest {
                edge_id: hinge,
                angle_degrees: 90.0,
            }],
        };
        adopt_request(&mut project, request);
        let state = AppState::new(project);
        let binding = current_binding(&state);

        assert!(matches!(
            certify_current_static_collision(&state, StaticCollisionLimits::default()),
            Err(CurrentStaticCollisionError::GeometryBlocking(
                StaticCollisionError::PairEvidenceUnavailable {
                    expected_unordered_face_pairs: 1,
                },
            ))
        ));

        let diagnosis = tauri::async_runtime::block_on(
            inspect_current_static_collision_with_limits(&state, StaticCollisionLimits::default()),
        )
        .expect("evidence-unavailable diagnosis");
        assert_eq!(diagnosis.binding, Some(binding));
        assert_eq!(
            diagnosis.status,
            CurrentStaticCollisionDiagnosticStatus::Blocking
        );
        assert_eq!(
            diagnosis.reason,
            Some(CurrentStaticCollisionDiagnosticReason::EvidenceUnavailable)
        );
        assert_eq!(diagnosis.expected_unordered_face_pairs, Some(1));
        assert_eq!(diagnosis.proven_transversal_pairs, None);
        assert_eq!(diagnosis.first_proven_transversal_pair, None);
        assert_eq!(
            diagnosis.pair_classification_counts,
            Some(CurrentStaticCollisionPairClassificationCounts {
                separated: 0,
                touching: 0,
                allowed: 0,
                penetrating: 0,
                indeterminate: 1,
                candidate_excluded: 0,
            })
        );
        let pair_diagnostics = diagnosis
            .pair_diagnostics
            .expect("unresolved pair must still be visible");
        assert_eq!(pair_diagnostics.len(), 1);
        assert_eq!(pair_diagnostics[0].topology, "shared_hinge_edge");
        assert_eq!(pair_diagnostics[0].disposition, "indeterminate");
        assert!(!pair_diagnostics[0].shared_hinge_boundary_contact_proven);
        assert!(pair_diagnostics[0].shared_hinge_solid_classified);
    }

    #[test]
    fn production_diagnosis_only_reports_safe_after_c_mint_and_fails_closed_otherwise() {
        let mut safe_project = no_hinge_project();
        adopt_no_hinge_pose(&mut safe_project);
        let safe_state = AppState::new(safe_project);
        let safe_binding = current_binding(&safe_state);
        assert_eq!(
            tauri::async_runtime::block_on(inspect_current_static_collision_with_limits(
                &safe_state,
                StaticCollisionLimits::default(),
            ))
            .expect("certified diagnosis"),
            CurrentStaticCollisionDiagnosticResponse {
                binding: Some(safe_binding),
                status: CurrentStaticCollisionDiagnosticStatus::CertifiedNonblocking,
                reason: None,
                expected_unordered_face_pairs: Some(0),
                proven_transversal_pairs: Some(0),
                first_proven_transversal_pair: None,
                pair_classification_counts: Some(CurrentStaticCollisionPairClassificationCounts {
                    separated: 0,
                    touching: 0,
                    allowed: 0,
                    penetrating: 0,
                    indeterminate: 0,
                    candidate_excluded: 0,
                },),
                pair_diagnostics: Some(Vec::new()),
            }
        );

        assert_eq!(
            tauri::async_runtime::block_on(inspect_current_static_collision_with_limits(
                &safe_state,
                StaticCollisionLimits {
                    max_faces: 0,
                    ..StaticCollisionLimits::default()
                },
            ))
            .expect("resource diagnosis"),
            CurrentStaticCollisionDiagnosticResponse {
                binding: Some(safe_binding),
                status: CurrentStaticCollisionDiagnosticStatus::Blocking,
                reason: Some(CurrentStaticCollisionDiagnosticReason::ResourceLimitExceeded),
                expected_unordered_face_pairs: None,
                proven_transversal_pairs: None,
                first_proven_transversal_pair: None,
                pair_classification_counts: None,
                pair_diagnostics: None,
            },
            "resource exhaustion must never become a safe success"
        );

        let unavailable_state = AppState::new(no_hinge_project());
        assert_eq!(
            tauri::async_runtime::block_on(inspect_current_static_collision_with_limits(
                &unavailable_state,
                StaticCollisionLimits::default(),
            ))
            .expect("unavailable diagnosis"),
            CurrentStaticCollisionDiagnosticResponse {
                binding: None,
                status: CurrentStaticCollisionDiagnosticStatus::Unavailable,
                reason: Some(CurrentStaticCollisionDiagnosticReason::PoseAuthorityUnavailable),
                expected_unordered_face_pairs: None,
                proven_transversal_pairs: None,
                first_proven_transversal_pair: None,
                pair_classification_counts: None,
                pair_diagnostics: None,
            }
        );

        assert_eq!(
            diagnostic_response_from_error(
                CurrentStaticCollisionError::InternalInconsistency,
                Some(safe_binding),
            )
            .expect("inconsistent diagnosis"),
            CurrentStaticCollisionDiagnosticResponse {
                binding: Some(safe_binding),
                status: CurrentStaticCollisionDiagnosticStatus::Blocking,
                reason: Some(CurrentStaticCollisionDiagnosticReason::InconsistentState),
                expected_unordered_face_pairs: None,
                proven_transversal_pairs: None,
                first_proven_transversal_pair: None,
                pair_classification_counts: None,
                pair_diagnostics: None,
            },
            "internal inconsistency must remain blocking"
        );
        assert_eq!(
            diagnostic_response_from_error(
                CurrentStaticCollisionError::PoseAuthorityUnavailable,
                Some(safe_binding),
            )
            .expect("stale pose diagnosis"),
            CurrentStaticCollisionDiagnosticResponse {
                binding: None,
                status: CurrentStaticCollisionDiagnosticStatus::Unavailable,
                reason: Some(CurrentStaticCollisionDiagnosticReason::PoseAuthorityUnavailable),
                expected_unordered_face_pairs: None,
                proven_transversal_pairs: None,
                first_proven_transversal_pair: None,
                pair_classification_counts: None,
                pair_diagnostics: None,
            },
            "an unavailable result must not claim that a stale pose binding is current"
        );
        assert_eq!(
            diagnostic_response_from_error(CurrentStaticCollisionError::LockUnavailable, None),
            Err(CURRENT_STATIC_COLLISION_DIAGNOSTIC_FAILED_MESSAGE.to_owned()),
            "operational errors expose one fixed sanitized message"
        );
    }
}
