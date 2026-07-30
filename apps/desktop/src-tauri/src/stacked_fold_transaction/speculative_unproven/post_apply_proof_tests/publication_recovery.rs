use super::*;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

fn prepare_layered_certified_resolution_job_v1() -> (AppState, PostApplyProofJobV1) {
    let (app_state, transaction_state, _, _) = prepare_started_actual_job_v1();
    let mut job = {
        let mut registry = transaction_state.3.lock().expect("post-Apply registry");
        let job = registry.jobs.pop_front().expect("published production job");
        registry.retained_bytes = registry.retained_bytes.saturating_sub(job.retained_bytes);
        registry.deadline_scheduler_registered = false;
        job
    };
    let premise = job.premise.take().expect("retained production premise");
    let deadline = Instant::now()
        .checked_add(Duration::from_secs(30))
        .expect("bounded test deadline");
    let attempt = run_attempt_v1(
        &job.job_token,
        premise,
        POST_APPLY_PROOF_SAMPLE_INTERVALS_V1[0],
        &AtomicBool::new(false),
        deadline,
    );
    let PostApplyProofWorkerCertificateV1::Certified(
        PostApplyProofCertifiedAuthorityV1::LayeredThreeFace(proof),
    ) = attempt.certificate
    else {
        panic!("production strip must issue only the layered typed authority");
    };
    job.resolution_report = None;
    job.state = PostApplyProofJobStateV1::Resolving {
        run_generation: 1,
        resolution: PostApplyProofResolutionV1::Certified(
            PostApplyProofCertifiedAuthorityV1::LayeredThreeFace(proof),
        ),
    };
    (app_state, job)
}

fn foreign_project_for_certified_retry_v1(app_state: &AppState) -> ProjectState {
    let project = crate::lock_project(app_state).expect("authority-owning project");
    ProjectState::new_with_paper(
        project.editor.pattern().clone(),
        project.editor.paper().clone(),
    )
}

#[test]
fn certified_resolver_panic_before_handoff_keeps_the_exact_authority_for_owner_retry_v1() {
    let _generation_guard =
        crate::stacked_fold_read::tests::lock_stacked_fold_read_generation_test();
    let (app_state, mut job) = prepare_layered_certified_resolution_job_v1();
    let _panic_guard = panic_next_post_apply_certified_resolution_before_v1(&job.job_token);
    let mut project = crate::lock_project(&app_state).expect("project");
    assert_eq!(
        resolve_locked_terminal_v1(&mut project, &mut job, PostApplyProofTerminalV1::Certified),
        Err(())
    );
    assert_eq!(
        project
            .editor
            .speculative_unproven_fold_summary_v1()
            .applied
            .awaiting_proof,
        1
    );
    assert!(matches!(
        &job.state,
        PostApplyProofJobStateV1::Resolving {
            resolution: PostApplyProofResolutionV1::Certified(
                PostApplyProofCertifiedAuthorityV1::LayeredThreeFace(_)
            ),
            ..
        }
    ));
    assert_eq!(progress_v1(&job).status, "proving");
    job.proof_deadline = Instant::now();
    resolve_locked_terminal_v1(
        &mut project,
        &mut job,
        PostApplyProofTerminalV1::UnknownResourceLimit,
    )
    .expect("the owner retry consumes the retained authority before a stop");
    assert!(matches!(
        &job.state,
        PostApplyProofJobStateV1::Terminal(PostApplyProofTerminalV1::Certified)
    ));
    let summary = project.editor.speculative_unproven_fold_summary_v1();
    assert_eq!(summary.applied.awaiting_proof, 0);
    assert_eq!(summary.applied.total(), 0);
}

