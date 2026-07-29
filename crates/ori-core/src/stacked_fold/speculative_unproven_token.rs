use std::sync::Arc;

use ori_collision::StackedFoldPathDiagnosticLimitsV1;
#[cfg(test)]
use ori_domain::Paper;
use ori_domain::{
    BeginnerDesignProfileV1, CreasePattern, DEFAULT_PROJECT_LAYER_ID, EdgeLayerAssignmentV1,
    InstructionHingeAngle, InstructionPose, InstructionPoseModel, InstructionStep,
    InstructionStepId, InstructionTimeline, InstructionVisual, MAX_INSTRUCTION_HINGE_RECORDS,
    MAX_INSTRUCTION_HINGES_PER_STEP, MAX_INSTRUCTION_STEPS, MAX_LAYER_EDGE_ASSIGNMENTS,
    MIN_INSTRUCTION_DURATION_MS, ProjectId, ProjectLayerDocumentV1, validate_instruction_timeline,
    validate_project_layer_document_against_pattern_v1, validate_project_layer_document_v1,
};
use thiserror::Error;

mod fallible_clone;

pub(crate) use fallible_clone::{
    try_clone_beginner_design_profile_v1, try_clone_crease_pattern_v1,
    try_clone_instruction_timeline_v1, try_clone_paper_v1, try_clone_project_layer_document_v1,
};
use fallible_clone::{try_clone_instruction_step_v1, try_owned_string};

use super::PreparedStackedFoldRequestIssuerSealV1;
use crate::{
    AppliedPoseErrorV1, AppliedPoseLimitsV1, AppliedPoseV1, MAX_REVISION,
    PreparedStackedFoldRequestedPoseV1, PreparedStackedFoldSourcePoseResourceV1, Revision,
    SourceEdgeSubdivisionV1, SpeculativeApproximateBlockingObservationV1,
    SpeculativeUnprovenFoldBindingV1, SpeculativeUnprovenFoldMetadataErrorV1,
    StackedFoldDocumentCommandV1, StackedFoldInitialLayerOrderV1,
    diagnose_stacked_fold_requested_path_with_initial_layer_order_v1, prepare_applied_pose_v1,
};

const SPECULATIVE_TARGET_SEAL_FIXED_RECORDS_V1: usize = 4;

/// Private, native-only binding between one issued token, its source editor,
/// and the complete command it may execute.
///
/// This seal deliberately implements neither `Clone` nor persistence traits.
/// It can leave this module only by being checked and destroyed together with
/// its containing one-shot token.
struct SpeculativeUnprovenTargetSealV1 {
    editor_instance_anchor: Arc<()>,
    source_applied_pose: Option<AppliedPoseV1>,
    target_revision: Revision,
    command: StackedFoldDocumentCommandV1,
    applied_pose: AppliedPoseV1,
    prepared_request_issuer_seal: Option<PreparedStackedFoldRequestIssuerSealV1>,
}

/// Crate-private inputs for native speculative-token issuance.
///
/// Keeping the complete issuance context together makes it harder for internal
/// callers to accidentally reorder independent identity, generation, and live
/// document arguments.
pub(crate) struct SpeculativeUnprovenFoldTokenIssueInputV1<'a> {
    pub(crate) editor_instance_anchor: Arc<()>,
    pub(crate) source_applied_pose: Option<&'a AppliedPoseV1>,
    pub(crate) source_instruction_timeline: &'a InstructionTimeline,
    pub(crate) source_project_layers: &'a ProjectLayerDocumentV1,
    pub(crate) source_beginner_design_profile: &'a BeginnerDesignProfileV1,
    pub(crate) project_instance_id: ProjectId,
    pub(crate) requested: &'a PreparedStackedFoldRequestedPoseV1,
    pub(crate) initial_layer_order: &'a StackedFoldInitialLayerOrderV1,
    pub(crate) pose_generation: u64,
    pub(crate) request_generation_id: ProjectId,
    pub(crate) paper_thickness_mm: f64,
}

struct ValidatedSpeculativeUnprovenFoldTokenPartsV1 {
    project_instance_id: ProjectId,
    project_id: ProjectId,
    source_revision: Revision,
    source_geometry_fingerprint_sha256: [u8; 32],
    pose_generation: u64,
    request_generation_id: ProjectId,
    paper_thickness_mm: f64,
    approximate_blocking_observation: SpeculativeApproximateBlockingObservationV1,
    target_seal: SpeculativeUnprovenTargetSealV1,
}

