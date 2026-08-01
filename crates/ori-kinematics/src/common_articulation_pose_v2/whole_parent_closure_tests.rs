//! Focused whole-parent closure tests using the shared canonical-Miura fixture.

use super::*;
use crate::{
    CanonicalCycleScheduleV1, CommonArticulationBlockClosureSetErrorV2,
    CommonArticulationBlockClosureSetInputV2, CommonArticulationBlockClosureSetLimitsV2,
    CommonArticulationBlockClosureSetV2, CommonArticulationPoseAuthorityV2,
    CommonArticulationResourceProfileV2, CommonArticulationWholeParentClosureErrorV2,
    CommonArticulationWholeParentClosureInputV2, CommonArticulationWholeParentClosureLimitsV2,
    CommonArticulationWholeParentClosureStopV2, CycleScheduleLimitsV1,
    DyadicIntervalClosureLimitsV1, MaterialHingeGraphGeometry,
    prove_common_articulation_block_closure_set_v2,
    prove_common_articulation_whole_parent_closure_v2,
    prove_common_articulation_whole_parent_closure_with_checkpoint_v2,
};
use ori_domain::FaceId;

const THICKNESS_MM: f64 = 0.1;
const CLOSURE_TOLERANCE: f64 = 0.0;

#[test]
fn n33_fixed_namespace_reissues_a_complete_non_authorizing_parent_observation() {
    let setup = whole_parent_setup_v2(
        33,
        33,
        ProjectId::schema_namespace([
            0x4f, 0x52, 0x49, 0x47, 0x41, 0x4d, 0x49, 0x32, 0x5f, 0x57, 0x50, 0x43, 0x56, 0x32, 0,
            1,
        ]),
    );
    let evidence = prove_common_articulation_whole_parent_closure_v2(setup.input()).unwrap();
    let reissued = prove_common_articulation_whole_parent_closure_v2(setup.input()).unwrap();

    assert_eq!(evidence.configured_max_blocks_v2(), 33);
    assert_eq!(evidence.actual_block_count_v2(), 33);
    assert_eq!(evidence.parent_closure_leaves_v2(), 1);
    assert_eq!(
        evidence.binding_fingerprint_v2(),
        [
            241, 95, 171, 250, 44, 118, 130, 94, 222, 78, 246, 16, 237, 21, 25, 205, 238, 28, 28,
            147, 1, 26, 17, 18, 114, 19, 48, 226, 139, 98, 30, 129,
        ],
        "the fixed namespace has a cross-run whole-parent binding golden"
    );
    assert_eq!(
        evidence.binding_fingerprint_v2(),
        reissued.binding_fingerprint_v2(),
        "the fixed namespace is a deterministic golden fixture"
    );
    assert_eq!(
        evidence.parent_closure_binding_fingerprint_v2(),
        reissued.parent_closure_binding_fingerprint_v2()
    );
    assert!(!evidence.authorizes_continuous_motion());
    assert!(!evidence.authorizes_collision_clearance());
    assert!(!evidence.authorizes_layer_transport());
    assert!(!evidence.authorizes_project_mutation());
    assert!(!evidence.authorizes_apply());
    assert!(!evidence.authorizes_viewer());
    evidence.revalidate_v2(setup.input()).unwrap();
}

