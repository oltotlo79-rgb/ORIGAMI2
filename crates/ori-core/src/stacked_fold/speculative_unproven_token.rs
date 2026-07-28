use ori_collision::StackedFoldPathDiagnosticLimitsV1;
use ori_domain::{CreasePattern, EdgeId, FaceId, Paper, ProjectId};
use thiserror::Error;

use crate::{
    APPLIED_POSE_MODEL_ID_V1, AppliedPoseV1, MAX_REVISION, PreparedStackedFoldRequestedPoseV1,
    Revision, SpeculativeApproximateBlockingObservationV1, SpeculativeUnprovenFoldBindingV1,
    SpeculativeUnprovenFoldMetadataErrorV1, StackedFoldInitialLayerOrderV1,
    diagnose_stacked_fold_requested_path_with_initial_layer_order_v1,
};

const SPECULATIVE_TARGET_SEAL_FIXED_RECORDS_V1: usize = 4;

struct SpeculativeUnprovenTargetHingeSealV1 {
    edge: EdgeId,
    angle_degrees_bits: u64,
}

/// Private, native-only binding between one issued token and its exact target.
///
/// This seal deliberately implements neither `Clone` nor persistence traits.
/// It can leave this module only by being checked and destroyed together with
/// its containing one-shot token.
struct SpeculativeUnprovenTargetSealV1 {
    target_revision: Revision,
    target_geometry_fingerprint_sha256: [u8; 32],
    pose_model_id: &'static str,
    fixed_face: Option<FaceId>,
    hinge_angles: Vec<SpeculativeUnprovenTargetHingeSealV1>,
}

/// Native-only, one-shot permission to record one speculative stacked fold.
///
/// The token is deliberately not `Clone`, `Copy`, `Serialize`, or
/// `Deserialize`. The only public mutation entry that accepts it consumes it
/// directly; it has no public conversion into history metadata or any
/// geometry, pose, collision, or layer-order proof authority.
///
/// A speculative token cannot satisfy a serialization bound:
///
/// ```compile_fail
/// fn requires_serialize<T: serde::Serialize>() {}
/// requires_serialize::<ori_core::SpeculativeUnprovenFoldTokenV1>();
/// ```
///
/// It cannot be cloned for a second Apply:
///
/// ```compile_fail
/// fn requires_clone<T: Clone>() {}
/// requires_clone::<ori_core::SpeculativeUnprovenFoldTokenV1>();
/// ```
///
/// It cannot be converted into proven stacked-fold geometry authority:
///
/// ```compile_fail
/// fn forbidden(
///     token: ori_core::SpeculativeUnprovenFoldTokenV1,
/// ) -> ori_core::StackedFoldGeometryProofV1 {
///     token.into()
/// }
/// ```
///
/// Unauthenticated metadata and a caller-asserted observation cannot mint a
/// token; issuance requires an opaque prepared production pose and reruns the
/// bounded native path diagnostic:
///
/// ```compile_fail
/// let _ = ori_core::issue_speculative_unproven_fold_token_v1(
///     ori_domain::ProjectId::new(),
///     ori_domain::ProjectId::new(),
///     0,
///     [0_u8; 32],
///     1,
///     ori_domain::ProjectId::new(),
///     0.1,
///     ori_core::SpeculativeApproximateBlockingObservationV1::
///         no_blocking_sample_observed(),
/// );
/// ```
pub struct SpeculativeUnprovenFoldTokenV1 {
    binding: SpeculativeUnprovenFoldBindingV1,
    target_seal: SpeculativeUnprovenTargetSealV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SpeculativeUnprovenFoldTokenIssueErrorV1 {
    #[error(transparent)]
    InvalidMetadata(#[from] SpeculativeUnprovenFoldMetadataErrorV1),
    #[error("the production bounded path diagnostic could not be reproduced")]
    PathDiagnosticUnavailable,
    #[error("a continuously certified path must use certified Apply")]
    ContinuousPathCertified,
    #[error("the sampled path observation is empty or internally inconsistent")]
    InvalidSampledObservation,
    #[error("a sampled blocking pose forbids speculative token issuance")]
    ApproximateBlockingSampleObserved,
    #[error("the speculative target revision is invalid")]
    InvalidTargetRevision,
    #[error(
        "the speculative target seal angle count {actual} exceeds the supported maximum {maximum}"
    )]
    TargetSealResourceLimitExceeded { actual: usize, maximum: usize },
    #[error("the speculative target seal resource count overflowed")]
    TargetSealResourceCountOverflow,
    #[error("memory for the speculative target seal could not be reserved")]
    TargetSealAllocationFailed,
}

