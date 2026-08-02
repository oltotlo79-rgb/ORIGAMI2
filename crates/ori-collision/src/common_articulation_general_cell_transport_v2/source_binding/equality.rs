//! Checkpointed structural equality for retained source snapshots.

use ori_domain::FaceId;
use ori_foldability::{
    ExactAffineTransform, ExactPointValue, ExactRationalValue, FacePairOrderSnapshot,
    FoldedFaceSnapshot, LayerOrderProvenance, LayerOrderSnapshot, OverlapCellSnapshot,
};

use super::super::checkpoint_v2;
use super::*;

pub(crate) fn source_equal_with_checkpoint_v2(
    expected: &LayerOrderSnapshot,
    actual: &LayerOrderSnapshot,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationGeneralCellTransportStopV2>,
) -> Result<bool, CommonArticulationGeneralCellTransportErrorV2> {
    checkpoint_v2(checkpoint)?;
    if expected.model_id != actual.model_id
        || !provenance_equal_v2(&expected.provenance, &actual.provenance)
        || expected.reference_face != actual.reference_face
        || !proof_summary_equal_v2(expected.proof_summary, actual.proof_summary)
        || !layer_faces_equal_v2(&expected.material_faces, &actual.material_faces, checkpoint)?
        || !optional_layer_faces_equal_v2(
            expected.global_bottom_to_top.as_deref(),
            actual.global_bottom_to_top.as_deref(),
            checkpoint,
        )?
        || !folded_faces_equal_v2(&expected.folded_faces, &actual.folded_faces, checkpoint)?
        || !cells_equal_v2(&expected.overlap_cells, &actual.overlap_cells, checkpoint)?
        || !pair_orders_equal_v2(
            &expected.face_pair_orders,
            &actual.face_pair_orders,
            checkpoint,
        )?
    {
        checkpoint_v2(checkpoint)?;
        return Ok(false);
    }
    checkpoint_v2(checkpoint)?;
    Ok(true)
}

fn provenance_equal_v2(left: &LayerOrderProvenance, right: &LayerOrderProvenance) -> bool {
    left == right
}

fn proof_summary_equal_v2(
    left: Option<ori_foldability::FacewiseProofSummary>,
    right: Option<ori_foldability::FacewiseProofSummary>,
) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => {
            left.material_faces == right.material_faces
                && left.overlap_face_pairs == right.overlap_face_pairs
                && left.overlap_cells == right.overlap_cells
                && left.constraints == right.constraints
                // Search effort is historical telemetry, not a mathematical
                // certificate field. It is deliberately excluded from V2
                // source identity so search implementation changes replay.
                && left.maximum_ply == right.maximum_ply
                && left.certificate_bytes == right.certificate_bytes
        }
        (None, None) => true,
        _ => false,
    }
}

fn optional_layer_faces_equal_v2(
    expected: Option<&[ori_foldability::LayerFace]>,
    actual: Option<&[ori_foldability::LayerFace]>,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationGeneralCellTransportStopV2>,
) -> Result<bool, CommonArticulationGeneralCellTransportErrorV2> {
    match (expected, actual) {
        (Some(expected), Some(actual)) => layer_faces_equal_v2(expected, actual, checkpoint),
        (None, None) => Ok(true),
        _ => Ok(false),
    }
}

fn layer_faces_equal_v2(
    expected: &[ori_foldability::LayerFace],
    actual: &[ori_foldability::LayerFace],
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationGeneralCellTransportStopV2>,
) -> Result<bool, CommonArticulationGeneralCellTransportErrorV2> {
    if expected.len() != actual.len() {
        return Ok(false);
    }
    for (left, right) in expected.iter().zip(actual) {
        checkpoint_v2(checkpoint)?;
        if left != right {
            checkpoint_v2(checkpoint)?;
            return Ok(false);
        }
    }
    Ok(true)
}

fn folded_faces_equal_v2(
    expected: &[FoldedFaceSnapshot],
    actual: &[FoldedFaceSnapshot],
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationGeneralCellTransportStopV2>,
) -> Result<bool, CommonArticulationGeneralCellTransportErrorV2> {
    if expected.len() != actual.len() {
        return Ok(false);
    }
    for (left, right) in expected.iter().zip(actual) {
        checkpoint_v2(checkpoint)?;
        if left.face != right.face
            || left.orientation != right.orientation
            || !transform_equal_v2(&left.source_to_flat, &right.source_to_flat, checkpoint)?
        {
            checkpoint_v2(checkpoint)?;
            return Ok(false);
        }
    }
    Ok(true)
}

