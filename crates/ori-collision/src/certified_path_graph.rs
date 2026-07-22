//! Deterministic bounded graph search over a native certified-transition oracle.
//!
//! This module is observation-only. Its result never authorizes project
//! mutation: an oracle must independently certify every admitted transition.

use std::collections::{BTreeMap, VecDeque};

use ori_domain::FaceId;
use ori_kinematics::{
    DyadicMaterialHingeIntervalClosureCertificateV1, GeneratedMultiHingePathCandidateV1,
    MaterialHingeGraphAudit, MaterialHingeGraphGeometry,
};
use sha2::{Digest, Sha256};

use crate::continuous_path::diagnose_scheduled_cycle_path_v1;

pub const CERTIFIED_PATH_GRAPH_MODEL_ID_V1: &str = "bounded_certified_pose_graph_path_v1";
pub const MAX_CERTIFIED_PATH_GRAPH_STATES_V1: usize = 32;
pub const MAX_CERTIFIED_PATH_GRAPH_TRANSITIONS_V1: usize = 64;

pub type PoseFingerprintV1 = [u8; 32];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CertifiedPathTransitionCandidateV1 {
    pub source: PoseFingerprintV1,
    pub target: PoseFingerprintV1,
    /// Stable oracle-specific ordering key. It contains no project identity.
    pub candidate_key: [u8; 32],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CertifiedPathTransitionEvidenceV1 {
    source: PoseFingerprintV1,
    target: PoseFingerprintV1,
    schedule_certificate: [u8; 32],
    collision_certificate: [u8; 32],
    closure_certificate: [u8; 32],
}

impl CertifiedPathTransitionEvidenceV1 {
    /// Creates the detached evidence returned by a native edge oracle.
    ///
    /// Construction alone is not authority. The graph certificate remains
    /// read-only and records exactly which three edge certificates the oracle
    /// bound to the source and target.
    #[must_use]
    pub const fn from_native_oracle(
        source: PoseFingerprintV1,
        target: PoseFingerprintV1,
        schedule_certificate: [u8; 32],
        collision_certificate: [u8; 32],
        closure_certificate: [u8; 32],
    ) -> Self {
        Self {
            source,
            target,
            schedule_certificate,
            collision_certificate,
            closure_certificate,
        }
    }

    #[must_use]
    pub const fn source(&self) -> PoseFingerprintV1 {
        self.source
    }
    #[must_use]
    pub const fn target(&self) -> PoseFingerprintV1 {
        self.target
    }
    #[must_use]
    pub const fn schedule_certificate(&self) -> [u8; 32] {
        self.schedule_certificate
    }
    #[must_use]
    pub const fn collision_certificate(&self) -> [u8; 32] {
        self.collision_certificate
    }
    #[must_use]
    pub const fn closure_certificate(&self) -> [u8; 32] {
        self.closure_certificate
    }
}

/// Adapts the existing schedule, full-domain closure and bounded CCD oracles
/// into one graph edge. Any missing or mismatched certificate rejects the
/// edge; an unresolved CCD result is never interpreted as collision-free.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn certify_scheduled_cycle_transition_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    candidate: &GeneratedMultiHingePathCandidateV1,
    closure: &DyadicMaterialHingeIntervalClosureCertificateV1,
    interval_count: usize,
    source: PoseFingerprintV1,
    target: PoseFingerprintV1,
) -> Option<CertifiedPathTransitionEvidenceV1> {
    let schedule = candidate.schedule();
    if closure.fixed_face() != fixed_face
        || !closure.every_leaf_covers_graph_v1(geometry)
        || closure.schedule_binding_fingerprint_v1()
            != schedule.certificate_binding_fingerprint_v1()
        || closure.graph_binding_fingerprint_v1() != schedule.graph_binding_fingerprint_v1()
        || !schedule.matches_binding(geometry, audit, fixed_face)
    {
        return None;
    }
    let collision = diagnose_scheduled_cycle_path_v1(
        geometry,
        audit,
        fixed_face,
        candidate,
        closure,
        interval_count,
    );
    collision.continuous_certificate_model_id()?;
    let schedule_certificate = schedule.certificate_binding_fingerprint_v1();
    let closure_certificate = hash_certificate_binding(
        b"dyadic_material_hinge_interval_closure_certificate_v1",
        &[
            &schedule_certificate,
            &closure.graph_binding_fingerprint_v1(),
            &closure.partition_binding_fingerprint_v1(),
        ],
    );
    let collision_certificate = hash_certificate_binding(
        b"stacked_fold_cycle_interval_continuous_certificate_v1",
        &[
            &schedule_certificate,
            &closure_certificate,
            &(collision.leaf_count() as u64).to_be_bytes(),
            &(collision.pair_work() as u64).to_be_bytes(),
        ],
    );
    Some(CertifiedPathTransitionEvidenceV1::from_native_oracle(
        source,
        target,
        schedule_certificate,
        collision_certificate,
        closure_certificate,
    ))
}

