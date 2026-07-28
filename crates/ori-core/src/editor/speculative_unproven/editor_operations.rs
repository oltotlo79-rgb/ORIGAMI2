use ori_domain::{CreasePattern, InstructionTimeline, Paper, ProjectLayerDocumentV1};

use crate::stacked_fold::SpeculativeUnprovenFoldTokenV1;

use super::{
    super::{AppliedPoseV1, CommandResult, EditorState, Revision},
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
    /// Applies a stacked-fold document and unproven metadata as one entry.
    ///
    /// The desktop layer must reauthenticate project-instance, project,
    /// pose-generation, and request-generation fields immediately before this
    /// call. Core consumes the opaque one-shot token, rechecks its exact target
    /// seal against these owned arguments, and independently rechecks every
    /// editor-owned binding before any mutation.
    #[allow(clippy::too_many_arguments)]
    pub fn execute_stacked_fold_document_with_unproven_mark_v1(
        &mut self,
        expected_revision: Revision,
        pattern: CreasePattern,
        paper: Paper,
        instruction_timeline: InstructionTimeline,
        project_layers: ProjectLayerDocumentV1,
        applied_pose: AppliedPoseV1,
        token: SpeculativeUnprovenFoldTokenV1,
    ) -> Result<CommandResult, SpeculativeUnprovenFoldApplyErrorV1> {
        let binding = token
            .into_unproven_binding_for_target_v1(expected_revision, &pattern, &paper, &applied_pose)
            .ok_or(SpeculativeUnprovenFoldApplyErrorV1::TargetSealMismatch)?;
        binding.validate()?;
        if binding.source_revision() != expected_revision || self.revision() != expected_revision {
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

        let result = self.execute_stacked_fold_document(
            expected_revision,
            pattern,
            paper,
            instruction_timeline,
            project_layers,
            applied_pose,
        )?;
        self.undo_stack
            .last_mut()
            .expect("successful stacked-fold Apply creates one history entry")
            .speculative_unproven_fold = Some(SpeculativeUnprovenFoldMarkV1::awaiting(binding));
        Ok(result)
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
