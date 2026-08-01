use ori_domain::Point2;
use ori_foldability::{FoldedFaceOrientation, LayerOrderSnapshot};
use ori_kinematics::{
    CanonicalCycleScheduleV1, CycleScheduleLimitsV1, DyadicIntervalClosureLimitsV1,
    DyadicMaterialHingeIntervalClosureCertificateV1, HalfAngleRationalEntryInputV1,
    MaterialHingeGraphAudit, MaterialHingeGraphGeometry, MaterialHingeGraphInstanceV1, Point3,
    RationalCoefficientV1,
};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{CooperativeOperationStopV1, PositiveThicknessContinuousCertificateV1};

pub const GENERAL_MULTI_FACE_CELL_TRANSPORT_MODEL_ID_V1: &str =
    "general_multi_face_positive_thickness_cell_transport_v1";

/// The currently proved chain classes contain at most three contiguous
/// transports. Longer arbitrary chains have no public completeness theorem and
/// therefore fail closed before proof allocation or certification begins.
pub const GENERAL_CELL_TRANSPORT_CHAIN_LIMIT_V1: usize = 3;
pub const REGULAR_QUAD_PETAL_RATIO_CANDIDATE_LIMIT_V1: usize = 3;
pub const DEGREE_FOUR_PETAL_COMPLETION_LIMIT_V1: usize = 4;

type RegularQuadPetalCandidateIteratorV1 = std::iter::Chain<
    std::array::IntoIter<
        RegularQuadPetalRatioCandidateV1,
        REGULAR_QUAD_PETAL_RATIO_CANDIDATE_LIMIT_V1,
    >,
    std::vec::IntoIter<RegularQuadPetalRatioCandidateV1>,
>;

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

pub(crate) fn degree_four_petal_completion_candidates_v1(
    geometry: &MaterialHingeGraphGeometry,
    mut selected: [(ori_domain::EdgeId, bool); 3],
) -> Vec<RegularQuadPetalRatioCandidateV1> {
    if geometry.hinges().len() != 4
        || geometry
            .hinges()
            .iter()
            .filter(|hinge| hinge.assignment() == ori_topology::FoldAssignment::Mountain)
            .count()
            .abs_diff(2)
            != 1
    {
        return Vec::new();
    }
    let first = &geometry.hinges()[0];
    let pivot = [first.start(), first.end()].into_iter().find(|point| {
        geometry
            .hinges()
            .iter()
            .all(|hinge| hinge.start() == *point || hinge.end() == *point)
    });
    if pivot.is_none()
        || selected
            .iter()
            .any(|(edge, _)| !geometry.hinges().iter().any(|hinge| hinge.edge() == *edge))
    {
        return Vec::new();
    }
    selected.sort_unstable_by_key(|(edge, _)| edge.canonical_bytes());
    let edges = selected.map(|(edge, _)| edge);
    let [Some(first_rank), Some(second_rank), Some(third_rank)] = edges.map(|edge| {
        geometry
            .hinges()
            .iter()
            .position(|hinge| hinge.edge() == edge)
    }) else {
        return Vec::new();
    };
    let ranks = [first_rank, second_rank, third_rank];
    let mut candidates = Vec::new();
    if candidates
        .try_reserve_exact(DEGREE_FOUR_PETAL_COMPLETION_LIMIT_V1)
        .is_err()
    {
        return Vec::new();
    }
    for phase in 0..2 {
        for sign in [1_i64, -1] {
            let stages = [64_u64, 32, 16].map(|base| {
                std::array::from_fn(|index| {
                    if ranks[index] % 2 == phase {
                        (sign, base)
                    } else {
                        (0, 1)
                    }
                })
            });
            candidates.push(RegularQuadPetalRatioCandidateV1 {
                hinges: edges,
                stage_endpoints: stages,
            });
        }
    }
    candidates
}

