#[test]
fn progressive_schedule_and_resource_bounds_are_fixed() {
    assert_eq!(POST_APPLY_PROOF_SAMPLE_INTERVALS_V1, [16, 32, 64]);
    assert_eq!(
        POST_APPLY_PROOF_SAMPLE_INTERVALS_V1.iter().sum::<usize>()
            * POST_APPLY_PROOF_MAX_DIAGNOSTIC_PASSES_PER_STAGE_V1,
        POST_APPLY_PROOF_TOTAL_WORK_V1
    );
    const {
        assert!(MAX_POST_APPLY_PROOF_JOBS_V1 > 0);
        assert!(MAX_POST_APPLY_PROOF_JOB_BYTES_V1 <= MAX_POST_APPLY_PROOF_RETAINED_BYTES_V1);
        assert!(MAX_POST_APPLY_DEADLINE_REGISTRATIONS_V1 >= MAX_POST_APPLY_PROOF_JOBS_V1);
        assert!(POST_APPLY_DEADLINE_SCHEDULER_QUEUE_V1 >= MAX_POST_APPLY_DEADLINE_REGISTRATIONS_V1);
    }
    assert!(POST_APPLY_PROOF_DEADLINE_V1 < POST_APPLY_PROOF_START_RETENTION_V1);
}

#[test]
fn retained_premise_binding_rejects_each_cross_boundary_drift_v1() {
    let _generation_guard =
        crate::stacked_fold_read::tests::lock_stacked_fold_read_generation_test();
    let (_app_state, transaction_state, _request, _) = prepare_started_actual_job_v1();
    let mut registry = transaction_state.3.lock().expect("post-Apply registry");
    let premise = registry
        .jobs
        .front_mut()
        .and_then(|job| job.premise.as_mut())
        .expect("retained production premise");
    assert!(premise_is_internally_bound_v1(premise));

    let original_binding = premise.binding.clone();
    let rebuild_binding = |source_revision,
                           source_fingerprint: String,
                           pose_generation,
                           paper_thickness_mm,
                           observation| {
        SpeculativeUnprovenFoldBindingV1::new(
            original_binding.project_instance_id(),
            original_binding.project_id(),
            source_revision,
            source_fingerprint,
            pose_generation,
            original_binding.request_generation_id(),
            paper_thickness_mm,
            observation,
        )
        .expect("individually valid drifted binding")
    };
    let original_thickness = f64::from_bits(original_binding.paper_thickness_bits());
    let no_blocking =
        ori_core::SpeculativeApproximateBlockingObservationV1::no_blocking_sample_observed();

    premise.binding = rebuild_binding(
        original_binding
            .source_revision()
            .checked_add(1)
            .expect("bounded source revision"),
        original_binding
            .source_geometry_fingerprint_sha256()
            .to_owned(),
        original_binding.pose_generation(),
        original_thickness,
        no_blocking,
    );
    assert!(
        !premise_is_internally_bound_v1(premise),
        "source-lineage revision drift must fail closed"
    );
    premise.binding = original_binding.clone();

    let original_target_revision = premise.target_revision;
    premise.target_revision = premise
        .target_revision
        .checked_add(1)
        .expect("bounded target revision");
    assert!(
        !premise_is_internally_bound_v1(premise),
        "target-lineage revision drift must fail closed"
    );
    premise.target_revision = original_target_revision;

    let mut source_fingerprint = original_binding
        .source_geometry_fingerprint_sha256()
        .to_owned();
    let first_replacement = if source_fingerprint.as_bytes()[0] == b'0' {
        "1"
    } else {
        "0"
    };
    source_fingerprint.replace_range(0..1, first_replacement);
    premise.binding = rebuild_binding(
        original_binding.source_revision(),
        source_fingerprint,
        original_binding.pose_generation(),
        original_thickness,
        no_blocking,
    );
    assert!(
        !premise_is_internally_bound_v1(premise),
        "source fingerprint drift must fail closed"
    );
    premise.binding = original_binding.clone();

    let original_target_fingerprint = premise.target_fingerprint;
    premise.target_fingerprint[0] ^= 1;
    assert!(
        !premise_is_internally_bound_v1(premise),
        "target fingerprint drift must fail closed"
    );
    premise.target_fingerprint = original_target_fingerprint;

    let original_target_generation = premise.target_pose_generation;
    premise.target_pose_generation = premise
        .target_pose_generation
        .checked_add(1)
        .expect("bounded target pose generation");
    assert!(
        !premise_is_internally_bound_v1(premise),
        "target pose-generation drift must fail closed"
    );
    premise.target_pose_generation = original_target_generation;

    let original_paper_thickness = premise.paper_thickness_mm;
    premise.paper_thickness_mm = f64::from_bits(original_paper_thickness.to_bits() ^ 1);
    assert!(
        !premise_is_internally_bound_v1(premise),
        "paper-thickness drift must fail closed"
    );
    premise.paper_thickness_mm = original_paper_thickness;

    premise.binding = rebuild_binding(
        original_binding.source_revision(),
        original_binding
            .source_geometry_fingerprint_sha256()
            .to_owned(),
        original_binding.pose_generation(),
        original_thickness,
        ori_core::SpeculativeApproximateBlockingObservationV1::blocking_sample_observed(1.0)
            .expect("valid blocking observation"),
    );
    assert!(
        !premise_is_internally_bound_v1(premise),
        "a blocking preview observation cannot authorize the retained proof"
    );
    premise.binding = original_binding;
    assert!(
        premise_is_internally_bound_v1(premise),
        "restoring every exact binding field restores the valid premise"
    );
}

