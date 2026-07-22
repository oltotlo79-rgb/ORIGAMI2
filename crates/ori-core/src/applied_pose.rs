//! Runtime semantic pose for the currently applied fold state.
//!
//! `AppliedPoseV1` is deliberately opaque. It is a validated semantic value,
//! not authority to mutate a project or proof that it belongs to the current
//! project revision.
//!
//! Its fields cannot be forged by downstream crates.
//!
//! ```compile_fail
//! use ori_core::AppliedPoseV1;
//!
//! fn inspect(pose: AppliedPoseV1) {
//!     let AppliedPoseV1 { fixed_face, .. } = pose;
//!     let _ = fixed_face;
//! }
//! ```
//!
//! Runtime poses deliberately do not implement persistence traits.
//!
//! ```compile_fail
//! use ori_core::AppliedPoseV1;
//!
//! fn require_serialize<T: serde::Serialize>() {}
//! require_serialize::<AppliedPoseV1>();
//! ```

use std::cmp::Ordering;

use ori_domain::{EdgeId, FaceId};
use thiserror::Error;

/// Stable semantic model identifier for the first applied-pose representation.
pub const APPLIED_POSE_MODEL_ID_V1: &str = "tree_absolute_hinge_angles_v1";
pub const CLOSED_GRAPH_APPLIED_POSE_MODEL_ID_V1: &str = "closed_graph_absolute_hinge_angles_v1";

/// Resource class checked while preparing an applied pose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppliedPoseResourceV1 {
    Faces,
    Hinges,
    AngleRecords,
    TotalRecords,
}

/// Explicit admission limits for an applied pose and its expected registries.
///
/// Every count is checked before the preparer allocates the owned angle
/// vector. Callers at a stronger trust boundary may choose smaller limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppliedPoseLimitsV1 {
    pub max_faces: usize,
    pub max_hinges: usize,
    pub max_angle_records: usize,
    pub max_total_records: usize,
}

impl Default for AppliedPoseLimitsV1 {
    fn default() -> Self {
        Self {
            max_faces: ori_foldability::DEFAULT_MAX_FACES,
            max_hinges: ori_foldability::DEFAULT_MAX_HINGES,
            max_angle_records: ori_foldability::DEFAULT_MAX_HINGES,
            max_total_records: ori_foldability::DEFAULT_MAX_TOTAL_RECORDS,
        }
    }
}

/// Fail-closed applied-pose admission error.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AppliedPoseErrorV1 {
    #[error("{resource:?} count {actual} exceeds the configured maximum {maximum}")]
    ResourceLimitExceeded {
        resource: AppliedPoseResourceV1,
        actual: usize,
        maximum: usize,
    },
    #[error("the combined applied-pose resource count overflowed")]
    ResourceCountOverflow,
    #[error("a tree pose requires at least one material face")]
    EmptyFaceRegistry,
    #[error("the hinge count cannot be incremented to derive a tree face count")]
    TreeFaceCountOverflow,
    #[error(
        "a tree pose with {hinges} hinges requires {expected_faces} faces, found {actual_faces}"
    )]
    TreeCardinalityMismatch {
        actual_faces: usize,
        hinges: usize,
        expected_faces: usize,
    },
    #[error("memory for {resource:?} could not be reserved")]
    AllocationFailed { resource: AppliedPoseResourceV1 },
    #[error("face {face:?} occurs more than once in the expected face registry")]
    DuplicateFace { face: FaceId },
    #[error("the expected face registry is not canonical: {previous:?} before {face:?}")]
    FaceRegistryNotCanonical { previous: FaceId, face: FaceId },
    #[error("hinge {edge:?} occurs more than once in the expected hinge registry")]
    DuplicateHinge { edge: EdgeId },
    #[error("the expected hinge registry is not canonical: {previous:?} before {edge:?}")]
    HingeRegistryNotCanonical { previous: EdgeId, edge: EdgeId },
    #[error("hinge {edge:?} occurs more than once in the applied angle vector")]
    DuplicateHingeAngle { edge: EdgeId },
    #[error("the applied angle vector is not canonical: {previous:?} before {edge:?}")]
    HingeAnglesNotCanonical { previous: EdgeId, edge: EdgeId },
    #[error("a pose with no hinges must not select a fixed face")]
    UnexpectedFixedFace,
    #[error("a pose with hinges requires a fixed face")]
    MissingFixedFace,
    #[error("the selected fixed face is not in the expected face registry")]
    UnknownFixedFace,
    #[error("the complete angle vector is missing hinge {edge:?}")]
    MissingHingeAngle { edge: EdgeId },
    #[error("the complete angle vector contains unexpected hinge {edge:?}")]
    ExtraHingeAngle { edge: EdgeId },
    #[error("hinge {edge:?} has a non-finite angle")]
    NonFiniteHingeAngle { edge: EdgeId },
    #[error("hinge {edge:?} has an angle outside the closed range 0 through 180 degrees")]
    HingeAngleOutOfRange { edge: EdgeId },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AppliedPoseModelV1 {
    TreeAbsoluteHingeAnglesV1,
    ClosedGraphAbsoluteHingeAnglesV1,
}