#[cfg(test)]
pub(crate) struct SpeculativeUnprovenFoldAppliedTargetInputV1<'a> {
    pub(crate) editor_instance_anchor: Arc<()>,
    pub(crate) source_applied_pose: Option<&'a AppliedPoseV1>,
    pub(crate) target_revision: Revision,
    pub(crate) pattern: &'a CreasePattern,
    pub(crate) paper: &'a Paper,
    pub(crate) instruction_timeline: &'a InstructionTimeline,
    pub(crate) project_layers: &'a ProjectLayerDocumentV1,
    pub(crate) beginner_design_profile: &'a BeginnerDesignProfileV1,
    pub(crate) applied_pose: &'a AppliedPoseV1,
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
/// token. Issuance is exposed only as an [`crate::EditorState`] method, which
/// binds the complete target command to that exact live editor instance and
/// reruns the bounded native path diagnostic.
pub struct SpeculativeUnprovenFoldTokenV1 {
    binding: SpeculativeUnprovenFoldBindingV1,
    target_seal: SpeculativeUnprovenTargetSealV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SpeculativeUnprovenFoldTokenIssueErrorV1 {
    #[error(transparent)]
    InvalidMetadata(#[from] SpeculativeUnprovenFoldMetadataErrorV1),
    #[error("the prepared speculative source revision is stale")]
    SourceRevisionMismatch,
    #[error("the prepared speculative source geometry is stale")]
    SourceGeometryFingerprintMismatch,
    #[error("the diagnostic paper thickness does not match the live editor")]
    SourcePaperThicknessMismatch,
    #[error("the prepared paper presentation does not match the live editor")]
    SourcePaperPresentationMismatch,
    #[error("the target no longer preserves the live paper-edge ratio reference")]
    TargetLengthDisplayReferenceInvalid,
    #[error("the prepared initial pose does not match the live editor's semantic source pose")]
    SourceAppliedPoseMismatch,
    #[error("the live source pose could not be reconstructed for speculative issuance")]
    SourceAppliedPoseReconstructionUnavailable,
    #[error(
        "the live source-pose {resource:?} count {actual} exceeds the supported maximum {maximum}"
    )]
    SourceAppliedPoseResourceLimitExceeded {
        resource: PreparedStackedFoldSourcePoseResourceV1,
        actual: usize,
        maximum: usize,
    },
    #[error("the live source-pose {resource:?} count overflowed")]
    SourceAppliedPoseResourceCountOverflow {
        resource: PreparedStackedFoldSourcePoseResourceV1,
    },
    #[error("memory for the live source-pose {resource:?} could not be reserved")]
    SourceAppliedPoseAllocationFailed {
        resource: PreparedStackedFoldSourcePoseResourceV1,
    },
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
    #[error(
        "the speculative target instruction step count {actual} exceeds the supported maximum {maximum}"
    )]
    TargetInstructionTimelineStepLimitExceeded { actual: usize, maximum: usize },
    #[error(
        "the speculative target instruction hinge-record count {actual} exceeds the supported maximum {maximum}"
    )]
    TargetInstructionTimelineHingeRecordLimitExceeded { actual: usize, maximum: usize },
    #[error("the speculative instruction step identity collides with the live timeline")]
    TargetInstructionTimelineStepIdCollision,
    #[error("the derived speculative target instruction timeline is invalid")]
    InvalidTargetInstructionTimeline,
    #[error(
        "the transported target layer-assignment count {actual} exceeds the supported maximum {maximum}"
    )]
    TargetProjectLayerAssignmentLimitExceeded { actual: usize, maximum: usize },
    #[error("the speculative fold would modify an edge on a locked project layer")]
    LockedSourceProjectLayerWouldBeModified,
    #[error("the derived speculative target project-layer document is invalid")]
    InvalidTargetProjectLayers,
    #[error("the speculative target semantic pose is invalid: {0}")]
    InvalidTargetPose(#[from] AppliedPoseErrorV1),
}

/// Issues a native one-shot token from an opaque production pose.
///
/// Core reruns the bounded collision diagnostic itself. A caller cannot turn
/// metadata, a hand-built binding, or a claimed nonblocking flag into Apply
/// authority.
pub(crate) fn issue_speculative_unproven_fold_token_v1(
    input: SpeculativeUnprovenFoldTokenIssueInputV1<'_>,
) -> Result<SpeculativeUnprovenFoldTokenV1, SpeculativeUnprovenFoldTokenIssueErrorV1> {
    let SpeculativeUnprovenFoldTokenIssueInputV1 {
        editor_instance_anchor,
        source_applied_pose,
        source_instruction_timeline,
        source_project_layers,
        source_beginner_design_profile,
        project_instance_id,
        requested,
        initial_layer_order,
        pose_generation,
        request_generation_id,
        paper_thickness_mm,
    } = input;
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
    let target_seal = SpeculativeUnprovenTargetSealV1::capture_requested_v1(
        editor_instance_anchor,
        source_applied_pose,
        source_instruction_timeline,
        source_project_layers,
        source_beginner_design_profile,
        requested,
    )?;
    issue_from_validated_parts_v1(ValidatedSpeculativeUnprovenFoldTokenPartsV1 {
        project_instance_id,
        project_id: lineage.identity_namespace(),
        source_revision: lineage.source_revision(),
        source_geometry_fingerprint_sha256: lineage.source_fingerprint().0,
        pose_generation,
        request_generation_id,
        paper_thickness_mm,
        approximate_blocking_observation:
            SpeculativeApproximateBlockingObservationV1::no_blocking_sample_observed(),
        target_seal,
    })
}

fn issue_from_validated_parts_v1(
    parts: ValidatedSpeculativeUnprovenFoldTokenPartsV1,
) -> Result<SpeculativeUnprovenFoldTokenV1, SpeculativeUnprovenFoldTokenIssueErrorV1> {
    let ValidatedSpeculativeUnprovenFoldTokenPartsV1 {
        project_instance_id,
        project_id,
        source_revision,
        source_geometry_fingerprint_sha256,
        pose_generation,
        request_generation_id,
        paper_thickness_mm,
        approximate_blocking_observation,
        target_seal,
    } = parts;
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
        try_lowercase_sha256(source_geometry_fingerprint_sha256)?,
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
    /// Checks the caller-visible binding dimensions retained by this token.
    ///
    /// This metadata check does not authenticate the private editor-instance
    /// anchor, captured source pose, or complete target seal, and grants no
    /// Apply authority. The consuming Apply path independently verifies those
    /// native-only premises before releasing the target command.
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
            && lowercase_sha256_matches(
                source_geometry_fingerprint_sha256,
                self.binding.source_geometry_fingerprint_sha256(),
            )
            && self.binding.pose_generation() == pose_generation
            && self.binding.request_generation_id() == request_generation_id
            && self.binding.paper_thickness_bits() == paper_thickness_bits
            && matches!(
                self.binding.approximate_blocking_observation(),
                SpeculativeApproximateBlockingObservationV1::NoBlockingSampleObserved
            )
    }

    /// Consumes the token and releases its binding plus the complete target
    /// command only when the source is the exact editor instance and runtime
    /// pose against which issuance occurred.
    ///
    /// No target document is accepted at consumption time, so a downstream
    /// caller has no substitution surface for pattern, paper, timeline,
    /// layers, face registry, or applied pose.
    pub(crate) fn into_authorized_target_v1(
        self,
        editor_instance_anchor: &Arc<()>,
        expected_source_revision: Revision,
        source_applied_pose: Option<&AppliedPoseV1>,
    ) -> Option<(
        SpeculativeUnprovenFoldBindingV1,
        StackedFoldDocumentCommandV1,
        AppliedPoseV1,
        Option<PreparedStackedFoldRequestIssuerSealV1>,
    )> {
        let Self {
            binding,
            target_seal,
        } = self;
        target_seal.into_authorized_target_v1(
            editor_instance_anchor,
            expected_source_revision,
            source_applied_pose,
            binding,
        )
    }
}

