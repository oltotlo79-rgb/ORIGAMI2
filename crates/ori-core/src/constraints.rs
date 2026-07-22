//! Persistable geometric-constraint records and a finite direct-conflict preflight.
//!
//! The persisted [`GeometricConstraintDocumentV1`] is deliberately separate
//! from [`GeometricConstraintSetV1`]. Deserializing a document is not evidence
//! that its IDs, references, scalar values, or resource use are valid.
//! [`validate_geometric_constraint_document_v1`] establishes the
//! geometry-independent persisted invariants, while
//! [`prepare_geometric_constraints_v1`] additionally establishes reference and
//! geometry invariants against one crease-pattern snapshot.
//!
//! The preflight is intentionally not a geometric solver. A
//! [`ConstraintPreflightV1::NoDirectConflict`] result only says that every
//! direct rule implemented in this module was scanned and found no conflict.
//! It is never a proof that the complete nonlinear constraint system is
//! satisfiable.
//!
//! The V1 count ceilings below bound logical input and output cardinality. They
//! are not an exact heap/RSS budget: standard-library tree nodes and map/vector
//! growth outside the explicit `try_reserve` calls still use the process
//! allocator. [`GeometricConstraintErrorV1::AllocationFailed`] therefore
//! reports only an explicit fallible reservation made by this module and does
//! not promise to convert an operating-system-wide OOM into a recoverable
//! result.

use std::collections::{BTreeMap, BTreeSet};

pub use ori_domain::{
    ConstraintId, DEFAULT_MAX_CONSTRAINT_EDGES, DEFAULT_MAX_CONSTRAINT_RECORDS,
    DEFAULT_MAX_CONSTRAINT_REFERENCES, DEFAULT_MAX_CONSTRAINT_VERTICES,
    GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1, GeometricConstraintDocumentV1,
    GeometricConstraintDocumentValidationErrorV1, GeometricConstraintKindV1,
    GeometricConstraintRecordV1, validate_geometric_constraint_document_v1,
};
use ori_domain::{CreasePattern, Edge, EdgeId, Vertex, VertexId};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Stable semantic identifier for the first geometric-constraint model.
pub const GEOMETRIC_CONSTRAINT_MODEL_ID_V1: &str = "geometric_constraints_v1";

/// Default and non-relaxable V1 preflight-record-count ceiling.
pub const DEFAULT_MAX_CONSTRAINT_PRECHECKS: usize = 10_000;
/// Maximum size of one deterministic direct-conflict cause witness.
pub const MAX_DIRECT_CONFLICT_CAUSE_IDS_V1: usize = 3;

