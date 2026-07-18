use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU8, AtomicU64, Ordering},
    },
    time::{Duration, SystemTime},
};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::DialogExt;

#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
#[cfg(target_os = "windows")]
use std::os::windows::fs::OpenOptionsExt;
#[cfg(target_os = "windows")]
use windows_sys::Win32::Storage::FileSystem::{
    DELETE, FILE_GENERIC_READ, FILE_GENERIC_WRITE, FILE_SHARE_READ,
};

const DIAGNOSTICS_FILE_NAME: &str = "redacted-diagnostics-v1.json";
#[cfg(test)]
const DIAGNOSTICS_SCHEMA: &str = "origami2.redacted-diagnostics.v1";
const MAX_DIAGNOSTICS_BYTES: usize = 8 * 1024;
const MAX_COUNT: u8 = 65;
const GENERIC_DIAGNOSTICS_ERROR: &str = "diagnostics unavailable";
const DIAGNOSTICS_SHARE_FILE_NAME: &str = "ORIGAMI2-diagnostics.json";
const STAGED_FILE_PREFIX: &str = ".redacted-diagnostics-v1-";
const STAGED_FILE_SUFFIX: &str = ".tmp";
const MAX_STALE_STAGE_SCAN_ENTRIES: usize = 512;
const MAX_STALE_STAGE_REMOVALS: usize = 32;
const STALE_STAGE_MINIMUM_AGE: Duration = Duration::from_secs(24 * 60 * 60);
static NEXT_STAGED_FILE_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum DiagnosticScope {
    #[serde(rename = "app.unhandled_error")]
    AppUnhandledError,
    #[serde(rename = "app.unhandled_rejection")]
    AppUnhandledRejection,
    #[serde(rename = "app.project_snapshot")]
    AppProjectSnapshot,
    #[serde(rename = "app.topology_analysis")]
    AppTopologyAnalysis,
    #[serde(rename = "app.close_guard")]
    AppCloseGuard,
    #[serde(rename = "app.validation")]
    AppValidation,
    #[serde(rename = "app.benchmark")]
    AppBenchmark,
    #[serde(rename = "fold_preview.geometry")]
    FoldPreviewGeometry,
    #[serde(rename = "fold_preview.render")]
    FoldPreviewRender,
    #[serde(rename = "fold_preview.scene_initialization")]
    FoldPreviewSceneInitialization,
    #[serde(rename = "fold_preview.pose_application")]
    FoldPreviewPoseApplication,
    #[serde(rename = "fold_preview.pose_schedule")]
    FoldPreviewPoseSchedule,
    #[serde(rename = "fold_preview.selection_render")]
    FoldPreviewSelectionRender,
    #[serde(rename = "fold_preview.camera")]
    FoldPreviewCamera,
    #[serde(rename = "fold_preview.resize")]
    FoldPreviewResize,
}

impl DiagnosticScope {
    const ALL: [Self; 15] = [
        Self::AppUnhandledError,
        Self::AppUnhandledRejection,
        Self::AppProjectSnapshot,
        Self::AppTopologyAnalysis,
        Self::AppCloseGuard,
        Self::AppValidation,
        Self::AppBenchmark,
        Self::FoldPreviewGeometry,
        Self::FoldPreviewRender,
        Self::FoldPreviewSceneInitialization,
        Self::FoldPreviewPoseApplication,
        Self::FoldPreviewPoseSchedule,
        Self::FoldPreviewSelectionRender,
        Self::FoldPreviewCamera,
        Self::FoldPreviewResize,
    ];

