#[test]
fn geometric_constraint_worker_gate_is_exclusive_and_releases_with_its_permit() {
    let gate = GeometricConstraintWorkerGate::default();
    let binding = GeometricConstraintAnalysisBinding {
        project_instance_id: ProjectId::new(),
        project_id: ProjectId::new(),
        revision: 7,
    };
    let request_generation_id = ProjectId::new();
    assert!(!gate.is_busy());
    assert_eq!(gate.pre_cancelled_count(), 0);
    let permit = gate
        .try_acquire(binding, request_generation_id)
        .expect("first worker permit");
    assert!(gate.is_busy());
    assert!(
        gate.try_acquire(binding, ProjectId::new()).is_none(),
        "parallel preflight must not allocate another worker"
    );
    assert!(
        !gate.cancel(
            GeometricConstraintAnalysisBinding {
                revision: binding.revision + 1,
                ..binding
            },
            request_generation_id,
        ),
        "a stale binding must not cancel the active worker"
    );
    assert!(
        !gate.cancel(binding, ProjectId::new()),
        "a stale request generation must not cancel the active worker"
    );
    assert!(!permit.cancellation.load(Ordering::Acquire));
    assert!(gate.cancel(binding, request_generation_id));
    assert!(permit.cancellation.load(Ordering::Acquire));
    drop(permit);
    assert!(!gate.is_busy());
    assert!(
        gate.try_acquire(binding, ProjectId::new()).is_some(),
        "the released gate must admit the next request generation"
    );
}

#[test]
fn geometric_constraint_gate_consumes_exact_cancel_before_acquire_once() {
    let gate = GeometricConstraintWorkerGate::default();
    let binding = GeometricConstraintAnalysisBinding {
        project_instance_id: ProjectId::new(),
        project_id: ProjectId::new(),
        revision: 11,
    };
    let request_generation_id = ProjectId::new();

    assert!(gate.cancel(binding, request_generation_id));
    assert!(gate.cancel(binding, request_generation_id));
    assert_eq!(
        gate.pre_cancelled_count(),
        1,
        "duplicate early cancellation must occupy one bounded slot"
    );
    let cancelled = gate
        .try_acquire(binding, request_generation_id)
        .expect("the matching request must still acquire the worker slot");
    assert!(cancelled.cancellation.load(Ordering::Acquire));
    assert_eq!(gate.pre_cancelled_count(), 0);
    drop(cancelled);

    let next_generation = gate
        .try_acquire(binding, ProjectId::new())
        .expect("the next generation must acquire independently");
    assert!(
        !next_generation.cancellation.load(Ordering::Acquire),
        "an early cancellation must be consumed only by its exact generation"
    );
}

#[test]
fn geometric_constraint_gate_retains_queued_cancel_while_another_generation_is_active() {
    let gate = GeometricConstraintWorkerGate::default();
    let binding = GeometricConstraintAnalysisBinding {
        project_instance_id: ProjectId::new(),
        project_id: ProjectId::new(),
        revision: 12,
    };
    let active_generation = ProjectId::new();
    let queued_generation = ProjectId::new();
    let active = gate
        .try_acquire(binding, active_generation)
        .expect("the first generation must acquire");

    assert!(
        !gate.cancel(binding, queued_generation),
        "the queued generation is not the currently active worker"
    );
    assert!(
        !active.cancellation.load(Ordering::Acquire),
        "a queued generation must not cancel the active generation"
    );
    assert_eq!(gate.pre_cancelled_count(), 1);
    drop(active);

    let queued = gate
        .try_acquire(binding, queued_generation)
        .expect("the queued generation must acquire after the active worker exits");
    assert!(
        queued.cancellation.load(Ordering::Acquire),
        "cancel arriving before the queued analyze future is first polled must be retained"
    );
    assert_eq!(gate.pre_cancelled_count(), 0);
}

