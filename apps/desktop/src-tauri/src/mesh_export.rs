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
    collections::{BTreeMap, BTreeSet, VecDeque},
    path::Path,
    sync::{Arc, Mutex, MutexGuard},
};

use num_bigint::{BigInt, Sign};
use ori_collision::{
    SingleHingeThicknessBoundaryObservationV1, prepare_single_hinge_thickness_boundary_v1,
    prepare_tree_hinge_thickness_boundaries_v1, revalidate_single_hinge_thickness_boundary_v1,
    revalidate_tree_hinge_thickness_boundaries_v1,
};
use ori_domain::{AssetId, ProjectId, VertexId};
use ori_formats::{
    ClosedSolidTriangleRegionV1, EmbeddedBaseColorTextureV1, EmbeddedTextureMediaTypeV1,
    IndexedTriangleMeshV1, MAX_STATIC_MESH_TRIANGLES, MAX_STATIC_MESH_VERTICES,
    STATIC_MESH_SOURCE_AXIS, STATIC_MESH_SOURCE_UNIT, StaticMeshExportArtifact,
    StaticMeshExportFormat, export_dual_sided_triangle_mesh_glb,
    export_regioned_closed_solid_triangle_mesh_glb, export_static_triangle_mesh,
    validate_indexed_triangle_mesh,
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
const CLOSED_FACE_SOLIDS_GEOMETRY_PROFILE: &str =
    "authenticated_exact_coplanar_face_union_solids_v1";
const MAX_EXACT_TRIANGULATION_PREDICATES: usize = 20_000_000;
const MAX_PRINTABILITY_TRIANGLE_PAIR_CHECKS: usize = 1_000_000;
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
    printability: StaticMeshPrintabilityReport,
    warnings: Arc<[StaticMeshExportWarning]>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct StaticMeshPrintabilityReport {
    status: StaticMeshPrintabilityStatus,
    watertight: bool,
    consistently_oriented: bool,
    nonzero_volume: bool,
    no_duplicate_triangles: bool,
    no_degenerate_triangles: bool,
    conservative_self_intersection_clear: bool,
    connected_component_count: usize,
    checked_edge_count: usize,
    checked_triangle_pair_count: usize,
    limitations: Arc<[StaticMeshPrintabilityLimitation]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum StaticMeshPrintabilityStatus {
    ManifoldVerified,
    NotVerified,
    NotApplicable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum StaticMeshPrintabilityLimitation {
    FormatNotCovered,
    NoPositiveThickness,
    OpenOrNonmanifoldEdges,
    InconsistentOrientation,
    ZeroOrInvalidVolume,
    DuplicateTriangles,
    DegenerateTriangles,
    PotentialSelfIntersection,
    CheckBudgetExceeded,
    ManifoldOnlyNotPrintability,
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
    paper_back_color_rgba: [u8; 4],
    paper_front_texture: Option<ResolvedStaticMeshTexture>,
    paper_back_texture: Option<ResolvedStaticMeshTexture>,
    paper_thickness_mm: f64,
    paper_thickness_bits: u64,
    model: MaterialTreeKinematicsModel,
    pose: MaterialTreePose,
}

/// Native-only result of resolving one project asset. The browser is never
/// allowed to supply this value: a future asset repository must mint it after
/// authenticating the requested `AssetId`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum PaperTextureSide {
    Front,
    Back,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct TextureAuthorityBinding {
    project_instance_id: ProjectId,
    project_id: ProjectId,
    revision: u64,
}

#[allow(dead_code)] // Constructed once a native asset byte repository is connected.
struct ResolvedStaticMeshTexture {
    binding: TextureAuthorityBinding,
    asset_id: AssetId,
    side: PaperTextureSide,
    media_type: EmbeddedTextureMediaTypeV1,
    bytes: Vec<u8>,
}

/// Fail-closed bridge between an authenticated project texture reference and
/// the format layer's embedded-image contract.
#[allow(dead_code)] // Strict seam for the future native asset byte repository.
fn stage_authenticated_texture(
    format: StaticMeshExportFormatRequest,
    expected_asset_id: Option<AssetId>,
    expected_side: PaperTextureSide,
    expected_binding: TextureAuthorityBinding,
    resolved: Option<ResolvedStaticMeshTexture>,
    material_tex_coords: Vec<[f32; 2]>,
    mesh: IndexedTriangleMeshV1,
) -> Result<IndexedTriangleMeshV1, String> {
    let vertex_count = mesh.positions_mm.len();
    let texture = authenticate_texture(
        format,
        expected_asset_id,
        expected_side,
        expected_binding,
        resolved,
        material_tex_coords,
        vertex_count,
    )?;
    Ok(match texture {
        Some(texture) => mesh.with_base_color_texture(texture),
        None => mesh,
    })
}

fn authenticate_texture(
    format: StaticMeshExportFormatRequest,
    expected_asset_id: Option<AssetId>,
    expected_side: PaperTextureSide,
    expected_binding: TextureAuthorityBinding,
    resolved: Option<ResolvedStaticMeshTexture>,
    material_tex_coords: Vec<[f32; 2]>,
    vertex_count: usize,
) -> Result<Option<EmbeddedBaseColorTextureV1>, String> {
    let Some(expected_asset_id) = expected_asset_id else {
        return if resolved.is_none() {
            Ok(None)
        } else {
            Err(PREVIEW_FAILED_MESSAGE.to_owned())
        };
    };
    if format != StaticMeshExportFormatRequest::Glb {
        return if resolved.is_none() {
            Ok(None)
        } else {
            Err(PREVIEW_FAILED_MESSAGE.to_owned())
        };
    }
    let resolved = resolved.ok_or_else(|| PREVIEW_FAILED_MESSAGE.to_owned())?;
    if resolved.asset_id != expected_asset_id
        || resolved.side != expected_side
        || resolved.binding != expected_binding
        || material_tex_coords.len() != vertex_count
    {
        return Err(PREVIEW_FAILED_MESSAGE.to_owned());
    }
    Ok(Some(EmbeddedBaseColorTextureV1 {
        media_type: resolved.media_type,
        bytes: resolved.bytes,
        tex_coords: material_tex_coords,
    }))
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
    printability: StaticMeshPrintabilityReport,
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
    if view.graph().is_some() {
        return Err(
            "閉路姿勢は静的メッシュに書き出せません。閉路折りパネルで姿勢を確認してください。"
                .to_owned(),
        );
    }
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
        paper_back_color_rgba: {
            let color = project.editor.paper().back.color;
            [color.red, color.green, color.blue, color.alpha]
        },
        paper_front_texture: project
            .editor
            .paper()
            .front
            .texture_asset
            .map(|asset_id| {
                let asset = project
                    .texture_assets
                    .iter()
                    .find(|asset| asset.id == asset_id)
                    .ok_or_else(|| PREVIEW_FAILED_MESSAGE.to_owned())?;
                Ok::<_, String>(ResolvedStaticMeshTexture {
                    binding: TextureAuthorityBinding {
                        project_instance_id: project.instance_id,
                        project_id: project.project_id,
                        revision: project.editor.revision(),
                    },
                    asset_id,
                    side: PaperTextureSide::Front,
                    media_type: match asset.media_type {
                        ori_formats::ProjectTextureMediaTypeV1::Png => {
                            EmbeddedTextureMediaTypeV1::Png
                        }
                        ori_formats::ProjectTextureMediaTypeV1::Jpeg => {
                            EmbeddedTextureMediaTypeV1::Jpeg
                        }
                    },
                    bytes: asset.bytes.clone(),
                })
            })
            .transpose()?,
        paper_back_texture: project
            .editor
            .paper()
            .back
            .texture_asset
            .map(|asset_id| {
                let asset = project
                    .texture_assets
                    .iter()
                    .find(|asset| asset.id == asset_id)
                    .ok_or_else(|| PREVIEW_FAILED_MESSAGE.to_owned())?;
                Ok::<_, String>(ResolvedStaticMeshTexture {
                    binding: TextureAuthorityBinding {
                        project_instance_id: project.instance_id,
                        project_id: project.project_id,
                        revision: project.editor.revision(),
                    },
                    asset_id,
                    side: PaperTextureSide::Back,
                    media_type: match asset.media_type {
                        ori_formats::ProjectTextureMediaTypeV1::Png => {
                            EmbeddedTextureMediaTypeV1::Png
                        }
                        ori_formats::ProjectTextureMediaTypeV1::Jpeg => {
                            EmbeddedTextureMediaTypeV1::Jpeg
                        }
                    },
                    bytes: asset.bytes.clone(),
                })
            })
            .transpose()?,
        paper_thickness_mm: canonical_zero(paper_thickness_mm),
        paper_thickness_bits,
        model: view.model().clone(),
        pose: view.pose().clone(),
        pose_capability,
    };
    Ok(source)
}

fn build_pending_export(source: StaticMeshExportSource) -> Result<PendingStaticMeshExport, String> {
    let bound = source
        .model
        .bind_pose(&source.pose)
        .map_err(|_| PREVIEW_FAILED_MESSAGE.to_owned())?;
    let face_count = source.pose.face_ids().len();
    let hinge_unions = if source.paper_thickness_mm > 0.0 && source.model.hinges().len() > 1 {
        prepare_tree_hinge_thickness_boundaries_v1(bound, source.paper_thickness_mm)
            .map_err(|_| PREVIEW_FAILED_MESSAGE.to_owned())?
            .and_then(|capability| {
                revalidate_tree_hinge_thickness_boundaries_v1(
                    &capability,
                    bound,
                    source.paper_thickness_mm,
                )
            })
            .unwrap_or_default()
    } else if source.paper_thickness_mm > 0.0 {
        prepare_single_hinge_thickness_boundary_v1(bound, source.paper_thickness_mm)
            .map_err(|_| PREVIEW_FAILED_MESSAGE.to_owned())?
            .and_then(|capability| {
                revalidate_single_hinge_thickness_boundary_v1(
                    &capability,
                    bound,
                    source.paper_thickness_mm,
                )
            })
            .into_iter()
            .collect()
    } else {
        Vec::new()
    };
    let hinge_unions = hinge_unions
        .into_iter()
        .filter(|observation| {
            observation.left_front.map(|point| point.map(f64::to_bits))
                != observation.right_front.map(|point| point.map(f64::to_bits))
        })
        .collect();
    let (mid_surface, material_tex_coords) = build_current_pose_mid_surface_mesh_with_material_uv(
        &source.project_name,
        &source.model,
        &source.pose,
    )?;
    let expected_front_asset = source
        .paper_front_texture
        .as_ref()
        .map(|texture| texture.asset_id);
    let expected_back_asset = source
        .paper_back_texture
        .as_ref()
        .map(|texture| texture.asset_id);
    if source.format == StaticMeshExportFormatRequest::Glb
        && source.paper_thickness_mm > 0.0
        && expected_front_asset.is_some() != expected_back_asset.is_some()
    {
        return Err(PREVIEW_FAILED_MESSAGE.to_owned());
    }
    let resolved_front = if source.format == StaticMeshExportFormatRequest::Glb {
        source.paper_front_texture
    } else {
        None
    };
    let back_texture = authenticate_texture(
        source.format,
        expected_back_asset,
        PaperTextureSide::Back,
        TextureAuthorityBinding {
            project_instance_id: source.expected_project_instance_id,
            project_id: source.expected_project_id,
            revision: source.expected_revision,
        },
        if source.format == StaticMeshExportFormatRequest::Glb {
            source.paper_back_texture
        } else {
            None
        },
        material_tex_coords.clone(),
        mid_surface.positions_mm.len(),
    )?;
    let mid_surface = stage_authenticated_texture(
        source.format,
        expected_front_asset,
        PaperTextureSide::Front,
        TextureAuthorityBinding {
            project_instance_id: source.expected_project_instance_id,
            project_id: source.expected_project_id,
            revision: source.expected_revision,
        },
        resolved_front,
        material_tex_coords,
        mid_surface,
    )?;
    let (mesh, solid_regions) = if source.paper_thickness_mm > 0.0 {
        let mid_surface = weld_exact_coplanar_mid_surface(mid_surface)?;
        let solid = extrude_closed_face_solids(
            mid_surface,
            source.paper_thickness_mm,
            source.paper_front_color_rgba,
            source.paper_back_color_rgba,
            hinge_unions,
        )?;
        (solid.mesh, Some(solid.regions))
    } else {
        (
            mid_surface.with_base_color_rgba(source.paper_front_color_rgba),
            None,
        )
    };
    let back_texture = back_texture.map(|mut texture| {
        texture
            .tex_coords
            .resize(mesh.positions_mm.len(), [0.0, 0.0]);
        texture
    });
    let validated =
        validate_indexed_triangle_mesh(&mesh).map_err(|_| PREVIEW_FAILED_MESSAGE.to_owned())?;
    let printability =
        build_printability_report(source.format, source.paper_thickness_mm, &validated);
    let artifact = match (back_texture, solid_regions.as_deref()) {
        (Some(back_texture), Some(regions)) => {
            let side_color = std::array::from_fn(|index| {
                ((u16::from(source.paper_front_color_rgba[index])
                    + u16::from(source.paper_back_color_rgba[index]))
                    / 2) as u8
            });
            export_regioned_closed_solid_triangle_mesh_glb(
                &validated,
                regions,
                back_texture,
                source.paper_back_color_rgba,
                side_color,
            )
        }
        (Some(back_texture), None) => export_dual_sided_triangle_mesh_glb(
            &validated,
            back_texture,
            source.paper_back_color_rgba,
        ),
        (None, _) => export_static_triangle_mesh(source.format.exporter_format(), &validated),
    }
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
        printability,
        warnings,
    })
}