    const fn index(self) -> usize {
        match self {
            Self::AppUnhandledError => 0,
            Self::AppUnhandledRejection => 1,
            Self::AppProjectSnapshot => 2,
            Self::AppTopologyAnalysis => 3,
            Self::AppCloseGuard => 4,
            Self::AppValidation => 5,
            Self::AppBenchmark => 6,
            Self::FoldPreviewGeometry => 7,
            Self::FoldPreviewRender => 8,
            Self::FoldPreviewSceneInitialization => 9,
            Self::FoldPreviewPoseApplication => 10,
            Self::FoldPreviewPoseSchedule => 11,
            Self::FoldPreviewSelectionRender => 12,
            Self::FoldPreviewCamera => 13,
            Self::FoldPreviewResize => 14,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum DiagnosticCountBucket {
    #[serde(rename = "0")]
    Zero,
    #[serde(rename = "1")]
    One,
    #[serde(rename = "2_4")]
    TwoToFour,
    #[serde(rename = "5_16")]
    FiveToSixteen,
    #[serde(rename = "17_64")]
    SeventeenToSixtyFour,
    #[serde(rename = "65_plus")]
    SixtyFivePlus,
}

impl DiagnosticCountBucket {
    const fn from_count(count: u8) -> Self {
        match count {
            0 => Self::Zero,
            1 => Self::One,
            2..=4 => Self::TwoToFour,
            5..=16 => Self::FiveToSixteen,
            17..=64 => Self::SeventeenToSixtyFour,
            _ => Self::SixtyFivePlus,
        }
    }

    const fn lower_bound(self) -> u8 {
        match self {
            Self::Zero => 0,
            Self::One => 1,
            Self::TwoToFour => 2,
            Self::FiveToSixteen => 5,
            Self::SeventeenToSixtyFour => 17,
            Self::SixtyFivePlus => 65,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum DiagnosticsSchema {
    #[serde(rename = "origami2.redacted-diagnostics.v1")]
    V1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredDiagnosticCount {
    scope: DiagnosticScope,
    count: DiagnosticCountBucket,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredDiagnostics {
    schema: DiagnosticsSchema,
    unexpected: Vec<StoredDiagnosticCount>,
}

impl StoredDiagnostics {
    fn from_counts(counts: &[u8; DiagnosticScope::ALL.len()]) -> Self {
        Self {
            schema: DiagnosticsSchema::V1,
            unexpected: DiagnosticScope::ALL
                .into_iter()
                .enumerate()
                .map(|(index, scope)| StoredDiagnosticCount {
                    scope,
                    count: DiagnosticCountBucket::from_count(counts[index]),
                })
                .collect(),
        }
    }

    fn validated_counts(&self) -> Result<[u8; DiagnosticScope::ALL.len()], DiagnosticsError> {
        if self.unexpected.len() != DiagnosticScope::ALL.len() {
            return Err(DiagnosticsError);
        }

        let mut counts = [0; DiagnosticScope::ALL.len()];
        for (index, entry) in self.unexpected.iter().enumerate() {
            if entry.scope != DiagnosticScope::ALL[index] {
                return Err(DiagnosticsError);
            }
            counts[index] = entry.count.lower_bound();
        }
        Ok(counts)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DiagnosticsError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct DiagnosticsSharePreviewResponse {
    preview_generation: u32,
    json: String,
    byte_length: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(crate) struct DiagnosticsShareSaveResponse {
    preview_generation: u32,
    byte_length: u32,
    canceled: bool,
}

#[derive(Clone)]
struct CachedDiagnosticsSharePreview {
    preview_generation: u32,
    bytes: Arc<[u8]>,
}

#[derive(Default)]
struct DiagnosticsSharePreviewCache {
    last_generation: u32,
    current: Option<CachedDiagnosticsSharePreview>,
}

pub(crate) struct DiagnosticsState {
    store: Mutex<DiagnosticsStore>,
    native_deliveries: [AtomicU8; DiagnosticScope::ALL.len()],
    persistence_gate: Arc<tauri::async_runtime::Mutex<()>>,
    share_preview_cache: Mutex<DiagnosticsSharePreviewCache>,
    share_save_gate: Arc<tauri::async_runtime::Mutex<()>>,
}

struct DiagnosticsStore {
    destination: Option<PathBuf>,
    counts: [u8; DiagnosticScope::ALL.len()],
    persistence_disabled: bool,
}

impl DiagnosticsState {
    pub(crate) fn from_app_handle(app_handle: &AppHandle) -> Self {
        let destination = app_handle
            .path()
            .app_log_dir()
            .ok()
            .map(|directory| directory.join(DIAGNOSTICS_FILE_NAME));
        Self::from_destination(destination)
    }

    fn from_destination(destination: Option<PathBuf>) -> Self {
        if let Some(parent) = destination.as_deref().and_then(Path::parent) {
            cleanup_stale_staged_files(parent, STALE_STAGE_MINIMUM_AGE);
        }
        let counts = destination
            .as_deref()
            .and_then(|path| load_counts(path).ok())
            .unwrap_or([0; DiagnosticScope::ALL.len()]);
        Self {
            store: Mutex::new(DiagnosticsStore {
                destination,
                counts,
                persistence_disabled: false,
            }),
            native_deliveries: std::array::from_fn(|_| AtomicU8::new(0)),
            persistence_gate: Arc::new(tauri::async_runtime::Mutex::new(())),
            share_preview_cache: Mutex::new(DiagnosticsSharePreviewCache::default()),
            share_save_gate: Arc::new(tauri::async_runtime::Mutex::new(())),
        }
    }

    fn try_reserve_delivery(&self, scope: DiagnosticScope) -> bool {
        self.native_deliveries[scope.index()]
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                (current < MAX_COUNT).then(|| current + 1)
            })
            .is_ok()
    }

    fn record(&self, scope: DiagnosticScope) -> Result<(), DiagnosticsError> {
        self.store
            .lock()
            .map_err(|_| DiagnosticsError)?
            .record(scope)
    }

    fn prepare_share_preview(&self) -> Result<DiagnosticsSharePreviewResponse, DiagnosticsError> {
        let counts = self.store.lock().map_err(|_| DiagnosticsError)?.counts;
        let bytes = serialize_canonical_diagnostics(&StoredDiagnostics::from_counts(&counts))?;
        let byte_length = u32::try_from(bytes.len()).map_err(|_| DiagnosticsError)?;
        let json = String::from_utf8(bytes.clone()).map_err(|_| DiagnosticsError)?;

        let mut cache = self
            .share_preview_cache
            .lock()
            .map_err(|_| DiagnosticsError)?;
        let preview_generation = cache
            .last_generation
            .checked_add(1)
            .ok_or(DiagnosticsError)?;
        cache.last_generation = preview_generation;
        cache.current = Some(CachedDiagnosticsSharePreview {
            preview_generation,
            bytes: Arc::from(bytes),
        });

        Ok(DiagnosticsSharePreviewResponse {
            preview_generation,
            json,
            byte_length,
        })
    }

    fn cached_share_preview(
        &self,
        preview_generation: u32,
    ) -> Result<CachedDiagnosticsSharePreview, DiagnosticsError> {
        let cache = self
            .share_preview_cache
            .lock()
            .map_err(|_| DiagnosticsError)?;
        let cached = cache.current.as_ref().ok_or(DiagnosticsError)?;
        if cached.preview_generation != preview_generation {
            return Err(DiagnosticsError);
        }
        validate_canonical_diagnostics(&cached.bytes)?;
        Ok(cached.clone())
    }
}

impl DiagnosticsStore {
    fn record(&mut self, scope: DiagnosticScope) -> Result<(), DiagnosticsError> {
        if self.persistence_disabled {
            return Err(DiagnosticsError);
        }
        let index = scope.index();
        let current = self.counts[index];
        let next = current.saturating_add(1).min(MAX_COUNT);
        let current_bucket = DiagnosticCountBucket::from_count(current);
        let next_bucket = DiagnosticCountBucket::from_count(next);

        if current_bucket == next_bucket {
            self.counts[index] = next;
            return Ok(());
        }

        let Some(destination) = self.destination.as_deref() else {
            self.persistence_disabled = true;
            return Err(DiagnosticsError);
        };
        let mut next_counts = self.counts;
        next_counts[index] = next;
        let snapshot = StoredDiagnostics::from_counts(&next_counts);
        if persist_snapshot(destination, &snapshot).is_err() {
            self.persistence_disabled = true;
            return Err(DiagnosticsError);
        }
        self.counts = next_counts;
        Ok(())
    }
}

#[tauri::command]
pub(crate) async fn record_unexpected_diagnostic(
    scope: DiagnosticScope,
    app_handle: AppHandle,
) -> Result<(), &'static str> {
    if !app_handle
        .state::<DiagnosticsState>()
        .try_reserve_delivery(scope)
    {
        return Ok(());
    }
    let blocking_app_handle = app_handle.clone();
    let persistence_gate = app_handle
        .state::<DiagnosticsState>()
        .persistence_gate
        .clone();
    let persistence_guard = persistence_gate.lock_owned().await;
    tauri::async_runtime::spawn_blocking(move || {
        let _persistence_guard = persistence_guard;
        blocking_app_handle
            .state::<DiagnosticsState>()
            .record(scope)
    })
    .await
    .map_err(|_| GENERIC_DIAGNOSTICS_ERROR)?
    .map_err(|_| GENERIC_DIAGNOSTICS_ERROR)
}

#[tauri::command]
pub(crate) async fn prepare_diagnostics_share_preview(
    app_handle: AppHandle,
) -> Result<DiagnosticsSharePreviewResponse, &'static str> {
    let blocking_app_handle = app_handle.clone();
    let persistence_gate = app_handle
        .state::<DiagnosticsState>()
        .persistence_gate
        .clone();
    let persistence_guard = persistence_gate.lock_owned().await;
    tauri::async_runtime::spawn_blocking(move || {
        let _persistence_guard = persistence_guard;
        blocking_app_handle
            .state::<DiagnosticsState>()
            .prepare_share_preview()
    })
    .await
    .map_err(|_| GENERIC_DIAGNOSTICS_ERROR)?
    .map_err(|_| GENERIC_DIAGNOSTICS_ERROR)
}

#[tauri::command]
pub(crate) async fn save_diagnostics_share_preview(
    preview_generation: u32,
    app_handle: AppHandle,
) -> Result<DiagnosticsShareSaveResponse, &'static str> {
    save_diagnostics_share_preview_inner(preview_generation, app_handle)
        .await
        .map_err(|_| GENERIC_DIAGNOSTICS_ERROR)
}

async fn save_diagnostics_share_preview_inner(
    preview_generation: u32,
    app_handle: AppHandle,
) -> Result<DiagnosticsShareSaveResponse, DiagnosticsError> {
    let share_save_gate = app_handle
        .state::<DiagnosticsState>()
        .share_save_gate
        .clone();
    let share_save_guard = share_save_gate.lock_owned().await;
    let cached = app_handle
        .state::<DiagnosticsState>()
        .cached_share_preview(preview_generation)?;

    let Some(destination) = choose_diagnostics_share_destination(&app_handle).await? else {
        return complete_diagnostics_share_save(cached, None, |_, _| Err(DiagnosticsError));
    };

    let persistence_gate = app_handle
        .state::<DiagnosticsState>()
        .persistence_gate
        .clone();
    let persistence_guard = persistence_gate.lock_owned().await;
    tauri::async_runtime::spawn_blocking(move || {
        let _share_save_guard = share_save_guard;
        let _persistence_guard = persistence_guard;
        complete_diagnostics_share_save(cached, Some(destination), persist_canonical_diagnostics)
    })
    .await
    .map_err(|_| DiagnosticsError)?
}

async fn choose_diagnostics_share_destination(
    app_handle: &AppHandle,
) -> Result<Option<PathBuf>, DiagnosticsError> {
    let (sender, mut receiver) =
        tauri::async_runtime::channel::<Result<Option<PathBuf>, DiagnosticsError>>(1);
    let mut dialog = app_handle
        .dialog()
        .file()
        .set_title("Save ORIGAMI2 diagnostics")
        .set_file_name(DIAGNOSTICS_SHARE_FILE_NAME)
        .add_filter("JSON", &["json"]);
    if let Some(window) = app_handle.get_webview_window("main") {
        dialog = dialog.set_parent(&window);
    }
    dialog.save_file(move |selected| {
        let selected = selected
            .map(|path| path.simplified().into_path().map_err(|_| DiagnosticsError))
            .transpose();
        let _ = sender.try_send(selected);
    });
    receiver.recv().await.ok_or(DiagnosticsError)?
}

fn complete_diagnostics_share_save<F>(
    cached: CachedDiagnosticsSharePreview,
    destination: Option<PathBuf>,
    persist: F,
) -> Result<DiagnosticsShareSaveResponse, DiagnosticsError>
where
    F: FnOnce(&Path, &[u8]) -> Result<(), DiagnosticsError>,
{
    validate_canonical_diagnostics(&cached.bytes)?;
    let byte_length = u32::try_from(cached.bytes.len()).map_err(|_| DiagnosticsError)?;
    let canceled = match destination {
        Some(destination) => {
            persist(&destination, &cached.bytes)?;
            false
        }
        None => true,
    };
    Ok(DiagnosticsShareSaveResponse {
        preview_generation: cached.preview_generation,
        byte_length,
        canceled,
    })
}

fn load_counts(destination: &Path) -> Result<[u8; DiagnosticScope::ALL.len()], DiagnosticsError> {
    let file = File::open(destination).map_err(|_| DiagnosticsError)?;
    let mut bytes = Vec::with_capacity(MAX_DIAGNOSTICS_BYTES.min(1024));
    file.take((MAX_DIAGNOSTICS_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| DiagnosticsError)?;
    if bytes.len() > MAX_DIAGNOSTICS_BYTES {
        return Err(DiagnosticsError);
    }
    let stored: StoredDiagnostics = serde_json::from_slice(&bytes).map_err(|_| DiagnosticsError)?;
    stored.validated_counts()
}

fn persist_snapshot(
    destination: &Path,
    snapshot: &StoredDiagnostics,
) -> Result<(), DiagnosticsError> {
    let bytes = serialize_canonical_diagnostics(snapshot)?;
    persist_canonical_diagnostics(destination, &bytes)
}

fn serialize_canonical_diagnostics(
    snapshot: &StoredDiagnostics,
) -> Result<Vec<u8>, DiagnosticsError> {
    let bytes = serde_json::to_vec(snapshot).map_err(|_| DiagnosticsError)?;
    validate_canonical_diagnostics(&bytes)?;
    Ok(bytes)
}

fn validate_canonical_diagnostics(bytes: &[u8]) -> Result<(), DiagnosticsError> {
    if bytes.len() > MAX_DIAGNOSTICS_BYTES {
        return Err(DiagnosticsError);
    }
    let stored: StoredDiagnostics = serde_json::from_slice(bytes).map_err(|_| DiagnosticsError)?;
    stored.validated_counts()?;
    if serde_json::to_vec(&stored).map_err(|_| DiagnosticsError)? != bytes {
        return Err(DiagnosticsError);
    }
    Ok(())
}

fn persist_canonical_diagnostics(destination: &Path, bytes: &[u8]) -> Result<(), DiagnosticsError> {
    validate_canonical_diagnostics(bytes)?;
    let parent = destination.parent().ok_or(DiagnosticsError)?;
    fs::create_dir_all(parent).map_err(|_| DiagnosticsError)?;
    let mut staged = StagedDiagnosticsFile::create(parent, destination)?;
    staged
        .file_mut()
        .write_all(bytes)
        .map_err(|_| DiagnosticsError)?;
    staged.file_mut().sync_all().map_err(|_| DiagnosticsError)?;
    staged
        .file_mut()
        .seek(SeekFrom::Start(0))
        .map_err(|_| DiagnosticsError)?;
    let mut verified = Vec::with_capacity(bytes.len());
    staged
        .file_mut()
        .read_to_end(&mut verified)
        .map_err(|_| DiagnosticsError)?;
    if verified != bytes {
        return Err(DiagnosticsError);
    }

    #[cfg(not(target_os = "windows"))]
    {
        fs::rename(&staged.path, destination).map_err(|_| DiagnosticsError)?;
        staged.committed = true;
        File::open(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(|_| DiagnosticsError)?;
    }
    #[cfg(target_os = "windows")]
    {
        super::rename_windows_staged_file(staged.file(), destination)
            .map_err(|_| DiagnosticsError)?;
        staged.committed = true;
    }
    Ok(())
}

fn cleanup_stale_staged_files(parent: &Path, minimum_age: Duration) {
    let Ok(entries) = fs::read_dir(parent) else {
        return;
    };
    let now = SystemTime::now();
    let mut removed = 0;
    for entry in entries.take(MAX_STALE_STAGE_SCAN_ENTRIES).flatten() {
        if removed >= MAX_STALE_STAGE_REMOVALS {
            break;
        }
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if !name.starts_with(STAGED_FILE_PREFIX) || !name.ends_with(STAGED_FILE_SUFFIX) {
            continue;
        }
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_file() {
            continue;
        }
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        let Ok(modified) = metadata.modified() else {
            continue;
        };
        let age = now.duration_since(modified).unwrap_or(Duration::ZERO);
        if age < minimum_age {
            continue;
        }
        if fs::remove_file(entry.path()).is_ok() {
            removed += 1;
        }
    }
}

struct StagedDiagnosticsFile {
    file: Option<File>,
    path: PathBuf,
    committed: bool,
}

impl StagedDiagnosticsFile {
    fn create(parent: &Path, _destination: &Path) -> Result<Self, DiagnosticsError> {
        for _ in 0..128 {
            let id = NEXT_STAGED_FILE_ID.fetch_add(1, Ordering::Relaxed);
            let path = parent.join(format!(
                "{STAGED_FILE_PREFIX}{}-{id}.tmp",
                std::process::id()
            ));
            let mut options = OpenOptions::new();
            options.read(true).write(true).create_new(true);
            #[cfg(unix)]
            options.mode(0o600);
            #[cfg(target_os = "windows")]
            options
                .access_mode(FILE_GENERIC_READ | FILE_GENERIC_WRITE | DELETE)
                .share_mode(FILE_SHARE_READ);
            match options.open(&path) {
                Ok(file) => {
                    let staged = Self {
                        file: Some(file),
                        path,
                        committed: false,
                    };
                    #[cfg(unix)]
                    {
                        let mode = fs::symlink_metadata(_destination)
                            .ok()
                            .filter(|metadata| metadata.file_type().is_file())
                            .map_or(0o600, |metadata| metadata.permissions().mode() & 0o600);
                        staged
                            .file()
                            .set_permissions(fs::Permissions::from_mode(mode))
                            .map_err(|_| DiagnosticsError)?;
                    }
                    return Ok(staged);
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(_) => return Err(DiagnosticsError),
            }
        }
        Err(DiagnosticsError)
    }

    fn file(&self) -> &File {
        self.file
            .as_ref()
            .expect("a staged diagnostics handle remains present until drop")
    }

    fn file_mut(&mut self) -> &mut File {
        self.file
            .as_mut()
            .expect("a staged diagnostics handle remains present until drop")
    }
}

impl Drop for StagedDiagnosticsFile {
    fn drop(&mut self) {
        self.file.take();
        if !self.committed {
            let _ = fs::remove_file(&self.path);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        cell::Cell,
        panic::{AssertUnwindSafe, catch_unwind},
        sync::atomic::{AtomicU64, Ordering as AtomicOrdering},
    };

    use super::*;

    static NEXT_TEST_DIRECTORY_ID: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn scope_and_bucket_wire_values_are_closed_allowlists() {
        let expected_scopes = [
            "app.unhandled_error",
            "app.unhandled_rejection",
            "app.project_snapshot",
            "app.topology_analysis",
            "app.close_guard",
            "app.validation",
            "app.benchmark",
            "fold_preview.geometry",
            "fold_preview.render",
            "fold_preview.scene_initialization",
            "fold_preview.pose_application",
            "fold_preview.pose_schedule",
            "fold_preview.selection_render",
            "fold_preview.camera",
            "fold_preview.resize",
        ];
        for (scope, expected) in DiagnosticScope::ALL.into_iter().zip(expected_scopes) {
            let encoded = serde_json::to_string(&scope).unwrap();
            assert_eq!(encoded, format!("\"{expected}\""));
            assert_eq!(
                serde_json::from_str::<DiagnosticScope>(&encoded).unwrap(),
                scope
            );
        }
        for invalid in [
            "\"app.file_operation\"",
            "\"fold_preview.render \"",
            "\"C:\\\\Users\\\\alice\\\\private.ori2\"",
            "null",
            "1",
            "{}",
        ] {
            assert!(serde_json::from_str::<DiagnosticScope>(invalid).is_err());
        }

        let buckets = [
            (0, DiagnosticCountBucket::Zero, "0"),
            (1, DiagnosticCountBucket::One, "1"),
            (2, DiagnosticCountBucket::TwoToFour, "2_4"),
            (4, DiagnosticCountBucket::TwoToFour, "2_4"),
            (5, DiagnosticCountBucket::FiveToSixteen, "5_16"),
            (16, DiagnosticCountBucket::FiveToSixteen, "5_16"),
            (17, DiagnosticCountBucket::SeventeenToSixtyFour, "17_64"),
            (64, DiagnosticCountBucket::SeventeenToSixtyFour, "17_64"),
            (65, DiagnosticCountBucket::SixtyFivePlus, "65_plus"),
            (u8::MAX, DiagnosticCountBucket::SixtyFivePlus, "65_plus"),
        ];
        for (count, bucket, wire) in buckets {
            assert_eq!(DiagnosticCountBucket::from_count(count), bucket);
            assert_eq!(
                serde_json::to_string(&bucket).unwrap(),
                format!("\"{wire}\"")
            );
        }
    }

    #[test]
    fn canonical_snapshot_contains_only_schema_and_all_scopes() {
        let directory = TestDirectory::new();
        let destination = directory.path.join(DIAGNOSTICS_FILE_NAME);
        let state = test_state(destination.clone());
        state
            .record(DiagnosticScope::FoldPreviewPoseApplication)
            .unwrap();

        let bytes = fs::read(&destination).unwrap();
        assert!(bytes.len() <= MAX_DIAGNOSTICS_BYTES);
        let stored: StoredDiagnostics = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(stored.schema, DiagnosticsSchema::V1);
        assert_eq!(stored.unexpected.len(), DiagnosticScope::ALL.len());
        for (index, entry) in stored.unexpected.iter().enumerate() {
            assert_eq!(entry.scope, DiagnosticScope::ALL[index]);
            assert_eq!(
                entry.count,
                if entry.scope == DiagnosticScope::FoldPreviewPoseApplication {
                    DiagnosticCountBucket::One
                } else {
                    DiagnosticCountBucket::Zero
                }
            );
        }

        let text = String::from_utf8(bytes).unwrap();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        let keys = value.as_object().unwrap().keys().collect::<Vec<_>>();
        assert_eq!(keys, ["schema", "unexpected"]);
        for secret in [
            r"C:\Users\alice\作品\dragon.ori2",
            "/Users/alice/private-fold.ori2",
            "private-project-name",
            "123e4567-e89b-12d3-a456-426614174000",
            "x=12.345,y=-67.89",
            "stack",
            "message",
            "app_version",
            "target_os",
            "target_arch",
        ] {
            assert!(!text.contains(secret));
        }
        assert!(text.contains(DIAGNOSTICS_SCHEMA));
    }

    #[test]
    fn counts_cross_exact_buckets_and_saturate_at_sixty_five() {
        let directory = TestDirectory::new();
        let destination = directory.path.join(DIAGNOSTICS_FILE_NAME);
        let state = test_state(destination);
        let scope = DiagnosticScope::FoldPreviewRender;
        let boundaries = [
            (1, DiagnosticCountBucket::One),
            (2, DiagnosticCountBucket::TwoToFour),
            (4, DiagnosticCountBucket::TwoToFour),
            (5, DiagnosticCountBucket::FiveToSixteen),
            (16, DiagnosticCountBucket::FiveToSixteen),
            (17, DiagnosticCountBucket::SeventeenToSixtyFour),
            (64, DiagnosticCountBucket::SeventeenToSixtyFour),
            (65, DiagnosticCountBucket::SixtyFivePlus),
            (256, DiagnosticCountBucket::SixtyFivePlus),
        ];
        let mut reports = 0;
        for (target, expected) in boundaries {
            while reports < target {
                state.record(scope).unwrap();
                reports += 1;
            }
            let count = state.store.lock().unwrap().counts[scope.index()];
            assert_eq!(DiagnosticCountBucket::from_count(count), expected);
        }
        assert_eq!(state.store.lock().unwrap().counts[scope.index()], MAX_COUNT);
    }

    #[test]
    fn native_delivery_reservations_are_independently_bounded_per_scope() {
        let directory = TestDirectory::new();
        let state = test_state(directory.path.join(DIAGNOSTICS_FILE_NAME));
        for scope in [
            DiagnosticScope::AppValidation,
            DiagnosticScope::FoldPreviewRender,
        ] {
            for _ in 0..MAX_COUNT {
                assert!(state.try_reserve_delivery(scope));
            }
            assert!(!state.try_reserve_delivery(scope));
            assert!(!state.try_reserve_delivery(scope));
            assert_eq!(
                state.native_deliveries[scope.index()].load(Ordering::Relaxed),
                MAX_COUNT
            );
        }
        assert_eq!(
            state.native_deliveries[DiagnosticScope::FoldPreviewCamera.index()]
                .load(Ordering::Relaxed),
            0
        );
    }

    #[test]
    fn share_preview_caches_exact_bytes_across_later_diagnostics() {
        let directory = TestDirectory::new();
        let destination = directory.path.join(DIAGNOSTICS_FILE_NAME);
        let export = directory.path.join(DIAGNOSTICS_SHARE_FILE_NAME);
        let state = test_state(destination);
        state.record(DiagnosticScope::AppProjectSnapshot).unwrap();

        let preview = state.prepare_share_preview().unwrap();
        assert_eq!(preview.preview_generation, 1);
        assert_eq!(
            usize::try_from(preview.byte_length).unwrap(),
            preview.json.len()
        );
        assert_eq!(
            serde_json::to_value(&preview).unwrap(),
            serde_json::json!({
                "preview_generation": preview.preview_generation,
                "json": preview.json,
                "byte_length": preview.byte_length,
            })
        );
        let cached = state
            .cached_share_preview(preview.preview_generation)
            .unwrap();
        assert_eq!(cached.bytes.as_ref(), preview.json.as_bytes());

        state.record(DiagnosticScope::FoldPreviewRender).unwrap();
        let after_record = state
            .cached_share_preview(preview.preview_generation)
            .unwrap();
        assert_eq!(after_record.bytes.as_ref(), preview.json.as_bytes());
        let saved = complete_diagnostics_share_save(
            after_record,
            Some(export.clone()),
            persist_canonical_diagnostics,
        )
        .unwrap();
        assert_eq!(
            saved,
            DiagnosticsShareSaveResponse {
                preview_generation: preview.preview_generation,
                byte_length: preview.byte_length,
                canceled: false,
            }
        );
        assert_eq!(fs::read(export).unwrap(), preview.json.as_bytes());

        let next = state.prepare_share_preview().unwrap();
        assert_eq!(next.preview_generation, 2);
        assert_ne!(next.json, preview.json);
        assert!(
            state
                .cached_share_preview(preview.preview_generation)
                .is_err()
        );
        assert!(state.cached_share_preview(next.preview_generation).is_ok());
    }

    #[test]
    fn share_save_cancel_skips_writes_and_returns_only_fixed_metadata() {
        let directory = TestDirectory::new();
        let state = test_state(directory.path.join(DIAGNOSTICS_FILE_NAME));
        let preview = state.prepare_share_preview().unwrap();
        let cached = state
            .cached_share_preview(preview.preview_generation)
            .unwrap();
        let writer_called = Cell::new(false);

        let response = complete_diagnostics_share_save(cached, None, |_, _| {
            writer_called.set(true);
            Ok(())
        })
        .unwrap();

        assert!(!writer_called.get());
        assert_eq!(
            response,
            DiagnosticsShareSaveResponse {
                preview_generation: preview.preview_generation,
                byte_length: preview.byte_length,
                canceled: true,
            }
        );
        assert_eq!(
            serde_json::to_value(response).unwrap(),
            serde_json::json!({
                "preview_generation": preview.preview_generation,
                "byte_length": preview.byte_length,
                "canceled": true,
            })
        );
        assert!(
            state
                .cached_share_preview(preview.preview_generation)
                .is_ok()
        );
    }

    #[test]
    fn share_preview_rejects_unknown_exhausted_and_oversized_cache_state() {
        let directory = TestDirectory::new();
        let state = test_state(directory.path.join(DIAGNOSTICS_FILE_NAME));
        let first = state.prepare_share_preview().unwrap();
        assert!(state.cached_share_preview(0).is_err());
        assert!(
            state
                .cached_share_preview(first.preview_generation + 1)
                .is_err()
        );
        let noncanonical = format!(" {}", first.json);
        assert!(validate_canonical_diagnostics(noncanonical.as_bytes()).is_err());

        {
            let mut cache = state.share_preview_cache.lock().unwrap();
            cache.last_generation = u32::MAX;
        }
        assert_eq!(state.prepare_share_preview(), Err(DiagnosticsError));
        assert!(state.cached_share_preview(first.preview_generation).is_ok());

        {
            let mut cache = state.share_preview_cache.lock().unwrap();
            cache.current.as_mut().unwrap().bytes =
                Arc::from(vec![b'x'; MAX_DIAGNOSTICS_BYTES + 1]);
        }
        assert!(
            state
                .cached_share_preview(first.preview_generation)
                .is_err()
        );
        assert_eq!(GENERIC_DIAGNOSTICS_ERROR, "diagnostics unavailable");
    }

    #[test]
    fn failed_share_write_preserves_the_cached_preview_and_exposes_no_path() {
        let directory = TestDirectory::new();
        let destination = directory.path.join(DIAGNOSTICS_FILE_NAME);
        let failed_destination = directory.path.join("selected-private-path");
        fs::create_dir(&failed_destination).unwrap();
        let state = test_state(destination);
        let preview = state.prepare_share_preview().unwrap();
        let cached = state
            .cached_share_preview(preview.preview_generation)
            .unwrap();

        assert_eq!(
            complete_diagnostics_share_save(
                cached,
                Some(failed_destination.clone()),
                persist_canonical_diagnostics,
            ),
            Err(DiagnosticsError)
        );
        assert!(failed_destination.is_dir());
        assert!(
            state
                .cached_share_preview(preview.preview_generation)
                .is_ok()
        );
        assert!(!GENERIC_DIAGNOSTICS_ERROR.contains("selected-private-path"));
    }

    #[test]
    fn restart_uses_only_the_persisted_bucket_lower_bound() {
        let directory = TestDirectory::new();
        let destination = directory.path.join(DIAGNOSTICS_FILE_NAME);
        let scope = DiagnosticScope::AppValidation;
        let state = test_state(destination.clone());
        for _ in 0..4 {
            state.record(scope).unwrap();
        }
        assert_eq!(state.store.lock().unwrap().counts[scope.index()], 4);
        assert_eq!(
            persisted_bucket(&destination, scope),
            DiagnosticCountBucket::TwoToFour
        );

        let restarted = test_state(destination.clone());
        assert_eq!(restarted.store.lock().unwrap().counts[scope.index()], 2);
        for _ in 0..3 {
            restarted.record(scope).unwrap();
        }
        assert_eq!(
            persisted_bucket(&destination, scope),
            DiagnosticCountBucket::FiveToSixteen
        );
    }

    #[test]
    fn persisted_shape_matches_the_frontend_v1_contract_exactly() {
        let directory = TestDirectory::new();
        let destination = directory.path.join(DIAGNOSTICS_FILE_NAME);
        let state = test_state(destination.clone());
        state.record(DiagnosticScope::AppProjectSnapshot).unwrap();

        let value: serde_json::Value =
            serde_json::from_slice(&fs::read(destination).unwrap()).unwrap();
        assert_eq!(
            value,
            serde_json::json!({
                "schema": "origami2.redacted-diagnostics.v1",
                "unexpected": DiagnosticScope::ALL
                    .into_iter()
                    .map(|scope| serde_json::json!({
                        "scope": scope,
                        "count": if scope == DiagnosticScope::AppProjectSnapshot {
                            "1"
                        } else {
                            "0"
                        },
                    }))
                    .collect::<Vec<_>>(),
            })
        );
    }

    #[test]
    fn malformed_bounded_and_oversized_files_fail_closed_then_recover() {
        let cases = [
            b"{not json".to_vec(),
            serde_json::to_vec(&serde_json::json!({
                "schema": DIAGNOSTICS_SCHEMA,
                "app_version": "0.1.0",
                "target_os": "windows",
                "target_arch": "x86_64",
                "unexpected": [],
            }))
            .unwrap(),
            serde_json::to_vec(&serde_json::json!({
                "schema": DIAGNOSTICS_SCHEMA,
                "unexpected": [],
            }))
            .unwrap(),
            vec![b' '; MAX_DIAGNOSTICS_BYTES + 1],
        ];

        for bytes in cases {
            let directory = TestDirectory::new();
            let destination = directory.path.join(DIAGNOSTICS_FILE_NAME);
            fs::write(&destination, &bytes).unwrap();
            let state = test_state(destination.clone());
            assert_eq!(fs::read(&destination).unwrap(), bytes);
            assert!(
                state
                    .store
                    .lock()
                    .unwrap()
                    .counts
                    .iter()
                    .all(|count| *count == 0)
            );
            state.record(DiagnosticScope::AppUnhandledError).unwrap();
            let recovered = load_counts(&destination).unwrap();
            assert_eq!(recovered[DiagnosticScope::AppUnhandledError.index()], 1);
        }
    }

    #[test]
    fn duplicate_missing_extra_and_noncanonical_entries_are_rejected() {
        let canonical = StoredDiagnostics::from_counts(&[0; DiagnosticScope::ALL.len()]);
        let bytes = serde_json::to_vec(&canonical).unwrap();
        let text = String::from_utf8(bytes).unwrap();
        let duplicate_schema = text.replacen(
            "\"schema\":",
            "\"schema\":\"origami2.redacted-diagnostics.v1\",\"schema\":",
            1,
        );
        assert!(serde_json::from_str::<StoredDiagnostics>(&duplicate_schema).is_err());

        let mut value = serde_json::to_value(&canonical).unwrap();
        value.as_object_mut().unwrap().remove("unexpected");
        assert!(serde_json::from_value::<StoredDiagnostics>(value).is_err());

        let mut value = serde_json::to_value(&canonical).unwrap();
        value
            .as_object_mut()
            .unwrap()
            .insert("project_path".to_owned(), serde_json::json!("secret.ori2"));
        assert!(serde_json::from_value::<StoredDiagnostics>(value).is_err());

        let mut reversed = canonical.clone();
        reversed.unexpected.reverse();
        assert!(reversed.validated_counts().is_err());

        let mut duplicate = canonical;
        duplicate.unexpected[1].scope = duplicate.unexpected[0].scope;
        assert!(duplicate.validated_counts().is_err());
    }

    #[test]
    fn atomic_replacement_leaves_no_staged_files() {
        let directory = TestDirectory::new();
        let destination = directory.path.join(DIAGNOSTICS_FILE_NAME);
        let state = test_state(destination.clone());
        state.record(DiagnosticScope::FoldPreviewGeometry).unwrap();
        let first = fs::read(&destination).unwrap();
        state.record(DiagnosticScope::FoldPreviewCamera).unwrap();
        let second = fs::read(&destination).unwrap();
        assert_ne!(first, second);
        assert_eq!(
            persisted_bucket(&destination, DiagnosticScope::FoldPreviewGeometry),
            DiagnosticCountBucket::One
        );
        assert_eq!(
            persisted_bucket(&destination, DiagnosticScope::FoldPreviewCamera),
            DiagnosticCountBucket::One
        );
        let staged = fs::read_dir(&directory.path)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(STAGED_FILE_PREFIX)
            })
            .count();
        assert_eq!(staged, 0);
    }

    #[test]
    fn failed_atomic_commit_removes_its_staged_file() {
        let directory = TestDirectory::new();
        let destination = directory.path.join(DIAGNOSTICS_FILE_NAME);
        fs::create_dir(&destination).unwrap();
        let state = test_state(destination.clone());
        assert_eq!(
            state.record(DiagnosticScope::FoldPreviewResize),
            Err(DiagnosticsError)
        );
        assert!(destination.is_dir());
        let staged = fs::read_dir(&directory.path)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(STAGED_FILE_PREFIX)
            })
            .count();
        assert_eq!(staged, 0);
    }

    #[test]
    fn startup_cleanup_is_bounded_to_matching_stale_regular_files() {
        let directory = TestDirectory::new();
        let stale_a = directory
            .path
            .join(format!("{STAGED_FILE_PREFIX}101-1{STAGED_FILE_SUFFIX}"));
        let stale_b = directory
            .path
            .join(format!("{STAGED_FILE_PREFIX}102-2{STAGED_FILE_SUFFIX}"));
        let unrelated = directory.path.join("unrelated.tmp");
        let matching_directory = directory
            .path
            .join(format!("{STAGED_FILE_PREFIX}directory{STAGED_FILE_SUFFIX}"));
        fs::write(&stale_a, b"redacted snapshot").unwrap();
        fs::write(&stale_b, b"redacted snapshot").unwrap();
        fs::write(&unrelated, b"keep").unwrap();
        fs::create_dir(&matching_directory).unwrap();

        cleanup_stale_staged_files(&directory.path, Duration::ZERO);

        assert!(!stale_a.exists());
        assert!(!stale_b.exists());
        assert!(unrelated.exists());
        assert!(matching_directory.is_dir());
        assert_eq!(MAX_STALE_STAGE_REMOVALS, 32);
        assert_eq!(MAX_STALE_STAGE_SCAN_ENTRIES, 512);
    }

    #[cfg(unix)]
    #[test]
    fn persisted_files_are_user_only_and_preserve_stricter_owner_mode() {
        use std::os::unix::fs::PermissionsExt as _;

        let directory = TestDirectory::new();
        let destination = directory.path.join(DIAGNOSTICS_FILE_NAME);
        let state = test_state(destination.clone());
        state.record(DiagnosticScope::FoldPreviewRender).unwrap();
        assert_eq!(
            fs::metadata(&destination).unwrap().permissions().mode() & 0o777,
            0o600
        );

        fs::set_permissions(&destination, fs::Permissions::from_mode(0o400)).unwrap();
        let restarted = test_state(destination.clone());
        restarted
            .record(DiagnosticScope::FoldPreviewCamera)
            .unwrap();
        assert_eq!(
            fs::metadata(&destination).unwrap().permissions().mode() & 0o777,
            0o400
        );
    }

    #[test]
    fn io_failures_disable_persistence_without_exposing_paths() {
        let directory = TestDirectory::new();
        let blocking_file = directory.path.join("not-a-directory");
        fs::write(&blocking_file, b"blocking file").unwrap();
        let failed_destination = blocking_file.join(DIAGNOSTICS_FILE_NAME);
        let failing = test_state(failed_destination.clone());
        assert_eq!(
            failing.record(DiagnosticScope::AppBenchmark),
            Err(DiagnosticsError)
        );
        assert!(failing.store.lock().unwrap().persistence_disabled);
        fs::remove_file(&blocking_file).unwrap();
        fs::create_dir(&blocking_file).unwrap();
        assert_eq!(
            failing.record(DiagnosticScope::AppBenchmark),
            Err(DiagnosticsError)
        );
        assert!(!failed_destination.exists());
        assert_eq!(GENERIC_DIAGNOSTICS_ERROR, "diagnostics unavailable");
        assert!(!GENERIC_DIAGNOSTICS_ERROR.contains("not-a-directory"));
    }

    #[test]
    fn poisoned_state_returns_only_the_generic_failure() {
        let directory = TestDirectory::new();
        let state = test_state(directory.path.join(DIAGNOSTICS_FILE_NAME));
        let _ = catch_unwind(AssertUnwindSafe(|| {
            let _guard = state.store.lock().unwrap();
            panic!("poison diagnostics mutex");
        }));
        assert_eq!(
            state.record(DiagnosticScope::AppUnhandledRejection),
            Err(DiagnosticsError)
        );
        assert_eq!(state.prepare_share_preview(), Err(DiagnosticsError));

        let cache_state = test_state(directory.path.join("cache-poison.json"));
        let preview = cache_state.prepare_share_preview().unwrap();
        let _ = catch_unwind(AssertUnwindSafe(|| {
            let _guard = cache_state.share_preview_cache.lock().unwrap();
            panic!("poison diagnostics share cache");
        }));
        assert_eq!(cache_state.prepare_share_preview(), Err(DiagnosticsError));
        assert!(
            cache_state
                .cached_share_preview(preview.preview_generation)
                .is_err()
        );
        assert_eq!(GENERIC_DIAGNOSTICS_ERROR, "diagnostics unavailable");
    }

    fn test_state(destination: PathBuf) -> DiagnosticsState {
        DiagnosticsState::from_destination(Some(destination))
    }

    fn persisted_bucket(destination: &Path, scope: DiagnosticScope) -> DiagnosticCountBucket {
        let stored: StoredDiagnostics =
            serde_json::from_slice(&fs::read(destination).unwrap()).unwrap();
        stored.unexpected[scope.index()].count
    }

    struct TestDirectory {
        path: PathBuf,
    }

    impl TestDirectory {
        fn new() -> Self {
            let root = std::env::temp_dir();
            for _ in 0..128 {
                let id = NEXT_TEST_DIRECTORY_ID.fetch_add(1, AtomicOrdering::Relaxed);
                let path = root.join(format!(
                    "origami2-diagnostics-test-{}-{id}",
                    std::process::id()
                ));
                match fs::create_dir(&path) {
                    Ok(()) => return Self { path },
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                    Err(error) => panic!("failed to create diagnostics test directory: {error}"),
                }
            }
            panic!("failed to allocate diagnostics test directory");
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}