#[test]
fn geometric_constraint_pre_cancel_ledger_is_bounded_and_evicts_oldest_only() {
    let gate = GeometricConstraintWorkerGate::default();
    let binding = GeometricConstraintAnalysisBinding {
        project_instance_id: ProjectId::new(),
        project_id: ProjectId::new(),
        revision: 13,
    };
    let request_generations = (0..=MAX_GEOMETRIC_CONSTRAINT_PRE_CANCELLED_REQUESTS)
        .map(|_| ProjectId::new())
        .collect::<Vec<_>>();
    for request_generation_id in &request_generations {
        assert!(gate.cancel(binding, *request_generation_id));
    }
    assert_eq!(
        gate.pre_cancelled_count(),
        MAX_GEOMETRIC_CONSTRAINT_PRE_CANCELLED_REQUESTS
    );

    let evicted = gate
        .try_acquire(binding, request_generations[0])
        .expect("the oldest evicted generation can acquire normally");
    assert!(!evicted.cancellation.load(Ordering::Acquire));
    drop(evicted);
    let newest = gate
        .try_acquire(
            binding,
            *request_generations
                .last()
                .expect("at least one request generation"),
        )
        .expect("the newest retained generation can acquire");
    assert!(newest.cancellation.load(Ordering::Acquire));
}

#[test]
fn geometric_constraint_gate_publishes_each_successful_acquire_before_cancel_can_observe_it() {
    for revision in 0..128 {
        let gate = GeometricConstraintWorkerGate::default();
        let binding = GeometricConstraintAnalysisBinding {
            project_instance_id: ProjectId::new(),
            project_id: ProjectId::new(),
            revision,
        };
        let request_generation_id = ProjectId::new();
        let worker_gate = gate.clone();
        let (acquired_tx, acquired_rx) = mpsc::sync_channel(0);
        let (release_tx, release_rx) = mpsc::sync_channel(0);
        let worker = thread::spawn(move || {
            let permit = worker_gate
                .try_acquire(binding, request_generation_id)
                .expect("the fresh gate must admit one worker");
            acquired_tx
                .send(permit.cancellation())
                .expect("publish acquired cancellation token");
            release_rx.recv().expect("release acquired worker permit");
            drop(permit);
        });
        let cancellation = acquired_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("the worker must publish its successful acquisition");

        assert!(
            gate.cancel(binding, request_generation_id),
            "a published successful acquisition must always be cancellable"
        );
        assert!(cancellation.load(Ordering::Acquire));
        release_tx.send(()).expect("release worker");
        worker.join().expect("worker must not panic");
        assert!(!gate.is_busy());
    }
}

#[test]
fn geometric_constraint_worker_cancel_is_bound_to_exact_request_generation() {
    let state = Arc::new(AppState::new(initial_project_state()));
    let binding_tuple = geometric_constraint_binding(&state);
    let binding = GeometricConstraintAnalysisBinding {
        project_instance_id: binding_tuple.0,
        project_id: binding_tuple.1,
        revision: binding_tuple.2,
    };
    let request_generation_id = ProjectId::new();
    let before = geometric_constraint_project_signature(&state);
    let worker_state = Arc::clone(&state);
    let (entered_tx, entered_rx) = mpsc::sync_channel(0);
    let (release_tx, release_rx) = mpsc::sync_channel(0);

    let worker = thread::spawn(move || {
        tauri::async_runtime::block_on(analyze_geometric_constraints_with_worker(
            &worker_state,
            binding.project_instance_id,
            binding.project_id,
            binding.revision,
            request_generation_id,
            move |pattern, document, runtime| {
                entered_tx.send(()).expect("announce worker entry");
                release_rx.recv().expect("release constraint worker");
                Ok(analyze_geometric_constraint_document_with_observer(
                    &pattern,
                    &document,
                    &mut GeometricConstraintAnalysisObserver::new(runtime),
                ))
            },
        ))
    });
    entered_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("geometric-constraint worker must start");

    assert!(
        !state.cancel_geometric_constraint_worker(binding, ProjectId::new()),
        "a stale generation must not cancel the current worker"
    );
    assert!(
        !state.cancel_geometric_constraint_worker(
            GeometricConstraintAnalysisBinding {
                revision: binding.revision + 1,
                ..binding
            },
            request_generation_id,
        ),
        "a stale binding must not cancel the current worker"
    );
    assert!(
        state.cancel_geometric_constraint_worker(binding, request_generation_id),
        "the exact binding and request generation must cancel the worker"
    );
    release_tx.send(()).expect("release cancelled worker");
    let response = worker
        .join()
        .expect("analysis caller must not panic")
        .expect("cancelled analysis returns a bound fail-closed result");

    assert_eq!(response.project_instance_id, binding.project_instance_id);
    assert_eq!(response.project_id, binding.project_id);
    assert_eq!(response.revision, binding.revision);
    assert!(matches!(
        response.result,
        GeometricConstraintPreflightResult::Unknown {
            reason: GeometricConstraintUnknownReason::Cancelled,
            ..
        }
    ));
    assert!(!state.geometric_constraint_worker_is_busy());
    assert_eq!(geometric_constraint_project_signature(&state), before);
}

