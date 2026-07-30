fn register_deadline_scheduler_v1(
    project: Arc<Mutex<ProjectState>>,
    registry: Arc<Mutex<PostApplyProofRegistryV1>>,
) -> Result<(), ()> {
    #[cfg(test)]
    if take_post_apply_deadline_registration_failure_for_test_v1(&registry) {
        return Err(());
    }
    let sender = deadline_scheduler_sender_v1()?;
    let lease = DeadlineSchedulerRegistrationLeaseV1::try_acquire_v1()?;
    let command = DeadlineSchedulerCommandV1::Register(DeadlineSchedulerRegistrationV1 {
        project: Arc::downgrade(&project),
        registry: Arc::downgrade(&registry),
        resource_retry: None,
        _lease: lease,
    });
    match sender.try_send(command) {
        Ok(()) => Ok(()),
        Err(TrySendError::Full(_)) | Err(TrySendError::Disconnected(_)) => Err(()),
    }
}

#[cfg(test)]
fn take_post_apply_deadline_registration_failure_for_test_v1(
    registry: &Arc<Mutex<PostApplyProofRegistryV1>>,
) -> bool {
    let registry = Arc::downgrade(registry);
    take_process_global_one_shot_fault_if_v1(
        &FAIL_NEXT_POST_APPLY_DEADLINE_REGISTRATION_V1,
        |target| Weak::ptr_eq(target, &registry),
    )
    .is_some()
}

fn deadline_scheduler_sender_v1() -> Result<&'static SyncSender<DeadlineSchedulerCommandV1>, ()> {
    POST_APPLY_DEADLINE_SCHEDULER_V1
        .get_or_init(|| {
            match catch_unwind(AssertUnwindSafe(|| {
                let mut registrations = Vec::new();
                registrations
                    .try_reserve_exact(MAX_POST_APPLY_DEADLINE_REGISTRATIONS_V1)
                    .map_err(|_| ())?;
                let (sender, receiver) = sync_channel(POST_APPLY_DEADLINE_SCHEDULER_QUEUE_V1);
                tauri::async_runtime::spawn_blocking(move || {
                    deadline_scheduler_loop_v1(receiver, registrations)
                });
                Ok(sender)
            })) {
                Ok(result) => result,
                Err(_) => Err(()),
            }
        })
        .as_ref()
        .map_err(|_| ())
}

fn wake_deadline_scheduler_v1() {
    let Some(Ok(sender)) = POST_APPLY_DEADLINE_SCHEDULER_V1.get() else {
        return;
    };
    let _ = sender.try_send(DeadlineSchedulerCommandV1::Wake);
}

fn deadline_scheduler_loop_v1(
    receiver: Receiver<DeadlineSchedulerCommandV1>,
    mut registrations: Vec<DeadlineSchedulerRegistrationV1>,
) {
    loop {
        match catch_unwind(AssertUnwindSafe(|| {
            deadline_scheduler_iteration_v1(&receiver, &mut registrations)
        })) {
            Ok(true) => {}
            Ok(false) => {
                fail_all_deadline_registrations_resource_v1(&mut registrations);
                drain_disconnected_deadline_registrations_v1(&mut registrations);
                return;
            }
            Err(_) => {
                fail_all_deadline_registrations_resource_v1(&mut registrations);
            }
        }
    }
}

fn drain_disconnected_deadline_registrations_v1(
    registrations: &mut Vec<DeadlineSchedulerRegistrationV1>,
) {
    while !registrations.is_empty() {
        let drained = catch_unwind(AssertUnwindSafe(|| {
            let now = Instant::now();
            retry_failed_deadline_registrations_v1(registrations, now);
            expire_due_deadline_registrations_v1(registrations, now);
            prune_and_next_deadline_v1(registrations)
        }));
        match drained {
            Ok(Some(next)) => {
                std::thread::sleep(
                    next.saturating_duration_since(Instant::now())
                        .min(Duration::from_millis(128)),
                );
            }
            Ok(None) => return,
            Err(_) => fail_all_deadline_registrations_resource_v1(registrations),
        }
    }
}

