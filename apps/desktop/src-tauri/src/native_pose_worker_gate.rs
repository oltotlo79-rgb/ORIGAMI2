//! Process-wide ownership gate for heavy native pose workers.
//!
//! The notified acquisition future registers its waker under the same mutex
//! that publishes permit release. This keeps worker handoff nonblocking for the
//! async runtime without opening a check-before-register lost-wakeup race.

use std::sync::{
    Arc, Mutex, MutexGuard,
    atomic::{AtomicU64, Ordering},
};

/// A native-pose request may have one active blocking worker and only a
/// small, bounded number of cancellable waiters. Reserving before a future is
/// first polled also bounds callers that create futures faster than the async
/// runtime can poll them.
const MAX_NATIVE_POSE_WORKER_WAITERS: usize = 16;

/// One process-wide heavy native pose worker per managed [`crate::AppState`].
///
/// A permit owns a monotonically issued epoch and can move into
/// `spawn_blocking`. Cancellation of the awaiting future therefore cannot
/// release capacity while the blocking closure is still running.
#[derive(Clone, Default)]
pub(super) struct NativePoseWorkerGate(Arc<NativePoseWorkerGateShared>);

#[derive(Default)]
struct NativePoseWorkerGateShared {
    state: Mutex<NativePoseWorkerGateState>,
    assigned_local_summary_generation: AtomicU64,
}

#[derive(Default)]
struct NativePoseWorkerGateState {
    active_owner: Option<u64>,
    next_owner: u64,
    next_waiter: u64,
    notification_epoch: u64,
    notification_exhausted: bool,
    owner_exhausted: bool,
    reserved_waiters: usize,
    waiters: Vec<(u64, std::task::Waker)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum NativePoseWorkerGenerationPublicationError {
    Exhausted,
    Superseded,
}

impl NativePoseWorkerGate {
    fn lock_state(&self) -> MutexGuard<'_, NativePoseWorkerGateState> {
        self.0
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub(super) fn try_acquire(&self) -> Option<NativePoseWorkerPermit> {
        let mut state = self.lock_state();
        Self::try_issue_permit(Arc::clone(&self.0), &mut state)
    }

    pub(super) fn assigned_local_summary_generation(&self) -> u64 {
        self.0
            .assigned_local_summary_generation
            .load(Ordering::SeqCst)
    }

    pub(super) fn try_publish_assigned_local_summary_generation(
        &self,
        observed_generation: u64,
    ) -> Result<u64, NativePoseWorkerGenerationPublicationError> {
        let generation = observed_generation
            .checked_add(1)
            .ok_or(NativePoseWorkerGenerationPublicationError::Exhausted)?;
        self.0
            .assigned_local_summary_generation
            .compare_exchange(
                observed_generation,
                generation,
                Ordering::SeqCst,
                Ordering::SeqCst,
            )
            .map(|_| generation)
            .map_err(|_| NativePoseWorkerGenerationPublicationError::Superseded)
    }

    pub(super) fn cancel_assigned_local_summary_generation(
        &self,
    ) -> Result<(), NativePoseWorkerGenerationPublicationError> {
        self.0
            .assigned_local_summary_generation
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |value| {
                value.checked_add(1)
            })
            .map(|_| ())
            .map_err(|_| NativePoseWorkerGenerationPublicationError::Exhausted)
    }

    pub(super) fn assigned_local_summary_generation_is_current(&self, generation: u64) -> bool {
        self.assigned_local_summary_generation() == generation
    }

    /// Waits without polling until capacity is published or the caller's
    /// ownership predicate is revoked.
    ///
    /// Call [`Self::notify_waiters`] when changing state observed by
    /// `keep_waiting`; this makes revocation prompt even if the active worker
    /// has not exited yet. The predicate is checked on both sides of permit
    /// acquisition, so a revoked waiter cannot retain capacity after waking.
    pub(super) fn acquire_notified_while(
        &self,
        keep_waiting: impl Fn() -> bool + Send + 'static,
    ) -> NativePoseWorkerAcquireFuture {
        let waiter_id = {
            let mut state = self.lock_state();
            if state.owner_exhausted
                || state.notification_exhausted
                || state.reserved_waiters >= MAX_NATIVE_POSE_WORKER_WAITERS
            {
                return NativePoseWorkerAcquireFuture::exhausted(self.clone());
            }
            let waiter_id = state.next_waiter;
            state.next_waiter = match waiter_id.checked_add(1) {
                Some(next) => next,
                None => return NativePoseWorkerAcquireFuture::exhausted(self.clone()),
            };
            state.reserved_waiters += 1;
            waiter_id
        };
        NativePoseWorkerAcquireFuture {
            gate: self.clone(),
            waiter_id: Some(waiter_id),
            keep_waiting: Box::new(keep_waiting),
            completed: false,
            has_waiter_reservation: true,
        }
    }

    /// Wakes notified acquirers after an external ownership predicate changes.
    pub(super) fn notify_waiters(&self) {
        let waiters = {
            let mut state = self.lock_state();
            match state.notification_epoch.checked_add(1) {
                Some(next) => state.notification_epoch = next,
                None => state.notification_exhausted = true,
            }
            std::mem::take(&mut state.waiters)
        };
        Self::wake_registered_waiters(waiters);
    }

