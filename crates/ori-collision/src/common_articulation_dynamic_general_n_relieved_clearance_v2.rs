//! Direct dynamic general-N whole-parent relieved clearance.
//!
//! This additive facade derives its shared-pair registry internally and then
//! fully replays the dynamic bridge, ordinary-pair interval proof, shared
//! relief proof, and whole-parent aggregation on every issue or revalidation.
//! Its opaque certificate is evidence only: it grants no project mutation,
//! Apply, viewer, or export authority.
//!
//! ```compile_fail
//! use ori_collision::CommonArticulationDynamicGeneralNRelievedClearanceCertificateV2;
//!
//! fn require_clone<T: Clone>() {}
//! require_clone::<CommonArticulationDynamicGeneralNRelievedClearanceCertificateV2>();
//! ```
//!
//! ```compile_fail
//! use ori_collision::CommonArticulationDynamicGeneralNRelievedClearanceCertificateV2;
//!
//! fn require_serialize<T: serde::Serialize>() {}
//! require_serialize::<CommonArticulationDynamicGeneralNRelievedClearanceCertificateV2>();
//! ```
//!
//! ```compile_fail
//! use ori_collision::CommonArticulationDynamicGeneralNRelievedClearanceCertificateV2;
//!
//! fn require_deref<T: std::ops::Deref>() {}
//! require_deref::<CommonArticulationDynamicGeneralNRelievedClearanceCertificateV2>();
//! ```
//!
//! ```compile_fail
//! use ori_collision::CommonArticulationDynamicGeneralNRelievedClearanceCertificateV2;
//!
//! fn fabricate() -> CommonArticulationDynamicGeneralNRelievedClearanceCertificateV2 {
//!     CommonArticulationDynamicGeneralNRelievedClearanceCertificateV2 {}
//! }
//! ```
//!
//! ```compile_fail
//! use ori_collision::{
//!     CommonArticulationClearancePrerequisiteV1,
//!     CommonArticulationDynamicGeneralNRelievedClearanceCertificateV2,
//! };
//!
//! fn convert(value: CommonArticulationDynamicGeneralNRelievedClearanceCertificateV2) {
//!     let _: CommonArticulationClearancePrerequisiteV1 = value.into();
//! }
//! ```
//!
//! ```compile_fail
//! use ori_collision::{
//!     CommonArticulationDynamicClosureClearancePrerequisiteV2,
//!     CommonArticulationDynamicGeneralNRelievedClearanceCertificateV2,
//! };
//!
//! fn convert(value: CommonArticulationDynamicGeneralNRelievedClearanceCertificateV2) {
//!     let _: CommonArticulationDynamicClosureClearancePrerequisiteV2 = value.into();
//! }
//! ```
//!
//! ```compile_fail
//! use ori_collision::{
//!     CommonArticulationDynamicGeneralNRelievedClearanceCertificateV2,
//!     CommonArticulationProfileBoundWholeParentPositiveThicknessCertificateV2,
//! };
//!
//! fn convert(value: CommonArticulationDynamicGeneralNRelievedClearanceCertificateV2) {
//!     let _: CommonArticulationProfileBoundWholeParentPositiveThicknessCertificateV2 =
//!         value.into();
//! }
//! ```
//!
//! ```compile_fail
//! use ori_collision::{
//!     CommonArticulationDynamicGeneralNRelievedClearanceCertificateV2,
//!     NativeHingeReliefPrerequisiteV1,
//! };
//!
//! fn convert(value: CommonArticulationDynamicGeneralNRelievedClearanceCertificateV2) {
//!     let _: NativeHingeReliefPrerequisiteV1 = value.into();
//! }
//! ```
//!
//! ```compile_fail
//! use ori_collision::CommonArticulationDynamicGeneralNRelievedClearanceCertificateV2;
//!
//! fn expose(value: &CommonArticulationDynamicGeneralNRelievedClearanceCertificateV2) {
//!     let _ = value.raw_evidence_v2();
//! }
//! ```