/// Issues a native one-shot token from an opaque production pose.
///
/// Core reruns the bounded collision diagnostic itself. A caller cannot turn
/// metadata, a hand-built binding, or a claimed nonblocking flag into Apply
/// authority.
pub fn issue_speculative_unproven_fold_token_v1(
    project_instance_id: ProjectId,
    requested: &PreparedStackedFoldRequestedPoseV1,
    initial_layer_order: &StackedFoldInitialLayerOrderV1,
    pose_generation: u64,
    request_generation_id: ProjectId,
    paper_thickness_mm: f64,
) -> Result<SpeculativeUnprovenFoldTokenV1, SpeculativeUnprovenFoldTokenIssueErrorV1> {
    let initial = requested.initial();
    let diagnostic = diagnose_stacked_fold_requested_path_with_initial_layer_order_v1(
        requested,
        paper_thickness_mm,
        StackedFoldPathDiagnosticLimitsV1::default(),
        initial_layer_order,
    )
    .map_err(|_| SpeculativeUnprovenFoldTokenIssueErrorV1::PathDiagnosticUnavailable)?;
    if diagnostic.continuous_certificate_model_id().is_some()
        || diagnostic.continuous_clearance_certified()
    {
        return Err(SpeculativeUnprovenFoldTokenIssueErrorV1::ContinuousPathCertified);
    }
    if diagnostic.first_sampled_blocking_angle_degrees().is_some() {
        return Err(SpeculativeUnprovenFoldTokenIssueErrorV1::ApproximateBlockingSampleObserved);
    }
    if diagnostic.sampled_pose_count() == 0
        || diagnostic.sampled_nonblocking_pose_count() != diagnostic.sampled_pose_count()
    {
        return Err(SpeculativeUnprovenFoldTokenIssueErrorV1::InvalidSampledObservation);
    }
    let lineage = initial.target().geometry().proof().lineage();
    if lineage.source_revision().checked_add(1) != Some(lineage.target_revision())
        || lineage.target_revision() > MAX_REVISION
    {
        return Err(SpeculativeUnprovenFoldTokenIssueErrorV1::InvalidTargetRevision);
    }
    let target_seal = SpeculativeUnprovenTargetSealV1::capture_requested_v1(requested)?;
    issue_from_validated_parts_v1(
        project_instance_id,
        lineage.identity_namespace(),
        lineage.source_revision(),
        lineage.source_fingerprint().0,
        pose_generation,
        request_generation_id,
        paper_thickness_mm,
        SpeculativeApproximateBlockingObservationV1::no_blocking_sample_observed(),
        target_seal,
    )
}

#[allow(clippy::too_many_arguments)]
fn issue_from_validated_parts_v1(
    project_instance_id: ProjectId,
    project_id: ProjectId,
    source_revision: Revision,
    source_geometry_fingerprint_sha256: [u8; 32],
    pose_generation: u64,
    request_generation_id: ProjectId,
    paper_thickness_mm: f64,
    approximate_blocking_observation: SpeculativeApproximateBlockingObservationV1,
    target_seal: SpeculativeUnprovenTargetSealV1,
) -> Result<SpeculativeUnprovenFoldTokenV1, SpeculativeUnprovenFoldTokenIssueErrorV1> {
    if matches!(
        approximate_blocking_observation,
        SpeculativeApproximateBlockingObservationV1::BlockingSampleObserved { .. }
    ) {
        return Err(SpeculativeUnprovenFoldTokenIssueErrorV1::ApproximateBlockingSampleObserved);
    }
    let binding = SpeculativeUnprovenFoldBindingV1::new(
        project_instance_id,
        project_id,
        source_revision,
        lowercase_sha256(source_geometry_fingerprint_sha256),
        pose_generation,
        request_generation_id,
        paper_thickness_mm,
        approximate_blocking_observation,
    )?;
    Ok(SpeculativeUnprovenFoldTokenV1 {
        binding,
        target_seal,
    })
}

