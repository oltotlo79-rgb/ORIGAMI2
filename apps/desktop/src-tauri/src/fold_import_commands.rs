//! Native FOLD crease-pattern import boundary.
//!
//! This module owns bounded file ingestion, one-slot opaque preview staging,
//! mapping revalidation, conversion, cancellation, and atomic project
//! replacement. General project identity and persistence mechanics remain in
//! the crate root.

use super::crease_pattern_boundary_support::validate_active_edge_containment;
use super::import_command_support::validate_import_scale;
use super::*;

pub(super) const MAX_FOLD_IMPORT_FILE_SIZE: u64 = 16 * 1024 * 1024;
pub(super) const MAX_FOLD_IMPORT_PREVIEW_EDGES: usize = 5_000;
pub(super) const FOLD_IMPORT_FILE_LABEL: &str = "選択したFOLDファイル";
const FOLD_IMPORT_FALLBACK_NAME: &str = "FOLDインポート";
pub(super) const FOLD_IMPORT_TASK_FAILED_MESSAGE: &str =
    "FOLDファイルの解析処理を完了できませんでした。もう一度実行してください。";
pub(super) const FOLD_CONVERSION_TASK_FAILED_MESSAGE: &str =
    "FOLDファイルの変換処理を完了できませんでした。もう一度実行してください。";
pub(super) const FOLD_FILE_OPEN_FAILED_MESSAGE: &str = "選択されたFOLDファイルを開けませんでした。";
pub(super) const FOLD_FILE_INSPECTION_FAILED_MESSAGE: &str =
    "選択されたFOLDファイルのサイズを確認できませんでした。";
pub(super) const FOLD_FILE_TOO_LARGE_MESSAGE: &str =
    "選択されたFOLDファイルはサイズ上限を超えています。";
pub(super) const FOLD_FILE_READ_FAILED_MESSAGE: &str =
    "選択されたFOLDファイルを読み込めませんでした。";
pub(super) const FOLD_FILE_INVALID_MESSAGE: &str =
    "選択されたFOLDファイルが破損しているか、対応していない形式です。";

pub(super) fn fold_import_task_error<T>(_: T) -> String {
    FOLD_IMPORT_TASK_FAILED_MESSAGE.to_owned()
}

pub(super) fn fold_conversion_task_error<T>(_: T) -> String {
    FOLD_CONVERSION_TASK_FAILED_MESSAGE.to_owned()
}

fn fold_file_invalid_error<T>(_: T) -> String {
    FOLD_FILE_INVALID_MESSAGE.to_owned()
}

#[derive(Default)]
pub(super) struct FoldImportState(Mutex<Option<PendingFoldImport>>);

#[derive(Clone)]
pub(super) struct PendingFoldImport {
    pub(super) import_id: ProjectId,
    pub(super) expected_instance_id: ProjectId,
    pub(super) expected_project_id: ProjectId,
    pub(super) expected_revision: u64,
    pub(super) bytes: Arc<[u8]>,
}

#[derive(Debug, Serialize)]
pub(super) struct FoldImportPreviewResponse {
    canceled: bool,
    preview: Option<FoldImportPreviewSnapshot>,
}

#[derive(Debug, Serialize)]
pub(super) struct FoldImportPreviewSnapshot {
    pub(super) import_id: ProjectId,
    pub(super) file_name: &'static str,
    pub(super) suggested_name: String,
    pub(super) file_spec: Option<String>,
    pub(super) frame_unit: Option<String>,
    pub(super) default_mm_per_unit: Option<f64>,
    pub(super) vertex_count: usize,
    pub(super) edge_count: usize,
    pub(super) boundary_edge_count: usize,
    pub(super) boundary_candidates: Vec<FoldImportBoundaryCandidateSnapshot>,
    pub(super) fixed_boundary_candidate_id: Option<u16>,
    pub(super) assignments: Vec<FoldImportAssignmentSummary>,
    pub(super) preview_vertices: Vec<FoldImportPreviewVertex>,
    pub(super) preview_edges: Vec<FoldImportPreviewEdge>,
    pub(super) preview_truncated: bool,
    pub(super) warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct FoldImportBoundaryCandidateSnapshot {
    pub(super) id: u16,
    pub(super) source: &'static str,
    pub(super) edge_indices: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct FoldImportAssignmentSummary {
    pub(super) assignment: String,
    pub(super) count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub(super) struct FoldImportPreviewVertex {
    pub(super) x: f64,
    pub(super) y: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct FoldImportPreviewEdge {
    pub(super) source_index: usize,
    pub(super) start: usize,
    pub(super) end: usize,
    pub(super) assignment: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(super) struct FoldImportAssignmentMappingRequest {
    pub(super) source: String,
    pub(super) target: FoldImportTargetRequest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum FoldImportTargetRequest {
    Mountain,
    Valley,
    Auxiliary,
    Cut,
    Ignore,
}

#[tauri::command]
pub(super) async fn preview_fold_import(
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
        let _project = lock_and_expect(
            &state,
            ProjectExpectation::new(expected_instance_id, expected_project_id, expected_revision),
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
pub(super) async fn apply_fold_import(
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
pub(super) fn cancel_fold_import(
    state: State<'_, FoldImportState>,
    preview_id: ProjectId,
) -> Result<(), String> {
    cancel_pending_fold_import(&state, preview_id)
}

pub(super) fn lock_fold_import(
    state: &FoldImportState,
) -> Result<MutexGuard<'_, Option<PendingFoldImport>>, String> {
    state
        .0
        .lock()
        .map_err(|_| "the FOLD import state lock is poisoned".to_owned())
}

pub(super) fn stage_pending_fold_import(
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

pub(super) fn pending_fold_import(
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

pub(super) fn cancel_pending_fold_import(
    state: &FoldImportState,
    import_id: ProjectId,
) -> Result<(), String> {
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

pub(super) fn commit_fold_import_replacement(
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
    ensure_project_expectation(
        project,
        ProjectExpectation::new(
            pending.expected_instance_id,
            pending.expected_project_id,
            pending.expected_revision,
        ),
    )?;
    commit_project_replacement(project, replacement).map_err(|error| error.to_string())?;
    *pending_slot = None;
    Ok(snapshot(project))
}

pub(super) fn validate_fold_import_mapping_requests(
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

pub(super) fn read_fold_import_bytes(path: &Path) -> Result<Vec<u8>, String> {
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

pub(super) fn load_fold_import_preview(path: &Path) -> Result<(Vec<u8>, FoldPreview), String> {
    let bytes = read_fold_import_bytes(path)?;
    let preview = read_fold_preview(&bytes).map_err(fold_file_invalid_error)?;
    Ok((bytes, preview))
}

pub(super) fn fold_import_preview_snapshot(
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

pub(super) fn build_fold_import_replacement(
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
    validate_active_edge_containment(&replacement, "FOLD")?;
    Ok(replacement)
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