type CanonicalId = [u8; 16];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GeometricConstraintResourceV1 {
    Vertices,
    Edges,
    Constraints,
    References,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GeometricConstraintLimitsV1 {
    /// Callers may tighten this value but cannot raise the V1 hard ceiling.
    pub max_vertices: usize,
    /// Callers may tighten this value but cannot raise the V1 hard ceiling.
    pub max_edges: usize,
    /// Callers may tighten this value but cannot raise the V1 hard ceiling.
    pub max_constraints: usize,
    /// Callers may tighten this value but cannot raise the V1 hard ceiling.
    pub max_references: usize,
    /// Maximum number of constraint records admitted to the direct preflight.
    ///
    /// The implementation indexes every direct rule and does not perform a
    /// quadratic pair scan. If this bound is exceeded, preparation still
    /// succeeds but preflight returns `Unknown(WorkLimitExceeded)` before
    /// examining any constraint. Callers may tighten this value but cannot
    /// raise the V1 hard ceiling.
    pub max_preflight_checks: usize,
}

impl GeometricConstraintLimitsV1 {
    fn effective(self) -> Self {
        Self {
            max_vertices: self.max_vertices.min(DEFAULT_MAX_CONSTRAINT_VERTICES),
            max_edges: self.max_edges.min(DEFAULT_MAX_CONSTRAINT_EDGES),
            max_constraints: self.max_constraints.min(DEFAULT_MAX_CONSTRAINT_RECORDS),
            max_references: self.max_references.min(DEFAULT_MAX_CONSTRAINT_REFERENCES),
            max_preflight_checks: self
                .max_preflight_checks
                .min(DEFAULT_MAX_CONSTRAINT_PRECHECKS),
        }
    }
}

impl Default for GeometricConstraintLimitsV1 {
    fn default() -> Self {
        Self {
            max_vertices: DEFAULT_MAX_CONSTRAINT_VERTICES,
            max_edges: DEFAULT_MAX_CONSTRAINT_EDGES,
            max_constraints: DEFAULT_MAX_CONSTRAINT_RECORDS,
            max_references: DEFAULT_MAX_CONSTRAINT_REFERENCES,
            max_preflight_checks: DEFAULT_MAX_CONSTRAINT_PRECHECKS,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConstraintVertexRoleV1 {
    AngleVertex,
    Point,
    FirstSymmetryPoint,
    SecondSymmetryPoint,
    RotationCenter,
    RotationSource,
    RotationTarget,
    BisectorVertex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConstraintEdgeRoleV1 {
    Target,
    First,
    Second,
    Line,
    SymmetryAxis,
    Bisector,
    Numerator,
    Denominator,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConstraintScalarFieldV1 {
    LengthMillimetres,
    AngleDegrees,
    RotationAngleDegrees,
    Ratio,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum GeometricConstraintErrorV1 {
    #[error("unsupported geometric-constraint schema version {actual}; expected {expected}")]
    UnsupportedSchemaVersion { actual: u32, expected: u32 },
    #[error("{resource:?} count {actual} exceeds the effective V1 maximum {maximum}")]
    ResourceLimitExceeded {
        resource: GeometricConstraintResourceV1,
        actual: usize,
        maximum: usize,
    },
    #[error("the geometric-constraint reference count overflowed")]
    ReferenceCountOverflow,
    #[error("memory for {resource:?} could not be reserved")]
    AllocationFailed {
        resource: GeometricConstraintResourceV1,
    },
    #[error("constraint IDs must not use the nil UUID")]
    NilConstraintId,
    #[error("vertex IDs must not use the nil UUID")]
    NilVertexId,
    #[error("edge IDs must not use the nil UUID")]
    NilEdgeId,
    #[error("constraint {constraint:?} occurs more than once")]
    DuplicateConstraintId { constraint: ConstraintId },
    #[error("vertex {vertex:?} occurs more than once in the geometry registry")]
    DuplicateVertexId { vertex: VertexId },
    #[error("edge {edge:?} occurs more than once in the geometry registry")]
    DuplicateEdgeId { edge: EdgeId },
    #[error("vertex {vertex:?} has a non-finite position")]
    NonFiniteVertexPosition { vertex: VertexId },
    #[error("edge {edge:?} refers to missing endpoint {vertex:?}")]
    EdgeEndpointMissing { edge: EdgeId, vertex: VertexId },
    #[error("edge {edge:?} is degenerate")]
    DegenerateGeometryEdge { edge: EdgeId },
    #[error("constraint {constraint:?} refers to missing {role:?} vertex {vertex:?}")]
    MissingVertex {
        constraint: ConstraintId,
        role: ConstraintVertexRoleV1,
        vertex: VertexId,
    },
    #[error("constraint {constraint:?} refers to missing {role:?} edge {edge:?}")]
    MissingEdge {
        constraint: ConstraintId,
        role: ConstraintEdgeRoleV1,
        edge: EdgeId,
    },
    #[error("constraint {constraint:?} repeats edge {edge:?} in distinct roles")]
    RepeatedEdgeReference {
        constraint: ConstraintId,
        edge: EdgeId,
    },
    #[error(
        "constraint {constraint:?} uses distinct edge IDs {first_edge:?} and {second_edge:?} for the same geometric segment"
    )]
    CoincidentEdgeReferences {
        constraint: ConstraintId,
        first_edge: EdgeId,
        second_edge: EdgeId,
    },
    #[error("constraint {constraint:?} repeats vertex {vertex:?} in distinct roles")]
    RepeatedVertexReference {
        constraint: ConstraintId,
        vertex: VertexId,
    },
    #[error(
        "constraint {constraint:?} uses distinct vertex IDs {first_vertex:?} and {second_vertex:?} at the same position"
    )]
    CoincidentVertexReferences {
        constraint: ConstraintId,
        first_vertex: VertexId,
        second_vertex: VertexId,
    },
    #[error("constraint {constraint:?} requires vertex {vertex:?} to be incident to edge {edge:?}")]
    VertexNotIncidentToEdge {
        constraint: ConstraintId,
        vertex: VertexId,
        edge: EdgeId,
    },
    #[error("constraint {constraint:?} uses line endpoint {vertex:?} as its point-on-line target")]
    PointIsLineEndpoint {
        constraint: ConstraintId,
        vertex: VertexId,
        line_edge: EdgeId,
    },
    #[error("constraint {constraint:?} uses symmetry-axis endpoint {vertex:?} as a mirrored point")]
    SymmetryPointIsAxisEndpoint {
        constraint: ConstraintId,
        vertex: VertexId,
        axis_edge: EdgeId,
    },
    #[error("constraint {constraint:?} has a non-finite {field:?}")]
    NonFiniteValue {
        constraint: ConstraintId,
        field: ConstraintScalarFieldV1,
    },
    #[error("constraint {constraint:?} requires a strictly positive length")]
    NonPositiveLength { constraint: ConstraintId },
    #[error("constraint {constraint:?} requires an angle in the closed range 0 through 180")]
    AngleOutOfRange { constraint: ConstraintId },
    #[error("constraint {constraint:?} requires a rotation angle strictly between 0 and 360")]
    RotationAngleOutOfRange { constraint: ConstraintId },
    #[error("constraint {constraint:?} requires a strictly positive ratio")]
    NonPositiveRatio { constraint: ConstraintId },
}

/// Canonical, reference-validated constraints borrowing one geometry snapshot.
///
/// The borrow prevents safe Rust from mutating or dropping the source pattern
/// while this value exists. It does not carry project/revision authority and is
/// not serializable or clonable. The raw document remains the persistence
/// boundary, and project integration must prepare a fresh set for each current
/// geometry snapshot.
///
/// ```compile_fail
/// use ori_core::{
///     GeometricConstraintDocumentV1, GeometricConstraintLimitsV1,
///     prepare_geometric_constraints_v1,
/// };
/// use ori_domain::CreasePattern;
///
/// let mut pattern = CreasePattern::empty();
/// let document = GeometricConstraintDocumentV1::default();
/// let prepared = prepare_geometric_constraints_v1(
///     &pattern,
///     &document,
///     GeometricConstraintLimitsV1::default(),
/// ).unwrap();
/// pattern.vertices.clear();
/// let _ = prepared.constraints();
/// ```
#[derive(Debug)]
pub struct GeometricConstraintSetV1<'pattern> {
    source_pattern: &'pattern CreasePattern,
    constraints: Vec<GeometricConstraintRecordV1>,
    max_preflight_checks: usize,
}

impl<'pattern> GeometricConstraintSetV1<'pattern> {
    #[must_use]
    pub const fn model_id(&self) -> &'static str {
        GEOMETRIC_CONSTRAINT_MODEL_ID_V1
    }

    /// Records are ordered by canonical constraint-ID bytes. Unordered
    /// geometric operands are normalized independently of storage order.
    #[must_use]
    pub fn constraints(&self) -> &[GeometricConstraintRecordV1] {
        &self.constraints
    }

    /// Returns the exact immutable pattern snapshot borrowed during
    /// preparation.
    #[must_use]
    pub const fn source_pattern(&self) -> &'pattern CreasePattern {
        self.source_pattern
    }

    /// Tests source authority by pointer identity, not merely equal geometry
    /// content.
    #[must_use]
    pub fn is_for_pattern(&self, pattern: &CreasePattern) -> bool {
        std::ptr::eq(self.source_pattern, pattern)
    }

    #[must_use]
    pub fn preflight(&self) -> ConstraintPreflightV1 {
        preflight_direct_conflicts_v1(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DirectConstraintConflictKindV1 {
    DifferentFixedLengths {
        edge: EdgeId,
    },
    DifferentFixedAngles {
        vertex: VertexId,
        first_edge: EdgeId,
        second_edge: EdgeId,
    },
    DifferentLengthRatios {
        numerator_edge: EdgeId,
        denominator_edge: EdgeId,
    },
    HorizontalAndVertical {
        edge: EdgeId,
    },
    EqualLengthWithDifferentFixedLengths {
        first_edge: EdgeId,
        second_edge: EdgeId,
    },
    EqualLengthWithNonUnitRatioAndFixedLength {
        first_edge: EdgeId,
        second_edge: EdgeId,
    },
    ParallelWithFixedNonParallelAngle {
        first_edge: EdgeId,
        second_edge: EdgeId,
    },
    ParallelWithPerpendicularOrientations {
        horizontal_edge: EdgeId,
        vertical_edge: EdgeId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DirectConstraintConflictV1 {
    conflict: DirectConstraintConflictKindV1,
    /// Canonically sorted, duplicate-free minimal witness sufficient for this
    /// direct contradiction. A witness contains at most three IDs, so repeated
    /// authored constraints cannot make preflight output quadratic.
    constraint_ids: Vec<ConstraintId>,
}

impl DirectConstraintConflictV1 {
    #[must_use]
    pub const fn conflict(&self) -> &DirectConstraintConflictKindV1 {
        &self.conflict
    }

    #[must_use]
    pub fn constraint_ids(&self) -> &[ConstraintId] {
        &self.constraint_ids
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GeometricConstraintUnknownReasonV1 {
    WorkLimitExceeded,
    SolverRequiredConstraintKinds,
}

/// Result of the finite direct-conflict scan.
///
/// `NoDirectConflict` is deliberately named narrowly. It is not `Solved` and
/// not a global satisfiability certificate. This is a native-produced output
/// DTO, not a deserializable certificate.
///
/// ```compile_fail
/// let _: ori_core::ConstraintPreflightV1 =
///     serde_json::from_str(r#"{"status":"no_direct_conflict"}"#).unwrap();
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ConstraintPreflightV1 {
    DirectConflict {
        conflicts: Vec<DirectConstraintConflictV1>,
    },
    NoDirectConflict,
    Unknown {
        reason: GeometricConstraintUnknownReasonV1,
        unchecked_constraint_ids: Vec<ConstraintId>,
    },
}

#[derive(Clone, Copy)]
struct GeometryRegistry<'a> {
    vertices: &'a BTreeMap<CanonicalId, &'a Vertex>,
    edges: &'a BTreeMap<CanonicalId, &'a Edge>,
}

/// Validates and canonicalizes a persisted constraint document against one
/// geometry snapshot.
///
/// An empty V1 document has no geometry references, so it is admitted without
/// scanning or imposing constraint-specific vertex and edge ceilings on the
/// borrowed pattern. The schema and constraint-count ceiling are still
/// checked. The first non-empty document performs the full bounded geometry
/// validation below.
pub fn prepare_geometric_constraints_v1<'pattern>(
    pattern: &'pattern CreasePattern,
    document: &GeometricConstraintDocumentV1,
    limits: GeometricConstraintLimitsV1,
) -> Result<GeometricConstraintSetV1<'pattern>, GeometricConstraintErrorV1> {
    if document.schema_version != GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1 {
        return Err(GeometricConstraintErrorV1::UnsupportedSchemaVersion {
            actual: document.schema_version,
            expected: GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
        });
    }
    let limits = limits.effective();
    check_resource(
        GeometricConstraintResourceV1::Constraints,
        document.constraints.len(),
        limits.max_constraints,
    )?;
    if document.constraints.is_empty() {
        return Ok(GeometricConstraintSetV1 {
            source_pattern: pattern,
            constraints: Vec::new(),
            max_preflight_checks: limits.max_preflight_checks,
        });
    }
    check_resource(
        GeometricConstraintResourceV1::Vertices,
        pattern.vertices.len(),
        limits.max_vertices,
    )?;
    check_resource(
        GeometricConstraintResourceV1::Edges,
        pattern.edges.len(),
        limits.max_edges,
    )?;
    let vertices = prepare_vertex_registry(&pattern.vertices)?;
    let edges = prepare_edge_registry(&pattern.edges, &vertices)?;
    let registry = GeometryRegistry {
        vertices: &vertices,
        edges: &edges,
    };

    let reference_count = document
        .constraints
        .iter()
        .try_fold(0usize, |count, record| {
            count.checked_add(record.constraint.reference_count())
        })
        .ok_or(GeometricConstraintErrorV1::ReferenceCountOverflow)?;
    check_resource(
        GeometricConstraintResourceV1::References,
        reference_count,
        limits.max_references,
    )?;

    let mut ordered = Vec::new();
    ordered
        .try_reserve_exact(document.constraints.len())
        .map_err(|_| GeometricConstraintErrorV1::AllocationFailed {
            resource: GeometricConstraintResourceV1::Constraints,
        })?;
    ordered.extend(document.constraints.iter());
    ordered.sort_unstable_by_key(|record| record.id.canonical_bytes());
    for pair in ordered.windows(2) {
        if pair[0].id == pair[1].id {
            return Err(GeometricConstraintErrorV1::DuplicateConstraintId {
                constraint: pair[1].id,
            });
        }
    }

    let mut constraints = Vec::new();
    constraints.try_reserve_exact(ordered.len()).map_err(|_| {
        GeometricConstraintErrorV1::AllocationFailed {
            resource: GeometricConstraintResourceV1::Constraints,
        }
    })?;
    for record in ordered {
        if record.id.canonical_bytes() == [0; 16] {
            return Err(GeometricConstraintErrorV1::NilConstraintId);
        }
        let normalized = GeometricConstraintRecordV1 {
            id: record.id,
            constraint: normalize_constraint(record.constraint.clone()),
        };
        validate_constraint(&normalized, registry)?;
        constraints.push(normalized);
    }
    // Run the geometry-independent persistence validator only after the
    // existing geometry and per-record checks. This reuses the low-level
    // contract without changing ori-core's established error precedence.
    validate_geometric_constraint_document_v1(document).map_err(map_persisted_document_error)?;

    Ok(GeometricConstraintSetV1 {
        source_pattern: pattern,
        constraints,
        max_preflight_checks: limits.max_preflight_checks,
    })
}

/// Validates one prospective record against only the geometry it references.
///
/// This is the editor admission boundary for adding a constraint to a
/// repairable document. Geometry-independent persisted invariants are checked
/// first. The geometry snapshot is then reduced to directly referenced
/// vertices, directly referenced edges, and the endpoints of those edges
/// before the ordinary V1 preparation contract is applied. Consequently,
/// malformed geometry that the new record cannot observe does not prevent the
/// record from being admitted, while duplicate IDs and malformed endpoints in
/// its dependency closure remain visible.
pub fn validate_geometric_constraint_record_against_pattern_v1(
    pattern: &CreasePattern,
    record: &GeometricConstraintRecordV1,
) -> Result<(), GeometricConstraintErrorV1> {
    let document = GeometricConstraintDocumentV1 {
        schema_version: GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
        constraints: vec![record.clone()],
    };
    validate_geometric_constraint_document_v1(&document).map_err(map_persisted_document_error)?;

    let mut referenced_vertices = BTreeSet::new();
    let mut referenced_edges = BTreeSet::new();
    collect_constraint_references(
        &record.constraint,
        &mut referenced_vertices,
        &mut referenced_edges,
    );

    let mut edge_indices = BTreeSet::new();
    for (index, edge) in pattern.edges.iter().enumerate() {
        if referenced_edges.contains(&edge.id.canonical_bytes()) {
            edge_indices.insert(index);
            referenced_vertices.insert(edge.start.canonical_bytes());
            referenced_vertices.insert(edge.end.canonical_bytes());
        }
    }
    let vertex_indices = pattern
        .vertices
        .iter()
        .enumerate()
        .filter_map(|(index, vertex)| {
            referenced_vertices
                .contains(&vertex.id.canonical_bytes())
                .then_some(index)
        })
        .collect::<BTreeSet<_>>();

    let relevant_pattern = CreasePattern {
        vertices: vertex_indices
            .into_iter()
            .map(|index| pattern.vertices[index].clone())
            .collect(),
        edges: edge_indices
            .into_iter()
            .map(|index| pattern.edges[index].clone())
            .collect(),
    };
    prepare_geometric_constraints_v1(
        &relevant_pattern,
        &document,
        GeometricConstraintLimitsV1::default(),
    )
    .map(|_| ())
}

fn collect_constraint_references(
    constraint: &GeometricConstraintKindV1,
    vertices: &mut BTreeSet<CanonicalId>,
    edges: &mut BTreeSet<CanonicalId>,
) {
    let mut vertex = |id: VertexId| {
        vertices.insert(id.canonical_bytes());
    };
    let mut edge = |id: EdgeId| {
        edges.insert(id.canonical_bytes());
    };
    match *constraint {
        GeometricConstraintKindV1::FixedLength { edge: target, .. }
        | GeometricConstraintKindV1::Horizontal { edge: target }
        | GeometricConstraintKindV1::Vertical { edge: target } => edge(target),
        GeometricConstraintKindV1::FixedAngle {
            vertex: angle_vertex,
            first_edge,
            second_edge,
            ..
        } => {
            vertex(angle_vertex);
            edge(first_edge);
            edge(second_edge);
        }
        GeometricConstraintKindV1::EqualLength {
            first_edge,
            second_edge,
        }
        | GeometricConstraintKindV1::Parallel {
            first_edge,
            second_edge,
        } => {
            edge(first_edge);
            edge(second_edge);
        }
        GeometricConstraintKindV1::PointOnLine {
            vertex: point,
            line_edge,
        } => {
            vertex(point);
            edge(line_edge);
        }
        GeometricConstraintKindV1::MirrorSymmetry {
            first_vertex,
            second_vertex,
            axis_edge,
        } => {
            vertex(first_vertex);
            vertex(second_vertex);
            edge(axis_edge);
        }
        GeometricConstraintKindV1::RotationalSymmetry {
            center_vertex,
            source_vertex,
            target_vertex,
            ..
        } => {
            vertex(center_vertex);
            vertex(source_vertex);
            vertex(target_vertex);
        }
        GeometricConstraintKindV1::AngleBisector {
            vertex: angle_vertex,
            first_edge,
            second_edge,
            bisector_edge,
        } => {
            vertex(angle_vertex);
            edge(first_edge);
            edge(second_edge);
            edge(bisector_edge);
        }
        GeometricConstraintKindV1::LengthRatio {
            numerator_edge,
            denominator_edge,
            ..
        } => {
            edge(numerator_edge);
            edge(denominator_edge);
        }
    }
}

fn check_resource(
    resource: GeometricConstraintResourceV1,
    actual: usize,
    maximum: usize,
) -> Result<(), GeometricConstraintErrorV1> {
    if actual > maximum {
        Err(GeometricConstraintErrorV1::ResourceLimitExceeded {
            resource,
            actual,
            maximum,
        })
    } else {
        Ok(())
    }
}

fn map_persisted_document_error(
    error: GeometricConstraintDocumentValidationErrorV1,
) -> GeometricConstraintErrorV1 {
    match error {
        GeometricConstraintDocumentValidationErrorV1::UnsupportedSchemaVersion {
            actual,
            expected,
        } => GeometricConstraintErrorV1::UnsupportedSchemaVersion { actual, expected },
        GeometricConstraintDocumentValidationErrorV1::TooManyConstraints { actual, maximum } => {
            GeometricConstraintErrorV1::ResourceLimitExceeded {
                resource: GeometricConstraintResourceV1::Constraints,
                actual,
                maximum,
            }
        }
        GeometricConstraintDocumentValidationErrorV1::TooManyReferences { actual, maximum } => {
            GeometricConstraintErrorV1::ResourceLimitExceeded {
                resource: GeometricConstraintResourceV1::References,
                actual,
                maximum,
            }
        }
        GeometricConstraintDocumentValidationErrorV1::ReferenceCountOverflow => {
            GeometricConstraintErrorV1::ReferenceCountOverflow
        }
        GeometricConstraintDocumentValidationErrorV1::AllocationFailed => {
            GeometricConstraintErrorV1::AllocationFailed {
                resource: GeometricConstraintResourceV1::Constraints,
            }
        }
        GeometricConstraintDocumentValidationErrorV1::NilConstraintId => {
            GeometricConstraintErrorV1::NilConstraintId
        }
        GeometricConstraintDocumentValidationErrorV1::DuplicateConstraintId { constraint } => {
            GeometricConstraintErrorV1::DuplicateConstraintId { constraint }
        }
        GeometricConstraintDocumentValidationErrorV1::NilVertexReference { .. } => {
            GeometricConstraintErrorV1::NilVertexId
        }
        GeometricConstraintDocumentValidationErrorV1::NilEdgeReference { .. } => {
            GeometricConstraintErrorV1::NilEdgeId
        }
        GeometricConstraintDocumentValidationErrorV1::RepeatedVertexReference {
            constraint,
            vertex,
        } => GeometricConstraintErrorV1::RepeatedVertexReference { constraint, vertex },
        GeometricConstraintDocumentValidationErrorV1::RepeatedEdgeReference {
            constraint,
            edge,
        } => GeometricConstraintErrorV1::RepeatedEdgeReference { constraint, edge },
        GeometricConstraintDocumentValidationErrorV1::NonFiniteFixedLength { constraint } => {
            GeometricConstraintErrorV1::NonFiniteValue {
                constraint,
                field: ConstraintScalarFieldV1::LengthMillimetres,
            }
        }
        GeometricConstraintDocumentValidationErrorV1::NonPositiveFixedLength { constraint } => {
            GeometricConstraintErrorV1::NonPositiveLength { constraint }
        }
        GeometricConstraintDocumentValidationErrorV1::NonFiniteFixedAngle { constraint } => {
            GeometricConstraintErrorV1::NonFiniteValue {
                constraint,
                field: ConstraintScalarFieldV1::AngleDegrees,
            }
        }
        GeometricConstraintDocumentValidationErrorV1::FixedAngleOutOfRange { constraint } => {
            GeometricConstraintErrorV1::AngleOutOfRange { constraint }
        }
        GeometricConstraintDocumentValidationErrorV1::NonFiniteRotationAngle { constraint } => {
            GeometricConstraintErrorV1::NonFiniteValue {
                constraint,
                field: ConstraintScalarFieldV1::RotationAngleDegrees,
            }
        }
        GeometricConstraintDocumentValidationErrorV1::RotationAngleOutOfRange { constraint } => {
            GeometricConstraintErrorV1::RotationAngleOutOfRange { constraint }
        }
        GeometricConstraintDocumentValidationErrorV1::NonFiniteLengthRatio { constraint } => {
            GeometricConstraintErrorV1::NonFiniteValue {
                constraint,
                field: ConstraintScalarFieldV1::Ratio,
            }
        }
        GeometricConstraintDocumentValidationErrorV1::NonPositiveLengthRatio { constraint } => {
            GeometricConstraintErrorV1::NonPositiveRatio { constraint }
        }
    }
}

fn prepare_vertex_registry(
    source: &[Vertex],
) -> Result<BTreeMap<CanonicalId, &Vertex>, GeometricConstraintErrorV1> {
    let mut ordered = Vec::new();
    ordered.try_reserve_exact(source.len()).map_err(|_| {
        GeometricConstraintErrorV1::AllocationFailed {
            resource: GeometricConstraintResourceV1::Vertices,
        }
    })?;
    ordered.extend(source);
    ordered.sort_unstable_by_key(|vertex| vertex.id.canonical_bytes());
    for pair in ordered.windows(2) {
        if pair[0].id == pair[1].id {
            return Err(GeometricConstraintErrorV1::DuplicateVertexId { vertex: pair[1].id });
        }
    }
    for vertex in &ordered {
        if vertex.id.canonical_bytes() == [0; 16] {
            return Err(GeometricConstraintErrorV1::NilVertexId);
        }
        if !vertex.position.x.is_finite() || !vertex.position.y.is_finite() {
            return Err(GeometricConstraintErrorV1::NonFiniteVertexPosition { vertex: vertex.id });
        }
    }
    Ok(ordered
        .into_iter()
        .map(|vertex| (vertex.id.canonical_bytes(), vertex))
        .collect())
}

fn prepare_edge_registry<'a>(
    source: &'a [Edge],
    vertices: &BTreeMap<CanonicalId, &'a Vertex>,
) -> Result<BTreeMap<CanonicalId, &'a Edge>, GeometricConstraintErrorV1> {
    let mut ordered = Vec::new();
    ordered.try_reserve_exact(source.len()).map_err(|_| {
        GeometricConstraintErrorV1::AllocationFailed {
            resource: GeometricConstraintResourceV1::Edges,
        }
    })?;
    ordered.extend(source);
    ordered.sort_unstable_by_key(|edge| edge.id.canonical_bytes());
    for pair in ordered.windows(2) {
        if pair[0].id == pair[1].id {
            return Err(GeometricConstraintErrorV1::DuplicateEdgeId { edge: pair[1].id });
        }
    }
    for edge in &ordered {
        if edge.id.canonical_bytes() == [0; 16] {
            return Err(GeometricConstraintErrorV1::NilEdgeId);
        }
        let start = vertices.get(&edge.start.canonical_bytes()).ok_or(
            GeometricConstraintErrorV1::EdgeEndpointMissing {
                edge: edge.id,
                vertex: edge.start,
            },
        )?;
        let end = vertices.get(&edge.end.canonical_bytes()).ok_or(
            GeometricConstraintErrorV1::EdgeEndpointMissing {
                edge: edge.id,
                vertex: edge.end,
            },
        )?;
        if edge.start == edge.end
            || (start.position.x == end.position.x && start.position.y == end.position.y)
        {
            return Err(GeometricConstraintErrorV1::DegenerateGeometryEdge { edge: edge.id });
        }
    }
    Ok(ordered
        .into_iter()
        .map(|edge| (edge.id.canonical_bytes(), edge))
        .collect())
}

fn validate_constraint(
    record: &GeometricConstraintRecordV1,
    registry: GeometryRegistry<'_>,
) -> Result<(), GeometricConstraintErrorV1> {
    let constraint = record.id;
    match &record.constraint {
        GeometricConstraintKindV1::FixedLength { edge, length_mm } => {
            require_edge(registry, constraint, ConstraintEdgeRoleV1::Target, *edge)?;
            require_finite(
                constraint,
                ConstraintScalarFieldV1::LengthMillimetres,
                *length_mm,
            )?;
            if *length_mm <= 0.0 {
                return Err(GeometricConstraintErrorV1::NonPositiveLength { constraint });
            }
        }
        GeometricConstraintKindV1::FixedAngle {
            vertex,
            first_edge,
            second_edge,
            angle_degrees,
        } => {
            require_distinct_edges(constraint, *first_edge, *second_edge)?;
            require_vertex(
                registry,
                constraint,
                ConstraintVertexRoleV1::AngleVertex,
                *vertex,
            )?;
            require_incident_edge(
                registry,
                constraint,
                *vertex,
                ConstraintEdgeRoleV1::First,
                *first_edge,
            )?;
            require_incident_edge(
                registry,
                constraint,
                *vertex,
                ConstraintEdgeRoleV1::Second,
                *second_edge,
            )?;
            require_distinct_edge_segments(registry, constraint, *first_edge, *second_edge)?;
            require_finite(
                constraint,
                ConstraintScalarFieldV1::AngleDegrees,
                *angle_degrees,
            )?;
            if !(0.0..=180.0).contains(angle_degrees) {
                return Err(GeometricConstraintErrorV1::AngleOutOfRange { constraint });
            }
        }
        GeometricConstraintKindV1::Horizontal { edge }
        | GeometricConstraintKindV1::Vertical { edge } => {
            require_edge(registry, constraint, ConstraintEdgeRoleV1::Target, *edge)?;
        }
        GeometricConstraintKindV1::EqualLength {
            first_edge,
            second_edge,
        }
        | GeometricConstraintKindV1::Parallel {
            first_edge,
            second_edge,
        } => {
            require_distinct_edges(constraint, *first_edge, *second_edge)?;
            require_edge(
                registry,
                constraint,
                ConstraintEdgeRoleV1::First,
                *first_edge,
            )?;
            require_edge(
                registry,
                constraint,
                ConstraintEdgeRoleV1::Second,
                *second_edge,
            )?;
            require_distinct_edge_segments(registry, constraint, *first_edge, *second_edge)?;
        }
        GeometricConstraintKindV1::PointOnLine { vertex, line_edge } => {
            let point =
                require_vertex(registry, constraint, ConstraintVertexRoleV1::Point, *vertex)?;
            let edge = require_edge(registry, constraint, ConstraintEdgeRoleV1::Line, *line_edge)?;
            if edge.start == *vertex
                || edge.end == *vertex
                || edge_endpoint_vertices(registry, edge)
                    .into_iter()
                    .any(|endpoint| same_position(point, endpoint))
            {
                return Err(GeometricConstraintErrorV1::PointIsLineEndpoint {
                    constraint,
                    vertex: *vertex,
                    line_edge: *line_edge,
                });
            }
        }
        GeometricConstraintKindV1::MirrorSymmetry {
            first_vertex,
            second_vertex,
            axis_edge,
        } => {
            require_distinct_vertices(constraint, *first_vertex, *second_vertex)?;
            let first = require_vertex(
                registry,
                constraint,
                ConstraintVertexRoleV1::FirstSymmetryPoint,
                *first_vertex,
            )?;
            let second = require_vertex(
                registry,
                constraint,
                ConstraintVertexRoleV1::SecondSymmetryPoint,
                *second_vertex,
            )?;
            let axis = require_edge(
                registry,
                constraint,
                ConstraintEdgeRoleV1::SymmetryAxis,
                *axis_edge,
            )?;
            if same_position(first, second) {
                return Err(GeometricConstraintErrorV1::CoincidentVertexReferences {
                    constraint,
                    first_vertex: *first_vertex,
                    second_vertex: *second_vertex,
                });
            }
            for vertex in [*first_vertex, *second_vertex] {
                let point = registry.vertices[&vertex.canonical_bytes()];
                if axis.start == vertex
                    || axis.end == vertex
                    || edge_endpoint_vertices(registry, axis)
                        .into_iter()
                        .any(|endpoint| same_position(point, endpoint))
                {
                    return Err(GeometricConstraintErrorV1::SymmetryPointIsAxisEndpoint {
                        constraint,
                        vertex,
                        axis_edge: *axis_edge,
                    });
                }
            }
        }
        GeometricConstraintKindV1::RotationalSymmetry {
            center_vertex,
            source_vertex,
            target_vertex,
            angle_degrees,
        } => {
            require_distinct_vertices(constraint, *center_vertex, *source_vertex)?;
            require_distinct_vertices(constraint, *center_vertex, *target_vertex)?;
            require_distinct_vertices(constraint, *source_vertex, *target_vertex)?;
            require_vertex(
                registry,
                constraint,
                ConstraintVertexRoleV1::RotationCenter,
                *center_vertex,
            )?;
            require_vertex(
                registry,
                constraint,
                ConstraintVertexRoleV1::RotationSource,
                *source_vertex,
            )?;
            require_vertex(
                registry,
                constraint,
                ConstraintVertexRoleV1::RotationTarget,
                *target_vertex,
            )?;
            require_distinct_vertex_positions(
                registry,
                constraint,
                *center_vertex,
                *source_vertex,
            )?;
            require_distinct_vertex_positions(
                registry,
                constraint,
                *center_vertex,
                *target_vertex,
            )?;
            require_distinct_vertex_positions(
                registry,
                constraint,
                *source_vertex,
                *target_vertex,
            )?;
            require_finite(
                constraint,
                ConstraintScalarFieldV1::RotationAngleDegrees,
                *angle_degrees,
            )?;
            if *angle_degrees <= 0.0 || *angle_degrees >= 360.0 {
                return Err(GeometricConstraintErrorV1::RotationAngleOutOfRange { constraint });
            }
        }
        GeometricConstraintKindV1::AngleBisector {
            vertex,
            first_edge,
            second_edge,
            bisector_edge,
        } => {
            require_all_distinct_edges(constraint, [*first_edge, *second_edge, *bisector_edge])?;
            require_vertex(
                registry,
                constraint,
                ConstraintVertexRoleV1::BisectorVertex,
                *vertex,
            )?;
            for (role, edge) in [
                (ConstraintEdgeRoleV1::First, *first_edge),
                (ConstraintEdgeRoleV1::Second, *second_edge),
                (ConstraintEdgeRoleV1::Bisector, *bisector_edge),
            ] {
                require_incident_edge(registry, constraint, *vertex, role, edge)?;
            }
            require_distinct_edge_segments(registry, constraint, *first_edge, *second_edge)?;
            require_distinct_edge_segments(registry, constraint, *first_edge, *bisector_edge)?;
            require_distinct_edge_segments(registry, constraint, *second_edge, *bisector_edge)?;
        }
        GeometricConstraintKindV1::LengthRatio {
            numerator_edge,
            denominator_edge,
            ratio,
        } => {
            require_distinct_edges(constraint, *numerator_edge, *denominator_edge)?;
            require_edge(
                registry,
                constraint,
                ConstraintEdgeRoleV1::Numerator,
                *numerator_edge,
            )?;
            require_edge(
                registry,
                constraint,
                ConstraintEdgeRoleV1::Denominator,
                *denominator_edge,
            )?;
            require_distinct_edge_segments(
                registry,
                constraint,
                *numerator_edge,
                *denominator_edge,
            )?;
            require_finite(constraint, ConstraintScalarFieldV1::Ratio, *ratio)?;
            if *ratio <= 0.0 {
                return Err(GeometricConstraintErrorV1::NonPositiveRatio { constraint });
            }
        }
    }
    Ok(())
}

fn require_vertex<'a>(
    registry: GeometryRegistry<'a>,
    constraint: ConstraintId,
    role: ConstraintVertexRoleV1,
    vertex: VertexId,
) -> Result<&'a Vertex, GeometricConstraintErrorV1> {
    registry
        .vertices
        .get(&vertex.canonical_bytes())
        .copied()
        .ok_or(GeometricConstraintErrorV1::MissingVertex {
            constraint,
            role,
            vertex,
        })
}

