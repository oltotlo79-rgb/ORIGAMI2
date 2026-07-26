//! Complete, bounded editor-impact derivation for pair-proof invalidation.

use std::time::{Duration, Instant};

use ori_collision::{MAX_PROOF_CACHE_INVALIDATION_WORK_V1, ProofCacheOperationControlV1};
use ori_domain::{CreasePattern, EdgeId, FaceId, Paper, VertexId};

use super::{ProjectState, applied_pose::PoseAuthorityInvalidation};

struct CompleteProofCacheEditImpactV1 {
    vertices: Vec<VertexId>,
    edges: Vec<EdgeId>,
    faces: Vec<FaceId>,
    preparation_work: usize,
}

/// Derives every directly edited primitive ID bit-exactly. Native FaceIds are
/// derived rather than editor-mutable: pair invalidation expands face impact
/// through each cached face's complete vertex/edge footprint and rejects any
/// old/current footprint change.
fn complete_proof_cache_edit_impact_v1(
    before_pattern: &CreasePattern,
    before_paper: &Paper,
    after_pattern: &CreasePattern,
    after_paper: &Paper,
    control: ProofCacheOperationControlV1<'_>,
) -> Option<CompleteProofCacheEditImpactV1> {
    complete_proof_cache_edit_impact_with_limit_v1(
        before_pattern,
        before_paper,
        after_pattern,
        after_paper,
        control,
        MAX_PROOF_CACHE_INVALIDATION_WORK_V1,
    )
}

