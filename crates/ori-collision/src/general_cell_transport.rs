use ori_domain::Point2;
use ori_foldability::{FoldedFaceOrientation, LayerOrderSnapshot};
use ori_kinematics::{
    CanonicalCycleScheduleV1, DyadicMaterialHingeIntervalClosureCertificateV1,
    MaterialHingeGraphAudit, MaterialHingeGraphGeometry, Point3,
};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::PositiveThicknessContinuousCertificateV1;

pub const GENERAL_MULTI_FACE_CELL_TRANSPORT_MODEL_ID_V1: &str =
    "general_multi_face_positive_thickness_cell_transport_v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GeneralCellTransportLimitsV1 {
    pub max_transitions: usize,
    pub max_cells: usize,
    pub max_layer_records: usize,
    pub max_boundary_samples: usize,
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum GeneralCellTransportErrorV1 {
    #[error("cell transport authority is stale, foreign, or malformed")]
    BindingMismatch,
    #[error("cell transport exceeds its resource limit")]
    ResourceLimit,
    #[error("cell geometry is degenerate or unavailable")]
    GeometryUnavailable,
    #[error("positive-thickness cell order crosses at a checkpoint")]
    Crossing,
}

pub struct GeneralCellTransportInputV1<'a> {
    pub geometry: &'a MaterialHingeGraphGeometry,
    pub audit: &'a MaterialHingeGraphAudit,
    pub source: &'a LayerOrderSnapshot,
    pub schedule: &'a CanonicalCycleScheduleV1,
    pub closure: &'a DyadicMaterialHingeIntervalClosureCertificateV1,
    pub positive_continuous: &'a PositiveThicknessContinuousCertificateV1,
    pub paper_thickness_mm: f64,
    pub tolerance: f64,
    pub limits: GeneralCellTransportLimitsV1,
}

#[derive(Debug, Clone)]
pub struct GeneralMultiFaceCellTransportProofV1 {
    issuer: MaterialHingeGraphGeometry,
    source_instance: usize,
    source: LayerOrderSnapshot,
    schedule_hash: [u8; 32],
    closure_hash: [u8; 32],
    thickness_bits: u64,
    checkpoint_hashes: Vec<[u8; 32]>,
}

impl GeneralMultiFaceCellTransportProofV1 {
    #[must_use]
    pub const fn model_id(&self) -> &'static str {
        GENERAL_MULTI_FACE_CELL_TRANSPORT_MODEL_ID_V1
    }

    #[must_use]
    pub fn checkpoint_hashes(&self) -> &[[u8; 32]] {
        &self.checkpoint_hashes
    }

    #[must_use]
    pub fn is_for(
        &self,
        geometry: &MaterialHingeGraphGeometry,
        source: &LayerOrderSnapshot,
        schedule: &CanonicalCycleScheduleV1,
        closure: &DyadicMaterialHingeIntervalClosureCertificateV1,
        thickness: f64,
    ) -> bool {
        self.issuer.same_instance(geometry)
            && self.source_instance == source as *const LayerOrderSnapshot as usize
            && self.source == *source
            && self.schedule_hash == schedule.certificate_binding_fingerprint_v1()
            && self.closure_hash == closure.partition_binding_fingerprint_v1()
            && self.thickness_bits == thickness.to_bits()
    }
}