/// One canonical absolute hinge angle in an [`AppliedPoseV1`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AppliedHingeAngleV1 {
    edge: EdgeId,
    angle_degrees: f64,
}

impl AppliedHingeAngleV1 {
    #[must_use]
    pub const fn edge(&self) -> EdgeId {
        self.edge
    }

    #[must_use]
    pub const fn angle_degrees(&self) -> f64 {
        self.angle_degrees
    }
}

/// Opaque runtime semantic value for the currently applied fold pose.
///
/// This type is separate from persisted instruction poses. It carries neither
/// a project identity nor a revision and is therefore not mutation authority.
/// A stronger boundary must bind a prepared value to the current project,
/// topology, revision, and material kinematics before adopting it.
#[derive(Debug, Clone, PartialEq)]
pub struct AppliedPoseV1 {
    model: AppliedPoseModelV1,
    fixed_face: Option<FaceId>,
    hinge_angles: Vec<AppliedHingeAngleV1>,
}

impl AppliedPoseV1 {
    #[must_use]
    pub const fn model_id(&self) -> &'static str {
        match self.model {
            AppliedPoseModelV1::TreeAbsoluteHingeAnglesV1 => APPLIED_POSE_MODEL_ID_V1,
            AppliedPoseModelV1::ClosedGraphAbsoluteHingeAnglesV1 => {
                CLOSED_GRAPH_APPLIED_POSE_MODEL_ID_V1
            }
        }
    }

    #[must_use]
    pub const fn fixed_face(&self) -> Option<FaceId> {
        self.fixed_face
    }

    #[must_use]
    pub fn hinge_angles(&self) -> &[AppliedHingeAngleV1] {
        &self.hinge_angles
    }

    /// Duplicates this validated semantic pose without using `Vec::clone`.
    ///
    /// This is intended for authority commit paths that must report an
    /// allocation failure before mutating live editor or certificate state.
    ///
    /// # Errors
    ///
    /// Returns [`AppliedPoseErrorV1::AllocationFailed`] when storage for the
    /// complete hinge-angle vector cannot be reserved.
    pub fn try_clone(&self) -> Result<Self, AppliedPoseErrorV1> {
        let mut hinge_angles = Vec::new();
        hinge_angles
            .try_reserve_exact(self.hinge_angles.len())
            .map_err(|_| AppliedPoseErrorV1::AllocationFailed {
                resource: AppliedPoseResourceV1::AngleRecords,
            })?;
        hinge_angles.extend_from_slice(&self.hinge_angles);
        Ok(Self {
            model: self.model,
            fixed_face: self.fixed_face,
            hinge_angles,
        })
    }
}