#[test]
fn certified_panic_retry_is_idempotent_across_duplicate_start_and_poll_v1() {
    let _generation_guard =
        crate::stacked_fold_read::tests::lock_stacked_fold_read_generation_test();
    let _deadline_override_guard =
        set_next_post_apply_proof_deadline_v1(Duration::from_secs(5 * 60));
    let (app_state, transaction_state, request, _) = prepare_started_actual_job_v1();
    let premise = transaction_state
        .3
        .lock()
        .expect("post-Apply registry")
        .jobs
        .front_mut()
        .and_then(|job| job.premise.take())
        .expect("retained production premise");
    let attempt = run_attempt_v1(
        &request.job_token,
        premise,
        POST_APPLY_PROOF_SAMPLE_INTERVALS_V1[0],
        &AtomicBool::new(false),
        Instant::now()
            .checked_add(Duration::from_secs(30))
            .expect("bounded test deadline"),
    );
    let PostApplyProofWorkerCertificateV1::Certified(authority) = attempt.certificate else {
        panic!("production strip must issue typed authority");
    };
    {
        let mut registry = transaction_state.3.lock().expect("post-Apply registry");
        registry.jobs.front_mut().expect("published job").state =
            PostApplyProofJobStateV1::Resolving {
                run_generation: 7,
                resolution: PostApplyProofResolutionV1::Certified(authority),
            };
    }

    let _panic_guard = panic_next_post_apply_certified_resolution_before_v1(&request.job_token);
    let interrupted = tauri::async_runtime::block_on(poll_post_apply_proof_job_inner_v1(
        &app_state,
        &transaction_state,
        request.clone(),
    ))
    .expect("the panic boundary keeps the job retryable");
    assert_eq!(interrupted.status, "proving");

    let start_request = || StartPostApplyProofJobRequestV1 {
        version: POST_APPLY_PROOF_PROTOCOL_VERSION_V1,
        project_instance_id: request.project_instance_id,
        project_id: request.project_id,
        revision: request.revision,
    };
    let certified =
        start_post_apply_proof_job_inner_v1(&app_state, &transaction_state, start_request())
            .expect("duplicate start retries the exact retained authority");
    assert_eq!(certified.status, "certified");
    let duplicate_start =
        start_post_apply_proof_job_inner_v1(&app_state, &transaction_state, start_request())
            .expect("terminal duplicate start is idempotent");
    assert_eq!(duplicate_start, certified);
    let duplicate_poll = tauri::async_runtime::block_on(poll_post_apply_proof_job_inner_v1(
        &app_state,
        &transaction_state,
        request,
    ))
    .expect("terminal duplicate poll is idempotent");
    assert_eq!(duplicate_poll, certified);
}

#[test]
fn certified_binding_not_found_rejection_retains_the_exact_authority_v1() {
    let _generation_guard =
        crate::stacked_fold_read::tests::lock_stacked_fold_read_generation_test();
    let (app_state, mut job) = prepare_layered_certified_resolution_job_v1();
    let mut project = crate::lock_project(&app_state).expect("project");
    let mut owner_snapshot =
        crate::stacked_fold_transaction::StackedFoldProjectRollbackSnapshotV1::capture(&project);
    let revision = project.editor.revision();
    project
        .editor
        .undo(revision)
        .expect("move the exact mark to Redo");
    let revision = project.editor.revision();
    project
        .editor
        .execute(
            revision,
            ori_core::Command::UpdateProjectMemo {
                memo: "abandon the marked Redo branch".to_owned(),
            },
        )
        .expect("drop the marked Redo branch");
    assert!(matches!(
        project
            .editor
            .inspect_speculative_unproven_fold_v1(&job.binding),
        Err(SpeculativeUnprovenFoldResolutionErrorV1::BindingNotFound)
    ));
    assert_eq!(
        resolve_locked_terminal_v1(&mut project, &mut job, PostApplyProofTerminalV1::Certified),
        Err(())
    );
    assert!(matches!(
        &job.state,
        PostApplyProofJobStateV1::Resolving {
            resolution: PostApplyProofResolutionV1::Certified(
                PostApplyProofCertifiedAuthorityV1::LayeredThreeFace(_)
            ),
            ..
        }
    ));
    assert_eq!(progress_v1(&job).status, "proving");
    assert_eq!(
        project
            .editor
            .speculative_unproven_fold_summary_v1()
            .applied
            .total(),
        0
    );
    owner_snapshot
        .restore(&mut project)
        .expect("restore the exact authority-owning editor image");
    resolve_locked_certified_terminal_v1(&mut project, &mut job)
        .expect("the same proof retries successfully after its binding returns");
    assert!(matches!(
        &job.state,
        PostApplyProofJobStateV1::Terminal(PostApplyProofTerminalV1::Certified)
    ));
    assert_eq!(
        project
            .editor
            .speculative_unproven_fold_summary_v1()
            .applied
            .total(),
        0
    );
}