#[test]
fn retained_premise_byte_charge_matches_publication_and_rejects_overflow_v1() {
    let _generation_guard =
        crate::stacked_fold_read::tests::lock_stacked_fold_read_generation_test();
    let (_app_state, transaction_state, _request, _) = prepare_started_actual_job_v1();
    let registry = transaction_state.3.lock().expect("post-Apply registry");
    let job = registry.jobs.front().expect("published production job");
    let premise = job.premise.as_ref().expect("retained production premise");
    let charged = retained_premise_bytes_v1(premise).expect("bounded retained-byte charge");
    assert!(charged > 0);
    assert_eq!(charged, job.retained_bytes);
    assert!(charged <= MAX_POST_APPLY_PROOF_JOB_BYTES_V1);
    assert!(
        retained_premise_byte_overflow_is_rejected_for_test_v1(),
        "every retained-byte multiplication and addition must fail closed on overflow"
    );
}

#[test]
fn unconsumed_deadline_override_is_cleared_on_early_return_and_unwind() {
    let original_deadline = Duration::from_secs(3);
    let original_guard = set_next_post_apply_proof_deadline_v1(original_deadline);
    let duplicate = std::panic::catch_unwind(|| {
        let _duplicate_guard = set_next_post_apply_proof_deadline_v1(Duration::from_secs(5));
    });
    assert!(duplicate.is_err());
    assert_eq!(
        next_post_apply_proof_deadline_v1(),
        original_deadline,
        "a rejected duplicate override cannot replace the original deadline"
    );
    let replacement_deadline = original_deadline;
    let replacement_guard = set_next_post_apply_proof_deadline_v1(replacement_deadline);
    drop(original_guard);
    assert_eq!(
        next_post_apply_proof_deadline_v1(),
        replacement_deadline,
        "a consumed deadline guard cannot clear an equal later slot with a new token"
    );
    drop(replacement_guard);

    let early_return: Result<(), ()> = {
        let _deadline_override_guard =
            set_next_post_apply_proof_deadline_v1(Duration::from_secs(11));
        Err(())
    };
    assert_eq!(early_return, Err(()));
    assert_eq!(
        next_post_apply_proof_deadline_v1(),
        POST_APPLY_PROOF_DEADLINE_V1
    );

    let unwound = std::panic::catch_unwind(|| {
        let _deadline_override_guard =
            set_next_post_apply_proof_deadline_v1(Duration::from_secs(13));
        panic!("inject deadline override setup unwind");
    });
    assert!(unwound.is_err());
    assert_eq!(
        next_post_apply_proof_deadline_v1(),
        POST_APPLY_PROOF_DEADLINE_V1
    );

    let consumed = Duration::from_secs(17);
    let deadline_override_guard = set_next_post_apply_proof_deadline_v1(consumed);
    assert_eq!(next_post_apply_proof_deadline_v1(), consumed);
    drop(deadline_override_guard);
    assert_eq!(
        next_post_apply_proof_deadline_v1(),
        POST_APPLY_PROOF_DEADLINE_V1,
        "dropping a consumed guard cannot clear or synthesize another override"
    );
}

