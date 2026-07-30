//! Private native boundary for geometric-constraint preflight analysis.
//!
//! This module owns the single-flight worker gate, cancellation/deadline runtime,
//! bounded direct-MUS classification, and read-only Tauri analysis commands.

use super::*;

#[cfg(test)]
use ori_core::{BoundedDirectMusV1, find_bounded_direct_mus_with_observer_v1};

mod semantic_mus;

use semantic_mus::{GeometricConstraintSemanticMusResult, analyze_semantic_direct_conflict_with};
#[cfg(test)]
use semantic_mus::{
    GeometricConstraintSemanticMusUnknownReason, map_semantic_direct_conflict_result,
};

pub(super) const GEOMETRIC_CONSTRAINT_ANALYSIS_BUSY_MESSAGE: &str =
    "geometric-constraint analysis is already in progress";
pub(super) const GEOMETRIC_CONSTRAINT_ANALYSIS_FAILED_MESSAGE: &str =
    "geometric-constraint analysis did not complete";
const GEOMETRIC_CONSTRAINT_ANALYSIS_DEADLINE: std::time::Duration =
    std::time::Duration::from_secs(2);

fn geometric_constraint_analysis_task_error<T>(_: T) -> String {
    GEOMETRIC_CONSTRAINT_ANALYSIS_FAILED_MESSAGE.to_owned()
}

/// Process-wide gate for bounded geometric-constraint preflight work.
///
/// The permit owns the active request's cancellation token and moves into
/// `spawn_blocking`, so abandoning an awaiting WebView request cannot release
/// the gate before the native worker actually exits. The same mutex publishes
/// the active binding/generation and a bounded early-cancel ledger atomically,
/// closing both cancel-after-acquire and cancel-before-acquire races without
/// allowing an old generation to stop a replacement worker. A cancellation
/// for a different generation is retained even while one worker is active,
/// because that queued analyze future may not have reached its first native
/// poll yet; frontend generation IDs are unique per work item, and the ledger
/// bounds abandoned entries.
#[derive(Default)]
struct GeometricConstraintWorkerShared {
    state: Mutex<GeometricConstraintWorkerState>,
}

#[derive(Default)]
struct GeometricConstraintWorkerState {
    active: Option<GeometricConstraintWorkerSlot>,
    pre_cancelled: VecDeque<GeometricConstraintWorkerKey>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct GeometricConstraintWorkerKey {
    binding: GeometricConstraintAnalysisBinding,
    request_generation_id: ProjectId,
}

struct GeometricConstraintWorkerSlot {
    key: GeometricConstraintWorkerKey,
    cancellation: Arc<AtomicBool>,
}

pub(super) const MAX_GEOMETRIC_CONSTRAINT_PRE_CANCELLED_REQUESTS: usize = 64;

#[derive(Clone, Default)]
pub(super) struct GeometricConstraintWorkerGate(Arc<GeometricConstraintWorkerShared>);

impl GeometricConstraintWorkerGate {
    pub(super) fn try_acquire(
        &self,
        binding: GeometricConstraintAnalysisBinding,
        request_generation_id: ProjectId,
    ) -> Option<GeometricConstraintWorkerPermit> {
        let key = GeometricConstraintWorkerKey {
            binding,
            request_generation_id,
        };
        let mut state = self
            .0
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.active.is_some() {
            return None;
        }
        let pre_cancelled = state
            .pre_cancelled
            .iter()
            .position(|candidate| *candidate == key)
            .and_then(|index| state.pre_cancelled.remove(index))
            .is_some();
        let cancellation = Arc::new(AtomicBool::new(pre_cancelled));
        state.active = Some(GeometricConstraintWorkerSlot {
            key,
            cancellation: Arc::clone(&cancellation),
        });
        Some(GeometricConstraintWorkerPermit {
            shared: Arc::clone(&self.0),
            cancellation,
        })
    }

    pub(super) fn cancel(
        &self,
        binding: GeometricConstraintAnalysisBinding,
        request_generation_id: ProjectId,
    ) -> bool {
        let key = GeometricConstraintWorkerKey {
            binding,
            request_generation_id,
        };
        let mut state = self
            .0
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(slot) = state.active.as_ref().filter(|slot| slot.key == key) {
            slot.cancellation.store(true, Ordering::Release);
            return true;
        }
        let active_worker_present = state.active.is_some();
        if state
            .pre_cancelled
            .iter()
            .all(|candidate| *candidate != key)
        {
            if state.pre_cancelled.len() >= MAX_GEOMETRIC_CONSTRAINT_PRE_CANCELLED_REQUESTS {
                state.pre_cancelled.pop_front();
            }
            state.pre_cancelled.push_back(key);
        }
        !active_worker_present
    }

