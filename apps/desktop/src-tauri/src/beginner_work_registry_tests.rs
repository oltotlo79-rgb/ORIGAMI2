#[test]
fn beginner_grid_registration_sequence_fails_closed_before_wrap() {
    let sequence = AtomicU64::new(u64::MAX - 1);

    assert_eq!(
        beginner_design_commands::take_beginner_grid_registration_sequence_v1(&sequence),
        Ok(u64::MAX - 1)
    );
    assert_eq!(sequence.load(AtomicOrdering::Acquire), u64::MAX);
    assert_eq!(
        beginner_design_commands::take_beginner_grid_registration_sequence_v1(&sequence),
        Err("grid_registration_sequence_exhausted".to_owned())
    );
    assert_eq!(
        sequence.load(AtomicOrdering::Acquire),
        u64::MAX,
        "sequence exhaustion must not wrap to zero"
    );

    let previously_wrapped = AtomicU64::new(0);
    assert_eq!(
        beginner_design_commands::take_beginner_grid_registration_sequence_v1(&previously_wrapped,),
        Err("grid_registration_sequence_exhausted".to_owned())
    );
    assert_eq!(previously_wrapped.load(AtomicOrdering::Acquire), 0);
}

#[test]
fn beginner_generation_tombstone_capacity_seals_without_forgetting_ids() {
    let first = ProjectId::new();
    let second = ProjectId::new();
    let third = ProjectId::new();
    let mut tombstones = std::collections::HashSet::new();
    let sealed = AtomicBool::new(false);

    assert_eq!(
        beginner_design_commands::reserve_generation_tombstone_v1(
            &mut tombstones,
            &sealed,
            first,
            1,
            "bounded",
        ),
        Ok(())
    );
    assert!(tombstones.contains(&first));
    assert_eq!(
        beginner_design_commands::reserve_generation_tombstone_v1(
            &mut tombstones,
            &sealed,
            second,
            1,
            "bounded",
        ),
        Err("bounded".to_owned())
    );
    assert!(sealed.load(Ordering::Acquire));
    assert!(tombstones.contains(&first));
    assert!(!tombstones.contains(&second));

    tombstones.clear();
    assert_eq!(
        beginner_design_commands::reserve_generation_tombstone_v1(
            &mut tombstones,
            &sealed,
            third,
            1,
            "bounded",
        ),
        Err("bounded".to_owned()),
        "an exhausted process-lifetime ledger must never resume after forgetting entries"
    );
    assert!(tombstones.is_empty());
}

#[test]
fn beginner_grid_registration_duplicate_preserves_exact_cancel_and_progress_owner() {
    let _serial = serial_beginner_grid_test();
    let generation = ProjectId::new();
    let foreign_generation = ProjectId::new();
    let work = Arc::new(BeginnerGridWork::default());
    let foreign_work = Arc::new(BeginnerGridWork::default());
    let registration = beginner_design_commands::register_beginner_grid_work_v1(generation, &work)
        .expect("register original grid work");
    let foreign_registration =
        beginner_design_commands::register_beginner_grid_work_v1(foreign_generation, &foreign_work)
            .expect("register foreign grid work");
    let duplicate = Arc::new(BeginnerGridWork::default());

    assert_eq!(
        beginner_design_commands::register_beginner_grid_work_v1(generation, &duplicate)
            .err()
            .as_deref(),
        Some("grid_generation_reused")
    );
    {
        let registry = beginner_grid_work().lock().unwrap();
        assert!(Arc::ptr_eq(
            registry
                .get(&generation)
                .expect("original remains registered"),
            &work
        ));
        assert!(Arc::ptr_eq(
            registry
                .get(&foreign_generation)
                .expect("foreign work remains registered"),
            &foreign_work
        ));
    }

    cancel_beginner_parameter_grid(generation).expect("cancel exact generation");
    assert!(work.cancelled.load(Ordering::Acquire));
    assert!(!duplicate.cancelled.load(Ordering::Acquire));
    assert!(!foreign_work.cancelled.load(Ordering::Acquire));
    assert_eq!(
        get_beginner_parameter_grid_progress(generation)
            .expect("cancelled progress remains queryable")
            .terminal_state,
        "cancelled"
    );
    assert_eq!(
        get_beginner_parameter_grid_progress(foreign_generation)
            .expect("foreign progress remains queryable")
            .terminal_state,
        "running"
    );

    drop(registration);
    assert_eq!(
        get_beginner_parameter_grid_progress(foreign_generation)
            .expect("foreign progress survives another registration drop")
            .terminal_state,
        "running"
    );
    drop(foreign_registration);
    assert_eq!(
        get_beginner_parameter_grid_progress(foreign_generation)
            .expect("abandoned foreign progress is retained")
            .terminal_state,
        "failed"
    );
}

#[test]
fn reference_consensus_generation_id_remains_tombstoned_after_owner_drop() {
    let _serial = serial_beginner_grid_test();
    let generation = ProjectId::new();
    let original_work = Arc::new(ReferenceConsensusWorkV1::default());

    assert_eq!(
        beginner_design_commands::run_registered_reference_consensus_work_v1(
            generation,
            &original_work,
            || Ok(37_u8),
        ),
        Ok(37)
    );
    assert!(
        !reference_consensus_work_v1()
            .lock()
            .unwrap()
            .contains_key(&generation),
        "completed consensus work is removed from the live registry"
    );

    let replacement_work = Arc::new(ReferenceConsensusWorkV1::default());
    assert_eq!(
        beginner_design_commands::register_reference_consensus_work_v1(
            generation,
            &replacement_work,
        )
        .err()
        .as_deref(),
        Some("reference_consensus_generation_reused"),
        "a delayed cancel handle must never be rebound to replacement work"
    );
    assert!(!replacement_work.registration_active.load(Ordering::Acquire));
    assert_eq!(replacement_work.terminal.load(Ordering::Acquire), 0);
    assert_eq!(
        cancel_reference_consensus(generation),
        Err("reference_consensus_generation_not_running".to_owned())
    );
    assert!(!replacement_work.cancelled.load(Ordering::Acquire));
}