impl SpeculativeUnprovenTargetSealV1 {
    fn capture_requested_v1(
        editor_instance_anchor: Arc<()>,
        source_applied_pose: Option<&AppliedPoseV1>,
        source_instruction_timeline: &InstructionTimeline,
        source_project_layers: &ProjectLayerDocumentV1,
        source_beginner_design_profile: &BeginnerDesignProfileV1,
        requested: &PreparedStackedFoldRequestedPoseV1,
    ) -> Result<Self, SpeculativeUnprovenFoldTokenIssueErrorV1> {
        let geometry = requested.initial().target().geometry();
        let lineage = geometry.proof().lineage();
        let pose = requested.pose();
        let hinge_count = pose.hinge_angles().len();
        check_target_seal_resource_counts_v1(hinge_count)?;

        let mut hinge_ids = Vec::new();
        hinge_ids
            .try_reserve_exact(pose.hinges().len())
            .map_err(|_| SpeculativeUnprovenFoldTokenIssueErrorV1::TargetSealAllocationFailed)?;
        hinge_ids.extend(pose.hinges().iter().map(|hinge| hinge.edge()));
        let mut hinge_angles = Vec::new();
        hinge_angles
            .try_reserve_exact(hinge_count)
            .map_err(|_| SpeculativeUnprovenFoldTokenIssueErrorV1::TargetSealAllocationFailed)?;
        hinge_angles.extend(
            pose.hinge_angles()
                .iter()
                .map(|angle| (angle.edge(), angle.angle_degrees())),
        );
        let applied_pose = prepare_applied_pose_v1(
            pose.face_ids(),
            &hinge_ids,
            pose.fixed_face(),
            &hinge_angles,
            AppliedPoseLimitsV1::default(),
        )?;
        let mut persisted_hinge_angles = Vec::new();
        persisted_hinge_angles
            .try_reserve_exact(hinge_count)
            .map_err(|_| SpeculativeUnprovenFoldTokenIssueErrorV1::TargetSealAllocationFailed)?;
        persisted_hinge_angles.extend(hinge_angles.iter().map(|(edge, angle_degrees)| {
            InstructionHingeAngle {
                edge: *edge,
                angle_degrees: *angle_degrees,
            }
        }));
        let persisted_pose = InstructionPose {
            model: InstructionPoseModel::AbsoluteHingeAnglesV1,
            source_model_fingerprint: try_lowercase_sha256(lineage.target_fingerprint().0)?,
            fixed_face: pose.fixed_face(),
            hinge_angles: persisted_hinge_angles,
        };
        let candidate = geometry.candidate();
        let instruction_timeline = append_speculative_instruction_step_v1(
            source_instruction_timeline,
            persisted_pose,
            InstructionStepId::new(),
        )?;
        let project_layers = transport_project_layers_to_target_v1(
            source_project_layers,
            geometry.proof().source_edges(),
            &candidate.pattern,
        )?;
        let beginner_design_profile =
            try_clone_beginner_design_profile_v1(source_beginner_design_profile)?;
        let command = StackedFoldDocumentCommandV1::new(
            try_clone_crease_pattern_v1(&candidate.pattern)?,
            try_clone_paper_v1(&candidate.paper)?,
            instruction_timeline,
            project_layers,
            // Every nested profile buffer has already been reserved fallibly.
            // Stable Rust does not expose a fallible `Box<T>` constructor, so
            // only this final fixed-size box follows the global allocator's
            // process-level allocation-failure contract.
            Box::new(beginner_design_profile),
        );
        Ok(Self {
            editor_instance_anchor,
            source_applied_pose: source_applied_pose
                .map(AppliedPoseV1::try_clone)
                .transpose()?,
            target_revision: lineage.target_revision(),
            command,
            applied_pose,
            prepared_request_issuer_seal: Some(PreparedStackedFoldRequestIssuerSealV1::capture(
                requested,
            )),
        })
    }

    #[cfg(test)]
    fn capture_applied_target_v1(
        input: SpeculativeUnprovenFoldAppliedTargetInputV1<'_>,
    ) -> Result<Self, SpeculativeUnprovenFoldTokenIssueErrorV1> {
        let SpeculativeUnprovenFoldAppliedTargetInputV1 {
            editor_instance_anchor,
            source_applied_pose,
            target_revision,
            pattern,
            paper,
            instruction_timeline,
            project_layers,
            beginner_design_profile,
            applied_pose,
        } = input;
        let hinge_count = applied_pose.hinge_angles().len();
        check_target_seal_resource_counts_v1(hinge_count)?;
        let beginner_design_profile =
            try_clone_beginner_design_profile_v1(beginner_design_profile)?;
        Ok(Self {
            editor_instance_anchor,
            source_applied_pose: source_applied_pose
                .map(AppliedPoseV1::try_clone)
                .transpose()?,
            target_revision,
            command: StackedFoldDocumentCommandV1::new(
                try_clone_crease_pattern_v1(pattern)?,
                try_clone_paper_v1(paper)?,
                try_clone_instruction_timeline_v1(instruction_timeline)?,
                try_clone_project_layer_document_v1(project_layers)?,
                // See `capture_requested_v1`: all nested allocations are
                // fallible; only the final fixed-size box uses `Box::new`.
                Box::new(beginner_design_profile),
            ),
            applied_pose: applied_pose.try_clone()?,
            // This constructor exists only for editor unit tests that exercise
            // document/history mutation without preparing native kinematics.
            // Public certification rejects a ticket without an exact request
            // issuer seal.
            prepared_request_issuer_seal: None,
        })
    }

