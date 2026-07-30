use std::mem::size_of;

use ori_core::SpeculativeApproximateBlockingObservationV1;
use ori_domain::{EdgeId, FaceId, ProjectId};
use ori_foldability::fold_model_fingerprint_v1;

use super::{PostApplyProofJobV1, PostApplyProofPremiseV1};
use crate::ProjectState;

struct LivePostApplyProofBindingV1 {
    project_instance_id: ProjectId,
    project_id: ProjectId,
    revision: u64,
    target_fingerprint: [u8; 32],
    paper_thickness_bits: u64,
    pose_generation: u64,
    face_ids: Vec<FaceId>,
    hinge_ids: Vec<EdgeId>,
    fixed_face: Option<FaceId>,
    hinge_angles: Vec<(EdgeId, u64)>,
}

fn capture_live_binding_v1(project: &ProjectState) -> Result<LivePostApplyProofBindingV1, ()> {
    let capability = project
        .applied_pose_authority
        .capture_capability(project)
        .map_err(|_| ())?
        .ok_or(())?;
    let (model, pose) = capability.tree().ok_or(())?;
    Ok(LivePostApplyProofBindingV1 {
        project_instance_id: project.instance_id,
        project_id: project.project_id,
        revision: project.editor.revision(),
        target_fingerprint: fold_model_fingerprint_v1(
            project.editor.pattern(),
            project.editor.paper(),
        )
        .0,
        paper_thickness_bits: project.editor.paper().thickness_mm.to_bits(),
        pose_generation: capability.generation(),
        face_ids: model.face_ids().to_vec(),
        hinge_ids: model.hinges().iter().map(|hinge| hinge.edge()).collect(),
        fixed_face: pose.fixed_face(),
        hinge_angles: pose
            .hinge_angles()
            .iter()
            .map(|angle| (angle.edge(), angle.angle_degrees().to_bits()))
            .collect(),
    })
}

pub(super) fn premise_is_internally_bound_v1(premise: &PostApplyProofPremiseV1) -> bool {
    let initial = premise.requested.initial();
    let lineage = initial.target().geometry().proof().lineage();
    let target_pose = premise.requested.pose();
    let expected_target_generation = premise.binding.pose_generation().checked_add(1);
    lineage.identity_namespace() == premise.binding.project_id()
        && lineage.source_revision() == premise.binding.source_revision()
        && lineage.target_revision() == premise.target_revision
        && lineage.source_fingerprint().to_hex()
            == premise.binding.source_geometry_fingerprint_sha256()
        && lineage.target_fingerprint().0 == premise.target_fingerprint
        && expected_target_generation == Some(premise.target_pose_generation)
        && premise.binding.paper_thickness_bits() == premise.paper_thickness_mm.to_bits()
        && matches!(
            premise.binding.approximate_blocking_observation(),
            SpeculativeApproximateBlockingObservationV1::NoBlockingSampleObserved
        )
        && initial.target().model().owns_pose(initial.pose())
        && initial.target().model().owns_pose(target_pose)
}

fn job_matches_start_live_v1(
    job: &PostApplyProofJobV1,
    live: &LivePostApplyProofBindingV1,
) -> bool {
    let retained_premise_matches = job.premise.as_ref().is_none_or(|premise| {
        premise_is_internally_bound_v1(premise) && job.binding == premise.binding
    });
    retained_premise_matches
        && job.binding.source_revision().checked_add(1) == Some(job.target_revision)
        && job.binding.project_instance_id() == live.project_instance_id
        && job.binding.project_id() == live.project_id
        && job.target_revision == live.revision
        && job.target_fingerprint == live.target_fingerprint
        && job.binding.paper_thickness_bits() == live.paper_thickness_bits
        && job.target_pose_generation == live.pose_generation
        && job.expected_face_ids == live.face_ids
        && job.expected_hinge_ids == live.hinge_ids
        && job.expected_fixed_face == live.fixed_face
        && job.expected_hinge_angles == live.hinge_angles
}

