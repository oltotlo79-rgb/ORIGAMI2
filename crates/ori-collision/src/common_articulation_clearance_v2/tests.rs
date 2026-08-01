//! Tests for the public V2 clearance prerequisite boundary.

use super::test_support::{
    miura_fixture_v2, miura_fixture_v2_with_profile, miura_fixture_v2_with_profile_and_namespace,
};
use super::validation::{
    canonical_pair_budget_v2, cross_block_pairs_equal_with_checkpoint_v2, dedup_sorted_pairs_v2,
    filter_pairs_not_local_v2, heap_sort_pairs_and_count_comparisons_v2,
    raw_pair_candidate_budget_v2, validate_submitted_pairs_v2,
};
use super::*;
use ori_domain::ProjectId;

#[path = "tests/parent_proof_tests.rs"]
mod parent_proof_tests;

fn distinct_pairs_v2(count: usize) -> Vec<CommonArticulationCrossBlockFacePairV2> {
    let shared_face = FaceId::new();
    (0..count)
        .map(|_| {
            CommonArticulationCrossBlockFacePairV2::new(shared_face, FaceId::new())
                .expect("distinct face pair")
        })
        .collect()
}

fn standard_sorted_pairs_v2(
    mut pairs: Vec<CommonArticulationCrossBlockFacePairV2>,
) -> Vec<CommonArticulationCrossBlockFacePairV2> {
    pairs.sort_unstable_by(|left, right| {
        left.first_v2()
            .canonical_bytes()
            .cmp(&right.first_v2().canonical_bytes())
            .then_with(|| {
                left.second_v2()
                    .canonical_bytes()
                    .cmp(&right.second_v2().canonical_bytes())
            })
    });
    pairs
}

#[test]
fn pair_normalization_rejects_identity_and_orders_faces() {
    let first = FaceId::new();
    let second = FaceId::new();
    assert_eq!(
        CommonArticulationCrossBlockFacePairV2::new(first, first),
        None
    );
    let pair = CommonArticulationCrossBlockFacePairV2::new(first, second).expect("pair");
    assert!(pair.first_v2().canonical_bytes() < pair.second_v2().canonical_bytes());
}

#[test]
fn canonical_profile_pair_arithmetic_has_the_n33_boundary() {
    assert_eq!(raw_pair_candidate_budget_v2(33), Ok(42_768));
    assert_eq!(canonical_pair_budget_v2(33), Ok(33_792));
    assert_eq!(raw_pair_candidate_budget_v2(32), Ok(40_176));
    assert_eq!(canonical_pair_budget_v2(32), Ok(31_744));
}

#[test]
fn submitted_pair_registry_must_be_exact_canonical_and_unique() {
    let mut faces = [FaceId::new(), FaceId::new(), FaceId::new(), FaceId::new()];
    faces.sort_unstable_by_key(FaceId::canonical_bytes);
    let first =
        CommonArticulationCrossBlockFacePairV2::new(faces[0], faces[1]).expect("first pair");
    let second =
        CommonArticulationCrossBlockFacePairV2::new(faces[0], faces[2]).expect("second pair");
    let expected = [first, second];
    assert_eq!(
        validate_submitted_pairs_v2(&[second, first], &expected, &mut || Ok(())),
        Err(CommonArticulationClearanceErrorV2::NonCanonicalCrossBlockPairRegistry),
    );
    assert_eq!(
        validate_submitted_pairs_v2(&[first, first], &expected, &mut || Ok(())),
        Err(CommonArticulationClearanceErrorV2::DuplicateCrossBlockPair),
    );
    assert_eq!(
        validate_submitted_pairs_v2(&[first], &expected, &mut || Ok(())),
        Err(
            CommonArticulationClearanceErrorV2::CrossBlockPairCoverageMismatch {
                expected: 2,
                actual: 1,
            },
        ),
    );
}

#[test]
fn pollable_heapsort_matches_standard_sort_for_edge_cases() {
    let pairs = distinct_pairs_v2(7);
    let cases = vec![
        Vec::new(),
        vec![pairs[0]],
        pairs.iter().rev().copied().collect(),
        vec![
            pairs[5], pairs[1], pairs[6], pairs[2], pairs[4], pairs[0], pairs[3],
        ],
        vec![pairs[4], pairs[1], pairs[4], pairs[6], pairs[1], pairs[0]],
    ];

    for mut actual in cases {
        let expected = standard_sorted_pairs_v2(actual.clone());
        heap_sort_pairs_and_count_comparisons_v2(&mut actual, &mut || Ok(()))
            .expect("pollable heap sort");
        assert_eq!(actual, expected);
    }
}

