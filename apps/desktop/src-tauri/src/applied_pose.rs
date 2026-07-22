//! Native authority for the currently applied material pose.
//!
//! The certificate implemented here proves only native tree kinematics (A)
//! and current binding (B). It deliberately does not certify static or
//! continuous collision safety, layer transport, or permission to execute
//! SIM-010.

#![allow(
    dead_code,
    reason = "A/B native authority is intentionally sealed before its IPC and collision consumers"
)]

mod static_collision;

// C remains sealed from mutation authority. The read-only production command
// receives only the redacted DTO below.
#[allow(unused_imports)]
pub(super) use static_collision::{
    CurrentStaticCollisionCertificate, CurrentStaticCollisionDiagnosticResponse,
    CurrentStaticCollisionError, CurrentStaticCollisionView, certify_current_static_collision,
    inspect_current_static_collision, with_revalidated_current_static_collision_certificate,
};

use std::{
    error::Error,
    fmt,
    sync::{Arc, Mutex, MutexGuard},
};

use ori_collision::{CENTERED_MID_SURFACE_THICKNESS_MODEL_V1, TOPOLOGY_CONTACT_POLICY_V2};
use ori_core::{
    APPLIED_POSE_MODEL_ID_V1, AppliedPoseLimitsV1, AppliedPoseV1,
    CLOSED_GRAPH_APPLIED_POSE_MODEL_ID_V1, TopologyAnalysisInput, TopologySnapshot,
    prepare_applied_pose_v1, prepare_closed_graph_applied_pose_v1,
};
use ori_domain::{EdgeId, FaceId, InstructionPose, InstructionPoseModel, ProjectId};
use ori_kinematics::{
    CanonicalHingeAngles, HingeAngle, MATERIAL_TREE_KINEMATICS_MODEL_ID,
    MaterialTreeKinematicsModel, MaterialTreePose, TreeKinematicsLimits,
};
use serde::{Deserialize, Serialize, Serializer};

use super::{AppState, ProjectState, lock_project};

/// A strictly limited, untrusted request for one complete native tree pose.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct NativePoseRequest {
    pub(super) expected_project_instance_id: ProjectId,
    pub(super) expected_project_id: ProjectId,
    pub(super) expected_revision: u64,
    pub(super) fixed_face_id: Option<FaceId>,
    pub(super) complete_hinge_angles: Vec<NativePoseHingeAngleRequest>,
}

/// One untrusted angle record. Unknown transform-like fields are rejected at
/// this nested boundary as well as on [`NativePoseRequest`].
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct NativePoseHingeAngleRequest {
    pub(super) edge_id: EdgeId,
    pub(super) angle_degrees: f64,
}

/// Stable identity of one adopted native pose. The frontend uses this binding
/// to reject a diagnosis returned for an older concurrent apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CurrentAppliedPoseBindingResponse {
    project_instance_id: ProjectId,
    project_id: ProjectId,
    revision: u64,
    #[serde(serialize_with = "serialize_u64_decimal")]
    pose_generation: u64,
}

/// Redacted success response for the production native-pose adoption command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ApplyCurrentNativePoseResponse {
    binding: CurrentAppliedPoseBindingResponse,
}

const APPLY_CURRENT_NATIVE_POSE_FAILED_MESSAGE: &str =
    "現在の3D姿勢を適用できませんでした。もう一度実行してください。";

fn serialize_u64_decimal<S>(value: &u64, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.collect_str(value)
}

/// Fixed-category failure at the native applied-pose authority boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PoseAuthorityError {
    LockUnavailable,
    InvalidRequest,
    StaleRequest,
    TopologyUnavailable,
    KinematicsUnavailable,
    SemanticPoseUnavailable,
    GenerationExhausted,
    WrongAuthority,
    ReplacementAuthorityNotFresh,
    InternalInconsistency,
}

impl fmt::Display for PoseAuthorityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::LockUnavailable => "the current applied-pose authority is unavailable",
            Self::InvalidRequest => "the native pose request is invalid",
            Self::StaleRequest => "the native pose request is stale",
            Self::TopologyUnavailable => "a simulation-ready topology is unavailable",
            Self::KinematicsUnavailable => "native material kinematics could not be prepared",
            Self::SemanticPoseUnavailable => "the semantic applied pose could not be prepared",
            Self::GenerationExhausted => "the current applied-pose generation is exhausted",
            Self::WrongAuthority => "the applied-pose authority belongs to another project slot",
            Self::ReplacementAuthorityNotFresh => {
                "the replacement project does not have a fresh applied-pose authority"
            }
            Self::InternalInconsistency => {
                "the current applied-pose authority failed an internal consistency check"
            }
        })
    }
}

impl Error for PoseAuthorityError {}

/// Project-subordinate native authority. Clones share one private slot.
#[derive(Clone, Default)]
pub(super) struct CurrentAppliedPoseAuthority(Arc<Mutex<CurrentAppliedPoseSlot>>);

#[derive(Default)]
struct CurrentAppliedPoseSlot {
    generation: u64,
    current: Option<Arc<CurrentAppliedPoseCertificate>>,
    pending: Option<PendingNativePoseRequest>,
}

#[derive(Clone)]
struct PoseSourceBinding {
    request_id: ProjectId,
    project_instance_id: ProjectId,
    project_id: ProjectId,
    revision: u64,
    topology_input: Arc<TopologyAnalysisInput>,
    fold_model_fingerprint: Arc<str>,
    paper_thickness_bits: u64,
}

struct PendingNativePoseRequest {
    request_id: ProjectId,
    binding: Arc<PoseSourceBinding>,
}

/// Immutable data captured under the project lock and safe to prepare after
/// that lock has been released.
pub(super) struct CapturedNativePoseRequest {
    slot: Arc<Mutex<CurrentAppliedPoseSlot>>,
    binding: Arc<PoseSourceBinding>,
    fixed_face: Option<FaceId>,
    complete_hinge_angles: Vec<(EdgeId, f64)>,
}

/// Clears only this request's pending marker if lock-free preparation fails,
/// unwinds, or its prepared output is abandoned before commit begins. A newer
/// request is never removed.
struct PendingPreparationCleanup {
    slot: Arc<Mutex<CurrentAppliedPoseSlot>>,
    binding: Arc<PoseSourceBinding>,
    armed: bool,
}

impl PendingPreparationCleanup {
    fn new(slot: &Arc<Mutex<CurrentAppliedPoseSlot>>, binding: &Arc<PoseSourceBinding>) -> Self {
        Self {
            slot: Arc::clone(slot),
            binding: Arc::clone(binding),
            armed: true,
        }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for PendingPreparationCleanup {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let Ok(mut slot) = self.slot.lock() else {
            return;
        };
        if slot.pending.as_ref().is_some_and(|pending| {
            pending.request_id == self.binding.request_id
                && Arc::ptr_eq(&pending.binding, &self.binding)
        }) {
            slot.pending = None;
        }
    }
}

/// Pure worker output. Possession is not authority to adopt it.
pub(super) struct PreparedNativePose {
    slot: Arc<Mutex<CurrentAppliedPoseSlot>>,
    binding: Arc<PoseSourceBinding>,
    pending_cleanup: PendingPreparationCleanup,
    topology: Arc<TopologySnapshot>,
    semantic_pose: Arc<AppliedPoseV1>,
    native_pose: CurrentNativeMaterialPose,
}

#[derive(Clone)]
struct CurrentAppliedPoseClaims {
    request_id: ProjectId,
    project_instance_id: ProjectId,
    project_id: ProjectId,
    revision: u64,
    topology_input: Arc<TopologyAnalysisInput>,
    topology: Arc<TopologySnapshot>,
    fold_model_fingerprint: Arc<str>,
    semantic_pose: Arc<AppliedPoseV1>,
    native_pose: CurrentNativeMaterialPose,
    material_faces: Arc<[FaceId]>,
    material_hinges: Arc<[EdgeId]>,
    paper_thickness_bits: u64,
    kinematics_model_id: &'static str,
    semantic_model_id: &'static str,
    thickness_model_id: &'static str,
    contact_policy_id: &'static str,
    generation: u64,
}

#[derive(Clone)]
enum CurrentNativeMaterialPose {
    Tree {
        model: Arc<MaterialTreeKinematicsModel>,
        pose: Arc<MaterialTreePose>,
        graph_geometry: Option<Arc<ori_kinematics::MaterialHingeGraphGeometry>>,
        graph_audit: Option<Arc<ori_kinematics::MaterialHingeGraphAudit>>,
        graph_pose: Option<Arc<ori_kinematics::ClosedMaterialHingeGraphPose>>,
    },
    Graph {
        geometry: Arc<ori_kinematics::MaterialHingeGraphGeometry>,
        audit: Arc<ori_kinematics::MaterialHingeGraphAudit>,
        pose: Arc<ori_kinematics::ClosedMaterialHingeGraphPose>,
    },
}

impl CurrentNativeMaterialPose {
    fn tree(&self) -> Option<(&MaterialTreeKinematicsModel, &MaterialTreePose)> {
        match self {
            Self::Tree { model, pose, .. } => Some((model.as_ref(), pose.as_ref())),
            Self::Graph { .. } => None,
        }
    }

    fn graph(
        &self,
    ) -> Option<(
        &ori_kinematics::MaterialHingeGraphGeometry,
        &ori_kinematics::MaterialHingeGraphAudit,
        &ori_kinematics::ClosedMaterialHingeGraphPose,
    )> {
        match self {
            Self::Tree {
                graph_geometry,
                graph_audit,
                graph_pose,
                ..
            } => Some((
                graph_geometry.as_ref()?.as_ref(),
                graph_audit.as_ref()?.as_ref(),
                graph_pose.as_ref()?.as_ref(),
            )),
            Self::Graph {
                geometry,
                audit,
                pose,
            } => Some((geometry.as_ref(), audit.as_ref(), pose.as_ref())),
        }
    }

    fn same_native_instance(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::Tree {
                    model: first_model,
                    pose: first_pose,
                    graph_geometry: first_graph_geometry,
                    graph_audit: first_graph_audit,
                    graph_pose: first_graph_pose,
                },
                Self::Tree {
                    model: second_model,
                    pose: second_pose,
                    graph_geometry: second_graph_geometry,
                    graph_audit: second_graph_audit,
                    graph_pose: second_graph_pose,
                },
            ) => {
                Arc::ptr_eq(first_model, second_model)
                    && Arc::ptr_eq(first_pose, second_pose)
                    && match (first_graph_geometry, second_graph_geometry) {
                        (Some(first), Some(second)) => Arc::ptr_eq(first, second),
                        (None, None) => true,
                        _ => false,
                    }
                    && match (first_graph_audit, second_graph_audit) {
                        (Some(first), Some(second)) => Arc::ptr_eq(first, second),
                        (None, None) => true,
                        _ => false,
                    }
                    && match (first_graph_pose, second_graph_pose) {
                        (Some(first), Some(second)) => Arc::ptr_eq(first, second),
                        (None, None) => true,
                        _ => false,
                    }
                    && first_pose.same_instance(second_pose)
            }
            (
                Self::Graph {
                    geometry: first_geometry,
                    audit: first_audit,
                    pose: first_pose,
                },
                Self::Graph {
                    geometry: second_geometry,
                    audit: second_audit,
                    pose: second_pose,
                },
            ) => {
                Arc::ptr_eq(first_geometry, second_geometry)
                    && Arc::ptr_eq(first_audit, second_audit)
                    && Arc::ptr_eq(first_pose, second_pose)
            }
            _ => false,
        }
    }
}

