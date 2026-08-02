//! Focused contract tests for the private owned dynamic closure bundle.

use std::collections::HashSet;

use super::*;
use crate::common_articulation_dynamic_closure_bundle_v2::{
    CommonArticulationDynamicClosureBundleErrorV2, CommonArticulationDynamicClosureBundleInputV2,
    CommonArticulationDynamicClosureBundleLimitsV2, CommonArticulationDynamicClosureBundleStopV2,
    prove_common_articulation_dynamic_closure_bundle_with_checkpoint_v2,
};
use crate::graph::DyadicIntervalClosureWorkspaceLimitsV2;
use crate::schedule::CycleScheduleRestrictionWorkspaceLimitsV2;

const N33_BLOCKS: usize = 33;

fn schedule_limits_v2(hinges: usize) -> CycleScheduleLimitsV1 {
    CycleScheduleLimitsV1 {
        max_hinges: hinges,
        max_degree: 1,
        max_coefficient_bits: 53,
        max_work: usize::MAX - 1,
    }
}

fn closure_limits_v2(hinges: usize) -> DyadicIntervalClosureWorkspaceLimitsV2 {
    let ceiling = usize::MAX - 1;
    DyadicIntervalClosureWorkspaceLimitsV2 {
        max_depth: 2,
        max_leaves: 4,
        max_work: ceiling,
        schedule_limits: schedule_limits_v2(hinges),
        max_theorem_recognizer_work: ceiling,
        max_theorem_recognizer_workspace_bytes: ceiling,
        max_carrier_index_workspace_bytes: ceiling,
        max_schedule_evaluation_workspace_bytes: ceiling,
        max_big_rational_payload_bytes: ceiling,
        max_exact_rational_object_bytes: ceiling,
        max_interval_closure_workspace_bytes: ceiling,
        max_partition_workspace_bytes: ceiling,
        max_retained_material_bytes: ceiling,
        max_publication_workspace_bytes: ceiling,
        max_peak_workspace_bytes: ceiling,
    }
}

fn generous_bundle_limits_v2(
    fixture: &MiuraFixtureV2,
) -> CommonArticulationDynamicClosureBundleLimitsV2 {
    let ceiling = usize::MAX - 1;
    CommonArticulationDynamicClosureBundleLimitsV2 {
        max_blocks: N33_BLOCKS,
        max_validation_work: ceiling,
        max_block_record_bytes: ceiling,
        max_total_restriction_work: ceiling,
        max_total_restricted_schedule_retained_bytes: ceiling,
        max_total_block_closure_retained_bytes: ceiling,
        max_total_block_leaves: ceiling,
        max_parent_schedule_retained_bytes: ceiling,
        max_parent_closure_retained_bytes: ceiling,
        max_parent_leaves: ceiling,
        max_bundle_retained_bytes: ceiling,
        max_issuance_peak_bytes: ceiling,
        max_revalidation_peak_bytes: ceiling,
        block_restriction_limits: CycleScheduleRestrictionWorkspaceLimitsV2 {
            max_work: ceiling,
            max_restricted_schedule_retained_bytes: ceiling,
            max_restriction_peak_bytes: ceiling,
        },
        parent_schedule_restriction_limits: CycleScheduleRestrictionWorkspaceLimitsV2 {
            max_work: ceiling,
            max_restricted_schedule_retained_bytes: ceiling,
            max_restriction_peak_bytes: ceiling,
        },
        per_block_closure_limits: closure_limits_v2(fixture.geometry.hinges().len()),
        parent_closure_limits: closure_limits_v2(fixture.geometry.hinges().len()),
    }
}

fn parent_schedule_v2(fixture: &MiuraFixtureV2) -> CanonicalCycleScheduleV1 {
    CanonicalCycleScheduleV1::prepare(
        &fixture.geometry,
        &fixture.audit,
        fixture.geometry.face_ids()[0],
        [0.0, 1.0],
        zero_cycle_schedule_entries_v2(&fixture.geometry),
        schedule_limits_v2(fixture.geometry.hinges().len()),
    )
    .expect("N33 zero parent schedule")
}

