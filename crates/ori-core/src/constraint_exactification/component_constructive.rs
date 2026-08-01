use std::collections::{BTreeMap, BTreeSet};

use ori_domain::{CreasePattern, GeometricConstraintDocumentV1, Point2, VertexId};

use super::{
    CurrentRuntimeExactConstraintAssignmentV1,
    pair_constructive::construct_pair_constraint_exact_assignment_v1,
    singleton_constructive::{
        MAX_BOUNDED_SINGLETON_COMPOSITION_CONSTRAINTS_V1,
        construct_single_constraint_exact_assignment_v1,
    },
    three_record_component_constructive::construct_pair_plus_singleton_leaf_exact_assignment_v1,
};
use crate::{
    ConstraintPreflightV1, GeometricConstraintLimitsV1,
    certify_binary64_exact_geometric_constraint_satisfaction_v1,
    constraint_solver::residual_referenced_vertices_by_record_v1, prepare_geometric_constraints_v1,
};

#[cfg(test)]
pub(super) const MAX_BOUNDED_COMPONENT_PREPARATION_OR_VERIFICATION_PASSES_V1: usize = 138;
#[cfg(test)]
pub(super) const MAX_BOUNDED_COMPONENT_FULL_PATTERN_CLONES_V1: usize = 112;

/// Constructs the exact-count bounded assignment used by the public
/// singleton-composition entry points.
///
/// Records are first grouped by the complete vertex set observed by their
/// production residual, including both endpoints of every referenced edge.
/// Components and records are processed in canonical constraint-ID order.
/// One-record components use the existing singleton constructor. Multi-record
/// components first retain the original bit-identical singleton merge; an
/// exactly two-record component may additionally use the existing ordinary
/// crease-pattern pair constructor when the singleton merge is incompatible.
/// An exactly three-record component may use the narrow ordinary-pair plus
/// single-articulation singleton-leaf constructor.
///
/// Each completed component is assembled on the progressively certified
/// candidate. This makes every later component constructor validate all
/// topology already changed by earlier components, including unreferenced
/// connector edges that could otherwise collapse under a fixed translation.
/// A final full-document exact certificate remains the sole authority that can
/// escape.
///
/// For `N <= 16`, `P` two-record components, and `T` admitted three-record
/// components, the conservative worst case remains at most 138 bounded
/// preparation-or-verification passes and 112 full-pattern clones:
///
/// - `N` singleton attempts, each with one preparation and at most four
///   verifier-backed candidates (`5N` passes and `4N` clones);
/// - `P` pair attempts, each with one preparation and at most five
///   verifier-backed candidates (`6P` passes and `5P` clones);
/// - `T` three-record attempts, each with one preparation and at most four
///   verifier-backed candidates (`5T` passes and `4T` clones);
/// - at most eight multi-singleton component merge verifications/clones; and
/// - one whole-document preparation plus one final full verification.
///
/// Since `2P + 3T <= N`, a two-record component remains the dominant cost per
/// record. The special two-record whole-document pair prepass is never retried
/// as a component pair, so it remains below the same global envelope. No
/// residual-only algebraic overlay participates in this crease-pattern path.
pub(super) fn construct_bounded_component_exact_assignment_v1(
    pattern: &CreasePattern,
    document: &GeometricConstraintDocumentV1,
    expected_constraint_count: usize,
) -> Option<CurrentRuntimeExactConstraintAssignmentV1> {
    if document.constraints.len() != expected_constraint_count
        || !(2..=MAX_BOUNDED_SINGLETON_COMPOSITION_CONSTRAINTS_V1)
            .contains(&expected_constraint_count)
    {
        return None;
    }

    // Preserve the established wider two-record prepass and its exact output.
    // A failed prepass is not repeated below when both records form one
    // component.
    if expected_constraint_count == 2
        && let Some(assignment) = construct_pair_constraint_exact_assignment_v1(pattern, document)
    {
        return Some(assignment);
    }

    let prepared =
        prepare_geometric_constraints_v1(pattern, document, GeometricConstraintLimitsV1::default())
            .ok()?;
    if matches!(
        prepared.preflight(),
        ConstraintPreflightV1::DirectConflict { .. }
    ) {
        return None;
    }

    let ordered_document = canonical_raw_document(document);
    let referenced_by_record =
        residual_referenced_vertices_by_record_v1(pattern, &ordered_document).ok()?;
    let components = canonical_components(&ordered_document, &referenced_by_record)?;
    let only_component_is_two_record_prepass =
        expected_constraint_count == 2 && components.len() == 1;

    let mut progressive_candidate = None;
    for component in &components {
        let base = progressive_candidate.as_ref().unwrap_or(pattern);
        let component_document = component.document(&ordered_document);
        let completed = if component.record_indices.len() == 1 {
            let assignment =
                construct_single_constraint_exact_assignment_v1(base, &component_document)?;
            let candidate = assignment.into_pattern();
            candidate_changes_only(base, &candidate, &component.referenced_vertices)
                .then_some(candidate)?
        } else {
            construct_component_from_singletons(
                base,
                &ordered_document,
                &referenced_by_record,
                component,
            )
            .or_else(|| {
                (component.record_indices.len() == 3)
                    .then(|| {
                        construct_pair_plus_singleton_leaf_exact_assignment_v1(
                            base,
                            &component_document,
                        )
                    })
                    .flatten()
                    .map(CurrentRuntimeExactConstraintAssignmentV1::into_pattern)
                    .filter(|candidate| {
                        candidate_changes_only(base, candidate, &component.referenced_vertices)
                    })
            })
            .or_else(|| {
                (component.record_indices.len() == 2 && !only_component_is_two_record_prepass)
                    .then(|| {
                        construct_pair_constraint_exact_assignment_v1(base, &component_document)
                    })
                    .flatten()
                    .map(CurrentRuntimeExactConstraintAssignmentV1::into_pattern)
                    .filter(|candidate| {
                        candidate_changes_only(base, candidate, &component.referenced_vertices)
                    })
            })?
        };
        progressive_candidate = Some(completed);
    }

    let candidate = progressive_candidate?;
    let certificate =
        certify_binary64_exact_geometric_constraint_satisfaction_v1(&candidate, document)
            .ok()
            .flatten()?;
    Some(CurrentRuntimeExactConstraintAssignmentV1 {
        pattern: candidate,
        certificate,
    })
}

