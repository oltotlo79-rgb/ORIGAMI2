//! Read-only material-component classification for closed cuts.
//!
//! The project/editor/history schemas have no persistent component keep/remove
//! selection. This module therefore exposes observations only and deliberately
//! provides no transaction or mutation conversion.

use std::collections::{HashMap, HashSet};

use ori_domain::{EdgeId, FaceId};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    ClosedCutLoopDiagnosticLimitsV1, ClosedCutTopologySnapshotErrorV1, EdgeIncidence,
    FaceExtractionInput, MaterialComponentKey, diagnose_closed_cut_topology_snapshot_v1,
};

pub const CUT_MATERIAL_COMPONENT_SELECTION_DIAGNOSTIC_MODEL_ID_V1: &str =
    "cut_material_component_selection_diagnostic_v1";
pub const CUT_MATERIAL_REMOVAL_PLAN_DIAGNOSTIC_MODEL_ID_V1: &str =
    "cut_material_removal_plan_diagnostic_v1";

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CutMaterialComponentSelectionErrorV1 {
    #[error("closed-cut topology prerequisite failed: {0}")]
    Topology(#[from] ClosedCutTopologySnapshotErrorV1),
    #[error("material-component selection classification failed closed")]
    InvalidClassification,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CutMaterialRemovalPlanErrorV1 {
    #[error("material-component selection prerequisite failed: {0}")]
    Selection(#[from] CutMaterialComponentSelectionErrorV1),
    #[error("requested component keys must be non-empty and strictly canonical")]
    InvalidRequest,
    #[error("requested component does not exist")]
    UnknownComponent,
    #[error("the original-boundary component cannot be removed")]
    BoundaryComponentRequested,
    #[error("cut-incidence component graph is not a rooted tree")]
    InvalidComponentGraph,
    #[error("material removal partition failed closed")]
    InvalidPartition,
}

/// A read-only plan for a future material-removal transaction.
///
/// This diagnostic deliberately cannot be converted into a mutation. Removing
/// a component implies removing its complete descendant closure away from the
/// unique original-boundary owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CutMaterialRemovalPlanDiagnosticV1 {
    requested_components: Vec<MaterialComponentKey>,
    removed_components: Vec<MaterialComponentKey>,
    retained_components: Vec<MaterialComponentKey>,
    boundary_component: MaterialComponentKey,
    removed_faces: Vec<FaceId>,
    retained_faces: Vec<FaceId>,
    crossing_cut_boundaries: Vec<EdgeId>,
    fingerprint: [u8; 32],
    limits: ClosedCutLoopDiagnosticLimitsV1,
}

impl CutMaterialRemovalPlanDiagnosticV1 {
    #[must_use]
    pub const fn model_id(&self) -> &'static str {
        CUT_MATERIAL_REMOVAL_PLAN_DIAGNOSTIC_MODEL_ID_V1
    }
    #[must_use]
    pub fn requested_components(&self) -> &[MaterialComponentKey] {
        &self.requested_components
    }
    #[must_use]
    pub fn removed_components(&self) -> &[MaterialComponentKey] {
        &self.removed_components
    }
    #[must_use]
    pub fn retained_components(&self) -> &[MaterialComponentKey] {
        &self.retained_components
    }
    #[must_use]
    pub const fn boundary_component(&self) -> MaterialComponentKey {
        self.boundary_component
    }
    #[must_use]
    pub fn removed_faces(&self) -> &[FaceId] {
        &self.removed_faces
    }
    #[must_use]
    pub fn retained_faces(&self) -> &[FaceId] {
        &self.retained_faces
    }
    #[must_use]
    pub fn crossing_cut_boundaries(&self) -> &[EdgeId] {
        &self.crossing_cut_boundaries
    }
    #[must_use]
    pub const fn fingerprint_v1(&self) -> [u8; 32] {
        self.fingerprint
    }
    #[must_use]
    pub const fn authorizes_project_mutation(&self) -> bool {
        false
    }
    #[must_use]
    pub const fn authorizes_material_removal(&self) -> bool {
        false
    }
    #[must_use]
    pub const fn authorizes_simulation_admission(&self) -> bool {
        false
    }
    #[must_use]
    pub fn is_for(
        &self,
        input: FaceExtractionInput<'_>,
        requested_components: &[MaterialComponentKey],
    ) -> bool {
        diagnose_cut_material_removal_plan_v1(input, requested_components, self.limits)
            .is_ok_and(|current| current.fingerprint == self.fingerprint)
    }
}

/// Canonical, non-authoritative classification of one material component.
///
/// `owns_original_boundary` is a factual topology observation. Such a component
/// must never be removed. A component without the boundary is only a possible
/// removal candidate; neither value grants deletion authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CutMaterialComponentSelectionV1 {
    pub component: MaterialComponentKey,
    pub faces: Vec<FaceId>,
    pub owns_original_boundary: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CutMaterialComponentSelectionDiagnosticV1 {
    selections: Vec<CutMaterialComponentSelectionV1>,
    fingerprint: [u8; 32],
    limits: ClosedCutLoopDiagnosticLimitsV1,
}

impl CutMaterialComponentSelectionDiagnosticV1 {
    #[must_use]
    pub const fn model_id(&self) -> &'static str {
        CUT_MATERIAL_COMPONENT_SELECTION_DIAGNOSTIC_MODEL_ID_V1
    }

    #[must_use]
    pub fn selections(&self) -> &[CutMaterialComponentSelectionV1] {
        &self.selections
    }

    #[must_use]
    pub const fn fingerprint_v1(&self) -> [u8; 32] {
        self.fingerprint
    }

    #[must_use]
    pub const fn authorizes_project_mutation(&self) -> bool {
        false
    }

    #[must_use]
    pub const fn authorizes_material_removal(&self) -> bool {
        false
    }

    #[must_use]
    pub const fn authorizes_simulation_admission(&self) -> bool {
        false
    }

    #[must_use]
    pub fn is_for(&self, input: FaceExtractionInput<'_>) -> bool {
        diagnose_cut_material_component_selection_v1(input, self.limits)
            .is_ok_and(|current| current.fingerprint == self.fingerprint)
    }
}