#[test]
fn abandoned_geometric_constraint_waiter_keeps_gate_until_worker_exit_then_retries() {
    let state = Arc::new(AppState::new(initial_project_state()));
    let binding = geometric_constraint_binding(&state);
    let before = geometric_constraint_project_signature(&state);
    let worker_state = Arc::clone(&state);
    let (entered_tx, entered_rx) = mpsc::sync_channel(0);
    let (release_tx, release_rx) = mpsc::sync_channel(0);

    let waiting = tauri::async_runtime::spawn(async move {
        analyze_geometric_constraints_with_worker(
            &worker_state,
            binding.0,
            binding.1,
            binding.2,
            ProjectId::new(),
            move |pattern, document, _runtime| {
                entered_tx.send(()).expect("announce worker entry");
                release_rx.recv().expect("release constraint worker");
                Ok(analyze_geometric_constraint_document(&pattern, &document))
            },
        )
        .await
    });

    entered_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("geometric-constraint worker must start");
    assert!(state.geometric_constraint_worker_is_busy());
    waiting.abort();
    assert!(
        tauri::async_runtime::block_on(waiting).is_err(),
        "the abandoned waiting future must be cancelled"
    );
    assert!(
        state.geometric_constraint_worker_is_busy(),
        "cancelling the waiter must not release a running blocking worker"
    );

    let busy_error = tauri::async_runtime::block_on(analyze_geometric_constraints_with_worker(
        &state,
        binding.0,
        binding.1,
        binding.2,
        ProjectId::new(),
        |_, _, _runtime| {
            panic!("a busy gate must reject before invoking another worker");
        },
    ))
    .expect_err("parallel analysis must be rejected");
    assert_eq!(busy_error, GEOMETRIC_CONSTRAINT_ANALYSIS_BUSY_MESSAGE);

    release_tx
        .send(())
        .expect("release abandoned geometric-constraint worker");
    wait_for_geometric_constraint_worker_idle(&state);
    assert!(!state.geometric_constraint_worker_is_busy());

    let retried = run_default_geometric_constraint_analysis(&state, binding)
        .expect("the gate must be reusable after the blocking worker exits");
    assert_eq!(retried.project_instance_id, binding.0);
    assert_eq!(retried.project_id, binding.1);
    assert_eq!(retried.revision, binding.2);
    assert_eq!(
        retried.result,
        GeometricConstraintPreflightResult::NoDirectConflict
    );
    assert_eq!(geometric_constraint_project_signature(&state), before);
}