struct CurrentAppliedPoseCertificate {
    binding: Arc<PoseSourceBinding>,
    claims: CurrentAppliedPoseClaims,
}

/// Opaque in-process evidence that one A certificate is still current (B).
///
/// This value is intentionally neither serializable nor clonable. It must not
/// be interpreted as collision, path, layer-order, or SIM-010 authority.
pub(super) struct CurrentAppliedPoseCapability {
    slot: Arc<Mutex<CurrentAppliedPoseSlot>>,
    certificate: Arc<CurrentAppliedPoseCertificate>,
    claims: CurrentAppliedPoseClaims,
}

pub(super) struct CurrentAppliedPoseCommitGuard<'a> {
    _slot: MutexGuard<'a, CurrentAppliedPoseSlot>,
}

pub(super) fn lock_revalidated_current_applied_pose_for_commit<'a>(
    project: &ProjectState,
    capability: &'a CurrentAppliedPoseCapability,
) -> Result<Option<CurrentAppliedPoseCommitGuard<'a>>, PoseAuthorityError> {
    let authority = project.applied_pose_authority.clone();
    if !Arc::ptr_eq(&authority.0, &capability.slot) {
        return Ok(None);
    }
    let slot = capability
        .slot
        .lock()
        .map_err(|_| PoseAuthorityError::LockUnavailable)?;
    let Some(current) = slot.current.as_ref() else {
        return Ok(None);
    };
    if !current_applied_pose_capability_matches_locked_slot(&slot, project, capability, current) {
        return Ok(None);
    }
    Ok(Some(CurrentAppliedPoseCommitGuard { _slot: slot }))
}

impl CurrentAppliedPoseCapability {
    #[must_use]
    pub(super) fn tree(&self) -> Option<(&MaterialTreeKinematicsModel, &MaterialTreePose)> {
        self.claims.native_pose.tree()
    }

    /// Observation-only access for detached native analysis. The caller must
    /// revalidate this capability against the live project before publishing
    /// any result.
    #[must_use]
    pub(super) fn model(&self) -> &MaterialTreeKinematicsModel {
        self.claims
            .native_pose
            .tree()
            .expect("tree pose capability")
            .0
    }

    #[must_use]
    pub(super) fn pose(&self) -> &MaterialTreePose {
        self.claims
            .native_pose
            .tree()
            .expect("tree pose capability")
            .1
    }

    #[must_use]
    pub(super) fn graph(
        &self,
    ) -> Option<(
        &ori_kinematics::MaterialHingeGraphGeometry,
        &ori_kinematics::MaterialHingeGraphAudit,
        &ori_kinematics::ClosedMaterialHingeGraphPose,
    )> {
        self.claims.native_pose.graph()
    }

    #[must_use]
    pub(super) const fn generation(&self) -> u64 {
        self.claims.generation
    }
}

/// Read-only access to the sealed native material pose.
pub(super) struct CurrentAppliedPoseView<'a> {
    certificate: &'a CurrentAppliedPoseCertificate,
}

impl CurrentAppliedPoseView<'_> {
    #[must_use]
    pub(super) fn semantic_pose(&self) -> &AppliedPoseV1 {
        self.certificate.claims.semantic_pose.as_ref()
    }

    #[must_use]
    pub(super) fn model(&self) -> &MaterialTreeKinematicsModel {
        self.certificate
            .claims
            .native_pose
            .tree()
            .expect("tree pose view")
            .0
    }

    #[must_use]
    pub(super) fn pose(&self) -> &MaterialTreePose {
        self.certificate
            .claims
            .native_pose
            .tree()
            .expect("tree pose view")
            .1
    }

    #[must_use]
    pub(super) fn graph(
        &self,
    ) -> Option<(
        &ori_kinematics::MaterialHingeGraphGeometry,
        &ori_kinematics::MaterialHingeGraphAudit,
        &ori_kinematics::ClosedMaterialHingeGraphPose,
    )> {
        self.certificate.claims.native_pose.graph()
    }

    #[must_use]
    pub(super) const fn generation(&self) -> u64 {
        self.certificate.claims.generation
    }

    #[must_use]
    pub(super) const fn paper_thickness_bits(&self) -> u64 {
        self.certificate.claims.paper_thickness_bits
    }

    #[must_use]
    pub(super) const fn thickness_model_id(&self) -> &'static str {
        self.certificate.claims.thickness_model_id
    }

    #[must_use]
    pub(super) const fn contact_policy_id(&self) -> &'static str {
        self.certificate.claims.contact_policy_id
    }
}

impl CurrentAppliedPoseBindingResponse {
    fn from_claims(claims: &CurrentAppliedPoseClaims) -> Self {
        Self {
            project_instance_id: claims.project_instance_id,
            project_id: claims.project_id,
            revision: claims.revision,
            pose_generation: claims.generation,
        }
    }
}

/// Captures an untrusted complete-pose request under the project lock,
/// performs topology and kinematics work in a blocking worker, then adopts it
/// only if the exact project instance and revision are still current.
///
/// This command owns the process-wide native-pose worker permit through its
/// complete capture/prepare/commit transaction. The separate
/// apply-response-to-inspection transaction is serialized by the frontend
/// coordinator because it necessarily spans two IPC commands.
pub(crate) async fn apply_current_native_pose(
    app_state: &AppState,
    request: NativePoseRequest,
) -> Result<ApplyCurrentNativePoseResponse, String> {
    let permit = app_state
        .try_acquire_native_pose_worker()
        .ok_or(APPLY_CURRENT_NATIVE_POSE_FAILED_MESSAGE)?;
    let (authority, captured) = {
        let project =
            lock_project(app_state).map_err(|_| APPLY_CURRENT_NATIVE_POSE_FAILED_MESSAGE)?;
        let authority = project.applied_pose_authority.clone();
        let captured = authority
            .capture_request(&project, request)
            .map_err(|_| APPLY_CURRENT_NATIVE_POSE_FAILED_MESSAGE)?;
        (authority, captured)
    };
    let (permit, prepared) =
        tauri::async_runtime::spawn_blocking(move || (permit, captured.prepare()))
            .await
            .map_err(|_| APPLY_CURRENT_NATIVE_POSE_FAILED_MESSAGE)?;
    let prepared = prepared.map_err(|_| APPLY_CURRENT_NATIVE_POSE_FAILED_MESSAGE)?;

    let mut project =
        lock_project(app_state).map_err(|_| APPLY_CURRENT_NATIVE_POSE_FAILED_MESSAGE)?;
    let capability = authority
        .commit_prepared(&mut project, prepared)
        .map_err(|_| APPLY_CURRENT_NATIVE_POSE_FAILED_MESSAGE)?;
    let response = ApplyCurrentNativePoseResponse {
        binding: CurrentAppliedPoseBindingResponse::from_claims(&capability.claims),
    };
    drop(project);
    drop(permit);
    Ok(response)
}

pub(super) fn restore_persisted_current_pose(
    project: &mut ProjectState,
    pose: &InstructionPose,
) -> Result<(), PoseAuthorityError> {
    if pose.model != InstructionPoseModel::AbsoluteHingeAnglesV1
        || pose.source_model_fingerprint != project.editor.fold_model_fingerprint_v1()
    {
        return Err(PoseAuthorityError::InvalidRequest);
    }
    let request = NativePoseRequest {
        expected_project_instance_id: project.instance_id,
        expected_project_id: project.project_id,
        expected_revision: project.editor.revision(),
        fixed_face_id: pose.fixed_face,
        complete_hinge_angles: pose
            .hinge_angles
            .iter()
            .map(|hinge| NativePoseHingeAngleRequest {
                edge_id: hinge.edge,
                angle_degrees: hinge.angle_degrees,
            })
            .collect(),
    };
    let authority = project.applied_pose_authority.clone();
    let captured = authority.capture_request(project, request)?;
    let prepared = captured.prepare()?;
    authority.commit_prepared(project, prepared)?;
    Ok(())
}

impl CurrentAppliedPoseAuthority {
    /// Captures and registers the latest native pose request.
    ///
    /// The caller must already hold the containing project lock. No topology
    /// or kinematics work is performed while the authority slot is locked.
    pub(super) fn capture_request(
        &self,
        project: &ProjectState,
        mut request: NativePoseRequest,
    ) -> Result<CapturedNativePoseRequest, PoseAuthorityError> {
        if !self.is_project_authority(project) {
            return Err(PoseAuthorityError::WrongAuthority);
        }
        if request.expected_project_instance_id != project.instance_id
            || request.expected_project_id != project.project_id
            || request.expected_revision != project.editor.revision()
        {
            return Err(PoseAuthorityError::StaleRequest);
        }
        let complete_hinge_angles = validate_and_normalize_request(&mut request)?;

        let topology_input = Arc::new(project.editor.topology_analysis_input(project.project_id));
        let fold_model_fingerprint: Arc<str> =
            Arc::from(project.editor.fold_model_fingerprint_v1());
        let binding = Arc::new(PoseSourceBinding {
            request_id: ProjectId::new(),
            project_instance_id: project.instance_id,
            project_id: project.project_id,
            revision: project.editor.revision(),
            topology_input,
            fold_model_fingerprint,
            paper_thickness_bits: project.editor.paper().thickness_mm.to_bits(),
        });

        let mut slot = self.lock()?;
        slot.generation
            .checked_add(1)
            .ok_or(PoseAuthorityError::GenerationExhausted)?;
        if let Some(current) = slot.current.as_ref()
            && !current_applied_pose_certificate_is_internally_consistent(current)
        {
            return Err(PoseAuthorityError::InternalInconsistency);
        }
        slot.pending = Some(PendingNativePoseRequest {
            request_id: binding.request_id,
            binding: Arc::clone(&binding),
        });
        Ok(CapturedNativePoseRequest {
            slot: Arc::clone(&self.0),
            binding,
            fixed_face: request.fixed_face_id,
            complete_hinge_angles,
        })
    }

    /// Adopts only the latest still-current prepared request.
    ///
    /// Semantic pose adoption, generation advance, pending removal, and
    /// certificate publication happen while the caller holds the project lock
    /// and this authority lock. Every fallible check precedes those mutations.
    pub(super) fn commit_prepared(
        &self,
        project: &mut ProjectState,
        prepared: PreparedNativePose,
    ) -> Result<CurrentAppliedPoseCapability, PoseAuthorityError> {
        self.commit_prepared_with_semantic_clone(project, prepared, |semantic| {
            semantic
                .try_clone()
                .map_err(|_| PoseAuthorityError::SemanticPoseUnavailable)
        })
    }