#[test]
fn unconsumed_publication_failure_is_cleared_on_early_return_and_unwind() {
    let original_guard = fail_next_post_apply_proof_publication_v1();
    let duplicate = std::panic::catch_unwind(|| {
        let _duplicate_guard = fail_next_post_apply_proof_publication_v1();
    });
    assert!(duplicate.is_err());
    assert!(
        take_post_apply_proof_publication_failure_for_test_v1(),
        "a rejected duplicate arm cannot replace the original publication fault"
    );
    let replacement_guard = fail_next_post_apply_proof_publication_v1();
    drop(original_guard);
    assert!(
        take_post_apply_proof_publication_failure_for_test_v1(),
        "a consumed publication guard cannot clear an equal later arm with a new token"
    );
    drop(replacement_guard);

    let early_return: Result<(), ()> = {
        let _publication_failure_guard = fail_next_post_apply_proof_publication_v1();
        Err(())
    };
    assert_eq!(early_return, Err(()));
    assert!(!take_post_apply_proof_publication_failure_for_test_v1());

    let unwound = std::panic::catch_unwind(|| {
        let _publication_failure_guard = fail_next_post_apply_proof_publication_v1();
        panic!("inject publication failure setup unwind");
    });
    assert!(unwound.is_err());
    assert!(!take_post_apply_proof_publication_failure_for_test_v1());

    let publication_failure_guard = fail_next_post_apply_proof_publication_v1();
    assert!(take_post_apply_proof_publication_failure_for_test_v1());
    drop(publication_failure_guard);
    assert!(
        !take_post_apply_proof_publication_failure_for_test_v1(),
        "dropping a consumed guard cannot clear or synthesize another fault"
    );
}

#[test]
fn publication_resolution_fault_guard_preserves_old_arm_and_clears_every_exit_path() {
    let original_guard = super::super::fail_next_post_apply_publication_resolution_for_test_v1();
    let duplicate = std::panic::catch_unwind(|| {
        let _duplicate_guard =
            super::super::panic_next_post_apply_publication_resolution_before_for_test_v1();
    });
    assert!(duplicate.is_err());
    assert_eq!(
        super::super::take_post_apply_publication_resolution_fault_v1(),
        1,
        "a rejected duplicate arm cannot replace the original fault"
    );
    let replacement_guard = super::super::fail_next_post_apply_publication_resolution_for_test_v1();
    drop(original_guard);
    assert_eq!(
        super::super::take_post_apply_publication_resolution_fault_v1(),
        1,
        "a consumed publication-resolution guard cannot clear a later arm with a new token"
    );
    drop(replacement_guard);

    let early_return: Result<(), ()> = {
        let _resolution_fault_guard =
            super::super::panic_next_post_apply_publication_resolution_before_for_test_v1();
        Err(())
    };
    assert_eq!(early_return, Err(()));
    assert_eq!(
        super::super::take_post_apply_publication_resolution_fault_v1(),
        0
    );

    let unwound = std::panic::catch_unwind(|| {
        let _resolution_fault_guard =
            super::super::panic_next_post_apply_publication_resolution_after_for_test_v1();
        panic!("inject publication resolution fault setup unwind");
    });
    assert!(unwound.is_err());
    assert_eq!(
        super::super::take_post_apply_publication_resolution_fault_v1(),
        0
    );

    let consumed_guard =
        super::super::panic_next_post_apply_publication_resolution_after_for_test_v1();
    assert_eq!(
        super::super::take_post_apply_publication_resolution_fault_v1(),
        3
    );
    drop(consumed_guard);
    assert_eq!(
        super::super::take_post_apply_publication_resolution_fault_v1(),
        0,
        "dropping a consumed guard cannot clear or synthesize another fault"
    );
}

