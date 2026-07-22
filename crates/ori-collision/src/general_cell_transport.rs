use ori_domain::Point2;
use ori_foldability::{FoldedFaceOrientation, LayerOrderSnapshot};
use ori_kinematics::{
    CanonicalCycleScheduleV1, CycleScheduleLimitsV1, DyadicIntervalClosureLimitsV1,
    DyadicMaterialHingeIntervalClosureCertificateV1, HalfAngleRationalEntryInputV1,
    MaterialHingeGraphAudit, MaterialHingeGraphGeometry, Point3, RationalCoefficientV1,
};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::PositiveThicknessContinuousCertificateV1;

pub const GENERAL_MULTI_FACE_CELL_TRANSPORT_MODEL_ID_V1: &str =
    "general_multi_face_positive_thickness_cell_transport_v1";

pub const REGULAR_QUAD_PETAL_RATIO_CANDIDATE_LIMIT_V1: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegularQuadPetalRatioCandidateV1 {
    pub hinges: [ori_domain::EdgeId; 3],
    pub stage_endpoints: [[(i64, u64); 3]; 3],
}

#[must_use]
pub fn regular_quad_petal_ratio_candidates_v1(
    mut hinges: [(ori_domain::EdgeId, bool); 3],
) -> [RegularQuadPetalRatioCandidateV1; REGULAR_QUAD_PETAL_RATIO_CANDIDATE_LIMIT_V1] {
    hinges.sort_unstable_by_key(|(edge, _)| edge.canonical_bytes());
    let edges = hinges.map(|(edge, _)| edge);
    let signs = hinges.map(|(_, mountain)| if mountain { 1_i64 } else { -1_i64 });
    [
        [(1_i64, 64_u64), (1, 32), (1, 16)],
        [(1, 48), (1, 24), (1, 12)],
        [(1, 32), (1, 16), (1, 8)],
    ]
    .map(|ratios| RegularQuadPetalRatioCandidateV1 {
        hinges: edges,
        stage_endpoints: [
            [
                (signs[0] * ratios[0].0, ratios[0].1),
                (signs[1] * ratios[0].0, ratios[0].1),
                (signs[2] * ratios[0].0, ratios[0].1),
            ],
            [
                (signs[0] * ratios[1].0, ratios[1].1),
                (signs[1] * ratios[1].0, ratios[1].1),
                (signs[2] * ratios[1].0, ratios[1].1),
            ],
            [
                (signs[0] * ratios[2].0, ratios[2].1),
                (signs[1] * ratios[2].0, ratios[2].1),
                (signs[2] * ratios[2].0, ratios[2].1),
            ],
        ],
    })
}

pub fn prepare_regular_quad_petal_schedules_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: ori_domain::FaceId,
    candidate: &RegularQuadPetalRatioCandidateV1,
    limits: CycleScheduleLimitsV1,
) -> Option<[CanonicalCycleScheduleV1; 3]> {
    let mut previous = [(0_i64, 1_u64); 3];
    let mut schedules = Vec::with_capacity(3);
    for targets in candidate.stage_endpoints {
        let mut entries = geometry
            .hinges()
            .iter()
            .map(|hinge| {
                let index = candidate
                    .hinges
                    .iter()
                    .position(|edge| *edge == hinge.edge());
                let source = index.map_or((0, 1), |index| previous[index]);
                let target = index.map_or((0, 1), |index| targets[index]);
                let denominator = source.1.checked_mul(target.1)?;
                let initial = source.0.checked_mul(i64::try_from(target.1).ok()?)?;
                let target_scaled = target.0.checked_mul(i64::try_from(source.1).ok()?)?;
                Some(HalfAngleRationalEntryInputV1 {
                    edge: hinge.edge(),
                    u_domain: [
                        RationalCoefficientV1 {
                            numerator: 0,
                            denominator: 1,
                        },
                        RationalCoefficientV1 {
                            numerator: 1,
                            denominator: 1,
                        },
                    ],
                    numerator_power_coefficients: vec![
                        RationalCoefficientV1 {
                            numerator: initial,
                            denominator: 1,
                        },
                        RationalCoefficientV1 {
                            numerator: target_scaled.checked_sub(initial)?,
                            denominator: 1,
                        },
                    ],
                    denominator_power_coefficients: vec![RationalCoefficientV1 {
                        numerator: i64::try_from(denominator).ok()?,
                        denominator: 1,
                    }],
                })
            })
            .collect::<Option<Vec<_>>>()?;
        entries.sort_unstable_by_key(|entry| entry.edge.canonical_bytes());
        schedules.push(
            CanonicalCycleScheduleV1::prepare_half_angle_rational(
                geometry, audit, fixed_face, entries, limits,
            )
            .ok()?,
        );
        previous = targets;
    }
    schedules.try_into().ok()
}