pub fn prepare_regular_quad_petal_schedules_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: ori_domain::FaceId,
    candidate: &RegularQuadPetalRatioCandidateV1,
    limits: CycleScheduleLimitsV1,
) -> Option<[CanonicalCycleScheduleV1; 3]> {
    if geometry.hinges().is_empty() || geometry.hinges().len() > limits.max_hinges {
        return None;
    }
    let mut previous = [(0_i64, 1_u64); 3];
    let mut schedules = Vec::new();
    schedules.try_reserve_exact(3).ok()?;
    let first_stage_denominators = candidate.stage_endpoints[0].map(|(_, denominator)| denominator);
    let [first, second, third] = first_stage_denominators;
    let has_degree_four_completion = geometry.hinges().len() == 4
        && ((first == second && second != third)
            || (first == third && first != second)
            || (second == third && first != second));
    for targets in candidate.stage_endpoints {
        let mut entries = Vec::new();
        entries.try_reserve_exact(geometry.hinges().len()).ok()?;
        for hinge in geometry.hinges() {
            let index = candidate
                .hinges
                .iter()
                .position(|edge| *edge == hinge.edge());
            let completion = |endpoints: &[(i64, u64); 3]| {
                if !has_degree_four_completion {
                    return None;
                }
                let rank = geometry
                    .hinges()
                    .iter()
                    .position(|other| other.edge() == hinge.edge())?;
                let (selected_index, &(numerator, denominator)) = endpoints
                    .iter()
                    .enumerate()
                    .find(|(_, (numerator, _))| *numerator != 0)?;
                let phase = geometry
                    .hinges()
                    .iter()
                    .position(|other| other.edge() == candidate.hinges[selected_index])?
                    % 2;
                Some(if rank % 2 == phase {
                    (numerator.signum(), denominator)
                } else {
                    (0, 1)
                })
            };
            let source = index.map_or_else(
                || completion(&previous).unwrap_or((0, 1)),
                |index| previous[index],
            );
            let target = index.map_or_else(
                || completion(&targets).unwrap_or((0, 1)),
                |index| targets[index],
            );
            let denominator = source.1.checked_mul(target.1)?;
            let initial = source.0.checked_mul(i64::try_from(target.1).ok()?)?;
            let target_scaled = target.0.checked_mul(i64::try_from(source.1).ok()?)?;
            let mut numerator_power_coefficients = Vec::new();
            numerator_power_coefficients.try_reserve_exact(2).ok()?;
            numerator_power_coefficients.push(RationalCoefficientV1 {
                numerator: initial,
                denominator: 1,
            });
            numerator_power_coefficients.push(RationalCoefficientV1 {
                numerator: target_scaled.checked_sub(initial)?,
                denominator: 1,
            });
            let mut denominator_power_coefficients = Vec::new();
            denominator_power_coefficients.try_reserve_exact(1).ok()?;
            denominator_power_coefficients.push(RationalCoefficientV1 {
                numerator: i64::try_from(denominator).ok()?,
                denominator: 1,
            });
            entries.push(HalfAngleRationalEntryInputV1 {
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
                numerator_power_coefficients,
                denominator_power_coefficients,
            });
        }
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

fn checked_regular_quad_petal_issuance_prefix_bytes_v1(
    candidate_storage_bytes: usize,
    schedules: &[CanonicalCycleScheduleV1; 3],
    closures: &[DyadicMaterialHingeIntervalClosureCertificateV1],
    closure_capacity: usize,
    positives: &[PositiveThicknessContinuousCertificateV1],
    positive_capacity: usize,
) -> Option<usize> {
    let mut total = std::mem::size_of::<RegularQuadPetalRatioCandidateV1>()
        .checked_add(candidate_storage_bytes)?
        .checked_add(std::mem::size_of::<[CanonicalCycleScheduleV1; 3]>())?
        .checked_add(std::mem::size_of::<Vec<GeneralCellTransportInputV1<'static>>>())?
        .checked_add(std::mem::size_of::<ChainedGeneralCellTransportAuthorityV1>())?;
    for schedule in schedules {
        total = total.checked_add(
            schedule
                .checked_deep_retained_bytes_v1()?
                .checked_sub(std::mem::size_of::<CanonicalCycleScheduleV1>())?,
        )?;
    }
    total = total
        .checked_add(std::mem::size_of::<
            Vec<DyadicMaterialHingeIntervalClosureCertificateV1>,
        >())?
        .checked_add(
            std::mem::size_of::<DyadicMaterialHingeIntervalClosureCertificateV1>()
                .checked_mul(closure_capacity)?,
        )?;
    for closure in closures {
        total = total.checked_add(closure.checked_deep_retained_bytes_v1()?.checked_sub(
            std::mem::size_of::<DyadicMaterialHingeIntervalClosureCertificateV1>(),
        )?)?;
    }
    total = total
        .checked_add(std::mem::size_of::<
            Vec<PositiveThicknessContinuousCertificateV1>,
        >())?
        .checked_add(
            std::mem::size_of::<PositiveThicknessContinuousCertificateV1>()
                .checked_mul(positive_capacity)?,
        )?;
    for positive in positives {
        total = total.checked_add(
            positive
                .checked_deep_retained_bytes_v1()?
                .checked_sub(std::mem::size_of::<PositiveThicknessContinuousCertificateV1>())?,
        )?;
    }
    Some(total)
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

    #[must_use]
    pub(crate) fn checked_deep_retained_bytes_v1(&self) -> Option<usize> {
        let mut total = std::mem::size_of::<Self>();
        for schedule in &self.schedules {
            total = total.checked_add(
                schedule
                    .checked_deep_retained_bytes_v1()?
                    .checked_sub(std::mem::size_of::<CanonicalCycleScheduleV1>())?,
            )?;
        }
        for closure in &self.closures {
            total = total.checked_add(closure.checked_deep_retained_bytes_v1()?.checked_sub(
                std::mem::size_of::<DyadicMaterialHingeIntervalClosureCertificateV1>(),
            )?)?;
        }
        for positive in &self.positives {
            total =
                total.checked_add(positive.checked_deep_retained_bytes_v1()?.checked_sub(
                    std::mem::size_of::<PositiveThicknessContinuousCertificateV1>(),
                )?)?;
        }
        total = total.checked_add(
            self.transport
                .checked_deep_retained_bytes_v1()?
                .checked_sub(std::mem::size_of::<ChainedGeneralCellTransportAuthorityV1>())?,
        )?;
        Some(total)
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
    let completion_candidates = degree_four_petal_completion_candidates_v1(geometry, hinges);
    let candidate_storage_bytes = std::mem::size_of::<RegularQuadPetalCandidateIteratorV1>()
        .checked_add(
            std::mem::size_of::<RegularQuadPetalRatioCandidateV1>()
                .checked_mul(completion_candidates.capacity())?,
        )?;
    let candidates = regular_quad_petal_ratio_candidates_v1(hinges)
        .into_iter()
        .chain(completion_candidates);
    'candidate: for candidate in candidates {
        let Some(schedules) = prepare_regular_quad_petal_schedules_v1(
            geometry,
            audit,
            fixed_face,
            &candidate,
            schedule_limits,
        ) else {
            continue;
        };
        let mut closures = Vec::new();
        closures.try_reserve_exact(schedules.len()).ok()?;
        for schedule in &schedules {
            let Ok(closure) = geometry.prove_dyadic_schedule_closure_v1(
                audit,
                fixed_face,
                schedule,
                tolerance,
                closure_limits,
            ) else {
                continue 'candidate;
            };
            closures.push(closure);
        }
        let mut positives = Vec::new();
        positives.try_reserve_exact(schedules.len()).ok()?;
        for (schedule, closure) in schedules.iter().zip(&closures) {
            let Some(positive) = crate::certify_canonical_positive_thickness_cycle_schedule_path_v1(
                geometry,
                audit,
                fixed_face,
                schedule,
                closure,
                paper_thickness_mm,
                1,
            ) else {
                continue 'candidate;
            };
            positives.push(positive);
        }
        let mut inputs = Vec::new();
        inputs.try_reserve_exact(schedules.len()).ok()?;
        for ((schedule, closure), positive) in schedules.iter().zip(&closures).zip(&positives) {
            let Some(transitions) = closure.leaves().len().checked_add(1) else {
                continue 'candidate;
            };
            let Some(layer_records) = source.overlap_cells.iter().try_fold(0usize, |sum, cell| {
                sum.checked_add(cell.bottom_to_top_faces.len())
            }) else {
                continue 'candidate;
            };
            let Some(boundary_samples) = source
                .overlap_cells
                .iter()
                .try_fold(0usize, |sum, cell| {
                    sum.checked_add(
                        cell.exact_boundary
                            .len()
                            .checked_mul(cell.bottom_to_top_faces.len())?,
                    )
                })
                .and_then(|samples| samples.checked_mul(transitions))
            else {
                continue 'candidate;
            };
            inputs.push(GeneralCellTransportInputV1 {
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
            });
        }
        let Some(wrapper_prefix_bytes) = checked_regular_quad_petal_issuance_prefix_bytes_v1(
            candidate_storage_bytes,
            &schedules,
            &closures,
            closures.capacity(),
            &positives,
            positives.capacity(),
        ) else {
            continue;
        };
        let Some(transport_peak_limit) =
            ori_foldability::DEFAULT_MAX_CERTIFICATE_BYTES.checked_sub(wrapper_prefix_bytes)
        else {
            continue;
        };
        let Ok(transport) = ChainedGeneralCellTransportAuthorityV1::issue_with_peak_limit(
            inputs,
            transport_peak_limit,
        ) else {
            continue;
        };
        let closures = closures.try_into().ok()?;
        let positives = positives.try_into().ok()?;
        let authority = RegularQuadPetalChainedAuthorityV1 {
            candidate,
            schedules,
            closures,
            positives,
            transport,
        };
        let retained_bytes = authority.checked_deep_retained_bytes_v1()?;
        if retained_bytes > ori_foldability::DEFAULT_MAX_CERTIFICATE_BYTES {
            continue;
        }
        return Some(authority);
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

pub(crate) fn preflight_general_cell_transport_source_retention_v1(
    retained_bytes: usize,
    maximum_retained_bytes: usize,
) -> Result<(), GeneralCellTransportErrorV1> {
    if retained_bytes > maximum_retained_bytes {
        return Err(GeneralCellTransportErrorV1::ResourceLimit);
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GeneralCellTransportMemoryWorkV1 {
    pub proof_retained_bytes: usize,
    pub peak_temporary_bytes: usize,
}

pub fn checked_general_cell_transport_memory_work_v1(
    transitions: usize,
    folded_faces: usize,
    cells: usize,
    maximum_boundary_points: usize,
) -> Option<GeneralCellTransportMemoryWorkV1> {
    let proof_shell_bytes = std::mem::size_of::<GeneralMultiFaceCellTransportProofV1>()
        .checked_sub(std::mem::size_of::<LayerOrderSnapshot>())?;
    let checkpoint_bytes = std::mem::size_of::<[u8; 32]>().checked_mul(transitions)?;
    let proof_retained_bytes = proof_shell_bytes.checked_add(checkpoint_bytes)?;
    let prepared_face_bytes =
        std::mem::size_of::<PreparedFoldedFaceV1<'static>>().checked_mul(folded_faces)?;
    let sorted_cell_bytes =
        std::mem::size_of::<&ori_foldability::OverlapCellSnapshot>().checked_mul(cells)?;
    let streaming_boundary_bytes =
        std::mem::size_of::<[f64; 3]>().checked_mul(maximum_boundary_points)?;
    let peak_temporary_bytes = prepared_face_bytes
        .checked_add(sorted_cell_bytes)?
        .checked_add(streaming_boundary_bytes)?;
    Some(GeneralCellTransportMemoryWorkV1 {
        proof_retained_bytes,
        peak_temporary_bytes,
    })
}

pub fn checked_general_cell_transport_peak_bytes_v1(
    source_retained_bytes: usize,
    memory: GeneralCellTransportMemoryWorkV1,
) -> Option<usize> {
    source_retained_bytes
        .checked_add(memory.proof_retained_bytes)?
        .checked_add(source_retained_bytes.max(memory.peak_temporary_bytes))
}

/// Computes a conservative operation peak for a contiguous proof chain.
///
/// Both input and proof-vector backing allocations are charged at their actual
/// capacities. Each completed proof remains charged while the next proof's
/// phase-exact source clone or temporary workspace is considered. Those two
/// allocations are mutually exclusive because temporary buffers are dropped
/// before cloning. The proof-vector element shells are deliberately charged
/// again through each proof's retained work so allocator over-capacity can
/// never be hidden by inline storage.
pub fn checked_chained_general_cell_transport_peak_bytes_v1(
    input_capacity: usize,
    proof_capacity: usize,
    source_retained_bytes: usize,
    memories: &[GeneralCellTransportMemoryWorkV1],
) -> Option<usize> {
    if memories.is_empty()
        || memories.len() > GENERAL_CELL_TRANSPORT_CHAIN_LIMIT_V1
        || input_capacity < memories.len()
        || proof_capacity < memories.len()
    {
        return None;
    }
    let input_buffer_bytes =
        std::mem::size_of::<GeneralCellTransportInputV1<'static>>().checked_mul(input_capacity)?;
    let proof_buffer_bytes =
        std::mem::size_of::<GeneralMultiFaceCellTransportProofV1>().checked_mul(proof_capacity)?;
    let mut retained_prefix = input_buffer_bytes.checked_add(proof_buffer_bytes)?;
    let mut peak = 0usize;
    for memory in memories {
        let current_peak =
            checked_general_cell_transport_peak_bytes_v1(source_retained_bytes, *memory)?;
        peak = peak.max(retained_prefix.checked_add(current_peak)?);
        let completed_proof_bytes =
            source_retained_bytes.checked_add(memory.proof_retained_bytes)?;
        retained_prefix = retained_prefix.checked_add(completed_proof_bytes)?;
    }
    peak = peak.max(retained_prefix.checked_add(source_retained_bytes)?);
    Some(peak)
}

pub fn preflight_general_cell_transport_peak_bytes_v1(
    peak_bytes: usize,
    maximum_peak_bytes: usize,
) -> Result<(), GeneralCellTransportErrorV1> {
    if peak_bytes > maximum_peak_bytes {
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

fn preflight_general_cell_transport_chain_count_v1(
    count: usize,
) -> Result<(), GeneralCellTransportErrorV1> {
    if count == 0 {
        return Err(GeneralCellTransportErrorV1::BindingMismatch);
    }
    if count > GENERAL_CELL_TRANSPORT_CHAIN_LIMIT_V1 {
        return Err(GeneralCellTransportErrorV1::ResourceLimit);
    }
    Ok(())
}

fn checked_general_cell_transport_input_memory_work_v1(
    input: &GeneralCellTransportInputV1<'_>,
) -> Option<GeneralCellTransportMemoryWorkV1> {
    let transitions = input.closure.leaves().len().checked_add(1)?;
    let maximum_boundary_points = input
        .source
        .overlap_cells
        .iter()
        .map(|cell| cell.exact_boundary.len())
        .max()
        .unwrap_or(0);
    let mut memory = checked_general_cell_transport_memory_work_v1(
        transitions,
        input.source.folded_faces.len(),
        input.source.overlap_cells.len(),
        maximum_boundary_points,
    )?;
    memory.peak_temporary_bytes = memory.peak_temporary_bytes.checked_add(
        checked_general_cell_transport_runtime_peak_bytes_v1(
            input.geometry,
            input.audit,
            input.source,
        )?,
    )?;
    Some(memory)
}

/// Conservative per-checkpoint heap peak outside transport-owned buffers.
///
/// This includes the fallible schedule angle vector and the closed graph-pose
/// solve with allocator slack. Exact coordinate projection is heapless.
#[must_use]
pub fn checked_general_cell_transport_runtime_peak_bytes_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    source: &LayerOrderSnapshot,
) -> Option<usize> {
    let angle_bytes = std::mem::size_of::<ori_kinematics::CanonicalHingeAngles>().checked_add(
        std::mem::size_of::<ori_kinematics::HingeAngle>().checked_mul(geometry.hinges().len())?,
    )?;
    let _ = source;
    angle_bytes.checked_add(geometry.checked_solve_closed_peak_bytes_v1(audit)?)
}

fn try_general_cell_transport_proof_buffer_v1(
    capacity: usize,
) -> Result<Vec<GeneralMultiFaceCellTransportProofV1>, GeneralCellTransportErrorV1> {
    let mut proofs = Vec::new();
    proofs
        .try_reserve_exact(capacity)
        .map_err(|_| GeneralCellTransportErrorV1::ResourceLimit)?;
    Ok(proofs)
}

fn preflight_general_cell_transport_schedule_continuity_v1(
    previous_target: Result<ori_kinematics::CanonicalHingeAngles, ori_kinematics::KinematicsError>,
    next_source: Result<ori_kinematics::CanonicalHingeAngles, ori_kinematics::KinematicsError>,
) -> Result<(), GeneralCellTransportErrorV1> {
    let map_error = |error| {
        if error == ori_kinematics::KinematicsError::ResourceLimitExceeded {
            GeneralCellTransportErrorV1::ResourceLimit
        } else {
            GeneralCellTransportErrorV1::BindingMismatch
        }
    };
    let previous_target = previous_target.map_err(map_error)?;
    let next_source = next_source.map_err(map_error)?;
    if previous_target != next_source {
        return Err(GeneralCellTransportErrorV1::BindingMismatch);
    }
    Ok(())
}

impl ChainedGeneralCellTransportAuthorityV1 {
    pub fn issue(
        inputs: Vec<GeneralCellTransportInputV1<'_>>,
    ) -> Result<Self, GeneralCellTransportErrorV1> {
        Self::issue_with_peak_limit(inputs, ori_foldability::DEFAULT_MAX_CERTIFICATE_BYTES)
    }

    fn issue_with_peak_limit(
        inputs: Vec<GeneralCellTransportInputV1<'_>>,
        maximum_peak_bytes: usize,
    ) -> Result<Self, GeneralCellTransportErrorV1> {
        preflight_general_cell_transport_chain_count_v1(inputs.len())?;
        for pair in inputs.windows(2) {
            if !pair[0].geometry.same_instance(pair[1].geometry)
                || !std::ptr::eq(pair[0].source, pair[1].source)
                || pair[0].paper_thickness_mm.to_bits() != pair[1].paper_thickness_mm.to_bits()
            {
                return Err(GeneralCellTransportErrorV1::BindingMismatch);
            }
            preflight_general_cell_transport_schedule_continuity_v1(
                pair[0].schedule.try_evaluate_v1(1.0),
                pair[1].schedule.try_evaluate_v1(0.0),
            )?;
        }
        let source_retained_bytes = inputs[0]
            .source
            .checked_deep_retained_bytes_v1()
            .ok_or(GeneralCellTransportErrorV1::ResourceLimit)?;
        let mut memories = [GeneralCellTransportMemoryWorkV1 {
            proof_retained_bytes: 0,
            peak_temporary_bytes: 0,
        }; GENERAL_CELL_TRANSPORT_CHAIN_LIMIT_V1];
        for (memory, input) in memories.iter_mut().zip(&inputs) {
            *memory = checked_general_cell_transport_input_memory_work_v1(input)
                .ok_or(GeneralCellTransportErrorV1::ResourceLimit)?;
        }
        let memories = &memories[..inputs.len()];
        let projected_peak = checked_chained_general_cell_transport_peak_bytes_v1(
            inputs.capacity(),
            inputs.len(),
            source_retained_bytes,
            memories,
        )
        .ok_or(GeneralCellTransportErrorV1::ResourceLimit)?;
        preflight_general_cell_transport_peak_bytes_v1(projected_peak, maximum_peak_bytes)?;

        let mut proofs = try_general_cell_transport_proof_buffer_v1(inputs.len())?;
        let actual_peak = checked_chained_general_cell_transport_peak_bytes_v1(
            inputs.capacity(),
            proofs.capacity(),
            source_retained_bytes,
            memories,
        )
        .ok_or(GeneralCellTransportErrorV1::ResourceLimit)?;
        preflight_general_cell_transport_peak_bytes_v1(actual_peak, maximum_peak_bytes)?;
        let input_buffer_bytes = std::mem::size_of::<GeneralCellTransportInputV1<'static>>()
            .checked_mul(inputs.capacity())
            .ok_or(GeneralCellTransportErrorV1::ResourceLimit)?;
        let proof_buffer_bytes = std::mem::size_of::<GeneralMultiFaceCellTransportProofV1>()
            .checked_mul(proofs.capacity())
            .ok_or(GeneralCellTransportErrorV1::ResourceLimit)?;
        let mut retained_prefix = input_buffer_bytes
            .checked_add(proof_buffer_bytes)
            .ok_or(GeneralCellTransportErrorV1::ResourceLimit)?;
        for input in inputs {
            let available_peak_bytes = maximum_peak_bytes
                .checked_sub(retained_prefix)
                .ok_or(GeneralCellTransportErrorV1::ResourceLimit)?;
            let proof = certify_general_multi_face_cell_transport_with_peak_limit_v1(
                input,
                available_peak_bytes,
            )?;
            let proof_retained_bytes = proof
                .checked_deep_retained_bytes_v1()
                .ok_or(GeneralCellTransportErrorV1::ResourceLimit)?;
            retained_prefix = retained_prefix
                .checked_add(proof_retained_bytes)
                .ok_or(GeneralCellTransportErrorV1::ResourceLimit)?;
            let retained_with_source = retained_prefix
                .checked_add(source_retained_bytes)
                .ok_or(GeneralCellTransportErrorV1::ResourceLimit)?;
            preflight_general_cell_transport_peak_bytes_v1(
                retained_with_source,
                maximum_peak_bytes,
            )?;
            proofs.push(proof);
        }
        Ok(Self { proofs })
    }

    pub fn proofs(&self) -> &[GeneralMultiFaceCellTransportProofV1] {
        &self.proofs
    }

    #[must_use]
    pub(crate) fn checked_deep_retained_bytes_v1(&self) -> Option<usize> {
        let mut total = std::mem::size_of::<Self>().checked_add(
            std::mem::size_of::<GeneralMultiFaceCellTransportProofV1>()
                .checked_mul(self.proofs.capacity())?,
        )?;
        for proof in &self.proofs {
            total = total.checked_add(
                proof
                    .checked_deep_retained_bytes_v1()?
                    .checked_sub(std::mem::size_of::<GeneralMultiFaceCellTransportProofV1>())?,
            )?;
        }
        Some(total)
    }

    #[must_use]
    pub fn into_proofs(self) -> Vec<GeneralMultiFaceCellTransportProofV1> {
        self.proofs
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

#[derive(Debug)]
pub struct GeneralMultiFaceCellTransportProofV1 {
    issuer: MaterialHingeGraphInstanceV1,
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
    pub fn checked_deep_retained_bytes_v1(&self) -> Option<usize> {
        let proof_shell_bytes =
            std::mem::size_of::<Self>().checked_sub(std::mem::size_of::<LayerOrderSnapshot>())?;
        let checkpoint_bytes =
            std::mem::size_of::<[u8; 32]>().checked_mul(self.checkpoint_hashes.capacity())?;
        self.source
            .checked_deep_retained_bytes_v1()?
            .checked_add(proof_shell_bytes)?
            .checked_add(checkpoint_bytes)
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
        self.is_for_with_checkpoint_v1(geometry, source, schedule, closure, thickness, || Ok(()))
            .unwrap_or(false)
    }

    pub(crate) fn is_for_with_checkpoint_v1(
        &self,
        geometry: &MaterialHingeGraphGeometry,
        source: &LayerOrderSnapshot,
        schedule: &CanonicalCycleScheduleV1,
        closure: &DyadicMaterialHingeIntervalClosureCertificateV1,
        thickness: f64,
        mut checkpoint: impl FnMut() -> Result<(), CooperativeOperationStopV1>,
    ) -> Result<bool, CooperativeOperationStopV1> {
        checkpoint()?;
        if !self.issuer.matches(geometry) {
            return Ok(false);
        }
        if self.source_instance != source as *const LayerOrderSnapshot as usize
            || self.schedule_hash != schedule.certificate_binding_fingerprint_v2()
            || self.closure_hash != closure.partition_binding_fingerprint_v2()
            || self.thickness_bits != thickness.to_bits()
        {
            return Ok(false);
        }
        layer_order_snapshot_equal_with_checkpoint_v1(&self.source, source, &mut checkpoint)
    }
}

fn layer_order_snapshot_equal_with_checkpoint_v1(
    expected: &LayerOrderSnapshot,
    actual: &LayerOrderSnapshot,
    checkpoint: &mut impl FnMut() -> Result<(), CooperativeOperationStopV1>,
) -> Result<bool, CooperativeOperationStopV1> {
    checkpoint()?;
    if expected.model_id != actual.model_id
        || expected.provenance != actual.provenance
        || expected.reference_face != actual.reference_face
        || expected.proof_summary != actual.proof_summary
        || !slice_equal_with_checkpoint_v1(
            &expected.material_faces,
            &actual.material_faces,
            checkpoint,
        )?
    {
        return Ok(false);
    }
    match (&expected.global_bottom_to_top, &actual.global_bottom_to_top) {
        (Some(expected), Some(actual)) => {
            if !slice_equal_with_checkpoint_v1(expected, actual, checkpoint)? {
                return Ok(false);
            }
        }
        (None, None) => {}
        _ => return Ok(false),
    }
    if expected.folded_faces.len() != actual.folded_faces.len() {
        return Ok(false);
    }
    for (expected, actual) in expected.folded_faces.iter().zip(&actual.folded_faces) {
        checkpoint()?;
        if expected != actual {
            return Ok(false);
        }
    }
    if expected.overlap_cells.len() != actual.overlap_cells.len() {
        return Ok(false);
    }
    for (expected, actual) in expected.overlap_cells.iter().zip(&actual.overlap_cells) {
        checkpoint()?;
        if expected.cell_key != actual.cell_key
            || !slice_equal_with_checkpoint_v1(
                &expected.exact_boundary,
                &actual.exact_boundary,
                checkpoint,
            )?
            || !slice_equal_with_checkpoint_v1(
                &expected.covering_faces,
                &actual.covering_faces,
                checkpoint,
            )?
            || !slice_equal_with_checkpoint_v1(
                &expected.bottom_to_top_faces,
                &actual.bottom_to_top_faces,
                checkpoint,
            )?
        {
            return Ok(false);
        }
    }
    if expected.face_pair_orders.len() != actual.face_pair_orders.len() {
        return Ok(false);
    }
    for (expected, actual) in expected
        .face_pair_orders
        .iter()
        .zip(&actual.face_pair_orders)
    {
        checkpoint()?;
        if expected.lower_face != actual.lower_face
            || expected.upper_face != actual.upper_face
            || !slice_equal_with_checkpoint_v1(
                &expected.supporting_cells,
                &actual.supporting_cells,
                checkpoint,
            )?
        {
            return Ok(false);
        }
    }
    checkpoint()?;
    Ok(true)
}

fn slice_equal_with_checkpoint_v1<T: PartialEq>(
    expected: &[T],
    actual: &[T],
    checkpoint: &mut impl FnMut() -> Result<(), CooperativeOperationStopV1>,
) -> Result<bool, CooperativeOperationStopV1> {
    if expected.len() != actual.len() {
        return Ok(false);
    }
    for (expected, actual) in expected.iter().zip(actual) {
        checkpoint()?;
        if expected != actual {
            return Ok(false);
        }
    }
    Ok(true)
}

pub fn certify_general_multi_face_cell_transport_v1(
    input: GeneralCellTransportInputV1<'_>,
) -> Result<GeneralMultiFaceCellTransportProofV1, GeneralCellTransportErrorV1> {
    certify_general_multi_face_cell_transport_with_peak_limit_v1(
        input,
        ori_foldability::DEFAULT_MAX_CERTIFICATE_BYTES,
    )
}

fn certify_general_multi_face_cell_transport_with_peak_limit_v1(
    input: GeneralCellTransportInputV1<'_>,
    maximum_peak_bytes: usize,
) -> Result<GeneralMultiFaceCellTransportProofV1, GeneralCellTransportErrorV1> {
    let source_retained_bytes = input
        .source
        .checked_deep_retained_bytes_v1()
        .ok_or(GeneralCellTransportErrorV1::ResourceLimit)?;
    preflight_general_cell_transport_source_retention_v1(
        source_retained_bytes,
        ori_foldability::DEFAULT_MAX_CERTIFICATE_BYTES,
    )?;
    if !input.paper_thickness_mm.is_finite()
        || input.paper_thickness_mm <= 0.0
        || !input.tolerance.is_finite()
        || input.tolerance < 0.0
        || !input.positive_continuous.is_for(
            input.geometry,
            input.audit,
            input.closure.fixed_face(),
            input.schedule,
            input.closure,
            input.paper_thickness_mm,
        )
    {
        return Err(GeneralCellTransportErrorV1::BindingMismatch);
    }
    let radial_bifold_family = crate::continuous_path::scheduled_opposite_radial_bifold_premises_v1(
        input.geometry,
        input.audit,
        input.closure.fixed_face(),
        input.schedule,
        input.closure,
        Some(input.paper_thickness_mm),
    )
        || crate::continuous_path::scheduled_separated_common_articulation_bifolds_premises_v1(
            input.geometry,
            input.audit,
            input.closure.fixed_face(),
            input.schedule,
            input.closure,
            input.paper_thickness_mm,
            None,
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
    let maximum_boundary_points = input
        .source
        .overlap_cells
        .iter()
        .map(|cell| cell.exact_boundary.len())
        .max()
        .unwrap_or(0);
    let memory = checked_general_cell_transport_input_memory_work_v1(&input)
        .ok_or(GeneralCellTransportErrorV1::ResourceLimit)?;
    let peak_bytes = checked_general_cell_transport_peak_bytes_v1(source_retained_bytes, memory)
        .ok_or(GeneralCellTransportErrorV1::ResourceLimit)?;
    preflight_general_cell_transport_peak_bytes_v1(peak_bytes, maximum_peak_bytes)?;

    let mut prepared_faces = Vec::new();
    prepared_faces
        .try_reserve_exact(input.source.folded_faces.len())
        .map_err(|_| GeneralCellTransportErrorV1::ResourceLimit)?;
    for folded in &input.source.folded_faces {
        prepared_faces.push(PreparedFoldedFaceV1 {
            face: folded.face.face_id,
            folded,
            inverse: prepare_inverse_flat_transform(folded)?,
        });
    }
    prepared_faces.sort_unstable_by_key(|entry| entry.face.canonical_bytes());
    if prepared_faces.len() != input.source.material_faces.len()
        || prepared_faces
            .windows(2)
            .any(|pair| pair[0].face == pair[1].face)
    {
        return Err(GeneralCellTransportErrorV1::BindingMismatch);
    }

    let mut cells = Vec::new();
    cells
        .try_reserve_exact(input.source.overlap_cells.len())
        .map_err(|_| GeneralCellTransportErrorV1::ResourceLimit)?;
    cells.extend(input.source.overlap_cells.iter());
    cells.sort_unstable_by_key(|cell| cell.cell_key.0);

    let parameters = input
        .closure
        .leaves()
        .iter()
        .map(|(depth, index, _)| *index as f64 / 2_f64.powi(*depth as i32))
        .chain(std::iter::once(1.0));
    let mut checkpoint_hashes = Vec::new();
    checkpoint_hashes
        .try_reserve_exact(transition_count)
        .map_err(|_| GeneralCellTransportErrorV1::ResourceLimit)?;
    let mut previous_layer_boundary = Vec::new();
    previous_layer_boundary
        .try_reserve_exact(maximum_boundary_points)
        .map_err(|_| GeneralCellTransportErrorV1::ResourceLimit)?;
    let mut actual_memory = checked_general_cell_transport_memory_work_v1(
        checkpoint_hashes.capacity(),
        prepared_faces.capacity(),
        cells.capacity(),
        previous_layer_boundary.capacity(),
    )
    .ok_or(GeneralCellTransportErrorV1::ResourceLimit)?;
    let projected_runtime_peak = checked_general_cell_transport_runtime_peak_bytes_v1(
        input.geometry,
        input.audit,
        input.source,
    )
    .ok_or(GeneralCellTransportErrorV1::ResourceLimit)?;
    actual_memory.peak_temporary_bytes = actual_memory
        .peak_temporary_bytes
        .checked_add(projected_runtime_peak)
        .ok_or(GeneralCellTransportErrorV1::ResourceLimit)?;
    let actual_peak =
        checked_general_cell_transport_peak_bytes_v1(source_retained_bytes, actual_memory)
            .ok_or(GeneralCellTransportErrorV1::ResourceLimit)?;
    preflight_general_cell_transport_peak_bytes_v1(actual_peak, maximum_peak_bytes)?;
    for parameter in parameters {
        let angles = input.schedule.try_evaluate_v1(parameter).map_err(|error| {
            if error == ori_kinematics::KinematicsError::ResourceLimitExceeded {
                GeneralCellTransportErrorV1::ResourceLimit
            } else {
                GeneralCellTransportErrorV1::BindingMismatch
            }
        })?;
        let pose = input
            .geometry
            .solve_closed(
                input.audit,
                input.closure.fixed_face(),
                &angles,
                input.tolerance.max(1.0e-12),
            )
            .map_err(|error| {
                if error == ori_kinematics::KinematicsError::ResourceLimitExceeded {
                    GeneralCellTransportErrorV1::ResourceLimit
                } else {
                    GeneralCellTransportErrorV1::BindingMismatch
                }
            })?;
        let actual_runtime_peak = angles
            .checked_retained_bytes_v1()
            .and_then(|bytes| bytes.checked_add(pose.checked_deep_retained_bytes_v1()?))
            .ok_or(GeneralCellTransportErrorV1::ResourceLimit)?;
        let mut iteration_memory = actual_memory;
        iteration_memory.peak_temporary_bytes = iteration_memory
            .peak_temporary_bytes
            .checked_sub(projected_runtime_peak)
            .and_then(|bytes| bytes.checked_add(projected_runtime_peak.max(actual_runtime_peak)))
            .ok_or(GeneralCellTransportErrorV1::ResourceLimit)?;
        let iteration_peak =
            checked_general_cell_transport_peak_bytes_v1(source_retained_bytes, iteration_memory)
                .ok_or(GeneralCellTransportErrorV1::ResourceLimit)?;
        preflight_general_cell_transport_peak_bytes_v1(iteration_peak, maximum_peak_bytes)?;
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
            previous_layer_boundary.clear();
            for (rank, face) in cell.bottom_to_top_faces.iter().copied().enumerate() {
                let prepared = prepared_faces
                    .binary_search_by_key(&face.canonical_bytes(), |entry| {
                        entry.face.canonical_bytes()
                    })
                    .ok()
                    .and_then(|index| prepared_faces.get(index))
                    .ok_or(GeneralCellTransportErrorV1::BindingMismatch)?;
                let transform = pose
                    .face_transform(face)
                    .ok_or(GeneralCellTransportErrorV1::BindingMismatch)?;
                let sign = match prepared.folded.orientation {
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
                for (point_index, point) in cell.exact_boundary.iter().enumerate() {
                    let flat = Point2::new(
                        point
                            .x
                            .to_f64_without_heap_v1()
                            .ok_or(GeneralCellTransportErrorV1::GeometryUnavailable)?,
                        point
                            .y
                            .to_f64_without_heap_v1()
                            .ok_or(GeneralCellTransportErrorV1::GeometryUnavailable)?,
                    );
                    let material = inverse_flat_point(&prepared.inverse, flat)?;
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
                    if rank == 0 {
                        previous_layer_boundary.push(offset_world);
                    } else {
                        let lower = previous_layer_boundary
                            .get_mut(point_index)
                            .ok_or(GeneralCellTransportErrorV1::BindingMismatch)?;
                        let separation = ((offset_world[0] - lower[0]).powi(2)
                            + (offset_world[1] - lower[1]).powi(2)
                            + (offset_world[2] - lower[2]).powi(2))
                        .sqrt();
                        if separation + input.tolerance < input.paper_thickness_mm
                            && !radial_bifold_family
                        {
                            return Err(GeneralCellTransportErrorV1::Crossing);
                        }
                        *lower = offset_world;
                    }
                }
                hash.update(face.canonical_bytes());
            }
            hash.update(cell.cell_key.0);
        }
        checkpoint_hashes.push(hash.finalize().into());
    }
    drop(previous_layer_boundary);
    drop(cells);
    drop(prepared_faces);
    let retained_source = input
        .source
        .try_clone_with_retained_byte_limit_v1(source_retained_bytes)
        .map_err(|_| GeneralCellTransportErrorV1::ResourceLimit)?;
    Ok(GeneralMultiFaceCellTransportProofV1 {
        issuer: input.geometry.instance_anchor_v1(),
        source_instance: input.source as *const LayerOrderSnapshot as usize,
        source: retained_source,
        schedule_hash: input.schedule.certificate_binding_fingerprint_v2(),
        closure_hash: input.closure.partition_binding_fingerprint_v2(),
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

#[derive(Clone, Copy)]
struct PreparedFoldedFaceV1<'a> {
    face: ori_domain::FaceId,
    folded: &'a ori_foldability::FoldedFaceSnapshot,
    inverse: InverseFlatTransform,
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

    #[test]
    fn retained_source_is_admitted_only_at_the_exact_byte_limit() {
        assert_eq!(
            preflight_general_cell_transport_source_retention_v1(4_096, 4_096),
            Ok(())
        );
        assert_eq!(
            preflight_general_cell_transport_source_retention_v1(4_096, 4_095),
            Err(GeneralCellTransportErrorV1::ResourceLimit)
        );
    }

    #[test]
    fn memory_peak_is_checked_for_proof_retention_and_streaming_workspace() {
        let memory =
            checked_general_cell_transport_memory_work_v1(2, 3, 5, 7).expect("bounded memory work");
        let expected_proof = (std::mem::size_of::<GeneralMultiFaceCellTransportProofV1>()
            - std::mem::size_of::<LayerOrderSnapshot>())
            + 2 * std::mem::size_of::<[u8; 32]>();
        let expected_temporary = 3 * std::mem::size_of::<PreparedFoldedFaceV1<'static>>()
            + 5 * std::mem::size_of::<&ori_foldability::OverlapCellSnapshot>()
            + 7 * std::mem::size_of::<[f64; 3]>();
        assert_eq!(memory.proof_retained_bytes, expected_proof);
        assert_eq!(memory.peak_temporary_bytes, expected_temporary);

        let peak = checked_general_cell_transport_peak_bytes_v1(101, memory)
            .expect("proof metadata plus the larger clone or workspace phase");
        assert_eq!(
            peak,
            101 + expected_proof + 101usize.max(expected_temporary)
        );
        assert_eq!(
            preflight_general_cell_transport_peak_bytes_v1(peak, peak),
            Ok(())
        );
        assert_eq!(
            preflight_general_cell_transport_peak_bytes_v1(peak, peak - 1),
            Err(GeneralCellTransportErrorV1::ResourceLimit)
        );
    }

    #[test]
    fn chain_count_fails_closed_outside_the_proved_one_to_three_range() {
        assert_eq!(
            preflight_general_cell_transport_chain_count_v1(0),
            Err(GeneralCellTransportErrorV1::BindingMismatch)
        );
        for count in 1..=GENERAL_CELL_TRANSPORT_CHAIN_LIMIT_V1 {
            assert_eq!(
                preflight_general_cell_transport_chain_count_v1(count),
                Ok(())
            );
        }
        assert_eq!(
            preflight_general_cell_transport_chain_count_v1(
                GENERAL_CELL_TRANSPORT_CHAIN_LIMIT_V1 + 1
            ),
            Err(GeneralCellTransportErrorV1::ResourceLimit)
        );
    }

    #[test]
    fn chain_schedule_continuity_never_equates_two_failed_endpoint_evaluations() {
        let edge = ori_domain::EdgeId::new();
        let canonical = |degrees| {
            ori_kinematics::CanonicalHingeAngles::new(vec![
                ori_kinematics::HingeAngle::new(edge, degrees).expect("finite test angle"),
            ])
            .expect("single canonical angle")
        };
        let first = canonical(0.0);
        let equal = first.try_clone_v1().expect("equal canonical angle");
        assert_eq!(
            preflight_general_cell_transport_schedule_continuity_v1(Ok(first), Ok(equal)),
            Ok(())
        );
        assert_eq!(
            preflight_general_cell_transport_schedule_continuity_v1(
                Ok(canonical(0.0)),
                Ok(canonical(1.0)),
            ),
            Err(GeneralCellTransportErrorV1::BindingMismatch)
        );
        assert_eq!(
            preflight_general_cell_transport_schedule_continuity_v1(
                Err(ori_kinematics::KinematicsError::UnrepresentableGeometry),
                Err(ori_kinematics::KinematicsError::UnrepresentableGeometry),
            ),
            Err(GeneralCellTransportErrorV1::BindingMismatch)
        );
        assert_eq!(
            preflight_general_cell_transport_schedule_continuity_v1(
                Err(ori_kinematics::KinematicsError::ResourceLimitExceeded),
                Ok(canonical(0.0)),
            ),
            Err(GeneralCellTransportErrorV1::ResourceLimit)
        );
    }

    #[test]
    fn chained_peak_accounts_for_input_buffer_retained_proofs_and_next_work() {
        let memories = [
            GeneralCellTransportMemoryWorkV1 {
                proof_retained_bytes: 17,
                peak_temporary_bytes: 19,
            },
            GeneralCellTransportMemoryWorkV1 {
                proof_retained_bytes: 23,
                peak_temporary_bytes: 29,
            },
        ];
        let source_retained_bytes = 101usize;
        let input_bytes = 2 * std::mem::size_of::<GeneralCellTransportInputV1<'static>>();
        let proof_buffer_bytes = 2 * std::mem::size_of::<GeneralMultiFaceCellTransportProofV1>();
        let first_peak = input_bytes
            + proof_buffer_bytes
            + source_retained_bytes
            + memories[0].proof_retained_bytes
            + source_retained_bytes.max(memories[0].peak_temporary_bytes);
        let second_peak = input_bytes
            + proof_buffer_bytes
            + source_retained_bytes
            + memories[0].proof_retained_bytes
            + source_retained_bytes
            + memories[1].proof_retained_bytes
            + source_retained_bytes.max(memories[1].peak_temporary_bytes);
        let final_retained = input_bytes
            + proof_buffer_bytes
            + 3 * source_retained_bytes
            + memories
                .iter()
                .map(|memory| memory.proof_retained_bytes)
                .sum::<usize>();
        let expected = first_peak.max(second_peak).max(final_retained);
        let peak = checked_chained_general_cell_transport_peak_bytes_v1(
            2,
            2,
            source_retained_bytes,
            &memories,
        )
        .expect("bounded two-proof chain");
        assert_eq!(peak, expected);
        assert_eq!(
            preflight_general_cell_transport_peak_bytes_v1(peak, peak),
            Ok(())
        );
        assert_eq!(
            preflight_general_cell_transport_peak_bytes_v1(peak, peak - 1),
            Err(GeneralCellTransportErrorV1::ResourceLimit)
        );
    }

    #[test]
    fn chained_peak_rejects_invalid_capacity_and_every_overflow_dimension() {
        let memory = GeneralCellTransportMemoryWorkV1 {
            proof_retained_bytes: 0,
            peak_temporary_bytes: 0,
        };
        assert_eq!(
            checked_chained_general_cell_transport_peak_bytes_v1(0, 1, 1, &[memory]),
            None
        );
        assert_eq!(
            checked_chained_general_cell_transport_peak_bytes_v1(1, 0, 1, &[memory]),
            None
        );
        assert_eq!(
            checked_chained_general_cell_transport_peak_bytes_v1(0, 0, 1, &[]),
            None
        );
        assert_eq!(
            checked_chained_general_cell_transport_peak_bytes_v1(
                4,
                4,
                1,
                &[memory; GENERAL_CELL_TRANSPORT_CHAIN_LIMIT_V1 + 1],
            ),
            None
        );
        assert_eq!(
            checked_chained_general_cell_transport_peak_bytes_v1(usize::MAX, 1, 1, &[memory],),
            None
        );
        assert_eq!(
            checked_chained_general_cell_transport_peak_bytes_v1(1, usize::MAX, 1, &[memory],),
            None
        );
        assert_eq!(
            checked_chained_general_cell_transport_peak_bytes_v1(1, 1, usize::MAX, &[memory],),
            None
        );
        assert_eq!(
            checked_chained_general_cell_transport_peak_bytes_v1(
                1,
                1,
                1,
                &[GeneralCellTransportMemoryWorkV1 {
                    proof_retained_bytes: usize::MAX,
                    peak_temporary_bytes: 0,
                }],
            ),
            None
        );
        assert_eq!(
            checked_chained_general_cell_transport_peak_bytes_v1(
                1,
                1,
                1,
                &[GeneralCellTransportMemoryWorkV1 {
                    proof_retained_bytes: 0,
                    peak_temporary_bytes: usize::MAX,
                }],
            ),
            None
        );
    }

    #[test]
    fn chained_proof_buffer_allocation_failure_is_recoverable() {
        assert!(matches!(
            try_general_cell_transport_proof_buffer_v1(usize::MAX),
            Err(GeneralCellTransportErrorV1::ResourceLimit)
        ));
    }

    #[test]
    fn every_memory_peak_dimension_rejects_overflow() {
        assert_eq!(
            checked_general_cell_transport_memory_work_v1(usize::MAX, 0, 0, 0),
            None
        );
        assert_eq!(
            checked_general_cell_transport_memory_work_v1(0, usize::MAX, 0, 0),
            None
        );
        assert_eq!(
            checked_general_cell_transport_memory_work_v1(0, 0, usize::MAX, 0),
            None
        );
        assert_eq!(
            checked_general_cell_transport_memory_work_v1(0, 0, 0, usize::MAX),
            None
        );
        let zero = GeneralCellTransportMemoryWorkV1 {
            proof_retained_bytes: 0,
            peak_temporary_bytes: 0,
        };
        assert_eq!(
            checked_general_cell_transport_peak_bytes_v1(usize::MAX, zero),
            None
        );
    }
}