type PrintPoint = [u64; 3];

fn print_point(point: [f64; 3]) -> PrintPoint {
    point.map(|value| canonical_zero(value).to_bits())
}

fn build_printability_report(
    format: StaticMeshExportFormatRequest,
    thickness_mm: f64,
    mesh: &ori_formats::ValidatedIndexedTriangleMesh,
) -> StaticMeshPrintabilityReport {
    let applicable = matches!(
        format,
        StaticMeshExportFormatRequest::Stl | StaticMeshExportFormatRequest::Glb
    ) && thickness_mm > 0.0;
    let positions = mesh.positions_mm();
    let triangles = mesh.triangles();
    let points: Vec<PrintPoint> = positions.iter().copied().map(print_point).collect();
    let mut edge_uses: BTreeMap<(PrintPoint, PrintPoint), Vec<(usize, bool)>> = BTreeMap::new();
    let mut unique_triangles = BTreeSet::new();
    let mut degenerate_count = 0usize;
    for (triangle_index, triangle) in triangles.iter().enumerate() {
        let triangle_points = [
            points[triangle[0] as usize],
            points[triangle[1] as usize],
            points[triangle[2] as usize],
        ];
        if triangle_points[0] == triangle_points[1]
            || triangle_points[1] == triangle_points[2]
            || triangle_points[2] == triangle_points[0]
        {
            degenerate_count += 1;
        }
        let mut canonical = triangle_points;
        canonical.sort();
        unique_triangles.insert(canonical);
        for (start, end) in [
            (triangle_points[0], triangle_points[1]),
            (triangle_points[1], triangle_points[2]),
            (triangle_points[2], triangle_points[0]),
        ] {
            let (edge, forward) = if start < end {
                ((start, end), true)
            } else {
                ((end, start), false)
            };
            edge_uses
                .entry(edge)
                .or_default()
                .push((triangle_index, forward));
        }
    }
    let watertight = edge_uses.values().all(|uses| uses.len() == 2);
    let consistently_oriented = edge_uses
        .values()
        .all(|uses| uses.len() == 2 && uses[0].1 != uses[1].1);
    let no_duplicate_triangles = unique_triangles.len() == triangles.len();
    let no_degenerate_triangles = degenerate_count == 0;

    let mut adjacency = vec![Vec::new(); triangles.len()];
    for uses in edge_uses.values() {
        for left in 0..uses.len() {
            for right in left + 1..uses.len() {
                adjacency[uses[left].0].push(uses[right].0);
                adjacency[uses[right].0].push(uses[left].0);
            }
        }
    }
    let mut component = vec![usize::MAX; triangles.len()];
    let mut component_count = 0usize;
    for start in 0..triangles.len() {
        if component[start] != usize::MAX {
            continue;
        }
        component[start] = component_count;
        let mut queue = VecDeque::from([start]);
        while let Some(current) = queue.pop_front() {
            for &next in &adjacency[current] {
                if component[next] == usize::MAX {
                    component[next] = component_count;
                    queue.push_back(next);
                }
            }
        }
        component_count += 1;
    }
    let mut volumes = vec![0.0f64; component_count];
    for (index, triangle) in triangles.iter().enumerate() {
        let a = positions[triangle[0] as usize];
        let b = positions[triangle[1] as usize];
        let c = positions[triangle[2] as usize];
        volumes[component[index]] += (a[0] * (b[1] * c[2] - b[2] * c[1])
            + a[1] * (b[2] * c[0] - b[0] * c[2])
            + a[2] * (b[0] * c[1] - b[1] * c[0]))
            / 6.0;
    }
    let mut bounds_min = [f64::INFINITY; 3];
    let mut bounds_max = [f64::NEG_INFINITY; 3];
    for point in positions {
        for axis in 0..3 {
            bounds_min[axis] = bounds_min[axis].min(point[axis]);
            bounds_max[axis] = bounds_max[axis].max(point[axis]);
        }
    }
    let diagonal_squared: f64 = (0..3)
        .map(|axis| (bounds_max[axis] - bounds_min[axis]).powi(2))
        .sum();
    let volume_epsilon = diagonal_squared.sqrt().powi(3) * 1.0e-12;
    let nonzero_volume = !volumes.is_empty()
        && volumes
            .iter()
            .all(|volume| volume.is_finite() && volume.abs() > volume_epsilon);

    let mut checked_triangle_pair_count = 0usize;
    let mut conservative_clear = true;
    let mut budget_exceeded = false;
    'pairs: for left in 0..triangles.len() {
        let left_indices = triangles[left].map(|index| index as usize);
        let left_points = left_indices.map(|index| points[index]);
        for right in left + 1..triangles.len() {
            let right_indices = triangles[right].map(|index| index as usize);
            let right_points = right_indices.map(|index| points[index]);
            // Edge-adjacent triangles may meet along that edge. A single
            // shared vertex does not prove that their interiors cannot cross.
            if left_points
                .iter()
                .filter(|point| right_points.contains(point))
                .count()
                >= 2
            {
                continue;
            }
            if checked_triangle_pair_count == MAX_PRINTABILITY_TRIANGLE_PAIR_CHECKS {
                conservative_clear = false;
                budget_exceeded = true;
                break 'pairs;
            }
            checked_triangle_pair_count += 1;
            let mut boxes_overlap = true;
            for axis in 0..3 {
                let left_values = left_indices.map(|index| positions[index][axis]);
                let right_values = right_indices.map(|index| positions[index][axis]);
                let left_min = left_values.into_iter().fold(f64::INFINITY, f64::min);
                let left_max = left_values.into_iter().fold(f64::NEG_INFINITY, f64::max);
                let right_min = right_values.into_iter().fold(f64::INFINITY, f64::min);
                let right_max = right_values.into_iter().fold(f64::NEG_INFINITY, f64::max);
                if left_max < right_min || right_max < left_min {
                    boxes_overlap = false;
                    break;
                }
            }
            if boxes_overlap {
                conservative_clear = false;
            }
        }
    }
    let mut limitations = Vec::new();
    if !matches!(
        format,
        StaticMeshExportFormatRequest::Stl | StaticMeshExportFormatRequest::Glb
    ) {
        limitations.push(StaticMeshPrintabilityLimitation::FormatNotCovered);
    }
    if thickness_mm <= 0.0 {
        limitations.push(StaticMeshPrintabilityLimitation::NoPositiveThickness);
    }
    if !watertight {
        limitations.push(StaticMeshPrintabilityLimitation::OpenOrNonmanifoldEdges);
    }
    if !consistently_oriented {
        limitations.push(StaticMeshPrintabilityLimitation::InconsistentOrientation);
    }
    if !nonzero_volume {
        limitations.push(StaticMeshPrintabilityLimitation::ZeroOrInvalidVolume);
    }
    if !no_duplicate_triangles {
        limitations.push(StaticMeshPrintabilityLimitation::DuplicateTriangles);
    }
    if !no_degenerate_triangles {
        limitations.push(StaticMeshPrintabilityLimitation::DegenerateTriangles);
    }
    if !conservative_clear {
        limitations.push(if budget_exceeded {
            StaticMeshPrintabilityLimitation::CheckBudgetExceeded
        } else {
            StaticMeshPrintabilityLimitation::PotentialSelfIntersection
        });
    }
    limitations.push(StaticMeshPrintabilityLimitation::ManifoldOnlyNotPrintability);
    let verified = applicable
        && watertight
        && consistently_oriented
        && nonzero_volume
        && no_duplicate_triangles
        && no_degenerate_triangles
        && conservative_clear;
    StaticMeshPrintabilityReport {
        status: if !applicable {
            StaticMeshPrintabilityStatus::NotApplicable
        } else if verified {
            StaticMeshPrintabilityStatus::ManifoldVerified
        } else {
            StaticMeshPrintabilityStatus::NotVerified
        },
        watertight,
        consistently_oriented,
        nonzero_volume,
        no_duplicate_triangles,
        no_degenerate_triangles,
        conservative_self_intersection_clear: conservative_clear,
        connected_component_count: component_count,
        checked_edge_count: edge_uses.len(),
        checked_triangle_pair_count,
        limitations: Arc::from(limitations),
    }
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
        || (artifact.triangle_count != mesh.triangles().len()
            && !(requested == StaticMeshExportFormatRequest::Glb
                && artifact.triangle_count == mesh.triangles().len().saturating_mul(2)))
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
        printability: pending.printability.clone(),
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

