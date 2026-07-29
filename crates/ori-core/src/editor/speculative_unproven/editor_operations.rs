use std::sync::Arc;

use ori_domain::ProjectId;

use crate::stacked_fold::{
    PreparedStackedFoldRequestedPoseV1, PreparedStackedFoldSourcePoseMatchErrorV1,
    SpeculativeUnprovenFoldTokenIssueErrorV1, SpeculativeUnprovenFoldTokenIssueInputV1,
    SpeculativeUnprovenFoldTokenV1, StackedFoldInitialLayerOrderV1,
    issue_speculative_unproven_fold_token_v1,
    prepared_stacked_fold_request_matches_applied_source_pose_v1,
};

use super::{
    super::{CommandResult, EditorState, speculative_stacked_fold_allocation_checkpoint_v1},
    MAX_PENDING_SPECULATIVE_UNPROVEN_FOLDS_V1, SpeculativeApproximateBlockingObservationV1,
    SpeculativeUnprovenFoldApplyErrorV1, SpeculativeUnprovenFoldApplyResourceV1,
    SpeculativeUnprovenFoldBindingV1, SpeculativeUnprovenFoldCertifiedProofV1,
    SpeculativeUnprovenFoldHistoryLocationV1, SpeculativeUnprovenFoldMarkV1,
    SpeculativeUnprovenFoldProofOutcomeV1, SpeculativeUnprovenFoldResolutionErrorV1,
    SpeculativeUnprovenFoldResolutionReportV1, SpeculativeUnprovenFoldResolutionTicketV1,
    SpeculativeUnprovenFoldStateMarkerV1, SpeculativeUnprovenFoldStatusV1,
    SpeculativeUnprovenFoldSummaryV1,
};

#[derive(Debug, Clone, Copy)]
enum MarkLocation {
    AppliedBase(usize),
    Undo(usize),
    Redo(usize),
}

#[derive(Debug, Clone, Copy)]
enum MarkLocationMatch {
    Missing,
    Unique(MarkLocation),
    Duplicate,
}

impl EditorState {
    /// Issues a native one-shot permission for one exact speculative command.
    ///
    /// Core derives and owns the complete target pattern, paper, instruction
    /// timeline, project layers, beginner profile, face registry, and semantic
    /// pose. The token is also tied to this live editor's non-persisted
    /// instance anchor and current runtime pose.
    pub fn issue_speculative_unproven_fold_token_v1(
        &self,
        project_instance_id: ProjectId,
        requested: &PreparedStackedFoldRequestedPoseV1,
        initial_layer_order: &StackedFoldInitialLayerOrderV1,
        pose_generation: u64,
        request_generation_id: ProjectId,
        paper_thickness_mm: f64,
    ) -> Result<SpeculativeUnprovenFoldTokenV1, SpeculativeUnprovenFoldTokenIssueErrorV1> {
        let lineage = requested.initial().target().geometry().proof().lineage();
        if lineage.source_revision() != self.revision() {
            return Err(SpeculativeUnprovenFoldTokenIssueErrorV1::SourceRevisionMismatch);
        }
        if lineage.source_fingerprint()
            != ori_foldability::fold_model_fingerprint_v1(self.pattern(), self.paper())
        {
            return Err(
                SpeculativeUnprovenFoldTokenIssueErrorV1::SourceGeometryFingerprintMismatch,
            );
        }
        if paper_thickness_mm.to_bits() != self.paper().thickness_mm.to_bits() {
            return Err(SpeculativeUnprovenFoldTokenIssueErrorV1::SourcePaperThicknessMismatch);
        }
        let requested_candidate = requested.initial().target().geometry().candidate();
        let requested_paper = &requested_candidate.paper;
        if requested_paper.cutting_allowed != self.paper().cutting_allowed
            || requested_paper.length_display_unit != self.paper().length_display_unit
            || requested_paper.front != self.paper().front
            || requested_paper.back != self.paper().back
        {
            return Err(SpeculativeUnprovenFoldTokenIssueErrorV1::SourcePaperPresentationMismatch);
        }
        if !self.length_display_reference_survives_document_replacement(
            &requested_candidate.pattern,
            requested_paper,
        ) {
            return Err(
                SpeculativeUnprovenFoldTokenIssueErrorV1::TargetLengthDisplayReferenceInvalid,
            );
        }
        let current_applied_pose = self
            .current_applied_pose()
            .ok_or(SpeculativeUnprovenFoldTokenIssueErrorV1::SourceAppliedPoseMismatch)?;
        prepared_stacked_fold_request_matches_applied_source_pose_v1(
            requested,
            self.pattern(),
            self.paper(),
            current_applied_pose,
        )
        .map_err(map_source_pose_match_error_v1)?;
        issue_speculative_unproven_fold_token_v1(SpeculativeUnprovenFoldTokenIssueInputV1 {
            editor_instance_anchor: self.runtime_instance_anchor.clone(),
            source_applied_pose: self.current_applied_pose(),
            source_instruction_timeline: self.instruction_timeline(),
            source_project_layers: self.project_layers(),
            source_beginner_design_profile: self.beginner_design_profile(),
            project_instance_id,
            requested,
            initial_layer_order,
            pose_generation,
            request_generation_id,
            paper_thickness_mm,
        })
    }