/// Prepares a complete canonical semantic pose against explicit registries.
///
/// Registry validation proves internal completeness only. The returned value
/// is not evidence that the registries or face IDs belong to a live project;
/// native certificate code must establish that stronger binding separately.
pub fn prepare_applied_pose_v1(
    expected_faces: &[FaceId],
    expected_hinges: &[EdgeId],
    fixed_face: Option<FaceId>,
    hinge_angles: &[(EdgeId, f64)],
    limits: AppliedPoseLimitsV1,
) -> Result<AppliedPoseV1, AppliedPoseErrorV1> {
    prepare_applied_pose_inner(
        expected_faces,
        expected_hinges,
        fixed_face,
        hinge_angles,
        limits,
        AppliedPoseModelV1::TreeAbsoluteHingeAnglesV1,
    )
}

pub fn prepare_closed_graph_applied_pose_v1(
    expected_faces: &[FaceId],
    expected_hinges: &[EdgeId],
    fixed_face: FaceId,
    hinge_angles: &[(EdgeId, f64)],
    limits: AppliedPoseLimitsV1,
) -> Result<AppliedPoseV1, AppliedPoseErrorV1> {
    prepare_applied_pose_inner(
        expected_faces,
        expected_hinges,
        Some(fixed_face),
        hinge_angles,
        limits,
        AppliedPoseModelV1::ClosedGraphAbsoluteHingeAnglesV1,
    )
}

fn prepare_applied_pose_inner(
    expected_faces: &[FaceId],
    expected_hinges: &[EdgeId],
    fixed_face: Option<FaceId>,
    hinge_angles: &[(EdgeId, f64)],
    limits: AppliedPoseLimitsV1,
    model: AppliedPoseModelV1,
) -> Result<AppliedPoseV1, AppliedPoseErrorV1> {
    check_resource(
        AppliedPoseResourceV1::Faces,
        expected_faces.len(),
        limits.max_faces,
    )?;
    check_resource(
        AppliedPoseResourceV1::Hinges,
        expected_hinges.len(),
        limits.max_hinges,
    )?;
    check_resource(
        AppliedPoseResourceV1::AngleRecords,
        hinge_angles.len(),
        limits.max_angle_records,
    )?;
    check_resource(
        AppliedPoseResourceV1::TotalRecords,
        checked_total_records(
            expected_faces.len(),
            expected_hinges.len(),
            hinge_angles.len(),
        )?,
        limits.max_total_records,
    )?;

    validate_face_registry(expected_faces)?;
    validate_hinge_registry(expected_hinges)?;
    validate_angle_order(hinge_angles)?;
    if model == AppliedPoseModelV1::TreeAbsoluteHingeAnglesV1 {
        validate_tree_cardinality(expected_faces.len(), expected_hinges.len())?;
    } else if expected_faces.is_empty() || expected_hinges.is_empty() {
        return Err(AppliedPoseErrorV1::EmptyFaceRegistry);
    }

    match (expected_hinges.is_empty(), fixed_face) {
        (true, Some(_)) if expected_faces.len() != 1 => {
            return Err(AppliedPoseErrorV1::UnexpectedFixedFace);
        }
        (false, None) => return Err(AppliedPoseErrorV1::MissingFixedFace),
        _ => {}
    }
    if let Some(fixed_face) = fixed_face
        && expected_faces
            .binary_search_by_key(&fixed_face.canonical_bytes(), FaceId::canonical_bytes)
            .is_err()
    {
        return Err(AppliedPoseErrorV1::UnknownFixedFace);
    }

    validate_complete_hinge_vector(expected_hinges, hinge_angles)?;
    for &(edge, angle_degrees) in hinge_angles {
        if !angle_degrees.is_finite() {
            return Err(AppliedPoseErrorV1::NonFiniteHingeAngle { edge });
        }
        if !(0.0..=180.0).contains(&angle_degrees) {
            return Err(AppliedPoseErrorV1::HingeAngleOutOfRange { edge });
        }
    }

    let mut owned_angles = Vec::new();
    owned_angles
        .try_reserve_exact(hinge_angles.len())
        .map_err(|_| AppliedPoseErrorV1::AllocationFailed {
            resource: AppliedPoseResourceV1::AngleRecords,
        })?;
    owned_angles.extend(hinge_angles.iter().copied().map(|(edge, angle_degrees)| {
        AppliedHingeAngleV1 {
            edge,
            angle_degrees: if angle_degrees == 0.0 {
                0.0
            } else {
                angle_degrees
            },
        }
    }));

    Ok(AppliedPoseV1 {
        model,
        fixed_face,
        hinge_angles: owned_angles,
    })
}

