//! Pair-local proof retention checks for complete revision rebinding.

use ori_domain::{EdgeId, FaceId, VertexId};

use super::super::PairProofCacheEntryV1;
use super::{
    AppliedEditImpactSetV1, ExactFacePoseCacheWitnessV1, FaceDependencyFootprintV1,
    ProofCacheErrorV1, ProofCacheOperationControlV1, ProofCacheRebindRequestV1, charge_many_v1,
    charge_v1,
};

pub(super) fn entry_is_retainable_v1(
    entry: &PairProofCacheEntryV1,
    request: &ProofCacheRebindRequestV1,
    work: &mut usize,
    work_limit: usize,
    control: &ProofCacheOperationControlV1<'_>,
) -> Result<bool, ProofCacheErrorV1> {
    if request.impact.target_revision != request.context.revision
        || request.context.revision <= entry.key.revision
        || request.context.pose_generation <= entry.key.pose_generation
        || request.context.paper_thickness_bits != entry.key.paper_thickness_bits
        || request.context.issuer_context != entry.key.issuer_context
    {
        return Ok(false);
    }

    for old_footprint in &entry.dependencies.footprints {
        let Some(current_footprint) = find_footprint_v1(
            &request.current_footprints,
            old_footprint.face,
            work,
            work_limit,
            control,
        )?
        else {
            return Ok(false);
        };
        charge_many_v1(
            work,
            old_footprint
                .vertices
                .len()
                .checked_add(old_footprint.edges.len())
                .and_then(|value| value.checked_add(current_footprint.vertices.len()))
                .and_then(|value| value.checked_add(current_footprint.edges.len()))
                .ok_or(ProofCacheErrorV1::ResourceLimitExceeded)?,
            work_limit,
            control,
        )?;
        if old_footprint != current_footprint
            || footprint_intersects_impact_v1(
                old_footprint,
                &request.impact,
                work,
                work_limit,
                control,
            )?
            || footprint_intersects_impact_v1(
                current_footprint,
                &request.impact,
                work,
                work_limit,
                control,
            )?
        {
            return Ok(false);
        }
    }

    for old_pose in &entry.dependencies.exact_poses {
        let Some(current_pose) = find_exact_pose_v1(
            &request.current_exact_poses,
            old_pose.face,
            work,
            work_limit,
            control,
        )?
        else {
            return Ok(false);
        };
        charge_many_v1(
            work,
            old_pose
                .canonical_exact_bytes
                .len()
                .checked_add(current_pose.canonical_exact_bytes.len())
                .ok_or(ProofCacheErrorV1::ResourceLimitExceeded)?,
            work_limit,
            control,
        )?;
        if old_pose != current_pose {
            return Ok(false);
        }
    }

    for dependency in &entry.dependencies.memo_dependencies {
        let mut authenticated = false;
        for current in &request.healthy_memo_dependencies {
            charge_v1(work, work_limit, control)?;
            if dependency == current {
                authenticated = true;
                break;
            }
        }
        if !authenticated {
            return Ok(false);
        }
    }
    Ok(true)
}

fn find_footprint_v1<'a>(
    footprints: &'a [FaceDependencyFootprintV1],
    face: FaceId,
    work: &mut usize,
    work_limit: usize,
    control: &ProofCacheOperationControlV1<'_>,
) -> Result<Option<&'a FaceDependencyFootprintV1>, ProofCacheErrorV1> {
    for footprint in footprints {
        charge_v1(work, work_limit, control)?;
        if footprint.face == face {
            return Ok(Some(footprint));
        }
    }
    Ok(None)
}

fn find_exact_pose_v1<'a>(
    poses: &'a [ExactFacePoseCacheWitnessV1],
    face: FaceId,
    work: &mut usize,
    work_limit: usize,
    control: &ProofCacheOperationControlV1<'_>,
) -> Result<Option<&'a ExactFacePoseCacheWitnessV1>, ProofCacheErrorV1> {
    for pose in poses {
        charge_v1(work, work_limit, control)?;
        if pose.face == face {
            return Ok(Some(pose));
        }
    }
    Ok(None)
}

fn footprint_intersects_impact_v1(
    footprint: &FaceDependencyFootprintV1,
    impact: &AppliedEditImpactSetV1,
    work: &mut usize,
    work_limit: usize,
    control: &ProofCacheOperationControlV1<'_>,
) -> Result<bool, ProofCacheErrorV1> {
    if contains_face_v1(&impact.faces, footprint.face, work, work_limit, control)? {
        return Ok(true);
    }
    for vertex in &footprint.vertices {
        if contains_vertex_v1(&impact.vertices, *vertex, work, work_limit, control)? {
            return Ok(true);
        }
    }
    for edge in &footprint.edges {
        if contains_edge_v1(&impact.edges, *edge, work, work_limit, control)? {
            return Ok(true);
        }
    }
    Ok(false)
}

macro_rules! contains_canonical_id_v1 {
    ($name:ident, $id:ty) => {
        fn $name(
            values: &[$id],
            target: $id,
            work: &mut usize,
            work_limit: usize,
            control: &ProofCacheOperationControlV1<'_>,
        ) -> Result<bool, ProofCacheErrorV1> {
            let lookup_work = if values.is_empty() {
                1
            } else {
                usize::try_from(usize::BITS - values.len().leading_zeros())
                    .map_err(|_| ProofCacheErrorV1::ResourceLimitExceeded)?
            };
            charge_many_v1(work, lookup_work, work_limit, control)?;
            Ok(values
                .binary_search_by(|value| value.canonical_bytes().cmp(&target.canonical_bytes()))
                .is_ok())
        }
    };
}

contains_canonical_id_v1!(contains_face_v1, FaceId);
contains_canonical_id_v1!(contains_vertex_v1, VertexId);
contains_canonical_id_v1!(contains_edge_v1, EdgeId);
