use std::collections::BTreeMap;

use ori_domain::{CreasePattern, GeometricConstraintDocumentV1, GeometricConstraintKindV1, Point2};

use crate::{
    Binary64ExactConstraintSatisfactionV1, ConstraintPreflightV1, ConstraintSolvePreviewV1,
    GeometricConstraintLimitsV1, certify_binary64_exact_geometric_constraint_satisfaction_v1,
    prepare_geometric_constraints_v1,
};

mod length_constructive;
mod pair_constructive;
mod singleton_constructive;
mod zero_closure_constructive;

pub(crate) use length_constructive::{
    MAX_LENGTH_CONSTRAINT_CONSTRUCTIVE_CONSTRAINTS_V1, MAX_LENGTH_CONSTRAINT_CONSTRUCTIVE_EDGES_V1,
    construct_length_constraint_residual_exact_assignment_v1,
};
pub(crate) use pair_constructive::{
    MAX_PAIR_CONSTRAINT_ALGEBRAIC_CANDIDATES_V1, MAX_PAIR_CONSTRAINT_CONSTRUCTIVE_CANDIDATES_V1,
    construct_pair_constraint_algebraic_exact_assignment_v1,
    construct_pair_constraint_exact_assignment_v1,
};
pub(crate) use singleton_constructive::MAX_SINGLE_CONSTRAINT_CONSTRUCTIVE_CANDIDATES_V1;
pub use singleton_constructive::construct_single_constraint_exact_assignment_v1;
pub(crate) use zero_closure_constructive::{
    construct_zero_length_closure_residual_exact_assignment_v1,
    zero_length_closure_constructive_candidate_bound_v1,
};

/// Explicit assignment obtained from a bounded native construction or
/// numerical preview and independently re-certified in the complete
/// deterministic binary64 residual language.
///
/// The assignment is observation-only. It is not bound to a project or
/// revision and does not authorize mutation. Cross-runtime re-certification is
/// advertised only on targets covered by the frozen transcendental model.
#[derive(Debug, Clone)]
pub struct CurrentRuntimeExactConstraintAssignmentV1 {
    pattern: CreasePattern,
    certificate: Binary64ExactConstraintSatisfactionV1,
}

impl CurrentRuntimeExactConstraintAssignmentV1 {
    #[must_use]
    pub const fn model_id(&self) -> &'static str {
        self.certificate.model_id()
    }

    #[must_use]
    pub const fn transcendental_model_id(&self) -> &'static str {
        self.certificate.transcendental_model_id()
    }

    #[must_use]
    pub const fn authorizes_project_mutation(&self) -> bool {
        self.certificate.authorizes_project_mutation()
    }

    #[must_use]
    pub const fn replayable_across_runtimes(&self) -> bool {
        self.certificate.replayable_across_runtimes()
    }

    /// Returns the complete candidate pattern whose positions were certified.
    #[must_use]
    pub const fn pattern(&self) -> &CreasePattern {
        &self.pattern
    }

    #[must_use]
    pub const fn certificate(&self) -> Binary64ExactConstraintSatisfactionV1 {
        self.certificate
    }
}

