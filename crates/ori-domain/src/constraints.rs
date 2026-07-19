//! Persisted geometric-constraint records and geometry-independent validation.
//!
//! This module is the low-level persistence boundary for EDT-008. Validation
//! here deliberately does not inspect a crease pattern: reference existence,
//! incidence, coincident geometry, intersections, and other geometry-dependent
//! checks remain the responsibility of `ori-core`.

use std::{error::Error, fmt};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{EdgeId, VertexId};

/// Exact persisted schema version accepted by this implementation.
pub const GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1: u32 = 1;
/// Non-relaxable V1 authored-constraint-count ceiling.
pub const DEFAULT_MAX_CONSTRAINT_RECORDS: usize = 10_000;
/// Non-relaxable V1 referenced-role-count ceiling.
pub const DEFAULT_MAX_CONSTRAINT_REFERENCES: usize = 40_000;
/// Non-relaxable V1 crease-pattern vertex-count ceiling while constraints exist.
pub const DEFAULT_MAX_CONSTRAINT_VERTICES: usize = 100_000;
/// Non-relaxable V1 crease-pattern edge-count ceiling while constraints exist.
pub const DEFAULT_MAX_CONSTRAINT_EDGES: usize = 100_000;

type CanonicalId = [u8; 16];

/// Stable, persistence-safe identity of one authored constraint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ConstraintId(Uuid);

impl ConstraintId {
    /// Creates a new UUID-v4 constraint identity.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Returns the UUID in canonical RFC byte order.
    #[must_use]
    pub const fn canonical_bytes(&self) -> CanonicalId {
        self.0.into_bytes()
    }

    /// Returns whether this is the reserved nil UUID.
    #[must_use]
    pub fn is_nil(&self) -> bool {
        self.canonical_bytes() == [0; 16]
    }
}

impl Default for ConstraintId {
    fn default() -> Self {
        Self::new()
    }
}

/// Persistence envelope. Unknown fields and unknown constraint kinds are
/// rejected by serde rather than silently discarded.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeometricConstraintDocumentV1 {
    pub schema_version: u32,
    pub constraints: Vec<GeometricConstraintRecordV1>,
}

impl GeometricConstraintDocumentV1 {
    /// Returns whether the document contains no authored constraints.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.constraints.is_empty()
    }
}

