use super::*;
use crate::proof_cache::{
    ExactFacePoseCacheWitnessV1, ExactFacePoseComponentsV1, FaceDependencyFootprintV1,
    ProofCacheErrorV1, ProofCachePairWorkLimitsV1, ProofCachePairWorkV1,
};

/// One exact pose preparation shared by pair-local cache misses.
///
/// This layer deliberately has no certificate-model, issuer-context, cache
/// key, or publication authority. It only exposes the exact pair observation
/// and the complete pair-local geometry dependencies; the continuous theorem
/// that consumes it remains the sole authority that may decide whether an
/// observation is cacheable.
pub(crate) struct PositiveThicknessExactPairCacheSessionV1<'a> {
    exact: RationalCayleyTreePose<'a>,
    half_thickness: BigRational,
    faces: Vec<PositiveThicknessExactFaceCacheSnapshotV1>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PositiveThicknessExactFaceCacheSnapshotV1 {
    pub(crate) footprint: FaceDependencyFootprintV1,
    pub(crate) exact_pose: ExactFacePoseCacheWitnessV1,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PositiveThicknessExactPairCacheObservationV1 {
    pub(crate) diagnostic: PositiveThicknessPrismPairDiagnosticV1,
    pub(crate) work: ProofCachePairWorkV1,
    pub(crate) dependencies: [PositiveThicknessExactFaceCacheSnapshotV1; 2],
}

pub(crate) fn prepare_positive_thickness_exact_pair_cache_session_v1(
    bound: BoundMaterialTreePose<'_>,
    paper_thickness_mm: f64,
) -> Result<PositiveThicknessExactPairCacheSessionV1<'_>, PositiveThicknessPrismScanErrorV1> {
    if !positive_finite_binary64(paper_thickness_mm)
        || bound.model().face_ids() != bound.pose().face_ids()
        || bound.model().hinges() != bound.pose().hinges()
    {
        return Err(PositiveThicknessPrismScanErrorV1::InconsistentPose);
    }
    let exact = prepare_rational_cayley_tree_pose_v1(bound, ExactTreePoseLimits::default())
        .map_err(map_exact_pair_cache_cayley_error_v1)?;
    if exact
        .faces
        .windows(2)
        .any(|faces| faces[0].face.canonical_bytes() >= faces[1].face.canonical_bytes())
    {
        return Err(PositiveThicknessPrismScanErrorV1::InconsistentPose);
    }
    let half_thickness = BigRational::from_float(paper_thickness_mm)
        .ok_or(PositiveThicknessPrismScanErrorV1::InconsistentPose)?
        / BigRational::from_integer(2.into());
    let mut faces = Vec::new();
    faces
        .try_reserve_exact(exact.faces.len())
        .map_err(|_| PositiveThicknessPrismScanErrorV1::ResourceLimitExceeded)?;
    for face in &exact.faces {
        let boundary = bound
            .face_boundary(face.face)
            .ok_or(PositiveThicknessPrismScanErrorV1::InconsistentPose)?;
        if boundary.vertices().len() != face.boundary.len()
            || boundary.vertices().len() < 3
            || boundary.edges().len() != boundary.vertices().len()
            || face
                .boundary
                .iter()
                .map(|(vertex, _)| *vertex)
                .ne(boundary.vertices().iter().copied())
        {
            return Err(PositiveThicknessPrismScanErrorV1::InconsistentPose);
        }
        let footprint = FaceDependencyFootprintV1::from_complete_face_v1(
            face.face,
            boundary.vertices().to_vec(),
            boundary.edges().to_vec(),
        )
        .map_err(map_exact_pair_cache_proof_error_v1)?;
        let mut exact_boundary = Vec::new();
        exact_boundary
            .try_reserve_exact(face.boundary.len())
            .map_err(|_| PositiveThicknessPrismScanErrorV1::ResourceLimitExceeded)?;
        for (vertex, point) in &face.boundary {
            exact_boundary.push((*vertex, point.coordinates.clone()));
        }
        let exact_pose =
            ExactFacePoseCacheWitnessV1::from_exact_components_v1(ExactFacePoseComponentsV1 {
                face: face.face,
                rotation: &face.transform.rotation,
                translation: &face.transform.translation.coordinates,
                boundary: &exact_boundary,
            })
            .map_err(map_exact_pair_cache_proof_error_v1)?;
        faces.push(PositiveThicknessExactFaceCacheSnapshotV1 {
            footprint,
            exact_pose,
        });
    }
    Ok(PositiveThicknessExactPairCacheSessionV1 {
        exact,
        half_thickness,
        faces,
    })
}