fn require_edge<'a>(
    registry: GeometryRegistry<'a>,
    constraint: ConstraintId,
    role: ConstraintEdgeRoleV1,
    edge: EdgeId,
) -> Result<&'a Edge, GeometricConstraintErrorV1> {
    registry.edges.get(&edge.canonical_bytes()).copied().ok_or(
        GeometricConstraintErrorV1::MissingEdge {
            constraint,
            role,
            edge,
        },
    )
}

fn require_incident_edge(
    registry: GeometryRegistry<'_>,
    constraint: ConstraintId,
    vertex: VertexId,
    role: ConstraintEdgeRoleV1,
    edge: EdgeId,
) -> Result<(), GeometricConstraintErrorV1> {
    let referenced = require_edge(registry, constraint, role, edge)?;
    if referenced.start == vertex || referenced.end == vertex {
        Ok(())
    } else {
        Err(GeometricConstraintErrorV1::VertexNotIncidentToEdge {
            constraint,
            vertex,
            edge,
        })
    }
}

fn require_distinct_edges(
    constraint: ConstraintId,
    first: EdgeId,
    second: EdgeId,
) -> Result<(), GeometricConstraintErrorV1> {
    if first == second {
        Err(GeometricConstraintErrorV1::RepeatedEdgeReference {
            constraint,
            edge: first,
        })
    } else {
        Ok(())
    }
}

fn require_all_distinct_edges(
    constraint: ConstraintId,
    edges: [EdgeId; 3],
) -> Result<(), GeometricConstraintErrorV1> {
    require_distinct_edges(constraint, edges[0], edges[1])?;
    require_distinct_edges(constraint, edges[0], edges[2])?;
    require_distinct_edges(constraint, edges[1], edges[2])
}

fn require_distinct_edge_segments(
    registry: GeometryRegistry<'_>,
    constraint: ConstraintId,
    first_edge: EdgeId,
    second_edge: EdgeId,
) -> Result<(), GeometricConstraintErrorV1> {
    let first = registry.edges[&first_edge.canonical_bytes()];
    let second = registry.edges[&second_edge.canonical_bytes()];
    let [first_start, first_end] = edge_endpoint_vertices(registry, first);
    let [second_start, second_end] = edge_endpoint_vertices(registry, second);
    if (same_position(first_start, second_start) && same_position(first_end, second_end))
        || (same_position(first_start, second_end) && same_position(first_end, second_start))
    {
        Err(GeometricConstraintErrorV1::CoincidentEdgeReferences {
            constraint,
            first_edge,
            second_edge,
        })
    } else {
        Ok(())
    }
}

fn edge_endpoint_vertices<'a>(registry: GeometryRegistry<'a>, edge: &Edge) -> [&'a Vertex; 2] {
    [
        registry.vertices[&edge.start.canonical_bytes()],
        registry.vertices[&edge.end.canonical_bytes()],
    ]
}

fn require_distinct_vertices(
    constraint: ConstraintId,
    first: VertexId,
    second: VertexId,
) -> Result<(), GeometricConstraintErrorV1> {
    if first == second {
        Err(GeometricConstraintErrorV1::RepeatedVertexReference {
            constraint,
            vertex: first,
        })
    } else {
        Ok(())
    }
}

fn require_distinct_vertex_positions(
    registry: GeometryRegistry<'_>,
    constraint: ConstraintId,
    first_vertex: VertexId,
    second_vertex: VertexId,
) -> Result<(), GeometricConstraintErrorV1> {
    let first = registry.vertices[&first_vertex.canonical_bytes()];
    let second = registry.vertices[&second_vertex.canonical_bytes()];
    if same_position(first, second) {
        Err(GeometricConstraintErrorV1::CoincidentVertexReferences {
            constraint,
            first_vertex,
            second_vertex,
        })
    } else {
        Ok(())
    }
}

fn same_position(first: &Vertex, second: &Vertex) -> bool {
    first.position.x == second.position.x && first.position.y == second.position.y
}

fn require_finite(
    constraint: ConstraintId,
    field: ConstraintScalarFieldV1,
    value: f64,
) -> Result<(), GeometricConstraintErrorV1> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(GeometricConstraintErrorV1::NonFiniteValue { constraint, field })
    }
}

fn normalize_constraint(mut constraint: GeometricConstraintKindV1) -> GeometricConstraintKindV1 {
    match &mut constraint {
        GeometricConstraintKindV1::FixedAngle {
            first_edge,
            second_edge,
            angle_degrees,
            ..
        } => {
            canonicalize_unordered_pair(first_edge, second_edge);
            *angle_degrees = canonical_zero(*angle_degrees);
        }
        GeometricConstraintKindV1::EqualLength {
            first_edge,
            second_edge,
        }
        | GeometricConstraintKindV1::Parallel {
            first_edge,
            second_edge,
        } => canonicalize_unordered_pair(first_edge, second_edge),
        GeometricConstraintKindV1::MirrorSymmetry {
            first_vertex,
            second_vertex,
            ..
        } => {
            if first_vertex.canonical_bytes() > second_vertex.canonical_bytes() {
                std::mem::swap(first_vertex, second_vertex);
            }
        }
        GeometricConstraintKindV1::AngleBisector {
            first_edge,
            second_edge,
            ..
        } => canonicalize_unordered_pair(first_edge, second_edge),
        GeometricConstraintKindV1::FixedLength { length_mm, .. } => {
            *length_mm = canonical_zero(*length_mm);
        }
        GeometricConstraintKindV1::RotationalSymmetry { angle_degrees, .. } => {
            *angle_degrees = canonical_zero(*angle_degrees);
        }
        GeometricConstraintKindV1::LengthRatio { ratio, .. } => {
            *ratio = canonical_zero(*ratio);
        }
        GeometricConstraintKindV1::Horizontal { .. }
        | GeometricConstraintKindV1::Vertical { .. }
        | GeometricConstraintKindV1::PointOnLine { .. } => {}
    }
    constraint
}