    #[cfg(test)]
    pub(super) fn is_busy(&self) -> bool {
        self.0
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .active
            .is_some()
    }

    #[cfg(test)]
    pub(super) fn pre_cancelled_count(&self) -> usize {
        self.0
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .pre_cancelled
            .len()
    }
}

pub(super) struct GeometricConstraintWorkerPermit {
    shared: Arc<GeometricConstraintWorkerShared>,
    pub(super) cancellation: Arc<AtomicBool>,
}

impl GeometricConstraintWorkerPermit {
    pub(super) fn cancellation(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.cancellation)
    }
}

impl Drop for GeometricConstraintWorkerPermit {
    fn drop(&mut self) {
        let mut state = self
            .shared
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state
            .active
            .as_ref()
            .is_some_and(|slot| Arc::ptr_eq(&slot.cancellation, &self.cancellation))
        {
            state.active = None;
        }
        debug_assert!(
            state.active.is_none(),
            "geometric constraint worker permit mismatch"
        );
    }
}

impl AppState {
    fn try_acquire_geometric_constraint_worker(
        &self,
        binding: GeometricConstraintAnalysisBinding,
        request_generation_id: ProjectId,
    ) -> Option<GeometricConstraintWorkerPermit> {
        self.2.try_acquire(binding, request_generation_id)
    }

    pub(super) fn cancel_geometric_constraint_worker(
        &self,
        binding: GeometricConstraintAnalysisBinding,
        request_generation_id: ProjectId,
    ) -> bool {
        self.2.cancel(binding, request_generation_id)
    }

    #[cfg(test)]
    pub(super) fn geometric_constraint_worker_is_busy(&self) -> bool {
        self.2.is_busy()
    }
}

