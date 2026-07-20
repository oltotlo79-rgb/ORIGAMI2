use std::{
    collections::VecDeque,
    error::Error,
    fmt,
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{Arc, Condvar, Mutex, MutexGuard},
    time::{Duration, Instant},
};

use ori_domain::ProjectId;
use ori_formats::{Ori2ProjectArchive, ProjectDocument};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Manager, State};

use super::{
    AppState, ProjectSnapshot, ProjectState, commit_project_replacement, ensure_expected_project,
    project_persistence::{
        FRONTEND_MAX_SAFE_INTEGER_U64, RecoveryPersistenceError, RecoveryProjectLoad,
        clear_recovery_document, inspect_recovery_project, persist_recovery_project,
    },
    snapshot, validate_loaded_numeric_expression_bindings,
};

pub(super) const RECOVERY_SCHEMA_VERSION: u32 = 1;
const RECOVERY_SLOT_FILE_NAME: &str = "current-project.ori2";
const MAX_RECORDED_SETTLEMENTS: usize = 128;
pub(super) const RECOVERY_AUTOSAVE_INTERVAL: Duration = Duration::from_secs(30);
pub(super) const RECOVERY_SETTLEMENT_TIMEOUT: Duration = Duration::from_secs(5);
const WINDOW_CLOSE_PREPARE_LIFETIME: Duration = Duration::from_secs(10);
const RECOVERY_STORAGE_FAILED_MESSAGE: &str =
    "The private recovery data could not be updated safely.";
const RECOVERY_STATE_FAILED_MESSAGE: &str = "The private recovery writer state is unavailable.";
const RECOVERY_GENERATION_EXHAUSTED_MESSAGE: &str =
    "The private recovery writer generation is exhausted.";
const RECOVERY_SETTLEMENT_TIMED_OUT_MESSAGE: &str =
    "The private recovery writer did not settle before the deadline.";
const RECOVERY_COMMAND_FAILED_MESSAGE: &str = "The recovery operation could not be completed.";
const RECOVERY_STALE_MESSAGE: &str = "The recovery candidate or project state changed.";
const WINDOW_CLOSE_PREPARE_FAILED_MESSAGE: &str = "The window close could not be prepared safely.";

/// A JSON number that is always exact in JavaScript/TypeScript.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub(super) struct FrontendSafeUnixMillis(u64);

impl FrontendSafeUnixMillis {
    #[must_use]
    pub(super) const fn new(value: u64) -> Option<Self> {
        if value <= FRONTEND_MAX_SAFE_INTEGER_U64 {
            Some(Self(value))
        } else {
            None
        }
    }
}

/// Opaque process-side token for one inspected recovery slot.
///
/// It deliberately wraps, rather than aliases, [`ProjectId`] so a recovery
/// token cannot be confused with persisted document identity. Wire input is
/// restricted to the same canonical lowercase, non-nil UUID form admitted by
/// the strict frontend parser.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct RecoveryId(ProjectId);

impl RecoveryId {
    #[must_use]
    pub(super) fn new() -> Self {
        Self(ProjectId::new())
    }
}

impl Serialize for RecoveryId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for RecoveryId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let encoded = String::deserialize(deserializer)?;
        if !is_canonical_non_nil_uuid(&encoded) {
            return Err(D::Error::custom(
                "recovery_id must be a canonical lowercase non-nil UUID",
            ));
        }
        let project_id = serde_json::from_value(serde_json::Value::String(encoded))
            .map_err(|_| D::Error::custom("recovery_id is invalid"))?;
        Ok(Self(project_id))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct ClosePrepareId(ProjectId);

impl ClosePrepareId {
    #[must_use]
    fn new() -> Self {
        Self(ProjectId::new())
    }
}

impl Serialize for ClosePrepareId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ClosePrepareId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let encoded = String::deserialize(deserializer)?;
        if !is_canonical_non_nil_uuid(&encoded) {
            return Err(D::Error::custom(
                "close_prepare_id must be a canonical lowercase non-nil UUID",
            ));
        }
        let project_id = serde_json::from_value(serde_json::Value::String(encoded))
            .map_err(|_| D::Error::custom("close_prepare_id is invalid"))?;
        Ok(Self(project_id))
    }
}

fn is_canonical_non_nil_uuid(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 36
        && [8, 13, 18, 23]
            .into_iter()
            .all(|index| bytes[index] == b'-')
        && bytes.iter().enumerate().all(|(index, byte)| {
            [8, 13, 18, 23].contains(&index)
                || byte.is_ascii_digit()
                || (b'a'..=b'f').contains(byte)
        })
        && bytes.iter().any(|byte| !matches!(byte, b'0' | b'-'))
}

fn deserialize_canonical_project_id<'de, D>(deserializer: D) -> Result<ProjectId, D::Error>
where
    D: Deserializer<'de>,
{
    let encoded = String::deserialize(deserializer)?;
    if !is_canonical_non_nil_uuid(&encoded) {
        return Err(D::Error::custom(
            "project ID must be a canonical lowercase non-nil UUID",
        ));
    }
    serde_json::from_value(serde_json::Value::String(encoded))
        .map_err(|_| D::Error::custom("project ID is invalid"))
}

/// Strict response contract for recovery discovery over the Tauri boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub(super) enum GetRecoveryCandidateResponse {
    None {
        schema_version: u32,
    },
    Invalid {
        schema_version: u32,
        recovery_id: RecoveryId,
    },
    Available {
        schema_version: u32,
        recovery_id: RecoveryId,
        project_id: ProjectId,
        updated_at_unix_ms: Option<FrontendSafeUnixMillis>,
    },
}

/// Redacted process-local health of the automatic recovery writer.
///
/// This boundary deliberately exposes neither storage paths nor internal
/// error categories. `transition_id` changes only when the status changes, so
/// a renderer can suppress repeated announcements without learning how many
/// recovery operations have occurred.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum RecoveryAutosaveHealthStatus {
    PendingFirstAttempt,
    Operational,
    PersistenceFailed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(super) struct GetRecoveryAutosaveStatusResponse {
    pub(super) schema_version: u32,
    pub(super) status: RecoveryAutosaveHealthStatus,
    pub(super) transition_id: u32,
}

/// `restore_recovery` accepts one top-level Tauri argument named `request`
/// whose value is exactly this object.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RestoreRecoveryRequest {
    pub(super) schema_version: u32,
    pub(super) recovery_id: RecoveryId,
    #[serde(deserialize_with = "deserialize_canonical_project_id")]
    pub(super) expected_project_id: ProjectId,
    #[serde(deserialize_with = "deserialize_canonical_project_id")]
    pub(super) expected_instance_id: ProjectId,
    pub(super) expected_revision: u64,
}

