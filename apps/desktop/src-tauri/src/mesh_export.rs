//! Native authority for exporting the exact currently displayed material pose.
//!
//! The frontend can choose a format and observe bounded metadata, but it never
//! receives mesh coordinates, encoded bytes, or a filesystem path. One
//! immutable staged generation remains bound to the exact native applied-pose
//! capability until it is saved, cancelled, replaced, or made stale.
//! Cut topology, holes, seams, and non-simple material faces cannot mint that
//! capability in the first place, so unsupported material is rejected before
//! mesh construction rather than being flattened or silently omitted.

use std::{
    cmp::Ordering,
    collections::BTreeMap,
    path::Path,
    sync::{Arc, Mutex, MutexGuard},
};

use num_bigint::{BigInt, Sign};
use ori_domain::{ProjectId, VertexId};
use ori_formats::{
    IndexedTriangleMeshV1, MAX_STATIC_MESH_TRIANGLES, MAX_STATIC_MESH_VERTICES,
    STATIC_MESH_SOURCE_AXIS, STATIC_MESH_SOURCE_UNIT, StaticMeshExportArtifact,
    StaticMeshExportFormat, export_static_triangle_mesh, validate_indexed_triangle_mesh,
};
use ori_kinematics::{MaterialTreeKinematicsModel, MaterialTreePose, Point3};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

use super::{
    AppState, ProjectState,
    applied_pose::{
        CurrentAppliedPoseCapability, capture_current_applied_pose_capability,
        revalidate_current_applied_pose_capability,
    },
    crease_export::persist_export_bytes_to_destination,
    lock_project,
};

const MID_SURFACE_GEOMETRY_PROFILE: &str = "authenticated_mid_surface_triangle_mesh_v1";
const CLOSED_FACE_SOLIDS_GEOMETRY_PROFILE: &str = "authenticated_closed_face_solids_v1";
const MAX_EXACT_TRIANGULATION_PREDICATES: usize = 20_000_000;
const GLTF_ENCODED_UNIT: &str = "meter";
const GLTF_ENCODED_AXIS: &str = "glTF 2.0 right-handed -X-right Y-up Z-forward";
const PREVIEW_FAILED_MESSAGE: &str =
    "現在表示中の認証済み3D姿勢からメッシュを書き出せませんでした。";
const STALE_PREVIEW_MESSAGE: &str =
    "3D姿勢または編集内容が変わったため、書き出しデータを作り直してください。";

#[derive(Default)]
pub(super) struct StaticMeshExportState(Mutex<StaticMeshExportSlot>);

#[derive(Default)]
struct StaticMeshExportSlot {
    active_generation_id: Option<ProjectId>,
    pending: Option<Arc<PendingStaticMeshExport>>,
    last_cancelled_id: Option<ProjectId>,
}

struct PendingStaticMeshExport {
    export_id: ProjectId,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    source_fingerprint: Arc<str>,
    pose_generation: u64,
    pose_capability: CurrentAppliedPoseCapability,
    format: StaticMeshExportFormatRequest,
    suggested_file_name: String,
    bytes: Arc<[u8]>,
    paper_thickness_mm: f64,
    paper_thickness_bits: u64,
    face_count: usize,
    vertex_count: usize,
    triangle_count: usize,
    warnings: Arc<[StaticMeshExportWarning]>,
}

struct StaticMeshExportSource {
    export_id: ProjectId,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    source_fingerprint: Arc<str>,
    pose_generation: u64,
    pose_capability: CurrentAppliedPoseCapability,
    format: StaticMeshExportFormatRequest,
    project_name: String,
    paper_front_color_rgba: [u8; 4],
    paper_thickness_mm: f64,
    paper_thickness_bits: u64,
    model: MaterialTreeKinematicsModel,
    pose: MaterialTreePose,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum StaticMeshExportFormatRequest {
    Obj,
    Stl,
    Glb,
}

impl StaticMeshExportFormatRequest {
    const fn exporter_format(self) -> StaticMeshExportFormat {
        match self {
            Self::Obj => StaticMeshExportFormat::Obj,
            Self::Stl => StaticMeshExportFormat::BinaryStl,
            Self::Glb => StaticMeshExportFormat::Glb20,
        }
    }

    const fn extension(self) -> &'static str {
        self.exporter_format().file_extension()
    }