pub(super) fn build_current_pose_mid_surface_mesh(
    name: &str,
    model: &MaterialTreeKinematicsModel,
    pose: &MaterialTreePose,
) -> Result<IndexedTriangleMeshV1, String> {
    build_current_pose_mid_surface_mesh_with_material_uv(name, model, pose).map(|built| built.0)
}

fn build_current_pose_mid_surface_mesh_with_material_uv(
    name: &str,
    model: &MaterialTreeKinematicsModel,
    pose: &MaterialTreePose,
) -> Result<(IndexedTriangleMeshV1, Vec<[f32; 2]>), String> {
    model
        .bind_pose(pose)
        .map_err(|_| PREVIEW_FAILED_MESSAGE.to_owned())?;
    if model.face_ids() != pose.face_ids() || pose.face_ids().is_empty() {
        return Err(PREVIEW_FAILED_MESSAGE.to_owned());
    }

    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut material_points = Vec::new();
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
            material_points.push(face_vertices[source_index].rest_2d);
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
    let material_tex_coords = normalized_material_tex_coords(&material_points)?;
    Ok((
        IndexedTriangleMeshV1::new(name, positions, normals, triangles),
        material_tex_coords,
    ))
}

fn normalized_material_tex_coords(points: &[[f64; 2]]) -> Result<Vec<[f32; 2]>, String> {
    let first = points
        .first()
        .copied()
        .ok_or_else(|| PREVIEW_FAILED_MESSAGE.to_owned())?;
    let (mut min_x, mut max_x, mut min_y, mut max_y) = (first[0], first[0], first[1], first[1]);
    for [x, y] in points.iter().copied() {
        if !x.is_finite() || !y.is_finite() {
            return Err(PREVIEW_FAILED_MESSAGE.to_owned());
        }
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_y = min_y.min(y);
        max_y = max_y.max(y);
    }
    let width = max_x - min_x;
    let height = max_y - min_y;
    if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
        return Err(PREVIEW_FAILED_MESSAGE.to_owned());
    }
    points
        .iter()
        .map(|[x, y]| {
            let u = ((x - min_x) / width) as f32;
            let v = ((y - min_y) / height) as f32;
            if u.is_finite() && v.is_finite() {
                Ok([u, v])
            } else {
                Err(PREVIEW_FAILED_MESSAGE.to_owned())
            }
        })
        .collect()
}

