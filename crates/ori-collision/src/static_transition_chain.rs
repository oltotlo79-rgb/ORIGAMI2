//! Bounded, observation-only storage for exact positive-thickness static poses.
//!
//! A chain proves only that its finitely many stored poses each passed the
//! native static classifier.  It says nothing about the open intervals
//! between poses and therefore never grants continuous-motion or mutation
//! authority.

use std::sync::Arc;

use ori_kinematics::{MaterialTreeKinematicsModel, MaterialTreePose};
use thiserror::Error;

use crate::NativeStaticCollisionGeometryProof;

pub const POSITIVE_THICKNESS_TRANSITION_STATIC_CHAIN_MODEL_ID_V1: &str =
    "positive_thickness_transition_static_chain_v1";
pub const MAX_POSITIVE_THICKNESS_STATIC_CHAIN_TRANSITIONS_V1: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum PositiveThicknessTransitionStaticChainErrorV1 {
    #[error("static transition chain exceeds its resource bound")]
    ResourceLimitExceeded,
    #[error("static transition chain has no transition")]
    Empty,
    #[error("static transition chain input is issuer-, order-, or proof-mismatched")]
    InputMismatch,
    #[error("static transition chain requires finite positive thickness")]
    InvalidThickness,
}

struct StaticPoseRecordV1 {
    pose: MaterialTreePose,
    proof: NativeStaticCollisionGeometryProof,
    angle_bits: Vec<u64>,
}

struct StaticChainInnerV1 {
    model: MaterialTreeKinematicsModel,
    thickness_bits: u64,
    face_count: usize,
    hinge_count: usize,
    expected_face_pairs: usize,
    analyzed_face_pairs: usize,
    expected_triangle_pairs: usize,
    analyzed_triangle_pairs: usize,
    poses: Vec<StaticPoseRecordV1>,
}

/// Opaque record of a bounded sequence of independently proven static poses.
///
/// Cloning preserves identity. This type intentionally implements neither
/// `Serialize` nor any API that exposes its constituent proof handles.
#[derive(Clone)]
pub struct PositiveThicknessTransitionStaticChainV1 {
    inner: Arc<StaticChainInnerV1>,
}

impl std::fmt::Debug for PositiveThicknessTransitionStaticChainV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PositiveThicknessTransitionStaticChainV1")
            .field("model_id", &self.model_id())
            .field("transition_count", &self.transition_count())
            .field("pose_count", &self.pose_count())
            .finish_non_exhaustive()
    }
}

impl PositiveThicknessTransitionStaticChainV1 {
    #[must_use]
    pub const fn model_id(&self) -> &'static str {
        POSITIVE_THICKNESS_TRANSITION_STATIC_CHAIN_MODEL_ID_V1
    }

    #[must_use]
    pub fn same_chain(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }

    #[must_use]
    pub fn transition_count(&self) -> usize {
        self.inner.poses.len() - 1
    }

    #[must_use]
    pub fn pose_count(&self) -> usize {
        self.inner.poses.len()
    }

    #[must_use]
    pub fn face_count(&self) -> usize {
        self.inner.face_count
    }
    #[must_use]
    pub fn hinge_count(&self) -> usize {
        self.inner.hinge_count
    }
    #[must_use]
    pub fn expected_unordered_face_pairs(&self) -> usize {
        self.inner.expected_face_pairs
    }
    #[must_use]
    pub fn analyzed_unordered_face_pairs(&self) -> usize {
        self.inner.analyzed_face_pairs
    }
    #[must_use]
    pub fn expected_triangle_pairs(&self) -> usize {
        self.inner.expected_triangle_pairs
    }
    #[must_use]
    pub fn analyzed_triangle_pairs(&self) -> usize {
        self.inner.analyzed_triangle_pairs
    }

    /// Static samples cannot prove clearance between samples.
    #[must_use]
    pub const fn authorizes_continuous_motion(&self) -> bool {
        false
    }

    #[must_use]
    pub const fn authorizes_project_mutation(&self) -> bool {
        false
    }

    /// Revalidates every identity, canonical angle vector, proof and count.
    #[must_use]
    pub fn is_for(
        &self,
        model: &MaterialTreeKinematicsModel,
        source: &MaterialTreePose,
        target: &MaterialTreePose,
        paper_thickness_mm: f64,
        poses: &[MaterialTreePose],
        proofs: &[NativeStaticCollisionGeometryProof],
    ) -> bool {
        model.owns_pose(source)
            && self.inner.model.owns_pose(source)
            && self.inner.thickness_bits == paper_thickness_mm.to_bits()
            && poses.len() == self.inner.poses.len()
            && proofs.len() == poses.len()
            && poses.first().is_some_and(|pose| pose.same_instance(source))
            && poses.last().is_some_and(|pose| pose.same_instance(target))
            && validate_records(model, paper_thickness_mm, poses, proofs)
                .is_some_and(|counts| counts == self.counts())
            && self
                .inner
                .poses
                .iter()
                .zip(poses)
                .zip(proofs)
                .all(|((stored, pose), proof)| {
                    stored.pose.same_instance(pose)
                        && stored.proof.same_proof(proof)
                        && stored.angle_bits == canonical_angle_bits(pose)
                })
    }

    fn counts(&self) -> [usize; 6] {
        [
            self.inner.face_count,
            self.inner.hinge_count,
            self.inner.expected_face_pairs,
            self.inner.analyzed_face_pairs,
            self.inner.expected_triangle_pairs,
            self.inner.analyzed_triangle_pairs,
        ]
    }
}

