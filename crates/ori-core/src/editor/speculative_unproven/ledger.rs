use super::{
    MAX_RETAINED_SPECULATIVE_UNPROVEN_BASE_MARKS_V1, SpeculativeUnprovenFoldMarkV1,
    SpeculativeUnprovenFoldStatusCountsV1, SpeculativeUnprovenFoldStatusV1,
};
use crate::MAX_REVISION;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpeculativeUnprovenFoldStateMarkerV1 {
    pub(crate) applied_base: AppliedBaseUnprovenLedgerV1,
    pub(crate) undo_marks: Vec<Option<SpeculativeUnprovenFoldMarkV1>>,
    pub(crate) redo_marks: Vec<Option<SpeculativeUnprovenFoldMarkV1>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AppliedBaseUnprovenMarkV1 {
    pub(crate) mark: SpeculativeUnprovenFoldMarkV1,
    pub(crate) subsequent_applied_entries: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct AppliedBaseUnprovenLedgerV1 {
    pub(crate) retained_marks: Vec<AppliedBaseUnprovenMarkV1>,
    pub(crate) collapsed_terminal: SpeculativeUnprovenFoldStatusCountsV1,
}

impl AppliedBaseUnprovenLedgerV1 {
    pub(crate) fn can_note_applied_entry_v1(&self) -> bool {
        self.retained_marks
            .iter()
            .all(|retained| retained.subsequent_applied_entries < MAX_REVISION)
    }

    pub(crate) fn note_applied_entry(&mut self) {
        for retained in &mut self.retained_marks {
            retained.subsequent_applied_entries = retained
                .subsequent_applied_entries
                .checked_add(1)
                .expect("applied history depth cannot exceed editor revisions");
        }
    }

    pub(crate) fn note_unapplied_entry(&mut self) {
        for retained in &mut self.retained_marks {
            retained.subsequent_applied_entries = retained
                .subsequent_applied_entries
                .checked_sub(1)
                .expect("a trimmed applied mark precedes every retained Undo entry");
        }
    }

    pub(crate) fn absorb_trimmed_applied_marks(
        &mut self,
        trimmed: Vec<Option<SpeculativeUnprovenFoldMarkV1>>,
        retained_applied_entries: usize,
    ) {
        let trimmed_len = trimmed.len();
        for (index, mark) in trimmed.into_iter().enumerate() {
            let Some(mark) = mark else {
                continue;
            };
            let subsequent = trimmed_len
                .saturating_sub(index + 1)
                .saturating_add(retained_applied_entries);
            self.retain(AppliedBaseUnprovenMarkV1 {
                mark,
                subsequent_applied_entries: u64::try_from(subsequent)
                    .expect("bounded history depth fits u64"),
            });
        }
    }

    pub(crate) fn try_reserve_one_trimmed_mark_v1(&mut self) -> bool {
        if self.retained_marks.len() < MAX_RETAINED_SPECULATIVE_UNPROVEN_BASE_MARKS_V1 {
            return self.retained_marks.try_reserve(1).is_ok();
        }
        true
    }

    pub(crate) fn absorb_one_trimmed_applied_mark_v1(
        &mut self,
        mark: Option<SpeculativeUnprovenFoldMarkV1>,
        retained_applied_entries: usize,
    ) {
        let Some(mark) = mark else {
            return;
        };
        self.retain(AppliedBaseUnprovenMarkV1 {
            mark,
            subsequent_applied_entries: u64::try_from(retained_applied_entries)
                .expect("bounded history depth fits u64"),
        });
    }

    fn retain(&mut self, retained: AppliedBaseUnprovenMarkV1) {
        if self.retained_marks.len() == MAX_RETAINED_SPECULATIVE_UNPROVEN_BASE_MARKS_V1 {
            let terminal_index = self
                .retained_marks
                .iter()
                .position(|item| item.mark.status != SpeculativeUnprovenFoldStatusV1::AwaitingProof)
                .expect(
                    "more retained base marks than the pending limit guarantees a terminal mark",
                );
            let collapsed = self.retained_marks.remove(terminal_index);
            self.collapsed_terminal.add_status(collapsed.mark.status);
        }
        self.retained_marks.push(retained);
    }

    pub(crate) fn pending_count(&self) -> usize {
        self.retained_marks
            .iter()
            .filter(|item| item.mark.status == SpeculativeUnprovenFoldStatusV1::AwaitingProof)
            .count()
    }

    pub(crate) fn add_to_counts(&self, counts: &mut SpeculativeUnprovenFoldStatusCountsV1) {
        counts.awaiting_proof += self.collapsed_terminal.awaiting_proof;
        counts.proof_blocked += self.collapsed_terminal.proof_blocked;
        counts.unknown_evidence_insufficient +=
            self.collapsed_terminal.unknown_evidence_insufficient;
        counts.unknown_resource_limit += self.collapsed_terminal.unknown_resource_limit;
        counts.unknown_cancelled += self.collapsed_terminal.unknown_cancelled;
        counts.unknown_deadline_reached += self.collapsed_terminal.unknown_deadline_reached;
        for item in &self.retained_marks {
            counts.add_status(item.mark.status);
        }
    }
}