#[test]
fn configured_n40_actual_n34_revalidates_and_parent_resource_envelopes_fail_closed() {
    let setup = whole_parent_setup_v2(40, 34, ProjectId::new());
    let evidence = prove_common_articulation_whole_parent_closure_v2(setup.input()).unwrap();
    assert_eq!(evidence.configured_max_blocks_v2(), 40);
    assert_eq!(evidence.actual_block_count_v2(), 34);
    let exact_limits = CommonArticulationWholeParentClosureLimitsV2 {
        max_parent_schedule_bytes: evidence.parent_schedule_bytes_v2(),
        max_parent_closure_bytes: evidence.parent_closure_bytes_v2(),
        max_parent_closure_leaves: evidence.parent_closure_leaves_v2(),
        ..setup.limits
    };
    prove_common_articulation_whole_parent_closure_v2(
        CommonArticulationWholeParentClosureInputV2 {
            limits: exact_limits,
            ..setup.input()
        },
    )
    .unwrap();

    for limits in [
        CommonArticulationWholeParentClosureLimitsV2 {
            max_parent_schedule_bytes: evidence.parent_schedule_bytes_v2() - 1,
            ..setup.limits
        },
        CommonArticulationWholeParentClosureLimitsV2 {
            max_parent_closure_bytes: evidence.parent_closure_bytes_v2() - 1,
            ..setup.limits
        },
        CommonArticulationWholeParentClosureLimitsV2 {
            max_parent_closure_leaves: evidence.parent_closure_leaves_v2() - 1,
            ..setup.limits
        },
    ] {
        assert_eq!(
            prove_common_articulation_whole_parent_closure_v2(
                CommonArticulationWholeParentClosureInputV2 {
                    limits,
                    ..setup.input()
                }
            )
            .unwrap_err(),
            CommonArticulationWholeParentClosureErrorV2::ResourceLimit,
        );
    }
}

#[test]
fn foreign_schedule_one_ulp_signed_zero_and_live_drift_fail_closed() {
    let setup = whole_parent_setup_v2(33, 33, ProjectId::new());
    let evidence = prove_common_articulation_whole_parent_closure_v2(setup.input()).unwrap();

    let mut entries = zero_cycle_schedule_entries_v2(&setup.fixture.geometry);
    entries[0].initial_angle_degrees_bits = 1;
    let one_ulp_schedule = CanonicalCycleScheduleV1::prepare(
        &setup.fixture.geometry,
        &setup.fixture.audit,
        setup.fixed_face,
        [0.0, 1.0],
        entries,
        parent_schedule_limits_v2(&setup.fixture.geometry),
    )
    .unwrap();
    assert!(matches!(
        prove_common_articulation_whole_parent_closure_v2(
            CommonArticulationWholeParentClosureInputV2 {
                parent_fixed_face: setup.fixture.geometry.face_ids()[1],
                ..setup.input()
            }
        ),
        Err(
            CommonArticulationWholeParentClosureErrorV2::BlockClosureSet(
                CommonArticulationBlockClosureSetErrorV2::InvalidInput
            )
        )
    ));
    assert!(matches!(
        prove_common_articulation_whole_parent_closure_v2(
            CommonArticulationWholeParentClosureInputV2 {
                parent_schedule: &one_ulp_schedule,
                ..setup.input()
            }
        ),
        Err(
            CommonArticulationWholeParentClosureErrorV2::BlockClosureSet(
                CommonArticulationBlockClosureSetErrorV2::InvalidInput
            )
        )
    ));
    assert!(matches!(
        prove_common_articulation_whole_parent_closure_v2(
            CommonArticulationWholeParentClosureInputV2 {
                closure_tolerance: -0.0,
                ..setup.input()
            }
        ),
        Err(
            CommonArticulationWholeParentClosureErrorV2::BlockClosureSet(
                CommonArticulationBlockClosureSetErrorV2::InvalidInput
            )
        )
    ));
    assert!(matches!(
        evidence.revalidate_v2(CommonArticulationWholeParentClosureInputV2 {
            paper_thickness_mm: 0.2,
            ..setup.input()
        }),
        Err(
            CommonArticulationWholeParentClosureErrorV2::BlockClosureSet(
                CommonArticulationBlockClosureSetErrorV2::IssuerMismatch
            )
        )
    ));
    assert_eq!(
        evidence.revalidate_v2(CommonArticulationWholeParentClosureInputV2 {
            limits: CommonArticulationWholeParentClosureLimitsV2 {
                max_parent_closure_bytes: evidence.parent_closure_bytes_v2() + 1,
                ..setup.limits
            },
            ..setup.input()
        }),
        Err(CommonArticulationWholeParentClosureErrorV2::IssuerMismatch),
    );
}