struct ExtrudedClosedFaceSolids {
    mesh: IndexedTriangleMeshV1,
    regions: Vec<ClosedSolidTriangleRegionV1>,
}

fn weld_exact_coplanar_mid_surface(
    mesh: IndexedTriangleMeshV1,
) -> Result<IndexedTriangleMeshV1, String> {
    let texture = mesh.base_color_texture.as_ref();
    let mut registry = BTreeMap::new();
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut colors = Vec::new();
    let mut tex_coords = Vec::new();
    let mut remap = Vec::with_capacity(mesh.positions_mm.len());
    for index in 0..mesh.positions_mm.len() {
        let position = mesh.positions_mm[index];
        let normal = mesh.normals[index];
        let uv = texture.map(|value| value.tex_coords[index]);
        let key = (
            position.map(f64::to_bits),
            normal.map(f64::to_bits),
            uv.map(|value| value.map(f32::to_bits)),
        );
        let mapped = if let Some(existing) = registry.get(&key).copied() {
            existing
        } else {
            let next =
                u32::try_from(positions.len()).map_err(|_| PREVIEW_FAILED_MESSAGE.to_owned())?;
            registry.insert(key, next);
            positions.push(position);
            normals.push(normal);
            if !mesh.vertex_colors_rgba.is_empty() {
                colors.push(mesh.vertex_colors_rgba[index]);
            }
            if let Some(uv) = uv {
                tex_coords.push(uv);
            }
            next
        };
        remap.push(mapped);
    }
    let triangles = mesh
        .triangles
        .iter()
        .map(|triangle| triangle.map(|index| remap[index as usize]))
        .collect::<Vec<_>>();
    let mut unique_triangles = std::collections::BTreeSet::new();
    for triangle in &triangles {
        let mut canonical = *triangle;
        canonical.sort_unstable();
        if !unique_triangles.insert(canonical) {
            return Err(PREVIEW_FAILED_MESSAGE.to_owned());
        }
    }
    let mut welded = IndexedTriangleMeshV1::new(mesh.name, positions, normals, triangles)
        .with_base_color_rgba(mesh.base_color_rgba);
    if !colors.is_empty() {
        welded = welded.with_vertex_colors_rgba(colors);
    }
    if let Some(texture) = mesh.base_color_texture {
        welded = welded.with_base_color_texture(EmbeddedBaseColorTextureV1 {
            tex_coords,
            ..texture
        });
    }
    Ok(welded)
}