pub fn diagnose_cut_material_component_selection_v1(
    input: FaceExtractionInput<'_>,
    limits: ClosedCutLoopDiagnosticLimitsV1,
) -> Result<CutMaterialComponentSelectionDiagnosticV1, CutMaterialComponentSelectionErrorV1> {
    let topology = diagnose_closed_cut_topology_snapshot_v1(input, limits)?;
    let snapshot = topology.snapshot();
    let paper_boundary_vertices = input
        .paper
        .boundary_vertices
        .iter()
        .copied()
        .collect::<HashSet<_>>();
    if input.pattern.edges.iter().any(|edge| {
        edge.kind == ori_domain::EdgeKind::Cut
            && (paper_boundary_vertices.contains(&edge.start)
                || paper_boundary_vertices.contains(&edge.end))
    }) {
        return Err(CutMaterialComponentSelectionErrorV1::InvalidClassification);
    }
    let face_ids = snapshot
        .faces
        .iter()
        .map(|face| face.id)
        .collect::<HashSet<_>>();
    let mut component_by_face = HashMap::with_capacity(face_ids.len());
    let mut component_keys = HashSet::with_capacity(snapshot.material_components.len());
    for component in &snapshot.material_components {
        if component.sheet_origin != input.identity_namespace
            || !component_keys.insert(component.key)
            || component.faces.is_empty()
        {
            return Err(CutMaterialComponentSelectionErrorV1::InvalidClassification);
        }
        for face in &component.faces {
            if !face_ids.contains(face) || component_by_face.insert(*face, component.key).is_some()
            {
                return Err(CutMaterialComponentSelectionErrorV1::InvalidClassification);
            }
        }
    }
    if component_by_face.len() != face_ids.len() {
        return Err(CutMaterialComponentSelectionErrorV1::InvalidClassification);
    }

    let boundary_edges = input
        .pattern
        .edges
        .iter()
        .filter(|edge| edge.kind == ori_domain::EdgeKind::Boundary)
        .map(|edge| edge.id)
        .collect::<HashSet<_>>();
    let mut boundary_components = HashSet::new();
    let mut observed_boundary_edges = HashSet::new();
    for (edge, incidence) in &snapshot.edge_incidence {
        if let EdgeIncidence::Boundary { material } = incidence {
            if !boundary_edges.contains(edge)
                || !observed_boundary_edges.insert(*edge)
                || !face_ids.contains(material)
            {
                return Err(CutMaterialComponentSelectionErrorV1::InvalidClassification);
            }
            boundary_components.insert(component_by_face[material]);
        }
    }
    if observed_boundary_edges != boundary_edges || boundary_components.len() != 1 {
        return Err(CutMaterialComponentSelectionErrorV1::InvalidClassification);
    }

    let mut selections = snapshot
        .material_components
        .iter()
        .map(|component| {
            let mut faces = component.faces.clone();
            faces.sort_unstable_by_key(FaceId::canonical_bytes);
            CutMaterialComponentSelectionV1 {
                component: component.key,
                faces,
                owns_original_boundary: boundary_components.contains(&component.key),
            }
        })
        .collect::<Vec<_>>();
    selections.sort_unstable_by_key(|selection| selection.component);
    if selections
        .iter()
        .all(|selection| selection.owns_original_boundary)
    {
        return Err(CutMaterialComponentSelectionErrorV1::InvalidClassification);
    }

    let mut hash = Sha256::new();
    hash.update(CUT_MATERIAL_COMPONENT_SELECTION_DIAGNOSTIC_MODEL_ID_V1.as_bytes());
    hash.update(topology.fingerprint_v1());
    hash.update((selections.len() as u64).to_be_bytes());
    for selection in &selections {
        hash.update(selection.component.0);
        hash.update([u8::from(selection.owns_original_boundary)]);
        hash.update((selection.faces.len() as u64).to_be_bytes());
        for face in &selection.faces {
            hash.update(face.canonical_bytes());
        }
    }
    Ok(CutMaterialComponentSelectionDiagnosticV1 {
        selections,
        fingerprint: hash.finalize().into(),
        limits,
    })
}

