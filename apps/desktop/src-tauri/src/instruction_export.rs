use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicBool, AtomicU8, Ordering},
    },
};

use ori_core::TopologyAnalysisInput;
use ori_domain::ProjectId;
use ori_formats::{
    INSTRUCTION_EXPORT_PROFILE, INSTRUCTION_EXPORT_WARNINGS,
    INSTRUCTION_PROJECTION_PROFILE as INSTRUCTION_EXPORT_PROJECTION_PROFILE,
    InstructionDiagramError, InstructionExportArtifact, InstructionExportError,
    InstructionExportFormat, InstructionExportWarning, export_instruction_document,
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

use super::crease_export::persist_export_bytes_atomically;
use super::{
    AppState, ProjectState, ensure_expected_project, ensure_project_identity, lock_project,
};

const GENERATION_CANCELLED_MESSAGE: &str = "折り図の生成はキャンセルされました。";
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

    fn from_internal(error: &str) -> Self {
        let category = if error == GENERATION_CANCELLED_MESSAGE {
            InstructionExportErrorCategory::GenerationCancelled
        } else if error.contains("置き換え") {
            InstructionExportErrorCategory::GenerationReplaced
        } else if error.contains("状態を利用できません") || error.contains("state lock is poisoned")
        {
            InstructionExportErrorCategory::StateUnavailable
        } else if error.contains("active project changed")
            || error.contains("open project instance changed")
            || error.contains("project changed while")
            || error.contains("expected revision")
            || error.contains("生成中に展開図")
            || error.contains("別の編集状態")
        {
            InstructionExportErrorCategory::ProjectChanged
        } else if error.contains("折り手順が1件もない") {
            InstructionExportErrorCategory::TimelineEmpty
        } else if error.contains("現在の展開図より古い")
            || error == "折り図の手順が現在の展開図より古くなっています。"
        {
            InstructionExportErrorCategory::TimelineStale
        } else if error.contains("元データが上限") {
            InstructionExportErrorCategory::SourceLimitExceeded
        } else if error.contains("3D折り図を生成できる面構造")
            || error == "折り図の面構造または形状に対応していません。"
        {
            InstructionExportErrorCategory::TopologyUnsupported
        } else if error == "折り図に含められない入力があります。" {
            InstructionExportErrorCategory::DocumentInputInvalid
        } else if error == "折り図の出力上限を超えています。" {
            InstructionExportErrorCategory::DocumentLimitExceeded
        } else if error == "折り図データの生成に失敗しました。"
            || error.contains("生成処理を完了できませんでした")
        {
            InstructionExportErrorCategory::DocumentGenerationFailed
        } else if error.contains("生成された折り図") || error.contains("進捗を前の段階")
        {
            InstructionExportErrorCategory::DocumentContractInvalid
        } else if error.contains("制約に関する確認が必要") {
            InstructionExportErrorCategory::WarningAcknowledgementRequired
        } else if error.contains("保存先はファイルパスではありません")
            || error.contains("ローカルファイルではありません")
        {
            InstructionExportErrorCategory::SaveTargetInvalid
        } else if error.contains("一時ファイル")
            || error.contains("書き出しファイルを原子的に確定")
            || error.contains("保存先ディレクトリ")
        {
            InstructionExportErrorCategory::SaveFailed
        } else if error.contains("既に開始")
            || error.contains("既に破棄")
            || error.contains("進捗状態が不正")
            || error.contains("プレビューは既に")
            || error.contains("指定された折り図プレビューは存在")
            || error.contains("形式が生成開始時")
        {
            InstructionExportErrorCategory::GenerationUnavailable
        } else {
            InstructionExportErrorCategory::UnexpectedFailure
        };
        Self::new(category)
    }
}