    fn try_issue_permit(
        shared: Arc<NativePoseWorkerGateShared>,
        state: &mut NativePoseWorkerGateState,
    ) -> Option<NativePoseWorkerPermit> {
        if state.active_owner.is_some() {
            return None;
        }
        if state.owner_exhausted {
            return None;
        }
        let owner = state.next_owner;
        match owner.checked_add(1) {
            Some(next) => state.next_owner = next,
            None => state.owner_exhausted = true,
        }
        state.active_owner = Some(owner);
        Some(NativePoseWorkerPermit { shared, owner })
    }

    fn remove_registered_waiter(
        state: &mut NativePoseWorkerGateState,
        waiter_id: u64,
    ) -> Option<std::task::Waker> {
        state
            .waiters
            .iter()
            .position(|(candidate, _)| *candidate == waiter_id)
            .map(|index| state.waiters.swap_remove(index).1)
    }

    fn wake_registered_waiters(waiters: Vec<(u64, std::task::Waker)>) {
        for (_, waiter) in waiters {
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| waiter.wake()));
        }
    }

    #[cfg(test)]
    pub(super) fn is_busy(&self) -> bool {
        self.lock_state().active_owner.is_some()
    }

    #[cfg(test)]
    pub(super) fn waiting_count(&self) -> usize {
        self.lock_state().waiters.len()
    }

    #[cfg(test)]
    pub(super) fn reserved_waiter_count(&self) -> usize {
        self.lock_state().reserved_waiters
    }
}

pub(super) struct NativePoseWorkerAcquireFuture {
    gate: NativePoseWorkerGate,
    waiter_id: Option<u64>,
    keep_waiting: Box<dyn Fn() -> bool + Send + 'static>,
    completed: bool,
    has_waiter_reservation: bool,
}

impl NativePoseWorkerAcquireFuture {
    fn exhausted(gate: NativePoseWorkerGate) -> Self {
        Self {
            gate,
            waiter_id: None,
            keep_waiting: Box::new(|| false),
            completed: false,
            has_waiter_reservation: false,
        }
    }

    fn release_waiter_reservation(&mut self) {
        if !self.has_waiter_reservation {
            return;
        }
        let Some(waiter_id) = self.waiter_id else {
            return;
        };
        let retired_waker = {
            let mut state = self.gate.lock_state();
            let retired_waker =
                NativePoseWorkerGate::remove_registered_waiter(&mut state, waiter_id);
            debug_assert!(state.reserved_waiters > 0);
            if let Some(next) = state.reserved_waiters.checked_sub(1) {
                state.reserved_waiters = next;
            } else {
                // This is unreachable while the gate's private reservation
                // invariant holds. Preserve fail-closed admission if a poisoned
                // mutex ever exposes corrupted state rather than wrapping and
                // admitting unbounded waiters.
                state.reserved_waiters = MAX_NATIVE_POSE_WORKER_WAITERS;
            }
            retired_waker
        };
        // A RawWaker drop is foreign code and may re-enter the gate. Never run
        // it while the state mutex is held.
        drop(retired_waker);
        self.has_waiter_reservation = false;
    }
}

impl std::future::Future for NativePoseWorkerAcquireFuture {
    type Output = Option<NativePoseWorkerPermit>;

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        context: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        if self.completed || self.waiter_id.is_none() {
            self.completed = true;
            return std::task::Poll::Ready(None);
        }
        loop {
            // Snapshot notification state before calling foreign predicate
            // code. A notification between this snapshot and waiter
            // registration forces another predicate check.
            let observed_notification = {
                let state = self.gate.lock_state();
                if state.notification_exhausted {
                    drop(state);
                    self.release_waiter_reservation();
                    self.completed = true;
                    return std::task::Poll::Ready(None);
                }
                state.notification_epoch
            };
            if !(self.keep_waiting)() {
                self.release_waiter_reservation();
                self.completed = true;
                return std::task::Poll::Ready(None);
            }

            let shared = Arc::clone(&self.gate.0);
            let waiter_id = self.waiter_id.expect("checked above");
            // RawWaker::clone is foreign code. Prepare the candidate before
            // taking the gate mutex, then retire any replaced waker after
            // releasing it.
            let mut candidate_waker = Some(context.waker().clone());
            let mut state = self.gate.lock_state();
            if state.notification_exhausted {
                drop(state);
                self.release_waiter_reservation();
                self.completed = true;
                return std::task::Poll::Ready(None);
            }
            if state.notification_epoch != observed_notification {
                drop(state);
                continue;
            }
            if state.active_owner.is_none() {
                let retired_waker =
                    NativePoseWorkerGate::remove_registered_waiter(&mut state, waiter_id);
                let permit = NativePoseWorkerGate::try_issue_permit(shared, &mut state);
                drop(state);
                drop(retired_waker);
                drop(candidate_waker);
                self.completed = true;
                self.release_waiter_reservation();
                let Some(permit) = permit else {
                    return std::task::Poll::Ready(None);
                };
                if !(self.keep_waiting)() {
                    drop(permit);
                    return std::task::Poll::Ready(None);
                }
                return std::task::Poll::Ready(Some(permit));
            }

            let waiter_capacity_available = state.waiters.len() < MAX_NATIVE_POSE_WORKER_WAITERS;
            let retired_waker = match state
                .waiters
                .iter_mut()
                .find(|(candidate, _)| *candidate == waiter_id)
            {
                Some((_, registered)) if !registered.will_wake(context.waker()) => Some(
                    std::mem::replace(registered, candidate_waker.take().expect("candidate waker")),
                ),
                Some(_) => None,
                None if waiter_capacity_available => {
                    if state.waiters.try_reserve(1).is_err() {
                        drop(state);
                        drop(candidate_waker);
                        self.release_waiter_reservation();
                        self.completed = true;
                        return std::task::Poll::Ready(None);
                    }
                    state
                        .waiters
                        .push((waiter_id, candidate_waker.take().expect("candidate waker")));
                    None
                }
                None => {
                    drop(state);
                    drop(candidate_waker);
                    self.release_waiter_reservation();
                    self.completed = true;
                    return std::task::Poll::Ready(None);
                }
            };
            drop(state);
            drop(retired_waker);
            drop(candidate_waker);
            return std::task::Poll::Pending;
        }
    }
}