impl Default for GeometricConstraintDocumentV1 {
    fn default() -> Self {
        Self {
            schema_version: GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
            constraints: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeometricConstraintRecordV1 {
    pub id: ConstraintId,
    pub constraint: GeometricConstraintKindV1,
}

/// The eleven first-version persisted geometric constraints required by
/// EDT-008.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum GeometricConstraintKindV1 {
    FixedLength {
        edge: EdgeId,
        length_mm: f64,
    },
    FixedAngle {
        vertex: VertexId,
        first_edge: EdgeId,
        second_edge: EdgeId,
        angle_degrees: f64,
    },
    Horizontal {
        edge: EdgeId,
    },
    Vertical {
        edge: EdgeId,
    },
    EqualLength {
        first_edge: EdgeId,
        second_edge: EdgeId,
    },
    Parallel {
        first_edge: EdgeId,
        second_edge: EdgeId,
    },
    PointOnLine {
        vertex: VertexId,
        line_edge: EdgeId,
    },
    MirrorSymmetry {
        first_vertex: VertexId,
        second_vertex: VertexId,
        axis_edge: EdgeId,
    },
    RotationalSymmetry {
        center_vertex: VertexId,
        source_vertex: VertexId,
        target_vertex: VertexId,
        angle_degrees: f64,
    },
    AngleBisector {
        vertex: VertexId,
        first_edge: EdgeId,
        second_edge: EdgeId,
        bisector_edge: EdgeId,
    },
    LengthRatio {
        numerator_edge: EdgeId,
        denominator_edge: EdgeId,
        ratio: f64,
    },
}

impl GeometricConstraintKindV1 {
    /// Number of persisted entity-reference roles occupied by this record.
    #[must_use]
    pub const fn reference_count(&self) -> usize {
        match self {
            Self::FixedLength { .. } | Self::Horizontal { .. } | Self::Vertical { .. } => 1,
            Self::EqualLength { .. }
            | Self::Parallel { .. }
            | Self::PointOnLine { .. }
            | Self::LengthRatio { .. } => 2,
            Self::FixedAngle { .. }
            | Self::MirrorSymmetry { .. }
            | Self::RotationalSymmetry { .. } => 3,
            Self::AngleBisector { .. } => 4,
        }
    }
}

/// Geometry-independent persisted-document validation failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GeometricConstraintDocumentValidationErrorV1 {
    UnsupportedSchemaVersion {
        actual: u32,
        expected: u32,
    },
    TooManyConstraints {
        actual: usize,
        maximum: usize,
    },
    TooManyReferences {
        actual: usize,
        maximum: usize,
    },
    ReferenceCountOverflow,
    AllocationFailed,
    NilConstraintId,
    DuplicateConstraintId {
        constraint: ConstraintId,
    },
    NilVertexReference {
        constraint: ConstraintId,
        vertex: VertexId,
    },
    NilEdgeReference {
        constraint: ConstraintId,
        edge: EdgeId,
    },
    RepeatedVertexReference {
        constraint: ConstraintId,
        vertex: VertexId,
    },
    RepeatedEdgeReference {
        constraint: ConstraintId,
        edge: EdgeId,
    },
    NonFiniteFixedLength {
        constraint: ConstraintId,
    },
    NonPositiveFixedLength {
        constraint: ConstraintId,
    },
    NonFiniteFixedAngle {
        constraint: ConstraintId,
    },
    FixedAngleOutOfRange {
        constraint: ConstraintId,
    },
    NonFiniteRotationAngle {
        constraint: ConstraintId,
    },
    RotationAngleOutOfRange {
        constraint: ConstraintId,
    },
    NonFiniteLengthRatio {
        constraint: ConstraintId,
    },
    NonPositiveLengthRatio {
        constraint: ConstraintId,
    },
}

impl fmt::Display for GeometricConstraintDocumentValidationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedSchemaVersion { actual, expected } => write!(
                formatter,
                "unsupported geometric-constraint schema version {actual}; expected {expected}"
            ),
            Self::TooManyConstraints { actual, maximum } => write!(
                formatter,
                "geometric-constraint count {actual} exceeds the hard maximum {maximum}"
            ),
            Self::TooManyReferences { actual, maximum } => write!(
                formatter,
                "geometric-constraint reference count {actual} exceeds the hard maximum {maximum}"
            ),
            Self::ReferenceCountOverflow => {
                formatter.write_str("geometric-constraint reference count overflowed")
            }
            Self::AllocationFailed => formatter
                .write_str("memory for geometric-constraint validation could not be reserved"),
            Self::NilConstraintId => {
                formatter.write_str("constraint IDs must not use the nil UUID")
            }
            Self::DuplicateConstraintId { constraint } => {
                write!(formatter, "constraint {constraint:?} occurs more than once")
            }
            Self::NilVertexReference { constraint, vertex } => write!(
                formatter,
                "constraint {constraint:?} contains nil vertex reference {vertex:?}"
            ),
            Self::NilEdgeReference { constraint, edge } => write!(
                formatter,
                "constraint {constraint:?} contains nil edge reference {edge:?}"
            ),
            Self::RepeatedVertexReference { constraint, vertex } => write!(
                formatter,
                "constraint {constraint:?} repeats vertex {vertex:?} in roles that must be distinct"
            ),
            Self::RepeatedEdgeReference { constraint, edge } => write!(
                formatter,
                "constraint {constraint:?} repeats edge {edge:?} in roles that must be distinct"
            ),
            Self::NonFiniteFixedLength { constraint } => {
                write!(
                    formatter,
                    "constraint {constraint:?} has a non-finite length"
                )
            }
            Self::NonPositiveFixedLength { constraint } => {
                write!(
                    formatter,
                    "constraint {constraint:?} requires a positive length"
                )
            }
            Self::NonFiniteFixedAngle { constraint } => {
                write!(
                    formatter,
                    "constraint {constraint:?} has a non-finite angle"
                )
            }
            Self::FixedAngleOutOfRange { constraint } => write!(
                formatter,
                "constraint {constraint:?} requires an angle in 0 through 180 degrees"
            ),
            Self::NonFiniteRotationAngle { constraint } => write!(
                formatter,
                "constraint {constraint:?} has a non-finite rotation angle"
            ),
            Self::RotationAngleOutOfRange { constraint } => write!(
                formatter,
                "constraint {constraint:?} requires a rotation angle strictly between 0 and 360 degrees"
            ),
            Self::NonFiniteLengthRatio { constraint } => {
                write!(
                    formatter,
                    "constraint {constraint:?} has a non-finite ratio"
                )
            }
            Self::NonPositiveLengthRatio { constraint } => {
                write!(
                    formatter,
                    "constraint {constraint:?} requires a positive ratio"
                )
            }
        }
    }
}

impl Error for GeometricConstraintDocumentValidationErrorV1 {}

/// Validates persisted constraint invariants that do not require geometry.
///
/// The validator is deliberately fail-closed and never clamps or normalizes
/// persisted values. It does not check whether referenced entities exist or
/// whether their geometry is usable.
pub fn validate_geometric_constraint_document_v1(
    document: &GeometricConstraintDocumentV1,
) -> Result<(), GeometricConstraintDocumentValidationErrorV1> {
    validate_geometric_constraint_document_with_limits_v1(
        document,
        DEFAULT_MAX_CONSTRAINT_RECORDS,
        DEFAULT_MAX_CONSTRAINT_REFERENCES,
    )
}

fn validate_geometric_constraint_document_with_limits_v1(
    document: &GeometricConstraintDocumentV1,
    max_constraints: usize,
    max_references: usize,
) -> Result<(), GeometricConstraintDocumentValidationErrorV1> {
    if document.schema_version != GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1 {
        return Err(
            GeometricConstraintDocumentValidationErrorV1::UnsupportedSchemaVersion {
                actual: document.schema_version,
                expected: GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
            },
        );
    }
    if document.constraints.len() > max_constraints {
        return Err(
            GeometricConstraintDocumentValidationErrorV1::TooManyConstraints {
                actual: document.constraints.len(),
                maximum: max_constraints,
            },
        );
    }
    let reference_count = checked_reference_count(
        document
            .constraints
            .iter()
            .map(|record| record.constraint.reference_count()),
    )?;
    if reference_count > max_references {
        return Err(
            GeometricConstraintDocumentValidationErrorV1::TooManyReferences {
                actual: reference_count,
                maximum: max_references,
            },
        );
    }

    let mut ordered = Vec::new();
    ordered
        .try_reserve_exact(document.constraints.len())
        .map_err(|_| GeometricConstraintDocumentValidationErrorV1::AllocationFailed)?;
    ordered.extend(document.constraints.iter());
    ordered.sort_unstable_by_key(|record| record.id.canonical_bytes());
    for pair in ordered.windows(2) {
        if pair[0].id == pair[1].id {
            return Err(
                GeometricConstraintDocumentValidationErrorV1::DuplicateConstraintId {
                    constraint: pair[1].id,
                },
            );
        }
    }
    for record in ordered {
        if record.id.is_nil() {
            return Err(GeometricConstraintDocumentValidationErrorV1::NilConstraintId);
        }
        validate_record(record)?;
    }
    Ok(())
}

