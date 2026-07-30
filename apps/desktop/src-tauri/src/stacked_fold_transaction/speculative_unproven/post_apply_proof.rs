#[cfg(test)]
use std::{cell::Cell, marker::PhantomData, rc::Rc, thread};
use std::{
    collections::VecDeque,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc, Mutex, MutexGuard, OnceLock, Weak,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc::{Receiver, RecvTimeoutError, SyncSender, TrySendError, sync_channel},
    },
    time::{Duration, Instant},
};

use ori_collision::{
    CooperativeOperationControlV1, CooperativeOperationStopV1, StackedFoldBoundedPathDiagnosticV1,
    StackedFoldPathDiagnosticErrorV1, StackedFoldPathDiagnosticLimitsV1,
    StackedFoldTreeContinuousCertificateV1, certify_tree_continuous_path_from_pose_with_control_v1,
    diagnose_collective_hinge_path_with_initial_sample_layer_admission_with_control_v1,
    prepare_stacked_fold_initial_sample_layer_admission_with_control_v1,
};
use ori_core::{
    PreparedStackedFoldRequestedPoseV1, SpeculativeUnprovenFoldBindingV1,
    SpeculativeUnprovenFoldCertificationErrorV1, SpeculativeUnprovenFoldCertifiedProofV1,
    SpeculativeUnprovenFoldLayeredFourFaceCertifiedProofV1,
    SpeculativeUnprovenFoldLayeredThreeFaceCertifiedProofV1, SpeculativeUnprovenFoldProofOutcomeV1,
    SpeculativeUnprovenFoldResolutionErrorV1, SpeculativeUnprovenFoldResolutionReportV1,
    SpeculativeUnprovenFoldResolutionTicketV1, SpeculativeUnprovenFoldUnknownReasonV1,
    StackedFoldInitialLayerOrderV1,
    bind_speculative_unproven_tree_continuous_proof_with_control_v1,
};
use ori_domain::{EdgeId, FaceId, ProjectId};
use tauri::State;

use super::super::super::StackedFoldTransactionState;
use super::resolution::{SpeculativeUnprovenFoldResolutionDtoV1, resolution_dto_v1};
use crate::{AppState, ProjectState, lock_project};

#[path = "post_apply_proof_atomic_revert.rs"]
mod atomic_revert;
mod layered_four_face_fallback;
mod layered_three_face_fallback;
mod premise_binding;
pub(crate) use atomic_revert::{
    RevertPostApplyProofFailureRequestV1, revert_post_apply_proof_failure_v1,
};
#[cfg(test)]
use layered_four_face_fallback::{
    LayeredFourFaceFallbackDecisionV1, layered_four_face_fallback_decision_v1,
};
use layered_four_face_fallback::{
    is_layered_four_face_fallback_candidate_v1, run_layered_four_face_fallback_v1,
};
#[cfg(test)]
use layered_three_face_fallback::{
    LayeredThreeFaceFallbackDecisionV1, layered_three_face_fallback_decision_v1,
};
use layered_three_face_fallback::{
    is_layered_three_face_fallback_candidate_v1, run_layered_three_face_fallback_v1,
    target_angles_for_premise_v1,
};
#[cfg(test)]
use premise_binding::retained_premise_byte_overflow_is_rejected_for_test_v1;
use premise_binding::{
    premise_is_internally_bound_v1, retained_premise_bytes_v1,
    unstarted_job_matches_live_binding_v1,
};

include!("post_apply_proof/protocol_state.rs");
include!("post_apply_proof/job_model_and_publication.rs");
include!("post_apply_proof/test_fault_controls.rs");
include!("post_apply_proof/job_commands.rs");
include!("post_apply_proof/resolution_lifecycle.rs");
include!("post_apply_proof/proof_worker.rs");
include!("post_apply_proof/terminal_projection.rs");
include!("post_apply_proof/deadline_dispatch.rs");
include!("post_apply_proof/deadline_resource_recovery.rs");
include!("post_apply_proof/deadline_expiration.rs");
include!("post_apply_proof/registry_storage.rs");

#[cfg(test)]
#[path = "post_apply_proof_tests.rs"]
mod tests;
