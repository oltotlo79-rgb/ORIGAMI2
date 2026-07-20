//! Native-only staging for bounded FOLD 1.2 3D frame previews.

use std::{
    fs,
    path::Path,
    sync::{Mutex, MutexGuard},
};

use base64::Engine;
use ori_core::Command;
use ori_domain::{
    EdgeId, EdgeKind, FaceId, InstructionHingeAngle, InstructionPose, InstructionPoseModel,
    InstructionStep, InstructionStepId, ProjectId, VertexId,
};
use ori_formats::{
    Fold3dAppliedPoseProposalV1, Fold3dFramesPreviewV1, FoldImportLimits,
    read_fold_3d_frames_preview_v1,
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

use super::{
    AppState, ProjectSnapshot, ProjectState,
    applied_pose::{
        ApplyCurrentNativePoseResponse, NativePoseHingeAngleRequest, NativePoseRequest,
    },
    execute_command, lock_project,
};

const MAX_BYTES: u64 = 16 * 1024 * 1024;
const STALE: &str = "the FOLD 3D frame preview is stale";

#[derive(Default)]
pub(super) struct Fold3dFramesImportState(Mutex<Slot>);

#[derive(Default)]
struct Slot {
    pending: Option<Pending>,
    last_cancelled: Option<ProjectId>,
}

struct Pending {
    token: ProjectId,
    instance_id: ProjectId,
    project_id: ProjectId,
    revision: u64,
    preview: Fold3dFramesPreviewV1,
    prepared: Option<PreparedPose>,
    prepared_timeline: Option<PreparedTimeline>,
}

struct PreparedPose {
    frame_index: usize,
    source_fingerprint: String,
    hinges: Vec<(EdgeId, f64)>,
    fixed_face: FaceId,
}

struct PreparedTimeline {
    source_fingerprint: String,
    fixed_face: FaceId,
    frames: Vec<Vec<(EdgeId, f64)>>,
    duration_ms: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Fold3dFramesPickerResponse {
    canceled: bool,
    preview: Option<Fold3dFramesMetadata>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Fold3dFramesMetadata {
    token: ProjectId,
    project_instance_id: ProjectId,
    project_id: ProjectId,
    revision: u64,
    frame_count: usize,
    frames: Vec<Fold3dFrameMetadata>,
    authorizes_project_import: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Fold3dFrameMetadata {
    index: usize,
    parent: Option<usize>,
    inherits: bool,
    vertex_count: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct SelectFold3dFrameRequest {
    token: ProjectId,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    frame_index: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Fold3dTimelineRequest {
    token: ProjectId,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    duration_ms: u32,
    confirmed: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PrepareFold3dTimelineResponse {
    token: ProjectId,
    frame_count: usize,
    hinge_count: usize,
    duration_ms: u32,
    source_fingerprint: String,
    geometry_unchanged: bool,
    requires_explicit_confirmation: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SelectFold3dFrameResponse {
    token: ProjectId,
    frame_index: usize,
    vertex_count: usize,
    source_sha256_hex: String,
    preview_image_data_url: String,
    preview_width: u32,
    preview_height: u32,
    render_coordinates_exposed: bool,
    authorizes_project_import: bool,
    authorizes_applied_pose: bool,
    authorizes_instruction_timeline: bool,
}

#[tauri::command]
pub(super) async fn preview_fold_3d_frames(
    app: AppHandle,
    app_state: State<'_, AppState>,
    state: State<'_, Fold3dFramesImportState>,
) -> Result<Fold3dFramesPickerResponse, String> {
    let (instance_id, project_id, revision) = {
        let project = lock_project(&app_state)?;
        (
            project.instance_id,
            project.project_id,
            project.editor.revision(),
        )
    };
    {
        let mut slot = lock(&state)?;
        slot.pending = None;
    }
    let Some(file) = app
        .dialog()
        .file()
        .add_filter("FOLD 1.2 3D frames", &["fold"])
        .set_title("FOLD 3D frames / FOLD 3Dフレーム")
        .blocking_pick_file()
    else {
        return Ok(Fold3dFramesPickerResponse {
            canceled: true,
            preview: None,
        });
    };
    let path = file
        .simplified()
        .into_path()
        .map_err(|_| "selected FOLD file is not local")?;
    let preview = tauri::async_runtime::spawn_blocking(move || load(&path))
        .await
        .map_err(|_| "FOLD 3D frame analysis failed".to_owned())??;
    {
        let project = lock_project(&app_state)?;
        if project.instance_id != instance_id
            || project.project_id != project_id
            || project.editor.revision() != revision
        {
            return Err(STALE.to_owned());
        }
    }
    let token = ProjectId::new();
    let metadata = metadata(token, instance_id, project_id, revision, &preview);
    lock(&state)?.pending = Some(Pending {
        token,
        instance_id,
        project_id,
        revision,
        preview,
        prepared: None,
        prepared_timeline: None,
    });
    Ok(Fold3dFramesPickerResponse {
        canceled: false,
        preview: Some(metadata),
    })
}

#[tauri::command]
pub(super) fn prepare_fold_3d_instruction_timeline(
    app_state: State<'_, AppState>,
    state: State<'_, Fold3dFramesImportState>,
    request: Fold3dTimelineRequest,
) -> Result<PrepareFold3dTimelineResponse, String> {
    if request.confirmed || !(100..=600_000).contains(&request.duration_ms) {
        return Err("FOLD timeline request is invalid".to_owned());
    }
    let mut slot = lock(&state)?;
    let pending = slot.pending.as_mut().ok_or_else(|| STALE.to_owned())?;
    let project = lock_project(&app_state)?;
    ensure_pending_current(
        pending,
        &project,
        request.token,
        request.expected_project_instance_id,
        request.expected_project_id,
        request.expected_revision,
    )?;
    let frames = pending.preview.frames();
    if !is_authenticated_linear_chain(&pending.preview) {
        return Err("FOLD frames are not one authenticated linear chain".to_owned());
    }
    let mut mapped = Vec::with_capacity(frames.len());
    let mut common: Option<(String, FaceId, Vec<EdgeId>)> = None;
    for index in 0..frames.len() {
        let proposal = pending
            .preview
            .prepare_applied_pose_proposal(index)
            .map_err(|_| "FOLD frame does not contain a complete rigid pose".to_owned())?;
        let (fingerprint, fixed_face, hinges) = map_proposal(&project, &proposal)?;
        let edge_ids = hinges.iter().map(|(edge, _)| *edge).collect::<Vec<_>>();
        if let Some((expected_fingerprint, expected_face, expected_edges)) = &common {
            if *expected_fingerprint != fingerprint
                || *expected_face != fixed_face
                || *expected_edges != edge_ids
            {
                return Err("FOLD frame poses are not model-compatible".to_owned());
            }
        } else {
            common = Some((fingerprint, fixed_face, edge_ids));
        }
        mapped.push(hinges);
    }
    let (source_fingerprint, fixed_face, hinge_ids) = common.ok_or_else(|| STALE.to_owned())?;
    pending.prepared_timeline = Some(PreparedTimeline {
        source_fingerprint: source_fingerprint.clone(),
        fixed_face,
        frames: mapped,
        duration_ms: request.duration_ms,
    });
    Ok(PrepareFold3dTimelineResponse {
        token: pending.token,
        frame_count: pending.prepared_timeline.as_ref().unwrap().frames.len(),
        hinge_count: hinge_ids.len(),
        duration_ms: request.duration_ms,
        source_fingerprint,
        geometry_unchanged: true,
        requires_explicit_confirmation: true,
    })
}

#[tauri::command]
pub(super) fn apply_fold_3d_instruction_timeline(
    app_state: State<'_, AppState>,
    state: State<'_, Fold3dFramesImportState>,
    request: Fold3dTimelineRequest,
) -> Result<ProjectSnapshot, String> {
    if !request.confirmed {
        return Err("explicit FOLD timeline confirmation is required".to_owned());
    }
    let mut slot = lock(&state)?;
    let pending = slot.pending.as_ref().ok_or_else(|| STALE.to_owned())?;
    let project = &mut *lock_project(&app_state)?;
    ensure_pending_current(
        pending,
        project,
        request.token,
        request.expected_project_instance_id,
        request.expected_project_id,
        request.expected_revision,
    )?;
    let prepared = pending
        .prepared_timeline
        .as_ref()
        .filter(|value| value.duration_ms == request.duration_ms)
        .ok_or_else(|| STALE.to_owned())?;
    if project.editor.fold_model_fingerprint_v1() != prepared.source_fingerprint {
        return Err(STALE.to_owned());
    }
    let steps = prepared
        .frames
        .iter()
        .enumerate()
        .map(|(index, hinges)| InstructionStep {
            id: InstructionStepId::new(),
            title: format!("FOLD frame {}", index + 1),
            description: String::new(),
            caution: String::new(),
            duration_ms: prepared.duration_ms,
            visual: Default::default(),
            pose: InstructionPose {
                model: InstructionPoseModel::AbsoluteHingeAnglesV1,
                source_model_fingerprint: prepared.source_fingerprint.clone(),
                fixed_face: Some(prepared.fixed_face),
                hinge_angles: hinges
                    .iter()
                    .map(|(edge, angle_degrees)| InstructionHingeAngle {
                        edge: *edge,
                        angle_degrees: *angle_degrees,
                    })
                    .collect(),
            },
        })
        .collect();
    let snapshot = execute_command(
        project,
        pending.instance_id,
        pending.project_id,
        pending.revision,
        Command::AppendInstructionSteps { steps },
    )?;
    slot.pending = None;
    Ok(snapshot)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PrepareFold3dPoseResponse {
    token: ProjectId,
    frame_index: usize,
    hinge_count: usize,
    source_fingerprint: String,
    authorizes_project_geometry_mutation: bool,
    requires_explicit_apply: bool,
}

#[tauri::command]
pub(super) fn prepare_fold_3d_applied_pose(
    app_state: State<'_, AppState>,
    state: State<'_, Fold3dFramesImportState>,
    request: SelectFold3dFrameRequest,
) -> Result<PrepareFold3dPoseResponse, String> {
    let mut slot = lock(&state)?;
    let pending = slot.pending.as_mut().ok_or_else(|| STALE.to_owned())?;
    let project = lock_project(&app_state)?;
    if pending.token != request.token
        || pending.instance_id != request.expected_project_instance_id
        || pending.project_id != request.expected_project_id
        || pending.revision != request.expected_revision
        || project.instance_id != pending.instance_id
        || project.project_id != pending.project_id
        || project.editor.revision() != pending.revision
    {
        return Err(STALE.to_owned());
    }
    let proposal = pending
        .preview
        .prepare_applied_pose_proposal(request.frame_index)
        .map_err(|_| "FOLD frame is not a rigid tree pose".to_owned())?;
    let pattern = project.editor.pattern();
    if proposal.rest_vertices().len() != pattern.vertices.len()
        || proposal.edges().len() != pattern.edges.len()
    {
        return Err("FOLD topology does not match the current project".to_owned());
    }
    let mut vertex_map = Vec::<VertexId>::new();
    for point in proposal.rest_vertices() {
        let matches = pattern
            .vertices
            .iter()
            .filter(|vertex| {
                vertex.position.x.to_bits() == point[0].to_bits()
                    && vertex.position.y.to_bits() == point[1].to_bits()
                    && point[2].to_bits() == 0.0_f64.to_bits()
            })
            .collect::<Vec<_>>();
        if matches.len() != 1 {
            return Err("FOLD vertex identity is ambiguous".to_owned());
        }
        vertex_map.push(matches[0].id);
    }
    let mut hinges = Vec::new();
    for &(edge_index, angle) in proposal.hinge_angles_degrees() {
        let endpoints = proposal.edges()[edge_index];
        let assignment = proposal.assignments()[edge_index].as_str();
        let candidates = pattern
            .edges
            .iter()
            .filter(|edge| {
                ((edge.start == vertex_map[endpoints[0]] && edge.end == vertex_map[endpoints[1]])
                    || (edge.end == vertex_map[endpoints[0]]
                        && edge.start == vertex_map[endpoints[1]]))
                    && matches!(
                        (assignment, edge.kind),
                        ("M", EdgeKind::Mountain) | ("V", EdgeKind::Valley)
                    )
            })
            .collect::<Vec<_>>();
        if candidates.len() != 1 {
            return Err("FOLD hinge identity does not match".to_owned());
        }
        hinges.push((candidates[0].id, angle.abs()));
    }
    let source_fingerprint = project.editor.fold_model_fingerprint_v1();
    let fixed_face = project
        .editor
        .topology_analysis_input(project.project_id)
        .analyze()
        .simulation_snapshot()
        .and_then(|snapshot| snapshot.faces.first())
        .map(|face| face.id)
        .ok_or_else(|| "current project topology is not simulation-ready".to_owned())?;
    pending.prepared = Some(PreparedPose {
        frame_index: request.frame_index,
        source_fingerprint: source_fingerprint.clone(),
        hinges,
        fixed_face,
    });
    Ok(PrepareFold3dPoseResponse {
        token: pending.token,
        frame_index: request.frame_index,
        hinge_count: pending.prepared.as_ref().unwrap().hinges.len(),
        source_fingerprint,
        authorizes_project_geometry_mutation: false,
        requires_explicit_apply: true,
    })
}

#[tauri::command]
pub(super) async fn apply_fold_3d_applied_pose(
    app_state: State<'_, AppState>,
    state: State<'_, Fold3dFramesImportState>,
    request: SelectFold3dFrameRequest,
) -> Result<ApplyCurrentNativePoseResponse, String> {
    let native_request = {
        let slot = lock(&state)?;
        let pending = slot.pending.as_ref().ok_or_else(|| STALE.to_owned())?;
        let prepared = pending
            .prepared
            .as_ref()
            .filter(|value| value.frame_index == request.frame_index)
            .ok_or_else(|| STALE.to_owned())?;
        let project = lock_project(&app_state)?;
        if pending.token != request.token
            || project.instance_id != pending.instance_id
            || project.project_id != pending.project_id
            || project.editor.revision() != pending.revision
            || project.editor.fold_model_fingerprint_v1() != prepared.source_fingerprint
        {
            return Err(STALE.to_owned());
        }
        NativePoseRequest {
            expected_project_instance_id: pending.instance_id,
            expected_project_id: pending.project_id,
            expected_revision: pending.revision,
            fixed_face_id: Some(prepared.fixed_face),
            complete_hinge_angles: prepared
                .hinges
                .iter()
                .map(|(edge_id, angle_degrees)| NativePoseHingeAngleRequest {
                    edge_id: *edge_id,
                    angle_degrees: *angle_degrees,
                })
                .collect(),
        }
    };
    super::applied_pose::apply_current_native_pose(&app_state, native_request)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(super) fn select_fold_3d_frame(
    app_state: State<'_, AppState>,
    state: State<'_, Fold3dFramesImportState>,
    request: SelectFold3dFrameRequest,
) -> Result<SelectFold3dFrameResponse, String> {
    let slot = lock(&state)?;
    let pending = slot.pending.as_ref().ok_or_else(|| STALE.to_owned())?;
    let project = lock_project(&app_state)?;
    if pending.token != request.token
        || pending.instance_id != request.expected_project_instance_id
        || pending.project_id != request.expected_project_id
        || pending.revision != request.expected_revision
        || project.instance_id != pending.instance_id
        || project.project_id != pending.project_id
        || project.editor.revision() != pending.revision
    {
        return Err(STALE.to_owned());
    }
    let selected = pending
        .preview
        .select_frame(request.frame_index)
        .map_err(|_| STALE.to_owned())?;
    let (png, width, height) = render_vertex_cloud_png(selected.vertices())?;
    Ok(SelectFold3dFrameResponse {
        token: pending.token,
        frame_index: selected.frame_index(),
        vertex_count: selected.vertices().len(),
        source_sha256_hex: selected
            .source_sha256()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
        preview_image_data_url: format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(png)
        ),
        preview_width: width,
        preview_height: height,
        render_coordinates_exposed: false,
        authorizes_project_import: false,
        authorizes_applied_pose: false,
        authorizes_instruction_timeline: false,
    })
}

fn ensure_pending_current(
    pending: &Pending,
    project: &ProjectState,
    token: ProjectId,
    instance_id: ProjectId,
    project_id: ProjectId,
    revision: u64,
) -> Result<(), String> {
    if pending.token != token
        || pending.instance_id != instance_id
        || pending.project_id != project_id
        || pending.revision != revision
        || project.instance_id != instance_id
        || project.project_id != project_id
        || project.editor.revision() != revision
    {
        return Err(STALE.to_owned());
    }
    Ok(())
}

fn is_authenticated_linear_chain(preview: &Fold3dFramesPreviewV1) -> bool {
    !preview.frames().is_empty()
        && preview.frames().iter().enumerate().all(|(index, frame)| {
            frame.topology_identity_authenticated()
                && frame.index() == index
                && frame.parent() == index.checked_sub(1)
        })
}

fn map_proposal(
    project: &ProjectState,
    proposal: &Fold3dAppliedPoseProposalV1,
) -> Result<(String, FaceId, Vec<(EdgeId, f64)>), String> {
    let pattern = project.editor.pattern();
    if proposal.rest_vertices().len() != pattern.vertices.len()
        || proposal.edges().len() != pattern.edges.len()
    {
        return Err("FOLD topology does not match the current project".to_owned());
    }
    let mut vertex_map = Vec::<VertexId>::new();
    for point in proposal.rest_vertices() {
        let matches = pattern
            .vertices
            .iter()
            .filter(|vertex| {
                vertex.position.x.to_bits() == point[0].to_bits()
                    && vertex.position.y.to_bits() == point[1].to_bits()
                    && point[2].to_bits() == 0.0_f64.to_bits()
            })
            .collect::<Vec<_>>();
        if matches.len() != 1 {
            return Err("FOLD vertex identity is ambiguous".to_owned());
        }
        vertex_map.push(matches[0].id);
    }
    let mut hinges = Vec::new();
    for &(edge_index, angle) in proposal.hinge_angles_degrees() {
        let endpoints = proposal.edges()[edge_index];
        let assignment = proposal.assignments()[edge_index].as_str();
        let candidates = pattern
            .edges
            .iter()
            .filter(|edge| {
                ((edge.start == vertex_map[endpoints[0]] && edge.end == vertex_map[endpoints[1]])
                    || (edge.end == vertex_map[endpoints[0]]
                        && edge.start == vertex_map[endpoints[1]]))
                    && matches!(
                        (assignment, edge.kind),
                        ("M", EdgeKind::Mountain) | ("V", EdgeKind::Valley)
                    )
            })
            .collect::<Vec<_>>();
        if candidates.len() != 1 {
            return Err("FOLD hinge identity does not match".to_owned());
        }
        hinges.push((candidates[0].id, angle.abs()));
    }
    let expected_hinges = pattern
        .edges
        .iter()
        .filter(|edge| matches!(edge.kind, EdgeKind::Mountain | EdgeKind::Valley))
        .count();
    if hinges.len() != expected_hinges {
        return Err("FOLD frame hinge vector is incomplete".to_owned());
    }
    let source_fingerprint = project.editor.fold_model_fingerprint_v1();
    let fixed_face = project
        .editor
        .topology_analysis_input(project.project_id)
        .analyze()
        .simulation_snapshot()
        .and_then(|snapshot| snapshot.faces.first())
        .map(|face| face.id)
        .ok_or_else(|| "current project topology is not simulation-ready".to_owned())?;
    Ok((source_fingerprint, fixed_face, hinges))
}

fn render_vertex_cloud_png(vertices: &[[f64; 3]]) -> Result<(Vec<u8>, u32, u32), String> {
    const WIDTH: usize = 512;
    const HEIGHT: usize = 384;
    const MARGIN: f64 = 24.0;
    if vertices.is_empty() {
        return Err("selected FOLD frame has no vertices".to_owned());
    }
    let projected = vertices
        .iter()
        .map(|point| [point[0] + point[2] * 0.5, point[1] - point[2] * 0.5])
        .collect::<Vec<_>>();
    let min_x = projected.iter().map(|p| p[0]).fold(f64::INFINITY, f64::min);
    let max_x = projected
        .iter()
        .map(|p| p[0])
        .fold(f64::NEG_INFINITY, f64::max);
    let min_y = projected.iter().map(|p| p[1]).fold(f64::INFINITY, f64::min);
    let max_y = projected
        .iter()
        .map(|p| p[1])
        .fold(f64::NEG_INFINITY, f64::max);
    let span_x = (max_x - min_x).max(1.0);
    let span_y = (max_y - min_y).max(1.0);
    let scale =
        ((WIDTH as f64 - 2.0 * MARGIN) / span_x).min((HEIGHT as f64 - 2.0 * MARGIN) / span_y);
    let mut pixels = vec![248_u8; WIDTH * HEIGHT * 4];
    for pixel in pixels.chunks_exact_mut(4) {
        pixel[3] = 255;
    }
    for point in projected {
        let x = (MARGIN + (point[0] - min_x) * scale).round() as isize;
        let y = (HEIGHT as f64 - MARGIN - (point[1] - min_y) * scale).round() as isize;
        for dy in -3..=3 {
            for dx in -3..=3 {
                if dx * dx + dy * dy > 9 {
                    continue;
                }
                let px = x + dx;
                let py = y + dy;
                if px >= 0 && py >= 0 && px < WIDTH as isize && py < HEIGHT as isize {
                    let offset = (py as usize * WIDTH + px as usize) * 4;
                    pixels[offset..offset + 4].copy_from_slice(&[34, 73, 105, 255]);
                }
            }
        }
    }
    let mut encoded = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut encoded, WIDTH as u32, HEIGHT as u32);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_compression(png::Compression::Fast);
        let mut writer = encoder.write_header().map_err(|_| "preview PNG failed")?;
        writer
            .write_image_data(&pixels)
            .map_err(|_| "preview PNG failed")?;
    }
    if encoded.len() > 512 * 1024 {
        return Err("preview PNG exceeds its size bound".to_owned());
    }
    Ok((encoded, WIDTH as u32, HEIGHT as u32))
}

#[tauri::command]
pub(super) fn cancel_fold_3d_frames(
    state: State<'_, Fold3dFramesImportState>,
    token: ProjectId,
) -> Result<(), String> {
    cancel_pending(&state, token)
}

fn cancel_pending(state: &Fold3dFramesImportState, token: ProjectId) -> Result<(), String> {
    let mut slot = lock(&state)?;
    if slot
        .pending
        .as_ref()
        .is_some_and(|pending| pending.token == token)
    {
        slot.pending = None;
        slot.last_cancelled = Some(token);
        return Ok(());
    }
    if slot.last_cancelled == Some(token) {
        return Ok(());
    }
    Err(STALE.to_owned())
}

fn load(path: &Path) -> Result<Fold3dFramesPreviewV1, String> {
    if fs::metadata(path)
        .map_err(|_| "cannot inspect FOLD file")?
        .len()
        > MAX_BYTES
    {
        return Err("FOLD file is too large".to_owned());
    }
    let bytes = fs::read(path).map_err(|_| "cannot read FOLD file")?;
    read_fold_3d_frames_preview_v1(&bytes, FoldImportLimits::default())
        .map_err(|_| "FOLD 3D frames are invalid".to_owned())
}

fn metadata(
    token: ProjectId,
    instance_id: ProjectId,
    project_id: ProjectId,
    revision: u64,
    preview: &Fold3dFramesPreviewV1,
) -> Fold3dFramesMetadata {
    Fold3dFramesMetadata {
        token,
        project_instance_id: instance_id,
        project_id,
        revision,
        frame_count: preview.frames().len(),
        frames: preview
            .frames()
            .iter()
            .map(|frame| Fold3dFrameMetadata {
                index: frame.index(),
                parent: frame.parent(),
                inherits: frame.inherits(),
                vertex_count: frame.vertex_count(),
            })
            .collect(),
        authorizes_project_import: false,
    }
}

fn lock(state: &Fold3dFramesImportState) -> Result<MutexGuard<'_, Slot>, String> {
    state
        .0
        .lock()
        .map_err(|_| "FOLD 3D frame registry is unavailable".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn preview() -> Fold3dFramesPreviewV1 {
        read_fold_3d_frames_preview_v1(
            br#"{"file_frames":[{"vertices_coords":[[0,0,0],[1,0,0],[0,1,0]]}]}"#,
            FoldImportLimits::default(),
        )
        .unwrap()
    }

    fn pending(token: ProjectId) -> Pending {
        Pending {
            token,
            instance_id: ProjectId::new(),
            project_id: ProjectId::new(),
            revision: 4,
            preview: preview(),
            prepared: None,
            prepared_timeline: None,
        }
    }

    #[test]
    fn cancel_is_idempotent_only_for_the_exact_native_token() {
        let state = Fold3dFramesImportState::default();
        let token = ProjectId::new();
        lock(&state).unwrap().pending = Some(pending(token));
        cancel_pending(&state, token).unwrap();
        cancel_pending(&state, token).unwrap();
        assert!(cancel_pending(&state, ProjectId::new()).is_err());
    }

    #[test]
    fn reentry_replaces_the_old_token_and_metadata_never_contains_coordinates() {
        let state = Fold3dFramesImportState::default();
        let old = ProjectId::new();
        let replacement = ProjectId::new();
        lock(&state).unwrap().pending = Some(pending(old));
        lock(&state).unwrap().pending = Some(pending(replacement));
        assert!(cancel_pending(&state, old).is_err());
        let slot = lock(&state).unwrap();
        let current = slot.pending.as_ref().unwrap();
        let value = serde_json::to_value(metadata(
            current.token,
            current.instance_id,
            current.project_id,
            current.revision,
            &current.preview,
        ))
        .unwrap();
        assert!(value.get("vertices").is_none());
        assert!(value["frames"][0].get("vertices").is_none());
        assert_eq!(value["authorizesProjectImport"], false);
    }

    #[test]
    fn native_camera_fit_png_is_deterministic_bounded_and_contains_no_coordinate_text() {
        let points = [[-123.5, 4.25, 9.0], [7.0, 88.0, -2.5], [0.0, 0.0, 0.0]];
        let first = render_vertex_cloud_png(&points).unwrap();
        let second = render_vertex_cloud_png(&points).unwrap();
        assert_eq!(first, second);
        assert_eq!((first.1, first.2), (512, 384));
        assert!(first.0.starts_with(b"\x89PNG\r\n\x1a\n"));
        assert!(first.0.len() <= 512 * 1024);
        assert!(!first.0.windows(6).any(|window| window == b"-123.5"));
    }

    #[test]
    fn timeline_chain_admission_rejects_branching_and_unauthenticated_topology() {
        let linear = read_fold_3d_frames_preview_v1(
            br#"{
              "vertices_coords":[[0,0,0],[1,0,0],[0,1,0]],
              "edges_vertices":[[0,1],[1,2],[2,0]],
              "edges_assignment":["B","B","B"],
              "faces_vertices":[[0,1,2]],
              "file_frames":[
                {"frame_inherit":true},
                {"frame_parent":0,"frame_inherit":true}
              ]
            }"#,
            FoldImportLimits::default(),
        )
        .unwrap();
        assert!(is_authenticated_linear_chain(&linear));

        let branching = read_fold_3d_frames_preview_v1(
            br#"{
              "vertices_coords":[[0,0,0],[1,0,0],[0,1,0]],
              "edges_vertices":[[0,1],[1,2],[2,0]],
              "edges_assignment":["B","B","B"],
              "faces_vertices":[[0,1,2]],
              "file_frames":[
                {"frame_inherit":true},
                {"frame_parent":0,"frame_inherit":true},
                {"frame_parent":0,"frame_inherit":true}
              ]
            }"#,
            FoldImportLimits::default(),
        )
        .unwrap();
        assert!(!is_authenticated_linear_chain(&branching));
        assert!(!is_authenticated_linear_chain(&preview()));
    }
}