fn stationary_bundle_input_v2<'a>(
    fixture: &'a MiuraFixtureV2,
    profile: &'a CommonArticulationResourceProfileV2,
    common_pose: &'a CommonArticulationPoseAuthorityV2,
    parent_schedule: &'a CanonicalCycleScheduleV1,
    limits: CommonArticulationDynamicClosureBundleLimitsV2,
) -> CommonArticulationDynamicClosureBundleInputV2<'a> {
    CommonArticulationDynamicClosureBundleInputV2 {
        geometry: &fixture.geometry,
        audit: &fixture.audit,
        pose: &fixture.pose,
        parent_fixed_face: fixture.geometry.face_ids()[0],
        parent_schedule,
        decomposition: &fixture.decomposition,
        common_pose,
        paper_thickness_mm: 0.1,
        closure_tolerance: 0.0,
        profile,
        limits,
    }
}

fn tighten_top_level_bundle_limits_v2(
    mut limits: CommonArticulationDynamicClosureBundleLimitsV2,
    resources: crate::common_articulation_dynamic_closure_bundle_v2::CommonArticulationDynamicClosureBundleResourcesV2,
) -> CommonArticulationDynamicClosureBundleLimitsV2 {
    limits.max_block_record_bytes = resources.charged_block_record_bytes;
    limits.max_validation_work = resources.charged_validation_work;
    limits.max_total_restriction_work = resources.charged_total_restriction_work;
    limits.max_total_restricted_schedule_retained_bytes =
        resources.charged_total_restricted_schedule_retained_upper_bound_bytes;
    limits.max_total_block_closure_retained_bytes =
        resources.charged_total_block_closure_retained_upper_bound_bytes;
    limits.max_total_block_leaves = resources.charged_total_block_leaves;
    limits.max_parent_schedule_retained_bytes =
        resources.charged_parent_schedule_retained_upper_bound_bytes;
    limits.max_parent_closure_retained_bytes =
        resources.charged_parent_closure_retained_upper_bound_bytes;
    limits.max_parent_leaves = resources.charged_parent_leaves;
    limits.max_bundle_retained_bytes = resources.charged_bundle_retained_upper_bound_bytes;
    limits.max_issuance_peak_bytes = resources.charged_issuance_peak_upper_bound_bytes;
    limits.max_revalidation_peak_bytes = resources.charged_revalidation_peak_upper_bound_bytes;
    limits
}

fn nonstationary_parent_schedule_v2(
    fixture: &MiuraFixtureV2,
) -> (CanonicalCycleScheduleV1, HashSet<EdgeId>) {
    let first_block = &fixture.decomposition.blocks()[0];
    let moving = (0..3)
        .flat_map(|dominant_axis| {
            first_block
                .geometry()
                .hinges()
                .iter()
                .filter(move |hinge| {
                    [hinge.axis().x(), hinge.axis().y(), hinge.axis().z()][dominant_axis].abs()
                        == 1.0
                        && hinge.assignment() == ori_topology::FoldAssignment::Mountain
                })
                .map(move |reference| {
                    let reference_start = [
                        reference.start().x(),
                        reference.start().y(),
                        reference.start().z(),
                    ];
                    first_block
                        .geometry()
                        .hinges()
                        .iter()
                        .filter(|hinge| {
                            let axis = [hinge.axis().x(), hinge.axis().y(), hinge.axis().z()];
                            let start = [hinge.start().x(), hinge.start().y(), hinge.start().z()];
                            axis[dominant_axis].abs() == 1.0
                                && axis.iter().enumerate().all(|(dimension, value)| {
                                    dimension == dominant_axis || *value == 0.0
                                })
                                && start.iter().enumerate().all(|(dimension, value)| {
                                    dimension == dominant_axis
                                        || value.to_bits() == reference_start[dimension].to_bits()
                                })
                                && hinge.assignment() == ori_topology::FoldAssignment::Mountain
                        })
                        .map(|hinge| hinge.edge())
                        .collect::<HashSet<_>>()
                })
        })
        .find(|family| family.len() == 3)
        .expect("one all-mountain parallel carrier family");
    assert_eq!(moving.len(), 3, "one complete collinear carrier");
    let zero = RationalCoefficientV1 {
        numerator: 0,
        denominator: 1,
    };
    let slope = RationalCoefficientV1 {
        numerator: 1,
        denominator: 2,
    };
    let mut entries = fixture
        .geometry
        .hinges()
        .iter()
        .map(|hinge| {
            let moves = moving.contains(&hinge.edge());
            CycleScheduleEntryInputV1 {
                edge: hinge.edge(),
                initial_angle_degrees_bits: if moves {
                    1.0_f64.to_bits()
                } else {
                    0.0_f64.to_bits()
                },
                chebyshev_coefficients: if moves { vec![zero, slope] } else { vec![zero] },
            }
        })
        .collect::<Vec<_>>();
    entries.sort_unstable_by_key(|entry| entry.edge.canonical_bytes());
    let schedule = CanonicalCycleScheduleV1::prepare(
        &fixture.geometry,
        &fixture.audit,
        fixture.geometry.face_ids()[0],
        [-1.0, 1.0],
        entries,
        schedule_limits_v2(fixture.geometry.hinges().len()),
    )
    .expect("N33 ordinary nonstationary parent schedule");
    assert!(moving.iter().all(|edge| {
        schedule
            .derivative_bound(*edge)
            .is_some_and(|bound| bound > 0.0)
    }));
    let lower = schedule.evaluate(-1.0).unwrap();
    let upper = schedule.evaluate(1.0).unwrap();
    let moving_edge = *moving.iter().next().unwrap();
    let angle = |values: &CanonicalHingeAngles| {
        values
            .as_slice()
            .iter()
            .find(|value| value.edge() == moving_edge)
            .unwrap()
            .angle_degrees()
    };
    assert_eq!(angle(&lower).to_bits(), 0.5_f64.to_bits());
    assert_eq!(angle(&upper).to_bits(), 1.5_f64.to_bits());
    (schedule, moving)
}

