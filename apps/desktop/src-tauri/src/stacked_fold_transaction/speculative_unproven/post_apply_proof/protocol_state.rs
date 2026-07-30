const POST_APPLY_PROOF_PROTOCOL_VERSION_V1: u8 = 1;
const POST_APPLY_PROOF_SAMPLE_INTERVALS_V1: [usize; 3] = [16, 32, 64];
// A layered fallback may additionally prepare its retained admission and
// perform its own mandatory native revalidation.
const POST_APPLY_PROOF_MAX_DIAGNOSTIC_PASSES_PER_STAGE_V1: usize = 5;
const POST_APPLY_PROOF_TOTAL_WORK_V1: usize =
    (16 + 32 + 64) * POST_APPLY_PROOF_MAX_DIAGNOSTIC_PASSES_PER_STAGE_V1;
const MAX_POST_APPLY_PROOF_JOBS_V1: usize = 8;
const MAX_POST_APPLY_PROOF_RETAINED_BYTES_V1: usize = 8 * 1024 * 1024;
const MAX_POST_APPLY_PROOF_JOB_BYTES_V1: usize = 2 * 1024 * 1024;
const POST_APPLY_PROOF_START_RETENTION_V1: Duration = Duration::from_secs(5 * 60);
const POST_APPLY_PROOF_DEADLINE_V1: Duration = Duration::from_secs(30);
const MAX_POST_APPLY_DEADLINE_REGISTRATIONS_V1: usize = 64;
const POST_APPLY_DEADLINE_SCHEDULER_QUEUE_V1: usize = 128;
const POST_APPLY_DEADLINE_RETRY_MAX_SHIFT_V1: u32 = 7;
// Attempts and elapsed time bound each retry window. If exact publication is
// still unavailable, the bounded owner waits one full window before retrying;
// it is never discarded while its editor mark remains Awaiting.
const POST_APPLY_DEADLINE_RESOURCE_RETRY_MAX_ATTEMPTS_V1: u32 = 4;
const POST_APPLY_DEADLINE_RESOURCE_RETRY_MAX_DURATION_V1: Duration = Duration::from_secs(1);

// All managed application states share one bounded dispatcher. Registrations
// retain only weak canonical-state handles, so neither a closed project nor a
// discarded transaction state is kept alive by its deadline.
static POST_APPLY_DEADLINE_SCHEDULER_V1: OnceLock<
    Result<SyncSender<DeadlineSchedulerCommandV1>, ()>,
> = OnceLock::new();
static ACTIVE_POST_APPLY_DEADLINE_REGISTRATIONS_V1: AtomicUsize = AtomicUsize::new(0);

#[cfg(test)]
#[derive(Clone, Copy)]
struct PostApplyProofDeadlineOverrideV1 {
    token: u64,
    deadline: Duration,
}

#[cfg(test)]
thread_local! {
    static NEXT_POST_APPLY_PROOF_PUBLICATION_FAILURE_TOKEN_V1: Cell<u64> =
        const { Cell::new(0) };
    static FAIL_NEXT_POST_APPLY_PROOF_PUBLICATION_V1: Cell<Option<u64>> =
        const { Cell::new(None) };
    static NEXT_POST_APPLY_START_FAIL_CLOSED_RESOLUTION_FAILURE_TOKEN_V1: Cell<u64> =
        const { Cell::new(0) };
    static FAIL_NEXT_POST_APPLY_START_FAIL_CLOSED_RESOLUTION_V1: Cell<Option<u64>> =
        const { Cell::new(None) };
    static NEXT_POST_APPLY_PROOF_DEADLINE_TOKEN_V1: Cell<u64> = const { Cell::new(0) };
    static NEXT_POST_APPLY_PROOF_DEADLINE_V1: Cell<Option<PostApplyProofDeadlineOverrideV1>> =
        const { Cell::new(None) };
}
#[cfg(test)]
static PANIC_NEXT_POST_APPLY_DEADLINE_REGISTRY_V1: Mutex<
    ProcessGlobalOneShotFaultSlotV1<Weak<Mutex<PostApplyProofRegistryV1>>>,
