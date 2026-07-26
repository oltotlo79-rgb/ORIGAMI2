//! Native SVG straight-line crease-pattern import boundary.
//!
//! This module owns bounded file ingestion, one-slot opaque preview staging,
//! generation-bound settings validation, explicit confirmations, conversion,
//! cancellation, and atomic project replacement. General project identity and
//! persistence mechanics remain in the crate root.

use super::crease_pattern_boundary_support::validate_active_edge_containment;
use super::import_command_support::validate_import_scale;
use super::*;

pub(super) const MAX_SVG_IMPORT_FILE_SIZE: u64 = 16 * 1024 * 1024;
pub(super) const MAX_SVG_IMPORT_PREVIEW_EDGES: usize = 5_000;
pub(super) const SVG_IMPORT_FILE_LABEL: &str = "選択したSVGファイル";
const SVG_IMPORT_FALLBACK_NAME: &str = "SVGインポート";
pub(super) const SVG_FILE_OPEN_FAILED_MESSAGE: &str = "選択されたSVGファイルを開けませんでした。";
pub(super) const SVG_FILE_INSPECTION_FAILED_MESSAGE: &str =
    "選択されたSVGファイルのサイズを確認できませんでした。";
pub(super) const SVG_FILE_TOO_LARGE_MESSAGE: &str =
    "選択されたSVGファイルはサイズ上限を超えています。";
pub(super) const SVG_FILE_READ_FAILED_MESSAGE: &str =
    "選択されたSVGファイルを読み込めませんでした。";
pub(super) const SVG_FILE_INVALID_MESSAGE: &str =
    "選択されたSVGファイルが破損しているか、対応していない形式です。";

#[derive(Default)]
pub(super) struct SvgImportState(Mutex<SvgImportSlot>);

#[derive(Default)]
pub(super) struct SvgImportSlot {
    pub(super) pending: Option<PendingSvgImport>,
    pub(super) validation_generation_id: Option<ProjectId>,
    pub(super) validation: Option<SvgImportSettingsValidation>,
    pub(super) last_cancelled_id: Option<ProjectId>,
}

#[derive(Clone)]
pub(super) struct PendingSvgImport {
    pub(super) import_id: ProjectId,
    pub(super) expected_instance_id: ProjectId,
    pub(super) expected_project_id: ProjectId,
    pub(super) expected_revision: u64,
    pub(super) bytes: Arc<[u8]>,
}

#[derive(Clone)]
pub(super) struct SvgImportSettingsValidation {
    pub(super) validation_id: ProjectId,
    pub(super) import_id: ProjectId,
    pub(super) expected_instance_id: ProjectId,
    pub(super) expected_project_id: ProjectId,
    pub(super) expected_revision: u64,
    pub(super) millimeters_per_unit_bits: u64,
    pub(super) boundary_candidate: Option<SvgBoundaryCandidateId>,
    pub(super) group_mappings: Vec<SvgGroupMapping>,
}

pub(super) struct SvgImportSettingsValidationCompletion {
    pub(super) validation: SvgImportSettingsValidation,
    pub(super) geometry: SvgImportGeometryValidation,
}

