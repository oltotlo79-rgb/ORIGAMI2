use std::collections::HashSet;

use ori_core::StackedFoldNonFlatLayerOrderV1;
use ori_domain::FaceId;
use thiserror::Error;

pub const NON_FLAT_CELL_TRANSPORT_MODEL_ID_V1: &str = "native_non_flat_exact_cell_transport_v1";

#[derive(Debug, Clone, PartialEq)]
pub struct NonFlatCellTransportProofV1 {
    source: StackedFoldNonFlatLayerOrderV1,
    target: StackedFoldNonFlatLayerOrderV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NonFlatCellTransportLimitsV1 {
    pub max_faces: usize,
    pub max_cells: usize,
    pub max_pairs: usize,
    pub max_boundary_points: usize,
}

impl Default for NonFlatCellTransportLimitsV1 {
    fn default() -> Self {
        Self {
            max_faces: 2_048,
            max_cells: 2_000_000,
            max_pairs: 2_000_000,
            max_boundary_points: 8_000_000,
        }
    }
}

impl NonFlatCellTransportProofV1 {
    #[must_use]
    pub const fn model_id(&self) -> &'static str {
        NON_FLAT_CELL_TRANSPORT_MODEL_ID_V1
    }
    #[must_use]
    pub fn target(&self) -> &StackedFoldNonFlatLayerOrderV1 {
        &self.target
    }
    #[must_use]
    pub fn is_for(
        &self,
        source: &StackedFoldNonFlatLayerOrderV1,
        target: &StackedFoldNonFlatLayerOrderV1,
    ) -> bool {
        self.source == *source && self.target == *target
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum NonFlatCellTransportErrorV1 {
    #[error("non-flat layer evidence is stale or belongs to another project")]
    BindingMismatch,
    #[error("non-flat exact face or cell coverage is incomplete")]
    IncompleteCoverage,
    #[error("non-flat cell order crosses or contradicts itself")]
    Crossing,
    #[error("non-flat cell transport exceeds its configured work bound")]
    ResourceLimit,
}

pub fn certify_non_flat_cell_transport_v1(
    source: &StackedFoldNonFlatLayerOrderV1,
    target: &StackedFoldNonFlatLayerOrderV1,
) -> Result<NonFlatCellTransportProofV1, NonFlatCellTransportErrorV1> {
    certify_non_flat_cell_transport_with_limits_v1(
        source,
        target,
        NonFlatCellTransportLimitsV1::default(),
    )
}

pub fn certify_non_flat_cell_transport_with_limits_v1(
    source: &StackedFoldNonFlatLayerOrderV1,
    target: &StackedFoldNonFlatLayerOrderV1,
    limits: NonFlatCellTransportLimitsV1,
) -> Result<NonFlatCellTransportProofV1, NonFlatCellTransportErrorV1> {
    if source.identity_namespace() != target.identity_namespace()
        || source.target_revision().checked_add(1) != Some(target.target_revision())
        || target.source_overlap_cells_authenticated() != source.overlap_cell_count()
    {
        return Err(NonFlatCellTransportErrorV1::BindingMismatch);
    }
    let boundary_points = target
        .overlap_cells()
        .iter()
        .try_fold(0usize, |sum, cell| {
            sum.checked_add(cell.exact_boundary().len())
        })
        .ok_or(NonFlatCellTransportErrorV1::ResourceLimit)?;
    preflight_non_flat_cell_transport_v1(
        target.material_faces().len(),
        target.overlap_cell_count(),
        target.face_pair_order_count(),
        boundary_points,
        limits,
    )?;
    validate_complete(target)?;
    Ok(NonFlatCellTransportProofV1 {
        source: source.clone(),
        target: target.clone(),
    })
}

pub fn preflight_non_flat_cell_transport_v1(
    faces: usize,
    cells: usize,
    pairs: usize,
    boundary_points: usize,
    limits: NonFlatCellTransportLimitsV1,
) -> Result<(), NonFlatCellTransportErrorV1> {
    if faces == 0
        || faces > limits.max_faces
        || cells > limits.max_cells
        || pairs > limits.max_pairs
        || boundary_points > limits.max_boundary_points
        || pairs != cells
    {
        return Err(NonFlatCellTransportErrorV1::ResourceLimit);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn work_preflight_is_inclusive_and_fail_closed() {
        let limits = NonFlatCellTransportLimitsV1 {
            max_faces: 16,
            max_cells: 32,
            max_pairs: 32,
            max_boundary_points: 128,
        };
        assert_eq!(
            preflight_non_flat_cell_transport_v1(16, 32, 32, 128, limits),
            Ok(())
        );
        for rejected in [
            (0, 0, 0, 0),
            (17, 32, 32, 128),
            (16, 33, 33, 128),
            (16, 32, 31, 128),
            (16, 32, 32, 129),
        ] {
            assert_eq!(
                preflight_non_flat_cell_transport_v1(
                    rejected.0, rejected.1, rejected.2, rejected.3, limits
                ),
                Err(NonFlatCellTransportErrorV1::ResourceLimit)
            );
        }
    }
}

fn validate_complete(
    value: &StackedFoldNonFlatLayerOrderV1,
) -> Result<(), NonFlatCellTransportErrorV1> {
    let faces = value
        .material_faces()
        .iter()
        .map(|face| face.face_id)
        .collect::<HashSet<_>>();
    if faces.is_empty()
        || faces.len() != value.material_faces().len()
        || value.folded_faces().len() != faces.len()
        || value
            .folded_faces()
            .iter()
            .map(|face| face.face().face_id)
            .collect::<HashSet<_>>()
            != faces
    {
        return Err(NonFlatCellTransportErrorV1::IncompleteCoverage);
    }
    for folded in value.folded_faces() {
        let transform = folded.source_to_plane();
        let values = [
            &transform.m00,
            &transform.m01,
            &transform.m10,
            &transform.m11,
            &transform.tx,
            &transform.ty,
        ]
        .into_iter()
        .map(|value| value.to_f64())
        .collect::<Option<Vec<_>>>()
        .ok_or(NonFlatCellTransportErrorV1::IncompleteCoverage)?;
        if folded.dropped_world_axis() > 2
            || (values[0] * values[3] - values[1] * values[2]).abs() <= f64::EPSILON
        {
            return Err(NonFlatCellTransportErrorV1::IncompleteCoverage);
        }
    }
    if value.overlap_cells().len() != value.face_pair_orders().len() {
        return Err(NonFlatCellTransportErrorV1::IncompleteCoverage);
    }
    let mut directed = HashSet::<(FaceId, FaceId)>::new();
    for (cell, pair) in value.overlap_cells().iter().zip(value.face_pair_orders()) {
        if cell.boundary().len() < 3
            || cell.exact_boundary().len() != cell.boundary().len()
            || cell.lower_face() != pair.lower_face()
            || cell.upper_face() != pair.upper_face()
            || !faces.contains(&cell.lower_face())
            || !faces.contains(&cell.upper_face())
            || cell.lower_face() == cell.upper_face()
        {
            return Err(NonFlatCellTransportErrorV1::IncompleteCoverage);
        }
        for (point, exact) in cell.boundary().iter().zip(cell.exact_boundary()) {
            if exact
                .x
                .to_f64()
                .is_none_or(|x| x.to_bits() != point.x.to_bits())
                || exact
                    .y
                    .to_f64()
                    .is_none_or(|y| y.to_bits() != point.y.to_bits())
            {
                return Err(NonFlatCellTransportErrorV1::IncompleteCoverage);
            }
        }
        if directed.contains(&(cell.upper_face(), cell.lower_face())) {
            return Err(NonFlatCellTransportErrorV1::Crossing);
        }
        directed.insert((cell.lower_face(), cell.upper_face()));
    }
    Ok(())
}