    fn commit_prepared_with_semantic_clone(
        &self,
        project: &mut ProjectState,
        mut prepared: PreparedNativePose,
        clone_semantic: impl FnOnce(&AppliedPoseV1) -> Result<AppliedPoseV1, PoseAuthorityError>,
    ) -> Result<CurrentAppliedPoseCapability, PoseAuthorityError> {
        // From this point onward every failure deliberately preserves the
        // pending marker and all other live state exactly as it was on entry.
        prepared.pending_cleanup.disarm();
        if !self.is_project_authority(project) || !Arc::ptr_eq(&self.0, &prepared.slot) {
            return Err(PoseAuthorityError::WrongAuthority);
        }
        if !prepared_native_pose_is_internally_consistent(&prepared) {
            return Err(PoseAuthorityError::InternalInconsistency);
        }
        if !binding_is_current(&prepared.binding, project) {
            return Err(PoseAuthorityError::StaleRequest);
        }
        let semantic_for_editor = clone_semantic(prepared.semantic_pose.as_ref())?;
        if !semantic_pose_bits_equal(&semantic_for_editor, &prepared.semantic_pose) {
            return Err(PoseAuthorityError::InternalInconsistency);
        }

        let mut slot = self.lock()?;
        let Some(pending) = slot.pending.as_ref() else {
            return Err(PoseAuthorityError::StaleRequest);
        };
        if pending.request_id != prepared.binding.request_id
            || !Arc::ptr_eq(&pending.binding, &prepared.binding)
            || !binding_claims_equal(&pending.binding, &prepared.binding)
        {
            return Err(PoseAuthorityError::StaleRequest);
        }
        let generation = slot
            .generation
            .checked_add(1)
            .ok_or(PoseAuthorityError::GenerationExhausted)?;

        let (face_ids, hinge_ids, kinematics_model_id, semantic_model_id) =
            match &prepared.native_pose {
                CurrentNativeMaterialPose::Tree { model, .. } => (
                    model.face_ids().to_vec(),
                    model
                        .hinges()
                        .iter()
                        .map(|hinge| hinge.edge())
                        .collect::<Vec<_>>(),
                    MATERIAL_TREE_KINEMATICS_MODEL_ID,
                    APPLIED_POSE_MODEL_ID_V1,
                ),
                CurrentNativeMaterialPose::Graph { geometry, .. } => (
                    geometry.face_ids().to_vec(),
                    geometry
                        .hinges()
                        .iter()
                        .map(|hinge| hinge.edge())
                        .collect::<Vec<_>>(),
                    "material_hinge_graph_pose_v1",
                    CLOSED_GRAPH_APPLIED_POSE_MODEL_ID_V1,
                ),
            };
        let mut material_faces = Vec::new();
        material_faces
            .try_reserve_exact(face_ids.len())
            .map_err(|_| PoseAuthorityError::SemanticPoseUnavailable)?;
        material_faces.extend_from_slice(&face_ids);
        let material_faces: Arc<[FaceId]> = Arc::from(material_faces.into_boxed_slice());
        let mut material_hinges = Vec::new();
        material_hinges
            .try_reserve_exact(hinge_ids.len())
            .map_err(|_| PoseAuthorityError::SemanticPoseUnavailable)?;
        material_hinges.extend(hinge_ids);
        let material_hinges: Arc<[EdgeId]> = Arc::from(material_hinges.into_boxed_slice());
        let claims = CurrentAppliedPoseClaims {
            request_id: prepared.binding.request_id,
            project_instance_id: prepared.binding.project_instance_id,
            project_id: prepared.binding.project_id,
            revision: prepared.binding.revision,
            topology_input: Arc::clone(&prepared.binding.topology_input),
            topology: Arc::clone(&prepared.topology),
            fold_model_fingerprint: Arc::clone(&prepared.binding.fold_model_fingerprint),
            semantic_pose: Arc::clone(&prepared.semantic_pose),
            native_pose: prepared.native_pose.clone(),
            material_faces,
            material_hinges,
            paper_thickness_bits: prepared.binding.paper_thickness_bits,
            kinematics_model_id,
            semantic_model_id,
            thickness_model_id: CENTERED_MID_SURFACE_THICKNESS_MODEL_V1,
            contact_policy_id: TOPOLOGY_CONTACT_POLICY_V2,
            generation,
        };
        let certificate = Arc::new(CurrentAppliedPoseCertificate {
            binding: Arc::clone(&prepared.binding),
            claims,
        });
        if !current_applied_pose_certificate_is_internally_consistent(&certificate) {
            return Err(PoseAuthorityError::InternalInconsistency);
        }
        let capability_claims = certificate.claims.clone();

        project
            .editor
            .adopt_current_applied_pose(semantic_for_editor);
        slot.generation = generation;
        slot.pending = None;
        slot.current = Some(Arc::clone(&certificate));

        Ok(CurrentAppliedPoseCapability {
            slot: Arc::clone(&self.0),
            certificate,
            claims: capability_claims,
        })
    }

    /// Captures a private capability only when the certificate is current.
    pub(super) fn capture_capability(
        &self,
        project: &ProjectState,
    ) -> Result<Option<CurrentAppliedPoseCapability>, PoseAuthorityError> {
        if !self.is_project_authority(project) {
            return Ok(None);
        }
        let slot = self.lock()?;
        let Some(certificate) = slot.current.as_ref() else {
            return Ok(None);
        };
        if certificate.claims.generation != slot.generation
            || !current_applied_pose_certificate_is_internally_consistent(certificate)
            || !current_applied_pose_certificate_is_current(certificate, project)
        {
            return Ok(None);
        }
        Ok(Some(CurrentAppliedPoseCapability {
            slot: Arc::clone(&self.0),
            certificate: Arc::clone(certificate),
            claims: certificate.claims.clone(),
        }))
    }

    /// Observation-only revalidation. Mutation must use the guarded closure.
    pub(super) fn revalidate_capability<'a>(
        &self,
        project: &ProjectState,
        capability: &'a CurrentAppliedPoseCapability,
    ) -> Result<Option<CurrentAppliedPoseView<'a>>, PoseAuthorityError> {
        if !self.is_project_authority(project) || !Arc::ptr_eq(&self.0, &capability.slot) {
            return Ok(None);
        }
        let slot = self.lock()?;
        let Some(current) = slot.current.as_ref() else {
            return Ok(None);
        };
        if !current_applied_pose_capability_matches_locked_slot(&slot, project, capability, current)
        {
            return Ok(None);
        }
        Ok(Some(CurrentAppliedPoseView {
            certificate: capability.certificate.as_ref(),
        }))
    }

    /// Preflights one generation advance and holds the authority lock.
    ///
    /// Dropping the returned guard changes nothing. `commit` is deliberately
    /// the only operation that invalidates the current/pending authority.
    pub(super) fn begin_invalidation(
        &self,
    ) -> Result<PoseAuthorityInvalidation<'_>, PoseAuthorityError> {
        let slot = self.lock()?;
        let next_generation = slot
            .generation
            .checked_add(1)
            .ok_or(PoseAuthorityError::GenerationExhausted)?;
        Ok(PoseAuthorityInvalidation {
            slot,
            next_generation,
        })
    }

    fn is_project_authority(&self, project: &ProjectState) -> bool {
        Arc::ptr_eq(&self.0, &project.applied_pose_authority.0)
    }

    fn lock(&self) -> Result<MutexGuard<'_, CurrentAppliedPoseSlot>, PoseAuthorityError> {
        self.0
            .lock()
            .map_err(|_| PoseAuthorityError::LockUnavailable)
    }

    #[cfg(test)]
    pub(super) fn test_snapshot(
        &self,
    ) -> Result<CurrentAppliedPoseAuthoritySnapshot, PoseAuthorityError> {
        let slot = self.lock()?;
        Ok(CurrentAppliedPoseAuthoritySnapshot {
            generation: slot.generation,
            has_current: slot.current.is_some(),
            has_pending: slot.pending.is_some(),
        })
    }

    #[cfg(test)]
    pub(super) fn set_generation_for_test(
        &self,
        generation: u64,
    ) -> Result<(), PoseAuthorityError> {
        self.lock()?.generation = generation;
        Ok(())
    }
}

