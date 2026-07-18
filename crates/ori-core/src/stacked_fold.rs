use std::cmp::Ordering;
use std::collections::HashMap;

use num_bigint::BigInt;
use ori_domain::{CreasePattern, FaceId, Paper, Point2, ProjectId, VertexId};
use ori_foldability::{
    FoldModelFingerprintV1, GlobalFlatFoldabilityProvenance, LAYER_ORDER_MODEL_ID, LayerFace,
    LayerOrderSnapshot, fold_model_fingerprint_v1,
};
use ori_geometry::{
    GeometryError, Orientation, PointPolygonRelation, exact_orientation, point_polygon_relation,
};
use ori_topology::{
    Face, FaceExtractionInput, TopologyIssueSeverity, TopologySnapshot, analyze_faces,
};
use thiserror::Error;

use crate::Revision;

pub const DEFAULT_MAX_FACE_LINEAGE_SOURCE_FACES: usize = 2_048;
pub const DEFAULT_MAX_FACE_LINEAGE_TARGET_FACES: usize = 2_048;
pub const DEFAULT_MAX_FACE_LINEAGE_BOUNDARY_HALF_EDGES: usize = 100_000;
pub const DEFAULT_MAX_FACE_LINEAGE_FACE_PAIRS: usize = 500_000;
pub const DEFAULT_MAX_FACE_LINEAGE_EXACT_CONTAINMENT_TESTS: usize = 100_000_000;

/// Deterministic limits for proving one crease-addition face lineage.
///
/// Equality is admitted. A caller must use the same limits when retrying the
/// same immutable input if it needs bit-for-bit repeatability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FaceLineageLimits {
    pub max_source_vertices: usize,
    pub max_source_edges: usize,
    pub max_source_paper_boundary_vertices: usize,
    pub max_target_vertices: usize,
    pub max_target_edges: usize,
    pub max_target_paper_boundary_vertices: usize,
    pub max_source_faces: usize,
    pub max_target_faces: usize,
    pub max_source_boundary_half_edges: usize,
    pub max_target_boundary_half_edges: usize,
    pub max_face_pairs: usize,
    pub max_exact_containment_tests: usize,
}

impl Default for FaceLineageLimits {
    fn default() -> Self {
        Self {
            max_source_vertices: crate::DEFAULT_MAX_SOURCE_VERTICES,
            max_source_edges: crate::DEFAULT_MAX_SOURCE_EDGES,
            max_source_paper_boundary_vertices: crate::DEFAULT_MAX_PAPER_BOUNDARY_VERTICES,
            max_target_vertices: crate::DEFAULT_MAX_SOURCE_VERTICES,
            max_target_edges: crate::DEFAULT_MAX_SOURCE_EDGES,
            max_target_paper_boundary_vertices: crate::DEFAULT_MAX_PAPER_BOUNDARY_VERTICES,
            max_source_faces: DEFAULT_MAX_FACE_LINEAGE_SOURCE_FACES,
            max_target_faces: DEFAULT_MAX_FACE_LINEAGE_TARGET_FACES,
            max_source_boundary_half_edges: DEFAULT_MAX_FACE_LINEAGE_BOUNDARY_HALF_EDGES,
            max_target_boundary_half_edges: DEFAULT_MAX_FACE_LINEAGE_BOUNDARY_HALF_EDGES,
            max_face_pairs: DEFAULT_MAX_FACE_LINEAGE_FACE_PAIRS,
            max_exact_containment_tests: DEFAULT_MAX_FACE_LINEAGE_EXACT_CONTAINMENT_TESTS,
        }
    }
}

/// Immutable source and candidate geometry for one future stacked-fold
/// transaction.
///
/// This input only prepares face lineage. It does not authorize a project
/// mutation: reverse mapping, per-layer assignments, collision-stop evidence,
/// the updated layer order, and timeline migration still have to succeed in
/// the eventual atomic `ApplyStackedFold` command. In particular, this module
/// neither proves that the target delta is one straight crease nor re-proves
/// overlap-cell stack ordering. `LayerOrderSnapshot` is public transport data,
/// so matching its provenance and material registry here is not authentication
/// that the solver minted it. That command must separately authenticate the
/// native current-layer-order slot, its immutable binding, and its complete
/// certificate immediately before commit.
#[derive(Debug, Clone, Copy)]
pub struct FaceLineageInput<'a> {
    pub identity_namespace: ProjectId,
    pub source_revision: Revision,
    pub source_paper: &'a Paper,
    pub source_pattern: &'a CreasePattern,
    pub source_layer_order: &'a LayerOrderSnapshot,
    pub target_revision: Revision,
    pub target_paper: &'a Paper,
    pub target_pattern: &'a CreasePattern,
}

/// One complete source-face to descendant-faces relation.
///
/// Descendants are ordered by canonical `FaceKey`, then by the face ID's RFC
/// bytes. At least one record in a valid lineage has two or more descendants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FaceLineageRecord {
    source: LayerFace,
    descendants: Vec<LayerFace>,
}