use ori_domain::FaceId;
use ori_kinematics::{
    CanonicalCycleScheduleV1, CanonicalMaterialEdgeBlockDecompositionV2,
    ClosedMaterialHingeGraphPose, CommonArticulationDynamicClosureBridgeV2,
    CommonArticulationPoseAuthorityV2, CommonArticulationResourceProfileV2,
    MaterialHingeGraphAudit, MaterialHingeGraphGeometry,
};
use thiserror::Error;

use crate::dynamic_general_n_positive_thickness_v2::public_adapter;
use crate::{HingeReliefPolicyRecordV1, VertexReliefPolicyRecordV1};

mod limits;

pub use limits::{
    CommonArticulationDynamicGeneralNOrdinaryIntervalLimitsV2,
    CommonArticulationDynamicGeneralNReliefAggregateLimitsV2,
    CommonArticulationDynamicGeneralNRelievedClearanceLimitsV2,
};

/// Stable identifier for the direct relieved-clearance certificate.
pub const COMMON_ARTICULATION_DYNAMIC_GENERAL_N_RELIEVED_CLEARANCE_MODEL_ID_V2: &str =
    "common_articulation_dynamic_general_n_relieved_clearance_v2";

/// Cooperative stop requested during issue or revalidation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommonArticulationDynamicGeneralNRelievedClearanceStopV2 {
    Cancelled,
    DeadlineExceeded,
}

/// Fail-closed issue or revalidation error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum CommonArticulationDynamicGeneralNRelievedClearanceErrorV2 {
    #[error("the direct relieved-clearance input is malformed")]
    InvalidInput,
    #[error("the direct relieved-clearance input exceeds its finite resource envelope")]
    ResourceLimit,
    #[error("a shared pair has unsupported topology")]
    UnsupportedSharedTopology,
    #[error("shared-feature relief was not proven over the complete path")]
    UnprovenSharedRelief,
    #[error("ordinary-pair clearance was not proven over the complete path")]
    OrdinaryProofUnavailable,
    #[error("the direct relieved-clearance certificate does not match the live input")]
    CertificateBindingMismatch,
    #[error("the direct relieved-clearance operation was cancelled")]
    Cancelled,
    #[error("the direct relieved-clearance operation deadline elapsed")]
    DeadlineExceeded,
}

/// Complete live input for direct whole-parent relieved clearance.
///
/// The caller supplies relief policy records, but never a private face-pair
/// registry. Pair classification is derived from `geometry` inside the sealed
/// adapter on every proof run.
#[derive(Clone, Copy)]
pub struct CommonArticulationDynamicGeneralNRelievedClearanceInputV2<'a> {
    pub geometry: &'a MaterialHingeGraphGeometry,
    pub audit: &'a MaterialHingeGraphAudit,
    pub pose: &'a ClosedMaterialHingeGraphPose,
    pub parent_fixed_face: FaceId,
    pub parent_schedule: &'a CanonicalCycleScheduleV1,
    pub decomposition: &'a CanonicalMaterialEdgeBlockDecompositionV2,
    pub common_pose: &'a CommonArticulationPoseAuthorityV2,
    pub profile: &'a CommonArticulationResourceProfileV2,
    pub dynamic_closure_bridge: &'a CommonArticulationDynamicClosureBridgeV2,
    pub paper_thickness_mm: f64,
    pub closure_tolerance: f64,
    pub hinge_policies: &'a [HingeReliefPolicyRecordV1],
    pub vertex_policies: &'a [VertexReliefPolicyRecordV1],
    pub limits: CommonArticulationDynamicGeneralNRelievedClearanceLimitsV2,
}

