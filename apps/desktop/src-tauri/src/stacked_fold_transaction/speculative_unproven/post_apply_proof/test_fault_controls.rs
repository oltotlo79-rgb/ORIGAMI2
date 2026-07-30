#[cfg(test)]
fn fail_next_post_apply_proof_publication_v1() -> ArmedPostApplyProofPublicationFailureGuardV1 {
    let token = NEXT_POST_APPLY_PROOF_PUBLICATION_FAILURE_TOKEN_V1.with(|next| {
        let token = next
            .get()
            .checked_add(1)
            .expect("proof publication failure token overflow");
        next.set(token);
        token
    });
    FAIL_NEXT_POST_APPLY_PROOF_PUBLICATION_V1.with(|slot| {
        assert!(slot.get().is_none(), "one publication failure may be armed");
        slot.set(Some(token));
    });
    ArmedPostApplyProofPublicationFailureGuardV1 {
        token,
        _not_send_or_sync: PhantomData,
    }
}

#[cfg(test)]
fn take_post_apply_proof_publication_failure_for_test_v1() -> bool {
    FAIL_NEXT_POST_APPLY_PROOF_PUBLICATION_V1
        .with(Cell::take)
        .is_some()
}

#[cfg(test)]
fn fail_next_post_apply_start_fail_closed_resolution_v1()
-> ArmedPostApplyStartFailClosedResolutionFailureGuardV1 {
    let token = NEXT_POST_APPLY_START_FAIL_CLOSED_RESOLUTION_FAILURE_TOKEN_V1.with(|next| {
        let token = next
            .get()
            .checked_add(1)
            .expect("start fail-closed resolution failure token overflow");
        next.set(token);
        token
    });
    FAIL_NEXT_POST_APPLY_START_FAIL_CLOSED_RESOLUTION_V1.with(|slot| {
        assert!(
            slot.get().is_none(),
            "one start fail-closed resolution failure may be armed"
        );
        slot.set(Some(token));
    });
    ArmedPostApplyStartFailClosedResolutionFailureGuardV1 {
        token,
        _not_send_or_sync: PhantomData,
    }
}

#[cfg(test)]
fn take_post_apply_start_fail_closed_resolution_failure_for_test_v1() -> bool {
    FAIL_NEXT_POST_APPLY_START_FAIL_CLOSED_RESOLUTION_V1
        .with(Cell::take)
        .is_some()
}

#[cfg(test)]
#[derive(Clone, Copy)]
enum InjectedPostApplyBinderFaultV1 {
    Allocation = 1,
    ValidationPanic = 2,
}

#[cfg(test)]
struct PostApplyBinderFaultTargetV1 {
    job_token: ProjectId,
    fault: InjectedPostApplyBinderFaultV1,
}

#[cfg(test)]
struct PostApplyCertifiedResolutionPanicTargetV1 {
    job_token: ProjectId,
    position: usize,
}

#[cfg(test)]
fn inject_next_post_apply_binder_fault_v1(
    job_token: &ProjectId,
    fault: InjectedPostApplyBinderFaultV1,
) -> ArmedProcessGlobalOneShotFaultGuardV1<PostApplyBinderFaultTargetV1> {
    arm_process_global_one_shot_fault_v1(
        &NEXT_POST_APPLY_BINDER_FAULT_V1,
        PostApplyBinderFaultTargetV1 {
            job_token: *job_token,
            fault,
        },
        "one binder fault may be armed",
    )
}

#[cfg(test)]
fn fail_next_post_apply_generic_resolution_v1(
    job_token: &ProjectId,
) -> ArmedProcessGlobalOneShotFaultGuardV1<ProjectId> {
    arm_process_global_one_shot_fault_v1(
        &FAIL_NEXT_POST_APPLY_GENERIC_RESOLUTION_V1,
        *job_token,
        "one generic resolution failure may be armed",
    )
}

#[cfg(test)]
fn panic_next_post_apply_worker_v1(
    job_token: &ProjectId,
) -> ArmedProcessGlobalOneShotFaultGuardV1<ProjectId> {
    arm_process_global_one_shot_fault_v1(
        &PANIC_NEXT_POST_APPLY_WORKER_V1,
        *job_token,
        "one worker panic may be armed",
    )
}

#[cfg(test)]
fn fail_next_post_apply_deadline_registration_v1(
    registry: &Arc<Mutex<PostApplyProofRegistryV1>>,
) -> ArmedProcessGlobalOneShotFaultGuardV1<Weak<Mutex<PostApplyProofRegistryV1>>> {
    arm_process_global_one_shot_fault_v1(
        &FAIL_NEXT_POST_APPLY_DEADLINE_REGISTRATION_V1,
        Arc::downgrade(registry),
        "one deadline registration failure may be armed",
    )
}

#[cfg(test)]
fn panic_next_post_apply_certified_resolution_before_v1(
    job_token: &ProjectId,
) -> ArmedProcessGlobalOneShotFaultGuardV1<PostApplyCertifiedResolutionPanicTargetV1> {
    arm_post_apply_certified_resolution_panic_v1(job_token, 1)
}

#[cfg(test)]
fn panic_next_post_apply_certified_resolution_after_v1(
    job_token: &ProjectId,
) -> ArmedProcessGlobalOneShotFaultGuardV1<PostApplyCertifiedResolutionPanicTargetV1> {
    arm_post_apply_certified_resolution_panic_v1(job_token, 2)
}

#[cfg(test)]
fn arm_post_apply_certified_resolution_panic_v1(
    job_token: &ProjectId,
    position: usize,
) -> ArmedProcessGlobalOneShotFaultGuardV1<PostApplyCertifiedResolutionPanicTargetV1> {
    arm_process_global_one_shot_fault_v1(
        &PANIC_NEXT_POST_APPLY_CERTIFIED_RESOLUTION_V1,
        PostApplyCertifiedResolutionPanicTargetV1 {
            job_token: *job_token,
            position,
        },
        "one certified resolution panic may be armed",
    )
}

#[cfg(test)]
fn set_next_post_apply_proof_deadline_v1(deadline: Duration) -> ArmedPostApplyProofDeadlineGuardV1 {
    let token = NEXT_POST_APPLY_PROOF_DEADLINE_TOKEN_V1.with(|next| {
        let token = next
            .get()
            .checked_add(1)
            .expect("proof deadline override token overflow");
        next.set(token);
        token
    });
    NEXT_POST_APPLY_PROOF_DEADLINE_V1.with(|slot| {
        assert!(
            slot.get().is_none(),
            "one proof deadline override may be armed"
        );
        slot.set(Some(PostApplyProofDeadlineOverrideV1 { token, deadline }));
    });
    ArmedPostApplyProofDeadlineGuardV1 {
        token,
        _not_send_or_sync: PhantomData,
    }
}

fn next_post_apply_proof_deadline_v1() -> Duration {
    #[cfg(test)]
    if let Some(armed) = NEXT_POST_APPLY_PROOF_DEADLINE_V1.with(Cell::take) {
        return armed.deadline;
    }
    POST_APPLY_PROOF_DEADLINE_V1
}
