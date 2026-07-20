mod applied_pose;
mod crease_export;
mod diagnostics;
mod fold_3d_frames_import;
mod fold_technique_file_io;
mod global_flat_foldability;
mod history_settings;
mod instruction_export;
mod mesh_animation_export;
mod mesh_export;
mod numeric_expression;
mod project_folder_io;
mod project_persistence;
#[allow(dead_code)]
mod recovery;
mod save_path;
mod stacked_fold_read;
mod stacked_fold_transaction;
use stacked_fold_transaction::StackedFoldTransactionState;

use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicBool, Ordering},
    },
};

use applied_pose::{
    ApplyCurrentNativePoseResponse, CurrentAppliedPoseAuthority,
    CurrentStaticCollisionDiagnosticResponse, NativePoseRequest,
    apply_current_native_pose as apply_current_native_pose_authority, commit_project_replacement,
    inspect_current_static_collision as inspect_current_static_collision_authority,
    restore_persisted_current_pose,
};
use crease_export::{
    CreaseExportState, cancel_crease_pattern_export, preview_crease_pattern_export,
    save_crease_pattern_export,
};
use diagnostics::{
    DiagnosticsState, prepare_diagnostics_share_preview, record_unexpected_diagnostic,
    save_diagnostics_share_preview,
};
use fold_3d_frames_import::{
    Fold3dFramesImportState, apply_fold_3d_applied_pose, apply_fold_3d_instruction_timeline,
    cancel_fold_3d_frames, prepare_fold_3d_applied_pose, prepare_fold_3d_instruction_timeline,
    preview_fold_3d_frames, select_fold_3d_frame,
};
use fold_technique_file_io::{
    FoldTechniqueFileIoState, open_fold_technique_file, save_fold_technique_file_as,
};
use global_flat_foldability::{
    GlobalFlatFoldabilityState, begin_global_flat_foldability, cancel_global_flat_foldability,
    get_global_flat_foldability_progress, get_global_flat_foldability_result,
};
use history_settings::{get_history_entry_limit, set_history_entry_limit};
use instruction_export::{
    InstructionExportState, begin_instruction_export, cancel_instruction_export,
    get_instruction_export_progress, preview_instruction_export, save_instruction_export,
};
use mesh_animation_export::{
    MeshAnimationExportState, cancel_instruction_mesh_animation,
    preview_instruction_mesh_animation, save_instruction_mesh_animation,
};
use mesh_export::{
    StaticMeshExportState, cancel_static_mesh_export, preview_static_mesh_export,
    save_static_mesh_export,
};
use numeric_expression::{
    PositiveMillimetrePairError, evaluate_finite_millimetre_pair, evaluate_numeric_expression,
    evaluate_positive_millimetre_pair, evaluate_positive_millimetre_pair_in_worker,
};
use ori_core::{
    BoundaryEdgeRef, Command, ConstraintPreflightV1, DirectConstraintConflictV1, EditorState,
    EditorTopology, GeometricConstraintLimitsV1, GeometricConstraintUnknownReasonV1,
    IntersectionEdgeTarget, JunctionVertexIntent, LocalFlatFoldabilityReport,
    MAX_EDITOR_HISTORY_ENTRIES, PaperValidationIssue, PointPolygonRelation, TopologyAnalysisInput,
    TopologyIssue, TopologySnapshot, ValidationIssue, VertexPositionUpdate,
    analyze_local_flat_foldability, create_rectangular_sheet, prepare_geometric_constraints_v1,
    segment_midpoint_polygon_relation, validate_paper,
};
use ori_domain::{
    AssetId, ConstraintId, CreasePattern, EdgeId, EdgeKind, FaceId, GeometricConstraintDocumentV1,
    GeometricConstraintKindV1, GeometricConstraintRecordV1, InstructionHingeAngle, InstructionPose,
    InstructionPoseModel, InstructionStep, InstructionStepId, InstructionTimeline,
    InstructionVisual, LayerContentKindV1, LayerId, LayerRecordV1, LengthDisplayUnit,
    MAX_INSTRUCTION_HINGES_PER_STEP, MAX_INSTRUCTION_STEPS, Paper, Point2, ProjectId,
    ProjectLayerDocumentV1, RgbaColor, VertexId,
};
use ori_formats::{
    CURRENT_FORMAT_VERSION, FoldAssignmentMapping, FoldAssignmentTarget, FoldBoundaryCandidateId,
    FoldBoundaryCandidateSource, FoldConversionOptions, FoldEdgeAssignment, FoldFrameUnit,
    FoldPreview, FoldPreviewWarning, MAX_PROJECT_TEXTURE_ASSET_BYTES,
    MAX_PROJECT_TEXTURE_ASSET_TOTAL_BYTES, Ori2ProjectArchive, PolarVertexConstructionExpressions,
    ProjectDocument, ProjectNumericExpressions, ProjectTextureAssetV1, ProjectTextureMediaTypeV1,
    RectangularPaperCreationExpressions, SvgBoundaryCandidateId, SvgBoundaryCandidateKind,
    SvgConversionOptions, SvgDashPattern, SvgGroupMapping, SvgGroupTarget, SvgLineCap, SvgPreview,
    SvgPreviewWarning, SvgRootPhysicalSize, SvgRootViewBox, SvgStyleGroupId, SvgWarningKind,
    VertexCoordinateExpressionChange, VertexCoordinateExpressionTransition,
    VertexCoordinateExpressions, generate_project_thumbnail_svg, read_fold_preview,
    read_svg_preview,
};
use project_folder_io::{ProjectFolderIoState, open_project_folder, save_project_folder_as};
#[cfg(test)]
use project_persistence::{
    PROJECT_FILE_INVALID_MESSAGE, PROJECT_FILE_OPEN_FAILED_MESSAGE, PROJECT_FILE_TOO_LARGE_MESSAGE,
    PROJECT_INSTRUCTIONS_INVALID_MESSAGE, PROJECT_INSTRUCTIONS_SAVE_FAILED_MESSAGE,
    containing_directory, load_document_from_path, persist_document, persist_project_archive,
    verify_generated_ori2,
};
use project_persistence::{
    PROJECT_FILE_INVALID_MESSAGE as PROJECT_ARCHIVE_INVALID_MESSAGE,
    PROJECT_SERIALIZATION_FAILED_MESSAGE, StagedFile, create_staged_file,
    load_project_archive_from_path, persist_project_archive_to_destination,
};
#[cfg(all(test, not(target_os = "windows")))]
use project_persistence::{commit_unix_staged_project_file, prepare_staged_file};
use recovery::{
    ExitRecoveryAuthorization, ExitRecoveryDisposition, PreparedWindowCloseSettlement,
    RecoveryRuntime, cancel_window_close_prepare, discard_recovery, get_recovery_autosave_status,
    get_recovery_candidate, prepare_window_close, restore_recovery, start_recovery_autosave_timer,
};
use serde::{Deserialize, Serialize};
use stacked_fold_read::propose_current_stacked_fold_read;
use stacked_fold_transaction::{
    apply_stacked_fold_transaction, cancel_stacked_fold_transaction_preview,
};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

#[cfg(target_os = "windows")]
use std::{
    mem::size_of,
    os::windows::{
        ffi::OsStrExt,
        io::{AsRawHandle, RawHandle},
    },
    ptr,
};
#[cfg(target_os = "windows")]
use windows_sys::Win32::Storage::FileSystem::{
    FILE_RENAME_INFO, FileRenameInfo, SetFileInformationByHandle,
};

#[cfg(target_os = "macos")]
use tauri::menu::{
    AboutMetadata, HELP_SUBMENU_ID, Menu, MenuItem, PredefinedMenuItem, Submenu, WINDOW_SUBMENU_ID,
};

const UNTITLED_PROJECT_NAME: &str = "Untitled";
const DEFAULT_SHEET_SIZE_MM: f64 = 400.0;
const MAX_PROJECT_NAME_CHARS: usize = 120;
const MAX_BENCHMARK_EDGE_COUNT: usize = 100_000;
const MAX_FOLD_IMPORT_FILE_SIZE: u64 = 16 * 1024 * 1024;
const MAX_FOLD_IMPORT_PREVIEW_EDGES: usize = 5_000;
const MAX_FOLD_IMPORT_CONTAINMENT_TESTS: usize = 1_000_000;
const FOLD_IMPORT_FILE_LABEL: &str = "選択したFOLDファイル";
const FOLD_IMPORT_FALLBACK_NAME: &str = "FOLDインポート";
const MAX_SVG_IMPORT_FILE_SIZE: u64 = 16 * 1024 * 1024;
const MAX_SVG_IMPORT_PREVIEW_EDGES: usize = 5_000;
const SVG_IMPORT_FILE_LABEL: &str = "選択したSVGファイル";
const SVG_IMPORT_FALLBACK_NAME: &str = "SVGインポート";
const TOPOLOGY_ANALYSIS_TASK_FAILED_MESSAGE: &str =
    "構造解析処理を完了できませんでした。もう一度実行してください。";
const INSTRUCTION_TOPOLOGY_ANALYSIS_TASK_FAILED_MESSAGE: &str =
    "折り手順の構造解析処理を完了できませんでした。もう一度実行してください。";
const FOLD_IMPORT_TASK_FAILED_MESSAGE: &str =
    "FOLDファイルの解析処理を完了できませんでした。もう一度実行してください。";
const FOLD_CONVERSION_TASK_FAILED_MESSAGE: &str =
    "FOLDファイルの変換処理を完了できませんでした。もう一度実行してください。";
const FOLD_FILE_OPEN_FAILED_MESSAGE: &str = "選択されたFOLDファイルを開けませんでした。";
const FOLD_FILE_INSPECTION_FAILED_MESSAGE: &str =
    "選択されたFOLDファイルのサイズを確認できませんでした。";
const FOLD_FILE_TOO_LARGE_MESSAGE: &str = "選択されたFOLDファイルはサイズ上限を超えています。";
const FOLD_FILE_READ_FAILED_MESSAGE: &str = "選択されたFOLDファイルを読み込めませんでした。";
const FOLD_FILE_INVALID_MESSAGE: &str =
    "選択されたFOLDファイルが破損しているか、対応していない形式です。";
const SVG_FILE_OPEN_FAILED_MESSAGE: &str = "選択されたSVGファイルを開けませんでした。";
const SVG_FILE_INSPECTION_FAILED_MESSAGE: &str =
    "選択されたSVGファイルのサイズを確認できませんでした。";
const SVG_FILE_TOO_LARGE_MESSAGE: &str = "選択されたSVGファイルはサイズ上限を超えています。";
const SVG_FILE_READ_FAILED_MESSAGE: &str = "選択されたSVGファイルを読み込めませんでした。";
const SVG_FILE_INVALID_MESSAGE: &str =
    "選択されたSVGファイルが破損しているか、対応していない形式です。";
const PROJECT_OPEN_TASK_FAILED_MESSAGE: &str =
    "プロジェクトの読み込み処理を完了できませんでした。もう一度実行してください。";
const PROJECT_NUMERIC_EXPRESSIONS_INVALID_MESSAGE: &str =
    "保存された作成時サイズ式を検証できませんでした。";
const PROJECT_NUMERIC_EXPRESSIONS_BUSY_MESSAGE: &str =
    "作成時サイズ式を評価中です。少し待ってからもう一度開いてください。";
const GEOMETRIC_CONSTRAINT_ANALYSIS_BUSY_MESSAGE: &str =
    "geometric-constraint analysis is already in progress";
const GEOMETRIC_CONSTRAINT_ANALYSIS_FAILED_MESSAGE: &str =
    "geometric-constraint analysis did not complete";
#[cfg(target_os = "macos")]
const MACOS_QUIT_MENU_ID: &str = "origami2_quit";

fn topology_analysis_task_error<T>(_: T) -> String {
    TOPOLOGY_ANALYSIS_TASK_FAILED_MESSAGE.to_owned()
}

fn instruction_topology_analysis_task_error<T>(_: T) -> String {
    INSTRUCTION_TOPOLOGY_ANALYSIS_TASK_FAILED_MESSAGE.to_owned()
}

fn fold_import_task_error<T>(_: T) -> String {
    FOLD_IMPORT_TASK_FAILED_MESSAGE.to_owned()
}

fn fold_conversion_task_error<T>(_: T) -> String {
    FOLD_CONVERSION_TASK_FAILED_MESSAGE.to_owned()
}

fn fold_file_invalid_error<T>(_: T) -> String {
    FOLD_FILE_INVALID_MESSAGE.to_owned()
}

fn geometric_constraint_analysis_task_error<T>(_: T) -> String {
    GEOMETRIC_CONSTRAINT_ANALYSIS_FAILED_MESSAGE.to_owned()
}

/// Process-lifetime application state.
///
/// The native pose worker gate deliberately lives beside, rather than inside,
/// `ProjectState`. Replacing or reopening a project therefore cannot create a
/// fresh gate while an obsolete project's heavy worker is still running.
struct AppState(
    Mutex<ProjectState>,
    NativePoseWorkerGate,
    GeometricConstraintWorkerGate,
);

impl AppState {
    fn new(project: ProjectState) -> Self {
        Self(
            Mutex::new(project),
            NativePoseWorkerGate::default(),
            GeometricConstraintWorkerGate::default(),
        )
    }

    fn try_acquire_native_pose_worker(&self) -> Option<NativePoseWorkerPermit> {
        self.1.try_acquire()
    }

    #[cfg(test)]
    fn native_pose_worker_is_busy(&self) -> bool {
        self.1.is_busy()
    }

    fn try_acquire_geometric_constraint_worker(&self) -> Option<GeometricConstraintWorkerPermit> {
        self.2.try_acquire()
    }

    #[cfg(test)]
    fn geometric_constraint_worker_is_busy(&self) -> bool {
        self.2.is_busy()
    }
}

/// One process-wide heavy native pose worker per managed [`AppState`].
///
/// The permit owns the shared atomic so it can move into `spawn_blocking`.
/// Cancellation of the awaiting future cannot release the gate while the
/// blocking closure is still running.
#[derive(Clone, Default)]
struct NativePoseWorkerGate(Arc<AtomicBool>);

impl NativePoseWorkerGate {
    fn try_acquire(&self) -> Option<NativePoseWorkerPermit> {
        self.0
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .ok()
            .map(|_| NativePoseWorkerPermit {
                busy: Arc::clone(&self.0),
            })
    }

    #[cfg(test)]
    fn is_busy(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

struct NativePoseWorkerPermit {
    busy: Arc<AtomicBool>,
}

impl Drop for NativePoseWorkerPermit {
    fn drop(&mut self) {
        let was_busy = self.busy.swap(false, Ordering::Release);
        debug_assert!(was_busy, "native pose worker permit released twice");
    }
}

/// Process-wide gate for bounded geometric-constraint preflight work.
///
/// The permit owns the shared atomic and moves into `spawn_blocking`, so
/// abandoning an awaiting WebView request cannot release the gate before the
/// native worker actually exits.
#[derive(Clone, Default)]
struct GeometricConstraintWorkerGate(Arc<AtomicBool>);

impl GeometricConstraintWorkerGate {
    fn try_acquire(&self) -> Option<GeometricConstraintWorkerPermit> {
        self.0
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .ok()
            .map(|_| GeometricConstraintWorkerPermit {
                busy: Arc::clone(&self.0),
            })
    }

    #[cfg(test)]
    fn is_busy(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

struct GeometricConstraintWorkerPermit {
    busy: Arc<AtomicBool>,
}

impl Drop for GeometricConstraintWorkerPermit {
    fn drop(&mut self) {
        let was_busy = self.busy.swap(false, Ordering::Release);
        debug_assert!(
            was_busy,
            "geometric constraint worker permit released twice"
        );
    }
}

#[derive(Default)]
struct FoldImportState(Mutex<Option<PendingFoldImport>>);

#[derive(Clone)]
struct PendingFoldImport {
    import_id: ProjectId,
    expected_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    bytes: Arc<[u8]>,
}

#[derive(Default)]
struct SvgImportState(Mutex<SvgImportSlot>);

#[derive(Default)]
struct SvgImportSlot {
    pending: Option<PendingSvgImport>,
    validation_generation_id: Option<ProjectId>,
    validation: Option<SvgImportSettingsValidation>,
    last_cancelled_id: Option<ProjectId>,
}

#[derive(Clone)]
struct PendingSvgImport {
    import_id: ProjectId,
    expected_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    bytes: Arc<[u8]>,
}

#[derive(Clone)]
struct SvgImportSettingsValidation {
    validation_id: ProjectId,
    import_id: ProjectId,
    expected_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    millimeters_per_unit_bits: u64,
    boundary_candidate: Option<SvgBoundaryCandidateId>,
    group_mappings: Vec<SvgGroupMapping>,
}

struct SvgImportSettingsValidationCompletion {
    validation: SvgImportSettingsValidation,
    geometry: SvgImportGeometryValidation,
}

#[derive(Default)]
struct ExitGuard {
    allow_once: AtomicBool,
    dialog_open: AtomicBool,
}

struct ProjectState {
    /// Non-persisted identity for this particular open/new project instance.
    ///
    /// A persisted project ID can legitimately reappear after reopening the
    /// same file. Delayed mutating work must therefore bind to this identity
    /// as well as the document ID and revision.
    instance_id: ProjectId,
    project_id: ProjectId,
    name: String,
    current_path: Option<PathBuf>,
    editor: EditorState,
    /// In-process native current-pose authority for this open project.
    ///
    /// The authority has its own slot so the global lock order remains
    /// `project -> pose -> layer order`. It is never persisted.
    applied_pose_authority: CurrentAppliedPoseAuthority,
    numeric_expressions: ProjectNumericExpressions,
    texture_assets: Vec<ori_formats::ProjectTextureAssetV1>,
    saved_revision: Option<u64>,
    saved_document: Option<ProjectDocument>,
}

impl ProjectState {
    #[cfg(test)]
    fn new(pattern: CreasePattern) -> Self {
        Self::new_with_paper(pattern, Paper::default())
    }

    fn new_with_paper(pattern: CreasePattern, paper: Paper) -> Self {
        let editor = EditorState::with_paper(pattern, paper);
        let mut project = Self {
            instance_id: ProjectId::new(),
            project_id: ProjectId::new(),
            name: UNTITLED_PROJECT_NAME.to_owned(),
            current_path: None,
            editor,
            applied_pose_authority: CurrentAppliedPoseAuthority::default(),
            numeric_expressions: ProjectNumericExpressions::default(),
            texture_assets: Vec::new(),
            saved_revision: None,
            saved_document: None,
        };
        // The built-in startup sheet is a clean baseline. In contrast, a
        // user-created project uses `new_unsaved` and remains dirty until its
        // first successful save.
        project.saved_document = Some(project.document());
        project
    }

    fn new_unsaved(name: String, pattern: CreasePattern, paper: Paper) -> Self {
        let editor = EditorState::with_paper(pattern, paper);
        Self {
            instance_id: ProjectId::new(),
            project_id: ProjectId::new(),
            name,
            current_path: None,
            editor,
            applied_pose_authority: CurrentAppliedPoseAuthority::default(),
            numeric_expressions: ProjectNumericExpressions::default(),
            texture_assets: Vec::new(),
            saved_revision: None,
            saved_document: None,
        }
    }

    #[cfg(test)]
    fn from_document(mut document: ProjectDocument, current_path: PathBuf) -> Self {
        if document.thumbnail_svg.is_none() {
            document.thumbnail_svg = generate_project_thumbnail_svg(&document).ok();
        }
        let mut saved_document = document.clone();
        saved_document.numeric_expressions.undo_stack.clear();
        saved_document.numeric_expressions.redo_stack.clear();
        saved_document.numeric_expressions.vertex_undo_stack.clear();
        saved_document.numeric_expressions.vertex_redo_stack.clear();
        let numeric_expressions = document.numeric_expressions;
        let texture_assets = document.texture_assets;
        let editor = EditorState::with_all_document_parts_and_memo(
            document.crease_pattern,
            document.paper,
            document.instruction_timeline,
            document.geometric_constraints,
            document.layers,
            document.element_metadata,
            document.memo,
        );
        Self {
            instance_id: ProjectId::new(),
            project_id: document.project_id,
            name: document.name,
            current_path: Some(current_path),
            saved_revision: Some(editor.revision()),
            applied_pose_authority: CurrentAppliedPoseAuthority::default(),
            numeric_expressions,
            texture_assets,
            saved_document: Some(saved_document),
            editor,
        }
    }

    fn from_project_archive(
        project: Ori2ProjectArchive,
        current_path: PathBuf,
    ) -> Result<Self, String> {
        let history_lengths = project
            .editor_history
            .as_ref()
            .map(|history| (history.undo_len(), history.redo_len()))
            .unwrap_or_default();
        let editor = restore_archive_editor(&project)
            .map_err(|_| PROJECT_ARCHIVE_INVALID_MESSAGE.to_owned())?;
        let mut document = project.document;
        let persisted_pose = document.current_pose.clone();
        if document.thumbnail_svg.is_none() {
            document.thumbnail_svg = generate_project_thumbnail_svg(&document).ok();
        }
        normalize_numeric_expression_history(
            &mut document.numeric_expressions,
            history_lengths.0,
            history_lengths.1,
        )
        .map_err(|_| PROJECT_ARCHIVE_INVALID_MESSAGE.to_owned())?;
        let mut saved_document = document.clone();
        saved_document.numeric_expressions.undo_stack.clear();
        saved_document.numeric_expressions.redo_stack.clear();
        saved_document.numeric_expressions.vertex_undo_stack.clear();
        saved_document.numeric_expressions.vertex_redo_stack.clear();
        let texture_assets = document.texture_assets.clone();
        let mut restored = Self {
            instance_id: ProjectId::new(),
            project_id: document.project_id,
            name: document.name,
            current_path: Some(current_path),
            saved_revision: Some(editor.revision()),
            applied_pose_authority: CurrentAppliedPoseAuthority::default(),
            numeric_expressions: document.numeric_expressions,
            texture_assets,
            saved_document: Some(saved_document),
            editor,
        };
        if let Some(pose) = persisted_pose.as_ref() {
            restore_persisted_current_pose(&mut restored, pose)
                .map_err(|_| PROJECT_ARCHIVE_INVALID_MESSAGE.to_owned())?;
        }
        Ok(restored)
    }

    fn from_recovery_project_archive(project: Ori2ProjectArchive) -> Result<Self, ()> {
        let history_lengths = project
            .editor_history
            .as_ref()
            .map(|history| (history.undo_len(), history.redo_len()))
            .unwrap_or_default();
        let editor = restore_archive_editor(&project)?;
        let mut document = project.document;
        let persisted_pose = document.current_pose.clone();
        if document.thumbnail_svg.is_none() {
            document.thumbnail_svg = generate_project_thumbnail_svg(&document).ok();
        }
        normalize_numeric_expression_history(
            &mut document.numeric_expressions,
            history_lengths.0,
            history_lengths.1,
        )?;
        let texture_assets = document.texture_assets.clone();
        let mut restored = Self {
            instance_id: ProjectId::new(),
            project_id: document.project_id,
            name: document.name,
            current_path: None,
            saved_revision: None,
            applied_pose_authority: CurrentAppliedPoseAuthority::default(),
            numeric_expressions: document.numeric_expressions,
            texture_assets,
            saved_document: None,
            editor,
        };
        if let Some(pose) = persisted_pose.as_ref() {
            restore_persisted_current_pose(&mut restored, pose).map_err(|_| ())?;
        }
        Ok(restored)
    }

    fn document(&self) -> ProjectDocument {
        let numeric_expressions = ProjectNumericExpressions {
            rectangular_paper_creation: self.numeric_expressions.rectangular_paper_creation.clone(),
            vertex_coordinates: self.numeric_expressions.vertex_coordinates.clone(),
            ..ProjectNumericExpressions::default()
        };
        let mut document = ProjectDocument {
            format_version: CURRENT_FORMAT_VERSION,
            project_id: self.project_id,
            name: self.name.clone(),
            memo: self.editor.project_memo().to_owned(),
            thumbnail_svg: None,
            current_pose: current_pose_document(&self.editor),
            paper: self.editor.paper().clone(),
            crease_pattern: self.editor.pattern().clone(),
            instruction_timeline: self.editor.instruction_timeline().clone(),
            numeric_expressions,
            geometric_constraints: self.editor.geometric_constraints().clone(),
            layers: self.editor.project_layers().clone(),
            element_metadata: self.editor.element_metadata().clone(),
            texture_assets: self.texture_assets.clone(),
        };
        document.thumbnail_svg = generate_project_thumbnail_svg(&document).ok();
        document
    }

    fn project_archive(&self) -> Result<Ori2ProjectArchive, String> {
        let mut document = self.document();
        document.numeric_expressions = self.numeric_expressions.clone();
        let history = self
            .editor
            .export_history_v1(self.project_id)
            .map_err(|_| PROJECT_SERIALIZATION_FAILED_MESSAGE.to_owned())?;
        trim_expression_stack(
            &mut document.numeric_expressions.undo_stack,
            history.undo_len(),
        );
        trim_expression_stack(
            &mut document.numeric_expressions.redo_stack,
            history.redo_len(),
        );
        trim_expression_stack(
            &mut document.numeric_expressions.vertex_undo_stack,
            history.undo_len(),
        );
        trim_expression_stack(
            &mut document.numeric_expressions.vertex_redo_stack,
            history.redo_len(),
        );
        normalize_numeric_expression_history(
            &mut document.numeric_expressions,
            history.undo_len(),
            history.redo_len(),
        )
        .map_err(|_| PROJECT_SERIALIZATION_FAILED_MESSAGE.to_owned())?;
        Ok(Ori2ProjectArchive {
            document,
            editor_history: (!history.is_default_empty()).then_some(history),
        })
    }

    fn is_dirty(&self) -> bool {
        let Some(saved) = &self.saved_document else {
            return true;
        };
        saved.format_version != CURRENT_FORMAT_VERSION
            || saved.project_id != self.project_id
            || saved.name != self.name
            || saved.memo != self.editor.project_memo()
            || saved.current_pose != current_pose_document(&self.editor)
            || saved.paper != *self.editor.paper()
            || saved.crease_pattern != *self.editor.pattern()
            || saved.instruction_timeline != *self.editor.instruction_timeline()
            || saved.numeric_expressions.rectangular_paper_creation
                != self.numeric_expressions.rectangular_paper_creation
            || saved.numeric_expressions.vertex_coordinates
                != self.numeric_expressions.vertex_coordinates
            || saved.geometric_constraints != *self.editor.geometric_constraints()
            || saved.layers != *self.editor.project_layers()
            || saved.element_metadata != *self.editor.element_metadata()
            || saved.texture_assets != self.texture_assets
    }

    fn record_numeric_expression_edit(&mut self) {
        self.numeric_expressions
            .undo_stack
            .push(self.numeric_expressions.rectangular_paper_creation.clone());
        let limit = self.editor.history_entry_limit();
        if self.numeric_expressions.undo_stack.len() > limit {
            let excess = self.numeric_expressions.undo_stack.len() - limit;
            self.numeric_expressions.undo_stack.drain(..excess);
        }
        self.numeric_expressions.redo_stack.clear();
        self.numeric_expressions.vertex_undo_stack.push(None);
        if self.numeric_expressions.vertex_undo_stack.len() > limit {
            let excess = self.numeric_expressions.vertex_undo_stack.len() - limit;
            self.numeric_expressions.vertex_undo_stack.drain(..excess);
        }
        self.numeric_expressions.vertex_redo_stack.clear();
    }

    fn undo_numeric_expression_edit(&mut self) {
        let Some(previous) = self.numeric_expressions.undo_stack.pop() else {
            return;
        };
        self.numeric_expressions
            .redo_stack
            .push(self.numeric_expressions.rectangular_paper_creation.take());
        self.numeric_expressions.rectangular_paper_creation = previous;
        let vertex_transition = self.numeric_expressions.vertex_undo_stack.pop().flatten();
        if let Some(transition) = vertex_transition {
            for change in &transition.changes {
                apply_vertex_expression_binding(
                    &mut self.numeric_expressions.vertex_coordinates,
                    change.vertex,
                    change.before.clone(),
                );
            }
            self.numeric_expressions
                .vertex_redo_stack
                .push(Some(transition));
        } else {
            self.numeric_expressions.vertex_redo_stack.push(None);
        }
    }

    fn redo_numeric_expression_edit(&mut self) {
        let Some(next) = self.numeric_expressions.redo_stack.pop() else {
            return;
        };
        self.numeric_expressions
            .undo_stack
            .push(self.numeric_expressions.rectangular_paper_creation.take());
        self.numeric_expressions.rectangular_paper_creation = next;
        let vertex_transition = self.numeric_expressions.vertex_redo_stack.pop().flatten();
        if let Some(transition) = vertex_transition {
            for change in &transition.changes {
                apply_vertex_expression_binding(
                    &mut self.numeric_expressions.vertex_coordinates,
                    change.vertex,
                    change.after.clone(),
                );
            }
            self.numeric_expressions
                .vertex_undo_stack
                .push(Some(transition));
        } else {
            self.numeric_expressions.vertex_undo_stack.push(None);
        }
    }

    fn adopt_vertex_coordinate_expression(&mut self, binding: VertexCoordinateExpressions) {
        let before = self
            .numeric_expressions
            .vertex_coordinates
            .iter()
            .find(|current| current.vertex == binding.vertex)
            .cloned();
        let vertex = binding.vertex;
        apply_vertex_expression_binding(
            &mut self.numeric_expressions.vertex_coordinates,
            vertex,
            Some(binding.clone()),
        );
        self.record_vertex_expression_change(vertex, before, Some(binding));
    }

    fn remove_vertex_coordinate_expression(&mut self, vertex: VertexId) {
        let before = self
            .numeric_expressions
            .vertex_coordinates
            .iter()
            .find(|current| current.vertex == vertex)
            .cloned();
        apply_vertex_expression_binding(
            &mut self.numeric_expressions.vertex_coordinates,
            vertex,
            None,
        );
        if before.is_some() {
            self.record_vertex_expression_change(vertex, before, None);
        }
    }

    fn record_vertex_expression_change(
        &mut self,
        vertex: VertexId,
        before: Option<VertexCoordinateExpressions>,
        after: Option<VertexCoordinateExpressions>,
    ) {
        let Some(slot) = self.numeric_expressions.vertex_undo_stack.last_mut() else {
            return;
        };
        let transition = slot.get_or_insert_with(|| VertexCoordinateExpressionTransition {
            changes: Vec::new(),
        });
        if let Some(existing) = transition
            .changes
            .iter_mut()
            .find(|change| change.vertex == vertex)
        {
            existing.after = after;
        } else {
            transition.changes.push(VertexCoordinateExpressionChange {
                vertex,
                before,
                after,
            });
            transition
                .changes
                .sort_by_key(|change| change.vertex.canonical_bytes());
        }
    }

    fn reconcile_vertex_coordinate_expressions(&mut self) {
        let stale = self
            .numeric_expressions
            .vertex_coordinates
            .iter()
            .filter(|binding| {
                self.editor
                    .pattern()
                    .vertices
                    .iter()
                    .find(|vertex| vertex.id == binding.vertex)
                    .is_none_or(|vertex| {
                        vertex.position.x.to_bits() != binding.adopted_x_mm.to_bits()
                            || vertex.position.y.to_bits() != binding.adopted_y_mm.to_bits()
                    })
            })
            .map(|binding| binding.vertex)
            .collect::<Vec<_>>();
        for vertex in stale {
            self.remove_vertex_coordinate_expression(vertex);
        }
    }

    fn trim_numeric_expression_history(&mut self, limit: usize) {
        trim_expression_stack(&mut self.numeric_expressions.undo_stack, limit);
        trim_expression_stack(&mut self.numeric_expressions.redo_stack, limit);
        trim_expression_stack(&mut self.numeric_expressions.vertex_undo_stack, limit);
        trim_expression_stack(&mut self.numeric_expressions.vertex_redo_stack, limit);
    }
}

fn current_pose_document(editor: &EditorState) -> Option<InstructionPose> {
    let pose = editor.current_applied_pose()?;
    Some(InstructionPose {
        model: InstructionPoseModel::AbsoluteHingeAnglesV1,
        source_model_fingerprint: editor.fold_model_fingerprint_v1(),
        fixed_face: pose.fixed_face(),
        hinge_angles: pose
            .hinge_angles()
            .iter()
            .map(|hinge| InstructionHingeAngle {
                edge: hinge.edge(),
                angle_degrees: hinge.angle_degrees(),
            })
            .collect(),
    })
}

fn trim_expression_stack<T>(stack: &mut Vec<T>, limit: usize) {
    if stack.len() > limit {
        let excess = stack.len() - limit;
        stack.drain(..excess);
    }
}

fn normalize_numeric_expression_history(
    expressions: &mut ProjectNumericExpressions,
    undo_len: usize,
    redo_len: usize,
) -> Result<(), ()> {
    if expressions.rectangular_paper_creation.is_none()
        && expressions.vertex_coordinates.is_empty()
        && expressions.undo_stack.is_empty()
        && expressions.redo_stack.is_empty()
        && expressions.vertex_undo_stack.is_empty()
        && expressions.vertex_redo_stack.is_empty()
    {
        return Ok(());
    }
    if expressions.undo_stack.len() > undo_len
        || expressions.redo_stack.len() > redo_len
        || expressions.vertex_undo_stack.len() > undo_len
        || expressions.vertex_redo_stack.len() > redo_len
    {
        return Err(());
    }
    prepend_expression_history_defaults(
        &mut expressions.undo_stack,
        undo_len,
        expressions.rectangular_paper_creation.clone(),
    );
    prepend_expression_history_defaults(
        &mut expressions.redo_stack,
        redo_len,
        expressions.rectangular_paper_creation.clone(),
    );
    prepend_expression_history_defaults(&mut expressions.vertex_undo_stack, undo_len, None);
    prepend_expression_history_defaults(&mut expressions.vertex_redo_stack, redo_len, None);
    Ok(())
}

fn prepend_expression_history_defaults<T: Clone>(stack: &mut Vec<T>, len: usize, value: T) {
    let missing = len.saturating_sub(stack.len());
    if missing > 0 {
        stack.splice(0..0, std::iter::repeat_n(value, missing));
    }
}

fn apply_vertex_expression_binding(
    bindings: &mut Vec<VertexCoordinateExpressions>,
    vertex: VertexId,
    value: Option<VertexCoordinateExpressions>,
) {
    bindings.retain(|binding| binding.vertex != vertex);
    if let Some(value) = value {
        bindings.push(value);
        bindings.sort_by_key(|binding| binding.vertex.canonical_bytes());
    }
}

fn restore_archive_editor(project: &Ori2ProjectArchive) -> Result<EditorState, ()> {
    let editor = match &project.editor_history {
        Some(history) => {
            if history.project_id() != project.document.project_id {
                return Err(());
            }
            EditorState::with_all_document_parts_memo_and_history_v1(
                project.document.crease_pattern.clone(),
                project.document.paper.clone(),
                project.document.instruction_timeline.clone(),
                project.document.geometric_constraints.clone(),
                project.document.layers.clone(),
                project.document.element_metadata.clone(),
                project.document.memo.clone(),
                history.clone(),
            )
            .map_err(|_| ())
        }
        None => Ok(EditorState::with_all_document_parts_and_memo(
            project.document.crease_pattern.clone(),
            project.document.paper.clone(),
            project.document.instruction_timeline.clone(),
            project.document.geometric_constraints.clone(),
            project.document.layers.clone(),
            project.document.element_metadata.clone(),
            project.document.memo.clone(),
        )),
    }?;
    validate_reachable_history_instruction_poses(&project.document, &editor)?;
    Ok(editor)
}

fn validate_reachable_history_instruction_poses(
    document: &ProjectDocument,
    editor: &EditorState,
) -> Result<(), ()> {
    fn validate_endpoint(document: &ProjectDocument, editor: &EditorState) -> Result<(), ()> {
        let mut endpoint = document.clone();
        endpoint.paper = editor.paper().clone();
        endpoint.crease_pattern = editor.pattern().clone();
        endpoint.instruction_timeline = editor.instruction_timeline().clone();
        endpoint.geometric_constraints = editor.geometric_constraints().clone();
        endpoint.layers = editor.project_layers().clone();
        validate_document_instruction_poses(&endpoint).map_err(|_| ())
    }

    validate_endpoint(document, editor)?;

    // Editor history is bounded to 128 entries per stack. Keep an explicit
    // traversal fence here as defense in depth if an internal constructor is
    // ever changed independently from the persisted-history validator.
    let mut undo_cursor = editor.clone();
    let mut undo_count = 0_usize;
    while undo_cursor.can_undo() {
        if undo_count == MAX_EDITOR_HISTORY_ENTRIES {
            return Err(());
        }
        undo_cursor.undo(undo_cursor.revision()).map_err(|_| ())?;
        validate_endpoint(document, &undo_cursor)?;
        undo_count += 1;
    }

    let mut redo_cursor = editor.clone();
    let mut redo_count = 0_usize;
    while redo_cursor.can_redo() {
        if redo_count == MAX_EDITOR_HISTORY_ENTRIES {
            return Err(());
        }
        redo_cursor.redo(redo_cursor.revision()).map_err(|_| ())?;
        validate_endpoint(document, &redo_cursor)?;
        redo_count += 1;
    }
    Ok(())
}

fn initial_project_state() -> ProjectState {
    let sheet = create_rectangular_sheet(DEFAULT_SHEET_SIZE_MM, DEFAULT_SHEET_SIZE_MM, false)
        .expect("the built-in default sheet dimensions must be valid");
    let (pattern, paper) = sheet.into_parts();
    ProjectState::new_with_paper(pattern, paper)
}

#[derive(Debug, Clone, PartialEq, Serialize)]
struct PatternResponse {
    requested_edge_count: usize,
    vertex_count: usize,
    edge_count: usize,
    vertices: Vec<BenchmarkVertex>,
    edges: Vec<BenchmarkEdge>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
struct BenchmarkVertex {
    id: String,
    position: Point2,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
struct BenchmarkEdge {
    id: String,
    start: String,
    end: String,
    kind: EdgeKind,
}

#[derive(Debug, Serialize)]
struct ProjectSnapshot {
    project_instance_id: ProjectId,
    project_id: ProjectId,
    name: String,
    memo: String,
    current_path: Option<String>,
    revision: u64,
    saved_revision: Option<u64>,
    is_dirty: bool,
    paper: Paper,
    crease_pattern: CreasePattern,
    instruction_timeline: InstructionTimeline,
    numeric_expressions: ProjectNumericExpressions,
    geometric_constraints: GeometricConstraintDocumentV1,
    project_layers: ProjectLayerDocumentV1,
    element_metadata: ori_domain::ElementMetadataDocumentV1,
    fold_model_fingerprint: String,
    can_undo: bool,
    can_redo: bool,
    cutting_allowed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum EdgeOrientationConstraint {
    Horizontal,
    Vertical,
}

#[derive(Debug, Serialize)]
struct GeometricConstraintPreflightResponse {
    project_instance_id: ProjectId,
    project_id: ProjectId,
    revision: u64,
    result: GeometricConstraintPreflightResult,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum GeometricConstraintPreflightResult {
    DirectConflict {
        conflicts: Vec<DirectConstraintConflictV1>,
    },
    NoDirectConflict,
    Unknown {
        reason: GeometricConstraintUnknownReason,
        unchecked_constraint_ids: Vec<ConstraintId>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum GeometricConstraintUnknownReason {
    WorkLimitExceeded,
    SolverRequiredConstraintKinds,
    InvalidDocumentOrGeometry,
}

#[derive(Debug, Serialize)]
struct ProjectFileResponse {
    canceled: bool,
    project: ProjectSnapshot,
}

#[derive(Debug, Serialize)]
struct FoldImportPreviewResponse {
    canceled: bool,
    preview: Option<FoldImportPreviewSnapshot>,
}

#[derive(Debug, Serialize)]
struct FoldImportPreviewSnapshot {
    import_id: ProjectId,
    file_name: &'static str,
    suggested_name: String,
    file_spec: Option<String>,
    frame_unit: Option<String>,
    default_mm_per_unit: Option<f64>,
    vertex_count: usize,
    edge_count: usize,
    boundary_edge_count: usize,
    boundary_candidates: Vec<FoldImportBoundaryCandidateSnapshot>,
    fixed_boundary_candidate_id: Option<u16>,
    assignments: Vec<FoldImportAssignmentSummary>,
    preview_vertices: Vec<FoldImportPreviewVertex>,
    preview_edges: Vec<FoldImportPreviewEdge>,
    preview_truncated: bool,
    warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct FoldImportBoundaryCandidateSnapshot {
    id: u16,
    source: &'static str,
    edge_indices: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct FoldImportAssignmentSummary {
    assignment: String,
    count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
struct FoldImportPreviewVertex {
    x: f64,
    y: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct FoldImportPreviewEdge {
    source_index: usize,
    start: usize,
    end: usize,
    assignment: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct FoldImportAssignmentMappingRequest {
    source: String,
    target: FoldImportTargetRequest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum FoldImportTargetRequest {
    Mountain,
    Valley,
    Auxiliary,
    Cut,
    Ignore,
}

#[derive(Debug, Serialize)]
struct SvgImportPreviewResponse {
    canceled: bool,
    preview: Option<SvgImportPreviewSnapshot>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
struct SvgImportSettingsValidationResponse {
    validation_id: ProjectId,
    preview_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    millimeters_per_unit: f64,
    boundary_candidate_id: Option<u16>,
    width_mm: f64,
    height_mm: f64,
    has_cuts: bool,
}

#[derive(Debug, Serialize)]
struct SvgImportPreviewSnapshot {
    import_id: ProjectId,
    file_name: &'static str,
    suggested_name: String,
    default_mm_per_unit: Option<f64>,
    root_view_box: Option<SvgRootViewBox>,
    root_physical_size: SvgRootPhysicalSize,
    source_segment_count: usize,
    style_groups: Vec<SvgImportStyleGroupSnapshot>,
    boundary_candidates: Vec<SvgBoundaryCandidateSnapshot>,
    preview_vertices: Vec<SvgImportPreviewVertex>,
    preview_edges: Vec<SvgImportPreviewEdge>,
    preview_truncated: bool,
    warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
struct SvgImportStyleGroupSnapshot {
    group_id: u16,
    element_count: usize,
    segment_count: usize,
    stroke: Option<String>,
    stroke_color: Option<String>,
    dash_array: Option<String>,
    line_cap: SvgLineCap,
    classes: Vec<String>,
    layer: Option<String>,
    representative_id: Option<String>,
    semantic_hint: Option<SvgImportTargetRequest>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
struct SvgBoundaryCandidateSnapshot {
    candidate_id: u16,
    kind: &'static str,
    segment_count: usize,
    width: f64,
    height: f64,
    vertices: Vec<SvgImportPreviewVertex>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
struct SvgImportPreviewVertex {
    x: f64,
    y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
struct SvgImportPreviewEdge {
    start: usize,
    end: usize,
    group_id: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SvgImportStyleMappingRequest {
    group_id: u16,
    target: SvgImportTargetRequest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum SvgImportTargetRequest {
    Boundary,
    Mountain,
    Valley,
    Auxiliary,
    Cut,
    Ignore,
}

struct LoadedProjectFile {
    replacement: ProjectState,
}

impl std::fmt::Debug for LoadedProjectFile {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LoadedProjectFile")
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Serialize)]
struct EdgeIntersectionResponse {
    snapshot: ProjectSnapshot,
    vertex_id: VertexId,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
enum IntersectionClusterRelation {
    Interior,
    Endpoint,
}

const MIN_INTERSECTION_CLUSTER_TARGETS: usize = 3;
const MAX_INTERSECTION_CLUSTER_TARGETS: usize = 64;

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IntersectionClusterTargetRequest {
    edge_id: EdgeId,
    relation: IntersectionClusterRelation,
}

#[derive(Debug, Serialize)]
struct TJunctionResponse {
    snapshot: ProjectSnapshot,
    vertex_id: VertexId,
}

#[derive(Debug, Serialize)]
struct ValidationSnapshot {
    project_id: ProjectId,
    revision: u64,
    is_valid: bool,
    issues: Vec<ValidationIssueSnapshot>,
    local_flat_foldability: LocalFlatFoldabilityReport,
}

struct ValidationAnalysisInput {
    project_instance_id: ProjectId,
    project_id: ProjectId,
    source: TopologyAnalysisInput,
}

struct AnalyzedProjectValidation {
    input: ValidationAnalysisInput,
    source_model_fingerprint: String,
    snapshot: ValidationSnapshot,
}

#[derive(Debug, Serialize)]
struct ValidationIssueSnapshot {
    code: &'static str,
    vertices: Vec<VertexId>,
    edges: Vec<EdgeId>,
}

#[derive(Debug, Serialize)]
struct ProjectTopologyResponse {
    project_id: ProjectId,
    revision: u64,
    /// Strict gate for folding consumers. A false response never carries a
    /// snapshot, even if analysis later gains partial diagnostic snapshots.
    simulation_ready: bool,
    snapshot: Option<TopologySnapshot>,
    issues: Vec<TopologyIssue>,
}

struct NewProjectParameters {
    name: String,
    width_expression: String,
    height_expression: String,
    /// Certified native values adopted from the two expressions before the
    /// project mutex is acquired. These fields never cross the IPC boundary.
    width_mm: f64,
    height_mm: f64,
    thickness_mm: f64,
    cutting_allowed: bool,
    front_color: RgbaColor,
    back_color: RgbaColor,
}

#[tauri::command]
fn generate_benchmark_pattern(edge_count: usize) -> PatternResponse {
    let edge_count = edge_count.min(MAX_BENCHMARK_EDGE_COUNT);
    if edge_count == 0 {
        return PatternResponse {
            requested_edge_count: edge_count,
            vertex_count: 0,
            edge_count: 0,
            vertices: Vec::new(),
            edges: Vec::new(),
        };
    }

    // Keep the payload independent from the open project and its undo history.
    // Stable index-based IDs also make native and browser benchmark fixtures
    // structurally comparable without leaking random domain IDs into metrics.
    let mut side = ((edge_count as f64 / 2.0).sqrt().ceil() as usize).max(2);
    while 2 * side * (side - 1) < edge_count {
        side += 1;
    }

    let vertices = (0..side * side)
        .map(|index| BenchmarkVertex {
            id: benchmark_vertex_id(index),
            position: Point2::new((index % side) as f64, (index / side) as f64),
        })
        .collect::<Vec<_>>();

    let mut edges = Vec::with_capacity(edge_count);
    'grid: for y in 0..side {
        for x in 0..side {
            let index = y * side + x;
            if x + 1 < side {
                edges.push(BenchmarkEdge {
                    id: benchmark_edge_id(edges.len()),
                    start: benchmark_vertex_id(index),
                    end: benchmark_vertex_id(index + 1),
                    kind: if y % 2 == 0 {
                        EdgeKind::Mountain
                    } else {
                        EdgeKind::Valley
                    },
                });
                if edges.len() == edge_count {
                    break 'grid;
                }
            }
            if y + 1 < side {
                edges.push(BenchmarkEdge {
                    id: benchmark_edge_id(edges.len()),
                    start: benchmark_vertex_id(index),
                    end: benchmark_vertex_id(index + side),
                    kind: if x % 2 == 0 {
                        EdgeKind::Valley
                    } else {
                        EdgeKind::Mountain
                    },
                });
                if edges.len() == edge_count {
                    break 'grid;
                }
            }
        }
    }

    PatternResponse {
        requested_edge_count: edge_count,
        vertex_count: vertices.len(),
        edge_count: edges.len(),
        vertices,
        edges,
    }
}

fn benchmark_vertex_id(index: usize) -> String {
    format!("benchmark-v-{index}")
}

fn benchmark_edge_id(index: usize) -> String {
    format!("benchmark-e-{index}")
}

#[tauri::command]
fn project_snapshot(state: State<'_, AppState>) -> Result<ProjectSnapshot, String> {
    let project = lock_project(&state)?;
    Ok(snapshot(&project))
}

#[tauri::command]
fn update_project_memo(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    memo: String,
) -> Result<ProjectSnapshot, String> {
    const MAX_PROJECT_MEMO_CHARS: usize = 16_000;
    if memo.chars().count() > MAX_PROJECT_MEMO_CHARS
        || memo
            .chars()
            .any(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
    {
        return Err("project memo must contain at most 16000 printable characters".to_owned());
    }
    let mut project = lock_project(&state)?;
    execute_command(
        &mut project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        Command::UpdateProjectMemo { memo },
    )
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
async fn new_project(
    state: State<'_, AppState>,
    recovery: State<'_, RecoveryRuntime>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    name: String,
    width_expression: String,
    height_expression: String,
    thickness_mm: f64,
    cutting_allowed: bool,
    front_color: RgbaColor,
    back_color: RgbaColor,
) -> Result<ProjectSnapshot, String> {
    let (width_mm, height_mm) = evaluate_positive_millimetre_pair_in_worker(
        width_expression.clone(),
        height_expression.clone(),
    )
    .await
    .map_err(|error| error.user_input_message().to_owned())?;
    let mut project = lock_project(&state)?;
    let response = replace_with_new_project(
        &mut project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        NewProjectParameters {
            name,
            width_expression,
            height_expression,
            width_mm,
            height_mm,
            thickness_mm,
            cutting_allowed,
            front_color,
            back_color,
        },
    )?;
    drop(project);
    let _ = recovery.clear_after_normal_completion(&state, &response);
    Ok(response)
}

#[tauri::command]
async fn validate_project(state: State<'_, AppState>) -> Result<ValidationSnapshot, String> {
    validate_project_with_worker(&state, |input| Ok(analyze_validation_input(input))).await
}

#[tauri::command]
async fn apply_current_native_pose(
    state: State<'_, AppState>,
    request: NativePoseRequest,
) -> Result<ApplyCurrentNativePoseResponse, String> {
    apply_current_native_pose_authority(&state, request).await
}

/// Read-only native diagnosis. Geometry work runs without the project or pose
/// lock, and the response contains fixed categories plus face IDs only.
#[tauri::command]
async fn inspect_current_static_collision(
    state: State<'_, AppState>,
) -> Result<CurrentStaticCollisionDiagnosticResponse, String> {
    inspect_current_static_collision_authority(&state).await
}

#[tauri::command]
async fn analyze_geometric_constraints(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
) -> Result<GeometricConstraintPreflightResponse, String> {
    analyze_geometric_constraints_with_worker(
        &state,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        |pattern, document| Ok(analyze_geometric_constraint_document(&pattern, &document)),
    )
    .await
}

#[derive(Clone, Copy)]
struct GeometricConstraintAnalysisBinding {
    project_instance_id: ProjectId,
    project_id: ProjectId,
    revision: u64,
}

struct GeometricConstraintAnalysisInput {
    binding: GeometricConstraintAnalysisBinding,
    pattern: CreasePattern,
    document: GeometricConstraintDocumentV1,
}

async fn analyze_geometric_constraints_with_worker<F>(
    state: &AppState,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    worker: F,
) -> Result<GeometricConstraintPreflightResponse, String>
where
    F: FnOnce(
            CreasePattern,
            GeometricConstraintDocumentV1,
        ) -> Result<GeometricConstraintPreflightResult, String>
        + Send
        + 'static,
{
    let permit = state
        .try_acquire_geometric_constraint_worker()
        .ok_or_else(|| GEOMETRIC_CONSTRAINT_ANALYSIS_BUSY_MESSAGE.to_owned())?;
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
        worker(input.pattern, input.document)
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
    ensure_expected_project(
        project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
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
    result: GeometricConstraintPreflightResult,
) -> Result<GeometricConstraintPreflightResponse, String> {
    ensure_expected_project(
        project,
        binding.project_instance_id,
        binding.project_id,
        binding.revision,
    )?;
    Ok(GeometricConstraintPreflightResponse {
        project_instance_id: binding.project_instance_id,
        project_id: binding.project_id,
        revision: binding.revision,
        result,
    })
}

fn analyze_geometric_constraint_document(
    pattern: &CreasePattern,
    document: &GeometricConstraintDocumentV1,
) -> GeometricConstraintPreflightResult {
    if document.schema_version == ori_domain::GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1
        && document.is_empty()
    {
        return GeometricConstraintPreflightResult::NoDirectConflict;
    }

    let Ok(prepared) =
        prepare_geometric_constraints_v1(pattern, document, GeometricConstraintLimitsV1::default())
    else {
        let mut unchecked_constraint_ids = document
            .constraints
            .iter()
            .map(|record| record.id)
            .collect::<Vec<_>>();
        unchecked_constraint_ids.sort_unstable_by_key(ConstraintId::canonical_bytes);
        return GeometricConstraintPreflightResult::Unknown {
            reason: GeometricConstraintUnknownReason::InvalidDocumentOrGeometry,
            unchecked_constraint_ids,
        };
    };

    match prepared.preflight() {
        ConstraintPreflightV1::DirectConflict { conflicts } => {
            GeometricConstraintPreflightResult::DirectConflict { conflicts }
        }
        ConstraintPreflightV1::NoDirectConflict => {
            GeometricConstraintPreflightResult::NoDirectConflict
        }
        ConstraintPreflightV1::Unknown {
            reason,
            unchecked_constraint_ids,
        } => GeometricConstraintPreflightResult::Unknown {
            reason: match reason {
                GeometricConstraintUnknownReasonV1::WorkLimitExceeded => {
                    GeometricConstraintUnknownReason::WorkLimitExceeded
                }
                GeometricConstraintUnknownReasonV1::SolverRequiredConstraintKinds => {
                    GeometricConstraintUnknownReason::SolverRequiredConstraintKinds
                }
            },
            unchecked_constraint_ids,
        },
    }
}

const VALIDATION_ANALYSIS_FAILED_MESSAGE: &str =
    "検証処理を完了できませんでした。もう一度実行してください。";

async fn validate_project_with_worker<F>(
    state: &AppState,
    worker: F,
) -> Result<ValidationSnapshot, String>
where
    F: FnOnce(ValidationAnalysisInput) -> Result<AnalyzedProjectValidation, String>
        + Send
        + 'static,
{
    let input = {
        let project = lock_project(state)?;
        capture_validation_input(&project)
    };
    let analyzed = tauri::async_runtime::spawn_blocking(move || worker(input))
        .await
        .map_err(|_| VALIDATION_ANALYSIS_FAILED_MESSAGE.to_owned())?
        .map_err(|_| VALIDATION_ANALYSIS_FAILED_MESSAGE.to_owned())?;

    let project = lock_project(state)?;
    finish_validation_snapshot(&project, analyzed)
}

/// Analyzes immutable topology input away from the project-state lock.
///
/// Unsupported or invalid folding geometry is a successful command response
/// with structured issues. Operational failures and stale results are command
/// errors, so the UI cannot accidentally display topology from another edit.
#[tauri::command]
async fn analyze_project_topology(
    state: State<'_, AppState>,
    expected_project_id: ProjectId,
    expected_revision: u64,
) -> Result<ProjectTopologyResponse, String> {
    let input = {
        let project = lock_project(&state)?;
        capture_topology_input(&project, expected_project_id, expected_revision)?
    };
    let (input, topology) = tauri::async_runtime::spawn_blocking(move || {
        let topology = input.analyze();
        (input, topology)
    })
    .await
    .map_err(topology_analysis_task_error)?;

    let project = lock_project(&state)?;
    finish_topology_response(&project, &input, topology)
}

#[tauri::command]
async fn open_project(
    app: AppHandle,
    state: State<'_, AppState>,
    recovery: State<'_, RecoveryRuntime>,
) -> Result<ProjectFileResponse, String> {
    let (expected_instance_id, expected_project_id, expected_revision, initial_directory) = {
        let project = lock_project(&state)?;
        (
            project.instance_id,
            project.project_id,
            project.editor.revision(),
            project
                .current_path
                .as_deref()
                .and_then(Path::parent)
                .map(Path::to_path_buf),
        )
    };

    let mut dialog = app
        .dialog()
        .file()
        .add_filter("ORIGAMI2 project", &["ori2"])
        .set_title("Open ORIGAMI2 project");
    if let Some(directory) = initial_directory {
        dialog = dialog.set_directory(directory);
    }

    let Some(selected) = dialog.blocking_pick_file() else {
        return canceled_file_response(&state);
    };
    let path = selected
        .simplified()
        .into_path()
        .map_err(|_| "選択されたファイルはローカルファイルではありません。".to_owned())?;
    let loaded = tauri::async_runtime::spawn_blocking(move || load_project_file(path))
        .await
        .map_err(|_| PROJECT_OPEN_TASK_FAILED_MESSAGE.to_owned())??;

    let mut project = lock_project(&state)?;
    let response = apply_loaded_project_file(
        &mut project,
        expected_instance_id,
        expected_project_id,
        expected_revision,
        loaded,
    )?;
    drop(project);
    let _ = recovery.clear_after_normal_completion(&state, &response.project);
    Ok(response)
}

#[tauri::command]
async fn save_project(
    app: AppHandle,
    state: State<'_, AppState>,
    recovery: State<'_, RecoveryRuntime>,
) -> Result<ProjectFileResponse, String> {
    let saved_to_current_path = {
        let mut project = lock_project(&state)?;
        if let Some(path) = project.current_path.clone() {
            Some(save_project_to_path(&mut project, path)?)
        } else {
            None
        }
    };
    if let Some(response) = saved_to_current_path {
        let _ = recovery.clear_after_normal_completion(&state, &response.project);
        return Ok(response);
    }
    let response = save_project_with_dialog(&app, &state)?;
    if !response.canceled {
        let _ = recovery.clear_after_normal_completion(&state, &response.project);
    }
    Ok(response)
}

#[tauri::command]
async fn save_project_as(
    app: AppHandle,
    state: State<'_, AppState>,
    recovery: State<'_, RecoveryRuntime>,
) -> Result<ProjectFileResponse, String> {
    let response = save_project_with_dialog(&app, &state)?;
    if !response.canceled {
        let _ = recovery.clear_after_normal_completion(&state, &response.project);
    }
    Ok(response)
}

#[tauri::command]
async fn preview_fold_import(
    app: AppHandle,
    state: State<'_, AppState>,
    import_state: State<'_, FoldImportState>,
) -> Result<FoldImportPreviewResponse, String> {
    let (expected_instance_id, expected_project_id, expected_revision, initial_directory) = {
        let project = lock_project(&state)?;
        (
            project.instance_id,
            project.project_id,
            project.editor.revision(),
            project
                .current_path
                .as_deref()
                .and_then(Path::parent)
                .map(Path::to_path_buf),
        )
    };
    // Starting a new picker invalidates an older preview. This keeps the
    // native staging bound at one validated source even if IPC is invoked
    // outside the normal modal UI.
    *lock_fold_import(&import_state)? = None;

    let mut dialog = app
        .dialog()
        .file()
        .add_filter("FOLD crease pattern", &["fold"])
        .set_title("FOLD展開図を取り込む");
    if let Some(directory) = initial_directory {
        dialog = dialog.set_directory(directory);
    }
    let Some(selected) = dialog.blocking_pick_file() else {
        return Ok(FoldImportPreviewResponse {
            canceled: true,
            preview: None,
        });
    };
    let path = selected
        .simplified()
        .into_path()
        .map_err(|_| "the selected location is not a local file".to_owned())?;
    let (bytes, preview) =
        tauri::async_runtime::spawn_blocking(move || load_fold_import_preview(&path))
            .await
            .map_err(fold_import_task_error)??;

    {
        let project = lock_project(&state)?;
        ensure_expected_project(
            &project,
            expected_instance_id,
            expected_project_id,
            expected_revision,
        )?;
    }
    let import_id = stage_pending_fold_import(
        &import_state,
        expected_instance_id,
        expected_project_id,
        expected_revision,
        bytes,
    )?;
    Ok(FoldImportPreviewResponse {
        canceled: false,
        preview: Some(fold_import_preview_snapshot(import_id, &preview)),
    })
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
async fn apply_fold_import(
    state: State<'_, AppState>,
    recovery: State<'_, RecoveryRuntime>,
    import_state: State<'_, FoldImportState>,
    preview_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    name: String,
    millimeters_per_unit: f64,
    boundary_candidate_id: u16,
    assignment_mappings: Vec<FoldImportAssignmentMappingRequest>,
) -> Result<ProjectSnapshot, String> {
    let name = normalize_project_name(&name)?;
    validate_import_scale(millimeters_per_unit)?;
    let mappings = validate_fold_import_mapping_requests(assignment_mappings)?;
    let pending = pending_fold_import(
        &import_state,
        preview_id,
        expected_project_id,
        expected_revision,
    )?;
    let bytes = Arc::clone(&pending.bytes);
    let replacement = tauri::async_runtime::spawn_blocking(move || {
        build_fold_import_replacement(
            &bytes,
            name,
            millimeters_per_unit,
            FoldBoundaryCandidateId(boundary_candidate_id),
            mappings,
        )
    })
    .await
    .map_err(fold_conversion_task_error)??;

    // Lock order is always import staging before project state. Cancellation
    // can invalidate the token while conversion runs, but cannot interleave
    // with the final checked replacement.
    let mut pending_slot = lock_fold_import(&import_state)?;
    let mut project = lock_project(&state)?;
    let response = commit_fold_import_replacement(
        &mut project,
        &mut pending_slot,
        preview_id,
        expected_project_id,
        expected_revision,
        replacement,
    )?;
    drop(project);
    drop(pending_slot);
    let _ = recovery.clear_after_normal_completion(&state, &response);
    Ok(response)
}

#[tauri::command]
fn cancel_fold_import(
    state: State<'_, FoldImportState>,
    preview_id: ProjectId,
) -> Result<(), String> {
    cancel_pending_fold_import(&state, preview_id)
}

#[tauri::command]
async fn preview_svg_import(
    app: AppHandle,
    state: State<'_, AppState>,
    import_state: State<'_, SvgImportState>,
) -> Result<SvgImportPreviewResponse, String> {
    let (expected_instance_id, expected_project_id, expected_revision, initial_directory) = {
        let project = lock_project(&state)?;
        (
            project.instance_id,
            project.project_id,
            project.editor.revision(),
            project
                .current_path
                .as_deref()
                .and_then(Path::parent)
                .map(Path::to_path_buf),
        )
    };
    {
        let mut slot = lock_svg_import(&import_state)?;
        slot.pending = None;
        slot.validation_generation_id = None;
        slot.validation = None;
        slot.last_cancelled_id = None;
    }

    let mut dialog = app
        .dialog()
        .file()
        .add_filter("SVG straight-line crease pattern", &["svg"])
        .set_title("SVG展開図を取り込む");
    if let Some(directory) = initial_directory {
        dialog = dialog.set_directory(directory);
    }
    let Some(selected) = dialog.blocking_pick_file() else {
        return Ok(SvgImportPreviewResponse {
            canceled: true,
            preview: None,
        });
    };
    let path = selected
        .simplified()
        .into_path()
        .map_err(|_| "the selected location is not a local file".to_owned())?;
    let (bytes, preview) =
        tauri::async_runtime::spawn_blocking(move || load_svg_import_preview(&path))
            .await
            .map_err(|_| "SVG import task failed".to_owned())??;

    {
        let project = lock_project(&state)?;
        ensure_expected_project(
            &project,
            expected_instance_id,
            expected_project_id,
            expected_revision,
        )?;
    }
    let import_id = stage_pending_svg_import(
        &import_state,
        expected_instance_id,
        expected_project_id,
        expected_revision,
        bytes,
    )?;
    let snapshot = match svg_import_preview_snapshot(import_id, &preview) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            cancel_pending_svg_import(&import_state, import_id)?;
            return Err(error);
        }
    };
    Ok(SvgImportPreviewResponse {
        canceled: false,
        preview: Some(snapshot),
    })
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
async fn validate_svg_import_settings(
    state: State<'_, AppState>,
    import_state: State<'_, SvgImportState>,
    preview_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    millimeters_per_unit: f64,
    boundary_candidate_id: Option<u16>,
    style_mappings: Vec<SvgImportStyleMappingRequest>,
) -> Result<SvgImportSettingsValidationResponse, String> {
    let validation_id = ProjectId::new();
    let pending = begin_svg_import_settings_validation(
        &import_state,
        validation_id,
        preview_id,
        expected_project_id,
        expected_revision,
    )?;

    let result = async {
        validate_import_scale(millimeters_per_unit)?;
        let group_mappings = svg_import_group_mappings(style_mappings)?;
        let boundary_candidate = boundary_candidate_id.map(SvgBoundaryCandidateId);
        {
            let project = lock_project(&state)?;
            ensure_expected_project(
                &project,
                pending.expected_instance_id,
                pending.expected_project_id,
                pending.expected_revision,
            )?;
        }

        let bytes = Arc::clone(&pending.bytes);
        let conversion_mappings = group_mappings.clone();
        let dimensions = tauri::async_runtime::spawn_blocking(move || {
            validate_svg_import_geometry(
                &bytes,
                millimeters_per_unit,
                conversion_mappings,
                boundary_candidate,
            )
        })
        .await
        .map_err(|_| "SVG boundary validation task failed".to_owned())??;

        let mut slot = lock_svg_import(&import_state)?;
        let project = lock_project(&state)?;
        complete_svg_import_settings_validation(
            &mut slot,
            &project,
            SvgImportSettingsValidationCompletion {
                validation: SvgImportSettingsValidation {
                    validation_id,
                    import_id: pending.import_id,
                    expected_instance_id: pending.expected_instance_id,
                    expected_project_id: pending.expected_project_id,
                    expected_revision: pending.expected_revision,
                    millimeters_per_unit_bits: millimeters_per_unit.to_bits(),
                    boundary_candidate,
                    group_mappings,
                },
                geometry: dimensions,
            },
        )
    }
    .await;

    if result.is_err() {
        let _ = abandon_svg_import_settings_validation(&import_state, validation_id);
    }
    result
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
async fn apply_svg_import(
    state: State<'_, AppState>,
    recovery: State<'_, RecoveryRuntime>,
    import_state: State<'_, SvgImportState>,
    preview_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    replace_dirty_project_confirmed: bool,
    name: String,
    millimeters_per_unit: f64,
    boundary_candidate_id: Option<u16>,
    validation_id: ProjectId,
    boundary_confirmed: bool,
    style_mappings: Vec<SvgImportStyleMappingRequest>,
    warnings_acknowledged: bool,
    cutting_allowed_confirmed: bool,
) -> Result<ProjectSnapshot, String> {
    let name = normalize_project_name(&name)?;
    validate_import_scale(millimeters_per_unit)?;
    let group_mappings = svg_import_group_mappings(style_mappings)?;
    let boundary_candidate = boundary_candidate_id.map(SvgBoundaryCandidateId);
    let pending = {
        let slot = lock_svg_import(&import_state)?;
        let pending =
            pending_svg_import_in_slot(&slot, preview_id, expected_project_id, expected_revision)?;
        ensure_svg_import_settings_validation(
            &slot,
            pending,
            validation_id,
            boundary_candidate,
            millimeters_per_unit,
            &group_mappings,
        )?;
        pending.clone()
    };
    let bytes = Arc::clone(&pending.bytes);
    let final_group_mappings = group_mappings.clone();
    let replacement = tauri::async_runtime::spawn_blocking(move || {
        build_svg_import_replacement(
            &bytes,
            SvgImportReplacementOptions {
                name,
                millimeters_per_unit,
                group_mappings,
                boundary_candidate,
                boundary_confirmed,
                warnings_acknowledged,
                cutting_allowed_confirmed,
            },
        )
    })
    .await
    .map_err(|_| "SVG conversion task failed".to_owned())??;

    let mut pending_slot = lock_svg_import(&import_state)?;
    let mut project = lock_project(&state)?;
    let pending = pending_svg_import_in_slot(
        &pending_slot,
        preview_id,
        expected_project_id,
        expected_revision,
    )?;
    ensure_svg_import_settings_validation(
        &pending_slot,
        pending,
        validation_id,
        boundary_candidate,
        millimeters_per_unit,
        &final_group_mappings,
    )?;
    let snapshot = commit_svg_import_replacement(
        &mut project,
        &mut pending_slot.pending,
        preview_id,
        expected_project_id,
        expected_revision,
        replace_dirty_project_confirmed,
        replacement,
    )?;
    pending_slot.validation_generation_id = None;
    pending_slot.validation = None;
    drop(project);
    drop(pending_slot);
    let _ = recovery.clear_after_normal_completion(&state, &snapshot);
    Ok(snapshot)
}

#[tauri::command]
fn cancel_svg_import(
    state: State<'_, SvgImportState>,
    preview_id: ProjectId,
) -> Result<(), String> {
    cancel_pending_svg_import(&state, preview_id)
}

#[tauri::command]
fn add_vertex(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    x: f64,
    y: f64,
    x_expression: String,
    y_expression: String,
) -> Result<ProjectSnapshot, String> {
    let mut project = lock_project(&state)?;
    validate_coordinate_expression_pair(&x_expression, &y_expression, x, y)?;
    let id = VertexId::new();
    execute_command(
        &mut project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        Command::AddVertex {
            id,
            position: Point2::new(x, y),
        },
    )?;
    project.adopt_vertex_coordinate_expression(VertexCoordinateExpressions::new(
        id,
        x_expression,
        y_expression,
        x,
        y,
    ));
    Ok(snapshot(&project))
}

#[tauri::command]
fn move_vertex(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    id: VertexId,
    x: f64,
    y: f64,
    x_expression: String,
    y_expression: String,
) -> Result<ProjectSnapshot, String> {
    let mut project = lock_project(&state)?;
    validate_coordinate_expression_pair(&x_expression, &y_expression, x, y)?;
    execute_command(
        &mut project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        Command::MoveVertex {
            id,
            position: Point2::new(x, y),
        },
    )?;
    project.adopt_vertex_coordinate_expression(VertexCoordinateExpressions::new(
        id,
        x_expression,
        y_expression,
        x,
        y,
    ));
    Ok(snapshot(&project))
}

#[tauri::command]
fn move_edge(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    id: EdgeId,
    delta_x_expression: String,
    delta_y_expression: String,
    delta_x_mm: f64,
    delta_y_mm: f64,
) -> Result<ProjectSnapshot, String> {
    let mut project = lock_project(&state)?;
    validate_coordinate_expression_pair(
        &delta_x_expression,
        &delta_y_expression,
        delta_x_mm,
        delta_y_mm,
    )?;
    let edge = project
        .editor
        .pattern()
        .edges
        .iter()
        .find(|edge| edge.id == id)
        .cloned()
        .ok_or_else(|| "edge not found".to_owned())?;
    let position = |vertex_id| {
        project
            .editor
            .pattern()
            .vertices
            .iter()
            .find(|vertex| vertex.id == vertex_id)
            .map(|vertex| vertex.position)
            .ok_or_else(|| "vertex not found".to_owned())
    };
    let start = position(edge.start)?;
    let end = position(edge.end)?;
    let start_position = Point2::new(start.x + delta_x_mm, start.y + delta_y_mm);
    let end_position = Point2::new(end.x + delta_x_mm, end.y + delta_y_mm);
    if !start_position.x.is_finite()
        || !start_position.y.is_finite()
        || !end_position.x.is_finite()
        || !end_position.y.is_finite()
    {
        return Err(PROJECT_NUMERIC_EXPRESSIONS_INVALID_MESSAGE.to_owned());
    }
    execute_command(
        &mut project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        Command::MoveEdge {
            id,
            start_position,
            end_position,
        },
    )?;
    for (vertex, previous, adopted) in [
        (edge.start, start, start_position),
        (edge.end, end, end_position),
    ] {
        project.adopt_vertex_coordinate_expression(VertexCoordinateExpressions::new(
            vertex,
            format!("({})+({delta_x_expression})", previous.x),
            format!("({})+({delta_y_expression})", previous.y),
            adopted.x,
            adopted.y,
        ));
    }
    Ok(snapshot(&project))
}

#[tauri::command]
fn mirror_edge_left_right(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    id: EdgeId,
    axis_x_expression: String,
    axis_x_mm: f64,
) -> Result<ProjectSnapshot, String> {
    let (evaluated, _) = evaluate_finite_millimetre_pair(axis_x_expression.clone(), "0".to_owned())
        .map_err(map_loaded_numeric_expression_error)?;
    if evaluated.to_bits() != axis_x_mm.to_bits() {
        return Err(PROJECT_NUMERIC_EXPRESSIONS_INVALID_MESSAGE.to_owned());
    }
    transform_edge_points(
        state,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        id,
        |point| mirror_point_left_right(point, axis_x_mm),
        |point| {
            (
                format!("2*({axis_x_expression})-({})", point.x),
                point.y.to_string(),
            )
        },
    )
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
fn rotate_edge_about_point(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    id: EdgeId,
    center_x_expression: String,
    center_y_expression: String,
    angle_degrees_expression: String,
    center_x_mm: f64,
    center_y_mm: f64,
    angle_degrees: f64,
) -> Result<ProjectSnapshot, String> {
    let (evaluated_x, evaluated_y) =
        evaluate_finite_millimetre_pair(center_x_expression.clone(), center_y_expression.clone())
            .map_err(map_loaded_numeric_expression_error)?;
    let (evaluated_angle, _) =
        evaluate_finite_millimetre_pair(angle_degrees_expression.clone(), "0".to_owned())
            .map_err(map_loaded_numeric_expression_error)?;
    if evaluated_x.to_bits() != center_x_mm.to_bits()
        || evaluated_y.to_bits() != center_y_mm.to_bits()
        || evaluated_angle.to_bits() != angle_degrees.to_bits()
    {
        return Err(PROJECT_NUMERIC_EXPRESSIONS_INVALID_MESSAGE.to_owned());
    }
    let (sin, cos) = symmetry_sin_cos(angle_degrees);
    if !sin.is_finite() || !cos.is_finite() {
        return Err(PROJECT_NUMERIC_EXPRESSIONS_INVALID_MESSAGE.to_owned());
    }
    transform_edge_points(
        state,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        id,
        |point| rotate_point_about(point, Point2::new(center_x_mm, center_y_mm), sin, cos),
        |point| {
            (
                format!(
                    "({center_x_expression})+(({})-({center_x_expression}))*cos(({angle_degrees_expression})*pi/180)-(({})-({center_y_expression}))*sin(({angle_degrees_expression})*pi/180)",
                    point.x, point.y
                ),
                format!(
                    "({center_y_expression})+(({})-({center_x_expression}))*sin(({angle_degrees_expression})*pi/180)+(({})-({center_y_expression}))*cos(({angle_degrees_expression})*pi/180)",
                    point.x, point.y
                ),
            )
        },
    )
}

fn mirror_point_left_right(point: Point2, axis_x: f64) -> Point2 {
    Point2::new(axis_x.mul_add(2.0, -point.x), point.y)
}

fn rotate_point_about(point: Point2, center: Point2, sin: f64, cos: f64) -> Point2 {
    let x = point.x - center.x;
    let y = point.y - center.y;
    Point2::new(center.x + x * cos - y * sin, center.y + x * sin + y * cos)
}

fn symmetry_sin_cos(angle_degrees: f64) -> (f64, f64) {
    match angle_degrees.rem_euclid(360.0) {
        0.0 => (0.0, 1.0),
        90.0 => (1.0, 0.0),
        180.0 => (0.0, -1.0),
        270.0 => (-1.0, 0.0),
        angle => angle.to_radians().sin_cos(),
    }
}

fn transform_edge_points(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    id: EdgeId,
    transform: impl Fn(Point2) -> Point2,
    expression: impl Fn(Point2) -> (String, String),
) -> Result<ProjectSnapshot, String> {
    let mut project = lock_project(&state)?;
    let edge = project
        .editor
        .pattern()
        .edges
        .iter()
        .find(|edge| edge.id == id)
        .cloned()
        .ok_or_else(|| "edge not found".to_owned())?;
    let position = |vertex_id| {
        project
            .editor
            .pattern()
            .vertices
            .iter()
            .find(|vertex| vertex.id == vertex_id)
            .map(|vertex| vertex.position)
            .ok_or_else(|| "vertex not found".to_owned())
    };
    let start = position(edge.start)?;
    let end = position(edge.end)?;
    let start_position = transform(start);
    let end_position = transform(end);
    if !start_position.x.is_finite()
        || !start_position.y.is_finite()
        || !end_position.x.is_finite()
        || !end_position.y.is_finite()
    {
        return Err(PROJECT_NUMERIC_EXPRESSIONS_INVALID_MESSAGE.to_owned());
    }
    execute_command(
        &mut project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        Command::MoveEdge {
            id,
            start_position,
            end_position,
        },
    )?;
    for (vertex, previous, adopted) in [
        (edge.start, start, start_position),
        (edge.end, end, end_position),
    ] {
        let (x_source, y_source) = expression(previous);
        project.adopt_vertex_coordinate_expression(VertexCoordinateExpressions::new(
            vertex, x_source, y_source, adopted.x, adopted.y,
        ));
    }
    Ok(snapshot(&project))
}

#[tauri::command]
fn move_vertices(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    vertices: Vec<VertexId>,
    delta_x_expression: String,
    delta_y_expression: String,
    delta_x_mm: f64,
    delta_y_mm: f64,
) -> Result<ProjectSnapshot, String> {
    let mut project = lock_project(&state)?;
    validate_coordinate_expression_pair(
        &delta_x_expression,
        &delta_y_expression,
        delta_x_mm,
        delta_y_mm,
    )?;
    if vertices.is_empty() || vertices.len() > ori_domain::DEFAULT_MAX_CONSTRAINT_VERTICES {
        return Err(PROJECT_NUMERIC_EXPRESSIONS_INVALID_MESSAGE.to_owned());
    }
    let mut unique = HashSet::with_capacity(vertices.len());
    let mut planned = Vec::with_capacity(vertices.len());
    for vertex in vertices {
        if !unique.insert(vertex) {
            return Err(PROJECT_NUMERIC_EXPRESSIONS_INVALID_MESSAGE.to_owned());
        }
        let previous = project
            .editor
            .pattern()
            .vertices
            .iter()
            .find(|candidate| candidate.id == vertex)
            .map(|candidate| candidate.position)
            .ok_or_else(|| "vertex not found".to_owned())?;
        let position = Point2::new(previous.x + delta_x_mm, previous.y + delta_y_mm);
        if !position.x.is_finite() || !position.y.is_finite() {
            return Err(PROJECT_NUMERIC_EXPRESSIONS_INVALID_MESSAGE.to_owned());
        }
        planned.push((vertex, previous, position));
    }
    execute_command(
        &mut project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        Command::MoveVertices {
            updates: planned
                .iter()
                .map(|(vertex, _, position)| VertexPositionUpdate {
                    vertex: *vertex,
                    position: *position,
                })
                .collect(),
        },
    )?;
    for (vertex, previous, adopted) in planned {
        project.adopt_vertex_coordinate_expression(VertexCoordinateExpressions::new(
            vertex,
            format!("({})+({delta_x_expression})", previous.x),
            format!("({})+({delta_y_expression})", previous.y),
            adopted.x,
            adopted.y,
        ));
    }
    Ok(snapshot(&project))
}

#[tauri::command]
fn remove_vertex(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    id: VertexId,
) -> Result<ProjectSnapshot, String> {
    let mut project = lock_project(&state)?;
    execute_command(
        &mut project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        Command::RemoveVertex { id },
    )?;
    project.remove_vertex_coordinate_expression(id);
    Ok(snapshot(&project))
}

#[tauri::command]
fn add_edge(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    start: VertexId,
    end: VertexId,
    kind: EdgeKind,
) -> Result<ProjectSnapshot, String> {
    let mut project = lock_project(&state)?;
    execute_command(
        &mut project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        Command::AddEdge {
            id: EdgeId::new(),
            start,
            end,
            kind,
        },
    )
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
fn add_connected_vertex(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    start: VertexId,
    x: f64,
    y: f64,
    length_expression: String,
    angle_degrees_expression: String,
    length_mm: f64,
    angle_degrees: f64,
    kind: EdgeKind,
) -> Result<ProjectSnapshot, String> {
    let mut project = lock_project(&state)?;
    let (evaluated_length_mm, evaluated_angle_degrees) = evaluate_finite_millimetre_pair(
        length_expression.clone(),
        angle_degrees_expression.clone(),
    )
    .map_err(map_loaded_numeric_expression_error)?;
    if evaluated_length_mm.to_bits() != length_mm.to_bits()
        || evaluated_angle_degrees.to_bits() != angle_degrees.to_bits()
        || length_mm <= 0.0
        || angle_degrees.abs() > 360_000.0
    {
        return Err(PROJECT_NUMERIC_EXPRESSIONS_INVALID_MESSAGE.to_owned());
    }
    let start_position = project
        .editor
        .pattern()
        .vertices
        .iter()
        .find(|vertex| vertex.id == start)
        .map(|vertex| vertex.position)
        .ok_or_else(|| PROJECT_NUMERIC_EXPRESSIONS_INVALID_MESSAGE.to_owned())?;
    let angle_radians = angle_degrees.to_radians();
    let expected_x = start_position.x + length_mm * angle_radians.cos();
    let expected_y = start_position.y + length_mm * angle_radians.sin();
    if expected_x.to_bits() != x.to_bits() || expected_y.to_bits() != y.to_bits() {
        return Err(PROJECT_NUMERIC_EXPRESSIONS_INVALID_MESSAGE.to_owned());
    }
    let vertex_id = VertexId::new();
    execute_command(
        &mut project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        Command::AddConnectedVertex {
            vertex_id,
            position: Point2::new(x, y),
            edge_id: EdgeId::new(),
            start,
            kind,
        },
    )?;
    let mut binding =
        VertexCoordinateExpressions::new(vertex_id, x.to_string(), y.to_string(), x, y);
    binding.polar_construction = Some(PolarVertexConstructionExpressions {
        schema_version: 1,
        start_vertex: start,
        adopted_start_x_mm: start_position.x,
        adopted_start_y_mm: start_position.y,
        length_source: length_expression,
        angle_degrees_source: angle_degrees_expression,
        adopted_length_mm: length_mm,
        adopted_angle_degrees: angle_degrees,
    });
    project.adopt_vertex_coordinate_expression(binding);
    Ok(snapshot(&project))
}

#[tauri::command]
fn remove_edge(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    id: EdgeId,
) -> Result<ProjectSnapshot, String> {
    let mut project = lock_project(&state)?;
    execute_command(
        &mut project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        Command::RemoveEdge { id },
    )
}

#[tauri::command]
fn create_project_layer(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    name: String,
    content_kind: LayerContentKindV1,
) -> Result<ProjectSnapshot, String> {
    let mut project = lock_project(&state)?;
    create_project_layer_in_project(
        &mut project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        name,
        content_kind,
    )
}

fn create_project_layer_in_project(
    project: &mut ProjectState,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    name: String,
    content_kind: LayerContentKindV1,
) -> Result<ProjectSnapshot, String> {
    ensure_expected_project(
        project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
    )?;
    let target_index = project.editor.project_layers().layers.len();
    execute_command(
        project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        Command::CreateLayer {
            layer: LayerRecordV1 {
                id: LayerId::new(),
                name,
                content_kind,
                visible: true,
                locked: false,
                opacity: 1.0,
            },
            target_index,
        },
    )
}

#[tauri::command]
fn rename_project_layer(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    layer: LayerId,
    name: String,
) -> Result<ProjectSnapshot, String> {
    let mut project = lock_project(&state)?;
    rename_project_layer_in_project(
        &mut project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        layer,
        name,
    )
}

fn rename_project_layer_in_project(
    project: &mut ProjectState,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    layer: LayerId,
    name: String,
) -> Result<ProjectSnapshot, String> {
    execute_command(
        project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        Command::RenameLayer { layer, name },
    )
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectLayerPresentationInput {
    visible: bool,
    locked: bool,
    opacity: f64,
}

#[tauri::command]
fn update_project_layer_presentation(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    layer: LayerId,
    presentation: ProjectLayerPresentationInput,
) -> Result<ProjectSnapshot, String> {
    let mut project = lock_project(&state)?;
    update_project_layer_presentation_in_project(
        &mut project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        layer,
        presentation,
    )
}

fn update_project_layer_presentation_in_project(
    project: &mut ProjectState,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    layer: LayerId,
    presentation: ProjectLayerPresentationInput,
) -> Result<ProjectSnapshot, String> {
    execute_command(
        project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        Command::UpdateLayerPresentation {
            layer,
            visible: presentation.visible,
            locked: presentation.locked,
            opacity: presentation.opacity,
        },
    )
}

#[tauri::command]
fn move_project_layer(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    layer: LayerId,
    target_index: usize,
) -> Result<ProjectSnapshot, String> {
    let mut project = lock_project(&state)?;
    move_project_layer_in_project(
        &mut project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        layer,
        target_index,
    )
}

fn move_project_layer_in_project(
    project: &mut ProjectState,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    layer: LayerId,
    target_index: usize,
) -> Result<ProjectSnapshot, String> {
    execute_command(
        project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        Command::MoveLayer {
            layer,
            target_index,
        },
    )
}

#[tauri::command]
fn delete_project_layer(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    layer: LayerId,
) -> Result<ProjectSnapshot, String> {
    let mut project = lock_project(&state)?;
    delete_project_layer_in_project(
        &mut project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        layer,
    )
}

fn delete_project_layer_in_project(
    project: &mut ProjectState,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    layer: LayerId,
) -> Result<ProjectSnapshot, String> {
    execute_command(
        project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        Command::DeleteLayer { layer },
    )
}

#[tauri::command]
fn assign_edge_to_project_layer(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    edge: EdgeId,
    layer: LayerId,
) -> Result<ProjectSnapshot, String> {
    let mut project = lock_project(&state)?;
    assign_edge_to_project_layer_in_project(
        &mut project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        edge,
        layer,
    )
}

fn assign_edge_to_project_layer_in_project(
    project: &mut ProjectState,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    edge: EdgeId,
    layer: LayerId,
) -> Result<ProjectSnapshot, String> {
    execute_command(
        project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        Command::AssignEdgeToLayer { edge, layer },
    )
}

#[tauri::command]
fn add_edge_orientation_constraint(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    edge: EdgeId,
    orientation: EdgeOrientationConstraint,
) -> Result<ProjectSnapshot, String> {
    let mut project = lock_project(&state)?;
    ensure_expected_project(
        &project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
    )?;
    let constraint = match orientation {
        EdgeOrientationConstraint::Horizontal => GeometricConstraintKindV1::Horizontal { edge },
        EdgeOrientationConstraint::Vertical => GeometricConstraintKindV1::Vertical { edge },
    };
    execute_command(
        &mut project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        Command::AddGeometricConstraint {
            record: GeometricConstraintRecordV1 {
                id: ConstraintId::new(),
                constraint,
            },
        },
    )
}

#[tauri::command]
fn add_geometric_constraint(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    constraint: GeometricConstraintKindV1,
) -> Result<ProjectSnapshot, String> {
    let mut project = lock_project(&state)?;
    ensure_expected_project(
        &project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
    )?;
    execute_command(
        &mut project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        Command::AddGeometricConstraint {
            record: GeometricConstraintRecordV1 {
                id: ConstraintId::new(),
                constraint,
            },
        },
    )
}

#[tauri::command]
fn remove_geometric_constraint(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    constraint: ConstraintId,
) -> Result<ProjectSnapshot, String> {
    let mut project = lock_project(&state)?;
    ensure_expected_project(
        &project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
    )?;
    execute_command(
        &mut project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        Command::RemoveGeometricConstraint { id: constraint },
    )
}

#[tauri::command]
fn undo(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
) -> Result<ProjectSnapshot, String> {
    let mut project = lock_project(&state)?;
    execute_undo(
        &mut project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
    )
}

#[tauri::command]
fn redo(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
) -> Result<ProjectSnapshot, String> {
    let mut project = lock_project(&state)?;
    execute_redo(
        &mut project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
    )
}

fn execute_undo(
    project: &mut ProjectState,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
) -> Result<ProjectSnapshot, String> {
    ensure_project_instance_identity(project, expected_project_instance_id, expected_project_id)?;
    if project.editor.revision() != expected_revision
        || !project.editor.can_undo()
        || project.editor.revision() == ori_core::MAX_REVISION
    {
        project
            .editor
            .undo(expected_revision)
            .map_err(|error| error.to_string())?;
        project.undo_numeric_expression_edit();
        return Ok(snapshot(project));
    }
    let authority = project.applied_pose_authority.clone();
    let invalidation = authority
        .begin_invalidation()
        .map_err(|error| error.to_string())?;
    project
        .editor
        .undo(expected_revision)
        .map_err(|error| error.to_string())?;
    project.undo_numeric_expression_edit();
    invalidation.commit();
    Ok(snapshot(project))
}

fn execute_redo(
    project: &mut ProjectState,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
) -> Result<ProjectSnapshot, String> {
    ensure_project_instance_identity(project, expected_project_instance_id, expected_project_id)?;
    if project.editor.revision() != expected_revision
        || !project.editor.can_redo()
        || project.editor.revision() == ori_core::MAX_REVISION
    {
        project
            .editor
            .redo(expected_revision)
            .map_err(|error| error.to_string())?;
        project.redo_numeric_expression_edit();
        return Ok(snapshot(project));
    }
    let authority = project.applied_pose_authority.clone();
    let invalidation = authority
        .begin_invalidation()
        .map_err(|error| error.to_string())?;
    project
        .editor
        .redo(expected_revision)
        .map_err(|error| error.to_string())?;
    project.redo_numeric_expression_edit();
    invalidation.commit();
    Ok(snapshot(project))
}

const NAMED_TECHNIQUE_TIMELINE_PROPOSAL_SCHEMA_VERSION_V1: u32 = 1;
const MAX_NAMED_TECHNIQUE_TIMELINE_PROPOSAL_BYTES: usize = 2 * 1024 * 1024;
const MAX_NAMED_TECHNIQUE_IDENTIFIER_BYTES: usize = 96;
const MAX_NAMED_TECHNIQUE_VERSION: u32 = 1_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum NamedTechniqueTimelineSourceKindV1 {
    Technique,
    Parameter,
    Precondition,
    Operation,
}

impl NamedTechniqueTimelineSourceKindV1 {
    const fn rank(self) -> u8 {
        match self {
            Self::Technique => 0,
            Self::Parameter => 1,
            Self::Precondition => 2,
            Self::Operation => 3,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct NamedTechniqueTimelineProposalStepV1 {
    source_kind: NamedTechniqueTimelineSourceKindV1,
    source_id: String,
    chunk_index: u32,
    chunk_count: u32,
    title: String,
    description: String,
    caution: String,
    duration_ms: u32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct NamedTechniqueTimelineProposalV1 {
    schema_version: u32,
    package_id: String,
    technique_id: String,
    technique_version: u32,
    steps: Vec<NamedTechniqueTimelineProposalStepV1>,
}

fn parse_named_technique_timeline_proposal(
    proposal_json: &str,
) -> Result<NamedTechniqueTimelineProposalV1, String> {
    if proposal_json.len() > MAX_NAMED_TECHNIQUE_TIMELINE_PROPOSAL_BYTES {
        return Err("the named-technique timeline proposal is too large".to_owned());
    }
    let proposal: NamedTechniqueTimelineProposalV1 = serde_json::from_str(proposal_json)
        .map_err(|_| "the named-technique timeline proposal is invalid".to_owned())?;
    if proposal.schema_version != NAMED_TECHNIQUE_TIMELINE_PROPOSAL_SCHEMA_VERSION_V1
        || !is_named_technique_identifier(&proposal.package_id)
        || !is_named_technique_identifier(&proposal.technique_id)
        || !(1..=MAX_NAMED_TECHNIQUE_VERSION).contains(&proposal.technique_version)
        || proposal.steps.is_empty()
        || proposal.steps.len() > MAX_INSTRUCTION_STEPS
        || proposal.steps.first().is_none_or(|step| {
            step.source_kind != NamedTechniqueTimelineSourceKindV1::Technique
                || step.source_id != proposal.technique_id
        })
    {
        return Err("the named-technique timeline proposal is invalid".to_owned());
    }

    let mut previous_rank = 0_u8;
    let mut previous_source: Option<(NamedTechniqueTimelineSourceKindV1, &str, u32, u32)> = None;
    let mut seen_sources = HashSet::with_capacity(proposal.steps.len());
    for step in &proposal.steps {
        if !is_named_technique_identifier(&step.source_id)
            || (step.source_kind == NamedTechniqueTimelineSourceKindV1::Technique
                && step.source_id != proposal.technique_id)
            || step.chunk_count == 0
            || step.chunk_count as usize > MAX_INSTRUCTION_STEPS
            || step.chunk_index == 0
            || step.chunk_index > step.chunk_count
            || step.source_kind.rank() < previous_rank
        {
            return Err("the named-technique timeline proposal is invalid".to_owned());
        }
        match previous_source {
            Some((kind, source_id, chunk_index, _chunk_count))
                if kind == step.source_kind && source_id == step.source_id =>
            {
                if step.chunk_index != chunk_index.saturating_add(1) {
                    return Err("the named-technique timeline proposal is invalid".to_owned());
                }
            }
            Some((_, _, chunk_index, chunk_count))
                if chunk_index != chunk_count || step.chunk_index != 1 =>
            {
                return Err("the named-technique timeline proposal is invalid".to_owned());
            }
            _ if step.chunk_index != 1 => {
                return Err("the named-technique timeline proposal is invalid".to_owned());
            }
            _ => {
                if !seen_sources.insert((step.source_kind.rank(), step.source_id.clone())) {
                    return Err("the named-technique timeline proposal is invalid".to_owned());
                }
            }
        }
        previous_rank = step.source_kind.rank();
        previous_source = Some((
            step.source_kind,
            &step.source_id,
            step.chunk_index,
            step.chunk_count,
        ));
    }
    if proposal
        .steps
        .last()
        .is_some_and(|step| step.chunk_index != step.chunk_count)
    {
        return Err("the named-technique timeline proposal is invalid".to_owned());
    }
    Ok(proposal)
}

fn is_named_technique_identifier(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.is_empty()
        || bytes.len() > MAX_NAMED_TECHNIQUE_IDENTIFIER_BYTES
        || !bytes[0].is_ascii_lowercase()
    {
        return false;
    }
    bytes.iter().copied().enumerate().all(|(index, byte)| {
        byte.is_ascii_lowercase()
            || byte.is_ascii_digit()
            || (matches!(byte, b'.' | b'_' | b'-')
                && index + 1 < bytes.len()
                && (bytes[index + 1].is_ascii_lowercase() || bytes[index + 1].is_ascii_digit()))
    })
}

#[tauri::command]
fn append_named_technique_instruction_steps(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    proposal_json: String,
) -> Result<ProjectSnapshot, String> {
    let proposal = parse_named_technique_timeline_proposal(&proposal_json)?;
    let mut project = lock_project(&state)?;
    ensure_expected_project(
        &project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
    )?;
    let fingerprint = project.editor.fold_model_fingerprint_v1();
    let steps = proposal
        .steps
        .into_iter()
        .map(|step| InstructionStep {
            id: InstructionStepId::new(),
            title: step.title,
            description: step.description,
            caution: step.caution,
            duration_ms: step.duration_ms,
            visual: Default::default(),
            pose: InstructionPose {
                model: InstructionPoseModel::DeclarativeOnlyV1,
                source_model_fingerprint: fingerprint.clone(),
                fixed_face: None,
                hinge_angles: Vec::new(),
            },
        })
        .collect();
    execute_command(
        &mut project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        Command::AppendInstructionSteps { steps },
    )
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
async fn add_instruction_step(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    title: String,
    description: String,
    caution: String,
    duration_ms: u32,
    fixed_face: Option<FaceId>,
    hinge_angles: Vec<InstructionHingeAngle>,
) -> Result<ProjectSnapshot, String> {
    let analyzed = analyze_instruction_pose(
        &state,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        fixed_face,
        hinge_angles,
    )
    .await?;
    let mut project = lock_project(&state)?;
    let pose = finish_instruction_pose(
        &project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        analyzed,
    )?;
    execute_command(
        &mut project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        Command::AddInstructionStep {
            step: InstructionStep {
                id: InstructionStepId::new(),
                title,
                description,
                caution,
                duration_ms,
                visual: Default::default(),
                pose,
            },
        },
    )
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
fn update_instruction_step_metadata(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    step_id: InstructionStepId,
    title: String,
    description: String,
    caution: String,
    duration_ms: u32,
    visual: InstructionVisual,
) -> Result<ProjectSnapshot, String> {
    let mut project = lock_project(&state)?;
    execute_command(
        &mut project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        Command::UpdateInstructionStepMetadata {
            step_id,
            title,
            description,
            caution,
            duration_ms,
            visual,
        },
    )
}

#[tauri::command]
async fn replace_instruction_step_pose(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    step_id: InstructionStepId,
    fixed_face: Option<FaceId>,
    hinge_angles: Vec<InstructionHingeAngle>,
) -> Result<ProjectSnapshot, String> {
    let analyzed = analyze_instruction_pose(
        &state,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        fixed_face,
        hinge_angles,
    )
    .await?;
    let mut project = lock_project(&state)?;
    let pose = finish_instruction_pose(
        &project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        analyzed,
    )?;
    execute_command(
        &mut project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        Command::ReplaceInstructionStepPose { step_id, pose },
    )
}

#[tauri::command]
fn remove_instruction_step(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    step_id: InstructionStepId,
) -> Result<ProjectSnapshot, String> {
    let mut project = lock_project(&state)?;
    execute_command(
        &mut project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        Command::RemoveInstructionStep { step_id },
    )
}

#[tauri::command]
fn move_instruction_step(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    step_id: InstructionStepId,
    target_index: usize,
) -> Result<ProjectSnapshot, String> {
    let mut project = lock_project(&state)?;
    execute_command(
        &mut project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        Command::MoveInstructionStep {
            step_id,
            target_index,
        },
    )
}

#[tauri::command]
fn set_cutting_allowed(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    allowed: bool,
) -> Result<ProjectSnapshot, String> {
    let mut project = lock_project(&state)?;
    execute_command(
        &mut project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        Command::SetCuttingAllowed { allowed },
    )
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
fn update_paper_properties(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    thickness_mm: f64,
    front_color: RgbaColor,
    back_color: RgbaColor,
    front_texture_asset: Option<ori_domain::AssetId>,
    back_texture_asset: Option<ori_domain::AssetId>,
    cutting_allowed: bool,
) -> Result<ProjectSnapshot, String> {
    let mut project = lock_project(&state)?;
    execute_command(
        &mut project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        Command::UpdatePaperProperties {
            thickness_mm,
            front_color,
            back_color,
            front_texture_asset,
            back_texture_asset,
            cutting_allowed,
        },
    )
}

/// Selects a bounded PNG/JPEG through the native picker, registers it in the
/// authenticated project, and selects it as the paper front in one operation.
///
/// A canceled picker is a successful no-op. The image bytes never cross the
/// webview boundary.
#[tauri::command]
fn import_front_paper_texture(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
) -> Result<ProjectSnapshot, String> {
    {
        let project = lock_project(&state)?;
        ensure_expected_project(
            &project,
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
        )?;
    }

    let selected = app
        .dialog()
        .file()
        .set_title("表面テクスチャ画像 / Front texture image")
        .add_filter("PNG or JPEG image", &["png", "jpg", "jpeg"])
        .blocking_pick_file();
    let Some(selected) = selected else {
        return lock_project(&state).map(|project| snapshot(&project));
    };
    let selected = selected
        .into_path()
        .map_err(|_| "ローカルのテクスチャ画像を選択してください。".to_owned())?;

    let metadata = std::fs::metadata(&selected)
        .map_err(|_| "テクスチャ画像を読み込めませんでした。".to_owned())?;
    if !metadata.is_file() || metadata.len() == 0
        || metadata.len() > MAX_PROJECT_TEXTURE_ASSET_BYTES as u64
    {
        return Err("テクスチャ画像は16 MiB以下のPNG/JPEGを選択してください。".to_owned());
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    File::open(&selected)
        .and_then(|file| {
            file.take((MAX_PROJECT_TEXTURE_ASSET_BYTES + 1) as u64)
                .read_to_end(&mut bytes)
        })
        .map_err(|_| "テクスチャ画像を読み込めませんでした。".to_owned())?;
    let media_type = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        ProjectTextureMediaTypeV1::Png
    } else if bytes.starts_with(&[0xff, 0xd8]) && bytes.ends_with(&[0xff, 0xd9]) {
        ProjectTextureMediaTypeV1::Jpeg
    } else {
        return Err("選択したファイルは有効なPNG/JPEGではありません。".to_owned());
    };

    let mut project = lock_project(&state)?;
    register_front_texture(
        &mut project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        media_type,
        bytes,
    )
}

fn register_front_texture(
    project: &mut ProjectState,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    media_type: ProjectTextureMediaTypeV1,
    bytes: Vec<u8>,
) -> Result<ProjectSnapshot, String> {
    ensure_expected_project(
        project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
    )?;
    let asset_id = AssetId::new();
    let mut retained_total = bytes.len();
    for asset in &project.texture_assets {
        retained_total = retained_total.saturating_add(asset.bytes.len());
    }
    if retained_total > MAX_PROJECT_TEXTURE_ASSET_TOTAL_BYTES
        || project.texture_assets.len() >= ori_formats::MAX_PROJECT_TEXTURE_ASSETS
    {
        return Err("プロジェクト内テクスチャの合計は32 MiB以下にしてください。".to_owned());
    }
    project.texture_assets.push(ProjectTextureAssetV1 {
        id: asset_id,
        media_type,
        bytes,
    });
    let paper = project.editor.paper().clone();
    let result = execute_command(
        project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        Command::UpdatePaperProperties {
            thickness_mm: paper.thickness_mm,
            front_color: paper.front.color,
            back_color: paper.back.color,
            front_texture_asset: Some(asset_id),
            back_texture_asset: paper.back.texture_asset,
            cutting_allowed: paper.cutting_allowed,
        },
    );
    if result.is_err() {
        project.texture_assets.retain(|asset| asset.id != asset_id);
    }
    result
}

#[tauri::command]
fn import_back_paper_texture(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
) -> Result<ProjectSnapshot, String> {
    {
        let project = lock_project(&state)?;
        ensure_expected_project(
            &project,
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
        )?;
    }
    let selected = app
        .dialog()
        .file()
        .set_title("裏面テクスチャ画像 / Back texture image")
        .add_filter("PNG or JPEG image", &["png", "jpg", "jpeg"])
        .blocking_pick_file();
    let Some(selected) = selected else {
        return lock_project(&state).map(|project| snapshot(&project));
    };
    let selected = selected
        .into_path()
        .map_err(|_| "ローカルのテクスチャ画像を選択してください。".to_owned())?;
    let metadata = std::fs::metadata(&selected)
        .map_err(|_| "テクスチャ画像を読み込めませんでした。".to_owned())?;
    if !metadata.is_file()
        || metadata.len() == 0
        || metadata.len() > MAX_PROJECT_TEXTURE_ASSET_BYTES as u64
    {
        return Err("テクスチャ画像は16 MiB以下のPNG/JPEGを選択してください。".to_owned());
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    File::open(&selected)
        .and_then(|file| {
            file.take((MAX_PROJECT_TEXTURE_ASSET_BYTES + 1) as u64)
                .read_to_end(&mut bytes)
        })
        .map_err(|_| "テクスチャ画像を読み込めませんでした。".to_owned())?;
    let media_type = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        ProjectTextureMediaTypeV1::Png
    } else if bytes.starts_with(&[0xff, 0xd8]) && bytes.ends_with(&[0xff, 0xd9]) {
        ProjectTextureMediaTypeV1::Jpeg
    } else {
        return Err("選択したファイルは有効なPNG/JPEGではありません。".to_owned());
    };
    let mut project = lock_project(&state)?;
    register_back_texture(
        &mut project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        media_type,
        bytes,
    )
}

fn register_back_texture(
    project: &mut ProjectState,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    media_type: ProjectTextureMediaTypeV1,
    bytes: Vec<u8>,
) -> Result<ProjectSnapshot, String> {
    ensure_expected_project(
        project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
    )?;
    let total = project.texture_assets.iter().try_fold(bytes.len(), |total, asset| {
        total.checked_add(asset.bytes.len())
    }).ok_or_else(|| "プロジェクト内テクスチャが大きすぎます。".to_owned())?;
    if total > MAX_PROJECT_TEXTURE_ASSET_TOTAL_BYTES
        || project.texture_assets.len() >= ori_formats::MAX_PROJECT_TEXTURE_ASSETS
    {
        return Err("プロジェクト内テクスチャの合計は32 MiB以下にしてください。".to_owned());
    }
    let asset_id = AssetId::new();
    project.texture_assets.push(ProjectTextureAssetV1 {
        id: asset_id,
        media_type,
        bytes,
    });
    let paper = project.editor.paper().clone();
    let result = execute_command(
        project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        Command::UpdatePaperProperties {
            thickness_mm: paper.thickness_mm,
            front_color: paper.front.color,
            back_color: paper.back.color,
            front_texture_asset: paper.front.texture_asset,
            back_texture_asset: Some(asset_id),
            cutting_allowed: paper.cutting_allowed,
        },
    );
    if result.is_err() {
        project.texture_assets.retain(|asset| asset.id != asset_id);
    }
    result
}

#[tauri::command]
fn set_element_metadata(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    target: ori_core::ElementMetadataTargetV1,
    metadata: Option<ori_domain::ElementMetadataV1>,
) -> Result<ProjectSnapshot, String> {
    let mut project = lock_project(&state)?;
    execute_command(
        &mut project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        Command::SetElementMetadata { target, metadata },
    )
}

#[tauri::command]
fn set_length_display_unit(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    unit: LengthDisplayUnit,
) -> Result<ProjectSnapshot, String> {
    let mut project = lock_project(&state)?;
    execute_command(
        &mut project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        Command::SetLengthDisplayUnit { unit },
    )
}

#[tauri::command]
fn resize_rectangular_paper(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    width_expression: String,
    height_expression: String,
    width_mm: f64,
    height_mm: f64,
) -> Result<ProjectSnapshot, String> {
    let mut project = lock_project(&state)?;
    let (evaluated_width_mm, evaluated_height_mm) =
        evaluate_positive_millimetre_pair(width_expression.clone(), height_expression.clone())
            .map_err(map_loaded_numeric_expression_error)?;
    if evaluated_width_mm.to_bits() != width_mm.to_bits()
        || evaluated_height_mm.to_bits() != height_mm.to_bits()
    {
        return Err(PROJECT_NUMERIC_EXPRESSIONS_INVALID_MESSAGE.to_owned());
    }
    execute_command(
        &mut project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        Command::ResizeRectangularPaper {
            width_mm,
            height_mm,
        },
    )?;
    project.numeric_expressions.rectangular_paper_creation =
        Some(RectangularPaperCreationExpressions::new(
            width_expression,
            height_expression,
            width_mm,
            height_mm,
        ));
    Ok(snapshot(&project))
}

#[tauri::command]
fn split_edge(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    edge: EdgeId,
    fraction: f64,
) -> Result<ProjectSnapshot, String> {
    let mut project = lock_project(&state)?;
    execute_edge_split(
        &mut project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        edge,
        fraction,
    )
}

fn execute_edge_split(
    project: &mut ProjectState,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    edge: EdgeId,
    fraction: f64,
) -> Result<ProjectSnapshot, String> {
    execute_command(
        project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        Command::SplitEdge {
            edge,
            new_vertex: VertexId::new(),
            new_edge: EdgeId::new(),
            fraction,
        },
    )
}

#[tauri::command]
fn connect_edge_intersection(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    first_edge: EdgeId,
    second_edge: EdgeId,
) -> Result<EdgeIntersectionResponse, String> {
    let mut project = lock_project(&state)?;
    execute_edge_intersection_connection(
        &mut project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        first_edge,
        second_edge,
    )
}

fn execute_edge_intersection_connection(
    project: &mut ProjectState,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    first_edge: EdgeId,
    second_edge: EdgeId,
) -> Result<EdgeIntersectionResponse, String> {
    let vertex_id = VertexId::new();
    let snapshot = execute_command(
        project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        Command::ConnectEdgeIntersection {
            first_edge,
            second_edge,
            new_vertex: vertex_id,
            first_new_edge: EdgeId::new(),
            second_new_edge: EdgeId::new(),
        },
    )?;
    Ok(EdgeIntersectionResponse {
        snapshot,
        vertex_id,
    })
}

#[tauri::command]
fn connect_intersection_cluster(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    targets: Vec<IntersectionClusterTargetRequest>,
    junction_vertex_id: Option<VertexId>,
) -> Result<EdgeIntersectionResponse, String> {
    validate_intersection_cluster_target_count(targets.len())?;
    let mut project = lock_project(&state)?;
    execute_intersection_cluster_connection(
        &mut project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        targets,
        junction_vertex_id,
    )
}

fn execute_intersection_cluster_connection(
    project: &mut ProjectState,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    targets: Vec<IntersectionClusterTargetRequest>,
    junction_vertex_id: Option<VertexId>,
) -> Result<EdgeIntersectionResponse, String> {
    validate_intersection_cluster_target_count(targets.len())?;
    let (junction, vertex_id) = match junction_vertex_id {
        Some(id) => (JunctionVertexIntent::Reuse { id }, id),
        None => {
            let id = VertexId::new();
            (JunctionVertexIntent::Create { id }, id)
        }
    };
    let targets = targets
        .into_iter()
        .map(|target| IntersectionEdgeTarget {
            edge: target.edge_id,
            new_edge: match target.relation {
                IntersectionClusterRelation::Interior => Some(EdgeId::new()),
                IntersectionClusterRelation::Endpoint => None,
            },
        })
        .collect();
    let snapshot = execute_command(
        project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        Command::ConnectIntersectionCluster { junction, targets },
    )?;
    Ok(EdgeIntersectionResponse {
        snapshot,
        vertex_id,
    })
}

fn validate_intersection_cluster_target_count(count: usize) -> Result<(), String> {
    if count < MIN_INTERSECTION_CLUSTER_TARGETS {
        return Err(format!(
            "an intersection cluster requires at least three target edges, found {count}"
        ));
    }
    if count > MAX_INTERSECTION_CLUSTER_TARGETS {
        return Err(format!(
            "an intersection cluster supports at most {MAX_INTERSECTION_CLUSTER_TARGETS} target edges, found {count}"
        ));
    }
    Ok(())
}

#[tauri::command]
fn connect_t_junction(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    first_edge: EdgeId,
    second_edge: EdgeId,
) -> Result<TJunctionResponse, String> {
    let mut project = lock_project(&state)?;
    execute_t_junction_connection(
        &mut project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        first_edge,
        second_edge,
    )
}

fn execute_t_junction_connection(
    project: &mut ProjectState,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    first_edge: EdgeId,
    second_edge: EdgeId,
) -> Result<TJunctionResponse, String> {
    let new_edge = EdgeId::new();
    let snapshot = execute_command(
        project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        Command::ConnectTJunction {
            first_edge,
            second_edge,
            new_edge,
        },
    )?;
    let vertex_id = snapshot
        .crease_pattern
        .edges
        .iter()
        .find(|edge| edge.id == new_edge)
        .map(|edge| edge.start)
        .expect("a successful T-junction command must create its requested edge");
    Ok(TJunctionResponse {
        snapshot,
        vertex_id,
    })
}

#[tauri::command]
fn split_boundary_edge(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    edge: EdgeId,
    fraction: f64,
) -> Result<ProjectSnapshot, String> {
    let mut project = lock_project(&state)?;
    execute_boundary_split(
        &mut project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        edge,
        fraction,
    )
}

fn execute_boundary_split(
    project: &mut ProjectState,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    edge: EdgeId,
    fraction: f64,
) -> Result<ProjectSnapshot, String> {
    execute_command(
        project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        Command::SplitBoundaryEdge {
            edge,
            new_vertex: VertexId::new(),
            new_edge: EdgeId::new(),
            fraction,
        },
    )
}

#[tauri::command]
fn remove_boundary_vertex(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    vertex: VertexId,
) -> Result<ProjectSnapshot, String> {
    let mut project = lock_project(&state)?;
    execute_command(
        &mut project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        Command::RemoveBoundaryVertex { vertex },
    )?;
    project.remove_vertex_coordinate_expression(vertex);
    Ok(snapshot(&project))
}

fn lock_project(state: &AppState) -> Result<MutexGuard<'_, ProjectState>, String> {
    state
        .0
        .lock()
        .map_err(|_| "the project state lock is poisoned".to_owned())
}

fn lock_fold_import(
    state: &FoldImportState,
) -> Result<MutexGuard<'_, Option<PendingFoldImport>>, String> {
    state
        .0
        .lock()
        .map_err(|_| "the FOLD import state lock is poisoned".to_owned())
}

fn lock_svg_import(state: &SvgImportState) -> Result<MutexGuard<'_, SvgImportSlot>, String> {
    state
        .0
        .lock()
        .map_err(|_| "the SVG import state lock is poisoned".to_owned())
}

fn stage_pending_fold_import(
    state: &FoldImportState,
    expected_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    bytes: Vec<u8>,
) -> Result<ProjectId, String> {
    let import_id = ProjectId::new();
    *lock_fold_import(state)? = Some(PendingFoldImport {
        import_id,
        expected_instance_id,
        expected_project_id,
        expected_revision,
        bytes: Arc::from(bytes),
    });
    Ok(import_id)
}

fn pending_fold_import(
    state: &FoldImportState,
    import_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
) -> Result<PendingFoldImport, String> {
    let pending = lock_fold_import(state)?;
    let pending = pending
        .as_ref()
        .ok_or_else(|| "the FOLD import preview is no longer available".to_owned())?;
    if pending.import_id != import_id {
        return Err("the FOLD import preview was replaced by a newer preview".to_owned());
    }
    if pending.expected_project_id != expected_project_id
        || pending.expected_revision != expected_revision
    {
        return Err("the FOLD import preview belongs to a different project state".to_owned());
    }
    Ok(pending.clone())
}

fn cancel_pending_fold_import(state: &FoldImportState, import_id: ProjectId) -> Result<(), String> {
    let mut pending = lock_fold_import(state)?;
    match pending.as_ref() {
        None => Ok(()),
        Some(current) if current.import_id == import_id => {
            *pending = None;
            Ok(())
        }
        Some(_) => Err("the FOLD import preview was replaced by a newer preview".to_owned()),
    }
}

fn commit_fold_import_replacement(
    project: &mut ProjectState,
    pending_slot: &mut Option<PendingFoldImport>,
    import_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    replacement: ProjectState,
) -> Result<ProjectSnapshot, String> {
    let pending = pending_slot
        .as_ref()
        .ok_or_else(|| "the FOLD import preview is no longer available".to_owned())?;
    if pending.import_id != import_id {
        return Err("the FOLD import preview was replaced by a newer preview".to_owned());
    }
    if pending.expected_project_id != expected_project_id
        || pending.expected_revision != expected_revision
    {
        return Err("the FOLD import preview belongs to a different project state".to_owned());
    }
    ensure_expected_project(
        project,
        pending.expected_instance_id,
        pending.expected_project_id,
        pending.expected_revision,
    )?;
    commit_project_replacement(project, replacement).map_err(|error| error.to_string())?;
    *pending_slot = None;
    Ok(snapshot(project))
}

fn stage_pending_svg_import(
    state: &SvgImportState,
    expected_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    bytes: Vec<u8>,
) -> Result<ProjectId, String> {
    let import_id = ProjectId::new();
    let mut slot = lock_svg_import(state)?;
    slot.validation_generation_id = None;
    slot.validation = None;
    slot.last_cancelled_id = None;
    slot.pending = Some(PendingSvgImport {
        import_id,
        expected_instance_id,
        expected_project_id,
        expected_revision,
        bytes: Arc::from(bytes),
    });
    Ok(import_id)
}

#[cfg(test)]
fn pending_svg_import(
    state: &SvgImportState,
    import_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
) -> Result<PendingSvgImport, String> {
    let slot = lock_svg_import(state)?;
    Ok(
        pending_svg_import_in_slot(&slot, import_id, expected_project_id, expected_revision)?
            .clone(),
    )
}

fn pending_svg_import_in_slot(
    slot: &SvgImportSlot,
    import_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
) -> Result<&PendingSvgImport, String> {
    let pending = slot
        .pending
        .as_ref()
        .ok_or_else(|| "the SVG import preview is no longer available".to_owned())?;
    if pending.import_id != import_id {
        return Err("the SVG import preview was replaced by a newer preview".to_owned());
    }
    if pending.expected_project_id != expected_project_id
        || pending.expected_revision != expected_revision
    {
        return Err("the SVG import preview belongs to a different project state".to_owned());
    }
    Ok(pending)
}

fn begin_svg_import_settings_validation(
    state: &SvgImportState,
    validation_id: ProjectId,
    import_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
) -> Result<PendingSvgImport, String> {
    let mut slot = lock_svg_import(state)?;
    let pending =
        pending_svg_import_in_slot(&slot, import_id, expected_project_id, expected_revision)?
            .clone();
    slot.validation_generation_id = Some(validation_id);
    slot.validation = None;
    Ok(pending)
}

fn abandon_svg_import_settings_validation(
    state: &SvgImportState,
    validation_id: ProjectId,
) -> Result<(), String> {
    let mut slot = lock_svg_import(state)?;
    if slot.validation_generation_id == Some(validation_id) {
        slot.validation_generation_id = None;
        slot.validation = None;
    }
    Ok(())
}

fn ensure_svg_import_settings_validation(
    slot: &SvgImportSlot,
    pending: &PendingSvgImport,
    validation_id: ProjectId,
    boundary_candidate: Option<SvgBoundaryCandidateId>,
    millimeters_per_unit: f64,
    group_mappings: &[SvgGroupMapping],
) -> Result<(), String> {
    let validation = slot
        .validation
        .as_ref()
        .ok_or_else(|| "the SVG import settings have not been validated".to_owned())?;
    if slot.validation_generation_id != Some(validation_id)
        || validation.validation_id != validation_id
        || validation.import_id != pending.import_id
        || validation.expected_instance_id != pending.expected_instance_id
        || validation.expected_project_id != pending.expected_project_id
        || validation.expected_revision != pending.expected_revision
        || validation.millimeters_per_unit_bits != millimeters_per_unit.to_bits()
        || validation.boundary_candidate != boundary_candidate
        || validation.group_mappings != group_mappings
    {
        return Err("the SVG import settings changed after validation".to_owned());
    }
    Ok(())
}

fn complete_svg_import_settings_validation(
    slot: &mut SvgImportSlot,
    project: &ProjectState,
    completion: SvgImportSettingsValidationCompletion,
) -> Result<SvgImportSettingsValidationResponse, String> {
    let validation = &completion.validation;
    let validation_id = validation.validation_id;
    if slot.validation_generation_id != Some(validation_id) {
        return Err("the SVG import settings validation was superseded".to_owned());
    }
    let current = pending_svg_import_in_slot(
        slot,
        validation.import_id,
        validation.expected_project_id,
        validation.expected_revision,
    )?;
    if current.expected_instance_id != validation.expected_instance_id {
        return Err("the SVG import preview was replaced by a newer preview".to_owned());
    }
    ensure_expected_project(
        project,
        validation.expected_instance_id,
        validation.expected_project_id,
        validation.expected_revision,
    )?;

    let response = SvgImportSettingsValidationResponse {
        validation_id,
        preview_id: validation.import_id,
        expected_project_id: validation.expected_project_id,
        expected_revision: validation.expected_revision,
        millimeters_per_unit: f64::from_bits(validation.millimeters_per_unit_bits),
        boundary_candidate_id: validation.boundary_candidate.map(|candidate| candidate.0),
        width_mm: completion.geometry.width_mm,
        height_mm: completion.geometry.height_mm,
        has_cuts: completion.geometry.has_cuts,
    };
    slot.validation = Some(completion.validation);
    Ok(response)
}

fn cancel_pending_svg_import(state: &SvgImportState, import_id: ProjectId) -> Result<(), String> {
    let mut slot = lock_svg_import(state)?;
    match slot.pending.as_ref() {
        None if slot.last_cancelled_id == Some(import_id) => Ok(()),
        None => Err("the SVG import preview is no longer available".to_owned()),
        Some(current) if current.import_id == import_id => {
            slot.pending = None;
            slot.validation_generation_id = None;
            slot.validation = None;
            slot.last_cancelled_id = Some(import_id);
            Ok(())
        }
        Some(_) => Err("the SVG import preview was replaced by a newer preview".to_owned()),
    }
}

fn commit_svg_import_replacement(
    project: &mut ProjectState,
    pending_slot: &mut Option<PendingSvgImport>,
    import_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    replace_dirty_project_confirmed: bool,
    replacement: ProjectState,
) -> Result<ProjectSnapshot, String> {
    let pending = pending_slot
        .as_ref()
        .ok_or_else(|| "the SVG import preview is no longer available".to_owned())?;
    if pending.import_id != import_id {
        return Err("the SVG import preview was replaced by a newer preview".to_owned());
    }
    if pending.expected_project_id != expected_project_id
        || pending.expected_revision != expected_revision
    {
        return Err("the SVG import preview belongs to a different project state".to_owned());
    }
    ensure_expected_project(
        project,
        pending.expected_instance_id,
        pending.expected_project_id,
        pending.expected_revision,
    )?;
    if project.is_dirty() && !replace_dirty_project_confirmed {
        return Err("replacing a dirty project requires explicit confirmation".to_owned());
    }

    commit_project_replacement(project, replacement).map_err(|error| error.to_string())?;
    *pending_slot = None;
    Ok(snapshot(project))
}

fn execute_command(
    project: &mut ProjectState,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    command: Command,
) -> Result<ProjectSnapshot, String> {
    ensure_project_instance_identity(project, expected_project_instance_id, expected_project_id)?;
    if project.editor.revision() != expected_revision
        || project.editor.revision() == ori_core::MAX_REVISION
    {
        project
            .editor
            .execute(expected_revision, command)
            .map_err(|error| error.to_string())?;
        project.record_numeric_expression_edit();
        project.reconcile_vertex_coordinate_expressions();
        return Ok(snapshot(project));
    }
    let authority = project.applied_pose_authority.clone();
    let invalidation = authority
        .begin_invalidation()
        .map_err(|error| error.to_string())?;
    project
        .editor
        .execute(expected_revision, command)
        .map_err(|error| error.to_string())?;
    project.record_numeric_expression_edit();
    project.reconcile_vertex_coordinate_expressions();
    invalidation.commit();
    Ok(snapshot(project))
}

fn replace_with_new_project(
    project: &mut ProjectState,
    expected_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    parameters: NewProjectParameters,
) -> Result<ProjectSnapshot, String> {
    ensure_expected_project(
        project,
        expected_instance_id,
        expected_project_id,
        expected_revision,
    )?;

    let replacement = create_new_project_state(parameters)?;
    commit_project_replacement(project, replacement).map_err(|error| error.to_string())?;
    Ok(snapshot(project))
}

fn create_new_project_state(parameters: NewProjectParameters) -> Result<ProjectState, String> {
    let name = normalize_project_name(&parameters.name)?;
    validate_paper_thickness(parameters.thickness_mm)?;
    let sheet = create_rectangular_sheet(
        parameters.width_mm,
        parameters.height_mm,
        parameters.cutting_allowed,
    )
    .map_err(|error| format!("failed to create the paper sheet: {error}"))?;
    let (pattern, mut paper) = sheet.into_parts();
    paper.thickness_mm = parameters.thickness_mm;
    paper.front.color = parameters.front_color;
    paper.back.color = parameters.back_color;

    if !validate_paper(&paper, &pattern).is_valid() {
        return Err("the generated paper failed final validation".to_owned());
    }

    let mut project = ProjectState::new_unsaved(name, pattern, paper);
    project.numeric_expressions.rectangular_paper_creation =
        Some(RectangularPaperCreationExpressions::new(
            parameters.width_expression,
            parameters.height_expression,
            parameters.width_mm,
            parameters.height_mm,
        ));
    Ok(project)
}

fn normalize_project_name(name: &str) -> Result<String, String> {
    let trimmed = name.trim();
    let character_count = trimmed.chars().count();
    if !(1..=MAX_PROJECT_NAME_CHARS).contains(&character_count) {
        return Err(format!(
            "project name must contain between 1 and {MAX_PROJECT_NAME_CHARS} characters after trimming"
        ));
    }
    if trimmed.chars().any(char::is_control) {
        return Err("project name must not contain control characters".to_owned());
    }
    Ok(trimmed.to_owned())
}

fn validate_import_scale(millimeters_per_unit: f64) -> Result<(), String> {
    if !millimeters_per_unit.is_finite() || millimeters_per_unit <= 0.0 {
        return Err("import scale must be a finite number greater than zero".to_owned());
    }
    if millimeters_per_unit > 1_000_000_000.0 {
        return Err("import scale must not exceed 1,000,000,000 mm per unit".to_owned());
    }
    Ok(())
}

fn validate_fold_import_mapping_requests(
    mappings: Vec<FoldImportAssignmentMappingRequest>,
) -> Result<HashMap<String, FoldImportTargetRequest>, String> {
    let mut validated = HashMap::with_capacity(mappings.len());
    for mapping in mappings {
        let source = mapping.source.as_str();
        let allowed = match source {
            "M" => matches!(mapping.target, FoldImportTargetRequest::Mountain),
            "V" => matches!(mapping.target, FoldImportTargetRequest::Valley),
            "F" => matches!(
                mapping.target,
                FoldImportTargetRequest::Auxiliary | FoldImportTargetRequest::Ignore
            ),
            "U" => matches!(
                mapping.target,
                FoldImportTargetRequest::Mountain
                    | FoldImportTargetRequest::Valley
                    | FoldImportTargetRequest::Auxiliary
                    | FoldImportTargetRequest::Ignore
            ),
            "C" => matches!(
                mapping.target,
                FoldImportTargetRequest::Cut | FoldImportTargetRequest::Ignore
            ),
            "J" => matches!(
                mapping.target,
                FoldImportTargetRequest::Auxiliary | FoldImportTargetRequest::Ignore
            ),
            _ => {
                return Err(format!(
                    "unsupported FOLD assignment mapping source {source:?}"
                ));
            }
        };
        if !allowed {
            return Err(format!(
                "FOLD assignment {source} cannot be imported as {:?}",
                mapping.target
            ));
        }
        if validated
            .insert(mapping.source.clone(), mapping.target)
            .is_some()
        {
            return Err(format!(
                "FOLD assignment {source} was mapped more than once"
            ));
        }
    }
    Ok(validated)
}

fn validate_paper_thickness(thickness_mm: f64) -> Result<(), String> {
    if !thickness_mm.is_finite() {
        return Err("paper thickness must be finite".to_owned());
    }
    if thickness_mm < 0.0 {
        return Err("paper thickness must be zero or greater".to_owned());
    }
    Ok(())
}

fn ensure_project_identity(
    project: &ProjectState,
    expected_project_id: ProjectId,
) -> Result<(), String> {
    if project.project_id == expected_project_id {
        Ok(())
    } else {
        Err("the active project changed before the command was applied".to_owned())
    }
}

fn ensure_project_instance_identity(
    project: &ProjectState,
    expected_instance_id: ProjectId,
    expected_project_id: ProjectId,
) -> Result<(), String> {
    if project.instance_id != expected_instance_id {
        return Err("the open project instance changed while the file dialog was open".to_owned());
    }
    ensure_project_identity(project, expected_project_id)
}

fn ensure_expected_project(
    project: &ProjectState,
    expected_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
) -> Result<(), String> {
    ensure_project_instance_identity(project, expected_instance_id, expected_project_id)?;
    if project.editor.revision() == expected_revision {
        Ok(())
    } else {
        Err("the project changed while the file dialog was open".to_owned())
    }
}

fn capture_topology_input(
    project: &ProjectState,
    expected_project_id: ProjectId,
    expected_revision: u64,
) -> Result<TopologyAnalysisInput, String> {
    ensure_project_identity(project, expected_project_id)?;
    if project.editor.revision() != expected_revision {
        return Err(format!(
            "expected revision {expected_revision}, but the current revision is {}",
            project.editor.revision()
        ));
    }
    Ok(project.editor.topology_analysis_input(project.project_id))
}

struct AnalyzedInstructionPose {
    project_instance_id: ProjectId,
    input: TopologyAnalysisInput,
    topology: EditorTopology,
    fixed_face: Option<FaceId>,
    hinge_angles: Vec<InstructionHingeAngle>,
}

async fn analyze_instruction_pose(
    state: &AppState,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    fixed_face: Option<FaceId>,
    hinge_angles: Vec<InstructionHingeAngle>,
) -> Result<AnalyzedInstructionPose, String> {
    if hinge_angles.len() > MAX_INSTRUCTION_HINGES_PER_STEP {
        return Err(format!(
            "an instruction step may contain at most {MAX_INSTRUCTION_HINGES_PER_STEP} hinges"
        ));
    }
    validate_instruction_hinge_angle_values(&hinge_angles)?;
    let (project_instance_id, input) = {
        let project = lock_project(state)?;
        ensure_expected_project(
            &project,
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
        )?;
        (
            project.instance_id,
            capture_topology_input(&project, expected_project_id, expected_revision)?,
        )
    };
    let (input, topology) = tauri::async_runtime::spawn_blocking(move || {
        let topology = input.analyze();
        (input, topology)
    })
    .await
    .map_err(instruction_topology_analysis_task_error)?;

    Ok(AnalyzedInstructionPose {
        project_instance_id,
        input,
        topology,
        fixed_face,
        hinge_angles,
    })
}

fn finish_instruction_pose(
    project: &ProjectState,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    analyzed: AnalyzedInstructionPose,
) -> Result<InstructionPose, String> {
    ensure_expected_project(
        project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
    )?;
    if project.instance_id != analyzed.project_instance_id {
        return Err(
            "the open project instance changed while the instruction pose was being analyzed"
                .to_owned(),
        );
    }
    if !analyzed
        .input
        .is_current_for(project.project_id, &project.editor)
    {
        return Err("the project changed while the instruction pose was being analyzed".to_owned());
    }
    if analyzed.topology.revision() != analyzed.input.revision() {
        return Err("instruction topology returned an unexpected revision".to_owned());
    }
    let topology = analyzed
        .topology
        .simulation_snapshot()
        .ok_or_else(|| "the current crease pattern cannot produce a foldable pose".to_owned())?;

    let topology = prepare_instruction_topology(topology)?;
    instruction_pose_from_context(
        &topology,
        project.editor.fold_model_fingerprint_v1(),
        analyzed.fixed_face,
        analyzed.hinge_angles,
    )
}

struct InstructionTopologyContext {
    face_ids: HashSet<FaceId>,
    expected_edges: Vec<EdgeId>,
    planar: bool,
}

fn prepare_instruction_topology(
    topology: &TopologySnapshot,
) -> Result<InstructionTopologyContext, String> {
    if topology.faces.is_empty() {
        return Err("an instruction pose requires at least one material face".to_owned());
    }
    if topology.hinge_adjacency.len() > MAX_INSTRUCTION_HINGES_PER_STEP {
        return Err(format!(
            "an instruction fold model may contain at most {MAX_INSTRUCTION_HINGES_PER_STEP} hinges"
        ));
    }

    let face_ids = topology
        .faces
        .iter()
        .map(|face| face.id)
        .collect::<HashSet<_>>();
    if face_ids.len() != topology.faces.len() {
        return Err("the fold model contains a duplicate material face".to_owned());
    }

    let planar = topology.hinge_adjacency.is_empty();
    if planar {
        if topology.faces.len() != 1 {
            return Err(
                "a hinge-free instruction pose must contain exactly one material face".to_owned(),
            );
        }
    } else {
        if topology.hinge_adjacency.len() + 1 != topology.faces.len() {
            return Err("instruction poses currently require a tree-shaped fold graph".to_owned());
        }
        let mut adjacency = face_ids
            .iter()
            .copied()
            .map(|face| (face, Vec::new()))
            .collect::<HashMap<_, _>>();
        for hinge in &topology.hinge_adjacency {
            if hinge.first == hinge.second
                || !face_ids.contains(&hinge.first)
                || !face_ids.contains(&hinge.second)
            {
                return Err("the fold model contains an invalid hinge face reference".to_owned());
            }
            adjacency
                .get_mut(&hinge.first)
                .expect("validated first hinge face must exist")
                .push(hinge.second);
            adjacency
                .get_mut(&hinge.second)
                .expect("validated second hinge face must exist")
                .push(hinge.first);
        }

        let mut reached = HashSet::with_capacity(topology.faces.len());
        let mut pending = vec![topology.faces[0].id];
        while let Some(face) = pending.pop() {
            if !reached.insert(face) {
                continue;
            }
            pending.extend(
                adjacency
                    .get(&face)
                    .expect("validated material face must have an adjacency entry")
                    .iter()
                    .copied(),
            );
        }
        if reached != face_ids {
            return Err("instruction poses currently require a connected fold graph".to_owned());
        }
    }

    let mut expected_edges = topology
        .hinge_adjacency
        .iter()
        .map(|hinge| hinge.edge)
        .collect::<Vec<_>>();
    expected_edges.sort_by_key(EdgeId::canonical_bytes);
    if expected_edges.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err("the fold model contains a duplicate hinge edge".to_owned());
    }

    Ok(InstructionTopologyContext {
        face_ids,
        expected_edges,
        planar,
    })
}

#[cfg(test)]
fn instruction_pose_from_topology(
    topology: &TopologySnapshot,
    source_model_fingerprint: String,
    fixed_face: Option<FaceId>,
    hinge_angles: Vec<InstructionHingeAngle>,
) -> Result<InstructionPose, String> {
    let topology = prepare_instruction_topology(topology)?;
    instruction_pose_from_context(
        &topology,
        source_model_fingerprint,
        fixed_face,
        hinge_angles,
    )
}

fn instruction_pose_from_context(
    topology: &InstructionTopologyContext,
    source_model_fingerprint: String,
    fixed_face: Option<FaceId>,
    mut hinge_angles: Vec<InstructionHingeAngle>,
) -> Result<InstructionPose, String> {
    if hinge_angles.len() > MAX_INSTRUCTION_HINGES_PER_STEP {
        return Err(format!(
            "an instruction step may contain at most {MAX_INSTRUCTION_HINGES_PER_STEP} hinges"
        ));
    }
    validate_instruction_hinge_angle_values(&hinge_angles)?;
    if topology.planar {
        if fixed_face.is_some() {
            return Err("a planar instruction pose must not specify a fixed face".to_owned());
        }
        if !hinge_angles.is_empty() {
            return Err("a planar instruction pose must not contain hinge angles".to_owned());
        }
    } else {
        let fixed_face = fixed_face
            .ok_or_else(|| "a folded instruction pose requires a fixed face".to_owned())?;
        if !topology.face_ids.contains(&fixed_face) {
            return Err("the fixed face does not exist in the current fold model".to_owned());
        }
    }

    hinge_angles.sort_by_key(|hinge| hinge.edge.canonical_bytes());
    if hinge_angles.len() != topology.expected_edges.len()
        || hinge_angles
            .iter()
            .zip(&topology.expected_edges)
            .any(|(angle, expected)| angle.edge != *expected)
    {
        return Err(
            "the instruction pose must contain every current hinge exactly once".to_owned(),
        );
    }
    Ok(InstructionPose {
        model: InstructionPoseModel::AbsoluteHingeAnglesV1,
        source_model_fingerprint,
        fixed_face,
        hinge_angles,
    })
}

fn validate_instruction_hinge_angle_values(
    hinge_angles: &[InstructionHingeAngle],
) -> Result<(), String> {
    if hinge_angles
        .iter()
        .any(|hinge| !hinge.angle_degrees.is_finite())
    {
        return Err("instruction hinge angles must be finite".to_owned());
    }
    if hinge_angles
        .iter()
        .any(|hinge| !(0.0..=180.0).contains(&hinge.angle_degrees))
    {
        return Err("instruction hinge angles must be between 0 and 180 degrees".to_owned());
    }
    Ok(())
}

fn finish_topology_response(
    project: &ProjectState,
    input: &TopologyAnalysisInput,
    topology: ori_core::EditorTopology,
) -> Result<ProjectTopologyResponse, String> {
    if !input.is_current_for(project.project_id, &project.editor) {
        return Err("the project changed while topology was being analyzed".to_owned());
    }
    if topology.revision() != input.revision() {
        return Err("topology analysis returned an unexpected revision".to_owned());
    }

    let simulation_ready = topology.is_simulation_ready();
    let report = topology.into_report();
    if report
        .snapshot
        .as_ref()
        .is_some_and(|snapshot| snapshot.source_revision != input.revision())
    {
        return Err("topology snapshot returned an unexpected source revision".to_owned());
    }
    Ok(ProjectTopologyResponse {
        project_id: project.project_id,
        revision: input.revision(),
        simulation_ready,
        snapshot: simulation_ready.then_some(report.snapshot).flatten(),
        issues: report.issues,
    })
}

fn snapshot(project: &ProjectState) -> ProjectSnapshot {
    ProjectSnapshot {
        project_instance_id: project.instance_id,
        project_id: project.project_id,
        name: project.name.clone(),
        memo: project.editor.project_memo().to_owned(),
        current_path: project
            .current_path
            .as_deref()
            .map(|path| path.to_string_lossy().into_owned()),
        revision: project.editor.revision(),
        saved_revision: project.saved_revision,
        is_dirty: project.is_dirty(),
        paper: project.editor.paper().clone(),
        crease_pattern: project.editor.pattern().clone(),
        instruction_timeline: project.editor.instruction_timeline().clone(),
        numeric_expressions: project.numeric_expressions.clone(),
        geometric_constraints: project.editor.geometric_constraints().clone(),
        project_layers: project.editor.project_layers().clone(),
        element_metadata: project.editor.element_metadata().clone(),
        fold_model_fingerprint: project.editor.fold_model_fingerprint_v1(),
        can_undo: project.editor.can_undo(),
        can_redo: project.editor.can_redo(),
        cutting_allowed: project.editor.cutting_allowed(),
    }
}

fn canceled_file_response(state: &AppState) -> Result<ProjectFileResponse, String> {
    let project = lock_project(state)?;
    Ok(ProjectFileResponse {
        canceled: true,
        project: snapshot(&project),
    })
}

fn save_project_with_dialog(
    app: &AppHandle,
    state: &AppState,
) -> Result<ProjectFileResponse, String> {
    let (
        expected_instance_id,
        expected_project_id,
        expected_revision,
        initial_directory,
        suggested_name,
    ) = {
        let project = lock_project(state)?;
        (
            project.instance_id,
            project.project_id,
            project.editor.revision(),
            project
                .current_path
                .as_deref()
                .and_then(Path::parent)
                .map(Path::to_path_buf),
            suggested_file_name(&project.name),
        )
    };

    let mut dialog = app
        .dialog()
        .file()
        .add_filter("ORIGAMI2 project", &["ori2"])
        .set_file_name(suggested_name)
        .set_title("Save ORIGAMI2 project");
    if let Some(directory) = initial_directory {
        dialog = dialog.set_directory(directory);
    }

    let Some(selected) = dialog.blocking_save_file() else {
        return canceled_file_response(state);
    };
    let path = selected
        .simplified()
        .into_path()
        .map_err(project_save_target_conversion_error)?;
    let mut project = lock_project(state)?;
    save_project_as_selected_path(
        &mut project,
        expected_instance_id,
        expected_project_id,
        expected_revision,
        path,
    )
}

fn project_save_target_conversion_error<T>(_: T) -> String {
    "選択された保存先はローカルファイルではありません。".to_owned()
}

fn save_project_as_selected_path(
    project: &mut ProjectState,
    expected_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    selected_path: PathBuf,
) -> Result<ProjectFileResponse, String> {
    ensure_expected_project(
        project,
        expected_instance_id,
        expected_project_id,
        expected_revision,
    )?;
    save_project_to_destination(project, ensure_ori2_extension(selected_path)?)
}

fn save_project_to_path(
    project: &mut ProjectState,
    path: PathBuf,
) -> Result<ProjectFileResponse, String> {
    save_project_to_destination(project, save_path::DialogSaveDestination::confirmed(path))
}

fn save_project_to_destination(
    project: &mut ProjectState,
    destination: save_path::DialogSaveDestination,
) -> Result<ProjectFileResponse, String> {
    let archive = project.project_archive()?;
    persist_project_archive_to_destination(&destination, &archive)?;
    let path = destination.into_path();
    project.current_path = Some(path);
    project.saved_revision = Some(project.editor.revision());
    project.saved_document = Some(project.document());
    Ok(ProjectFileResponse {
        canceled: false,
        project: snapshot(project),
    })
}

fn load_project_file(path: PathBuf) -> Result<LoadedProjectFile, String> {
    let archive = load_project_archive_from_path(&path)?;
    validate_loaded_numeric_expression_bindings(&archive.document)?;
    let replacement = ProjectState::from_project_archive(archive, path)?;
    Ok(LoadedProjectFile { replacement })
}

fn validate_loaded_numeric_expression_bindings(document: &ProjectDocument) -> Result<(), String> {
    for binding in document
        .numeric_expressions
        .rectangular_paper_creation
        .iter()
        .chain(document.numeric_expressions.undo_stack.iter().flatten())
        .chain(document.numeric_expressions.redo_stack.iter().flatten())
    {
        validate_loaded_numeric_expression_binding(binding)?;
    }
    for binding in &document.numeric_expressions.vertex_coordinates {
        validate_coordinate_expression_pair(
            &binding.x_source,
            &binding.y_source,
            binding.adopted_x_mm,
            binding.adopted_y_mm,
        )?;
        let matching = document
            .crease_pattern
            .vertices
            .iter()
            .filter(|vertex| vertex.id == binding.vertex)
            .collect::<Vec<_>>();
        if matching.len() != 1
            || matching[0].position.x.to_bits() != binding.adopted_x_mm.to_bits()
            || matching[0].position.y.to_bits() != binding.adopted_y_mm.to_bits()
        {
            return Err(PROJECT_NUMERIC_EXPRESSIONS_INVALID_MESSAGE.to_owned());
        }
        if let Some(polar) = &binding.polar_construction {
            let (length_mm, angle_degrees) = evaluate_finite_millimetre_pair(
                polar.length_source.clone(),
                polar.angle_degrees_source.clone(),
            )
            .map_err(map_loaded_numeric_expression_error)?;
            let radians = angle_degrees.to_radians();
            if length_mm.to_bits() != polar.adopted_length_mm.to_bits()
                || angle_degrees.to_bits() != polar.adopted_angle_degrees.to_bits()
                || (polar.adopted_start_x_mm + length_mm * radians.cos()).to_bits()
                    != binding.adopted_x_mm.to_bits()
                || (polar.adopted_start_y_mm + length_mm * radians.sin()).to_bits()
                    != binding.adopted_y_mm.to_bits()
            {
                return Err(PROJECT_NUMERIC_EXPRESSIONS_INVALID_MESSAGE.to_owned());
            }
        }
    }
    for transition in document
        .numeric_expressions
        .vertex_undo_stack
        .iter()
        .chain(&document.numeric_expressions.vertex_redo_stack)
        .flatten()
    {
        for binding in transition
            .changes
            .iter()
            .flat_map(|change| change.before.iter().chain(change.after.iter()))
        {
            validate_coordinate_expression_pair(
                &binding.x_source,
                &binding.y_source,
                binding.adopted_x_mm,
                binding.adopted_y_mm,
            )?;
            if let Some(polar) = &binding.polar_construction {
                let (length_mm, angle_degrees) = evaluate_finite_millimetre_pair(
                    polar.length_source.clone(),
                    polar.angle_degrees_source.clone(),
                )
                .map_err(map_loaded_numeric_expression_error)?;
                let radians = angle_degrees.to_radians();
                if length_mm.to_bits() != polar.adopted_length_mm.to_bits()
                    || angle_degrees.to_bits() != polar.adopted_angle_degrees.to_bits()
                    || (polar.adopted_start_x_mm + length_mm * radians.cos()).to_bits()
                        != binding.adopted_x_mm.to_bits()
                    || (polar.adopted_start_y_mm + length_mm * radians.sin()).to_bits()
                        != binding.adopted_y_mm.to_bits()
                {
                    return Err(PROJECT_NUMERIC_EXPRESSIONS_INVALID_MESSAGE.to_owned());
                }
            }
        }
    }
    Ok(())
}

fn validate_loaded_numeric_expression_binding(
    binding: &RectangularPaperCreationExpressions,
) -> Result<(), String> {
    let (width_mm, height_mm) = evaluate_positive_millimetre_pair(
        binding.width_source.clone(),
        binding.height_source.clone(),
    )
    .map_err(map_loaded_numeric_expression_error)?;
    if width_mm.to_bits() != binding.adopted_width_mm.to_bits()
        || height_mm.to_bits() != binding.adopted_height_mm.to_bits()
    {
        return Err(PROJECT_NUMERIC_EXPRESSIONS_INVALID_MESSAGE.to_owned());
    }
    Ok(())
}

fn validate_coordinate_expression_pair(
    x_source: &str,
    y_source: &str,
    adopted_x_mm: f64,
    adopted_y_mm: f64,
) -> Result<(), String> {
    let (x_mm, y_mm) = evaluate_finite_millimetre_pair(x_source.to_owned(), y_source.to_owned())
        .map_err(map_loaded_numeric_expression_error)?;
    if x_mm.to_bits() != adopted_x_mm.to_bits() || y_mm.to_bits() != adopted_y_mm.to_bits() {
        return Err(PROJECT_NUMERIC_EXPRESSIONS_INVALID_MESSAGE.to_owned());
    }
    Ok(())
}

fn map_loaded_numeric_expression_error(error: PositiveMillimetrePairError) -> String {
    if error.is_worker_busy() {
        PROJECT_NUMERIC_EXPRESSIONS_BUSY_MESSAGE.to_owned()
    } else {
        PROJECT_NUMERIC_EXPRESSIONS_INVALID_MESSAGE.to_owned()
    }
}

fn apply_loaded_project_file(
    project: &mut ProjectState,
    expected_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    loaded: LoadedProjectFile,
) -> Result<ProjectFileResponse, String> {
    ensure_expected_project(
        project,
        expected_instance_id,
        expected_project_id,
        expected_revision,
    )?;
    commit_project_replacement(project, loaded.replacement).map_err(|error| error.to_string())?;
    Ok(ProjectFileResponse {
        canceled: false,
        project: snapshot(project),
    })
}

fn read_fold_import_bytes(path: &Path) -> Result<Vec<u8>, String> {
    let file = File::open(path).map_err(|_| FOLD_FILE_OPEN_FAILED_MESSAGE.to_owned())?;
    let declared_size = file
        .metadata()
        .map_err(|_| FOLD_FILE_INSPECTION_FAILED_MESSAGE.to_owned())?
        .len();
    if declared_size > MAX_FOLD_IMPORT_FILE_SIZE {
        return Err(FOLD_FILE_TOO_LARGE_MESSAGE.to_owned());
    }

    let capacity = usize::try_from(declared_size)
        .unwrap_or(0)
        .min(usize::try_from(MAX_FOLD_IMPORT_FILE_SIZE).unwrap_or(usize::MAX));
    let mut bytes = Vec::with_capacity(capacity);
    file.take(MAX_FOLD_IMPORT_FILE_SIZE.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| FOLD_FILE_READ_FAILED_MESSAGE.to_owned())?;
    if bytes.len() as u64 > MAX_FOLD_IMPORT_FILE_SIZE {
        return Err(FOLD_FILE_TOO_LARGE_MESSAGE.to_owned());
    }
    Ok(bytes)
}

fn read_svg_import_bytes(path: &Path) -> Result<Vec<u8>, String> {
    let file = File::open(path).map_err(|_| SVG_FILE_OPEN_FAILED_MESSAGE.to_owned())?;
    let declared_size = file
        .metadata()
        .map_err(|_| SVG_FILE_INSPECTION_FAILED_MESSAGE.to_owned())?
        .len();
    if declared_size > MAX_SVG_IMPORT_FILE_SIZE {
        return Err(SVG_FILE_TOO_LARGE_MESSAGE.to_owned());
    }

    let capacity = usize::try_from(declared_size)
        .unwrap_or(0)
        .min(usize::try_from(MAX_SVG_IMPORT_FILE_SIZE).unwrap_or(usize::MAX));
    let mut bytes = Vec::with_capacity(capacity);
    file.take(MAX_SVG_IMPORT_FILE_SIZE.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| SVG_FILE_READ_FAILED_MESSAGE.to_owned())?;
    if bytes.len() as u64 > MAX_SVG_IMPORT_FILE_SIZE {
        return Err(SVG_FILE_TOO_LARGE_MESSAGE.to_owned());
    }
    Ok(bytes)
}

fn load_fold_import_preview(path: &Path) -> Result<(Vec<u8>, FoldPreview), String> {
    let bytes = read_fold_import_bytes(path)?;
    let preview = read_fold_preview(&bytes).map_err(fold_file_invalid_error)?;
    Ok((bytes, preview))
}

fn load_svg_import_preview(path: &Path) -> Result<(Vec<u8>, SvgPreview), String> {
    let bytes = read_svg_import_bytes(path)?;
    let preview = read_svg_preview(&bytes).map_err(|_| SVG_FILE_INVALID_MESSAGE.to_owned())?;
    Ok((bytes, preview))
}

fn fold_import_preview_snapshot(
    import_id: ProjectId,
    preview: &FoldPreview,
) -> FoldImportPreviewSnapshot {
    let counts = preview.assignment_counts();
    let assignments = [
        (FoldEdgeAssignment::Boundary, counts.boundary),
        (FoldEdgeAssignment::Mountain, counts.mountain),
        (FoldEdgeAssignment::Valley, counts.valley),
        (FoldEdgeAssignment::Flat, counts.flat),
        (FoldEdgeAssignment::Unassigned, counts.unassigned),
        (FoldEdgeAssignment::Cut, counts.cut),
        (FoldEdgeAssignment::Join, counts.join),
    ]
    .into_iter()
    .filter(|(_, count)| *count > 0)
    .map(|(assignment, count)| FoldImportAssignmentSummary {
        assignment: assignment.token().to_owned(),
        count,
    })
    .collect();

    let boundary_candidates = preview
        .boundary_candidates()
        .iter()
        .map(|candidate| FoldImportBoundaryCandidateSnapshot {
            id: candidate.id.0,
            source: match candidate.source {
                FoldBoundaryCandidateSource::AssignedBoundary => "assigned_boundary",
                FoldBoundaryCandidateSource::InferredOuterFace => "inferred_outer_face",
            },
            edge_indices: candidate.edge_indices.clone(),
        })
        .collect::<Vec<_>>();
    let boundary_edge_indices = preview
        .boundary_candidates()
        .iter()
        .flat_map(|candidate| candidate.edge_indices.iter().copied())
        .collect::<HashSet<_>>();
    let mut selected_edges = preview
        .edges()
        .iter()
        .filter(|edge| boundary_edge_indices.contains(&edge.index))
        .take(MAX_FOLD_IMPORT_PREVIEW_EDGES)
        .collect::<Vec<_>>();
    let sampled_assignments = [
        FoldEdgeAssignment::Mountain,
        FoldEdgeAssignment::Valley,
        FoldEdgeAssignment::Flat,
        FoldEdgeAssignment::Unassigned,
        FoldEdgeAssignment::Cut,
        FoldEdgeAssignment::Join,
    ];
    let buckets = sampled_assignments
        .iter()
        .map(|assignment| {
            preview
                .edges()
                .iter()
                .filter(|edge| {
                    edge.assignment == *assignment && !boundary_edge_indices.contains(&edge.index)
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let mut bucket_offsets = vec![0_usize; buckets.len()];
    while selected_edges.len() < MAX_FOLD_IMPORT_PREVIEW_EDGES {
        let mut progressed = false;
        for (bucket_index, bucket) in buckets.iter().enumerate() {
            if selected_edges.len() == MAX_FOLD_IMPORT_PREVIEW_EDGES {
                break;
            }
            let offset = &mut bucket_offsets[bucket_index];
            if let Some(edge) = bucket.get(*offset) {
                selected_edges.push(*edge);
                *offset += 1;
                progressed = true;
            }
        }
        if !progressed {
            break;
        }
    }
    selected_edges.sort_unstable_by_key(|edge| edge.index);
    let mut source_vertex_indices = selected_edges
        .iter()
        .flat_map(|edge| edge.vertices)
        .collect::<Vec<_>>();
    source_vertex_indices.sort_unstable();
    source_vertex_indices.dedup();
    let dense_vertex_indices = source_vertex_indices
        .iter()
        .enumerate()
        .map(|(dense, source)| (*source, dense))
        .collect::<HashMap<_, _>>();
    let preview_vertices = source_vertex_indices
        .iter()
        .map(|source| {
            let position = preview.vertices()[*source].position;
            FoldImportPreviewVertex {
                x: position.x,
                y: position.y,
            }
        })
        .collect();
    let preview_edges = selected_edges
        .iter()
        .map(|edge| FoldImportPreviewEdge {
            source_index: edge.index,
            start: dense_vertex_indices[&edge.vertices[0]],
            end: dense_vertex_indices[&edge.vertices[1]],
            assignment: edge.assignment.token().to_owned(),
        })
        .collect();

    let mut warnings = preview
        .warnings()
        .iter()
        .map(fold_import_warning_message)
        .collect::<Vec<_>>();
    if preview
        .title()
        .is_some_and(|title| normalize_project_name(title).is_err())
    {
        warnings.push(
            "FOLD内のタイトルは作品名の条件に合わないため、既定の作品名を使用します。".to_owned(),
        );
    }
    if counts.flat > 0 {
        warnings.push(
            "F（平らな折り筋）は同じ意味の線種がないため、補助線または除外へ変換します。"
                .to_owned(),
        );
    }
    if counts.unassigned > 0 {
        warnings.push(
            "U（未割当）は山折り・谷折り・補助線・除外のいずれかを選ぶ必要があります。".to_owned(),
        );
    }
    if counts.join > 0 {
        warnings.push(
            "J（面の結合）は同じ意味の線種がないため、補助線または除外へ変換します。".to_owned(),
        );
    }

    FoldImportPreviewSnapshot {
        import_id,
        file_name: FOLD_IMPORT_FILE_LABEL,
        suggested_name: preview
            .title()
            .and_then(|title| normalize_project_name(title).ok())
            .unwrap_or_else(|| FOLD_IMPORT_FALLBACK_NAME.to_owned()),
        file_spec: preview.file_spec().map(|value| value.to_string()),
        frame_unit: fold_frame_unit_name(preview.frame_unit()),
        default_mm_per_unit: preview.recommended_millimetres_per_unit(),
        vertex_count: preview.vertices().len(),
        edge_count: preview.edges().len(),
        boundary_edge_count: counts.boundary,
        boundary_candidates,
        fixed_boundary_candidate_id: preview
            .fixed_boundary_candidate()
            .map(|candidate| candidate.0),
        assignments,
        preview_vertices,
        preview_edges,
        preview_truncated: preview.edges().len() > MAX_FOLD_IMPORT_PREVIEW_EDGES,
        warnings,
    }
}

fn fold_frame_unit_name(unit: &FoldFrameUnit) -> Option<String> {
    match unit {
        FoldFrameUnit::Unspecified => None,
        FoldFrameUnit::Unitless => Some("unit".to_owned()),
        FoldFrameUnit::Inch => Some("in".to_owned()),
        FoldFrameUnit::Point => Some("pt".to_owned()),
        FoldFrameUnit::Metre => Some("m".to_owned()),
        FoldFrameUnit::Centimetre => Some("cm".to_owned()),
        FoldFrameUnit::Millimetre => Some("mm".to_owned()),
        FoldFrameUnit::Micrometre => Some("um".to_owned()),
        FoldFrameUnit::Nanometre => Some("nm".to_owned()),
        FoldFrameUnit::Custom(value) => Some(value.clone()),
    }
}

fn fold_import_warning_message(warning: &FoldPreviewWarning) -> String {
    match warning {
        FoldPreviewWarning::MissingFileSpec => {
            "FOLD仕様バージョンの記載がありません。対応範囲として慎重に解釈します。".to_owned()
        }
        FoldPreviewWarning::MissingEdgesAssignment => {
            "辺の割当情報（edges_assignment）がないため、折り線種を確認・指定してください。"
                .to_owned()
        }
        FoldPreviewWarning::BoundaryAssignmentsNeedSelection => {
            "外周を一意に確定できないため、取り込む用紙外周を選択してください。".to_owned()
        }
        FoldPreviewWarning::UnitNeedsScaleSelection => {
            "実寸へ換算できる単位情報がないため、1単位あたりのmm値を指定してください。".to_owned()
        }
        FoldPreviewWarning::IgnoredFields { names } => {
            let known_count = names
                .iter()
                .filter(|name| fold_ignored_field_label(name).is_some())
                .count();
            let mut labels = Vec::new();
            for label in names
                .iter()
                .filter_map(|name| fold_ignored_field_label(name))
            {
                if !labels.contains(&label) {
                    labels.push(label);
                }
            }
            let unknown_count = names.len().saturating_sub(known_count);
            let mut details = labels.join("、");
            if unknown_count > 0 {
                if !details.is_empty() {
                    details.push('、');
                }
                details.push_str(&format!("その他の拡張フィールド{unknown_count}件"));
            }
            format!("取り込まないFOLD情報: {details}。")
        }
    }
}

fn svg_import_preview_snapshot(
    import_id: ProjectId,
    preview: &SvgPreview,
) -> Result<SvgImportPreviewSnapshot, String> {
    let mut selected_positions = Vec::new();
    let mut selected = vec![false; preview.edges().len()];
    let edge_positions = preview
        .edges()
        .iter()
        .enumerate()
        .map(|(position, edge)| (edge.index, position))
        .collect::<HashMap<_, _>>();

    for source_edge in preview
        .boundary_candidates()
        .iter()
        .flat_map(|candidate| candidate.source_edge_indices.iter().copied())
    {
        let Some(&position) = edge_positions.get(&source_edge) else {
            continue;
        };
        if !selected[position] && selected_positions.len() < MAX_SVG_IMPORT_PREVIEW_EDGES {
            selected[position] = true;
            selected_positions.push(position);
        }
    }
    for group in preview.style_groups() {
        let Some(position) = preview
            .edges()
            .iter()
            .position(|edge| edge.style_group == group.id)
        else {
            continue;
        };
        if !selected[position] && selected_positions.len() < MAX_SVG_IMPORT_PREVIEW_EDGES {
            selected[position] = true;
            selected_positions.push(position);
        }
    }
    for (position, is_selected) in selected.iter_mut().enumerate() {
        if selected_positions.len() == MAX_SVG_IMPORT_PREVIEW_EDGES {
            break;
        }
        if !*is_selected {
            *is_selected = true;
            selected_positions.push(position);
        }
    }
    selected_positions.sort_unstable_by_key(|position| preview.edges()[*position].index);

    let vertex_positions = preview
        .vertices()
        .iter()
        .enumerate()
        .map(|(position, vertex)| (vertex.index, position))
        .collect::<HashMap<_, _>>();
    let mut source_vertex_indices = selected_positions
        .iter()
        .flat_map(|position| preview.edges()[*position].vertices)
        .filter(|source| vertex_positions.contains_key(source))
        .collect::<Vec<_>>();
    source_vertex_indices.sort_unstable();
    source_vertex_indices.dedup();
    let dense_vertex_indices = source_vertex_indices
        .iter()
        .enumerate()
        .map(|(dense, source)| (*source, dense))
        .collect::<HashMap<_, _>>();
    let preview_vertices = source_vertex_indices
        .iter()
        .filter_map(|source| {
            let source_position = *vertex_positions.get(source)?;
            let position = preview.vertices().get(source_position)?.position;
            Some(SvgImportPreviewVertex {
                x: position.x,
                y: position.y,
            })
        })
        .collect::<Vec<_>>();
    let preview_edges = selected_positions
        .iter()
        .filter_map(|position| {
            let edge = preview.edges().get(*position)?;
            Some(SvgImportPreviewEdge {
                start: *dense_vertex_indices.get(&edge.vertices[0])?,
                end: *dense_vertex_indices.get(&edge.vertices[1])?,
                group_id: edge.style_group.0,
            })
        })
        .collect::<Vec<_>>();

    let style_groups = preview
        .style_groups()
        .iter()
        .map(|group| {
            let color = svg_import_color(group.stroke);
            SvgImportStyleGroupSnapshot {
                group_id: group.id.0,
                element_count: group.element_count,
                segment_count: group.segment_count,
                stroke: Some(format!("{color} / 幅 {}", group.stroke_width)),
                stroke_color: Some(color),
                dash_array: match &group.dash_pattern {
                    SvgDashPattern::Solid => None,
                    SvgDashPattern::Dashes(lengths) => Some(
                        lengths
                            .iter()
                            .map(|length| length.to_string())
                            .collect::<Vec<_>>()
                            .join(" "),
                    ),
                },
                line_cap: group.line_cap,
                classes: group.classes.clone(),
                layer: group.layer.clone(),
                representative_id: group.representative_id.clone(),
                semantic_hint: group.semantic.as_deref().and_then(svg_import_semantic_hint),
            }
        })
        .collect::<Vec<_>>();
    let boundary_candidates = preview
        .boundary_candidates()
        .iter()
        .map(|candidate| {
            let vertices = candidate
                .vertex_indices
                .iter()
                .filter_map(|source| {
                    let source_position = *vertex_positions.get(source)?;
                    let position = preview.vertices().get(source_position)?.position;
                    Some(SvgImportPreviewVertex {
                        x: position.x,
                        y: position.y,
                    })
                })
                .collect::<Vec<_>>();
            let (width, height) = svg_import_candidate_dimensions(&vertices);
            SvgBoundaryCandidateSnapshot {
                candidate_id: candidate.id.0,
                kind: match candidate.kind {
                    SvgBoundaryCandidateKind::ViewBox => "view_box",
                    SvgBoundaryCandidateKind::Polygon => "polygon",
                    SvgBoundaryCandidateKind::Polyline => "polyline",
                    SvgBoundaryCandidateKind::Rectangle => "rectangle",
                    SvgBoundaryCandidateKind::ClosedPath => "closed_path",
                },
                segment_count: candidate.vertex_indices.len(),
                width,
                height,
                vertices,
            }
        })
        .collect::<Vec<_>>();

    let mut warnings = preview
        .warnings()
        .iter()
        .map(svg_import_warning_message)
        .collect::<Vec<_>>();
    if preview
        .title()
        .is_some_and(|title| normalize_project_name(title).is_err())
    {
        warnings.push(
            "SVG内のタイトルは作品名の条件に合わないため、既定の作品名を使用します。".to_owned(),
        );
    }
    if !preview.style_groups().is_empty() {
        warnings.push(
            "SVGのstroke色、透明度、線幅、破線・線端表現は線種確認にだけ使用し、取込後には保存しません。"
                .to_owned(),
        );
    }
    if preview.style_groups().iter().any(|group| {
        !group.classes.is_empty()
            || group.layer.is_some()
            || group.representative_id.is_some()
            || group.semantic.is_some()
    }) {
        warnings.push(
            "SVGのレイヤー、class、代表ID、data-origami-kindは線種確認にだけ使用し、取込後には保存しません。"
                .to_owned(),
        );
    }
    if preview.edges().len() > MAX_SVG_IMPORT_PREVIEW_EDGES {
        warnings.push(format!(
            "表示上限により{}本の線をプレビューから省略しました。取込本体からは省略しません。",
            preview.edges().len() - MAX_SVG_IMPORT_PREVIEW_EDGES
        ));
    }
    if warnings.len() > 64 {
        return Err("SVG import has more than 64 distinct warning categories".to_owned());
    }

    Ok(SvgImportPreviewSnapshot {
        import_id,
        file_name: SVG_IMPORT_FILE_LABEL,
        suggested_name: preview
            .title()
            .and_then(|title| normalize_project_name(title).ok())
            .unwrap_or_else(|| SVG_IMPORT_FALLBACK_NAME.to_owned()),
        default_mm_per_unit: preview.recommended_millimetres_per_unit(),
        root_view_box: preview.root_view_box(),
        root_physical_size: preview.root_physical_size(),
        source_segment_count: preview.edges().len(),
        style_groups,
        boundary_candidates,
        preview_vertices,
        preview_edges,
        preview_truncated: selected_positions.len() < preview.edges().len(),
        warnings,
    })
}

fn svg_import_color(color: RgbaColor) -> String {
    if color.alpha == u8::MAX {
        format!("#{:02x}{:02x}{:02x}", color.red, color.green, color.blue)
    } else {
        format!(
            "#{:02x}{:02x}{:02x}{:02x}",
            color.red, color.green, color.blue, color.alpha
        )
    }
}

fn svg_import_requires_warning_acknowledgement(preview: &SvgPreview) -> bool {
    !preview.warnings().is_empty()
        || !preview.style_groups().is_empty()
        || preview
            .title()
            .is_some_and(|title| normalize_project_name(title).is_err())
        || preview.style_groups().iter().any(|group| {
            !group.classes.is_empty()
                || group.layer.is_some()
                || group.representative_id.is_some()
                || group.semantic.is_some()
        })
        || preview.edges().len() > MAX_SVG_IMPORT_PREVIEW_EDGES
}

fn svg_import_semantic_hint(value: &str) -> Option<SvgImportTargetRequest> {
    match value.trim().to_ascii_lowercase().as_str() {
        "boundary" => Some(SvgImportTargetRequest::Boundary),
        "mountain" => Some(SvgImportTargetRequest::Mountain),
        "valley" => Some(SvgImportTargetRequest::Valley),
        "auxiliary" => Some(SvgImportTargetRequest::Auxiliary),
        "cut" => Some(SvgImportTargetRequest::Cut),
        "ignore" => Some(SvgImportTargetRequest::Ignore),
        _ => None,
    }
}

fn svg_import_candidate_dimensions(vertices: &[SvgImportPreviewVertex]) -> (f64, f64) {
    let Some(first) = vertices.first() else {
        return (0.0, 0.0);
    };
    let (mut min_x, mut max_x, mut min_y, mut max_y) = (first.x, first.x, first.y, first.y);
    for vertex in &vertices[1..] {
        min_x = min_x.min(vertex.x);
        max_x = max_x.max(vertex.x);
        min_y = min_y.min(vertex.y);
        max_y = max_y.max(vertex.y);
    }
    (max_x - min_x, max_y - min_y)
}

fn svg_import_warning_message(warning: &SvgPreviewWarning) -> String {
    let count = warning.occurrences;
    let detail = match &warning.kind {
        SvgWarningKind::UnsupportedElement(name) => {
            format!("未対応の要素「{name}」を除外")
        }
        SvgWarningKind::UnsupportedAttribute(name) => {
            format!("未対応の属性「{name}」を無視")
        }
        SvgWarningKind::UnsupportedStyleProperty(name) => {
            format!("未対応のstyle property「{name}」を無視")
        }
        SvgWarningKind::UnsupportedCssSelector(_) => "未対応のCSS selectorを無視".to_owned(),
        SvgWarningKind::UnsupportedPathCommand(command) => {
            format!("曲線など未対応のpath command「{command}」を含むpathを除外")
        }
        SvgWarningKind::UnsupportedPaint(_) => "未対応のstroke指定を持つ線を除外".to_owned(),
        SvgWarningKind::UnsupportedLengthUnit(_) => {
            "解決できない長さ指定を持つ形状を除外".to_owned()
        }
        SvgWarningKind::ExternalReferenceIgnored => "外部参照を取得せず除外".to_owned(),
        SvgWarningKind::HiddenGeometryIgnored => "非表示の形状を除外".to_owned(),
        SvgWarningKind::GeometryWithoutStrokeIgnored => "strokeのない形状を除外".to_owned(),
        SvgWarningKind::FillIgnored => "塗り情報を保存しない".to_owned(),
        SvgWarningKind::MetadataIgnored => "SVG metadataを保存しない".to_owned(),
        SvgWarningKind::EmptyGeometryIgnored => "空の形状を除外".to_owned(),
        SvgWarningKind::PhysicalScaleNeedsSelection => {
            "物理寸法を一意に決められないため縮尺の入力が必要".to_owned()
        }
        SvgWarningKind::CssPixelScaleAssumed => {
            "CSSの96 px = 1 inch換算を使用しました。作者の意図と一致しない可能性があります"
                .to_owned()
        }
    };
    format!("{detail}（{count}件）。")
}

fn svg_import_group_target(target: SvgImportTargetRequest) -> SvgGroupTarget {
    match target {
        SvgImportTargetRequest::Boundary => SvgGroupTarget::Boundary,
        SvgImportTargetRequest::Mountain => SvgGroupTarget::Mountain,
        SvgImportTargetRequest::Valley => SvgGroupTarget::Valley,
        SvgImportTargetRequest::Auxiliary => SvgGroupTarget::Auxiliary,
        SvgImportTargetRequest::Cut => SvgGroupTarget::Cut,
        SvgImportTargetRequest::Ignore => SvgGroupTarget::Ignore,
    }
}

fn svg_import_group_mappings(
    style_mappings: Vec<SvgImportStyleMappingRequest>,
) -> Result<Vec<SvgGroupMapping>, String> {
    if style_mappings.len() > 64 {
        return Err("SVG style mapping has more than 64 groups".to_owned());
    }
    let mut group_mappings = style_mappings
        .into_iter()
        .map(|mapping| SvgGroupMapping {
            group: SvgStyleGroupId(mapping.group_id),
            target: svg_import_group_target(mapping.target),
        })
        .collect::<Vec<_>>();
    group_mappings.sort_by_key(|mapping| mapping.group);
    Ok(group_mappings)
}

fn fold_ignored_field_label(name: &str) -> Option<&'static str> {
    match name {
        "file_frames" => Some("複数フレーム"),
        "file_creator" => Some("作成ソフト情報"),
        "file_author" => Some("作者情報"),
        "file_description" => Some("説明"),
        "file_classes" => Some("ファイル分類"),
        "frame_classes" => Some("フレーム分類"),
        "frame_attributes" => Some("フレーム属性"),
        "frame_title" => Some("フレーム名"),
        "frame_parent" | "frame_inherit" => Some("フレーム継承"),
        "faces_vertices" | "faces_edges" | "edges_faces" => Some("面情報（辺から再計算）"),
        "faceOrders" | "edgeOrders" => Some("重なり順"),
        "edges_foldAngle" => Some("折り角度"),
        "edges_length" => Some("辺長メタデータ"),
        "frame_transform" => Some("フレーム変換"),
        _ => None,
    }
}

fn build_fold_import_replacement(
    bytes: &[u8],
    name: String,
    millimeters_per_unit: f64,
    boundary_candidate: FoldBoundaryCandidateId,
    mappings: HashMap<String, FoldImportTargetRequest>,
) -> Result<ProjectState, String> {
    let preview = read_fold_preview(bytes).map_err(fold_file_invalid_error)?;
    let counts = preview.assignment_counts();
    for source in mappings.keys() {
        let present = match source.as_str() {
            "M" => counts.mountain > 0,
            "V" => counts.valley > 0,
            "F" => counts.flat > 0,
            "U" => counts.unassigned > 0,
            "C" => counts.cut > 0,
            "J" => counts.join > 0,
            _ => false,
        };
        if !present {
            return Err(format!(
                "FOLD assignment {source} does not occur in the staged preview"
            ));
        }
    }
    let assignment_mapping = FoldAssignmentMapping {
        boundary: Some(FoldAssignmentTarget::ImportAs {
            edge_kind: EdgeKind::Boundary,
        }),
        mountain: fold_import_assignment_target(&mappings, "M"),
        valley: fold_import_assignment_target(&mappings, "V"),
        flat: fold_import_assignment_target(&mappings, "F"),
        unassigned: fold_import_assignment_target(&mappings, "U"),
        cut: fold_import_assignment_target(&mappings, "C"),
        join: fold_import_assignment_target(&mappings, "J"),
    };
    let conversion = preview
        .convert_with_boundary_candidate(
            &FoldConversionOptions {
                assignment_mapping,
                millimetres_per_unit: millimeters_per_unit,
            },
            boundary_candidate,
        )
        .map_err(|error| format!("FOLD mapping could not be applied: {error}"))?;
    let (crease_pattern, _, _, boundary_vertices) = conversion.into_parts();
    let mut paper = Paper {
        boundary_vertices,
        ..Paper::default()
    };
    paper.cutting_allowed = crease_pattern
        .edges
        .iter()
        .any(|edge| edge.kind == EdgeKind::Cut);

    let replacement = ProjectState::new_unsaved(name, crease_pattern, paper);
    let pattern_validation = replacement.editor.validation();
    if !pattern_validation.is_valid() {
        return Err(format!(
            "converted FOLD crease pattern has {} validation issue(s)",
            pattern_validation.issues().len()
        ));
    }
    let paper_validation = validate_paper(replacement.editor.paper(), replacement.editor.pattern());
    if !paper_validation.is_valid() {
        return Err(format!(
            "converted FOLD paper boundary has {} validation issue(s)",
            paper_validation.issues.len()
        ));
    }
    validate_import_active_edge_containment(&replacement, "FOLD")?;
    Ok(replacement)
}

struct SvgImportReplacementOptions {
    name: String,
    millimeters_per_unit: f64,
    group_mappings: Vec<SvgGroupMapping>,
    boundary_candidate: Option<SvgBoundaryCandidateId>,
    boundary_confirmed: bool,
    warnings_acknowledged: bool,
    cutting_allowed_confirmed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct SvgImportGeometryValidation {
    width_mm: f64,
    height_mm: f64,
    has_cuts: bool,
}

fn build_svg_import_replacement(
    bytes: &[u8],
    options: SvgImportReplacementOptions,
) -> Result<ProjectState, String> {
    let SvgImportReplacementOptions {
        name,
        millimeters_per_unit,
        group_mappings,
        boundary_candidate,
        boundary_confirmed,
        warnings_acknowledged,
        cutting_allowed_confirmed,
    } = options;
    let preview = read_svg_preview(bytes)
        .map_err(|_| "staged SVG preview could not be revalidated".to_owned())?;
    if !boundary_confirmed {
        return Err("SVG paper boundary must be explicitly confirmed".to_owned());
    }
    if svg_import_requires_warning_acknowledgement(&preview) && !warnings_acknowledged {
        return Err("SVG import warnings must be explicitly acknowledged".to_owned());
    }
    let (replacement, has_cuts) = convert_svg_import_project(
        &preview,
        name,
        millimeters_per_unit,
        group_mappings,
        boundary_candidate,
    )?;
    if has_cuts && !cutting_allowed_confirmed {
        return Err(
            "SVG contains imported cut lines; cutting must be explicitly allowed".to_owned(),
        );
    }
    Ok(replacement)
}

fn validate_svg_import_geometry(
    bytes: &[u8],
    millimeters_per_unit: f64,
    group_mappings: Vec<SvgGroupMapping>,
    boundary_candidate: Option<SvgBoundaryCandidateId>,
) -> Result<SvgImportGeometryValidation, String> {
    validate_import_scale(millimeters_per_unit)?;
    let preview = read_svg_preview(bytes)
        .map_err(|_| "staged SVG preview could not be revalidated".to_owned())?;
    let (project, has_cuts) = convert_svg_import_project(
        &preview,
        SVG_IMPORT_FALLBACK_NAME.to_owned(),
        millimeters_per_unit,
        group_mappings,
        boundary_candidate,
    )?;
    let (width_mm, height_mm) = svg_import_paper_dimensions(&project)?;
    Ok(SvgImportGeometryValidation {
        width_mm,
        height_mm,
        has_cuts,
    })
}

fn convert_svg_import_project(
    preview: &SvgPreview,
    name: String,
    millimeters_per_unit: f64,
    group_mappings: Vec<SvgGroupMapping>,
    boundary_candidate: Option<SvgBoundaryCandidateId>,
) -> Result<(ProjectState, bool), String> {
    let conversion = preview
        .convert(&SvgConversionOptions {
            millimetres_per_unit: millimeters_per_unit,
            group_mappings,
            boundary_candidate,
        })
        .map_err(|error| format!("SVG mapping could not be applied: {error}"))?;
    let (crease_pattern, boundary_vertices, _, has_cuts) = conversion.into_parts();
    let mut paper = Paper {
        boundary_vertices,
        ..Paper::default()
    };
    paper.cutting_allowed = has_cuts;

    let replacement = ProjectState::new_unsaved(name, crease_pattern, paper);
    let pattern_validation = replacement.editor.validation();
    if !pattern_validation.is_valid() {
        return Err(format!(
            "converted SVG crease pattern has {} validation issue(s)",
            pattern_validation.issues().len()
        ));
    }
    let paper_validation = validate_paper(replacement.editor.paper(), replacement.editor.pattern());
    if !paper_validation.is_valid() {
        return Err(format!(
            "converted SVG paper boundary has {} validation issue(s)",
            paper_validation.issues.len()
        ));
    }
    validate_import_active_edge_containment(&replacement, "SVG")?;
    Ok((replacement, has_cuts))
}

fn svg_import_paper_dimensions(project: &ProjectState) -> Result<(f64, f64), String> {
    let positions = project
        .editor
        .pattern()
        .vertices
        .iter()
        .map(|vertex| (vertex.id, vertex.position))
        .collect::<HashMap<_, _>>();
    let mut boundary_positions = project
        .editor
        .paper()
        .boundary_vertices
        .iter()
        .map(|vertex_id| {
            positions.get(vertex_id).copied().ok_or_else(|| {
                "converted SVG paper boundary references a missing vertex".to_owned()
            })
        });
    let first = boundary_positions
        .next()
        .transpose()?
        .ok_or_else(|| "converted SVG paper boundary is empty".to_owned())?;
    let (mut min_x, mut max_x, mut min_y, mut max_y) = (first.x, first.x, first.y, first.y);
    for position in boundary_positions {
        let position = position?;
        min_x = min_x.min(position.x);
        max_x = max_x.max(position.x);
        min_y = min_y.min(position.y);
        max_y = max_y.max(position.y);
    }
    let width = max_x - min_x;
    let height = max_y - min_y;
    if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
        return Err("converted SVG paper dimensions are invalid".to_owned());
    }
    Ok((width, height))
}

fn validate_import_active_edge_containment(
    project: &ProjectState,
    format_label: &str,
) -> Result<(), String> {
    let positions = project
        .editor
        .pattern()
        .vertices
        .iter()
        .map(|vertex| (vertex.id, vertex.position))
        .collect::<HashMap<_, _>>();
    let boundary = project
        .editor
        .paper()
        .boundary_vertices
        .iter()
        .map(|vertex| {
            positions
                .get(vertex)
                .copied()
                .ok_or_else(|| format!("converted {format_label} boundary could not be resolved"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let active_edges = project
        .editor
        .pattern()
        .edges
        .iter()
        .filter(|edge| {
            matches!(
                edge.kind,
                EdgeKind::Mountain | EdgeKind::Valley | EdgeKind::Cut
            )
        })
        .collect::<Vec<_>>();
    let containment_tests = active_edges
        .len()
        .checked_mul(boundary.len())
        .ok_or_else(|| format!("converted {format_label} containment work is not representable"))?;
    if containment_tests > MAX_FOLD_IMPORT_CONTAINMENT_TESTS {
        return Err(format!(
            "converted {format_label} needs {containment_tests} containment tests; the limit is {MAX_FOLD_IMPORT_CONTAINMENT_TESTS}"
        ));
    }

    let mut outside_count = 0;
    for edge in active_edges {
        let start = positions
            .get(&edge.start)
            .copied()
            .ok_or_else(|| format!("converted {format_label} edge start could not be resolved"))?;
        let end = positions
            .get(&edge.end)
            .copied()
            .ok_or_else(|| format!("converted {format_label} edge end could not be resolved"))?;
        let relation = segment_midpoint_polygon_relation(start, end, &boundary).map_err(|_| {
            format!("converted {format_label} edge containment could not be classified")
        })?;
        if relation != PointPolygonRelation::Inside {
            outside_count += 1;
        }
    }
    if outside_count > 0 {
        return Err(format!(
            "converted {format_label} has {outside_count} active edge(s) outside the paper boundary"
        ));
    }
    Ok(())
}

fn fold_import_assignment_target(
    mappings: &HashMap<String, FoldImportTargetRequest>,
    source: &str,
) -> Option<FoldAssignmentTarget> {
    mappings.get(source).copied().map(|target| match target {
        FoldImportTargetRequest::Mountain => FoldAssignmentTarget::ImportAs {
            edge_kind: EdgeKind::Mountain,
        },
        FoldImportTargetRequest::Valley => FoldAssignmentTarget::ImportAs {
            edge_kind: EdgeKind::Valley,
        },
        FoldImportTargetRequest::Auxiliary => FoldAssignmentTarget::ImportAs {
            edge_kind: EdgeKind::Auxiliary,
        },
        FoldImportTargetRequest::Cut => FoldAssignmentTarget::ImportAs {
            edge_kind: EdgeKind::Cut,
        },
        FoldImportTargetRequest::Ignore => FoldAssignmentTarget::Ignore,
    })
}

fn validate_document_instruction_poses(document: &ProjectDocument) -> Result<(), String> {
    if document.instruction_timeline.steps.is_empty() {
        return Ok(());
    }
    let editor = EditorState::with_paper(document.crease_pattern.clone(), document.paper.clone());
    let current_fingerprint = editor.fold_model_fingerprint_v1();
    if !document.instruction_timeline.steps.iter().any(|step| {
        step.pose.model == InstructionPoseModel::AbsoluteHingeAnglesV1
            && step.pose.source_model_fingerprint == current_fingerprint
    }) {
        // Poses authored for an older crease pattern remain intentionally
        // loadable as stale, editable records. Playback keeps them disabled
        // until the user captures a new pose against the current model.
        return Ok(());
    }

    let topology = editor
        .topology_analysis_input(document.project_id)
        .analyze();
    let snapshot = topology.simulation_snapshot().ok_or_else(|| {
        "a current instruction pose refers to a crease pattern that is not simulation-ready"
            .to_owned()
    })?;
    let topology = prepare_instruction_topology(snapshot)?;
    for (index, step) in document.instruction_timeline.steps.iter().enumerate() {
        if step.pose.model == InstructionPoseModel::DeclarativeOnlyV1
            || step.pose.source_model_fingerprint != current_fingerprint
        {
            continue;
        }
        instruction_pose_from_context(
            &topology,
            current_fingerprint.clone(),
            step.pose.fixed_face,
            step.pose.hinge_angles.clone(),
        )
        .map_err(|error| format!("instruction step {} is invalid: {error}", index + 1))?;
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn rename_windows_staged_file(staged_file: &File, destination: &Path) -> Result<(), String> {
    rename_windows_staged_file_with_policy(
        staged_file,
        destination,
        save_path::ExistingDestinationPolicy::ReplaceConfirmed,
    )
}

#[cfg(target_os = "windows")]
fn rename_windows_staged_file_with_policy(
    staged_file: &File,
    destination: &Path,
    existing_destination_policy: save_path::ExistingDestinationPolicy,
) -> Result<(), String> {
    let destination_wide = destination.as_os_str().encode_wide().collect::<Vec<_>>();
    if destination_wide.contains(&0) {
        return Err(format!(
            "failed to commit {} atomically: the path contains a NUL character",
            destination.display()
        ));
    }

    let file_name_bytes = destination_wide
        .len()
        .checked_mul(size_of::<u16>())
        .and_then(|length| u32::try_from(length).ok())
        .ok_or_else(|| {
            format!(
                "failed to commit {} atomically: the path is too long",
                destination.display()
            )
        })?;
    let buffer_size = size_of::<FILE_RENAME_INFO>()
        .checked_add(file_name_bytes as usize)
        .ok_or_else(|| {
            format!(
                "failed to commit {} atomically: the rename request is too large",
                destination.display()
            )
        })?;
    let buffer_size_u32 = u32::try_from(buffer_size).map_err(|_| {
        format!(
            "failed to commit {} atomically: the rename request is too large",
            destination.display()
        )
    })?;
    let word_size = size_of::<usize>();
    let word_count = buffer_size
        .checked_add(word_size - 1)
        .map(|length| length / word_size)
        .ok_or_else(|| {
            format!(
                "failed to commit {} atomically: the rename request is too large",
                destination.display()
            )
        })?;
    let mut buffer = vec![0usize; word_count];
    let info = buffer.as_mut_ptr().cast::<FILE_RENAME_INFO>();

    // SAFETY: `buffer` is usize-aligned and large enough for the fixed header,
    // destination UTF-16 units, and a trailing NUL. The handle remains owned
    // by `staged_file` throughout the call. FileRenameInfo renames that exact
    // open file, so a pathname swap cannot substitute unverified bytes.
    let renamed = unsafe {
        (*info).Anonymous.ReplaceIfExists = matches!(
            existing_destination_policy,
            save_path::ExistingDestinationPolicy::ReplaceConfirmed
        );
        (*info).RootDirectory = ptr::null_mut();
        (*info).FileNameLength = file_name_bytes;
        let file_name = ptr::addr_of_mut!((*info).FileName).cast::<u16>();
        ptr::copy_nonoverlapping(destination_wide.as_ptr(), file_name, destination_wide.len());
        SetFileInformationByHandle(
            staged_file.as_raw_handle() as RawHandle,
            FileRenameInfo,
            info.cast(),
            buffer_size_u32,
        )
    };
    if renamed == 0 {
        return Err(format!(
            "failed to commit {} atomically: {}",
            destination.display(),
            std::io::Error::last_os_error()
        ));
    }
    Ok(())
}

fn ensure_ori2_extension(path: PathBuf) -> Result<save_path::DialogSaveDestination, String> {
    save_path::normalize_dialog_save_path(path, "ori2")
}

fn suggested_file_name(project_name: &str) -> String {
    let mut sanitized = String::new();
    for character in project_name.trim().chars().take(80) {
        if character.is_control()
            || matches!(
                character,
                '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'
            )
        {
            sanitized.push('_');
        } else {
            sanitized.push(character);
        }
    }
    let sanitized = sanitized.trim_matches([' ', '.']);
    let base = if sanitized.is_empty() {
        UNTITLED_PROJECT_NAME
    } else {
        sanitized
    };
    format!("{base}.ori2")
}

fn capture_validation_input(project: &ProjectState) -> ValidationAnalysisInput {
    ValidationAnalysisInput {
        project_instance_id: project.instance_id,
        project_id: project.project_id,
        source: project.editor.topology_analysis_input(project.project_id),
    }
}

#[cfg(test)]
fn validation_snapshot(project: &ProjectState) -> ValidationSnapshot {
    finish_validation_snapshot(
        project,
        analyze_validation_input(capture_validation_input(project)),
    )
    .expect("synchronous validation fixture must remain current")
}

fn analyze_validation_input(input: ValidationAnalysisInput) -> AnalyzedProjectValidation {
    let analysis_editor =
        EditorState::with_paper(input.source.pattern().clone(), input.source.paper().clone());
    let source_model_fingerprint = analysis_editor.fold_model_fingerprint_v1();
    let crease_validation = analysis_editor.validation();
    let paper_validation = validate_paper(analysis_editor.paper(), analysis_editor.pattern());
    let local_flat_foldability =
        analyze_local_flat_foldability(analysis_editor.paper(), analysis_editor.pattern());
    let mut issues =
        Vec::with_capacity(crease_validation.issues().len() + paper_validation.issues.len());
    issues.extend(
        crease_validation
            .issues()
            .iter()
            .map(validation_issue_snapshot),
    );
    issues.extend(paper_validation.issues.iter().map(|issue| {
        paper_validation_issue_snapshot(issue, analysis_editor.paper(), analysis_editor.pattern())
    }));
    AnalyzedProjectValidation {
        snapshot: ValidationSnapshot {
            project_id: input.project_id,
            revision: input.source.revision(),
            is_valid: issues.is_empty(),
            issues,
            local_flat_foldability,
        },
        input,
        source_model_fingerprint,
    }
}

fn finish_validation_snapshot(
    project: &ProjectState,
    analyzed: AnalyzedProjectValidation,
) -> Result<ValidationSnapshot, String> {
    if project.instance_id != analyzed.input.project_instance_id
        || !analyzed
            .input
            .source
            .is_current_for(project.project_id, &project.editor)
    {
        return Err("the project changed while validation was being analyzed".to_owned());
    }
    if analyzed.snapshot.project_id != project.project_id
        || analyzed.snapshot.revision != analyzed.input.source.revision()
    {
        return Err("validation analysis returned unexpected source identity".to_owned());
    }
    if !valid_fold_model_fingerprint(&analyzed.source_model_fingerprint) {
        return Err("validation analysis returned an invalid source fingerprint".to_owned());
    }
    Ok(analyzed.snapshot)
}

fn valid_fold_model_fingerprint(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn validation_issue_snapshot(issue: &ValidationIssue) -> ValidationIssueSnapshot {
    match issue {
        ValidationIssue::NonFiniteVertex { vertex, .. } => ValidationIssueSnapshot {
            code: "non_finite_vertex",
            vertices: vec![*vertex],
            edges: Vec::new(),
        },
        ValidationIssue::DuplicateVertex {
            first, duplicate, ..
        } => ValidationIssueSnapshot {
            code: "duplicate_vertex",
            vertices: vec![*first, *duplicate],
            edges: Vec::new(),
        },
        ValidationIssue::MissingEndpoint { edge, vertex, .. } => ValidationIssueSnapshot {
            code: "missing_endpoint",
            vertices: vec![*vertex],
            edges: vec![*edge],
        },
        ValidationIssue::ZeroLengthEdge { edge } => ValidationIssueSnapshot {
            code: "zero_length_edge",
            vertices: Vec::new(),
            edges: vec![*edge],
        },
        ValidationIssue::UnsplitIntersection {
            first_edge,
            second_edge,
            ..
        } => ValidationIssueSnapshot {
            code: "unsplit_intersection",
            vertices: Vec::new(),
            edges: vec![*first_edge, *second_edge],
        },
        ValidationIssue::IntersectionCalculationFailed {
            first_edge,
            second_edge,
            ..
        } => ValidationIssueSnapshot {
            code: "intersection_calculation_failed",
            vertices: Vec::new(),
            edges: vec![*first_edge, *second_edge],
        },
    }
}

fn paper_validation_issue_snapshot(
    issue: &PaperValidationIssue,
    paper: &Paper,
    pattern: &CreasePattern,
) -> ValidationIssueSnapshot {
    match issue {
        PaperValidationIssue::NonFiniteThickness { .. } => ValidationIssueSnapshot {
            code: "non_finite_thickness",
            vertices: Vec::new(),
            edges: Vec::new(),
        },
        PaperValidationIssue::NegativeThickness { .. } => ValidationIssueSnapshot {
            code: "negative_thickness",
            vertices: Vec::new(),
            edges: Vec::new(),
        },
        PaperValidationIssue::TooFewBoundaryVertices { .. } => ValidationIssueSnapshot {
            code: "too_few_boundary_vertices",
            vertices: unique_vertex_ids(paper.boundary_vertices.iter().copied()),
            edges: Vec::new(),
        },
        PaperValidationIssue::DuplicateBoundaryVertex { vertex, .. } => ValidationIssueSnapshot {
            code: "duplicate_boundary_vertex",
            vertices: vec![*vertex],
            edges: Vec::new(),
        },
        PaperValidationIssue::MissingBoundaryVertex { vertex, .. } => ValidationIssueSnapshot {
            code: "missing_boundary_vertex",
            vertices: vec![*vertex],
            edges: Vec::new(),
        },
        PaperValidationIssue::NonFiniteBoundaryVertex { vertex, .. } => ValidationIssueSnapshot {
            code: "non_finite_boundary_vertex",
            vertices: vec![*vertex],
            edges: Vec::new(),
        },
        PaperValidationIssue::MissingBoundaryEdge { boundary_edge } => ValidationIssueSnapshot {
            code: "missing_boundary_edge",
            vertices: boundary_vertices(&[*boundary_edge]),
            edges: Vec::new(),
        },
        PaperValidationIssue::DuplicateBoundaryEdge {
            boundary_edge,
            first_edge,
            duplicate_edge,
        } => ValidationIssueSnapshot {
            code: "duplicate_boundary_edge",
            vertices: boundary_vertices(&[*boundary_edge]),
            edges: unique_edge_ids([*first_edge, *duplicate_edge]),
        },
        PaperValidationIssue::UnexpectedBoundaryEdge { edge, start, end } => {
            ValidationIssueSnapshot {
                code: "unexpected_boundary_edge",
                vertices: unique_vertex_ids([*start, *end]),
                edges: vec![*edge],
            }
        }
        PaperValidationIssue::ZeroLengthBoundaryEdge { edge } => ValidationIssueSnapshot {
            code: "zero_length_boundary_edge",
            vertices: boundary_vertices(&[*edge]),
            edges: boundary_edge_ids(pattern, &[*edge]),
        },
        PaperValidationIssue::SelfIntersection {
            first_edge,
            second_edge,
            ..
        } => {
            let boundary_edges = [*first_edge, *second_edge];
            ValidationIssueSnapshot {
                code: "boundary_self_intersection",
                vertices: boundary_vertices(&boundary_edges),
                edges: boundary_edge_ids(pattern, &boundary_edges),
            }
        }
        PaperValidationIssue::IntersectionCalculationFailed {
            first_edge,
            second_edge,
            ..
        } => {
            let boundary_edges = [*first_edge, *second_edge];
            ValidationIssueSnapshot {
                code: "boundary_intersection_calculation_failed",
                vertices: boundary_vertices(&boundary_edges),
                edges: boundary_edge_ids(pattern, &boundary_edges),
            }
        }
        PaperValidationIssue::ZeroArea { boundary_vertices } => ValidationIssueSnapshot {
            code: "zero_area_boundary",
            vertices: unique_vertex_ids(boundary_vertices.iter().copied()),
            edges: Vec::new(),
        },
        PaperValidationIssue::AreaCalculationFailed {
            boundary_vertices, ..
        } => ValidationIssueSnapshot {
            code: "boundary_area_calculation_failed",
            vertices: unique_vertex_ids(boundary_vertices.iter().copied()),
            edges: Vec::new(),
        },
    }
}

fn boundary_vertices(boundary_edges: &[BoundaryEdgeRef]) -> Vec<VertexId> {
    unique_vertex_ids(
        boundary_edges
            .iter()
            .flat_map(|edge| [edge.start, edge.end]),
    )
}

fn unique_vertex_ids(vertices: impl IntoIterator<Item = VertexId>) -> Vec<VertexId> {
    let mut unique = Vec::new();
    for vertex in vertices {
        if !unique.contains(&vertex) {
            unique.push(vertex);
        }
    }
    unique
}

fn unique_edge_ids(edges: impl IntoIterator<Item = EdgeId>) -> Vec<EdgeId> {
    let mut unique = Vec::new();
    for edge in edges {
        if !unique.contains(&edge) {
            unique.push(edge);
        }
    }
    unique
}

fn boundary_edge_ids(pattern: &CreasePattern, boundary_edges: &[BoundaryEdgeRef]) -> Vec<EdgeId> {
    let mut matching = Vec::new();
    for boundary_edge in boundary_edges {
        for edge in &pattern.edges {
            let endpoints_match = (edge.start == boundary_edge.start
                && edge.end == boundary_edge.end)
                || (edge.start == boundary_edge.end && edge.end == boundary_edge.start);
            if edge.kind == EdgeKind::Boundary && endpoints_match && !matching.contains(&edge.id) {
                matching.push(edge.id);
            }
        }
    }
    matching
}

#[cfg(target_os = "macos")]
fn macos_menu(app_handle: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let package = app_handle.package_info();
    let config = app_handle.config();
    let about_metadata = AboutMetadata {
        name: Some(package.name.clone()),
        version: Some(package.version.to_string()),
        copyright: config.bundle.copyright.clone(),
        authors: config
            .bundle
            .publisher
            .clone()
            .map(|publisher| vec![publisher]),
        ..Default::default()
    };
    let quit = MenuItem::with_id(
        app_handle,
        MACOS_QUIT_MENU_ID,
        format!("Quit {}", package.name),
        true,
        Some("CmdOrCtrl+Q"),
    )?;
    let app_menu = Submenu::with_items(
        app_handle,
        package.name.clone(),
        true,
        &[
            &PredefinedMenuItem::about(app_handle, None, Some(about_metadata))?,
            &PredefinedMenuItem::separator(app_handle)?,
            &PredefinedMenuItem::services(app_handle, None)?,
            &PredefinedMenuItem::separator(app_handle)?,
            &PredefinedMenuItem::hide(app_handle, None)?,
            &PredefinedMenuItem::hide_others(app_handle, None)?,
            &PredefinedMenuItem::separator(app_handle)?,
            &quit,
        ],
    )?;
    let file_menu = Submenu::with_items(
        app_handle,
        "File",
        true,
        &[&PredefinedMenuItem::close_window(app_handle, None)?],
    )?;
    let edit_menu = Submenu::with_items(
        app_handle,
        "Edit",
        true,
        &[
            &PredefinedMenuItem::undo(app_handle, None)?,
            &PredefinedMenuItem::redo(app_handle, None)?,
            &PredefinedMenuItem::separator(app_handle)?,
            &PredefinedMenuItem::cut(app_handle, None)?,
            &PredefinedMenuItem::copy(app_handle, None)?,
            &PredefinedMenuItem::paste(app_handle, None)?,
            &PredefinedMenuItem::select_all(app_handle, None)?,
        ],
    )?;
    let view_menu = Submenu::with_items(
        app_handle,
        "View",
        true,
        &[&PredefinedMenuItem::fullscreen(app_handle, None)?],
    )?;
    let window_menu = Submenu::with_id_and_items(
        app_handle,
        WINDOW_SUBMENU_ID,
        "Window",
        true,
        &[
            &PredefinedMenuItem::minimize(app_handle, None)?,
            &PredefinedMenuItem::maximize(app_handle, None)?,
            &PredefinedMenuItem::separator(app_handle)?,
            &PredefinedMenuItem::close_window(app_handle, None)?,
        ],
    )?;
    let help_menu = Submenu::with_id_and_items(app_handle, HELP_SUBMENU_ID, "Help", true, &[])?;

    Menu::with_items(
        app_handle,
        &[
            &app_menu,
            &file_menu,
            &edit_menu,
            &view_menu,
            &window_menu,
            &help_menu,
        ],
    )
}

pub fn run() {
    // Tauri plugins run in registration order. Single-instance must remain
    // first so no other plugin initializes in a secondary process.
    let builder =
        tauri::Builder::default().plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // Privacy boundary: command-line arguments and the working
            // directory are intentionally neither inspected nor recorded.
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }));
    #[cfg(target_os = "macos")]
    let builder = builder
        .enable_macos_default_menu(false)
        .menu(macos_menu)
        .on_menu_event(|app_handle, event| {
            if event.id() == MACOS_QUIT_MENU_ID {
                app_handle.exit(0);
            }
        });

    let app = builder
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let app_data_root = app.path().app_data_dir().map_err(|_| {
                std::io::Error::other("the private recovery directory could not be initialized")
            })?;
            let _ = std::fs::create_dir_all(&app_data_root);
            let recovery_root = app_data_root.join("recovery");
            let recovery = RecoveryRuntime::new(recovery_root);
            let project_folder_io =
                ProjectFolderIoState::new(app_data_root.join("project-folder-recovery"));
            // External parents may be offline during startup. Attempt
            // recovery now, retain the native registry on failure, and let
            // only project-folder commands retry/fail closed later.
            let _ = project_folder_io.recover_pending_replacement();
            app.manage(AppState::new(initial_project_state()));
            app.manage(recovery);
            app.manage(project_folder_io);
            app.manage(DiagnosticsState::from_app_handle(app.handle()));
            start_recovery_autosave_timer(app.handle().clone()).map_err(|_| {
                std::io::Error::other("the private recovery timer could not be initialized")
            })?;
            Ok(())
        })
        .manage(FoldImportState::default())
        .manage(Fold3dFramesImportState::default())
        .manage(FoldTechniqueFileIoState::default())
        .manage(SvgImportState::default())
        .manage(CreaseExportState::default())
        .manage(StaticMeshExportState::default())
        .manage(MeshAnimationExportState::default())
        .manage(GlobalFlatFoldabilityState::default())
        .manage(InstructionExportState::default())
        .manage(StackedFoldTransactionState::default())
        .manage(ExitGuard::default())
        .invoke_handler(tauri::generate_handler![
            generate_benchmark_pattern,
            project_snapshot,
            update_project_memo,
            get_history_entry_limit,
            set_history_entry_limit,
            get_recovery_candidate,
            get_recovery_autosave_status,
            restore_recovery,
            discard_recovery,
            prepare_window_close,
            cancel_window_close_prepare,
            new_project,
            validate_project,
            apply_current_native_pose,
            inspect_current_static_collision,
            analyze_geometric_constraints,
            evaluate_numeric_expression,
            analyze_project_topology,
            begin_global_flat_foldability,
            get_global_flat_foldability_progress,
            get_global_flat_foldability_result,
            cancel_global_flat_foldability,
            propose_current_stacked_fold_read,
            cancel_stacked_fold_transaction_preview,
            apply_stacked_fold_transaction,
            open_project,
            save_project,
            save_project_as,
            open_project_folder,
            save_project_folder_as,
            open_fold_technique_file,
            save_fold_technique_file_as,
            preview_crease_pattern_export,
            save_crease_pattern_export,
            cancel_crease_pattern_export,
            preview_static_mesh_export,
            save_static_mesh_export,
            cancel_static_mesh_export,
            preview_instruction_mesh_animation,
            save_instruction_mesh_animation,
            cancel_instruction_mesh_animation,
            begin_instruction_export,
            preview_instruction_export,
            get_instruction_export_progress,
            save_instruction_export,
            cancel_instruction_export,
            preview_fold_import,
            preview_fold_3d_frames,
            select_fold_3d_frame,
            prepare_fold_3d_applied_pose,
            apply_fold_3d_applied_pose,
            cancel_fold_3d_frames,
            apply_fold_import,
            prepare_fold_3d_instruction_timeline,
            apply_fold_3d_instruction_timeline,
            cancel_fold_import,
            preview_svg_import,
            validate_svg_import_settings,
            apply_svg_import,
            cancel_svg_import,
            add_vertex,
            move_vertex,
            move_edge,
            mirror_edge_left_right,
            rotate_edge_about_point,
            move_vertices,
            remove_vertex,
            add_edge,
            add_connected_vertex,
            remove_edge,
            create_project_layer,
            rename_project_layer,
            update_project_layer_presentation,
            move_project_layer,
            delete_project_layer,
            assign_edge_to_project_layer,
            add_edge_orientation_constraint,
            add_geometric_constraint,
            remove_geometric_constraint,
            undo,
            redo,
            add_instruction_step,
            append_named_technique_instruction_steps,
            update_instruction_step_metadata,
            replace_instruction_step_pose,
            remove_instruction_step,
            move_instruction_step,
            set_cutting_allowed,
            update_paper_properties,
            import_front_paper_texture,
            import_back_paper_texture,
            set_element_metadata,
            set_length_display_unit,
            resize_rectangular_paper,
            split_edge,
            connect_edge_intersection,
            connect_intersection_cluster,
            connect_t_junction,
            split_boundary_edge,
            remove_boundary_vertex,
            record_unexpected_diagnostic,
            prepare_diagnostics_share_preview,
            save_diagnostics_share_preview
        ])
        .build(tauri::generate_context!())
        .expect("failed to build ORIGAMI2 desktop application");

    app.run(|app_handle, event| {
        let tauri::RunEvent::ExitRequested { api, .. } = event else {
            return;
        };

        let project_state = app_handle.state::<AppState>();
        match app_handle
            .state::<RecoveryRuntime>()
            .settle_prepared_window_close(&project_state)
        {
            Ok(PreparedWindowCloseSettlement::Settled) => return,
            Ok(PreparedWindowCloseSettlement::Rejected) | Err(_) => {
                // The WebView's close authorization was stale or its bounded
                // recovery clear failed. If the window still exists, keep the
                // process open and report the fixed error. With no remaining
                // window, allow exit while retaining the recovery slot rather
                // than leave an invisible process running.
                if !app_handle.webview_windows().is_empty() {
                    api.prevent_exit();
                    app_handle
                        .dialog()
                        .message(
                            "The private recovery data could not be settled. The application remains open.",
                        )
                        .title("ORIGAMI2")
                        .kind(MessageDialogKind::Error)
                        .buttons(MessageDialogButtons::Ok)
                        .show(|_| {});
                }
                return;
            }
            Ok(PreparedWindowCloseSettlement::NotPrepared) => {}
        }

        // A missing WebView is not proof that the JavaScript close listener
        // ran: listener setup, the renderer, or an OS shutdown path may have
        // failed. Preserve dirty recovery fail-closed unless native state can
        // prove there is no unsaved work. App-level quit paths (notably Cmd+Q
        // on macOS) arrive while the main window still exists and use the
        // native confirmation below.
        if app_handle.webview_windows().is_empty() {
            // A failed or project-changed clear leaves the file in place,
            // which is safer than delaying exit with no remaining window to
            // explain it.
            let _ = app_handle
                .state::<RecoveryRuntime>()
                .clear_for_exit(&project_state, ExitRecoveryAuthorization::Clean);
            return;
        }

        let exit_guard = app_handle.state::<ExitGuard>();
        if exit_guard.allow_once.swap(false, Ordering::SeqCst) {
            return;
        }

        let project_is_dirty = lock_project(&project_state)
            .map(|project| project.is_dirty())
            .unwrap_or(true);
        if !project_is_dirty {
            match app_handle
                .state::<RecoveryRuntime>()
                .clear_for_exit(&project_state, ExitRecoveryAuthorization::Clean)
            {
                Ok(ExitRecoveryDisposition::ProjectChanged) => {
                    // A delayed edit committed after the first clean check.
                    // Continue into the native discard confirmation below.
                }
                Ok(
                    ExitRecoveryDisposition::Cleared
                    | ExitRecoveryDisposition::PreservedStartupCandidate,
                ) => return,
                Err(_) => {
                    api.prevent_exit();
                    app_handle
                        .dialog()
                        .message(
                            "The private recovery data could not be settled. The application remains open.",
                        )
                        .title("ORIGAMI2")
                        .kind(MessageDialogKind::Error)
                        .buttons(MessageDialogButtons::Ok)
                        .show(|_| {});
                    return;
                }
            }
        }

        api.prevent_exit();
        if exit_guard.dialog_open.swap(true, Ordering::SeqCst) {
            return;
        }

        let mut dialog = app_handle
            .dialog()
            .message("未保存の変更があります。変更を破棄して終了しますか？")
            .title("ORIGAMI2")
            .kind(MessageDialogKind::Warning)
            .buttons(MessageDialogButtons::OkCancelCustom(
                "変更を破棄して終了".to_owned(),
                "キャンセル".to_owned(),
            ));
        if let Some(window) = app_handle.get_webview_window("main") {
            dialog = dialog.parent(&window);
        }

        let exit_handle = app_handle.clone();
        dialog.show(move |discard_changes| {
            let exit_guard = exit_handle.state::<ExitGuard>();
            exit_guard.dialog_open.store(false, Ordering::SeqCst);
            if discard_changes {
                if exit_handle
                    .state::<RecoveryRuntime>()
                    .clear_for_exit(
                        &exit_handle.state::<AppState>(),
                        ExitRecoveryAuthorization::DiscardConfirmed,
                    )
                    .is_ok()
                {
                    exit_guard.allow_once.store(true, Ordering::SeqCst);
                    exit_handle.exit(0);
                } else {
                    exit_handle
                        .dialog()
                        .message(
                            "The private recovery data could not be settled. The application remains open.",
                        )
                        .title("ORIGAMI2")
                        .kind(MessageDialogKind::Error)
                        .buttons(MessageDialogButtons::Ok)
                        .show(|_| {});
                }
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeSet,
        fs,
        sync::{
            Arc,
            atomic::{AtomicU64, Ordering as AtomicOrdering},
            mpsc,
        },
        thread,
        time::{Duration, SystemTime, UNIX_EPOCH},
    };

    use ori_domain::{Edge, LayerContentKindV1, LayerRecordV1, Vertex};
    use ori_formats::{
        Ori2Limits, read_project_ori2_with_limits, write_project_archive_ori2, write_project_ori2,
    };
    #[cfg(target_os = "windows")]
    use std::fs::OpenOptions;

    use super::*;

    static NEXT_TEST_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn symmetry_transforms_are_exact_at_cardinal_angles() {
        assert_eq!(
            mirror_point_left_right(Point2::new(3.0, 4.0), 1.0),
            Point2::new(-1.0, 4.0)
        );
        let center = Point2::new(1.0, 2.0);
        let point = Point2::new(3.0, 4.0);
        for (angle, expected) in [
            (0.0, Point2::new(3.0, 4.0)),
            (90.0, Point2::new(-1.0, 4.0)),
            (180.0, Point2::new(-1.0, 0.0)),
            (270.0, Point2::new(3.0, 0.0)),
        ] {
            let (sin, cos) = symmetry_sin_cos(angle);
            assert_eq!(rotate_point_about(point, center, sin, cos), expected);
        }
    }

    fn execute_command(
        project: &mut ProjectState,
        expected_project_id: ProjectId,
        expected_revision: u64,
        command: Command,
    ) -> Result<ProjectSnapshot, String> {
        let expected_project_instance_id = project.instance_id;
        super::execute_command(
            project,
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
            command,
        )
    }

    fn execute_undo(
        project: &mut ProjectState,
        expected_project_id: ProjectId,
        expected_revision: u64,
    ) -> Result<ProjectSnapshot, String> {
        let expected_project_instance_id = project.instance_id;
        super::execute_undo(
            project,
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
        )
    }

    fn execute_redo(
        project: &mut ProjectState,
        expected_project_id: ProjectId,
        expected_revision: u64,
    ) -> Result<ProjectSnapshot, String> {
        let expected_project_instance_id = project.instance_id;
        super::execute_redo(
            project,
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
        )
    }

    fn execute_edge_split(
        project: &mut ProjectState,
        expected_project_id: ProjectId,
        expected_revision: u64,
        edge: EdgeId,
        fraction: f64,
    ) -> Result<ProjectSnapshot, String> {
        let expected_project_instance_id = project.instance_id;
        super::execute_edge_split(
            project,
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
            edge,
            fraction,
        )
    }

    fn execute_edge_intersection_connection(
        project: &mut ProjectState,
        expected_project_id: ProjectId,
        expected_revision: u64,
        first_edge: EdgeId,
        second_edge: EdgeId,
    ) -> Result<EdgeIntersectionResponse, String> {
        let expected_project_instance_id = project.instance_id;
        super::execute_edge_intersection_connection(
            project,
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
            first_edge,
            second_edge,
        )
    }

    fn execute_intersection_cluster_connection(
        project: &mut ProjectState,
        expected_project_id: ProjectId,
        expected_revision: u64,
        targets: Vec<IntersectionClusterTargetRequest>,
        junction_vertex_id: Option<VertexId>,
    ) -> Result<EdgeIntersectionResponse, String> {
        let expected_project_instance_id = project.instance_id;
        super::execute_intersection_cluster_connection(
            project,
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
            targets,
            junction_vertex_id,
        )
    }

    fn execute_t_junction_connection(
        project: &mut ProjectState,
        expected_project_id: ProjectId,
        expected_revision: u64,
        first_edge: EdgeId,
        second_edge: EdgeId,
    ) -> Result<TJunctionResponse, String> {
        let expected_project_instance_id = project.instance_id;
        super::execute_t_junction_connection(
            project,
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
            first_edge,
            second_edge,
        )
    }

    fn execute_boundary_split(
        project: &mut ProjectState,
        expected_project_id: ProjectId,
        expected_revision: u64,
        edge: EdgeId,
        fraction: f64,
    ) -> Result<ProjectSnapshot, String> {
        let expected_project_instance_id = project.instance_id;
        super::execute_boundary_split(
            project,
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
            edge,
            fraction,
        )
    }

    struct TestDirectory {
        path: PathBuf,
    }

    impl TestDirectory {
        fn new() -> Self {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock must follow the Unix epoch")
                .as_nanos();
            let sequence = NEXT_TEST_DIRECTORY.fetch_add(1, AtomicOrdering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "origami2-native-file-tests-{}-{timestamp}-{sequence}",
                std::process::id()
            ));
            fs::create_dir(&path).expect("create isolated native-file test directory");
            Self { path }
        }

        #[cfg(target_os = "windows")]
        fn new_relative() -> Self {
            let sequence = NEXT_TEST_DIRECTORY.fetch_add(1, AtomicOrdering::Relaxed);
            let path = PathBuf::from(format!(
                ".origami2-relative-native-file-test-{}-{sequence}",
                std::process::id()
            ));
            fs::create_dir(&path).expect("create isolated relative native-file test directory");
            Self { path }
        }

        fn join(&self, name: impl AsRef<Path>) -> PathBuf {
            self.path.join(name)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn new_project_parameters() -> NewProjectParameters {
        NewProjectParameters {
            name: "  Test sheet  ".to_owned(),
            width_expression: "210".to_owned(),
            height_expression: "297".to_owned(),
            width_mm: 210.0,
            height_mm: 297.0,
            thickness_mm: 0.2,
            cutting_allowed: true,
            front_color: RgbaColor {
                red: 10,
                green: 20,
                blue: 30,
                alpha: 240,
            },
            back_color: RgbaColor {
                red: 220,
                green: 210,
                blue: 200,
                alpha: 230,
            },
        }
    }

    fn cellular_multi_fold_project_state() -> ProjectState {
        let positions = [
            Point2::new(0.0, 0.0),
            Point2::new(2.0, 0.0),
            Point2::new(6.0, 0.0),
            Point2::new(8.0, 0.0),
            Point2::new(8.0, 6.0),
            Point2::new(6.0, 6.0),
            Point2::new(2.0, 6.0),
            Point2::new(0.0, 6.0),
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
        edges.extend([
            Edge {
                id: EdgeId::new(),
                start: vertices[1].id,
                end: vertices[6].id,
                kind: EdgeKind::Mountain,
            },
            Edge {
                id: EdgeId::new(),
                start: vertices[2].id,
                end: vertices[5].id,
                kind: EdgeKind::Valley,
            },
        ]);
        let paper = Paper {
            boundary_vertices: vertices.iter().map(|vertex| vertex.id).collect(),
            ..Paper::default()
        };
        ProjectState::new_with_paper(CreasePattern { vertices, edges }, paper)
    }

    fn four_ray_square_project_state(
        fold_endpoint_indices: [usize; 4],
        assignments: [EdgeKind; 4],
    ) -> (ProjectState, VertexId) {
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
            fold_endpoint_indices
                .into_iter()
                .zip(assignments)
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

    #[derive(Debug, PartialEq)]
    struct ProjectStateSignature {
        instance_id: ProjectId,
        project_id: ProjectId,
        document: ProjectDocument,
        editor_debug: String,
        applied_pose_authority: applied_pose::CurrentAppliedPoseAuthoritySnapshot,
        current_path: Option<PathBuf>,
        saved_revision: Option<u64>,
        saved_document: Option<ProjectDocument>,
        revision: u64,
        can_undo: bool,
        can_redo: bool,
        is_dirty: bool,
    }

    fn project_state_signature(project: &ProjectState) -> ProjectStateSignature {
        ProjectStateSignature {
            instance_id: project.instance_id,
            project_id: project.project_id,
            document: project.document(),
            editor_debug: format!("{:?}", project.editor),
            applied_pose_authority: project
                .applied_pose_authority
                .test_snapshot()
                .expect("capture applied-pose authority"),
            current_path: project.current_path.clone(),
            saved_revision: project.saved_revision,
            saved_document: project.saved_document.clone(),
            revision: project.editor.revision(),
            can_undo: project.editor.can_undo(),
            can_redo: project.editor.can_redo(),
            is_dirty: project.is_dirty(),
        }
    }

    fn geometric_constraint_binding(state: &AppState) -> (ProjectId, ProjectId, u64) {
        let project = lock_project(state).expect("lock geometric-constraint project");
        (
            project.instance_id,
            project.project_id,
            project.editor.revision(),
        )
    }

    fn geometric_constraint_project_signature(state: &AppState) -> ProjectStateSignature {
        let project = lock_project(state).expect("lock geometric-constraint project");
        project_state_signature(&project)
    }

    fn run_default_geometric_constraint_analysis(
        state: &AppState,
        binding: (ProjectId, ProjectId, u64),
    ) -> Result<GeometricConstraintPreflightResponse, String> {
        tauri::async_runtime::block_on(analyze_geometric_constraints_with_worker(
            state,
            binding.0,
            binding.1,
            binding.2,
            |pattern, document| Ok(analyze_geometric_constraint_document(&pattern, &document)),
        ))
    }

    fn wait_for_geometric_constraint_worker_idle(state: &Arc<AppState>) {
        let observer_state = Arc::clone(state);
        let (idle_tx, idle_rx) = mpsc::sync_channel(0);
        let observer = thread::spawn(move || {
            while observer_state.geometric_constraint_worker_is_busy() {
                thread::yield_now();
            }
            idle_tx.send(()).expect("announce idle worker gate");
        });
        idle_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("geometric-constraint worker gate must become idle");
        observer
            .join()
            .expect("worker-gate observer must not panic");
    }

    #[test]
    fn geometric_constraint_document_is_dirty_undoable_and_loadable() {
        let mut project = initial_project_state();
        let edge = project.editor.pattern().edges[0].id;
        let record = GeometricConstraintRecordV1 {
            id: ConstraintId::new(),
            constraint: GeometricConstraintKindV1::Horizontal { edge },
        };
        let project_id = project.project_id;

        let added = execute_command(
            &mut project,
            project_id,
            0,
            Command::AddGeometricConstraint {
                record: record.clone(),
            },
        )
        .expect("add constraint through native project bridge");
        assert_eq!(
            added.geometric_constraints.constraints,
            vec![record.clone()]
        );
        assert!(added.is_dirty);
        assert_eq!(
            project.document().geometric_constraints.constraints,
            vec![record.clone()]
        );

        let undone = execute_undo(&mut project, project_id, 1).expect("undo constraint");
        assert!(undone.geometric_constraints.is_empty());
        assert!(!undone.is_dirty);
        let redone = execute_redo(&mut project, project_id, 2).expect("redo constraint");
        assert_eq!(
            redone.geometric_constraints.constraints,
            vec![record.clone()]
        );
        assert!(redone.is_dirty);

        let document = project.document();
        let loaded =
            ProjectState::from_document(document.clone(), PathBuf::from("constraint.ori2"));
        assert_eq!(loaded.document(), document);
        assert_eq!(
            loaded.editor.geometric_constraints().constraints,
            vec![record]
        );
        assert!(!loaded.is_dirty());
        assert!(!loaded.editor.can_undo());
        assert!(!loaded.editor.can_redo());
    }

    #[test]
    fn project_layers_are_snapshotted_dirty_tracked_saved_and_reopened_with_history() {
        let mut project = initial_project_state();
        let project_id = project.project_id;
        let edge = project.editor.pattern().edges[0].id;
        let layer = LayerRecordV1 {
            id: ori_domain::LayerId::new(),
            name: "Details".to_owned(),
            content_kind: LayerContentKindV1::CreasePattern,
            visible: true,
            locked: false,
            opacity: 1.0,
        };

        let created = execute_command(
            &mut project,
            project_id,
            0,
            Command::CreateLayer {
                layer: layer.clone(),
                target_index: 1,
            },
        )
        .expect("create layer through native project bridge");
        assert_eq!(created.project_layers.layers[1], layer);
        assert!(created.project_layers.edge_assignments.is_empty());
        assert!(created.is_dirty);

        let assigned = execute_command(
            &mut project,
            project_id,
            1,
            Command::AssignEdgeToLayer {
                edge,
                layer: layer.id,
            },
        )
        .expect("assign edge through native project bridge");
        assert_eq!(assigned.project_layers.layer_for_edge(edge), layer.id);
        assert_eq!(project.document().layers, assigned.project_layers);
        assert!(project.is_dirty());

        let presented = execute_command(
            &mut project,
            project_id,
            2,
            Command::UpdateLayerPresentation {
                layer: layer.id,
                visible: false,
                locked: true,
                opacity: 0.25,
            },
        )
        .expect("update layer presentation through native project bridge");
        assert_eq!(project.document().layers, presented.project_layers);
        assert!(!presented.project_layers.layers[1].visible);
        assert!(presented.project_layers.layers[1].locked);
        assert_eq!(presented.project_layers.layers[1].opacity, 0.25);

        let document = project.document();
        let loaded_without_history =
            ProjectState::from_document(document.clone(), PathBuf::from("layers.ori2"));
        assert_eq!(
            loaded_without_history.editor.project_layers(),
            &document.layers
        );
        assert!(!loaded_without_history.is_dirty());

        let directory = TestDirectory::new();
        let path = directory.join("layer-history.ori2");
        save_project_to_path(&mut project, path.clone()).expect("save layered archive");
        assert!(!project.is_dirty());

        let mut reopened = ProjectState::new(CreasePattern::empty());
        let replaced_instance_id = reopened.instance_id;
        let replaced_project_id = reopened.project_id;
        let loaded = load_project_file(path.clone()).expect("load layered archive");
        apply_loaded_project_file(
            &mut reopened,
            replaced_instance_id,
            replaced_project_id,
            0,
            loaded,
        )
        .expect("apply layered archive");
        assert_eq!(reopened.document(), document);
        assert_eq!(reopened.editor.project_layers(), &document.layers);
        assert_eq!(snapshot(&reopened).project_layers, document.layers);
        assert!(!reopened.is_dirty());

        reopened
            .editor
            .undo(0)
            .expect("undo reopened layer presentation");
        assert!(reopened.editor.project_layers().layers[1].visible);
        assert!(!reopened.editor.project_layers().layers[1].locked);
        assert_eq!(reopened.editor.project_layers().layers[1].opacity, 1.0);
        reopened.editor.undo(1).expect("undo reopened assignment");
        assert_eq!(
            reopened.editor.project_layers().layer_for_edge(edge),
            ori_domain::DEFAULT_PROJECT_LAYER_ID
        );
        assert!(reopened.is_dirty());
        reopened
            .editor
            .undo(2)
            .expect("undo reopened layer creation");
        assert_eq!(
            reopened.editor.project_layers(),
            &ProjectLayerDocumentV1::default()
        );
        reopened
            .editor
            .redo(3)
            .expect("redo reopened layer creation");
        reopened.editor.redo(4).expect("redo reopened assignment");
        reopened
            .editor
            .redo(5)
            .expect("redo reopened layer presentation");
        assert_eq!(reopened.document(), document);
        assert!(!reopened.is_dirty());
    }

    #[test]
    fn project_layer_ipc_helpers_guard_binding_and_apply_every_supported_mutation() {
        let mut project = initial_project_state();
        let project_instance_id = project.instance_id;
        let project_id = project.project_id;
        let edge = project.editor.pattern().edges[0].id;
        let original_document = project.document();

        assert!(
            create_project_layer_in_project(
                &mut project,
                ProjectId::new(),
                project_id,
                0,
                "Foreign".to_owned(),
                LayerContentKindV1::CreasePattern,
            )
            .is_err()
        );
        assert_eq!(project.document(), original_document);
        assert_eq!(project.editor.revision(), 0);

        let created_crease = create_project_layer_in_project(
            &mut project,
            project_instance_id,
            project_id,
            0,
            "Details".to_owned(),
            LayerContentKindV1::CreasePattern,
        )
        .expect("create crease-pattern layer");
        let crease_layer = created_crease.project_layers.layers[1].id;
        assert_eq!(created_crease.revision, 1);

        let created_annotation = create_project_layer_in_project(
            &mut project,
            project_instance_id,
            project_id,
            1,
            "Notes".to_owned(),
            LayerContentKindV1::Annotation,
        )
        .expect("create empty annotation layer");
        let annotation_layer = created_annotation.project_layers.layers[2].id;
        assert_eq!(
            created_annotation.project_layers.layers[2].content_kind,
            LayerContentKindV1::Annotation
        );

        let renamed = rename_project_layer_in_project(
            &mut project,
            project_instance_id,
            project_id,
            2,
            crease_layer,
            "Primary folds".to_owned(),
        )
        .expect("rename project layer");
        assert_eq!(renamed.project_layers.layers[1].name, "Primary folds");

        let presented = update_project_layer_presentation_in_project(
            &mut project,
            project_instance_id,
            project_id,
            3,
            crease_layer,
            ProjectLayerPresentationInput {
                visible: false,
                locked: true,
                opacity: 0.4,
            },
        )
        .expect("update project layer presentation");
        let presented_layer = presented
            .project_layers
            .layers
            .iter()
            .find(|layer| layer.id == crease_layer)
            .expect("presented layer");
        assert!(!presented_layer.visible);
        assert!(presented_layer.locked);
        assert_eq!(presented_layer.opacity, 0.4);

        let unlocked = update_project_layer_presentation_in_project(
            &mut project,
            project_instance_id,
            project_id,
            4,
            crease_layer,
            ProjectLayerPresentationInput {
                visible: true,
                locked: false,
                opacity: 0.4,
            },
        )
        .expect("unlock project layer");
        assert!(!unlocked.project_layers.layers[1].locked);

        let moved = move_project_layer_in_project(
            &mut project,
            project_instance_id,
            project_id,
            5,
            annotation_layer,
            0,
        )
        .expect("move project layer");
        assert_eq!(moved.project_layers.layers[0].id, annotation_layer);

        let assigned = assign_edge_to_project_layer_in_project(
            &mut project,
            project_instance_id,
            project_id,
            6,
            edge,
            crease_layer,
        )
        .expect("assign selected edge to crease-pattern layer");
        assert_eq!(assigned.project_layers.layer_for_edge(edge), crease_layer);

        let deleted = delete_project_layer_in_project(
            &mut project,
            project_instance_id,
            project_id,
            7,
            crease_layer,
        )
        .expect("delete project layer");
        assert_eq!(
            deleted.project_layers.layer_for_edge(edge),
            ori_domain::DEFAULT_PROJECT_LAYER_ID
        );
        assert!(
            deleted
                .project_layers
                .layers
                .iter()
                .all(|layer| layer.id != crease_layer)
        );

        assert!(
            delete_project_layer_in_project(
                &mut project,
                project_instance_id,
                project_id,
                8,
                ori_domain::DEFAULT_PROJECT_LAYER_ID,
            )
            .is_err()
        );
        assert_eq!(project.editor.revision(), 8);
        assert_eq!(project.editor.project_layers(), &deleted.project_layers);
    }

    #[test]
    fn project_layer_presentation_ipc_input_is_a_strict_nested_record() {
        let admitted = serde_json::from_value::<ProjectLayerPresentationInput>(serde_json::json!({
            "visible": false,
            "locked": true,
            "opacity": 0.4
        }))
        .expect("strict presentation input");
        assert!(!admitted.visible);
        assert!(admitted.locked);
        assert_eq!(admitted.opacity, 0.4);
        assert!(
            serde_json::from_value::<ProjectLayerPresentationInput>(serde_json::json!({
                "visible": false,
                "locked": true,
                "opacity": 0.4,
                "future": "rejected"
            }),)
            .is_err()
        );
        assert!(
            serde_json::from_value::<ProjectLayerPresentationInput>(serde_json::json!({
                "visible": false,
                "opacity": 0.4
            }),)
            .is_err()
        );
    }

    #[test]
    fn geometric_constraint_preflight_exposes_all_three_safe_states() {
        let project = initial_project_state();
        let pattern = project.editor.pattern();
        let first_edge = pattern.edges[0].id;
        let second_edge = pattern.edges[1].id;
        let horizontal = GeometricConstraintRecordV1 {
            id: ConstraintId::new(),
            constraint: GeometricConstraintKindV1::Horizontal { edge: first_edge },
        };

        let no_direct = GeometricConstraintDocumentV1 {
            schema_version: ori_domain::GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
            constraints: vec![horizontal.clone()],
        };
        assert_eq!(
            analyze_geometric_constraint_document(pattern, &no_direct),
            GeometricConstraintPreflightResult::NoDirectConflict
        );

        let direct = GeometricConstraintDocumentV1 {
            schema_version: ori_domain::GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
            constraints: vec![
                horizontal,
                GeometricConstraintRecordV1 {
                    id: ConstraintId::new(),
                    constraint: GeometricConstraintKindV1::Vertical { edge: first_edge },
                },
            ],
        };
        let GeometricConstraintPreflightResult::DirectConflict { conflicts } =
            analyze_geometric_constraint_document(pattern, &direct)
        else {
            panic!("horizontal plus vertical must be a direct conflict");
        };
        assert_eq!(conflicts.len(), 1);

        let solver_required = GeometricConstraintDocumentV1 {
            schema_version: ori_domain::GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
            constraints: vec![GeometricConstraintRecordV1 {
                id: ConstraintId::new(),
                constraint: GeometricConstraintKindV1::LengthRatio {
                    numerator_edge: first_edge,
                    denominator_edge: second_edge,
                    ratio: 2.0,
                },
            }],
        };
        assert!(matches!(
            analyze_geometric_constraint_document(pattern, &solver_required),
            GeometricConstraintPreflightResult::Unknown {
                reason: GeometricConstraintUnknownReason::SolverRequiredConstraintKinds,
                ..
            }
        ));
    }

    fn oversized_geometric_constraint_vertex_pattern() -> CreasePattern {
        let vertices = (0..=ori_domain::DEFAULT_MAX_CONSTRAINT_VERTICES)
            .map(|index| Vertex {
                id: VertexId::new(),
                position: Point2::new(index as f64, (index % 2) as f64),
            })
            .collect::<Vec<_>>();
        let edges = vec![Edge {
            id: EdgeId::new(),
            start: vertices[0].id,
            end: vertices[1].id,
            kind: EdgeKind::Mountain,
        }];
        CreasePattern { vertices, edges }
    }

    #[test]
    fn geometric_constraint_empty_v1_preflight_skips_oversized_and_repair_geometry() {
        let empty = GeometricConstraintDocumentV1::default();
        let empty_before = empty.clone();
        let oversized = oversized_geometric_constraint_vertex_pattern();
        let oversized_before = oversized.clone();

        assert_eq!(oversized.vertices.len(), 100_001);
        assert_eq!(
            analyze_geometric_constraint_document(&oversized, &empty),
            GeometricConstraintPreflightResult::NoDirectConflict
        );
        assert_eq!(oversized, oversized_before);
        assert_eq!(empty, empty_before);

        let duplicate_vertex = VertexId::new();
        let repair_geometry = CreasePattern {
            vertices: vec![
                Vertex {
                    id: duplicate_vertex,
                    position: Point2::new(0.0, 0.0),
                },
                Vertex {
                    id: duplicate_vertex,
                    position: Point2::new(1.0, 0.0),
                },
            ],
            edges: vec![Edge {
                id: EdgeId::new(),
                start: duplicate_vertex,
                end: VertexId::new(),
                kind: EdgeKind::Valley,
            }],
        };
        let repair_geometry_before = repair_geometry.clone();

        assert_eq!(
            analyze_geometric_constraint_document(&repair_geometry, &empty),
            GeometricConstraintPreflightResult::NoDirectConflict
        );
        assert_eq!(repair_geometry, repair_geometry_before);
        assert_eq!(empty, empty_before);
    }

    #[test]
    fn geometric_constraint_empty_invalid_schema_remains_unknown() {
        let pattern = CreasePattern::empty();
        let invalid = GeometricConstraintDocumentV1 {
            schema_version: ori_domain::GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1 + 1,
            constraints: Vec::new(),
        };
        let pattern_before = pattern.clone();
        let invalid_before = invalid.clone();

        assert_eq!(
            analyze_geometric_constraint_document(&pattern, &invalid),
            GeometricConstraintPreflightResult::Unknown {
                reason: GeometricConstraintUnknownReason::InvalidDocumentOrGeometry,
                unchecked_constraint_ids: Vec::new(),
            }
        );
        assert_eq!(pattern, pattern_before);
        assert_eq!(invalid, invalid_before);
    }

    #[test]
    fn geometric_constraint_non_empty_oversized_geometry_remains_unknown() {
        let pattern = oversized_geometric_constraint_vertex_pattern();
        let constraint_id = ConstraintId::new();
        let document = GeometricConstraintDocumentV1 {
            schema_version: ori_domain::GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
            constraints: vec![GeometricConstraintRecordV1 {
                id: constraint_id,
                constraint: GeometricConstraintKindV1::Horizontal {
                    edge: pattern.edges[0].id,
                },
            }],
        };
        let pattern_before = pattern.clone();
        let document_before = document.clone();

        assert_eq!(
            analyze_geometric_constraint_document(&pattern, &document),
            GeometricConstraintPreflightResult::Unknown {
                reason: GeometricConstraintUnknownReason::InvalidDocumentOrGeometry,
                unchecked_constraint_ids: vec![constraint_id],
            }
        );
        assert_eq!(pattern, pattern_before);
        assert_eq!(document, document_before);
    }

    #[test]
    fn geometric_constraint_preflight_fails_closed_for_invalid_references() {
        let project = initial_project_state();
        let first = ConstraintId::new();
        let second = ConstraintId::new();
        let invalid = GeometricConstraintDocumentV1 {
            schema_version: ori_domain::GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
            constraints: vec![
                GeometricConstraintRecordV1 {
                    id: first,
                    constraint: GeometricConstraintKindV1::Horizontal {
                        edge: EdgeId::new(),
                    },
                },
                GeometricConstraintRecordV1 {
                    id: second,
                    constraint: GeometricConstraintKindV1::Vertical {
                        edge: EdgeId::new(),
                    },
                },
            ],
        };

        let GeometricConstraintPreflightResult::Unknown {
            reason,
            unchecked_constraint_ids,
        } = analyze_geometric_constraint_document(project.editor.pattern(), &invalid)
        else {
            panic!("invalid references must not be reported as safe");
        };
        assert_eq!(
            reason,
            GeometricConstraintUnknownReason::InvalidDocumentOrGeometry
        );
        let mut expected = vec![first, second];
        expected.sort_unstable_by_key(ConstraintId::canonical_bytes);
        assert_eq!(unchecked_constraint_ids, expected);
    }

    #[test]
    fn geometric_constraint_worker_gate_is_exclusive_and_releases_with_its_permit() {
        let gate = GeometricConstraintWorkerGate::default();
        let permit = gate.try_acquire().expect("first worker permit");
        assert!(gate.is_busy());
        assert!(
            gate.try_acquire().is_none(),
            "parallel preflight must not allocate another worker"
        );
        drop(permit);
        assert!(!gate.is_busy());
        assert!(gate.try_acquire().is_some());
    }

    #[test]
    fn abandoned_geometric_constraint_waiter_keeps_gate_until_worker_exit_then_retries() {
        let state = Arc::new(AppState::new(initial_project_state()));
        let binding = geometric_constraint_binding(&state);
        let before = geometric_constraint_project_signature(&state);
        let worker_state = Arc::clone(&state);
        let (entered_tx, entered_rx) = mpsc::sync_channel(0);
        let (release_tx, release_rx) = mpsc::sync_channel(0);

        let waiting = tauri::async_runtime::spawn(async move {
            analyze_geometric_constraints_with_worker(
                &worker_state,
                binding.0,
                binding.1,
                binding.2,
                move |pattern, document| {
                    entered_tx.send(()).expect("announce worker entry");
                    release_rx.recv().expect("release constraint worker");
                    Ok(analyze_geometric_constraint_document(&pattern, &document))
                },
            )
            .await
        });

        entered_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("geometric-constraint worker must start");
        assert!(state.geometric_constraint_worker_is_busy());
        waiting.abort();
        assert!(
            tauri::async_runtime::block_on(waiting).is_err(),
            "the abandoned waiting future must be cancelled"
        );
        assert!(
            state.geometric_constraint_worker_is_busy(),
            "cancelling the waiter must not release a running blocking worker"
        );

        let busy_error = tauri::async_runtime::block_on(analyze_geometric_constraints_with_worker(
            &state,
            binding.0,
            binding.1,
            binding.2,
            |_, _| {
                panic!("a busy gate must reject before invoking another worker");
            },
        ))
        .expect_err("parallel analysis must be rejected");
        assert_eq!(busy_error, GEOMETRIC_CONSTRAINT_ANALYSIS_BUSY_MESSAGE);

        release_tx
            .send(())
            .expect("release abandoned geometric-constraint worker");
        wait_for_geometric_constraint_worker_idle(&state);
        assert!(!state.geometric_constraint_worker_is_busy());

        let retried = run_default_geometric_constraint_analysis(&state, binding)
            .expect("the gate must be reusable after the blocking worker exits");
        assert_eq!(retried.project_instance_id, binding.0);
        assert_eq!(retried.project_id, binding.1);
        assert_eq!(retried.revision, binding.2);
        assert_eq!(
            retried.result,
            GeometricConstraintPreflightResult::NoDirectConflict
        );
        assert_eq!(geometric_constraint_project_signature(&state), before);
    }

    #[test]
    fn geometric_constraint_worker_releases_project_lock_and_discards_reopen_aba_completion() {
        let state = Arc::new(AppState::new(initial_project_state()));
        let stale_binding = geometric_constraint_binding(&state);
        let document = {
            let project = lock_project(&state).expect("capture original project document");
            project.document()
        };
        let worker_state = Arc::clone(&state);
        let (entered_tx, entered_rx) = mpsc::sync_channel(0);
        let (release_tx, release_rx) = mpsc::sync_channel(0);

        let analysis = thread::spawn(move || {
            tauri::async_runtime::block_on(analyze_geometric_constraints_with_worker(
                &worker_state,
                stale_binding.0,
                stale_binding.1,
                stale_binding.2,
                move |pattern, constraints| {
                    entered_tx.send(()).expect("announce worker entry");
                    release_rx.recv().expect("release constraint worker");
                    Ok(analyze_geometric_constraint_document(
                        &pattern,
                        &constraints,
                    ))
                },
            ))
        });

        entered_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("geometric-constraint worker must start");
        let (current_binding, reopened_before) = {
            let Ok(mut project) = state.0.try_lock() else {
                release_tx
                    .send(())
                    .expect("release blocked geometric-constraint worker");
                analysis
                    .join()
                    .expect("analysis caller must not panic")
                    .expect("unchanged analysis must finish");
                panic!("the project lock must be released during constraint analysis");
            };
            *project =
                ProjectState::from_document(document, PathBuf::from("same-constraints.ori2"));
            assert_eq!(project.project_id, stale_binding.1);
            assert_eq!(project.editor.revision(), stale_binding.2);
            assert_ne!(project.instance_id, stale_binding.0);
            (
                (
                    project.instance_id,
                    project.project_id,
                    project.editor.revision(),
                ),
                project_state_signature(&project),
            )
        };

        release_tx
            .send(())
            .expect("release stale geometric-constraint worker");
        let stale_error = analysis
            .join()
            .expect("analysis caller must not panic")
            .expect_err("same-ID and revision reopen must reject stale completion");
        assert_eq!(
            stale_error,
            "the open project instance changed while the file dialog was open"
        );
        assert!(!state.geometric_constraint_worker_is_busy());
        assert_eq!(
            geometric_constraint_project_signature(&state),
            reopened_before
        );

        let retried = run_default_geometric_constraint_analysis(&state, current_binding)
            .expect("the reopened instance must be able to retry");
        assert_eq!(retried.project_instance_id, current_binding.0);
        assert_eq!(retried.project_id, current_binding.1);
        assert_eq!(retried.revision, current_binding.2);
        assert_eq!(
            geometric_constraint_project_signature(&state),
            reopened_before
        );
    }

    #[test]
    fn geometric_constraint_worker_failures_are_redacted_release_gate_and_preserve_state() {
        let state = Arc::new(AppState::new(initial_project_state()));
        let binding = geometric_constraint_binding(&state);
        let before = geometric_constraint_project_signature(&state);
        let private_failure = r"C:\Users\alice\private-constraints.ori2; constraint_id=secret-17";

        let reported_error =
            tauri::async_runtime::block_on(analyze_geometric_constraints_with_worker(
                &state,
                binding.0,
                binding.1,
                binding.2,
                move |_, _| Err(private_failure.to_owned()),
            ))
            .expect_err("a reported worker failure must fail the command");
        assert_eq!(reported_error, GEOMETRIC_CONSTRAINT_ANALYSIS_FAILED_MESSAGE);
        assert!(!reported_error.contains("alice"));
        assert!(!reported_error.contains("private-constraints"));
        assert!(!reported_error.contains("secret-17"));
        assert!(!state.geometric_constraint_worker_is_busy());
        assert_eq!(geometric_constraint_project_signature(&state), before);
        run_default_geometric_constraint_analysis(&state, binding)
            .expect("the gate must be reusable after a reported worker failure");

        let private_panic = r"C:\Users\bob\private-constraints.ori2; constraint_id=panic-secret-23";
        let panic_error =
            tauri::async_runtime::block_on(analyze_geometric_constraints_with_worker(
                &state,
                binding.0,
                binding.1,
                binding.2,
                move |_, _| -> Result<GeometricConstraintPreflightResult, String> {
                    panic!("{private_panic}");
                },
            ))
            .expect_err("a panicking worker must fail the command");
        assert_eq!(panic_error, GEOMETRIC_CONSTRAINT_ANALYSIS_FAILED_MESSAGE);
        assert!(!panic_error.contains("bob"));
        assert!(!panic_error.contains("private-constraints"));
        assert!(!panic_error.contains("panic-secret-23"));
        assert!(!state.geometric_constraint_worker_is_busy());
        assert_eq!(geometric_constraint_project_signature(&state), before);
        run_default_geometric_constraint_analysis(&state, binding)
            .expect("the gate must be reusable after a panicking worker");
        assert_eq!(geometric_constraint_project_signature(&state), before);
    }

    #[test]
    fn geometric_constraint_capture_rejections_and_success_all_release_gate() {
        let state = Arc::new(AppState::new(initial_project_state()));
        let binding = geometric_constraint_binding(&state);
        let before = geometric_constraint_project_signature(&state);
        let rejection_cases = [
            (
                (ProjectId::new(), binding.1, binding.2),
                "the open project instance changed while the file dialog was open",
            ),
            (
                (binding.0, ProjectId::new(), binding.2),
                "the active project changed before the command was applied",
            ),
            (
                (binding.0, binding.1, binding.2 + 1),
                "the project changed while the file dialog was open",
            ),
        ];

        for (rejected_binding, expected_error) in rejection_cases {
            let error = tauri::async_runtime::block_on(analyze_geometric_constraints_with_worker(
                &state,
                rejected_binding.0,
                rejected_binding.1,
                rejected_binding.2,
                |_, _| {
                    panic!("capture rejection must happen before worker invocation");
                },
            ))
            .expect_err("invalid capture binding must be rejected");
            assert_eq!(error, expected_error);
            assert!(!state.geometric_constraint_worker_is_busy());
            assert_eq!(geometric_constraint_project_signature(&state), before);
        }

        let response = run_default_geometric_constraint_analysis(&state, binding)
            .expect("a valid capture and worker must succeed");
        assert_eq!(response.project_instance_id, binding.0);
        assert_eq!(response.project_id, binding.1);
        assert_eq!(response.revision, binding.2);
        assert!(!state.geometric_constraint_worker_is_busy());
        assert_eq!(geometric_constraint_project_signature(&state), before);
    }

    #[test]
    fn topology_bridge_returns_revision_bound_boundary_snapshot_without_mutation() {
        let project = initial_project_state();
        let before = project_state_signature(&project);
        let input =
            capture_topology_input(&project, project.project_id, 0).expect("capture initial sheet");
        let topology = input.analyze();

        let response =
            finish_topology_response(&project, &input, topology).expect("finish current topology");

        assert_eq!(response.project_id, project.project_id);
        assert_eq!(response.revision, 0);
        assert!(response.simulation_ready);
        assert!(response.issues.is_empty());
        let snapshot = response.snapshot.expect("boundary snapshot");
        assert_eq!(snapshot.source_revision, response.revision);
        assert_eq!(snapshot.faces.len(), 1);
        assert!(snapshot.hinge_adjacency.is_empty());
        assert_eq!(project_state_signature(&project), before);
    }

    #[test]
    fn topology_bridge_returns_two_faces_and_one_hinge_for_one_fold() {
        let mut project = initial_project_state();
        let fold = EdgeId::new();
        let endpoints = [
            project.editor.paper().boundary_vertices[0],
            project.editor.paper().boundary_vertices[2],
        ];
        let project_id = project.project_id;
        execute_command(
            &mut project,
            project_id,
            0,
            Command::AddEdge {
                id: fold,
                start: endpoints[0],
                end: endpoints[1],
                kind: EdgeKind::Mountain,
            },
        )
        .expect("add one fold");
        let before = project_state_signature(&project);
        let input = capture_topology_input(&project, project_id, 1).expect("capture fold");

        let response = finish_topology_response(&project, &input, input.analyze())
            .expect("finish fold topology");

        assert!(response.simulation_ready);
        assert!(response.issues.is_empty());
        let snapshot = response.snapshot.expect("fold snapshot");
        assert_eq!(snapshot.source_revision, 1);
        assert_eq!(snapshot.faces.len(), 2);
        assert_eq!(snapshot.hinge_adjacency.len(), 1);
        assert_eq!(snapshot.hinge_adjacency[0].edge, fold);
        assert_eq!(project_state_signature(&project), before);
    }

    #[test]
    fn instruction_pose_accepts_planar_and_complete_tree_models() {
        let project = initial_project_state();
        let input = capture_topology_input(&project, project.project_id, 0)
            .expect("capture planar instruction model");
        let topology = input.analyze();
        let planar = instruction_pose_from_topology(
            topology
                .simulation_snapshot()
                .expect("planar topology must be simulation-ready"),
            "0".repeat(64),
            None,
            Vec::new(),
        )
        .expect("accept planar instruction pose");
        assert_eq!(planar.fixed_face, None);
        assert!(planar.hinge_angles.is_empty());

        let mut folded = initial_project_state();
        let fold = EdgeId::new();
        let boundary = folded.editor.paper().boundary_vertices.clone();
        let project_id = folded.project_id;
        execute_command(
            &mut folded,
            project_id,
            0,
            Command::AddEdge {
                id: fold,
                start: boundary[0],
                end: boundary[2],
                kind: EdgeKind::Mountain,
            },
        )
        .expect("add one instruction hinge");
        let input = capture_topology_input(&folded, project_id, 1).expect("capture fold model");
        let topology = input.analyze();
        let snapshot = topology
            .simulation_snapshot()
            .expect("one-fold topology must be simulation-ready");
        let fixed_face = snapshot.faces[0].id;
        let pose = instruction_pose_from_topology(
            snapshot,
            folded.editor.fold_model_fingerprint_v1(),
            Some(fixed_face),
            vec![InstructionHingeAngle {
                edge: fold,
                angle_degrees: 37.5,
            }],
        )
        .expect("accept complete one-fold instruction pose");

        assert_eq!(pose.fixed_face, Some(fixed_face));
        assert_eq!(pose.hinge_angles.len(), 1);
        assert_eq!(pose.hinge_angles[0].edge, fold);
        assert_eq!(pose.hinge_angles[0].angle_degrees, 37.5);
        assert_eq!(
            pose.source_model_fingerprint,
            folded.editor.fold_model_fingerprint_v1()
        );
    }

    #[test]
    fn instruction_pose_rejects_wrong_faces_incomplete_hinges_and_bad_angles() {
        let mut project = initial_project_state();
        let fold = EdgeId::new();
        let boundary = project.editor.paper().boundary_vertices.clone();
        let project_id = project.project_id;
        execute_command(
            &mut project,
            project_id,
            0,
            Command::AddEdge {
                id: fold,
                start: boundary[0],
                end: boundary[2],
                kind: EdgeKind::Valley,
            },
        )
        .expect("add one instruction hinge");
        let input = capture_topology_input(&project, project_id, 1).expect("capture fold model");
        let topology = input.analyze();
        let snapshot = topology
            .simulation_snapshot()
            .expect("one-fold topology must be simulation-ready");
        let fingerprint = project.editor.fold_model_fingerprint_v1();

        assert_eq!(
            instruction_pose_from_topology(
                snapshot,
                fingerprint.clone(),
                None,
                vec![InstructionHingeAngle {
                    edge: fold,
                    angle_degrees: 45.0,
                }],
            )
            .expect_err("a folded pose needs a fixed face"),
            "a folded instruction pose requires a fixed face"
        );
        assert_eq!(
            instruction_pose_from_topology(
                snapshot,
                fingerprint.clone(),
                Some(FaceId::new()),
                vec![InstructionHingeAngle {
                    edge: fold,
                    angle_degrees: 45.0,
                }],
            )
            .expect_err("the fixed face must be current"),
            "the fixed face does not exist in the current fold model"
        );
        assert_eq!(
            instruction_pose_from_topology(
                snapshot,
                fingerprint.clone(),
                Some(snapshot.faces[0].id),
                Vec::new(),
            )
            .expect_err("every hinge is required"),
            "the instruction pose must contain every current hinge exactly once"
        );
        assert_eq!(
            instruction_pose_from_topology(
                snapshot,
                fingerprint,
                Some(snapshot.faces[0].id),
                vec![InstructionHingeAngle {
                    edge: fold,
                    angle_degrees: f64::NAN,
                }],
            )
            .expect_err("non-finite angles are rejected"),
            "instruction hinge angles must be finite"
        );
    }

    #[test]
    fn instruction_pose_rejects_fold_graph_cycles() {
        let (project, _) = four_ray_square_project_state(
            [1, 3, 5, 7],
            [
                EdgeKind::Mountain,
                EdgeKind::Valley,
                EdgeKind::Mountain,
                EdgeKind::Valley,
            ],
        );
        let input = capture_topology_input(&project, project.project_id, 0)
            .expect("capture cyclic fold model");
        let topology = input.analyze();
        let snapshot = topology
            .simulation_snapshot()
            .expect("the topology layer admits the cyclic model");
        let hinge_angles = snapshot
            .hinge_adjacency
            .iter()
            .map(|hinge| InstructionHingeAngle {
                edge: hinge.edge,
                angle_degrees: 0.0,
            })
            .collect::<Vec<_>>();

        assert_eq!(
            instruction_pose_from_topology(
                snapshot,
                project.editor.fold_model_fingerprint_v1(),
                Some(snapshot.faces[0].id),
                hinge_angles,
            )
            .expect_err("the first instruction player supports trees only"),
            "instruction poses currently require a tree-shaped fold graph"
        );
    }

    #[test]
    fn named_technique_timeline_proposal_is_strict_bounded_and_ordered() {
        let valid = serde_json::json!({
            "schema_version": 1,
            "package_id": "builtin.origami2",
            "technique_id": "inside-reverse",
            "technique_version": 1,
            "steps": [
                {
                    "source_kind": "technique",
                    "source_id": "inside-reverse",
                    "chunk_index": 1,
                    "chunk_count": 1,
                    "title": "Technique",
                    "description": "source-json-v1:\n{}",
                    "caution": "description only",
                    "duration_ms": 1500
                },
                {
                    "source_kind": "operation",
                    "source_id": "open",
                    "chunk_index": 1,
                    "chunk_count": 2,
                    "title": "Operation (1/2)",
                    "description": "first",
                    "caution": "no physical command",
                    "duration_ms": 1500
                },
                {
                    "source_kind": "operation",
                    "source_id": "open",
                    "chunk_index": 2,
                    "chunk_count": 2,
                    "title": "Operation (2/2)",
                    "description": "second",
                    "caution": "no physical command",
                    "duration_ms": 1500
                }
            ]
        });
        let proposal = parse_named_technique_timeline_proposal(
            &serde_json::to_string(&valid).expect("proposal JSON"),
        )
        .expect("valid proposal");
        assert_eq!(proposal.steps.len(), 3);

        let mut invalid_values = Vec::new();
        let mut unknown_root = valid.clone();
        unknown_root["private_path"] = serde_json::Value::String("secret".to_owned());
        invalid_values.push(unknown_root);
        let mut unknown_step = valid.clone();
        unknown_step["steps"][0]["fixed_face"] = serde_json::Value::Null;
        invalid_values.push(unknown_step);
        let mut wrong_first_kind = valid.clone();
        wrong_first_kind["steps"][0]["source_kind"] =
            serde_json::Value::String("operation".to_owned());
        invalid_values.push(wrong_first_kind);
        let mut wrong_technique_source = valid.clone();
        wrong_technique_source["steps"][0]["source_id"] =
            serde_json::Value::String("other".to_owned());
        invalid_values.push(wrong_technique_source);
        let mut incomplete_chunks = valid.clone();
        incomplete_chunks["steps"]
            .as_array_mut()
            .expect("steps")
            .pop();
        invalid_values.push(incomplete_chunks);
        let mut repeated_source = valid.clone();
        repeated_source["steps"]
            .as_array_mut()
            .expect("steps")
            .push(serde_json::json!({
                "source_kind": "operation",
                "source_id": "open",
                "chunk_index": 1,
                "chunk_count": 1,
                "title": "Repeated",
                "description": "repeated",
                "caution": "",
                "duration_ms": 1500
            }));
        invalid_values.push(repeated_source);
        let mut invalid_identifier = valid.clone();
        invalid_identifier["package_id"] = serde_json::Value::String("../private".to_owned());
        invalid_values.push(invalid_identifier);

        for invalid in invalid_values {
            assert_eq!(
                parse_named_technique_timeline_proposal(
                    &serde_json::to_string(&invalid).expect("invalid fixture JSON"),
                )
                .expect_err("invalid proposal"),
                "the named-technique timeline proposal is invalid"
            );
        }
        assert_eq!(
            parse_named_technique_timeline_proposal(
                &" ".repeat(MAX_NAMED_TECHNIQUE_TIMELINE_PROPOSAL_BYTES + 1),
            )
            .expect_err("oversized proposal"),
            "the named-technique timeline proposal is too large"
        );
    }

    #[test]
    fn instruction_step_updates_snapshot_document_dirty_state_and_history() {
        let mut project = initial_project_state();
        let project_id = project.project_id;
        let fingerprint = project.editor.fold_model_fingerprint_v1();
        let step_id = InstructionStepId::new();
        let response = execute_command(
            &mut project,
            project_id,
            0,
            Command::AddInstructionStep {
                step: InstructionStep {
                    id: step_id,
                    title: "折る前".to_owned(),
                    description: "平らな開始姿勢".to_owned(),
                    caution: String::new(),
                    duration_ms: 1_500,
                    visual: Default::default(),
                    pose: InstructionPose {
                        model: InstructionPoseModel::AbsoluteHingeAnglesV1,
                        source_model_fingerprint: fingerprint.clone(),
                        fixed_face: None,
                        hinge_angles: Vec::new(),
                    },
                },
            },
        )
        .expect("add planar instruction step");

        assert_eq!(response.revision, 1);
        assert!(response.is_dirty);
        assert_eq!(response.fold_model_fingerprint, fingerprint);
        assert_eq!(response.instruction_timeline.steps.len(), 1);
        assert_eq!(response.instruction_timeline.steps[0].id, step_id);
        assert_eq!(
            project.document().instruction_timeline,
            response.instruction_timeline
        );

        let bytes = write_project_ori2(&project.document()).expect("persist instruction timeline");
        let restored = read_project_ori2_with_limits(&bytes, Ori2Limits::default())
            .expect("restore instruction timeline");
        assert_eq!(
            restored.instruction_timeline,
            project.document().instruction_timeline
        );

        project.editor.undo(1).expect("undo instruction addition");
        assert!(project.editor.instruction_timeline().steps.is_empty());
        assert!(!project.is_dirty());
        project.editor.redo(2).expect("redo instruction addition");
        assert_eq!(project.editor.instruction_timeline().steps[0].id, step_id);
        assert!(project.is_dirty());
    }

    #[test]
    fn loaded_current_instruction_poses_are_semantically_checked_but_stale_ones_survive() {
        let project = initial_project_state();
        let mut document = project.document();
        let current_fingerprint = project.editor.fold_model_fingerprint_v1();
        let invalid_current_step = InstructionStep {
            id: InstructionStepId::new(),
            title: "不正な現在姿勢".to_owned(),
            description: String::new(),
            caution: String::new(),
            duration_ms: 1_000,
            visual: Default::default(),
            pose: InstructionPose {
                model: InstructionPoseModel::AbsoluteHingeAnglesV1,
                source_model_fingerprint: current_fingerprint,
                fixed_face: Some(FaceId::new()),
                hinge_angles: Vec::new(),
            },
        };
        document
            .instruction_timeline
            .steps
            .push(invalid_current_step.clone());

        assert_eq!(
            validate_document_instruction_poses(&document)
                .expect_err("current malformed pose must fail semantic loading"),
            "instruction step 1 is invalid: a planar instruction pose must not specify a fixed face"
        );

        document.instruction_timeline.steps[0]
            .pose
            .source_model_fingerprint = "f".repeat(64);
        validate_document_instruction_poses(&document)
            .expect("an old-model pose remains loadable as an editable stale step");
    }

    #[test]
    fn delayed_instruction_pose_cannot_land_after_reopening_the_same_document() {
        let project = initial_project_state();
        let project_id = project.project_id;
        let input =
            capture_topology_input(&project, project_id, 0).expect("capture instruction topology");
        let topology = input.analyze();
        let analyzed = AnalyzedInstructionPose {
            project_instance_id: project.instance_id,
            input,
            topology,
            fixed_face: None,
            hinge_angles: Vec::new(),
        };

        let reopened =
            ProjectState::from_document(project.document(), PathBuf::from("same-project.ori2"));
        assert_eq!(reopened.project_id, project_id);
        assert_eq!(reopened.editor.revision(), 0);
        assert_eq!(reopened.editor.pattern(), project.editor.pattern());
        assert_eq!(reopened.editor.paper(), project.editor.paper());
        assert_ne!(reopened.instance_id, project.instance_id);
        let before = project_state_signature(&reopened);

        assert_eq!(
            super::finish_instruction_pose(
                &reopened,
                reopened.instance_id,
                project_id,
                0,
                analyzed,
            )
            .expect_err("an old open-instance analysis must not mutate the reopened project"),
            "the open project instance changed while the instruction pose was being analyzed"
        );
        assert_eq!(project_state_signature(&reopened), before);
    }

    #[test]
    fn instruction_pose_capture_rejects_same_document_revision_after_reopen_aba() {
        let project = initial_project_state();
        let stale_instance_id = project.instance_id;
        let expected_project_id = project.project_id;
        let expected_revision = project.editor.revision();
        let reopened =
            ProjectState::from_document(project.document(), PathBuf::from("same-project.ori2"));
        assert_eq!(reopened.project_id, expected_project_id);
        assert_eq!(reopened.editor.revision(), expected_revision);
        assert_ne!(reopened.instance_id, stale_instance_id);
        let state = AppState::new(reopened);
        let before = {
            let project = lock_project(&state).expect("lock reopened project");
            project_state_signature(&project)
        };

        let result = tauri::async_runtime::block_on(analyze_instruction_pose(
            &state,
            stale_instance_id,
            expected_project_id,
            expected_revision,
            None,
            Vec::new(),
        ));
        let error = match result {
            Err(error) => error,
            Ok(_) => panic!("reopened ABA instance must reject delayed instruction analysis"),
        };

        assert_eq!(
            error,
            "the open project instance changed while the file dialog was open"
        );
        let project = lock_project(&state).expect("lock unchanged reopened project");
        assert_eq!(project_state_signature(&project), before);
    }

    #[test]
    fn semantic_instruction_failure_cannot_overwrite_an_existing_save() {
        let project = initial_project_state();
        let mut document = project.document();
        document.instruction_timeline.steps.push(InstructionStep {
            id: InstructionStepId::new(),
            title: "不正な現在姿勢".to_owned(),
            description: String::new(),
            caution: String::new(),
            duration_ms: 1_000,
            visual: Default::default(),
            pose: InstructionPose {
                model: InstructionPoseModel::AbsoluteHingeAnglesV1,
                source_model_fingerprint: project.editor.fold_model_fingerprint_v1(),
                fixed_face: Some(FaceId::new()),
                hinge_angles: Vec::new(),
            },
        });
        let directory = TestDirectory::new();
        let path = directory.join("existing.ori2");
        let original = b"existing project bytes";
        fs::write(&path, original).expect("create existing target");

        let error = persist_document(&path, &document)
            .expect_err("semantic validation must run before staging a save");

        assert_eq!(error, PROJECT_INSTRUCTIONS_SAVE_FAILED_MESSAGE);
        assert!(!error.contains("不正な現在姿勢"));
        assert_eq!(fs::read(&path).expect("read preserved target"), original);
        assert_eq!(
            fs::read_dir(&directory.path)
                .expect("inspect save directory")
                .count(),
            1,
            "semantic rejection must not leave a staged file"
        );
    }

    #[test]
    fn topology_bridge_preserves_three_faces_and_two_hinges_for_multiple_folds() {
        let project = cellular_multi_fold_project_state();
        let before = project_state_signature(&project);
        let input = capture_topology_input(&project, project.project_id, 0)
            .expect("capture cellular fold graph");

        let response = finish_topology_response(&project, &input, input.analyze())
            .expect("finish cellular fold topology");

        assert!(response.simulation_ready);
        assert!(response.issues.is_empty());
        let snapshot = response.snapshot.expect("cellular fold snapshot");
        assert_eq!(snapshot.source_revision, 0);
        assert_eq!(snapshot.faces.len(), 3);
        assert_eq!(snapshot.hinge_adjacency.len(), 2);
        assert_eq!(project_state_signature(&project), before);
    }

    #[test]
    fn topology_bridge_preserves_structured_unsupported_diagnostics() {
        let sheet = create_rectangular_sheet(100.0, 100.0, true).expect("cut-enabled sheet");
        let (pattern, paper) = sheet.into_parts();
        let mut project = ProjectState::new_with_paper(pattern, paper);
        let boundary = project.editor.paper().boundary_vertices.clone();
        let cut = EdgeId::new();
        let project_id = project.project_id;
        execute_command(
            &mut project,
            project_id,
            0,
            Command::AddEdge {
                id: cut,
                start: boundary[0],
                end: boundary[2],
                kind: EdgeKind::Cut,
            },
        )
        .expect("add supported editor cut");
        let input =
            capture_topology_input(&project, project_id, 1).expect("capture unsupported graph");

        let response = finish_topology_response(&project, &input, input.analyze())
            .expect("unsupported topology is a diagnostic response");

        assert!(!response.simulation_ready);
        assert!(response.snapshot.is_none());
        assert!(matches!(
            response.issues.as_slice(),
            [TopologyIssue {
                kind: ori_core::TopologyIssueKind::UnsupportedActiveEdge {
                    edge,
                    edge_kind: EdgeKind::Cut,
                },
                ..
            }] if *edge == cut
        ));
    }

    #[test]
    fn topology_bridge_rejects_stale_capture_and_delayed_aba_result() {
        let mut project = initial_project_state();
        let project_id = project.project_id;
        assert_eq!(
            capture_topology_input(&project, project_id, 1).expect_err("stale requested revision"),
            "expected revision 1, but the current revision is 0"
        );
        assert!(capture_topology_input(&project, ProjectId::new(), 0).is_err());

        let input = capture_topology_input(&project, project_id, 0).expect("capture old input");
        let topology = input.analyze();
        let replacement =
            create_rectangular_sheet(210.0, 297.0, false).expect("replacement rectangle");
        let (pattern, paper) = replacement.into_parts();
        project.editor = EditorState::with_paper(pattern, paper);
        assert_eq!(project.editor.revision(), 0, "ABA revision fixture");

        assert_eq!(
            finish_topology_response(&project, &input, topology)
                .expect_err("same identity/revision with different content is stale"),
            "the project changed while topology was being analyzed"
        );
    }

    #[test]
    fn validation_worker_releases_project_lock_during_exact_analysis() {
        let state = Arc::new(AppState::new(initial_project_state()));
        let worker_state = Arc::clone(&state);
        let (entered_tx, entered_rx) = mpsc::sync_channel(0);
        let (release_tx, release_rx) = mpsc::sync_channel(0);

        let validation = thread::spawn(move || {
            tauri::async_runtime::block_on(validate_project_with_worker(
                &worker_state,
                move |input| {
                    entered_tx.send(()).expect("announce worker entry");
                    release_rx.recv().expect("release validation worker");
                    Ok(analyze_validation_input(input))
                },
            ))
        });

        entered_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("validation worker must start");
        let lock_was_available = state.0.try_lock().is_ok();
        release_tx.send(()).expect("release validation worker");

        let snapshot = validation
            .join()
            .expect("validation caller thread must not panic")
            .expect("unchanged validation must finish");
        assert!(
            lock_was_available,
            "the project mutex must not be held during exact validation"
        );
        assert_eq!(snapshot.revision, 0);
    }

    #[test]
    fn validation_worker_rejects_same_revision_aba_content() {
        let state = Arc::new(AppState::new(initial_project_state()));
        let worker_state = Arc::clone(&state);
        let (entered_tx, entered_rx) = mpsc::sync_channel(0);
        let (release_tx, release_rx) = mpsc::sync_channel(0);

        let validation = thread::spawn(move || {
            tauri::async_runtime::block_on(validate_project_with_worker(
                &worker_state,
                move |input| {
                    entered_tx.send(()).expect("announce worker entry");
                    release_rx.recv().expect("release validation worker");
                    Ok(analyze_validation_input(input))
                },
            ))
        });

        entered_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("validation worker must start");
        {
            let Ok(mut project) = state.0.try_lock() else {
                release_tx
                    .send(())
                    .expect("release blocked validation worker");
                validation
                    .join()
                    .expect("validation caller thread must not panic")
                    .expect("unchanged validation must finish");
                panic!("the project mutex must be available while validation is running");
            };
            let replacement =
                create_rectangular_sheet(210.0, 297.0, false).expect("replacement rectangle");
            let (pattern, paper) = replacement.into_parts();
            project.editor = EditorState::with_paper(pattern, paper);
            assert_eq!(project.editor.revision(), 0, "ABA revision fixture");
        }
        release_tx.send(()).expect("release validation worker");

        let error = validation
            .join()
            .expect("validation caller thread must not panic")
            .expect_err("same-revision replacement must make the result stale");
        assert_eq!(
            error,
            "the project changed while validation was being analyzed"
        );
    }

    #[test]
    fn validation_worker_panic_and_reported_failure_are_redacted_and_fail_closed() {
        let state = AppState::new(initial_project_state());
        let private_panic = r"C:\Users\alice\秘密の作品.ori2 at vertex=(12.3,45.6)";

        let panic_error = tauri::async_runtime::block_on(validate_project_with_worker(
            &state,
            move |_| -> Result<AnalyzedProjectValidation, String> {
                panic!("{private_panic}");
            },
        ))
        .expect_err("a panicking worker must fail the command");
        assert_eq!(panic_error, VALIDATION_ANALYSIS_FAILED_MESSAGE);
        assert!(!panic_error.contains("alice"));
        assert!(!panic_error.contains("秘密の作品"));
        assert!(!panic_error.contains("12.3"));

        let private_failure = r"C:\Users\bob\非公開.ori2; internal_id=validation-7";
        let reported_error =
            tauri::async_runtime::block_on(validate_project_with_worker(&state, move |_| {
                Err(private_failure.to_owned())
            }))
            .expect_err("a reported worker failure must fail the command");
        assert_eq!(reported_error, VALIDATION_ANALYSIS_FAILED_MESSAGE);
        assert!(!reported_error.contains("bob"));
        assert!(!reported_error.contains("非公開"));
        assert!(!reported_error.contains("validation-7"));
        assert!(
            state.0.try_lock().is_ok(),
            "worker failures must not poison or retain the project mutex"
        );
    }

    #[test]
    fn background_task_failures_discard_private_panic_payloads() {
        let private_payload = r"C:\Users\alice\秘密の作品.ori2; face_id=private; point=(12.3,45.6)";
        let errors = [
            topology_analysis_task_error(private_payload),
            instruction_topology_analysis_task_error(private_payload),
            fold_import_task_error(private_payload),
            fold_conversion_task_error(private_payload),
        ];

        assert_eq!(
            errors,
            [
                TOPOLOGY_ANALYSIS_TASK_FAILED_MESSAGE,
                INSTRUCTION_TOPOLOGY_ANALYSIS_TASK_FAILED_MESSAGE,
                FOLD_IMPORT_TASK_FAILED_MESSAGE,
                FOLD_CONVERSION_TASK_FAILED_MESSAGE,
            ]
        );
        for error in errors {
            assert!(!error.contains("alice"));
            assert!(!error.contains("秘密の作品"));
            assert!(!error.contains("face_id"));
            assert!(!error.contains("12.3"));
        }
    }

    fn unsaved_project_with_redo_history(name: &str) -> ProjectState {
        let mut project =
            ProjectState::new_unsaved(name.to_owned(), CreasePattern::empty(), Paper::default());
        let project_id = project.project_id;
        execute_command(
            &mut project,
            project_id,
            0,
            Command::AddVertex {
                id: VertexId::new(),
                position: Point2::new(12.0, 34.0),
            },
        )
        .expect("add history fixture vertex");
        project.editor.undo(1).expect("create redo history");
        assert!(project.editor.can_redo());
        project
    }

    fn unsaved_project_with_undo_and_redo_history(
        name: &str,
    ) -> (ProjectState, VertexId, VertexId) {
        let mut project =
            ProjectState::new_unsaved(name.to_owned(), CreasePattern::empty(), Paper::default());
        project
            .editor
            .set_history_entry_limit(17)
            .expect("configure persisted history limit");
        let project_id = project.project_id;
        let first = VertexId::new();
        let second = VertexId::new();
        execute_command(
            &mut project,
            project_id,
            0,
            Command::AddVertex {
                id: first,
                position: Point2::new(12.0, 34.0),
            },
        )
        .expect("add first history fixture vertex");
        execute_command(
            &mut project,
            project_id,
            1,
            Command::AddVertex {
                id: second,
                position: Point2::new(56.0, 78.0),
            },
        )
        .expect("add second history fixture vertex");
        project
            .editor
            .undo(2)
            .expect("leave both Undo and Redo stacks populated");
        assert!(project.editor.can_undo());
        assert!(project.editor.can_redo());
        (project, first, second)
    }

    fn project_with_reachable_invalid_instruction_pose(name: &str) -> ProjectState {
        let sheet = create_rectangular_sheet(40.0, 40.0, false).expect("valid history test sheet");
        let (pattern, paper) = sheet.into_parts();
        let mut project = ProjectState::new_unsaved(name.to_owned(), pattern, paper);
        let project_id = project.project_id;
        let old_fingerprint = project.editor.fold_model_fingerprint_v1();
        execute_command(
            &mut project,
            project_id,
            0,
            Command::AddInstructionStep {
                step: InstructionStep {
                    id: InstructionStepId::new(),
                    title: "invalid only after Undo".to_owned(),
                    description: String::new(),
                    caution: String::new(),
                    duration_ms: 1_000,
                    visual: Default::default(),
                    pose: InstructionPose {
                        model: InstructionPoseModel::AbsoluteHingeAnglesV1,
                        source_model_fingerprint: old_fingerprint.clone(),
                        fixed_face: Some(FaceId::new()),
                        hinge_angles: Vec::new(),
                    },
                },
            },
        )
        .expect("the editor accepts structurally valid pose metadata");
        execute_command(
            &mut project,
            project_id,
            1,
            Command::AddVertex {
                id: VertexId::new(),
                position: Point2::new(20.0, 20.0),
            },
        )
        .expect("make the invalid instruction pose stale in the current document");
        assert_ne!(project.editor.fold_model_fingerprint_v1(), old_fingerprint);
        assert!(
            validate_document_instruction_poses(&project.document()).is_ok(),
            "the final stale pose is intentionally accepted"
        );
        let mut undo_endpoint = project.editor.clone();
        undo_endpoint.undo(2).expect("reach old model endpoint");
        let mut endpoint_document = project.document();
        endpoint_document.paper = undo_endpoint.paper().clone();
        endpoint_document.crease_pattern = undo_endpoint.pattern().clone();
        endpoint_document.instruction_timeline = undo_endpoint.instruction_timeline().clone();
        endpoint_document.geometric_constraints = undo_endpoint.geometric_constraints().clone();
        endpoint_document.layers = undo_endpoint.project_layers().clone();
        assert!(
            validate_document_instruction_poses(&endpoint_document).is_err(),
            "the same pose becomes current and invalid after Undo"
        );
        project
    }

    fn project_with_redo_reachable_invalid_instruction_pose(name: &str) -> ProjectState {
        let sheet = create_rectangular_sheet(40.0, 40.0, false).expect("valid history test sheet");
        let (pattern, paper) = sheet.into_parts();
        let mut project = ProjectState::new_unsaved(name.to_owned(), pattern, paper);
        let project_id = project.project_id;
        let fingerprint = project.editor.fold_model_fingerprint_v1();
        execute_command(
            &mut project,
            project_id,
            0,
            Command::AddInstructionStep {
                step: InstructionStep {
                    id: InstructionStepId::new(),
                    title: "invalid only after Redo".to_owned(),
                    description: String::new(),
                    caution: String::new(),
                    duration_ms: 1_000,
                    visual: Default::default(),
                    pose: InstructionPose {
                        model: InstructionPoseModel::AbsoluteHingeAnglesV1,
                        source_model_fingerprint: fingerprint,
                        fixed_face: Some(FaceId::new()),
                        hinge_angles: Vec::new(),
                    },
                },
            },
        )
        .expect("the editor accepts structurally valid pose metadata");
        project
            .editor
            .undo(1)
            .expect("leave the invalid step only on the Redo endpoint");
        assert!(project.editor.instruction_timeline().steps.is_empty());
        assert!(project.editor.can_redo());
        assert!(validate_document_instruction_poses(&project.document()).is_ok());
        project
    }

    fn corrupt_editor_history_payload(mut bytes: Vec<u8>) -> Vec<u8> {
        const LOCAL_FILE_HEADER_SIZE: usize = 30;
        const HISTORY_PATH: &[u8] = b"editor-history.json";
        let name_start = bytes
            .windows(HISTORY_PATH.len())
            .position(|window| window == HISTORY_PATH)
            .expect("history local-header name");
        let header_start = name_start
            .checked_sub(LOCAL_FILE_HEADER_SIZE)
            .expect("history local-header offset");
        assert_eq!(
            &bytes[header_start..header_start + 4],
            b"PK\x03\x04",
            "the first history path must belong to its local ZIP header"
        );
        let compressed_size = u32::from_le_bytes(
            bytes[header_start + 18..header_start + 22]
                .try_into()
                .expect("compressed-size field"),
        ) as usize;
        let extra_length = u16::from_le_bytes(
            bytes[header_start + 28..header_start + 30]
                .try_into()
                .expect("extra-length field"),
        ) as usize;
        assert!(compressed_size > 0);
        let payload_start = name_start + HISTORY_PATH.len() + extra_length;
        let corrupt_at = payload_start + compressed_size / 2;
        bytes[corrupt_at] ^= 0x01;
        bytes
    }

    fn file_document(name: &str, x: f64) -> ProjectDocument {
        ProjectDocument::new(
            name,
            CreasePattern {
                vertices: vec![Vertex {
                    id: VertexId::new(),
                    position: Point2::new(x, 5.0),
                }],
                edges: Vec::new(),
            },
        )
    }

    fn crossing_project() -> (ProjectState, Edge, Edge) {
        let sheet = create_rectangular_sheet(100.0, 100.0, true).expect("valid test sheet");
        let (mut pattern, paper) = sheet.into_parts();
        let ids = [
            VertexId::new(),
            VertexId::new(),
            VertexId::new(),
            VertexId::new(),
        ];
        pattern.vertices.extend([
            Vertex {
                id: ids[0],
                position: Point2::new(20.0, 20.0),
            },
            Vertex {
                id: ids[1],
                position: Point2::new(80.0, 80.0),
            },
            Vertex {
                id: ids[2],
                position: Point2::new(20.0, 80.0),
            },
            Vertex {
                id: ids[3],
                position: Point2::new(80.0, 20.0),
            },
        ]);
        let first = Edge {
            id: EdgeId::new(),
            start: ids[0],
            end: ids[1],
            kind: EdgeKind::Mountain,
        };
        let second = Edge {
            id: EdgeId::new(),
            start: ids[2],
            end: ids[3],
            kind: EdgeKind::Valley,
        };
        pattern.edges.extend([first.clone(), second.clone()]);
        (ProjectState::new_with_paper(pattern, paper), first, second)
    }

    fn t_junction_project() -> (ProjectState, Edge, Edge, VertexId) {
        let sheet = create_rectangular_sheet(100.0, 100.0, true).expect("valid test sheet");
        let (mut pattern, paper) = sheet.into_parts();
        let interior_start = VertexId::new();
        let interior_end = VertexId::new();
        let stem_other = VertexId::new();
        let junction = VertexId::new();
        pattern.vertices.extend([
            Vertex {
                id: interior_start,
                position: Point2::new(10.0, 40.0),
            },
            Vertex {
                id: interior_end,
                position: Point2::new(90.0, 40.0),
            },
            Vertex {
                id: stem_other,
                position: Point2::new(34.0, 10.0),
            },
            Vertex {
                id: junction,
                position: Point2::new(34.0, 40.0),
            },
        ]);
        let interior = Edge {
            id: EdgeId::new(),
            start: interior_start,
            end: interior_end,
            kind: EdgeKind::Mountain,
        };
        let stem = Edge {
            id: EdgeId::new(),
            start: stem_other,
            end: junction,
            kind: EdgeKind::Valley,
        };
        pattern.edges.extend([interior.clone(), stem.clone()]);
        (
            ProjectState::new_with_paper(pattern, paper),
            interior,
            stem,
            junction,
        )
    }

    fn boundary_t_junction_project() -> (ProjectState, Edge, Edge, VertexId) {
        let sheet = create_rectangular_sheet(100.0, 100.0, true).expect("valid test sheet");
        let (mut pattern, paper) = sheet.into_parts();
        let boundary = pattern.edges[0].clone();
        let junction = VertexId::new();
        let stem_other = VertexId::new();
        pattern.vertices.extend([
            Vertex {
                id: junction,
                position: Point2::new(40.0, 0.0),
            },
            Vertex {
                id: stem_other,
                position: Point2::new(40.0, 30.0),
            },
        ]);
        let stem = Edge {
            id: EdgeId::new(),
            start: stem_other,
            end: junction,
            kind: EdgeKind::Mountain,
        };
        pattern.edges.push(stem.clone());
        (
            ProjectState::new_with_paper(pattern, paper),
            boundary,
            stem,
            junction,
        )
    }

    fn append_cluster_test_edge(
        pattern: &mut CreasePattern,
        start_position: Point2,
        end_position: Point2,
        kind: EdgeKind,
    ) -> Edge {
        let start = VertexId::new();
        let end = VertexId::new();
        pattern.vertices.extend([
            Vertex {
                id: start,
                position: start_position,
            },
            Vertex {
                id: end,
                position: end_position,
            },
        ]);
        let edge = Edge {
            id: EdgeId::new(),
            start,
            end,
            kind,
        };
        pattern.edges.push(edge.clone());
        edge
    }

    fn create_cluster_project(include_omitted_edge: bool) -> (ProjectState, Vec<Edge>) {
        let sheet = create_rectangular_sheet(100.0, 100.0, true).expect("valid test sheet");
        let (mut pattern, paper) = sheet.into_parts();
        let mut edges = vec![
            append_cluster_test_edge(
                &mut pattern,
                Point2::new(10.0, 50.0),
                Point2::new(90.0, 50.0),
                EdgeKind::Mountain,
            ),
            append_cluster_test_edge(
                &mut pattern,
                Point2::new(50.0, 10.0),
                Point2::new(50.0, 90.0),
                EdgeKind::Valley,
            ),
            append_cluster_test_edge(
                &mut pattern,
                Point2::new(20.0, 20.0),
                Point2::new(80.0, 80.0),
                EdgeKind::Auxiliary,
            ),
        ];
        if include_omitted_edge {
            edges.push(append_cluster_test_edge(
                &mut pattern,
                Point2::new(20.0, 80.0),
                Point2::new(80.0, 20.0),
                EdgeKind::Mountain,
            ));
        }
        (ProjectState::new_with_paper(pattern, paper), edges)
    }

    fn maximum_cluster_project() -> (ProjectState, Vec<Edge>) {
        let sheet = create_rectangular_sheet(100.0, 100.0, true).expect("valid test sheet");
        let (mut pattern, paper) = sheet.into_parts();
        let mut edges = Vec::with_capacity(MAX_INTERSECTION_CLUSTER_TARGETS);
        for index in 0..MAX_INTERSECTION_CLUSTER_TARGETS {
            let offset = index as f64 - 32.0;
            let edge = append_cluster_test_edge(
                &mut pattern,
                Point2::new(10.0, 50.0 - offset),
                Point2::new(90.0, 50.0 + offset),
                match index % 4 {
                    0 => EdgeKind::Mountain,
                    1 => EdgeKind::Valley,
                    2 => EdgeKind::Auxiliary,
                    _ => EdgeKind::Cut,
                },
            );
            edges.push(edge);
        }
        (ProjectState::new_with_paper(pattern, paper), edges)
    }

    fn reuse_cluster_project() -> (ProjectState, [Edge; 3], VertexId) {
        let sheet = create_rectangular_sheet(100.0, 100.0, true).expect("valid test sheet");
        let (mut pattern, paper) = sheet.into_parts();
        let horizontal = append_cluster_test_edge(
            &mut pattern,
            Point2::new(10.0, 50.0),
            Point2::new(90.0, 50.0),
            EdgeKind::Mountain,
        );
        let vertical = append_cluster_test_edge(
            &mut pattern,
            Point2::new(50.0, 10.0),
            Point2::new(50.0, 90.0),
            EdgeKind::Valley,
        );
        let junction = VertexId::new();
        let stem_start = VertexId::new();
        pattern.vertices.extend([
            Vertex {
                id: stem_start,
                position: Point2::new(20.0, 20.0),
            },
            Vertex {
                id: junction,
                position: Point2::new(50.0, 50.0),
            },
        ]);
        let stem = Edge {
            id: EdgeId::new(),
            start: stem_start,
            end: junction,
            kind: EdgeKind::Auxiliary,
        };
        pattern.edges.push(stem.clone());
        (
            ProjectState::new_with_paper(pattern, paper),
            [horizontal, vertical, stem],
            junction,
        )
    }

    #[test]
    fn benchmark_pattern_response_contains_stable_renderable_geometry() {
        let response = generate_benchmark_pattern(4);

        assert_eq!(response.requested_edge_count, 4);
        assert_eq!(response.vertex_count, 4);
        assert_eq!(response.edge_count, 4);
        assert_eq!(
            response.vertices,
            vec![
                BenchmarkVertex {
                    id: "benchmark-v-0".to_owned(),
                    position: Point2::new(0.0, 0.0),
                },
                BenchmarkVertex {
                    id: "benchmark-v-1".to_owned(),
                    position: Point2::new(1.0, 0.0),
                },
                BenchmarkVertex {
                    id: "benchmark-v-2".to_owned(),
                    position: Point2::new(0.0, 1.0),
                },
                BenchmarkVertex {
                    id: "benchmark-v-3".to_owned(),
                    position: Point2::new(1.0, 1.0),
                },
            ]
        );
        assert_eq!(
            response.edges,
            vec![
                BenchmarkEdge {
                    id: "benchmark-e-0".to_owned(),
                    start: "benchmark-v-0".to_owned(),
                    end: "benchmark-v-1".to_owned(),
                    kind: EdgeKind::Mountain,
                },
                BenchmarkEdge {
                    id: "benchmark-e-1".to_owned(),
                    start: "benchmark-v-0".to_owned(),
                    end: "benchmark-v-2".to_owned(),
                    kind: EdgeKind::Valley,
                },
                BenchmarkEdge {
                    id: "benchmark-e-2".to_owned(),
                    start: "benchmark-v-1".to_owned(),
                    end: "benchmark-v-3".to_owned(),
                    kind: EdgeKind::Mountain,
                },
                BenchmarkEdge {
                    id: "benchmark-e-3".to_owned(),
                    start: "benchmark-v-2".to_owned(),
                    end: "benchmark-v-3".to_owned(),
                    kind: EdgeKind::Valley,
                },
            ]
        );
        assert_eq!(generate_benchmark_pattern(4), response);
    }

    #[test]
    fn benchmark_pattern_response_has_all_ten_thousand_edges_and_valid_references() {
        let response = generate_benchmark_pattern(10_000);

        assert_eq!(response.requested_edge_count, 10_000);
        assert_eq!(response.vertex_count, 5_184);
        assert_eq!(response.edge_count, 10_000);
        let vertex_ids = response
            .vertices
            .iter()
            .map(|vertex| vertex.id.as_str())
            .collect::<std::collections::HashSet<_>>();
        assert!(response.edges.iter().all(|edge| {
            vertex_ids.contains(edge.start.as_str()) && vertex_ids.contains(edge.end.as_str())
        }));
    }

    #[test]
    fn benchmark_pattern_response_is_empty_for_zero_edges() {
        let response = generate_benchmark_pattern(0);

        assert_eq!(response.requested_edge_count, 0);
        assert_eq!(response.vertex_count, 0);
        assert_eq!(response.edge_count, 0);
        assert!(response.vertices.is_empty());
        assert!(response.edges.is_empty());
    }

    #[test]
    fn project_name_is_trimmed_and_validated_by_unicode_character_count() {
        assert_eq!(normalize_project_name("  Crane  "), Ok("Crane".to_owned()));
        assert_eq!(
            normalize_project_name("\n  Crane  \t"),
            Ok("Crane".to_owned())
        );
        assert!(normalize_project_name("").is_err());
        assert!(normalize_project_name(" \t\n ").is_err());
        assert!(normalize_project_name("Crane\0draft").is_err());

        let maximum = "鶴".repeat(MAX_PROJECT_NAME_CHARS);
        assert_eq!(normalize_project_name(&maximum), Ok(maximum.clone()));
        assert!(normalize_project_name(&format!("{maximum}鶴")).is_err());
    }

    #[test]
    fn paper_thickness_accepts_zero_and_rejects_negative_or_non_finite_values() {
        assert_eq!(validate_paper_thickness(0.0), Ok(()));
        assert_eq!(validate_paper_thickness(-0.0), Ok(()));
        for invalid in [-f64::MIN_POSITIVE, -1.0, f64::NAN, f64::INFINITY] {
            assert!(validate_paper_thickness(invalid).is_err());
        }
    }

    #[test]
    fn new_project_state_has_requested_paper_and_no_saved_baseline() {
        let parameters = new_project_parameters();
        let expected_front = parameters.front_color;
        let expected_back = parameters.back_color;

        let project = create_new_project_state(parameters).expect("valid new project");
        let response = snapshot(&project);

        assert_eq!(project.name, "Test sheet");
        assert!(project.current_path.is_none());
        assert!(project.saved_revision.is_none());
        assert!(project.saved_document.is_none());
        assert_eq!(project.editor.revision(), 0);
        assert!(!project.editor.can_undo());
        assert!(!project.editor.can_redo());
        assert!(project.editor.cutting_allowed());
        assert!(project.is_dirty());
        assert_eq!(project.editor.paper().thickness_mm, 0.2);
        assert_eq!(project.editor.paper().front.color, expected_front);
        assert_eq!(project.editor.paper().back.color, expected_back);
        assert_eq!(project.editor.paper().front.texture_asset, None);
        assert_eq!(project.editor.paper().back.texture_asset, None);
        let creation_expressions = project
            .numeric_expressions
            .rectangular_paper_creation
            .as_ref()
            .expect("new project keeps both creation expressions");
        assert_eq!(creation_expressions.schema_version, 1);
        assert_eq!(creation_expressions.width_source, "210");
        assert_eq!(creation_expressions.height_source, "297");
        assert_eq!(creation_expressions.adopted_width_mm, 210.0);
        assert_eq!(creation_expressions.adopted_height_mm, 297.0);
        assert_eq!(
            response.numeric_expressions, project.numeric_expressions,
            "snapshot and persisted document share the same bounded metadata"
        );
        assert_eq!(
            project.document().numeric_expressions,
            project.numeric_expressions
        );
        assert_eq!(
            project.editor.pattern().vertices[2].position,
            Point2::new(210.0, 297.0)
        );
        assert!(validate_paper(project.editor.paper(), project.editor.pattern()).is_valid());

        assert_eq!(response.project_id, project.project_id);
        assert_eq!(response.name, "Test sheet");
        assert!(response.current_path.is_none());
        assert_eq!(response.revision, 0);
        assert!(response.saved_revision.is_none());
        assert!(response.is_dirty);
        assert_eq!(&response.paper, project.editor.paper());
        assert!(response.cutting_allowed);
        assert!(!response.can_undo);
        assert!(!response.can_redo);
    }

    #[test]
    fn loaded_numeric_expressions_are_re_evaluated_against_saved_adopted_values() {
        assert_eq!(
            map_loaded_numeric_expression_error(PositiveMillimetrePairError::WorkerBusy),
            PROJECT_NUMERIC_EXPRESSIONS_BUSY_MESSAGE
        );
        let project =
            create_new_project_state(new_project_parameters()).expect("valid new project");
        let document = project.document();
        validate_loaded_numeric_expression_bindings(&document)
            .expect("untampered expressions remain loadable");

        let mut changed_source = document.clone();
        changed_source
            .numeric_expressions
            .rectangular_paper_creation
            .as_mut()
            .expect("creation expressions")
            .width_source = "211".to_owned();
        assert_eq!(
            validate_loaded_numeric_expression_bindings(&changed_source),
            Err(PROJECT_NUMERIC_EXPRESSIONS_INVALID_MESSAGE.to_owned())
        );

        let mut changed_value = document.clone();
        changed_value
            .numeric_expressions
            .rectangular_paper_creation
            .as_mut()
            .expect("creation expressions")
            .adopted_height_mm = 298.0;
        assert_eq!(
            validate_loaded_numeric_expression_bindings(&changed_value),
            Err(PROJECT_NUMERIC_EXPRESSIONS_INVALID_MESSAGE.to_owned())
        );

        let mut legacy = document;
        legacy.numeric_expressions = ProjectNumericExpressions::default();
        validate_loaded_numeric_expression_bindings(&legacy)
            .expect("legacy projects without expressions migrate safely");
    }

    #[test]
    fn vertex_coordinate_expressions_follow_native_history_and_archive_round_trip() {
        let mut project = initial_project_state();
        let project_id = project.project_id;
        let vertex = VertexId::new();
        execute_command(
            &mut project,
            project_id,
            0,
            Command::AddVertex {
                id: vertex,
                position: Point2::new(0.5, -2.0),
            },
        )
        .expect("add expression-backed vertex");
        project.adopt_vertex_coordinate_expression(VertexCoordinateExpressions::new(
            vertex, "1 / 2", "-sqrt(4)", 0.5, -2.0,
        ));
        let binding = project.numeric_expressions.vertex_coordinates[0].clone();
        assert_eq!(binding.x_source, "1 / 2");
        assert_eq!(binding.y_source, "-sqrt(4)");
        validate_loaded_numeric_expression_bindings(
            &project
                .project_archive()
                .expect("serialize expression history")
                .document,
        )
        .expect("re-evaluate every persisted expression");

        execute_undo(&mut project, project_id, 1).expect("undo vertex");
        assert!(project.numeric_expressions.vertex_coordinates.is_empty());
        execute_redo(&mut project, project_id, 2).expect("redo vertex");
        assert_eq!(
            project.numeric_expressions.vertex_coordinates,
            vec![binding]
        );
    }

    #[test]
    fn creation_expressions_follow_document_dirty_state_without_entering_editor_undo_history() {
        let mut project =
            create_new_project_state(new_project_parameters()).expect("valid new project");
        let project_id = project.project_id;
        let saved_document = project.document();
        let saved_expressions = project.numeric_expressions.clone();
        project.saved_document = Some(saved_document.clone());
        project.saved_revision = Some(project.editor.revision());
        assert!(!project.is_dirty());

        let resized = execute_command(
            &mut project,
            project_id,
            0,
            Command::ResizeRectangularPaper {
                width_mm: 420.0,
                height_mm: 594.0,
            },
        )
        .expect("resize paper");
        assert!(resized.is_dirty);
        assert_eq!(
            project.numeric_expressions.rectangular_paper_creation,
            saved_expressions.rectangular_paper_creation
        );

        project.editor.undo(1).expect("undo resize");
        assert_eq!(project.document(), saved_document);
        assert_eq!(
            project.numeric_expressions.rectangular_paper_creation,
            saved_expressions.rectangular_paper_creation
        );
        assert!(!project.is_dirty());

        project
            .numeric_expressions
            .rectangular_paper_creation
            .as_mut()
            .expect("creation expressions")
            .width_source = "210 + 0".to_owned();
        assert!(project.is_dirty());
    }

    #[test]
    fn snapshot_paper_uses_the_current_editor_cutting_setting() {
        let mut project = initial_project_state();
        let project_id = project.project_id;
        assert!(!project.editor.paper().cutting_allowed);

        let response = execute_command(
            &mut project,
            project_id,
            0,
            Command::SetCuttingAllowed { allowed: true },
        )
        .expect("enable cutting");

        assert!(response.cutting_allowed);
        assert!(response.paper.cutting_allowed);
        assert!(project.document().paper.cutting_allowed);
    }

    #[test]
    fn paper_properties_follow_undo_redo_dirty_save_and_validation() {
        let mut project = initial_project_state();
        let project_id = project.project_id;
        let original = project.editor.paper().clone();
        let front_color = RgbaColor::opaque(15, 35, 55);
        let back_color = RgbaColor::opaque(205, 185, 165);

        let response = execute_command(
            &mut project,
            project_id,
            0,
            Command::UpdatePaperProperties {
                thickness_mm: 0.0,
                front_color,
                back_color,
                front_texture_asset: None,
                back_texture_asset: None,
                cutting_allowed: true,
            },
        )
        .expect("update paper properties");

        assert_eq!(response.revision, 1);
        assert!(response.is_dirty);
        assert_eq!(response.paper.thickness_mm, 0.0);
        assert_eq!(response.paper.front.color, front_color);
        assert_eq!(response.paper.back.color, back_color);
        assert!(response.paper.cutting_allowed);
        assert!(validation_snapshot(&project).is_valid);

        project.editor.undo(1).expect("undo properties");
        assert_eq!(project.editor.paper(), &original);
        assert!(!project.is_dirty());

        project.editor.redo(2).expect("redo properties");
        assert!(project.is_dirty());
        let saved_document = project.document();
        project.saved_revision = Some(project.editor.revision());
        project.saved_document = Some(saved_document.clone());
        assert!(!project.is_dirty());
        assert_eq!(project.document(), saved_document);

        project.editor.undo(3).expect("undo after save");
        assert!(project.is_dirty());
        project.editor.redo(4).expect("redo to saved content");
        assert!(!project.is_dirty());
    }

    #[test]
    fn imported_front_textures_remain_live_across_undo_redo() {
        let mut project = initial_project_state();
        let instance_id = project.instance_id;
        let project_id = project.project_id;
        let png = |tag| {
            let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
            bytes.push(tag);
            bytes
        };

        register_front_texture(
            &mut project,
            instance_id,
            project_id,
            0,
            ProjectTextureMediaTypeV1::Png,
            png(1),
        )
        .expect("first texture");
        let first = project.editor.paper().front.texture_asset.unwrap();
        register_front_texture(
            &mut project,
            instance_id,
            project_id,
            1,
            ProjectTextureMediaTypeV1::Png,
            png(2),
        )
        .expect("replacement texture");
        let second = project.editor.paper().front.texture_asset.unwrap();
        assert_ne!(first, second);
        assert_eq!(project.texture_assets.len(), 2);

        project.editor.undo(2).expect("undo texture replacement");
        assert_eq!(project.editor.paper().front.texture_asset, Some(first));
        ori_formats::write_project_json(&project.document()).expect("undo document");
        project.editor.redo(3).expect("redo texture replacement");
        assert_eq!(project.editor.paper().front.texture_asset, Some(second));
        ori_formats::write_project_json(&project.document()).expect("redo document");
    }

    #[test]
    fn imported_back_textures_remain_live_across_undo_redo() {
        let mut project = initial_project_state();
        let instance_id = project.instance_id;
        let project_id = project.project_id;
        let png = |tag| {
            let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
            bytes.push(tag);
            bytes
        };
        register_back_texture(
            &mut project, instance_id, project_id, 0,
            ProjectTextureMediaTypeV1::Png, png(1),
        ).expect("first back texture");
        let first = project.editor.paper().back.texture_asset.unwrap();
        register_back_texture(
            &mut project, instance_id, project_id, 1,
            ProjectTextureMediaTypeV1::Png, png(2),
        ).expect("replacement back texture");
        let second = project.editor.paper().back.texture_asset.unwrap();
        assert_ne!(first, second);
        project.editor.undo(2).expect("undo back texture");
        assert_eq!(project.editor.paper().back.texture_asset, Some(first));
        ori_formats::write_project_json(&project.document()).expect("undo document");
        project.editor.redo(3).expect("redo back texture");
        assert_eq!(project.editor.paper().back.texture_asset, Some(second));
        ori_formats::write_project_json(&project.document()).expect("redo document");
    }

    #[test]
    fn length_display_unit_follows_snapshot_dirty_history_and_fingerprint_contracts() {
        let mut project = initial_project_state();
        let project_id = project.project_id;
        let original_document = project.document();
        let fingerprint = project.editor.fold_model_fingerprint_v1();
        let reference_edge = project.editor.pattern().edges[0].id;

        let response = execute_command(
            &mut project,
            project_id,
            0,
            Command::SetLengthDisplayUnit {
                unit: LengthDisplayUnit::PaperEdgeRatio { reference_edge },
            },
        )
        .expect("set native length display unit");

        assert_eq!(response.revision, 1);
        assert!(response.is_dirty);
        assert!(response.can_undo);
        assert!(!response.can_redo);
        assert_eq!(
            response.paper.length_display_unit,
            LengthDisplayUnit::PaperEdgeRatio { reference_edge }
        );
        assert_eq!(response.fold_model_fingerprint, fingerprint);
        assert_eq!(project.editor.fold_model_fingerprint_v1(), fingerprint);
        assert_eq!(
            project.document().paper.length_display_unit,
            LengthDisplayUnit::PaperEdgeRatio { reference_edge }
        );

        project.editor.undo(1).expect("undo display unit");
        assert_eq!(project.document(), original_document);
        assert!(!project.is_dirty());
        assert_eq!(project.editor.fold_model_fingerprint_v1(), fingerprint);

        project.editor.redo(2).expect("redo display unit");
        assert!(project.is_dirty());
        assert_eq!(
            project.editor.paper().length_display_unit,
            LengthDisplayUnit::PaperEdgeRatio { reference_edge }
        );
        assert_eq!(project.editor.fold_model_fingerprint_v1(), fingerprint);
    }

    #[test]
    fn invalid_paper_property_command_preserves_project_state() {
        let mut project = initial_project_state();
        let project_id = project.project_id;
        let before = project_state_signature(&project);

        let conflict = execute_command(
            &mut project,
            project_id,
            1,
            Command::UpdatePaperProperties {
                thickness_mm: 0.3,
                front_color: RgbaColor::opaque(1, 2, 3),
                back_color: RgbaColor::opaque(4, 5, 6),
                front_texture_asset: None,
                back_texture_asset: None,
                cutting_allowed: true,
            },
        )
        .expect_err("stale property update must fail");
        assert_eq!(
            conflict,
            "expected revision 1, but the current revision is 0"
        );
        assert_eq!(project_state_signature(&project), before);

        let error = execute_command(
            &mut project,
            project_id,
            0,
            Command::UpdatePaperProperties {
                thickness_mm: f64::NAN,
                front_color: RgbaColor::opaque(1, 2, 3),
                back_color: RgbaColor::opaque(4, 5, 6),
                front_texture_asset: None,
                back_texture_asset: None,
                cutting_allowed: true,
            },
        )
        .expect_err("invalid thickness must fail");

        assert_eq!(error, "paper thickness must be finite");
        assert_eq!(project_state_signature(&project), before);
    }

    #[test]
    fn rectangular_resize_updates_document_dirty_state_and_undo_redo() {
        let mut project = initial_project_state();
        let project_id = project.project_id;
        let original_document = project.document();
        let original_vertex_ids = project
            .editor
            .pattern()
            .vertices
            .iter()
            .map(|vertex| vertex.id)
            .collect::<Vec<_>>();
        let original_edges = project.editor.pattern().edges.clone();
        let original_paper = project.editor.paper().clone();

        let response = execute_command(
            &mut project,
            project_id,
            0,
            Command::ResizeRectangularPaper {
                width_mm: 210.0,
                height_mm: 297.0,
            },
        )
        .expect("resize paper");

        assert_eq!(response.revision, 1);
        assert!(response.is_dirty);
        assert!(response.can_undo);
        assert!(!response.can_redo);
        assert_eq!(response.paper, original_paper);
        assert_eq!(
            response
                .crease_pattern
                .vertices
                .iter()
                .map(|vertex| vertex.id)
                .collect::<Vec<_>>(),
            original_vertex_ids
        );
        assert_eq!(response.crease_pattern.edges, original_edges);
        assert!(
            response
                .crease_pattern
                .vertices
                .iter()
                .any(|vertex| vertex.position == Point2::new(210.0, 297.0))
        );
        assert!(validation_snapshot(&project).is_valid);
        let resized_document = project.document();
        assert_ne!(resized_document, original_document);
        assert_eq!(resized_document.paper, original_paper);

        project.editor.undo(1).expect("undo resize");
        assert_eq!(project.editor.revision(), 2);
        assert_eq!(project.document(), original_document);
        assert!(!project.is_dirty());

        project.editor.redo(2).expect("redo resize");
        assert_eq!(project.editor.revision(), 3);
        assert_eq!(project.document(), resized_document);
        assert!(project.is_dirty());
    }

    #[test]
    fn same_size_resize_has_history_without_making_the_document_dirty() {
        let mut project = initial_project_state();
        let project_id = project.project_id;
        let original_document = project.document();

        let response = execute_command(
            &mut project,
            project_id,
            0,
            Command::ResizeRectangularPaper {
                width_mm: DEFAULT_SHEET_SIZE_MM,
                height_mm: DEFAULT_SHEET_SIZE_MM,
            },
        )
        .expect("same-size resize");

        assert_eq!(response.revision, 1);
        assert!(response.can_undo);
        assert!(!response.is_dirty);
        assert_eq!(project.document(), original_document);
    }

    #[test]
    fn resize_conflicts_invalid_dimensions_and_overflow_preserve_project_state() {
        let mut project = initial_project_state();
        let project_id = project.project_id;
        let before = project_state_signature(&project);

        let conflict = execute_command(
            &mut project,
            project_id,
            1,
            Command::ResizeRectangularPaper {
                width_mm: 210.0,
                height_mm: 297.0,
            },
        )
        .expect_err("stale resize must fail");
        assert_eq!(
            conflict,
            "expected revision 1, but the current revision is 0"
        );
        assert_eq!(project_state_signature(&project), before);

        let invalid = execute_command(
            &mut project,
            project_id,
            0,
            Command::ResizeRectangularPaper {
                width_mm: 0.0,
                height_mm: 297.0,
            },
        )
        .expect_err("zero width must fail");
        assert_eq!(invalid, "paper width must be greater than zero");
        assert_eq!(project_state_signature(&project), before);

        let overflow = execute_command(
            &mut project,
            project_id,
            0,
            Command::ResizeRectangularPaper {
                width_mm: f64::MAX,
                height_mm: 2.0,
            },
        )
        .expect_err("unrepresentable area must fail");
        assert_eq!(
            overflow,
            "target paper area is too large to represent safely"
        );
        assert_eq!(project_state_signature(&project), before);
    }

    #[test]
    fn generated_id_edge_split_updates_snapshot_document_and_history() {
        let sheet = create_rectangular_sheet(100.0, 80.0, false).expect("valid rectangle");
        let (mut pattern, paper) = sheet.into_parts();
        let crease = Edge {
            id: EdgeId::new(),
            start: paper.boundary_vertices[0],
            end: paper.boundary_vertices[2],
            kind: EdgeKind::Valley,
        };
        pattern.edges.push(crease.clone());
        let original_vertex_ids = pattern
            .vertices
            .iter()
            .map(|vertex| vertex.id)
            .collect::<Vec<_>>();
        let original_edge_ids = pattern.edges.iter().map(|edge| edge.id).collect::<Vec<_>>();
        let original_edge_index = pattern.edges.len() - 1;
        let mut project = ProjectState::new_with_paper(pattern, paper);
        let project_id = project.project_id;
        let original_document = project.document();

        let response = execute_edge_split(&mut project, project_id, 0, crease.id, 0.5)
            .expect("split crease edge");

        assert_eq!(response.revision, 1);
        assert!(response.is_dirty);
        assert!(response.can_undo);
        assert!(!response.can_redo);
        assert_eq!(response.paper, original_document.paper);
        assert_eq!(response.crease_pattern.vertices.len(), 5);
        let generated_vertices = response
            .crease_pattern
            .vertices
            .iter()
            .filter(|vertex| !original_vertex_ids.contains(&vertex.id))
            .collect::<Vec<_>>();
        assert_eq!(generated_vertices.len(), 1);
        let generated_vertex = generated_vertices[0];
        assert_eq!(generated_vertex.position, Point2::new(50.0, 40.0));
        assert_eq!(response.crease_pattern.edges.len(), 6);
        assert_eq!(
            response.crease_pattern.edges[original_edge_index],
            Edge {
                end: generated_vertex.id,
                ..crease.clone()
            }
        );
        let generated_edge = &response.crease_pattern.edges[original_edge_index + 1];
        assert!(!original_edge_ids.contains(&generated_edge.id));
        assert_eq!(generated_edge.start, generated_vertex.id);
        assert_eq!(generated_edge.end, crease.end);
        assert_eq!(generated_edge.kind, EdgeKind::Valley);
        assert!(validation_snapshot(&project).is_valid);
        let split_document = project.document();
        assert_ne!(split_document, original_document);

        project.editor.undo(1).expect("undo edge split");
        assert_eq!(project.editor.revision(), 2);
        assert_eq!(project.document(), original_document);
        assert!(!project.is_dirty());

        project.editor.redo(2).expect("redo edge split");
        assert_eq!(project.editor.revision(), 3);
        assert_eq!(project.document(), split_document);
        assert!(project.is_dirty());
        assert!(validation_snapshot(&project).is_valid);
    }

    #[test]
    fn edge_split_conflicts_invalid_fractions_and_boundary_targets_preserve_project_state() {
        let sheet = create_rectangular_sheet(100.0, 80.0, false).expect("valid rectangle");
        let (mut pattern, paper) = sheet.into_parts();
        let boundary_edge = pattern.edges[0].id;
        let crease = Edge {
            id: EdgeId::new(),
            start: paper.boundary_vertices[0],
            end: paper.boundary_vertices[2],
            kind: EdgeKind::Mountain,
        };
        pattern.edges.push(crease.clone());
        let mut project = ProjectState::new_with_paper(pattern, paper);
        let project_id = project.project_id;
        let before = project_state_signature(&project);

        let conflict = execute_edge_split(&mut project, project_id, 1, crease.id, 0.5)
            .expect_err("stale split must fail");
        assert_eq!(
            conflict,
            "expected revision 1, but the current revision is 0"
        );
        assert_eq!(project_state_signature(&project), before);

        let invalid = execute_edge_split(&mut project, project_id, 0, crease.id, f64::NAN)
            .expect_err("non-finite split must fail");
        assert_eq!(invalid, "edge split fraction must be finite");
        assert_eq!(project_state_signature(&project), before);

        let boundary = execute_edge_split(&mut project, project_id, 0, boundary_edge, 0.5)
            .expect_err("boundary split must use the sheet command");
        assert!(boundary.contains("must be changed through a sheet-boundary operation"));
        assert_eq!(project_state_signature(&project), before);
    }

    #[test]
    fn edge_intersection_connection_returns_vertex_and_exact_undoable_snapshot() {
        let (mut project, first, second) = crossing_project();
        let project_id = project.project_id;
        let original_document = project.document();
        let original_vertex_ids = original_document
            .crease_pattern
            .vertices
            .iter()
            .map(|vertex| vertex.id)
            .collect::<Vec<_>>();
        let original_edge_ids = original_document
            .crease_pattern
            .edges
            .iter()
            .map(|edge| edge.id)
            .collect::<Vec<_>>();

        let response =
            execute_edge_intersection_connection(&mut project, project_id, 0, second.id, first.id)
                .expect("connect crossing edges");

        assert_eq!(response.snapshot.revision, 1);
        assert!(response.snapshot.is_dirty);
        assert!(response.snapshot.can_undo);
        assert!(!response.snapshot.can_redo);
        let created_vertex = response
            .snapshot
            .crease_pattern
            .vertices
            .iter()
            .find(|vertex| vertex.id == response.vertex_id)
            .expect("explicitly returned generated vertex");
        assert_eq!(created_vertex.position, Point2::new(50.0, 50.0));
        assert!(!original_vertex_ids.contains(&response.vertex_id));
        let generated_edges = response
            .snapshot
            .crease_pattern
            .edges
            .iter()
            .filter(|edge| !original_edge_ids.contains(&edge.id))
            .collect::<Vec<_>>();
        assert_eq!(generated_edges.len(), 2);
        assert!(
            generated_edges
                .iter()
                .all(|edge| edge.start == response.vertex_id)
        );
        assert_eq!(
            generated_edges
                .iter()
                .map(|edge| edge.kind)
                .collect::<Vec<_>>(),
            vec![EdgeKind::Mountain, EdgeKind::Valley]
        );
        assert_eq!(
            response.snapshot.crease_pattern,
            project.editor.pattern().clone()
        );
        assert!(validation_snapshot(&project).is_valid);
        let connected_document = project.document();

        project
            .editor
            .undo(1)
            .expect("undo intersection connection");
        assert_eq!(project.editor.revision(), 2);
        assert_eq!(project.document(), original_document);
        assert!(!project.is_dirty());

        project
            .editor
            .redo(2)
            .expect("redo intersection connection");
        assert_eq!(project.editor.revision(), 3);
        assert_eq!(project.document(), connected_document);
        assert!(project.is_dirty());
        assert!(validation_snapshot(&project).is_valid);
    }

    #[test]
    fn edge_intersection_api_rejections_preserve_entire_project_state() {
        let (mut project, first, second) = crossing_project();
        let project_id = project.project_id;
        let before = project_state_signature(&project);

        let wrong_project = execute_edge_intersection_connection(
            &mut project,
            ProjectId::new(),
            0,
            first.id,
            second.id,
        )
        .expect_err("wrong project must fail");
        assert!(wrong_project.contains("active project changed"));
        assert_eq!(project_state_signature(&project), before);

        let stale =
            execute_edge_intersection_connection(&mut project, project_id, 4, first.id, second.id)
                .expect_err("stale revision must fail");
        assert_eq!(stale, "expected revision 4, but the current revision is 0");
        assert_eq!(project_state_signature(&project), before);

        let same_edge =
            execute_edge_intersection_connection(&mut project, project_id, 0, first.id, first.id)
                .expect_err("same target edge must fail");
        assert_eq!(same_edge, "the two intersection edge IDs must be different");
        assert_eq!(project_state_signature(&project), before);

        let boundary = project.editor.pattern().edges[0].id;
        let boundary_error =
            execute_edge_intersection_connection(&mut project, project_id, 0, boundary, first.id)
                .expect_err("boundary target must fail");
        assert!(boundary_error.contains("must not be a boundary edge"));
        assert_eq!(project_state_signature(&project), before);
    }

    #[test]
    fn edge_intersection_api_rejects_t_junction_without_mutation() {
        let (project, first, second) = crossing_project();
        let mut document = project.document();
        document
            .crease_pattern
            .vertices
            .iter_mut()
            .find(|vertex| vertex.id == second.start)
            .expect("second start")
            .position = Point2::new(50.0, 50.0);
        let mut project = ProjectState::new_with_paper(document.crease_pattern, document.paper);
        let project_id = project.project_id;
        let before = project_state_signature(&project);

        let error =
            execute_edge_intersection_connection(&mut project, project_id, 0, first.id, second.id)
                .expect_err("T-junction must fail");

        assert_eq!(
            error,
            "the selected edges must intersect strictly inside both edges"
        );
        assert_eq!(project_state_signature(&project), before);
    }

    #[test]
    fn intersection_cluster_api_creates_three_way_junction_with_one_step_history() {
        let (mut project, edges) = create_cluster_project(false);
        let project_id = project.project_id;
        let original_document = project.document();
        let original_vertex_ids = original_document
            .crease_pattern
            .vertices
            .iter()
            .map(|vertex| vertex.id)
            .collect::<Vec<_>>();
        let original_edge_ids = original_document
            .crease_pattern
            .edges
            .iter()
            .map(|edge| edge.id)
            .collect::<Vec<_>>();
        let targets = edges
            .iter()
            .map(|edge| IntersectionClusterTargetRequest {
                edge_id: edge.id,
                relation: IntersectionClusterRelation::Interior,
            })
            .collect();

        let response =
            execute_intersection_cluster_connection(&mut project, project_id, 0, targets, None)
                .expect("connect a newly created three-edge intersection cluster");

        assert_eq!(response.snapshot.revision, 1);
        assert!(response.snapshot.is_dirty);
        assert!(response.snapshot.can_undo);
        assert!(!response.snapshot.can_redo);
        assert_eq!(response.snapshot.paper, original_document.paper);
        assert!(!original_vertex_ids.contains(&response.vertex_id));
        assert_eq!(
            response
                .snapshot
                .crease_pattern
                .vertices
                .iter()
                .find(|vertex| vertex.id == response.vertex_id)
                .expect("created cluster junction")
                .position,
            Point2::new(50.0, 50.0)
        );
        assert_eq!(
            response.snapshot.crease_pattern.vertices.len(),
            original_document.crease_pattern.vertices.len() + 1
        );
        assert_eq!(
            response.snapshot.crease_pattern.edges.len(),
            original_document.crease_pattern.edges.len() + edges.len()
        );
        for edge in &edges {
            let split_original = response
                .snapshot
                .crease_pattern
                .edges
                .iter()
                .find(|candidate| candidate.id == edge.id)
                .expect("split original cluster edge");
            assert_eq!(split_original.start, edge.start);
            assert_eq!(split_original.end, response.vertex_id);
            assert_eq!(split_original.kind, edge.kind);
            let generated = response
                .snapshot
                .crease_pattern
                .edges
                .iter()
                .find(|candidate| {
                    !original_edge_ids.contains(&candidate.id)
                        && candidate.start == response.vertex_id
                        && candidate.end == edge.end
                })
                .expect("generated cluster edge");
            assert_eq!(generated.kind, edge.kind);
        }
        assert!(validation_snapshot(&project).is_valid);
        let connected_document = project.document();

        project
            .editor
            .undo(1)
            .expect("undo created intersection cluster");
        assert_eq!(project.editor.revision(), 2);
        assert_eq!(project.document(), original_document);
        assert!(!project.is_dirty());

        project
            .editor
            .redo(2)
            .expect("redo created intersection cluster");
        assert_eq!(project.editor.revision(), 3);
        assert_eq!(project.document(), connected_document);
        assert!(project.is_dirty());
        assert!(validation_snapshot(&project).is_valid);
    }

    #[test]
    fn intersection_cluster_api_accepts_64_targets_and_returns_the_created_junction() {
        let (mut project, edges) = maximum_cluster_project();
        assert_eq!(edges.len(), MAX_INTERSECTION_CLUSTER_TARGETS);
        let project_id = project.project_id;
        let original_document = project.document();
        let original_vertex_ids = original_document
            .crease_pattern
            .vertices
            .iter()
            .map(|vertex| vertex.id)
            .collect::<Vec<_>>();
        let targets = edges
            .iter()
            .map(|edge| IntersectionClusterTargetRequest {
                edge_id: edge.id,
                relation: IntersectionClusterRelation::Interior,
            })
            .collect();

        let response =
            execute_intersection_cluster_connection(&mut project, project_id, 0, targets, None)
                .expect("the inclusive 64-target API limit must connect");

        assert_eq!(response.snapshot.revision, 1);
        assert!(response.snapshot.is_dirty);
        assert!(response.snapshot.can_undo);
        assert!(!response.snapshot.can_redo);
        assert!(!original_vertex_ids.contains(&response.vertex_id));
        assert_eq!(
            response
                .snapshot
                .crease_pattern
                .vertices
                .iter()
                .find(|vertex| vertex.id == response.vertex_id),
            Some(&Vertex {
                id: response.vertex_id,
                position: Point2::new(50.0, 50.0),
            })
        );
        assert_eq!(
            response.snapshot.crease_pattern.vertices.len(),
            original_document.crease_pattern.vertices.len() + 1
        );
        assert_eq!(
            response.snapshot.crease_pattern.edges.len(),
            original_document.crease_pattern.edges.len() + MAX_INTERSECTION_CLUSTER_TARGETS
        );
        for source in &edges {
            let split_original = response
                .snapshot
                .crease_pattern
                .edges
                .iter()
                .find(|edge| edge.id == source.id)
                .expect("each maximum-cluster source edge remains");
            assert_eq!(split_original.start, source.start);
            assert_eq!(split_original.end, response.vertex_id);
            assert_eq!(split_original.kind, source.kind);
            let generated = response
                .snapshot
                .crease_pattern
                .edges
                .iter()
                .find(|edge| {
                    !edges.iter().any(|source| source.id == edge.id)
                        && edge.start == response.vertex_id
                        && edge.end == source.end
                })
                .expect("each maximum-cluster source gets one generated half");
            assert_eq!(generated.kind, source.kind);
        }
        assert!(validation_snapshot(&project).is_valid);

        let (mut rejected_project, rejected_edges) = maximum_cluster_project();
        let rejected_project_id = rejected_project.project_id;
        let rejected_before = project_state_signature(&rejected_project);
        let error = execute_intersection_cluster_connection(
            &mut rejected_project,
            rejected_project_id,
            0,
            (0..=MAX_INTERSECTION_CLUSTER_TARGETS)
                .map(|index| IntersectionClusterTargetRequest {
                    edge_id: rejected_edges[index % rejected_edges.len()].id,
                    relation: IntersectionClusterRelation::Interior,
                })
                .collect(),
            None,
        )
        .expect_err("65 targets must be rejected at the API boundary");
        assert_eq!(
            error,
            "an intersection cluster supports at most 64 target edges, found 65"
        );
        assert_eq!(project_state_signature(&rejected_project), rejected_before);
    }

    #[test]
    fn intersection_cluster_api_reuses_junction_with_interior_and_endpoint_targets() {
        let (mut project, [horizontal, vertical, stem], junction) = reuse_cluster_project();
        let project_id = project.project_id;
        let original_document = project.document();
        let original_edge_ids = original_document
            .crease_pattern
            .edges
            .iter()
            .map(|edge| edge.id)
            .collect::<Vec<_>>();
        let targets = vec![
            IntersectionClusterTargetRequest {
                edge_id: stem.id,
                relation: IntersectionClusterRelation::Endpoint,
            },
            IntersectionClusterTargetRequest {
                edge_id: vertical.id,
                relation: IntersectionClusterRelation::Interior,
            },
            IntersectionClusterTargetRequest {
                edge_id: horizontal.id,
                relation: IntersectionClusterRelation::Interior,
            },
        ];

        let response = execute_intersection_cluster_connection(
            &mut project,
            project_id,
            0,
            targets,
            Some(junction),
        )
        .expect("connect a mixed interior/endpoint cluster at an existing vertex");

        assert_eq!(response.vertex_id, junction);
        assert_eq!(response.snapshot.revision, 1);
        assert!(response.snapshot.is_dirty);
        assert!(response.snapshot.can_undo);
        assert!(!response.snapshot.can_redo);
        assert_eq!(
            response.snapshot.crease_pattern.vertices,
            original_document.crease_pattern.vertices
        );
        assert_eq!(
            response.snapshot.crease_pattern.edges.len(),
            original_document.crease_pattern.edges.len() + 2
        );
        assert!(
            response
                .snapshot
                .crease_pattern
                .edges
                .iter()
                .any(|edge| edge == &stem)
        );
        for edge in [&horizontal, &vertical] {
            let split_original = response
                .snapshot
                .crease_pattern
                .edges
                .iter()
                .find(|candidate| candidate.id == edge.id)
                .expect("split original cluster edge");
            assert_eq!(split_original.start, edge.start);
            assert_eq!(split_original.end, junction);
            assert_eq!(split_original.kind, edge.kind);
            let generated = response
                .snapshot
                .crease_pattern
                .edges
                .iter()
                .find(|candidate| {
                    !original_edge_ids.contains(&candidate.id)
                        && candidate.start == junction
                        && candidate.end == edge.end
                })
                .expect("generated cluster edge");
            assert_eq!(generated.kind, edge.kind);
        }
        assert!(validation_snapshot(&project).is_valid);
        let connected_document = project.document();

        project
            .editor
            .undo(1)
            .expect("undo reused intersection cluster");
        assert_eq!(project.editor.revision(), 2);
        assert_eq!(project.document(), original_document);
        assert!(!project.is_dirty());

        project
            .editor
            .redo(2)
            .expect("redo reused intersection cluster");
        assert_eq!(project.editor.revision(), 3);
        assert_eq!(project.document(), connected_document);
        assert!(project.is_dirty());
        assert!(validation_snapshot(&project).is_valid);
    }

    #[test]
    fn intersection_cluster_api_rejections_are_atomic_and_boundary_remains_unsupported() {
        let interior_target = |edge: &Edge| IntersectionClusterTargetRequest {
            edge_id: edge.id,
            relation: IntersectionClusterRelation::Interior,
        };

        let (mut bounded_project, bounded_edges) = create_cluster_project(false);
        let bounded_project_id = bounded_project.project_id;
        let bounded_before = project_state_signature(&bounded_project);
        let too_few_error = execute_intersection_cluster_connection(
            &mut bounded_project,
            bounded_project_id,
            0,
            bounded_edges[..2].iter().map(interior_target).collect(),
            None,
        )
        .expect_err("fewer than three request targets must fail before ID allocation");
        assert_eq!(
            too_few_error,
            "an intersection cluster requires at least three target edges, found 2"
        );
        let too_many_error = execute_intersection_cluster_connection(
            &mut bounded_project,
            bounded_project_id,
            0,
            (0..65)
                .map(|_| interior_target(&bounded_edges[0]))
                .collect(),
            None,
        )
        .expect_err("more than 64 request targets must fail before ID allocation");
        assert_eq!(
            too_many_error,
            "an intersection cluster supports at most 64 target edges, found 65"
        );
        assert_eq!(project_state_signature(&bounded_project), bounded_before);

        let (mut stale_project, stale_edges) = create_cluster_project(false);
        let stale_project_id = stale_project.project_id;
        let stale_before = project_state_signature(&stale_project);
        let stale_error = execute_intersection_cluster_connection(
            &mut stale_project,
            stale_project_id,
            1,
            stale_edges.iter().map(interior_target).collect(),
            None,
        )
        .expect_err("stale cluster command must fail");
        assert_eq!(
            stale_error,
            "expected revision 1, but the current revision is 0"
        );
        assert_eq!(project_state_signature(&stale_project), stale_before);

        let (mut incomplete_project, incomplete_edges) = create_cluster_project(true);
        let incomplete_project_id = incomplete_project.project_id;
        let incomplete_before = project_state_signature(&incomplete_project);
        let incomplete_error = execute_intersection_cluster_connection(
            &mut incomplete_project,
            incomplete_project_id,
            0,
            incomplete_edges[..3].iter().map(interior_target).collect(),
            None,
        )
        .expect_err("an omitted intersecting edge must reject the whole cluster");
        assert!(incomplete_error.contains("also passes through the intersection cluster"));
        assert!(incomplete_error.contains(&format!("{:?}", incomplete_edges[3].id)));
        assert_eq!(
            project_state_signature(&incomplete_project),
            incomplete_before
        );

        let (mut boundary_project, boundary_edges) = create_cluster_project(false);
        let boundary_project_id = boundary_project.project_id;
        let boundary_before = project_state_signature(&boundary_project);
        let boundary = boundary_project.editor.pattern().edges[0].clone();
        let boundary_error = execute_intersection_cluster_connection(
            &mut boundary_project,
            boundary_project_id,
            0,
            vec![
                interior_target(&boundary),
                interior_target(&boundary_edges[1]),
                interior_target(&boundary_edges[2]),
            ],
            None,
        )
        .expect_err("boundary clusters remain unsupported in the first core increment");
        assert!(boundary_error.contains("does not yet support boundary edge"));
        assert_eq!(project_state_signature(&boundary_project), boundary_before);
    }

    #[test]
    fn t_junction_connection_returns_reused_vertex_and_undoable_dirty_snapshot() {
        let (mut project, interior, stem, junction) = t_junction_project();
        let project_id = project.project_id;
        let original_document = project.document();
        let original_vertex_count = original_document.crease_pattern.vertices.len();
        let original_edge_ids = original_document
            .crease_pattern
            .edges
            .iter()
            .map(|edge| edge.id)
            .collect::<Vec<_>>();

        let response =
            execute_t_junction_connection(&mut project, project_id, 0, stem.id, interior.id)
                .expect("connect T-junction with reverse arguments");

        assert_eq!(response.vertex_id, junction);
        assert_eq!(response.snapshot.revision, 1);
        assert!(response.snapshot.is_dirty);
        assert!(response.snapshot.can_undo);
        assert!(!response.snapshot.can_redo);
        assert_eq!(
            response.snapshot.crease_pattern.vertices.len(),
            original_vertex_count
        );
        assert_eq!(
            response.snapshot.crease_pattern.vertices,
            original_document.crease_pattern.vertices
        );
        let split_original = response
            .snapshot
            .crease_pattern
            .edges
            .iter()
            .find(|edge| edge.id == interior.id)
            .expect("split original edge");
        assert_eq!(split_original.start, interior.start);
        assert_eq!(split_original.end, junction);
        assert_eq!(split_original.kind, EdgeKind::Mountain);
        let generated = response
            .snapshot
            .crease_pattern
            .edges
            .iter()
            .find(|edge| !original_edge_ids.contains(&edge.id))
            .expect("generated T-junction edge");
        assert_eq!(generated.start, junction);
        assert_eq!(generated.end, interior.end);
        assert_eq!(generated.kind, EdgeKind::Mountain);
        assert!(
            response
                .snapshot
                .crease_pattern
                .edges
                .iter()
                .any(|edge| edge == &stem)
        );
        assert!(validation_snapshot(&project).is_valid);
        let connected_document = project.document();

        project.editor.undo(1).expect("undo T-junction connection");
        assert_eq!(project.editor.revision(), 2);
        assert_eq!(project.document(), original_document);
        assert!(!project.is_dirty());

        project.editor.redo(2).expect("redo T-junction connection");
        assert_eq!(project.editor.revision(), 3);
        assert_eq!(project.document(), connected_document);
        assert!(project.is_dirty());
        assert!(validation_snapshot(&project).is_valid);
    }

    #[test]
    fn boundary_t_junction_api_splits_sheet_outline_with_reused_vertex_and_exact_history() {
        let (mut project, boundary, stem, junction) = boundary_t_junction_project();
        let project_id = project.project_id;
        let original_document = project.document();
        let original_vertex_count = original_document.crease_pattern.vertices.len();
        let original_edge_ids = original_document
            .crease_pattern
            .edges
            .iter()
            .map(|edge| edge.id)
            .collect::<Vec<_>>();
        let original_boundary_vertices = original_document.paper.boundary_vertices.clone();

        let response =
            execute_t_junction_connection(&mut project, project_id, 0, stem.id, boundary.id)
                .expect("connect a crease endpoint to the strict interior of the sheet boundary");

        assert_eq!(response.vertex_id, junction);
        assert_eq!(response.snapshot.revision, 1);
        assert!(response.snapshot.is_dirty);
        assert!(response.snapshot.can_undo);
        assert!(!response.snapshot.can_redo);
        assert_eq!(
            response.snapshot.crease_pattern.vertices.len(),
            original_vertex_count
        );
        assert_eq!(
            response.snapshot.crease_pattern.vertices,
            original_document.crease_pattern.vertices
        );
        assert_eq!(
            response.snapshot.paper.boundary_vertices,
            vec![
                original_boundary_vertices[0],
                junction,
                original_boundary_vertices[1],
                original_boundary_vertices[2],
                original_boundary_vertices[3],
            ]
        );

        let split_original = response
            .snapshot
            .crease_pattern
            .edges
            .iter()
            .find(|edge| edge.id == boundary.id)
            .expect("original boundary segment");
        assert_eq!(split_original.start, boundary.start);
        assert_eq!(split_original.end, junction);
        assert_eq!(split_original.kind, EdgeKind::Boundary);
        let generated = response
            .snapshot
            .crease_pattern
            .edges
            .iter()
            .find(|edge| !original_edge_ids.contains(&edge.id))
            .expect("generated boundary segment");
        assert_eq!(generated.start, junction);
        assert_eq!(generated.end, boundary.end);
        assert_eq!(generated.kind, EdgeKind::Boundary);
        assert!(
            response
                .snapshot
                .crease_pattern
                .edges
                .iter()
                .any(|edge| edge == &stem)
        );
        assert!(validation_snapshot(&project).is_valid);
        let connected_document = project.document();

        project
            .editor
            .undo(1)
            .expect("undo boundary T-junction connection");
        assert_eq!(project.editor.revision(), 2);
        assert_eq!(project.document(), original_document);
        assert!(!project.is_dirty());

        project
            .editor
            .redo(2)
            .expect("redo boundary T-junction connection");
        assert_eq!(project.editor.revision(), 3);
        assert_eq!(project.document(), connected_document);
        assert!(project.is_dirty());
        assert!(validation_snapshot(&project).is_valid);
    }

    #[test]
    fn t_junction_api_conflicts_and_wrong_geometry_preserve_project_state() {
        let (mut project, interior, stem, _) = t_junction_project();
        let project_id = project.project_id;
        let before = project_state_signature(&project);

        let wrong_project =
            execute_t_junction_connection(&mut project, ProjectId::new(), 0, interior.id, stem.id)
                .expect_err("wrong project must fail");
        assert!(wrong_project.contains("active project changed"));
        assert_eq!(project_state_signature(&project), before);

        let stale =
            execute_t_junction_connection(&mut project, project_id, 3, interior.id, stem.id)
                .expect_err("stale revision must fail");
        assert_eq!(stale, "expected revision 3, but the current revision is 0");
        assert_eq!(project_state_signature(&project), before);

        let boundary = project.editor.pattern().edges[0].id;
        let boundary_error =
            execute_t_junction_connection(&mut project, project_id, 0, boundary, interior.id)
                .expect_err("non-intersecting boundary target must fail");
        assert_eq!(
            boundary_error,
            "the selected edges do not form exactly one strict T-junction"
        );
        assert_eq!(project_state_signature(&project), before);

        let (mut crossing, first, second) = crossing_project();
        let crossing_project_id = crossing.project_id;
        let crossing_before = project_state_signature(&crossing);
        let proper_x = execute_t_junction_connection(
            &mut crossing,
            crossing_project_id,
            0,
            first.id,
            second.id,
        )
        .expect_err("proper X must not be accepted as T-junction");
        assert_eq!(
            proper_x,
            "the selected edges do not form exactly one strict T-junction"
        );
        assert_eq!(project_state_signature(&crossing), crossing_before);
    }

    #[test]
    fn generated_id_boundary_split_handles_reverse_closing_edge_and_document_history() {
        let sheet = create_rectangular_sheet(100.0, 80.0, false).expect("valid rectangle");
        let (mut pattern, paper) = sheet.into_parts();
        let forward_closing_edge = pattern.edges[3].clone();
        pattern.edges[3] = Edge {
            start: forward_closing_edge.end,
            end: forward_closing_edge.start,
            ..forward_closing_edge
        };
        let target_edge = pattern.edges[3].clone();
        let original_vertex_ids = pattern
            .vertices
            .iter()
            .map(|vertex| vertex.id)
            .collect::<Vec<_>>();
        let original_edge_ids = pattern.edges.iter().map(|edge| edge.id).collect::<Vec<_>>();
        let mut project = ProjectState::new_with_paper(pattern, paper);
        let project_id = project.project_id;
        let original_document = project.document();

        let response = execute_boundary_split(&mut project, project_id, 0, target_edge.id, 0.25)
            .expect("split reverse closing edge");

        assert_eq!(response.revision, 1);
        assert!(response.is_dirty);
        assert!(response.can_undo);
        assert!(!response.can_redo);
        assert_eq!(response.paper.boundary_vertices.len(), 5);
        let new_vertex = response.paper.boundary_vertices[4];
        assert!(!original_vertex_ids.contains(&new_vertex));
        assert_eq!(response.crease_pattern.vertices.len(), 5);
        assert_eq!(
            response.crease_pattern.vertices[4],
            Vertex {
                id: new_vertex,
                position: Point2::new(0.0, 20.0),
            }
        );
        assert_eq!(response.crease_pattern.edges.len(), 5);
        assert_eq!(response.crease_pattern.edges[3].id, target_edge.id);
        assert_eq!(response.crease_pattern.edges[3].start, target_edge.start);
        assert_eq!(response.crease_pattern.edges[3].end, new_vertex);
        let generated_edge = &response.crease_pattern.edges[4];
        assert!(!original_edge_ids.contains(&generated_edge.id));
        assert_eq!(generated_edge.start, new_vertex);
        assert_eq!(generated_edge.end, target_edge.end);
        assert_eq!(generated_edge.kind, EdgeKind::Boundary);
        assert!(validation_snapshot(&project).is_valid);
        let split_document = project.document();
        assert_ne!(split_document, original_document);

        project.editor.undo(1).expect("undo boundary split");
        assert_eq!(project.editor.revision(), 2);
        assert_eq!(project.document(), original_document);
        assert!(!project.is_dirty());

        project.editor.redo(2).expect("redo boundary split");
        assert_eq!(project.editor.revision(), 3);
        assert_eq!(project.document(), split_document);
        assert!(project.is_dirty());
        assert!(validation_snapshot(&project).is_valid);
    }

    #[test]
    fn boundary_split_conflict_and_invalid_fraction_preserve_project_state() {
        let mut project = initial_project_state();
        let project_id = project.project_id;
        let edge = project.editor.pattern().edges[0].id;
        let before = project_state_signature(&project);

        let conflict = execute_boundary_split(&mut project, project_id, 1, edge, 0.5)
            .expect_err("stale split must fail");
        assert_eq!(
            conflict,
            "expected revision 1, but the current revision is 0"
        );
        assert_eq!(project_state_signature(&project), before);

        let invalid = execute_boundary_split(&mut project, project_id, 0, edge, f64::NAN)
            .expect_err("non-finite split must fail");
        assert_eq!(invalid, "boundary split fraction must be finite");
        assert_eq!(project_state_signature(&project), before);
    }

    #[test]
    fn boundary_vertex_removal_updates_document_dirty_state_and_history() {
        let mut project = initial_project_state();
        let project_id = project.project_id;
        let original_document = project.document();
        let target = project.editor.paper().boundary_vertices[1];
        let previous = project.editor.paper().boundary_vertices[0];
        let next = project.editor.paper().boundary_vertices[2];
        let remaining = project.editor.paper().boundary_vertices[3];
        let kept_edge = project.editor.pattern().edges[0].clone();
        let removed_edge = project.editor.pattern().edges[1].clone();

        let response = execute_command(
            &mut project,
            project_id,
            0,
            Command::RemoveBoundaryVertex { vertex: target },
        )
        .expect("remove boundary vertex");

        assert_eq!(response.revision, 1);
        assert!(response.is_dirty);
        assert!(response.can_undo);
        assert!(!response.can_redo);
        assert_eq!(
            response.paper.boundary_vertices,
            vec![previous, next, remaining]
        );
        assert!(
            !response
                .crease_pattern
                .vertices
                .iter()
                .any(|vertex| vertex.id == target)
        );
        assert_eq!(response.crease_pattern.edges[0].id, kept_edge.id);
        assert_eq!(response.crease_pattern.edges[0].start, previous);
        assert_eq!(response.crease_pattern.edges[0].end, next);
        assert!(
            !response
                .crease_pattern
                .edges
                .iter()
                .any(|edge| edge.id == removed_edge.id)
        );
        assert!(validation_snapshot(&project).is_valid);
        let removed_document = project.document();
        assert_ne!(removed_document, original_document);

        project.editor.undo(1).expect("undo boundary removal");
        assert_eq!(project.editor.revision(), 2);
        assert_eq!(project.document(), original_document);
        assert!(!project.is_dirty());

        project.editor.redo(2).expect("redo boundary removal");
        assert_eq!(project.editor.revision(), 3);
        assert_eq!(project.document(), removed_document);
        assert!(project.is_dirty());
        assert!(validation_snapshot(&project).is_valid);
    }

    #[test]
    fn boundary_vertex_removal_conflict_preserves_project_state() {
        let mut project = initial_project_state();
        let project_id = project.project_id;
        let target = project.editor.paper().boundary_vertices[1];
        let before = project_state_signature(&project);

        let error = execute_command(
            &mut project,
            project_id,
            1,
            Command::RemoveBoundaryVertex { vertex: target },
        )
        .expect_err("stale boundary removal must fail");

        assert_eq!(error, "expected revision 1, but the current revision is 0");
        assert_eq!(project_state_signature(&project), before);
    }

    #[test]
    fn new_project_replaces_only_the_expected_unchanged_project() {
        let mut project = initial_project_state();
        let old_instance_id = project.instance_id;
        let old_project_id = project.project_id;

        let response = replace_with_new_project(
            &mut project,
            old_instance_id,
            old_project_id,
            0,
            new_project_parameters(),
        )
        .expect("replace current project");

        assert_ne!(response.project_id, old_project_id);
        assert_eq!(response.project_id, project.project_id);
        assert_eq!(response.name, "Test sheet");
        assert!(response.current_path.is_none());
        assert_eq!(response.revision, 0);
        assert!(response.saved_revision.is_none());
        assert!(response.is_dirty);
        assert!(!response.can_undo);
        assert!(!response.can_redo);
        assert!(project.saved_document.is_none());
    }

    #[test]
    fn new_project_errors_leave_existing_state_untouched() {
        let mut project = initial_project_state();
        let instance_id = project.instance_id;
        let project_id = project.project_id;
        let before = project_state_signature(&project);

        assert!(
            replace_with_new_project(
                &mut project,
                instance_id,
                ProjectId::new(),
                0,
                new_project_parameters(),
            )
            .is_err()
        );
        assert_eq!(project_state_signature(&project), before);

        assert!(
            replace_with_new_project(
                &mut project,
                instance_id,
                project_id,
                1,
                new_project_parameters(),
            )
            .is_err()
        );
        assert_eq!(project_state_signature(&project), before);

        let mut invalid_name = new_project_parameters();
        invalid_name.name = " \0 ".to_owned();
        assert!(
            replace_with_new_project(&mut project, instance_id, project_id, 0, invalid_name)
                .is_err()
        );
        assert_eq!(project_state_signature(&project), before);

        let mut invalid_dimensions = new_project_parameters();
        invalid_dimensions.width_mm = 0.0;
        assert!(
            replace_with_new_project(&mut project, instance_id, project_id, 0, invalid_dimensions,)
                .is_err()
        );
        assert_eq!(project_state_signature(&project), before);

        let mut invalid_thickness = new_project_parameters();
        invalid_thickness.thickness_mm = f64::NAN;
        assert!(
            replace_with_new_project(&mut project, instance_id, project_id, 0, invalid_thickness,)
                .is_err()
        );
        assert_eq!(project_state_signature(&project), before);
    }

    #[test]
    fn delayed_new_project_rejects_same_document_revision_after_reopen_aba() {
        let mut project = initial_project_state();
        let stale_instance_id = project.instance_id;
        let expected_project_id = project.project_id;
        let expected_revision = project.editor.revision();
        let document = project.document();
        project = ProjectState::from_document(document, PathBuf::from("same-project.ori2"));
        assert_eq!(project.project_id, expected_project_id);
        assert_eq!(project.editor.revision(), expected_revision);
        assert_ne!(project.instance_id, stale_instance_id);
        let before = project_state_signature(&project);

        let error = replace_with_new_project(
            &mut project,
            stale_instance_id,
            expected_project_id,
            expected_revision,
            new_project_parameters(),
        )
        .expect_err("reopened ABA instance must reject delayed new-project work");

        assert_eq!(
            error,
            "the open project instance changed while the file dialog was open"
        );
        assert_eq!(project_state_signature(&project), before);
    }

    #[test]
    fn execute_command_rejects_same_document_revision_after_reopen_aba() {
        let project = initial_project_state();
        let stale_instance_id = project.instance_id;
        let expected_project_id = project.project_id;
        let expected_revision = project.editor.revision();
        let mut reopened =
            ProjectState::from_document(project.document(), PathBuf::from("same-project.ori2"));
        assert_eq!(reopened.project_id, expected_project_id);
        assert_eq!(reopened.editor.revision(), expected_revision);
        assert_ne!(reopened.instance_id, stale_instance_id);
        let before = project_state_signature(&reopened);

        let error = super::execute_command(
            &mut reopened,
            stale_instance_id,
            expected_project_id,
            expected_revision,
            Command::AddVertex {
                id: VertexId::new(),
                position: Point2::new(25.0, 25.0),
            },
        )
        .expect_err("reopened ABA instance must reject a delayed edit command");

        assert_eq!(
            error,
            "the open project instance changed while the file dialog was open"
        );
        assert_eq!(project_state_signature(&reopened), before);
    }

    #[test]
    fn execute_undo_rejects_same_project_and_revision_from_a_foreign_instance() {
        let mut stale_project = initial_project_state();
        let expected_project_id = stale_project.project_id;
        execute_command(
            &mut stale_project,
            expected_project_id,
            0,
            Command::SetCuttingAllowed { allowed: true },
        )
        .expect("advance the stale project to revision one");
        let stale_instance_id = stale_project.instance_id;
        let expected_revision = stale_project.editor.revision();

        let mut reopened = ProjectState::from_document(
            stale_project.document(),
            PathBuf::from("same-project.ori2"),
        );
        execute_command(
            &mut reopened,
            expected_project_id,
            0,
            Command::SetCuttingAllowed { allowed: false },
        )
        .expect("create undo history at the same revision");
        assert_eq!(reopened.editor.revision(), expected_revision);
        assert!(reopened.editor.can_undo());
        assert_ne!(reopened.instance_id, stale_instance_id);
        let before = project_state_signature(&reopened);

        let error = super::execute_undo(
            &mut reopened,
            stale_instance_id,
            expected_project_id,
            expected_revision,
        )
        .expect_err("foreign project instance must not consume undo history");

        assert_eq!(
            error,
            "the open project instance changed while the file dialog was open"
        );
        assert_eq!(project_state_signature(&reopened), before);
    }

    #[test]
    fn execute_redo_rejects_same_project_and_revision_from_a_foreign_instance() {
        let mut stale_project = initial_project_state();
        let expected_project_id = stale_project.project_id;
        execute_command(
            &mut stale_project,
            expected_project_id,
            0,
            Command::SetCuttingAllowed { allowed: true },
        )
        .expect("advance the stale project to revision one");
        execute_command(
            &mut stale_project,
            expected_project_id,
            1,
            Command::SetCuttingAllowed { allowed: false },
        )
        .expect("advance the stale project to revision two");
        let stale_instance_id = stale_project.instance_id;
        let expected_revision = stale_project.editor.revision();

        let mut reopened = ProjectState::from_document(
            stale_project.document(),
            PathBuf::from("same-project.ori2"),
        );
        execute_command(
            &mut reopened,
            expected_project_id,
            0,
            Command::SetCuttingAllowed { allowed: true },
        )
        .expect("create current-instance undo history");
        execute_undo(&mut reopened, expected_project_id, 1)
            .expect("create redo history at revision two");
        assert_eq!(reopened.editor.revision(), expected_revision);
        assert!(reopened.editor.can_redo());
        assert_ne!(reopened.instance_id, stale_instance_id);
        let before = project_state_signature(&reopened);

        let error = super::execute_redo(
            &mut reopened,
            stale_instance_id,
            expected_project_id,
            expected_revision,
        )
        .expect_err("foreign project instance must not consume redo history");

        assert_eq!(
            error,
            "the open project instance changed while the file dialog was open"
        );
        assert_eq!(project_state_signature(&reopened), before);
    }

    #[test]
    fn move_vertex_returns_the_updated_revision_and_snapshot() {
        let id = VertexId::new();
        let mut project = ProjectState::new(CreasePattern {
            vertices: vec![Vertex {
                id,
                position: Point2::new(1.0, 2.0),
            }],
            edges: Vec::new(),
        });
        let project_id = project.project_id;
        assert!(!project.is_dirty());

        let response = execute_command(
            &mut project,
            project_id,
            0,
            Command::MoveVertex {
                id,
                position: Point2::new(3.0, 5.0),
            },
        )
        .expect("move vertex");

        assert_eq!(response.revision, 1);
        assert_eq!(
            response.crease_pattern.vertices[0].position,
            Point2::new(3.0, 5.0)
        );
        assert!(response.can_undo);
        assert!(response.is_dirty);
    }

    #[test]
    fn face_vertex_batch_is_one_persisted_undo_redo_entry() {
        let first = VertexId::new();
        let second = VertexId::new();
        let edge = EdgeId::new();
        let mut project = ProjectState::new_unsaved(
            "face batch".to_owned(),
            CreasePattern {
                vertices: vec![
                    ori_domain::Vertex {
                        id: first,
                        position: Point2::new(1.0, 2.0),
                    },
                    ori_domain::Vertex {
                        id: second,
                        position: Point2::new(3.0, 4.0),
                    },
                ],
                edges: vec![ori_domain::Edge {
                    id: edge,
                    start: first,
                    end: second,
                    kind: EdgeKind::Mountain,
                }],
            },
            Paper::default(),
        );
        let project_id = project.project_id;
        execute_command(
            &mut project,
            project_id,
            0,
            Command::MoveVertices {
                updates: vec![
                    VertexPositionUpdate {
                        vertex: first,
                        position: Point2::new(11.0, 12.0),
                    },
                    VertexPositionUpdate {
                        vertex: second,
                        position: Point2::new(13.0, 14.0),
                    },
                ],
            },
        )
        .expect("move face vertices");
        let archive = project
            .project_archive()
            .expect("persist face move history");
        let mut reopened =
            ProjectState::from_project_archive(archive, PathBuf::from("face-batch.ori2"))
                .expect("restore face move history");
        assert_eq!(
            reopened.editor.pattern().vertices[0].position,
            Point2::new(11.0, 12.0)
        );
        assert_eq!(
            reopened.editor.pattern().vertices[1].position,
            Point2::new(13.0, 14.0)
        );
        let reopened_project_id = reopened.project_id;
        let undo_revision = reopened.editor.revision();
        execute_undo(&mut reopened, reopened_project_id, undo_revision)
            .expect("undo the face move as one entry");
        assert_eq!(
            reopened.editor.pattern().vertices[0].position,
            Point2::new(1.0, 2.0)
        );
        assert_eq!(
            reopened.editor.pattern().vertices[1].position,
            Point2::new(3.0, 4.0)
        );
        let redo_revision = reopened.editor.revision();
        execute_redo(&mut reopened, reopened_project_id, redo_revision)
            .expect("redo the face move as one entry");
        assert_eq!(
            reopened.editor.pattern().vertices[0].position,
            Point2::new(11.0, 12.0)
        );
        assert_eq!(
            reopened.editor.pattern().vertices[1].position,
            Point2::new(13.0, 14.0)
        );
    }

    #[test]
    fn initial_project_is_a_clean_square_sheet() {
        let project = initial_project_state();
        let snapshot = snapshot(&project);

        assert!(!snapshot.is_dirty);
        assert_eq!(snapshot.revision, 0);
        assert_eq!(project.editor.paper().boundary_vertices.len(), 4);
        assert_eq!(snapshot.crease_pattern.vertices.len(), 4);
        assert_eq!(snapshot.crease_pattern.edges.len(), 4);
        assert!(
            snapshot
                .crease_pattern
                .edges
                .iter()
                .all(|edge| edge.kind == EdgeKind::Boundary)
        );
    }

    #[test]
    fn remove_edge_then_vertex_returns_each_current_snapshot() {
        let start = VertexId::new();
        let end = VertexId::new();
        let edge = EdgeId::new();
        let mut project = ProjectState::new(CreasePattern {
            vertices: vec![
                Vertex {
                    id: start,
                    position: Point2::new(0.0, 0.0),
                },
                Vertex {
                    id: end,
                    position: Point2::new(1.0, 0.0),
                },
            ],
            edges: vec![Edge {
                id: edge,
                start,
                end,
                kind: EdgeKind::Mountain,
            }],
        });
        let project_id = project.project_id;

        let response = execute_command(
            &mut project,
            project_id,
            0,
            Command::RemoveEdge { id: edge },
        )
        .expect("remove edge");
        assert_eq!(response.revision, 1);
        assert!(response.crease_pattern.edges.is_empty());

        let response = execute_command(
            &mut project,
            project_id,
            1,
            Command::RemoveVertex { id: start },
        )
        .expect("remove vertex");
        assert_eq!(response.revision, 2);
        assert_eq!(response.crease_pattern.vertices.len(), 1);
        assert_eq!(response.crease_pattern.vertices[0].id, end);
    }

    #[test]
    fn edit_commands_preserve_revision_conflict_errors() {
        let id = VertexId::new();
        let mut project = ProjectState::new(CreasePattern {
            vertices: vec![Vertex {
                id,
                position: Point2::new(0.0, 0.0),
            }],
            edges: Vec::new(),
        });
        let project_id = project.project_id;

        let error = execute_command(&mut project, project_id, 4, Command::RemoveVertex { id })
            .expect_err("stale command must fail");

        assert_eq!(error, "expected revision 4, but the current revision is 0");
        assert_eq!(project.editor.pattern().vertices.len(), 1);
    }

    #[test]
    fn validation_snapshot_identifies_both_crossing_edges() {
        let vertices = [
            Vertex {
                id: VertexId::new(),
                position: Point2::new(0.0, 0.0),
            },
            Vertex {
                id: VertexId::new(),
                position: Point2::new(2.0, 2.0),
            },
            Vertex {
                id: VertexId::new(),
                position: Point2::new(0.0, 2.0),
            },
            Vertex {
                id: VertexId::new(),
                position: Point2::new(2.0, 0.0),
            },
        ];
        let first_edge = EdgeId::new();
        let second_edge = EdgeId::new();
        let project = ProjectState::new(CreasePattern {
            vertices: vertices.to_vec(),
            edges: vec![
                Edge {
                    id: first_edge,
                    start: vertices[0].id,
                    end: vertices[1].id,
                    kind: EdgeKind::Mountain,
                },
                Edge {
                    id: second_edge,
                    start: vertices[2].id,
                    end: vertices[3].id,
                    kind: EdgeKind::Valley,
                },
            ],
        });

        let response = validation_snapshot(&project);

        assert!(!response.is_valid);
        assert_eq!(response.project_id, project.project_id);
        assert_eq!(response.revision, 0);
        assert_eq!(response.issues.len(), 2);
        let crossing = response
            .issues
            .iter()
            .find(|issue| issue.code == "unsplit_intersection")
            .expect("crease-pattern issue");
        assert_eq!(crossing.edges, vec![first_edge, second_edge]);
        assert!(
            response
                .issues
                .iter()
                .any(|issue| issue.code == "too_few_boundary_vertices")
        );
    }

    #[test]
    fn valid_initial_sheet_has_no_combined_validation_issues() {
        let project = initial_project_state();

        let response = validation_snapshot(&project);

        assert!(response.is_valid);
        assert!(response.issues.is_empty());
    }

    #[test]
    fn initial_sheet_reports_boundary_vertices_as_locally_not_applicable() {
        let project = initial_project_state();

        let response = validation_snapshot(&project);
        let encoded = serde_json::to_value(&response).expect("serialize validation snapshot");
        let local = &encoded["local_flat_foldability"];

        assert_eq!(local["model"], "interior_single_vertex_zero_thickness_v1");
        assert_eq!(local["status"], "not_applicable");
        assert_eq!(local["total_vertices"], 4);
        assert_eq!(local["applicable_vertices"], 0);
        assert_eq!(local["not_applicable_vertices"], 4);
        for vertex in local["vertices"].as_array().expect("vertex reports") {
            assert_eq!(vertex["verdict"], "not_applicable");
            assert_eq!(vertex["reason"], "paper_boundary");
            assert_eq!(vertex["kawasaki"], "not_applicable");
            assert_eq!(vertex["maekawa"], "not_applicable");
        }
    }

    #[test]
    fn cardinal_mmmv_vertex_reports_both_local_conditions_satisfied() {
        let (project, center) = four_ray_square_project_state(
            [3, 5, 7, 1],
            [
                EdgeKind::Mountain,
                EdgeKind::Mountain,
                EdgeKind::Mountain,
                EdgeKind::Valley,
            ],
        );

        let response = validation_snapshot(&project);
        let encoded = serde_json::to_value(&response).expect("serialize validation snapshot");
        let center = serde_json::to_value(center).expect("serialize center vertex ID");
        let local = encoded["local_flat_foldability"]
            .as_object()
            .expect("local report object");
        let center_report = local["vertices"]
            .as_array()
            .expect("vertex reports")
            .iter()
            .find(|report| report["vertex"] == center)
            .expect("center report");

        assert_eq!(local["status"], "necessary_conditions_satisfied");
        assert_eq!(local["applicable_vertices"], 1);
        assert_eq!(local["satisfied_vertices"], 1);
        assert_eq!(center_report["fold_degree"], 4);
        assert_eq!(center_report["mountain_count"], 3);
        assert_eq!(center_report["valley_count"], 1);
        assert_eq!(center_report["verdict"], "satisfied");
        assert_eq!(center_report["reason"], serde_json::Value::Null);
        assert_eq!(center_report["kawasaki"], "satisfied");
        assert_eq!(center_report["maekawa"], "satisfied");
    }

    #[test]
    fn local_report_keeps_kawasaki_and_maekawa_violations_independent() {
        let (kawasaki_project, kawasaki_center) = four_ray_square_project_state(
            [3, 5, 7, 0],
            [
                EdgeKind::Mountain,
                EdgeKind::Mountain,
                EdgeKind::Mountain,
                EdgeKind::Valley,
            ],
        );
        let (maekawa_project, maekawa_center) = four_ray_square_project_state(
            [3, 5, 7, 1],
            [
                EdgeKind::Mountain,
                EdgeKind::Mountain,
                EdgeKind::Valley,
                EdgeKind::Valley,
            ],
        );

        let kawasaki = validation_snapshot(&kawasaki_project);
        let kawasaki_json =
            serde_json::to_value(&kawasaki).expect("serialize Kawasaki counterexample");
        let kawasaki_center =
            serde_json::to_value(kawasaki_center).expect("serialize Kawasaki center vertex ID");
        let kawasaki_center_report = kawasaki_json["local_flat_foldability"]["vertices"]
            .as_array()
            .expect("Kawasaki vertex reports")
            .iter()
            .find(|report| report["vertex"] == kawasaki_center)
            .expect("Kawasaki center report");
        assert_eq!(kawasaki_center_report["kawasaki"], "violated");
        assert_eq!(kawasaki_center_report["maekawa"], "satisfied");
        assert_eq!(kawasaki_center_report["verdict"], "violated");

        let maekawa = validation_snapshot(&maekawa_project);
        let maekawa_json =
            serde_json::to_value(&maekawa).expect("serialize Maekawa counterexample");
        let maekawa_center =
            serde_json::to_value(maekawa_center).expect("serialize Maekawa center vertex ID");
        let maekawa_center_report = maekawa_json["local_flat_foldability"]["vertices"]
            .as_array()
            .expect("Maekawa vertex reports")
            .iter()
            .find(|report| report["vertex"] == maekawa_center)
            .expect("Maekawa center report");
        assert_eq!(maekawa_center_report["kawasaki"], "satisfied");
        assert_eq!(maekawa_center_report["maekawa"], "violated");
        assert_eq!(maekawa_center_report["verdict"], "violated");
    }

    #[test]
    fn local_flat_foldability_json_contract_is_exact_and_does_not_change_geometry_validity() {
        let (project, center) = four_ray_square_project_state(
            [3, 5, 7, 1],
            [
                EdgeKind::Mountain,
                EdgeKind::Mountain,
                EdgeKind::Valley,
                EdgeKind::Valley,
            ],
        );

        let response = validation_snapshot(&project);
        assert!(response.is_valid);
        assert!(response.issues.is_empty());
        let encoded = serde_json::to_value(&response).expect("serialize validation snapshot");
        let center = serde_json::to_value(center).expect("serialize center vertex ID");
        let root_keys = encoded
            .as_object()
            .expect("validation object")
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let local = encoded["local_flat_foldability"]
            .as_object()
            .expect("local report object");
        let local_keys = local.keys().map(String::as_str).collect::<BTreeSet<_>>();
        let center_report = local["vertices"]
            .as_array()
            .expect("vertex reports")
            .iter()
            .find(|report| report["vertex"] == center)
            .expect("center report")
            .as_object()
            .expect("center report object");
        let center_keys = center_report
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();

        assert_eq!(
            root_keys,
            [
                "project_id",
                "revision",
                "is_valid",
                "issues",
                "local_flat_foldability"
            ]
            .into_iter()
            .collect()
        );
        assert_eq!(
            local_keys,
            [
                "model",
                "max_exact_fold_degree",
                "status",
                "total_vertices",
                "applicable_vertices",
                "satisfied_vertices",
                "violated_vertices",
                "not_applicable_vertices",
                "indeterminate_vertices",
                "vertices",
            ]
            .into_iter()
            .collect()
        );
        assert_eq!(
            center_keys,
            [
                "vertex",
                "fold_degree",
                "mountain_count",
                "valley_count",
                "verdict",
                "reason",
                "kawasaki",
                "maekawa",
            ]
            .into_iter()
            .collect()
        );
        assert_eq!(local["status"], "violated");
        assert_eq!(center_report["kawasaki"], "satisfied");
        assert_eq!(center_report["maekawa"], "violated");
    }

    #[test]
    fn paper_thickness_issues_are_included_without_highlight_targets() {
        let sheet = create_rectangular_sheet(20.0, 20.0, false).expect("valid square");
        let (pattern, mut paper) = sheet.into_parts();
        paper.thickness_mm = -0.01;
        let project = ProjectState::new_with_paper(pattern.clone(), paper);

        let response = validation_snapshot(&project);

        assert!(!response.is_valid);
        assert_eq!(response.issues.len(), 1);
        assert_eq!(response.issues[0].code, "negative_thickness");
        assert!(response.issues[0].vertices.is_empty());
        assert!(response.issues[0].edges.is_empty());

        let mut zero_paper = project.editor.paper().clone();
        zero_paper.thickness_mm = 0.0;
        let zero_project = ProjectState::new_with_paper(pattern, zero_paper);
        let zero_thickness = validation_snapshot(&zero_project);
        assert!(zero_thickness.is_valid);
        assert!(zero_thickness.issues.is_empty());
    }

    #[test]
    fn paper_intersection_maps_boundary_references_to_domain_edges() {
        let vertices = [
            Vertex {
                id: VertexId::new(),
                position: Point2::new(0.0, 0.0),
            },
            Vertex {
                id: VertexId::new(),
                position: Point2::new(2.0, 2.0),
            },
            Vertex {
                id: VertexId::new(),
                position: Point2::new(0.0, 2.0),
            },
            Vertex {
                id: VertexId::new(),
                position: Point2::new(2.0, 0.0),
            },
        ];
        let boundary_edges = [EdgeId::new(), EdgeId::new(), EdgeId::new(), EdgeId::new()];
        let pattern = CreasePattern {
            vertices: vertices.to_vec(),
            edges: vec![
                Edge {
                    id: boundary_edges[0],
                    start: vertices[0].id,
                    end: vertices[1].id,
                    kind: EdgeKind::Boundary,
                },
                Edge {
                    id: boundary_edges[1],
                    start: vertices[1].id,
                    end: vertices[2].id,
                    kind: EdgeKind::Boundary,
                },
                // Domain edges are undirected for boundary highlighting, so
                // mapping also accepts the reverse of the paper's order.
                Edge {
                    id: boundary_edges[2],
                    start: vertices[3].id,
                    end: vertices[2].id,
                    kind: EdgeKind::Boundary,
                },
                Edge {
                    id: boundary_edges[3],
                    start: vertices[3].id,
                    end: vertices[0].id,
                    kind: EdgeKind::Boundary,
                },
            ],
        };
        let paper = Paper {
            boundary_vertices: vertices.iter().map(|vertex| vertex.id).collect(),
            ..Paper::default()
        };
        let project = ProjectState::new_with_paper(pattern, paper);

        let response = validation_snapshot(&project);
        let intersection = response
            .issues
            .iter()
            .find(|issue| issue.code == "boundary_self_intersection")
            .expect("paper self-intersection issue");

        assert_eq!(
            intersection.vertices,
            vec![
                vertices[0].id,
                vertices[1].id,
                vertices[2].id,
                vertices[3].id
            ]
        );
        assert_eq!(
            intersection.edges,
            vec![boundary_edges[0], boundary_edges[2]]
        );
    }

    #[test]
    fn paper_boundary_topology_issues_include_actionable_targets() {
        let sheet = create_rectangular_sheet(20.0, 20.0, false).expect("valid square");
        let (mut pattern, paper) = sheet.into_parts();
        let boundary = paper.boundary_vertices.clone();

        pattern.edges[0].kind = EdgeKind::Mountain;
        let first_duplicate = pattern.edges[1].id;
        let duplicate_edge = Edge {
            id: EdgeId::new(),
            start: pattern.edges[1].end,
            end: pattern.edges[1].start,
            kind: EdgeKind::Boundary,
        };
        let duplicate = duplicate_edge.id;
        pattern.edges.push(duplicate_edge);
        let unexpected_edge = Edge {
            id: EdgeId::new(),
            start: boundary[0],
            end: boundary[2],
            kind: EdgeKind::Boundary,
        };
        let unexpected = unexpected_edge.id;
        pattern.edges.push(unexpected_edge);
        let project = ProjectState::new_with_paper(pattern, paper);

        let response = validation_snapshot(&project);
        let missing = response
            .issues
            .iter()
            .find(|issue| issue.code == "missing_boundary_edge")
            .expect("wrong-kind edge is missing from the Boundary set");
        assert_eq!(missing.vertices, vec![boundary[0], boundary[1]]);
        assert!(missing.edges.is_empty());

        let duplicate_issue = response
            .issues
            .iter()
            .find(|issue| issue.code == "duplicate_boundary_edge")
            .expect("duplicate Boundary record");
        assert_eq!(duplicate_issue.vertices, vec![boundary[1], boundary[2]]);
        assert_eq!(duplicate_issue.edges, vec![first_duplicate, duplicate]);

        let unexpected_issue = response
            .issues
            .iter()
            .find(|issue| issue.code == "unexpected_boundary_edge")
            .expect("unexpected Boundary chord");
        assert_eq!(unexpected_issue.vertices, vec![boundary[0], boundary[2]]);
        assert_eq!(unexpected_issue.edges, vec![unexpected]);
    }

    #[test]
    fn native_save_as_writes_a_loadable_file_and_preserves_editor_history() {
        let directory = TestDirectory::new();
        let selected_path = directory.join("折り紙設計.backup");
        let expected_path = directory.join("折り紙設計.ori2");
        let mut project = unsaved_project_with_redo_history("First project");
        let expected_instance_id = project.instance_id;
        let expected_project_id = project.project_id;
        let expected_revision = project.editor.revision();
        let document = project.document();
        let persisted_document = project
            .project_archive()
            .expect("serializable project")
            .document;
        let can_undo = project.editor.can_undo();
        let can_redo = project.editor.can_redo();

        let response = save_project_as_selected_path(
            &mut project,
            expected_instance_id,
            expected_project_id,
            expected_revision,
            selected_path,
        )
        .expect("save project under a selected path");

        assert!(!response.canceled);
        assert_eq!(
            project.current_path.as_deref(),
            Some(expected_path.as_path())
        );
        assert_eq!(project.saved_revision, Some(expected_revision));
        assert_eq!(project.saved_document.as_ref(), Some(&document));
        assert!(!project.is_dirty());
        assert_eq!(project.editor.revision(), expected_revision);
        assert_eq!(project.editor.can_undo(), can_undo);
        assert_eq!(project.editor.can_redo(), can_redo);
        assert_eq!(
            load_document_from_path(&expected_path).unwrap(),
            persisted_document
        );
        assert_eq!(fs::read_dir(&directory.path).unwrap().count(), 1);
    }

    #[test]
    fn native_save_then_reopen_restores_limit_and_both_history_stacks_in_order() {
        let directory = TestDirectory::new();
        let path = directory.join("history-roundtrip.ori2");
        let (mut source, first, second) =
            unsaved_project_with_undo_and_redo_history("History roundtrip");
        let source_project_id = source.project_id;
        let saved_document = source.document();
        let expected_history = source
            .editor
            .export_history_v1(source_project_id)
            .expect("export source history");

        save_project_to_path(&mut source, path.clone()).expect("save history archive");
        assert_eq!(source.editor.history_entry_limit(), 17);
        assert!(source.editor.can_undo());
        assert!(source.editor.can_redo());
        assert_eq!(
            source
                .editor
                .export_history_v1(source_project_id)
                .expect("history remains usable after save"),
            expected_history
        );

        let mut reopened = ProjectState::new(CreasePattern::empty());
        let replaced_instance_id = reopened.instance_id;
        let replaced_project_id = reopened.project_id;
        let loaded = load_project_file(path.clone()).expect("load saved history archive");
        apply_loaded_project_file(
            &mut reopened,
            replaced_instance_id,
            replaced_project_id,
            0,
            loaded,
        )
        .expect("apply saved history archive");

        assert_eq!(reopened.project_id, source_project_id);
        assert_ne!(reopened.instance_id, replaced_instance_id);
        assert_eq!(reopened.current_path.as_deref(), Some(path.as_path()));
        assert_eq!(reopened.saved_revision, Some(0));
        assert_eq!(reopened.saved_document.as_ref(), Some(&saved_document));
        assert!(!reopened.is_dirty());
        assert_eq!(reopened.editor.revision(), 0);
        assert_eq!(reopened.editor.history_entry_limit(), 17);
        assert!(reopened.editor.can_undo());
        assert!(reopened.editor.can_redo());
        assert!(reopened.editor.current_applied_pose().is_none());
        assert_eq!(
            reopened
                .editor
                .export_history_v1(source_project_id)
                .expect("re-export reopened history"),
            expected_history
        );

        reopened.editor.redo(0).expect("redo second command first");
        assert_eq!(
            reopened
                .editor
                .pattern()
                .vertices
                .iter()
                .map(|vertex| vertex.id)
                .collect::<Vec<_>>(),
            vec![first, second]
        );
        reopened.editor.undo(1).expect("undo second command");
        assert_eq!(reopened.document(), saved_document);
        reopened.editor.undo(2).expect("undo first command");
        assert!(reopened.editor.pattern().vertices.is_empty());
        reopened.editor.redo(3).expect("redo first command first");
        assert_eq!(reopened.editor.pattern().vertices[0].id, first);
        reopened.editor.redo(4).expect("redo second command second");
        assert_eq!(
            reopened
                .editor
                .pattern()
                .vertices
                .iter()
                .map(|vertex| vertex.id)
                .collect::<Vec<_>>(),
            vec![first, second]
        );
    }

    #[test]
    fn native_open_legacy_two_entry_archive_uses_default_empty_history() {
        let directory = TestDirectory::new();
        let path = directory.join("legacy-two-entry.ori2");
        let document = file_document("Legacy project", 23.0);
        fs::write(
            &path,
            write_project_ori2(&document).expect("write legacy two-entry archive"),
        )
        .expect("persist legacy archive");

        let mut reopened = ProjectState::new(CreasePattern::empty());
        let loaded = load_project_file(path.clone()).expect("load legacy archive");
        let expected_instance_id = reopened.instance_id;
        let expected_project_id = reopened.project_id;
        apply_loaded_project_file(
            &mut reopened,
            expected_instance_id,
            expected_project_id,
            0,
            loaded,
        )
        .expect("apply legacy archive");

        assert_eq!(reopened.document(), document);
        assert_eq!(reopened.editor.revision(), 0);
        assert_eq!(reopened.editor.history_entry_limit(), 128);
        assert!(!reopened.editor.can_undo());
        assert!(!reopened.editor.can_redo());
        assert_eq!(
            reopened
                .project_archive()
                .expect("export canonical legacy state")
                .editor_history,
            None
        );
        assert!(!reopened.is_dirty());
    }

    #[test]
    fn native_save_overwrites_atomically_and_keeps_undo_redo_history() {
        let directory = TestDirectory::new();
        let path = directory.join("overwrite.ori2");
        fs::write(&path, b"pre-existing invalid project").expect("write overwrite sentinel");
        let mut project = unsaved_project_with_redo_history("Overwrite project");

        save_project_to_path(&mut project, path.clone()).expect("replace existing file");
        let first_bytes = fs::read(&path).expect("read first native save");
        let first_persisted_document = project
            .project_archive()
            .expect("serializable project")
            .document;
        assert_ne!(first_bytes, b"pre-existing invalid project");
        assert_eq!(
            load_document_from_path(&path).unwrap(),
            first_persisted_document
        );
        assert!(project.editor.can_redo());

        let revision_before_redo = project.editor.revision();
        project
            .editor
            .redo(revision_before_redo)
            .expect("restore the saved redo command");
        assert!(project.is_dirty());
        let second_persisted_document = project
            .project_archive()
            .expect("serializable edited project")
            .document;
        let revision_before_save = project.editor.revision();
        let can_undo = project.editor.can_undo();
        let can_redo = project.editor.can_redo();

        save_project_to_path(&mut project, path.clone()).expect("overwrite with edited project");
        let second_bytes = fs::read(&path).expect("read overwritten native save");
        assert_ne!(second_bytes, first_bytes);
        assert_eq!(
            load_document_from_path(&path).unwrap(),
            second_persisted_document
        );
        assert_eq!(project.editor.revision(), revision_before_save);
        assert_eq!(project.editor.can_undo(), can_undo);
        assert_eq!(project.editor.can_redo(), can_redo);
        assert!(!project.is_dirty());
        assert_eq!(fs::read_dir(&directory.path).unwrap().count(), 1);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_staged_save_denies_concurrent_writers_and_cleans_up() {
        let directory = TestDirectory::new();
        let path = directory.join("writer-sharing.ori2");
        let staged = create_staged_file(&path).expect("create protected staged file");

        let writer_error = OpenOptions::new()
            .write(true)
            .open(&staged.path)
            .expect_err("a concurrent writer must be denied while staging");
        let rename_error = fs::rename(&staged.path, directory.join("swapped-stage"))
            .expect_err("a concurrent rename must be denied while staging");

        assert_eq!(writer_error.raw_os_error(), Some(32));
        assert_eq!(rename_error.raw_os_error(), Some(32));
        drop(staged);
        assert_eq!(fs::read_dir(&directory.path).unwrap().count(), 0);
    }

    #[cfg(unix)]
    #[test]
    fn native_save_overwrite_preserves_unix_file_mode() {
        use std::os::unix::fs::PermissionsExt;

        let directory = TestDirectory::new();
        let path = directory.join("mode-preservation.ori2");
        fs::write(&path, b"pre-existing invalid project").expect("write mode fixture");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).expect("set fixture mode");
        let mut project = unsaved_project_with_redo_history("Mode preservation");

        save_project_to_path(&mut project, path.clone()).expect("overwrite mode fixture");

        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o640
        );
        assert_eq!(load_document_from_path(&path).unwrap(), project.document());
    }

    #[cfg(unix)]
    #[test]
    fn unix_directory_sync_failure_is_only_reported_before_publish() {
        let directory = TestDirectory::new();
        let path = directory.join("directory-sync.ori2");
        let document = file_document("Directory sync", 42.0);
        let archive = Ori2ProjectArchive::document_only(document.clone());
        let bytes = write_project_ori2(&document).unwrap();

        fs::write(&path, b"keep before failed pre-publish sync").unwrap();
        let mut staged = prepare_staged_file(&path, &archive, &bytes).unwrap();
        let error = commit_unix_staged_project_file(
            &mut staged,
            &path,
            save_path::ExistingDestinationPolicy::ReplaceConfirmed,
            || Err(std::io::Error::other("injected pre-publish sync failure")),
        )
        .expect_err("a pre-publish directory sync failure must abort the commit");
        assert_eq!(error.kind(), std::io::ErrorKind::Other);
        drop(staged);
        assert_eq!(
            fs::read(&path).unwrap(),
            b"keep before failed pre-publish sync"
        );
        assert_eq!(fs::read_dir(&directory.path).unwrap().count(), 1);

        let mut staged = prepare_staged_file(&path, &archive, &bytes).unwrap();
        let mut sync_calls = 0_u8;
        commit_unix_staged_project_file(
            &mut staged,
            &path,
            save_path::ExistingDestinationPolicy::ReplaceConfirmed,
            || {
                sync_calls += 1;
                if sync_calls == 1 {
                    Ok(())
                } else {
                    Err(std::io::Error::other("injected post-publish sync failure"))
                }
            },
        )
        .expect("a post-publish durability failure must not report an ordinary save failure");
        drop(staged);

        assert_eq!(sync_calls, 2);
        assert_eq!(load_document_from_path(&path).unwrap(), document);
        assert_eq!(fs::read_dir(&directory.path).unwrap().count(), 1);
    }

    #[test]
    fn native_open_replaces_the_project_only_after_loading_and_validation() {
        let directory = TestDirectory::new();
        let path = directory.join("opened.ori2");
        let mut document = file_document("Opened project", 42.0);
        document.paper.cutting_allowed = true;
        persist_document(&path, &document).expect("write open fixture");

        let mut project = unsaved_project_with_redo_history("Replaced project");
        let expected_instance_id = project.instance_id;
        let replaced_project_id = project.project_id;
        let expected_revision = project.editor.revision();
        let loaded = load_project_file(path.clone()).expect("load native project");
        let response = apply_loaded_project_file(
            &mut project,
            expected_instance_id,
            replaced_project_id,
            expected_revision,
            loaded,
        )
        .expect("apply validated native project");

        assert!(!response.canceled);
        assert_ne!(project.project_id, replaced_project_id);
        let persisted = project.document();
        assert!(persisted.thumbnail_svg.is_some());
        document.thumbnail_svg = persisted.thumbnail_svg.clone();
        assert_eq!(persisted, document);
        assert_eq!(project.current_path.as_deref(), Some(path.as_path()));
        assert_eq!(project.editor.revision(), 0);
        assert!(!project.editor.can_undo());
        assert!(!project.editor.can_redo());
        assert!(!project.is_dirty());
    }

    #[test]
    fn corrupt_native_open_preserves_project_state_and_history() {
        let directory = TestDirectory::new();
        let secret_name = "private-client-corrupt.ori2";
        let path = directory.join(secret_name);
        let private_payload = b"not an ORIGAMI2 archive: SECRET_PROJECT_CONTENT";
        fs::write(&path, private_payload).expect("write corrupt fixture");
        let project = unsaved_project_with_redo_history("Unaffected project");
        let before = project_state_signature(&project);

        let error =
            load_project_file(path.clone()).expect_err("corrupt project must fail validation");

        assert_eq!(error, PROJECT_FILE_INVALID_MESSAGE);
        assert!(!error.contains(secret_name));
        assert!(!error.contains("SECRET_PROJECT_CONTENT"));
        assert!(!error.contains(&directory.path.to_string_lossy().into_owned()));
        assert_eq!(project_state_signature(&project), before);
    }

    #[test]
    fn corrupt_native_history_open_preserves_every_existing_project_field() {
        let directory = TestDirectory::new();
        let secret_name = "private-client-history-corrupt.ori2";
        let path = directory.join(secret_name);
        let (source, _, _) =
            unsaved_project_with_undo_and_redo_history("History corruption source");
        persist_project_archive(
            &path,
            &source.project_archive().expect("export source archive"),
        )
        .expect("write valid archive before targeted corruption");
        let corrupt_bytes =
            corrupt_editor_history_payload(fs::read(&path).expect("read valid history archive"));
        fs::write(&path, corrupt_bytes).expect("corrupt only the compressed history payload");

        let (project, _, _) =
            unsaved_project_with_undo_and_redo_history("Unaffected active project");
        let before = project_state_signature(&project);
        let error =
            load_project_file(path).expect_err("corrupt editor history must reject the open");

        assert_eq!(error, PROJECT_FILE_INVALID_MESSAGE);
        assert!(!error.contains(secret_name));
        assert!(!error.contains(&directory.path.to_string_lossy().into_owned()));
        assert_eq!(project_state_signature(&project), before);
    }

    #[test]
    fn save_rejects_an_invalid_instruction_pose_at_a_reachable_history_endpoint() {
        let directory = TestDirectory::new();
        let path = directory.join("must-not-save-reachable-pose.ori2");
        let mut project =
            project_with_reachable_invalid_instruction_pose("Unsafe history endpoint");
        let before = project_state_signature(&project);

        let error = save_project_to_path(&mut project, path.clone())
            .expect_err("save must validate every reachable history endpoint");

        assert_eq!(error, PROJECT_SERIALIZATION_FAILED_MESSAGE);
        assert_eq!(project_state_signature(&project), before);
        assert!(!path.exists());
    }

    #[test]
    fn save_rejects_an_invalid_instruction_pose_at_a_redo_endpoint() {
        let directory = TestDirectory::new();
        let path = directory.join("must-not-save-redo-pose.ori2");
        let mut project =
            project_with_redo_reachable_invalid_instruction_pose("Unsafe Redo endpoint");
        let before = project_state_signature(&project);

        let error = save_project_to_path(&mut project, path.clone())
            .expect_err("save must validate every reachable Redo endpoint");

        assert_eq!(error, PROJECT_SERIALIZATION_FAILED_MESSAGE);
        assert_eq!(project_state_signature(&project), before);
        assert!(!path.exists());
    }

    #[test]
    fn native_open_rejects_reachable_invalid_pose_history_without_mutating_current_state() {
        let directory = TestDirectory::new();
        let secret_name = "private-reachable-pose-history.ori2";
        let path = directory.join(secret_name);
        let source =
            project_with_reachable_invalid_instruction_pose("External unsafe history endpoint");
        let external_archive = Ori2ProjectArchive {
            document: source.document(),
            editor_history: Some(
                source
                    .editor
                    .export_history_v1(source.project_id)
                    .expect("export external history fixture"),
            ),
        };
        fs::write(
            &path,
            write_project_archive_ori2(&external_archive)
                .expect("the format boundary accepts replay-consistent external history"),
        )
        .expect("write external history fixture");

        let (active, _, _) =
            unsaved_project_with_undo_and_redo_history("Unaffected active project");
        let before = project_state_signature(&active);
        let error =
            load_project_file(path).expect_err("semantic history endpoint must reject open");

        assert_eq!(error, PROJECT_FILE_INVALID_MESSAGE);
        assert!(!error.contains(secret_name));
        assert!(!error.contains("instruction"));
        assert_eq!(project_state_signature(&active), before);
    }

    #[test]
    fn internal_archive_restore_rejects_a_history_bound_to_another_project() {
        let (source, _, _) = unsaved_project_with_undo_and_redo_history("Bound history");
        let mut archive = source.project_archive().expect("export bound history");
        archive.document.project_id = ProjectId::new();

        assert!(restore_archive_editor(&archive).is_err());
    }

    #[test]
    fn native_open_file_failures_use_fixed_path_free_categories() {
        let directory = TestDirectory::new();
        let secret_name = "private-client-missing.ori2";
        let missing_path = directory.join(secret_name);

        let missing_error =
            load_project_file(missing_path).expect_err("missing project must be rejected");
        assert_eq!(missing_error, PROJECT_FILE_OPEN_FAILED_MESSAGE);
        assert!(!missing_error.contains(secret_name));
        assert!(!missing_error.contains(&directory.path.to_string_lossy().into_owned()));
        assert!(!missing_error.to_ascii_lowercase().contains("os error"));

        let oversized_name = "private-client-oversized.ori2";
        let oversized_path = directory.join(oversized_name);
        File::create(&oversized_path)
            .expect("create oversized project fixture")
            .set_len(Ori2Limits::default().max_archive_size + 1)
            .expect("make sparse oversized project fixture");

        let oversized_error =
            load_project_file(oversized_path).expect_err("oversized project must be rejected");
        assert_eq!(oversized_error, PROJECT_FILE_TOO_LARGE_MESSAGE);
        assert!(!oversized_error.contains(oversized_name));
        assert!(
            !oversized_error.contains(&(Ori2Limits::default().max_archive_size + 1).to_string())
        );
        assert!(!oversized_error.contains(&directory.path.to_string_lossy().into_owned()));
    }

    #[test]
    fn native_open_instruction_failure_discards_private_semantic_details() {
        let project = initial_project_state();
        let mut document = project.document();
        let private_title = "SECRET_PRIVATE_INSTRUCTION";
        let private_face = FaceId::new();
        document.instruction_timeline.steps.push(InstructionStep {
            id: InstructionStepId::new(),
            title: private_title.to_owned(),
            description: String::new(),
            caution: String::new(),
            duration_ms: 1_000,
            visual: Default::default(),
            pose: InstructionPose {
                model: InstructionPoseModel::AbsoluteHingeAnglesV1,
                source_model_fingerprint: project.editor.fold_model_fingerprint_v1(),
                fixed_face: Some(private_face),
                hinge_angles: Vec::new(),
            },
        });
        let bytes = write_project_ori2(&document)
            .expect("syntactically valid project can carry a semantically invalid pose");
        let directory = TestDirectory::new();
        let secret_name = "private-instruction-project.ori2";
        let path = directory.join(secret_name);
        fs::write(&path, bytes).expect("write instruction failure fixture");

        let error =
            load_project_file(path).expect_err("semantic instruction failure must be rejected");

        assert_eq!(error, PROJECT_INSTRUCTIONS_INVALID_MESSAGE);
        assert!(!error.contains(private_title));
        assert!(!error.contains(&format!("{private_face:?}")));
        assert!(!error.contains(secret_name));
        assert!(!error.contains(&directory.path.to_string_lossy().into_owned()));
    }

    #[test]
    fn stale_native_open_is_rejected_without_replacing_newer_history() {
        let directory = TestDirectory::new();
        let path = directory.join("stale-open.ori2");
        persist_document(&path, &file_document("Stale open", 17.0))
            .expect("write stale-open fixture");
        let mut project = unsaved_project_with_redo_history("Active project");
        let expected_instance_id = project.instance_id;
        let expected_project_id = project.project_id;
        let stale_revision = project.editor.revision();
        let loaded = load_project_file(path).expect("prepare native open");
        execute_command(
            &mut project,
            expected_project_id,
            stale_revision,
            Command::AddVertex {
                id: VertexId::new(),
                position: Point2::new(8.0, 9.0),
            },
        )
        .expect("edit while the file dialog is open");
        let before_apply = project_state_signature(&project);

        let error = apply_loaded_project_file(
            &mut project,
            expected_instance_id,
            expected_project_id,
            stale_revision,
            loaded,
        )
        .expect_err("stale open must not replace the active project");

        assert_eq!(error, "the project changed while the file dialog was open");
        assert_eq!(project_state_signature(&project), before_apply);
    }

    #[test]
    fn native_file_dialog_results_cannot_land_after_reopening_the_same_document() {
        let directory = TestDirectory::new();
        let current_path = directory.join("same-document.ori2");
        let opened_path = directory.join("other-document.ori2");
        let selected_path = directory.join("must-not-save.ori2");
        let document = file_document("Same document", 21.0);
        persist_document(&current_path, &document).expect("write same-document fixture");
        persist_document(&opened_path, &file_document("Other document", 34.0))
            .expect("write other-document fixture");

        let mut project = ProjectState::from_document(document.clone(), current_path.clone());
        let stale_instance_id = project.instance_id;
        let expected_project_id = project.project_id;
        let expected_revision = project.editor.revision();
        let loaded = load_project_file(opened_path).expect("load delayed open result");

        project = ProjectState::from_document(document, current_path);
        assert_eq!(project.project_id, expected_project_id);
        assert_eq!(project.editor.revision(), expected_revision);
        assert_ne!(project.instance_id, stale_instance_id);
        let before = project_state_signature(&project);

        let open_error = apply_loaded_project_file(
            &mut project,
            stale_instance_id,
            expected_project_id,
            expected_revision,
            loaded,
        )
        .expect_err("a delayed open must not replace a reopened project instance");
        assert_eq!(
            open_error,
            "the open project instance changed while the file dialog was open"
        );
        assert_eq!(project_state_signature(&project), before);

        let save_error = save_project_as_selected_path(
            &mut project,
            stale_instance_id,
            expected_project_id,
            expected_revision,
            selected_path.clone(),
        )
        .expect_err("a delayed save must not target a reopened project instance");
        assert_eq!(
            save_error,
            "the open project instance changed while the file dialog was open"
        );
        assert_eq!(project_state_signature(&project), before);
        assert!(!selected_path.exists());
    }

    #[test]
    fn native_save_failure_preserves_state_history_and_existing_target() {
        let directory = TestDirectory::new();
        let occupied_path = directory.join("occupied.ori2");
        fs::create_dir(&occupied_path).expect("create an unreplaceable save target");
        let sentinel = occupied_path.join("keep.txt");
        fs::write(&sentinel, b"keep this directory").expect("write save-failure sentinel");
        let mut project = unsaved_project_with_redo_history("Failed save");
        let expected_instance_id = project.instance_id;
        let expected_project_id = project.project_id;
        let expected_revision = project.editor.revision();
        let before = project_state_signature(&project);

        let error = save_project_as_selected_path(
            &mut project,
            expected_instance_id,
            expected_project_id,
            expected_revision,
            occupied_path.clone(),
        )
        .expect_err("a directory cannot be replaced by a project file");

        assert_eq!(
            error,
            "プロジェクトを保存先へ安全に確定できなかったため、保存を中止しました。"
        );
        assert!(!error.contains("occupied.ori2"));
        assert!(!error.contains(&directory.path.display().to_string()));
        assert_eq!(project_state_signature(&project), before);
        assert_eq!(fs::read(&sentinel).unwrap(), b"keep this directory");
        assert!(occupied_path.is_dir());
        assert_eq!(fs::read_dir(&directory.path).unwrap().count(), 1);
    }

    #[test]
    fn stale_native_save_as_is_rejected_before_touching_the_selected_path() {
        let directory = TestDirectory::new();
        let selected_path = directory.join("stale-save");
        let normalized_path = directory.join("stale-save.ori2");
        let mut project = unsaved_project_with_redo_history("Stale save");
        let expected_instance_id = project.instance_id;
        let expected_project_id = project.project_id;
        let stale_revision = project.editor.revision();
        execute_command(
            &mut project,
            expected_project_id,
            stale_revision,
            Command::AddVertex {
                id: VertexId::new(),
                position: Point2::new(99.0, 100.0),
            },
        )
        .expect("edit before stale save-as is applied");
        let before_save = project_state_signature(&project);

        let error = save_project_as_selected_path(
            &mut project,
            expected_instance_id,
            expected_project_id,
            stale_revision,
            selected_path,
        )
        .expect_err("stale save-as must fail");

        assert_eq!(error, "the project changed while the file dialog was open");
        assert_eq!(project_state_signature(&project), before_save);
        assert!(!normalized_path.exists());
    }

    #[test]
    fn native_save_as_cannot_overwrite_an_existing_unconfirmed_corrected_path() {
        let directory = TestDirectory::new();
        let selected_path = directory.join("project.txt");
        let corrected_path = directory.join("project.ori2");
        fs::write(&corrected_path, b"keep existing project").unwrap();
        let mut project = unsaved_project_with_redo_history("Protected project");
        let expected_instance_id = project.instance_id;
        let expected_project_id = project.project_id;
        let expected_revision = project.editor.revision();
        let before = project_state_signature(&project);

        let error = save_project_as_selected_path(
            &mut project,
            expected_instance_id,
            expected_project_id,
            expected_revision,
            selected_path.clone(),
        )
        .expect_err("an unconfirmed corrected destination must not be overwritten");

        assert!(error.contains("上書き確認"));
        assert_eq!(project_state_signature(&project), before);
        assert_eq!(fs::read(corrected_path).unwrap(), b"keep existing project");
        assert!(!selected_path.exists());
    }

    #[test]
    fn project_save_target_conversion_error_discards_the_raw_path_and_os_error() {
        let raw_error = r"C:\Users\private-work\secret.ori2: injected operating-system detail";

        let error = project_save_target_conversion_error(raw_error);

        assert_eq!(error, "選択された保存先はローカルファイルではありません。");
        assert!(!error.contains("private-work"));
        assert!(!error.contains("operating-system"));
    }

    #[test]
    fn extension_correction_race_cannot_replace_a_new_destination() {
        let directory = TestDirectory::new();
        let selected_path = directory.join("race-target.backup");
        let corrected_path = directory.join("race-target.ori2");
        let destination =
            ensure_ori2_extension(selected_path).expect("preflight an unused corrected path");
        assert_eq!(
            destination.existing_destination_policy(),
            save_path::ExistingDestinationPolicy::RejectExisting
        );

        let protected_bytes = b"created after extension preflight";
        fs::write(&corrected_path, protected_bytes).unwrap();
        let mut project = unsaved_project_with_redo_history("Race-safe project");
        let before = project_state_signature(&project);

        let error = save_project_to_destination(&mut project, destination)
            .expect_err("atomic create-new commit must reject the intervening destination");

        assert!(error.contains("安全に確定"));
        assert!(!error.contains("race-target"));
        assert_eq!(fs::read(&corrected_path).unwrap(), protected_bytes);
        assert_eq!(project_state_signature(&project), before);
        assert!(
            fs::read_dir(&directory.path)
                .unwrap()
                .filter_map(Result::ok)
                .all(|entry| !entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".origami2-")),
            "a rejected create-new commit must clean its staged file"
        );
    }

    #[test]
    fn correct_extension_keeps_the_dialog_confirmed_overwrite() {
        let directory = TestDirectory::new();
        let path = directory.join("confirmed.ori2");
        fs::write(&path, b"OS-confirmed old bytes").unwrap();
        let mut project = unsaved_project_with_redo_history("Confirmed overwrite");
        let expected_persisted_document = project
            .project_archive()
            .expect("serializable project")
            .document;
        let destination =
            ensure_ori2_extension(path.clone()).expect("accept a dialog-confirmed extension");

        save_project_to_destination(&mut project, destination)
            .expect("replace the dialog-confirmed destination");

        assert_eq!(
            load_document_from_path(&path).unwrap(),
            expected_persisted_document
        );
        assert_eq!(project.current_path.as_deref(), Some(path.as_path()));
    }

    #[test]
    fn save_as_extension_is_normalized_without_changing_valid_case() {
        assert_eq!(
            ensure_ori2_extension(PathBuf::from("crane")).unwrap(),
            PathBuf::from("crane.ori2")
        );
        assert_eq!(
            ensure_ori2_extension(PathBuf::from("crane.json")).unwrap(),
            PathBuf::from("crane.ori2")
        );
        assert_eq!(
            ensure_ori2_extension(PathBuf::from("crane.ORI2")).unwrap(),
            PathBuf::from("crane.ORI2")
        );
    }

    #[test]
    fn relative_save_path_uses_the_current_directory_for_staging_and_sync() {
        assert_eq!(
            containing_directory(Path::new("bird.ori2")),
            Some(Path::new("."))
        );
        assert_eq!(
            containing_directory(Path::new("projects/bird.ori2")),
            Some(Path::new("projects"))
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_relative_save_path_publishes_the_verified_project() {
        let directory = TestDirectory::new_relative();
        let path = directory.join("relative.ori2");
        let document = file_document("Relative Windows save", 31.0);
        assert!(path.is_relative());

        persist_document(&path, &document).expect("publish to a relative Windows path");

        assert_eq!(load_document_from_path(&path).unwrap(), document);
        assert_eq!(fs::read_dir(&directory.path).unwrap().count(), 1);
    }

    #[test]
    fn suggested_name_removes_platform_forbidden_characters() {
        assert_eq!(
            suggested_file_name("  Bird: prototype?  "),
            "Bird_ prototype_.ori2"
        );
        assert_eq!(suggested_file_name("..."), "Untitled.ori2");
    }

    #[test]
    fn generated_container_verification_is_pure_and_checks_identity() {
        let document = ProjectDocument::new("Bird", CreasePattern::empty());
        let archive = Ori2ProjectArchive::document_only(document.clone());
        let bytes = write_project_ori2(&document).expect("generate .ori2");
        verify_generated_ori2(&archive, &bytes).expect("verify generated .ori2");

        let different_document = ProjectDocument::new("Different", CreasePattern::empty());
        let different_archive = Ori2ProjectArchive::document_only(different_document);
        let error = verify_generated_ori2(&different_archive, &bytes)
            .expect_err("a different project must not verify");
        assert_eq!(error, "generated .ori2 data did not round-trip exactly");

        let (history_project, _, _) =
            unsaved_project_with_undo_and_redo_history("History must not disappear");
        let history_archive = history_project
            .project_archive()
            .expect("export nonempty history");
        let document_only_bytes = write_project_ori2(&history_archive.document)
            .expect("write bytes that intentionally omit history");
        let error = verify_generated_ori2(&history_archive, &document_only_bytes)
            .expect_err("stage verification must reject silently dropped history");
        assert_eq!(error, "generated .ori2 data did not round-trip exactly");
    }

    #[test]
    fn document_snapshot_keeps_identity_name_and_dirty_state() {
        let mut document = ProjectDocument::new("Loaded bird", CreasePattern::empty());
        document.memo = "Check the reverse side.".to_owned();
        document.paper.cutting_allowed = true;
        let project = ProjectState::from_document(document.clone(), PathBuf::from("bird.ori2"));
        let response = snapshot(&project);

        assert_eq!(response.project_id, document.project_id);
        assert_eq!(response.name, "Loaded bird");
        assert_eq!(response.memo, "Check the reverse side.");
        assert_eq!(response.current_path.as_deref(), Some("bird.ori2"));
        assert!(!response.is_dirty);
        assert_eq!(response.paper, document.paper);
        assert!(response.cutting_allowed);
        assert!(!response.can_undo);
        let persisted = project.document();
        assert!(persisted.thumbnail_svg.is_some());
        document.thumbnail_svg = persisted.thumbnail_svg.clone();
        assert_eq!(persisted, document);
    }

    #[test]
    fn project_memo_is_dirty_undoable_and_round_trips_through_history() {
        let mut project = ProjectState::new(CreasePattern::empty());
        project
            .editor
            .execute(
                0,
                Command::UpdateProjectMemo {
                    memo: "First draft".to_owned(),
                },
            )
            .unwrap();
        assert_eq!(project.document().memo, "First draft");
        assert!(project.is_dirty());

        let archive = project.project_archive().unwrap();
        let mut reopened =
            ProjectState::from_project_archive(archive, PathBuf::from("memo.ori2")).unwrap();
        assert_eq!(reopened.document().memo, "First draft");
        reopened.editor.undo(reopened.editor.revision()).unwrap();
        assert!(reopened.document().memo.is_empty());
        reopened.editor.redo(reopened.editor.revision()).unwrap();
        assert_eq!(reopened.document().memo, "First draft");
    }

    #[test]
    fn stale_project_identity_cannot_mutate_a_replacement_project() {
        let mut project = ProjectState::new(CreasePattern::empty());
        let stale_project_id = ProjectId::new();

        let error = execute_command(
            &mut project,
            stale_project_id,
            0,
            Command::AddVertex {
                id: VertexId::new(),
                position: Point2::new(1.0, 1.0),
            },
        )
        .expect_err("a command for another project must fail");

        assert_eq!(
            error,
            "the active project changed before the command was applied"
        );
        assert!(project.editor.pattern().vertices.is_empty());
    }

    #[test]
    fn undoing_to_saved_content_clears_dirty_state() {
        let vertex_id = VertexId::new();
        let document = ProjectDocument::new(
            "Saved bird",
            CreasePattern {
                vertices: vec![Vertex {
                    id: vertex_id,
                    position: Point2::new(1.0, 2.0),
                }],
                edges: Vec::new(),
            },
        );
        let mut project = ProjectState::from_document(document, PathBuf::from("bird.ori2"));
        let project_id = project.project_id;

        execute_command(
            &mut project,
            project_id,
            0,
            Command::MoveVertex {
                id: vertex_id,
                position: Point2::new(3.0, 4.0),
            },
        )
        .expect("move vertex");
        assert!(project.is_dirty());

        project.editor.undo(1).expect("undo to save point");
        assert!(!project.is_dirty());
    }

    #[test]
    fn undoing_a_removal_to_saved_order_clears_dirty_state() {
        let vertices = [
            Vertex {
                id: VertexId::new(),
                position: Point2::new(0.0, 0.0),
            },
            Vertex {
                id: VertexId::new(),
                position: Point2::new(1.0, 0.0),
            },
            Vertex {
                id: VertexId::new(),
                position: Point2::new(2.0, 0.0),
            },
        ];
        let document = ProjectDocument::new(
            "Saved bird",
            CreasePattern {
                vertices: vertices.to_vec(),
                edges: Vec::new(),
            },
        );
        let mut project = ProjectState::from_document(document, PathBuf::from("bird.ori2"));
        let project_id = project.project_id;

        execute_command(
            &mut project,
            project_id,
            0,
            Command::RemoveVertex { id: vertices[1].id },
        )
        .expect("remove middle vertex");
        assert!(project.is_dirty());

        project.editor.undo(1).expect("undo to saved ordering");
        assert!(!project.is_dirty());
    }

    #[test]
    fn fold_import_staging_keeps_only_the_latest_preview_and_cancel_is_scoped() {
        let state = FoldImportState::default();
        let project = initial_project_state();
        let first = stage_pending_fold_import(
            &state,
            project.instance_id,
            project.project_id,
            project.editor.revision(),
            br#"{"file_spec":1.2}"#.to_vec(),
        )
        .expect("stage first import");
        let second = stage_pending_fold_import(
            &state,
            project.instance_id,
            project.project_id,
            project.editor.revision(),
            br#"{"file_spec":1.2,"file_title":"newer"}"#.to_vec(),
        )
        .expect("stage replacement import");

        assert_ne!(first, second);
        assert!(pending_fold_import(&state, first, project.project_id, 0).is_err());
        assert_eq!(
            cancel_pending_fold_import(&state, first).unwrap_err(),
            "the FOLD import preview was replaced by a newer preview"
        );
        assert!(pending_fold_import(&state, second, project.project_id, 0).is_ok());
        cancel_pending_fold_import(&state, second).expect("cancel current import");
        cancel_pending_fold_import(&state, second).expect("cancel remains idempotent");
        assert!(lock_fold_import(&state).unwrap().is_none());
    }

    #[test]
    fn svg_import_staging_keeps_only_the_latest_preview_and_cancel_is_scoped() {
        let state = SvgImportState::default();
        let project = initial_project_state();
        let first = stage_pending_svg_import(
            &state,
            project.instance_id,
            project.project_id,
            project.editor.revision(),
            br#"<svg xmlns="http://www.w3.org/2000/svg"/>"#.to_vec(),
        )
        .expect("stage first SVG import");
        let second = stage_pending_svg_import(
            &state,
            project.instance_id,
            project.project_id,
            project.editor.revision(),
            br#"<svg xmlns="http://www.w3.org/2000/svg"><title>newer</title></svg>"#.to_vec(),
        )
        .expect("stage replacement SVG import");

        assert_ne!(first, second);
        assert!(pending_svg_import(&state, first, project.project_id, 0).is_err());
        assert_eq!(
            cancel_pending_svg_import(&state, first).unwrap_err(),
            "the SVG import preview was replaced by a newer preview"
        );
        assert!(pending_svg_import(&state, second, project.project_id, 0).is_ok());
        cancel_pending_svg_import(&state, second).expect("cancel current import");
        cancel_pending_svg_import(&state, second).expect("cancel remains idempotent");
        assert!(lock_svg_import(&state).unwrap().pending.is_none());
        assert!(cancel_pending_svg_import(&state, ProjectId::new()).is_err());
    }

    #[test]
    fn svg_import_settings_validation_returns_exact_dimensions_without_replacing_project() {
        let bytes = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 50">
              <rect x="0" y="0" width="100" height="50"
                    fill="none" stroke="#222" data-origami-kind="boundary"/>
              <line x1="0" y1="25" x2="100" y2="25"
                    stroke="#111" data-origami-kind="cut"/>
            </svg>"##;
        let preview = read_svg_preview(bytes).expect("read validation fixture");
        let mut mappings = preview
            .style_groups()
            .iter()
            .map(|group| SvgGroupMapping {
                group: group.id,
                target: match group.semantic.as_deref() {
                    Some("boundary") => SvgGroupTarget::Boundary,
                    Some("cut") => SvgGroupTarget::Cut,
                    _ => SvgGroupTarget::Ignore,
                },
            })
            .collect::<Vec<_>>();
        mappings.sort_by_key(|mapping| mapping.group);

        let state = SvgImportState::default();
        let project = initial_project_state();
        let project_before = project_state_signature(&project);
        let import_id = stage_pending_svg_import(
            &state,
            project.instance_id,
            project.project_id,
            project.editor.revision(),
            bytes.to_vec(),
        )
        .expect("stage validation fixture");
        let validation_id = ProjectId::new();
        let pending = begin_svg_import_settings_validation(
            &state,
            validation_id,
            import_id,
            project.project_id,
            project.editor.revision(),
        )
        .expect("begin validation");
        let geometry = validate_svg_import_geometry(&pending.bytes, 2.0, mappings.clone(), None)
            .expect("validate boundary-group geometry");

        let response = {
            let mut slot = lock_svg_import(&state).expect("lock validation state");
            let response = complete_svg_import_settings_validation(
                &mut slot,
                &project,
                SvgImportSettingsValidationCompletion {
                    validation: SvgImportSettingsValidation {
                        validation_id,
                        import_id: pending.import_id,
                        expected_instance_id: pending.expected_instance_id,
                        expected_project_id: pending.expected_project_id,
                        expected_revision: pending.expected_revision,
                        millimeters_per_unit_bits: 2.0_f64.to_bits(),
                        boundary_candidate: None,
                        group_mappings: mappings.clone(),
                    },
                    geometry,
                },
            )
            .expect("complete validation");
            let current =
                pending_svg_import_in_slot(&slot, import_id, project.project_id, 0).unwrap();
            ensure_svg_import_settings_validation(
                &slot,
                current,
                validation_id,
                None,
                2.0,
                &mappings,
            )
            .expect("bind validation to exact settings");
            assert!(
                slot.pending.is_some(),
                "validation must retain staged bytes"
            );
            response
        };

        assert_eq!(response.validation_id, validation_id);
        assert_eq!(response.preview_id, import_id);
        assert_eq!(response.expected_project_id, project.project_id);
        assert_eq!(response.expected_revision, 0);
        assert_eq!(response.millimeters_per_unit, 2.0);
        assert_eq!(response.boundary_candidate_id, None);
        assert_eq!(response.width_mm, 200.0);
        assert_eq!(response.height_mm, 100.0);
        assert!(response.has_cuts);
        assert_eq!(project_state_signature(&project), project_before);
    }

    #[test]
    fn svg_import_settings_validation_binds_candidate_and_effective_cut_result() {
        let bytes = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 50">
              <polygon points="0,0 100,0 100,50 0,50"
                       fill="none" stroke="#111" data-origami-kind="cut"/>
            </svg>"##;
        let preview = read_svg_preview(bytes).expect("read candidate fixture");
        let candidate = preview
            .boundary_candidates()
            .iter()
            .find(|candidate| candidate.kind == SvgBoundaryCandidateKind::Polygon)
            .expect("polygon candidate");
        let mappings = preview
            .style_groups()
            .iter()
            .map(|group| SvgGroupMapping {
                group: group.id,
                target: SvgGroupTarget::Cut,
            })
            .collect::<Vec<_>>();
        let snapshot = svg_import_preview_snapshot(ProjectId::new(), &preview)
            .expect("build candidate snapshot");
        assert!(
            snapshot
                .boundary_candidates
                .iter()
                .any(|candidate| candidate.kind == "polygon")
        );

        let state = SvgImportState::default();
        let project = initial_project_state();
        let import_id = stage_pending_svg_import(
            &state,
            project.instance_id,
            project.project_id,
            0,
            bytes.to_vec(),
        )
        .expect("stage candidate fixture");
        let validation_id = ProjectId::new();
        let pending = begin_svg_import_settings_validation(
            &state,
            validation_id,
            import_id,
            project.project_id,
            0,
        )
        .expect("begin candidate validation");
        let geometry =
            validate_svg_import_geometry(&pending.bytes, 1.0, mappings.clone(), Some(candidate.id))
                .expect("validate selected polygon");
        let response = {
            let mut slot = lock_svg_import(&state).unwrap();
            complete_svg_import_settings_validation(
                &mut slot,
                &project,
                SvgImportSettingsValidationCompletion {
                    validation: SvgImportSettingsValidation {
                        validation_id,
                        import_id: pending.import_id,
                        expected_instance_id: pending.expected_instance_id,
                        expected_project_id: pending.expected_project_id,
                        expected_revision: pending.expected_revision,
                        millimeters_per_unit_bits: 1.0_f64.to_bits(),
                        boundary_candidate: Some(candidate.id),
                        group_mappings: mappings,
                    },
                    geometry,
                },
            )
            .expect("complete candidate validation")
        };

        assert_eq!(response.boundary_candidate_id, Some(candidate.id.0));
        assert_eq!((response.width_mm, response.height_mm), (100.0, 50.0));
        assert!(
            !response.has_cuts,
            "selected source edges become Boundary before effective Cut detection"
        );
    }

    #[test]
    fn svg_import_preview_preserves_every_boundary_candidate_origin() {
        let bytes = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"
                              fill="none" stroke="#111">
              <polygon points="0,0 10,0 10,10 0,10"/>
              <polyline points="20,0 30,0 30,10 20,10 20,0"/>
              <rect x="40" y="0" width="10" height="10"/>
              <path d="M 60 0 L 70 0 L 70 10 L 60 10 Z"/>
            </svg>"##;
        let preview = read_svg_preview(bytes).expect("read every candidate origin");
        let snapshot = svg_import_preview_snapshot(ProjectId::new(), &preview)
            .expect("build every candidate origin");
        let kinds = snapshot
            .boundary_candidates
            .iter()
            .map(|candidate| candidate.kind)
            .collect::<BTreeSet<_>>();

        assert_eq!(
            kinds,
            BTreeSet::from([
                "closed_path",
                "polygon",
                "polyline",
                "rectangle",
                "view_box"
            ])
        );
    }

    #[test]
    fn svg_import_settings_validation_rejects_stale_and_superseded_requests() {
        let bytes = br##"<svg xmlns="http://www.w3.org/2000/svg">
              <rect x="0" y="0" width="10" height="20"
                    fill="none" stroke="#222" data-origami-kind="boundary"/>
            </svg>"##;
        let preview = read_svg_preview(bytes).expect("read validation fixture");
        let mappings = preview
            .style_groups()
            .iter()
            .map(|group| SvgGroupMapping {
                group: group.id,
                target: SvgGroupTarget::Boundary,
            })
            .collect::<Vec<_>>();
        let state = SvgImportState::default();
        let project = initial_project_state();
        let import_id = stage_pending_svg_import(
            &state,
            project.instance_id,
            project.project_id,
            0,
            bytes.to_vec(),
        )
        .expect("stage validation fixture");

        assert!(
            begin_svg_import_settings_validation(
                &state,
                ProjectId::new(),
                ProjectId::new(),
                project.project_id,
                0,
            )
            .is_err()
        );
        assert!(
            begin_svg_import_settings_validation(
                &state,
                ProjectId::new(),
                import_id,
                project.project_id,
                1,
            )
            .is_err()
        );

        let first_validation_id = ProjectId::new();
        let first = begin_svg_import_settings_validation(
            &state,
            first_validation_id,
            import_id,
            project.project_id,
            0,
        )
        .expect("begin first generation");
        let first_geometry =
            validate_svg_import_geometry(&first.bytes, 1.0, mappings.clone(), None).unwrap();
        let second_validation_id = ProjectId::new();
        let second = begin_svg_import_settings_validation(
            &state,
            second_validation_id,
            import_id,
            project.project_id,
            0,
        )
        .expect("begin second generation");
        {
            let mut slot = lock_svg_import(&state).unwrap();
            assert!(
                complete_svg_import_settings_validation(
                    &mut slot,
                    &project,
                    SvgImportSettingsValidationCompletion {
                        validation: SvgImportSettingsValidation {
                            validation_id: first_validation_id,
                            import_id: first.import_id,
                            expected_instance_id: first.expected_instance_id,
                            expected_project_id: first.expected_project_id,
                            expected_revision: first.expected_revision,
                            millimeters_per_unit_bits: 1.0_f64.to_bits(),
                            boundary_candidate: None,
                            group_mappings: mappings.clone(),
                        },
                        geometry: first_geometry,
                    },
                )
                .is_err(),
                "late completion from the old generation must be rejected"
            );
        }
        let second_geometry =
            validate_svg_import_geometry(&second.bytes, 2.0, mappings.clone(), None).unwrap();
        {
            let mut slot = lock_svg_import(&state).unwrap();
            complete_svg_import_settings_validation(
                &mut slot,
                &project,
                SvgImportSettingsValidationCompletion {
                    validation: SvgImportSettingsValidation {
                        validation_id: second_validation_id,
                        import_id: second.import_id,
                        expected_instance_id: second.expected_instance_id,
                        expected_project_id: second.expected_project_id,
                        expected_revision: second.expected_revision,
                        millimeters_per_unit_bits: 2.0_f64.to_bits(),
                        boundary_candidate: None,
                        group_mappings: mappings.clone(),
                    },
                    geometry: second_geometry,
                },
            )
            .expect("complete current generation");
            let pending =
                pending_svg_import_in_slot(&slot, import_id, project.project_id, 0).unwrap();
            assert!(
                ensure_svg_import_settings_validation(
                    &slot,
                    pending,
                    first_validation_id,
                    None,
                    2.0,
                    &mappings,
                )
                .is_err()
            );
            assert!(
                ensure_svg_import_settings_validation(
                    &slot,
                    pending,
                    second_validation_id,
                    None,
                    1.0,
                    &mappings,
                )
                .is_err(),
                "a changed scale must not reuse old dimensions"
            );
            let mut changed_mappings = mappings.clone();
            changed_mappings[0].target = SvgGroupTarget::Ignore;
            assert!(
                ensure_svg_import_settings_validation(
                    &slot,
                    pending,
                    second_validation_id,
                    None,
                    2.0,
                    &changed_mappings,
                )
                .is_err(),
                "changed mappings must not reuse old dimensions"
            );
        }

        let replacement_id = stage_pending_svg_import(
            &state,
            project.instance_id,
            project.project_id,
            0,
            bytes.to_vec(),
        )
        .expect("stage a newer preview");
        let slot = lock_svg_import(&state).unwrap();
        assert_ne!(replacement_id, import_id);
        assert!(slot.validation.is_none());
        assert!(slot.validation_generation_id.is_none());
    }

    #[test]
    fn svg_import_settings_validation_rejects_a_project_revision_change_without_mutation() {
        let bytes = br##"<svg xmlns="http://www.w3.org/2000/svg">
              <rect x="0" y="0" width="10" height="20"
                    fill="none" stroke="#222" data-origami-kind="boundary"/>
            </svg>"##;
        let preview = read_svg_preview(bytes).expect("read stale revision fixture");
        let mappings = preview
            .style_groups()
            .iter()
            .map(|group| SvgGroupMapping {
                group: group.id,
                target: SvgGroupTarget::Boundary,
            })
            .collect::<Vec<_>>();
        let state = SvgImportState::default();
        let mut project = initial_project_state();
        let import_id = stage_pending_svg_import(
            &state,
            project.instance_id,
            project.project_id,
            0,
            bytes.to_vec(),
        )
        .expect("stage stale revision fixture");
        let validation_id = ProjectId::new();
        let pending = begin_svg_import_settings_validation(
            &state,
            validation_id,
            import_id,
            project.project_id,
            0,
        )
        .expect("begin stale revision validation");
        let geometry =
            validate_svg_import_geometry(&pending.bytes, 1.0, mappings.clone(), None).unwrap();
        execute_command(
            &mut project,
            pending.expected_project_id,
            0,
            Command::AddVertex {
                id: VertexId::new(),
                position: Point2::new(12.0, 34.0),
            },
        )
        .expect("change project after validation starts");
        let changed_project = project_state_signature(&project);

        {
            let mut slot = lock_svg_import(&state).unwrap();
            assert!(
                complete_svg_import_settings_validation(
                    &mut slot,
                    &project,
                    SvgImportSettingsValidationCompletion {
                        validation: SvgImportSettingsValidation {
                            validation_id,
                            import_id: pending.import_id,
                            expected_instance_id: pending.expected_instance_id,
                            expected_project_id: pending.expected_project_id,
                            expected_revision: pending.expected_revision,
                            millimeters_per_unit_bits: 1.0_f64.to_bits(),
                            boundary_candidate: None,
                            group_mappings: mappings,
                        },
                        geometry,
                    },
                )
                .is_err()
            );
            assert!(slot.validation.is_none());
            assert!(slot.pending.is_some());
        }
        abandon_svg_import_settings_validation(&state, validation_id)
            .expect("clear failed validation generation");
        assert_eq!(project_state_signature(&project), changed_project);
    }

    #[test]
    fn svg_import_settings_validation_rejects_invalid_boundaries_and_mappings() {
        let open = br##"<svg xmlns="http://www.w3.org/2000/svg" stroke="#111">
              <line x1="0" y1="0" x2="10" y2="0" data-origami-kind="boundary"/>
              <line x1="10" y1="0" x2="10" y2="10" data-origami-kind="boundary"/>
              <line x1="10" y1="10" x2="0" y2="10" data-origami-kind="boundary"/>
            </svg>"##;
        let open_preview = read_svg_preview(open).expect("read open boundary");
        let open_mappings = open_preview
            .style_groups()
            .iter()
            .map(|group| SvgGroupMapping {
                group: group.id,
                target: SvgGroupTarget::Boundary,
            })
            .collect();
        assert!(validate_svg_import_geometry(open, 1.0, open_mappings, None).is_err());

        let multiple = br##"<svg xmlns="http://www.w3.org/2000/svg" stroke="#111">
              <rect x="0" y="0" width="10" height="10"
                    fill="none" data-origami-kind="boundary"/>
              <rect x="20" y="0" width="10" height="10"
                    fill="none" data-origami-kind="boundary"/>
            </svg>"##;
        let multiple_preview = read_svg_preview(multiple).expect("read multiple boundaries");
        let multiple_mappings = multiple_preview
            .style_groups()
            .iter()
            .map(|group| SvgGroupMapping {
                group: group.id,
                target: SvgGroupTarget::Boundary,
            })
            .collect();
        assert!(validate_svg_import_geometry(multiple, 1.0, multiple_mappings, None).is_err());

        let valid = br##"<svg xmlns="http://www.w3.org/2000/svg" stroke="#111">
              <rect x="0" y="0" width="10" height="10"
                    fill="none" data-origami-kind="boundary"/>
              <line x1="0" y1="5" x2="10" y2="5" data-origami-kind="mountain"/>
            </svg>"##;
        let valid_preview = read_svg_preview(valid).expect("read complete mapping fixture");
        let boundary_only = valid_preview
            .style_groups()
            .iter()
            .filter(|group| group.semantic.as_deref() == Some("boundary"))
            .map(|group| SvgGroupMapping {
                group: group.id,
                target: SvgGroupTarget::Boundary,
            })
            .collect();
        assert!(
            validate_svg_import_geometry(valid, 1.0, boundary_only, None).is_err(),
            "every retained style group must be mapped"
        );
        assert!(validate_svg_import_geometry(valid, 0.0, Vec::new(), None).is_err());
    }

    #[test]
    fn svg_import_cancel_rejects_an_applied_token() {
        let state = SvgImportState::default();
        let mut project = initial_project_state();
        let expected_project_id = project.project_id;
        let expected_revision = project.editor.revision();
        let import_id = stage_pending_svg_import(
            &state,
            project.instance_id,
            expected_project_id,
            expected_revision,
            br#"<svg xmlns="http://www.w3.org/2000/svg"/>"#.to_vec(),
        )
        .expect("stage SVG import");
        {
            let mut slot = lock_svg_import(&state).expect("lock SVG stage");
            commit_svg_import_replacement(
                &mut project,
                &mut slot.pending,
                import_id,
                expected_project_id,
                expected_revision,
                true,
                create_new_project_state(new_project_parameters()).unwrap(),
            )
            .expect("apply SVG replacement");
        }
        assert!(cancel_pending_svg_import(&state, import_id).is_err());
    }

    #[test]
    fn fold_import_commit_is_an_atomic_new_unsaved_project_replacement() {
        let mut project = unsaved_project_with_redo_history("Existing project");
        let expected_instance_id = project.instance_id;
        let expected_project_id = project.project_id;
        let expected_revision = project.editor.revision();
        let import_id = ProjectId::new();
        let mut pending = Some(PendingFoldImport {
            import_id,
            expected_instance_id,
            expected_project_id,
            expected_revision,
            bytes: Arc::from(br#"{"file_spec":1.2}"#.as_slice()),
        });
        let replacement = create_new_project_state(new_project_parameters())
            .expect("create import replacement fixture");
        let replacement_project_id = replacement.project_id;
        let replacement_instance_id = replacement.instance_id;

        let response = commit_fold_import_replacement(
            &mut project,
            &mut pending,
            import_id,
            expected_project_id,
            expected_revision,
            replacement,
        )
        .expect("commit current import");

        assert_eq!(response.project_id, replacement_project_id);
        assert_eq!(project.instance_id, replacement_instance_id);
        assert_ne!(project.project_id, expected_project_id);
        assert_eq!(project.editor.revision(), 0);
        assert!(!project.editor.can_undo());
        assert!(!project.editor.can_redo());
        assert!(project.current_path.is_none());
        assert!(project.saved_revision.is_none());
        assert!(project.saved_document.is_none());
        assert!(project.is_dirty());
        assert!(pending.is_none());
    }

    #[test]
    fn svg_import_commit_is_an_atomic_new_unsaved_project_replacement() {
        let mut project = unsaved_project_with_redo_history("Existing project");
        let expected_instance_id = project.instance_id;
        let expected_project_id = project.project_id;
        let expected_revision = project.editor.revision();
        let import_id = ProjectId::new();
        let mut pending = Some(PendingSvgImport {
            import_id,
            expected_instance_id,
            expected_project_id,
            expected_revision,
            bytes: Arc::from(br#"<svg xmlns="http://www.w3.org/2000/svg"/>"#.as_slice()),
        });
        let replacement = create_new_project_state(new_project_parameters())
            .expect("create SVG import replacement fixture");

        let before = project_state_signature(&project);
        let error = commit_svg_import_replacement(
            &mut project,
            &mut pending,
            import_id,
            expected_project_id,
            expected_revision,
            false,
            replacement,
        )
        .expect_err("dirty SVG replacement must require confirmation");
        assert!(error.contains("explicit confirmation"));
        assert_eq!(project_state_signature(&project), before);
        assert!(pending.is_some());

        let replacement = create_new_project_state(new_project_parameters())
            .expect("create confirmed SVG import replacement fixture");
        let replacement_project_id = replacement.project_id;
        let replacement_instance_id = replacement.instance_id;
        let response = commit_svg_import_replacement(
            &mut project,
            &mut pending,
            import_id,
            expected_project_id,
            expected_revision,
            true,
            replacement,
        )
        .expect("commit current SVG import");

        assert_eq!(response.project_id, replacement_project_id);
        assert_eq!(project.instance_id, replacement_instance_id);
        assert_ne!(project.project_id, expected_project_id);
        assert_eq!(project.editor.revision(), 0);
        assert!(!project.editor.can_undo());
        assert!(!project.editor.can_redo());
        assert!(project.current_path.is_none());
        assert!(project.saved_revision.is_none());
        assert!(project.saved_document.is_none());
        assert!(project.is_dirty());
        assert!(pending.is_none());
    }

    #[test]
    fn svg_import_commit_rejects_revision_and_instance_aba_without_mutation() {
        let mut project = unsaved_project_with_redo_history("Existing project");
        let stale_instance_id = project.instance_id;
        let expected_project_id = project.project_id;
        let expected_revision = project.editor.revision();
        let import_id = ProjectId::new();
        let pending_template = PendingSvgImport {
            import_id,
            expected_instance_id: stale_instance_id,
            expected_project_id,
            expected_revision,
            bytes: Arc::from(br#"<svg xmlns="http://www.w3.org/2000/svg"/>"#.as_slice()),
        };

        project
            .editor
            .execute(
                expected_revision,
                Command::AddVertex {
                    id: VertexId::new(),
                    position: Point2::new(12.0, 13.0),
                },
            )
            .expect("edit after SVG preview");
        let revision_before = project_state_signature(&project);
        let mut pending = Some(pending_template.clone());
        let error = commit_svg_import_replacement(
            &mut project,
            &mut pending,
            import_id,
            expected_project_id,
            expected_revision,
            true,
            create_new_project_state(new_project_parameters()).unwrap(),
        )
        .expect_err("stale SVG revision must fail");
        assert_eq!(error, "the project changed while the file dialog was open");
        assert_eq!(project_state_signature(&project), revision_before);
        assert!(pending.is_some());

        let document = project.document();
        project = ProjectState::from_document(document, PathBuf::from("same.ori2"));
        project.project_id = expected_project_id;
        assert_ne!(project.instance_id, stale_instance_id);
        let instance_before = project_state_signature(&project);
        let mut pending = Some(pending_template);
        let error = commit_svg_import_replacement(
            &mut project,
            &mut pending,
            import_id,
            expected_project_id,
            expected_revision,
            true,
            create_new_project_state(new_project_parameters()).unwrap(),
        )
        .expect_err("reopened project instance must fail");
        assert_eq!(
            error,
            "the open project instance changed while the file dialog was open"
        );
        assert_eq!(project_state_signature(&project), instance_before);
        assert!(pending.is_some());
    }

    #[test]
    fn fold_import_commit_rejects_revision_and_instance_aba_without_mutation() {
        let mut project = unsaved_project_with_redo_history("Existing project");
        let stale_instance_id = project.instance_id;
        let expected_project_id = project.project_id;
        let expected_revision = project.editor.revision();
        let import_id = ProjectId::new();
        let pending_template = PendingFoldImport {
            import_id,
            expected_instance_id: stale_instance_id,
            expected_project_id,
            expected_revision,
            bytes: Arc::from(br#"{"file_spec":1.2}"#.as_slice()),
        };

        project
            .editor
            .execute(
                expected_revision,
                Command::AddVertex {
                    id: VertexId::new(),
                    position: Point2::new(12.0, 13.0),
                },
            )
            .expect("edit after preview");
        let revision_before = project_state_signature(&project);
        let mut pending = Some(pending_template.clone());
        let error = commit_fold_import_replacement(
            &mut project,
            &mut pending,
            import_id,
            expected_project_id,
            expected_revision,
            create_new_project_state(new_project_parameters()).unwrap(),
        )
        .expect_err("stale revision must fail");
        assert_eq!(error, "the project changed while the file dialog was open");
        assert_eq!(project_state_signature(&project), revision_before);
        assert!(pending.is_some());

        let document = project.document();
        project = ProjectState::from_document(document, PathBuf::from("same.ori2"));
        project.project_id = expected_project_id;
        assert_ne!(project.instance_id, stale_instance_id);
        let instance_before = project_state_signature(&project);
        let mut pending = Some(pending_template);
        let error = commit_fold_import_replacement(
            &mut project,
            &mut pending,
            import_id,
            expected_project_id,
            expected_revision,
            create_new_project_state(new_project_parameters()).unwrap(),
        )
        .expect_err("reopened project instance must fail");
        assert_eq!(
            error,
            "the open project instance changed while the file dialog was open"
        );
        assert_eq!(project_state_signature(&project), instance_before);
        assert!(pending.is_some());
    }

    #[test]
    fn fold_import_mapping_and_scale_validation_reject_ambiguous_requests() {
        assert!(validate_import_scale(1.0).is_ok());
        for invalid in [0.0, -1.0, f64::NAN, f64::INFINITY, 1_000_000_000.000_001] {
            assert!(validate_import_scale(invalid).is_err());
        }

        let valid = validate_fold_import_mapping_requests(vec![
            FoldImportAssignmentMappingRequest {
                source: "M".to_owned(),
                target: FoldImportTargetRequest::Mountain,
            },
            FoldImportAssignmentMappingRequest {
                source: "U".to_owned(),
                target: FoldImportTargetRequest::Valley,
            },
            FoldImportAssignmentMappingRequest {
                source: "J".to_owned(),
                target: FoldImportTargetRequest::Ignore,
            },
        ])
        .expect("validate supported mappings");
        assert_eq!(valid.len(), 3);

        assert!(
            validate_fold_import_mapping_requests(vec![FoldImportAssignmentMappingRequest {
                source: "M".to_owned(),
                target: FoldImportTargetRequest::Valley,
            }])
            .is_err()
        );
        assert!(
            validate_fold_import_mapping_requests(vec![FoldImportAssignmentMappingRequest {
                source: "X".to_owned(),
                target: FoldImportTargetRequest::Ignore,
            }])
            .is_err()
        );
        assert!(
            validate_fold_import_mapping_requests(vec![
                FoldImportAssignmentMappingRequest {
                    source: "F".to_owned(),
                    target: FoldImportTargetRequest::Auxiliary,
                },
                FoldImportAssignmentMappingRequest {
                    source: "F".to_owned(),
                    target: FoldImportTargetRequest::Ignore,
                },
            ])
            .is_err()
        );
    }

    #[test]
    fn fold_import_mapping_or_geometry_failure_preserves_project_and_pending_preview() {
        let project = unsaved_project_with_redo_history("Keep this project");
        let before = project_state_signature(&project);
        let valid_bytes = serde_json::to_vec(&serde_json::json!({
            "file_spec": 1.2,
            "frame_unit": "mm",
            "vertices_coords": [[0.0, 0.0], [2.0, 0.0], [2.0, 2.0], [0.0, 2.0]],
            "edges_vertices": [[0, 1], [1, 2], [2, 3], [3, 0], [0, 2]],
            "edges_assignment": ["B", "B", "B", "B", "M"]
        }))
        .expect("serialize mapping fixture");
        let import_id = ProjectId::new();
        let mut pending = Some(PendingFoldImport {
            import_id,
            expected_instance_id: project.instance_id,
            expected_project_id: project.project_id,
            expected_revision: project.editor.revision(),
            bytes: Arc::from(valid_bytes.clone()),
        });

        let mapping_error = build_fold_import_replacement(
            &valid_bytes,
            "Missing mapping".to_owned(),
            1.0,
            FoldBoundaryCandidateId(0),
            HashMap::new(),
        )
        .err()
        .expect("missing M mapping must fail");
        assert!(mapping_error.contains("no mapping was selected"));
        assert_eq!(project_state_signature(&project), before);
        assert_eq!(
            pending.as_ref().map(|value| value.import_id),
            Some(import_id)
        );

        let crossing_bytes = serde_json::to_vec(&serde_json::json!({
            "file_spec": 1.2,
            "frame_unit": "mm",
            "vertices_coords": [[0.0, 0.0], [2.0, 0.0], [2.0, 2.0], [0.0, 2.0]],
            "edges_vertices": [
                [0, 1], [1, 2], [2, 3], [3, 0],
                [0, 2], [1, 3]
            ],
            "edges_assignment": ["B", "B", "B", "B", "M", "V"]
        }))
        .expect("serialize crossing fixture");
        let geometry_error = build_fold_import_replacement(
            &crossing_bytes,
            "Crossing".to_owned(),
            1.0,
            FoldBoundaryCandidateId(0),
            HashMap::from([
                ("M".to_owned(), FoldImportTargetRequest::Mountain),
                ("V".to_owned(), FoldImportTargetRequest::Valley),
            ]),
        )
        .err()
        .expect("unsplit crossing must fail final validation");
        assert!(geometry_error.contains("validation issue(s)"));
        assert_eq!(project_state_signature(&project), before);
        assert_eq!(
            pending.as_ref().map(|value| value.import_id),
            Some(import_id)
        );

        let replacement =
            create_new_project_state(new_project_parameters()).expect("create unused replacement");
        // The failed conversion path never reaches the only replacement
        // boundary; retaining this assertion guards accidental future calls.
        assert_ne!(replacement.project_id, project.project_id);
        assert!(pending.take().is_some());
        assert_eq!(project_state_signature(&project), before);
    }

    #[test]
    fn fold_import_file_errors_do_not_expose_the_selected_path() {
        let directory = TestDirectory::new();
        let secret_name = "private-client-design.fold";
        let path = directory.join(secret_name);

        let missing_error =
            read_fold_import_bytes(&path).expect_err("missing import must be rejected");
        assert_eq!(missing_error, FOLD_FILE_OPEN_FAILED_MESSAGE);
        assert!(!missing_error.contains(secret_name));
        assert!(!missing_error.contains(&directory.path.to_string_lossy().into_owned()));
        assert!(!missing_error.to_ascii_lowercase().contains("os error"));

        let private_file_spec = 987_654_321.125_f64;
        let private_value = private_file_spec.to_string();
        let malformed = serde_json::to_vec(&serde_json::json!({
            "file_spec": private_file_spec,
            "vertices_coords": [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
            "edges_vertices": [[0, 1], [1, 2], [2, 3], [3, 0]],
            "edges_assignment": ["B", "B", "B", "B"]
        }))
        .expect("serialize private malformed FOLD fixture");
        fs::write(&path, &malformed).expect("write malformed FOLD fixture");
        let malformed_error =
            load_fold_import_preview(&path).expect_err("unsupported FOLD version must be rejected");
        assert_eq!(malformed_error, FOLD_FILE_INVALID_MESSAGE);
        assert!(!malformed_error.contains(&private_value));
        assert!(!malformed_error.contains(secret_name));

        let staged_error = build_fold_import_replacement(
            &malformed,
            "Private staged input".to_owned(),
            1.0,
            FoldBoundaryCandidateId(0),
            HashMap::new(),
        )
        .err()
        .expect("staged unsupported FOLD version must be rejected");
        assert_eq!(staged_error, FOLD_FILE_INVALID_MESSAGE);
        assert!(!staged_error.contains(&private_value));

        File::create(&path)
            .expect("create oversized fixture")
            .set_len(MAX_FOLD_IMPORT_FILE_SIZE + 1)
            .expect("make sparse oversized fixture");
        let oversized_error =
            read_fold_import_bytes(&path).expect_err("oversized import must be rejected");
        assert_eq!(oversized_error, FOLD_FILE_TOO_LARGE_MESSAGE);
        assert!(!oversized_error.contains(secret_name));
        assert!(!oversized_error.contains(&directory.path.to_string_lossy().into_owned()));
        assert!(!oversized_error.contains(&(MAX_FOLD_IMPORT_FILE_SIZE + 1).to_string()));
    }

    #[test]
    fn fold_import_preview_contract_and_conversion_create_a_valid_editable_project() {
        let bytes = serde_json::to_vec(&serde_json::json!({
            "file_spec": 1.2,
            "file_title": "  取込テスト  ",
            "frame_unit": "cm",
            "vertices_coords": [[0.0, 0.0], [2.0, 0.0], [2.0, 2.0], [0.0, 2.0]],
            "edges_vertices": [[0, 1], [1, 2], [2, 3], [3, 0], [0, 2]],
            "edges_assignment": ["B", "B", "B", "B", "M"]
        }))
        .expect("serialize FOLD fixture");
        let preview = read_fold_preview(&bytes).expect("read FOLD preview");
        let import_id = ProjectId::new();
        let response = fold_import_preview_snapshot(import_id, &preview);

        assert_eq!(response.import_id, import_id);
        assert_eq!(response.file_name, FOLD_IMPORT_FILE_LABEL);
        assert_eq!(response.suggested_name, "取込テスト");
        assert_eq!(response.file_spec.as_deref(), Some("1.2"));
        assert_eq!(response.frame_unit.as_deref(), Some("cm"));
        assert_eq!(response.default_mm_per_unit, Some(10.0));
        assert_eq!(response.vertex_count, 4);
        assert_eq!(response.edge_count, 5);
        assert_eq!(response.boundary_edge_count, 4);
        assert_eq!(response.fixed_boundary_candidate_id, Some(0));
        assert_eq!(
            response.boundary_candidates,
            vec![FoldImportBoundaryCandidateSnapshot {
                id: 0,
                source: "assigned_boundary",
                edge_indices: vec![0, 1, 2, 3],
            }]
        );
        assert_eq!(
            response.assignments,
            vec![
                FoldImportAssignmentSummary {
                    assignment: "B".to_owned(),
                    count: 4,
                },
                FoldImportAssignmentSummary {
                    assignment: "M".to_owned(),
                    count: 1,
                },
            ]
        );
        assert_eq!(response.preview_vertices.len(), 4);
        assert_eq!(response.preview_edges.len(), 5);
        assert_eq!(
            response
                .preview_edges
                .iter()
                .map(|edge| edge.source_index)
                .collect::<Vec<_>>(),
            vec![0, 1, 2, 3, 4]
        );
        assert!(!response.preview_truncated);
        assert!(response.warnings.is_empty());

        let replacement = build_fold_import_replacement(
            &bytes,
            "取込テスト".to_owned(),
            10.0,
            FoldBoundaryCandidateId(0),
            HashMap::from([("M".to_owned(), FoldImportTargetRequest::Mountain)]),
        )
        .expect("convert FOLD into a project");
        assert_eq!(replacement.name, "取込テスト");
        assert_eq!(replacement.editor.pattern().vertices.len(), 4);
        assert_eq!(replacement.editor.pattern().edges.len(), 5);
        assert_eq!(replacement.editor.paper().boundary_vertices.len(), 4);
        assert!(
            replacement
                .editor
                .pattern()
                .vertices
                .iter()
                .any(|vertex| vertex.position == Point2::new(20.0, 20.0))
        );
        assert_eq!(
            replacement
                .editor
                .pattern()
                .edges
                .iter()
                .filter(|edge| edge.kind == EdgeKind::Mountain)
                .count(),
            1
        );
        assert!(!replacement.editor.paper().cutting_allowed);
        assert!(replacement.editor.instruction_timeline().steps.is_empty());
        assert_eq!(replacement.editor.revision(), 0);
        assert!(!replacement.editor.can_undo());
        assert!(!replacement.editor.can_redo());
        assert!(replacement.current_path.is_none());
        assert!(replacement.saved_document.is_none());
        assert!(replacement.is_dirty());
    }

    #[test]
    fn fold_import_requires_and_revalidates_an_inferred_boundary_choice() {
        let bytes = serde_json::to_vec(&serde_json::json!({
            "file_spec": 1.2,
            "frame_unit": "mm",
            "vertices_coords": [[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0]],
            "edges_vertices": [[0, 1], [1, 2], [2, 3], [3, 0], [0, 2]]
        }))
        .expect("serialize assignment-free FOLD fixture");
        let preview = read_fold_preview(&bytes).expect("read assignment-free FOLD preview");
        let response = fold_import_preview_snapshot(ProjectId::new(), &preview);

        assert_eq!(response.fixed_boundary_candidate_id, None);
        assert_eq!(response.boundary_candidates.len(), 1);
        let candidate = &response.boundary_candidates[0];
        assert_eq!(candidate.source, "inferred_outer_face");
        assert_eq!(candidate.edge_indices, vec![0, 1, 2, 3]);
        assert!(
            response
                .preview_edges
                .iter()
                .filter(|edge| candidate.edge_indices.contains(&edge.source_index))
                .all(|edge| edge.assignment == "U")
        );

        let replacement = build_fold_import_replacement(
            &bytes,
            "外周候補を選択".to_owned(),
            1.0,
            FoldBoundaryCandidateId(candidate.id),
            HashMap::from([("U".to_owned(), FoldImportTargetRequest::Auxiliary)]),
        )
        .expect("convert with the explicitly selected candidate");
        assert_eq!(replacement.editor.paper().boundary_vertices.len(), 4);
        assert_eq!(
            replacement
                .editor
                .pattern()
                .edges
                .iter()
                .filter(|edge| edge.kind == EdgeKind::Boundary)
                .count(),
            4
        );
        assert_eq!(
            replacement
                .editor
                .pattern()
                .edges
                .iter()
                .filter(|edge| edge.kind == EdgeKind::Auxiliary)
                .count(),
            1
        );

        let stale_error = match build_fold_import_replacement(
            &bytes,
            "存在しない候補".to_owned(),
            1.0,
            FoldBoundaryCandidateId(candidate.id.saturating_add(1)),
            HashMap::from([("U".to_owned(), FoldImportTargetRequest::Auxiliary)]),
        ) {
            Ok(_) => panic!("an absent candidate ID must be rejected after reparsing"),
            Err(error) => error,
        };
        assert!(stale_error.contains("is not present in this preview"));
    }

    #[test]
    fn fold_import_rejects_an_active_edge_outside_the_paper() {
        let bytes = serde_json::to_vec(&serde_json::json!({
            "file_spec": 1.2,
            "frame_unit": "mm",
            "vertices_coords": [
                [0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0],
                [2.0, 0.0], [2.0, 1.0]
            ],
            "edges_vertices": [[0, 1], [1, 2], [2, 3], [3, 0], [4, 5]],
            "edges_assignment": ["B", "B", "B", "B", "M"]
        }))
        .expect("serialize outside-edge fixture");

        let error = build_fold_import_replacement(
            &bytes,
            "紙外の折り線".to_owned(),
            1.0,
            FoldBoundaryCandidateId(0),
            HashMap::from([("M".to_owned(), FoldImportTargetRequest::Mountain)]),
        )
        .err()
        .expect("an active edge outside the paper must be rejected");

        assert!(error.contains("active edge(s) outside the paper boundary"));
    }

    #[test]
    fn fold_import_applies_valley_cut_and_ignore_mapping_with_scale() {
        let bytes = serde_json::to_vec(&serde_json::json!({
            "file_spec": 1.2,
            "frame_unit": "unit",
            "vertices_coords": [
                [0.0, 0.0], [2.0, 0.0], [4.0, 0.0],
                [4.0, 4.0], [2.0, 4.0], [0.0, 4.0]
            ],
            "edges_vertices": [
                [0, 1], [1, 2], [2, 3], [3, 4], [4, 5], [5, 0],
                [0, 3], [0, 4], [1, 3], [2, 5]
            ],
            "edges_assignment": ["B", "B", "B", "B", "B", "B", "M", "V", "C", "F"]
        }))
        .expect("serialize mapped FOLD fixture");
        let replacement = build_fold_import_replacement(
            &bytes,
            "複数線種".to_owned(),
            2.5,
            FoldBoundaryCandidateId(0),
            HashMap::from([
                ("M".to_owned(), FoldImportTargetRequest::Mountain),
                ("V".to_owned(), FoldImportTargetRequest::Valley),
                ("C".to_owned(), FoldImportTargetRequest::Cut),
                ("F".to_owned(), FoldImportTargetRequest::Ignore),
            ]),
        )
        .expect("convert explicit mapped assignments");
        let edges = &replacement.editor.pattern().edges;

        assert_eq!(edges.len(), 9);
        assert_eq!(
            edges
                .iter()
                .filter(|edge| edge.kind == EdgeKind::Boundary)
                .count(),
            6
        );
        assert_eq!(
            edges
                .iter()
                .filter(|edge| edge.kind == EdgeKind::Mountain)
                .count(),
            1
        );
        assert_eq!(
            edges
                .iter()
                .filter(|edge| edge.kind == EdgeKind::Valley)
                .count(),
            1
        );
        assert_eq!(
            edges
                .iter()
                .filter(|edge| edge.kind == EdgeKind::Cut)
                .count(),
            1
        );
        assert!(replacement.editor.paper().cutting_allowed);
        assert!(
            replacement
                .editor
                .pattern()
                .vertices
                .iter()
                .any(|vertex| vertex.position == Point2::new(10.0, 10.0))
        );
    }

    #[test]
    fn fold_import_preview_truncation_remaps_every_rendered_endpoint() {
        let interior_edge_count = MAX_FOLD_IMPORT_PREVIEW_EDGES - 3;
        let mut vertices = vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
        let mut edges = Vec::new();
        let mut assignments = Vec::new();
        for index in 0..interior_edge_count {
            let x = 10.0 + index as f64;
            let start = vertices.len();
            vertices.push([x, 2.0]);
            vertices.push([x, 3.0]);
            edges.push([start, start + 1]);
            assignments.push("F");
        }
        edges.extend([[0_usize, 1_usize], [1, 2], [2, 3], [3, 0]]);
        assignments.extend(["B"; 4]);
        let bytes = serde_json::to_vec(&serde_json::json!({
            "vertices_coords": vertices,
            "edges_vertices": edges,
            "edges_assignment": assignments,
            "file_classes": ["singleModel"]
        }))
        .expect("serialize large preview fixture");
        let preview = read_fold_preview(&bytes).expect("read large preview");
        let response = fold_import_preview_snapshot(ProjectId::new(), &preview);

        assert!(response.preview_truncated);
        assert_eq!(response.preview_edges.len(), MAX_FOLD_IMPORT_PREVIEW_EDGES);
        assert!(response.preview_vertices.len() < response.vertex_count);
        assert!(response.preview_edges.iter().all(|edge| {
            edge.start < response.preview_vertices.len()
                && edge.end < response.preview_vertices.len()
        }));
        assert_eq!(
            response
                .preview_edges
                .iter()
                .filter(|edge| edge.assignment == "B")
                .count(),
            4
        );
        assert_eq!(
            response
                .assignments
                .iter()
                .map(|summary| summary.assignment.as_str())
                .collect::<Vec<_>>(),
            vec!["B", "F"]
        );
        assert!(response.warnings.iter().all(|warning| !warning.is_ascii()));
        assert!(
            response
                .warnings
                .iter()
                .any(|warning| warning.contains("ファイル分類"))
        );
    }

    #[test]
    fn svg_import_file_errors_do_not_expose_the_selected_path() {
        let directory = TestDirectory::new();
        let secret_name = "private-client-design.svg";
        let path = directory.join(secret_name);

        let missing_error =
            read_svg_import_bytes(&path).expect_err("missing SVG import must be rejected");
        assert_eq!(missing_error, SVG_FILE_OPEN_FAILED_MESSAGE);
        assert!(!missing_error.contains(secret_name));
        assert!(!missing_error.contains(&directory.path.to_string_lossy().into_owned()));
        assert!(!missing_error.to_ascii_lowercase().contains("os error"));

        fs::write(
            &path,
            br#"<svg xmlns="http://www.w3.org/2000/svg"><SECRET_MARKER></OTHER_SECRET></svg>"#,
        )
        .expect("write malformed SVG fixture");
        let malformed_error =
            load_svg_import_preview(&path).expect_err("malformed SVG import must be rejected");
        assert_eq!(malformed_error, SVG_FILE_INVALID_MESSAGE);
        assert!(!malformed_error.contains("SECRET_MARKER"));
        assert!(!malformed_error.contains("OTHER_SECRET"));
        assert!(!malformed_error.contains(secret_name));

        File::create(&path)
            .expect("create oversized SVG fixture")
            .set_len(MAX_SVG_IMPORT_FILE_SIZE + 1)
            .expect("make sparse oversized SVG fixture");
        let oversized_error =
            read_svg_import_bytes(&path).expect_err("oversized SVG import must be rejected");
        assert_eq!(oversized_error, SVG_FILE_TOO_LARGE_MESSAGE);
        assert!(!oversized_error.contains(secret_name));
        assert!(!oversized_error.contains(&directory.path.to_string_lossy().into_owned()));
        assert!(!oversized_error.contains(&(MAX_SVG_IMPORT_FILE_SIZE + 1).to_string()));
    }

    #[test]
    fn svg_import_warning_messages_do_not_echo_source_style_values() {
        for kind in [
            SvgWarningKind::UnsupportedCssSelector("#SECRET_SELECTOR".to_owned()),
            SvgWarningKind::UnsupportedPaint("url(SECRET_PAINT)".to_owned()),
            SvgWarningKind::UnsupportedLengthUnit("SECRET_LENGTH".to_owned()),
        ] {
            let message = svg_import_warning_message(&SvgPreviewWarning {
                kind,
                occurrences: 1,
            });
            assert!(!message.contains("SECRET"));
        }

        let source = br##"<svg xmlns="http://www.w3.org/2000/svg"
                              viewBox="0 0 10 10" width="10mm" height="10mm"
                              fill="none">
              <line stroke="#111111" stroke-linecap="SECRET_LINE_CAP"
                    x1="0" y1="0" x2="10" y2="10"/>
            </svg>"##;
        let preview = read_svg_preview(source).expect("parse unknown line-cap fixture");
        assert_eq!(
            preview.warnings(),
            &[SvgPreviewWarning {
                kind: SvgWarningKind::UnsupportedAttribute("stroke-linecap".to_owned()),
                occurrences: 1,
            }]
        );
        let response = svg_import_preview_snapshot(ProjectId::new(), &preview)
            .expect("build unknown line-cap snapshot");
        let encoded = serde_json::to_string(&response).expect("serialize SVG preview snapshot");
        assert!(!encoded.contains("SECRET"));
        assert!(!encoded.contains("LINE_CAP"));
    }

    #[test]
    fn svg_import_preview_contract_and_conversion_create_a_valid_editable_project() {
        let source = r##"<?xml version="1.0" encoding="UTF-8"?>
            <svg xmlns="http://www.w3.org/2000/svg"
                 viewBox="0 0 100 100" width="100mm" height="100mm">
              <title>  SVG取込テスト  </title>
              <rect x="0" y="0" width="100" height="100"
                    fill="none" stroke="#222222" data-origami-kind="boundary"/>
              <line id="main-fold" x1="0" y1="0" x2="100" y2="100"
                    stroke="#cc3344" stroke-linecap="round"
                    data-origami-kind="mountain"/>
            </svg>"##;
        let bytes = source.as_bytes();
        let preview = read_svg_preview(bytes).expect("read SVG preview");
        let import_id = ProjectId::new();
        let response =
            svg_import_preview_snapshot(import_id, &preview).expect("build bounded SVG preview");

        assert_eq!(response.import_id, import_id);
        assert_eq!(response.file_name, SVG_IMPORT_FILE_LABEL);
        assert_eq!(response.suggested_name, "SVG取込テスト");
        assert_eq!(response.default_mm_per_unit, Some(1.0));
        assert_eq!(
            response.root_view_box,
            Some(SvgRootViewBox {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
            })
        );
        assert_eq!(response.root_physical_size.width_millimetres, Some(100.0));
        assert_eq!(response.root_physical_size.height_millimetres, Some(100.0));
        assert_eq!(response.source_segment_count, 5);
        assert_eq!(response.style_groups.len(), 2);
        assert!(response.style_groups.iter().all(|group| {
            group.element_count > 0
                && group.segment_count > 0
                && matches!(
                    group.line_cap,
                    SvgLineCap::Butt | SvgLineCap::Round | SvgLineCap::Square
                )
                && group
                    .stroke_color
                    .as_deref()
                    .is_some_and(|color| color.starts_with('#'))
        }));
        let main_fold_group = response
            .style_groups
            .iter()
            .find(|group| group.representative_id.as_deref() == Some("main-fold"))
            .expect("main fold style group");
        assert_eq!(main_fold_group.element_count, 1);
        assert_eq!(main_fold_group.segment_count, 1);
        assert_eq!(main_fold_group.line_cap, SvgLineCap::Round);
        assert_eq!(
            serde_json::to_value(main_fold_group)
                .expect("serialize SVG style group snapshot")
                .get("line_cap")
                .and_then(serde_json::Value::as_str),
            Some("round")
        );
        assert_eq!(response.preview_edges.len(), 5);
        assert!(!response.preview_truncated);
        assert!(response.preview_edges.iter().all(|edge| {
            edge.start < response.preview_vertices.len()
                && edge.end < response.preview_vertices.len()
        }));
        assert!(
            response
                .boundary_candidates
                .iter()
                .any(|candidate| candidate.kind == "view_box")
        );
        assert!(
            response
                .boundary_candidates
                .iter()
                .any(|candidate| candidate.kind == "rectangle")
        );
        assert!(response.boundary_candidates.iter().all(|candidate| {
            candidate.segment_count == candidate.vertices.len() && candidate.segment_count >= 3
        }));
        assert!(
            response
                .warnings
                .iter()
                .any(|warning| warning.contains("data-origami-kind"))
        );

        let rectangle = preview
            .boundary_candidates()
            .iter()
            .find(|candidate| candidate.kind == SvgBoundaryCandidateKind::Rectangle)
            .expect("rectangle boundary candidate");
        let mappings: Vec<SvgGroupMapping> = preview
            .style_groups()
            .iter()
            .map(|group| SvgGroupMapping {
                group: group.id,
                target: match group.semantic.as_deref() {
                    Some("mountain") => SvgGroupTarget::Mountain,
                    _ => SvgGroupTarget::Ignore,
                },
            })
            .collect();
        let boundary_error = build_svg_import_replacement(
            bytes,
            SvgImportReplacementOptions {
                name: "SVG取込テスト".to_owned(),
                millimeters_per_unit: 1.0,
                group_mappings: mappings.clone(),
                boundary_candidate: Some(rectangle.id),
                boundary_confirmed: false,
                warnings_acknowledged: true,
                cutting_allowed_confirmed: false,
            },
        )
        .err()
        .expect("boundary must require explicit confirmation");
        assert!(boundary_error.contains("boundary must be explicitly confirmed"));
        let warning_error = build_svg_import_replacement(
            bytes,
            SvgImportReplacementOptions {
                name: "SVG取込テスト".to_owned(),
                millimeters_per_unit: 1.0,
                group_mappings: mappings.clone(),
                boundary_candidate: Some(rectangle.id),
                boundary_confirmed: true,
                warnings_acknowledged: false,
                cutting_allowed_confirmed: false,
            },
        )
        .err()
        .expect("warnings must require explicit confirmation");
        assert!(warning_error.contains("warnings must be explicitly acknowledged"));
        let replacement = build_svg_import_replacement(
            bytes,
            SvgImportReplacementOptions {
                name: "SVG取込テスト".to_owned(),
                millimeters_per_unit: 1.0,
                group_mappings: mappings,
                boundary_candidate: Some(rectangle.id),
                boundary_confirmed: true,
                warnings_acknowledged: true,
                cutting_allowed_confirmed: false,
            },
        )
        .expect("convert SVG into a project");

        assert_eq!(replacement.name, "SVG取込テスト");
        assert_eq!(replacement.editor.pattern().edges.len(), 5);
        assert_eq!(replacement.editor.paper().boundary_vertices.len(), 4);
        assert_eq!(
            replacement
                .editor
                .pattern()
                .edges
                .iter()
                .filter(|edge| edge.kind == EdgeKind::Mountain)
                .count(),
            1
        );
        assert!(!replacement.editor.paper().cutting_allowed);
        assert!(replacement.editor.instruction_timeline().steps.is_empty());
        assert_eq!(replacement.editor.revision(), 0);
        assert!(!replacement.editor.can_undo());
        assert!(!replacement.editor.can_redo());
        assert!(replacement.current_path.is_none());
        assert!(replacement.saved_document.is_none());
        assert!(replacement.is_dirty());
    }

    #[test]
    fn svg_import_preview_rejects_more_than_sixty_four_warning_categories() {
        let mut source = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"
                     width="100mm" height="100mm" fill="none" stroke="#111">
                   <title>{}</title>"##,
            "a".repeat(MAX_PROJECT_NAME_CHARS + 1)
        );
        for index in 0..63 {
            let class = if index == 0 { r#" class="fold""# } else { "" };
            source.push_str(&format!(
                r#"<line{class} unsupported{index}="x" x1="0" y1="{index}" x2="1" y2="{index}"/>"#
            ));
        }
        source.push_str("</svg>");

        let preview = read_svg_preview(source.as_bytes()).expect("bounded warning fixture");
        assert_eq!(preview.warnings().len(), 63);
        let error = svg_import_preview_snapshot(ProjectId::new(), &preview)
            .expect_err("synthetic warning categories must not be truncated");
        assert!(error.contains("more than 64"));
    }

    #[test]
    fn svg_cut_mapping_requires_explicit_permission_and_splits_crossings() {
        let bytes = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
              <rect x="0" y="0" width="100" height="100"
                    fill="none" stroke="#222" data-origami-kind="boundary"/>
              <line x1="0" y1="0" x2="100" y2="100"
                    stroke="#c33" data-origami-kind="mountain"/>
              <line x1="0" y1="50" x2="100" y2="50"
                    stroke="#111" data-origami-kind="cut"/>
            </svg>"##;
        let preview = read_svg_preview(bytes).expect("read cut SVG preview");
        let rectangle = preview
            .boundary_candidates()
            .iter()
            .find(|candidate| candidate.kind == SvgBoundaryCandidateKind::Rectangle)
            .expect("rectangle boundary candidate");
        let mappings = preview
            .style_groups()
            .iter()
            .map(|group| SvgGroupMapping {
                group: group.id,
                target: match group.semantic.as_deref() {
                    Some("mountain") => SvgGroupTarget::Mountain,
                    Some("cut") => SvgGroupTarget::Cut,
                    _ => SvgGroupTarget::Ignore,
                },
            })
            .collect::<Vec<_>>();

        let error = build_svg_import_replacement(
            bytes,
            SvgImportReplacementOptions {
                name: "切断確認".to_owned(),
                millimeters_per_unit: 1.0,
                group_mappings: mappings.clone(),
                boundary_candidate: Some(rectangle.id),
                boundary_confirmed: true,
                warnings_acknowledged: true,
                cutting_allowed_confirmed: false,
            },
        )
        .err()
        .expect("cutting must require explicit confirmation");
        assert!(error.contains("cutting must be explicitly allowed"));

        let replacement = build_svg_import_replacement(
            bytes,
            SvgImportReplacementOptions {
                name: "切断確認".to_owned(),
                millimeters_per_unit: 1.0,
                group_mappings: mappings,
                boundary_candidate: Some(rectangle.id),
                boundary_confirmed: true,
                warnings_acknowledged: true,
                cutting_allowed_confirmed: true,
            },
        )
        .expect("confirmed cut SVG must convert");
        let edges = &replacement.editor.pattern().edges;
        assert!(replacement.editor.paper().cutting_allowed);
        assert_eq!(
            edges
                .iter()
                .filter(|edge| edge.kind == EdgeKind::Mountain)
                .count(),
            2,
            "the mountain line must split at the X intersection"
        );
        assert_eq!(
            edges
                .iter()
                .filter(|edge| edge.kind == EdgeKind::Cut)
                .count(),
            2,
            "the cut line must split at the X intersection"
        );
        assert!(
            replacem