/// Complete live input required for certificate replay.
///
/// No issue-time policy or live source is silently retained; every field is
/// submitted again and the entire private proof is rerun before comparison.
#[derive(Clone, Copy)]
pub struct CommonArticulationDynamicGeneralNRelievedClearanceRevalidationInputV2<'a> {
    pub geometry: &'a MaterialHingeGraphGeometry,
    pub audit: &'a MaterialHingeGraphAudit,
    pub pose: &'a ClosedMaterialHingeGraphPose,
    pub parent_fixed_face: FaceId,
    pub parent_schedule: &'a CanonicalCycleScheduleV1,
    pub decomposition: &'a CanonicalMaterialEdgeBlockDecompositionV2,
    pub common_pose: &'a CommonArticulationPoseAuthorityV2,
    pub profile: &'a CommonArticulationResourceProfileV2,
    pub dynamic_closure_bridge: &'a CommonArticulationDynamicClosureBridgeV2,
    pub paper_thickness_mm: f64,
    pub closure_tolerance: f64,
    pub hinge_policies: &'a [HingeReliefPolicyRecordV1],
    pub vertex_policies: &'a [VertexReliefPolicyRecordV1],
    pub limits: CommonArticulationDynamicGeneralNRelievedClearanceLimitsV2,
}

/// Opaque direct certificate for dynamic general-N relieved clearance.
///
/// It has private fields and deliberately implements neither `Clone`, serde,
/// `Deref`, conversion to another authority, nor raw-evidence access.
#[repr(transparent)]
pub struct CommonArticulationDynamicGeneralNRelievedClearanceCertificateV2 {
    evidence: public_adapter::DirectClearanceEvidenceV2,
}

impl std::fmt::Debug for CommonArticulationDynamicGeneralNRelievedClearanceCertificateV2 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CommonArticulationDynamicGeneralNRelievedClearanceCertificateV2")
            .field("model", &self.model_id_v2())
            .field("actual_block_count", &self.actual_block_count_v2())
            .field("total_face_pairs", &self.total_face_pairs_v2())
            .finish_non_exhaustive()
    }
}