pub struct RegularQuadPetalChainedAuthorityV1 {
    candidate: RegularQuadPetalRatioCandidateV1,
    schedules: [CanonicalCycleScheduleV1; 3],
    closures: [DyadicMaterialHingeIntervalClosureCertificateV1; 3],
    positives: [PositiveThicknessContinuousCertificateV1; 3],
    transport: ChainedGeneralCellTransportAuthorityV1,
}

impl RegularQuadPetalChainedAuthorityV1 {
    #[must_use]
    pub const fn candidate(&self) -> &RegularQuadPetalRatioCandidateV1 {
        &self.candidate
    }

    #[must_use]
    pub fn proofs(&self) -> &[GeneralMultiFaceCellTransportProofV1] {
        self.transport.proofs()
    }

    pub fn into_parts(
        self,
    ) -> (
        RegularQuadPetalRatioCandidateV1,
        [CanonicalCycleScheduleV1; 3],
        [DyadicMaterialHingeIntervalClosureCertificateV1; 3],
        [PositiveThicknessContinuousCertificateV1; 3],
        ChainedGeneralCellTransportAuthorityV1,
    ) {
        (
            self.candidate,
            self.schedules,
            self.closures,
            self.positives,
            self.transport,
        )
    }
}

#[allow(clippy::too_many_arguments)]
pub fn issue_regular_quad_petal_chained_authority_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    source: &LayerOrderSnapshot,
    fixed_face: ori_domain::FaceId,
    hinges: [(ori_domain::EdgeId, bool); 3],
    paper_thickness_mm: f64,
    tolerance: f64,
    schedule_limits: CycleScheduleLimitsV1,
    closure_limits: DyadicIntervalClosureLimitsV1,
) -> Option<RegularQuadPetalChainedAuthorityV1> {
    for candidate in regular_quad_petal_ratio_candidates_v1(hinges) {
        let Some(schedules) = prepare_regular_quad_petal_schedules_v1(
            geometry,
            audit,
            fixed_face,
            &candidate,
            schedule_limits,
        ) else {
            continue;
        };
        let closures = schedules
            .iter()
            .map(|schedule| {
                geometry
                    .prove_dyadic_schedule_closure_v1(
                        audit,
                        fixed_face,
                        schedule,
                        tolerance,
                        closure_limits,
                    )
                    .ok()
            })
            .collect::<Option<Vec<_>>>();
        let Some(closures) = closures else { continue };
        let positives = schedules
            .iter()
            .zip(&closures)
            .map(|(schedule, closure)| {
                crate::certify_canonical_positive_thickness_cycle_schedule_path_v1(
                    geometry,
                    audit,
                    fixed_face,
                    schedule,
                    closure,
                    paper_thickness_mm,
                    1,
                )
            })
            .collect::<Option<Vec<_>>>();
        let Some(positives) = positives else { continue };
        let inputs = schedules
            .iter()
            .zip(&closures)
            .zip(&positives)
            .map(|((schedule, closure), positive)| {
                let transitions = closure.leaves().len().checked_add(1)?;
                let layer_records = source.overlap_cells.iter().try_fold(0usize, |sum, cell| {
                    sum.checked_add(cell.bottom_to_top_faces.len())
                })?;
                let boundary_samples = source
                    .overlap_cells
                    .iter()
                    .try_fold(0usize, |sum, cell| {
                        sum.checked_add(
                            cell.exact_boundary
                                .len()
                                .checked_mul(cell.bottom_to_top_faces.len())?,
                        )
                    })?
                    .checked_mul(transitions)?;
                Some(GeneralCellTransportInputV1 {
                    geometry,
                    audit,
                    source,
                    schedule,
                    closure,
                    positive_continuous: positive,
                    paper_thickness_mm,
                    tolerance,
                    limits: GeneralCellTransportLimitsV1 {
                        max_transitions: transitions,
                        max_cells: source.overlap_cells.len(),
                        max_layer_records: layer_records,
                        max_boundary_samples: boundary_samples,
                    },
                })
            })
            .collect::<Option<Vec<_>>>();
        let Some(inputs) = inputs else { continue };
        if let Ok(transport) = ChainedGeneralCellTransportAuthorityV1::issue(inputs) {
            return Some(RegularQuadPetalChainedAuthorityV1 {
                candidate,
                schedules,
                closures: closures.try_into().ok()?,
                positives: positives.try_into().ok()?,
                transport,
            });
        }
    }
    None
}

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