fn cells_equal_v2(
    expected: &[OverlapCellSnapshot],
    actual: &[OverlapCellSnapshot],
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationGeneralCellTransportStopV2>,
) -> Result<bool, CommonArticulationGeneralCellTransportErrorV2> {
    if expected.len() != actual.len() {
        return Ok(false);
    }
    for (left, right) in expected.iter().zip(actual) {
        checkpoint_v2(checkpoint)?;
        if left.cell_key != right.cell_key
            || !points_equal_v2(&left.exact_boundary, &right.exact_boundary, checkpoint)?
            || !layer_faces_equal_v2(&left.covering_faces, &right.covering_faces, checkpoint)?
            || !face_ids_equal_v2(
                &left.bottom_to_top_faces,
                &right.bottom_to_top_faces,
                checkpoint,
            )?
        {
            checkpoint_v2(checkpoint)?;
            return Ok(false);
        }
    }
    Ok(true)
}

fn pair_orders_equal_v2(
    expected: &[FacePairOrderSnapshot],
    actual: &[FacePairOrderSnapshot],
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationGeneralCellTransportStopV2>,
) -> Result<bool, CommonArticulationGeneralCellTransportErrorV2> {
    if expected.len() != actual.len() {
        return Ok(false);
    }
    for (left, right) in expected.iter().zip(actual) {
        checkpoint_v2(checkpoint)?;
        if left.lower_face != right.lower_face
            || left.upper_face != right.upper_face
            || !slice_equal_v2(&left.supporting_cells, &right.supporting_cells, checkpoint)?
        {
            checkpoint_v2(checkpoint)?;
            return Ok(false);
        }
    }
    Ok(true)
}

fn points_equal_v2(
    expected: &[ExactPointValue],
    actual: &[ExactPointValue],
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationGeneralCellTransportStopV2>,
) -> Result<bool, CommonArticulationGeneralCellTransportErrorV2> {
    if expected.len() != actual.len() {
        return Ok(false);
    }
    for (left, right) in expected.iter().zip(actual) {
        checkpoint_v2(checkpoint)?;
        if !rational_equal_v2(&left.x, &right.x, checkpoint)?
            || !rational_equal_v2(&left.y, &right.y, checkpoint)?
        {
            checkpoint_v2(checkpoint)?;
            return Ok(false);
        }
    }
    Ok(true)
}

fn transform_equal_v2(
    left: &ExactAffineTransform,
    right: &ExactAffineTransform,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationGeneralCellTransportStopV2>,
) -> Result<bool, CommonArticulationGeneralCellTransportErrorV2> {
    for (left, right) in [
        (&left.m00, &right.m00),
        (&left.m01, &right.m01),
        (&left.m10, &right.m10),
        (&left.m11, &right.m11),
        (&left.tx, &right.tx),
        (&left.ty, &right.ty),
    ] {
        if !rational_equal_v2(left, right, checkpoint)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn rational_equal_v2(
    left: &ExactRationalValue,
    right: &ExactRationalValue,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationGeneralCellTransportStopV2>,
) -> Result<bool, CommonArticulationGeneralCellTransportErrorV2> {
    if left.sign != right.sign {
        return Ok(false);
    }
    slice_equal_v2(
        &left.numerator_magnitude_be,
        &right.numerator_magnitude_be,
        checkpoint,
    )
    .and_then(|same| {
        if same {
            slice_equal_v2(&left.denominator_be, &right.denominator_be, checkpoint)
        } else {
            Ok(false)
        }
    })
}

fn face_ids_equal_v2(
    expected: &[FaceId],
    actual: &[FaceId],
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationGeneralCellTransportStopV2>,
) -> Result<bool, CommonArticulationGeneralCellTransportErrorV2> {
    slice_equal_v2(expected, actual, checkpoint)
}

fn slice_equal_v2<T: PartialEq>(
    expected: &[T],
    actual: &[T],
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationGeneralCellTransportStopV2>,
) -> Result<bool, CommonArticulationGeneralCellTransportErrorV2> {
    if expected.len() != actual.len() {
        return Ok(false);
    }
    for (left, right) in expected.iter().zip(actual) {
        checkpoint_v2(checkpoint)?;
        if left != right {
            checkpoint_v2(checkpoint)?;
            return Ok(false);
        }
    }
    Ok(true)
}
