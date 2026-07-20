//! Read-only staging boundary for an authenticated instruction-timeline GLB.
//!
//! The encoded bytes remain native and immutable.  This first boundary exposes
//! only bounded metadata; a later UI/save flow can consume the staged artifact
//! without trusting browser-supplied geometry or animation frames.

use std::{
    path::Path,
    sync::{Arc, Mutex, MutexGuard},
};

use ori_domain::{InstructionPoseModel, InstructionTimeline, ProjectId};
use ori_formats::{IndexedTriangleMeshAnimationV1, export_animated_triangle_mesh_glb};
use ori_kinematics::{
    CanonicalHingeAngles, HingeAngle, MaterialTreeKinematicsModel, TreeKinematicsLimits,
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

use super::{
    AppState, ProjectState, TopologyAnalysisInput,
    crease_export::persist_export_bytes_to_destination, lock_project,
    mesh_export::build_current_pose_mid_surface_mesh,
};

const PREVIEW_FAILED: &str = "the instruction animation preview could not be prepared";
const STALE_PREVIEW: &str = "the project changed while preparing the instruction animation";

#[derive(Default)]
pub(super) struct MeshAnimationExportState(Mutex<MeshAnimationExportSlot>);

#[derive(Default)]
struct MeshAnimationExportSlot {
    active_generation_id: Option<ProjectId>,
    pending: Option<Arc<PendingMeshAnimationExport>>,
}

struct PendingMeshAnimationExport {
    export_id: ProjectId,
    project_instance_id: ProjectId,
    project_id: ProjectId,
    revision: u64,
    source_fingerprint: Arc<str>,
    timeline: InstructionTimeline,
    bytes: Arc<[u8]>,
    frame_count: usize,
    vertex_count: usize,
    triangle_count: usize,
    duration_seconds: f32,
    suggested_file_name: String,
}

struct MeshAnimationSource {
    export_id: ProjectId,
    project_instance_id: ProjectId,
    project_id: ProjectId,
    revision: u64,
    project_name: String,
    source_fingerprint: Arc<str>,
    topology_input: TopologyAnalysisInput,
    timeline: InstructionTimeline,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct MeshAnimationPreviewRequest {
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct MeshAnimationPreviewResponse {
    export_id: ProjectId,
    project_instance_id: ProjectId,
    project_id: ProjectId,
    revision: u64,
    source_fingerprint: String,
    frame_count: usize,
    vertex_count: usize,
    triangle_count: usize,
    duration_seconds: f32,
    byte_count: usize,
    media_type: &'static str,
    file_extension: &'static str,
    suggested_file_name: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct MeshAnimationSaveRequest {
    export_id: ProjectId,
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    expected_source_fingerprint: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct MeshAnimationSaveResponse {
    canceled: bool,
}

#[tauri::command]
pub(super) async fn preview_instruction_mesh_animation(
    state: State<'_, AppState>,
    export_state: State<'_, MeshAnimationExportState>,
    request: MeshAnimationPreviewRequest,
) -> Result<MeshAnimationPreviewResponse, String> {
    let export_id = ProjectId::new();
    {
        let mut slot = lock_slot(&export_state)?;
        slot.active_generation_id = Some(export_id);
        slot.pending = None;
    }
    let source = capture_source(&state, export_id, request)?;
    let permit = state
        .try_acquire_native_pose_worker()
        .ok_or_else(|| "another native pose analysis is already running".to_owned())?;
    let pending = tauri::async_runtime::spawn_blocking(move || {
        let _permit = permit;
        build_pending(source)
    })
    .await
    .map_err(|_| PREVIEW_FAILED)?
    .map(Arc::new)?;
    let mut slot = lock_slot(&export_state)?;
    if slot.active_generation_id != Some(export_id) {
        return Err(STALE_PREVIEW.to_owned());
    }
    let project = lock_project(&state)?;
    if project.instance_id != pending.project_instance_id
        || project.project_id != pending.project_id
        || project.editor.revision() != pending.revision
        || project.editor.fold_model_fingerprint_v1() != pending.source_fingerprint.as_ref()
        || project.editor.instruction_timeline() != &pending.timeline
    {
        slot.active_generation_id = None;
        return Err(STALE_PREVIEW.to_owned());
    }
    let response = response(&pending);
    slot.pending = Some(pending);
    Ok(response)
}

#[tauri::command]
pub(super) async fn save_instruction_mesh_animation(
    app: AppHandle,
    state: State<'_, AppState>,
    export_state: State<'_, MeshAnimationExportState>,
    request: MeshAnimationSaveRequest,
) -> Result<MeshAnimationSaveResponse, String> {
    let (pending, initial_directory) = {
        let slot = lock_slot(&export_state)?;
        let project = lock_project(&state)?;
        let pending = checked_pending(&slot, &project, &request)?;
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
        .add_filter("glTF 2.0 binary animation", &["glb"])
        .set_file_name(pending.suggested_file_name.clone())
        .set_title("手順アニメーションをGLBとして保存");
    if let Some(directory) = initial_directory {
        dialog = dialog.set_directory(directory);
    }
    let Some(selected) = dialog.blocking_save_file() else {
        // Keep the immutable generation for a retry, but authenticate it again
        // after the native picker closes.
        let slot = lock_slot(&export_state)?;
        let project = lock_project(&state)?;
        checked_pending(&slot, &project, &request)?;
        return Ok(MeshAnimationSaveResponse { canceled: true });
    };
    let selected_path = selected
        .simplified()
        .into_path()
        .map_err(|_| "the selected animation destination is not a local file".to_owned())?;
    let destination = super::save_path::normalize_dialog_save_path(selected_path, "glb")?;
    commit_pending_to_destination(&export_state, &state, &request, &destination)?;
    Ok(MeshAnimationSaveResponse { canceled: false })
}

#[tauri::command]
pub(super) fn cancel_instruction_mesh_animation(
    state: State<'_, MeshAnimationExportState>,
    export_id: ProjectId,
) -> Result<(), String> {
    let mut slot = lock_slot(&state)?;
    if slot.pending.as_ref().map(|pending| pending.export_id) == Some(export_id)
        || slot.active_generation_id == Some(export_id)
    {
        slot.pending = None;
        slot.active_generation_id = None;
        return Ok(());
    }
    Err(STALE_PREVIEW.to_owned())
}

fn capture_source(
    state: &AppState,
    export_id: ProjectId,
    request: MeshAnimationPreviewRequest,
) -> Result<MeshAnimationSource, String> {
    let project = lock_project(state)?;
    if project.instance_id != request.expected_project_instance_id
        || project.project_id != request.expected_project_id
        || project.editor.revision() != request.expected_revision
    {
        return Err(STALE_PREVIEW.to_owned());
    }
    let source_fingerprint: Arc<str> = Arc::from(project.editor.fold_model_fingerprint_v1());
    if !super::valid_fold_model_fingerprint(&source_fingerprint) {
        return Err(PREVIEW_FAILED.to_owned());
    }
    let timeline = project.editor.instruction_timeline().clone();
    if timeline.steps.is_empty()
        || timeline.steps.iter().any(|step| {
            step.pose.model != InstructionPoseModel::AbsoluteHingeAnglesV1
                || step.pose.source_model_fingerprint != source_fingerprint.as_ref()
        })
    {
        return Err(PREVIEW_FAILED.to_owned());
    }
    Ok(MeshAnimationSource {
        export_id,
        project_instance_id: project.instance_id,
        project_id: project.project_id,
        revision: project.editor.revision(),
        project_name: project.name.clone(),
        source_fingerprint,
        topology_input: project.editor.topology_analysis_input(project.project_id),
        timeline,
    })
}

fn build_pending(source: MeshAnimationSource) -> Result<PendingMeshAnimationExport, String> {
    let topology = source
        .topology_input
        .analyze()
        .simulation_snapshot()
        .cloned()
        .ok_or_else(|| PREVIEW_FAILED.to_owned())?;
    let model = MaterialTreeKinematicsModel::prepare(
        source.topology_input.pattern(),
        source.topology_input.paper(),
        &topology,
        TreeKinematicsLimits::default(),
    )
    .map_err(|_| PREVIEW_FAILED.to_owned())?;
    let initial_angles = model
        .hinges()
        .iter()
        .map(|hinge| HingeAngle::new(hinge.edge(), 0.0))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| PREVIEW_FAILED.to_owned())?;
    let initial_angles =
        CanonicalHingeAngles::new(initial_angles).map_err(|_| PREVIEW_FAILED.to_owned())?;
    let initial_fixed_face = if model.hinges().is_empty() {
        None
    } else {
        Some(
            source
                .timeline
                .steps
                .first()
                .and_then(|step| step.pose.fixed_face)
                .ok_or_else(|| PREVIEW_FAILED.to_owned())?,
        )
    };
    let initial = model
        .solve(initial_fixed_face, &initial_angles)
        .map_err(|_| PREVIEW_FAILED.to_owned())?;
    let mut frames = vec![build_current_pose_mid_surface_mesh(
        &source.project_name,
        &model,
        &initial,
    )?];
    let mut times_seconds = vec![0.0_f32];
    let mut elapsed_ms = 0_u64;
    for step in &source.timeline.steps {
        let angles = step
            .pose
            .hinge_angles
            .iter()
            .map(|angle| HingeAngle::new(angle.edge, angle.angle_degrees))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| PREVIEW_FAILED.to_owned())?;
        let angles = CanonicalHingeAngles::new(angles).map_err(|_| PREVIEW_FAILED.to_owned())?;
        let pose = model
            .solve(step.pose.fixed_face, &angles)
            .map_err(|_| PREVIEW_FAILED.to_owned())?;
        elapsed_ms = elapsed_ms
            .checked_add(u64::from(step.duration_ms))
            .ok_or_else(|| PREVIEW_FAILED.to_owned())?;
        let time = elapsed_ms as f32 / 1_000.0;
        if !time.is_finite() {
            return Err(PREVIEW_FAILED.to_owned());
        }
        times_seconds.push(time);
        frames.push(build_current_pose_mid_surface_mesh(
            &source.project_name,
            &model,
            &pose,
        )?);
    }
    let document = IndexedTriangleMeshAnimationV1::new(times_seconds, frames);
    let artifact =
        export_animated_triangle_mesh_glb(&document).map_err(|_| PREVIEW_FAILED.to_owned())?;
    Ok(PendingMeshAnimationExport {
        export_id: source.export_id,
        project_instance_id: source.project_instance_id,
        project_id: source.project_id,
        revision: source.revision,
        source_fingerprint: source.source_fingerprint,
        timeline: source.timeline,
        bytes: Arc::from(artifact.bytes),
        frame_count: artifact.frame_count,
        vertex_count: artifact.vertex_count,
        triangle_count: artifact.triangle_count,
        duration_seconds: *document.times_seconds.last().unwrap_or(&0.0),
        suggested_file_name: format!(
            "{}-instruction-animation.glb",
            safe_file_stem(&source.project_name)
        ),
    })
}

fn response(pending: &PendingMeshAnimationExport) -> MeshAnimationPreviewResponse {
    MeshAnimationPreviewResponse {
        export_id: pending.export_id,
        project_instance_id: pending.project_instance_id,
        project_id: pending.project_id,
        revision: pending.revision,
        source_fingerprint: pending.source_fingerprint.to_string(),
        frame_count: pending.frame_count,
        vertex_count: pending.vertex_count,
        triangle_count: pending.triangle_count,
        duration_seconds: pending.duration_seconds,
        byte_count: pending.bytes.len(),
        media_type: "model/gltf-binary",
        file_extension: "glb",
        suggested_file_name: pending.suggested_file_name.clone(),
    }
}

fn checked_pending<'a>(
    slot: &'a MeshAnimationExportSlot,
    project: &ProjectState,
    request: &MeshAnimationSaveRequest,
) -> Result<&'a Arc<PendingMeshAnimationExport>, String> {
    let pending = slot
        .pending
        .as_ref()
        .ok_or_else(|| STALE_PREVIEW.to_owned())?;
    if slot.active_generation_id != Some(request.export_id)
        || pending.export_id != request.export_id
        || pending.project_instance_id != request.expected_project_instance_id
        || pending.project_id != request.expected_project_id
        || pending.revision != request.expected_revision
        || pending.source_fingerprint.as_ref() != request.expected_source_fingerprint
        || !pending_is_current(project, pending)
    {
        return Err(STALE_PREVIEW.to_owned());
    }
    Ok(pending)
}

fn pending_is_current(project: &ProjectState, pending: &PendingMeshAnimationExport) -> bool {
    project.instance_id == pending.project_instance_id
        && project.project_id == pending.project_id
        && project.editor.revision() == pending.revision
        && project.editor.fold_model_fingerprint_v1() == pending.source_fingerprint.as_ref()
        && project.editor.instruction_timeline() == &pending.timeline
}

fn commit_pending_to_destination(
    export_state: &MeshAnimationExportState,
    state: &AppState,
    request: &MeshAnimationSaveRequest,
    destination: &super::save_path::DialogSaveDestination,
) -> Result<(), String> {
    let mut slot = lock_slot(export_state)?;
    let project = lock_project(state)?;
    let pending = checked_pending(&slot, &project, request)?;
    persist_export_bytes_to_destination(destination, &pending.bytes)?;
    slot.pending = None;
    slot.active_generation_id = None;
    Ok(())
}

fn safe_file_stem(name: &str) -> String {
    let stem: String = name
        .chars()
        .map(|character| {
            if character.is_control()
                || matches!(
                    character,
                    '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'
                )
            {
                '_'
            } else {
                character
            }
        })
        .collect();
    let stem = stem.trim().trim_matches('.').trim();
    if stem.is_empty() { "origami2" } else { stem }.to_owned()
}

fn lock_slot(
    state: &MeshAnimationExportState,
) -> Result<MutexGuard<'_, MeshAnimationExportSlot>, String> {
    state.0.lock().map_err(|_| PREVIEW_FAILED.to_owned())
}

#[cfg(test)]
mod tests {
    use ori_core::Command;
    use ori_domain::{
        CreasePattern, Edge, EdgeId, EdgeKind, InstructionHingeAngle, InstructionPose,
        InstructionStep, InstructionStepId, InstructionVisual, Paper, Point2, Vertex, VertexId,
    };

    use super::*;

    fn state_with_one_executable_step() -> AppState {
        let mut project = crate::initial_project_state();
        let fingerprint = project.editor.fold_model_fingerprint_v1();
        project
            .editor
            .execute(
                0,
                Command::AddInstructionStep {
                    step: InstructionStep {
                        id: InstructionStepId::new(),
                        title: "Planar".to_owned(),
                        description: String::new(),
                        caution: String::new(),
                        duration_ms: 1_250,
                        visual: InstructionVisual::default(),
                        pose: InstructionPose {
                            model: InstructionPoseModel::AbsoluteHingeAnglesV1,
                            source_model_fingerprint: fingerprint,
                            fixed_face: None,
                            hinge_angles: Vec::new(),
                        },
                    },
                },
            )
            .expect("append executable step");
        AppState::new(project)
    }

    fn state_with_two_hinges(missing_fixed_face: bool) -> AppState {
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
        let hinges = [
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
        ];
        let hinge_ids = hinges.each_ref().map(|edge| edge.id);
        edges.extend(hinges);
        let paper = Paper {
            boundary_vertices: vertices.iter().map(|vertex| vertex.id).collect(),
            ..Paper::default()
        };
        let mut project = ProjectState::new_with_paper(CreasePattern { vertices, edges }, paper);
        let actual_fixed_face = {
            project
                .editor
                .topology_analysis_input(project.project_id)
                .analyze()
                .simulation_snapshot()
                .and_then(|snapshot| snapshot.faces.first().map(|face| face.id))
        };
        let fingerprint = project.editor.fold_model_fingerprint_v1();
        project
            .editor
            .execute(
                0,
                Command::AddInstructionStep {
                    step: InstructionStep {
                        id: InstructionStepId::new(),
                        title: "Two hinges".to_owned(),
                        description: String::new(),
                        caution: String::new(),
                        duration_ms: 1_000,
                        visual: InstructionVisual::default(),
                        pose: InstructionPose {
                            model: InstructionPoseModel::AbsoluteHingeAnglesV1,
                            source_model_fingerprint: fingerprint,
                            fixed_face: if missing_fixed_face {
                                None
                            } else {
                                actual_fixed_face
                            },
                            hinge_angles: hinge_ids
                                .into_iter()
                                .map(|edge| InstructionHingeAngle {
                                    edge,
                                    angle_degrees: 30.0,
                                })
                                .collect(),
                        },
                    },
                },
            )
            .unwrap();
        AppState::new(project)
    }

    fn temporary_directory() -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "origami2-mesh-animation-test-{gap:?}",
            gap = ProjectId::new()
        ));
        std::fs::create_dir(&path).unwrap();
        path
    }

    #[test]
    fn authenticated_timeline_builds_bounded_immutable_glb_stage() {
        let state = state_with_one_executable_step();
        let request = {
            let project = lock_project(&state).expect("project");
            MeshAnimationPreviewRequest {
                expected_project_instance_id: project.instance_id,
                expected_project_id: project.project_id,
                expected_revision: project.editor.revision(),
            }
        };
        let source = capture_source(&state, ProjectId::new(), request).expect("capture");
        let pending = build_pending(source).expect("animated GLB");
        assert_eq!(pending.frame_count, 2);
        assert_eq!(pending.duration_seconds, 1.25);
        assert!(pending.vertex_count >= 4);
        assert!(pending.triangle_count >= 2);
        assert!(pending.bytes.starts_with(b"glTF"));
        assert_eq!(lock_project(&state).unwrap().editor.revision(), 1);
    }

    #[test]
    fn multi_hinge_animation_uses_first_step_fixed_face_for_initial_frame() {
        let state = state_with_two_hinges(false);
        let request = {
            let project = lock_project(&state).unwrap();
            MeshAnimationPreviewRequest {
                expected_project_instance_id: project.instance_id,
                expected_project_id: project.project_id,
                expected_revision: project.editor.revision(),
            }
        };
        let source = capture_source(&state, ProjectId::new(), request).unwrap();
        let pending = build_pending(source).expect("multi-hinge initial frame");
        assert_eq!(pending.frame_count, 2);
        assert!(pending.bytes.starts_with(b"glTF"));
    }

    #[test]
    fn multi_hinge_animation_without_fixed_face_fails_closed() {
        let state = state_with_two_hinges(true);
        let request = {
            let project = lock_project(&state).unwrap();
            MeshAnimationPreviewRequest {
                expected_project_instance_id: project.instance_id,
                expected_project_id: project.project_id,
                expected_revision: project.editor.revision(),
            }
        };
        let source = capture_source(&state, ProjectId::new(), request).unwrap();
        assert_eq!(build_pending(source).err().as_deref(), Some(PREVIEW_FAILED));
    }

    #[test]
    fn stale_or_browser_extended_requests_are_rejected() {
        let state = state_with_one_executable_step();
        let project = lock_project(&state).expect("project");
        let value = serde_json::json!({
            "expectedProjectInstanceId": project.instance_id,
            "expectedProjectId": project.project_id,
            "expectedRevision": project.editor.revision(),
            "frames": [],
        });
        drop(project);
        assert!(serde_json::from_value::<MeshAnimationPreviewRequest>(value).is_err());

        let project = lock_project(&state).expect("project");
        let request = MeshAnimationPreviewRequest {
            expected_project_instance_id: project.instance_id,
            expected_project_id: project.project_id,
            expected_revision: project.editor.revision() + 1,
        };
        drop(project);
        assert!(capture_source(&state, ProjectId::new(), request).is_err());
    }

    fn staged_export(
        state: &AppState,
    ) -> (
        MeshAnimationExportState,
        MeshAnimationSaveRequest,
        Arc<[u8]>,
    ) {
        let (request, export_id) = {
            let project = lock_project(state).unwrap();
            (
                MeshAnimationPreviewRequest {
                    expected_project_instance_id: project.instance_id,
                    expected_project_id: project.project_id,
                    expected_revision: project.editor.revision(),
                },
                ProjectId::new(),
            )
        };
        let pending =
            Arc::new(build_pending(capture_source(state, export_id, request).unwrap()).unwrap());
        let bytes = Arc::clone(&pending.bytes);
        let save_request = MeshAnimationSaveRequest {
            export_id,
            expected_project_instance_id: pending.project_instance_id,
            expected_project_id: pending.project_id,
            expected_revision: pending.revision,
            expected_source_fingerprint: pending.source_fingerprint.to_string(),
        };
        let export_state = MeshAnimationExportState::default();
        {
            let mut slot = lock_slot(&export_state).unwrap();
            slot.active_generation_id = Some(export_id);
            slot.pending = Some(pending);
        }
        (export_state, save_request, bytes)
    }

    #[test]
    fn canceled_picker_retains_exact_generation_for_retry() {
        let state = state_with_one_executable_step();
        let (export_state, request, _) = staged_export(&state);
        {
            let slot = lock_slot(&export_state).unwrap();
            let project = lock_project(&state).unwrap();
            checked_pending(&slot, &project, &request).expect("first canceled picker");
        }
        let slot = lock_slot(&export_state).unwrap();
        let project = lock_project(&state).unwrap();
        checked_pending(&slot, &project, &request).expect("retry uses same immutable bytes");
    }

    #[test]
    fn successful_save_consumes_pending_generation_and_writes_exact_bytes() {
        let state = state_with_one_executable_step();
        let (export_state, request, expected) = staged_export(&state);
        let directory = temporary_directory();
        let path = directory.join("animation.glb");
        let destination = super::super::save_path::DialogSaveDestination::confirmed(path.clone());
        commit_pending_to_destination(&export_state, &state, &request, &destination).unwrap();
        assert_eq!(std::fs::read(path).unwrap(), expected.as_ref());
        let slot = lock_slot(&export_state).unwrap();
        assert!(slot.pending.is_none());
        assert!(slot.active_generation_id.is_none());
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn stale_project_is_rejected_before_save_and_bytes_remain_pending() {
        let state = state_with_one_executable_step();
        let (export_state, request, _) = staged_export(&state);
        {
            let mut project = lock_project(&state).unwrap();
            let fingerprint = project.editor.fold_model_fingerprint_v1();
            project
                .editor
                .execute(
                    request.expected_revision,
                    Command::AddInstructionStep {
                        step: InstructionStep {
                            id: InstructionStepId::new(),
                            title: "Changed".to_owned(),
                            description: String::new(),
                            caution: String::new(),
                            duration_ms: 500,
                            visual: InstructionVisual::default(),
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
        }
        let directory = temporary_directory();
        let destination =
            super::super::save_path::DialogSaveDestination::confirmed(directory.join("stale.glb"));
        assert_eq!(
            commit_pending_to_destination(&export_state, &state, &request, &destination),
            Err(STALE_PREVIEW.to_owned())
        );
        assert!(lock_slot(&export_state).unwrap().pending.is_some());
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn save_request_schema_is_strict() {
        let state = state_with_one_executable_step();
        let (_, request, _) = staged_export(&state);
        let mut value = serde_json::to_value(&request).unwrap();
        value
            .as_object_mut()
            .unwrap()
            .insert("bytes".to_owned(), serde_json::json!([1, 2, 3]));
        assert!(serde_json::from_value::<MeshAnimationSaveRequest>(value).is_err());
    }
}