fn deadline_scheduler_iteration_v1(
    receiver: &Receiver<DeadlineSchedulerCommandV1>,
    registrations: &mut Vec<DeadlineSchedulerRegistrationV1>,
) -> bool {
    inject_deadline_scheduler_iteration_panic_for_test_v1(registrations);
    let now = Instant::now();
    retry_failed_deadline_registrations_v1(registrations, now);
    expire_due_deadline_registrations_v1(registrations, now);
    let next_deadline = prune_and_next_deadline_v1(registrations);
    let command = match next_deadline {
        Some(deadline) => {
            let wait = deadline.saturating_duration_since(Instant::now());
            match receiver.recv_timeout(wait) {
                Ok(command) => Some(command),
                Err(RecvTimeoutError::Timeout) => None,
                Err(RecvTimeoutError::Disconnected) => return false,
            }
        }
        None => match receiver.recv() {
            Ok(command) => Some(command),
            Err(_) => return false,
        },
    };
    if let Some(DeadlineSchedulerCommandV1::Register(registration)) = command {
        // The dispatcher reserves the process-wide registration maximum
        // before its sender is published. The RAII lease cap therefore proves
        // this push allocation-free.
        registrations.push(registration);
    }
    true
}

#[cfg(test)]
fn panic_next_deadline_scheduler_iteration_v1(
    state: &StackedFoldTransactionState,
) -> ArmedProcessGlobalOneShotFaultGuardV1<Weak<Mutex<PostApplyProofRegistryV1>>> {
    arm_deadline_scheduler_iteration_panic_for_registry_v1(&state.3)
}

#[cfg(test)]
fn arm_deadline_scheduler_iteration_panic_for_registry_v1(
    registry: &Arc<Mutex<PostApplyProofRegistryV1>>,
) -> ArmedProcessGlobalOneShotFaultGuardV1<Weak<Mutex<PostApplyProofRegistryV1>>> {
    arm_process_global_one_shot_fault_v1(
        &PANIC_NEXT_POST_APPLY_DEADLINE_REGISTRY_V1,
        Arc::downgrade(registry),
        "one scheduler panic may be armed",
    )
}

#[cfg(test)]
fn deadline_scheduler_panic_targets_registry_for_test_v1(
    registry: &Arc<Mutex<PostApplyProofRegistryV1>>,
) -> bool {
    let registry = Arc::downgrade(registry);
    PANIC_NEXT_POST_APPLY_DEADLINE_REGISTRY_V1
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .armed
        .as_ref()
        .is_some_and(|armed| Weak::ptr_eq(&armed.payload, &registry))
}

#[cfg(test)]
fn take_deadline_scheduler_panic_for_registry_for_test_v1(
    registry: &Arc<Mutex<PostApplyProofRegistryV1>>,
) -> bool {
    let registry = Arc::downgrade(registry);
    take_process_global_one_shot_fault_if_v1(
        &PANIC_NEXT_POST_APPLY_DEADLINE_REGISTRY_V1,
        |target| Weak::ptr_eq(target, &registry),
    )
    .is_some()
}

fn inject_deadline_scheduler_iteration_panic_for_test_v1(
    registrations: &[DeadlineSchedulerRegistrationV1],
) {
    #[cfg(test)]
    {
        let should_panic = take_process_global_one_shot_fault_if_v1(
            &PANIC_NEXT_POST_APPLY_DEADLINE_REGISTRY_V1,
            |target| {
                registrations
                    .iter()
                    .any(|registration| Weak::ptr_eq(&registration.registry, target))
            },
        )
        .is_some();
        if should_panic {
            panic!("injected post-Apply deadline scheduler iteration panic");
        }
    }
    #[cfg(not(test))]
    let _ = registrations;
}

