use std::sync::Arc;

use ori_kinematics::{
    MATERIAL_TREE_KINEMATICS_MODEL_ID, MaterialTreeKinematicsModel, MaterialTreePose,
};
use thiserror::Error;

use crate::TOPOLOGY_CONTACT_POLICY_V2;

/// Initial paper-thickness interpretation used by native collision geometry.
pub const CENTERED_MID_SURFACE_THICKNESS_MODEL_V1: &str = "centered_mid_surface_v1";

/// First opaque native static-collision geometry-proof format.
///
/// Version 1 initially admits only the complete zero-pair proof for a
/// no-hinge, single-material-face pose. Multi-face poses fail closed until the
/// native pair evidence generator, positive-thickness SAT and finite shared
/// hinge corridor are available.
///
/// This proof does not claim that the pose is current for a project. A
/// stronger authority boundary must bind the exact proof object to the exact
/// current-pose certificate and generation.
pub const NATIVE_STATIC_COLLISION_GEOMETRY_PROOF_V1: &str =
    "native_static_collision_geometry_proof_v1";

/// Hard bounds applied before a native static analysis may allocate or scan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StaticCollisionLimits {
    pub max_faces: usize,
    pub max_unordered_face_pairs: usize,
}

impl Default for StaticCollisionLimits {
    fn default() -> Self {
        Self {
            max_faces: 10_001,
            max_unordered_face_pairs: 50_000_000,
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
        "native pair evidence is not yet available for all {expected_unordered_face_pairs} unordered face pairs"
    )]
    PairEvidenceUnavailable {
        expected_unordered_face_pairs: usize,
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
}

/// Proves static collision geometry for one exact native material pose.
///
/// The current implementation intentionally succeeds only for the complete
/// zero-pair case: exactly one material face and no material hinge. A
/// multi-face pose returns a blocking error until all unordered pairs have
/// authenticated topology, intersection and finite-hinge evidence.
pub fn prove_static_collision_geometry(
    model: &MaterialTreeKinematicsModel,
    pose: &MaterialTreePose,
    paper_thickness_mm: f64,
    limits: StaticCollisionLimits,
) -> Result<NativeStaticCollisionGeometryProof, StaticCollisionError> {
    model
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

    if face_count != 1 || !pose.hinges().is_empty() {
        return Err(StaticCollisionError::PairEvidenceUnavailable {
            expected_unordered_face_pairs,
        });
    }

    Ok(NativeStaticCollisionGeometryProof {
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
        }),
    })
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
    use super::{StaticCollisionError, checked_unordered_pair_count};

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
}