#[derive(Debug, Serialize)]
pub(super) struct SvgImportPreviewResponse {
    canceled: bool,
    preview: Option<SvgImportPreviewSnapshot>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub(super) struct SvgImportSettingsValidationResponse {
    pub(super) validation_id: ProjectId,
    pub(super) preview_id: ProjectId,
    pub(super) expected_project_id: ProjectId,
    pub(super) expected_revision: u64,
    pub(super) millimeters_per_unit: f64,
    pub(super) boundary_candidate_id: Option<u16>,
    pub(super) width_mm: f64,
    pub(super) height_mm: f64,
    pub(super) has_cuts: bool,
}

#[derive(Debug, Serialize)]
pub(super) struct SvgImportPreviewSnapshot {
    pub(super) import_id: ProjectId,
    pub(super) file_name: &'static str,
    pub(super) suggested_name: String,
    pub(super) default_mm_per_unit: Option<f64>,
    pub(super) root_view_box: Option<SvgRootViewBox>,
    pub(super) root_physical_size: SvgRootPhysicalSize,
    pub(super) source_segment_count: usize,
    pub(super) style_groups: Vec<SvgImportStyleGroupSnapshot>,
    pub(super) boundary_candidates: Vec<SvgBoundaryCandidateSnapshot>,
    pub(super) preview_vertices: Vec<SvgImportPreviewVertex>,
    pub(super) preview_edges: Vec<SvgImportPreviewEdge>,
    pub(super) preview_truncated: bool,
    pub(super) warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub(super) struct SvgImportStyleGroupSnapshot {
    pub(super) group_id: u16,
    pub(super) element_count: usize,
    pub(super) segment_count: usize,
    pub(super) stroke: Option<String>,
    pub(super) stroke_color: Option<String>,
    pub(super) dash_array: Option<String>,
    pub(super) line_cap: SvgLineCap,
    pub(super) classes: Vec<String>,
    pub(super) layer: Option<String>,
    pub(super) representative_id: Option<String>,
    pub(super) semantic_hint: Option<SvgImportTargetRequest>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub(super) struct SvgBoundaryCandidateSnapshot {
    pub(super) candidate_id: u16,
    pub(super) kind: &'static str,
    pub(super) segment_count: usize,
    pub(super) width: f64,
    pub(super) height: f64,
    pub(super) vertices: Vec<SvgImportPreviewVertex>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub(super) struct SvgImportPreviewVertex {
    pub(super) x: f64,
    pub(super) y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(super) struct SvgImportPreviewEdge {
    pub(super) start: usize,
    pub(super) end: usize,
    pub(super) group_id: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SvgImportStyleMappingRequest {
    pub(super) group_id: u16,
    pub(super) target: SvgImportTargetRequest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum SvgImportTargetRequest {
    Boundary,
    Mountain,
    Valley,
    Auxiliary,
    Cut,
    Ignore,
}

#[tauri::command]
pub(super) async fn preview_svg_import(
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
        let _project = lock_and_expect(
            &state,
            ProjectExpectation::new(expected_instance_id, expected_project_id, expected_revision),
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
pub(super) async fn validate_svg_import_settings(
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
            let _project = lock_and_expect(
                &state,
                ProjectExpectation::new(
                    pending.expected_instance_id,
                    pending.expected_project_id,
                    pending.expected_revision,
                ),
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
pub(super) async fn apply_svg_import(
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
pub(super) fn cancel_svg_import(
    state: State<'_, SvgImportState>,
    preview_id: ProjectId,
) -> Result<(), String> {
    cancel_pending_svg_import(&state, preview_id)
}

pub(super) fn lock_svg_import(
    state: &SvgImportState,
) -> Result<MutexGuard<'_, SvgImportSlot>, String> {
    state
        .0
        .lock()
        .map_err(|_| "the SVG import state lock is poisoned".to_owned())
}

pub(super) fn stage_pending_svg_import(
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
pub(super) fn pending_svg_import(
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

pub(super) fn pending_svg_import_in_slot(
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

pub(super) fn begin_svg_import_settings_validation(
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

pub(super) fn abandon_svg_import_settings_validation(
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

pub(super) fn ensure_svg_import_settings_validation(
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

pub(super) fn complete_svg_import_settings_validation(
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
    ensure_project_expectation(
        project,
        ProjectExpectation::new(
            validation.expected_instance_id,
            validation.expected_project_id,
            validation.expected_revision,
        ),
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

pub(super) fn cancel_pending_svg_import(
    state: &SvgImportState,
    import_id: ProjectId,
) -> Result<(), String> {
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

pub(super) fn commit_svg_import_replacement(
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
    ensure_project_expectation(
        project,
        ProjectExpectation::new(
            pending.expected_instance_id,
            pending.expected_project_id,
            pending.expected_revision,
        ),
    )?;
    if project.is_dirty() && !replace_dirty_project_confirmed {
        return Err("replacing a dirty project requires explicit confirmation".to_owned());
    }

    commit_project_replacement(project, replacement).map_err(|error| error.to_string())?;
    *pending_slot = None;
    Ok(snapshot(project))
}

pub(super) fn read_svg_import_bytes(path: &Path) -> Result<Vec<u8>, String> {
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

pub(super) fn load_svg_import_preview(path: &Path) -> Result<(Vec<u8>, SvgPreview), String> {
    let bytes = read_svg_import_bytes(path)?;
    let preview = read_svg_preview(&bytes).map_err(|_| SVG_FILE_INVALID_MESSAGE.to_owned())?;
    Ok((bytes, preview))
}

pub(super) fn svg_import_preview_snapshot(
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

pub(super) fn svg_import_warning_message(warning: &SvgPreviewWarning) -> String {
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

pub(super) struct SvgImportReplacementOptions {
    pub(super) name: String,
    pub(super) millimeters_per_unit: f64,
    pub(super) group_mappings: Vec<SvgGroupMapping>,
    pub(super) boundary_candidate: Option<SvgBoundaryCandidateId>,
    pub(super) boundary_confirmed: bool,
    pub(super) warnings_acknowledged: bool,
    pub(super) cutting_allowed_confirmed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct SvgImportGeometryValidation {
    pub(super) width_mm: f64,
    pub(super) height_mm: f64,
    pub(super) has_cuts: bool,
}

pub(super) fn build_svg_import_replacement(
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

pub(super) fn validate_svg_import_geometry(
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
    validate_active_edge_containment(&replacement, "SVG")?;
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