#[test]
fn pollable_heapsort_honors_cancel_and_deadline_mid_operation() {
    let mut cancelled = distinct_pairs_v2(8);
    cancelled.reverse();
    assert_eq!(
        heap_sort_pairs_and_count_comparisons_v2(&mut cancelled, &mut || {
            Err(CommonArticulationClearanceStopV2::Cancelled)
        }),
        Err(CommonArticulationClearanceErrorV2::Cancelled),
    );

    let mut deadline = distinct_pairs_v2(8);
    deadline.reverse();
    let mut checkpoints = 0usize;
    assert_eq!(
        heap_sort_pairs_and_count_comparisons_v2(&mut deadline, &mut || {
            checkpoints += 1;
            if checkpoints >= 3 {
                Err(CommonArticulationClearanceStopV2::DeadlineExceeded)
            } else {
                Ok(())
            }
        }),
        Err(CommonArticulationClearanceErrorV2::DeadlineExceeded),
    );
    assert_eq!(checkpoints, 3);
}

#[test]
fn local_filter_compacts_exact_membership_and_deduplicates_pollably() {
    let pairs = distinct_pairs_v2(6);
    let mut local_pairs = vec![pairs[4], pairs[1], pairs[4], pairs[1]];
    heap_sort_pairs_and_count_comparisons_v2(&mut local_pairs, &mut || Ok(()))
        .expect("local heap sort");
    dedup_sorted_pairs_v2(&mut local_pairs, &mut || Ok(())).expect("local dedup");
    assert_eq!(
        local_pairs,
        standard_sorted_pairs_v2(vec![pairs[1], pairs[4]])
    );

    let mut candidates = vec![pairs[4], pairs[0], pairs[1], pairs[5], pairs[4], pairs[2]];
    filter_pairs_not_local_v2(&mut candidates, &local_pairs, &mut || Ok(()))
        .expect("explicit write-index compaction");
    assert_eq!(candidates, vec![pairs[0], pairs[5], pairs[2]]);

    let mut duplicate_candidates = vec![pairs[3], pairs[0], pairs[3], pairs[2], pairs[0]];
    heap_sort_pairs_and_count_comparisons_v2(&mut duplicate_candidates, &mut || Ok(()))
        .expect("registry heap sort");
    dedup_sorted_pairs_v2(&mut duplicate_candidates, &mut || Ok(())).expect("registry dedup");
    assert_eq!(
        duplicate_candidates,
        standard_sorted_pairs_v2(vec![pairs[0], pairs[2], pairs[3]]),
    );
}

#[test]
fn pair_registry_comparison_prioritizes_stops_around_a_mismatch() {
    let pairs = distinct_pairs_v2(2);
    let retained = [pairs[0]];
    let candidate = [pairs[1]];
    for (stop_at, expected) in [
        (2, CommonArticulationClearanceErrorV2::Cancelled),
        (3, CommonArticulationClearanceErrorV2::DeadlineExceeded),
    ] {
        let mut polls = 0usize;
        assert_eq!(
            cross_block_pairs_equal_with_checkpoint_v2(&retained, &candidate, &mut || {
                polls += 1;
                if polls == stop_at {
                    Err(match expected {
                        CommonArticulationClearanceErrorV2::Cancelled => {
                            CommonArticulationClearanceStopV2::Cancelled
                        }
                        CommonArticulationClearanceErrorV2::DeadlineExceeded => {
                            CommonArticulationClearanceStopV2::DeadlineExceeded
                        }
                        _ => unreachable!("test only supplies stop errors"),
                    })
                } else {
                    Ok(())
                }
            }),
            Err(expected),
            "stop at poll {stop_at} must outrank the pair mismatch",
        );
    }
}

#[test]
fn raw_heapsort_comparisons_fit_the_n33_profile_envelope() {
    let profile =
        CommonArticulationResourceProfileV2::exact_canonical_miura_3x3_v2(33).expect("N33 profile");
    let resources = profile.actual_v2();
    let mut raw_pairs = distinct_pairs_v2(resources.raw_cross_block_pair_candidates_v2());
    raw_pairs.reverse();
    let comparisons = heap_sort_pairs_and_count_comparisons_v2(&mut raw_pairs, &mut || Ok(()))
        .expect("raw heap sort");
    let envelope = resources
        .raw_cross_block_pair_candidates_v2()
        .checked_mul(resources.raw_sort_comparisons_per_item_v2())
        .expect("N33 profile envelope fits usize");
    assert!(comparisons <= envelope, "{comparisons} <= {envelope}");
}