fn extrude_closed_face_solids(
    mesh: IndexedTriangleMeshV1,
    thickness_mm: f64,
    front_color: [u8; 4],
    back_color: [u8; 4],
    hinge_unions: Vec<SingleHingeThicknessBoundaryObservationV1>,
) -> Result<ExtrudedClosedFaceSolids, String> {
    if !thickness_mm.is_finite() || thickness_mm <= 0.0 {
        return Err(PREVIEW_FAILED_MESSAGE.to_owned());
    }
    let source_vertex_count = mesh.positions_mm.len();
    let half = thickness_mm / 2.0;
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut colors = Vec::new();
    positions
        .try_reserve(source_vertex_count.saturating_mul(2))
        .map_err(|_| PREVIEW_FAILED_MESSAGE.to_owned())?;
    normals
        .try_reserve(source_vertex_count.saturating_mul(2))
        .map_err(|_| PREVIEW_FAILED_MESSAGE.to_owned())?;
    colors
        .try_reserve(source_vertex_count.saturating_mul(2))
        .map_err(|_| PREVIEW_FAILED_MESSAGE.to_owned())?;
    for (position, normal) in mesh.positions_mm.iter().zip(&mesh.normals) {
        positions.push([
            position[0] + normal[0] * half,
            position[1] + normal[1] * half,
            position[2] + normal[2] * half,
        ]);
        normals.push(*normal);
        colors.push(front_color);
    }
    for (position, normal) in mesh.positions_mm.iter().zip(&mesh.normals) {
        positions.push([
            position[0] - normal[0] * half,
            position[1] - normal[1] * half,
            position[2] - normal[2] * half,
        ]);
        normals.push(normal.map(|component| -component));
        colors.push(back_color);
    }
    let bottom_offset =
        u32::try_from(source_vertex_count).map_err(|_| PREVIEW_FAILED_MESSAGE.to_owned())?;
    let mut triangles = Vec::new();
    let mut regions = Vec::new();
    let union_rails = hinge_unions
        .into_iter()
        .map(|observation| {
            (
                observation.left_front.map(kinematics_array_to_export),
                observation.right_front.map(kinematics_array_to_export),
                observation.left_back.map(kinematics_array_to_export),
                observation.right_back.map(kinematics_array_to_export),
            )
        })
        .collect::<Vec<_>>();
    let mut union_edges = vec![[None, None]; union_rails.len()];
    let mut edges = BTreeMap::<(u32, u32), (usize, u32, u32)>::new();
    for triangle in &mesh.triangles {
        triangles.push(*triangle);
        regions.push(ClosedSolidTriangleRegionV1::FrontCap);
        triangles.push([
            triangle[2] + bottom_offset,
            triangle[1] + bottom_offset,
            triangle[0] + bottom_offset,
        ]);
        regions.push(ClosedSolidTriangleRegionV1::BackCap);
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
        let mut matched = None;
        for (union_index, (left, right, _, _)) in union_rails.iter().copied().enumerate() {
            for (side, rail) in [left, right].into_iter().enumerate() {
                if unordered_segment_bits_eq([top_a, top_b], rail) {
                    if matched.is_some()
                        || union_edges[union_index][side]
                            .replace((start, end))
                            .is_some()
                    {
                        return Err(PREVIEW_FAILED_MESSAGE.to_owned());
                    }
                    matched = Some(());
                }
            }
        }
        if matched.is_some() {
            continue;
        }
        positions.extend([top_a, top_b, bottom_b, bottom_a]);
        normals.extend([side_normal; 4]);
        let edge_color = edge_color(front_color, back_color);
        colors.extend([edge_color; 4]);
        triangles.push([base, base + 1, base + 2]);
        regions.push(ClosedSolidTriangleRegionV1::SideWall);
        triangles.push([base, base + 2, base + 3]);
        regions.push(ClosedSolidTriangleRegionV1::SideWall);
    }
    for edges in union_edges {
        let [Some((left_start, left_end)), Some((right_start, right_end))] = edges else {
            return Err(PREVIEW_FAILED_MESSAGE.to_owned());
        };
        let left = [left_start, left_end];
        let right =
            align_segment_indices(&positions, [right_start, right_end], [left_start, left_end]);
        append_bridge_quad(
            &mut positions,
            &mut normals,
            &mut colors,
            &mut triangles,
            &mut regions,
            left,
            right,
            edge_color(front_color, back_color),
        )?;
        append_bridge_quad(
            &mut positions,
            &mut normals,
            &mut colors,
            &mut triangles,
            &mut regions,
            [left[0], right[0]],
            [left[0] + bottom_offset, right[0] + bottom_offset],
            edge_color(front_color, back_color),
        )?;
        append_bridge_quad(
            &mut positions,
            &mut normals,
            &mut colors,
            &mut triangles,
            &mut regions,
            [right[1], left[1]],
            [right[1] + bottom_offset, left[1] + bottom_offset],
            edge_color(front_color, back_color),
        )?;
        append_bridge_quad(
            &mut positions,
            &mut normals,
            &mut colors,
            &mut triangles,
            &mut regions,
            [left[1] + bottom_offset, left[0] + bottom_offset],
            [right[1] + bottom_offset, right[0] + bottom_offset],
            edge_color(front_color, back_color),
        )?;
    }
    if positions.len() > MAX_STATIC_MESH_VERTICES || triangles.len() > MAX_STATIC_MESH_TRIANGLES {
        return Err(PREVIEW_FAILED_MESSAGE.to_owned());
    }
    let mut solid = IndexedTriangleMeshV1::new(mesh.name, positions, normals, triangles)
        .with_vertex_colors_rgba(colors);
    if let Some(mut texture) = mesh.base_color_texture {
        let source_uvs = texture.tex_coords.clone();
        texture.tex_coords.extend(source_uvs);
        texture
            .tex_coords
            .resize(solid.positions_mm.len(), [0.0, 0.0]);
        solid = solid.with_base_color_texture(texture);
    }
    validate_watertight_triangle_geometry(&solid)?;
    // The exact hinge capability proves the local corridor. The remaining
    // conservative scan rejects any disjoint triangle pair whose AABBs overlap.
    validate_bounded_non_self_intersecting_volume(&solid, &union_rails)?;
    Ok(ExtrudedClosedFaceSolids {
        mesh: solid,
        regions,
    })
}

fn kinematics_array_to_export(point: [f64; 3]) -> [f64; 3] {
    [point[0], -point[2], point[1]]
}

fn unordered_segment_bits_eq(left: [[f64; 3]; 2], right: [[f64; 3]; 2]) -> bool {
    let bits = |point: [f64; 3]| point.map(f64::to_bits);
    (bits(left[0]) == bits(right[0]) && bits(left[1]) == bits(right[1]))
        || (bits(left[0]) == bits(right[1]) && bits(left[1]) == bits(right[0]))
}

fn align_segment_indices(
    positions: &[[f64; 3]],
    candidate: [u32; 2],
    reference: [u32; 2],
) -> [u32; 2] {
    let distance_squared = |a: u32, b: u32| {
        positions[a as usize]
            .iter()
            .zip(positions[b as usize])
            .map(|(left, right)| (left - right) * (left - right))
            .sum::<f64>()
    };
    if distance_squared(candidate[0], reference[0]) <= distance_squared(candidate[1], reference[0])
    {
        candidate
    } else {
        [candidate[1], candidate[0]]
    }
}

fn edge_color(front: [u8; 4], back: [u8; 4]) -> [u8; 4] {
    std::array::from_fn(|index| {
        let total = u16::from(front[index]) + u16::from(back[index]);
        u8::try_from(total / 2).expect("the average of two u8 values is a u8")
    })
}

#[allow(clippy::too_many_arguments)]
fn append_bridge_quad(
    positions: &mut Vec<[f64; 3]>,
    normals: &mut Vec<[f64; 3]>,
    colors: &mut Vec<[u8; 4]>,
    triangles: &mut Vec<[u32; 3]>,
    regions: &mut Vec<ClosedSolidTriangleRegionV1>,
    first: [u32; 2],
    second: [u32; 2],
    color: [u8; 4],
) -> Result<(), String> {
    let points = [
        positions[first[0] as usize],
        positions[first[1] as usize],
        positions[second[1] as usize],
        positions[second[0] as usize],
    ];
    let normal = triangle_normal([points[0], points[1], points[2]])
        .ok_or_else(|| PREVIEW_FAILED_MESSAGE.to_owned())?;
    let base = u32::try_from(positions.len()).map_err(|_| PREVIEW_FAILED_MESSAGE.to_owned())?;
    positions.extend(points);
    normals.extend([normal; 4]);
    colors.extend([color; 4]);
    triangles.extend([[base, base + 1, base + 2], [base, base + 2, base + 3]]);
    regions.extend([ClosedSolidTriangleRegionV1::SideWall; 2]);
    Ok(())
}

fn validate_watertight_triangle_geometry(mesh: &IndexedTriangleMeshV1) -> Result<(), String> {
    let mut edges = BTreeMap::<([u64; 3], [u64; 3]), usize>::new();
    for triangle in &mesh.triangles {
        for (start, end) in [
            (triangle[0], triangle[1]),
            (triangle[1], triangle[2]),
            (triangle[2], triangle[0]),
        ] {
            let a = mesh.positions_mm[start as usize].map(f64::to_bits);
            let b = mesh.positions_mm[end as usize].map(f64::to_bits);
            let key = if a <= b { (a, b) } else { (b, a) };
            *edges.entry(key).or_default() += 1;
        }
    }
    if edges.is_empty() || edges.values().any(|incidence| *incidence != 2) {
        return Err(PREVIEW_FAILED_MESSAGE.to_owned());
    }
    Ok(())
}