#[test]
fn beginner_grid_registration_handles_early_return_unwind_and_stale_aba_drop() {
    let _serial = serial_beginner_grid_test();
    let early_generation = ProjectId::new();
    let early_work = Arc::new(BeginnerGridWork::default());
    let early_result = (|| -> Result<(), String> {
        let _registration = beginner_design_commands::register_beginner_grid_work_v1(
            early_generation,
            &early_work,
        )?;
        Err("simulated_early_return".to_owned())
    })();
    assert_eq!(early_result, Err("simulated_early_return".to_owned()));
    assert_eq!(
        get_beginner_parameter_grid_progress(early_generation)
            .expect("early-return progress remains queryable")
            .terminal_state,
        "failed"
    );

    let unwind_generation = ProjectId::new();
    let unwind_work = Arc::new(BeginnerGridWork::default());
    let unwind = std::panic::catch_unwind(|| {
        let _registration = beginner_design_commands::register_beginner_grid_work_v1(
            unwind_generation,
            &unwind_work,
        )
        .expect("register unwind grid work");
        panic!("simulate grid worker unwind");
    });
    assert!(unwind.is_err());
    assert_eq!(
        get_beginner_parameter_grid_progress(unwind_generation)
            .expect("unwind progress remains queryable")
            .terminal_state,
        "failed"
    );

    let reused_generation = ProjectId::new();
    let original_work = Arc::new(BeginnerGridWork::default());
    let stale_registration =
        beginner_design_commands::register_beginner_grid_work_v1(reused_generation, &original_work)
            .expect("register original ABA work");
    let replacement_work = Arc::new(BeginnerGridWork::default());
    beginner_grid_work()
        .lock()
        .unwrap()
        .insert(reused_generation, Arc::clone(&replacement_work));

    drop(stale_registration);
    assert_eq!(original_work.terminal.load(Ordering::Acquire), 3);
    assert!(!original_work.registration_active.load(Ordering::Acquire));
    assert_eq!(replacement_work.terminal.load(Ordering::Acquire), 0);
    assert!(Arc::ptr_eq(
        beginner_grid_work()
            .lock()
            .unwrap()
            .get(&reused_generation)
            .expect("replacement remains registered"),
        &replacement_work
    ));
    cancel_beginner_parameter_grid(reused_generation).expect("cancel replacement owner");
    assert!(replacement_work.cancelled.load(Ordering::Acquire));
    assert!(!original_work.cancelled.load(Ordering::Acquire));
}

#[test]
fn beginner_grid_registry_rejects_work_aliases_dirty_reuse_and_live_cap_overflow() {
    let _serial = serial_beginner_grid_test();
    let first_generation = ProjectId::new();
    let first_work = Arc::new(BeginnerGridWork::default());
    let first_registration =
        beginner_design_commands::register_beginner_grid_work_v1(first_generation, &first_work)
            .expect("register first grid owner");

    let alias_generation = ProjectId::new();
    assert_eq!(
        beginner_design_commands::register_beginner_grid_work_v1(alias_generation, &first_work)
            .err()
            .as_deref(),
        Some("grid_work_reused")
    );
    assert!(
        !beginner_grid_work()
            .lock()
            .unwrap()
            .contains_key(&alias_generation)
    );

    let dirty_generation = ProjectId::new();
    let dirty_work = Arc::new(BeginnerGridWork::default());
    dirty_work.enumerated.store(1, Ordering::Release);
    assert_eq!(
        beginner_design_commands::register_beginner_grid_work_v1(dirty_generation, &dirty_work)
            .err()
            .as_deref(),
        Some("grid_work_not_fresh")
    );
    assert!(!dirty_work.registration_active.load(Ordering::Acquire));

    let mut registrations =
        Vec::with_capacity(beginner_design_commands::MAX_BEGINNER_GRID_WORK_REGISTRATIONS_V1);
    registrations.push(first_registration);
    while registrations.len() < beginner_design_commands::MAX_BEGINNER_GRID_WORK_REGISTRATIONS_V1 {
        let generation = ProjectId::new();
        let work = Arc::new(BeginnerGridWork::default());
        registrations.push(
            beginner_design_commands::register_beginner_grid_work_v1(generation, &work)
                .expect("fill bounded grid registry"),
        );
    }

    let overflow_generation = ProjectId::new();
    let overflow_work = Arc::new(BeginnerGridWork::default());
    assert_eq!(
        beginner_design_commands::register_beginner_grid_work_v1(
            overflow_generation,
            &overflow_work,
        )
        .err()
        .as_deref(),
        Some("grid_registry_resource_limit")
    );
    assert!(
        !overflow_work.registration_active.load(Ordering::Acquire),
        "a rejected allocation must roll back its work claim"
    );

    drop(registrations.pop());
    assert_eq!(
        beginner_design_commands::register_beginner_grid_work_v1(
            overflow_generation,
            &overflow_work,
        )
        .err()
        .as_deref(),
        Some("grid_generation_reused"),
        "even a capacity-rejected cancellation handle remains single use"
    );
    let released_generation = ProjectId::new();
    let released_work = Arc::new(BeginnerGridWork::default());
    registrations.push(
        beginner_design_commands::register_beginner_grid_work_v1(
            released_generation,
            &released_work,
        )
        .expect("a released slot accepts a distinct generation"),
    );
}