#[test]
fn foreign_same_shape_input_and_parent_fixed_face_drift_fail_closed() {
    let retained = whole_parent_setup_v2(
        33,
        33,
        ProjectId::schema_namespace([
            0x4f, 0x52, 0x49, 0x47, 0x41, 0x4d, 0x49, 0x32, 0x5f, 0x57, 0x50, 0x43, 0x56, 0x32, 0,
            2,
        ]),
    );
    let foreign = whole_parent_setup_v2(
        33,
        33,
        ProjectId::schema_namespace([
            0x4f, 0x52, 0x49, 0x47, 0x41, 0x4d, 0x49, 0x32, 0x5f, 0x57, 0x50, 0x43, 0x56, 0x32, 0,
            3,
        ]),
    );
    let evidence = prove_common_articulation_whole_parent_closure_v2(retained.input()).unwrap();

    // The foreign input is a coherent same-shape replacement: geometry,
    // pose, decomposition, profile, schedule, and lower closure set all
    // reissue together, so only the retained whole-parent issuer detects it.
    assert_eq!(
        evidence.revalidate_v2(foreign.input()),
        Err(CommonArticulationWholeParentClosureErrorV2::IssuerMismatch),
    );
    for foreign_input in [
        CommonArticulationWholeParentClosureInputV2 {
            geometry: &foreign.fixture.geometry,
            ..retained.input()
        },
        CommonArticulationWholeParentClosureInputV2 {
            pose: &foreign.fixture.pose,
            ..retained.input()
        },
        CommonArticulationWholeParentClosureInputV2 {
            decomposition: &foreign.decomposition,
            ..retained.input()
        },
        CommonArticulationWholeParentClosureInputV2 {
            parent_schedule: &foreign.schedule,
            ..retained.input()
        },
    ] {
        assert_eq!(
            evidence.revalidate_v2(foreign_input),
            Err(
                CommonArticulationWholeParentClosureErrorV2::BlockClosureSet(
                    CommonArticulationBlockClosureSetErrorV2::InvalidInput
                )
            ),
        );
    }
    assert_eq!(
        evidence.revalidate_v2(CommonArticulationWholeParentClosureInputV2 {
            common_pose: &foreign.common_pose,
            ..retained.input()
        }),
        Err(
            CommonArticulationWholeParentClosureErrorV2::BlockClosureSet(
                CommonArticulationBlockClosureSetErrorV2::IssuerMismatch
            )
        ),
    );
    let foreign_profile = CommonArticulationResourceProfileV2::for_canonical_miura_3x3_v2(40, 33)
        .expect("same-shape N33 actual profile with a foreign configured bound");
    assert_eq!(
        evidence.revalidate_v2(CommonArticulationWholeParentClosureInputV2 {
            profile: &foreign_profile,
            ..retained.input()
        }),
        Err(
            CommonArticulationWholeParentClosureErrorV2::BlockClosureSet(
                CommonArticulationBlockClosureSetErrorV2::InvalidInput
            )
        ),
    );
    assert_eq!(
        evidence.revalidate_v2(CommonArticulationWholeParentClosureInputV2 {
            block_closure_set: &foreign.block_closure_set,
            ..retained.input()
        }),
        Err(
            CommonArticulationWholeParentClosureErrorV2::BlockClosureSet(
                CommonArticulationBlockClosureSetErrorV2::IssuerMismatch
            )
        ),
    );
    assert!(matches!(
        evidence.revalidate_v2(CommonArticulationWholeParentClosureInputV2 {
            parent_fixed_face: retained.fixture.geometry.face_ids()[1],
            ..retained.input()
        }),
        Err(
            CommonArticulationWholeParentClosureErrorV2::BlockClosureSet(
                CommonArticulationBlockClosureSetErrorV2::InvalidInput
            )
        )
    ));
}