impl CommonArticulationDynamicGeneralNRelievedClearanceCertificateV2 {
    #[must_use]
    pub const fn model_id_v2(&self) -> &'static str {
        COMMON_ARTICULATION_DYNAMIC_GENERAL_N_RELIEVED_CLEARANCE_MODEL_ID_V2
    }

    #[must_use]
    pub const fn actual_block_count_v2(&self) -> usize {
        self.evidence.actual_block_count_v2()
    }

    #[must_use]
    pub const fn total_face_pairs_v2(&self) -> usize {
        self.evidence.total_face_pairs_v2()
    }

    #[must_use]
    pub const fn ordinary_face_pairs_v2(&self) -> usize {
        self.evidence.ordinary_face_pairs_v2()
    }

    #[must_use]
    pub const fn shared_hinge_pairs_v2(&self) -> usize {
        self.evidence.shared_hinge_pairs_v2()
    }

    #[must_use]
    pub const fn shared_vertex_pairs_v2(&self) -> usize {
        self.evidence.shared_vertex_pairs_v2()
    }

    #[must_use]
    pub const fn aggregate_peak_bytes_upper_bound_v2(&self) -> usize {
        self.evidence.aggregate_peak_bytes_v2()
    }

    #[must_use]
    pub const fn whole_parent_positive_thickness_proven_v2(&self) -> bool {
        true
    }

    /// Crate-private theorem seal for a stricter downstream proof promotion.
    /// It exposes aggregate boundary counts only inside `ori-collision`; no
    /// dyadic leaf descriptor or partition digest crosses the public facade.
    pub(crate) const fn closed_dyadic_domain_boundary_coverage_seal_v2(
        &self,
    ) -> public_adapter::ClosedDyadicDomainBoundaryCoverageV2 {
        self.evidence.closed_domain_boundary_coverage_v2()
    }

    /// Checks the complete replay resource policy without running geometry.
    /// This is crate-private so downstream promotion can reject policy drift
    /// before entering an expensive proof while exposing no retained limits.
    pub(crate) fn replay_limits_match_v2(
        &self,
        limits: CommonArticulationDynamicGeneralNRelievedClearanceLimitsV2,
    ) -> bool {
        self.evidence.replay_limits_match_v2(limits)
    }

    /// Maximum aggregate peak admitted by the retained exact replay policy.
    pub(crate) const fn replay_aggregate_peak_cap_v2(&self) -> usize {
        self.evidence.replay_aggregate_peak_cap_v2()
    }

    pub fn revalidate_v2(
        &self,
        input: CommonArticulationDynamicGeneralNRelievedClearanceRevalidationInputV2<'_>,
    ) -> Result<(), CommonArticulationDynamicGeneralNRelievedClearanceErrorV2> {
        self.revalidate_with_checkpoint_v2(input, || Ok(()))
    }

    pub fn revalidate_with_checkpoint_v2(
        &self,
        input: CommonArticulationDynamicGeneralNRelievedClearanceRevalidationInputV2<'_>,
        mut checkpoint: impl FnMut() -> Result<
            (),
            CommonArticulationDynamicGeneralNRelievedClearanceStopV2,
        >,
    ) -> Result<(), CommonArticulationDynamicGeneralNRelievedClearanceErrorV2> {
        let candidate = prove_private_v2(input.into_issue_input_v2(), &mut checkpoint)?;
        checkpoint().map_err(map_stop_v2)?;
        if !self.evidence.matches_v2(&candidate) {
            return Err(
                CommonArticulationDynamicGeneralNRelievedClearanceErrorV2::CertificateBindingMismatch,
            );
        }
        checkpoint().map_err(map_stop_v2)
    }

    #[must_use]
    pub const fn authorizes_project_mutation(&self) -> bool {
        false
    }

    #[must_use]
    pub const fn authorizes_apply(&self) -> bool {
        false
    }

    #[must_use]
    pub const fn authorizes_viewer(&self) -> bool {
        false
    }

    #[must_use]
    pub const fn authorizes_export(&self) -> bool {
        false
    }
}

impl<'a> CommonArticulationDynamicGeneralNRelievedClearanceRevalidationInputV2<'a> {
    fn into_issue_input_v2(self) -> CommonArticulationDynamicGeneralNRelievedClearanceInputV2<'a> {
        CommonArticulationDynamicGeneralNRelievedClearanceInputV2 {
            geometry: self.geometry,
            audit: self.audit,
            pose: self.pose,
            parent_fixed_face: self.parent_fixed_face,
            parent_schedule: self.parent_schedule,
            decomposition: self.decomposition,
            common_pose: self.common_pose,
            profile: self.profile,
            dynamic_closure_bridge: self.dynamic_closure_bridge,
            paper_thickness_mm: self.paper_thickness_mm,
            closure_tolerance: self.closure_tolerance,
            hinge_policies: self.hinge_policies,
            vertex_policies: self.vertex_policies,
            limits: self.limits,
        }
    }
}

/// Proves direct dynamic general-N whole-parent relieved clearance.
pub fn prove_common_articulation_dynamic_general_n_relieved_clearance_v2(
    input: CommonArticulationDynamicGeneralNRelievedClearanceInputV2<'_>,
) -> Result<
    CommonArticulationDynamicGeneralNRelievedClearanceCertificateV2,
    CommonArticulationDynamicGeneralNRelievedClearanceErrorV2,
> {
    prove_common_articulation_dynamic_general_n_relieved_clearance_with_checkpoint_v2(input, || {
        Ok(())
    })
}

