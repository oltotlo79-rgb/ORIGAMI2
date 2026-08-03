use ori_topology::FoldAssignment;
use sha2::{Digest, Sha256};

use crate::{RigidTransform, TreeHinge};

use super::*;

type ErrorV2 = CanonicalBinary64PosePairTransformRealizationErrorV2;
type StopV2 = CanonicalBinary64PosePairTransformRealizationStopV2;

pub(super) fn audit_binding_with_checkpoint_v2(
    audit: &MaterialHingeGraphAudit,
    checkpoint: &mut impl FnMut() -> Result<(), StopV2>,
) -> Result<[u8; 32], ErrorV2> {
    resources::checkpoint_v2(checkpoint)?;
    let mut hash = Sha256::new();
    hash.update(b"ORIGAMI2_CANONICAL_BINARY64_POSE_PAIR_AUDIT_BINDING_V2");
    update_len_v2(&mut hash, audit.faces().len())?;
    for face in audit.faces() {
        resources::checkpoint_v2(checkpoint)?;
        hash.update(face.canonical_bytes());
    }
    update_len_v2(&mut hash, audit.spanning_hinges().len())?;
    for edge in audit.spanning_hinges() {
        resources::checkpoint_v2(checkpoint)?;
        hash.update(edge.canonical_bytes());
    }
    update_len_v2(&mut hash, audit.closure_hinges().len())?;
    for edge in audit.closure_hinges() {
        resources::checkpoint_v2(checkpoint)?;
        hash.update(edge.canonical_bytes());
    }
    resources::checkpoint_v2(checkpoint)?;
    Ok(hash.finalize().into())
}

pub(super) fn evidence_binding_with_checkpoint_v2(
    input: CanonicalBinary64PosePairTransformRealizationInputV2<'_>,
    audit_binding: [u8; 32],
    resources: CanonicalBinary64PosePairTransformRealizationResourcesV2,
    checkpoint: &mut impl FnMut() -> Result<(), StopV2>,
) -> Result<[u8; 32], ErrorV2> {
    resources::checkpoint_v2(checkpoint)?;
    let mut hash = Sha256::new();
    hash.update(CANONICAL_BINARY64_POSE_PAIR_TRANSFORM_REALIZATION_EVIDENCE_MODEL_ID_V2);
    hash.update(b"pose-arc-authority-retained-outside-fingerprint:canonical-binary64-spanning-tree-only:no-exact-closure-or-pose-realization");
    hash.update(input.fixed_face.canonical_bytes());
    hash.update(audit_binding);

    update_geometry_v2(&mut hash, input.geometry, checkpoint)?;
    update_pose_v2(&mut hash, b"lower", input.lower_pose, checkpoint)?;
    update_pose_v2(&mut hash, b"upper", input.upper_pose, checkpoint)?;
    for value in [
        resources.face_count,
        resources.hinge_count,
        resources.spanning_hinge_count,
        resources.pose_pair_deep_retained_bytes,
        resources.logical_work,
        resources.workspace_structural_requirement_bytes,
        input.limits.max_faces,
        input.limits.max_hinges,
        input.limits.max_pose_pair_deep_retained_bytes,
        input.limits.max_logical_work,
        input.limits.max_workspace_bytes,
    ] {
        update_usize_v2(&mut hash, value)?;
    }
    resources::checkpoint_v2(checkpoint)?;
    Ok(hash.finalize().into())
}

fn update_geometry_v2(
    hash: &mut Sha256,
    geometry: &MaterialHingeGraphGeometry,
    checkpoint: &mut impl FnMut() -> Result<(), StopV2>,
) -> Result<(), ErrorV2> {
    update_len_v2(hash, geometry.face_ids().len())?;
    for face in geometry.face_ids() {
        resources::checkpoint_v2(checkpoint)?;
        hash.update(face.canonical_bytes());
    }
    update_len_v2(hash, geometry.hinges().len())?;
    for hinge in geometry.hinges() {
        resources::checkpoint_v2(checkpoint)?;
        update_hinge_v2(hash, hinge);
    }
    Ok(())
}

fn update_hinge_v2(hash: &mut Sha256, hinge: &TreeHinge) {
    hash.update(hinge.edge().canonical_bytes());
    hash.update([match hinge.assignment() {
        FoldAssignment::Mountain => 0,
        FoldAssignment::Valley => 1,
    }]);
    hash.update(hinge.left_face().canonical_bytes());
    hash.update(hinge.right_face().canonical_bytes());
    for point in [hinge.start(), hinge.end(), hinge.axis()] {
        for value in [point.x(), point.y(), point.z()] {
            hash.update(value.to_bits().to_le_bytes());
        }
    }
}

fn update_pose_v2(
    hash: &mut Sha256,
    tag: &[u8],
    pose: &ClosedMaterialHingeGraphPose,
    checkpoint: &mut impl FnMut() -> Result<(), StopV2>,
) -> Result<(), ErrorV2> {
    hash.update(tag);
    hash.update(pose.fixed_face().canonical_bytes());
    update_len_v2(hash, pose.hinge_angles().as_slice().len())?;
    for angle in pose.hinge_angles().as_slice() {
        resources::checkpoint_v2(checkpoint)?;
        hash.update(angle.edge().canonical_bytes());
        hash.update(angle.angle_degrees().to_bits().to_le_bytes());
    }
    update_len_v2(hash, pose.transforms().len())?;
    for transform in pose.transforms() {
        resources::checkpoint_v2(checkpoint)?;
        hash.update(transform.face().canonical_bytes());
        for bits in transform_bits_v2(transform.transform()) {
            resources::checkpoint_v2(checkpoint)?;
            hash.update(bits.to_le_bytes());
        }
    }
    Ok(())
}

pub(super) fn transform_bits_v2(transform: RigidTransform) -> [u64; 12] {
    let rows = transform.rotation_rows();
    let translation = transform.translation();
    [
        rows[0][0].to_bits(),
        rows[0][1].to_bits(),
        rows[0][2].to_bits(),
        rows[1][0].to_bits(),
        rows[1][1].to_bits(),
        rows[1][2].to_bits(),
        rows[2][0].to_bits(),
        rows[2][1].to_bits(),
        rows[2][2].to_bits(),
        translation.x().to_bits(),
        translation.y().to_bits(),
        translation.z().to_bits(),
    ]
}

fn update_len_v2(hash: &mut Sha256, value: usize) -> Result<(), ErrorV2> {
    update_usize_v2(hash, value)
}

fn update_usize_v2(hash: &mut Sha256, value: usize) -> Result<(), ErrorV2> {
    hash.update(
        u64::try_from(value)
            .map_err(|_| ErrorV2::ResourceLimit)?
            .to_le_bytes(),
    );
    Ok(())
}
