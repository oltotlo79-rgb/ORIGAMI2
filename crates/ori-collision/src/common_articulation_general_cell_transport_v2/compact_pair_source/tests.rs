use std::mem::size_of;

use ori_domain::ProjectId;
use ori_foldability::{
    DEFAULT_MAX_COMPACT_LAYER_ORDER_PEAK_BYTES_V2, GlobalFlatFoldabilityInput,
    GlobalFlatFoldabilityLimits, GlobalFlatLayerOrderCompactPairAssignmentInputV2,
    GlobalFlatLayerOrderCompactPairAssignmentLimitsV2, GlobalFlatLayerOrderRevalidationLimitsV2,
    global_flat_layer_order_compact_pair_assignment_sha256_v2,
    issue_global_flat_layer_order_from_compact_pair_assignment_v2,
};

use super::super::test_support::{
    TransportFixtureV2, exact_transport_limits_for_live_n33_source_v2,
    golden_n33_transport_fixture_v2,
};
use super::*;

use super::super::n33_compact_pair_assignment_fixture_v2::{
    n33_compact_pair_assignment_sha256_v2, n33_compact_pair_assignment_v2,
};

#[test]
fn n33_compact_assignment_receipt_is_pinned_v2() {
    let (variable_count, registry_digest, direction_bits) = n33_compact_pair_assignment_v2();
    assert_eq!(
        global_flat_layer_order_compact_pair_assignment_sha256_v2(
            variable_count,
            registry_digest,
            &direction_bits,
        )
        .expect("checked N=33 assignment digest"),
        n33_compact_pair_assignment_sha256_v2()
    );
}

fn transport_live_input_v2<'a>(
    fixture: &'a TransportFixtureV2,
    transport_limits: CommonArticulationGeneralCellTransportLimitsV2,
) -> CommonArticulationCompactPairGeneralCellTransportLiveInputV2<'a> {
    CommonArticulationCompactPairGeneralCellTransportLiveInputV2 {
        geometry: &fixture.clearance_fixture.geometry,
        audit: &fixture.clearance_fixture.audit,
        pose: &fixture.clearance_fixture.pose,
        decomposition: &fixture.clearance_fixture.decomposition,
        common_pose: &fixture.clearance_fixture.common_pose,
        parent_fixed_face: fixture.clearance_fixture.parent_fixed_face,
        parent_schedule: &fixture.clearance_fixture.parent_schedule,
        profile: &fixture.clearance_fixture.profile,
        paper_thickness_mm: 0.1,
        closure_tolerance: fixture.clearance_fixture.closure_tolerance,
        block_closure_set: &fixture.clearance_fixture.block_closure_set,
        whole_parent_closure: &fixture.clearance_fixture.whole_parent_closure,
        whole_parent_closure_limits: fixture.clearance_fixture.whole_parent_closure_limits,
        clearance: &fixture.clearance,
        transport_limits,
    }
}

fn bridge_input_v2<'a>(
    fixture: &'a TransportFixtureV2,
    compact_authority: &'a GlobalFlatLayerOrderCompactPairAssignmentAuthorityV2,
    direction_bits_le: &'a [u8],
    foldability_source: GlobalFlatFoldabilityInput<'a>,
    source_revalidation_limits: GlobalFlatLayerOrderRevalidationLimitsV2,
    transport_limits: CommonArticulationGeneralCellTransportLimitsV2,
    limits: CommonArticulationCompactPairGeneralCellTransportLimitsV2,
) -> CommonArticulationCompactPairGeneralCellTransportInputV2<'a> {
    CommonArticulationCompactPairGeneralCellTransportInputV2 {
        compact_authority,
        direction_bits_le,
        foldability_source,
        source_revalidation_limits,
        live: transport_live_input_v2(fixture, transport_limits),
        limits,
    }
}

