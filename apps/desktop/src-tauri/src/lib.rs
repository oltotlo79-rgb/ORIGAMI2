#![deny(unsafe_op_in_unsafe_fn)]
#![allow(
    clippy::clone_on_copy,
    clippy::collapsible_else_if,
    clippy::collapsible_if,
    clippy::large_enum_variant,
    clippy::needless_borrow,
    clippy::needless_range_loop,
    clippy::too_many_arguments,
    clippy::type_complexity,
    clippy::unwrap_or_default
)]

mod applied_pose;
mod beginner_design_commands;
mod beginner_recognition;
mod crease_export;
mod crease_pattern_boundary_support;
mod current_non_flat_layer_order_view;
mod diagnostics;
mod fold_3d_frames_import;
mod fold_import_commands;
mod fold_technique_file_io;
mod geometric_constraint_analysis;
mod geometric_constraint_commands;
mod global_flat_foldability;
mod history_settings;
mod import_command_support;
mod instruction_export;
mod mesh_animation_export;
mod mesh_export;
mod numeric_expression;
mod pattern_edit_commands;
mod project_folder_io;
mod project_lifecycle_commands;
mod project_persistence;
#[allow(dead_code)]
mod recent_projects;
#[allow(dead_code)]
mod recovery;
mod runtime_update;
mod save_path;
mod stacked_fold_read;
mod stacked_fold_transaction;
mod svg_import_commands;
#[cfg(test)]
use beginner_design_commands::{
    BeginnerGridWork, BeginnerReferenceModelSuggestionV1, BeginnerReferenceSurfaceAssignmentV1,
    BeginnerReferenceSurfaceEditV1, MAX_BEGINNER_FOLDED_LANDMARKS_V1, ReferenceConsensusWorkV1,
    apply_beginner_generated_plan_document, apply_grid_plan_document,
    assess_beginner_generated_plan, assess_beginner_generated_plan_with_deadline,
    beginner_contour_placement_witness, beginner_grid_work, bounded_folded_pose_landmark_score_v1,
    certify_beginner_fold_path_v1, configure_symmetric_profile,
    derive_reference_model_suggestion_v1, disconnected_glb_stick_tree_v1, grid_template_plan,
    normalized_contour_error_millionths, preset_weighted_refinement_score_v1,
    reference_consensus_work_v1, reference_model_suggestion_matches_live_v1,
    reference_model_surface_range_is_connected_v1,
    reference_model_surface_selection_matches_live_v1, temporary_symmetric_profile_for_grid,
    validate_beginner_manufacturability_v1,
};
use beginner_design_commands::{
    activate_beginner_reference_model_asset, apply_beginner_generated_plan,
    apply_beginner_parameter_grid_candidate, apply_beginner_reference_model_features,
    apply_beginner_symmetric_parameters, archive_beginner_reference_model_asset,
    cancel_beginner_parameter_grid, cancel_reference_consensus, evaluate_beginner_candidates,
    evaluate_beginner_parameter_grid, get_beginner_parameter_grid_progress,
    get_beginner_reference_model_geometry, get_beginner_symmetric_parameter_estimate,
    import_beginner_reference_model, suggest_beginner_reference_model_features,
    update_beginner_design_profile, update_beginner_reference_consensus,
};
use beginner_recognition::{
    apply_beginner_outline_candidate, apply_beginner_part_assignments,
    recognize_beginner_outline_candidates, recognize_beginner_part_suggestions,
    recognize_beginner_silhouette, recognize_beginner_target,
};
#[cfg(test)]
use pattern_edit_commands::{
    BenchmarkEdge, BenchmarkVertex, LinearArrayRequestV1, RadialArrayRequestV1,
    confirm_linear_array_inner, confirm_radial_array_inner, execute_boundary_split,
    execute_edge_intersection_connection, execute_edge_split,
    execute_intersection_cluster_connection, execute_t_junction_connection,
    linear_array_request_sha256, mirror_point_left_right, preview_linear_array_inner,
    preview_radial_array_inner, rotate_point_about, symmetry_sin_cos,
};
use pattern_edit_commands::{
    add_connected_vertex, add_edge, add_ray_to_first_target, add_vertex, apply_mirror_selection,
    confirm_linear_array, confirm_radial_array, connect_edge_intersection,
    connect_intersection_cluster, connect_t_junction, generate_benchmark_pattern,
    mirror_edge_left_right, move_edge, move_vertex, move_vertices, preflight_mirror_selection,
    preview_linear_array, preview_radial_array, remove_boundary_vertex, remove_edge, remove_vertex,
    repair_all_unsplit_intersections, resize_rectangular_paper, rotate_edge_about_point,
    set_cutting_allowed, split_boundary_edge, split_edge,
};
use project_lifecycle_commands::{
    list_recent_projects, new_project, open_project, open_recent_project, project_snapshot,
    save_project, save_project_as, update_project_memo, validate_project,
};
use stacked_fold_transaction::StackedFoldTransactionState;

