use super::*;

type PolicyMutation = (
    &'static str,
    fn(&mut CommonArticulationDynamicClosureBridgeLimitsV2),
);

fn assert_issuer_mismatch(
    bridge: &CommonArticulationDynamicClosureBridgeV2,
    input: CommonArticulationDynamicClosureBridgeRevalidationInputV2<'_>,
    label: &str,
) {
    assert_eq!(
        bridge.revalidate_v2(input).expect_err(label),
        CommonArticulationDynamicClosureBridgeErrorV2::IssuerMismatch,
        "{label}"
    );
}

#[test]
fn bridge_replay_binds_every_live_component_and_value_replays_are_stable() {
    let issuer =
        miura_fixture_with_namespace_v2(N33_BLOCKS, ProjectId::schema_namespace([0x71; 16]));
    let foreign =
        miura_fixture_with_namespace_v2(N33_BLOCKS, ProjectId::schema_namespace([0x72; 16]));
    let profile =
        CommonArticulationResourceProfileV2::exact_canonical_miura_3x3_v2(N33_BLOCKS).unwrap();
    let reconstructed_profile =
        CommonArticulationResourceProfileV2::exact_canonical_miura_3x3_v2(N33_BLOCKS).unwrap();
    let foreign_profile =
        CommonArticulationResourceProfileV2::for_canonical_miura_3x3_v2(34, N33_BLOCKS).unwrap();
    let (issuer_schedule, _) = nonstationary_parent_schedule_v2(&issuer);
    let (foreign_schedule, _) = nonstationary_parent_schedule_v2(&foreign);
    let issuer_pose = issuer
        .geometry
        .solve_closed(
            &issuer.audit,
            issuer.geometry.face_ids()[0],
            &issuer_schedule.evaluate(0.0).unwrap(),
            1.0e-8,
        )
        .unwrap();
    let foreign_pose = foreign
        .geometry
        .solve_closed(
            &foreign.audit,
            foreign.geometry.face_ids()[0],
            &foreign_schedule.evaluate(0.0).unwrap(),
            1.0e-8,
        )
        .unwrap();
    let issuer_common_pose =
        prove_common_articulation_pose_authority_v2(CommonArticulationPoseInputV2 {
            geometry: &issuer.geometry,
            pose: &issuer_pose,
            decomposition: &issuer.decomposition,
            paper_thickness_mm: 0.1,
            profile: &profile,
        })
        .unwrap();
    let replay_common_pose =
        prove_common_articulation_pose_authority_v2(CommonArticulationPoseInputV2 {
            geometry: &issuer.geometry,
            pose: &issuer_pose,
            decomposition: &issuer.decomposition,
            paper_thickness_mm: 0.1,
            profile: &reconstructed_profile,
        })
        .unwrap();
    let foreign_common_pose =
        prove_common_articulation_pose_authority_v2(CommonArticulationPoseInputV2 {
            geometry: &foreign.geometry,
            pose: &foreign_pose,
            decomposition: &foreign.decomposition,
            paper_thickness_mm: 0.1,
            profile: &profile,
        })
        .unwrap();
    let limits = bridge_limits_v2();
    let bridge = prove_common_articulation_dynamic_closure_bridge_v2(bridge_input_v2(
        &issuer,
        &profile,
        &issuer_pose,
        &issuer_common_pose,
        &issuer_schedule,
        limits,
    ))
    .unwrap();
    let repeated = prove_common_articulation_dynamic_closure_bridge_v2(bridge_input_v2(
        &issuer,
        &profile,
        &issuer_pose,
        &issuer_common_pose,
        &issuer_schedule,
        limits,
    ))
    .unwrap();
    assert_eq!(
        bridge.binding_fingerprint_v2(),
        repeated.binding_fingerprint_v2()
    );

    let base = bridge_revalidation_input_v2(
        &issuer,
        &profile,
        &issuer_pose,
        &issuer_common_pose,
        &issuer_schedule,
    );
    bridge
        .revalidate_v2(CommonArticulationDynamicClosureBridgeRevalidationInputV2 {
            profile: &reconstructed_profile,
            common_pose: &replay_common_pose,
            ..base
        })
        .expect("value-equivalent profile and authority replay");

    let fresh_decomposition = issuer.decomposition_with_profile(&profile);
    for (label, candidate) in [
        (
            "foreign geometry",
            CommonArticulationDynamicClosureBridgeRevalidationInputV2 {
                geometry: &foreign.geometry,
                ..base
            },
        ),
        (
            "foreign audit",
            CommonArticulationDynamicClosureBridgeRevalidationInputV2 {
                audit: &foreign.audit,
                ..base
            },
        ),
        (
            "foreign pose",
            CommonArticulationDynamicClosureBridgeRevalidationInputV2 {
                pose: &foreign_pose,
                ..base
            },
        ),
        (
            "foreign fixed face",
            CommonArticulationDynamicClosureBridgeRevalidationInputV2 {
                parent_fixed_face: issuer.geometry.face_ids()[1],
                ..base
            },
        ),
        (
            "foreign schedule",
            CommonArticulationDynamicClosureBridgeRevalidationInputV2 {
                parent_schedule: &foreign_schedule,
                ..base
            },
        ),
        (
            "foreign decomposition instance",
            CommonArticulationDynamicClosureBridgeRevalidationInputV2 {
                decomposition: &fresh_decomposition,
                ..base
            },
        ),
        (
            "foreign common-pose authority",
            CommonArticulationDynamicClosureBridgeRevalidationInputV2 {
                common_pose: &foreign_common_pose,
                ..base
            },
        ),
        (
            "foreign thickness",
            CommonArticulationDynamicClosureBridgeRevalidationInputV2 {
                paper_thickness_mm: 0.2,
                ..base
            },
        ),
        (
            "foreign closure tolerance",
            CommonArticulationDynamicClosureBridgeRevalidationInputV2 {
                closure_tolerance: 1.0e-8,
                ..base
            },
        ),
        (
            "foreign resource profile",
            CommonArticulationDynamicClosureBridgeRevalidationInputV2 {
                profile: &foreign_profile,
                ..base
            },
        ),
    ] {
        assert_issuer_mismatch(&bridge, candidate, label);
    }

    let policy_mutations: [PolicyMutation; 16] = [
        ("validation work", |value| value.max_validation_work += 1),
        ("restriction work", |value| {
            value.max_total_restriction_work += 1
        }),
        ("block schedule retained", |value| {
            value.max_total_restricted_schedule_retained_bytes += 1
        }),
        ("block closure retained", |value| {
            value.max_total_block_closure_retained_bytes += 1
        }),
        ("block leaves", |value| value.max_total_block_leaves += 1),
        ("parent schedule retained", |value| {
            value.max_parent_schedule_retained_bytes += 1
        }),
        ("parent closure retained", |value| {
            value.max_parent_closure_retained_bytes += 1
        }),
        ("parent leaves", |value| value.max_parent_leaves += 1),
        ("bundle retained", |value| {
            value.max_bundle_retained_bytes += 1
        }),
        ("issuance peak", |value| value.max_issuance_peak_bytes += 1),
        ("revalidation peak", |value| {
            value.max_revalidation_peak_bytes += 1
        }),
        ("schedule degree", |value| value.max_schedule_degree += 1),
        ("coefficient bits", |value| {
            value.max_schedule_coefficient_bits += 1
        }),
        ("dyadic depth", |value| value.max_dyadic_depth += 1),
        ("per-closure leaves", |value| {
            value.max_dyadic_leaves_per_closure += 1
        }),
        ("per-closure work", |value| {
            value.max_dyadic_work_per_closure += 1
        }),
    ];
    for (label, mutate) in policy_mutations {
        let mut changed_policy = limits;
        mutate(&mut changed_policy);
        let changed = prove_common_articulation_dynamic_closure_bridge_v2(bridge_input_v2(
            &issuer,
            &profile,
            &issuer_pose,
            &issuer_common_pose,
            &issuer_schedule,
            changed_policy,
        ))
        .unwrap_or_else(|error| panic!("{label}: {error:?}"));
        assert_ne!(
            bridge.binding_fingerprint_v2(),
            changed.binding_fingerprint_v2(),
            "sealed public policy field: {label}"
        );
    }
}