fn exact_bridge_limits_v2(
    compact: &GlobalFlatLayerOrderCompactPairAssignmentAuthorityV2,
    assignment_bytes: usize,
    source_revalidation_limits: GlobalFlatLayerOrderRevalidationLimitsV2,
    transport_limits: CommonArticulationGeneralCellTransportLimitsV2,
) -> CommonArticulationCompactPairGeneralCellTransportLimitsV2 {
    let authority_shell_bytes = size_of::<GlobalFlatLayerOrderCompactPairAssignmentAuthorityV2>()
        - size_of::<LayerOrderSnapshot>();
    let compact_source_retained_bytes = compact
        .resources_v2()
        .layer_order_retained_bytes
        .checked_add(authority_shell_bytes)
        .expect("N=33 compact source retained charge");
    let logical_work = checked_compact_work_v2(compact.work_counts_v2())
        .expect("N=33 compact work")
        .checked_add(assignment_bytes)
        .and_then(|value| value.checked_add(transport_limits.max_logical_work))
        .and_then(|value| value.checked_add(COMPACT_PAIR_TRANSPORT_BASE_WORK_V2))
        .expect("N=33 bridge logical work");
    let outer_retained_bytes =
        size_of::<CommonArticulationCompactPairGeneralCellTransportPrerequisiteV2>();
    let retained_bytes = transport_limits
        .max_retained_bytes
        .checked_add(outer_retained_bytes)
        .expect("N=33 bridge retained bytes");
    let peak_bytes = source_revalidation_limits
        .max_peak_bytes
        .max(transport_limits.max_peak_bytes)
        .checked_add(authority_shell_bytes)
        .and_then(|value| value.checked_add(assignment_bytes))
        .and_then(|value| value.checked_add(outer_retained_bytes))
        .and_then(|value| value.checked_add(COMPACT_PAIR_TRANSPORT_WORKSPACE_BYTES_V2))
        .expect("N=33 bridge peak bytes");
    CommonArticulationCompactPairGeneralCellTransportLimitsV2 {
        max_compact_assignment_bytes: assignment_bytes,
        max_compact_source_retained_bytes: compact_source_retained_bytes,
        max_logical_work: logical_work,
        max_retained_bytes: retained_bytes,
        max_peak_bytes: peak_bytes,
    }
}