    /// Applies a stacked-fold document and unproven metadata as one entry.
    ///
    /// Core consumes the opaque one-shot token and obtains the complete target
    /// command from it. No caller-supplied pattern, paper, timeline, layers, or
    /// pose can be substituted at Apply time.
    ///
    /// This compatibility entry point permanently discards the only runtime
    /// resolution ticket produced by a successful Apply. Its awaiting-proof
    /// mark therefore cannot later be removed as `Certified`; callers that
    /// intend to run post-Apply certification must use
    /// [`Self::execute_stacked_fold_document_with_unproven_mark_and_resolution_ticket_v1`].
    pub fn execute_stacked_fold_document_with_unproven_mark_v1(
        &mut self,
        token: SpeculativeUnprovenFoldTokenV1,
    ) -> Result<CommandResult, SpeculativeUnprovenFoldApplyErrorV1> {
        self.execute_stacked_fold_document_with_unproven_mark_and_resolution_ticket_v1(token)
            .map(|(result, _ticket)| result)
    }

    /// Applies one speculative stacked-fold document and returns the only
    /// runtime ticket that can later bind a typed continuous proof.
    ///
    /// Ticket state is fully retained before the document mutation begins.
    /// The successful command, awaiting-proof mark, and returned ticket
    /// therefore describe one exact atomic Apply.
    pub fn execute_stacked_fold_document_with_unproven_mark_and_resolution_ticket_v1(
        &mut self,
        token: SpeculativeUnprovenFoldTokenV1,
    ) -> Result<
        (CommandResult, SpeculativeUnprovenFoldResolutionTicketV1),
        SpeculativeUnprovenFoldApplyErrorV1,
    > {
        let (binding, command, applied_pose) = token
            .into_authorized_target_v1(
                &self.runtime_instance_anchor,
                self.revision(),
                self.current_applied_pose(),
            )
            .ok_or(SpeculativeUnprovenFoldApplyErrorV1::TargetSealMismatch)?;
        binding.validate()?;
        let expected_revision = binding.source_revision();
        if self.revision() != expected_revision {
            return Err(SpeculativeUnprovenFoldApplyErrorV1::SourceRevisionMismatch);
        }
        if !lowercase_sha256_matches_v1(
            ori_foldability::fold_model_fingerprint_v1(self.pattern(), self.paper()).0,
            binding.source_geometry_fingerprint_sha256(),
        ) {
            return Err(SpeculativeUnprovenFoldApplyErrorV1::SourceGeometryFingerprintMismatch);
        }
        if binding.paper_thickness_bits() != self.paper().thickness_mm.to_bits() {
            return Err(SpeculativeUnprovenFoldApplyErrorV1::PaperThicknessBitsMismatch);
        }
        if matches!(
            binding.approximate_blocking_observation(),
            SpeculativeApproximateBlockingObservationV1::BlockingSampleObserved { .. }
        ) {
            return Err(SpeculativeUnprovenFoldApplyErrorV1::ApproximateBlockingSampleObserved);
        }
        if !matches!(
            self.find_mark_location(&binding),
            MarkLocationMatch::Missing
        ) {
            return Err(SpeculativeUnprovenFoldApplyErrorV1::DuplicateBinding);
        }
        let pending_after_redo_discard = self.applied_base_unproven.pending_count()
            + self
                .undo_stack
                .iter()
                .filter(|entry| {
                    entry
                        .speculative_unproven_fold
                        .as_ref()
                        .is_some_and(|mark| {
                            mark.status == SpeculativeUnprovenFoldStatusV1::AwaitingProof
                        })
                })
                .count();
        if pending_after_redo_discard >= MAX_PENDING_SPECULATIVE_UNPROVEN_FOLDS_V1 {
            return Err(SpeculativeUnprovenFoldApplyErrorV1::PendingMarkLimitReached);
        }
        self.ensure_applied_history_depth_can_advance_v1()
            .map_err(|_| {
                SpeculativeUnprovenFoldApplyErrorV1::AppliedBaseHistoryDepthLimitReached
            })?;

        let target_revision = expected_revision
            .checked_add(1)
            .filter(|revision| *revision <= crate::MAX_REVISION)
            .ok_or(SpeculativeUnprovenFoldApplyErrorV1::InvalidTargetRevision)?;
        let target_geometry_fingerprint =
            ori_foldability::fold_model_fingerprint_v1(&command.pattern, &command.paper).0;
        speculative_stacked_fold_allocation_checkpoint_v1(
            SpeculativeUnprovenFoldApplyResourceV1::HistoryMarkBinding,
        )?;
        let mark_binding = binding.try_clone_for_runtime_commit_v1().ok_or(
            SpeculativeUnprovenFoldApplyErrorV1::CommitPreparationResourceLimit {
                resource: SpeculativeUnprovenFoldApplyResourceV1::HistoryMarkBinding,
            },
        )?;
        let editor_instance_anchor = Arc::clone(&self.runtime_instance_anchor);
        let prepared = self.prepare_speculative_stacked_fold_commit_v1(
            expected_revision,
            command,
            applied_pose,
        )?;
        let (result, target_applied_pose) = self.commit_prepared_speculative_stacked_fold_v1(
            prepared,
            SpeculativeUnprovenFoldMarkV1::awaiting(mark_binding),
        );
        let resolution_ticket = SpeculativeUnprovenFoldResolutionTicketV1::new(
            editor_instance_anchor,
            binding,
            target_revision,
            target_geometry_fingerprint,
            target_applied_pose,
        );
        debug_assert_eq!(result.revision, target_revision);
        Ok((result, resolution_ticket))
    }