#[test]
fn beginner_grid_registry_linearizes_concurrent_shared_work_claims() {
    let _serial = serial_beginner_grid_test();
    let generations = [ProjectId::new(), ProjectId::new()];
    let work = Arc::new(BeginnerGridWork::default());
    let start = Arc::new(std::sync::Barrier::new(3));
    let release = Arc::new(std::sync::Barrier::new(3));
    let (result_sender, result_receiver) = mpsc::channel();
    let mut workers = Vec::with_capacity(2);

    for generation in generations {
        let worker_work = Arc::clone(&work);
        let worker_start = Arc::clone(&start);
        let worker_release = Arc::clone(&release);
        let worker_sender = result_sender.clone();
        workers.push(thread::spawn(move || {
            worker_start.wait();
            let registration =
                beginner_design_commands::register_beginner_grid_work_v1(generation, &worker_work);
            let reported = match &registration {
                Ok(_) => Ok(()),
                Err(error) => Err(error.clone()),
            };
            worker_sender.send(reported).unwrap();
            worker_release.wait();
            drop(registration);
        }));
    }
    drop(result_sender);
    start.wait();

    let first_result = result_receiver.recv_timeout(Duration::from_secs(1));
    let second_result = result_receiver.recv_timeout(Duration::from_secs(1));
    let live_registry_len = beginner_grid_work().lock().unwrap().len();
    let active_while_held = work.registration_active.load(Ordering::Acquire);

    release.wait();
    for worker in workers {
        worker.join().expect("join concurrent grid claimant");
    }

    let results = [
        first_result.expect("first concurrent grid claim reports"),
        second_result.expect("second concurrent grid claim reports"),
    ];
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| {
                result
                    .as_ref()
                    .err()
                    .is_some_and(|error| error == "grid_work_reused")
            })
            .count(),
        1
    );
    assert_eq!(live_registry_len, 1);
    assert!(active_while_held);
    assert_eq!(work.terminal.load(Ordering::Acquire), 3);
    assert!(!work.registration_active.load(Ordering::Acquire));
}

#[test]
fn beginner_grid_aba_replacement_prevents_stale_owner_publication() {
    let _serial = serial_beginner_grid_test();
    let generation = ProjectId::new();
    let original_work = Arc::new(BeginnerGridWork::default());
    let replacement_work = Arc::new(BeginnerGridWork::default());

    let result = beginner_design_commands::run_registered_beginner_grid_work_v1(
        generation,
        &original_work,
        || {
            beginner_grid_work()
                .lock()
                .unwrap()
                .insert(generation, Arc::clone(&replacement_work));
            Ok(17_u8)
        },
    );

    assert_eq!(result, Err("grid_evaluation_failed".to_owned()));
    assert_eq!(original_work.terminal.load(Ordering::Acquire), 3);
    assert!(!original_work.registration_active.load(Ordering::Acquire));
    assert_eq!(replacement_work.terminal.load(Ordering::Acquire), 0);
    assert!(Arc::ptr_eq(
        beginner_grid_work()
            .lock()
            .unwrap()
            .get(&generation)
            .expect("ABA replacement remains the registry target"),
        &replacement_work
    ));
    cancel_beginner_parameter_grid(generation)
        .expect("replacement remains independently cancellable");
    assert!(replacement_work.cancelled.load(Ordering::Acquire));
}

#[test]
fn reference_consensus_duplicate_preserves_exact_cancel_owner_and_foreign_target() {
    let _serial = serial_beginner_grid_test();
    let generation = ProjectId::new();
    let foreign_generation = ProjectId::new();
    let work = Arc::new(ReferenceConsensusWorkV1::default());
    let foreign_work = Arc::new(ReferenceConsensusWorkV1::default());
    let registration =
        beginner_design_commands::register_reference_consensus_work_v1(generation, &work)
            .expect("register original consensus work");
    let foreign_registration = beginner_design_commands::register_reference_consensus_work_v1(
        foreign_generation,
        &foreign_work,
    )
    .expect("register foreign consensus work");
    let duplicate = Arc::new(ReferenceConsensusWorkV1::default());

    assert_eq!(
        beginner_design_commands::register_reference_consensus_work_v1(generation, &duplicate)
            .err()
            .as_deref(),
        Some("reference_consensus_generation_reused")
    );
    assert!(Arc::ptr_eq(
        reference_consensus_work_v1()
            .lock()
            .unwrap()
            .get(&generation)
            .expect("original remains registered"),
        &work
    ));
    cancel_reference_consensus(generation).unwrap();
    cancel_reference_consensus(generation).unwrap();
    assert!(work.cancelled.load(Ordering::Acquire));
    assert!(!duplicate.cancelled.load(Ordering::Acquire));
    assert!(!foreign_work.cancelled.load(Ordering::Acquire));

    drop(registration);
    assert!(cancel_reference_consensus(generation).is_err());
    assert!(Arc::ptr_eq(
        reference_consensus_work_v1()
            .lock()
            .unwrap()
            .get(&foreign_generation)
            .expect("foreign registration remains"),
        &foreign_work
    ));
    drop(foreign_registration);
    assert!(cancel_reference_consensus(foreign_generation).is_err());
}