/// `discard_recovery` accepts one top-level Tauri argument named `request`
/// whose value is exactly this object.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DiscardRecoveryRequest {
    pub(super) schema_version: u32,
    pub(super) recovery_id: RecoveryId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum DiscardRecoveryStatus {
    Discarded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(super) struct DiscardRecoveryResponse {
    pub(super) schema_version: u32,
    pub(super) status: DiscardRecoveryStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum ExitRecoveryAuthorization {
    Clean,
    DiscardConfirmed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PrepareWindowCloseRequest {
    pub(super) schema_version: u32,
    #[serde(deserialize_with = "deserialize_canonical_project_id")]
    pub(super) project_instance_id: ProjectId,
    #[serde(deserialize_with = "deserialize_canonical_project_id")]
    pub(super) project_id: ProjectId,
    pub(super) revision: u64,
    pub(super) authorization: ExitRecoveryAuthorization,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum PrepareWindowCloseStatus {
    Prepared,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(super) struct PrepareWindowCloseResponse {
    pub(super) schema_version: u32,
    pub(super) status: PrepareWindowCloseStatus,
    pub(super) close_prepare_id: ClosePrepareId,
    pub(super) project_instance_id: ProjectId,
    pub(super) project_id: ProjectId,
    pub(super) revision: u64,
    pub(super) authorization: ExitRecoveryAuthorization,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CancelWindowClosePrepareRequest {
    pub(super) schema_version: u32,
    pub(super) close_prepare_id: ClosePrepareId,
    #[serde(deserialize_with = "deserialize_canonical_project_id")]
    pub(super) project_instance_id: ProjectId,
    #[serde(deserialize_with = "deserialize_canonical_project_id")]
    pub(super) project_id: ProjectId,
    pub(super) revision: u64,
    pub(super) authorization: ExitRecoveryAuthorization,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum CancelWindowClosePrepareStatus {
    Canceled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(super) struct CancelWindowClosePrepareResponse {
    pub(super) schema_version: u32,
    pub(super) status: CancelWindowClosePrepareStatus,
    pub(super) close_prepare_id: ClosePrepareId,
    pub(super) project_instance_id: ProjectId,
    pub(super) project_id: ProjectId,
    pub(super) revision: u64,
    pub(super) authorization: ExitRecoveryAuthorization,
}

/// Marker for the direct response of the restore command.
///
/// This alias prevents an accidental response wrapper from being introduced.
pub(super) type RestoreRecoveryResponse = ProjectSnapshot;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct RecoveryGeneration(u64);

impl RecoveryGeneration {
    #[must_use]
    pub(super) const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RecoverySubmissionDisposition {
    /// This caller transitioned the single writer from idle to scheduled.
    Drained,
    /// A writer was already active. This operation replaced any older pending
    /// operation and will be drained by that writer.
    Coalesced,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RecoverySubmission {
    pub(super) generation: RecoveryGeneration,
    pub(super) disposition: RecoverySubmissionDisposition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RecoveryWriterStatus {
    pub(super) latest_generation: Option<RecoveryGeneration>,
    /// Latest operation known durable. A successful clear also advances this
    /// fence even though no recovery file remains.
    pub(super) durable_generation: Option<RecoveryGeneration>,
    pub(super) failed_generation: Option<RecoveryGeneration>,
    pub(super) writer_active: bool,
    pub(super) has_pending_operation: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RecoverySettlement {
    Durable,
    Failed,
    Superseded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RecoveryStorageError {
    StorageUnavailable,
    StateUnavailable,
    GenerationExhausted,
    SettlementTimedOut,
}

impl fmt::Display for RecoveryStorageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::StorageUnavailable => RECOVERY_STORAGE_FAILED_MESSAGE,
            Self::StateUnavailable => RECOVERY_STATE_FAILED_MESSAGE,
            Self::GenerationExhausted => RECOVERY_GENERATION_EXHAUSTED_MESSAGE,
            Self::SettlementTimedOut => RECOVERY_SETTLEMENT_TIMED_OUT_MESSAGE,
        })
    }
}

impl Error for RecoveryStorageError {}

/// A validated startup candidate. Its source path is intentionally absent.
pub(super) struct RecoveryStartupCandidate {
    project: Box<Ori2ProjectArchive>,
    updated_at_unix_ms: Option<u64>,
}

impl RecoveryStartupCandidate {
    #[must_use]
    pub(super) const fn document(&self) -> &ProjectDocument {
        &self.project.document
    }

    #[must_use]
    pub(super) const fn updated_at_unix_ms(&self) -> Option<u64> {
        self.updated_at_unix_ms
    }

    /// Restores into a fresh unsaved instance. No original path, saved
    /// baseline, or runtime pose exists in the recovery payload.
    pub(super) fn into_project_state(self) -> Result<ProjectState, ()> {
        ProjectState::from_recovery_project_archive(*self.project)
    }
}

pub(super) enum RecoveryStartup {
    None,
    Invalid,
    Available(RecoveryStartupCandidate),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RecoveryProjectBinding {
    project_instance_id: ProjectId,
    project_id: ProjectId,
    revision: u64,
}

impl RecoveryProjectBinding {
    fn from_project(project: &ProjectState) -> Self {
        Self {
            project_instance_id: project.instance_id,
            project_id: project.project_id,
            revision: project.editor.revision(),
        }
    }

    fn from_snapshot(project: &ProjectSnapshot) -> Self {
        Self {
            project_instance_id: project.project_instance_id,
            project_id: project.project_id,
            revision: project.revision,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StoredRecoveryIdentity {
    binding: RecoveryProjectBinding,
    history_digest: [u8; 32],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DurableRecoveryAction {
    Stored(StoredRecoveryIdentity),
    Cleared(RecoveryProjectBinding),
}

struct Sha256Writer(Sha256);

impl Sha256Writer {
    fn new() -> Self {
        Self(Sha256::new())
    }

    fn finish(self) -> [u8; 32] {
        self.0.finalize().into()
    }
}

impl Write for Sha256Writer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn editor_history_digest(project: &Ori2ProjectArchive) -> Result<[u8; 32], RecoveryStorageError> {
    let mut writer = Sha256Writer::new();
    serde_json::to_writer(&mut writer, &project.editor_history)
        .map_err(|_| RecoveryStorageError::StorageUnavailable)?;
    Ok(writer.finish())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PreparedWindowClose {
    close_prepare_id: ClosePrepareId,
    binding: RecoveryProjectBinding,
    authorization: ExitRecoveryAuthorization,
    expires_at: Instant,
}

enum CachedRecoveryCandidate {
    None,
    Invalid {
        recovery_id: RecoveryId,
    },
    Available {
        recovery_id: RecoveryId,
        project: Box<Ori2ProjectArchive>,
        updated_at_unix_ms: Option<u64>,
        restored: bool,
    },
}

impl CachedRecoveryCandidate {
    fn from_startup(startup: RecoveryStartup) -> Self {
        match startup {
            RecoveryStartup::None => Self::None,
            RecoveryStartup::Invalid => Self::Invalid {
                recovery_id: RecoveryId::new(),
            },
            RecoveryStartup::Available(candidate)
                if validate_loaded_numeric_expression_bindings(&candidate.project.document)
                    .is_ok()
                    && crate::restore_archive_editor(&candidate.project).is_ok() =>
            {
                Self::Available {
                    recovery_id: RecoveryId::new(),
                    project: candidate.project,
                    updated_at_unix_ms: candidate.updated_at_unix_ms,
                    restored: false,
                }
            }
            RecoveryStartup::Available(_) => Self::Invalid {
                recovery_id: RecoveryId::new(),
            },
        }
    }

    fn blocks_automatic_writes(&self) -> bool {
        matches!(
            self,
            Self::Invalid { .. }
                | Self::Available {
                    restored: false,
                    ..
                }
        )
    }

    fn recovery_id(&self) -> Option<RecoveryId> {
        match self {
            Self::None => None,
            Self::Invalid { recovery_id } | Self::Available { recovery_id, .. } => {
                Some(*recovery_id)
            }
        }
    }
}

struct RecoveryRuntimeState {
    epoch: u64,
    next_operation_id: u64,
    candidate: CachedRecoveryCandidate,
    last_durable_action: Option<DurableRecoveryAction>,
    in_flight_operation: Option<u64>,
    automatic_writes_stopped: bool,
    prepared_window_close: Option<PreparedWindowClose>,
    autosave_health_status: RecoveryAutosaveHealthStatus,
    autosave_health_transition_id: u32,
}

impl RecoveryRuntimeState {
    fn new(candidate: CachedRecoveryCandidate) -> Self {
        Self {
            epoch: 0,
            next_operation_id: 0,
            candidate,
            last_durable_action: None,
            in_flight_operation: None,
            automatic_writes_stopped: false,
            prepared_window_close: None,
            autosave_health_status: RecoveryAutosaveHealthStatus::PendingFirstAttempt,
            autosave_health_transition_id: 0,
        }
    }

    fn next_epoch(&self) -> Result<u64, RecoveryStorageError> {
        self.epoch
            .checked_add(1)
            .ok_or(RecoveryStorageError::GenerationExhausted)
    }

    fn begin_operation(&mut self) -> Result<u64, RecoveryStorageError> {
        let operation_id = self
            .next_operation_id
            .checked_add(1)
            .ok_or(RecoveryStorageError::GenerationExhausted)?;
        self.next_operation_id = operation_id;
        self.in_flight_operation = Some(operation_id);
        Ok(operation_id)
    }

    fn record_autosave_health(&mut self, requested: RecoveryAutosaveHealthStatus) {
        if self.autosave_health_status == requested
            || self.autosave_health_transition_id == u32::MAX
        {
            return;
        }

        let next = self
            .autosave_health_transition_id
            .checked_add(1)
            .expect("the exhausted transition ID is handled above");
        self.autosave_health_transition_id = next;
        if next == u32::MAX {
            // Reserve the terminal transition as a visible fail-closed latch.
            // It can never wrap to make an older renderer response look new.
            self.autosave_health_status = RecoveryAutosaveHealthStatus::PersistenceFailed;
            return;
        }
        self.autosave_health_status = requested;
    }
}

struct RecoveryRuntimeInner {
    storage: RecoveryStorage,
    /// Serializes runtime-owned slot I/O.
    ///
    /// The autosave timer acquires this gate while it still owns the project
    /// lock, captures the document, then releases the project before doing
    /// I/O. Normal completion and exit clears acquire only this gate. This
    /// makes every captured autosave fall wholly before or wholly after a
    /// clear, so an old capture cannot resurrect a cleared recovery slot.
    operation_gate: Mutex<()>,
    state: Mutex<RecoveryRuntimeState>,
}

/// Process-lifetime owner of the startup candidate and autosave lifecycle.
///
/// Any operation that also mutates a project must acquire the project lock
/// first and this state lock second. Runtime methods that perform filesystem
/// I/O never acquire the project lock.
#[derive(Clone)]
pub(super) struct RecoveryRuntime(Arc<RecoveryRuntimeInner>);

struct PreparedRecoveryRestore {
    recovery_id: RecoveryId,
    epoch: u64,
    project: Box<Ori2ProjectArchive>,
    replacement: ProjectState,
}

struct RecoveryAutosaveCapture {
    binding: RecoveryProjectBinding,
    project: Option<Ori2ProjectArchive>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RecoveryAutosaveOutcome {
    Stored,
    Cleared,
    Duplicate,
    StartupDecisionPending,
    AutomaticWritesStopped,
    Superseded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum NormalRecoveryClearOutcome {
    Cleared,
    SkippedProjectChanged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ExitRecoveryDisposition {
    Cleared,
    PreservedStartupCandidate,
    ProjectChanged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PreparedWindowCloseSettlement {
    NotPrepared,
    Settled,
    Rejected,
}

trait RecoveryIo: Send + Sync {
    fn inspect(&self, slot_path: &Path) -> RecoveryStartup;
    fn store(
        &self,
        slot_path: &Path,
        project: &Ori2ProjectArchive,
    ) -> Result<(), RecoveryPersistenceError>;
    fn clear(&self, slot_path: &Path) -> Result<(), RecoveryPersistenceError>;
}

struct FileRecoveryIo;

impl RecoveryIo for FileRecoveryIo {
    fn inspect(&self, slot_path: &Path) -> RecoveryStartup {
        match inspect_recovery_project(slot_path) {
            RecoveryProjectLoad::Missing => RecoveryStartup::None,
            RecoveryProjectLoad::Invalid => RecoveryStartup::Invalid,
            RecoveryProjectLoad::Available {
                project,
                updated_at_unix_ms,
            } => RecoveryStartup::Available(RecoveryStartupCandidate {
                project,
                updated_at_unix_ms,
            }),
        }
    }

    fn store(
        &self,
        slot_path: &Path,
        project: &Ori2ProjectArchive,
    ) -> Result<(), RecoveryPersistenceError> {
        persist_recovery_project(slot_path, project)
    }

    fn clear(&self, slot_path: &Path) -> Result<(), RecoveryPersistenceError> {
        clear_recovery_document(slot_path)
    }
}

enum RecoveryAction {
    Store(Box<Ori2ProjectArchive>),
    Clear,
}

struct RecoveryWork {
    generation: RecoveryGeneration,
    action: RecoveryAction,
}

#[derive(Default)]
struct RecoveryWriterState {
    last_generation: u64,
    latest_generation: Option<RecoveryGeneration>,
    durable_generation: Option<RecoveryGeneration>,
    failed_generation: Option<RecoveryGeneration>,
    writer_active: bool,
    pending: Option<RecoveryWork>,
    failed_work: Option<RecoveryWork>,
    settlements: VecDeque<(RecoveryGeneration, RecoverySettlement)>,
}

impl RecoveryWriterState {
    fn settlement(&self, generation: RecoveryGeneration) -> Option<RecoverySettlement> {
        self.settlements
            .iter()
            .rev()
            .find_map(|(recorded, settlement)| (*recorded == generation).then_some(*settlement))
    }

    fn record_settlement(
        &mut self,
        generation: RecoveryGeneration,
        settlement: RecoverySettlement,
    ) {
        if let Some((_, recorded)) = self
            .settlements
            .iter_mut()
            .rev()
            .find(|(recorded, _)| *recorded == generation)
        {
            *recorded = settlement;
            return;
        }
        if self.settlements.len() == MAX_RECORDED_SETTLEMENTS {
            self.settlements.pop_front();
        }
        self.settlements.push_back((generation, settlement));
    }

    fn remove_settlement(&mut self, generation: RecoveryGeneration) {
        if let Some(index) = self
            .settlements
            .iter()
            .position(|(recorded, _)| *recorded == generation)
        {
            self.settlements.remove(index);
        }
    }
}

struct RecoveryStorageInner {
    slot_path: PathBuf,
    io: Arc<dyn RecoveryIo>,
    writer: Mutex<RecoveryWriterState>,
    settled: Condvar,
}

/// Generation-fenced, coalescing single-writer core for one private slot.
///
/// The fixed-slot design assumes the production application registers
/// `tauri-plugin-single-instance` before every other plugin. Its callback must
/// ignore arguments and the working directory and may only reveal/focus the
/// main window. Production startup establishes that process boundary before
/// constructing this storage from the app-private data directory.
///
/// `store` accepts an owned detached [`Ori2ProjectArchive`]. A caller can clone
/// the project archive while holding the project mutex, release that mutex, and then
/// call this API. Filesystem I/O never requires access to live `ProjectState`.
#[derive(Clone)]
pub(super) struct RecoveryStorage(Arc<RecoveryStorageInner>);

impl fmt::Debug for RecoveryStorage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RecoveryStorage")
            .finish_non_exhaustive()
    }
}

impl RecoveryStorage {
    #[must_use]
    pub(super) fn new(root: impl Into<PathBuf>) -> Self {
        Self::with_io_inner(root.into(), Arc::new(FileRecoveryIo))
    }

    fn with_io_inner(root: PathBuf, io: Arc<dyn RecoveryIo>) -> Self {
        Self(Arc::new(RecoveryStorageInner {
            slot_path: root.join(RECOVERY_SLOT_FILE_NAME),
            io,
            writer: Mutex::new(RecoveryWriterState::default()),
            settled: Condvar::new(),
        }))
    }

    /// Performs a bounded startup inspection without exposing the fixed path.
    ///
    /// This is a startup-only operation and is intentionally independent from
    /// the writer mutex. Atomic publication means it observes either the old
    /// verified file or the new verified file, never a staging file.
    #[must_use]
    pub(super) fn inspect_startup(&self) -> RecoveryStartup {
        self.0.io.inspect(&self.0.slot_path)
    }

    pub(super) fn store_project(
        &self,
        project: Ori2ProjectArchive,
    ) -> Result<RecoverySubmission, RecoveryStorageError> {
        self.enqueue(RecoveryAction::Store(Box::new(project)))
    }

    #[cfg(test)]
    pub(super) fn store(
        &self,
        document: ProjectDocument,
    ) -> Result<RecoverySubmission, RecoveryStorageError> {
        self.store_project(Ori2ProjectArchive::document_only(document))
    }

    pub(super) fn clear(&self) -> Result<RecoverySubmission, RecoveryStorageError> {
        self.enqueue(RecoveryAction::Clear)
    }

    /// Waits a bounded amount of time for one exact generation to finish.
    ///
    /// Explicit discard and application quit must use this fence after
    /// `clear`; a coalesced clear is only queued and is not yet durable.
    pub(super) fn wait_for_settlement(
        &self,
        generation: RecoveryGeneration,
        timeout: Duration,
    ) -> Result<RecoverySettlement, RecoveryStorageError> {
        let started = Instant::now();
        let mut state = self.lock_writer()?;
        loop {
            if let Some(settlement) = state.settlement(generation) {
                return Ok(settlement);
            }
            let elapsed = started.elapsed();
            if elapsed >= timeout {
                return Err(RecoveryStorageError::SettlementTimedOut);
            }
            let remaining = timeout.saturating_sub(elapsed);
            let (next_state, timed_out) = self
                .0
                .settled
                .wait_timeout(state, remaining)
                .map_err(|_| RecoveryStorageError::StateUnavailable)?;
            state = next_state;
            if timed_out.timed_out() && state.settlement(generation).is_none() {
                return Err(RecoveryStorageError::SettlementTimedOut);
            }
        }
    }

    pub(super) fn clear_and_wait(
        &self,
        timeout: Duration,
    ) -> Result<RecoverySettlement, RecoveryStorageError> {
        let clear = self.clear()?;
        self.wait_for_settlement(clear.generation, timeout)
    }

    /// Retries the latest failed operation without issuing a new generation.
    ///
    /// A newer store or clear always replaces this retry payload.
    pub(super) fn retry_failed(&self) -> Result<bool, RecoveryStorageError> {
        {
            let mut state = self.lock_writer()?;
            if state.writer_active {
                return Ok(false);
            }
            let Some(work) = state.failed_work.take() else {
                return Ok(false);
            };
            state.writer_active = true;
            state.failed_generation = None;
            state.remove_settlement(work.generation);
            state.pending = Some(work);
        }
        self.spawn_writer()?;
        Ok(true)
    }

    pub(super) fn status(&self) -> Result<RecoveryWriterStatus, RecoveryStorageError> {
        let state = self.lock_writer()?;
        Ok(RecoveryWriterStatus {
            latest_generation: state.latest_generation,
            durable_generation: state.durable_generation,
            failed_generation: state.failed_generation,
            writer_active: state.writer_active,
            has_pending_operation: state.pending.is_some(),
        })
    }

    fn enqueue(&self, action: RecoveryAction) -> Result<RecoverySubmission, RecoveryStorageError> {
        let (generation, disposition) = {
            let mut state = self.lock_writer()?;
            let next = state
                .last_generation
                .checked_add(1)
                .ok_or(RecoveryStorageError::GenerationExhausted)?;
            state.last_generation = next;
            let generation = RecoveryGeneration(next);
            state.latest_generation = Some(generation);
            state.failed_generation = None;
            state.failed_work = None;
            if let Some(superseded) = state.pending.replace(RecoveryWork { generation, action }) {
                state.record_settlement(superseded.generation, RecoverySettlement::Superseded);
                self.0.settled.notify_all();
            }

            if state.writer_active {
                return Ok(RecoverySubmission {
                    generation,
                    disposition: RecoverySubmissionDisposition::Coalesced,
                });
            }
            state.writer_active = true;
            (generation, RecoverySubmissionDisposition::Drained)
        };
        self.spawn_writer()?;
        Ok(RecoverySubmission {
            generation,
            disposition,
        })
    }

    /// Starts one detached writer burst and returns before any slot I/O.
    ///
    /// A bounded settlement wait must include the filesystem operation itself,
    /// so callers cannot be the thread that performs `store` or `clear`.
    fn spawn_writer(&self) -> Result<(), RecoveryStorageError> {
        let storage = self.clone();
        if std::thread::Builder::new()
            .name("origami2-recovery-writer".to_owned())
            .spawn(move || storage.drain_pending())
            .is_ok()
        {
            return Ok(());
        }

        let mut state = self.lock_writer()?;
        state.writer_active = false;
        if let Some(failed) = state.pending.take() {
            let generation = failed.generation;
            state.failed_generation = Some(generation);
            state.record_settlement(generation, RecoverySettlement::Failed);
            state.failed_work = Some(failed);
        }
        self.0.settled.notify_all();
        Err(RecoveryStorageError::StorageUnavailable)
    }

    fn drain_pending(&self) {
        let work = match self.lock_writer() {
            Ok(mut state) => match state.pending.take() {
                Some(work) => work,
                None => {
                    state.writer_active = false;
                    self.0.settled.notify_all();
                    return;
                }
            },
            Err(_) => return,
        };
        let _ = self.drain(work);
    }

    /// Drains one active work item and every latest-wins item queued during its
    /// I/O. The writer mutex is released before each filesystem operation.
    fn drain(&self, mut work: RecoveryWork) -> Result<(), RecoveryStorageError> {
        loop {
            let io_result = match &work.action {
                RecoveryAction::Store(document) => self.0.io.store(&self.0.slot_path, document),
                RecoveryAction::Clear => self.0.io.clear(&self.0.slot_path),
            };
            let failed = io_result.is_err();
            let completed_generation = work.generation;

            let mut state = self.lock_writer()?;
            if !failed {
                state.durable_generation = Some(completed_generation);
            }
            state.record_settlement(
                completed_generation,
                if failed {
                    RecoverySettlement::Failed
                } else {
                    RecoverySettlement::Durable
                },
            );
            self.0.settled.notify_all();
            if let Some(next) = state.pending.take() {
                // Only the newest pending action matters. A failure from an
                // older generation cannot override or block it.
                state.failed_generation = None;
                state.failed_work = None;
                drop(state);
                work = next;
                continue;
            }

            state.writer_active = false;
            if failed {
                state.failed_generation = Some(completed_generation);
                state.failed_work = Some(work);
                return Err(RecoveryStorageError::StorageUnavailable);
            }
            state.failed_generation = None;
            state.failed_work = None;
            return Ok(());
        }
    }

    fn lock_writer(&self) -> Result<MutexGuard<'_, RecoveryWriterState>, RecoveryStorageError> {
        self.0
            .writer
            .lock()
            .map_err(|_| RecoveryStorageError::StateUnavailable)
    }

    #[cfg(test)]
    fn with_io<I>(root: impl Into<PathBuf>, io: Arc<I>) -> Self
    where
        I: RecoveryIo + 'static,
    {
        Self::with_io_inner(root.into(), io)
    }

    #[cfg(test)]
    fn slot_path_for_test(&self) -> &Path {
        &self.0.slot_path
    }
}

impl RecoveryRuntime {
    #[must_use]
    pub(super) fn new(root: impl Into<PathBuf>) -> Self {
        let storage = RecoveryStorage::new(root);
        let startup = storage.inspect_startup();
        let candidate = CachedRecoveryCandidate::from_startup(startup);
        Self(Arc::new(RecoveryRuntimeInner {
            storage,
            operation_gate: Mutex::new(()),
            state: Mutex::new(RecoveryRuntimeState::new(candidate)),
        }))
    }

    fn lock_operation_gate(&self) -> Result<MutexGuard<'_, ()>, RecoveryStorageError> {
        self.0
            .operation_gate
            .lock()
            .map_err(|_| RecoveryStorageError::StateUnavailable)
    }

    fn lock_state(&self) -> Result<MutexGuard<'_, RecoveryRuntimeState>, RecoveryStorageError> {
        self.0
            .state
            .lock()
            .map_err(|_| RecoveryStorageError::StateUnavailable)
    }

    fn candidate_response(&self) -> Result<GetRecoveryCandidateResponse, RecoveryStorageError> {
        let state = self.lock_state()?;
        Ok(match &state.candidate {
            CachedRecoveryCandidate::None
            | CachedRecoveryCandidate::Available { restored: true, .. } => {
                GetRecoveryCandidateResponse::None {
                    schema_version: RECOVERY_SCHEMA_VERSION,
                }
            }
            CachedRecoveryCandidate::Invalid { recovery_id } => {
                GetRecoveryCandidateResponse::Invalid {
                    schema_version: RECOVERY_SCHEMA_VERSION,
                    recovery_id: *recovery_id,
                }
            }
            CachedRecoveryCandidate::Available {
                recovery_id,
                project,
                updated_at_unix_ms,
                restored: false,
            } => GetRecoveryCandidateResponse::Available {
                schema_version: RECOVERY_SCHEMA_VERSION,
                recovery_id: *recovery_id,
                project_id: project.document.project_id,
                updated_at_unix_ms: updated_at_unix_ms.and_then(FrontendSafeUnixMillis::new),
            },
        })
    }

    fn autosave_health_response(
        &self,
    ) -> Result<GetRecoveryAutosaveStatusResponse, RecoveryStorageError> {
        let state = self.lock_state()?;
        Ok(GetRecoveryAutosaveStatusResponse {
            schema_version: RECOVERY_SCHEMA_VERSION,
            status: state.autosave_health_status,
            transition_id: state.autosave_health_transition_id,
        })
    }

    fn record_autosave_observation(
        &self,
        observation: &Result<RecoveryAutosaveOutcome, RecoveryStorageError>,
    ) {
        let status = match observation {
            Ok(
                RecoveryAutosaveOutcome::Stored
                | RecoveryAutosaveOutcome::Cleared
                | RecoveryAutosaveOutcome::Duplicate,
            ) => RecoveryAutosaveHealthStatus::Operational,
            Err(_) => RecoveryAutosaveHealthStatus::PersistenceFailed,
            Ok(
                RecoveryAutosaveOutcome::StartupDecisionPending
                | RecoveryAutosaveOutcome::AutomaticWritesStopped
                | RecoveryAutosaveOutcome::Superseded,
            ) => return,
        };
        if let Ok(mut state) = self.lock_state() {
            state.record_autosave_health(status);
        }
    }

    /// Performs bounded file reinspection without holding either the project
    /// or recovery runtime lock.
    fn prepare_restore(
        &self,
        recovery_id: RecoveryId,
    ) -> Result<PreparedRecoveryRestore, RecoveryStorageError> {
        let (epoch, cached_project) = {
            let state = self.lock_state()?;
            match &state.candidate {
                CachedRecoveryCandidate::Available {
                    recovery_id: current_id,
                    project,
                    restored: false,
                    ..
                } if *current_id == recovery_id => (state.epoch, project.clone()),
                _ => return Err(RecoveryStorageError::StateUnavailable),
            }
        };

        let RecoveryStartup::Available(inspected) = self.0.storage.inspect_startup() else {
            return Err(RecoveryStorageError::StateUnavailable);
        };
        if inspected.project.as_ref() != cached_project.as_ref() {
            return Err(RecoveryStorageError::StateUnavailable);
        }
        let replacement =
            ProjectState::from_recovery_project_archive(inspected.project.as_ref().clone())
                .map_err(|_| RecoveryStorageError::StateUnavailable)?;
        Ok(PreparedRecoveryRestore {
            recovery_id,
            epoch,
            project: inspected.project,
            replacement,
        })
    }

    /// Commits restore under the required `project -> recovery` lock order.
    fn commit_restore(
        &self,
        project: &mut ProjectState,
        request: &RestoreRecoveryRequest,
        prepared: PreparedRecoveryRestore,
    ) -> Result<ProjectSnapshot, RecoveryStorageError> {
        ensure_expected_project(
            project,
            request.expected_instance_id,
            request.expected_project_id,
            request.expected_revision,
        )
        .map_err(|_| RecoveryStorageError::StateUnavailable)?;

        let mut state = self.lock_state()?;
        let next_epoch = state.next_epoch()?;
        let candidate_is_current = matches!(
            &state.candidate,
            CachedRecoveryCandidate::Available {
                recovery_id,
                project,
                restored: false,
                ..
            } if *recovery_id == prepared.recovery_id
                && state.epoch == prepared.epoch
                && project.as_ref() == prepared.project.as_ref()
        );
        if !candidate_is_current {
            return Err(RecoveryStorageError::StateUnavailable);
        }

        commit_project_replacement(project, prepared.replacement)
            .map_err(|_| RecoveryStorageError::StateUnavailable)?;
        if let CachedRecoveryCandidate::Available { restored, .. } = &mut state.candidate {
            *restored = true;
        }
        state.epoch = next_epoch;
        state.last_durable_action = None;
        state.in_flight_operation = None;
        state.prepared_window_close = None;
        Ok(snapshot(project))
    }

    fn capture_autosave(
        project: &ProjectState,
    ) -> Result<RecoveryAutosaveCapture, RecoveryStorageError> {
        Ok(RecoveryAutosaveCapture {
            binding: RecoveryProjectBinding::from_project(project),
            project: if project.is_dirty() {
                Some(
                    project
                        .project_archive()
                        .map_err(|_| RecoveryStorageError::StorageUnavailable)?,
                )
            } else {
                None
            },
        })
    }

    fn autosave(
        &self,
        capture: RecoveryAutosaveCapture,
    ) -> Result<RecoveryAutosaveOutcome, RecoveryStorageError> {
        {
            let state = self.lock_state()?;
            if state.automatic_writes_stopped {
                return Ok(RecoveryAutosaveOutcome::AutomaticWritesStopped);
            }
            if state.candidate.blocks_automatic_writes() {
                return Ok(RecoveryAutosaveOutcome::StartupDecisionPending);
            }
        }

        // Hash the detached persisted-history representation without first
        // allocating another history-sized JSON buffer. The project mutex was
        // released by the caller before entering this method.
        let desired_action = match &capture.project {
            Some(project) => DurableRecoveryAction::Stored(StoredRecoveryIdentity {
                binding: capture.binding,
                history_digest: editor_history_digest(project)?,
            }),
            None => DurableRecoveryAction::Cleared(capture.binding),
        };
        {
            let state = self.lock_state()?;
            if state.automatic_writes_stopped {
                return Ok(RecoveryAutosaveOutcome::AutomaticWritesStopped);
            }
            if state.candidate.blocks_automatic_writes() {
                return Ok(RecoveryAutosaveOutcome::StartupDecisionPending);
            }
            if state.last_durable_action == Some(desired_action) {
                return Ok(RecoveryAutosaveOutcome::Duplicate);
            }
        }

        if let Some(project) = &capture.project {
            // Capture only clones the document and history while the live
            // project lock is held. Rebuild and validate every reachable
            // Undo/Redo instruction-pose endpoint here, after that lock has
            // been released and only for a changed checkpoint identity.
            crate::restore_archive_editor(project)
                .map_err(|_| RecoveryStorageError::StorageUnavailable)?;
        }

        let (epoch, operation_id) = {
            let mut state = self.lock_state()?;
            // Recheck after the potentially expensive semantic validation.
            // A concurrent discard/exit/restore may have changed the runtime
            // epoch or duplicate identity while no state lock was held.
            if state.automatic_writes_stopped {
                return Ok(RecoveryAutosaveOutcome::AutomaticWritesStopped);
            }
            if state.candidate.blocks_automatic_writes() {
                return Ok(RecoveryAutosaveOutcome::StartupDecisionPending);
            }
            if state.last_durable_action == Some(desired_action) {
                return Ok(RecoveryAutosaveOutcome::Duplicate);
            }
            let epoch = state.epoch;
            let operation_id = state.begin_operation()?;
            (epoch, operation_id)
        };

        let submission = match capture.project {
            Some(project) => self.0.storage.store_project(project),
            None => self.0.storage.clear(),
        };
        let submission = match submission {
            Ok(submission) => submission,
            Err(error) => {
                self.finish_failed_operation(operation_id);
                return Err(error);
            }
        };
        let settlement = match self
            .0
            .storage
            .wait_for_settlement(submission.generation, RECOVERY_SETTLEMENT_TIMEOUT)
        {
            Ok(settlement) => settlement,
            Err(error) => {
                self.finish_failed_operation(operation_id);
                return Err(error);
            }
        };

        let mut state = self.lock_state()?;
        if state.in_flight_operation == Some(operation_id) {
            state.in_flight_operation = None;
        }
        if state.epoch != epoch || settlement == RecoverySettlement::Superseded {
            return Ok(RecoveryAutosaveOutcome::Superseded);
        }
        if settlement != RecoverySettlement::Durable {
            return Err(RecoveryStorageError::StorageUnavailable);
        }
        state.last_durable_action = Some(desired_action);
        if matches!(
            state.candidate,
            CachedRecoveryCandidate::Available { restored: true, .. }
        ) {
            state.candidate = CachedRecoveryCandidate::None;
        }
        Ok(match desired_action {
            DurableRecoveryAction::Stored(_) => RecoveryAutosaveOutcome::Stored,
            DurableRecoveryAction::Cleared(_) => RecoveryAutosaveOutcome::Cleared,
        })
    }

    fn finish_failed_operation(&self, operation_id: u64) {
        if let Ok(mut state) = self.lock_state()
            && state.in_flight_operation == Some(operation_id)
        {
            state.in_flight_operation = None;
        }
    }

    /// Clears an obsolete recovery after a successful normal save or project
    /// replacement. The caller must release the project lock first.
    pub(super) fn clear_after_normal_completion(
        &self,
        project_state: &AppState,
        completed_project: &ProjectSnapshot,
    ) -> Result<NormalRecoveryClearOutcome, RecoveryStorageError> {
        // Rebind the delayed clear to the exact project state that completed
        // the normal save/replacement. A newer edit may have committed after
        // the command released its first project guard. Acquiring project then
        // the operation gate makes that edit fall wholly before or after this
        // clear; it can never have a newer recovery store erased by an older
        // completion.
        let completed_binding = RecoveryProjectBinding::from_snapshot(completed_project);
        let project = project_state
            .0
            .lock()
            .map_err(|_| RecoveryStorageError::StateUnavailable)?;
        if RecoveryProjectBinding::from_project(&project) != completed_binding {
            return Ok(NormalRecoveryClearOutcome::SkippedProjectChanged);
        }
        let _operation = self.lock_operation_gate()?;
        let epoch = {
            let mut state = self.lock_state()?;
            let epoch = state.next_epoch()?;
            state.epoch = epoch;
            state.candidate = CachedRecoveryCandidate::None;
            state.last_durable_action = None;
            state.in_flight_operation = None;
            state.prepared_window_close = None;
            epoch
        };
        drop(project);
        let settlement = self.0.storage.clear_and_wait(RECOVERY_SETTLEMENT_TIMEOUT)?;
        if settlement != RecoverySettlement::Durable {
            return Err(RecoveryStorageError::StorageUnavailable);
        }
        let mut state = self.lock_state()?;
        if state.epoch == epoch {
            state.last_durable_action = Some(DurableRecoveryAction::Cleared(completed_binding));
        }
        Ok(NormalRecoveryClearOutcome::Cleared)
    }

    fn discard(&self, recovery_id: RecoveryId) -> Result<(), RecoveryStorageError> {
        let _operation = self.lock_operation_gate()?;
        let mut state = self.lock_state()?;
        if state.candidate.recovery_id() != Some(recovery_id) {
            return Err(RecoveryStorageError::StateUnavailable);
        }
        let next_epoch = state.next_epoch()?;
        state.epoch = next_epoch;
        state.last_durable_action = None;
        state.in_flight_operation = None;
        state.prepared_window_close = None;
        let settlement = self.0.storage.clear_and_wait(RECOVERY_SETTLEMENT_TIMEOUT)?;
        if settlement != RecoverySettlement::Durable {
            return Err(RecoveryStorageError::StorageUnavailable);
        }
        state.candidate = CachedRecoveryCandidate::None;
        Ok(())
    }

    /// Settles a final clear unless an untouched startup candidate still
    /// awaits the user's restore/discard decision.
    pub(super) fn clear_for_exit(
        &self,
        project_state: &AppState,
        authorization: ExitRecoveryAuthorization,
    ) -> Result<ExitRecoveryDisposition, RecoveryStorageError> {
        // Bind the exit decision to the latest native project while taking
        // the same project -> recovery gate order as autosave. A clean exit
        // that races with a new edit is converted into an explicit dirty
        // decision instead of erasing that edit's recovery.
        let project = project_state
            .0
            .lock()
            .map_err(|_| RecoveryStorageError::StateUnavailable)?;
        self.clear_for_exit_locked(project, authorization)
    }

    fn clear_for_exit_locked(
        &self,
        project: MutexGuard<'_, ProjectState>,
        authorization: ExitRecoveryAuthorization,
    ) -> Result<ExitRecoveryDisposition, RecoveryStorageError> {
        if authorization == ExitRecoveryAuthorization::Clean && project.is_dirty() {
            return Ok(ExitRecoveryDisposition::ProjectChanged);
        }
        let _operation = self.lock_operation_gate()?;
        let mut state = self.lock_state()?;
        if state.candidate.blocks_automatic_writes() {
            return Ok(ExitRecoveryDisposition::PreservedStartupCandidate);
        }
        let next_epoch = state.next_epoch()?;
        state.epoch = next_epoch;
        state.last_durable_action = None;
        state.in_flight_operation = None;
        state.automatic_writes_stopped = true;
        state.prepared_window_close = None;
        drop(state);

        let settlement = match self.0.storage.clear_and_wait(RECOVERY_SETTLEMENT_TIMEOUT) {
            Ok(settlement) => settlement,
            Err(error) => {
                if let Ok(mut state) = self.lock_state() {
                    state.automatic_writes_stopped = false;
                }
                return Err(error);
            }
        };
        if settlement != RecoverySettlement::Durable {
            if let Ok(mut state) = self.lock_state() {
                state.automatic_writes_stopped = false;
            }
            return Err(RecoveryStorageError::StorageUnavailable);
        }
        let mut state = self.lock_state()?;
        state.candidate = CachedRecoveryCandidate::None;
        Ok(ExitRecoveryDisposition::Cleared)
    }

    fn prepare_window_close(
        &self,
        project_state: &AppState,
        request: PrepareWindowCloseRequest,
    ) -> Result<PrepareWindowCloseResponse, RecoveryStorageError> {
        if request.schema_version != RECOVERY_SCHEMA_VERSION {
            return Err(RecoveryStorageError::StateUnavailable);
        }
        let project = project_state
            .0
            .lock()
            .map_err(|_| RecoveryStorageError::StateUnavailable)?;
        let requested_binding = RecoveryProjectBinding {
            project_instance_id: request.project_instance_id,
            project_id: request.project_id,
            revision: request.revision,
        };
        if RecoveryProjectBinding::from_project(&project) != requested_binding
            || (request.authorization == ExitRecoveryAuthorization::Clean && project.is_dirty())
        {
            return Err(RecoveryStorageError::StateUnavailable);
        }
        let now = Instant::now();
        let mut state = self.lock_state()?;
        if state.candidate.blocks_automatic_writes() {
            return Err(RecoveryStorageError::StateUnavailable);
        }
        if state
            .prepared_window_close
            .is_some_and(|prepared| prepared.expires_at <= now)
        {
            state.prepared_window_close = None;
        }
        if let Some(prepared) = state.prepared_window_close {
            if prepared.binding != requested_binding
                || prepared.authorization != request.authorization
            {
                return Err(RecoveryStorageError::StateUnavailable);
            }
            return Ok(PrepareWindowCloseResponse {
                schema_version: RECOVERY_SCHEMA_VERSION,
                status: PrepareWindowCloseStatus::Prepared,
                close_prepare_id: prepared.close_prepare_id,
                project_instance_id: request.project_instance_id,
                project_id: request.project_id,
                revision: request.revision,
                authorization: request.authorization,
            });
        }
        let expires_at = now
            .checked_add(WINDOW_CLOSE_PREPARE_LIFETIME)
            .ok_or(RecoveryStorageError::GenerationExhausted)?;
        let close_prepare_id = ClosePrepareId::new();
        state.prepared_window_close = Some(PreparedWindowClose {
            close_prepare_id,
            binding: requested_binding,
            authorization: request.authorization,
            expires_at,
        });
        Ok(PrepareWindowCloseResponse {
            schema_version: RECOVERY_SCHEMA_VERSION,
            status: PrepareWindowCloseStatus::Prepared,
            close_prepare_id,
            project_instance_id: request.project_instance_id,
            project_id: request.project_id,
            revision: request.revision,
            authorization: request.authorization,
        })
    }

    fn cancel_window_close_prepare(
        &self,
        request: CancelWindowClosePrepareRequest,
    ) -> Result<CancelWindowClosePrepareResponse, RecoveryStorageError> {
        if request.schema_version != RECOVERY_SCHEMA_VERSION {
            return Err(RecoveryStorageError::StateUnavailable);
        }
        let requested_binding = RecoveryProjectBinding {
            project_instance_id: request.project_instance_id,
            project_id: request.project_id,
            revision: request.revision,
        };
        let mut state = self.lock_state()?;
        let Some(prepared) = state.prepared_window_close else {
            return Err(RecoveryStorageError::StateUnavailable);
        };
        if prepared.close_prepare_id != request.close_prepare_id
            || prepared.binding != requested_binding
            || prepared.authorization != request.authorization
            || prepared.expires_at <= Instant::now()
        {
            if prepared.expires_at <= Instant::now() {
                state.prepared_window_close = None;
            }
            return Err(RecoveryStorageError::StateUnavailable);
        }
        state.prepared_window_close = None;
        Ok(CancelWindowClosePrepareResponse {
            schema_version: RECOVERY_SCHEMA_VERSION,
            status: CancelWindowClosePrepareStatus::Canceled,
            close_prepare_id: request.close_prepare_id,
            project_instance_id: request.project_instance_id,
            project_id: request.project_id,
            revision: request.revision,
            authorization: request.authorization,
        })
    }

    pub(super) fn settle_prepared_window_close(
        &self,
        project_state: &AppState,
    ) -> Result<PreparedWindowCloseSettlement, RecoveryStorageError> {
        let project = project_state
            .0
            .lock()
            .map_err(|_| RecoveryStorageError::StateUnavailable)?;
        let binding = RecoveryProjectBinding::from_project(&project);
        let prepared = {
            let mut state = self.lock_state()?;
            let Some(prepared) = state.prepared_window_close.take() else {
                return Ok(PreparedWindowCloseSettlement::NotPrepared);
            };
            prepared
        };
        if prepared.expires_at <= Instant::now()
            || prepared.binding != binding
            || (prepared.authorization == ExitRecoveryAuthorization::Clean && project.is_dirty())
        {
            return Ok(PreparedWindowCloseSettlement::Rejected);
        }
        if self.clear_for_exit_locked(project, prepared.authorization)?
            == ExitRecoveryDisposition::Cleared
        {
            Ok(PreparedWindowCloseSettlement::Settled)
        } else {
            Ok(PreparedWindowCloseSettlement::Rejected)
        }
    }

    #[cfg(test)]
    fn with_storage(storage: RecoveryStorage, startup: RecoveryStartup) -> Self {
        Self(Arc::new(RecoveryRuntimeInner {
            storage,
            operation_gate: Mutex::new(()),
            state: Mutex::new(RecoveryRuntimeState::new(
                CachedRecoveryCandidate::from_startup(startup),
            )),
        }))
    }
}

fn run_recovery_autosave_tick(
    project_state: &AppState,
    recovery: &RecoveryRuntime,
) -> Result<RecoveryAutosaveOutcome, RecoveryStorageError> {
    let outcome = (|| {
        // Required lock order: project -> recovery operation gate -> recovery
        // state. The gate remains held after the project snapshot is detached,
        // while the project lock is released before any slot I/O.
        let project = project_state
            .0
            .lock()
            .map_err(|_| RecoveryStorageError::StateUnavailable)?;
        let operation = recovery.lock_operation_gate()?;
        let capture = RecoveryRuntime::capture_autosave(&project)?;
        drop(project);
        let outcome = recovery.autosave(capture);
        drop(operation);
        outcome
    })();
    recovery.record_autosave_observation(&outcome);
    outcome
}

pub(super) fn start_recovery_autosave_timer(
    app_handle: AppHandle,
) -> Result<(), RecoveryStorageError> {
    std::thread::Builder::new()
        .name("origami2-recovery-autosave".to_owned())
        .spawn(move || {
            loop {
                std::thread::sleep(RECOVERY_AUTOSAVE_INTERVAL);
                let project_state = app_handle.state::<AppState>();
                let recovery = app_handle.state::<RecoveryRuntime>();
                // The tick records its redacted health transition before it
                // returns. Its internal error category never crosses into UI.
                let _observed = run_recovery_autosave_tick(&project_state, &recovery);
            }
        })
        .map_err(|_| RecoveryStorageError::StorageUnavailable)?;
    Ok(())
}

#[tauri::command]
pub(super) fn get_recovery_candidate(
    recovery: State<'_, RecoveryRuntime>,
) -> Result<GetRecoveryCandidateResponse, String> {
    recovery
        .candidate_response()
        .map_err(|_| RECOVERY_COMMAND_FAILED_MESSAGE.to_owned())
}

#[tauri::command]
pub(super) fn get_recovery_autosave_status(
    recovery: State<'_, RecoveryRuntime>,
) -> Result<GetRecoveryAutosaveStatusResponse, String> {
    recovery
        .autosave_health_response()
        .map_err(|_| RECOVERY_COMMAND_FAILED_MESSAGE.to_owned())
}

#[tauri::command]
pub(super) async fn restore_recovery(
    state: State<'_, AppState>,
    recovery: State<'_, RecoveryRuntime>,
    request: RestoreRecoveryRequest,
) -> Result<RestoreRecoveryResponse, String> {
    if request.schema_version != RECOVERY_SCHEMA_VERSION {
        return Err(RECOVERY_STALE_MESSAGE.to_owned());
    }
    let runtime = (*recovery).clone();
    let recovery_id = request.recovery_id;
    let prepared =
        tauri::async_runtime::spawn_blocking(move || runtime.prepare_restore(recovery_id))
            .await
            .map_err(|_| RECOVERY_COMMAND_FAILED_MESSAGE.to_owned())?
            .map_err(|_| RECOVERY_STALE_MESSAGE.to_owned())?;

    let mut project = state
        .0
        .lock()
        .map_err(|_| RECOVERY_COMMAND_FAILED_MESSAGE.to_owned())?;
    recovery
        .commit_restore(&mut project, &request, prepared)
        .map_err(|_| RECOVERY_STALE_MESSAGE.to_owned())
}

#[tauri::command]
pub(super) async fn discard_recovery(
    recovery: State<'_, RecoveryRuntime>,
    request: DiscardRecoveryRequest,
) -> Result<DiscardRecoveryResponse, String> {
    if request.schema_version != RECOVERY_SCHEMA_VERSION {
        return Err(RECOVERY_STALE_MESSAGE.to_owned());
    }
    let runtime = (*recovery).clone();
    tauri::async_runtime::spawn_blocking(move || runtime.discard(request.recovery_id))
        .await
        .map_err(|_| RECOVERY_COMMAND_FAILED_MESSAGE.to_owned())?
        .map_err(|_| RECOVERY_STALE_MESSAGE.to_owned())?;
    Ok(DiscardRecoveryResponse {
        schema_version: RECOVERY_SCHEMA_VERSION,
        status: DiscardRecoveryStatus::Discarded,
    })
}

#[tauri::command]
pub(super) fn prepare_window_close(
    state: State<'_, AppState>,
    recovery: State<'_, RecoveryRuntime>,
    request: PrepareWindowCloseRequest,
) -> Result<PrepareWindowCloseResponse, String> {
    recovery
        .prepare_window_close(&state, request)
        .map_err(|_| WINDOW_CLOSE_PREPARE_FAILED_MESSAGE.to_owned())
}

#[tauri::command]
pub(super) fn cancel_window_close_prepare(
    recovery: State<'_, RecoveryRuntime>,
    request: CancelWindowClosePrepareRequest,
) -> Result<CancelWindowClosePrepareResponse, String> {
    recovery
        .cancel_window_close_prepare(request)
        .map_err(|_| WINDOW_CLOSE_PREPARE_FAILED_MESSAGE.to_owned())
}

#[cfg(test)]
mod tests {
    use std::{
        fs::{self, File},
        path::Path,
        sync::{
            Arc, Condvar, Mutex,
            atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        },
        thread,
    };

    use ori_core::{Command, create_rectangular_sheet};
    use ori_domain::{
        CreasePattern, FaceId, InstructionPose, InstructionPoseModel, InstructionStep,
        InstructionStepId, Paper, Point2, VertexId,
    };
    use ori_formats::{Ori2Limits, ProjectDocument};
    use serde_json::json;

    use super::*;
    use crate::project_persistence::{frontend_safe_unix_millis, stage_recovery_project_for_test};

    static NEXT_TEST_DIRECTORY_ID: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new(label: &str) -> Self {
            let id = NEXT_TEST_DIRECTORY_ID.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "origami2-recovery-{label}-{}-{id}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("create recovery test directory");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn document(name: &str) -> ProjectDocument {
        ProjectDocument::new(name, CreasePattern::empty())
    }

    fn project(document: ProjectDocument) -> Ori2ProjectArchive {
        Ori2ProjectArchive::document_only(document)
    }

    fn state_with_reachable_invalid_instruction_pose() -> ProjectState {
        let sheet = create_rectangular_sheet(40.0, 40.0, false).expect("valid history test sheet");
        let (pattern, paper) = sheet.into_parts();
        let mut source = ProjectState::new_unsaved("unsafe recovery".to_owned(), pattern, paper);
        let old_fingerprint = source.editor.fold_model_fingerprint_v1();
        source
            .editor
            .execute(
                0,
                Command::AddInstructionStep {
                    step: InstructionStep {
                        id: InstructionStepId::new(),
                        title: "invalid after Undo".to_owned(),
                        description: String::new(),
                        caution: String::new(),
                        duration_ms: 1_000,
                        visual: Default::default(),
                        pose: InstructionPose {
                            model: InstructionPoseModel::AbsoluteHingeAnglesV1,
                            source_model_fingerprint: old_fingerprint,
                            fixed_face: Some(FaceId::new()),
                            hinge_angles: Vec::new(),
                        },
                    },
                },
            )
            .expect("add structurally valid instruction step");
        source
            .editor
            .execute(
                1,
                Command::AddVertex {
                    id: VertexId::new(),
                    position: Point2::new(20.0, 20.0),
                },
            )
            .expect("make the invalid pose stale at the current endpoint");
        source
    }

    fn project_with_reachable_invalid_instruction_pose() -> Ori2ProjectArchive {
        let source = state_with_reachable_invalid_instruction_pose();
        Ori2ProjectArchive {
            document: source.document(),
            editor_history: Some(
                source
                    .editor
                    .export_history_v1(source.project_id)
                    .expect("export unsafe recovery fixture"),
            ),
        }
    }

    fn prepare_close_request(
        project: &ProjectState,
        authorization: ExitRecoveryAuthorization,
    ) -> PrepareWindowCloseRequest {
        PrepareWindowCloseRequest {
            schema_version: RECOVERY_SCHEMA_VERSION,
            project_instance_id: project.instance_id,
            project_id: project.project_id,
            revision: project.editor.revision(),
            authorization,
        }
    }

    fn available_document(storage: &RecoveryStorage) -> ProjectDocument {
        match storage.inspect_startup() {
            RecoveryStartup::Available(candidate) => candidate.project.document,
            RecoveryStartup::None => panic!("expected an available recovery document"),
            RecoveryStartup::Invalid => panic!("expected a valid recovery document"),
        }
    }

    struct MemoryRecoveryIo {
        document: Mutex<Option<Ori2ProjectArchive>>,
        calls: Mutex<Vec<String>>,
        fail_next_clear: AtomicBool,
    }

    impl MemoryRecoveryIo {
        fn new(document: Option<ProjectDocument>) -> Self {
            Self::new_project(document.map(project))
        }

        fn new_project(project: Option<Ori2ProjectArchive>) -> Self {
            Self {
                document: Mutex::new(project),
                calls: Mutex::new(Vec::new()),
                fail_next_clear: AtomicBool::new(false),
            }
        }

        fn fail_next_clear(&self) {
            self.fail_next_clear.store(true, Ordering::SeqCst);
        }

        fn document(&self) -> Option<ProjectDocument> {
            self.document
                .lock()
                .unwrap()
                .as_ref()
                .map(|project| project.document.clone())
        }

        fn project(&self) -> Option<Ori2ProjectArchive> {
            self.document.lock().unwrap().clone()
        }

        fn calls(&self) -> Vec<String> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl RecoveryIo for MemoryRecoveryIo {
        fn inspect(&self, _slot_path: &Path) -> RecoveryStartup {
            match self.project() {
                Some(project) => RecoveryStartup::Available(RecoveryStartupCandidate {
                    project: Box::new(project),
                    updated_at_unix_ms: Some(123),
                }),
                None => RecoveryStartup::None,
            }
        }

        fn store(
            &self,
            _slot_path: &Path,
            project: &Ori2ProjectArchive,
        ) -> Result<(), RecoveryPersistenceError> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("store:{}", project.document.name));
            *self.document.lock().unwrap() = Some(project.clone());
            Ok(())
        }

        fn clear(&self, _slot_path: &Path) -> Result<(), RecoveryPersistenceError> {
            self.calls.lock().unwrap().push("clear".to_owned());
            if self.fail_next_clear.swap(false, Ordering::SeqCst) {
                return Err(RecoveryPersistenceError);
            }
            *self.document.lock().unwrap() = None;
            Ok(())
        }
    }

    #[test]
    fn recovery_dto_schema_is_exact_and_requests_deny_unknown_fields() {
        let project_id = ProjectId::new();
        let instance_id = ProjectId::new();
        let recovery_id = RecoveryId::new();
        let recovery_id_value = serde_json::to_value(recovery_id).unwrap();
        let recovery_id_text = recovery_id_value.as_str().unwrap();
        assert!(is_canonical_non_nil_uuid(recovery_id_text));
        assert_eq!(recovery_id_text, recovery_id_text.to_ascii_lowercase());
        let none = serde_json::to_value(GetRecoveryCandidateResponse::None {
            schema_version: RECOVERY_SCHEMA_VERSION,
        })
        .unwrap();
        assert_eq!(none, json!({"schema_version": 1, "status": "none"}));
        assert_eq!(
            serde_json::to_value(GetRecoveryAutosaveStatusResponse {
                schema_version: RECOVERY_SCHEMA_VERSION,
                status: RecoveryAutosaveHealthStatus::PendingFirstAttempt,
                transition_id: 0,
            })
            .unwrap(),
            json!({
                "schema_version": 1,
                "status": "pending_first_attempt",
                "transition_id": 0
            })
        );

        let invalid = serde_json::to_value(GetRecoveryCandidateResponse::Invalid {
            schema_version: RECOVERY_SCHEMA_VERSION,
            recovery_id,
        })
        .unwrap();
        assert_eq!(
            invalid,
            json!({
                "schema_version": 1,
                "status": "invalid",
                "recovery_id": recovery_id
            })
        );

        let available = serde_json::to_value(GetRecoveryCandidateResponse::Available {
            schema_version: RECOVERY_SCHEMA_VERSION,
            recovery_id,
            project_id,
            updated_at_unix_ms: None,
        })
        .unwrap();
        assert_eq!(
            available,
            json!({
                "schema_version": 1,
                "status": "available",
                "recovery_id": recovery_id,
                "project_id": project_id,
                "updated_at_unix_ms": null
            })
        );
        assert!(available.get("project_name").is_none());
        assert_eq!(
            serde_json::to_value(GetRecoveryCandidateResponse::Available {
                schema_version: RECOVERY_SCHEMA_VERSION,
                recovery_id,
                project_id,
                updated_at_unix_ms: FrontendSafeUnixMillis::new(123),
            })
            .unwrap()["updated_at_unix_ms"],
            json!(123)
        );
        assert_eq!(
            frontend_safe_unix_millis(Duration::from_millis(FRONTEND_MAX_SAFE_INTEGER_U64)),
            Some(FRONTEND_MAX_SAFE_INTEGER_U64)
        );
        assert_eq!(
            frontend_safe_unix_millis(Duration::from_millis(FRONTEND_MAX_SAFE_INTEGER_U64 + 1)),
            None
        );
        assert!(FrontendSafeUnixMillis::new(FRONTEND_MAX_SAFE_INTEGER_U64 + 1).is_none());

        let restore_value = json!({
            "schema_version": 1,
            "recovery_id": recovery_id,
            "expected_project_id": project_id,
            "expected_instance_id": instance_id,
            "expected_revision": 7
        });
        let restore: RestoreRecoveryRequest =
            serde_json::from_value(restore_value.clone()).unwrap();
        assert_eq!(restore.expected_revision, 7);
        let mut restore_with_noncanonical_project = restore_value.clone();
        restore_with_noncanonical_project["expected_project_id"] = json!(
            serde_json::to_value(project_id)
                .unwrap()
                .as_str()
                .unwrap()
                .to_ascii_uppercase()
        );
        assert!(
            serde_json::from_value::<RestoreRecoveryRequest>(restore_with_noncanonical_project)
                .is_err()
        );
        let mut restore_with_unknown = restore_value;
        restore_with_unknown["unexpected"] = json!(true);
        assert!(serde_json::from_value::<RestoreRecoveryRequest>(restore_with_unknown).is_err());

        let discard: DiscardRecoveryRequest = serde_json::from_value(json!({
            "schema_version": 1,
            "recovery_id": recovery_id
        }))
        .unwrap();
        assert_eq!(discard.recovery_id, recovery_id);
        assert!(
            serde_json::from_value::<DiscardRecoveryRequest>(json!({
                "schema_version": 1,
                "recovery_id": recovery_id,
                "unexpected": 1
            }))
            .is_err()
        );
        assert!(
            serde_json::from_value::<DiscardRecoveryRequest>(json!({
                "schema_version": 1,
                "recovery_id": recovery_id_text.to_ascii_uppercase()
            }))
            .is_err()
        );
        assert!(
            serde_json::from_value::<DiscardRecoveryRequest>(json!({
                "schema_version": 1,
                "recovery_id": "00000000-0000-0000-0000-000000000000"
            }))
            .is_err()
        );
        assert_eq!(
            serde_json::to_value(DiscardRecoveryResponse {
                schema_version: RECOVERY_SCHEMA_VERSION,
                status: DiscardRecoveryStatus::Discarded,
            })
            .unwrap(),
            json!({"schema_version": 1, "status": "discarded"})
        );

        let prepare_id = ClosePrepareId::new();
        let prepare_request_value = json!({
            "schema_version": 1,
            "project_instance_id": instance_id,
            "project_id": project_id,
            "revision": 7,
            "authorization": "discard_confirmed"
        });
        let prepare_request: PrepareWindowCloseRequest =
            serde_json::from_value(prepare_request_value.clone()).unwrap();
        assert_eq!(
            prepare_request.authorization,
            ExitRecoveryAuthorization::DiscardConfirmed
        );
        let mut prepare_with_unknown = prepare_request_value;
        prepare_with_unknown["unexpected"] = json!(true);
        assert!(serde_json::from_value::<PrepareWindowCloseRequest>(prepare_with_unknown).is_err());
        assert_eq!(
            serde_json::to_value(PrepareWindowCloseResponse {
                schema_version: RECOVERY_SCHEMA_VERSION,
                status: PrepareWindowCloseStatus::Prepared,
                close_prepare_id: prepare_id,
                project_instance_id: instance_id,
                project_id,
                revision: 7,
                authorization: ExitRecoveryAuthorization::DiscardConfirmed,
            })
            .unwrap(),
            json!({
                "schema_version": 1,
                "status": "prepared",
                "close_prepare_id": prepare_id,
                "project_instance_id": instance_id,
                "project_id": project_id,
                "revision": 7,
                "authorization": "discard_confirmed"
            })
        );

        let cancel_value = json!({
            "schema_version": 1,
            "close_prepare_id": prepare_id,
            "project_instance_id": instance_id,
            "project_id": project_id,
            "revision": 7,
            "authorization": "discard_confirmed"
        });
        let cancel_request: CancelWindowClosePrepareRequest =
            serde_json::from_value(cancel_value.clone()).unwrap();
        assert_eq!(cancel_request.close_prepare_id, prepare_id);
        let mut cancel_with_uppercase_token = cancel_value;
        cancel_with_uppercase_token["close_prepare_id"] = json!(
            serde_json::to_value(prepare_id)
                .unwrap()
                .as_str()
                .unwrap()
                .to_ascii_uppercase()
        );
        assert!(
            serde_json::from_value::<CancelWindowClosePrepareRequest>(cancel_with_uppercase_token)
                .is_err()
        );
        assert_eq!(
            serde_json::to_value(CancelWindowClosePrepareResponse {
                schema_version: RECOVERY_SCHEMA_VERSION,
                status: CancelWindowClosePrepareStatus::Canceled,
                close_prepare_id: prepare_id,
                project_instance_id: instance_id,
                project_id,
                revision: 7,
                authorization: ExitRecoveryAuthorization::DiscardConfirmed,
            })
            .unwrap(),
            json!({
                "schema_version": 1,
                "status": "canceled",
                "close_prepare_id": prepare_id,
                "project_instance_id": instance_id,
                "project_id": project_id,
                "revision": 7,
                "authorization": "discard_confirmed"
            })
        );
    }

    #[test]
    fn recovery_round_trips_and_clear_is_idempotent() {
        let directory = TestDirectory::new("roundtrip");
        let storage = RecoveryStorage::new(directory.path());
        assert!(matches!(storage.inspect_startup(), RecoveryStartup::None));

        let expected = document("roundtrip");
        let submission = storage.store(expected.clone()).unwrap();
        assert_eq!(submission.generation.get(), 1);
        assert_eq!(
            submission.disposition,
            RecoverySubmissionDisposition::Drained
        );
        assert_eq!(
            storage
                .wait_for_settlement(submission.generation, Duration::from_secs(1))
                .unwrap(),
            RecoverySettlement::Durable
        );
        match storage.inspect_startup() {
            RecoveryStartup::Available(candidate) => {
                assert_eq!(candidate.document(), &expected);
                // Filesystems may omit or predate a representable timestamp.
                let _ = candidate.updated_at_unix_ms();
            }
            RecoveryStartup::None => panic!("recovery store was not published"),
            RecoveryStartup::Invalid => panic!("published recovery store was invalid"),
        }

        let cleared = storage.clear().unwrap();
        assert_eq!(cleared.generation.get(), 2);
        assert_eq!(
            storage
                .wait_for_settlement(cleared.generation, Duration::from_secs(1))
                .unwrap(),
            RecoverySettlement::Durable
        );
        assert!(matches!(storage.inspect_startup(), RecoveryStartup::None));
        assert_eq!(
            storage.clear_and_wait(Duration::from_secs(1)).unwrap(),
            RecoverySettlement::Durable
        );
        assert!(matches!(storage.inspect_startup(), RecoveryStartup::None));
    }

    #[test]
    fn invalid_and_oversized_slots_are_bounded_and_remain_explicit() {
        let directory = TestDirectory::new("invalid");
        let storage = RecoveryStorage::new(directory.path());
        fs::write(storage.slot_path_for_test(), b"not an ori2 archive").unwrap();
        assert!(matches!(
            storage.inspect_startup(),
            RecoveryStartup::Invalid
        ));

        storage.clear_and_wait(Duration::from_secs(1)).unwrap();
        let oversized = File::create(storage.slot_path_for_test()).unwrap();
        oversized
            .set_len(Ori2Limits::default().max_archive_size.saturating_add(1))
            .unwrap();
        drop(oversized);
        assert!(matches!(
            storage.inspect_startup(),
            RecoveryStartup::Invalid
        ));

        storage.clear_and_wait(Duration::from_secs(1)).unwrap();
        assert!(matches!(storage.inspect_startup(), RecoveryStartup::None));
    }

    #[test]
    fn reachable_invalid_instruction_pose_history_is_a_startup_invalid_candidate() {
        let directory = TestDirectory::new("invalid-history-endpoint");
        let io = Arc::new(MemoryRecoveryIo::new_project(Some(
            project_with_reachable_invalid_instruction_pose(),
        )));
        let storage = RecoveryStorage::with_io(directory.path(), Arc::clone(&io));
        let startup = storage.inspect_startup();
        assert!(matches!(startup, RecoveryStartup::Available(_)));

        let runtime = RecoveryRuntime::with_storage(storage, startup);

        assert!(matches!(
            runtime.candidate_response().unwrap(),
            GetRecoveryCandidateResponse::Invalid { .. }
        ));
    }

    #[test]
    fn autosave_validates_detached_history_only_after_project_capture() {
        let directory = TestDirectory::new("detached-history-validation");
        let source = state_with_reachable_invalid_instruction_pose();
        let capture = RecoveryRuntime::capture_autosave(&source)
            .expect("capture only detaches document and history");
        assert!(capture.project.is_some());

        let io = Arc::new(MemoryRecoveryIo::new(None));
        let storage = RecoveryStorage::with_io(directory.path(), Arc::clone(&io));
        let runtime = RecoveryRuntime::with_storage(storage, RecoveryStartup::None);
        assert_eq!(
            runtime.autosave(capture),
            Err(RecoveryStorageError::StorageUnavailable)
        );
        assert!(io.calls().is_empty());
        assert!(io.project().is_none());
    }

    #[test]
    fn abandoning_a_verified_stage_preserves_the_previous_recovery() {
        let directory = TestDirectory::new("atomic-interruption");
        let storage = RecoveryStorage::new(directory.path());
        let old = document("old");
        let new = document("new");
        let old_store = storage.store(old.clone()).unwrap();
        assert_eq!(
            storage
                .wait_for_settlement(old_store.generation, Duration::from_secs(1))
                .unwrap(),
            RecoverySettlement::Durable
        );

        let staged =
            stage_recovery_project_for_test(storage.slot_path_for_test(), &project(new)).unwrap();
        let staged_path = staged.path.clone();
        assert!(staged_path.exists());
        drop(staged);

        assert!(!staged_path.exists());
        assert_eq!(available_document(&storage), old);
    }

    #[test]
    fn restore_uses_fresh_runtime_state_and_never_inherits_the_original_path() {
        let directory = TestDirectory::new("restore");
        let original_path = directory.path().join("original.ori2");
        fs::write(&original_path, b"original sentinel").unwrap();
        let original_bytes = fs::read(&original_path).unwrap();
        let expected = document("recover me");
        let opened = ProjectState::from_document(expected.clone(), original_path.clone());
        let old_instance = opened.instance_id;

        let restored = RecoveryStartupCandidate {
            project: Box::new(project(expected.clone())),
            updated_at_unix_ms: Some(123),
        }
        .into_project_state()
        .expect("restore validated recovery archive");

        assert_eq!(restored.project_id, expected.project_id);
        assert_ne!(restored.instance_id, old_instance);
        assert_eq!(restored.name, expected.name);
        assert!(restored.current_path.is_none());
        assert!(restored.saved_revision.is_none());
        assert!(restored.saved_document.is_none());
        assert_eq!(restored.editor.revision(), 0);
        assert!(!restored.editor.can_undo());
        assert!(!restored.editor.can_redo());
        assert!(restored.editor.current_applied_pose().is_none());
        let authority = restored.applied_pose_authority.test_snapshot().unwrap();
        assert_eq!(authority.generation, 0);
        assert!(!authority.has_current);
        assert!(!authority.has_pending);
        assert!(restored.is_dirty());
        assert_eq!(restored.document(), expected);
        assert_eq!(fs::read(original_path).unwrap(), original_bytes);
    }

    #[test]
    fn runtime_restore_keeps_internal_candidate_until_the_next_autosave() {
        let directory = TestDirectory::new("runtime-restore");
        let recovered = document("recovered");
        let io = Arc::new(MemoryRecoveryIo::new(Some(recovered.clone())));
        let storage = RecoveryStorage::with_io(directory.path(), Arc::clone(&io));
        let runtime = RecoveryRuntime::with_storage(
            storage,
            RecoveryStartup::Available(RecoveryStartupCandidate {
                project: Box::new(project(recovered.clone())),
                updated_at_unix_ms: Some(123),
            }),
        );
        let recovery_id = match runtime.candidate_response().unwrap() {
            GetRecoveryCandidateResponse::Available { recovery_id, .. } => recovery_id,
            _ => panic!("startup recovery must be available"),
        };
        let app_state = AppState::new(crate::initial_project_state());
        let request = {
            let project = app_state.0.lock().unwrap();
            RestoreRecoveryRequest {
                schema_version: RECOVERY_SCHEMA_VERSION,
                recovery_id,
                expected_project_id: project.project_id,
                expected_instance_id: project.instance_id,
                expected_revision: project.editor.revision(),
            }
        };
        let stale_prepared = runtime.prepare_restore(recovery_id).unwrap();
        let mut stale_request = request.clone();
        stale_request.expected_revision = stale_request.expected_revision.saturating_add(1);
        {
            let mut project = app_state.0.lock().unwrap();
            assert!(
                runtime
                    .commit_restore(&mut project, &stale_request, stale_prepared)
                    .is_err()
            );
        }
        assert!(matches!(
            runtime.candidate_response().unwrap(),
            GetRecoveryCandidateResponse::Available { .. }
        ));

        let prepared = runtime.prepare_restore(recovery_id).unwrap();
        {
            let mut project = app_state.0.lock().unwrap();
            let restored = runtime
                .commit_restore(&mut project, &request, prepared)
                .unwrap();
            assert_eq!(restored.project_id, recovered.project_id);
            assert!(restored.is_dirty);
        }
        assert!(matches!(
            runtime.candidate_response().unwrap(),
            GetRecoveryCandidateResponse::None { .. }
        ));
        assert!(matches!(
            &runtime.lock_state().unwrap().candidate,
            CachedRecoveryCandidate::Available { restored: true, .. }
        ));

        assert_eq!(
            run_recovery_autosave_tick(&app_state, &runtime).unwrap(),
            RecoveryAutosaveOutcome::Stored
        );
        assert!(matches!(
            &runtime.lock_state().unwrap().candidate,
            CachedRecoveryCandidate::None
        ));
        assert_eq!(io.document(), Some(recovered));
        assert_eq!(io.calls(), vec!["store:recovered"]);
    }

    #[test]
    fn dirty_autosave_and_startup_restore_preserve_limit_and_both_history_stacks() {
        let directory = TestDirectory::new("history-roundtrip");
        let mut source = ProjectState::new_unsaved(
            "history recovery".to_owned(),
            CreasePattern::empty(),
            Paper::default(),
        );
        source
            .editor
            .set_history_entry_limit(17)
            .expect("configure persisted history limit");
        let project_id = source.project_id;
        let source_instance_id = source.instance_id;
        let first = VertexId::new();
        let second = VertexId::new();
        source
            .editor
            .execute(
                0,
                Command::AddVertex {
                    id: first,
                    position: Point2::new(12.0, 34.0),
                },
            )
            .expect("first recovery history command");
        source
            .editor
            .execute(
                1,
                Command::AddVertex {
                    id: second,
                    position: Point2::new(56.0, 78.0),
                },
            )
            .expect("second recovery history command");
        source
            .editor
            .undo(2)
            .expect("populate the recovery Redo stack");
        assert!(source.is_dirty());
        assert!(source.editor.can_undo());
        assert!(source.editor.can_redo());
        let expected_document = source.document();
        let expected_history = source
            .editor
            .export_history_v1(project_id)
            .expect("export recovery history");

        let app_state = AppState::new(source);
        let io = Arc::new(MemoryRecoveryIo::new(None));
        let storage = RecoveryStorage::with_io(directory.path(), Arc::clone(&io));
        let runtime = RecoveryRuntime::with_storage(storage, RecoveryStartup::None);
        assert_eq!(
            run_recovery_autosave_tick(&app_state, &runtime).unwrap(),
            RecoveryAutosaveOutcome::Stored
        );
        let stored = io.project().expect("autosave persisted an archive");
        assert_eq!(stored.document, expected_document);
        assert_eq!(
            stored.editor_history.as_ref(),
            Some(&expected_history),
            "the autosave checkpoint must include exact history"
        );

        let restarted_io = Arc::new(MemoryRecoveryIo::new_project(Some(stored)));
        let restarted_storage =
            RecoveryStorage::with_io(directory.path(), Arc::clone(&restarted_io));
        let startup = restarted_storage.inspect_startup();
        let restarted_runtime = RecoveryRuntime::with_storage(restarted_storage, startup);
        let recovery_id = match restarted_runtime.candidate_response().unwrap() {
            GetRecoveryCandidateResponse::Available { recovery_id, .. } => recovery_id,
            _ => panic!("the persisted recovery must be offered at startup"),
        };
        let mut restored = crate::initial_project_state();
        let replaced_instance_id = restored.instance_id;
        let request = RestoreRecoveryRequest {
            schema_version: RECOVERY_SCHEMA_VERSION,
            recovery_id,
            expected_project_id: restored.project_id,
            expected_instance_id: restored.instance_id,
            expected_revision: restored.editor.revision(),
        };
        let prepared = restarted_runtime.prepare_restore(recovery_id).unwrap();
        restarted_runtime
            .commit_restore(&mut restored, &request, prepared)
            .expect("commit startup history recovery");

        assert_eq!(restored.project_id, project_id);
        assert_ne!(restored.instance_id, source_instance_id);
        assert_ne!(restored.instance_id, replaced_instance_id);
        assert_eq!(restored.document(), expected_document);
        assert!(restored.current_path.is_none());
        assert!(restored.saved_revision.is_none());
        assert!(restored.saved_document.is_none());
        assert!(restored.is_dirty());
        assert_eq!(restored.editor.revision(), 0);
        assert_eq!(restored.editor.history_entry_limit(), 17);
        assert!(restored.editor.can_undo());
        assert!(restored.editor.can_redo());
        assert!(restored.editor.current_applied_pose().is_none());
        let authority = restored.applied_pose_authority.test_snapshot().unwrap();
        assert_eq!(authority.generation, 1);
        assert!(!authority.has_current);
        assert!(!authority.has_pending);
        assert_eq!(
            restored
                .editor
                .export_history_v1(project_id)
                .expect("re-export restored recovery history"),
            expected_history
        );

        restored.editor.redo(0).expect("redo second command");
        assert_eq!(
            restored
                .editor
                .pattern()
                .vertices
                .iter()
                .map(|vertex| vertex.id)
                .collect::<Vec<_>>(),
            vec![first, second]
        );
        restored.editor.undo(1).expect("undo second command");
        assert_eq!(restored.document(), expected_document);
        restored.editor.undo(2).expect("undo first command");
        assert!(restored.editor.pattern().vertices.is_empty());
    }

    #[test]
    fn startup_candidate_blocks_timer_but_normal_completion_clears_it() {
        let directory = TestDirectory::new("startup-gate");
        let recovered = document("pending");
        let io = Arc::new(MemoryRecoveryIo::new(Some(recovered.clone())));
        let storage = RecoveryStorage::with_io(directory.path(), Arc::clone(&io));
        let runtime = RecoveryRuntime::with_storage(
            storage,
            RecoveryStartup::Available(RecoveryStartupCandidate {
                project: Box::new(project(recovered)),
                updated_at_unix_ms: None,
            }),
        );
        let app_state = AppState::new(ProjectState::new_unsaved(
            "new work".to_owned(),
            CreasePattern::empty(),
            Paper::default(),
        ));
        assert_eq!(
            run_recovery_autosave_tick(&app_state, &runtime).unwrap(),
            RecoveryAutosaveOutcome::StartupDecisionPending
        );
        assert!(io.calls().is_empty());

        let current = {
            let project = app_state.0.lock().unwrap();
            snapshot(&project)
        };
        assert_eq!(
            runtime
                .clear_after_normal_completion(&app_state, &current)
                .unwrap(),
            NormalRecoveryClearOutcome::Cleared
        );
        assert_eq!(io.calls(), vec!["clear"]);
        assert!(io.document().is_none());
        assert!(matches!(
            runtime.candidate_response().unwrap(),
            GetRecoveryCandidateResponse::None { .. }
        ));
    }

    #[test]
    fn explicit_discard_requires_the_cached_token_and_settles_clear() {
        let directory = TestDirectory::new("explicit-discard");
        let recovered = document("discard");
        let io = Arc::new(MemoryRecoveryIo::new(Some(recovered.clone())));
        let storage = RecoveryStorage::with_io(directory.path(), Arc::clone(&io));
        let runtime = RecoveryRuntime::with_storage(
            storage,
            RecoveryStartup::Available(RecoveryStartupCandidate {
                project: Box::new(project(recovered)),
                updated_at_unix_ms: None,
            }),
        );
        let recovery_id = match runtime.candidate_response().unwrap() {
            GetRecoveryCandidateResponse::Available { recovery_id, .. } => recovery_id,
            _ => panic!("candidate must be available"),
        };
        assert!(runtime.discard(RecoveryId::new()).is_err());
        assert!(io.document().is_some());

        runtime.discard(recovery_id).unwrap();
        assert!(io.document().is_none());
        assert_eq!(io.calls(), vec!["clear"]);
        assert!(matches!(
            runtime.candidate_response().unwrap(),
            GetRecoveryCandidateResponse::None { .. }
        ));
    }

    #[test]
    fn restore_reinspection_rejects_a_changed_slot_without_consuming_candidate() {
        let directory = TestDirectory::new("restore-reinspect");
        let cached = document("cached");
        let io = Arc::new(MemoryRecoveryIo::new(Some(cached.clone())));
        let storage = RecoveryStorage::with_io(directory.path(), Arc::clone(&io));
        let runtime = RecoveryRuntime::with_storage(
            storage,
            RecoveryStartup::Available(RecoveryStartupCandidate {
                project: Box::new(project(cached)),
                updated_at_unix_ms: None,
            }),
        );
        let recovery_id = match runtime.candidate_response().unwrap() {
            GetRecoveryCandidateResponse::Available { recovery_id, .. } => recovery_id,
            _ => panic!("candidate must be available"),
        };
        *io.document.lock().unwrap() = Some(project(document("externally changed")));

        assert!(runtime.prepare_restore(recovery_id).is_err());
        assert!(matches!(
            runtime.candidate_response().unwrap(),
            GetRecoveryCandidateResponse::Available { .. }
        ));
    }

    #[test]
    fn normal_clear_failure_is_retried_by_the_next_clean_tick() {
        let directory = TestDirectory::new("normal-clear-retry");
        let stale = document("stale");
        let io = Arc::new(MemoryRecoveryIo::new(Some(stale.clone())));
        io.fail_next_clear();
        let storage = RecoveryStorage::with_io(directory.path(), Arc::clone(&io));
        let runtime = RecoveryRuntime::with_storage(
            storage,
            RecoveryStartup::Available(RecoveryStartupCandidate {
                project: Box::new(project(stale)),
                updated_at_unix_ms: None,
            }),
        );
        let app_state = AppState::new(crate::initial_project_state());
        let clean = {
            let project = app_state.0.lock().unwrap();
            snapshot(&project)
        };
        assert!(
            runtime
                .clear_after_normal_completion(&app_state, &clean)
                .is_err()
        );
        assert!(io.document().is_some());
        assert!(runtime.lock_state().unwrap().last_durable_action.is_none());

        assert_eq!(
            run_recovery_autosave_tick(&app_state, &runtime).unwrap(),
            RecoveryAutosaveOutcome::Cleared
        );
        assert!(io.document().is_none());
        assert_eq!(io.calls(), vec!["clear", "clear"]);
    }

    #[test]
    fn delayed_normal_clear_preserves_recovery_for_a_newer_project_revision() {
        let directory = TestDirectory::new("normal-clear-stale-binding");
        let io = Arc::new(MemoryRecoveryIo::new(None));
        let storage = RecoveryStorage::with_io(directory.path(), Arc::clone(&io));
        let runtime = RecoveryRuntime::with_storage(storage, RecoveryStartup::None);
        let app_state = AppState::new(ProjectState::new_unsaved(
            "edited-after-save".to_owned(),
            CreasePattern::empty(),
            Paper::default(),
        ));
        let completed_save = {
            let mut project = app_state.0.lock().unwrap();
            let saved_document = project.document();
            project.saved_revision = Some(project.editor.revision());
            project.saved_document = Some(saved_document);
            let completed = snapshot(&project);
            project
                .editor
                .execute(
                    completed.revision,
                    Command::AddVertex {
                        id: VertexId::new(),
                        position: Point2::new(12.0, 34.0),
                    },
                )
                .unwrap();
            completed
        };

        assert_eq!(
            run_recovery_autosave_tick(&app_state, &runtime).unwrap(),
            RecoveryAutosaveOutcome::Stored
        );
        assert_eq!(
            runtime
                .clear_after_normal_completion(&app_state, &completed_save)
                .unwrap(),
            NormalRecoveryClearOutcome::SkippedProjectChanged
        );
        assert!(io.document().is_some());
        assert_eq!(io.calls(), vec!["store:edited-after-save"]);
    }

    #[test]
    fn same_revision_history_limit_change_updates_checkpoint_then_deduplicates() {
        let directory = TestDirectory::new("history-digest-identity");
        let mut source = ProjectState::new_unsaved(
            "history digest".to_owned(),
            CreasePattern::empty(),
            Paper::default(),
        );
        source
            .editor
            .set_history_entry_limit(17)
            .expect("set initial history limit");
        for index in 0_u64..12 {
            source
                .editor
                .execute(
                    index,
                    Command::AddVertex {
                        id: VertexId::new(),
                        position: Point2::new(index as f64, index as f64 + 0.5),
                    },
                )
                .expect("populate bounded Undo history");
        }
        let unchanged_revision = source.editor.revision();
        let app_state = AppState::new(source);
        let io = Arc::new(MemoryRecoveryIo::new(None));
        let storage = RecoveryStorage::with_io(directory.path(), Arc::clone(&io));
        let runtime = RecoveryRuntime::with_storage(storage, RecoveryStartup::None);

        assert_eq!(
            run_recovery_autosave_tick(&app_state, &runtime).unwrap(),
            RecoveryAutosaveOutcome::Stored
        );
        let first_history = io
            .project()
            .and_then(|project| project.editor_history)
            .expect("initial history checkpoint");
        assert_eq!(first_history.history_entry_limit(), 17);

        {
            let mut project = app_state.0.lock().unwrap();
            project
                .editor
                .set_history_entry_limit(9)
                .expect("change and trim history without editing the document");
            assert_eq!(project.editor.revision(), unchanged_revision);
        }
        assert_eq!(
            run_recovery_autosave_tick(&app_state, &runtime).unwrap(),
            RecoveryAutosaveOutcome::Stored
        );
        let updated_history = io
            .project()
            .and_then(|project| project.editor_history)
            .expect("updated history checkpoint");
        assert_eq!(updated_history.history_entry_limit(), 9);
        assert_eq!(
            serde_json::to_value(&updated_history).unwrap()["undo_stack"]
                .as_array()
                .unwrap()
                .len(),
            9
        );

        assert_eq!(
            run_recovery_autosave_tick(&app_state, &runtime).unwrap(),
            RecoveryAutosaveOutcome::Duplicate
        );
        assert_eq!(
            io.calls(),
            vec!["store:history digest", "store:history digest"]
        );
    }

    #[test]
    fn exit_clears_settled_work_but_preserves_an_undecided_startup_candidate() {
        let directory = TestDirectory::new("exit-clear");
        let stale = document("stale");
        let app_state = AppState::new(crate::initial_project_state());
        let clear_io = Arc::new(MemoryRecoveryIo::new(Some(stale.clone())));
        let clear_storage = RecoveryStorage::with_io(directory.path(), Arc::clone(&clear_io));
        let clear_runtime = RecoveryRuntime::with_storage(clear_storage, RecoveryStartup::None);
        assert_eq!(
            clear_runtime
                .clear_for_exit(&app_state, ExitRecoveryAuthorization::Clean)
                .unwrap(),
            ExitRecoveryDisposition::Cleared
        );
        assert!(clear_io.document().is_none());

        let preserve_io = Arc::new(MemoryRecoveryIo::new(Some(stale.clone())));
        let preserve_storage = RecoveryStorage::with_io(directory.path(), Arc::clone(&preserve_io));
        let preserve_runtime = RecoveryRuntime::with_storage(
            preserve_storage,
            RecoveryStartup::Available(RecoveryStartupCandidate {
                project: Box::new(project(stale)),
                updated_at_unix_ms: None,
            }),
        );
        assert_eq!(
            preserve_runtime
                .clear_for_exit(&app_state, ExitRecoveryAuthorization::Clean)
                .unwrap(),
            ExitRecoveryDisposition::PreservedStartupCandidate
        );
        assert!(preserve_io.document().is_some());
        assert!(preserve_io.calls().is_empty());
    }

    #[test]
    fn clean_only_exit_preserves_recovery_if_the_project_became_dirty() {
        let directory = TestDirectory::new("exit-clean-race");
        let stale = document("stale");
        let io = Arc::new(MemoryRecoveryIo::new(Some(stale)));
        let storage = RecoveryStorage::with_io(directory.path(), Arc::clone(&io));
        let runtime = RecoveryRuntime::with_storage(storage, RecoveryStartup::None);
        let app_state = AppState::new(ProjectState::new_unsaved(
            "new edit".to_owned(),
            CreasePattern::empty(),
            Paper::default(),
        ));

        assert_eq!(
            runtime
                .clear_for_exit(&app_state, ExitRecoveryAuthorization::Clean)
                .unwrap(),
            ExitRecoveryDisposition::ProjectChanged
        );
        assert!(io.document().is_some());
        assert!(io.calls().is_empty());
        assert!(!runtime.lock_state().unwrap().automatic_writes_stopped);
    }

    #[test]
    fn window_close_prepare_only_arms_an_idempotent_cancelable_token() {
        let directory = TestDirectory::new("window-close-prepare");
        let io = Arc::new(MemoryRecoveryIo::new(Some(document("existing"))));
        let storage = RecoveryStorage::with_io(directory.path(), Arc::clone(&io));
        let runtime = RecoveryRuntime::with_storage(storage, RecoveryStartup::None);
        let app_state = AppState::new(ProjectState::new_unsaved(
            "dirty".to_owned(),
            CreasePattern::empty(),
            Paper::default(),
        ));
        let request = {
            let project = app_state.0.lock().unwrap();
            prepare_close_request(&project, ExitRecoveryAuthorization::DiscardConfirmed)
        };

        assert!(
            runtime
                .prepare_window_close(
                    &app_state,
                    PrepareWindowCloseRequest {
                        authorization: ExitRecoveryAuthorization::Clean,
                        ..request
                    }
                )
                .is_err()
        );
        let first = runtime.prepare_window_close(&app_state, request).unwrap();
        let repeated = runtime.prepare_window_close(&app_state, request).unwrap();
        assert_eq!(first, repeated);
        assert!(io.calls().is_empty());
        assert!(io.document().is_some());
        assert!(!runtime.lock_state().unwrap().automatic_writes_stopped);

        let canceled = runtime
            .cancel_window_close_prepare(CancelWindowClosePrepareRequest {
                schema_version: RECOVERY_SCHEMA_VERSION,
                close_prepare_id: first.close_prepare_id,
                project_instance_id: first.project_instance_id,
                project_id: first.project_id,
                revision: first.revision,
                authorization: first.authorization,
            })
            .unwrap();
        assert_eq!(canceled.status, CancelWindowClosePrepareStatus::Canceled);
        assert_eq!(
            runtime.settle_prepared_window_close(&app_state).unwrap(),
            PreparedWindowCloseSettlement::NotPrepared
        );
        assert_eq!(
            run_recovery_autosave_tick(&app_state, &runtime).unwrap(),
            RecoveryAutosaveOutcome::Stored
        );
        assert_eq!(io.calls(), vec!["store:dirty"]);
    }

    #[test]
    fn prepared_window_close_revalidates_then_clears_exactly_once() {
        let directory = TestDirectory::new("window-close-settle");
        let io = Arc::new(MemoryRecoveryIo::new(Some(document("existing"))));
        let storage = RecoveryStorage::with_io(directory.path(), Arc::clone(&io));
        let runtime = RecoveryRuntime::with_storage(storage, RecoveryStartup::None);
        let app_state = AppState::new(ProjectState::new_unsaved(
            "discard".to_owned(),
            CreasePattern::empty(),
            Paper::default(),
        ));
        let request = {
            let project = app_state.0.lock().unwrap();
            prepare_close_request(&project, ExitRecoveryAuthorization::DiscardConfirmed)
        };
        let _ = runtime.prepare_window_close(&app_state, request).unwrap();

        assert_eq!(
            runtime.settle_prepared_window_close(&app_state).unwrap(),
            PreparedWindowCloseSettlement::Settled
        );
        assert_eq!(
            runtime.settle_prepared_window_close(&app_state).unwrap(),
            PreparedWindowCloseSettlement::NotPrepared
        );
        assert!(io.document().is_none());
        assert_eq!(io.calls(), vec!["clear"]);
        assert!(runtime.lock_state().unwrap().automatic_writes_stopped);
    }

    #[test]
    fn stale_or_expired_window_close_prepare_never_clears_recovery() {
        let directory = TestDirectory::new("window-close-stale");
        let io = Arc::new(MemoryRecoveryIo::new(Some(document("existing"))));
        let storage = RecoveryStorage::with_io(directory.path(), Arc::clone(&io));
        let runtime = RecoveryRuntime::with_storage(storage, RecoveryStartup::None);
        let app_state = AppState::new(ProjectState::new_unsaved(
            "changed".to_owned(),
            CreasePattern::empty(),
            Paper::default(),
        ));
        let request = {
            let project = app_state.0.lock().unwrap();
            prepare_close_request(&project, ExitRecoveryAuthorization::DiscardConfirmed)
        };
        let _ = runtime.prepare_window_close(&app_state, request).unwrap();
        {
            let mut project = app_state.0.lock().unwrap();
            project
                .editor
                .execute(
                    request.revision,
                    Command::AddVertex {
                        id: VertexId::new(),
                        position: Point2::new(1.0, 2.0),
                    },
                )
                .unwrap();
        }
        assert_eq!(
            runtime.settle_prepared_window_close(&app_state).unwrap(),
            PreparedWindowCloseSettlement::Rejected
        );
        assert!(io.calls().is_empty());
        assert!(io.document().is_some());

        let current_request = {
            let project = app_state.0.lock().unwrap();
            prepare_close_request(&project, ExitRecoveryAuthorization::DiscardConfirmed)
        };
        let _ = runtime
            .prepare_window_close(&app_state, current_request)
            .unwrap();
        runtime
            .lock_state()
            .unwrap()
            .prepared_window_close
            .as_mut()
            .unwrap()
            .expires_at = Instant::now();
        assert_eq!(
            runtime.settle_prepared_window_close(&app_state).unwrap(),
            PreparedWindowCloseSettlement::Rejected
        );
        assert!(io.calls().is_empty());
        assert!(!runtime.lock_state().unwrap().automatic_writes_stopped);
    }

    #[test]
    fn failed_prepared_window_close_clear_consumes_token_and_keeps_autosave_running() {
        let directory = TestDirectory::new("window-close-clear-failure");
        let io = Arc::new(MemoryRecoveryIo::new(Some(document("existing"))));
        io.fail_next_clear();
        let storage = RecoveryStorage::with_io(directory.path(), Arc::clone(&io));
        let runtime = RecoveryRuntime::with_storage(storage, RecoveryStartup::None);
        let app_state = AppState::new(ProjectState::new_unsaved(
            "continue".to_owned(),
            CreasePattern::empty(),
            Paper::default(),
        ));
        let request = {
            let project = app_state.0.lock().unwrap();
            prepare_close_request(&project, ExitRecoveryAuthorization::DiscardConfirmed)
        };
        let _ = runtime.prepare_window_close(&app_state, request).unwrap();

        assert!(runtime.settle_prepared_window_close(&app_state).is_err());
        assert!(!runtime.lock_state().unwrap().automatic_writes_stopped);
        assert_eq!(
            runtime.settle_prepared_window_close(&app_state).unwrap(),
            PreparedWindowCloseSettlement::NotPrepared
        );
        assert_eq!(
            run_recovery_autosave_tick(&app_state, &runtime).unwrap(),
            RecoveryAutosaveOutcome::Stored
        );
        assert_eq!(io.calls(), vec!["clear", "store:continue"]);
    }

    #[test]
    fn identical_dirty_binding_is_written_only_once() {
        let directory = TestDirectory::new("autosave-deduplicate");
        let io = Arc::new(MemoryRecoveryIo::new(None));
        let storage = RecoveryStorage::with_io(directory.path(), Arc::clone(&io));
        let runtime = RecoveryRuntime::with_storage(storage, RecoveryStartup::None);
        let app_state = AppState::new(ProjectState::new_unsaved(
            "dirty".to_owned(),
            CreasePattern::empty(),
            Paper::default(),
        ));
        assert_eq!(
            run_recovery_autosave_tick(&app_state, &runtime).unwrap(),
            RecoveryAutosaveOutcome::Stored
        );
        assert_eq!(
            run_recovery_autosave_tick(&app_state, &runtime).unwrap(),
            RecoveryAutosaveOutcome::Duplicate
        );
        assert_eq!(io.calls(), vec!["store:dirty"]);
    }

    #[test]
    fn autosave_health_reports_redacted_failure_once_and_recovers_after_durable_retry() {
        let directory = TestDirectory::new("autosave-health");
        let io = Arc::new(MemoryRecoveryIo::new(None));
        let storage = RecoveryStorage::with_io(directory.path(), Arc::clone(&io));
        let runtime = RecoveryRuntime::with_storage(storage, RecoveryStartup::None);
        let app_state = AppState::new(crate::initial_project_state());

        assert_eq!(
            runtime.autosave_health_response().unwrap(),
            GetRecoveryAutosaveStatusResponse {
                schema_version: RECOVERY_SCHEMA_VERSION,
                status: RecoveryAutosaveHealthStatus::PendingFirstAttempt,
                transition_id: 0,
            }
        );

        io.fail_next_clear();
        assert!(run_recovery_autosave_tick(&app_state, &runtime).is_err());
        let failed = runtime.autosave_health_response().unwrap();
        assert_eq!(
            failed,
            GetRecoveryAutosaveStatusResponse {
                schema_version: RECOVERY_SCHEMA_VERSION,
                status: RecoveryAutosaveHealthStatus::PersistenceFailed,
                transition_id: 1,
            }
        );
        let failed_wire = serde_json::to_value(failed).unwrap();
        assert_eq!(
            failed_wire,
            json!({
                "schema_version": 1,
                "status": "persistence_failed",
                "transition_id": 1
            })
        );
        for forbidden in ["path", "error", "generation", "project_id", "revision"] {
            assert!(
                failed_wire.get(forbidden).is_none(),
                "health response leaked {forbidden}"
            );
        }

        io.fail_next_clear();
        assert!(run_recovery_autosave_tick(&app_state, &runtime).is_err());
        assert_eq!(
            runtime.autosave_health_response().unwrap(),
            failed,
            "a repeated failure must not create another UI announcement"
        );

        assert_eq!(
            run_recovery_autosave_tick(&app_state, &runtime).unwrap(),
            RecoveryAutosaveOutcome::Cleared
        );
        assert_eq!(
            runtime.autosave_health_response().unwrap(),
            GetRecoveryAutosaveStatusResponse {
                schema_version: RECOVERY_SCHEMA_VERSION,
                status: RecoveryAutosaveHealthStatus::Operational,
                transition_id: 2,
            }
        );
        assert_eq!(
            run_recovery_autosave_tick(&app_state, &runtime).unwrap(),
            RecoveryAutosaveOutcome::Duplicate
        );
        assert_eq!(runtime.autosave_health_response().unwrap().transition_id, 2);
    }

    #[test]
    fn autosave_health_ignores_non_attempts_and_latches_failed_without_id_wrap() {
        let directory = TestDirectory::new("autosave-health-non-attempt");
        let recovered = document("pending");
        let io = Arc::new(MemoryRecoveryIo::new(Some(recovered.clone())));
        let storage = RecoveryStorage::with_io(directory.path(), io);
        let runtime = RecoveryRuntime::with_storage(
            storage,
            RecoveryStartup::Available(RecoveryStartupCandidate {
                project: Box::new(project(recovered)),
                updated_at_unix_ms: None,
            }),
        );
        let app_state = AppState::new(ProjectState::new_unsaved(
            "blocked".to_owned(),
            CreasePattern::empty(),
            Paper::default(),
        ));
        assert_eq!(
            run_recovery_autosave_tick(&app_state, &runtime).unwrap(),
            RecoveryAutosaveOutcome::StartupDecisionPending
        );
        assert_eq!(
            runtime.autosave_health_response().unwrap().status,
            RecoveryAutosaveHealthStatus::PendingFirstAttempt
        );

        let mut state = runtime.lock_state().unwrap();
        state.autosave_health_status = RecoveryAutosaveHealthStatus::PendingFirstAttempt;
        state.autosave_health_transition_id = u32::MAX - 1;
        state.record_autosave_health(RecoveryAutosaveHealthStatus::Operational);
        assert_eq!(
            state.autosave_health_status,
            RecoveryAutosaveHealthStatus::PersistenceFailed
        );
        assert_eq!(state.autosave_health_transition_id, u32::MAX);
        state.record_autosave_health(RecoveryAutosaveHealthStatus::Operational);
        assert_eq!(
            state.autosave_health_status,
            RecoveryAutosaveHealthStatus::PersistenceFailed
        );
        assert_eq!(state.autosave_health_transition_id, u32::MAX);
    }

    struct BlockingMemoryIo {
        document: Mutex<Option<Ori2ProjectArchive>>,
        calls: Mutex<Vec<String>>,
        block_first_store: AtomicBool,
        first_store_started: (Mutex<bool>, Condvar),
        release_first_store: (Mutex<bool>, Condvar),
        active_calls: AtomicUsize,
        maximum_active_calls: AtomicUsize,
    }

    impl BlockingMemoryIo {
        fn new() -> Self {
            Self {
                document: Mutex::new(None),
                calls: Mutex::new(Vec::new()),
                block_first_store: AtomicBool::new(true),
                first_store_started: (Mutex::new(false), Condvar::new()),
                release_first_store: (Mutex::new(false), Condvar::new()),
                active_calls: AtomicUsize::new(0),
                maximum_active_calls: AtomicUsize::new(0),
            }
        }

        fn enter(&self) {
            let active = self.active_calls.fetch_add(1, Ordering::SeqCst) + 1;
            self.maximum_active_calls
                .fetch_max(active, Ordering::SeqCst);
        }

        fn leave(&self) {
            self.active_calls.fetch_sub(1, Ordering::SeqCst);
        }

        fn wait_until_first_store_started(&self) {
            let (lock, ready) = &self.first_store_started;
            let mut started = lock.lock().unwrap();
            while !*started {
                started = ready.wait(started).unwrap();
            }
        }

        fn release_first_store(&self) {
            let (lock, ready) = &self.release_first_store;
            *lock.lock().unwrap() = true;
            ready.notify_all();
        }

        fn maybe_block_first_store(&self) {
            if !self.block_first_store.swap(false, Ordering::SeqCst) {
                return;
            }
            let (started_lock, started_ready) = &self.first_store_started;
            *started_lock.lock().unwrap() = true;
            started_ready.notify_all();

            let (release_lock, release_ready) = &self.release_first_store;
            let mut released = release_lock.lock().unwrap();
            while !*released {
                released = release_ready.wait(released).unwrap();
            }
        }

        fn calls(&self) -> Vec<String> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl RecoveryIo for BlockingMemoryIo {
        fn inspect(&self, _slot_path: &Path) -> RecoveryStartup {
            match self.document.lock().unwrap().clone() {
                Some(project) => RecoveryStartup::Available(RecoveryStartupCandidate {
                    project: Box::new(project),
                    updated_at_unix_ms: None,
                }),
                None => RecoveryStartup::None,
            }
        }

        fn store(
            &self,
            _slot_path: &Path,
            project: &Ori2ProjectArchive,
        ) -> Result<(), RecoveryPersistenceError> {
            self.enter();
            self.maybe_block_first_store();
            self.calls
                .lock()
                .unwrap()
                .push(format!("store:{}", project.document.name));
            *self.document.lock().unwrap() = Some(project.clone());
            self.leave();
            Ok(())
        }

        fn clear(&self, _slot_path: &Path) -> Result<(), RecoveryPersistenceError> {
            self.enter();
            self.calls.lock().unwrap().push("clear".to_owned());
            *self.document.lock().unwrap() = None;
            self.leave();
            Ok(())
        }
    }

    struct BlockingClearIo {
        release: (Mutex<bool>, Condvar),
    }

    impl BlockingClearIo {
        fn new() -> Self {
            Self {
                release: (Mutex::new(false), Condvar::new()),
            }
        }

        fn release(&self) {
            let (lock, ready) = &self.release;
            *lock.lock().unwrap() = true;
            ready.notify_all();
        }
    }

    impl RecoveryIo for BlockingClearIo {
        fn inspect(&self, _slot_path: &Path) -> RecoveryStartup {
            RecoveryStartup::None
        }

        fn store(
            &self,
            _slot_path: &Path,
            _project: &Ori2ProjectArchive,
        ) -> Result<(), RecoveryPersistenceError> {
            Ok(())
        }

        fn clear(&self, _slot_path: &Path) -> Result<(), RecoveryPersistenceError> {
            let (lock, ready) = &self.release;
            let mut released = lock.lock().unwrap();
            while !*released {
                released = ready.wait(released).unwrap();
            }
            Ok(())
        }
    }

    #[test]
    fn first_writer_io_is_included_in_the_finite_settlement_timeout() {
        let directory = TestDirectory::new("finite-first-writer-timeout");
        let io = Arc::new(BlockingClearIo::new());
        let storage = RecoveryStorage::with_io(directory.path(), Arc::clone(&io));
        let started = Instant::now();

        assert_eq!(
            storage
                .clear_and_wait(Duration::from_millis(20))
                .unwrap_err(),
            RecoveryStorageError::SettlementTimedOut
        );
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "the timeout must bound the blocking clear itself"
        );

        io.release();
        assert_eq!(
            storage
                .wait_for_settlement(RecoveryGeneration(1), Duration::from_secs(1))
                .unwrap(),
            RecoverySettlement::Durable
        );
    }

    #[test]
    fn newer_store_is_coalesced_and_fenced_behind_the_only_writer() {
        let directory = TestDirectory::new("coalesce-store");
        let io = Arc::new(BlockingMemoryIo::new());
        let storage = RecoveryStorage::with_io(directory.path(), Arc::clone(&io));
        let worker_storage = storage.clone();
        let worker = thread::spawn(move || worker_storage.store(document("old")));
        io.wait_until_first_store_started();

        let queued = storage.store(document("new")).unwrap();
        assert_eq!(queued.generation.get(), 2);
        assert_eq!(queued.disposition, RecoverySubmissionDisposition::Coalesced);
        io.release_first_store();
        worker.join().unwrap().unwrap();
        assert_eq!(
            storage
                .wait_for_settlement(queued.generation, Duration::from_secs(2))
                .unwrap(),
            RecoverySettlement::Durable
        );

        assert_eq!(available_document(&storage).name, "new");
        assert_eq!(io.calls(), vec!["store:old", "store:new"]);
        assert_eq!(io.maximum_active_calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            storage.status().unwrap(),
            RecoveryWriterStatus {
                latest_generation: Some(RecoveryGeneration(2)),
                durable_generation: Some(RecoveryGeneration(2)),
                failed_generation: None,
                writer_active: false,
                has_pending_operation: false,
            }
        );
    }

    #[test]
    fn clear_supersedes_a_pending_store_after_an_old_writer() {
        let directory = TestDirectory::new("coalesce-clear");
        let io = Arc::new(BlockingMemoryIo::new());
        let storage = RecoveryStorage::with_io(directory.path(), Arc::clone(&io));
        let worker_storage = storage.clone();
        let worker = thread::spawn(move || worker_storage.store(document("old")));
        io.wait_until_first_store_started();

        let new = storage.store(document("never-published")).unwrap();
        assert_eq!(new.generation.get(), 2);
        let clear = storage.clear().unwrap();
        assert_eq!(clear.generation.get(), 3);
        assert_eq!(clear.disposition, RecoverySubmissionDisposition::Coalesced);
        assert_eq!(
            storage
                .wait_for_settlement(clear.generation, Duration::ZERO)
                .unwrap_err(),
            RecoveryStorageError::SettlementTimedOut
        );
        let waiting_storage = storage.clone();
        let waiting_generation = clear.generation;
        let settlement_waiter = thread::spawn(move || {
            waiting_storage.wait_for_settlement(waiting_generation, Duration::from_secs(2))
        });
        io.release_first_store();
        worker.join().unwrap().unwrap();
        assert_eq!(
            settlement_waiter.join().unwrap().unwrap(),
            RecoverySettlement::Durable
        );

        assert!(matches!(storage.inspect_startup(), RecoveryStartup::None));
        assert_eq!(io.calls(), vec!["store:old", "clear"]);
        assert_eq!(io.maximum_active_calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            storage
                .wait_for_settlement(new.generation, Duration::ZERO)
                .unwrap(),
            RecoverySettlement::Superseded
        );
        assert_eq!(
            storage
                .wait_for_settlement(clear.generation, Duration::ZERO)
                .unwrap(),
            RecoverySettlement::Durable
        );
        let status = storage.status().unwrap();
        assert_eq!(status.latest_generation, Some(RecoveryGeneration(3)));
        assert_eq!(status.durable_generation, Some(RecoveryGeneration(3)));
    }

    #[test]
    fn normal_clear_serializes_behind_an_in_flight_autosave_without_lock_inversion() {
        let directory = TestDirectory::new("runtime-autosave-clear-race");
        let io = Arc::new(BlockingMemoryIo::new());
        let storage = RecoveryStorage::with_io(directory.path(), Arc::clone(&io));
        let runtime = RecoveryRuntime::with_storage(storage, RecoveryStartup::None);
        let app_state = Arc::new(AppState::new(ProjectState::new_unsaved(
            "racing".to_owned(),
            CreasePattern::empty(),
            Paper::default(),
        )));
        let current = {
            let project = app_state.0.lock().unwrap();
            snapshot(&project)
        };

        let autosave_state = Arc::clone(&app_state);
        let autosave_runtime = runtime.clone();
        let autosave =
            thread::spawn(move || run_recovery_autosave_tick(&autosave_state, &autosave_runtime));
        io.wait_until_first_store_started();

        let clear_runtime = runtime.clone();
        let clear_started = Arc::new(AtomicBool::new(false));
        let clear_started_in_thread = Arc::clone(&clear_started);
        let clear_state = Arc::clone(&app_state);
        let clear = thread::spawn(move || {
            clear_started_in_thread.store(true, Ordering::SeqCst);
            clear_runtime.clear_after_normal_completion(&clear_state, &current)
        });
        while !clear_started.load(Ordering::SeqCst) {
            thread::yield_now();
        }
        assert_eq!(
            runtime.0.storage.status().unwrap().latest_generation,
            Some(RecoveryGeneration(1)),
            "the recovery operation gate must not enqueue clear ahead of the active store"
        );
        io.release_first_store();

        assert_eq!(
            autosave.join().unwrap().unwrap(),
            RecoveryAutosaveOutcome::Stored
        );
        clear.join().unwrap().unwrap();
        assert!(matches!(
            runtime.0.storage.inspect_startup(),
            RecoveryStartup::None
        ));
        assert_eq!(io.calls(), vec!["store:racing", "clear"]);
        assert_eq!(io.maximum_active_calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn normal_clear_cannot_overtake_an_autosave_paused_before_submission() {
        let directory = TestDirectory::new("runtime-presubmit-clear-race");
        let io = Arc::new(MemoryRecoveryIo::new(None));
        let storage = RecoveryStorage::with_io(directory.path(), Arc::clone(&io));
        let runtime = RecoveryRuntime::with_storage(storage, RecoveryStartup::None);
        let app_state = AppState::new(ProjectState::new_unsaved(
            "pre-submit".to_owned(),
            CreasePattern::empty(),
            Paper::default(),
        ));

        // This is the timer's exact ordering boundary: it owns project then
        // recovery, detaches the dirty document, and releases project before
        // any filesystem operation.
        let mut project = app_state.0.lock().unwrap();
        let operation = runtime.lock_operation_gate().unwrap();
        let capture =
            RecoveryRuntime::capture_autosave(&project).expect("capture pre-submit recovery");
        let saved_document = project.document();
        project.saved_revision = Some(project.editor.revision());
        project.saved_document = Some(saved_document);
        let saved_snapshot = snapshot(&project);
        assert!(!saved_snapshot.is_dirty);
        drop(project);

        let clear_runtime = runtime.clone();
        let clear_started = Arc::new(AtomicBool::new(false));
        let clear_started_in_thread = Arc::clone(&clear_started);
        let clear_state = Arc::new(app_state);
        let clear_state_in_thread = Arc::clone(&clear_state);
        let clear = thread::spawn(move || {
            clear_started_in_thread.store(true, Ordering::SeqCst);
            clear_runtime.clear_after_normal_completion(&clear_state_in_thread, &saved_snapshot)
        });
        while !clear_started.load(Ordering::SeqCst) {
            thread::yield_now();
        }
        assert!(io.calls().is_empty());

        assert_eq!(
            runtime.autosave(capture).unwrap(),
            RecoveryAutosaveOutcome::Stored
        );
        drop(operation);
        clear.join().unwrap().unwrap();

        assert!(io.document().is_none());
        assert_eq!(io.calls(), vec!["store:pre-submit", "clear"]);
    }

    #[test]
    fn a_successful_exit_clear_stops_late_autosave_ticks() {
        let directory = TestDirectory::new("exit-stops-autosave");
        let io = Arc::new(MemoryRecoveryIo::new(Some(document("stale"))));
        let storage = RecoveryStorage::with_io(directory.path(), Arc::clone(&io));
        let runtime = RecoveryRuntime::with_storage(storage, RecoveryStartup::None);
        let app_state = AppState::new(ProjectState::new_unsaved(
            "late".to_owned(),
            CreasePattern::empty(),
            Paper::default(),
        ));

        assert_eq!(
            runtime
                .clear_for_exit(&app_state, ExitRecoveryAuthorization::DiscardConfirmed)
                .unwrap(),
            ExitRecoveryDisposition::Cleared
        );
        assert_eq!(
            run_recovery_autosave_tick(&app_state, &runtime).unwrap(),
            RecoveryAutosaveOutcome::AutomaticWritesStopped
        );
        assert!(io.document().is_none());
        assert_eq!(io.calls(), vec!["clear"]);
    }

    #[test]
    fn a_failed_exit_clear_reenables_autosave_when_the_app_stays_open() {
        let directory = TestDirectory::new("exit-clear-failure");
        let io = Arc::new(MemoryRecoveryIo::new(Some(document("stale"))));
        io.fail_next_clear();
        let storage = RecoveryStorage::with_io(directory.path(), Arc::clone(&io));
        let runtime = RecoveryRuntime::with_storage(storage, RecoveryStartup::None);
        let app_state = AppState::new(ProjectState::new_unsaved(
            "continued".to_owned(),
            CreasePattern::empty(),
            Paper::default(),
        ));

        assert!(
            runtime
                .clear_for_exit(&app_state, ExitRecoveryAuthorization::DiscardConfirmed)
                .is_err()
        );
        assert_eq!(
            run_recovery_autosave_tick(&app_state, &runtime).unwrap(),
            RecoveryAutosaveOutcome::Stored
        );
        assert_eq!(io.document().unwrap().name, "continued");
        assert_eq!(io.calls(), vec!["clear", "store:continued"]);
    }

    #[test]
    fn raw_io_errors_and_private_paths_are_redacted_and_retryable() {
        let directory = TestDirectory::new("redaction");
        let secret_root = directory.path().join("private-secret-root");
        fs::write(&secret_root, b"this is a file, not a directory").unwrap();
        let storage = RecoveryStorage::new(&secret_root);

        let submission = storage.store(document("cannot persist")).unwrap();
        assert_eq!(
            storage
                .wait_for_settlement(submission.generation, Duration::from_secs(1))
                .unwrap(),
            RecoverySettlement::Failed
        );
        let error = RecoveryStorageError::StorageUnavailable;
        assert_eq!(error.to_string(), RECOVERY_STORAGE_FAILED_MESSAGE);
        assert!(!error.to_string().contains("private-secret-root"));
        assert!(!format!("{storage:?}").contains("private-secret-root"));
        let status = storage.status().unwrap();
        assert_eq!(status.latest_generation, Some(RecoveryGeneration(1)));
        assert_eq!(status.failed_generation, Some(RecoveryGeneration(1)));
        assert!(!status.writer_active);
        assert_eq!(
            storage
                .wait_for_settlement(RecoveryGeneration(1), Duration::ZERO)
                .unwrap(),
            RecoverySettlement::Failed
        );
        assert!(storage.retry_failed().unwrap());
        assert_eq!(
            storage
                .wait_for_settlement(RecoveryGeneration(1), Duration::from_secs(1))
                .unwrap(),
            RecoverySettlement::Failed
        );
    }
}
