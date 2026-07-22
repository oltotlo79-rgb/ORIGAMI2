use std::{
    io::{Cursor, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicBool, AtomicU8, Ordering},
    },
};

use ori_core::TopologyAnalysisInput;
use ori_domain::{InstructionPoseModel, ProjectId};
use ori_formats::{
    INSTRUCTION_EXPORT_PROFILE, INSTRUCTION_EXPORT_WARNINGS,
    INSTRUCTION_PROJECTION_PROFILE as INSTRUCTION_EXPORT_PROJECTION_PROFILE,
    InstructionDiagramError, InstructionExportArtifact, InstructionExportError,
    InstructionExportFormat, InstructionExportWarning, export_instruction_document,
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

#[cfg(test)]
use super::crease_export::persist_export_bytes_atomically;
use super::crease_export::persist_export_bytes_to_destination;
use super::save_path::DialogSaveDestination;
use super::{
    AppState, ProjectState, ensure_expected_project, ensure_project_identity, lock_project,
};

const PHASE_VALIDATING: u8 = 0;
const PHASE_ANALYZING_TOPOLOGY: u8 = 1;
const PHASE_BUILDING_DOCUMENT: u8 = 2;
const PHASE_READY: u8 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum InstructionExportErrorCategory {
    StateUnavailable,
    GenerationUnavailable,
    GenerationReplaced,
    GenerationCancelled,
    ProjectChanged,
    TimelineEmpty,
    TimelineStale,
    SourceLimitExceeded,
    TopologyUnsupported,
    DocumentInputInvalid,
    DocumentLimitExceeded,
    DocumentGenerationFailed,
    DocumentContractInvalid,
    WarningAcknowledgementRequired,
    SaveTargetInvalid,
    SaveFailed,
    // Stable fail-closed IPC category. Keep this wire value even when every
    // currently known failure boundary has a more specific category.
    #[allow(dead_code)]
    UnexpectedFailure,
}

impl InstructionExportErrorCategory {
    const fn message_ja(self) -> &'static str {
        match self {
            Self::StateUnavailable => {
                "折り図書き出しの状態を利用できません。アプリを再起動してください。"
            }
            Self::GenerationUnavailable => {
                "この折り図生成は利用できません。現在の編集内容から作り直してください。"
            }
            Self::GenerationReplaced => "この折り図生成は新しい処理に置き換えられました。",
            Self::GenerationCancelled => "折り図の生成はキャンセルされました。",
            Self::ProjectChanged => {
                "生成を開始した後に編集内容が変わりました。現在の編集内容から作り直してください。"
            }
            Self::TimelineEmpty => "折り手順が1件もないため、折り図を書き出せません。",
            Self::TimelineStale => {
                "現在の展開図より古い折り手順があります。該当する姿勢を取り直してください。"
            }
            Self::SourceLimitExceeded => "折り図の元データが初版の処理上限を超えています。",
            Self::TopologyUnsupported => {
                "現在の展開図は3D折り図を生成できる面構造になっていません。"
            }
            Self::DocumentInputInvalid => "折り図に含められない文字または手順情報があります。",
            Self::DocumentLimitExceeded => {
                "折り図のページ数またはデータ量が初版の出力上限を超えています。"
            }
            Self::DocumentGenerationFailed => "折り図データを生成できませんでした。",
            Self::DocumentContractInvalid => "生成された折り図が対応する出力仕様と一致しません。",
            Self::WarningAcknowledgementRequired => "折り図の制約に関する確認が必要です。",
            Self::SaveTargetInvalid => "選択された保存先を折り図の保存先として使用できません。",
            Self::SaveFailed => {
                "折り図ファイルを安全に保存できませんでした。保存先を変えて再試行してください。"
            }
            Self::UnexpectedFailure => "折り図書き出しを完了できませんでした。",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(super) struct InstructionExportCommandError {
    category: InstructionExportErrorCategory,
    message_ja: &'static str,
}

impl InstructionExportCommandError {
    const fn new(category: InstructionExportErrorCategory) -> Self {
        Self {
            category,
            message_ja: category.message_ja(),
        }
    }
}

impl From<InstructionExportErrorCategory> for InstructionExportCommandError {
    fn from(category: InstructionExportErrorCategory) -> Self {
        Self::new(category)
    }
}

type InstructionExportResult<T> = Result<T, InstructionExportErrorCategory>;

#[derive(Default)]
pub(super) struct InstructionExportState(Mutex<InstructionExportSlot>);

#[derive(Default)]
struct InstructionExportSlot {
    active: Option<ActiveInstructionExport>,
    pending: Option<PendingInstructionExport>,
    last_cancelled_id: Option<ProjectId>,
}

struct ActiveInstructionExport {
    export_id: ProjectId,
    cancellation: Arc<AtomicBool>,
    phase: Arc<AtomicU8>,
    claimed_format: Option<InstructionExportFormatRequest>,
}

#[derive(Clone)]
struct PendingInstructionExport {
    export_id: ProjectId,
    expected_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    topology_input: TopologyAnalysisInput,
    format: InstructionExportFormatRequest,
    profile: &'static str,
    projection_profile: &'static str,
    format_summary: String,
    suggested_file_name: String,
    bytes: Arc<[u8]>,
    step_count: usize,
    page_count: usize,
    caution_count: usize,
    warnings: Vec<InstructionExportWarningSnapshot>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum InstructionExportFormatRequest {
    Pdf,
    SvgZip,
}

impl InstructionExportFormatRequest {
    fn exporter_format(self) -> InstructionExportFormat {
        match self {
            Self::Pdf => InstructionExportFormat::Pdf17,
            Self::SvgZip => InstructionExportFormat::SvgPageZip,
        }
    }

    fn extension(self) -> &'static str {
        match self {
            Self::Pdf => "pdf",
            Self::SvgZip => "zip",
        }
    }

    fn media_type(self) -> &'static str {
        match self {
            Self::Pdf => "application/pdf",
            Self::SvgZip => "application/zip",
        }
    }

    fn filter_label(self) -> &'static str {
        match self {
            Self::Pdf => "PDF 1.7 折り図",
            Self::SvgZip => "SVG 折り図画像 ZIP",
        }
    }

    fn format_summary(self) -> &'static str {
        match self {
            Self::Pdf => "PDF 1.7・A4縦・固定アイソメトリック投影・複数ページ",
            Self::SvgZip => "SVGページ画像・固定アイソメトリック投影・ZIPアーカイブ",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct InstructionExportPreviewResponse {
    preview: InstructionExportPreviewSnapshot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(super) struct InstructionExportBeginResponse {
    export_id: ProjectId,
    profile: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(super) struct InstructionExportProgressResponse {
    export_id: ProjectId,
    phase: InstructionExportPhase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum InstructionExportPhase {
    Validating,
    AnalyzingTopology,
    BuildingDocument,
    Ready,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct InstructionExportPreviewSnapshot {
    export_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    format: InstructionExportFormatRequest,
    profile: &'static str,
    projection_profile: &'static str,
    format_summary: String,
    suggested_file_name: String,
    byte_count: usize,
    step_count: usize,
    page_count: usize,
    caution_count: usize,
    warnings: Vec<InstructionExportWarningSnapshot>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
struct InstructionExportWarningSnapshot {
    category: &'static str,
    message_ja: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(super) struct InstructionExportSaveResponse {
    canceled: bool,
}

struct InstructionExportSource {
    export_id: ProjectId,
    expected_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    topology_input: TopologyAnalysisInput,
    format: InstructionExportFormatRequest,
    name: String,
    pattern: ori_domain::CreasePattern,
    paper: ori_domain::Paper,
    timeline: ori_domain::InstructionTimeline,
    current_fold_model_fingerprint: String,
    cancellation: Arc<AtomicBool>,
    phase: Arc<AtomicU8>,
    consensus_summary: Option<ori_domain::BeginnerReferenceConsensusSummaryV1>,
}

#[tauri::command]
pub(super) fn begin_instruction_export(
    export_state: State<'_, InstructionExportState>,
) -> Result<InstructionExportBeginResponse, InstructionExportCommandError> {
    let export_id = ProjectId::new();
    begin_export_generation(&export_state, export_id)?;
    Ok(InstructionExportBeginResponse {
        export_id,
        profile: INSTRUCTION_EXPORT_PROFILE,
    })
}

#[tauri::command]
pub(super) async fn preview_instruction_export(
    state: State<'_, AppState>,
    export_state: State<'_, InstructionExportState>,
    export_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    format: InstructionExportFormatRequest,
) -> Result<InstructionExportPreviewResponse, InstructionExportCommandError> {
    let (cancellation, phase) = claim_generation(&export_state, export_id, format)?;
    let source = match capture_export_source(
        &state,
        export_id,
        expected_project_id,
        expected_revision,
        format,
        cancellation,
        phase,
    ) {
        Ok(source) => source,
        Err(error) => {
            abandon_export_generation(&export_state, export_id)?;
            return Err(error.into());
        }
    };

    let built =
        match tauri::async_runtime::spawn_blocking(move || build_pending_export(source)).await {
            Ok(built) => built,
            Err(_) => {
                abandon_export_generation(&export_state, export_id)?;
                return Err(InstructionExportErrorCategory::DocumentGenerationFailed.into());
            }
        };
    let pending = match built {
        Ok(pending) => pending,
        Err(error) => {
            abandon_export_generation(&export_state, export_id)?;
            return Err(error.into());
        }
    };

    let mut slot = lock_instruction_export(&export_state)?;
    let project =
        lock_project(&state).map_err(|_| InstructionExportErrorCategory::StateUnavailable)?;
    ensure_generation_is_current(&slot, export_id)?;
    if let Err(error) = ensure_pending_is_current(&pending, &project) {
        slot.active = None;
        slot.pending = None;
        return Err(error.into());
    }
    let preview = preview_snapshot(&pending);
    slot.pending = Some(pending);
    Ok(InstructionExportPreviewResponse { preview })
}

#[tauri::command]
pub(super) fn get_instruction_export_progress(
    state: State<'_, InstructionExportState>,
    export_id: ProjectId,
) -> Result<InstructionExportProgressResponse, InstructionExportCommandError> {
    let slot = lock_instruction_export(&state)?;
    ensure_generation_is_current(&slot, export_id)?;
    let active = slot
        .active
        .as_ref()
        .ok_or(InstructionExportErrorCategory::GenerationUnavailable)?;
    Ok(InstructionExportProgressResponse {
        export_id,
        phase: instruction_export_phase(active.phase.load(Ordering::SeqCst))?,
    })
}

#[tauri::command]
pub(super) async fn save_instruction_export(
    app: AppHandle,
    state: State<'_, AppState>,
    export_state: State<'_, InstructionExportState>,
    export_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    warnings_acknowledged: bool,
) -> Result<InstructionExportSaveResponse, InstructionExportCommandError> {
    let (pending, initial_directory) = {
        let slot = lock_instruction_export(&export_state)?;
        let project =
            lock_project(&state).map_err(|_| InstructionExportErrorCategory::StateUnavailable)?;
        let pending = checked_pending(
            &slot,
            &project,
            export_id,
            expected_project_id,
            expected_revision,
        )?;
        require_warning_acknowledgement(pending, warnings_acknowledged)?;
        (
            pending.clone(),
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
        .add_filter(pending.format.filter_label(), &[pending.format.extension()])
        .set_file_name(pending.suggested_file_name.clone())
        .set_title("折り図を書き出す");
    if let Some(directory) = initial_directory {
        dialog = dialog.set_directory(directory);
    }
    let Some(selected) = dialog.blocking_save_file() else {
        let slot = lock_instruction_export(&export_state)?;
        let project =
            lock_project(&state).map_err(|_| InstructionExportErrorCategory::StateUnavailable)?;
        let pending = checked_pending(
            &slot,
            &project,
            export_id,
            expected_project_id,
            expected_revision,
        )?;
        require_warning_acknowledgement(pending, warnings_acknowledged)?;
        return Ok(InstructionExportSaveResponse { canceled: true });
    };
    let selected_path = selected
        .simplified()
        .into_path()
        .map_err(|_| InstructionExportErrorCategory::SaveTargetInvalid)?;
    let destination = ensure_export_extension(selected_path, pending.format)?;

    let mut slot = lock_instruction_export(&export_state)?;
    let project =
        lock_project(&state).map_err(|_| InstructionExportErrorCategory::StateUnavailable)?;
    commit_pending_export_to_destination(
        &mut slot,
        &project,
        export_id,
        expected_project_id,
        expected_revision,
        warnings_acknowledged,
        &destination,
    )?;
    Ok(InstructionExportSaveResponse { canceled: false })
}

#[tauri::command]
pub(super) fn cancel_instruction_export(
    state: State<'_, InstructionExportState>,
    export_id: ProjectId,
) -> Result<(), InstructionExportCommandError> {
    cancel_export_generation(&state, export_id).map_err(InstructionExportCommandError::from)
}

fn lock_instruction_export(
    state: &InstructionExportState,
) -> InstructionExportResult<MutexGuard<'_, InstructionExportSlot>> {
    state
        .0
        .lock()
        .map_err(|_| InstructionExportErrorCategory::StateUnavailable)
}

fn begin_export_generation(
    state: &InstructionExportState,
    export_id: ProjectId,
) -> InstructionExportResult<Arc<AtomicBool>> {
    let mut slot = lock_instruction_export(state)?;
    if let Some(active) = slot.active.take() {
        active.cancellation.store(true, Ordering::SeqCst);
    }
    let cancellation = Arc::new(AtomicBool::new(false));
    let phase = Arc::new(AtomicU8::new(PHASE_VALIDATING));
    slot.pending = None;
    slot.active = Some(ActiveInstructionExport {
        export_id,
        cancellation: Arc::clone(&cancellation),
        phase,
        claimed_format: None,
    });
    Ok(cancellation)
}

fn claim_generation(
    state: &InstructionExportState,
    export_id: ProjectId,
    format: InstructionExportFormatRequest,
) -> InstructionExportResult<(Arc<AtomicBool>, Arc<AtomicU8>)> {
    let mut slot = lock_instruction_export(state)?;
    ensure_generation_is_current(&slot, export_id)?;
    if slot.pending.is_some() {
        return Err(InstructionExportErrorCategory::GenerationUnavailable);
    }
    let active = slot
        .active
        .as_mut()
        .ok_or(InstructionExportErrorCategory::GenerationUnavailable)?;
    if active.claimed_format.is_some() {
        return Err(InstructionExportErrorCategory::GenerationUnavailable);
    }
    if active.phase.load(Ordering::SeqCst) != PHASE_VALIDATING {
        return Err(InstructionExportErrorCategory::GenerationUnavailable);
    }
    active.claimed_format = Some(format);
    Ok((Arc::clone(&active.cancellation), Arc::clone(&active.phase)))
}

fn advance_generation_phase(phase: &AtomicU8, next: u8) -> InstructionExportResult<()> {
    if next > PHASE_READY {
        return Err(InstructionExportErrorCategory::GenerationUnavailable);
    }
    let mut current = phase.load(Ordering::SeqCst);
    loop {
        if current > next {
            return Err(InstructionExportErrorCategory::DocumentContractInvalid);
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

fn instruction_export_phase(value: u8) -> InstructionExportResult<InstructionExportPhase> {
    match value {
        PHASE_VALIDATING => Ok(InstructionExportPhase::Validating),
        PHASE_ANALYZING_TOPOLOGY => Ok(InstructionExportPhase::AnalyzingTopology),
        PHASE_BUILDING_DOCUMENT => Ok(InstructionExportPhase::BuildingDocument),
        PHASE_READY => Ok(InstructionExportPhase::Ready),
        _ => Err(InstructionExportErrorCategory::GenerationUnavailable),
    }
}

fn abandon_export_generation(
    state: &InstructionExportState,
    export_id: ProjectId,
) -> InstructionExportResult<()> {
    let mut slot = lock_instruction_export(state)?;
    if slot.active.as_ref().map(|active| active.export_id) == Some(export_id) {
        slot.active = None;
        slot.pending = None;
    }
    Ok(())
}

fn ensure_generation_is_current(
    slot: &InstructionExportSlot,
    export_id: ProjectId,
) -> InstructionExportResult<()> {
    match slot.active.as_ref() {
        Some(active)
            if active.export_id == export_id && !active.cancellation.load(Ordering::SeqCst) =>
        {
            Ok(())
        }
        Some(_) => Err(InstructionExportErrorCategory::GenerationReplaced),
        None if slot.last_cancelled_id == Some(export_id) => {
            Err(InstructionExportErrorCategory::GenerationCancelled)
        }
        None => Err(InstructionExportErrorCategory::GenerationUnavailable),
    }
}

fn ensure_not_cancelled(cancellation: &AtomicBool) -> InstructionExportResult<()> {
    if cancellation.load(Ordering::SeqCst) {
        Err(InstructionExportErrorCategory::GenerationCancelled)
    } else {
        Ok(())
    }
}

fn capture_export_source(
    state: &AppState,
    export_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    format: InstructionExportFormatRequest,
    cancellation: Arc<AtomicBool>,
    phase: Arc<AtomicU8>,
) -> InstructionExportResult<InstructionExportSource> {
    ensure_not_cancelled(&cancellation)?;
    let project =
        lock_project(state).map_err(|_| InstructionExportErrorCategory::StateUnavailable)?;
    ensure_project_identity(&project, expected_project_id)
        .map_err(|_| InstructionExportErrorCategory::ProjectChanged)?;
    if project.editor.revision() != expected_revision {
        return Err(InstructionExportErrorCategory::ProjectChanged);
    }
    let timeline = project.editor.instruction_timeline().clone();
    if timeline.steps.is_empty() {
        return Err(InstructionExportErrorCategory::TimelineEmpty);
    }
    validate_source_counts(
        project.editor.pattern().vertices.len(),
        project.editor.pattern().edges.len(),
    )?;
    let current_fold_model_fingerprint = project.editor.fold_model_fingerprint_v1();
    if timeline.steps.iter().any(|step| {
        step.pose.model == InstructionPoseModel::AbsoluteHingeAnglesV1
            && step.pose.source_model_fingerprint != current_fold_model_fingerprint
    }) {
        return Err(InstructionExportErrorCategory::TimelineStale);
    }
    let generation_provenance = project
        .editor
        .beginner_design_profile()
        .generation_provenance
        .as_ref();
    if generation_provenance
        .is_some_and(|value| !ori_domain::validate_beginner_generation_provenance_v1(value))
    {
        return Err(InstructionExportErrorCategory::DocumentContractInvalid);
    }
    Ok(InstructionExportSource {
        export_id,
        expected_instance_id: project.instance_id,
        expected_project_id: project.project_id,
        expected_revision,
        topology_input: project.editor.topology_analysis_input(project.project_id),
        format,
        name: project.name.clone(),
        pattern: project.editor.pattern().clone(),
        paper: project.editor.paper().clone(),
        timeline,
        current_fold_model_fingerprint,
        cancellation,
        phase,
        consensus_summary: generation_provenance
            .and_then(|value| value.reference_consensus_summary.clone()),
    })
}

fn build_pending_export(
    source: InstructionExportSource,
) -> InstructionExportResult<PendingInstructionExport> {
    ensure_not_cancelled(&source.cancellation)?;
    validate_source_counts(source.pattern.vertices.len(), source.pattern.edges.len())?;
    advance_generation_phase(&source.phase, PHASE_ANALYZING_TOPOLOGY)?;
    let topology = source.topology_input.analyze();
    ensure_not_cancelled(&source.cancellation)?;
    if topology.revision() != source.expected_revision {
        return Err(InstructionExportErrorCategory::DocumentContractInvalid);
    }
    let snapshot = topology
        .simulation_snapshot()
        .ok_or(InstructionExportErrorCategory::TopologyUnsupported)?;
    advance_generation_phase(&source.phase, PHASE_BUILDING_DOCUMENT)?;
    let mut artifact = export_instruction_document(
        source.format.exporter_format(),
        &source.name,
        &source.current_fold_model_fingerprint,
        &source.pattern,
        &source.paper,
        &source.timeline,
        snapshot,
    )
    .map_err(instruction_document_failure_category)?;
    ensure_not_cancelled(&source.cancellation)?;
    validate_artifact_contract(source.format, &source.timeline, &artifact)?;
    if let Some(summary) = &source.consensus_summary {
        embed_instruction_consensus_summary(&mut artifact, source.format, summary)?;
    }
    let warnings = instruction_export_warning_snapshots(&artifact.warnings);
    advance_generation_phase(&source.phase, PHASE_READY)?;
    Ok(PendingInstructionExport {
        export_id: source.export_id,
        expected_instance_id: source.expected_instance_id,
        expected_project_id: source.expected_project_id,
        expected_revision: source.expected_revision,
        topology_input: source.topology_input,
        format: source.format,
        profile: artifact.profile,
        projection_profile: artifact.projection_profile,
        format_summary: source.format.format_summary().to_owned(),
        suggested_file_name: suggested_export_file_name(&source.name, source.format.extension()),
        bytes: Arc::from(artifact.bytes),
        step_count: artifact.step_count,
        page_count: artifact.page_count,
        caution_count: artifact.caution_count,
        warnings,
    })
}

fn embed_instruction_consensus_summary(
    artifact: &mut InstructionExportArtifact,
    format: InstructionExportFormatRequest,
    summary: &ori_domain::BeginnerReferenceConsensusSummaryV1,
) -> InstructionExportResult<()> {
    let json = serde_json::to_vec(summary)
        .map_err(|_| InstructionExportErrorCategory::DocumentContractInvalid)?;
    match format {
        InstructionExportFormatRequest::Pdf => {
            artifact
                .bytes
                .extend_from_slice(b"\n% ORIGAMI2_REFERENCE_CONSENSUS_SUMMARY ");
            artifact.bytes.extend_from_slice(&json);
            artifact.bytes.push(b'\n');
        }
        InstructionExportFormatRequest::SvgZip => {
            let cursor = Cursor::new(std::mem::take(&mut artifact.bytes));
            let mut archive = zip::ZipWriter::new_append(cursor)
                .map_err(|_| InstructionExportErrorCategory::DocumentContractInvalid)?;
            archive
                .start_file(
                    "origami2-reference-consensus-summary.json",
                    zip::write::SimpleFileOptions::default()
                        .compression_method(zip::CompressionMethod::Deflated),
                )
                .map_err(|_| InstructionExportErrorCategory::DocumentContractInvalid)?;
            archive
                .write_all(&json)
                .map_err(|_| InstructionExportErrorCategory::DocumentContractInvalid)?;
            artifact.bytes = archive
                .finish()
                .map_err(|_| InstructionExportErrorCategory::DocumentContractInvalid)?
                .into_inner();
        }
    }
    Ok(())
}

fn instruction_document_failure_category(
    error: InstructionExportError,
) -> InstructionExportErrorCategory {
    match error {
        InstructionExportError::TitleTooLong { .. }
        | InstructionExportError::InvalidTitle
        | InstructionExportError::InvalidPathCertificateReference { .. }
        | InstructionExportError::UnsupportedGlyph { .. } => {
            InstructionExportErrorCategory::DocumentInputInvalid
        }
        InstructionExportError::Diagram(error) => match error {
            InstructionDiagramError::InvalidTimeline => {
                InstructionExportErrorCategory::DocumentInputInvalid
            }
            InstructionDiagramError::EmptyTimeline => InstructionExportErrorCategory::TimelineEmpty,
            InstructionDiagramError::StaleStep { .. } => {
                InstructionExportErrorCategory::TimelineStale
            }
            InstructionDiagramError::UnsupportedTopology
            | InstructionDiagramError::UnrepresentableGeometry => {
                InstructionExportErrorCategory::TopologyUnsupported
            }
            InstructionDiagramError::ResourceLimitExceeded => {
                InstructionExportErrorCategory::DocumentLimitExceeded
            }
        },
        InstructionExportError::LayoutLimitExceeded
        | InstructionExportError::PageTooLarge { .. }
        | InstructionExportError::OutputTooLarge { .. } => {
            InstructionExportErrorCategory::DocumentLimitExceeded
        }
        InstructionExportError::InvalidBundledFont
        | InstructionExportError::FontAssetMismatch
        | InstructionExportError::StructureNotRepresentable
        | InstructionExportError::Zip(_)
        | InstructionExportError::Io(_)
        | InstructionExportError::Json(_) => {
            InstructionExportErrorCategory::DocumentGenerationFailed
        }
    }
}

fn validate_source_counts(vertex_count: usize, edge_count: usize) -> InstructionExportResult<()> {
    let limits = ori_formats::InstructionDiagramLimits::default();
    if vertex_count > limits.max_source_vertices || edge_count > limits.max_source_edges {
        return Err(InstructionExportErrorCategory::SourceLimitExceeded);
    }
    Ok(())
}

fn validate_artifact_contract(
    requested: InstructionExportFormatRequest,
    timeline: &ori_domain::InstructionTimeline,
    artifact: &InstructionExportArtifact,
) -> InstructionExportResult<()> {
    if artifact.format != requested.exporter_format()
        || artifact.file_extension != requested.extension()
        || artifact.media_type != requested.media_type()
        || artifact.profile != INSTRUCTION_EXPORT_PROFILE
        || artifact.projection_profile != INSTRUCTION_EXPORT_PROJECTION_PROFILE
        || artifact.warnings != INSTRUCTION_EXPORT_WARNINGS
    {
        return Err(InstructionExportErrorCategory::DocumentContractInvalid);
    }
    let caution_count = timeline
        .steps
        .iter()
        .filter(|step| !step.caution.is_empty())
        .count();
    if artifact.step_count != timeline.steps.len()
        || artifact.caution_count != caution_count
        || artifact.page_count < artifact.step_count
        || artifact.glyph_count == 0
        || artifact.projected_vertex_visits == 0
        || artifact.bytes.is_empty()
    {
        return Err(InstructionExportErrorCategory::DocumentContractInvalid);
    }
    Ok(())
}

fn instruction_export_warning_snapshots(
    warnings: &[InstructionExportWarning],
) -> Vec<InstructionExportWarningSnapshot> {
    warnings
        .iter()
        .map(|warning| InstructionExportWarningSnapshot {
            category: warning.category(),
            message_ja: warning.message_ja(),
        })
        .collect()
}

fn preview_snapshot(pending: &PendingInstructionExport) -> InstructionExportPreviewSnapshot {
    InstructionExportPreviewSnapshot {
        export_id: pending.export_id,
        expected_project_id: pending.expected_project_id,
        expected_revision: pending.expected_revision,
        format: pending.format,
        profile: pending.profile,
        projection_profile: pending.projection_profile,
        format_summary: pending.format_summary.clone(),
        suggested_file_name: pending.suggested_file_name.clone(),
        byte_count: pending.bytes.len(),
        step_count: pending.step_count,
        page_count: pending.page_count,
        caution_count: pending.caution_count,
        warnings: pending.warnings.clone(),
    }
}

fn ensure_pending_is_current(
    pending: &PendingInstructionExport,
    project: &ProjectState,
) -> InstructionExportResult<()> {
    ensure_expected_project(
        project,
        pending.expected_instance_id,
        pending.expected_project_id,
        pending.expected_revision,
    )
    .map_err(|_| InstructionExportErrorCategory::ProjectChanged)?;
    if !pending
        .topology_input
        .is_current_for(project.project_id, &project.editor)
    {
        return Err(InstructionExportErrorCategory::ProjectChanged);
    }
    Ok(())
}

fn checked_pending<'a>(
    slot: &'a InstructionExportSlot,
    project: &ProjectState,
    export_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
) -> InstructionExportResult<&'a PendingInstructionExport> {
    let pending = slot
        .pending
        .as_ref()
        .ok_or(InstructionExportErrorCategory::GenerationUnavailable)?;
    if pending.export_id != export_id {
        return Err(InstructionExportErrorCategory::GenerationReplaced);
    }
    if pending.expected_project_id != expected_project_id
        || pending.expected_revision != expected_revision
    {
        return Err(InstructionExportErrorCategory::ProjectChanged);
    }
    ensure_generation_is_current(slot, export_id)?;
    let active = slot
        .active
        .as_ref()
        .ok_or(InstructionExportErrorCategory::GenerationUnavailable)?;
    if active.claimed_format != Some(pending.format) {
        return Err(InstructionExportErrorCategory::GenerationUnavailable);
    }
    ensure_pending_is_current(pending, project)?;
    Ok(pending)
}

fn require_warning_acknowledgement(
    pending: &PendingInstructionExport,
    warnings_acknowledged: bool,
) -> InstructionExportResult<()> {
    if !pending.warnings.is_empty() && !warnings_acknowledged {
        Err(InstructionExportErrorCategory::WarningAcknowledgementRequired)
    } else {
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
#[cfg(test)]
fn commit_pending_export_to_path(
    slot: &mut InstructionExportSlot,
    project: &ProjectState,
    export_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    warnings_acknowledged: bool,
    path: &Path,
) -> InstructionExportResult<()> {
    commit_pending_export_to_destination(
        slot,
        project,
        export_id,
        expected_project_id,
        expected_revision,
        warnings_acknowledged,
        &DialogSaveDestination::confirmed(path.to_path_buf()),
    )
}

#[allow(clippy::too_many_arguments)]
fn commit_pending_export_to_destination(
    slot: &mut InstructionExportSlot,
    project: &ProjectState,
    export_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    warnings_acknowledged: bool,
    destination: &DialogSaveDestination,
) -> InstructionExportResult<()> {
    let pending = checked_pending(
        slot,
        project,
        export_id,
        expected_project_id,
        expected_revision,
    )?;
    require_warning_acknowledgement(pending, warnings_acknowledged)?;
    persist_instruction_export_bytes_to_destination(destination, &pending.bytes)?;
    slot.pending = None;
    slot.active = None;
    Ok(())
}

#[cfg(test)]
fn persist_instruction_export_bytes(path: &Path, bytes: &[u8]) -> InstructionExportResult<()> {
    if path.file_name().is_none() {
        return Err(InstructionExportErrorCategory::SaveTargetInvalid);
    }
    persist_export_bytes_atomically(path, bytes)
        .map_err(|_| InstructionExportErrorCategory::SaveFailed)
}

fn persist_instruction_export_bytes_to_destination(
    destination: &DialogSaveDestination,
    bytes: &[u8],
) -> InstructionExportResult<()> {
    if destination.path().file_name().is_none() {
        return Err(InstructionExportErrorCategory::SaveTargetInvalid);
    }
    persist_export_bytes_to_destination(destination, bytes)
        .map_err(|_| InstructionExportErrorCategory::SaveFailed)
}

fn cancel_export_generation(
    state: &InstructionExportState,
    export_id: ProjectId,
) -> InstructionExportResult<()> {
    let mut slot = lock_instruction_export(state)?;
    if slot.active.as_ref().map(|active| active.export_id) == Some(export_id) {
        if let Some(active) = slot.active.take() {
            active.cancellation.store(true, Ordering::SeqCst);
        }
        slot.pending = None;
        slot.last_cancelled_id = Some(export_id);
        return Ok(());
    }
    if slot.last_cancelled_id == Some(export_id) {
        return Ok(());
    }
    if slot.active.is_some() {
        return Err(InstructionExportErrorCategory::GenerationReplaced);
    }
    Err(InstructionExportErrorCategory::GenerationUnavailable)
}

fn suggested_export_file_name(project_name: &str, extension: &str) -> String {
    let mut sanitized = String::new();
    for character in project_name.trim().chars().take(76) {
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
        "Untitled"
    } else {
        sanitized
    };
    format!("{base}-折り図.{extension}")
}

fn ensure_export_extension(
    path: PathBuf,
    format: InstructionExportFormatRequest,
) -> InstructionExportResult<DialogSaveDestination> {
    super::save_path::normalize_dialog_save_path(path, format.extension())
        .map_err(|_| InstructionExportErrorCategory::SaveTargetInvalid)
}

#[cfg(test)]
pub(crate) mod tests {
    use std::{
        collections::BTreeSet,
        fs,
        io::{Cursor, Read},
        sync::{
            Barrier,
            atomic::{AtomicU64, Ordering as AtomicOrdering},
        },
        thread,
    };

    use ori_core::Command;
    use ori_domain::{
        EdgeId, EdgeKind, InstructionHingeAngle, InstructionPose, InstructionPoseModel,
        InstructionStep, InstructionStepId, Point2, VertexId,
    };
    use sha2::{Digest, Sha256};

    use ori_instructions::{
        AccordionFoldMotionRequestV1, BasicFoldKindV1, BasicFoldMotionRequestV1,
        BookFoldMotionRequestV1, CrimpFoldMotionRequestV1, FOLD_TECHNIQUE_FILE_SCHEMA_V1,
        FOLD_TECHNIQUE_FILE_VERSION_V1, FoldTechniqueActionV1, FoldTechniqueCapabilityV1,
        FoldTechniqueExecutionSupportV1, FoldTechniqueFileDocumentV1, FoldTechniqueFileV1,
        FoldTechniqueLocalizedTextV1, FoldTechniqueMetadataV1, FoldTechniqueOperationV1,
        FoldTechniqueParameterBindingV1, FoldTechniqueParameterDefinitionV1,
        FoldTechniqueParameterTypeV1, FoldTechniqueSinkKindV1, FoldTechniqueSourceV1,
        FoldTechniqueTemplateV1, FoldTechniqueUnsupportedPhysicalOperationV1,
        LayerSelectiveMotionRequestV1, PetalFoldMotionRequestV1, ReverseFoldKindV1,
        ReverseFoldMotionRequestV1, SinkFoldMotionRequestV1, SquashFoldMotionRequestV1,
        compile_certified_accordion_fold_timeline_v1, compile_certified_basic_fold_timeline_v1,
        compile_certified_book_fold_timeline_v1, compile_certified_crimp_fold_timeline_v1,
        compile_certified_layer_selective_timeline_v1, compile_certified_petal_fold_timeline_v1,
        compile_certified_reverse_fold_timeline_v1, compile_certified_sink_fold_timeline_v1,
        compile_certified_squash_fold_timeline_v1, instruction_pose_fingerprint_v1,
        validate_fold_technique_file_v1,
    };

    use super::*;

    static NEXT_TEST_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let id = NEXT_TEST_DIRECTORY.fetch_add(1, AtomicOrdering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "origami2-instruction-export-test-{}-{id}",
                std::process::id()
            ));
            fs::create_dir(&path).unwrap();
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

    fn project_with_instruction() -> ProjectState {
        let mut project = super::super::initial_project_state();
        let fingerprint = project.editor.fold_model_fingerprint_v1();
        project
            .editor
            .execute(
                0,
                Command::AddInstructionStep {
                    step: InstructionStep {
                        id: InstructionStepId::new(),
                        title: "四角形を確認する".to_owned(),
                        description: "用紙全体の向きを確認します。".to_owned(),
                        caution: "表裏を取り違えないでください。".to_owned(),
                        duration_ms: 1_000,
                        visual: Default::default(),
                        pose: InstructionPose {
                            model: InstructionPoseModel::AbsoluteHingeAnglesV1,
                            source_model_fingerprint: fingerprint,
                            fixed_face: None,
                            hinge_angles: Vec::new(),
                        },
                    },
                },
            )
            .unwrap();
        project
    }

    fn project_with_structured_proof_instruction() -> ProjectState {
        let mut project = super::super::initial_project_state();
        let fold = EdgeId::new();
        let boundary = &project.editor.paper().boundary_vertices;
        project
            .editor
            .execute(
                0,
                Command::AddEdge {
                    id: fold,
                    start: boundary[0],
                    end: boundary[2],
                    kind: EdgeKind::Mountain,
                },
            )
            .expect("add fold");
        let model = project.editor.fold_model_fingerprint_v1();
        let topology = project
            .editor
            .topology_analysis_input(project.project_id)
            .analyze();
        let fixed_face = topology.simulation_snapshot().expect("fold topology").faces[0].id;
        let source_angles = vec![InstructionHingeAngle {
            edge: fold,
            angle_degrees: 5.0,
        }];
        let middle_angles = vec![InstructionHingeAngle {
            edge: fold,
            angle_degrees: 45.0,
        }];
        let target_angles = vec![InstructionHingeAngle {
            edge: fold,
            angle_degrees: 90.0,
        }];
        let graph_hash = |angles: &[InstructionHingeAngle]| {
            let mut hash = Sha256::new();
            hash.update(b"stacked_fold_certified_path_graph_state_v1");
            hash.update((angles.len() as u64).to_be_bytes());
            for angle in angles {
                hash.update(angle.edge.canonical_bytes());
                hash.update(angle.angle_degrees.to_bits().to_be_bytes());
            }
            <[u8; 32]>::from(hash.finalize())
        };
        let source_pose = graph_hash(&source_angles);
        let middle_pose = graph_hash(&middle_angles);
        let target_pose = graph_hash(&target_angles);
        let candidates = [
            ori_collision::CertifiedPathTransitionCandidateV1 {
                source: source_pose,
                target: middle_pose,
                candidate_key: [7; 32],
            },
            ori_collision::CertifiedPathTransitionCandidateV1 {
                source: middle_pose,
                target: target_pose,
                candidate_key: [8; 32],
            },
        ];
        let certificate = match ori_collision::search_certified_pose_graph_v1(
            &[source_pose, middle_pose, target_pose],
            &candidates,
            source_pose,
            target_pose,
            |edge| {
                Some(
                    ori_collision::CertifiedPathTransitionEvidenceV1::from_native_oracle(
                        edge.source,
                        edge.target,
                        [1; 32],
                        [2; 32],
                        [3; 32],
                    ),
                )
            },
        ) {
            ori_collision::CertifiedPathGraphSearchResultV1::Certified(certificate) => certificate,
            other => panic!("expected native certificate, got {other:?}"),
        };
        let original_timeline = ori_domain::InstructionTimeline::default();
        let mut tampered_target = target_angles.clone();
        tampered_target[0].angle_degrees = 91.0;
        assert!(
            ori_instructions::append_certified_dyadic_path_timeline_v1(
                &original_timeline,
                "atomic dyadic fold",
                &model,
                fixed_face,
                &source_angles,
                &[tampered_target],
                &certificate,
            )
            .is_err()
        );
        assert!(
            original_timeline.steps.is_empty(),
            "failed atomic apply is a timeline no-op"
        );
        let timeline = ori_instructions::append_certified_dyadic_path_timeline_v1(
            &original_timeline,
            "atomic dyadic fold",
            &model,
            fixed_face,
            &source_angles,
            &[middle_angles, target_angles],
            &certificate,
        )
        .expect("atomic proof-bearing timeline");
        for (revision, step) in timeline.steps.into_iter().enumerate() {
            project
                .editor
                .execute(revision as u64 + 1, Command::AddInstructionStep { step })
                .expect("add proof step");
        }
        project
    }

    fn localized(text: &str) -> Vec<FoldTechniqueLocalizedTextV1> {
        vec![FoldTechniqueLocalizedTextV1 {
            locale: "ja".to_owned(),
            text: text.to_owned(),
        }]
    }

    fn book_fold_file() -> FoldTechniqueFileV1 {
        validate_fold_technique_file_v1(FoldTechniqueFileDocumentV1 {
            schema: FOLD_TECHNIQUE_FILE_SCHEMA_V1.to_owned(),
            version: FOLD_TECHNIQUE_FILE_VERSION_V1,
            package_id: "user.export.book-fold".to_owned(),
            metadata: FoldTechniqueMetadataV1 {
                authors: vec!["ORIGAMI2 test".to_owned()],
                source: FoldTechniqueSourceV1::UserAuthored,
                license_spdx_id: "MIT".to_owned(),
            },
            techniques: vec![FoldTechniqueTemplateV1 {
                id: "book-fold".to_owned(),
                version: 1,
                names: localized("二つ折り"),
                descriptions: localized("直線に沿う二つ折り"),
                parameters: vec![FoldTechniqueParameterDefinitionV1 {
                    id: "target_angle".to_owned(),
                    names: localized("目標角度"),
                    descriptions: localized("折り終わりの角度"),
                    parameter_type: FoldTechniqueParameterTypeV1::AngleMicrodegrees {
                        minimum: 1,
                        maximum: 180_000_000,
                        default: 90_000_000,
                    },
                }],
                preconditions: vec![],
                operations: vec![
                    FoldTechniqueOperationV1 {
                        id: "prepare".to_owned(),
                        names: localized("準備"),
                        action: FoldTechniqueActionV1::InstructionCue {
                            instructions: localized("紙を置く"),
                        },
                        parameter_bindings: vec![],
                        precondition_ids: vec![],
                        required_capabilities: vec![
                            FoldTechniqueCapabilityV1::HumanInterpretationV1,
                        ],
                        execution_support: FoldTechniqueExecutionSupportV1::DeclarativeOnly,
                    },
                    FoldTechniqueOperationV1 {
                        id: "fold".to_owned(),
                        names: localized("折る"),
                        action: FoldTechniqueActionV1::StraightLineStackedFold,
                        parameter_bindings: vec![FoldTechniqueParameterBindingV1 {
                            role: "target_angle".to_owned(),
                            parameter_id: "target_angle".to_owned(),
                        }],
                        precondition_ids: vec![],
                        required_capabilities: vec![
                            FoldTechniqueCapabilityV1::StraightLineStackedFoldV1,
                        ],
                        execution_support: FoldTechniqueExecutionSupportV1::DeclarativeOnly,
                    },
                ],
            }],
        })
        .expect("valid book-fold fixture")
    }

    fn native_certificate(
        source: [u8; 32],
        target: [u8; 32],
    ) -> ori_collision::CertifiedPoseGraphPathCertificateV1 {
        let candidate = ori_collision::CertifiedPathTransitionCandidateV1 {
            source,
            target,
            candidate_key: [7; 32],
        };
        match ori_collision::search_certified_pose_graph_v1(
            &[source, target],
            &[candidate],
            source,
            target,
            |edge| {
                Some(
                    ori_collision::CertifiedPathTransitionEvidenceV1::from_native_oracle(
                        edge.source,
                        edge.target,
                        [1; 32],
                        [2; 32],
                        [3; 32],
                    ),
                )
            },
        ) {
            ori_collision::CertifiedPathGraphSearchResultV1::Certified(certificate) => certificate,
            other => panic!("expected native certificate, got {other:?}"),
        }
    }

    fn basic_fold_file(title: &str) -> FoldTechniqueFileV1 {
        let mut document = book_fold_file().document().clone();
        document.techniques[0].names = localized(title);
        validate_fold_technique_file_v1(document).expect("valid basic-fold fixture")
    }

    fn project_with_parallel_folds(count: usize) -> (ProjectState, Vec<EdgeId>) {
        assert!((1..=3).contains(&count));
        let mut project = super::super::initial_project_state();
        let boundary = project.editor.paper().boundary_vertices.clone();
        let boundary_edge = |start: VertexId, end: VertexId, project: &ProjectState| {
            project
                .editor
                .pattern()
                .edges
                .iter()
                .find(|edge| {
                    (edge.start == start && edge.end == end)
                        || (edge.start == end && edge.end == start)
                })
                .expect("boundary edge")
                .id
        };
        let mut top_edge = boundary_edge(boundary[0], boundary[1], &project);
        let mut bottom_edge = boundary_edge(boundary[2], boundary[3], &project);
        let mut top_vertices = Vec::new();
        let mut bottom_vertices = Vec::new();
        for remaining in (1..=count).rev() {
            let fraction = 1.0 / (remaining as f64 + 1.0);
            for (edge, vertices) in [
                (&mut top_edge, &mut top_vertices),
                (&mut bottom_edge, &mut bottom_vertices),
            ] {
                let new_vertex = VertexId::new();
                let new_edge = EdgeId::new();
                let revision = project.editor.revision();
                project
                    .editor
                    .execute(
                        revision,
                        Command::SplitBoundaryEdge {
                            edge: *edge,
                            new_vertex,
                            new_edge,
                            fraction,
                        },
                    )
                    .expect("split boundary for parallel fold");
                *edge = new_edge;
                vertices.push(new_vertex);
            }
        }
        let mut folds = Vec::new();
        for (top, bottom) in top_vertices
            .into_iter()
            .zip(bottom_vertices.into_iter().rev())
        {
            let edge = EdgeId::new();
            let revision = project.editor.revision();
            project
                .editor
                .execute(
                    revision,
                    Command::AddEdge {
                        id: edge,
                        start: top,
                        end: bottom,
                        kind: EdgeKind::Mountain,
                    },
                )
                .expect("add parallel fold");
            folds.push(edge);
        }
        (project, folds)
    }

    fn two_segment_file(
        title: &str,
        action: FoldTechniqueActionV1,
        capability: FoldTechniqueCapabilityV1,
        unsupported: FoldTechniqueUnsupportedPhysicalOperationV1,
    ) -> FoldTechniqueFileV1 {
        let mut document = book_fold_file().document().clone();
        let technique = &mut document.techniques[0];
        technique.names = localized(title);
        let operation = &mut technique.operations[1];
        operation.action = action;
        operation.required_capabilities = vec![capability];
        operation.execution_support =
            FoldTechniqueExecutionSupportV1::UnsupportedPhysicalOperation {
                operation: unsupported,
            };
        validate_fold_technique_file_v1(document).expect("valid two-segment technique")
    }

    fn accordion_file() -> FoldTechniqueFileV1 {
        let mut document = book_fold_file().document().clone();
        let technique = &mut document.techniques[0];
        technique.names = localized("蛇腹折り");
        let physical = technique.operations[1].clone();
        technique.operations = (1..=3)
            .map(|index| {
                let mut operation = physical.clone();
                operation.id = format!("pleat-{index}");
                operation
            })
            .collect();
        validate_fold_technique_file_v1(document).expect("valid accordion fixture")
    }

    fn crimp_file() -> FoldTechniqueFileV1 {
        let mut document = book_fold_file().document().clone();
        let technique = &mut document.techniques[0];
        technique.names = localized("段折り");
        let physical = technique.operations[1].clone();
        technique.operations = (1..=2)
            .map(|index| {
                let mut operation = physical.clone();
                operation.id = format!("crimp-{index}");
                operation
            })
            .collect();
        validate_fold_technique_file_v1(document).expect("valid crimp fixture")
    }

    fn source_for(
        project: &ProjectState,
        format: InstructionExportFormatRequest,
    ) -> InstructionExportSource {
        InstructionExportSource {
            export_id: ProjectId::new(),
            expected_instance_id: project.instance_id,
            expected_project_id: project.project_id,
            expected_revision: project.editor.revision(),
            topology_input: project.editor.topology_analysis_input(project.project_id),
            format,
            name: project.name.clone(),
            pattern: project.editor.pattern().clone(),
            paper: project.editor.paper().clone(),
            timeline: project.editor.instruction_timeline().clone(),
            current_fold_model_fingerprint: project.editor.fold_model_fingerprint_v1(),
            cancellation: Arc::new(AtomicBool::new(false)),
            phase: Arc::new(AtomicU8::new(PHASE_VALIDATING)),
            consensus_summary: project
                .editor
                .beginner_design_profile()
                .generation_provenance
                .as_ref()
                .and_then(|value| value.reference_consensus_summary.clone()),
        }
    }

    fn pdf_utf16be_hex(text: &str) -> String {
        let mut encoded = String::from("FEFF");
        for unit in text.encode_utf16() {
            encoded.push_str(&format!("{unit:04X}"));
        }
        encoded
    }

    #[test]
    fn instruction_exports_embed_only_safe_consensus_summary() {
        let summary = ori_domain::BeginnerReferenceConsensusSummaryV1 {
            schema_version: 1,
            model: "component_extent_branch_v1".to_owned(),
            source_count: 3,
            excluded_count: 1,
            agreement_score: 81,
            component_subscore: 90,
            extent_subscore: 76,
            branch_subscore: 78,
        };
        let artifact = |format, bytes| InstructionExportArtifact {
            format,
            media_type: "test",
            file_extension: "test",
            profile: INSTRUCTION_EXPORT_PROFILE,
            projection_profile: INSTRUCTION_EXPORT_PROJECTION_PROFILE,
            bytes,
            step_count: 1,
            page_count: 1,
            glyph_count: 0,
            projected_vertex_visits: 0,
            caution_count: 0,
            warnings: Vec::new(),
        };
        let mut pdf = artifact(InstructionExportFormat::Pdf17, b"%PDF-1.7".to_vec());
        embed_instruction_consensus_summary(
            &mut pdf,
            InstructionExportFormatRequest::Pdf,
            &summary,
        )
        .unwrap();
        let pdf_text = String::from_utf8(pdf.bytes).unwrap();
        assert!(pdf_text.contains("ORIGAMI2_REFERENCE_CONSENSUS_SUMMARY"));
        assert!(pdf_text.contains("component_subscore"));
        assert!(!pdf_text.contains("asset_id"));
        assert!(!pdf_text.contains("sha256"));

        let cursor = Cursor::new(Vec::new());
        let zip_bytes = zip::ZipWriter::new(cursor).finish().unwrap().into_inner();
        let mut svg = artifact(InstructionExportFormat::SvgPageZip, zip_bytes);
        embed_instruction_consensus_summary(
            &mut svg,
            InstructionExportFormatRequest::SvgZip,
            &summary,
        )
        .unwrap();
        let mut archive = zip::ZipArchive::new(Cursor::new(svg.bytes)).unwrap();
        let mut text = String::new();
        archive
            .by_name("origami2-reference-consensus-summary.json")
            .unwrap()
            .read_to_string(&mut text)
            .unwrap();
        assert_eq!(
            serde_json::from_str::<ori_domain::BeginnerReferenceConsensusSummaryV1>(&text).unwrap(),
            summary
        );
    }

    fn assert_compiler_artifact_content(
        project: &ProjectState,
        technique_title: &str,
        timeline: &ori_domain::InstructionTimeline,
    ) {
        let proof_bindings = timeline.steps[1..]
            .iter()
            .map(|step| {
                let reference = step
                    .visual
                    .path_certificate_reference_v1
                    .as_ref()
                    .expect("compiler step has structured proof");
                let binding = reference
                    .binding_sha256
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>();
                assert!(step.description.contains(&binding));
                binding
            })
            .collect::<Vec<_>>();

        let mut pdf_source = source_for(project, InstructionExportFormatRequest::Pdf);
        pdf_source.name = technique_title.to_owned();
        pdf_source.timeline = timeline.clone();
        let pdf = build_pending_export(pdf_source).expect("compiler PDF content");
        let body = std::str::from_utf8(&pdf.bytes[15..]).expect("ASCII PDF body");
        assert!(body.contains(&format!("/Title <{}>", pdf_utf16be_hex(technique_title))));
        assert!(body.matches("\nstream\n").count() >= timeline.steps.len());
        assert!(body.contains(" m\n") && body.contains(" l\n"));

        let mut without_summary = timeline.clone();
        for step in &mut without_summary.steps[1..] {
            step.description = "証明要約を除いた比較用手順".to_owned();
            step.visual.path_certificate_reference_v1 = None;
        }
        let mut stripped_source = source_for(project, InstructionExportFormatRequest::Pdf);
        stripped_source.name = technique_title.to_owned();
        stripped_source.timeline = without_summary;
        let stripped = build_pending_export(stripped_source).expect("stripped comparison PDF");
        assert_ne!(pdf.bytes.as_ref(), stripped.bytes.as_ref());

        let mut svg_source = source_for(project, InstructionExportFormatRequest::SvgZip);
        svg_source.name = technique_title.to_owned();
        svg_source.timeline = timeline.clone();
        let svg = build_pending_export(svg_source).expect("compiler SVG content");
        let mut archive = zip::ZipArchive::new(Cursor::new(svg.bytes.as_ref())).expect("SVG ZIP");
        let mut manifest = String::new();
        archive
            .by_name("manifest.json")
            .expect("SVG manifest")
            .read_to_string(&mut manifest)
            .expect("read SVG manifest");
        let manifest: serde_json::Value = serde_json::from_str(&manifest).expect("manifest JSON");
        assert_eq!(manifest["title"], technique_title);
        let mut pages = String::new();
        for page in 1..=svg.page_count {
            archive
                .by_name(&format!("pages/page-{page:04}.svg"))
                .expect("SVG page")
                .read_to_string(&mut pages)
                .expect("read SVG page");
        }
        assert!(pages.contains(technique_title));
        for binding in proof_bindings {
            assert!(pages.contains(&binding[..8]));
        }
        assert!(pages.contains("<polygon "));
        assert!(pages.contains("<line "));
        assert!(pages.contains("data-text-run=\"1\""));
    }

    pub(crate) fn assert_structured_timeline_exports_pdf_and_svg_zip(
        project: &super::super::ProjectState,
        expected_steps: usize,
    ) {
        for (format, magic) in [
            (InstructionExportFormatRequest::Pdf, b"%PDF-1.7".as_slice()),
            (InstructionExportFormatRequest::SvgZip, b"PK".as_slice()),
        ] {
            let pending = build_pending_export(source_for(project, format))
                .expect("proof-bearing project exports through the native formatter");
            assert_eq!(pending.step_count, expected_steps);
            assert!(pending.bytes.starts_with(magic));
        }
    }

    fn fake_pending(
        project: &ProjectState,
        format: InstructionExportFormatRequest,
        bytes: &[u8],
    ) -> PendingInstructionExport {
        let caution_count = project
            .editor
            .instruction_timeline()
            .steps
            .iter()
            .filter(|step| !step.caution.is_empty())
            .count();
        PendingInstructionExport {
            export_id: ProjectId::new(),
            expected_instance_id: project.instance_id,
            expected_project_id: project.project_id,
            expected_revision: project.editor.revision(),
            topology_input: project.editor.topology_analysis_input(project.project_id),
            format,
            profile: INSTRUCTION_EXPORT_PROFILE,
            projection_profile: INSTRUCTION_EXPORT_PROJECTION_PROFILE,
            format_summary: format.format_summary().to_owned(),
            suggested_file_name: suggested_export_file_name(&project.name, format.extension()),
            bytes: Arc::from(bytes),
            step_count: project.editor.instruction_timeline().steps.len(),
            page_count: project.editor.instruction_timeline().steps.len(),
            caution_count,
            warnings: instruction_export_warning_snapshots(&INSTRUCTION_EXPORT_WARNINGS),
        }
    }

    #[test]
    fn all_formats_build_bounded_previews_with_exact_allowlist() {
        let project = project_with_instruction();
        for format in [
            InstructionExportFormatRequest::Pdf,
            InstructionExportFormatRequest::SvgZip,
        ] {
            let pending = build_pending_export(source_for(&project, format)).expect("build export");
            let preview = preview_snapshot(&pending);
            assert_eq!(preview.expected_project_id, project.project_id);
            assert_eq!(preview.expected_revision, project.editor.revision());
            assert_eq!(preview.step_count, 1);
            assert!(preview.page_count >= preview.step_count);
            assert_eq!(preview.caution_count, 1);
            assert!(preview.byte_count > 0);
            assert!(!preview.format_summary.is_empty());
            assert_eq!(preview.profile, INSTRUCTION_EXPORT_PROFILE);
            assert_eq!(
                preview.projection_profile,
                INSTRUCTION_EXPORT_PROJECTION_PROFILE
            );
            assert_eq!(preview.warnings.len(), 4);

            let json = serde_json::to_value(InstructionExportPreviewResponse { preview }).unwrap();
            assert_eq!(
                json.as_object()
                    .unwrap()
                    .keys()
                    .map(String::as_str)
                    .collect::<BTreeSet<_>>(),
                BTreeSet::from(["preview"])
            );
            assert_eq!(
                json["preview"]
                    .as_object()
                    .unwrap()
                    .keys()
                    .map(String::as_str)
                    .collect::<BTreeSet<_>>(),
                BTreeSet::from([
                    "byte_count",
                    "caution_count",
                    "expected_project_id",
                    "expected_revision",
                    "export_id",
                    "format",
                    "format_summary",
                    "page_count",
                    "profile",
                    "projection_profile",
                    "step_count",
                    "suggested_file_name",
                    "warnings",
                ])
            );
            for warning in json["preview"]["warnings"].as_array().unwrap() {
                assert_eq!(
                    warning
                        .as_object()
                        .unwrap()
                        .keys()
                        .map(String::as_str)
                        .collect::<BTreeSet<_>>(),
                    BTreeSet::from(["category", "message_ja"])
                );
            }
        }
    }

    #[test]
    fn command_errors_are_closed_and_do_not_expose_internal_values() {
        let private_value = r"C:\Users\alice\秘密の作品.ori";
        let error =
            InstructionExportCommandError::from(InstructionExportErrorCategory::UnexpectedFailure);
        assert_eq!(
            error.category,
            InstructionExportErrorCategory::UnexpectedFailure
        );
        let json = serde_json::to_value(error).unwrap();
        assert_eq!(
            json.as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect::<BTreeSet<_>>(),
            BTreeSet::from(["category", "message_ja"])
        );
        let serialized = serde_json::to_string(&json).unwrap();
        assert!(!serialized.contains("alice"));
        assert!(!serialized.contains("秘密の作品"));
        assert!(!serialized.contains(private_value));

        let unsupported_glyph =
            instruction_document_failure_category(InstructionExportError::UnsupportedGlyph {
                code_point: 0x1F4A5,
            });
        assert_eq!(
            unsupported_glyph,
            InstructionExportErrorCategory::DocumentInputInvalid
        );
        let output_limit =
            instruction_document_failure_category(InstructionExportError::OutputTooLarge {
                actual: usize::MAX,
                maximum: 1,
            });
        assert_eq!(
            output_limit,
            InstructionExportErrorCategory::DocumentLimitExceeded
        );
    }

    #[test]
    fn command_error_category_wire_values_remain_stable() {
        for (category, wire_value) in [
            (
                InstructionExportErrorCategory::StateUnavailable,
                "state_unavailable",
            ),
            (
                InstructionExportErrorCategory::GenerationUnavailable,
                "generation_unavailable",
            ),
            (
                InstructionExportErrorCategory::GenerationReplaced,
                "generation_replaced",
            ),
            (
                InstructionExportErrorCategory::GenerationCancelled,
                "generation_cancelled",
            ),
            (
                InstructionExportErrorCategory::ProjectChanged,
                "project_changed",
            ),
            (
                InstructionExportErrorCategory::TimelineEmpty,
                "timeline_empty",
            ),
            (
                InstructionExportErrorCategory::TimelineStale,
                "timeline_stale",
            ),
            (
                InstructionExportErrorCategory::SourceLimitExceeded,
                "source_limit_exceeded",
            ),
            (
                InstructionExportErrorCategory::TopologyUnsupported,
                "topology_unsupported",
            ),
            (
                InstructionExportErrorCategory::DocumentInputInvalid,
                "document_input_invalid",
            ),
            (
                InstructionExportErrorCategory::DocumentLimitExceeded,
                "document_limit_exceeded",
            ),
            (
                InstructionExportErrorCategory::DocumentGenerationFailed,
                "document_generation_failed",
            ),
            (
                InstructionExportErrorCategory::DocumentContractInvalid,
                "document_contract_invalid",
            ),
            (
                InstructionExportErrorCategory::WarningAcknowledgementRequired,
                "warning_acknowledgement_required",
            ),
            (
                InstructionExportErrorCategory::SaveTargetInvalid,
                "save_target_invalid",
            ),
            (InstructionExportErrorCategory::SaveFailed, "save_failed"),
            (
                InstructionExportErrorCategory::UnexpectedFailure,
                "unexpected_failure",
            ),
        ] {
            let value =
                serde_json::to_value(InstructionExportCommandError::from(category)).unwrap();
            assert_eq!(value["category"], wire_value);
            assert_eq!(value["message_ja"], category.message_ja());
        }
    }

    #[test]
    fn internal_failure_categories_are_assigned_at_their_sources() {
        let cancellation = AtomicBool::new(true);
        assert_eq!(
            ensure_not_cancelled(&cancellation),
            Err(InstructionExportErrorCategory::GenerationCancelled)
        );

        let limits = ori_formats::InstructionDiagramLimits::default();
        assert_eq!(
            validate_source_counts(limits.max_source_vertices + 1, limits.max_source_edges),
            Err(InstructionExportErrorCategory::SourceLimitExceeded)
        );
        assert_eq!(
            persist_instruction_export_bytes(Path::new(""), b"not written"),
            Err(InstructionExportErrorCategory::SaveTargetInvalid)
        );
    }

    #[test]
    fn empty_and_stale_timelines_are_rejected_before_background_work() {
        let empty = super::super::initial_project_state();
        let empty_project_id = empty.project_id;
        let cancellation = Arc::new(AtomicBool::new(false));
        let empty_error = capture_export_source(
            &AppState::new(empty),
            ProjectId::new(),
            empty_project_id,
            0,
            InstructionExportFormatRequest::Pdf,
            cancellation,
            Arc::new(AtomicU8::new(PHASE_VALIDATING)),
        )
        .err()
        .expect("empty timeline must be rejected");
        assert_eq!(empty_error, InstructionExportErrorCategory::TimelineEmpty);

        let mut stale = project_with_instruction();
        stale
            .editor
            .execute(
                stale.editor.revision(),
                Command::AddVertex {
                    id: VertexId::new(),
                    position: Point2::new(20.0, 20.0),
                },
            )
            .unwrap();
        let stale_project_id = stale.project_id;
        let stale_revision = stale.editor.revision();
        let stale_error = capture_export_source(
            &AppState::new(stale),
            ProjectId::new(),
            stale_project_id,
            stale_revision,
            InstructionExportFormatRequest::Pdf,
            Arc::new(AtomicBool::new(false)),
            Arc::new(AtomicU8::new(PHASE_VALIDATING)),
        )
        .err()
        .expect("stale step must be rejected");
        assert_eq!(stale_error, InstructionExportErrorCategory::TimelineStale);
    }

    #[test]
    fn compiled_accordion_reopens_into_native_exports_with_all_ordered_segments() {
        let (project, fold_edges) = project_with_parallel_folds(3);
        let edges: [EdgeId; 3] = fold_edges.try_into().expect("three accordion folds");
        let model = project.editor.fold_model_fingerprint_v1();
        let topology = project
            .editor
            .topology_analysis_input(project.project_id)
            .analyze();
        let fixed_face = topology
            .simulation_snapshot()
            .expect("accordion topology")
            .faces[0]
            .id;
        let targets = [45_000_000, 90_000_000, 135_000_000];
        let mut pose = edges
            .iter()
            .copied()
            .map(|edge| InstructionHingeAngle {
                edge,
                angle_degrees: 5.0,
            })
            .collect::<Vec<_>>();
        let source = pose.clone();
        let certificates = edges
            .iter()
            .copied()
            .zip(targets)
            .map(|(edge, target)| {
                let source_hash = instruction_pose_fingerprint_v1(&model, fixed_face, &pose);
                pose.iter_mut()
                    .find(|hinge| hinge.edge == edge)
                    .expect("accordion hinge")
                    .angle_degrees = target as f64 / 1_000_000.0;
                let target_hash = instruction_pose_fingerprint_v1(&model, fixed_face, &pose);
                native_certificate(source_hash, target_hash)
            })
            .collect::<Vec<_>>();
        let timeline = compile_certified_accordion_fold_timeline_v1(AccordionFoldMotionRequestV1 {
            technique_file: &accordion_file(),
            technique_id: "book-fold",
            source_model_fingerprint: &model,
            fixed_face,
            source_hinge_angles: &source,
            ordered_edges: &edges,
            ordered_target_angles_microdegrees: &targets,
            ordered_path_certificates: &certificates,
        })
        .expect("compile real accordion");
        assert_eq!(timeline.steps.len(), 4);
        assert!(timeline.steps[3].title.contains("蛇腹折り"));
        for (index, certificate) in certificates.iter().enumerate() {
            let reference = timeline.steps[index + 1]
                .visual
                .path_certificate_reference_v1
                .as_ref()
                .expect("accordion compiler proof");
            assert_eq!(reference.source_pose_sha256, certificate.source());
            assert_eq!(reference.target_pose_sha256, certificate.target());
            if index > 0 {
                let previous = timeline.steps[index]
                    .visual
                    .path_certificate_reference_v1
                    .as_ref()
                    .expect("previous accordion proof");
                assert_eq!(previous.target_pose_sha256, reference.source_pose_sha256);
            }
            let proof_hex = reference
                .binding_sha256
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            assert!(timeline.steps[index + 1].description.contains(&proof_hex));
        }
        let archived = serde_json::to_vec(&timeline).expect("archive compiled accordion");
        let reopened: ori_domain::InstructionTimeline =
            serde_json::from_slice(&archived).expect("reopen compiled accordion");
        assert_compiler_artifact_content(&project, "蛇腹折り", &reopened);
        for (format, magic) in [
            (InstructionExportFormatRequest::Pdf, b"%PDF-1.7".as_slice()),
            (InstructionExportFormatRequest::SvgZip, b"PK".as_slice()),
        ] {
            let mut source = source_for(&project, format);
            source.timeline = reopened.clone();
            let artifact = build_pending_export(source).expect("export real accordion compiler");
            assert_eq!(artifact.step_count, 4);
            assert!(artifact.bytes.starts_with(magic));
        }
    }

    #[test]
    fn compiled_two_segment_techniques_reopen_into_native_exports_with_exact_chaining() {
        let (project, fold_edges) = project_with_parallel_folds(2);
        let [fold_edge, second_edge]: [EdgeId; 2] =
            fold_edges.try_into().expect("two technique folds");
        let model = project.editor.fold_model_fingerprint_v1();
        let topology = project
            .editor
            .topology_analysis_input(project.project_id)
            .analyze();
        let fixed_face = topology.simulation_snapshot().expect("fold topology").faces[0].id;
        let source = vec![
            InstructionHingeAngle {
                edge: fold_edge,
                angle_degrees: 5.0,
            },
            InstructionHingeAngle {
                edge: second_edge,
                angle_degrees: 5.0,
            },
        ];
        let middle = vec![
            InstructionHingeAngle {
                edge: fold_edge,
                angle_degrees: 45.0,
            },
            InstructionHingeAngle {
                edge: second_edge,
                angle_degrees: 5.0,
            },
        ];
        let target = vec![
            InstructionHingeAngle {
                edge: fold_edge,
                angle_degrees: 45.0,
            },
            InstructionHingeAngle {
                edge: second_edge,
                angle_degrees: 90.0,
            },
        ];
        let first = native_certificate(
            instruction_pose_fingerprint_v1(&model, fixed_face, &source),
            instruction_pose_fingerprint_v1(&model, fixed_face, &middle),
        );
        let second = native_certificate(
            instruction_pose_fingerprint_v1(&model, fixed_face, &middle),
            instruction_pose_fingerprint_v1(&model, fixed_face, &target),
        );

        let inside_file = two_segment_file(
            "中割り折り",
            FoldTechniqueActionV1::InsideReverseFold,
            FoldTechniqueCapabilityV1::InsideReverseFoldMotionV1,
            FoldTechniqueUnsupportedPhysicalOperationV1::InsideReverseFoldMotionV1,
        );
        let sink_file = two_segment_file(
            "沈め折り",
            FoldTechniqueActionV1::SinkFold {
                sink_kind: FoldTechniqueSinkKindV1::Open,
            },
            FoldTechniqueCapabilityV1::SinkFoldMotionV1,
            FoldTechniqueUnsupportedPhysicalOperationV1::SinkFoldMotionV1,
        );
        let squash_file = two_segment_file(
            "つぶし折り",
            FoldTechniqueActionV1::SinkFold {
                sink_kind: FoldTechniqueSinkKindV1::Open,
            },
            FoldTechniqueCapabilityV1::SinkFoldMotionV1,
            FoldTechniqueUnsupportedPhysicalOperationV1::SinkFoldMotionV1,
        );
        let crimp_file = crimp_file();
        let layer_file = two_segment_file(
            "層選択折り",
            FoldTechniqueActionV1::LayerSelectiveManipulation {
                instructions: localized("対象層を選ぶ"),
            },
            FoldTechniqueCapabilityV1::LayerSelectiveMotionV1,
            FoldTechniqueUnsupportedPhysicalOperationV1::LayerSelectiveMotionV1,
        );
        assert!(matches!(
            compile_certified_petal_fold_timeline_v1(PetalFoldMotionRequestV1 {
                technique_file: &sink_file,
                technique_id: "book-fold",
                source_model_fingerprint: &model,
                fixed_face,
                first_edge: fold_edge,
                second_edge,
                source_hinge_angles: &source,
                intermediate_angle_microdegrees: 45_000_000,
                target_angle_microdegrees: 90_000_000,
                first_path_certificate: &first,
                second_path_certificate: &second,
            }),
            Err(ori_instructions::ReverseFoldMotionError::UnsupportedTechnique)
        ));
        let timelines = [
            (
                "中割り折り",
                compile_certified_reverse_fold_timeline_v1(ReverseFoldMotionRequestV1 {
                    technique_file: &inside_file,
                    technique_id: "book-fold",
                    kind: ReverseFoldKindV1::Inside,
                    source_model_fingerprint: &model,
                    fixed_face,
                    first_edge: fold_edge,
                    second_edge,
                    source_hinge_angles: &source,
                    intermediate_angle_microdegrees: 45_000_000,
                    target_angle_microdegrees: 90_000_000,
                    first_path_certificate: &first,
                    second_path_certificate: &second,
                })
                .expect("compile inside reverse"),
            ),
            (
                "沈め折り",
                compile_certified_sink_fold_timeline_v1(SinkFoldMotionRequestV1 {
                    technique_file: &sink_file,
                    technique_id: "book-fold",
                    source_model_fingerprint: &model,
                    fixed_face,
                    first_edge: fold_edge,
                    second_edge,
                    source_hinge_angles: &source,
                    intermediate_angle_microdegrees: 45_000_000,
                    target_angle_microdegrees: 90_000_000,
                    first_path_certificate: &first,
                    second_path_certificate: &second,
                })
                .expect("compile sink"),
            ),
            (
                "つぶし折り",
                compile_certified_squash_fold_timeline_v1(SquashFoldMotionRequestV1 {
                    technique_file: &squash_file,
                    technique_id: "book-fold",
                    source_model_fingerprint: &model,
                    fixed_face,
                    first_edge: fold_edge,
                    second_edge,
                    source_hinge_angles: &source,
                    intermediate_angle_microdegrees: 45_000_000,
                    target_angle_microdegrees: 90_000_000,
                    first_path_certificate: &first,
                    second_path_certificate: &second,
                })
                .expect("compile squash from validated sink primitive"),
            ),
            (
                "段折り",
                compile_certified_crimp_fold_timeline_v1(CrimpFoldMotionRequestV1 {
                    technique_file: &crimp_file,
                    technique_id: "book-fold",
                    source_model_fingerprint: &model,
                    fixed_face,
                    first_edge: fold_edge,
                    second_edge,
                    source_hinge_angles: &source,
                    intermediate_angle_microdegrees: 45_000_000,
                    target_angle_microdegrees: 90_000_000,
                    first_path_certificate: &first,
                    second_path_certificate: &second,
                })
                .expect("compile crimp from two straight-fold primitives"),
            ),
            (
                "層選択折り",
                compile_certified_layer_selective_timeline_v1(LayerSelectiveMotionRequestV1 {
                    technique_file: &layer_file,
                    technique_id: "book-fold",
                    source_model_fingerprint: &model,
                    fixed_face,
                    first_edge: fold_edge,
                    second_edge,
                    source_hinge_angles: &source,
                    intermediate_angle_microdegrees: 45_000_000,
                    target_angle_microdegrees: 90_000_000,
                    first_path_certificate: &first,
                    second_path_certificate: &second,
                })
                .expect("compile layer-selective"),
            ),
        ];
        for (title, timeline) in timelines {
            assert_eq!(timeline.steps.len(), 3);
            assert!(timeline.steps[2].title.contains(title));
            let first_reference = timeline.steps[1]
                .visual
                .path_certificate_reference_v1
                .as_ref()
                .expect("first compiler proof");
            let second_reference = timeline.steps[2]
                .visual
                .path_certificate_reference_v1
                .as_ref()
                .expect("second compiler proof");
            assert_eq!(
                first_reference.target_pose_sha256,
                second_reference.source_pose_sha256
            );
            for (step, reference) in timeline.steps[1..]
                .iter()
                .zip([first_reference, second_reference])
            {
                let proof_hex = reference
                    .binding_sha256
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>();
                assert!(step.description.contains(&proof_hex));
            }
            let archived = serde_json::to_vec(&timeline).expect("archive compiler timeline");
            let reopened: ori_domain::InstructionTimeline =
                serde_json::from_slice(&archived).expect("reopen compiler timeline");
            assert_compiler_artifact_content(&project, title, &reopened);
            for (format, magic) in [
                (InstructionExportFormatRequest::Pdf, b"%PDF-1.7".as_slice()),
                (InstructionExportFormatRequest::SvgZip, b"PK".as_slice()),
            ] {
                let mut export_source = source_for(&project, format);
                export_source.timeline = reopened.clone();
                let artifact = build_pending_export(export_source)
                    .expect("export real two-segment compiler timeline");
                assert_eq!(artifact.step_count, 3);
                assert!(artifact.bytes.starts_with(magic));
            }
        }
    }

    #[test]
    fn compiled_book_fold_reopens_into_native_pdf_and_svg_with_exact_proof_content() {
        let mut project = super::super::initial_project_state();
        let fold_edge = EdgeId::new();
        let boundary = &project.editor.paper().boundary_vertices;
        project
            .editor
            .execute(
                0,
                Command::AddEdge {
                    id: fold_edge,
                    start: boundary[0],
                    end: boundary[2],
                    kind: EdgeKind::Mountain,
                },
            )
            .expect("add book-fold edge");
        let model = project.editor.fold_model_fingerprint_v1();
        let topology = project
            .editor
            .topology_analysis_input(project.project_id)
            .analyze();
        let fixed_face = topology.simulation_snapshot().expect("book topology").faces[0].id;
        let source_angles = vec![InstructionHingeAngle {
            edge: fold_edge,
            angle_degrees: 5.0,
        }];
        let target_angles = vec![InstructionHingeAngle {
            edge: fold_edge,
            angle_degrees: 90.0,
        }];
        let certificate = native_certificate(
            instruction_pose_fingerprint_v1(&model, fixed_face, &source_angles),
            instruction_pose_fingerprint_v1(&model, fixed_face, &target_angles),
        );
        let compiled = compile_certified_book_fold_timeline_v1(BookFoldMotionRequestV1 {
            technique_file: &book_fold_file(),
            technique_id: "book-fold",
            source_model_fingerprint: &model,
            fixed_face,
            fold_edge,
            source_hinge_angles: &source_angles,
            target_angle_microdegrees: 90_000_000,
            path_certificate: &certificate,
        })
        .expect("compile real book-fold timeline");
        assert_eq!(compiled.steps.len(), 2);
        assert!(compiled.steps[1].title.contains("二つ折り"));
        let reference = compiled.steps[1]
            .visual
            .path_certificate_reference_v1
            .as_ref()
            .expect("compiler emits structured proof");
        assert_eq!(reference.source_pose_sha256, certificate.source());
        assert_eq!(reference.target_pose_sha256, certificate.target());
        let proof_hex = reference
            .binding_sha256
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert!(compiled.steps[1].description.contains(&proof_hex));

        let archived = serde_json::to_vec(&compiled).expect("archive compiled book fold");
        let reopened: ori_domain::InstructionTimeline =
            serde_json::from_slice(&archived).expect("reopen compiled book fold");
        assert_compiler_artifact_content(&project, "二つ折り", &reopened);
        for step in reopened.steps.clone() {
            let revision = project.editor.revision();
            project
                .editor
                .execute(revision, Command::AddInstructionStep { step })
                .expect("persist compiler step");
        }
        for (format, magic) in [
            (InstructionExportFormatRequest::Pdf, b"%PDF-1.7".as_slice()),
            (InstructionExportFormatRequest::SvgZip, b"PK".as_slice()),
        ] {
            let artifact = build_pending_export(source_for(&project, format))
                .expect("export compiler-authored book fold");
            assert_eq!(artifact.step_count, 2);
            assert!(artifact.bytes.starts_with(magic));
        }
    }

    #[test]
    fn compiled_mountain_and_valley_folds_preserve_kind_through_native_pdf_and_svg() {
        for (kind, edge_kind, title) in [
            (BasicFoldKindV1::Mountain, EdgeKind::Mountain, "山折り"),
            (BasicFoldKindV1::Valley, EdgeKind::Valley, "谷折り"),
        ] {
            let mut project = super::super::initial_project_state();
            let fold_edge = EdgeId::new();
            let boundary = &project.editor.paper().boundary_vertices;
            project
                .editor
                .execute(
                    0,
                    Command::AddEdge {
                        id: fold_edge,
                        start: boundary[0],
                        end: boundary[2],
                        kind: edge_kind,
                    },
                )
                .expect("add assigned basic-fold edge");
            let model = project.editor.fold_model_fingerprint_v1();
            let topology = project
                .editor
                .topology_analysis_input(project.project_id)
                .analyze();
            let fixed_face = topology
                .simulation_snapshot()
                .expect("basic topology")
                .faces[0]
                .id;
            let source_angles = vec![InstructionHingeAngle {
                edge: fold_edge,
                angle_degrees: 5.0,
            }];
            let target_angles = vec![InstructionHingeAngle {
                edge: fold_edge,
                angle_degrees: 90.0,
            }];
            let certificate = native_certificate(
                instruction_pose_fingerprint_v1(&model, fixed_face, &source_angles),
                instruction_pose_fingerprint_v1(&model, fixed_face, &target_angles),
            );
            let file = basic_fold_file(title);
            let compiled = compile_certified_basic_fold_timeline_v1(BasicFoldMotionRequestV1 {
                kind,
                straight_fold: BookFoldMotionRequestV1 {
                    technique_file: &file,
                    technique_id: "book-fold",
                    source_model_fingerprint: &model,
                    fixed_face,
                    fold_edge,
                    source_hinge_angles: &source_angles,
                    target_angle_microdegrees: 90_000_000,
                    path_certificate: &certificate,
                },
            })
            .expect("compile assigned basic fold");
            assert!(compiled.steps[1].title.contains(title));
            let archived = serde_json::to_vec(&compiled).expect("archive basic fold");
            let reopened: ori_domain::InstructionTimeline =
                serde_json::from_slice(&archived).expect("reopen basic fold");
            assert_compiler_artifact_content(&project, title, &reopened);
            for (format, magic) in [
                (InstructionExportFormatRequest::Pdf, b"%PDF-1.7".as_slice()),
                (InstructionExportFormatRequest::SvgZip, b"PK".as_slice()),
            ] {
                let mut source = source_for(&project, format);
                source.timeline = reopened.clone();
                let artifact = build_pending_export(source).expect("export assigned basic fold");
                assert_eq!(artifact.step_count, 2);
                assert!(artifact.bytes.starts_with(magic));
            }
        }
        assert_eq!(
            compile_certified_basic_fold_timeline_v1(BasicFoldMotionRequestV1 {
                kind: BasicFoldKindV1::Valley,
                straight_fold: BookFoldMotionRequestV1 {
                    technique_file: &basic_fold_file("山折り"),
                    technique_id: "book-fold",
                    source_model_fingerprint: &"ab".repeat(32),
                    fixed_face: ori_domain::FaceId::new(),
                    fold_edge: EdgeId::new(),
                    source_hinge_angles: &[],
                    target_angle_microdegrees: 90_000_000,
                    path_certificate: &native_certificate([1; 32], [2; 32]),
                },
            }),
            Err(ori_instructions::BookFoldMotionError::UnsupportedTechnique),
            "assignment kind and technique name cannot be interchanged"
        );
    }

    #[test]
    fn reopened_proof_binding_reaches_native_export_and_tampering_stays_opaque() {
        let project = project_with_structured_proof_instruction();
        let source = source_for(&project, InstructionExportFormatRequest::Pdf);
        let archived = serde_json::to_vec(&source.timeline).expect("archive timeline");
        let reopened_timeline: ori_domain::InstructionTimeline =
            serde_json::from_slice(&archived).expect("reopen timeline");
        assert_eq!(reopened_timeline.steps.len(), 3);
        let first_reference = reopened_timeline.steps[1]
            .visual
            .path_certificate_reference_v1
            .as_ref()
            .expect("first Miura path reference");
        let second_reference = reopened_timeline.steps[2]
            .visual
            .path_certificate_reference_v1
            .as_ref()
            .expect("second Miura path reference");
        assert_eq!(first_reference.transition_count, 2);
        assert_eq!(second_reference.transition_count, 2);
        assert_eq!(
            first_reference.binding_sha256,
            second_reference.binding_sha256
        );
        assert_eq!(
            first_reference.target_pose_sha256,
            second_reference.source_pose_sha256
        );
        for (format, magic) in [
            (InstructionExportFormatRequest::Pdf, b"%PDF-1.7".as_slice()),
            (InstructionExportFormatRequest::SvgZip, b"PK".as_slice()),
        ] {
            let mut reopened_source = source_for(&project, format);
            reopened_source.timeline = reopened_timeline.clone();
            let reopened = build_pending_export(reopened_source)
                .expect("native export after proof-bearing reopen");
            assert_eq!(reopened.step_count, 3);
            assert!(reopened.bytes.starts_with(magic));
        }

        // The same real, two-transition certificate must survive an archive round-trip
        // when it is authored as a general reverse-fold technique, not only as Miura.
        let mut reverse_timeline = reopened_timeline.clone();
        reverse_timeline.steps[0].title = "中割り折りの開始姿勢".to_owned();
        reverse_timeline.steps[1].title = "中割り折り 1".to_owned();
        reverse_timeline.steps[2].title = "中割り折り 2".to_owned();
        let reverse_archive = serde_json::to_vec(&reverse_timeline).expect("archive reverse fold");
        let reopened_reverse: ori_domain::InstructionTimeline =
            serde_json::from_slice(&reverse_archive).expect("reopen reverse fold");
        for step in &reopened_reverse.steps[1..] {
            let reference = step
                .visual
                .path_certificate_reference_v1
                .as_ref()
                .expect("reverse fold path reference");
            assert_eq!(reference.transition_count, 2);
            assert_eq!(reference.binding_sha256, first_reference.binding_sha256);
        }
        for (format, magic) in [
            (InstructionExportFormatRequest::Pdf, b"%PDF-1.7".as_slice()),
            (InstructionExportFormatRequest::SvgZip, b"PK".as_slice()),
        ] {
            let mut reverse_source = source_for(&project, format);
            reverse_source.timeline = reopened_reverse.clone();
            let artifact =
                build_pending_export(reverse_source).expect("native reverse-fold export");
            assert_eq!(artifact.step_count, 3);
            assert!(artifact.bytes.starts_with(magic));
        }

        let mut sink_timeline = reopened_reverse.clone();
        sink_timeline.steps[0].title = "沈め折りの開始姿勢".to_owned();
        sink_timeline.steps[1].title = "沈め折り 1".to_owned();
        sink_timeline.steps[2].title = "沈め折り 2".to_owned();
        let sink_archive = serde_json::to_vec(&sink_timeline).expect("archive sink fold");
        let reopened_sink: ori_domain::InstructionTimeline =
            serde_json::from_slice(&sink_archive).expect("reopen sink fold");
        for (format, magic) in [
            (InstructionExportFormatRequest::Pdf, b"%PDF-1.7".as_slice()),
            (InstructionExportFormatRequest::SvgZip, b"PK".as_slice()),
        ] {
            let mut sink_source = source_for(&project, format);
            sink_source.timeline = reopened_sink.clone();
            let artifact = build_pending_export(sink_source).expect("native sink-fold export");
            assert_eq!(artifact.step_count, 3);
            assert!(artifact.bytes.starts_with(magic));
        }

        // A manually authored sink remains exportable without being presented as
        // certificate-backed: removing the DTO also removes its proof summary.
        let mut uncertified_sink = reopened_sink.clone();
        for step in &mut uncertified_sink.steps[1..] {
            step.visual.path_certificate_reference_v1 = None;
            step.description = "手動で作成された沈め折りです。".to_owned();
        }
        for format in [
            InstructionExportFormatRequest::Pdf,
            InstructionExportFormatRequest::SvgZip,
        ] {
            let mut sink_source = source_for(&project, format);
            sink_source.timeline = uncertified_sink.clone();
            let artifact = build_pending_export(sink_source)
                .expect("uncertified sink keeps the non-proof export boundary");
            assert_eq!(artifact.step_count, 3);
        }

        let mut accordion_timeline = reopened_sink.clone();
        accordion_timeline.steps[0].title = "蛇腹折りの開始姿勢".to_owned();
        accordion_timeline.steps[1].title = "蛇腹折り 1".to_owned();
        accordion_timeline.steps[2].title = "蛇腹折り 2".to_owned();
        let accordion_archive =
            serde_json::to_vec(&accordion_timeline).expect("archive accordion fold");
        let reopened_accordion: ori_domain::InstructionTimeline =
            serde_json::from_slice(&accordion_archive).expect("reopen accordion fold");
        for (format, magic) in [
            (InstructionExportFormatRequest::Pdf, b"%PDF-1.7".as_slice()),
            (InstructionExportFormatRequest::SvgZip, b"PK".as_slice()),
        ] {
            let mut source = source_for(&project, format);
            source.timeline = reopened_accordion.clone();
            let artifact = build_pending_export(source).expect("native accordion-fold export");
            assert_eq!(artifact.step_count, 3);
            assert!(artifact.bytes.starts_with(magic));
        }

        let mut uncertified_accordion = reopened_accordion;
        for step in &mut uncertified_accordion.steps[1..] {
            step.visual.path_certificate_reference_v1 = None;
            step.description = "手動で作成された蛇腹折りです。".to_owned();
        }
        for format in [
            InstructionExportFormatRequest::Pdf,
            InstructionExportFormatRequest::SvgZip,
        ] {
            let mut source = source_for(&project, format);
            source.timeline = uncertified_accordion.clone();
            assert!(build_pending_export(source).is_ok());
        }

        let mut layer_selective_timeline = sink_timeline;
        layer_selective_timeline.steps[0].title = "層選択折りの開始姿勢".to_owned();
        layer_selective_timeline.steps[1].title = "層選択折り 1".to_owned();
        layer_selective_timeline.steps[2].title = "層選択折り 2".to_owned();
        let layer_selective_archive =
            serde_json::to_vec(&layer_selective_timeline).expect("archive layer-selective fold");
        let reopened_layer_selective: ori_domain::InstructionTimeline =
            serde_json::from_slice(&layer_selective_archive).expect("reopen layer-selective fold");
        for (format, magic) in [
            (InstructionExportFormatRequest::Pdf, b"%PDF-1.7".as_slice()),
            (InstructionExportFormatRequest::SvgZip, b"PK".as_slice()),
        ] {
            let mut source = source_for(&project, format);
            source.timeline = reopened_layer_selective.clone();
            let artifact = build_pending_export(source).expect("native layer-selective export");
            assert_eq!(artifact.step_count, 3);
            assert!(artifact.bytes.starts_with(magic));
        }

        let mut uncertified_layer_selective = reopened_layer_selective;
        for step in &mut uncertified_layer_selective.steps[1..] {
            step.visual.path_certificate_reference_v1 = None;
            step.description = "手動で作成された層選択折りです。".to_owned();
        }
        for format in [
            InstructionExportFormatRequest::Pdf,
            InstructionExportFormatRequest::SvgZip,
        ] {
            let mut source = source_for(&project, format);
            source.timeline = uncertified_layer_selective.clone();
            assert!(build_pending_export(source).is_ok());
        }

        let mut book_fold_timeline = reverse_timeline;
        book_fold_timeline.steps[0].title = "二つ折りの開始姿勢".to_owned();
        book_fold_timeline.steps[1].title = "二つ折り 1".to_owned();
        book_fold_timeline.steps[2].title = "二つ折り 2".to_owned();
        let book_fold_archive = serde_json::to_vec(&book_fold_timeline).expect("archive book fold");
        let reopened_book_fold: ori_domain::InstructionTimeline =
            serde_json::from_slice(&book_fold_archive).expect("reopen book fold");
        for (format, magic) in [
            (InstructionExportFormatRequest::Pdf, b"%PDF-1.7".as_slice()),
            (InstructionExportFormatRequest::SvgZip, b"PK".as_slice()),
        ] {
            let mut source = source_for(&project, format);
            source.timeline = reopened_book_fold.clone();
            let artifact = build_pending_export(source).expect("native book-fold export");
            assert_eq!(artifact.step_count, 3);
            assert!(artifact.bytes.starts_with(magic));
        }

        let mut uncertified_book_fold = reopened_book_fold;
        for step in &mut uncertified_book_fold.steps[1..] {
            step.visual.path_certificate_reference_v1 = None;
            step.description = "手動で作成された二つ折りです。".to_owned();
        }
        for format in [
            InstructionExportFormatRequest::Pdf,
            InstructionExportFormatRequest::SvgZip,
        ] {
            let mut source = source_for(&project, format);
            source.timeline = uncertified_book_fold.clone();
            assert!(build_pending_export(source).is_ok());
        }

        let mut outside_reverse_timeline = reopened_reverse.clone();
        outside_reverse_timeline.steps[0].title = "外割り折りの開始姿勢".to_owned();
        outside_reverse_timeline.steps[1].title = "外割り折り 1".to_owned();
        outside_reverse_timeline.steps[2].title = "外割り折り 2".to_owned();
        let outside_reverse_archive =
            serde_json::to_vec(&outside_reverse_timeline).expect("archive outside reverse fold");
        let reopened_outside_reverse: ori_domain::InstructionTimeline =
            serde_json::from_slice(&outside_reverse_archive).expect("reopen outside reverse fold");
        assert_ne!(
            reopened_outside_reverse.steps[2].title, reopened_reverse.steps[2].title,
            "outside and inside reverse folds remain explicitly distinguishable"
        );
        for (format, magic) in [
            (InstructionExportFormatRequest::Pdf, b"%PDF-1.7".as_slice()),
            (InstructionExportFormatRequest::SvgZip, b"PK".as_slice()),
        ] {
            let mut source = source_for(&project, format);
            source.timeline = reopened_outside_reverse.clone();
            let artifact = build_pending_export(source).expect("native outside-reverse export");
            assert_eq!(artifact.step_count, 3);
            assert!(artifact.bytes.starts_with(magic));
        }

        let mut uncertified_outside_reverse = reopened_outside_reverse;
        for step in &mut uncertified_outside_reverse.steps[1..] {
            step.visual.path_certificate_reference_v1 = None;
            step.description = "手動で作成された外割り折りです。".to_owned();
        }
        for format in [
            InstructionExportFormatRequest::Pdf,
            InstructionExportFormatRequest::SvgZip,
        ] {
            let mut source = source_for(&project, format);
            source.timeline = uncertified_outside_reverse.clone();
            assert!(build_pending_export(source).is_ok());
        }

        let mut squash_timeline = reopened_reverse.clone();
        squash_timeline.steps[0].title = "つぶし折りの開始姿勢".to_owned();
        squash_timeline.steps[1].title = "つぶし折り 1".to_owned();
        squash_timeline.steps[2].title = "つぶし折り 2".to_owned();
        let squash_archive = serde_json::to_vec(&squash_timeline).expect("archive squash fold");
        let reopened_squash: ori_domain::InstructionTimeline =
            serde_json::from_slice(&squash_archive).expect("reopen squash fold");
        for (format, magic) in [
            (InstructionExportFormatRequest::Pdf, b"%PDF-1.7".as_slice()),
            (InstructionExportFormatRequest::SvgZip, b"PK".as_slice()),
        ] {
            let mut source = source_for(&project, format);
            source.timeline = reopened_squash.clone();
            let artifact = build_pending_export(source).expect("native squash-fold export");
            assert_eq!(artifact.step_count, 3);
            assert!(artifact.bytes.starts_with(magic));
        }
        let mut uncertified_squash = reopened_squash;
        for step in &mut uncertified_squash.steps[1..] {
            step.visual.path_certificate_reference_v1 = None;
            step.description = "手動で作成されたつぶし折りです。".to_owned();
        }
        for format in [
            InstructionExportFormatRequest::Pdf,
            InstructionExportFormatRequest::SvgZip,
        ] {
            let mut source = source_for(&project, format);
            source.timeline = uncertified_squash.clone();
            assert!(build_pending_export(source).is_ok());
        }

        let mut petal_timeline = reopened_reverse.clone();
        petal_timeline.steps[0].title = "花弁折りの開始姿勢".to_owned();
        petal_timeline.steps[1].title = "花弁折り 1".to_owned();
        petal_timeline.steps[2].title = "花弁折り 2".to_owned();
        for step in &mut petal_timeline.steps[1..] {
            step.visual.path_certificate_reference_v1 = None;
            step.description = "手動で作成された花弁折りです。物理運動は未証明です。".to_owned();
        }
        let petal_archive = serde_json::to_vec(&petal_timeline).expect("archive petal fold");
        let reopened_petal: ori_domain::InstructionTimeline =
            serde_json::from_slice(&petal_archive).expect("reopen petal fold");
        for (format, magic) in [
            (InstructionExportFormatRequest::Pdf, b"%PDF-1.7".as_slice()),
            (InstructionExportFormatRequest::SvgZip, b"PK".as_slice()),
        ] {
            let mut source = source_for(&project, format);
            source.timeline = reopened_petal.clone();
            let artifact = build_pending_export(source).expect("uncertified petal-fold export");
            assert_eq!(artifact.step_count, 3);
            assert!(artifact.bytes.starts_with(magic));
        }
        let mut crimp_timeline = reopened_reverse.clone();
        crimp_timeline.steps[0].title = "段折りの開始姿勢".to_owned();
        crimp_timeline.steps[1].title = "段折り 1".to_owned();
        crimp_timeline.steps[2].title = "段折り 2".to_owned();
        let crimp_archive = serde_json::to_vec(&crimp_timeline).expect("archive crimp fold");
        let reopened_crimp: ori_domain::InstructionTimeline =
            serde_json::from_slice(&crimp_archive).expect("reopen crimp fold");
        for (format, magic) in [
            (InstructionExportFormatRequest::Pdf, b"%PDF-1.7".as_slice()),
            (InstructionExportFormatRequest::SvgZip, b"PK".as_slice()),
        ] {
            let mut source = source_for(&project, format);
            source.timeline = reopened_crimp.clone();
            let artifact = build_pending_export(source).expect("native crimp-fold export");
            assert_eq!(artifact.step_count, 3);
            assert!(artifact.bytes.starts_with(magic));
        }
        let mut uncertified_crimp = reopened_crimp;
        for step in &mut uncertified_crimp.steps[1..] {
            step.visual.path_certificate_reference_v1 = None;
            step.description = "手動で作成された段折りです。".to_owned();
        }
        for format in [
            InstructionExportFormatRequest::Pdf,
            InstructionExportFormatRequest::SvgZip,
        ] {
            let mut source = source_for(&project, format);
            source.timeline = uncertified_crimp.clone();
            assert!(build_pending_export(source).is_ok());
        }

        let mut tampered_reverse = reopened_reverse.clone();
        tampered_reverse.steps[2]
            .visual
            .path_certificate_reference_v1
            .as_mut()
            .expect("tamper reverse fold path reference")
            .source_model_binding_sha256[0] ^= 1;
        for format in [
            InstructionExportFormatRequest::Pdf,
            InstructionExportFormatRequest::SvgZip,
        ] {
            let mut source = source_for(&project, format);
            source.timeline = tampered_reverse.clone();
            assert_eq!(
                build_pending_export(source).map(|_| ()),
                Err(InstructionExportErrorCategory::DocumentInputInvalid)
            );
        }

        let mut tampered_archive: serde_json::Value =
            serde_json::from_slice(&archived).expect("inspect archived timeline");
        tampered_archive["steps"][2]["description"] = serde_json::Value::String(format!(
            "経路証明 SHA-256: {} / 元モデル SHA-256: {}",
            "7c".repeat(32),
            "b".repeat(64)
        ));
        let mut tampered = source_for(&project, InstructionExportFormatRequest::SvgZip);
        tampered.timeline =
            serde_json::from_value(tampered_archive).expect("reopen tampered archive");
        assert_eq!(
            build_pending_export(tampered).map(|_| ()),
            Err(InstructionExportErrorCategory::DocumentInputInvalid),
            "strict IPC exposes only the closed error category"
        );

        let mut stale_revision = source_for(&project, InstructionExportFormatRequest::Pdf);
        stale_revision.timeline = reopened_timeline;
        stale_revision.expected_revision += 1;
        assert_eq!(
            build_pending_export(stale_revision).map(|_| ()),
            Err(InstructionExportErrorCategory::DocumentContractInvalid)
        );
    }

    #[test]
    fn warnings_are_always_present_and_acknowledgement_is_enforced() {
        let project = project_with_instruction();
        let pending = fake_pending(&project, InstructionExportFormatRequest::Pdf, b"%PDF");
        assert_eq!(pending.warnings.len(), 4);
        assert!(
            pending
                .warnings
                .iter()
                .any(|warning| warning.category == "fixed_automatic_camera"
                    && warning.message_ja.contains("カメラ"))
        );
        assert!(
            pending
                .warnings
                .iter()
                .any(|warning| warning.category == "visual_effects_omitted"
                    && warning.message_ja.contains("透明"))
        );
        assert!(
            pending
                .warnings
                .iter()
                .any(|warning| warning.category == "authored_guides_omitted"
                    && warning.message_ja.contains("持ち替え"))
        );
        assert!(
            pending
                .warnings
                .iter()
                .any(|warning| warning.category == "discrete_step_endpoints_only"
                    && warning.message_ja.contains("連続動作"))
        );
        assert_eq!(
            require_warning_acknowledgement(&pending, false),
            Err(InstructionExportErrorCategory::WarningAcknowledgementRequired)
        );
        require_warning_acknowledgement(&pending, true).unwrap();
    }

    #[test]
    fn source_limits_reject_before_topology_analysis_and_progress_is_closed() {
        let limits = ori_formats::InstructionDiagramLimits::default();
        validate_source_counts(limits.max_source_vertices, limits.max_source_edges)
            .expect("inclusive source limits");
        assert_eq!(
            validate_source_counts(limits.max_source_vertices + 1, limits.max_source_edges)
                .unwrap_err(),
            InstructionExportErrorCategory::SourceLimitExceeded
        );
        assert_eq!(
            validate_source_counts(limits.max_source_vertices, limits.max_source_edges + 1)
                .unwrap_err(),
            InstructionExportErrorCategory::SourceLimitExceeded
        );

        assert_eq!(
            instruction_export_phase(PHASE_VALIDATING).unwrap(),
            InstructionExportPhase::Validating
        );
        assert_eq!(
            instruction_export_phase(PHASE_ANALYZING_TOPOLOGY).unwrap(),
            InstructionExportPhase::AnalyzingTopology
        );
        assert_eq!(
            instruction_export_phase(PHASE_BUILDING_DOCUMENT).unwrap(),
            InstructionExportPhase::BuildingDocument
        );
        assert_eq!(
            instruction_export_phase(PHASE_READY).unwrap(),
            InstructionExportPhase::Ready
        );
        assert_eq!(
            instruction_export_phase(u8::MAX),
            Err(InstructionExportErrorCategory::GenerationUnavailable)
        );

        let phase = AtomicU8::new(PHASE_VALIDATING);
        advance_generation_phase(&phase, PHASE_ANALYZING_TOPOLOGY).unwrap();
        advance_generation_phase(&phase, PHASE_BUILDING_DOCUMENT).unwrap();
        advance_generation_phase(&phase, PHASE_READY).unwrap();
        assert_eq!(
            advance_generation_phase(&phase, PHASE_VALIDATING),
            Err(InstructionExportErrorCategory::DocumentContractInvalid)
        );
        assert_eq!(phase.load(Ordering::SeqCst), PHASE_READY);
    }

    #[test]
    fn generation_token_is_claimed_once_and_cannot_switch_format_or_reverse_progress() {
        let state = InstructionExportState::default();
        let export_id = ProjectId::new();
        begin_export_generation(&state, export_id).unwrap();

        let (_, phase) =
            claim_generation(&state, export_id, InstructionExportFormatRequest::Pdf).unwrap();
        advance_generation_phase(&phase, PHASE_BUILDING_DOCUMENT).unwrap();
        let error = claim_generation(&state, export_id, InstructionExportFormatRequest::SvgZip)
            .unwrap_err();
        assert_eq!(error, InstructionExportErrorCategory::GenerationUnavailable);
        assert_eq!(phase.load(Ordering::SeqCst), PHASE_BUILDING_DOCUMENT);
        let slot = lock_instruction_export(&state).unwrap();
        assert_eq!(
            slot.active
                .as_ref()
                .and_then(|active| active.claimed_format),
            Some(InstructionExportFormatRequest::Pdf)
        );
    }

    #[test]
    fn concurrent_generation_claim_allows_exactly_one_format() {
        let state = Arc::new(InstructionExportState::default());
        let export_id = ProjectId::new();
        begin_export_generation(&state, export_id).unwrap();
        let barrier = Arc::new(Barrier::new(2));
        let workers = [
            InstructionExportFormatRequest::Pdf,
            InstructionExportFormatRequest::SvgZip,
        ]
        .map(|format| {
            let state = Arc::clone(&state);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                (format, claim_generation(&state, export_id, format))
            })
        });
        let results = workers.map(|worker| worker.join().unwrap());
        assert_eq!(
            results.iter().filter(|(_, result)| result.is_ok()).count(),
            1
        );
        assert_eq!(
            results.iter().filter(|(_, result)| result.is_err()).count(),
            1
        );
        let winning_format = results
            .iter()
            .find_map(|(format, result)| result.is_ok().then_some(*format))
            .unwrap();
        let slot = lock_instruction_export(&state).unwrap();
        let active = slot.active.as_ref().unwrap();
        assert_eq!(active.claimed_format, Some(winning_format));
        assert_eq!(active.phase.load(Ordering::SeqCst), PHASE_VALIDATING);
    }

    #[test]
    fn native_stage_rejects_unknown_artifact_profiles_and_warning_sets() {
        let project = project_with_instruction();
        let source = source_for(&project, InstructionExportFormatRequest::Pdf);
        let topology = source.topology_input.analyze();
        let snapshot = topology.simulation_snapshot().unwrap();
        let mut artifact = export_instruction_document(
            InstructionExportFormat::Pdf17,
            &source.name,
            &source.current_fold_model_fingerprint,
            &source.pattern,
            &source.paper,
            &source.timeline,
            snapshot,
        )
        .unwrap();
        validate_artifact_contract(
            InstructionExportFormatRequest::Pdf,
            &source.timeline,
            &artifact,
        )
        .unwrap();

        artifact.profile = "instruction_export_future";
        assert!(
            validate_artifact_contract(
                InstructionExportFormatRequest::Pdf,
                &source.timeline,
                &artifact,
            )
            .is_err()
        );
        artifact.profile = INSTRUCTION_EXPORT_PROFILE;
        artifact.projection_profile = "unknown_projection";
        assert!(
            validate_artifact_contract(
                InstructionExportFormatRequest::Pdf,
                &source.timeline,
                &artifact,
            )
            .is_err()
        );
        artifact.projection_profile = INSTRUCTION_EXPORT_PROJECTION_PROFILE;
        artifact.warnings.pop();
        assert!(
            validate_artifact_contract(
                InstructionExportFormatRequest::Pdf,
                &source.timeline,
                &artifact,
            )
            .is_err()
        );
    }

    #[test]
    fn replacement_tokens_and_cancellation_fail_closed() {
        let project = project_with_instruction();
        let state = InstructionExportState::default();
        let first_id = ProjectId::new();
        let first_cancellation = begin_export_generation(&state, first_id).unwrap();
        let mut first = fake_pending(&project, InstructionExportFormatRequest::Pdf, b"first");
        first.export_id = first_id;
        lock_instruction_export(&state).unwrap().pending = Some(first);

        let second_id = ProjectId::new();
        let second_cancellation = begin_export_generation(&state, second_id).unwrap();
        assert!(first_cancellation.load(Ordering::SeqCst));
        let mut second = fake_pending(&project, InstructionExportFormatRequest::SvgZip, b"second");
        second.export_id = second_id;
        lock_instruction_export(&state).unwrap().pending = Some(second);

        assert!(cancel_export_generation(&state, first_id).is_err());
        assert!(!second_cancellation.load(Ordering::SeqCst));
        assert_eq!(
            lock_instruction_export(&state)
                .unwrap()
                .pending
                .as_ref()
                .map(|pending| pending.export_id),
            Some(second_id)
        );
        cancel_export_generation(&state, second_id).unwrap();
        assert!(second_cancellation.load(Ordering::SeqCst));
        cancel_export_generation(&state, second_id).unwrap();
    }

    #[test]
    fn cancelled_background_source_never_becomes_a_stage() {
        let project = project_with_instruction();
        let source = source_for(&project, InstructionExportFormatRequest::Pdf);
        source.cancellation.store(true, Ordering::SeqCst);

        assert_eq!(
            build_pending_export(source)
                .err()
                .expect("cancelled work must fail"),
            InstructionExportErrorCategory::GenerationCancelled
        );
    }

    #[test]
    fn stale_project_state_is_rejected_without_consuming_the_stage() {
        let mut project = project_with_instruction();
        let pending = fake_pending(&project, InstructionExportFormatRequest::Pdf, b"%PDF");
        let export_id = pending.export_id;
        let state = InstructionExportState::default();
        {
            let mut slot = lock_instruction_export(&state).unwrap();
            slot.active = Some(ActiveInstructionExport {
                export_id,
                cancellation: Arc::new(AtomicBool::new(false)),
                phase: Arc::new(AtomicU8::new(PHASE_READY)),
                claimed_format: Some(InstructionExportFormatRequest::Pdf),
            });
            slot.pending = Some(pending);
        }
        project
            .editor
            .execute(
                project.editor.revision(),
                Command::AddVertex {
                    id: VertexId::new(),
                    position: Point2::new(25.0, 25.0),
                },
            )
            .unwrap();

        let slot = lock_instruction_export(&state).unwrap();
        assert!(
            checked_pending(
                &slot,
                &project,
                export_id,
                project.project_id,
                project.editor.revision() - 1,
            )
            .is_err()
        );
        assert!(slot.pending.is_some());
    }

    #[test]
    fn atomic_save_persists_exact_bytes_for_every_format_without_mutating_project() {
        let project = project_with_instruction();
        let before = project.document();
        let was_dirty = project.is_dirty();
        let directory = TestDirectory::new();
        for (format, bytes) in [
            (
                InstructionExportFormatRequest::Pdf,
                b"%PDF-exact".as_slice(),
            ),
            (
                InstructionExportFormatRequest::SvgZip,
                b"PK\x03\x04-exact".as_slice(),
            ),
        ] {
            let pending = fake_pending(&project, format, bytes);
            let export_id = pending.export_id;
            let expected = Arc::clone(&pending.bytes);
            let mut slot = InstructionExportSlot {
                active: Some(ActiveInstructionExport {
                    export_id,
                    cancellation: Arc::new(AtomicBool::new(false)),
                    phase: Arc::new(AtomicU8::new(PHASE_READY)),
                    claimed_format: Some(format),
                }),
                pending: Some(pending),
                last_cancelled_id: None,
            };
            let path = directory
                .path()
                .join(format!("sample.{}", format.extension()));
            fs::write(&path, b"previous export").unwrap();
            let destination = ensure_export_extension(path.clone(), format)
                .expect("accept the existing dialog-confirmed destination");

            commit_pending_export_to_destination(
                &mut slot,
                &project,
                export_id,
                project.project_id,
                project.editor.revision(),
                true,
                &destination,
            )
            .unwrap();

            assert_eq!(fs::read(&path).unwrap(), expected.as_ref());
            assert!(slot.pending.is_none());
            assert!(slot.active.is_none());
            assert_eq!(project.document(), before);
            assert_eq!(project.is_dirty(), was_dirty);
        }
    }

    #[test]
    fn a_saved_token_cannot_cancel_or_write_again() {
        let project = project_with_instruction();
        let pending = fake_pending(&project, InstructionExportFormatRequest::Pdf, b"%PDF-once");
        let export_id = pending.export_id;
        let state = InstructionExportState::default();
        {
            let mut slot = lock_instruction_export(&state).unwrap();
            slot.active = Some(ActiveInstructionExport {
                export_id,
                cancellation: Arc::new(AtomicBool::new(false)),
                phase: Arc::new(AtomicU8::new(PHASE_READY)),
                claimed_format: Some(InstructionExportFormatRequest::Pdf),
            });
            slot.pending = Some(pending);
        }
        let directory = TestDirectory::new();
        let path = directory.path().join("once.pdf");
        {
            let mut slot = lock_instruction_export(&state).unwrap();
            commit_pending_export_to_path(
                &mut slot,
                &project,
                export_id,
                project.project_id,
                project.editor.revision(),
                true,
                &path,
            )
            .unwrap();
            assert!(
                commit_pending_export_to_path(
                    &mut slot,
                    &project,
                    export_id,
                    project.project_id,
                    project.editor.revision(),
                    true,
                    &path,
                )
                .is_err()
            );
        }
        assert!(cancel_export_generation(&state, export_id).is_err());
        assert_eq!(fs::read(path).unwrap(), b"%PDF-once");
    }

    #[test]
    fn io_failure_does_not_consume_the_stage() {
        let project = project_with_instruction();
        let pending = fake_pending(&project, InstructionExportFormatRequest::SvgZip, b"exact");
        let export_id = pending.export_id;
        let state = InstructionExportState::default();
        {
            let mut slot = lock_instruction_export(&state).unwrap();
            slot.active = Some(ActiveInstructionExport {
                export_id,
                cancellation: Arc::new(AtomicBool::new(false)),
                phase: Arc::new(AtomicU8::new(PHASE_READY)),
                claimed_format: Some(InstructionExportFormatRequest::SvgZip),
            });
            slot.pending = Some(pending);
        }
        let directory = TestDirectory::new();
        let path = directory.path().join("missing").join("sample.zip");

        let mut slot = lock_instruction_export(&state).unwrap();
        assert_eq!(
            commit_pending_export_to_path(
                &mut slot,
                &project,
                export_id,
                project.project_id,
                project.editor.revision(),
                true,
                &path,
            ),
            Err(InstructionExportErrorCategory::SaveFailed)
        );
        assert!(slot.pending.is_some());
        assert_eq!(
            slot.active.as_ref().map(|active| active.export_id),
            Some(export_id)
        );
        assert!(!path.exists());

        fs::create_dir(path.parent().unwrap()).unwrap();
        commit_pending_export_to_path(
            &mut slot,
            &project,
            export_id,
            project.project_id,
            project.editor.revision(),
            true,
            &path,
        )
        .unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"exact");
        assert!(slot.pending.is_none());
        assert!(slot.active.is_none());
    }

    #[test]
    fn replacement_failure_cleans_temporary_file_and_allows_retry() {
        let project = project_with_instruction();
        let pending = fake_pending(
            &project,
            InstructionExportFormatRequest::SvgZip,
            b"replacement",
        );
        let export_id = pending.export_id;
        let mut slot = InstructionExportSlot {
            active: Some(ActiveInstructionExport {
                export_id,
                cancellation: Arc::new(AtomicBool::new(false)),
                phase: Arc::new(AtomicU8::new(PHASE_READY)),
                claimed_format: Some(InstructionExportFormatRequest::SvgZip),
            }),
            pending: Some(pending),
            last_cancelled_id: None,
        };
        let directory = TestDirectory::new();
        let occupied = directory.path().join("occupied.zip");
        fs::create_dir(&occupied).unwrap();

        let result = commit_pending_export_to_path(
            &mut slot,
            &project,
            export_id,
            project.project_id,
            project.editor.revision(),
            true,
            &occupied,
        );
        assert_eq!(result, Err(InstructionExportErrorCategory::SaveFailed));
        let serialized = serde_json::to_string(&InstructionExportCommandError::from(
            InstructionExportErrorCategory::SaveFailed,
        ))
        .unwrap();
        assert!(!serialized.contains("occupied.zip"));
        assert!(!serialized.contains(&directory.path().display().to_string()));
        assert!(occupied.is_dir());
        assert!(slot.pending.is_some());
        assert!(
            fs::read_dir(directory.path())
                .unwrap()
                .filter_map(Result::ok)
                .all(|entry| !entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".origami2-"))
        );

        let recovered = directory.path().join("recovered.zip");
        commit_pending_export_to_path(
            &mut slot,
            &project,
            export_id,
            project.project_id,
            project.editor.revision(),
            true,
            &recovered,
        )
        .unwrap();
        assert_eq!(fs::read(recovered).unwrap(), b"replacement");
    }

    #[test]
    fn warning_rejection_does_not_write_or_consume_the_stage() {
        let project = project_with_instruction();
        let pending = fake_pending(&project, InstructionExportFormatRequest::Pdf, b"%PDF");
        let export_id = pending.export_id;
        let cancellation = Arc::new(AtomicBool::new(false));
        let mut slot = InstructionExportSlot {
            active: Some(ActiveInstructionExport {
                export_id,
                cancellation,
                phase: Arc::new(AtomicU8::new(PHASE_READY)),
                claimed_format: Some(InstructionExportFormatRequest::Pdf),
            }),
            pending: Some(pending),
            last_cancelled_id: None,
        };
        let directory = TestDirectory::new();
        let path = directory.path().join("unacknowledged.pdf");

        assert!(
            commit_pending_export_to_path(
                &mut slot,
                &project,
                export_id,
                project.project_id,
                project.editor.revision(),
                false,
                &path,
            )
            .is_err()
        );
        assert!(slot.pending.is_some());
        assert!(!path.exists());
    }

    #[test]
    fn extension_format_and_wire_contracts_are_closed() {
        for (format, exporter, extension, media_type, wire) in [
            (
                InstructionExportFormatRequest::Pdf,
                InstructionExportFormat::Pdf17,
                "pdf",
                "application/pdf",
                "pdf",
            ),
            (
                InstructionExportFormatRequest::SvgZip,
                InstructionExportFormat::SvgPageZip,
                "zip",
                "application/zip",
                "svg_zip",
            ),
        ] {
            assert_eq!(format.exporter_format(), exporter);
            assert_eq!(format.extension(), extension);
            assert_eq!(format.media_type(), media_type);
            assert!(!format.filter_label().is_empty());
            assert!(!format.format_summary().is_empty());
            assert_eq!(
                serde_json::to_value(format).unwrap(),
                serde_json::json!(wire)
            );
            assert_eq!(
                serde_json::from_value::<InstructionExportFormatRequest>(serde_json::json!(wire))
                    .unwrap(),
                format
            );
        }
        for invalid in [
            serde_json::json!(""),
            serde_json::json!("PDF"),
            serde_json::json!("svg"),
            serde_json::Value::Null,
        ] {
            assert!(serde_json::from_value::<InstructionExportFormatRequest>(invalid).is_err());
        }
        assert_eq!(
            ensure_export_extension(
                PathBuf::from("guide.txt"),
                InstructionExportFormatRequest::Pdf,
            )
            .unwrap(),
            PathBuf::from("guide.pdf")
        );
        assert_eq!(
            ensure_export_extension(
                PathBuf::from("guide.ZIP"),
                InstructionExportFormatRequest::SvgZip,
            )
            .unwrap(),
            PathBuf::from("guide.ZIP")
        );
        assert_eq!(
            suggested_export_file_name("  鶴:<test>/\u{0001}  ", "pdf"),
            "鶴__test___-折り図.pdf"
        );
    }

    #[test]
    fn extension_correction_cannot_target_an_existing_unconfirmed_instruction_export() {
        let directory = TestDirectory::new();
        let selected_path = directory.path().join("instructions.txt");
        let corrected_path = directory.path().join("instructions.pdf");
        fs::write(&corrected_path, b"keep existing instructions").unwrap();

        let category = ensure_export_extension(selected_path, InstructionExportFormatRequest::Pdf)
            .expect_err("an unconfirmed corrected destination must not be overwritten");
        assert_eq!(category, InstructionExportErrorCategory::SaveTargetInvalid);
        let serialized =
            serde_json::to_string(&InstructionExportCommandError::from(category)).unwrap();
        assert!(!serialized.contains("instructions.pdf"));
        assert!(!serialized.contains(&directory.path().display().to_string()));
        assert_eq!(
            fs::read(corrected_path).unwrap(),
            b"keep existing instructions"
        );
    }

    #[test]
    fn extension_correction_race_preserves_the_instruction_stage_and_new_destination() {
        let project = project_with_instruction();
        let pending = fake_pending(
            &project,
            InstructionExportFormatRequest::Pdf,
            b"%PDF-protected-stage",
        );
        let export_id = pending.export_id;
        let mut slot = InstructionExportSlot {
            active: Some(ActiveInstructionExport {
                export_id,
                cancellation: Arc::new(AtomicBool::new(false)),
                phase: Arc::new(AtomicU8::new(PHASE_READY)),
                claimed_format: Some(InstructionExportFormatRequest::Pdf),
            }),
            pending: Some(pending),
            last_cancelled_id: None,
        };
        let directory = TestDirectory::new();
        let selected_path = directory.path().join("race-target.txt");
        let corrected_path = directory.path().join("race-target.pdf");
        let destination =
            ensure_export_extension(selected_path, InstructionExportFormatRequest::Pdf)
                .expect("preflight an unused corrected path");
        fs::write(&corrected_path, b"created after extension preflight").unwrap();

        let result = commit_pending_export_to_destination(
            &mut slot,
            &project,
            export_id,
            project.project_id,
            project.editor.revision(),
            true,
            &destination,
        );

        assert_eq!(result, Err(InstructionExportErrorCategory::SaveFailed));
        assert_eq!(
            fs::read(&corrected_path).unwrap(),
            b"created after extension preflight"
        );
        assert_eq!(
            slot.pending.as_ref().map(|pending| pending.export_id),
            Some(export_id)
        );
        assert_eq!(
            slot.active.as_ref().map(|active| active.export_id),
            Some(export_id)
        );
        assert!(
            fs::read_dir(directory.path())
                .unwrap()
                .filter_map(Result::ok)
                .all(|entry| !entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".origami2-")),
            "a rejected create-new commit must clean its staged file"
        );
    }
}