#[test]
fn reference_consensus_registration_cleans_early_return_unwind_and_stale_aba_drop() {
    let _serial = serial_beginner_grid_test();
    let early_generation = ProjectId::new();
    let early_work = Arc::new(ReferenceConsensusWorkV1::default());
    let early_result = (|| -> Result<(), String> {
        let _registration = beginner_design_commands::register_reference_consensus_work_v1(
            early_generation,
            &early_work,
        )?;
        Err("simulated_early_return".to_owned())
    })();
    assert_eq!(early_result, Err("simulated_early_return".to_owned()));
    assert!(cancel_reference_consensus(early_generation).is_err());

    let unwind_generation = ProjectId::new();
    let unwind_work = Arc::new(ReferenceConsensusWorkV1::default());
    let unwind = std::panic::catch_unwind(|| {
        let _registration = beginner_design_commands::register_reference_consensus_work_v1(
            unwind_generation,
            &unwind_work,
        )
        .expect("register unwind consensus work");
        panic!("simulate consensus worker unwind");
    });
    assert!(unwind.is_err());
    assert!(cancel_reference_consensus(unwind_generation).is_err());

    let reused_generation = ProjectId::new();
    let original_work = Arc::new(ReferenceConsensusWorkV1::default());
    let stale_registration = beginner_design_commands::register_reference_consensus_work_v1(
        reused_generation,
        &original_work,
    )
    .expect("register original ABA work");
    let replacement_work = Arc::new(ReferenceConsensusWorkV1::default());
    reference_consensus_work_v1()
        .lock()
        .unwrap()
        .insert(reused_generation, Arc::clone(&replacement_work));

    drop(stale_registration);
    assert_eq!(original_work.terminal.load(Ordering::Acquire), 3);
    assert!(!original_work.registration_active.load(Ordering::Acquire));
    assert!(Arc::ptr_eq(
        reference_consensus_work_v1()
            .lock()
            .unwrap()
            .get(&reused_generation)
            .expect("replacement remains registered"),
        &replacement_work
    ));
    cancel_reference_consensus(reused_generation).expect("cancel replacement owner");
    assert!(replacement_work.cancelled.load(Ordering::Acquire));
    assert!(!original_work.cancelled.load(Ordering::Acquire));
}

#[test]
fn reference_consensus_registry_rejects_work_aliases_dirty_reuse_and_live_cap_overflow() {
    let _serial = serial_beginner_grid_test();
    let first_generation = ProjectId::new();
    let first_work = Arc::new(ReferenceConsensusWorkV1::default());
    let first_registration = beginner_design_commands::register_reference_consensus_work_v1(
        first_generation,
        &first_work,
    )
    .expect("register first consensus owner");

    let alias_generation = ProjectId::new();
    assert_eq!(
        beginner_design_commands::register_reference_consensus_work_v1(
            alias_generation,
            &first_work,
        )
        .err()
        .as_deref(),
        Some("reference_consensus_work_reused")
    );
    assert!(
        !reference_consensus_work_v1()
            .lock()
            .unwrap()
            .contains_key(&alias_generation)
    );

    let dirty_generation = ProjectId::new();
    let dirty_work = Arc::new(ReferenceConsensusWorkV1::default());
    dirty_work.cancelled.store(true, Ordering::Release);
    assert_eq!(
        beginner_design_commands::register_reference_consensus_work_v1(
            dirty_generation,
            &dirty_work,
        )
        .err()
        .as_deref(),
        Some("reference_consensus_work_not_fresh")
    );
    assert!(!dirty_work.registration_active.load(Ordering::Acquire));

    let mut registrations =
        Vec::with_capacity(beginner_design_commands::MAX_REFERENCE_CONSENSUS_WORK_REGISTRATIONS_V1);
    registrations.push(first_registration);
    while registrations.len()
        < beginner_design_commands::MAX_REFERENCE_CONSENSUS_WORK_REGISTRATIONS_V1
    {
        let generation = ProjectId::new();
        let work = Arc::new(ReferenceConsensusWorkV1::default());
        registrations.push(
            beginner_design_commands::register_reference_consensus_work_v1(generation, &work)
                .expect("fill bounded consensus registry"),
        );
    }

    let overflow_generation = ProjectId::new();
    let overflow_work = Arc::new(ReferenceConsensusWorkV1::default());
    assert_eq!(
        beginner_design_commands::register_reference_consensus_work_v1(
            overflow_generation,
            &overflow_work,
        )
        .err()
        .as_deref(),
        Some("reference_consensus_registry_resource_limit")
    );
    assert!(
        !overflow_work.registration_active.load(Ordering::Acquire),
        "a rejected allocation must roll back its work claim"
    );

    drop(registrations.pop());
    assert_eq!(
        beginner_design_commands::register_reference_consensus_work_v1(
            overflow_generation,
            &overflow_work,
        )
        .err()
        .as_deref(),
        Some("reference_consensus_generation_reused"),
        "even a capacity-rejected consensus cancellation handle remains single use"
    );
    let released_generation = ProjectId::new();
    let released_work = Arc::new(ReferenceConsensusWorkV1::default());
    registrations.push(
        beginner_design_commands::register_reference_consensus_work_v1(
            released_generation,
            &released_work,
        )
        .expect("a released consensus slot accepts a distinct generation"),
    );
}