fn canonical_raw_document(
    document: &GeometricConstraintDocumentV1,
) -> GeometricConstraintDocumentV1 {
    let mut constraints = document.constraints.clone();
    constraints.sort_unstable_by_key(|record| record.id.canonical_bytes());
    GeometricConstraintDocumentV1 {
        schema_version: document.schema_version,
        constraints,
    }
}

struct CanonicalComponent {
    record_indices: Vec<usize>,
    referenced_vertices: BTreeSet<[u8; 16]>,
}

impl CanonicalComponent {
    fn document(
        &self,
        ordered_document: &GeometricConstraintDocumentV1,
    ) -> GeometricConstraintDocumentV1 {
        GeometricConstraintDocumentV1 {
            schema_version: ordered_document.schema_version,
            constraints: self
                .record_indices
                .iter()
                .map(|index| ordered_document.constraints[*index].clone())
                .collect(),
        }
    }
}

fn canonical_components(
    ordered_document: &GeometricConstraintDocumentV1,
    referenced_by_record: &[Vec<VertexId>],
) -> Option<Vec<CanonicalComponent>> {
    if ordered_document.constraints.len() != referenced_by_record.len()
        || ordered_document.constraints.is_empty()
    {
        return None;
    }

    let mut disjoint_set = CanonicalDisjointSet::new(ordered_document.constraints.len());
    let mut first_record_by_vertex = BTreeMap::new();
    for (record_index, vertices) in referenced_by_record.iter().enumerate() {
        if vertices.is_empty() {
            return None;
        }
        for vertex in vertices {
            let key = vertex.canonical_bytes();
            match first_record_by_vertex.get(&key) {
                Some((stored_vertex, first_record)) => {
                    if *stored_vertex != *vertex {
                        return None;
                    }
                    disjoint_set.union(record_index, *first_record);
                }
                None => {
                    first_record_by_vertex.insert(key, (*vertex, record_index));
                }
            }
        }
    }

    let mut indices_by_root = BTreeMap::<usize, Vec<usize>>::new();
    for record_index in 0..ordered_document.constraints.len() {
        let root = disjoint_set.find(record_index);
        indices_by_root.entry(root).or_default().push(record_index);
    }

    indices_by_root
        .into_values()
        .map(|record_indices| {
            let referenced_vertices = record_indices
                .iter()
                .flat_map(|index| &referenced_by_record[*index])
                .map(VertexId::canonical_bytes)
                .collect::<BTreeSet<_>>();
            (!referenced_vertices.is_empty()).then_some(CanonicalComponent {
                record_indices,
                referenced_vertices,
            })
        })
        .collect()
}