    fn into_authorized_target_v1(
        self,
        editor_instance_anchor: &Arc<()>,
        expected_source_revision: Revision,
        source_applied_pose: Option<&AppliedPoseV1>,
        binding: SpeculativeUnprovenFoldBindingV1,
    ) -> Option<(
        SpeculativeUnprovenFoldBindingV1,
        StackedFoldDocumentCommandV1,
        AppliedPoseV1,
        Option<PreparedStackedFoldRequestIssuerSealV1>,
    )> {
        let source_matches = Arc::ptr_eq(&self.editor_instance_anchor, editor_instance_anchor)
            && self.source_applied_pose.as_ref() == source_applied_pose;
        let revision_matches = expected_source_revision
            .checked_add(1)
            .filter(|revision| *revision <= MAX_REVISION)
            == Some(self.target_revision);
        (source_matches && revision_matches).then_some((
            binding,
            self.command,
            self.applied_pose,
            self.prepared_request_issuer_seal,
        ))
    }
}

fn append_speculative_instruction_step_v1(
    source: &InstructionTimeline,
    persisted_pose: InstructionPose,
    step_id: InstructionStepId,
) -> Result<InstructionTimeline, SpeculativeUnprovenFoldTokenIssueErrorV1> {
    let source_hinge_record_count = source.steps.iter().try_fold(0_usize, |total, step| {
        total
            .checked_add(step.pose.hinge_angles.len())
            .ok_or(SpeculativeUnprovenFoldTokenIssueErrorV1::TargetSealResourceCountOverflow)
    })?;
    check_target_instruction_timeline_resource_counts_v1(
        source.steps.len(),
        source_hinge_record_count,
        persisted_pose.hinge_angles.len(),
    )?;
    if source.steps.iter().any(|step| step.id == step_id) {
        return Err(
            SpeculativeUnprovenFoldTokenIssueErrorV1::TargetInstructionTimelineStepIdCollision,
        );
    }
    validate_instruction_timeline(source)
        .map_err(|_| SpeculativeUnprovenFoldTokenIssueErrorV1::InvalidTargetInstructionTimeline)?;

    let target_step_count = source
        .steps
        .len()
        .checked_add(1)
        .ok_or(SpeculativeUnprovenFoldTokenIssueErrorV1::TargetSealResourceCountOverflow)?;
    let mut steps = Vec::new();
    steps
        .try_reserve_exact(target_step_count)
        .map_err(|_| SpeculativeUnprovenFoldTokenIssueErrorV1::TargetSealAllocationFailed)?;
    for step in &source.steps {
        steps.push(try_clone_instruction_step_v1(step)?);
    }
    let title = try_owned_string("Stacked fold (awaiting proof)")?;
    steps.push(InstructionStep {
        id: step_id,
        title,
        description: String::new(),
        caution: String::new(),
        duration_ms: MIN_INSTRUCTION_DURATION_MS,
        visual: InstructionVisual::default(),
        pose: persisted_pose,
    });
    let target = InstructionTimeline { steps };
    validate_instruction_timeline(&target)
        .map_err(|_| SpeculativeUnprovenFoldTokenIssueErrorV1::InvalidTargetInstructionTimeline)?;
    Ok(target)
}

fn check_target_instruction_timeline_resource_counts_v1(
    source_step_count: usize,
    source_hinge_record_count: usize,
    target_hinge_record_count: usize,
) -> Result<(), SpeculativeUnprovenFoldTokenIssueErrorV1> {
    let target_step_count = source_step_count
        .checked_add(1)
        .ok_or(SpeculativeUnprovenFoldTokenIssueErrorV1::TargetSealResourceCountOverflow)?;
    if target_step_count > MAX_INSTRUCTION_STEPS {
        return Err(
            SpeculativeUnprovenFoldTokenIssueErrorV1::TargetInstructionTimelineStepLimitExceeded {
                actual: target_step_count,
                maximum: MAX_INSTRUCTION_STEPS,
            },
        );
    }

    let target_total_hinge_record_count = source_hinge_record_count
        .checked_add(target_hinge_record_count)
        .ok_or(SpeculativeUnprovenFoldTokenIssueErrorV1::TargetSealResourceCountOverflow)?;
    if target_total_hinge_record_count > MAX_INSTRUCTION_HINGE_RECORDS {
        return Err(
            SpeculativeUnprovenFoldTokenIssueErrorV1::TargetInstructionTimelineHingeRecordLimitExceeded {
                actual: target_total_hinge_record_count,
                maximum: MAX_INSTRUCTION_HINGE_RECORDS,
            },
        );
    }
    Ok(())
}