fn hash_certificate_binding(domain: &[u8], fields: &[&[u8]]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(domain);
    for field in fields {
        hash.update((field.len() as u64).to_be_bytes());
        hash.update(field);
    }
    hash.finalize().into()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CertifiedPoseGraphPathCertificateV1 {
    source: PoseFingerprintV1,
    target: PoseFingerprintV1,
    edges: Vec<CertifiedPathTransitionEvidenceV1>,
    explored_state_count: usize,
    evaluated_transition_count: usize,
}

impl CertifiedPoseGraphPathCertificateV1 {
    #[must_use]
    pub const fn model_id(&self) -> &'static str {
        CERTIFIED_PATH_GRAPH_MODEL_ID_V1
    }
    #[must_use]
    pub const fn version(&self) -> u8 {
        1
    }
    #[must_use]
    pub const fn source(&self) -> PoseFingerprintV1 {
        self.source
    }
    #[must_use]
    pub const fn target(&self) -> PoseFingerprintV1 {
        self.target
    }
    #[must_use]
    pub fn edges(&self) -> &[CertifiedPathTransitionEvidenceV1] {
        &self.edges
    }
    #[must_use]
    pub const fn explored_state_count(&self) -> usize {
        self.explored_state_count
    }
    #[must_use]
    pub const fn evaluated_transition_count(&self) -> usize {
        self.evaluated_transition_count
    }
    #[must_use]
    pub const fn authorizes_project_mutation(&self) -> bool {
        false
    }

    /// Canonical digest binding every endpoint, transition proof and search count.
    #[must_use]
    pub fn binding_fingerprint_v1(&self) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(b"certified_pose_graph_path_certificate_binding_v1");
        hash.update(self.source);
        hash.update(self.target);
        hash.update((self.edges.len() as u64).to_be_bytes());
        for edge in &self.edges {
            hash.update(edge.source());
            hash.update(edge.target());
            hash.update(edge.schedule_certificate());
            hash.update(edge.collision_certificate());
            hash.update(edge.closure_certificate());
        }
        hash.update((self.explored_state_count as u64).to_be_bytes());
        hash.update((self.evaluated_transition_count as u64).to_be_bytes());
        hash.finalize().into()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CertifiedPathGraphIndeterminateReasonV1 {
    ResourceLimit,
    NoCertifiedPath,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CertifiedPathGraphSearchResultV1 {
    Certified(CertifiedPoseGraphPathCertificateV1),
    Indeterminate {
        reason: CertifiedPathGraphIndeterminateReasonV1,
        explored_state_count: usize,
        evaluated_transition_count: usize,
    },
}

/// Detached observation emitted while a bounded search is running.
///
/// Progress is never certificate evidence and never authorizes mutation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CertifiedPathGraphProgressV1 {
    pub explored_state_count: usize,
    pub evaluated_transition_count: usize,
    pub state_limit: usize,
    pub transition_limit: usize,
}

/// Runs canonical breadth-first search over at most 32 states and 64 candidate
/// transitions. The oracle is called once per reachable canonical candidate;
/// only exact source/target-bound evidence is admitted.
pub fn search_certified_pose_graph_v1(
    states: &[PoseFingerprintV1],
    transitions: &[CertifiedPathTransitionCandidateV1],
    source: PoseFingerprintV1,
    target: PoseFingerprintV1,
    oracle: impl FnMut(&CertifiedPathTransitionCandidateV1) -> Option<CertifiedPathTransitionEvidenceV1>,
) -> CertifiedPathGraphSearchResultV1 {
    search_certified_pose_graph_with_progress_v1(
        states,
        transitions,
        source,
        target,
        || true,
        |_| {},
        oracle,
    )
}

/// Cancellable form of [`search_certified_pose_graph_v1`]. The checkpoint is
/// observed before every state, every transition oracle call and certificate
/// publication. Cancellation never publishes a partial path.
pub fn search_certified_pose_graph_with_checkpoint_v1(
    states: &[PoseFingerprintV1],
    transitions: &[CertifiedPathTransitionCandidateV1],
    source: PoseFingerprintV1,
    target: PoseFingerprintV1,
    checkpoint: impl FnMut() -> bool,
    oracle: impl FnMut(&CertifiedPathTransitionCandidateV1) -> Option<CertifiedPathTransitionEvidenceV1>,
) -> CertifiedPathGraphSearchResultV1 {
    search_certified_pose_graph_with_progress_v1(
        states,
        transitions,
        source,
        target,
        checkpoint,
        |_| {},
        oracle,
    )
}

/// Cancellable search with bounded monotonic progress observations. Progress
/// is detached from the eventual certificate and may be discarded at any time.
pub fn search_certified_pose_graph_with_progress_v1(
    states: &[PoseFingerprintV1],
    transitions: &[CertifiedPathTransitionCandidateV1],
    source: PoseFingerprintV1,
    target: PoseFingerprintV1,
    mut checkpoint: impl FnMut() -> bool,
    mut progress: impl FnMut(CertifiedPathGraphProgressV1),
    mut oracle: impl FnMut(
        &CertifiedPathTransitionCandidateV1,
    ) -> Option<CertifiedPathTransitionEvidenceV1>,
) -> CertifiedPathGraphSearchResultV1 {
    let publish_progress = |progress: &mut dyn FnMut(CertifiedPathGraphProgressV1),
                            explored_state_count,
                            evaluated_transition_count| {
        progress(CertifiedPathGraphProgressV1 {
            explored_state_count,
            evaluated_transition_count,
            state_limit: MAX_CERTIFIED_PATH_GRAPH_STATES_V1,
            transition_limit: MAX_CERTIFIED_PATH_GRAPH_TRANSITIONS_V1,
        });
    };
    publish_progress(&mut progress, 0, 0);
    if !checkpoint() {
        return indeterminate(CertifiedPathGraphIndeterminateReasonV1::Cancelled, 0, 0);
    }
    if states.is_empty()
        || states.len() > MAX_CERTIFIED_PATH_GRAPH_STATES_V1
        || transitions.len() > MAX_CERTIFIED_PATH_GRAPH_TRANSITIONS_V1
    {
        return indeterminate(CertifiedPathGraphIndeterminateReasonV1::ResourceLimit, 0, 0);
    }
    let mut canonical_states = states.to_vec();
    canonical_states.sort_unstable();
    canonical_states.dedup();
    if canonical_states.len() != states.len()
        || canonical_states.binary_search(&source).is_err()
        || canonical_states.binary_search(&target).is_err()
    {
        return indeterminate(
            CertifiedPathGraphIndeterminateReasonV1::NoCertifiedPath,
            0,
            0,
        );
    }
    if source == target {
        return CertifiedPathGraphSearchResultV1::Certified(CertifiedPoseGraphPathCertificateV1 {
            source,
            target,
            edges: Vec::new(),
            explored_state_count: 1,
            evaluated_transition_count: 0,
        });
    }

    let mut canonical_transitions = transitions.to_vec();
    canonical_transitions
        .sort_unstable_by_key(|edge| (edge.source, edge.target, edge.candidate_key));
    canonical_transitions.dedup();
    let mut queue = VecDeque::from([source]);
    let mut parents =
        BTreeMap::<PoseFingerprintV1, (PoseFingerprintV1, CertifiedPathTransitionEvidenceV1)>::new(
        );
    let mut visited = BTreeMap::from([(source, ())]);
    let mut evaluated = 0usize;
    let mut explored = 0usize;

    while let Some(current) = queue.pop_front() {
        if !checkpoint() {
            return indeterminate(
                CertifiedPathGraphIndeterminateReasonV1::Cancelled,
                explored,
                evaluated,
            );
        }
        explored += 1;
        publish_progress(&mut progress, explored, evaluated);
        for candidate in canonical_transitions
            .iter()
            .filter(|edge| edge.source == current)
        {
            if !checkpoint() {
                return indeterminate(
                    CertifiedPathGraphIndeterminateReasonV1::Cancelled,
                    explored,
                    evaluated,
                );
            }
            if canonical_states.binary_search(&candidate.target).is_err() {
                continue;
            }
            evaluated += 1;
            publish_progress(&mut progress, explored, evaluated);
            let Some(evidence) = oracle(candidate) else {
                continue;
            };
            if evidence.source != candidate.source || evidence.target != candidate.target {
                continue;
            }
            if visited.contains_key(&candidate.target) {
                continue;
            }
            visited.insert(candidate.target, ());
            parents.insert(candidate.target, (current, evidence));
            if candidate.target == target {
                let mut edges = Vec::new();
                let mut cursor = target;
                while cursor != source {
                    let Some((parent, edge)) = parents.get(&cursor).copied() else {
                        return indeterminate(
                            CertifiedPathGraphIndeterminateReasonV1::NoCertifiedPath,
                            explored,
                            evaluated,
                        );
                    };
                    edges.push(edge);
                    cursor = parent;
                }
                edges.reverse();
                if !checkpoint() {
                    return indeterminate(
                        CertifiedPathGraphIndeterminateReasonV1::Cancelled,
                        explored,
                        evaluated,
                    );
                }
                return CertifiedPathGraphSearchResultV1::Certified(
                    CertifiedPoseGraphPathCertificateV1 {
                        source,
                        target,
                        edges,
                        explored_state_count: explored,
                        evaluated_transition_count: evaluated,
                    },
                );
            }
            queue.push_back(candidate.target);
        }
    }
    indeterminate(
        CertifiedPathGraphIndeterminateReasonV1::NoCertifiedPath,
        explored,
        evaluated,
    )
}

fn indeterminate(
    reason: CertifiedPathGraphIndeterminateReasonV1,
    explored_state_count: usize,
    evaluated_transition_count: usize,
) -> CertifiedPathGraphSearchResultV1 {
    CertifiedPathGraphSearchResultV1::Indeterminate {
        reason,
        explored_state_count,
        evaluated_transition_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ori_domain::EdgeId;
    use ori_kinematics::{
        CanonicalHingeAngles, DyadicPoseGraphLimitsV1, HingeAngle,
        generate_bounded_dyadic_pose_graph_at_levels_v1, generate_bounded_dyadic_pose_graph_v1,
    };

    fn fingerprint(value: u8) -> PoseFingerprintV1 {
        [value; 32]
    }
    fn candidate(source: u8, target: u8, key: u8) -> CertifiedPathTransitionCandidateV1 {
        CertifiedPathTransitionCandidateV1 {
            source: fingerprint(source),
            target: fingerprint(target),
            candidate_key: fingerprint(key),
        }
    }
    fn certify(
        candidate: &CertifiedPathTransitionCandidateV1,
    ) -> CertifiedPathTransitionEvidenceV1 {
        CertifiedPathTransitionEvidenceV1::from_native_oracle(
            candidate.source,
            candidate.target,
            fingerprint(10),
            fingerprint(11),
            fingerprint(12),
        )
    }

    #[test]
    fn generated_two_hinge_grid_supports_certified_detours_and_fails_closed() {
        let mut edges = [EdgeId::new(), EdgeId::new()];
        edges.sort_unstable_by_key(EdgeId::canonical_bytes);
        let angles = |values: [f64; 2]| {
            CanonicalHingeAngles::new(
                edges
                    .into_iter()
                    .zip(values)
                    .map(|(edge, value)| HingeAngle::new(edge, value).unwrap())
                    .collect(),
            )
            .unwrap()
        };
        let generated = generate_bounded_dyadic_pose_graph_v1(
            &angles([0.0, 0.0]),
            &angles([90.0, 120.0]),
            DyadicPoseGraphLimitsV1::default(),
            || true,
        )
        .unwrap();
        let states = (0..generated.states().len())
            .map(|index| fingerprint(index as u8 + 1))
            .collect::<Vec<_>>();
        let candidates = generated
            .transitions()
            .iter()
            .map(|edge| CertifiedPathTransitionCandidateV1 {
                source: states[edge.source_state],
                target: states[edge.target_state],
                candidate_key: if edge.moving_hinge == edges[0] {
                    fingerprint(1)
                } else {
                    fingerprint(2)
                },
            })
            .collect::<Vec<_>>();
        let source = states[generated.source_state()];
        let target = states[generated.target_state()];
        assert!(matches!(
            search_certified_pose_graph_v1(&states, &candidates, source, target, |candidate| Some(
                certify(candidate)
            )),
            CertifiedPathGraphSearchResultV1::Certified(_)
        ));
        let blocked = candidates[0];
        assert!(matches!(
            search_certified_pose_graph_v1(&states, &candidates, source, target, |candidate| {
                (*candidate != blocked).then(|| certify(candidate))
            }),
            CertifiedPathGraphSearchResultV1::Certified(_)
        ));
        assert!(matches!(
            search_certified_pose_graph_v1(&states, &candidates, source, target, |candidate| {
                (candidate.source != source).then(|| certify(candidate))
            }),
            CertifiedPathGraphSearchResultV1::Indeterminate {
                reason: CertifiedPathGraphIndeterminateReasonV1::NoCertifiedPath,
                ..
            }
        ));
        assert!(matches!(
            search_certified_pose_graph_v1(&states, &candidates, source, target, |candidate| {
                let mut evidence = certify(candidate);
                evidence.source = fingerprint(250);
                Some(evidence)
            }),
            CertifiedPathGraphSearchResultV1::Indeterminate {
                reason: CertifiedPathGraphIndeterminateReasonV1::NoCertifiedPath,
                ..
            }
        ));
        let mut reordered = candidates.clone();
        reordered.reverse();
        let first =
            search_certified_pose_graph_v1(&states, &candidates, source, target, |candidate| {
                Some(certify(candidate))
            });
        let second =
            search_certified_pose_graph_v1(&states, &reordered, source, target, |candidate| {
                Some(certify(candidate))
            });
        let (
            CertifiedPathGraphSearchResultV1::Certified(first),
            CertifiedPathGraphSearchResultV1::Certified(second),
        ) = (first, second)
        else {
            panic!("both canonical searches certify")
        };
        assert_eq!(
            first.binding_fingerprint_v1(),
            second.binding_fingerprint_v1()
        );
    }

    #[test]
    fn canonical_bfs_uses_only_certified_edges_and_binds_all_edge_certificates() {
        let states = [
            fingerprint(3),
            fingerprint(1),
            fingerprint(4),
            fingerprint(2),
        ];
        let transitions = [
            candidate(2, 4, 3),
            candidate(1, 3, 2),
            candidate(3, 4, 2),
            candidate(1, 2, 1),
        ];
        let result = search_certified_pose_graph_v1(
            &states,
            &transitions,
            fingerprint(1),
            fingerprint(4),
            |candidate| Some(certify(candidate)),
        );
        let CertifiedPathGraphSearchResultV1::Certified(certificate) = result else {
            panic!("a certified route must be found");
        };
        assert_eq!(certificate.model_id(), CERTIFIED_PATH_GRAPH_MODEL_ID_V1);
        assert_eq!(certificate.version(), 1);
        assert!(!certificate.authorizes_project_mutation());
        assert_eq!(
            certificate
                .edges()
                .iter()
                .map(|edge| (edge.source(), edge.target()))
                .collect::<Vec<_>>(),
            vec![
                (fingerprint(1), fingerprint(2)),
                (fingerprint(2), fingerprint(4))
            ],
        );
        assert!(certificate.edges().iter().all(|edge| {
            edge.schedule_certificate() == fingerprint(10)
                && edge.collision_certificate() == fingerprint(11)
                && edge.closure_certificate() == fingerprint(12)
        }));
        let binding = certificate.binding_fingerprint_v1();
        assert_ne!(binding, [0; 32]);

        let mut reversed = transitions;
        reversed.reverse();
        let repeated = search_certified_pose_graph_v1(
            &states,
            &reversed,
            fingerprint(1),
            fingerprint(4),
            |candidate| Some(certify(candidate)),
        );
        assert_eq!(
            repeated,
            CertifiedPathGraphSearchResultV1::Certified(certificate),
            "candidate enumeration order must not change the certificate"
        );
        let CertifiedPathGraphSearchResultV1::Certified(repeated_certificate) = repeated else {
            unreachable!();
        };
        assert_eq!(repeated_certificate.binding_fingerprint_v1(), binding);
    }

    #[test]
    fn uncertified_and_misbound_edges_never_form_a_path() {
        let result = search_certified_pose_graph_v1(
            &[fingerprint(1), fingerprint(2), fingerprint(3)],
            &[candidate(1, 2, 1), candidate(2, 3, 1)],
            fingerprint(1),
            fingerprint(3),
            |candidate| {
                (candidate.source == fingerprint(2)).then(|| {
                    CertifiedPathTransitionEvidenceV1::from_native_oracle(
                        fingerprint(9),
                        candidate.target,
                        fingerprint(10),
                        fingerprint(11),
                        fingerprint(12),
                    )
                })
            },
        );
        assert!(matches!(
            result,
            CertifiedPathGraphSearchResultV1::Indeterminate {
                reason: CertifiedPathGraphIndeterminateReasonV1::NoCertifiedPath,
                ..
            }
        ));
    }

    #[test]
    fn hard_bounds_return_resource_indeterminate_never_impossible() {
        let states = vec![fingerprint(1); MAX_CERTIFIED_PATH_GRAPH_STATES_V1 + 1];
        assert!(matches!(
            search_certified_pose_graph_v1(&states, &[], fingerprint(1), fingerprint(2), |_| None,),
            CertifiedPathGraphSearchResultV1::Indeterminate {
                reason: CertifiedPathGraphIndeterminateReasonV1::ResourceLimit,
                ..
            }
        ));
        let transitions = vec![candidate(1, 2, 1); MAX_CERTIFIED_PATH_GRAPH_TRANSITIONS_V1 + 1];
        assert!(matches!(
            search_certified_pose_graph_v1(
                &[fingerprint(1), fingerprint(2)],
                &transitions,
                fingerprint(1),
                fingerprint(2),
                |_| None,
            ),
            CertifiedPathGraphSearchResultV1::Indeterminate {
                reason: CertifiedPathGraphIndeterminateReasonV1::ResourceLimit,
                ..
            }
        ));
    }

    #[test]
    fn cancellation_is_cooperative_and_never_publishes_a_partial_certificate() {
        let mut checkpoints = 0;
        let mut oracle_calls = 0;
        let result = search_certified_pose_graph_with_checkpoint_v1(
            &[fingerprint(1), fingerprint(2), fingerprint(3)],
            &[candidate(1, 2, 1), candidate(2, 3, 1)],
            fingerprint(1),
            fingerprint(3),
            || {
                checkpoints += 1;
                checkpoints < 5
            },
            |candidate| {
                oracle_calls += 1;
                Some(certify(candidate))
            },
        );
        assert!(matches!(
            result,
            CertifiedPathGraphSearchResultV1::Indeterminate {
                reason: CertifiedPathGraphIndeterminateReasonV1::Cancelled,
                ..
            }
        ));
        assert_eq!(oracle_calls, 1);
    }

    #[test]
    fn progress_is_monotonic_bounded_and_detached_from_the_certificate() {
        let mut observations = Vec::new();
        let result = search_certified_pose_graph_with_progress_v1(
            &[fingerprint(1), fingerprint(2), fingerprint(3)],
            &[candidate(1, 2, 1), candidate(2, 3, 1)],
            fingerprint(1),
            fingerprint(3),
            || true,
            |value| observations.push(value),
            |candidate| Some(certify(candidate)),
        );
        assert!(matches!(
            result,
            CertifiedPathGraphSearchResultV1::Certified(_)
        ));
        assert_eq!(
            observations.first().copied(),
            Some(CertifiedPathGraphProgressV1 {
                explored_state_count: 0,
                evaluated_transition_count: 0,
                state_limit: MAX_CERTIFIED_PATH_GRAPH_STATES_V1,
                transition_limit: MAX_CERTIFIED_PATH_GRAPH_TRANSITIONS_V1,
            })
        );
        assert!(observations.windows(2).all(|pair| {
            pair[0].explored_state_count <= pair[1].explored_state_count
                && pair[0].evaluated_transition_count <= pair[1].evaluated_transition_count
        }));
        assert!(observations.iter().all(|value| {
            value.explored_state_count <= value.state_limit
                && value.evaluated_transition_count <= value.transition_limit
                && value.state_limit == MAX_CERTIFIED_PATH_GRAPH_STATES_V1
                && value.transition_limit == MAX_CERTIFIED_PATH_GRAPH_TRANSITIONS_V1
        }));
    }

    #[test]
    fn quarter_level_unlocks_a_certified_obstacle_detour() {
        let mut edges = [EdgeId::new(), EdgeId::new()];
        edges.sort_unstable_by_key(EdgeId::canonical_bytes);
        let angles = |values: [f64; 2]| {
            CanonicalHingeAngles::new(
                edges
                    .into_iter()
                    .zip(values)
                    .map(|(edge, value)| HingeAngle::new(edge, value).unwrap())
                    .collect(),
            )
            .unwrap()
        };
        for (levels, expected_certified) in [(3, false), (5, true)] {
            let graph = generate_bounded_dyadic_pose_graph_at_levels_v1(
                &angles([0.0, 0.0]),
                &angles([90.0, 120.0]),
                levels,
                DyadicPoseGraphLimitsV1 {
                    max_states: 25,
                    max_transitions: 80,
                },
                || true,
            )
            .unwrap();
            let states = (0..graph.states().len())
                .map(|index| fingerprint(index as u8 + 1))
                .collect::<Vec<_>>();
            let candidates = graph
                .transitions()
                .iter()
                .map(|edge| CertifiedPathTransitionCandidateV1 {
                    source: states[edge.source_state],
                    target: states[edge.target_state],
                    candidate_key: fingerprint(200),
                })
                .collect::<Vec<_>>();
            let allowed = |fingerprint: PoseFingerprintV1| {
                let index = states.iter().position(|value| *value == fingerprint)?;
                let values = graph.states()[index].as_slice();
                let x = values[0].angle_degrees();
                let y = values[1].angle_degrees();
                Some((y == 0.0 && x <= 22.5) || x == 22.5 || (y == 120.0 && x >= 22.5))
            };
            let searched = search_certified_pose_graph_v1(
                &states,
                &candidates,
                states[graph.source_state()],
                states[graph.target_state()],
                |candidate| {
                    (allowed(candidate.source) == Some(true)
                        && allowed(candidate.target) == Some(true))
                    .then(|| certify(candidate))
                },
            );
            assert_eq!(
                matches!(searched, CertifiedPathGraphSearchResultV1::Certified(_)),
                expected_certified
            );
            if !expected_certified {
                assert!(matches!(
                    searched,
                    CertifiedPathGraphSearchResultV1::Indeterminate {
                        reason: CertifiedPathGraphIndeterminateReasonV1::NoCertifiedPath,
                        ..
                    }
                ));
            }
        }
    }
}