impl Drop for NativePoseWorkerAcquireFuture {
    fn drop(&mut self) {
        if !self.completed {
            self.release_waiter_reservation();
        }
    }
}

pub(super) struct NativePoseWorkerPermit {
    shared: Arc<NativePoseWorkerGateShared>,
    owner: u64,
}

impl Drop for NativePoseWorkerPermit {
    fn drop(&mut self) {
        let waiters = {
            let mut state = self
                .shared
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if state.active_owner != Some(self.owner) {
                return;
            }
            state.active_owner = None;
            match state.notification_epoch.checked_add(1) {
                Some(next) => state.notification_epoch = next,
                None => state.notification_exhausted = true,
            }
            std::mem::take(&mut state.waiters)
        };
        NativePoseWorkerGate::wake_registered_waiters(waiters);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        future::Future as _,
        sync::{
            Arc,
            atomic::{AtomicBool, AtomicUsize, Ordering},
        },
        task::{Context, Poll, Wake, Waker},
    };

    #[derive(Default)]
    struct CountWake(AtomicUsize);

    impl CountWake {
        fn count(&self) -> usize {
            self.0.load(Ordering::Acquire)
        }
    }

    impl Wake for CountWake {
        fn wake(self: Arc<Self>) {
            self.0.fetch_add(1, Ordering::AcqRel);
        }

        fn wake_by_ref(self: &Arc<Self>) {
            self.0.fetch_add(1, Ordering::AcqRel);
        }
    }

    struct GateLockProbeWake {
        gate: NativePoseWorkerGate,
        dropped_after_unlock: Arc<AtomicBool>,
    }

    impl Wake for GateLockProbeWake {
        fn wake(self: Arc<Self>) {}

        fn wake_by_ref(self: &Arc<Self>) {}
    }

    impl Drop for GateLockProbeWake {
        fn drop(&mut self) {
            self.dropped_after_unlock
                .store(self.gate.0.state.try_lock().is_ok(), Ordering::Release);
        }
    }

    fn poll_once(
        future: &mut std::pin::Pin<Box<NativePoseWorkerAcquireFuture>>,
        wake: &Arc<CountWake>,
    ) -> Poll<Option<NativePoseWorkerPermit>> {
        let waker = Waker::from(Arc::clone(wake));
        let mut context = Context::from_waker(&waker);
        future.as_mut().poll(&mut context)
    }

    #[test]
    fn notified_acquire_waits_for_exact_permit_release() {
        let gate = NativePoseWorkerGate::default();
        let held = gate.try_acquire().expect("initial permit");
        let wake = Arc::new(CountWake::default());
        let mut waiting = Box::pin(gate.acquire_notified_while(|| true));

        assert!(poll_once(&mut waiting, &wake).is_pending());
        assert_eq!(gate.waiting_count(), 1);
        assert_eq!(wake.count(), 0);

        drop(held);
        assert_eq!(wake.count(), 1);
        let acquired = match poll_once(&mut waiting, &wake) {
            Poll::Ready(Some(permit)) => permit,
            _ => panic!("released capacity must satisfy the notified waiter"),
        };
        assert!(gate.is_busy());
        drop(acquired);
        assert!(!gate.is_busy());
    }

    #[test]
    fn dropping_or_revoking_waiter_never_leaks_worker_capacity() {
        let gate = NativePoseWorkerGate::default();
        let held = gate.try_acquire().expect("initial permit");
        let keep_waiting = Arc::new(AtomicBool::new(true));
        let predicate = Arc::clone(&keep_waiting);
        let wake = Arc::new(CountWake::default());
        let mut waiting =
            Box::pin(gate.acquire_notified_while(move || predicate.load(Ordering::Acquire)));

        assert!(poll_once(&mut waiting, &wake).is_pending());
        keep_waiting.store(false, Ordering::Release);
        gate.notify_waiters();
        assert_eq!(wake.count(), 1);
        assert!(matches!(poll_once(&mut waiting, &wake), Poll::Ready(None)));
        assert_eq!(gate.waiting_count(), 0);
        assert!(gate.is_busy(), "revocation cannot release another owner");

        drop(waiting);
        drop(held);
        assert!(!gate.is_busy());
        drop(
            gate.try_acquire()
                .expect("capacity after waiter cancellation"),
        );
        assert!(!gate.is_busy());
    }