pub fn preflight_general_cell_transport_work_v1(
    transitions: usize,
    cells: usize,
    layer_records: usize,
    boundary_samples: usize,
    limits: GeneralCellTransportLimitsV1,
) -> Result<(), GeneralCellTransportErrorV1> {
    if transitions == 0
        || transitions > limits.max_transitions
        || cells > limits.max_cells
        || layer_records > limits.max_layer_records
        || boundary_samples > limits.max_boundary_samples
    {
        return Err(GeneralCellTransportErrorV1::ResourceLimit);
    }
    Ok(())
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

/// Issuer-private bundle proving a continuous sequence without inventing a
/// non-flat `LayerOrderSnapshot`.
pub struct ChainedGeneralCellTransportAuthorityV1 {
    proofs: Vec<GeneralMultiFaceCellTransportProofV1>,
}

impl ChainedGeneralCellTransportAuthorityV1 {
    pub fn issue(
        inputs: Vec<GeneralCellTransportInputV1<'_>>,
    ) -> Result<Self, GeneralCellTransportErrorV1> {
        if inputs.is_empty()
            || inputs.windows(2).any(|pair| {
                !pair[0].geometry.same_instance(pair[1].geometry)
                    || !std::ptr::eq(pair[0].source, pair[1].source)
                    || pair[0].paper_thickness_mm.to_bits() != pair[1].paper_thickness_mm.to_bits()
                    || pair[0].schedule.evaluate(1.0) != pair[1].schedule.evaluate(0.0)
            })
        {
            return Err(GeneralCellTransportErrorV1::BindingMismatch);
        }
        let proofs = inputs
            .into_iter()
            .map(certify_general_multi_face_cell_transport_v1)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { proofs })
    }

    pub fn proofs(&self) -> &[GeneralMultiFaceCellTransportProofV1] {
        &self.proofs
    }
}

#[cfg(test)]
pub(crate) struct RegularQuadPetalPrivateRecordV1 {
    token: u128,
    revision: u64,
    target_binding: [u8; 32],
    path_binding: [u8; 32],
    authority: ChainedGeneralCellTransportAuthorityV1,
}

#[cfg(test)]
impl RegularQuadPetalPrivateRecordV1 {
    pub(crate) fn issue(
        token: u128,
        revision: u64,
        target_binding: [u8; 32],
        path_binding: [u8; 32],
        inputs: Vec<GeneralCellTransportInputV1<'_>>,
    ) -> Result<Self, GeneralCellTransportErrorV1> {
        let authority = ChainedGeneralCellTransportAuthorityV1::issue(inputs)?;
        if authority.proofs().len() != 3 {
            return Err(GeneralCellTransportErrorV1::BindingMismatch);
        }
        Ok(Self {
            token,
            revision,
            target_binding,
            path_binding,
            authority,
        })
    }

    pub(crate) fn revalidates_for_apply_v1(
        &self,
        token: u128,
        revision: u64,
        target_binding: [u8; 32],
        path_binding: [u8; 32],
    ) -> bool {
        self.token == token
            && self.revision == revision
            && self.target_binding == target_binding
            && self.path_binding == path_binding
            && self.authority.proofs().len() == 3
    }
}

#[derive(Debug, Clone)]
pub struct GeneralMultiFaceCellTransportProofV1 {
    issuer: MaterialHingeGraphGeometry,
    source_instance: usize,
    source: LayerOrderSnapshot,
    schedule_hash: [u8; 32],
    closure_hash: [u8; 32],
    thickness_bits: u64,
    pair_order_count: usize,
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
    pub fn transition_hashes(&self) -> &[[u8; 32]] {
        &self.checkpoint_hashes
    }

    #[must_use]
    pub const fn pair_order_count(&self) -> usize {
        self.pair_order_count
    }

    #[must_use]
    pub fn paper_thickness_mm(&self) -> f64 {
        f64::from_bits(self.thickness_bits)
    }

    #[must_use]
    pub fn matches_source_content_v1(&self, source: &LayerOrderSnapshot) -> bool {
        self.source == *source
    }