use base64::Engine as _;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, MutexGuard, OnceLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
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
#[cfg(test)]
use fold_import_commands::*;
use fold_import_commands::{
    FoldImportState, apply_fold_import, cancel_fold_import, preview_fold_import,
};
use fold_technique_file_io::{
    FoldTechniqueFileIoState, open_fold_technique_file, save_fold_technique_file_as,
};
#[cfg(test)]
use geometric_constraint_analysis::{
    BoundedDirectMusResult, BoundedDirectMusUnknownReason,
    GEOMETRIC_CONSTRAINT_ANALYSIS_BUSY_MESSAGE, GEOMETRIC_CONSTRAINT_ANALYSIS_FAILED_MESSAGE,
    GeometricConstraintAnalysisBinding, GeometricConstraintAnalysisObserver,
    GeometricConstraintAnalysisRuntime, GeometricConstraintPreflightResponse,
    GeometricConstraintPreflightResult, GeometricConstraintUnknownReason,
    MAX_GEOMETRIC_CONSTRAINT_PRE_CANCELLED_REQUESTS, analyze_bounded_direct_mus_with_observer,
    analyze_geometric_constraint_document, analyze_geometric_constraint_document_with_observer,
    analyze_geometric_constraints_with_worker,
};
use geometric_constraint_analysis::{
    GeometricConstraintWorkerGate, analyze_geometric_constraints,
    cancel_geometric_constraint_analysis,
};
use geometric_constraint_commands::{
    GeometricConstraintSolveStage, add_edge_orientation_constraint, add_geometric_constraint,
    apply_geometric_constraint_solve, preview_geometric_constraint_edge_solve,
    preview_geometric_constraint_expression_solve, preview_geometric_constraint_solve,
    reevaluate_saved_vertex_expressions, remove_geometric_constraint,
};
#[cfg(test)]
use geometric_constraint_commands::{
    apply_geometric_constraint_solve_stage, expand_saved_vertex_references,
};
use global_flat_foldability::{
    GlobalFlatFoldabilityState, archive_revalidation_deadline, begin_global_flat_foldability,
    cancel_global_flat_foldability, get_current_layer_order_view,
    get_global_flat_foldability_progress, get_global_flat_foldability_result,
    reanalyze_editor_flat_layer_order_with_required_pairs_and_deadline,
    revalidate_archived_flat_layer_evidence_with_deadline,
    revalidate_authenticated_non_refining_graph_layer_evidence,
};
use history_settings::{get_history_entry_limit, set_history_entry_limit};
#[cfg(test)]
use import_command_support::validate_import_scale;
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
use ori_collision::{
    EffectiveCutCollisionGeometryInputV1, diagnose_effective_cut_multi_hinge_union_gaps_v1,
    diagnose_effective_cut_source_flat_pairs_v1, prepare_effective_cut_collision_geometry_v1,
    prepare_effective_cut_static_pair_registry_bridge_v1,
    prepare_effective_cut_static_thickness_prerequisite_v1,
};
use ori_core::{
    BoundaryEdgeRef, BoundedDirectMusObserverV1, BoundedDirectMusV1, Command,
    ConstraintPreflightV1, ConstraintSolveLimitsV1, DirectConstraintConflictV1, EditorState,
    EditorTopology, GeometricConstraintLimitsV1, GeometricConstraintPreflightObserverControlV1,
    GeometricConstraintPreflightObserverV1, GeometricConstraintUnknownReasonV1,
    GlobalFlatFoldabilityCheckpoint, GlobalFlatFoldabilityInput, GlobalFlatFoldabilityLimits,
    GlobalFlatFoldabilityObserver, GlobalFlatFoldabilityOutcome,
    GlobalFlatFoldabilityUnknownReason, IntersectionEdgeTarget, JunctionVertexIntent,
    LocalFlatFoldabilityReport, LocalFlatFoldabilityReportStatus,
    MAX_BOUNDED_DIRECT_MUS_CONSTRAINTS_V1, MAX_EDITOR_HISTORY_ENTRIES, MirrorAxisV1,
    MirrorSelectionModeV1, PaperValidationIssue, PointPolygonRelation, TopologyAnalysisInput,
    TopologyIssue, TopologySnapshot, ValidationIssue, VertexPositionUpdate,
    analyze_global_flat_foldability_with_observer, analyze_local_flat_foldability,
    create_rectangular_sheet, find_bounded_direct_mus_with_observer_v1,
    prepare_geometric_constraints_v1, segment_midpoint_polygon_relation,
    solve_geometric_constraints_v1, solve_geometric_constraints_with_drivers_v1,
    validate_crease_pattern, validate_paper,
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
    FoldPreview, FoldPreviewWarning, LayerEvidenceArchiveKindV1, LayerEvidenceArchiveV1,
    MAX_PROJECT_TEXTURE_ASSET_BYTES, MAX_PROJECT_TEXTURE_ASSET_TOTAL_BYTES, Ori2ProjectArchive,
    PolarVertexConstructionExpressions, ProjectDocument, ProjectNumericExpressions,
    ProjectTextureAssetV1, ProjectTextureMediaTypeV1, RectangularPaperCreationExpressions,
    SvgBoundaryCandidateId, SvgBoundaryCandidateKind, SvgConversionOptions, SvgDashPattern,
    SvgGroupMapping, SvgGroupTarget, SvgLineCap, SvgPreview, SvgPreviewWarning,
    SvgRootPhysicalSize, SvgRootViewBox, SvgStyleGroupId, SvgWarningKind,
    VertexCoordinateExpressionChange, VertexCoordinateExpressionTransition,
    VertexCoordinateExpressions, generate_project_thumbnail_svg, read_fold_preview,
    read_svg_preview,
};
use ori_kinematics::{
    CanonicalHingeAngles, HingeAngle, prepare_effective_cut_kinematics_diagnostic_v1,
    prepare_effective_cut_retained_face_pair_registry_v1,
};
use ori_topology::{
    FaceExtractionInput, MaterialComponentKey, diagnose_closed_cut_topology_snapshot_v1,
    diagnose_cut_material_component_selection_v1, diagnose_cut_material_removal_plan_v1,
    diagnose_effective_cut_material_snapshot_v1,
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
use sha2::Digest as _;
use stacked_fold_read::{
    DyadicPathPreviewState, apply_dyadic_pose_path_preview_v1, cancel_current_stacked_fold_read_v1,
    cancel_dyadic_pose_path_preview_v1, mint_dyadic_pose_path_preview_v1,
    propose_current_cycle_pose_v1, propose_current_stacked_fold_read,
    read_bounded_dyadic_pose_graph_v1, read_even_cycle_candidates_v1, read_live_hinge_registry_v1,
};
use stacked_fold_transaction::{
    apply_named_accordion_fold_transaction, apply_named_book_fold_transaction,
    apply_named_layer_selective_transaction, apply_named_reverse_fold_transaction,
    apply_named_sink_fold_transaction, apply_stacked_fold_transaction,
    cancel_stacked_fold_transaction_preview, preview_named_basic_fold_timeline,
};
#[cfg(test)]
use svg_import_commands::*;
use svg_import_commands::{
    SvgImportState, apply_svg_import, cancel_svg_import, preview_svg_import,
    validate_svg_import_settings,
};
use tauri::{AppHandle, Emitter, Manager, State};
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
const TOPOLOGY_ANALYSIS_TASK_FAILED_MESSAGE: &str =
    "構造解析処理を完了できませんでした。もう一度実行してください。";
const INSTRUCTION_TOPOLOGY_ANALYSIS_TASK_FAILED_MESSAGE: &str =
    "折り手順の構造解析処理を完了できませんでした。もう一度実行してください。";
const PROJECT_OPEN_TASK_FAILED_MESSAGE: &str =
    "プロジェクトの読み込み処理を完了できませんでした。もう一度実行してください。";
const PROJECT_NUMERIC_EXPRESSIONS_INVALID_MESSAGE: &str =
    "保存された作成時サイズ式を検証できませんでした。";
const PROJECT_NUMERIC_EXPRESSIONS_BUSY_MESSAGE: &str =
    "作成時サイズ式を評価中です。少し待ってからもう一度開いてください。";
#[cfg(target_os = "macos")]
const MACOS_QUIT_MENU_ID: &str = "origami2_quit";

fn topology_analysis_task_error<T>(_: T) -> String {
    TOPOLOGY_ANALYSIS_TASK_FAILED_MESSAGE.to_owned()
}

fn instruction_topology_analysis_task_error<T>(_: T) -> String {
    INSTRUCTION_TOPOLOGY_ANALYSIS_TASK_FAILED_MESSAGE.to_owned()
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
    Mutex<Option<GeometricConstraintSolveStage>>,
);

impl AppState {
    fn new(project: ProjectState) -> Self {
        Self(
            Mutex::new(project),
            NativePoseWorkerGate::default(),
            GeometricConstraintWorkerGate::default(),
            Mutex::new(None),
        )
    }

    fn try_acquire_native_pose_worker(&self) -> Option<NativePoseWorkerPermit> {
        self.1.try_acquire()
    }

    #[cfg(test)]
    fn native_pose_worker_is_busy(&self) -> bool {
        self.1.is_busy()
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
    current_layer_evidence: Option<stacked_fold_transaction::CurrentLayerEvidence>,
    numeric_expressions: ProjectNumericExpressions,
    texture_assets: Vec<ori_formats::ProjectTextureAssetV1>,
    reference_model_assets: Vec<ori_formats::ProjectReferenceModelAssetV1>,
    material_void_evidence: ori_domain::MaterialVoidEvidenceDocumentV1,
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
            current_layer_evidence: None,
            numeric_expressions: ProjectNumericExpressions::default(),
            texture_assets: Vec::new(),
            reference_model_assets: Vec::new(),
            material_void_evidence: Default::default(),
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
            current_layer_evidence: None,
            numeric_expressions: ProjectNumericExpressions::default(),
            texture_assets: Vec::new(),
            reference_model_assets: Vec::new(),
            material_void_evidence: Default::default(),
            saved_revision: None,
            saved_document: None,
        }
    }

    fn from_document(mut document: ProjectDocument, current_path: PathBuf) -> Result<Self, String> {
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
        let reference_model_assets = document.reference_model_assets;
        let material_void_evidence = document.material_void_evidence;
        let mut editor = EditorState::with_all_document_parts_annotations_underlays_and_memo(
            document.crease_pattern,
            document.paper,
            document.instruction_timeline,
            document.geometric_constraints,
            document.layers,
            document.element_metadata,
            document.annotations,
            document.underlays,
            document.memo,
        );
        editor
            .restore_beginner_design_profile(document.beginner_design_profile)
            .map_err(|_| PROJECT_ARCHIVE_INVALID_MESSAGE.to_owned())?;
        Ok(Self {
            instance_id: ProjectId::new(),
            project_id: document.project_id,
            name: document.name,
            current_path: Some(current_path),
            saved_revision: Some(editor.revision()),
            applied_pose_authority: CurrentAppliedPoseAuthority::default(),
            current_layer_evidence: None,
            numeric_expressions,
            texture_assets,
            reference_model_assets,
            material_void_evidence,
            saved_document: Some(saved_document),
            editor,
        })
    }

    #[cfg(test)]
    fn from_valid_document(document: ProjectDocument, current_path: PathBuf) -> Self {
        Self::from_document(document, current_path).expect("valid project document")
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
        let reference_model_assets = document.reference_model_assets.clone();
        let material_void_evidence = document.material_void_evidence.clone();
        let archived_layer_evidence = project.layer_evidence.clone();
        let mut restored = Self {
            instance_id: ProjectId::new(),
            project_id: document.project_id,
            name: document.name,
            current_path: Some(current_path),
            saved_revision: Some(editor.revision()),
            applied_pose_authority: CurrentAppliedPoseAuthority::default(),
            current_layer_evidence: None,
            numeric_expressions: document.numeric_expressions,
            texture_assets,
            reference_model_assets,
            material_void_evidence,
            saved_document: Some(saved_document),
            editor,
        };
        if let Some(pose) = persisted_pose.as_ref() {
            restore_persisted_current_pose(&mut restored, pose)
                .map_err(|_| PROJECT_ARCHIVE_INVALID_MESSAGE.to_owned())?;
        }
        if let Some(evidence) = archived_layer_evidence.as_ref() {
            restored.current_layer_evidence =
                Some(revalidate_archived_layer_evidence(&restored, evidence)?);
        }
        Ok(restored)
    }

    fn from_recovery_project_archive(project: Ori2ProjectArchive) -> Result<Self, ()> {
        let archived_layer_evidence = project.layer_evidence.clone();
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
        let reference_model_assets = document.reference_model_assets.clone();
        let material_void_evidence = document.material_void_evidence.clone();
        let mut restored = Self {
            instance_id: ProjectId::new(),
            project_id: document.project_id,
            name: document.name,
            current_path: None,
            saved_revision: None,
            applied_pose_authority: CurrentAppliedPoseAuthority::default(),
            current_layer_evidence: None,
            numeric_expressions: document.numeric_expressions,
            texture_assets,
            reference_model_assets,
            material_void_evidence,
            saved_document: None,
            editor,
        };
        if let Some(pose) = persisted_pose.as_ref() {
            restore_persisted_current_pose(&mut restored, pose).map_err(|_| ())?;
        }
        if let Some(evidence) = archived_layer_evidence.as_ref() {
            restored.current_layer_evidence =
                Some(revalidate_archived_layer_evidence(&restored, evidence).map_err(|_| ())?);
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
            material_void_evidence: self.material_void_evidence.clone(),
            instruction_timeline: self.editor.instruction_timeline().clone(),
            numeric_expressions,
            geometric_constraints: self.editor.geometric_constraints().clone(),
            layers: self.editor.project_layers().clone(),
            annotations: self.editor.annotations().clone(),
            underlays: self.editor.underlays().clone(),
            element_metadata: self.editor.element_metadata().clone(),
            beginner_design_profile: self.editor.beginner_design_profile().clone(),
            texture_assets: self.texture_assets.clone(),
            reference_model_assets: self.reference_model_assets.clone(),
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
            layer_evidence: self.archived_layer_evidence()?,
            document,
            editor_history: (!history.is_default_empty()).then_some(history),
        })
    }

    fn archived_layer_evidence(&self) -> Result<Option<LayerEvidenceArchiveV1>, String> {
        let Some(current) = &self.current_layer_evidence else {
            return Ok(None);
        };
        let evidence = match current {
            stacked_fold_transaction::CurrentLayerEvidence::CertifiedFlat(snapshot) => {
                let mut snapshot = snapshot.clone();
                snapshot.provenance.source.source_revision = 0;
                LayerEvidenceArchiveKindV1::Flat {
                    canonical_snapshot_json: serde_json::to_string(&snapshot)
                        .map_err(|_| PROJECT_SERIALIZATION_FAILED_MESSAGE.to_owned())?,
                }
            }
            stacked_fold_transaction::CurrentLayerEvidence::NonFlat(proof) => {
                LayerEvidenceArchiveKindV1::NonFlat {
                    fixed_face: proof.fixed_face().map(|face| wire_id(&face)).transpose()?,
                    hinge_angles: proof
                        .hinge_angles()
                        .iter()
                        .map(|angle| {
                            Ok(ori_formats::LayerEvidenceHingeAngleV1 {
                                edge: wire_id(&angle.edge())?,
                                angle_degrees: angle.angle_degrees(),
                            })
                        })
                        .collect::<Result<Vec<_>, String>>()?,
                    material_faces: proof
                        .material_faces()
                        .iter()
                        .map(|face| {
                            Ok(ori_formats::LayerEvidenceFaceV1 {
                                face_id: wire_id(&face.face_id)?,
                                face_key_sha256: lowercase_hex(&face.face_key.0),
                            })
                        })
                        .collect::<Result<Vec<_>, String>>()?,
                    cells: proof
                        .overlap_cells()
                        .iter()
                        .map(|cell| {
                            Ok(ori_formats::LayerEvidenceCellV1 {
                                boundary_xy: cell
                                    .boundary()
                                    .iter()
                                    .map(|point| [point.x, point.y])
                                    .collect(),
                                lower_face: wire_id(&cell.lower_face())?,
                                upper_face: wire_id(&cell.upper_face())?,
                            })
                        })
                        .collect::<Result<Vec<_>, String>>()?,
                    pair_orders: proof
                        .face_pair_orders()
                        .iter()
                        .map(|pair| {
                            Ok(ori_formats::LayerEvidencePairOrderV1 {
                                lower_face: wire_id(&pair.lower_face())?,
                                upper_face: wire_id(&pair.upper_face())?,
                            })
                        })
                        .collect::<Result<Vec<_>, String>>()?,
                }
            }
        };
        Ok(Some(LayerEvidenceArchiveV1 {
            version: ori_formats::LAYER_EVIDENCE_SCHEMA_VERSION_V1,
            project_instance_id: wire_id(&self.instance_id)?,
            project_id: wire_id(&self.project_id)?,
            // Persisted editor history deliberately reopens at revision zero.
            // Bind evidence to that canonical admission revision rather than
            // to the process-local counter that is not part of the archive.
            revision: 0,
            fold_model_fingerprint_sha256: self.editor.fold_model_fingerprint_v1(),
            evidence,
        }))
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
            || saved.beginner_design_profile != *self.editor.beginner_design_profile()
            || saved.texture_assets != self.texture_assets
            || saved.reference_model_assets != self.reference_model_assets
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
    let mut editor = match &project.editor_history {
        Some(history) => {
            if history.project_id() != project.document.project_id {
                return Err(());
            }
            EditorState::with_all_document_parts_annotations_underlays_memo_and_history_v1(
                project.document.crease_pattern.clone(),
                project.document.paper.clone(),
                project.document.instruction_timeline.clone(),
                project.document.geometric_constraints.clone(),
                project.document.layers.clone(),
                project.document.element_metadata.clone(),
                project.document.annotations.clone(),
                project.document.underlays.clone(),
                project.document.memo.clone(),
                history.clone(),
            )
            .map_err(|_| ())
        }
        None => Ok(
            EditorState::with_all_document_parts_annotations_underlays_and_memo(
                project.document.crease_pattern.clone(),
                project.document.paper.clone(),
                project.document.instruction_timeline.clone(),
                project.document.geometric_constraints.clone(),
                project.document.layers.clone(),
                project.document.element_metadata.clone(),
                project.document.annotations.clone(),
                project.document.underlays.clone(),
                project.document.memo.clone(),
            ),
        ),
    }?;
    editor
        .restore_beginner_design_profile(project.document.beginner_design_profile.clone())
        .map_err(|_| ())?;
    validate_reachable_history_instruction_poses(&project.document, &editor)?;
    Ok(editor)
}

