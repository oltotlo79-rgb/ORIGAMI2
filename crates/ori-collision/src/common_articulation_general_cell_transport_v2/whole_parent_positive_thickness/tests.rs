use std::sync::Arc;

use ori_foldability::{
    DEFAULT_MAX_COMPACT_LAYER_ORDER_PEAK_BYTES_V2, GlobalFlatFoldabilityLimits,
    GlobalFlatLayerOrderCompactPairAssignmentInputV2,
    GlobalFlatLayerOrderCompactPairAssignmentLimitsV2, GlobalFlatLayerOrderRevalidationLimitsV2,
    global_flat_layer_order_compact_pair_assignment_sha256_v2,
    issue_global_flat_layer_order_from_compact_pair_assignment_v2,
};

use super::super::super::graph_positive_thickness::{
    CommonArticulationPositiveThicknessParentGraphAdmissionLimitsV2,
    admit_common_articulation_positive_thickness_parent_graph_v2,
};
use super::super::test_support::{TransportFixtureV2, golden_n33_transport_fixture_v2};
use super::*;

use super::super::n33_compact_pair_assignment_fixture_v2::{
    n33_compact_pair_assignment_sha256_v2, n33_compact_pair_assignment_v2,
};

fn issue_n33_compact_source_v2(
    fixture: &TransportFixtureV2,
) -> (
    ori_foldability::GlobalFlatLayerOrderCompactPairAssignmentAuthorityV2,
    GlobalFlatLayerOrderRevalidationLimitsV2,
) {
    let live = fixture.n33_live_global_input_v2();
    let (variable_count, variable_registry_sha256, direction_bits_le) =
        n33_compact_pair_assignment_v2();
    assert_eq!(
        global_flat_layer_order_compact_pair_assignment_sha256_v2(
            variable_count,
            variable_registry_sha256,
            &direction_bits_le,
        )
        .expect("checked N=33 assignment digest"),
        n33_compact_pair_assignment_sha256_v2()
    );
    let analysis = GlobalFlatFoldabilityLimits {
        max_search_nodes: 0,
        ..GlobalFlatFoldabilityLimits::default()
    };
    let compact = issue_global_flat_layer_order_from_compact_pair_assignment_v2(
        GlobalFlatLayerOrderCompactPairAssignmentInputV2 {
            source: live.input(fixture),
            variable_count,
            variable_registry_sha256,
            direction_bits_le: &direction_bits_le,
        },
        GlobalFlatLayerOrderCompactPairAssignmentLimitsV2 {
            analysis,
            ..GlobalFlatLayerOrderCompactPairAssignmentLimitsV2::default()
        },
    )
    .expect("checked N=33 compact assignment replays without search");
    let source_limits = GlobalFlatLayerOrderRevalidationLimitsV2 {
        analysis,
        max_source_retained_bytes: compact.resources_v2().layer_order_retained_bytes,
        max_peak_bytes: DEFAULT_MAX_COMPACT_LAYER_ORDER_PEAK_BYTES_V2,
    };
    (compact, source_limits)
}

fn foreign_live_input_v2(
    fixture: &TransportFixtureV2,
) -> CommonArticulationGeneralCellTransportRevalidationInputV2<'_> {
    let input = fixture.input();
    CommonArticulationGeneralCellTransportRevalidationInputV2 {
        geometry: input.geometry,
        audit: input.audit,
        pose: input.pose,
        decomposition: input.decomposition,
        common_pose: input.common_pose,
        parent_fixed_face: input.parent_fixed_face,
        parent_schedule: input.parent_schedule,
        profile: input.profile,
        paper_thickness_mm: input.paper_thickness_mm,
        closure_tolerance: input.closure_tolerance,
        block_closure_set: input.block_closure_set,
        whole_parent_closure: input.whole_parent_closure,
        whole_parent_closure_limits: input.whole_parent_closure_limits,
        clearance: input.clearance,
        source_authority: input.source_authority,
        limits: input.limits,
    }
}

#[test]
fn pair_evidence_unavailable_maps_only_to_unpromoted_v2() {
    assert_eq!(
        classify_stationary_graph_failure_v2(
            CommonArticulationAdmittedPositiveThicknessGraphProofErrorV2::Geometry(
                PositiveThicknessGraphProofErrorV1::PairEvidenceUnavailable,
            ),
        ),
        Ok(
            CommonArticulationProfileBoundWholeParentPositiveThicknessUnpromotedReasonV2::StationaryPairEvidenceUnavailable,
        )
    );
    assert_eq!(
        classify_stationary_graph_failure_v2(
            CommonArticulationAdmittedPositiveThicknessGraphProofErrorV2::Geometry(
                PositiveThicknessGraphProofErrorV1::ResourceLimit,
            ),
        ),
        Err(
            CommonArticulationProfileBoundWholeParentPositiveThicknessErrorV2::PositiveThickness(
                CommonArticulationAdmittedPositiveThicknessGraphProofErrorV2::Geometry(
                    PositiveThicknessGraphProofErrorV1::ResourceLimit,
                ),
            ),
        )
    );
}

