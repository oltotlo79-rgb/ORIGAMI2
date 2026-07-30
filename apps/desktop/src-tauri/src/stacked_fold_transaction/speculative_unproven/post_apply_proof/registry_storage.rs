fn lock_registry_v1(
    state: &StackedFoldTransactionState,
) -> Result<MutexGuard<'_, PostApplyProofRegistryV1>, ()> {
    state.3.lock().map_err(|_| ())
}

fn reclaim_jobs_v1(
    registry: &mut PostApplyProofRegistryV1,
    mut reclaim: impl FnMut(&PostApplyProofJobV1) -> bool,
) {
    let mut index = 0;
    while index < registry.jobs.len() {
        if reclaim(&registry.jobs[index]) {
            remove_job_v1(registry, index);
        } else {
            index += 1;
        }
    }
}

fn remove_job_v1(registry: &mut PostApplyProofRegistryV1, index: usize) {
    if let Some(job) = registry.jobs.remove(index) {
        signal_inflight_cancellation_v1(&job);
        registry.retained_bytes = registry.retained_bytes.saturating_sub(job.retained_bytes);
    }
}

fn clear_jobs_v1(registry: &mut PostApplyProofRegistryV1) {
    while let Some(job) = registry.jobs.pop_front() {
        signal_inflight_cancellation_v1(&job);
    }
    registry.retained_bytes = 0;
}

fn unavailable_message_v1() -> String {
    "The post-Apply proof job is unavailable.".to_owned()
}
