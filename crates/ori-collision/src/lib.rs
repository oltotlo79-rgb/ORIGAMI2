//! Native collision/contact policy boundaries.
//!
//! This crate deliberately separates evidence classification from geometry
//! evidence generation. A caller must positively prove both the topology
//! relation and the intersection evidence before using this table.
//!
//! Native static geometry proofs are opaque runtime values and cannot be
//! persisted or reconstructed from caller-provided fields. A geometry proof
//! is intentionally not current-project authority: a stronger desktop
//! boundary must bind it to the exact current-pose certificate identity and
//! generation before calling the result a current collision certificate.
//!
//! ```compile_fail
//! use ori_collision::NativeStaticCollisionGeometryProof;
//!
//! fn require_serialize<T: serde::Serialize>() {}
//! require_serialize::<NativeStaticCollisionGeometryProof>();
//! ```

#![forbid(unsafe_code)]

#[allow(dead_code)]
mod cayley;
mod cell_order_transport;
mod certified_path_graph;
mod continuous_layer_transport;
mod continuous_path;
mod flat_endpoint_layer_order;
mod graph_positive_thickness;
mod stacked_fold_read;
mod static_collision;
mod zero_thickness;

pub use cayley::{
    MAX_COMPOSED_THICKNESS_HINGES_V1, NativeSingleHingeThicknessBoundaryV1,
    NativeTreeHingeThicknessBoundariesV1, SingleHingeThicknessBoundaryErrorV1,
    SingleHingeThicknessBoundaryObservationV1, prepare_single_hinge_thickness_boundary_v1,
    prepare_tree_hinge_thickness_boundaries_v1, revalidate_single_hinge_thickness_boundary_v1,
    revalidate_tree_hinge_thickness_boundaries_v1,
};

