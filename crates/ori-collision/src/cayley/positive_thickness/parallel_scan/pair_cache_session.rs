use std::{cmp::Ordering, collections::HashSet};

use num_traits::{Signed, Zero};

use super::*;
use crate::proof_cache::{
    ExactFacePoseCacheWitnessV1, ExactFacePoseComponentsV1, FaceDependencyFootprintV1,
    ProofCacheErrorV1, ProofCachePairWorkLimitsV1, ProofCachePairWorkV1,
};

const EXACT_ENDPOINT_FIXED_AXIS_COUNT_V2: usize = 7;
const EXACT_ENDPOINT_LAZY_FIXED_AXIS_COUNT_V2: usize = EXACT_ENDPOINT_FIXED_AXIS_COUNT_V2 - 1;

#[derive(Debug)]
struct ExactEndpointAxisIntervalV2 {
    lower: BigRational,
    upper: BigRational,
}

#[derive(Debug)]
struct ExactEndpointFaceBoundsV2 {
    face: FaceId,
    exact_face_index: usize,
    x_axis_interval: ExactEndpointAxisIntervalV2,
    lazy_fixed_axis_intervals:
        [Option<ExactEndpointAxisIntervalV2>; EXACT_ENDPOINT_LAZY_FIXED_AXIS_COUNT_V2],
}

/// One exact pose preparation shared by pair-local cache misses.
///
/// This layer deliberately has no certificate-model, issuer-context, cache
/// key, or publication authority. It only exposes the exact pair observation
/// and the complete pair-local geometry dependencies; the continuous theorem
/// that consumes it remains the sole authority that may decide whether an
/// observation is cacheable.
pub(crate) struct PositiveThicknessExactPairCacheSessionV1<'a> {
    endpoint: PositiveThicknessExactEndpointSessionV2<'a>,
    faces: Vec<PositiveThicknessExactFaceCacheSnapshotV1>,
}

/// One exact endpoint pose shared by conservative broad-phase observations.
///
/// Unlike [`PositiveThicknessExactPairCacheSessionV1`], preparing this session
/// does not encode cache witnesses or dependency footprints.
pub(crate) struct PositiveThicknessExactEndpointSessionV2<'a> {
    exact: RationalCayleyTreePose<'a>,
    half_thickness: BigRational,
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
    let endpoint = prepare_positive_thickness_exact_endpoint_session_v2(bound, paper_thickness_mm)?;
    let faces = prepare_positive_thickness_exact_face_cache_snapshots_v1(&endpoint.exact)?;
    Ok(PositiveThicknessExactPairCacheSessionV1 { endpoint, faces })
}

pub(crate) fn prepare_positive_thickness_exact_endpoint_session_v2(
    bound: BoundMaterialTreePose<'_>,
    paper_thickness_mm: f64,
) -> Result<PositiveThicknessExactEndpointSessionV2<'_>, PositiveThicknessPrismScanErrorV1> {
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
    if !half_thickness.is_positive() {
        return Err(PositiveThicknessPrismScanErrorV1::InconsistentPose);
    }
    Ok(PositiveThicknessExactEndpointSessionV2 {
        exact,
        half_thickness,
    })
}