#[test]
fn n33_ordinary_nonstationary_bundle_closes_every_block_and_whole_parent() {
    let fixture = miura_fixture_with_namespace_v2(
        N33_BLOCKS,
        ProjectId::schema_namespace([
            0x4f, 0x52, 0x49, 0x47, 0x41, 0x4d, 0x49, 0x32, 0x5f, 0x44, 0x43, 0x42, 0x56, 0x32, 0,
            1,
        ]),
    );
    let profile =
        CommonArticulationResourceProfileV2::exact_canonical_miura_3x3_v2(N33_BLOCKS).unwrap();
    let (parent_schedule, moving) = nonstationary_parent_schedule_v2(&fixture);
    let angles = parent_schedule.evaluate(0.0).unwrap();
    assert!(angles.as_slice().iter().any(|angle| {
        moving.contains(&angle.edge()) && angle.angle_degrees().to_bits() == 1.0_f64.to_bits()
    }));
    let pose = fixture
        .geometry
        .solve_closed(
            &fixture.audit,
            fixture.geometry.face_ids()[0],
            &angles,
            1.0e-8,
        )
        .unwrap();
    let common_pose = prove_common_articulation_pose_authority_v2(CommonArticulationPoseInputV2 {
        geometry: &fixture.geometry,
        pose: &pose,
        decomposition: &fixture.decomposition,
        paper_thickness_mm: 0.1,
        profile: &profile,
    })
    .unwrap();
    let debug_limits = generous_bundle_limits_v2(&fixture);
    let input = CommonArticulationDynamicClosureBundleInputV2 {
        geometry: &fixture.geometry,
        audit: &fixture.audit,
        pose: &pose,
        parent_fixed_face: fixture.geometry.face_ids()[0],
        parent_schedule: &parent_schedule,
        decomposition: &fixture.decomposition,
        common_pose: &common_pose,
        paper_thickness_mm: 0.1,
        closure_tolerance: 0.0,
        profile: &profile,
        limits: debug_limits,
    };
    let bundle =
        prove_common_articulation_dynamic_closure_bundle_with_checkpoint_v2(input, || Ok(()))
            .unwrap();

    for block_index in 0..N33_BLOCKS {
        let leaf = bundle.block_leaf_descriptor(block_index, 0).unwrap();
        assert_eq!((leaf.depth(), leaf.index()), (0, 0));
        assert!(bundle.block_leaf_descriptor(block_index, 1).is_none());
    }
    let parent = bundle.parent_leaf_descriptor(0).unwrap();
    assert_eq!((parent.depth(), parent.index()), (0, 0));
    assert!(bundle.parent_leaf_descriptor(1).is_none());
    assert_ne!(bundle.binding_fingerprint_v2(), [0; 32]);
}