impl From<String> for InstructionExportCommandError {
    fn from(error: String) -> Self {
        Self::from_internal(&error)
    }
}

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
                return Err("折り図の生成処理を完了できませんでした。".to_owned().into());
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
    let project = lock_project(&state)?;
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
        .ok_or_else(|| "この折り図生成は既に破棄されています。".to_owned())?;
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
        let project = lock_project(&state)?;
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
        let project = lock_project(&state)?;
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
        .map_err(|_| "選択された保存先はローカルファイルではありません。".to_owned())?;
    let path = ensure_export_extension(selected_path, pending.format);

    let mut slot = lock_instruction_export(&export_state)?;
    let project = lock_project(&state)?;
    commit_pending_export_to_path(
        &mut slot,
        &project,
        export_id,
        expected_project_id,
        expected_revision,
        warnings_acknowledged,
        &path,
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
) -> Result<MutexGuard<'_, InstructionExportSlot>, String> {
    state
        .0
        .lock()
        .map_err(|_| "折り図書き出し状態を利用できません。".to_owned())
}

fn begin_export_generation(
    state: &InstructionExportState,
    export_id: ProjectId,
) -> Result<Arc<AtomicBool>, String> {
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
) -> Result<(Arc<AtomicBool>, Arc<AtomicU8>), String> {
    let mut slot = lock_instruction_export(state)?;
    ensure_generation_is_current(&slot, export_id)?;
    if slot.pending.is_some() {
        return Err(
            "この折り図生成は既に開始されています。新しい生成を開始してください。".to_owned(),
        );
    }
    let active = slot
        .active
        .as_mut()
        .ok_or_else(|| "この折り図生成は既に破棄されています。".to_owned())?;
    if active.claimed_format.is_some() {
        return Err(
            "この折り図生成は既に開始されています。新しい生成を開始してください。".to_owned(),
        );
    }
    if active.phase.load(Ordering::SeqCst) != PHASE_VALIDATING {
        return Err("折り図生成の進捗状態が不正です。".to_owned());
    }
    active.claimed_format = Some(format);
    Ok((Arc::clone(&active.cancellation), Arc::clone(&active.phase)))
}