> = Mutex::new(ProcessGlobalOneShotFaultSlotV1::new());
#[cfg(test)]
static PANIC_NEXT_POST_APPLY_DEADLINE_RESOLUTION_V1: Mutex<DeadlineResolutionPanicSlotV1> =
    Mutex::new(DeadlineResolutionPanicSlotV1 {
        next_token: 0,
        armed: None,
    });
#[cfg(test)]
static NEXT_POST_APPLY_BINDER_FAULT_V1: Mutex<
    ProcessGlobalOneShotFaultSlotV1<PostApplyBinderFaultTargetV1>,
> = Mutex::new(ProcessGlobalOneShotFaultSlotV1::new());
#[cfg(test)]
static OBSERVED_POST_APPLY_BINDER_VALIDATION_PANICS_V1: AtomicUsize = AtomicUsize::new(0);
#[cfg(test)]
static FAIL_NEXT_POST_APPLY_GENERIC_RESOLUTION_V1: Mutex<
    ProcessGlobalOneShotFaultSlotV1<ProjectId>,
> = Mutex::new(ProcessGlobalOneShotFaultSlotV1::new());
#[cfg(test)]
static PANIC_NEXT_POST_APPLY_WORKER_V1: Mutex<ProcessGlobalOneShotFaultSlotV1<ProjectId>> =
    Mutex::new(ProcessGlobalOneShotFaultSlotV1::new());
#[cfg(test)]
static FAIL_NEXT_POST_APPLY_DEADLINE_REGISTRATION_V1: Mutex<
    ProcessGlobalOneShotFaultSlotV1<Weak<Mutex<PostApplyProofRegistryV1>>>,
> = Mutex::new(ProcessGlobalOneShotFaultSlotV1::new());
#[cfg(test)]
static PANIC_NEXT_POST_APPLY_CERTIFIED_RESOLUTION_V1: Mutex<
    ProcessGlobalOneShotFaultSlotV1<PostApplyCertifiedResolutionPanicTargetV1>,
> = Mutex::new(ProcessGlobalOneShotFaultSlotV1::new());
#[cfg(test)]
static FORCE_POST_APPLY_DEADLINE_RESOURCE_FAILURES_V1: Mutex<
    ProcessGlobalOneShotFaultSlotV1<ForcedPostApplyDeadlineResourceFailuresTargetV1>,
> = Mutex::new(ProcessGlobalOneShotFaultSlotV1::new());

#[cfg(test)]
struct ProcessGlobalOneShotFaultV1<Payload> {
    token: u64,
    payload: Payload,
}

#[cfg(test)]
struct ProcessGlobalOneShotFaultSlotV1<Payload> {
    next_token: u64,
    armed: Option<ProcessGlobalOneShotFaultV1<Payload>>,
}

#[cfg(test)]
impl<Payload> ProcessGlobalOneShotFaultSlotV1<Payload> {
    const fn new() -> Self {
        Self {
            next_token: 0,
            armed: None,
        }
    }
}

#[cfg(test)]
#[must_use = "the process-global fault remains armed only while this guard is held"]
struct ArmedProcessGlobalOneShotFaultGuardV1<Payload: 'static> {
    slot: &'static Mutex<ProcessGlobalOneShotFaultSlotV1<Payload>>,
    token: u64,
}

#[cfg(test)]
impl<Payload: 'static> Drop for ArmedProcessGlobalOneShotFaultGuardV1<Payload> {
    fn drop(&mut self) {
        let mut slot = self
            .slot
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if slot
            .armed
            .as_ref()
            .is_some_and(|armed| armed.token == self.token)
        {
            slot.armed = None;
        }
    }
}