impl FaceLineageRecord {
    #[must_use]
    pub const fn source(&self) -> LayerFace {
        self.source
    }

    #[must_use]
    pub fn descendants(&self) -> &[LayerFace] {
        &self.descendants
    }
}

/// Canonical, revision-bound proof that candidate faces refine source faces.
///
/// Fields remain private so callers cannot forge an accepted mapping by
/// constructing this type directly. The proof is deliberately not a project
/// command, does not confer authority for any layer stack, and carries no
/// authority to mutate an [`crate::EditorState`].
///
/// ```compile_fail
/// use ori_core::FaceLineageV1;
///
/// fn discard_records(proof: FaceLineageV1) -> FaceLineageV1 {
///     FaceLineageV1 {
///         records: Vec::new(),
///         ..proof
///     }
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FaceLineageV1 {
    identity_namespace: ProjectId,
    source_revision: Revision,
    target_revision: Revision,
    source_fingerprint: FoldModelFingerprintV1,
    target_fingerprint: FoldModelFingerprintV1,
    records: Vec<FaceLineageRecord>,
}

impl FaceLineageV1 {
    #[must_use]
    pub const fn identity_namespace(&self) -> ProjectId {
        self.identity_namespace
    }

    #[must_use]
    pub const fn source_revision(&self) -> Revision {
        self.source_revision
    }

    #[must_use]
    pub const fn target_revision(&self) -> Revision {
        self.target_revision
    }

    #[must_use]
    pub const fn source_fingerprint(&self) -> FoldModelFingerprintV1 {
        self.source_fingerprint
    }

    #[must_use]
    pub const fn target_fingerprint(&self) -> FoldModelFingerprintV1 {
        self.target_fingerprint
    }

