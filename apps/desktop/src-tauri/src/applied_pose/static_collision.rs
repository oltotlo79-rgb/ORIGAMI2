//! Current-project wrapper for native static-collision geometry evidence.
//!
//! This is the deliberately limited C boundary. It can currently certify only
//! the complete zero-pair case supported by `ori-collision`; it grants neither
//! project-mutation nor SIM-010 authority.

use std::{error::Error, fmt, sync::Arc};

use ori_collision::{
    CENTERED_MID_SURFACE_THICKNESS_MODEL_V1, NATIVE_STATIC_COLLISION_GEOMETRY_PROOF_V1,
    NativeStaticCollisionGeometryProof, StaticCollisionError, StaticCollisionLimits,
    TOPOLOGY_CONTACT_POLICY_V2, prove_static_collision_geometry,
};
use ori_domain::ProjectId;
use ori_kinematics::{
    MATERIAL_TREE_KINEMATICS_MODEL_ID, MaterialTreeKinematicsModel, MaterialTreePose,
};

use super::{
    CurrentAppliedPoseCapability, current_applied_pose_capability_matches_locked_slot,
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
        self.certificate.pose_capability.claims.model.as_ref()
    }

    #[must_use]
    pub(super) fn pose(&self) -> &MaterialTreePose {
        self.certificate.pose_capability.claims.pose.as_ref()
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
    let prepared = prepare_static_collision(capability, limits)?;
    mint_current_static_collision(app_state, prepared)
}

/// Revalidates the embedded B capability and runs an observation-only action
/// while the project and pose slot remain locked.
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

    Ok(Some(action(CurrentStaticCollisionView {
        certificate: data,
    })))
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

fn prepare_static_collision(
    capability: CurrentAppliedPoseCapability,
    limits: StaticCollisionLimits,
) -> Result<PreparedCurrentStaticCollision, CurrentStaticCollisionError> {
    if !detached_pose_capability_is_internally_consistent(&capability) {
        return Err(CurrentStaticCollisionError::InternalInconsistency);
    }
    let pose_claims = &capability.claims;
    let paper_thickness_mm = f64::from_bits(pose_claims.paper_thickness_bits);
    let geometry_proof = prove_static_collision_geometry(
        pose_claims.model.as_ref(),
        pose_claims.pose.as_ref(),
        paper_thickness_mm,
        limits,
    )
    .map_err(CurrentStaticCollisionError::GeometryBlocking)?;
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
        return Err(CurrentStaticCollisionError::InternalInconsistency);
    }
    Ok(prepared)
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
        && pose_claims
            .model
            .bind_pose(pose_claims.pose.as_ref())
            .is_ok()
        && geometry_proof.is_for_geometry(
            pose_claims.model.as_ref(),
            pose_claims.pose.as_ref(),
            f64::from_bits(claims.paper_thickness_bits),
        )
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
    use std::{marker::PhantomData, path::PathBuf, sync::Mutex};

    use ori_collision::{StaticCollisionError, prove_static_collision_geometry};
    use ori_core::{Command, create_rectangular_sheet};
    use ori_domain::{Edge, EdgeId, EdgeKind, Paper, Point2, VertexId};
    use ori_kinematics::{CanonicalHingeAngles, MaterialTreeKinematicsModel, TreeKinematicsLimits};

    use super::*;
    use crate::{
        ProjectState, applied_pose::NativePoseHingeAngleRequest, applied_pose::NativePoseRequest,
        commit_project_replacement, execute_command,
    };

    fn no_hinge_project_with_thickness(thickness_mm: f64) -> ProjectState {
        let sheet = create_rectangular_sheet(40.0, 30.0, false).expect("rectangle fixture");
        let (pattern, mut paper) = sheet.into_parts();
        paper.thickness_mm = thickness_mm;
        ProjectState::new_with_paper(pattern, paper)
    }

    fn no_hinge_project() -> ProjectState {
        no_hinge_project_with_thickness(Paper::default().thickness_mm)
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
        let state = AppState(Mutex::new(project));
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
        let state = AppState(Mutex::new(project));
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
        let other_state = AppState(Mutex::new(other_project));
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
        let state = AppState(Mutex::new(project));
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
        proof_identity_mismatch.geometry_proof = prove_static_collision_geometry(
            pose_claims.model.as_ref(),
            pose_claims.pose.as_ref(),
            -0.0,
            StaticCollisionLimits::default(),
        )
        .expect("second exact proof");
        assert!(matches!(
            mint_current_static_collision(&state, proof_identity_mismatch),
            Err(CurrentStaticCollisionError::InternalInconsistency)
        ));

        let mut pose_mismatch = prepared_from_current(&state);
        let pose_claims = &pose_mismatch.pose_capability.claims;
        let second_pose = pose_claims
            .model
            .solve(
                None,
                &CanonicalHingeAngles::new(Vec::new()).expect("empty angles"),
            )
            .expect("same-angle pose");
        let second_pose_proof = prove_static_collision_geometry(
            pose_claims.model.as_ref(),
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
        let positive_zero_proof = prove_static_collision_geometry(
            pose_claims.model.as_ref(),
            pose_claims.pose.as_ref(),
            0.0,
            StaticCollisionLimits::default(),
        )
        .expect("positive-zero proof");
        zero_sign_mismatch.claims.proof_identity = positive_zero_proof.clone();
        zero_sign_mismatch.geometry_proof = positive_zero_proof;
        assert!(matches!(
            mint_current_static_collision(&state, zero_sign_mismatch),
            Err(CurrentStaticCollisionError::InternalInconsistency)
        ));
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
        let state = AppState(Mutex::new(project));

        assert!(matches!(
            certify_current_static_collision(&state, StaticCollisionLimits::default()),
            Err(CurrentStaticCollisionError::GeometryBlocking(
                StaticCollisionError::PairEvidenceUnavailable {
                    expected_unordered_face_pairs: 1,
                },
            ))
        ));
    }
}