fn canonicalize_unordered_pair(first: &mut EdgeId, second: &mut EdgeId) {
    if first.canonical_bytes() > second.canonical_bytes() {
        std::mem::swap(first, second);
    }
}

fn canonical_zero(value: f64) -> f64 {
    if value == 0.0 { 0.0 } else { value }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct EdgePairKey {
    first: CanonicalId,
    second: CanonicalId,
}

impl EdgePairKey {
    fn unordered(first: EdgeId, second: EdgeId) -> Self {
        let first_bytes = first.canonical_bytes();
        let second_bytes = second.canonical_bytes();
        if first_bytes < second_bytes {
            Self {
                first: first_bytes,
                second: second_bytes,
            }
        } else {
            Self {
                first: second_bytes,
                second: first_bytes,
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct AngleKey {
    vertex: CanonicalId,
    edges: EdgePairKey,
}

#[derive(Debug, Clone, Copy)]
struct ScalarAssignment {
    id: ConstraintId,
    value: f64,
}

#[derive(Debug, Clone, Copy)]
struct ScalarGroupSummary {
    representative: ScalarAssignment,
    first_different: Option<ScalarAssignment>,
}

impl ScalarGroupSummary {
    fn new(representative: ScalarAssignment) -> Self {
        #[cfg(test)]
        record_fixed_length_summary_visit();
        Self {
            representative,
            first_different: None,
        }
    }

    fn observe(&mut self, assignment: ScalarAssignment) {
        #[cfg(test)]
        record_fixed_length_summary_visit();
        if self.first_different.is_none()
            && assignment.value.to_bits() != self.representative.value.to_bits()
        {
            self.first_different = Some(assignment);
        }
    }

    fn different_witness(&self) -> Option<[ConstraintId; 2]> {
        self.first_different
            .map(|different| [self.representative.id, different.id])
    }

    fn consistent_assignment(&self) -> Option<ScalarAssignment> {
        self.first_different
            .is_none()
            .then_some(self.representative)
    }
}

#[cfg(test)]
std::thread_local! {
    static FIXED_LENGTH_SUMMARY_VISITS: std::cell::Cell<Option<usize>> =
        const { std::cell::Cell::new(None) };
}

#[cfg(test)]
fn record_fixed_length_summary_visit() {
    FIXED_LENGTH_SUMMARY_VISITS.with(|visits| {
        if let Some(current) = visits.get() {
            visits.set(Some(
                current
                    .checked_add(1)
                    .expect("test-only fixed-length summary counter overflow"),
            ));
        }
    });
}

#[cfg(test)]
fn begin_fixed_length_summary_visit_count() {
    FIXED_LENGTH_SUMMARY_VISITS.with(|visits| {
        assert_eq!(
            visits.replace(Some(0)),
            None,
            "fixed-length summary counter is already active on this test thread"
        );
    });
}

#[cfg(test)]
fn finish_fixed_length_summary_visit_count() -> usize {
    FIXED_LENGTH_SUMMARY_VISITS.with(|visits| {
        visits
            .replace(None)
            .expect("fixed-length summary counter was not active")
    })
}

/// Exhaustively scans the finite set of direct contradiction rules.
///
/// With `N` prepared records, total time is `O(N log N + output)` and storage
/// is `O(N + output)`. The deterministic output sort is absorbed by the
/// `O(N log N)` term because conflict output is itself linear in `N`.
/// Fixed-length assignments are summarized during the single canonical record
/// pass, so reusing one edge from many equal-length constraints never causes a
/// cross-product rescan of fixed-length groups and equal-length pairs.
#[must_use]
pub fn preflight_direct_conflicts_v1(set: &GeometricConstraintSetV1<'_>) -> ConstraintPreflightV1 {
    if set.constraints.len() > set.max_preflight_checks {
        return ConstraintPreflightV1::Unknown {
            reason: GeometricConstraintUnknownReasonV1::WorkLimitExceeded,
            unchecked_constraint_ids: canonical_constraint_ids(&set.constraints),
        };
    }

    let mut fixed_lengths: BTreeMap<CanonicalId, ScalarGroupSummary> = BTreeMap::new();
    let mut fixed_angles: BTreeMap<AngleKey, Vec<ScalarAssignment>> = BTreeMap::new();
    let mut fixed_angles_by_pair: BTreeMap<EdgePairKey, Vec<ScalarAssignment>> = BTreeMap::new();
    let mut ratios: BTreeMap<(CanonicalId, CanonicalId), Vec<ScalarAssignment>> = BTreeMap::new();
    let mut horizontal: BTreeMap<CanonicalId, Vec<ConstraintId>> = BTreeMap::new();
    let mut vertical: BTreeMap<CanonicalId, Vec<ConstraintId>> = BTreeMap::new();
    let mut equal_lengths: BTreeMap<EdgePairKey, Vec<ConstraintId>> = BTreeMap::new();
    let mut parallels: BTreeMap<EdgePairKey, Vec<ConstraintId>> = BTreeMap::new();
    let mut unchecked = Vec::new();

    for record in &set.constraints {
        match &record.constraint {
            GeometricConstraintKindV1::FixedLength { edge, length_mm } => {
                let assignment = ScalarAssignment {
                    id: record.id,
                    value: *length_mm,
                };
                fixed_lengths
                    .entry(edge.canonical_bytes())
                    .and_modify(|summary| summary.observe(assignment))
                    .or_insert_with(|| ScalarGroupSummary::new(assignment));
            }
            GeometricConstraintKindV1::FixedAngle {
                vertex,
                first_edge,
                second_edge,
                angle_degrees,
            } => {
                let edges = EdgePairKey::unordered(*first_edge, *second_edge);
                fixed_angles
                    .entry(AngleKey {
                        vertex: vertex.canonical_bytes(),
                        edges,
                    })
                    .or_default()
                    .push(ScalarAssignment {
                        id: record.id,
                        value: *angle_degrees,
                    });
                fixed_angles_by_pair
                    .entry(edges)
                    .or_default()
                    .push(ScalarAssignment {
                        id: record.id,
                        value: *angle_degrees,
                    });
                unchecked.push(record.id);
            }
            GeometricConstraintKindV1::Horizontal { edge } => {
                horizontal
                    .entry(edge.canonical_bytes())
                    .or_default()
                    .push(record.id);
            }
            GeometricConstraintKindV1::Vertical { edge } => {
                vertical
                    .entry(edge.canonical_bytes())
                    .or_default()
                    .push(record.id);
            }
            GeometricConstraintKindV1::EqualLength {
                first_edge,
                second_edge,
            } => {
                equal_lengths
                    .entry(EdgePairKey::unordered(*first_edge, *second_edge))
                    .or_default()
                    .push(record.id);
                unchecked.push(record.id);
            }
            GeometricConstraintKindV1::Parallel {
                first_edge,
                second_edge,
            } => {
                parallels
                    .entry(EdgePairKey::unordered(*first_edge, *second_edge))
                    .or_default()
                    .push(record.id);
                unchecked.push(record.id);
            }
            GeometricConstraintKindV1::LengthRatio {
                numerator_edge,
                denominator_edge,
                ratio,
            } => {
                ratios
                    .entry((
                        numerator_edge.canonical_bytes(),
                        denominator_edge.canonical_bytes(),
                    ))
                    .or_default()
                    .push(ScalarAssignment {
                        id: record.id,
                        value: *ratio,
                    });
                unchecked.push(record.id);
            }
            GeometricConstraintKindV1::PointOnLine { .. }
            | GeometricConstraintKindV1::MirrorSymmetry { .. }
            | GeometricConstraintKindV1::RotationalSymmetry { .. }
            | GeometricConstraintKindV1::AngleBisector { .. } => {
                unchecked.push(record.id);
            }
        }
    }

    let edge_ids = edge_id_lookup(&set.constraints);
    let vertex_ids = vertex_id_lookup(&set.constraints);
    let mut conflicts = Vec::new();

    for (edge, summary) in &fixed_lengths {
        if let Some(witness) = summary.different_witness() {
            push_conflict(
                &mut conflicts,
                DirectConstraintConflictKindV1::DifferentFixedLengths {
                    edge: edge_ids[edge],
                },
                witness,
            );
        }
    }
    for (key, assignments) in &fixed_angles {
        if let Some(witness) = different_scalar_witness(assignments) {
            push_conflict(
                &mut conflicts,
                DirectConstraintConflictKindV1::DifferentFixedAngles {
                    vertex: vertex_ids[&key.vertex],
                    first_edge: edge_ids[&key.edges.first],
                    second_edge: edge_ids[&key.edges.second],
                },
                witness,
            );
        }
    }
    for ((numerator, denominator), assignments) in &ratios {
        if let Some(witness) = different_scalar_witness(assignments) {
            push_conflict(
                &mut conflicts,
                DirectConstraintConflictKindV1::DifferentLengthRatios {
                    numerator_edge: edge_ids[numerator],
                    denominator_edge: edge_ids[denominator],
                },
                witness,
            );
        }
    }
    for (edge, horizontal_ids) in &horizontal {
        if let (Some(horizontal_id), Some(vertical_id)) = (
            horizontal_ids.first(),
            vertical.get(edge).and_then(|ids| ids.first()),
        ) {
            push_conflict(
                &mut conflicts,
                DirectConstraintConflictKindV1::HorizontalAndVertical {
                    edge: edge_ids[edge],
                },
                [*horizontal_id, *vertical_id],
            );
        }
    }
    for (pair, equal_ids) in &equal_lengths {
        let Some(first) = fixed_lengths
            .get(&pair.first)
            .and_then(ScalarGroupSummary::consistent_assignment)
        else {
            continue;
        };
        let Some(second) = fixed_lengths
            .get(&pair.second)
            .and_then(ScalarGroupSummary::consistent_assignment)
        else {
            continue;
        };
        if first.value.to_bits() != second.value.to_bits()
            && let Some(equal_id) = equal_ids.first()
        {
            push_conflict(
                &mut conflicts,
                DirectConstraintConflictKindV1::EqualLengthWithDifferentFixedLengths {
                    first_edge: edge_ids[&pair.first],
                    second_edge: edge_ids[&pair.second],
                },
                [*equal_id, first.id, second.id],
            );
        }
    }
    for (pair, equal_ids) in &equal_lengths {
        let fixed = fixed_lengths
            .get(&pair.first)
            .and_then(ScalarGroupSummary::consistent_assignment)
            .or_else(|| {
                fixed_lengths
                    .get(&pair.second)
                    .and_then(ScalarGroupSummary::consistent_assignment)
            });
        let ratio = ratios
            .get(&(pair.first, pair.second))
            .into_iter()
            .chain(ratios.get(&(pair.second, pair.first)))
            .flatten()
            .filter(|assignment| assignment.value.to_bits() != 1.0_f64.to_bits())
            .min_by_key(|assignment| assignment.id.canonical_bytes());
        if let (Some(equal_id), Some(fixed), Some(ratio)) = (equal_ids.first(), fixed, ratio) {
            push_conflict(
                &mut conflicts,
                DirectConstraintConflictKindV1::EqualLengthWithNonUnitRatioAndFixedLength {
                    first_edge: edge_ids[&pair.first],
                    second_edge: edge_ids[&pair.second],
                },
                [*equal_id, fixed.id, ratio.id],
            );
        }
    }
    for (pair, parallel_ids) in &parallels {
        if let (Some(parallel_id), Some(angle_assignment)) = (
            parallel_ids.first(),
            fixed_angles_by_pair.get(pair).and_then(|assignments| {
                assignments
                    .iter()
                    .find(|assignment| assignment.value != 0.0 && assignment.value != 180.0)
            }),
        ) {
            push_conflict(
                &mut conflicts,
                DirectConstraintConflictKindV1::ParallelWithFixedNonParallelAngle {
                    first_edge: edge_ids[&pair.first],
                    second_edge: edge_ids[&pair.second],
                },
                [*parallel_id, angle_assignment.id],
            );
        }
        let first_horizontal = horizontal.get(&pair.first);
        let first_vertical = vertical.get(&pair.first);
        let second_horizontal = horizontal.get(&pair.second);
        let second_vertical = vertical.get(&pair.second);
        if let (Some(parallel_id), Some(horizontal_id), Some(vertical_id)) = (
            parallel_ids.first(),
            first_horizontal.and_then(|ids| ids.first()),
            second_vertical.and_then(|ids| ids.first()),
        ) {
            push_conflict(
                &mut conflicts,
                DirectConstraintConflictKindV1::ParallelWithPerpendicularOrientations {
                    horizontal_edge: edge_ids[&pair.first],
                    vertical_edge: edge_ids[&pair.second],
                },
                [*parallel_id, *horizontal_id, *vertical_id],
            );
        }
        if let (Some(parallel_id), Some(vertical_id), Some(horizontal_id)) = (
            parallel_ids.first(),
            first_vertical.and_then(|ids| ids.first()),
            second_horizontal.and_then(|ids| ids.first()),
        ) {
            push_conflict(
                &mut conflicts,
                DirectConstraintConflictKindV1::ParallelWithPerpendicularOrientations {
                    horizontal_edge: edge_ids[&pair.second],
                    vertical_edge: edge_ids[&pair.first],
                },
                [*parallel_id, *horizontal_id, *vertical_id],
            );
        }
    }

    conflicts.sort_unstable_by(|left, right| {
        conflict_sort_key(&left.conflict)
            .cmp(&conflict_sort_key(&right.conflict))
            .then_with(|| canonical_id_slice_cmp(&left.constraint_ids, &right.constraint_ids))
    });
    conflicts.dedup();
    if !conflicts.is_empty() {
        return ConstraintPreflightV1::DirectConflict { conflicts };
    }

    canonicalize_constraint_ids(&mut unchecked);
    if unchecked.is_empty() {
        ConstraintPreflightV1::NoDirectConflict
    } else {
        ConstraintPreflightV1::Unknown {
            reason: GeometricConstraintUnknownReasonV1::SolverRequiredConstraintKinds,
            unchecked_constraint_ids: unchecked,
        }
    }
}

fn canonical_constraint_ids(records: &[GeometricConstraintRecordV1]) -> Vec<ConstraintId> {
    let mut ids = records.iter().map(|record| record.id).collect::<Vec<_>>();
    canonicalize_constraint_ids(&mut ids);
    ids
}

fn canonicalize_constraint_ids(ids: &mut Vec<ConstraintId>) {
    ids.sort_unstable_by_key(ConstraintId::canonical_bytes);
    ids.dedup();
}

fn different_scalar_witness(assignments: &[ScalarAssignment]) -> Option<[ConstraintId; 2]> {
    let first = assignments.first()?;
    assignments[1..]
        .iter()
        .find(|item| item.value.to_bits() != first.value.to_bits())
        .map(|different| [first.id, different.id])
}

fn push_conflict(
    output: &mut Vec<DirectConstraintConflictV1>,
    conflict: DirectConstraintConflictKindV1,
    ids: impl IntoIterator<Item = ConstraintId>,
) {
    let mut constraint_ids = ids.into_iter().collect::<Vec<_>>();
    canonicalize_constraint_ids(&mut constraint_ids);
    debug_assert!(constraint_ids.len() <= MAX_DIRECT_CONFLICT_CAUSE_IDS_V1);
    output.push(DirectConstraintConflictV1 {
        conflict,
        constraint_ids,
    });
}

fn edge_id_lookup(records: &[GeometricConstraintRecordV1]) -> BTreeMap<CanonicalId, EdgeId> {
    let mut result = BTreeMap::new();
    for record in records {
        match &record.constraint {
            GeometricConstraintKindV1::FixedLength { edge, .. }
            | GeometricConstraintKindV1::Horizontal { edge }
            | GeometricConstraintKindV1::Vertical { edge } => {
                result.insert(edge.canonical_bytes(), *edge);
            }
            GeometricConstraintKindV1::FixedAngle {
                first_edge,
                second_edge,
                ..
            }
            | GeometricConstraintKindV1::EqualLength {
                first_edge,
                second_edge,
            }
            | GeometricConstraintKindV1::Parallel {
                first_edge,
                second_edge,
            } => {
                result.insert(first_edge.canonical_bytes(), *first_edge);
                result.insert(second_edge.canonical_bytes(), *second_edge);
            }
            GeometricConstraintKindV1::PointOnLine { line_edge, .. } => {
                result.insert(line_edge.canonical_bytes(), *line_edge);
            }
            GeometricConstraintKindV1::MirrorSymmetry { axis_edge, .. } => {
                result.insert(axis_edge.canonical_bytes(), *axis_edge);
            }
            GeometricConstraintKindV1::AngleBisector {
                first_edge,
                second_edge,
                bisector_edge,
                ..
            } => {
                result.insert(first_edge.canonical_bytes(), *first_edge);
                result.insert(second_edge.canonical_bytes(), *second_edge);
                result.insert(bisector_edge.canonical_bytes(), *bisector_edge);
            }
            GeometricConstraintKindV1::LengthRatio {
                numerator_edge,
                denominator_edge,
                ..
            } => {
                result.insert(numerator_edge.canonical_bytes(), *numerator_edge);
                result.insert(denominator_edge.canonical_bytes(), *denominator_edge);
            }
            GeometricConstraintKindV1::RotationalSymmetry { .. } => {}
        }
    }
    result
}

fn vertex_id_lookup(records: &[GeometricConstraintRecordV1]) -> BTreeMap<CanonicalId, VertexId> {
    let mut result = BTreeMap::new();
    for record in records {
        match &record.constraint {
            GeometricConstraintKindV1::FixedAngle { vertex, .. }
            | GeometricConstraintKindV1::PointOnLine { vertex, .. }
            | GeometricConstraintKindV1::AngleBisector { vertex, .. } => {
                result.insert(vertex.canonical_bytes(), *vertex);
            }
            GeometricConstraintKindV1::MirrorSymmetry {
                first_vertex,
                second_vertex,
                ..
            } => {
                result.insert(first_vertex.canonical_bytes(), *first_vertex);
                result.insert(second_vertex.canonical_bytes(), *second_vertex);
            }
            GeometricConstraintKindV1::RotationalSymmetry {
                center_vertex,
                source_vertex,
                target_vertex,
                ..
            } => {
                result.insert(center_vertex.canonical_bytes(), *center_vertex);
                result.insert(source_vertex.canonical_bytes(), *source_vertex);
                result.insert(target_vertex.canonical_bytes(), *target_vertex);
            }
            GeometricConstraintKindV1::FixedLength { .. }
            | GeometricConstraintKindV1::Horizontal { .. }
            | GeometricConstraintKindV1::Vertical { .. }
            | GeometricConstraintKindV1::EqualLength { .. }
            | GeometricConstraintKindV1::Parallel { .. }
            | GeometricConstraintKindV1::LengthRatio { .. } => {}
        }
    }
    result
}

fn conflict_sort_key(
    conflict: &DirectConstraintConflictKindV1,
) -> (u8, CanonicalId, CanonicalId, CanonicalId) {
    let zero = [0; 16];
    match conflict {
        DirectConstraintConflictKindV1::DifferentFixedLengths { edge } => {
            (0, edge.canonical_bytes(), zero, zero)
        }
        DirectConstraintConflictKindV1::DifferentFixedAngles {
            vertex,
            first_edge,
            second_edge,
        } => (
            1,
            vertex.canonical_bytes(),
            first_edge.canonical_bytes(),
            second_edge.canonical_bytes(),
        ),
        DirectConstraintConflictKindV1::DifferentLengthRatios {
            numerator_edge,
            denominator_edge,
        } => (
            2,
            numerator_edge.canonical_bytes(),
            denominator_edge.canonical_bytes(),
            zero,
        ),
        DirectConstraintConflictKindV1::HorizontalAndVertical { edge } => {
            (3, edge.canonical_bytes(), zero, zero)
        }
        DirectConstraintConflictKindV1::EqualLengthWithDifferentFixedLengths {
            first_edge,
            second_edge,
        } => (
            4,
            first_edge.canonical_bytes(),
            second_edge.canonical_bytes(),
            zero,
        ),
        DirectConstraintConflictKindV1::EqualLengthWithNonUnitRatioAndFixedLength {
            first_edge,
            second_edge,
        } => (
            5,
            first_edge.canonical_bytes(),
            second_edge.canonical_bytes(),
            zero,
        ),
        DirectConstraintConflictKindV1::ParallelWithFixedNonParallelAngle {
            first_edge,
            second_edge,
        } => (
            6,
            first_edge.canonical_bytes(),
            second_edge.canonical_bytes(),
            zero,
        ),
        DirectConstraintConflictKindV1::ParallelWithPerpendicularOrientations {
            horizontal_edge,
            vertical_edge,
        } => (
            7,
            horizontal_edge.canonical_bytes(),
            vertical_edge.canonical_bytes(),
            zero,
        ),
    }
}

fn canonical_id_slice_cmp(left: &[ConstraintId], right: &[ConstraintId]) -> std::cmp::Ordering {
    left.iter()
        .map(ConstraintId::canonical_bytes)
        .cmp(right.iter().map(ConstraintId::canonical_bytes))
}

#[cfg(test)]
mod tests {
    use ori_domain::{EdgeKind, Point2};
    use serde_json::{Value, json};

    use super::*;

    struct Fixture {
        pattern: CreasePattern,
        vertices: [VertexId; 7],
        edges: [EdgeId; 6],
    }

    impl Fixture {
        fn new() -> Self {
            let vertices = std::array::from_fn(|_| VertexId::new());
            let positions = [
                Point2::new(0.0, 0.0),
                Point2::new(1.0, 0.0),
                Point2::new(0.0, 1.0),
                Point2::new(-1.0, 0.0),
                Point2::new(0.0, -1.0),
                Point2::new(2.0, 0.0),
                Point2::new(2.0, 1.0),
            ];
            let vertex_records = vertices
                .into_iter()
                .zip(positions)
                .map(|(id, position)| Vertex { id, position })
                .collect();
            let edges = std::array::from_fn(|_| EdgeId::new());
            let endpoints = [
                (vertices[0], vertices[1]),
                (vertices[0], vertices[2]),
                (vertices[0], vertices[3]),
                (vertices[0], vertices[4]),
                (vertices[5], vertices[6]),
                (vertices[1], vertices[5]),
            ];
            let edge_records = edges
                .into_iter()
                .zip(endpoints)
                .map(|(id, (start, end))| Edge {
                    id,
                    start,
                    end,
                    kind: EdgeKind::Auxiliary,
                })
                .collect();
            Self {
                pattern: CreasePattern {
                    vertices: vertex_records,
                    edges: edge_records,
                },
                vertices,
                edges,
            }
        }

        fn all_kinds(&self) -> Vec<GeometricConstraintKindV1> {
            vec![
                GeometricConstraintKindV1::FixedLength {
                    edge: self.edges[0],
                    length_mm: 20.0,
                },
                GeometricConstraintKindV1::FixedAngle {
                    vertex: self.vertices[0],
                    first_edge: self.edges[0],
                    second_edge: self.edges[1],
                    angle_degrees: 90.0,
                },
                GeometricConstraintKindV1::Horizontal {
                    edge: self.edges[0],
                },
                GeometricConstraintKindV1::Vertical {
                    edge: self.edges[1],
                },
                GeometricConstraintKindV1::EqualLength {
                    first_edge: self.edges[0],
                    second_edge: self.edges[1],
                },
                GeometricConstraintKindV1::Parallel {
                    first_edge: self.edges[0],
                    second_edge: self.edges[4],
                },
                GeometricConstraintKindV1::PointOnLine {
                    vertex: self.vertices[2],
                    line_edge: self.edges[5],
                },
                GeometricConstraintKindV1::MirrorSymmetry {
                    first_vertex: self.vertices[2],
                    second_vertex: self.vertices[4],
                    axis_edge: self.edges[0],
                },
                GeometricConstraintKindV1::RotationalSymmetry {
                    center_vertex: self.vertices[0],
                    source_vertex: self.vertices[1],
                    target_vertex: self.vertices[2],
                    angle_degrees: 90.0,
                },
                GeometricConstraintKindV1::AngleBisector {
                    vertex: self.vertices[0],
                    first_edge: self.edges[0],
                    second_edge: self.edges[1],
                    bisector_edge: self.edges[2],
                },
                GeometricConstraintKindV1::LengthRatio {
                    numerator_edge: self.edges[0],
                    denominator_edge: self.edges[1],
                    ratio: 2.0,
                },
            ]
        }
    }

    fn record(constraint: GeometricConstraintKindV1) -> GeometricConstraintRecordV1 {
        GeometricConstraintRecordV1 {
            id: ConstraintId::new(),
            constraint,
        }
    }

    fn document(
        constraints: impl IntoIterator<Item = GeometricConstraintRecordV1>,
    ) -> GeometricConstraintDocumentV1 {
        GeometricConstraintDocumentV1 {
            schema_version: GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
            constraints: constraints.into_iter().collect(),
        }
    }

    fn prepare<'pattern>(
        fixture: &'pattern Fixture,
        document: &GeometricConstraintDocumentV1,
    ) -> Result<GeometricConstraintSetV1<'pattern>, GeometricConstraintErrorV1> {
        prepare_geometric_constraints_v1(
            &fixture.pattern,
            document,
            GeometricConstraintLimitsV1::default(),
        )
    }

    #[test]
    fn all_eleven_constraint_kinds_are_persistable_and_preparable() {
        let fixture = Fixture::new();
        let raw = document(fixture.all_kinds().into_iter().map(record));
        let json = serde_json::to_string(&raw).expect("serialize all constraint kinds");
        let restored: GeometricConstraintDocumentV1 =
            serde_json::from_str(&json).expect("deserialize all constraint kinds");
        assert_eq!(restored, raw);

        let prepared = prepare(&fixture, &restored).expect("all eleven kinds are valid");
        assert_eq!(prepared.model_id(), GEOMETRIC_CONSTRAINT_MODEL_ID_V1);
        assert_eq!(prepared.constraints().len(), 11);

        let value: Value = serde_json::from_str(&json).expect("valid JSON value");
        let kinds = value["constraints"]
            .as_array()
            .expect("constraint array")
            .iter()
            .map(|entry| entry["constraint"]["kind"].as_str().expect("kind"))
            .collect::<Vec<_>>();
        assert_eq!(
            kinds,
            [
                "fixed_length",
                "fixed_angle",
                "horizontal",
                "vertical",
                "equal_length",
                "parallel",
                "point_on_line",
                "mirror_symmetry",
                "rotational_symmetry",
                "angle_bisector",
                "length_ratio",
            ]
        );
    }

    #[test]
    fn serde_rejects_unknown_kind_and_unknown_fields() {
        let fixture = Fixture::new();
        let raw = document([record(GeometricConstraintKindV1::Horizontal {
            edge: fixture.edges[0],
        })]);
        let mut unknown_kind = serde_json::to_value(&raw).expect("serialize document");
        unknown_kind["constraints"][0]["constraint"]["kind"] = json!("future_constraint");
        assert!(serde_json::from_value::<GeometricConstraintDocumentV1>(unknown_kind).is_err());

        let mut unknown_document_field = serde_json::to_value(&raw).expect("serialize document");
        unknown_document_field["future"] = json!(true);
        assert!(
            serde_json::from_value::<GeometricConstraintDocumentV1>(unknown_document_field)
                .is_err()
        );

        let mut unknown_constraint_field = serde_json::to_value(&raw).expect("serialize document");
        unknown_constraint_field["constraints"][0]["constraint"]["future"] = json!(true);
        assert!(
            serde_json::from_value::<GeometricConstraintDocumentV1>(unknown_constraint_field)
                .is_err()
        );
    }

    #[test]
    fn unsupported_version_nil_id_and_duplicate_ids_fail_closed() {
        let fixture = Fixture::new();
        let mut wrong_version = document([]);
        wrong_version.schema_version = 2;
        assert_eq!(
            prepare(&fixture, &wrong_version).expect_err("future schema must fail"),
            GeometricConstraintErrorV1::UnsupportedSchemaVersion {
                actual: 2,
                expected: 1,
            }
        );

        let nil_json = format!(
            r#"{{"schema_version":1,"constraints":[{{"id":"00000000-0000-0000-0000-000000000000","constraint":{{"kind":"horizontal","edge":"{}"}}}}]}}"#,
            uuid_string(fixture.edges[0])
        );
        let nil_document: GeometricConstraintDocumentV1 =
            serde_json::from_str(&nil_json).expect("nil UUID has valid wire syntax");
        assert_eq!(
            prepare(&fixture, &nil_document).expect_err("nil constraint ID must fail"),
            GeometricConstraintErrorV1::NilConstraintId
        );

        let duplicate = record(GeometricConstraintKindV1::Horizontal {
            edge: fixture.edges[0],
        });
        let duplicate_document = document([duplicate.clone(), duplicate.clone()]);
        assert_eq!(
            prepare(&fixture, &duplicate_document).expect_err("duplicate ID must fail"),
            GeometricConstraintErrorV1::DuplicateConstraintId {
                constraint: duplicate.id,
            }
        );
    }

    #[test]
    fn nil_geometry_ids_fail_closed_before_reference_validation() {
        let nil_vertex: VertexId = serde_json::from_str("\"00000000-0000-0000-0000-000000000000\"")
            .expect("nil vertex ID has valid UUID wire syntax");
        let mut nil_vertex_fixture = Fixture::new();
        nil_vertex_fixture.pattern.vertices[0].id = nil_vertex;
        let vertex_document = document([record(GeometricConstraintKindV1::Horizontal {
            edge: nil_vertex_fixture.edges[0],
        })]);
        assert_eq!(
            prepare(&nil_vertex_fixture, &vertex_document).expect_err("nil vertex ID must fail"),
            GeometricConstraintErrorV1::NilVertexId
        );

        let nil_edge: EdgeId = serde_json::from_str("\"00000000-0000-0000-0000-000000000000\"")
            .expect("nil edge ID has valid UUID wire syntax");
        let mut nil_edge_fixture = Fixture::new();
        nil_edge_fixture.pattern.edges[0].id = nil_edge;
        let edge_document = document([record(GeometricConstraintKindV1::Horizontal {
            edge: nil_edge,
        })]);
        assert_eq!(
            prepare(&nil_edge_fixture, &edge_document).expect_err("nil edge ID must fail"),
            GeometricConstraintErrorV1::NilEdgeId
        );
    }

    #[test]
    fn duplicate_and_invalid_geometry_registries_are_rejected_deterministically() {
        let fixture = Fixture::new();
        let referenced = document([record(GeometricConstraintKindV1::Horizontal {
            edge: fixture.edges[0],
        })]);

        let mut duplicate_vertex = fixture.pattern.clone();
        duplicate_vertex
            .vertices
            .push(duplicate_vertex.vertices[0].clone());
        assert!(matches!(
            prepare_geometric_constraints_v1(
                &duplicate_vertex,
                &referenced,
                GeometricConstraintLimitsV1::default()
            ),
            Err(GeometricConstraintErrorV1::DuplicateVertexId { .. })
        ));

        let mut duplicate_edge = fixture.pattern.clone();
        duplicate_edge.edges.push(duplicate_edge.edges[0].clone());
        assert!(matches!(
            prepare_geometric_constraints_v1(
                &duplicate_edge,
                &referenced,
                GeometricConstraintLimitsV1::default()
            ),
            Err(GeometricConstraintErrorV1::DuplicateEdgeId { .. })
        ));

        let mut non_finite = fixture.pattern.clone();
        non_finite.vertices[0].position.x = f64::NAN;
        assert!(matches!(
            prepare_geometric_constraints_v1(
                &non_finite,
                &referenced,
                GeometricConstraintLimitsV1::default()
            ),
            Err(GeometricConstraintErrorV1::NonFiniteVertexPosition { .. })
        ));

        let mut missing_endpoint = fixture.pattern.clone();
        missing_endpoint.edges[0].start = VertexId::new();
        assert!(matches!(
            prepare_geometric_constraints_v1(
                &missing_endpoint,
                &referenced,
                GeometricConstraintLimitsV1::default()
            ),
            Err(GeometricConstraintErrorV1::EdgeEndpointMissing { .. })
        ));

        let mut degenerate_identity = fixture.pattern.clone();
        degenerate_identity.edges[0].end = degenerate_identity.edges[0].start;
        assert!(matches!(
            prepare_geometric_constraints_v1(
                &degenerate_identity,
                &referenced,
                GeometricConstraintLimitsV1::default()
            ),
            Err(GeometricConstraintErrorV1::DegenerateGeometryEdge { .. })
        ));

        let mut degenerate_position = fixture.pattern.clone();
        degenerate_position.vertices[1].position = degenerate_position.vertices[0].position;
        assert!(matches!(
            prepare_geometric_constraints_v1(
                &degenerate_position,
                &referenced,
                GeometricConstraintLimitsV1::default()
            ),
            Err(GeometricConstraintErrorV1::DegenerateGeometryEdge { .. })
        ));
    }

    #[test]
    fn empty_v1_document_skips_unreferenced_geometry_but_first_constraint_enforces_the_cap() {
        let repeated = Vertex {
            id: VertexId::new(),
            position: Point2::new(f64::NAN, 0.0),
        };
        let oversized = CreasePattern {
            vertices: vec![repeated; DEFAULT_MAX_CONSTRAINT_VERTICES + 1],
            edges: Vec::new(),
        };
        let empty = document([]);
        let prepared = prepare_geometric_constraints_v1(
            &oversized,
            &empty,
            GeometricConstraintLimitsV1::default(),
        )
        .expect("an empty document has no geometry references to validate");
        assert!(prepared.is_for_pattern(&oversized));
        assert!(prepared.constraints().is_empty());
        assert_eq!(
            prepared.preflight(),
            ConstraintPreflightV1::NoDirectConflict
        );

        let first_constraint = document([record(GeometricConstraintKindV1::Horizontal {
            edge: EdgeId::new(),
        })]);
        assert_eq!(
            prepare_geometric_constraints_v1(
                &oversized,
                &first_constraint,
                GeometricConstraintLimitsV1::default(),
            )
            .expect_err("the first constraint activates the shared geometry ceiling"),
            GeometricConstraintErrorV1::ResourceLimitExceeded {
                resource: GeometricConstraintResourceV1::Vertices,
                actual: DEFAULT_MAX_CONSTRAINT_VERTICES + 1,
                maximum: DEFAULT_MAX_CONSTRAINT_VERTICES,
            }
        );

        let mut future_empty = empty;
        future_empty.schema_version += 1;
        assert!(matches!(
            prepare_geometric_constraints_v1(
                &oversized,
                &future_empty,
                GeometricConstraintLimitsV1::default(),
            ),
            Err(GeometricConstraintErrorV1::UnsupportedSchemaVersion { .. })
        ));
    }

    #[test]
    fn missing_vertex_and_edge_references_are_rejected() {
        let fixture = Fixture::new();
        let missing_edge = EdgeId::new();
        let edge_record = record(GeometricConstraintKindV1::FixedLength {
            edge: missing_edge,
            length_mm: 1.0,
        });
        assert_eq!(
            prepare(&fixture, &document([edge_record.clone()]))
                .expect_err("missing edge must fail"),
            GeometricConstraintErrorV1::MissingEdge {
                constraint: edge_record.id,
                role: ConstraintEdgeRoleV1::Target,
                edge: missing_edge,
            }
        );

        let missing_vertex = VertexId::new();
        let vertex_record = record(GeometricConstraintKindV1::PointOnLine {
            vertex: missing_vertex,
            line_edge: fixture.edges[5],
        });
        assert_eq!(
            prepare(&fixture, &document([vertex_record.clone()]))
                .expect_err("missing vertex must fail"),
            GeometricConstraintErrorV1::MissingVertex {
                constraint: vertex_record.id,
                role: ConstraintVertexRoleV1::Point,
                vertex: missing_vertex,
            }
        );
    }

    #[test]
    fn self_references_and_degenerate_semantic_references_are_rejected() {
        let fixture = Fixture::new();
        for constraint in [
            GeometricConstraintKindV1::EqualLength {
                first_edge: fixture.edges[0],
                second_edge: fixture.edges[0],
            },
            GeometricConstraintKindV1::Parallel {
                first_edge: fixture.edges[1],
                second_edge: fixture.edges[1],
            },
            GeometricConstraintKindV1::LengthRatio {
                numerator_edge: fixture.edges[2],
                denominator_edge: fixture.edges[2],
                ratio: 1.0,
            },
        ] {
            assert!(matches!(
                prepare(&fixture, &document([record(constraint)])),
                Err(GeometricConstraintErrorV1::RepeatedEdgeReference { .. })
            ));
        }

        assert!(matches!(
            prepare(
                &fixture,
                &document([record(GeometricConstraintKindV1::RotationalSymmetry {
                    center_vertex: fixture.vertices[0],
                    source_vertex: fixture.vertices[0],
                    target_vertex: fixture.vertices[2],
                    angle_degrees: 90.0,
                })])
            ),
            Err(GeometricConstraintErrorV1::RepeatedVertexReference { .. })
        ));

        assert!(matches!(
            prepare(
                &fixture,
                &document([record(GeometricConstraintKindV1::PointOnLine {
                    vertex: fixture.vertices[1],
                    line_edge: fixture.edges[0],
                })])
            ),
            Err(GeometricConstraintErrorV1::PointIsLineEndpoint { .. })
        ));

        assert!(matches!(
            prepare(
                &fixture,
                &document([record(GeometricConstraintKindV1::MirrorSymmetry {
                    first_vertex: fixture.vertices[0],
                    second_vertex: fixture.vertices[2],
                    axis_edge: fixture.edges[0],
                })])
            ),
            Err(GeometricConstraintErrorV1::SymmetryPointIsAxisEndpoint { .. })
        ));

        assert!(matches!(
            prepare(
                &fixture,
                &document([record(GeometricConstraintKindV1::FixedAngle {
                    vertex: fixture.vertices[6],
                    first_edge: fixture.edges[0],
                    second_edge: fixture.edges[1],
                    angle_degrees: 90.0,
                })])
            ),
            Err(GeometricConstraintErrorV1::VertexNotIncidentToEdge { .. })
        ));
    }

    #[test]
    fn distinct_ids_at_coincident_geometry_are_degenerate_references() {
        let fixture = Fixture::new();

        let coincident_edge = EdgeId::new();
        let mut duplicate_carrier_pattern = fixture.pattern.clone();
        duplicate_carrier_pattern.edges.push(Edge {
            id: coincident_edge,
            start: fixture.vertices[1],
            end: fixture.vertices[0],
            kind: EdgeKind::Auxiliary,
        });
        let carrier_constraint = record(GeometricConstraintKindV1::EqualLength {
            first_edge: fixture.edges[0],
            second_edge: coincident_edge,
        });
        assert!(matches!(
            prepare_geometric_constraints_v1(
                &duplicate_carrier_pattern,
                &document([carrier_constraint]),
                GeometricConstraintLimitsV1::default()
            ),
            Err(GeometricConstraintErrorV1::CoincidentEdgeReferences { .. })
        ));

        let coincident_vertex = VertexId::new();
        let mut duplicate_position_pattern = fixture.pattern.clone();
        duplicate_position_pattern.vertices.push(Vertex {
            id: coincident_vertex,
            position: duplicate_position_pattern.vertices[1].position,
        });
        let rotation = record(GeometricConstraintKindV1::RotationalSymmetry {
            center_vertex: fixture.vertices[0],
            source_vertex: fixture.vertices[1],
            target_vertex: coincident_vertex,
            angle_degrees: 90.0,
        });
        assert!(matches!(
            prepare_geometric_constraints_v1(
                &duplicate_position_pattern,
                &document([rotation]),
                GeometricConstraintLimitsV1::default()
            ),
            Err(GeometricConstraintErrorV1::CoincidentVertexReferences { .. })
        ));

        let endpoint_alias = VertexId::new();
        duplicate_position_pattern.vertices.push(Vertex {
            id: endpoint_alias,
            position: duplicate_position_pattern.vertices[1].position,
        });
        let point_on_line = record(GeometricConstraintKindV1::PointOnLine {
            vertex: endpoint_alias,
            line_edge: fixture.edges[0],
        });
        assert!(matches!(
            prepare_geometric_constraints_v1(
                &duplicate_position_pattern,
                &document([point_on_line]),
                GeometricConstraintLimitsV1::default()
            ),
            Err(GeometricConstraintErrorV1::PointIsLineEndpoint { .. })
        ));
    }

    #[test]
    fn every_scalar_family_rejects_non_finite_values() {
        let fixture = Fixture::new();
        let cases = [
            (
                GeometricConstraintKindV1::FixedLength {
                    edge: fixture.edges[0],
                    length_mm: f64::INFINITY,
                },
                ConstraintScalarFieldV1::LengthMillimetres,
            ),
            (
                GeometricConstraintKindV1::FixedAngle {
                    vertex: fixture.vertices[0],
                    first_edge: fixture.edges[0],
                    second_edge: fixture.edges[1],
                    angle_degrees: f64::NEG_INFINITY,
                },
                ConstraintScalarFieldV1::AngleDegrees,
            ),
            (
                GeometricConstraintKindV1::RotationalSymmetry {
                    center_vertex: fixture.vertices[0],
                    source_vertex: fixture.vertices[1],
                    target_vertex: fixture.vertices[2],
                    angle_degrees: f64::NAN,
                },
                ConstraintScalarFieldV1::RotationAngleDegrees,
            ),
            (
                GeometricConstraintKindV1::LengthRatio {
                    numerator_edge: fixture.edges[0],
                    denominator_edge: fixture.edges[1],
                    ratio: f64::INFINITY,
                },
                ConstraintScalarFieldV1::Ratio,
            ),
        ];
        for (constraint, expected_field) in cases {
            assert!(matches!(
                prepare(&fixture, &document([record(constraint)])),
                Err(GeometricConstraintErrorV1::NonFiniteValue {
                    field,
                    ..
                }) if field == expected_field
            ));
        }
    }

    #[test]
    fn scalar_boundary_matrix_is_fail_closed() {
        let fixture = Fixture::new();
        for (length_mm, valid) in [
            (-f64::MIN_POSITIVE, false),
            (-0.0, false),
            (0.0, false),
            (f64::MIN_POSITIVE, true),
            (f64::MAX, true),
        ] {
            let result = prepare(
                &fixture,
                &document([record(GeometricConstraintKindV1::FixedLength {
                    edge: fixture.edges[0],
                    length_mm,
                })]),
            );
            assert_eq!(result.is_ok(), valid, "length {length_mm:?}");
        }
        for (angle_degrees, valid) in [
            (-f64::MIN_POSITIVE, false),
            (-0.0, true),
            (0.0, true),
            (180.0, true),
            (180.0_f64.next_up(), false),
        ] {
            let result = prepare(
                &fixture,
                &document([record(GeometricConstraintKindV1::FixedAngle {
                    vertex: fixture.vertices[0],
                    first_edge: fixture.edges[0],
                    second_edge: fixture.edges[1],
                    angle_degrees,
                })]),
            );
            assert_eq!(result.is_ok(), valid, "angle {angle_degrees:?}");
        }
        for (angle_degrees, valid) in [
            (0.0, false),
            (f64::MIN_POSITIVE, true),
            (360.0_f64.next_down(), true),
            (360.0, false),
        ] {
            let result = prepare(
                &fixture,
                &document([record(GeometricConstraintKindV1::RotationalSymmetry {
                    center_vertex: fixture.vertices[0],
                    source_vertex: fixture.vertices[1],
                    target_vertex: fixture.vertices[2],
                    angle_degrees,
                })]),
            );
            assert_eq!(result.is_ok(), valid, "rotation {angle_degrees:?}");
        }
        for (ratio, valid) in [
            (-1.0, false),
            (0.0, false),
            (f64::MIN_POSITIVE, true),
            (f64::MAX, true),
        ] {
            let result = prepare(
                &fixture,
                &document([record(GeometricConstraintKindV1::LengthRatio {
                    numerator_edge: fixture.edges[0],
                    denominator_edge: fixture.edges[1],
                    ratio,
                })]),
            );
            assert_eq!(result.is_ok(), valid, "ratio {ratio:?}");
        }
    }

    #[test]
    fn resource_limits_cover_geometry_constraints_references_and_preflight() {
        let fixture = Fixture::new();
        let one = document([record(GeometricConstraintKindV1::AngleBisector {
            vertex: fixture.vertices[0],
            first_edge: fixture.edges[0],
            second_edge: fixture.edges[1],
            bisector_edge: fixture.edges[2],
        })]);
        let exact_limits = GeometricConstraintLimitsV1 {
            max_vertices: fixture.pattern.vertices.len(),
            max_edges: fixture.pattern.edges.len(),
            max_constraints: 1,
            max_references: 4,
            max_preflight_checks: 1,
        };
        prepare_geometric_constraints_v1(&fixture.pattern, &one, exact_limits)
            .expect("every resource limit admits exact equality");

        for (resource, limits) in [
            (
                GeometricConstraintResourceV1::Vertices,
                GeometricConstraintLimitsV1 {
                    max_vertices: fixture.pattern.vertices.len() - 1,
                    ..Default::default()
                },
            ),
            (
                GeometricConstraintResourceV1::Edges,
                GeometricConstraintLimitsV1 {
                    max_edges: fixture.pattern.edges.len() - 1,
                    ..Default::default()
                },
            ),
            (
                GeometricConstraintResourceV1::Constraints,
                GeometricConstraintLimitsV1 {
                    max_constraints: 0,
                    ..Default::default()
                },
            ),
            (
                GeometricConstraintResourceV1::References,
                GeometricConstraintLimitsV1 {
                    max_references: 3,
                    ..Default::default()
                },
            ),
        ] {
            assert!(matches!(
                prepare_geometric_constraints_v1(&fixture.pattern, &one, limits),
                Err(GeometricConstraintErrorV1::ResourceLimitExceeded {
                    resource: actual,
                    ..
                }) if actual == resource
            ));
        }

        let prepared = prepare_geometric_constraints_v1(
            &fixture.pattern,
            &one,
            GeometricConstraintLimitsV1 {
                max_preflight_checks: 0,
                ..Default::default()
            },
        )
        .expect("preflight work limit is represented as Unknown");
        assert!(matches!(
            prepared.preflight(),
            ConstraintPreflightV1::Unknown {
                reason: GeometricConstraintUnknownReasonV1::WorkLimitExceeded,
                ..
            }
        ));
    }

    #[test]
    fn preflight_defaults_use_the_domain_shared_geometry_hard_ceilings() {
        let limits = GeometricConstraintLimitsV1::default();
        assert_eq!(
            limits.max_vertices,
            ori_domain::DEFAULT_MAX_CONSTRAINT_VERTICES
        );
        assert_eq!(limits.max_edges, ori_domain::DEFAULT_MAX_CONSTRAINT_EDGES);
        assert_eq!(
            DEFAULT_MAX_CONSTRAINT_VERTICES,
            ori_domain::DEFAULT_MAX_CONSTRAINT_VERTICES
        );
        assert_eq!(
            DEFAULT_MAX_CONSTRAINT_EDGES,
            ori_domain::DEFAULT_MAX_CONSTRAINT_EDGES
        );
    }

    #[test]
    fn caller_limits_can_tighten_but_cannot_relax_v1_hard_ceilings() {
        let fixture = Fixture::new();
        let records = (0..=DEFAULT_MAX_CONSTRAINT_RECORDS)
            .map(|_| {
                record(GeometricConstraintKindV1::Horizontal {
                    edge: fixture.edges[0],
                })
            })
            .collect::<Vec<_>>();
        let mut over_ceiling = document(records);
        let relaxed = GeometricConstraintLimitsV1 {
            max_vertices: usize::MAX,
            max_edges: usize::MAX,
            max_constraints: usize::MAX,
            max_references: usize::MAX,
            max_preflight_checks: usize::MAX,
        };
        assert_eq!(
            prepare_geometric_constraints_v1(&fixture.pattern, &over_ceiling, relaxed,)
                .expect_err("caller limits must not relax the V1 hard ceiling"),
            GeometricConstraintErrorV1::ResourceLimitExceeded {
                resource: GeometricConstraintResourceV1::Constraints,
                actual: DEFAULT_MAX_CONSTRAINT_RECORDS + 1,
                maximum: DEFAULT_MAX_CONSTRAINT_RECORDS,
            }
        );

        over_ceiling
            .constraints
            .pop()
            .expect("fixture has exactly one record beyond the ceiling");
        let exact = prepare_geometric_constraints_v1(&fixture.pattern, &over_ceiling, relaxed)
            .expect("the non-relaxable V1 hard ceiling admits exact equality");
        assert_eq!(exact.constraints().len(), DEFAULT_MAX_CONSTRAINT_RECORDS);

        assert_eq!(relaxed.effective(), GeometricConstraintLimitsV1::default());
        let tightened = GeometricConstraintLimitsV1 {
            max_vertices: 1,
            max_edges: 2,
            max_constraints: 3,
            max_references: 4,
            max_preflight_checks: 5,
        };
        assert_eq!(tightened.effective(), tightened);
    }

    #[test]
    fn equal_length_non_unit_ratio_with_positive_fixed_length_has_minimal_cause() {
        let fixture = Fixture::new();
        let fixed = record(GeometricConstraintKindV1::FixedLength {
            edge: fixture.edges[0],
            length_mm: 10.0,
        });
        let equal = record(GeometricConstraintKindV1::EqualLength {
            first_edge: fixture.edges[0],
            second_edge: fixture.edges[1],
        });
        let ratio = record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: fixture.edges[0],
            denominator_edge: fixture.edges[1],
            ratio: 2.0,
        });
        let records = [fixed.clone(), equal.clone(), ratio.clone()];
        let prepared = prepare(&fixture, &document(records.clone()))
            .expect("the individually valid constraints prepare");
        let ConstraintPreflightV1::DirectConflict { conflicts } = prepared.preflight() else {
            panic!("equal lengths and a non-unit ratio contradict a positive fixed length");
        };
        assert_eq!(conflicts.len(), 1);
        let mut canonical_edges = [fixture.edges[0], fixture.edges[1]];
        canonical_edges.sort_unstable_by_key(EdgeId::canonical_bytes);
        assert_eq!(
            conflicts[0].conflict(),
            &DirectConstraintConflictKindV1::EqualLengthWithNonUnitRatioAndFixedLength {
                first_edge: canonical_edges[0],
                second_edge: canonical_edges[1],
            }
        );
        let mut expected_ids = records.iter().map(|record| record.id).collect::<Vec<_>>();
        expected_ids.sort_unstable_by_key(ConstraintId::canonical_bytes);
        assert_eq!(conflicts[0].constraint_ids(), expected_ids);

        for removed in 0..records.len() {
            let subset = records
                .iter()
                .enumerate()
                .filter(|(index, _)| *index != removed)
                .map(|(_, record)| record.clone())
                .collect::<Vec<_>>();
            let prepared = prepare(&fixture, &document(subset)).expect("proper subset prepares");
            assert!(
                !matches!(
                    prepared.preflight(),
                    ConstraintPreflightV1::DirectConflict { .. }
                ),
                "removing any one cause constraint must remove the direct contradiction"
            );
        }
    }

    #[test]
    fn shared_fixed_length_groups_keep_scan_and_conflict_output_linear() {
        const SHARED_FIXED_COUNT: usize = 1_000;
        const PAIR_COUNT: usize = 1_000;

        let center = VertexId::new();
        let common_end = VertexId::new();
        let mut vertices = vec![
            Vertex {
                id: center,
                position: Point2::new(0.0, 0.0),
            },
            Vertex {
                id: common_end,
                position: Point2::new(1.0, 0.0),
            },
        ];
        let common_edge = EdgeId::new();
        let mut edges = vec![Edge {
            id: common_edge,
            start: center,
            end: common_end,
            kind: EdgeKind::Auxiliary,
        }];
        let mut secondary_edges = Vec::with_capacity(PAIR_COUNT);
        for index in 0..PAIR_COUNT {
            let endpoint = VertexId::new();
            vertices.push(Vertex {
                id: endpoint,
                position: Point2::new(index as f64 + 2.0, 1.0),
            });
            let edge = EdgeId::new();
            edges.push(Edge {
                id: edge,
                start: center,
                end: endpoint,
                kind: EdgeKind::Auxiliary,
            });
            secondary_edges.push(edge);
        }
        let pattern = CreasePattern { vertices, edges };

        let mut records = Vec::with_capacity(SHARED_FIXED_COUNT + 2 * PAIR_COUNT);
        records.extend((0..SHARED_FIXED_COUNT).map(|_| {
            record(GeometricConstraintKindV1::FixedLength {
                edge: common_edge,
                length_mm: 1.0,
            })
        }));
        for edge in secondary_edges {
            records.push(record(GeometricConstraintKindV1::FixedLength {
                edge,
                length_mm: 2.0,
            }));
            records.push(record(GeometricConstraintKindV1::EqualLength {
                first_edge: common_edge,
                second_edge: edge,
            }));
        }
        let record_count = records.len();
        let raw = document(records);
        let limits = GeometricConstraintLimitsV1 {
            max_vertices: pattern.vertices.len(),
            max_edges: pattern.edges.len(),
            max_constraints: record_count,
            max_references: SHARED_FIXED_COUNT + 3 * PAIR_COUNT,
            max_preflight_checks: record_count,
        };
        let prepared = prepare_geometric_constraints_v1(&pattern, &raw, limits)
            .expect("stress input is exactly within every limit");
        begin_fixed_length_summary_visit_count();
        let ConstraintPreflightV1::DirectConflict { conflicts } = prepared.preflight() else {
            panic!("each equal-length relation directly contradicts fixed lengths");
        };
        assert_eq!(
            finish_fixed_length_summary_visit_count(),
            SHARED_FIXED_COUNT + PAIR_COUNT,
            "each fixed-length assignment must be summarized exactly once regardless of how many equal-length pairs reuse its edge"
        );
        assert_eq!(conflicts.len(), PAIR_COUNT);
        assert!(conflicts.iter().all(|conflict| {
            conflict.constraint_ids().len() == MAX_DIRECT_CONFLICT_CAUSE_IDS_V1
        }));
        assert_eq!(
            conflicts
                .iter()
                .map(|conflict| conflict.constraint_ids().len())
                .sum::<usize>(),
            MAX_DIRECT_CONFLICT_CAUSE_IDS_V1 * PAIR_COUNT
        );

        let one_short = prepare_geometric_constraints_v1(
            &pattern,
            &raw,
            GeometricConstraintLimitsV1 {
                max_preflight_checks: record_count - 1,
                ..limits
            },
        )
        .expect("a preflight work limit does not invalidate persistence");
        assert!(matches!(
            one_short.preflight(),
            ConstraintPreflightV1::Unknown {
                reason: GeometricConstraintUnknownReasonV1::WorkLimitExceeded,
                ref unchecked_constraint_ids,
            } if unchecked_constraint_ids.len() == record_count
        ));
    }

    #[test]
    fn differing_fixed_length_angle_and_ratio_report_all_cause_ids() {
        let fixture = Fixture::new();
        let length_a = record(GeometricConstraintKindV1::FixedLength {
            edge: fixture.edges[0],
            length_mm: 1.0,
        });
        let length_b = record(GeometricConstraintKindV1::FixedLength {
            edge: fixture.edges[0],
            length_mm: 2.0,
        });
        let angle_a = record(GeometricConstraintKindV1::FixedAngle {
            vertex: fixture.vertices[0],
            first_edge: fixture.edges[0],
            second_edge: fixture.edges[1],
            angle_degrees: 45.0,
        });
        let angle_b = record(GeometricConstraintKindV1::FixedAngle {
            vertex: fixture.vertices[0],
            first_edge: fixture.edges[1],
            second_edge: fixture.edges[0],
            angle_degrees: 90.0,
        });
        let ratio_a = record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: fixture.edges[0],
            denominator_edge: fixture.edges[1],
            ratio: 1.0,
        });
        let ratio_b = record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: fixture.edges[0],
            denominator_edge: fixture.edges[1],
            ratio: 2.0,
        });
        let prepared = prepare(
            &fixture,
            &document([
                ratio_b.clone(),
                length_b.clone(),
                angle_a.clone(),
                length_a.clone(),
                ratio_a.clone(),
                angle_b.clone(),
            ]),
        )
        .expect("valid references");
        let ConstraintPreflightV1::DirectConflict { conflicts } = prepared.preflight() else {
            panic!("different direct scalar assignments must conflict");
        };
        assert_eq!(conflicts.len(), 3);
        for conflict in &conflicts {
            assert!(
                conflict
                    .constraint_ids()
                    .windows(2)
                    .all(|pair| { pair[0].canonical_bytes() < pair[1].canonical_bytes() })
            );
        }
        assert!(conflicts.iter().any(|conflict| {
            matches!(
                conflict.conflict(),
                DirectConstraintConflictKindV1::DifferentFixedLengths { .. }
            ) && same_ids(conflict.constraint_ids(), &[length_a.id, length_b.id])
        }));
        assert!(conflicts.iter().any(|conflict| {
            matches!(
                conflict.conflict(),
                DirectConstraintConflictKindV1::DifferentFixedAngles { .. }
            ) && same_ids(conflict.constraint_ids(), &[angle_a.id, angle_b.id])
        }));
        assert!(conflicts.iter().any(|conflict| {
            matches!(
                conflict.conflict(),
                DirectConstraintConflictKindV1::DifferentLengthRatios { .. }
            ) && same_ids(conflict.constraint_ids(), &[ratio_a.id, ratio_b.id])
        }));
    }

    #[test]
    fn nondegenerate_edge_cannot_be_both_horizontal_and_vertical() {
        let fixture = Fixture::new();
        let horizontal = record(GeometricConstraintKindV1::Horizontal {
            edge: fixture.edges[0],
        });
        let vertical = record(GeometricConstraintKindV1::Vertical {
            edge: fixture.edges[0],
        });
        let prepared = prepare(&fixture, &document([vertical.clone(), horizontal.clone()]))
            .expect("each constraint is locally valid");
        assert_eq!(
            prepared.preflight(),
            ConstraintPreflightV1::DirectConflict {
                conflicts: vec![DirectConstraintConflictV1 {
                    conflict: DirectConstraintConflictKindV1::HorizontalAndVertical {
                        edge: fixture.edges[0],
                    },
                    constraint_ids: sorted_ids(&[horizontal.id, vertical.id]),
                }],
            }
        );
    }

    #[test]
    fn direct_three_constraint_relations_are_detected() {
        let fixture = Fixture::new();
        let first_length = record(GeometricConstraintKindV1::FixedLength {
            edge: fixture.edges[0],
            length_mm: 1.0,
        });
        let second_length = record(GeometricConstraintKindV1::FixedLength {
            edge: fixture.edges[1],
            length_mm: 2.0,
        });
        let equal = record(GeometricConstraintKindV1::EqualLength {
            first_edge: fixture.edges[1],
            second_edge: fixture.edges[0],
        });
        let parallel = record(GeometricConstraintKindV1::Parallel {
            first_edge: fixture.edges[1],
            second_edge: fixture.edges[0],
        });
        let angle = record(GeometricConstraintKindV1::FixedAngle {
            vertex: fixture.vertices[0],
            first_edge: fixture.edges[0],
            second_edge: fixture.edges[1],
            angle_degrees: 90.0,
        });
        let prepared = prepare(
            &fixture,
            &document([equal, second_length, parallel, first_length, angle]),
        )
        .expect("locally valid");
        let ConstraintPreflightV1::DirectConflict { conflicts } = prepared.preflight() else {
            panic!("direct relations must conflict");
        };
        assert!(conflicts.iter().any(|conflict| matches!(
            conflict.conflict(),
            DirectConstraintConflictKindV1::EqualLengthWithDifferentFixedLengths { .. }
        )));
        assert!(conflicts.iter().any(|conflict| matches!(
            conflict.conflict(),
            DirectConstraintConflictKindV1::ParallelWithFixedNonParallelAngle { .. }
        )));
    }

    #[test]
    fn proven_direct_conflict_causes_are_canonical_and_deletion_minimal() {
        let fixture = Fixture::new();
        let cases = [
            vec![
                record(GeometricConstraintKindV1::Horizontal {
                    edge: fixture.edges[0],
                }),
                record(GeometricConstraintKindV1::Vertical {
                    edge: fixture.edges[0],
                }),
            ],
            vec![
                record(GeometricConstraintKindV1::FixedLength {
                    edge: fixture.edges[0],
                    length_mm: 1.0,
                }),
                record(GeometricConstraintKindV1::FixedLength {
                    edge: fixture.edges[1],
                    length_mm: 2.0,
                }),
                record(GeometricConstraintKindV1::EqualLength {
                    first_edge: fixture.edges[0],
                    second_edge: fixture.edges[1],
                }),
            ],
        ];

        for records in cases {
            let prepared = prepare(&fixture, &document(records.clone())).expect("valid cause");
            let ConstraintPreflightV1::DirectConflict { conflicts } = prepared.preflight() else {
                panic!("complete direct witness must prove a conflict");
            };
            assert_eq!(conflicts.len(), 1);
            let cause = &conflicts[0];
            assert_eq!(cause.constraint_ids().len(), records.len());
            assert!(
                cause
                    .constraint_ids()
                    .windows(2)
                    .all(|pair| { pair[0].canonical_bytes() < pair[1].canonical_bytes() })
            );

            for removed in cause.constraint_ids() {
                let subset = records
                    .iter()
                    .filter(|record| record.id != *removed)
                    .cloned()
                    .collect::<Vec<_>>();
                assert!(!matches!(
                    prepare(&fixture, &document(subset))
                        .expect("proper witness subset remains valid input")
                        .preflight(),
                    ConstraintPreflightV1::DirectConflict { .. }
                ));
            }
        }
    }

    #[test]
    fn partially_checked_fixed_angle_and_ratio_kinds_return_unknown() {
        let fixture = Fixture::new();

        let fixed_angle = record(GeometricConstraintKindV1::FixedAngle {
            vertex: fixture.vertices[0],
            first_edge: fixture.edges[0],
            second_edge: fixture.edges[1],
            angle_degrees: 90.0,
        });
        let both_horizontal = prepare(
            &fixture,
            &document([
                record(GeometricConstraintKindV1::Horizontal {
                    edge: fixture.edges[0],
                }),
                record(GeometricConstraintKindV1::Horizontal {
                    edge: fixture.edges[1],
                }),
                fixed_angle.clone(),
            ]),
        )
        .expect("locally valid fixed-angle fixture");
        assert_eq!(
            both_horizontal.preflight(),
            ConstraintPreflightV1::Unknown {
                reason: GeometricConstraintUnknownReasonV1::SolverRequiredConstraintKinds,
                unchecked_constraint_ids: vec![fixed_angle.id],
            }
        );

        let incompatible_ratio = record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: fixture.edges[0],
            denominator_edge: fixture.edges[1],
            ratio: 2.0,
        });
        let fixed_lengths_and_ratio = prepare(
            &fixture,
            &document([
                record(GeometricConstraintKindV1::FixedLength {
                    edge: fixture.edges[0],
                    length_mm: 1.0,
                }),
                record(GeometricConstraintKindV1::FixedLength {
                    edge: fixture.edges[1],
                    length_mm: 1.0,
                }),
                incompatible_ratio.clone(),
            ]),
        )
        .expect("locally valid fixed-length and ratio fixture");
        assert_eq!(
            fixed_lengths_and_ratio.preflight(),
            ConstraintPreflightV1::Unknown {
                reason: GeometricConstraintUnknownReasonV1::SolverRequiredConstraintKinds,
                unchecked_constraint_ids: vec![incompatible_ratio.id],
            }
        );

        let forward_ratio = record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: fixture.edges[0],
            denominator_edge: fixture.edges[1],
            ratio: 2.0,
        });
        let reverse_ratio = record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: fixture.edges[1],
            denominator_edge: fixture.edges[0],
            ratio: 2.0,
        });
        let inverse_pair = prepare(
            &fixture,
            &document([reverse_ratio.clone(), forward_ratio.clone()]),
        )
        .expect("locally valid inverse ratio fixture");
        assert_eq!(
            inverse_pair.preflight(),
            ConstraintPreflightV1::Unknown {
                reason: GeometricConstraintUnknownReasonV1::SolverRequiredConstraintKinds,
                unchecked_constraint_ids: sorted_ids(&[forward_ratio.id, reverse_ratio.id]),
            }
        );
    }

    #[test]
    fn parallel_horizontal_vertical_cross_relation_is_detected() {
        let fixture = Fixture::new();
        let records = [
            record(GeometricConstraintKindV1::Parallel {
                first_edge: fixture.edges[0],
                second_edge: fixture.edges[4],
            }),
            record(GeometricConstraintKindV1::Horizontal {
                edge: fixture.edges[0],
            }),
            record(GeometricConstraintKindV1::Vertical {
                edge: fixture.edges[4],
            }),
        ];
        let prepared = prepare(&fixture, &document(records)).expect("locally valid");
        assert!(matches!(
            prepared.preflight(),
            ConstraintPreflightV1::DirectConflict { ref conflicts }
                if conflicts.iter().any(|conflict| matches!(
                    conflict.conflict(),
                    DirectConstraintConflictKindV1::ParallelWithPerpendicularOrientations { .. }
                ))
        ));
    }

    #[test]
    fn no_direct_conflict_and_unknown_are_distinct_canonical_native_outputs() {
        let fixture = Fixture::new();
        let checked = prepare(
            &fixture,
            &document([
                record(GeometricConstraintKindV1::FixedLength {
                    edge: fixture.edges[0],
                    length_mm: 1.0,
                }),
                record(GeometricConstraintKindV1::Horizontal {
                    edge: fixture.edges[0],
                }),
            ]),
        )
        .expect("valid checked constraints");
        assert_eq!(checked.preflight(), ConstraintPreflightV1::NoDirectConflict);

        let solver_required = record(GeometricConstraintKindV1::PointOnLine {
            vertex: fixture.vertices[2],
            line_edge: fixture.edges[5],
        });
        let unchecked = prepare(&fixture, &document([solver_required.clone()]))
            .expect("valid solver-required constraint");
        let outcome = unchecked.preflight();
        assert_eq!(
            outcome,
            ConstraintPreflightV1::Unknown {
                reason: GeometricConstraintUnknownReasonV1::SolverRequiredConstraintKinds,
                unchecked_constraint_ids: vec![solver_required.id],
            }
        );
        let wire = serde_json::to_string(&outcome).expect("serialize preflight result");
        let expected_wire = format!(
            r#"{{"status":"unknown","reason":"solver_required_constraint_kinds","unchecked_constraint_ids":["{}"]}}"#,
            uuid_string(solver_required.id)
        );
        assert_eq!(wire, expected_wire);
        assert_eq!(
            serde_json::from_str::<Value>(&wire).expect("native output is valid JSON"),
            json!({
                "status": "unknown",
                "reason": "solver_required_constraint_kinds",
                "unchecked_constraint_ids": [uuid_string(solver_required.id)],
            })
        );
    }

    #[test]
    fn storage_order_geometry_order_and_unordered_operand_property_are_invariant() {
        let fixture = Fixture::new();
        let mut records = fixture
            .all_kinds()
            .into_iter()
            .map(record)
            .collect::<Vec<_>>();
        records.push(record(GeometricConstraintKindV1::FixedLength {
            edge: fixture.edges[0],
            length_mm: 21.0,
        }));

        let baseline = prepare(&fixture, &document(records.clone())).expect("baseline");
        let baseline_outcome = baseline.preflight();

        let mut reordered_pattern = fixture.pattern.clone();
        reordered_pattern.vertices.reverse();
        reordered_pattern.edges.reverse();
        let reordered_fixture = Fixture {
            pattern: reordered_pattern,
            vertices: fixture.vertices,
            edges: fixture.edges,
        };

        let mut seed = 0x9e37_79b9_u64;
        for _ in 0..128 {
            deterministic_shuffle(&mut records, &mut seed);
            for record in &mut records {
                reverse_unordered_operands(&mut record.constraint);
            }
            let candidate =
                prepare(&reordered_fixture, &document(records.clone())).expect("permutation");
            assert_eq!(candidate.constraints(), baseline.constraints());
            assert_eq!(candidate.preflight(), baseline_outcome);
        }
    }

    #[test]
    fn validation_error_selection_is_invariant_to_storage_permutations() {
        let fixture = Fixture::new();
        let missing_a = EdgeId::new();
        let missing_b = EdgeId::new();
        let first = record(GeometricConstraintKindV1::Horizontal { edge: missing_a });
        let second = record(GeometricConstraintKindV1::Vertical { edge: missing_b });
        let expected_id = if first.id.canonical_bytes() < second.id.canonical_bytes() {
            first.id
        } else {
            second.id
        };
        let forward = prepare(&fixture, &document([first.clone(), second.clone()]))
            .expect_err("both documents contain missing references");
        let reverse = prepare(&fixture, &document([second, first]))
            .expect_err("both documents contain missing references");
        assert_eq!(forward, reverse);
        assert!(matches!(
            forward,
            GeometricConstraintErrorV1::MissingEdge { constraint, .. }
                if constraint == expected_id
        ));
    }

    #[test]
    fn validation_normalizes_unordered_operands_before_selecting_an_error() {
        let fixture = Fixture::new();
        let first_missing = EdgeId::new();
        let second_missing = EdgeId::new();
        let constraint_id = ConstraintId::new();
        let forward = GeometricConstraintRecordV1 {
            id: constraint_id,
            constraint: GeometricConstraintKindV1::EqualLength {
                first_edge: first_missing,
                second_edge: second_missing,
            },
        };
        let reverse = GeometricConstraintRecordV1 {
            id: constraint_id,
            constraint: GeometricConstraintKindV1::EqualLength {
                first_edge: second_missing,
                second_edge: first_missing,
            },
        };
        let forward_error =
            prepare(&fixture, &document([forward])).expect_err("both references are missing");
        let reverse_error =
            prepare(&fixture, &document([reverse])).expect_err("both references are missing");
        assert_eq!(forward_error, reverse_error);

        let canonical_first = if first_missing.canonical_bytes() < second_missing.canonical_bytes()
        {
            first_missing
        } else {
            second_missing
        };
        assert_eq!(
            forward_error,
            GeometricConstraintErrorV1::MissingEdge {
                constraint: constraint_id,
                role: ConstraintEdgeRoleV1::First,
                edge: canonical_first,
            }
        );
    }

    #[test]
    fn prepared_set_borrows_and_identifies_its_exact_source_pattern() {
        let fixture = Fixture::new();
        let prepared = prepare(&fixture, &document([])).expect("empty constraints are valid");
        assert!(std::ptr::eq(prepared.source_pattern(), &fixture.pattern));
        assert!(prepared.is_for_pattern(&fixture.pattern));

        let equal_but_distinct_pattern = fixture.pattern.clone();
        assert_eq!(equal_but_distinct_pattern, fixture.pattern);
        assert!(!prepared.is_for_pattern(&equal_but_distinct_pattern));
    }

    fn sorted_ids(ids: &[ConstraintId]) -> Vec<ConstraintId> {
        let mut result = ids.to_vec();
        canonicalize_constraint_ids(&mut result);
        result
    }

    fn same_ids(actual: &[ConstraintId], expected: &[ConstraintId]) -> bool {
        actual == sorted_ids(expected)
    }

    fn uuid_string<T: Serialize>(id: T) -> String {
        serde_json::to_string(&id)
            .expect("serialize UUID-backed ID")
            .trim_matches('"')
            .to_owned()
    }

    fn deterministic_shuffle<T>(items: &mut [T], state: &mut u64) {
        for index in (1..items.len()).rev() {
            *state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let target = (*state as usize) % (index + 1);
            items.swap(index, target);
        }
    }

    fn reverse_unordered_operands(constraint: &mut GeometricConstraintKindV1) {
        match constraint {
            GeometricConstraintKindV1::FixedAngle {
                first_edge,
                second_edge,
                ..
            }
            | GeometricConstraintKindV1::EqualLength {
                first_edge,
                second_edge,
            }
            | GeometricConstraintKindV1::Parallel {
                first_edge,
                second_edge,
            }
            | GeometricConstraintKindV1::AngleBisector {
                first_edge,
                second_edge,
                ..
            } => std::mem::swap(first_edge, second_edge),
            GeometricConstraintKindV1::MirrorSymmetry {
                first_vertex,
                second_vertex,
                ..
            } => std::mem::swap(first_vertex, second_vertex),
            GeometricConstraintKindV1::FixedLength { .. }
            | GeometricConstraintKindV1::Horizontal { .. }
            | GeometricConstraintKindV1::Vertical { .. }
            | GeometricConstraintKindV1::PointOnLine { .. }
            | GeometricConstraintKindV1::RotationalSymmetry { .. }
            | GeometricConstraintKindV1::LengthRatio { .. } => {}
        }
    }
}