/// As [`prove_common_articulation_dynamic_general_n_relieved_clearance_v2`],
/// with cooperative cancellation and deadline checkpoints.
pub fn prove_common_articulation_dynamic_general_n_relieved_clearance_with_checkpoint_v2(
    input: CommonArticulationDynamicGeneralNRelievedClearanceInputV2<'_>,
    mut checkpoint: impl FnMut() -> Result<(), CommonArticulationDynamicGeneralNRelievedClearanceStopV2>,
) -> Result<
    CommonArticulationDynamicGeneralNRelievedClearanceCertificateV2,
    CommonArticulationDynamicGeneralNRelievedClearanceErrorV2,
> {
    let evidence = prove_private_v2(input, &mut checkpoint)?;
    checkpoint().map_err(map_stop_v2)?;
    Ok(CommonArticulationDynamicGeneralNRelievedClearanceCertificateV2 { evidence })
}

fn prove_private_v2(
    input: CommonArticulationDynamicGeneralNRelievedClearanceInputV2<'_>,
    checkpoint: &mut impl FnMut()
        -> Result<(), CommonArticulationDynamicGeneralNRelievedClearanceStopV2>,
) -> Result<
    public_adapter::DirectClearanceEvidenceV2,
    CommonArticulationDynamicGeneralNRelievedClearanceErrorV2,
> {
    public_adapter::prove_with_checkpoint_v2(input, || checkpoint().map_err(map_public_stop_v2))
        .map_err(map_adapter_error_v2)
}

const fn map_public_stop_v2(
    stop: CommonArticulationDynamicGeneralNRelievedClearanceStopV2,
) -> public_adapter::AdapterStopV2 {
    match stop {
        CommonArticulationDynamicGeneralNRelievedClearanceStopV2::Cancelled => {
            public_adapter::AdapterStopV2::Cancelled
        }
        CommonArticulationDynamicGeneralNRelievedClearanceStopV2::DeadlineExceeded => {
            public_adapter::AdapterStopV2::DeadlineExceeded
        }
    }
}

const fn map_stop_v2(
    stop: CommonArticulationDynamicGeneralNRelievedClearanceStopV2,
) -> CommonArticulationDynamicGeneralNRelievedClearanceErrorV2 {
    match stop {
        CommonArticulationDynamicGeneralNRelievedClearanceStopV2::Cancelled => {
            CommonArticulationDynamicGeneralNRelievedClearanceErrorV2::Cancelled
        }
        CommonArticulationDynamicGeneralNRelievedClearanceStopV2::DeadlineExceeded => {
            CommonArticulationDynamicGeneralNRelievedClearanceErrorV2::DeadlineExceeded
        }
    }
}

const fn map_adapter_error_v2(
    error: public_adapter::AdapterErrorV2,
) -> CommonArticulationDynamicGeneralNRelievedClearanceErrorV2 {
    match error {
        public_adapter::AdapterErrorV2::InvalidInput => {
            CommonArticulationDynamicGeneralNRelievedClearanceErrorV2::InvalidInput
        }
        public_adapter::AdapterErrorV2::ResourceLimit => {
            CommonArticulationDynamicGeneralNRelievedClearanceErrorV2::ResourceLimit
        }
        public_adapter::AdapterErrorV2::UnsupportedSharedTopology => {
            CommonArticulationDynamicGeneralNRelievedClearanceErrorV2::UnsupportedSharedTopology
        }
        public_adapter::AdapterErrorV2::UnprovenSharedRelief => {
            CommonArticulationDynamicGeneralNRelievedClearanceErrorV2::UnprovenSharedRelief
        }
        public_adapter::AdapterErrorV2::OrdinaryProofUnavailable => {
            CommonArticulationDynamicGeneralNRelievedClearanceErrorV2::OrdinaryProofUnavailable
        }
        public_adapter::AdapterErrorV2::Cancelled => {
            CommonArticulationDynamicGeneralNRelievedClearanceErrorV2::Cancelled
        }
        public_adapter::AdapterErrorV2::DeadlineExceeded => {
            CommonArticulationDynamicGeneralNRelievedClearanceErrorV2::DeadlineExceeded
        }
    }
}