/// Seals `N + 1` exact static poses into a non-authorizing chain (`1 <= N <= 64`).
pub fn issue_positive_thickness_transition_static_chain_v1(
    model: &MaterialTreeKinematicsModel,
    source: &MaterialTreePose,
    paper_thickness_mm: f64,
    poses: &[MaterialTreePose],
    proofs: &[NativeStaticCollisionGeometryProof],
) -> Result<PositiveThicknessTransitionStaticChainV1, PositiveThicknessTransitionStaticChainErrorV1>
{
    // This cardinality gate deliberately precedes every allocation and scan.
    let transition_count = poses
        .len()
        .checked_sub(1)
        .ok_or(PositiveThicknessTransitionStaticChainErrorV1::Empty)?;
    if transition_count == 0 {
        return Err(PositiveThicknessTransitionStaticChainErrorV1::Empty);
    }
    if transition_count > MAX_POSITIVE_THICKNESS_STATIC_CHAIN_TRANSITIONS_V1 {
        return Err(PositiveThicknessTransitionStaticChainErrorV1::ResourceLimitExceeded);
    }
    if proofs.len() != poses.len() || !paper_thickness_mm.is_finite() || paper_thickness_mm <= 0.0 {
        return Err(
            if !paper_thickness_mm.is_finite() || paper_thickness_mm <= 0.0 {
                PositiveThicknessTransitionStaticChainErrorV1::InvalidThickness
            } else {
                PositiveThicknessTransitionStaticChainErrorV1::InputMismatch
            },
        );
    }
    if !poses[0].same_instance(source) {
        return Err(PositiveThicknessTransitionStaticChainErrorV1::InputMismatch);
    }
    let counts = validate_records(model, paper_thickness_mm, poses, proofs)
        .ok_or(PositiveThicknessTransitionStaticChainErrorV1::InputMismatch)?;
    let records = poses
        .iter()
        .zip(proofs)
        .map(|(pose, proof)| StaticPoseRecordV1 {
            pose: pose.clone(),
            proof: proof.clone(),
            angle_bits: canonical_angle_bits(pose),
        })
        .collect();
    Ok(PositiveThicknessTransitionStaticChainV1 {
        inner: Arc::new(StaticChainInnerV1 {
            model: model.clone(),
            thickness_bits: paper_thickness_mm.to_bits(),
            face_count: counts[0],
            hinge_count: counts[1],
            expected_face_pairs: counts[2],
            analyzed_face_pairs: counts[3],
            expected_triangle_pairs: counts[4],
            analyzed_triangle_pairs: counts[5],
            poses: records,
        }),
    })
}

fn canonical_angle_bits(pose: &MaterialTreePose) -> Vec<u64> {
    pose.hinge_angles()
        .iter()
        .map(|angle| angle.angle_degrees().to_bits())
        .collect()
}

fn validate_records(
    model: &MaterialTreeKinematicsModel,
    thickness: f64,
    poses: &[MaterialTreePose],
    proofs: &[NativeStaticCollisionGeometryProof],
) -> Option<[usize; 6]> {
    let first = proofs.first()?;
    let counts = [
        first.face_count(),
        poses.first()?.hinges().len(),
        first.expected_unordered_face_pairs(),
        first.analyzed_unordered_face_pairs(),
        first.expected_triangle_pairs(),
        first.analyzed_triangle_pairs(),
    ];
    poses
        .iter()
        .zip(proofs)
        .all(|(pose, proof)| {
            model.owns_pose(pose)
                && pose.face_ids() == poses[0].face_ids()
                && pose.hinges() == poses[0].hinges()
                && pose.hinge_angles().len() == counts[1]
                && pose
                    .hinges()
                    .iter()
                    .zip(pose.hinge_angles())
                    .all(|(hinge, angle)| hinge.edge() == angle.edge())
                && proof.is_for_geometry(model, pose, thickness)
                && [
                    proof.face_count(),
                    pose.hinges().len(),
                    proof.expected_unordered_face_pairs(),
                    proof.analyzed_unordered_face_pairs(),
                    proof.expected_triangle_pairs(),
                    proof.analyzed_triangle_pairs(),
                ] == counts
                && counts[2] == counts[3]
                && counts[4] == counts[5]
        })
        .then_some(counts)
}