#[test]
fn layered_three_face_foreign_resolver_failure_then_owner_retry_consumes_once_v1() {
    let _generation_guard =
        crate::stacked_fold_read::tests::lock_stacked_fold_read_generation_test();
    let (app_state, mut job) = prepare_layered_certified_resolution_job_v1();
    let mut foreign = foreign_project_for_certified_retry_v1(&app_state);
    assert_eq!(
        resolve_locked_certified_terminal_v1(&mut foreign, &mut job),
        Err(())
    );
    assert!(matches!(
        &job.state,
        PostApplyProofJobStateV1::Resolving {
            resolution: PostApplyProofResolutionV1::Certified(
                PostApplyProofCertifiedAuthorityV1::LayeredThreeFace(_)
            ),
            ..
        }
    ));
    let mut project = crate::lock_project(&app_state).expect("project");
    assert_eq!(
        project
            .editor
            .speculative_unproven_fold_summary_v1()
            .applied
            .awaiting_proof,
        1
    );
    resolve_locked_certified_terminal_v1(&mut project, &mut job)
        .expect("the authority-owning editor accepts the exact recovered proof");
    assert_eq!(
        project
            .editor
            .speculative_unproven_fold_summary_v1()
            .applied
            .awaiting_proof,
        0
    );
    assert!(matches!(
        &job.state,
        PostApplyProofJobStateV1::Terminal(PostApplyProofTerminalV1::Certified)
    ));
    assert_eq!(
        resolve_locked_certified_terminal_v1(&mut project, &mut job),
        Err(()),
        "a duplicate direct resolver call cannot consume authority twice"
    );
    assert!(matches!(
        &job.state,
        PostApplyProofJobStateV1::Terminal(PostApplyProofTerminalV1::Certified)
    ));
}

#[test]
fn layered_three_face_resolver_panic_after_consumption_recovers_certified_v1() {
    let _generation_guard =
        crate::stacked_fold_read::tests::lock_stacked_fold_read_generation_test();
    let (app_state, mut job) = prepare_layered_certified_resolution_job_v1();
    let _panic_guard = panic_next_post_apply_certified_resolution_after_v1(&job.job_token);
    let mut project = crate::lock_project(&app_state).expect("project");
    resolve_locked_certified_terminal_v1(&mut project, &mut job)
        .expect("exact absence after layered consumption recovers Certified");
    assert!(matches!(
        &job.state,
        PostApplyProofJobStateV1::Terminal(PostApplyProofTerminalV1::Certified)
    ));
    assert!(matches!(
        project
            .editor
            .inspect_speculative_unproven_fold_v1(&job.binding),
        Err(SpeculativeUnprovenFoldResolutionErrorV1::BindingNotFound)
    ));
    assert_eq!(
        project
            .editor
            .speculative_unproven_fold_summary_v1()
            .applied
            .total(),
        0
    );
}

#[test]
fn late_layered_worker_authority_cannot_resolve_a_replacement_generation_v1() {
    let _generation_guard =
        crate::stacked_fold_read::tests::lock_stacked_fold_read_generation_test();
    let (app_state, transaction_state, request, _) = prepare_started_actual_job_v1();
    let old_generation = 71;
    let replacement_generation = 72;
    let old_premise = {
        let mut registry = transaction_state.3.lock().expect("post-Apply registry");
        let job = registry.jobs.front_mut().expect("published job");
        let premise = job.premise.take().expect("worker-owned premise");
        job.state = PostApplyProofJobStateV1::InFlight {
            run_generation: old_generation,
            stage: 0,
            cancellation: std::sync::Arc::new(AtomicBool::new(false)),
        };
        premise
    };
    let deadline = Instant::now()
        .checked_add(Duration::from_secs(30))
        .expect("bounded worker deadline");
    let late_attempt = run_attempt_v1(
        &request.job_token,
        old_premise,
        POST_APPLY_PROOF_SAMPLE_INTERVALS_V1[0],
        &AtomicBool::new(false),
        deadline,
    );
    assert!(matches!(
        &late_attempt.certificate,
        PostApplyProofWorkerCertificateV1::Certified(
            PostApplyProofCertifiedAuthorityV1::LayeredThreeFace(_)
        )
    ));

    let replacement_cancellation = std::sync::Arc::new(AtomicBool::new(false));
    {
        let mut registry = transaction_state.3.lock().expect("replacement registry");
        let job = registry.jobs.front_mut().expect("replacement job");
        job.state = PostApplyProofJobStateV1::InFlight {
            run_generation: replacement_generation,
            stage: 0,
            cancellation: std::sync::Arc::clone(&replacement_cancellation),
        };
    }
    complete_worker_attempt_v1(
        &transaction_state.3,
        &request,
        old_generation,
        0,
        late_attempt,
    );
    {
        let project = crate::lock_project(&app_state).expect("awaiting replacement project");
        assert_eq!(
            project
                .editor
                .speculative_unproven_fold_summary_v1()
                .applied
                .awaiting_proof,
            1,
            "a late authority must not resolve the replacement Awaiting mark"
        );
        let registry = transaction_state.3.lock().expect("replacement registry");
        assert!(matches!(
            &registry.jobs.front().expect("replacement job").state,
            PostApplyProofJobStateV1::InFlight { run_generation, stage: 0, .. }
                if *run_generation == replacement_generation
        ));
    }
    assert!(
        !replacement_cancellation.load(std::sync::atomic::Ordering::Acquire),
        "a discarded old completion must not signal the replacement cancellation token"
    );
}
