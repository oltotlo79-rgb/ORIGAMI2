use ori_domain::ProjectId;

use crate::stacked_fold::{
    PreparedStackedFoldRequestedPoseV1, SpeculativeUnprovenFoldTokenIssueErrorV1,
    SpeculativeUnprovenFoldTokenV1, StackedFoldInitialLayerOrderV1,
    issue_speculative_unproven_fold_token_v1,
    prepared_stacked_fold_request_matches_applied_source_pose_v1,
};

use super::{
    super::{CommandResult, EditorState},
    MAX_PENDING_SPECULATIVE_UNPROVEN_FOLDS_V1, SpeculativeApproximateBlockingObservationV1,
    SpeculativeUnprovenFoldApplyErrorV1, SpeculativeUnprovenFoldBindingV1,
    SpeculativeUnprovenFoldHistoryLocationV1, SpeculativeUnprovenFoldMarkV1,
    SpeculativeUnprovenFoldProofOutcomeV1, SpeculativeUnprovenFoldResolutionErrorV1,
    SpeculativeUnprovenFoldResolutionReportV1, SpeculativeUnprovenFoldStateMarkerV1,
    SpeculativeUnprovenFoldStatusV1, SpeculativeUnprovenFoldSummaryV1,
};

#[derive(Debug, Clone, Copy)]
enum MarkLocation {
    AppliedBase(usize),
    Undo(usize),
    Redo(usize),
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
        if lineage.source_fingerprint().to_hex() != self.fold_model_fingerprint_v1() {
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
        if !self.current_applied_pose().is_some_and(|current| {
            prepared_stacked_fold_request_matches_applied_source_pose_v1(
                requested,
                self.pattern(),
                self.paper(),
                current,
            )
        }) {
            return Err(SpeculativeUnprovenFoldTokenIssueErrorV1::SourceAppliedPoseMismatch);
        }
        issue_speculative_unproven_fold_token_v1(
            self.runtime_instance_anchor.clone(),
            self.current_applied_pose(),
            self.instruction_timeline(),
            self.project_layers(),
            self.beginner_design_profile(),
            project_instance_id,
            requested,
            initial_layer_order,
            pose_generation,
            request_generation_id,
            paper_thickness_mm,
        )
    }

    /// Applies a stacked-fold document and unproven metadata as one entry.
    ///
    /// Core consumes the opaque one-shot token and obtains the complete target
    /// command from it. No caller-supplied pattern, paper, timeline, layers, or
    /// pose can be substituted at Apply time.
    pub fn execute_stacked_fold_document_with_unproven_mark_v1(
        &mut self,
        token: SpeculativeUnprovenFoldTokenV1,
    ) -> Result<CommandResult, SpeculativeUnprovenFoldApplyErrorV1> {
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
        if binding.source_geometry_fingerprint_sha256() != self.fold_model_fingerprint_v1() {
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
        if !self.find_mark_locations(&binding).is_empty() {
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

        let result = self.execute_stacked_fold_document_command_v1(
            expected_revision,
            command,
            applied_pose,
        )?;
        self.undo_stack
            .last_mut()
            .expect("successful stacked-fold Apply creates one history entry")
            .speculative_unproven_fold = Some(SpeculativeUnprovenFoldMarkV1::awaiting(binding));
        Ok(result)
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
        let location = match self.find_mark_locations(binding).as_slice() {
            [] => return Err(SpeculativeUnprovenFoldResolutionErrorV1::BindingNotFound),
            [location] => *location,
            _ => return Err(SpeculativeUnprovenFoldResolutionErrorV1::DuplicateBinding),
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
    /// `Certified` is deliberately rejected here. Only a future resolver that
    /// receives and revalidates an opaque typed proof may remove an unproven
    /// mark.
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
        let location = match self.find_mark_locations(binding).as_slice() {
            [] => return Err(SpeculativeUnprovenFoldResolutionErrorV1::BindingNotFound),
            [location] => *location,
            _ => return Err(SpeculativeUnprovenFoldResolutionErrorV1::DuplicateBinding),
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

    fn find_mark_locations(&self, binding: &SpeculativeUnprovenFoldBindingV1) -> Vec<MarkLocation> {
        let mut locations = Vec::new();
        locations.extend(
            self.applied_base_unproven
                .retained_marks
                .iter()
                .enumerate()
                .filter(|(_, item)| item.mark.binding.has_same_request_identity(binding))
                .map(|(index, _)| MarkLocation::AppliedBase(index)),
        );
        for (index, entry) in self.undo_stack.iter().enumerate() {
            if entry
                .speculative_unproven_fold
                .as_ref()
                .is_some_and(|mark| mark.binding.has_same_request_identity(binding))
            {
                locations.push(MarkLocation::Undo(index));
            }
        }
        for (index, entry) in self.redo_stack.iter().enumerate() {
            if entry
                .speculative_unproven_fold
                .as_ref()
                .is_some_and(|mark| mark.binding.has_same_request_identity(binding))
            {
                locations.push(MarkLocation::Redo(index));
            }
        }
        locations
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
