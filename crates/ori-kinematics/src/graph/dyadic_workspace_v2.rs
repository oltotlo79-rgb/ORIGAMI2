use std::{collections::VecDeque, mem::size_of};

use ori_domain::EdgeId;
use ori_topology::FoldAssignment;
use sha2::{Digest, Sha256};

use super::*;
use crate::schedule::{CycleScheduleDyadicEvaluationErrorV2, CycleScheduleDyadicWorkspaceBoundV2};

/// Resource policy for the allocation-bounded, adaptive dyadic V2 engine.
///
/// This type deliberately remains crate-private until the general-N wrappers
/// define their public compatibility surface. Every byte field is a hard
/// ceiling; `usize::MAX` is rejected rather than treated as unbounded. The
/// caller-owned borrowed schedule's retained heap is outside this primitive's
/// accounting. A wrapper that owns or restricts a schedule must charge that
/// material and its construction peak separately.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DyadicIntervalClosureWorkspaceLimitsV2 {
    pub(crate) max_depth: u32,
    pub(crate) max_leaves: usize,
    pub(crate) max_work: usize,
    pub(crate) schedule_limits: CycleScheduleLimitsV1,
    pub(crate) max_carrier_index_workspace_bytes: usize,
    pub(crate) max_schedule_evaluation_workspace_bytes: usize,
    pub(crate) max_big_rational_payload_bytes: usize,
    pub(crate) max_exact_rational_object_bytes: usize,
    pub(crate) max_interval_closure_workspace_bytes: usize,
    pub(crate) max_partition_workspace_bytes: usize,
    pub(crate) max_retained_material_bytes: usize,
    pub(crate) max_publication_workspace_bytes: usize,
    pub(crate) max_peak_workspace_bytes: usize,
}

/// Charged ceilings retained with one successful V2 proof.
///
/// These values are not claimed to be allocator-independent minima. For each
/// phase the issuer records the greater of its checked preflight ceiling and
/// the physical capacities it can observe after fallible reservation. Rust's
/// reservation API reports capacity only after allocation, so an allocator may
/// briefly return an over-limit buffer before the issuer observes and rejects
/// it; no interval-closure arithmetic or material publication follows that
/// rejection. Deterministic carrier/adjacency construction may precede an
/// aggregate nested-capacity check. Big-rational payload and exact-object
/// fields are subceilings already contained in schedule-evaluation bytes and
/// must not be added to the overall peak a second time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DyadicIntervalClosureWorkspaceResourcesV2 {
    pub(crate) charged_binding_validation_upper_bound_bytes: usize,
    pub(crate) charged_theorem_recognizer_upper_bound_bytes: usize,
    pub(crate) charged_carrier_index_workspace_upper_bound_bytes: usize,
    pub(crate) charged_schedule_evaluation_workspace_upper_bound_bytes: usize,
    pub(crate) charged_big_rational_payload_upper_bound_bytes: usize,
    pub(crate) charged_exact_rational_object_upper_bound_bytes: usize,
    pub(crate) charged_interval_closure_workspace_upper_bound_bytes: usize,
    pub(crate) charged_partition_workspace_upper_bound_bytes: usize,
    pub(crate) charged_retained_material_upper_bound_bytes: usize,
    pub(crate) charged_publication_workspace_upper_bound_bytes: usize,
    pub(crate) charged_peak_workspace_upper_bound_bytes: usize,
    pub(crate) visited_partition_nodes: usize,
    pub(crate) issued_leaves: usize,
}

/// Opaque closure material produced only by the workspace-bounded issuer.
///
/// The carrier is stored once rather than once per leaf. This is intentionally
/// `Debug`-only: it is neither cloneable, serializable, nor an authority token.
#[derive(Debug)]
pub(crate) struct WorkspaceBoundedDyadicMaterialHingeIntervalClosureV2 {
    issuer_geometry: MaterialHingeGraphInstanceV1,
    fixed_face: FaceId,
    schedule_binding_fingerprint_v2: [u8; 32],
    graph_binding_fingerprint_v1: [u8; 32],
    tolerance_bits: u64,
    policy: DyadicIntervalClosureWorkspaceLimitsV2,
    partition: Vec<(u32, u64)>,
    canonical_checked_hinges: Vec<EdgeId>,
    resources: DyadicIntervalClosureWorkspaceResourcesV2,
    partition_binding_fingerprint_v2: [u8; 32],
}

impl WorkspaceBoundedDyadicMaterialHingeIntervalClosureV2 {
    #[must_use]
    pub(crate) const fn resources(&self) -> DyadicIntervalClosureWorkspaceResourcesV2 {
        self.resources
    }

    #[must_use]
    pub(crate) fn partition(&self) -> &[(u32, u64)] {
        &self.partition
    }

    #[must_use]
    pub(crate) fn canonical_checked_hinges(&self) -> &[EdgeId] {
        &self.canonical_checked_hinges
    }

    #[must_use]
    pub(crate) fn has_nonempty_canonical_complete_partition_v2(&self) -> bool {
        has_nonempty_canonical_complete_partition_v2(&self.partition)
    }

    /// Precomputed, domain-separated binding for the policy, complete
    /// partition and normalized all-hinge carrier.
    #[must_use]
    pub(crate) const fn partition_binding_fingerprint_v2(&self) -> [u8; 32] {
        self.partition_binding_fingerprint_v2
    }
}