#[test]
fn n33_issues_an_unpromoted_profile_bound_prerequisite() {
    let fixture = miura_fixture_v2();
    let outcome = issue_common_articulation_clearance_prerequisite_v2(fixture.input())
        .expect("N33 unpromoted prerequisite");
    assert!(!outcome.is_certified_v2());
    assert!(!outcome.authorizes_continuous_motion());
    assert!(!outcome.authorizes_collision_clearance());
    assert!(!outcome.authorizes_project_mutation());
    assert!(!outcome.authorizes_apply());
    assert!(!outcome.authorizes_viewer());
    assert!(!outcome.authorizes_layer_transport());

    let prerequisite = outcome.as_unpromoted_v2();
    assert_eq!(
            prerequisite.unpromoted_reason_v2(),
            CommonArticulationClearanceUnpromotedReasonV2::WholeParentPositiveThicknessEvidenceUnavailable
        );
    assert_eq!(prerequisite.actual_block_count_v2(), 33);
    assert_eq!(prerequisite.face_count_v2(), 265);
    assert_eq!(prerequisite.hinge_count_v2(), 396);
    assert_eq!(prerequisite.cross_block_pairs_v2().len(), 33_792);
    assert_eq!(prerequisite.logical_work_v2(), 10_015_062);
    assert_eq!(prerequisite.storage_bytes_upper_bound_v2(), 2_497_536);
    assert!(!prerequisite.cross_block_open_interval_clearance_proven_v2());
    assert!(!prerequisite.authorizes_continuous_motion());
    assert!(!prerequisite.authorizes_collision_clearance());
    assert!(!prerequisite.authorizes_project_mutation());
    assert!(!prerequisite.authorizes_apply());
    assert!(!prerequisite.authorizes_viewer());
    assert!(!prerequisite.authorizes_layer_transport());
    prerequisite
        .revalidate_v2(fixture.revalidation_input())
        .expect("same exact live V2 inputs");
}

#[test]
fn n34_with_configured_n40_is_exactly_bound_and_fails_closed_across_caps() {
    let fixture = miura_fixture_v2_with_profile(40, 34);
    let exact_n34 = CommonArticulationResourceProfileV2::exact_canonical_miura_3x3_v2(34)
        .expect("exact N34 profile");
    assert_eq!(fixture.profile.configured_max_blocks_v2(), 40);
    assert_eq!(fixture.profile.actual_block_count_v2(), 34);
    assert_eq!(
        fixture.profile.actual_v2().canonical_cross_block_pairs_v2(),
        35_904
    );
    assert_ne!(
        fixture.profile.binding_fingerprint_v2(),
        exact_n34.binding_fingerprint_v2(),
        "configured N is part of the live profile binding"
    );

    let outcome = issue_common_articulation_clearance_prerequisite_v2(fixture.input())
        .expect("configured-N40, actual-N34 prerequisite");
    let prerequisite = outcome.as_unpromoted_v2();
    assert_eq!(prerequisite.actual_block_count_v2(), 34);
    assert_eq!(prerequisite.cross_block_pairs_v2().len(), 35_904);
    assert_eq!(
        prerequisite.profile_binding_fingerprint_v2(),
        fixture.profile.binding_fingerprint_v2()
    );
    assert!(!outcome.is_certified_v2());
    assert!(!outcome.authorizes_continuous_motion());
    assert!(!outcome.authorizes_collision_clearance());
    assert!(!outcome.authorizes_project_mutation());
    assert!(!outcome.authorizes_apply());
    assert!(!outcome.authorizes_viewer());
    assert!(!outcome.authorizes_layer_transport());
    assert!(!prerequisite.cross_block_open_interval_clearance_proven_v2());
    assert!(!prerequisite.authorizes_continuous_motion());
    assert!(!prerequisite.authorizes_collision_clearance());
    assert!(!prerequisite.authorizes_project_mutation());
    assert!(!prerequisite.authorizes_apply());
    assert!(!prerequisite.authorizes_viewer());
    assert!(!prerequisite.authorizes_layer_transport());
    prerequisite
        .revalidate_v2(fixture.revalidation_input())
        .expect("same configured and actual N revalidate");

    let cross_cap = CommonArticulationResourceProfileV2::for_canonical_miura_3x3_v2(41, 34)
        .expect("different configured cap, same actual N");
    assert_ne!(
        cross_cap.binding_fingerprint_v2(),
        fixture.profile.binding_fingerprint_v2()
    );
    assert_eq!(
        prerequisite
            .revalidate_v2(CommonArticulationClearanceRevalidationInputV2 {
                profile: &cross_cap,
                ..fixture.revalidation_input()
            })
            .expect_err("cross-cap replay must fail closed"),
        CommonArticulationClearanceErrorV2::ResourceLimit,
    );

    let one_short = miura_fixture_v2_with_profile(40, 33);
    assert_eq!(
        prerequisite
            .revalidate_v2(one_short.revalidation_input())
            .expect_err("N34 prerequisite cannot be replayed with actual N33 inputs"),
        CommonArticulationClearanceErrorV2::PrerequisiteBindingMismatch,
    );
}