pub use cell_order_transport::{
    CURRENT_POSE_CELL_ORDER_MODEL_ID_V1, CellOrderTransportErrorV1, CellOrderTransportLimitsV1,
    CellOrderTransportResourceV1, CurrentPoseCellKeyV1, CurrentPoseLayerCellV1,
    NATIVE_CELL_ORDER_TRANSPORT_PROOF_V1, NativeCellOrderTransportProofV1,
    prove_single_face_cell_order_transport_v1, revalidate_single_face_cell_order_transport_v1,
};
pub use certified_path_graph::{
    CERTIFIED_PATH_GRAPH_MODEL_ID_V1, CertifiedPathGraphIndeterminateReasonV1,
    CertifiedPathGraphProgressV1, CertifiedPathGraphSearchResultV1,
    CertifiedPathTransitionCandidateV1, CertifiedPathTransitionEvidenceV1,
    CertifiedPoseGraphPathCertificateV1, MAX_CERTIFIED_PATH_GRAPH_STATES_V1,
    MAX_CERTIFIED_PATH_GRAPH_TRANSITIONS_V1, PoseFingerprintV1,
    certify_scheduled_cycle_transition_v1, search_certified_pose_graph_v1,
    search_certified_pose_graph_with_checkpoint_v1, search_certified_pose_graph_with_progress_v1,
};
pub use continuous_layer_transport::{
    CONTINUOUS_LAYER_TRANSPORT_CERTIFICATE_MODEL_ID_V1, ContinuousLayerTransportCertificateV1,
    ContinuousLayerTransportErrorV1, ContinuousLayerTransportFromPosesInputV1,
    ContinuousLayerTransportLimitsV1, derive_continuous_layer_transport_from_poses_v1,
    preflight_continuous_layer_transport_work_v1, prove_continuous_layer_transport_v1,
};
pub use continuous_path::{
    MAX_STACKED_FOLD_INTERVAL_TREE_HINGES_V1, MAX_STACKED_FOLD_PATH_SAMPLES_V1,
    STACKED_FOLD_BOUNDED_PATH_DIAGNOSTIC_MODEL_ID_V1,
    STACKED_FOLD_CACTUS_POSITIVE_THICKNESS_CONTINUOUS_CERTIFICATE_MODEL_ID_V1,
    STACKED_FOLD_COLLINEAR_TREE_CONTINUOUS_CERTIFICATE_MODEL_ID_V1,
    STACKED_FOLD_CYCLE_INTERVAL_CONTINUOUS_CERTIFICATE_MODEL_ID_V1,
    STACKED_FOLD_SINGLE_HINGE_CONTINUOUS_CERTIFICATE_MODEL_ID_V1,
    STACKED_FOLD_SINGLE_HINGE_POSITIVE_THICKNESS_CONTINUOUS_CERTIFICATE_MODEL_ID_V1,
    STACKED_FOLD_TREE_INTERVAL_CONTINUOUS_CERTIFICATE_MODEL_ID_V1,
    STACKED_FOLD_TWO_HINGE_INTERVAL_CONTINUOUS_CERTIFICATE_MODEL_ID_V1,
    STACKED_FOLD_TWO_HINGE_POSITIVE_THICKNESS_CONTINUOUS_CERTIFICATE_MODEL_ID_V1,
    StackedFoldBoundedPathDiagnosticV1, StackedFoldCyclePathDiagnosticV1,
    StackedFoldPathDiagnosticErrorV1, StackedFoldPathDiagnosticLimitsV1,
    UniformCycleClosureRootsV1, diagnose_canonical_cycle_schedule_path_v1,
    diagnose_canonical_positive_thickness_cycle_schedule_path_v1,
    diagnose_collective_cycle_path_v1, diagnose_collective_hinge_path_v1,
    diagnose_scheduled_cycle_path_v1, diagnose_scheduled_positive_thickness_cycle_path_v1,
    enumerate_uniform_cycle_closure_roots_v1, supports_scheduled_positive_thickness_path_v1,
};
pub use flat_endpoint_layer_order::{
    FLAT_ENDPOINT_LAYER_ORDER_ANCHOR_MODEL_ID_V1, FlatEndpointCellKeyV1, FlatEndpointLayerCellV1,
    FlatEndpointLayerOrderAnchorErrorV1, FlatEndpointLayerOrderInputV1,
    FlatEndpointLayerOrderLimitsV1, FlatEndpointLayerOrderResourceV1, FlatEndpointLayerOrderWorkV1,
    NativeFlatEndpointLayerOrderAnchorV1, anchor_flat_endpoint_layer_order_v1,
    revalidate_flat_endpoint_layer_order_anchor_v1,
};
pub use graph_positive_thickness::{
    NativePositiveThicknessGraphGeometryProofV1, POSITIVE_THICKNESS_GRAPH_GEOMETRY_PROOF_V1,
    PositiveThicknessGraphLimitsV1, PositiveThicknessGraphProofErrorV1,
    prove_positive_thickness_graph_geometry_v1,
};
pub use stacked_fold_read::{
    NativeStackedFoldMaterialMapV1, NativeStackedFoldReadGuardV1, NativeStackedFoldReadProposalV1,
    STACKED_FOLD_MATERIAL_MAP_MODEL_ID_V1, STACKED_FOLD_READ_GUARD_MODEL_ID_V1,
    STACKED_FOLD_READ_PROPOSAL_MODEL_ID_V1, StackedFoldFixedSideV1, StackedFoldLinearCandidateV1,
    StackedFoldMaterialMapErrorV1, StackedFoldMaterialMapLimitsV1, StackedFoldMaterialSegmentV1,
    StackedFoldReadBindingV1, StackedFoldReadCellV1, StackedFoldReadErrorV1,
    StackedFoldReadFailureClassV1, StackedFoldReadLimitsV1, StackedFoldReadResourceV1,
    StackedFoldReadSupportV1, StackedFoldReadWorkV1, StackedFoldRotationDirectionV1,
    capture_stacked_fold_read_guard_v1, propose_linear_stacked_fold_read_v1,
    revalidate_linear_stacked_fold_read_proposal_v1, revalidate_stacked_fold_read_guard_v1,
    reverse_map_linear_stacked_fold_material_v1,
};
pub use static_collision::{
    CENTERED_MID_SURFACE_THICKNESS_MODEL_V1, NATIVE_STATIC_COLLISION_GEOMETRY_PROOF_V1,
    NATIVE_STATIC_COLLISION_MAX_PAIR_DIAGNOSTICS_V1, NativePositiveThicknessPairSeparationV1,
    NativeStaticCollisionGeometryProof, StaticCollisionDiagnosticSnapshot, StaticCollisionError,
    StaticCollisionLimits, StaticCollisionPairDiagnostic, StaticCollisionPairDisposition,
    classify_static_collision_pair_disposition, diagnose_static_collision_geometry,
    prepare_positive_thickness_pair_separation_v1, prove_static_collision_geometry,
    revalidate_positive_thickness_pair_separation_v1,
};