fn construct_component_from_singletons(
    base: &CreasePattern,
    ordered_document: &GeometricConstraintDocumentV1,
    referenced_by_record: &[Vec<VertexId>],
    component: &CanonicalComponent,
) -> Option<CreasePattern> {
    let mut merged = BTreeMap::<[u8; 16], (VertexId, Point2)>::new();
    for record_index in &component.record_indices {
        let record = ordered_document.constraints.get(*record_index)?.clone();
        let singleton_document = GeometricConstraintDocumentV1 {
            schema_version: ordered_document.schema_version,
            constraints: vec![record],
        };
        let assignment =
            construct_single_constraint_exact_assignment_v1(base, &singleton_document)?;
        let referenced = referenced_by_record.get(*record_index)?;
        let allowed = referenced
            .iter()
            .map(VertexId::canonical_bytes)
            .collect::<BTreeSet<_>>();
        if !candidate_changes_only(base, assignment.pattern(), &allowed) {
            return None;
        }

        for vertex in referenced {
            let assigned = assignment
                .pattern()
                .vertices
                .iter()
                .find(|candidate| candidate.id == *vertex)?;
            let key = vertex.canonical_bytes();
            match merged.get(&key) {
                Some((stored_vertex, stored_point))
                    if *stored_vertex != *vertex
                        || !point_bits_equal(*stored_point, assigned.position) =>
                {
                    return None;
                }
                Some(_) => {}
                None => {
                    merged.insert(key, (*vertex, assigned.position));
                }
            }
        }
    }

    let mut candidate = base.clone();
    let mut applied = 0usize;
    for vertex in &mut candidate.vertices {
        let Some((stored_vertex, point)) = merged.get(&vertex.id.canonical_bytes()) else {
            continue;
        };
        if *stored_vertex != vertex.id {
            return None;
        }
        vertex.position = *point;
        applied = applied.checked_add(1)?;
    }
    if applied != merged.len()
        || !candidate_changes_only(base, &candidate, &component.referenced_vertices)
    {
        return None;
    }

    let component_document = component.document(ordered_document);
    certify_binary64_exact_geometric_constraint_satisfaction_v1(&candidate, &component_document)
        .ok()
        .flatten()
        .map(|_| candidate)
}

fn candidate_changes_only(
    base: &CreasePattern,
    candidate: &CreasePattern,
    allowed_vertices: &BTreeSet<[u8; 16]>,
) -> bool {
    if base.edges != candidate.edges || base.vertices.len() != candidate.vertices.len() {
        return false;
    }
    base.vertices
        .iter()
        .zip(&candidate.vertices)
        .all(|(before, after)| {
            before.id == after.id
                && (allowed_vertices.contains(&before.id.canonical_bytes())
                    || point_bits_equal(before.position, after.position))
        })
}

fn point_bits_equal(left: Point2, right: Point2) -> bool {
    left.x.to_bits() == right.x.to_bits() && left.y.to_bits() == right.y.to_bits()
}

#[derive(Debug)]
struct CanonicalDisjointSet {
    parents: Vec<usize>,
}

impl CanonicalDisjointSet {
    fn new(size: usize) -> Self {
        Self {
            parents: (0..size).collect(),
        }
    }

    fn find(&mut self, value: usize) -> usize {
        let parent = self.parents[value];
        if parent == value {
            value
        } else {
            let root = self.find(parent);
            self.parents[value] = root;
            root
        }
    }

    fn union(&mut self, left: usize, right: usize) {
        let left = self.find(left);
        let right = self.find(right);
        let (minimum, maximum) = if left <= right {
            (left, right)
        } else {
            (right, left)
        };
        self.parents[maximum] = minimum;
    }
}