fn validate_bounded_non_self_intersecting_volume(
    mesh: &IndexedTriangleMeshV1,
    authenticated_hinge_rails: &[([[f64; 3]; 2], [[f64; 3]; 2], [[f64; 3]; 2], [[f64; 3]; 2])],
) -> Result<(), String> {
    let mut signed_six_volume = 0.0;
    for triangle in &mesh.triangles {
        let [a, b, c] = triangle.map(|index| mesh.positions_mm[index as usize]);
        let cross = [
            b[1] * c[2] - b[2] * c[1],
            b[2] * c[0] - b[0] * c[2],
            b[0] * c[1] - b[1] * c[0],
        ];
        signed_six_volume += a[0] * cross[0] + a[1] * cross[1] + a[2] * cross[2];
    }
    if !signed_six_volume.is_finite() || signed_six_volume == 0.0 {
        return Err(PREVIEW_FAILED_MESSAGE.to_owned());
    }
    for first in 0..mesh.triangles.len() {
        let first_points = mesh.triangles[first].map(|index| mesh.positions_mm[index as usize]);
        for second in first + 1..mesh.triangles.len() {
            let second_points =
                mesh.triangles[second].map(|index| mesh.positions_mm[index as usize]);
            let touches_authenticated_rail = |triangle| {
                authenticated_hinge_rails.iter().any(
                    |(left_front, right_front, left_back, right_back)| {
                        [*left_front, *right_front, *left_back, *right_back]
                            .into_iter()
                            .any(|rail| triangle_touches_segment(triangle, rail))
                    },
                )
            };
            if touches_authenticated_rail(first_points) && touches_authenticated_rail(second_points)
            {
                continue;
            }
            if authenticated_hinge_rails.iter().any(
                |(left_front, right_front, left_back, right_back)| {
                    let left = [*left_front, *left_back];
                    let right = [*right_front, *right_back];
                    left.into_iter().any(|left| {
                        right.into_iter().any(|right| {
                            triangle_touches_segment(first_points, left)
                                && triangle_touches_segment(second_points, right)
                                || triangle_touches_segment(first_points, right)
                                    && triangle_touches_segment(second_points, left)
                        })
                    })
                },
            ) {
                continue;
            }
            if first_points.iter().any(|left| {
                second_points
                    .iter()
                    .any(|right| left.map(f64::to_bits) == right.map(f64::to_bits))
            }) {
                continue;
            }
            let overlaps = (0..3).all(|axis| {
                let first_min = first_points
                    .iter()
                    .map(|point| point[axis])
                    .fold(f64::INFINITY, f64::min);
                let first_max = first_points
                    .iter()
                    .map(|point| point[axis])
                    .fold(f64::NEG_INFINITY, f64::max);
                let second_min = second_points
                    .iter()
                    .map(|point| point[axis])
                    .fold(f64::INFINITY, f64::min);
                let second_max = second_points
                    .iter()
                    .map(|point| point[axis])
                    .fold(f64::NEG_INFINITY, f64::max);
                first_min <= second_max && second_min <= first_max
            });
            if overlaps && !triangles_proven_plane_separated(first_points, second_points) {
                return Err(PREVIEW_FAILED_MESSAGE.to_owned());
            }
        }
    }
    Ok(())
}

fn triangles_proven_plane_separated(first: [[f64; 3]; 3], second: [[f64; 3]; 3]) -> bool {
    let separated = |plane: [[f64; 3]; 3], points: [[f64; 3]; 3]| {
        let u: [f64; 3] = std::array::from_fn(|axis| plane[1][axis] - plane[0][axis]);
        let v: [f64; 3] = std::array::from_fn(|axis| plane[2][axis] - plane[0][axis]);
        let normal = [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ];
        let signs = points.map(|point| {
            (0..3)
                .map(|axis| normal[axis] * (point[axis] - plane[0][axis]))
                .sum::<f64>()
        });
        let finite = signs.iter().all(|value| value.is_finite());
        finite
            && (signs.iter().all(|value| *value >= 0.0) && signs.iter().any(|value| *value > 0.0)
                || signs.iter().all(|value| *value <= 0.0)
                    && signs.iter().any(|value| *value < 0.0))
    };
    separated(first, second) || separated(second, first)
}