    /// Inspects one speculative history binding without changing its status,
    /// document revision, or Undo/Redo location.
    ///
    /// `Ok(None)` means the unique matching mark is still awaiting proof.
    /// Terminal blocked/unknown marks return the same location-aware report
    /// shape as resolution. A missing, duplicated, or metadata-drifted
    /// binding fails closed.
    pub fn inspect_speculative_unproven_fold_v1(
        &self,
        binding: &SpeculativeUnprovenFoldBindingV1,
    ) -> Result<
        Option<SpeculativeUnprovenFoldResolutionReportV1>,
        SpeculativeUnprovenFoldResolutionErrorV1,
    > {
        binding.validate()?;
        let location = match self.find_mark_location(binding) {
            MarkLocationMatch::Missing => {
                return Err(SpeculativeUnprovenFoldResolutionErrorV1::BindingNotFound);
            }
            MarkLocationMatch::Unique(location) => location,
            MarkLocationMatch::Duplicate => {
                return Err(SpeculativeUnprovenFoldResolutionErrorV1::DuplicateBinding);
            }
        };
        let located = self.mark_at(location).expect("located mark");
        if located.binding != *binding {
            return Err(SpeculativeUnprovenFoldResolutionErrorV1::BindingMetadataMismatch);
        }
        let outcome = match located.status {
            SpeculativeUnprovenFoldStatusV1::AwaitingProof => return Ok(None),
            SpeculativeUnprovenFoldStatusV1::ProofBlocked => {
                SpeculativeUnprovenFoldProofOutcomeV1::Blocked
            }
            SpeculativeUnprovenFoldStatusV1::ProofUnknown { reason } => {
                SpeculativeUnprovenFoldProofOutcomeV1::Unknown { reason }
            }
        };
        Ok(Some(self.resolution_report(location, outcome)))
    }