pub fn diagnose_cut_material_removal_plan_v1(
    input: FaceExtractionInput<'_>,
    requested_components: &[MaterialComponentKey],
    limits: ClosedCutLoopDiagnosticLimitsV1,
) -> Result<CutMaterialRemovalPlanDiagnosticV1, CutMaterialRemovalPlanErrorV1> {
    if requested_components.is_empty()
        || requested_components
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
    {
        return Err(CutMaterialRemovalPlanErrorV1::InvalidRequest);
    }

    let selection = diagnose_cut_material_component_selection_v1(input, limits)?;
    let topology = diagnose_closed_cut_topology_snapshot_v1(input, limits)
        .map_err(CutMaterialComponentSelectionErrorV1::from)
        .map_err(CutMaterialRemovalPlanErrorV1::from)?;
    let snapshot = topology.snapshot();
    let boundary_components = selection
        .selections()
        .iter()
        .filter(|entry| entry.owns_original_boundary)
        .map(|entry| entry.component)
        .collect::<Vec<_>>();
    if boundary_components.len() != 1 {
        return Err(CutMaterialRemovalPlanErrorV1::InvalidComponentGraph);
    }
    let root = boundary_components[0];
    let known = selection
        .selections()
        .iter()
        .map(|entry| entry.component)
        .collect::<HashSet<_>>();
    for requested in requested_components {
        if !known.contains(requested) {
            return Err(CutMaterialRemovalPlanErrorV1::UnknownComponent);
        }
        if *requested == root {
            return Err(CutMaterialRemovalPlanErrorV1::BoundaryComponentRequested);
        }
    }

    let component_by_face = selection
        .selections()
        .iter()
        .flat_map(|entry| entry.faces.iter().map(move |face| (*face, entry.component)))
        .collect::<HashMap<_, _>>();
    let mut cut_pairs = HashSet::new();
    let mut cut_incidence = Vec::new();
    for (edge, incidence) in &snapshot.edge_incidence {
        if let EdgeIncidence::Cut { left, right } = incidence {
            let left = *component_by_face
                .get(left)
                .ok_or(CutMaterialRemovalPlanErrorV1::InvalidComponentGraph)?;
            let right = *component_by_face
                .get(right)
                .ok_or(CutMaterialRemovalPlanErrorV1::InvalidComponentGraph)?;
            if left == right {
                return Err(CutMaterialRemovalPlanErrorV1::InvalidComponentGraph);
            }
            let pair = if left < right {
                (left, right)
            } else {
                (right, left)
            };
            cut_pairs.insert(pair);
            cut_incidence.push((*edge, left, right));
        }
    }
    if cut_pairs.len() != known.len().saturating_sub(1) {
        return Err(CutMaterialRemovalPlanErrorV1::InvalidComponentGraph);
    }
    let mut adjacency = HashMap::<MaterialComponentKey, Vec<MaterialComponentKey>>::new();
    for component in &known {
        adjacency.insert(*component, Vec::new());
    }
    for (left, right) in cut_pairs {
        adjacency.get_mut(&left).unwrap().push(right);
        adjacency.get_mut(&right).unwrap().push(left);
    }
    for neighbors in adjacency.values_mut() {
        neighbors.sort_unstable();
    }

    let mut parent = HashMap::with_capacity(known.len());
    parent.insert(root, root);
    let mut queue = std::collections::VecDeque::from([root]);
    while let Some(component) = queue.pop_front() {
        for neighbor in &adjacency[&component] {
            if !parent.contains_key(neighbor) {
                parent.insert(*neighbor, component);
                queue.push_back(*neighbor);
            }
        }
    }
    if parent.len() != known.len() {
        return Err(CutMaterialRemovalPlanErrorV1::InvalidComponentGraph);
    }

    let requested = requested_components.iter().copied().collect::<HashSet<_>>();
    let mut removed = HashSet::new();
    for component in known.iter().copied().filter(|component| *component != root) {
        let mut cursor = component;
        loop {
            if requested.contains(&cursor) {
                removed.insert(component);
                break;
            }
            let next = parent[&cursor];
            if next == cursor {
                break;
            }
            cursor = next;
        }
    }
    if removed.is_empty() || removed.contains(&root) {
        return Err(CutMaterialRemovalPlanErrorV1::InvalidPartition);
    }
    let mut removed_components = removed.iter().copied().collect::<Vec<_>>();
    removed_components.sort_unstable();
    let mut retained_components = known
        .iter()
        .copied()
        .filter(|component| !removed.contains(component))
        .collect::<Vec<_>>();
    retained_components.sort_unstable();
    if retained_components.len() + removed_components.len() != known.len()
        || !retained_components.contains(&root)
    {
        return Err(CutMaterialRemovalPlanErrorV1::InvalidPartition);
    }
    let mut removed_faces = Vec::new();
    let mut retained_faces = Vec::new();
    for entry in selection.selections() {
        if removed.contains(&entry.component) {
            removed_faces.extend_from_slice(&entry.faces);
        } else {
            retained_faces.extend_from_slice(&entry.faces);
        }
    }
    removed_faces.sort_unstable_by_key(FaceId::canonical_bytes);
    retained_faces.sort_unstable_by_key(FaceId::canonical_bytes);
    if removed_faces.is_empty()
        || retained_faces.is_empty()
        || removed_faces.iter().any(|face| {
            retained_faces
                .binary_search_by_key(&face.canonical_bytes(), FaceId::canonical_bytes)
                .is_ok()
        })
        || removed_faces.len() + retained_faces.len() != snapshot.faces.len()
    {
        return Err(CutMaterialRemovalPlanErrorV1::InvalidPartition);
    }
    let mut crossing_cut_boundaries = cut_incidence
        .into_iter()
        .filter_map(|(edge, left, right)| {
            (removed.contains(&left) != removed.contains(&right)).then_some(edge)
        })
        .collect::<Vec<_>>();
    crossing_cut_boundaries.sort_unstable_by_key(EdgeId::canonical_bytes);
    if crossing_cut_boundaries.is_empty() {
        return Err(CutMaterialRemovalPlanErrorV1::InvalidPartition);
    }

    let mut hash = Sha256::new();
    hash.update(CUT_MATERIAL_REMOVAL_PLAN_DIAGNOSTIC_MODEL_ID_V1.as_bytes());
    hash.update(selection.fingerprint_v1());
    hash.update(root.0);
    hash.update((requested_components.len() as u64).to_be_bytes());
    for component in requested_components {
        hash.update(component.0);
    }
    hash.update((removed_components.len() as u64).to_be_bytes());
    for component in &removed_components {
        hash.update(component.0);
    }
    hash.update((retained_components.len() as u64).to_be_bytes());
    for component in &retained_components {
        hash.update(component.0);
    }
    hash.update((removed_faces.len() as u64).to_be_bytes());
    for face in &removed_faces {
        hash.update(face.canonical_bytes());
    }
    hash.update((retained_faces.len() as u64).to_be_bytes());
    for face in &retained_faces {
        hash.update(face.canonical_bytes());
    }
    hash.update((crossing_cut_boundaries.len() as u64).to_be_bytes());
    for edge in &crossing_cut_boundaries {
        hash.update(edge.canonical_bytes());
    }
    Ok(CutMaterialRemovalPlanDiagnosticV1 {
        requested_components: requested_components.to_vec(),
        removed_components,
        retained_components,
        boundary_component: root,
        removed_faces,
        retained_faces,
        crossing_cut_boundaries,
        fingerprint: hash.finalize().into(),
        limits,
    })
}