#[test]
fn deadline_resolution_panic_guard_clears_unreached_work_and_isolates_other_targets() {
    let _generation_guard =
        crate::stacked_fold_read::tests::lock_stacked_fold_read_generation_test();
    let duplicate_target = ProjectId::new();
    let duplicate_foreign = ProjectId::new();
    let original_guard = panic_next_deadline_resolution_and_recovery_v1(duplicate_target);
    let duplicate = std::panic::catch_unwind(|| {
        let _duplicate_guard = panic_next_deadline_resolution_and_recovery_v1(duplicate_foreign);
    });
    assert!(duplicate.is_err());
    assert!(
        deadline_resolution_panic_targets_job_for_test_v1(duplicate_target),
        "a duplicate-arm panic cannot replace the original deadline target"
    );
    assert!(
        !take_deadline_resolution_panic_for_job_for_test_v1(duplicate_foreign),
        "poison recovery cannot consume a foreign duplicate target"
    );
    assert!(take_deadline_resolution_panic_for_job_for_test_v1(
        duplicate_target
    ));
    drop(original_guard);

    let unreached_job = ProjectId::new();
    let scheduler_not_reached: Result<(), ()> = {
        let _deadline_resolution_guard =
            panic_next_deadline_resolution_and_recovery_v1(unreached_job);
        Err(())
    };
    assert_eq!(scheduler_not_reached, Err(()));
    assert!(!deadline_resolution_panic_targets_job_for_test_v1(
        unreached_job
    ));

    let panic_job = ProjectId::new();
    let unwound = std::panic::catch_unwind(|| {
        let _deadline_resolution_guard = panic_next_deadline_resolution_and_recovery_v1(panic_job);
        panic!("inject deadline resolution setup unwind");
    });
    assert!(unwound.is_err());
    assert!(!deadline_resolution_panic_targets_job_for_test_v1(
        panic_job
    ));

    let target_job = ProjectId::new();
    let foreign_job = ProjectId::new();
    let consumed_guard = panic_next_deadline_resolution_and_recovery_v1(target_job);
    assert!(deadline_resolution_panic_targets_job_for_test_v1(
        target_job
    ));
    assert!(!deadline_resolution_panic_targets_job_for_test_v1(
        foreign_job
    ));
    assert!(
        !take_deadline_resolution_panic_for_job_for_test_v1(foreign_job),
        "a foreign recovery cannot consume the armed target"
    );
    assert!(deadline_resolution_panic_targets_job_for_test_v1(
        target_job
    ));
    assert!(take_deadline_resolution_panic_for_job_for_test_v1(
        target_job
    ));
    assert!(!deadline_resolution_panic_targets_job_for_test_v1(
        target_job
    ));

    let later_target = target_job;
    let later_guard = panic_next_deadline_resolution_and_recovery_v1(later_target);
    drop(consumed_guard);
    assert!(
        deadline_resolution_panic_targets_job_for_test_v1(later_target),
        "a stale consumed guard cannot clear a later target with a new token"
    );
    assert!(take_deadline_resolution_panic_for_job_for_test_v1(
        later_target
    ));
    drop(later_guard);
}

#[test]
fn start_fail_closed_resolution_fault_guard_preserves_old_arm_and_clears_every_exit_path() {
    let original_guard = fail_next_post_apply_start_fail_closed_resolution_v1();
    let duplicate = std::panic::catch_unwind(|| {
        let _duplicate_guard = fail_next_post_apply_start_fail_closed_resolution_v1();
    });
    assert!(duplicate.is_err());
    assert!(
        take_post_apply_start_fail_closed_resolution_failure_for_test_v1(),
        "a rejected duplicate arm cannot replace the original fault"
    );
    let replacement_guard = fail_next_post_apply_start_fail_closed_resolution_v1();
    drop(original_guard);
    assert!(
        take_post_apply_start_fail_closed_resolution_failure_for_test_v1(),
        "a consumed start-resolution guard cannot clear a later arm with a new token"
    );
    drop(replacement_guard);

    let early_return: Result<(), ()> = {
        let _resolution_failure_guard = fail_next_post_apply_start_fail_closed_resolution_v1();
        Err(())
    };
    assert_eq!(early_return, Err(()));
    assert!(!take_post_apply_start_fail_closed_resolution_failure_for_test_v1());

    let unwound = std::panic::catch_unwind(|| {
        let _resolution_failure_guard = fail_next_post_apply_start_fail_closed_resolution_v1();
        panic!("inject start fail-closed resolution setup unwind");
    });
    assert!(unwound.is_err());
    assert!(!take_post_apply_start_fail_closed_resolution_failure_for_test_v1());

    let consumed_guard = fail_next_post_apply_start_fail_closed_resolution_v1();
    assert!(take_post_apply_start_fail_closed_resolution_failure_for_test_v1());
    drop(consumed_guard);
    assert!(
        !take_post_apply_start_fail_closed_resolution_failure_for_test_v1(),
        "dropping a consumed guard cannot clear or synthesize another fault"
    );
}