#[test]
fn reference_consensus_registry_linearizes_concurrent_shared_work_claims() {
    let _serial = serial_beginner_grid_test();
    let generations = [ProjectId::new(), ProjectId::new()];
    let work = Arc::new(ReferenceConsensusWorkV1::default());
    let start = Arc::new(std::sync::Barrier::new(3));
    let release = Arc::new(std::sync::Barrier::new(3));
    let (result_sender, result_receiver) = mpsc::channel();
    let mut workers = Vec::with_capacity(2);

    for generation in generations {
        let worker_work = Arc::clone(&work);
        let worker_start = Arc::clone(&start);
        let worker_release = Arc::clone(&release);
        let worker_sender = result_sender.clone();
        workers.push(thread::spawn(move || {
            worker_start.wait();
            let registration = beginner_design_commands::register_reference_consensus_work_v1(
                generation,
                &worker_work,
            );
            let reported = match &registration {
                Ok(_) => Ok(()),
                Err(error) => Err(error.clone()),
            };
            worker_sender.send(reported).unwrap();
            worker_release.wait();
            drop(registration);
        }));
    }
    drop(result_sender);
    start.wait();

    let first_result = result_receiver.recv_timeout(Duration::from_secs(1));
    let second_result = result_receiver.recv_timeout(Duration::from_secs(1));
    let live_registry_len = reference_consensus_work_v1().lock().unwrap().len();
    let active_while_held = work.registration_active.load(Ordering::Acquire);

    release.wait();
    for worker in workers {
        worker
            .join()
            .expect("join concurrent reference-consensus claimant");
    }

    let results = [
        first_result.expect("first concurrent consensus claim reports"),
        second_result.expect("second concurrent consensus claim reports"),
    ];
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| {
                result
                    .as_ref()
                    .err()
                    .is_some_and(|error| error == "reference_consensus_work_reused")
            })
            .count(),
        1
    );
    assert_eq!(live_registry_len, 1);
    assert!(active_while_held);
    assert_eq!(work.terminal.load(Ordering::Acquire), 3);
    assert!(!work.registration_active.load(Ordering::Acquire));
    assert!(reference_consensus_work_v1().lock().unwrap().is_empty());
}

#[test]
fn reference_consensus_aba_replacement_prevents_stale_owner_publication() {
    let _serial = serial_beginner_grid_test();
    let generation = ProjectId::new();
    let original_work = Arc::new(ReferenceConsensusWorkV1::default());
    let replacement_work = Arc::new(ReferenceConsensusWorkV1::default());

    let result = beginner_design_commands::run_registered_reference_consensus_work_v1(
        generation,
        &original_work,
        || {
            reference_consensus_work_v1()
                .lock()
                .unwrap()
                .insert(generation, Arc::clone(&replacement_work));
            Ok(19_u8)
        },
    );

    assert_eq!(result, Err("reference_consensus_failed".to_owned()));
    assert_eq!(original_work.terminal.load(Ordering::Acquire), 3);
    assert!(!original_work.registration_active.load(Ordering::Acquire));
    assert_eq!(replacement_work.terminal.load(Ordering::Acquire), 0);
    assert!(Arc::ptr_eq(
        reference_consensus_work_v1()
            .lock()
            .unwrap()
            .get(&generation)
            .expect("ABA replacement remains the consensus registry target"),
        &replacement_work
    ));
    cancel_reference_consensus(generation).expect("replacement remains independently cancellable");
    assert!(replacement_work.cancelled.load(Ordering::Acquire));
}

#[test]
fn reference_consensus_terminal_transition_makes_cancel_win_publication_races() {
    let _serial = serial_beginner_grid_test();

    let complete_generation = ProjectId::new();
    let complete_work = Arc::new(ReferenceConsensusWorkV1::default());
    let completed = beginner_design_commands::run_registered_reference_consensus_work_v1(
        complete_generation,
        &complete_work,
        || {
            cancel_reference_consensus(complete_generation)?;
            Ok(11_u8)
        },
    );
    assert_eq!(
        completed,
        Err("reference_consensus_cancelled".to_owned()),
        "a cancellation linearized before success must prevent publication"
    );

    let failure_generation = ProjectId::new();
    let failure_work = Arc::new(ReferenceConsensusWorkV1::default());
    let failed = beginner_design_commands::run_registered_reference_consensus_work_v1::<()>(
        failure_generation,
        &failure_work,
        || {
            cancel_reference_consensus(failure_generation)?;
            Err("simulated_consensus_failure".to_owned())
        },
    );
    assert_eq!(
        failed,
        Err("reference_consensus_cancelled".to_owned()),
        "the same terminal cancellation must dominate a concurrent failure"
    );
}

#[test]
fn reference_consensus_cancel_flag_precedes_terminal_and_finish_race_rolls_back_losers() {
    let _serial = serial_beginner_grid_test();

    let forced_generation = ProjectId::new();
    let forced_work = Arc::new(ReferenceConsensusWorkV1::default());
    let forced_registration = beginner_design_commands::register_reference_consensus_work_v1(
        forced_generation,
        &forced_work,
    )
    .expect("register forced consensus cancellation");
    let observed_work = Arc::clone(&forced_work);
    let observer = thread::spawn(move || {
        while observed_work.terminal.load(Ordering::Acquire) == 0 {
            std::hint::spin_loop();
        }
        (
            observed_work.terminal.load(Ordering::Acquire),
            observed_work.cancelled.load(Ordering::Acquire),
        )
    });
    cancel_reference_consensus(forced_generation).expect("publish forced consensus cancellation");
    assert_eq!(
        observer.join().expect("join consensus terminal observer"),
        (2, true),
        "an Acquire observer of terminal=2 must also observe the prior stop flag"
    );
    drop(forced_registration);

    for _ in 0..8 {
        let generation = ProjectId::new();
        let work = Arc::new(ReferenceConsensusWorkV1::default());
        let start = Arc::new(std::sync::Barrier::new(2));
        let worker_start = Arc::clone(&start);
        let worker_work = Arc::clone(&work);
        let (entered_sender, entered_receiver) = mpsc::channel();
        let worker = thread::spawn(move || {
            beginner_design_commands::run_registered_reference_consensus_work_v1(
                generation,
                &worker_work,
                || {
                    entered_sender.send(()).unwrap();
                    worker_start.wait();
                    Ok(41_u8)
                },
            )
        });
        entered_receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("consensus worker registered before finish/cancel race");

        let observer_work = Arc::clone(&work);
        let observer = thread::spawn(move || {
            let terminal = loop {
                let terminal = observer_work.terminal.load(Ordering::Acquire);
                if terminal != 0 {
                    break terminal;
                }
                std::hint::spin_loop();
            };
            (terminal, observer_work.cancelled.load(Ordering::Acquire))
        });
        let cancel_start = Arc::clone(&start);
        let canceller = thread::spawn(move || {
            cancel_start.wait();
            cancel_reference_consensus(generation)
        });

        let worker_result = worker.join().expect("join consensus publication racer");
        let cancel_result = canceller.join().expect("join consensus cancellation racer");
        let observed = observer.join().expect("join consensus race observer");
        let terminal = work.terminal.load(Ordering::Acquire);
        let cancelled = work.cancelled.load(Ordering::Acquire);
        match terminal {
            1 => {
                assert_eq!(worker_result, Ok(41));
                assert!(!cancelled);
                assert!(
                    cancel_result.is_ok()
                        || matches!(
                            &cancel_result,
                            Err(error) if error == "reference_consensus_generation_not_running"
                        )
                );
            }
            2 => {
                assert_eq!(
                    worker_result,
                    Err("reference_consensus_cancelled".to_owned())
                );
                assert_eq!(cancel_result, Ok(()));
                assert!(cancelled);
                assert_eq!(observed, (2, true));
            }
            other => panic!("unexpected consensus race terminal {other}"),
        }
    }
}