impl PositiveThicknessExactPairCacheSessionV1<'_> {
    pub(crate) fn analyze_pair_v1(
        &self,
        first: FaceId,
        second: FaceId,
    ) -> Result<PositiveThicknessExactPairCacheObservationV1, PositiveThicknessPrismScanErrorV1>
    {
        if first == second {
            return Err(PositiveThicknessPrismScanErrorV1::InconsistentPose);
        }
        let first = self
            .face_index_v1(first)
            .ok_or(PositiveThicknessPrismScanErrorV1::InconsistentPose)?;
        let second = self
            .face_index_v1(second)
            .ok_or(PositiveThicknessPrismScanErrorV1::InconsistentPose)?;
        let (first, second) = if first < second {
            (first, second)
        } else {
            (second, first)
        };
        let task = analyze_positive_thickness_prism_pair_v1(
            &self.exact,
            &self.half_thickness,
            first,
            second,
            ExactPrismLimits::default(),
        )?;
        Ok(PositiveThicknessExactPairCacheObservationV1 {
            diagnostic: task.diagnostic,
            work: proof_cache_pair_work_from_exact_prism_v1(&task.work),
            dependencies: [self.faces[first].clone(), self.faces[second].clone()],
        })
    }

    pub(crate) fn complete_face_snapshots_v1(
        &self,
    ) -> &[PositiveThicknessExactFaceCacheSnapshotV1] {
        &self.faces
    }

    fn face_index_v1(&self, face: FaceId) -> Option<usize> {
        self.exact
            .faces
            .binary_search_by_key(&face.canonical_bytes(), |item| item.face.canonical_bytes())
            .ok()
    }
}

pub(crate) fn positive_thickness_exact_pair_cache_work_limits_v1(
    pair_count: usize,
) -> Result<ProofCachePairWorkLimitsV1, PositiveThicknessPrismScanErrorV1> {
    let envelope = reserve_positive_thickness_prism_parallel_envelope_v1(pair_count)?;
    let exact = &envelope.exact_limits;
    let envelope_work = proof_cache_pair_work_from_exact_prism_v1(&envelope.exact_prism);
    let mut additive = *envelope_work.additive_counters();
    additive[17] = exact.max_interval_operations;
    additive[18] = exact.max_interval_operations;
    additive[19] = exact.max_interval_operations;
    additive[20] = exact.max_interval_operations;
    additive[21] = exact.max_gcd_fallback_calls;
    additive[22] = exact.max_gcd_fallback_input_bits;
    additive[23] = exact.max_rational_allocations;
    additive[24] = exact.max_total_rational_allocation_bits;
    let maximum = [
        envelope.exact_prism.max_input_rational_storage_bits,
        exact.max_machin_terms_per_series,
        exact.max_trig_terms_per_series,
        exact.max_sqrt_refinements,
        exact.max_shift_bits,
        exact.max_intermediate_bits,
        exact.max_intermediate_bits.max(exact.max_output_bits),
        exact.max_gcd_fallback_input_bits,
        exact.max_rational_allocation_bits,
        exact.max_output_bits,
    ];
    Ok(ProofCachePairWorkLimitsV1::new(additive, maximum))
}

fn proof_cache_pair_work_from_exact_prism_v1(work: &ExactPrismWork) -> ProofCachePairWorkV1 {
    ProofCachePairWorkV1::from_exact_pair_counters_v1(
        [
            work.prisms,
            work.solid_vertices,
            work.facets,
            work.halfspaces,
            work.prism_volume_tests,
            work.facet_vertex_checks,
            work.plane_triples,
            work.singular_plane_triples,
            work.nonsingular_solves,
            work.membership_tests,
            work.candidate_vertices,
            work.dedup_comparisons,
            work.affine_rank_tests,
            work.support_plane_vertex_tests,
            work.support_pair_tests,
            work.input_rationals,
            work.total_input_storage_bits,
            work.exact.interval_operations,
            work.exact.machin_terms,
            work.exact.trig_terms,
            work.exact.sqrt_refinements,
            work.exact.gcd_fallback_calls,
            work.exact.gcd_fallback_input_bits,
            work.exact.rational_allocations,
            work.exact.total_rational_allocation_bits,
        ],
        [
            work.max_input_rational_storage_bits,
            work.exact.max_machin_series_terms,
            work.exact.max_trig_series_terms,
            work.exact.max_sqrt_call_refinements,
            work.exact.max_shift_bits,
            work.exact.max_preflight_bits,
            work.exact.max_observed_bits,
            work.exact.max_gcd_fallback_call_input_bits,
            work.exact.max_rational_allocation_bits,
            work.exact.max_output_bits,
        ],
    )
}

const fn map_exact_pair_cache_cayley_error_v1(
    error: CayleyError,
) -> PositiveThicknessPrismScanErrorV1 {
    match error {
        CayleyError::ResourceLimitExceeded { .. } => {
            PositiveThicknessPrismScanErrorV1::ResourceLimitExceeded
        }
        _ => PositiveThicknessPrismScanErrorV1::InconsistentPose,
    }
}

const fn map_exact_pair_cache_proof_error_v1(
    error: ProofCacheErrorV1,
) -> PositiveThicknessPrismScanErrorV1 {
    match error {
        ProofCacheErrorV1::ResourceLimitExceeded => {
            PositiveThicknessPrismScanErrorV1::ResourceLimitExceeded
        }
        _ => PositiveThicknessPrismScanErrorV1::InconsistentPose,
    }
}