#[test]
fn process_global_one_shot_fault_guards_are_targeted_and_aba_safe() {
    let _generation_guard =
        crate::stacked_fold_read::tests::lock_stacked_fold_read_generation_test();

    let target_job = ProjectId::new();
    let foreign_job = ProjectId::new();
    let original_guard = fail_next_post_apply_generic_resolution_v1(&target_job);
    let duplicate = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _duplicate_guard = fail_next_post_apply_generic_resolution_v1(&foreign_job);
    }));
    assert!(duplicate.is_err());
    assert!(
        !take_post_apply_generic_resolution_failure_for_test_v1(&foreign_job),
        "a rejected duplicate arm cannot replace the original target"
    );
    assert!(
        take_post_apply_generic_resolution_failure_for_test_v1(&target_job),
        "the original target remains consumable after a duplicate arm"
    );

    let replacement_guard = fail_next_post_apply_generic_resolution_v1(&target_job);
    drop(original_guard);
    assert!(
        take_post_apply_generic_resolution_failure_for_test_v1(&target_job),
        "a consumed guard cannot clear an equal later payload with a new token"
    );
    drop(replacement_guard);

    let early_job = ProjectId::new();
    let early_return: Result<(), ()> = {
        let _guard = fail_next_post_apply_generic_resolution_v1(&early_job);
        Err(())
    };
    assert_eq!(early_return, Err(()));
    assert!(!take_post_apply_generic_resolution_failure_for_test_v1(
        &early_job
    ));

    let panic_job = ProjectId::new();
    let unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _guard = fail_next_post_apply_generic_resolution_v1(&panic_job);
        panic!("inject process-global one-shot setup unwind");
    }));
    assert!(unwound.is_err());
    assert!(!take_post_apply_generic_resolution_failure_for_test_v1(
        &panic_job
    ));

    let binder_guard = inject_next_post_apply_binder_fault_v1(
        &target_job,
        InjectedPostApplyBinderFaultV1::Allocation,
    );
    assert!(
        take_injected_post_apply_binder_fault_v1(&foreign_job).is_none(),
        "a foreign worker cannot consume the binder fault"
    );
    assert!(matches!(
        take_injected_post_apply_binder_fault_v1(&target_job),
        Some(InjectedPostApplyBinderFaultV1::Allocation)
    ));
    drop(binder_guard);

    let worker_guard = panic_next_post_apply_worker_v1(&target_job);
    inject_post_apply_worker_panic_for_test_v1(&foreign_job);
    let worker_unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        inject_post_apply_worker_panic_for_test_v1(&target_job);
    }));
    assert!(
        worker_unwound.is_err(),
        "only the intended worker consumes the panic fault"
    );
    drop(worker_guard);

    let target_registry = Arc::new(Mutex::new(PostApplyProofRegistryV1::default()));
    let foreign_registry = Arc::new(Mutex::new(PostApplyProofRegistryV1::default()));
    let registration_guard = fail_next_post_apply_deadline_registration_v1(&target_registry);
    assert!(
        !take_post_apply_deadline_registration_failure_for_test_v1(&foreign_registry),
        "a foreign registry cannot consume the registration fault"
    );
    assert!(take_post_apply_deadline_registration_failure_for_test_v1(
        &target_registry
    ));
    drop(registration_guard);

    let certified_guard = panic_next_post_apply_certified_resolution_after_v1(&target_job);
    assert_eq!(
        take_post_apply_certified_resolution_panic_v1(&foreign_job),
        0,
        "a foreign resolver cannot consume the certified-resolution fault"
    );
    assert_eq!(
        take_post_apply_certified_resolution_panic_v1(&target_job),
        2
    );
    drop(certified_guard);
}