fn prepare_positive_thickness_exact_face_cache_snapshots_v1(
    exact: &RationalCayleyTreePose<'_>,
) -> Result<Vec<PositiveThicknessExactFaceCacheSnapshotV1>, PositiveThicknessPrismScanErrorV1> {
    let mut faces = Vec::new();
    faces
        .try_reserve_exact(exact.faces.len())
        .map_err(|_| PositiveThicknessPrismScanErrorV1::ResourceLimitExceeded)?;
    for face in &exact.faces {
        let boundary = exact
            .bound
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
    Ok(faces)
}

impl PositiveThicknessExactEndpointSessionV2<'_> {
    /// Returns a conservative broad-phase observation over the same exact E
    /// solids consumed by
    /// [`PositiveThicknessExactPairCacheSessionV1::analyze_pair_v1`].
    ///
    /// This method issues no certificate or cache authority. Every projection
    /// is computed directly from the authenticated exact boundary. Fixed axes
    /// use the face-specific exact normal projection of the half-thickness;
    /// planar edge axes use a bounded rational L1 enclosure.
    pub(crate) fn exact_endpoint_candidates_v2(
        &self,
        max_candidates: usize,
    ) -> Result<Vec<(FaceId, FaceId)>, PositiveThicknessPrismScanErrorV1> {
        if self.exact.faces.is_empty()
            || !self.half_thickness.is_positive()
            || self
                .exact
                .faces
                .windows(2)
                .any(|faces| faces[0].face.canonical_bytes() >= faces[1].face.canonical_bytes())
            || self.exact.faces.iter().any(|face| face.boundary.is_empty())
        {
            return Err(PositiveThicknessPrismScanErrorV1::InconsistentPose);
        }

        let face_count = self.exact.faces.len();
        let unordered_face_pairs = face_count
            .checked_mul(face_count.saturating_sub(1))
            .and_then(|value| value.checked_div(2))
            .ok_or(PositiveThicknessPrismScanErrorV1::ResourceLimitExceeded)?;
        let exact_limits = ExactTreePoseLimits::default().cayley;
        let mut meter = WorkMeter::new(&exact_limits);
        let fixed_axes = exact_endpoint_fixed_axes_v2();
        let mut bounds = Vec::new();
        bounds
            .try_reserve_exact(face_count)
            .map_err(|_| PositiveThicknessPrismScanErrorV1::ResourceLimitExceeded)?;
        for (exact_face_index, face) in self.exact.faces.iter().enumerate() {
            let x_axis_radius = exact_endpoint_face_thickness_radius_v2(
                face,
                &self.half_thickness,
                &fixed_axes[0],
                &mut meter,
            )?;
            let x_axis_interval = exact_endpoint_solid_projection_with_radius_v2(
                face,
                &fixed_axes[0],
                &x_axis_radius,
                &mut meter,
            )?
            .ok_or(PositiveThicknessPrismScanErrorV1::InconsistentPose)?;
            bounds.push(ExactEndpointFaceBoundsV2 {
                face: face.face,
                exact_face_index,
                x_axis_interval,
                lazy_fixed_axis_intervals: std::array::from_fn(|_| None),
            });
        }
        exact_endpoint_sort_face_bounds_v2(&mut bounds, &mut meter)?;

        let mut adjacent_pairs = HashSet::new();
        adjacent_pairs
            .try_reserve(self.exact.bound.model().hinges().len())
            .map_err(|_| PositiveThicknessPrismScanErrorV1::ResourceLimitExceeded)?;
        for hinge in self.exact.bound.model().hinges() {
            let pair =
                exact_endpoint_canonical_face_pair_v2(hinge.left_face(), hinge.right_face())?;
            if !adjacent_pairs.insert(pair) {
                return Err(PositiveThicknessPrismScanErrorV1::InconsistentPose);
            }
        }

        let mut candidates = Vec::new();
        candidates
            .try_reserve_exact(unordered_face_pairs.min(max_candidates))
            .map_err(|_| PositiveThicknessPrismScanErrorV1::ResourceLimitExceeded)?;
        for first in 0..bounds.len() {
            for second in first + 1..bounds.len() {
                meter
                    .operation(CayleyStage::Containment)
                    .map_err(map_exact_pair_cache_cayley_error_v1)?;
                if meter
                    .compare_rational(
                        &bounds[second].x_axis_interval.lower,
                        &bounds[first].x_axis_interval.upper,
                        CayleyStage::Containment,
                    )
                    .map_err(map_exact_pair_cache_cayley_error_v1)?
                    == Ordering::Greater
                {
                    break;
                }

                let pair =
                    exact_endpoint_canonical_face_pair_v2(bounds[first].face, bounds[second].face)?;
                if adjacent_pairs.contains(&pair) {
                    continue;
                }
                let first_exact_face_index = bounds[first].exact_face_index;
                let second_exact_face_index = bounds[second].exact_face_index;
                let first_exact_face = &self.exact.faces[first_exact_face_index];
                let second_exact_face = &self.exact.faces[second_exact_face_index];
                if exact_endpoint_retain_shared_vertex_pair_v2(
                    first_exact_face,
                    second_exact_face,
                    pair,
                    &mut candidates,
                    max_candidates,
                    &mut meter,
                )? {
                    continue;
                }
                if exact_endpoint_faces_separated_by_lazy_fixed_axes_v2(
                    &mut bounds,
                    first,
                    second,
                    first_exact_face,
                    second_exact_face,
                    &fixed_axes,
                    &self.half_thickness,
                    &mut meter,
                )? || exact_endpoint_faces_separated_by_planar_edge_v2(
                    first_exact_face,
                    second_exact_face,
                    &self.half_thickness,
                    &mut meter,
                )? {
                    continue;
                }

                exact_endpoint_push_candidate_v2(&mut candidates, pair, max_candidates)?;
            }
        }
        candidates
            .sort_unstable_by_key(|pair| (pair.0.canonical_bytes(), pair.1.canonical_bytes()));
        candidates.dedup();
        Ok(candidates)
    }

    pub(crate) fn exact_pair_strictly_separated_v2(
        &self,
        first: FaceId,
        second: FaceId,
    ) -> Result<bool, PositiveThicknessPrismScanErrorV1> {
        let (task, _, _) = self.analyze_pair_task_v2(first, second)?;
        Ok(task.diagnostic.disposition == PositiveThicknessPrismPairDispositionV1::Separated)
    }

    fn analyze_pair_task_v2(
        &self,
        first: FaceId,
        second: FaceId,
    ) -> Result<(PositiveThicknessPrismPairTaskV1, usize, usize), PositiveThicknessPrismScanErrorV1>
    {
        if first == second {
            return Err(PositiveThicknessPrismScanErrorV1::InconsistentPose);
        }
        let first = self
            .face_index_v2(first)
            .ok_or(PositiveThicknessPrismScanErrorV1::InconsistentPose)?;
        let second = self
            .face_index_v2(second)
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
        Ok((task, first, second))
    }

    fn face_index_v2(&self, face: FaceId) -> Option<usize> {
        self.exact
            .faces
            .binary_search_by_key(&face.canonical_bytes(), |item| item.face.canonical_bytes())
            .ok()
    }
}

impl PositiveThicknessExactPairCacheSessionV1<'_> {
    pub(crate) fn analyze_pair_v1(
        &self,
        first: FaceId,
        second: FaceId,
    ) -> Result<PositiveThicknessExactPairCacheObservationV1, PositiveThicknessPrismScanErrorV1>
    {
        let (task, first, second) = self.endpoint.analyze_pair_task_v2(first, second)?;
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
}

fn exact_endpoint_fixed_axes_v2() -> [[BigRational; 3]; EXACT_ENDPOINT_FIXED_AXIS_COUNT_V2] {
    let exact = |value: i64| BigRational::from_integer(value.into());
    [
        [exact(1), exact(0), exact(0)],
        [exact(0), exact(1), exact(0)],
        [exact(0), exact(0), exact(1)],
        [exact(1), exact(1), exact(0)],
        [exact(1), exact(-1), exact(0)],
        [exact(1), exact(25), exact(0)],
        [exact(1), exact(-25), exact(0)],
    ]
}

fn exact_endpoint_sort_face_bounds_v2(
    bounds: &mut [ExactEndpointFaceBoundsV2],
    meter: &mut WorkMeter<'_>,
) -> Result<(), PositiveThicknessPrismScanErrorV1> {
    for current in 1..bounds.len() {
        let mut lower = 0;
        let mut upper = current;
        while lower < upper {
            let middle = lower + (upper - lower) / 2;
            if exact_endpoint_compare_face_bounds_v2(&bounds[current], &bounds[middle], meter)?
                == Ordering::Less
            {
                upper = middle;
            } else {
                lower = middle + 1;
            }
        }
        if lower < current {
            // Precharge every slot move before mutating the slice. This keeps
            // the sort fail-atomic and bounds the physical rotation work, not
            // only the comparisons used to choose its insertion point.
            for _ in lower..current {
                meter
                    .operation(CayleyStage::Containment)
                    .map_err(map_exact_pair_cache_cayley_error_v1)?;
            }
            bounds[lower..=current].rotate_right(1);
        }
    }
    Ok(())
}