fn triangle_touches_segment(triangle: [[f64; 3]; 3], segment: [[f64; 3]; 2]) -> bool {
    segment.iter().any(|endpoint| {
        triangle
            .iter()
            .any(|point| point.map(f64::to_bits) == endpoint.map(f64::to_bits))
    })
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

    fn texture_stage_mesh() -> IndexedTriangleMeshV1 {
        IndexedTriangleMeshV1::new(
            "texture-stage",
            vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            vec![[0.0, 0.0, 1.0]; 3],
            vec![[0, 1, 2]],
        )
    }

    fn validated_tetrahedron() -> ori_formats::ValidatedIndexedTriangleMesh {
        validate_indexed_triangle_mesh(&IndexedTriangleMeshV1::new(
            "printability-tetrahedron",
            vec![
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, 0.0, 1.0],
            ],
            vec![[0.0, 0.0, 1.0]; 4],
            vec![[0, 2, 1], [0, 1, 3], [1, 2, 3], [2, 0, 3]],
        ))
        .expect("valid tetrahedron")
    }

    #[test]
    fn printability_report_verifies_only_bounded_positive_thickness_stl_glb() {
        let mesh = validated_tetrahedron();
        let stl = build_printability_report(StaticMeshExportFormatRequest::Stl, 0.1, &mesh);
        assert_eq!(stl.status, StaticMeshPrintabilityStatus::ManifoldVerified);
        assert!(stl.watertight);
        assert!(stl.consistently_oriented);
        assert!(stl.nonzero_volume);
        assert_eq!(stl.connected_component_count, 1);

        let obj = build_printability_report(StaticMeshExportFormatRequest::Obj, 0.1, &mesh);
        assert_eq!(obj.status, StaticMeshPrintabilityStatus::NotApplicable);
        assert!(
            obj.limitations
                .contains(&StaticMeshPrintabilityLimitation::FormatNotCovered)
        );
        let zero = build_printability_report(StaticMeshExportFormatRequest::Glb, 0.0, &mesh);
        assert_eq!(zero.status, StaticMeshPrintabilityStatus::NotApplicable);
    }

    #[test]
    fn printability_report_fails_closed_for_open_mesh() {
        let open = validate_indexed_triangle_mesh(&texture_stage_mesh()).expect("valid open mesh");
        let report = build_printability_report(StaticMeshExportFormatRequest::Stl, 0.1, &open);
        assert_eq!(report.status, StaticMeshPrintabilityStatus::NotVerified);
        assert!(!report.watertight);
        assert!(!report.consistently_oriented);
        assert!(!report.nonzero_volume);
    }

    #[test]
    fn authenticated_texture_stage_requires_native_asset_identity_and_glb() {
        let expected = AssetId::new();
        let foreign = AssetId::new();
        let binding = TextureAuthorityBinding {
            project_instance_id: ProjectId::new(),
            project_id: ProjectId::new(),
            revision: 7,
        };
        let resolved = |asset_id| ResolvedStaticMeshTexture {
            binding,
            asset_id,
            side: PaperTextureSide::Front,
            media_type: EmbeddedTextureMediaTypeV1::Png,
            bytes: b"\x89PNG\r\n\x1a\nnative".to_vec(),
        };
        let uvs = vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]];

        assert!(
            stage_authenticated_texture(
                StaticMeshExportFormatRequest::Glb,
                Some(expected),
                PaperTextureSide::Front,
                binding,
                Some(resolved(foreign)),
                uvs.clone(),
                texture_stage_mesh(),
            )
            .is_err()
        );
        assert!(
            stage_authenticated_texture(
                StaticMeshExportFormatRequest::Obj,
                Some(expected),
                PaperTextureSide::Front,
                binding,
                Some(resolved(expected)),
                uvs.clone(),
                texture_stage_mesh(),
            )
            .is_err()
        );
        let staged = stage_authenticated_texture(
            StaticMeshExportFormatRequest::Glb,
            Some(expected),
            PaperTextureSide::Front,
            binding,
            Some(resolved(expected)),
            uvs.clone(),
            texture_stage_mesh(),
        )
        .unwrap();
        let texture = staged.base_color_texture.unwrap();
        assert_eq!(texture.tex_coords, uvs);
        assert_eq!(texture.bytes, b"\x89PNG\r\n\x1a\nnative");

        let back = ResolvedStaticMeshTexture {
            side: PaperTextureSide::Back,
            ..resolved(expected)
        };
        let authenticated_back = authenticate_texture(
            StaticMeshExportFormatRequest::Glb,
            Some(expected),
            PaperTextureSide::Back,
            binding,
            Some(back),
            vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
            3,
        )
        .expect("authenticated back texture")
        .expect("present back texture");
        assert_eq!(authenticated_back.bytes, b"\x89PNG\r\n\x1a\nnative");

        let stale = TextureAuthorityBinding {
            revision: 8,
            ..binding
        };
        assert!(
            stage_authenticated_texture(
                StaticMeshExportFormatRequest::Glb,
                Some(expected),
                PaperTextureSide::Front,
                stale,
                Some(resolved(expected)),
                vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
                texture_stage_mesh(),
            )
            .is_err()
        );
    }

    #[test]
    fn material_texture_coordinates_are_bounded_and_reject_degenerate_bounds() {
        assert_eq!(
            normalized_material_tex_coords(&[[2.0, -4.0], [6.0, -4.0], [2.0, 8.0]]).unwrap(),
            [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]]
        );
        assert!(normalized_material_tex_coords(&[[1.0, 1.0], [1.0, 2.0]]).is_err());
        assert!(normalized_material_tex_coords(&[[0.0, 0.0], [f64::NAN, 1.0]]).is_err());
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

    fn app_state_with_dual_textures(thickness_mm: f64) -> AppState {
        let mut project = crate::initial_project_state();
        let front = AssetId::new();
        let back = AssetId::new();
        project.texture_assets = vec![
            ori_formats::ProjectTextureAssetV1 {
                id: front,
                media_type: ori_formats::ProjectTextureMediaTypeV1::Png,
                bytes: b"\x89PNG\r\n\x1a\nfront".to_vec(),
            },
            ori_formats::ProjectTextureAssetV1 {
                id: back,
                media_type: ori_formats::ProjectTextureMediaTypeV1::Jpeg,
                bytes: vec![0xff, 0xd8, b'b', b'a', b'c', b'k', 0xff, 0xd9],
            },
        ];
        let instance = project.instance_id;
        let project_id = project.project_id;
        let paper = project.editor.paper().clone();
        crate::execute_command(
            &mut project,
            instance,
            project_id,
            0,
            Command::UpdatePaperProperties {
                thickness_mm,
                front_color: paper.front.color,
                back_color: paper.back.color,
                front_texture_asset: Some(front),
                back_texture_asset: Some(back),
                cutting_allowed: paper.cutting_allowed,
            },
        )
        .expect("select dual textures");
        let authority = project.applied_pose_authority.clone();
        let captured = authority
            .capture_request(
                &project,
                NativePoseRequest {
                    expected_project_instance_id: instance,
                    expected_project_id: project_id,
                    expected_revision: 1,
                    fixed_face_id: None,
                    complete_hinge_angles: Vec::new(),
                },
            )
            .expect("capture textured pose");
        let prepared = captured.prepare().expect("prepare textured pose");
        authority
            .commit_prepared(&mut project, prepared)
            .expect("commit textured pose");
        AppState::new(project)
    }

    #[test]
    fn zero_thickness_glb_carries_authenticated_front_and_back_assets() {
        let state = app_state_with_dual_textures(0.0);
        let request = preview_request(&state, StaticMeshExportFormatRequest::Glb);
        let source =
            capture_export_source(&state, ProjectId::new(), request).expect("capture source");
        let pending = build_pending_export(source).expect("dual-sided GLB");
        assert_eq!(&pending.bytes[0..4], b"glTF");
        let json_length = u32::from_le_bytes(pending.bytes[12..16].try_into().unwrap()) as usize;
        let json: serde_json::Value =
            serde_json::from_slice(&pending.bytes[20..20 + json_length]).expect("GLB JSON");
        assert_eq!(json["meshes"][0]["primitives"].as_array().unwrap().len(), 2);
        assert_eq!(json["materials"].as_array().unwrap().len(), 2);
        assert_eq!(json["images"].as_array().unwrap().len(), 2);
        assert_eq!(json["images"][0]["mimeType"], "image/png");
        assert_eq!(json["images"][1]["mimeType"], "image/jpeg");
    }

    #[test]
    fn positive_thickness_glb_separates_front_back_and_untextured_side() {
        let state = app_state_with_dual_textures(0.2);
        let request = preview_request(&state, StaticMeshExportFormatRequest::Glb);
        let source =
            capture_export_source(&state, ProjectId::new(), request).expect("capture source");
        let pending = build_pending_export(source).expect("regioned solid GLB");
        let json_length = u32::from_le_bytes(pending.bytes[12..16].try_into().unwrap()) as usize;
        let json: serde_json::Value =
            serde_json::from_slice(&pending.bytes[20..20 + json_length]).expect("GLB JSON");
        let primitives = json["meshes"][0]["primitives"].as_array().unwrap();
        assert_eq!(primitives.len(), 3);
        assert!(primitives[0]["attributes"].get("TEXCOORD_0").is_some());
        assert!(primitives[1]["attributes"].get("TEXCOORD_0").is_some());
        assert!(primitives[2]["attributes"].get("TEXCOORD_0").is_none());
        assert_eq!(json["materials"].as_array().unwrap().len(), 3);
        assert_eq!(json["images"].as_array().unwrap().len(), 2);
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
    fn closed_graph_pose_mesh_export_fails_without_panicking_or_poisoning_project() {
        let (mut project, hinges) = crate::applied_pose::tests::four_vertex_cycle_project();
        crate::applied_pose::tests::install_flat_graph_pose_authority(&mut project, hinges);
        let state = AppState::new(project);
        let request = preview_request(&state, StaticMeshExportFormatRequest::Glb);

        let error = match capture_export_source(&state, ProjectId::new(), request) {
            Ok(_) => panic!("closed graph export must fail closed"),
            Err(error) => error,
        };
        assert!(error.contains("閉路姿勢"), "{error}");
        assert!(
            lock_project(&state).is_ok(),
            "project mutex must remain usable"
        );
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
        let mid_vertex_count = mid.positions_mm.len();
        let solid =
            extrude_closed_face_solids(mid, 0.1, [255, 0, 0, 255], [0, 0, 255, 255], vec![])
                .expect("solid extrusion");
        assert_eq!(solid.regions.len(), 12);
        assert_eq!(
            solid
                .regions
                .iter()
                .filter(|region| **region == ClosedSolidTriangleRegionV1::FrontCap)
                .count(),
            2
        );
        assert_eq!(
            solid
                .regions
                .iter()
                .filter(|region| **region == ClosedSolidTriangleRegionV1::BackCap)
                .count(),
            2
        );
        let solid = solid.mesh;
        assert_eq!(solid.positions_mm.len(), 24);
        assert_eq!(solid.triangles.len(), 12);
        assert_eq!(solid.vertex_colors_rgba.len(), solid.positions_mm.len());
        assert_eq!(solid.vertex_colors_rgba[0], [255, 0, 0, 255]);
        assert_eq!(solid.vertex_colors_rgba[mid_vertex_count], [0, 0, 255, 255]);
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
    fn exact_coplanar_adjacent_faces_weld_and_remove_the_internal_wall() {
        let duplicated = IndexedTriangleMeshV1::new(
            "adjacent",
            vec![
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [1.0, 0.0, 0.0],
                [1.0, 1.0, 0.0],
                [0.0, 1.0, 0.0],
            ],
            vec![[0.0, 0.0, 1.0]; 6],
            vec![[0, 1, 2], [3, 4, 5]],
        );
        let welded = weld_exact_coplanar_mid_surface(duplicated).expect("exact weld");
        assert_eq!(welded.positions_mm.len(), 4);
        let solid = extrude_closed_face_solids(
            welded,
            0.2,
            [255, 255, 255, 255],
            [240, 240, 240, 255],
            vec![],
        )
        .expect("watertight union");
        assert_eq!(solid.mesh.triangles.len(), 12);
        assert_eq!(
            solid
                .regions
                .iter()
                .filter(|region| **region == ClosedSolidTriangleRegionV1::SideWall)
                .count(),
            8
        );
        validate_watertight_triangle_geometry(&solid.mesh).expect("manifold");
        let validated = validate_indexed_triangle_mesh(&solid.mesh).expect("validated union mesh");
        let stl = export_static_triangle_mesh(StaticMeshExportFormat::BinaryStl, &validated)
            .expect("independently verified STL");
        assert_eq!(stl.triangle_count, 12);
    }

    #[test]
    fn coincident_duplicate_faces_fail_watertight_union_closed() {
        let duplicate = IndexedTriangleMeshV1::new(
            "overlap",
            vec![
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
            ],
            vec![[0.0, 0.0, 1.0]; 6],
            vec![[0, 1, 2], [3, 4, 5]],
        );
        assert!(weld_exact_coplanar_mid_surface(duplicate).is_err());
    }

    #[test]
    fn certified_nonplanar_hinge_replaces_two_internal_walls_with_bridges() {
        let sine = 0.5_f64;
        let cosine = (1.0_f64 - sine * sine).sqrt();
        let mesh = IndexedTriangleMeshV1::new(
            "certified-single-hinge",
            vec![
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 0.0, 0.0],
                [0.0, cosine, sine],
            ],
            vec![
                [0.0, 0.0, 1.0],
                [0.0, 0.0, 1.0],
                [0.0, 0.0, 1.0],
                [0.0, -sine, cosine],
                [0.0, -sine, cosine],
                [0.0, -sine, cosine],
            ],
            vec![[0, 1, 2], [3, 4, 5]],
        );
        let half = 0.1;
        let to_kinematics = |point: [f64; 3]| [point[0], point[2], -point[1]];
        let observation = SingleHingeThicknessBoundaryObservationV1 {
            hinge: ori_domain::EdgeId::new(),
            left_face: ori_domain::FaceId::new(),
            right_face: ori_domain::FaceId::new(),
            endpoint_vertices: [ori_domain::VertexId::new(), ori_domain::VertexId::new()],
            left_front: [
                to_kinematics([0.0, 0.0, half]),
                to_kinematics([1.0, 0.0, half]),
            ],
            left_back: [
                to_kinematics([0.0, 0.0, -half]),
                to_kinematics([1.0, 0.0, -half]),
            ],
            right_front: [
                to_kinematics([0.0, -sine * half, cosine * half]),
                to_kinematics([1.0, -sine * half, cosine * half]),
            ],
            right_back: [
                to_kinematics([0.0, sine * half, -cosine * half]),
                to_kinematics([1.0, sine * half, -cosine * half]),
            ],
        };
        let solid = extrude_closed_face_solids(
            mesh,
            0.2,
            [255, 255, 255, 255],
            [240, 240, 240, 255],
            vec![observation],
        )
        .expect("certified hinge union");
        validate_watertight_triangle_geometry(&solid.mesh).expect("watertight bridge");
        validate_bounded_non_self_intersecting_volume(
            &solid.mesh,
            &[(
                observation.left_front.map(kinematics_array_to_export),
                observation.right_front.map(kinematics_array_to_export),
                observation.left_back.map(kinematics_array_to_export),
                observation.right_back.map(kinematics_array_to_export),
            )],
        )
        .expect("bounded outer shell");
        let validated = validate_indexed_triangle_mesh(&solid.mesh).expect("validated mesh");
        for format in [
            StaticMeshExportFormat::BinaryStl,
            StaticMeshExportFormat::Glb20,
        ] {
            export_static_triangle_mesh(format, &validated).expect("independent reader");
        }
    }

    #[test]
    fn certified_two_hinge_tree_exports_watertight_stl_and_glb() {
        let sine = 0.5_f64;
        let cosine = (1.0_f64 - sine * sine).sqrt();
        let mesh = IndexedTriangleMeshV1::new(
            "certified-two-hinge-tree",
            vec![
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 0.0, 0.0],
                [0.0, cosine, sine],
                [0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [-cosine, 0.0, sine],
            ],
            vec![
                [0.0, 0.0, 1.0],
                [0.0, 0.0, 1.0],
                [0.0, 0.0, 1.0],
                [0.0, -sine, cosine],
                [0.0, -sine, cosine],
                [0.0, -sine, cosine],
                [sine, 0.0, cosine],
                [sine, 0.0, cosine],
                [sine, 0.0, cosine],
            ],
            vec![[0, 1, 2], [3, 4, 5], [6, 7, 8]],
        );
        let half = 0.1;
        let origin = ori_domain::VertexId::new();
        let x = ori_domain::VertexId::new();
        let y = ori_domain::VertexId::new();
        let to_kinematics = |point: [f64; 3]| [point[0], point[2], -point[1]];
        let observation = |endpoint_vertices: [ori_domain::VertexId; 2],
                           left: [[f64; 3]; 2],
                           right: [[f64; 3]; 2],
                           right_normal: [f64; 3]| {
            let offset = |rail: [[f64; 3]; 2], normal: [f64; 3], distance: f64| {
                rail.map(|point| {
                    to_kinematics(std::array::from_fn(|axis| {
                        point[axis] + normal[axis] * distance
                    }))
                })
            };
            SingleHingeThicknessBoundaryObservationV1 {
                hinge: ori_domain::EdgeId::new(),
                left_face: ori_domain::FaceId::new(),
                right_face: ori_domain::FaceId::new(),
                endpoint_vertices,
                left_front: offset(left, [0.0, 0.0, 1.0], half),
                left_back: offset(left, [0.0, 0.0, 1.0], -half),
                right_front: offset(right, right_normal, half),
                right_back: offset(right, right_normal, -half),
            }
        };
        let observations = vec![
            observation(
                [origin, x],
                [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]],
                [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]],
                [0.0, -sine, cosine],
            ),
            observation(
                [origin, y],
                [[0.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
                [[0.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
                [sine, 0.0, cosine],
            ),
        ];
        let solid = extrude_closed_face_solids(
            mesh,
            0.2,
            [255, 255, 255, 255],
            [240, 240, 240, 255],
            observations,
        )
        .expect("certified tree union");
        validate_watertight_triangle_geometry(&solid.mesh).expect("watertight tree shell");
        let validated = validate_indexed_triangle_mesh(&solid.mesh).expect("validated tree mesh");
        for format in [
            StaticMeshExportFormat::BinaryStl,
            StaticMeshExportFormat::Glb20,
        ] {
            export_static_triangle_mesh(format, &validated).expect("independent tree reader");
        }
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
