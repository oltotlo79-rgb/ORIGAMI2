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
mod unit_parallel_fixed_angle;
mod unit_terminal_two_hop_parallel_angle;
mod unit_two_hop_parallel;

use unit_parallel_fixed_angle::fixed_angle_rejects_zero_cross_binary64_v1;

#[cfg(test)]
pub(crate) use unit_parallel_fixed_angle::is_proven_exact_forty_five_single_unit_parallel_angle_shape_for_test_v1;
#[cfg(test)]
pub(crate) use unit_terminal_two_hop_parallel_angle::{
    charge_work_for_test_v1 as charge_unit_terminal_angle_parallel_work_for_test_v1,
    replace_test_limits_v1 as replace_unit_terminal_angle_parallel_test_limits_v1,
    reserve_storage_for_test_v1 as reserve_unit_terminal_angle_parallel_storage_for_test_v1,
    test_observed_v1 as unit_terminal_angle_parallel_test_observed_v1,
};

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
    /// Production residual roles before unordered persistence normalization.
    ///
    /// Mirror reflection is mathematically symmetric but its binary64
    /// operation order is directional. Direct proofs must therefore anchor the
    /// raw source actually reflected by `constraint_solver::residuals`, while
    /// the normalized records remain the canonical grouping/output boundary.
    raw_mirror_roles: BTreeMap<CanonicalId, [VertexId; 2]>,
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

    /// Clones one prepared record while restoring operand roles whose
    /// production binary64 residual is directional.
    ///
    /// Persistence normalization orders the two mirror vertices so grouping
    /// and wire output remain canonical. The residual evaluator, however,
    /// reflects the raw first vertex toward the raw second vertex. Semantic
    /// deletion witnesses must therefore use this clone instead of evaluating
    /// the normalized operand order.
    pub(crate) fn residual_role_preserving_record(
        &self,
        id: ConstraintId,
    ) -> Option<GeometricConstraintRecordV1> {
        let mut record = self
            .constraints
            .iter()
            .find(|record| record.id == id)?
            .clone();
        if let GeometricConstraintKindV1::MirrorSymmetry { axis_edge, .. } = &record.constraint {
            let axis_edge = *axis_edge;
            let [first_vertex, second_vertex] =
                *self.raw_mirror_roles.get(&id.canonical_bytes())?;
            record.constraint = GeometricConstraintKindV1::MirrorSymmetry {
                first_vertex,
                second_vertex,
                axis_edge,
            };
        }
        Some(record)
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
    /// a graph of consistent positive finite `LengthRatio` records. Forward
    /// arcs use the production binary64 multiplication. Reverse arcs use a
    /// division-free bit-ordered search for the complete conservative
    /// multiplication preimage, preserving underflow plateaus, overflow, and
    /// rounding aliases. Two root-derived paths close at one edge with
    /// disjoint value domains. The canonical witness contains the root
    /// constraint and between three and 255 ratio constraints.
    InconsistentLengthRatioGraphWithFixedLength {
        fixed_edge: EdgeId,
        ratio_constraint_count: u16,
    },
    /// Two distinct edges have independent consistent positive finite
    /// `FixedLength` roots. Each root is propagated through consistent positive
    /// finite `LengthRatio` records using the same forward multiplication hull
    /// and division-free reverse binary64 preimage as the single-root graph
    /// proof. The two paths meet at one exact edge with disjoint conservative
    /// value domains. Replaying only the canonical cause graph in production
    /// direction from either fixed root must remain finite at every reachable
    /// step. The canonical witness contains both fixed constraints and between
    /// two and 254 unique ratio constraints.
    InconsistentLengthRatioGraphBetweenFixedLengths {
        first_fixed_edge: EdgeId,
        second_fixed_edge: EdgeId,
        ratio_constraint_count: u16,
    },
    DifferentFixedLengthsInEqualLengthComponent {
        first_edge: EdgeId,
        second_edge: EdgeId,
        equal_constraint_count: u16,
    },
    /// An exact horizontal edge and an exact vertical edge are joined by
    /// exactly two `Parallel` records through one distinct middle edge. The
    /// vertical terminal also has one consistent bit-exact unit
    /// `FixedLength`.
    ///
    /// The unit terminal removes the otherwise exploitable scale factor from
    /// the second normalized cross. If that residual underflows to zero, the
    /// pinned `libm::hypot` exponent-gap branch makes the middle edge's
    /// magnitude bit-identical to its vertical component. The first
    /// normalized cross then evaluates to signed one or a non-zero-safe
    /// `NaN`, never zero. Longer paths and non-unit terminal lengths retain
    /// the same wire tag for compatibility but remain solver-required.
    PerpendicularOrientationsInParallelComponent {
        horizontal_edge: EdgeId,
        vertical_edge: EdgeId,
        parallel_constraint_count: u16,
    },
    /// Two distinct fixed-angle terminal edges are joined through one distinct
    /// middle edge by exactly two `Parallel` records. The terminal edges each
    /// have one bit-exact unit `FixedLength`, and their common validated
    /// `FixedAngle` is bit-exactly 90 degrees. The canonical cause contains
    /// exactly those five records.
    ///
    /// Unit terminal hypotenuses make each normalized parallel denominator the
    /// middle edge's pinned `libm::hypot` result, so the overflow scale escape
    /// available to the legacy general component rule is unavailable. A
    /// normal middle hypot confines both unit terminals to the same unoriented
    /// line within explicit binary64 error bounds. A subnormal middle hypot
    /// instead forces both rounded cross numerators to exact zero; the uniform
    /// minimum-subnormal lattice then confines their angle to at most 60
    /// degrees or at least 120 degrees. In both cases the production
    /// `atan2(abs(cross), dot)` result is disjoint from the exact-zero
    /// enclosure of the frozen 90-degree residual.
    ///
    /// Non-unit terminals, other angles, direct or longer paths, and any cause
    /// shape other than this exact five-ID core retain this wire tag for
    /// compatibility but remain solver-required.
    NonParallelFixedAngleInParallelComponent {
        vertex: VertexId,
        first_edge: EdgeId,
        second_edge: EdgeId,
        parallel_constraint_count: u16,
    },
    /// Two edges have one parallel record and a fixed-angle residual that
    /// rejects the normalized-cross classes admitted by the accompanying
    /// fixed-length proof.
    ///
    /// The exact 45-degree three-record form needs a bit-exact unit
    /// `FixedLength` on either edge. Multiplication by that unit hypot leaves
    /// the other finite hypot unchanged. A non-zero raw cross that nevertheless
    /// divides to signed zero is then confined to a few minimum subnormals
    /// relative to the dot product, disjoint from the frozen 45-degree
    /// residual's exact-zero enclosure.
    ///
    /// The legacy four-record form retains bit-exact unit lengths on both
    /// edges. Its normalized denominator is exactly one, so it remains sound
    /// for every angle accepted by the frozen zero-cross rejection helper.
    /// Other three-record angles and other length scales remain
    /// solver-required.
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
    /// subset whose production-residual source is forced to the axis start by
    /// an exact horizontal/vertical connector is proven separately. That
    /// four-record proof accepts any consistent positive fixed separation; the
    /// generic three-record form remains solver-required.
    MirrorSymmetryWithPointOnAxisAndFixedSeparation {
        first_vertex: VertexId,
        second_vertex: VertexId,
        axis_edge: EdgeId,
        fixed_separation_edge: EdgeId,
    },
    /// An exact stored 90- or 270-degree rotation maps a nonzero source radius
    /// to a perpendicular target radius. `PointOnLine` places that target on
    /// the same real directed center-to-source pattern edge, so satisfying
    /// both records would collapse the edge. Reversed edges, other angles, and
    /// caller-invented endpoint relations remain solver-required.
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
                raw_mirror_roles: set.raw_mirror_roles.clone(),
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
            raw_mirror_roles: BTreeMap::new(),
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
    let mut raw_mirror_roles = BTreeMap::new();
    constraints.try_reserve_exact(ordered.len()).map_err(|_| {
        GeometricConstraintErrorV1::AllocationFailed {
            resource: GeometricConstraintResourceV1::Constraints,
        }
    })?;
    for record in ordered {
        if record.id.canonical_bytes() == [0; 16] {
            return Err(GeometricConstraintErrorV1::NilConstraintId);
        }
        if let GeometricConstraintKindV1::MirrorSymmetry {
            first_vertex,
            second_vertex,
            ..
        } = &record.constraint
        {
            raw_mirror_roles.insert(record.id.canonical_bytes(), [*first_vertex, *second_vertex]);
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
        raw_mirror_roles,
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

    /// Returns the canonical exact stored 90- or 270-degree witness. For either
    /// exact quarter-turn, a target on the line of the real directed,
    /// nondegenerate center-to-source radius is impossible without collapsing
    /// that radius.
    fn quarter_turn_witness(&self) -> Option<ScalarAssignment> {
        [
            self.by_class[RotationCardinalClass::QuarterTurn.index()],
            self.by_class[RotationCardinalClass::ThreeQuarterTurn.index()],
        ]
        .into_iter()
        .flatten()
        .filter(|assignment| {
            let bits = assignment.value.to_bits();
            bits == 90.0_f64.to_bits() || bits == 270.0_f64.to_bits()
        })
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
                mirror_roles.insert(
                    record.id.canonical_bytes(),
                    set.raw_mirror_roles[&record.id.canonical_bytes()],
                );
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
        unit_parallel_fixed_angle::collect_conflicts_v1(
            &mut conflicts,
            *pair,
            parallel_ids,
            &fixed_lengths,
            &fixed_angles_by_pair,
            &edge_ids,
        );
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
    let has_point_mirror_candidate = mirrors.keys().any(|key| {
        points_on_lines.contains_key(&(key.first, key.axis))
            || points_on_lines.contains_key(&(key.second, key.axis))
    });
    let has_mirror_candidate = !mirrors.is_empty();
    let has_collinear_rotation_candidate = cardinal_rotations
        .values()
        .any(|summary| summary.quarter_turn_witness().is_some())
        && !points_on_lines.is_empty();
    if has_same_role_rotation_candidate
        || has_inverse_role_rotation_candidate
        || has_mirror_candidate
        || has_collinear_rotation_candidate
    {
        let pattern_edges = pattern_edge_index(
            set.source_pattern,
            has_same_role_rotation_candidate
                || has_inverse_role_rotation_candidate
                || has_mirror_candidate,
            has_collinear_rotation_candidate || has_mirror_candidate,
        );
        if has_collinear_rotation_candidate {
            let point_line_witnesses =
                canonical_point_line_witnesses(&pattern_edges, &points_on_lines);
            for (key, summary) in &cardinal_rotations {
                let Some(rotation) = summary.quarter_turn_witness() else {
                    continue;
                };
                let [center_vertex, source_vertex, target_vertex] = rotation_roles[key];
                let Some((point_id, line_edge)) = point_line_witnesses
                    .get(&(
                        target_vertex.canonical_bytes(),
                        center_vertex.canonical_bytes(),
                        source_vertex.canonical_bytes(),
                    ))
                    .copied()
                else {
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
        }
        if has_mirror_candidate {
            let mut fixed_separation_witnesses: BTreeMap<
                VertexPairKey,
                Option<(ConstraintId, EdgeId)>,
            > = BTreeMap::new();
            for (key, mirror_ids) in &mirrors {
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

                // Preserve the legacy generic candidate for test-only
                // quarantine diagnostics. PointOnLine alone does not make its
                // independently rounded mirror residual a direct theorem.
                if has_point_mirror_candidate
                    && let Some(mirror_id) = mirror_ids
                        .iter()
                        .copied()
                        .min_by_key(ConstraintId::canonical_bytes)
                    && let Some(point_id) = [key.first, key.second]
                        .into_iter()
                        .filter_map(|vertex| points_on_lines.get(&(vertex, key.axis)))
                        .flatten()
                        .copied()
                        .min_by_key(ConstraintId::canonical_bytes)
                {
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
                }

                let Some(axis_start) = pattern_edges.axis_starts.get(&key.axis).copied() else {
                    continue;
                };
                let mut exact_anchor_witness: Option<(ConstraintId, ConstraintId, ConstraintId)> =
                    None;
                for candidate_mirror_id in mirror_ids {
                    let [raw_source_vertex, _] =
                        mirror_roles[&candidate_mirror_id.canonical_bytes()];
                    let Some((connector_horizontal_id, connector_vertical_id)) =
                        canonical_horizontal_vertical_edge_witness(
                            &pattern_edges.by_pair,
                            &horizontal,
                            &vertical,
                            VertexPairKey::unordered(axis_start, raw_source_vertex),
                        )
                    else {
                        continue;
                    };
                    let candidate = (
                        *candidate_mirror_id,
                        connector_horizontal_id,
                        connector_vertical_id,
                    );
                    if exact_anchor_witness.is_none_or(|current| {
                        (
                            candidate.0.canonical_bytes(),
                            candidate.1.canonical_bytes(),
                            candidate.2.canonical_bytes(),
                        ) < (
                            current.0.canonical_bytes(),
                            current.1.canonical_bytes(),
                            current.2.canonical_bytes(),
                        )
                    }) {
                        exact_anchor_witness = Some(candidate);
                    }
                }
                let Some((exact_mirror_id, connector_horizontal_id, connector_vertical_id)) =
                    exact_anchor_witness
                else {
                    continue;
                };
                // Horizontal and Vertical on the real connector force the
                // production residual's raw source coordinates to equal the
                // axis start bitwise apart from signed zero. If the axis unit
                // vector cannot be formed, MirrorSymmetry itself is not
                // satisfied. Otherwise both source-origin deltas and therefore
                // the projection are signed zero. Doubling a finite origin is
                // either exact (and subtracting the same source recovers the
                // origin by Sterbenz) or non-finite, which cannot yield a zero
                // mirror residual against a finite target. Thus a zero mirror
                // residual forces the target to the same numeric point, while
                // any consistent positive fixed separation rejects collapse.
                push_conflict(
                    &mut conflicts,
                    DirectConstraintConflictKindV1::
                        MirrorSymmetryWithPointOnAxisAndFixedSeparation {
                            first_vertex: vertex_ids[&key.first],
                            second_vertex: vertex_ids[&key.second],
                            axis_edge: edge_ids[&key.axis],
                            fixed_separation_edge,
                        },
                    [
                        exact_mirror_id,
                        fixed_id,
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

    quarantine_unproven_direct_conflicts_v1(&mut conflicts, &mut unchecked, &set.constraints);

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
        match unit_two_hop_parallel::conflict_v1(
            &parallels,
            &horizontal,
            &vertical,
            &fixed_lengths,
            &edge_ids,
            zero_closure_observer,
        ) {
            Ok(Some(candidate)) if is_proven_direct_conflict_v1(&candidate, &set.constraints) => {
                conflicts.push(candidate);
            }
            Ok(Some(candidate)) => {
                debug_assert!(
                    false,
                    "unit two-hop scanner produced a cause shape outside its proof allowlist"
                );
                unchecked.extend(candidate.constraint_ids);
            }
            Ok(None) => {}
            Err(reason) => {
                return ConstraintPreflightV1::Unknown {
                    reason,
                    unchecked_constraint_ids: canonical_constraint_ids(&set.constraints),
                };
            }
        }
    }

    if conflicts.is_empty() {
        match unit_terminal_two_hop_parallel_angle::conflict_v1(
            &parallels,
            &fixed_angles,
            &fixed_lengths,
            &vertex_ids,
            &edge_ids,
            zero_closure_observer,
        ) {
            Ok(Some(candidate)) if is_proven_direct_conflict_v1(&candidate, &set.constraints) => {
                conflicts.push(candidate);
            }
            Ok(Some(candidate)) => {
                debug_assert!(
                    false,
                    "unit-terminal angle scanner produced a cause shape outside its proof allowlist"
                );
                unchecked.extend(candidate.constraint_ids);
            }
            Ok(None) => {}
            Err(reason) => {
                return ConstraintPreflightV1::Unknown {
                    reason,
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
                debug_assert!(!is_proven_direct_conflict_v1(&candidate, &set.constraints));
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

fn is_proven_direct_conflict_v1(
    candidate: &DirectConstraintConflictV1,
    records: &[GeometricConstraintRecordV1],
) -> bool {
    if matches!(
        &candidate.conflict,
        DirectConstraintConflictKindV1::ParallelWithFixedNonParallelAngle { .. }
    ) {
        return unit_parallel_fixed_angle::
            is_proven_exact_forty_five_single_unit_parallel_angle_shape_v1(candidate, records)
            || unit_parallel_fixed_angle::is_proven_legacy_two_unit_parallel_angle_shape_v1(
                candidate, records,
            );
    }
    if matches!(
        &candidate.conflict,
        DirectConstraintConflictKindV1::PerpendicularOrientationsInParallelComponent { .. }
    ) {
        return unit_two_hop_parallel::is_proven_shape_v1(candidate, records);
    }
    if matches!(
        &candidate.conflict,
        DirectConstraintConflictKindV1::NonParallelFixedAngleInParallelComponent { .. }
    ) {
        return unit_terminal_two_hop_parallel_angle::is_proven_shape_v1(candidate, records);
    }
    if matches!(
        &candidate.conflict,
        DirectConstraintConflictKindV1::RotationalSymmetryWithCollinearRadius { .. }
    ) {
        return candidate.constraint_ids.len() == 2;
    }
    if matches!(
        &candidate.conflict,
        DirectConstraintConflictKindV1::MirrorSymmetryWithPointOnAxisAndFixedSeparation { .. }
    ) {
        // The generic mirror candidate has three records. The four-record
        // subset instead anchors the production residual's raw source vertex
        // to the real axis start with exact horizontal/vertical residuals.
        return candidate.constraint_ids.len() == 4;
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
            | DirectConstraintConflictKindV1::
                InconsistentLengthRatioGraphBetweenFixedLengths { .. }
            | DirectConstraintConflictKindV1::DifferentFixedLengthsInEqualLengthComponent { .. }
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
    records: &[GeometricConstraintRecordV1],
) {
    conflicts.retain(|candidate| {
        if is_proven_direct_conflict_v1(candidate, records) {
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
    axis_ends: BTreeMap<CanonicalId, VertexId>,
}

fn pattern_edge_index(
    pattern: &CreasePattern,
    needs_pair_index: bool,
    needs_id_index: bool,
) -> PatternEdgeIndex {
    let mut by_pair: BTreeMap<VertexPairKey, Vec<EdgeId>> = BTreeMap::new();
    let mut by_id = BTreeMap::new();
    let mut axis_starts = BTreeMap::new();
    let mut axis_ends = BTreeMap::new();
    for edge in &pattern.edges {
        let pair = VertexPairKey::unordered(edge.start, edge.end);
        if needs_pair_index {
            by_pair.entry(pair).or_default().push(edge.id);
        }
        if needs_id_index {
            by_id.insert(edge.id.canonical_bytes(), (edge.id, pair));
            axis_starts.insert(edge.id.canonical_bytes(), edge.start);
            axis_ends.insert(edge.id.canonical_bytes(), edge.end);
        }
    }
    PatternEdgeIndex {
        by_pair,
        by_id,
        axis_starts,
        axis_ends,
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

/// Joins every distinct `(point, line edge)` record group to its real pattern
/// directed start/end pair exactly once.
///
/// The resulting key discards the edge identity only after the exact edge
/// lookup and retains the canonical smallest complete witness for that point
/// and endpoint pair. Thus duplicate real edges remain deterministic without
/// allowing any relation to trigger a repeated scan of the same edge bucket.
fn canonical_point_line_witnesses(
    edges: &PatternEdgeIndex,
    points_on_lines: &BTreeMap<(CanonicalId, CanonicalId), Vec<ConstraintId>>,
) -> BTreeMap<(CanonicalId, CanonicalId, CanonicalId), (ConstraintId, EdgeId)> {
    let mut result: BTreeMap<(CanonicalId, CanonicalId, CanonicalId), (ConstraintId, EdgeId)> =
        BTreeMap::new();
    for ((point_vertex, line_edge), point_ids) in points_on_lines {
        #[cfg(test)]
        record_point_line_join_visit();
        let Some((edge_id, _)) = edges.by_id.get(line_edge).copied() else {
            continue;
        };
        let (Some(start), Some(end)) = (
            edges.axis_starts.get(line_edge),
            edges.axis_ends.get(line_edge),
        ) else {
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
            .entry((
                *point_vertex,
                start.canonical_bytes(),
                end.canonical_bytes(),
            ))
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
        DirectConstraintConflictKindV1::InconsistentLengthRatioGraphBetweenFixedLengths {
            first_fixed_edge,
            second_fixed_edge,
            ratio_constraint_count,
        } => (
            23,
            first_fixed_edge.canonical_bytes(),
            second_fixed_edge.canonical_bytes(),
            u128::from(*ratio_constraint_count).to_be_bytes(),
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
#[path = "constraints/tests.rs"]
mod tests;