    /// Records a blocked or unknown result without Undo or document revision
    /// changes.
    ///
    /// `Certified` is deliberately rejected here. Only
    /// [`Self::resolve_speculative_unproven_fold_certified_v1`], which consumes
    /// an opaque typed proof, may remove an unproven mark.
    pub fn resolve_speculative_unproven_fold_v1(
        &mut self,
        binding: &SpeculativeUnprovenFoldBindingV1,
        outcome: SpeculativeUnprovenFoldProofOutcomeV1,
    ) -> Result<SpeculativeUnprovenFoldResolutionReportV1, SpeculativeUnprovenFoldResolutionErrorV1>
    {
        let terminal_status = match outcome {
            SpeculativeUnprovenFoldProofOutcomeV1::Certified => {
                return Err(SpeculativeUnprovenFoldResolutionErrorV1::CertifiedRequiresTypedProof);
            }
            SpeculativeUnprovenFoldProofOutcomeV1::Blocked => {
                SpeculativeUnprovenFoldStatusV1::ProofBlocked
            }
            SpeculativeUnprovenFoldProofOutcomeV1::Unknown { reason } => {
                SpeculativeUnprovenFoldStatusV1::ProofUnknown { reason }
            }
        };
        binding.validate()?;
        let location = match self.find_mark_location(binding) {
            MarkLocationMatch::Missing => {
                return Err(SpeculativeUnprovenFoldResolutionErrorV1::BindingNotFound);
            }
            MarkLocationMatch::Unique(location) => location,
            MarkLocationMatch::Duplicate => {
                return Err(SpeculativeUnprovenFoldResolutionErrorV1::DuplicateBinding);
            }
        };
        let located = self.mark_at(location).expect("located mark");
        if located.binding != *binding {
            return Err(SpeculativeUnprovenFoldResolutionErrorV1::BindingMetadataMismatch);
        }
        if located.status != SpeculativeUnprovenFoldStatusV1::AwaitingProof {
            return Err(SpeculativeUnprovenFoldResolutionErrorV1::AlreadyResolved);
        }

        let report = self.resolution_report(location, outcome);
        self.mark_at_mut(location).expect("located mark").status = terminal_status;
        Ok(report)
    }

