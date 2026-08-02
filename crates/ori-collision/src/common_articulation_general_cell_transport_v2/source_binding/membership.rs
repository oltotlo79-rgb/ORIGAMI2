//! Checkpointed membership scans used by V2 source-shape validation.

use ori_domain::FaceId;
use ori_foldability::{
    FacePairOrderSnapshot, FoldedFaceSnapshot, LayerFace, LayerOrderSnapshot, OverlapCellKey,
    OverlapCellSnapshot,
};

use super::super::checkpoint_v2;
use super::*;

pub(super) fn contains_geometry_face_v2(
    geometry: &MaterialHingeGraphGeometry,
    target: FaceId,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationGeneralCellTransportStopV2>,
) -> Result<bool, CommonArticulationGeneralCellTransportErrorV2> {
    for face in geometry.face_ids() {
        checkpoint_v2(checkpoint)?;
        if *face == target {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(super) fn contains_prior_face_id_v2(
    faces: &[LayerFace],
    target: FaceId,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationGeneralCellTransportStopV2>,
) -> Result<bool, CommonArticulationGeneralCellTransportErrorV2> {
    for face in faces {
        checkpoint_v2(checkpoint)?;
        if face.face_id == target {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(super) fn contains_material_face_id_v2(
    source: &LayerOrderSnapshot,
    target: FaceId,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationGeneralCellTransportStopV2>,
) -> Result<bool, CommonArticulationGeneralCellTransportErrorV2> {
    contains_prior_face_id_v2(&source.material_faces, target, checkpoint)
}

pub(super) fn contains_layer_face_v2(
    faces: &[LayerFace],
    target: &LayerFace,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationGeneralCellTransportStopV2>,
) -> Result<bool, CommonArticulationGeneralCellTransportErrorV2> {
    for face in faces {
        checkpoint_v2(checkpoint)?;
        if face == target {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(super) fn contains_prior_folded_face_id_v2(
    faces: &[FoldedFaceSnapshot],
    target: FaceId,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationGeneralCellTransportStopV2>,
) -> Result<bool, CommonArticulationGeneralCellTransportErrorV2> {
    for face in faces {
        checkpoint_v2(checkpoint)?;
        if face.face.face_id == target {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(super) fn contains_face_id_v2(
    faces: &[FaceId],
    target: FaceId,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationGeneralCellTransportStopV2>,
) -> Result<bool, CommonArticulationGeneralCellTransportErrorV2> {
    for face in faces {
        checkpoint_v2(checkpoint)?;
        if *face == target {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(super) fn contains_cell_key_v2(
    cells: &[OverlapCellKey],
    target: OverlapCellKey,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationGeneralCellTransportStopV2>,
) -> Result<bool, CommonArticulationGeneralCellTransportErrorV2> {
    for cell in cells {
        checkpoint_v2(checkpoint)?;
        if *cell == target {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(super) fn contains_overlap_cell_key_v2(
    cells: &[OverlapCellSnapshot],
    target: OverlapCellKey,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationGeneralCellTransportStopV2>,
) -> Result<bool, CommonArticulationGeneralCellTransportErrorV2> {
    for cell in cells {
        checkpoint_v2(checkpoint)?;
        if cell.cell_key == target {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(super) type DirectedPairOrderKeyV2 = ([u8; 32], [u8; 32], [u8; 16], [u8; 16]);

pub(super) fn directed_pair_order_key_v2(pair: &FacePairOrderSnapshot) -> DirectedPairOrderKeyV2 {
    (
        pair.lower_face.face_key.0,
        pair.upper_face.face_key.0,
        pair.lower_face.face_id.canonical_bytes(),
        pair.upper_face.face_id.canonical_bytes(),
    )
}

/// Looks up the opposite direction in a registry already proved to be in the
/// foldability issuer's canonical directed order. The search remains fully
/// checkpointed without allocating an auxiliary set.
pub(super) fn contains_reversed_pair_order_v2(
    pairs: &[FacePairOrderSnapshot],
    pair: &FacePairOrderSnapshot,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationGeneralCellTransportStopV2>,
) -> Result<bool, CommonArticulationGeneralCellTransportErrorV2> {
    let target = (
        pair.upper_face.face_key.0,
        pair.lower_face.face_key.0,
        pair.upper_face.face_id.canonical_bytes(),
        pair.lower_face.face_id.canonical_bytes(),
    );
    let mut start = 0usize;
    let mut end = pairs.len();
    while start < end {
        checkpoint_v2(checkpoint)?;
        let middle = start + (end - start) / 2;
        match directed_pair_order_key_v2(&pairs[middle]).cmp(&target) {
            std::cmp::Ordering::Less => start = middle + 1,
            std::cmp::Ordering::Greater => end = middle,
            std::cmp::Ordering::Equal => return Ok(true),
        }
    }
    Ok(false)
}