#[test]
fn n32_profiles_are_outside_the_general_n_whole_parent_boundary() {
    let setup = whole_parent_setup_v2(33, 33, ProjectId::new());
    let configured_n32 = CommonArticulationResourceProfileV2::exact_canonical_miura_3x3_v2(32)
        .expect("N32 resource arithmetic remains representable");
    let actual_n32 = CommonArticulationResourceProfileV2::for_canonical_miura_3x3_v2(33, 32)
        .expect("configured N33 / actual N32 resource arithmetic remains representable");

    for profile in [&configured_n32, &actual_n32] {
        assert!(matches!(
            prove_common_articulation_whole_parent_closure_v2(
                CommonArticulationWholeParentClosureInputV2 {
                    profile,
                    ..setup.input()
                }
            ),
            Err(
                CommonArticulationWholeParentClosureErrorV2::BlockClosureSet(
                    CommonArticulationBlockClosureSetErrorV2::InvalidInput
                )
            )
        ));
    }
}

#[test]
fn cancellation_and_deadline_are_observed_at_every_issue_and_revalidation_boundary() {
    let setup = whole_parent_setup_v2(33, 33, ProjectId::new());
    let mut successful_issue_polls = 0usize;
    prove_common_articulation_whole_parent_closure_with_checkpoint_v2(setup.input(), || {
        successful_issue_polls += 1;
        Ok(())
    })
    .unwrap();
    assert!(successful_issue_polls > 4);

    for (stop, expected) in [
        (
            CommonArticulationWholeParentClosureStopV2::Cancelled,
            CommonArticulationWholeParentClosureErrorV2::Cancelled,
        ),
        (
            CommonArticulationWholeParentClosureStopV2::DeadlineExceeded,
            CommonArticulationWholeParentClosureErrorV2::DeadlineExceeded,
        ),
    ] {
        for stop_at in [1, successful_issue_polls / 2, successful_issue_polls] {
            let mut polls = 0usize;
            assert_eq!(
                prove_common_articulation_whole_parent_closure_with_checkpoint_v2(
                    setup.input(),
                    || {
                        polls += 1;
                        (polls != stop_at).then_some(()).ok_or(stop)
                    }
                )
                .unwrap_err(),
                expected,
            );
            assert_eq!(polls, stop_at);
        }
    }

    let evidence = prove_common_articulation_whole_parent_closure_v2(setup.input()).unwrap();
    let mut successful_revalidation_polls = 0usize;
    evidence
        .revalidate_with_checkpoint_v2(setup.input(), || {
            successful_revalidation_polls += 1;
            Ok(())
        })
        .unwrap();
    assert!(successful_revalidation_polls > successful_issue_polls);

    for (stop, expected) in [
        (
            CommonArticulationWholeParentClosureStopV2::Cancelled,
            CommonArticulationWholeParentClosureErrorV2::Cancelled,
        ),
        (
            CommonArticulationWholeParentClosureStopV2::DeadlineExceeded,
            CommonArticulationWholeParentClosureErrorV2::DeadlineExceeded,
        ),
    ] {
        for stop_at in [
            1,
            successful_revalidation_polls / 2,
            successful_revalidation_polls,
        ] {
            let mut polls = 0usize;
            assert_eq!(
                evidence
                    .revalidate_with_checkpoint_v2(setup.input(), || {
                        polls += 1;
                        (polls != stop_at).then_some(()).ok_or(stop)
                    })
                    .unwrap_err(),
                expected,
            );
            assert_eq!(polls, stop_at);
        }
    }
}

struct WholeParentSetupV2 {
    fixture: MiuraFixtureV2,
    profile: CommonArticulationResourceProfileV2,
    decomposition: CanonicalMaterialEdgeBlockDecompositionV2,
    common_pose: CommonArticulationPoseAuthorityV2,
    schedule: CanonicalCycleScheduleV1,
    block_closure_set: CommonArticulationBlockClosureSetV2,
    fixed_face: FaceId,
    limits: CommonArticulationWholeParentClosureLimitsV2,
}

