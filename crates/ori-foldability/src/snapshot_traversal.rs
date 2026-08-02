use super::*;

fn poll_slice<T, E>(values: &[T], checkpoint: &mut impl FnMut() -> Result<(), E>) -> Result<(), E> {
    for _ in values {
        checkpoint()?;
    }
    Ok(())
}

fn checked_add_vec<T>(total: &mut usize, elements: usize) -> Option<()> {
    let bytes = std::mem::size_of::<T>().checked_mul(elements)?;
    *total = total.checked_add(bytes)?;
    Some(())
}

fn checked_add_rational_projected<E>(
    total: &mut usize,
    value: &ExactRationalValue,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Option<()>, E> {
    poll_slice(&value.numerator_magnitude_be, checkpoint)?;
    poll_slice(&value.denominator_be, checkpoint)?;
    Ok(
        checked_add_vec::<u8>(total, value.numerator_magnitude_be.len())
            .and_then(|()| checked_add_vec::<u8>(total, value.denominator_be.len())),
    )
}

fn checked_add_point_projected<E>(
    total: &mut usize,
    value: &ExactPointValue,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Option<()>, E> {
    if checked_add_rational_projected(total, &value.x, checkpoint)?.is_none() {
        return Ok(None);
    }
    checked_add_rational_projected(total, &value.y, checkpoint)
}

fn checked_add_transform_projected<E>(
    total: &mut usize,
    value: &ExactAffineTransform,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Option<()>, E> {
    for coefficient in [
        &value.m00, &value.m01, &value.m10, &value.m11, &value.tx, &value.ty,
    ] {
        checkpoint()?;
        if checked_add_rational_projected(total, coefficient, checkpoint)?.is_none() {
            return Ok(None);
        }
    }
    Ok(Some(()))
}

struct RetainedByteLimitLedgerV2 {
    total: usize,
    maximum: usize,
}

impl RetainedByteLimitLedgerV2 {
    const fn new(maximum: usize) -> Self {
        Self {
            total: std::mem::size_of::<LayerOrderSnapshot>(),
            maximum,
        }
    }

    const fn exceeded(&self) -> Option<LayerOrderSnapshotRetainedByteLimitV2> {
        if self.total > self.maximum {
            Some(LayerOrderSnapshotRetainedByteLimitV2::Exceeded {
                observed_lower_bound: self.total,
            })
        } else {
            None
        }
    }

    fn add_vec<T>(&mut self, capacity: usize) -> Option<LayerOrderSnapshotRetainedByteLimitV2> {
        let Some(bytes) = std::mem::size_of::<T>().checked_mul(capacity) else {
            return Some(LayerOrderSnapshotRetainedByteLimitV2::Exceeded {
                observed_lower_bound: usize::MAX,
            });
        };
        let Some(total) = self.total.checked_add(bytes) else {
            return Some(LayerOrderSnapshotRetainedByteLimitV2::Exceeded {
                observed_lower_bound: usize::MAX,
            });
        };
        self.total = total;
        self.exceeded()
    }
}

fn checked_add_rational_with_limit<E>(
    ledger: &mut RetainedByteLimitLedgerV2,
    value: &ExactRationalValue,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Option<LayerOrderSnapshotRetainedByteLimitV2>, E> {
    if let Some(exceeded) = ledger.add_vec::<u8>(value.numerator_magnitude_be.capacity()) {
        return Ok(Some(exceeded));
    }
    poll_slice(&value.numerator_magnitude_be, checkpoint)?;
    if let Some(exceeded) = ledger.add_vec::<u8>(value.denominator_be.capacity()) {
        return Ok(Some(exceeded));
    }
    poll_slice(&value.denominator_be, checkpoint)?;
    Ok(None)
}

fn checked_add_point_with_limit<E>(
    ledger: &mut RetainedByteLimitLedgerV2,
    value: &ExactPointValue,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Option<LayerOrderSnapshotRetainedByteLimitV2>, E> {
    if let Some(exceeded) = checked_add_rational_with_limit(ledger, &value.x, checkpoint)? {
        return Ok(Some(exceeded));
    }
    checked_add_rational_with_limit(ledger, &value.y, checkpoint)
}

fn checked_add_transform_with_limit<E>(
    ledger: &mut RetainedByteLimitLedgerV2,
    value: &ExactAffineTransform,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Option<LayerOrderSnapshotRetainedByteLimitV2>, E> {
    for coefficient in [
        &value.m00, &value.m01, &value.m10, &value.m11, &value.tx, &value.ty,
    ] {
        checkpoint()?;
        if let Some(exceeded) = checked_add_rational_with_limit(ledger, coefficient, checkpoint)? {
            return Ok(Some(exceeded));
        }
    }
    Ok(None)
}

pub(super) fn checked_deep_retained_bytes_with_limit_and_checkpoint_v2<E>(
    snapshot: &LayerOrderSnapshot,
    maximum: usize,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<LayerOrderSnapshotRetainedByteLimitV2, E> {
    checkpoint()?;
    let mut ledger = RetainedByteLimitLedgerV2::new(maximum);
    if let Some(exceeded) = ledger.exceeded() {
        return Ok(exceeded);
    }
    if let Some(exceeded) = ledger.add_vec::<LayerFace>(snapshot.material_faces.capacity()) {
        return Ok(exceeded);
    }
    poll_slice(&snapshot.material_faces, checkpoint)?;
    if let Some(global) = &snapshot.global_bottom_to_top {
        if let Some(exceeded) = ledger.add_vec::<LayerFace>(global.capacity()) {
            return Ok(exceeded);
        }
        poll_slice(global, checkpoint)?;
    }
    if let Some(exceeded) = ledger.add_vec::<FoldedFaceSnapshot>(snapshot.folded_faces.capacity()) {
        return Ok(exceeded);
    }
    for folded in &snapshot.folded_faces {
        checkpoint()?;
        if let Some(exceeded) =
            checked_add_transform_with_limit(&mut ledger, &folded.source_to_flat, checkpoint)?
        {
            return Ok(exceeded);
        }
    }
    if let Some(exceeded) = ledger.add_vec::<OverlapCellSnapshot>(snapshot.overlap_cells.capacity())
    {
        return Ok(exceeded);
    }
    for cell in &snapshot.overlap_cells {
        checkpoint()?;
        if let Some(exceeded) = ledger.add_vec::<ExactPointValue>(cell.exact_boundary.capacity()) {
            return Ok(exceeded);
        }
        for point in &cell.exact_boundary {
            checkpoint()?;
            if let Some(exceeded) = checked_add_point_with_limit(&mut ledger, point, checkpoint)? {
                return Ok(exceeded);
            }
        }
        if let Some(exceeded) = ledger.add_vec::<LayerFace>(cell.covering_faces.capacity()) {
            return Ok(exceeded);
        }
        poll_slice(&cell.covering_faces, checkpoint)?;
        if let Some(exceeded) = ledger.add_vec::<FaceId>(cell.bottom_to_top_faces.capacity()) {
            return Ok(exceeded);
        }
        poll_slice(&cell.bottom_to_top_faces, checkpoint)?;
    }
    if let Some(exceeded) =
        ledger.add_vec::<FacePairOrderSnapshot>(snapshot.face_pair_orders.capacity())
    {
        return Ok(exceeded);
    }
    for pair in &snapshot.face_pair_orders {
        checkpoint()?;
        if let Some(exceeded) = ledger.add_vec::<OverlapCellKey>(pair.supporting_cells.capacity()) {
            return Ok(exceeded);
        }
        poll_slice(&pair.supporting_cells, checkpoint)?;
    }
    checkpoint()?;
    Ok(LayerOrderSnapshotRetainedByteLimitV2::WithinLimit {
        retained_bytes: ledger.total,
    })
}

pub(super) fn checked_deep_retained_bytes_with_checkpoint_v2<E>(
    snapshot: &LayerOrderSnapshot,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Option<usize>, E> {
    Ok(
        match checked_deep_retained_bytes_with_limit_and_checkpoint_v2(
            snapshot,
            usize::MAX,
            checkpoint,
        )? {
            LayerOrderSnapshotRetainedByteLimitV2::WithinLimit { retained_bytes } => {
                Some(retained_bytes)
            }
            LayerOrderSnapshotRetainedByteLimitV2::Exceeded { .. } => None,
        },
    )
}

fn checked_projected_bytes_with_checkpoint_v2<E>(
    snapshot: &LayerOrderSnapshot,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Option<usize>, E> {
    checkpoint()?;
    let mut total = std::mem::size_of::<LayerOrderSnapshot>();
    if checked_add_vec::<LayerFace>(&mut total, snapshot.material_faces.len()).is_none() {
        return Ok(None);
    }
    poll_slice(&snapshot.material_faces, checkpoint)?;
    if let Some(global) = &snapshot.global_bottom_to_top {
        if checked_add_vec::<LayerFace>(&mut total, global.len()).is_none() {
            return Ok(None);
        }
        poll_slice(global, checkpoint)?;
    }
    if checked_add_vec::<FoldedFaceSnapshot>(&mut total, snapshot.folded_faces.len()).is_none() {
        return Ok(None);
    }
    for folded in &snapshot.folded_faces {
        checkpoint()?;
        if checked_add_transform_projected(&mut total, &folded.source_to_flat, checkpoint)?
            .is_none()
        {
            return Ok(None);
        }
    }
    if checked_add_vec::<OverlapCellSnapshot>(&mut total, snapshot.overlap_cells.len()).is_none() {
        return Ok(None);
    }
    for cell in &snapshot.overlap_cells {
        checkpoint()?;
        if checked_add_vec::<ExactPointValue>(&mut total, cell.exact_boundary.len()).is_none() {
            return Ok(None);
        }
        for point in &cell.exact_boundary {
            checkpoint()?;
            if checked_add_point_projected(&mut total, point, checkpoint)?.is_none() {
                return Ok(None);
            }
        }
        if checked_add_vec::<LayerFace>(&mut total, cell.covering_faces.len()).is_none()
            || checked_add_vec::<FaceId>(&mut total, cell.bottom_to_top_faces.len()).is_none()
        {
            return Ok(None);
        }
        poll_slice(&cell.covering_faces, checkpoint)?;
        poll_slice(&cell.bottom_to_top_faces, checkpoint)?;
    }
    if checked_add_vec::<FacePairOrderSnapshot>(&mut total, snapshot.face_pair_orders.len())
        .is_none()
    {
        return Ok(None);
    }
    for pair in &snapshot.face_pair_orders {
        checkpoint()?;
        if checked_add_vec::<OverlapCellKey>(&mut total, pair.supporting_cells.len()).is_none() {
            return Ok(None);
        }
        poll_slice(&pair.supporting_cells, checkpoint)?;
    }
    checkpoint()?;
    Ok(Some(total))
}

enum CheckpointedCloneError<E> {
    Stop(E),
    Clone(LayerOrderSnapshotCloneErrorV1),
}

fn poll_clone<E>(
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<(), CheckpointedCloneError<E>> {
    checkpoint().map_err(CheckpointedCloneError::Stop)
}

fn allocate<T, E>(
    budget: &mut LayerOrderSnapshotCloneBudgetV1,
    capacity: usize,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Vec<T>, CheckpointedCloneError<E>> {
    poll_clone(checkpoint)?;
    budget
        .try_vec_with_exact_capacity(capacity)
        .map_err(CheckpointedCloneError::Clone)
}

fn clone_exact_bytes<E>(
    source: &[u8],
    budget: &mut LayerOrderSnapshotCloneBudgetV1,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Vec<u8>, CheckpointedCloneError<E>> {
    let mut cloned = allocate(budget, source.len(), checkpoint)?;
    for byte in source {
        poll_clone(checkpoint)?;
        cloned.push(*byte);
    }
    Ok(cloned)
}

fn clone_rational<E>(
    source: &ExactRationalValue,
    budget: &mut LayerOrderSnapshotCloneBudgetV1,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<ExactRationalValue, CheckpointedCloneError<E>> {
    poll_clone(checkpoint)?;
    Ok(ExactRationalValue {
        sign: source.sign,
        numerator_magnitude_be: clone_exact_bytes(
            &source.numerator_magnitude_be,
            budget,
            checkpoint,
        )?,
        denominator_be: clone_exact_bytes(&source.denominator_be, budget, checkpoint)?,
    })
}

fn clone_point<E>(
    source: &ExactPointValue,
    budget: &mut LayerOrderSnapshotCloneBudgetV1,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<ExactPointValue, CheckpointedCloneError<E>> {
    poll_clone(checkpoint)?;
    Ok(ExactPointValue {
        x: clone_rational(&source.x, budget, checkpoint)?,
        y: clone_rational(&source.y, budget, checkpoint)?,
    })
}

fn clone_transform<E>(
    source: &ExactAffineTransform,
    budget: &mut LayerOrderSnapshotCloneBudgetV1,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<ExactAffineTransform, CheckpointedCloneError<E>> {
    poll_clone(checkpoint)?;
    Ok(ExactAffineTransform {
        m00: clone_rational(&source.m00, budget, checkpoint)?,
        m01: clone_rational(&source.m01, budget, checkpoint)?,
        m10: clone_rational(&source.m10, budget, checkpoint)?,
        m11: clone_rational(&source.m11, budget, checkpoint)?,
        tx: clone_rational(&source.tx, budget, checkpoint)?,
        ty: clone_rational(&source.ty, budget, checkpoint)?,
    })
}

fn clone_snapshot<E>(
    source: &LayerOrderSnapshot,
    budget: &mut LayerOrderSnapshotCloneBudgetV1,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<LayerOrderSnapshot, CheckpointedCloneError<E>> {
    let mut material_faces = allocate(budget, source.material_faces.len(), checkpoint)?;
    for face in &source.material_faces {
        poll_clone(checkpoint)?;
        material_faces.push(*face);
    }

    let global_bottom_to_top = if let Some(global) = &source.global_bottom_to_top {
        let mut cloned = allocate(budget, global.len(), checkpoint)?;
        for face in global {
            poll_clone(checkpoint)?;
            cloned.push(*face);
        }
        Some(cloned)
    } else {
        None
    };

    let mut folded_faces = allocate(budget, source.folded_faces.len(), checkpoint)?;
    for folded in &source.folded_faces {
        poll_clone(checkpoint)?;
        folded_faces.push(FoldedFaceSnapshot {
            face: folded.face,
            source_to_flat: clone_transform(&folded.source_to_flat, budget, checkpoint)?,
            orientation: folded.orientation,
        });
    }

    let mut overlap_cells = allocate(budget, source.overlap_cells.len(), checkpoint)?;
    for cell in &source.overlap_cells {
        poll_clone(checkpoint)?;
        let mut exact_boundary = allocate(budget, cell.exact_boundary.len(), checkpoint)?;
        for point in &cell.exact_boundary {
            poll_clone(checkpoint)?;
            exact_boundary.push(clone_point(point, budget, checkpoint)?);
        }
        let mut covering_faces = allocate(budget, cell.covering_faces.len(), checkpoint)?;
        for face in &cell.covering_faces {
            poll_clone(checkpoint)?;
            covering_faces.push(*face);
        }
        let mut bottom_to_top_faces = allocate(budget, cell.bottom_to_top_faces.len(), checkpoint)?;
        for face in &cell.bottom_to_top_faces {
            poll_clone(checkpoint)?;
            bottom_to_top_faces.push(*face);
        }
        overlap_cells.push(OverlapCellSnapshot {
            cell_key: cell.cell_key,
            exact_boundary,
            covering_faces,
            bottom_to_top_faces,
        });
    }

    let mut face_pair_orders = allocate(budget, source.face_pair_orders.len(), checkpoint)?;
    for pair in &source.face_pair_orders {
        poll_clone(checkpoint)?;
        let mut supporting_cells = allocate(budget, pair.supporting_cells.len(), checkpoint)?;
        for cell in &pair.supporting_cells {
            poll_clone(checkpoint)?;
            supporting_cells.push(*cell);
        }
        face_pair_orders.push(FacePairOrderSnapshot {
            lower_face: pair.lower_face,
            upper_face: pair.upper_face,
            supporting_cells,
        });
    }

    poll_clone(checkpoint)?;
    Ok(LayerOrderSnapshot {
        model_id: source.model_id,
        material_faces,
        global_bottom_to_top,
        provenance: source.provenance,
        reference_face: source.reference_face,
        folded_faces,
        overlap_cells,
        face_pair_orders,
        proof_summary: source.proof_summary,
    })
}

pub(super) fn try_clone_with_retained_byte_limit_with_checkpoint_v2<E>(
    source: &LayerOrderSnapshot,
    maximum: usize,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Result<LayerOrderSnapshot, LayerOrderSnapshotCloneErrorV1>, E> {
    let projected = match checked_projected_bytes_with_checkpoint_v2(source, checkpoint)? {
        Some(projected) => projected,
        None => return Ok(Err(LayerOrderSnapshotCloneErrorV1::SizeOverflow)),
    };
    if let Err(error) = check_layer_order_snapshot_byte_limit_v1(projected, maximum) {
        return Ok(Err(error));
    }
    let mut budget = match LayerOrderSnapshotCloneBudgetV1::new(maximum) {
        Ok(budget) => budget,
        Err(error) => return Ok(Err(error)),
    };
    let cloned = match clone_snapshot(source, &mut budget, checkpoint) {
        Ok(cloned) => cloned,
        Err(CheckpointedCloneError::Stop(stop)) => return Err(stop),
        Err(CheckpointedCloneError::Clone(error)) => return Ok(Err(error)),
    };
    let observed = match checked_deep_retained_bytes_with_checkpoint_v2(&cloned, checkpoint)? {
        Some(observed) => observed,
        None => return Ok(Err(LayerOrderSnapshotCloneErrorV1::SizeOverflow)),
    };
    if observed != budget.observed {
        return Ok(Err(LayerOrderSnapshotCloneErrorV1::SizeOverflow));
    }
    if let Err(error) = check_layer_order_snapshot_byte_limit_v1(observed, maximum) {
        return Ok(Err(error));
    }
    checkpoint()?;
    Ok(Ok(cloned))
}
