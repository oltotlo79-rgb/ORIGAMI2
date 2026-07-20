//! Read-only staging boundary for an authenticated instruction-timeline GLB.
//!
//! The encoded bytes remain native and immutable.  This first boundary exposes
//! only bounded metadata; a later UI/save flow can consume the staged artifact
//! without trusting browser-supplied geometry or animation frames.

use std::sync::{Arc, Mutex, MutexGuard};

use ori_domain::{InstructionPoseModel, InstructionTimeline, ProjectId};
use ori_formats::{IndexedTriangleMeshAnimationV1, export_animated_triangle_mesh_glb};
use ori_kinematics::{
    CanonicalHingeAngles, HingeAngle, MaterialTreeKinematicsModel, TreeKinematicsLimits,
};
use serde::{Deserialize, Serialize};
use tauri::State;

use super::{
    AppState, TopologyAnalysisInput, lock_project, mesh_export::build_current_pose_mid_surface_mesh,
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

#[derive(Debug, Deserialize)]
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
    let initial = model
        .solve(None, &initial_angles)
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
    }
}

fn lock_slot(
    state: &MeshAnimationExportState,
) -> Result<MutexGuard<'_, MeshAnimationExportSlot>, String> {
    state.0.lock().map_err(|_| PREVIEW_FAILED.to_owned())
}

#[cfg(test)]
mod tests {
    use ori_core::Command;
    use ori_domain::{InstructionPose, InstructionStep, InstructionStepId, InstructionVisual};

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
}