    /// Consumes a typed continuous proof and removes only its exact
    /// awaiting-proof mark.
    ///
    /// The document, revision, applied pose, and Undo/Redo entry order are
    /// unchanged. A foreign editor, metadata drift, duplicate location, or
    /// terminal mark fails before any mutation.
    pub fn resolve_speculative_unproven_fold_certified_v1(
        &mut self,
        proof: SpeculativeUnprovenFoldCertifiedProofV1,
    ) -> Result<SpeculativeUnprovenFoldResolutionReportV1, SpeculativeUnprovenFoldResolutionErrorV1>
    {
        let (
            editor_instance_anchor,
            binding,
            target_revision,
            _target_geometry_fingerprint,
            _target_applied_pose,
        ) = proof.into_resolution_parts();
        if !Arc::ptr_eq(&editor_instance_anchor, &self.runtime_instance_anchor) {
            return Err(SpeculativeUnprovenFoldResolutionErrorV1::ForeignEditor);
        }
        binding.validate()?;
        if binding
            .source_revision()
            .checked_add(1)
            .filter(|revision| *revision <= crate::MAX_REVISION)
            != Some(target_revision)
        {
            return Err(SpeculativeUnprovenFoldResolutionErrorV1::InvalidCertifiedProof);
        }
        let location = match self.find_mark_location(&binding) {
            MarkLocationMatch::Missing => {
                return Err(SpeculativeUnprovenFoldResolutionErrorV1::BindingNotFound);
            }
            MarkLocationMatch::Unique(location) => location,
            MarkLocationMatch::Duplicate => {
                return Err(SpeculativeUnprovenFoldResolutionErrorV1::DuplicateBinding);
            }
        };
        let located = self.mark_at(location).expect("located mark");
        if located.binding != binding {
            return Err(SpeculativeUnprovenFoldResolutionErrorV1::BindingMetadataMismatch);
        }
        if located.status != SpeculativeUnprovenFoldStatusV1::AwaitingProof {
            return Err(SpeculativeUnprovenFoldResolutionErrorV1::AlreadyResolved);
        }

        let report =
            self.resolution_report(location, SpeculativeUnprovenFoldProofOutcomeV1::Certified);
        let removed = match location {
            MarkLocation::AppliedBase(index) => {
                self.applied_base_unproven.retained_marks.remove(index).mark
            }
            MarkLocation::Undo(index) => self.undo_stack[index]
                .speculative_unproven_fold
                .take()
                .expect("the validated Undo mark remains present"),
            MarkLocation::Redo(index) => self.redo_stack[index]
                .speculative_unproven_fold
                .take()
                .expect("the validated Redo mark remains present"),
        };
        debug_assert_eq!(removed.binding, binding);
        Ok(report)
    }

    #[must_use]
    pub fn speculative_unproven_fold_summary_v1(&self) -> SpeculativeUnprovenFoldSummaryV1 {
        let mut summary = SpeculativeUnprovenFoldSummaryV1::default();
        self.applied_base_unproven
            .add_to_counts(&mut summary.applied);
        for entry in &self.undo_stack {
            if let Some(mark) = &entry.speculative_unproven_fold {
                summary.applied.add_status(mark.status);
            }
        }
        for entry in &self.redo_stack {
            if let Some(mark) = &entry.speculative_unproven_fold {
                summary.unapplied_redo.add_status(mark.status);
            }
        }
        summary
    }

    #[must_use]
    pub fn requires_speculative_unproven_fold_feature_v1(&self) -> bool {
        let summary = self.speculative_unproven_fold_summary_v1();
        summary.applied.total() > 0 || summary.unapplied_redo.total() > 0
    }

    #[must_use]
    pub fn speculative_unproven_fold_state_marker_v1(
        &self,
    ) -> SpeculativeUnprovenFoldStateMarkerV1 {
        SpeculativeUnprovenFoldStateMarkerV1 {
            applied_base: self.applied_base_unproven.clone(),
            undo_marks: self
                .undo_stack
                .iter()
                .map(|entry| entry.speculative_unproven_fold.clone())
                .collect(),
            redo_marks: self
                .redo_stack
                .iter()
                .map(|entry| entry.speculative_unproven_fold.clone())
                .collect(),
        }
    }