#[derive(Debug, Serialize)]
pub(super) struct GeometricConstraintPreflightResponse {
    pub(super) project_instance_id: ProjectId,
    pub(super) project_id: ProjectId,
    pub(super) revision: u64,
    pub(super) result: GeometricConstraintPreflightResult,
    /// Present on every native response. `None` serializes as an explicit
    /// `null` for non-direct outcomes; direct-conflict analysis always
    /// publishes either a certified or fail-closed semantic-MUS DTO.
    pub(super) semantic_mus: Option<GeometricConstraintSemanticMusResult>,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct GeometricConstraintAnalysisOutcome {
    pub(super) result: GeometricConstraintPreflightResult,
    pub(super) semantic_mus: Option<GeometricConstraintSemanticMusResult>,
}

impl From<GeometricConstraintPreflightResult> for GeometricConstraintAnalysisOutcome {
    fn from(result: GeometricConstraintPreflightResult) -> Self {
        Self {
            result,
            semantic_mus: None,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub(super) enum GeometricConstraintPreflightResult {
    DirectConflict {
        conflicts: Vec<DirectConstraintConflictV1>,
        bounded_direct_mus: BoundedDirectMusResult,
    },
    ProvenSatisfiable {
        model_id: &'static str,
        transcendental_model_id: &'static str,
        evidence_kind: GeometricConstraintSatisfactionEvidenceKind,
        constraint_count: usize,
        equation_count: usize,
        authorizes_project_mutation: bool,
        replayable_across_runtimes: bool,
    },
    NoDirectConflict,
    Unknown {
        reason: GeometricConstraintUnknownReason,
        unchecked_constraint_ids: Vec<ConstraintId>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum GeometricConstraintSatisfactionEvidenceKind {
    CurrentAssignment,
    DetachedConstructedAssignment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum GeometricConstraintUnknownReason {
    WorkLimitExceeded,
    ConstraintLimitExceeded,
    StorageLimitExceeded,
    Cancelled,
    DeadlineReached,
    SolverRequiredConstraintKinds,
    InvalidDocumentOrGeometry,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub(super) enum BoundedDirectMusResult {
    ProvenUnsatisfiable {
        constraint_ids: Vec<ConstraintId>,
        oracle_calls: usize,
    },
    Unknown {
        reason: BoundedDirectMusUnknownReason,
        oracle_calls: usize,
        max_constraints: usize,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum BoundedDirectMusUnknownReason {
    ConstraintLimitExceeded,
    OracleIncomplete,
    Cancelled,
    DeadlineReached,
}

#[tauri::command]
pub(super) async fn analyze_geometric_constraints(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    request_generation_id: ProjectId,
) -> Result<GeometricConstraintPreflightResponse, String> {
    if request_generation_id.canonical_bytes() == [0; 16] {
        return Err(GEOMETRIC_CONSTRAINT_ANALYSIS_FAILED_MESSAGE.to_owned());
    }
    analyze_geometric_constraints_with_outcome_worker(
        &state,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        request_generation_id,
        |pattern, document, runtime| {
            analyze_geometric_constraint_document_outcome_with_observer(
                &pattern,
                &document,
                &mut GeometricConstraintAnalysisObserver::new(runtime),
            )
            .map_err(|()| GEOMETRIC_CONSTRAINT_ANALYSIS_FAILED_MESSAGE.to_owned())
        },
    )
    .await
}

#[tauri::command]
pub(super) fn cancel_geometric_constraint_analysis(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    request_generation_id: ProjectId,
) -> bool {
    if request_generation_id.canonical_bytes() == [0; 16] {
        return false;
    }
    state.cancel_geometric_constraint_worker(
        GeometricConstraintAnalysisBinding {
            project_instance_id: expected_project_instance_id,
            project_id: expected_project_id,
            revision: expected_revision,
        },
        request_generation_id,
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct GeometricConstraintAnalysisBinding {
    pub(super) project_instance_id: ProjectId,
    pub(super) project_id: ProjectId,
    pub(super) revision: u64,
}

#[derive(Clone)]
pub(super) struct GeometricConstraintAnalysisRuntime {
    pub(super) cancellation: Arc<AtomicBool>,
    pub(super) deadline: std::time::Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GeometricConstraintAnalysisStop {
    Cancelled,
    DeadlineReached,
}

pub(super) struct GeometricConstraintAnalysisObserver {
    runtime: GeometricConstraintAnalysisRuntime,
    stop: Option<GeometricConstraintAnalysisStop>,
}

impl GeometricConstraintAnalysisObserver {
    pub(super) fn new(runtime: GeometricConstraintAnalysisRuntime) -> Self {
        Self {
            runtime,
            stop: None,
        }
    }

    fn checkpoint(&mut self) -> Option<GeometricConstraintAnalysisStop> {
        let stop = if self.runtime.cancellation.load(Ordering::Acquire) {
            Some(GeometricConstraintAnalysisStop::Cancelled)
        } else if std::time::Instant::now() >= self.runtime.deadline {
            Some(GeometricConstraintAnalysisStop::DeadlineReached)
        } else {
            None
        };
        self.stop = self.stop.or(stop);
        self.stop
    }
}

impl GeometricConstraintPreflightObserverV1 for GeometricConstraintAnalysisObserver {
    fn checkpoint(&mut self) -> GeometricConstraintPreflightObserverControlV1 {
        match self.checkpoint() {
            None => GeometricConstraintPreflightObserverControlV1::Continue,
            Some(GeometricConstraintAnalysisStop::Cancelled) => {
                GeometricConstraintPreflightObserverControlV1::Cancelled
            }
            Some(GeometricConstraintAnalysisStop::DeadlineReached) => {
                GeometricConstraintPreflightObserverControlV1::DeadlineReached
            }
        }
    }
}

impl BoundedDirectMusObserverV1 for GeometricConstraintAnalysisObserver {
    fn should_cancel(&mut self, _completed_oracle_calls: usize) -> bool {
        self.checkpoint().is_some()
    }
}

impl ori_core::BoundedSemanticMusObserverV1 for GeometricConstraintAnalysisObserver {
    fn checkpoint(
        &mut self,
        _progress: ori_core::BoundedSemanticMusProgressV1,
    ) -> ori_core::BoundedSemanticMusObserverControlV1 {
        match self.checkpoint() {
            None => ori_core::BoundedSemanticMusObserverControlV1::Continue,
            Some(GeometricConstraintAnalysisStop::Cancelled) => {
                ori_core::BoundedSemanticMusObserverControlV1::Cancelled
            }
            Some(GeometricConstraintAnalysisStop::DeadlineReached) => {
                ori_core::BoundedSemanticMusObserverControlV1::DeadlineReached
            }
        }
    }
}

struct GeometricConstraintAnalysisInput {
    binding: GeometricConstraintAnalysisBinding,
    pattern: CreasePattern,
    document: GeometricConstraintDocumentV1,
}

#[cfg(test)]
pub(super) async fn analyze_geometric_constraints_with_worker<F>(
    state: &AppState,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    request_generation_id: ProjectId,
    worker: F,
) -> Result<GeometricConstraintPreflightResponse, String>
where
    F: FnOnce(
            CreasePattern,
            GeometricConstraintDocumentV1,
            GeometricConstraintAnalysisRuntime,
        ) -> Result<GeometricConstraintPreflightResult, String>
        + Send
        + 'static,
{
    analyze_geometric_constraints_with_outcome_worker(
        state,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        request_generation_id,
        move |pattern, document, runtime| {
            worker(pattern, document, runtime).map(GeometricConstraintAnalysisOutcome::from)
        },
    )
    .await
}

async fn analyze_geometric_constraints_with_outcome_worker<F>(
    state: &AppState,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    request_generation_id: ProjectId,
    worker: F,
) -> Result<GeometricConstraintPreflightResponse, String>
where
    F: FnOnce(
            CreasePattern,
            GeometricConstraintDocumentV1,
            GeometricConstraintAnalysisRuntime,
        ) -> Result<GeometricConstraintAnalysisOutcome, String>
        + Send
        + 'static,
{
    let permit = state
        .try_acquire_geometric_constraint_worker(
            GeometricConstraintAnalysisBinding {
                project_instance_id: expected_project_instance_id,
                project_id: expected_project_id,
                revision: expected_revision,
            },
            request_generation_id,
        )
        .ok_or_else(|| GEOMETRIC_CONSTRAINT_ANALYSIS_BUSY_MESSAGE.to_owned())?;
    let runtime = GeometricConstraintAnalysisRuntime {
        cancellation: permit.cancellation(),
        deadline: std::time::Instant::now()
            .checked_add(GEOMETRIC_CONSTRAINT_ANALYSIS_DEADLINE)
            .ok_or_else(|| GEOMETRIC_CONSTRAINT_ANALYSIS_FAILED_MESSAGE.to_owned())?,
    };
    let input = {
        let project = lock_project(state)?;
        capture_geometric_constraint_analysis(
            &project,
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
        )?
    };
    let binding = input.binding;
    let result = tauri::async_runtime::spawn_blocking(move || {
        let _permit = permit;
        worker(input.pattern, input.document, runtime)
    })
    .await
    .map_err(geometric_constraint_analysis_task_error)?
    .map_err(geometric_constraint_analysis_task_error)?;

    let project = lock_project(state)?;
    finish_geometric_constraint_analysis(&project, binding, result)
}

fn capture_geometric_constraint_analysis(
    project: &ProjectState,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
) -> Result<GeometricConstraintAnalysisInput, String> {
    ensure_project_expectation(
        project,
        ProjectExpectation::new(
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
        ),
    )?;
    Ok(GeometricConstraintAnalysisInput {
        binding: GeometricConstraintAnalysisBinding {
            project_instance_id: project.instance_id,
            project_id: project.project_id,
            revision: project.editor.revision(),
        },
        pattern: project.editor.pattern().clone(),
        document: project.editor.geometric_constraints().clone(),
    })
}

fn finish_geometric_constraint_analysis(
    project: &ProjectState,
    binding: GeometricConstraintAnalysisBinding,
    outcome: GeometricConstraintAnalysisOutcome,
) -> Result<GeometricConstraintPreflightResponse, String> {
    ensure_project_expectation(
        project,
        ProjectExpectation::new(
            binding.project_instance_id,
            binding.project_id,
            binding.revision,
        ),
    )?;
    Ok(GeometricConstraintPreflightResponse {
        project_instance_id: binding.project_instance_id,
        project_id: binding.project_id,
        revision: binding.revision,
        result: outcome.result,
        semantic_mus: outcome.semantic_mus,
    })
}

#[cfg(test)]
pub(super) fn analyze_geometric_constraint_document(
    pattern: &CreasePattern,
    document: &GeometricConstraintDocumentV1,
) -> GeometricConstraintPreflightResult {
    let runtime = GeometricConstraintAnalysisRuntime {
        cancellation: Arc::new(AtomicBool::new(false)),
        deadline: std::time::Instant::now()
            .checked_add(std::time::Duration::from_secs(3_600))
            .expect("one-hour test/default observer deadline must be representable"),
    };
    analyze_geometric_constraint_document_with_observer(
        pattern,
        document,
        &mut GeometricConstraintAnalysisObserver::new(runtime),
    )
}

#[cfg(test)]
pub(super) fn analyze_geometric_constraint_document_with_observer(
    pattern: &CreasePattern,
    document: &GeometricConstraintDocumentV1,
    observer: &mut GeometricConstraintAnalysisObserver,
) -> GeometricConstraintPreflightResult {
    analyze_geometric_constraint_document_outcome_with_observer(pattern, document, observer)
        .map_or_else(
            |()| invalid_geometric_constraint_analysis_result(document),
            |outcome| outcome.result,
        )
}

fn analyze_geometric_constraint_document_outcome_with_observer(
    pattern: &CreasePattern,
    document: &GeometricConstraintDocumentV1,
    observer: &mut GeometricConstraintAnalysisObserver,
) -> Result<GeometricConstraintAnalysisOutcome, ()> {
    if let Some(stop) = observer.checkpoint() {
        return Ok(stopped_geometric_constraint_analysis_result(document, stop).into());
    }
    if document.schema_version == ori_domain::GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1
        && document.is_empty()
    {
        return Ok(GeometricConstraintPreflightResult::NoDirectConflict.into());
    }

    let Ok(prepared) =
        prepare_geometric_constraints_v1(pattern, document, GeometricConstraintLimitsV1::default())
    else {
        return Ok(invalid_geometric_constraint_analysis_result(document).into());
    };

    let preflight = prepared.preflight_with_observer(observer);
    if let Some(stop) = observer.checkpoint() {
        return Ok(stopped_geometric_constraint_analysis_result(document, stop).into());
    }
    if !matches!(preflight, ConstraintPreflightV1::DirectConflict { .. })
        && let Ok(Some(certificate)) =
            certify_binary64_exact_geometric_constraint_satisfaction_v1(pattern, document)
    {
        return Ok(
            finish_exact_geometric_constraint_satisfaction(document, observer, certificate).into(),
        );
    }
    if let Some(stop) = observer.checkpoint() {
        return Ok(stopped_geometric_constraint_analysis_result(document, stop).into());
    }
    let constructive_assignment =
        (!matches!(preflight, ConstraintPreflightV1::DirectConflict { .. }))
            .then(|| match document.constraints.len() {
                1 => ori_core::construct_single_constraint_exact_assignment_v1(pattern, document),
                count
                    if (2..=ori_core::MAX_BOUNDED_SINGLETON_COMPOSITION_CONSTRAINTS_V1)
                        .contains(&count) =>
                {
                    ori_core::construct_bounded_singleton_composition_exact_assignment_v1(
                        pattern, document,
                    )
                }
                _ => None,
            })
            .flatten();
    let constructive_assignment =
        match recheck_after_constructive_assignment_attempt(observer, constructive_assignment) {
            Ok(assignment) => assignment,
            Err(stop) => {
                return Ok(stopped_geometric_constraint_analysis_result(document, stop).into());
            }
        };
    if let Some(assignment) = constructive_assignment {
        // This is an observation-only SAT witness. The constructed candidate
        // never crosses the native DTO boundary and cannot authorize project
        // mutation; only its independently re-certified full-document
        // certificate is reduced to ProvenSatisfiable with an explicit
        // detached-construction evidence kind.
        return Ok(finish_constructed_exact_geometric_constraint_satisfaction(
            document,
            observer,
            assignment.certificate(),
        )
        .into());
    }

    let outcome = match preflight {
        ConstraintPreflightV1::DirectConflict { conflicts } => {
            let (bounded_direct_mus, semantic_mus) = analyze_semantic_direct_conflict_with(
                &prepared,
                observer,
                |prepared, observer| {
                    ori_core::certify_bounded_current_runtime_semantic_mus_with_observer_v1(
                        prepared,
                        ori_core::BoundedSemanticMusLimitsV1::default(),
                        observer,
                    )
                },
            )?;
            GeometricConstraintAnalysisOutcome {
                result: GeometricConstraintPreflightResult::DirectConflict {
                    conflicts,
                    bounded_direct_mus,
                },
                semantic_mus: Some(semantic_mus),
            }
        }
        ConstraintPreflightV1::NoDirectConflict => {
            GeometricConstraintPreflightResult::NoDirectConflict.into()
        }
        ConstraintPreflightV1::Unknown {
            reason,
            unchecked_constraint_ids,
        } => GeometricConstraintPreflightResult::Unknown {
            reason: match reason {
                GeometricConstraintUnknownReasonV1::WorkLimitExceeded => {
                    GeometricConstraintUnknownReason::WorkLimitExceeded
                }
                GeometricConstraintUnknownReasonV1::ConstraintLimitExceeded => {
                    GeometricConstraintUnknownReason::ConstraintLimitExceeded
                }
                GeometricConstraintUnknownReasonV1::StorageLimitExceeded => {
                    GeometricConstraintUnknownReason::StorageLimitExceeded
                }
                GeometricConstraintUnknownReasonV1::Cancelled => {
                    GeometricConstraintUnknownReason::Cancelled
                }
                GeometricConstraintUnknownReasonV1::DeadlineReached => {
                    GeometricConstraintUnknownReason::DeadlineReached
                }
                GeometricConstraintUnknownReasonV1::SolverRequiredConstraintKinds => {
                    GeometricConstraintUnknownReason::SolverRequiredConstraintKinds
                }
            },
            unchecked_constraint_ids,
        }
        .into(),
    };
    Ok(outcome)
}

fn recheck_after_constructive_assignment_attempt<T>(
    observer: &mut GeometricConstraintAnalysisObserver,
    attempt: T,
) -> Result<T, GeometricConstraintAnalysisStop> {
    // Construction can perform up to the independently bounded sixteen-record
    // proof envelope without consulting the native runtime observer. Recheck
    // unconditionally after either `Some` or `None` so cancellation/deadline
    // that arrived during construction cannot publish a witness or fall
    // through to a stale clear/unknown result.
    match observer.checkpoint() {
        Some(stop) => Err(stop),
        None => Ok(attempt),
    }
}

pub(super) fn finish_exact_geometric_constraint_satisfaction(
    document: &GeometricConstraintDocumentV1,
    observer: &mut GeometricConstraintAnalysisObserver,
    certificate: ori_core::Binary64ExactConstraintSatisfactionV1,
) -> GeometricConstraintPreflightResult {
    finish_exact_geometric_constraint_satisfaction_with_evidence(
        document,
        observer,
        certificate,
        GeometricConstraintSatisfactionEvidenceKind::CurrentAssignment,
    )
}

fn finish_constructed_exact_geometric_constraint_satisfaction(
    document: &GeometricConstraintDocumentV1,
    observer: &mut GeometricConstraintAnalysisObserver,
    certificate: ori_core::Binary64ExactConstraintSatisfactionV1,
) -> GeometricConstraintPreflightResult {
    finish_exact_geometric_constraint_satisfaction_with_evidence(
        document,
        observer,
        certificate,
        GeometricConstraintSatisfactionEvidenceKind::DetachedConstructedAssignment,
    )
}

fn finish_exact_geometric_constraint_satisfaction_with_evidence(
    document: &GeometricConstraintDocumentV1,
    observer: &mut GeometricConstraintAnalysisObserver,
    certificate: ori_core::Binary64ExactConstraintSatisfactionV1,
    evidence_kind: GeometricConstraintSatisfactionEvidenceKind,
) -> GeometricConstraintPreflightResult {
    if let Some(stop) = observer.checkpoint() {
        return stopped_geometric_constraint_analysis_result(document, stop);
    }
    let constraint_count = certificate.constraint_count();
    let equation_count = certificate.equation_count();
    let maximum_equation_count = constraint_count.checked_mul(2);
    if certificate.model_id() != GEOMETRIC_CONSTRAINT_CURRENT_RUNTIME_EXACT_SATISFACTION_MODEL_ID_V1
        || certificate.transcendental_model_id()
            != ori_numeric::DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1
        || certificate.authorizes_project_mutation()
        || certificate.replayable_across_runtimes()
            != ori_numeric::deterministic_transcendental_model_supported_v1()
        || constraint_count != document.constraints.len()
        || constraint_count == 0
        || equation_count < constraint_count
        || maximum_equation_count.is_none_or(|maximum| equation_count > maximum)
    {
        return invalid_geometric_constraint_analysis_result(document);
    }
    GeometricConstraintPreflightResult::ProvenSatisfiable {
        model_id: certificate.model_id(),
        transcendental_model_id: certificate.transcendental_model_id(),
        evidence_kind,
        constraint_count,
        equation_count,
        authorizes_project_mutation: certificate.authorizes_project_mutation(),
        replayable_across_runtimes: certificate.replayable_across_runtimes(),
    }
}

fn invalid_geometric_constraint_analysis_result(
    document: &GeometricConstraintDocumentV1,
) -> GeometricConstraintPreflightResult {
    let mut unchecked_constraint_ids = document
        .constraints
        .iter()
        .map(|record| record.id)
        .collect::<Vec<_>>();
    unchecked_constraint_ids.sort_unstable_by_key(ConstraintId::canonical_bytes);
    unchecked_constraint_ids.dedup();
    GeometricConstraintPreflightResult::Unknown {
        reason: GeometricConstraintUnknownReason::InvalidDocumentOrGeometry,
        unchecked_constraint_ids,
    }
}

fn stopped_geometric_constraint_analysis_result(
    document: &GeometricConstraintDocumentV1,
    stop: GeometricConstraintAnalysisStop,
) -> GeometricConstraintPreflightResult {
    let mut unchecked_constraint_ids = document
        .constraints
        .iter()
        .map(|record| record.id)
        .collect::<Vec<_>>();
    unchecked_constraint_ids.sort_unstable_by_key(ConstraintId::canonical_bytes);
    unchecked_constraint_ids.dedup();
    GeometricConstraintPreflightResult::Unknown {
        reason: match stop {
            GeometricConstraintAnalysisStop::Cancelled => {
                GeometricConstraintUnknownReason::Cancelled
            }
            GeometricConstraintAnalysisStop::DeadlineReached => {
                GeometricConstraintUnknownReason::DeadlineReached
            }
        },
        unchecked_constraint_ids,
    }
}

#[cfg(test)]
pub(super) fn analyze_bounded_direct_mus_with_observer(
    prepared: &ori_core::GeometricConstraintSetV1<'_>,
    observer: &mut GeometricConstraintAnalysisObserver,
) -> BoundedDirectMusResult {
    if prepared.constraints().len() > MAX_BOUNDED_DIRECT_MUS_CONSTRAINTS_V1 {
        return BoundedDirectMusResult::Unknown {
            reason: BoundedDirectMusUnknownReason::ConstraintLimitExceeded,
            oracle_calls: 0,
            max_constraints: MAX_BOUNDED_DIRECT_MUS_CONSTRAINTS_V1,
        };
    }
    match find_bounded_direct_mus_with_observer_v1(prepared, observer) {
        BoundedDirectMusV1::ProvenUnsatisfiable {
            constraint_ids,
            oracle_calls,
        } => BoundedDirectMusResult::ProvenUnsatisfiable {
            constraint_ids,
            oracle_calls,
        },
        BoundedDirectMusV1::Unknown { oracle_calls } => BoundedDirectMusResult::Unknown {
            reason: match observer.stop {
                Some(GeometricConstraintAnalysisStop::Cancelled) => {
                    BoundedDirectMusUnknownReason::Cancelled
                }
                Some(GeometricConstraintAnalysisStop::DeadlineReached) => {
                    BoundedDirectMusUnknownReason::DeadlineReached
                }
                None => BoundedDirectMusUnknownReason::OracleIncomplete,
            },
            oracle_calls,
            max_constraints: MAX_BOUNDED_DIRECT_MUS_CONSTRAINTS_V1,
        },
    }
}

#[cfg(test)]
#[path = "geometric_constraint_analysis_semantic_tests.rs"]
mod semantic_mus_tests;

#[cfg(test)]
#[path = "geometric_constraint_analysis/singleton_constructive_sat_tests.rs"]
mod singleton_constructive_sat_tests;