#[test]
fn deadline_scheduler_fault_guards_preserve_targets_counts_and_tokens() {
    let _generation_guard =
        crate::stacked_fold_read::tests::lock_stacked_fold_read_generation_test();
    let target_registry = Arc::new(Mutex::new(PostApplyProofRegistryV1::default()));
    let foreign_registry = Arc::new(Mutex::new(PostApplyProofRegistryV1::default()));

    let original_panic_guard =
        arm_deadline_scheduler_iteration_panic_for_registry_v1(&target_registry);
    let duplicate = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _duplicate_guard =
            arm_deadline_scheduler_iteration_panic_for_registry_v1(&foreign_registry);
    }));
    assert!(duplicate.is_err());
    assert!(deadline_scheduler_panic_targets_registry_for_test_v1(
        &target_registry
    ));
    assert!(
        !take_deadline_scheduler_panic_for_registry_for_test_v1(&foreign_registry),
        "a foreign registry cannot consume or replace the scheduler panic target"
    );
    assert!(take_deadline_scheduler_panic_for_registry_for_test_v1(
        &target_registry
    ));
    let replacement_panic_guard =
        arm_deadline_scheduler_iteration_panic_for_registry_v1(&target_registry);
    drop(original_panic_guard);
    assert!(
        take_deadline_scheduler_panic_for_registry_for_test_v1(&target_registry),
        "an old consumed guard cannot clear an equal later scheduler target"
    );
    drop(replacement_panic_guard);

    let scheduler_early_return: Result<(), ()> = {
        let _guard = arm_deadline_scheduler_iteration_panic_for_registry_v1(&target_registry);
        Err(())
    };
    assert_eq!(scheduler_early_return, Err(()));
    assert!(!deadline_scheduler_panic_targets_registry_for_test_v1(
        &target_registry
    ));
    let scheduler_unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _guard = arm_deadline_scheduler_iteration_panic_for_registry_v1(&target_registry);
        panic!("inject scheduler panic-arm setup unwind");
    }));
    assert!(scheduler_unwound.is_err());
    assert!(!deadline_scheduler_panic_targets_registry_for_test_v1(
        &target_registry
    ));

    let original_resource_guard =
        force_next_post_apply_deadline_resource_failures_v1(&target_registry, 2);
    let duplicate = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _duplicate_guard =
            force_next_post_apply_deadline_resource_failures_v1(&foreign_registry, 7);
    }));
    assert!(duplicate.is_err());
    assert_eq!(
        forced_post_apply_deadline_resource_failures_remaining_for_test_v1(&target_registry),
        2,
        "a rejected duplicate arm preserves the original count and target"
    );
    assert_eq!(
        forced_post_apply_deadline_resource_failures_remaining_for_test_v1(&foreign_registry),
        0
    );
    assert!(
        !force_post_apply_deadline_resource_failure_for_test_v1(&foreign_registry),
        "a foreign registry cannot consume a forced resource failure"
    );
    assert!(force_post_apply_deadline_resource_failure_for_test_v1(
        &target_registry
    ));
    assert_eq!(
        forced_post_apply_deadline_resource_failures_remaining_for_test_v1(&target_registry),
        1
    );
    assert!(force_post_apply_deadline_resource_failure_for_test_v1(
        &target_registry
    ));

    let replacement_resource_guard =
        force_next_post_apply_deadline_resource_failures_v1(&target_registry, 2);
    drop(original_resource_guard);
    assert_eq!(
        forced_post_apply_deadline_resource_failures_remaining_for_test_v1(&target_registry),
        2,
        "an old consumed guard cannot clear an equal later count with a new token"
    );
    assert!(force_post_apply_deadline_resource_failure_for_test_v1(
        &target_registry
    ));
    drop(replacement_resource_guard);
    assert_eq!(
        forced_post_apply_deadline_resource_failures_remaining_for_test_v1(&target_registry),
        0,
        "dropping a current partially consumed guard clears its own remainder"
    );

    let resource_early_return: Result<(), ()> = {
        let _guard = force_next_post_apply_deadline_resource_failures_v1(&target_registry, 3);
        Err(())
    };
    assert_eq!(resource_early_return, Err(()));
    assert_eq!(
        forced_post_apply_deadline_resource_failures_remaining_for_test_v1(&target_registry),
        0
    );
    let resource_unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _guard = force_next_post_apply_deadline_resource_failures_v1(&target_registry, 3);
        panic!("inject resource-failure setup unwind");
    }));
    assert!(resource_unwound.is_err());
    assert_eq!(
        forced_post_apply_deadline_resource_failures_remaining_for_test_v1(&target_registry),
        0
    );
}