    #[test]
    fn dropping_a_pending_waiter_unregisters_its_waker() {
        let gate = NativePoseWorkerGate::default();
        let held = gate.try_acquire().expect("initial permit");
        let wake = Arc::new(CountWake::default());
        let mut waiting = Box::pin(gate.acquire_notified_while(|| true));

        assert!(poll_once(&mut waiting, &wake).is_pending());
        assert_eq!(gate.waiting_count(), 1);
        assert_eq!(gate.reserved_waiter_count(), 1);
        drop(waiting);
        assert_eq!(gate.waiting_count(), 0);
        assert_eq!(gate.reserved_waiter_count(), 0);

        drop(held);
        assert_eq!(wake.count(), 0, "a dropped waiter must not be woken");
        drop(gate.try_acquire().expect("capacity after waiter drop"));
        assert!(!gate.is_busy());
    }

    #[test]
    fn dropping_registered_waker_runs_foreign_drop_after_gate_unlock() {
        let gate = NativePoseWorkerGate::default();
        let held = gate.try_acquire().expect("initial permit");
        let dropped_after_unlock = Arc::new(AtomicBool::new(false));
        let waker = Waker::from(Arc::new(GateLockProbeWake {
            gate: gate.clone(),
            dropped_after_unlock: Arc::clone(&dropped_after_unlock),
        }));
        let mut waiting = Box::pin(gate.acquire_notified_while(|| true));
        {
            let mut context = Context::from_waker(&waker);
            assert!(waiting.as_mut().poll(&mut context).is_pending());
        }
        drop(waker);
        assert_eq!(gate.waiting_count(), 1);

        drop(waiting);
        assert!(
            dropped_after_unlock.load(Ordering::Acquire),
            "RawWaker drop must be retired outside the gate mutex"
        );
        drop(held);
        assert!(!gate.is_busy());
    }

    #[test]
    fn waiter_limit_is_exact_fail_closed_and_drop_releases_reservations() {
        let gate = NativePoseWorkerGate::default();
        let held = gate.try_acquire().expect("initial permit");
        let wake = Arc::new(CountWake::default());
        let mut waiters = (0..MAX_NATIVE_POSE_WORKER_WAITERS)
            .map(|_| Box::pin(gate.acquire_notified_while(|| true)))
            .collect::<Vec<_>>();

        for waiter in &mut waiters {
            assert!(poll_once(waiter, &wake).is_pending());
        }
        assert_eq!(gate.waiting_count(), MAX_NATIVE_POSE_WORKER_WAITERS);
        assert_eq!(gate.reserved_waiter_count(), MAX_NATIVE_POSE_WORKER_WAITERS);

        let mut one_over = Box::pin(gate.acquire_notified_while(|| true));
        assert!(matches!(poll_once(&mut one_over, &wake), Poll::Ready(None)));
        assert!(gate.is_busy(), "rejected waiter cannot release the owner");
        assert_eq!(gate.waiting_count(), MAX_NATIVE_POSE_WORKER_WAITERS);
        assert_eq!(
            gate.reserved_waiter_count(),
            MAX_NATIVE_POSE_WORKER_WAITERS,
            "one-over admission must not consume or release a reservation"
        );

        drop(one_over);
        drop(waiters);
        assert_eq!(gate.waiting_count(), 0);
        assert_eq!(gate.reserved_waiter_count(), 0);
        drop(held);
        assert!(!gate.is_busy());
    }

    #[test]
    fn unpolled_futures_consume_the_exact_waiter_reservation_limit() {
        let gate = NativePoseWorkerGate::default();
        let held = gate.try_acquire().expect("initial permit");
        let waiters = (0..MAX_NATIVE_POSE_WORKER_WAITERS)
            .map(|_| gate.acquire_notified_while(|| true))
            .collect::<Vec<_>>();
        assert_eq!(gate.waiting_count(), 0, "unpolled futures have no wakers");
        assert_eq!(
            gate.reserved_waiter_count(),
            MAX_NATIVE_POSE_WORKER_WAITERS,
            "reservation occurs when the future is created"
        );

        let wake = Arc::new(CountWake::default());
        let mut one_over = Box::pin(gate.acquire_notified_while(|| true));
        assert!(matches!(poll_once(&mut one_over, &wake), Poll::Ready(None)));
        assert_eq!(
            gate.reserved_waiter_count(),
            MAX_NATIVE_POSE_WORKER_WAITERS,
            "an exhausted future owns no reservation"
        );

        drop(waiters);
        assert_eq!(gate.reserved_waiter_count(), 0);
        drop(held);
        assert!(!gate.is_busy());
    }

    #[test]
    fn final_owner_epoch_exhausts_without_leaking_waiters_or_pending_forever() {
        let gate = NativePoseWorkerGate::default();
        {
            let mut state = gate.lock_state();
            state.next_owner = u64::MAX;
        }
        let final_owner = gate.try_acquire().expect("u64::MAX owner is issued once");
        assert_eq!(final_owner.owner, u64::MAX);
        assert!(gate.lock_state().owner_exhausted);
        assert!(gate.try_acquire().is_none(), "no owner may wrap while busy");

        let wake = Arc::new(CountWake::default());
        let mut rejected = Box::pin(gate.acquire_notified_while(|| true));
        assert!(matches!(poll_once(&mut rejected, &wake), Poll::Ready(None)));
        assert_eq!(gate.waiting_count(), 0);
        assert_eq!(gate.reserved_waiter_count(), 0);

        drop(final_owner);
        assert!(!gate.is_busy());
        assert!(
            gate.try_acquire().is_none(),
            "owner allocation remains exhausted"
        );
    }