#[test]
fn n33_owned_dynamic_bundle_issues_complete_borrowed_leaf_views() {
    let fixture = miura_fixture_v2(N33_BLOCKS);
    let profile =
        CommonArticulationResourceProfileV2::exact_canonical_miura_3x3_v2(N33_BLOCKS).unwrap();
    let common_pose = prove_common_articulation_pose_authority_v2(fixture.input(&profile)).unwrap();
    assert!(
        common_pose
            .matches_live_input_with_checkpoint_v2(fixture.input(&profile), || Ok::<(), ()>(()))
            .unwrap()
    );
    let parent_schedule = parent_schedule_v2(&fixture);
    let debug_limits = generous_bundle_limits_v2(&fixture);
    let input = CommonArticulationDynamicClosureBundleInputV2 {
        geometry: &fixture.geometry,
        audit: &fixture.audit,
        pose: &fixture.pose,
        parent_fixed_face: fixture.geometry.face_ids()[0],
        parent_schedule: &parent_schedule,
        decomposition: &fixture.decomposition,
        common_pose: &common_pose,
        paper_thickness_mm: 0.1,
        closure_tolerance: 1.0e-9,
        profile: &profile,
        limits: debug_limits,
    };
    let bundle =
        prove_common_articulation_dynamic_closure_bundle_with_checkpoint_v2(input, || Ok(()))
            .unwrap();
    let resources = bundle.resources();

    assert!(resources.charged_total_restriction_work > 0);
    assert!(resources.charged_total_restricted_schedule_retained_upper_bound_bytes > 0);
    assert_eq!(resources.charged_total_block_leaves, N33_BLOCKS);
    assert_eq!(resources.charged_parent_leaves, 1);
    assert_eq!(
        resources.charged_revalidation_peak_upper_bound_bytes,
        resources
            .charged_bundle_retained_upper_bound_bytes
            .checked_add(resources.charged_issuance_peak_upper_bound_bytes)
            .unwrap()
    );
    for block_index in 0..N33_BLOCKS {
        let leaf = bundle.block_leaf_descriptor(block_index, 0).unwrap();
        assert_eq!(leaf.depth(), 0);
        assert_eq!(leaf.index(), 0);
        assert!(bundle.block_leaf_descriptor(block_index, 1).is_none());
    }
    let parent_leaf = bundle.parent_leaf_descriptor(0).unwrap();
    assert_eq!(parent_leaf.depth(), 0);
    assert_eq!(parent_leaf.index(), 0);
    assert!(bundle.parent_leaf_descriptor(1).is_none());
}