#[test]
fn reference_consensus_command_claims_generation_before_project_lock_wait() {
    let _serial = serial_beginner_grid_test();
    let generation = ProjectId::new();
    let work = Arc::new(ReferenceConsensusWorkV1::default());
    let project_gate = Arc::new(Mutex::new(()));
    let held_project = project_gate.lock().unwrap();
    let worker_gate = Arc::clone(&project_gate);
    let worker_work = Arc::clone(&work);
    let (entered_sender, entered_receiver) = mpsc::channel();

    let worker = thread::spawn(move || {
        beginner_design_commands::run_registered_reference_consensus_work_v1(
            generation,
            &worker_work,
            || {
                entered_sender.send(()).unwrap();
                let _project = worker_gate.lock().unwrap();
                if worker_work.cancelled.load(Ordering::Acquire) {
                    Err("reference_consensus_cancelled".to_owned())
                } else {
                    Ok(())
                }
            },
        )
    });

    entered_receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("generation is claimed before the project lock wait");
    assert!(Arc::ptr_eq(
        reference_consensus_work_v1()
            .lock()
            .unwrap()
            .get(&generation)
            .expect("waiting command owns the generation"),
        &work
    ));
    let duplicate = Arc::new(ReferenceConsensusWorkV1::default());
    assert_eq!(
        beginner_design_commands::register_reference_consensus_work_v1(generation, &duplicate)
            .err()
            .as_deref(),
        Some("reference_consensus_generation_reused")
    );
    cancel_reference_consensus(generation).expect("cancel waiting command");
    drop(held_project);
    assert_eq!(
        worker.join().expect("join waiting command"),
        Err("reference_consensus_cancelled".to_owned())
    );
    assert!(cancel_reference_consensus(generation).is_err());
}

#[test]
fn beginner_grid_command_claims_generation_before_project_lock_wait() {
    let _serial = serial_beginner_grid_test();
    let generation = ProjectId::new();
    let work = Arc::new(BeginnerGridWork::default());
    let project_gate = Arc::new(Mutex::new(()));
    let held_project = project_gate.lock().unwrap();
    let worker_gate = Arc::clone(&project_gate);
    let worker_work = Arc::clone(&work);
    let (entered_sender, entered_receiver) = mpsc::channel();

    let worker = thread::spawn(move || {
        beginner_design_commands::run_registered_beginner_grid_work_v1(
            generation,
            &worker_work,
            || {
                entered_sender.send(()).unwrap();
                let _project = worker_gate.lock().unwrap();
                Ok(())
            },
        )
    });

    entered_receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("generation is claimed before the project lock wait");
    assert!(work.registration_active.load(Ordering::Acquire));
    assert!(Arc::ptr_eq(
        beginner_grid_work()
            .lock()
            .unwrap()
            .get(&generation)
            .expect("waiting command owns the generation"),
        &work
    ));
    let duplicate = Arc::new(BeginnerGridWork::default());
    assert_eq!(
        beginner_design_commands::register_beginner_grid_work_v1(generation, &duplicate)
            .err()
            .as_deref(),
        Some("grid_generation_reused")
    );
    drop(held_project);
    assert_eq!(worker.join().expect("join waiting command"), Ok(()));
    assert_eq!(work.terminal.load(Ordering::Acquire), 1);
    assert!(!work.registration_active.load(Ordering::Acquire));
}

#[test]
fn beginner_grid_registration_retains_terminal_generation_id_until_bounded_eviction() {
    let _serial = serial_beginner_grid_test();
    let generation = ProjectId::new();
    let work = Arc::new(BeginnerGridWork::default());
    let registration = beginner_design_commands::register_beginner_grid_work_v1(generation, &work)
        .expect("register live terminal owner");
    assert!(work.registration_active.load(Ordering::Acquire));
    assert_eq!(
        beginner_design_commands::finish_beginner_grid_work_v1(generation, &work, Ok(7_u8)),
        Ok(7)
    );

    let duplicate = Arc::new(BeginnerGridWork::default());
    assert_eq!(
        beginner_design_commands::register_beginner_grid_work_v1(generation, &duplicate)
            .err()
            .as_deref(),
        Some("grid_generation_reused")
    );
    assert!(Arc::ptr_eq(
        beginner_grid_work()
            .lock()
            .unwrap()
            .get(&generation)
            .expect("live completed owner remains registered"),
        &work
    ));

    drop(registration);
    assert!(!work.registration_active.load(Ordering::Acquire));
    assert_eq!(
        get_beginner_parameter_grid_progress(generation)
            .expect("completed history remains queryable")
            .terminal_state,
        "completed"
    );
    let replacement = Arc::new(BeginnerGridWork::default());
    assert_eq!(
        beginner_design_commands::register_beginner_grid_work_v1(generation, &replacement)
            .err()
            .as_deref(),
        Some("grid_generation_reused"),
        "terminal history reserves the exact generation ID until bounded eviction"
    );
    assert!(!replacement.registration_active.load(Ordering::Acquire));
}