fn checked_reference_count(
    counts: impl IntoIterator<Item = usize>,
) -> Result<usize, GeometricConstraintDocumentValidationErrorV1> {
    counts.into_iter().try_fold(0usize, |total, count| {
        total
            .checked_add(count)
            .ok_or(GeometricConstraintDocumentValidationErrorV1::ReferenceCountOverflow)
    })
}

fn validate_record(
    record: &GeometricConstraintRecordV1,
) -> Result<(), GeometricConstraintDocumentValidationErrorV1> {
    let constraint = record.id;
    match &record.constraint {
        GeometricConstraintKindV1::FixedLength { edge, length_mm } => {
            require_edge(constraint, *edge)?;
            if !length_mm.is_finite() {
                return Err(
                    GeometricConstraintDocumentValidationErrorV1::NonFiniteFixedLength {
                        constraint,
                    },
                );
            }
            if *length_mm <= 0.0 {
                return Err(
                    GeometricConstraintDocumentValidationErrorV1::NonPositiveFixedLength {
                        constraint,
                    },
                );
            }
        }
        GeometricConstraintKindV1::FixedAngle {
            vertex,
            first_edge,
            second_edge,
            angle_degrees,
        } => {
            require_distinct_edges(constraint, *first_edge, *second_edge)?;
            require_vertex(constraint, *vertex)?;
            require_edge(constraint, *first_edge)?;
            require_edge(constraint, *second_edge)?;
            if !angle_degrees.is_finite() {
                return Err(
                    GeometricConstraintDocumentValidationErrorV1::NonFiniteFixedAngle {
                        constraint,
                    },
                );
            }
            if !(0.0..=180.0).contains(angle_degrees) {
                return Err(
                    GeometricConstraintDocumentValidationErrorV1::FixedAngleOutOfRange {
                        constraint,
                    },
                );
            }
        }
        GeometricConstraintKindV1::Horizontal { edge }
        | GeometricConstraintKindV1::Vertical { edge } => require_edge(constraint, *edge)?,
        GeometricConstraintKindV1::EqualLength {
            first_edge,
            second_edge,
        }
        | GeometricConstraintKindV1::Parallel {
            first_edge,
            second_edge,
        } => {
            require_distinct_edges(constraint, *first_edge, *second_edge)?;
            require_edge(constraint, *first_edge)?;
            require_edge(constraint, *second_edge)?;
        }
        GeometricConstraintKindV1::PointOnLine { vertex, line_edge } => {
            require_vertex(constraint, *vertex)?;
            require_edge(constraint, *line_edge)?;
        }
        GeometricConstraintKindV1::MirrorSymmetry {
            first_vertex,
            second_vertex,
            axis_edge,
        } => {
            require_distinct_vertices(constraint, *first_vertex, *second_vertex)?;
            require_vertex(constraint, *first_vertex)?;
            require_vertex(constraint, *second_vertex)?;
            require_edge(constraint, *axis_edge)?;
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
            require_vertex(constraint, *center_vertex)?;
            require_vertex(constraint, *source_vertex)?;
            require_vertex(constraint, *target_vertex)?;
            if !angle_degrees.is_finite() {
                return Err(
                    GeometricConstraintDocumentValidationErrorV1::NonFiniteRotationAngle {
                        constraint,
                    },
                );
            }
            if *angle_degrees <= 0.0 || *angle_degrees >= 360.0 {
                return Err(
                    GeometricConstraintDocumentValidationErrorV1::RotationAngleOutOfRange {
                        constraint,
                    },
                );
            }
        }
        GeometricConstraintKindV1::AngleBisector {
            vertex,
            first_edge,
            second_edge,
            bisector_edge,
        } => {
            require_distinct_edges(constraint, *first_edge, *second_edge)?;
            require_distinct_edges(constraint, *first_edge, *bisector_edge)?;
            require_distinct_edges(constraint, *second_edge, *bisector_edge)?;
            require_vertex(constraint, *vertex)?;
            require_edge(constraint, *first_edge)?;
            require_edge(constraint, *second_edge)?;
            require_edge(constraint, *bisector_edge)?;
        }
        GeometricConstraintKindV1::LengthRatio {
            numerator_edge,
            denominator_edge,
            ratio,
        } => {
            require_distinct_edges(constraint, *numerator_edge, *denominator_edge)?;
            require_edge(constraint, *numerator_edge)?;
            require_edge(constraint, *denominator_edge)?;
            if !ratio.is_finite() {
                return Err(
                    GeometricConstraintDocumentValidationErrorV1::NonFiniteLengthRatio {
                        constraint,
                    },
                );
            }
            if *ratio <= 0.0 {
                return Err(
                    GeometricConstraintDocumentValidationErrorV1::NonPositiveLengthRatio {
                        constraint,
                    },
                );
            }
        }
    }
    Ok(())
}