fn exact_endpoint_compare_face_bounds_v2(
    first: &ExactEndpointFaceBoundsV2,
    second: &ExactEndpointFaceBoundsV2,
    meter: &mut WorkMeter<'_>,
) -> Result<Ordering, PositiveThicknessPrismScanErrorV1> {
    Ok(meter
        .compare_rational(
            &first.x_axis_interval.lower,
            &second.x_axis_interval.lower,
            CayleyStage::Containment,
        )
        .map_err(map_exact_pair_cache_cayley_error_v1)?
        .then_with(|| {
            first
                .face
                .canonical_bytes()
                .cmp(&second.face.canonical_bytes())
        }))
}

fn exact_endpoint_canonical_face_pair_v2(
    first: FaceId,
    second: FaceId,
) -> Result<[FaceId; 2], PositiveThicknessPrismScanErrorV1> {
    if first == second {
        return Err(PositiveThicknessPrismScanErrorV1::InconsistentPose);
    }
    Ok(if first.canonical_bytes() < second.canonical_bytes() {
        [first, second]
    } else {
        [second, first]
    })
}

fn exact_endpoint_push_candidate_v2(
    candidates: &mut Vec<(FaceId, FaceId)>,
    pair: [FaceId; 2],
    max_candidates: usize,
) -> Result<(), PositiveThicknessPrismScanErrorV1> {
    if pair[0].canonical_bytes() >= pair[1].canonical_bytes() {
        return Err(PositiveThicknessPrismScanErrorV1::InconsistentPose);
    }
    let next = candidates
        .len()
        .checked_add(1)
        .ok_or(PositiveThicknessPrismScanErrorV1::ResourceLimitExceeded)?;
    if next > max_candidates {
        return Err(PositiveThicknessPrismScanErrorV1::ResourceLimitExceeded);
    }
    candidates.push((pair[0], pair[1]));
    Ok(())
}

fn exact_endpoint_retain_shared_vertex_pair_v2(
    first: &ExactFacePose,
    second: &ExactFacePose,
    pair: [FaceId; 2],
    candidates: &mut Vec<(FaceId, FaceId)>,
    max_candidates: usize,
    meter: &mut WorkMeter<'_>,
) -> Result<bool, PositiveThicknessPrismScanErrorV1> {
    if first.face == second.face
        || pair[0].canonical_bytes() >= pair[1].canonical_bytes()
        || !pair.contains(&first.face)
        || !pair.contains(&second.face)
    {
        return Err(PositiveThicknessPrismScanErrorV1::InconsistentPose);
    }
    let mut shared_vertex = false;
    'first_boundary: for (vertex, _) in &first.boundary {
        for (candidate, _) in &second.boundary {
            meter
                .operation(CayleyStage::Containment)
                .map_err(map_exact_pair_cache_cayley_error_v1)?;
            if candidate == vertex {
                shared_vertex = true;
                break 'first_boundary;
            }
        }
    }
    if !shared_vertex {
        return Ok(false);
    }

    // The exact tree-pose registry has already required every occurrence of
    // this material vertex to have one canonical E point. The two positive
    // solids therefore touch at that point and may never be broad-phase
    // excluded. Retain the pair before any remaining projection work.
    exact_endpoint_push_candidate_v2(candidates, pair, max_candidates)?;
    Ok(true)
}

