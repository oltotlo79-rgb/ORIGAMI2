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

use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub use ori_domain::{
    ConstraintId, DEFAULT_MAX_CONSTRAINT_EDGES, DEFAULT_MAX_CONSTRAINT_RECORDS,
    DEFAULT_MAX_CONSTRAINT_REFERENCES, DEFAULT_MAX_CONSTRAINT_VERTICES,
    GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1, GeometricConstraintDocumentV1,
    GeometricConstraintDocumentValidationErrorV1, GeometricConstraintKindV1,
    GeometricConstraintRecordV1, validate_geometric_constraint_document_v1,
};
use ori_domain::{CreasePattern, Edge, EdgeId, Vertex, VertexId};
use ori_numeric::{
    deterministic_atan2_v1, deterministic_degrees_to_radians_v1, deterministic_sin_cos_degrees_v1,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

mod bounded_zero_closure;
mod directed_ratio_closure;

/// Stable semantic identifier for the first geometric-constraint model.
pub const GEOMETRIC_CONSTRAINT_MODEL_ID_V1: &str = "geometric_constraints_v1";

/// Default and non-relaxable V1 preflight-record-count ceiling.
pub const DEFAULT_MAX_CONSTRAINT_PRECHECKS: usize = 10_000;
/// Maximum size of one deterministic direct-conflict cause witness.
pub const MAX_DIRECT_CONFLICT_CAUSE_IDS_V1: usize = 256;
const MAX_GENERAL_EQUAL_GRAPH_WORK_V1: u64 = 40_000;
const MAX_GENERAL_PARALLEL_GRAPH_WORK_V1: u64 = 40_000;

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

    /// Runs the same bounded direct preflight with cooperative cancellation
    /// and deadline checkpoints.
    #[must_use]
    pub fn preflight_with_observer(
        &self,
        observer: &mut impl GeometricConstraintPreflightObserverV1,
    ) -> ConstraintPreflightV1 {
        preflight_direct_conflicts_with_observer_v1(self, observer)
    }
}

/// Constraint family whose production binary64 residual rejects collapse of
/// the reported edge in every possible signed-zero execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ZeroLengthClosureProviderKindV1 {
    PointOnLine,
    MirrorSymmetryAxis,
    AngleBisector,
    Parallel,
    FixedAngle,
}

/// Stable V1 wire tags for direct-conflict output.
///
/// Some legacy variants remain for serialization compatibility even though
/// their binary64 recognizers are now quarantined as solver-required
/// candidates. Native output emits only variants accepted by the internal
/// residual-proof allowlist.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DirectConstraintConflictKindV1 {
    DifferentFixedLengths {
        edge: EdgeId,
    },
    /// Two fixed-angle records name the same validated vertex and unordered
    /// edge pair, and their conservative deterministic-binary64 zero-residual
    /// enclosures are disjoint. Merely differing stored degree bits are not
    /// sufficient: adjacent values and values that collapse during the frozen
    /// degree-to-radian conversion remain solver-required.
    DifferentFixedAngles {
        vertex: VertexId,
        first_edge: EdgeId,
        second_edge: EdgeId,
    },
    /// Two bit-distinct positive finite ratios target the same ordered edge
    /// pair, and the denominator has one consistent positive finite fixed
    /// length. The three-record witness is emitted only when the two
    /// denominator products, evaluated in the solver's binary64 operation
    /// order, differ numerically or at least one product is non-finite.
    DifferentLengthRatios {
        numerator_edge: EdgeId,
        denominator_edge: EdgeId,
    },
    /// Horizontal and vertical constraints target the same edge while a third
    /// exact constraint forbids the zero-length edge that would satisfy both
    /// orientations.
    ///
    /// The third cause is either a consistent positive `FixedLength`, or a
    /// `PointOnLine` line, `MirrorSymmetry` axis, or `AngleBisector` edge role
    /// whose solver residual rejects collapse. Only the exact edge ID is used;
    /// current coordinates and geometrically coincident alias edges are not
    /// evidence for this contradiction.
    HorizontalAndVertical {
        edge: EdgeId,
    },
    EqualLengthWithDifferentFixedLengths {
        first_edge: EdgeId,
        second_edge: EdgeId,
    },
    /// `EqualLength` forces both edge lengths to the same positive binary64
    /// value supplied by one consistent `FixedLength`. Evaluating the joined
    /// `LengthRatio` in the solver's exact multiplication-then-subtraction
    /// order produces a non-zero or non-finite residual at that forced value.
    EqualLengthWithNonUnitRatioAndFixedLength {
        first_edge: EdgeId,
        second_edge: EdgeId,
    },
    /// Opposing `LengthRatio` records join the same exact edge IDs and one
    /// edge has a consistent positive finite fixed length. The other edge is
    /// derived once through the ratio residual that names the fixed edge as
    /// denominator; substituting that binary64 result into the opposing
    /// production residual yields a non-zero or non-finite closure.
    NonReciprocalLengthRatiosWithFixedLength {
        first_edge: EdgeId,
        second_edge: EdgeId,
    },
    /// Both edges have consistent positive finite fixed lengths, and evaluating
    /// the exact binary64 operation used by the solver's length-ratio residual
    /// with those two lengths produces a non-zero or non-finite result.
    LengthRatioWithIncompatibleFixedLengths {
        numerator_edge: EdgeId,
        denominator_edge: EdgeId,
    },
    /// Three directed `LengthRatio` records form the reported exact edge
    /// cycle and one edge has a consistent positive finite fixed length.
    /// Following the two residuals that derive the other edge lengths, once
    /// each in production binary64 order, makes the final shared residual
    /// non-zero or non-finite.
    NonUnitLengthRatioCycleWithFixedLength {
        first_edge: EdgeId,
        second_edge: EdgeId,
        third_edge: EdgeId,
        fixed_edge: EdgeId,
    },
    /// A consistent positive finite `FixedLength` is used as the sole root of
    /// a directed graph of consistent positive finite `LengthRatio` records.
    /// Every arc is followed only from denominator to numerator using one
    /// production binary64 multiplication. Two root-derived paths close at one
    /// already forced finite edge, where the production residual is non-zero
    /// or non-finite. The canonical witness contains the root constraint and
    /// between three and 255 ratio constraints.
    InconsistentLengthRatioGraphWithFixedLength {
        fixed_edge: EdgeId,
        ratio_constraint_count: u16,
    },
    DifferentFixedLengthsInEqualLengthComponent {
        first_edge: EdgeId,
        second_edge: EdgeId,
        equal_constraint_count: u16,
    },
    PerpendicularOrientationsInParallelComponent {
        horizontal_edge: EdgeId,
        vertical_edge: EdgeId,
        parallel_constraint_count: u16,
    },
    NonParallelFixedAngleInParallelComponent {
        vertex: VertexId,
        first_edge: EdgeId,
        second_edge: EdgeId,
        parallel_constraint_count: u16,
    },
    /// Two edges have one parallel record, exact unit fixed lengths, and a
    /// fixed-angle residual that rejects every zero-cross class.  Unit
    /// lengths make the solver's normalized parallel denominator exactly one,
    /// so the parallel residual directly binds the raw cross used by the
    /// fixed-angle residual.  Other length scales remain solver-required.
    ParallelWithFixedNonParallelAngle {
        first_edge: EdgeId,
        second_edge: EdgeId,
    },
    ParallelWithPerpendicularOrientations {
        horizontal_edge: EdgeId,
        vertical_edge: EdgeId,
    },
    /// Two exact, distinct pattern edges share the validated `FixedAngle`
    /// vertex and are both constrained horizontal or both constrained
    /// vertical. Their proof-evaluator cross term is therefore either signed
    /// zero or non-finite, including every collapse and endpoint-direction
    /// case. The conflict is emitted only when the frozen fixed-angle residual
    /// rejects every finite deterministic `atan2(+0, dot)` class, including
    /// both signed-zero dot values. Stored-degree inequality, tolerance, and
    /// current coordinates are never used as evidence.
    SameOrientationWithFixedNonParallelAngle {
        first_edge: EdgeId,
        second_edge: EdgeId,
    },
    /// Two exact, distinct pattern edges share the validated `FixedAngle`
    /// vertex; one is constrained horizontal and the other vertical. At exact
    /// orientation residual zero, frozen `abs(cross)` and deterministic
    /// `atan2` can reach only the enumerated zero-cross, right-angle,
    /// straight-angle, or non-finite classes, including underflow, overflow,
    /// and collapse. The conflict is emitted only when the deterministic
    /// fixed-angle residual rejects every class. Stored-degree inequality,
    /// tolerance, and current coordinates are never evidence.
    PerpendicularOrientationsWithFixedNonRightAngle {
        horizontal_edge: EdgeId,
        vertical_edge: EdgeId,
    },
    /// Two rotational-symmetry records have the same ordered roles but
    /// distinct exact cardinal matrices under the frozen deterministic
    /// transcendental model. A consistent positive fixed length binds a real
    /// center-source or center-target radius edge, so the only common fixed
    /// point of those matrices (the zero vector) is impossible. Stored angle
    /// inequality alone is never evidence.
    DifferentRotationalSymmetryAnglesWithFixedRadius {
        center_vertex: VertexId,
        source_vertex: VertexId,
        target_vertex: VertexId,
        fixed_radius_edge: EdgeId,
    },
    /// Two rotational-symmetry records have exactly reversed source/target
    /// roles at the same center. Both rotations are exact non-identity
    /// cardinal matrices under the frozen deterministic transcendental model,
    /// and their quarter-turn composition is not identity. A consistent
    /// positive fixed length binds a real center-source or center-target
    /// radius edge, excluding the only common solution: role collapse.
    /// Stored-angle sums, epsilon, platform trigonometry, and current
    /// coordinates are never evidence.
    NonComplementaryInverseRotationalSymmetryAnglesWithFixedRadius {
        center_vertex: VertexId,
        source_vertex: VertexId,
        target_vertex: VertexId,
        fixed_radius_edge: EdgeId,
    },
    /// Legacy wire tag retained for compatibility. Independently rounded
    /// mirror and point-on-line residuals can admit positive separation. The
    /// exact-unit horizontal-axis subset whose selected point is forced to the
    /// axis start by an exact horizontal/vertical connector is proven
    /// separately; the generic three-record form remains solver-required.
    MirrorSymmetryWithPointOnAxisAndFixedSeparation {
        first_vertex: VertexId,
        second_vertex: VertexId,
        axis_edge: EdgeId,
        fixed_separation_edge: EdgeId,
    },
    /// Legacy wire tag retained for compatibility. A stored non-half-turn can
    /// round to an implemented identity or half-turn residual. The exact
    /// quarter-turn, exact-unit horizontal-radius subset is proven separately;
    /// the generic two-record form remains solver-required.
    RotationalSymmetryWithCollinearRadius {
        center_vertex: VertexId,
        source_vertex: VertexId,
        target_vertex: VertexId,
        line_edge: EdgeId,
    },
    /// A bounded, graph-derived exact-zero proof forces one edge length to
    /// binary64 zero, then propagates that zero through `EqualLength` and the
    /// denominator-to-numerator direction of `LengthRatio` until it reaches an
    /// edge with a positive finite `FixedLength`.
    ///
    /// This is deliberately a distinct V1 wire tag rather than disguising a
    /// variable-length graph proof as one of the legacy fixed-size direct
    /// families.
    PositiveFixedLengthInBoundedZeroLengthClosure {
        fixed_edge: EdgeId,
        forced_zero_edge: EdgeId,
        horizontal_constraint_count: u16,
        vertical_constraint_count: u16,
        zero_propagation_constraint_count: u16,
    },
    /// A bounded exact-zero implication closure reaches an edge role whose
    /// production residual is proven to reject collapse. The provider record
    /// itself is the final witness ID.
    ///
    /// This does not infer a general relation for the provider family. In
    /// particular, `Parallel` is used only as a non-degeneracy terminal, and a
    /// `FixedAngle` provider is admitted only after evaluating both collapsed
    /// `atan2` outcomes (zero and pi) through the production residual helper.
    ZeroLengthClosureReachesNondegenerateProvider {
        provider_kind: ZeroLengthClosureProviderKindV1,
        provider_edge: EdgeId,
        forced_zero_edge: EdgeId,
        horizontal_constraint_count: u16,
        vertical_constraint_count: u16,
        zero_propagation_constraint_count: u16,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DirectConstraintConflictV1 {
    conflict: DirectConstraintConflictKindV1,
    /// Canonically sorted, duplicate-free witness for an emitted, allowlisted
    /// contradiction. Native preflight does not emit candidate witnesses for
    /// the retained legacy variants.
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
    ConstraintLimitExceeded,
    StorageLimitExceeded,
    Cancelled,
    DeadlineReached,
    SolverRequiredConstraintKinds,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeometricConstraintPreflightObserverControlV1 {
    Continue,
    Cancelled,
    DeadlineReached,
}

/// Cooperative stop hook shared by whole-document direct preflight and the
/// separately bounded subset oracle at the desktop analysis boundary.
pub trait GeometricConstraintPreflightObserverV1 {
    fn checkpoint(&mut self) -> GeometricConstraintPreflightObserverControlV1;
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

pub const MAX_BOUNDED_DIRECT_MUS_CONSTRAINTS_V1: usize = 16;
pub const MAX_BOUNDED_DIRECT_MUS_ORACLE_CALLS_V1: usize =
    (1_usize << MAX_BOUNDED_DIRECT_MUS_CONSTRAINTS_V1) - 1;

/// Sound but intentionally incomplete subset oracle over the allowlisted
/// contradiction theorems. `Unknown` never means satisfiable.
///
/// `ProvenUnsatisfiable` is the canonical cardinality-smallest subset accepted
/// by this oracle. The legacy `Mus` type name does not claim that deleting each
/// returned ID has been accompanied by an independent semantic satisfiability
/// certificate for the full nonlinear eleven-kind residual language.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum BoundedDirectMusV1 {
    ProvenUnsatisfiable {
        constraint_ids: Vec<ConstraintId>,
        oracle_calls: usize,
    },
    Unknown {
        oracle_calls: usize,
    },
}

/// Cooperative stop hook for bounded subset enumeration.
///
/// Cancellation is fail-closed: it produces `Unknown` with the number of
/// completed oracle calls and can never manufacture an unsatisfiability proof.
pub trait BoundedDirectMusObserverV1 {
    fn should_cancel(&mut self, completed_oracle_calls: usize) -> bool;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct NoopBoundedDirectMusObserverV1;

impl BoundedDirectMusObserverV1 for NoopBoundedDirectMusObserverV1 {
    fn should_cancel(&mut self, _completed_oracle_calls: usize) -> bool {
        false
    }
}

pub fn find_bounded_direct_mus_v1(set: &GeometricConstraintSetV1<'_>) -> BoundedDirectMusV1 {
    find_bounded_direct_mus_with_observer_v1(set, &mut NoopBoundedDirectMusObserverV1)
}

pub fn find_bounded_direct_mus_with_observer_v1(
    set: &GeometricConstraintSetV1<'_>,
    observer: &mut impl BoundedDirectMusObserverV1,
) -> BoundedDirectMusV1 {
    let count = set.constraints.len();
    if count == 0 || count > MAX_BOUNDED_DIRECT_MUS_CONSTRAINTS_V1 {
        return BoundedDirectMusV1::Unknown { oracle_calls: 0 };
    }
    let mut oracle_calls = 0_usize;
    for size in 1..=count {
        for mask in 1_u64..(1_u64 << count) {
            if mask.count_ones() as usize != size {
                continue;
            }
            if observer.should_cancel(oracle_calls) {
                return BoundedDirectMusV1::Unknown { oracle_calls };
            }
            oracle_calls += 1;
            debug_assert!(oracle_calls <= MAX_BOUNDED_DIRECT_MUS_ORACLE_CALLS_V1);
            let constraints = set
                .constraints
                .iter()
                .enumerate()
                .filter(|(index, _)| mask & (1_u64 << index) != 0)
                .map(|(_, record)| record.clone())
                .collect::<Vec<_>>();
            let candidate = GeometricConstraintSetV1 {
                source_pattern: set.source_pattern,
                constraints,
                max_preflight_checks: set.max_preflight_checks,
            };
            if matches!(
                preflight_direct_conflicts_v1(&candidate),
                ConstraintPreflightV1::DirectConflict { .. }
            ) {
                return BoundedDirectMusV1::ProvenUnsatisfiable {
                    constraint_ids: candidate
                        .constraints
                        .iter()
                        .map(|record| record.id)
                        .collect(),
                    oracle_calls,
                };
            }
        }
    }
    BoundedDirectMusV1::Unknown { oracle_calls }
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

/// Role-ordered rotational-symmetry identity.
///
/// The three roles are kept in place, so a swapped `source`/`target` or any
/// other role permutation is a different relation and never joins the same
/// angle group.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct RotationRoleKey {
    center: CanonicalId,
    source: CanonicalId,
    target: CanonicalId,
}

impl RotationRoleKey {
    fn roles(center: VertexId, source: VertexId, target: VertexId) -> Self {
        Self {
            center: center.canonical_bytes(),
            source: source.canonical_bytes(),
            target: target.canonical_bytes(),
        }
    }

    fn inverse(self) -> Self {
        Self {
            center: self.center,
            source: self.target,
            target: self.source,
        }
    }
}

/// Canonical mirror relation with an unordered mirrored vertex pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct MirrorAxisKey {
    first: CanonicalId,
    second: CanonicalId,
    axis: CanonicalId,
}

impl MirrorAxisKey {
    fn relation(first: VertexId, second: VertexId, axis: EdgeId) -> Self {
        let first_bytes = first.canonical_bytes();
        let second_bytes = second.canonical_bytes();
        let (first, second) = if first_bytes < second_bytes {
            (first_bytes, second_bytes)
        } else {
            (second_bytes, first_bytes)
        };
        Self {
            first,
            second,
            axis: axis.canonical_bytes(),
        }
    }
}

/// Canonical unordered vertex pair used to find a real radius edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct VertexPairKey {
    first: CanonicalId,
    second: CanonicalId,
}

impl VertexPairKey {
    fn unordered(first: VertexId, second: VertexId) -> Self {
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
        if assignment.value.to_bits() == self.representative.value.to_bits() {
            if assignment.id.canonical_bytes() < self.representative.id.canonical_bytes() {
                self.representative = assignment;
            }
        } else if self
            .first_different
            .is_none_or(|current| assignment.id.canonical_bytes() < current.id.canonical_bytes())
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

/// Exact cardinal rotation classes produced by the frozen proof evaluator.
///
/// The tuple order is `(sin, cos)`, matching
/// `deterministic_sin_cos_degrees_v1` and the production rotation residual.
/// Only bit-exact coefficients are admitted. Subnormal stored angles that
/// underflow to the identity matrix are deliberately left solver-required.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RotationCardinalClass {
    Identity,
    QuarterTurn,
    HalfTurn,
    ThreeQuarterTurn,
}

impl RotationCardinalClass {
    fn from_angle_degrees(angle_degrees: f64) -> Option<Self> {
        if !angle_degrees.is_normal() {
            return None;
        }
        let (sin, cos) = deterministic_sin_cos_degrees_v1(angle_degrees).ok()?;
        match (sin.to_bits(), cos.to_bits()) {
            (sin_bits, cos_bits)
                if sin_bits == 0.0_f64.to_bits() && cos_bits == 1.0_f64.to_bits() =>
            {
                Some(Self::Identity)
            }
            (sin_bits, cos_bits)
                if sin_bits == 1.0_f64.to_bits() && cos_bits == 0.0_f64.to_bits() =>
            {
                Some(Self::QuarterTurn)
            }
            (sin_bits, cos_bits)
                if sin_bits == 0.0_f64.to_bits() && cos_bits == (-1.0_f64).to_bits() =>
            {
                Some(Self::HalfTurn)
            }
            (sin_bits, cos_bits)
                if sin_bits == (-1.0_f64).to_bits() && cos_bits == 0.0_f64.to_bits() =>
            {
                Some(Self::ThreeQuarterTurn)
            }
            _ => None,
        }
    }

    const fn index(self) -> usize {
        match self {
            Self::Identity => 0,
            Self::QuarterTurn => 1,
            Self::HalfTurn => 2,
            Self::ThreeQuarterTurn => 3,
        }
    }

    const fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(Self::Identity),
            1 => Some(Self::QuarterTurn),
            2 => Some(Self::HalfTurn),
            3 => Some(Self::ThreeQuarterTurn),
            _ => None,
        }
    }

    const fn is_non_identity(self) -> bool {
        !matches!(self, Self::Identity)
    }

    const fn quarter_turns(self) -> u8 {
        match self {
            Self::Identity => 0,
            Self::QuarterTurn => 1,
            Self::HalfTurn => 2,
            Self::ThreeQuarterTurn => 3,
        }
    }

    const fn composes_to_identity(self, other: Self) -> bool {
        (self.quarter_turns() + other.quarter_turns()).is_multiple_of(4)
    }
}

/// Bounded, order-independent summary for one exact ordered rotation role.
///
/// One canonical-smallest record is retained per cardinal class. Selecting
/// the two globally smallest occupied slots gives the lexicographically
/// smallest sorted witness pair without constructing quadratic pairs.
#[derive(Debug, Clone, Copy, Default)]
struct RotationCardinalGroupSummary {
    by_class: [Option<ScalarAssignment>; 4],
}

impl RotationCardinalGroupSummary {
    fn observe(&mut self, class: RotationCardinalClass, assignment: ScalarAssignment) {
        let slot = &mut self.by_class[class.index()];
        if slot.is_none_or(|current| assignment.id.canonical_bytes() < current.id.canonical_bytes())
        {
            *slot = Some(assignment);
        }
    }

    fn different_witness(&self) -> Option<[ConstraintId; 2]> {
        let mut first: Option<ScalarAssignment> = None;
        let mut second: Option<ScalarAssignment> = None;
        for assignment in self.by_class.iter().flatten().copied() {
            if first.is_none_or(|current| {
                assignment.id.canonical_bytes() < current.id.canonical_bytes()
            }) {
                second = first;
                first = Some(assignment);
            } else if second.is_none_or(|current| {
                assignment.id.canonical_bytes() < current.id.canonical_bytes()
            }) {
                second = Some(assignment);
            }
        }
        let (first, second) = (first?, second?);
        Some([first.id, second.id])
    }

    /// Returns the canonical exact 90 or 270-degree witness. These are the
    /// only cardinal rotations that map a horizontal nonzero radius to a
    /// vertical nonzero radius without a rounded coefficient.
    fn quarter_turn_witness(&self) -> Option<ScalarAssignment> {
        [
            self.by_class[RotationCardinalClass::QuarterTurn.index()],
            self.by_class[RotationCardinalClass::ThreeQuarterTurn.index()],
        ]
        .into_iter()
        .flatten()
        .min_by_key(|assignment| assignment.id.canonical_bytes())
    }

    /// Chooses the canonical two-record witness whose exact cardinal
    /// composition is not identity.
    ///
    /// The summaries belong to exactly reversed role keys. Identity matrices
    /// are excluded even if a future frozen evaluator makes another stored
    /// angle map to identity: this proof family is intentionally limited to
    /// the exact 90/180/270-degree classes.
    fn nonidentity_inverse_composition_witness(&self, inverse: &Self) -> Option<[ConstraintId; 2]> {
        let mut best: Option<[ConstraintId; 2]> = None;
        for (forward_index, forward) in self.by_class.iter().enumerate() {
            let (Some(forward), Some(forward_class)) =
                (*forward, RotationCardinalClass::from_index(forward_index))
            else {
                continue;
            };
            if !forward_class.is_non_identity() {
                continue;
            }
            for (inverse_index, inverse) in inverse.by_class.iter().enumerate() {
                let (Some(inverse), Some(inverse_class)) =
                    (*inverse, RotationCardinalClass::from_index(inverse_index))
                else {
                    continue;
                };
                if !inverse_class.is_non_identity()
                    || forward_class.composes_to_identity(inverse_class)
                {
                    continue;
                }
                let candidate = canonical_constraint_id_pair(forward.id, inverse.id);
                if best.is_none_or(|current| {
                    (
                        candidate[0].canonical_bytes(),
                        candidate[1].canonical_bytes(),
                    ) < (current[0].canonical_bytes(), current[1].canonical_bytes())
                }) {
                    best = Some(candidate);
                }
            }
        }
        best
    }
}

/// Retains the canonical-smallest constraint that uses one exact edge in a
/// residual which rejects a zero-length vector.
fn observe_exact_nondegenerate_edge_use(
    uses: &mut BTreeMap<CanonicalId, ConstraintId>,
    edge: EdgeId,
    constraint: ConstraintId,
) {
    uses.entry(edge.canonical_bytes())
        .and_modify(|current| {
            if constraint.canonical_bytes() < current.canonical_bytes() {
                *current = constraint;
            }
        })
        .or_insert(constraint);
}