#[test]
fn geometric_constraint_worker_releases_project_lock_and_discards_reopen_aba_completion() {
    let state = Arc::new(AppState::new(initial_project_state()));
    let stale_binding = geometric_constraint_binding(&state);
    let document = {
        let project = lock_project(&state).expect("capture original project document");
        project.document()
    };
    let worker_state = Arc::clone(&state);
    let (entered_tx, entered_rx) = mpsc::sync_channel(0);
    let (release_tx, release_rx) = mpsc::sync_channel(0);

    let analysis = thread::spawn(move || {
        tauri::async_runtime::block_on(analyze_geometric_constraints_with_worker(
            &worker_state,
            stale_binding.0,
            stale_binding.1,
            stale_binding.2,
            ProjectId::new(),
            move |pattern, constraints, _runtime| {
                entered_tx.send(()).expect("announce worker entry");
                release_rx.recv().expect("release constraint worker");
                Ok(analyze_geometric_constraint_document(
                    &pattern,
                    &constraints,
                ))
            },
        ))
    });

    entered_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("geometric-constraint worker must start");
    let (current_binding, reopened_before) = {
        let Ok(mut project) = state.0.try_lock() else {
            release_tx
                .send(())
                .expect("release blocked geometric-constraint worker");
            analysis
                .join()
                .expect("analysis caller must not panic")
                .expect("unchanged analysis must finish");
            panic!("the project lock must be released during constraint analysis");
        };
        *project =
            ProjectState::from_valid_document(document, PathBuf::from("same-constraints.ori2"));
        assert_eq!(project.project_id, stale_binding.1);
        assert_eq!(project.editor.revision(), stale_binding.2);
        assert_ne!(project.instance_id, stale_binding.0);
        (
            (
                project.instance_id,
                project.project_id,
                project.editor.revision(),
            ),
            project_state_signature(&project),
        )
    };

    release_tx
        .send(())
        .expect("release stale geometric-constraint worker");
    let stale_error = analysis
        .join()
        .expect("analysis caller must not panic")
        .expect_err("same-ID and revision reopen must reject stale completion");
    assert_eq!(
        stale_error,
        "the open project instance changed while the file dialog was open"
    );
    assert!(!state.geometric_constraint_worker_is_busy());
    assert_eq!(
        geometric_constraint_project_signature(&state),
        reopened_before
    );

    let retried = run_default_geometric_constraint_analysis(&state, current_binding)
        .expect("the reopened instance must be able to retry");
    assert_eq!(retried.project_instance_id, current_binding.0);
    assert_eq!(retried.project_id, current_binding.1);
    assert_eq!(retried.revision, current_binding.2);
    assert_eq!(
        geometric_constraint_project_signature(&state),
        reopened_before
    );
}

#[test]
fn geometric_constraint_worker_failures_are_redacted_release_gate_and_preserve_state() {
    let state = Arc::new(AppState::new(initial_project_state()));
    let binding = geometric_constraint_binding(&state);
    let before = geometric_constraint_project_signature(&state);
    let private_failure = r"C:\Users\alice\private-constraints.ori2; constraint_id=secret-17";

    let reported_error = tauri::async_runtime::block_on(analyze_geometric_constraints_with_worker(
        &state,
        binding.0,
        binding.1,
        binding.2,
        ProjectId::new(),
        move |_, _, _runtime| Err(private_failure.to_owned()),
    ))
    .expect_err("a reported worker failure must fail the command");
    assert_eq!(reported_error, GEOMETRIC_CONSTRAINT_ANALYSIS_FAILED_MESSAGE);
    assert!(!reported_error.contains("alice"));
    assert!(!reported_error.contains("private-constraints"));
    assert!(!reported_error.contains("secret-17"));
    assert!(!state.geometric_constraint_worker_is_busy());
    assert_eq!(geometric_constraint_project_signature(&state), before);
    run_default_geometric_constraint_analysis(&state, binding)
        .expect("the gate must be reusable after a reported worker failure");

    let private_panic = r"C:\Users\bob\private-constraints.ori2; constraint_id=panic-secret-23";
    let panic_error = tauri::async_runtime::block_on(analyze_geometric_constraints_with_worker(
        &state,
        binding.0,
        binding.1,
        binding.2,
        ProjectId::new(),
        move |_, _, _runtime| -> Result<GeometricConstraintPreflightResult, String> {
            panic!("{private_panic}");
        },
    ))
    .expect_err("a panicking worker must fail the command");
    assert_eq!(panic_error, GEOMETRIC_CONSTRAINT_ANALYSIS_FAILED_MESSAGE);
    assert!(!panic_error.contains("bob"));
    assert!(!panic_error.contains("private-constraints"));
    assert!(!panic_error.contains("panic-secret-23"));
    assert!(!state.geometric_constraint_worker_is_busy());
    assert_eq!(geometric_constraint_project_signature(&state), before);
    run_default_geometric_constraint_analysis(&state, binding)
        .expect("the gate must be reusable after a panicking worker");
    assert_eq!(geometric_constraint_project_signature(&state), before);
}