fn revalidate_archived_layer_evidence(
    project: &ProjectState,
    archived: &LayerEvidenceArchiveV1,
) -> Result<stacked_fold_transaction::CurrentLayerEvidence, String> {
    let archived_instance = serde_json::from_value::<ProjectId>(serde_json::Value::String(
        archived.project_instance_id.clone(),
    ))
    .map_err(|_| PROJECT_ARCHIVE_INVALID_MESSAGE.to_owned())?;
    let archived_project =
        serde_json::from_value::<ProjectId>(serde_json::Value::String(archived.project_id.clone()))
            .map_err(|_| PROJECT_ARCHIVE_INVALID_MESSAGE.to_owned())?;
    if archived_instance.canonical_bytes() == [0; 16]
        || archived_project != project.project_id
        || archived.revision != project.editor.revision()
        || archived.fold_model_fingerprint_sha256 != project.editor.fold_model_fingerprint_v1()
    {
        return Err(PROJECT_ARCHIVE_INVALID_MESSAGE.to_owned());
    }
    let revalidation_deadline =
        archive_revalidation_deadline().map_err(|_| PROJECT_ARCHIVE_INVALID_MESSAGE.to_owned())?;
    match &archived.evidence {
        LayerEvidenceArchiveKindV1::Flat {
            canonical_snapshot_json,
        } => revalidate_archived_flat_layer_evidence_with_deadline(
            project,
            canonical_snapshot_json,
            revalidation_deadline,
        )
        .map_err(|_| PROJECT_ARCHIVE_INVALID_MESSAGE.to_owned()),
        LayerEvidenceArchiveKindV1::NonFlat {
            fixed_face,
            hinge_angles,
            material_faces,
            cells,
            pair_orders,
        } => {
            let parse_face = |value: &str| {
                serde_json::from_value::<FaceId>(serde_json::Value::String(value.to_owned()))
                    .map_err(|_| PROJECT_ARCHIVE_INVALID_MESSAGE.to_owned())
            };
            let parse_edge = |value: &str| {
                serde_json::from_value::<EdgeId>(serde_json::Value::String(value.to_owned()))
                    .map_err(|_| PROJECT_ARCHIVE_INVALID_MESSAGE.to_owned())
            };
            let fixed_face = fixed_face.as_deref().map(parse_face).transpose()?;
            let mut angles = Vec::new();
            angles
                .try_reserve_exact(hinge_angles.len())
                .map_err(|_| PROJECT_ARCHIVE_INVALID_MESSAGE.to_owned())?;
            let mut previous_edge = None;
            for angle in hinge_angles {
                if angle.angle_degrees.to_bits() == (-0.0_f64).to_bits() {
                    return Err(PROJECT_ARCHIVE_INVALID_MESSAGE.to_owned());
                }
                let edge = parse_edge(&angle.edge)?;
                if previous_edge.is_some_and(|previous: EdgeId| {
                    previous.canonical_bytes() >= edge.canonical_bytes()
                }) {
                    return Err(PROJECT_ARCHIVE_INVALID_MESSAGE.to_owned());
                }
                previous_edge = Some(edge);
                angles.push(
                    HingeAngle::new(edge, angle.angle_degrees)
                        .map_err(|_| PROJECT_ARCHIVE_INVALID_MESSAGE.to_owned())?,
                );
            }
            let angles = CanonicalHingeAngles::new(angles)
                .map_err(|_| PROJECT_ARCHIVE_INVALID_MESSAGE.to_owned())?;
            let current_applied_pose = project
                .editor
                .current_applied_pose()
                .ok_or_else(|| PROJECT_ARCHIVE_INVALID_MESSAGE.to_owned())?;
            let predecessor = project
                .editor
                .clone_predecessor_if_last_stacked_fold_v1()
                .ok_or_else(|| PROJECT_ARCHIVE_INVALID_MESSAGE.to_owned())?;
            let archived_pairs = pair_orders
                .iter()
                .map(|pair| {
                    Ok(ori_core::ArchivedNonFlatFacePairOrderInputV1 {
                        lower_face: parse_face(&pair.lower_face)?,
                        upper_face: parse_face(&pair.upper_face)?,
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            let trusted = if predecessor.pattern() == project.editor.pattern()
                && predecessor.paper() == project.editor.paper()
            {
                revalidate_authenticated_non_refining_graph_layer_evidence(
                    project.project_id,
                    &predecessor,
                    &project.editor,
                    fixed_face,
                    &angles,
                    revalidation_deadline,
                )
                .map_err(|_| PROJECT_ARCHIVE_INVALID_MESSAGE.to_owned())?
            } else {
                let prepared = ori_core::prepare_archived_refined_non_flat_layer_order_v1(
                    ori_core::PrepareArchivedRefinedNonFlatLayerOrderRequestV1 {
                        identity_namespace: project.project_id,
                        source_revision: predecessor.revision(),
                        source_pattern: predecessor.pattern(),
                        source_paper: predecessor.paper(),
                        target_admission_revision: project.editor.revision(),
                        target_pattern: project.editor.pattern(),
                        target_paper: project.editor.paper(),
                        fixed_face,
                        hinge_angles: &angles,
                        archived_pair_orders: &archived_pairs,
                        lineage_limits: ori_core::FaceLineageLimits::default(),
                        geometry_limits: ori_core::StackedFoldGeometryLimitsV1::default(),
                        max_face_pairs: ori_core::DEFAULT_MAX_STACKED_FOLD_NON_FLAT_FACE_PAIRS,
                    },
                )
                .map_err(|_| PROJECT_ARCHIVE_INVALID_MESSAGE.to_owned())?;
                let constrained_source_flat =
                    reanalyze_editor_flat_layer_order_with_required_pairs_and_deadline(
                        project.project_id,
                        &predecessor,
                        prepared.required_source_pair_orders(),
                        revalidation_deadline,
                    )
                    .map_err(|_| PROJECT_ARCHIVE_INVALID_MESSAGE.to_owned())?;
                ori_core::finish_archived_refined_non_flat_layer_order_v1(
                    prepared,
                    &constrained_source_flat,
                    current_applied_pose,
                )
                .map_err(|_| PROJECT_ARCHIVE_INVALID_MESSAGE.to_owned())?
            };
            let materials_match = trusted.material_faces().len() == material_faces.len()
                && trusted
                    .material_faces()
                    .iter()
                    .zip(material_faces)
                    .all(|(actual, expected)| {
                        parse_face(&expected.face_id) == Ok(actual.face_id)
                            && lowercase_hex_matches(&actual.face_key.0, &expected.face_key_sha256)
                    });
            let cells_match = trusted.overlap_cells().len() == cells.len()
                && trusted
                    .overlap_cells()
                    .iter()
                    .zip(cells)
                    .all(|(actual, expected)| {
                        parse_face(&expected.lower_face) == Ok(actual.lower_face())
                            && parse_face(&expected.upper_face) == Ok(actual.upper_face())
                            && actual.boundary().len() == expected.boundary_xy.len()
                            && actual.boundary().iter().zip(&expected.boundary_xy).all(
                                |(point, expected)| {
                                    point.x.to_bits() == expected[0].to_bits()
                                        && point.y.to_bits() == expected[1].to_bits()
                                },
                            )
                    });
            let pairs_match =
                trusted.face_pair_orders().len() == pair_orders.len()
                    && trusted.face_pair_orders().iter().zip(pair_orders).all(
                        |(actual, expected)| {
                            parse_face(&expected.lower_face) == Ok(actual.lower_face())
                                && parse_face(&expected.upper_face) == Ok(actual.upper_face())
                        },
                    );
            if trusted.fixed_face() != fixed_face
                || trusted.hinge_angles() != angles.as_slice()
                || !materials_match
                || !cells_match
                || !pairs_match
            {
                return Err(PROJECT_ARCHIVE_INVALID_MESSAGE.to_owned());
            }
            Ok(stacked_fold_transaction::CurrentLayerEvidence::NonFlat(
                trusted,
            ))
        }
    }
}

fn lowercase_hex_matches(bytes: &[u8], expected: &str) -> bool {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    expected.len() == bytes.len() * 2
        && bytes.iter().enumerate().all(|(index, byte)| {
            expected.as_bytes()[index * 2] == HEX[(byte >> 4) as usize]
                && expected.as_bytes()[index * 2 + 1] == HEX[(byte & 0x0f) as usize]
        })
}

fn lowercase_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut value = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        value.push(HEX[(byte >> 4) as usize] as char);
        value.push(HEX[(byte & 0x0f) as usize] as char);
    }
    value
}

fn wire_id(value: &impl Serialize) -> Result<String, String> {
    serde_json::to_value(value)
        .map_err(|_| PROJECT_SERIALIZATION_FAILED_MESSAGE.to_owned())?
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| PROJECT_SERIALIZATION_FAILED_MESSAGE.to_owned())
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

#[derive(Debug, Serialize)]
struct ProjectSnapshot {
    project_instance_id: ProjectId,
    project_id: ProjectId,
    name: String,
    memo: String,
    beginner_design_profile: ori_domain::BeginnerDesignProfileV1,
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
    annotations: ori_domain::AnnotationDocumentV1,
    underlays: ori_domain::UnderlayDocumentV1,
    fold_model_fingerprint: String,
    can_undo: bool,
    can_redo: bool,
    cutting_allowed: bool,
    reference_model_assets: Vec<ReferenceModelAssetSummaryV1>,
}

#[derive(Debug, Serialize)]
struct ReferenceModelAssetSummaryV1 {
    asset_id: AssetId,
    sha256: [u8; 32],
}

#[derive(Debug, Serialize)]
struct ProjectFileResponse {
    canceled: bool,
    project: ProjectSnapshot,
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AssignedLocalSufficiencyRequestV1 {
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    vertex: VertexId,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AssignedLocalSufficiencyResponseV1 {
    version: u32,
    project_instance_id: ProjectId,
    project_id: ProjectId,
    revision: u64,
    result: ori_topology::AssignedLocalSufficiencyV1,
    authorizes_project_mutation: bool,
}

const MAX_ASSIGNED_LOCAL_SUMMARY_VERTICES_V1: usize = 4096;
const MAX_ASSIGNED_LOCAL_SUMMARY_REDUCTIONS_V1: usize = 16_384;
static ASSIGNED_LOCAL_SUMMARY_GENERATION_V1: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AssignedLocalSufficiencySummaryRequestV1 {
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    expected_fold_model_fingerprint: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AssignedLocalSufficiencySummaryResponseV1 {
    version: u32,
    project_instance_id: ProjectId,
    project_id: ProjectId,
    revision: u64,
    fold_model_fingerprint: String,
    vertices: Vec<AssignedLocalSufficiencySummaryVertexV1>,
    total_reduction_steps: usize,
    authorizes_project_mutation: bool,
}

#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum AssignedLocalSufficiencySummaryVertexV1 {
    NecessaryFailed {
        vertex: VertexId,
    },
    SufficientProven {
        vertex: VertexId,
        model_id: &'static str,
        reduction_steps: usize,
    },
    Indeterminate {
        vertex: VertexId,
        reason: ori_topology::AssignedLocalSufficiencyReasonV1,
    },
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

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct EffectiveCutReadOnlyRequestV1 {
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    expected_fold_model_fingerprint: String,
    requested_component_keys: Vec<[u8; 32]>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EffectiveCutReadOnlyResponseV1 {
    version: u8,
    project_instance_id: ProjectId,
    project_id: ProjectId,
    revision: u64,
    fold_model_fingerprint: String,
    effective_snapshot_fingerprint: [u8; 32],
    geometry_model_id: &'static str,
    geometry_fingerprint: [u8; 32],
    pair_observation_model_id: &'static str,
    pair_observation_fingerprint: [u8; 32],
    multi_hinge_gap_model_id: &'static str,
    multi_hinge_gap_fingerprint: [u8; 32],
    source_flat_pair_count: usize,
    separated_pairs: usize,
    touching_pairs: usize,
    shared_hinge_corridor_observed_pairs: usize,
    shared_vertex_corridor_observed_pairs: usize,
    penetrating_pairs: usize,
    indeterminate_pairs: usize,
    multi_hinge_pairs: usize,
    multi_hinge_union_corridor_unproved_pairs: usize,
    authorizes_project_mutation: bool,
    authorizes_persistence: bool,
    authorizes_simulation_admission: bool,
    authorizes_pair_classification: bool,
    authorizes_collision_free_classification: bool,
    authorizes_pose_solving: bool,
    authorizes_material_removal: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct EffectiveCutCandidateListRequestV1 {
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    expected_fold_model_fingerprint: String,
}

#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct EffectiveCutCandidateV1 {
    component_key: [u8; 32],
    owns_original_boundary: bool,
    face_count: usize,
    area_square_mm: f64,
    closure_component_count: usize,
    closure_face_count: usize,
    nested_dependency_count: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EffectiveCutCandidateListResponseV1 {
    version: u8,
    project_instance_id: ProjectId,
    project_id: ProjectId,
    revision: u64,
    fold_model_fingerprint: String,
    model_id: &'static str,
    diagnostic_fingerprint: [u8; 32],
    total_component_count: usize,
    boundary_component_count: usize,
    candidates: Vec<EffectiveCutCandidateV1>,
    authorizes_project_mutation: bool,
    authorizes_persistence: bool,
    authorizes_simulation_admission: bool,
    authorizes_material_removal: bool,
}

struct AnalyzedEffectiveCutCandidatesV1 {
    model_id: &'static str,
    diagnostic_fingerprint: [u8; 32],
    total_component_count: usize,
    boundary_component_count: usize,
    candidates: Vec<EffectiveCutCandidateV1>,
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
async fn prove_current_assigned_local_sufficiency_v1(
    state: State<'_, AppState>,
    request: AssignedLocalSufficiencyRequestV1,
) -> Result<AssignedLocalSufficiencyResponseV1, String> {
    let permit = state
        .try_acquire_native_pose_worker()
        .ok_or_else(|| "Another native pose analysis is already running.".to_owned())?;
    let (paper, pattern, source_fingerprint) = {
        let project = lock_project(&state)?;
        if project.instance_id != request.expected_project_instance_id
            || project.project_id != request.expected_project_id
            || project.editor.revision() != request.expected_revision
        {
            return Err("The project changed while local sufficiency was analyzed.".to_owned());
        }
        (
            project.editor.paper().clone(),
            project.editor.pattern().clone(),
            project.editor.fold_model_fingerprint_v1(),
        )
    };
    let vertex = request.vertex;
    let result = tauri::async_runtime::spawn_blocking(move || {
        let _permit = permit;
        ori_topology::prove_assigned_local_sufficiency_v1(
            &paper,
            &pattern,
            vertex,
            ori_topology::AssignedLocalSufficiencyLimitsV1::default(),
        )
    })
    .await
    .map_err(|_| "Local sufficiency analysis failed.".to_owned())?;
    {
        let project = lock_project(&state)?;
        if project.instance_id != request.expected_project_instance_id
            || project.project_id != request.expected_project_id
            || project.editor.revision() != request.expected_revision
            || project.editor.fold_model_fingerprint_v1() != source_fingerprint
        {
            return Err("The project changed while local sufficiency was analyzed.".to_owned());
        }
    }
    Ok(AssignedLocalSufficiencyResponseV1 {
        version: 1,
        project_instance_id: request.expected_project_instance_id,
        project_id: request.expected_project_id,
        revision: request.expected_revision,
        result,
        authorizes_project_mutation: false,
    })
}

#[tauri::command]
fn cancel_current_assigned_local_sufficiency_summary_v1() -> Result<(), String> {
    ASSIGNED_LOCAL_SUMMARY_GENERATION_V1
        .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |value| {
            value.checked_add(1)
        })
        .map(|_| ())
        .map_err(|_| "The local sufficiency summary generation is exhausted.".to_owned())
}

#[tauri::command]
async fn summarize_current_assigned_local_sufficiency_v1(
    state: State<'_, AppState>,
    request: AssignedLocalSufficiencySummaryRequestV1,
) -> Result<AssignedLocalSufficiencySummaryResponseV1, String> {
    let generation = ASSIGNED_LOCAL_SUMMARY_GENERATION_V1
        .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |value| {
            value.checked_add(1)
        })
        .map(|previous| previous + 1)
        .map_err(|_| "The local sufficiency summary generation is exhausted.".to_owned())?;
    let permit = state
        .try_acquire_native_pose_worker()
        .ok_or_else(|| "Another native pose analysis is already running.".to_owned())?;
    let (paper, pattern) = {
        let project = lock_project(&state)?;
        if project.instance_id != request.expected_project_instance_id
            || project.project_id != request.expected_project_id
            || project.editor.revision() != request.expected_revision
            || project.editor.fold_model_fingerprint_v1() != request.expected_fold_model_fingerprint
        {
            return Err("The project changed while local sufficiency was analyzed.".to_owned());
        }
        if project.editor.pattern().vertices.len() > MAX_ASSIGNED_LOCAL_SUMMARY_VERTICES_V1 {
            return Err("The local sufficiency summary vertex limit was reached.".to_owned());
        }
        (
            project.editor.paper().clone(),
            project.editor.pattern().clone(),
        )
    };
    let result = tauri::async_runtime::spawn_blocking(move || {
        let _permit = permit;
        let mut checkpoint = || {
            if ASSIGNED_LOCAL_SUMMARY_GENERATION_V1.load(Ordering::SeqCst) == generation {
                ori_topology::CooperativeAnalysisCheckpoint::Continue
            } else {
                ori_topology::CooperativeAnalysisCheckpoint::Cancelled
            }
        };
        let batch = ori_topology::prove_all_assigned_local_sufficiency_v1(
            &paper,
            &pattern,
            ori_topology::AssignedLocalSufficiencyLimitsV1 {
                max_vertices: MAX_ASSIGNED_LOCAL_SUMMARY_VERTICES_V1,
                max_reductions:
                    ori_topology::AssignedLocalSufficiencyLimitsV1::default().max_reductions,
            },
            MAX_ASSIGNED_LOCAL_SUMMARY_REDUCTIONS_V1,
            &mut checkpoint,
        );
        let vertices = batch
            .vertices
            .into_iter()
            .map(|proof| match proof {
                ori_topology::AssignedLocalSufficiencyV1::Proven {
                    vertex,
                    model_id,
                    reduction_steps,
                    ..
                } => AssignedLocalSufficiencySummaryVertexV1::SufficientProven {
                    vertex,
                    model_id,
                    reduction_steps,
                },
                ori_topology::AssignedLocalSufficiencyV1::Indeterminate {
                    vertex,
                    reason:
                        ori_topology::AssignedLocalSufficiencyReasonV1::NecessaryConditionsNotSatisfied,
                } => AssignedLocalSufficiencySummaryVertexV1::NecessaryFailed { vertex },
                ori_topology::AssignedLocalSufficiencyV1::Indeterminate { vertex, reason } => {
                    AssignedLocalSufficiencySummaryVertexV1::Indeterminate { vertex, reason }
                }
            })
            .collect();
        (vertices, batch.total_reduction_steps)
    })
    .await
    .map_err(|_| "Local sufficiency summary analysis failed.".to_owned())?;
    {
        let project = lock_project(&state)?;
        if project.instance_id != request.expected_project_instance_id
            || project.project_id != request.expected_project_id
            || project.editor.revision() != request.expected_revision
            || project.editor.fold_model_fingerprint_v1() != request.expected_fold_model_fingerprint
        {
            return Err("The project changed while local sufficiency was analyzed.".to_owned());
        }
    }
    Ok(AssignedLocalSufficiencySummaryResponseV1 {
        version: 1,
        project_instance_id: request.expected_project_instance_id,
        project_id: request.expected_project_id,
        revision: request.expected_revision,
        fold_model_fingerprint: request.expected_fold_model_fingerprint,
        vertices: result.0,
        total_reduction_steps: result.1,
        authorizes_project_mutation: false,
    })
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
    foldability_state: State<'_, GlobalFlatFoldabilityState>,
) -> Result<CurrentStaticCollisionDiagnosticResponse, String> {
    inspect_current_static_collision_authority(&state, &foldability_state).await
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

fn analyze_effective_cut_candidates_v1(
    source: FaceExtractionInput<'_>,
) -> Result<AnalyzedEffectiveCutCandidatesV1, String> {
    const MAX_COMPONENTS: usize = 64;
    const MAX_FACES: usize = 50_000;
    let limits = Default::default();
    let selection = diagnose_cut_material_component_selection_v1(source, limits)
        .map_err(|_| "Cut material components are unsupported.".to_owned())?;
    if selection.selections().len() > MAX_COMPONENTS
        || selection
            .selections()
            .iter()
            .map(|entry| entry.faces.len())
            .try_fold(0_usize, usize::checked_add)
            .is_none_or(|count| count > MAX_FACES)
        || selection.authorizes_project_mutation()
        || selection.authorizes_material_removal()
        || selection.authorizes_simulation_admission()
    {
        return Err("Cut material candidate invariants failed closed.".to_owned());
    }
    let topology = diagnose_closed_cut_topology_snapshot_v1(source, limits)
        .map_err(|_| "Cut topology is unsupported.".to_owned())?;
    let mut areas = HashMap::new();
    areas
        .try_reserve(topology.snapshot().faces.len())
        .map_err(|_| "Cut material face list is too large.".to_owned())?;
    for face in &topology.snapshot().faces {
        if areas.insert(face.id, face.area).is_some() {
            return Err("Cut material face identities are ambiguous.".to_owned());
        }
    }
    if areas.len() > MAX_FACES {
        return Err("Cut material face list is too large.".to_owned());
    }
    let boundary_component_count = selection
        .selections()
        .iter()
        .filter(|entry| entry.owns_original_boundary)
        .count();
    if boundary_component_count != 1 {
        return Err("Cut material boundary ownership is ambiguous.".to_owned());
    }
    let boundary_component = selection
        .selections()
        .iter()
        .find(|entry| entry.owns_original_boundary)
        .map(|entry| entry.component)
        .ok_or_else(|| "Cut material boundary ownership is missing.".to_owned())?;
    if selection
        .selections()
        .windows(2)
        .any(|pair| pair[0].component >= pair[1].component)
    {
        return Err("Cut material candidate ordering is non-canonical.".to_owned());
    }
    let mut candidates = Vec::new();
    candidates
        .try_reserve_exact(selection.selections().len().saturating_sub(1))
        .map_err(|_| "Cut material candidate list is too large.".to_owned())?;
    for entry in selection
        .selections()
        .iter()
        .filter(|entry| !entry.owns_original_boundary)
    {
        if entry.faces.is_empty() {
            return Err("Cut material candidate has no material faces.".to_owned());
        }
        let plan = diagnose_cut_material_removal_plan_v1(source, &[entry.component], limits)
            .map_err(|_| "Cut material dependency closure is unsupported.".to_owned())?;
        if plan.authorizes_project_mutation()
            || plan.authorizes_material_removal()
            || plan.authorizes_simulation_admission()
            || plan.requested_components() != [entry.component]
            || plan.boundary_component() != boundary_component
            || plan.removed_components().is_empty()
            || plan.removed_components().len() > MAX_COMPONENTS
            || plan.removed_faces().len() > MAX_FACES
            || !plan.removed_components().contains(&entry.component)
            || plan.removed_components().contains(&boundary_component)
            || !plan.retained_components().contains(&boundary_component)
            || plan
                .removed_components()
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || plan
                .removed_faces()
                .windows(2)
                .any(|pair| pair[0].canonical_bytes() >= pair[1].canonical_bytes())
        {
            return Err("Cut material dependency closure failed closed.".to_owned());
        }
        let area_square_mm = entry.faces.iter().try_fold(0.0_f64, |sum, face| {
            let area = *areas.get(face)?;
            let next = sum + area;
            (area.is_finite() && area >= 0.0 && next.is_finite()).then_some(next)
        });
        let Some(area_square_mm) = area_square_mm else {
            return Err("Cut material candidate area is invalid.".to_owned());
        };
        candidates.push(EffectiveCutCandidateV1 {
            component_key: entry.component.0,
            owns_original_boundary: false,
            face_count: entry.faces.len(),
            area_square_mm,
            closure_component_count: plan.removed_components().len(),
            closure_face_count: plan.removed_faces().len(),
            nested_dependency_count: plan.removed_components().len().saturating_sub(1),
        });
    }
    if candidates.len() + boundary_component_count != selection.selections().len() {
        return Err("Cut material candidate partition is incomplete.".to_owned());
    }
    Ok(AnalyzedEffectiveCutCandidatesV1 {
        model_id: selection.model_id(),
        diagnostic_fingerprint: selection.fingerprint_v1(),
        total_component_count: selection.selections().len(),
        boundary_component_count,
        candidates,
    })
}

#[tauri::command]
async fn list_effective_cut_candidates_v1(
    state: State<'_, AppState>,
    request: EffectiveCutCandidateListRequestV1,
) -> Result<EffectiveCutCandidateListResponseV1, String> {
    if !valid_fold_model_fingerprint(&request.expected_fold_model_fingerprint) {
        return Err("The fold-model fingerprint must be canonical lowercase SHA-256.".into());
    }
    let input = {
        let project = lock_and_expect(
            &state,
            ProjectExpectation::new(
                request.expected_project_instance_id,
                request.expected_project_id,
                request.expected_revision,
            ),
        )?;
        if project.editor.fold_model_fingerprint_v1() != request.expected_fold_model_fingerprint {
            return Err("The effective-cut candidate request fingerprint is stale.".into());
        }
        project
            .editor
            .topology_analysis_input(request.expected_project_id)
    };
    let worker_request = request.clone();
    let (analyzed, analyzed_input) = tauri::async_runtime::spawn_blocking(move || {
        let source = FaceExtractionInput {
            identity_namespace: worker_request.expected_project_id,
            source_revision: worker_request.expected_revision,
            paper: input.paper(),
            pattern: input.pattern(),
        };
        let analyzed = analyze_effective_cut_candidates_v1(source)?;
        Ok::<_, String>((analyzed, input))
    })
    .await
    .map_err(|_| "Cut material candidate listing failed.".to_owned())??;
    let project = lock_project(&state)?;
    if project.instance_id != request.expected_project_instance_id
        || project.project_id != request.expected_project_id
        || project.editor.revision() != request.expected_revision
        || project.editor.fold_model_fingerprint_v1() != request.expected_fold_model_fingerprint
        || !analyzed_input.is_current_for(project.project_id, &project.editor)
    {
        return Err("The project changed while cut candidates were analyzed.".into());
    }
    Ok(EffectiveCutCandidateListResponseV1 {
        version: 1,
        project_instance_id: request.expected_project_instance_id,
        project_id: request.expected_project_id,
        revision: request.expected_revision,
        fold_model_fingerprint: request.expected_fold_model_fingerprint,
        model_id: analyzed.model_id,
        diagnostic_fingerprint: analyzed.diagnostic_fingerprint,
        total_component_count: analyzed.total_component_count,
        boundary_component_count: analyzed.boundary_component_count,
        candidates: analyzed.candidates,
        authorizes_project_mutation: false,
        authorizes_persistence: false,
        authorizes_simulation_admission: false,
        authorizes_material_removal: false,
    })
}

#[tauri::command]
async fn inspect_effective_cut_read_only_v1(
    state: State<'_, AppState>,
    request: EffectiveCutReadOnlyRequestV1,
) -> Result<EffectiveCutReadOnlyResponseV1, String> {
    validate_effective_cut_read_only_request_v1(&request)?;
    let input = {
        let project = lock_and_expect(
            &state,
            ProjectExpectation::new(
                request.expected_project_instance_id,
                request.expected_project_id,
                request.expected_revision,
            ),
        )?;
        if project.editor.fold_model_fingerprint_v1() != request.expected_fold_model_fingerprint {
            return Err("The effective-cut request fingerprint is stale.".into());
        }
        project
            .editor
            .topology_analysis_input(request.expected_project_id)
    };
    inspect_effective_cut_read_only_v1_with_input(state, request, input).await
}

fn validate_effective_cut_read_only_request_v1(
    request: &EffectiveCutReadOnlyRequestV1,
) -> Result<(), String> {
    const MAX_REQUESTED_COMPONENTS: usize = 64;
    if !valid_fold_model_fingerprint(&request.expected_fold_model_fingerprint) {
        return Err("The fold-model fingerprint must be canonical lowercase SHA-256.".into());
    }
    if request.requested_component_keys.is_empty()
        || request.requested_component_keys.len() > MAX_REQUESTED_COMPONENTS
        || request
            .requested_component_keys
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
    {
        return Err(
            "Effective-cut component keys must be non-empty, bounded, and canonical.".into(),
        );
    }
    Ok(())
}

async fn inspect_effective_cut_read_only_v1_with_input(
    state: State<'_, AppState>,
    request: EffectiveCutReadOnlyRequestV1,
    input: TopologyAnalysisInput,
) -> Result<EffectiveCutReadOnlyResponseV1, String> {
    let mut requested = Vec::new();
    requested
        .try_reserve_exact(request.requested_component_keys.len())
        .map_err(|_| "Effective-cut request is too large.".to_owned())?;
    requested.extend(
        request
            .requested_component_keys
            .iter()
            .copied()
            .map(MaterialComponentKey),
    );
    let worker_request = request.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let source = FaceExtractionInput {
            identity_namespace: worker_request.expected_project_id,
            source_revision: worker_request.expected_revision,
            paper: input.paper(),
            pattern: input.pattern(),
        };
        let effective =
            diagnose_effective_cut_material_snapshot_v1(source, &requested, Default::default())
                .map_err(|_| "Effective-cut material selection is unsupported.".to_owned())?;
        let kinematics =
            prepare_effective_cut_kinematics_diagnostic_v1(&effective, source, Default::default())
                .map_err(|_| "Effective-cut kinematics is unsupported.".to_owned())?;
        let registry_limits = Default::default();
        let registry = prepare_effective_cut_retained_face_pair_registry_v1(
            &kinematics,
            &effective,
            source,
            Default::default(),
            registry_limits,
        )
        .map_err(|_| "Effective-cut pair registry is unsupported.".to_owned())?;
        let prerequisite_limits = Default::default();
        let prerequisite = prepare_effective_cut_static_thickness_prerequisite_v1(
            &kinematics,
            &effective,
            source,
            Default::default(),
            prerequisite_limits,
        )
        .map_err(|_| "Effective-cut positive thickness is unsupported.".to_owned())?;
        let bridge = prepare_effective_cut_static_pair_registry_bridge_v1(
            &prerequisite,
            &registry,
            &kinematics,
            &effective,
            source,
            Default::default(),
            prerequisite_limits,
            registry_limits,
        )
        .map_err(|_| "Effective-cut pair binding is unsupported.".to_owned())?;
        let geometry_input = EffectiveCutCollisionGeometryInputV1 {
            bridge: &bridge,
            prerequisite: &prerequisite,
            registry: &registry,
            kinematics: &kinematics,
            effective: &effective,
            source,
            kinematics_limits: Default::default(),
            prerequisite_limits,
            registry_limits,
            geometry_limits: Default::default(),
        };
        let geometry = prepare_effective_cut_collision_geometry_v1(geometry_input)
            .map_err(|_| "Effective-cut source-flat geometry is unsupported.".to_owned())?;
        let observation = diagnose_effective_cut_source_flat_pairs_v1(
            &geometry,
            geometry_input,
            Default::default(),
        )
        .map_err(|_| "Effective-cut source-flat pair scan is unsupported.".to_owned())?;
        let gap = diagnose_effective_cut_multi_hinge_union_gaps_v1(
            &geometry,
            geometry_input,
            Default::default(),
        )
        .map_err(|_| "Effective-cut multi-hinge gap scan is unsupported.".to_owned())?;
        let counted_pairs = [
            observation.separated_pairs(),
            observation.touching_pairs(),
            observation.shared_hinge_allowed_pairs(),
            observation.shared_vertex_allowed_pairs(),
            observation.penetrating_pairs(),
            observation.indeterminate_pairs(),
        ]
        .into_iter()
        .try_fold(0_usize, usize::checked_add)
        .ok_or_else(|| "Effective-cut pair counts overflowed.".to_owned())?;
        if counted_pairs != observation.pair_count()
            || gap.multi_hinge_pairs() != gap.union_corridor_unproved_pairs()
            || effective.authorizes_project_mutation()
            || effective.authorizes_material_removal()
            || effective.authorizes_persistence()
            || effective.authorizes_simulation_admission()
            || kinematics.authorizes_simulation_admission()
            || kinematics.authorizes_pose_solving()
            || kinematics.authorizes_project_mutation()
            || kinematics.authorizes_persistence()
            || registry.authorizes_pair_classification()
            || registry.authorizes_collision_free_classification()
            || registry.authorizes_simulation_admission()
            || registry.authorizes_project_mutation()
            || registry.authorizes_material_removal()
            || registry.authorizes_persistence()
            || prerequisite.authorizes_collision_free_classification()
            || prerequisite.authorizes_simulation_admission()
            || prerequisite.authorizes_project_mutation()
            || prerequisite.authorizes_material_removal()
            || prerequisite.authorizes_persistence()
            || bridge.authorizes_pair_classification()
            || bridge.authorizes_collision_free_classification()
            || bridge.authorizes_simulation_admission()
            || bridge.authorizes_project_mutation()
            || bridge.authorizes_material_removal()
            || bridge.authorizes_persistence()
            || geometry.authorizes_pair_classification()
            || geometry.authorizes_collision_free_classification()
            || geometry.authorizes_pose_solving()
            || geometry.authorizes_simulation_admission()
            || geometry.authorizes_project_mutation()
            || geometry.authorizes_material_removal()
            || geometry.authorizes_persistence()
            || observation.authorizes_pair_classification()
            || observation.authorizes_collision_free_classification()
            || observation.authorizes_pose_solving()
            || observation.authorizes_simulation_admission()
            || observation.authorizes_project_mutation()
            || observation.authorizes_material_removal()
            || observation.authorizes_persistence()
            || gap.authorizes_pair_classification()
            || gap.authorizes_collision_free_classification()
            || gap.authorizes_pose_solving()
            || gap.authorizes_simulation_admission()
            || gap.authorizes_project_mutation()
            || gap.authorizes_material_removal()
            || gap.authorizes_persistence()
        {
            return Err("Effective-cut diagnostic invariants failed closed.".to_owned());
        }
        Ok::<_, String>((
            effective.fingerprint_v1(),
            geometry.model_id(),
            geometry.fingerprint_v1(),
            observation,
            gap,
            input,
        ))
    })
    .await
    .map_err(|_| "Effective-cut read-only analysis failed.".to_owned())??;
    let project = lock_project(&state)?;
    if project.instance_id != request.expected_project_instance_id
        || project.project_id != request.expected_project_id
        || project.editor.revision() != request.expected_revision
        || project.editor.fold_model_fingerprint_v1() != request.expected_fold_model_fingerprint
        || !result.5.is_current_for(project.project_id, &project.editor)
    {
        return Err("The project changed while effective-cut geometry was analyzed.".into());
    }
    Ok(EffectiveCutReadOnlyResponseV1 {
        version: 1,
        project_instance_id: request.expected_project_instance_id,
        project_id: request.expected_project_id,
        revision: request.expected_revision,
        fold_model_fingerprint: request.expected_fold_model_fingerprint,
        effective_snapshot_fingerprint: result.0,
        geometry_model_id: result.1,
        geometry_fingerprint: result.2,
        pair_observation_model_id: result.3.model_id(),
        pair_observation_fingerprint: result.3.fingerprint_v1(),
        multi_hinge_gap_model_id: result.4.model_id(),
        multi_hinge_gap_fingerprint: result.4.fingerprint_v1(),
        source_flat_pair_count: result.3.pair_count(),
        separated_pairs: result.3.separated_pairs(),
        touching_pairs: result.3.touching_pairs(),
        shared_hinge_corridor_observed_pairs: result.3.shared_hinge_allowed_pairs(),
        shared_vertex_corridor_observed_pairs: result.3.shared_vertex_allowed_pairs(),
        penetrating_pairs: result.3.penetrating_pairs(),
        indeterminate_pairs: result.3.indeterminate_pairs(),
        multi_hinge_pairs: result.4.multi_hinge_pairs(),
        multi_hinge_union_corridor_unproved_pairs: result.4.union_corridor_unproved_pairs(),
        authorizes_project_mutation: false,
        authorizes_persistence: false,
        authorizes_simulation_admission: false,
        authorizes_pair_classification: false,
        authorizes_collision_free_classification: false,
        authorizes_pose_solving: false,
        authorizes_material_removal: false,
    })
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
    ensure_project_expectation(
        project,
        ProjectExpectation::new(
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
        ),
    )?;
    let target_index = project.editor.project_layers().layers.len();
    execute_expected_command(
        project,
        ProjectExpectation::new(
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
        ),
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
    execute_expected_command(
        project,
        ProjectExpectation::new(
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
        ),
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
    execute_expected_command(
        project,
        ProjectExpectation::new(
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
        ),
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
    execute_expected_command(
        project,
        ProjectExpectation::new(
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
        ),
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
    execute_expected_command(
        project,
        ProjectExpectation::new(
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
        ),
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
    execute_expected_command(
        project,
        ProjectExpectation::new(
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
        ),
        Command::AssignEdgeToLayer { edge, layer },
    )
}

#[tauri::command]
fn add_annotation(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    record: ori_domain::AnnotationRecordV1,
) -> Result<ProjectSnapshot, String> {
    let expectation = ProjectExpectation::new(
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
    );
    let mut project = lock_project(&state)?;
    execute_expected_command(&mut project, expectation, Command::AddAnnotation { record })
}

#[tauri::command]
fn update_annotation(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    record: ori_domain::AnnotationRecordV1,
) -> Result<ProjectSnapshot, String> {
    let expectation = ProjectExpectation::new(
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
    );
    let mut project = lock_project(&state)?;
    execute_expected_command(
        &mut project,
        expectation,
        Command::UpdateAnnotation { record },
    )
}

#[tauri::command]
fn remove_annotation(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    id: ori_domain::AnnotationId,
) -> Result<ProjectSnapshot, String> {
    let expectation = ProjectExpectation::new(
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
    );
    let mut project = lock_project(&state)?;
    execute_expected_command(&mut project, expectation, Command::RemoveAnnotation { id })
}

#[tauri::command]
fn add_underlay(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    record: ori_domain::UnderlayRecordV1,
) -> Result<ProjectSnapshot, String> {
    let mut project = lock_project(&state)?;
    ensure_underlay_asset_exists(&project, record.asset)?;
    execute_expected_command(
        &mut project,
        ProjectExpectation::new(
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
        ),
        Command::AddUnderlay { record },
    )
}

#[tauri::command]
fn update_underlay(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    record: ori_domain::UnderlayRecordV1,
) -> Result<ProjectSnapshot, String> {
    let mut project = lock_project(&state)?;
    if matches!(
        project
            .editor
            .beginner_design_profile()
            .generation_constraints
            .target_asset,
        Some(ori_domain::BeginnerTargetAssetReferenceV1::ReferenceImage {
            underlay_id,
            asset_id,
        }) if underlay_id == record.id && asset_id != record.asset
    ) {
        return Err("the target reference image asset cannot be replaced".to_owned());
    }
    ensure_underlay_asset_exists(&project, record.asset)?;
    execute_expected_command(
        &mut project,
        ProjectExpectation::new(
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
        ),
        Command::UpdateUnderlay { record },
    )
}

fn ensure_underlay_asset_exists(project: &ProjectState, asset: AssetId) -> Result<(), String> {
    project
        .texture_assets
        .iter()
        .any(|candidate| candidate.id == asset)
        .then_some(())
        .ok_or_else(|| "underlay asset is unavailable".to_owned())
}

#[tauri::command]
fn remove_underlay(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    id: ori_domain::UnderlayId,
) -> Result<ProjectSnapshot, String> {
    let mut project = lock_project(&state)?;
    if matches!(
        project
            .editor
            .beginner_design_profile()
            .generation_constraints
            .target_asset,
        Some(ori_domain::BeginnerTargetAssetReferenceV1::ReferenceImage {
            underlay_id,
            ..
        }) if underlay_id == id
    ) {
        return Err("the underlay is the active beginner target reference image".to_owned());
    }
    execute_expected_command(
        &mut project,
        ProjectExpectation::new(
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
        ),
        Command::RemoveUnderlay { id },
    )
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct UnderlayImportDraft {
    id: ori_domain::UnderlayId,
    transform: ori_domain::UnderlayTransformV1,
    opacity: f64,
    layer: ori_domain::LayerId,
}

#[tauri::command]
fn import_underlay_image(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    draft: UnderlayImportDraft,
) -> Result<ProjectSnapshot, String> {
    {
        let _project = lock_and_expect(
            &state,
            ProjectExpectation::new(
                expected_project_instance_id,
                expected_project_id,
                expected_revision,
            ),
        )?;
    }
    let selected = app
        .dialog()
        .file()
        .set_title("下絵画像 / Underlay image")
        .add_filter("PNG or JPEG image", &["png", "jpg", "jpeg"])
        .blocking_pick_file();
    let Some(selected) = selected else {
        let project = lock_and_expect(
            &state,
            ProjectExpectation::new(
                expected_project_instance_id,
                expected_project_id,
                expected_revision,
            ),
        )?;
        return Ok(snapshot(&project));
    };
    let path = selected
        .into_path()
        .map_err(|_| "select a local image".to_owned())?;
    let bytes = read_bounded_regular_import_file(
        &path,
        MAX_PROJECT_TEXTURE_ASSET_BYTES,
        "could not read image",
        "image must be a PNG/JPEG no larger than 16 MiB",
    )?;
    let media_type = if valid_png_image_envelope(&bytes) {
        ProjectTextureMediaTypeV1::Png
    } else if valid_jpeg_image_envelope(&bytes) {
        ProjectTextureMediaTypeV1::Jpeg
    } else {
        return Err("selected file is not a valid PNG/JPEG".to_owned());
    };
    let mut project = lock_and_expect(
        &state,
        ProjectExpectation::new(
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
        ),
    )?;
    let retained_total = project
        .texture_assets
        .iter()
        .fold(bytes.len(), |total, asset| {
            total.saturating_add(asset.bytes.len())
        });
    if retained_total > MAX_PROJECT_TEXTURE_ASSET_TOTAL_BYTES
        || project.texture_assets.len() >= ori_formats::MAX_PROJECT_TEXTURE_ASSETS
    {
        return Err("project image asset limit exceeded".to_owned());
    }
    let asset = AssetId::new();
    project.texture_assets.push(ProjectTextureAssetV1 {
        id: asset,
        media_type,
        bytes,
    });
    let result = execute_expected_command(
        &mut project,
        ProjectExpectation::new(
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
        ),
        Command::AddUnderlay {
            record: ori_domain::UnderlayRecordV1 {
                id: draft.id,
                asset,
                transform: draft.transform,
                opacity: draft.opacity,
                layer: draft.layer,
            },
        },
    );
    if result.is_err() {
        project
            .texture_assets
            .retain(|candidate| candidate.id != asset);
    }
    result
}

fn read_bounded_regular_import_file(
    path: &Path,
    maximum_bytes: usize,
    read_error: &str,
    bounds_error: &str,
) -> Result<Vec<u8>, String> {
    let metadata = std::fs::symlink_metadata(path).map_err(|_| read_error.to_owned())?;
    if !metadata.file_type().is_file()
        || metadata.file_type().is_symlink()
        || metadata.len() == 0
        || metadata.len() > maximum_bytes as u64
    {
        return Err(bounds_error.to_owned());
    }
    let expected_len = usize::try_from(metadata.len()).map_err(|_| bounds_error.to_owned())?;
    let mut bytes = Vec::with_capacity(expected_len);
    File::open(path)
        .and_then(|file| {
            file.take((maximum_bytes + 1) as u64)
                .read_to_end(&mut bytes)
        })
        .map_err(|_| read_error.to_owned())?;
    if bytes.len() != expected_len || bytes.len() > maximum_bytes {
        return Err(bounds_error.to_owned());
    }
    Ok(bytes)
}

fn valid_png_image_envelope(bytes: &[u8]) -> bool {
    if bytes.len() < 45
        || !bytes.starts_with(b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR")
        || !bytes.ends_with(b"\x00\x00\x00\x00IEND\xaeB`\x82")
    {
        return false;
    }
    let width = u32::from_be_bytes(bytes[16..20].try_into().unwrap_or_default());
    let height = u32::from_be_bytes(bytes[20..24].try_into().unwrap_or_default());
    (1..=32_768).contains(&width) && (1..=32_768).contains(&height)
}

fn valid_jpeg_image_envelope(bytes: &[u8]) -> bool {
    if bytes.len() < 12 || !bytes.starts_with(&[0xff, 0xd8]) || !bytes.ends_with(&[0xff, 0xd9]) {
        return false;
    }
    let mut offset = 2usize;
    while offset + 4 <= bytes.len() - 2 {
        if bytes[offset] != 0xff {
            offset += 1;
            continue;
        }
        let marker = bytes[offset + 1];
        offset += 2;
        if marker == 0xd9 || marker == 0xda {
            break;
        }
        if marker == 0x00 || marker == 0x01 || (0xd0..=0xd7).contains(&marker) {
            continue;
        }
        if offset + 2 > bytes.len() {
            return false;
        }
        let length = usize::from(u16::from_be_bytes([bytes[offset], bytes[offset + 1]]));
        if length < 2
            || offset
                .checked_add(length)
                .is_none_or(|end| end > bytes.len())
        {
            return false;
        }
        if matches!(marker, 0xc0..=0xc3 | 0xc5..=0xc7 | 0xc9..=0xcb | 0xcd..=0xcf) {
            if length < 7 {
                return false;
            }
            let height = u16::from_be_bytes([bytes[offset + 3], bytes[offset + 4]]);
            let width = u16::from_be_bytes([bytes[offset + 5], bytes[offset + 6]]);
            return width != 0 && height != 0 && width <= 32_768 && height <= 32_768;
        }
        offset += length;
    }
    false
}

#[tauri::command]
fn read_underlay_asset_data_url(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    asset: AssetId,
) -> Result<String, String> {
    let project = lock_and_expect(
        &state,
        ProjectExpectation::new(
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
        ),
    )?;
    if !project
        .editor
        .underlays()
        .underlays
        .iter()
        .any(|underlay| underlay.asset == asset)
    {
        return Err("underlay asset is unavailable".to_owned());
    }
    let item = project
        .texture_assets
        .iter()
        .find(|item| item.id == asset)
        .ok_or_else(|| "underlay asset is unavailable".to_owned())?;
    let media = match item.media_type {
        ProjectTextureMediaTypeV1::Png => "image/png",
        ProjectTextureMediaTypeV1::Jpeg => "image/jpeg",
    };
    Ok(format!(
        "data:{media};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(&item.bytes)
    ))
}

#[tauri::command]
fn undo(
    state: State<'_, AppState>,
    foldability_state: State<'_, GlobalFlatFoldabilityState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
) -> Result<ProjectSnapshot, String> {
    let mut project = lock_project(&state)?;
    let snapshot = execute_undo(
        &mut project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
    )?;
    global_flat_foldability::invalidate_current_layer_order_after_history_mutation(
        &foldability_state,
    )
    .map_err(|_| "The layer-order authority could not be invalidated.".to_owned())?;
    Ok(snapshot)
}

#[tauri::command]
fn redo(
    state: State<'_, AppState>,
    foldability_state: State<'_, GlobalFlatFoldabilityState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
) -> Result<ProjectSnapshot, String> {
    let mut project = lock_project(&state)?;
    let snapshot = execute_redo(
        &mut project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
    )?;
    global_flat_foldability::invalidate_current_layer_order_after_history_mutation(
        &foldability_state,
    )
    .map_err(|_| "The layer-order authority could not be invalidated.".to_owned())?;
    Ok(snapshot)
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
        project.current_layer_evidence = None;
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
    project.current_layer_evidence = None;
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
        project.current_layer_evidence = None;
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
    project.current_layer_evidence = None;
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
    let mut project = lock_and_expect(
        &state,
        ProjectExpectation::new(
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
        ),
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
    execute_expected_command(
        &mut project,
        ProjectExpectation::new(
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
        ),
        Command::AppendInstructionSteps { steps },
    )
}

#[tauri::command]
fn append_generic_tree_instruction_proposal(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    expected_topology_sha256: [u8; 32],
    confirmed: bool,
) -> Result<ProjectSnapshot, String> {
    if !confirmed {
        return Err("generic_tree_instruction_confirmation_required".to_owned());
    }
    let mut project = lock_and_expect(
        &state,
        ProjectExpectation::new(
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
        ),
    )?;
    let profile = project.editor.beginner_design_profile();
    if !ori_domain::validate_beginner_design_profile_v1(profile) {
        return Err("generic_tree_instruction_provenance_invalid".to_owned());
    }
    let tree = profile
        .generation_provenance
        .as_ref()
        .and_then(|value| value.generic_tree.as_ref())
        .ok_or_else(|| "generic_tree_instruction_proof_missing".to_owned())?;
    let proposal = tree
        .instruction_proposal
        .as_ref()
        .ok_or_else(|| "generic_tree_instruction_proof_missing".to_owned())?;
    let live_topology: [u8; 32] = sha2::Sha256::digest(
        serde_json::to_vec(&profile.generation_constraints.skeleton_segments)
            .map_err(|_| "generic_tree_instruction_provenance_invalid")?,
    )
    .into();
    let live_asset_sha256 = match profile.generation_constraints.target_asset {
        Some(ori_domain::BeginnerTargetAssetReferenceV1::ReferenceImage { asset_id, .. }) => {
            project
                .texture_assets
                .iter()
                .find(|asset| asset.id == asset_id)
                .map(|asset| <[u8; 32]>::from(sha2::Sha256::digest(&asset.bytes)))
        }
        Some(ori_domain::BeginnerTargetAssetReferenceV1::ReferenceModel { asset_id }) => project
            .reference_model_assets
            .iter()
            .find(|asset| asset.id == asset_id)
            .map(|asset| <[u8; 32]>::from(sha2::Sha256::digest(&asset.bytes))),
        None => None,
    };
    if expected_topology_sha256 != tree.tree_topology_sha256
        || live_topology != tree.tree_topology_sha256
        || live_asset_sha256 != tree.asset_content_sha256
        || proposal.topology_sha256 != tree.tree_topology_sha256
        || proposal.authorizes_apply
        || proposal.physical_motion_proof
    {
        return Err("generic_tree_instruction_proposal_stale".to_owned());
    }
    let fingerprint = project.editor.fold_model_fingerprint_v1();
    let display_name = ori_domain::custom_object_display_name_v1(&profile.generation_constraints)
        .unwrap_or(ori_domain::BEGINNER_CUSTOM_OBJECT_DISPLAY_NAME_V1)
        .to_owned();
    let steps = proposal
        .steps
        .iter()
        .map(|step| InstructionStep {
            id: InstructionStepId::new(),
            title: format!(
                "{display_name}: {} {}",
                step.assignment, step.canonical_crease_id
            ),
            description: format!(
                "Fold {} at tree depth {} toward {}; keep the {} side fixed.",
                step.target_branch, step.tree_depth, step.assignment, step.fixed_side
            ),
            caution: step.caution.clone(),
            duration_ms: 1_500,
            visual: InstructionVisual::default(),
            pose: InstructionPose {
                model: InstructionPoseModel::DeclarativeOnlyV1,
                source_model_fingerprint: fingerprint.clone(),
                fixed_face: None,
                hinge_angles: Vec::new(),
            },
        })
        .collect();
    execute_expected_command(
        &mut project,
        ProjectExpectation::new(
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
        ),
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
    execute_expected_command(
        &mut project,
        ProjectExpectation::new(
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
        ),
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
    let expectation = ProjectExpectation::new(
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
    );
    let mut project = lock_project(&state)?;
    execute_expected_command(
        &mut project,
        expectation,
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
    execute_expected_command(
        &mut project,
        ProjectExpectation::new(
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
        ),
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
    let expectation = ProjectExpectation::new(
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
    );
    let mut project = lock_project(&state)?;
    execute_expected_command(
        &mut project,
        expectation,
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
    let expectation = ProjectExpectation::new(
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
    );
    let mut project = lock_project(&state)?;
    execute_expected_command(
        &mut project,
        expectation,
        Command::MoveInstructionStep {
            step_id,
            target_index,
        },
    )
}

#[tauri::command]
fn duplicate_instruction_step(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    step_id: InstructionStepId,
) -> Result<ProjectSnapshot, String> {
    let expectation = ProjectExpectation::new(
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
    );
    let mut project = lock_and_expect(&state, expectation)?;
    let step = duplicate_instruction_step_record(project.editor.instruction_timeline(), step_id)?;
    execute_expected_command(
        &mut project,
        expectation,
        Command::AddInstructionStep { step },
    )
}

fn duplicate_instruction_step_record(
    timeline: &ori_domain::InstructionTimeline,
    step_id: InstructionStepId,
) -> Result<InstructionStep, String> {
    let mut step = timeline
        .steps
        .iter()
        .find(|candidate| candidate.id == step_id)
        .cloned()
        .ok_or_else(|| "instruction_step_not_found".to_owned())?;
    step.id = InstructionStepId::new();
    // Persisted proof/provenance records are sequence-bound. A duplicate has
    // a different predecessor and compiler output, even though its pose and
    // authored visual guidance are identical.
    step.visual.path_certificate_reference_v1 = None;
    step.visual.cycle_layer_order_proof_v1 = None;
    step.visual.named_technique_compiler_v1 = None;
    Ok(step)
}

#[tauri::command]
fn split_instruction_step(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    step_id: InstructionStepId,
) -> Result<ProjectSnapshot, String> {
    let expectation = ProjectExpectation::new(
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
    );
    let mut project = lock_and_expect(&state, expectation)?;
    let mut timeline = project.editor.instruction_timeline().clone();
    let index = timeline
        .steps
        .iter()
        .position(|step| step.id == step_id)
        .ok_or_else(|| "The instruction step is unavailable.".to_owned())?;
    let total = timeline.steps[index].duration_ms;
    let first = total / 2;
    let second = total - first;
    if first < ori_domain::MIN_INSTRUCTION_DURATION_MS
        || second < ori_domain::MIN_INSTRUCTION_DURATION_MS
    {
        return Err("The instruction duration is too short to split.".to_owned());
    }
    let mut added = timeline.steps[index].clone();
    timeline.steps[index].duration_ms = first;
    added.id = InstructionStepId::new();
    added.duration_ms = second;
    timeline.steps.insert(index + 1, added);
    execute_expected_command(
        &mut project,
        expectation,
        Command::RewriteInstructionTimelineSplitMerge { timeline },
    )
}

#[tauri::command]
fn merge_adjacent_instruction_steps(
    state: State<'_, AppState>,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    first_step_id: InstructionStepId,
    second_step_id: InstructionStepId,
) -> Result<ProjectSnapshot, String> {
    let expectation = ProjectExpectation::new(
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
    );
    let mut project = lock_and_expect(&state, expectation)?;
    let mut timeline = project.editor.instruction_timeline().clone();
    let index = timeline
        .steps
        .iter()
        .position(|step| step.id == first_step_id)
        .ok_or_else(|| "The first instruction step is unavailable.".to_owned())?;
    if timeline
        .steps
        .get(index + 1)
        .is_none_or(|step| step.id != second_step_id)
    {
        return Err("The instruction steps are not adjacent.".to_owned());
    }
    let second = timeline.steps.remove(index + 1);
    timeline.steps[index].duration_ms = timeline.steps[index]
        .duration_ms
        .checked_add(second.duration_ms)
        .ok_or_else(|| "The merged instruction duration is invalid.".to_owned())?;
    execute_expected_command(
        &mut project,
        expectation,
        Command::RewriteInstructionTimelineSplitMerge { timeline },
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
    let expectation = ProjectExpectation::new(
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
    );
    let mut project = lock_project(&state)?;
    execute_expected_command(
        &mut project,
        expectation,
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
        let _project = lock_and_expect(
            &state,
            ProjectExpectation::new(
                expected_project_instance_id,
                expected_project_id,
                expected_revision,
            ),
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
    ensure_project_expectation(
        project,
        ProjectExpectation::new(
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
        ),
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
    let result = execute_expected_command(
        project,
        ProjectExpectation::new(
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
        ),
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
        let _project = lock_and_expect(
            &state,
            ProjectExpectation::new(
                expected_project_instance_id,
                expected_project_id,
                expected_revision,
            ),
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
    ensure_project_expectation(
        project,
        ProjectExpectation::new(
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
        ),
    )?;
    let total = project
        .texture_assets
        .iter()
        .try_fold(bytes.len(), |total, asset| {
            total.checked_add(asset.bytes.len())
        })
        .ok_or_else(|| "プロジェクト内テクスチャが大きすぎます。".to_owned())?;
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
    let result = execute_expected_command(
        project,
        ProjectExpectation::new(
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
        ),
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
    let expectation = ProjectExpectation::new(
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
    );
    let mut project = lock_project(&state)?;
    execute_expected_command(
        &mut project,
        expectation,
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
    let expectation = ProjectExpectation::new(
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
    );
    let mut project = lock_project(&state)?;
    execute_expected_command(
        &mut project,
        expectation,
        Command::SetLengthDisplayUnit { unit },
    )
}

fn lock_project(state: &AppState) -> Result<MutexGuard<'_, ProjectState>, String> {
    state
        .0
        .lock()
        .map_err(|_| "the project state lock is poisoned".to_owned())
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
        project.current_layer_evidence = None;
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
    project.current_layer_evidence = None;
    invalidation.commit();
    Ok(snapshot(project))
}

fn execute_expected_command(
    project: &mut ProjectState,
    expectation: ProjectExpectation,
    command: Command,
) -> Result<ProjectSnapshot, String> {
    execute_command(
        project,
        expectation.instance_id,
        expectation.project_id,
        expectation.revision,
        command,
    )
}

fn replace_with_new_project(
    project: &mut ProjectState,
    expected_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    parameters: NewProjectParameters,
) -> Result<ProjectSnapshot, String> {
    ensure_project_expectation(
        project,
        ProjectExpectation::new(expected_instance_id, expected_project_id, expected_revision),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ProjectExpectation {
    instance_id: ProjectId,
    project_id: ProjectId,
    revision: u64,
}

impl ProjectExpectation {
    const fn new(instance_id: ProjectId, project_id: ProjectId, revision: u64) -> Self {
        Self {
            instance_id,
            project_id,
            revision,
        }
    }
}

fn ensure_project_expectation(
    project: &ProjectState,
    expectation: ProjectExpectation,
) -> Result<(), String> {
    ensure_expected_project(
        project,
        expectation.instance_id,
        expectation.project_id,
        expectation.revision,
    )
}

fn lock_and_expect(
    state: &AppState,
    expectation: ProjectExpectation,
) -> Result<MutexGuard<'_, ProjectState>, String> {
    let project = lock_project(state)?;
    ensure_project_expectation(&project, expectation)?;
    Ok(project)
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
        let project = lock_and_expect(
            state,
            ProjectExpectation::new(
                expected_project_instance_id,
                expected_project_id,
                expected_revision,
            ),
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
    ensure_project_expectation(
        project,
        ProjectExpectation::new(
            expected_project_instance_id,
            expected_project_id,
            expected_revision,
        ),
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
    prepare_instruction_topology_with_cycle_policy(topology, false)
}

fn prepare_instruction_topology_with_cycle_policy(
    topology: &TopologySnapshot,
    allow_cycles: bool,
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
        if !allow_cycles && topology.hinge_adjacency.len() + 1 != topology.faces.len() {
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
        beginner_design_profile: project.editor.beginner_design_profile().clone(),
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
        annotations: project.editor.annotations().clone(),
        underlays: project.editor.underlays().clone(),
        fold_model_fingerprint: project.editor.fold_model_fingerprint_v1(),
        can_undo: project.editor.can_undo(),
        can_redo: project.editor.can_redo(),
        cutting_allowed: project.editor.cutting_allowed(),
        reference_model_assets: project
            .reference_model_assets
            .iter()
            .map(|asset| ReferenceModelAssetSummaryV1 {
                asset_id: asset.id,
                sha256: sha2::Sha256::digest(&asset.bytes).into(),
            })
            .collect(),
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
    ensure_project_expectation(
        project,
        ProjectExpectation::new(expected_instance_id, expected_project_id, expected_revision),
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
        if !contains_geometry_reference(&binding.x_source)
            && !contains_geometry_reference(&binding.y_source)
        {
            validate_coordinate_expression_pair(
                &binding.x_source,
                &binding.y_source,
                binding.adopted_x_mm,
                binding.adopted_y_mm,
            )?;
        }
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
    if document
        .numeric_expressions
        .vertex_coordinates
        .iter()
        .any(|binding| {
            contains_geometry_reference(&binding.x_source)
                || contains_geometry_reference(&binding.y_source)
        })
    {
        let staged = ProjectState::from_document(document.clone(), PathBuf::new())?;
        let resolved = reevaluate_saved_vertex_expressions(&staged)
            .map_err(|_| PROJECT_NUMERIC_EXPRESSIONS_INVALID_MESSAGE.to_owned())?;
        for binding in &document.numeric_expressions.vertex_coordinates {
            let point = resolved
                .iter()
                .find(|(vertex, _)| *vertex == binding.vertex)
                .map(|(_, point)| *point)
                .ok_or_else(|| PROJECT_NUMERIC_EXPRESSIONS_INVALID_MESSAGE.to_owned())?;
            if point.x.to_bits() != binding.adopted_x_mm.to_bits()
                || point.y.to_bits() != binding.adopted_y_mm.to_bits()
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

fn contains_geometry_reference(source: &str) -> bool {
    source.contains("v.") || source.contains("e.")
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
    ensure_project_expectation(
        project,
        ProjectExpectation::new(expected_instance_id, expected_project_id, expected_revision),
    )?;
    commit_project_replacement(project, loaded.replacement).map_err(|error| error.to_string())?;
    Ok(ProjectFileResponse {
        canceled: false,
        project: snapshot(project),
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
    let has_cycles = snapshot.hinge_adjacency.len() + 1 != snapshot.faces.len();
    let topology = prepare_instruction_topology_with_cycle_policy(snapshot, true)?;
    let cyclic_geometry = has_cycles
        .then(|| {
            let geometry = ori_kinematics::MaterialHingeGraphGeometry::prepare(
                &document.crease_pattern,
                &document.paper,
                snapshot,
                ori_kinematics::TreeKinematicsLimits::default(),
            )
            .map_err(|_| "the cyclic instruction fold graph is unsupported".to_owned())?;
            let audit = ori_kinematics::MaterialHingeGraphAudit::prepare(
                snapshot,
                ori_kinematics::TreeKinematicsLimits::default(),
            )
            .map_err(|_| "the cyclic instruction fold graph is unsupported".to_owned())?;
            Ok::<_, String>((geometry, audit))
        })
        .transpose()?;
    for (index, step) in document.instruction_timeline.steps.iter().enumerate() {
        if step.pose.model == InstructionPoseModel::DeclarativeOnlyV1
            || step.pose.source_model_fingerprint != current_fingerprint
        {
            continue;
        }
        let validated = instruction_pose_from_context(
            &topology,
            current_fingerprint.clone(),
            step.pose.fixed_face,
            step.pose.hinge_angles.clone(),
        )
        .map_err(|error| format!("instruction step {} is invalid: {error}", index + 1))?;
        if let Some((geometry, audit)) = &cyclic_geometry {
            let fixed_face = validated.fixed_face.ok_or_else(|| {
                format!("instruction step {} has no cyclic fixed face", index + 1)
            })?;
            let angles = ori_kinematics::CanonicalHingeAngles::new(
                validated
                    .hinge_angles
                    .iter()
                    .map(|hinge| ori_kinematics::HingeAngle::new(hinge.edge, hinge.angle_degrees))
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|_| format!("instruction step {} has invalid angles", index + 1))?,
            )
            .map_err(|_| format!("instruction step {} has invalid angles", index + 1))?;
            geometry
                .solve_closed(
                    audit,
                    fixed_face,
                    &angles,
                    ori_core::STACKED_FOLD_GRAPH_CLOSURE_TOLERANCE_V1,
                )
                .map_err(|_| format!("instruction step {} is not cycle-closing", index + 1))?;
        }
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

fn valid_runtime_update_token(token: &str) -> bool {
    token.len() == 36
        && token.bytes().enumerate().all(|(index, byte)| {
            matches!(index, 8 | 13 | 18 | 23) && byte == b'-'
                || !matches!(index, 8 | 13 | 18 | 23) && byte.is_ascii_hexdigit()
        })
}

#[tauri::command]
fn runtime_update_recover_pending(
    state: tauri::State<'_, runtime_update::State>,
) -> Result<&'static str, String> {
    state
        .0
        .lock()
        .map_err(|_| "disk".to_owned())?
        .recover()
        .map_err(str::to_owned)
}

#[tauri::command]
fn runtime_update_check(
    token: String,
    state: tauri::State<'_, runtime_update::State>,
) -> Result<runtime_update::Candidate, String> {
    if !valid_runtime_update_token(&token) {
        return Err("malformed".into());
    }
    *state.2.lock().map_err(|_| "disk".to_owned())? = Some(token);
    state
        .0
        .lock()
        .map_err(|_| "disk".to_owned())?
        .check()
        .map_err(str::to_owned)
}

#[tauri::command]
fn runtime_update_download_verify_stage(
    token: String,
    version: String,
    platform: String,
    state: tauri::State<'_, runtime_update::State>,
) -> Result<&'static str, String> {
    if !valid_runtime_update_token(&token) {
        return Err("malformed".into());
    }
    if state.2.lock().map_err(|_| "disk".to_owned())?.as_deref() != Some(token.as_str()) {
        return Err("malformed".into());
    }
    state
        .0
        .lock()
        .map_err(|_| "disk".to_owned())?
        .download(&version, &platform)
        .map_err(str::to_owned)
}

#[tauri::command]
fn runtime_update_apply(
    version: String,
    platform: String,
    state: tauri::State<'_, runtime_update::State>,
) -> Result<&'static str, String> {
    state
        .0
        .lock()
        .map_err(|_| "disk".to_owned())?
        .apply(&version, &platform)
        .map_err(str::to_owned)
}

#[tauri::command]
fn runtime_update_cancel(token: String, state: tauri::State<'_, runtime_update::State>) {
    if state
        .2
        .lock()
        .is_ok_and(|active| active.as_deref() == Some(token.as_str()))
    {
        state.1.store(true, std::sync::atomic::Ordering::Release);
    }
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
        .manage(DyadicPathPreviewState::default())
        .manage(runtime_update::State::default())
        .manage(ExitGuard::default())
        .invoke_handler(tauri::generate_handler![
            runtime_update_recover_pending,
            runtime_update_check,
            runtime_update_download_verify_stage,
            runtime_update_apply,
            runtime_update_cancel,
            generate_benchmark_pattern,
            project_snapshot,
            evaluate_beginner_candidates,
            cancel_reference_consensus,
            evaluate_beginner_parameter_grid,
            get_beginner_parameter_grid_progress,
            cancel_beginner_parameter_grid,
            apply_beginner_parameter_grid_candidate,
            get_beginner_symmetric_parameter_estimate,
            apply_beginner_symmetric_parameters,
            recognize_beginner_target,
            recognize_beginner_silhouette,
            recognize_beginner_outline_candidates,
            recognize_beginner_part_suggestions,
            apply_beginner_outline_candidate,
            apply_beginner_part_assignments,
            apply_beginner_generated_plan,
            update_project_memo,
            update_beginner_design_profile,
            update_beginner_reference_consensus,
            import_beginner_reference_model,
            activate_beginner_reference_model_asset,
            archive_beginner_reference_model_asset,
            get_beginner_reference_model_geometry,
            suggest_beginner_reference_model_features,
            apply_beginner_reference_model_features,
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
            prove_current_assigned_local_sufficiency_v1,
            cancel_current_assigned_local_sufficiency_summary_v1,
            summarize_current_assigned_local_sufficiency_v1,
            apply_current_native_pose,
            inspect_current_static_collision,
            analyze_geometric_constraints,
            cancel_geometric_constraint_analysis,
            evaluate_numeric_expression,
            analyze_project_topology,
            list_effective_cut_candidates_v1,
            inspect_effective_cut_read_only_v1,
            begin_global_flat_foldability,
            get_current_layer_order_view,
            current_non_flat_layer_order_view::get_current_non_flat_layer_order_view_v1,
            get_global_flat_foldability_progress,
            get_global_flat_foldability_result,
            cancel_global_flat_foldability,
            propose_current_stacked_fold_read,
            propose_current_cycle_pose_v1,
            cancel_current_stacked_fold_read_v1,
            read_even_cycle_candidates_v1,
            read_bounded_dyadic_pose_graph_v1,
            mint_dyadic_pose_path_preview_v1,
            apply_dyadic_pose_path_preview_v1,
            cancel_dyadic_pose_path_preview_v1,
            read_live_hinge_registry_v1,
            cancel_stacked_fold_transaction_preview,
            apply_stacked_fold_transaction,
            preview_named_basic_fold_timeline,
            apply_named_book_fold_transaction,
            apply_named_reverse_fold_transaction,
            apply_named_sink_fold_transaction,
            apply_named_layer_selective_transaction,
            apply_named_accordion_fold_transaction,
            open_project,
            save_project,
            save_project_as,
            list_recent_projects,
            open_recent_project,
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
            prepare_fold_3d_instruction_timeline,
            apply_fold_3d_instruction_timeline,
            cancel_fold_3d_frames,
            apply_fold_import,
            cancel_fold_import,
            preview_svg_import,
            validate_svg_import_settings,
            apply_svg_import,
            cancel_svg_import,
            add_vertex,
            move_vertex,
            move_edge,
            mirror_edge_left_right,
            preflight_mirror_selection,
            apply_mirror_selection,
            preview_linear_array,
            confirm_linear_array,
            preview_radial_array,
            confirm_radial_array,
            rotate_edge_about_point,
            move_vertices,
            preview_geometric_constraint_solve,
            preview_geometric_constraint_edge_solve,
            preview_geometric_constraint_expression_solve,
            apply_geometric_constraint_solve,
            remove_vertex,
            add_edge,
            add_ray_to_first_target,
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
            add_annotation,
            update_annotation,
            remove_annotation,
            add_underlay,
            update_underlay,
            remove_underlay,
            import_underlay_image,
            read_underlay_asset_data_url,
            undo,
            redo,
            add_instruction_step,
            append_named_technique_instruction_steps,
            append_generic_tree_instruction_proposal,
            update_instruction_step_metadata,
            replace_instruction_step_pose,
            remove_instruction_step,
            move_instruction_step,
            duplicate_instruction_step,
            split_instruction_step,
            merge_adjacent_instruction_steps,
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
            repair_all_unsplit_intersections,
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
mod tests;