impl SpeculativeUnprovenFoldTokenV1 {
    /// Reauthenticates every token premise against the locked live project.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn reauthenticates_v1(
        &self,
        project_instance_id: ProjectId,
        project_id: ProjectId,
        source_revision: Revision,
        source_geometry_fingerprint_sha256: [u8; 32],
        pose_generation: u64,
        request_generation_id: ProjectId,
        paper_thickness_bits: u64,
    ) -> bool {
        self.binding.project_instance_id() == project_instance_id
            && self.binding.project_id() == project_id
            && self.binding.source_revision() == source_revision
            && self.binding.source_geometry_fingerprint_sha256()
                == lowercase_sha256(source_geometry_fingerprint_sha256)
            && self.binding.pose_generation() == pose_generation
            && self.binding.request_generation_id() == request_generation_id
            && self.binding.paper_thickness_bits() == paper_thickness_bits
            && matches!(
                self.binding.approximate_blocking_observation(),
                SpeculativeApproximateBlockingObservationV1::NoBlockingSampleObserved
            )
    }

    /// Consumes the token and releases its persisted binding only when every
    /// caller-supplied target dimension matches the private issuance seal.
    ///
    /// The target comparison happens before the editor mutation path receives
    /// the binding, so every mismatch consumes the one-shot token while
    /// leaving the editor untouched.
    pub(crate) fn into_unproven_binding_for_target_v1(
        self,
        expected_source_revision: Revision,
        pattern: &CreasePattern,
        paper: &Paper,
        applied_pose: &AppliedPoseV1,
    ) -> Option<SpeculativeUnprovenFoldBindingV1> {
        let Self {
            binding,
            target_seal,
        } = self;
        target_seal
            .matches_apply_target_v1(expected_source_revision, pattern, paper, applied_pose)
            .then_some(binding)
    }
}

impl SpeculativeUnprovenTargetSealV1 {
    fn capture_requested_v1(
        requested: &PreparedStackedFoldRequestedPoseV1,
    ) -> Result<Self, SpeculativeUnprovenFoldTokenIssueErrorV1> {
        let geometry = requested.initial().target().geometry();
        let lineage = geometry.proof().lineage();
        let pose = requested.pose();
        let hinge_count = pose.hinge_angles().len();
        check_target_seal_resource_counts_v1(hinge_count)?;

        let mut hinge_angles = Vec::new();
        hinge_angles
            .try_reserve_exact(hinge_count)
            .map_err(|_| SpeculativeUnprovenFoldTokenIssueErrorV1::TargetSealAllocationFailed)?;
        hinge_angles.extend(pose.hinge_angles().iter().map(|angle| {
            SpeculativeUnprovenTargetHingeSealV1 {
                edge: angle.edge(),
                angle_degrees_bits: angle.angle_degrees().to_bits(),
            }
        }));
        Ok(Self {
            target_revision: lineage.target_revision(),
            target_geometry_fingerprint_sha256: lineage.target_fingerprint().0,
            pose_model_id: APPLIED_POSE_MODEL_ID_V1,
            fixed_face: pose.fixed_face(),
            hinge_angles,
        })
    }

    #[cfg(test)]
    fn capture_applied_target_v1(
        target_revision: Revision,
        pattern: &CreasePattern,
        paper: &Paper,
        applied_pose: &AppliedPoseV1,
    ) -> Result<Self, SpeculativeUnprovenFoldTokenIssueErrorV1> {
        let hinge_count = applied_pose.hinge_angles().len();
        check_target_seal_resource_counts_v1(hinge_count)?;
        let mut hinge_angles = Vec::new();
        hinge_angles
            .try_reserve_exact(hinge_count)
            .map_err(|_| SpeculativeUnprovenFoldTokenIssueErrorV1::TargetSealAllocationFailed)?;
        hinge_angles.extend(applied_pose.hinge_angles().iter().map(|angle| {
            SpeculativeUnprovenTargetHingeSealV1 {
                edge: angle.edge(),
                angle_degrees_bits: angle.angle_degrees().to_bits(),
            }
        }));
        Ok(Self {
            target_revision,
            target_geometry_fingerprint_sha256: ori_foldability::fold_model_fingerprint_v1(
                pattern, paper,
            )
            .0,
            pose_model_id: applied_pose.model_id(),
            fixed_face: applied_pose.fixed_face(),
            hinge_angles,
        })
    }