/// Immutable identifier for the first native topology/contact policy.
pub const TOPOLOGY_CONTACT_POLICY_V1: &str = "topology_contact_policy_v1";

/// Immutable identifier for the policy that adds positive-area boundary
/// contact between positive-thickness material solids.
pub const TOPOLOGY_CONTACT_POLICY_V2: &str = "topology_contact_policy_v2";

/// Policy label for the topology relation between two material faces.
///
/// Constructing this enum does not authenticate the relation. A geometry
/// engine must bind a positively verified relation to its private evidence
/// certificate before this label can affect a runtime collision result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TopologyRelation {
    NoSharedFeature,
    SharedVertex,
    SharedHingeEdge,
    SameFace,
}

impl TopologyRelation {
    /// Canonical order shared by the normative v1 and v2 policy corpora.
    pub const ALL: [Self; 4] = [
        Self::NoSharedFeature,
        Self::SharedVertex,
        Self::SharedHingeEdge,
        Self::SameFace,
    ];

    #[must_use]
    pub const fn identifier(self) -> &'static str {
        match self {
            Self::NoSharedFeature => "no_shared_feature",
            Self::SharedVertex => "shared_vertex",
            Self::SharedHingeEdge => "shared_hinge_edge",
            Self::SameFace => "same_face",
        }
    }

    const fn table_index(self) -> usize {
        match self {
            Self::NoSharedFeature => 0,
            Self::SharedVertex => 1,
            Self::SharedHingeEdge => 2,
            Self::SameFace => 3,
        }
    }
}

/// Policy label for geometry evidence about one material-face pair.
///
/// Constructing this enum does not prove the evidence. Runtime callers must
/// derive it inside a private, pose-bound evidence boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IntersectionEvidence {
    Separated,
    PointContact,
    BoundaryLineContact,
    SharedFeatureContact,
    SharedFeatureThicknessOverlap,
    SharedFeatureFlatStack,
    CoplanarAreaOverlap,
    TransversalCrossing,
    PositiveVolumeOverlap,
    Indeterminate,
}

impl IntersectionEvidence {
    /// Canonical order used by the normative 4 × 10 policy corpus.
    pub const ALL: [Self; 10] = [
        Self::Separated,
        Self::PointContact,
        Self::BoundaryLineContact,
        Self::SharedFeatureContact,
        Self::SharedFeatureThicknessOverlap,
        Self::SharedFeatureFlatStack,
        Self::CoplanarAreaOverlap,
        Self::TransversalCrossing,
        Self::PositiveVolumeOverlap,
        Self::Indeterminate,
    ];

    #[must_use]
    pub const fn identifier(self) -> &'static str {
        match self {
            Self::Separated => "separated",
            Self::PointContact => "point_contact",
            Self::BoundaryLineContact => "boundary_line_contact",
            Self::SharedFeatureContact => "shared_feature_contact",
            Self::SharedFeatureThicknessOverlap => "shared_feature_thickness_overlap",
            Self::SharedFeatureFlatStack => "shared_feature_flat_stack",
            Self::CoplanarAreaOverlap => "coplanar_area_overlap",
            Self::TransversalCrossing => "transversal_crossing",
            Self::PositiveVolumeOverlap => "positive_volume_overlap",
            Self::Indeterminate => "indeterminate",
        }
    }

    const fn table_index(self) -> usize {
        match self {
            Self::Separated => 0,
            Self::PointContact => 1,
            Self::BoundaryLineContact => 2,
            Self::SharedFeatureContact => 3,
            Self::SharedFeatureThicknessOverlap => 4,
            Self::SharedFeatureFlatStack => 5,
            Self::CoplanarAreaOverlap => 6,
            Self::TransversalCrossing => 7,
            Self::PositiveVolumeOverlap => 8,
            Self::Indeterminate => 9,
        }
    }
}