#[derive(Debug, Clone, Copy)]
struct WorkspacePreflightV2 {
    schedule: CycleScheduleDyadicWorkspaceBoundV2,
    resources: DyadicIntervalClosureWorkspaceResourcesV2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IntervalAttemptErrorV2 {
    InvalidInput,
    ResourceLimit,
    Unproven,
    Cancelled,
    DeadlineExceeded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct IntervalAttemptSuccessV2 {
    physical_capacity_bytes: usize,
}

fn checked_vec_bytes_v2<T>(count: usize) -> Option<usize> {
    size_of::<T>().checked_mul(count)
}

fn limits_contain_usize_max_v2(limits: DyadicIntervalClosureWorkspaceLimitsV2) -> bool {
    [
        limits.max_leaves,
        limits.max_work,
        limits.schedule_limits.max_hinges,
        limits.schedule_limits.max_degree,
        limits.schedule_limits.max_work,
        limits.max_carrier_index_workspace_bytes,
        limits.max_schedule_evaluation_workspace_bytes,
        limits.max_big_rational_payload_bytes,
        limits.max_exact_rational_object_bytes,
        limits.max_interval_closure_workspace_bytes,
        limits.max_partition_workspace_bytes,
        limits.max_retained_material_bytes,
        limits.max_publication_workspace_bytes,
        limits.max_peak_workspace_bytes,
    ]
    .contains(&usize::MAX)
}

fn checked_interval_workspace_upper_bound_v2(
    _geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
) -> Option<usize> {
    let faces = audit.faces().len();
    let spanning = audit.spanning_hinges().len();
    let mut total = checked_vec_bytes_v2::<Vec<(usize, usize, bool)>>(faces)?;
    total = total
        .checked_add(checked_vec_bytes_v2::<(usize, usize, bool)>(
            spanning.checked_mul(2)?,
        )?)?
        .checked_add(checked_vec_bytes_v2::<usize>(faces)?)?
        .checked_add(checked_vec_bytes_v2::<Option<IntervalRigidTransformV1>>(
            faces,
        )?)?
        .checked_add(checked_vec_bytes_v2::<usize>(faces)?)?;
    Some(total)
}

fn checked_preflight_v2(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    schedule: &CanonicalCycleScheduleV1,
    limits: DyadicIntervalClosureWorkspaceLimitsV2,
) -> Option<WorkspacePreflightV2> {
    let schedule = schedule
        .checked_dyadic_workspace_upper_bound_v2(limits.max_depth, limits.schedule_limits)?;
    let hinges = geometry.hinges().len();
    let carrier_index = checked_vec_bytes_v2::<usize>(hinges)?;
    let interval = checked_interval_workspace_upper_bound_v2(geometry, audit)?;
    let partition_stack = checked_vec_bytes_v2::<(u32, u64)>(limits.max_leaves)?;
    let retained = size_of::<WorkspaceBoundedDyadicMaterialHingeIntervalClosureV2>()
        .checked_add(checked_vec_bytes_v2::<(u32, u64)>(limits.max_leaves)?)?
        .checked_add(checked_vec_bytes_v2::<EdgeId>(hinges)?)?;
    // SHA-256 and the result shell are stack-resident, but charging them here
    // makes the publication phase explicit and keeps the peak conservative.
    let publication = size_of::<Sha256>().checked_add(size_of::<
        WorkspaceBoundedDyadicMaterialHingeIntervalClosureV2,
    >())?;
    let proof_phase = schedule.peak_bytes().checked_add(interval)?;
    let peak = carrier_index
        .checked_add(partition_stack)?
        .checked_add(retained)?
        .checked_add(proof_phase.max(publication))?;
    Some(WorkspacePreflightV2 {
        schedule,
        resources: DyadicIntervalClosureWorkspaceResourcesV2 {
            charged_binding_validation_upper_bound_bytes: 0,
            // This V2 route never calls the legacy theorem recognizers.
            charged_theorem_recognizer_upper_bound_bytes: 0,
            charged_carrier_index_workspace_upper_bound_bytes: carrier_index,
            charged_schedule_evaluation_workspace_upper_bound_bytes: schedule.peak_bytes(),
            charged_big_rational_payload_upper_bound_bytes: schedule.big_rational_payload_bytes(),
            charged_exact_rational_object_upper_bound_bytes: schedule.exact_object_bytes(),
            charged_interval_closure_workspace_upper_bound_bytes: interval,
            charged_partition_workspace_upper_bound_bytes: partition_stack,
            charged_retained_material_upper_bound_bytes: retained,
            charged_publication_workspace_upper_bound_bytes: publication,
            charged_peak_workspace_upper_bound_bytes: peak,
            visited_partition_nodes: 0,
            issued_leaves: 0,
        },
    })
}

fn resources_fit_limits_v2(
    resources: DyadicIntervalClosureWorkspaceResourcesV2,
    limits: DyadicIntervalClosureWorkspaceLimitsV2,
) -> bool {
    resources.charged_carrier_index_workspace_upper_bound_bytes
        <= limits.max_carrier_index_workspace_bytes
        && resources.charged_schedule_evaluation_workspace_upper_bound_bytes
            <= limits.max_schedule_evaluation_workspace_bytes
        && resources.charged_big_rational_payload_upper_bound_bytes
            <= limits.max_big_rational_payload_bytes
        && resources.charged_exact_rational_object_upper_bound_bytes
            <= limits.max_exact_rational_object_bytes
        && resources.charged_interval_closure_workspace_upper_bound_bytes
            <= limits.max_interval_closure_workspace_bytes
        && resources.charged_partition_workspace_upper_bound_bytes
            <= limits.max_partition_workspace_bytes
        && resources.charged_retained_material_upper_bound_bytes
            <= limits.max_retained_material_bytes
        && resources.charged_publication_workspace_upper_bound_bytes
            <= limits.max_publication_workspace_bytes
        && resources.charged_peak_workspace_upper_bound_bytes <= limits.max_peak_workspace_bytes
}

fn refresh_peak_v2(resources: &mut DyadicIntervalClosureWorkspaceResourcesV2) -> Option<()> {
    let proof_phase = resources
        .charged_schedule_evaluation_workspace_upper_bound_bytes
        .checked_add(resources.charged_interval_closure_workspace_upper_bound_bytes)?;
    resources.charged_peak_workspace_upper_bound_bytes = resources
        .charged_carrier_index_workspace_upper_bound_bytes
        .checked_add(resources.charged_partition_workspace_upper_bound_bytes)?
        .checked_add(resources.charged_retained_material_upper_bound_bytes)?
        .checked_add(proof_phase.max(resources.charged_publication_workspace_upper_bound_bytes))?;
    Some(())
}

fn map_checkpoint_v2(
    checkpoint: &mut impl FnMut() -> Result<(), DyadicIntervalClosureStopV1>,
) -> Result<(), IntervalAttemptErrorV2> {
    checkpoint().map_err(|stop| match stop {
        DyadicIntervalClosureStopV1::Cancelled => IntervalAttemptErrorV2::Cancelled,
        DyadicIntervalClosureStopV1::DeadlineExceeded => IntervalAttemptErrorV2::DeadlineExceeded,
    })
}

fn face_index_v2(audit: &MaterialHingeGraphAudit, face: FaceId) -> Option<usize> {
    audit
        .faces()
        .binary_search_by_key(&face.canonical_bytes(), FaceId::canonical_bytes)
        .ok()
}

fn is_spanning_v2(audit: &MaterialHingeGraphAudit, edge: EdgeId) -> bool {
    audit
        .spanning_hinges()
        .binary_search_by_key(&edge.canonical_bytes(), EdgeId::canonical_bytes)
        .is_ok()
}

fn checked_interval_physical_capacity_bytes_v2(
    adjacency: &[Vec<(usize, usize, bool)>],
    adjacency_outer_capacity: usize,
    degree_capacity: usize,
    poses_capacity: usize,
    queue_capacity: usize,
) -> Option<usize> {
    let mut total = checked_vec_bytes_v2::<Vec<(usize, usize, bool)>>(adjacency_outer_capacity)?;
    for neighbors in adjacency {
        total = total.checked_add(checked_vec_bytes_v2::<(usize, usize, bool)>(
            neighbors.capacity(),
        )?)?;
    }
    total = total
        .checked_add(checked_vec_bytes_v2::<usize>(degree_capacity)?)?
        .checked_add(checked_vec_bytes_v2::<Option<IntervalRigidTransformV1>>(
            poses_capacity,
        )?)?
        .checked_add(checked_vec_bytes_v2::<usize>(queue_capacity)?)?;
    Some(total)
}

struct IntervalClosureRequestV2<'a> {
    geometry: &'a MaterialHingeGraphGeometry,
    audit: &'a MaterialHingeGraphAudit,
    fixed_face: FaceId,
    canonical_hinge_indices: &'a [usize],
    angle_boxes: &'a [(EdgeId, OutwardIntervalV1)],
    tolerance: f64,
    max_work: usize,
    max_workspace_bytes: usize,
}

fn prove_interval_closure_with_workspace_v2(
    request: IntervalClosureRequestV2<'_>,
    checkpoint: &mut impl FnMut() -> Result<(), DyadicIntervalClosureStopV1>,
) -> Result<IntervalAttemptSuccessV2, IntervalAttemptErrorV2> {
    let IntervalClosureRequestV2 {
        geometry,
        audit,
        fixed_face,
        canonical_hinge_indices,
        angle_boxes,
        tolerance,
        max_work,
        max_workspace_bytes,
    } = request;
    map_checkpoint_v2(checkpoint)?;
    if !tolerance.is_finite()
        || tolerance < 0.0
        || max_work == 0
        || geometry.face_ids() != audit.faces()
        || geometry.hinges().len() != angle_boxes.len()
        || geometry.hinges().len() != canonical_hinge_indices.len()
        || geometry.hinges().len()
            != audit
                .spanning_hinges()
                .len()
                .checked_add(audit.closure_hinges().len())
                .ok_or(IntervalAttemptErrorV2::ResourceLimit)?
        || !audit.faces().contains(&fixed_face)
    {
        return Err(IntervalAttemptErrorV2::InvalidInput);
    }
    for (position, geometry_index) in canonical_hinge_indices.iter().copied().enumerate() {
        map_checkpoint_v2(checkpoint)?;
        let hinge = geometry
            .hinges()
            .get(geometry_index)
            .ok_or(IntervalAttemptErrorV2::InvalidInput)?;
        if angle_boxes.get(position).map(|(edge, _)| *edge) != Some(hinge.edge()) {
            return Err(IntervalAttemptErrorV2::InvalidInput);
        }
    }

    let faces = audit.faces().len();
    let mut adjacency = Vec::<Vec<(usize, usize, bool)>>::new();
    adjacency
        .try_reserve_exact(faces)
        .map_err(|_| IntervalAttemptErrorV2::ResourceLimit)?;
    adjacency.resize_with(faces, Vec::new);
    let mut degrees = Vec::<usize>::new();
    degrees
        .try_reserve_exact(faces)
        .map_err(|_| IntervalAttemptErrorV2::ResourceLimit)?;
    degrees.resize(faces, 0);
    for geometry_index in canonical_hinge_indices.iter().copied() {
        map_checkpoint_v2(checkpoint)?;
        let hinge = &geometry.hinges()[geometry_index];
        if !is_spanning_v2(audit, hinge.edge()) {
            continue;
        }
        let left =
            face_index_v2(audit, hinge.left_face()).ok_or(IntervalAttemptErrorV2::InvalidInput)?;
        let right =
            face_index_v2(audit, hinge.right_face()).ok_or(IntervalAttemptErrorV2::InvalidInput)?;
        degrees[left] = degrees[left]
            .checked_add(1)
            .ok_or(IntervalAttemptErrorV2::ResourceLimit)?;
        degrees[right] = degrees[right]
            .checked_add(1)
            .ok_or(IntervalAttemptErrorV2::ResourceLimit)?;
    }
    for (neighbors, degree) in adjacency.iter_mut().zip(&degrees) {
        map_checkpoint_v2(checkpoint)?;
        neighbors
            .try_reserve_exact(*degree)
            .map_err(|_| IntervalAttemptErrorV2::ResourceLimit)?;
    }
    for (position, geometry_index) in canonical_hinge_indices.iter().copied().enumerate() {
        map_checkpoint_v2(checkpoint)?;
        let hinge = &geometry.hinges()[geometry_index];
        if !is_spanning_v2(audit, hinge.edge()) {
            continue;
        }
        let left =
            face_index_v2(audit, hinge.left_face()).ok_or(IntervalAttemptErrorV2::InvalidInput)?;
        let right =
            face_index_v2(audit, hinge.right_face()).ok_or(IntervalAttemptErrorV2::InvalidInput)?;
        adjacency[left].push((right, position, false));
        adjacency[right].push((left, position, true));
    }

    let mut poses = Vec::<Option<IntervalRigidTransformV1>>::new();
    poses
        .try_reserve_exact(faces)
        .map_err(|_| IntervalAttemptErrorV2::ResourceLimit)?;
    for _ in 0..faces {
        poses.push(None);
    }
    let mut queue = VecDeque::<usize>::new();
    queue
        .try_reserve(faces)
        .map_err(|_| IntervalAttemptErrorV2::ResourceLimit)?;
    let physical_capacity_bytes = checked_interval_physical_capacity_bytes_v2(
        &adjacency,
        adjacency.capacity(),
        degrees.capacity(),
        poses.capacity(),
        queue.capacity(),
    )
    .ok_or(IntervalAttemptErrorV2::ResourceLimit)?;
    if physical_capacity_bytes > max_workspace_bytes {
        return Err(IntervalAttemptErrorV2::ResourceLimit);
    }

    let interval_error = |error| match error {
        crate::OutwardIntervalErrorV1::ResourceLimit => IntervalAttemptErrorV2::ResourceLimit,
        crate::OutwardIntervalErrorV1::InvalidEndpoint
        | crate::OutwardIntervalErrorV1::DivisionByZeroInterval => IntervalAttemptErrorV2::Unproven,
    };
    let fixed_index =
        face_index_v2(audit, fixed_face).ok_or(IntervalAttemptErrorV2::InvalidInput)?;
    poses[fixed_index] = Some(IntervalRigidTransformV1::identity().map_err(interval_error)?);
    queue.push_back(fixed_index);
    let mut charged = 0usize;
    while let Some(parent_face) = queue.pop_front() {
        map_checkpoint_v2(checkpoint)?;
        let parent = poses[parent_face].ok_or(IntervalAttemptErrorV2::InvalidInput)?;
        for &(child_face, hinge_position, reverse) in &adjacency[parent_face] {
            map_checkpoint_v2(checkpoint)?;
            if poses[child_face].is_some() {
                continue;
            }
            charged = charged
                .checked_add(1)
                .filter(|value| *value <= max_work)
                .ok_or(IntervalAttemptErrorV2::ResourceLimit)?;
            let geometry_index = canonical_hinge_indices[hinge_position];
            let hinge = &geometry.hinges()[geometry_index];
            let degrees = angle_boxes[hinge_position].1;
            let mountain = hinge.assignment() == FoldAssignment::Mountain;
            let sign = if reverse ^ !mountain { -1.0 } else { 1.0 };
            let local = IntervalRigidTransformV1::about_axis(
                [
                    sign * hinge.axis().x(),
                    sign * hinge.axis().y(),
                    sign * hinge.axis().z(),
                ],
                [hinge.start().x(), hinge.start().y(), hinge.start().z()],
                degrees,
                max_work,
            )
            .map_err(interval_error)?;
            poses[child_face] = Some(parent.compose(local, max_work).map_err(interval_error)?);
            queue.push_back(child_face);
        }
    }
    if poses.iter().any(Option::is_none) {
        return Err(IntervalAttemptErrorV2::InvalidInput);
    }

    for (position, geometry_index) in canonical_hinge_indices.iter().copied().enumerate() {
        map_checkpoint_v2(checkpoint)?;
        charged = charged
            .checked_add(1)
            .filter(|value| *value <= max_work)
            .ok_or(IntervalAttemptErrorV2::ResourceLimit)?;
        let hinge = &geometry.hinges()[geometry_index];
        if is_spanning_v2(audit, hinge.edge()) {
            continue;
        }
        let left = poses[face_index_v2(audit, hinge.left_face())
            .ok_or(IntervalAttemptErrorV2::InvalidInput)?]
        .ok_or(IntervalAttemptErrorV2::InvalidInput)?;
        let right = poses[face_index_v2(audit, hinge.right_face())
            .ok_or(IntervalAttemptErrorV2::InvalidInput)?]
        .ok_or(IntervalAttemptErrorV2::InvalidInput)?;
        let degrees = angle_boxes[position].1;
        let sign = if hinge.assignment() == FoldAssignment::Mountain {
            1.0
        } else {
            -1.0
        };
        let local = IntervalRigidTransformV1::about_axis(
            [
                sign * hinge.axis().x(),
                sign * hinge.axis().y(),
                sign * hinge.axis().z(),
            ],
            [hinge.start().x(), hinge.start().y(), hinge.start().z()],
            degrees,
            max_work,
        )
        .map_err(interval_error)?;
        let expected = left.compose(local, max_work).map_err(interval_error)?;
        if !expected.universally_matches_within(right, tolerance) {
            return Err(IntervalAttemptErrorV2::Unproven);
        }
    }
    Ok(IntervalAttemptSuccessV2 {
        physical_capacity_bytes,
    })
}

fn has_nonempty_canonical_complete_partition_v2(partition: &[(u32, u64)]) -> bool {
    if partition.is_empty() {
        return false;
    }
    let mut cursor = 0_u128;
    for (depth, index) in partition {
        if *depth >= 64 || *index >= (1_u64 << depth) {
            return false;
        }
        let width = 1_u128 << (64 - depth);
        let start = u128::from(*index) * width;
        if start != cursor {
            return false;
        }
        cursor = match cursor.checked_add(width) {
            Some(value) => value,
            None => return false,
        };
    }
    cursor == (1_u128 << 64)
}

fn validate_partition_with_checkpoint_v2(
    partition: &[(u32, u64)],
    checkpoint: &mut impl FnMut() -> Result<(), DyadicIntervalClosureStopV1>,
) -> Result<bool, DyadicIntervalClosureControlErrorV1> {
    if partition.is_empty() {
        return Ok(false);
    }
    let mut cursor = 0_u128;
    for (depth, index) in partition {
        closure_checkpoint_v1(checkpoint)?;
        if *depth >= 64 || *index >= (1_u64 << depth) {
            return Ok(false);
        }
        let width = 1_u128 << (64 - depth);
        let start = u128::from(*index) * width;
        if start != cursor {
            return Ok(false);
        }
        cursor = match cursor.checked_add(width) {
            Some(value) => value,
            None => return Ok(false),
        };
    }
    Ok(cursor == (1_u128 << 64))
}

fn validate_audit_order_with_checkpoint_v2(
    audit: &MaterialHingeGraphAudit,
    checkpoint: &mut impl FnMut() -> Result<(), DyadicIntervalClosureStopV1>,
) -> Result<bool, DyadicIntervalClosureControlErrorV1> {
    for pair in audit.faces().windows(2) {
        closure_checkpoint_v1(checkpoint)?;
        if pair[0].canonical_bytes() >= pair[1].canonical_bytes() {
            return Ok(false);
        }
    }
    for edges in [audit.spanning_hinges(), audit.closure_hinges()] {
        for pair in edges.windows(2) {
            closure_checkpoint_v1(checkpoint)?;
            if pair[0].canonical_bytes() >= pair[1].canonical_bytes() {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

fn validate_carrier_with_checkpoint_v2(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    canonical_hinge_indices: &[usize],
    canonical_checked_hinges: &[EdgeId],
    checkpoint: &mut impl FnMut() -> Result<(), DyadicIntervalClosureStopV1>,
) -> Result<bool, DyadicIntervalClosureControlErrorV1> {
    if canonical_hinge_indices.len() != geometry.hinges().len()
        || canonical_checked_hinges.len() != geometry.hinges().len()
    {
        return Ok(false);
    }
    for position in 0..canonical_checked_hinges.len() {
        closure_checkpoint_v1(checkpoint)?;
        let edge = canonical_checked_hinges[position];
        if geometry.hinges()[canonical_hinge_indices[position]].edge() != edge
            || (position > 0
                && canonical_checked_hinges[position - 1].canonical_bytes()
                    >= edge.canonical_bytes())
        {
            return Ok(false);
        }
        let spanning = is_spanning_v2(audit, edge);
        let closure = audit
            .closure_hinges()
            .binary_search_by_key(&edge.canonical_bytes(), EdgeId::canonical_bytes)
            .is_ok();
        if spanning == closure {
            return Ok(false);
        }
    }
    Ok(true)
}

#[allow(clippy::too_many_arguments)]
fn compute_partition_binding_with_checkpoint_v2(
    fixed_face: FaceId,
    schedule_binding_fingerprint_v2: [u8; 32],
    graph_binding_fingerprint_v1: [u8; 32],
    tolerance_bits: u64,
    policy: DyadicIntervalClosureWorkspaceLimitsV2,
    partition: &[(u32, u64)],
    canonical_checked_hinges: &[EdgeId],
    resources: DyadicIntervalClosureWorkspaceResourcesV2,
    checkpoint: &mut impl FnMut() -> Result<(), DyadicIntervalClosureStopV1>,
) -> Result<[u8; 32], DyadicIntervalClosureControlErrorV1> {
    closure_checkpoint_v1(checkpoint)?;
    let mut hash = Sha256::new();
    hash.update(b"ORIGAMI2_WORKSPACE_BOUNDED_DYADIC_CLOSURE_V2");
    hash.update(fixed_face.canonical_bytes());
    hash.update(schedule_binding_fingerprint_v2);
    hash.update(graph_binding_fingerprint_v1);
    hash.update(tolerance_bits.to_be_bytes());
    hash.update(policy.max_depth.to_be_bytes());
    for value in [
        policy.max_leaves,
        policy.max_work,
        policy.schedule_limits.max_hinges,
        policy.schedule_limits.max_degree,
        policy.schedule_limits.max_work,
        policy.max_carrier_index_workspace_bytes,
        policy.max_schedule_evaluation_workspace_bytes,
        policy.max_big_rational_payload_bytes,
        policy.max_exact_rational_object_bytes,
        policy.max_interval_closure_workspace_bytes,
        policy.max_partition_workspace_bytes,
        policy.max_retained_material_bytes,
        policy.max_publication_workspace_bytes,
        policy.max_peak_workspace_bytes,
    ] {
        closure_checkpoint_v1(checkpoint)?;
        let framed =
            u64::try_from(value).map_err(|_| DyadicIntervalClosureErrorV1::ResourceLimit)?;
        hash.update(framed.to_be_bytes());
    }
    hash.update(policy.schedule_limits.max_coefficient_bits.to_be_bytes());
    for value in [
        resources.charged_binding_validation_upper_bound_bytes,
        resources.charged_theorem_recognizer_upper_bound_bytes,
        resources.charged_carrier_index_workspace_upper_bound_bytes,
        resources.charged_schedule_evaluation_workspace_upper_bound_bytes,
        resources.charged_big_rational_payload_upper_bound_bytes,
        resources.charged_exact_rational_object_upper_bound_bytes,
        resources.charged_interval_closure_workspace_upper_bound_bytes,
        resources.charged_partition_workspace_upper_bound_bytes,
        resources.charged_retained_material_upper_bound_bytes,
        resources.charged_publication_workspace_upper_bound_bytes,
        resources.charged_peak_workspace_upper_bound_bytes,
        resources.visited_partition_nodes,
        resources.issued_leaves,
    ] {
        closure_checkpoint_v1(checkpoint)?;
        let framed =
            u64::try_from(value).map_err(|_| DyadicIntervalClosureErrorV1::ResourceLimit)?;
        hash.update(framed.to_be_bytes());
    }
    hash.update(
        u64::try_from(partition.len())
            .map_err(|_| DyadicIntervalClosureErrorV1::ResourceLimit)?
            .to_be_bytes(),
    );
    for (depth, index) in partition {
        closure_checkpoint_v1(checkpoint)?;
        hash.update(depth.to_be_bytes());
        hash.update(index.to_be_bytes());
    }
    hash.update(
        u64::try_from(canonical_checked_hinges.len())
            .map_err(|_| DyadicIntervalClosureErrorV1::ResourceLimit)?
            .to_be_bytes(),
    );
    for edge in canonical_checked_hinges {
        closure_checkpoint_v1(checkpoint)?;
        hash.update(edge.canonical_bytes());
    }
    closure_checkpoint_v1(checkpoint)?;
    Ok(hash.finalize().into())
}

fn map_interval_control_error_v2(
    error: IntervalAttemptErrorV2,
) -> DyadicIntervalClosureControlErrorV1 {
    match error {
        IntervalAttemptErrorV2::InvalidInput => DyadicIntervalClosureErrorV1::InvalidInput.into(),
        IntervalAttemptErrorV2::ResourceLimit => DyadicIntervalClosureErrorV1::ResourceLimit.into(),
        IntervalAttemptErrorV2::Unproven => unreachable!("unproven is handled by subdivision"),
        IntervalAttemptErrorV2::Cancelled => DyadicIntervalClosureControlErrorV1::Cancelled,
        IntervalAttemptErrorV2::DeadlineExceeded => {
            DyadicIntervalClosureControlErrorV1::DeadlineExceeded
        }
    }
}

fn map_heap_sort_error_v2(
    error: CheckpointHeapSortErrorV1<DyadicIntervalClosureStopV1>,
) -> DyadicIntervalClosureControlErrorV1 {
    match error {
        CheckpointHeapSortErrorV1::ResourceLimit => {
            DyadicIntervalClosureErrorV1::ResourceLimit.into()
        }
        CheckpointHeapSortErrorV1::Stop(DyadicIntervalClosureStopV1::Cancelled) => {
            DyadicIntervalClosureControlErrorV1::Cancelled
        }
        CheckpointHeapSortErrorV1::Stop(DyadicIntervalClosureStopV1::DeadlineExceeded) => {
            DyadicIntervalClosureControlErrorV1::DeadlineExceeded
        }
    }
}

fn split_partition_leaf_v2(
    depth: u32,
    index: u64,
    pending: &mut Vec<(u32, u64)>,
    completed_len: usize,
    limits: DyadicIntervalClosureWorkspaceLimitsV2,
) -> Result<(), DyadicIntervalClosureControlErrorV1> {
    let child_depth = depth
        .checked_add(1)
        .ok_or(DyadicIntervalClosureErrorV1::ResourceLimit)?;
    let left = index
        .checked_mul(2)
        .ok_or(DyadicIntervalClosureErrorV1::ResourceLimit)?;
    let right = left
        .checked_add(1)
        .ok_or(DyadicIntervalClosureErrorV1::ResourceLimit)?;
    let future_leaves = pending
        .len()
        .checked_add(2)
        .and_then(|count| count.checked_add(completed_len))
        .ok_or(DyadicIntervalClosureErrorV1::ResourceLimit)?;
    if future_leaves > limits.max_leaves {
        return Err(DyadicIntervalClosureErrorV1::ResourceLimit.into());
    }
    // Stack order makes traversal and retained publication left-first.
    pending.push((child_depth, right));
    pending.push((child_depth, left));
    Ok(())
}

impl MaterialHingeGraphGeometry {
    /// Generic, allocation-bounded adaptive dyadic closure primitive.
    ///
    /// It intentionally knows nothing about common-articulation or N>=33
    /// admission. Higher-level wrappers may apply those structural policies to
    /// a geometry/audit/schedule before invoking this crate-private engine. The
    /// borrowed schedule's retained bytes are not part of this engine's peak;
    /// an owning/restriction wrapper must add them and restriction scratch.
    #[allow(dead_code)] // Phase 2 connects the general-N wrappers to this seam.
    pub(crate) fn prove_dyadic_schedule_closure_with_workspace_and_checkpoint_v2(
        &self,
        audit: &MaterialHingeGraphAudit,
        fixed_face: FaceId,
        schedule: &CanonicalCycleScheduleV1,
        tolerance: f64,
        limits: DyadicIntervalClosureWorkspaceLimitsV2,
        mut checkpoint: impl FnMut() -> Result<(), DyadicIntervalClosureStopV1>,
    ) -> Result<
        WorkspaceBoundedDyadicMaterialHingeIntervalClosureV2,
        DyadicIntervalClosureControlErrorV1,
    > {
        closure_checkpoint_v1(&mut checkpoint)?;
        let binding_matches = schedule
            .matches_binding_with_checkpoint_v2(self, audit, fixed_face, &mut checkpoint)
            .map_err(|stop| match stop {
                DyadicIntervalClosureStopV1::Cancelled => {
                    DyadicIntervalClosureControlErrorV1::Cancelled
                }
                DyadicIntervalClosureStopV1::DeadlineExceeded => {
                    DyadicIntervalClosureControlErrorV1::DeadlineExceeded
                }
            })?;
        if !binding_matches
            || !tolerance.is_finite()
            || tolerance < 0.0
            || limits.max_depth >= 64
            || self.face_ids() != audit.faces()
            || audit.faces().is_empty()
            || !audit.faces().contains(&fixed_face)
        {
            return Err(DyadicIntervalClosureErrorV1::InvalidInput.into());
        }
        if limits_contain_usize_max_v2(limits)
            || limits.max_leaves == 0
            || limits.max_work == 0
            || limits.schedule_limits.max_hinges == 0
            || limits.schedule_limits.max_work == 0
            || limits.schedule_limits.max_coefficient_bits == u32::MAX
        {
            return Err(DyadicIntervalClosureErrorV1::ResourceLimit.into());
        }
        if !validate_audit_order_with_checkpoint_v2(audit, &mut checkpoint)? {
            return Err(DyadicIntervalClosureErrorV1::InvalidInput.into());
        }
        let audit_hinge_count = audit
            .spanning_hinges()
            .len()
            .checked_add(audit.closure_hinges().len())
            .ok_or(DyadicIntervalClosureErrorV1::ResourceLimit)?;
        if self.hinges().is_empty() || self.hinges().len() != audit_hinge_count {
            return Err(DyadicIntervalClosureErrorV1::InvalidInput.into());
        }

        let preflight = checked_preflight_v2(self, audit, schedule, limits)
            .ok_or(DyadicIntervalClosureErrorV1::ResourceLimit)?;
        let mut resources = preflight.resources;
        if !resources_fit_limits_v2(resources, limits) {
            return Err(DyadicIntervalClosureErrorV1::ResourceLimit.into());
        }

        closure_checkpoint_v1(&mut checkpoint)?;
        let mut canonical_hinge_indices = Vec::<usize>::new();
        canonical_hinge_indices
            .try_reserve_exact(self.hinges().len())
            .map_err(|_| DyadicIntervalClosureErrorV1::ResourceLimit)?;
        let physical_carrier_index_bytes =
            checked_vec_bytes_v2::<usize>(canonical_hinge_indices.capacity())
                .ok_or(DyadicIntervalClosureErrorV1::ResourceLimit)?;
        resources.charged_carrier_index_workspace_upper_bound_bytes = resources
            .charged_carrier_index_workspace_upper_bound_bytes
            .max(physical_carrier_index_bytes);
        refresh_peak_v2(&mut resources).ok_or(DyadicIntervalClosureErrorV1::ResourceLimit)?;
        if !resources_fit_limits_v2(resources, limits) {
            return Err(DyadicIntervalClosureErrorV1::ResourceLimit.into());
        }
        canonical_hinge_indices.extend(0..self.hinges().len());
        checkpoint_heap_sort_by_key_v1(
            &mut canonical_hinge_indices,
            |index| self.hinges()[*index].edge().canonical_bytes(),
            &mut checkpoint,
        )
        .map_err(map_heap_sort_error_v2)?;
        let mut canonical_checked_hinges = Vec::<EdgeId>::new();
        canonical_checked_hinges
            .try_reserve_exact(self.hinges().len())
            .map_err(|_| DyadicIntervalClosureErrorV1::ResourceLimit)?;
        let retained_after_carrier_reserve =
            size_of::<WorkspaceBoundedDyadicMaterialHingeIntervalClosureV2>()
                .checked_add(
                    checked_vec_bytes_v2::<(u32, u64)>(limits.max_leaves)
                        .ok_or(DyadicIntervalClosureErrorV1::ResourceLimit)?,
                )
                .and_then(|bytes| {
                    checked_vec_bytes_v2::<EdgeId>(canonical_checked_hinges.capacity())
                        .and_then(|carrier| bytes.checked_add(carrier))
                })
                .ok_or(DyadicIntervalClosureErrorV1::ResourceLimit)?;
        resources.charged_retained_material_upper_bound_bytes = resources
            .charged_retained_material_upper_bound_bytes
            .max(retained_after_carrier_reserve);
        refresh_peak_v2(&mut resources).ok_or(DyadicIntervalClosureErrorV1::ResourceLimit)?;
        if !resources_fit_limits_v2(resources, limits) {
            return Err(DyadicIntervalClosureErrorV1::ResourceLimit.into());
        }
        for index in &canonical_hinge_indices {
            closure_checkpoint_v1(&mut checkpoint)?;
            canonical_checked_hinges.push(self.hinges()[*index].edge());
        }
        if !validate_carrier_with_checkpoint_v2(
            self,
            audit,
            &canonical_hinge_indices,
            &canonical_checked_hinges,
            &mut checkpoint,
        )? {
            return Err(DyadicIntervalClosureErrorV1::InvalidInput.into());
        }
        let mut pending = Vec::<(u32, u64)>::new();
        pending
            .try_reserve_exact(limits.max_leaves)
            .map_err(|_| DyadicIntervalClosureErrorV1::ResourceLimit)?;
        let physical_partition_workspace = checked_vec_bytes_v2::<(u32, u64)>(pending.capacity())
            .ok_or(DyadicIntervalClosureErrorV1::ResourceLimit)?;
        resources.charged_partition_workspace_upper_bound_bytes = resources
            .charged_partition_workspace_upper_bound_bytes
            .max(physical_partition_workspace);
        refresh_peak_v2(&mut resources).ok_or(DyadicIntervalClosureErrorV1::ResourceLimit)?;
        if !resources_fit_limits_v2(resources, limits) {
            return Err(DyadicIntervalClosureErrorV1::ResourceLimit.into());
        }
        let mut partition = Vec::<(u32, u64)>::new();
        partition
            .try_reserve_exact(limits.max_leaves)
            .map_err(|_| DyadicIntervalClosureErrorV1::ResourceLimit)?;
        let physical_retained = size_of::<WorkspaceBoundedDyadicMaterialHingeIntervalClosureV2>()
            .checked_add(
                checked_vec_bytes_v2::<(u32, u64)>(partition.capacity())
                    .ok_or(DyadicIntervalClosureErrorV1::ResourceLimit)?,
            )
            .and_then(|bytes| {
                checked_vec_bytes_v2::<EdgeId>(canonical_checked_hinges.capacity())
                    .and_then(|carrier| bytes.checked_add(carrier))
            })
            .ok_or(DyadicIntervalClosureErrorV1::ResourceLimit)?;
        resources.charged_retained_material_upper_bound_bytes = resources
            .charged_retained_material_upper_bound_bytes
            .max(physical_retained);
        refresh_peak_v2(&mut resources).ok_or(DyadicIntervalClosureErrorV1::ResourceLimit)?;
        if !resources_fit_limits_v2(resources, limits) {
            return Err(DyadicIntervalClosureErrorV1::ResourceLimit.into());
        }

        pending.push((0, 0));
        let mut visited = 0usize;
        while let Some((depth, index)) = pending.pop() {
            closure_checkpoint_v1(&mut checkpoint)?;
            visited = visited
                .checked_add(1)
                .filter(|value| *value <= limits.max_work)
                .ok_or(DyadicIntervalClosureErrorV1::ResourceLimit)?;

            let evaluation = schedule.evaluate_angle_box_dyadic_with_workspace_v2(
                depth,
                index,
                limits.schedule_limits,
                preflight.schedule,
                limits.max_schedule_evaluation_workspace_bytes,
            );
            let evaluation = match evaluation {
                Ok(evaluation) => evaluation,
                Err(CycleScheduleDyadicEvaluationErrorV2::WorkspaceLimit) => {
                    return Err(DyadicIntervalClosureErrorV1::ResourceLimit.into());
                }
                Err(CycleScheduleDyadicEvaluationErrorV2::Prepare(
                    crate::CycleSchedulePrepareErrorV1::ResourceLimit,
                )) if depth < limits.max_depth => {
                    closure_checkpoint_v1(&mut checkpoint)?;
                    split_partition_leaf_v2(depth, index, &mut pending, partition.len(), limits)?;
                    continue;
                }
                Err(CycleScheduleDyadicEvaluationErrorV2::Prepare(
                    crate::CycleSchedulePrepareErrorV1::ResourceLimit,
                )) => return Err(DyadicIntervalClosureErrorV1::ResourceLimit.into()),
                Err(CycleScheduleDyadicEvaluationErrorV2::Prepare(_)) => {
                    return Err(DyadicIntervalClosureErrorV1::InvalidInput.into());
                }
            };
            let observed_exact_objects = evaluation
                .exact_vector_capacity_peak_bytes
                .checked_add(preflight.schedule.exact_nonvector_object_bytes())
                .ok_or(DyadicIntervalClosureErrorV1::ResourceLimit)?;
            resources.charged_exact_rational_object_upper_bound_bytes = resources
                .charged_exact_rational_object_upper_bound_bytes
                .max(observed_exact_objects);
            let observed_schedule_peak = evaluation
                .angle_box_capacity_bytes
                .checked_add(preflight.schedule.big_rational_payload_bytes())
                .and_then(|bytes| {
                    bytes.checked_add(resources.charged_exact_rational_object_upper_bound_bytes)
                })
                .ok_or(DyadicIntervalClosureErrorV1::ResourceLimit)?;
            resources.charged_schedule_evaluation_workspace_upper_bound_bytes = resources
                .charged_schedule_evaluation_workspace_upper_bound_bytes
                .max(observed_schedule_peak);
            refresh_peak_v2(&mut resources).ok_or(DyadicIntervalClosureErrorV1::ResourceLimit)?;
            if !resources_fit_limits_v2(resources, limits) {
                return Err(DyadicIntervalClosureErrorV1::ResourceLimit.into());
            }

            let interval = prove_interval_closure_with_workspace_v2(
                IntervalClosureRequestV2 {
                    geometry: self,
                    audit,
                    fixed_face,
                    canonical_hinge_indices: &canonical_hinge_indices,
                    angle_boxes: &evaluation.angle_boxes,
                    tolerance,
                    max_work: limits.max_work,
                    max_workspace_bytes: limits.max_interval_closure_workspace_bytes,
                },
                &mut checkpoint,
            );
            match interval {
                Ok(success) => {
                    resources.charged_interval_closure_workspace_upper_bound_bytes = resources
                        .charged_interval_closure_workspace_upper_bound_bytes
                        .max(success.physical_capacity_bytes);
                    refresh_peak_v2(&mut resources)
                        .ok_or(DyadicIntervalClosureErrorV1::ResourceLimit)?;
                    if !resources_fit_limits_v2(resources, limits)
                        || partition.len() >= limits.max_leaves
                    {
                        return Err(DyadicIntervalClosureErrorV1::ResourceLimit.into());
                    }
                    closure_checkpoint_v1(&mut checkpoint)?;
                    partition.push((depth, index));
                }
                Err(IntervalAttemptErrorV2::Unproven) if depth < limits.max_depth => {
                    closure_checkpoint_v1(&mut checkpoint)?;
                    split_partition_leaf_v2(depth, index, &mut pending, partition.len(), limits)?;
                }
                Err(IntervalAttemptErrorV2::Unproven) => {
                    return Err(
                        DyadicIntervalClosureErrorV1::UnprovenClosure { depth, index }.into(),
                    );
                }
                Err(error) => return Err(map_interval_control_error_v2(error)),
            }
        }

        resources.visited_partition_nodes = visited;
        resources.issued_leaves = partition.len();
        if !validate_partition_with_checkpoint_v2(&partition, &mut checkpoint)?
            || !validate_carrier_with_checkpoint_v2(
                self,
                audit,
                &canonical_hinge_indices,
                &canonical_checked_hinges,
                &mut checkpoint,
            )?
        {
            return Err(DyadicIntervalClosureErrorV1::InvalidInput.into());
        }
        let schedule_binding_fingerprint_v2 = schedule.certificate_binding_fingerprint_v2();
        let graph_binding_fingerprint_v1 = schedule.graph_binding_fingerprint_v1();
        let partition_binding_fingerprint_v2 = compute_partition_binding_with_checkpoint_v2(
            fixed_face,
            schedule_binding_fingerprint_v2,
            graph_binding_fingerprint_v1,
            tolerance.to_bits(),
            limits,
            &partition,
            &canonical_checked_hinges,
            resources,
            &mut checkpoint,
        )?;
        let material = WorkspaceBoundedDyadicMaterialHingeIntervalClosureV2 {
            issuer_geometry: self.instance_anchor_v1(),
            fixed_face,
            schedule_binding_fingerprint_v2,
            graph_binding_fingerprint_v1,
            tolerance_bits: tolerance.to_bits(),
            policy: limits,
            partition,
            canonical_checked_hinges,
            resources,
            partition_binding_fingerprint_v2,
        };
        // Publication self-audit: consume every sealed field through the same
        // observation seam that the Phase 2 wrapper will use. No allocation or
        // exact/interval arithmetic occurs here.
        if !material.issuer_geometry.matches(self)
            || material.fixed_face != fixed_face
            || material.schedule_binding_fingerprint_v2 != schedule_binding_fingerprint_v2
            || material.graph_binding_fingerprint_v1 != graph_binding_fingerprint_v1
            || material.tolerance_bits != tolerance.to_bits()
            || material.policy != limits
            || material.resources() != resources
            || material.partition().len() != resources.issued_leaves
            || !material.has_nonempty_canonical_complete_partition_v2()
            || material.canonical_checked_hinges().len() != self.hinges().len()
            || material.partition_binding_fingerprint_v2() != partition_binding_fingerprint_v2
        {
            return Err(DyadicIntervalClosureErrorV1::InvalidInput.into());
        }
        // Publication checkpoint: no allocation or fallible proof work occurs
        // between this poll and returning the sealed material.
        closure_checkpoint_v1(&mut checkpoint)?;
        Ok(material)
    }
}

#[cfg(test)]
mod tests;