#[test]
fn genuine_n33_compact_authority_reaches_only_unpromoted_transport_v2() {
    let fixture = golden_n33_transport_fixture_v2();
    let live = fixture.n33_live_global_input_v2();
    let (variable_count, registry_digest, direction_bits) = n33_compact_pair_assignment_v2();
    let analysis = GlobalFlatFoldabilityLimits {
        max_search_nodes: 0,
        ..GlobalFlatFoldabilityLimits::default()
    };
    let compact_limits = GlobalFlatLayerOrderCompactPairAssignmentLimitsV2 {
        analysis,
        ..GlobalFlatLayerOrderCompactPairAssignmentLimitsV2::default()
    };
    let compact = issue_global_flat_layer_order_from_compact_pair_assignment_v2(
        GlobalFlatLayerOrderCompactPairAssignmentInputV2 {
            source: live.input(&fixture),
            variable_count,
            variable_registry_sha256: registry_digest,
            direction_bits_le: &direction_bits,
        },
        compact_limits,
    )
    .expect("genuine N=33 compact authority");
    assert_eq!(compact.work_counts_v2().search_nodes, 0);
    assert_eq!(compact.variable_count_v2(), variable_count);
    let expected_assignment_digest = n33_compact_pair_assignment_sha256_v2();
    assert_eq!(
        compact.direction_assignment_sha256_v2(),
        expected_assignment_digest,
        "the bridge authority binds the checked N=33 assignment asset"
    );
    assert_eq!(
        expected_assignment_digest,
        global_flat_layer_order_compact_pair_assignment_sha256_v2(
            variable_count,
            registry_digest,
            &direction_bits,
        )
        .expect("canonical N=33 assignment digest"),
        "the production domain-separated digest reproduces the pinned receipt"
    );

    let transport_limits =
        exact_transport_limits_for_live_n33_source_v2(&fixture, compact.layer_order_snapshot_v2())
            .expect("exact N=33 transport limits");
    let source_revalidation_limits = GlobalFlatLayerOrderRevalidationLimitsV2 {
        analysis,
        max_source_retained_bytes: compact.resources_v2().layer_order_retained_bytes,
        max_peak_bytes: DEFAULT_MAX_COMPACT_LAYER_ORDER_PEAK_BYTES_V2,
    };
    let bridge_limits = exact_bridge_limits_v2(
        &compact,
        direction_bits.len(),
        source_revalidation_limits,
        transport_limits,
    );
    let normal_input = || {
        bridge_input_v2(
            &fixture,
            &compact,
            &direction_bits,
            live.input(&fixture),
            source_revalidation_limits,
            transport_limits,
            bridge_limits,
        )
    };

    // Every outer one-short is rejected by the allocation-free envelope
    // preflight. Poll three would be the first direction-hash chunk, and poll
    // six the first live source-revalidation checkpoint.
    for field in ["assignment", "source", "work", "retained", "peak"] {
        let mut one_short = bridge_limits;
        match field {
            "assignment" => one_short.max_compact_assignment_bytes -= 1,
            "source" => one_short.max_compact_source_retained_bytes -= 1,
            "work" => one_short.max_logical_work -= 1,
            "retained" => one_short.max_retained_bytes -= 1,
            "peak" => one_short.max_peak_bytes -= 1,
            _ => unreachable!(),
        }
        let mut polls = 0usize;
        let error =
            issue_common_articulation_compact_pair_general_cell_transport_prerequisite_with_checkpoint_v2(
                bridge_input_v2(
                    &fixture,
                    &compact,
                    &direction_bits,
                    live.input(&fixture),
                    source_revalidation_limits,
                    transport_limits,
                    one_short,
                ),
                || {
                    polls += 1;
                    assert!(polls <= 2, "{field} reached direction hashing or source replay");
                    Ok(())
                },
            )
            .expect_err("outer one-short fails before source replay");
        assert_eq!(
            error,
            CommonArticulationCompactPairGeneralCellTransportErrorV2::ResourceLimit,
            "{field} one-short"
        );
        assert_eq!(polls, 2, "{field} stops at the outer preflight");
    }

    // The compact authority already owns the exact retained snapshot size.
    // Known-impossible inner revalidation bounds therefore fail at the same
    // allocation-free preflight, before direction hashing begins.
    for field in ["source retained", "source peak"] {
        let mut one_short = source_revalidation_limits;
        match field {
            "source retained" => {
                one_short.max_source_retained_bytes =
                    compact.resources_v2().layer_order_retained_bytes - 1;
            }
            "source peak" => {
                one_short.max_peak_bytes = compact.resources_v2().layer_order_retained_bytes - 1;
            }
            _ => unreachable!(),
        }
        let mut polls = 0usize;
        let error =
            issue_common_articulation_compact_pair_general_cell_transport_prerequisite_with_checkpoint_v2(
                bridge_input_v2(
                    &fixture,
                    &compact,
                    &direction_bits,
                    live.input(&fixture),
                    one_short,
                    transport_limits,
                    bridge_limits,
                ),
                || {
                    polls += 1;
                    assert!(polls <= 2, "{field} reached direction hashing");
                    Ok(())
                },
            )
            .expect_err("known-impossible source envelope fails before hashing");
        assert_eq!(
            error,
            CommonArticulationCompactPairGeneralCellTransportErrorV2::ResourceLimit,
            "{field} one-short"
        );
        assert_eq!(polls, 2, "{field} stops at the outer preflight");
    }

    // Both stop classes cross the bridge-to-foldability adapter before any
    // transport candidate can be returned. For this two-chunk fixture, poll
    // six is the first live source-revalidation checkpoint.
    for stop in [
        CommonArticulationGeneralCellTransportStopV2::Cancelled,
        CommonArticulationGeneralCellTransportStopV2::DeadlineExceeded,
    ] {
        let mut polls = 0usize;
        let error =
            issue_common_articulation_compact_pair_general_cell_transport_prerequisite_with_checkpoint_v2(
                normal_input(),
                || {
                    polls += 1;
                    if polls == 6 { Err(stop) } else { Ok(()) }
                },
            )
            .expect_err("source-revalidation stop hides the transport outcome");
        assert_eq!(
            error,
            match stop {
                CommonArticulationGeneralCellTransportStopV2::Cancelled => {
                    CommonArticulationCompactPairGeneralCellTransportErrorV2::Cancelled
                }
                CommonArticulationGeneralCellTransportStopV2::DeadlineExceeded => {
                    CommonArticulationCompactPairGeneralCellTransportErrorV2::DeadlineExceeded
                }
            }
        );
    }

    let mut tampered_bits = direction_bits.clone();
    tampered_bits[0] ^= 1;
    assert!(matches!(
        issue_common_articulation_compact_pair_general_cell_transport_prerequisite_v2(
            bridge_input_v2(
                &fixture,
                &compact,
                &tampered_bits,
                live.input(&fixture),
                source_revalidation_limits,
                transport_limits,
                bridge_limits,
            )
        ),
        Err(CommonArticulationCompactPairGeneralCellTransportErrorV2::CompactSourceBindingMismatch)
    ));
    let mut foreign_registry = registry_digest;
    foreign_registry[0] ^= 1;
    assert_ne!(
        compact.direction_assignment_sha256_v2(),
        global_flat_layer_order_compact_pair_assignment_sha256_v2(
            variable_count,
            foreign_registry,
            &direction_bits,
        )
        .expect("foreign registry still has a well-formed assignment digest"),
        "the direction receipt binds the complete canonical registry"
    );

    let issued = issue_common_articulation_compact_pair_general_cell_transport_prerequisite_v2(
        normal_input(),
    )
    .expect("genuine compact N=33 source reaches transport");
    let prerequisite = issued.as_unpromoted_v2();
    assert!(!issued.is_certified_v2());
    assert!(!prerequisite.authorizes_continuous_motion());
    assert!(!prerequisite.authorizes_collision_clearance());
    assert!(!prerequisite.authorizes_layer_transport());
    assert!(!prerequisite.authorizes_project_mutation());
    assert!(!prerequisite.authorizes_apply());
    assert!(!prerequisite.authorizes_viewer());
    assert!(
        !prerequisite
            .transport_outcome_v2()
            .as_unpromoted_v2()
            .authorizes_layer_transport()
    );
    let resources = prerequisite.resources_v2();
    assert_eq!(
        (
            resources.compact_assignment_bytes,
            resources.compact_source_retained_bytes,
            resources.logical_work,
            resources.retained_bytes,
            resources.peak_bytes,
        ),
        (
            bridge_limits.max_compact_assignment_bytes,
            bridge_limits.max_compact_source_retained_bytes,
            bridge_limits.max_logical_work,
            bridge_limits.max_retained_bytes,
            bridge_limits.max_peak_bytes,
        ),
        "every bridge cap admits exact equality"
    );
    for field in ["assignment", "source", "work", "retained", "peak"] {
        let mut one_short = bridge_limits;
        match field {
            "assignment" => one_short.max_compact_assignment_bytes -= 1,
            "source" => one_short.max_compact_source_retained_bytes -= 1,
            "work" => one_short.max_logical_work -= 1,
            "retained" => one_short.max_retained_bytes -= 1,
            "peak" => one_short.max_peak_bytes -= 1,
            _ => unreachable!(),
        }
        assert_eq!(
            checked_bridge_resources_v2(
                &compact,
                direction_bits.len(),
                source_revalidation_limits,
                prerequisite.transport.as_unpromoted_v2(),
                one_short,
            ),
            Err(CommonArticulationCompactPairGeneralCellTransportErrorV2::ResourceLimit),
            "{field} one-short must fail closed"
        );
    }

    // Replay owns an independent entry path, so keep the same pre-hash
    // atomicity guarantee there rather than relying only on issuance tests.
    for field in ["assignment", "source", "work", "retained", "peak"] {
        let mut one_short = bridge_limits;
        match field {
            "assignment" => one_short.max_compact_assignment_bytes -= 1,
            "source" => one_short.max_compact_source_retained_bytes -= 1,
            "work" => one_short.max_logical_work -= 1,
            "retained" => one_short.max_retained_bytes -= 1,
            "peak" => one_short.max_peak_bytes -= 1,
            _ => unreachable!(),
        }
        let mut polls = 0usize;
        let error = prerequisite
            .revalidate_with_checkpoint_v2(
                bridge_input_v2(
                    &fixture,
                    &compact,
                    &direction_bits,
                    live.input(&fixture),
                    source_revalidation_limits,
                    transport_limits,
                    one_short,
                ),
                || {
                    polls += 1;
                    assert!(polls <= 2, "replay {field} reached direction hashing");
                    Ok(())
                },
            )
            .expect_err("replay outer one-short fails before source replay");
        assert_eq!(
            error,
            CommonArticulationCompactPairGeneralCellTransportErrorV2::ResourceLimit,
            "replay {field} one-short"
        );
        assert_eq!(polls, 2, "replay {field} stops at the outer preflight");
    }

    for stop in [
        CommonArticulationGeneralCellTransportStopV2::Cancelled,
        CommonArticulationGeneralCellTransportStopV2::DeadlineExceeded,
    ] {
        let mut polls = 0usize;
        let error = prerequisite
            .revalidate_with_checkpoint_v2(normal_input(), || {
                polls += 1;
                if polls == 6 { Err(stop) } else { Ok(()) }
            })
            .expect_err("replay source stop hides the retained observation");
        assert_eq!(
            error,
            match stop {
                CommonArticulationGeneralCellTransportStopV2::Cancelled => {
                    CommonArticulationCompactPairGeneralCellTransportErrorV2::Cancelled
                }
                CommonArticulationGeneralCellTransportStopV2::DeadlineExceeded => {
                    CommonArticulationCompactPairGeneralCellTransportErrorV2::DeadlineExceeded
                }
            }
        );
    }

    prerequisite
        .revalidate_v2(normal_input())
        .expect("same live compact source replays through transport");

    let mut foreign_source = live.input(&fixture);
    foreign_source.identity_namespace = Some(ProjectId::schema_namespace([
        0x4f, 0x52, 0x49, 0x47, 0x41, 0x4d, 0x49, 0x32, 0x5f, 0x46, 0x4f, 0x52, 0x45, 0x49, 0x47,
        0x4e,
    ]));
    assert!(matches!(
        prerequisite.revalidate_v2(bridge_input_v2(
            &fixture,
            &compact,
            &direction_bits,
            foreign_source,
            source_revalidation_limits,
            transport_limits,
            bridge_limits,
        )),
        Err(CommonArticulationCompactPairGeneralCellTransportErrorV2::SourceRevalidation(_))
    ));

    let mut drifted_pattern = fixture.clearance_fixture.pattern.clone();
    drifted_pattern.vertices[0].position.x =
        f64::from_bits(drifted_pattern.vertices[0].position.x.to_bits() + 1);
    let drifted_source = GlobalFlatFoldabilityInput::current_with_geometry(
        fixture
            .clearance_fixture
            .geometry
            .source_identity_namespace_v1()
            .expect("canonical namespace"),
        &fixture.clearance_fixture.paper,
        &drifted_pattern,
        live.input(&fixture).topology,
        live.input(&fixture).local_flat_foldability,
    );
    assert!(matches!(
        prerequisite.revalidate_v2(bridge_input_v2(
            &fixture,
            &compact,
            &direction_bits,
            drifted_source,
            source_revalidation_limits,
            transport_limits,
            bridge_limits,
        )),
        Err(CommonArticulationCompactPairGeneralCellTransportErrorV2::SourceRevalidation(_))
    ));
}
