use std::{
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard},
};

use ori_domain::{EdgeKind, ProjectId};
use ori_formats::{
    CreasePatternExportArtifact, CreasePatternExportFormat, export_crease_pattern_with_provenance,
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

#[cfg(not(target_os = "windows"))]
use super::project_persistence::{containing_directory, publish_unix_staged_file};
#[cfg(target_os = "windows")]
use super::rename_windows_staged_file_with_policy;
use super::save_path::{DialogSaveDestination, ExistingDestinationPolicy};
use super::{
    AppState, ProjectState, StagedFile, create_staged_file, ensure_expected_project,
    ensure_project_identity, lock_project, validate_import_active_edge_containment,
};
#[cfg(not(target_os = "windows"))]
use std::fs::File;

#[derive(Default)]
pub(super) struct CreaseExportState(Mutex<CreaseExportSlot>);

#[derive(Default)]
struct CreaseExportSlot {
    active_generation_id: Option<ProjectId>,
    pending: Option<PendingCreaseExport>,
    last_cancelled_id: Option<ProjectId>,
}

#[derive(Clone)]
struct PendingCreaseExport {
    export_id: ProjectId,
    expected_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    format: CreaseExportFormatRequest,
    format_summary: String,
    suggested_file_name: String,
    bytes: Arc<[u8]>,
    vertex_count: usize,
    edge_count: usize,
    assignment_counts: CreaseExportAssignmentCounts,
    has_cuts: bool,
    warnings: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum CreaseExportFormatRequest {
    Fold,
    Svg,
    Pdf,
    Dxf,
}

impl CreaseExportFormatRequest {
    fn exporter_format(self) -> CreasePatternExportFormat {
        match self {
            Self::Fold => CreasePatternExportFormat::Fold12,
            Self::Svg => CreasePatternExportFormat::Svg,
            Self::Pdf => CreasePatternExportFormat::Pdf17,
            Self::Dxf => CreasePatternExportFormat::Dxf2007Ascii,
        }
    }

    fn extension(self) -> &'static str {
        match self {
            Self::Fold => "fold",
            Self::Svg => "svg",
            Self::Pdf => "pdf",
            Self::Dxf => "dxf",
        }
    }

    fn media_type(self) -> &'static str {
        match self {
            Self::Fold => "application/json",
            Self::Svg => "image/svg+xml",
            Self::Pdf => "application/pdf",
            Self::Dxf => "image/vnd.dxf",
        }
    }

    fn filter_label(self) -> &'static str {
        match self {
            Self::Fold => "FOLD 1.2 crease pattern",
            Self::Svg => "SVG crease pattern",
            Self::Pdf => "PDF 1.7 full-scale crease pattern",
            Self::Dxf => "AutoCAD 2007 DXF crease pattern",
        }
    }

    fn format_label(self) -> &'static str {
        match self {
            Self::Fold => "FOLD 1.2",
            Self::Svg => "SVG",
            Self::Pdf => "PDF 1.7",
            Self::Dxf => "DXF（AutoCAD 2007）",
        }
    }

    fn format_summary(self) -> &'static str {
        match self {
            Self::Fold => "FOLD 1.2・2D creasePattern・座標単位mm",
            Self::Svg => "静的直線SVG・1 SVG unit = 1 mm",
            Self::Pdf => "実寸1:1ベクター・図面範囲＋四辺10 mm余白",
            Self::Dxf => "AC1021 text-form・UTF-8・mm・5意味レイヤー",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
struct CreaseExportAssignmentCounts {
    boundary: usize,
    mountain: usize,
    valley: usize,
    auxiliary: usize,
    cut: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct CreaseExportPreviewResponse {
    preview: CreaseExportPreviewSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct CreaseExportPreviewSnapshot {
    export_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    format: CreaseExportFormatRequest,
    format_summary: String,
    suggested_file_name: String,
    byte_count: usize,
    vertex_count: usize,
    edge_count: usize,
    assignment_counts: CreaseExportAssignmentCounts,
    has_cuts: bool,
    warnings: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(super) struct CreaseExportSaveResponse {
    canceled: bool,
}

struct CreaseExportSource {
    export_id: ProjectId,
    expected_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    format: CreaseExportFormatRequest,
    name: String,
    pattern: ori_domain::CreasePattern,
    paper: ori_domain::Paper,
    instruction_step_count: usize,
    generation_provenance: Option<ori_domain::BeginnerGenerationProvenanceV1>,
}

#[tauri::command]
pub(super) async fn preview_crease_pattern_export(
    state: State<'_, AppState>,
    export_state: State<'_, CreaseExportState>,
    expected_project_id: ProjectId,
    expected_revision: u64,
    format: CreaseExportFormatRequest,
) -> Result<CreaseExportPreviewResponse, String> {
    let export_id = ProjectId::new();
    begin_export_generation(&export_state, export_id)?;
    let source = match capture_export_source(
        &state,
        export_id,
        expected_project_id,
        expected_revision,
        format,
    ) {
        Ok(source) => source,
        Err(error) => {
            abandon_export_generation(&export_state, export_id)?;
            return Err(error);
        }
    };

    let built =
        match tauri::async_runtime::spawn_blocking(move || build_pending_export(source)).await {
            Ok(built) => built,
            Err(_) => {
                abandon_export_generation(&export_state, export_id)?;
                return Err("展開図の書き出し処理を完了できませんでした。".to_owned());
            }
        };
    let pending = match built {
        Ok(pending) => pending,
        Err(error) => {
            abandon_export_generation(&export_state, export_id)?;
            return Err(error);
        }
    };

    let mut slot = lock_crease_export(&export_state)?;
    let project = lock_project(&state)?;
    ensure_generation_is_current(&slot, export_id)?;
    if let Err(error) = ensure_expected_project(
        &project,
        pending.expected_instance_id,
        pending.expected_project_id,
        pending.expected_revision,
    ) {
        slot.active_generation_id = None;
        slot.pending = None;
        return Err(error);
    }
    let preview = preview_snapshot(&pending);
    slot.pending = Some(pending);
    Ok(CreaseExportPreviewResponse { preview })
}

#[tauri::command]
pub(super) async fn save_crease_pattern_export(
    app: AppHandle,
    state: State<'_, AppState>,
    export_state: State<'_, CreaseExportState>,
    export_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    warnings_acknowledged: bool,
) -> Result<CreaseExportSaveResponse, String> {
    let (pending, initial_directory) = {
        let slot = lock_crease_export(&export_state)?;
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
        .set_title(format!("{}展開図を書き出す", pending.format.format_label()));
    if let Some(directory) = initial_directory {
        dialog = dialog.set_directory(directory);
    }
    let Some(selected) = dialog.blocking_save_file() else {
        // A picker cancellation deliberately retains this exact immutable
        // generation so the confirmation dialog can retry without rebuilding.
        // Recheck after the native dialog closes because another IPC caller
        // may have replaced the generation while the picker was open.
        let slot = lock_crease_export(&export_state)?;
        let project = lock_project(&state)?;
        let pending = checked_pending(
            &slot,
            &project,
            export_id,
            expected_project_id,
            expected_revision,
        )?;
        require_warning_acknowledgement(pending, warnings_acknowledged)?;
        return Ok(CreaseExportSaveResponse { canceled: true });
    };
    let selected_path = selected
        .simplified()
        .into_path()
        .map_err(|_| "選択された保存先はローカルファイルではありません。".to_owned())?;
    let destination = ensure_export_extension(selected_path, pending.format)?;

    let mut slot = lock_crease_export(&export_state)?;
    let project = lock_project(&state)?;
    commit_pending_export_to_destination(
        &mut slot,
        &project,
        export_id,
        expected_project_id,
        expected_revision,
        warnings_acknowledged,
        &destination,
    )?;
    Ok(CreaseExportSaveResponse { canceled: false })
}

#[tauri::command]
pub(super) fn cancel_crease_pattern_export(
    state: State<'_, CreaseExportState>,
    export_id: ProjectId,
) -> Result<(), String> {
    cancel_pending_export(&state, export_id)
}

fn lock_crease_export(
    state: &CreaseExportState,
) -> Result<MutexGuard<'_, CreaseExportSlot>, String> {
    state
        .0
        .lock()
        .map_err(|_| "展開図書き出し状態を利用できません。".to_owned())
}

fn begin_export_generation(state: &CreaseExportState, export_id: ProjectId) -> Result<(), String> {
    let mut slot = lock_crease_export(state)?;
    slot.pending = None;
    slot.active_generation_id = Some(export_id);
    Ok(())
}

fn abandon_export_generation(
    state: &CreaseExportState,
    export_id: ProjectId,
) -> Result<(), String> {
    let mut slot = lock_crease_export(state)?;
    if slot.active_generation_id == Some(export_id) {
        slot.active_generation_id = None;
        slot.pending = None;
    }
    Ok(())
}

fn ensure_generation_is_current(
    slot: &CreaseExportSlot,
    export_id: ProjectId,
) -> Result<(), String> {
    if slot.active_generation_id == Some(export_id) {
        Ok(())
    } else {
        Err("この書き出し処理は新しい処理に置き換えられました。".to_owned())
    }
}

fn capture_export_source(
    state: &AppState,
    export_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    format: CreaseExportFormatRequest,
) -> Result<CreaseExportSource, String> {
    let project = lock_project(state)?;
    ensure_project_identity(&project, expected_project_id)?;
    if project.editor.revision() != expected_revision {
        return Err(format!(
            "編集内容が変更されています（要求revision {expected_revision}、現在revision {}）。",
            project.editor.revision()
        ));
    }
    validate_project_for_export(&project)?;
    Ok(CreaseExportSource {
        export_id,
        expected_instance_id: project.instance_id,
        expected_project_id: project.project_id,
        expected_revision,
        format,
        name: project.name.clone(),
        pattern: project.editor.pattern().clone(),
        paper: project.editor.paper().clone(),
        instruction_step_count: project.editor.instruction_timeline().steps.len(),
        generation_provenance: project
            .editor
            .beginner_design_profile()
            .generation_provenance
            .clone(),
    })
}

fn validate_project_for_export(project: &ProjectState) -> Result<(), String> {
    // The bounded exporter performs the authoritative pattern and paper
    // validation after applying its candidate-work limits. Only the
    // editor-specific active-edge containment rule is checked here.
    validate_import_active_edge_containment(project, "export")
        .map_err(|_| "用紙外に有効な折線があるため書き出せません。".to_owned())
}

fn build_pending_export(source: CreaseExportSource) -> Result<PendingCreaseExport, String> {
    let artifact = export_crease_pattern_with_provenance(
        source.format.exporter_format(),
        &source.name,
        &source.pattern,
        &source.paper,
        source.generation_provenance.as_ref(),
    )
    .map_err(|error| format!("展開図データを生成できませんでした: {error}"))?;
    validate_artifact_contract(source.format, &source.pattern, &artifact)?;
    let assignment_counts = assignment_counts(&source.pattern);
    let cutting_permission_without_cut = source.paper.cutting_allowed && !artifact.has_cuts;
    let warnings = export_warnings(
        source.format,
        source.instruction_step_count,
        cutting_permission_without_cut,
    );
    Ok(PendingCreaseExport {
        export_id: source.export_id,
        expected_instance_id: source.expected_instance_id,
        expected_project_id: source.expected_project_id,
        expected_revision: source.expected_revision,
        format: source.format,
        format_summary: source.format.format_summary().to_owned(),
        suggested_file_name: suggested_export_file_name(&source.name, source.format.extension()),
        bytes: Arc::from(artifact.bytes),
        vertex_count: artifact.vertex_count,
        edge_count: artifact.edge_count,
        assignment_counts,
        has_cuts: artifact.has_cuts,
        warnings,
    })
}

fn validate_artifact_contract(
    requested: CreaseExportFormatRequest,
    pattern: &ori_domain::CreasePattern,
    artifact: &CreasePatternExportArtifact,
) -> Result<(), String> {
    if artifact.format != requested.exporter_format()
        || artifact.file_extension != requested.extension()
        || artifact.media_type != requested.media_type()
    {
        return Err("生成された展開図の形式が要求と一致しません。".to_owned());
    }
    if artifact.vertex_count != pattern.vertices.len() || artifact.edge_count != pattern.edges.len()
    {
        return Err("生成された展開図の件数が元データと一致しません。".to_owned());
    }
    if artifact.has_cuts != pattern.edges.iter().any(|edge| edge.kind == EdgeKind::Cut) {
        return Err("生成された展開図の切断線情報が元データと一致しません。".to_owned());
    }
    if artifact.bytes.is_empty() {
        return Err("生成された展開図データが空です。".to_owned());
    }
    Ok(())
}

fn assignment_counts(pattern: &ori_domain::CreasePattern) -> CreaseExportAssignmentCounts {
    let mut counts = CreaseExportAssignmentCounts {
        boundary: 0,
        mountain: 0,
        valley: 0,
        auxiliary: 0,
        cut: 0,
    };
    for edge in &pattern.edges {
        match edge.kind {
            EdgeKind::Boundary => counts.boundary += 1,
            EdgeKind::Mountain => counts.mountain += 1,
            EdgeKind::Valley => counts.valley += 1,
            EdgeKind::Auxiliary => counts.auxiliary += 1,
            EdgeKind::Cut => counts.cut += 1,
        }
    }
    counts
}

fn export_warnings(
    format: CreaseExportFormatRequest,
    instruction_step_count: usize,
    cutting_permission_without_cut: bool,
) -> Vec<String> {
    let label = format.format_label();
    let mut warnings = vec![
        format!("紙の表裏色・厚み・テクスチャは{label}出力に含まれません。"),
        format!("ORIGAMI2の頂点・辺ID、編集履歴、選択状態は{label}出力に含まれません。"),
        format!("現在の3D表示姿勢とカメラ状態は{label}出力に含まれません。"),
    ];
    match format {
        CreaseExportFormatRequest::Pdf => {
            warnings.push(
                "PDFは印刷用の視覚出力で、構造化された線種や座標原点を保持せず、ORIGAMI2へ再取込できません。"
                    .to_owned(),
            );
            warnings.push(
                "実寸で印刷するには、PDF viewerの印刷倍率を100%にし「用紙に合わせる」を無効にしてください。"
                    .to_owned(),
            );
        }
        CreaseExportFormatRequest::Dxf => {
            warnings.push(
                "折り線の意味はORIGAMI2独自のDXFレイヤー名で表し、CAD固有の標準意味ではありません。"
                    .to_owned(),
            );
            warnings.push(
                "作品名はDXFコメントに格納されますが、CADで再保存すると失われる場合があります。"
                    .to_owned(),
            );
        }
        CreaseExportFormatRequest::Fold | CreaseExportFormatRequest::Svg => {}
    }
    if instruction_step_count > 0 {
        warnings.push(format!(
            "{instruction_step_count}件の折り手順は{label}出力に含まれません。"
        ));
    }
    if cutting_permission_without_cut {
        warnings.push(format!(
            "切断線を作成できるプロジェクト設定は、切断線がないため{label}出力に含まれません。"
        ));
    }
    warnings
}

fn preview_snapshot(pending: &PendingCreaseExport) -> CreaseExportPreviewSnapshot {
    CreaseExportPreviewSnapshot {
        export_id: pending.export_id,
        expected_project_id: pending.expected_project_id,
        expected_revision: pending.expected_revision,
        format: pending.format,
        format_summary: pending.format_summary.clone(),
        suggested_file_name: pending.suggested_file_name.clone(),
        byte_count: pending.bytes.len(),
        vertex_count: pending.vertex_count,
        edge_count: pending.edge_count,
        assignment_counts: pending.assignment_counts,
        has_cuts: pending.has_cuts,
        warnings: pending.warnings.clone(),
    }
}

fn checked_pending<'a>(
    slot: &'a CreaseExportSlot,
    project: &ProjectState,
    export_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
) -> Result<&'a PendingCreaseExport, String> {
    let pending = slot
        .pending
        .as_ref()
        .ok_or_else(|| "書き出しプレビューは既に破棄されています。".to_owned())?;
    if pending.export_id != export_id {
        return Err("書き出しプレビューは新しいプレビューに置き換えられました。".to_owned());
    }
    if pending.expected_project_id != expected_project_id
        || pending.expected_revision != expected_revision
    {
        return Err("書き出しプレビューは別の編集状態に属しています。".to_owned());
    }
    ensure_generation_is_current(slot, export_id)?;
    ensure_expected_project(
        project,
        pending.expected_instance_id,
        pending.expected_project_id,
        pending.expected_revision,
    )?;
    Ok(pending)
}

#[allow(clippy::too_many_arguments)]
#[cfg(test)]
fn commit_pending_export_to_path(
    slot: &mut CreaseExportSlot,
    project: &ProjectState,
    export_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    warnings_acknowledged: bool,
    path: &Path,
) -> Result<(), String> {
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
    slot: &mut CreaseExportSlot,
    project: &ProjectState,
    export_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    warnings_acknowledged: bool,
    destination: &DialogSaveDestination,
) -> Result<(), String> {
    let pending = checked_pending(
        slot,
        project,
        export_id,
        expected_project_id,
        expected_revision,
    )?;
    require_warning_acknowledgement(pending, warnings_acknowledged)?;
    persist_export_bytes_to_destination(destination, &pending.bytes)?;
    slot.pending = None;
    slot.active_generation_id = None;
    Ok(())
}

fn require_warning_acknowledgement(
    pending: &PendingCreaseExport,
    warnings_acknowledged: bool,
) -> Result<(), String> {
    if !pending.warnings.is_empty() && !warnings_acknowledged {
        Err("情報損失の確認が必要です。".to_owned())
    } else {
        Ok(())
    }
}

fn cancel_pending_export(state: &CreaseExportState, export_id: ProjectId) -> Result<(), String> {
    let mut slot = lock_crease_export(state)?;
    if slot.pending.as_ref().map(|pending| pending.export_id) == Some(export_id) {
        slot.pending = None;
        slot.active_generation_id = None;
        slot.last_cancelled_id = Some(export_id);
        return Ok(());
    }
    if slot.last_cancelled_id == Some(export_id) {
        return Ok(());
    }
    if slot.pending.is_some() {
        return Err("このプレビューは新しい書き出しプレビューに置き換えられました。".to_owned());
    }
    Err("指定された書き出しプレビューは存在しません。".to_owned())
}

fn suggested_export_file_name(project_name: &str, extension: &str) -> String {
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
        "Untitled"
    } else {
        sanitized
    };
    format!("{base}.{extension}")
}

fn ensure_export_extension(
    path: PathBuf,
    format: CreaseExportFormatRequest,
) -> Result<DialogSaveDestination, String> {
    super::save_path::normalize_dialog_save_path(path, format.extension())
}

#[cfg(test)]
pub(super) fn persist_export_bytes_atomically(path: &Path, bytes: &[u8]) -> Result<(), String> {
    persist_export_bytes_to_destination(
        &DialogSaveDestination::confirmed(path.to_path_buf()),
        bytes,
    )
}

pub(super) fn persist_export_bytes_to_destination(
    destination: &DialogSaveDestination,
    bytes: &[u8],
) -> Result<(), String> {
    let path = destination.path();
    if path.file_name().is_none() {
        return Err("選択された保存先はファイルパスではありません。".to_owned());
    }
    let mut staged = prepare_staged_export_file(path, bytes)?;
    commit_staged_export_file(&mut staged, path, destination.existing_destination_policy())
}

fn prepare_staged_export_file(path: &Path, bytes: &[u8]) -> Result<StagedFile, String> {
    let mut staged = create_staged_file(path)
        .map_err(|_| "保存先と同じ場所に一時ファイルを作成できませんでした。".to_owned())?;
    staged
        .file_mut()
        .write_all(bytes)
        .map_err(|_| "一時ファイルへ書き込めませんでした。".to_owned())?;
    staged
        .file_mut()
        .sync_all()
        .map_err(|_| "一時ファイルを同期できませんでした。".to_owned())?;
    staged
        .file_mut()
        .seek(SeekFrom::Start(0))
        .map_err(|_| "一時ファイルを検証できませんでした。".to_owned())?;
    let mut verified = Vec::with_capacity(bytes.len());
    staged
        .file_mut()
        .read_to_end(&mut verified)
        .map_err(|_| "一時ファイルを検証できませんでした。".to_owned())?;
    if verified != bytes {
        return Err("保存直前に一時ファイルの内容が変化しました。".to_owned());
    }
    Ok(staged)
}

#[cfg(not(target_os = "windows"))]
fn commit_staged_export_file(
    staged: &mut StagedFile,
    path: &Path,
    existing_destination_policy: ExistingDestinationPolicy,
) -> Result<(), String> {
    let parent = containing_directory(path)
        .ok_or_else(|| "選択された保存先はファイルパスではありません。".to_owned())?;
    let directory =
        File::open(parent).map_err(|_| "保存先ディレクトリを同期できませんでした。".to_owned())?;
    directory
        .sync_all()
        .map_err(|_| "保存先ディレクトリを同期できませんでした。".to_owned())?;
    publish_unix_staged_file(staged, path, existing_destination_policy)
        .map_err(|_| "書き出しファイルを原子的に確定できませんでした。".to_owned())?;
    // A directory-sync failure after rename must not be reported as an
    // ordinary save failure: the visible destination has already changed and
    // callers would otherwise retry under the false promise that it had not.
    // The file itself was synced before rename; post-rename directory sync is
    // a best-effort durability barrier.
    let _ = directory.sync_all();
    Ok(())
}

#[cfg(target_os = "windows")]
fn commit_staged_export_file(
    staged: &mut StagedFile,
    path: &Path,
    existing_destination_policy: ExistingDestinationPolicy,
) -> Result<(), String> {
    rename_windows_staged_file_with_policy(staged.file(), path, existing_destination_policy)
        .map_err(|_| "書き出しファイルを原子的に確定できませんでした。".to_owned())?;
    staged.committed = true;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeSet,
        fs,
        sync::atomic::{AtomicU64, Ordering},
    };

    use ori_core::{Command, create_rectangular_sheet};
    use ori_domain::{Edge, EdgeId, Point2, Vertex, VertexId};

    use super::*;

    static NEXT_TEST_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let id = NEXT_TEST_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "origami2-crease-export-test-{}-{id}",
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

    fn pending_for(
        project: &ProjectState,
        format: CreaseExportFormatRequest,
    ) -> PendingCreaseExport {
        build_pending_export(CreaseExportSource {
            export_id: ProjectId::new(),
            expected_instance_id: project.instance_id,
            expected_project_id: project.project_id,
            expected_revision: project.editor.revision(),
            format,
            name: project.name.clone(),
            pattern: project.editor.pattern().clone(),
            paper: project.editor.paper().clone(),
            instruction_step_count: project.editor.instruction_timeline().steps.len(),
            generation_provenance: project
                .editor
                .beginner_design_profile()
                .generation_provenance
                .clone(),
        })
        .expect("build export")
    }

    #[test]
    fn all_format_previews_contain_only_bounded_metadata() {
        let project = super::super::initial_project_state();
        for format in [
            CreaseExportFormatRequest::Fold,
            CreaseExportFormatRequest::Svg,
            CreaseExportFormatRequest::Pdf,
            CreaseExportFormatRequest::Dxf,
        ] {
            let pending = pending_for(&project, format);
            let preview = preview_snapshot(&pending);
            assert_eq!(preview.expected_project_id, project.project_id);
            assert_eq!(preview.expected_revision, 0);
            assert_eq!(preview.vertex_count, 4);
            assert_eq!(preview.edge_count, 4);
            assert_eq!(preview.assignment_counts.boundary, 4);
            assert!(preview.byte_count > 0);
            assert!(!preview.format_summary.is_empty());
            assert!(!preview.warnings.is_empty());

            let json = serde_json::to_value(CreaseExportPreviewResponse { preview }).unwrap();
            assert_eq!(
                json.as_object()
                    .unwrap()
                    .keys()
                    .map(String::as_str)
                    .collect::<BTreeSet<_>>(),
                BTreeSet::from(["preview"])
            );
            let object = json["preview"].as_object().unwrap();
            assert_eq!(
                object.keys().map(String::as_str).collect::<BTreeSet<_>>(),
                BTreeSet::from([
                    "assignment_counts",
                    "byte_count",
                    "edge_count",
                    "expected_project_id",
                    "expected_revision",
                    "export_id",
                    "format",
                    "format_summary",
                    "has_cuts",
                    "suggested_file_name",
                    "vertex_count",
                    "warnings",
                ])
            );
            assert_eq!(
                object["assignment_counts"]
                    .as_object()
                    .unwrap()
                    .keys()
                    .map(String::as_str)
                    .collect::<BTreeSet<_>>(),
                BTreeSet::from(["auxiliary", "boundary", "cut", "mountain", "valley"])
            );
        }
    }

    #[test]
    fn desktop_export_preserves_typed_generation_provenance() {
        let project = super::super::initial_project_state();
        let provenance = ori_domain::BeginnerGenerationProvenanceV1 {
            fold_path_certificate_sha256: None,
            schema_version: 1,
            topology_authority_sha256: [0x5a; 32],
            confidence_score: 73,
            confidence_reasons: vec!["desktop_roundtrip".to_owned()],
            explicit_override: true,
            source_asset_fingerprint: "asset:desktop-test".to_owned(),
            semantic_landmark_provenance: None,
            generic_tree: None,
        };
        for format in [
            CreaseExportFormatRequest::Fold,
            CreaseExportFormatRequest::Svg,
            CreaseExportFormatRequest::Pdf,
            CreaseExportFormatRequest::Dxf,
        ] {
            let pending = build_pending_export(CreaseExportSource {
                export_id: ProjectId::new(),
                expected_instance_id: project.instance_id,
                expected_project_id: project.project_id,
                expected_revision: project.editor.revision(),
                format,
                name: project.name.clone(),
                pattern: project.editor.pattern().clone(),
                paper: project.editor.paper().clone(),
                instruction_step_count: 0,
                generation_provenance: Some(provenance.clone()),
            })
            .expect("build desktop export with provenance");
            assert_eq!(
                ori_formats::read_crease_pattern_generation_provenance(
                    format.exporter_format(),
                    &pending.bytes,
                )
                .unwrap(),
                Some(provenance.clone())
            );
        }
    }

    #[test]
    fn assignment_counts_and_cut_flag_match_the_source() {
        let sheet = create_rectangular_sheet(100.0, 80.0, true).unwrap();
        let (mut pattern, paper) = sheet.into_parts();
        let center = VertexId::new();
        pattern.vertices.push(Vertex {
            id: center,
            position: Point2::new(50.0, 40.0),
        });
        pattern.edges.extend([
            Edge {
                id: EdgeId::new(),
                start: center,
                end: paper.boundary_vertices[0],
                kind: EdgeKind::Mountain,
            },
            Edge {
                id: EdgeId::new(),
                start: center,
                end: paper.boundary_vertices[1],
                kind: EdgeKind::Cut,
            },
        ]);
        let project = ProjectState::new_with_paper(pattern, paper);
        let pending = pending_for(&project, CreaseExportFormatRequest::Fold);

        assert_eq!(pending.assignment_counts.boundary, 4);
        assert_eq!(pending.assignment_counts.mountain, 1);
        assert_eq!(pending.assignment_counts.cut, 1);
        assert!(pending.has_cuts);
    }

    #[test]
    fn warning_acknowledgement_is_enforced_natively() {
        let project = super::super::initial_project_state();
        let pending = pending_for(&project, CreaseExportFormatRequest::Svg);
        assert_eq!(
            require_warning_acknowledgement(&pending, false).unwrap_err(),
            "情報損失の確認が必要です。"
        );
        require_warning_acknowledgement(&pending, true).unwrap();
    }

    #[test]
    fn a_replacement_token_cannot_cancel_the_newer_preview() {
        let project = super::super::initial_project_state();
        let state = CreaseExportState::default();
        let first = pending_for(&project, CreaseExportFormatRequest::Fold);
        let first_id = first.export_id;
        {
            let mut slot = lock_crease_export(&state).unwrap();
            slot.active_generation_id = Some(first_id);
            slot.pending = Some(first);
        }
        let second = pending_for(&project, CreaseExportFormatRequest::Svg);
        let second_id = second.export_id;
        begin_export_generation(&state, second_id).unwrap();
        {
            let mut slot = lock_crease_export(&state).unwrap();
            slot.pending = Some(second);
        }

        assert!(cancel_pending_export(&state, first_id).is_err());
        assert_eq!(
            lock_crease_export(&state)
                .unwrap()
                .pending
                .as_ref()
                .map(|pending| pending.export_id),
            Some(second_id)
        );
        cancel_pending_export(&state, second_id).unwrap();
        cancel_pending_export(&state, second_id).unwrap();
    }

    #[test]
    fn stale_project_state_is_rejected_without_consuming_the_stage() {
        let mut project = super::super::initial_project_state();
        let pending = pending_for(&project, CreaseExportFormatRequest::Fold);
        let export_id = pending.export_id;
        let expected_project_id = project.project_id;
        let state = CreaseExportState::default();
        {
            let mut slot = lock_crease_export(&state).unwrap();
            slot.active_generation_id = Some(export_id);
            slot.pending = Some(pending);
        }
        project
            .editor
            .execute(
                0,
                Command::AddVertex {
                    id: VertexId::new(),
                    position: Point2::new(25.0, 25.0),
                },
            )
            .unwrap();

        let directory = TestDirectory::new();
        let path = directory.path().join("stale.fold");
        let mut slot = lock_crease_export(&state).unwrap();
        assert!(
            commit_pending_export_to_path(
                &mut slot,
                &project,
                export_id,
                expected_project_id,
                0,
                true,
                &path,
            )
            .is_err()
        );
        assert!(slot.pending.is_some());
        assert!(!path.exists());
    }

    #[test]
    fn io_failure_retains_the_stage_and_does_not_touch_the_project() {
        let project = super::super::initial_project_state();
        let before = project.document();
        let pending = pending_for(&project, CreaseExportFormatRequest::Fold);
        let export_id = pending.export_id;
        let mut slot = CreaseExportSlot {
            active_generation_id: Some(export_id),
            pending: Some(pending),
            last_cancelled_id: None,
        };
        let directory = TestDirectory::new();
        let path = directory.path().join("missing").join("sample.fold");

        let error = commit_pending_export_to_path(
            &mut slot,
            &project,
            export_id,
            project.project_id,
            project.editor.revision(),
            true,
            &path,
        )
        .expect_err("a missing confirmed destination directory must fail safely");

        assert_eq!(
            error,
            "保存先と同じ場所に一時ファイルを作成できませんでした。"
        );
        assert!(!error.contains("sample.fold"));
        assert!(!error.contains(&directory.path().display().to_string()));
        assert!(slot.pending.is_some());
        assert_eq!(slot.active_generation_id, Some(export_id));
        assert_eq!(slot.last_cancelled_id, None);
        assert!(!path.exists());
        assert_eq!(project.document(), before);
        assert!(!project.is_dirty());
    }

    #[test]
    fn atomic_export_persists_exact_bytes_for_every_format_without_touching_the_project() {
        let project = super::super::initial_project_state();
        let before = project.document();
        let directory = TestDirectory::new();
        for format in [
            CreaseExportFormatRequest::Fold,
            CreaseExportFormatRequest::Svg,
            CreaseExportFormatRequest::Pdf,
            CreaseExportFormatRequest::Dxf,
        ] {
            let pending = pending_for(&project, format);
            let export_id = pending.export_id;
            let expected_bytes = Arc::clone(&pending.bytes);
            let mut slot = CreaseExportSlot {
                active_generation_id: Some(export_id),
                pending: Some(pending),
                last_cancelled_id: None,
            };
            let path = directory
                .path()
                .join(format!("sample.{}", format.extension()));
            fs::write(&path, b"OS-confirmed previous export").unwrap();
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

            assert_eq!(fs::read(&path).unwrap(), expected_bytes.as_ref());
            assert!(slot.pending.is_none());
            assert_eq!(slot.active_generation_id, None);
            assert_eq!(slot.last_cancelled_id, None);
            assert_eq!(project.document(), before);
            assert!(!project.is_dirty());

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
            assert_eq!(fs::read(path).unwrap(), expected_bytes.as_ref());
        }
    }

    #[test]
    fn a_saved_token_is_rejected_by_cancel() {
        let project = super::super::initial_project_state();
        let pending = pending_for(&project, CreaseExportFormatRequest::Fold);
        let export_id = pending.export_id;
        let state = CreaseExportState::default();
        {
            let mut slot = lock_crease_export(&state).unwrap();
            slot.active_generation_id = Some(export_id);
            slot.pending = Some(pending);
        }
        let directory = TestDirectory::new();
        let path = directory.path().join("saved.fold");
        {
            let mut slot = lock_crease_export(&state).unwrap();
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
        }

        assert!(cancel_pending_export(&state, export_id).is_err());
        assert!(path.exists());
    }

    #[test]
    fn extension_and_hostile_project_name_are_normalized() {
        assert_eq!(
            suggested_export_file_name("  鶴:<test>/\u{0001}  ", "fold"),
            "鶴__test___.fold"
        );
        assert_eq!(suggested_export_file_name("...  ", "svg"), "Untitled.svg");
        assert_eq!(
            ensure_export_extension(
                PathBuf::from("sample.json"),
                CreaseExportFormatRequest::Fold
            )
            .unwrap(),
            PathBuf::from("sample.fold")
        );
        assert_eq!(
            ensure_export_extension(PathBuf::from("sample.SVG"), CreaseExportFormatRequest::Svg)
                .unwrap(),
            PathBuf::from("sample.SVG")
        );
        assert_eq!(
            ensure_export_extension(PathBuf::from("sample.txt"), CreaseExportFormatRequest::Pdf)
                .unwrap(),
            PathBuf::from("sample.pdf")
        );
        assert_eq!(
            ensure_export_extension(PathBuf::from("sample.DXF"), CreaseExportFormatRequest::Dxf)
                .unwrap(),
            PathBuf::from("sample.DXF")
        );
    }

    #[test]
    fn extension_correction_cannot_target_an_existing_unconfirmed_crease_export() {
        let directory = TestDirectory::new();
        let selected_path = directory.path().join("crease.txt");
        let corrected_path = directory.path().join("crease.pdf");
        let confirmed_path = directory.path().join("confirmed.PDF");
        fs::write(&corrected_path, b"keep existing crease export").unwrap();
        fs::write(&confirmed_path, b"OS-confirmed overwrite target").unwrap();

        let error = ensure_export_extension(selected_path, CreaseExportFormatRequest::Pdf)
            .expect_err("an unconfirmed corrected destination must not be overwritten");

        assert!(error.contains("上書き確認"));
        assert!(
            !error.contains(&corrected_path.display().to_string()),
            "save-path errors crossing IPC must not expose the local path"
        );
        assert_eq!(
            fs::read(&corrected_path).unwrap(),
            b"keep existing crease export"
        );
        assert_eq!(
            ensure_export_extension(confirmed_path.clone(), CreaseExportFormatRequest::Pdf)
                .expect("an existing selected destination was confirmed by the OS dialog"),
            confirmed_path
        );
    }

    #[test]
    fn extension_correction_race_preserves_the_crease_stage_and_new_destination() {
        let project = super::super::initial_project_state();
        let pending = pending_for(&project, CreaseExportFormatRequest::Pdf);
        let export_id = pending.export_id;
        let mut slot = CreaseExportSlot {
            active_generation_id: Some(export_id),
            pending: Some(pending),
            last_cancelled_id: None,
        };
        let directory = TestDirectory::new();
        let selected_path = directory.path().join("race-target.txt");
        let corrected_path = directory.path().join("race-target.pdf");
        let destination = ensure_export_extension(selected_path, CreaseExportFormatRequest::Pdf)
            .expect("preflight an unused corrected path");
        fs::write(&corrected_path, b"created after extension preflight").unwrap();

        let error = commit_pending_export_to_destination(
            &mut slot,
            &project,
            export_id,
            project.project_id,
            project.editor.revision(),
            true,
            &destination,
        )
        .expect_err("atomic create-new commit must reject the intervening destination");

        assert!(!error.contains("race-target"));
        assert_eq!(
            fs::read(&corrected_path).unwrap(),
            b"created after extension preflight"
        );
        assert_eq!(
            slot.pending.as_ref().map(|pending| pending.export_id),
            Some(export_id)
        );
        assert_eq!(slot.active_generation_id, Some(export_id));
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

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_extension_correction_detects_an_existing_target_with_different_path_case() {
        let directory = TestDirectory::new();
        let selected_path = directory.path().join("CaseTarget.txt");
        let existing_path = directory.path().join("casetarget.PDF");
        fs::write(&existing_path, b"keep case-insensitive target").unwrap();

        ensure_export_extension(selected_path, CreaseExportFormatRequest::Pdf)
            .expect_err("Windows path lookup must reject a differently-cased existing target");

        assert_eq!(
            fs::read(existing_path).unwrap(),
            b"keep case-insensitive target"
        );
    }

    #[test]
    fn format_contract_and_specific_warnings_cover_all_outputs() {
        let expected = [
            (
                CreaseExportFormatRequest::Fold,
                CreasePatternExportFormat::Fold12,
                "fold",
                "application/json",
            ),
            (
                CreaseExportFormatRequest::Svg,
                CreasePatternExportFormat::Svg,
                "svg",
                "image/svg+xml",
            ),
            (
                CreaseExportFormatRequest::Pdf,
                CreasePatternExportFormat::Pdf17,
                "pdf",
                "application/pdf",
            ),
            (
                CreaseExportFormatRequest::Dxf,
                CreasePatternExportFormat::Dxf2007Ascii,
                "dxf",
                "image/vnd.dxf",
            ),
        ];
        for (request, exporter, extension, media_type) in expected {
            assert_eq!(request.exporter_format(), exporter);
            assert_eq!(request.extension(), extension);
            assert_eq!(request.media_type(), media_type);
            assert!(!request.filter_label().is_empty());
            assert!(!request.format_label().is_empty());
            assert!(!request.format_summary().is_empty());
        }

        let pdf = export_warnings(CreaseExportFormatRequest::Pdf, 0, false);
        assert!(
            pdf.iter()
                .any(|warning| warning.contains("再取込できません"))
        );
        assert!(pdf.iter().any(|warning| warning.contains("印刷倍率を100%")));
        let dxf = export_warnings(CreaseExportFormatRequest::Dxf, 0, false);
        assert!(dxf.iter().any(|warning| warning.contains("DXFレイヤー名")));
        assert!(
            dxf.iter()
                .any(|warning| warning.contains("再保存すると失われる"))
        );
    }

    #[test]
    fn format_wire_values_are_closed_and_canonical() {
        for (format, wire) in [
            (CreaseExportFormatRequest::Fold, "fold"),
            (CreaseExportFormatRequest::Svg, "svg"),
            (CreaseExportFormatRequest::Pdf, "pdf"),
            (CreaseExportFormatRequest::Dxf, "dxf"),
        ] {
            assert_eq!(
                serde_json::to_value(format).unwrap(),
                serde_json::json!(wire)
            );
            assert_eq!(
                serde_json::from_value::<CreaseExportFormatRequest>(serde_json::json!(wire))
                    .unwrap(),
                format
            );
        }
        for invalid in [
            serde_json::json!(""),
            serde_json::json!("PDF"),
            serde_json::json!("obj"),
            serde_json::Value::Null,
            serde_json::json!({ "value": "pdf" }),
        ] {
            assert!(
                serde_json::from_value::<CreaseExportFormatRequest>(invalid).is_err(),
                "unknown wire value must fail closed"
            );
        }
    }

    #[test]
    fn invalid_pattern_is_refused_before_export() {
        let project = ProjectState::new(ori_domain::CreasePattern {
            vertices: vec![Vertex {
                id: VertexId::new(),
                position: Point2::new(f64::NAN, 0.0),
            }],
            edges: vec![],
        });
        validate_project_for_export(&project).unwrap();
        assert!(
            build_pending_export(CreaseExportSource {
                export_id: ProjectId::new(),
                expected_instance_id: project.instance_id,
                expected_project_id: project.project_id,
                expected_revision: project.editor.revision(),
                format: CreaseExportFormatRequest::Fold,
                name: project.name.clone(),
                pattern: project.editor.pattern().clone(),
                paper: project.editor.paper().clone(),
                instruction_step_count: 0,
                generation_provenance: None,
            })
            .is_err()
        );
    }
}
