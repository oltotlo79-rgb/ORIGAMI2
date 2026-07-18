//! Native collision/contact policy boundaries.
//!
//! This crate deliberately separates evidence classification from geometry
//! evidence generation. A caller must positively prove both the topology
//! relation and the intersection evidence before using this table.

/// Immutable identifier for the first native topology/contact policy.
pub const TOPOLOGY_CONTACT_POLICY_V1: &str = "topology_contact_policy_v1";

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
    /// Canonical order used by the normative 4 × 10 policy corpus.
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

    fn normative_corpus() -> NormativePolicyCorpus {
        serde_json::from_str(include_str!(
            "../../../docs/collision-contact-policy-v1.json"
        ))
        .expect("the normative policy corpus must remain valid JSON")
    }

    #[test]
    fn normative_corpus_and_native_table_match_all_forty_cells() {
        let corpus = normative_corpus();
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
}
