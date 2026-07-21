//! Deterministic, renderer-independent rigid kinematics for tree fold graphs.
//!
//! The public API is added after its fail-closed contract tests.
//!
//! Rigid transforms are observation-only values with no public raw-matrix
//! constructor.
//!
//! ```compile_fail
//! use ori_kinematics::{Point3, RigidTransform};
//!
//! let _forged = RigidTransform {
//!     rotation: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
//!     translation: Point3::new(0.0, 0.0, 0.0).unwrap(),
//! };
//! ```
//!
//! Transforms deliberately do not implement persistence traits.
//!
//! ```compile_fail
//! use ori_kinematics::RigidTransform;
//!
//! fn require_serialize<T: serde::Serialize>() {}
//! require_serialize::<RigidTransform>();
//! ```
//!
//! Caller-provided observation poses cannot be passed where a native material
//! pose is required.
//!
//! ```compile_fail
//! use ori_kinematics::{MaterialTreePose, ObservationTreePose};
//!
//! fn material_only(_: MaterialTreePose) {}
//! fn reject_observation(pose: ObservationTreePose) {
//!     material_only(pose);
//! }
//! ```
//!
//! Material poses retain private issuer provenance and are not persistence
//! payloads.
//!
//! ```compile_fail
//! use ori_kinematics::MaterialTreePose;
//!
//! fn require_serialize<T: serde::Serialize>() {}
//! require_serialize::<MaterialTreePose>();
//! ```
//!
//! Native material-face boundary views retain private source provenance. They
//! cannot be constructed from matching public identifiers and are not
//! persistence payloads.
//!
//! ```compile_fail
//! use ori_kinematics::MaterialFaceBoundary;
//!
//! fn forge<'a>() -> MaterialFaceBoundary<'a> {
//!     MaterialFaceBoundary {
//!         source: todo!(),
//!         index: 0,
//!     }
//! }
//! ```
//!
//! ```compile_fail
//! use ori_kinematics::MaterialFaceBoundary;
//!
//! fn require_serialize<T: serde::Serialize>() {}
//! require_serialize::<MaterialFaceBoundary<'static>>();
//! ```

#![forbid(unsafe_code)]

mod graph;
mod interval;
mod schedule;
mod transform;
mod tree;

use ori_domain::{EdgeId, FaceId};
use thiserror::Error;

pub use graph::{
    CandidateFaceTransform, CanonicalCycleBasisV1, ClosedMaterialHingeGraphPose,
    CycleBasisLimitsV1, DyadicIntervalClosureErrorV1, DyadicIntervalClosureLimitsV1,
    DyadicMaterialHingeIntervalClosureCertificateV1,
    MATERIAL_HINGE_INTERVAL_CLOSURE_CERTIFICATE_VERSION_V1, MaterialHingeClosureCertificate,
    MaterialHingeClosureResidual, MaterialHingeGraphAudit,
    MaterialHingeIntervalClosureCertificateV1, SimultaneousCycleBasisClosureCertificateV1,
    enumerate_even_single_vertex_opposite_pairs_v1, theta_opposite_pair_cycle_closure_premises_v1,
};
pub use interval::{
    IntervalRigidTransformV1, IntervalRotationMatrixV1, OutwardIntervalErrorV1, OutwardIntervalV1,
    atan_interval_v1, sin_cos_degrees_interval_v1,
};
pub use schedule::{
    CanonicalCycleScheduleV1, CycleScheduleEntryInputV1, CycleScheduleLimitsV1,
    CycleSchedulePrepareErrorV1, GeneratedMultiHingePathCandidateV1, HalfAngleDomainV1,
    HalfAngleRationalEntryInputV1, MultiHingePathCandidateErrorV1, MultiHingePathCandidateLimitsV1,
    PoleFreeBernsteinCertificateV1, PreparedHalfAngleRationalEntryV1, RationalCoefficientV1,
    admit_canonical_multi_hinge_path_candidate_v1,
    evaluate_half_angle_rational_degrees_interval_v1,
    evaluate_half_angle_rational_derivative_interval_v1, evaluate_pole_free_atan2_interval_v1,
    evaluate_pole_free_rational_dyadic_v1, evaluate_pole_free_rational_interval_v1,
    generate_bounded_degree_four_kawasaki_path_candidate_v1,
    generate_kawasaki_120_120_60_60_path_candidate_v1,
    generate_linear_multi_hinge_path_candidate_v1, prepare_pole_free_bernstein_certificate_v1,
};
pub use transform::{Point3, RigidTransform, deterministic_sin_cos_degrees};
pub use tree::{
    BoundMaterialTreePose, CALLER_EMBEDDING_OBSERVATION_MODEL_ID, CanonicalHingeAngles, HingeAngle,
    MATERIAL_TREE_KINEMATICS_MODEL_ID, MaterialFaceBoundary, MaterialHingeGraphGeometry,
    MaterialHingePairCanonicalInputV1, MaterialHingePairProjectionV1, MaterialTreeKinematicsModel,
    MaterialTreePose, ObservationTreeKinematicsModel, ObservationTreePose, TreeHinge,
    TreeKinematicsLimits, VertexPosition3, assignment_signed_angle_degrees_v1,
    prepare_material_hinge_pair_projection_v1, revalidate_material_hinge_pair_projection_v1,
};

/// A fail-closed error produced while preparing or solving tree kinematics.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum KinematicsError {
    #[error("the source geometry cannot be represented by finite rigid kinematics")]
    UnrepresentableGeometry,
    #[error("the supplied topology is invalid or unsupported")]
    UnsupportedTopology,
    #[error("kinematics work exceeds the configured resource limit")]
    ResourceLimitExceeded,
    #[error("hinge {edge:?} has a non-finite angle")]
    NonFiniteHingeAngle { edge: EdgeId },
    #[error("hinge {edge:?} has an angle outside 0 through 180 degrees")]
    HingeAngleOutOfRange { edge: EdgeId },
    #[error("hinge {edge:?} appears more than once in the angle vector")]
    DuplicateHingeAngle { edge: EdgeId },
    #[error("hinge angle order is not canonical: {previous_edge:?} before {edge:?}")]
    NonCanonicalHingeAngles { previous_edge: EdgeId, edge: EdgeId },
    #[error("the complete angle vector is missing hinge {edge:?}")]
    MissingHingeAngle { edge: EdgeId },
    #[error("the complete angle vector has extra hinge {edge:?}")]
    ExtraHingeAngle { edge: EdgeId },
    #[error("the angle vector refers to unknown hinge {edge:?}")]
    UnknownHingeAngle { edge: EdgeId },
    #[error("a non-planar tree pose requires a fixed face")]
    MissingFixedFace,
    #[error("fixed face {face:?} does not belong to the tree")]
    UnknownFixedFace { face: FaceId },
    #[error("planar pose unexpectedly fixed face {face:?}")]
    UnexpectedFixedFace { face: FaceId },
    #[error("the material pose was issued by a different kinematics model instance")]
    MaterialPoseIssuerMismatch,
}
