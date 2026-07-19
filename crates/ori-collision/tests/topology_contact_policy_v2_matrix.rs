use std::collections::{BTreeMap, BTreeSet};

use ori_collision::{
    IntersectionEvidenceV2, TOPOLOGY_CONTACT_POLICY_V2, TopologyContactDecision, TopologyRelation,
    classify_runtime_topology_contact_v2, classify_topology_contact_v2,
};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct NormativePolicyCorpus {
    policy_id: String,
    topology_relations: Vec<String>,
    intersection_evidence: Vec<String>,
    decisions: BTreeMap<String, Vec<String>>,
}

fn normative_corpus() -> NormativePolicyCorpus {
    serde_json::from_str(include_str!(
        "../../../docs/collision-contact-policy-v2.json"
    ))
    .expect("the normative V2 policy corpus must remain valid JSON")
}

#[test]
fn every_v2_policy_cell_has_one_canonical_case_and_one_public_result() {
    let corpus = normative_corpus();
    let topology_identifiers = TopologyRelation::ALL
        .map(TopologyRelation::identifier)
        .map(str::to_owned);
    let evidence_identifiers = IntersectionEvidenceV2::ALL
        .map(IntersectionEvidenceV2::identifier)
        .map(str::to_owned);

    assert_eq!(corpus.policy_id, TOPOLOGY_CONTACT_POLICY_V2);
    assert_eq!(corpus.topology_relations, topology_identifiers);
    assert_eq!(corpus.intersection_evidence, evidence_identifiers);
    assert_eq!(
        corpus
            .topology_relations
            .iter()
            .collect::<BTreeSet<_>>()
            .len(),
        TopologyRelation::ALL.len(),
        "a topology axis entry is duplicated"
    );
    assert_eq!(
        corpus
            .intersection_evidence
            .iter()
            .collect::<BTreeSet<_>>()
            .len(),
        IntersectionEvidenceV2::ALL.len(),
        "an evidence axis entry is duplicated"
    );
    assert_eq!(
        corpus.decisions.keys().cloned().collect::<BTreeSet<_>>(),
        topology_identifiers.into_iter().collect(),
        "the corpus must contain exactly the four canonical rows"
    );

    let valid_decisions = [
        "separated",
        "touching",
        "allowed_shared_vertex_contact",
        "requires_hinge_model",
        "penetrating",
        "indeterminate",
        "ignored_self",
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    let mut canonical_cell_ids = BTreeSet::new();

    for topology in TopologyRelation::ALL {
        let row = corpus
            .decisions
            .get(topology.identifier())
            .unwrap_or_else(|| panic!("missing row for {}", topology.identifier()));
        assert_eq!(
            row.len(),
            IntersectionEvidenceV2::ALL.len(),
            "{} must contain one decision for every evidence kind",
            topology.identifier()
        );

        for (evidence_index, evidence) in IntersectionEvidenceV2::ALL.into_iter().enumerate() {
            let cell_id = format!("{}::{}", topology.identifier(), evidence.identifier());
            assert!(
                canonical_cell_ids.insert(cell_id.clone()),
                "duplicate canonical cell {cell_id}"
            );
            let expected = &row[evidence_index];
            assert!(
                valid_decisions.contains(expected.as_str()),
                "unknown decision in {cell_id}: {expected}"
            );

            let policy_decision = classify_topology_contact_v2(topology, evidence);
            assert_eq!(
                policy_decision.identifier(),
                expected,
                "normative decision mismatch in {cell_id}"
            );

            let expected_runtime = if topology == TopologyRelation::SameFace {
                TopologyContactDecision::Indeterminate
            } else {
                policy_decision
            };
            assert_eq!(
                classify_runtime_topology_contact_v2(topology, evidence),
                expected_runtime,
                "runtime boundary mismatch in {cell_id}"
            );
        }
    }

    assert_eq!(
        canonical_cell_ids.len(),
        TopologyRelation::ALL.len() * IntersectionEvidenceV2::ALL.len()
    );
    assert_eq!(canonical_cell_ids.len(), 44);
}

#[test]
fn penetration_dimensions_are_never_exempted_by_shared_identity() {
    for topology in [
        TopologyRelation::NoSharedFeature,
        TopologyRelation::SharedVertex,
        TopologyRelation::SharedHingeEdge,
    ] {
        for evidence in [
            IntersectionEvidenceV2::CoplanarAreaOverlap,
            IntersectionEvidenceV2::TransversalCrossing,
            IntersectionEvidenceV2::PositiveVolumeOverlap,
        ] {
            assert_eq!(
                classify_topology_contact_v2(topology, evidence),
                TopologyContactDecision::Penetrating,
                "{}::{}",
                topology.identifier(),
                evidence.identifier()
            );
        }
    }
}
