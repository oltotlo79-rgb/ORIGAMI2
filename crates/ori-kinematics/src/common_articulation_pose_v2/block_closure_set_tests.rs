//! Focused general-N all-block closure-set contract tests.

use super::*;
use crate::{
    CanonicalCycleScheduleV1, CommonArticulationBlockClosureSetErrorV2,
    CommonArticulationBlockClosureSetInputV2, CommonArticulationBlockClosureSetLimitsV2,
    CommonArticulationBlockClosureSetStopV2, CycleScheduleEntryInputV1, CycleScheduleLimitsV1,
    DyadicIntervalClosureLimitsV1, RationalCoefficientV1,
    prove_common_articulation_block_closure_set_v2,
    prove_common_articulation_block_closure_set_with_checkpoint_v2,
};

const N34_BLOCKS: usize = 34;
const N34_HINGES: usize = 408;

#[test]
fn n34_all_blocks_reissue_and_revalidate_as_non_authorizing_observations() {
    let fixture = golden_n34_fixture_v2();
    let profile =
        CommonArticulationResourceProfileV2::exact_canonical_miura_3x3_v2(N34_BLOCKS).unwrap();
    let common_pose = prove_common_articulation_pose_authority_v2(fixture.input(&profile)).unwrap();
    let schedule = parent_schedule_v2(&fixture);
    let limits = closure_set_limits_v2(N34_BLOCKS);
    let input = CommonArticulationBlockClosureSetInputV2 {
        geometry: &fixture.geometry,
        audit: &fixture.audit,
        pose: &fixture.pose,
        parent_fixed_face: fixture.geometry.face_ids()[0],
        parent_schedule: &schedule,
        decomposition: &fixture.decomposition,
        common_pose: &common_pose,
        paper_thickness_mm: 0.1,
        closure_tolerance: 1.0e-9,
        profile: &profile,
        limits,
    };
    let evidence = prove_common_articulation_block_closure_set_v2(input).unwrap();
    let reissued = prove_common_articulation_block_closure_set_v2(input).unwrap();
    let parent_schedule_bytes = schedule.checked_deep_retained_bytes_v1().unwrap();
    let (block_schedule_bytes, block_closure_bytes) = first_block_observation_bytes_v2(
        &fixture,
        &schedule,
        input.parent_fixed_face,
        limits.per_block_closure_limits,
        input.closure_tolerance,
    );

    assert_eq!(evidence.configured_max_blocks_v2(), N34_BLOCKS);
    assert_eq!(evidence.actual_block_count_v2(), N34_BLOCKS);
    assert_eq!(evidence.total_closure_leaves_v2(), N34_BLOCKS);
    assert_eq!(
        evidence.binding_fingerprint_v2(),
        reissued.binding_fingerprint_v2()
    );
    assert_eq!(
        evidence.binding_fingerprint_v2(),
        [
            250, 84, 98, 134, 213, 33, 240, 247, 136, 109, 76, 190, 122, 204, 235, 249, 188, 204,
            69, 97, 11, 34, 19, 40, 65, 60, 210, 42, 132, 96, 189, 69,
        ]
    );
    assert!(evidence.total_block_schedule_bytes_v2() > 0);
    assert!(evidence.total_block_closure_bytes_v2() > 0);
    assert!(!evidence.authorizes_continuous_motion());
    assert!(!evidence.authorizes_collision_clearance());
    assert!(!evidence.authorizes_project_mutation());
    assert!(!evidence.authorizes_apply());
    assert!(!evidence.authorizes_viewer());
    assert!(!evidence.authorizes_layer_transport());
    evidence.revalidate_v2(input).unwrap();

    let one_short_leaves = CommonArticulationBlockClosureSetInputV2 {
        limits: CommonArticulationBlockClosureSetLimitsV2 {
            max_total_closure_leaves: N34_BLOCKS - 1,
            ..limits
        },
        ..input
    };
    assert_eq!(
        prove_common_articulation_block_closure_set_v2(one_short_leaves).unwrap_err(),
        CommonArticulationBlockClosureSetErrorV2::ResourceLimit,
    );
    let one_short_closure_bytes = CommonArticulationBlockClosureSetInputV2 {
        limits: CommonArticulationBlockClosureSetLimitsV2 {
            max_total_block_closure_bytes: evidence.total_block_closure_bytes_v2() - 1,
            ..limits
        },
        ..input
    };
    assert_eq!(
        prove_common_articulation_block_closure_set_v2(one_short_closure_bytes).unwrap_err(),
        CommonArticulationBlockClosureSetErrorV2::ResourceLimit,
    );
    for resource_limited in [
        CommonArticulationBlockClosureSetInputV2 {
            limits: CommonArticulationBlockClosureSetLimitsV2 {
                max_parent_schedule_bytes: parent_schedule_bytes - 1,
                ..limits
            },
            ..input
        },
        CommonArticulationBlockClosureSetInputV2 {
            limits: CommonArticulationBlockClosureSetLimitsV2 {
                max_block_schedule_bytes: block_schedule_bytes - 1,
                ..limits
            },
            ..input
        },
        CommonArticulationBlockClosureSetInputV2 {
            limits: CommonArticulationBlockClosureSetLimitsV2 {
                max_total_block_schedule_bytes: evidence.total_block_schedule_bytes_v2() - 1,
                ..limits
            },
            ..input
        },
        CommonArticulationBlockClosureSetInputV2 {
            limits: CommonArticulationBlockClosureSetLimitsV2 {
                max_block_closure_bytes: block_closure_bytes - 1,
                ..limits
            },
            ..input
        },
    ] {
        assert_eq!(
            prove_common_articulation_block_closure_set_v2(resource_limited).unwrap_err(),
            CommonArticulationBlockClosureSetErrorV2::ResourceLimit,
        );
    }
    let one_short_per_block_leaves = CommonArticulationBlockClosureSetInputV2 {
        limits: CommonArticulationBlockClosureSetLimitsV2 {
            per_block_closure_limits: DyadicIntervalClosureLimitsV1 {
                max_leaves: 0,
                ..limits.per_block_closure_limits
            },
            ..limits
        },
        ..input
    };
    assert_eq!(
        prove_common_articulation_block_closure_set_v2(one_short_per_block_leaves).unwrap_err(),
        CommonArticulationBlockClosureSetErrorV2::InvalidInput,
    );

    assert_eq!(
        evidence.revalidate_v2(CommonArticulationBlockClosureSetInputV2 {
            closure_tolerance: 0.0,
            ..input
        }),
        Err(CommonArticulationBlockClosureSetErrorV2::IssuerMismatch),
    );
    assert_eq!(
        prove_common_articulation_block_closure_set_v2(CommonArticulationBlockClosureSetInputV2 {
            closure_tolerance: -0.0,
            ..input
        })
        .unwrap_err(),
        CommonArticulationBlockClosureSetErrorV2::InvalidInput,
    );
    assert_eq!(
        evidence.revalidate_v2(CommonArticulationBlockClosureSetInputV2 {
            paper_thickness_mm: 0.2,
            ..input
        }),
        Err(CommonArticulationBlockClosureSetErrorV2::IssuerMismatch),
    );
    let foreign_fixed_face = fixture.geometry.face_ids()[1];
    assert_eq!(
        prove_common_articulation_block_closure_set_v2(CommonArticulationBlockClosureSetInputV2 {
            parent_fixed_face: foreign_fixed_face,
            ..input
        })
        .unwrap_err(),
        CommonArticulationBlockClosureSetErrorV2::InvalidInput,
    );
    let mut one_ulp_entries = zero_schedule_entries_v2(&fixture.geometry);
    one_ulp_entries[0].initial_angle_degrees_bits = 1;
    let one_ulp_schedule = CanonicalCycleScheduleV1::prepare(
        &fixture.geometry,
        &fixture.audit,
        input.parent_fixed_face,
        [0.0, 1.0],
        one_ulp_entries,
        parent_schedule_limits_for_v2(),
    )
    .unwrap();
    assert_eq!(
        prove_common_articulation_block_closure_set_v2(CommonArticulationBlockClosureSetInputV2 {
            parent_schedule: &one_ulp_schedule,
            ..input
        })
        .unwrap_err(),
        CommonArticulationBlockClosureSetErrorV2::InvalidInput,
    );
}