/// Geometry evidence admitted by [`TOPOLOGY_CONTACT_POLICY_V2`].
///
/// V2 preserves every V1 evidence kind and adds `BoundaryAreaContact` for a
/// positive-area, zero-positive-volume intersection of two positive-thickness
/// material-solid boundaries. Constructing this enum is not a proof; runtime
/// evidence generators must bind a positive proof to the exact pose and pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IntersectionEvidenceV2 {
    Separated,
    PointContact,
    BoundaryLineContact,
    BoundaryAreaContact,
    SharedFeatureContact,
    SharedFeatureThicknessOverlap,
    SharedFeatureFlatStack,
    CoplanarAreaOverlap,
    TransversalCrossing,
    PositiveVolumeOverlap,
    Indeterminate,
}

impl IntersectionEvidenceV2 {
    /// Canonical order used by the normative 4 × 11 policy corpus.
    pub const ALL: [Self; 11] = [
        Self::Separated,
        Self::PointContact,
        Self::BoundaryLineContact,
        Self::BoundaryAreaContact,
        Self::SharedFeatureContact,
        Self::SharedFeatureThicknessOverlap,
        Self::SharedFeatureFlatStack,
        Self::CoplanarAreaOverlap,
        Self::TransversalCrossing,
        Self::PositiveVolumeOverlap,
        Self::Indeterminate,
    ];

    #[must_use]
    pub const fn identifier(self) -> &'static str {
        match self {
            Self::Separated => "separated",
            Self::PointContact => "point_contact",
            Self::BoundaryLineContact => "boundary_line_contact",
            Self::BoundaryAreaContact => "boundary_area_contact",
            Self::SharedFeatureContact => "shared_feature_contact",
            Self::SharedFeatureThicknessOverlap => "shared_feature_thickness_overlap",
            Self::SharedFeatureFlatStack => "shared_feature_flat_stack",
            Self::CoplanarAreaOverlap => "coplanar_area_overlap",
            Self::TransversalCrossing => "transversal_crossing",
            Self::PositiveVolumeOverlap => "positive_volume_overlap",
            Self::Indeterminate => "indeterminate",
        }
    }

    /// Embeds one frozen V1 evidence kind into V2 without changing its
    /// semantics.
    #[must_use]
    pub const fn from_v1(evidence: IntersectionEvidence) -> Self {
        match evidence {
            IntersectionEvidence::Separated => Self::Separated,
            IntersectionEvidence::PointContact => Self::PointContact,
            IntersectionEvidence::BoundaryLineContact => Self::BoundaryLineContact,
            IntersectionEvidence::SharedFeatureContact => Self::SharedFeatureContact,
            IntersectionEvidence::SharedFeatureThicknessOverlap => {
                Self::SharedFeatureThicknessOverlap
            }
            IntersectionEvidence::SharedFeatureFlatStack => Self::SharedFeatureFlatStack,
            IntersectionEvidence::CoplanarAreaOverlap => Self::CoplanarAreaOverlap,
            IntersectionEvidence::TransversalCrossing => Self::TransversalCrossing,
            IntersectionEvidence::PositiveVolumeOverlap => Self::PositiveVolumeOverlap,
            IntersectionEvidence::Indeterminate => Self::Indeterminate,
        }
    }

    const fn table_index(self) -> usize {
        match self {
            Self::Separated => 0,
            Self::PointContact => 1,
            Self::BoundaryLineContact => 2,
            Self::BoundaryAreaContact => 3,
            Self::SharedFeatureContact => 4,
            Self::SharedFeatureThicknessOverlap => 5,
            Self::SharedFeatureFlatStack => 6,
            Self::CoplanarAreaOverlap => 7,
            Self::TransversalCrossing => 8,
            Self::PositiveVolumeOverlap => 9,
            Self::Indeterminate => 10,
        }
    }
}

/// Policy result. This is not itself a geometry certificate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TopologyContactDecision {
    Separated,
    Touching,
    AllowedSharedVertexContact,
    RequiresHingeModel,
    Penetrating,
    Indeterminate,
    IgnoredSelf,
}

impl TopologyContactDecision {
    #[must_use]
    pub const fn identifier(self) -> &'static str {
        match self {
            Self::Separated => "separated",
            Self::Touching => "touching",
            Self::AllowedSharedVertexContact => "allowed_shared_vertex_contact",
            Self::RequiresHingeModel => "requires_hinge_model",
            Self::Penetrating => "penetrating",
            Self::Indeterminate => "indeterminate",
            Self::IgnoredSelf => "ignored_self",
        }
    }
}