    #[test]
    fn queued_waiter_after_final_owner_exits_instead_of_remaining_pending() {
        let gate = NativePoseWorkerGate::default();
        let initial_owner = gate.try_acquire().expect("initial permit");
        {
            let mut state = gate.lock_state();
            state.next_owner = u64::MAX;
        }
        let first_wake = Arc::new(CountWake::default());
        let second_wake = Arc::new(CountWake::default());
        let mut first = Box::pin(gate.acquire_notified_while(|| true));
        let mut second = Box::pin(gate.acquire_notified_while(|| true));
        assert!(poll_once(&mut first, &first_wake).is_pending());
        assert!(poll_once(&mut second, &second_wake).is_pending());
        assert_eq!(gate.reserved_waiter_count(), 2);

        drop(initial_owner);
        let final_owner = match poll_once(&mut first, &first_wake) {
            Poll::Ready(Some(permit)) => permit,
            _ => panic!("first queued waiter must receive the final owner epoch"),
        };
        assert_eq!(final_owner.owner, u64::MAX);
        assert!(gate.lock_state().owner_exhausted);
        assert!(poll_once(&mut second, &second_wake).is_pending());
        assert_eq!(gate.waiting_count(), 1);
        assert_eq!(gate.reserved_waiter_count(), 1);

        drop(final_owner);
        assert_eq!(
            second_wake.count(),
            2,
            "the final owner release must wake the re-registered waiter"
        );
        assert!(matches!(
            poll_once(&mut second, &second_wake),
            Poll::Ready(None)
        ));
        assert_eq!(gate.waiting_count(), 0);
        assert_eq!(gate.reserved_waiter_count(), 0);
        assert!(!gate.is_busy());
    }

    #[test]
    fn exhausted_notification_epoch_rejects_before_reserving_an_unpolled_future() {
        let gate = NativePoseWorkerGate::default();
        {
            let mut state = gate.lock_state();
            state.notification_epoch = u64::MAX;
        }
        gate.notify_waiters();
        assert!(gate.lock_state().notification_exhausted);

        let wake = Arc::new(CountWake::default());
        let mut rejected = Box::pin(gate.acquire_notified_while(|| true));
        assert_eq!(gate.reserved_waiter_count(), 0);
        assert!(matches!(poll_once(&mut rejected, &wake), Poll::Ready(None)));
        assert_eq!(gate.waiting_count(), 0);
        assert_eq!(gate.reserved_waiter_count(), 0);
    }