    fn find_mark_location(&self, binding: &SpeculativeUnprovenFoldBindingV1) -> MarkLocationMatch {
        let mut found = None;
        let mut observe = |location| {
            if found.replace(location).is_some() {
                MarkLocationMatch::Duplicate
            } else {
                MarkLocationMatch::Unique(location)
            }
        };
        for (index, item) in self.applied_base_unproven.retained_marks.iter().enumerate() {
            if item.mark.binding.has_same_request_identity(binding)
                && matches!(
                    observe(MarkLocation::AppliedBase(index)),
                    MarkLocationMatch::Duplicate
                )
            {
                return MarkLocationMatch::Duplicate;
            }
        }
        for (index, entry) in self.undo_stack.iter().enumerate() {
            if entry
                .speculative_unproven_fold
                .as_ref()
                .is_some_and(|mark| mark.binding.has_same_request_identity(binding))
                && matches!(
                    observe(MarkLocation::Undo(index)),
                    MarkLocationMatch::Duplicate
                )
            {
                return MarkLocationMatch::Duplicate;
            }
        }
        for (index, entry) in self.redo_stack.iter().enumerate() {
            if entry
                .speculative_unproven_fold
                .as_ref()
                .is_some_and(|mark| mark.binding.has_same_request_identity(binding))
                && matches!(
                    observe(MarkLocation::Redo(index)),
                    MarkLocationMatch::Duplicate
                )
            {
                return MarkLocationMatch::Duplicate;
            }
        }
        found.map_or(MarkLocationMatch::Missing, MarkLocationMatch::Unique)
    }

    fn mark_at(&self, location: MarkLocation) -> Option<&SpeculativeUnprovenFoldMarkV1> {
        match location {
            MarkLocation::AppliedBase(index) => self
                .applied_base_unproven
                .retained_marks
                .get(index)
                .map(|item| &item.mark),
            MarkLocation::Undo(index) => self
                .undo_stack
                .get(index)?
                .speculative_unproven_fold
                .as_ref(),
            MarkLocation::Redo(index) => self
                .redo_stack
                .get(index)?
                .speculative_unproven_fold
                .as_ref(),
        }
    }

    fn mark_at_mut(
        &mut self,
        location: MarkLocation,
    ) -> Option<&mut SpeculativeUnprovenFoldMarkV1> {
        match location {
            MarkLocation::AppliedBase(index) => self
                .applied_base_unproven
                .retained_marks
                .get_mut(index)
                .map(|item| &mut item.mark),
            MarkLocation::Undo(index) => self
                .undo_stack
                .get_mut(index)?
                .speculative_unproven_fold
                .as_mut(),
            MarkLocation::Redo(index) => self
                .redo_stack
                .get_mut(index)?
                .speculative_unproven_fold
                .as_mut(),
        }
    }

    fn resolution_report(
        &self,
        location: MarkLocation,
        outcome: SpeculativeUnprovenFoldProofOutcomeV1,
    ) -> SpeculativeUnprovenFoldResolutionReportV1 {
        match location {
            MarkLocation::AppliedBase(index) => SpeculativeUnprovenFoldResolutionReportV1 {
                location: SpeculativeUnprovenFoldHistoryLocationV1::AppliedTrimmedBase,
                outcome,
                subsequent_edit_count: self.applied_base_unproven.retained_marks[index]
                    .subsequent_applied_entries,
                undo_steps_to_revert: None,
            },
            MarkLocation::Undo(index) => {
                let subsequent = self.undo_stack.len().saturating_sub(index + 1);
                SpeculativeUnprovenFoldResolutionReportV1 {
                    location: SpeculativeUnprovenFoldHistoryLocationV1::AppliedRetainedUndo,
                    outcome,
                    subsequent_edit_count: subsequent as u64,
                    undo_steps_to_revert: Some((subsequent + 1) as u32),
                }
            }
            MarkLocation::Redo(_) => SpeculativeUnprovenFoldResolutionReportV1 {
                location: SpeculativeUnprovenFoldHistoryLocationV1::UnappliedRedo,
                outcome,
                subsequent_edit_count: 0,
                undo_steps_to_revert: None,
            },
        }
    }
}