fn check_resource(
    resource: AppliedPoseResourceV1,
    actual: usize,
    maximum: usize,
) -> Result<(), AppliedPoseErrorV1> {
    if actual > maximum {
        Err(AppliedPoseErrorV1::ResourceLimitExceeded {
            resource,
            actual,
            maximum,
        })
    } else {
        Ok(())
    }
}

fn checked_total_records(
    faces: usize,
    hinges: usize,
    angles: usize,
) -> Result<usize, AppliedPoseErrorV1> {
    faces
        .checked_add(hinges)
        .and_then(|total| total.checked_add(angles))
        .ok_or(AppliedPoseErrorV1::ResourceCountOverflow)
}

fn validate_tree_cardinality(faces: usize, hinges: usize) -> Result<(), AppliedPoseErrorV1> {
    if faces == 0 {
        return Err(AppliedPoseErrorV1::EmptyFaceRegistry);
    }
    let expected_faces = hinges
        .checked_add(1)
        .ok_or(AppliedPoseErrorV1::TreeFaceCountOverflow)?;
    if faces != expected_faces {
        return Err(AppliedPoseErrorV1::TreeCardinalityMismatch {
            actual_faces: faces,
            hinges,
            expected_faces,
        });
    }
    Ok(())
}

fn validate_face_registry(faces: &[FaceId]) -> Result<(), AppliedPoseErrorV1> {
    for pair in faces.windows(2) {
        match pair[0].canonical_bytes().cmp(&pair[1].canonical_bytes()) {
            Ordering::Less => {}
            Ordering::Equal => {
                return Err(AppliedPoseErrorV1::DuplicateFace { face: pair[1] });
            }
            Ordering::Greater => {
                return Err(AppliedPoseErrorV1::FaceRegistryNotCanonical {
                    previous: pair[0],
                    face: pair[1],
                });
            }
        }
    }
    Ok(())
}

fn validate_hinge_registry(hinges: &[EdgeId]) -> Result<(), AppliedPoseErrorV1> {
    for pair in hinges.windows(2) {
        match pair[0].canonical_bytes().cmp(&pair[1].canonical_bytes()) {
            Ordering::Less => {}
            Ordering::Equal => {
                return Err(AppliedPoseErrorV1::DuplicateHinge { edge: pair[1] });
            }
            Ordering::Greater => {
                return Err(AppliedPoseErrorV1::HingeRegistryNotCanonical {
                    previous: pair[0],
                    edge: pair[1],
                });
            }
        }
    }
    Ok(())
}

fn validate_angle_order(hinge_angles: &[(EdgeId, f64)]) -> Result<(), AppliedPoseErrorV1> {
    for pair in hinge_angles.windows(2) {
        match pair[0]
            .0
            .canonical_bytes()
            .cmp(&pair[1].0.canonical_bytes())
        {
            Ordering::Less => {}
            Ordering::Equal => {
                return Err(AppliedPoseErrorV1::DuplicateHingeAngle { edge: pair[1].0 });
            }
            Ordering::Greater => {
                return Err(AppliedPoseErrorV1::HingeAnglesNotCanonical {
                    previous: pair[0].0,
                    edge: pair[1].0,
                });
            }
        }
    }
    Ok(())
}