#[allow(clippy::too_many_arguments)]
fn exact_endpoint_faces_separated_by_lazy_fixed_axes_v2(
    bounds: &mut [ExactEndpointFaceBoundsV2],
    first_index: usize,
    second_index: usize,
    first_face: &ExactFacePose,
    second_face: &ExactFacePose,
    fixed_axes: &[[BigRational; 3]; EXACT_ENDPOINT_FIXED_AXIS_COUNT_V2],
    half_thickness: &BigRational,
    meter: &mut WorkMeter<'_>,
) -> Result<bool, PositiveThicknessPrismScanErrorV1> {
    if first_index >= second_index || second_index >= bounds.len() {
        return Err(PositiveThicknessPrismScanErrorV1::InconsistentPose);
    }
    let (before_second, second_and_after) = bounds.split_at_mut(second_index);
    let first_bound = &mut before_second[first_index];
    let second_bound = &mut second_and_after[0];
    if first_bound.face != first_face.face
        || second_bound.face != second_face.face
        || first_bound.exact_face_index == second_bound.exact_face_index
    {
        return Err(PositiveThicknessPrismScanErrorV1::InconsistentPose);
    }

    for (fixed_axis_index, fixed_axis) in fixed_axes.iter().enumerate().skip(1) {
        let lazy_axis_index = fixed_axis_index - 1;
        exact_endpoint_prepare_lazy_fixed_axis_interval_v2(
            first_bound,
            first_face,
            lazy_axis_index,
            fixed_axis,
            half_thickness,
            meter,
        )?;
        exact_endpoint_prepare_lazy_fixed_axis_interval_v2(
            second_bound,
            second_face,
            lazy_axis_index,
            fixed_axis,
            half_thickness,
            meter,
        )?;
        let first_interval = first_bound.lazy_fixed_axis_intervals[lazy_axis_index]
            .as_ref()
            .ok_or(PositiveThicknessPrismScanErrorV1::InconsistentPose)?;
        let second_interval = second_bound.lazy_fixed_axis_intervals[lazy_axis_index]
            .as_ref()
            .ok_or(PositiveThicknessPrismScanErrorV1::InconsistentPose)?;
        if exact_endpoint_intervals_strictly_separated_v2(first_interval, second_interval, meter)? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn exact_endpoint_prepare_lazy_fixed_axis_interval_v2(
    bound: &mut ExactEndpointFaceBoundsV2,
    face: &ExactFacePose,
    lazy_axis_index: usize,
    axis: &[BigRational; 3],
    half_thickness: &BigRational,
    meter: &mut WorkMeter<'_>,
) -> Result<(), PositiveThicknessPrismScanErrorV1> {
    if bound.face != face.face {
        return Err(PositiveThicknessPrismScanErrorV1::InconsistentPose);
    }
    let interval = bound
        .lazy_fixed_axis_intervals
        .get_mut(lazy_axis_index)
        .ok_or(PositiveThicknessPrismScanErrorV1::InconsistentPose)?;
    if interval.is_none() {
        let radius = exact_endpoint_face_thickness_radius_v2(face, half_thickness, axis, meter)?;
        *interval = Some(
            exact_endpoint_solid_projection_with_radius_v2(face, axis, &radius, meter)?
                .ok_or(PositiveThicknessPrismScanErrorV1::InconsistentPose)?,
        );
    }
    Ok(())
}

fn exact_endpoint_dot_v2(
    first: &[BigRational; 3],
    second: &[BigRational; 3],
    meter: &mut WorkMeter<'_>,
) -> Result<BigRational, PositiveThicknessPrismScanErrorV1> {
    let mut result = BigRational::zero();
    for axis in 0..3 {
        let product = meter
            .multiply_rational(&first[axis], &second[axis], CayleyStage::Containment)
            .map_err(map_exact_pair_cache_cayley_error_v1)?;
        result = meter
            .add_rational(&result, &product, CayleyStage::Containment)
            .map_err(map_exact_pair_cache_cayley_error_v1)?;
    }
    Ok(result)
}

fn exact_endpoint_solid_projection_with_radius_v2(
    face: &ExactFacePose,
    axis: &[BigRational; 3],
    radius: &BigRational,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<ExactEndpointAxisIntervalV2>, PositiveThicknessPrismScanErrorV1> {
    if axis.iter().all(Zero::is_zero) {
        return Ok(None);
    }
    if face.boundary.is_empty() || radius.is_negative() {
        return Err(PositiveThicknessPrismScanErrorV1::InconsistentPose);
    }

    let mut lower = None::<BigRational>;
    let mut upper = None::<BigRational>;
    for (_, point) in &face.boundary {
        let projection = exact_endpoint_dot_v2(&point.coordinates, axis, meter)?;
        let candidate_lower = meter
            .subtract_rational(&projection, radius, CayleyStage::Containment)
            .map_err(map_exact_pair_cache_cayley_error_v1)?;
        let candidate_upper = meter
            .add_rational(&projection, radius, CayleyStage::Containment)
            .map_err(map_exact_pair_cache_cayley_error_v1)?;
        let replace_lower = match lower.as_ref() {
            Some(current) => {
                meter
                    .compare_rational(&candidate_lower, current, CayleyStage::Containment)
                    .map_err(map_exact_pair_cache_cayley_error_v1)?
                    == Ordering::Less
            }
            None => true,
        };
        if replace_lower {
            lower = Some(candidate_lower);
        }
        let replace_upper = match upper.as_ref() {
            Some(current) => {
                meter
                    .compare_rational(&candidate_upper, current, CayleyStage::Containment)
                    .map_err(map_exact_pair_cache_cayley_error_v1)?
                    == Ordering::Greater
            }
            None => true,
        };
        if replace_upper {
            upper = Some(candidate_upper);
        }
    }
    Ok(lower
        .zip(upper)
        .map(|(lower, upper)| ExactEndpointAxisIntervalV2 { lower, upper }))
}

fn exact_endpoint_face_thickness_radius_v2(
    face: &ExactFacePose,
    half_thickness: &BigRational,
    axis: &[BigRational; 3],
    meter: &mut WorkMeter<'_>,
) -> Result<BigRational, PositiveThicknessPrismScanErrorV1> {
    if !half_thickness.is_positive() || axis.iter().all(Zero::is_zero) {
        return Err(PositiveThicknessPrismScanErrorV1::InconsistentPose);
    }

    // The exact pair kernel constructs the E solid from every mid-surface
    // boundary point `p` and the two offsets `p ± h*n_face`. Projecting that
    // same solid onto `axis` therefore expands the mid-surface interval by the
    // exact face-specific radius `h * |axis·n_face|`.
    let mut normal_projection = BigRational::zero();
    for (axis_component, rotation_row) in axis.iter().zip(&face.transform.rotation) {
        let product = meter
            .multiply_rational(axis_component, &rotation_row[1], CayleyStage::Containment)
            .map_err(map_exact_pair_cache_cayley_error_v1)?;
        normal_projection = meter
            .add_rational(&normal_projection, &product, CayleyStage::Containment)
            .map_err(map_exact_pair_cache_cayley_error_v1)?;
    }
    let absolute_projection = meter
        .absolute_rational(&normal_projection, CayleyStage::Containment)
        .map_err(map_exact_pair_cache_cayley_error_v1)?;
    meter
        .multiply_rational(
            half_thickness,
            &absolute_projection,
            CayleyStage::Containment,
        )
        .map_err(map_exact_pair_cache_cayley_error_v1)
}

fn exact_endpoint_planar_thickness_enclosure_radius_v2(
    half_thickness: &BigRational,
    axis: &[BigRational; 3],
    meter: &mut WorkMeter<'_>,
) -> Result<BigRational, PositiveThicknessPrismScanErrorV1> {
    if !half_thickness.is_positive() || axis.iter().all(Zero::is_zero) {
        return Err(PositiveThicknessPrismScanErrorV1::InconsistentPose);
    }

    // A planar edge axis is pair-local and changes for every source edge.
    // Use one symmetric rational enclosure for both faces so its arithmetic
    // cost stays bounded independently of their rotations. Since every
    // authenticated material normal has Euclidean norm one,
    // `|axis·n_face| <= ||axis||_2 <= ||axis||_1`; this may retain an extra
    // pair but can never exclude either exact E solid.
    let mut axis_l1_norm = BigRational::zero();
    for coordinate in axis {
        let absolute = meter
            .absolute_rational(coordinate, CayleyStage::Containment)
            .map_err(map_exact_pair_cache_cayley_error_v1)?;
        axis_l1_norm = meter
            .add_rational(&axis_l1_norm, &absolute, CayleyStage::Containment)
            .map_err(map_exact_pair_cache_cayley_error_v1)?;
    }
    meter
        .multiply_rational(half_thickness, &axis_l1_norm, CayleyStage::Containment)
        .map_err(map_exact_pair_cache_cayley_error_v1)
}

fn exact_endpoint_intervals_strictly_separated_v2(
    first: &ExactEndpointAxisIntervalV2,
    second: &ExactEndpointAxisIntervalV2,
    meter: &mut WorkMeter<'_>,
) -> Result<bool, PositiveThicknessPrismScanErrorV1> {
    if meter
        .compare_rational(&first.upper, &second.lower, CayleyStage::Containment)
        .map_err(map_exact_pair_cache_cayley_error_v1)?
        == Ordering::Less
    {
        return Ok(true);
    }
    Ok(meter
        .compare_rational(&second.upper, &first.lower, CayleyStage::Containment)
        .map_err(map_exact_pair_cache_cayley_error_v1)?
        == Ordering::Less)
}

fn exact_endpoint_planar_edge_axis_v2(
    start: &ExactPoint3,
    end: &ExactPoint3,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<[BigRational; 3]>, PositiveThicknessPrismScanErrorV1> {
    let delta_z = meter
        .subtract_rational(
            &end.coordinates[2],
            &start.coordinates[2],
            CayleyStage::Containment,
        )
        .map_err(map_exact_pair_cache_cayley_error_v1)?;
    let axis_x = meter
        .negate_rational(&delta_z, CayleyStage::Containment)
        .map_err(map_exact_pair_cache_cayley_error_v1)?;
    let axis_z = meter
        .subtract_rational(
            &end.coordinates[0],
            &start.coordinates[0],
            CayleyStage::Containment,
        )
        .map_err(map_exact_pair_cache_cayley_error_v1)?;
    let axis = [axis_x, BigRational::zero(), axis_z];
    Ok((!axis.iter().all(Zero::is_zero)).then_some(axis))
}

fn exact_endpoint_faces_separated_by_planar_edge_v2(
    first: &ExactFacePose,
    second: &ExactFacePose,
    half_thickness: &BigRational,
    meter: &mut WorkMeter<'_>,
) -> Result<bool, PositiveThicknessPrismScanErrorV1> {
    if first.boundary.is_empty() || second.boundary.is_empty() {
        return Err(PositiveThicknessPrismScanErrorV1::InconsistentPose);
    }
    for source in [first, second] {
        for index in 0..source.boundary.len() {
            meter
                .operation(CayleyStage::Containment)
                .map_err(map_exact_pair_cache_cayley_error_v1)?;
            let start = &source.boundary[index].1;
            let end = &source.boundary[(index + 1) % source.boundary.len()].1;
            let Some(axis) = exact_endpoint_planar_edge_axis_v2(start, end, meter)? else {
                continue;
            };
            let radius =
                exact_endpoint_planar_thickness_enclosure_radius_v2(half_thickness, &axis, meter)?;
            let first_interval =
                exact_endpoint_solid_projection_with_radius_v2(first, &axis, &radius, meter)?
                    .ok_or(PositiveThicknessPrismScanErrorV1::InconsistentPose)?;
            let second_interval =
                exact_endpoint_solid_projection_with_radius_v2(second, &axis, &radius, meter)?
                    .ok_or(PositiveThicknessPrismScanErrorV1::InconsistentPose)?;
            if exact_endpoint_intervals_strictly_separated_v2(
                &first_interval,
                &second_interval,
                meter,
            )? {
                return Ok(true);
            }
        }
    }
    Ok(false)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cayley::exact_identity_transform;
    use crate::cayley::positive_thickness::exact_prism::ExactPrismIntersectionKind;

    fn exact_integer(value: i64) -> BigRational {
        BigRational::from_integer(value.into())
    }

    fn exact_ratio(numerator: i64, denominator: i64) -> BigRational {
        BigRational::new(numerator.into(), denominator.into())
    }

    fn exact_point(x: i64, y: i64, z: i64) -> ExactPoint3 {
        ExactPoint3 {
            coordinates: [exact_integer(x), exact_integer(y), exact_integer(z)],
        }
    }

    fn exact_face(
        face: FaceId,
        vertices: impl IntoIterator<Item = (VertexId, ExactPoint3)>,
    ) -> ExactFacePose {
        ExactFacePose {
            face,
            transform: exact_identity_transform(),
            boundary: vertices.into_iter().collect(),
        }
    }

    fn exact_bound(face: FaceId, exact_face_index: usize, lower: i64) -> ExactEndpointFaceBoundsV2 {
        ExactEndpointFaceBoundsV2 {
            face,
            exact_face_index,
            x_axis_interval: ExactEndpointAxisIntervalV2 {
                lower: exact_integer(lower),
                upper: exact_integer(lower + 1),
            },
            lazy_fixed_axis_intervals: std::array::from_fn(|_| None),
        }
    }

    #[test]
    fn minimum_subnormal_exact_half_thickness_retains_touching_interval() {
        let exact_thickness =
            BigRational::from_float(f64::from_bits(1)).expect("minimum subnormal is rational");
        let half_thickness = exact_thickness / BigRational::from_integer(2.into());
        assert!(
            half_thickness.is_positive(),
            "exact division must not round the minimum subnormal thickness to zero"
        );

        let exact = |value: i64| BigRational::from_integer(value.into());
        let axis = [exact(0), exact(1), exact(0)];
        let face = exact_face(FaceId::new(), [(VertexId::new(), exact_point(0, 0, 0))]);
        let exact_limits = ExactTreePoseLimits::default().cayley;
        let mut meter = WorkMeter::new(&exact_limits);
        let radius =
            exact_endpoint_face_thickness_radius_v2(&face, &half_thickness, &axis, &mut meter)
                .unwrap();
        assert_eq!(radius, half_thickness);

        let first = ExactEndpointAxisIntervalV2 {
            lower: -radius.clone(),
            upper: radius.clone(),
        };
        let second = ExactEndpointAxisIntervalV2 {
            lower: radius.clone(),
            upper: radius * BigRational::from_integer(3.into()),
        };
        assert!(
            !exact_endpoint_intervals_strictly_separated_v2(&first, &second, &mut meter).unwrap(),
            "interval equality is contact, so the pair must remain a candidate"
        );
    }

    #[test]
    fn face_specific_radius_uses_exact_material_normal_projection_and_is_bounded() {
        let half_thickness = exact_integer(2);
        let axis = [exact_integer(1), exact_integer(25), exact_integer(0)];
        let face = exact_face(FaceId::new(), [(VertexId::new(), exact_point(0, 0, 0))]);
        let exact_limits = ExactTreePoseLimits::default().cayley;
        let mut meter = WorkMeter::new(&exact_limits);

        let radius =
            exact_endpoint_face_thickness_radius_v2(&face, &half_thickness, &axis, &mut meter)
                .unwrap();
        assert_eq!(radius, exact_integer(50));
        assert_eq!(meter.work.interval_operations, 7);

        let mut planar_meter = WorkMeter::new(&exact_limits);
        let planar_enclosure = exact_endpoint_planar_thickness_enclosure_radius_v2(
            &half_thickness,
            &axis,
            &mut planar_meter,
        )
        .unwrap();
        assert_eq!(planar_enclosure, exact_integer(52));
        assert_eq!(planar_meter.work.interval_operations, 4);

        let mut one_short_limits = ExactTreePoseLimits::default().cayley;
        one_short_limits.max_interval_operations = 6;
        let mut one_short_meter = WorkMeter::new(&one_short_limits);
        assert!(matches!(
            exact_endpoint_face_thickness_radius_v2(
                &face,
                &half_thickness,
                &axis,
                &mut one_short_meter,
            ),
            Err(PositiveThicknessPrismScanErrorV1::ResourceLimitExceeded)
        ));
        assert_eq!(one_short_meter.work.interval_operations, 6);
    }

    #[test]
    fn zero_face_radius_projects_the_exact_mid_surface() {
        let face = exact_face(
            FaceId::new(),
            [
                (VertexId::new(), exact_point(-2, 7, 0)),
                (VertexId::new(), exact_point(3, -5, 0)),
            ],
        );
        let axis = [exact_integer(1), exact_integer(0), exact_integer(0)];
        let half_thickness = exact_integer(2);
        let exact_limits = ExactTreePoseLimits::default().cayley;
        let mut meter = WorkMeter::new(&exact_limits);
        let radius =
            exact_endpoint_face_thickness_radius_v2(&face, &half_thickness, &axis, &mut meter)
                .unwrap();
        assert!(radius.is_zero());

        let interval =
            exact_endpoint_solid_projection_with_radius_v2(&face, &axis, &radius, &mut meter)
                .unwrap()
                .expect("a non-empty face and non-zero axis have a projection");
        assert_eq!(interval.lower, exact_integer(-2));
        assert_eq!(interval.upper, exact_integer(3));
    }

    #[test]
    fn face_radius_matches_explicit_rational_prism_vertex_projections() {
        let mut face = exact_face(
            FaceId::new(),
            [
                (VertexId::new(), exact_point(1, 2, 3)),
                (VertexId::new(), exact_point(-1, 0, 2)),
                (VertexId::new(), exact_point(0, 0, 2)),
            ],
        );
        face.transform.rotation = [
            [exact_ratio(4, 5), exact_ratio(3, 5), exact_integer(0)],
            [exact_ratio(-3, 5), exact_ratio(4, 5), exact_integer(0)],
            [exact_integer(0), exact_integer(0), exact_integer(1)],
        ];
        let axis = [exact_integer(2), exact_integer(-1), exact_integer(3)];
        let half_thickness = exact_ratio(5, 2);
        let exact_limits = ExactTreePoseLimits::default().cayley;
        let mut meter = WorkMeter::new(&exact_limits);

        let radius =
            exact_endpoint_face_thickness_radius_v2(&face, &half_thickness, &axis, &mut meter)
                .unwrap();
        assert_eq!(radius, exact_integer(1));
        let interval =
            exact_endpoint_solid_projection_with_radius_v2(&face, &axis, &radius, &mut meter)
                .unwrap()
                .expect("the rational prism has an exact projection");

        // The two mid-surface projections are 9 and 4. The explicit offsets
        // `±h*n_face = ±(3/2, 2, 0)` project to ±1 on this axis.
        assert_eq!(interval.lower, exact_integer(3));
        assert_eq!(interval.upper, exact_integer(10));
    }

    #[test]
    fn different_face_normals_receive_independent_exact_radii() {
        let identity = exact_face(FaceId::new(), [(VertexId::new(), exact_point(0, 0, 0))]);
        let mut rotated = exact_face(FaceId::new(), [(VertexId::new(), exact_point(0, 0, 0))]);
        rotated.transform.rotation = [
            [exact_integer(0), exact_integer(1), exact_integer(0)],
            [exact_integer(1), exact_integer(0), exact_integer(0)],
            [exact_integer(0), exact_integer(0), exact_integer(-1)],
        ];
        let axis = [exact_integer(1), exact_integer(0), exact_integer(0)];
        let half_thickness = exact_integer(3);
        let exact_limits = ExactTreePoseLimits::default().cayley;
        let mut meter = WorkMeter::new(&exact_limits);

        let identity_radius =
            exact_endpoint_face_thickness_radius_v2(&identity, &half_thickness, &axis, &mut meter)
                .unwrap();
        let rotated_radius =
            exact_endpoint_face_thickness_radius_v2(&rotated, &half_thickness, &axis, &mut meter)
                .unwrap();
        assert!(identity_radius.is_zero());
        assert_eq!(rotated_radius, half_thickness);
    }

    #[test]
    fn planar_edge_l1_enclosure_retains_face_specific_contact() {
        let first = exact_face(
            FaceId::new(),
            [
                (VertexId::new(), exact_point(0, 0, 0)),
                (VertexId::new(), exact_point(0, 0, 1)),
                (VertexId::new(), exact_point(1, 0, 0)),
            ],
        );
        let mut second = exact_face(
            FaceId::new(),
            [
                (VertexId::new(), exact_point(2, 0, 0)),
                (VertexId::new(), exact_point(2, 0, 1)),
                (VertexId::new(), exact_point(2, 1, 0)),
            ],
        );
        second.transform.rotation = [
            [exact_integer(0), exact_integer(1), exact_integer(0)],
            [exact_integer(1), exact_integer(0), exact_integer(0)],
            [exact_integer(0), exact_integer(0), exact_integer(-1)],
        ];
        let exact_limits = ExactTreePoseLimits::default().cayley;
        let mut meter = WorkMeter::new(&exact_limits);

        assert!(
            !exact_endpoint_faces_separated_by_planar_edge_v2(
                &first,
                &second,
                &exact_integer(2),
                &mut meter,
            )
            .unwrap(),
            "the first mid-surface touches the second face's x-normal solid at x=0"
        );
    }

    #[test]
    fn rational_face_broadphase_exclusion_is_exact_prism_separation() {
        let mut first = exact_face(
            FaceId::new(),
            [
                (VertexId::new(), exact_point(0, 0, 0)),
                (
                    VertexId::new(),
                    ExactPoint3 {
                        coordinates: [exact_ratio(4, 5), exact_ratio(-3, 5), exact_integer(0)],
                    },
                ),
                (VertexId::new(), exact_point(0, 0, 1)),
            ],
        );
        first.transform.rotation = [
            [exact_ratio(4, 5), exact_ratio(3, 5), exact_integer(0)],
            [exact_ratio(-3, 5), exact_ratio(4, 5), exact_integer(0)],
            [exact_integer(0), exact_integer(0), exact_integer(1)],
        ];
        let mut second = exact_face(
            FaceId::new(),
            [
                (VertexId::new(), exact_point(10, 0, 0)),
                (
                    VertexId::new(),
                    ExactPoint3 {
                        coordinates: [exact_ratio(54, 5), exact_ratio(3, 5), exact_integer(0)],
                    },
                ),
                (VertexId::new(), exact_point(10, 0, 1)),
            ],
        );
        second.transform.rotation = [
            [exact_ratio(4, 5), exact_ratio(-3, 5), exact_integer(0)],
            [exact_ratio(3, 5), exact_ratio(4, 5), exact_integer(0)],
            [exact_integer(0), exact_integer(0), exact_integer(1)],
        ];
        let half_thickness = exact_integer(1);
        let exact_limits = ExactTreePoseLimits::default().cayley;
        let mut meter = WorkMeter::new(&exact_limits);
        let mut broadphase_separated = false;
        for axis in exact_endpoint_fixed_axes_v2() {
            let first_radius =
                exact_endpoint_face_thickness_radius_v2(&first, &half_thickness, &axis, &mut meter)
                    .unwrap();
            let second_radius = exact_endpoint_face_thickness_radius_v2(
                &second,
                &half_thickness,
                &axis,
                &mut meter,
            )
            .unwrap();
            let first_interval = exact_endpoint_solid_projection_with_radius_v2(
                &first,
                &axis,
                &first_radius,
                &mut meter,
            )
            .unwrap()
            .unwrap();
            let second_interval = exact_endpoint_solid_projection_with_radius_v2(
                &second,
                &axis,
                &second_radius,
                &mut meter,
            )
            .unwrap()
            .unwrap();
            broadphase_separated |= exact_endpoint_intervals_strictly_separated_v2(
                &first_interval,
                &second_interval,
                &mut meter,
            )
            .unwrap();
        }
        broadphase_separated |= exact_endpoint_faces_separated_by_planar_edge_v2(
            &first,
            &second,
            &half_thickness,
            &mut meter,
        )
        .unwrap();
        assert!(
            broadphase_separated,
            "the exact endpoint broadphase must discharge this distant rational pair"
        );

        let prism = |face: &ExactFacePose| ExactTriangularPrismInput {
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
        };
        let analysis = analyze_exact_prism_pair_v1(
            &prism(&first),
            &prism(&second),
            ExactPrismLimits::default(),
        )
        .expect("the explicit rational prisms are within hard work limits");
        assert_eq!(
            analysis
                .intersection
                .expect("the explicit rational prisms are valid")
                .kind(),
            ExactPrismIntersectionKind::Empty,
            "every broadphase exclusion must agree with the exact prism kernel"
        );
    }

    #[test]
    fn lazy_fixed_axis_interval_charges_face_radius_only_once() {
        let face = exact_face(
            FaceId::new(),
            [
                (VertexId::new(), exact_point(0, 0, 0)),
                (VertexId::new(), exact_point(1, 0, 0)),
                (VertexId::new(), exact_point(0, 0, 1)),
            ],
        );
        let mut bound = exact_bound(face.face, 0, 0);
        let axis = [exact_integer(0), exact_integer(1), exact_integer(0)];
        let exact_limits = ExactTreePoseLimits::default().cayley;
        let mut meter = WorkMeter::new(&exact_limits);

        let mut one_short_limits = ExactTreePoseLimits::default().cayley;
        one_short_limits.max_interval_operations = 34;
        let mut one_short_meter = WorkMeter::new(&one_short_limits);
        let mut rejected = exact_bound(face.face, 0, 0);
        assert!(matches!(
            exact_endpoint_prepare_lazy_fixed_axis_interval_v2(
                &mut rejected,
                &face,
                0,
                &axis,
                &exact_integer(1),
                &mut one_short_meter,
            ),
            Err(PositiveThicknessPrismScanErrorV1::ResourceLimitExceeded)
        ));
        assert!(
            rejected.lazy_fixed_axis_intervals[0].is_none(),
            "a failed lazy projection must leave its slot unmaterialized"
        );

        exact_endpoint_prepare_lazy_fixed_axis_interval_v2(
            &mut bound,
            &face,
            0,
            &axis,
            &exact_integer(1),
            &mut meter,
        )
        .unwrap();
        assert_eq!(meter.work.interval_operations, 35);
        let first_interval = bound.lazy_fixed_axis_intervals[0]
            .as_ref()
            .expect("first call materializes the interval");
        assert_eq!(first_interval.lower, exact_integer(-1));
        assert_eq!(first_interval.upper, exact_integer(1));

        exact_endpoint_prepare_lazy_fixed_axis_interval_v2(
            &mut bound,
            &face,
            0,
            &axis,
            &exact_integer(1),
            &mut meter,
        )
        .unwrap();
        assert_eq!(
            meter.work.interval_operations, 35,
            "a populated lazy slot must not recompute either radius or projection"
        );
    }

    #[test]
    fn shared_vertex_pair_is_retained_before_projection_work() {
        let mut faces = [FaceId::new(), FaceId::new()];
        faces.sort_unstable_by_key(FaceId::canonical_bytes);
        let shared = VertexId::new();
        let first = exact_face(faces[0], [(shared, exact_point(0, 0, 0))]);
        let second = exact_face(faces[1], [(shared, exact_point(0, 0, 0))]);
        let pair = [faces[0], faces[1]];
        let mut candidates = Vec::new();
        let exact_limits = ExactTreePoseLimits::default().cayley;
        let mut meter = WorkMeter::new(&exact_limits);

        assert!(
            exact_endpoint_retain_shared_vertex_pair_v2(
                &first,
                &second,
                pair,
                &mut candidates,
                1,
                &mut meter,
            )
            .unwrap(),
            "a shared exact material vertex is touching and must bypass projection exclusion"
        );
        assert_eq!(candidates, vec![(faces[0], faces[1])]);

        let mut capped_candidates = Vec::new();
        let mut capped_meter = WorkMeter::new(&exact_limits);
        assert!(matches!(
            exact_endpoint_retain_shared_vertex_pair_v2(
                &first,
                &second,
                pair,
                &mut capped_candidates,
                0,
                &mut capped_meter,
            ),
            Err(PositiveThicknessPrismScanErrorV1::ResourceLimitExceeded)
        ));
        assert!(capped_candidates.is_empty());
    }

    #[test]
    fn shared_vertex_search_is_charged_and_resource_bounded() {
        let mut faces = [FaceId::new(), FaceId::new()];
        faces.sort_unstable_by_key(FaceId::canonical_bytes);
        let vertices = [
            VertexId::new(),
            VertexId::new(),
            VertexId::new(),
            VertexId::new(),
            VertexId::new(),
        ];
        let first = exact_face(
            faces[0],
            [
                (vertices[0], exact_point(0, 0, 0)),
                (vertices[1], exact_point(1, 0, 0)),
                (vertices[4], exact_point(0, 1, 0)),
            ],
        );
        let second = exact_face(
            faces[1],
            [
                (vertices[2], exact_point(2, 0, 0)),
                (vertices[3], exact_point(2, 1, 0)),
                (vertices[4], exact_point(0, 1, 0)),
            ],
        );
        let pair = [faces[0], faces[1]];

        let mut eight_comparison_limits = ExactTreePoseLimits::default().cayley;
        eight_comparison_limits.max_interval_operations = 8;
        let mut eight_comparison_meter = WorkMeter::new(&eight_comparison_limits);
        let mut rejected_candidates = Vec::new();
        assert!(matches!(
            exact_endpoint_retain_shared_vertex_pair_v2(
                &first,
                &second,
                pair,
                &mut rejected_candidates,
                1,
                &mut eight_comparison_meter,
            ),
            Err(PositiveThicknessPrismScanErrorV1::ResourceLimitExceeded)
        ));
        assert!(rejected_candidates.is_empty());

        let mut nine_comparison_limits = ExactTreePoseLimits::default().cayley;
        nine_comparison_limits.max_interval_operations = 9;
        let mut nine_comparison_meter = WorkMeter::new(&nine_comparison_limits);
        let mut retained_candidates = Vec::new();
        assert!(
            exact_endpoint_retain_shared_vertex_pair_v2(
                &first,
                &second,
                pair,
                &mut retained_candidates,
                1,
                &mut nine_comparison_meter,
            )
            .unwrap()
        );
        assert_eq!(retained_candidates, vec![(faces[0], faces[1])]);
        assert_eq!(nine_comparison_meter.work.interval_operations, 9);
    }

    #[test]
    fn binary_insertion_sort_is_deterministic_with_exact_lower_ties() {
        let mut faces = (0..6).map(|_| FaceId::new()).collect::<Vec<_>>();
        faces.sort_unstable_by_key(FaceId::canonical_bytes);
        let lowers = [2, 1, 1, -1, 2, 0];
        let first_order = [0, 1, 2, 3, 4, 5];
        let second_order = [5, 3, 1, 4, 2, 0];
        let make_bounds = |order: [usize; 6]| {
            order
                .into_iter()
                .map(|index| exact_bound(faces[index], index, lowers[index]))
                .collect::<Vec<_>>()
        };
        let mut first = make_bounds(first_order);
        let mut second = make_bounds(second_order);
        let exact_limits = ExactTreePoseLimits::default().cayley;
        let mut first_meter = WorkMeter::new(&exact_limits);
        let mut second_meter = WorkMeter::new(&exact_limits);

        exact_endpoint_sort_face_bounds_v2(&mut first, &mut first_meter).unwrap();
        exact_endpoint_sort_face_bounds_v2(&mut second, &mut second_meter).unwrap();

        let first_faces = first.iter().map(|bound| bound.face).collect::<Vec<_>>();
        let second_faces = second.iter().map(|bound| bound.face).collect::<Vec<_>>();
        let expected = vec![faces[3], faces[5], faces[1], faces[2], faces[0], faces[4]];
        assert_eq!(first_faces, expected);
        assert_eq!(second_faces, expected);
    }

    #[test]
    fn binary_insertion_sort_charges_slot_moves_before_mutation() {
        let mut faces = [FaceId::new(), FaceId::new()];
        faces.sort_unstable_by_key(FaceId::canonical_bytes);
        let make_reverse_bounds = || vec![exact_bound(faces[0], 0, 1), exact_bound(faces[1], 1, 0)];

        let mut comparison_only_limits = ExactTreePoseLimits::default().cayley;
        comparison_only_limits.max_interval_operations = 1;
        let mut comparison_only_meter = WorkMeter::new(&comparison_only_limits);
        let mut rejected = make_reverse_bounds();
        let original_faces = rejected.iter().map(|bound| bound.face).collect::<Vec<_>>();
        assert!(matches!(
            exact_endpoint_sort_face_bounds_v2(&mut rejected, &mut comparison_only_meter),
            Err(PositiveThicknessPrismScanErrorV1::ResourceLimitExceeded)
        ));
        assert_eq!(
            rejected.iter().map(|bound| bound.face).collect::<Vec<_>>(),
            original_faces,
            "a failed move precharge must leave the slice untouched"
        );

        let mut comparison_and_move_limits = ExactTreePoseLimits::default().cayley;
        comparison_and_move_limits.max_interval_operations = 2;
        let mut comparison_and_move_meter = WorkMeter::new(&comparison_and_move_limits);
        let mut sorted = make_reverse_bounds();
        exact_endpoint_sort_face_bounds_v2(&mut sorted, &mut comparison_and_move_meter).unwrap();
        assert_eq!(
            sorted.iter().map(|bound| bound.face).collect::<Vec<_>>(),
            vec![faces[1], faces[0]]
        );
        assert_eq!(comparison_and_move_meter.work.interval_operations, 2);
    }
}
