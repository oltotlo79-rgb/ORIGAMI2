use super::atomic_revert::{
    RevertProofLocationV1, RevertProofOutcomeV1, RevertProofReasonV1,
    revert_post_apply_proof_failure_inner_v1,
    revert_post_apply_proof_failure_with_interleave_for_test_v1, revert_unavailable_message_v1,
    validate_revert_request_v1,
};
use super::*;
use crate::global_flat_foldability::GlobalFlatFoldabilityState;
use ori_core::SpeculativeUnprovenFoldHistoryLocationV1;

#[path = "post_apply_proof_tests/admission_only.rs"]
mod admission_only;
#[path = "post_apply_proof_tests/deadline_scheduler.rs"]
mod deadline_scheduler;
#[path = "post_apply_proof_tests/layered_four_face.rs"]
mod layered_four_face;
#[path = "post_apply_proof_tests/layered_three_face.rs"]
mod layered_three_face;
#[path = "post_apply_proof_tests/publication_recovery.rs"]
mod publication_recovery;

include!("post_apply_proof_tests/fixture_helpers.rs");
include!("post_apply_proof_tests/premise_binding_and_fault_guards.rs");
include!("post_apply_proof_tests/publication_and_start_recovery.rs");
include!("post_apply_proof_tests/deadline_scheduler_recovery.rs");
include!("post_apply_proof_tests/progress_and_worker_lifecycle.rs");
include!("post_apply_proof_tests/cancellation_and_atomic_revert.rs");