#[cfg(test)]
std::thread_local! {
    static FIXED_LENGTH_SUMMARY_VISITS: std::cell::Cell<Option<usize>> =
        const { std::cell::Cell::new(None) };
    static LAST_QUARANTINED_DIRECT_CONFLICTS: std::cell::RefCell<Vec<DirectConstraintConflictV1>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

#[cfg(test)]
fn begin_quarantined_direct_conflict_capture() {
    LAST_QUARANTINED_DIRECT_CONFLICTS.with(|candidates| candidates.borrow_mut().clear());
}

#[cfg(test)]
fn record_quarantined_direct_conflict(candidate: &DirectConstraintConflictV1) {
    LAST_QUARANTINED_DIRECT_CONFLICTS
        .with(|candidates| candidates.borrow_mut().push(candidate.clone()));
}

#[cfg(test)]
fn last_quarantined_direct_conflicts() -> Vec<DirectConstraintConflictV1> {
    LAST_QUARANTINED_DIRECT_CONFLICTS.with(|candidates| candidates.borrow().clone())
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

/// Exhaustively scans the finite set of direct candidate rules.
///
/// With `N` prepared records and `E` validated pattern edges, total time is
/// `O(E log E + N² + K log K)` and storage is `O(E + N + K)`, where `K` is
/// the number of reported conflicts. The edge term builds only the endpoint
/// indices needed by candidate rotation/mirror rules. The quadratic term comes
/// from joining two directed ratio steps while looking up a closing third
/// step; the prepared-record and geometry limits bound both finite scans.
/// Fixed-length assignments are summarized during the single canonical record
/// pass, so reusing one edge from many equal-length constraints never causes a
/// cross-product rescan of fixed-length groups and equal-length pairs.
/// Point-on-line, mirror-axis, and angle-bisector edge references are also
/// indexed once during that pass for exact non-degeneracy witnesses.
/// Candidate relations outside the residual-proven allowlist are returned as
/// solver-required instead of emitted as direct conflicts.
#[must_use]
pub fn preflight_direct_conflicts_v1(set: &GeometricConstraintSetV1<'_>) -> ConstraintPreflightV1 {
    preflight_direct_conflicts_with_zero_closure_controls_v1(
        set,
        bounded_zero_closure::Limits::default(),
        &mut bounded_zero_closure::NoopObserver,
    )
}

struct PublicPreflightObserverAdapter<'a, O>(&'a mut O);

impl<O: GeometricConstraintPreflightObserverV1> bounded_zero_closure::Observer
    for PublicPreflightObserverAdapter<'_, O>
{
    fn checkpoint(
        &mut self,
        _checkpoint: bounded_zero_closure::Checkpoint,
    ) -> bounded_zero_closure::ObserverControl {
        match self.0.checkpoint() {
            GeometricConstraintPreflightObserverControlV1::Continue => {
                bounded_zero_closure::ObserverControl::Continue
            }
            GeometricConstraintPreflightObserverControlV1::Cancelled => {
                bounded_zero_closure::ObserverControl::Cancelled
            }
            GeometricConstraintPreflightObserverControlV1::DeadlineReached => {
                bounded_zero_closure::ObserverControl::DeadlineReached
            }
        }
    }
}

#[must_use]
pub fn preflight_direct_conflicts_with_observer_v1(
    set: &GeometricConstraintSetV1<'_>,
    observer: &mut impl GeometricConstraintPreflightObserverV1,
) -> ConstraintPreflightV1 {
    preflight_direct_conflicts_with_zero_closure_controls_v1(
        set,
        bounded_zero_closure::Limits::default(),
        &mut PublicPreflightObserverAdapter(observer),
    )
}

fn preflight_observer_stop_reason(
    observer: &mut impl bounded_zero_closure::Observer,
    phase: bounded_zero_closure::Phase,
    completed_work: u64,
) -> Option<GeometricConstraintUnknownReasonV1> {
    match observer.checkpoint(bounded_zero_closure::Checkpoint {
        phase,
        completed_work,
        reserved_storage_units: 0,
    }) {
        bounded_zero_closure::ObserverControl::Continue => None,
        bounded_zero_closure::ObserverControl::Cancelled => {
            Some(GeometricConstraintUnknownReasonV1::Cancelled)
        }
        bounded_zero_closure::ObserverControl::DeadlineReached => {
            Some(GeometricConstraintUnknownReasonV1::DeadlineReached)
        }
    }
}

fn preflight_direct_conflicts_with_zero_closure_controls_v1(
    set: &GeometricConstraintSetV1<'_>,
    zero_closure_limits: bounded_zero_closure::Limits,
    zero_closure_observer: &mut impl bounded_zero_closure::Observer,
) -> ConstraintPreflightV1 {
    #[cfg(test)]
    begin_quarantined_direct_conflict_capture();

    if let Some(reason) =
        preflight_observer_stop_reason(zero_closure_observer, bounded_zero_closure::Phase::Start, 0)
    {
        return ConstraintPreflightV1::Unknown {
            reason,
            unchecked_constraint_ids: canonical_constraint_ids(&set.constraints),
        };
    }

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
    let mut cardinal_rotations: BTreeMap<RotationRoleKey, RotationCardinalGroupSummary> =
        BTreeMap::new();
    let mut non_half_turn_rotations: BTreeMap<RotationRoleKey, ScalarAssignment> = BTreeMap::new();
    let mut rotation_roles: BTreeMap<RotationRoleKey, [VertexId; 3]> = BTreeMap::new();
    let mut points_on_lines: BTreeMap<(CanonicalId, CanonicalId), Vec<ConstraintId>> =
        BTreeMap::new();
    let mut mirrors: BTreeMap<MirrorAxisKey, Vec<ConstraintId>> = BTreeMap::new();
    let mut mirror_roles: BTreeMap<CanonicalId, [VertexId; 2]> = BTreeMap::new();
    let mut exact_nondegenerate_edge_uses: BTreeMap<CanonicalId, ConstraintId> = BTreeMap::new();
    let mut unchecked = Vec::new();

    for (record_index, record) in set.constraints.iter().enumerate() {
        if record_index != 0
            && record_index % 128 == 0
            && let Some(reason) = preflight_observer_stop_reason(
                zero_closure_observer,
                bounded_zero_closure::Phase::DirectPreflightScan,
                u64::try_from(record_index).unwrap_or(u64::MAX),
            )
        {
            return ConstraintPreflightV1::Unknown {
                reason,
                unchecked_constraint_ids: canonical_constraint_ids(&set.constraints),
            };
        }
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
            GeometricConstraintKindV1::RotationalSymmetry {
                center_vertex,
                source_vertex,
                target_vertex,
                angle_degrees,
            } => {
                let key = RotationRoleKey::roles(*center_vertex, *source_vertex, *target_vertex);
                let assignment = ScalarAssignment {
                    id: record.id,
                    value: *angle_degrees,
                };
                if let Some(class) = RotationCardinalClass::from_angle_degrees(*angle_degrees) {
                    cardinal_rotations
                        .entry(key)
                        .or_default()
                        .observe(class, assignment);
                }
                if angle_degrees.to_bits() != 180.0_f64.to_bits() {
                    non_half_turn_rotations
                        .entry(key)
                        .and_modify(|current| {
                            if assignment.id.canonical_bytes() < current.id.canonical_bytes() {
                                *current = assignment;
                            }
                        })
                        .or_insert(assignment);
                }
                rotation_roles.entry(key).or_insert([
                    *center_vertex,
                    *source_vertex,
                    *target_vertex,
                ]);
                unchecked.push(record.id);
            }
            GeometricConstraintKindV1::PointOnLine { vertex, line_edge } => {
                observe_exact_nondegenerate_edge_use(
                    &mut exact_nondegenerate_edge_uses,
                    *line_edge,
                    record.id,
                );
                points_on_lines
                    .entry((vertex.canonical_bytes(), line_edge.canonical_bytes()))
                    .or_default()
                    .push(record.id);
                unchecked.push(record.id);
            }
            GeometricConstraintKindV1::MirrorSymmetry {
                first_vertex,
                second_vertex,
                axis_edge,
            } => {
                observe_exact_nondegenerate_edge_use(
                    &mut exact_nondegenerate_edge_uses,
                    *axis_edge,
                    record.id,
                );
                mirrors
                    .entry(MirrorAxisKey::relation(
                        *first_vertex,
                        *second_vertex,
                        *axis_edge,
                    ))
                    .or_default()
                    .push(record.id);
                mirror_roles.insert(record.id.canonical_bytes(), [*first_vertex, *second_vertex]);
                unchecked.push(record.id);
            }
            GeometricConstraintKindV1::AngleBisector {
                first_edge,
                second_edge,
                bisector_edge,
                ..
            } => {
                for edge in [*first_edge, *second_edge, *bisector_edge] {
                    observe_exact_nondegenerate_edge_use(
                        &mut exact_nondegenerate_edge_uses,
                        edge,
                        record.id,
                    );
                }
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
        if let Some(witness) = incompatible_fixed_angle_pair_witness_v1(assignments) {
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
        let denominator_length = fixed_lengths
            .get(denominator)
            .and_then(ScalarGroupSummary::consistent_assignment);
        if let Some(denominator_length) = denominator_length
            && let Some(witness) = incompatible_ratio_pair_with_fixed_denominator_witness_v1(
                assignments,
                denominator_length,
            )
        {
            push_conflict(
                &mut conflicts,
                DirectConstraintConflictKindV1::DifferentLengthRatios {
                    numerator_edge: edge_ids[numerator],
                    denominator_edge: edge_ids[denominator],
                },
                [witness[0].id, witness[1].id, denominator_length.id],
            );
        }
        let Some(ratio) = consistent_scalar_assignment(assignments) else {
            continue;
        };
        let Some(numerator_length) = fixed_lengths
            .get(numerator)
            .and_then(ScalarGroupSummary::consistent_assignment)
        else {
            continue;
        };
        let Some(denominator_length) = denominator_length else {
            continue;
        };
        if numerator_length.value.is_finite()
            && numerator_length.value > 0.0
            && denominator_length.value.is_finite()
            && denominator_length.value > 0.0
            && ratio.value.is_finite()
            && ratio.value > 0.0
            && length_ratio_residual_binary64_v1(
                numerator_length.value,
                ratio.value,
                denominator_length.value,
            ) != 0.0
        {
            push_conflict(
                &mut conflicts,
                DirectConstraintConflictKindV1::LengthRatioWithIncompatibleFixedLengths {
                    numerator_edge: edge_ids[numerator],
                    denominator_edge: edge_ids[denominator],
                },
                [numerator_length.id, denominator_length.id, ratio.id],
            );
        }
    }
    for (edge, horizontal_ids) in &horizontal {
        if let (Some(horizontal_id), Some(vertical_id)) = (
            horizontal_ids.first(),
            vertical.get(edge).and_then(|ids| ids.first()),
        ) {
            let fixed_id = fixed_lengths
                .get(edge)
                .and_then(ScalarGroupSummary::consistent_assignment)
                .map(|assignment| assignment.id);
            let noncollapse_id = fixed_id
                .into_iter()
                .chain(exact_nondegenerate_edge_uses.get(edge).copied())
                .min_by_key(ConstraintId::canonical_bytes);
            if let Some(noncollapse_id) = noncollapse_id {
                push_conflict(
                    &mut conflicts,
                    DirectConstraintConflictKindV1::HorizontalAndVertical {
                        edge: edge_ids[edge],
                    },
                    [*horizontal_id, *vertical_id, noncollapse_id],
                );
            } else {
                unchecked.extend([*horizontal_id, *vertical_id]);
            }
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
    for ((first, second), forward_assignments) in &ratios {
        if first >= second {
            continue;
        }
        let Some(reverse_assignments) = ratios.get(&(*second, *first)) else {
            continue;
        };
        if let Some(witness) = nonreciprocal_length_ratio_fixed_witness_v1(
            *first,
            *second,
            forward_assignments,
            reverse_assignments,
            &fixed_lengths,
        ) {
            push_conflict(
                &mut conflicts,
                DirectConstraintConflictKindV1::NonReciprocalLengthRatiosWithFixedLength {
                    first_edge: edge_ids[first],
                    second_edge: edge_ids[second],
                },
                witness,
            );
        }
    }
    let mut consistent_outgoing: BTreeMap<CanonicalId, Vec<(CanonicalId, ScalarAssignment)>> =
        BTreeMap::new();
    for ((numerator, denominator), assignments) in &ratios {
        if let Some(assignment) = consistent_scalar_assignment(assignments) {
            consistent_outgoing
                .entry(*numerator)
                .or_default()
                .push((*denominator, assignment));
        }
    }
    for (first, first_steps) in &consistent_outgoing {
        for (second, first_ratio) in first_steps {
            let Some(second_steps) = consistent_outgoing.get(second) else {
                continue;
            };
            for (third, second_ratio) in second_steps {
                if third == first || third == second || first >= second || first >= third {
                    continue;
                }
                let Some(third_ratio) = ratios
                    .get(&(*third, *first))
                    .and_then(|items| consistent_scalar_assignment(items))
                else {
                    continue;
                };
                if let Some((fixed_edge, witness)) = length_ratio_cycle_fixed_witness_v1(
                    [*first, *second, *third],
                    [*first_ratio, *second_ratio, third_ratio],
                    &fixed_lengths,
                ) {
                    push_conflict(
                        &mut conflicts,
                        DirectConstraintConflictKindV1::NonUnitLengthRatioCycleWithFixedLength {
                            first_edge: edge_ids[first],
                            second_edge: edge_ids[second],
                            third_edge: edge_ids[third],
                            fixed_edge: edge_ids[&fixed_edge],
                        },
                        witness,
                    );
                }
            }
        }
    }
    for (pair, equal_ids) in &equal_lengths {
        if let Some(witness) =
            equal_length_ratio_fixed_witness_v1(*pair, equal_ids, &fixed_lengths, &ratios)
        {
            push_conflict(
                &mut conflicts,
                DirectConstraintConflictKindV1::EqualLengthWithNonUnitRatioAndFixedLength {
                    first_edge: edge_ids[&pair.first],
                    second_edge: edge_ids[&pair.second],
                },
                witness,
            );
        }
    }
    for (pair, parallel_ids) in &parallels {
        if let (Some(parallel_id), Some(angle_assignment)) = (
            parallel_ids.first(),
            fixed_angles_by_pair.get(pair).and_then(|assignments| {
                assignments
                    .iter()
                    .find(|assignment| fixed_angle_rejects_zero_cross_binary64_v1(assignment.value))
            }),
        ) && let (Some(first_unit), Some(second_unit)) = (
            fixed_lengths
                .get(&pair.first)
                .and_then(ScalarGroupSummary::consistent_assignment)
                .filter(|assignment| assignment.value.to_bits() == 1.0_f64.to_bits()),
            fixed_lengths
                .get(&pair.second)
                .and_then(ScalarGroupSummary::consistent_assignment)
                .filter(|assignment| assignment.value.to_bits() == 1.0_f64.to_bits()),
        ) {
            // The solver computes `cross / (hypot(first) * hypot(second))`.
            // These two exact unit-length residuals make that denominator
            // bit-exactly 1.0, so a zero parallel residual proves the raw
            // cross used by FixedAngle is also exactly zero.  The frozen
            // angle helper then rejects every reachable atan2(+0, dot)
            // class; no stored-angle inequality is used as evidence.
            push_conflict(
                &mut conflicts,
                DirectConstraintConflictKindV1::ParallelWithFixedNonParallelAngle {
                    first_edge: edge_ids[&pair.first],
                    second_edge: edge_ids[&pair.second],
                },
                [
                    *parallel_id,
                    angle_assignment.id,
                    first_unit.id,
                    second_unit.id,
                ],
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
            // Parallel uses a length-normalized cross product in the solver.
            // A collapsed edge makes that residual non-finite rather than
            // satisfying it, while non-collapsed horizontal/vertical vectors
            // have normalized cross magnitude one.
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
            // The same soundness argument applies with the canonical pair's
            // horizontal and vertical roles reversed.
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
    for (pair, angles) in &fixed_angles_by_pair {
        let angle = angles
            .iter()
            .find(|assignment| fixed_angle_rejects_zero_cross_binary64_v1(assignment.value));
        let same_orientation = horizontal
            .get(&pair.first)
            .zip(horizontal.get(&pair.second))
            .or_else(|| vertical.get(&pair.first).zip(vertical.get(&pair.second)));
        if let (Some(angle), Some((first, second))) = (angle, same_orientation)
            && let (Some(first_id), Some(second_id)) = (first.first(), second.first())
        {
            push_conflict(
                &mut conflicts,
                DirectConstraintConflictKindV1::SameOrientationWithFixedNonParallelAngle {
                    first_edge: edge_ids[&pair.first],
                    second_edge: edge_ids[&pair.second],
                },
                [angle.id, *first_id, *second_id],
            );
        }

        let angle = angles
            .iter()
            .find(|assignment| fixed_angle_rejects_perpendicular_binary64_v1(assignment.value));
        let perpendicular_orientation = horizontal
            .get(&pair.first)
            .zip(vertical.get(&pair.second))
            .map(|(horizontal, vertical)| (horizontal, vertical, false))
            .or_else(|| {
                horizontal
                    .get(&pair.second)
                    .zip(vertical.get(&pair.first))
                    .map(|(horizontal, vertical)| (horizontal, vertical, true))
            });
        if let (Some(angle), Some((horizontal, vertical, reversed))) =
            (angle, perpendicular_orientation)
            && let (Some(horizontal_id), Some(vertical_id)) = (horizontal.first(), vertical.first())
        {
            let (horizontal_edge, vertical_edge) = if reversed {
                (edge_ids[&pair.second], edge_ids[&pair.first])
            } else {
                (edge_ids[&pair.first], edge_ids[&pair.second])
            };
            push_conflict(
                &mut conflicts,
                DirectConstraintConflictKindV1::PerpendicularOrientationsWithFixedNonRightAngle {
                    horizontal_edge,
                    vertical_edge,
                },
                [angle.id, *horizontal_id, *vertical_id],
            );
        }
    }

    let has_same_role_rotation_candidate = cardinal_rotations
        .values()
        .any(|summary| summary.different_witness().is_some());
    let has_inverse_role_rotation_candidate = cardinal_rotations.iter().any(|(key, summary)| {
        let inverse_key = key.inverse();
        *key < inverse_key
            && cardinal_rotations
                .get(&inverse_key)
                .and_then(|inverse| summary.nonidentity_inverse_composition_witness(inverse))
                .is_some()
    });
    let has_mirror_axis_candidate = mirrors.keys().any(|key| {
        points_on_lines.contains_key(&(key.first, key.axis))
            || points_on_lines.contains_key(&(key.second, key.axis))
    });
    let has_collinear_rotation_candidate =
        !non_half_turn_rotations.is_empty() && !points_on_lines.is_empty();
    if has_same_role_rotation_candidate
        || has_inverse_role_rotation_candidate
        || has_mirror_axis_candidate
        || has_collinear_rotation_candidate
    {
        let pattern_edges = pattern_edge_index(
            set.source_pattern,
            has_same_role_rotation_candidate
                || has_inverse_role_rotation_candidate
                || has_mirror_axis_candidate
                || has_collinear_rotation_candidate,
            has_collinear_rotation_candidate || has_mirror_axis_candidate,
        );
        if has_collinear_rotation_candidate {
            let point_line_witnesses =
                canonical_point_line_witnesses(&pattern_edges.by_id, &points_on_lines);
            for (key, rotation) in &non_half_turn_rotations {
                let [center_vertex, source_vertex, target_vertex] = rotation_roles[key];
                let Some((point_id, line_edge)) = canonical_collinear_rotation_witness(
                    &point_line_witnesses,
                    center_vertex,
                    source_vertex,
                    target_vertex,
                ) else {
                    continue;
                };
                push_conflict(
                    &mut conflicts,
                    DirectConstraintConflictKindV1::RotationalSymmetryWithCollinearRadius {
                        center_vertex,
                        source_vertex,
                        target_vertex,
                        line_edge,
                    },
                    [rotation.id, point_id],
                );
            }
            for (key, summary) in &cardinal_rotations {
                let Some(rotation) = summary.quarter_turn_witness() else {
                    continue;
                };
                let [center_vertex, source_vertex, target_vertex] = rotation_roles[key];
                let Some((point_id, line_edge)) = point_line_witnesses
                    .get(&(
                        target_vertex.canonical_bytes(),
                        VertexPairKey::unordered(center_vertex, source_vertex),
                    ))
                    .copied()
                else {
                    continue;
                };
                let Some((fixed_radius_id, horizontal_radius_id)) =
                    canonical_unit_horizontal_radius_witness(
                        &pattern_edges.by_pair,
                        &fixed_lengths,
                        &horizontal,
                        center_vertex,
                        source_vertex,
                    )
                else {
                    continue;
                };
                // The frozen 90/270-degree matrix contains only +/-1 and
                // +0. An exact-unit horizontal source radius is therefore
                // mapped to a vertical delta of exact magnitude one, while
                // PointOnLine on that same horizontal radius forces the
                // target's vertical delta to exact zero.
                push_conflict(
                    &mut conflicts,
                    DirectConstraintConflictKindV1::RotationalSymmetryWithCollinearRadius {
                        center_vertex,
                        source_vertex,
                        target_vertex,
                        line_edge,
                    },
                    [rotation.id, point_id, fixed_radius_id, horizontal_radius_id],
                );
            }
        }
        if has_mirror_axis_candidate {
            let mut fixed_separation_witnesses: BTreeMap<
                VertexPairKey,
                Option<(ConstraintId, EdgeId)>,
            > = BTreeMap::new();
            for (key, mirror_ids) in &mirrors {
                let Some(mirror_id) = mirror_ids
                    .iter()
                    .copied()
                    .min_by_key(ConstraintId::canonical_bytes)
                else {
                    continue;
                };
                let Some(point_id) = [key.first, key.second]
                    .into_iter()
                    .filter_map(|vertex| points_on_lines.get(&(vertex, key.axis)))
                    .flatten()
                    .copied()
                    .min_by_key(ConstraintId::canonical_bytes)
                else {
                    continue;
                };
                let pair = VertexPairKey {
                    first: key.first,
                    second: key.second,
                };
                let Some((fixed_id, fixed_separation_edge)) =
                    *fixed_separation_witnesses.entry(pair).or_insert_with(|| {
                        canonical_positive_fixed_edge_witness(
                            &pattern_edges.by_pair,
                            &fixed_lengths,
                            pair,
                        )
                    })
                else {
                    continue;
                };
                push_conflict(
                    &mut conflicts,
                    DirectConstraintConflictKindV1::
                        MirrorSymmetryWithPointOnAxisAndFixedSeparation {
                            first_vertex: vertex_ids[&key.first],
                            second_vertex: vertex_ids[&key.second],
                            axis_edge: edge_ids[&key.axis],
                            fixed_separation_edge,
                        },
                    [mirror_id, point_id, fixed_id],
                );

                let Some(axis_unit) = fixed_lengths
                    .get(&key.axis)
                    .and_then(ScalarGroupSummary::consistent_assignment)
                    .filter(|assignment| assignment.value.to_bits() == 1.0_f64.to_bits())
                else {
                    continue;
                };
                let Some(axis_horizontal_id) = horizontal
                    .get(&key.axis)
                    .into_iter()
                    .flatten()
                    .copied()
                    .min_by_key(ConstraintId::canonical_bytes)
                else {
                    continue;
                };
                let Some((unit_separation_id, unit_separation_edge)) =
                    canonical_unit_fixed_edge_witness(&pattern_edges.by_pair, &fixed_lengths, pair)
                else {
                    continue;
                };
                let Some(axis_start) = pattern_edges.axis_starts.get(&key.axis).copied() else {
                    continue;
                };
                let mut exact_anchor_witness: Option<(
                    ConstraintId,
                    ConstraintId,
                    ConstraintId,
                    ConstraintId,
                )> = None;
                for candidate_mirror_id in mirror_ids {
                    let [point_vertex, _] = mirror_roles[&candidate_mirror_id.canonical_bytes()];
                    let Some(point_id) = points_on_lines
                        .get(&(point_vertex.canonical_bytes(), key.axis))
                        .into_iter()
                        .flatten()
                        .copied()
                        .min_by_key(ConstraintId::canonical_bytes)
                    else {
                        continue;
                    };
                    let Some((connector_horizontal_id, connector_vertical_id)) =
                        canonical_horizontal_vertical_edge_witness(
                            &pattern_edges.by_pair,
                            &horizontal,
                            &vertical,
                            VertexPairKey::unordered(axis_start, point_vertex),
                        )
                    else {
                        continue;
                    };
                    let candidate = (
                        *candidate_mirror_id,
                        point_id,
                        connector_horizontal_id,
                        connector_vertical_id,
                    );
                    if exact_anchor_witness.is_none_or(|current| {
                        (
                            candidate.0.canonical_bytes(),
                            candidate.1.canonical_bytes(),
                            candidate.2.canonical_bytes(),
                            candidate.3.canonical_bytes(),
                        ) < (
                            current.0.canonical_bytes(),
                            current.1.canonical_bytes(),
                            current.2.canonical_bytes(),
                            current.3.canonical_bytes(),
                        )
                    }) {
                        exact_anchor_witness = Some(candidate);
                    }
                }
                let Some((
                    exact_mirror_id,
                    exact_point_id,
                    connector_horizontal_id,
                    connector_vertical_id,
                )) = exact_anchor_witness
                else {
                    continue;
                };
                // Horizontal and Vertical on the real connector force the
                // selected point's two binary64 coordinate deltas from the
                // axis start to zero. The exact-unit horizontal axis then has
                // direction `( +/-1, +0 )`; the mirror projection is exactly
                // zero and the reflected point is the axis start. A distinct
                // endpoint therefore cannot satisfy an exact unit separation.
                push_conflict(
                    &mut conflicts,
                    DirectConstraintConflictKindV1::
                        MirrorSymmetryWithPointOnAxisAndFixedSeparation {
                            first_vertex: vertex_ids[&key.first],
                            second_vertex: vertex_ids[&key.second],
                            axis_edge: edge_ids[&key.axis],
                            fixed_separation_edge: unit_separation_edge,
                        },
                    [
                        exact_mirror_id,
                        exact_point_id,
                        unit_separation_id,
                        axis_unit.id,
                        axis_horizontal_id,
                        connector_horizontal_id,
                        connector_vertical_id,
                    ],
                );
            }
        }
        if has_same_role_rotation_candidate {
            // The frozen residual evaluates
            //   fl(fl(target - center) - R(fl(source - center))).
            // Exact zero therefore makes the shared finite target delta equal
            // to both cardinal transforms. Cardinal coefficients are only
            // +/-1 and +0, so those transforms introduce no rounding for
            // finite deltas; two distinct cardinal matrices have only the
            // zero vector as a common input. The selected positive fixed
            // radius then rules out that forced source/target collapse.
            for (key, summary) in &cardinal_rotations {
                let Some(witness) = summary.different_witness() else {
                    continue;
                };
                let [center_vertex, source_vertex, target_vertex] = rotation_roles[key];
                let Some((fixed_id, fixed_radius_edge)) = canonical_positive_radius_witness(
                    &pattern_edges.by_pair,
                    &fixed_lengths,
                    center_vertex,
                    source_vertex,
                    target_vertex,
                ) else {
                    continue;
                };
                push_conflict(
                    &mut conflicts,
                    DirectConstraintConflictKindV1::
                        DifferentRotationalSymmetryAnglesWithFixedRadius {
                            center_vertex,
                            source_vertex,
                            target_vertex,
                            fixed_radius_edge,
                        },
                    [witness[0], witness[1], fixed_id],
                );
            }
        }
        if has_inverse_role_rotation_candidate {
            // With exact zero production residuals, reversed cardinal roles
            // establish `t = Rf(s)` and `s = Ri(t)` for finite center-relative
            // deltas. The frozen 90/180/270 matrices contain only +/-1 and
            // +0, so the binary64 transforms are exact. A non-identity
            // quarter-turn composition therefore fixes only the zero vector;
            // a consistent positive radius rules that collapse out.
            for (key, summary) in &cardinal_rotations {
                let inverse_key = key.inverse();
                if *key >= inverse_key {
                    continue;
                }
                let Some(witness) = cardinal_rotations
                    .get(&inverse_key)
                    .and_then(|inverse| summary.nonidentity_inverse_composition_witness(inverse))
                else {
                    continue;
                };
                let [center_vertex, source_vertex, target_vertex] = rotation_roles[key];
                let Some((fixed_id, fixed_radius_edge)) = canonical_positive_radius_witness(
                    &pattern_edges.by_pair,
                    &fixed_lengths,
                    center_vertex,
                    source_vertex,
                    target_vertex,
                ) else {
                    continue;
                };
                push_conflict(
                    &mut conflicts,
                    DirectConstraintConflictKindV1::
                        NonComplementaryInverseRotationalSymmetryAnglesWithFixedRadius {
                            center_vertex,
                            source_vertex,
                            target_vertex,
                            fixed_radius_edge,
                        },
                    [witness[0], witness[1], fixed_id],
                );
            }
        }
    }

    quarantine_unproven_direct_conflicts_v1(&mut conflicts, &mut unchecked);

    if conflicts.is_empty() {
        match general_equal_length_graph_conflict_v1(&equal_lengths, &fixed_lengths, &edge_ids) {
            Ok(Some(conflict)) => conflicts.push(conflict),
            Ok(None) => {}
            Err(()) => {
                return ConstraintPreflightV1::Unknown {
                    reason: GeometricConstraintUnknownReasonV1::WorkLimitExceeded,
                    unchecked_constraint_ids: canonical_constraint_ids(&set.constraints),
                };
            }
        }
    }

    if conflicts.is_empty() {
        match bounded_zero_closure::conflict_with_limits_and_observer(
            set,
            &fixed_lengths,
            &horizontal,
            &vertical,
            &equal_lengths,
            &ratios,
            &edge_ids,
            zero_closure_limits,
            zero_closure_observer,
        ) {
            bounded_zero_closure::Outcome::Proven(conflict) => conflicts.push(conflict),
            bounded_zero_closure::Outcome::NoProof => {}
            bounded_zero_closure::Outcome::Unknown { reason, .. } => {
                return ConstraintPreflightV1::Unknown {
                    reason: match reason {
                        bounded_zero_closure::UnknownReason::ConstraintLimitExceeded => {
                            GeometricConstraintUnknownReasonV1::ConstraintLimitExceeded
                        }
                        bounded_zero_closure::UnknownReason::WorkLimitExceeded => {
                            GeometricConstraintUnknownReasonV1::WorkLimitExceeded
                        }
                        bounded_zero_closure::UnknownReason::StorageLimitExceeded => {
                            GeometricConstraintUnknownReasonV1::StorageLimitExceeded
                        }
                        bounded_zero_closure::UnknownReason::Cancelled => {
                            GeometricConstraintUnknownReasonV1::Cancelled
                        }
                        bounded_zero_closure::UnknownReason::DeadlineReached => {
                            GeometricConstraintUnknownReasonV1::DeadlineReached
                        }
                    },
                    unchecked_constraint_ids: canonical_constraint_ids(&set.constraints),
                };
            }
        }
    }

    if conflicts.is_empty() {
        match general_parallel_graph_conflict_v1(
            &parallels,
            &horizontal,
            &vertical,
            &fixed_angles,
            &vertex_ids,
            &edge_ids,
        ) {
            Ok(Some(candidate)) => {
                debug_assert!(!is_proven_direct_conflict_v1(&candidate));
                #[cfg(test)]
                record_quarantined_direct_conflict(&candidate);
                unchecked.extend(candidate.constraint_ids);
            }
            Ok(None) => {}
            Err(()) => {
                return ConstraintPreflightV1::Unknown {
                    reason: GeometricConstraintUnknownReasonV1::WorkLimitExceeded,
                    unchecked_constraint_ids: canonical_constraint_ids(&set.constraints),
                };
            }
        }
    }

    if conflicts.is_empty() {
        match directed_ratio_closure::conflict(
            &ratios,
            &fixed_lengths,
            &edge_ids,
            zero_closure_observer,
        ) {
            directed_ratio_closure::Outcome::Proven(conflict) => conflicts.push(conflict),
            directed_ratio_closure::Outcome::NoProof => {}
            directed_ratio_closure::Outcome::Unknown { reason, .. } => {
                return ConstraintPreflightV1::Unknown {
                    reason: match reason {
                        bounded_zero_closure::UnknownReason::ConstraintLimitExceeded => {
                            GeometricConstraintUnknownReasonV1::ConstraintLimitExceeded
                        }
                        bounded_zero_closure::UnknownReason::WorkLimitExceeded => {
                            GeometricConstraintUnknownReasonV1::WorkLimitExceeded
                        }
                        bounded_zero_closure::UnknownReason::StorageLimitExceeded => {
                            GeometricConstraintUnknownReasonV1::StorageLimitExceeded
                        }
                        bounded_zero_closure::UnknownReason::Cancelled => {
                            GeometricConstraintUnknownReasonV1::Cancelled
                        }
                        bounded_zero_closure::UnknownReason::DeadlineReached => {
                            GeometricConstraintUnknownReasonV1::DeadlineReached
                        }
                    },
                    unchecked_constraint_ids: canonical_constraint_ids(&set.constraints),
                };
            }
        }
    }

    if let Some(reason) = preflight_observer_stop_reason(
        zero_closure_observer,
        bounded_zero_closure::Phase::Complete,
        u64::try_from(set.constraints.len()).unwrap_or(u64::MAX),
    ) {
        return ConstraintPreflightV1::Unknown {
            reason,
            unchecked_constraint_ids: canonical_constraint_ids(&set.constraints),
        };
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

fn consistent_scalar_assignment(assignments: &[ScalarAssignment]) -> Option<ScalarAssignment> {
    let first = assignments.first()?;
    assignments
        .iter()
        .all(|assignment| assignment.value.to_bits() == first.value.to_bits())
        .then(|| {
            *assignments
                .iter()
                .min_by_key(|assignment| assignment.id.canonical_bytes())
                .expect("non-empty assignments have a minimum")
        })
}

/// Returns the canonical three-record proof that one exact equal-length join
/// makes a ratio residual impossible at a consistent fixed binary64 length.
///
/// A zero fixed-length residual forces the named edge length to `fixed.value`;
/// the equal-length residual then forces the other edge to that same binary64
/// value. Both ratio orientations consequently use the same forced numerator
/// and denominator. Stored inequality from one is not evidence: only the
/// production multiplication-then-subtraction result is.
fn equal_length_ratio_fixed_witness_v1(
    pair: EdgePairKey,
    equal_ids: &[ConstraintId],
    fixed_lengths: &BTreeMap<CanonicalId, ScalarGroupSummary>,
    ratios: &BTreeMap<(CanonicalId, CanonicalId), Vec<ScalarAssignment>>,
) -> Option<[ConstraintId; 3]> {
    let equal_id = equal_ids
        .iter()
        .copied()
        .min_by_key(ConstraintId::canonical_bytes)?;
    let fixed = [pair.first, pair.second]
        .into_iter()
        .filter_map(|edge| {
            fixed_lengths
                .get(&edge)
                .and_then(ScalarGroupSummary::consistent_assignment)
        })
        .filter(|assignment| assignment.value.is_finite() && assignment.value > 0.0);
    let ratio = ratios
        .get(&(pair.first, pair.second))
        .into_iter()
        .chain(ratios.get(&(pair.second, pair.first)))
        .flatten()
        .copied()
        .filter(|assignment| assignment.value.is_finite() && assignment.value > 0.0);

    fixed
        .flat_map(|fixed| {
            ratio
                .clone()
                .filter(move |ratio| {
                    length_ratio_residual_binary64_v1(fixed.value, ratio.value, fixed.value) != 0.0
                })
                .map(move |ratio| {
                    let mut ids = [equal_id, fixed.id, ratio.id];
                    ids.sort_unstable_by_key(ConstraintId::canonical_bytes);
                    ids
                })
        })
        .min_by(|left, right| canonical_id_slice_cmp(left, right))
}

/// Returns the canonical three-record binary64 closure proof for two opposing
/// ratio residuals anchored by one exact fixed length.
///
/// When the canonical first edge is fixed, a zero reverse residual forces the
/// second length to the once-rounded `reverse * first` product; the forward
/// residual is then evaluated at exactly that value. The symmetric derivation
/// is used when the second edge is fixed. This deliberately makes no inference
/// from the exact-real product of the two stored ratios.
fn nonreciprocal_length_ratio_fixed_witness_v1(
    first: CanonicalId,
    second: CanonicalId,
    forward_assignments: &[ScalarAssignment],
    reverse_assignments: &[ScalarAssignment],
    fixed_lengths: &BTreeMap<CanonicalId, ScalarGroupSummary>,
) -> Option<[ConstraintId; 3]> {
    let forward = consistent_scalar_assignment(forward_assignments)?;
    let reverse = consistent_scalar_assignment(reverse_assignments)?;
    if !forward.value.is_finite()
        || forward.value <= 0.0
        || !reverse.value.is_finite()
        || reverse.value <= 0.0
    {
        return None;
    }

    let mut best: Option<[ConstraintId; 3]> = None;
    for (fixed_edge, fixed_is_first) in [(first, true), (second, false)] {
        let Some(fixed) = fixed_lengths
            .get(&fixed_edge)
            .and_then(ScalarGroupSummary::consistent_assignment)
        else {
            continue;
        };
        if !fixed.value.is_finite() || fixed.value <= 0.0 {
            continue;
        }

        let closure_residual = if fixed_is_first {
            let second_length =
                length_ratio_scaled_denominator_binary64_v1(reverse.value, fixed.value);
            length_ratio_residual_binary64_v1(fixed.value, forward.value, second_length)
        } else {
            let first_length =
                length_ratio_scaled_denominator_binary64_v1(forward.value, fixed.value);
            length_ratio_residual_binary64_v1(fixed.value, reverse.value, first_length)
        };
        if closure_residual == 0.0 {
            continue;
        }

        let mut ids = [forward.id, reverse.id, fixed.id];
        ids.sort_unstable_by_key(ConstraintId::canonical_bytes);
        if best
            .as_ref()
            .is_none_or(|current| canonical_id_slice_cmp(&ids, current).is_lt())
        {
            best = Some(ids);
        }
    }
    best
}

/// Evaluates a directed three-ratio closure from one fixed edge.
///
/// Ratios are ordered as `A/B`, `B/C`, `C/A`. Starting at the fixed edge, the
/// two incoming residuals each force one once-rounded numerator length; the
/// remaining ratio is evaluated through the shared production residual.
fn length_ratio_cycle_closure_residual_binary64_v1(
    fixed_index: usize,
    fixed_length: f64,
    ratios: [f64; 3],
) -> f64 {
    debug_assert!(fixed_index < 3);
    let first_derived =
        length_ratio_scaled_denominator_binary64_v1(ratios[(fixed_index + 2) % 3], fixed_length);
    let second_derived =
        length_ratio_scaled_denominator_binary64_v1(ratios[(fixed_index + 1) % 3], first_derived);
    length_ratio_residual_binary64_v1(fixed_length, ratios[fixed_index], second_derived)
}

/// Returns the canonical four-record proof for a directed three-edge ratio
/// cycle whose production binary64 closure cannot be zero.
fn length_ratio_cycle_fixed_witness_v1(
    edges: [CanonicalId; 3],
    ratios: [ScalarAssignment; 3],
    fixed_lengths: &BTreeMap<CanonicalId, ScalarGroupSummary>,
) -> Option<(CanonicalId, [ConstraintId; 4])> {
    if ratios
        .iter()
        .any(|ratio| !ratio.value.is_finite() || ratio.value <= 0.0)
    {
        return None;
    }
    let ratio_values = ratios.map(|ratio| ratio.value);
    let mut best: Option<(CanonicalId, [ConstraintId; 4])> = None;
    for (fixed_index, fixed_edge) in edges.into_iter().enumerate() {
        let Some(fixed) = fixed_lengths
            .get(&fixed_edge)
            .and_then(ScalarGroupSummary::consistent_assignment)
        else {
            continue;
        };
        if !fixed.value.is_finite() || fixed.value <= 0.0 {
            continue;
        }
        if length_ratio_cycle_closure_residual_binary64_v1(fixed_index, fixed.value, ratio_values)
            == 0.0
        {
            continue;
        }

        let mut ids = [ratios[0].id, ratios[1].id, ratios[2].id, fixed.id];
        ids.sort_unstable_by_key(ConstraintId::canonical_bytes);
        if best.as_ref().is_none_or(|(current_edge, current_ids)| {
            canonical_id_slice_cmp(&ids, current_ids)
                .then_with(|| fixed_edge.cmp(current_edge))
                .is_lt()
        }) {
            best = Some((fixed_edge, ids));
        }
    }
    best
}

/// Returns the canonical ratio pair whose implemented denominator products
/// cannot both equal one numerator length.
///
/// A positive finite fixed denominator makes every zero fixed-length residual
/// use exactly `denominator.value`. For two ratio residuals to both be zero,
/// their separately rounded products must therefore be the same finite
/// binary64 value. Equal finite products (including a shared underflow to zero)
/// deliberately remain solver-required.
fn incompatible_ratio_pair_with_fixed_denominator_witness_v1(
    assignments: &[ScalarAssignment],
    denominator: ScalarAssignment,
) -> Option<[ScalarAssignment; 2]> {
    if !denominator.value.is_finite() || denominator.value <= 0.0 {
        return None;
    }

    let first = *assignments
        .iter()
        .filter(|assignment| assignment.value.is_finite() && assignment.value > 0.0)
        .min_by_key(|assignment| assignment.id.canonical_bytes())?;
    let first_product = length_ratio_scaled_denominator_binary64_v1(first.value, denominator.value);
    let second = *assignments
        .iter()
        .filter(|assignment| {
            assignment.value.is_finite()
                && assignment.value > 0.0
                && assignment.value.to_bits() != first.value.to_bits()
        })
        .filter(|assignment| {
            let second_product =
                length_ratio_scaled_denominator_binary64_v1(assignment.value, denominator.value);
            !first_product.is_finite()
                || !second_product.is_finite()
                || first_product != second_product
        })
        .min_by_key(|assignment| assignment.id.canonical_bytes())?;

    // If any incompatible pair exists, the canonical-smallest valid
    // assignment participates in one: otherwise every bit-distinct ratio has
    // the same finite product as `first`, so no two products are incompatible.
    let mut witness = [first, second];
    witness.sort_unstable_by_key(|assignment| assignment.id.canonical_bytes());
    debug_assert!({
        let first_product =
            length_ratio_scaled_denominator_binary64_v1(witness[0].value, denominator.value);
        let second_product =
            length_ratio_scaled_denominator_binary64_v1(witness[1].value, denominator.value);
        !first_product.is_finite() || !second_product.is_finite() || first_product != second_product
    });
    Some(witness)
}

fn length_ratio_scaled_denominator_binary64_v1(ratio: f64, denominator_length: f64) -> f64 {
    ratio * denominator_length
}

/// Evaluates the V1 length-ratio residual in its canonical binary64 operation
/// order.
///
/// Both preflight proofs and the numerical solver call this function so a
/// direct contradiction can never be inferred from exact-real multiplication
/// while the implemented rounded residual is zero.
pub(crate) fn length_ratio_residual_binary64_v1(
    numerator_length: f64,
    ratio: f64,
    denominator_length: f64,
) -> f64 {
    let scaled_denominator = length_ratio_scaled_denominator_binary64_v1(ratio, denominator_length);
    numerator_length - scaled_denominator
}

/// Evaluates the V1 fixed-angle residual in the exact operation order used by
/// the numerical solver preview.
///
/// The expected angle deliberately retains the original standard-library
/// degree conversion. This helper is not proof authority.
pub(crate) fn fixed_angle_residual_binary64_v1(actual_radians: f64, angle_degrees: f64) -> f64 {
    fixed_angle_residual_from_expected_radians_binary64_v1(
        actual_radians,
        angle_degrees.to_radians(),
    )
}

/// Evaluates the fixed-angle residual under the frozen V1 proof model.
///
/// Direct proofs and exact certificates must use this helper rather than
/// inheriting the platform preview's expected-angle conversion.
pub(crate) fn deterministic_fixed_angle_residual_binary64_v1(
    actual_radians: f64,
    angle_degrees: f64,
) -> f64 {
    let Ok(expected_radians) = deterministic_degrees_to_radians_v1(angle_degrees) else {
        return f64::NAN;
    };
    fixed_angle_residual_from_expected_radians_binary64_v1(actual_radians, expected_radians)
}

fn fixed_angle_residual_from_expected_radians_binary64_v1(
    actual_radians: f64,
    expected_radians: f64,
) -> f64 {
    let difference = actual_radians - expected_radians;
    (difference + std::f64::consts::PI).rem_euclid(2.0 * std::f64::consts::PI)
        - std::f64::consts::PI
}

/// Returns whether the shared production residual rejects every result class
/// reachable when the production absolute cross term is exactly `+0.0`.
///
/// `atan2(+0, dot)` depends on the dot product's sign and signed-zero class.
/// Finite same-orientation vectors cover positive, negative, `+0.0`, and
/// `-0.0`; overflow can additionally produce either infinity or NaN. The
/// representative inputs below are evaluated through the same frozen
/// deterministic `atan2` and fixed-angle helper as the exact certificate
/// instead of assuming stored degree inequality or a hard-coded pi bit
/// pattern.
fn fixed_angle_rejects_zero_cross_binary64_v1(angle_degrees: f64) -> bool {
    debug_assert!(angle_degrees.is_finite() && (0.0..=180.0).contains(&angle_degrees));
    [
        0.0,
        -0.0,
        1.0,
        -1.0,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NAN,
    ]
    .into_iter()
    .all(|dot| match deterministic_atan2_v1(0.0, dot) {
        Ok(actual) => {
            let residual = deterministic_fixed_angle_residual_binary64_v1(actual, angle_degrees);
            !residual.is_finite() || residual != 0.0
        }
        Err(_) => true,
    })
}

/// Returns whether the shared production residual rejects every result class
/// reachable from one exact horizontal and one exact vertical residual.
///
/// Their production dot is signed zero or NaN. Their absolute cross can be
/// positive finite, `+0.0` after collapse or product underflow, positive
/// infinity after product overflow, or NaN after a non-finite intermediate.
/// Representative binary64 operands intentionally go through the frozen
/// deterministic `atan2`, which preserves signed-zero inputs and therefore the
/// `atan2(+0, -0) == pi` branch-cut class shared with platform preview. No
/// stored-degree comparison or epsilon stands in for those calls.
fn fixed_angle_rejects_perpendicular_binary64_v1(angle_degrees: f64) -> bool {
    debug_assert!(angle_degrees.is_finite() && (0.0..=180.0).contains(&angle_degrees));
    [
        (0.0, 0.0),
        (0.0, -0.0),
        (f64::from_bits(1), 0.0),
        (f64::from_bits(1), -0.0),
        (1.0, 0.0),
        (1.0, -0.0),
        (f64::MAX, 0.0),
        (f64::MAX, -0.0),
        (f64::INFINITY, 0.0),
        (f64::INFINITY, -0.0),
        (f64::NAN, 0.0),
        (1.0, f64::NAN),
    ]
    .into_iter()
    .all(
        |(absolute_cross, dot)| match deterministic_atan2_v1(absolute_cross, dot) {
            Ok(actual) => {
                let residual =
                    deterministic_fixed_angle_residual_binary64_v1(actual, angle_degrees);
                !residual.is_finite() || residual != 0.0
            }
            Err(_) => true,
        },
    )
}

fn general_equal_length_graph_conflict_v1(
    equal_lengths: &BTreeMap<EdgePairKey, Vec<ConstraintId>>,
    fixed_lengths: &BTreeMap<CanonicalId, ScalarGroupSummary>,
    edge_ids: &BTreeMap<CanonicalId, EdgeId>,
) -> Result<Option<DirectConstraintConflictV1>, ()> {
    let mut graph: BTreeMap<CanonicalId, Vec<(CanonicalId, ConstraintId)>> = BTreeMap::new();
    for (pair, ids) in equal_lengths {
        let Some(id) = ids.iter().min_by_key(|id| id.canonical_bytes()) else {
            continue;
        };
        graph
            .entry(pair.first)
            .or_default()
            .push((pair.second, *id));
        graph
            .entry(pair.second)
            .or_default()
            .push((pair.first, *id));
    }
    for arcs in graph.values_mut() {
        arcs.sort_unstable_by_key(|(neighbor, id)| (*neighbor, id.canonical_bytes()));
    }

    #[cfg(test)]
    let max_work = GENERAL_EQUAL_TEST_WORK_LIMIT
        .with(|limit| limit.get().unwrap_or(MAX_GENERAL_EQUAL_GRAPH_WORK_V1));
    #[cfg(not(test))]
    let max_work = MAX_GENERAL_EQUAL_GRAPH_WORK_V1;
    let mut work = 0_u64;
    #[cfg(test)]
    GENERAL_EQUAL_TEST_WORK_OBSERVED.with(|observed| observed.set(0));
    let mut owners: BTreeMap<CanonicalId, (CanonicalId, ScalarAssignment)> = BTreeMap::new();
    let mut parents: BTreeMap<CanonicalId, (CanonicalId, ConstraintId)> = BTreeMap::new();
    let mut sources = graph
        .keys()
        .filter_map(|edge| {
            fixed_lengths
                .get(edge)
                .and_then(ScalarGroupSummary::consistent_assignment)
                .map(|assignment| (*edge, assignment))
        })
        .collect::<Vec<_>>();
    sources.sort_unstable_by_key(|(edge, assignment)| (assignment.id.canonical_bytes(), *edge));
    let mut queue = VecDeque::new();
    for (edge, assignment) in sources {
        owners.insert(edge, (edge, assignment));
        queue.push_back(edge);
    }
    let mut best: Option<(Vec<ConstraintId>, CanonicalId, CanonicalId, usize)> = None;
    let mut oversized = false;
    while let Some(node) = queue.pop_front() {
        let owner = owners.get(&node).copied().ok_or(())?;
        for (neighbor, edge_constraint_id) in graph.get(&node).into_iter().flatten() {
            charge_general_equal_work_v1(&mut work, max_work, 1)?;
            let Some(neighbor_owner) = owners.get(neighbor).copied() else {
                owners.insert(*neighbor, owner);
                parents.insert(*neighbor, (node, *edge_constraint_id));
                queue.push_back(*neighbor);
                continue;
            };
            if owner.0 == neighbor_owner.0
                || owner.1.value.to_bits() == neighbor_owner.1.value.to_bits()
            {
                continue;
            }
            let mut ids = Vec::new();
            let mut cursor = node;
            while let Some((parent, id)) = parents.get(&cursor) {
                charge_general_equal_work_v1(&mut work, max_work, 1)?;
                ids.push(*id);
                if ids.len() > MAX_DIRECT_CONFLICT_CAUSE_IDS_V1 - 2 {
                    oversized = true;
                    break;
                }
                cursor = *parent;
            }
            if ids.len() > MAX_DIRECT_CONFLICT_CAUSE_IDS_V1 - 2 {
                continue;
            }
            cursor = *neighbor;
            while let Some((parent, id)) = parents.get(&cursor) {
                charge_general_equal_work_v1(&mut work, max_work, 1)?;
                ids.push(*id);
                if ids.len() > MAX_DIRECT_CONFLICT_CAUSE_IDS_V1 - 2 {
                    oversized = true;
                    break;
                }
                cursor = *parent;
            }
            if ids.len() > MAX_DIRECT_CONFLICT_CAUSE_IDS_V1 - 2 {
                continue;
            }
            charge_general_equal_work_v1(&mut work, max_work, 1)?;
            ids.push(*edge_constraint_id);
            if ids.len() > MAX_DIRECT_CONFLICT_CAUSE_IDS_V1 - 2 {
                oversized = true;
                continue;
            }
            let equal_constraint_count = ids.len();
            let (first_edge, first, second_edge, second) =
                if owner.1.id.canonical_bytes() < neighbor_owner.1.id.canonical_bytes() {
                    (owner.0, owner.1, neighbor_owner.0, neighbor_owner.1)
                } else {
                    (neighbor_owner.0, neighbor_owner.1, owner.0, owner.1)
                };
            ids.extend([first.id, second.id]);
            let sort_factor = usize::BITS - ids.len().leading_zeros();
            let sort_work = u64::try_from(ids.len())
                .ok()
                .and_then(|length| length.checked_mul(u64::from(sort_factor) + 1))
                .ok_or(())?;
            charge_general_equal_work_v1(&mut work, max_work, sort_work)?;
            canonicalize_constraint_ids(&mut ids);
            if ids.len() > MAX_DIRECT_CONFLICT_CAUSE_IDS_V1 {
                oversized = true;
                continue;
            }
            if let Some(current) = &best {
                charge_general_equal_work_v1(
                    &mut work,
                    max_work,
                    u64::try_from(ids.len().min(current.0.len())).map_err(|_| ())?,
                )?;
            }
            if best.as_ref().is_none_or(|current| {
                ids.len() < current.0.len()
                    || (ids.len() == current.0.len()
                        && canonical_id_slice_cmp(&ids, &current.0).is_lt())
            }) {
                best = Some((ids, first_edge, second_edge, equal_constraint_count));
            }
        }
    }
    let Some((constraint_ids, first_edge, second_edge, equal_constraint_count)) = best else {
        return if oversized { Err(()) } else { Ok(None) };
    };
    Ok(Some(DirectConstraintConflictV1 {
        conflict: DirectConstraintConflictKindV1::DifferentFixedLengthsInEqualLengthComponent {
            first_edge: edge_ids[&first_edge],
            second_edge: edge_ids[&second_edge],
            equal_constraint_count: u16::try_from(equal_constraint_count).map_err(|_| ())?,
        },
        constraint_ids,
    }))
}

fn charge_general_equal_work_v1(work: &mut u64, max_work: u64, amount: u64) -> Result<(), ()> {
    *work = work.checked_add(amount).ok_or(())?;
    #[cfg(test)]
    GENERAL_EQUAL_TEST_WORK_OBSERVED.with(|observed| observed.set(*work));
    (*work <= max_work).then_some(()).ok_or(())
}

fn general_parallel_graph_conflict_v1(
    parallels: &BTreeMap<EdgePairKey, Vec<ConstraintId>>,
    horizontal: &BTreeMap<CanonicalId, Vec<ConstraintId>>,
    vertical: &BTreeMap<CanonicalId, Vec<ConstraintId>>,
    fixed_angles: &BTreeMap<AngleKey, Vec<ScalarAssignment>>,
    vertex_ids: &BTreeMap<CanonicalId, VertexId>,
    edge_ids: &BTreeMap<CanonicalId, EdgeId>,
) -> Result<Option<DirectConstraintConflictV1>, ()> {
    #[cfg(test)]
    let max_work = GENERAL_PARALLEL_TEST_WORK_LIMIT
        .with(|limit| limit.get().unwrap_or(MAX_GENERAL_PARALLEL_GRAPH_WORK_V1));
    #[cfg(not(test))]
    let max_work = MAX_GENERAL_PARALLEL_GRAPH_WORK_V1;
    #[cfg(test)]
    GENERAL_PARALLEL_TEST_WORK_OBSERVED.with(|observed| observed.set(0));
    let mut work = 0_u64;
    let mut graph: BTreeMap<CanonicalId, Vec<(CanonicalId, ConstraintId)>> = BTreeMap::new();
    for (pair, ids) in parallels {
        charge_general_parallel_work_v1(&mut work, max_work, 1)?;
        let Some(id) = ids.iter().min_by_key(|id| id.canonical_bytes()) else {
            continue;
        };
        graph
            .entry(pair.first)
            .or_default()
            .push((pair.second, *id));
        graph
            .entry(pair.second)
            .or_default()
            .push((pair.first, *id));
    }
    for arcs in graph.values_mut() {
        let factor = usize::BITS - arcs.len().max(1).leading_zeros();
        charge_general_parallel_work_v1(
            &mut work,
            max_work,
            u64::try_from(arcs.len()).map_err(|_| ())? * (u64::from(factor) + 1),
        )?;
        arcs.sort_unstable_by_key(|(neighbor, id)| (*neighbor, id.canonical_bytes()));
    }
    let mut best: Option<(Vec<ConstraintId>, CanonicalId, CanonicalId, usize)> = None;
    for node in graph.keys() {
        let label_work = horizontal.get(node).map_or(0, Vec::len)
            + vertical.get(node).map_or(0, Vec::len)
            + graph.get(node).map_or(0, Vec::len);
        charge_general_parallel_work_v1(
            &mut work,
            max_work,
            1 + u64::try_from(label_work).map_err(|_| ())?,
        )?;
        let Some(horizontal_id) = horizontal
            .get(node)
            .and_then(|ids| ids.iter().min_by_key(|id| id.canonical_bytes()))
        else {
            continue;
        };
        let Some(vertical_id) = vertical
            .get(node)
            .and_then(|ids| ids.iter().min_by_key(|id| id.canonical_bytes()))
        else {
            continue;
        };
        let Some(parallel_id) = graph
            .get(node)
            .into_iter()
            .flatten()
            .map(|(_, id)| id)
            .min_by_key(|id| id.canonical_bytes())
        else {
            continue;
        };
        let mut ids = vec![*horizontal_id, *vertical_id, *parallel_id];
        charge_general_parallel_work_v1(&mut work, max_work, 3 * 3)?;
        canonicalize_constraint_ids(&mut ids);
        if let Some(current) = &best {
            charge_general_parallel_work_v1(
                &mut work,
                max_work,
                u64::try_from(ids.len().min(current.0.len())).map_err(|_| ())?,
            )?;
        }
        if best
            .as_ref()
            .is_none_or(|current| canonical_id_slice_cmp(&ids, &current.0).is_lt())
        {
            best = Some((ids, *node, *node, 1));
        }
    }

    let mut sources = Vec::new();
    for node in graph.keys() {
        charge_general_parallel_work_v1(
            &mut work,
            max_work,
            u64::try_from(
                horizontal.get(node).map_or(0, Vec::len) + vertical.get(node).map_or(0, Vec::len),
            )
            .map_err(|_| ())?,
        )?;
        if let Some(id) = horizontal
            .get(node)
            .and_then(|ids| ids.iter().min_by_key(|id| id.canonical_bytes()))
        {
            sources.push((*node, *id, true));
        }
        if let Some(id) = vertical
            .get(node)
            .and_then(|ids| ids.iter().min_by_key(|id| id.canonical_bytes()))
        {
            sources.push((*node, *id, false));
        }
    }
    sources
        .sort_unstable_by_key(|(node, id, horizontal)| (id.canonical_bytes(), *node, *horizontal));
    let source_factor = usize::BITS - sources.len().max(1).leading_zeros();
    charge_general_parallel_work_v1(
        &mut work,
        max_work,
        u64::try_from(sources.len()).map_err(|_| ())? * (u64::from(source_factor) + 1),
    )?;
    let mut owners: BTreeMap<CanonicalId, (CanonicalId, ConstraintId, bool)> = BTreeMap::new();
    let mut parents: BTreeMap<CanonicalId, (CanonicalId, ConstraintId)> = BTreeMap::new();
    let mut queue = VecDeque::new();
    for source in sources {
        charge_general_parallel_work_v1(&mut work, max_work, 1)?;
        if let std::collections::btree_map::Entry::Vacant(entry) = owners.entry(source.0) {
            entry.insert(source);
            queue.push_back(source.0);
        }
    }
    let mut oversized = false;
    while let Some(node) = queue.pop_front() {
        let owner = owners.get(&node).copied().ok_or(())?;
        for (neighbor, parallel_id) in graph.get(&node).into_iter().flatten() {
            charge_general_parallel_work_v1(&mut work, max_work, 1)?;
            let Some(neighbor_owner) = owners.get(neighbor).copied() else {
                owners.insert(*neighbor, owner);
                parents.insert(*neighbor, (node, *parallel_id));
                queue.push_back(*neighbor);
                charge_general_parallel_work_v1(&mut work, max_work, 3)?;
                continue;
            };
            if owner.2 == neighbor_owner.2 || owner.0 == neighbor_owner.0 {
                continue;
            }
            let mut ids = Vec::new();
            for mut cursor in [node, *neighbor] {
                while let Some((parent, id)) = parents.get(&cursor) {
                    charge_general_parallel_work_v1(&mut work, max_work, 1)?;
                    ids.push(*id);
                    if ids.len() > MAX_DIRECT_CONFLICT_CAUSE_IDS_V1 - 2 {
                        oversized = true;
                        break;
                    }
                    cursor = *parent;
                }
                if ids.len() > MAX_DIRECT_CONFLICT_CAUSE_IDS_V1 - 2 {
                    break;
                }
            }
            if ids.len() > MAX_DIRECT_CONFLICT_CAUSE_IDS_V1 - 2 {
                continue;
            }
            ids.push(*parallel_id);
            charge_general_parallel_work_v1(&mut work, max_work, 1)?;
            if ids.len() > MAX_DIRECT_CONFLICT_CAUSE_IDS_V1 - 2 {
                oversized = true;
                continue;
            }
            let parallel_constraint_count = ids.len();
            let (horizontal_owner, vertical_owner) = if owner.2 {
                (owner, neighbor_owner)
            } else {
                (neighbor_owner, owner)
            };
            ids.extend([horizontal_owner.1, vertical_owner.1]);
            let factor = usize::BITS - ids.len().leading_zeros();
            charge_general_parallel_work_v1(
                &mut work,
                max_work,
                u64::try_from(ids.len()).map_err(|_| ())? * (u64::from(factor) + 1),
            )?;
            canonicalize_constraint_ids(&mut ids);
            if let Some(current) = &best {
                charge_general_parallel_work_v1(
                    &mut work,
                    max_work,
                    u64::try_from(ids.len().min(current.0.len())).map_err(|_| ())?,
                )?;
            }
            if best.as_ref().is_none_or(|current| {
                ids.len() < current.0.len()
                    || (ids.len() == current.0.len()
                        && canonical_id_slice_cmp(&ids, &current.0).is_lt())
            }) {
                best = Some((
                    ids,
                    horizontal_owner.0,
                    vertical_owner.0,
                    parallel_constraint_count,
                ));
            }
        }
    }
    let mut result = best
        .map(
            |(constraint_ids, horizontal_edge, vertical_edge, parallel_constraint_count)| {
                Ok(DirectConstraintConflictV1 {
                conflict:
                    DirectConstraintConflictKindV1::PerpendicularOrientationsInParallelComponent {
                        horizontal_edge: edge_ids[&horizontal_edge],
                        vertical_edge: edge_ids[&vertical_edge],
                        parallel_constraint_count: u16::try_from(parallel_constraint_count)
                            .map_err(|_| ())?,
                    },
                constraint_ids,
            })
            },
        )
        .transpose()?;

    for (key, assignments) in fixed_angles {
        charge_general_parallel_work_v1(
            &mut work,
            max_work,
            u64::try_from(assignments.len()).map_err(|_| ())?,
        )?;
        let Some(angle) = consistent_scalar_assignment(assignments) else {
            continue;
        };
        if angle.value == 0.0 || angle.value == 180.0 {
            continue;
        }
        let (path, path_oversized) = canonical_shortest_parallel_path_v1(
            key.edges.first,
            key.edges.second,
            &graph,
            &mut work,
            max_work,
        )?;
        oversized |= path_oversized;
        let Some(mut ids) = path else {
            continue;
        };
        if ids.is_empty() {
            continue;
        }
        let parallel_constraint_count = ids.len();
        ids.push(angle.id);
        if ids.len() > MAX_DIRECT_CONFLICT_CAUSE_IDS_V1 {
            oversized = true;
            continue;
        }
        let factor = usize::BITS - ids.len().leading_zeros();
        charge_general_parallel_work_v1(
            &mut work,
            max_work,
            u64::try_from(ids.len()).map_err(|_| ())? * (u64::from(factor) + 1),
        )?;
        canonicalize_constraint_ids(&mut ids);
        if let Some(current) = &result {
            charge_general_parallel_work_v1(
                &mut work,
                max_work,
                u64::try_from(ids.len().min(current.constraint_ids.len())).map_err(|_| ())?,
            )?;
        }
        if result.as_ref().is_none_or(|current| {
            ids.len() < current.constraint_ids.len()
                || (ids.len() == current.constraint_ids.len()
                    && canonical_id_slice_cmp(&ids, &current.constraint_ids).is_lt())
        }) {
            result = Some(DirectConstraintConflictV1 {
                conflict:
                    DirectConstraintConflictKindV1::NonParallelFixedAngleInParallelComponent {
                        vertex: vertex_ids[&key.vertex],
                        first_edge: edge_ids[&key.edges.first],
                        second_edge: edge_ids[&key.edges.second],
                        parallel_constraint_count: u16::try_from(parallel_constraint_count)
                            .map_err(|_| ())?,
                    },
                constraint_ids: ids,
            });
        }
    }
    if result.is_none() && oversized {
        Err(())
    } else {
        Ok(result)
    }
}

fn canonical_shortest_parallel_path_v1(
    start: CanonicalId,
    target: CanonicalId,
    graph: &BTreeMap<CanonicalId, Vec<(CanonicalId, ConstraintId)>>,
    work: &mut u64,
    max_work: u64,
) -> Result<(Option<Vec<ConstraintId>>, bool), ()> {
    if start == target {
        return Ok((None, false));
    }
    let mut distances = BTreeMap::from([(start, 0_usize)]);
    let mut queue = VecDeque::from([start]);
    while let Some(node) = queue.pop_front() {
        charge_general_parallel_work_v1(work, max_work, 1)?;
        let distance = distances[&node];
        for (neighbor, _) in graph.get(&node).into_iter().flatten() {
            charge_general_parallel_work_v1(work, max_work, 1)?;
            if let std::collections::btree_map::Entry::Vacant(entry) = distances.entry(*neighbor) {
                entry.insert(distance + 1);
                queue.push_back(*neighbor);
                charge_general_parallel_work_v1(work, max_work, 2)?;
            }
        }
    }
    let Some(&target_distance) = distances.get(&target) else {
        return Ok((None, false));
    };
    if target_distance > MAX_DIRECT_CONFLICT_CAUSE_IDS_V1 - 1 {
        return Ok((None, true));
    }
    #[derive(Clone)]
    struct PathLink {
        id: ConstraintId,
        previous: Option<std::rc::Rc<PathLink>>,
    }
    let mut queue = VecDeque::from([(start, None::<std::rc::Rc<PathLink>>)]);
    let mut best: Option<Vec<ConstraintId>> = None;
    while let Some((node, path)) = queue.pop_front() {
        charge_general_parallel_work_v1(work, max_work, 1)?;
        let distance = distances[&node];
        for (neighbor, id) in graph.get(&node).into_iter().flatten() {
            charge_general_parallel_work_v1(work, max_work, 1)?;
            if distances.get(neighbor) != Some(&(distance + 1)) {
                continue;
            }
            let next_path = std::rc::Rc::new(PathLink {
                id: *id,
                previous: path.clone(),
            });
            charge_general_parallel_work_v1(work, max_work, 2)?;
            if *neighbor == target {
                let mut ids = Vec::with_capacity(target_distance);
                let mut cursor = Some(next_path);
                while let Some(link) = cursor {
                    charge_general_parallel_work_v1(work, max_work, 1)?;
                    ids.push(link.id);
                    cursor = link.previous.clone();
                }
                let factor = usize::BITS - ids.len().max(1).leading_zeros();
                charge_general_parallel_work_v1(
                    work,
                    max_work,
                    u64::try_from(ids.len()).map_err(|_| ())? * (u64::from(factor) + 1),
                )?;
                canonicalize_constraint_ids(&mut ids);
                if let Some(current) = &best {
                    charge_general_parallel_work_v1(
                        work,
                        max_work,
                        u64::try_from(ids.len().min(current.len())).map_err(|_| ())?,
                    )?;
                }
                if best
                    .as_ref()
                    .is_none_or(|current| canonical_id_slice_cmp(&ids, current).is_lt())
                {
                    best = Some(ids);
                }
                continue;
            }
            queue.push_back((*neighbor, Some(next_path)));
            charge_general_parallel_work_v1(work, max_work, 1)?;
        }
    }
    Ok((best, false))
}

fn charge_general_parallel_work_v1(work: &mut u64, max_work: u64, amount: u64) -> Result<(), ()> {
    *work = work.checked_add(amount).ok_or(())?;
    #[cfg(test)]
    GENERAL_PARALLEL_TEST_WORK_OBSERVED.with(|observed| observed.set(*work));
    (*work <= max_work).then_some(()).ok_or(())
}

#[cfg(test)]
std::thread_local! {
    static GENERAL_PARALLEL_TEST_WORK_LIMIT: std::cell::Cell<Option<u64>> = const {
        std::cell::Cell::new(None)
    };
    static GENERAL_PARALLEL_TEST_WORK_OBSERVED: std::cell::Cell<u64> = const {
        std::cell::Cell::new(0)
    };
}

#[cfg(test)]
std::thread_local! {
    static GENERAL_EQUAL_TEST_WORK_LIMIT: std::cell::Cell<Option<u64>> = const {
        std::cell::Cell::new(None)
    };
    static GENERAL_EQUAL_TEST_WORK_OBSERVED: std::cell::Cell<u64> = const {
        std::cell::Cell::new(0)
    };
}

#[cfg(test)]
std::thread_local! {
    static DIRECTED_RATIO_TEST_LIMITS: std::cell::Cell<Option<directed_ratio_closure::Limits>> =
        const { std::cell::Cell::new(None) };
}

#[cfg(test)]
fn directed_ratio_test_limits_v1() -> directed_ratio_closure::Limits {
    DIRECTED_RATIO_TEST_LIMITS.with(|slot| slot.get().unwrap_or_default())
}

#[cfg(test)]
fn replace_directed_ratio_test_limits_v1(
    limits: Option<directed_ratio_closure::Limits>,
) -> Option<directed_ratio_closure::Limits> {
    DIRECTED_RATIO_TEST_LIMITS.with(|slot| slot.replace(limits))
}

fn canonicalize_constraint_ids(ids: &mut Vec<ConstraintId>) {
    ids.sort_unstable_by_key(ConstraintId::canonical_bytes);
    ids.dedup();
}

fn canonical_constraint_id_pair(first: ConstraintId, second: ConstraintId) -> [ConstraintId; 2] {
    if first.canonical_bytes() < second.canonical_bytes() {
        [first, second]
    } else {
        [second, first]
    }
}

/// Conservatively encloses every finite angle, in radians, that can make the
/// frozen fixed-angle residual exactly binary64 zero.
///
/// Let `d = fl(actual - expected)` and `a = fl(d + PI)`. For validated angles
/// and deterministic `atan2(abs(cross), dot)`, both `actual` and `expected`
/// are in `[0, PI]`, so the `rem_euclid(2 * PI)` branch cannot produce `PI`
/// from another congruence class. A zero final subtraction therefore requires
/// `a == PI`. Correct rounding bounds `d` by one upward spacing at `PI`; the
/// predecessor/successor expansion below also encloses the rounding error in
/// `actual - expected` and both endpoint additions. This deliberately wider
/// interval permits false negatives but no false positive conflict.
pub(crate) fn fixed_angle_zero_actual_enclosure_v1(angle_degrees: f64) -> Option<(f64, f64)> {
    let expected = deterministic_degrees_to_radians_v1(angle_degrees).ok()?;
    let pi = std::f64::consts::PI;
    if !expected.is_finite() || expected < 0.0 || expected > pi {
        return None;
    }
    let pi_upward_spacing = pi.next_up() - pi;
    let radius = pi_upward_spacing.next_up();
    let lower = (expected - radius).next_down().max(0.0);
    let upper = (expected + radius).next_up().min(pi);
    (lower <= upper).then_some((lower, upper))
}

fn incompatible_fixed_angle_pair_witness_v1(
    assignments: &[ScalarAssignment],
) -> Option<[ConstraintId; 2]> {
    let mut lowest_upper: Option<(f64, ConstraintId)> = None;
    let mut highest_lower: Option<(f64, ConstraintId)> = None;
    for assignment in assignments {
        let (lower, upper) = fixed_angle_zero_actual_enclosure_v1(assignment.value)?;
        if lowest_upper.is_none_or(|(current, id)| {
            upper < current
                || (upper.to_bits() == current.to_bits()
                    && assignment.id.canonical_bytes() < id.canonical_bytes())
        }) {
            lowest_upper = Some((upper, assignment.id));
        }
        if highest_lower.is_none_or(|(current, id)| {
            lower > current
                || (lower.to_bits() == current.to_bits()
                    && assignment.id.canonical_bytes() < id.canonical_bytes())
        }) {
            highest_lower = Some((lower, assignment.id));
        }
    }
    let (upper, upper_id) = lowest_upper?;
    let (lower, lower_id) = highest_lower?;
    (upper < lower && upper_id != lower_id).then_some([upper_id, lower_id])
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

fn is_proven_direct_conflict_v1(candidate: &DirectConstraintConflictV1) -> bool {
    // The legacy collinear-rotation candidate has two records. Only the
    // independently derived four-record exact quarter-turn/unit-horizontal
    // witness is proven; arbitrary stored angles remain solver-required.
    if matches!(
        &candidate.conflict,
        DirectConstraintConflictKindV1::RotationalSymmetryWithCollinearRadius { .. }
    ) {
        return candidate.constraint_ids.len() == 4;
    }
    if matches!(
        &candidate.conflict,
        DirectConstraintConflictKindV1::MirrorSymmetryWithPointOnAxisAndFixedSeparation { .. }
    ) {
        // The generic mirror candidate has three records. The seven-record
        // subset additionally anchors the solver's directional first vertex
        // to the real axis start with exact horizontal/vertical residuals.
        return candidate.constraint_ids.len() == 7;
    }
    matches!(
        &candidate.conflict,
        DirectConstraintConflictKindV1::DifferentFixedLengths { .. }
            | DirectConstraintConflictKindV1::DifferentFixedAngles { .. }
            | DirectConstraintConflictKindV1::DifferentLengthRatios { .. }
            | DirectConstraintConflictKindV1::HorizontalAndVertical { .. }
            | DirectConstraintConflictKindV1::EqualLengthWithDifferentFixedLengths { .. }
            | DirectConstraintConflictKindV1::EqualLengthWithNonUnitRatioAndFixedLength { .. }
            | DirectConstraintConflictKindV1::NonReciprocalLengthRatiosWithFixedLength { .. }
            | DirectConstraintConflictKindV1::LengthRatioWithIncompatibleFixedLengths { .. }
            | DirectConstraintConflictKindV1::NonUnitLengthRatioCycleWithFixedLength { .. }
            | DirectConstraintConflictKindV1::InconsistentLengthRatioGraphWithFixedLength { .. }
            | DirectConstraintConflictKindV1::DifferentFixedLengthsInEqualLengthComponent { .. }
            | DirectConstraintConflictKindV1::ParallelWithFixedNonParallelAngle { .. }
            | DirectConstraintConflictKindV1::ParallelWithPerpendicularOrientations { .. }
            | DirectConstraintConflictKindV1::SameOrientationWithFixedNonParallelAngle { .. }
            | DirectConstraintConflictKindV1::PerpendicularOrientationsWithFixedNonRightAngle { .. }
            | DirectConstraintConflictKindV1::DifferentRotationalSymmetryAnglesWithFixedRadius { .. }
            | DirectConstraintConflictKindV1::
                NonComplementaryInverseRotationalSymmetryAnglesWithFixedRadius { .. }
            | DirectConstraintConflictKindV1::PositiveFixedLengthInBoundedZeroLengthClosure { .. }
            | DirectConstraintConflictKindV1::ZeroLengthClosureReachesNondegenerateProvider { .. }
    )
}

fn quarantine_unproven_direct_conflicts_v1(
    conflicts: &mut Vec<DirectConstraintConflictV1>,
    unchecked: &mut Vec<ConstraintId>,
) {
    conflicts.retain(|candidate| {
        if is_proven_direct_conflict_v1(candidate) {
            true
        } else {
            #[cfg(test)]
            record_quarantined_direct_conflict(candidate);
            unchecked.extend(candidate.constraint_ids.iter().copied());
            false
        }
    });
}

/// Indexes real pattern edges by their canonical unordered endpoint pair.
///
/// Constraint records only name edge IDs, so [`edge_id_lookup`] cannot prove
/// that an edge actually joins two vertices. The endpoint relation that makes
/// an edge a radius of a rotation triple is read once from the source pattern,
/// linearly in the pattern edge count.
struct PatternEdgeIndex {
    by_pair: BTreeMap<VertexPairKey, Vec<EdgeId>>,
    by_id: BTreeMap<CanonicalId, (EdgeId, VertexPairKey)>,
    axis_starts: BTreeMap<CanonicalId, VertexId>,
}

fn pattern_edge_index(
    pattern: &CreasePattern,
    needs_pair_index: bool,
    needs_id_index: bool,
) -> PatternEdgeIndex {
    let mut by_pair: BTreeMap<VertexPairKey, Vec<EdgeId>> = BTreeMap::new();
    let mut by_id = BTreeMap::new();
    let mut axis_starts = BTreeMap::new();
    for edge in &pattern.edges {
        let pair = VertexPairKey::unordered(edge.start, edge.end);
        if needs_pair_index {
            by_pair.entry(pair).or_default().push(edge.id);
        }
        if needs_id_index {
            by_id.insert(edge.id.canonical_bytes(), (edge.id, pair));
            axis_starts.insert(edge.id.canonical_bytes(), edge.start);
        }
    }
    PatternEdgeIndex {
        by_pair,
        by_id,
        axis_starts,
    }
}

/// Selects the canonical positive radius witness for one rotation triple.
///
/// Both `{center, source}` and `{center, target}` radii are candidates and the
/// smallest `(fixed constraint ID, edge ID)` pair wins, so neither side is
/// preferred. Only edges whose fixed lengths agree on a single positive value
/// are admitted; inconsistent groups stay with
/// [`DirectConstraintConflictKindV1::DifferentFixedLengths`] instead of being
/// mined for a convenient radius.
fn canonical_positive_radius_witness(
    radius_edges: &BTreeMap<VertexPairKey, Vec<EdgeId>>,
    fixed_lengths: &BTreeMap<CanonicalId, ScalarGroupSummary>,
    center_vertex: VertexId,
    source_vertex: VertexId,
    target_vertex: VertexId,
) -> Option<(ConstraintId, EdgeId)> {
    let mut best: Option<(ConstraintId, EdgeId)> = None;
    for pair in [
        VertexPairKey::unordered(center_vertex, source_vertex),
        VertexPairKey::unordered(center_vertex, target_vertex),
    ] {
        for edge in radius_edges.get(&pair).into_iter().flatten() {
            let Some(assignment) = fixed_lengths
                .get(&edge.canonical_bytes())
                .and_then(ScalarGroupSummary::consistent_assignment)
            else {
                continue;
            };
            if !assignment.value.is_finite() || assignment.value <= 0.0 {
                continue;
            }
            let candidate = (assignment.id, *edge);
            if best.is_none_or(|current| {
                (candidate.0.canonical_bytes(), candidate.1.canonical_bytes())
                    < (current.0.canonical_bytes(), current.1.canonical_bytes())
            }) {
                best = Some(candidate);
            }
        }
    }
    best
}

/// Selects one exact line/radius edge for the collinear-rotation theorem.
///
/// The only admitted shapes are `source on line(center, target)` and `target
/// on line(center, source)`. All per-edge joining is completed once by
/// [`canonical_point_line_witnesses`], so each rotation relation performs
/// only two ordered-map lookups rather than rescanning a shared edge bucket.
fn canonical_collinear_rotation_witness(
    point_lines: &BTreeMap<(CanonicalId, VertexPairKey), (ConstraintId, EdgeId)>,
    center_vertex: VertexId,
    source_vertex: VertexId,
    target_vertex: VertexId,
) -> Option<(ConstraintId, EdgeId)> {
    [
        (
            source_vertex.canonical_bytes(),
            VertexPairKey::unordered(center_vertex, target_vertex),
        ),
        (
            target_vertex.canonical_bytes(),
            VertexPairKey::unordered(center_vertex, source_vertex),
        ),
    ]
    .into_iter()
    .filter_map(|key| point_lines.get(&key).copied())
    .min_by_key(|candidate| (candidate.0.canonical_bytes(), candidate.1.canonical_bytes()))
}

/// Joins every distinct `(point, line edge)` record group to its real pattern
/// endpoint pair exactly once.
///
/// The resulting key discards the edge identity only after the exact edge
/// lookup and retains the canonical smallest complete witness for that point
/// and endpoint pair. Thus duplicate real edges remain deterministic without
/// allowing any relation to trigger a repeated scan of the same edge bucket.
fn canonical_point_line_witnesses(
    edge_pairs: &BTreeMap<CanonicalId, (EdgeId, VertexPairKey)>,
    points_on_lines: &BTreeMap<(CanonicalId, CanonicalId), Vec<ConstraintId>>,
) -> BTreeMap<(CanonicalId, VertexPairKey), (ConstraintId, EdgeId)> {
    let mut result: BTreeMap<(CanonicalId, VertexPairKey), (ConstraintId, EdgeId)> =
        BTreeMap::new();
    for ((point_vertex, line_edge), point_ids) in points_on_lines {
        #[cfg(test)]
        record_point_line_join_visit();
        let Some((edge_id, edge_pair)) = edge_pairs.get(line_edge).copied() else {
            continue;
        };
        let Some(point_id) = point_ids
            .iter()
            .copied()
            .min_by_key(ConstraintId::canonical_bytes)
        else {
            continue;
        };
        let candidate = (point_id, edge_id);
        result
            .entry((*point_vertex, edge_pair))
            .and_modify(|current| {
                if (candidate.0.canonical_bytes(), candidate.1.canonical_bytes())
                    < (current.0.canonical_bytes(), current.1.canonical_bytes())
                {
                    *current = candidate;
                }
            })
            .or_insert(candidate);
    }
    result
}

#[cfg(test)]
std::thread_local! {
    static POINT_LINE_JOIN_VISITS: std::cell::Cell<Option<usize>> =
        const { std::cell::Cell::new(None) };
}

#[cfg(test)]
fn record_point_line_join_visit() {
    POINT_LINE_JOIN_VISITS.with(|visits| {
        if let Some(current) = visits.get() {
            visits.set(Some(
                current
                    .checked_add(1)
                    .expect("test-only point-line join counter overflow"),
            ));
        }
    });
}

#[cfg(test)]
fn begin_point_line_join_visit_count() {
    POINT_LINE_JOIN_VISITS.with(|visits| {
        assert_eq!(
            visits.replace(Some(0)),
            None,
            "point-line join counter is already active on this test thread"
        );
    });
}

#[cfg(test)]
fn finish_point_line_join_visit_count() -> usize {
    POINT_LINE_JOIN_VISITS.with(|visits| {
        visits
            .replace(None)
            .expect("point-line join counter was not active")
    })
}

/// Selects the canonical positive fixed-length witness for exactly one
/// unordered pattern-vertex pair.
///
/// Fixed-length groups must agree bit-for-bit. A contradictory group is never
/// reused as secondary evidence for a mirror contradiction.
fn canonical_positive_fixed_edge_witness(
    edges_by_pair: &BTreeMap<VertexPairKey, Vec<EdgeId>>,
    fixed_lengths: &BTreeMap<CanonicalId, ScalarGroupSummary>,
    pair: VertexPairKey,
) -> Option<(ConstraintId, EdgeId)> {
    let mut best: Option<(ConstraintId, EdgeId)> = None;
    for edge in edges_by_pair.get(&pair).into_iter().flatten() {
        let Some(assignment) = fixed_lengths
            .get(&edge.canonical_bytes())
            .and_then(ScalarGroupSummary::consistent_assignment)
        else {
            continue;
        };
        if !assignment.value.is_finite() || assignment.value <= 0.0 {
            continue;
        }
        let candidate = (assignment.id, *edge);
        if best.is_none_or(|current| {
            (candidate.0.canonical_bytes(), candidate.1.canonical_bytes())
                < (current.0.canonical_bytes(), current.1.canonical_bytes())
        }) {
            best = Some(candidate);
        }
    }
    best
}

/// Selects the canonical exact-unit fixed-length witness for exactly one
/// unordered pattern-vertex pair.
///
/// The strict mirror proof needs its separation residual to force one full
/// binary64 unit, not merely an arbitrary positive magnitude.
fn canonical_unit_fixed_edge_witness(
    edges_by_pair: &BTreeMap<VertexPairKey, Vec<EdgeId>>,
    fixed_lengths: &BTreeMap<CanonicalId, ScalarGroupSummary>,
    pair: VertexPairKey,
) -> Option<(ConstraintId, EdgeId)> {
    let mut best: Option<(ConstraintId, EdgeId)> = None;
    for edge in edges_by_pair.get(&pair).into_iter().flatten() {
        let Some(assignment) = fixed_lengths
            .get(&edge.canonical_bytes())
            .and_then(ScalarGroupSummary::consistent_assignment)
            .filter(|assignment| assignment.value.to_bits() == 1.0_f64.to_bits())
        else {
            continue;
        };
        let candidate = (assignment.id, *edge);
        if best.is_none_or(|current| {
            (candidate.0.canonical_bytes(), candidate.1.canonical_bytes())
                < (current.0.canonical_bytes(), current.1.canonical_bytes())
        }) {
            best = Some(candidate);
        }
    }
    best
}

/// Selects one real connector edge whose horizontal and vertical residuals
/// jointly force both endpoint coordinate deltas to binary64 zero.
fn canonical_horizontal_vertical_edge_witness(
    edges_by_pair: &BTreeMap<VertexPairKey, Vec<EdgeId>>,
    horizontal: &BTreeMap<CanonicalId, Vec<ConstraintId>>,
    vertical: &BTreeMap<CanonicalId, Vec<ConstraintId>>,
    pair: VertexPairKey,
) -> Option<(ConstraintId, ConstraintId)> {
    let mut best: Option<(ConstraintId, ConstraintId)> = None;
    for edge in edges_by_pair.get(&pair).into_iter().flatten() {
        let Some(horizontal_id) = horizontal
            .get(&edge.canonical_bytes())
            .into_iter()
            .flatten()
            .copied()
            .min_by_key(ConstraintId::canonical_bytes)
        else {
            continue;
        };
        let Some(vertical_id) = vertical
            .get(&edge.canonical_bytes())
            .into_iter()
            .flatten()
            .copied()
            .min_by_key(ConstraintId::canonical_bytes)
        else {
            continue;
        };
        let candidate = (horizontal_id, vertical_id);
        if best.is_none_or(|current| {
            (candidate.0.canonical_bytes(), candidate.1.canonical_bytes())
                < (current.0.canonical_bytes(), current.1.canonical_bytes())
        }) {
            best = Some(candidate);
        }
    }
    best
}

/// Selects the canonical exact-unit horizontal radius on one ordered
/// center-source pair.
///
/// The quarter-turn direct proof needs both properties on the same real edge:
/// a unit length makes normalization exact and Horizontal makes the source
/// delta's vertical component exactly zero.
fn canonical_unit_horizontal_radius_witness(
    edges_by_pair: &BTreeMap<VertexPairKey, Vec<EdgeId>>,
    fixed_lengths: &BTreeMap<CanonicalId, ScalarGroupSummary>,
    horizontal: &BTreeMap<CanonicalId, Vec<ConstraintId>>,
    center_vertex: VertexId,
    source_vertex: VertexId,
) -> Option<(ConstraintId, ConstraintId)> {
    let mut best: Option<(ConstraintId, ConstraintId)> = None;
    let pair = VertexPairKey::unordered(center_vertex, source_vertex);
    for edge in edges_by_pair.get(&pair).into_iter().flatten() {
        let Some(fixed) = fixed_lengths
            .get(&edge.canonical_bytes())
            .and_then(ScalarGroupSummary::consistent_assignment)
            .filter(|assignment| assignment.value.to_bits() == 1.0_f64.to_bits())
        else {
            continue;
        };
        let Some(horizontal_id) = horizontal
            .get(&edge.canonical_bytes())
            .into_iter()
            .flatten()
            .copied()
            .min_by_key(ConstraintId::canonical_bytes)
        else {
            continue;
        };
        let candidate = (fixed.id, horizontal_id);
        if best.is_none_or(|current| {
            (candidate.0.canonical_bytes(), candidate.1.canonical_bytes())
                < (current.0.canonical_bytes(), current.1.canonical_bytes())
        }) {
            best = Some(candidate);
        }
    }
    best
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

/// Total order over conflict kinds.
///
/// The final slot lets four-entity variants include their complete identity
/// while preserving every pre-existing variant's ordering; shorter keys pad
/// unused slots with zero. New variants are appended by rank.
fn conflict_sort_key(
    conflict: &DirectConstraintConflictKindV1,
) -> (u8, CanonicalId, CanonicalId, CanonicalId, CanonicalId) {
    let zero = [0; 16];
    match conflict {
        DirectConstraintConflictKindV1::DifferentFixedLengths { edge } => {
            (0, edge.canonical_bytes(), zero, zero, zero)
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
            zero,
        ),
        DirectConstraintConflictKindV1::DifferentLengthRatios {
            numerator_edge,
            denominator_edge,
        } => (
            2,
            numerator_edge.canonical_bytes(),
            denominator_edge.canonical_bytes(),
            zero,
            zero,
        ),
        DirectConstraintConflictKindV1::HorizontalAndVertical { edge } => {
            (3, edge.canonical_bytes(), zero, zero, zero)
        }
        DirectConstraintConflictKindV1::EqualLengthWithDifferentFixedLengths {
            first_edge,
            second_edge,
        } => (
            4,
            first_edge.canonical_bytes(),
            second_edge.canonical_bytes(),
            zero,
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
            zero,
        ),
        DirectConstraintConflictKindV1::NonReciprocalLengthRatiosWithFixedLength {
            first_edge,
            second_edge,
        } => (
            6,
            first_edge.canonical_bytes(),
            second_edge.canonical_bytes(),
            zero,
            zero,
        ),
        DirectConstraintConflictKindV1::LengthRatioWithIncompatibleFixedLengths {
            numerator_edge,
            denominator_edge,
        } => (
            7,
            numerator_edge.canonical_bytes(),
            denominator_edge.canonical_bytes(),
            zero,
            zero,
        ),
        DirectConstraintConflictKindV1::NonUnitLengthRatioCycleWithFixedLength {
            first_edge,
            second_edge,
            third_edge,
            fixed_edge: _,
        } => (
            8,
            first_edge.canonical_bytes(),
            second_edge.canonical_bytes(),
            third_edge.canonical_bytes(),
            zero,
        ),
        DirectConstraintConflictKindV1::InconsistentLengthRatioGraphWithFixedLength {
            fixed_edge,
            ratio_constraint_count,
        } => (
            9,
            fixed_edge.canonical_bytes(),
            [0; 16],
            u128::from(*ratio_constraint_count).to_be_bytes(),
            zero,
        ),
        DirectConstraintConflictKindV1::DifferentFixedLengthsInEqualLengthComponent {
            first_edge,
            second_edge,
            equal_constraint_count,
        } => (
            10,
            first_edge.canonical_bytes(),
            second_edge.canonical_bytes(),
            u128::from(*equal_constraint_count).to_be_bytes(),
            zero,
        ),
        DirectConstraintConflictKindV1::PerpendicularOrientationsInParallelComponent {
            horizontal_edge,
            vertical_edge,
            parallel_constraint_count,
        } => (
            11,
            horizontal_edge.canonical_bytes(),
            vertical_edge.canonical_bytes(),
            u128::from(*parallel_constraint_count).to_be_bytes(),
            zero,
        ),
        DirectConstraintConflictKindV1::NonParallelFixedAngleInParallelComponent {
            vertex,
            first_edge,
            second_edge,
            parallel_constraint_count: _,
        } => (
            12,
            vertex.canonical_bytes(),
            first_edge.canonical_bytes(),
            second_edge.canonical_bytes(),
            zero,
        ),
        DirectConstraintConflictKindV1::ParallelWithFixedNonParallelAngle {
            first_edge,
            second_edge,
        } => (
            13,
            first_edge.canonical_bytes(),
            second_edge.canonical_bytes(),
            zero,
            zero,
        ),
        DirectConstraintConflictKindV1::ParallelWithPerpendicularOrientations {
            horizontal_edge,
            vertical_edge,
        } => (
            14,
            horizontal_edge.canonical_bytes(),
            vertical_edge.canonical_bytes(),
            zero,
            zero,
        ),
        DirectConstraintConflictKindV1::SameOrientationWithFixedNonParallelAngle {
            first_edge,
            second_edge,
        } => (
            15,
            first_edge.canonical_bytes(),
            second_edge.canonical_bytes(),
            zero,
            zero,
        ),
        DirectConstraintConflictKindV1::PerpendicularOrientationsWithFixedNonRightAngle {
            horizontal_edge,
            vertical_edge,
        } => (
            16,
            horizontal_edge.canonical_bytes(),
            vertical_edge.canonical_bytes(),
            zero,
            zero,
        ),
        DirectConstraintConflictKindV1::DifferentRotationalSymmetryAnglesWithFixedRadius {
            center_vertex,
            source_vertex,
            target_vertex,
            fixed_radius_edge,
        } => (
            17,
            center_vertex.canonical_bytes(),
            source_vertex.canonical_bytes(),
            target_vertex.canonical_bytes(),
            fixed_radius_edge.canonical_bytes(),
        ),
        DirectConstraintConflictKindV1::
            NonComplementaryInverseRotationalSymmetryAnglesWithFixedRadius {
                center_vertex,
                source_vertex,
                target_vertex,
                fixed_radius_edge,
            } => (
                18,
                center_vertex.canonical_bytes(),
                source_vertex.canonical_bytes(),
                target_vertex.canonical_bytes(),
                fixed_radius_edge.canonical_bytes(),
            ),
        DirectConstraintConflictKindV1::
            MirrorSymmetryWithPointOnAxisAndFixedSeparation {
                first_vertex,
                second_vertex,
                axis_edge,
                fixed_separation_edge,
            } => (
                19,
                first_vertex.canonical_bytes(),
                second_vertex.canonical_bytes(),
                axis_edge.canonical_bytes(),
                fixed_separation_edge.canonical_bytes(),
            ),
        DirectConstraintConflictKindV1::
            RotationalSymmetryWithCollinearRadius {
                center_vertex,
                source_vertex,
                target_vertex,
                line_edge,
        } => (
            20,
            center_vertex.canonical_bytes(),
            source_vertex.canonical_bytes(),
            target_vertex.canonical_bytes(),
            line_edge.canonical_bytes(),
        ),
        DirectConstraintConflictKindV1::PositiveFixedLengthInBoundedZeroLengthClosure {
            fixed_edge,
            forced_zero_edge,
            horizontal_constraint_count,
            vertical_constraint_count,
            zero_propagation_constraint_count,
        } => {
            let mut counts = [0_u8; 16];
            counts[0..2].copy_from_slice(&horizontal_constraint_count.to_be_bytes());
            counts[2..4].copy_from_slice(&vertical_constraint_count.to_be_bytes());
            counts[4..6].copy_from_slice(&zero_propagation_constraint_count.to_be_bytes());
            (
                21,
                fixed_edge.canonical_bytes(),
                forced_zero_edge.canonical_bytes(),
                counts,
                zero,
            )
        }
        DirectConstraintConflictKindV1::ZeroLengthClosureReachesNondegenerateProvider {
            provider_kind,
            provider_edge,
            forced_zero_edge,
            horizontal_constraint_count,
            vertical_constraint_count,
            zero_propagation_constraint_count,
        } => {
            let mut metadata = [0_u8; 16];
            metadata[0] = match provider_kind {
                ZeroLengthClosureProviderKindV1::PointOnLine => 0,
                ZeroLengthClosureProviderKindV1::MirrorSymmetryAxis => 1,
                ZeroLengthClosureProviderKindV1::AngleBisector => 2,
                ZeroLengthClosureProviderKindV1::Parallel => 3,
                ZeroLengthClosureProviderKindV1::FixedAngle => 4,
            };
            metadata[2..4].copy_from_slice(&horizontal_constraint_count.to_be_bytes());
            metadata[4..6].copy_from_slice(&vertical_constraint_count.to_be_bytes());
            metadata[6..8].copy_from_slice(&zero_propagation_constraint_count.to_be_bytes());
            (
                22,
                provider_edge.canonical_bytes(),
                forced_zero_edge.canonical_bytes(),
                metadata,
                zero,
            )
        }
    }
}

fn canonical_id_slice_cmp(left: &[ConstraintId], right: &[ConstraintId]) -> std::cmp::Ordering {
    left.iter()
        .map(ConstraintId::canonical_bytes)
        .cmp(right.iter().map(ConstraintId::canonical_bytes))
}

#[cfg(test)]
#[path = "constraints_equal_ratio_fixed_tests.rs"]
mod equal_ratio_fixed_tests;

#[cfg(test)]
#[path = "constraints_different_fixed_angles_tests.rs"]
mod different_fixed_angles_tests;

#[cfg(test)]
#[path = "constraints_general_ratio_graph_limits_tests.rs"]
mod general_ratio_graph_limits_tests;

#[cfg(test)]
#[path = "constraints_general_ratio_graph_soundness_tests.rs"]
mod general_ratio_graph_soundness_tests;

#[cfg(test)]
#[path = "constraints_general_ratio_graph_tests.rs"]
mod general_ratio_graph_tests;

#[cfg(test)]
#[path = "constraints_inverse_cardinal_rotation_tests.rs"]
mod inverse_cardinal_rotation_tests;

#[cfg(test)]
#[path = "constraints_inverse_cardinal_rotation_limits_tests.rs"]
mod inverse_cardinal_rotation_limits_tests;

#[cfg(test)]
#[path = "constraints_nonreciprocal_ratio_fixed_tests.rs"]
mod nonreciprocal_ratio_fixed_tests;

#[cfg(test)]
#[path = "constraints_ratio_cycle_limits_tests.rs"]
mod ratio_cycle_limits_tests;

#[cfg(test)]
#[path = "constraints_ratio_cycle_tests.rs"]
mod ratio_cycle_tests;

#[cfg(test)]
#[path = "constraints_perpendicular_angle_limits_tests.rs"]
mod perpendicular_angle_limits_tests;

#[cfg(test)]
#[path = "constraints_perpendicular_angle_tests.rs"]
mod perpendicular_angle_tests;

#[cfg(test)]
#[path = "constraints_unit_parallel_angle_tests.rs"]
mod unit_parallel_angle_tests;

#[cfg(test)]
#[path = "constraints_unit_quarter_turn_tests.rs"]
mod unit_quarter_turn_tests;

#[cfg(test)]
#[path = "constraints_anchored_unit_mirror_tests.rs"]
mod anchored_unit_mirror_tests;

#[cfg(test)]
#[path = "constraints_preflight_contract_tests.rs"]
mod preflight_contract_tests;

#[cfg(test)]
#[path = "constraints_same_orientation_angle_limits_tests.rs"]
mod same_orientation_angle_limits_tests;

#[cfg(test)]
#[path = "constraints_same_orientation_angle_tests.rs"]
mod same_orientation_angle_tests;

#[cfg(test)]
mod tests {
    use ori_domain::{EdgeKind, Point2};
    use serde_json::{Value, json};

    use super::*;

    pub(super) struct Fixture {
        pub(super) pattern: CreasePattern,
        pub(super) vertices: [VertexId; 7],
        pub(super) edges: [EdgeId; 6],
    }

    impl Fixture {
        pub(super) fn new() -> Self {
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

        pub(super) fn all_kinds(&self) -> Vec<GeometricConstraintKindV1> {
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

    pub(super) fn record(constraint: GeometricConstraintKindV1) -> GeometricConstraintRecordV1 {
        GeometricConstraintRecordV1 {
            id: ConstraintId::new(),
            constraint,
        }
    }

    pub(super) fn document(
        constraints: impl IntoIterator<Item = GeometricConstraintRecordV1>,
    ) -> GeometricConstraintDocumentV1 {
        GeometricConstraintDocumentV1 {
            schema_version: GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
            constraints: constraints.into_iter().collect(),
        }
    }

    pub(super) fn prepare<'pattern>(
        fixture: &'pattern Fixture,
        document: &GeometricConstraintDocumentV1,
    ) -> Result<GeometricConstraintSetV1<'pattern>, GeometricConstraintErrorV1> {
        prepare_geometric_constraints_v1(
            &fixture.pattern,
            document,
            GeometricConstraintLimitsV1::default(),
        )
    }

    fn rotation(
        fixture: &Fixture,
        center: usize,
        source: usize,
        target: usize,
        angle_degrees: f64,
    ) -> GeometricConstraintKindV1 {
        GeometricConstraintKindV1::RotationalSymmetry {
            center_vertex: fixture.vertices[center],
            source_vertex: fixture.vertices[source],
            target_vertex: fixture.vertices[target],
            angle_degrees,
        }
    }

    fn fixed_length(fixture: &Fixture, edge: usize, length_mm: f64) -> GeometricConstraintKindV1 {
        GeometricConstraintKindV1::FixedLength {
            edge: fixture.edges[edge],
            length_mm,
        }
    }

    fn radius_padding(fixture: &Fixture) -> GeometricConstraintKindV1 {
        GeometricConstraintKindV1::Horizontal {
            edge: fixture.edges[0],
        }
    }

    fn assert_solver_required(preflight: &ConstraintPreflightV1) {
        assert!(matches!(
            preflight,
            ConstraintPreflightV1::Unknown {
                reason: GeometricConstraintUnknownReasonV1::SolverRequiredConstraintKinds,
                unchecked_constraint_ids,
            } if !unchecked_constraint_ids.is_empty()
        ));
    }

    fn assert_bounded_direct_oracle_unknown(prepared: &GeometricConstraintSetV1<'_>) {
        assert!(matches!(
            find_bounded_direct_mus_v1(prepared),
            BoundedDirectMusV1::Unknown { .. }
        ));
    }

    fn assert_same_edge_parallel_zero_closure_is_exact_and_minimal(
        fixture: &Fixture,
        records: &[GeometricConstraintRecordV1],
        edge: EdgeId,
    ) {
        assert_eq!(records.len(), 3, "the exact proof core has three records");
        let expected_ids = sorted_ids(&records.iter().map(|record| record.id).collect::<Vec<_>>());
        let prepared = prepare(fixture, &document(records.iter().cloned()))
            .expect("same-edge parallel non-degeneracy terminal");
        assert_eq!(
            prepared.preflight(),
            ConstraintPreflightV1::DirectConflict {
                conflicts: vec![DirectConstraintConflictV1 {
                    conflict: DirectConstraintConflictKindV1::
                        ZeroLengthClosureReachesNondegenerateProvider {
                            provider_kind: ZeroLengthClosureProviderKindV1::Parallel,
                            provider_edge: edge,
                            forced_zero_edge: edge,
                            horizontal_constraint_count: 1,
                            vertical_constraint_count: 1,
                            zero_propagation_constraint_count: 0,
                        },
                    constraint_ids: expected_ids.clone(),
                }],
            }
        );
        let BoundedDirectMusV1::ProvenUnsatisfiable {
            constraint_ids,
            oracle_calls,
        } = find_bounded_direct_mus_v1(&prepared)
        else {
            panic!("the parallel non-degeneracy terminal must feed the bounded oracle");
        };
        assert_eq!(constraint_ids, expected_ids);
        assert_eq!(oracle_calls, 7);
        for removed in records.iter().map(|record| record.id) {
            let subset = records
                .iter()
                .filter(|record| record.id != removed)
                .cloned();
            assert!(!matches!(
                prepare(fixture, &document(subset))
                    .expect("proper same-edge parallel terminal subset")
                    .preflight(),
                ConstraintPreflightV1::DirectConflict { .. }
            ));
        }
    }

    // These helpers inspect both emitted allowlisted conflicts and quarantined
    // legacy recognizer output. Each test must separately assert whether its
    // family is proof-authoritative or remains solver-required.
    fn emitted_and_quarantined_conflicts(
        preflight: &ConstraintPreflightV1,
    ) -> Vec<DirectConstraintConflictV1> {
        let mut candidates = match preflight {
            ConstraintPreflightV1::DirectConflict { conflicts } => conflicts.clone(),
            ConstraintPreflightV1::NoDirectConflict | ConstraintPreflightV1::Unknown { .. } => {
                Vec::new()
            }
        };
        candidates.extend(last_quarantined_direct_conflicts());
        candidates
    }

    fn rotation_conflicts(preflight: &ConstraintPreflightV1) -> Vec<DirectConstraintConflictV1> {
        let conflicts = match preflight {
            ConstraintPreflightV1::DirectConflict { conflicts } => conflicts.clone(),
            ConstraintPreflightV1::NoDirectConflict | ConstraintPreflightV1::Unknown { .. } => {
                Vec::new()
            }
        };
        conflicts
            .into_iter()
            .filter(|conflict| {
                matches!(
                    conflict.conflict(),
                    DirectConstraintConflictKindV1::
                        DifferentRotationalSymmetryAnglesWithFixedRadius { .. }
                )
            })
            .collect()
    }

    fn only_rotation_conflict(
        fixture: &Fixture,
        raw: &GeometricConstraintDocumentV1,
    ) -> Option<DirectConstraintConflictV1> {
        let prepared = prepare(fixture, raw).expect("rotation fixture prepares");
        let preflight = prepared.preflight();
        let mut found = rotation_conflicts(&preflight);
        (found.len() == 1).then(|| found.remove(0))
    }

    fn inverse_rotation_conflicts(
        preflight: &ConstraintPreflightV1,
    ) -> Vec<DirectConstraintConflictV1> {
        let conflicts = match preflight {
            ConstraintPreflightV1::DirectConflict { conflicts } => conflicts.clone(),
            ConstraintPreflightV1::NoDirectConflict | ConstraintPreflightV1::Unknown { .. } => {
                Vec::new()
            }
        };
        conflicts
            .into_iter()
            .filter(|conflict| {
                matches!(
                    conflict.conflict(),
                    DirectConstraintConflictKindV1::
                        NonComplementaryInverseRotationalSymmetryAnglesWithFixedRadius { .. }
                )
            })
            .collect()
    }

    fn only_inverse_rotation_conflict(
        fixture: &Fixture,
        raw: &GeometricConstraintDocumentV1,
    ) -> Option<DirectConstraintConflictV1> {
        let prepared = prepare(fixture, raw).expect("inverse rotation fixture prepares");
        let preflight = prepared.preflight();
        let mut found = inverse_rotation_conflicts(&preflight);
        (found.len() == 1).then(|| found.remove(0))
    }

    fn mirror_axis_conflicts(preflight: &ConstraintPreflightV1) -> Vec<DirectConstraintConflictV1> {
        emitted_and_quarantined_conflicts(preflight)
            .into_iter()
            .filter(|conflict| {
                matches!(
                    conflict.conflict(),
                    DirectConstraintConflictKindV1::
                        MirrorSymmetryWithPointOnAxisAndFixedSeparation { .. }
                )
            })
            .collect()
    }

    fn only_mirror_axis_conflict(
        fixture: &Fixture,
        raw: &GeometricConstraintDocumentV1,
    ) -> Option<DirectConstraintConflictV1> {
        let prepared = prepare(fixture, raw).expect("mirror-axis fixture prepares");
        let preflight = prepared.preflight();
        assert_solver_required(&preflight);
        let mut found = mirror_axis_conflicts(&preflight);
        (found.len() == 1).then(|| found.remove(0))
    }

    fn collinear_rotation_conflicts(
        preflight: &ConstraintPreflightV1,
    ) -> Vec<DirectConstraintConflictV1> {
        emitted_and_quarantined_conflicts(preflight)
            .into_iter()
            .filter(|conflict| {
                matches!(
                    conflict.conflict(),
                    DirectConstraintConflictKindV1::RotationalSymmetryWithCollinearRadius { .. }
                )
            })
            .collect()
    }

    fn only_collinear_rotation_conflict(
        fixture: &Fixture,
        raw: &GeometricConstraintDocumentV1,
    ) -> Option<DirectConstraintConflictV1> {
        let prepared = prepare(fixture, raw).expect("collinear-rotation fixture prepares");
        let preflight = prepared.preflight();
        assert_solver_required(&preflight);
        let mut found = collinear_rotation_conflicts(&preflight);
        (found.len() == 1).then(|| found.remove(0))
    }

    fn collinear_rotation_witness_records(
        fixture: &Fixture,
        source_is_line_point: bool,
        angle_degrees: f64,
    ) -> [GeometricConstraintRecordV1; 2] {
        let (source, target) = if source_is_line_point { (2, 5) } else { (5, 2) };
        [
            record(rotation(fixture, 1, source, target, angle_degrees)),
            record(GeometricConstraintKindV1::PointOnLine {
                vertex: fixture.vertices[2],
                line_edge: fixture.edges[5],
            }),
        ]
    }

    #[test]
    fn non_half_turn_rotation_conflicts_when_either_radius_is_the_exact_line() {
        for source_is_line_point in [true, false] {
            let fixture = Fixture::new();
            let records = collinear_rotation_witness_records(&fixture, source_is_line_point, 90.0);
            let raw = document(records.clone());
            let conflict = only_collinear_rotation_conflict(&fixture, &raw)
                .expect("a normalized collinear radius excludes every non-half-turn rotation");
            let (source_vertex, target_vertex) = if source_is_line_point {
                (fixture.vertices[2], fixture.vertices[5])
            } else {
                (fixture.vertices[5], fixture.vertices[2])
            };
            assert_eq!(
                *conflict.conflict(),
                DirectConstraintConflictKindV1::RotationalSymmetryWithCollinearRadius {
                    center_vertex: fixture.vertices[1],
                    source_vertex,
                    target_vertex,
                    line_edge: fixture.edges[5],
                }
            );
            assert_eq!(
                conflict.constraint_ids(),
                sorted_ids(&records.map(|record| record.id))
            );

            let prepared = prepare(&fixture, &raw).expect("the exact witness prepares");
            assert_bounded_direct_oracle_unknown(&prepared);
        }
    }

    #[test]
    fn collinear_rotation_conflict_requires_non_half_turn_and_exact_roles_and_edge() {
        let fixture = Fixture::new();
        let negatives = [
            document(collinear_rotation_witness_records(&fixture, true, 180.0)),
            document([
                record(rotation(&fixture, 1, 2, 5, 90.0)),
                record(GeometricConstraintKindV1::PointOnLine {
                    vertex: fixture.vertices[2],
                    line_edge: fixture.edges[4],
                }),
            ]),
            document([
                record(rotation(&fixture, 1, 2, 5, 90.0)),
                record(GeometricConstraintKindV1::PointOnLine {
                    vertex: fixture.vertices[6],
                    line_edge: fixture.edges[5],
                }),
            ]),
        ];
        for raw in negatives {
            let preflight = prepare(&fixture, &raw)
                .expect("strict negative fixture prepares")
                .preflight();
            assert!(
                collinear_rotation_conflicts(&preflight).is_empty(),
                "half turns and unrelated roles or edges stay unchecked"
            );
        }

        let irrelevant_fixed_group = document([
            record(rotation(&fixture, 1, 2, 5, 90.0)),
            record(GeometricConstraintKindV1::PointOnLine {
                vertex: fixture.vertices[2],
                line_edge: fixture.edges[5],
            }),
            record(fixed_length(&fixture, 5, 1.0)),
            record(fixed_length(&fixture, 5, 1.0_f64.next_up())),
        ]);
        let preflight = prepare(&fixture, &irrelevant_fixed_group)
            .expect("bit-distinct positive lengths prepare")
            .preflight();
        assert_eq!(
            collinear_rotation_conflicts(&preflight).len(),
            1,
            "unrelated scalar conflicts neither establish nor suppress the two-ID theorem"
        );
        let ConstraintPreflightV1::DirectConflict { conflicts } = preflight else {
            panic!("both independent direct conflicts remain visible")
        };
        assert!(conflicts.iter().any(|conflict| matches!(
            conflict.conflict(),
            DirectConstraintConflictKindV1::DifferentFixedLengths { .. }
        )));
    }

    #[test]
    fn collinear_rotation_uses_constraints_not_initial_coordinates_and_admits_exact_extremes() {
        let fixture = Fixture::new();
        let initially_collinear = document([
            record(rotation(&fixture, 0, 1, 3, 90.0)),
            record(fixed_length(&fixture, 0, 1.0)),
        ]);
        assert_eq!(fixture.pattern.vertices[0].position.y, 0.0);
        assert_eq!(fixture.pattern.vertices[1].position.y, 0.0);
        assert_eq!(fixture.pattern.vertices[3].position.y, 0.0);
        assert!(
            collinear_rotation_conflicts(
                &prepare(&fixture, &initially_collinear)
                    .expect("initially collinear geometry prepares")
                    .preflight()
            )
            .is_empty(),
            "initial coordinates never replace an exact PointOnLine record"
        );

        for angle in [
            f64::from_bits(1),
            f64::MIN_POSITIVE,
            180.0_f64.next_down(),
            180.0_f64.next_up(),
            360.0_f64.next_down(),
        ] {
            let raw = document(collinear_rotation_witness_records(&fixture, true, angle));
            assert!(
                only_collinear_rotation_conflict(&fixture, &raw).is_some(),
                "every exact non-half-turn binary64 angle remains a real contradiction"
            );
        }
    }

    #[test]
    fn collinear_rotation_candidate_core_is_canonical_irredundant_and_order_independent() {
        let fixture = Fixture::new();
        let first_rotation = record(rotation(&fixture, 1, 2, 5, 90.0));
        let second_rotation = record(rotation(&fixture, 1, 2, 5, 90.0));
        let first_point = record(GeometricConstraintKindV1::PointOnLine {
            vertex: fixture.vertices[2],
            line_edge: fixture.edges[5],
        });
        let second_point = record(GeometricConstraintKindV1::PointOnLine {
            vertex: fixture.vertices[2],
            line_edge: fixture.edges[5],
        });
        let records = vec![
            first_rotation.clone(),
            second_rotation.clone(),
            first_point.clone(),
            second_point.clone(),
        ];
        let expected_ids = [
            [first_rotation.id, second_rotation.id]
                .into_iter()
                .min_by_key(ConstraintId::canonical_bytes)
                .expect("rotation minimum"),
            [first_point.id, second_point.id]
                .into_iter()
                .min_by_key(ConstraintId::canonical_bytes)
                .expect("point minimum"),
        ];
        let forward = document(records.clone());
        let forward_preflight = prepare(&fixture, &forward)
            .expect("duplicate witness prepares")
            .preflight();
        let mut found = collinear_rotation_conflicts(&forward_preflight);
        assert_eq!(found.len(), 1);
        assert_eq!(found.remove(0).constraint_ids(), sorted_ids(&expected_ids));

        let mut reversed_pattern = fixture.pattern.clone();
        reversed_pattern.edges.reverse();
        let reversed = document(records.into_iter().rev());
        let reversed_preflight = prepare_geometric_constraints_v1(
            &reversed_pattern,
            &reversed,
            GeometricConstraintLimitsV1::default(),
        )
        .expect("reversed duplicate witness prepares")
        .preflight();
        assert_eq!(
            serde_json::to_value(forward_preflight).unwrap(),
            serde_json::to_value(reversed_preflight).unwrap()
        );

        let minimal = collinear_rotation_witness_records(&fixture, true, 90.0);
        for omitted in 0..minimal.len() {
            let subset = document(
                minimal
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| *index != omitted)
                    .map(|(_, record)| record.clone()),
            );
            assert!(
                collinear_rotation_conflicts(&prepare(&fixture, &subset).unwrap().preflight())
                    .is_empty()
            );
        }
    }

    #[test]
    fn collinear_rotation_join_work_depends_on_unique_point_edge_buckets_not_rotation_count() {
        let fixture = Fixture::new();
        let mut records = Vec::new();
        let roles = [
            (1, 2, 5),
            (5, 2, 1),
            (1, 5, 2),
            (5, 1, 2),
            (0, 2, 1),
            (1, 2, 0),
            (0, 1, 2),
            (1, 0, 2),
        ];
        for _ in 0..24 {
            records.extend(roles.into_iter().map(|(center, source, target)| {
                record(rotation(&fixture, center, source, target, 90.0))
            }));
        }
        records.extend([
            record(GeometricConstraintKindV1::PointOnLine {
                vertex: fixture.vertices[2],
                line_edge: fixture.edges[5],
            }),
            record(GeometricConstraintKindV1::PointOnLine {
                vertex: fixture.vertices[2],
                line_edge: fixture.edges[0],
            }),
        ]);
        let raw = document(records);
        let prepared = prepare(&fixture, &raw).expect("large duplicate bucket prepares");
        begin_point_line_join_visit_count();
        let preflight = prepared.preflight();
        assert_eq!(
            finish_point_line_join_visit_count(),
            2,
            "each indexed (point, edge) bucket is joined once, regardless of rotation count"
        );
        assert_eq!(
            collinear_rotation_conflicts(&preflight).len(),
            roles.len(),
            "all distinct role keys reuse the two prejoined buckets"
        );
    }

    #[test]
    fn collinear_rotation_conflict_serializes_and_has_the_new_final_sort_rank() {
        let fixture = Fixture::new();
        let raw = document(collinear_rotation_witness_records(&fixture, true, 90.0));
        let conflict = only_collinear_rotation_conflict(&fixture, &raw)
            .expect("collinear-rotation witness exists");
        let value = serde_json::to_value(&conflict).expect("serialize collinear rotation conflict");
        assert_eq!(
            value["conflict"]["kind"],
            "rotational_symmetry_with_collinear_radius"
        );
        assert_eq!(
            value["conflict"]["line_edge"],
            serde_json::to_value(fixture.edges[5]).expect("serialize radius edge")
        );
        assert_eq!(conflict_sort_key(conflict.conflict()).0, 20);
        assert_eq!(value["constraint_ids"].as_array().unwrap().len(), 2);
    }

    fn mirror_axis_witness_records(
        fixture: &Fixture,
        point_vertex: usize,
    ) -> [GeometricConstraintRecordV1; 3] {
        [
            record(GeometricConstraintKindV1::MirrorSymmetry {
                first_vertex: fixture.vertices[1],
                second_vertex: fixture.vertices[5],
                axis_edge: fixture.edges[1],
            }),
            record(GeometricConstraintKindV1::PointOnLine {
                vertex: fixture.vertices[point_vertex],
                line_edge: fixture.edges[1],
            }),
            record(fixed_length(fixture, 5, f64::MIN_POSITIVE)),
        ]
    }

    #[test]
    fn mirrored_point_on_the_same_axis_conflicts_with_positive_fixed_separation() {
        for point_vertex in [1, 5] {
            let fixture = Fixture::new();
            let records = mirror_axis_witness_records(&fixture, point_vertex);
            let raw = document(records.clone());
            let conflict = only_mirror_axis_conflict(&fixture, &raw)
                .expect("either mirrored member on the exact axis forces collapse");
            let (first_vertex, second_vertex) =
                if fixture.vertices[1].canonical_bytes() < fixture.vertices[5].canonical_bytes() {
                    (fixture.vertices[1], fixture.vertices[5])
                } else {
                    (fixture.vertices[5], fixture.vertices[1])
                };
            assert_eq!(
                *conflict.conflict(),
                DirectConstraintConflictKindV1::MirrorSymmetryWithPointOnAxisAndFixedSeparation {
                    first_vertex,
                    second_vertex,
                    axis_edge: fixture.edges[1],
                    fixed_separation_edge: fixture.edges[5],
                }
            );
            assert_eq!(
                conflict.constraint_ids(),
                sorted_ids(&records.map(|record| record.id))
            );
            let prepared = prepare(&fixture, &raw).expect("the exact witness prepares");
            assert_bounded_direct_oracle_unknown(&prepared);
        }
    }

    #[test]
    fn mirror_axis_conflict_requires_exact_axis_vertex_pair_and_pattern_edge() {
        let fixture = Fixture::new();
        let negative_documents = [
            document([
                record(GeometricConstraintKindV1::MirrorSymmetry {
                    first_vertex: fixture.vertices[1],
                    second_vertex: fixture.vertices[5],
                    axis_edge: fixture.edges[1],
                }),
                record(GeometricConstraintKindV1::PointOnLine {
                    vertex: fixture.vertices[1],
                    line_edge: fixture.edges[2],
                }),
                record(fixed_length(&fixture, 5, 5.0)),
            ]),
            document([
                record(GeometricConstraintKindV1::MirrorSymmetry {
                    first_vertex: fixture.vertices[1],
                    second_vertex: fixture.vertices[5],
                    axis_edge: fixture.edges[1],
                }),
                record(GeometricConstraintKindV1::PointOnLine {
                    vertex: fixture.vertices[6],
                    line_edge: fixture.edges[1],
                }),
                record(fixed_length(&fixture, 5, 5.0)),
            ]),
            document([
                record(GeometricConstraintKindV1::MirrorSymmetry {
                    first_vertex: fixture.vertices[1],
                    second_vertex: fixture.vertices[5],
                    axis_edge: fixture.edges[1],
                }),
                record(GeometricConstraintKindV1::PointOnLine {
                    vertex: fixture.vertices[1],
                    line_edge: fixture.edges[1],
                }),
                record(fixed_length(&fixture, 4, 5.0)),
            ]),
            document([
                record(GeometricConstraintKindV1::MirrorSymmetry {
                    first_vertex: fixture.vertices[1],
                    second_vertex: fixture.vertices[5],
                    axis_edge: fixture.edges[1],
                }),
                record(GeometricConstraintKindV1::PointOnLine {
                    vertex: fixture.vertices[1],
                    line_edge: fixture.edges[1],
                }),
                record(GeometricConstraintKindV1::Horizontal {
                    edge: fixture.edges[5],
                }),
            ]),
        ];
        for raw in negative_documents {
            let prepared = prepare(&fixture, &raw).expect("exact negative fixture prepares");
            assert!(
                mirror_axis_conflicts(&prepared.preflight()).is_empty(),
                "different axes, outside vertices, unrelated edges, and missing fixed lengths stay unknown"
            );
        }
    }

    #[test]
    fn mirror_axis_conflict_never_uses_initial_collinearity_or_approximate_lengths() {
        let fixture = Fixture::new();
        let initially_on_axis = fixture.pattern.vertices[1].position;
        let axis_start = fixture.pattern.vertices[0].position;
        let axis_end = fixture.pattern.vertices[3].position;
        assert_eq!(
            (initially_on_axis.y - axis_start.y) * (axis_end.x - axis_start.x),
            (initially_on_axis.x - axis_start.x) * (axis_end.y - axis_start.y)
        );

        let no_point_constraint = document([
            record(GeometricConstraintKindV1::MirrorSymmetry {
                first_vertex: fixture.vertices[1],
                second_vertex: fixture.vertices[5],
                axis_edge: fixture.edges[2],
            }),
            record(fixed_length(&fixture, 5, 5.0)),
        ]);
        let prepared = prepare(&fixture, &no_point_constraint)
            .expect("initial collinearity is valid geometry");
        assert!(mirror_axis_conflicts(&prepared.preflight()).is_empty());

        let raw = document([
            record(GeometricConstraintKindV1::MirrorSymmetry {
                first_vertex: fixture.vertices[1],
                second_vertex: fixture.vertices[5],
                axis_edge: fixture.edges[1],
            }),
            record(GeometricConstraintKindV1::PointOnLine {
                vertex: fixture.vertices[1],
                line_edge: fixture.edges[1],
            }),
            record(fixed_length(&fixture, 5, 5.0)),
            record(fixed_length(&fixture, 5, 5.0_f64.next_up())),
        ]);
        let preflight = prepare(&fixture, &raw)
            .expect("adjacent positive binary64 lengths prepare")
            .preflight();
        assert!(mirror_axis_conflicts(&preflight).is_empty());
        let ConstraintPreflightV1::DirectConflict { conflicts } = preflight else {
            panic!("bit-distinct fixed lengths retain their primary conflict");
        };
        assert!(conflicts.iter().all(|conflict| matches!(
            conflict.conflict(),
            DirectConstraintConflictKindV1::DifferentFixedLengths { .. }
        )));
    }

    #[test]
    fn mirror_axis_candidate_core_is_canonical_irredundant_and_order_independent() {
        let fixture = Fixture::new();
        let first_mirror = record(GeometricConstraintKindV1::MirrorSymmetry {
            first_vertex: fixture.vertices[5],
            second_vertex: fixture.vertices[1],
            axis_edge: fixture.edges[1],
        });
        let second_mirror = record(GeometricConstraintKindV1::MirrorSymmetry {
            first_vertex: fixture.vertices[1],
            second_vertex: fixture.vertices[5],
            axis_edge: fixture.edges[1],
        });
        let first_point = record(GeometricConstraintKindV1::PointOnLine {
            vertex: fixture.vertices[1],
            line_edge: fixture.edges[1],
        });
        let second_point = record(GeometricConstraintKindV1::PointOnLine {
            vertex: fixture.vertices[5],
            line_edge: fixture.edges[1],
        });
        let first_fixed = record(fixed_length(&fixture, 5, 5.0));
        let second_fixed = record(fixed_length(&fixture, 5, 5.0));
        let records = vec![
            first_mirror.clone(),
            second_mirror.clone(),
            first_point.clone(),
            second_point.clone(),
            first_fixed.clone(),
            second_fixed.clone(),
        ];
        let expected_ids = [
            [first_mirror.id, second_mirror.id]
                .into_iter()
                .min_by_key(ConstraintId::canonical_bytes)
                .expect("mirror minimum"),
            [first_point.id, second_point.id]
                .into_iter()
                .min_by_key(ConstraintId::canonical_bytes)
                .expect("point minimum"),
            [first_fixed.id, second_fixed.id]
                .into_iter()
                .min_by_key(ConstraintId::canonical_bytes)
                .expect("fixed minimum"),
        ];
        let forward = document(records.clone());
        let forward_conflict =
            only_mirror_axis_conflict(&fixture, &forward).expect("canonical witness exists");
        assert_eq!(forward_conflict.constraint_ids(), sorted_ids(&expected_ids));

        let mut reversed_records = records;
        reversed_records.reverse();
        let reversed = document(reversed_records);
        let mut reversed_pattern = fixture.pattern.clone();
        reversed_pattern.edges.reverse();
        let reversed_preflight = prepare_geometric_constraints_v1(
            &reversed_pattern,
            &reversed,
            GeometricConstraintLimitsV1::default(),
        )
        .expect("reversed order prepares")
        .preflight();
        assert_eq!(
            serde_json::to_value(prepare(&fixture, &forward).unwrap().preflight()).unwrap(),
            serde_json::to_value(reversed_preflight).unwrap()
        );

        let minimal_records = mirror_axis_witness_records(&fixture, 1);
        for omitted in 0..minimal_records.len() {
            let subset = document(
                minimal_records
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| *index != omitted)
                    .map(|(_, record)| record.clone()),
            );
            assert!(
                mirror_axis_conflicts(&prepare(&fixture, &subset).unwrap().preflight()).is_empty()
            );
        }
    }

    #[test]
    fn mirror_axis_fixed_separation_selects_the_global_canonical_real_edge() {
        let fixture = Fixture::new();
        let alternate_edge = EdgeId::new();
        let mut pattern = fixture.pattern.clone();
        pattern.edges.push(Edge {
            id: alternate_edge,
            start: fixture.vertices[5],
            end: fixture.vertices[1],
            kind: EdgeKind::Auxiliary,
        });
        let mirror = record(GeometricConstraintKindV1::MirrorSymmetry {
            first_vertex: fixture.vertices[1],
            second_vertex: fixture.vertices[5],
            axis_edge: fixture.edges[1],
        });
        let point = record(GeometricConstraintKindV1::PointOnLine {
            vertex: fixture.vertices[1],
            line_edge: fixture.edges[1],
        });
        let first_fixed = record(fixed_length(&fixture, 5, 5.0));
        let second_fixed = record(GeometricConstraintKindV1::FixedLength {
            edge: alternate_edge,
            length_mm: 7.0,
        });
        let expected = [
            (first_fixed.id, fixture.edges[5]),
            (second_fixed.id, alternate_edge),
        ]
        .into_iter()
        .min_by_key(|(id, edge)| (id.canonical_bytes(), edge.canonical_bytes()))
        .expect("two fixed-separation candidates have a minimum");
        let records = vec![mirror, point, first_fixed, second_fixed];
        let forward = document(records.clone());
        let forward_preflight = prepare_geometric_constraints_v1(
            &pattern,
            &forward,
            GeometricConstraintLimitsV1::default(),
        )
        .expect("duplicate real separation edges prepare")
        .preflight();
        let mut conflicts = mirror_axis_conflicts(&forward_preflight);
        assert_eq!(conflicts.len(), 1);
        let conflict = conflicts.remove(0);
        let DirectConstraintConflictKindV1::MirrorSymmetryWithPointOnAxisAndFixedSeparation {
            fixed_separation_edge,
            ..
        } = *conflict.conflict()
        else {
            panic!("the filtered conflict has the mirror-axis kind")
        };
        assert_eq!(fixed_separation_edge, expected.1);
        assert!(conflict.constraint_ids().contains(&expected.0));

        pattern.edges.reverse();
        let reversed = document(records.into_iter().rev());
        let reversed_preflight = prepare_geometric_constraints_v1(
            &pattern,
            &reversed,
            GeometricConstraintLimitsV1::default(),
        )
        .expect("reversed duplicate-edge fixture prepares")
        .preflight();
        assert_eq!(
            serde_json::to_value(forward_preflight).unwrap(),
            serde_json::to_value(reversed_preflight).unwrap()
        );
    }

    #[test]
    fn mirror_axis_conflict_serializes_and_keeps_the_new_final_sort_rank() {
        let fixture = Fixture::new();
        let raw = document(mirror_axis_witness_records(&fixture, 1));
        let conflict =
            only_mirror_axis_conflict(&fixture, &raw).expect("mirror-axis witness exists");
        let value = serde_json::to_value(&conflict).expect("serialize mirror-axis conflict");
        assert_eq!(
            value["conflict"]["kind"],
            "mirror_symmetry_with_point_on_axis_and_fixed_separation"
        );
        assert_eq!(
            value["conflict"]["fixed_separation_edge"],
            serde_json::to_value(fixture.edges[5]).expect("serialize edge ID")
        );
        assert_eq!(conflict_sort_key(conflict.conflict()).0, 19);
        assert_eq!(value["constraint_ids"].as_array().unwrap().len(), 3);
    }

    #[test]
    fn different_rotation_angles_conflict_with_a_center_source_radius() {
        let fixture = Fixture::new();
        let raw = document([
            record(rotation(&fixture, 0, 1, 2, 90.0)),
            record(rotation(&fixture, 0, 1, 2, 180.0)),
            record(fixed_length(&fixture, 0, 5.0)),
        ]);
        let conflict =
            only_rotation_conflict(&fixture, &raw).expect("a positive radius forbids the collapse");
        assert_eq!(
            *conflict.conflict(),
            DirectConstraintConflictKindV1::DifferentRotationalSymmetryAnglesWithFixedRadius {
                center_vertex: fixture.vertices[0],
                source_vertex: fixture.vertices[1],
                target_vertex: fixture.vertices[2],
                fixed_radius_edge: fixture.edges[0],
            }
        );
        assert_eq!(conflict.constraint_ids().len(), 3);
    }

    #[test]
    fn different_rotation_angles_conflict_with_a_center_target_radius() {
        let fixture = Fixture::new();
        let raw = document([
            record(rotation(&fixture, 0, 1, 2, 90.0)),
            record(rotation(&fixture, 0, 1, 2, 180.0)),
            record(fixed_length(&fixture, 1, 5.0)),
        ]);
        let conflict =
            only_rotation_conflict(&fixture, &raw).expect("either radius proves the same collapse");
        assert_eq!(
            *conflict.conflict(),
            DirectConstraintConflictKindV1::DifferentRotationalSymmetryAnglesWithFixedRadius {
                center_vertex: fixture.vertices[0],
                source_vertex: fixture.vertices[1],
                target_vertex: fixture.vertices[2],
                fixed_radius_edge: fixture.edges[1],
            }
        );
    }

    #[test]
    fn different_rotation_angles_alone_keep_the_zero_radius_escape() {
        let fixture = Fixture::new();
        let raw = document([
            record(rotation(&fixture, 0, 1, 2, 90.0)),
            record(rotation(&fixture, 0, 1, 2, 180.0)),
        ]);
        let prepared = prepare(&fixture, &raw).expect("two rotations prepare");
        assert!(matches!(
            prepared.preflight(),
            ConstraintPreflightV1::Unknown {
                reason: GeometricConstraintUnknownReasonV1::SolverRequiredConstraintKinds,
                ..
            }
        ));
    }

    #[test]
    fn distant_current_coordinates_are_never_radius_evidence() {
        let fixture = Fixture::new();
        let center = fixture.pattern.vertices[0].position;
        let source = fixture.pattern.vertices[1].position;
        assert_ne!((center.x, center.y), (source.x, source.y));
        let raw = document([
            record(rotation(&fixture, 0, 1, 2, 90.0)),
            record(rotation(&fixture, 0, 1, 2, 270.0)),
        ]);
        let prepared = prepare(&fixture, &raw).expect("distant vertices prepare");
        assert!(rotation_conflicts(&prepared.preflight()).is_empty());
    }

    #[test]
    fn identical_rotation_angles_with_a_radius_do_not_conflict() {
        let fixture = Fixture::new();
        let raw = document([
            record(rotation(&fixture, 0, 1, 2, 90.0)),
            record(rotation(&fixture, 0, 1, 2, 90.0)),
            record(fixed_length(&fixture, 0, 5.0)),
        ]);
        let prepared = prepare(&fixture, &raw).expect("equal angles prepare");
        assert!(rotation_conflicts(&prepared.preflight()).is_empty());
    }

    #[test]
    fn adjacent_binary64_rotation_angles_remain_solver_required() {
        let fixture = Fixture::new();
        let raw = document([
            record(rotation(&fixture, 0, 1, 2, 90.0)),
            record(rotation(&fixture, 0, 1, 2, 90.0_f64.next_up())),
            record(fixed_length(&fixture, 0, f64::MIN_POSITIVE)),
        ]);
        let prepared = prepare(&fixture, &raw).expect("adjacent rotations prepare");
        assert!(rotation_conflicts(&prepared.preflight()).is_empty());
        assert_solver_required(&prepared.preflight());
    }

    #[test]
    fn all_reachable_distinct_cardinal_pairs_accept_either_positive_radius() {
        let fixture = Fixture::new();
        let minimum = f64::from_bits(1);
        for (first_angle, second_angle) in [(90.0, 180.0), (90.0, 270.0), (180.0, 270.0)] {
            for radius_edge in [0, 1] {
                let records = [
                    record(rotation(&fixture, 0, 1, 2, first_angle)),
                    record(rotation(&fixture, 0, 1, 2, second_angle)),
                    record(fixed_length(&fixture, radius_edge, minimum)),
                ];
                let raw = document(records.clone());
                let prepared = prepare(&fixture, &raw).expect("cardinal rotation pair prepares");
                let conflict = only_rotation_conflict(&fixture, &raw)
                    .expect("distinct exact cardinal matrices force both radii to zero");
                assert_eq!(
                    conflict.constraint_ids(),
                    sorted_ids(&records.map(|item| item.id))
                );
                assert!(matches!(
                    find_bounded_direct_mus_v1(&prepared),
                    BoundedDirectMusV1::ProvenUnsatisfiable {
                        ref constraint_ids,
                        ..
                    } if constraint_ids == conflict.constraint_ids()
                ));
            }
        }
    }

    #[test]
    fn cardinal_one_ulp_neighbors_and_identity_underflow_remain_solver_required() {
        let fixture = Fixture::new();
        for (cardinal, adjacent) in [
            (90.0, 90.0_f64.next_down()),
            (90.0, 90.0_f64.next_up()),
            (180.0, 180.0_f64.next_down()),
            (180.0, 180.0_f64.next_up()),
            (270.0, 270.0_f64.next_down()),
            (270.0, 270.0_f64.next_up()),
            (f64::from_bits(1), f64::from_bits(2)),
        ] {
            let raw = document([
                record(rotation(&fixture, 0, 1, 2, cardinal)),
                record(rotation(&fixture, 0, 1, 2, adjacent)),
                record(fixed_length(&fixture, 0, f64::from_bits(1))),
            ]);
            let prepared = prepare(&fixture, &raw).expect("fail-closed angle pair prepares");
            let preflight = prepared.preflight();
            assert!(
                rotation_conflicts(&preflight).is_empty(),
                "{cardinal:?}/{adjacent:?} must not cross the bit-exact cardinal boundary"
            );
            assert_solver_required(&preflight);
            assert_bounded_direct_oracle_unknown(&prepared);
        }
    }

    #[test]
    fn cardinal_rotation_pair_selects_the_global_canonical_class_witness() {
        let fixture = Fixture::new();
        let rotations = [
            record(rotation(&fixture, 0, 1, 2, 90.0)),
            record(rotation(&fixture, 0, 1, 2, 180.0)),
            record(rotation(&fixture, 0, 1, 2, 270.0)),
        ];
        let fixed = record(fixed_length(&fixture, 0, 5.0));
        let mut expected_rotations = rotations.iter().map(|record| record.id).collect::<Vec<_>>();
        expected_rotations.sort_unstable_by_key(ConstraintId::canonical_bytes);
        let raw = document(
            rotations
                .iter()
                .cloned()
                .chain(std::iter::once(fixed.clone())),
        );
        let conflict = only_rotation_conflict(&fixture, &raw)
            .expect("three occupied cardinal classes have a canonical pair");
        assert!(conflict.constraint_ids().contains(&expected_rotations[0]));
        assert!(conflict.constraint_ids().contains(&expected_rotations[1]));
        assert!(!conflict.constraint_ids().contains(&expected_rotations[2]));
        assert!(conflict.constraint_ids().contains(&fixed.id));
    }

    #[test]
    fn a_fixed_length_on_an_unrelated_edge_is_not_a_radius() {
        let fixture = Fixture::new();
        let raw = document([
            record(rotation(&fixture, 0, 1, 2, 90.0)),
            record(rotation(&fixture, 0, 1, 2, 180.0)),
            record(fixed_length(&fixture, 4, 5.0)),
        ]);
        let prepared = prepare(&fixture, &raw).expect("unrelated fixed length prepares");
        assert!(rotation_conflicts(&prepared.preflight()).is_empty());
    }

    #[test]
    fn rotation_roles_must_match_exactly() {
        let fixture = Fixture::new();
        for second in [
            rotation(&fixture, 0, 2, 1, 180.0),
            rotation(&fixture, 3, 1, 2, 180.0),
            rotation(&fixture, 1, 0, 2, 180.0),
        ] {
            let raw = document([
                record(rotation(&fixture, 0, 1, 2, 90.0)),
                record(second),
                record(fixed_length(&fixture, 0, 5.0)),
            ]);
            let prepared = prepare(&fixture, &raw).expect("role permutations prepare");
            assert!(
                rotation_conflicts(&prepared.preflight()).is_empty(),
                "a different role order is a different relation"
            );
        }
    }

    #[test]
    fn rotation_conflict_is_record_and_edge_order_independent() {
        let fixture = Fixture::new();
        let forward = document([
            record(rotation(&fixture, 0, 1, 2, 90.0)),
            record(rotation(&fixture, 0, 1, 2, 180.0)),
            record(fixed_length(&fixture, 0, 5.0)),
        ]);
        let mut reversed_records = forward.constraints.clone();
        reversed_records.reverse();
        let reversed = document(reversed_records);
        let mut reversed_pattern = fixture.pattern.clone();
        reversed_pattern.edges.reverse();
        let forward_preflight = prepare(&fixture, &forward)
            .expect("forward order prepares")
            .preflight();
        let reversed_preflight = prepare_geometric_constraints_v1(
            &reversed_pattern,
            &reversed,
            GeometricConstraintLimitsV1::default(),
        )
        .expect("reversed order prepares")
        .preflight();
        assert_eq!(
            serde_json::to_value(&forward_preflight).expect("serialize forward"),
            serde_json::to_value(&reversed_preflight).expect("serialize reversed"),
        );
    }

    #[test]
    fn rotation_conflict_selects_the_global_canonical_radius_witness() {
        let fixture = Fixture::new();
        let source_radius = record(fixed_length(&fixture, 0, 5.0));
        let target_radius = record(fixed_length(&fixture, 1, 7.0));
        let (expected_id, expected_edge) =
            if source_radius.id.canonical_bytes() < target_radius.id.canonical_bytes() {
                (source_radius.id, fixture.edges[0])
            } else {
                (target_radius.id, fixture.edges[1])
            };
        let raw = document([
            record(rotation(&fixture, 0, 1, 2, 90.0)),
            record(rotation(&fixture, 0, 1, 2, 180.0)),
            source_radius.clone(),
            target_radius.clone(),
        ]);
        let conflict = only_rotation_conflict(&fixture, &raw).expect("both radii are candidates");
        let DirectConstraintConflictKindV1::DifferentRotationalSymmetryAnglesWithFixedRadius {
            fixed_radius_edge,
            ..
        } = *conflict.conflict()
        else {
            panic!("the rotation conflict kind is filtered above");
        };
        assert_eq!(fixed_radius_edge, expected_edge);
        assert!(conflict.constraint_ids().contains(&expected_id));
    }

    #[test]
    fn duplicate_equal_fixed_lengths_use_the_canonical_minimum_id() {
        let fixture = Fixture::new();
        let first = record(fixed_length(&fixture, 0, 5.0));
        let second = record(fixed_length(&fixture, 0, 5.0));
        let expected = if first.id.canonical_bytes() < second.id.canonical_bytes() {
            first.id
        } else {
            second.id
        };
        let raw = document([
            record(rotation(&fixture, 0, 1, 2, 90.0)),
            record(rotation(&fixture, 0, 1, 2, 180.0)),
            first.clone(),
            second.clone(),
        ]);
        let conflict =
            only_rotation_conflict(&fixture, &raw).expect("equal values stay consistent");
        assert!(conflict.constraint_ids().contains(&expected));
    }

    #[test]
    fn an_inconsistent_fixed_length_group_is_not_radius_evidence() {
        let fixture = Fixture::new();
        let raw = document([
            record(rotation(&fixture, 0, 1, 2, 90.0)),
            record(rotation(&fixture, 0, 1, 2, 180.0)),
            record(fixed_length(&fixture, 0, 5.0)),
            record(fixed_length(&fixture, 0, 6.0)),
        ]);
        let prepared = prepare(&fixture, &raw).expect("inconsistent lengths prepare");
        let preflight = prepared.preflight();
        assert!(rotation_conflicts(&preflight).is_empty());
        let ConstraintPreflightV1::DirectConflict { conflicts } = preflight else {
            panic!("the inconsistent fixed lengths still conflict");
        };
        assert!(conflicts.iter().all(|conflict| matches!(
            conflict.conflict(),
            DirectConstraintConflictKindV1::DifferentFixedLengths { .. }
        )));
    }

    #[test]
    fn removing_any_rotation_candidate_record_withdraws_that_candidate() {
        let fixture = Fixture::new();
        let records = [
            record(rotation(&fixture, 0, 1, 2, 90.0)),
            record(rotation(&fixture, 0, 1, 2, 180.0)),
            record(fixed_length(&fixture, 0, 5.0)),
        ];
        for omitted in 0..records.len() {
            let kept = records
                .iter()
                .enumerate()
                .filter(|(index, _)| *index != omitted)
                .map(|(_, value)| value.clone());
            let raw = document(kept);
            let prepared = prepare(&fixture, &raw).expect("each pair prepares");
            assert!(
                rotation_conflicts(&prepared.preflight()).is_empty(),
                "removing witness {omitted} must withdraw the affirmation"
            );
        }
    }

    #[test]
    fn proven_rotation_conflict_stays_stable_for_growing_documents() {
        let fixture = Fixture::new();
        let witness_records = [
            record(rotation(&fixture, 0, 1, 2, 90.0)),
            record(rotation(&fixture, 0, 1, 2, 180.0)),
            record(fixed_length(&fixture, 0, 5.0)),
        ];
        let mut expected: Option<Vec<ConstraintId>> = None;
        for total in [4_usize, 8, 16] {
            let mut records = witness_records.to_vec();
            while records.len() < total {
                records.push(record(radius_padding(&fixture)));
            }
            let raw = document(records);
            let prepared = prepare(&fixture, &raw).expect("padded documents prepare");
            let conflict = only_rotation_conflict(&fixture, &raw)
                .expect("padding never hides the rotation witness");
            assert!(matches!(
                find_bounded_direct_mus_v1(&prepared),
                BoundedDirectMusV1::ProvenUnsatisfiable {
                    ref constraint_ids,
                    ..
                } if constraint_ids == conflict.constraint_ids()
            ));
            let constraint_ids = conflict.constraint_ids().to_vec();
            assert_eq!(constraint_ids.len(), 3);
            if let Some(previous) = &expected {
                assert_eq!(&constraint_ids, previous);
            } else {
                expected = Some(constraint_ids);
            }
        }
    }

    #[test]
    fn cardinal_rotation_preflight_observer_stops_fail_closed() {
        struct Stop(GeometricConstraintPreflightObserverControlV1);
        impl GeometricConstraintPreflightObserverV1 for Stop {
            fn checkpoint(&mut self) -> GeometricConstraintPreflightObserverControlV1 {
                self.0
            }
        }

        let fixture = Fixture::new();
        let raw = document([
            record(rotation(&fixture, 0, 1, 2, 90.0)),
            record(rotation(&fixture, 0, 1, 2, 270.0)),
            record(fixed_length(&fixture, 1, f64::from_bits(1))),
        ]);
        let prepared = prepare(&fixture, &raw).expect("observer fixture prepares");
        for (control, expected) in [
            (
                GeometricConstraintPreflightObserverControlV1::Cancelled,
                GeometricConstraintUnknownReasonV1::Cancelled,
            ),
            (
                GeometricConstraintPreflightObserverControlV1::DeadlineReached,
                GeometricConstraintUnknownReasonV1::DeadlineReached,
            ),
        ] {
            assert!(matches!(
                prepared.preflight_with_observer(&mut Stop(control)),
                ConstraintPreflightV1::Unknown { reason, .. } if reason == expected
            ));
        }
        assert!(matches!(
            prepared.preflight(),
            ConstraintPreflightV1::DirectConflict { .. }
        ));
    }

    #[test]
    fn seventeen_records_keep_the_candidate_without_bounded_subset_minimization() {
        let fixture = Fixture::new();
        let mut records = vec![
            record(rotation(&fixture, 0, 1, 2, 90.0)),
            record(rotation(&fixture, 0, 1, 2, 180.0)),
            record(fixed_length(&fixture, 0, 5.0)),
        ];
        while records.len() < MAX_BOUNDED_DIRECT_MUS_CONSTRAINTS_V1 + 1 {
            records.push(record(radius_padding(&fixture)));
        }
        let raw = document(records);
        let prepared = prepare(&fixture, &raw).expect("seventeen records prepare");
        assert!(only_rotation_conflict(&fixture, &raw).is_some());
        assert_eq!(
            find_bounded_direct_mus_v1(&prepared),
            BoundedDirectMusV1::Unknown { oracle_calls: 0 }
        );
    }

    #[test]
    fn collapsing_every_role_zeroes_both_rotation_residuals() {
        use crate::constraint_solver::{
            ConstraintSolveLimitsV1, solve_geometric_constraints_with_drivers_v1,
        };
        let fixture = Fixture::new();
        let raw = document([
            record(rotation(&fixture, 0, 1, 2, 90.0)),
            record(rotation(&fixture, 0, 1, 2, 180.0)),
        ]);
        let collapsed = Point2::new(0.0, 0.0);
        let preview = solve_geometric_constraints_with_drivers_v1(
            &fixture.pattern,
            &raw,
            &[
                (fixture.vertices[0], collapsed),
                (fixture.vertices[1], collapsed),
                (fixture.vertices[2], collapsed),
            ],
            ConstraintSolveLimitsV1::default(),
        )
        .expect("the collapsed assignment satisfies both rotation angles at once");
        assert_eq!(preview.maximum_residual, 0.0);

        // The escape above is exactly why the preflight stays silent until a
        // positive radius rules the collapse out. The affirmation below is
        // never derived from these solver numbers.
        let prepared = prepare(&fixture, &raw).expect("the same document prepares");
        assert!(rotation_conflicts(&prepared.preflight()).is_empty());
    }

    #[test]
    fn the_rotation_conflict_serializes_its_kind_and_four_entities() {
        let fixture = Fixture::new();
        let raw = document([
            record(rotation(&fixture, 0, 1, 2, 90.0)),
            record(rotation(&fixture, 0, 1, 2, 180.0)),
            record(fixed_length(&fixture, 0, 5.0)),
        ]);
        let conflict = only_rotation_conflict(&fixture, &raw).expect("the witness exists");
        let value = serde_json::to_value(&conflict).expect("serialize the rotation conflict");
        assert_eq!(
            value["conflict"],
            json!({
                "kind": "different_rotational_symmetry_angles_with_fixed_radius",
                "center_vertex": fixture.vertices[0],
                "source_vertex": fixture.vertices[1],
                "target_vertex": fixture.vertices[2],
                "fixed_radius_edge": fixture.edges[0],
            })
        );
        let Value::Array(ids) = &value["constraint_ids"] else {
            panic!("the witness serializes an ID array");
        };
        assert_eq!(ids.len(), 3);
        assert_eq!(
            ids.clone(),
            serde_json::to_value(conflict.constraint_ids())
                .expect("serialize witness ids")
                .as_array()
                .expect("witness ids are an array")
                .clone()
        );
    }

    #[test]
    fn inverse_exact_cardinal_nonidentity_composition_conflicts_with_either_radius() {
        let fixture = Fixture::new();
        for radius_edge in [0, 1] {
            let forward = record(rotation(&fixture, 0, 1, 2, 90.0));
            let inverse = record(rotation(&fixture, 0, 2, 1, 180.0));
            let fixed = record(fixed_length(&fixture, radius_edge, f64::MIN_POSITIVE));
            let raw = document([forward.clone(), inverse.clone(), fixed.clone()]);
            let conflict = only_inverse_rotation_conflict(&fixture, &raw)
                .expect("a non-full-turn composition and positive radius are unsatisfiable");
            let (source_vertex, target_vertex) =
                if fixture.vertices[1].canonical_bytes() < fixture.vertices[2].canonical_bytes() {
                    (fixture.vertices[1], fixture.vertices[2])
                } else {
                    (fixture.vertices[2], fixture.vertices[1])
                };
            assert_eq!(
                *conflict.conflict(),
                DirectConstraintConflictKindV1::
                    NonComplementaryInverseRotationalSymmetryAnglesWithFixedRadius {
                        center_vertex: fixture.vertices[0],
                        source_vertex,
                        target_vertex,
                        fixed_radius_edge: fixture.edges[radius_edge],
                    }
            );
            let mut expected_ids = vec![forward.id, inverse.id, fixed.id];
            canonicalize_constraint_ids(&mut expected_ids);
            assert_eq!(conflict.constraint_ids(), expected_ids);
        }
    }

    #[test]
    fn inverse_rotation_exact_full_turn_is_not_a_direct_conflict() {
        let fixture = Fixture::new();
        for (forward, inverse) in [(90.0, 270.0), (180.0, 180.0)] {
            let raw = document([
                record(rotation(&fixture, 0, 1, 2, forward)),
                record(rotation(&fixture, 0, 2, 1, inverse)),
                record(fixed_length(&fixture, 0, 5.0)),
            ]);
            let prepared = prepare(&fixture, &raw).expect("complementary rotations prepare");
            assert!(inverse_rotation_conflicts(&prepared.preflight()).is_empty());
        }
    }

    #[test]
    fn inverse_rotation_sum_rounded_to_full_turn_is_deliberately_left_unproven() {
        let fixture = Fixture::new();
        let adjacent = 90.0_f64.next_up();
        assert_ne!(adjacent.to_bits(), 90.0_f64.to_bits());
        assert_eq!(
            (adjacent + 270.0).to_bits(),
            360.0_f64.to_bits(),
            "the exact non-360 dyadic sum is absorbed by binary64 rounding"
        );
        let raw = document([
            record(rotation(&fixture, 0, 1, 2, adjacent)),
            record(rotation(&fixture, 0, 2, 1, 270.0)),
            record(fixed_length(&fixture, 0, 5.0)),
        ]);
        let prepared = prepare(&fixture, &raw).expect("adjacent angles prepare");
        assert!(
            inverse_rotation_conflicts(&prepared.preflight()).is_empty(),
            "a rounded 360 result must fail closed even when the exact sum differs"
        );
    }

    #[test]
    fn inverse_rotation_subnormal_and_near_full_turn_remain_solver_required() {
        let fixture = Fixture::new();
        let first = f64::from_bits(1);
        let second = 360.0_f64.next_down();
        let raw = document([
            record(rotation(&fixture, 0, 1, 2, first)),
            record(rotation(&fixture, 0, 2, 1, second)),
            record(fixed_length(&fixture, 0, f64::from_bits(1))),
        ]);
        let prepared = prepare(&fixture, &raw).expect("boundary angles prepare");
        assert!(inverse_rotation_conflicts(&prepared.preflight()).is_empty());
        assert_solver_required(&prepared.preflight());
    }

    #[test]
    fn inverse_rotation_requires_radius_and_exactly_reversed_roles() {
        let fixture = Fixture::new();
        let cases = [
            document([
                record(rotation(&fixture, 0, 1, 2, 90.0)),
                record(rotation(&fixture, 0, 2, 1, 180.0)),
            ]),
            document([
                record(rotation(&fixture, 0, 1, 2, 90.0)),
                record(rotation(&fixture, 0, 2, 1, 180.0)),
                record(fixed_length(&fixture, 4, 5.0)),
            ]),
            document([
                record(rotation(&fixture, 0, 1, 2, 90.0)),
                record(rotation(&fixture, 3, 2, 1, 180.0)),
                record(fixed_length(&fixture, 0, 5.0)),
            ]),
            document([
                record(rotation(&fixture, 0, 1, 2, 90.0)),
                record(rotation(&fixture, 0, 1, 2, 180.0)),
                record(fixed_length(&fixture, 0, 5.0)),
            ]),
        ];
        for raw in cases {
            let prepared = prepare(&fixture, &raw).expect("negative case prepares");
            assert!(
                inverse_rotation_conflicts(&prepared.preflight()).is_empty(),
                "missing radius, unrelated edge, different center, and same roles must fail closed"
            );
        }
    }

    #[test]
    fn inverse_rotation_zero_radius_is_rejected_before_preflight() {
        let fixture = Fixture::new();
        let raw = document([
            record(rotation(&fixture, 0, 1, 2, 90.0)),
            record(rotation(&fixture, 0, 2, 1, 180.0)),
            record(fixed_length(&fixture, 0, 0.0)),
        ]);
        assert!(matches!(
            prepare(&fixture, &raw),
            Err(GeometricConstraintErrorV1::NonPositiveLength { .. })
        ));
    }

    #[test]
    fn duplicate_equal_radius_constraints_choose_the_canonical_inverse_witness() {
        let fixture = Fixture::new();
        let first = record(fixed_length(&fixture, 0, 5.0));
        let second = record(fixed_length(&fixture, 0, 5.0));
        let expected = [first.id, second.id]
            .into_iter()
            .min_by_key(ConstraintId::canonical_bytes)
            .expect("two radius constraints have a minimum");
        let raw = document([
            record(rotation(&fixture, 0, 1, 2, 90.0)),
            record(rotation(&fixture, 0, 2, 1, 180.0)),
            first,
            second,
        ]);
        let conflict = only_inverse_rotation_conflict(&fixture, &raw)
            .expect("equal duplicate fixed lengths remain consistent evidence");
        assert!(conflict.constraint_ids().contains(&expected));
        assert_eq!(conflict.constraint_ids().len(), 3);
    }

    #[test]
    fn contradictory_fixed_lengths_are_not_inverse_rotation_radius_evidence() {
        let fixture = Fixture::new();
        let raw = document([
            record(rotation(&fixture, 0, 1, 2, 90.0)),
            record(rotation(&fixture, 0, 2, 1, 180.0)),
            record(fixed_length(&fixture, 0, 5.0)),
            record(fixed_length(&fixture, 0, 6.0)),
        ]);
        let prepared = prepare(&fixture, &raw).expect("contradictory lengths prepare");
        let preflight = prepared.preflight();
        assert!(inverse_rotation_conflicts(&preflight).is_empty());
        let ConstraintPreflightV1::DirectConflict { conflicts } = preflight else {
            panic!("the contradictory fixed lengths still have their own conflict");
        };
        assert!(conflicts.iter().all(|conflict| matches!(
            conflict.conflict(),
            DirectConstraintConflictKindV1::DifferentFixedLengths { .. }
        )));
    }

    #[test]
    fn inverse_rotation_candidate_core_is_irredundant_and_order_independent() {
        let fixture = Fixture::new();
        let records = [
            record(rotation(&fixture, 0, 1, 2, 90.0)),
            record(rotation(&fixture, 0, 2, 1, 180.0)),
            record(fixed_length(&fixture, 0, 5.0)),
        ];
        let forward = document(records.clone());
        let mut reversed_records = records.to_vec();
        reversed_records.reverse();
        let reversed = document(reversed_records);
        assert_eq!(
            serde_json::to_value(
                prepare(&fixture, &forward)
                    .expect("forward order prepares")
                    .preflight()
            )
            .expect("serialize forward preflight"),
            serde_json::to_value(
                prepare(&fixture, &reversed)
                    .expect("reverse order prepares")
                    .preflight()
            )
            .expect("serialize reverse preflight")
        );
        for omitted in 0..records.len() {
            let raw = document(
                records
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| *index != omitted)
                    .map(|(_, value)| value.clone()),
            );
            let prepared = prepare(&fixture, &raw).expect("each pair prepares");
            assert!(inverse_rotation_conflicts(&prepared.preflight()).is_empty());
        }
    }

    #[test]
    fn inverse_rotation_conflict_serializes_its_distinct_kind() {
        let fixture = Fixture::new();
        let raw = document([
            record(rotation(&fixture, 0, 1, 2, 90.0)),
            record(rotation(&fixture, 0, 2, 1, 180.0)),
            record(fixed_length(&fixture, 0, 5.0)),
        ]);
        let conflict =
            only_inverse_rotation_conflict(&fixture, &raw).expect("inverse witness exists");
        let value = serde_json::to_value(&conflict).expect("serialize inverse conflict");
        assert_eq!(
            value["conflict"]["kind"],
            "non_complementary_inverse_rotational_symmetry_angles_with_fixed_radius"
        );
        assert_eq!(
            value["conflict"]["fixed_radius_edge"],
            serde_json::to_value(fixture.edges[0]).expect("serialize edge ID")
        );
        assert_eq!(
            value["constraint_ids"]
                .as_array()
                .expect("witness IDs are an array")
                .len(),
            3
        );
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
            (-0.0, false),
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
    fn equal_length_ratio_rounded_residual_conflict_is_irredundant() {
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
        assert!(matches!(
            prepared.preflight(),
            ConstraintPreflightV1::DirectConflict { ref conflicts }
                if conflicts.len() == 1
                    && matches!(
                        conflicts[0].conflict(),
                        DirectConstraintConflictKindV1::
                            EqualLengthWithNonUnitRatioAndFixedLength { .. }
                    )
                    && conflicts[0].constraint_ids()
                        == sorted_ids(&[fixed.id, equal.id, ratio.id])
        ));
        assert!(matches!(
            find_bounded_direct_mus_v1(&prepared),
            BoundedDirectMusV1::ProvenUnsatisfiable {
                ref constraint_ids,
                ..
            } if constraint_ids == &sorted_ids(&[fixed.id, equal.id, ratio.id])
        ));

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
                "removing any cause record must withdraw the rounded-residual proof"
            );
        }
    }

    #[test]
    fn non_reciprocal_ratio_binary64_closure_conflict_is_irredundant() {
        let fixture = Fixture::new();
        let fixed = record(GeometricConstraintKindV1::FixedLength {
            edge: fixture.edges[0],
            length_mm: 10.0,
        });
        let forward = record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: fixture.edges[0],
            denominator_edge: fixture.edges[1],
            ratio: 2.0,
        });
        let reverse = record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: fixture.edges[1],
            denominator_edge: fixture.edges[0],
            ratio: 0.25,
        });
        let records = [fixed.clone(), forward.clone(), reverse.clone()];
        let prepared = prepare(&fixture, &document(records.clone()))
            .expect("the individually valid constraints prepare");
        assert!(matches!(
            prepared.preflight(),
            ConstraintPreflightV1::DirectConflict { ref conflicts }
                if conflicts.len() == 1
                    && matches!(
                        conflicts[0].conflict(),
                        DirectConstraintConflictKindV1::
                            NonReciprocalLengthRatiosWithFixedLength { .. }
                    )
                    && conflicts[0].constraint_ids()
                        == sorted_ids(&[fixed.id, forward.id, reverse.id])
        ));
        assert!(matches!(
            find_bounded_direct_mus_v1(&prepared),
            BoundedDirectMusV1::ProvenUnsatisfiable {
                ref constraint_ids,
                ..
            } if constraint_ids == &sorted_ids(&[fixed.id, forward.id, reverse.id])
        ));

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
                "removing any cause record must withdraw the binary64 closure proof"
            );
        }
        let reciprocal = record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: fixture.edges[1],
            denominator_edge: fixture.edges[0],
            ratio: 0.5,
        });
        let prepared = prepare(&fixture, &document([fixed, forward, reciprocal]))
            .expect("reciprocal ratios prepare");
        assert!(
            !matches!(
                prepared.preflight(),
                ConstraintPreflightV1::DirectConflict { .. }
            ),
            "exact reciprocal ratios must not be reported as contradictory"
        );
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
        assert!(
            conflicts
                .iter()
                .all(|conflict| conflict.constraint_ids().len() == 3)
        );
        assert_eq!(
            conflicts
                .iter()
                .map(|conflict| conflict.constraint_ids().len())
                .sum::<usize>(),
            3 * PAIR_COUNT
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
        assert_eq!(conflicts.len(), 2);
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
        assert!(conflicts.iter().all(is_proven_direct_conflict_v1));
    }

    #[test]
    fn horizontal_and_vertical_require_an_exact_noncollapse_witness() {
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
            ConstraintPreflightV1::Unknown {
                reason: GeometricConstraintUnknownReasonV1::SolverRequiredConstraintKinds,
                unchecked_constraint_ids: sorted_ids(&[horizontal.id, vertical.id]),
            }
        );

        let fixed = record(GeometricConstraintKindV1::FixedLength {
            edge: fixture.edges[0],
            length_mm: 1.0,
        });
        let prepared = prepare(
            &fixture,
            &document([vertical.clone(), fixed.clone(), horizontal.clone()]),
        )
        .expect("positive fixed length excludes the zero-length escape");
        assert_eq!(
            prepared.preflight(),
            ConstraintPreflightV1::DirectConflict {
                conflicts: vec![DirectConstraintConflictV1 {
                    conflict: DirectConstraintConflictKindV1::HorizontalAndVertical {
                        edge: fixture.edges[0],
                    },
                    constraint_ids: sorted_ids(&[horizontal.id, vertical.id, fixed.id]),
                }],
            }
        );
    }

    #[test]
    fn horizontal_and_vertical_use_normalized_edge_constraints_as_noncollapse_witnesses() {
        let fixture = Fixture::new();
        let providers = [
            (
                "point-on-line",
                GeometricConstraintKindV1::PointOnLine {
                    vertex: fixture.vertices[2],
                    line_edge: fixture.edges[0],
                },
            ),
            (
                "mirror axis",
                GeometricConstraintKindV1::MirrorSymmetry {
                    first_vertex: fixture.vertices[2],
                    second_vertex: fixture.vertices[4],
                    axis_edge: fixture.edges[0],
                },
            ),
            (
                "angle-bisector arm",
                GeometricConstraintKindV1::AngleBisector {
                    vertex: fixture.vertices[0],
                    first_edge: fixture.edges[0],
                    second_edge: fixture.edges[1],
                    bisector_edge: fixture.edges[2],
                },
            ),
        ];

        for (description, provider_kind) in providers {
            let horizontal = record(GeometricConstraintKindV1::Horizontal {
                edge: fixture.edges[0],
            });
            let vertical = record(GeometricConstraintKindV1::Vertical {
                edge: fixture.edges[0],
            });
            let provider = record(provider_kind);
            let records = vec![vertical.clone(), provider.clone(), horizontal.clone()];
            let prepared = prepare(&fixture, &document(records.clone()))
                .unwrap_or_else(|error| panic!("{description} witness must prepare: {error:?}"));
            assert_eq!(
                prepared.preflight(),
                ConstraintPreflightV1::DirectConflict {
                    conflicts: vec![DirectConstraintConflictV1 {
                        conflict: DirectConstraintConflictKindV1::HorizontalAndVertical {
                            edge: fixture.edges[0],
                        },
                        constraint_ids: sorted_ids(&[horizontal.id, vertical.id, provider.id,]),
                    }],
                },
                "{description}"
            );

            let BoundedDirectMusV1::ProvenUnsatisfiable {
                constraint_ids,
                oracle_calls,
            } = find_bounded_direct_mus_v1(&prepared)
            else {
                panic!("{description} must feed the bounded direct-conflict oracle")
            };
            assert_eq!(
                constraint_ids,
                sorted_ids(&[horizontal.id, vertical.id, provider.id]),
                "{description}"
            );
            assert_eq!(oracle_calls, 7, "{description}");

            for removed in [horizontal.id, vertical.id, provider.id] {
                let subset = records
                    .iter()
                    .filter(|record| record.id != removed)
                    .cloned()
                    .collect::<Vec<_>>();
                assert!(
                    !matches!(
                        prepare(&fixture, &document(subset))
                            .expect("proper normalized-edge witness subset")
                            .preflight(),
                        ConstraintPreflightV1::DirectConflict { .. }
                    ),
                    "{description}: deleting {removed:?} must remove the direct contradiction"
                );
            }
        }
    }

    #[test]
    fn horizontal_and_vertical_detect_every_angle_bisector_edge_role() {
        let fixture = Fixture::new();
        let roles = [
            (fixture.edges[0], fixture.edges[1], fixture.edges[2]),
            (fixture.edges[1], fixture.edges[0], fixture.edges[2]),
            (fixture.edges[1], fixture.edges[2], fixture.edges[0]),
        ];

        for (first_edge, second_edge, bisector_edge) in roles {
            let horizontal = record(GeometricConstraintKindV1::Horizontal {
                edge: fixture.edges[0],
            });
            let vertical = record(GeometricConstraintKindV1::Vertical {
                edge: fixture.edges[0],
            });
            let bisector = record(GeometricConstraintKindV1::AngleBisector {
                vertex: fixture.vertices[0],
                first_edge,
                second_edge,
                bisector_edge,
            });
            assert_eq!(
                prepare(
                    &fixture,
                    &document([bisector.clone(), horizontal.clone(), vertical.clone()]),
                )
                .expect("every angle-bisector role is locally valid")
                .preflight(),
                ConstraintPreflightV1::DirectConflict {
                    conflicts: vec![DirectConstraintConflictV1 {
                        conflict: DirectConstraintConflictKindV1::HorizontalAndVertical {
                            edge: fixture.edges[0],
                        },
                        constraint_ids: sorted_ids(&[horizontal.id, vertical.id, bisector.id,]),
                    }],
                }
            );
        }
    }

    #[test]
    fn horizontal_and_vertical_select_the_canonical_noncollapse_witness() {
        let fixture = Fixture::new();
        let first_horizontal = record(GeometricConstraintKindV1::Horizontal {
            edge: fixture.edges[0],
        });
        let second_horizontal = record(GeometricConstraintKindV1::Horizontal {
            edge: fixture.edges[0],
        });
        let first_vertical = record(GeometricConstraintKindV1::Vertical {
            edge: fixture.edges[0],
        });
        let second_vertical = record(GeometricConstraintKindV1::Vertical {
            edge: fixture.edges[0],
        });
        let fixed = record(fixed_length(&fixture, 0, 1.0));
        let point = record(GeometricConstraintKindV1::PointOnLine {
            vertex: fixture.vertices[2],
            line_edge: fixture.edges[0],
        });
        let mirror = record(GeometricConstraintKindV1::MirrorSymmetry {
            first_vertex: fixture.vertices[2],
            second_vertex: fixture.vertices[4],
            axis_edge: fixture.edges[0],
        });
        let bisector = record(GeometricConstraintKindV1::AngleBisector {
            vertex: fixture.vertices[0],
            first_edge: fixture.edges[0],
            second_edge: fixture.edges[1],
            bisector_edge: fixture.edges[2],
        });
        let expected_horizontal = [first_horizontal.id, second_horizontal.id]
            .into_iter()
            .min_by_key(ConstraintId::canonical_bytes)
            .unwrap();
        let expected_vertical = [first_vertical.id, second_vertical.id]
            .into_iter()
            .min_by_key(ConstraintId::canonical_bytes)
            .unwrap();
        let expected_provider = [fixed.id, point.id, mirror.id, bisector.id]
            .into_iter()
            .min_by_key(ConstraintId::canonical_bytes)
            .unwrap();
        let expected = ConstraintPreflightV1::DirectConflict {
            conflicts: vec![DirectConstraintConflictV1 {
                conflict: DirectConstraintConflictKindV1::HorizontalAndVertical {
                    edge: fixture.edges[0],
                },
                constraint_ids: sorted_ids(&[
                    expected_horizontal,
                    expected_vertical,
                    expected_provider,
                ]),
            }],
        };
        let mut records = vec![
            first_horizontal,
            second_vertical,
            fixed,
            point,
            second_horizontal,
            first_vertical,
            mirror,
            bisector,
        ];
        let forward = prepare(&fixture, &document(records.clone()))
            .expect("duplicate canonical witnesses prepare")
            .preflight();
        records.reverse();
        let reverse = prepare(&fixture, &document(records))
            .expect("source-reversed canonical witnesses prepare")
            .preflight();
        assert_eq!(forward, expected);
        assert_eq!(reverse, expected);
    }

    #[test]
    fn horizontal_and_vertical_noncollapse_witness_requires_the_same_exact_edge() {
        let fixture = Fixture::new();
        let providers = [
            GeometricConstraintKindV1::PointOnLine {
                vertex: fixture.vertices[2],
                line_edge: fixture.edges[5],
            },
            GeometricConstraintKindV1::MirrorSymmetry {
                first_vertex: fixture.vertices[2],
                second_vertex: fixture.vertices[4],
                axis_edge: fixture.edges[4],
            },
            GeometricConstraintKindV1::AngleBisector {
                vertex: fixture.vertices[0],
                first_edge: fixture.edges[1],
                second_edge: fixture.edges[2],
                bisector_edge: fixture.edges[3],
            },
        ];

        for provider_kind in providers {
            let horizontal = record(GeometricConstraintKindV1::Horizontal {
                edge: fixture.edges[0],
            });
            let vertical = record(GeometricConstraintKindV1::Vertical {
                edge: fixture.edges[0],
            });
            let provider = record(provider_kind);
            assert_eq!(
                prepare(
                    &fixture,
                    &document([horizontal.clone(), vertical.clone(), provider.clone()]),
                )
                .expect("nonmatching exact edge witness prepares")
                .preflight(),
                ConstraintPreflightV1::Unknown {
                    reason: GeometricConstraintUnknownReasonV1::SolverRequiredConstraintKinds,
                    unchecked_constraint_ids: sorted_ids(&[
                        horizontal.id,
                        vertical.id,
                        provider.id,
                    ]),
                }
            );
        }
    }

    #[test]
    fn normalized_edge_witness_precedes_general_parallel_for_horizontal_and_vertical() {
        let fixture = Fixture::new();
        let horizontal = record(GeometricConstraintKindV1::Horizontal {
            edge: fixture.edges[0],
        });
        let vertical = record(GeometricConstraintKindV1::Vertical {
            edge: fixture.edges[0],
        });
        let point = record(GeometricConstraintKindV1::PointOnLine {
            vertex: fixture.vertices[2],
            line_edge: fixture.edges[0],
        });
        let parallel = record(GeometricConstraintKindV1::Parallel {
            first_edge: fixture.edges[0],
            second_edge: fixture.edges[4],
        });

        assert_eq!(
            prepare(
                &fixture,
                &document([
                    parallel.clone(),
                    vertical.clone(),
                    point.clone(),
                    horizontal.clone(),
                ]),
            )
            .expect("fixed normalized-edge witness with incident parallel")
            .preflight(),
            ConstraintPreflightV1::DirectConflict {
                conflicts: vec![DirectConstraintConflictV1 {
                    conflict: DirectConstraintConflictKindV1::HorizontalAndVertical {
                        edge: fixture.edges[0],
                    },
                    constraint_ids: sorted_ids(&[horizontal.id, vertical.id, point.id]),
                }],
            }
        );
        let without_point_records = vec![parallel.clone(), vertical.clone(), horizontal.clone()];
        assert_same_edge_parallel_zero_closure_is_exact_and_minimal(
            &fixture,
            &without_point_records,
            fixture.edges[0],
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
        assert!(conflicts.iter().all(is_proven_direct_conflict_v1));
    }

    #[test]
    fn proven_direct_conflict_oracle_cores_are_canonical_and_irredundant() {
        let fixture = Fixture::new();
        let cases = [
            vec![
                record(GeometricConstraintKindV1::FixedLength {
                    edge: fixture.edges[0],
                    length_mm: 1.0,
                }),
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
            vec![
                record(GeometricConstraintKindV1::FixedLength {
                    edge: fixture.edges[0],
                    length_mm: 1.0,
                }),
                record(GeometricConstraintKindV1::FixedLength {
                    edge: fixture.edges[1],
                    length_mm: 1.0,
                }),
                record(GeometricConstraintKindV1::LengthRatio {
                    numerator_edge: fixture.edges[0],
                    denominator_edge: fixture.edges[1],
                    ratio: 2.0,
                }),
            ],
            vec![
                record(GeometricConstraintKindV1::FixedLength {
                    edge: fixture.edges[0],
                    length_mm: 1.0,
                }),
                record(GeometricConstraintKindV1::LengthRatio {
                    numerator_edge: fixture.edges[0],
                    denominator_edge: fixture.edges[1],
                    ratio: 2.0,
                }),
                record(GeometricConstraintKindV1::LengthRatio {
                    numerator_edge: fixture.edges[1],
                    denominator_edge: fixture.edges[2],
                    ratio: 3.0,
                }),
                record(GeometricConstraintKindV1::LengthRatio {
                    numerator_edge: fixture.edges[2],
                    denominator_edge: fixture.edges[0],
                    ratio: 0.25,
                }),
            ],
        ];

        for records in cases {
            let prepared = prepare(&fixture, &document(records.clone())).expect("valid cause");
            let ConstraintPreflightV1::DirectConflict { conflicts } = prepared.preflight() else {
                panic!("the allowlisted direct witness must prove a conflict");
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
    fn fixed_lengths_and_ratio_share_the_solver_binary64_residual() {
        let minimum = f64::from_bits(1);
        let one_up = 1.0_f64.next_up();
        assert_eq!(length_ratio_residual_binary64_v1(6.0, 2.0, 3.0), 0.0);
        assert_eq!(
            length_ratio_residual_binary64_v1(minimum, one_up, minimum),
            0.0,
            "a real-product mismatch can disappear in the implemented rounded multiplication"
        );
        assert_ne!(length_ratio_residual_binary64_v1(0.3, 3.0, 0.1), 0.0);
        assert_ne!(
            length_ratio_residual_binary64_v1(minimum, 0.5, minimum),
            0.0,
            "underflow to zero cannot satisfy a positive fixed numerator"
        );
        assert!(
            !length_ratio_residual_binary64_v1(f64::MAX, 2.0, f64::MAX).is_finite(),
            "overflow is rejected by the numerical residual boundary"
        );

        let fixture = Fixture::new();
        let ratio = record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: fixture.edges[0],
            denominator_edge: fixture.edges[1],
            ratio: 2.0,
        });
        let prepared = prepare(
            &fixture,
            &document([
                record(GeometricConstraintKindV1::FixedLength {
                    edge: fixture.edges[0],
                    length_mm: 6.0,
                }),
                record(GeometricConstraintKindV1::FixedLength {
                    edge: fixture.edges[1],
                    length_mm: 3.0,
                }),
                ratio.clone(),
            ]),
        )
        .expect("exactly compatible fixed lengths and ratio");
        assert_eq!(
            prepared.preflight(),
            ConstraintPreflightV1::Unknown {
                reason: GeometricConstraintUnknownReasonV1::SolverRequiredConstraintKinds,
                unchecked_constraint_ids: vec![ratio.id],
            }
        );

        let rounded_ratio = record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: fixture.edges[0],
            denominator_edge: fixture.edges[1],
            ratio: one_up,
        });
        let rounded_compatible = prepare(
            &fixture,
            &document([
                record(GeometricConstraintKindV1::FixedLength {
                    edge: fixture.edges[0],
                    length_mm: minimum,
                }),
                record(GeometricConstraintKindV1::FixedLength {
                    edge: fixture.edges[1],
                    length_mm: minimum,
                }),
                rounded_ratio.clone(),
            ]),
        )
        .expect("rounded-compatible fixed lengths and ratio");
        assert_eq!(
            rounded_compatible.preflight(),
            ConstraintPreflightV1::Unknown {
                reason: GeometricConstraintUnknownReasonV1::SolverRequiredConstraintKinds,
                unchecked_constraint_ids: vec![rounded_ratio.id],
            },
            "zero in the shared residual must never become a direct contradiction"
        );
        assert_bounded_direct_oracle_unknown(&rounded_compatible);

        let numerator = record(GeometricConstraintKindV1::FixedLength {
            edge: fixture.edges[0],
            length_mm: 0.3,
        });
        let denominator = record(GeometricConstraintKindV1::FixedLength {
            edge: fixture.edges[1],
            length_mm: 0.1,
        });
        let incompatible_ratio = record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: fixture.edges[0],
            denominator_edge: fixture.edges[1],
            ratio: 3.0,
        });
        let prepared = prepare(
            &fixture,
            &document([
                numerator.clone(),
                denominator.clone(),
                incompatible_ratio.clone(),
            ]),
        )
        .expect("binary64-incompatible fixed lengths and ratio");
        let expected = ConstraintPreflightV1::DirectConflict {
            conflicts: vec![DirectConstraintConflictV1 {
                conflict: DirectConstraintConflictKindV1::LengthRatioWithIncompatibleFixedLengths {
                    numerator_edge: fixture.edges[0],
                    denominator_edge: fixture.edges[1],
                },
                constraint_ids: sorted_ids(&[numerator.id, denominator.id, incompatible_ratio.id]),
            }],
        };
        assert_eq!(prepared.preflight(), expected);
        let BoundedDirectMusV1::ProvenUnsatisfiable {
            constraint_ids,
            oracle_calls,
        } = find_bounded_direct_mus_v1(&prepared)
        else {
            panic!("the shared non-zero residual must prove the three-record direct cause")
        };
        assert_eq!(
            constraint_ids,
            sorted_ids(&[numerator.id, denominator.id, incompatible_ratio.id])
        );
        assert_eq!(oracle_calls, 7);

        let records = vec![
            numerator.clone(),
            denominator.clone(),
            incompatible_ratio.clone(),
        ];
        for removed in records.iter().map(|record| record.id) {
            let subset = records
                .iter()
                .filter(|record| record.id != removed)
                .cloned()
                .collect::<Vec<_>>();
            assert!(
                !matches!(
                    prepare(&fixture, &document(subset))
                        .expect("proper rounded-residual witness subset")
                        .preflight(),
                    ConstraintPreflightV1::DirectConflict { .. }
                ),
                "deleting {removed:?} must remove the direct contradiction"
            );
        }

        let mut reversed = records;
        reversed.reverse();
        assert_eq!(
            prepare(&fixture, &document(reversed))
                .expect("source-order reversed direct cause")
                .preflight(),
            expected
        );

        for (label, numerator_length, ratio, denominator_length) in [
            ("underflow", minimum, 0.5, minimum),
            ("overflow", f64::MAX, 2.0, f64::MAX),
        ] {
            let prepared = prepare(
                &fixture,
                &document([
                    record(GeometricConstraintKindV1::FixedLength {
                        edge: fixture.edges[0],
                        length_mm: numerator_length,
                    }),
                    record(GeometricConstraintKindV1::FixedLength {
                        edge: fixture.edges[1],
                        length_mm: denominator_length,
                    }),
                    record(GeometricConstraintKindV1::LengthRatio {
                        numerator_edge: fixture.edges[0],
                        denominator_edge: fixture.edges[1],
                        ratio,
                    }),
                ]),
            )
            .unwrap_or_else(|error| panic!("{label} boundary must prepare: {error:?}"));
            assert!(
                matches!(
                    prepared.preflight(),
                    ConstraintPreflightV1::DirectConflict {
                        ref conflicts
                    } if conflicts.len() == 1
                        && matches!(
                            conflicts[0].conflict(),
                            DirectConstraintConflictKindV1::
                                LengthRatioWithIncompatibleFixedLengths { .. }
                        )
                ),
                "{label}: the shared residual boundary must prove the contradiction"
            );
        }
    }

    #[test]
    fn different_ratios_need_a_fixed_denominator_and_incompatible_binary64_products() {
        let fixture = Fixture::new();
        let numerator_edge = fixture.edges[0];
        let denominator_edge = fixture.edges[1];
        let fixed = record(GeometricConstraintKindV1::FixedLength {
            edge: denominator_edge,
            length_mm: 1.0,
        });
        let first_ratio = record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge,
            denominator_edge,
            ratio: 2.0,
        });
        let second_ratio = record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge,
            denominator_edge,
            ratio: 3.0,
        });
        let records = vec![fixed.clone(), first_ratio.clone(), second_ratio.clone()];
        let expected = ConstraintPreflightV1::DirectConflict {
            conflicts: vec![DirectConstraintConflictV1 {
                conflict: DirectConstraintConflictKindV1::DifferentLengthRatios {
                    numerator_edge,
                    denominator_edge,
                },
                constraint_ids: sorted_ids(&[fixed.id, first_ratio.id, second_ratio.id]),
            }],
        };
        let prepared = prepare(&fixture, &document(records.clone()))
            .expect("two incompatible ratio products and a fixed denominator prepare");
        assert_eq!(prepared.preflight(), expected);
        let BoundedDirectMusV1::ProvenUnsatisfiable {
            constraint_ids,
            oracle_calls,
        } = find_bounded_direct_mus_v1(&prepared)
        else {
            panic!("the three-record rounded-product contradiction must feed the bounded oracle")
        };
        assert_eq!(
            constraint_ids,
            sorted_ids(&[fixed.id, first_ratio.id, second_ratio.id])
        );
        assert_eq!(oracle_calls, 7);

        for removed in records.iter().map(|record| record.id) {
            let subset = records
                .iter()
                .filter(|record| record.id != removed)
                .cloned()
                .collect::<Vec<_>>();
            assert!(
                !matches!(
                    prepare(&fixture, &document(subset))
                        .expect("proper product-conflict subset prepares")
                        .preflight(),
                    ConstraintPreflightV1::DirectConflict { .. }
                ),
                "deleting {removed:?} must remove the three-record proof"
            );
        }

        let mut reversed = records;
        reversed.reverse();
        assert_eq!(
            prepare(&fixture, &document(reversed))
                .expect("source-order reversal prepares")
                .preflight(),
            expected,
            "canonical IDs, not source order, select the witness"
        );

        let without_fixed = prepare(
            &fixture,
            &document([first_ratio.clone(), second_ratio.clone()]),
        )
        .expect("the unsafe two-ratio counterexample prepares");
        assert_eq!(
            without_fixed.preflight(),
            ConstraintPreflightV1::Unknown {
                reason: GeometricConstraintUnknownReasonV1::SolverRequiredConstraintKinds,
                unchecked_constraint_ids: sorted_ids(&[first_ratio.id, second_ratio.id]),
            }
        );
        assert!(
            last_quarantined_direct_conflicts()
                .iter()
                .all(|candidate| !matches!(
                    candidate.conflict(),
                    DirectConstraintConflictKindV1::DifferentLengthRatios { .. }
                )),
            "an unsafe two-ID ratio pair must remain unchecked without becoming a candidate"
        );
        assert_bounded_direct_oracle_unknown(&without_fixed);

        let duplicate_ratio = record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge,
            denominator_edge,
            ratio: 2.0,
        });
        let duplicate_only = prepare(
            &fixture,
            &document([fixed.clone(), first_ratio.clone(), duplicate_ratio.clone()]),
        )
        .expect("bit-identical duplicate ratios prepare");
        assert_eq!(
            duplicate_only.preflight(),
            ConstraintPreflightV1::Unknown {
                reason: GeometricConstraintUnknownReasonV1::SolverRequiredConstraintKinds,
                unchecked_constraint_ids: sorted_ids(&[first_ratio.id, duplicate_ratio.id]),
            },
            "duplicate values never establish a contradiction"
        );

        let duplicate_fixed = record(GeometricConstraintKindV1::FixedLength {
            edge: denominator_edge,
            length_mm: 1.0,
        });
        let canonical_fixed = [fixed.id, duplicate_fixed.id]
            .into_iter()
            .min_by_key(ConstraintId::canonical_bytes)
            .expect("two fixed denominator IDs have a minimum");
        let duplicate_fixed_group = prepare(
            &fixture,
            &document([
                duplicate_fixed,
                second_ratio.clone(),
                fixed.clone(),
                first_ratio.clone(),
            ]),
        )
        .expect("a consistent duplicate fixed-denominator group prepares");
        assert_eq!(
            duplicate_fixed_group.preflight(),
            ConstraintPreflightV1::DirectConflict {
                conflicts: vec![DirectConstraintConflictV1 {
                    conflict: DirectConstraintConflictKindV1::DifferentLengthRatios {
                        numerator_edge,
                        denominator_edge,
                    },
                    constraint_ids: sorted_ids(
                        &[canonical_fixed, first_ratio.id, second_ratio.id,]
                    ),
                }],
            },
            "the consistent fixed group must select its canonical-smallest ID"
        );

        let conflicting_fixed = record(GeometricConstraintKindV1::FixedLength {
            edge: denominator_edge,
            length_mm: 2.0,
        });
        let inconsistent_denominator = prepare(
            &fixture,
            &document([
                fixed.clone(),
                conflicting_fixed.clone(),
                first_ratio.clone(),
                second_ratio.clone(),
            ]),
        )
        .expect("an inconsistent fixed-denominator group still prepares");
        let ConstraintPreflightV1::DirectConflict { conflicts } =
            inconsistent_denominator.preflight()
        else {
            panic!("the fixed lengths themselves must conflict")
        };
        assert_eq!(conflicts.len(), 1);
        assert!(matches!(
            conflicts[0].conflict(),
            DirectConstraintConflictKindV1::DifferentFixedLengths { edge }
                if *edge == denominator_edge
        ));
        assert_eq!(
            conflicts[0].constraint_ids(),
            sorted_ids(&[fixed.id, conflicting_fixed.id])
        );
    }

    #[test]
    fn different_ratio_products_cover_underflow_rounding_and_overflow_boundaries() {
        let minimum = f64::from_bits(1);
        let one_up = 1.0_f64.next_up();
        let cases = [
            ("ordinary different products", 1.0, 2.0, 3.0, true),
            ("zero versus subnormal", minimum, 0.5, 1.0, true),
            ("both underflow to zero", minimum, 0.25, 0.5, false),
            ("same rounded subnormal", minimum, 1.0, one_up, false),
            ("finite versus overflow", f64::MAX, 1.0, 2.0, true),
            ("both overflow", f64::MAX, 2.0, 3.0, true),
        ];

        for (label, denominator_length, first_value, second_value, proven) in cases {
            let first_product =
                length_ratio_scaled_denominator_binary64_v1(first_value, denominator_length);
            let second_product =
                length_ratio_scaled_denominator_binary64_v1(second_value, denominator_length);
            assert_eq!(
                proven,
                !first_product.is_finite()
                    || !second_product.is_finite()
                    || first_product != second_product,
                "{label}: the test matrix must match the authoritative product predicate"
            );

            let fixture = Fixture::new();
            let fixed = record(GeometricConstraintKindV1::FixedLength {
                edge: fixture.edges[1],
                length_mm: denominator_length,
            });
            let first = record(GeometricConstraintKindV1::LengthRatio {
                numerator_edge: fixture.edges[0],
                denominator_edge: fixture.edges[1],
                ratio: first_value,
            });
            let second = record(GeometricConstraintKindV1::LengthRatio {
                numerator_edge: fixture.edges[0],
                denominator_edge: fixture.edges[1],
                ratio: second_value,
            });
            let prepared = prepare(
                &fixture,
                &document([fixed.clone(), first.clone(), second.clone()]),
            )
            .unwrap_or_else(|error| panic!("{label}: valid scalar boundary failed: {error:?}"));
            if proven {
                assert!(
                    matches!(
                        prepared.preflight(),
                        ConstraintPreflightV1::DirectConflict {
                            ref conflicts
                        } if conflicts.len() == 1
                            && matches!(
                                conflicts[0].conflict(),
                                DirectConstraintConflictKindV1::DifferentLengthRatios { .. }
                            )
                            && conflicts[0].constraint_ids()
                                == sorted_ids(&[fixed.id, first.id, second.id])
                    ),
                    "{label}: incompatible products must emit the exact three-ID proof"
                );
            } else {
                assert_eq!(
                    prepared.preflight(),
                    ConstraintPreflightV1::Unknown {
                        reason: GeometricConstraintUnknownReasonV1::SolverRequiredConstraintKinds,
                        unchecked_constraint_ids: sorted_ids(&[first.id, second.id]),
                    },
                    "{label}: a common rounded numerator must stay solver-required"
                );
                assert_bounded_direct_oracle_unknown(&prepared);
            }
        }
    }

    #[test]
    fn three_ratio_cycle_uses_binary64_closure_instead_of_exact_unit_product() {
        assert_eq!(2.0 * 4.0 * 0.125, 1.0);
        assert_eq!(
            f64::from_bits(1) * f64::from_bits(0x7fe0_0000_0000_0000) * 2_f64.powi(51),
            1.0
        );
        assert_ne!(2.0 * 3.0 * 0.25, 1.0);

        let fixture = Fixture::new();
        let fixed = record(GeometricConstraintKindV1::FixedLength {
            edge: fixture.edges[0],
            length_mm: 1.0,
        });
        let first = record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: fixture.edges[0],
            denominator_edge: fixture.edges[1],
            ratio: 2.0,
        });
        let second = record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: fixture.edges[1],
            denominator_edge: fixture.edges[2],
            ratio: 3.0,
        });
        let third = record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: fixture.edges[2],
            denominator_edge: fixture.edges[0],
            ratio: 0.25,
        });
        let prepared = prepare(
            &fixture,
            &document([fixed.clone(), first.clone(), second.clone(), third.clone()]),
        )
        .expect("incompatible directed ratio cycle");
        let expected_ids = sorted_ids(&[fixed.id, first.id, second.id, third.id]);
        assert!(matches!(
            prepared.preflight(),
            ConstraintPreflightV1::DirectConflict { ref conflicts }
                if conflicts.len() == 1
                    && matches!(
                        conflicts[0].conflict(),
                        DirectConstraintConflictKindV1::
                            NonUnitLengthRatioCycleWithFixedLength { .. }
                    )
                    && conflicts[0].constraint_ids() == expected_ids
        ));
        assert!(matches!(
            find_bounded_direct_mus_v1(&prepared),
            BoundedDirectMusV1::ProvenUnsatisfiable {
                ref constraint_ids,
                ..
            } if constraint_ids == &expected_ids
        ));

        let without_fixed = prepare(&fixture, &document([first, second, third]))
            .expect("zero-length solution remains admissible")
            .preflight();
        assert!(!matches!(
            without_fixed,
            ConstraintPreflightV1::DirectConflict { .. }
        ));

        let compatible = prepare(
            &fixture,
            &document([
                record(GeometricConstraintKindV1::FixedLength {
                    edge: fixture.edges[0],
                    length_mm: 1.0,
                }),
                record(GeometricConstraintKindV1::LengthRatio {
                    numerator_edge: fixture.edges[0],
                    denominator_edge: fixture.edges[1],
                    ratio: 2.0,
                }),
                record(GeometricConstraintKindV1::LengthRatio {
                    numerator_edge: fixture.edges[1],
                    denominator_edge: fixture.edges[2],
                    ratio: 4.0,
                }),
                record(GeometricConstraintKindV1::LengthRatio {
                    numerator_edge: fixture.edges[2],
                    denominator_edge: fixture.edges[0],
                    ratio: 0.125,
                }),
            ]),
        )
        .expect("exactly reciprocal cycle")
        .preflight();
        assert!(!matches!(
            compatible,
            ConstraintPreflightV1::DirectConflict { .. }
        ));
    }

    #[test]
    fn reverse_only_exact_real_ratio_graph_remains_solver_required_and_irredundant() {
        let fixture = Fixture::new();
        let records = vec![
            record(GeometricConstraintKindV1::FixedLength {
                edge: fixture.edges[4],
                length_mm: 7.0,
            }),
            record(GeometricConstraintKindV1::LengthRatio {
                numerator_edge: fixture.edges[4],
                denominator_edge: fixture.edges[0],
                ratio: 11.0,
            }),
            record(GeometricConstraintKindV1::LengthRatio {
                numerator_edge: fixture.edges[0],
                denominator_edge: fixture.edges[1],
                ratio: 2.0,
            }),
            record(GeometricConstraintKindV1::LengthRatio {
                numerator_edge: fixture.edges[1],
                denominator_edge: fixture.edges[2],
                ratio: 3.0,
            }),
            record(GeometricConstraintKindV1::LengthRatio {
                numerator_edge: fixture.edges[2],
                denominator_edge: fixture.edges[3],
                ratio: 5.0,
            }),
            record(GeometricConstraintKindV1::LengthRatio {
                numerator_edge: fixture.edges[3],
                denominator_edge: fixture.edges[0],
                ratio: 0.1,
            }),
        ];
        let prepared = prepare(&fixture, &document(records.clone()))
            .expect("bounded inconsistent ratio graph");
        assert_solver_required(&prepared.preflight());
        assert_bounded_direct_oracle_unknown(&prepared);

        let duplicate_fixed = record(GeometricConstraintKindV1::FixedLength {
            edge: fixture.edges[4],
            length_mm: 7.0,
        });
        let duplicate_ratio = record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: fixture.edges[4],
            denominator_edge: fixture.edges[0],
            ratio: 11.0,
        });
        let mut duplicated = records.clone();
        duplicated.extend([duplicate_fixed.clone(), duplicate_ratio.clone()]);
        let forward = prepare(&fixture, &document(duplicated.clone()))
            .expect("equal duplicate assignments")
            .preflight();
        duplicated.reverse();
        let reversed = prepare(&fixture, &document(duplicated))
            .expect("source-reordered equal duplicate assignments")
            .preflight();
        assert_eq!(forward, reversed);
        assert_solver_required(&forward);

        for removed in records.iter().map(|record| record.id) {
            let subset = records
                .iter()
                .filter(|record| record.id != removed)
                .cloned()
                .collect::<Vec<_>>();
            assert!(!matches!(
                prepare(&fixture, &document(subset))
                    .expect("proper general witness subset")
                    .preflight(),
                ConstraintPreflightV1::DirectConflict { .. }
            ));
        }

        let disconnected_fixed = prepare(
            &fixture,
            &document([
                record(GeometricConstraintKindV1::FixedLength {
                    edge: fixture.edges[5],
                    length_mm: 7.0,
                }),
                records[2].clone(),
                records[3].clone(),
                records[4].clone(),
                records[5].clone(),
            ]),
        )
        .expect("fixed length disconnected from the inconsistent cycle")
        .preflight();
        assert!(!matches!(
            disconnected_fixed,
            ConstraintPreflightV1::DirectConflict { .. }
        ));
    }

    #[test]
    fn general_ratio_graph_never_infers_the_forbidden_reverse_ratio_direction() {
        let fixture = Fixture::new();
        let fixed = record(GeometricConstraintKindV1::FixedLength {
            edge: fixture.edges[0],
            length_mm: 1.0,
        });
        let connector_a = record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: fixture.edges[0],
            denominator_edge: fixture.edges[1],
            ratio: 1.0,
        });
        let forward_a = record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: fixture.edges[1],
            denominator_edge: fixture.edges[2],
            ratio: 2.0,
        });
        let reverse_a = record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: fixture.edges[2],
            denominator_edge: fixture.edges[1],
            ratio: 0.25,
        });
        let connector_b = record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: fixture.edges[0],
            denominator_edge: fixture.edges[3],
            ratio: 1.0,
        });
        let forward_b = record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: fixture.edges[3],
            denominator_edge: fixture.edges[4],
            ratio: 4.0,
        });
        let reverse_b = record(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: fixture.edges[4],
            denominator_edge: fixture.edges[3],
            ratio: 0.125,
        });
        let records = vec![
            fixed.clone(),
            connector_a.clone(),
            forward_a.clone(),
            reverse_a.clone(),
            connector_b.clone(),
            forward_b.clone(),
            reverse_b.clone(),
        ];
        let prepared = prepare(&fixture, &document(records))
            .expect("two inconsistent ratio cycles connected to one remote fixed edge");
        assert_solver_required(&prepared.preflight());
        assert_bounded_direct_oracle_unknown(&prepared);

        let reverse_kind = |record: &GeometricConstraintRecordV1| {
            let GeometricConstraintKindV1::LengthRatio {
                numerator_edge,
                denominator_edge,
                ratio,
            } = record.constraint
            else {
                panic!("ratio record");
            };
            GeometricConstraintRecordV1 {
                id: record.id,
                constraint: GeometricConstraintKindV1::LengthRatio {
                    numerator_edge: denominator_edge,
                    denominator_edge: numerator_edge,
                    ratio: 1.0 / ratio,
                },
            }
        };
        let oriented_forward = prepare(
            &fixture,
            &document([
                fixed.clone(),
                connector_a.clone(),
                forward_a.clone(),
                reverse_a.clone(),
            ]),
        )
        .expect("remote two-edge cycle")
        .preflight();
        let oriented_reverse = prepare(
            &fixture,
            &document([
                fixed,
                reverse_kind(&connector_a),
                reverse_kind(&forward_a),
                reverse_kind(&reverse_a),
            ]),
        )
        .expect("fully direction-reversed remote two-edge cycle")
        .preflight();
        assert!(!matches!(
            oriented_forward,
            ConstraintPreflightV1::DirectConflict { .. }
        ));
        assert!(matches!(
            oriented_reverse,
            ConstraintPreflightV1::DirectConflict { conflicts }
                if conflicts.iter().any(|conflict| matches!(
                    conflict.conflict(),
                    DirectConstraintConflictKindV1::InconsistentLengthRatioGraphWithFixedLength {
                        ratio_constraint_count: 3,
                        ..
                    }
                ))
        ));
    }

    #[test]
    fn equal_length_graph_returns_a_canonical_shortest_oracle_proof_core() {
        let fixture = Fixture::new();
        let records = vec![
            record(GeometricConstraintKindV1::FixedLength {
                edge: fixture.edges[0],
                length_mm: 1.0,
            }),
            record(GeometricConstraintKindV1::EqualLength {
                first_edge: fixture.edges[0],
                second_edge: fixture.edges[1],
            }),
            record(GeometricConstraintKindV1::EqualLength {
                first_edge: fixture.edges[1],
                second_edge: fixture.edges[2],
            }),
            record(GeometricConstraintKindV1::FixedLength {
                edge: fixture.edges[2],
                length_mm: 2.0,
            }),
        ];
        let ConstraintPreflightV1::DirectConflict { conflicts } =
            prepare(&fixture, &document(records.clone()))
                .expect("different fixed lengths connected by an equal-length path")
                .preflight()
        else {
            panic!("equal-length component must conflict");
        };
        assert_eq!(conflicts.len(), 1);
        assert!(matches!(
            conflicts[0].conflict(),
            DirectConstraintConflictKindV1::DifferentFixedLengthsInEqualLengthComponent {
                equal_constraint_count: 2,
                ..
            }
        ));
        assert_eq!(conflicts[0].constraint_ids().len(), 4);
        for removed in conflicts[0].constraint_ids() {
            let subset = records
                .iter()
                .filter(|record| record.id != *removed)
                .cloned()
                .collect::<Vec<_>>();
            assert!(!matches!(
                prepare(&fixture, &document(subset))
                    .expect("proper equal-length witness subset")
                    .preflight(),
                ConstraintPreflightV1::DirectConflict { .. }
            ));
        }

        let same_lengths = prepare(
            &fixture,
            &document([
                records[0].clone(),
                records[1].clone(),
                records[2].clone(),
                record(GeometricConstraintKindV1::FixedLength {
                    edge: fixture.edges[2],
                    length_mm: 1.0,
                }),
            ]),
        )
        .expect("equal fixed lengths across the component")
        .preflight();
        assert!(!matches!(
            same_lengths,
            ConstraintPreflightV1::DirectConflict { .. }
        ));

        let duplicate_equal = record(GeometricConstraintKindV1::EqualLength {
            first_edge: fixture.edges[0],
            second_edge: fixture.edges[1],
        });
        let duplicate_fixed = record(GeometricConstraintKindV1::FixedLength {
            edge: fixture.edges[0],
            length_mm: 1.0,
        });
        let mut duplicated = records.clone();
        duplicated.extend([duplicate_equal, duplicate_fixed]);
        let forward = prepare(&fixture, &document(duplicated.clone()))
            .expect("equal duplicates")
            .preflight();
        duplicated.reverse();
        let reversed = prepare(&fixture, &document(duplicated))
            .expect("source-reordered equal duplicates")
            .preflight();
        assert_eq!(forward, reversed);

        GENERAL_EQUAL_TEST_WORK_LIMIT.with(|limit| {
            assert_eq!(limit.replace(Some(MAX_GENERAL_EQUAL_GRAPH_WORK_V1)), None);
        });
        let baseline = prepare(&fixture, &document(records.clone()))
            .expect("baseline work-accounted equal graph")
            .preflight();
        let exact_work = GENERAL_EQUAL_TEST_WORK_OBSERVED.with(std::cell::Cell::get);
        GENERAL_EQUAL_TEST_WORK_LIMIT.with(|limit| {
            assert_eq!(
                limit.replace(Some(exact_work)),
                Some(MAX_GENERAL_EQUAL_GRAPH_WORK_V1)
            );
        });
        assert_eq!(
            prepare(&fixture, &document(records.clone()))
                .expect("exact equal-graph work budget")
                .preflight(),
            baseline
        );
        GENERAL_EQUAL_TEST_WORK_LIMIT.with(|limit| limit.set(Some(exact_work - 1)));
        let limited = prepare(&fixture, &document(records.clone()))
            .expect("one-short equal-graph work budget")
            .preflight();
        GENERAL_EQUAL_TEST_WORK_LIMIT.with(|limit| limit.set(None));
        let mut all_ids = records.iter().map(|record| record.id).collect::<Vec<_>>();
        canonicalize_constraint_ids(&mut all_ids);
        assert_eq!(
            limited,
            ConstraintPreflightV1::Unknown {
                reason: GeometricConstraintUnknownReasonV1::WorkLimitExceeded,
                unchecked_constraint_ids: all_ids,
            }
        );
    }

    #[test]
    fn equal_length_graph_diamond_with_three_values_is_source_order_invariant() {
        let fixture = Fixture::new();
        let mut records = vec![
            record(GeometricConstraintKindV1::FixedLength {
                edge: fixture.edges[0],
                length_mm: 1.0,
            }),
            record(GeometricConstraintKindV1::FixedLength {
                edge: fixture.edges[3],
                length_mm: 2.0,
            }),
            record(GeometricConstraintKindV1::FixedLength {
                edge: fixture.edges[4],
                length_mm: 3.0,
            }),
            record(GeometricConstraintKindV1::EqualLength {
                first_edge: fixture.edges[0],
                second_edge: fixture.edges[1],
            }),
            record(GeometricConstraintKindV1::EqualLength {
                first_edge: fixture.edges[1],
                second_edge: fixture.edges[3],
            }),
            record(GeometricConstraintKindV1::EqualLength {
                first_edge: fixture.edges[0],
                second_edge: fixture.edges[2],
            }),
            record(GeometricConstraintKindV1::EqualLength {
                first_edge: fixture.edges[2],
                second_edge: fixture.edges[3],
            }),
            record(GeometricConstraintKindV1::EqualLength {
                first_edge: fixture.edges[1],
                second_edge: fixture.edges[4],
            }),
        ];
        let forward = prepare(&fixture, &document(records.clone()))
            .expect("three-value equal-length diamond")
            .preflight();
        records.reverse();
        let reverse = prepare(&fixture, &document(records))
            .expect("source-reordered equal-length diamond")
            .preflight();
        assert_eq!(forward, reverse);
        let ConstraintPreflightV1::DirectConflict { conflicts } = forward else {
            panic!("diamond must select one deterministic shortest conflict");
        };
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].constraint_ids().len(), 4);
    }

    #[test]
    fn equal_length_graph_witness_cap_keeps_searching_for_a_short_pair() {
        let scan = |path_edges: usize, include_short_pair: bool| {
            let node_count = path_edges + 1 + usize::from(include_short_pair) * 3;
            let edges = (0..node_count).map(|_| EdgeId::new()).collect::<Vec<_>>();
            let mut equal_lengths = BTreeMap::new();
            for index in 0..path_edges {
                equal_lengths.insert(
                    EdgePairKey::unordered(edges[index], edges[index + 1]),
                    vec![ConstraintId::new()],
                );
            }
            let mut fixed_lengths = BTreeMap::from([
                (
                    edges[0].canonical_bytes(),
                    ScalarGroupSummary::new(ScalarAssignment {
                        id: ConstraintId::new(),
                        value: 1.0,
                    }),
                ),
                (
                    edges[path_edges].canonical_bytes(),
                    ScalarGroupSummary::new(ScalarAssignment {
                        id: ConstraintId::new(),
                        value: 2.0,
                    }),
                ),
            ]);
            if include_short_pair {
                let first = path_edges + 1;
                for offset in 0..2 {
                    equal_lengths.insert(
                        EdgePairKey::unordered(edges[first + offset], edges[first + offset + 1]),
                        vec![ConstraintId::new()],
                    );
                }
                fixed_lengths.insert(
                    edges[first].canonical_bytes(),
                    ScalarGroupSummary::new(ScalarAssignment {
                        id: ConstraintId::new(),
                        value: 3.0,
                    }),
                );
                fixed_lengths.insert(
                    edges[first + 2].canonical_bytes(),
                    ScalarGroupSummary::new(ScalarAssignment {
                        id: ConstraintId::new(),
                        value: 4.0,
                    }),
                );
            }
            let edge_ids = edges
                .iter()
                .map(|edge| (edge.canonical_bytes(), *edge))
                .collect::<BTreeMap<_, _>>();
            general_equal_length_graph_conflict_v1(&equal_lengths, &fixed_lengths, &edge_ids)
        };
        assert_eq!(
            scan(254, false).unwrap().unwrap().constraint_ids().len(),
            256
        );
        assert_eq!(scan(255, false), Err(()));
        assert_eq!(scan(255, true).unwrap().unwrap().constraint_ids().len(), 4);
    }

    #[test]
    fn partially_checked_fixed_angle_and_ratio_kinds_return_unknown() {
        let fixture = Fixture::new();

        let fixed_angle = record(GeometricConstraintKindV1::FixedAngle {
            vertex: fixture.vertices[0],
            first_edge: fixture.edges[0],
            second_edge: fixture.edges[1],
            angle_degrees: 0.0,
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
    fn parallel_graph_detects_perpendicular_orientation_paths_and_same_node_labels() {
        let fixture = Fixture::new();
        let path_records = vec![
            record(GeometricConstraintKindV1::Horizontal {
                edge: fixture.edges[0],
            }),
            record(GeometricConstraintKindV1::Parallel {
                first_edge: fixture.edges[0],
                second_edge: fixture.edges[1],
            }),
            record(GeometricConstraintKindV1::Parallel {
                first_edge: fixture.edges[1],
                second_edge: fixture.edges[2],
            }),
            record(GeometricConstraintKindV1::Vertical {
                edge: fixture.edges[2],
            }),
        ];
        let prepared = prepare(&fixture, &document(path_records.clone()))
            .expect("perpendicular orientations connected by a parallel path");
        assert_solver_required(&prepared.preflight());
        assert_bounded_direct_oracle_unknown(&prepared);
        let mut duplicated = path_records.clone();
        duplicated.extend([
            record(GeometricConstraintKindV1::Parallel {
                first_edge: fixture.edges[0],
                second_edge: fixture.edges[1],
            }),
            record(GeometricConstraintKindV1::Horizontal {
                edge: fixture.edges[0],
            }),
        ]);
        let forward = prepare(&fixture, &document(duplicated.clone()))
            .expect("duplicate parallel graph labels")
            .preflight();
        duplicated.reverse();
        let reverse = prepare(&fixture, &document(duplicated))
            .expect("source-reordered duplicate parallel graph labels")
            .preflight();
        assert_eq!(forward, reverse);
        assert_solver_required(&forward);
        for removed in path_records.iter().map(|record| record.id) {
            let subset = path_records
                .iter()
                .filter(|record| record.id != removed)
                .cloned()
                .collect::<Vec<_>>();
            assert!(!matches!(
                prepare(&fixture, &document(subset))
                    .expect("proper parallel-path witness subset")
                    .preflight(),
                ConstraintPreflightV1::DirectConflict { .. }
            ));
        }

        let same_node_records = vec![
            record(GeometricConstraintKindV1::Horizontal {
                edge: fixture.edges[0],
            }),
            record(GeometricConstraintKindV1::Vertical {
                edge: fixture.edges[0],
            }),
            record(GeometricConstraintKindV1::Parallel {
                first_edge: fixture.edges[0],
                second_edge: fixture.edges[1],
            }),
        ];
        assert_same_edge_parallel_zero_closure_is_exact_and_minimal(
            &fixture,
            &same_node_records,
            fixture.edges[0],
        );

        GENERAL_PARALLEL_TEST_WORK_LIMIT.with(|limit| {
            assert_eq!(
                limit.replace(Some(MAX_GENERAL_PARALLEL_GRAPH_WORK_V1)),
                None
            );
        });
        let baseline = prepare(&fixture, &document(path_records.clone()))
            .expect("work-accounted parallel graph")
            .preflight();
        let exact_work = GENERAL_PARALLEL_TEST_WORK_OBSERVED.with(std::cell::Cell::get);
        GENERAL_PARALLEL_TEST_WORK_LIMIT.with(|limit| limit.set(Some(exact_work)));
        assert_eq!(
            prepare(&fixture, &document(path_records.clone()))
                .expect("exact parallel work limit")
                .preflight(),
            baseline
        );
        GENERAL_PARALLEL_TEST_WORK_LIMIT.with(|limit| limit.set(Some(exact_work - 1)));
        let limited = prepare(&fixture, &document(path_records.clone()))
            .expect("one-short parallel work limit")
            .preflight();
        GENERAL_PARALLEL_TEST_WORK_LIMIT.with(|limit| limit.set(None));
        let mut all_ids = path_records
            .iter()
            .map(|record| record.id)
            .collect::<Vec<_>>();
        canonicalize_constraint_ids(&mut all_ids);
        assert_eq!(
            limited,
            ConstraintPreflightV1::Unknown {
                reason: GeometricConstraintUnknownReasonV1::WorkLimitExceeded,
                unchecked_constraint_ids: all_ids,
            }
        );
    }

    #[test]
    fn parallel_graph_witness_cap_keeps_searching_for_a_short_remote_pair() {
        let scan = |path_edges: usize, include_short_pair: bool| {
            let node_count = path_edges + 1 + usize::from(include_short_pair) * 3;
            let edges = (0..node_count).map(|_| EdgeId::new()).collect::<Vec<_>>();
            let mut parallels = BTreeMap::new();
            for index in 0..path_edges {
                parallels.insert(
                    EdgePairKey::unordered(edges[index], edges[index + 1]),
                    vec![ConstraintId::new()],
                );
            }
            let mut horizontal =
                BTreeMap::from([(edges[0].canonical_bytes(), vec![ConstraintId::new()])]);
            let mut vertical = BTreeMap::from([(
                edges[path_edges].canonical_bytes(),
                vec![ConstraintId::new()],
            )]);
            if include_short_pair {
                let first = path_edges + 1;
                for offset in 0..2 {
                    parallels.insert(
                        EdgePairKey::unordered(edges[first + offset], edges[first + offset + 1]),
                        vec![ConstraintId::new()],
                    );
                }
                horizontal.insert(edges[first].canonical_bytes(), vec![ConstraintId::new()]);
                vertical.insert(
                    edges[first + 2].canonical_bytes(),
                    vec![ConstraintId::new()],
                );
            }
            let edge_ids = edges
                .iter()
                .map(|edge| (edge.canonical_bytes(), *edge))
                .collect::<BTreeMap<_, _>>();
            general_parallel_graph_conflict_v1(
                &parallels,
                &horizontal,
                &vertical,
                &BTreeMap::new(),
                &BTreeMap::new(),
                &edge_ids,
            )
        };
        assert_eq!(
            scan(254, false).unwrap().unwrap().constraint_ids().len(),
            256
        );
        assert_eq!(scan(255, false), Err(()));
        assert_eq!(scan(255, true).unwrap().unwrap().constraint_ids().len(), 4);
    }

    #[test]
    fn parallel_graph_diamond_selects_the_canonical_minimum_equal_length_path() {
        let fixture = Fixture::new();
        let horizontal = record(GeometricConstraintKindV1::Horizontal {
            edge: fixture.edges[0],
        });
        let vertical = record(GeometricConstraintKindV1::Vertical {
            edge: fixture.edges[3],
        });
        let mut parallel_records = (0..4)
            .map(|_| {
                record(GeometricConstraintKindV1::Parallel {
                    first_edge: fixture.edges[0],
                    second_edge: fixture.edges[1],
                })
            })
            .collect::<Vec<_>>();
        parallel_records.sort_unstable_by_key(|record| record.id.canonical_bytes());
        let paths = [
            (fixture.edges[0], fixture.edges[1]),
            (fixture.edges[1], fixture.edges[3]),
            (fixture.edges[0], fixture.edges[2]),
            (fixture.edges[2], fixture.edges[3]),
        ];
        for (record, (first_edge, second_edge)) in parallel_records.iter_mut().zip(paths) {
            record.constraint = GeometricConstraintKindV1::Parallel {
                first_edge,
                second_edge,
            };
        }
        let mut records = vec![horizontal.clone(), vertical.clone()];
        records.extend(parallel_records.clone());
        let forward = prepare(&fixture, &document(records.clone()))
            .expect("parallel diamond")
            .preflight();
        records.reverse();
        let reverse = prepare(&fixture, &document(records))
            .expect("source-reordered parallel diamond")
            .preflight();
        assert_eq!(forward, reverse);
        assert_solver_required(&forward);
    }

    #[test]
    fn fixed_angle_parallel_graph_requires_a_nonempty_path_and_excludes_zero_and_180() {
        let fixture = Fixture::new();
        let first_parallel = record(GeometricConstraintKindV1::Parallel {
            first_edge: fixture.edges[0],
            second_edge: fixture.edges[1],
        });
        let second_parallel = record(GeometricConstraintKindV1::Parallel {
            first_edge: fixture.edges[1],
            second_edge: fixture.edges[2],
        });
        let angle = record(GeometricConstraintKindV1::FixedAngle {
            vertex: fixture.vertices[0],
            first_edge: fixture.edges[0],
            second_edge: fixture.edges[2],
            angle_degrees: 90.0,
        });
        let records = vec![
            first_parallel.clone(),
            second_parallel.clone(),
            angle.clone(),
        ];
        let prepared = prepare(&fixture, &document(records.clone()))
            .expect("nonparallel fixed angle inside a parallel component");
        let baseline = prepared.preflight();
        assert_solver_required(&baseline);
        assert_bounded_direct_oracle_unknown(&prepared);
        let mut reversed_angle = angle.clone();
        reversed_angle.constraint = GeometricConstraintKindV1::FixedAngle {
            vertex: fixture.vertices[0],
            first_edge: fixture.edges[2],
            second_edge: fixture.edges[0],
            angle_degrees: 90.0,
        };
        assert_eq!(
            prepare(
                &fixture,
                &document([
                    first_parallel.clone(),
                    second_parallel.clone(),
                    reversed_angle,
                ]),
            )
            .expect("operand-reversed fixed angle")
            .preflight(),
            baseline
        );
        for removed in records.iter().map(|record| record.id) {
            let subset = records
                .iter()
                .filter(|record| record.id != removed)
                .cloned()
                .collect::<Vec<_>>();
            assert!(!matches!(
                prepare(&fixture, &document(subset))
                    .expect("proper fixed-angle parallel witness subset")
                    .preflight(),
                ConstraintPreflightV1::DirectConflict { .. }
            ));
        }

        for allowed in [0.0, -0.0, 180.0] {
            let outcome = prepare(
                &fixture,
                &document([
                    first_parallel.clone(),
                    second_parallel.clone(),
                    record(GeometricConstraintKindV1::FixedAngle {
                        vertex: fixture.vertices[0],
                        first_edge: fixture.edges[0],
                        second_edge: fixture.edges[2],
                        angle_degrees: allowed,
                    }),
                ]),
            )
            .expect("allowed parallel fixed angle")
            .preflight();
            assert!(!matches!(
                outcome,
                ConstraintPreflightV1::DirectConflict { .. }
            ));
        }
        let signed_zero_duplicates = prepare(
            &fixture,
            &document([
                first_parallel.clone(),
                second_parallel.clone(),
                record(GeometricConstraintKindV1::FixedAngle {
                    vertex: fixture.vertices[0],
                    first_edge: fixture.edges[0],
                    second_edge: fixture.edges[2],
                    angle_degrees: 0.0,
                }),
                record(GeometricConstraintKindV1::FixedAngle {
                    vertex: fixture.vertices[0],
                    first_edge: fixture.edges[0],
                    second_edge: fixture.edges[2],
                    angle_degrees: -0.0,
                }),
            ]),
        )
        .expect("signed zero fixed-angle duplicates")
        .preflight();
        assert!(!matches!(
            signed_zero_duplicates,
            ConstraintPreflightV1::DirectConflict { .. }
        ));
        for incompatible in [f64::from_bits(1), f64::from_bits(180.0_f64.to_bits() - 1)] {
            let prepared = prepare(
                &fixture,
                &document([
                    first_parallel.clone(),
                    second_parallel.clone(),
                    record(GeometricConstraintKindV1::FixedAngle {
                        vertex: fixture.vertices[0],
                        first_edge: fixture.edges[0],
                        second_edge: fixture.edges[2],
                        angle_degrees: incompatible,
                    }),
                ]),
            )
            .expect("one-ULP incompatible angle");
            assert_solver_required(&prepared.preflight());
        }

        let no_path = prepare(
            &fixture,
            &document([
                record(GeometricConstraintKindV1::Parallel {
                    first_edge: fixture.edges[3],
                    second_edge: fixture.edges[4],
                }),
                angle,
            ]),
        )
        .expect("fixed-angle operands do not participate in the parallel component")
        .preflight();
        assert!(!matches!(
            no_path,
            ConstraintPreflightV1::DirectConflict { .. }
        ));
    }

    #[test]
    fn fixed_angle_parallel_graph_has_canonical_path_cap_and_work_boundary() {
        let scan = |path_edges: usize| {
            let edges = (0..=path_edges).map(|_| EdgeId::new()).collect::<Vec<_>>();
            let vertex = VertexId::new();
            let angle_id = ConstraintId::new();
            let mut parallels = BTreeMap::new();
            for index in 0..path_edges {
                parallels.insert(
                    EdgePairKey::unordered(edges[index], edges[index + 1]),
                    vec![ConstraintId::new()],
                );
            }
            let fixed_angles = BTreeMap::from([(
                AngleKey {
                    vertex: vertex.canonical_bytes(),
                    edges: EdgePairKey::unordered(edges[0], edges[path_edges]),
                },
                vec![ScalarAssignment {
                    id: angle_id,
                    value: 90.0,
                }],
            )]);
            let vertex_ids = BTreeMap::from([(vertex.canonical_bytes(), vertex)]);
            let edge_ids = edges
                .iter()
                .map(|edge| (edge.canonical_bytes(), *edge))
                .collect::<BTreeMap<_, _>>();
            let result = general_parallel_graph_conflict_v1(
                &parallels,
                &BTreeMap::new(),
                &BTreeMap::new(),
                &fixed_angles,
                &vertex_ids,
                &edge_ids,
            );
            (result, angle_id)
        };

        let (bounded, _) = scan(255);
        assert_eq!(bounded.unwrap().unwrap().constraint_ids().len(), 256);
        assert_eq!(scan(256).0, Err(()));

        GENERAL_PARALLEL_TEST_WORK_LIMIT.with(|limit| limit.set(None));
        let (baseline, _) = scan(3);
        assert!(baseline.is_ok());
        let exact_work = GENERAL_PARALLEL_TEST_WORK_OBSERVED.with(std::cell::Cell::get);
        GENERAL_PARALLEL_TEST_WORK_LIMIT.with(|limit| limit.set(Some(exact_work)));
        assert!(scan(3).0.is_ok());
        GENERAL_PARALLEL_TEST_WORK_LIMIT.with(|limit| limit.set(Some(exact_work - 1)));
        assert_eq!(scan(3).0, Err(()));
        GENERAL_PARALLEL_TEST_WORK_LIMIT.with(|limit| limit.set(None));
    }

    #[test]
    fn fixed_angle_parallel_diamond_uses_minimum_constraint_ids() {
        let fixture = Fixture::new();
        let angle = record(GeometricConstraintKindV1::FixedAngle {
            vertex: fixture.vertices[0],
            first_edge: fixture.edges[0],
            second_edge: fixture.edges[3],
            angle_degrees: 90.0,
        });
        let mut parallels = (0..4)
            .map(|_| {
                record(GeometricConstraintKindV1::Parallel {
                    first_edge: fixture.edges[0],
                    second_edge: fixture.edges[1],
                })
            })
            .collect::<Vec<_>>();
        parallels.sort_unstable_by_key(|record| record.id.canonical_bytes());
        for (record, (first_edge, second_edge)) in parallels.iter_mut().zip([
            (fixture.edges[0], fixture.edges[2]),
            (fixture.edges[2], fixture.edges[3]),
            (fixture.edges[0], fixture.edges[1]),
            (fixture.edges[1], fixture.edges[3]),
        ]) {
            record.constraint = GeometricConstraintKindV1::Parallel {
                first_edge,
                second_edge,
            };
        }
        let mut records = vec![angle.clone()];
        records.extend(parallels.clone());
        let forward = prepare(&fixture, &document(records.clone()))
            .expect("fixed-angle parallel diamond")
            .preflight();
        records.reverse();
        let reverse = prepare(&fixture, &document(records))
            .expect("reordered fixed-angle parallel diamond")
            .preflight();
        assert_eq!(forward, reverse);
        assert_solver_required(&forward);
    }

    pub(super) fn sorted_ids(ids: &[ConstraintId]) -> Vec<ConstraintId> {
        let mut result = ids.to_vec();
        canonicalize_constraint_ids(&mut result);
        result
    }

    fn same_ids(actual: &[ConstraintId], expected: &[ConstraintId]) -> bool {
        actual == sorted_ids(expected)
    }

    pub(super) fn uuid_string<T: Serialize>(id: T) -> String {
        serde_json::to_string(&id)
            .expect("serialize UUID-backed ID")
            .trim_matches('"')
            .to_owned()
    }

    pub(super) fn deterministic_shuffle<T>(items: &mut [T], state: &mut u64) {
        for index in (1..items.len()).rev() {
            *state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let target = (*state as usize) % (index + 1);
            items.swap(index, target);
        }
    }

    pub(super) fn reverse_unordered_operands(constraint: &mut GeometricConstraintKindV1) {
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

    #[test]
    fn parallel_with_perpendicular_orientations_feeds_the_bounded_direct_oracle() {
        for count in [4, 8, 16] {
            let fixture = Fixture::new();
            let mut records = vec![
                record(GeometricConstraintKindV1::Parallel {
                    first_edge: fixture.edges[1],
                    second_edge: fixture.edges[0],
                }),
                record(GeometricConstraintKindV1::Horizontal {
                    edge: fixture.edges[0],
                }),
                record(GeometricConstraintKindV1::Vertical {
                    edge: fixture.edges[1],
                }),
            ];
            records.extend((3..count).map(|index| {
                record(GeometricConstraintKindV1::EqualLength {
                    first_edge: fixture.edges[index % 6],
                    second_edge: fixture.edges[(index + 1) % 6],
                })
            }));
            let prepared = prepare(&fixture, &document(records)).unwrap();
            let BoundedDirectMusV1::ProvenUnsatisfiable { constraint_ids, .. } =
                find_bounded_direct_mus_v1(&prepared)
            else {
                panic!("normalized parallel residual cannot accept perpendicular directions")
            };
            assert_eq!(constraint_ids.len(), 3);
            for removed in &constraint_ids {
                let constraints = prepared
                    .constraints
                    .iter()
                    .filter(|record| constraint_ids.contains(&record.id) && record.id != *removed)
                    .cloned()
                    .collect();
                let subset = GeometricConstraintSetV1 {
                    source_pattern: &fixture.pattern,
                    constraints,
                    max_preflight_checks: prepared.max_preflight_checks,
                };
                assert!(!matches!(
                    subset.preflight(),
                    ConstraintPreflightV1::DirectConflict { .. }
                ));
            }
        }
    }

    #[test]
    fn equal_length_with_different_positive_fixed_lengths_feeds_the_bounded_direct_oracle() {
        for count in [4, 8, 16] {
            let fixture = Fixture::new();
            let mut records = vec![
                record(GeometricConstraintKindV1::EqualLength {
                    first_edge: fixture.edges[1],
                    second_edge: fixture.edges[0],
                }),
                record(GeometricConstraintKindV1::FixedLength {
                    edge: fixture.edges[0],
                    length_mm: 1.0,
                }),
                record(GeometricConstraintKindV1::FixedLength {
                    edge: fixture.edges[1],
                    length_mm: 2.0,
                }),
            ];
            records.extend((3..count).map(|index| {
                record(GeometricConstraintKindV1::Horizontal {
                    edge: fixture.edges[index % 6],
                })
            }));
            let prepared = prepare(&fixture, &document(records)).unwrap();
            let BoundedDirectMusV1::ProvenUnsatisfiable { constraint_ids, .. } =
                find_bounded_direct_mus_v1(&prepared)
            else {
                panic!("equal positive lengths cannot have different fixed values")
            };
            assert_eq!(constraint_ids.len(), 3);
            for removed in &constraint_ids {
                let constraints = prepared
                    .constraints
                    .iter()
                    .filter(|record| constraint_ids.contains(&record.id) && record.id != *removed)
                    .cloned()
                    .collect();
                let subset = GeometricConstraintSetV1 {
                    source_pattern: &fixture.pattern,
                    constraints,
                    max_preflight_checks: prepared.max_preflight_checks,
                };
                assert!(!matches!(
                    subset.preflight(),
                    ConstraintPreflightV1::DirectConflict { .. }
                ));
            }
        }

        let fixture = Fixture::new();
        let compatible = prepare(
            &fixture,
            &document([
                record(GeometricConstraintKindV1::EqualLength {
                    first_edge: fixture.edges[0],
                    second_edge: fixture.edges[1],
                }),
                record(GeometricConstraintKindV1::FixedLength {
                    edge: fixture.edges[0],
                    length_mm: 1.0,
                }),
                record(GeometricConstraintKindV1::FixedLength {
                    edge: fixture.edges[1],
                    length_mm: 1.0,
                }),
            ]),
        )
        .unwrap();
        assert!(!matches!(
            compatible.preflight(),
            ConstraintPreflightV1::DirectConflict { .. }
        ));
    }

    fn bounded_zero_closure_records(
        fixture: &Fixture,
        use_ratio: bool,
        fixed_length: f64,
        ratio: f64,
    ) -> Vec<GeometricConstraintRecordV1> {
        let mut records = vec![
            record(GeometricConstraintKindV1::Horizontal {
                edge: fixture.edges[4],
            }),
            record(GeometricConstraintKindV1::Vertical {
                edge: fixture.edges[4],
            }),
        ];
        records.push(if use_ratio {
            record(GeometricConstraintKindV1::LengthRatio {
                numerator_edge: fixture.edges[0],
                denominator_edge: fixture.edges[4],
                ratio,
            })
        } else {
            record(GeometricConstraintKindV1::EqualLength {
                first_edge: fixture.edges[0],
                second_edge: fixture.edges[4],
            })
        });
        records.push(record(GeometricConstraintKindV1::FixedLength {
            edge: fixture.edges[0],
            length_mm: fixed_length,
        }));
        records
    }

    fn only_bounded_zero_closure_conflict(
        preflight: &ConstraintPreflightV1,
    ) -> &DirectConstraintConflictV1 {
        let ConstraintPreflightV1::DirectConflict { conflicts } = preflight else {
            panic!("the bounded zero-length closure must prove a conflict");
        };
        let [conflict] = conflicts.as_slice() else {
            panic!("the fixture must emit exactly one conflict: {conflicts:?}");
        };
        assert!(matches!(
            conflict.conflict(),
            DirectConstraintConflictKindV1::PositiveFixedLengthInBoundedZeroLengthClosure { .. }
        ));
        conflict
    }

    fn only_nondegenerate_provider_closure_conflict(
        preflight: &ConstraintPreflightV1,
    ) -> &DirectConstraintConflictV1 {
        let ConstraintPreflightV1::DirectConflict { conflicts } = preflight else {
            panic!("the non-degeneracy terminal closure must prove a conflict");
        };
        let [conflict] = conflicts.as_slice() else {
            panic!("the fixture must emit exactly one conflict: {conflicts:?}");
        };
        assert!(matches!(
            conflict.conflict(),
            DirectConstraintConflictKindV1::ZeroLengthClosureReachesNondegenerateProvider { .. }
        ));
        conflict
    }

    #[test]
    fn bounded_zero_length_closure_crosses_equal_length_and_ratio_without_solver_assumptions() {
        for use_ratio in [false, true] {
            for (fixed_length, ratio) in
                [(f64::from_bits(1), f64::from_bits(1)), (f64::MAX, f64::MAX)]
            {
                let fixture = Fixture::new();
                let records =
                    bounded_zero_closure_records(&fixture, use_ratio, fixed_length, ratio);
                let expected_ids =
                    sorted_ids(&records.iter().map(|record| record.id).collect::<Vec<_>>());
                let prepared = prepare(&fixture, &document(records)).unwrap();
                let preflight = prepared.preflight();
                let conflict = only_bounded_zero_closure_conflict(&preflight);
                assert_eq!(conflict.constraint_ids(), expected_ids);
                assert!(matches!(
                    conflict.conflict(),
                    DirectConstraintConflictKindV1::
                        PositiveFixedLengthInBoundedZeroLengthClosure {
                            fixed_edge,
                            forced_zero_edge,
                            horizontal_constraint_count: 1,
                            vertical_constraint_count: 1,
                            zero_propagation_constraint_count: 1,
                        } if *fixed_edge == fixture.edges[0]
                            && *forced_zero_edge == fixture.edges[4]
                ));
            }
        }
    }

    #[test]
    fn bounded_zero_length_closure_serializes_its_distinct_wire_tag_and_counts() {
        let fixture = Fixture::new();
        let records = bounded_zero_closure_records(&fixture, true, 1.0, 2.0);
        let prepared = prepare(&fixture, &document(records)).unwrap();
        let preflight = prepared.preflight();
        let conflict = only_bounded_zero_closure_conflict(&preflight);
        let value = serde_json::to_value(conflict).expect("serialize bounded zero-length conflict");

        assert_eq!(
            value["conflict"],
            json!({
                "kind": "positive_fixed_length_in_bounded_zero_length_closure",
                "fixed_edge": fixture.edges[0],
                "forced_zero_edge": fixture.edges[4],
                "horizontal_constraint_count": 1,
                "vertical_constraint_count": 1,
                "zero_propagation_constraint_count": 1,
            })
        );
        assert_eq!(conflict_sort_key(conflict.conflict()).0, 21);
        assert_eq!(value["constraint_ids"].as_array().unwrap().len(), 4);
    }

    #[test]
    fn bounded_zero_length_closure_uses_multi_edge_coordinate_equality_paths() {
        let fixture = Fixture::new();
        let bridge = EdgeId::new();
        let mut pattern = fixture.pattern.clone();
        pattern.edges.push(Edge {
            id: bridge,
            start: fixture.vertices[6],
            end: fixture.vertices[1],
            kind: EdgeKind::Auxiliary,
        });
        let records = vec![
            record(GeometricConstraintKindV1::Horizontal {
                edge: fixture.edges[4],
            }),
            record(GeometricConstraintKindV1::Horizontal { edge: bridge }),
            record(GeometricConstraintKindV1::Vertical {
                edge: fixture.edges[4],
            }),
            record(GeometricConstraintKindV1::Vertical { edge: bridge }),
            record(GeometricConstraintKindV1::FixedLength {
                edge: fixture.edges[5],
                length_mm: 1.0,
            }),
        ];
        let expected_ids = sorted_ids(&records.iter().map(|record| record.id).collect::<Vec<_>>());
        let prepared = prepare_geometric_constraints_v1(
            &pattern,
            &document(records),
            GeometricConstraintLimitsV1::default(),
        )
        .unwrap();
        let preflight = prepared.preflight();
        let conflict = only_bounded_zero_closure_conflict(&preflight);
        assert_eq!(conflict.constraint_ids(), expected_ids);
        assert!(matches!(
            conflict.conflict(),
            DirectConstraintConflictKindV1::PositiveFixedLengthInBoundedZeroLengthClosure {
                fixed_edge,
                forced_zero_edge,
                horizontal_constraint_count: 2,
                vertical_constraint_count: 2,
                zero_propagation_constraint_count: 0,
            } if *fixed_edge == fixture.edges[5] && *forced_zero_edge == fixture.edges[5]
        ));
    }

    #[test]
    fn bounded_zero_length_closure_core_is_canonical_and_cardinality_smallest_for_the_oracle() {
        let fixture = Fixture::new();
        let records = bounded_zero_closure_records(&fixture, true, 1.0, 2.0);
        let expected_ids = sorted_ids(&records.iter().map(|record| record.id).collect::<Vec<_>>());
        let baseline = prepare(&fixture, &document(records.clone())).unwrap();
        let baseline_preflight = baseline.preflight();
        assert_eq!(
            only_bounded_zero_closure_conflict(&baseline_preflight).constraint_ids(),
            expected_ids
        );

        let mut reversed = records.clone();
        reversed.reverse();
        let reversed = prepare(&fixture, &document(reversed)).unwrap();
        assert_eq!(reversed.preflight(), baseline_preflight);

        let BoundedDirectMusV1::ProvenUnsatisfiable {
            constraint_ids,
            oracle_calls,
        } = find_bounded_direct_mus_v1(&baseline)
        else {
            panic!("the bounded oracle must retain the exact-zero proof core");
        };
        assert_eq!(constraint_ids, expected_ids);
        assert!(oracle_calls > 0);
        for removed in &constraint_ids {
            let constraints = baseline
                .constraints
                .iter()
                .filter(|record| record.id != *removed)
                .cloned()
                .collect();
            let subset = GeometricConstraintSetV1 {
                source_pattern: &fixture.pattern,
                constraints,
                max_preflight_checks: baseline.max_preflight_checks,
            };
            assert!(
                !matches!(
                    subset.preflight(),
                    ConstraintPreflightV1::DirectConflict { conflicts }
                        if conflicts.iter().any(|conflict| matches!(
                            conflict.conflict(),
                            DirectConstraintConflictKindV1::
                                PositiveFixedLengthInBoundedZeroLengthClosure { .. }
                        ))
                ),
                "deletion removes this theorem's proof; this is not a semantic SAT claim"
            );
        }
    }

    #[test]
    fn bounded_zero_length_closure_selects_the_canonical_duplicate_core_across_storage_orders() {
        let fixture = Fixture::new();
        let mut records = Vec::new();
        for constraint in [
            GeometricConstraintKindV1::Horizontal {
                edge: fixture.edges[4],
            },
            GeometricConstraintKindV1::Vertical {
                edge: fixture.edges[4],
            },
            GeometricConstraintKindV1::LengthRatio {
                numerator_edge: fixture.edges[0],
                denominator_edge: fixture.edges[4],
                ratio: 2.0,
            },
            GeometricConstraintKindV1::FixedLength {
                edge: fixture.edges[0],
                length_mm: 1.0,
            },
        ] {
            records.extend([record(constraint.clone()), record(constraint)]);
        }
        let canonical_minimum_for = |predicate: &dyn Fn(&GeometricConstraintKindV1) -> bool| {
            records
                .iter()
                .filter(|record| predicate(&record.constraint))
                .map(|record| record.id)
                .min_by_key(ConstraintId::canonical_bytes)
                .expect("each duplicated role has a canonical minimum")
        };
        let expected_ids = sorted_ids(&[
            canonical_minimum_for(&|kind| {
                matches!(kind, GeometricConstraintKindV1::Horizontal { .. })
            }),
            canonical_minimum_for(&|kind| {
                matches!(kind, GeometricConstraintKindV1::Vertical { .. })
            }),
            canonical_minimum_for(&|kind| {
                matches!(kind, GeometricConstraintKindV1::LengthRatio { .. })
            }),
            canonical_minimum_for(&|kind| {
                matches!(kind, GeometricConstraintKindV1::FixedLength { .. })
            }),
        ]);

        let forward = prepare(&fixture, &document(records.clone())).unwrap();
        assert_eq!(
            only_bounded_zero_closure_conflict(&forward.preflight()).constraint_ids(),
            expected_ids
        );
        let BoundedDirectMusV1::ProvenUnsatisfiable {
            constraint_ids: forward_core,
            ..
        } = find_bounded_direct_mus_v1(&forward)
        else {
            panic!("the bounded oracle must retain the canonical duplicate proof core");
        };
        assert_eq!(forward_core, expected_ids);

        records.reverse();
        let mut reversed_pattern = fixture.pattern.clone();
        reversed_pattern.edges.reverse();
        let reversed = prepare_geometric_constraints_v1(
            &reversed_pattern,
            &document(records),
            GeometricConstraintLimitsV1::default(),
        )
        .unwrap();
        assert_eq!(reversed.preflight(), forward.preflight());
        assert_eq!(
            find_bounded_direct_mus_v1(&reversed),
            find_bounded_direct_mus_v1(&forward)
        );
    }

    #[test]
    fn bounded_zero_length_closure_uses_each_binary64_proven_provider_terminal() {
        let fixture = Fixture::new();
        let providers = [
            (
                ZeroLengthClosureProviderKindV1::PointOnLine,
                fixture.edges[5],
                GeometricConstraintKindV1::PointOnLine {
                    vertex: fixture.vertices[2],
                    line_edge: fixture.edges[5],
                },
            ),
            (
                ZeroLengthClosureProviderKindV1::MirrorSymmetryAxis,
                fixture.edges[0],
                GeometricConstraintKindV1::MirrorSymmetry {
                    first_vertex: fixture.vertices[2],
                    second_vertex: fixture.vertices[4],
                    axis_edge: fixture.edges[0],
                },
            ),
            (
                ZeroLengthClosureProviderKindV1::AngleBisector,
                fixture.edges[0],
                GeometricConstraintKindV1::AngleBisector {
                    vertex: fixture.vertices[0],
                    first_edge: fixture.edges[0],
                    second_edge: fixture.edges[1],
                    bisector_edge: fixture.edges[2],
                },
            ),
            (
                ZeroLengthClosureProviderKindV1::Parallel,
                fixture.edges[0],
                GeometricConstraintKindV1::Parallel {
                    first_edge: fixture.edges[0],
                    second_edge: fixture.edges[1],
                },
            ),
            (
                ZeroLengthClosureProviderKindV1::FixedAngle,
                fixture.edges[0],
                GeometricConstraintKindV1::FixedAngle {
                    vertex: fixture.vertices[0],
                    first_edge: fixture.edges[0],
                    second_edge: fixture.edges[1],
                    angle_degrees: 90.0,
                },
            ),
        ];

        for (provider_kind, provider_edge, provider) in providers {
            let records = vec![
                record(GeometricConstraintKindV1::Horizontal {
                    edge: fixture.edges[4],
                }),
                record(GeometricConstraintKindV1::Vertical {
                    edge: fixture.edges[4],
                }),
                record(GeometricConstraintKindV1::EqualLength {
                    first_edge: fixture.edges[4],
                    second_edge: provider_edge,
                }),
                record(provider),
            ];
            let expected_ids =
                sorted_ids(&records.iter().map(|record| record.id).collect::<Vec<_>>());
            let prepared = prepare(&fixture, &document(records.clone())).unwrap();
            let preflight = prepared.preflight();
            let conflict = only_nondegenerate_provider_closure_conflict(&preflight);
            assert_eq!(conflict.constraint_ids(), expected_ids);
            assert!(matches!(
                conflict.conflict(),
                DirectConstraintConflictKindV1::
                    ZeroLengthClosureReachesNondegenerateProvider {
                        provider_kind: actual_kind,
                        provider_edge: actual_edge,
                        forced_zero_edge,
                        horizontal_constraint_count: 1,
                        vertical_constraint_count: 1,
                        zero_propagation_constraint_count: 1,
                    } if *actual_kind == provider_kind
                        && *actual_edge == provider_edge
                        && *forced_zero_edge == fixture.edges[4]
            ));

            let mut reversed_records = records;
            reversed_records.reverse();
            let mut reversed_pattern = fixture.pattern.clone();
            reversed_pattern.edges.reverse();
            let reversed = prepare_geometric_constraints_v1(
                &reversed_pattern,
                &document(reversed_records),
                GeometricConstraintLimitsV1::default(),
            )
            .unwrap();
            assert_eq!(reversed.preflight(), preflight);
        }
    }

    #[test]
    fn nondegenerate_provider_closure_serializes_its_closed_wire_contract() {
        let fixture = Fixture::new();
        let prepared = prepare(
            &fixture,
            &document([
                record(GeometricConstraintKindV1::Horizontal {
                    edge: fixture.edges[4],
                }),
                record(GeometricConstraintKindV1::Vertical {
                    edge: fixture.edges[4],
                }),
                record(GeometricConstraintKindV1::EqualLength {
                    first_edge: fixture.edges[4],
                    second_edge: fixture.edges[0],
                }),
                record(GeometricConstraintKindV1::Parallel {
                    first_edge: fixture.edges[0],
                    second_edge: fixture.edges[1],
                }),
            ]),
        )
        .unwrap();
        let preflight = prepared.preflight();
        let conflict = only_nondegenerate_provider_closure_conflict(&preflight);
        assert_eq!(
            serde_json::to_value(conflict.conflict()).unwrap(),
            json!({
                "kind": "zero_length_closure_reaches_nondegenerate_provider",
                "provider_kind": "parallel",
                "provider_edge": fixture.edges[0],
                "forced_zero_edge": fixture.edges[4],
                "horizontal_constraint_count": 1,
                "vertical_constraint_count": 1,
                "zero_propagation_constraint_count": 1,
            })
        );
        assert_eq!(conflict.constraint_ids().len(), 4);
        assert_eq!(conflict_sort_key(conflict.conflict()).0, 22);
    }

    #[test]
    fn fixed_angle_zero_terminal_rejects_signed_zero_pi_and_radian_underflow_false_positives() {
        let fixture = Fixture::new();
        for angle_degrees in [0.0, -0.0, 180.0, f64::from_bits(1)] {
            let prepared = prepare(
                &fixture,
                &document([
                    record(GeometricConstraintKindV1::Horizontal {
                        edge: fixture.edges[4],
                    }),
                    record(GeometricConstraintKindV1::Vertical {
                        edge: fixture.edges[4],
                    }),
                    record(GeometricConstraintKindV1::EqualLength {
                        first_edge: fixture.edges[4],
                        second_edge: fixture.edges[0],
                    }),
                    record(GeometricConstraintKindV1::FixedAngle {
                        vertex: fixture.vertices[0],
                        first_edge: fixture.edges[0],
                        second_edge: fixture.edges[1],
                        angle_degrees,
                    }),
                ]),
            )
            .unwrap();
            assert!(
                !matches!(
                    prepared.preflight(),
                    ConstraintPreflightV1::DirectConflict { conflicts }
                        if conflicts.iter().any(|conflict| matches!(
                            conflict.conflict(),
                            DirectConstraintConflictKindV1::
                                ZeroLengthClosureReachesNondegenerateProvider {
                                    provider_kind:
                                        ZeroLengthClosureProviderKindV1::FixedAngle,
                                    ..
                                }
                        ))
                ),
                "{angle_degrees:?} must retain a collapsed binary64 escape"
            );
        }
    }

    #[test]
    fn parallel_overflow_counterexample_forbids_zero_length_propagation() {
        let first = (1.0e308_f64, 0.0_f64);
        let second = (1.0e308_f64, 1.0e-308_f64);
        let cross = first.0 * second.1 - first.1 * second.0;
        let denominator = first.0.hypot(first.1) * second.0.hypot(second.1);
        let residual = cross / denominator;

        assert!(cross.is_finite());
        assert_ne!(cross, 0.0);
        assert!(denominator.is_infinite());
        assert_eq!(residual, 0.0);
    }

    #[test]
    fn bounded_zero_length_closure_obeys_four_eight_sixteen_seventeen_256_and_257_boundaries() {
        for count in [4, 8, 16, 17, 256] {
            let fixture = Fixture::new();
            let mut records = bounded_zero_closure_records(&fixture, false, 1.0, 1.0);
            let proof_ids = sorted_ids(&records.iter().map(|record| record.id).collect::<Vec<_>>());
            records.extend((records.len()..count).map(|_| {
                record(GeometricConstraintKindV1::Horizontal {
                    edge: fixture.edges[2],
                })
            }));
            let prepared = prepare(&fixture, &document(records)).unwrap();
            assert_eq!(
                only_bounded_zero_closure_conflict(&prepared.preflight()).constraint_ids(),
                proof_ids
            );
            if count <= MAX_BOUNDED_DIRECT_MUS_CONSTRAINTS_V1 {
                let BoundedDirectMusV1::ProvenUnsatisfiable { constraint_ids, .. } =
                    find_bounded_direct_mus_v1(&prepared)
                else {
                    panic!("{count}: the bounded oracle must prove its core");
                };
                assert_eq!(constraint_ids, proof_ids);
            } else {
                assert_eq!(
                    find_bounded_direct_mus_v1(&prepared),
                    BoundedDirectMusV1::Unknown { oracle_calls: 0 }
                );
            }
        }

        let fixture = Fixture::new();
        let mut records = bounded_zero_closure_records(&fixture, false, 1.0, 1.0);
        records.extend((records.len()..257).map(|_| {
            record(GeometricConstraintKindV1::Horizontal {
                edge: fixture.edges[2],
            })
        }));
        let prepared = prepare(&fixture, &document(records)).unwrap();
        assert!(matches!(
            prepared.preflight(),
            ConstraintPreflightV1::Unknown {
                reason: GeometricConstraintUnknownReasonV1::ConstraintLimitExceeded,
                ..
            }
        ));
        assert_eq!(
            find_bounded_direct_mus_v1(&prepared),
            BoundedDirectMusV1::Unknown { oracle_calls: 0 }
        );
    }

    #[test]
    fn bounded_zero_length_closure_resource_and_observer_stops_are_fail_closed() {
        struct StopAt {
            phase: bounded_zero_closure::Phase,
            minimum_work: u64,
            control: bounded_zero_closure::ObserverControl,
            checkpoints: usize,
            stopped: bool,
        }

        impl bounded_zero_closure::Observer for StopAt {
            fn checkpoint(
                &mut self,
                checkpoint: bounded_zero_closure::Checkpoint,
            ) -> bounded_zero_closure::ObserverControl {
                self.checkpoints += 1;
                if checkpoint.phase == self.phase && checkpoint.completed_work >= self.minimum_work
                {
                    self.stopped = true;
                    self.control
                } else {
                    bounded_zero_closure::ObserverControl::Continue
                }
            }
        }

        fn controlled(
            prepared: &GeometricConstraintSetV1<'_>,
            limits: bounded_zero_closure::Limits,
            observer: &mut impl bounded_zero_closure::Observer,
        ) -> ConstraintPreflightV1 {
            preflight_direct_conflicts_with_zero_closure_controls_v1(prepared, limits, observer)
        }

        let fixture = Fixture::new();
        let records = bounded_zero_closure_records(&fixture, false, 1.0, 1.0);
        let prepared = prepare(&fixture, &document(records.clone())).unwrap();
        let exact_work =
            bounded_zero_closure::required_work(fixture.pattern.edges.len(), records.len())
                .unwrap();
        let exact_storage = records.len() * 32;

        let mut noop = bounded_zero_closure::NoopObserver;
        assert!(matches!(
            controlled(
                &prepared,
                bounded_zero_closure::Limits {
                    max_work: exact_work - 1,
                    ..bounded_zero_closure::Limits::default()
                },
                &mut noop,
            ),
            ConstraintPreflightV1::Unknown {
                reason: GeometricConstraintUnknownReasonV1::WorkLimitExceeded,
                ..
            }
        ));
        assert!(matches!(
            controlled(
                &prepared,
                bounded_zero_closure::Limits {
                    max_work: exact_work,
                    ..bounded_zero_closure::Limits::default()
                },
                &mut noop,
            ),
            ConstraintPreflightV1::DirectConflict { .. }
        ));
        assert!(matches!(
            controlled(
                &prepared,
                bounded_zero_closure::Limits {
                    max_storage_units: exact_storage - 1,
                    ..bounded_zero_closure::Limits::default()
                },
                &mut noop,
            ),
            ConstraintPreflightV1::Unknown {
                reason: GeometricConstraintUnknownReasonV1::StorageLimitExceeded,
                ..
            }
        ));
        assert!(matches!(
            controlled(
                &prepared,
                bounded_zero_closure::Limits {
                    max_storage_units: exact_storage,
                    ..bounded_zero_closure::Limits::default()
                },
                &mut noop,
            ),
            ConstraintPreflightV1::DirectConflict { .. }
        ));

        for (phase, minimum_work, control, expected_reason) in [
            (
                bounded_zero_closure::Phase::Start,
                0,
                bounded_zero_closure::ObserverControl::Cancelled,
                GeometricConstraintUnknownReasonV1::Cancelled,
            ),
            (
                bounded_zero_closure::Phase::ProofSearch,
                1,
                bounded_zero_closure::ObserverControl::DeadlineReached,
                GeometricConstraintUnknownReasonV1::DeadlineReached,
            ),
        ] {
            let mut stop = StopAt {
                phase,
                minimum_work,
                control,
                checkpoints: 0,
                stopped: false,
            };
            assert!(matches!(
                controlled(
                    &prepared,
                    bounded_zero_closure::Limits::default(),
                    &mut stop,
                ),
                ConstraintPreflightV1::Unknown {
                    reason,
                    ..
                } if reason == expected_reason
            ));
            assert!(stop.stopped);
            assert!(stop.checkpoints > 0);
        }

        let mut expanded_pattern = fixture.pattern.clone();
        expanded_pattern.edges.extend((0..140).map(|_| Edge {
            id: EdgeId::new(),
            start: fixture.vertices[0],
            end: fixture.vertices[1],
            kind: EdgeKind::Auxiliary,
        }));
        let expanded = prepare_geometric_constraints_v1(
            &expanded_pattern,
            &document(records),
            GeometricConstraintLimitsV1::default(),
        )
        .unwrap();
        let mut mid_scan = StopAt {
            phase: bounded_zero_closure::Phase::SourcePatternScan,
            minimum_work: 128,
            control: bounded_zero_closure::ObserverControl::Cancelled,
            checkpoints: 0,
            stopped: false,
        };
        assert!(matches!(
            controlled(
                &expanded,
                bounded_zero_closure::Limits::default(),
                &mut mid_scan,
            ),
            ConstraintPreflightV1::Unknown {
                reason: GeometricConstraintUnknownReasonV1::Cancelled,
                ..
            }
        ));
        assert!(mid_scan.stopped);
    }

    #[test]
    fn bounded_subset_cancellation_and_preflight_work_limits_fail_closed() {
        struct CancelAt {
            completed: usize,
            checkpoints: usize,
        }
        impl BoundedDirectMusObserverV1 for CancelAt {
            fn should_cancel(&mut self, completed_oracle_calls: usize) -> bool {
                self.checkpoints += 1;
                completed_oracle_calls >= self.completed
            }
        }

        let fixture = Fixture::new();
        let records = bounded_zero_closure_records(&fixture, false, 1.0, 1.0);
        let prepared = prepare(&fixture, &document(records.clone())).unwrap();
        let mut immediate = CancelAt {
            completed: 0,
            checkpoints: 0,
        };
        assert_eq!(
            find_bounded_direct_mus_with_observer_v1(&prepared, &mut immediate),
            BoundedDirectMusV1::Unknown { oracle_calls: 0 }
        );
        assert_eq!(immediate.checkpoints, 1);

        let mut after_two = CancelAt {
            completed: 2,
            checkpoints: 0,
        };
        assert_eq!(
            find_bounded_direct_mus_with_observer_v1(&prepared, &mut after_two),
            BoundedDirectMusV1::Unknown { oracle_calls: 2 }
        );

        let limited = prepare_geometric_constraints_v1(
            &fixture.pattern,
            &document(records),
            GeometricConstraintLimitsV1 {
                max_preflight_checks: 3,
                ..GeometricConstraintLimitsV1::default()
            },
        )
        .unwrap();
        assert!(matches!(
            limited.preflight(),
            ConstraintPreflightV1::Unknown {
                reason: GeometricConstraintUnknownReasonV1::WorkLimitExceeded,
                ..
            }
        ));

        let mut oversized = bounded_zero_closure_records(&fixture, false, 1.0, 1.0);
        oversized.extend((oversized.len()..17).map(|_| {
            record(GeometricConstraintKindV1::Horizontal {
                edge: fixture.edges[2],
            })
        }));
        let oversized = prepare(&fixture, &document(oversized)).unwrap();
        let mut resource_precedes_cancel = CancelAt {
            completed: 0,
            checkpoints: 0,
        };
        assert_eq!(
            find_bounded_direct_mus_with_observer_v1(&oversized, &mut resource_precedes_cancel,),
            BoundedDirectMusV1::Unknown { oracle_calls: 0 }
        );
        assert_eq!(resource_precedes_cancel.checkpoints, 0);
    }
}