#[test]
fn bundle_top_level_resources_are_exact_and_every_one_short_limit_fails() {
    let fixture = miura_fixture_v2(N33_BLOCKS);
    let profile =
        CommonArticulationResourceProfileV2::exact_canonical_miura_3x3_v2(N33_BLOCKS).unwrap();
    let common_pose = prove_common_articulation_pose_authority_v2(fixture.input(&profile)).unwrap();
    let parent_schedule = parent_schedule_v2(&fixture);
    let generous = generous_bundle_limits_v2(&fixture);
    let issued = prove_common_articulation_dynamic_closure_bundle_with_checkpoint_v2(
        stationary_bundle_input_v2(&fixture, &profile, &common_pose, &parent_schedule, generous),
        || Ok(()),
    )
    .unwrap();
    let mut exact = tighten_top_level_bundle_limits_v2(generous, issued.resources());
    let mut converged = false;
    for _ in 0..8 {
        let tightened_issue = prove_common_articulation_dynamic_closure_bundle_with_checkpoint_v2(
            stationary_bundle_input_v2(&fixture, &profile, &common_pose, &parent_schedule, exact),
            || Ok(()),
        )
        .expect("tightening top-level ceilings must remain admissible");
        let tightened = tighten_top_level_bundle_limits_v2(exact, tightened_issue.resources());
        if tightened == exact {
            converged = true;
            break;
        }
        exact = tightened;
    }
    assert!(
        converged,
        "top-level resource ceilings must reach a fixed point"
    );
    let exact_bundle = prove_common_articulation_dynamic_closure_bundle_with_checkpoint_v2(
        stationary_bundle_input_v2(&fixture, &profile, &common_pose, &parent_schedule, exact),
        || Ok(()),
    )
    .expect("fixed-point exact ceilings must issue");
    exact_bundle
        .revalidate_with_checkpoint_v2(
            stationary_bundle_input_v2(&fixture, &profile, &common_pose, &parent_schedule, exact),
            || Ok(()),
        )
        .expect("one complete revalidation must fit the exact validation-work ceiling");

    let block_restriction_work = (0..N33_BLOCKS)
        .map(|block_index| {
            exact_bundle
                .block_restriction_resources_v2(block_index)
                .unwrap()
                .charged_work
        })
        .max()
        .unwrap();
    let block_theorem_work = (0..N33_BLOCKS)
        .map(|block_index| {
            exact_bundle
                .block_closure_resources_v2(block_index)
                .unwrap()
                .charged_theorem_recognizer_work
        })
        .max()
        .unwrap();
    let parent_restriction_retained = exact_bundle
        .parent_restriction_resources_v2()
        .charged_restricted_schedule_retained_upper_bound_bytes;
    let parent_closure_peak = exact_bundle
        .parent_closure_resources_v2()
        .charged_peak_workspace_upper_bound_bytes;

    let mut inner_exact = exact;
    inner_exact.block_restriction_limits.max_work = block_restriction_work;
    inner_exact
        .parent_schedule_restriction_limits
        .max_restricted_schedule_retained_bytes = parent_restriction_retained;
    inner_exact
        .per_block_closure_limits
        .max_theorem_recognizer_work = block_theorem_work;
    inner_exact.parent_closure_limits.max_peak_workspace_bytes = parent_closure_peak;
    prove_common_articulation_dynamic_closure_bundle_with_checkpoint_v2(
        stationary_bundle_input_v2(
            &fixture,
            &profile,
            &common_pose,
            &parent_schedule,
            inner_exact,
        ),
        || Ok(()),
    )
    .expect("representative inner workspace ceilings must admit the measured bundle");

    let assert_inner_one_short = |limits, label| {
        assert_eq!(
            prove_common_articulation_dynamic_closure_bundle_with_checkpoint_v2(
                stationary_bundle_input_v2(
                    &fixture,
                    &profile,
                    &common_pose,
                    &parent_schedule,
                    limits,
                ),
                || Ok(()),
            )
            .expect_err(label),
            CommonArticulationDynamicClosureBundleErrorV2::ResourceLimit,
            "{label}"
        );
    };
    let mut one_short = inner_exact;
    one_short.block_restriction_limits.max_work = block_restriction_work.checked_sub(1).unwrap();
    assert_inner_one_short(one_short, "block_restriction_limits.max_work");
    let mut one_short = inner_exact;
    one_short
        .parent_schedule_restriction_limits
        .max_restricted_schedule_retained_bytes =
        parent_restriction_retained.checked_sub(1).unwrap();
    assert_inner_one_short(
        one_short,
        "parent_schedule_restriction_limits.max_restricted_schedule_retained_bytes",
    );
    let mut one_short = inner_exact;
    one_short
        .per_block_closure_limits
        .max_theorem_recognizer_work = block_theorem_work.checked_sub(1).unwrap();
    assert_inner_one_short(
        one_short,
        "per_block_closure_limits.max_theorem_recognizer_work",
    );
    let mut one_short = inner_exact;
    one_short.parent_closure_limits.max_peak_workspace_bytes =
        parent_closure_peak.checked_sub(1).unwrap();
    assert_inner_one_short(one_short, "parent_closure_limits.max_peak_workspace_bytes");

    macro_rules! assert_one_short {
        ($field:ident) => {{
            let mut one_short = exact;
            one_short.$field = one_short.$field.checked_sub(1).unwrap();
            assert_eq!(
                prove_common_articulation_dynamic_closure_bundle_with_checkpoint_v2(
                    stationary_bundle_input_v2(
                        &fixture,
                        &profile,
                        &common_pose,
                        &parent_schedule,
                        one_short,
                    ),
                    || Ok(()),
                )
                .expect_err(stringify!($field)),
                CommonArticulationDynamicClosureBundleErrorV2::ResourceLimit,
                "{}",
                stringify!($field)
            );
        }};
    }
    assert_one_short!(max_block_record_bytes);
    assert_one_short!(max_validation_work);
    assert_one_short!(max_total_restriction_work);
    assert_one_short!(max_total_restricted_schedule_retained_bytes);
    assert_one_short!(max_total_block_closure_retained_bytes);
    assert_one_short!(max_total_block_leaves);
    assert_one_short!(max_parent_schedule_retained_bytes);
    assert_one_short!(max_parent_closure_retained_bytes);
    assert_one_short!(max_parent_leaves);
    assert_one_short!(max_bundle_retained_bytes);
    assert_one_short!(max_issuance_peak_bytes);
    assert_one_short!(max_revalidation_peak_bytes);
}