fn require_vertex(
    constraint: ConstraintId,
    vertex: VertexId,
) -> Result<(), GeometricConstraintDocumentValidationErrorV1> {
    if vertex.canonical_bytes() == [0; 16] {
        Err(GeometricConstraintDocumentValidationErrorV1::NilVertexReference { constraint, vertex })
    } else {
        Ok(())
    }
}

fn require_edge(
    constraint: ConstraintId,
    edge: EdgeId,
) -> Result<(), GeometricConstraintDocumentValidationErrorV1> {
    if edge.canonical_bytes() == [0; 16] {
        Err(GeometricConstraintDocumentValidationErrorV1::NilEdgeReference { constraint, edge })
    } else {
        Ok(())
    }
}

fn require_distinct_edges(
    constraint: ConstraintId,
    first: EdgeId,
    second: EdgeId,
) -> Result<(), GeometricConstraintDocumentValidationErrorV1> {
    if first == second {
        Err(
            GeometricConstraintDocumentValidationErrorV1::RepeatedEdgeReference {
                constraint,
                edge: first,
            },
        )
    } else {
        Ok(())
    }
}

fn require_distinct_vertices(
    constraint: ConstraintId,
    first: VertexId,
    second: VertexId,
) -> Result<(), GeometricConstraintDocumentValidationErrorV1> {
    if first == second {
        Err(
            GeometricConstraintDocumentValidationErrorV1::RepeatedVertexReference {
                constraint,
                vertex: first,
            },
        )
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;

    fn constraint_id(index: usize) -> ConstraintId {
        serde_json::from_str(&format!("\"10000000-0000-4000-9000-{index:012x}\""))
            .expect("fixed constraint ID")
    }

    fn vertex_id(index: usize) -> VertexId {
        serde_json::from_str(&format!("\"20000000-0000-4000-9000-{index:012x}\""))
            .expect("fixed vertex ID")
    }

    fn edge_id(index: usize) -> EdgeId {
        serde_json::from_str(&format!("\"30000000-0000-4000-9000-{index:012x}\""))
            .expect("fixed edge ID")
    }

    fn nil_vertex_id() -> VertexId {
        serde_json::from_str("\"00000000-0000-0000-0000-000000000000\"").expect("nil vertex ID")
    }

    fn nil_edge_id() -> EdgeId {
        serde_json::from_str("\"00000000-0000-0000-0000-000000000000\"").expect("nil edge ID")
    }

    fn record(index: usize, constraint: GeometricConstraintKindV1) -> GeometricConstraintRecordV1 {
        GeometricConstraintRecordV1 {
            id: constraint_id(index),
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

    fn all_kinds() -> Vec<GeometricConstraintKindV1> {
        let vertices = [vertex_id(1), vertex_id(2), vertex_id(3)];
        let edges = [edge_id(1), edge_id(2), edge_id(3)];
        vec![
            GeometricConstraintKindV1::FixedLength {
                edge: edges[0],
                length_mm: 10.0,
            },
            GeometricConstraintKindV1::FixedAngle {
                vertex: vertices[0],
                first_edge: edges[0],
                second_edge: edges[1],
                angle_degrees: 90.0,
            },
            GeometricConstraintKindV1::Horizontal { edge: edges[0] },
            GeometricConstraintKindV1::Vertical { edge: edges[1] },
            GeometricConstraintKindV1::EqualLength {
                first_edge: edges[0],
                second_edge: edges[1],
            },
            GeometricConstraintKindV1::Parallel {
                first_edge: edges[0],
                second_edge: edges[1],
            },
            GeometricConstraintKindV1::PointOnLine {
                vertex: vertices[2],
                line_edge: edges[0],
            },
            GeometricConstraintKindV1::MirrorSymmetry {
                first_vertex: vertices[1],
                second_vertex: vertices[2],
                axis_edge: edges[0],
            },
            GeometricConstraintKindV1::RotationalSymmetry {
                center_vertex: vertices[0],
                source_vertex: vertices[1],
                target_vertex: vertices[2],
                angle_degrees: 120.0,
            },
            GeometricConstraintKindV1::AngleBisector {
                vertex: vertices[0],
                first_edge: edges[0],
                second_edge: edges[1],
                bisector_edge: edges[2],
            },
            GeometricConstraintKindV1::LengthRatio {
                numerator_edge: edges[0],
                denominator_edge: edges[1],
                ratio: 2.0,
            },
        ]
    }

    #[test]
    fn default_is_empty_and_new_ids_are_stable_non_nil_uuids() {
        let empty = GeometricConstraintDocumentV1::default();
        assert!(empty.is_empty());
        assert_eq!(empty.schema_version, GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1);
        validate_geometric_constraint_document_v1(&empty).expect("empty document");

        let id = ConstraintId::new();
        assert!(!id.is_nil());
        let wire = serde_json::to_string(&id).expect("serialize constraint ID");
        let restored: ConstraintId =
            serde_json::from_str(&wire).expect("deserialize constraint ID");
        assert_eq!(restored, id);
        assert_eq!(restored.canonical_bytes(), id.canonical_bytes());
    }

    #[test]
    fn all_eleven_kinds_round_trip_with_a_stable_strict_wire_contract() {
        const EXPECTED_WIRE: &str = concat!(
            r#"{"schema_version":1,"constraints":["#,
            r#"{"id":"10000000-0000-4000-9000-000000000001","constraint":{"kind":"fixed_length","edge":"30000000-0000-4000-9000-000000000001","length_mm":10.0}},"#,
            r#"{"id":"10000000-0000-4000-9000-000000000002","constraint":{"kind":"fixed_angle","vertex":"20000000-0000-4000-9000-000000000001","first_edge":"30000000-0000-4000-9000-000000000001","second_edge":"30000000-0000-4000-9000-000000000002","angle_degrees":90.0}},"#,
            r#"{"id":"10000000-0000-4000-9000-000000000003","constraint":{"kind":"horizontal","edge":"30000000-0000-4000-9000-000000000001"}},"#,
            r#"{"id":"10000000-0000-4000-9000-000000000004","constraint":{"kind":"vertical","edge":"30000000-0000-4000-9000-000000000002"}},"#,
            r#"{"id":"10000000-0000-4000-9000-000000000005","constraint":{"kind":"equal_length","first_edge":"30000000-0000-4000-9000-000000000001","second_edge":"30000000-0000-4000-9000-000000000002"}},"#,
            r#"{"id":"10000000-0000-4000-9000-000000000006","constraint":{"kind":"parallel","first_edge":"30000000-0000-4000-9000-000000000001","second_edge":"30000000-0000-4000-9000-000000000002"}},"#,
            r#"{"id":"10000000-0000-4000-9000-000000000007","constraint":{"kind":"point_on_line","vertex":"20000000-0000-4000-9000-000000000003","line_edge":"30000000-0000-4000-9000-000000000001"}},"#,
            r#"{"id":"10000000-0000-4000-9000-000000000008","constraint":{"kind":"mirror_symmetry","first_vertex":"20000000-0000-4000-9000-000000000002","second_vertex":"20000000-0000-4000-9000-000000000003","axis_edge":"30000000-0000-4000-9000-000000000001"}},"#,
            r#"{"id":"10000000-0000-4000-9000-000000000009","constraint":{"kind":"rotational_symmetry","center_vertex":"20000000-0000-4000-9000-000000000001","source_vertex":"20000000-0000-4000-9000-000000000002","target_vertex":"20000000-0000-4000-9000-000000000003","angle_degrees":120.0}},"#,
            r#"{"id":"10000000-0000-4000-9000-00000000000a","constraint":{"kind":"angle_bisector","vertex":"20000000-0000-4000-9000-000000000001","first_edge":"30000000-0000-4000-9000-000000000001","second_edge":"30000000-0000-4000-9000-000000000002","bisector_edge":"30000000-0000-4000-9000-000000000003"}},"#,
            r#"{"id":"10000000-0000-4000-9000-00000000000b","constraint":{"kind":"length_ratio","numerator_edge":"30000000-0000-4000-9000-000000000001","denominator_edge":"30000000-0000-4000-9000-000000000002","ratio":2.0}}]}"#,
        );
        let constraints = all_kinds()
            .into_iter()
            .enumerate()
            .map(|(index, constraint)| record(index + 1, constraint))
            .collect::<Vec<_>>();
        let original = document(constraints);
        validate_geometric_constraint_document_v1(&original).expect("all eleven kinds");
        let wire = serde_json::to_string(&original).expect("serialize all kinds");
        assert_eq!(wire, EXPECTED_WIRE, "V1 compact JSON is a wire contract");
        assert_eq!(
            serde_json::to_value(&original).expect("serialize all kinds as JSON value"),
            serde_json::from_str::<Value>(EXPECTED_WIRE).expect("parse exact V1 JSON golden"),
            "V1 field names, kind tags, values, and UUID strings are fixed"
        );
        let restored: GeometricConstraintDocumentV1 =
            serde_json::from_str(&wire).expect("deserialize all kinds");
        assert_eq!(restored, original);
        assert_eq!(restored.constraints.len(), 11);
        validate_geometric_constraint_document_v1(&restored).expect("restored all kinds");
    }

    #[test]
    fn serde_rejects_unknown_kind_and_extra_fields() {
        let original = document([record(
            1,
            GeometricConstraintKindV1::Horizontal { edge: edge_id(1) },
        )]);
        let mut unknown_kind = serde_json::to_value(&original).expect("serialize document");
        unknown_kind["constraints"][0]["constraint"]["kind"] = json!("future_constraint");
        assert!(serde_json::from_value::<GeometricConstraintDocumentV1>(unknown_kind).is_err());

        let mut extra_document = serde_json::to_value(&original).expect("serialize document");
        extra_document["future"] = Value::Bool(true);
        assert!(serde_json::from_value::<GeometricConstraintDocumentV1>(extra_document).is_err());

        let mut extra_record = serde_json::to_value(&original).expect("serialize document");
        extra_record["constraints"][0]["future"] = Value::Bool(true);
        assert!(serde_json::from_value::<GeometricConstraintDocumentV1>(extra_record).is_err());

        let mut extra_constraint = serde_json::to_value(&original).expect("serialize document");
        extra_constraint["constraints"][0]["constraint"]["future"] = Value::Bool(true);
        assert!(serde_json::from_value::<GeometricConstraintDocumentV1>(extra_constraint).is_err());
    }

    #[test]
    fn schema_nil_and_duplicate_ids_fail_closed() {
        let mut unsupported = GeometricConstraintDocumentV1::default();
        unsupported.schema_version += 1;
        assert!(matches!(
            validate_geometric_constraint_document_v1(&unsupported),
            Err(GeometricConstraintDocumentValidationErrorV1::UnsupportedSchemaVersion { .. })
        ));

        let nil_wire = format!(
            r#"{{"schema_version":1,"constraints":[{{"id":"00000000-0000-0000-0000-000000000000","constraint":{{"kind":"horizontal","edge":"{}"}}}}]}}"#,
            serde_json::to_value(edge_id(1))
                .expect("edge value")
                .as_str()
                .expect("edge string")
        );
        let nil_document: GeometricConstraintDocumentV1 =
            serde_json::from_str(&nil_wire).expect("nil ID wire");
        assert_eq!(
            validate_geometric_constraint_document_v1(&nil_document),
            Err(GeometricConstraintDocumentValidationErrorV1::NilConstraintId)
        );

        let duplicate = record(
            1,
            GeometricConstraintKindV1::Horizontal { edge: edge_id(1) },
        );
        let duplicate_id = document([duplicate.clone(), duplicate]);
        assert!(matches!(
            validate_geometric_constraint_document_v1(&duplicate_id),
            Err(GeometricConstraintDocumentValidationErrorV1::DuplicateConstraintId { .. })
        ));
    }

    #[test]
    fn every_nil_reference_kind_fails_without_geometry() {
        #[derive(Debug, Clone, Copy)]
        enum ReferenceKind {
            Vertex,
            Edge,
        }

        let v1 = vertex_id(1);
        let v2 = vertex_id(2);
        let v3 = vertex_id(3);
        let e1 = edge_id(1);
        let e2 = edge_id(2);
        let e3 = edge_id(3);
        let cases = vec![
            (
                "fixed_length.edge",
                GeometricConstraintKindV1::FixedLength {
                    edge: nil_edge_id(),
                    length_mm: 10.0,
                },
                ReferenceKind::Edge,
            ),
            (
                "fixed_angle.vertex",
                GeometricConstraintKindV1::FixedAngle {
                    vertex: nil_vertex_id(),
                    first_edge: e1,
                    second_edge: e2,
                    angle_degrees: 90.0,
                },
                ReferenceKind::Vertex,
            ),
            (
                "fixed_angle.first_edge",
                GeometricConstraintKindV1::FixedAngle {
                    vertex: v1,
                    first_edge: nil_edge_id(),
                    second_edge: e2,
                    angle_degrees: 90.0,
                },
                ReferenceKind::Edge,
            ),
            (
                "fixed_angle.second_edge",
                GeometricConstraintKindV1::FixedAngle {
                    vertex: v1,
                    first_edge: e1,
                    second_edge: nil_edge_id(),
                    angle_degrees: 90.0,
                },
                ReferenceKind::Edge,
            ),
            (
                "horizontal.edge",
                GeometricConstraintKindV1::Horizontal {
                    edge: nil_edge_id(),
                },
                ReferenceKind::Edge,
            ),
            (
                "vertical.edge",
                GeometricConstraintKindV1::Vertical {
                    edge: nil_edge_id(),
                },
                ReferenceKind::Edge,
            ),
            (
                "equal_length.first_edge",
                GeometricConstraintKindV1::EqualLength {
                    first_edge: nil_edge_id(),
                    second_edge: e2,
                },
                ReferenceKind::Edge,
            ),
            (
                "equal_length.second_edge",
                GeometricConstraintKindV1::EqualLength {
                    first_edge: e1,
                    second_edge: nil_edge_id(),
                },
                ReferenceKind::Edge,
            ),
            (
                "parallel.first_edge",
                GeometricConstraintKindV1::Parallel {
                    first_edge: nil_edge_id(),
                    second_edge: e2,
                },
                ReferenceKind::Edge,
            ),
            (
                "parallel.second_edge",
                GeometricConstraintKindV1::Parallel {
                    first_edge: e1,
                    second_edge: nil_edge_id(),
                },
                ReferenceKind::Edge,
            ),
            (
                "point_on_line.vertex",
                GeometricConstraintKindV1::PointOnLine {
                    vertex: nil_vertex_id(),
                    line_edge: e1,
                },
                ReferenceKind::Vertex,
            ),
            (
                "point_on_line.line_edge",
                GeometricConstraintKindV1::PointOnLine {
                    vertex: v1,
                    line_edge: nil_edge_id(),
                },
                ReferenceKind::Edge,
            ),
            (
                "mirror_symmetry.first_vertex",
                GeometricConstraintKindV1::MirrorSymmetry {
                    first_vertex: nil_vertex_id(),
                    second_vertex: v2,
                    axis_edge: e1,
                },
                ReferenceKind::Vertex,
            ),
            (
                "mirror_symmetry.second_vertex",
                GeometricConstraintKindV1::MirrorSymmetry {
                    first_vertex: v1,
                    second_vertex: nil_vertex_id(),
                    axis_edge: e1,
                },
                ReferenceKind::Vertex,
            ),
            (
                "mirror_symmetry.axis_edge",
                GeometricConstraintKindV1::MirrorSymmetry {
                    first_vertex: v1,
                    second_vertex: v2,
                    axis_edge: nil_edge_id(),
                },
                ReferenceKind::Edge,
            ),
            (
                "rotational_symmetry.center_vertex",
                GeometricConstraintKindV1::RotationalSymmetry {
                    center_vertex: nil_vertex_id(),
                    source_vertex: v2,
                    target_vertex: v3,
                    angle_degrees: 120.0,
                },
                ReferenceKind::Vertex,
            ),
            (
                "rotational_symmetry.source_vertex",
                GeometricConstraintKindV1::RotationalSymmetry {
                    center_vertex: v1,
                    source_vertex: nil_vertex_id(),
                    target_vertex: v3,
                    angle_degrees: 120.0,
                },
                ReferenceKind::Vertex,
            ),
            (
                "rotational_symmetry.target_vertex",
                GeometricConstraintKindV1::RotationalSymmetry {
                    center_vertex: v1,
                    source_vertex: v2,
                    target_vertex: nil_vertex_id(),
                    angle_degrees: 120.0,
                },
                ReferenceKind::Vertex,
            ),
            (
                "angle_bisector.vertex",
                GeometricConstraintKindV1::AngleBisector {
                    vertex: nil_vertex_id(),
                    first_edge: e1,
                    second_edge: e2,
                    bisector_edge: e3,
                },
                ReferenceKind::Vertex,
            ),
            (
                "angle_bisector.first_edge",
                GeometricConstraintKindV1::AngleBisector {
                    vertex: v1,
                    first_edge: nil_edge_id(),
                    second_edge: e2,
                    bisector_edge: e3,
                },
                ReferenceKind::Edge,
            ),
            (
                "angle_bisector.second_edge",
                GeometricConstraintKindV1::AngleBisector {
                    vertex: v1,
                    first_edge: e1,
                    second_edge: nil_edge_id(),
                    bisector_edge: e3,
                },
                ReferenceKind::Edge,
            ),
            (
                "angle_bisector.bisector_edge",
                GeometricConstraintKindV1::AngleBisector {
                    vertex: v1,
                    first_edge: e1,
                    second_edge: e2,
                    bisector_edge: nil_edge_id(),
                },
                ReferenceKind::Edge,
            ),
            (
                "length_ratio.numerator_edge",
                GeometricConstraintKindV1::LengthRatio {
                    numerator_edge: nil_edge_id(),
                    denominator_edge: e2,
                    ratio: 2.0,
                },
                ReferenceKind::Edge,
            ),
            (
                "length_ratio.denominator_edge",
                GeometricConstraintKindV1::LengthRatio {
                    numerator_edge: e1,
                    denominator_edge: nil_edge_id(),
                    ratio: 2.0,
                },
                ReferenceKind::Edge,
            ),
        ];
        assert_eq!(cases.len(), 24, "all eleven kinds expose 24 roles");

        for (index, (role, constraint, expected_kind)) in cases.into_iter().enumerate() {
            let expected_constraint = constraint_id(index + 1);
            let result = validate_geometric_constraint_document_v1(&document([record(
                index + 1,
                constraint,
            )]));
            match (expected_kind, result) {
                (
                    ReferenceKind::Vertex,
                    Err(GeometricConstraintDocumentValidationErrorV1::NilVertexReference {
                        constraint,
                        vertex,
                    }),
                ) => {
                    assert_eq!(constraint, expected_constraint, "{role}");
                    assert_eq!(vertex, nil_vertex_id(), "{role}");
                }
                (
                    ReferenceKind::Edge,
                    Err(GeometricConstraintDocumentValidationErrorV1::NilEdgeReference {
                        constraint,
                        edge,
                    }),
                ) => {
                    assert_eq!(constraint, expected_constraint, "{role}");
                    assert_eq!(edge, nil_edge_id(), "{role}");
                }
                (expected, actual) => {
                    panic!("{role}: expected nil {expected:?} error, got {actual:?}")
                }
            }
        }
    }

    #[test]
    fn scalar_finite_and_range_contract_is_fail_closed_at_exact_bounds() {
        let valid = [
            GeometricConstraintKindV1::FixedLength {
                edge: edge_id(1),
                length_mm: f64::MIN_POSITIVE,
            },
            GeometricConstraintKindV1::FixedAngle {
                vertex: vertex_id(1),
                first_edge: edge_id(1),
                second_edge: edge_id(2),
                angle_degrees: 0.0,
            },
            GeometricConstraintKindV1::FixedAngle {
                vertex: vertex_id(1),
                first_edge: edge_id(1),
                second_edge: edge_id(2),
                angle_degrees: 180.0,
            },
            GeometricConstraintKindV1::RotationalSymmetry {
                center_vertex: vertex_id(1),
                source_vertex: vertex_id(2),
                target_vertex: vertex_id(3),
                angle_degrees: f64::MIN_POSITIVE,
            },
            GeometricConstraintKindV1::LengthRatio {
                numerator_edge: edge_id(1),
                denominator_edge: edge_id(2),
                ratio: f64::MIN_POSITIVE,
            },
        ];
        for (index, constraint) in valid.into_iter().enumerate() {
            validate_geometric_constraint_document_v1(&document([record(index + 1, constraint)]))
                .expect("inclusive/exact valid scalar bound");
        }

        let invalid = [
            GeometricConstraintKindV1::FixedLength {
                edge: edge_id(1),
                length_mm: f64::NAN,
            },
            GeometricConstraintKindV1::FixedLength {
                edge: edge_id(1),
                length_mm: 0.0,
            },
            GeometricConstraintKindV1::FixedAngle {
                vertex: vertex_id(1),
                first_edge: edge_id(1),
                second_edge: edge_id(2),
                angle_degrees: f64::INFINITY,
            },
            GeometricConstraintKindV1::FixedAngle {
                vertex: vertex_id(1),
                first_edge: edge_id(1),
                second_edge: edge_id(2),
                angle_degrees: 180.0 + f64::EPSILON * 180.0,
            },
            GeometricConstraintKindV1::RotationalSymmetry {
                center_vertex: vertex_id(1),
                source_vertex: vertex_id(2),
                target_vertex: vertex_id(3),
                angle_degrees: 360.0,
            },
            GeometricConstraintKindV1::LengthRatio {
                numerator_edge: edge_id(1),
                denominator_edge: edge_id(2),
                ratio: -1.0,
            },
        ];
        for (index, constraint) in invalid.into_iter().enumerate() {
            assert!(
                validate_geometric_constraint_document_v1(&document([record(
                    index + 1,
                    constraint
                )]))
                .is_err()
            );
        }
    }

    #[test]
    fn repeated_entity_roles_fail_without_geometry() {
        let edge = edge_id(1);
        let vertex = vertex_id(1);
        let cases = [
            GeometricConstraintKindV1::FixedAngle {
                vertex,
                first_edge: edge,
                second_edge: edge,
                angle_degrees: 90.0,
            },
            GeometricConstraintKindV1::EqualLength {
                first_edge: edge,
                second_edge: edge,
            },
            GeometricConstraintKindV1::Parallel {
                first_edge: edge,
                second_edge: edge,
            },
            GeometricConstraintKindV1::MirrorSymmetry {
                first_vertex: vertex,
                second_vertex: vertex,
                axis_edge: edge,
            },
            GeometricConstraintKindV1::RotationalSymmetry {
                center_vertex: vertex,
                source_vertex: vertex,
                target_vertex: vertex_id(2),
                angle_degrees: 90.0,
            },
            GeometricConstraintKindV1::AngleBisector {
                vertex,
                first_edge: edge,
                second_edge: edge_id(2),
                bisector_edge: edge,
            },
            GeometricConstraintKindV1::LengthRatio {
                numerator_edge: edge,
                denominator_edge: edge,
                ratio: 1.0,
            },
        ];
        for (index, constraint) in cases.into_iter().enumerate() {
            assert!(matches!(
                validate_geometric_constraint_document_v1(&document([record(
                    index + 1,
                    constraint
                )])),
                Err(
                    GeometricConstraintDocumentValidationErrorV1::RepeatedEdgeReference { .. }
                        | GeometricConstraintDocumentValidationErrorV1::RepeatedVertexReference { .. }
                )
            ));
        }
    }

    #[test]
    fn hard_record_and_reference_ceilings_admit_equality_and_reject_one_over() {
        let exact_records = document((0..DEFAULT_MAX_CONSTRAINT_RECORDS).map(|index| {
            record(
                index + 1,
                GeometricConstraintKindV1::Horizontal { edge: edge_id(1) },
            )
        }));
        validate_geometric_constraint_document_v1(&exact_records)
            .expect("hard record ceiling is inclusive");

        let one_over_records = document((0..=DEFAULT_MAX_CONSTRAINT_RECORDS).map(|index| {
            record(
                index + 1,
                GeometricConstraintKindV1::Horizontal { edge: edge_id(1) },
            )
        }));
        assert_eq!(
            validate_geometric_constraint_document_v1(&one_over_records),
            Err(
                GeometricConstraintDocumentValidationErrorV1::TooManyConstraints {
                    actual: DEFAULT_MAX_CONSTRAINT_RECORDS + 1,
                    maximum: DEFAULT_MAX_CONSTRAINT_RECORDS,
                }
            )
        );

        let exact_references = document((0..DEFAULT_MAX_CONSTRAINT_RECORDS).map(|index| {
            record(
                index + 1,
                GeometricConstraintKindV1::AngleBisector {
                    vertex: vertex_id(1),
                    first_edge: edge_id(1),
                    second_edge: edge_id(2),
                    bisector_edge: edge_id(3),
                },
            )
        }));
        assert_eq!(
            exact_references
                .constraints
                .iter()
                .map(|record| record.constraint.reference_count())
                .sum::<usize>(),
            DEFAULT_MAX_CONSTRAINT_REFERENCES
        );
        validate_geometric_constraint_document_v1(&exact_references)
            .expect("hard reference ceiling is inclusive");

        let one_record = document([record(
            1,
            GeometricConstraintKindV1::AngleBisector {
                vertex: vertex_id(1),
                first_edge: edge_id(1),
                second_edge: edge_id(2),
                bisector_edge: edge_id(3),
            },
        )]);
        assert_eq!(
            validate_geometric_constraint_document_with_limits_v1(&one_record, 1, 3),
            Err(
                GeometricConstraintDocumentValidationErrorV1::TooManyReferences {
                    actual: 4,
                    maximum: 3,
                }
            )
        );
    }

    #[test]
    fn reference_counting_uses_checked_addition() {
        assert_eq!(
            checked_reference_count([usize::MAX, 1]),
            Err(GeometricConstraintDocumentValidationErrorV1::ReferenceCountOverflow)
        );
    }
}