impl WholeParentSetupV2 {
    fn input(&self) -> CommonArticulationWholeParentClosureInputV2<'_> {
        CommonArticulationWholeParentClosureInputV2 {
            geometry: &self.fixture.geometry,
            audit: &self.fixture.audit,
            pose: &self.fixture.pose,
            parent_fixed_face: self.fixed_face,
            parent_schedule: &self.schedule,
            decomposition: &self.decomposition,
            common_pose: &self.common_pose,
            paper_thickness_mm: THICKNESS_MM,
            closure_tolerance: CLOSURE_TOLERANCE,
            profile: &self.profile,
            block_closure_set: &self.block_closure_set,
            limits: self.limits,
        }
    }
}

fn whole_parent_setup_v2(
    configured_max_blocks: usize,
    actual_block_count: usize,
    namespace: ProjectId,
) -> WholeParentSetupV2 {
    let fixture = miura_fixture_with_namespace_v2(actual_block_count, namespace);
    let profile = CommonArticulationResourceProfileV2::for_canonical_miura_3x3_v2(
        configured_max_blocks,
        actual_block_count,
    )
    .unwrap();
    let decomposition = fixture.decomposition_with_profile(&profile);
    let common_pose = prove_common_articulation_pose_authority_v2(CommonArticulationPoseInputV2 {
        geometry: &fixture.geometry,
        pose: &fixture.pose,
        decomposition: &decomposition,
        paper_thickness_mm: THICKNESS_MM,
        profile: &profile,
    })
    .unwrap();
    let fixed_face = fixture.geometry.face_ids()[0];
    let schedule = CanonicalCycleScheduleV1::prepare(
        &fixture.geometry,
        &fixture.audit,
        fixed_face,
        [0.0, 1.0],
        zero_cycle_schedule_entries_v2(&fixture.geometry),
        parent_schedule_limits_v2(&fixture.geometry),
    )
    .unwrap();
    let block_closure_limits = block_closure_set_limits_v2(configured_max_blocks);
    let block_closure_set =
        prove_common_articulation_block_closure_set_v2(CommonArticulationBlockClosureSetInputV2 {
            geometry: &fixture.geometry,
            audit: &fixture.audit,
            pose: &fixture.pose,
            parent_fixed_face: fixed_face,
            parent_schedule: &schedule,
            decomposition: &decomposition,
            common_pose: &common_pose,
            paper_thickness_mm: THICKNESS_MM,
            closure_tolerance: CLOSURE_TOLERANCE,
            profile: &profile,
            limits: block_closure_limits,
        })
        .unwrap();
    let parent_closure_limits = DyadicIntervalClosureLimitsV1 {
        max_depth: 0,
        max_leaves: 1,
        max_work: 1,
        schedule_limits: parent_schedule_limits_v2(&fixture.geometry),
    };
    WholeParentSetupV2 {
        fixture,
        profile,
        decomposition,
        common_pose,
        schedule,
        block_closure_set,
        fixed_face,
        limits: CommonArticulationWholeParentClosureLimitsV2 {
            block_closure_set_limits: block_closure_limits,
            max_parent_schedule_bytes: 65_536,
            max_parent_closure_bytes: 65_536,
            max_parent_closure_leaves: 1,
            parent_closure_limits,
        },
    }
}

fn parent_schedule_limits_v2(geometry: &MaterialHingeGraphGeometry) -> CycleScheduleLimitsV1 {
    CycleScheduleLimitsV1 {
        max_hinges: geometry.hinges().len(),
        max_degree: 0,
        max_coefficient_bits: 1,
        max_work: geometry.hinges().len(),
    }
}

fn block_closure_set_limits_v2(
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