fn map_source_pose_match_error_v1(
    error: PreparedStackedFoldSourcePoseMatchErrorV1,
) -> SpeculativeUnprovenFoldTokenIssueErrorV1 {
    match error {
        PreparedStackedFoldSourcePoseMatchErrorV1::Mismatch => {
            SpeculativeUnprovenFoldTokenIssueErrorV1::SourceAppliedPoseMismatch
        }
        PreparedStackedFoldSourcePoseMatchErrorV1::ReconstructionUnavailable => {
            SpeculativeUnprovenFoldTokenIssueErrorV1::SourceAppliedPoseReconstructionUnavailable
        }
        PreparedStackedFoldSourcePoseMatchErrorV1::ResourceLimitExceeded {
            resource,
            actual,
            maximum,
        } => SpeculativeUnprovenFoldTokenIssueErrorV1::SourceAppliedPoseResourceLimitExceeded {
            resource,
            actual,
            maximum,
        },
        PreparedStackedFoldSourcePoseMatchErrorV1::ResourceCountOverflow { resource } => {
            SpeculativeUnprovenFoldTokenIssueErrorV1::SourceAppliedPoseResourceCountOverflow {
                resource,
            }
        }
        PreparedStackedFoldSourcePoseMatchErrorV1::AllocationFailed { resource } => {
            SpeculativeUnprovenFoldTokenIssueErrorV1::SourceAppliedPoseAllocationFailed { resource }
        }
    }
}

fn lowercase_sha256_matches_v1(bytes: [u8; 32], encoded: &str) -> bool {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    encoded.len() == 64
        && bytes.iter().enumerate().all(|(index, byte)| {
            encoded.as_bytes()[index * 2] == DIGITS[usize::from(byte >> 4)]
                && encoded.as_bytes()[index * 2 + 1] == DIGITS[usize::from(byte & 0x0f)]
        })
}

#[cfg(test)]
mod source_pose_error_mapping_tests {
    use crate::stacked_fold::PreparedStackedFoldSourcePoseResourceV1;

    use super::*;

    #[test]
    fn every_typed_source_pose_failure_keeps_its_category_and_resource() {
        let resource = PreparedStackedFoldSourcePoseResourceV1::TargetEdgeMappingRecords;
        assert_eq!(
            map_source_pose_match_error_v1(PreparedStackedFoldSourcePoseMatchErrorV1::Mismatch),
            SpeculativeUnprovenFoldTokenIssueErrorV1::SourceAppliedPoseMismatch
        );
        assert_eq!(
            map_source_pose_match_error_v1(
                PreparedStackedFoldSourcePoseMatchErrorV1::ReconstructionUnavailable
            ),
            SpeculativeUnprovenFoldTokenIssueErrorV1::SourceAppliedPoseReconstructionUnavailable
        );
        assert_eq!(
            map_source_pose_match_error_v1(
                PreparedStackedFoldSourcePoseMatchErrorV1::ResourceLimitExceeded {
                    resource,
                    actual: 11,
                    maximum: 10,
                }
            ),
            SpeculativeUnprovenFoldTokenIssueErrorV1::SourceAppliedPoseResourceLimitExceeded {
                resource,
                actual: 11,
                maximum: 10,
            }
        );
        assert_eq!(
            map_source_pose_match_error_v1(
                PreparedStackedFoldSourcePoseMatchErrorV1::ResourceCountOverflow { resource }
            ),
            SpeculativeUnprovenFoldTokenIssueErrorV1::SourceAppliedPoseResourceCountOverflow {
                resource,
            }
        );
        assert_eq!(
            map_source_pose_match_error_v1(
                PreparedStackedFoldSourcePoseMatchErrorV1::AllocationFailed { resource }
            ),
            SpeculativeUnprovenFoldTokenIssueErrorV1::SourceAppliedPoseAllocationFailed {
                resource,
            }
        );
    }
}