#[cfg(test)]
mod tests {
    use ori_domain::{CreasePattern, Edge, EdgeKind, Paper, Point2, ProjectId, Vertex};
    use serde::de::DeserializeOwned;

    use super::*;

    fn id<T: DeserializeOwned>(suffix: u64) -> T {
        serde_json::from_str(&format!("\"00000000-0000-0000-0000-{suffix:012x}\"")).unwrap()
    }

    fn vertex(suffix: u64, x: f64, y: f64) -> Vertex {
        Vertex {
            id: id(suffix),
            position: Point2::new(x, y),
        }
    }

    fn edge(suffix: u64, a: &Vertex, b: &Vertex, kind: EdgeKind) -> Edge {
        Edge {
            id: id(suffix),
            start: a.id,
            end: b.id,
            kind,
        }
    }

    fn fixture(two_loops: bool) -> (ProjectId, Paper, CreasePattern) {
        let a = vertex(1, 0.0, 0.0);
        let b = vertex(2, 12.0, 0.0);
        let c = vertex(3, 12.0, 8.0);
        let d = vertex(4, 0.0, 8.0);
        let p = vertex(5, 2.0, 2.0);
        let q = vertex(6, 5.0, 2.0);
        let r = vertex(7, 3.5, 5.0);
        let s = vertex(8, 7.0, 2.0);
        let t = vertex(9, 10.0, 2.0);
        let u = vertex(10, 8.5, 5.0);
        let mut vertices = vec![
            a.clone(),
            b.clone(),
            c.clone(),
            d.clone(),
            p.clone(),
            q.clone(),
            r.clone(),
        ];
        let mut edges = vec![
            edge(20, &a, &b, EdgeKind::Boundary),
            edge(21, &b, &c, EdgeKind::Boundary),
            edge(22, &c, &d, EdgeKind::Boundary),
            edge(23, &d, &a, EdgeKind::Boundary),
            edge(30, &p, &q, EdgeKind::Cut),
            edge(31, &q, &r, EdgeKind::Cut),
            edge(32, &r, &p, EdgeKind::Cut),
        ];
        if two_loops {
            vertices.extend([s.clone(), t.clone(), u.clone()]);
            edges.extend([
                edge(33, &s, &t, EdgeKind::Cut),
                edge(34, &t, &u, EdgeKind::Cut),
                edge(35, &u, &s, EdgeKind::Cut),
            ]);
        }
        let paper = Paper {
            boundary_vertices: vec![a.id, b.id, c.id, d.id],
            cutting_allowed: true,
            ..Paper::default()
        };
        (id(100), paper, CreasePattern { vertices, edges })
    }