#[test]
fn geometric_constraint_capture_rejections_and_success_all_release_gate() {
    let state = Arc::new(AppState::new(initial_project_state()));
    let binding = geometric_constraint_binding(&state);
    let before = geometric_constraint_project_signature(&state);
    let rejection_cases = [
        (
            (ProjectId::new(), binding.1, binding.2),
            "the open project instance changed while the file dialog was open",
        ),
        (
            (binding.0, ProjectId::new(), binding.2),
            "the active project changed before the command was applied",
        ),
        (
            (binding.0, binding.1, binding.2 + 1),
            "the project changed while the file dialog was open",
        ),
    ];

    for (rejected_binding, expected_error) in rejection_cases {
        let error = tauri::async_runtime::block_on(analyze_geometric_constraints_with_worker(
            &state,
            rejected_binding.0,
            rejected_binding.1,
            rejected_binding.2,
            ProjectId::new(),
            |_, _, _runtime| {
                panic!("capture rejection must happen before worker invocation");
            },
        ))
        .expect_err("invalid capture binding must be rejected");
        assert_eq!(error, expected_error);
        assert!(!state.geometric_constraint_worker_is_busy());
        assert_eq!(geometric_constraint_project_signature(&state), before);
    }

    let response = run_default_geometric_constraint_analysis(&state, binding)
        .expect("a valid capture and worker must succeed");
    assert_eq!(response.project_instance_id, binding.0);
    assert_eq!(response.project_id, binding.1);
    assert_eq!(response.revision, binding.2);
    assert!(!state.geometric_constraint_worker_is_busy());
    assert_eq!(geometric_constraint_project_signature(&state), before);
}

#[test]
fn lock_and_expect_preserves_project_expectation_order_and_errors() {
    let state = AppState::new(initial_project_state());
    let binding = {
        let project = lock_project(&state).expect("project lock");
        ProjectExpectation::new(
            project.instance_id,
            project.project_id,
            project.editor.revision(),
        )
    };

    let project = lock_and_expect(&state, binding).expect("matching expectation");
    assert_eq!(project.instance_id, binding.instance_id);
    assert_eq!(project.project_id, binding.project_id);
    assert_eq!(project.editor.revision(), binding.revision);
    drop(project);

    let Err(instance_error) = lock_and_expect(
        &state,
        ProjectExpectation::new(ProjectId::new(), binding.project_id, binding.revision),
    ) else {
        panic!("instance mismatch must fail");
    };
    assert_eq!(
        instance_error,
        "the open project instance changed while the file dialog was open"
    );

    let Err(project_error) = lock_and_expect(
        &state,
        ProjectExpectation::new(binding.instance_id, ProjectId::new(), binding.revision),
    ) else {
        panic!("project mismatch must fail");
    };
    assert_eq!(
        project_error,
        "the active project changed before the command was applied"
    );

    let Err(revision_error) = lock_and_expect(
        &state,
        ProjectExpectation::new(
            binding.instance_id,
            binding.project_id,
            binding.revision + 1,
        ),
    ) else {
        panic!("revision mismatch must fail");
    };
    assert_eq!(
        revision_error,
        "the project changed while the file dialog was open"
    );
}
