//! Private fingerprint and checkpoint helpers for the V2 pose issuer.

use sha2::{Digest, Sha256};

use super::*;

pub(super) fn decomposition_binding_matches_with_checkpoint_v2<Stop>(
    expected: [u8; 32],
    decomposition: &CanonicalMaterialEdgeBlockDecompositionV2,
    checkpoint: &mut impl FnMut() -> Result<(), Stop>,
) -> Result<bool, Stop> {
    Ok(
        decomposition_binding_candidate_with_checkpoint_v2(decomposition, checkpoint)?
            .is_some_and(|binding| binding == expected),
    )
}

fn decomposition_binding_candidate_with_checkpoint_v2<Stop>(
    decomposition: &CanonicalMaterialEdgeBlockDecompositionV2,
    checkpoint: &mut impl FnMut() -> Result<(), Stop>,
) -> Result<Option<[u8; 32]>, Stop> {
    let mut hash = Sha256::new();
    hash.update(b"common_articulation_pose_decomposition_v2");
    for value in [
        decomposition.limits().max_blocks,
        decomposition.limits().max_faces_per_block,
        decomposition.limits().max_hinges_per_block,
        decomposition.blocks().len(),
        decomposition.articulation_faces().len(),
    ] {
        let Ok(value) = u64::try_from(value) else {
            return Ok(None);
        };
        hash.update(value.to_le_bytes());
    }
    for face in decomposition.articulation_faces() {
        checkpoint()?;
        hash.update(face.canonical_bytes());
    }
    for block in decomposition.blocks() {
        checkpoint()?;
        for value in [
            block.geometry().face_ids().len(),
            block.geometry().hinges().len(),
        ] {
            let Ok(value) = u64::try_from(value) else {
                return Ok(None);
            };
            hash.update(value.to_le_bytes());
        }
        for face in block.geometry().face_ids() {
            checkpoint()?;
            hash.update(face.canonical_bytes());
        }
        for hinge in block.geometry().hinges() {
            checkpoint()?;
            hash.update(hinge.edge().canonical_bytes());
            hash.update(hinge.left_face().canonical_bytes());
            hash.update(hinge.right_face().canonical_bytes());
        }
    }
    Ok(Some(hash.finalize().into()))
}

pub(super) fn decomposition_binding_with_checkpoint_v2(
    decomposition: &CanonicalMaterialEdgeBlockDecompositionV2,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationPoseStopV2>,
) -> Result<[u8; 32], CommonArticulationPoseErrorV2> {
    decomposition_binding_candidate_with_checkpoint_v2(decomposition, &mut || {
        checkpoint_v2(checkpoint)
    })?
    .ok_or(CommonArticulationPoseErrorV2::ResourceLimit)
}

pub(super) fn pose_binding_with_checkpoint_v2(
    input: CommonArticulationPoseInputV2<'_>,
    profile_binding: [u8; 32],
    decomposition_binding: [u8; 32],
    logical_work: usize,
    retained_bytes: usize,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationPoseStopV2>,
) -> Result<[u8; 32], CommonArticulationPoseErrorV2> {
    let mut hash = Sha256::new();
    hash.update(COMMON_ARTICULATION_POSE_MODEL_ID_V2.as_bytes());
    hash.update(profile_binding);
    hash.update(decomposition_binding);
    hash.update(input.paper_thickness_mm.to_bits().to_le_bytes());
    update_usize_v2(&mut hash, logical_work)?;
    update_usize_v2(&mut hash, retained_bytes)?;
    hash.update(input.pose.fixed_face().canonical_bytes());
    for face in input.geometry.face_ids() {
        checkpoint_v2(checkpoint)?;
        hash.update(face.canonical_bytes());
    }
    for hinge in input.geometry.hinges() {
        checkpoint_v2(checkpoint)?;
        hash.update(hinge.edge().canonical_bytes());
        hash.update(hinge.left_face().canonical_bytes());
        hash.update(hinge.right_face().canonical_bytes());
    }
    for transform in input.pose.transforms() {
        checkpoint_v2(checkpoint)?;
        hash.update(transform.face().canonical_bytes());
        for bits in transform_bits_v2(transform.transform()) {
            hash.update(bits.to_le_bytes());
        }
    }
    for angle in input.pose.hinge_angles().as_slice() {
        checkpoint_v2(checkpoint)?;
        hash.update(angle.edge().canonical_bytes());
        hash.update(angle.angle_degrees().to_bits().to_le_bytes());
    }
    Ok(hash.finalize().into())
}

pub(super) fn transform_bits_v2(transform: crate::RigidTransform) -> [u64; 12] {
    let rows = transform.rotation_rows();
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
        transform.translation().x().to_bits(),
        transform.translation().y().to_bits(),
        transform.translation().z().to_bits(),
    ]
}

fn update_usize_v2(hash: &mut Sha256, value: usize) -> Result<(), CommonArticulationPoseErrorV2> {
    let value = u64::try_from(value).map_err(|_| CommonArticulationPoseErrorV2::ResourceLimit)?;
    hash.update(value.to_le_bytes());
    Ok(())
}

pub(super) fn checkpoint_v2(
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationPoseStopV2>,
) -> Result<(), CommonArticulationPoseErrorV2> {
    checkpoint().map_err(|stop| match stop {
        CommonArticulationPoseStopV2::Cancelled => CommonArticulationPoseErrorV2::Cancelled,
        CommonArticulationPoseStopV2::DeadlineExceeded => {
            CommonArticulationPoseErrorV2::DeadlineExceeded
        }
    })
}