/// Attempts to turn a bounded numerical solve preview into an explicit exact
/// assignment without trusting its tolerance, diagnostics, or claimed rank.
///
/// Preview positions must be finite, unique, and refer to existing vertices.
/// The candidate is then projected only along the exact equivalence relations
/// implied by `Horizontal` (equal endpoint Y) and `Vertical` (equal endpoint
/// X) constraints. Each equivalence class copies the coordinate of its
/// canonical minimum vertex, making the operation independent of storage and
/// constraint order.
///
/// The projected assignment is returned only after the ordinary complete
/// certificate revalidates the document and every production residual,
/// including all non-axis constraint kinds, as finite binary64 zero. `None`
/// is therefore returned for invalid input, a proven direct conflict,
/// projection collapse, or any nonzero residual, and is not itself evidence
/// of unsatisfiability.
#[must_use]
pub fn exactify_axis_aligned_constraint_preview_v1(
    pattern: &CreasePattern,
    document: &GeometricConstraintDocumentV1,
    preview: &ConstraintSolvePreviewV1,
) -> Option<CurrentRuntimeExactConstraintAssignmentV1> {
    if document.constraints.is_empty() {
        return None;
    }
    let prepared =
        prepare_geometric_constraints_v1(pattern, document, GeometricConstraintLimitsV1::default())
            .ok()?;
    if matches!(
        prepared.preflight(),
        ConstraintPreflightV1::DirectConflict { .. }
    ) || preview.positions.len() > pattern.vertices.len()
    {
        return None;
    }

    let mut candidate = pattern.clone();
    let mut ordered_vertices = candidate
        .vertices
        .iter()
        .enumerate()
        .map(|(index, vertex)| (vertex.id.canonical_bytes(), index))
        .collect::<Vec<_>>();
    ordered_vertices.sort_unstable_by_key(|(id, _)| *id);

    let vertex_ordinals = ordered_vertices
        .iter()
        .enumerate()
        .map(|(ordinal, (id, _))| (*id, ordinal))
        .collect::<BTreeMap<_, _>>();
    let candidate_indices = ordered_vertices
        .iter()
        .map(|(id, index)| (*id, *index))
        .collect::<BTreeMap<_, _>>();

    let mut updates = BTreeMap::new();
    for (vertex, point) in &preview.positions {
        if !point.x.is_finite()
            || !point.y.is_finite()
            || updates
                .insert(vertex.canonical_bytes(), (*vertex, *point))
                .is_some()
        {
            return None;
        }
    }
    for (id, (vertex, point)) in updates {
        let index = *candidate_indices.get(&id)?;
        if candidate.vertices[index].id != vertex {
            return None;
        }
        candidate.vertices[index].position = point;
    }

    let mut edge_endpoints = BTreeMap::new();
    for edge in &candidate.edges {
        let endpoints = (
            *vertex_ordinals.get(&edge.start.canonical_bytes())?,
            *vertex_ordinals.get(&edge.end.canonical_bytes())?,
        );
        if edge_endpoints
            .insert(edge.id.canonical_bytes(), endpoints)
            .is_some()
        {
            return None;
        }
    }

    let mut x_classes = CanonicalDisjointSet::new(candidate.vertices.len());
    let mut y_classes = CanonicalDisjointSet::new(candidate.vertices.len());
    for record in &document.constraints {
        let (edge, classes) = match record.constraint {
            GeometricConstraintKindV1::Horizontal { edge } => (edge, &mut y_classes),
            GeometricConstraintKindV1::Vertical { edge } => (edge, &mut x_classes),
            _ => continue,
        };
        let &(start, end) = edge_endpoints.get(&edge.canonical_bytes())?;
        classes.union(start, end);
    }

    let original_points = ordered_vertices
        .iter()
        .map(|(_, index)| candidate.vertices[*index].position)
        .collect::<Vec<Point2>>();
    for (ordinal, (_, index)) in ordered_vertices.iter().enumerate() {
        let x = original_points[x_classes.find(ordinal)].x;
        let y = original_points[y_classes.find(ordinal)].y;
        candidate.vertices[*index].position = Point2::new(x, y);
    }

    let certificate =
        certify_binary64_exact_geometric_constraint_satisfaction_v1(&candidate, document)
            .ok()
            .flatten()?;
    Some(CurrentRuntimeExactConstraintAssignmentV1 {
        pattern: candidate,
        certificate,
    })
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

#[cfg(test)]
#[path = "constraint_exactification/pair_constructive_tests.rs"]
mod pair_constructive_tests;

#[cfg(test)]
#[path = "constraint_exactification/pair_constructive_cardinal_rotation_tests.rs"]
mod pair_constructive_cardinal_rotation_tests;

#[cfg(test)]
#[path = "constraint_exactification/pair_constructive_algebraic_tests.rs"]
mod pair_constructive_algebraic_tests;

#[cfg(test)]
#[path = "constraint_exactification/length_constructive_tests.rs"]
mod length_constructive_tests;

#[cfg(test)]
#[path = "constraint_exactification/zero_closure_constructive_tests.rs"]
mod zero_closure_constructive_tests;

#[cfg(test)]
#[path = "constraint_singleton_constructive_tests.rs"]
mod singleton_constructive_tests;