    #[must_use]
    pub fn records(&self) -> &[FaceLineageRecord] {
        &self.records
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaceLineageTopology {
    Source,
    Target,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaceLineageResource {
    SourceVertices,
    SourceEdges,
    SourcePaperBoundaryVertices,
    TargetVertices,
    TargetEdges,
    TargetPaperBoundaryVertices,
    SourceFaces,
    TargetFaces,
    SourceBoundaryHalfEdges,
    TargetBoundaryHalfEdges,
    FacePairs,
    ExactContainmentTests,
}

#[derive(Debug, Error, PartialEq)]
pub enum FaceLineageError {
    #[error("source revision cannot advance")]
    SourceRevisionCannotAdvance,
    #[error("target revision {actual} is not the required next revision {expected}")]
    TargetRevisionNotNext {
        expected: Revision,
        actual: Revision,
    },
    #[error("{topology:?} topology is not safe and complete ({issue_count} blocking issue(s))")]
    TopologyNotSimulationReady {
        topology: FaceLineageTopology,
        issue_count: usize,
    },
    #[error("the supplied layer order is not current for the source geometry")]
    LayerOrderNotCurrent,
    #[error("the supplied layer order does not use the required model")]
    LayerOrderModelMismatch,
    #[error("the supplied layer-order material registry does not match source topology")]
    LayerOrderMaterialRegistryMismatch,
    #[error("a face-lineage candidate cannot change non-boundary paper properties")]
    PaperPropertiesChanged,
    #[error("{resource:?} exceeds its limit: {actual} > {maximum}")]
    ResourceLimit {
        resource: FaceLineageResource,
        actual: usize,
        maximum: usize,
    },
    #[error("source face {face:?} is not convex and cannot use this proof model")]
    SourceFaceNotConvex { face: FaceId },
    #[error("target face {face:?} is not contained by any source face")]
    TargetFaceWithoutSource { face: FaceId },
    #[error("target face {face:?} is contained by more than one source face")]
    TargetFaceWithMultipleSources { face: FaceId },
    #[error("source face {face:?} has no target descendant")]
    SourceFaceWithoutDescendants { face: FaceId },
    #[error("descendants do not conserve the exact area of source face {face:?}")]
    SourceFaceAreaMismatch { face: FaceId },
    #[error("candidate geometry does not split any source face")]
    NoSourceFaceSplit,
    #[error("validated topology lost an internal vertex or face invariant")]
    ValidatedTopologyInvariantLost,
    #[error("exact containment predicate failed: {0}")]
    Geometry(#[from] GeometryError),
}

#[derive(Debug)]
struct PolygonRecord {
    face: LayerFace,
    points: Vec<Point2>,
    exact_double_area: BigInt,
}

/// Proves a complete, deterministic source-face to target-face lineage.
///
/// Both topologies are rebuilt from immutable geometry. Source faces must be
/// convex, matching the current `facewise_layer_order_v1` target class. Every
/// target vertex is then classified exactly against every source polygon. A
/// target is accepted only inside one source, and exact dyadic area
/// conservation is checked independently for every source face.
///
/// The function is read-only. Every error therefore leaves project geometry,
/// revision, timeline, and Undo/Redo history unchanged.
///
/// This foundation has deterministic count limits but no deadline or
/// cancellation channel. A future UI command must run it away from the
/// project-state lock, support cooperative cancellation around this bounded
/// phase, and revalidate the immutable project/layer-order binding before an
/// atomic commit.
pub fn prepare_face_lineage_v1(
    input: FaceLineageInput<'_>,
    limits: FaceLineageLimits,
) -> Result<FaceLineageV1, FaceLineageError> {
    check_limit(
        FaceLineageResource::SourceVertices,
        input.source_pattern.vertices.len(),
        limits.max_source_vertices,
    )?;
    check_limit(
        FaceLineageResource::SourceEdges,
        input.source_pattern.edges.len(),
        limits.max_source_edges,
    )?;
    check_limit(
        FaceLineageResource::SourcePaperBoundaryVertices,
        input.source_paper.boundary_vertices.len(),
        limits.max_source_paper_boundary_vertices,
    )?;
    check_limit(
        FaceLineageResource::TargetVertices,
        input.target_pattern.vertices.len(),
        limits.max_target_vertices,
    )?;
    check_limit(
        FaceLineageResource::TargetEdges,
        input.target_pattern.edges.len(),
        limits.max_target_edges,
    )?;
    check_limit(
        FaceLineageResource::TargetPaperBoundaryVertices,
        input.target_paper.boundary_vertices.len(),
        limits.max_target_paper_boundary_vertices,
    )?;

    let expected_target_revision = input
        .source_revision
        .checked_add(1)
        .ok_or(FaceLineageError::SourceRevisionCannotAdvance)?;
    if input.target_revision != expected_target_revision {
        return Err(FaceLineageError::TargetRevisionNotNext {
            expected: expected_target_revision,
            actual: input.target_revision,
        });
    }
    if input.source_paper.thickness_mm.to_bits() != input.target_paper.thickness_mm.to_bits()
        || input.source_paper.cutting_allowed != input.target_paper.cutting_allowed
        || input.source_paper.front != input.target_paper.front
        || input.source_paper.back != input.target_paper.back
    {
        return Err(FaceLineageError::PaperPropertiesChanged);
    }

    let source_topology = simulation_snapshot(
        input.identity_namespace,
        input.source_revision,
        input.source_paper,
        input.source_pattern,
        FaceLineageTopology::Source,
    )?;
    check_topology_limits(&source_topology, FaceLineageTopology::Source, limits)?;

    let source_fingerprint = fold_model_fingerprint_v1(input.source_pattern, input.source_paper);
    let source_provenance = GlobalFlatFoldabilityProvenance::for_geometry(
        input.identity_namespace,
        input.source_revision,
        input.source_paper,
        input.source_pattern,
    );
    if !input.source_layer_order.is_current_for(&source_provenance) {
        return Err(FaceLineageError::LayerOrderNotCurrent);
    }
    if input.source_layer_order.model_id != LAYER_ORDER_MODEL_ID {
        return Err(FaceLineageError::LayerOrderModelMismatch);
    }

    let source_registry = canonical_registry(&source_topology);
    if input.source_layer_order.material_faces.len() != source_registry.len()
        || input.source_layer_order.material_faces != source_registry
    {
        return Err(FaceLineageError::LayerOrderMaterialRegistryMismatch);
    }

    let target_topology = simulation_snapshot(
        input.identity_namespace,
        input.target_revision,
        input.target_paper,
        input.target_pattern,
        FaceLineageTopology::Target,
    )?;
    check_topology_limits(&target_topology, FaceLineageTopology::Target, limits)?;

    let source_polygons = polygon_records(input.source_pattern, &source_topology)?;
    let target_polygons = polygon_records(input.target_pattern, &target_topology)?;
    for source in &source_polygons {
        ensure_convex(source)?;
    }

    let pair_count = source_polygons
        .len()
        .checked_mul(target_polygons.len())
        .ok_or(FaceLineageError::ResourceLimit {
            resource: FaceLineageResource::FacePairs,
            actual: usize::MAX,
            maximum: limits.max_face_pairs,
        })?;
    check_limit(
        FaceLineageResource::FacePairs,
        pair_count,
        limits.max_face_pairs,
    )?;

    let mut exact_tests = 0_usize;
    let mut descendants = vec![Vec::<usize>::new(); source_polygons.len()];
    for (target_index, target) in target_polygons.iter().enumerate() {
        let mut matching_source = None;
        for (source_index, source) in source_polygons.iter().enumerate() {
            let pair_tests = source
                .points
                .len()
                .checked_mul(target.points.len())
                .and_then(|value| value.checked_mul(2))
                .ok_or(FaceLineageError::ResourceLimit {
                    resource: FaceLineageResource::ExactContainmentTests,
                    actual: usize::MAX,
                    maximum: limits.max_exact_containment_tests,
                })?;
            exact_tests =
                exact_tests
                    .checked_add(pair_tests)
                    .ok_or(FaceLineageError::ResourceLimit {
                        resource: FaceLineageResource::ExactContainmentTests,
                        actual: usize::MAX,
                        maximum: limits.max_exact_containment_tests,
                    })?;
            check_limit(
                FaceLineageResource::ExactContainmentTests,
                exact_tests,
                limits.max_exact_containment_tests,
            )?;

            if polygon_is_within_convex_source(&target.points, &source.points)?
                && matching_source.replace(source_index).is_some()
            {
                return Err(FaceLineageError::TargetFaceWithMultipleSources {
                    face: target.face.face_id,
                });
            }
        }
        let source_index = matching_source.ok_or(FaceLineageError::TargetFaceWithoutSource {
            face: target.face.face_id,
        })?;
        descendants[source_index].push(target_index);
    }

    let mut records = Vec::with_capacity(source_polygons.len());
    let mut split_found = false;
    for (source_index, source) in source_polygons.iter().enumerate() {
        let target_indices = &descendants[source_index];
        if target_indices.is_empty() {
            return Err(FaceLineageError::SourceFaceWithoutDescendants {
                face: source.face.face_id,
            });
        }
        split_found |= target_indices.len() > 1;

        let descendant_area = target_indices
            .iter()
            .fold(BigInt::from(0_u8), |area, index| {
                area + &target_polygons[*index].exact_double_area
            });
        if descendant_area != source.exact_double_area {
            return Err(FaceLineageError::SourceFaceAreaMismatch {
                face: source.face.face_id,
            });
        }

        let mut canonical_descendants = target_indices
            .iter()
            .map(|index| target_polygons[*index].face)
            .collect::<Vec<_>>();
        canonical_descendants.sort_unstable_by(compare_layer_faces);
        records.push(FaceLineageRecord {
            source: source.face,
            descendants: canonical_descendants,
        });
    }
    if !split_found {
        return Err(FaceLineageError::NoSourceFaceSplit);
    }
    records.sort_unstable_by(|left, right| compare_layer_faces(&left.source, &right.source));

    Ok(FaceLineageV1 {
        identity_namespace: input.identity_namespace,
        source_revision: input.source_revision,
        target_revision: input.target_revision,
        source_fingerprint,
        target_fingerprint: fold_model_fingerprint_v1(input.target_pattern, input.target_paper),
        records,
    })
}

fn check_limit(
    resource: FaceLineageResource,
    actual: usize,
    maximum: usize,
) -> Result<(), FaceLineageError> {
    if actual > maximum {
        Err(FaceLineageError::ResourceLimit {
            resource,
            actual,
            maximum,
        })
    } else {
        Ok(())
    }
}

fn simulation_snapshot(
    identity_namespace: ProjectId,
    source_revision: Revision,
    paper: &Paper,
    pattern: &CreasePattern,
    topology: FaceLineageTopology,
) -> Result<TopologySnapshot, FaceLineageError> {
    let report = analyze_faces(FaceExtractionInput {
        identity_namespace,
        source_revision,
        paper,
        pattern,
    });
    let blocking_issue_count = report
        .issues
        .iter()
        .filter(|issue| issue.severity != TopologyIssueSeverity::Warning)
        .count();
    match (blocking_issue_count, report.snapshot) {
        (0, Some(snapshot)) => Ok(snapshot),
        _ => Err(FaceLineageError::TopologyNotSimulationReady {
            topology,
            issue_count: blocking_issue_count,
        }),
    }
}

fn check_topology_limits(
    topology: &TopologySnapshot,
    side: FaceLineageTopology,
    limits: FaceLineageLimits,
) -> Result<(), FaceLineageError> {
    let face_resource = match side {
        FaceLineageTopology::Source => FaceLineageResource::SourceFaces,
        FaceLineageTopology::Target => FaceLineageResource::TargetFaces,
    };
    let face_limit = match side {
        FaceLineageTopology::Source => limits.max_source_faces,
        FaceLineageTopology::Target => limits.max_target_faces,
    };
    check_limit(face_resource, topology.faces.len(), face_limit)?;

    let boundary_half_edges = topology
        .faces
        .iter()
        .try_fold(0_usize, |total, face| {
            total.checked_add(face.outer.half_edges.len())
        })
        .ok_or(FaceLineageError::ResourceLimit {
            resource: match side {
                FaceLineageTopology::Source => FaceLineageResource::SourceBoundaryHalfEdges,
                FaceLineageTopology::Target => FaceLineageResource::TargetBoundaryHalfEdges,
            },
            actual: usize::MAX,
            maximum: match side {
                FaceLineageTopology::Source => limits.max_source_boundary_half_edges,
                FaceLineageTopology::Target => limits.max_target_boundary_half_edges,
            },
        })?;
    let (resource, maximum) = match side {
        FaceLineageTopology::Source => (
            FaceLineageResource::SourceBoundaryHalfEdges,
            limits.max_source_boundary_half_edges,
        ),
        FaceLineageTopology::Target => (
            FaceLineageResource::TargetBoundaryHalfEdges,
            limits.max_target_boundary_half_edges,
        ),
    };
    check_limit(resource, boundary_half_edges, maximum)
}

fn canonical_registry(topology: &TopologySnapshot) -> Vec<LayerFace> {
    let mut registry = topology
        .faces
        .iter()
        .map(|face| LayerFace {
            face_id: face.id,
            face_key: face.key,
        })
        .collect::<Vec<_>>();
    registry.sort_unstable_by(compare_layer_faces);
    registry
}

fn compare_layer_faces(left: &LayerFace, right: &LayerFace) -> Ordering {
    left.face_key.cmp(&right.face_key).then_with(|| {
        left.face_id
            .canonical_bytes()
            .cmp(&right.face_id.canonical_bytes())
    })
}

fn polygon_records(
    pattern: &CreasePattern,
    topology: &TopologySnapshot,
) -> Result<Vec<PolygonRecord>, FaceLineageError> {
    let positions = pattern
        .vertices
        .iter()
        .map(|vertex| (vertex.id, vertex.position))
        .collect::<HashMap<VertexId, Point2>>();
    let mut records = topology
        .faces
        .iter()
        .map(|face| polygon_record(face, &positions))
        .collect::<Result<Vec<_>, _>>()?;
    records.sort_unstable_by(|left, right| compare_layer_faces(&left.face, &right.face));
    Ok(records)
}

fn polygon_record(
    face: &Face,
    positions: &HashMap<VertexId, Point2>,
) -> Result<PolygonRecord, FaceLineageError> {
    let points = face
        .outer
        .half_edges
        .iter()
        .map(|half_edge| {
            positions
                .get(&half_edge.origin)
                .copied()
                .ok_or(FaceLineageError::ValidatedTopologyInvariantLost)
        })
        .collect::<Result<Vec<_>, _>>()?;
    if points.len() < 3 {
        return Err(FaceLineageError::ValidatedTopologyInvariantLost);
    }
    let exact_double_area = exact_polygon_double_area(&points);
    if exact_double_area <= BigInt::from(0_u8) {
        return Err(FaceLineageError::ValidatedTopologyInvariantLost);
    }
    Ok(PolygonRecord {
        face: LayerFace {
            face_id: face.id,
            face_key: face.key,
        },
        points,
        exact_double_area,
    })
}

fn ensure_convex(source: &PolygonRecord) -> Result<(), FaceLineageError> {
    for index in 0..source.points.len() {
        let previous = source.points[(index + source.points.len() - 1) % source.points.len()];
        let current = source.points[index];
        let next = source.points[(index + 1) % source.points.len()];
        if exact_orientation(previous, current, next)? == Orientation::Clockwise {
            return Err(FaceLineageError::SourceFaceNotConvex {
                face: source.face.face_id,
            });
        }
    }
    Ok(())
}

fn polygon_is_within_convex_source(
    target: &[Point2],
    source: &[Point2],
) -> Result<bool, GeometryError> {
    for point in target {
        if point_polygon_relation(*point, source)? == PointPolygonRelation::Outside {
            return Ok(false);
        }
    }

    // The source is independently proven convex and the target topology is a
    // simple material face. Every segment between two accepted target
    // vertices, and therefore the entire target polygon, is inside the closed
    // convex source. Rechecking each midpoint would add no proof strength and
    // would repeatedly allocate an exact copy of the source polygon.
    Ok(true)
}

/// Returns the exact signed double area at the common `2^-2148` scale.
///
/// Every finite binary64 coordinate is an integer multiple of `2^-1074`.
/// Products therefore share this fixed scale, so equality remains exact
/// without an epsilon or an independently rounded `f64` area.
fn exact_polygon_double_area(points: &[Point2]) -> BigInt {
    let mut area = BigInt::from(0_u8);
    for index in 0..points.len() {
        let current = points[index];
        let next = points[(index + 1) % points.len()];
        area += exact_f64_at_minimum_scale(current.x) * exact_f64_at_minimum_scale(next.y);
        area -= exact_f64_at_minimum_scale(current.y) * exact_f64_at_minimum_scale(next.x);
    }
    area
}

fn exact_f64_at_minimum_scale(value: f64) -> BigInt {
    debug_assert!(value.is_finite());
    let bits = value.to_bits();
    let negative = bits >> 63 != 0;
    let exponent = ((bits >> 52) & 0x7ff) as usize;
    let fraction = bits & ((1_u64 << 52) - 1);
    let (significand, shift) = if exponent == 0 {
        (fraction, 0)
    } else {
        ((1_u64 << 52) | fraction, exponent - 1)
    };
    let integer = BigInt::from(significand) << shift;
    if negative { -integer } else { integer }
}

#[cfg(test)]
mod tests {
    use ori_domain::{Edge, EdgeId, EdgeKind, Vertex};
    use ori_foldability::{
        GlobalFlatFoldabilityInput, GlobalFlatFoldabilityLimits, analyze_global_flat_foldability,
    };
    use ori_topology::{analyze_faces, analyze_local_flat_foldability};

    use super::*;
    use crate::{EditorState, create_rectangular_sheet};

    struct Fixture {
        identity: ProjectId,
        source_pattern: CreasePattern,
        source_paper: Paper,
        source_layer_order: LayerOrderSnapshot,
        target_pattern: CreasePattern,
        target_paper: Paper,
    }

    impl Fixture {
        fn input(&self) -> FaceLineageInput<'_> {
            FaceLineageInput {
                identity_namespace: self.identity,
                source_revision: 7,
                source_paper: &self.source_paper,
                source_pattern: &self.source_pattern,
                source_layer_order: &self.source_layer_order,
                target_revision: 8,
                target_paper: &self.target_paper,
                target_pattern: &self.target_pattern,
            }
        }
    }

    fn fixture() -> Fixture {
        let identity = ProjectId::new();
        let sheet = create_rectangular_sheet(400.0, 400.0, false).expect("create rectangle");
        let (source_pattern, source_paper) = sheet.into_parts();
        let source_layer_order = proven_layer_order(identity, 7, &source_pattern, &source_paper);

        let mut target_pattern = source_pattern.clone();
        target_pattern.edges.push(Edge {
            id: EdgeId::new(),
            start: source_paper.boundary_vertices[0],
            end: source_paper.boundary_vertices[2],
            kind: EdgeKind::Mountain,
        });

        Fixture {
            identity,
            source_pattern,
            source_paper: source_paper.clone(),
            source_layer_order,
            target_pattern,
            target_paper: source_paper,
        }
    }

    fn proven_layer_order(
        identity: ProjectId,
        revision: Revision,
        pattern: &CreasePattern,
        paper: &Paper,
    ) -> LayerOrderSnapshot {
        let source_topology = analyze_faces(FaceExtractionInput {
            identity_namespace: identity,
            source_revision: revision,
            paper,
            pattern,
        })
        .snapshot
        .expect("source topology");
        let local = analyze_local_flat_foldability(paper, pattern);
        let report = analyze_global_flat_foldability(
            GlobalFlatFoldabilityInput::current_with_geometry(
                identity,
                paper,
                pattern,
                &source_topology,
                &local,
            ),
            GlobalFlatFoldabilityLimits::default(),
        )
        .expect("global analysis");
        report.layer_order().expect("possible layer order").clone()
    }

    #[test]
    fn proves_one_source_face_split_into_two_canonical_descendants() {
        let fixture = fixture();
        let lineage = prepare_face_lineage_v1(fixture.input(), FaceLineageLimits::default())
            .expect("prove lineage");

        assert_eq!(lineage.identity_namespace(), fixture.identity);
        assert_eq!(lineage.source_revision(), 7);
        assert_eq!(lineage.target_revision(), 8);
        assert_eq!(
            lineage.source_fingerprint(),
            fold_model_fingerprint_v1(&fixture.source_pattern, &fixture.source_paper)
        );
        assert_eq!(
            lineage.target_fingerprint(),
            fold_model_fingerprint_v1(&fixture.target_pattern, &fixture.target_paper)
        );
        assert_eq!(lineage.records().len(), 1);
        assert_eq!(lineage.records()[0].descendants().len(), 2);
        assert!(
            lineage.records()[0]
                .descendants()
                .windows(2)
                .all(|faces| compare_layer_faces(&faces[0], &faces[1]) == Ordering::Less)
        );
    }

    #[test]
    fn lineage_is_invariant_to_storage_order_and_new_edge_direction() {
        let fixture = fixture();
        let expected = prepare_face_lineage_v1(fixture.input(), FaceLineageLimits::default())
            .expect("baseline lineage");

        let mut reordered = fixture.target_pattern.clone();
        reordered.vertices.reverse();
        reordered.edges.reverse();
        let mut reordered_paper = fixture.target_paper.clone();
        reordered_paper.boundary_vertices.rotate_left(1);
        reordered_paper.boundary_vertices.reverse();
        let fold = reordered
            .edges
            .iter_mut()
            .find(|edge| matches!(edge.kind, EdgeKind::Mountain))
            .expect("new fold");
        std::mem::swap(&mut fold.start, &mut fold.end);
        let input = FaceLineageInput {
            target_pattern: &reordered,
            target_paper: &reordered_paper,
            ..fixture.input()
        };

        assert_eq!(
            prepare_face_lineage_v1(input, FaceLineageLimits::default()),
            Ok(expected)
        );
    }

    #[test]
    fn proves_two_source_faces_each_split_after_shared_hinge_subdivision() {
        let identity = ProjectId::new();
        let sheet = create_rectangular_sheet(400.0, 400.0, false).expect("create rectangle");
        let (mut source_pattern, paper) = sheet.into_parts();
        let source_hinge = EdgeId::new();
        source_pattern.edges.push(Edge {
            id: source_hinge,
            start: paper.boundary_vertices[0],
            end: paper.boundary_vertices[2],
            kind: EdgeKind::Mountain,
        });
        let source_layer_order = proven_layer_order(identity, 12, &source_pattern, &paper);

        let mut target_pattern = source_pattern.clone();
        let center = VertexId::new();
        target_pattern.vertices.push(Vertex {
            id: center,
            position: Point2::new(200.0, 200.0),
        });
        target_pattern
            .edges
            .iter_mut()
            .find(|edge| edge.id == source_hinge)
            .expect("source hinge")
            .end = center;
        target_pattern.edges.extend([
            Edge {
                id: EdgeId::new(),
                start: center,
                end: paper.boundary_vertices[2],
                kind: EdgeKind::Mountain,
            },
            Edge {
                id: EdgeId::new(),
                start: paper.boundary_vertices[1],
                end: center,
                kind: EdgeKind::Valley,
            },
            Edge {
                id: EdgeId::new(),
                start: center,
                end: paper.boundary_vertices[3],
                kind: EdgeKind::Valley,
            },
        ]);

        let lineage = prepare_face_lineage_v1(
            FaceLineageInput {
                identity_namespace: identity,
                source_revision: 12,
                source_paper: &paper,
                source_pattern: &source_pattern,
                source_layer_order: &source_layer_order,
                target_revision: 13,
                target_paper: &paper,
                target_pattern: &target_pattern,
            },
            FaceLineageLimits::default(),
        )
        .expect("prove two-face lineage");

        assert_eq!(lineage.records().len(), 2);
        assert!(
            lineage
                .records()
                .iter()
                .all(|record| record.descendants().len() == 2)
        );
        let descendant_ids = lineage
            .records()
            .iter()
            .flat_map(FaceLineageRecord::descendants)
            .map(|face| face.face_id)
            .collect::<std::collections::HashSet<_>>();
        assert_eq!(descendant_ids.len(), 4);
    }

    #[test]
    fn stale_layer_order_is_rejected_before_any_lineage_is_published() {
        let mut fixture = fixture();
        fixture.source_layer_order.provenance.source.source_revision = 6;

        assert_eq!(
            prepare_face_lineage_v1(fixture.input(), FaceLineageLimits::default()),
            Err(FaceLineageError::LayerOrderNotCurrent)
        );
    }

    #[test]
    fn oversized_layer_registry_is_rejected_without_clone_or_target_work() {
        let mut fixture = fixture();
        let repeated_face = fixture.source_layer_order.material_faces[0];
        fixture
            .source_layer_order
            .material_faces
            .resize(DEFAULT_MAX_FACE_LINEAGE_SOURCE_FACES + 1, repeated_face);
        fixture.target_pattern.edges[0].start = VertexId::new();

        assert_eq!(
            prepare_face_lineage_v1(fixture.input(), FaceLineageLimits::default()),
            Err(FaceLineageError::LayerOrderMaterialRegistryMismatch)
        );
    }

    #[test]
    fn revision_gap_and_unrelated_paper_changes_are_rejected() {
        let fixture = fixture();
        let revision_gap = FaceLineageInput {
            target_revision: 9,
            ..fixture.input()
        };
        assert_eq!(
            prepare_face_lineage_v1(revision_gap, FaceLineageLimits::default()),
            Err(FaceLineageError::TargetRevisionNotNext {
                expected: 8,
                actual: 9,
            })
        );

        let mut changed_paper = fixture.target_paper.clone();
        changed_paper.front.color.red ^= 1;
        let paper_change = FaceLineageInput {
            target_paper: &changed_paper,
            ..fixture.input()
        };
        assert_eq!(
            prepare_face_lineage_v1(paper_change, FaceLineageLimits::default()),
            Err(FaceLineageError::PaperPropertiesChanged)
        );
    }

    #[test]
    fn exact_per_source_area_rejects_material_loss() {
        let fixture = fixture();
        let smaller = create_rectangular_sheet(200.0, 200.0, false).expect("smaller rectangle");
        let (mut target_pattern, target_paper) = smaller.into_parts();
        target_pattern.edges.push(Edge {
            id: EdgeId::new(),
            start: target_paper.boundary_vertices[0],
            end: target_paper.boundary_vertices[2],
            kind: EdgeKind::Mountain,
        });
        let input = FaceLineageInput {
            target_pattern: &target_pattern,
            target_paper: &target_paper,
            ..fixture.input()
        };

        assert!(matches!(
            prepare_face_lineage_v1(input, FaceLineageLimits::default()),
            Err(FaceLineageError::SourceFaceAreaMismatch { .. })
        ));
    }

    #[test]
    fn no_geometry_split_is_not_a_stacked_fold_lineage() {
        let fixture = fixture();
        let input = FaceLineageInput {
            target_pattern: &fixture.source_pattern,
            target_paper: &fixture.source_paper,
            ..fixture.input()
        };

        assert_eq!(
            prepare_face_lineage_v1(input, FaceLineageLimits::default()),
            Err(FaceLineageError::NoSourceFaceSplit)
        );
    }

    #[test]
    fn stale_revision_and_resource_failure_leave_editor_state_unchanged() {
        let fixture = fixture();
        let editor =
            EditorState::with_paper(fixture.source_pattern.clone(), fixture.source_paper.clone());
        let before_pattern = editor.pattern().clone();
        let before_paper = editor.paper().clone();
        let before_timeline = editor.instruction_timeline().clone();
        let before_revision = editor.revision();
        let before_undo = editor.can_undo();
        let before_redo = editor.can_redo();

        let stale = FaceLineageInput {
            source_revision: u64::MAX,
            target_revision: 0,
            ..fixture.input()
        };
        assert_eq!(
            prepare_face_lineage_v1(stale, FaceLineageLimits::default()),
            Err(FaceLineageError::SourceRevisionCannotAdvance)
        );

        let limits = FaceLineageLimits {
            max_target_edges: fixture.target_pattern.edges.len() - 1,
            ..FaceLineageLimits::default()
        };
        assert!(matches!(
            prepare_face_lineage_v1(fixture.input(), limits),
            Err(FaceLineageError::ResourceLimit {
                resource: FaceLineageResource::TargetEdges,
                ..
            })
        ));

        assert_eq!(editor.pattern(), &before_pattern);
        assert_eq!(editor.paper(), &before_paper);
        assert_eq!(editor.instruction_timeline(), &before_timeline);
        assert_eq!(editor.revision(), before_revision);
        assert_eq!(editor.can_undo(), before_undo);
        assert_eq!(editor.can_redo(), before_redo);
    }

    #[test]
    fn exact_work_limits_admit_equality_and_reject_the_next_operation() {
        let fixture = fixture();
        let exact_limit = 2 * 4 * 3 * 2;
        let inclusive = FaceLineageLimits {
            max_face_pairs: 2,
            max_exact_containment_tests: exact_limit,
            ..FaceLineageLimits::default()
        };
        prepare_face_lineage_v1(fixture.input(), inclusive)
            .expect("the documented resource limits admit equality");

        let pair_limited = FaceLineageLimits {
            max_face_pairs: 1,
            ..inclusive
        };
        assert_eq!(
            prepare_face_lineage_v1(fixture.input(), pair_limited),
            Err(FaceLineageError::ResourceLimit {
                resource: FaceLineageResource::FacePairs,
                actual: 2,
                maximum: 1,
            })
        );

        let predicate_limited = FaceLineageLimits {
            max_exact_containment_tests: exact_limit - 1,
            ..inclusive
        };
        assert_eq!(
            prepare_face_lineage_v1(fixture.input(), predicate_limited),
            Err(FaceLineageError::ResourceLimit {
                resource: FaceLineageResource::ExactContainmentTests,
                actual: exact_limit,
                maximum: exact_limit - 1,
            })
        );
    }

    #[test]
    fn convex_vertex_certificate_contains_whole_target_edges() {
        let source = [
            Point2::new(0.0, 0.0),
            Point2::new(10.0, 0.0),
            Point2::new(10.0, 10.0),
            Point2::new(0.0, 10.0),
        ];
        let boundary_chord = [
            Point2::new(0.0, 0.0),
            Point2::new(10.0, 10.0),
            Point2::new(0.0, 10.0),
        ];
        let concave_target = [
            Point2::new(1.0, 1.0),
            Point2::new(9.0, 1.0),
            Point2::new(5.0, 5.0),
            Point2::new(9.0, 9.0),
            Point2::new(1.0, 9.0),
        ];
        let outside = [
            Point2::new(1.0, 1.0),
            Point2::new(11.0, 5.0),
            Point2::new(1.0, 9.0),
        ];

        assert!(polygon_is_within_convex_source(&boundary_chord, &source).unwrap());
        assert!(polygon_is_within_convex_source(&concave_target, &source).unwrap());
        assert!(!polygon_is_within_convex_source(&outside, &source).unwrap());
    }

    #[test]
    fn exact_binary64_units_cover_subnormal_normal_and_maximum_values() {
        let minimum_subnormal = f64::from_bits(1);
        assert_eq!(
            exact_f64_at_minimum_scale(minimum_subnormal),
            BigInt::from(1_u8)
        );
        assert_eq!(
            exact_f64_at_minimum_scale(-minimum_subnormal),
            BigInt::from(-1_i8)
        );
        assert_eq!(
            exact_f64_at_minimum_scale(f64::MIN_POSITIVE),
            BigInt::from(1_u8) << 52_usize
        );
        assert_eq!(
            exact_f64_at_minimum_scale(1.0),
            BigInt::from(1_u8) << 1074_usize
        );
        assert_eq!(
            exact_f64_at_minimum_scale(f64::MAX),
            BigInt::from((1_u64 << 53) - 1) << 2045_usize
        );
        assert_eq!(exact_f64_at_minimum_scale(-0.0), BigInt::from(0_u8));
    }

    #[test]
    fn exact_area_uses_binary64_values_without_rounding_the_sum() {
        let huge = f64::from_bits(0x7fe0_0000_0000_0000);
        let tiny = f64::from_bits(1);
        let polygon = [
            Point2::new(0.0, 0.0),
            Point2::new(huge, 0.0),
            Point2::new(huge, tiny),
            Point2::new(0.0, tiny),
        ];
        assert!(exact_polygon_double_area(&polygon) > BigInt::from(0_u8));
        let mut reversed = polygon;
        reversed.reverse();
        assert_eq!(
            exact_polygon_double_area(&reversed),
            -exact_polygon_double_area(&polygon)
        );
    }
}
