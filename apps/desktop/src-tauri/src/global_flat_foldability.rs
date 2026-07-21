use std::{
    collections::{HashMap, HashSet},
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use ori_core::{
    CooperativeAnalysisAbort, CooperativeAnalysisCheckpoint, TopologyAnalysisInput,
    TopologySnapshot, analyze_local_flat_foldability,
    analyze_local_flat_foldability_with_checkpoint,
};
use ori_domain::{CreasePattern, FaceId, Paper, ProjectId};
use ori_foldability::{
    FlatFoldabilityProofIncompleteReason, FlatFoldabilityResource,
    GLOBAL_FLAT_FOLDABILITY_MODEL_ID, GlobalFlatFoldabilityCheckpoint,
    GlobalFlatFoldabilityExecutionError, GlobalFlatFoldabilityImpossibleReason,
    GlobalFlatFoldabilityInput, GlobalFlatFoldabilityLimits, GlobalFlatFoldabilityObserver,
    GlobalFlatFoldabilityOutcome, GlobalFlatFoldabilityPhase, GlobalFlatFoldabilityProgress,
    GlobalFlatFoldabilityReport, GlobalFlatFoldabilityUnknownReason,
    GlobalFlatFoldabilityWorkCounts, LAYER_ORDER_MODEL_ID, LayerFace, LayerOrderProvenance,
    LayerOrderSnapshot, analyze_global_flat_foldability,
    analyze_global_flat_foldability_with_observer,
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};

use super::stacked_fold_transaction::CurrentLayerEvidence;
use super::{AppState, ProjectState, lock_project};

pub(super) fn revalidate_archived_flat_layer_evidence(
    project: &ProjectState,
    canonical_snapshot_json: &str,
) -> Result<CurrentLayerEvidence, ()> {
    let layer_order = reanalyze_current_flat_layer_order(project)?;
    if serde_json::to_string(&layer_order).map_err(|_| ())? != canonical_snapshot_json {
        return Err(());
    }
    Ok(CurrentLayerEvidence::CertifiedFlat(layer_order))
}

pub(super) fn reanalyze_current_flat_layer_order(
    project: &ProjectState,
) -> Result<LayerOrderSnapshot, ()> {
    let topology_input = project.editor.topology_analysis_input(project.project_id);
    if topology_input.revision() != project.editor.revision() {
        return Err(());
    }
    let topology = topology_input.analyze();
    let simulation = topology.simulation_snapshot().ok_or(())?;
    let local = analyze_local_flat_foldability(project.editor.paper(), project.editor.pattern());
    let report = analyze_global_flat_foldability(
        GlobalFlatFoldabilityInput::current_with_geometry(
            project.project_id,
            project.editor.paper(),
            project.editor.pattern(),
            simulation,
            &local,
        ),
        native_limits(),
    )
    .map_err(|_| ())?;
    let GlobalFlatFoldabilityOutcome::Possible { layer_order, .. } = report.outcome else {
        return Err(());
    };
    let provenance = layer_order.provenance.source;
    if provenance.identity_namespace != Some(project.project_id)
        || provenance.source_revision != project.editor.revision()
        || provenance.source_fingerprint.is_none_or(|fingerprint| {
            fingerprint.to_hex() != project.editor.fold_model_fingerprint_v1()
        })
    {
        return Err(());
    }
    Ok(*layer_order)
}

const MIN_TIME_LIMIT_MS: u64 = 1_000;
const MAX_TIME_LIMIT_MS: u64 = 300_000;
const MAX_REPORTED_ELAPSED_MS: u64 = 24 * 60 * 60 * 1_000;
const MAX_REPORTED_FACES: u64 = 2_048;
const MAX_REPORTED_OVERLAP_CELLS: u64 = 500_000;
const MAX_REPORTED_CONSTRAINTS: u64 = 5_000_000;
const MAX_REPORTED_WORK: u64 = 10_000_000;
const MAX_REPORTED_PROOF_FACES: usize = 20;

const PHASE_CAPTURING: u8 = 0;
const PHASE_VALIDATING_LOCAL_CONDITIONS: u8 = 1;
const PHASE_BUILDING_FLAT_EMBEDDING: u8 = 2;
const PHASE_BUILDING_OVERLAP_ARRANGEMENT: u8 = 3;
const PHASE_BUILDING_CONSTRAINTS: u8 = 4;
const PHASE_PROPAGATING: u8 = 5;
const PHASE_SEARCHING: u8 = 6;
const PHASE_VERIFYING_CERTIFICATE: u8 = 7;
const PHASE_COMPLETED: u8 = 8;

const PROOF_MODEL_AUTHORITY_ID: &str = "convex_faces_facewise_v1";
const LAYER_MODEL_AUTHORITY_ID: &str = "facewise_layer_order_v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(transparent)]
pub(super) struct GlobalFlatFoldabilityJobId(ProjectId);

impl GlobalFlatFoldabilityJobId {
    fn new() -> Self {
        Self(ProjectId::new())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum GlobalFlatFoldabilityErrorCategory {
    InvalidRequest,
    SnapshotUnavailable,
    WorkerUnavailable,
    ResultUnavailable,
    InternalFailure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(super) struct GlobalFlatFoldabilityCommandError {
    category: GlobalFlatFoldabilityErrorCategory,
}

impl GlobalFlatFoldabilityCommandError {
    const fn new(category: GlobalFlatFoldabilityErrorCategory) -> Self {
        Self { category }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum GlobalFlatFoldabilityModelDto {
    ConvexFacesFacewiseV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum GlobalFlatFoldabilityPhaseDto {
    Capturing,
    ValidatingLocalConditions,
    BuildingFlatEmbedding,
    BuildingOverlapArrangement,
    BuildingConstraints,
    Propagating,
    Searching,
    VerifyingCertificate,
    Completed,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub(super) struct GlobalFlatFoldabilityCountsDto {
    face_count: u64,
    overlap_cell_count: u64,
    constraint_count: u64,
    search_node_count: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(super) struct GlobalFlatFoldabilityProgressDto {
    model_id: GlobalFlatFoldabilityModelDto,
    phase: GlobalFlatFoldabilityPhaseDto,
    completed_work: u64,
    total_work: Option<u64>,
    elapsed_ms: u64,
    counts: GlobalFlatFoldabilityCountsDto,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(super) struct GlobalFlatFoldabilitySummaryDto {
    model_id: GlobalFlatFoldabilityModelDto,
    elapsed_ms: u64,
    counts: GlobalFlatFoldabilityCountsDto,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
pub(super) enum GlobalFlatFoldabilityUnknownReasonDto {
    UnsupportedTopology,
    NonConvexFace,
    TimeLimitReached,
    WorkLimitReached,
    ExactNumberLimitReached,
    OverlapArrangementLimitReached,
    ConstraintLimitReached,
    ProofNotCompleted,
    LocalConditionsIndeterminate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
pub(super) enum GlobalFlatFoldabilityProofCategoryDto {
    LocalConditionsViolated,
    InconsistentFlatEmbedding,
    LayerConstraintsContradictory,
    ExhaustiveSearchNoSolution,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum LayerOrderModelDto {
    FacewiseLayerOrderV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(super) struct LayerOrderSummaryDto {
    model_id: LayerOrderModelDto,
    layer_count: u64,
    max_ply: u64,
    reference_face_number: u64,
    layer_view_available: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct GlobalFlatFoldabilityProofDto {
    category: GlobalFlatFoldabilityProofCategoryDto,
    face_numbers: Vec<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "verdict", rename_all = "snake_case")]
pub(super) enum GlobalFlatFoldabilityResultDto {
    Possible {
        summary: GlobalFlatFoldabilitySummaryDto,
        layer_order: LayerOrderSummaryDto,
    },
    Impossible {
        summary: GlobalFlatFoldabilitySummaryDto,
        proof: GlobalFlatFoldabilityProofDto,
    },
    Unknown {
        summary: GlobalFlatFoldabilitySummaryDto,
        reason: GlobalFlatFoldabilityUnknownReasonDto,
    },
}

impl GlobalFlatFoldabilityResultDto {
    fn with_summary(self, summary: GlobalFlatFoldabilitySummaryDto) -> Self {
        match self {
            Self::Possible { layer_order, .. } => Self::Possible {
                summary,
                layer_order,
            },
            Self::Impossible { proof, .. } => Self::Impossible { summary, proof },
            Self::Unknown { reason, .. } => Self::Unknown { summary, reason },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub(super) enum GlobalFlatFoldabilityJobDto {
    Queued {
        cancel_requested: bool,
        progress: GlobalFlatFoldabilityProgressDto,
    },
    Running {
        cancel_requested: bool,
        progress: GlobalFlatFoldabilityProgressDto,
    },
    Completed {
        result: GlobalFlatFoldabilityResultDto,
    },
    Cancelled {
        summary: GlobalFlatFoldabilitySummaryDto,
    },
    Failed {
        summary: GlobalFlatFoldabilitySummaryDto,
        error_category: GlobalFlatFoldabilityErrorCategory,
    },
    Stale {
        summary: GlobalFlatFoldabilitySummaryDto,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct GlobalFlatFoldabilityBeginResponse {
    job_id: GlobalFlatFoldabilityJobId,
    job: GlobalFlatFoldabilityJobDto,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct CurrentLayerOrderViewRequest {
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CurrentLayerOrderViewResponse {
    project_instance_id: ProjectId,
    project_id: ProjectId,
    revision: u64,
    layer_order_generation: u64,
    cells: Vec<CurrentLayerOrderCellDto>,
    read_only: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CurrentLayerOrderCellDto {
    cell_key_sha256: String,
    bottom_to_top_faces: Vec<FaceId>,
    boundary_world: Vec<[f64; 3]>,
}

#[tauri::command]
pub(super) fn get_current_layer_order_view(
    app_state: State<'_, AppState>,
    state: State<'_, GlobalFlatFoldabilityState>,
    request: CurrentLayerOrderViewRequest,
) -> Result<CurrentLayerOrderViewResponse, GlobalFlatFoldabilityCommandError> {
    let project = lock_project(&app_state).map_err(|_| {
        GlobalFlatFoldabilityCommandError::new(GlobalFlatFoldabilityErrorCategory::InternalFailure)
    })?;
    if project.instance_id != request.expected_project_instance_id
        || project.project_id != request.expected_project_id
        || project.editor.revision() != request.expected_revision
    {
        return Err(GlobalFlatFoldabilityCommandError::new(
            GlobalFlatFoldabilityErrorCategory::SnapshotUnavailable,
        ));
    }
    let capability =
        capture_current_layer_order_capability(&state, &project)?.ok_or_else(|| {
            GlobalFlatFoldabilityCommandError::new(
                GlobalFlatFoldabilityErrorCategory::SnapshotUnavailable,
            )
        })?;
    let generation = capability.generation();
    let snapshot = revalidate_current_layer_order_capability(&state, &project, &capability)?
        .ok_or_else(|| {
            GlobalFlatFoldabilityCommandError::new(
                GlobalFlatFoldabilityErrorCategory::SnapshotUnavailable,
            )
        })?;
    let cells = snapshot
        .overlap_cells
        .iter()
        .map(|cell| {
            let boundary_world = cell
                .exact_boundary
                .iter()
                .map(|point| Some([point.x.to_f64()?, 0.0, -point.y.to_f64()?]))
                .collect::<Option<Vec<_>>>()
                .ok_or_else(|| {
                    GlobalFlatFoldabilityCommandError::new(
                        GlobalFlatFoldabilityErrorCategory::InternalFailure,
                    )
                })?;
            Ok(CurrentLayerOrderCellDto {
                cell_key_sha256: cell
                    .cell_key
                    .0
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect(),
                bottom_to_top_faces: cell.bottom_to_top_faces.clone(),
                boundary_world,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(CurrentLayerOrderViewResponse {
        project_instance_id: project.instance_id,
        project_id: project.project_id,
        revision: project.editor.revision(),
        layer_order_generation: generation,
        cells,
        read_only: true,
    })
}

#[derive(Default)]
pub(super) struct GlobalFlatFoldabilityState(Arc<Mutex<GlobalFlatFoldabilitySlot>>);

#[derive(Default)]
struct GlobalFlatFoldabilitySlot {
    active: Option<ActiveGlobalFlatFoldabilityJob>,
    terminal: Option<TerminalGlobalFlatFoldabilityJob>,
    last_cancelled_id: Option<GlobalFlatFoldabilityJobId>,
    last_replaced_id: Option<GlobalFlatFoldabilityJobId>,
    current_layer_order: Option<Arc<CurrentLayerOrderCertificate>>,
    layer_order_generation: u64,
}

struct ActiveGlobalFlatFoldabilityJob {
    job_id: GlobalFlatFoldabilityJobId,
    binding: Arc<GlobalFlatFoldabilityBinding>,
    runtime: Arc<GlobalFlatFoldabilityRuntime>,
}

struct TerminalGlobalFlatFoldabilityJob {
    job_id: GlobalFlatFoldabilityJobId,
    dto: GlobalFlatFoldabilityJobDto,
    claimed: bool,
}

#[derive(Clone)]
struct CurrentLayerOrderClaims {
    project_instance_id: ProjectId,
    project_id: ProjectId,
    revision: u64,
    topology_input: Arc<TopologyAnalysisInput>,
    fold_model_fingerprint: Arc<str>,
    proof_model_id: Arc<str>,
    layer_model_id: Arc<str>,
    snapshot_identity: Arc<LayerOrderSnapshot>,
    snapshot_provenance: LayerOrderProvenance,
    material_registry: Arc<[LayerFace]>,
    generation: u64,
}

struct CurrentLayerOrderCertificate {
    binding: Arc<GlobalFlatFoldabilityBinding>,
    snapshot: Arc<LayerOrderSnapshot>,
    claims: CurrentLayerOrderClaims,
}

/// An in-process authority token for a native, still-current layer order.
///
/// Its fields deliberately remain private and the type is neither serializable
/// nor clonable. A later native mutation boundary must use the guarded closure
/// API so revalidation and commit share the same project and slot locks.
#[allow(dead_code)]
pub(super) struct CurrentLayerOrderCapability {
    slot: Arc<Mutex<GlobalFlatFoldabilitySlot>>,
    certificate: Arc<CurrentLayerOrderCertificate>,
    claims: CurrentLayerOrderClaims,
}

pub(super) struct CurrentLayerOrderCommitGuard<'a> {
    slot: MutexGuard<'a, GlobalFlatFoldabilitySlot>,
}

impl CurrentLayerOrderCommitGuard<'_> {
    pub(super) fn preflight_certified_target(
        &self,
        expected_project_id: ProjectId,
        expected_revision: u64,
        expected_fingerprint: [u8; 32],
        snapshot: &LayerOrderSnapshot,
    ) -> bool {
        let provenance = snapshot.provenance.source;
        self.slot.layer_order_generation.checked_add(1).is_some()
            && provenance.identity_namespace == Some(expected_project_id)
            && provenance.source_revision == expected_revision
            && provenance
                .source_fingerprint
                .is_some_and(|value| value.0 == expected_fingerprint)
    }

    /// Invalidates the source-revision certificate while the layer slot is
    /// still held at the native commit boundary. A later target certificate
    /// must be freshly minted; stale authority is never carried across edits.
    pub(super) fn invalidate_after_project_mutation(mut self) {
        self.slot.current_layer_order = None;
    }

    /// Replaces the consumed source certificate with a flat-layer certificate
    /// that is independently bound to the already committed target project.
    #[allow(dead_code)]
    pub(super) fn install_certified_target_after_project_mutation(
        mut self,
        project: &ProjectState,
        snapshot: LayerOrderSnapshot,
    ) -> Result<(), ()> {
        let fingerprint = project.editor.fold_model_fingerprint_v1();
        let provenance = snapshot.provenance.source;
        if provenance.identity_namespace != Some(project.project_id)
            || provenance.source_revision != project.editor.revision()
            || provenance
                .source_fingerprint
                .is_none_or(|value| value.to_hex() != fingerprint)
        {
            self.slot.current_layer_order = None;
            return Err(());
        }
        let binding = Arc::new(GlobalFlatFoldabilityBinding {
            project_instance_id: project.instance_id,
            project_id: project.project_id,
            revision: project.editor.revision(),
            topology_input: Arc::new(project.editor.topology_analysis_input(project.project_id)),
            fold_model_fingerprint: Arc::from(fingerprint),
        });
        let certificate = mint_current_layer_order_certificate(&mut self.slot, binding, snapshot)?;
        self.slot.current_layer_order = Some(certificate);
        Ok(())
    }
}

pub(super) fn lock_revalidated_current_layer_order_for_commit<'a>(
    state: &'a GlobalFlatFoldabilityState,
    project: &ProjectState,
    capability: &'a CurrentLayerOrderCapability,
) -> Result<Option<CurrentLayerOrderCommitGuard<'a>>, GlobalFlatFoldabilityCommandError> {
    if !Arc::ptr_eq(&state.0, &capability.slot) {
        return Ok(None);
    }
    let slot = lock_foldability_state(state)?;
    let Some(current) = slot.current_layer_order.as_ref() else {
        return Ok(None);
    };
    if !current_layer_order_capability_matches_locked_slot(&slot, project, capability, current) {
        return Ok(None);
    }
    Ok(Some(CurrentLayerOrderCommitGuard { slot }))
}

pub(super) fn invalidate_current_layer_order_after_history_mutation(
    state: &GlobalFlatFoldabilityState,
) -> Result<(), GlobalFlatFoldabilityCommandError> {
    let mut slot = lock_foldability_state(state)?;
    slot.current_layer_order = None;
    Ok(())
}

impl CurrentLayerOrderCapability {
    /// Observation-only access for detached native analysis. The exact Arc
    /// identity remains sealed in the capability and is rechecked before a
    /// result can be published.
    #[must_use]
    pub(super) fn snapshot(&self) -> &LayerOrderSnapshot {
        self.certificate.snapshot.as_ref()
    }

    #[must_use]
    pub(super) const fn generation(&self) -> u64 {
        self.claims.generation
    }
}

struct GlobalFlatFoldabilityBinding {
    project_instance_id: ProjectId,
    project_id: ProjectId,
    revision: u64,
    topology_input: Arc<TopologyAnalysisInput>,
    fold_model_fingerprint: Arc<str>,
}

struct GlobalFlatFoldabilityRuntime {
    cancellation: AtomicBool,
    worker_started: AtomicBool,
    phase: AtomicU8,
    completed_work: AtomicU64,
    face_count: AtomicU64,
    overlap_cell_count: AtomicU64,
    constraint_count: AtomicU64,
    search_node_count: AtomicU64,
    started_at: Instant,
    deadline: Instant,
}

/// Allows at most one expensive global flat-foldability worker in the process.
///
/// The permit is owned by the blocking worker, so cancellation only requests
/// cooperative exit and cannot release capacity before the worker observes a
/// checkpoint and actually returns.
static GLOBAL_FLAT_FOLDABILITY_WORKER_GATE: GlobalFlatFoldabilityWorkerGate =
    GlobalFlatFoldabilityWorkerGate::new();
#[cfg(test)]
static GLOBAL_FLAT_FOLDABILITY_WORKER_TEST_LOCK: Mutex<()> = Mutex::new(());

struct GlobalFlatFoldabilityWorkerGate(AtomicBool);

impl GlobalFlatFoldabilityWorkerGate {
    const fn new() -> Self {
        Self(AtomicBool::new(false))
    }

    fn try_acquire(&self) -> Option<GlobalFlatFoldabilityWorkerPermit<'_>> {
        self.0
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .ok()
            .map(|_| GlobalFlatFoldabilityWorkerPermit { busy: &self.0 })
    }

    #[cfg(test)]
    fn is_busy(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

struct GlobalFlatFoldabilityWorkerPermit<'a> {
    busy: &'a AtomicBool,
}

impl Drop for GlobalFlatFoldabilityWorkerPermit<'_> {
    fn drop(&mut self) {
        let was_busy = self.busy.swap(false, Ordering::Release);
        debug_assert!(
            was_busy,
            "global flat-foldability worker permit released twice"
        );
    }
}

impl GlobalFlatFoldabilityRuntime {
    fn new(time_limit_ms: u64) -> Self {
        let started_at = Instant::now();
        let deadline = started_at
            .checked_add(Duration::from_millis(time_limit_ms))
            .unwrap_or(started_at);
        Self {
            cancellation: AtomicBool::new(false),
            worker_started: AtomicBool::new(false),
            phase: AtomicU8::new(PHASE_CAPTURING),
            completed_work: AtomicU64::new(0),
            face_count: AtomicU64::new(0),
            overlap_cell_count: AtomicU64::new(0),
            constraint_count: AtomicU64::new(0),
            search_node_count: AtomicU64::new(0),
            started_at,
            deadline,
        }
    }

    fn elapsed_ms(&self) -> u64 {
        duration_millis(self.started_at.elapsed()).min(MAX_REPORTED_ELAPSED_MS)
    }

    fn counts(&self) -> GlobalFlatFoldabilityCountsDto {
        GlobalFlatFoldabilityCountsDto {
            face_count: self
                .face_count
                .load(Ordering::SeqCst)
                .min(MAX_REPORTED_FACES),
            overlap_cell_count: self
                .overlap_cell_count
                .load(Ordering::SeqCst)
                .min(MAX_REPORTED_OVERLAP_CELLS),
            constraint_count: self
                .constraint_count
                .load(Ordering::SeqCst)
                .min(MAX_REPORTED_CONSTRAINTS),
            search_node_count: self
                .search_node_count
                .load(Ordering::SeqCst)
                .min(MAX_REPORTED_WORK),
        }
    }

    fn summary(&self) -> GlobalFlatFoldabilitySummaryDto {
        GlobalFlatFoldabilitySummaryDto {
            model_id: GlobalFlatFoldabilityModelDto::ConvexFacesFacewiseV1,
            elapsed_ms: self.elapsed_ms(),
            counts: self.counts(),
        }
    }

    fn progress(&self) -> Result<GlobalFlatFoldabilityProgressDto, ()> {
        let completed_work = self
            .completed_work
            .load(Ordering::SeqCst)
            .min(MAX_REPORTED_WORK);
        Ok(GlobalFlatFoldabilityProgressDto {
            model_id: GlobalFlatFoldabilityModelDto::ConvexFacesFacewiseV1,
            phase: phase_dto(self.phase.load(Ordering::SeqCst))?,
            completed_work,
            // Core totals are phase-local while completed_work is cumulative.
            // Publishing a mixed-unit total would fabricate a percentage and can regress.
            total_work: None,
            elapsed_ms: self.elapsed_ms(),
            counts: self.counts(),
        })
    }

    fn set_partial_work(&self, face_count: usize, completed_work: usize) {
        let face_count = usize_to_bounded_u64(face_count, MAX_REPORTED_FACES);
        let completed_work = usize_to_bounded_u64(completed_work, MAX_REPORTED_WORK);
        self.face_count.fetch_max(face_count, Ordering::SeqCst);
        self.completed_work
            .fetch_max(completed_work, Ordering::SeqCst);
    }

    fn observe_progress(&self, progress: GlobalFlatFoldabilityProgress) {
        if !self.cancellation.load(Ordering::SeqCst) {
            let _ = advance_phase(&self.phase, core_phase_value(progress.phase));
        }
        self.completed_work.fetch_max(
            usize_to_bounded_u64(progress.completed_work, MAX_REPORTED_WORK),
            Ordering::SeqCst,
        );
        self.overlap_cell_count.fetch_max(
            usize_to_bounded_u64(progress.overlap_cells, MAX_REPORTED_OVERLAP_CELLS),
            Ordering::SeqCst,
        );
        self.constraint_count.fetch_max(
            usize_to_bounded_u64(progress.constraints, MAX_REPORTED_CONSTRAINTS),
            Ordering::SeqCst,
        );
        self.search_node_count.fetch_max(
            usize_to_bounded_u64(progress.search_nodes, MAX_REPORTED_WORK),
            Ordering::SeqCst,
        );
    }

    fn set_reported_counts(&self, work: GlobalFlatFoldabilityWorkCounts) {
        let completed_work = work
            .total_records
            .saturating_add(work.arrangement_segments)
            .saturating_add(work.constraints)
            .saturating_add(work.search_nodes);
        self.set_partial_work(work.face_records, completed_work);
        self.overlap_cell_count.fetch_max(
            usize_to_bounded_u64(work.overlap_cells, MAX_REPORTED_OVERLAP_CELLS),
            Ordering::SeqCst,
        );
        self.constraint_count.fetch_max(
            usize_to_bounded_u64(work.constraints, MAX_REPORTED_CONSTRAINTS),
            Ordering::SeqCst,
        );
        self.search_node_count.fetch_max(
            usize_to_bounded_u64(work.search_nodes, MAX_REPORTED_WORK),
            Ordering::SeqCst,
        );
    }
}

struct NativeGlobalFlatFoldabilityObserver<'a> {
    runtime: &'a GlobalFlatFoldabilityRuntime,
}

impl GlobalFlatFoldabilityObserver for NativeGlobalFlatFoldabilityObserver<'_> {
    fn checkpoint(&mut self) -> GlobalFlatFoldabilityCheckpoint {
        match preprocessing_checkpoint(self.runtime) {
            CooperativeAnalysisCheckpoint::Continue => GlobalFlatFoldabilityCheckpoint::Continue,
            CooperativeAnalysisCheckpoint::Cancelled => GlobalFlatFoldabilityCheckpoint::Cancelled,
            CooperativeAnalysisCheckpoint::DeadlineReached => {
                GlobalFlatFoldabilityCheckpoint::DeadlineReached
            }
        }
    }

    fn on_progress(&mut self, progress: GlobalFlatFoldabilityProgress) {
        self.runtime.observe_progress(progress);
    }
}

struct GlobalFlatFoldabilitySource {
    job_id: GlobalFlatFoldabilityJobId,
    binding: Arc<GlobalFlatFoldabilityBinding>,
    runtime: Arc<GlobalFlatFoldabilityRuntime>,
}

enum GlobalFlatFoldabilityCapture {
    Ready(GlobalFlatFoldabilitySource),
    SourceLimit {
        job_id: GlobalFlatFoldabilityJobId,
        runtime: Arc<GlobalFlatFoldabilityRuntime>,
        violation: SourceRecordLimitViolation,
    },
}

enum PreparedGlobalFlatFoldabilityBegin {
    Registered {
        source: GlobalFlatFoldabilitySource,
        queued: GlobalFlatFoldabilityJobDto,
    },
    UnregisteredSourceLimit(GlobalFlatFoldabilityBeginResponse),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SourceRecordLimitViolation {
    resource: FlatFoldabilityResource,
    limit: usize,
    observed: usize,
}

#[derive(Clone)]
struct GlobalFlatFoldabilityCompletionContext {
    job_id: GlobalFlatFoldabilityJobId,
    binding: Arc<GlobalFlatFoldabilityBinding>,
}

struct GlobalFlatFoldabilityCompletion {
    context: GlobalFlatFoldabilityCompletionContext,
    outcome: WorkerOutcome,
}

#[derive(Debug)]
enum WorkerOutcome {
    Completed {
        result: GlobalFlatFoldabilityResultDto,
        layer_order: Option<Box<LayerOrderSnapshot>>,
    },
    Cancelled,
    Failed(GlobalFlatFoldabilityErrorCategory),
}

enum WorkerCheckpoint {
    Cancelled,
    TimedOut,
    Failed,
}

#[tauri::command]
pub(super) fn begin_global_flat_foldability(
    app: AppHandle,
    state: State<'_, AppState>,
    foldability_state: State<'_, GlobalFlatFoldabilityState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    expected_fold_model_fingerprint: String,
    time_limit_ms: u64,
) -> Result<GlobalFlatFoldabilityBeginResponse, GlobalFlatFoldabilityCommandError> {
    validate_time_limit(time_limit_ms)?;
    validate_fold_model_fingerprint(&expected_fold_model_fingerprint)?;

    let job_id = GlobalFlatFoldabilityJobId::new();
    let runtime = Arc::new(GlobalFlatFoldabilityRuntime::new(time_limit_ms));
    match prepare_global_flat_foldability_begin(
        &state,
        &foldability_state,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        &expected_fold_model_fingerprint,
        job_id,
        runtime,
        native_limits(),
        || {},
    )? {
        PreparedGlobalFlatFoldabilityBegin::Registered { source, queued } => {
            spawn_global_flat_foldability_worker(app, source);
            Ok(GlobalFlatFoldabilityBeginResponse {
                job_id,
                job: queued,
            })
        }
        PreparedGlobalFlatFoldabilityBegin::UnregisteredSourceLimit(response) => Ok(response),
    }
}

#[tauri::command]
pub(super) fn get_global_flat_foldability_progress(
    state: State<'_, GlobalFlatFoldabilityState>,
    job_id: GlobalFlatFoldabilityJobId,
) -> Result<GlobalFlatFoldabilityJobDto, GlobalFlatFoldabilityCommandError> {
    let mut slot = lock_foldability_state(&state)?;
    poll_global_flat_foldability_job(&mut slot, job_id)
}

fn poll_global_flat_foldability_job(
    slot: &mut GlobalFlatFoldabilitySlot,
    job_id: GlobalFlatFoldabilityJobId,
) -> Result<GlobalFlatFoldabilityJobDto, GlobalFlatFoldabilityCommandError> {
    if let Some(active) = slot.active.as_ref().filter(|job| job.job_id == job_id) {
        return active_job_dto(&active.runtime);
    }
    // The worker may have completed after get_result reported "unavailable".
    // Claiming here under the same slot lock closes that result→progress race.
    claim_terminal_result(slot, job_id)
}

#[tauri::command]
pub(super) fn get_global_flat_foldability_result(
    state: State<'_, GlobalFlatFoldabilityState>,
    job_id: GlobalFlatFoldabilityJobId,
) -> Result<GlobalFlatFoldabilityJobDto, GlobalFlatFoldabilityCommandError> {
    let mut slot = lock_foldability_state(&state)?;
    if slot.active.as_ref().is_some_and(|job| job.job_id == job_id) {
        return Err(GlobalFlatFoldabilityCommandError::new(
            GlobalFlatFoldabilityErrorCategory::ResultUnavailable,
        ));
    }
    claim_terminal_result(&mut slot, job_id)
}

#[tauri::command]
pub(super) fn cancel_global_flat_foldability(
    state: State<'_, GlobalFlatFoldabilityState>,
    job_id: GlobalFlatFoldabilityJobId,
) -> Result<(), GlobalFlatFoldabilityCommandError> {
    let mut slot = lock_foldability_state(&state)?;
    cancel_job(&mut slot, job_id)
}

fn validate_time_limit(time_limit_ms: u64) -> Result<(), GlobalFlatFoldabilityCommandError> {
    if (MIN_TIME_LIMIT_MS..=MAX_TIME_LIMIT_MS).contains(&time_limit_ms) {
        Ok(())
    } else {
        Err(GlobalFlatFoldabilityCommandError::new(
            GlobalFlatFoldabilityErrorCategory::InvalidRequest,
        ))
    }
}

fn validate_fold_model_fingerprint(
    fingerprint: &str,
) -> Result<(), GlobalFlatFoldabilityCommandError> {
    if fingerprint.len() == 64
        && fingerprint
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(())
    } else {
        Err(GlobalFlatFoldabilityCommandError::new(
            GlobalFlatFoldabilityErrorCategory::InvalidRequest,
        ))
    }
}

fn lock_foldability_state(
    state: &GlobalFlatFoldabilityState,
) -> Result<MutexGuard<'_, GlobalFlatFoldabilitySlot>, GlobalFlatFoldabilityCommandError> {
    state.0.lock().map_err(|_| {
        GlobalFlatFoldabilityCommandError::new(GlobalFlatFoldabilityErrorCategory::InternalFailure)
    })
}

#[allow(clippy::too_many_arguments)]
fn prepare_global_flat_foldability_begin(
    state: &AppState,
    foldability_state: &GlobalFlatFoldabilityState,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    expected_fold_model_fingerprint: &str,
    job_id: GlobalFlatFoldabilityJobId,
    runtime: Arc<GlobalFlatFoldabilityRuntime>,
    limits: GlobalFlatFoldabilityLimits,
    after_capture: impl FnOnce(),
) -> Result<PreparedGlobalFlatFoldabilityBegin, GlobalFlatFoldabilityCommandError> {
    let project = lock_project(state).map_err(|_| {
        GlobalFlatFoldabilityCommandError::new(
            GlobalFlatFoldabilityErrorCategory::SnapshotUnavailable,
        )
    })?;
    let capture = capture_source(
        &project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        expected_fold_model_fingerprint,
        job_id,
        Arc::clone(&runtime),
        limits,
    )?;
    // Tests synchronize at this exact boundary. Production supplies a no-op.
    // The project guard must remain live through the later slot installation.
    after_capture();
    let source = match capture {
        GlobalFlatFoldabilityCapture::Ready(source) => source,
        GlobalFlatFoldabilityCapture::SourceLimit {
            job_id,
            runtime,
            violation,
        } => {
            // This preflight result is deliberately unregistered. Return
            // before taking the slot so an existing job, terminal result,
            // or current layer certificate remains bit-for-bit unchanged.
            debug_assert!(violation.observed > violation.limit);
            debug_assert!(matches!(
                violation.resource,
                FlatFoldabilityResource::SourceVertices
                    | FlatFoldabilityResource::SourceEdges
                    | FlatFoldabilityResource::PaperBoundaryVertices
                    | FlatFoldabilityResource::TotalRecords
            ));
            return Ok(PreparedGlobalFlatFoldabilityBegin::UnregisteredSourceLimit(
                source_limit_begin_response(job_id, &runtime),
            ));
        }
    };
    // Finish every fallible, non-mutating response conversion before the slot
    // changes. Holding project from capture through install linearizes begin.
    let queued = active_job_dto(&runtime)?;
    let mut slot = lock_foldability_state(foldability_state)?;
    install_captured_source_if_current(&mut slot, &project, &source)?;
    Ok(PreparedGlobalFlatFoldabilityBegin::Registered { source, queued })
}

fn capture_source(
    project: &ProjectState,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    expected_fold_model_fingerprint: &str,
    job_id: GlobalFlatFoldabilityJobId,
    runtime: Arc<GlobalFlatFoldabilityRuntime>,
    limits: GlobalFlatFoldabilityLimits,
) -> Result<GlobalFlatFoldabilityCapture, GlobalFlatFoldabilityCommandError> {
    if project.instance_id != expected_project_instance_id
        || project.project_id != expected_project_id
        || project.editor.revision() != expected_revision
    {
        return Err(GlobalFlatFoldabilityCommandError::new(
            GlobalFlatFoldabilityErrorCategory::SnapshotUnavailable,
        ));
    }
    let fold_model_fingerprint = project.editor.fold_model_fingerprint_v1();
    if fold_model_fingerprint != expected_fold_model_fingerprint {
        return Err(GlobalFlatFoldabilityCommandError::new(
            GlobalFlatFoldabilityErrorCategory::SnapshotUnavailable,
        ));
    }
    if let Some(violation) =
        source_record_limit_violation(project.editor.pattern(), project.editor.paper(), limits)
    {
        return Ok(GlobalFlatFoldabilityCapture::SourceLimit {
            job_id,
            runtime,
            violation,
        });
    }
    let binding = Arc::new(GlobalFlatFoldabilityBinding {
        project_instance_id: project.instance_id,
        project_id: project.project_id,
        revision: expected_revision,
        topology_input: Arc::new(project.editor.topology_analysis_input(project.project_id)),
        fold_model_fingerprint: Arc::from(fold_model_fingerprint),
    });
    Ok(GlobalFlatFoldabilityCapture::Ready(
        GlobalFlatFoldabilitySource {
            job_id,
            binding,
            runtime,
        },
    ))
}

fn source_record_limit_violation(
    pattern: &CreasePattern,
    paper: &Paper,
    limits: GlobalFlatFoldabilityLimits,
) -> Option<SourceRecordLimitViolation> {
    let source_vertices = pattern.vertices.len();
    if source_vertices > limits.max_source_vertices {
        return Some(SourceRecordLimitViolation {
            resource: FlatFoldabilityResource::SourceVertices,
            limit: limits.max_source_vertices,
            observed: source_vertices,
        });
    }
    let source_edges = pattern.edges.len();
    if source_edges > limits.max_source_edges {
        return Some(SourceRecordLimitViolation {
            resource: FlatFoldabilityResource::SourceEdges,
            limit: limits.max_source_edges,
            observed: source_edges,
        });
    }
    let boundary_vertices = paper.boundary_vertices.len();
    if boundary_vertices > limits.max_paper_boundary_vertices {
        return Some(SourceRecordLimitViolation {
            resource: FlatFoldabilityResource::PaperBoundaryVertices,
            limit: limits.max_paper_boundary_vertices,
            observed: boundary_vertices,
        });
    }
    let total_records = source_vertices
        .checked_add(source_edges)
        .and_then(|total| total.checked_add(boundary_vertices));
    match total_records {
        Some(observed) if observed <= limits.max_total_records => None,
        Some(observed) => Some(SourceRecordLimitViolation {
            resource: FlatFoldabilityResource::TotalRecords,
            limit: limits.max_total_records,
            observed,
        }),
        None => Some(SourceRecordLimitViolation {
            resource: FlatFoldabilityResource::TotalRecords,
            limit: limits.max_total_records,
            observed: usize::MAX,
        }),
    }
}

fn source_limit_begin_response(
    job_id: GlobalFlatFoldabilityJobId,
    runtime: &GlobalFlatFoldabilityRuntime,
) -> GlobalFlatFoldabilityBeginResponse {
    GlobalFlatFoldabilityBeginResponse {
        job_id,
        job: GlobalFlatFoldabilityJobDto::Completed {
            result: unknown_result(
                runtime.summary(),
                GlobalFlatFoldabilityUnknownReasonDto::WorkLimitReached,
            ),
        },
    }
}

fn install_captured_source_if_current(
    slot: &mut GlobalFlatFoldabilitySlot,
    project: &ProjectState,
    source: &GlobalFlatFoldabilitySource,
) -> Result<(), GlobalFlatFoldabilityCommandError> {
    if !binding_is_current(&source.binding, project) {
        return Err(GlobalFlatFoldabilityCommandError::new(
            GlobalFlatFoldabilityErrorCategory::SnapshotUnavailable,
        ));
    }
    install_active_job(slot, source);
    Ok(())
}

fn install_active_job(slot: &mut GlobalFlatFoldabilitySlot, source: &GlobalFlatFoldabilitySource) {
    if let Some(active) = slot.active.take() {
        active.runtime.cancellation.store(true, Ordering::SeqCst);
        slot.last_replaced_id = Some(active.job_id);
    }
    if let Some(terminal) = slot.terminal.take() {
        slot.last_replaced_id = Some(terminal.job_id);
    }
    slot.current_layer_order = None;
    slot.active = Some(ActiveGlobalFlatFoldabilityJob {
        job_id: source.job_id,
        binding: Arc::clone(&source.binding),
        runtime: Arc::clone(&source.runtime),
    });
}

fn spawn_global_flat_foldability_worker(app: AppHandle, source: GlobalFlatFoldabilitySource) {
    let fallback_context = completion_context(&source);
    tauri::async_runtime::spawn(async move {
        let joined = tauri::async_runtime::spawn_blocking(move || guarded_run_worker(source)).await;
        let completion = match joined {
            Ok(completion) => completion,
            Err(_) => GlobalFlatFoldabilityCompletion {
                context: fallback_context,
                outcome: WorkerOutcome::Failed(
                    GlobalFlatFoldabilityErrorCategory::WorkerUnavailable,
                ),
            },
        };
        finish_background_completion(&app, completion);
    });
}

fn completion_context(
    source: &GlobalFlatFoldabilitySource,
) -> GlobalFlatFoldabilityCompletionContext {
    GlobalFlatFoldabilityCompletionContext {
        job_id: source.job_id,
        binding: Arc::clone(&source.binding),
    }
}

fn guarded_run_worker(source: GlobalFlatFoldabilitySource) -> GlobalFlatFoldabilityCompletion {
    let context = completion_context(&source);
    // Unit fixtures invoke the production worker synchronously from Rust's
    // parallel test threads. Serialize those fixtures without weakening the
    // process-wide nonblocking behavior used by production.
    #[cfg(test)]
    let _test_lock = match GLOBAL_FLAT_FOLDABILITY_WORKER_TEST_LOCK.lock() {
        Ok(lock) => lock,
        Err(_) => {
            return GlobalFlatFoldabilityCompletion {
                context,
                outcome: WorkerOutcome::Failed(
                    GlobalFlatFoldabilityErrorCategory::WorkerUnavailable,
                ),
            };
        }
    };
    let Some(_permit) = GLOBAL_FLAT_FOLDABILITY_WORKER_GATE.try_acquire() else {
        return GlobalFlatFoldabilityCompletion {
            context,
            outcome: WorkerOutcome::Failed(GlobalFlatFoldabilityErrorCategory::WorkerUnavailable),
        };
    };
    let outcome = catch_worker_failure(|| run_worker(source));
    GlobalFlatFoldabilityCompletion { context, outcome }
}

fn catch_worker_failure(worker: impl FnOnce() -> WorkerOutcome) -> WorkerOutcome {
    match catch_unwind(AssertUnwindSafe(worker)) {
        Ok(outcome) => outcome,
        Err(_) => WorkerOutcome::Failed(GlobalFlatFoldabilityErrorCategory::InternalFailure),
    }
}

fn run_worker(source: GlobalFlatFoldabilitySource) -> WorkerOutcome {
    source.runtime.worker_started.store(true, Ordering::SeqCst);
    if let Some(outcome) = checkpoint_outcome(&source.runtime, PHASE_CAPTURING) {
        return outcome;
    }

    let topology = {
        let mut checkpoint = || preprocessing_checkpoint(&source.runtime);
        match source
            .binding
            .topology_input
            .analyze_with_checkpoint(&mut checkpoint)
        {
            Ok(topology) => topology,
            Err(abort) => return preprocessing_abort_outcome(&source.runtime, abort),
        }
    };
    if let Some(outcome) = checkpoint_outcome(&source.runtime, PHASE_VALIDATING_LOCAL_CONDITIONS) {
        return outcome;
    }

    let local_flat_foldability = {
        let mut checkpoint = || preprocessing_checkpoint(&source.runtime);
        match analyze_local_flat_foldability_with_checkpoint(
            source.binding.topology_input.paper(),
            source.binding.topology_input.pattern(),
            &mut checkpoint,
        ) {
            Ok(report) => report,
            Err(abort) => return preprocessing_abort_outcome(&source.runtime, abort),
        }
    };
    let partial_face_count = topology
        .report()
        .snapshot
        .as_ref()
        .map_or(0, |snapshot| snapshot.faces.len());
    source
        .runtime
        .set_partial_work(partial_face_count, local_flat_foldability.vertices.len());

    let Some(topology_snapshot) = topology.simulation_snapshot() else {
        let result = unknown_result(
            source.runtime.summary(),
            GlobalFlatFoldabilityUnknownReasonDto::UnsupportedTopology,
        );
        return complete_worker_result(&source.runtime, result, None);
    };
    let report =
        match analyze_captured_snapshot(&source, topology_snapshot, &local_flat_foldability) {
            Ok(report) => report,
            Err(GlobalFlatFoldabilityExecutionError::Cancelled) => return WorkerOutcome::Cancelled,
            Err(GlobalFlatFoldabilityExecutionError::Internal { .. }) => {
                return WorkerOutcome::Failed(GlobalFlatFoldabilityErrorCategory::InternalFailure);
            }
        };
    source.runtime.set_reported_counts(report.work_counts);

    if source.runtime.phase.load(Ordering::SeqCst) < PHASE_VERIFYING_CERTIFICATE
        && let Some(outcome) = checkpoint_outcome(&source.runtime, PHASE_VERIFYING_CERTIFICATE)
    {
        return outcome;
    }

    let (result, layer_order) = match report_to_dto(
        report,
        source.runtime.summary(),
        topology_snapshot,
        source.binding.project_id,
        &source.binding.fold_model_fingerprint,
    ) {
        Ok(converted) => converted,
        Err(()) => {
            return WorkerOutcome::Failed(GlobalFlatFoldabilityErrorCategory::InternalFailure);
        }
    };
    complete_worker_result(&source.runtime, result, layer_order)
}

fn preprocessing_checkpoint(
    runtime: &GlobalFlatFoldabilityRuntime,
) -> CooperativeAnalysisCheckpoint {
    if runtime.cancellation.load(Ordering::SeqCst) {
        CooperativeAnalysisCheckpoint::Cancelled
    } else if Instant::now() >= runtime.deadline {
        CooperativeAnalysisCheckpoint::DeadlineReached
    } else {
        CooperativeAnalysisCheckpoint::Continue
    }
}

fn preprocessing_abort_outcome(
    runtime: &GlobalFlatFoldabilityRuntime,
    abort: CooperativeAnalysisAbort,
) -> WorkerOutcome {
    match abort {
        CooperativeAnalysisAbort::Cancelled => WorkerOutcome::Cancelled,
        CooperativeAnalysisAbort::DeadlineReached => WorkerOutcome::Completed {
            result: unknown_result(
                runtime.summary(),
                GlobalFlatFoldabilityUnknownReasonDto::TimeLimitReached,
            ),
            layer_order: None,
        },
    }
}

fn analyze_captured_snapshot(
    source: &GlobalFlatFoldabilitySource,
    topology: &ori_core::TopologySnapshot,
    local_flat_foldability: &ori_core::LocalFlatFoldabilityReport,
) -> Result<GlobalFlatFoldabilityReport, GlobalFlatFoldabilityExecutionError> {
    let mut observer = NativeGlobalFlatFoldabilityObserver {
        runtime: &source.runtime,
    };
    // Native "possible" results always carry the same geometry-backed,
    // independently verified certificate, including 0/1-hinge projects.
    let input = GlobalFlatFoldabilityInput::current_with_geometry(
        source.binding.project_id,
        source.binding.topology_input.paper(),
        source.binding.topology_input.pattern(),
        topology,
        local_flat_foldability,
    );
    analyze_global_flat_foldability_with_observer(input, native_limits(), &mut observer)
}

fn complete_worker_result(
    runtime: &GlobalFlatFoldabilityRuntime,
    result: GlobalFlatFoldabilityResultDto,
    layer_order: Option<LayerOrderSnapshot>,
) -> WorkerOutcome {
    if let Some(outcome) = checkpoint_outcome(runtime, PHASE_COMPLETED) {
        return outcome;
    }
    WorkerOutcome::Completed {
        result,
        layer_order: layer_order.map(Box::new),
    }
}

fn checkpoint_outcome(
    runtime: &GlobalFlatFoldabilityRuntime,
    next_phase: u8,
) -> Option<WorkerOutcome> {
    match worker_checkpoint(runtime, next_phase) {
        Ok(()) => None,
        Err(WorkerCheckpoint::Cancelled) => Some(WorkerOutcome::Cancelled),
        Err(WorkerCheckpoint::TimedOut) => Some(WorkerOutcome::Completed {
            result: unknown_result(
                runtime.summary(),
                GlobalFlatFoldabilityUnknownReasonDto::TimeLimitReached,
            ),
            layer_order: None,
        }),
        Err(WorkerCheckpoint::Failed) => Some(WorkerOutcome::Failed(
            GlobalFlatFoldabilityErrorCategory::InternalFailure,
        )),
    }
}

fn worker_checkpoint(
    runtime: &GlobalFlatFoldabilityRuntime,
    next_phase: u8,
) -> Result<(), WorkerCheckpoint> {
    if runtime.cancellation.load(Ordering::SeqCst) {
        return Err(WorkerCheckpoint::Cancelled);
    }
    if Instant::now() >= runtime.deadline {
        return Err(WorkerCheckpoint::TimedOut);
    }
    advance_phase(&runtime.phase, next_phase).map_err(|_| WorkerCheckpoint::Failed)
}

fn advance_phase(phase: &AtomicU8, next: u8) -> Result<(), ()> {
    if next > PHASE_COMPLETED {
        return Err(());
    }
    let mut current = phase.load(Ordering::SeqCst);
    loop {
        if current > next {
            return Err(());
        }
        if current == next {
            return Ok(());
        }
        match phase.compare_exchange(current, next, Ordering::SeqCst, Ordering::SeqCst) {
            Ok(_) => return Ok(()),
            Err(observed) => current = observed,
        }
    }
}

fn phase_dto(value: u8) -> Result<GlobalFlatFoldabilityPhaseDto, ()> {
    match value {
        PHASE_CAPTURING => Ok(GlobalFlatFoldabilityPhaseDto::Capturing),
        PHASE_VALIDATING_LOCAL_CONDITIONS => {
            Ok(GlobalFlatFoldabilityPhaseDto::ValidatingLocalConditions)
        }
        PHASE_BUILDING_FLAT_EMBEDDING => Ok(GlobalFlatFoldabilityPhaseDto::BuildingFlatEmbedding),
        PHASE_BUILDING_OVERLAP_ARRANGEMENT => {
            Ok(GlobalFlatFoldabilityPhaseDto::BuildingOverlapArrangement)
        }
        PHASE_BUILDING_CONSTRAINTS => Ok(GlobalFlatFoldabilityPhaseDto::BuildingConstraints),
        PHASE_PROPAGATING => Ok(GlobalFlatFoldabilityPhaseDto::Propagating),
        PHASE_SEARCHING => Ok(GlobalFlatFoldabilityPhaseDto::Searching),
        PHASE_VERIFYING_CERTIFICATE => Ok(GlobalFlatFoldabilityPhaseDto::VerifyingCertificate),
        PHASE_COMPLETED => Ok(GlobalFlatFoldabilityPhaseDto::Completed),
        _ => Err(()),
    }
}

const fn core_phase_value(phase: GlobalFlatFoldabilityPhase) -> u8 {
    match phase {
        GlobalFlatFoldabilityPhase::Capturing => PHASE_CAPTURING,
        GlobalFlatFoldabilityPhase::ValidatingLocalConditions => PHASE_VALIDATING_LOCAL_CONDITIONS,
        GlobalFlatFoldabilityPhase::BuildingFlatEmbedding => PHASE_BUILDING_FLAT_EMBEDDING,
        GlobalFlatFoldabilityPhase::BuildingOverlapArrangement => {
            PHASE_BUILDING_OVERLAP_ARRANGEMENT
        }
        GlobalFlatFoldabilityPhase::BuildingConstraints => PHASE_BUILDING_CONSTRAINTS,
        GlobalFlatFoldabilityPhase::Propagating => PHASE_PROPAGATING,
        GlobalFlatFoldabilityPhase::Searching => PHASE_SEARCHING,
        GlobalFlatFoldabilityPhase::VerifyingCertificate => PHASE_VERIFYING_CERTIFICATE,
        GlobalFlatFoldabilityPhase::Completed => PHASE_COMPLETED,
    }
}

fn native_limits() -> GlobalFlatFoldabilityLimits {
    GlobalFlatFoldabilityLimits::default()
}

fn report_to_dto(
    report: GlobalFlatFoldabilityReport,
    summary: GlobalFlatFoldabilitySummaryDto,
    topology: &TopologySnapshot,
    expected_project_id: ProjectId,
    expected_fold_model_fingerprint: &str,
) -> Result<(GlobalFlatFoldabilityResultDto, Option<LayerOrderSnapshot>), ()> {
    if report.provenance.model_id != GLOBAL_FLAT_FOLDABILITY_MODEL_ID
        || report.provenance.source_revision != topology.source_revision
        || report.provenance.identity_namespace != Some(expected_project_id)
    {
        return Err(());
    }
    let source_fingerprint_matches = report
        .provenance
        .source_fingerprint
        .map(|fingerprint| fingerprint.to_hex() == expected_fold_model_fingerprint);
    if source_fingerprint_matches == Some(false) {
        return Err(());
    }
    let work_counts = report.work_counts;
    match report.outcome {
        GlobalFlatFoldabilityOutcome::Possible { layer_order, .. } => {
            if source_fingerprint_matches != Some(true) {
                return Err(());
            }
            validate_proof_summary_counts(summary, work_counts)?;
            let canonical_faces = canonical_face_registry(topology, summary.counts.face_count)?;
            if layer_order.model_id != LAYER_ORDER_MODEL_ID
                || !layer_order.is_current_for(&report.provenance)
                || layer_order.material_faces.is_empty()
            {
                return Err(());
            }
            let layer_summary = layer_order_summary(&layer_order, &canonical_faces, work_counts)?;
            Ok((
                GlobalFlatFoldabilityResultDto::Possible {
                    summary,
                    layer_order: layer_summary,
                },
                Some(*layer_order),
            ))
        }
        GlobalFlatFoldabilityOutcome::Impossible { reason } => {
            if source_fingerprint_matches != Some(true) {
                return Err(());
            }
            validate_proof_summary_counts(summary, work_counts)?;
            let canonical_faces = canonical_face_registry(topology, summary.counts.face_count)?;
            let proof = impossible_proof(reason, topology, &canonical_faces, work_counts)?;
            Ok((
                GlobalFlatFoldabilityResultDto::Impossible { summary, proof },
                None,
            ))
        }
        GlobalFlatFoldabilityOutcome::Unknown { reason } => {
            Ok((unknown_result(summary, map_unknown_reason(reason)), None))
        }
    }
}

fn validate_proof_summary_counts(
    summary: GlobalFlatFoldabilitySummaryDto,
    work: GlobalFlatFoldabilityWorkCounts,
) -> Result<(), ()> {
    let expected = GlobalFlatFoldabilityCountsDto {
        face_count: usize_to_exact_bounded_u64(work.face_records, MAX_REPORTED_FACES).ok_or(())?,
        overlap_cell_count: usize_to_exact_bounded_u64(
            work.overlap_cells,
            MAX_REPORTED_OVERLAP_CELLS,
        )
        .ok_or(())?,
        constraint_count: usize_to_exact_bounded_u64(work.constraints, MAX_REPORTED_CONSTRAINTS)
            .ok_or(())?,
        search_node_count: usize_to_exact_bounded_u64(work.search_nodes, MAX_REPORTED_WORK)
            .ok_or(())?,
    };
    if summary.counts == expected {
        Ok(())
    } else {
        Err(())
    }
}

fn canonical_face_registry(
    topology: &TopologySnapshot,
    reported_face_count: u64,
) -> Result<Vec<LayerFace>, ()> {
    if usize_to_exact_bounded_u64(topology.faces.len(), MAX_REPORTED_FACES)
        != Some(reported_face_count)
    {
        return Err(());
    }
    let mut face_ids = HashSet::with_capacity(topology.faces.len());
    let mut face_keys = HashSet::with_capacity(topology.faces.len());
    let mut canonical_faces = Vec::with_capacity(topology.faces.len());
    for face in &topology.faces {
        if !face_ids.insert(face.id) || !face_keys.insert(face.key) {
            return Err(());
        }
        canonical_faces.push(LayerFace {
            face_id: face.id,
            face_key: face.key,
        });
    }
    canonical_faces.sort_by_key(|face| (face.face_key, face.face_id.canonical_bytes()));
    Ok(canonical_faces)
}

fn impossible_proof(
    reason: GlobalFlatFoldabilityImpossibleReason,
    topology: &TopologySnapshot,
    canonical_faces: &[LayerFace],
    work: GlobalFlatFoldabilityWorkCounts,
) -> Result<GlobalFlatFoldabilityProofDto, ()> {
    if canonical_faces.is_empty() {
        return Err(());
    }
    let (category, face_numbers) = match reason {
        GlobalFlatFoldabilityImpossibleReason::LocalNecessaryConditionViolated { violations } => {
            if violations.is_empty() {
                return Err(());
            }
            let mut incident_faces = HashMap::new();
            for face in &topology.faces {
                let face_number = canonical_face_number(
                    canonical_faces,
                    LayerFace {
                        face_id: face.id,
                        face_key: face.key,
                    },
                )?;
                for half_edge in &face.outer.half_edges {
                    incident_faces
                        .entry(half_edge.origin)
                        .or_insert_with(Vec::new)
                        .push(face_number);
                    incident_faces
                        .entry(half_edge.destination)
                        .or_insert_with(Vec::new)
                        .push(face_number);
                }
            }
            let mut seen_vertices = HashSet::with_capacity(violations.len());
            let mut numbers = HashSet::new();
            for violation in violations {
                if (!violation.kawasaki_violated && !violation.maekawa_violated)
                    || !seen_vertices.insert(violation.vertex)
                {
                    return Err(());
                }
                let incident = incident_faces.get(&violation.vertex).ok_or(())?;
                if incident.is_empty() {
                    return Err(());
                }
                numbers.extend(incident.iter().copied());
            }
            (
                GlobalFlatFoldabilityProofCategoryDto::LocalConditionsViolated,
                bounded_face_numbers(numbers)?,
            )
        }
        GlobalFlatFoldabilityImpossibleReason::InconsistentFlatEmbedding {
            face,
            conflicting_hinge,
            conflicting_vertex,
        } => {
            let face_number = canonical_face_number(canonical_faces, face)?;
            let source_face = topology
                .faces
                .iter()
                .find(|candidate| candidate.id == face.face_id && candidate.key == face.face_key)
                .ok_or(())?;
            if !source_face
                .outer
                .half_edges
                .iter()
                .any(|half_edge| half_edge.edge == conflicting_hinge)
                || !source_face.outer.half_edges.iter().any(|half_edge| {
                    half_edge.origin == conflicting_vertex
                        || half_edge.destination == conflicting_vertex
                })
            {
                return Err(());
            }
            (
                GlobalFlatFoldabilityProofCategoryDto::InconsistentFlatEmbedding,
                vec![face_number],
            )
        }
        GlobalFlatFoldabilityImpossibleReason::FacewiseConstraintContradiction {
            faces, ..
        } => {
            if faces.is_empty() {
                return Err(());
            }
            let mut seen_face_ids = HashSet::with_capacity(faces.len());
            let mut seen_face_keys = HashSet::with_capacity(faces.len());
            let mut numbers = HashSet::with_capacity(faces.len());
            for face in faces {
                if !seen_face_ids.insert(face.face_id) || !seen_face_keys.insert(face.face_key) {
                    return Err(());
                }
                numbers.insert(canonical_face_number(canonical_faces, face)?);
            }
            (
                GlobalFlatFoldabilityProofCategoryDto::LayerConstraintsContradictory,
                bounded_face_numbers(numbers)?,
            )
        }
        GlobalFlatFoldabilityImpossibleReason::FacewiseSearchExhausted {
            variable_count,
            constraint_count,
        } => {
            if variable_count != work.overlap_face_pairs || constraint_count != work.constraints {
                return Err(());
            }
            let upper = canonical_faces.len().min(MAX_REPORTED_PROOF_FACES);
            let face_numbers = (1..=upper)
                .map(|number| u64::try_from(number).map_err(|_| ()))
                .collect::<Result<Vec<_>, _>>()?;
            (
                GlobalFlatFoldabilityProofCategoryDto::ExhaustiveSearchNoSolution,
                face_numbers,
            )
        }
    };
    if face_numbers.is_empty() || face_numbers.len() > MAX_REPORTED_PROOF_FACES {
        return Err(());
    }
    Ok(GlobalFlatFoldabilityProofDto {
        category,
        face_numbers,
    })
}

fn canonical_face_number(canonical_faces: &[LayerFace], face: LayerFace) -> Result<u64, ()> {
    canonical_faces
        .iter()
        .position(|candidate| *candidate == face)
        .and_then(|index| u64::try_from(index + 1).ok())
        .ok_or(())
}

fn bounded_face_numbers(numbers: HashSet<u64>) -> Result<Vec<u64>, ()> {
    let mut numbers = numbers.into_iter().collect::<Vec<_>>();
    numbers.sort_unstable();
    numbers.truncate(MAX_REPORTED_PROOF_FACES);
    if numbers.is_empty() {
        Err(())
    } else {
        Ok(numbers)
    }
}

fn layer_order_summary(
    layer_order: &LayerOrderSnapshot,
    canonical_faces: &[LayerFace],
    work: GlobalFlatFoldabilityWorkCounts,
) -> Result<LayerOrderSummaryDto, ()> {
    let layer_count =
        usize_to_exact_bounded_u64(canonical_faces.len(), MAX_REPORTED_FACES).ok_or(())?;
    if layer_count == 0 || layer_order.material_faces.as_slice() != canonical_faces {
        return Err(());
    }
    let reference_face = layer_order.reference_face.ok_or(())?;
    let mut folded_face_registry = layer_order
        .folded_faces
        .iter()
        .map(|folded| folded.face)
        .collect::<Vec<_>>();
    let folded_face_count = folded_face_registry.len();
    folded_face_registry.sort_by_key(|face| (face.face_key, face.face_id.canonical_bytes()));
    folded_face_registry.dedup();
    if folded_face_registry.len() != folded_face_count
        || folded_face_registry.as_slice() != canonical_faces
    {
        return Err(());
    }
    let reference_face_number = canonical_faces
        .iter()
        .position(|face| *face == reference_face)
        .and_then(|index| u64::try_from(index + 1).ok())
        .ok_or(())?;
    let proof_summary = layer_order.proof_summary.ok_or(())?;
    if proof_summary.material_faces != canonical_faces.len()
        || proof_summary.overlap_face_pairs != work.overlap_face_pairs
        || proof_summary.overlap_cells != work.overlap_cells
        || proof_summary.constraints != work.constraints
        || proof_summary.search_nodes != work.search_nodes
        || proof_summary.certificate_bytes != work.certificate_bytes
    {
        return Err(());
    }
    let maximum_ply = proof_summary.maximum_ply;
    let max_ply = usize_to_exact_bounded_u64(maximum_ply, layer_count).ok_or(())?;
    if max_ply == 0 {
        return Err(());
    }
    Ok(LayerOrderSummaryDto {
        model_id: LayerOrderModelDto::FacewiseLayerOrderV1,
        layer_count,
        max_ply,
        reference_face_number,
        // The verified snapshot is available internally for SIM-010, but the
        // dedicated layer-order 3D viewer is not part of the UI yet.
        layer_view_available: false,
    })
}

fn map_unknown_reason(
    reason: GlobalFlatFoldabilityUnknownReason,
) -> GlobalFlatFoldabilityUnknownReasonDto {
    match reason {
        GlobalFlatFoldabilityUnknownReason::ResourceLimitReached { resource, .. } => {
            map_resource_limit(resource)
        }
        GlobalFlatFoldabilityUnknownReason::UnsupportedTargetClass { .. } => {
            GlobalFlatFoldabilityUnknownReasonDto::UnsupportedTopology
        }
        GlobalFlatFoldabilityUnknownReason::UnsupportedTopology { .. } => {
            GlobalFlatFoldabilityUnknownReasonDto::UnsupportedTopology
        }
        GlobalFlatFoldabilityUnknownReason::NonConvexFace { .. } => {
            GlobalFlatFoldabilityUnknownReasonDto::NonConvexFace
        }
        GlobalFlatFoldabilityUnknownReason::TimeLimitReached { .. } => {
            GlobalFlatFoldabilityUnknownReasonDto::TimeLimitReached
        }
        GlobalFlatFoldabilityUnknownReason::ExactNumberLimitReached { .. } => {
            GlobalFlatFoldabilityUnknownReasonDto::ExactNumberLimitReached
        }
        GlobalFlatFoldabilityUnknownReason::OverlapArrangementLimitReached { .. } => {
            GlobalFlatFoldabilityUnknownReasonDto::OverlapArrangementLimitReached
        }
        GlobalFlatFoldabilityUnknownReason::ConstraintLimitReached { .. } => {
            GlobalFlatFoldabilityUnknownReasonDto::ConstraintLimitReached
        }
        GlobalFlatFoldabilityUnknownReason::ProofIncomplete { reason } => match reason {
            FlatFoldabilityProofIncompleteReason::LocalNecessaryConditionsBlocked
            | FlatFoldabilityProofIncompleteReason::LocalNecessaryConditionsIndeterminate => {
                GlobalFlatFoldabilityUnknownReasonDto::LocalConditionsIndeterminate
            }
            FlatFoldabilityProofIncompleteReason::NoMaterialFaces
            | FlatFoldabilityProofIncompleteReason::DisconnectedFacesWithoutHinge => {
                GlobalFlatFoldabilityUnknownReasonDto::UnsupportedTopology
            }
            FlatFoldabilityProofIncompleteReason::SingleHingeDoesNotCoverExactlyTwoFaces => {
                GlobalFlatFoldabilityUnknownReasonDto::ProofNotCompleted
            }
            FlatFoldabilityProofIncompleteReason::GeometryInputUnavailable
            | FlatFoldabilityProofIncompleteReason::CertificateReverificationFailed => {
                GlobalFlatFoldabilityUnknownReasonDto::ProofNotCompleted
            }
        },
        GlobalFlatFoldabilityUnknownReason::StaleProvenance { .. }
        | GlobalFlatFoldabilityUnknownReason::InconsistentInput { .. } => {
            GlobalFlatFoldabilityUnknownReasonDto::ProofNotCompleted
        }
    }
}

const fn map_resource_limit(
    resource: FlatFoldabilityResource,
) -> GlobalFlatFoldabilityUnknownReasonDto {
    match resource {
        FlatFoldabilityResource::OverlapFacePairs
        | FlatFoldabilityResource::ArrangementSegments
        | FlatFoldabilityResource::OverlapCells => {
            GlobalFlatFoldabilityUnknownReasonDto::OverlapArrangementLimitReached
        }
        FlatFoldabilityResource::Constraints => {
            GlobalFlatFoldabilityUnknownReasonDto::ConstraintLimitReached
        }
        FlatFoldabilityResource::SourceVertices
        | FlatFoldabilityResource::SourceEdges
        | FlatFoldabilityResource::PaperBoundaryVertices
        | FlatFoldabilityResource::Faces
        | FlatFoldabilityResource::FaceBoundaryHalfEdges
        | FlatFoldabilityResource::Hinges
        | FlatFoldabilityResource::EdgeIncidenceRecords
        | FlatFoldabilityResource::LocalVertices
        | FlatFoldabilityResource::TotalRecords
        | FlatFoldabilityResource::SearchNodes
        | FlatFoldabilityResource::ExactOperations
        | FlatFoldabilityResource::CertificateBytes => {
            GlobalFlatFoldabilityUnknownReasonDto::WorkLimitReached
        }
    }
}

fn unknown_result(
    summary: GlobalFlatFoldabilitySummaryDto,
    reason: GlobalFlatFoldabilityUnknownReasonDto,
) -> GlobalFlatFoldabilityResultDto {
    GlobalFlatFoldabilityResultDto::Unknown { summary, reason }
}

fn finish_background_completion(app: &AppHandle, completion: GlobalFlatFoldabilityCompletion) {
    let project_state = app.state::<AppState>();
    let foldability_state = app.state::<GlobalFlatFoldabilityState>();
    let Ok(project) = lock_project(&project_state) else {
        finish_without_project(&foldability_state, completion);
        return;
    };
    let Ok(mut slot) = lock_foldability_state(&foldability_state) else {
        return;
    };
    finish_completion(&mut slot, &project, completion);
}

fn finish_without_project(
    state: &GlobalFlatFoldabilityState,
    completion: GlobalFlatFoldabilityCompletion,
) {
    let Ok(mut slot) = lock_foldability_state(state) else {
        return;
    };
    let Some(active) = slot
        .active
        .as_ref()
        .filter(|active| active.job_id == completion.context.job_id)
    else {
        return;
    };
    let summary = active.runtime.summary();
    slot.current_layer_order = None;
    slot.active = None;
    slot.terminal = Some(TerminalGlobalFlatFoldabilityJob {
        job_id: completion.context.job_id,
        dto: GlobalFlatFoldabilityJobDto::Failed {
            summary,
            error_category: GlobalFlatFoldabilityErrorCategory::InternalFailure,
        },
        claimed: false,
    });
}

fn finish_completion(
    slot: &mut GlobalFlatFoldabilitySlot,
    project: &ProjectState,
    completion: GlobalFlatFoldabilityCompletion,
) {
    let Some(active) = slot
        .active
        .as_ref()
        .filter(|active| active.job_id == completion.context.job_id)
    else {
        return;
    };
    if !bindings_equal(&active.binding, &completion.context.binding) {
        let summary = active.runtime.summary();
        return finish_as_internal_failure(slot, completion.context.job_id, summary);
    }

    // Capture the summary only after acquiring the slot. A progress request may
    // have observed a later elapsed time while completion waited for this lock.
    let summary = active.runtime.summary();
    let cancellation_requested = active.runtime.cancellation.load(Ordering::SeqCst);
    let current = binding_is_current(&completion.context.binding, project);
    let (dto, completed_layer_order) =
        if cancellation_requested || matches!(completion.outcome, WorkerOutcome::Cancelled) {
            (GlobalFlatFoldabilityJobDto::Cancelled { summary }, None)
        } else if !current {
            (GlobalFlatFoldabilityJobDto::Stale { summary }, None)
        } else {
            match completion.outcome {
                WorkerOutcome::Completed {
                    result,
                    layer_order,
                } => {
                    let result = result.with_summary(summary);
                    let adopted = match (&result, layer_order) {
                        (GlobalFlatFoldabilityResultDto::Possible { .. }, Some(layer_order)) => {
                            Some((Arc::clone(&completion.context.binding), *layer_order))
                        }
                        (
                            GlobalFlatFoldabilityResultDto::Impossible { .. }
                            | GlobalFlatFoldabilityResultDto::Unknown { .. },
                            None,
                        ) => None,
                        _ => {
                            return finish_as_internal_failure(
                                slot,
                                completion.context.job_id,
                                summary,
                            );
                        }
                    };
                    (GlobalFlatFoldabilityJobDto::Completed { result }, adopted)
                }
                WorkerOutcome::Cancelled => unreachable!("cancellation handled above"),
                WorkerOutcome::Failed(error_category) => (
                    GlobalFlatFoldabilityJobDto::Failed {
                        summary,
                        error_category,
                    },
                    None,
                ),
            }
        };

    let adopted_layer_order = match completed_layer_order {
        Some((binding, snapshot)) => {
            let Ok(certificate) = mint_current_layer_order_certificate(slot, binding, snapshot)
            else {
                return finish_as_internal_failure(slot, completion.context.job_id, summary);
            };
            Some(certificate)
        }
        None => None,
    };
    slot.current_layer_order = adopted_layer_order;
    slot.active = None;
    slot.terminal = Some(TerminalGlobalFlatFoldabilityJob {
        job_id: completion.context.job_id,
        dto,
        claimed: false,
    });
}

fn mint_current_layer_order_certificate(
    slot: &mut GlobalFlatFoldabilitySlot,
    binding: Arc<GlobalFlatFoldabilityBinding>,
    snapshot: LayerOrderSnapshot,
) -> Result<Arc<CurrentLayerOrderCertificate>, ()> {
    let generation = slot.layer_order_generation.checked_add(1).ok_or(())?;
    let snapshot = Arc::new(snapshot);
    let material_registry: Arc<[LayerFace]> =
        Arc::from(snapshot.material_faces.clone().into_boxed_slice());
    let claims = CurrentLayerOrderClaims {
        project_instance_id: binding.project_instance_id,
        project_id: binding.project_id,
        revision: binding.revision,
        topology_input: Arc::clone(&binding.topology_input),
        fold_model_fingerprint: Arc::clone(&binding.fold_model_fingerprint),
        proof_model_id: Arc::from(PROOF_MODEL_AUTHORITY_ID),
        layer_model_id: Arc::from(LAYER_MODEL_AUTHORITY_ID),
        snapshot_identity: Arc::clone(&snapshot),
        snapshot_provenance: snapshot.provenance,
        material_registry,
        generation,
    };
    let certificate = Arc::new(CurrentLayerOrderCertificate {
        binding,
        snapshot,
        claims,
    });
    if !current_layer_order_certificate_is_internally_consistent(&certificate) {
        return Err(());
    }
    slot.layer_order_generation = generation;
    Ok(certificate)
}

fn current_layer_order_certificate_is_internally_consistent(
    certificate: &CurrentLayerOrderCertificate,
) -> bool {
    let binding = &certificate.binding;
    let snapshot = &certificate.snapshot;
    let claims = &certificate.claims;
    let source = snapshot.provenance.source;

    claims.generation != 0
        && claims.project_instance_id == binding.project_instance_id
        && claims.project_id == binding.project_id
        && claims.revision == binding.revision
        && binding.topology_input.revision() == binding.revision
        && Arc::ptr_eq(&claims.topology_input, &binding.topology_input)
        && claims.topology_input.as_ref() == binding.topology_input.as_ref()
        && Arc::ptr_eq(
            &claims.fold_model_fingerprint,
            &binding.fold_model_fingerprint,
        )
        && claims.fold_model_fingerprint.as_ref() == binding.fold_model_fingerprint.as_ref()
        && claims.proof_model_id.as_ref() == PROOF_MODEL_AUTHORITY_ID
        && claims.layer_model_id.as_ref() == LAYER_MODEL_AUTHORITY_ID
        && Arc::ptr_eq(&claims.snapshot_identity, snapshot)
        && claims.snapshot_identity.as_ref() == snapshot.as_ref()
        && snapshot.model_id == LAYER_ORDER_MODEL_ID
        && source.model_id == GLOBAL_FLAT_FOLDABILITY_MODEL_ID
        && snapshot.provenance == claims.snapshot_provenance
        && source.identity_namespace == Some(binding.project_id)
        && source.source_revision == binding.revision
        && source.source_fingerprint.is_some_and(|fingerprint| {
            fingerprint.to_hex() == binding.fold_model_fingerprint.as_ref()
        })
        && !snapshot.material_faces.is_empty()
        && snapshot.material_faces.as_slice() == claims.material_registry.as_ref()
}

fn current_layer_order_claims_match(
    first: &CurrentLayerOrderClaims,
    second: &CurrentLayerOrderClaims,
) -> bool {
    first.project_instance_id == second.project_instance_id
        && first.project_id == second.project_id
        && first.revision == second.revision
        && Arc::ptr_eq(&first.topology_input, &second.topology_input)
        && first.topology_input.as_ref() == second.topology_input.as_ref()
        && Arc::ptr_eq(
            &first.fold_model_fingerprint,
            &second.fold_model_fingerprint,
        )
        && first.fold_model_fingerprint.as_ref() == second.fold_model_fingerprint.as_ref()
        && Arc::ptr_eq(&first.proof_model_id, &second.proof_model_id)
        && first.proof_model_id.as_ref() == second.proof_model_id.as_ref()
        && Arc::ptr_eq(&first.layer_model_id, &second.layer_model_id)
        && first.layer_model_id.as_ref() == second.layer_model_id.as_ref()
        && Arc::ptr_eq(&first.snapshot_identity, &second.snapshot_identity)
        && first.snapshot_identity.as_ref() == second.snapshot_identity.as_ref()
        && first.snapshot_provenance == second.snapshot_provenance
        && Arc::ptr_eq(&first.material_registry, &second.material_registry)
        && first.material_registry.as_ref() == second.material_registry.as_ref()
        && first.generation == second.generation
}

fn finish_as_internal_failure(
    slot: &mut GlobalFlatFoldabilitySlot,
    job_id: GlobalFlatFoldabilityJobId,
    summary: GlobalFlatFoldabilitySummaryDto,
) {
    slot.current_layer_order = None;
    slot.active = None;
    slot.terminal = Some(TerminalGlobalFlatFoldabilityJob {
        job_id,
        dto: GlobalFlatFoldabilityJobDto::Failed {
            summary,
            error_category: GlobalFlatFoldabilityErrorCategory::InternalFailure,
        },
        claimed: false,
    });
}

fn binding_is_current(binding: &GlobalFlatFoldabilityBinding, project: &ProjectState) -> bool {
    let current_fingerprint = project.editor.fold_model_fingerprint_v1();
    binding.project_instance_id == project.instance_id
        && binding.project_id == project.project_id
        && binding.revision == project.editor.revision()
        && binding
            .topology_input
            .is_current_for(project.project_id, &project.editor)
        && binding.fold_model_fingerprint.as_ref() == current_fingerprint
}

fn bindings_equal(
    first: &Arc<GlobalFlatFoldabilityBinding>,
    second: &Arc<GlobalFlatFoldabilityBinding>,
) -> bool {
    Arc::ptr_eq(first, second)
}

fn active_job_dto(
    runtime: &GlobalFlatFoldabilityRuntime,
) -> Result<GlobalFlatFoldabilityJobDto, GlobalFlatFoldabilityCommandError> {
    let progress = runtime.progress().map_err(|()| {
        GlobalFlatFoldabilityCommandError::new(GlobalFlatFoldabilityErrorCategory::InternalFailure)
    })?;
    let cancel_requested = runtime.cancellation.load(Ordering::SeqCst);
    if runtime.worker_started.load(Ordering::SeqCst) {
        Ok(GlobalFlatFoldabilityJobDto::Running {
            cancel_requested,
            progress,
        })
    } else {
        Ok(GlobalFlatFoldabilityJobDto::Queued {
            cancel_requested,
            progress,
        })
    }
}

fn current_terminal_job_mut(
    slot: &mut GlobalFlatFoldabilitySlot,
    job_id: GlobalFlatFoldabilityJobId,
) -> Result<&mut TerminalGlobalFlatFoldabilityJob, GlobalFlatFoldabilityCommandError> {
    match slot.terminal.as_mut() {
        Some(terminal) if terminal.job_id == job_id => Ok(terminal),
        Some(_) | None => Err(GlobalFlatFoldabilityCommandError::new(
            GlobalFlatFoldabilityErrorCategory::ResultUnavailable,
        )),
    }
}

fn claim_terminal_result(
    slot: &mut GlobalFlatFoldabilitySlot,
    job_id: GlobalFlatFoldabilityJobId,
) -> Result<GlobalFlatFoldabilityJobDto, GlobalFlatFoldabilityCommandError> {
    let terminal = current_terminal_job_mut(slot, job_id)?;
    if terminal.claimed {
        return Err(GlobalFlatFoldabilityCommandError::new(
            GlobalFlatFoldabilityErrorCategory::ResultUnavailable,
        ));
    }
    terminal.claimed = true;
    Ok(terminal.dto.clone())
}

fn cancel_job(
    slot: &mut GlobalFlatFoldabilitySlot,
    job_id: GlobalFlatFoldabilityJobId,
) -> Result<(), GlobalFlatFoldabilityCommandError> {
    if let Some(active) = slot.active.as_ref() {
        if active.job_id == job_id {
            active.runtime.cancellation.store(true, Ordering::SeqCst);
            slot.last_cancelled_id = Some(job_id);
            slot.current_layer_order = None;
            return Ok(());
        }
        if slot.last_cancelled_id == Some(job_id) || slot.last_replaced_id == Some(job_id) {
            return Ok(());
        }
        return Err(GlobalFlatFoldabilityCommandError::new(
            GlobalFlatFoldabilityErrorCategory::ResultUnavailable,
        ));
    }
    if slot
        .terminal
        .as_ref()
        .is_some_and(|terminal| terminal.job_id == job_id)
    {
        // Completion has already won the race. Cancellation is idempotent for
        // the caller, but must not revoke a certificate produced by that
        // completed Possible result or relabel the terminal job as cancelled.
        return Ok(());
    }
    if slot.last_cancelled_id == Some(job_id) {
        return Ok(());
    }
    Err(GlobalFlatFoldabilityCommandError::new(
        GlobalFlatFoldabilityErrorCategory::ResultUnavailable,
    ))
}

/// Captures a private native capability without cloning the layer snapshot.
///
/// This is intentionally not a Tauri command and its return type is not
/// serializable. A mutating caller must pass it to
/// `with_revalidated_current_layer_order_capability`.
#[allow(dead_code)]
pub(super) fn capture_current_layer_order_capability(
    state: &GlobalFlatFoldabilityState,
    project: &ProjectState,
) -> Result<Option<CurrentLayerOrderCapability>, GlobalFlatFoldabilityCommandError> {
    let slot = lock_foldability_state(state)?;
    let Some(certificate) = slot.current_layer_order.as_ref() else {
        return Ok(None);
    };
    if certificate.claims.generation != slot.layer_order_generation
        || !current_layer_order_certificate_is_internally_consistent(certificate)
        || !binding_is_current(&certificate.binding, project)
    {
        return Ok(None);
    }
    Ok(Some(CurrentLayerOrderCapability {
        slot: Arc::clone(&state.0),
        certificate: Arc::clone(certificate),
        claims: certificate.claims.clone(),
    }))
}

/// Revalidates every sealed authority claim against the live slot and project.
///
/// This borrow-returning helper is observation-only: the slot lock is released
/// before the caller receives the snapshot, so it must never authorize a
/// mutation. Use `with_revalidated_current_layer_order_capability` at a native
/// commit boundary.
#[allow(dead_code)]
pub(super) fn revalidate_current_layer_order_capability<'a>(
    state: &GlobalFlatFoldabilityState,
    project: &ProjectState,
    capability: &'a CurrentLayerOrderCapability,
) -> Result<Option<&'a LayerOrderSnapshot>, GlobalFlatFoldabilityCommandError> {
    if !Arc::ptr_eq(&state.0, &capability.slot) {
        return Ok(None);
    }
    let slot = lock_foldability_state(state)?;
    let Some(current) = slot.current_layer_order.as_ref() else {
        return Ok(None);
    };
    if !current_layer_order_capability_matches_locked_slot(&slot, project, capability, current) {
        return Ok(None);
    }
    Ok(Some(capability.certificate.snapshot.as_ref()))
}

/// Runs a native layer-aware observation while project and authority stay locked.
///
/// The global lock order is project (`AppState`) first, then the layer-order
/// slot. Background completion uses the same order; cancellation only takes
/// the slot. Holding both guards through `action` prevents cancellation,
/// replacement analysis, project editing, and reopen from creating a
/// revalidate-to-observe race. The project is deliberately shared-only: a
/// future mutation that also needs current pose authority must use a dedicated
/// `project -> pose -> layer-order` combined commit helper.
#[allow(dead_code)]
pub(super) fn with_revalidated_current_layer_order_capability<R>(
    app_state: &AppState,
    foldability_state: &GlobalFlatFoldabilityState,
    capability: &CurrentLayerOrderCapability,
    action: impl FnOnce(&ProjectState, &LayerOrderSnapshot) -> R,
) -> Result<Option<R>, GlobalFlatFoldabilityCommandError> {
    if !Arc::ptr_eq(&foldability_state.0, &capability.slot) {
        return Ok(None);
    }
    // Do not reverse this order anywhere that needs both locks.
    let project = lock_project(app_state).map_err(|_| {
        GlobalFlatFoldabilityCommandError::new(GlobalFlatFoldabilityErrorCategory::InternalFailure)
    })?;
    let slot = lock_foldability_state(foldability_state)?;
    let Some(current) = slot.current_layer_order.as_ref() else {
        return Ok(None);
    };
    if !current_layer_order_capability_matches_locked_slot(&slot, &project, capability, current) {
        return Ok(None);
    }

    let output = action(&project, capability.certificate.snapshot.as_ref());
    // Keep the layer slot locked for the complete action, even if future
    // compiler lifetime shortening would otherwise make the guard look dead.
    drop(slot);
    Ok(Some(output))
}

fn current_layer_order_capability_matches_locked_slot(
    slot: &GlobalFlatFoldabilitySlot,
    project: &ProjectState,
    capability: &CurrentLayerOrderCapability,
    current: &Arc<CurrentLayerOrderCertificate>,
) -> bool {
    Arc::ptr_eq(current, &capability.certificate)
        && current.claims.generation == slot.layer_order_generation
        && capability.claims.generation == slot.layer_order_generation
        && current_layer_order_claims_match(&capability.claims, &current.claims)
        && current_layer_order_certificate_is_internally_consistent(current)
        && binding_is_current(&current.binding, project)
}

fn usize_to_bounded_u64(value: usize, maximum: u64) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX).min(maximum)
}

fn usize_to_exact_bounded_u64(value: usize, maximum: u64) -> Option<u64> {
    u64::try_from(value).ok().filter(|value| *value <= maximum)
}

fn duration_millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
pub(super) mod tests {
    use std::{
        cell::Cell,
        path::PathBuf,
        sync::mpsc::{self, RecvTimeoutError},
        thread,
    };

    use ori_core::{Command, EditorState, analyze_local_flat_foldability};
    use ori_domain::{Edge, EdgeId, EdgeKind, Point2, Vertex, VertexId};
    use ori_foldability::{FacewiseConstraintKind, LocalNecessaryConditionViolation};
    use serde_json::json;

    use super::*;
    use crate::initial_project_state;

    fn source_for(
        project: &ProjectState,
        job_id: GlobalFlatFoldabilityJobId,
    ) -> GlobalFlatFoldabilitySource {
        let expected_fingerprint = project.editor.fold_model_fingerprint_v1();
        let capture = capture_source(
            project,
            project.instance_id,
            project.project_id,
            project.editor.revision(),
            &expected_fingerprint,
            job_id,
            Arc::new(GlobalFlatFoldabilityRuntime::new(30_000)),
            native_limits(),
        )
        .expect("capture fixture");
        let GlobalFlatFoldabilityCapture::Ready(source) = capture else {
            panic!("ordinary fixture must be within native source limits");
        };
        source
    }

    pub(crate) fn install_possible_layer_order(
        state: &GlobalFlatFoldabilityState,
        project: &ProjectState,
    ) {
        let source = source_for(project, GlobalFlatFoldabilityJobId::new());
        {
            let mut slot = lock_foldability_state(state).expect("lock authority slot");
            install_active_job(&mut slot, &source);
        }
        let completion = guarded_run_worker(source);
        {
            let mut slot = lock_foldability_state(state).expect("lock completed authority slot");
            finish_completion(&mut slot, project, completion);
            assert!(
                slot.current_layer_order.is_some(),
                "fixture must install native layer authority"
            );
        }
    }

    #[test]
    fn archived_flat_snapshot_is_recomputed_before_native_install() {
        let project = initial_project_state();
        let state = GlobalFlatFoldabilityState::default();
        install_possible_layer_order(&state, &project);
        let canonical = {
            let slot = lock_foldability_state(&state).unwrap();
            serde_json::to_string(
                slot.current_layer_order
                    .as_ref()
                    .expect("installed layer order")
                    .snapshot
                    .as_ref(),
            )
            .unwrap()
        };

        let restored = revalidate_archived_flat_layer_evidence(&project, &canonical).unwrap();
        let CurrentLayerEvidence::CertifiedFlat(snapshot) = restored else {
            panic!("flat archive must restore flat evidence");
        };
        assert_eq!(serde_json::to_string(&snapshot).unwrap(), canonical);

        let mut stale: serde_json::Value = serde_json::from_str(&canonical).unwrap();
        stale["model_id"] = serde_json::Value::String("forged".to_owned());
        assert!(revalidate_archived_flat_layer_evidence(&project, &stale.to_string()).is_err());
    }

    fn copy_layer_order_capability(
        capability: &CurrentLayerOrderCapability,
    ) -> CurrentLayerOrderCapability {
        CurrentLayerOrderCapability {
            slot: Arc::clone(&capability.slot),
            certificate: Arc::clone(&capability.certificate),
            claims: capability.claims.clone(),
        }
    }

    #[test]
    fn certified_target_preflight_rejects_every_stale_binding_without_project_mutation() {
        let project = initial_project_state();
        let state = GlobalFlatFoldabilityState::default();
        install_possible_layer_order(&state, &project);
        let capability = capture_current_layer_order_capability(&state, &project)
            .unwrap()
            .expect("current layer capability");
        let snapshot = capability.snapshot().clone();
        let fingerprint = snapshot
            .provenance
            .source
            .source_fingerprint
            .expect("bound fingerprint")
            .0;
        let revision = project.editor.revision();
        let guard = lock_revalidated_current_layer_order_for_commit(&state, &project, &capability)
            .unwrap()
            .expect("commit guard");
        assert!(guard.preflight_certified_target(
            project.project_id,
            revision,
            fingerprint,
            &snapshot,
        ));
        assert!(!guard.preflight_certified_target(
            ProjectId::new(),
            revision,
            fingerprint,
            &snapshot,
        ));
        assert!(!guard.preflight_certified_target(
            project.project_id,
            revision + 1,
            fingerprint,
            &snapshot,
        ));
        assert!(!guard.preflight_certified_target(
            project.project_id,
            revision,
            [0xa5; 32],
            &snapshot,
        ));
        assert_eq!(project.editor.revision(), revision);
    }

    #[test]
    fn certified_target_install_remints_generation_and_rejects_old_aba_capability() {
        let project = initial_project_state();
        let state = GlobalFlatFoldabilityState::default();
        install_possible_layer_order(&state, &project);
        let old = capture_current_layer_order_capability(&state, &project)
            .unwrap()
            .expect("old capability");
        let old_generation = old.generation();
        let snapshot = old.snapshot().clone();
        let guard = lock_revalidated_current_layer_order_for_commit(&state, &project, &old)
            .unwrap()
            .expect("commit guard");
        guard
            .install_certified_target_after_project_mutation(&project, snapshot)
            .expect("remint target certificate");
        assert!(
            revalidate_current_layer_order_capability(&state, &project, &old)
                .unwrap()
                .is_none(),
            "old Arc identity must not survive a remint"
        );
        let current = capture_current_layer_order_capability(&state, &project)
            .unwrap()
            .expect("reminted capability");
        assert_eq!(current.generation(), old_generation + 1);
        assert!(
            revalidate_current_layer_order_capability(&state, &project, &current)
                .unwrap()
                .is_some()
        );
    }

    fn deep_clone_layer_order_certificate(
        certificate: &CurrentLayerOrderCertificate,
    ) -> CurrentLayerOrderCertificate {
        let snapshot = Arc::new((*certificate.snapshot).clone());
        let mut claims = certificate.claims.clone();
        claims.snapshot_identity = Arc::clone(&snapshot);
        CurrentLayerOrderCertificate {
            binding: Arc::clone(&certificate.binding),
            snapshot,
            claims,
        }
    }

    struct DeadlineAtFirstCoreCheckpoint;

    impl GlobalFlatFoldabilityObserver for DeadlineAtFirstCoreCheckpoint {
        fn checkpoint(&mut self) -> GlobalFlatFoldabilityCheckpoint {
            GlobalFlatFoldabilityCheckpoint::DeadlineReached
        }
    }

    fn core_report_for(
        project: &ProjectState,
        topology: &TopologySnapshot,
        limits: GlobalFlatFoldabilityLimits,
    ) -> GlobalFlatFoldabilityReport {
        let local =
            analyze_local_flat_foldability(project.editor.paper(), project.editor.pattern());
        ori_core::analyze_global_flat_foldability(
            GlobalFlatFoldabilityInput::current_with_geometry(
                project.project_id,
                project.editor.paper(),
                project.editor.pattern(),
                topology,
                &local,
            ),
            limits,
        )
        .expect("core fixture analysis")
    }

    fn unknown_completion(source: &GlobalFlatFoldabilitySource) -> GlobalFlatFoldabilityCompletion {
        GlobalFlatFoldabilityCompletion {
            context: completion_context(source),
            outcome: WorkerOutcome::Completed {
                result: unknown_result(
                    source.runtime.summary(),
                    GlobalFlatFoldabilityUnknownReasonDto::ProofNotCompleted,
                ),
                layer_order: None,
            },
        }
    }

    fn centered_single_hinge_project() -> ProjectState {
        let positions = [
            Point2::new(0.0, 0.0),
            Point2::new(200.0, 0.0),
            Point2::new(400.0, 0.0),
            Point2::new(400.0, 400.0),
            Point2::new(200.0, 400.0),
            Point2::new(0.0, 400.0),
        ];
        let vertices = positions
            .into_iter()
            .map(|position| Vertex {
                id: VertexId::new(),
                position,
            })
            .collect::<Vec<_>>();
        let mut edges = (0..vertices.len())
            .map(|index| Edge {
                id: EdgeId::new(),
                start: vertices[index].id,
                end: vertices[(index + 1) % vertices.len()].id,
                kind: EdgeKind::Boundary,
            })
            .collect::<Vec<_>>();
        edges.push(Edge {
            id: EdgeId::new(),
            start: vertices[1].id,
            end: vertices[4].id,
            kind: EdgeKind::Mountain,
        });
        let paper = Paper {
            boundary_vertices: vertices.iter().map(|vertex| vertex.id).collect(),
            ..Paper::default()
        };
        ProjectState::new_with_paper(CreasePattern { vertices, edges }, paper)
    }

    fn four_ray_local_violation_project() -> (ProjectState, VertexId) {
        let boundary_positions = [
            Point2::new(0.0, 0.0),
            Point2::new(10.0, 0.0),
            Point2::new(20.0, 0.0),
            Point2::new(20.0, 10.0),
            Point2::new(20.0, 20.0),
            Point2::new(10.0, 20.0),
            Point2::new(0.0, 20.0),
            Point2::new(0.0, 10.0),
        ];
        let mut vertices = boundary_positions
            .into_iter()
            .map(|position| Vertex {
                id: VertexId::new(),
                position,
            })
            .collect::<Vec<_>>();
        let center = Vertex {
            id: VertexId::new(),
            position: Point2::new(10.0, 10.0),
        };
        let center_id = center.id;
        vertices.push(center);

        let mut edges = (0..boundary_positions.len())
            .map(|index| Edge {
                id: EdgeId::new(),
                start: vertices[index].id,
                end: vertices[(index + 1) % boundary_positions.len()].id,
                kind: EdgeKind::Boundary,
            })
            .collect::<Vec<_>>();
        edges.extend(
            [3_usize, 5, 7, 0]
                .into_iter()
                .zip([
                    EdgeKind::Mountain,
                    EdgeKind::Mountain,
                    EdgeKind::Mountain,
                    EdgeKind::Valley,
                ])
                .map(|(endpoint, kind)| Edge {
                    id: EdgeId::new(),
                    start: center_id,
                    end: vertices[endpoint].id,
                    kind,
                }),
        );
        let paper = Paper {
            boundary_vertices: vertices[..boundary_positions.len()]
                .iter()
                .map(|vertex| vertex.id)
                .collect(),
            ..Paper::default()
        };
        (
            ProjectState::new_with_paper(CreasePattern { vertices, edges }, paper),
            center_id,
        )
    }

    fn simulation_topology(project: &ProjectState) -> TopologySnapshot {
        project
            .editor
            .topology_analysis_input(project.project_id)
            .analyze()
            .simulation_snapshot()
            .cloned()
            .expect("simulation-ready topology")
    }

    fn proof_work_counts(
        face_records: usize,
        overlap_face_pairs: usize,
        constraints: usize,
    ) -> GlobalFlatFoldabilityWorkCounts {
        GlobalFlatFoldabilityWorkCounts {
            source_vertex_records: 0,
            source_edge_records: 0,
            paper_boundary_vertex_records: 0,
            face_records,
            face_boundary_half_edges: 0,
            hinge_records: 0,
            edge_incidence_records: 0,
            local_vertex_records: 0,
            total_records: 0,
            overlap_face_pairs,
            arrangement_segments: 0,
            overlap_cells: 0,
            constraints,
            search_nodes: 0,
            exact_operations: 0,
            exact_values: 0,
            certificate_bytes: 0,
        }
    }

    #[test]
    fn time_limit_accepts_exact_endpoints_and_rejects_neighbors() {
        assert!(validate_time_limit(MIN_TIME_LIMIT_MS).is_ok());
        assert!(validate_time_limit(MAX_TIME_LIMIT_MS).is_ok());
        assert_eq!(
            validate_time_limit(MIN_TIME_LIMIT_MS - 1)
                .expect_err("below minimum")
                .category,
            GlobalFlatFoldabilityErrorCategory::InvalidRequest
        );
        assert_eq!(
            validate_time_limit(MAX_TIME_LIMIT_MS + 1)
                .expect_err("above maximum")
                .category,
            GlobalFlatFoldabilityErrorCategory::InvalidRequest
        );
    }

    #[test]
    fn fold_model_fingerprint_request_is_closed_and_lowercase() {
        assert!(validate_fold_model_fingerprint(&"a".repeat(64)).is_ok());
        for invalid in [
            "a".repeat(63),
            "a".repeat(65),
            "A".repeat(64),
            "g".repeat(64),
            "０".repeat(64),
        ] {
            assert_eq!(
                validate_fold_model_fingerprint(&invalid)
                    .expect_err("invalid fingerprint")
                    .category,
                GlobalFlatFoldabilityErrorCategory::InvalidRequest
            );
        }
    }

    #[test]
    fn source_preflight_is_inclusive_but_cannot_bypass_fingerprint_binding() {
        let base = initial_project_state();
        let vertex_count = base.editor.pattern().vertices.len();
        let edge_count = base.editor.pattern().edges.len();
        let boundary_count = base.editor.paper().boundary_vertices.len();
        let exact_limits = GlobalFlatFoldabilityLimits {
            max_source_vertices: vertex_count,
            max_source_edges: edge_count,
            max_paper_boundary_vertices: boundary_count,
            max_total_records: vertex_count + edge_count + boundary_count,
            ..GlobalFlatFoldabilityLimits::default()
        };
        assert_eq!(
            source_record_limit_violation(base.editor.pattern(), base.editor.paper(), exact_limits,),
            None,
            "each exact source boundary is admitted"
        );

        let mut pattern = base.editor.pattern().clone();
        pattern.vertices.push(Vertex {
            id: VertexId::new(),
            position: Point2::new(1.0, 1.0),
        });
        let project = ProjectState::new_with_paper(pattern, base.editor.paper().clone());
        let violation = source_record_limit_violation(
            project.editor.pattern(),
            project.editor.paper(),
            exact_limits,
        )
        .expect("limit + 1 is rejected");
        assert_eq!(violation.resource, FlatFoldabilityResource::SourceVertices);
        assert_eq!(violation.limit, vertex_count);
        assert_eq!(violation.observed, vertex_count + 1);

        let stale = match capture_source(
            &project,
            project.instance_id,
            project.project_id,
            project.editor.revision(),
            &"f".repeat(64),
            GlobalFlatFoldabilityJobId::new(),
            Arc::new(GlobalFlatFoldabilityRuntime::new(30_000)),
            exact_limits,
        ) {
            Ok(_) => panic!("over-limit input must still reject a stale fingerprint"),
            Err(error) => error,
        };
        assert_eq!(
            stale.category,
            GlobalFlatFoldabilityErrorCategory::SnapshotUnavailable
        );

        let capture = capture_source(
            &project,
            project.instance_id,
            project.project_id,
            project.editor.revision(),
            &project.editor.fold_model_fingerprint_v1(),
            GlobalFlatFoldabilityJobId::new(),
            Arc::new(GlobalFlatFoldabilityRuntime::new(30_000)),
            exact_limits,
        )
        .expect("bound over-limit input returns a conservative result");
        assert!(matches!(
            capture,
            GlobalFlatFoldabilityCapture::SourceLimit { violation, .. }
                if violation.resource == FlatFoldabilityResource::SourceVertices
        ));
    }

    #[test]
    fn normal_capture_requires_expected_fingerprint_and_shares_one_binding() {
        let project = initial_project_state();
        let wrong_instance = match capture_source(
            &project,
            ProjectId::new(),
            project.project_id,
            project.editor.revision(),
            &project.editor.fold_model_fingerprint_v1(),
            GlobalFlatFoldabilityJobId::new(),
            Arc::new(GlobalFlatFoldabilityRuntime::new(30_000)),
            native_limits(),
        ) {
            Ok(_) => panic!("a reopened project instance must reject the old begin request"),
            Err(error) => error,
        };
        assert_eq!(
            wrong_instance.category,
            GlobalFlatFoldabilityErrorCategory::SnapshotUnavailable
        );

        let mismatch = match capture_source(
            &project,
            project.instance_id,
            project.project_id,
            project.editor.revision(),
            &"f".repeat(64),
            GlobalFlatFoldabilityJobId::new(),
            Arc::new(GlobalFlatFoldabilityRuntime::new(30_000)),
            native_limits(),
        ) {
            Ok(_) => panic!("a current in-range source needs an exact expected fingerprint"),
            Err(error) => error,
        };
        assert_eq!(
            mismatch.category,
            GlobalFlatFoldabilityErrorCategory::SnapshotUnavailable
        );

        let source = source_for(&project, GlobalFlatFoldabilityJobId::new());
        let context = completion_context(&source);
        let mut slot = GlobalFlatFoldabilitySlot::default();
        install_active_job(&mut slot, &source);
        let active = slot.active.as_ref().expect("active job");
        assert!(Arc::ptr_eq(&source.binding, &context.binding));
        assert!(Arc::ptr_eq(&source.binding, &active.binding));
    }

    #[test]
    fn stale_capture_after_same_content_reopen_preserves_new_authority() {
        let original = initial_project_state();
        let old_source = source_for(&original, GlobalFlatFoldabilityJobId::new());
        let reopened = ProjectState::from_document(
            original.document(),
            PathBuf::from("same-content-reopen.ori2"),
        );
        assert_eq!(reopened.project_id, original.project_id);
        assert_eq!(reopened.editor.revision(), original.editor.revision());
        assert_eq!(
            reopened.editor.fold_model_fingerprint_v1(),
            original.editor.fold_model_fingerprint_v1()
        );
        assert_ne!(reopened.instance_id, original.instance_id);

        let app_state = Arc::new(AppState::new(reopened));
        let foldability_state = Arc::new(GlobalFlatFoldabilityState::default());
        {
            let project = lock_project(&app_state).expect("lock reopened project");
            install_possible_layer_order(&foldability_state, &project);
        }
        let (
            terminal_id,
            terminal_dto,
            terminal_claimed,
            certificate,
            generation,
            last_cancelled_id,
            last_replaced_id,
        ) = {
            let slot = lock_foldability_state(&foldability_state).expect("capture new authority");
            let terminal = slot
                .terminal
                .as_ref()
                .expect("new authority has a terminal result");
            (
                terminal.job_id,
                terminal.dto.clone(),
                terminal.claimed,
                Arc::clone(
                    slot.current_layer_order
                        .as_ref()
                        .expect("new authority certificate"),
                ),
                slot.layer_order_generation,
                slot.last_cancelled_id,
                slot.last_replaced_id,
            )
        };

        let old_app_state = Arc::clone(&app_state);
        let old_foldability_state = Arc::clone(&foldability_state);
        let (attempted_tx, attempted_rx) = mpsc::channel();
        let old_install = thread::spawn(move || {
            attempted_tx.send(()).expect("announce old install");
            let project = lock_project(&old_app_state).expect("lock current project");
            let mut slot =
                lock_foldability_state(&old_foldability_state).expect("lock current authority");
            install_captured_source_if_current(&mut slot, &project, &old_source)
        });
        attempted_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("old install attempt starts");
        assert_eq!(
            old_install.join().expect("join old install"),
            Err(GlobalFlatFoldabilityCommandError::new(
                GlobalFlatFoldabilityErrorCategory::SnapshotUnavailable
            ))
        );

        let slot = lock_foldability_state(&foldability_state).expect("inspect new authority");
        assert!(slot.active.is_none());
        assert_eq!(
            slot.terminal.as_ref().map(|terminal| terminal.job_id),
            Some(terminal_id)
        );
        assert_eq!(
            slot.terminal.as_ref().map(|terminal| &terminal.dto),
            Some(&terminal_dto)
        );
        assert_eq!(
            slot.terminal.as_ref().map(|terminal| terminal.claimed),
            Some(terminal_claimed)
        );
        assert_eq!(slot.layer_order_generation, generation);
        assert_eq!(slot.last_cancelled_id, last_cancelled_id);
        assert_eq!(slot.last_replaced_id, last_replaced_id);
        assert!(
            Arc::ptr_eq(
                slot.current_layer_order
                    .as_ref()
                    .expect("new certificate survives"),
                &certificate,
            ),
            "a same-content old capture must not erase or replace new authority"
        );
    }

    #[test]
    fn project_lock_linearizes_same_revision_begin_install_and_later_begin_wins() {
        let app_state = Arc::new(AppState::new(initial_project_state()));
        let foldability_state = Arc::new(GlobalFlatFoldabilityState::default());
        let (project_instance_id, project_id, revision, fingerprint) = {
            let project = lock_project(&app_state).expect("read begin request");
            (
                project.instance_id,
                project.project_id,
                project.editor.revision(),
                project.editor.fold_model_fingerprint_v1(),
            )
        };
        let first_job_id = GlobalFlatFoldabilityJobId::new();
        let first_runtime = Arc::new(GlobalFlatFoldabilityRuntime::new(30_000));
        let (first_captured_tx, first_captured_rx) = mpsc::channel();
        let (release_first_tx, release_first_rx) = mpsc::channel();
        let first_app_state = Arc::clone(&app_state);
        let first_foldability_state = Arc::clone(&foldability_state);
        let first_fingerprint = fingerprint.clone();
        let first_runtime_for_thread = Arc::clone(&first_runtime);
        let first_thread = thread::spawn(move || {
            prepare_global_flat_foldability_begin(
                &first_app_state,
                &first_foldability_state,
                project_instance_id,
                project_id,
                revision,
                &first_fingerprint,
                first_job_id,
                first_runtime_for_thread,
                native_limits(),
                || {
                    first_captured_tx
                        .send(())
                        .expect("announce first capture while project stays locked");
                    release_first_rx
                        .recv()
                        .expect("release first begin to install");
                },
            )
            .expect("prepare first begin")
        });
        first_captured_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("first begin captures source");

        let (second_attempted_tx, second_attempted_rx) = mpsc::channel();
        let (second_captured_tx, second_captured_rx) = mpsc::channel();
        let second_app_state = Arc::clone(&app_state);
        let second_foldability_state = Arc::clone(&foldability_state);
        let second_callback_state = Arc::clone(&foldability_state);
        let second_fingerprint = fingerprint;
        let second_job_id = GlobalFlatFoldabilityJobId::new();
        let second_thread = thread::spawn(move || {
            second_attempted_tx
                .send(())
                .expect("announce second project-lock attempt");
            prepare_global_flat_foldability_begin(
                &second_app_state,
                &second_foldability_state,
                project_instance_id,
                project_id,
                revision,
                &second_fingerprint,
                second_job_id,
                Arc::new(GlobalFlatFoldabilityRuntime::new(30_000)),
                native_limits(),
                || {
                    let slot = lock_foldability_state(&second_callback_state)
                        .expect("inspect authority at second capture");
                    assert_eq!(
                        slot.active.as_ref().map(|active| active.job_id),
                        Some(first_job_id),
                        "the first install must linearize before the project lock admits a second capture"
                    );
                    drop(slot);
                    second_captured_tx
                        .send(())
                        .expect("announce second capture after first install");
                },
            )
            .expect("prepare second begin")
        });
        second_attempted_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("second begin attempts the project lock");
        assert_eq!(
            second_captured_rx.recv_timeout(Duration::from_millis(100)),
            Err(RecvTimeoutError::Timeout),
            "the second same-revision begin must not pass capture while the first holds project"
        );

        release_first_tx.send(()).expect("release first begin");
        assert!(matches!(
            first_thread.join().expect("join first begin"),
            PreparedGlobalFlatFoldabilityBegin::Registered { source, .. }
                if source.job_id == first_job_id
        ));
        second_captured_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("second begin captures after the first install");
        assert!(matches!(
            second_thread.join().expect("join second begin"),
            PreparedGlobalFlatFoldabilityBegin::Registered { source, .. }
                if source.job_id == second_job_id
        ));

        let slot = lock_foldability_state(&foldability_state).expect("inspect linearized begins");
        assert_eq!(
            slot.active.as_ref().map(|active| active.job_id),
            Some(second_job_id)
        );
        assert_eq!(slot.last_replaced_id, Some(first_job_id));
        assert!(
            first_runtime.cancellation.load(Ordering::SeqCst),
            "the later project-lock acquirer replaces and cancels the first begin"
        );
        assert!(slot.terminal.is_none());
        assert!(slot.current_layer_order.is_none());
    }

    #[test]
    fn source_limit_begin_preserves_every_occupied_slot_field() {
        let app_state = Arc::new(AppState::new(initial_project_state()));
        let foldability_state = Arc::new(GlobalFlatFoldabilityState::default());
        let (project_instance_id, project_id, revision, fingerprint, active_source) = {
            let project = lock_project(&app_state).expect("lock source-limit project");
            install_possible_layer_order(&foldability_state, &project);
            (
                project.instance_id,
                project.project_id,
                project.editor.revision(),
                project.editor.fold_model_fingerprint_v1(),
                source_for(&project, GlobalFlatFoldabilityJobId::new()),
            )
        };
        let active_job_id = active_source.job_id;
        let active_binding = Arc::clone(&active_source.binding);
        let active_runtime = Arc::clone(&active_source.runtime);
        let last_cancelled_id = GlobalFlatFoldabilityJobId::new();
        let last_replaced_id = GlobalFlatFoldabilityJobId::new();
        let (terminal_job_id, terminal_dto, terminal_claimed, certificate, generation) = {
            let mut slot =
                lock_foldability_state(&foldability_state).expect("occupy every slot field");
            slot.active = Some(ActiveGlobalFlatFoldabilityJob {
                job_id: active_job_id,
                binding: Arc::clone(&active_binding),
                runtime: Arc::clone(&active_runtime),
            });
            slot.last_cancelled_id = Some(last_cancelled_id);
            slot.last_replaced_id = Some(last_replaced_id);
            let terminal = slot
                .terminal
                .as_ref()
                .expect("possible analysis installs terminal result");
            (
                terminal.job_id,
                terminal.dto.clone(),
                terminal.claimed,
                Arc::clone(
                    slot.current_layer_order
                        .as_ref()
                        .expect("possible analysis installs current certificate"),
                ),
                slot.layer_order_generation,
            )
        };

        let job_id = GlobalFlatFoldabilityJobId::new();
        let runtime = Arc::new(GlobalFlatFoldabilityRuntime::new(30_000));
        let callback_called = Cell::new(false);
        let limits = GlobalFlatFoldabilityLimits {
            max_source_vertices: 0,
            ..native_limits()
        };
        let prepared = prepare_global_flat_foldability_begin(
            &app_state,
            &foldability_state,
            project_instance_id,
            project_id,
            revision,
            &fingerprint,
            job_id,
            runtime,
            limits,
            || callback_called.set(true),
        )
        .expect("source limit is an unregistered bounded result");
        assert!(callback_called.get(), "capture synchronization hook runs");
        let PreparedGlobalFlatFoldabilityBegin::UnregisteredSourceLimit(response) = prepared else {
            panic!("over-limit source must not register a worker");
        };
        assert_eq!(response.job_id, job_id);
        assert!(matches!(
            response.job,
            GlobalFlatFoldabilityJobDto::Completed {
                result: GlobalFlatFoldabilityResultDto::Unknown {
                    reason: GlobalFlatFoldabilityUnknownReasonDto::WorkLimitReached,
                    ..
                }
            }
        ));

        let slot =
            lock_foldability_state(&foldability_state).expect("inspect untouched occupied slot");
        let active = slot.active.as_ref().expect("active job survives");
        assert_eq!(active.job_id, active_job_id);
        assert!(Arc::ptr_eq(&active.binding, &active_binding));
        assert!(Arc::ptr_eq(&active.runtime, &active_runtime));
        assert!(
            !active.runtime.cancellation.load(Ordering::SeqCst),
            "source preflight must not cancel the existing active job"
        );
        let terminal = slot.terminal.as_ref().expect("terminal job survives");
        assert_eq!(terminal.job_id, terminal_job_id);
        assert_eq!(terminal.dto, terminal_dto);
        assert_eq!(terminal.claimed, terminal_claimed);
        assert_eq!(slot.last_cancelled_id, Some(last_cancelled_id));
        assert_eq!(slot.last_replaced_id, Some(last_replaced_id));
        assert_eq!(slot.layer_order_generation, generation);
        assert!(Arc::ptr_eq(
            slot.current_layer_order
                .as_ref()
                .expect("current certificate survives"),
            &certificate
        ));
    }

    #[test]
    fn certificate_counts_reject_overflow_instead_of_clamping() {
        assert_eq!(usize_to_exact_bounded_u64(0, 2), Some(0));
        assert_eq!(usize_to_exact_bounded_u64(2, 2), Some(2));
        assert_eq!(usize_to_exact_bounded_u64(3, 2), None);
    }

    #[test]
    fn phase_is_monotonic_and_cancel_prevents_a_new_phase() {
        let runtime = GlobalFlatFoldabilityRuntime::new(30_000);
        advance_phase(&runtime.phase, PHASE_BUILDING_FLAT_EMBEDDING).expect("advance");
        advance_phase(&runtime.phase, PHASE_SEARCHING).expect("skip empty phases");
        assert!(advance_phase(&runtime.phase, PHASE_BUILDING_CONSTRAINTS).is_err());
        assert_eq!(runtime.phase.load(Ordering::SeqCst), PHASE_SEARCHING);

        runtime.cancellation.store(true, Ordering::SeqCst);
        assert!(matches!(
            worker_checkpoint(&runtime, PHASE_VERIFYING_CERTIFICATE),
            Err(WorkerCheckpoint::Cancelled)
        ));
        assert_eq!(runtime.phase.load(Ordering::SeqCst), PHASE_SEARCHING);
    }

    #[test]
    fn preprocessing_checkpoint_and_outcome_preserve_cancel_vs_deadline() {
        let cancelled_runtime = GlobalFlatFoldabilityRuntime::new(30_000);
        assert_eq!(
            preprocessing_checkpoint(&cancelled_runtime),
            CooperativeAnalysisCheckpoint::Continue
        );
        cancelled_runtime.cancellation.store(true, Ordering::SeqCst);
        assert_eq!(
            preprocessing_checkpoint(&cancelled_runtime),
            CooperativeAnalysisCheckpoint::Cancelled
        );
        assert!(matches!(
            preprocessing_abort_outcome(&cancelled_runtime, CooperativeAnalysisAbort::Cancelled),
            WorkerOutcome::Cancelled
        ));

        let now = Instant::now();
        let deadline_runtime = GlobalFlatFoldabilityRuntime {
            cancellation: AtomicBool::new(false),
            worker_started: AtomicBool::new(true),
            phase: AtomicU8::new(PHASE_VALIDATING_LOCAL_CONDITIONS),
            completed_work: AtomicU64::new(0),
            face_count: AtomicU64::new(0),
            overlap_cell_count: AtomicU64::new(0),
            constraint_count: AtomicU64::new(0),
            search_node_count: AtomicU64::new(0),
            started_at: now
                .checked_sub(Duration::from_secs(2))
                .expect("past instant"),
            deadline: now
                .checked_sub(Duration::from_secs(1))
                .expect("past deadline"),
        };
        assert_eq!(
            preprocessing_checkpoint(&deadline_runtime),
            CooperativeAnalysisCheckpoint::DeadlineReached
        );
        assert!(matches!(
            preprocessing_abort_outcome(
                &deadline_runtime,
                CooperativeAnalysisAbort::DeadlineReached
            ),
            WorkerOutcome::Completed {
                result: GlobalFlatFoldabilityResultDto::Unknown {
                    reason: GlobalFlatFoldabilityUnknownReasonDto::TimeLimitReached,
                    ..
                },
                layer_order: None,
            }
        ));
    }

    #[test]
    fn deadline_is_unknown_but_explicit_cancel_remains_cancelled() {
        let now = Instant::now();
        let runtime = GlobalFlatFoldabilityRuntime {
            cancellation: AtomicBool::new(false),
            worker_started: AtomicBool::new(true),
            phase: AtomicU8::new(PHASE_CAPTURING),
            completed_work: AtomicU64::new(0),
            face_count: AtomicU64::new(0),
            overlap_cell_count: AtomicU64::new(0),
            constraint_count: AtomicU64::new(0),
            search_node_count: AtomicU64::new(0),
            started_at: now
                .checked_sub(Duration::from_secs(2))
                .expect("past instant"),
            deadline: now
                .checked_sub(Duration::from_secs(1))
                .expect("past deadline"),
        };
        let timed_out = checkpoint_outcome(&runtime, PHASE_VALIDATING_LOCAL_CONDITIONS)
            .expect("deadline terminal");
        assert!(matches!(
            timed_out,
            WorkerOutcome::Completed {
                result: GlobalFlatFoldabilityResultDto::Unknown {
                    reason: GlobalFlatFoldabilityUnknownReasonDto::TimeLimitReached,
                    ..
                },
                layer_order: None,
            }
        ));

        runtime.cancellation.store(true, Ordering::SeqCst);
        assert!(matches!(
            checkpoint_outcome(&runtime, PHASE_VALIDATING_LOCAL_CONDITIONS),
            Some(WorkerOutcome::Cancelled)
        ));
    }

    #[test]
    fn runtime_progress_is_monotonic_and_does_not_mix_phase_local_totals() {
        let runtime = GlobalFlatFoldabilityRuntime::new(30_000);
        runtime.set_partial_work(4, 20);
        advance_phase(&runtime.phase, PHASE_BUILDING_FLAT_EMBEDDING).expect("advance");

        runtime.observe_progress(GlobalFlatFoldabilityProgress {
            phase: GlobalFlatFoldabilityPhase::Capturing,
            completed_work: 5,
            total_work: Some(5),
            exact_operations: 0,
            overlap_face_pairs: 0,
            overlap_cells: 1,
            constraints: 0,
            search_nodes: 0,
        });
        runtime.observe_progress(GlobalFlatFoldabilityProgress {
            phase: GlobalFlatFoldabilityPhase::BuildingFlatEmbedding,
            completed_work: 30,
            total_work: None,
            exact_operations: 0,
            overlap_face_pairs: 0,
            overlap_cells: 3,
            constraints: 2,
            search_nodes: 1,
        });
        runtime.set_reported_counts(GlobalFlatFoldabilityWorkCounts {
            source_vertex_records: 0,
            source_edge_records: 0,
            paper_boundary_vertex_records: 0,
            face_records: 3,
            face_boundary_half_edges: 0,
            hinge_records: 0,
            edge_incidence_records: 0,
            local_vertex_records: 0,
            total_records: 10,
            overlap_face_pairs: 0,
            arrangement_segments: 5,
            overlap_cells: 2,
            constraints: 2,
            search_nodes: 1,
            exact_operations: 0,
            exact_values: 0,
            certificate_bytes: 0,
        });

        let progress = runtime.progress().expect("valid progress");
        assert_eq!(
            progress.phase,
            GlobalFlatFoldabilityPhaseDto::BuildingFlatEmbedding
        );
        assert_eq!(progress.completed_work, 30);
        assert_eq!(progress.total_work, None);
        assert_eq!(progress.counts.face_count, 4);
        assert_eq!(progress.counts.overlap_cell_count, 3);
        assert_eq!(progress.counts.constraint_count, 2);
        assert_eq!(progress.counts.search_node_count, 1);
    }

    #[test]
    fn native_possible_result_uses_a_geometry_backed_certificate() {
        let project = initial_project_state();
        let source = source_for(&project, GlobalFlatFoldabilityJobId::new());

        let WorkerOutcome::Completed {
            result:
                GlobalFlatFoldabilityResultDto::Possible {
                    layer_order: summary,
                    ..
                },
            layer_order: Some(layer_order),
        } = run_worker(source)
        else {
            panic!("the initial single-face project must be provably possible");
        };

        assert_eq!(summary.layer_count, 1);
        assert_eq!(summary.max_ply, 1);
        assert!(!summary.layer_view_available);
        assert_eq!(layer_order.folded_faces.len(), 1);
        assert!(layer_order.proof_summary.is_some());
    }

    #[test]
    fn native_impossible_result_reports_incident_faces_from_the_real_worker() {
        let (project, _) = four_ray_local_violation_project();
        let expected_face_count = simulation_topology(&project).faces.len();
        let source = source_for(&project, GlobalFlatFoldabilityJobId::new());

        let WorkerOutcome::Completed {
            result: GlobalFlatFoldabilityResultDto::Impossible { proof, summary },
            layer_order: None,
        } = run_worker(source)
        else {
            panic!("the local Kawasaki counterexample must be provably impossible");
        };

        assert_eq!(
            proof.category,
            GlobalFlatFoldabilityProofCategoryDto::LocalConditionsViolated
        );
        assert_eq!(
            proof.face_numbers,
            (1..=u64::try_from(expected_face_count).expect("bounded fixture")).collect::<Vec<_>>()
        );
        assert_eq!(
            summary.counts.face_count,
            u64::try_from(expected_face_count).expect("bounded fixture")
        );
    }

    #[test]
    fn native_worker_deadline_is_an_unknown_terminal_not_an_impossible_result() {
        let project = initial_project_state();
        let mut source = source_for(&project, GlobalFlatFoldabilityJobId::new());
        let now = Instant::now();
        source.runtime = Arc::new(GlobalFlatFoldabilityRuntime {
            cancellation: AtomicBool::new(false),
            worker_started: AtomicBool::new(false),
            phase: AtomicU8::new(PHASE_CAPTURING),
            completed_work: AtomicU64::new(0),
            face_count: AtomicU64::new(0),
            overlap_cell_count: AtomicU64::new(0),
            constraint_count: AtomicU64::new(0),
            search_node_count: AtomicU64::new(0),
            started_at: now.checked_sub(Duration::from_secs(2)).expect("past start"),
            deadline: now
                .checked_sub(Duration::from_secs(1))
                .expect("past deadline"),
        });

        assert!(matches!(
            run_worker(source),
            WorkerOutcome::Completed {
                result: GlobalFlatFoldabilityResultDto::Unknown {
                    reason: GlobalFlatFoldabilityUnknownReasonDto::TimeLimitReached,
                    ..
                },
                layer_order: None,
            }
        ));
    }

    #[test]
    fn report_conversion_accepts_capturing_deadline_without_a_source_fingerprint() {
        let project = initial_project_state();
        let topology = simulation_topology(&project);
        let local =
            analyze_local_flat_foldability(project.editor.paper(), project.editor.pattern());
        let mut observer = DeadlineAtFirstCoreCheckpoint;
        let report = analyze_global_flat_foldability_with_observer(
            GlobalFlatFoldabilityInput::current_with_geometry(
                project.project_id,
                project.editor.paper(),
                project.editor.pattern(),
                &topology,
                &local,
            ),
            GlobalFlatFoldabilityLimits::default(),
            &mut observer,
        )
        .expect("deadline is a mathematical unknown");
        assert_eq!(report.provenance.source_fingerprint, None);
        assert!(matches!(
            &report.outcome,
            GlobalFlatFoldabilityOutcome::Unknown {
                reason: GlobalFlatFoldabilityUnknownReason::TimeLimitReached {
                    phase: GlobalFlatFoldabilityPhase::Capturing,
                },
            }
        ));

        let converted = report_to_dto(
            report,
            GlobalFlatFoldabilityRuntime::new(30_000).summary(),
            &topology,
            project.project_id,
            &project.editor.fold_model_fingerprint_v1(),
        )
        .expect("early deadline must remain a valid unknown result");
        assert!(matches!(
            converted,
            (
                GlobalFlatFoldabilityResultDto::Unknown {
                    reason: GlobalFlatFoldabilityUnknownReasonDto::TimeLimitReached,
                    ..
                },
                None,
            )
        ));
    }

    #[test]
    fn report_conversion_accepts_source_limit_unknown_without_a_source_fingerprint() {
        let project = initial_project_state();
        let topology = simulation_topology(&project);
        let report = core_report_for(
            &project,
            &topology,
            GlobalFlatFoldabilityLimits {
                max_source_vertices: project.editor.pattern().vertices.len() - 1,
                ..GlobalFlatFoldabilityLimits::default()
            },
        );
        assert_eq!(report.provenance.source_fingerprint, None);
        assert!(matches!(
            &report.outcome,
            GlobalFlatFoldabilityOutcome::Unknown {
                reason: GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                    resource: FlatFoldabilityResource::SourceVertices,
                    ..
                },
            }
        ));

        let mut mismatched_fingerprint = report.clone();
        mismatched_fingerprint.provenance.source_fingerprint =
            Some(ori_core::FoldModelFingerprintV1([0xa5; 32]));
        assert!(
            report_to_dto(
                mismatched_fingerprint,
                GlobalFlatFoldabilityRuntime::new(30_000).summary(),
                &topology,
                project.project_id,
                &project.editor.fold_model_fingerprint_v1(),
            )
            .is_err(),
            "an unknown report with a present but mismatched fingerprint must be rejected"
        );

        let converted = report_to_dto(
            report,
            GlobalFlatFoldabilityRuntime::new(30_000).summary(),
            &topology,
            project.project_id,
            &project.editor.fold_model_fingerprint_v1(),
        )
        .expect("source limit must remain a valid unknown result");
        assert!(matches!(
            converted,
            (
                GlobalFlatFoldabilityResultDto::Unknown {
                    reason: GlobalFlatFoldabilityUnknownReasonDto::WorkLimitReached,
                    ..
                },
                None,
            )
        ));
    }

    #[test]
    fn report_conversion_rejects_fingerprintless_proof_verdicts() {
        let possible_project = initial_project_state();
        let possible_topology = simulation_topology(&possible_project);
        let mut possible_report = core_report_for(
            &possible_project,
            &possible_topology,
            GlobalFlatFoldabilityLimits::default(),
        );
        assert!(matches!(
            &possible_report.outcome,
            GlobalFlatFoldabilityOutcome::Possible { .. }
        ));
        possible_report.provenance.source_fingerprint = None;
        assert!(
            report_to_dto(
                possible_report,
                GlobalFlatFoldabilityRuntime::new(30_000).summary(),
                &possible_topology,
                possible_project.project_id,
                &possible_project.editor.fold_model_fingerprint_v1(),
            )
            .is_err()
        );

        let (impossible_project, _) = four_ray_local_violation_project();
        let impossible_topology = simulation_topology(&impossible_project);
        let mut impossible_report = core_report_for(
            &impossible_project,
            &impossible_topology,
            GlobalFlatFoldabilityLimits::default(),
        );
        assert!(matches!(
            &impossible_report.outcome,
            GlobalFlatFoldabilityOutcome::Impossible { .. }
        ));
        impossible_report.provenance.source_fingerprint = None;
        assert!(
            report_to_dto(
                impossible_report,
                GlobalFlatFoldabilityRuntime::new(30_000).summary(),
                &impossible_topology,
                impossible_project.project_id,
                &impossible_project.editor.fold_model_fingerprint_v1(),
            )
            .is_err()
        );
    }

    #[test]
    fn every_impossible_reason_maps_to_bounded_canonical_face_numbers() {
        let (project, center) = four_ray_local_violation_project();
        let topology = simulation_topology(&project);
        let face_count = u64::try_from(topology.faces.len()).expect("bounded fixture");
        let canonical_faces =
            canonical_face_registry(&topology, face_count).expect("canonical fixture registry");
        let work = proof_work_counts(canonical_faces.len(), 6, 9);

        let local = impossible_proof(
            GlobalFlatFoldabilityImpossibleReason::LocalNecessaryConditionViolated {
                violations: vec![LocalNecessaryConditionViolation {
                    vertex: center,
                    kawasaki_violated: true,
                    maekawa_violated: false,
                }],
            },
            &topology,
            &canonical_faces,
            work,
        )
        .expect("local proof faces");
        assert_eq!(local.face_numbers, (1..=face_count).collect::<Vec<_>>());

        let first_face = canonical_faces[0];
        let source_face = topology
            .faces
            .iter()
            .find(|face| face.id == first_face.face_id && face.key == first_face.face_key)
            .expect("first source face");
        let first_half_edge = source_face.outer.half_edges[0];
        let embedding = impossible_proof(
            GlobalFlatFoldabilityImpossibleReason::InconsistentFlatEmbedding {
                face: first_face,
                conflicting_hinge: first_half_edge.edge,
                conflicting_vertex: first_half_edge.origin,
            },
            &topology,
            &canonical_faces,
            work,
        )
        .expect("embedding proof face");
        assert_eq!(embedding.face_numbers, vec![1]);

        let constraint = impossible_proof(
            GlobalFlatFoldabilityImpossibleReason::FacewiseConstraintContradiction {
                constraint_kind: FacewiseConstraintKind::MountainValley,
                faces: vec![canonical_faces[1], canonical_faces[0]],
                supporting_cell: None,
            },
            &topology,
            &canonical_faces,
            work,
        )
        .expect("constraint proof faces");
        assert_eq!(constraint.face_numbers, vec![1, 2]);

        let search = impossible_proof(
            GlobalFlatFoldabilityImpossibleReason::FacewiseSearchExhausted {
                variable_count: work.overlap_face_pairs,
                constraint_count: work.constraints,
            },
            &topology,
            &canonical_faces,
            work,
        )
        .expect("global search proof faces");
        assert_eq!(search.face_numbers, (1..=face_count).collect::<Vec<_>>());
    }

    #[test]
    fn impossible_face_mapping_fails_closed_on_unbound_or_duplicate_evidence() {
        let (project, _) = four_ray_local_violation_project();
        let topology = simulation_topology(&project);
        let face_count = u64::try_from(topology.faces.len()).expect("bounded fixture");
        let canonical_faces =
            canonical_face_registry(&topology, face_count).expect("canonical fixture registry");
        let work = proof_work_counts(canonical_faces.len(), 0, 1);

        let mut unbound_face = canonical_faces[0];
        unbound_face.face_key.0[0] ^= 0xff;
        assert!(
            impossible_proof(
                GlobalFlatFoldabilityImpossibleReason::FacewiseConstraintContradiction {
                    constraint_kind: FacewiseConstraintKind::MountainValley,
                    faces: vec![unbound_face],
                    supporting_cell: None,
                },
                &topology,
                &canonical_faces,
                work,
            )
            .is_err()
        );
        assert!(
            impossible_proof(
                GlobalFlatFoldabilityImpossibleReason::FacewiseConstraintContradiction {
                    constraint_kind: FacewiseConstraintKind::MountainValley,
                    faces: vec![canonical_faces[0], canonical_faces[0]],
                    supporting_cell: None,
                },
                &topology,
                &canonical_faces,
                work,
            )
            .is_err()
        );
        assert!(
            impossible_proof(
                GlobalFlatFoldabilityImpossibleReason::LocalNecessaryConditionViolated {
                    violations: vec![LocalNecessaryConditionViolation {
                        vertex: VertexId::new(),
                        kawasaki_violated: true,
                        maekawa_violated: false,
                    }],
                },
                &topology,
                &canonical_faces,
                work,
            )
            .is_err()
        );
        assert!(
            impossible_proof(
                GlobalFlatFoldabilityImpossibleReason::FacewiseSearchExhausted {
                    variable_count: work.overlap_face_pairs + 1,
                    constraint_count: work.constraints,
                },
                &topology,
                &canonical_faces,
                work,
            )
            .is_err()
        );

        let mut duplicate_registry = topology.clone();
        duplicate_registry.faces.push(topology.faces[0].clone());
        assert!(
            canonical_face_registry(
                &duplicate_registry,
                u64::try_from(duplicate_registry.faces.len()).expect("bounded fixture"),
            )
            .is_err()
        );
    }

    #[test]
    fn native_single_hinge_geometry_summary_reports_two_ply_overlap() {
        let project = centered_single_hinge_project();
        let source = source_for(&project, GlobalFlatFoldabilityJobId::new());

        let WorkerOutcome::Completed {
            result:
                GlobalFlatFoldabilityResultDto::Possible {
                    layer_order: summary,
                    ..
                },
            layer_order: Some(layer_order),
        } = run_worker(source)
        else {
            panic!("the centered single-hinge square must be provably possible");
        };

        assert_eq!(summary.layer_count, 2);
        assert_eq!(summary.max_ply, 2);
        assert!(!summary.layer_view_available);
        assert_eq!(layer_order.folded_faces.len(), 2);
        assert_eq!(
            layer_order
                .proof_summary
                .expect("geometry certificate summary")
                .maximum_ply,
            2
        );
    }

    #[test]
    fn newer_generation_cancels_old_worker_and_old_cancel_is_harmless() {
        let project = initial_project_state();
        let first_id = GlobalFlatFoldabilityJobId::new();
        let second_id = GlobalFlatFoldabilityJobId::new();
        let first = source_for(&project, first_id);
        let second = source_for(&project, second_id);
        let first_context = completion_context(&first);
        let mut slot = GlobalFlatFoldabilitySlot::default();

        install_active_job(&mut slot, &first);
        install_active_job(&mut slot, &second);

        assert!(first.runtime.cancellation.load(Ordering::SeqCst));
        assert_eq!(slot.last_replaced_id, Some(first_id));
        assert!(cancel_job(&mut slot, first_id).is_ok());
        assert!(
            !second.runtime.cancellation.load(Ordering::SeqCst),
            "an old job ID must never cancel the current generation"
        );

        finish_completion(
            &mut slot,
            &project,
            GlobalFlatFoldabilityCompletion {
                context: first_context,
                outcome: WorkerOutcome::Cancelled,
            },
        );
        assert_eq!(
            slot.active.as_ref().map(|active| active.job_id),
            Some(second_id)
        );
        assert!(slot.terminal.is_none());
    }

    #[test]
    fn cancellation_is_idempotent_and_separate_from_unknown() {
        let project = initial_project_state();
        let job_id = GlobalFlatFoldabilityJobId::new();
        let source = source_for(&project, job_id);
        let context = completion_context(&source);
        let mut slot = GlobalFlatFoldabilitySlot::default();
        install_active_job(&mut slot, &source);

        cancel_job(&mut slot, job_id).expect("first cancel");
        cancel_job(&mut slot, job_id).expect("second cancel");
        finish_completion(
            &mut slot,
            &project,
            GlobalFlatFoldabilityCompletion {
                context,
                outcome: WorkerOutcome::Cancelled,
            },
        );
        assert!(matches!(
            slot.terminal.as_ref().map(|terminal| &terminal.dto),
            Some(GlobalFlatFoldabilityJobDto::Cancelled { .. })
        ));
        assert!(cancel_job(&mut slot, job_id).is_ok());
    }

    #[test]
    fn cancelling_completed_possible_job_preserves_layer_order_authority() {
        let project = initial_project_state();
        let state = GlobalFlatFoldabilityState::default();
        install_possible_layer_order(&state, &project);

        let mut slot = lock_foldability_state(&state).expect("lock completed possible job");
        let job_id = slot
            .terminal
            .as_ref()
            .expect("possible result has terminal job")
            .job_id;
        let certificate = Arc::clone(
            slot.current_layer_order
                .as_ref()
                .expect("possible result has layer-order authority"),
        );

        cancel_job(&mut slot, job_id).expect("late cancellation is idempotent");

        assert!(Arc::ptr_eq(
            slot.current_layer_order
                .as_ref()
                .expect("late cancellation preserves authority"),
            &certificate,
        ));
        assert_ne!(slot.last_cancelled_id, Some(job_id));
        assert!(matches!(
            slot.terminal.as_ref().map(|terminal| &terminal.dto),
            Some(GlobalFlatFoldabilityJobDto::Completed {
                result: GlobalFlatFoldabilityResultDto::Possible { .. }
            })
        ));
    }

    #[test]
    fn replaced_completion_cannot_overwrite_current_terminal_or_layer_state() {
        let project = initial_project_state();
        let first = source_for(&project, GlobalFlatFoldabilityJobId::new());
        let second = source_for(&project, GlobalFlatFoldabilityJobId::new());
        let mut slot = GlobalFlatFoldabilitySlot::default();
        install_active_job(&mut slot, &first);
        let stale_callback = unknown_completion(&first);
        install_active_job(&mut slot, &second);
        let current_callback = unknown_completion(&second);

        finish_completion(&mut slot, &project, stale_callback);
        assert_eq!(
            slot.active.as_ref().map(|active| active.job_id),
            Some(second.job_id)
        );
        finish_completion(&mut slot, &project, current_callback);
        assert!(slot.active.is_none());
        assert_eq!(
            slot.terminal.as_ref().map(|terminal| terminal.job_id),
            Some(second.job_id)
        );
        assert!(slot.current_layer_order.is_none());
    }

    #[test]
    fn same_revision_different_full_input_is_stale() {
        let mut project = initial_project_state();
        let job_id = GlobalFlatFoldabilityJobId::new();
        let source = source_for(&project, job_id);
        let mut slot = GlobalFlatFoldabilitySlot::default();
        install_active_job(&mut slot, &source);
        let completion = guarded_run_worker(source);

        let mut changed_pattern = project.editor.pattern().clone();
        changed_pattern.vertices[0].position.x += 0.5;
        project.editor = EditorState::with_paper(changed_pattern, project.editor.paper().clone());
        assert_eq!(
            project.editor.revision(),
            completion.context.binding.revision,
            "fixture preserves the ABA revision"
        );

        finish_completion(&mut slot, &project, completion);
        assert!(matches!(
            slot.terminal.as_ref().map(|terminal| &terminal.dto),
            Some(GlobalFlatFoldabilityJobDto::Stale { .. })
        ));
        assert!(slot.current_layer_order.is_none());
    }

    #[test]
    fn instance_and_fingerprint_are_both_part_of_stale_binding() {
        let mut project = initial_project_state();
        let mut source = source_for(&project, GlobalFlatFoldabilityJobId::new());
        Arc::get_mut(&mut source.binding)
            .expect("fixture has the only binding owner")
            .fold_model_fingerprint = Arc::from("different-internal-fingerprint");
        let mut slot = GlobalFlatFoldabilitySlot::default();
        install_active_job(&mut slot, &source);
        let completion = unknown_completion(&source);
        finish_completion(&mut slot, &project, completion);
        assert!(matches!(
            slot.terminal.as_ref().map(|terminal| &terminal.dto),
            Some(GlobalFlatFoldabilityJobDto::Stale { .. })
        ));

        let source = source_for(&project, GlobalFlatFoldabilityJobId::new());
        install_active_job(&mut slot, &source);
        let completion = unknown_completion(&source);
        project.instance_id = ProjectId::new();
        finish_completion(&mut slot, &project, completion);
        assert!(matches!(
            slot.terminal.as_ref().map(|terminal| &terminal.dto),
            Some(GlobalFlatFoldabilityJobDto::Stale { .. })
        ));
    }

    #[test]
    fn only_current_possible_result_adopts_layer_order() {
        let project = initial_project_state();
        let possible_source = source_for(&project, GlobalFlatFoldabilityJobId::new());
        let mut slot = GlobalFlatFoldabilitySlot::default();
        install_active_job(&mut slot, &possible_source);
        let possible_completion = guarded_run_worker(possible_source);
        assert!(
            matches!(
                &possible_completion.outcome,
                WorkerOutcome::Completed {
                    result: GlobalFlatFoldabilityResultDto::Possible { .. },
                    layer_order: Some(_),
                }
            ),
            "worker was {:?}",
            possible_completion.outcome
        );
        finish_completion(&mut slot, &project, possible_completion);

        assert!(
            matches!(
                slot.terminal.as_ref().map(|terminal| &terminal.dto),
                Some(GlobalFlatFoldabilityJobDto::Completed {
                    result: GlobalFlatFoldabilityResultDto::Possible { .. }
                })
            ),
            "terminal was {:?}",
            slot.terminal.as_ref().map(|terminal| &terminal.dto)
        );
        assert!(slot.current_layer_order.is_some());

        let unknown_source = source_for(&project, GlobalFlatFoldabilityJobId::new());
        install_active_job(&mut slot, &unknown_source);
        assert!(
            slot.current_layer_order.is_none(),
            "starting a replacement invalidates the old layer authority"
        );
        let completion = unknown_completion(&unknown_source);
        finish_completion(&mut slot, &project, completion);
        assert!(matches!(
            slot.terminal.as_ref().map(|terminal| &terminal.dto),
            Some(GlobalFlatFoldabilityJobDto::Completed {
                result: GlobalFlatFoldabilityResultDto::Unknown { .. }
            })
        ));
        assert!(slot.current_layer_order.is_none());
    }

    #[test]
    fn current_layer_order_capability_rejects_same_content_reanalysis_aba() {
        let project = initial_project_state();
        let state = GlobalFlatFoldabilityState::default();
        install_possible_layer_order(&state, &project);
        let first = capture_current_layer_order_capability(&state, &project)
            .expect("capture first authority")
            .expect("first layer authority");
        let first_snapshot = Arc::clone(&first.certificate.snapshot);
        let first_generation = first.claims.generation;

        install_possible_layer_order(&state, &project);
        let second = capture_current_layer_order_capability(&state, &project)
            .expect("capture replacement authority")
            .expect("replacement layer authority");

        assert_eq!(
            *first_snapshot, *second.certificate.snapshot,
            "the ABA fixture deliberately re-analyzes identical contents"
        );
        assert!(
            !Arc::ptr_eq(&first.certificate, &second.certificate),
            "each successful analysis must mint a distinct certificate"
        );
        assert!(
            second.claims.generation > first_generation,
            "the authority generation must advance monotonically"
        );
        assert!(
            revalidate_current_layer_order_capability(&state, &project, &first)
                .expect("revalidate replaced authority")
                .is_none(),
            "an old capture must not regain authority after identical re-analysis"
        );
        assert!(
            revalidate_current_layer_order_capability(&state, &project, &second)
                .expect("revalidate current authority")
                .is_some()
        );
    }

    #[test]
    fn current_layer_order_capability_rejects_edit_undo_and_project_reopen() {
        let mut project = initial_project_state();
        let state = GlobalFlatFoldabilityState::default();
        install_possible_layer_order(&state, &project);
        let captured = capture_current_layer_order_capability(&state, &project)
            .expect("capture authority")
            .expect("layer authority");
        let original_document = project.document();
        let original_fingerprint = project.editor.fold_model_fingerprint_v1();
        let original_revision = project.editor.revision();

        project
            .editor
            .execute(
                original_revision,
                Command::SetCuttingAllowed {
                    allowed: !project.editor.cutting_allowed(),
                },
            )
            .expect("edit project");
        project
            .editor
            .undo(project.editor.revision())
            .expect("undo project edit");

        assert_eq!(project.document(), original_document);
        assert_eq!(
            project.editor.fold_model_fingerprint_v1(),
            original_fingerprint
        );
        assert!(
            project.editor.revision() > original_revision,
            "Undo restores content, not the captured revision identity"
        );
        assert!(
            revalidate_current_layer_order_capability(&state, &project, &captured)
                .expect("revalidate after Undo")
                .is_none()
        );

        let reopened_state = GlobalFlatFoldabilityState::default();
        let original = initial_project_state();
        install_possible_layer_order(&reopened_state, &original);
        let reopened_capture = capture_current_layer_order_capability(&reopened_state, &original)
            .expect("capture before reopen")
            .expect("layer authority before reopen");
        let reopened =
            ProjectState::from_document(original.document(), PathBuf::from("reopened.ori2"));

        assert_eq!(reopened.project_id, original.project_id);
        assert_eq!(reopened.editor.revision(), original.editor.revision());
        assert_eq!(
            reopened.editor.fold_model_fingerprint_v1(),
            original.editor.fold_model_fingerprint_v1()
        );
        assert_ne!(reopened.instance_id, original.instance_id);
        assert!(
            revalidate_current_layer_order_capability(
                &reopened_state,
                &reopened,
                &reopened_capture,
            )
            .expect("revalidate after reopen")
            .is_none()
        );
    }

    #[test]
    fn current_layer_order_capability_rejects_wrong_claims_and_forged_clones() {
        let project = initial_project_state();
        let state = GlobalFlatFoldabilityState::default();
        install_possible_layer_order(&state, &project);
        let captured = capture_current_layer_order_capability(&state, &project)
            .expect("capture authority")
            .expect("layer authority");
        assert!(
            revalidate_current_layer_order_capability(&state, &project, &captured)
                .expect("revalidate genuine authority")
                .is_some()
        );

        let mut forged = copy_layer_order_capability(&captured);
        forged.claims.project_instance_id = ProjectId::new();
        assert!(
            revalidate_current_layer_order_capability(&state, &project, &forged)
                .expect("reject forged project instance")
                .is_none()
        );

        let mut forged = copy_layer_order_capability(&captured);
        forged.claims.project_id = ProjectId::new();
        assert!(
            revalidate_current_layer_order_capability(&state, &project, &forged)
                .expect("reject forged project ID")
                .is_none()
        );

        let mut forged = copy_layer_order_capability(&captured);
        forged.claims.revision = forged.claims.revision.saturating_add(1);
        assert!(
            revalidate_current_layer_order_capability(&state, &project, &forged)
                .expect("reject forged revision")
                .is_none()
        );

        let mut forged = copy_layer_order_capability(&captured);
        forged.claims.proof_model_id = Arc::from("forged-proof-model");
        assert!(
            revalidate_current_layer_order_capability(&state, &project, &forged)
                .expect("reject forged proof model")
                .is_none()
        );

        let mut forged = copy_layer_order_capability(&captured);
        forged.claims.layer_model_id = Arc::from("forged-layer-model");
        assert!(
            revalidate_current_layer_order_capability(&state, &project, &forged)
                .expect("reject forged layer model")
                .is_none()
        );

        let mut forged = copy_layer_order_capability(&captured);
        forged.claims.fold_model_fingerprint = Arc::from("forged-fingerprint");
        assert!(
            revalidate_current_layer_order_capability(&state, &project, &forged)
                .expect("reject forged fingerprint")
                .is_none()
        );

        let mut forged = copy_layer_order_capability(&captured);
        forged.claims.topology_input = Arc::new((*captured.claims.topology_input).clone());
        assert!(
            revalidate_current_layer_order_capability(&state, &project, &forged)
                .expect("reject same-content topology clone")
                .is_none()
        );

        let mut forged = copy_layer_order_capability(&captured);
        forged.claims.snapshot_identity = Arc::new((*captured.claims.snapshot_identity).clone());
        assert!(
            revalidate_current_layer_order_capability(&state, &project, &forged)
                .expect("reject same-content snapshot clone")
                .is_none()
        );

        let mut forged = copy_layer_order_capability(&captured);
        forged.claims.snapshot_provenance.source.source_revision = forged
            .claims
            .snapshot_provenance
            .source
            .source_revision
            .saturating_add(1);
        assert!(
            revalidate_current_layer_order_capability(&state, &project, &forged)
                .expect("reject forged provenance")
                .is_none()
        );

        let mut forged = copy_layer_order_capability(&captured);
        forged.claims.material_registry = Arc::from(
            captured
                .claims
                .material_registry
                .to_vec()
                .into_boxed_slice(),
        );
        assert!(
            revalidate_current_layer_order_capability(&state, &project, &forged)
                .expect("reject same-content material-registry clone")
                .is_none()
        );

        let mut forged = copy_layer_order_capability(&captured);
        forged.claims.generation = forged.claims.generation.saturating_add(1);
        assert!(
            revalidate_current_layer_order_capability(&state, &project, &forged)
                .expect("reject forged generation")
                .is_none()
        );

        let mut forged = copy_layer_order_capability(&captured);
        forged.certificate = Arc::new(deep_clone_layer_order_certificate(&captured.certificate));
        assert!(
            revalidate_current_layer_order_capability(&state, &project, &forged)
                .expect("reject deep-cloned certificate")
                .is_none()
        );

        let wrong_state = GlobalFlatFoldabilityState::default();
        assert!(
            revalidate_current_layer_order_capability(&wrong_state, &project, &captured)
                .expect("reject a different authority slot")
                .is_none()
        );
    }

    #[test]
    fn capture_rejects_corrupted_current_layer_order_certificate() {
        let project = initial_project_state();
        let state = GlobalFlatFoldabilityState::default();
        install_possible_layer_order(&state, &project);
        let captured = capture_current_layer_order_capability(&state, &project)
            .expect("capture authority")
            .expect("layer authority");
        let genuine = Arc::clone(&captured.certificate);

        let mut corrupted = deep_clone_layer_order_certificate(&genuine);
        corrupted.claims.proof_model_id = Arc::from("wrong-proof-model");
        {
            let mut slot = lock_foldability_state(&state).expect("replace certificate");
            slot.current_layer_order = Some(Arc::new(corrupted));
        }
        assert!(
            capture_current_layer_order_capability(&state, &project)
                .expect("capture checks proof model")
                .is_none()
        );

        let mut corrupted = deep_clone_layer_order_certificate(&genuine);
        corrupted.claims.fold_model_fingerprint = Arc::from("wrong-fingerprint");
        {
            let mut slot = lock_foldability_state(&state).expect("replace certificate");
            slot.current_layer_order = Some(Arc::new(corrupted));
        }
        assert!(
            capture_current_layer_order_capability(&state, &project)
                .expect("capture checks fingerprint")
                .is_none()
        );

        let mut corrupted = deep_clone_layer_order_certificate(&genuine);
        corrupted.claims.material_registry = Arc::from(Vec::<LayerFace>::new().into_boxed_slice());
        {
            let mut slot = lock_foldability_state(&state).expect("replace certificate");
            slot.current_layer_order = Some(Arc::new(corrupted));
        }
        assert!(
            capture_current_layer_order_capability(&state, &project)
                .expect("capture checks material registry")
                .is_none()
        );
    }

    #[test]
    fn commit_closure_rejects_old_generation_and_deep_clone_before_action() {
        let app_state = AppState::new(initial_project_state());
        let foldability_state = GlobalFlatFoldabilityState::default();
        let old_capability = {
            let project = lock_project(&app_state).expect("lock project");
            install_possible_layer_order(&foldability_state, &project);
            capture_current_layer_order_capability(&foldability_state, &project)
                .expect("capture old authority")
                .expect("old layer authority")
        };
        {
            let project = lock_project(&app_state).expect("lock project for re-analysis");
            install_possible_layer_order(&foldability_state, &project);
        }

        let action_called = Cell::new(false);
        assert!(
            with_revalidated_current_layer_order_capability(
                &app_state,
                &foldability_state,
                &old_capability,
                |_, _| action_called.set(true),
            )
            .expect("reject old generation")
            .is_none()
        );
        assert!(
            !action_called.get(),
            "an old generation must be rejected before the action runs"
        );

        let current_capability = {
            let project = lock_project(&app_state).expect("lock current project");
            capture_current_layer_order_capability(&foldability_state, &project)
                .expect("capture current authority")
                .expect("current layer authority")
        };
        let mut forged = copy_layer_order_capability(&current_capability);
        forged.certificate = Arc::new(deep_clone_layer_order_certificate(
            &current_capability.certificate,
        ));
        let action_called = Cell::new(false);
        assert!(
            with_revalidated_current_layer_order_capability(
                &app_state,
                &foldability_state,
                &forged,
                |_, _| action_called.set(true),
            )
            .expect("reject cloned certificate")
            .is_none()
        );
        assert!(
            !action_called.get(),
            "a cloned certificate must be rejected before the action runs"
        );
    }

    #[test]
    fn commit_closure_holds_slot_until_action_finishes_then_late_cancel_preserves_authority() {
        let app_state = Arc::new(AppState::new(initial_project_state()));
        let foldability_state = Arc::new(GlobalFlatFoldabilityState::default());
        let capability = {
            let project = lock_project(&app_state).expect("lock project");
            install_possible_layer_order(&foldability_state, &project);
            capture_current_layer_order_capability(&foldability_state, &project)
                .expect("capture authority")
                .expect("layer authority")
        };
        let job_id = {
            let slot = lock_foldability_state(&foldability_state).expect("read terminal job");
            slot.terminal
                .as_ref()
                .expect("possible result has a terminal job")
                .job_id
        };

        let (action_entered_tx, action_entered_rx) = mpsc::channel();
        let (release_action_tx, release_action_rx) = mpsc::channel();
        let action_app_state = Arc::clone(&app_state);
        let action_foldability_state = Arc::clone(&foldability_state);
        let action_thread = thread::spawn(move || {
            with_revalidated_current_layer_order_capability(
                &action_app_state,
                &action_foldability_state,
                &capability,
                |_, snapshot| {
                    action_entered_tx.send(()).expect("announce action entry");
                    release_action_rx.recv().expect("release action");
                    snapshot.material_faces.len()
                },
            )
            .expect("run guarded action")
            .expect("current authority")
        });
        action_entered_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("action enters with slot locked");

        let (cancel_attempted_tx, cancel_attempted_rx) = mpsc::channel();
        let (cancel_finished_tx, cancel_finished_rx) = mpsc::channel();
        let cancel_foldability_state = Arc::clone(&foldability_state);
        let cancel_thread = thread::spawn(move || {
            cancel_attempted_tx
                .send(())
                .expect("announce cancellation attempt");
            let mut slot =
                lock_foldability_state(&cancel_foldability_state).expect("lock for cancellation");
            cancel_job(&mut slot, job_id).expect("cancel completed authority");
            cancel_finished_tx
                .send(())
                .expect("announce cancellation completion");
        });
        cancel_attempted_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("cancellation thread starts");
        assert_eq!(
            cancel_finished_rx.recv_timeout(Duration::from_millis(100)),
            Err(RecvTimeoutError::Timeout),
            "cancellation must wait while the commit action holds the slot"
        );

        release_action_tx.send(()).expect("finish guarded action");
        assert!(
            action_thread.join().expect("join action thread") > 0,
            "fixture has at least one material face"
        );
        cancel_finished_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("cancellation completes after the action");
        cancel_thread.join().expect("join cancellation thread");

        let slot = lock_foldability_state(&foldability_state).expect("inspect preserved authority");
        assert!(slot.current_layer_order.is_some());
        assert_ne!(slot.last_cancelled_id, Some(job_id));
    }

    #[test]
    fn layer_order_generation_exhaustion_fails_closed_without_wrapping() {
        let project = initial_project_state();
        let state = GlobalFlatFoldabilityState::default();
        {
            let mut slot = lock_foldability_state(&state).expect("lock authority slot");
            slot.layer_order_generation = u64::MAX;
        }
        let source = source_for(&project, GlobalFlatFoldabilityJobId::new());
        {
            let mut slot = lock_foldability_state(&state).expect("install active job");
            install_active_job(&mut slot, &source);
        }
        let completion = guarded_run_worker(source);
        {
            let mut slot = lock_foldability_state(&state).expect("finish exhausted generation");
            finish_completion(&mut slot, &project, completion);
            assert_eq!(slot.layer_order_generation, u64::MAX);
            assert!(slot.current_layer_order.is_none());
            assert!(matches!(
                slot.terminal.as_ref().map(|terminal| &terminal.dto),
                Some(GlobalFlatFoldabilityJobDto::Failed {
                    error_category: GlobalFlatFoldabilityErrorCategory::InternalFailure,
                    ..
                })
            ));
        }
    }

    #[test]
    fn possible_without_native_layer_authority_fails_closed() {
        let project = initial_project_state();
        let source = source_for(&project, GlobalFlatFoldabilityJobId::new());
        let context = completion_context(&source);
        let mut slot = GlobalFlatFoldabilitySlot::default();
        install_active_job(&mut slot, &source);
        let result = GlobalFlatFoldabilityResultDto::Possible {
            summary: source.runtime.summary(),
            layer_order: LayerOrderSummaryDto {
                model_id: LayerOrderModelDto::FacewiseLayerOrderV1,
                layer_count: 1,
                max_ply: 1,
                reference_face_number: 1,
                layer_view_available: true,
            },
        };
        finish_completion(
            &mut slot,
            &project,
            GlobalFlatFoldabilityCompletion {
                context,
                outcome: WorkerOutcome::Completed {
                    result,
                    layer_order: None,
                },
            },
        );
        assert!(matches!(
            slot.terminal.as_ref().map(|terminal| &terminal.dto),
            Some(GlobalFlatFoldabilityJobDto::Failed {
                error_category: GlobalFlatFoldabilityErrorCategory::InternalFailure,
                ..
            })
        ));
        assert!(slot.current_layer_order.is_none());
    }

    #[test]
    fn terminal_result_is_claimed_once() {
        let job_id = GlobalFlatFoldabilityJobId::new();
        let summary = GlobalFlatFoldabilityRuntime::new(30_000).summary();
        let mut slot = GlobalFlatFoldabilitySlot {
            terminal: Some(TerminalGlobalFlatFoldabilityJob {
                job_id,
                dto: GlobalFlatFoldabilityJobDto::Completed {
                    result: unknown_result(
                        summary,
                        GlobalFlatFoldabilityUnknownReasonDto::ProofNotCompleted,
                    ),
                },
                claimed: false,
            }),
            ..GlobalFlatFoldabilitySlot::default()
        };
        assert!(poll_global_flat_foldability_job(&mut slot, job_id).is_ok());
        assert_eq!(
            poll_global_flat_foldability_job(&mut slot, job_id)
                .expect_err("one-shot result")
                .category,
            GlobalFlatFoldabilityErrorCategory::ResultUnavailable
        );
    }

    #[test]
    fn completion_refreshes_the_result_summary_after_waiting_for_the_slot() {
        let project = initial_project_state();
        let source = source_for(&project, GlobalFlatFoldabilityJobId::new());
        let context = completion_context(&source);
        let stale_summary = source.runtime.summary();
        source.runtime.set_partial_work(3, 7);
        let mut slot = GlobalFlatFoldabilitySlot::default();
        install_active_job(&mut slot, &source);

        finish_completion(
            &mut slot,
            &project,
            GlobalFlatFoldabilityCompletion {
                context,
                outcome: WorkerOutcome::Completed {
                    result: unknown_result(
                        stale_summary,
                        GlobalFlatFoldabilityUnknownReasonDto::ProofNotCompleted,
                    ),
                    layer_order: None,
                },
            },
        );

        assert!(matches!(
            slot.terminal.as_ref().map(|terminal| &terminal.dto),
            Some(GlobalFlatFoldabilityJobDto::Completed {
                result: GlobalFlatFoldabilityResultDto::Unknown {
                    summary: GlobalFlatFoldabilitySummaryDto {
                        counts: GlobalFlatFoldabilityCountsDto { face_count: 3, .. },
                        ..
                    },
                    ..
                }
            })
        ));
    }

    #[test]
    fn worker_panic_and_command_errors_never_serialize_raw_details() {
        let raw_secret = r"C:\private\model.ori2 123e4567-e89b-12d3-a456-426614174000 (12.5,-8.0)";
        let outcome = catch_worker_failure(|| panic!("{raw_secret}"));
        assert!(matches!(
            outcome,
            WorkerOutcome::Failed(GlobalFlatFoldabilityErrorCategory::InternalFailure)
        ));

        let summary = GlobalFlatFoldabilityRuntime::new(30_000).summary();
        let failed = GlobalFlatFoldabilityJobDto::Failed {
            summary,
            error_category: GlobalFlatFoldabilityErrorCategory::InternalFailure,
        };
        let command_error = GlobalFlatFoldabilityCommandError::new(
            GlobalFlatFoldabilityErrorCategory::InternalFailure,
        );
        let encoded =
            serde_json::to_string(&(failed, command_error)).expect("serialize closed DTO");
        for fragment in [
            "private",
            "model.ori2",
            "123e4567",
            "12.5",
            "-8.0",
            "internal error",
        ] {
            assert!(!encoded.contains(fragment), "leaked fragment {fragment:?}");
        }
        assert_eq!(
            serde_json::to_value(command_error).expect("error JSON"),
            json!({
                "category": "internal_failure"
            })
        );
    }

    #[test]
    fn worker_gate_is_nonblocking_and_holds_capacity_until_worker_exit() {
        let gate = GlobalFlatFoldabilityWorkerGate::new();
        let permit = gate.try_acquire().expect("first worker acquires permit");
        assert!(gate.is_busy());
        assert!(
            gate.try_acquire().is_none(),
            "a replacement worker must fail closed instead of waiting unboundedly"
        );

        drop(permit);
        assert!(!gate.is_busy());
        assert!(
            gate.try_acquire().is_some(),
            "capacity returns only after the old worker permit is dropped"
        );
    }

    #[test]
    fn worker_gate_releases_capacity_after_caught_panic() {
        let gate = GlobalFlatFoldabilityWorkerGate::new();
        {
            let _permit = gate.try_acquire().expect("worker acquires permit");
            let outcome = catch_worker_failure(|| panic!("bounded worker panic"));
            assert!(matches!(
                outcome,
                WorkerOutcome::Failed(GlobalFlatFoldabilityErrorCategory::InternalFailure)
            ));
            assert!(
                gate.is_busy(),
                "panic handling must not release a live permit early"
            );
        }
        assert!(!gate.is_busy());
        assert!(gate.try_acquire().is_some());
    }

    #[test]
    fn active_and_terminal_json_match_the_frontend_closed_contract() {
        let runtime = GlobalFlatFoldabilityRuntime::new(30_000);
        let queued = active_job_dto(&runtime).expect("queued DTO");
        let queued_json = serde_json::to_value(queued).expect("queued JSON");
        assert_eq!(
            queued_json,
            json!({
                "state": "queued",
                "cancel_requested": false,
                "progress": {
                    "model_id": "convex_faces_facewise_v1",
                    "phase": "capturing",
                    "completed_work": 0,
                    "total_work": null,
                    "elapsed_ms": queued_json["progress"]["elapsed_ms"],
                    "counts": {
                        "face_count": 0,
                        "overlap_cell_count": 0,
                        "constraint_count": 0,
                        "search_node_count": 0
                    }
                }
            })
        );

        let terminal = GlobalFlatFoldabilityJobDto::Completed {
            result: unknown_result(
                runtime.summary(),
                GlobalFlatFoldabilityUnknownReasonDto::TimeLimitReached,
            ),
        };
        let terminal_json = serde_json::to_value(terminal).expect("terminal JSON");
        assert_eq!(terminal_json["state"], "completed");
        assert_eq!(terminal_json["result"]["verdict"], "unknown");
        assert_eq!(terminal_json["result"]["reason"], "time_limit_reached");
        assert!(terminal_json.get("job_id").is_none());
    }
}