fn complete_proof_cache_edit_impact_with_limit_v1(
    before_pattern: &CreasePattern,
    before_paper: &Paper,
    after_pattern: &CreasePattern,
    after_paper: &Paper,
    control: ProofCacheOperationControlV1<'_>,
    work_limit: usize,
) -> Option<CompleteProofCacheEditImpactV1> {
    control.check_v1().ok()?;
    let edge_capacity = before_pattern
        .edges
        .len()
        .checked_add(after_pattern.edges.len())?;
    let vertex_capacity = before_pattern
        .vertices
        .len()
        .checked_add(after_pattern.vertices.len())?
        .checked_add(before_paper.boundary_vertices.len())?
        .checked_add(after_paper.boundary_vertices.len())?
        .checked_add(edge_capacity.checked_mul(2)?)?;
    let mut preparation_work = bounded_edit_sort_work_v1(before_pattern.vertices.len())?
        .checked_add(bounded_edit_sort_work_v1(after_pattern.vertices.len())?)?
        .checked_add(bounded_edit_sort_work_v1(before_pattern.edges.len())?)?
        .checked_add(bounded_edit_sort_work_v1(after_pattern.edges.len())?)?
        .checked_add(vertex_capacity)?
        .checked_add(edge_capacity)?;
    if preparation_work > work_limit {
        return None;
    }

    let mut before_vertices = Vec::new();
    before_vertices
        .try_reserve_exact(before_pattern.vertices.len())
        .ok()?;
    before_vertices.extend(before_pattern.vertices.iter());
    let mut after_vertices = Vec::new();
    after_vertices
        .try_reserve_exact(after_pattern.vertices.len())
        .ok()?;
    after_vertices.extend(after_pattern.vertices.iter());
    let mut before_edges = Vec::new();
    before_edges
        .try_reserve_exact(before_pattern.edges.len())
        .ok()?;
    before_edges.extend(before_pattern.edges.iter());
    let mut after_edges = Vec::new();
    after_edges
        .try_reserve_exact(after_pattern.edges.len())
        .ok()?;
    after_edges.extend(after_pattern.edges.iter());
    control.check_v1().ok()?;
    before_vertices.sort_unstable_by_key(|vertex| vertex.id.canonical_bytes());
    control.check_v1().ok()?;
    after_vertices.sort_unstable_by_key(|vertex| vertex.id.canonical_bytes());
    control.check_v1().ok()?;
    before_edges.sort_unstable_by_key(|edge| edge.id.canonical_bytes());
    control.check_v1().ok()?;
    after_edges.sort_unstable_by_key(|edge| edge.id.canonical_bytes());
    if before_vertices
        .windows(2)
        .any(|pair| pair[0].id == pair[1].id)
        || after_vertices
            .windows(2)
            .any(|pair| pair[0].id == pair[1].id)
        || before_edges.windows(2).any(|pair| pair[0].id == pair[1].id)
        || after_edges.windows(2).any(|pair| pair[0].id == pair[1].id)
    {
        return None;
    }

    let mut vertices = Vec::new();
    vertices.try_reserve(vertex_capacity).ok()?;
    let mut edges = Vec::new();
    edges.try_reserve(edge_capacity).ok()?;
    let mut before_index = 0usize;
    let mut after_index = 0usize;
    while before_index < before_vertices.len() || after_index < after_vertices.len() {
        if (before_index + after_index).is_multiple_of(1024) {
            control.check_v1().ok()?;
        }
        match (
            before_vertices.get(before_index),
            after_vertices.get(after_index),
        ) {
            (Some(before), Some(after)) => {
                match before.id.canonical_bytes().cmp(&after.id.canonical_bytes()) {
                    std::cmp::Ordering::Less => {
                        vertices.push(before.id);
                        before_index += 1;
                    }
                    std::cmp::Ordering::Greater => {
                        vertices.push(after.id);
                        after_index += 1;
                    }
                    std::cmp::Ordering::Equal => {
                        if before.position.x.to_bits() != after.position.x.to_bits()
                            || before.position.y.to_bits() != after.position.y.to_bits()
                        {
                            vertices.push(before.id);
                        }
                        before_index += 1;
                        after_index += 1;
                    }
                }
            }
            (Some(before), None) => {
                vertices.push(before.id);
                before_index += 1;
            }
            (None, Some(after)) => {
                vertices.push(after.id);
                after_index += 1;
            }
            (None, None) => break,
        }
    }
    before_index = 0;
    after_index = 0;
    while before_index < before_edges.len() || after_index < after_edges.len() {
        if (before_index + after_index).is_multiple_of(1024) {
            control.check_v1().ok()?;
        }
        let mut changed = None;
        match (before_edges.get(before_index), after_edges.get(after_index)) {
            (Some(before), Some(after)) => {
                match before.id.canonical_bytes().cmp(&after.id.canonical_bytes()) {
                    std::cmp::Ordering::Less => {
                        changed = Some(*before);
                        before_index += 1;
                    }
                    std::cmp::Ordering::Greater => {
                        changed = Some(*after);
                        after_index += 1;
                    }
                    std::cmp::Ordering::Equal => {
                        if before.start != after.start
                            || before.end != after.end
                            || before.kind != after.kind
                        {
                            edges.push(before.id);
                            vertices.extend([before.start, before.end, after.start, after.end]);
                        }
                        before_index += 1;
                        after_index += 1;
                    }
                }
            }
            (Some(before), None) => {
                changed = Some(*before);
                before_index += 1;
            }
            (None, Some(after)) => {
                changed = Some(*after);
                after_index += 1;
            }
            (None, None) => break,
        }
        if let Some(edge) = changed {
            edges.push(edge.id);
            vertices.extend([edge.start, edge.end]);
        }
    }
    if before_paper.boundary_vertices != after_paper.boundary_vertices {
        vertices.extend_from_slice(&before_paper.boundary_vertices);
        vertices.extend_from_slice(&after_paper.boundary_vertices);
    }

    preparation_work = preparation_work
        .checked_add(bounded_edit_sort_work_v1(vertices.len())?)?
        .checked_add(bounded_edit_sort_work_v1(edges.len())?)?;
    if preparation_work > work_limit {
        return None;
    }
    control.check_v1().ok()?;
    vertices.sort_unstable_by_key(VertexId::canonical_bytes);
    vertices.dedup();
    control.check_v1().ok()?;
    edges.sort_unstable_by_key(EdgeId::canonical_bytes);
    edges.dedup();
    control.check_v1().ok()?;
    Some(CompleteProofCacheEditImpactV1 {
        vertices,
        edges,
        faces: Vec::new(),
        preparation_work,
    })
}