#[test]
fn configured_n40_actual_n34_is_admitted_but_wrong_bound_and_stop_reject() {
    let fixture = miura_fixture_v2(N34_BLOCKS);
    let profile =
        CommonArticulationResourceProfileV2::for_canonical_miura_3x3_v2(40, N34_BLOCKS).unwrap();
    let decomposition = fixture.decomposition_with_profile(&profile);
    let common_pose = prove_common_articulation_pose_authority_v2(CommonArticulationPoseInputV2 {
        geometry: &fixture.geometry,
        pose: &fixture.pose,
        decomposition: &decomposition,
        paper_thickness_mm: 0.1,
        profile: &profile,
    })
    .unwrap();
    let schedule = parent_schedule_v2(&fixture);
    let limits = closure_set_limits_v2(40);
    let input = CommonArticulationBlockClosureSetInputV2 {
        geometry: &fixture.geometry,
        audit: &fixture.audit,
        pose: &fixture.pose,
        parent_fixed_face: fixture.geometry.face_ids()[0],
        parent_schedule: &schedule,
        decomposition: &decomposition,
        common_pose: &common_pose,
        paper_thickness_mm: 0.1,
        closure_tolerance: 1.0e-9,
        profile: &profile,
        limits,
    };
    let evidence = prove_common_articulation_block_closure_set_v2(input).unwrap();
    assert_eq!(evidence.configured_max_blocks_v2(), 40);
    assert_eq!(evidence.actual_block_count_v2(), N34_BLOCKS);

    let wrong_configured_bound = CommonArticulationBlockClosureSetInputV2 {
        limits: CommonArticulationBlockClosureSetLimitsV2 {
            max_blocks: N34_BLOCKS,
            ..limits
        },
        ..input
    };
    assert_eq!(
        prove_common_articulation_block_closure_set_v2(wrong_configured_bound).unwrap_err(),
        CommonArticulationBlockClosureSetErrorV2::InvalidInput,
    );

    let mut successful_polls = 0usize;
    prove_common_articulation_block_closure_set_with_checkpoint_v2(input, || {
        successful_polls += 1;
        Ok(())
    })
    .unwrap();
    assert!(successful_polls > 2);
    let mut polls = 0usize;
    assert_eq!(
        prove_common_articulation_block_closure_set_with_checkpoint_v2(input, || {
            polls += 1;
            (polls != successful_polls)
                .then_some(())
                .ok_or(CommonArticulationBlockClosureSetStopV2::Cancelled)
        })
        .unwrap_err(),
        CommonArticulationBlockClosureSetErrorV2::Cancelled,
    );
    assert_eq!(polls, successful_polls);
}