use TopologyContactDecision::{
    AllowedSharedVertexContact, IgnoredSelf, Indeterminate, Penetrating, RequiresHingeModel,
    Separated, Touching,
};

const TOPOLOGY_CONTACT_POLICY_TABLE_V1: [[TopologyContactDecision; 10]; 4] = [
    [
        Separated,
        Touching,
        Touching,
        Indeterminate,
        Indeterminate,
        Indeterminate,
        Penetrating,
        Penetrating,
        Penetrating,
        Indeterminate,
    ],
    [
        Indeterminate,
        Touching,
        Touching,
        AllowedSharedVertexContact,
        AllowedSharedVertexContact,
        Indeterminate,
        Penetrating,
        Penetrating,
        Penetrating,
        Indeterminate,
    ],
    [
        Indeterminate,
        Indeterminate,
        Indeterminate,
        RequiresHingeModel,
        RequiresHingeModel,
        RequiresHingeModel,
        Penetrating,
        Penetrating,
        Penetrating,
        Indeterminate,
    ],
    [IgnoredSelf; 10],
];

const TOPOLOGY_CONTACT_POLICY_TABLE_V2: [[TopologyContactDecision; 11]; 4] = [
    [
        Separated,
        Touching,
        Touching,
        Touching,
        Indeterminate,
        Indeterminate,
        Indeterminate,
        Penetrating,
        Penetrating,
        Penetrating,
        Indeterminate,
    ],
    [
        Indeterminate,
        Touching,
        Touching,
        Touching,
        AllowedSharedVertexContact,
        AllowedSharedVertexContact,
        Indeterminate,
        Penetrating,
        Penetrating,
        Penetrating,
        Indeterminate,
    ],
    [
        Indeterminate,
        Indeterminate,
        Indeterminate,
        RequiresHingeModel,
        RequiresHingeModel,
        RequiresHingeModel,
        RequiresHingeModel,
        Penetrating,
        Penetrating,
        Penetrating,
        Indeterminate,
    ],
    [IgnoredSelf; 11],
];

/// Applies the complete topology relation × intersection evidence policy.
///
/// This pure function does not prove either input and does not grant a hinge
/// exception. `RequiresHingeModel` is an obligation to run the separate,
/// finite shared-hinge model. `IgnoredSelf` describes table semantics; a
/// runtime pair dispatcher must normally remove same-face pairs before
/// evidence classification.
#[must_use]
pub const fn classify_topology_contact_v1(
    topology: TopologyRelation,
    evidence: IntersectionEvidence,
) -> TopologyContactDecision {
    TOPOLOGY_CONTACT_POLICY_TABLE_V1[topology.table_index()][evidence.table_index()]
}

/// Applies the complete V2 topology relation × intersection evidence policy.
///
/// `BoundaryAreaContact` is a generic non-penetrating contact for unrelated
/// or shared-vertex faces. A shared-hinge pair must still pass the separate
/// finite-axis hinge model; this pure table never grants that exception.
#[must_use]
pub const fn classify_topology_contact_v2(
    topology: TopologyRelation,
    evidence: IntersectionEvidenceV2,
) -> TopologyContactDecision {
    TOPOLOGY_CONTACT_POLICY_TABLE_V2[topology.table_index()][evidence.table_index()]
}