pub fn certify_general_multi_face_cell_transport_v1(
    input: GeneralCellTransportInputV1<'_>,
) -> Result<GeneralMultiFaceCellTransportProofV1, GeneralCellTransportErrorV1> {
    if !input.paper_thickness_mm.is_finite()
        || input.paper_thickness_mm <= 0.0
        || !input.tolerance.is_finite()
        || input.tolerance < 0.0
        || !input.positive_continuous.is_for(
            input.geometry,
            input.closure.fixed_face(),
            input.schedule,
            input.closure,
            input.paper_thickness_mm,
        )
    {
        return Err(GeneralCellTransportErrorV1::BindingMismatch);
    }
    let transition_count = input
        .closure
        .leaves()
        .len()
        .checked_add(1)
        .ok_or(GeneralCellTransportErrorV1::ResourceLimit)?;
    let layer_records = input
        .source
        .overlap_cells
        .iter()
        .try_fold(0usize, |sum, cell| {
            sum.checked_add(cell.bottom_to_top_faces.len())
        })
        .ok_or(GeneralCellTransportErrorV1::ResourceLimit)?;
    let boundary_samples = input
        .source
        .overlap_cells
        .iter()
        .try_fold(0usize, |sum, cell| {
            cell.exact_boundary
                .len()
                .checked_mul(cell.bottom_to_top_faces.len())
                .and_then(|work| sum.checked_add(work))
        })
        .and_then(|work| work.checked_mul(transition_count))
        .ok_or(GeneralCellTransportErrorV1::ResourceLimit)?;
    if transition_count > input.limits.max_transitions
        || input.source.overlap_cells.len() > input.limits.max_cells
        || layer_records > input.limits.max_layer_records
        || boundary_samples > input.limits.max_boundary_samples
    {
        return Err(GeneralCellTransportErrorV1::ResourceLimit);
    }
    let folded = input
        .source
        .folded_faces
        .iter()
        .map(|face| (face.face.face_id, face))
        .collect::<std::collections::HashMap<_, _>>();
    if folded.len() != input.source.material_faces.len() {
        return Err(GeneralCellTransportErrorV1::BindingMismatch);
    }
    let mut parameters = input
        .closure
        .leaves()
        .iter()
        .map(|(depth, index, _)| *index as f64 / 2_f64.powi(*depth as i32))
        .collect::<Vec<_>>();
    parameters.push(1.0);
    let mut checkpoint_hashes = Vec::with_capacity(parameters.len());
    for parameter in parameters {
        let angles = input
            .schedule
            .evaluate(parameter)
            .ok_or(GeneralCellTransportErrorV1::BindingMismatch)?;
        let pose = input
            .geometry
            .solve_closed(
                input.audit,
                input.closure.fixed_face(),
                &angles,
                input.tolerance.max(1.0e-12),
            )
            .map_err(|_| GeneralCellTransportErrorV1::BindingMismatch)?;
        let mut hash = Sha256::new();
        hash.update(GENERAL_MULTI_FACE_CELL_TRANSPORT_MODEL_ID_V1.as_bytes());
        hash.update(parameter.to_bits().to_be_bytes());
        for cell in &input.source.overlap_cells {
            if cell.bottom_to_top_faces.is_empty()
                || cell.exact_boundary.len() < 3
                || cell.covering_faces.len() != cell.bottom_to_top_faces.len()
            {
                return Err(GeneralCellTransportErrorV1::BindingMismatch);
            }
            let count = cell.bottom_to_top_faces.len();
            let mut layer_boundaries = Vec::with_capacity(count);
            for (rank, face) in cell.bottom_to_top_faces.iter().copied().enumerate() {
                let folded_face = folded
                    .get(&face)
                    .ok_or(GeneralCellTransportErrorV1::BindingMismatch)?;
                let transform = pose
                    .face_transform(face)
                    .ok_or(GeneralCellTransportErrorV1::BindingMismatch)?;
                let sign = match folded_face.orientation {
                    FoldedFaceOrientation::FrontUp => 1.0,
                    FoldedFaceOrientation::BackUp => -1.0,
                };
                let normal = transform
                    .apply_vector(
                        Point3::new(0.0, 0.0, sign)
                            .map_err(|_| GeneralCellTransportErrorV1::GeometryUnavailable)?,
                    )
                    .map_err(|_| GeneralCellTransportErrorV1::GeometryUnavailable)?;
                let offset = (rank as f64 - (count - 1) as f64 * 0.5) * input.paper_thickness_mm;
                let mut layer_boundary = Vec::with_capacity(cell.exact_boundary.len());
                for point in &cell.exact_boundary {
                    let flat = Point2::new(
                        point
                            .x
                            .to_f64()
                            .ok_or(GeneralCellTransportErrorV1::GeometryUnavailable)?,
                        point
                            .y
                            .to_f64()
                            .ok_or(GeneralCellTransportErrorV1::GeometryUnavailable)?,
                    );
                    let material = inverse_flat_point(folded_face, flat)?;
                    let world = transform
                        .apply_point(material)
                        .map_err(|_| GeneralCellTransportErrorV1::GeometryUnavailable)?;
                    let offset_world = [
                        world.x() + normal.x() * offset,
                        world.y() + normal.y() * offset,
                        world.z() + normal.z() * offset,
                    ];
                    for value in offset_world {
                        hash.update(value.to_bits().to_be_bytes());
                    }
                    layer_boundary.push(offset_world);
                }
                layer_boundaries.push(layer_boundary);
                hash.update(face.canonical_bytes());
            }
            for pair in layer_boundaries.windows(2) {
                for (lower, upper) in pair[0].iter().zip(&pair[1]) {
                    let separation = ((upper[0] - lower[0]).powi(2)
                        + (upper[1] - lower[1]).powi(2)
                        + (upper[2] - lower[2]).powi(2))
                    .sqrt();
                    if separation + input.tolerance < input.paper_thickness_mm {
                        return Err(GeneralCellTransportErrorV1::Crossing);
                    }
                }
            }
            hash.update(cell.cell_key.0);
        }
        checkpoint_hashes.push(hash.finalize().into());
    }
    Ok(GeneralMultiFaceCellTransportProofV1 {
        issuer: input.geometry.clone(),
        source_instance: input.source as *const LayerOrderSnapshot as usize,
        source: input.source.clone(),
        schedule_hash: input.schedule.certificate_binding_fingerprint_v1(),
        closure_hash: input.closure.partition_binding_fingerprint_v1(),
        thickness_bits: input.paper_thickness_mm.to_bits(),
        checkpoint_hashes,
    })
}

fn inverse_flat_point(
    folded: &ori_foldability::FoldedFaceSnapshot,
    flat: Point2,
) -> Result<Point3, GeneralCellTransportErrorV1> {
    let value = &folded.source_to_flat;
    let values = [
        value.m00.to_f64(),
        value.m01.to_f64(),
        value.m10.to_f64(),
        value.m11.to_f64(),
        value.tx.to_f64(),
        value.ty.to_f64(),
    ];
    let [
        Some(m00),
        Some(m01),
        Some(m10),
        Some(m11),
        Some(tx),
        Some(ty),
    ] = values
    else {
        return Err(GeneralCellTransportErrorV1::GeometryUnavailable);
    };
    let determinant = m00 * m11 - m01 * m10;
    if !determinant.is_finite() || determinant == 0.0 {
        return Err(GeneralCellTransportErrorV1::GeometryUnavailable);
    }
    let dx = flat.x - tx;
    let dy = flat.y - ty;
    Point3::new(
        (m11 * dx - m01 * dy) / determinant,
        (-m10 * dx + m00 * dy) / determinant,
        0.0,
    )
    .map_err(|_| GeneralCellTransportErrorV1::GeometryUnavailable)
}