impl CapturedNativePoseRequest {
    /// Performs topology, material kinematics, and semantic preparation
    /// without holding the live authority slot. Failure, unwind, or abandoning
    /// the prepared output before commit clears only this request's
    /// still-current pending marker.
    pub(super) fn prepare(self) -> Result<PreparedNativePose, PoseAuthorityError> {
        let pending_cleanup = PendingPreparationCleanup::new(&self.slot, &self.binding);
        let analysis = self.binding.topology_input.analyze();
        let topology = analysis
            .simulation_snapshot()
            .cloned()
            .ok_or(PoseAuthorityError::TopologyUnavailable)?;
        let topology = Arc::new(topology);
        let tree_model = MaterialTreeKinematicsModel::prepare(
            self.binding.topology_input.pattern(),
            self.binding.topology_input.paper(),
            &topology,
            TreeKinematicsLimits::default(),
        );

        let mut native_angles = Vec::new();
        native_angles
            .try_reserve_exact(self.complete_hinge_angles.len())
            .map_err(|_| PoseAuthorityError::KinematicsUnavailable)?;
        for &(edge, angle_degrees) in &self.complete_hinge_angles {
            native_angles.push(
                HingeAngle::new(edge, angle_degrees)
                    .map_err(|_| PoseAuthorityError::InvalidRequest)?,
            );
        }
        let canonical_angles = CanonicalHingeAngles::new(native_angles)
            .map_err(|_| PoseAuthorityError::InvalidRequest)?;
        let (native_pose, face_ids, expected_hinges, semantic_fixed_face) =
            if let Ok(model) = tree_model {
                let model = Arc::new(model);
                let pose = Arc::new(
                    model
                        .solve(self.fixed_face, &canonical_angles)
                        .map_err(|_| PoseAuthorityError::KinematicsUnavailable)?,
                );
                model
                    .bind_pose(&pose)
                    .map_err(|_| PoseAuthorityError::InternalInconsistency)?;
                let hinges = model
                    .hinges()
                    .iter()
                    .map(|hinge| hinge.edge())
                    .collect::<Vec<_>>();
                let fixed_face = self.fixed_face.ok_or(PoseAuthorityError::InvalidRequest)?;
                let graph_companion = ori_kinematics::MaterialHingeGraphGeometry::prepare(
                    self.binding.topology_input.pattern(),
                    self.binding.topology_input.paper(),
                    &topology,
                    TreeKinematicsLimits::default(),
                )
                .ok()
                .and_then(|geometry| {
                    let audit = ori_kinematics::MaterialHingeGraphAudit::prepare(
                        &topology,
                        TreeKinematicsLimits::default(),
                    )
                    .ok()?;
                    let graph_pose = geometry
                        .solve_closed(&audit, fixed_face, &canonical_angles, 1.0e-9)
                        .ok()?;
                    Some((Arc::new(geometry), Arc::new(audit), Arc::new(graph_pose)))
                });
                let (graph_geometry, graph_audit, graph_pose) = graph_companion
                    .map_or((None, None, None), |(geometry, audit, pose)| {
                        (Some(geometry), Some(audit), Some(pose))
                    });
                (
                    CurrentNativeMaterialPose::Tree {
                        model: Arc::clone(&model),
                        pose,
                        graph_geometry,
                        graph_audit,
                        graph_pose,
                    },
                    model.face_ids().to_vec(),
                    hinges,
                    self.fixed_face,
                )
            } else {
                let fixed_face = self.fixed_face.ok_or(PoseAuthorityError::InvalidRequest)?;
                let geometry = Arc::new(
                    ori_kinematics::MaterialHingeGraphGeometry::prepare(
                        self.binding.topology_input.pattern(),
                        self.binding.topology_input.paper(),
                        &topology,
                        TreeKinematicsLimits::default(),
                    )
                    .map_err(|_| PoseAuthorityError::KinematicsUnavailable)?,
                );
                let audit = Arc::new(
                    ori_kinematics::MaterialHingeGraphAudit::prepare(
                        &topology,
                        TreeKinematicsLimits::default(),
                    )
                    .map_err(|_| PoseAuthorityError::KinematicsUnavailable)?,
                );
                let pose = Arc::new(
                    geometry
                        .solve_closed(&audit, fixed_face, &canonical_angles, 1.0e-9)
                        .map_err(|_| PoseAuthorityError::KinematicsUnavailable)?,
                );
                let hinges = geometry
                    .hinges()
                    .iter()
                    .map(|hinge| hinge.edge())
                    .collect::<Vec<_>>();
                (
                    CurrentNativeMaterialPose::Graph {
                        geometry: Arc::clone(&geometry),
                        audit,
                        pose,
                    },
                    geometry.face_ids().to_vec(),
                    hinges,
                    Some(fixed_face),
                )
            };
        let semantic_pose = Arc::new(
            match &native_pose {
                CurrentNativeMaterialPose::Tree { .. } => prepare_applied_pose_v1(
                    &face_ids,
                    &expected_hinges,
                    semantic_fixed_face,
                    &self.complete_hinge_angles,
                    AppliedPoseLimitsV1::default(),
                ),
                CurrentNativeMaterialPose::Graph { .. } => prepare_closed_graph_applied_pose_v1(
                    &face_ids,
                    &expected_hinges,
                    semantic_fixed_face.ok_or(PoseAuthorityError::InvalidRequest)?,
                    &self.complete_hinge_angles,
                    AppliedPoseLimitsV1::default(),
                ),
            }
            .map_err(|_| PoseAuthorityError::SemanticPoseUnavailable)?,
        );

        let prepared = PreparedNativePose {
            slot: self.slot,
            binding: self.binding,
            pending_cleanup,
            topology,
            semantic_pose,
            native_pose,
        };
        if !prepared_native_pose_is_internally_consistent(&prepared) {
            return Err(PoseAuthorityError::InternalInconsistency);
        }
        Ok(prepared)
    }
}

/// Guard used by the centralized editor mutation funnel.
pub(super) struct PoseAuthorityInvalidation<'a> {
    slot: MutexGuard<'a, CurrentAppliedPoseSlot>,
    next_generation: u64,
}

impl PoseAuthorityInvalidation<'_> {
    /// Commits the already-preflighted invalidation. This operation is
    /// infallible so it can safely follow a successful editor mutation.
    pub(super) fn commit(mut self) {
        self.slot.current = None;
        self.slot.pending = None;
        self.slot.generation = self.next_generation;
    }
}

/// Replaces a project while invalidating every token issued by the old slot.
///
/// The replacement must carry a fresh authority. Its generation is advanced
/// to the same monotonic value before the final, infallible assignment.
pub(super) fn commit_project_replacement(
    current: &mut ProjectState,
    replacement: ProjectState,
) -> Result<(), PoseAuthorityError> {
    let old_authority = current.applied_pose_authority.clone();
    let new_authority = replacement.applied_pose_authority.clone();
    if Arc::ptr_eq(&old_authority.0, &new_authority.0) {
        return Err(PoseAuthorityError::ReplacementAuthorityNotFresh);
    }

    let mut old_slot = old_authority.lock()?;
    let next_generation = old_slot
        .generation
        .checked_add(1)
        .ok_or(PoseAuthorityError::GenerationExhausted)?;
    let mut new_slot = new_authority.lock()?;
    if new_slot.generation != 0
        || new_slot.current.is_some()
        || new_slot.pending.is_some()
        || replacement.editor.current_applied_pose().is_some()
    {
        return Err(PoseAuthorityError::ReplacementAuthorityNotFresh);
    }

    old_slot.current = None;
    old_slot.pending = None;
    old_slot.generation = next_generation;
    new_slot.generation = next_generation;
    drop(new_slot);
    drop(old_slot);
    *current = replacement;
    Ok(())
}

/// Captures the current capability from a project-owned authority.
#[allow(dead_code)]
pub(super) fn capture_current_applied_pose_capability(
    project: &ProjectState,
) -> Result<Option<CurrentAppliedPoseCapability>, PoseAuthorityError> {
    project.applied_pose_authority.capture_capability(project)
}

/// Observation-only capability revalidation.
#[allow(dead_code)]
pub(super) fn revalidate_current_applied_pose_capability<'a>(
    project: &ProjectState,
    capability: &'a CurrentAppliedPoseCapability,
) -> Result<Option<CurrentAppliedPoseView<'a>>, PoseAuthorityError> {
    project
        .applied_pose_authority
        .revalidate_capability(project, capability)
}

