//! Read-only desktop bridge for the first authenticated SIM-010 boundary.
//!
//! No value returned by this module authorizes project mutation. Heavy exact
//! analysis runs over detached immutable capabilities and is revalidated
//! against both live native slots before its bounded observation is returned.

use ori_collision::{
    FlatEndpointLayerOrderInputV1, StackedFoldFixedSideV1, StackedFoldLinearCandidateV1,
    StackedFoldReadBindingV1, StackedFoldReadLimitsV1, StackedFoldReadSupportV1,
    StackedFoldRotationDirectionV1, capture_stacked_fold_read_guard_v1,
    propose_linear_stacked_fold_read_v1,
};
use ori_domain::{FaceId, ProjectId};
use ori_kinematics::Point3;
use serde::{Deserialize, Serialize};
use tauri::State;

use super::{
    AppState,
    global_flat_foldability::{
        GlobalFlatFoldabilityState, capture_current_layer_order_capability,
        revalidate_current_layer_order_capability,
    },
    lock_project,
};

const UNAVAILABLE_MESSAGE: &str =
    "The current pose and certified layer order cannot prepare a stacked-fold proposal.";
const INVALID_REQUEST_MESSAGE: &str = "The stacked-fold line request is invalid.";
const ANALYSIS_FAILED_MESSAGE: &str =
    "The stacked-fold proposal is unsupported or could not be certified.";
const BUSY_MESSAGE: &str = "Another native pose analysis is already running.";
const STALE_MESSAGE: &str =
    "The project, current pose, or certified layer order changed during analysis.";

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum FixedSideRequest {
    Left,
    Right,
}