    fn matches_apply_target_v1(
        &self,
        expected_source_revision: Revision,
        pattern: &CreasePattern,
        paper: &Paper,
        applied_pose: &AppliedPoseV1,
    ) -> bool {
        expected_source_revision
            .checked_add(1)
            .filter(|revision| *revision <= MAX_REVISION)
            == Some(self.target_revision)
            && ori_foldability::fold_model_fingerprint_v1(pattern, paper).0
                == self.target_geometry_fingerprint_sha256
            && applied_pose.model_id() == self.pose_model_id
            && applied_pose.fixed_face() == self.fixed_face
            && applied_pose.hinge_angles().len() == self.hinge_angles.len()
            && applied_pose
                .hinge_angles()
                .iter()
                .zip(&self.hinge_angles)
                .all(|(actual, sealed)| {
                    actual.edge() == sealed.edge
                        && actual.angle_degrees().to_bits() == sealed.angle_degrees_bits
                })
    }
}

fn check_target_seal_resource_counts_v1(
    hinge_count: usize,
) -> Result<(), SpeculativeUnprovenFoldTokenIssueErrorV1> {
    hinge_count
        .checked_add(SPECULATIVE_TARGET_SEAL_FIXED_RECORDS_V1)
        .ok_or(SpeculativeUnprovenFoldTokenIssueErrorV1::TargetSealResourceCountOverflow)?;
    let maximum = ori_foldability::DEFAULT_MAX_HINGES;
    if hinge_count > maximum {
        return Err(
            SpeculativeUnprovenFoldTokenIssueErrorV1::TargetSealResourceLimitExceeded {
                actual: hinge_count,
                maximum,
            },
        );
    }
    Ok(())
}

#[cfg(test)]
pub(crate) fn issue_speculative_unproven_fold_token_for_test_v1(
    binding: SpeculativeUnprovenFoldBindingV1,
    target_revision: Revision,
    pattern: &CreasePattern,
    paper: &Paper,
    applied_pose: &AppliedPoseV1,
) -> Result<SpeculativeUnprovenFoldTokenV1, SpeculativeUnprovenFoldTokenIssueErrorV1> {
    binding.validate()?;
    let target_seal = SpeculativeUnprovenTargetSealV1::capture_applied_target_v1(
        target_revision,
        pattern,
        paper,
        applied_pose,
    )?;
    Ok(SpeculativeUnprovenFoldTokenV1 {
        binding,
        target_seal,
    })
}