pub(super) fn unstarted_job_matches_live_binding_v1(
    job: &PostApplyProofJobV1,
    project: &ProjectState,
) -> bool {
    job.frontend_started
        || capture_live_binding_v1(project)
            .as_ref()
            .is_ok_and(|live| job_matches_start_live_v1(job, live))
}

struct RetainedPremiseByteInputsV1 {
    initial_layer_order_bytes: usize,
    pattern_vertices: usize,
    pattern_edges: usize,
    boundary_vertices: usize,
    face_boundary_vertices: usize,
    faces: usize,
    hinges: usize,
    hinge_angles: usize,
    binding_fingerprint_bytes: usize,
}

fn checked_retained_premise_bytes_v1(inputs: RetainedPremiseByteInputsV1) -> Option<usize> {
    let mut bytes = size_of::<PostApplyProofJobV1>()
        .checked_add(size_of::<PostApplyProofPremiseV1>())?
        .checked_add(inputs.initial_layer_order_bytes)?
        .checked_add(
            inputs
                .pattern_vertices
                .checked_mul(size_of::<ori_domain::Vertex>())?,
        )?
        .checked_add(
            inputs
                .pattern_edges
                .checked_mul(size_of::<ori_domain::Edge>())?,
        )?
        .checked_add(
            inputs
                .boundary_vertices
                .checked_mul(size_of::<ori_domain::VertexId>())?,
        )?
        .checked_add(inputs.face_boundary_vertices.checked_mul(64)?)?
        .checked_add(inputs.faces.checked_mul(256)?)?
        .checked_add(inputs.hinges.checked_mul(512)?)?
        .checked_add(
            inputs
                .hinge_angles
                .checked_mul(size_of::<ori_kinematics::HingeAngle>())?,
        )?;
    // Account for both retained binding strings plus allocator slack. The
    // final multiplier also covers the semantic-pose vectors held first by
    // the one-shot ticket and later by the mutually exclusive typed proof.
    bytes = bytes.checked_add(inputs.binding_fingerprint_bytes.checked_mul(2)?)?;
    bytes.checked_mul(2)
}

pub(super) fn retained_premise_bytes_v1(premise: &PostApplyProofPremiseV1) -> Option<usize> {
    let initial = premise.requested.initial();
    let target = initial.target();
    let candidate = target.geometry().candidate();
    let model = target.model();
    let face_boundary_vertices = model.face_ids().iter().try_fold(0_usize, |sum, face| {
        sum.checked_add(model.face_boundary(*face)?.vertices().len())
    })?;
    checked_retained_premise_bytes_v1(RetainedPremiseByteInputsV1 {
        initial_layer_order_bytes: premise
            .initial_layer_order
            .retained_bytes_upper_bound_v1()?,
        pattern_vertices: candidate.pattern.vertices.len(),
        pattern_edges: candidate.pattern.edges.len(),
        boundary_vertices: candidate.paper.boundary_vertices.len(),
        face_boundary_vertices,
        faces: model.face_ids().len(),
        hinges: model.hinges().len(),
        hinge_angles: initial
            .pose()
            .hinge_angles()
            .len()
            .checked_add(premise.requested.pose().hinge_angles().len())?,
        binding_fingerprint_bytes: premise.binding.source_geometry_fingerprint_sha256().len(),
    })
}

#[cfg(test)]
pub(super) fn retained_premise_byte_overflow_is_rejected_for_test_v1() -> bool {
    checked_retained_premise_bytes_v1(RetainedPremiseByteInputsV1 {
        initial_layer_order_bytes: 0,
        pattern_vertices: usize::MAX,
        pattern_edges: 0,
        boundary_vertices: 0,
        face_boundary_vertices: 0,
        faces: 0,
        hinges: 0,
        hinge_angles: 0,
        binding_fingerprint_bytes: 0,
    })
    .is_none()
}