impl From<FixedSideRequest> for StackedFoldFixedSideV1 {
    fn from(value: FixedSideRequest) -> Self {
        match value {
            FixedSideRequest::Left => Self::Left,
            FixedSideRequest::Right => Self::Right,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RotationDirectionRequest {
    Positive,
    Negative,
}

impl From<RotationDirectionRequest> for StackedFoldRotationDirectionV1 {
    fn from(value: RotationDirectionRequest) -> Self {
        match value {
            RotationDirectionRequest::Positive => Self::Positive,
            RotationDirectionRequest::Negative => Self::Negative,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct StackedFoldReadRequest {
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    first: [f64; 3],
    second: [f64; 3],
    fixed_side: FixedSideRequest,
    rotation_direction: RotationDirectionRequest,
    requested_angle_degrees: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum StackedFoldReadSupportDto {
    NoHingeSingleFace,
    BitExactFlatEndpointTree,
}

impl From<StackedFoldReadSupportV1> for StackedFoldReadSupportDto {
    fn from(value: StackedFoldReadSupportV1) -> Self {
        match value {
            StackedFoldReadSupportV1::NoHingeSingleFace => Self::NoHingeSingleFace,
            StackedFoldReadSupportV1::BitExactFlatEndpointTree => Self::BitExactFlatEndpointTree,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct StackedFoldReadBindingDto {
    project_instance_id: ProjectId,
    project_id: ProjectId,
    source_revision: u64,
    pose_generation: u64,
    layer_order_generation: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct StackedFoldReadCellDto {
    cell_key_sha256: String,
    bottom_to_top_faces: Vec<FaceId>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct StackedFoldReadWorkDto {
    scanned_cells: usize,
    total_boundary_vertices: usize,
    total_layer_records: usize,
    orientation_tests: usize,
    exact_arithmetic_operations: usize,
    maximum_exact_integer_bits: usize,
    total_exact_integer_bits: usize,
    retained_cells: usize,
    retained_target_faces: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct StackedFoldReadResponse {
    guard_model_id: &'static str,
    proposal_model_id: &'static str,
    binding: StackedFoldReadBindingDto,
    support: StackedFoldReadSupportDto,
    crossed_cells: Vec<StackedFoldReadCellDto>,
    target_faces: Vec<FaceId>,
    work: StackedFoldReadWorkDto,
    authorizes_project_mutation: bool,
    authorizes_apply_stacked_fold: bool,
}

#[tauri::command]
pub(super) async fn propose_current_stacked_fold_read(
    app_state: State<'_, AppState>,
    foldability_state: State<'_, GlobalFlatFoldabilityState>,
    request: StackedFoldReadRequest,
) -> Result<StackedFoldReadResponse, String> {
    let worker_permit = app_state
        .try_acquire_native_pose_worker()
        .ok_or_else(|| BUSY_MESSAGE.to_owned())?;
    let (paper, pattern, pose_capability, layer_capability, binding) = {
        let project = lock_project(&app_state).map_err(|_| UNAVAILABLE_MESSAGE.to_owned())?;
        if project.instance_id != request.expected_project_instance_id
            || project.project_id != request.expected_project_id
            || project.editor.revision() != request.expected_revision
        {
            return Err(STALE_MESSAGE.to_owned());
        }
        let pose_capability = project
            .applied_pose_authority
            .capture_capability(&project)
            .map_err(|_| UNAVAILABLE_MESSAGE.to_owned())?
            .ok_or_else(|| UNAVAILABLE_MESSAGE.to_owned())?;
        let layer_capability = capture_current_layer_order_capability(&foldability_state, &project)
            .map_err(|_| UNAVAILABLE_MESSAGE.to_owned())?
            .ok_or_else(|| UNAVAILABLE_MESSAGE.to_owned())?;
        let binding = StackedFoldReadBindingV1::new(
            project.instance_id,
            project.project_id,
            project.editor.revision(),
            pose_capability.generation(),
            layer_capability.generation(),
        );
        (
            project.editor.paper().clone(),
            project.editor.pattern().clone(),
            pose_capability,
            layer_capability,
            binding,
        )
    };

    let first = Point3::new(request.first[0], request.first[1], request.first[2])
        .map_err(|_| INVALID_REQUEST_MESSAGE.to_owned())?;
    let second = Point3::new(request.second[0], request.second[1], request.second[2])
        .map_err(|_| INVALID_REQUEST_MESSAGE.to_owned())?;
    let candidate = StackedFoldLinearCandidateV1::new(
        first,
        second,
        request.fixed_side.into(),
        request.rotation_direction.into(),
        request.requested_angle_degrees,
    )
    .map_err(|_| INVALID_REQUEST_MESSAGE.to_owned())?;
    let analysis = tauri::async_runtime::spawn_blocking(move || {
        let input = FlatEndpointLayerOrderInputV1 {
            identity_namespace: binding.project_id(),
            source_revision: binding.source_revision(),
            paper: &paper,
            pattern: &pattern,
            model: pose_capability.model(),
            pose: pose_capability.pose(),
            layer_order: layer_capability.snapshot(),
        };
        let limits = StackedFoldReadLimitsV1::default();
        let guard = capture_stacked_fold_read_guard_v1(binding, input, limits)
            .map_err(|_| ANALYSIS_FAILED_MESSAGE.to_owned())?;
        let proposal =
            propose_linear_stacked_fold_read_v1(&guard, binding, input, candidate, limits)
                .map_err(|_| ANALYSIS_FAILED_MESSAGE.to_owned())?;
        let crossed_cells = proposal
            .crossed_cells()
            .iter()
            .map(|cell| StackedFoldReadCellDto {
                cell_key_sha256: lowercase_hex(cell.cell_key().canonical_bytes()),
                bottom_to_top_faces: cell.bottom_to_top_faces().to_vec(),
            })
            .collect();
        let work = proposal.work();
        let support = proposal.support();
        let target_faces = proposal.target_faces().to_vec();
        drop(proposal);
        drop(guard);
        Ok::<_, String>((
            worker_permit,
            pose_capability,
            layer_capability,
            support,
            crossed_cells,
            target_faces,
            work,
        ))
    })
    .await
    .map_err(|_| ANALYSIS_FAILED_MESSAGE.to_owned())??;
    let (
        worker_permit,
        pose_capability,
        layer_capability,
        support,
        crossed_cells,
        target_faces,
        work,
    ) = analysis;

    {
        let project = lock_project(&app_state).map_err(|_| STALE_MESSAGE.to_owned())?;
        let pose_is_current = project
            .applied_pose_authority
            .revalidate_capability(&project, &pose_capability)
            .map_err(|_| STALE_MESSAGE.to_owned())?
            .is_some();
        let layer_is_current = revalidate_current_layer_order_capability(
            &foldability_state,
            &project,
            &layer_capability,
        )
        .map_err(|_| STALE_MESSAGE.to_owned())?
        .is_some();
        if !pose_is_current || !layer_is_current {
            return Err(STALE_MESSAGE.to_owned());
        }
    }
    drop(worker_permit);

    Ok(StackedFoldReadResponse {
        guard_model_id: ori_collision::STACKED_FOLD_READ_GUARD_MODEL_ID_V1,
        proposal_model_id: ori_collision::STACKED_FOLD_READ_PROPOSAL_MODEL_ID_V1,
        binding: StackedFoldReadBindingDto {
            project_instance_id: binding.project_instance_id(),
            project_id: binding.project_id(),
            source_revision: binding.source_revision(),
            pose_generation: binding.pose_generation(),
            layer_order_generation: binding.layer_order_generation(),
        },
        support: support.into(),
        crossed_cells,
        target_faces,
        work: StackedFoldReadWorkDto {
            scanned_cells: work.scanned_cells,
            total_boundary_vertices: work.total_boundary_vertices,
            total_layer_records: work.total_layer_records,
            orientation_tests: work.orientation_tests,
            exact_arithmetic_operations: work.exact_arithmetic_operations,
            maximum_exact_integer_bits: work.maximum_exact_integer_bits,
            total_exact_integer_bits: work.total_exact_integer_bits,
            retained_cells: work.retained_cells,
            retained_target_faces: work.retained_target_faces,
        },
        authorizes_project_mutation: false,
        authorizes_apply_stacked_fold: false,
    })
}

fn lowercase_hex(bytes: [u8; 32]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(64);
    for byte in bytes {
        output.push(char::from(DIGITS[usize::from(byte >> 4)]));
        output.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_schema_is_closed_and_rejects_non_finite_points() {
        let project_instance_id = ProjectId::new();
        let project_id = ProjectId::new();
        let json = serde_json::json!({
            "expectedProjectInstanceId": project_instance_id,
            "expectedProjectId": project_id,
            "expectedRevision": 7,
            "first": [10.0, 0.0, 0.0],
            "second": [10.0, 0.0, -20.0],
            "fixedSide": "left",
            "rotationDirection": "positive",
            "requestedAngleDegrees": 90.0
        });
        let request: StackedFoldReadRequest =
            serde_json::from_value(json.clone()).expect("valid request");
        assert_eq!(request.expected_revision, 7);
        assert!(
            StackedFoldLinearCandidateV1::new(
                Point3::new(request.first[0], request.first[1], request.first[2]).unwrap(),
                Point3::new(request.second[0], request.second[1], request.second[2]).unwrap(),
                request.fixed_side.into(),
                request.rotation_direction.into(),
                request.requested_angle_degrees,
            )
            .is_ok()
        );

        let mut unknown = json.clone();
        unknown
            .as_object_mut()
            .unwrap()
            .insert("future".to_owned(), serde_json::Value::Bool(true));
        assert!(serde_json::from_value::<StackedFoldReadRequest>(unknown).is_err());

        let mut non_finite = json;
        non_finite["first"][0] = serde_json::json!(f64::INFINITY);
        assert!(
            serde_json::from_value::<StackedFoldReadRequest>(non_finite)
                .ok()
                .and_then(|request| {
                    Point3::new(request.first[0], request.first[1], request.first[2]).ok()
                })
                .is_none()
        );
    }

    #[test]
    fn cell_keys_use_fixed_lowercase_sha256_hex() {
        let mut bytes = [0_u8; 32];
        bytes[0] = 0xab;
        bytes[31] = 0xef;
        let encoded = lowercase_hex(bytes);
        assert_eq!(encoded.len(), 64);
        assert!(encoded.starts_with("ab00"));
        assert!(encoded.ends_with("00ef"));
        assert!(encoded.bytes().all(|byte| byte.is_ascii_hexdigit()));
        assert!(!encoded.bytes().any(|byte| byte.is_ascii_uppercase()));
    }
}