#[test]
fn beginner_grid_terminal_transitions_linearize_cancel_complete_failure_and_drop() {
    let _serial = serial_beginner_grid_test();

    let cancel_before_complete = ProjectId::new();
    let cancel_before_complete_work = Arc::new(BeginnerGridWork::default());
    let completed = beginner_design_commands::run_registered_beginner_grid_work_v1(
        cancel_before_complete,
        &cancel_before_complete_work,
        || {
            cancel_beginner_parameter_grid(cancel_before_complete)?;
            Ok(11_u8)
        },
    );
    assert_eq!(completed, Err("grid_evaluation_cancelled".to_owned()));
    assert_eq!(
        cancel_before_complete_work.terminal.load(Ordering::Acquire),
        2
    );

    let cancel_before_failure = ProjectId::new();
    let cancel_before_failure_work = Arc::new(BeginnerGridWork::default());
    let failed = beginner_design_commands::run_registered_beginner_grid_work_v1::<()>(
        cancel_before_failure,
        &cancel_before_failure_work,
        || {
            cancel_beginner_parameter_grid(cancel_before_failure)?;
            Err("simulated_grid_failure".to_owned())
        },
    );
    assert_eq!(failed, Err("grid_evaluation_cancelled".to_owned()));
    assert_eq!(
        cancel_before_failure_work.terminal.load(Ordering::Acquire),
        2
    );

    let cancel_before_drop = ProjectId::new();
    let cancel_before_drop_work = Arc::new(BeginnerGridWork::default());
    let cancel_before_drop_registration = beginner_design_commands::register_beginner_grid_work_v1(
        cancel_before_drop,
        &cancel_before_drop_work,
    )
    .expect("register cancellation drop work");
    cancel_beginner_parameter_grid(cancel_before_drop).expect("cancel before owner drop");
    drop(cancel_before_drop_registration);
    assert_eq!(cancel_before_drop_work.terminal.load(Ordering::Acquire), 2);

    let complete_before_cancel = ProjectId::new();
    let complete_before_cancel_work = Arc::new(BeginnerGridWork::default());
    assert_eq!(
        beginner_design_commands::run_registered_beginner_grid_work_v1(
            complete_before_cancel,
            &complete_before_cancel_work,
            || Ok(23_u8),
        ),
        Ok(23)
    );
    cancel_beginner_parameter_grid(complete_before_cancel)
        .expect("late cancellation revokes an unconsumed completed generation");
    assert_eq!(
        complete_before_cancel_work.terminal.load(Ordering::Acquire),
        2
    );
    assert!(
        complete_before_cancel_work
            .cancelled
            .load(Ordering::Acquire)
    );

    let failure_before_cancel = ProjectId::new();
    let failure_before_cancel_work = Arc::new(BeginnerGridWork::default());
    assert_eq!(
        beginner_design_commands::run_registered_beginner_grid_work_v1::<()>(
            failure_before_cancel,
            &failure_before_cancel_work,
            || Err("simulated_grid_failure".to_owned()),
        ),
        Err("simulated_grid_failure".to_owned())
    );
    assert_eq!(
        cancel_beginner_parameter_grid(failure_before_cancel),
        Err("grid_generation_not_running".to_owned())
    );
    assert_eq!(
        failure_before_cancel_work.terminal.load(Ordering::Acquire),
        3
    );
    assert!(!failure_before_cancel_work.cancelled.load(Ordering::Acquire));
}

#[test]
fn beginner_grid_cancel_flag_precedes_terminal_and_finish_race_rolls_back_losers() {
    let _serial = serial_beginner_grid_test();

    let forced_generation = ProjectId::new();
    let forced_work = Arc::new(BeginnerGridWork::default());
    let forced_registration =
        beginner_design_commands::register_beginner_grid_work_v1(forced_generation, &forced_work)
            .expect("register forced grid cancellation");
    let observed_work = Arc::clone(&forced_work);
    let observer = thread::spawn(move || {
        while observed_work.terminal.load(Ordering::Acquire) == 0 {
            std::hint::spin_loop();
        }
        (
            observed_work.terminal.load(Ordering::Acquire),
            observed_work.cancelled.load(Ordering::Acquire),
        )
    });
    cancel_beginner_parameter_grid(forced_generation).expect("publish forced grid cancellation");
    assert_eq!(
        observer.join().expect("join grid terminal observer"),
        (2, true),
        "an Acquire observer of terminal=2 must also observe the prior stop flag"
    );
    drop(forced_registration);

    for _ in 0..8 {
        let generation = ProjectId::new();
        let work = Arc::new(BeginnerGridWork::default());
        let start = Arc::new(std::sync::Barrier::new(2));
        let worker_start = Arc::clone(&start);
        let worker_work = Arc::clone(&work);
        let (entered_sender, entered_receiver) = mpsc::channel();
        let worker = thread::spawn(move || {
            beginner_design_commands::run_registered_beginner_grid_work_v1(
                generation,
                &worker_work,
                || {
                    entered_sender.send(()).unwrap();
                    worker_start.wait();
                    Ok(43_u8)
                },
            )
        });
        entered_receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("grid worker registered before finish/cancel race");

        let observer_work = Arc::clone(&work);
        let observer = thread::spawn(move || {
            let terminal = loop {
                let terminal = observer_work.terminal.load(Ordering::Acquire);
                if terminal != 0 {
                    break terminal;
                }
                std::hint::spin_loop();
            };
            (terminal, observer_work.cancelled.load(Ordering::Acquire))
        });
        let cancel_start = Arc::clone(&start);
        let canceller = thread::spawn(move || {
            cancel_start.wait();
            cancel_beginner_parameter_grid(generation)
        });

        let worker_result = worker.join().expect("join grid publication racer");
        let cancel_result = canceller.join().expect("join grid cancellation racer");
        let observed = observer.join().expect("join grid race observer");
        let terminal = work.terminal.load(Ordering::Acquire);
        let cancelled = work.cancelled.load(Ordering::Acquire);
        assert_eq!(cancel_result, Ok(()));
        assert_eq!(terminal, 2);
        assert!(cancelled);
        match worker_result {
            Ok(43) => {
                assert!(
                    observed == (1, false) || observed == (2, true),
                    "completion may publish immediately before the completed generation is revoked"
                );
            }
            Err(error) => {
                assert_eq!(error, "grid_evaluation_cancelled");
                assert_eq!(observed, (2, true));
            }
            other => panic!("unexpected grid race result {other:?}"),
        }
    }
}