#[cfg(test)]
fn panic_next_deadline_resolution_and_recovery_v1(
    job_token: ProjectId,
) -> ArmedDeadlineResolutionPanicGuardV1 {
    let mut slot = PANIC_NEXT_POST_APPLY_DEADLINE_RESOLUTION_V1
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if slot.armed.is_some() {
        drop(slot);
        panic!("one deadline resolution panic may be armed");
    }
    let token = slot
        .next_token
        .checked_add(1)
        .expect("deadline resolution panic token overflow");
    slot.next_token = token;
    let target = DeadlineResolutionPanicTargetV1 { token, job_token };
    slot.armed = Some(target);
    ArmedDeadlineResolutionPanicGuardV1 { target }
}

#[cfg(test)]
fn deadline_resolution_panic_targets_job_for_test_v1(job_token: ProjectId) -> bool {
    PANIC_NEXT_POST_APPLY_DEADLINE_RESOLUTION_V1
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .armed
        .is_some_and(|target| target.job_token == job_token)
}

#[cfg(test)]
fn take_deadline_resolution_panic_for_job_for_test_v1(job_token: ProjectId) -> bool {
    let mut slot = PANIC_NEXT_POST_APPLY_DEADLINE_RESOLUTION_V1
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if slot
        .armed
        .is_some_and(|target| target.job_token == job_token)
    {
        slot.armed = None;
        return true;
    }
    false
}

fn inject_deadline_resolution_panic_for_test_v1(job: &PostApplyProofJobV1) {
    #[cfg(test)]
    {
        if deadline_resolution_panic_targets_job_for_test_v1(job.job_token) {
            panic!("injected deadline resolution panic");
        }
    }
    #[cfg(not(test))]
    let _ = job;
}

fn fail_deadline_resolution_recovery_for_test_v1(job: &PostApplyProofJobV1) -> bool {
    #[cfg(test)]
    {
        if take_deadline_resolution_panic_for_job_for_test_v1(job.job_token) {
            return true;
        }
    }
    #[cfg(not(test))]
    let _ = job;
    false
}

#[cfg(test)]
fn force_next_post_apply_deadline_resource_failures_v1(
    registry: &Arc<Mutex<PostApplyProofRegistryV1>>,
    count: usize,
) -> ArmedProcessGlobalOneShotFaultGuardV1<ForcedPostApplyDeadlineResourceFailuresTargetV1> {
    assert!(count > 0, "at least one resource failure must be requested");
    arm_process_global_one_shot_fault_v1(
        &FORCE_POST_APPLY_DEADLINE_RESOURCE_FAILURES_V1,
        ForcedPostApplyDeadlineResourceFailuresTargetV1 {
            registry: Arc::downgrade(registry),
            remaining: count,
        },
        "one deadline resource-failure sequence may be armed",
    )
}

#[cfg(test)]
fn forced_post_apply_deadline_resource_failures_remaining_for_test_v1(
    registry: &Arc<Mutex<PostApplyProofRegistryV1>>,
) -> usize {
    let registry = Arc::downgrade(registry);
    FORCE_POST_APPLY_DEADLINE_RESOURCE_FAILURES_V1
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .armed
        .as_ref()
        .filter(|armed| Weak::ptr_eq(&armed.payload.registry, &registry))
        .map_or(0, |armed| armed.payload.remaining)
}

fn force_post_apply_deadline_resource_failure_for_test_v1(
    registry: &Arc<Mutex<PostApplyProofRegistryV1>>,
) -> bool {
    #[cfg(test)]
    {
        let registry = Arc::downgrade(registry);
        let mut slot = FORCE_POST_APPLY_DEADLINE_RESOURCE_FAILURES_V1
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let matches_target = slot
            .armed
            .as_ref()
            .is_some_and(|armed| Weak::ptr_eq(&armed.payload.registry, &registry));
        if !matches_target {
            return false;
        }
        let armed = slot
            .armed
            .as_mut()
            .expect("a matching resource-failure sequence is armed");
        armed.payload.remaining -= 1;
        if armed.payload.remaining == 0 {
            slot.armed = None;
        }
        true
    }
    #[cfg(not(test))]
    {
        let _ = registry;
        false
    }
}