fn bounded_edit_sort_work_v1(item_count: usize) -> Option<usize> {
    let levels = if item_count <= 1 {
        0
    } else {
        usize::try_from(usize::BITS - (item_count - 1).leading_zeros()).ok()?
    };
    item_count.checked_mul(levels.checked_add(2)?)
}

pub(super) fn commit_editor_pose_and_proof_invalidation_v1(
    invalidation: PoseAuthorityInvalidation<'_>,
    source_revision: u64,
    before_pattern: &CreasePattern,
    before_paper: &Paper,
    project: &ProjectState,
) {
    let Some(impact) = complete_proof_cache_edit_impact_v1(
        before_pattern,
        before_paper,
        project.editor.pattern(),
        project.editor.paper(),
        ProofCacheOperationControlV1::new(None, Instant::now() + Duration::from_secs(30)),
    ) else {
        invalidation.commit();
        return;
    };
    invalidation.commit_with_complete_impact(
        source_revision,
        project.editor.revision(),
        impact.vertices,
        impact.edges,
        impact.faces,
        impact.preparation_work,
        ProofCacheOperationControlV1::new(None, Instant::now() + Duration::from_secs(30)),
    );
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicBool;

    use ori_domain::{Point2, ProjectId, Vertex};

    use super::*;

    fn single_vertex_transition_v1() -> (CreasePattern, CreasePattern, Paper) {
        let namespace = ProjectId::new();
        let vertex = VertexId::derive_v5(namespace, b"bounded-impact-vertex");
        (
            CreasePattern {
                vertices: vec![Vertex {
                    id: vertex,
                    position: Point2::new(0.0, 0.0),
                }],
                edges: Vec::new(),
            },
            CreasePattern {
                vertices: vec![Vertex {
                    id: vertex,
                    position: Point2::new(-0.0, 0.0),
                }],
                edges: Vec::new(),
            },
            Paper::default(),
        )
    }

    #[test]
    fn complete_impact_charges_final_sort_and_rejects_one_short() {
        let (before, after, paper) = single_vertex_transition_v1();
        let control =
            ProofCacheOperationControlV1::new(None, Instant::now() + Duration::from_secs(5));
        let impact = complete_proof_cache_edit_impact_with_limit_v1(
            &before,
            &paper,
            &after,
            &paper,
            control,
            MAX_PROOF_CACHE_INVALIDATION_WORK_V1,
        )
        .expect("bounded impact");
        assert_eq!(impact.vertices.len(), 1);
        assert!(impact.preparation_work > 0);
        assert!(
            complete_proof_cache_edit_impact_with_limit_v1(
                &before,
                &paper,
                &after,
                &paper,
                control,
                impact.preparation_work - 1,
            )
            .is_none()
        );
    }

    #[test]
    fn complete_impact_honours_cancellation_and_deadline() {
        let (before, after, paper) = single_vertex_transition_v1();
        let cancelled = AtomicBool::new(true);
        assert!(
            complete_proof_cache_edit_impact_v1(
                &before,
                &paper,
                &after,
                &paper,
                ProofCacheOperationControlV1::new(
                    Some(&cancelled),
                    Instant::now() + Duration::from_secs(5),
                ),
            )
            .is_none()
        );
        assert!(
            complete_proof_cache_edit_impact_v1(
                &before,
                &paper,
                &after,
                &paper,
                ProofCacheOperationControlV1::new(None, Instant::now()),
            )
            .is_none()
        );
    }
}