fn validate_complete_hinge_vector(
    expected_hinges: &[EdgeId],
    hinge_angles: &[(EdgeId, f64)],
) -> Result<(), AppliedPoseErrorV1> {
    let mut expected_index = 0;
    let mut actual_index = 0;
    while let (Some(expected), Some((actual, _))) = (
        expected_hinges.get(expected_index),
        hinge_angles.get(actual_index),
    ) {
        match expected.canonical_bytes().cmp(&actual.canonical_bytes()) {
            Ordering::Less => {
                return Err(AppliedPoseErrorV1::MissingHingeAngle { edge: *expected });
            }
            Ordering::Greater => {
                return Err(AppliedPoseErrorV1::ExtraHingeAngle { edge: *actual });
            }
            Ordering::Equal => {
                expected_index += 1;
                actual_index += 1;
            }
        }
    }
    if let Some(expected) = expected_hinges.get(expected_index) {
        return Err(AppliedPoseErrorV1::MissingHingeAngle { edge: *expected });
    }
    if let Some((actual, _)) = hinge_angles.get(actual_index) {
        return Err(AppliedPoseErrorV1::ExtraHingeAngle { edge: *actual });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use ori_domain::{EdgeId, FaceId, ProjectId};

    use super::*;

    fn sorted_faces(count: usize) -> Vec<FaceId> {
        let namespace = ProjectId::new();
        let mut faces = (0..count)
            .map(|index| FaceId::derive_v5(namespace, &index.to_be_bytes()))
            .collect::<Vec<_>>();
        faces.sort_by_key(FaceId::canonical_bytes);
        faces
    }

    fn sorted_edges(count: usize) -> Vec<EdgeId> {
        let mut edges = (0..count).map(|_| EdgeId::new()).collect::<Vec<_>>();
        edges.sort_by_key(EdgeId::canonical_bytes);
        edges
    }

    fn limits() -> AppliedPoseLimitsV1 {
        AppliedPoseLimitsV1 {
            max_faces: 4,
            max_hinges: 3,
            max_angle_records: 3,
            max_total_records: 10,
        }
    }

    #[test]
    fn prepares_complete_canonical_pose_and_normalizes_negative_zero() {
        let faces = sorted_faces(3);
        let hinges = sorted_edges(2);
        let pose = prepare_applied_pose_v1(
            &faces,
            &hinges,
            Some(faces[1]),
            &[(hinges[0], -0.0), (hinges[1], 180.0)],
            limits(),
        )
        .expect("valid complete pose");

        assert_eq!(pose.model_id(), APPLIED_POSE_MODEL_ID_V1);
        assert_eq!(pose.fixed_face(), Some(faces[1]));
        assert_eq!(pose.hinge_angles().len(), 2);
        assert_eq!(pose.hinge_angles()[0].edge(), hinges[0]);
        assert_eq!(pose.hinge_angles()[0].angle_degrees().to_bits(), 0);
        assert_eq!(pose.hinge_angles()[1].edge(), hinges[1]);
        assert_eq!(pose.hinge_angles()[1].angle_degrees(), 180.0);
    }

    #[test]
    fn prepares_closed_graph_pose_without_tree_cardinality() {
        let faces = sorted_faces(3);
        let hinges = sorted_edges(3);
        let pose = prepare_closed_graph_applied_pose_v1(
            &faces,
            &hinges,
            faces[0],
            &[(hinges[0], 30.0), (hinges[1], 60.0), (hinges[2], 90.0)],
            limits(),
        )
        .expect("closed graph semantic pose");
        assert_eq!(pose.model_id(), CLOSED_GRAPH_APPLIED_POSE_MODEL_ID_V1);
        assert_eq!(pose.fixed_face(), Some(faces[0]));

        assert!(
            prepare_applied_pose_v1(
                &faces,
                &hinges,
                Some(faces[0]),
                &[(hinges[0], 30.0), (hinges[1], 60.0), (hinges[2], 90.0),],
                limits(),
            )
            .is_err()
        );
    }

    #[test]
    fn fallible_clone_preserves_the_complete_pose_and_uses_independent_storage() {
        let faces = sorted_faces(2);
        let hinge = sorted_edges(1)[0];
        let pose = prepare_applied_pose_v1(
            &faces,
            &[hinge],
            Some(faces[0]),
            &[(hinge, 135.0)],
            limits(),
        )
        .expect("valid pose");

        let cloned = pose.try_clone().expect("fallible clone");

        assert_eq!(cloned, pose);
        assert_ne!(
            cloned.hinge_angles().as_ptr(),
            pose.hinge_angles().as_ptr(),
            "a semantic duplicate must own an independent angle allocation"
        );
    }

    #[test]
    fn single_face_pose_accepts_canonical_anchor_and_folded_pose_requires_one() {
        let faces = sorted_faces(3);
        let material_faces = &faces[..2];
        let face = faces[0];
        let hinge = sorted_edges(1)[0];

        let planar =
            prepare_applied_pose_v1(&[face], &[], None, &[], limits()).expect("planar pose");
        assert_eq!(planar.fixed_face(), None);
        assert!(planar.hinge_angles().is_empty());

        let anchored = prepare_applied_pose_v1(&[face], &[], Some(face), &[], limits())
            .expect("single material face may carry its canonical semantic anchor");
        assert_eq!(anchored.fixed_face(), Some(face));
        assert_eq!(
            prepare_applied_pose_v1(material_faces, &[hinge], None, &[(hinge, 10.0)], limits()),
            Err(AppliedPoseErrorV1::MissingFixedFace)
        );
        assert_eq!(
            prepare_applied_pose_v1(
                material_faces,
                &[hinge],
                Some(faces[2]),
                &[(hinge, 10.0)],
                limits(),
            ),
            Err(AppliedPoseErrorV1::UnknownFixedFace)
        );
    }

    #[test]
    fn registries_and_angle_vector_must_be_strictly_canonical() {
        let faces = sorted_faces(2);
        let hinges = sorted_edges(2);

        assert!(matches!(
            prepare_applied_pose_v1(
                &[faces[1], faces[0]],
                &hinges,
                Some(faces[0]),
                &[(hinges[0], 0.0), (hinges[1], 0.0)],
                limits(),
            ),
            Err(AppliedPoseErrorV1::FaceRegistryNotCanonical { .. })
        ));
        assert!(matches!(
            prepare_applied_pose_v1(
                &[faces[0], faces[0]],
                &hinges,
                Some(faces[0]),
                &[(hinges[0], 0.0), (hinges[1], 0.0)],
                limits(),
            ),
            Err(AppliedPoseErrorV1::DuplicateFace { .. })
        ));
        assert!(matches!(
            prepare_applied_pose_v1(
                &faces,
                &[hinges[1], hinges[0]],
                Some(faces[0]),
                &[(hinges[0], 0.0), (hinges[1], 0.0)],
                limits(),
            ),
            Err(AppliedPoseErrorV1::HingeRegistryNotCanonical { .. })
        ));
        assert!(matches!(
            prepare_applied_pose_v1(
                &faces,
                &[hinges[0], hinges[0]],
                Some(faces[0]),
                &[(hinges[0], 0.0), (hinges[0], 0.0)],
                limits(),
            ),
            Err(AppliedPoseErrorV1::DuplicateHinge { .. })
        ));
        assert!(matches!(
            prepare_applied_pose_v1(
                &faces,
                &hinges,
                Some(faces[0]),
                &[(hinges[1], 0.0), (hinges[0], 0.0)],
                limits(),
            ),
            Err(AppliedPoseErrorV1::HingeAnglesNotCanonical { .. })
        ));
        assert!(matches!(
            prepare_applied_pose_v1(
                &faces,
                &hinges,
                Some(faces[0]),
                &[(hinges[0], 0.0), (hinges[0], 0.0)],
                limits(),
            ),
            Err(AppliedPoseErrorV1::DuplicateHingeAngle { .. })
        ));
    }

    #[test]
    fn complete_vector_rejects_missing_and_extra_hinges() {
        let faces = sorted_faces(3);
        let hinges = sorted_edges(3);

        assert_eq!(
            prepare_applied_pose_v1(
                &faces,
                &hinges[..2],
                Some(faces[0]),
                &[(hinges[0], 10.0)],
                limits(),
            ),
            Err(AppliedPoseErrorV1::MissingHingeAngle { edge: hinges[1] })
        );
        assert_eq!(
            prepare_applied_pose_v1(
                &faces,
                &hinges[..2],
                Some(faces[0]),
                &[(hinges[0], 10.0), (hinges[1], 20.0), (hinges[2], 30.0)],
                limits(),
            ),
            Err(AppliedPoseErrorV1::ExtraHingeAngle { edge: hinges[2] })
        );
    }

    #[test]
    fn angles_must_be_finite_and_inside_closed_range() {
        let faces = sorted_faces(2);
        let face = faces[0];
        let hinge = sorted_edges(1)[0];

        for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert_eq!(
                prepare_applied_pose_v1(
                    &faces,
                    &[hinge],
                    Some(face),
                    &[(hinge, invalid)],
                    limits(),
                ),
                Err(AppliedPoseErrorV1::NonFiniteHingeAngle { edge: hinge })
            );
        }
        for invalid in [-f64::MIN_POSITIVE, f64::from_bits(180.0_f64.to_bits() + 1)] {
            assert_eq!(
                prepare_applied_pose_v1(
                    &faces,
                    &[hinge],
                    Some(face),
                    &[(hinge, invalid)],
                    limits(),
                ),
                Err(AppliedPoseErrorV1::HingeAngleOutOfRange { edge: hinge })
            );
        }
    }

    #[test]
    fn every_resource_limit_is_checked_before_structural_validation() {
        let faces = sorted_faces(2);
        let hinges = sorted_edges(2);
        let malformed_faces = [faces[1], faces[0]];
        let malformed_hinges = [hinges[1], hinges[0]];
        let malformed_angles = [(hinges[1], f64::NAN), (hinges[0], f64::NAN)];

        let cases = [
            (
                AppliedPoseLimitsV1 {
                    max_faces: 1,
                    ..limits()
                },
                AppliedPoseResourceV1::Faces,
                2,
                1,
            ),
            (
                AppliedPoseLimitsV1 {
                    max_hinges: 1,
                    ..limits()
                },
                AppliedPoseResourceV1::Hinges,
                2,
                1,
            ),
            (
                AppliedPoseLimitsV1 {
                    max_angle_records: 1,
                    ..limits()
                },
                AppliedPoseResourceV1::AngleRecords,
                2,
                1,
            ),
            (
                AppliedPoseLimitsV1 {
                    max_total_records: 5,
                    ..limits()
                },
                AppliedPoseResourceV1::TotalRecords,
                6,
                5,
            ),
        ];

        for (case_limits, resource, actual, maximum) in cases {
            assert_eq!(
                prepare_applied_pose_v1(
                    &malformed_faces,
                    &malformed_hinges,
                    Some(faces[0]),
                    &malformed_angles,
                    case_limits,
                ),
                Err(AppliedPoseErrorV1::ResourceLimitExceeded {
                    resource,
                    actual,
                    maximum,
                })
            );
        }
    }

    #[test]
    fn total_record_overflow_fails_closed_before_validation() {
        assert_eq!(
            checked_total_records(usize::MAX, 1, 0),
            Err(AppliedPoseErrorV1::ResourceCountOverflow)
        );
    }

    #[test]
    fn tree_face_hinge_cardinality_is_fail_closed() {
        let faces = sorted_faces(2);
        let hinge = sorted_edges(1)[0];

        assert_eq!(
            prepare_applied_pose_v1(&[], &[], None, &[], limits()),
            Err(AppliedPoseErrorV1::EmptyFaceRegistry)
        );
        assert_eq!(
            prepare_applied_pose_v1(&faces, &[], None, &[], limits()),
            Err(AppliedPoseErrorV1::TreeCardinalityMismatch {
                actual_faces: 2,
                hinges: 0,
                expected_faces: 1,
            })
        );
        assert_eq!(
            prepare_applied_pose_v1(
                &[faces[0]],
                &[hinge],
                Some(faces[0]),
                &[(hinge, 0.0)],
                limits()
            ),
            Err(AppliedPoseErrorV1::TreeCardinalityMismatch {
                actual_faces: 1,
                hinges: 1,
                expected_faces: 2,
            })
        );
        assert_eq!(
            validate_tree_cardinality(1, usize::MAX),
            Err(AppliedPoseErrorV1::TreeFaceCountOverflow)
        );
    }
}
