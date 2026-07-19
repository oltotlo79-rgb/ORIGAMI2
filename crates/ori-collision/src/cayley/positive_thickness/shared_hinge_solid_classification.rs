//! Private composition of the complete positive-thickness shared-hinge scan.
//!
//! This module keeps three facts separate:
//!
//! - the exact prism intersection dimension observed independently in
//!   canonical exact `E` and literal affine `F`;
//! - whether the complete intersections were contained by the finite hinge
//!   model and how the two corridor descriptions were reconciled; and
//! - the semantic V2 topology-policy cell that a later reviewed bridge would
//!   have to discharge.
//!
//! The resulting record is diagnostic-only. It is not a production collision
//! decision, a safe-scene certificate, a public DTO, persistence authority,
//! continuous-collision authority, or project-mutation authority. In
//! particular, a positive-volume prism intersection is retained as such even
//! when the finite hinge model explains the complete overlap.
//!
//! For two valid triangular faces connected only by their common hinge, the
//! positive-thickness wedge overlap is geometrically expected to remain in
//! the authenticated finite hinge corridor. The outside-corridor
//! `PositiveVolumeIntersection` branch is nevertheless implemented and
//! regression-tested with explicitly synthetic exact prisms so malformed or
//! future richer inputs fail closed. That synthetic reachability test is not
//! evidence that a production two-face material model can reach the branch.
//! Vertex-sharing V-fold pairs do not have `SharedHingeEdge` topology and
//! belong to the separate shared-vertex classification path.

use std::cmp::Ordering;

use num_rational::BigRational;
use num_traits::Zero;
use ori_domain::{EdgeId, FaceId, VertexId};
use ori_kinematics::BoundMaterialTreePose;

use crate::{
    IntersectionEvidenceV2, TopologyContactDecision, TopologyRelation,
    classify_runtime_topology_contact_v2,
};

use super::direct_f_corridor::{
    DirectFFiniteHingeCorridorAnalysis, DirectFFiniteHingeCorridorCapabilityV1,
    DirectFFiniteHingeCorridorResult, DirectFFiniteHingeCorridorWork,
    revalidate_direct_f_finite_hinge_corridor_v1,
};
use super::ef_boundary::{
    AxisAlignedEfBoundaryCapabilityV1, BoundBinary64FaceTransformBits,
    revalidate_axis_aligned_ef_boundary_v1,
};
use super::exact_e_corridor::{
    ExactEFiniteHingeCorridorAnalysis, ExactEFiniteHingeCorridorCapabilityV1,
    ExactEFiniteHingeCorridorResult, ExactEFiniteHingeCorridorWork,
    ExactEFiniteHingeInteractionKind, revalidate_exact_e_finite_hinge_corridor_v1,
};
use super::exact_prism::ExactPrismWork;
use super::exact_prism::{
    ExactPrebuiltTriangularPrismView, ExactPrismIntersectionKind, ExactPrismIntersectionReport,
    ExactPrismLimits, analyze_exact_prebuilt_prism_pair_with_meter_v1,
};
use super::shared_hinge_corridor_admission::{
    SharedHingeCorridorAdmissionAnalysisV1, SharedHingeCorridorAdmissionCapabilityV1,
    SharedHingeCorridorAdmissionResultV1, SharedHingeCorridorAdmissionWorkV1,
    revalidate_shared_hinge_corridor_admission_v1,
};
use super::shared_hinge_topology_margin::{
    SharedHingeNativeExactTopologyMarginAnalysisV1,
    SharedHingeNativeExactTopologyMarginCapabilityV1, SharedHingeNativeExactTopologyMarginResultV1,
    SharedHingeNativeExactTopologyMarginWorkV1,
    revalidate_shared_hinge_native_exact_topology_margin_v1,
};
use super::*;

const FACE_COUNT: usize = 2;
const HINGE_COUNT: usize = 1;
const UNORDERED_FACE_PAIR_COUNT: usize = 1;
const TRIANGLE_PAIR_COUNT: usize = 1;
const PRISM_SCAN_COUNT: usize = 2;
const UPSTREAM_CAPABILITY_REVALIDATIONS: usize = 5;
const SEALED_PRIOR_WORK_BINDINGS: usize = 3;
const ROOT_BINDINGS: usize = 1;
const ANGLE_BINDINGS: usize = 1;
const FACE_IDENTITY_BINDINGS: usize = FACE_COUNT;
const HINGE_IDENTITY_BINDINGS: usize = 5;
const ENDPOINT_IDENTITY_BINDINGS: usize = 2;
const FACE_TRANSFORM_BIT_BINDINGS: usize = FACE_COUNT * 12;
const HINGE_PARENT_TRANSFORM_BIT_BINDINGS: usize = 12;
const INTERACTION_KIND_BINDINGS: usize = 2;
const POLICY_CELL_BINDINGS: usize = 1;
const CLASSIFICATION_SEAL_BINDINGS: usize = 1;
const MARGIN_COMPONENT_COUNT: usize = 10;
const MAX_INDEPENDENT_CORRIDOR_VERTEX_CHECKS: usize = 240;
const INDEPENDENT_UPSTREAM_CAPABILITY_REVALIDATIONS: usize = 3;
const INDEPENDENT_SEALED_PRIOR_WORK_BINDINGS: usize = 1;

fn independent_corridor_hard_exact_limits() -> CayleyLimits {
    let exact = CayleyLimits::default();
    CayleyLimits {
        max_precision_rounds: 0,
        max_guard_bits: 0,
        max_candidate_bits: 0,
        max_machin_terms_per_series: 0,
        max_trig_terms_per_series: 0,
        max_sqrt_refinements: 0,
        max_interval_operations: 65_536,
        max_shift_bits: exact.max_shift_bits,
        max_intermediate_bits: exact.max_intermediate_bits,
        max_gcd_fallback_calls: 8_192,
        max_gcd_fallback_input_bits: exact.max_gcd_fallback_input_bits,
        max_rational_allocations: 65_536,
        max_rational_allocation_bits: exact.max_rational_allocation_bits,
        max_total_rational_allocation_bits: 268_435_456,
        max_output_bits: 0,
    }
}

/// Caller-non-expandable structural limits for the private composition.
///
/// Exact arithmetic belongs to the already sealed prism/corridor/margin
/// phases. This phase only revalidates identities, complete-scan counters,
/// categorical mappings, and exact rational ordering already retained by the
/// margin capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SharedHingeSolidClassificationLimitsV1 {
    pub(super) max_authenticated_faces: usize,
    pub(super) max_authenticated_hinges: usize,
    pub(super) max_unordered_face_pair_count_calculations: usize,
    pub(super) max_unordered_face_pairs: usize,
    pub(super) max_triangle_pairs: usize,
    pub(super) max_prism_complete_scan_checks: usize,
    pub(super) max_upstream_capability_revalidations: usize,
    pub(super) max_sealed_prior_work_bindings: usize,
    pub(super) max_root_bindings: usize,
    pub(super) max_angle_bindings: usize,
    pub(super) max_face_identity_bindings: usize,
    pub(super) max_hinge_identity_bindings: usize,
    pub(super) max_endpoint_identity_bindings: usize,
    pub(super) max_face_transform_bit_bindings: usize,
    pub(super) max_hinge_parent_transform_bit_bindings: usize,
    pub(super) max_interaction_kind_bindings: usize,
    pub(super) max_policy_cell_bindings: usize,
    pub(super) max_classification_seal_bindings: usize,
    pub(super) max_margin_component_checks: usize,
    pub(super) max_independent_corridor_vertex_checks: usize,
    pub(super) prism: ExactPrismLimits,
    pub(super) corridor_exact: CayleyLimits,
}

impl Default for SharedHingeSolidClassificationLimitsV1 {
    fn default() -> Self {
        Self {
            max_authenticated_faces: FACE_COUNT,
            max_authenticated_hinges: HINGE_COUNT,
            max_unordered_face_pair_count_calculations: 1,
            max_unordered_face_pairs: UNORDERED_FACE_PAIR_COUNT,
            max_triangle_pairs: TRIANGLE_PAIR_COUNT,
            max_prism_complete_scan_checks: PRISM_SCAN_COUNT,
            max_upstream_capability_revalidations: UPSTREAM_CAPABILITY_REVALIDATIONS,
            max_sealed_prior_work_bindings: SEALED_PRIOR_WORK_BINDINGS,
            max_root_bindings: ROOT_BINDINGS,
            max_angle_bindings: ANGLE_BINDINGS,
            max_face_identity_bindings: FACE_IDENTITY_BINDINGS,
            max_hinge_identity_bindings: HINGE_IDENTITY_BINDINGS,
            max_endpoint_identity_bindings: ENDPOINT_IDENTITY_BINDINGS,
            max_face_transform_bit_bindings: FACE_TRANSFORM_BIT_BINDINGS,
            max_hinge_parent_transform_bit_bindings: HINGE_PARENT_TRANSFORM_BIT_BINDINGS,
            max_interaction_kind_bindings: INTERACTION_KIND_BINDINGS,
            max_policy_cell_bindings: POLICY_CELL_BINDINGS,
            max_classification_seal_bindings: CLASSIFICATION_SEAL_BINDINGS,
            max_margin_component_checks: MARGIN_COMPONENT_COUNT,
            max_independent_corridor_vertex_checks: MAX_INDEPENDENT_CORRIDOR_VERTEX_CHECKS,
            prism: ExactPrismLimits::default(),
            corridor_exact: independent_corridor_hard_exact_limits(),
        }
    }
}