    #[must_use]
    pub fn target_order_hash(&self) -> [u8; 32] {
        self.checkpoint_hashes.last().copied().unwrap_or([0; 32])
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
    let opposite_radial_bifold =
        crate::continuous_path::scheduled_opposite_radial_bifold_premises_v1(
            input.geometry,
            input.audit,
            input.closure.fixed_face(),
            input.schedule,
            input.closure,
        );
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
    preflight_general_cell_transport_work_v1(
        transition_count,
        input.source.overlap_cells.len(),
        layer_records,
        boundary_samples,
        input.limits,
    )?;
    let folded = input
        .source
        .folded_faces
        .iter()
        .map(|face| (face.face.face_id, face))
        .collect::<std::collections::HashMap<_, _>>();
    if folded.len() != input.source.material_faces.len() {
        return Err(GeneralCellTransportErrorV1::BindingMismatch);
    }
    let mut cells = input.source.overlap_cells.iter().collect::<Vec<_>>();
    cells.sort_unstable_by_key(|cell| cell.cell_key.0);
    let inverse_transforms = folded
        .iter()
        .map(|(face, folded)| prepare_inverse_flat_transform(folded).map(|value| (*face, value)))
        .collect::<Result<std::collections::HashMap<_, _>, _>>()?;
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
        for cell in &cells {
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
                    let material = inverse_flat_point(
                        inverse_transforms
                            .get(&face)
                            .ok_or(GeneralCellTransportErrorV1::BindingMismatch)?,
                        flat,
                    )?;
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
                    if separation + input.tolerance < input.paper_thickness_mm
                        && !opposite_radial_bifold
                    {
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
        pair_order_count: input.source.face_pair_orders.len(),
        checkpoint_hashes,
    })
}

#[derive(Clone, Copy)]
struct InverseFlatTransform {
    m00: f64,
    m01: f64,
    m10: f64,
    m11: f64,
    tx: f64,
    ty: f64,
    determinant: f64,
}

fn prepare_inverse_flat_transform(
    folded: &ori_foldability::FoldedFaceSnapshot,
) -> Result<InverseFlatTransform, GeneralCellTransportErrorV1> {
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
    Ok(InverseFlatTransform {
        m00,
        m01,
        m10,
        m11,
        tx,
        ty,
        determinant,
    })
}

fn inverse_flat_point(
    transform: &InverseFlatTransform,
    flat: Point2,
) -> Result<Point3, GeneralCellTransportErrorV1> {
    let dx = flat.x - transform.tx;
    let dy = flat.y - transform.ty;
    Point3::new(
        (transform.m11 * dx - transform.m01 * dy) / transform.determinant,
        (-transform.m10 * dx + transform.m00 * dy) / transform.determinant,
        0.0,
    )
    .map_err(|_| GeneralCellTransportErrorV1::GeometryUnavailable)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regular_quad_petal_ratios_are_exact_canonical_and_stage_contiguous() {
        let mut input = [
            (ori_domain::EdgeId::new(), true),
            (ori_domain::EdgeId::new(), false),
            (ori_domain::EdgeId::new(), true),
        ];
        let first = regular_quad_petal_ratio_candidates_v1(input);
        input.reverse();
        assert_eq!(first, regular_quad_petal_ratio_candidates_v1(input));
        assert_eq!(first.len(), REGULAR_QUAD_PETAL_RATIO_CANDIDATE_LIMIT_V1);
        for candidate in first {
            assert!(
                candidate
                    .hinges
                    .windows(2)
                    .all(|pair| { pair[0].canonical_bytes() < pair[1].canonical_bytes() })
            );
            for stage in candidate.stage_endpoints {
                let normalized = stage.map(|(p, q)| (p.unsigned_abs(), q));
                assert_eq!(normalized[0], normalized[1]);
                assert_eq!(normalized[1], normalized[2]);
            }
            assert!(
                candidate
                    .stage_endpoints
                    .iter()
                    .flatten()
                    .all(|(p, q)| { p.unsigned_abs() <= 64 && *q <= 64 && *q != 0 })
            );
        }
    }

    #[test]
    fn rank_one_twenty_eight_work_is_admitted_only_at_exact_limits() {
        let limits = GeneralCellTransportLimitsV1 {
            max_transitions: 2,
            max_cells: 128,
            max_layer_records: 512,
            max_boundary_samples: 4_096,
        };
        assert_eq!(
            preflight_general_cell_transport_work_v1(2, 128, 512, 4_096, limits),
            Ok(())
        );
        assert_eq!(
            preflight_general_cell_transport_work_v1(
                2,
                128,
                512,
                4_096,
                GeneralCellTransportLimitsV1 {
                    max_boundary_samples: 4_095,
                    ..limits
                },
            ),
            Err(GeneralCellTransportErrorV1::ResourceLimit)
        );
    }
}