#[test]
fn beginner_grid_registry_recovers_poison_across_registration_progress_cancel_and_publication() {
    let _serial = serial_beginner_grid_test();
    let registry = beginner_grid_work();
    let generation = ProjectId::new();
    let orphaned_work = Arc::new(BeginnerGridWork::default());
    registry
        .lock()
        .unwrap()
        .insert(generation, Arc::clone(&orphaned_work));
    poison_mutex_for_test(registry);

    let duplicate = Arc::new(BeginnerGridWork::default());
    assert_eq!(
        beginner_design_commands::register_beginner_grid_work_v1(generation, &duplicate)
            .err()
            .as_deref(),
        Some("grid_generation_reused"),
        "poison recovery must not silently reclaim an inactive running ID and create an ABA alias"
    );
    assert!(!registry.is_poisoned());
    assert!(Arc::ptr_eq(
        registry
            .lock()
            .unwrap()
            .get(&generation)
            .expect("orphaned ID remains fail-closed"),
        &orphaned_work
    ));

    let recovered_generation = ProjectId::new();
    let work = Arc::new(BeginnerGridWork::default());
    let registration =
        beginner_design_commands::register_beginner_grid_work_v1(recovered_generation, &work)
            .expect("registration recovers poison for a distinct generation");
    assert!(!registry.is_poisoned());
    assert!(Arc::ptr_eq(
        registry
            .lock()
            .unwrap()
            .get(&recovered_generation)
            .expect("recovered registration owns the generation"),
        &work
    ));
    assert!(!orphaned_work.registration_active.load(Ordering::Acquire));

    poison_mutex_for_test(registry);
    assert_eq!(
        get_beginner_parameter_grid_progress(recovered_generation)
            .expect("progress recovers registry poison")
            .terminal_state,
        "running"
    );
    assert!(!registry.is_poisoned());

    poison_mutex_for_test(registry);
    cancel_beginner_parameter_grid(recovered_generation).expect("cancel recovers registry poison");
    assert!(work.cancelled.load(Ordering::Acquire));
    assert!(!registry.is_poisoned());
    drop(registration);

    let publication_generation = ProjectId::new();
    let publication_work = Arc::new(BeginnerGridWork::default());
    assert_eq!(
        beginner_design_commands::run_registered_beginner_grid_work_v1(
            publication_generation,
            &publication_work,
            || {
                poison_mutex_for_test(registry);
                Ok(29_u8)
            },
        ),
        Ok(29),
        "terminal publication must recover a poison raised during owned work"
    );
    assert!(!registry.is_poisoned());
}

#[test]
fn reference_consensus_registry_recovers_poison_across_registration_cancel_and_publication() {
    let _serial = serial_beginner_grid_test();
    let registry = reference_consensus_work_v1();
    let generation = ProjectId::new();
    let orphaned_work = Arc::new(ReferenceConsensusWorkV1::default());
    registry
        .lock()
        .unwrap()
        .insert(generation, Arc::clone(&orphaned_work));
    poison_mutex_for_test(registry);

    let work = Arc::new(ReferenceConsensusWorkV1::default());
    let registration =
        beginner_design_commands::register_reference_consensus_work_v1(generation, &work)
            .expect("registration recovers poison and reclaims an inactive orphan");
    assert!(!registry.is_poisoned());
    assert!(Arc::ptr_eq(
        registry
            .lock()
            .unwrap()
            .get(&generation)
            .expect("recovered consensus registration owns the generation"),
        &work
    ));
    assert!(!orphaned_work.registration_active.load(Ordering::Acquire));

    poison_mutex_for_test(registry);
    cancel_reference_consensus(generation).expect("cancel recovers registry poison");
    assert!(work.cancelled.load(Ordering::Acquire));
    assert!(!registry.is_poisoned());
    drop(registration);
    assert!(cancel_reference_consensus(generation).is_err());

    let publication_generation = ProjectId::new();
    let publication_work = Arc::new(ReferenceConsensusWorkV1::default());
    assert_eq!(
        beginner_design_commands::run_registered_reference_consensus_work_v1(
            publication_generation,
            &publication_work,
            || {
                poison_mutex_for_test(registry);
                Ok(31_u8)
            },
        ),
        Ok(31),
        "consensus publication must recover a poison raised during owned work"
    );
    assert!(!registry.is_poisoned());
}