/// Runs an action while the project and pose-authority slot remain locked.
///
/// The action receives only an immutable project reference; A/B never bypasses
/// the centralized mutation funnel. SIM-010 will use a separate combined
/// project/pose/layer commit helper after C through E exist.
///
/// The fixed global order is project first, pose slot second. A future helper
/// combining layer order must continue with the layer-order slot third.
#[allow(dead_code)]
pub(super) fn with_revalidated_current_applied_pose_capability<R>(
    app_state: &AppState,
    capability: &CurrentAppliedPoseCapability,
    action: impl FnOnce(&ProjectState, CurrentAppliedPoseView<'_>) -> R,
) -> Result<Option<R>, PoseAuthorityError> {
    let project = lock_project(app_state).map_err(|_| PoseAuthorityError::LockUnavailable)?;
    let authority = project.applied_pose_authority.clone();
    if !Arc::ptr_eq(&authority.0, &capability.slot) {
        return Ok(None);
    }
    let slot = authority.lock()?;
    let Some(current) = slot.current.as_ref() else {
        return Ok(None);
    };
    if !current_applied_pose_capability_matches_locked_slot(&slot, &project, capability, current) {
        return Ok(None);
    }
    let output = action(
        &project,
        CurrentAppliedPoseView {
            certificate: capability.certificate.as_ref(),
        },
    );
    drop(slot);
    Ok(Some(output))
}

fn validate_and_normalize_request(
    request: &mut NativePoseRequest,
) -> Result<Vec<(EdgeId, f64)>, PoseAuthorityError> {
    let limits = AppliedPoseLimitsV1::default();
    if request.complete_hinge_angles.len() > limits.max_angle_records {
        return Err(PoseAuthorityError::InvalidRequest);
    }
    match (
        request.complete_hinge_angles.is_empty(),
        request.fixed_face_id,
    ) {
        (true, Some(_)) | (false, None) => return Err(PoseAuthorityError::InvalidRequest),
        _ => {}
    }
    for pair in request.complete_hinge_angles.windows(2) {
        if pair[0].edge_id.canonical_bytes() >= pair[1].edge_id.canonical_bytes() {
            return Err(PoseAuthorityError::InvalidRequest);
        }
    }

    let mut normalized = Vec::new();
    normalized
        .try_reserve_exact(request.complete_hinge_angles.len())
        .map_err(|_| PoseAuthorityError::InvalidRequest)?;
    for angle in &mut request.complete_hinge_angles {
        if !angle.angle_degrees.is_finite() || !(0.0..=180.0).contains(&angle.angle_degrees) {
            return Err(PoseAuthorityError::InvalidRequest);
        }
        if angle.angle_degrees == 0.0 {
            angle.angle_degrees = 0.0;
        }
        normalized.push((angle.edge_id, angle.angle_degrees));
    }
    Ok(normalized)
}

fn prepared_native_pose_is_internally_consistent(prepared: &PreparedNativePose) -> bool {
    prepared.binding.revision == prepared.binding.topology_input.revision()
        && prepared.topology.source_revision == prepared.binding.revision
        && prepared.binding.paper_thickness_bits
            == prepared
                .binding
                .topology_input
                .paper()
                .thickness_mm
                .to_bits()
        && match &prepared.native_pose {
            CurrentNativeMaterialPose::Tree { model, pose, .. } => {
                model.model_id() == MATERIAL_TREE_KINEMATICS_MODEL_ID
                    && model.owns_pose(pose)
                    && model.bind_pose(pose).is_ok()
                    && material_pose_matches_semantic(pose, &prepared.semantic_pose)
                    && model_and_pose_registries_match(model, pose)
            }
            CurrentNativeMaterialPose::Graph {
                geometry,
                audit,
                pose,
            } => {
                !audit.closure_hinges().is_empty()
                    && !geometry.face_ids().is_empty()
                    && pose.fixed_face()
                        == prepared
                            .semantic_pose
                            .fixed_face()
                            .unwrap_or(pose.fixed_face())
                    && pose.hinge_angles().as_slice().len()
                        == prepared.semantic_pose.hinge_angles().len()
                    && pose
                        .hinge_angles()
                        .as_slice()
                        .iter()
                        .zip(prepared.semantic_pose.hinge_angles())
                        .all(|(native, semantic)| {
                            native.edge() == semantic.edge()
                                && native.angle_degrees().to_bits()
                                    == semantic.angle_degrees().to_bits()
                        })
            }
        }
}

fn current_applied_pose_certificate_is_internally_consistent(
    certificate: &CurrentAppliedPoseCertificate,
) -> bool {
    let binding = &certificate.binding;
    let claims = &certificate.claims;
    claims.generation != 0
        && claims.request_id == binding.request_id
        && claims.project_instance_id == binding.project_instance_id
        && claims.project_id == binding.project_id
        && claims.revision == binding.revision
        && claims.revision == claims.topology_input.revision()
        && claims.topology.source_revision == claims.revision
        && Arc::ptr_eq(&claims.topology_input, &binding.topology_input)
        && claims.topology_input.as_ref() == binding.topology_input.as_ref()
        && Arc::ptr_eq(
            &claims.fold_model_fingerprint,
            &binding.fold_model_fingerprint,
        )
        && claims.fold_model_fingerprint.as_ref() == binding.fold_model_fingerprint.as_ref()
        && claims.paper_thickness_bits == binding.paper_thickness_bits
        && claims.paper_thickness_bits == claims.topology_input.paper().thickness_mm.to_bits()
        && claims.semantic_pose.model_id() == claims.semantic_model_id
        && claims.thickness_model_id == CENTERED_MID_SURFACE_THICKNESS_MODEL_V1
        && claims.contact_policy_id == TOPOLOGY_CONTACT_POLICY_V2
        && match &claims.native_pose {
            CurrentNativeMaterialPose::Tree { model, pose, .. } => {
                claims.kinematics_model_id == MATERIAL_TREE_KINEMATICS_MODEL_ID
                    && model.model_id() == claims.kinematics_model_id
                    && model.owns_pose(pose)
                    && model.bind_pose(pose).is_ok()
                    && material_pose_matches_semantic(pose, &claims.semantic_pose)
                    && registries_match_model_and_pose(
                        model,
                        pose,
                        &claims.material_faces,
                        &claims.material_hinges,
                    )
            }
            CurrentNativeMaterialPose::Graph {
                geometry,
                audit: _,
                pose,
            } => {
                claims.kinematics_model_id == "material_hinge_graph_pose_v1"
                    && geometry.face_ids() == claims.material_faces.as_ref()
                    && geometry
                        .hinges()
                        .iter()
                        .map(|hinge| hinge.edge())
                        .eq(claims.material_hinges.iter().copied())
                    && pose.fixed_face()
                        == claims
                            .semantic_pose
                            .fixed_face()
                            .unwrap_or(pose.fixed_face())
                    && pose
                        .hinge_angles()
                        .as_slice()
                        .iter()
                        .zip(claims.semantic_pose.hinge_angles())
                        .all(|(native, semantic)| {
                            native.edge() == semantic.edge()
                                && native.angle_degrees().to_bits()
                                    == semantic.angle_degrees().to_bits()
                        })
            }
        }
}

fn current_applied_pose_certificate_is_current(
    certificate: &CurrentAppliedPoseCertificate,
    project: &ProjectState,
) -> bool {
    binding_is_current(&certificate.binding, project)
        && project
            .editor
            .current_applied_pose()
            .is_some_and(|current| {
                semantic_pose_bits_equal(current, &certificate.claims.semantic_pose)
            })
}

fn binding_is_current(binding: &PoseSourceBinding, project: &ProjectState) -> bool {
    binding.project_instance_id == project.instance_id
        && binding.project_id == project.project_id
        && binding.revision == project.editor.revision()
        && binding
            .topology_input
            .is_current_for(project.project_id, &project.editor)
        && binding.fold_model_fingerprint.as_ref() == project.editor.fold_model_fingerprint_v1()
        && binding.paper_thickness_bits == project.editor.paper().thickness_mm.to_bits()
}

fn binding_claims_equal(first: &PoseSourceBinding, second: &PoseSourceBinding) -> bool {
    first.request_id == second.request_id
        && first.project_instance_id == second.project_instance_id
        && first.project_id == second.project_id
        && first.revision == second.revision
        && Arc::ptr_eq(&first.topology_input, &second.topology_input)
        && first.topology_input.as_ref() == second.topology_input.as_ref()
        && Arc::ptr_eq(
            &first.fold_model_fingerprint,
            &second.fold_model_fingerprint,
        )
        && first.fold_model_fingerprint.as_ref() == second.fold_model_fingerprint.as_ref()
        && first.paper_thickness_bits == second.paper_thickness_bits
}

fn current_applied_pose_claims_match(
    first: &CurrentAppliedPoseClaims,
    second: &CurrentAppliedPoseClaims,
) -> bool {
    first.request_id == second.request_id
        && first.project_instance_id == second.project_instance_id
        && first.project_id == second.project_id
        && first.revision == second.revision
        && Arc::ptr_eq(&first.topology_input, &second.topology_input)
        && first.topology_input.as_ref() == second.topology_input.as_ref()
        && Arc::ptr_eq(&first.topology, &second.topology)
        && first.topology.as_ref() == second.topology.as_ref()
        && Arc::ptr_eq(
            &first.fold_model_fingerprint,
            &second.fold_model_fingerprint,
        )
        && first.fold_model_fingerprint.as_ref() == second.fold_model_fingerprint.as_ref()
        && Arc::ptr_eq(&first.semantic_pose, &second.semantic_pose)
        && semantic_pose_bits_equal(&first.semantic_pose, &second.semantic_pose)
        && first.native_pose.same_native_instance(&second.native_pose)
        && Arc::ptr_eq(&first.material_faces, &second.material_faces)
        && first.material_faces.as_ref() == second.material_faces.as_ref()
        && Arc::ptr_eq(&first.material_hinges, &second.material_hinges)
        && first.material_hinges.as_ref() == second.material_hinges.as_ref()
        && first.paper_thickness_bits == second.paper_thickness_bits
        && first.kinematics_model_id == second.kinematics_model_id
        && first.semantic_model_id == second.semantic_model_id
        && first.thickness_model_id == second.thickness_model_id
        && first.contact_policy_id == second.contact_policy_id
        && first.generation == second.generation
}

fn current_applied_pose_capability_matches_locked_slot(
    slot: &CurrentAppliedPoseSlot,
    project: &ProjectState,
    capability: &CurrentAppliedPoseCapability,
    current: &Arc<CurrentAppliedPoseCertificate>,
) -> bool {
    Arc::ptr_eq(current, &capability.certificate)
        && current.claims.generation == slot.generation
        && capability.claims.generation == slot.generation
        && current_applied_pose_claims_match(&capability.claims, &current.claims)
        && current_applied_pose_certificate_is_internally_consistent(current)
        && current_applied_pose_certificate_is_current(current, project)
}

fn material_pose_matches_semantic(pose: &MaterialTreePose, semantic: &AppliedPoseV1) -> bool {
    semantic.model_id() == APPLIED_POSE_MODEL_ID_V1
        && pose.fixed_face() == semantic.fixed_face()
        && pose.hinge_angles().len() == semantic.hinge_angles().len()
        && pose
            .hinge_angles()
            .iter()
            .zip(semantic.hinge_angles())
            .all(|(native, semantic)| {
                native.edge() == semantic.edge()
                    && native.angle_degrees().to_bits() == semantic.angle_degrees().to_bits()
            })
}

fn semantic_pose_bits_equal(first: &AppliedPoseV1, second: &AppliedPoseV1) -> bool {
    first.model_id() == second.model_id()
        && first.fixed_face() == second.fixed_face()
        && first.hinge_angles().len() == second.hinge_angles().len()
        && first
            .hinge_angles()
            .iter()
            .zip(second.hinge_angles())
            .all(|(first, second)| {
                first.edge() == second.edge()
                    && first.angle_degrees().to_bits() == second.angle_degrees().to_bits()
            })
}

fn registries_match_model_and_pose(
    model: &MaterialTreeKinematicsModel,
    pose: &MaterialTreePose,
    faces: &[FaceId],
    hinges: &[EdgeId],
) -> bool {
    model.face_ids() == faces
        && pose.face_ids() == faces
        && model.hinges().len() == hinges.len()
        && pose.hinges().len() == hinges.len()
        && model
            .hinges()
            .iter()
            .zip(hinges)
            .all(|(hinge, edge)| hinge.edge() == *edge)
        && pose
            .hinges()
            .iter()
            .zip(hinges)
            .all(|(hinge, edge)| hinge.edge() == *edge)
}

fn model_and_pose_registries_match(
    model: &MaterialTreeKinematicsModel,
    pose: &MaterialTreePose,
) -> bool {
    model.face_ids() == pose.face_ids()
        && model.hinges().len() == pose.hinges().len()
        && model
            .hinges()
            .iter()
            .zip(pose.hinges())
            .all(|(model_hinge, pose_hinge)| model_hinge.edge() == pose_hinge.edge())
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct CurrentAppliedPoseAuthoritySnapshot {
    pub(super) generation: u64,
    pub(super) has_current: bool,
    pub(super) has_pending: bool,
}

#[cfg(test)]
pub(super) mod tests {
    use ori_core::{Command, create_rectangular_sheet};
    use ori_domain::{CreasePattern, Edge, EdgeKind, Paper, Point2, Vertex, VertexId};
    use serde_json::json;

    use super::*;
    use crate::{
        execute_command as execute_bound_command, execute_redo as execute_bound_redo,
        execute_undo as execute_bound_undo,
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

    fn execute_undo(
        project: &mut ProjectState,
        expected_project_id: ProjectId,
        expected_revision: u64,
    ) -> Result<crate::ProjectSnapshot, String> {
        execute_bound_undo(
            project,
            project.instance_id,
            expected_project_id,
            expected_revision,
        )
    }

    fn execute_redo(
        project: &mut ProjectState,
        expected_project_id: ProjectId,
        expected_revision: u64,
    ) -> Result<crate::ProjectSnapshot, String> {
        execute_bound_redo(
            project,
            project.instance_id,
            expected_project_id,
            expected_revision,
        )
    }

    fn no_hinge_project() -> ProjectState {
        let sheet = create_rectangular_sheet(40.0, 30.0, false).expect("rectangle fixture");
        let (pattern, paper) = sheet.into_parts();
        ProjectState::new_with_paper(pattern, paper)
    }

    fn invalid_topology_project() -> ProjectState {
        ProjectState::new_with_paper(CreasePattern::empty(), Paper::default())
    }

    fn fixed_id<T: serde::de::DeserializeOwned>(prefix: &str, suffix: u64) -> T {
        serde_json::from_value(serde_json::json!(format!(
            "{prefix}0000-0000-7000-8000-{suffix:012x}"
        )))
        .expect("fixed test id")
    }

    pub(crate) fn four_vertex_cycle_project() -> (ProjectState, Vec<EdgeId>) {
        let points = [
            (100.0, 0.0),
            (-50.0, 86.602_540_378_443_86),
            (-50.0, -86.602_540_378_443_86),
            (50.0, -86.602_540_378_443_86),
            (0.0, 0.0),
        ];
        let vertices = points
            .into_iter()
            .enumerate()
            .map(|(index, (x, y))| Vertex {
                id: fixed_id("ae00", index as u64 + 1),
                position: Point2::new(x, y),
            })
            .collect::<Vec<_>>();
        let boundary = vertices[..4]
            .iter()
            .map(|vertex| vertex.id)
            .collect::<Vec<_>>();
        let center = vertices[4].id;
        let mut edges = (0..4)
            .map(|index| Edge {
                id: fixed_id("af00", index as u64 + 1),
                start: boundary[index],
                end: boundary[(index + 1) % 4],
                kind: EdgeKind::Boundary,
            })
            .collect::<Vec<_>>();
        let hinges = (0..4)
            .map(|index| fixed_id("af00", index as u64 + 10))
            .collect::<Vec<_>>();
        edges.extend((0..4).map(|index| Edge {
            id: hinges[index],
            start: boundary[index],
            end: center,
            kind: if index == 3 {
                EdgeKind::Mountain
            } else {
                EdgeKind::Valley
            },
        }));
        let paper = Paper {
            boundary_vertices: boundary,
            ..Paper::default()
        };
        let mut project = ProjectState::new_with_paper(CreasePattern { vertices, edges }, paper);
        project.instance_id = fixed_id("aa00", 2);
        project.project_id = fixed_id("aa00", 1);
        (project, hinges)
    }

    pub(crate) fn flat_foldable_cross_cycle_project() -> (ProjectState, Vec<EdgeId>) {
        let points = [
            (100.0, 0.0),
            (-50.0, 86.602_540_378_443_86),
            (-50.0, -86.602_540_378_443_86),
            (50.0, -86.602_540_378_443_86),
            (0.0, 0.0),
        ];
        let vertices = points
            .into_iter()
            .map(|(x, y)| Vertex {
                id: VertexId::new(),
                position: Point2::new(x, y),
            })
            .collect::<Vec<_>>();
        let boundary = vertices[..4]
            .iter()
            .map(|vertex| vertex.id)
            .collect::<Vec<_>>();
        let center = vertices[4].id;
        let mut edges = (0..4)
            .map(|index| Edge {
                id: EdgeId::new(),
                start: boundary[index],
                end: boundary[(index + 1) % 4],
                kind: EdgeKind::Boundary,
            })
            .collect::<Vec<_>>();
        let hinges = (0..4).map(|_| EdgeId::new()).collect::<Vec<_>>();
        edges.extend((0..4).map(|index| Edge {
            id: hinges[index],
            start: boundary[index],
            end: center,
            kind: if index == 3 {
                EdgeKind::Mountain
            } else {
                EdgeKind::Valley
            },
        }));
        let paper = Paper {
            boundary_vertices: boundary,
            ..Paper::default()
        };
        (
            ProjectState::new_with_paper(CreasePattern { vertices, edges }, paper),
            hinges,
        )
    }

    pub(crate) fn install_flat_graph_pose_authority(
        project: &mut ProjectState,
        mut hinges: Vec<EdgeId>,
    ) {
        hinges.sort_unstable_by_key(EdgeId::canonical_bytes);
        let topology = project
            .editor
            .topology_analysis_input(project.project_id)
            .analyze();
        let fixed_face = topology.simulation_snapshot().unwrap().faces[0].id;
        install_flat_graph_pose_authority_on_face(project, hinges, fixed_face);
    }

    pub(crate) fn install_flat_graph_pose_authority_on_face(
        project: &mut ProjectState,
        mut hinges: Vec<EdgeId>,
        fixed_face: FaceId,
    ) {
        hinges.sort_unstable_by_key(EdgeId::canonical_bytes);
        let request = NativePoseRequest {
            expected_project_instance_id: project.instance_id,
            expected_project_id: project.project_id,
            expected_revision: project.editor.revision(),
            fixed_face_id: Some(fixed_face),
            complete_hinge_angles: hinges
                .into_iter()
                .map(|edge_id| NativePoseHingeAngleRequest {
                    edge_id,
                    angle_degrees: 0.0,
                })
                .collect(),
        };
        let authority = project.applied_pose_authority.clone();
        let prepared = authority
            .capture_request(project, request)
            .unwrap()
            .prepare()
            .unwrap();
        authority.commit_prepared(project, prepared).unwrap();
    }

    #[test]
    fn graph_pose_capture_commit_and_revalidation_are_native_bound() {
        let (mut project, mut hinges) = four_vertex_cycle_project();
        hinges.sort_unstable_by_key(EdgeId::canonical_bytes);
        let topology = project
            .editor
            .topology_analysis_input(project.project_id)
            .analyze();
        let fixed_face = topology.simulation_snapshot().unwrap().faces[0].id;
        let request = NativePoseRequest {
            expected_project_instance_id: project.instance_id,
            expected_project_id: project.project_id,
            expected_revision: project.editor.revision(),
            fixed_face_id: Some(fixed_face),
            complete_hinge_angles: hinges
                .into_iter()
                .map(|edge_id| NativePoseHingeAngleRequest {
                    edge_id,
                    angle_degrees: 0.0,
                })
                .collect(),
        };
        let authority = project.applied_pose_authority.clone();
        let prepared = authority
            .capture_request(&project, request)
            .unwrap()
            .prepare()
            .unwrap();
        assert!(prepared.native_pose.graph().is_some());
        let capability = authority.commit_prepared(&mut project, prepared).unwrap();
        assert!(capability.graph().is_some());
        assert!(
            authority
                .revalidate_capability(&project, &capability)
                .unwrap()
                .is_some()
        );
        let mut foreign = no_hinge_project();
        foreign.applied_pose_authority = authority;
        assert!(
            foreign
                .applied_pose_authority
                .revalidate_capability(&foreign, &capability)
                .unwrap()
                .is_none()
        );
    }

    pub(super) fn request_for(project: &ProjectState) -> NativePoseRequest {
        NativePoseRequest {
            expected_project_instance_id: project.instance_id,
            expected_project_id: project.project_id,
            expected_revision: project.editor.revision(),
            fixed_face_id: None,
            complete_hinge_angles: Vec::new(),
        }
    }

    #[test]
    fn public_pose_binding_serializes_generation_without_javascript_precision_loss() {
        let binding = CurrentAppliedPoseBindingResponse {
            project_instance_id: ProjectId::new(),
            project_id: ProjectId::new(),
            revision: 7,
            pose_generation: u64::MAX,
        };
        let encoded = serde_json::to_value(binding).expect("serialize pose binding");
        assert_eq!(encoded["revision"], 7);
        assert_eq!(encoded["poseGeneration"], u64::MAX.to_string());
    }

    #[test]
    fn current_pose_round_trips_as_independent_native_authority() {
        let mut project = no_hinge_project();
        install_current_pose_authority(&mut project);
        let archive = project.project_archive().expect("archive current pose");
        let persisted = archive
            .document
            .current_pose
            .clone()
            .expect("persisted current pose");

        let reopened = ProjectState::from_project_archive(
            archive,
            std::path::PathBuf::from("current-pose.ori2"),
        )
        .expect("restore current pose");
        assert_eq!(reopened.document().current_pose, Some(persisted));
        assert!(reopened.editor.current_applied_pose().is_some());
        assert!(
            reopened
                .applied_pose_authority
                .capture_capability(&reopened)
                .expect("capture restored authority")
                .is_some()
        );
        assert!(!reopened.is_dirty());
    }

    #[test]
    fn busy_process_worker_rejects_apply_without_touching_project_or_pose_authority() {
        let mut project = no_hinge_project();
        let (authority, current_capability) = install_current_pose_authority(&mut project);
        let request = request_for(&project);
        let document_before = project.document();
        let revision_before = project.editor.revision();
        let authority_before = authority.test_snapshot().expect("authority snapshot");
        let state = AppState::new(project);

        let permit = state
            .try_acquire_native_pose_worker()
            .expect("first process worker permit");
        assert!(state.native_pose_worker_is_busy());
        assert!(state.try_acquire_native_pose_worker().is_none());
        let error =
            tauri::async_runtime::block_on(apply_current_native_pose(&state, request.clone()))
                .expect_err("busy worker must reject another apply");
        assert_eq!(error, APPLY_CURRENT_NATIVE_POSE_FAILED_MESSAGE);
        assert_eq!(
            authority.test_snapshot().expect("unchanged authority"),
            authority_before
        );
        {
            let project = state.0.lock().expect("project lock");
            assert_eq!(project.document(), document_before);
            assert_eq!(project.editor.revision(), revision_before);
            assert!(project.editor.current_applied_pose().is_some());
            assert!(
                authority
                    .revalidate_capability(&project, &current_capability)
                    .expect("revalidate current capability")
                    .is_some()
            );
        }

        drop(permit);
        assert!(!state.native_pose_worker_is_busy());
        tauri::async_runtime::block_on(apply_current_native_pose(&state, request))
            .expect("released permit must allow apply");
        assert!(!state.native_pose_worker_is_busy());
    }

    #[test]
    fn failed_blocking_preparation_releases_process_permit_and_pending_marker() {
        let project = invalid_topology_project();
        let authority = project.applied_pose_authority.clone();
        let request = request_for(&project);
        let state = AppState::new(project);

        let error = tauri::async_runtime::block_on(apply_current_native_pose(&state, request))
            .expect_err("invalid topology must fail preparation");
        assert_eq!(error, APPLY_CURRENT_NATIVE_POSE_FAILED_MESSAGE);
        assert!(!state.native_pose_worker_is_busy());
        assert_eq!(
            authority.test_snapshot().expect("authority snapshot"),
            CurrentAppliedPoseAuthoritySnapshot {
                generation: 0,
                has_current: false,
                has_pending: false,
            }
        );
        assert!(state.try_acquire_native_pose_worker().is_some());
    }

    #[test]
    fn process_worker_gate_survives_project_replacement() {
        let state = AppState::new(no_hinge_project());
        let permit = state
            .try_acquire_native_pose_worker()
            .expect("process worker permit");
        {
            let mut project = state.0.lock().expect("project lock");
            let document = project.document();
            let replacement =
                ProjectState::from_document(document, std::path::PathBuf::from("same.ori2"));
            commit_project_replacement(&mut project, replacement).expect("replacement");
        }
        assert!(
            state.try_acquire_native_pose_worker().is_none(),
            "a fresh project authority must not bypass the process-wide gate"
        );
        drop(permit);
        assert!(state.try_acquire_native_pose_worker().is_some());
    }

    #[test]
    fn blocking_worker_panic_releases_process_permit() {
        let state = AppState::new(no_hinge_project());
        let permit = state
            .try_acquire_native_pose_worker()
            .expect("process worker permit");
        let join =
            tauri::async_runtime::block_on(tauri::async_runtime::spawn_blocking(move || {
                let _permit = permit;
                panic!("injected native pose worker panic");
            }));
        assert!(join.is_err(), "panic must surface as a join error");
        assert!(!state.native_pose_worker_is_busy());
        assert!(state.try_acquire_native_pose_worker().is_some());
    }

    pub(super) fn install_current_pose_authority(
        project: &mut ProjectState,
    ) -> (CurrentAppliedPoseAuthority, CurrentAppliedPoseCapability) {
        let authority = project.applied_pose_authority.clone();
        let captured = authority
            .capture_request(project, request_for(project))
            .expect("capture");
        let prepared = captured.prepare().expect("prepare");
        let capability = authority
            .commit_prepared(project, prepared)
            .expect("commit");
        (authority, capability)
    }

    pub(crate) fn install_flat_pose_authority(
        project: &mut ProjectState,
    ) -> (CurrentAppliedPoseAuthority, CurrentAppliedPoseCapability) {
        let topology = project
            .editor
            .topology_analysis_input(project.project_id)
            .analyze()
            .simulation_snapshot()
            .cloned()
            .expect("simulation topology");
        let model = MaterialTreeKinematicsModel::prepare(
            project.editor.pattern(),
            project.editor.paper(),
            &topology,
            TreeKinematicsLimits::default(),
        )
        .expect("tree pose fixture");
        let fixed_face_id = topology
            .faces
            .iter()
            .min_by_key(|face| (face.key, face.id.canonical_bytes()))
            .map(|face| face.id);
        let authority = project.applied_pose_authority.clone();
        let captured = authority
            .capture_request(
                project,
                NativePoseRequest {
                    expected_project_instance_id: project.instance_id,
                    expected_project_id: project.project_id,
                    expected_revision: project.editor.revision(),
                    fixed_face_id,
                    complete_hinge_angles: model
                        .hinges()
                        .iter()
                        .map(|hinge| NativePoseHingeAngleRequest {
                            edge_id: hinge.edge(),
                            angle_degrees: 180.0,
                        })
                        .collect(),
                },
            )
            .expect("capture flat pose");
        let prepared = captured.prepare().expect("prepare flat pose");
        let capability = authority
            .commit_prepared(project, prepared)
            .expect("commit flat pose");
        (authority, capability)
    }

    #[test]
    fn no_hinge_pose_is_adopted_without_revision_or_history_and_marks_document_dirty() {
        let mut project = no_hinge_project();
        let document_before = project.document();
        let revision_before = project.editor.revision();
        let can_undo_before = project.editor.can_undo();
        let can_redo_before = project.editor.can_redo();
        assert!(!project.is_dirty());

        let authority = project.applied_pose_authority.clone();
        let captured = authority
            .capture_request(&project, request_for(&project))
            .expect("capture");
        assert_eq!(
            authority.test_snapshot().expect("snapshot"),
            CurrentAppliedPoseAuthoritySnapshot {
                generation: 0,
                has_current: false,
                has_pending: true,
            }
        );
        let prepared = captured.prepare().expect("prepare");
        let capability = authority
            .commit_prepared(&mut project, prepared)
            .expect("commit");

        assert_eq!(project.editor.revision(), revision_before);
        assert_eq!(project.editor.can_undo(), can_undo_before);
        assert_eq!(project.editor.can_redo(), can_redo_before);
        let mut document_after = project.document();
        assert!(
            document_after.current_pose.is_some(),
            "adopted pose must be persisted in the projected document"
        );
        document_after.current_pose = document_before.current_pose.clone();
        assert_eq!(document_after, document_before);
        assert!(project.is_dirty());
        assert_eq!(
            authority.test_snapshot().expect("snapshot"),
            CurrentAppliedPoseAuthoritySnapshot {
                generation: 1,
                has_current: true,
                has_pending: false,
            }
        );
        let view = authority
            .revalidate_capability(&project, &capability)
            .expect("revalidate")
            .expect("current");
        assert_eq!(view.generation(), 1);
        assert_eq!(view.model().face_ids().len(), 1);
        assert!(view.pose().hinge_angles().is_empty());
        assert!(view.semantic_pose().hinge_angles().is_empty());
        assert_eq!(
            view.thickness_model_id(),
            CENTERED_MID_SURFACE_THICKNESS_MODEL_V1
        );
        assert_eq!(view.contact_policy_id(), TOPOLOGY_CONTACT_POLICY_V2);
        assert_eq!(
            view.paper_thickness_bits(),
            project.editor.paper().thickness_mm.to_bits()
        );
    }

    #[test]
    fn only_the_latest_pending_request_can_commit() {
        let mut project = no_hinge_project();
        let authority = project.applied_pose_authority.clone();
        let first = authority
            .capture_request(&project, request_for(&project))
            .expect("first capture");
        let second = authority
            .capture_request(&project, request_for(&project))
            .expect("second capture");
        let first = first.prepare().expect("first prepare");
        let second = second.prepare().expect("second prepare");
        let before = authority.test_snapshot().expect("snapshot");

        assert_eq!(
            authority.commit_prepared(&mut project, first).err(),
            Some(PoseAuthorityError::StaleRequest)
        );
        assert_eq!(authority.test_snapshot().expect("snapshot"), before);
        assert!(project.editor.current_applied_pose().is_none());

        authority
            .commit_prepared(&mut project, second)
            .expect("latest request commits");
        assert_eq!(
            authority.test_snapshot().expect("snapshot"),
            CurrentAppliedPoseAuthoritySnapshot {
                generation: 1,
                has_current: true,
                has_pending: false,
            }
        );
    }

    #[test]
    fn dropping_prepared_pose_before_commit_clears_its_pending_marker() {
        let project = no_hinge_project();
        let authority = project.applied_pose_authority.clone();
        let prepared = authority
            .capture_request(&project, request_for(&project))
            .expect("capture")
            .prepare()
            .expect("prepare");
        assert_eq!(
            authority.test_snapshot().expect("snapshot"),
            CurrentAppliedPoseAuthoritySnapshot {
                generation: 0,
                has_current: false,
                has_pending: true,
            }
        );

        drop(prepared);

        assert_eq!(
            authority.test_snapshot().expect("snapshot"),
            CurrentAppliedPoseAuthoritySnapshot {
                generation: 0,
                has_current: false,
                has_pending: false,
            }
        );
    }

    #[test]
    fn dropping_older_prepared_pose_does_not_clear_a_newer_pending_request() {
        let project = no_hinge_project();
        let authority = project.applied_pose_authority.clone();
        let older_prepared = authority
            .capture_request(&project, request_for(&project))
            .expect("older capture")
            .prepare()
            .expect("older prepare");
        let newer = authority
            .capture_request(&project, request_for(&project))
            .expect("newer capture");

        drop(older_prepared);
        assert_eq!(
            authority.test_snapshot().expect("snapshot"),
            CurrentAppliedPoseAuthoritySnapshot {
                generation: 0,
                has_current: false,
                has_pending: true,
            }
        );

        drop(newer.prepare().expect("newer prepare"));
        assert_eq!(
            authority.test_snapshot().expect("snapshot"),
            CurrentAppliedPoseAuthoritySnapshot {
                generation: 0,
                has_current: false,
                has_pending: false,
            }
        );
    }

    #[test]
    fn cancelled_blocking_receiver_drops_prepared_pose_and_clears_pending_marker() {
        use std::{
            sync::{Barrier, mpsc},
            time::Duration,
        };

        struct AbandonedWorkerOutput {
            prepared: Option<Result<PreparedNativePose, PoseAuthorityError>>,
            dropped: Option<mpsc::Sender<()>>,
        }

        impl Drop for AbandonedWorkerOutput {
            fn drop(&mut self) {
                drop(self.prepared.take());
                if let Some(dropped) = self.dropped.take() {
                    let _ = dropped.send(());
                }
            }
        }

        let project = no_hinge_project();
        let authority = project.applied_pose_authority.clone();
        let captured = authority
            .capture_request(&project, request_for(&project))
            .expect("capture");
        let started = Arc::new(Barrier::new(2));
        let release = Arc::new(Barrier::new(2));
        let worker_started = Arc::clone(&started);
        let worker_release = Arc::clone(&release);
        let (dropped_sender, dropped_receiver) = mpsc::channel();

        tauri::async_runtime::block_on(async move {
            let handle = tauri::async_runtime::spawn_blocking(move || {
                worker_started.wait();
                worker_release.wait();
                AbandonedWorkerOutput {
                    prepared: Some(captured.prepare()),
                    dropped: Some(dropped_sender),
                }
            });
            started.wait();
            handle.abort();
            drop(handle);
            release.wait();
            dropped_receiver
                .recv_timeout(Duration::from_secs(10))
                .expect("abandoned worker output must be dropped");
        });

        assert_eq!(
            authority.test_snapshot().expect("snapshot"),
            CurrentAppliedPoseAuthoritySnapshot {
                generation: 0,
                has_current: false,
                has_pending: false,
            }
        );
    }

    #[test]
    fn failed_preparation_clears_only_its_own_pending_marker() {
        let project = invalid_topology_project();
        let authority = project.applied_pose_authority.clone();
        let first = authority
            .capture_request(&project, request_for(&project))
            .expect("first capture");
        let second = authority
            .capture_request(&project, request_for(&project))
            .expect("second capture");

        assert_eq!(
            first.prepare().err(),
            Some(PoseAuthorityError::TopologyUnavailable)
        );
        assert_eq!(
            authority.test_snapshot().expect("snapshot"),
            CurrentAppliedPoseAuthoritySnapshot {
                generation: 0,
                has_current: false,
                has_pending: true,
            }
        );

        assert_eq!(
            second.prepare().err(),
            Some(PoseAuthorityError::TopologyUnavailable)
        );
        assert_eq!(
            authority.test_snapshot().expect("snapshot"),
            CurrentAppliedPoseAuthoritySnapshot {
                generation: 0,
                has_current: false,
                has_pending: false,
            }
        );
    }

    #[test]
    fn stale_commit_and_stale_capture_leave_authority_unchanged() {
        let mut project = no_hinge_project();
        let authority = project.applied_pose_authority.clone();
        let captured = authority
            .capture_request(&project, request_for(&project))
            .expect("capture");
        let prepared = captured.prepare().expect("prepare");
        project
            .editor
            .execute(
                0,
                Command::AddVertex {
                    id: VertexId::new(),
                    position: Point2::new(1.0, 1.0),
                },
            )
            .expect("unrelated edit");
        let before = authority.test_snapshot().expect("snapshot");

        assert_eq!(
            authority.commit_prepared(&mut project, prepared).err(),
            Some(PoseAuthorityError::StaleRequest)
        );
        assert_eq!(authority.test_snapshot().expect("snapshot"), before);
        assert!(project.editor.current_applied_pose().is_none());

        let mut stale = request_for(&project);
        stale.expected_revision -= 1;
        assert_eq!(
            authority.capture_request(&project, stale).err(),
            Some(PoseAuthorityError::StaleRequest)
        );
        assert_eq!(authority.test_snapshot().expect("snapshot"), before);
    }

    #[test]
    fn generation_exhaustion_fails_without_adopting_or_clearing_pending() {
        let mut project = no_hinge_project();
        let authority = project.applied_pose_authority.clone();
        let prepared = authority
            .capture_request(&project, request_for(&project))
            .expect("capture")
            .prepare()
            .expect("prepare");
        authority
            .set_generation_for_test(u64::MAX)
            .expect("set generation");
        let before = authority.test_snapshot().expect("snapshot");

        assert_eq!(
            authority
                .capture_request(&project, request_for(&project))
                .err(),
            Some(PoseAuthorityError::GenerationExhausted)
        );
        assert_eq!(authority.test_snapshot().expect("snapshot"), before);
        assert_eq!(
            authority.commit_prepared(&mut project, prepared).err(),
            Some(PoseAuthorityError::GenerationExhausted)
        );
        assert_eq!(authority.test_snapshot().expect("snapshot"), before);
        assert!(project.editor.current_applied_pose().is_none());
    }

    #[test]
    fn semantic_clone_failure_precedes_slot_lock_and_preserves_every_live_state() {
        let mut project = no_hinge_project();
        let (authority, current_capability) = install_current_pose_authority(&mut project);
        let prepared = authority
            .capture_request(&project, request_for(&project))
            .expect("capture replacement pose")
            .prepare()
            .expect("prepare replacement pose");
        let document_before = project.document();
        let revision_before = project.editor.revision();
        let can_undo_before = project.editor.can_undo();
        let can_redo_before = project.editor.can_redo();
        let semantic_before = project
            .editor
            .current_applied_pose()
            .expect("current semantic pose")
            .try_clone()
            .expect("test semantic snapshot");
        let authority_before = authority.test_snapshot().expect("authority snapshot");

        assert_eq!(
            authority
                .commit_prepared_with_semantic_clone(&mut project, prepared, |_| {
                    assert!(
                        authority.0.try_lock().is_ok(),
                        "semantic duplication must run before the pose slot is locked"
                    );
                    Err(PoseAuthorityError::SemanticPoseUnavailable)
                })
                .err(),
            Some(PoseAuthorityError::SemanticPoseUnavailable)
        );

        assert_eq!(project.document(), document_before);
        assert_eq!(project.editor.revision(), revision_before);
        assert_eq!(project.editor.can_undo(), can_undo_before);
        assert_eq!(project.editor.can_redo(), can_redo_before);
        assert_eq!(
            project.editor.current_applied_pose(),
            Some(&semantic_before)
        );
        assert_eq!(
            authority.test_snapshot().expect("authority snapshot"),
            authority_before
        );
        assert!(
            authority
                .revalidate_capability(&project, &current_capability)
                .expect("revalidate current capability")
                .is_some()
        );
    }

    #[test]
    fn same_angle_resolve_is_a_new_pose_and_invalidates_the_old_capability() {
        let mut project = no_hinge_project();
        let (authority, first_capability) = install_current_pose_authority(&mut project);
        let first_pose = first_capability
            .certificate
            .claims
            .native_pose
            .tree()
            .unwrap()
            .1
            .clone();

        let second_prepared = authority
            .capture_request(&project, request_for(&project))
            .expect("same-angle capture")
            .prepare()
            .expect("same-angle prepare");
        let second_capability = authority
            .commit_prepared(&mut project, second_prepared)
            .expect("same-angle commit");

        assert!(
            !first_pose.same_instance(
                second_capability
                    .certificate
                    .claims
                    .native_pose
                    .tree()
                    .unwrap()
                    .1
            ),
            "a new solve must not collapse same-angle ABA"
        );
        assert!(
            authority
                .revalidate_capability(&project, &first_capability)
                .expect("old revalidation")
                .is_none()
        );
        assert!(
            authority
                .revalidate_capability(&project, &second_capability)
                .expect("new revalidation")
                .is_some()
        );
    }

    #[test]
    fn capability_and_prepared_pose_cannot_cross_authority_slots() {
        let mut first_project = no_hinge_project();
        let second_project = no_hinge_project();
        let (first_authority, capability) = install_current_pose_authority(&mut first_project);
        let prepared = first_authority
            .capture_request(&first_project, request_for(&first_project))
            .expect("capture")
            .prepare()
            .expect("prepare");
        let second_authority = second_project.applied_pose_authority.clone();
        let second_before = second_authority.test_snapshot().expect("snapshot");

        assert!(
            second_authority
                .revalidate_capability(&second_project, &capability)
                .expect("revalidation")
                .is_none()
        );
        let mut second_project = second_project;
        assert_eq!(
            second_authority
                .commit_prepared(&mut second_project, prepared)
                .err(),
            Some(PoseAuthorityError::WrongAuthority)
        );
        assert_eq!(
            second_authority.test_snapshot().expect("snapshot"),
            second_before
        );
    }

    #[test]
    fn request_dto_rejects_unknown_fields() {
        let project = no_hinge_project();
        let value = json!({
            "expectedProjectInstanceId": project.instance_id,
            "expectedProjectId": project.project_id,
            "expectedRevision": project.editor.revision(),
            "fixedFaceId": null,
            "completeHingeAngles": [],
            "faceTransforms": [],
        });

        assert!(serde_json::from_value::<NativePoseRequest>(value).is_err());

        let nested = json!({
            "expectedProjectInstanceId": project.instance_id,
            "expectedProjectId": project.project_id,
            "expectedRevision": project.editor.revision(),
            "fixedFaceId": FaceId::derive_v5(project.project_id, b"face"),
            "completeHingeAngles": [{
                "edgeId": EdgeId::new(),
                "angleDegrees": 10.0,
                "transform": [1.0, 0.0, 0.0, 1.0],
            }],
        });
        assert!(serde_json::from_value::<NativePoseRequest>(nested).is_err());
    }

    #[test]
    fn invalid_request_does_not_replace_a_valid_pending_request() {
        let project = no_hinge_project();
        let authority = project.applied_pose_authority.clone();
        let _valid = authority
            .capture_request(&project, request_for(&project))
            .expect("valid pending");
        let before = authority.test_snapshot().expect("snapshot");
        let mut invalid = request_for(&project);
        invalid.fixed_face_id = Some(FaceId::derive_v5(project.project_id, b"unexpected"));

        assert_eq!(
            authority.capture_request(&project, invalid).err(),
            Some(PoseAuthorityError::InvalidRequest)
        );
        assert_eq!(authority.test_snapshot().expect("snapshot"), before);
    }

    #[test]
    fn dropped_invalidation_is_unchanged_and_committed_invalidation_is_monotonic() {
        let mut project = no_hinge_project();
        let (authority, capability) = install_current_pose_authority(&mut project);
        let before = authority.test_snapshot().expect("snapshot");

        drop(authority.begin_invalidation().expect("preflight"));
        assert_eq!(authority.test_snapshot().expect("snapshot"), before);
        assert!(
            authority
                .revalidate_capability(&project, &capability)
                .expect("revalidate")
                .is_some()
        );

        authority.begin_invalidation().expect("preflight").commit();
        assert_eq!(
            authority.test_snapshot().expect("snapshot"),
            CurrentAppliedPoseAuthoritySnapshot {
                generation: 2,
                has_current: false,
                has_pending: false,
            }
        );
        assert!(
            authority
                .revalidate_capability(&project, &capability)
                .expect("revalidate")
                .is_none()
        );
    }

    #[test]
    fn replacement_invalidates_old_slot_and_carries_monotonic_generation() {
        let mut current = no_hinge_project();
        let (old_authority, old_capability) = install_current_pose_authority(&mut current);
        let replacement = no_hinge_project();

        commit_project_replacement(&mut current, replacement).expect("replace");

        assert_eq!(
            old_authority.test_snapshot().expect("old snapshot"),
            CurrentAppliedPoseAuthoritySnapshot {
                generation: 2,
                has_current: false,
                has_pending: false,
            }
        );
        assert_eq!(
            current
                .applied_pose_authority
                .test_snapshot()
                .expect("new snapshot"),
            CurrentAppliedPoseAuthoritySnapshot {
                generation: 2,
                has_current: false,
                has_pending: false,
            }
        );
        assert!(
            current
                .applied_pose_authority
                .revalidate_capability(&current, &old_capability)
                .expect("revalidate")
                .is_none()
        );
    }

    #[test]
    fn guarded_closure_revalidates_under_project_then_pose_lock() {
        let mut project = no_hinge_project();
        let (_, capability) = install_current_pose_authority(&mut project);
        let app_state = AppState::new(project);

        let generation = with_revalidated_current_applied_pose_capability(
            &app_state,
            &capability,
            |_project, view| view.generation(),
        )
        .expect("guarded call");
        assert_eq!(generation, Some(1));
    }

    #[test]
    fn editor_command_funnel_invalidates_only_after_a_successful_revision_change() {
        let mut project = no_hinge_project();
        let (authority, old_capability) = install_current_pose_authority(&mut project);
        let vertex = VertexId::new();
        let project_id = project.project_id;

        execute_command(
            &mut project,
            project_id,
            0,
            Command::AddVertex {
                id: vertex,
                position: Point2::new(1.0, 1.0),
            },
        )
        .expect("successful editor command");
        assert_eq!(
            authority.test_snapshot().expect("snapshot"),
            CurrentAppliedPoseAuthoritySnapshot {
                generation: 2,
                has_current: false,
                has_pending: false,
            }
        );
        assert!(
            authority
                .revalidate_capability(&project, &old_capability)
                .expect("revalidate")
                .is_none()
        );

        let (_, current_capability) = install_current_pose_authority(&mut project);
        let state_before = authority.test_snapshot().expect("snapshot");
        let revision_before = project.editor.revision();
        let document_before = project.document();
        assert!(
            execute_command(
                &mut project,
                project_id,
                revision_before,
                Command::AddVertex {
                    id: vertex,
                    position: Point2::new(2.0, 2.0),
                },
            )
            .is_err()
        );
        assert_eq!(project.editor.revision(), revision_before);
        assert_eq!(project.document(), document_before);
        assert_eq!(authority.test_snapshot().expect("snapshot"), state_before);
        assert!(
            authority
                .revalidate_capability(&project, &current_capability)
                .expect("revalidate")
                .is_some()
        );
    }

    #[test]
    fn undo_redo_funnel_skips_empty_stacks_and_invalidates_moved_entries() {
        let mut project = no_hinge_project();
        let authority = project.applied_pose_authority.clone();
        let project_id = project.project_id;

        execute_undo(&mut project, project_id, 0).expect("empty undo");
        execute_redo(&mut project, project_id, 0).expect("empty redo");
        assert_eq!(
            authority.test_snapshot().expect("snapshot"),
            CurrentAppliedPoseAuthoritySnapshot {
                generation: 0,
                has_current: false,
                has_pending: false,
            }
        );

        install_current_pose_authority(&mut project);
        execute_command(
            &mut project,
            project_id,
            0,
            Command::AddVertex {
                id: VertexId::new(),
                position: Point2::new(1.0, 1.0),
            },
        )
        .expect("create history");
        install_current_pose_authority(&mut project);
        execute_undo(&mut project, project_id, 1).expect("entry-moving undo");
        assert_eq!(
            authority.test_snapshot().expect("snapshot"),
            CurrentAppliedPoseAuthoritySnapshot {
                generation: 4,
                has_current: false,
                has_pending: false,
            }
        );

        install_current_pose_authority(&mut project);
        execute_redo(&mut project, project_id, 2).expect("entry-moving redo");
        assert_eq!(
            authority.test_snapshot().expect("snapshot"),
            CurrentAppliedPoseAuthoritySnapshot {
                generation: 6,
                has_current: false,
                has_pending: false,
            }
        );
    }
}