#[test]
fn independently_rebuilt_n34_configured_n40_replays_bit_identically() {
    let namespace = ProjectId::new();
    let first = miura_fixture_v2_with_profile_and_namespace(40, 34, namespace);
    let second = miura_fixture_v2_with_profile_and_namespace(40, 34, namespace);
    assert_eq!(first.pairs, second.pairs, "canonical registry replay");
    assert_eq!(
        first.profile.binding_fingerprint_v2(),
        second.profile.binding_fingerprint_v2(),
        "profile binding replay"
    );
    assert_eq!(
        first.decomposition.binding_fingerprint_v2(),
        second.decomposition.binding_fingerprint_v2(),
        "decomposition binding replay"
    );
    assert_eq!(
        first.common_pose.binding_fingerprint_v2(),
        second.common_pose.binding_fingerprint_v2(),
        "pose binding replay"
    );

    let first_outcome = issue_common_articulation_clearance_prerequisite_v2(first.input())
        .expect("first N34/configured-N40 issuance");
    let second_outcome = issue_common_articulation_clearance_prerequisite_v2(second.input())
        .expect("independent N34/configured-N40 replay issuance");
    let first_prerequisite = first_outcome.as_unpromoted_v2();
    let second_prerequisite = second_outcome.as_unpromoted_v2();
    assert_eq!(
        first_prerequisite.cross_block_pairs_v2(),
        second_prerequisite.cross_block_pairs_v2()
    );
    assert_eq!(
        first_prerequisite.binding_fingerprint_v2(),
        second_prerequisite.binding_fingerprint_v2(),
        "full clearance binding replay"
    );
    assert_eq!(
        first_prerequisite.logical_work_v2(),
        second_prerequisite.logical_work_v2()
    );
    assert_eq!(
        first_prerequisite.storage_bytes_upper_bound_v2(),
        second_prerequisite.storage_bytes_upper_bound_v2()
    );
    first_prerequisite
        .revalidate_v2(second.revalidation_input())
        .expect("independently rebuilt, same-identity replay");
}

#[test]
fn missing_single_source_substitutions_are_rejected_individually() {
    let fixture = miura_fixture_v2();
    let foreign = miura_fixture_v2();
    let outcome = issue_common_articulation_clearance_prerequisite_v2(fixture.input())
        .expect("base prerequisite");
    let prerequisite = outcome.as_unpromoted_v2();

    // Profile-only replacement is already covered by the cross-cap cases in
    // `n32_one_short_and_cross_cap_profiles_fail_closed` and the N34 test.
    assert_eq!(
        prerequisite
            .revalidate_v2(CommonArticulationClearanceRevalidationInputV2 {
                audit: &foreign.audit,
                ..fixture.revalidation_input()
            })
            .expect_err("audit-only substitution"),
        CommonArticulationClearanceErrorV2::AuditBindingMismatch,
    );
    assert_eq!(
        prerequisite
            .revalidate_v2(CommonArticulationClearanceRevalidationInputV2 {
                decomposition: &foreign.decomposition,
                ..fixture.revalidation_input()
            })
            .expect_err("decomposition-only substitution"),
        CommonArticulationClearanceErrorV2::ResourceLimit,
    );
    assert_eq!(
        prerequisite
            .revalidate_v2(CommonArticulationClearanceRevalidationInputV2 {
                pose: &foreign.pose,
                ..fixture.revalidation_input()
            })
            .expect_err("pose-only substitution"),
        CommonArticulationClearanceErrorV2::CommonPose(
            CommonArticulationPoseErrorV2::PoseIssuerMismatch
        ),
    );
    assert_eq!(
        prerequisite
            .revalidate_v2(CommonArticulationClearanceRevalidationInputV2 {
                paper_thickness_mm: 0.2,
                ..fixture.revalidation_input()
            })
            .expect_err("thickness-only substitution"),
        CommonArticulationClearanceErrorV2::CommonPose(
            CommonArticulationPoseErrorV2::IssuerMismatch
        ),
    );

    let mut replaced_registry = fixture.pairs.clone();
    replaced_registry.swap(0, 1);
    assert_eq!(
        issue_common_articulation_clearance_prerequisite_v2(CommonArticulationClearanceInputV2 {
            submitted_cross_block_pairs: &replaced_registry,
            ..fixture.input()
        },)
        .expect_err("registry-only substitution"),
        CommonArticulationClearanceErrorV2::NonCanonicalCrossBlockPairRegistry,
    );
}