impl SharedHingeSolidClassificationLimitsV1 {
    fn projected(self) -> Self {
        let hard = Self::default();
        Self {
            max_authenticated_faces: self
                .max_authenticated_faces
                .min(hard.max_authenticated_faces),
            max_authenticated_hinges: self
                .max_authenticated_hinges
                .min(hard.max_authenticated_hinges),
            max_unordered_face_pair_count_calculations: self
                .max_unordered_face_pair_count_calculations
                .min(hard.max_unordered_face_pair_count_calculations),
            max_unordered_face_pairs: self
                .max_unordered_face_pairs
                .min(hard.max_unordered_face_pairs),
            max_triangle_pairs: self.max_triangle_pairs.min(hard.max_triangle_pairs),
            max_prism_complete_scan_checks: self
                .max_prism_complete_scan_checks
                .min(hard.max_prism_complete_scan_checks),
            max_upstream_capability_revalidations: self
                .max_upstream_capability_revalidations
                .min(hard.max_upstream_capability_revalidations),
            max_sealed_prior_work_bindings: self
                .max_sealed_prior_work_bindings
                .min(hard.max_sealed_prior_work_bindings),
            max_root_bindings: self.max_root_bindings.min(hard.max_root_bindings),
            max_angle_bindings: self.max_angle_bindings.min(hard.max_angle_bindings),
            max_face_identity_bindings: self
                .max_face_identity_bindings
                .min(hard.max_face_identity_bindings),
            max_hinge_identity_bindings: self
                .max_hinge_identity_bindings
                .min(hard.max_hinge_identity_bindings),
            max_endpoint_identity_bindings: self
                .max_endpoint_identity_bindings
                .min(hard.max_endpoint_identity_bindings),
            max_face_transform_bit_bindings: self
                .max_face_transform_bit_bindings
                .min(hard.max_face_transform_bit_bindings),
            max_hinge_parent_transform_bit_bindings: self
                .max_hinge_parent_transform_bit_bindings
                .min(hard.max_hinge_parent_transform_bit_bindings),
            max_interaction_kind_bindings: self
                .max_interaction_kind_bindings
                .min(hard.max_interaction_kind_bindings),
            max_policy_cell_bindings: self
                .max_policy_cell_bindings
                .min(hard.max_policy_cell_bindings),
            max_classification_seal_bindings: self
                .max_classification_seal_bindings
                .min(hard.max_classification_seal_bindings),
            max_margin_component_checks: self
                .max_margin_component_checks
                .min(hard.max_margin_component_checks),
            max_independent_corridor_vertex_checks: self
                .max_independent_corridor_vertex_checks
                .min(hard.max_independent_corridor_vertex_checks),
            prism: self.prism.projected(),
            corridor_exact: project_cayley_limits(self.corridor_exact, hard.corridor_exact),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct SharedHingeSolidClassificationWorkV1 {
    pub(super) authenticated_faces: usize,
    pub(super) authenticated_hinges: usize,
    pub(super) unordered_face_pair_count_calculations: usize,
    pub(super) unordered_face_pairs: usize,
    pub(super) triangle_pairs: usize,
    pub(super) prism_complete_scan_checks: usize,
    pub(super) upstream_capability_revalidations: usize,
    pub(super) sealed_prior_work_bindings: usize,
    pub(super) root_bindings: usize,
    pub(super) angle_bindings: usize,
    pub(super) face_identity_bindings: usize,
    pub(super) hinge_identity_bindings: usize,
    pub(super) endpoint_identity_bindings: usize,
    pub(super) face_transform_bit_bindings: usize,
    pub(super) hinge_parent_transform_bit_bindings: usize,
    pub(super) interaction_kind_bindings: usize,
    pub(super) policy_cell_bindings: usize,
    pub(super) classification_seal_bindings: usize,
    pub(super) margin_component_checks: usize,
    pub(super) independent_corridor_vertex_checks: usize,
    pub(super) independent_exact_prism: ExactPrismWork,
    pub(super) independent_direct_prism: ExactPrismWork,
    pub(super) independent_exact_corridor: CayleyWork,
    pub(super) independent_direct_corridor: CayleyWork,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SharedHingeSolidClassificationErrorV1 {
    ResourceLimitExceeded,
}

/// The exact geometric dimension produced by both complete prism scans.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SharedHingeSolidIntersectionDimensionV1 {
    Point,
    Line,
    BoundaryArea,
    PositiveVolume,
}

/// How exact `E` and literal `F` were reconciled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SharedHingeCorridorReconciliationV1 {
    BitExactIdenticalCorridor,
    NativeExactTopologyMargin,
    ExactSharedFeatureGeometry,
    BothOutsideFiniteCorridors,
}

/// Complete private classification vocabulary.
///
/// The two `Allowed*` names describe the finite hinge model's local semantic
/// category only. They do not grant production-safe authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SharedHingePositiveThicknessPairClassV1 {
    SharedFeatureOnlyContact,
    AllowedFiniteCorridorOverlap,
    AllowedBoundaryContact,
    PositiveVolumeIntersection,
    EvidenceUnavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SharedHingeEvidenceUnavailableReasonV1 {
    PrerequisiteUnavailable,
    EfBoundaryUnavailable,
    ExactIntersectionUnavailable,
    DirectIntersectionUnavailable,
    InteractionKindMismatch,
    CorridorReconciliationUnavailable,
    IncompletePairCoverage,
    AuthorityOrSealMismatch,
    LayerOffsetUnmodeled,
}

/// Frozen semantic relationship to the public 4 x 11 policy table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SharedHingePolicyContractV1 {
    semantic_evidence: IntersectionEvidenceV2,
    baseline_decision: TopologyContactDecision,
}

fn policy_contract(class: SharedHingePositiveThicknessPairClassV1) -> SharedHingePolicyContractV1 {
    let semantic_evidence = match class {
        SharedHingePositiveThicknessPairClassV1::SharedFeatureOnlyContact => {
            IntersectionEvidenceV2::SharedFeatureContact
        }
        SharedHingePositiveThicknessPairClassV1::AllowedFiniteCorridorOverlap => {
            IntersectionEvidenceV2::SharedFeatureThicknessOverlap
        }
        SharedHingePositiveThicknessPairClassV1::AllowedBoundaryContact => {
            IntersectionEvidenceV2::BoundaryAreaContact
        }
        SharedHingePositiveThicknessPairClassV1::PositiveVolumeIntersection => {
            IntersectionEvidenceV2::PositiveVolumeOverlap
        }
        SharedHingePositiveThicknessPairClassV1::EvidenceUnavailable => {
            IntersectionEvidenceV2::Indeterminate
        }
    };
    SharedHingePolicyContractV1 {
        semantic_evidence,
        baseline_decision: classify_runtime_topology_contact_v2(
            TopologyRelation::SharedHingeEdge,
            semantic_evidence,
        ),
    }
}

/// Sealed diagnostic payload. Raw prism evidence is retained independently
/// from the topology-refined semantic evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SharedHingeSolidClassificationSnapshotV1 {
    class: SharedHingePositiveThicknessPairClassV1,
    exact_intersection: SharedHingeSolidIntersectionDimensionV1,
    direct_intersection: SharedHingeSolidIntersectionDimensionV1,
    raw_prism_evidence: IntersectionEvidenceV2,
    semantic_evidence: IntersectionEvidenceV2,
    baseline_decision: TopologyContactDecision,
    reconciliation: SharedHingeCorridorReconciliationV1,
    expected_unordered_face_pairs: usize,
    classified_unordered_face_pairs: usize,
    expected_triangle_pairs: usize,
    classified_triangle_pairs: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ReconciliationWorkSealV1 {
    BitExact(SharedHingeCorridorAdmissionWorkV1),
    Margin(SharedHingeNativeExactTopologyMarginWorkV1),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PriorWorkSealV1 {
    exact_e: ExactEFiniteHingeCorridorWork,
    direct_f: DirectFFiniteHingeCorridorWork,
    reconciliation: ReconciliationWorkSealV1,
}

#[derive(Debug)]
enum ReconciliationAuthorityV1<
    'admission,
    'margin,
    'prerequisite,
    'ef,
    'exact_e_corridor,
    'direct_f_corridor,
    'exact,
    'pose,
> {
    BitExact(
        &'admission SharedHingeCorridorAdmissionCapabilityV1<
            'prerequisite,
            'ef,
            'exact_e_corridor,
            'direct_f_corridor,
            'exact,
            'pose,
        >,
    ),
    Margin(
        &'margin SharedHingeNativeExactTopologyMarginCapabilityV1<
            'prerequisite,
            'ef,
            'exact,
            'pose,
        >,
    ),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BoundHingeAngleBitsV1 {
    edge: EdgeId,
    angle_degrees_bits: u64,
}

/// Borrow-bound, sealed, diagnostic-only complete pair classification.
///
/// This type deliberately implements neither `Clone`, `Copy`, nor
/// serialization and has no public constructor. Even a successfully
/// revalidated value cannot be converted to a production collision decision
/// or a safe certificate by this module.
#[derive(Debug)]
pub(super) struct SharedHingeSolidClassificationRecordV1<
    'admission,
    'margin,
    'prerequisite,
    'ef,
    'exact_e_corridor,
    'direct_f_corridor,
    'exact,
    'pose,
> {
    prerequisite: &'prerequisite AuthenticatedSingleTriangularHingePrerequisitesV1<'exact, 'pose>,
    ef_boundary: &'ef AxisAlignedEfBoundaryCapabilityV1<'prerequisite, 'exact, 'pose>,
    exact_e_corridor:
        &'exact_e_corridor ExactEFiniteHingeCorridorCapabilityV1<'prerequisite, 'ef, 'exact, 'pose>,
    direct_f_corridor: &'direct_f_corridor DirectFFiniteHingeCorridorCapabilityV1<
        'prerequisite,
        'ef,
        'exact_e_corridor,
        'exact,
        'pose,
    >,
    reconciliation_authority: ReconciliationAuthorityV1<
        'admission,
        'margin,
        'prerequisite,
        'ef,
        'exact_e_corridor,
        'direct_f_corridor,
        'exact,
        'pose,
    >,
    exact: &'exact RationalCayleyTreePose<'pose>,
    bound: BoundMaterialTreePose<'pose>,
    paper_thickness_bits: u64,
    fixed_face: FaceId,
    face_ids: [FaceId; FACE_COUNT],
    hinge_edge: EdgeId,
    hinge_parent: FaceId,
    hinge_child: FaceId,
    hinge_endpoint_vertices: [VertexId; 2],
    hinge_angle: BoundHingeAngleBitsV1,
    binary64_face_transforms: [BoundBinary64FaceTransformBits; FACE_COUNT],
    hinge_parent_transform: BoundBinary64FaceTransformBits,
    snapshot: SharedHingeSolidClassificationSnapshotV1,
    prior_work: PriorWorkSealV1,
    work: SharedHingeSolidClassificationWorkV1,
    sealed_snapshot: Option<SharedHingeSolidClassificationSnapshotV1>,
    sealed_prior_work: Option<PriorWorkSealV1>,
    sealed_work: Option<SharedHingeSolidClassificationWorkV1>,
}

impl SharedHingeSolidClassificationRecordV1<'_, '_, '_, '_, '_, '_, '_, '_> {
    #[cfg(test)]
    pub(super) fn class_for_test(&self) -> SharedHingePositiveThicknessPairClassV1 {
        self.snapshot.class
    }

    #[cfg(test)]
    pub(super) fn intersection_for_test(
        &self,
    ) -> (
        SharedHingeSolidIntersectionDimensionV1,
        SharedHingeSolidIntersectionDimensionV1,
    ) {
        (
            self.snapshot.exact_intersection,
            self.snapshot.direct_intersection,
        )
    }

    #[cfg(test)]
    pub(super) fn policy_for_test(
        &self,
    ) -> (
        IntersectionEvidenceV2,
        IntersectionEvidenceV2,
        TopologyContactDecision,
    ) {
        (
            self.snapshot.raw_prism_evidence,
            self.snapshot.semantic_evidence,
            self.snapshot.baseline_decision,
        )
    }

    #[cfg(test)]
    pub(super) fn reconciliation_for_test(&self) -> SharedHingeCorridorReconciliationV1 {
        self.snapshot.reconciliation
    }

    #[cfg(test)]
    pub(super) fn coverage_for_test(&self) -> (usize, usize, usize, usize) {
        (
            self.snapshot.expected_unordered_face_pairs,
            self.snapshot.classified_unordered_face_pairs,
            self.snapshot.expected_triangle_pairs,
            self.snapshot.classified_triangle_pairs,
        )
    }

    #[cfg(test)]
    pub(super) fn toggle_class_for_test(&mut self) {
        self.snapshot.class = match self.snapshot.class {
            SharedHingePositiveThicknessPairClassV1::AllowedBoundaryContact => {
                SharedHingePositiveThicknessPairClassV1::AllowedFiniteCorridorOverlap
            }
            _ => SharedHingePositiveThicknessPairClassV1::AllowedBoundaryContact,
        };
    }

    #[cfg(test)]
    pub(super) fn increment_coverage_for_test(&mut self) {
        self.snapshot.classified_triangle_pairs += 1;
    }

    #[cfg(test)]
    pub(super) fn decrement_coverage_for_test(&mut self) {
        self.snapshot.classified_triangle_pairs -= 1;
    }

    #[cfg(test)]
    pub(super) fn increment_prior_work_for_test(&mut self) {
        self.prior_work.exact_e.authenticated_faces += 1;
    }

    #[cfg(test)]
    pub(super) fn decrement_prior_work_for_test(&mut self) {
        self.prior_work.exact_e.authenticated_faces -= 1;
    }

    #[cfg(test)]
    pub(super) fn increment_record_work_for_test(&mut self) {
        self.work.authenticated_faces += 1;
    }

    #[cfg(test)]
    pub(super) fn decrement_record_work_for_test(&mut self) {
        self.work.authenticated_faces -= 1;
    }

    #[cfg(test)]
    pub(super) fn swap_face_identities_for_test(&mut self) {
        self.face_ids.swap(0, 1);
    }

    #[cfg(test)]
    pub(super) fn swap_endpoint_identities_for_test(&mut self) {
        self.hinge_endpoint_vertices.swap(0, 1);
    }

    #[cfg(test)]
    pub(super) fn flip_face_transform_bit_for_test(&mut self, face: usize, coefficient: usize) {
        if coefficient < 9 {
            self.binary64_face_transforms[face].rotation[coefficient / 3][coefficient % 3] ^= 1;
        } else {
            self.binary64_face_transforms[face].translation[coefficient - 9] ^= 1;
        }
    }

    #[cfg(test)]
    pub(super) fn flip_hinge_parent_transform_bit_for_test(&mut self, coefficient: usize) {
        if coefficient < 9 {
            self.hinge_parent_transform.rotation[coefficient / 3][coefficient % 3] ^= 1;
        } else {
            self.hinge_parent_transform.translation[coefficient - 9] ^= 1;
        }
    }
}

#[derive(Debug)]
pub(super) struct RevalidatedSharedHingeSolidClassificationRecordV1<
    'record,
    'admission,
    'margin,
    'prerequisite,
    'ef,
    'exact_e_corridor,
    'direct_f_corridor,
    'exact,
    'pose,
> {
    pub(super) record: &'record SharedHingeSolidClassificationRecordV1<
        'admission,
        'margin,
        'prerequisite,
        'ef,
        'exact_e_corridor,
        'direct_f_corridor,
        'exact,
        'pose,
    >,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct IndependentCorridorScanV1 {
    vertex_count: usize,
    outside_vertex_count: usize,
    first_outside_vertex_index: Option<usize>,
    every_vertex_on_finite_axis: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct IndependentPrismSummaryV1 {
    kind: ExactPrismIntersectionKind,
    affine_rank: Option<u8>,
    vertex_count: usize,
}

impl IndependentPrismSummaryV1 {
    fn from_report(report: &ExactPrismIntersectionReport) -> Self {
        Self {
            kind: report.kind(),
            affine_rank: report.affine_rank(),
            vertex_count: report.canonical_vertices().len(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct IndependentGeometrySealV1 {
    exact_report: IndependentPrismSummaryV1,
    direct_report: IndependentPrismSummaryV1,
    exact_corridor_scan: IndependentCorridorScanV1,
    direct_corridor_scan: IndependentCorridorScanV1,
}

/// Sealed independent E/F prism classification used only when the earlier
/// containment capability stack cannot explain the interaction.
///
/// A positive-volume result is issued only when both complete prism kernels
/// prove rank three and both complete vertex scans prove extension outside
/// their respective finite corridors. This is diagnostic blocking evidence,
/// not a production collision certificate.
#[derive(Debug)]
pub(super) struct IndependentSharedHingeSolidClassificationRecordV1<
    'margin,
    'prerequisite,
    'ef,
    'exact,
    'pose,
> {
    prerequisite: &'prerequisite AuthenticatedSingleTriangularHingePrerequisitesV1<'exact, 'pose>,
    ef_boundary: &'ef AxisAlignedEfBoundaryCapabilityV1<'prerequisite, 'exact, 'pose>,
    margin: &'margin SharedHingeNativeExactTopologyMarginCapabilityV1<
        'prerequisite,
        'ef,
        'exact,
        'pose,
    >,
    exact: &'exact RationalCayleyTreePose<'pose>,
    bound: BoundMaterialTreePose<'pose>,
    paper_thickness_bits: u64,
    fixed_face: FaceId,
    face_ids: [FaceId; FACE_COUNT],
    hinge_edge: EdgeId,
    hinge_parent: FaceId,
    hinge_child: FaceId,
    hinge_endpoint_vertices: [VertexId; 2],
    hinge_angle: BoundHingeAngleBitsV1,
    binary64_face_transforms: [BoundBinary64FaceTransformBits; FACE_COUNT],
    hinge_parent_transform: BoundBinary64FaceTransformBits,
    exact_report: IndependentPrismSummaryV1,
    direct_report: IndependentPrismSummaryV1,
    exact_corridor_scan: IndependentCorridorScanV1,
    direct_corridor_scan: IndependentCorridorScanV1,
    snapshot: SharedHingeSolidClassificationSnapshotV1,
    margin_work: SharedHingeNativeExactTopologyMarginWorkV1,
    work: SharedHingeSolidClassificationWorkV1,
    sealed_snapshot: Option<SharedHingeSolidClassificationSnapshotV1>,
    sealed_geometry: Option<IndependentGeometrySealV1>,
    sealed_margin_work: Option<SharedHingeNativeExactTopologyMarginWorkV1>,
    sealed_work: Option<SharedHingeSolidClassificationWorkV1>,
}

impl IndependentSharedHingeSolidClassificationRecordV1<'_, '_, '_, '_, '_> {
    #[cfg(test)]
    pub(super) fn class_for_test(&self) -> SharedHingePositiveThicknessPairClassV1 {
        self.snapshot.class
    }

    #[cfg(test)]
    pub(super) fn outside_counts_for_test(&self) -> (usize, usize) {
        (
            self.exact_corridor_scan.outside_vertex_count,
            self.direct_corridor_scan.outside_vertex_count,
        )
    }

    #[cfg(test)]
    pub(super) fn policy_for_test(
        &self,
    ) -> (
        IntersectionEvidenceV2,
        IntersectionEvidenceV2,
        TopologyContactDecision,
    ) {
        (
            self.snapshot.raw_prism_evidence,
            self.snapshot.semantic_evidence,
            self.snapshot.baseline_decision,
        )
    }

    #[cfg(test)]
    pub(super) fn toggle_class_for_test(&mut self) {
        self.snapshot.class = SharedHingePositiveThicknessPairClassV1::EvidenceUnavailable;
    }

    #[cfg(test)]
    pub(super) fn restore_positive_volume_class_for_test(&mut self) {
        self.snapshot.class = SharedHingePositiveThicknessPairClassV1::PositiveVolumeIntersection;
    }

    #[cfg(test)]
    pub(super) fn increment_margin_work_for_test(&mut self) {
        self.margin_work.authenticated_faces += 1;
    }

    #[cfg(test)]
    pub(super) fn decrement_margin_work_for_test(&mut self) {
        self.margin_work.authenticated_faces -= 1;
    }
}

#[derive(Debug)]
pub(super) struct RevalidatedIndependentSharedHingeSolidClassificationRecordV1<
    'record,
    'margin,
    'prerequisite,
    'ef,
    'exact,
    'pose,
> {
    pub(super) record: &'record IndependentSharedHingeSolidClassificationRecordV1<
        'margin,
        'prerequisite,
        'ef,
        'exact,
        'pose,
    >,
}

#[derive(Debug)]
pub(super) enum SharedHingeSolidClassificationResultV1<
    'admission,
    'margin,
    'prerequisite,
    'ef,
    'exact_e_corridor,
    'direct_f_corridor,
    'exact,
    'pose,
> {
    Classified(
        Box<
            SharedHingeSolidClassificationRecordV1<
                'admission,
                'margin,
                'prerequisite,
                'ef,
                'exact_e_corridor,
                'direct_f_corridor,
                'exact,
                'pose,
            >,
        >,
    ),
    IndependentlyClassified(
        Box<
            IndependentSharedHingeSolidClassificationRecordV1<
                'margin,
                'prerequisite,
                'ef,
                'exact,
                'pose,
            >,
        >,
    ),
    EvidenceUnavailable(SharedHingeEvidenceUnavailableReasonV1),
}

#[derive(Debug)]
pub(super) struct SharedHingeSolidClassificationAnalysisV1<
    'admission,
    'margin,
    'prerequisite,
    'ef,
    'exact_e_corridor,
    'direct_f_corridor,
    'exact,
    'pose,
> {
    pub(super) result: SharedHingeSolidClassificationResultV1<
        'admission,
        'margin,
        'prerequisite,
        'ef,
        'exact_e_corridor,
        'direct_f_corridor,
        'exact,
        'pose,
    >,
    pub(super) work: SharedHingeSolidClassificationWorkV1,
}

impl<'admission, 'margin, 'prerequisite, 'ef, 'exact_e_corridor, 'direct_f_corridor, 'exact, 'pose>
    SharedHingeSolidClassificationAnalysisV1<
        'admission,
        'margin,
        'prerequisite,
        'ef,
        'exact_e_corridor,
        'direct_f_corridor,
        'exact,
        'pose,
    >
{
    pub(super) fn sealed_non_authoritative_record_and_work(
        &self,
    ) -> Option<(
        &SharedHingeSolidClassificationRecordV1<
            'admission,
            'margin,
            'prerequisite,
            'ef,
            'exact_e_corridor,
            'direct_f_corridor,
            'exact,
            'pose,
        >,
        &SharedHingeSolidClassificationWorkV1,
    )> {
        let SharedHingeSolidClassificationResultV1::Classified(record) = &self.result else {
            return None;
        };
        if record.work != self.work
            || record.sealed_work.as_ref() != Some(&record.work)
            || record.sealed_snapshot.as_ref() != Some(&record.snapshot)
            || record.sealed_prior_work.as_ref() != Some(&record.prior_work)
        {
            return None;
        }
        Some((record.as_ref(), &record.work))
    }

    pub(super) fn sealed_non_authoritative_independent_record_and_work(
        &self,
    ) -> Option<(
        &IndependentSharedHingeSolidClassificationRecordV1<
            'margin,
            'prerequisite,
            'ef,
            'exact,
            'pose,
        >,
        &SharedHingeSolidClassificationWorkV1,
    )> {
        let SharedHingeSolidClassificationResultV1::IndependentlyClassified(record) = &self.result
        else {
            return None;
        };
        let sealed_geometry = record.sealed_geometry.as_ref()?;
        if record.work != self.work
            || record.sealed_work.as_ref() != Some(&record.work)
            || record.sealed_snapshot.as_ref() != Some(&record.snapshot)
            || sealed_geometry.exact_report != record.exact_report
            || sealed_geometry.direct_report != record.direct_report
            || sealed_geometry.exact_corridor_scan != record.exact_corridor_scan
            || sealed_geometry.direct_corridor_scan != record.direct_corridor_scan
            || record.sealed_margin_work.as_ref() != Some(&record.margin_work)
        {
            return None;
        }
        Some((record.as_ref(), &record.work))
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn analyze_shared_hinge_solid_classification_v1<
    'admission,
    'margin,
    'prerequisite,
    'ef,
    'exact_e_corridor,
    'direct_f_corridor,
    'exact,
    'pose,
>(
    prerequisite_analysis: &'prerequisite SingleTriangularHingePrerequisiteAnalysis<'exact, 'pose>,
    ef_boundary: Option<&'ef AxisAlignedEfBoundaryCapabilityV1<'prerequisite, 'exact, 'pose>>,
    exact_e_analysis: &'exact_e_corridor ExactEFiniteHingeCorridorAnalysis<
        'prerequisite,
        'ef,
        'exact,
        'pose,
    >,
    direct_f_analysis: &'direct_f_corridor DirectFFiniteHingeCorridorAnalysis<
        'prerequisite,
        'ef,
        'exact_e_corridor,
        'exact,
        'pose,
    >,
    admission_analysis: &'admission SharedHingeCorridorAdmissionAnalysisV1<
        'prerequisite,
        'ef,
        'exact_e_corridor,
        'direct_f_corridor,
        'exact,
        'pose,
    >,
    margin_analysis: &'margin SharedHingeNativeExactTopologyMarginAnalysisV1<
        'prerequisite,
        'ef,
        'exact,
        'pose,
    >,
    exact: &'exact RationalCayleyTreePose<'pose>,
    bound: BoundMaterialTreePose<'pose>,
    paper_thickness_mm: f64,
    limits: SharedHingeSolidClassificationLimitsV1,
) -> Result<
    SharedHingeSolidClassificationAnalysisV1<
        'admission,
        'margin,
        'prerequisite,
        'ef,
        'exact_e_corridor,
        'direct_f_corridor,
        'exact,
        'pose,
    >,
    SharedHingeSolidClassificationErrorV1,
> {
    let limits = limits.projected();
    let mut work = SharedHingeSolidClassificationWorkV1::default();
    let result = calculate_shared_hinge_solid_classification_v1(
        prerequisite_analysis,
        ef_boundary,
        exact_e_analysis,
        direct_f_analysis,
        admission_analysis,
        margin_analysis,
        exact,
        bound,
        paper_thickness_mm,
        &limits,
        &mut work,
    );
    match result {
        Ok(mut result) => {
            match &mut result {
                SharedHingeSolidClassificationResultV1::Classified(record) => {
                    record.work = work.clone();
                    record.sealed_work = Some(work.clone());
                    record.sealed_snapshot = Some(record.snapshot);
                    record.sealed_prior_work = Some(record.prior_work.clone());
                }
                SharedHingeSolidClassificationResultV1::IndependentlyClassified(record) => {
                    record.work = work.clone();
                    record.sealed_work = Some(work.clone());
                    record.sealed_snapshot = Some(record.snapshot);
                    record.sealed_geometry = Some(IndependentGeometrySealV1 {
                        exact_report: record.exact_report,
                        direct_report: record.direct_report,
                        exact_corridor_scan: record.exact_corridor_scan,
                        direct_corridor_scan: record.direct_corridor_scan,
                    });
                    record.sealed_margin_work = Some(record.margin_work.clone());
                }
                SharedHingeSolidClassificationResultV1::EvidenceUnavailable(_) => {}
            }
            Ok(SharedHingeSolidClassificationAnalysisV1 { result, work })
        }
        Err(CayleyError::ResourceLimitExceeded { .. }) => {
            Err(SharedHingeSolidClassificationErrorV1::ResourceLimitExceeded)
        }
        Err(_) => Ok(SharedHingeSolidClassificationAnalysisV1 {
            result: SharedHingeSolidClassificationResultV1::EvidenceUnavailable(
                SharedHingeEvidenceUnavailableReasonV1::AuthorityOrSealMismatch,
            ),
            work,
        }),
    }
}

#[allow(clippy::too_many_arguments)]
fn calculate_shared_hinge_solid_classification_v1<
    'admission,
    'margin,
    'prerequisite,
    'ef,
    'exact_e_corridor,
    'direct_f_corridor,
    'exact,
    'pose,
>(
    prerequisite_analysis: &'prerequisite SingleTriangularHingePrerequisiteAnalysis<'exact, 'pose>,
    ef_boundary: Option<&'ef AxisAlignedEfBoundaryCapabilityV1<'prerequisite, 'exact, 'pose>>,
    exact_e_analysis: &'exact_e_corridor ExactEFiniteHingeCorridorAnalysis<
        'prerequisite,
        'ef,
        'exact,
        'pose,
    >,
    direct_f_analysis: &'direct_f_corridor DirectFFiniteHingeCorridorAnalysis<
        'prerequisite,
        'ef,
        'exact_e_corridor,
        'exact,
        'pose,
    >,
    admission_analysis: &'admission SharedHingeCorridorAdmissionAnalysisV1<
        'prerequisite,
        'ef,
        'exact_e_corridor,
        'direct_f_corridor,
        'exact,
        'pose,
    >,
    margin_analysis: &'margin SharedHingeNativeExactTopologyMarginAnalysisV1<
        'prerequisite,
        'ef,
        'exact,
        'pose,
    >,
    exact: &'exact RationalCayleyTreePose<'pose>,
    bound: BoundMaterialTreePose<'pose>,
    paper_thickness_mm: f64,
    limits: &SharedHingeSolidClassificationLimitsV1,
    work: &mut SharedHingeSolidClassificationWorkV1,
) -> Result<
    SharedHingeSolidClassificationResultV1<
        'admission,
        'margin,
        'prerequisite,
        'ef,
        'exact_e_corridor,
        'direct_f_corridor,
        'exact,
        'pose,
    >,
    CayleyError,
> {
    if is_layer_offset(
        prerequisite_analysis,
        exact_e_analysis,
        direct_f_analysis,
        admission_analysis,
        margin_analysis,
    ) {
        return Ok(SharedHingeSolidClassificationResultV1::EvidenceUnavailable(
            SharedHingeEvidenceUnavailableReasonV1::LayerOffsetUnmodeled,
        ));
    }
    let SingleTriangularHingePrerequisiteResult::Authenticated(prerequisite) =
        &prerequisite_analysis.result
    else {
        return Ok(SharedHingeSolidClassificationResultV1::EvidenceUnavailable(
            SharedHingeEvidenceUnavailableReasonV1::PrerequisiteUnavailable,
        ));
    };
    let Some(ef_boundary) = ef_boundary else {
        return Ok(SharedHingeSolidClassificationResultV1::EvidenceUnavailable(
            SharedHingeEvidenceUnavailableReasonV1::EfBoundaryUnavailable,
        ));
    };

    if !positive_finite_binary64(paper_thickness_mm)
        || exact.version != RATIONAL_CAYLEY_TREE_POSE_V1
        || !exact.is_for(bound)
        || bound.model() != exact.bound.model()
        || !bound.pose().same_instance(exact.bound.pose())
        || revalidate_single_triangular_hinge_prerequisites_v1(
            prerequisite,
            exact,
            paper_thickness_mm,
        )
        .is_none()
        || revalidate_axis_aligned_ef_boundary_v1(
            ef_boundary,
            prerequisite,
            exact,
            bound,
            paper_thickness_mm,
        )
        .is_none()
    {
        return Ok(SharedHingeSolidClassificationResultV1::EvidenceUnavailable(
            SharedHingeEvidenceUnavailableReasonV1::AuthorityOrSealMismatch,
        ));
    }
    let exact_e_contained = exact_e_analysis.authenticated_contained_capability_and_work();
    let direct_f_contained = direct_f_analysis.authenticated_contained_capability_and_work();
    if exact_e_contained.is_none() || direct_f_contained.is_none() {
        let unavailable_reason = if exact_e_contained.is_none() {
            SharedHingeEvidenceUnavailableReasonV1::ExactIntersectionUnavailable
        } else {
            SharedHingeEvidenceUnavailableReasonV1::DirectIntersectionUnavailable
        };
        return calculate_independent_shared_hinge_solid_classification_v1(
            prerequisite,
            ef_boundary,
            margin_analysis,
            exact,
            bound,
            paper_thickness_mm,
            unavailable_reason,
            limits,
            work,
        );
    }
    let (exact_e_corridor, exact_e_work) =
        exact_e_contained.ok_or(CayleyError::InvariantFailure { stage: STAGE })?;
    let (direct_f_corridor, direct_f_work) =
        direct_f_contained.ok_or(CayleyError::InvariantFailure { stage: STAGE })?;
    if revalidate_exact_e_finite_hinge_corridor_v1(
        exact_e_corridor,
        prerequisite,
        ef_boundary,
        exact,
        bound,
        paper_thickness_mm,
    )
    .is_none()
        || revalidate_direct_f_finite_hinge_corridor_v1(
            direct_f_corridor,
            prerequisite,
            ef_boundary,
            exact_e_corridor,
            exact,
            bound,
            paper_thickness_mm,
        )
        .is_none()
    {
        return Ok(SharedHingeSolidClassificationResultV1::EvidenceUnavailable(
            SharedHingeEvidenceUnavailableReasonV1::AuthorityOrSealMismatch,
        ));
    }
    if exact_e_corridor.interaction_kind() != direct_f_corridor.interaction_kind() {
        return Ok(SharedHingeSolidClassificationResultV1::EvidenceUnavailable(
            SharedHingeEvidenceUnavailableReasonV1::InteractionKindMismatch,
        ));
    }
    if !complete_prism_scan(&exact_e_work.prism) || !complete_prism_scan(&direct_f_work.prism) {
        return Ok(SharedHingeSolidClassificationResultV1::EvidenceUnavailable(
            SharedHingeEvidenceUnavailableReasonV1::IncompletePairCoverage,
        ));
    }

    let face_count = exact.faces.len();
    let expected_pairs = face_count
        .checked_mul(face_count.saturating_sub(1))
        .and_then(|twice| twice.checked_div(2))
        .ok_or(CayleyError::ResourceLimitExceeded {
            stage: STAGE,
            resource: "shared_hinge_classification_pair_count",
        })?;
    if face_count != FACE_COUNT
        || exact.hinges.len() != HINGE_COUNT
        || expected_pairs != UNORDERED_FACE_PAIR_COUNT
        || prerequisite.left_face_index == prerequisite.right_face_index
        || prerequisite.left_face_index >= face_count
        || prerequisite.right_face_index >= face_count
        || prerequisite.hinge_index >= HINGE_COUNT
    {
        return Ok(SharedHingeSolidClassificationResultV1::EvidenceUnavailable(
            SharedHingeEvidenceUnavailableReasonV1::IncompletePairCoverage,
        ));
    }

    let (
        reconciliation_authority,
        reconciliation_kind,
        reconciliation_work,
        margin_component_checks,
    ) = if let Some((admission, admission_work)) =
        admission_analysis.authenticated_admission_capability_and_work()
    {
        if revalidate_shared_hinge_corridor_admission_v1(
            admission,
            prerequisite,
            ef_boundary,
            exact_e_corridor,
            direct_f_corridor,
            exact,
            bound,
            paper_thickness_mm,
        )
        .is_none()
        {
            return Ok(SharedHingeSolidClassificationResultV1::EvidenceUnavailable(
                SharedHingeEvidenceUnavailableReasonV1::AuthorityOrSealMismatch,
            ));
        }
        (
            ReconciliationAuthorityV1::BitExact(admission),
            SharedHingeCorridorReconciliationV1::BitExactIdenticalCorridor,
            ReconciliationWorkSealV1::BitExact(admission_work.clone()),
            0,
        )
    } else {
        if !matches!(
            &admission_analysis.result,
            SharedHingeCorridorAdmissionResultV1::BoundaryMismatch(_)
        ) {
            return Ok(SharedHingeSolidClassificationResultV1::EvidenceUnavailable(
                SharedHingeEvidenceUnavailableReasonV1::CorridorReconciliationUnavailable,
            ));
        }
        let Some((margin, margin_work)) = margin_analysis.authenticated_capability_and_work()
        else {
            return Ok(SharedHingeSolidClassificationResultV1::EvidenceUnavailable(
                SharedHingeEvidenceUnavailableReasonV1::CorridorReconciliationUnavailable,
            ));
        };
        if revalidate_shared_hinge_native_exact_topology_margin_v1(
            margin,
            prerequisite,
            ef_boundary,
            exact,
            bound,
            paper_thickness_mm,
        )
        .is_none()
            || !margin_reconciles_corridors(margin)
        {
            return Ok(SharedHingeSolidClassificationResultV1::EvidenceUnavailable(
                SharedHingeEvidenceUnavailableReasonV1::CorridorReconciliationUnavailable,
            ));
        }
        (
            ReconciliationAuthorityV1::Margin(margin),
            SharedHingeCorridorReconciliationV1::NativeExactTopologyMargin,
            ReconciliationWorkSealV1::Margin(margin_work.clone()),
            MARGIN_COMPONENT_COUNT,
        )
    };

    charge_fixed_work(work, limits, margin_component_checks)?;

    let snapshot =
        classification_snapshot(exact_e_corridor.interaction_kind(), reconciliation_kind)
            .ok_or(CayleyError::InvariantFailure { stage: STAGE })?;
    let fixed_face = exact
        .fixed_face
        .ok_or(CayleyError::InvariantFailure { stage: STAGE })?;
    let exact_hinge = exact
        .hinges
        .first()
        .ok_or(CayleyError::InvariantFailure { stage: STAGE })?;
    let native_hinge = bound
        .model()
        .hinges()
        .first()
        .ok_or(CayleyError::InvariantFailure { stage: STAGE })?;
    let native_angles = bound.pose().hinge_angles();
    let native_angle = native_angles
        .first()
        .copied()
        .ok_or(CayleyError::InvariantFailure { stage: STAGE })?;
    if native_angles.len() != HINGE_COUNT
        || bound.pose().fixed_face() != Some(fixed_face)
        || exact_hinge.edge != native_hinge.edge()
        || native_angle.edge() != exact_hinge.edge
        || native_angle.angle_degrees().to_bits() != exact_hinge.angle_magnitude_bits
    {
        return Ok(SharedHingeSolidClassificationResultV1::EvidenceUnavailable(
            SharedHingeEvidenceUnavailableReasonV1::AuthorityOrSealMismatch,
        ));
    }

    let binary64_face_transforms = direct_f_corridor.binary64_face_transforms;
    let hinge_parent_transform = direct_f_corridor.hinge_parent_transform_bits();
    let prior_work = PriorWorkSealV1 {
        exact_e: exact_e_work.clone(),
        direct_f: direct_f_work.clone(),
        reconciliation: reconciliation_work,
    };
    Ok(SharedHingeSolidClassificationResultV1::Classified(
        Box::new(SharedHingeSolidClassificationRecordV1 {
            prerequisite,
            ef_boundary,
            exact_e_corridor,
            direct_f_corridor,
            reconciliation_authority,
            exact,
            bound,
            paper_thickness_bits: paper_thickness_mm.to_bits(),
            fixed_face,
            face_ids: [exact.faces[0].face, exact.faces[1].face],
            hinge_edge: exact_hinge.edge,
            hinge_parent: exact_hinge.parent,
            hinge_child: exact_hinge.child,
            hinge_endpoint_vertices: exact_hinge.endpoint_vertices,
            hinge_angle: BoundHingeAngleBitsV1 {
                edge: native_angle.edge(),
                angle_degrees_bits: native_angle.angle_degrees().to_bits(),
            },
            binary64_face_transforms,
            hinge_parent_transform,
            snapshot,
            prior_work,
            work: SharedHingeSolidClassificationWorkV1::default(),
            sealed_snapshot: None,
            sealed_prior_work: None,
            sealed_work: None,
        }),
    ))
}

#[allow(clippy::too_many_arguments)]
fn calculate_independent_shared_hinge_solid_classification_v1<
    'admission,
    'margin,
    'prerequisite,
    'ef,
    'exact_e_corridor,
    'direct_f_corridor,
    'exact,
    'pose,
>(
    prerequisite: &'prerequisite AuthenticatedSingleTriangularHingePrerequisitesV1<'exact, 'pose>,
    ef_boundary: &'ef AxisAlignedEfBoundaryCapabilityV1<'prerequisite, 'exact, 'pose>,
    margin_analysis: &'margin SharedHingeNativeExactTopologyMarginAnalysisV1<
        'prerequisite,
        'ef,
        'exact,
        'pose,
    >,
    exact: &'exact RationalCayleyTreePose<'pose>,
    bound: BoundMaterialTreePose<'pose>,
    paper_thickness_mm: f64,
    unavailable_reason: SharedHingeEvidenceUnavailableReasonV1,
    limits: &SharedHingeSolidClassificationLimitsV1,
    work: &mut SharedHingeSolidClassificationWorkV1,
) -> Result<
    SharedHingeSolidClassificationResultV1<
        'admission,
        'margin,
        'prerequisite,
        'ef,
        'exact_e_corridor,
        'direct_f_corridor,
        'exact,
        'pose,
    >,
    CayleyError,
> {
    let Some((margin, margin_work)) = margin_analysis.authenticated_capability_and_work() else {
        return Ok(SharedHingeSolidClassificationResultV1::EvidenceUnavailable(
            unavailable_reason,
        ));
    };
    if revalidate_shared_hinge_native_exact_topology_margin_v1(
        margin,
        prerequisite,
        ef_boundary,
        exact,
        bound,
        paper_thickness_mm,
    )
    .is_none()
    {
        return Ok(SharedHingeSolidClassificationResultV1::EvidenceUnavailable(
            SharedHingeEvidenceUnavailableReasonV1::AuthorityOrSealMismatch,
        ));
    }
    if exact.faces.len() != FACE_COUNT
        || exact.hinges.len() != HINGE_COUNT
        || prerequisite.left_face_index == prerequisite.right_face_index
        || prerequisite.left_face_index >= FACE_COUNT
        || prerequisite.right_face_index >= FACE_COUNT
        || prerequisite.hinge_index >= HINGE_COUNT
    {
        return Ok(SharedHingeSolidClassificationResultV1::EvidenceUnavailable(
            SharedHingeEvidenceUnavailableReasonV1::IncompletePairCoverage,
        ));
    }

    charge_independent_fixed_work(work, limits)?;

    let exact_solids = margin.exact_solid_vertices();
    let direct_solids = margin.direct_solid_vertices();
    let exact_first = prebuilt_prism_view(&exact_solids[0]);
    let exact_second = prebuilt_prism_view(&exact_solids[1]);
    let direct_first = prebuilt_prism_view(&direct_solids[0]);
    let direct_second = prebuilt_prism_view(&direct_solids[1]);

    let mut exact_prism_meter = WorkMeter::new(&limits.prism.exact);
    let Some(exact_report) = analyze_exact_prebuilt_prism_pair_with_meter_v1(
        exact_first,
        exact_second,
        limits.prism,
        &mut work.independent_exact_prism,
        &mut exact_prism_meter,
    )?
    else {
        return Ok(SharedHingeSolidClassificationResultV1::EvidenceUnavailable(
            unavailable_reason,
        ));
    };
    if work.independent_exact_prism.exact != exact_prism_meter.work {
        return Err(CayleyError::InvariantFailure { stage: STAGE });
    }

    let mut direct_prism_meter = WorkMeter::new(&limits.prism.exact);
    let Some(direct_report) = analyze_exact_prebuilt_prism_pair_with_meter_v1(
        direct_first,
        direct_second,
        limits.prism,
        &mut work.independent_direct_prism,
        &mut direct_prism_meter,
    )?
    else {
        return Ok(SharedHingeSolidClassificationResultV1::EvidenceUnavailable(
            unavailable_reason,
        ));
    };
    if work.independent_direct_prism.exact != direct_prism_meter.work
        || !complete_prism_scan(&work.independent_exact_prism)
        || !complete_prism_scan(&work.independent_direct_prism)
    {
        return Err(CayleyError::InvariantFailure { stage: STAGE });
    }

    let mut exact_corridor_meter = WorkMeter::new(&limits.corridor_exact);
    let exact_corridor_scan = scan_independent_intersection_against_corridor(
        &exact_report,
        margin.exact_corridor_components(),
        &mut work.independent_corridor_vertex_checks,
        limits.max_independent_corridor_vertex_checks,
        &mut exact_corridor_meter,
    )?;
    work.independent_exact_corridor = exact_corridor_meter.work;

    let mut direct_corridor_meter = WorkMeter::new(&limits.corridor_exact);
    let direct_corridor_scan = scan_independent_intersection_against_corridor(
        &direct_report,
        margin.direct_corridor_components(),
        &mut work.independent_corridor_vertex_checks,
        limits.max_independent_corridor_vertex_checks,
        &mut direct_corridor_meter,
    )?;
    work.independent_direct_corridor = direct_corridor_meter.work;

    let exact_report_summary = IndependentPrismSummaryV1::from_report(&exact_report);
    let direct_report_summary = IndependentPrismSummaryV1::from_report(&direct_report);
    let Some(snapshot) = independent_classification_snapshot(
        exact_report_summary,
        direct_report_summary,
        exact_corridor_scan,
        direct_corridor_scan,
    ) else {
        return Ok(SharedHingeSolidClassificationResultV1::EvidenceUnavailable(
            unavailable_reason,
        ));
    };

    let fixed_face = exact
        .fixed_face
        .ok_or(CayleyError::InvariantFailure { stage: STAGE })?;
    let exact_hinge = exact
        .hinges
        .first()
        .ok_or(CayleyError::InvariantFailure { stage: STAGE })?;
    let native_hinge = bound
        .model()
        .hinges()
        .first()
        .ok_or(CayleyError::InvariantFailure { stage: STAGE })?;
    let native_angles = bound.pose().hinge_angles();
    let native_angle = native_angles
        .first()
        .copied()
        .ok_or(CayleyError::InvariantFailure { stage: STAGE })?;
    if native_angles.len() != HINGE_COUNT
        || bound.pose().fixed_face() != Some(fixed_face)
        || exact_hinge.edge != native_hinge.edge()
        || native_angle.edge() != exact_hinge.edge
        || native_angle.angle_degrees().to_bits() != exact_hinge.angle_magnitude_bits
    {
        return Ok(SharedHingeSolidClassificationResultV1::EvidenceUnavailable(
            SharedHingeEvidenceUnavailableReasonV1::AuthorityOrSealMismatch,
        ));
    }

    Ok(
        SharedHingeSolidClassificationResultV1::IndependentlyClassified(Box::new(
            IndependentSharedHingeSolidClassificationRecordV1 {
                prerequisite,
                ef_boundary,
                margin,
                exact,
                bound,
                paper_thickness_bits: paper_thickness_mm.to_bits(),
                fixed_face,
                face_ids: [exact.faces[0].face, exact.faces[1].face],
                hinge_edge: exact_hinge.edge,
                hinge_parent: exact_hinge.parent,
                hinge_child: exact_hinge.child,
                hinge_endpoint_vertices: exact_hinge.endpoint_vertices,
                hinge_angle: BoundHingeAngleBitsV1 {
                    edge: native_angle.edge(),
                    angle_degrees_bits: native_angle.angle_degrees().to_bits(),
                },
                binary64_face_transforms: margin.binary64_face_transforms(),
                hinge_parent_transform: margin.hinge_parent_transform_bits(),
                exact_report: exact_report_summary,
                direct_report: direct_report_summary,
                exact_corridor_scan,
                direct_corridor_scan,
                snapshot,
                margin_work: margin_work.clone(),
                work: SharedHingeSolidClassificationWorkV1::default(),
                sealed_snapshot: None,
                sealed_geometry: None,
                sealed_margin_work: None,
                sealed_work: None,
            },
        )),
    )
}

fn prebuilt_prism_view(vertices: &[ExactPoint3; 6]) -> ExactPrebuiltTriangularPrismView<'_> {
    ExactPrebuiltTriangularPrismView {
        vertices: [
            &vertices[0],
            &vertices[1],
            &vertices[2],
            &vertices[3],
            &vertices[4],
            &vertices[5],
        ],
    }
}

fn scan_independent_intersection_against_corridor(
    report: &ExactPrismIntersectionReport,
    components: &[BigRational; MARGIN_COMPONENT_COUNT],
    cumulative_vertex_checks: &mut usize,
    max_vertex_checks: usize,
    meter: &mut WorkMeter<'_>,
) -> Result<IndependentCorridorScanV1, CayleyError> {
    let axis_start = ExactPoint3 {
        coordinates: [
            meter.clone_rational(&components[0], STAGE)?,
            meter.clone_rational(&components[1], STAGE)?,
            meter.clone_rational(&components[2], STAGE)?,
        ],
    };
    let axis = ExactVector3 {
        coordinates: [
            meter.clone_rational(&components[3], STAGE)?,
            meter.clone_rational(&components[4], STAGE)?,
            meter.clone_rational(&components[5], STAGE)?,
        ],
    };
    let length_squared = &components[6];
    let half_thickness = &components[7];
    let cosine_half_squared = &components[8];
    let radial_limit_product = &components[9];
    let zero = BigRational::zero();
    if meter.compare_rational(length_squared, &zero, STAGE)? != Ordering::Greater
        || meter.compare_rational(half_thickness, &zero, STAGE)? != Ordering::Greater
        || meter.compare_rational(cosine_half_squared, &zero, STAGE)? != Ordering::Greater
        || meter.compare_rational(radial_limit_product, &zero, STAGE)? == Ordering::Less
    {
        return Err(CayleyError::InvariantFailure { stage: STAGE });
    }

    let mut first_outside_vertex_index = None;
    let mut outside_vertex_count = 0_usize;
    let mut every_vertex_on_finite_axis = true;
    for (vertex_index, vertex) in report.canonical_vertices().iter().enumerate() {
        *cumulative_vertex_checks =
            cumulative_vertex_checks
                .checked_add(1)
                .ok_or(CayleyError::ResourceLimitExceeded {
                    stage: STAGE,
                    resource: "shared_hinge_independent_corridor_vertices",
                })?;
        if *cumulative_vertex_checks > max_vertex_checks {
            return Err(CayleyError::ResourceLimitExceeded {
                stage: STAGE,
                resource: "shared_hinge_independent_corridor_vertices",
            });
        }
        let relative = exact_between(&axis_start, vertex, meter)?;
        let axial = exact_dot(&relative, &axis, meter)?;
        let cross = exact_cross_for_classification(&relative, &axis, meter)?;
        let radial_squared = exact_dot(&cross, &cross, meter)?;
        let radial_left = meter.multiply_rational(cosine_half_squared, &radial_squared, STAGE)?;
        let axial_before = meter.compare_rational(&axial, &zero, STAGE)? == Ordering::Less;
        let axial_after =
            meter.compare_rational(&axial, length_squared, STAGE)? == Ordering::Greater;
        let radial_outside =
            meter.compare_rational(&radial_left, radial_limit_product, STAGE)? == Ordering::Greater;
        if axial_before || axial_after || radial_outside {
            first_outside_vertex_index.get_or_insert(vertex_index);
            outside_vertex_count =
                outside_vertex_count
                    .checked_add(1)
                    .ok_or(CayleyError::ResourceLimitExceeded {
                        stage: STAGE,
                        resource: "shared_hinge_independent_outside_vertices",
                    })?;
        }
        every_vertex_on_finite_axis &= !axial_before && !axial_after && radial_squared.is_zero();
    }
    Ok(IndependentCorridorScanV1 {
        vertex_count: report.canonical_vertices().len(),
        outside_vertex_count,
        first_outside_vertex_index,
        every_vertex_on_finite_axis,
    })
}

fn exact_cross_for_classification(
    first: &ExactVector3,
    second: &ExactVector3,
    meter: &mut WorkMeter<'_>,
) -> Result<ExactVector3, CayleyError> {
    const COMPONENTS: [(usize, usize, usize, usize); 3] =
        [(1, 2, 2, 1), (2, 0, 0, 2), (0, 1, 1, 0)];
    Ok(ExactVector3 {
        coordinates: try_array3(|axis| {
            let (first_left, second_left, first_right, second_right) = COMPONENTS[axis];
            let left = meter.multiply_rational(
                &first.coordinates[first_left],
                &second.coordinates[second_left],
                STAGE,
            )?;
            let right = meter.multiply_rational(
                &first.coordinates[first_right],
                &second.coordinates[second_right],
                STAGE,
            )?;
            meter.subtract_rational(&left, &right, STAGE)
        })?,
    })
}

fn independent_classification_snapshot(
    exact_report: IndependentPrismSummaryV1,
    direct_report: IndependentPrismSummaryV1,
    exact_scan: IndependentCorridorScanV1,
    direct_scan: IndependentCorridorScanV1,
) -> Option<SharedHingeSolidClassificationSnapshotV1> {
    let (class, dimension, raw_prism_evidence, reconciliation) =
        match (exact_report.kind, direct_report.kind) {
            (
                ExactPrismIntersectionKind::PositiveVolume,
                ExactPrismIntersectionKind::PositiveVolume,
            ) if exact_report.affine_rank == Some(3)
                && direct_report.affine_rank == Some(3)
                && exact_report.vertex_count == exact_scan.vertex_count
                && direct_report.vertex_count == direct_scan.vertex_count
                && exact_scan.vertex_count >= 4
                && direct_scan.vertex_count >= 4
                && exact_scan.outside_vertex_count > 0
                && direct_scan.outside_vertex_count > 0 =>
            {
                (
                    SharedHingePositiveThicknessPairClassV1::PositiveVolumeIntersection,
                    SharedHingeSolidIntersectionDimensionV1::PositiveVolume,
                    IntersectionEvidenceV2::PositiveVolumeOverlap,
                    SharedHingeCorridorReconciliationV1::BothOutsideFiniteCorridors,
                )
            }
            (ExactPrismIntersectionKind::Point, ExactPrismIntersectionKind::Point)
                if exact_report.affine_rank == Some(0)
                    && direct_report.affine_rank == Some(0)
                    && exact_report.vertex_count == exact_scan.vertex_count
                    && direct_report.vertex_count == direct_scan.vertex_count
                    && exact_scan.vertex_count == 1
                    && direct_scan.vertex_count == 1
                    && exact_scan.every_vertex_on_finite_axis
                    && direct_scan.every_vertex_on_finite_axis =>
            {
                (
                    SharedHingePositiveThicknessPairClassV1::SharedFeatureOnlyContact,
                    SharedHingeSolidIntersectionDimensionV1::Point,
                    IntersectionEvidenceV2::PointContact,
                    SharedHingeCorridorReconciliationV1::ExactSharedFeatureGeometry,
                )
            }
            (ExactPrismIntersectionKind::Line, ExactPrismIntersectionKind::Line)
                if exact_report.affine_rank == Some(1)
                    && direct_report.affine_rank == Some(1)
                    && exact_report.vertex_count == exact_scan.vertex_count
                    && direct_report.vertex_count == direct_scan.vertex_count
                    && exact_scan.vertex_count >= 2
                    && direct_scan.vertex_count >= 2
                    && exact_scan.every_vertex_on_finite_axis
                    && direct_scan.every_vertex_on_finite_axis =>
            {
                (
                    SharedHingePositiveThicknessPairClassV1::SharedFeatureOnlyContact,
                    SharedHingeSolidIntersectionDimensionV1::Line,
                    IntersectionEvidenceV2::BoundaryLineContact,
                    SharedHingeCorridorReconciliationV1::ExactSharedFeatureGeometry,
                )
            }
            _ => return None,
        };
    let contract = policy_contract(class);
    if !matches!(
        (class, contract.baseline_decision),
        (
            SharedHingePositiveThicknessPairClassV1::PositiveVolumeIntersection,
            TopologyContactDecision::Penetrating
        ) | (
            SharedHingePositiveThicknessPairClassV1::SharedFeatureOnlyContact,
            TopologyContactDecision::RequiresHingeModel
        )
    ) {
        return None;
    }
    Some(SharedHingeSolidClassificationSnapshotV1 {
        class,
        exact_intersection: dimension,
        direct_intersection: dimension,
        raw_prism_evidence,
        semantic_evidence: contract.semantic_evidence,
        baseline_decision: contract.baseline_decision,
        reconciliation,
        expected_unordered_face_pairs: UNORDERED_FACE_PAIR_COUNT,
        classified_unordered_face_pairs: UNORDERED_FACE_PAIR_COUNT,
        expected_triangle_pairs: TRIANGLE_PAIR_COUNT,
        classified_triangle_pairs: TRIANGLE_PAIR_COUNT,
    })
}

fn is_layer_offset(
    prerequisite: &SingleTriangularHingePrerequisiteAnalysis<'_, '_>,
    exact_e: &ExactEFiniteHingeCorridorAnalysis<'_, '_, '_, '_>,
    direct_f: &DirectFFiniteHingeCorridorAnalysis<'_, '_, '_, '_, '_>,
    admission: &SharedHingeCorridorAdmissionAnalysisV1<'_, '_, '_, '_, '_, '_>,
    margin: &SharedHingeNativeExactTopologyMarginAnalysisV1<'_, '_, '_, '_>,
) -> bool {
    matches!(
        &prerequisite.result,
        SingleTriangularHingePrerequisiteResult::LayerOffsetUnmodeled
    ) || matches!(
        &exact_e.result,
        ExactEFiniteHingeCorridorResult::LayerOffsetUnmodeled
    ) || matches!(
        &direct_f.result,
        DirectFFiniteHingeCorridorResult::LayerOffsetUnmodeled
    ) || matches!(
        &admission.result,
        SharedHingeCorridorAdmissionResultV1::LayerOffsetUnmodeled
    ) || matches!(
        &margin.result,
        SharedHingeNativeExactTopologyMarginResultV1::LayerOffsetUnmodeled
    )
}

fn complete_prism_scan(work: &ExactPrismWork) -> bool {
    work.prisms == 2
        && work.solid_vertices == 12
        && work.facets == 10
        && work.halfspaces == 10
        && work.prism_volume_tests == 2
        && work.facet_vertex_checks == 60
        && work.plane_triples == 120
}

fn margin_reconciles_corridors(
    margin: &SharedHingeNativeExactTopologyMarginCapabilityV1<'_, '_, '_, '_>,
) -> bool {
    let errors = margin.corridor_component_error();
    let margins = margin.corridor_component_margin();
    errors
        .iter()
        .zip(margins)
        .all(|(error, limit)| error <= limit)
        && errors.iter().any(|error| !error.is_zero())
}

fn classification_snapshot(
    interaction: ExactEFiniteHingeInteractionKind,
    reconciliation: SharedHingeCorridorReconciliationV1,
) -> Option<SharedHingeSolidClassificationSnapshotV1> {
    let (class, dimension, raw_prism_evidence) = match interaction {
        ExactEFiniteHingeInteractionKind::BoundaryAreaContact => (
            SharedHingePositiveThicknessPairClassV1::AllowedBoundaryContact,
            SharedHingeSolidIntersectionDimensionV1::BoundaryArea,
            IntersectionEvidenceV2::BoundaryAreaContact,
        ),
        ExactEFiniteHingeInteractionKind::PositiveVolume => (
            SharedHingePositiveThicknessPairClassV1::AllowedFiniteCorridorOverlap,
            SharedHingeSolidIntersectionDimensionV1::PositiveVolume,
            IntersectionEvidenceV2::PositiveVolumeOverlap,
        ),
    };
    let contract = policy_contract(class);
    if !matches!(
        (class, contract.baseline_decision),
        (
            SharedHingePositiveThicknessPairClassV1::AllowedBoundaryContact,
            TopologyContactDecision::RequiresHingeModel
        ) | (
            SharedHingePositiveThicknessPairClassV1::AllowedFiniteCorridorOverlap,
            TopologyContactDecision::RequiresHingeModel
        )
    ) {
        return None;
    }
    Some(SharedHingeSolidClassificationSnapshotV1 {
        class,
        exact_intersection: dimension,
        direct_intersection: dimension,
        raw_prism_evidence,
        semantic_evidence: contract.semantic_evidence,
        baseline_decision: contract.baseline_decision,
        reconciliation,
        expected_unordered_face_pairs: UNORDERED_FACE_PAIR_COUNT,
        classified_unordered_face_pairs: UNORDERED_FACE_PAIR_COUNT,
        expected_triangle_pairs: TRIANGLE_PAIR_COUNT,
        classified_triangle_pairs: TRIANGLE_PAIR_COUNT,
    })
}

fn charge_fixed_work(
    work: &mut SharedHingeSolidClassificationWorkV1,
    limits: &SharedHingeSolidClassificationLimitsV1,
    margin_component_checks: usize,
) -> Result<(), CayleyError> {
    charge_common_fixed_work(
        work,
        limits,
        UPSTREAM_CAPABILITY_REVALIDATIONS,
        SEALED_PRIOR_WORK_BINDINGS,
        margin_component_checks,
    )
}

fn charge_independent_fixed_work(
    work: &mut SharedHingeSolidClassificationWorkV1,
    limits: &SharedHingeSolidClassificationLimitsV1,
) -> Result<(), CayleyError> {
    charge_common_fixed_work(
        work,
        limits,
        INDEPENDENT_UPSTREAM_CAPABILITY_REVALIDATIONS,
        INDEPENDENT_SEALED_PRIOR_WORK_BINDINGS,
        MARGIN_COMPONENT_COUNT,
    )
}

fn charge_common_fixed_work(
    work: &mut SharedHingeSolidClassificationWorkV1,
    limits: &SharedHingeSolidClassificationLimitsV1,
    upstream_capability_revalidations: usize,
    sealed_prior_work_bindings: usize,
    margin_component_checks: usize,
) -> Result<(), CayleyError> {
    for (counter, required, maximum, resource) in [
        (
            &mut work.authenticated_faces,
            FACE_COUNT,
            limits.max_authenticated_faces,
            "shared_hinge_classification_faces",
        ),
        (
            &mut work.authenticated_hinges,
            HINGE_COUNT,
            limits.max_authenticated_hinges,
            "shared_hinge_classification_hinges",
        ),
        (
            &mut work.unordered_face_pair_count_calculations,
            1,
            limits.max_unordered_face_pair_count_calculations,
            "shared_hinge_classification_pair_count_calculations",
        ),
        (
            &mut work.unordered_face_pairs,
            UNORDERED_FACE_PAIR_COUNT,
            limits.max_unordered_face_pairs,
            "shared_hinge_classification_face_pairs",
        ),
        (
            &mut work.triangle_pairs,
            TRIANGLE_PAIR_COUNT,
            limits.max_triangle_pairs,
            "shared_hinge_classification_triangle_pairs",
        ),
        (
            &mut work.prism_complete_scan_checks,
            PRISM_SCAN_COUNT,
            limits.max_prism_complete_scan_checks,
            "shared_hinge_classification_prism_scans",
        ),
        (
            &mut work.upstream_capability_revalidations,
            upstream_capability_revalidations,
            limits.max_upstream_capability_revalidations,
            "shared_hinge_classification_upstream_revalidations",
        ),
        (
            &mut work.sealed_prior_work_bindings,
            sealed_prior_work_bindings,
            limits.max_sealed_prior_work_bindings,
            "shared_hinge_classification_prior_work",
        ),
        (
            &mut work.root_bindings,
            ROOT_BINDINGS,
            limits.max_root_bindings,
            "shared_hinge_classification_root",
        ),
        (
            &mut work.angle_bindings,
            ANGLE_BINDINGS,
            limits.max_angle_bindings,
            "shared_hinge_classification_angle",
        ),
        (
            &mut work.face_identity_bindings,
            FACE_IDENTITY_BINDINGS,
            limits.max_face_identity_bindings,
            "shared_hinge_classification_face_identities",
        ),
        (
            &mut work.hinge_identity_bindings,
            HINGE_IDENTITY_BINDINGS,
            limits.max_hinge_identity_bindings,
            "shared_hinge_classification_hinge_identities",
        ),
        (
            &mut work.endpoint_identity_bindings,
            ENDPOINT_IDENTITY_BINDINGS,
            limits.max_endpoint_identity_bindings,
            "shared_hinge_classification_endpoint_identities",
        ),
        (
            &mut work.face_transform_bit_bindings,
            FACE_TRANSFORM_BIT_BINDINGS,
            limits.max_face_transform_bit_bindings,
            "shared_hinge_classification_face_transform_bits",
        ),
        (
            &mut work.hinge_parent_transform_bit_bindings,
            HINGE_PARENT_TRANSFORM_BIT_BINDINGS,
            limits.max_hinge_parent_transform_bit_bindings,
            "shared_hinge_classification_parent_transform_bits",
        ),
        (
            &mut work.interaction_kind_bindings,
            INTERACTION_KIND_BINDINGS,
            limits.max_interaction_kind_bindings,
            "shared_hinge_classification_interaction_kinds",
        ),
        (
            &mut work.policy_cell_bindings,
            POLICY_CELL_BINDINGS,
            limits.max_policy_cell_bindings,
            "shared_hinge_classification_policy_cell",
        ),
        (
            &mut work.classification_seal_bindings,
            CLASSIFICATION_SEAL_BINDINGS,
            limits.max_classification_seal_bindings,
            "shared_hinge_classification_seal",
        ),
        (
            &mut work.margin_component_checks,
            margin_component_checks,
            limits.max_margin_component_checks,
            "shared_hinge_classification_margin_components",
        ),
    ] {
        set_fixed_counter(counter, required, maximum, resource)?;
    }
    Ok(())
}

fn set_fixed_counter(
    counter: &mut usize,
    required: usize,
    maximum: usize,
    resource: &'static str,
) -> Result<(), CayleyError> {
    if *counter != 0 {
        return Err(CayleyError::InvariantFailure { stage: STAGE });
    }
    if required > maximum {
        return Err(CayleyError::ResourceLimitExceeded {
            stage: STAGE,
            resource,
        });
    }
    *counter = required;
    Ok(())
}

fn expected_work(
    reconciliation: SharedHingeCorridorReconciliationV1,
) -> SharedHingeSolidClassificationWorkV1 {
    SharedHingeSolidClassificationWorkV1 {
        authenticated_faces: FACE_COUNT,
        authenticated_hinges: HINGE_COUNT,
        unordered_face_pair_count_calculations: 1,
        unordered_face_pairs: UNORDERED_FACE_PAIR_COUNT,
        triangle_pairs: TRIANGLE_PAIR_COUNT,
        prism_complete_scan_checks: PRISM_SCAN_COUNT,
        upstream_capability_revalidations: UPSTREAM_CAPABILITY_REVALIDATIONS,
        sealed_prior_work_bindings: SEALED_PRIOR_WORK_BINDINGS,
        root_bindings: ROOT_BINDINGS,
        angle_bindings: ANGLE_BINDINGS,
        face_identity_bindings: FACE_IDENTITY_BINDINGS,
        hinge_identity_bindings: HINGE_IDENTITY_BINDINGS,
        endpoint_identity_bindings: ENDPOINT_IDENTITY_BINDINGS,
        face_transform_bit_bindings: FACE_TRANSFORM_BIT_BINDINGS,
        hinge_parent_transform_bit_bindings: HINGE_PARENT_TRANSFORM_BIT_BINDINGS,
        interaction_kind_bindings: INTERACTION_KIND_BINDINGS,
        policy_cell_bindings: POLICY_CELL_BINDINGS,
        classification_seal_bindings: CLASSIFICATION_SEAL_BINDINGS,
        margin_component_checks: match reconciliation {
            SharedHingeCorridorReconciliationV1::BitExactIdenticalCorridor => 0,
            SharedHingeCorridorReconciliationV1::NativeExactTopologyMargin
            | SharedHingeCorridorReconciliationV1::ExactSharedFeatureGeometry
            | SharedHingeCorridorReconciliationV1::BothOutsideFiniteCorridors => {
                MARGIN_COMPONENT_COUNT
            }
        },
        ..SharedHingeSolidClassificationWorkV1::default()
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn revalidate_shared_hinge_solid_classification_v1<
    'record,
    'admission,
    'margin,
    'prerequisite,
    'ef,
    'exact_e_corridor,
    'direct_f_corridor,
    'exact,
    'pose,
>(
    record: &'record SharedHingeSolidClassificationRecordV1<
        'admission,
        'margin,
        'prerequisite,
        'ef,
        'exact_e_corridor,
        'direct_f_corridor,
        'exact,
        'pose,
    >,
    prerequisite: &AuthenticatedSingleTriangularHingePrerequisitesV1<'_, '_>,
    ef_boundary: &AxisAlignedEfBoundaryCapabilityV1<'_, '_, '_>,
    exact_e_corridor: &ExactEFiniteHingeCorridorCapabilityV1<'_, '_, '_, '_>,
    direct_f_corridor: &DirectFFiniteHingeCorridorCapabilityV1<'_, '_, '_, '_, '_>,
    admission: Option<&SharedHingeCorridorAdmissionCapabilityV1<'_, '_, '_, '_, '_, '_>>,
    margin: Option<&SharedHingeNativeExactTopologyMarginCapabilityV1<'_, '_, '_, '_>>,
    exact: &RationalCayleyTreePose<'_>,
    bound: BoundMaterialTreePose<'_>,
    paper_thickness_mm: f64,
) -> Option<
    RevalidatedSharedHingeSolidClassificationRecordV1<
        'record,
        'admission,
        'margin,
        'prerequisite,
        'ef,
        'exact_e_corridor,
        'direct_f_corridor,
        'exact,
        'pose,
    >,
> {
    let exact_hinge = exact.hinges.first()?;
    let native_hinge = bound.model().hinges().first()?;
    let native_angles = bound.pose().hinge_angles();
    let native_angle = native_angles.first().copied()?;
    if !positive_finite_binary64(paper_thickness_mm)
        || record.sealed_work.as_ref() != Some(&record.work)
        || record.sealed_snapshot.as_ref() != Some(&record.snapshot)
        || record.sealed_prior_work.as_ref() != Some(&record.prior_work)
        || !std::ptr::eq(record.prerequisite, prerequisite)
        || !std::ptr::eq(record.ef_boundary, ef_boundary)
        || !std::ptr::eq(record.exact_e_corridor, exact_e_corridor)
        || !std::ptr::eq(record.direct_f_corridor, direct_f_corridor)
        || !std::ptr::eq(record.exact, exact)
        || record.paper_thickness_bits != paper_thickness_mm.to_bits()
        || record.bound.model() != bound.model()
        || !record.bound.pose().same_instance(bound.pose())
        || !exact.is_for(bound)
        || exact.version != RATIONAL_CAYLEY_TREE_POSE_V1
        || exact.faces.len() != FACE_COUNT
        || exact.hinges.len() != HINGE_COUNT
        || native_angles.len() != HINGE_COUNT
        || exact.fixed_face != Some(record.fixed_face)
        || bound.pose().fixed_face() != Some(record.fixed_face)
        || record.face_ids != [exact.faces[0].face, exact.faces[1].face]
        || record.hinge_edge != exact_hinge.edge
        || record.hinge_edge != native_hinge.edge()
        || record.hinge_parent != exact_hinge.parent
        || record.hinge_child != exact_hinge.child
        || record.hinge_endpoint_vertices != exact_hinge.endpoint_vertices
        || record.hinge_angle.edge != native_angle.edge()
        || record.hinge_angle.angle_degrees_bits != native_angle.angle_degrees().to_bits()
        || record.hinge_angle.angle_degrees_bits != exact_hinge.angle_magnitude_bits
        || record.binary64_face_transforms != direct_f_corridor.binary64_face_transforms
        || record.hinge_parent_transform != direct_f_corridor.hinge_parent_transform_bits()
        || record.snapshot.expected_unordered_face_pairs != UNORDERED_FACE_PAIR_COUNT
        || record.snapshot.classified_unordered_face_pairs != UNORDERED_FACE_PAIR_COUNT
        || record.snapshot.expected_triangle_pairs != TRIANGLE_PAIR_COUNT
        || record.snapshot.classified_triangle_pairs != TRIANGLE_PAIR_COUNT
        || record.snapshot.exact_intersection != record.snapshot.direct_intersection
        || revalidate_single_triangular_hinge_prerequisites_v1(
            prerequisite,
            exact,
            paper_thickness_mm,
        )
        .is_none()
        || revalidate_axis_aligned_ef_boundary_v1(
            ef_boundary,
            prerequisite,
            exact,
            bound,
            paper_thickness_mm,
        )
        .is_none()
        || revalidate_exact_e_finite_hinge_corridor_v1(
            exact_e_corridor,
            prerequisite,
            ef_boundary,
            exact,
            bound,
            paper_thickness_mm,
        )
        .is_none()
        || revalidate_direct_f_finite_hinge_corridor_v1(
            direct_f_corridor,
            prerequisite,
            ef_boundary,
            exact_e_corridor,
            exact,
            bound,
            paper_thickness_mm,
        )
        .is_none()
        || exact_e_corridor.interaction_kind() != direct_f_corridor.interaction_kind()
        || !complete_prism_scan(&record.prior_work.exact_e.prism)
        || !complete_prism_scan(&record.prior_work.direct_f.prism)
        || exact_e_corridor.sealed_work()? != &record.prior_work.exact_e
        || direct_f_corridor.sealed_work()? != &record.prior_work.direct_f
    {
        return None;
    }

    let reconciliation = match (
        &record.reconciliation_authority,
        &record.prior_work.reconciliation,
        admission,
        margin,
    ) {
        (
            ReconciliationAuthorityV1::BitExact(stored),
            ReconciliationWorkSealV1::BitExact(stored_work),
            Some(live),
            None,
        ) if std::ptr::eq(*stored, live)
            && live.sealed_work() == Some(stored_work)
            && revalidate_shared_hinge_corridor_admission_v1(
                live,
                prerequisite,
                ef_boundary,
                exact_e_corridor,
                direct_f_corridor,
                exact,
                bound,
                paper_thickness_mm,
            )
            .is_some() =>
        {
            SharedHingeCorridorReconciliationV1::BitExactIdenticalCorridor
        }
        (
            ReconciliationAuthorityV1::Margin(stored),
            ReconciliationWorkSealV1::Margin(stored_work),
            None,
            Some(live),
        ) if std::ptr::eq(*stored, live)
            && live.sealed_work() == Some(stored_work)
            && revalidate_shared_hinge_native_exact_topology_margin_v1(
                live,
                prerequisite,
                ef_boundary,
                exact,
                bound,
                paper_thickness_mm,
            )
            .is_some()
            && margin_reconciles_corridors(live) =>
        {
            SharedHingeCorridorReconciliationV1::NativeExactTopologyMargin
        }
        _ => return None,
    };
    let live_snapshot =
        classification_snapshot(exact_e_corridor.interaction_kind(), reconciliation)?;
    if record.snapshot != live_snapshot || record.work != expected_work(reconciliation) {
        return None;
    }
    Some(RevalidatedSharedHingeSolidClassificationRecordV1 { record })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn revalidate_independent_shared_hinge_solid_classification_v1<
    'record,
    'margin,
    'prerequisite,
    'ef,
    'exact,
    'pose,
>(
    record: &'record IndependentSharedHingeSolidClassificationRecordV1<
        'margin,
        'prerequisite,
        'ef,
        'exact,
        'pose,
    >,
    prerequisite: &AuthenticatedSingleTriangularHingePrerequisitesV1<'_, '_>,
    ef_boundary: &AxisAlignedEfBoundaryCapabilityV1<'_, '_, '_>,
    margin: &SharedHingeNativeExactTopologyMarginCapabilityV1<'_, '_, '_, '_>,
    exact: &RationalCayleyTreePose<'_>,
    bound: BoundMaterialTreePose<'_>,
    paper_thickness_mm: f64,
) -> Option<
    RevalidatedIndependentSharedHingeSolidClassificationRecordV1<
        'record,
        'margin,
        'prerequisite,
        'ef,
        'exact,
        'pose,
    >,
> {
    let sealed_geometry = record.sealed_geometry.as_ref()?;
    let exact_hinge = exact.hinges.first()?;
    let native_hinge = bound.model().hinges().first()?;
    let native_angles = bound.pose().hinge_angles();
    let native_angle = native_angles.first().copied()?;
    let live_snapshot = independent_classification_snapshot(
        record.exact_report,
        record.direct_report,
        record.exact_corridor_scan,
        record.direct_corridor_scan,
    )?;
    if !positive_finite_binary64(paper_thickness_mm)
        || record.sealed_work.as_ref() != Some(&record.work)
        || record.sealed_snapshot.as_ref() != Some(&record.snapshot)
        || sealed_geometry.exact_report != record.exact_report
        || sealed_geometry.direct_report != record.direct_report
        || sealed_geometry.exact_corridor_scan != record.exact_corridor_scan
        || sealed_geometry.direct_corridor_scan != record.direct_corridor_scan
        || record.sealed_margin_work.as_ref() != Some(&record.margin_work)
        || !std::ptr::eq(record.prerequisite, prerequisite)
        || !std::ptr::eq(record.ef_boundary, ef_boundary)
        || !std::ptr::eq(record.margin, margin)
        || !std::ptr::eq(record.exact, exact)
        || record.paper_thickness_bits != paper_thickness_mm.to_bits()
        || record.bound.model() != bound.model()
        || !record.bound.pose().same_instance(bound.pose())
        || !exact.is_for(bound)
        || exact.version != RATIONAL_CAYLEY_TREE_POSE_V1
        || exact.faces.len() != FACE_COUNT
        || exact.hinges.len() != HINGE_COUNT
        || native_angles.len() != HINGE_COUNT
        || exact.fixed_face != Some(record.fixed_face)
        || bound.pose().fixed_face() != Some(record.fixed_face)
        || record.face_ids != [exact.faces[0].face, exact.faces[1].face]
        || record.hinge_edge != exact_hinge.edge
        || record.hinge_edge != native_hinge.edge()
        || record.hinge_parent != exact_hinge.parent
        || record.hinge_child != exact_hinge.child
        || record.hinge_endpoint_vertices != exact_hinge.endpoint_vertices
        || record.hinge_angle.edge != native_angle.edge()
        || record.hinge_angle.angle_degrees_bits != native_angle.angle_degrees().to_bits()
        || record.hinge_angle.angle_degrees_bits != exact_hinge.angle_magnitude_bits
        || record.binary64_face_transforms != margin.binary64_face_transforms()
        || record.hinge_parent_transform != margin.hinge_parent_transform_bits()
        || record.snapshot != live_snapshot
        || record.snapshot.expected_unordered_face_pairs != UNORDERED_FACE_PAIR_COUNT
        || record.snapshot.classified_unordered_face_pairs != UNORDERED_FACE_PAIR_COUNT
        || record.snapshot.expected_triangle_pairs != TRIANGLE_PAIR_COUNT
        || record.snapshot.classified_triangle_pairs != TRIANGLE_PAIR_COUNT
        || record.snapshot.exact_intersection != record.snapshot.direct_intersection
        || !independent_fixed_work_is_complete(&record.work)
        || !valid_independent_corridor_scan(record.exact_corridor_scan)
        || !valid_independent_corridor_scan(record.direct_corridor_scan)
        || record.work.independent_corridor_vertex_checks
            != record
                .exact_corridor_scan
                .vertex_count
                .checked_add(record.direct_corridor_scan.vertex_count)?
        || revalidate_single_triangular_hinge_prerequisites_v1(
            prerequisite,
            exact,
            paper_thickness_mm,
        )
        .is_none()
        || revalidate_axis_aligned_ef_boundary_v1(
            ef_boundary,
            prerequisite,
            exact,
            bound,
            paper_thickness_mm,
        )
        .is_none()
        || revalidate_shared_hinge_native_exact_topology_margin_v1(
            margin,
            prerequisite,
            ef_boundary,
            exact,
            bound,
            paper_thickness_mm,
        )
        .is_none()
        || margin.sealed_work()? != &record.margin_work
    {
        return None;
    }
    Some(RevalidatedIndependentSharedHingeSolidClassificationRecordV1 { record })
}

fn valid_independent_corridor_scan(scan: IndependentCorridorScanV1) -> bool {
    scan.outside_vertex_count <= scan.vertex_count
        && match (scan.outside_vertex_count, scan.first_outside_vertex_index) {
            (0, None) => true,
            (0, Some(_)) | (_, None) => false,
            (_, Some(index)) => index < scan.vertex_count,
        }
        && (!scan.every_vertex_on_finite_axis || scan.outside_vertex_count == 0)
}

fn independent_fixed_work_is_complete(work: &SharedHingeSolidClassificationWorkV1) -> bool {
    work.authenticated_faces == FACE_COUNT
        && work.authenticated_hinges == HINGE_COUNT
        && work.unordered_face_pair_count_calculations == 1
        && work.unordered_face_pairs == UNORDERED_FACE_PAIR_COUNT
        && work.triangle_pairs == TRIANGLE_PAIR_COUNT
        && work.prism_complete_scan_checks == PRISM_SCAN_COUNT
        && work.upstream_capability_revalidations == INDEPENDENT_UPSTREAM_CAPABILITY_REVALIDATIONS
        && work.sealed_prior_work_bindings == INDEPENDENT_SEALED_PRIOR_WORK_BINDINGS
        && work.root_bindings == ROOT_BINDINGS
        && work.angle_bindings == ANGLE_BINDINGS
        && work.face_identity_bindings == FACE_IDENTITY_BINDINGS
        && work.hinge_identity_bindings == HINGE_IDENTITY_BINDINGS
        && work.endpoint_identity_bindings == ENDPOINT_IDENTITY_BINDINGS
        && work.face_transform_bit_bindings == FACE_TRANSFORM_BIT_BINDINGS
        && work.hinge_parent_transform_bit_bindings == HINGE_PARENT_TRANSFORM_BIT_BINDINGS
        && work.interaction_kind_bindings == INTERACTION_KIND_BINDINGS
        && work.policy_cell_bindings == POLICY_CELL_BINDINGS
        && work.classification_seal_bindings == CLASSIFICATION_SEAL_BINDINGS
        && work.margin_component_checks == MARGIN_COMPONENT_COUNT
        && complete_prism_scan(&work.independent_exact_prism)
        && complete_prism_scan(&work.independent_direct_prism)
        && work.independent_exact_prism.exact != CayleyWork::default()
        && work.independent_direct_prism.exact != CayleyWork::default()
        && work.independent_exact_corridor != CayleyWork::default()
        && work.independent_direct_corridor != CayleyWork::default()
}

#[cfg(test)]
pub(super) fn classify_independent_exact_fixture_for_test(
    exact_solids: &[[ExactPoint3; 6]; FACE_COUNT],
    direct_solids: &[[ExactPoint3; 6]; FACE_COUNT],
    exact_corridor_components: &[BigRational; MARGIN_COMPONENT_COUNT],
    direct_corridor_components: &[BigRational; MARGIN_COMPONENT_COUNT],
) -> Result<
    (
        Option<SharedHingePositiveThicknessPairClassV1>,
        usize,
        usize,
        SharedHingeSolidClassificationWorkV1,
    ),
    SharedHingeSolidClassificationErrorV1,
> {
    let limits = SharedHingeSolidClassificationLimitsV1::default().projected();
    let mut work = SharedHingeSolidClassificationWorkV1::default();
    charge_independent_fixed_work(&mut work, &limits)
        .map_err(|_| SharedHingeSolidClassificationErrorV1::ResourceLimitExceeded)?;

    let mut exact_prism_meter = WorkMeter::new(&limits.prism.exact);
    let exact_report = analyze_exact_prebuilt_prism_pair_with_meter_v1(
        prebuilt_prism_view(&exact_solids[0]),
        prebuilt_prism_view(&exact_solids[1]),
        limits.prism,
        &mut work.independent_exact_prism,
        &mut exact_prism_meter,
    )
    .map_err(|_| SharedHingeSolidClassificationErrorV1::ResourceLimitExceeded)?
    .ok_or(SharedHingeSolidClassificationErrorV1::ResourceLimitExceeded)?;
    let mut direct_prism_meter = WorkMeter::new(&limits.prism.exact);
    let direct_report = analyze_exact_prebuilt_prism_pair_with_meter_v1(
        prebuilt_prism_view(&direct_solids[0]),
        prebuilt_prism_view(&direct_solids[1]),
        limits.prism,
        &mut work.independent_direct_prism,
        &mut direct_prism_meter,
    )
    .map_err(|_| SharedHingeSolidClassificationErrorV1::ResourceLimitExceeded)?
    .ok_or(SharedHingeSolidClassificationErrorV1::ResourceLimitExceeded)?;

    let mut exact_corridor_meter = WorkMeter::new(&limits.corridor_exact);
    let exact_scan = scan_independent_intersection_against_corridor(
        &exact_report,
        exact_corridor_components,
        &mut work.independent_corridor_vertex_checks,
        limits.max_independent_corridor_vertex_checks,
        &mut exact_corridor_meter,
    )
    .map_err(|_| SharedHingeSolidClassificationErrorV1::ResourceLimitExceeded)?;
    work.independent_exact_corridor = exact_corridor_meter.work;
    let mut direct_corridor_meter = WorkMeter::new(&limits.corridor_exact);
    let direct_scan = scan_independent_intersection_against_corridor(
        &direct_report,
        direct_corridor_components,
        &mut work.independent_corridor_vertex_checks,
        limits.max_independent_corridor_vertex_checks,
        &mut direct_corridor_meter,
    )
    .map_err(|_| SharedHingeSolidClassificationErrorV1::ResourceLimitExceeded)?;
    work.independent_direct_corridor = direct_corridor_meter.work;

    let snapshot = independent_classification_snapshot(
        IndependentPrismSummaryV1::from_report(&exact_report),
        IndependentPrismSummaryV1::from_report(&direct_report),
        exact_scan,
        direct_scan,
    );
    Ok((
        snapshot.map(|snapshot| snapshot.class),
        exact_scan.outside_vertex_count,
        direct_scan.outside_vertex_count,
        work,
    ))
}

#[cfg(test)]
pub(super) fn policy_contract_for_test(
    class: SharedHingePositiveThicknessPairClassV1,
) -> (IntersectionEvidenceV2, TopologyContactDecision) {
    let contract = policy_contract(class);
    (contract.semantic_evidence, contract.baseline_decision)
}