fn transport_project_layers_to_target_v1(
    source: &ProjectLayerDocumentV1,
    source_subdivisions: &[SourceEdgeSubdivisionV1],
    target_pattern: &CreasePattern,
) -> Result<ProjectLayerDocumentV1, SpeculativeUnprovenFoldTokenIssueErrorV1> {
    validate_project_layer_document_v1(source)
        .map_err(|_| SpeculativeUnprovenFoldTokenIssueErrorV1::InvalidTargetProjectLayers)?;

    let default_layer = source
        .layers
        .iter()
        .find(|layer| layer.id == DEFAULT_PROJECT_LAYER_ID)
        .ok_or(SpeculativeUnprovenFoldTokenIssueErrorV1::InvalidTargetProjectLayers)?;
    if default_layer.locked {
        return Err(
            SpeculativeUnprovenFoldTokenIssueErrorV1::LockedSourceProjectLayerWouldBeModified,
        );
    }

    let mut previous_source_edge = None;
    for subdivision in source_subdivisions {
        let source_edge = subdivision.source_edge();
        let source_edge_bytes = source_edge.canonical_bytes();
        if source_edge_bytes == [0; 16]
            || previous_source_edge.is_some_and(|previous| previous >= source_edge_bytes)
        {
            return Err(SpeculativeUnprovenFoldTokenIssueErrorV1::InvalidTargetProjectLayers);
        }
        previous_source_edge = Some(source_edge_bytes);

        let target_edges = subdivision.target_edges();
        if target_edges.is_empty()
            || target_edges
                .iter()
                .any(|edge| edge.canonical_bytes() == [0; 16])
            || target_edges
                .windows(2)
                .any(|pair| pair[0].canonical_bytes() >= pair[1].canonical_bytes())
            || target_edges
                .binary_search_by_key(&source_edge_bytes, |edge| edge.canonical_bytes())
                .is_err()
        {
            return Err(SpeculativeUnprovenFoldTokenIssueErrorV1::InvalidTargetProjectLayers);
        }

        if target_edges != [source_edge] {
            let source_layer = source.layer_for_edge(source_edge);
            let layer = source
                .layers
                .iter()
                .find(|layer| layer.id == source_layer)
                .ok_or(SpeculativeUnprovenFoldTokenIssueErrorV1::InvalidTargetProjectLayers)?;
            if layer.locked {
                return Err(
                    SpeculativeUnprovenFoldTokenIssueErrorV1::LockedSourceProjectLayerWouldBeModified,
                );
            }
        }
    }

    let mut target_assignment_count = 0_usize;
    for assignment in &source.edge_assignments {
        let subdivision_index = source_subdivisions
            .binary_search_by_key(&assignment.edge.canonical_bytes(), |subdivision| {
                subdivision.source_edge().canonical_bytes()
            })
            .map_err(|_| SpeculativeUnprovenFoldTokenIssueErrorV1::InvalidTargetProjectLayers)?;
        target_assignment_count = checked_target_layer_assignment_count_v1(
            target_assignment_count,
            source_subdivisions[subdivision_index].target_edges().len(),
        )?;
    }

    let mut layers = Vec::new();
    layers
        .try_reserve_exact(source.layers.len())
        .map_err(|_| SpeculativeUnprovenFoldTokenIssueErrorV1::TargetSealAllocationFailed)?;
    for layer in &source.layers {
        layers.push(ori_domain::LayerRecordV1 {
            id: layer.id,
            name: try_owned_string(&layer.name)?,
            content_kind: layer.content_kind,
            visible: layer.visible,
            locked: layer.locked,
            opacity: layer.opacity,
        });
    }

    let mut edge_assignments = Vec::new();
    edge_assignments
        .try_reserve_exact(target_assignment_count)
        .map_err(|_| SpeculativeUnprovenFoldTokenIssueErrorV1::TargetSealAllocationFailed)?;
    for assignment in &source.edge_assignments {
        let subdivision_index = source_subdivisions
            .binary_search_by_key(&assignment.edge.canonical_bytes(), |subdivision| {
                subdivision.source_edge().canonical_bytes()
            })
            .map_err(|_| SpeculativeUnprovenFoldTokenIssueErrorV1::InvalidTargetProjectLayers)?;
        edge_assignments.extend(
            source_subdivisions[subdivision_index]
                .target_edges()
                .iter()
                .copied()
                .map(|edge| EdgeLayerAssignmentV1 {
                    edge,
                    layer: assignment.layer,
                }),
        );
    }
    edge_assignments.sort_unstable_by_key(|assignment| assignment.edge.canonical_bytes());
    if edge_assignments
        .windows(2)
        .any(|pair| pair[0].edge == pair[1].edge)
    {
        return Err(SpeculativeUnprovenFoldTokenIssueErrorV1::InvalidTargetProjectLayers);
    }

    let target = ProjectLayerDocumentV1 {
        schema_version: source.schema_version,
        layers,
        edge_assignments,
    };
    validate_project_layer_document_against_pattern_v1(&target, target_pattern)
        .map_err(|_| SpeculativeUnprovenFoldTokenIssueErrorV1::InvalidTargetProjectLayers)?;
    Ok(target)
}

fn checked_target_layer_assignment_count_v1(
    current: usize,
    additional: usize,
) -> Result<usize, SpeculativeUnprovenFoldTokenIssueErrorV1> {
    let actual = current
        .checked_add(additional)
        .ok_or(SpeculativeUnprovenFoldTokenIssueErrorV1::TargetSealResourceCountOverflow)?;
    if actual > MAX_LAYER_EDGE_ASSIGNMENTS {
        return Err(
            SpeculativeUnprovenFoldTokenIssueErrorV1::TargetProjectLayerAssignmentLimitExceeded {
                actual,
                maximum: MAX_LAYER_EDGE_ASSIGNMENTS,
            },
        );
    }
    Ok(actual)
}

fn check_target_seal_resource_counts_v1(
    hinge_count: usize,
) -> Result<(), SpeculativeUnprovenFoldTokenIssueErrorV1> {
    hinge_count
        .checked_add(SPECULATIVE_TARGET_SEAL_FIXED_RECORDS_V1)
        .ok_or(SpeculativeUnprovenFoldTokenIssueErrorV1::TargetSealResourceCountOverflow)?;
    let maximum = ori_foldability::DEFAULT_MAX_HINGES.min(MAX_INSTRUCTION_HINGES_PER_STEP);
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
    target: SpeculativeUnprovenFoldAppliedTargetInputV1<'_>,
) -> Result<SpeculativeUnprovenFoldTokenV1, SpeculativeUnprovenFoldTokenIssueErrorV1> {
    binding.validate()?;
    let target_seal = SpeculativeUnprovenTargetSealV1::capture_applied_target_v1(target)?;
    Ok(SpeculativeUnprovenFoldTokenV1 {
        binding,
        target_seal,
    })
}

