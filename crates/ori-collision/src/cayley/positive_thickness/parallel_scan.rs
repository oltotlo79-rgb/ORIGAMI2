use std::sync::atomic::AtomicBool;

use super::super::{checked_work_product, checked_work_sum, parallel_meter};
use super::exact_prism::{
    ExactPrismAnalysis, ExactPrismLimits, ExactPrismWork, analyze_exact_prism_pair_v1,
};
use super::*;

#[cfg(test)]
#[path = "parallel_tests.rs"]
mod tests;

#[derive(Debug, Clone, Copy)]
pub(crate) struct PositiveThicknessPrismParallelConfigV1<'a> {
    pub(crate) worker_threads: usize,
    pub(crate) cancellation: Option<&'a AtomicBool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PositiveThicknessPrismScanErrorV1 {
    Cancelled,
    ResourceLimitExceeded,
    InconsistentPose,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PositiveThicknessPrismScanV1 {
    diagnostics: Vec<PositiveThicknessPrismPairDiagnosticV1>,
    work: PositiveThicknessPrismScanWorkV1,
}

impl PositiveThicknessPrismScanV1 {
    pub(crate) fn into_diagnostics(self) -> Vec<PositiveThicknessPrismPairDiagnosticV1> {
        self.diagnostics
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct PositiveThicknessPrismScanWorkV1 {
    expected_pairs: usize,
    completed_pairs: usize,
    exact_prism: ExactPrismWork,
}

#[derive(Debug)]
struct PositiveThicknessPrismPairTaskV1 {
    diagnostic: PositiveThicknessPrismPairDiagnosticV1,
    work: ExactPrismWork,
}

#[derive(Debug, Clone)]
struct PositiveThicknessPrismParallelEnvelopeV1 {
    exact_prism: ExactPrismWork,
    exact_limits: CayleyLimits,
}

pub(crate) fn diagnose_bound_positive_thickness_prism_pairs_v1(
    bound: BoundMaterialTreePose<'_>,
    paper_thickness_mm: f64,
    max_unordered_face_pairs: usize,
) -> Result<Vec<PositiveThicknessPrismPairDiagnosticV1>, SharedHingeSolidDiagnosticErrorV1> {
    diagnose_bound_positive_thickness_prism_pairs_with_observer_v1(
        bound,
        paper_thickness_mm,
        max_unordered_face_pairs,
        PositiveThicknessPrismParallelConfigV1 {
            worker_threads: 1,
            cancellation: None,
        },
        &|_| {},
        &|_| ExactPrismLimits::default(),
    )
    .map(PositiveThicknessPrismScanV1::into_diagnostics)
    .map_err(|error| match error {
        PositiveThicknessPrismScanErrorV1::ResourceLimitExceeded => {
            SharedHingeSolidDiagnosticErrorV1::ResourceLimitExceeded
        }
        PositiveThicknessPrismScanErrorV1::Cancelled
        | PositiveThicknessPrismScanErrorV1::InconsistentPose => {
            SharedHingeSolidDiagnosticErrorV1::InconsistentPose
        }
    })
}

pub(crate) fn diagnose_bound_positive_thickness_prism_pairs_parallel_v1(
    bound: BoundMaterialTreePose<'_>,
    paper_thickness_mm: f64,
    max_unordered_face_pairs: usize,
    config: PositiveThicknessPrismParallelConfigV1<'_>,
) -> Result<PositiveThicknessPrismScanV1, PositiveThicknessPrismScanErrorV1> {
    diagnose_bound_positive_thickness_prism_pairs_with_observer_v1(
        bound,
        paper_thickness_mm,
        max_unordered_face_pairs,
        config,
        &|_| {},
        &|_| ExactPrismLimits::default(),
    )
}

fn diagnose_bound_positive_thickness_prism_pairs_with_observer_v1(
    bound: BoundMaterialTreePose<'_>,
    paper_thickness_mm: f64,
    max_unordered_face_pairs: usize,
    config: PositiveThicknessPrismParallelConfigV1<'_>,
    observe_completion: &(dyn Fn(usize) + Sync),
    pair_limits: &(dyn Fn(usize) -> ExactPrismLimits + Sync),
) -> Result<PositiveThicknessPrismScanV1, PositiveThicknessPrismScanErrorV1> {
    if !positive_finite_binary64(paper_thickness_mm)
        || bound.model().face_ids() != bound.pose().face_ids()
        || bound.model().hinges() != bound.pose().hinges()
    {
        return Err(PositiveThicknessPrismScanErrorV1::InconsistentPose);
    }
    let exact = prepare_rational_cayley_tree_pose_v1(bound, ExactTreePoseLimits::default())
        .map_err(|error| match error {
            CayleyError::ResourceLimitExceeded { .. } => {
                PositiveThicknessPrismScanErrorV1::ResourceLimitExceeded
            }
            _ => PositiveThicknessPrismScanErrorV1::InconsistentPose,
        })?;
    let pair_count = exact
        .faces
        .len()
        .checked_mul(exact.faces.len().saturating_sub(1))
        .and_then(|value| value.checked_div(2))
        .ok_or(PositiveThicknessPrismScanErrorV1::ResourceLimitExceeded)?;
    if pair_count > max_unordered_face_pairs {
        return Err(PositiveThicknessPrismScanErrorV1::ResourceLimitExceeded);
    }
    let half_thickness = BigRational::from_float(paper_thickness_mm)
        .ok_or(PositiveThicknessPrismScanErrorV1::InconsistentPose)?
        / BigRational::from_integer(2.into());
    if config.worker_threads == 0
        || config.worker_threads > MAX_POSITIVE_THICKNESS_PRISM_PARALLEL_WORKERS_V1
    {
        return Err(PositiveThicknessPrismScanErrorV1::ResourceLimitExceeded);
    }
    if exact
        .faces
        .windows(2)
        .any(|faces| faces[0].face.canonical_bytes() >= faces[1].face.canonical_bytes())
    {
        return Err(PositiveThicknessPrismScanErrorV1::InconsistentPose);
    }

    // The authenticated exact pose is already strictly FaceId-canonical, so
    // this nested enumeration is the stable canonical pair sort without an
    // allocation-prone second sorting pass.
    let mut canonical_pairs = Vec::new();
    canonical_pairs
        .try_reserve_exact(pair_count)
        .map_err(|_| PositiveThicknessPrismScanErrorV1::ResourceLimitExceeded)?;
    for first in 0..exact.faces.len() {
        for second in first + 1..exact.faces.len() {
            canonical_pairs.push((first, second));
        }
    }
    let envelope = reserve_positive_thickness_prism_parallel_envelope_v1(pair_count)?;
    let mut diagnostics = Vec::new();
    diagnostics
        .try_reserve_exact(pair_count)
        .map_err(|_| PositiveThicknessPrismScanErrorV1::ResourceLimitExceeded)?;
    let tasks = parallel_meter::execute_canonical_pairs(
        pair_count,
        config.worker_threads,
        config.cancellation,
        |pair_index| {
            let (first, second) = canonical_pairs[pair_index];
            let result = analyze_positive_thickness_prism_pair_v1(
                &exact,
                &half_thickness,
                first,
                second,
                pair_limits(pair_index),
            );
            observe_completion(pair_index);
            result
        },
    )
    .map_err(|error| match error {
        parallel_meter::CanonicalPairExecutionError::Cancelled => {
            PositiveThicknessPrismScanErrorV1::Cancelled
        }
        parallel_meter::CanonicalPairExecutionError::ResourceLimitExceeded => {
            PositiveThicknessPrismScanErrorV1::ResourceLimitExceeded
        }
    })?;

    let mut exact_prism = ExactPrismWork::default();
    for task in tasks {
        let task = task?;
        exact_prism =
            checked_merge_exact_prism_parallel_work_v1(&exact_prism, &task.work, &envelope)?;
        diagnostics.push(task.diagnostic);
    }
    Ok(PositiveThicknessPrismScanV1 {
        diagnostics,
        work: PositiveThicknessPrismScanWorkV1 {
            expected_pairs: pair_count,
            completed_pairs: pair_count,
            exact_prism,
        },
    })
}

fn analyze_positive_thickness_prism_pair_v1(
    exact: &RationalCayleyTreePose,
    half_thickness: &BigRational,
    first: usize,
    second: usize,
    limits: ExactPrismLimits,
) -> Result<PositiveThicknessPrismPairTaskV1, PositiveThicknessPrismScanErrorV1> {
    let prism = |face: &ExactFacePose| -> Option<ExactTriangularPrismInput> {
        (face.boundary.len() == 3).then(|| ExactTriangularPrismInput {
            mid_surface: [
                face.boundary[0].1.clone(),
                face.boundary[1].1.clone(),
                face.boundary[2].1.clone(),
            ],
            material_normal: ExactVector3 {
                coordinates: [
                    face.transform.rotation[0][1].clone(),
                    face.transform.rotation[1][1].clone(),
                    face.transform.rotation[2][1].clone(),
                ],
            },
            half_thickness: half_thickness.clone(),
        })
    };
    let (Some(first_prism), Some(second_prism)) =
        (prism(&exact.faces[first]), prism(&exact.faces[second]))
    else {
        return Err(PositiveThicknessPrismScanErrorV1::InconsistentPose);
    };
    let mut shared_vertices = [None, None, None];
    let mut shared_vertex_count = 0;
    for entry in &exact.faces[first].boundary {
        if exact.faces[second]
            .boundary
            .iter()
            .any(|(other, _)| *other == entry.0)
        {
            shared_vertices[shared_vertex_count] = Some(entry);
            shared_vertex_count += 1;
        }
    }
    let (first_lower, first_upper) = exact_triangular_prism_bounds_v1(&first_prism);
    let (second_lower, second_upper) = exact_triangular_prism_bounds_v1(&second_prism);
    if (0..3).any(|axis| {
        first_lower[axis] > second_upper[axis] || second_lower[axis] > first_upper[axis]
    }) {
        return Ok(PositiveThicknessPrismPairTaskV1 {
            diagnostic: PositiveThicknessPrismPairDiagnosticV1 {
                first_face: exact.faces[first].face,
                second_face: exact.faces[second].face,
                disposition: PositiveThicknessPrismPairDispositionV1::Separated,
            },
            work: ExactPrismWork::default(),
        });
    }
    let ExactPrismAnalysis { intersection, work } =
        analyze_exact_prism_pair_v1(&first_prism, &second_prism, limits)
            .map_err(|_| PositiveThicknessPrismScanErrorV1::ResourceLimitExceeded)?;
    let intersection = intersection.ok_or(PositiveThicknessPrismScanErrorV1::InconsistentPose)?;
    let shared_vertex_corridor = (shared_vertex_count == 1).then(|| {
        let center = &shared_vertices[0].expect("one counted shared vertex").1;
        let radius = half_thickness
            * BigRational::from_integer(SHARED_FEATURE_CORRIDOR_HALF_EXTENT_MULTIPLIER_V1.into());
        intersection.canonical_vertices().iter().all(|point| {
            point
                .coordinates
                .iter()
                .zip(&center.coordinates)
                .all(|(coordinate, origin)| (coordinate - origin).abs() <= radius)
        })
    }) == Some(true);
    let shared_vertex_ids = (shared_vertex_count == 2).then(|| {
        [
            shared_vertices[0].expect("two counted shared vertices").0,
            shared_vertices[1].expect("two counted shared vertices").0,
        ]
    });
    let has_bound_shared_hinge = shared_vertex_ids.is_some_and(|vertices| {
        exact.hinges.iter().any(|hinge| {
            exact_hinge_binds_face_pair_vertices_v1(
                hinge,
                exact.faces[first].face,
                exact.faces[second].face,
                vertices,
            )
        })
    });
    let shared_hinge_corridor = has_bound_shared_hinge.then(|| {
        let first_shared = shared_vertices[0].expect("two counted shared vertices");
        let second_shared = shared_vertices[1].expect("two counted shared vertices");
        let radius = half_thickness
            * BigRational::from_integer(SHARED_FEATURE_CORRIDOR_HALF_EXTENT_MULTIPLIER_V1.into());
        intersection.canonical_vertices().iter().all(|point| {
            (0..3).all(|axis| {
                let first = &first_shared.1.coordinates[axis];
                let second = &second_shared.1.coordinates[axis];
                let lower = first.min(second) - &radius;
                let upper = first.max(second) + &radius;
                point.coordinates[axis] >= lower && point.coordinates[axis] <= upper
            })
        })
    }) == Some(true);
    let disposition = classify_exact_prism_pair_disposition_v1(
        intersection.kind(),
        shared_hinge_corridor,
        shared_vertex_corridor,
    );
    Ok(PositiveThicknessPrismPairTaskV1 {
        diagnostic: PositiveThicknessPrismPairDiagnosticV1 {
            first_face: exact.faces[first].face,
            second_face: exact.faces[second].face,
            disposition,
        },
        work,
    })
}

fn exact_triangular_prism_bounds_v1(
    input: &ExactTriangularPrismInput,
) -> ([BigRational; 3], [BigRational; 3]) {
    let mut lower: [Option<BigRational>; 3] = [None, None, None];
    let mut upper: [Option<BigRational>; 3] = [None, None, None];
    for point in &input.mid_surface {
        for sign in [-1_i8, 1_i8] {
            for axis in 0..3 {
                let offset = &input.material_normal.coordinates[axis] * &input.half_thickness;
                let value = if sign < 0 {
                    &point.coordinates[axis] - offset
                } else {
                    &point.coordinates[axis] + offset
                };
                lower[axis] = Some(
                    lower[axis]
                        .as_ref()
                        .map_or_else(|| value.clone(), |current| current.min(&value).clone()),
                );
                upper[axis] = Some(
                    upper[axis]
                        .as_ref()
                        .map_or_else(|| value.clone(), |current| current.max(&value).clone()),
                );
            }
        }
    }
    (
        lower.map(|value| value.expect("a triangular prism has vertices")),
        upper.map(|value| value.expect("a triangular prism has vertices")),
    )
}

fn reserve_positive_thickness_prism_parallel_envelope_v1(
    pair_count: usize,
) -> Result<PositiveThicknessPrismParallelEnvelopeV1, PositiveThicknessPrismScanErrorV1> {
    // StaticCollisionLimits preflights the canonical pair count before this
    // phase, but it has no counters corresponding to ExactPrismWork. The
    // established sequential contract instead grants one non-expandable
    // ExactPrismLimits envelope per admitted pair. Preserve that contract
    // exactly: checked-reserve the aggregate of those pair-local envelopes
    // before pool creation, then merge and revalidate every observed delta.
    // Charging these counters to merely similar static rational counters
    // would be dimensionally false and would shrink the old acceptance set.
    let limits = ExactPrismLimits::default().projected();
    let exact_limits = scaled_exact_prism_cayley_limits_v1(limits.exact, pair_count)?;
    Ok(PositiveThicknessPrismParallelEnvelopeV1 {
        exact_prism: ExactPrismWork {
            prisms: parallel_work_product_v1(limits.max_prisms, pair_count, "prisms")?,
            solid_vertices: parallel_work_product_v1(
                limits.max_solid_vertices,
                pair_count,
                "solid_vertices",
            )?,
            facets: parallel_work_product_v1(limits.max_facets, pair_count, "facets")?,
            halfspaces: parallel_work_product_v1(limits.max_halfspaces, pair_count, "halfspaces")?,
            prism_volume_tests: parallel_work_product_v1(
                limits.max_prism_volume_tests,
                pair_count,
                "prism_volume_tests",
            )?,
            facet_vertex_checks: parallel_work_product_v1(
                limits.max_facet_vertex_checks,
                pair_count,
                "facet_vertex_checks",
            )?,
            plane_triples: parallel_work_product_v1(
                limits.max_plane_triples,
                pair_count,
                "plane_triples",
            )?,
            singular_plane_triples: parallel_work_product_v1(
                limits.max_singular_plane_triples,
                pair_count,
                "singular_plane_triples",
            )?,
            nonsingular_solves: parallel_work_product_v1(
                limits.max_nonsingular_solves,
                pair_count,
                "nonsingular_solves",
            )?,
            membership_tests: parallel_work_product_v1(
                limits.max_membership_tests,
                pair_count,
                "membership_tests",
            )?,
            candidate_vertices: parallel_work_product_v1(
                limits.max_candidate_vertices,
                pair_count,
                "candidate_vertices",
            )?,
            dedup_comparisons: parallel_work_product_v1(
                limits.max_dedup_comparisons,
                pair_count,
                "dedup_comparisons",
            )?,
            affine_rank_tests: parallel_work_product_v1(
                limits.max_affine_rank_tests,
                pair_count,
                "affine_rank_tests",
            )?,
            support_plane_vertex_tests: parallel_work_product_v1(
                limits.max_support_plane_vertex_tests,
                pair_count,
                "support_plane_vertex_tests",
            )?,
            support_pair_tests: parallel_work_product_v1(
                limits.max_support_pair_tests,
                pair_count,
                "support_pair_tests",
            )?,
            input_rationals: parallel_work_product_v1(
                limits.max_input_rationals,
                pair_count,
                "input_rationals",
            )?,
            max_input_rational_storage_bits: limits.max_input_rational_storage_bits,
            total_input_storage_bits: parallel_work_product_v1(
                limits.max_total_input_storage_bits,
                pair_count,
                "total_input_storage_bits",
            )?,
            exact: CayleyWork::default(),
        },
        exact_limits,
    })
}

fn scaled_exact_prism_cayley_limits_v1(
    limits: CayleyLimits,
    pair_count: usize,
) -> Result<CayleyLimits, PositiveThicknessPrismScanErrorV1> {
    Ok(CayleyLimits {
        max_precision_rounds: limits.max_precision_rounds,
        max_guard_bits: limits.max_guard_bits,
        max_candidate_bits: limits.max_candidate_bits,
        max_machin_terms_per_series: limits.max_machin_terms_per_series,
        max_trig_terms_per_series: limits.max_trig_terms_per_series,
        max_sqrt_refinements: limits.max_sqrt_refinements,
        max_interval_operations: parallel_work_product_v1(
            limits.max_interval_operations,
            pair_count,
            "interval_operations",
        )?,
        max_shift_bits: limits.max_shift_bits,
        max_intermediate_bits: limits.max_intermediate_bits,
        max_gcd_fallback_calls: parallel_work_product_v1(
            limits.max_gcd_fallback_calls,
            pair_count,
            "gcd_fallback_calls",
        )?,
        max_gcd_fallback_input_bits: parallel_work_product_v1(
            limits.max_gcd_fallback_input_bits,
            pair_count,
            "gcd_fallback_input_bits",
        )?,
        max_rational_allocations: parallel_work_product_v1(
            limits.max_rational_allocations,
            pair_count,
            "rational_allocations",
        )?,
        max_rational_allocation_bits: limits.max_rational_allocation_bits,
        max_total_rational_allocation_bits: parallel_work_product_v1(
            limits.max_total_rational_allocation_bits,
            pair_count,
            "total_rational_allocation_bits",
        )?,
        max_output_bits: limits.max_output_bits,
    })
}

fn checked_merge_exact_prism_parallel_work_v1(
    accumulated: &ExactPrismWork,
    additional: &ExactPrismWork,
    envelope: &PositiveThicknessPrismParallelEnvelopeV1,
) -> Result<ExactPrismWork, PositiveThicknessPrismScanErrorV1> {
    let merged = ExactPrismWork {
        prisms: parallel_work_sum_v1(accumulated.prisms, additional.prisms, "prisms")?,
        solid_vertices: parallel_work_sum_v1(
            accumulated.solid_vertices,
            additional.solid_vertices,
            "solid_vertices",
        )?,
        facets: parallel_work_sum_v1(accumulated.facets, additional.facets, "facets")?,
        halfspaces: parallel_work_sum_v1(
            accumulated.halfspaces,
            additional.halfspaces,
            "halfspaces",
        )?,
        prism_volume_tests: parallel_work_sum_v1(
            accumulated.prism_volume_tests,
            additional.prism_volume_tests,
            "prism_volume_tests",
        )?,
        facet_vertex_checks: parallel_work_sum_v1(
            accumulated.facet_vertex_checks,
            additional.facet_vertex_checks,
            "facet_vertex_checks",
        )?,
        plane_triples: parallel_work_sum_v1(
            accumulated.plane_triples,
            additional.plane_triples,
            "plane_triples",
        )?,
        singular_plane_triples: parallel_work_sum_v1(
            accumulated.singular_plane_triples,
            additional.singular_plane_triples,
            "singular_plane_triples",
        )?,
        nonsingular_solves: parallel_work_sum_v1(
            accumulated.nonsingular_solves,
            additional.nonsingular_solves,
            "nonsingular_solves",
        )?,
        membership_tests: parallel_work_sum_v1(
            accumulated.membership_tests,
            additional.membership_tests,
            "membership_tests",
        )?,
        candidate_vertices: parallel_work_sum_v1(
            accumulated.candidate_vertices,
            additional.candidate_vertices,
            "candidate_vertices",
        )?,
        dedup_comparisons: parallel_work_sum_v1(
            accumulated.dedup_comparisons,
            additional.dedup_comparisons,
            "dedup_comparisons",
        )?,
        affine_rank_tests: parallel_work_sum_v1(
            accumulated.affine_rank_tests,
            additional.affine_rank_tests,
            "affine_rank_tests",
        )?,
        support_plane_vertex_tests: parallel_work_sum_v1(
            accumulated.support_plane_vertex_tests,
            additional.support_plane_vertex_tests,
            "support_plane_vertex_tests",
        )?,
        support_pair_tests: parallel_work_sum_v1(
            accumulated.support_pair_tests,
            additional.support_pair_tests,
            "support_pair_tests",
        )?,
        input_rationals: parallel_work_sum_v1(
            accumulated.input_rationals,
            additional.input_rationals,
            "input_rationals",
        )?,
        max_input_rational_storage_bits: accumulated
            .max_input_rational_storage_bits
            .max(additional.max_input_rational_storage_bits),
        total_input_storage_bits: parallel_work_sum_v1(
            accumulated.total_input_storage_bits,
            additional.total_input_storage_bits,
            "total_input_storage_bits",
        )?,
        exact: accumulated
            .exact
            .checked_merge(
                &additional.exact,
                &envelope.exact_limits,
                None,
                CayleyStage::Containment,
            )
            .map_err(|_| PositiveThicknessPrismScanErrorV1::ResourceLimitExceeded)?,
    };
    for (actual, maximum) in [
        (merged.prisms, envelope.exact_prism.prisms),
        (merged.solid_vertices, envelope.exact_prism.solid_vertices),
        (merged.facets, envelope.exact_prism.facets),
        (merged.halfspaces, envelope.exact_prism.halfspaces),
        (
            merged.prism_volume_tests,
            envelope.exact_prism.prism_volume_tests,
        ),
        (
            merged.facet_vertex_checks,
            envelope.exact_prism.facet_vertex_checks,
        ),
        (merged.plane_triples, envelope.exact_prism.plane_triples),
        (
            merged.singular_plane_triples,
            envelope.exact_prism.singular_plane_triples,
        ),
        (
            merged.nonsingular_solves,
            envelope.exact_prism.nonsingular_solves,
        ),
        (
            merged.membership_tests,
            envelope.exact_prism.membership_tests,
        ),
        (
            merged.candidate_vertices,
            envelope.exact_prism.candidate_vertices,
        ),
        (
            merged.dedup_comparisons,
            envelope.exact_prism.dedup_comparisons,
        ),
        (
            merged.affine_rank_tests,
            envelope.exact_prism.affine_rank_tests,
        ),
        (
            merged.support_plane_vertex_tests,
            envelope.exact_prism.support_plane_vertex_tests,
        ),
        (
            merged.support_pair_tests,
            envelope.exact_prism.support_pair_tests,
        ),
        (merged.input_rationals, envelope.exact_prism.input_rationals),
        (
            merged.max_input_rational_storage_bits,
            envelope.exact_prism.max_input_rational_storage_bits,
        ),
        (
            merged.total_input_storage_bits,
            envelope.exact_prism.total_input_storage_bits,
        ),
    ] {
        if actual > maximum {
            return Err(PositiveThicknessPrismScanErrorV1::ResourceLimitExceeded);
        }
    }
    Ok(merged)
}

fn parallel_work_product_v1(
    value: usize,
    count: usize,
    resource: &'static str,
) -> Result<usize, PositiveThicknessPrismScanErrorV1> {
    checked_work_product(value, count, CayleyStage::Containment, resource)
        .map_err(|_| PositiveThicknessPrismScanErrorV1::ResourceLimitExceeded)
}

fn parallel_work_sum_v1(
    left: usize,
    right: usize,
    resource: &'static str,
) -> Result<usize, PositiveThicknessPrismScanErrorV1> {
    checked_work_sum(left, right, CayleyStage::Containment, resource)
        .map_err(|_| PositiveThicknessPrismScanErrorV1::ResourceLimitExceeded)
}