fn advance_generation_phase(phase: &AtomicU8, next: u8) -> Result<(), String> {
    if next > PHASE_READY {
        return Err("折り図生成の進捗状態が不正です。".to_owned());
    }
    let mut current = phase.load(Ordering::SeqCst);
    loop {
        if current > next {
            return Err("折り図生成の進捗を前の段階へ戻せません。".to_owned());
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

fn instruction_export_phase(value: u8) -> Result<InstructionExportPhase, String> {
    match value {
        PHASE_VALIDATING => Ok(InstructionExportPhase::Validating),
        PHASE_ANALYZING_TOPOLOGY => Ok(InstructionExportPhase::AnalyzingTopology),
        PHASE_BUILDING_DOCUMENT => Ok(InstructionExportPhase::BuildingDocument),
        PHASE_READY => Ok(InstructionExportPhase::Ready),
        _ => Err("折り図生成の進捗状態が不正です。".to_owned()),
    }
}

fn abandon_export_generation(
    state: &InstructionExportState,
    export_id: ProjectId,
) -> Result<(), String> {
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
) -> Result<(), String> {
    match slot.active.as_ref() {
        Some(active)
            if active.export_id == export_id && !active.cancellation.load(Ordering::SeqCst) =>
        {
            Ok(())
        }
        Some(_) => Err("この折り図生成は新しい処理に置き換えられました。".to_owned()),
        None if slot.last_cancelled_id == Some(export_id) => {
            Err(GENERATION_CANCELLED_MESSAGE.to_owned())
        }
        None => Err("この折り図生成は既に破棄されています。".to_owned()),
    }
}

fn ensure_not_cancelled(cancellation: &AtomicBool) -> Result<(), String> {
    if cancellation.load(Ordering::SeqCst) {
        Err(GENERATION_CANCELLED_MESSAGE.to_owned())
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
) -> Result<InstructionExportSource, String> {
    ensure_not_cancelled(&cancellation)?;
    let project = lock_project(state)?;
    ensure_project_identity(&project, expected_project_id)?;
    if project.editor.revision() != expected_revision {
        return Err(format!(
            "expected revision {expected_revision}, but the current revision is {}",
            project.editor.revision()
        ));
    }
    let timeline = project.editor.instruction_timeline().clone();
    if timeline.steps.is_empty() {
        return Err("折り手順が1件もないため、折り図を書き出せません。".to_owned());
    }
    validate_source_counts(
        project.editor.pattern().vertices.len(),
        project.editor.pattern().edges.len(),
    )?;
    let current_fold_model_fingerprint = project.editor.fold_model_fingerprint_v1();
    if let Some(step_index) = timeline
        .steps
        .iter()
        .position(|step| step.pose.source_model_fingerprint != current_fold_model_fingerprint)
    {
        return Err(format!(
            "手順{}は現在の展開図より古いため、姿勢を取り直してください。",
            step_index + 1
        ));
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
    })
}

fn build_pending_export(
    source: InstructionExportSource,
) -> Result<PendingInstructionExport, String> {
    ensure_not_cancelled(&source.cancellation)?;
    validate_source_counts(source.pattern.vertices.len(), source.pattern.edges.len())?;
    advance_generation_phase(&source.phase, PHASE_ANALYZING_TOPOLOGY)?;
    let topology = source.topology_input.analyze();
    ensure_not_cancelled(&source.cancellation)?;
    if topology.revision() != source.expected_revision {
        return Err("折り図用の面解析が予期しない編集リビジョンを返しました。".to_owned());
    }
    let snapshot = topology
        .simulation_snapshot()
        .ok_or_else(|| "現在の展開図は3D折り図を生成できる面構造になっていません。".to_owned())?;
    advance_generation_phase(&source.phase, PHASE_BUILDING_DOCUMENT)?;
    let artifact = export_instruction_document(
        source.format.exporter_format(),
        &source.name,
        &source.current_fold_model_fingerprint,
        &source.pattern,
        &source.paper,
        &source.timeline,
        snapshot,
    )
    .map_err(|error| instruction_document_failure_message(error).to_owned())?;
    ensure_not_cancelled(&source.cancellation)?;
    validate_artifact_contract(source.format, &source.timeline, &artifact)?;
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

fn instruction_document_failure_message(error: InstructionExportError) -> &'static str {
    match error {
        InstructionExportError::TitleTooLong { .. }
        | InstructionExportError::InvalidTitle
        | InstructionExportError::UnsupportedGlyph { .. } => "折り図に含められない入力があります。",
        InstructionExportError::Diagram(error) => match error {
            InstructionDiagramError::InvalidTimeline => "折り図に含められない入力があります。",
            InstructionDiagramError::EmptyTimeline => {
                "折り手順が1件もないため、折り図を書き出せません。"
            }
            InstructionDiagramError::StaleStep { .. } => {
                "折り図の手順が現在の展開図より古くなっています。"
            }
            InstructionDiagramError::UnsupportedTopology
            | InstructionDiagramError::UnrepresentableGeometry => {
                "折り図の面構造または形状に対応していません。"
            }
            InstructionDiagramError::ResourceLimitExceeded => "折り図の出力上限を超えています。",
        },
        InstructionExportError::LayoutLimitExceeded
        | InstructionExportError::PageTooLarge { .. }
        | InstructionExportError::OutputTooLarge { .. } => "折り図の出力上限を超えています。",
        InstructionExportError::InvalidBundledFont
        | InstructionExportError::FontAssetMismatch
        | InstructionExportError::StructureNotRepresentable
        | InstructionExportError::Zip(_)
        | InstructionExportError::Io(_)
        | InstructionExportError::Json(_) => "折り図データの生成に失敗しました。",
    }
}

fn validate_source_counts(vertex_count: usize, edge_count: usize) -> Result<(), String> {
    let limits = ori_formats::InstructionDiagramLimits::default();
    if vertex_count > limits.max_source_vertices || edge_count > limits.max_source_edges {
        return Err(format!(
            "折り図の元データが上限を超えています（頂点は最大{}件、辺は最大{}件）。",
            limits.max_source_vertices, limits.max_source_edges
        ));
    }
    Ok(())
}

fn validate_artifact_contract(
    requested: InstructionExportFormatRequest,
    timeline: &ori_domain::InstructionTimeline,
    artifact: &InstructionExportArtifact,
) -> Result<(), String> {
    if artifact.format != requested.exporter_format()
        || artifact.file_extension != requested.extension()
        || artifact.media_type != requested.media_type()
        || artifact.profile != INSTRUCTION_EXPORT_PROFILE
        || artifact.projection_profile != INSTRUCTION_EXPORT_PROJECTION_PROFILE
        || artifact.warnings != INSTRUCTION_EXPORT_WARNINGS
    {
        return Err("生成された折り図の形式が要求と一致しません。".to_owned());
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
        return Err("生成された折り図の件数または内容が入力と一致しません。".to_owned());
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
) -> Result<(), String> {
    ensure_expected_project(
        project,
        pending.expected_instance_id,
        pending.expected_project_id,
        pending.expected_revision,
    )?;
    if !pending
        .topology_input
        .is_current_for(project.project_id, &project.editor)
    {
        return Err("折り図の生成中に展開図または用紙設定が変わりました。".to_owned());
    }
    Ok(())
}

fn checked_pending<'a>(
    slot: &'a InstructionExportSlot,
    project: &ProjectState,
    export_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
) -> Result<&'a PendingInstructionExport, String> {
    let pending = slot
        .pending
        .as_ref()
        .ok_or_else(|| "折り図のプレビューは既に破棄されています。".to_owned())?;
    if pending.export_id != export_id {
        return Err("折り図のプレビューは新しいプレビューに置き換えられました。".to_owned());
    }
    if pending.expected_project_id != expected_project_id
        || pending.expected_revision != expected_revision
    {
        return Err("折り図のプレビューは別の編集状態に属しています。".to_owned());
    }
    ensure_generation_is_current(slot, export_id)?;
    let active = slot
        .active
        .as_ref()
        .ok_or_else(|| "この折り図生成は既に破棄されています。".to_owned())?;
    if active.claimed_format != Some(pending.format) {
        return Err("折り図の形式が生成開始時の指定と一致しません。".to_owned());
    }
    ensure_pending_is_current(pending, project)?;
    Ok(pending)
}

fn require_warning_acknowledgement(
    pending: &PendingInstructionExport,
    warnings_acknowledged: bool,
) -> Result<(), String> {
    if !pending.warnings.is_empty() && !warnings_acknowledged {
        Err("折り図の制約に関する確認が必要です。".to_owned())
    } else {
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
fn commit_pending_export_to_path(
    slot: &mut InstructionExportSlot,
    project: &ProjectState,
    export_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    warnings_acknowledged: bool,
    path: &Path,
) -> Result<(), String> {
    let pending = checked_pending(
        slot,
        project,
        export_id,
        expected_project_id,
        expected_revision,
    )?;
    require_warning_acknowledgement(pending, warnings_acknowledged)?;
    persist_export_bytes_atomically(path, &pending.bytes)?;
    slot.pending = None;
    slot.active = None;
    Ok(())
}

fn cancel_export_generation(
    state: &InstructionExportState,
    export_id: ProjectId,
) -> Result<(), String> {
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
        return Err("このプレビューは新しい折り図生成に置き換えられました。".to_owned());
    }
    Err("指定された折り図プレビューは存在しません。".to_owned())
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

fn ensure_export_extension(mut path: PathBuf, format: InstructionExportFormatRequest) -> PathBuf {
    let expected = format.extension();
    let already_expected = path
        .extension()
        .and_then(OsStr::to_str)
        .is_some_and(|extension| extension.eq_ignore_ascii_case(expected));
    if !already_expected {
        path.set_extension(expected);
    }
    path
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeSet,
        fs,
        sync::{
            Barrier,
            atomic::{AtomicU64, Ordering as AtomicOrdering},
        },
        thread,
    };

    use ori_core::Command;
    use ori_domain::{
        InstructionPose, InstructionPoseModel, InstructionStep, InstructionStepId, Point2, VertexId,
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
        let error = InstructionExportCommandError::from_internal(private_value);
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

        let unsupported_glyph =
            instruction_document_failure_message(InstructionExportError::UnsupportedGlyph {
                code_point: 0x1F4A5,
            });
        assert_eq!(unsupported_glyph, "折り図に含められない入力があります。");
        assert!(!unsupported_glyph.contains("1F4A5"));
        let output_limit =
            instruction_document_failure_message(InstructionExportError::OutputTooLarge {
                actual: usize::MAX,
                maximum: 1,
            });
        assert_eq!(output_limit, "折り図の出力上限を超えています。");
        assert!(!output_limit.contains(&usize::MAX.to_string()));
    }

    #[test]
    fn empty_and_stale_timelines_are_rejected_before_background_work() {
        let empty = super::super::initial_project_state();
        let empty_project_id = empty.project_id;
        let cancellation = Arc::new(AtomicBool::new(false));
        assert!(
            capture_export_source(
                &AppState(Mutex::new(empty)),
                ProjectId::new(),
                empty_project_id,
                0,
                InstructionExportFormatRequest::Pdf,
                cancellation,
                Arc::new(AtomicU8::new(PHASE_VALIDATING)),
            )
            .is_err()
        );

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
        let error = capture_export_source(
            &AppState(Mutex::new(stale)),
            ProjectId::new(),
            stale_project_id,
            stale_revision,
            InstructionExportFormatRequest::Pdf,
            Arc::new(AtomicBool::new(false)),
            Arc::new(AtomicU8::new(PHASE_VALIDATING)),
        )
        .err()
        .expect("stale step must be rejected");
        assert!(error.contains("古い"));
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
        assert!(
            require_warning_acknowledgement(&pending, false)
                .unwrap_err()
                .contains("確認")
        );
        require_warning_acknowledgement(&pending, true).unwrap();
    }

    #[test]
    fn source_limits_reject_before_topology_analysis_and_progress_is_closed() {
        let limits = ori_formats::InstructionDiagramLimits::default();
        validate_source_counts(limits.max_source_vertices, limits.max_source_edges)
            .expect("inclusive source limits");
        assert!(
            validate_source_counts(limits.max_source_vertices + 1, limits.max_source_edges)
                .unwrap_err()
                .contains("上限")
        );
        assert!(
            validate_source_counts(limits.max_source_vertices, limits.max_source_edges + 1)
                .unwrap_err()
                .contains("上限")
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
        assert!(instruction_export_phase(u8::MAX).is_err());

        let phase = AtomicU8::new(PHASE_VALIDATING);
        advance_generation_phase(&phase, PHASE_ANALYZING_TOPOLOGY).unwrap();
        advance_generation_phase(&phase, PHASE_BUILDING_DOCUMENT).unwrap();
        advance_generation_phase(&phase, PHASE_READY).unwrap();
        assert!(advance_generation_phase(&phase, PHASE_VALIDATING).is_err());
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
        assert!(error.contains("既に開始"));
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
            GENERATION_CANCELLED_MESSAGE
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

        assert!(
            commit_pending_export_to_path(
                &mut slot,
                &project,
                export_id,
                project.project_id,
                project.editor.revision(),
                true,
                &occupied,
            )
            .is_err()
        );
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
            ),
            PathBuf::from("guide.pdf")
        );
        assert_eq!(
            ensure_export_extension(
                PathBuf::from("guide.ZIP"),
                InstructionExportFormatRequest::SvgZip,
            ),
            PathBuf::from("guide.ZIP")
        );
        assert_eq!(
            suggested_export_file_name("  鶴:<test>/\u{0001}  ", "pdf"),
            "鶴__test___-折り図.pdf"
        );
    }
}