fn try_lowercase_sha256(
    bytes: [u8; 32],
) -> Result<String, SpeculativeUnprovenFoldTokenIssueErrorV1> {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::new();
    output
        .try_reserve_exact(64)
        .map_err(|_| SpeculativeUnprovenFoldTokenIssueErrorV1::TargetSealAllocationFailed)?;
    for byte in bytes {
        output.push(char::from(DIGITS[usize::from(byte >> 4)]));
        output.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    Ok(output)
}

fn lowercase_sha256_matches(bytes: [u8; 32], encoded: &str) -> bool {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    encoded.len() == 64
        && bytes.iter().enumerate().all(|(index, byte)| {
            encoded.as_bytes()[index * 2] == DIGITS[usize::from(byte >> 4)]
                && encoded.as_bytes()[index * 2 + 1] == DIGITS[usize::from(byte & 0x0f)]
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ori_domain::{
        Edge, EdgeId, EdgeKind, LayerContentKindV1, LayerId, LayerRecordV1, VertexId,
    };

    fn instruction_pose() -> InstructionPose {
        InstructionPose {
            model: InstructionPoseModel::AbsoluteHingeAnglesV1,
            source_model_fingerprint: "00".repeat(32),
            fixed_face: None,
            hinge_angles: Vec::new(),
        }
    }

    fn instruction_step(id: InstructionStepId) -> InstructionStep {
        InstructionStep {
            id,
            title: "Source step".to_owned(),
            description: String::new(),
            caution: String::new(),
            duration_ms: MIN_INSTRUCTION_DURATION_MS,
            visual: InstructionVisual::default(),
            pose: instruction_pose(),
        }
    }

    fn source_edge_subdivisions(
        mut records: Vec<(EdgeId, Vec<EdgeId>)>,
    ) -> Vec<SourceEdgeSubdivisionV1> {
        records.sort_unstable_by_key(|(source_edge, _)| source_edge.canonical_bytes());
        records
            .into_iter()
            .map(|(source_edge, mut target_edges)| {
                target_edges.sort_unstable_by_key(EdgeId::canonical_bytes);
                SourceEdgeSubdivisionV1 {
                    source_edge,
                    target_edges,
                }
            })
            .collect()
    }

    fn pattern_with_edges(edges: &[EdgeId]) -> CreasePattern {
        CreasePattern {
            vertices: Vec::new(),
            edges: edges
                .iter()
                .copied()
                .map(|id| Edge {
                    id,
                    start: VertexId::new(),
                    end: VertexId::new(),
                    kind: EdgeKind::Mountain,
                })
                .collect(),
        }
    }

    fn project_layers_with_assignment(
        source_edge: EdgeId,
        assignment_layer_locked: bool,
    ) -> ProjectLayerDocumentV1 {
        let assignment_layer = LayerId::new();
        ProjectLayerDocumentV1 {
            schema_version: ProjectLayerDocumentV1::default().schema_version,
            layers: vec![
                LayerRecordV1::default_crease_pattern(),
                LayerRecordV1 {
                    id: assignment_layer,
                    name: "Fold details".to_owned(),
                    content_kind: LayerContentKindV1::CreasePattern,
                    visible: false,
                    locked: assignment_layer_locked,
                    opacity: 0.5,
                },
            ],
            edge_assignments: vec![EdgeLayerAssignmentV1 {
                edge: source_edge,
                layer: assignment_layer,
            }],
        }
    }

    fn target_seal() -> SpeculativeUnprovenTargetSealV1 {
        let sheet = crate::create_rectangular_sheet(80.0, 60.0, false)
            .expect("rectangular target document");
        let (pattern, paper) = sheet.into_parts();
        let face = ori_domain::FaceId::new();
        let pose = crate::prepare_applied_pose_v1(
            &[face],
            &[],
            Some(face),
            &[],
            crate::AppliedPoseLimitsV1::default(),
        )
        .expect("single-face target pose");
        SpeculativeUnprovenTargetSealV1::capture_applied_target_v1(
            SpeculativeUnprovenFoldAppliedTargetInputV1 {
                editor_instance_anchor: Arc::new(()),
                source_applied_pose: None,
                target_revision: 8,
                pattern: &pattern,
                paper: &paper,
                instruction_timeline: &InstructionTimeline::default(),
                project_layers: &ProjectLayerDocumentV1::default(),
                beginner_design_profile: &BeginnerDesignProfileV1::default(),
                applied_pose: &pose,
            },
        )
        .expect("target seal")
    }

    fn issue(
        observation: SpeculativeApproximateBlockingObservationV1,
    ) -> Result<SpeculativeUnprovenFoldTokenV1, SpeculativeUnprovenFoldTokenIssueErrorV1> {
        issue_from_validated_parts_v1(ValidatedSpeculativeUnprovenFoldTokenPartsV1 {
            project_instance_id: ProjectId::new(),
            project_id: ProjectId::new(),
            source_revision: 7,
            source_geometry_fingerprint_sha256: [0x5a; 32],
            pose_generation: 11,
            request_generation_id: ProjectId::new(),
            paper_thickness_mm: 0.25,
            approximate_blocking_observation: observation,
            target_seal: target_seal(),
        })
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
        let token = issue_from_validated_parts_v1(ValidatedSpeculativeUnprovenFoldTokenPartsV1 {
            project_instance_id: instance,
            project_id: project,
            source_revision: 7,
            source_geometry_fingerprint_sha256: [0x5a; 32],
            pose_generation: 11,
            request_generation_id: request,
            paper_thickness_mm: 0.25,
            approximate_blocking_observation:
                SpeculativeApproximateBlockingObservationV1::no_blocking_sample_observed(),
            target_seal: target_seal(),
        })
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
    fn target_seal_requires_the_exact_editor_anchor_and_source_pose() {
        let binding = SpeculativeUnprovenFoldBindingV1::new(
            ProjectId::new(),
            ProjectId::new(),
            7,
            "5a".repeat(32),
            11,
            ProjectId::new(),
            0.25,
            SpeculativeApproximateBlockingObservationV1::no_blocking_sample_observed(),
        )
        .expect("binding");
        let seal = target_seal();
        assert!(
            seal.into_authorized_target_v1(&Arc::new(()), 7, None, binding)
                .is_none()
        );

        let binding = SpeculativeUnprovenFoldBindingV1::new(
            ProjectId::new(),
            ProjectId::new(),
            7,
            "5a".repeat(32),
            11,
            ProjectId::new(),
            0.25,
            SpeculativeApproximateBlockingObservationV1::no_blocking_sample_observed(),
        )
        .expect("binding");
        let seal = target_seal();
        let anchor = seal.editor_instance_anchor.clone();
        let face = ori_domain::FaceId::new();
        let source_pose = crate::prepare_applied_pose_v1(
            &[face],
            &[],
            Some(face),
            &[],
            crate::AppliedPoseLimitsV1::default(),
        )
        .expect("source pose");
        assert!(
            seal.into_authorized_target_v1(&anchor, 7, Some(&source_pose), binding)
                .is_none()
        );

        let binding = SpeculativeUnprovenFoldBindingV1::new(
            ProjectId::new(),
            ProjectId::new(),
            7,
            "5a".repeat(32),
            11,
            ProjectId::new(),
            0.25,
            SpeculativeApproximateBlockingObservationV1::no_blocking_sample_observed(),
        )
        .expect("binding");
        let seal = target_seal();
        let anchor = seal.editor_instance_anchor.clone();
        assert!(
            seal.into_authorized_target_v1(&anchor, 7, None, binding)
                .is_some()
        );
    }

    #[test]
    fn target_seal_resource_failures_are_explicit() {
        assert!(matches!(
            check_target_seal_resource_counts_v1(usize::MAX),
            Err(SpeculativeUnprovenFoldTokenIssueErrorV1::TargetSealResourceCountOverflow)
        ));
        let maximum = ori_foldability::DEFAULT_MAX_HINGES.min(MAX_INSTRUCTION_HINGES_PER_STEP);
        assert_eq!(check_target_seal_resource_counts_v1(maximum), Ok(()));
        let over_limit = maximum + 1;
        assert_eq!(
            check_target_seal_resource_counts_v1(over_limit),
            Err(
                SpeculativeUnprovenFoldTokenIssueErrorV1::TargetSealResourceLimitExceeded {
                    actual: over_limit,
                    maximum,
                }
            )
        );
    }

    #[test]
    fn speculative_instruction_step_is_validated_before_sealing() {
        let source_id = InstructionStepId::new();
        let target_id = InstructionStepId::new();
        let source = InstructionTimeline {
            steps: vec![instruction_step(source_id)],
        };

        let target = append_speculative_instruction_step_v1(&source, instruction_pose(), target_id)
            .expect("valid derived timeline");

        assert_eq!(target.steps.len(), 2);
        assert_eq!(target.steps[0], source.steps[0]);
        assert_eq!(target.steps[1].id, target_id);
        assert_eq!(
            target.steps[1].pose.model,
            InstructionPoseModel::AbsoluteHingeAnglesV1
        );
        validate_instruction_timeline(&target).expect("derived timeline remains valid");
    }

    #[test]
    fn speculative_instruction_step_id_collision_is_explicit() {
        let step_id = InstructionStepId::new();
        let source = InstructionTimeline {
            steps: vec![instruction_step(step_id)],
        };

        assert_eq!(
            append_speculative_instruction_step_v1(&source, instruction_pose(), step_id),
            Err(SpeculativeUnprovenFoldTokenIssueErrorV1::TargetInstructionTimelineStepIdCollision)
        );
    }

    #[test]
    fn invalid_derived_instruction_pose_is_rejected_before_sealing() {
        let mut invalid_pose = instruction_pose();
        invalid_pose.source_model_fingerprint = "not-a-sha256".to_owned();

        assert_eq!(
            append_speculative_instruction_step_v1(
                &InstructionTimeline::default(),
                invalid_pose,
                InstructionStepId::new(),
            ),
            Err(SpeculativeUnprovenFoldTokenIssueErrorV1::InvalidTargetInstructionTimeline)
        );
    }

    #[test]
    fn speculative_instruction_timeline_limits_are_explicit() {
        assert_eq!(
            check_target_instruction_timeline_resource_counts_v1(
                MAX_INSTRUCTION_STEPS,
                0,
                0
            ),
            Err(
                SpeculativeUnprovenFoldTokenIssueErrorV1::TargetInstructionTimelineStepLimitExceeded {
                    actual: MAX_INSTRUCTION_STEPS + 1,
                    maximum: MAX_INSTRUCTION_STEPS,
                }
            )
        );
        assert_eq!(
            check_target_instruction_timeline_resource_counts_v1(
                0,
                MAX_INSTRUCTION_HINGE_RECORDS,
                1,
            ),
            Err(
                SpeculativeUnprovenFoldTokenIssueErrorV1::TargetInstructionTimelineHingeRecordLimitExceeded {
                    actual: MAX_INSTRUCTION_HINGE_RECORDS + 1,
                    maximum: MAX_INSTRUCTION_HINGE_RECORDS,
                }
            )
        );
        assert_eq!(
            check_target_instruction_timeline_resource_counts_v1(usize::MAX, 0, 0),
            Err(SpeculativeUnprovenFoldTokenIssueErrorV1::TargetSealResourceCountOverflow)
        );
        assert_eq!(
            check_target_instruction_timeline_resource_counts_v1(0, usize::MAX, 1),
            Err(SpeculativeUnprovenFoldTokenIssueErrorV1::TargetSealResourceCountOverflow)
        );
    }

    #[test]
    fn fingerprint_hex_encoding_is_fallible_and_allocation_free_to_compare() {
        let bytes = [0xa5; 32];
        let encoded = try_lowercase_sha256(bytes).expect("reserve fixed digest string");
        assert_eq!(encoded, "a5".repeat(32));
        assert!(lowercase_sha256_matches(bytes, &encoded));
        assert!(!lowercase_sha256_matches([0x5a; 32], &encoded));
        assert!(!lowercase_sha256_matches(bytes, &encoded[..63]));
    }

    #[test]
    fn non_default_layer_assignment_is_inherited_by_every_descendant_edge() {
        let source_edge = EdgeId::new();
        let split_edge = EdgeId::new();
        let implicit_source_edge = EdgeId::new();
        let new_crease = EdgeId::new();
        let source_layers = project_layers_with_assignment(source_edge, false);
        let assignment_layer = source_layers.edge_assignments[0].layer;
        let subdivisions = source_edge_subdivisions(vec![
            (source_edge, vec![source_edge, split_edge]),
            (implicit_source_edge, vec![implicit_source_edge]),
        ]);
        let target_pattern =
            pattern_with_edges(&[source_edge, split_edge, implicit_source_edge, new_crease]);

        let target =
            transport_project_layers_to_target_v1(&source_layers, &subdivisions, &target_pattern)
                .expect("layer assignments are transported");

        assert_eq!(target.layers, source_layers.layers);
        assert_eq!(target.layer_for_edge(source_edge), assignment_layer);
        assert_eq!(target.layer_for_edge(split_edge), assignment_layer);
        assert_eq!(
            target.layer_for_edge(implicit_source_edge),
            DEFAULT_PROJECT_LAYER_ID
        );
        assert_eq!(target.layer_for_edge(new_crease), DEFAULT_PROJECT_LAYER_ID);
        assert!(
            target
                .edge_assignments
                .windows(2)
                .all(|pair| pair[0].edge.canonical_bytes() < pair[1].edge.canonical_bytes())
        );
        validate_project_layer_document_against_pattern_v1(&target, &target_pattern)
            .expect("transported layer document remains valid");
    }

    #[test]
    fn transported_layer_assignment_count_uses_checked_arithmetic() {
        assert_eq!(
            checked_target_layer_assignment_count_v1(MAX_LAYER_EDGE_ASSIGNMENTS - 1, 1),
            Ok(MAX_LAYER_EDGE_ASSIGNMENTS)
        );
        assert_eq!(
            checked_target_layer_assignment_count_v1(MAX_LAYER_EDGE_ASSIGNMENTS, 1),
            Err(
                SpeculativeUnprovenFoldTokenIssueErrorV1::TargetProjectLayerAssignmentLimitExceeded {
                    actual: MAX_LAYER_EDGE_ASSIGNMENTS + 1,
                    maximum: MAX_LAYER_EDGE_ASSIGNMENTS,
                }
            )
        );
        assert_eq!(
            checked_target_layer_assignment_count_v1(usize::MAX, 1),
            Err(SpeculativeUnprovenFoldTokenIssueErrorV1::TargetSealResourceCountOverflow)
        );
    }

    #[test]
    fn locked_assigned_edge_may_survive_but_may_not_be_subdivided() {
        let source_edge = EdgeId::new();
        let split_edge = EdgeId::new();
        let new_crease = EdgeId::new();
        let source_layers = project_layers_with_assignment(source_edge, true);
        let unchanged = source_edge_subdivisions(vec![(source_edge, vec![source_edge])]);
        let unchanged_pattern = pattern_with_edges(&[source_edge, new_crease]);
        transport_project_layers_to_target_v1(&source_layers, &unchanged, &unchanged_pattern)
            .expect("an unchanged edge on a locked layer is preserved");

        let subdivided =
            source_edge_subdivisions(vec![(source_edge, vec![source_edge, split_edge])]);
        let subdivided_pattern = pattern_with_edges(&[source_edge, split_edge, new_crease]);
        assert_eq!(
            transport_project_layers_to_target_v1(&source_layers, &subdivided, &subdivided_pattern,),
            Err(SpeculativeUnprovenFoldTokenIssueErrorV1::LockedSourceProjectLayerWouldBeModified)
        );
    }

    #[test]
    fn locked_default_layer_rejects_the_new_implicit_crease_assignment() {
        let source_edge = EdgeId::new();
        let new_crease = EdgeId::new();
        let mut source_layers = ProjectLayerDocumentV1::default();
        source_layers.layers[0].locked = true;
        let subdivisions = source_edge_subdivisions(vec![(source_edge, vec![source_edge])]);
        let target_pattern = pattern_with_edges(&[source_edge, new_crease]);

        assert_eq!(
            transport_project_layers_to_target_v1(&source_layers, &subdivisions, &target_pattern,),
            Err(SpeculativeUnprovenFoldTokenIssueErrorV1::LockedSourceProjectLayerWouldBeModified)
        );
    }

    #[test]
    fn duplicate_descendant_assignment_is_rejected_instead_of_deduplicated() {
        let first_source_edge = EdgeId::new();
        let second_source_edge = EdgeId::new();
        let shared_descendant = EdgeId::new();
        let assignment_layer = LayerId::new();
        let mut edge_assignments = vec![
            EdgeLayerAssignmentV1 {
                edge: first_source_edge,
                layer: assignment_layer,
            },
            EdgeLayerAssignmentV1 {
                edge: second_source_edge,
                layer: assignment_layer,
            },
        ];
        edge_assignments.sort_unstable_by_key(|assignment| assignment.edge.canonical_bytes());
        let source_layers = ProjectLayerDocumentV1 {
            schema_version: ProjectLayerDocumentV1::default().schema_version,
            layers: vec![
                LayerRecordV1::default_crease_pattern(),
                LayerRecordV1 {
                    id: assignment_layer,
                    name: "Fold details".to_owned(),
                    content_kind: LayerContentKindV1::CreasePattern,
                    visible: true,
                    locked: false,
                    opacity: 1.0,
                },
            ],
            edge_assignments,
        };
        let subdivisions = source_edge_subdivisions(vec![
            (
                first_source_edge,
                vec![first_source_edge, shared_descendant],
            ),
            (
                second_source_edge,
                vec![second_source_edge, shared_descendant],
            ),
        ]);
        let target_pattern =
            pattern_with_edges(&[first_source_edge, second_source_edge, shared_descendant]);

        assert_eq!(
            transport_project_layers_to_target_v1(&source_layers, &subdivisions, &target_pattern,),
            Err(SpeculativeUnprovenFoldTokenIssueErrorV1::InvalidTargetProjectLayers)
        );
    }
}