/// Applies the V2 table at the runtime pair-dispatch boundary.
///
/// A correctly constructed dispatcher enumerates each unordered pair once and
/// therefore never observes [`TopologyRelation::SameFace`]. Reaching that
/// relation at runtime is an internal coverage/identity inconsistency, not an
/// `IgnoredSelf` permission. It consequently fails closed as
/// [`TopologyContactDecision::Indeterminate`].
///
/// Like the policy-table function, this function does not authenticate either
/// input and is not a collision certificate.
#[must_use]
pub const fn classify_runtime_topology_contact_v2(
    topology: TopologyRelation,
    evidence: IntersectionEvidenceV2,
) -> TopologyContactDecision {
    if matches!(topology, TopologyRelation::SameFace) {
        TopologyContactDecision::Indeterminate
    } else {
        classify_topology_contact_v2(topology, evidence)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use serde::Deserialize;

    use super::*;

    #[derive(Debug, Deserialize)]
    struct NormativePolicyCorpus {
        policy_id: String,
        topology_relations: Vec<String>,
        intersection_evidence: Vec<String>,
        decisions: BTreeMap<String, Vec<String>>,
    }

    fn normative_corpus_v1() -> NormativePolicyCorpus {
        serde_json::from_str(include_str!(
            "../../../docs/collision-contact-policy-v1.json"
        ))
        .expect("the normative V1 policy corpus must remain valid JSON")
    }

    fn normative_corpus_v2() -> NormativePolicyCorpus {
        serde_json::from_str(include_str!(
            "../../../docs/collision-contact-policy-v2.json"
        ))
        .expect("the normative V2 policy corpus must remain valid JSON")
    }

    #[test]
    fn normative_corpus_and_native_table_match_all_forty_cells() {
        let corpus = normative_corpus_v1();
        assert_eq!(corpus.policy_id, TOPOLOGY_CONTACT_POLICY_V1);
        assert_eq!(
            corpus.topology_relations,
            TopologyRelation::ALL
                .map(TopologyRelation::identifier)
                .map(str::to_owned)
        );
        assert_eq!(
            corpus.intersection_evidence,
            IntersectionEvidence::ALL
                .map(IntersectionEvidence::identifier)
                .map(str::to_owned)
        );
        assert_eq!(
            corpus.topology_relations.len() * corpus.intersection_evidence.len(),
            40
        );

        for topology in TopologyRelation::ALL {
            let expected = corpus
                .decisions
                .get(topology.identifier())
                .unwrap_or_else(|| panic!("missing corpus row for {}", topology.identifier()));
            let actual = IntersectionEvidence::ALL
                .map(|evidence| classify_topology_contact_v1(topology, evidence).identifier());
            assert_eq!(
                expected,
                &actual.map(str::to_owned),
                "{}",
                topology.identifier()
            );
        }
        assert_eq!(
            corpus.decisions.keys().cloned().collect::<BTreeSet<_>>(),
            TopologyRelation::ALL
                .map(TopologyRelation::identifier)
                .map(str::to_owned)
                .into_iter()
                .collect()
        );
    }

    #[test]
    fn v2_corpus_and_native_table_match_all_forty_four_cells() {
        let corpus = normative_corpus_v2();
        assert_eq!(corpus.policy_id, TOPOLOGY_CONTACT_POLICY_V2);
        assert_eq!(
            corpus.topology_relations,
            TopologyRelation::ALL
                .map(TopologyRelation::identifier)
                .map(str::to_owned)
        );
        assert_eq!(
            corpus.intersection_evidence,
            IntersectionEvidenceV2::ALL
                .map(IntersectionEvidenceV2::identifier)
                .map(str::to_owned)
        );
        assert_eq!(
            corpus.topology_relations.len() * corpus.intersection_evidence.len(),
            44
        );

        for topology in TopologyRelation::ALL {
            let expected = corpus
                .decisions
                .get(topology.identifier())
                .unwrap_or_else(|| panic!("missing corpus row for {}", topology.identifier()));
            let actual = IntersectionEvidenceV2::ALL
                .map(|evidence| classify_topology_contact_v2(topology, evidence).identifier());
            assert_eq!(
                expected,
                &actual.map(str::to_owned),
                "{}",
                topology.identifier()
            );
        }
        assert_eq!(
            corpus.decisions.keys().cloned().collect::<BTreeSet<_>>(),
            TopologyRelation::ALL
                .map(TopologyRelation::identifier)
                .map(str::to_owned)
                .into_iter()
                .collect()
        );
    }

    #[test]
    fn v2_embeds_every_v1_cell_without_reinterpretation() {
        for topology in TopologyRelation::ALL {
            for evidence in IntersectionEvidence::ALL {
                assert_eq!(
                    classify_topology_contact_v2(
                        topology,
                        IntersectionEvidenceV2::from_v1(evidence)
                    ),
                    classify_topology_contact_v1(topology, evidence),
                    "{}:{}",
                    topology.identifier(),
                    evidence.identifier()
                );
            }
        }
    }

    #[test]
    fn positive_area_boundary_contact_has_all_four_fixed_decisions() {
        for (topology, expected) in [
            (
                TopologyRelation::NoSharedFeature,
                TopologyContactDecision::Touching,
            ),
            (
                TopologyRelation::SharedVertex,
                TopologyContactDecision::Touching,
            ),
            (
                TopologyRelation::SharedHingeEdge,
                TopologyContactDecision::RequiresHingeModel,
            ),
            (
                TopologyRelation::SameFace,
                TopologyContactDecision::IgnoredSelf,
            ),
        ] {
            assert_eq!(
                classify_topology_contact_v2(topology, IntersectionEvidenceV2::BoundaryAreaContact),
                expected,
                "{}",
                topology.identifier()
            );
        }
    }

    #[test]
    fn shared_identity_never_exempts_area_transversal_or_volume_penetration() {
        for topology in [
            TopologyRelation::SharedVertex,
            TopologyRelation::SharedHingeEdge,
        ] {
            for evidence in [
                IntersectionEvidence::CoplanarAreaOverlap,
                IntersectionEvidence::TransversalCrossing,
                IntersectionEvidence::PositiveVolumeOverlap,
            ] {
                assert_eq!(
                    classify_topology_contact_v1(topology, evidence),
                    TopologyContactDecision::Penetrating,
                    "{}:{}",
                    topology.identifier(),
                    evidence.identifier()
                );
            }
        }
    }

    #[test]
    fn only_feature_bound_evidence_can_enter_a_topology_allowance() {
        for topology in TopologyRelation::ALL {
            for evidence in IntersectionEvidence::ALL {
                let decision = classify_topology_contact_v1(topology, evidence);
                if !matches!(
                    decision,
                    TopologyContactDecision::AllowedSharedVertexContact
                        | TopologyContactDecision::RequiresHingeModel
                ) {
                    continue;
                }
                assert!(
                    matches!(
                        evidence,
                        IntersectionEvidence::SharedFeatureContact
                            | IntersectionEvidence::SharedFeatureThicknessOverlap
                            | IntersectionEvidence::SharedFeatureFlatStack
                    ),
                    "{}:{}",
                    topology.identifier(),
                    evidence.identifier()
                );
                if evidence == IntersectionEvidence::SharedFeatureFlatStack {
                    assert_eq!(topology, TopologyRelation::SharedHingeEdge);
                }
            }
        }
    }

    #[test]
    fn impossible_shared_feature_cells_fail_closed() {
        assert_eq!(
            classify_topology_contact_v1(
                TopologyRelation::SharedVertex,
                IntersectionEvidence::Separated
            ),
            TopologyContactDecision::Indeterminate
        );
        for evidence in [
            IntersectionEvidence::Separated,
            IntersectionEvidence::PointContact,
            IntersectionEvidence::BoundaryLineContact,
        ] {
            assert_eq!(
                classify_topology_contact_v1(TopologyRelation::SharedHingeEdge, evidence),
                TopologyContactDecision::Indeterminate,
                "{}",
                evidence.identifier()
            );
        }
    }

    #[test]
    fn same_face_is_ignored_for_every_evidence_cell() {
        for evidence in IntersectionEvidence::ALL {
            assert_eq!(
                classify_topology_contact_v1(TopologyRelation::SameFace, evidence),
                TopologyContactDecision::IgnoredSelf
            );
        }
    }

    #[test]
    fn runtime_same_face_arrival_is_an_internal_indeterminate() {
        for evidence in IntersectionEvidenceV2::ALL {
            assert_eq!(
                classify_runtime_topology_contact_v2(TopologyRelation::SameFace, evidence),
                TopologyContactDecision::Indeterminate,
                "{}",
                evidence.identifier()
            );
        }
        for topology in [
            TopologyRelation::NoSharedFeature,
            TopologyRelation::SharedVertex,
            TopologyRelation::SharedHingeEdge,
        ] {
            for evidence in IntersectionEvidenceV2::ALL {
                assert_eq!(
                    classify_runtime_topology_contact_v2(topology, evidence),
                    classify_topology_contact_v2(topology, evidence),
                    "{}:{}",
                    topology.identifier(),
                    evidence.identifier()
                );
            }
        }
    }
}