fn first_block_observation_bytes_v2(
    fixture: &MiuraFixtureV2,
    schedule: &CanonicalCycleScheduleV1,
    parent_fixed_face: ori_domain::FaceId,
    closure_limits: DyadicIntervalClosureLimitsV1,
    tolerance: f64,
) -> (usize, usize) {
    let block = &fixture.decomposition.blocks()[0];
    let fixed_face = block
        .geometry()
        .face_ids()
        .iter()
        .filter(|face| fixture.decomposition.articulation_faces().contains(face))
        .min_by_key(|face| face.canonical_bytes())
        .copied()
        .unwrap();
    let restricted = schedule
        .restrict_to_edge_block_with_fixed_face_v1(
            &fixture.geometry,
            &fixture.audit,
            block.geometry(),
            block.audit(),
            fixed_face,
        )
        .unwrap();
    assert!(schedule.matches_binding(&fixture.geometry, &fixture.audit, parent_fixed_face));
    let closure = block
        .geometry()
        .prove_dyadic_schedule_closure_v1(
            block.audit(),
            fixed_face,
            &restricted,
            tolerance,
            closure_limits,
        )
        .unwrap();
    (
        restricted.checked_deep_retained_bytes_v1().unwrap(),
        closure.checked_deep_retained_bytes_v1().unwrap(),
    )
}

fn parent_schedule_v2(fixture: &MiuraFixtureV2) -> CanonicalCycleScheduleV1 {
    CanonicalCycleScheduleV1::prepare(
        &fixture.geometry,
        &fixture.audit,
        fixture.geometry.face_ids()[0],
        [0.0, 1.0],
        zero_schedule_entries_v2(&fixture.geometry),
        parent_schedule_limits_for_v2(),
    )
    .unwrap()
}

fn golden_n34_fixture_v2() -> MiuraFixtureV2 {
    miura_fixture_with_namespace_v2(
        N34_BLOCKS,
        ori_domain::ProjectId::schema_namespace([
            0x4f, 0x52, 0x49, 0x47, 0x41, 0x4d, 0x49, 0x32, 0x5f, 0x4e, 0x5f, 0x56, 0x32, 0, 0, 1,
        ]),
    )
}

fn parent_schedule_limits_for_v2() -> CycleScheduleLimitsV1 {
    CycleScheduleLimitsV1 {
        max_hinges: N34_HINGES,
        max_degree: 0,
        max_coefficient_bits: 1,
        max_work: N34_HINGES,
    }
}

fn zero_schedule_entries_v2(
    geometry: &MaterialHingeGraphGeometry,
) -> Vec<CycleScheduleEntryInputV1> {
    let mut entries = geometry
        .hinges()
        .iter()
        .map(|hinge| CycleScheduleEntryInputV1 {
            edge: hinge.edge(),
            initial_angle_degrees_bits: 0.0_f64.to_bits(),
            chebyshev_coefficients: vec![RationalCoefficientV1 {
                numerator: 0,
                denominator: 1,
            }],
        })
        .collect::<Vec<_>>();
    entries.sort_unstable_by_key(|entry| entry.edge.canonical_bytes());
    entries
}

fn closure_set_limits_v2(
    configured_max_blocks: usize,
) -> CommonArticulationBlockClosureSetLimitsV2 {
    const PER_BLOCK_BYTES: usize = 8_192;
    CommonArticulationBlockClosureSetLimitsV2 {
        max_blocks: configured_max_blocks,
        max_parent_schedule_bytes: 65_536,
        max_block_schedule_bytes: PER_BLOCK_BYTES,
        max_total_block_schedule_bytes: configured_max_blocks * PER_BLOCK_BYTES,
        max_block_closure_bytes: PER_BLOCK_BYTES,
        max_total_block_closure_bytes: configured_max_blocks * PER_BLOCK_BYTES,
        max_total_closure_leaves: configured_max_blocks,
        per_block_closure_limits: DyadicIntervalClosureLimitsV1 {
            max_depth: 0,
            max_leaves: 1,
            max_work: 1,
            schedule_limits: CycleScheduleLimitsV1 {
                max_hinges: 12,
                max_degree: 0,
                max_coefficient_bits: 1,
                max_work: 12,
            },
        },
    }
}