    const fn filter_label(self) -> &'static str {
        match self {
            Self::Obj => "Wavefront OBJ mid-surface mesh",
            Self::Stl => "Binary STL mid-surface mesh",
            Self::Glb => "glTF 2.0 binary mid-surface mesh",
        }
    }

    const fn format_summary(self) -> &'static str {
        match self {
            Self::Obj => "Wavefront OBJ・mm・右手系Z-up・静的三角形",
            Self::Stl => "Binary STL・mm・右手系Z-up・静的三角形",
            Self::Glb => "glTF 2.0 GLB・m・右手系Y-up・静的三角形",
        }
    }

    const fn encoded_unit(self) -> &'static str {
        match self {
            Self::Obj | Self::Stl => STATIC_MESH_SOURCE_UNIT,
            Self::Glb => GLTF_ENCODED_UNIT,
        }
    }

    const fn encoded_axis(self) -> &'static str {
        match self {
            Self::Obj | Self::Stl => STATIC_MESH_SOURCE_AXIS,
            Self::Glb => GLTF_ENCODED_AXIS,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum StaticMeshExportWarning {
    MidSurfaceOnly,
    NoThicknessSolid,
    IndependentFaceSolids,
    NoTexturesAnimation,
    NoProjectSemantics,
    StlTriangleSoupFacetNormals,
    StlPrintabilityNotGuaranteed,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct StaticMeshExportPreviewRequest {
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    format: StaticMeshExportFormatRequest,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct StaticMeshExportSaveRequest {
    export_id: ProjectId,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    expected_source_fingerprint: String,
    expected_pose_generation: String,
    warnings_acknowledged: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct StaticMeshExportPreviewResponse {
    preview: StaticMeshExportPreviewSnapshot,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct StaticMeshExportPreviewSnapshot {
    export_id: ProjectId,
    project_instance_id: ProjectId,
    project_id: ProjectId,
    revision: u64,
    source_fingerprint: String,
    pose_generation: String,
    format: StaticMeshExportFormatRequest,
    format_summary: String,
    suggested_file_name: String,
    byte_count: usize,
    paper_thickness_mm: f64,
    face_count: usize,
    vertex_count: usize,
    triangle_count: usize,
    geometry_profile: &'static str,
    source_unit: &'static str,
    encoded_unit: &'static str,
    source_axis: &'static str,
    encoded_axis: &'static str,
    warnings: Arc<[StaticMeshExportWarning]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct StaticMeshExportSaveResponse {
    canceled: bool,
}

#[tauri::command]
pub(super) async fn preview_static_mesh_export(
    state: State<'_, AppState>,
    export_state: State<'_, StaticMeshExportState>,
    request: StaticMeshExportPreviewRequest,
) -> Result<StaticMeshExportPreviewResponse, String> {
    let export_id = ProjectId::new();
    begin_export_generation(&export_state, export_id)?;
    let source = match capture_export_source(&state, export_id, request) {
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
                return Err(PREVIEW_FAILED_MESSAGE.to_owned());
            }
        };
    let pending = match built {
        Ok(pending) => Arc::new(pending),
        Err(error) => {
            abandon_export_generation(&export_state, export_id)?;
            return Err(error);
        }
    };

    let mut slot = lock_static_mesh_export(&export_state)?;
    ensure_generation_is_current(&slot, export_id)?;
    let project = lock_project(&state)?;
    if !pending_is_current(&project, &pending)? {
        slot.active_generation_id = None;
        slot.pending = None;
        return Err(STALE_PREVIEW_MESSAGE.to_owned());
    }
    let preview = preview_snapshot(&pending);
    slot.pending = Some(pending);
    Ok(StaticMeshExportPreviewResponse { preview })
}

#[tauri::command]
pub(super) async fn save_static_mesh_export(
    app: AppHandle,
    state: State<'_, AppState>,
    export_state: State<'_, StaticMeshExportState>,
    request: StaticMeshExportSaveRequest,
) -> Result<StaticMeshExportSaveResponse, String> {
    let expected_pose_generation = parse_canonical_u64(&request.expected_pose_generation)?;
    if !super::valid_fold_model_fingerprint(&request.expected_source_fingerprint) {
        return Err("3Dメッシュの生成元指紋が正しくありません。".to_owned());
    }
    let (pending, initial_directory) = {
        let slot = lock_static_mesh_export(&export_state)?;
        let project = lock_project(&state)?;
        let pending = checked_pending(&slot, &project, &request, expected_pose_generation)?;
        require_warning_acknowledgement(pending, request.warnings_acknowledged)?;
        (
            Arc::clone(pending),
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
        .set_title("現在の3D姿勢をメッシュとして書き出す");
    if let Some(directory) = initial_directory {
        dialog = dialog.set_directory(directory);
    }
    let Some(selected) = dialog.blocking_save_file() else {
        let slot = lock_static_mesh_export(&export_state)?;
        let project = lock_project(&state)?;
        let retained = checked_pending(&slot, &project, &request, expected_pose_generation)?;
        require_warning_acknowledgement(retained, request.warnings_acknowledged)?;
        return Ok(StaticMeshExportSaveResponse { canceled: true });
    };
    let selected_path = selected
        .simplified()
        .into_path()
        .map_err(|_| "選択された保存先はローカルファイルではありません。".to_owned())?;
    let destination =
        super::save_path::normalize_dialog_save_path(selected_path, pending.format.extension())?;

    let mut slot = lock_static_mesh_export(&export_state)?;
    let project = lock_project(&state)?;
    let pending = checked_pending(&slot, &project, &request, expected_pose_generation)?;
    require_warning_acknowledgement(pending, request.warnings_acknowledged)?;
    persist_export_bytes_to_destination(&destination, &pending.bytes)?;
    slot.pending = None;
    slot.active_generation_id = None;
    Ok(StaticMeshExportSaveResponse { canceled: false })
}

#[tauri::command]
pub(super) fn cancel_static_mesh_export(
    state: State<'_, StaticMeshExportState>,
    export_id: ProjectId,
) -> Result<(), String> {
    cancel_pending_export(&state, export_id)
}

fn capture_export_source(
    state: &AppState,
    export_id: ProjectId,
    request: StaticMeshExportPreviewRequest,
) -> Result<StaticMeshExportSource, String> {
    let project = lock_project(state)?;
    if project.instance_id != request.expected_project_instance_id
        || project.project_id != request.expected_project_id
        || project.editor.revision() != request.expected_revision
    {
        return Err(STALE_PREVIEW_MESSAGE.to_owned());
    }
    let pose_capability = capture_current_applied_pose_capability(&project)
        .map_err(|_| PREVIEW_FAILED_MESSAGE.to_owned())?
        .ok_or_else(|| {
            "書き出せる認証済み3D姿勢がありません。3D表示の更新完了後に再試行してください。"
                .to_owned()
        })?;
    let view = revalidate_current_applied_pose_capability(&project, &pose_capability)
        .map_err(|_| PREVIEW_FAILED_MESSAGE.to_owned())?
        .ok_or_else(|| STALE_PREVIEW_MESSAGE.to_owned())?;
    let source_fingerprint: Arc<str> = Arc::from(project.editor.fold_model_fingerprint_v1());
    if !super::valid_fold_model_fingerprint(&source_fingerprint) {
        return Err(PREVIEW_FAILED_MESSAGE.to_owned());
    }
    let paper_thickness_bits = view.paper_thickness_bits();
    let paper_thickness_mm = f64::from_bits(paper_thickness_bits);
    if !paper_thickness_mm.is_finite() || paper_thickness_mm < 0.0 {
        return Err(PREVIEW_FAILED_MESSAGE.to_owned());
    }
    let source = StaticMeshExportSource {
        export_id,
        expected_project_instance_id: project.instance_id,
        expected_project_id: project.project_id,
        expected_revision: project.editor.revision(),
        source_fingerprint,
        pose_generation: view.generation(),
        format: request.format,
        project_name: project.name.clone(),
        paper_front_color_rgba: {
            let color = project.editor.paper().front.color;
            [color.red, color.green, color.blue, color.alpha]
        },
        paper_thickness_mm: canonical_zero(paper_thickness_mm),
        paper_thickness_bits,
        model: view.model().clone(),
        pose: view.pose().clone(),
        pose_capability,
    };
    Ok(source)
}

fn build_pending_export(source: StaticMeshExportSource) -> Result<PendingStaticMeshExport, String> {
    source
        .model
        .bind_pose(&source.pose)
        .map_err(|_| PREVIEW_FAILED_MESSAGE.to_owned())?;
    let face_count = source.pose.face_ids().len();
    let mid_surface =
        build_current_pose_mid_surface_mesh(&source.project_name, &source.model, &source.pose)?;
    let mesh = if source.paper_thickness_mm > 0.0 {
        extrude_closed_face_solids(mid_surface, source.paper_thickness_mm)?
    } else {
        mid_surface
    }
    .with_base_color_rgba(source.paper_front_color_rgba);
    let validated =
        validate_indexed_triangle_mesh(&mesh).map_err(|_| PREVIEW_FAILED_MESSAGE.to_owned())?;
    let artifact = export_static_triangle_mesh(source.format.exporter_format(), &validated)
        .map_err(|_| PREVIEW_FAILED_MESSAGE.to_owned())?;
    validate_artifact_contract(source.format, &validated, &artifact)?;
    let warnings: Arc<[StaticMeshExportWarning]> = Arc::from(export_warnings(
        source.format,
        source.paper_thickness_mm > 0.0,
    ));
    Ok(PendingStaticMeshExport {
        export_id: source.export_id,
        expected_project_instance_id: source.expected_project_instance_id,
        expected_project_id: source.expected_project_id,
        expected_revision: source.expected_revision,
        source_fingerprint: source.source_fingerprint,
        pose_generation: source.pose_generation,
        pose_capability: source.pose_capability,
        format: source.format,
        suggested_file_name: suggested_export_file_name(
            &source.project_name,
            source.format.extension(),
        ),
        bytes: Arc::from(artifact.bytes),
        paper_thickness_mm: source.paper_thickness_mm,
        paper_thickness_bits: source.paper_thickness_bits,
        face_count,
        vertex_count: artifact.vertex_count,
        triangle_count: artifact.triangle_count,
        warnings,
    })
}

fn validate_artifact_contract(
    requested: StaticMeshExportFormatRequest,
    mesh: &ori_formats::ValidatedIndexedTriangleMesh,
    artifact: &StaticMeshExportArtifact,
) -> Result<(), String> {
    if artifact.format != requested.exporter_format()
        || artifact.media_type != requested.exporter_format().media_type()
        || artifact.file_extension != requested.extension()
        || artifact.bytes.is_empty()
        || artifact.vertex_count != mesh.positions_mm().len()
        || artifact.triangle_count != mesh.triangles().len()
    {
        return Err(PREVIEW_FAILED_MESSAGE.to_owned());
    }
    Ok(())
}

fn export_warnings(
    format: StaticMeshExportFormatRequest,
    has_thickness: bool,
) -> Vec<StaticMeshExportWarning> {
    let mut warnings = if has_thickness {
        vec![StaticMeshExportWarning::IndependentFaceSolids]
    } else {
        vec![
            StaticMeshExportWarning::MidSurfaceOnly,
            StaticMeshExportWarning::NoThicknessSolid,
        ]
    };
    warnings.extend([
        StaticMeshExportWarning::NoTexturesAnimation,
        StaticMeshExportWarning::NoProjectSemantics,
    ]);
    if format == StaticMeshExportFormatRequest::Stl {
        warnings.push(StaticMeshExportWarning::StlTriangleSoupFacetNormals);
        warnings.push(StaticMeshExportWarning::StlPrintabilityNotGuaranteed);
    }
    warnings
}

fn preview_snapshot(pending: &PendingStaticMeshExport) -> StaticMeshExportPreviewSnapshot {
    StaticMeshExportPreviewSnapshot {
        export_id: pending.export_id,
        project_instance_id: pending.expected_project_instance_id,
        project_id: pending.expected_project_id,
        revision: pending.expected_revision,
        source_fingerprint: pending.source_fingerprint.to_string(),
        pose_generation: pending.pose_generation.to_string(),
        format: pending.format,
        format_summary: pending.format.format_summary().to_owned(),
        suggested_file_name: pending.suggested_file_name.clone(),
        byte_count: pending.bytes.len(),
        paper_thickness_mm: pending.paper_thickness_mm,
        face_count: pending.face_count,
        vertex_count: pending.vertex_count,
        triangle_count: pending.triangle_count,
        geometry_profile: if pending.paper_thickness_mm > 0.0 {
            CLOSED_FACE_SOLIDS_GEOMETRY_PROFILE
        } else {
            MID_SURFACE_GEOMETRY_PROFILE
        },
        source_unit: STATIC_MESH_SOURCE_UNIT,
        encoded_unit: pending.format.encoded_unit(),
        source_axis: STATIC_MESH_SOURCE_AXIS,
        encoded_axis: pending.format.encoded_axis(),
        warnings: Arc::clone(&pending.warnings),
    }
}

fn checked_pending<'a>(
    slot: &'a StaticMeshExportSlot,
    project: &ProjectState,
    request: &StaticMeshExportSaveRequest,
    expected_pose_generation: u64,
) -> Result<&'a Arc<PendingStaticMeshExport>, String> {
    let pending = slot
        .pending
        .as_ref()
        .ok_or_else(|| "3Dメッシュの書き出しプレビューは既に破棄されています。".to_owned())?;
    if pending.export_id != request.export_id {
        return Err(
            "3Dメッシュの書き出しプレビューは新しいプレビューに置き換えられました。".to_owned(),
        );
    }
    if pending.expected_project_instance_id != request.expected_project_instance_id
        || pending.expected_project_id != request.expected_project_id
        || pending.expected_revision != request.expected_revision
        || pending.source_fingerprint.as_ref() != request.expected_source_fingerprint
        || pending.pose_generation != expected_pose_generation
    {
        return Err(STALE_PREVIEW_MESSAGE.to_owned());
    }
    ensure_generation_is_current(slot, request.export_id)?;
    if !pending_is_current(project, pending)? {
        return Err(STALE_PREVIEW_MESSAGE.to_owned());
    }
    Ok(pending)
}

fn pending_is_current(
    project: &ProjectState,
    pending: &PendingStaticMeshExport,
) -> Result<bool, String> {
    if project.instance_id != pending.expected_project_instance_id
        || project.project_id != pending.expected_project_id
        || project.editor.revision() != pending.expected_revision
        || project.editor.fold_model_fingerprint_v1() != pending.source_fingerprint.as_ref()
    {
        return Ok(false);
    }
    let view = revalidate_current_applied_pose_capability(project, &pending.pose_capability)
        .map_err(|_| PREVIEW_FAILED_MESSAGE.to_owned())?;
    Ok(view.is_some_and(|view| {
        view.generation() == pending.pose_generation
            && view.paper_thickness_bits() == pending.paper_thickness_bits
    }))
}

fn require_warning_acknowledgement(
    pending: &PendingStaticMeshExport,
    warnings_acknowledged: bool,
) -> Result<(), String> {
    if !pending.warnings.is_empty() && !warnings_acknowledged {
        Err("3Dメッシュの情報損失について確認が必要です。".to_owned())
    } else {
        Ok(())
    }
}

fn lock_static_mesh_export(
    state: &StaticMeshExportState,
) -> Result<MutexGuard<'_, StaticMeshExportSlot>, String> {
    state
        .0
        .lock()
        .map_err(|_| "3Dメッシュ書き出し状態を利用できません。".to_owned())
}

fn begin_export_generation(
    state: &StaticMeshExportState,
    export_id: ProjectId,
) -> Result<(), String> {
    let mut slot = lock_static_mesh_export(state)?;
    slot.pending = None;
    slot.active_generation_id = Some(export_id);
    Ok(())
}

fn abandon_export_generation(
    state: &StaticMeshExportState,
    export_id: ProjectId,
) -> Result<(), String> {
    let mut slot = lock_static_mesh_export(state)?;
    if slot.active_generation_id == Some(export_id) {
        slot.active_generation_id = None;
        slot.pending = None;
    }
    Ok(())
}

fn ensure_generation_is_current(
    slot: &StaticMeshExportSlot,
    export_id: ProjectId,
) -> Result<(), String> {
    if slot.active_generation_id == Some(export_id) {
        Ok(())
    } else {
        Err("この3Dメッシュ生成は新しい書き出し処理に置き換えられました。".to_owned())
    }
}

fn cancel_pending_export(
    state: &StaticMeshExportState,
    export_id: ProjectId,
) -> Result<(), String> {
    let mut slot = lock_static_mesh_export(state)?;
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
        return Err("この3Dメッシュプレビューは新しいプレビューに置き換えられました。".to_owned());
    }
    Err("指定された3Dメッシュプレビューは存在しません。".to_owned())
}

fn parse_canonical_u64(value: &str) -> Result<u64, String> {
    if value.is_empty()
        || value.len() > 20
        || !value.bytes().all(|byte| byte.is_ascii_digit())
        || (value.len() > 1 && value.starts_with('0'))
    {
        return Err("3D姿勢世代の形式が正しくありません。".to_owned());
    }
    let parsed = value
        .parse::<u64>()
        .map_err(|_| "3D姿勢世代の形式が正しくありません。".to_owned())?;
    if parsed.to_string() != value {
        return Err("3D姿勢世代の形式が正しくありません。".to_owned());
    }
    Ok(parsed)
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
    format!("{base}-pose.{extension}")
}

fn build_current_pose_mid_surface_mesh(
    name: &str,
    model: &MaterialTreeKinematicsModel,
    pose: &MaterialTreePose,
) -> Result<IndexedTriangleMeshV1, String> {
    model
        .bind_pose(pose)
        .map_err(|_| PREVIEW_FAILED_MESSAGE.to_owned())?;
    if model.face_ids() != pose.face_ids() || pose.face_ids().is_empty() {
        return Err(PREVIEW_FAILED_MESSAGE.to_owned());
    }

    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut triangles = Vec::new();
    let mut budget = ExactPredicateBudget::new(MAX_EXACT_TRIANGULATION_PREDICATES);

    for face in pose.face_ids().iter().copied() {
        let boundary = model
            .face_boundary(face)
            .filter(|boundary| pose.owns_face_boundary(*boundary))
            .ok_or_else(|| PREVIEW_FAILED_MESSAGE.to_owned())?;
        let transform = pose
            .face_transform(face)
            .ok_or_else(|| PREVIEW_FAILED_MESSAGE.to_owned())?;
        let mut face_vertices = Vec::new();
        face_vertices
            .try_reserve_exact(boundary.vertices().len())
            .map_err(|_| PREVIEW_FAILED_MESSAGE.to_owned())?;
        for vertex in boundary.vertices().iter().copied() {
            let rest = pose
                .vertex_position(vertex)
                .ok_or_else(|| PREVIEW_FAILED_MESSAGE.to_owned())?;
            let world = transform
                .apply_point(rest)
                .map_err(|_| PREVIEW_FAILED_MESSAGE.to_owned())?;
            face_vertices.push(FaceVertex {
                id: vertex,
                rest_2d: [rest.x(), -rest.z()],
                world_export: kinematics_to_export_coordinates(world),
            });
        }
        let triangulation = triangulate_face(&face_vertices, &mut budget)?;
        let next_vertex_count = positions
            .len()
            .checked_add(triangulation.active_vertices.len())
            .ok_or_else(|| PREVIEW_FAILED_MESSAGE.to_owned())?;
        let next_triangle_count = triangles
            .len()
            .checked_add(triangulation.triangles.len())
            .ok_or_else(|| PREVIEW_FAILED_MESSAGE.to_owned())?;
        if next_vertex_count > MAX_STATIC_MESH_VERTICES
            || next_triangle_count > MAX_STATIC_MESH_TRIANGLES
        {
            return Err("3Dメッシュが書き出し上限を超えています。".to_owned());
        }

        let base = u32::try_from(positions.len()).map_err(|_| PREVIEW_FAILED_MESSAGE.to_owned())?;
        let mut remap = Vec::new();
        remap
            .try_reserve_exact(face_vertices.len())
            .map_err(|_| PREVIEW_FAILED_MESSAGE.to_owned())?;
        remap.resize(face_vertices.len(), None);
        for (local, source_index) in triangulation.active_vertices.iter().copied().enumerate() {
            let local = u32::try_from(local).map_err(|_| PREVIEW_FAILED_MESSAGE.to_owned())?;
            remap[source_index] = Some(
                base.checked_add(local)
                    .ok_or_else(|| PREVIEW_FAILED_MESSAGE.to_owned())?,
            );
            positions.push(face_vertices[source_index].world_export);
        }

        let material_normal = transform
            .apply_vector(
                Point3::new(0.0, 1.0, 0.0).map_err(|_| PREVIEW_FAILED_MESSAGE.to_owned())?,
            )
            .map_err(|_| PREVIEW_FAILED_MESSAGE.to_owned())?;
        let expected_normal = normalize(kinematics_to_export_coordinates(material_normal))
            .ok_or_else(|| PREVIEW_FAILED_MESSAGE.to_owned())?;
        let first = triangulation
            .triangles
            .first()
            .copied()
            .ok_or_else(|| PREVIEW_FAILED_MESSAGE.to_owned())?;
        let geometric_normal =
            triangle_normal(first.map(|index| face_vertices[index].world_export))
                .ok_or_else(|| PREVIEW_FAILED_MESSAGE.to_owned())?;
        let reverse = dot(geometric_normal, expected_normal) < 0.0;
        normals.extend(std::iter::repeat_n(
            expected_normal,
            triangulation.active_vertices.len(),
        ));
        for source_triangle in triangulation.triangles {
            let mut triangle = source_triangle.map(|index| {
                remap[index].expect("every triangulated vertex belongs to the active registry")
            });
            if reverse {
                triangle.swap(1, 2);
            }
            triangles.push(triangle);
        }
    }
    Ok(IndexedTriangleMeshV1::new(
        name, positions, normals, triangles,
    ))
}

fn extrude_closed_face_solids(
    mesh: IndexedTriangleMeshV1,
    thickness_mm: f64,
) -> Result<IndexedTriangleMeshV1, String> {
    if !thickness_mm.is_finite() || thickness_mm <= 0.0 {
        return Err(PREVIEW_FAILED_MESSAGE.to_owned());
    }
    let source_vertex_count = mesh.positions_mm.len();
    let half = thickness_mm / 2.0;
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    positions
        .try_reserve(source_vertex_count.saturating_mul(2))
        .map_err(|_| PREVIEW_FAILED_MESSAGE.to_owned())?;
    normals
        .try_reserve(source_vertex_count.saturating_mul(2))
        .map_err(|_| PREVIEW_FAILED_MESSAGE.to_owned())?;
    for (position, normal) in mesh.positions_mm.iter().zip(&mesh.normals) {
        positions.push([
            position[0] + normal[0] * half,
            position[1] + normal[1] * half,
            position[2] + normal[2] * half,
        ]);
        normals.push(*normal);
    }
    for (position, normal) in mesh.positions_mm.iter().zip(&mesh.normals) {
        positions.push([
            position[0] - normal[0] * half,
            position[1] - normal[1] * half,
            position[2] - normal[2] * half,
        ]);
        normals.push(normal.map(|component| -component));
    }
    let bottom_offset =
        u32::try_from(source_vertex_count).map_err(|_| PREVIEW_FAILED_MESSAGE.to_owned())?;
    let mut triangles = Vec::new();
    let mut edges = BTreeMap::<(u32, u32), (usize, u32, u32)>::new();
    for triangle in &mesh.triangles {
        triangles.push(*triangle);
        triangles.push([
            triangle[2] + bottom_offset,
            triangle[1] + bottom_offset,
            triangle[0] + bottom_offset,
        ]);
        for (start, end) in [
            (triangle[0], triangle[1]),
            (triangle[1], triangle[2]),
            (triangle[2], triangle[0]),
        ] {
            let key = (start.min(end), start.max(end));
            edges
                .entry(key)
                .and_modify(|entry| entry.0 += 1)
                .or_insert((1, start, end));
        }
    }
    for (_, (count, start, end)) in edges {
        if count != 1 {
            continue;
        }
        let start_index = usize::try_from(start).map_err(|_| PREVIEW_FAILED_MESSAGE.to_owned())?;
        let end_index = usize::try_from(end).map_err(|_| PREVIEW_FAILED_MESSAGE.to_owned())?;
        let a = mesh.positions_mm[start_index];
        let b = mesh.positions_mm[end_index];
        let normal = mesh.normals[start_index];
        let edge = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let side_normal = normalize([
            edge[1] * normal[2] - edge[2] * normal[1],
            edge[2] * normal[0] - edge[0] * normal[2],
            edge[0] * normal[1] - edge[1] * normal[0],
        ])
        .ok_or_else(|| PREVIEW_FAILED_MESSAGE.to_owned())?;
        let base = u32::try_from(positions.len()).map_err(|_| PREVIEW_FAILED_MESSAGE.to_owned())?;
        let top_a = positions[start_index];
        let top_b = positions[end_index];
        let bottom_a = positions[start_index + source_vertex_count];
        let bottom_b = positions[end_index + source_vertex_count];
        positions.extend([top_a, top_b, bottom_b, bottom_a]);
        normals.extend([side_normal; 4]);
        triangles.push([base, base + 1, base + 2]);
        triangles.push([base, base + 2, base + 3]);
    }
    if positions.len() > MAX_STATIC_MESH_VERTICES || triangles.len() > MAX_STATIC_MESH_TRIANGLES {
        return Err(PREVIEW_FAILED_MESSAGE.to_owned());
    }
    Ok(
        IndexedTriangleMeshV1::new(mesh.name, positions, normals, triangles)
            .with_base_color_rgba(mesh.base_color_rgba),
    )
}

#[derive(Clone, Copy)]
struct FaceVertex {
    id: VertexId,
    rest_2d: [f64; 2],
    world_export: [f64; 3],
}

struct FaceTriangulation {
    active_vertices: Vec<usize>,
    triangles: Vec<[usize; 3]>,
}

fn triangulate_face(
    boundary: &[FaceVertex],
    budget: &mut ExactPredicateBudget,
) -> Result<FaceTriangulation, String> {
    if boundary.len() < 3 || boundary.len() > MAX_STATIC_MESH_VERTICES {
        return Err(PREVIEW_FAILED_MESSAGE.to_owned());
    }
    let mut active = Vec::new();
    active
        .try_reserve_exact(boundary.len())
        .map_err(|_| PREVIEW_FAILED_MESSAGE.to_owned())?;
    active.extend(0..boundary.len());
    remove_collinear_vertices(boundary, &mut active, budget)?;
    if active.len() < 3 {
        return Err(PREVIEW_FAILED_MESSAGE.to_owned());
    }
    let orientation = polygon_orientation(boundary, &active, budget)?;
    if orientation == Ordering::Equal {
        return Err(PREVIEW_FAILED_MESSAGE.to_owned());
    }
    let expected_count = active
        .len()
        .checked_sub(2)
        .ok_or_else(|| PREVIEW_FAILED_MESSAGE.to_owned())?;
    let active_vertices = active.clone();
    let mut triangles = Vec::new();
    triangles
        .try_reserve_exact(expected_count)
        .map_err(|_| PREVIEW_FAILED_MESSAGE.to_owned())?;

    while active.len() > 3 {
        let mut selected: Option<usize> = None;
        for position in 0..active.len() {
            if is_ear(boundary, &active, position, orientation, budget)?
                && selected.is_none_or(|candidate| {
                    boundary[active[position]].id.canonical_bytes()
                        < boundary[active[candidate]].id.canonical_bytes()
                })
            {
                selected = Some(position);
            }
        }
        let position = selected.ok_or_else(|| PREVIEW_FAILED_MESSAGE.to_owned())?;
        let previous = active[(position + active.len() - 1) % active.len()];
        let current = active[position];
        let next = active[(position + 1) % active.len()];
        triangles.push(canonical_triangle_cycle(
            boundary,
            [previous, current, next],
        ));
        active.remove(position);
    }
    triangles.push(canonical_triangle_cycle(
        boundary,
        [active[0], active[1], active[2]],
    ));
    triangles.sort_unstable_by_key(|triangle| {
        triangle.map(|index| boundary[index].id.canonical_bytes())
    });
    if triangles.len() != expected_count {
        return Err(PREVIEW_FAILED_MESSAGE.to_owned());
    }
    Ok(FaceTriangulation {
        active_vertices,
        triangles,
    })
}

fn remove_collinear_vertices(
    boundary: &[FaceVertex],
    active: &mut Vec<usize>,
    budget: &mut ExactPredicateBudget,
) -> Result<(), String> {
    loop {
        let mut selected: Option<usize> = None;
        for position in 0..active.len() {
            let previous = active[(position + active.len() - 1) % active.len()];
            let current = active[position];
            let next = active[(position + 1) % active.len()];
            if exact_orientation(
                boundary[previous].rest_2d,
                boundary[current].rest_2d,
                boundary[next].rest_2d,
                budget,
            )? != Ordering::Equal
                || !point_between(
                    boundary[current].rest_2d,
                    boundary[previous].rest_2d,
                    boundary[next].rest_2d,
                )
            {
                continue;
            }
            if selected.is_none_or(|candidate| {
                boundary[current].id.canonical_bytes()
                    < boundary[active[candidate]].id.canonical_bytes()
            }) {
                selected = Some(position);
            }
        }
        let Some(position) = selected else {
            return Ok(());
        };
        active.remove(position);
        if active.len() < 3 {
            return Err(PREVIEW_FAILED_MESSAGE.to_owned());
        }
    }
}

fn polygon_orientation(
    boundary: &[FaceVertex],
    active: &[usize],
    budget: &mut ExactPredicateBudget,
) -> Result<Ordering, String> {
    let mut area = Dyadic::zero();
    for index in 0..active.len() {
        budget.charge()?;
        let current = boundary[active[index]].rest_2d;
        let next = boundary[active[(index + 1) % active.len()]].rest_2d;
        area = area.add(
            Dyadic::from_f64(current[0])
                .multiply(Dyadic::from_f64(next[1]))
                .subtract(Dyadic::from_f64(current[1]).multiply(Dyadic::from_f64(next[0]))),
        );
    }
    Ok(area.ordering())
}

fn is_ear(
    boundary: &[FaceVertex],
    active: &[usize],
    position: usize,
    polygon_orientation: Ordering,
    budget: &mut ExactPredicateBudget,
) -> Result<bool, String> {
    let previous = active[(position + active.len() - 1) % active.len()];
    let current = active[position];
    let next = active[(position + 1) % active.len()];
    if exact_orientation(
        boundary[previous].rest_2d,
        boundary[current].rest_2d,
        boundary[next].rest_2d,
        budget,
    )? != polygon_orientation
    {
        return Ok(false);
    }
    for candidate in active.iter().copied() {
        if candidate == previous || candidate == current || candidate == next {
            continue;
        }
        if point_in_or_on_triangle(
            boundary[candidate].rest_2d,
            boundary[previous].rest_2d,
            boundary[current].rest_2d,
            boundary[next].rest_2d,
            polygon_orientation,
            budget,
        )? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn point_in_or_on_triangle(
    point: [f64; 2],
    first: [f64; 2],
    second: [f64; 2],
    third: [f64; 2],
    orientation: Ordering,
    budget: &mut ExactPredicateBudget,
) -> Result<bool, String> {
    for (start, end) in [(first, second), (second, third), (third, first)] {
        let side = exact_orientation(start, end, point, budget)?;
        if side != Ordering::Equal && side != orientation {
            return Ok(false);
        }
    }
    Ok(true)
}

fn exact_orientation(
    first: [f64; 2],
    second: [f64; 2],
    third: [f64; 2],
    budget: &mut ExactPredicateBudget,
) -> Result<Ordering, String> {
    budget.charge()?;
    let first_x = Dyadic::from_f64(first[0]);
    let first_y = Dyadic::from_f64(first[1]);
    let left = Dyadic::from_f64(second[0])
        .subtract(first_x.clone())
        .multiply(Dyadic::from_f64(third[1]).subtract(first_y.clone()));
    let right = Dyadic::from_f64(second[1])
        .subtract(first_y)
        .multiply(Dyadic::from_f64(third[0]).subtract(first_x));
    Ok(left.subtract(right).ordering())
}

fn canonical_triangle_cycle(boundary: &[FaceVertex], mut triangle: [usize; 3]) -> [usize; 3] {
    let smallest = (0..3)
        .min_by_key(|index| boundary[triangle[*index]].id.canonical_bytes())
        .expect("a triangle has three vertices");
    triangle.rotate_left(smallest);
    triangle
}

fn point_between(point: [f64; 2], start: [f64; 2], end: [f64; 2]) -> bool {
    (start[0].min(end[0])..=start[0].max(end[0])).contains(&point[0])
        && (start[1].min(end[1])..=start[1].max(end[1])).contains(&point[1])
}

struct ExactPredicateBudget {
    remaining: usize,
}

impl ExactPredicateBudget {
    const fn new(maximum: usize) -> Self {
        Self { remaining: maximum }
    }

    fn charge(&mut self) -> Result<(), String> {
        self.remaining = self
            .remaining
            .checked_sub(1)
            .ok_or_else(|| "3Dメッシュの三角形分割が処理上限を超えています。".to_owned())?;
        Ok(())
    }
}

#[derive(Clone)]
struct Dyadic {
    coefficient: BigInt,
    exponent: i32,
}

impl Dyadic {
    fn zero() -> Self {
        Self {
            coefficient: BigInt::from(0_u8),
            exponent: 0,
        }
    }

    fn from_f64(value: f64) -> Self {
        debug_assert!(value.is_finite());
        let bits = value.to_bits();
        let negative = bits >> 63 != 0;
        let exponent_bits = ((bits >> 52) & 0x7ff) as i32;
        let fraction = bits & ((1_u64 << 52) - 1);
        let (significand, exponent) = if exponent_bits == 0 {
            (fraction, -1074)
        } else {
            (fraction | (1_u64 << 52), exponent_bits - 1075)
        };
        let mut coefficient = BigInt::from(significand);
        if negative {
            coefficient = -coefficient;
        }
        Self {
            coefficient,
            exponent,
        }
    }

    fn add(self, other: Self) -> Self {
        let exponent = self.exponent.min(other.exponent);
        let left_shift = usize::try_from(self.exponent - exponent)
            .expect("finite f64 exponents have a bounded difference");
        let right_shift = usize::try_from(other.exponent - exponent)
            .expect("finite f64 exponents have a bounded difference");
        Self {
            coefficient: (self.coefficient << left_shift) + (other.coefficient << right_shift),
            exponent,
        }
    }

    fn subtract(self, other: Self) -> Self {
        let exponent = self.exponent.min(other.exponent);
        let left_shift = usize::try_from(self.exponent - exponent)
            .expect("finite f64 exponents have a bounded difference");
        let right_shift = usize::try_from(other.exponent - exponent)
            .expect("finite f64 exponents have a bounded difference");
        Self {
            coefficient: (self.coefficient << left_shift) - (other.coefficient << right_shift),
            exponent,
        }
    }

    fn multiply(self, other: Self) -> Self {
        Self {
            coefficient: self.coefficient * other.coefficient,
            exponent: self.exponent + other.exponent,
        }
    }

    fn ordering(&self) -> Ordering {
        match self.coefficient.sign() {
            Sign::Minus => Ordering::Less,
            Sign::NoSign => Ordering::Equal,
            Sign::Plus => Ordering::Greater,
        }
    }
}

fn kinematics_to_export_coordinates(point: Point3) -> [f64; 3] {
    [
        canonical_zero(point.x()),
        canonical_zero(-point.z()),
        canonical_zero(point.y()),
    ]
}

fn canonical_zero(value: f64) -> f64 {
    if value == 0.0 { 0.0 } else { value }
}

fn triangle_normal(triangle: [[f64; 3]; 3]) -> Option<[f64; 3]> {
    let first = subtract(triangle[1], triangle[0])?;
    let second = subtract(triangle[2], triangle[0])?;
    normalize([
        first[1] * second[2] - first[2] * second[1],
        first[2] * second[0] - first[0] * second[2],
        first[0] * second[1] - first[1] * second[0],
    ])
}

fn subtract(left: [f64; 3], right: [f64; 3]) -> Option<[f64; 3]> {
    let result = [left[0] - right[0], left[1] - right[1], left[2] - right[2]];
    result
        .iter()
        .all(|value| value.is_finite())
        .then_some(result)
}

fn normalize(vector: [f64; 3]) -> Option<[f64; 3]> {
    let scale = vector
        .iter()
        .map(|value| value.abs())
        .fold(0.0_f64, f64::max);
    if !scale.is_finite() || scale == 0.0 {
        return None;
    }
    let scaled = vector.map(|value| value / scale);
    let length = scaled.iter().map(|value| value * value).sum::<f64>().sqrt();
    if !length.is_finite() || length == 0.0 {
        return None;
    }
    Some(scaled.map(|value| canonical_zero(value / length)))
}

fn dot(left: [f64; 3], right: [f64; 3]) -> f64 {
    left[0] * right[0] + left[1] * right[1] + left[2] * right[2]
}

#[cfg(test)]
mod tests {
    use ori_core::Command;
    use ori_domain::LengthDisplayUnit;

    use super::*;
    use crate::applied_pose::NativePoseRequest;

    fn vertex(_id: u8, x: f64, y: f64) -> FaceVertex {
        FaceVertex {
            id: VertexId::new(),
            rest_2d: [x, y],
            world_export: [x, y, 0.0],
        }
    }

    #[test]
    fn exact_orientation_handles_nearly_collinear_binary64_values() {
        let mut budget = ExactPredicateBudget::new(10);
        assert_eq!(
            exact_orientation(
                [0.0, 0.0],
                [1.0, f64::from_bits(1)],
                [2.0, f64::from_bits(3)],
                &mut budget,
            )
            .unwrap(),
            Ordering::Greater
        );
    }

    #[test]
    fn triangulates_convex_concave_and_collinear_faces() {
        let cases = [
            (
                vec![
                    vertex(1, 0.0, 0.0),
                    vertex(2, 4.0, 0.0),
                    vertex(3, 4.0, 4.0),
                    vertex(4, 0.0, 4.0),
                ],
                4,
                2,
            ),
            (
                vec![
                    vertex(1, 0.0, 0.0),
                    vertex(2, 4.0, 0.0),
                    vertex(3, 4.0, 4.0),
                    vertex(4, 2.0, 2.0),
                    vertex(5, 0.0, 4.0),
                ],
                5,
                3,
            ),
            (
                vec![
                    vertex(1, 0.0, 0.0),
                    vertex(2, 2.0, 0.0),
                    vertex(3, 4.0, 0.0),
                    vertex(4, 4.0, 4.0),
                    vertex(5, 0.0, 4.0),
                ],
                4,
                2,
            ),
        ];
        for (face, expected_vertices, expected_triangles) in cases {
            let result = triangulate_face(
                &face,
                &mut ExactPredicateBudget::new(MAX_EXACT_TRIANGULATION_PREDICATES),
            )
            .unwrap();
            assert_eq!(result.active_vertices.len(), expected_vertices);
            assert_eq!(result.triangles.len(), expected_triangles);
        }
    }

    #[test]
    fn canonical_u64_parser_rejects_noncanonical_and_overflow_values() {
        assert_eq!(parse_canonical_u64("0").unwrap(), 0);
        assert_eq!(
            parse_canonical_u64("18446744073709551615").unwrap(),
            u64::MAX
        );
        for invalid in ["", "00", "01", "-1", "+1", "1.0", "18446744073709551616"] {
            assert!(parse_canonical_u64(invalid).is_err(), "{invalid}");
        }
    }

    #[test]
    fn preview_and_save_requests_reject_unknown_fields() {
        let instance = ProjectId::new();
        let project = ProjectId::new();
        let preview = serde_json::json!({
            "expectedProjectInstanceId": instance,
            "expectedProjectId": project,
            "expectedRevision": 0,
            "format": "obj",
            "bytes": [1, 2, 3],
        });
        assert!(serde_json::from_value::<StaticMeshExportPreviewRequest>(preview).is_err());
    }

    fn app_state_with_current_pose() -> AppState {
        let mut project = crate::initial_project_state();
        let authority = project.applied_pose_authority.clone();
        let captured = authority
            .capture_request(
                &project,
                NativePoseRequest {
                    expected_project_instance_id: project.instance_id,
                    expected_project_id: project.project_id,
                    expected_revision: project.editor.revision(),
                    fixed_face_id: None,
                    complete_hinge_angles: Vec::new(),
                },
            )
            .expect("capture initial planar pose");
        let prepared = captured.prepare().expect("prepare initial planar pose");
        authority
            .commit_prepared(&mut project, prepared)
            .expect("commit initial planar pose");
        AppState::new(project)
    }

    fn preview_request(
        state: &AppState,
        format: StaticMeshExportFormatRequest,
    ) -> StaticMeshExportPreviewRequest {
        let project = lock_project(state).expect("project");
        StaticMeshExportPreviewRequest {
            expected_project_instance_id: project.instance_id,
            expected_project_id: project.project_id,
            expected_revision: project.editor.revision(),
            format,
        }
    }

    #[test]
    fn current_authenticated_pose_builds_all_three_immutable_artifacts() {
        let state = app_state_with_current_pose();
        for format in [
            StaticMeshExportFormatRequest::Obj,
            StaticMeshExportFormatRequest::Stl,
            StaticMeshExportFormatRequest::Glb,
        ] {
            let request = preview_request(&state, format);
            let source =
                capture_export_source(&state, ProjectId::new(), request).expect("capture source");
            let pending = build_pending_export(source).expect("build staged mesh");
            assert_eq!(pending.face_count, 1);
            assert_eq!(pending.vertex_count, 24);
            assert_eq!(pending.triangle_count, 12);
            assert!(!pending.bytes.is_empty());
            match format {
                StaticMeshExportFormatRequest::Obj => {
                    assert!(pending.bytes.starts_with(b"# ORIGAMI2"));
                }
                StaticMeshExportFormatRequest::Stl => {
                    assert!(pending.bytes.starts_with(b"ORIGAMI2"));
                }
                StaticMeshExportFormatRequest::Glb => {
                    assert_eq!(&pending.bytes[..4], b"glTF");
                }
            }
            let json = serde_json::to_value(StaticMeshExportPreviewResponse {
                preview: preview_snapshot(&pending),
            })
            .expect("serialize bounded preview");
            let rendered = json.to_string();
            assert!(!rendered.contains("positions"));
            assert!(!rendered.contains("triangles"));
            assert!(!rendered.contains("bytes"));
            assert!(!rendered.contains("path"));
        }
    }

    #[test]
    fn positive_thickness_extrudes_front_back_and_closed_side_faces() {
        use std::collections::BTreeMap;

        let state = app_state_with_current_pose();
        let request = preview_request(&state, StaticMeshExportFormatRequest::Stl);
        let source =
            capture_export_source(&state, ProjectId::new(), request).expect("capture source");
        let mid =
            build_current_pose_mid_surface_mesh(&source.project_name, &source.model, &source.pose)
                .expect("mid surface");
        let solid = extrude_closed_face_solids(mid, 0.1).expect("solid extrusion");
        assert_eq!(solid.positions_mm.len(), 24);
        assert_eq!(solid.triangles.len(), 12);
        let min_z = solid
            .positions_mm
            .iter()
            .map(|position| position[2])
            .fold(f64::INFINITY, f64::min);
        let max_z = solid
            .positions_mm
            .iter()
            .map(|position| position[2])
            .fold(f64::NEG_INFINITY, f64::max);
        assert_eq!(max_z - min_z, 0.1);
        validate_indexed_triangle_mesh(&solid).expect("validated closed face solids");
        let point_key =
            |index: u32| solid.positions_mm[index as usize].map(|component| component.to_bits());
        let mut geometric_edges = BTreeMap::new();
        for triangle in &solid.triangles {
            for (start, end) in [
                (triangle[0], triangle[1]),
                (triangle[1], triangle[2]),
                (triangle[2], triangle[0]),
            ] {
                let a = point_key(start);
                let b = point_key(end);
                let key = if a <= b { (a, b) } else { (b, a) };
                *geometric_edges.entry(key).or_insert(0_usize) += 1;
            }
        }
        assert!(
            geometric_edges.values().all(|incidence| *incidence == 2),
            "every geometric edge must belong to exactly two triangles"
        );
    }

    #[test]
    fn warning_allowlist_is_closed_and_stl_discloses_triangle_soup_conversion() {
        assert_eq!(
            export_warnings(StaticMeshExportFormatRequest::Obj, false),
            vec![
                StaticMeshExportWarning::MidSurfaceOnly,
                StaticMeshExportWarning::NoThicknessSolid,
                StaticMeshExportWarning::NoTexturesAnimation,
                StaticMeshExportWarning::NoProjectSemantics,
            ]
        );
        assert_eq!(
            export_warnings(StaticMeshExportFormatRequest::Glb, false),
            export_warnings(StaticMeshExportFormatRequest::Obj, false)
        );
        assert_eq!(
            export_warnings(StaticMeshExportFormatRequest::Glb, true),
            vec![
                StaticMeshExportWarning::IndependentFaceSolids,
                StaticMeshExportWarning::NoTexturesAnimation,
                StaticMeshExportWarning::NoProjectSemantics,
            ]
        );
        assert_eq!(
            export_warnings(StaticMeshExportFormatRequest::Stl, false),
            vec![
                StaticMeshExportWarning::MidSurfaceOnly,
                StaticMeshExportWarning::NoThicknessSolid,
                StaticMeshExportWarning::NoTexturesAnimation,
                StaticMeshExportWarning::NoProjectSemantics,
                StaticMeshExportWarning::StlTriangleSoupFacetNormals,
                StaticMeshExportWarning::StlPrintabilityNotGuaranteed,
            ]
        );
    }

    #[test]
    fn edit_after_prepare_makes_stage_stale_without_mutating_it() {
        let state = app_state_with_current_pose();
        let request = preview_request(&state, StaticMeshExportFormatRequest::Obj);
        let source =
            capture_export_source(&state, ProjectId::new(), request).expect("capture source");
        let pending = build_pending_export(source).expect("build staged mesh");
        let original_bytes = Arc::clone(&pending.bytes);
        {
            let mut project = lock_project(&state).expect("project");
            let instance = project.instance_id;
            let project_id = project.project_id;
            let revision = project.editor.revision();
            crate::execute_command(
                &mut project,
                instance,
                project_id,
                revision,
                Command::SetLengthDisplayUnit {
                    unit: LengthDisplayUnit::Centimeter,
                },
            )
            .expect("revision-changing edit");
            assert!(!pending_is_current(&project, &pending).expect("revalidation"));
        }
        assert!(Arc::ptr_eq(&original_bytes, &pending.bytes));
        assert!(!pending.bytes.is_empty());
    }

    #[test]
    fn strict_mode_supersession_cannot_abandon_the_newer_generation() {
        let state = StaticMeshExportState::default();
        let first = ProjectId::new();
        let second = ProjectId::new();
        begin_export_generation(&state, first).unwrap();
        begin_export_generation(&state, second).unwrap();
        {
            let slot = lock_static_mesh_export(&state).unwrap();
            assert!(ensure_generation_is_current(&slot, first).is_err());
            ensure_generation_is_current(&slot, second).unwrap();
        }
        abandon_export_generation(&state, first).unwrap();
        let slot = lock_static_mesh_export(&state).unwrap();
        ensure_generation_is_current(&slot, second).unwrap();
    }

    #[test]
    fn cancel_is_idempotent_and_never_discards_a_newer_stage() {
        let app_state = app_state_with_current_pose();
        let first_request = preview_request(&app_state, StaticMeshExportFormatRequest::Obj);
        let first = Arc::new(
            build_pending_export(
                capture_export_source(&app_state, ProjectId::new(), first_request).unwrap(),
            )
            .unwrap(),
        );
        let export_state = StaticMeshExportState::default();
        {
            let mut slot = lock_static_mesh_export(&export_state).unwrap();
            slot.active_generation_id = Some(first.export_id);
            slot.pending = Some(Arc::clone(&first));
        }
        cancel_pending_export(&export_state, first.export_id).unwrap();
        cancel_pending_export(&export_state, first.export_id).unwrap();

        let second_request = preview_request(&app_state, StaticMeshExportFormatRequest::Glb);
        let second = Arc::new(
            build_pending_export(
                capture_export_source(&app_state, ProjectId::new(), second_request).unwrap(),
            )
            .unwrap(),
        );
        {
            let mut slot = lock_static_mesh_export(&export_state).unwrap();
            slot.active_generation_id = Some(second.export_id);
            slot.pending = Some(Arc::clone(&second));
        }
        cancel_pending_export(&export_state, first.export_id).unwrap();
        let slot = lock_static_mesh_export(&export_state).unwrap();
        assert_eq!(
            slot.pending.as_ref().map(|pending| pending.export_id),
            Some(second.export_id)
        );
        assert!(Arc::ptr_eq(
            slot.pending.as_ref().expect("new stage"),
            &second
        ));
    }

    #[test]
    fn repeated_save_preflight_retains_the_same_immutable_stage() {
        let app_state = app_state_with_current_pose();
        let preview_request = preview_request(&app_state, StaticMeshExportFormatRequest::Stl);
        let pending = Arc::new(
            build_pending_export(
                capture_export_source(&app_state, ProjectId::new(), preview_request).unwrap(),
            )
            .unwrap(),
        );
        let request = StaticMeshExportSaveRequest {
            export_id: pending.export_id,
            expected_project_instance_id: pending.expected_project_instance_id,
            expected_project_id: pending.expected_project_id,
            expected_revision: pending.expected_revision,
            expected_source_fingerprint: pending.source_fingerprint.to_string(),
            expected_pose_generation: pending.pose_generation.to_string(),
            warnings_acknowledged: true,
        };
        let export_state = StaticMeshExportState::default();
        {
            let mut slot = lock_static_mesh_export(&export_state).unwrap();
            slot.active_generation_id = Some(pending.export_id);
            slot.pending = Some(Arc::clone(&pending));
        }
        let slot = lock_static_mesh_export(&export_state).unwrap();
        let project = lock_project(&app_state).unwrap();
        let first = checked_pending(&slot, &project, &request, pending.pose_generation).unwrap();
        let second = checked_pending(&slot, &project, &request, pending.pose_generation).unwrap();
        assert!(Arc::ptr_eq(first, second));
        assert!(Arc::ptr_eq(&first.bytes, &pending.bytes));
    }
}