#[test]
fn genuine_n33_source_proves_only_stationary_whole_parent_thickness_v2() {
    let fixture = golden_n33_transport_fixture_v2();
    let (compact, source_limits) = issue_n33_compact_source_v2(&fixture);
    let live_global = fixture.n33_live_global_input_v2();

    let issue_source = compact
        .revalidate_live_source_v2(live_global.input(&fixture), source_limits)
        .expect("exact N=33 source earns a fresh live authority");
    let transport = match issue_common_articulation_general_cell_transport_prerequisite_v2(
        fixture
            .input_from_n33_live_authority_v2(issue_source)
            .expect("exact N=33 transport envelope"),
    )
    .expect("exact N=33 source reaches the ordinary transport boundary")
    {
        CommonArticulationGeneralCellTransportOutcomeV2::Unpromoted(prerequisite) => *prerequisite,
    };

    let maximum = fixture.clearance_fixture.profile.maximum_v2();
    let admission = Arc::new(
        admit_common_articulation_positive_thickness_parent_graph_v2(
            &fixture.clearance_fixture.geometry,
            CommonArticulationPositiveThicknessParentGraphAdmissionLimitsV2 {
                max_faces: maximum.face_count_v2(),
                max_hinges: maximum.hinge_count_v2(),
                max_face_pair_tests: maximum.unordered_face_pair_count_v2(),
                ..CommonArticulationPositiveThicknessParentGraphAdmissionLimitsV2::default()
            },
        )
        .expect("canonical N=33 parent graph has an exact planar admission"),
    );
    let proof_source = compact
        .revalidate_live_source_v2(live_global.input(&fixture), source_limits)
        .expect("proof uses the same live N=33 source");
    let outcome = prove_common_articulation_profile_bound_whole_parent_positive_thickness_v2(
        CommonArticulationProfileBoundWholeParentPositiveThicknessInputV2 {
            transport_prerequisite: transport,
            live: fixture
                .revalidation_input_from_n33_live_authority_v2(proof_source)
                .expect("same exact N=33 replay envelope"),
            parent_graph_admission: admission,
        },
    )
    .expect("stationary N=33 proof boundary is well formed");
    let certificate = outcome
        .as_proven_v2()
        .expect("canonical N=33 rest pose has strict evidence for every non-contact pair");
    assert_eq!(
        certificate.actual_block_count_v2(),
        fixture.clearance_fixture.profile.actual_block_count_v2()
    );
    assert_eq!(
        certificate.analyzed_unordered_face_pairs_v2(),
        maximum.unordered_face_pair_count_v2()
    );
    assert!(certificate.stationary_whole_parent_positive_thickness_proven_v2());
    assert!(!certificate.authorizes_continuous_motion());
    assert!(!certificate.authorizes_collision_clearance());
    assert!(!certificate.authorizes_layer_transport());
    assert!(!certificate.authorizes_project_mutation());
    assert!(!certificate.authorizes_apply());
    assert!(!certificate.authorizes_viewer());
    assert!(!outcome.authorizes_continuous_motion());
    assert!(!outcome.authorizes_collision_clearance());
    assert!(!outcome.authorizes_layer_transport());
    assert!(!outcome.authorizes_project_mutation());
    assert!(!outcome.authorizes_apply());
    assert!(!outcome.authorizes_viewer());

    let replay_source = compact
        .revalidate_live_source_v2(live_global.input(&fixture), source_limits)
        .expect("replay uses a fresh authority for the same N=33 source");
    certificate
        .revalidate_v2(
            CommonArticulationProfileBoundWholeParentPositiveThicknessRevalidationInputV2 {
                live: fixture
                    .revalidation_input_from_n33_live_authority_v2(replay_source)
                    .expect("same exact N=33 replay envelope"),
            },
        )
        .expect("the unchanged live source/profile/namespace/limits tuple replays");

    let mut malformed = foreign_live_input_v2(&fixture);
    malformed.paper_thickness_mm = f64::NAN;
    assert_eq!(
        certificate
            .revalidate_v2(
                CommonArticulationProfileBoundWholeParentPositiveThicknessRevalidationInputV2 {
                    live: malformed,
                },
            )
            .expect_err("malformed stationary input fails closed"),
        CommonArticulationProfileBoundWholeParentPositiveThicknessErrorV2::Transport(
            CommonArticulationGeneralCellTransportErrorV2::InvalidInput,
        )
    );

    let mut one_short = foreign_live_input_v2(&fixture);
    one_short.limits.max_blocks -= 1;
    assert_eq!(
        certificate
            .revalidate_v2(
                CommonArticulationProfileBoundWholeParentPositiveThicknessRevalidationInputV2 {
                    live: one_short,
                },
            )
            .expect_err("one-short profile envelope fails before source replay"),
        CommonArticulationProfileBoundWholeParentPositiveThicknessErrorV2::Transport(
            CommonArticulationGeneralCellTransportErrorV2::ResourceLimit,
        )
    );

    for stop in [
        CommonArticulationProfileBoundWholeParentPositiveThicknessStopV2::Cancelled,
        CommonArticulationProfileBoundWholeParentPositiveThicknessStopV2::DeadlineExceeded,
    ] {
        assert_eq!(
            certificate
                .revalidate_with_checkpoint_v2(
                    CommonArticulationProfileBoundWholeParentPositiveThicknessRevalidationInputV2 {
                        live: foreign_live_input_v2(&fixture),
                    },
                    || Err(stop),
                )
                .expect_err("a cooperative stop hides every proof result"),
            match stop {
                CommonArticulationProfileBoundWholeParentPositiveThicknessStopV2::Cancelled => {
                    CommonArticulationProfileBoundWholeParentPositiveThicknessErrorV2::Cancelled
                }
                CommonArticulationProfileBoundWholeParentPositiveThicknessStopV2::DeadlineExceeded => {
                    CommonArticulationProfileBoundWholeParentPositiveThicknessErrorV2::DeadlineExceeded
                }
            }
        );
    }
}