#[test]
fn n32_one_short_and_cross_cap_profiles_fail_closed() {
    let fixture = miura_fixture_v2();
    let n32 = CommonArticulationResourceProfileV2::exact_canonical_miura_3x3_v2(32)
        .expect("profile arithmetic remains constructible at N32");
    assert_eq!(
        issue_common_articulation_clearance_prerequisite_v2(CommonArticulationClearanceInputV2 {
            profile: &n32,
            ..fixture.input()
        },)
        .expect_err("N32 must not enter the V2 authority family"),
        CommonArticulationClearanceErrorV2::ResourceLimit,
    );

    let one_short = CommonArticulationResourceProfileV2::for_canonical_miura_3x3_v2(33, 32)
        .expect("configured-N33, actual-N32 profile");
    assert_eq!(
        issue_common_articulation_clearance_prerequisite_v2(CommonArticulationClearanceInputV2 {
            profile: &one_short,
            ..fixture.input()
        },)
        .expect_err("one-short actual block count"),
        CommonArticulationClearanceErrorV2::ResourceLimit,
    );

    let outcome = issue_common_articulation_clearance_prerequisite_v2(fixture.input())
        .expect("N33 prerequisite");
    let cross_cap = CommonArticulationResourceProfileV2::for_canonical_miura_3x3_v2(34, 33)
        .expect("N34 configured N33 actual profile");
    assert_eq!(
        outcome
            .as_unpromoted_v2()
            .revalidate_v2(CommonArticulationClearanceRevalidationInputV2 {
                profile: &cross_cap,
                ..fixture.revalidation_input()
            })
            .expect_err("exact-N33 issuance cannot be replayed under N34 configured cap"),
        CommonArticulationClearanceErrorV2::ResourceLimit,
    );
}

#[test]
fn foreign_live_sources_and_stop_requests_fail_closed() {
    let fixture = miura_fixture_v2();
    let outcome = issue_common_articulation_clearance_prerequisite_v2(fixture.input())
        .expect("N33 prerequisite");
    let foreign = miura_fixture_v2();
    assert_eq!(
        outcome
            .as_unpromoted_v2()
            .revalidate_v2(foreign.revalidation_input())
            .expect_err("foreign geometry/audit/pose/decomposition"),
        CommonArticulationClearanceErrorV2::PrerequisiteBindingMismatch,
    );
    assert_eq!(
        issue_common_articulation_clearance_prerequisite_with_checkpoint_v2(
            fixture.input(),
            || Err(CommonArticulationClearanceStopV2::Cancelled),
        )
        .expect_err("start cancellation"),
        CommonArticulationClearanceErrorV2::Cancelled,
    );
    assert_eq!(
        issue_common_articulation_clearance_prerequisite_with_checkpoint_v2(
            fixture.input(),
            || Err(CommonArticulationClearanceStopV2::DeadlineExceeded),
        )
        .expect_err("start deadline"),
        CommonArticulationClearanceErrorV2::DeadlineExceeded,
    );
    let mut checkpoints = 0usize;
    assert_eq!(
        issue_common_articulation_clearance_prerequisite_with_checkpoint_v2(
            fixture.input(),
            || {
                checkpoints += 1;
                if checkpoints >= 2 {
                    Err(CommonArticulationClearanceStopV2::DeadlineExceeded)
                } else {
                    Ok(())
                }
            },
        )
        .expect_err("batched deadline"),
        CommonArticulationClearanceErrorV2::DeadlineExceeded,
    );
    assert_eq!(
        outcome
            .as_unpromoted_v2()
            .revalidate_with_checkpoint_v2(fixture.revalidation_input(), || {
                Err(CommonArticulationClearanceStopV2::Cancelled)
            })
            .expect_err("revalidation cancellation"),
        CommonArticulationClearanceErrorV2::Cancelled,
    );
}