#[test]
fn bundle_issuance_and_revalidation_preserve_both_stop_classes() {
    let fixture = miura_fixture_v2(N33_BLOCKS);
    let profile =
        CommonArticulationResourceProfileV2::exact_canonical_miura_3x3_v2(N33_BLOCKS).unwrap();
    let common_pose = prove_common_articulation_pose_authority_v2(fixture.input(&profile)).unwrap();
    let parent_schedule = parent_schedule_v2(&fixture);
    let limits = generous_bundle_limits_v2(&fixture);
    let input =
        stationary_bundle_input_v2(&fixture, &profile, &common_pose, &parent_schedule, limits);

    assert_eq!(
        prove_common_articulation_dynamic_closure_bundle_with_checkpoint_v2(input, || {
            Err(CommonArticulationDynamicClosureBundleStopV2::Cancelled)
        })
        .unwrap_err(),
        CommonArticulationDynamicClosureBundleErrorV2::Cancelled
    );
    assert_eq!(
        prove_common_articulation_dynamic_closure_bundle_with_checkpoint_v2(input, || {
            Err(CommonArticulationDynamicClosureBundleStopV2::DeadlineExceeded)
        })
        .unwrap_err(),
        CommonArticulationDynamicClosureBundleErrorV2::DeadlineExceeded
    );

    let bundle =
        prove_common_articulation_dynamic_closure_bundle_with_checkpoint_v2(input, || Ok(()))
            .unwrap();
    assert_eq!(
        bundle
            .revalidate_with_checkpoint_v2(input, || {
                Err(CommonArticulationDynamicClosureBundleStopV2::Cancelled)
            })
            .unwrap_err(),
        CommonArticulationDynamicClosureBundleErrorV2::Cancelled
    );
    assert_eq!(
        bundle
            .revalidate_with_checkpoint_v2(input, || {
                Err(CommonArticulationDynamicClosureBundleStopV2::DeadlineExceeded)
            })
            .unwrap_err(),
        CommonArticulationDynamicClosureBundleErrorV2::DeadlineExceeded
    );
}

#[test]
fn bundle_revalidation_rejects_value_equal_foreign_issuer_material() {
    let issuer =
        miura_fixture_with_namespace_v2(N33_BLOCKS, ProjectId::schema_namespace([0x61; 16]));
    let foreign =
        miura_fixture_with_namespace_v2(N33_BLOCKS, ProjectId::schema_namespace([0x62; 16]));
    let profile =
        CommonArticulationResourceProfileV2::exact_canonical_miura_3x3_v2(N33_BLOCKS).unwrap();
    let issuer_pose = prove_common_articulation_pose_authority_v2(issuer.input(&profile)).unwrap();
    let foreign_pose =
        prove_common_articulation_pose_authority_v2(foreign.input(&profile)).unwrap();
    let issuer_schedule = parent_schedule_v2(&issuer);
    let foreign_schedule = parent_schedule_v2(&foreign);
    let limits = generous_bundle_limits_v2(&issuer);

    let bundle = prove_common_articulation_dynamic_closure_bundle_with_checkpoint_v2(
        stationary_bundle_input_v2(&issuer, &profile, &issuer_pose, &issuer_schedule, limits),
        || Ok(()),
    )
    .unwrap();
    assert_eq!(
        bundle
            .revalidate_with_checkpoint_v2(
                stationary_bundle_input_v2(
                    &foreign,
                    &profile,
                    &foreign_pose,
                    &foreign_schedule,
                    limits,
                ),
                || Ok(()),
            )
            .unwrap_err(),
        CommonArticulationDynamicClosureBundleErrorV2::IssuerMismatch
    );
}