fn lowercase_sha256(bytes: [u8; 32]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(64);
    for byte in bytes {
        output.push(char::from(DIGITS[usize::from(byte >> 4)]));
        output.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target_seal() -> SpeculativeUnprovenTargetSealV1 {
        SpeculativeUnprovenTargetSealV1 {
            target_revision: 8,
            target_geometry_fingerprint_sha256: [0xa5; 32],
            pose_model_id: APPLIED_POSE_MODEL_ID_V1,
            fixed_face: None,
            hinge_angles: Vec::new(),
        }
    }

    fn issue(
        observation: SpeculativeApproximateBlockingObservationV1,
    ) -> Result<SpeculativeUnprovenFoldTokenV1, SpeculativeUnprovenFoldTokenIssueErrorV1> {
        issue_from_validated_parts_v1(
            ProjectId::new(),
            ProjectId::new(),
            7,
            [0x5a; 32],
            11,
            ProjectId::new(),
            0.25,
            observation,
            target_seal(),
        )
    }

    #[test]
    fn blocking_observation_is_rejected_at_issuance() {
        let observation =
            SpeculativeApproximateBlockingObservationV1::blocking_sample_observed(45.0)
                .expect("valid blocking angle");
        assert!(matches!(
            issue(observation),
            Err(SpeculativeUnprovenFoldTokenIssueErrorV1::ApproximateBlockingSampleObserved)
        ));
    }

    #[test]
    fn every_live_binding_dimension_is_bit_exact() {
        let instance = ProjectId::new();
        let project = ProjectId::new();
        let request = ProjectId::new();
        let token = issue_from_validated_parts_v1(
            instance,
            project,
            7,
            [0x5a; 32],
            11,
            request,
            0.25,
            SpeculativeApproximateBlockingObservationV1::no_blocking_sample_observed(),
            target_seal(),
        )
        .expect("sound speculative token");
        let exact = (
            instance,
            project,
            7,
            [0x5a; 32],
            11,
            request,
            0.25_f64.to_bits(),
        );
        assert!(token.reauthenticates_v1(
            exact.0, exact.1, exact.2, exact.3, exact.4, exact.5, exact.6
        ));
        assert!(!token.reauthenticates_v1(
            ProjectId::new(),
            exact.1,
            exact.2,
            exact.3,
            exact.4,
            exact.5,
            exact.6,
        ));
        assert!(!token.reauthenticates_v1(
            exact.0,
            ProjectId::new(),
            exact.2,
            exact.3,
            exact.4,
            exact.5,
            exact.6,
        ));
        assert!(
            !token.reauthenticates_v1(exact.0, exact.1, 8, exact.3, exact.4, exact.5, exact.6,)
        );
        assert!(!token.reauthenticates_v1(
            exact.0, exact.1, exact.2, [0xa5; 32], exact.4, exact.5, exact.6,
        ));
        assert!(
            !token.reauthenticates_v1(exact.0, exact.1, exact.2, exact.3, 12, exact.5, exact.6,)
        );
        assert!(!token.reauthenticates_v1(
            exact.0,
            exact.1,
            exact.2,
            exact.3,
            exact.4,
            ProjectId::new(),
            exact.6,
        ));
        assert!(!token.reauthenticates_v1(
            exact.0,
            exact.1,
            exact.2,
            exact.3,
            exact.4,
            exact.5,
            exact.6 + 1,
        ));
    }

    #[test]
    fn target_seal_rejects_pose_model_and_order_substitution() {
        let sheet = crate::create_rectangular_sheet(80.0, 60.0, false).expect("rectangular target");
        let (pattern, paper) = sheet.into_parts();
        let mut faces = [FaceId::new(), FaceId::new(), FaceId::new()];
        faces.sort_by_key(FaceId::canonical_bytes);
        let mut hinges = [EdgeId::new(), EdgeId::new()];
        hinges.sort_by_key(EdgeId::canonical_bytes);
        let pose = crate::prepare_applied_pose_v1(
            &faces,
            &hinges,
            Some(faces[0]),
            &[(hinges[0], 45.0), (hinges[1], 90.0)],
            crate::AppliedPoseLimitsV1::default(),
        )
        .expect("canonical target pose");
        let mut seal =
            SpeculativeUnprovenTargetSealV1::capture_applied_target_v1(1, &pattern, &paper, &pose)
                .expect("target seal");
        assert!(seal.matches_apply_target_v1(0, &pattern, &paper, &pose));

        seal.hinge_angles.swap(0, 1);
        assert!(!seal.matches_apply_target_v1(0, &pattern, &paper, &pose));
        seal.hinge_angles.swap(0, 1);
        seal.pose_model_id = crate::CLOSED_GRAPH_APPLIED_POSE_MODEL_ID_V1;
        assert!(!seal.matches_apply_target_v1(0, &pattern, &paper, &pose));
    }

    #[test]
    fn target_seal_resource_failures_are_explicit() {
        assert!(matches!(
            check_target_seal_resource_counts_v1(usize::MAX),
            Err(SpeculativeUnprovenFoldTokenIssueErrorV1::TargetSealResourceCountOverflow)
        ));
        let over_limit = ori_foldability::DEFAULT_MAX_HINGES + 1;
        assert_eq!(
            check_target_seal_resource_counts_v1(over_limit),
            Err(
                SpeculativeUnprovenFoldTokenIssueErrorV1::TargetSealResourceLimitExceeded {
                    actual: over_limit,
                    maximum: ori_foldability::DEFAULT_MAX_HINGES,
                }
            )
        );
    }
}