    fn input<'a>(
        namespace: ProjectId,
        revision: u64,
        paper: &'a Paper,
        pattern: &'a CreasePattern,
    ) -> FaceExtractionInput<'a> {
        FaceExtractionInput {
            identity_namespace: namespace,
            source_revision: revision,
            paper,
            pattern,
        }
    }

    #[test]
    fn boundary_component_is_kept_and_inner_components_are_candidates_only() {
        for two_loops in [false, true] {
            let (namespace, paper, pattern) = fixture(two_loops);
            let source = input(namespace, 7, &paper, &pattern);
            let diagnostic =
                diagnose_cut_material_component_selection_v1(source, Default::default()).unwrap();
            assert_eq!(
                diagnostic
                    .selections()
                    .iter()
                    .filter(|entry| entry.owns_original_boundary)
                    .count(),
                1
            );
            assert_eq!(
                diagnostic
                    .selections()
                    .iter()
                    .filter(|entry| !entry.owns_original_boundary)
                    .count(),
                if two_loops { 2 } else { 1 }
            );
            assert!(!diagnostic.authorizes_project_mutation());
            assert!(!diagnostic.authorizes_simulation_admission());
            assert!(diagnostic.is_for(source));
            assert!(!diagnostic.is_for(input(namespace, 8, &paper, &pattern)));
        }
    }

    #[test]
    fn pattern_paper_and_resource_tamper_fail_closed() {
        let (namespace, paper, pattern) = fixture(false);
        let source = input(namespace, 7, &paper, &pattern);
        let diagnostic =
            diagnose_cut_material_component_selection_v1(source, Default::default()).unwrap();
        let mut changed = pattern.clone();
        changed.vertices[4].position.x += 0.25;
        assert!(!diagnostic.is_for(input(namespace, 7, &paper, &changed)));
        let mut changed_paper = paper.clone();
        changed_paper.thickness_mm = 0.2;
        assert!(!diagnostic.is_for(input(namespace, 7, &changed_paper, &pattern)));
        assert!(
            diagnose_cut_material_component_selection_v1(
                source,
                ClosedCutLoopDiagnosticLimitsV1 {
                    max_edges: pattern.edges.len() - 1,
                    ..Default::default()
                },
            )
            .is_err()
        );
    }

    #[test]
    fn nested_loops_and_storage_reordering_are_canonical() {
        let (namespace, paper, mut pattern) = fixture(true);
        pattern.vertices[7].position = Point2::new(3.0, 2.5);
        pattern.vertices[8].position = Point2::new(4.0, 2.5);
        pattern.vertices[9].position = Point2::new(3.5, 3.5);
        let source = input(namespace, 7, &paper, &pattern);
        let first =
            diagnose_cut_material_component_selection_v1(source, Default::default()).unwrap();
        assert_eq!(first.selections().len(), 3);
        assert_eq!(
            first
                .selections()
                .iter()
                .filter(|entry| entry.owns_original_boundary)
                .count(),
            1
        );
        assert_eq!(
            first
                .selections()
                .iter()
                .filter(|entry| !entry.owns_original_boundary)
                .count(),
            2
        );
        assert!(!first.authorizes_material_removal());

        let mut reordered = pattern.clone();
        reordered.vertices.reverse();
        reordered.edges.reverse();
        let second = diagnose_cut_material_component_selection_v1(
            input(namespace, 7, &paper, &reordered),
            Default::default(),
        )
        .unwrap();
        assert_eq!(first.fingerprint_v1(), second.fingerprint_v1());
        assert_eq!(first.selections(), second.selections());
    }

    #[test]
    fn cut_loop_touching_original_boundary_fails_closed() {
        let (namespace, paper, mut pattern) = fixture(false);
        pattern.edges[4].start = paper.boundary_vertices[0];
        pattern.edges[6].end = paper.boundary_vertices[0];
        assert!(
            diagnose_cut_material_component_selection_v1(
                input(namespace, 7, &paper, &pattern),
                Default::default(),
            )
            .is_err()
        );
    }

    fn component_keys(
        source: FaceExtractionInput<'_>,
    ) -> (MaterialComponentKey, Vec<MaterialComponentKey>) {
        let selection =
            diagnose_cut_material_component_selection_v1(source, Default::default()).unwrap();
        let root = selection
            .selections()
            .iter()
            .find(|entry| entry.owns_original_boundary)
            .unwrap()
            .component;
        let candidates = selection
            .selections()
            .iter()
            .filter(|entry| !entry.owns_original_boundary)
            .map(|entry| entry.component)
            .collect();
        (root, candidates)
    }

    #[test]
    fn disjoint_requests_make_a_canonical_read_only_partition() {
        let (namespace, paper, pattern) = fixture(true);
        let source = input(namespace, 7, &paper, &pattern);
        let (root, candidates) = component_keys(source);
        let plan =
            diagnose_cut_material_removal_plan_v1(source, &candidates, Default::default()).unwrap();
        assert_eq!(plan.requested_components(), candidates);
        assert_eq!(plan.removed_components(), candidates);
        assert_eq!(plan.boundary_component(), root);
        assert_eq!(plan.retained_components(), &[root]);
        assert_eq!(plan.crossing_cut_boundaries().len(), 6);
        assert!(!plan.removed_faces().is_empty());
        assert!(!plan.retained_faces().is_empty());
        assert!(!plan.authorizes_project_mutation());
        assert!(!plan.authorizes_material_removal());
        assert!(!plan.authorizes_simulation_admission());
        assert!(plan.is_for(source, &candidates));
        assert!(!plan.is_for(input(namespace, 8, &paper, &pattern), &candidates));
    }

    #[test]
    fn disjoint_unrequested_sibling_remains_explicitly_retained() {
        let (namespace, paper, pattern) = fixture(true);
        let source = input(namespace, 7, &paper, &pattern);
        let (root, candidates) = component_keys(source);
        let plan =
            diagnose_cut_material_removal_plan_v1(source, &candidates[0..1], Default::default())
                .unwrap();
        assert_eq!(plan.removed_components(), &candidates[0..1]);
        assert_eq!(plan.retained_components().len(), 2);
        assert!(plan.retained_components().contains(&root));
        assert!(plan.retained_components().contains(&candidates[1]));
        assert_eq!(plan.crossing_cut_boundaries().len(), 3);
    }

    #[test]
    fn nested_parent_request_closes_over_its_descendant() {
        let (namespace, paper, mut pattern) = fixture(true);
        pattern.vertices[7].position = Point2::new(3.0, 2.5);
        pattern.vertices[8].position = Point2::new(4.0, 2.5);
        pattern.vertices[9].position = Point2::new(3.5, 3.5);
        let source = input(namespace, 7, &paper, &pattern);
        let (_, candidates) = component_keys(source);
        let first =
            diagnose_cut_material_removal_plan_v1(source, &candidates[0..1], Default::default())
                .unwrap();
        let second =
            diagnose_cut_material_removal_plan_v1(source, &candidates[1..2], Default::default())
                .unwrap();
        let parent = if first.removed_components().len() == 2 {
            &first
        } else {
            &second
        };
        assert_eq!(parent.removed_components().len(), 2);
        assert_eq!(parent.crossing_cut_boundaries().len(), 3);

        let both =
            diagnose_cut_material_removal_plan_v1(source, &candidates, Default::default()).unwrap();
        assert_eq!(both.removed_components(), parent.removed_components());
        assert_ne!(both.fingerprint_v1(), parent.fingerprint_v1());
    }

    #[test]
    fn removal_request_validation_and_resource_caps_fail_closed() {
        let (namespace, paper, pattern) = fixture(true);
        let source = input(namespace, 7, &paper, &pattern);
        let (root, candidates) = component_keys(source);
        assert_eq!(
            diagnose_cut_material_removal_plan_v1(source, &[], Default::default()),
            Err(CutMaterialRemovalPlanErrorV1::InvalidRequest)
        );
        assert_eq!(
            diagnose_cut_material_removal_plan_v1(source, &[root], Default::default()),
            Err(CutMaterialRemovalPlanErrorV1::BoundaryComponentRequested)
        );
        assert_eq!(
            diagnose_cut_material_removal_plan_v1(
                source,
                &[MaterialComponentKey([0xff; 32])],
                Default::default()
            ),
            Err(CutMaterialRemovalPlanErrorV1::UnknownComponent)
        );
        assert_eq!(
            diagnose_cut_material_removal_plan_v1(
                source,
                &[candidates[0], candidates[0]],
                Default::default()
            ),
            Err(CutMaterialRemovalPlanErrorV1::InvalidRequest)
        );
        let mut reversed = candidates.clone();
        reversed.reverse();
        assert_eq!(
            diagnose_cut_material_removal_plan_v1(source, &reversed, Default::default()),
            Err(CutMaterialRemovalPlanErrorV1::InvalidRequest)
        );
        assert!(matches!(
            diagnose_cut_material_removal_plan_v1(
                source,
                &candidates[0..1],
                ClosedCutLoopDiagnosticLimitsV1 {
                    max_edges: pattern.edges.len() - 1,
                    ..Default::default()
                }
            ),
            Err(CutMaterialRemovalPlanErrorV1::Selection(_))
        ));
    }

    #[test]
    fn plan_binds_complete_topology_selection_and_request() {
        let (namespace, paper, pattern) = fixture(false);
        let source = input(namespace, 7, &paper, &pattern);
        let (_, candidates) = component_keys(source);
        let plan =
            diagnose_cut_material_removal_plan_v1(source, &candidates, Default::default()).unwrap();
        let mut changed_pattern = pattern.clone();
        changed_pattern.vertices[4].position.x += 0.125;
        assert!(!plan.is_for(input(namespace, 7, &paper, &changed_pattern), &candidates));
        let mut changed_paper = paper.clone();
        changed_paper.thickness_mm = 0.2;
        assert!(!plan.is_for(input(namespace, 7, &changed_paper, &pattern), &candidates));
        assert!(!plan.is_for(source, &[MaterialComponentKey([0xfe; 32])]));
    }
}