    #[test]
    fn poisoned_gate_recovers_without_leaking_a_waiter_reservation() {
        let gate = NativePoseWorkerGate::default();
        let poisoned = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _state = gate.lock_state();
            panic!("inject gate mutex poison");
        }));
        assert!(poisoned.is_err());

        let held = gate.try_acquire().expect("poison recovery issues a permit");
        let wake = Arc::new(CountWake::default());
        let mut waiting = Box::pin(gate.acquire_notified_while(|| true));
        assert!(poll_once(&mut waiting, &wake).is_pending());
        assert_eq!(gate.waiting_count(), 1);
        assert_eq!(gate.reserved_waiter_count(), 1);

        drop(waiting);
        assert_eq!(gate.waiting_count(), 0);
        assert_eq!(gate.reserved_waiter_count(), 0);
        drop(held);
        assert!(!gate.is_busy());
    }

    #[test]
    fn notification_between_predicate_check_and_registration_is_not_lost() {
        let gate = NativePoseWorkerGate::default();
        let held = gate.try_acquire().expect("initial permit");
        let predicate_calls = Arc::new(AtomicUsize::new(0));
        let predicate_gate = gate.clone();
        let predicate_count = Arc::clone(&predicate_calls);
        let wake = Arc::new(CountWake::default());
        let mut waiting = Box::pin(gate.acquire_notified_while(move || {
            if predicate_count.fetch_add(1, Ordering::AcqRel) == 0 {
                predicate_gate.notify_waiters();
            }
            true
        }));

        assert!(poll_once(&mut waiting, &wake).is_pending());
        assert_eq!(
            predicate_calls.load(Ordering::Acquire),
            2,
            "an intervening notification must force a fresh ownership check",
        );
        assert_eq!(gate.waiting_count(), 1);

        drop(held);
        assert_eq!(wake.count(), 1);
        let permit = match poll_once(&mut waiting, &wake) {
            Poll::Ready(Some(permit)) => permit,
            _ => panic!("release after re-registration must wake and acquire"),
        };
        drop(permit);
        assert!(!gate.is_busy());
    }

    #[test]
    fn multiple_notified_waiters_serialize_without_losing_a_wakeup() {
        let gate = NativePoseWorkerGate::default();
        let held = gate.try_acquire().expect("initial permit");
        let first_wake = Arc::new(CountWake::default());
        let second_wake = Arc::new(CountWake::default());
        let mut first = Box::pin(gate.acquire_notified_while(|| true));
        let mut second = Box::pin(gate.acquire_notified_while(|| true));

        assert!(poll_once(&mut first, &first_wake).is_pending());
        assert!(poll_once(&mut second, &second_wake).is_pending());
        assert_eq!(gate.waiting_count(), 2);
        drop(held);
        assert_eq!(first_wake.count(), 1);
        assert_eq!(second_wake.count(), 1);

        let first_permit = match poll_once(&mut first, &first_wake) {
            Poll::Ready(Some(permit)) => permit,
            _ => panic!("first waiter must acquire released capacity"),
        };
        assert!(poll_once(&mut second, &second_wake).is_pending());
        drop(first_permit);
        assert_eq!(second_wake.count(), 2);
        let second_permit = match poll_once(&mut second, &second_wake) {
            Poll::Ready(Some(permit)) => permit,
            _ => panic!("second waiter must acquire the next release"),
        };
        drop(second_permit);
        assert!(!gate.is_busy());
    }

    #[test]
    fn panic_unwind_drops_owner_and_wakes_waiter() {
        let gate = NativePoseWorkerGate::default();
        let held = gate.try_acquire().expect("initial permit");
        let wake = Arc::new(CountWake::default());
        let mut waiting = Box::pin(gate.acquire_notified_while(|| true));
        assert!(poll_once(&mut waiting, &wake).is_pending());

        let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _permit = held;
            panic!("injected native worker panic");
        }));
        assert!(unwind.is_err());
        assert_eq!(wake.count(), 1);
        let recovered = match poll_once(&mut waiting, &wake) {
            Poll::Ready(Some(permit)) => permit,
            _ => panic!("panic release must satisfy the waiter"),
        };
        drop(recovered);
        assert!(!gate.is_busy());
    }

    #[test]
    fn revocation_after_owner_release_cannot_adopt_the_woken_capacity() {
        let gate = NativePoseWorkerGate::default();
        let held = gate.try_acquire().expect("initial permit");
        let keep_waiting = Arc::new(AtomicBool::new(true));
        let predicate = Arc::clone(&keep_waiting);
        let wake = Arc::new(CountWake::default());
        let mut waiting =
            Box::pin(gate.acquire_notified_while(move || predicate.load(Ordering::Acquire)));
        assert!(poll_once(&mut waiting, &wake).is_pending());

        drop(held);
        assert_eq!(wake.count(), 1);
        keep_waiting.store(false, Ordering::Release);
        gate.notify_waiters();
        assert!(matches!(poll_once(&mut waiting, &wake), Poll::Ready(None)));
        assert_eq!(gate.reserved_waiter_count(), 0);
        assert!(!gate.is_busy());
    }

    #[test]
    fn stale_owner_epoch_drop_cannot_release_a_newer_owner() {
        let gate = NativePoseWorkerGate::default();
        let first = gate.try_acquire().expect("first permit");
        let stale_owner = first.owner;
        let shared = Arc::clone(&first.shared);
        drop(first);
        let current = gate.try_acquire().expect("newer permit");
        assert_ne!(current.owner, stale_owner);

        drop(NativePoseWorkerPermit {
            shared,
            owner: stale_owner,
        });
        assert!(
            gate.is_busy(),
            "a delayed stale epoch must not release the current owner"
        );
        drop(current);
        assert!(!gate.is_busy());
    }

    #[test]
    fn active_owner_release_recovers_a_poisoned_gate_and_wakes_waiters() {
        let gate = NativePoseWorkerGate::default();
        let held = gate.try_acquire().expect("initial permit");
        let wake = Arc::new(CountWake::default());
        let mut waiting = Box::pin(gate.acquire_notified_while(|| true));
        assert!(poll_once(&mut waiting, &wake).is_pending());

        let poisoned = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _state = gate.lock_state();
            panic!("inject poison while an owner and waiter are live");
        }));
        assert!(poisoned.is_err());
        drop(held);
        assert_eq!(wake.count(), 1);

        let recovered = match poll_once(&mut waiting, &wake) {
            Poll::Ready(Some(permit)) => permit,
            _ => panic!("poison recovery must preserve the exact waiter handoff"),
        };
        drop(recovered);
        assert!(!gate.is_busy());
    }

    #[test]
    fn permit_arc_keeps_its_gate_alive_until_blocking_work_releases_it() {
        let gate = NativePoseWorkerGate::default();
        let shared = Arc::downgrade(&gate.0);
        let permit = gate.try_acquire().expect("worker permit");
        drop(gate);
        assert!(
            shared.upgrade().is_some(),
            "the moved permit owns the process gate during blocking work"
        );
        drop(permit);
        assert!(
            shared.upgrade().is_none(),
            "the gate allocation is released after its final permit"
        );
    }

    #[test]
    fn busy_summary_rejection_does_not_cancel_the_admitted_worker() {
        let state = crate::AppState::new(crate::initial_project_state());
        let (admitted_generation, held) =
            crate::try_begin_assigned_local_sufficiency_summary_v1(&state)
                .expect("admitted summary worker");
        let generation_before = state.1.assigned_local_summary_generation();

        let error = match crate::try_begin_assigned_local_sufficiency_summary_v1(&state) {
            Ok(_) => panic!("a parallel summary must not bypass the worker gate"),
            Err(error) => error,
        };
        assert_eq!(error, crate::ASSIGNED_LOCAL_SUMMARY_BUSY_MESSAGE_V1);
        assert_eq!(
            state.1.assigned_local_summary_generation(),
            generation_before,
            "a rejected request owns no cancellation generation"
        );
        assert!(
            crate::ensure_assigned_local_sufficiency_summary_generation_v1(
                &state,
                admitted_generation,
            )
            .is_ok(),
            "a rejected request must not cancel the admitted summary"
        );
        drop(held);
        assert!(!state.native_pose_worker_is_busy());
    }

    #[test]
    fn cancellation_wins_the_admission_to_generation_publication_race() {
        let state = crate::AppState::new(crate::initial_project_state());
        let error =
            match crate::try_begin_assigned_local_sufficiency_summary_with_pre_publish_hook_v1(
                &state,
                || {
                    crate::cancel_assigned_local_sufficiency_summary_for_state_v1(&state)
                        .expect("publish cancellation");
                },
            ) {
                Ok(_) => panic!("cancellation between admission and publication must win"),
                Err(error) => error,
            };
        assert_eq!(error, crate::ASSIGNED_LOCAL_SUMMARY_CANCELLED_MESSAGE_V1);
        assert!(
            !state.native_pose_worker_is_busy(),
            "failed generation publication must release its permit"
        );
    }

    #[test]
    fn cancellation_after_worker_exit_invalidates_delayed_summary_adoption() {
        let state = crate::AppState::new(crate::initial_project_state());
        let (generation, permit) = crate::try_begin_assigned_local_sufficiency_summary_v1(&state)
            .expect("admit summary worker");
        drop(permit);
        crate::cancel_assigned_local_sufficiency_summary_for_state_v1(&state)
            .expect("cancel delayed adoption");

        assert_eq!(
            crate::ensure_assigned_local_sufficiency_summary_generation_v1(&state, generation),
            Err(crate::ASSIGNED_LOCAL_SUMMARY_CANCELLED_MESSAGE_V1.to_owned())
        );
        assert!(!state.native_pose_worker_is_busy());
    }

    #[test]
    fn assigned_local_generations_are_isolated_per_app_state_without_aba() {
        let first = crate::AppState::new(crate::initial_project_state());
        let second = crate::AppState::new(crate::initial_project_state());
        let (first_generation, first_permit) =
            crate::try_begin_assigned_local_sufficiency_summary_v1(&first)
                .expect("first state summary");
        let (second_generation, second_permit) =
            crate::try_begin_assigned_local_sufficiency_summary_v1(&second)
                .expect("second state summary");

        crate::cancel_assigned_local_sufficiency_summary_for_state_v1(&first)
            .expect("cancel first state only");
        assert_eq!(
            crate::ensure_assigned_local_sufficiency_summary_generation_v1(
                &first,
                first_generation,
            ),
            Err(crate::ASSIGNED_LOCAL_SUMMARY_CANCELLED_MESSAGE_V1.to_owned())
        );
        assert!(
            crate::ensure_assigned_local_sufficiency_summary_generation_v1(
                &second,
                second_generation,
            )
            .is_ok(),
            "one managed AppState must not cancel another generation"
        );

        drop(first_permit);
        drop(second_permit);
        assert!(!first.native_pose_worker_is_busy());
        assert!(!second.native_pose_worker_is_busy());
    }

    #[test]
    fn exhausted_assigned_local_generation_releases_admission_permit() {
        let state = crate::AppState::new(crate::initial_project_state());
        state
            .1
            .0
            .assigned_local_summary_generation
            .store(u64::MAX, Ordering::SeqCst);

        let error = match crate::try_begin_assigned_local_sufficiency_summary_v1(&state) {
            Ok(_) => panic!("an exhausted generation must fail closed"),
            Err(error) => error,
        };
        assert_eq!(
            error,
            crate::ASSIGNED_LOCAL_SUMMARY_GENERATION_EXHAUSTED_MESSAGE_V1
        );
        assert!(
            !state.native_pose_worker_is_busy(),
            "failed generation publication must release admission"
        );
    }

    #[test]
    fn invalid_summary_binding_never_publishes_or_owns_a_generation() {
        let state = crate::AppState::new(crate::initial_project_state());
        let mut request = {
            let project = crate::lock_project(&state).expect("project");
            crate::AssignedLocalSufficiencySummaryRequestV1 {
                expected_project_instance_id: project.instance_id,
                expected_project_id: project.project_id,
                expected_revision: project.editor.revision(),
                expected_fold_model_fingerprint: project.editor.fold_model_fingerprint_v1(),
            }
        };
        let replacement = if request.expected_fold_model_fingerprint.starts_with('0') {
            "1"
        } else {
            "0"
        };
        request
            .expected_fold_model_fingerprint
            .replace_range(..1, replacement);
        let generation_before = state.1.assigned_local_summary_generation();

        let error = match crate::try_prepare_assigned_local_sufficiency_summary_v1(&state, &request)
        {
            Ok(_) => panic!("a stale binding must be rejected before publication"),
            Err(error) => error,
        };
        assert_eq!(
            error,
            crate::ASSIGNED_LOCAL_SUMMARY_PROJECT_CHANGED_MESSAGE_V1
        );
        assert_eq!(
            state.1.assigned_local_summary_generation(),
            generation_before,
            "a rejected binding owns no cancellation generation"
        );
        assert!(
            !state.native_pose_worker_is_busy(),
            "a rejected binding owns no worker permit"
        );
    }

    #[test]
    fn assigned_local_input_count_limits_are_inclusive_and_one_over_is_rejected() {
        let project = crate::initial_project_state();
        let mut paper = project.editor.paper().clone();
        let mut pattern = project.editor.pattern().clone();
        let vertex = pattern.vertices.first().expect("vertex").clone();
        let edge = pattern.edges.first().expect("edge").clone();
        let boundary_vertex = paper.boundary_vertices[0];
        pattern.vertices.resize(
            crate::MAX_ASSIGNED_LOCAL_SUMMARY_VERTICES_V1,
            vertex.clone(),
        );
        pattern
            .edges
            .resize(crate::MAX_ASSIGNED_LOCAL_SUMMARY_EDGES_V1, edge.clone());
        paper.boundary_vertices.resize(
            crate::MAX_ASSIGNED_LOCAL_SUMMARY_BOUNDARY_VERTICES_V1,
            boundary_vertex,
        );
        assert!(
            crate::ensure_assigned_local_sufficiency_input_limits_v1(&paper, &pattern).is_ok(),
            "all exact count ceilings are admitted"
        );

        pattern.vertices.push(vertex);
        assert_eq!(
            crate::ensure_assigned_local_sufficiency_input_limits_v1(&paper, &pattern),
            Err(crate::ASSIGNED_LOCAL_SUMMARY_INPUT_LIMIT_MESSAGE_V1.to_owned())
        );
        pattern.vertices.pop();
        pattern.edges.push(edge);
        assert_eq!(
            crate::ensure_assigned_local_sufficiency_input_limits_v1(&paper, &pattern),
            Err(crate::ASSIGNED_LOCAL_SUMMARY_INPUT_LIMIT_MESSAGE_V1.to_owned())
        );
        pattern.edges.pop();
        paper.boundary_vertices.push(boundary_vertex);
        assert_eq!(
            crate::ensure_assigned_local_sufficiency_input_limits_v1(&paper, &pattern),
            Err(crate::ASSIGNED_LOCAL_SUMMARY_INPUT_LIMIT_MESSAGE_V1.to_owned())
        );
    }

    #[test]
    fn summary_conversion_rejects_one_over_duplicate_and_inconsistent_proofs() {
        let vertex = ori_domain::VertexId::new();
        let indeterminate = ori_topology::AssignedLocalSufficiencyV1::Indeterminate {
            vertex,
            reason: ori_topology::AssignedLocalSufficiencyReasonV1::ResourceLimit,
        };
        let valid = ori_topology::AssignedLocalSufficiencyBatchV1 {
            vertices: vec![
                indeterminate.clone(),
                ori_topology::AssignedLocalSufficiencyV1::Indeterminate {
                    vertex: ori_domain::VertexId::new(),
                    reason: ori_topology::AssignedLocalSufficiencyReasonV1::Cancelled,
                },
            ],
            total_reduction_steps: 0,
        };
        let (converted, reductions) =
            crate::convert_assigned_local_sufficiency_summary_batch_v1(valid)
                .expect("bounded unique batch");
        assert_eq!(converted.len(), 2);
        assert_eq!(reductions, 0);

        let one_over = ori_topology::AssignedLocalSufficiencyBatchV1 {
            vertices: vec![
                indeterminate.clone();
                crate::MAX_ASSIGNED_LOCAL_SUMMARY_VERTICES_V1 + 1
            ],
            total_reduction_steps: 0,
        };
        assert!(matches!(
            crate::convert_assigned_local_sufficiency_summary_batch_v1(one_over),
            Err(crate::ASSIGNED_LOCAL_SUMMARY_OUTPUT_LIMIT_MESSAGE_V1)
        ));

        let duplicate = ori_topology::AssignedLocalSufficiencyBatchV1 {
            vertices: vec![indeterminate.clone(), indeterminate],
            total_reduction_steps: 0,
        };
        assert!(matches!(
            crate::convert_assigned_local_sufficiency_summary_batch_v1(duplicate),
            Err(crate::ASSIGNED_LOCAL_SUMMARY_OUTPUT_LIMIT_MESSAGE_V1)
        ));

        let wrong_model = ori_topology::AssignedLocalSufficiencyBatchV1 {
            vertices: vec![ori_topology::AssignedLocalSufficiencyV1::Proven {
                model_id: "wrong_assigned_local_model",
                vertex: ori_domain::VertexId::new(),
                reduction_steps: 0,
                reductions: Vec::new(),
            }],
            total_reduction_steps: 0,
        };
        assert!(matches!(
            crate::convert_assigned_local_sufficiency_summary_batch_v1(wrong_model),
            Err(crate::ASSIGNED_LOCAL_SUMMARY_OUTPUT_LIMIT_MESSAGE_V1)
        ));

        let inconsistent_total = ori_topology::AssignedLocalSufficiencyBatchV1 {
            vertices: vec![ori_topology::AssignedLocalSufficiencyV1::Proven {
                model_id: ori_topology::ASSIGNED_LOCAL_SUFFICIENCY_MODEL_ID_V1,
                vertex: ori_domain::VertexId::new(),
                reduction_steps: 0,
                reductions: Vec::new(),
            }],
            total_reduction_steps: 1,
        };
        assert!(matches!(
            crate::convert_assigned_local_sufficiency_summary_batch_v1(inconsistent_total),
            Err(crate::ASSIGNED_LOCAL_SUMMARY_OUTPUT_LIMIT_MESSAGE_V1)
        ));
    }
}