#[cfg(test)]
fn arm_process_global_one_shot_fault_v1<Payload: 'static>(
    slot: &'static Mutex<ProcessGlobalOneShotFaultSlotV1<Payload>>,
    payload: Payload,
    duplicate_message: &'static str,
) -> ArmedProcessGlobalOneShotFaultGuardV1<Payload> {
    let mut slot_state = slot
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if slot_state.armed.is_some() {
        drop(slot_state);
        panic!("{duplicate_message}");
    }
    let token = slot_state
        .next_token
        .checked_add(1)
        .expect("process-global one-shot fault token overflow");
    slot_state.next_token = token;
    slot_state.armed = Some(ProcessGlobalOneShotFaultV1 { token, payload });
    drop(slot_state);
    ArmedProcessGlobalOneShotFaultGuardV1 { slot, token }
}

#[cfg(test)]
fn take_process_global_one_shot_fault_if_v1<Payload: 'static>(
    slot: &'static Mutex<ProcessGlobalOneShotFaultSlotV1<Payload>>,
    matches_target: impl FnOnce(&Payload) -> bool,
) -> Option<Payload> {
    let mut slot = slot
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if slot
        .armed
        .as_ref()
        .is_some_and(|armed| matches_target(&armed.payload))
    {
        return slot.armed.take().map(|armed| armed.payload);
    }
    None
}

#[cfg(test)]
#[must_use = "the start-resolution fault remains armed only while this guard is held"]
struct ArmedPostApplyStartFailClosedResolutionFailureGuardV1 {
    token: u64,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

#[cfg(test)]
impl Drop for ArmedPostApplyStartFailClosedResolutionFailureGuardV1 {
    fn drop(&mut self) {
        FAIL_NEXT_POST_APPLY_START_FAIL_CLOSED_RESOLUTION_V1.with(|slot| {
            if slot.get() == Some(self.token) {
                slot.set(None);
            }
        });
    }
}

#[cfg(test)]
#[derive(Clone, Copy)]
struct DeadlineResolutionPanicTargetV1 {
    token: u64,
    job_token: ProjectId,
}

#[cfg(test)]
struct DeadlineResolutionPanicSlotV1 {
    next_token: u64,
    armed: Option<DeadlineResolutionPanicTargetV1>,
}

#[cfg(test)]
#[must_use = "the deadline-resolution fault remains armed only while this guard is held"]
struct ArmedDeadlineResolutionPanicGuardV1 {
    target: DeadlineResolutionPanicTargetV1,
}

#[cfg(test)]
impl Drop for ArmedDeadlineResolutionPanicGuardV1 {
    fn drop(&mut self) {
        let mut slot = PANIC_NEXT_POST_APPLY_DEADLINE_RESOLUTION_V1
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if slot.armed.is_some_and(|armed| {
            armed.token == self.target.token && armed.job_token == self.target.job_token
        }) {
            slot.armed = None;
        }
    }
}

#[cfg(test)]
#[must_use = "the publication fault remains armed only while this guard is held"]
struct ArmedPostApplyProofPublicationFailureGuardV1 {
    token: u64,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

#[cfg(test)]
impl Drop for ArmedPostApplyProofPublicationFailureGuardV1 {
    fn drop(&mut self) {
        FAIL_NEXT_POST_APPLY_PROOF_PUBLICATION_V1.with(|slot| {
            if slot.get() == Some(self.token) {
                slot.set(None);
            }
        });
    }
}

#[cfg(test)]
#[must_use = "the deadline override remains armed only while this guard is held"]
struct ArmedPostApplyProofDeadlineGuardV1 {
    token: u64,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

#[cfg(test)]
impl Drop for ArmedPostApplyProofDeadlineGuardV1 {
    fn drop(&mut self) {
        NEXT_POST_APPLY_PROOF_DEADLINE_V1.with(|slot| {
            if slot.get().is_some_and(|armed| armed.token == self.token) {
                slot.set(None);
            }
        });
    }
}

#[cfg(test)]
struct ForcedPostApplyDeadlineResourceFailuresTargetV1 {
    registry: Weak<Mutex<PostApplyProofRegistryV1>>,
    remaining: usize,
}
